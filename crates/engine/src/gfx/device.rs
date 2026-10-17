//! Physical device selection and `VkDevice` creation: pick a GPU that can render and present to
//! our surface, check it has the Vulkan 1.3 features the engine relies on, and create the logical
//! device and its queue with those features enabled.

use std::ffi::{CStr, c_char};

use ash::{khr, vk};

use super::{GfxError, Instance, Result, Surface, version_string, vk_err};

/// Lowest device API version accepted. MoltenVK 1.4 reports 1.3 (clamped to what the instance
/// asks for), so there is no need for a 1.2 + KHR-extensions fallback.
const MIN_API_VERSION: u32 = vk::API_VERSION_1_3;

/// Device extensions every candidate must have. Presenting is an extension, like surfaces are at
/// instance level.
const REQUIRED_EXTENSIONS: &[&CStr] = &[khr::swapchain::NAME];

/// The feature structs we query and enable. Since 1.2, features are grouped per core version in
/// `VkPhysicalDeviceVulkan1XFeatures` structs, chained through `pNext` from
/// `VkPhysicalDeviceFeatures2` (to query) or `VkDeviceCreateInfo` (to enable). A feature that is
/// supported is still off unless it is enabled at device creation.
#[derive(Default)]
struct FeatureChain {
    v12: vk::PhysicalDeviceVulkan12Features<'static>,
    v13: vk::PhysicalDeviceVulkan13Features<'static>,
}

/// Field accessor for one feature flag, like a C++ pointer-to-member.
type FeatureField = fn(&mut FeatureChain) -> &mut vk::Bool32;

/// Features the engine relies on, used both to reject devices that lack one and to build the
/// enable list, so the two cannot drift apart.
const REQUIRED_FEATURES: &[(&str, FeatureField)] = &[
    // Begin/end rendering into image views directly; no VkRenderPass / VkFramebuffer objects.
    ("dynamicRendering", |f| &mut f.v13.dynamic_rendering),
    // vkCmdPipelineBarrier2 and friends: stage and access masks per barrier instead of per call,
    // 64-bit flags, and simpler image layouts.
    ("synchronization2", |f| &mut f.v13.synchronization2),
    // GPU pointers to buffers (vkGetBufferDeviceAddress), readable in shaders as
    // `buffer_reference`. Memory backing such buffers must then be allocated with
    // VK_MEMORY_ALLOCATE_DEVICE_ADDRESS_BIT.
    ("bufferDeviceAddress", |f| &mut f.v12.buffer_device_address),
    // Descriptor indexing, for the bindless texture array in milestone 3: an unsized array in
    // the shader, a variable-sized binding, indices that may differ per invocation, slots that
    // can be left empty or updated after the set is bound.
    ("runtimeDescriptorArray", |f| {
        &mut f.v12.runtime_descriptor_array
    }),
    ("descriptorBindingPartiallyBound", |f| {
        &mut f.v12.descriptor_binding_partially_bound
    }),
    ("descriptorBindingVariableDescriptorCount", |f| {
        &mut f.v12.descriptor_binding_variable_descriptor_count
    }),
    ("shaderSampledImageArrayNonUniformIndexing", |f| {
        &mut f.v12.shader_sampled_image_array_non_uniform_indexing
    }),
    ("descriptorBindingSampledImageUpdateAfterBind", |f| {
        &mut f.v12.descriptor_binding_sampled_image_update_after_bind
    }),
];

/// Owns the logical device (`VkDevice`) and remembers which physical device and queue it uses.
pub struct Device {
    /// `ash::Device` is the `VkDevice` handle plus its table of device-level function pointers
    /// (from `vkGetDeviceProcAddr`), which skip both the loader's and the instance's dispatch.
    device: ash::Device,
    physical_device: vk::PhysicalDevice,
    /// The single queue family used for graphics, compute, transfer and present.
    queue_family_index: u32,
    queue: vk::Queue,
    name: String,
}

/// A physical device that passed every check, with what is needed to create the logical device.
struct Candidate {
    physical_device: vk::PhysicalDevice,
    properties: vk::PhysicalDeviceProperties,
    queue_family_index: u32,
    /// Supported VK_KHR_portability_subset features, if the device is a portability
    /// implementation (MoltenVK).
    portability_subset: Option<vk::PhysicalDevicePortabilitySubsetFeaturesKHR<'static>>,
}

impl Device {
    /// Selects the best physical device that can present to `surface` and creates the logical
    /// device on it.
    pub fn new(instance: &Instance, surface: &Surface) -> Result<Self> {
        let candidate = select_physical_device(instance, surface)?;
        let vk_instance = instance.raw();

        let mut extensions: Vec<&CStr> = REQUIRED_EXTENSIONS.to_vec();

        // One queue from the chosen family. Priorities only matter between queues of the same
        // device; with one queue any value works.
        let priorities = [1.0];
        let queue_infos = [vk::DeviceQueueCreateInfo::default()
            .queue_family_index(candidate.queue_family_index)
            .queue_priorities(&priorities)];

        let mut enabled = FeatureChain::default();
        for (_, field) in REQUIRED_FEATURES {
            *field(&mut enabled) = vk::TRUE;
        }

        // A portability implementation lists the parts of core Vulkan it cannot do as features
        // of VK_KHR_portability_subset, and the spec requires the app to enable the extension
        // whenever the device offers it. Its features are things ordinary Vulkan allows (e.g.
        // image view swizzles, triangle fans), so enable every one the device supports;
        // validation then flags only the uses that would genuinely fail on this device.
        let mut portability_features = candidate.portability_subset;
        if portability_features.is_some() {
            extensions.push(khr::portability_subset::NAME);
        }

        let extension_ptrs: Vec<*const c_char> = extensions.iter().map(|e| e.as_ptr()).collect();
        let mut create_info = vk::DeviceCreateInfo::default()
            .queue_create_infos(&queue_infos)
            .enabled_extension_names(&extension_ptrs)
            .push_next(&mut enabled.v12)
            .push_next(&mut enabled.v13);
        if let Some(features) = portability_features.as_mut() {
            create_info = create_info.push_next(features);
        }

        // SAFETY: the physical device came from this instance, and every pointer in
        // `create_info` (queue infos, feature chain, extension names) refers to locals that
        // outlive this call.
        let device =
            unsafe { vk_instance.create_device(candidate.physical_device, &create_info, None) }
                .map_err(vk_err("vkCreateDevice"))?;

        // Queues are created with the device and owned by it; this only fetches the handle.
        // SAFETY: queue 0 of this family was requested in `queue_infos`.
        let queue = unsafe { device.get_device_queue(candidate.queue_family_index, 0) };

        let name = device_name(&candidate.properties);
        log::info!(
            "Vulkan device created on {name}, queue family {}",
            candidate.queue_family_index
        );
        log::debug!("device extensions: {extensions:?}");

        Ok(Self {
            device,
            physical_device: candidate.physical_device,
            queue_family_index: candidate.queue_family_index,
            queue,
            name,
        })
    }

    /// The GPU's name, as reported by the driver.
    pub fn name(&self) -> &str {
        &self.name
    }
}

#[expect(
    dead_code,
    reason = "used from step 1.4 (swapchain) and 1.5 (frame sync)"
)]
impl Device {
    pub(super) fn raw(&self) -> &ash::Device {
        &self.device
    }

    pub(super) fn physical_device(&self) -> vk::PhysicalDevice {
        self.physical_device
    }

    pub(super) fn queue_family_index(&self) -> u32 {
        self.queue_family_index
    }

    pub(super) fn queue(&self) -> vk::Queue {
        self.queue
    }
}

impl Drop for Device {
    fn drop(&mut self) {
        // Destroying a device that still has work in flight is invalid, and Drop has no way to
        // know whether the owner waited, so wait here. An error (device lost) changes nothing:
        // the device is destroyed either way.
        // SAFETY: the device was created by `new` and is destroyed only here; everything created
        // from it is already gone (its owner drops those first).
        unsafe {
            let _ = self.device.device_wait_idle();
            self.device.destroy_device(None);
        }
    }
}

/// Checks every physical device, logs each with the reason it was rejected, and returns the best
/// remaining one: discrete GPUs first, then integrated, then anything else, in enumeration order
/// within a type.
fn select_physical_device(instance: &Instance, surface: &Surface) -> Result<Candidate> {
    // SAFETY: the instance is valid.
    let physical_devices = unsafe { instance.raw().enumerate_physical_devices() }
        .map_err(vk_err("vkEnumeratePhysicalDevices"))?;

    let mut best: Option<Candidate> = None;
    let mut rejections = Vec::new();
    for physical_device in physical_devices {
        // SAFETY: `physical_device` was just enumerated from this instance.
        let properties = unsafe {
            instance
                .raw()
                .get_physical_device_properties(physical_device)
        };
        let name = device_name(&properties);
        let description = format!(
            "{name} ({:?}, Vulkan {})",
            properties.device_type,
            version_string(properties.api_version)
        );
        match check_physical_device(instance, surface, physical_device, properties)? {
            Ok(candidate) => {
                log::info!("physical device: {description}: suitable");
                let better = best.as_ref().is_none_or(|b| {
                    type_rank(candidate.properties.device_type)
                        < type_rank(b.properties.device_type)
                });
                if better {
                    best = Some(candidate);
                }
            }
            Err(reason) => {
                log::info!("physical device: {description}: rejected, {reason}");
                rejections.push(format!("{name}: {reason}"));
            }
        }
    }

    let best = best.ok_or_else(|| {
        GfxError::NoSuitableDevice(if rejections.is_empty() {
            "no Vulkan devices found".to_owned()
        } else {
            rejections.join("; ")
        })
    })?;
    if let Some(portability) = &best.portability_subset {
        log_unsupported_portability_features(portability);
    }
    Ok(best)
}

/// Returns the candidate if the device meets every requirement, or the first unmet one. The outer
/// `Result` is for Vulkan calls that failed; the inner one is the verdict.
fn check_physical_device(
    instance: &Instance,
    surface: &Surface,
    physical_device: vk::PhysicalDevice,
    properties: vk::PhysicalDeviceProperties,
) -> Result<std::result::Result<Candidate, String>> {
    let vk_instance = instance.raw();

    // Check the version first: chaining VkPhysicalDeviceVulkan13Features into the query below
    // is only valid on a 1.3 device.
    if properties.api_version < MIN_API_VERSION {
        return Ok(Err(format!(
            "needs Vulkan {}",
            version_string(MIN_API_VERSION)
        )));
    }

    // SAFETY: `physical_device` belongs to this instance.
    let available_extensions =
        unsafe { vk_instance.enumerate_device_extension_properties(physical_device) }
            .map_err(vk_err("vkEnumerateDeviceExtensionProperties"))?;
    let has_extension = |name: &CStr| {
        available_extensions
            .iter()
            .any(|e| e.extension_name_as_c_str() == Ok(name))
    };
    if let Some(missing) = REQUIRED_EXTENSIONS.iter().find(|&&e| !has_extension(e)) {
        return Ok(Err(format!(
            "missing extension {}",
            missing.to_string_lossy()
        )));
    }

    // Required features. vkGetPhysicalDeviceFeatures2 fills in every struct on the pNext chain.
    let mut supported = FeatureChain::default();
    let has_portability_subset = has_extension(khr::portability_subset::NAME);
    let mut portability = vk::PhysicalDevicePortabilitySubsetFeaturesKHR::default();
    {
        let mut features2 = vk::PhysicalDeviceFeatures2::default()
            .push_next(&mut supported.v12)
            .push_next(&mut supported.v13);
        if has_portability_subset {
            features2 = features2.push_next(&mut portability);
        }
        // SAFETY: the device supports 1.3 (checked above), so every struct on the chain is
        // valid for it, and the chain points at locals that outlive the call.
        unsafe { vk_instance.get_physical_device_features2(physical_device, &mut features2) };
    }
    // The query linked the structs' pNext pointers to each other. They are only read from here
    // on, never passed to Vulkan again, so clear the links rather than leave dangling pointers in
    // values that get moved and copied.
    supported.v12.p_next = std::ptr::null_mut();
    supported.v13.p_next = std::ptr::null_mut();
    portability.p_next = std::ptr::null_mut();

    let missing_features: Vec<&str> = REQUIRED_FEATURES
        .iter()
        .filter(|(_, field)| *field(&mut supported) == vk::FALSE)
        .map(|(name, _)| *name)
        .collect();
    if !missing_features.is_empty() {
        return Ok(Err(format!(
            "missing features {}",
            missing_features.join(", ")
        )));
    }

    // A queue family that can do graphics and present to this surface. Present support is per
    // family *and* per surface (a GPU may not be wired to the display a window is on), so it is
    // a surface query, not a family flag. Graphics families always support compute and transfer
    // too (the spec guarantees at least one family with all three), so this one queue can be
    // used for everything until we want async compute.
    // SAFETY: `physical_device` belongs to this instance.
    let families =
        unsafe { vk_instance.get_physical_device_queue_family_properties(physical_device) };
    let mut queue_family_index = None;
    for (index, family) in (0u32..).zip(&families) {
        if family.queue_flags.contains(vk::QueueFlags::GRAPHICS)
            && surface.supports_present(physical_device, index)?
        {
            queue_family_index = Some(index);
            break;
        }
    }
    let Some(queue_family_index) = queue_family_index else {
        return Ok(Err(
            "no queue family with both graphics and present".to_owned()
        ));
    };

    Ok(Ok(Candidate {
        physical_device,
        properties,
        queue_family_index,
        portability_subset: has_portability_subset.then_some(portability),
    }))
}

/// Lower is better.
fn type_rank(device_type: vk::PhysicalDeviceType) -> u32 {
    match device_type {
        vk::PhysicalDeviceType::DISCRETE_GPU => 0,
        vk::PhysicalDeviceType::INTEGRATED_GPU => 1,
        vk::PhysicalDeviceType::VIRTUAL_GPU => 2,
        vk::PhysicalDeviceType::CPU => 3,
        _ => 4,
    }
}

fn device_name(properties: &vk::PhysicalDeviceProperties) -> String {
    properties
        .device_name_as_c_str()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_default()
}

/// Logs the parts of core Vulkan a portability implementation cannot do, so they are visible
/// before some later step trips over one.
fn log_unsupported_portability_features(f: &vk::PhysicalDevicePortabilitySubsetFeaturesKHR) {
    let features = [
        (
            "constantAlphaColorBlendFactors",
            f.constant_alpha_color_blend_factors,
        ),
        ("events", f.events),
        (
            "imageViewFormatReinterpretation",
            f.image_view_format_reinterpretation,
        ),
        ("imageViewFormatSwizzle", f.image_view_format_swizzle),
        ("imageView2DOn3DImage", f.image_view2_d_on3_d_image),
        ("multisampleArrayImage", f.multisample_array_image),
        ("mutableComparisonSamplers", f.mutable_comparison_samplers),
        ("pointPolygons", f.point_polygons),
        ("samplerMipLodBias", f.sampler_mip_lod_bias),
        ("separateStencilMaskRef", f.separate_stencil_mask_ref),
        (
            "shaderSampleRateInterpolationFunctions",
            f.shader_sample_rate_interpolation_functions,
        ),
        ("tessellationIsolines", f.tessellation_isolines),
        ("tessellationPointMode", f.tessellation_point_mode),
        ("triangleFans", f.triangle_fans),
        (
            "vertexAttributeAccessBeyondStride",
            f.vertex_attribute_access_beyond_stride,
        ),
    ];
    let unsupported: Vec<&str> = features
        .iter()
        .filter(|(_, supported)| *supported == vk::FALSE)
        .map(|(name, _)| *name)
        .collect();
    log::info!(
        "portability subset device; unsupported: {}",
        if unsupported.is_empty() {
            "none".to_owned()
        } else {
            unsupported.join(", ")
        }
    );
}
