//! `VkInstance` creation: loading the Vulkan loader, the instance extensions needed to present to
//! a window, the portability flag MoltenVK needs, and validation with a debug messenger in debug
//! builds.

use std::ffi::{CStr, c_char, c_void};

use ash::{ext, khr, vk};
use raw_window_handle::RawDisplayHandle;

use super::{GfxError, Result, version_string, vk_err};

const VALIDATION_LAYER: &CStr = c"VK_LAYER_KHRONOS_validation";

/// The highest Vulkan version the engine is written against. For the instance this is a promise
/// about what we will use, not a requirement: any loader newer than 1.0 accepts it. Whether the
/// device actually supports 1.3 is checked at device selection (step 1.3).
const API_VERSION: u32 = vk::API_VERSION_1_3;

/// Owns the Vulkan loader (`ash::Entry`), the `VkInstance` and, when validation is on, the debug
/// messenger.
pub struct Instance {
    /// The loaded loader library. `instance` holds function pointers into it, so it must stay
    /// alive as long as `instance`; it is dropped after `Drop::drop` has destroyed the instance.
    entry: ash::Entry,
    /// `ash::Instance` is the `VkInstance` handle plus its table of instance-level function
    /// pointers (fetched with `vkGetInstanceProcAddr`), so calls skip the loader's dispatch.
    instance: ash::Instance,
    debug_utils: Option<DebugUtils>,
}

struct DebugUtils {
    /// Function table for `VK_EXT_debug_utils`. Extension functions are not part of
    /// `ash::Instance`; each extension has its own loader struct in ash.
    loader: ext::debug_utils::Instance,
    messenger: vk::DebugUtilsMessengerEXT,
}

impl Instance {
    /// Loads Vulkan and creates an instance with the extensions needed to create a surface for
    /// windows on `display` (from the windowing system, e.g. AppKit, Win32 or Wayland).
    pub fn new(display: RawDisplayHandle) -> Result<Self> {
        let entry = load_entry()?;

        // SAFETY: global query with no preconditions.
        let loader_version = unsafe { entry.try_enumerate_instance_version() }
            .map_err(vk_err("vkEnumerateInstanceVersion"))?
            .unwrap_or(vk::API_VERSION_1_0);
        log::info!("Vulkan loader {}", version_string(loader_version));

        // SAFETY: global query with no preconditions.
        let available_extensions = unsafe { entry.enumerate_instance_extension_properties(None) }
            .map_err(vk_err("vkEnumerateInstanceExtensionProperties"))?;
        let is_available = |name: &CStr| {
            available_extensions
                .iter()
                .any(|e| e.extension_name_as_c_str() == Ok(name))
        };

        // Surface extensions: VK_KHR_surface plus the platform one (VK_EXT_metal_surface on macOS,
        // VK_KHR_win32_surface on Windows). Surfaces are instance extensions, not core, because
        // Vulkan can run headless.
        let mut extensions: Vec<&CStr> = Vec::new();
        let surface_extensions = ash_window::enumerate_required_extensions(display)
            .map_err(vk_err("ash_window::enumerate_required_extensions"))?;
        for &name in surface_extensions {
            // SAFETY: ash-window returns pointers to its 'static, NUL-terminated extension names.
            let name = unsafe { CStr::from_ptr(name) };
            if !is_available(name) {
                return Err(GfxError::MissingExtension(
                    name.to_string_lossy().into_owned(),
                ));
            }
            extensions.push(name);
        }

        // Portability enumeration. Since loader 1.3.216 the loader hides "portability" drivers,
        // ones that do not implement all of Vulkan, such as MoltenVK, unless the app opts in with
        // this extension and the matching create flag. Without it, enumerating physical devices
        // on a Mac returns none. Enabling it elsewhere is harmless (it only adds portability
        // drivers to the list), so it is decided at runtime rather than per platform.
        let mut flags = vk::InstanceCreateFlags::empty();
        if is_available(khr::portability_enumeration::NAME) {
            extensions.push(khr::portability_enumeration::NAME);
            flags |= vk::InstanceCreateFlags::ENUMERATE_PORTABILITY_KHR;
        }

        // Validation, debug builds only. The validation layer checks every call against the
        // spec's valid-usage rules and reports through VK_EXT_debug_utils. A missing layer is a
        // warning, not an error, so a debug build still runs on a machine without the SDK.
        let mut layers: Vec<&CStr> = Vec::new();
        let mut debug_utils_enabled = false;
        if cfg!(debug_assertions) {
            // SAFETY: global query with no preconditions.
            let available_layers = unsafe { entry.enumerate_instance_layer_properties() }
                .map_err(vk_err("vkEnumerateInstanceLayerProperties"))?;
            if available_layers
                .iter()
                .any(|l| l.layer_name_as_c_str() == Ok(VALIDATION_LAYER))
            {
                layers.push(VALIDATION_LAYER);
            } else {
                log::warn!(
                    "{} not found, running without validation (see docs/SETUP.md)",
                    VALIDATION_LAYER.to_string_lossy()
                );
            }
            if is_available(ext::debug_utils::NAME) {
                extensions.push(ext::debug_utils::NAME);
                debug_utils_enabled = true;
            }
        }

        let app_info = vk::ApplicationInfo::default()
            .application_name(c"splats")
            .engine_name(c"splats")
            .api_version(API_VERSION);
        let extension_ptrs: Vec<*const c_char> = extensions.iter().map(|e| e.as_ptr()).collect();
        let layer_ptrs: Vec<*const c_char> = layers.iter().map(|l| l.as_ptr()).collect();

        let mut create_info = vk::InstanceCreateInfo::default()
            .flags(flags)
            .application_info(&app_info)
            .enabled_extension_names(&extension_ptrs)
            .enabled_layer_names(&layer_ptrs);
        // A messenger can only be created once the instance exists, so on its own it would miss
        // anything reported by vkCreateInstance and vkDestroyInstance. Chaining a messenger
        // create-info into the instance's pNext chain makes the layer report those two calls
        // through a temporary messenger.
        let mut instance_messenger_info = messenger_create_info();
        if debug_utils_enabled {
            create_info = create_info.push_next(&mut instance_messenger_info);
        }

        // SAFETY: every pointer in `create_info` refers to locals that outlive this call.
        let instance = unsafe { entry.create_instance(&create_info, None) }
            .map_err(vk_err("vkCreateInstance"))?;

        log::info!(
            "Vulkan instance created (API {}, validation {})",
            version_string(API_VERSION),
            if layers.is_empty() { "off" } else { "on" }
        );
        log::debug!("instance layers: {layers:?}");
        log::debug!("instance extensions: {extensions:?}");

        // From here `Self` owns the instance, so an early return below (the `?`) drops it and
        // `Drop` destroys the instance: no manual cleanup on the error path.
        let mut this = Self {
            entry,
            instance,
            debug_utils: None,
        };

        if debug_utils_enabled {
            let loader = ext::debug_utils::Instance::new(&this.entry, &this.instance);
            // SAFETY: the instance is valid and was created with VK_EXT_debug_utils enabled.
            let messenger =
                unsafe { loader.create_debug_utils_messenger(&messenger_create_info(), None) }
                    .map_err(vk_err("vkCreateDebugUtilsMessengerEXT"))?;
            this.debug_utils = Some(DebugUtils { loader, messenger });
        }

        Ok(this)
    }

    pub(super) fn entry(&self) -> &ash::Entry {
        &self.entry
    }

    pub(super) fn raw(&self) -> &ash::Instance {
        &self.instance
    }
}

impl Drop for Instance {
    fn drop(&mut self) {
        // Children before parents: the messenger belongs to the instance. Everything else created
        // from the instance (surfaces, devices) must already be gone, which the owner of
        // `Instance` guarantees by dropping those first.
        // SAFETY: the messenger and instance were created by `new` and are destroyed only here.
        unsafe {
            if let Some(debug_utils) = self.debug_utils.take() {
                debug_utils
                    .loader
                    .destroy_debug_utils_messenger(debug_utils.messenger, None);
            }
            self.instance.destroy_instance(None);
        }
    }
}

/// Opens the Vulkan loader library. `Entry::load` opens it by name (vulkan-1.dll, libvulkan.so.1,
/// libvulkan.dylib) at runtime, so the binary has no link-time dependency on the loader.
fn load_entry() -> Result<ash::Entry> {
    // SAFETY: opening the library runs its initialisers; the Khronos loader has no preconditions
    // for that. The same holds for the explicit paths below.
    let entry = unsafe { ash::Entry::load() };

    // On macOS a bare library name is not searched for in /usr/local/lib, where the SDK's global
    // install puts the loader: dyld's default fallback path is only /usr/lib, and the DYLD_*
    // variables that would extend it are stripped by SIP. So try the install locations by full
    // path: the global install, then the SDK folder if `VULKAN_SDK` is set (setup-env.sh).
    #[cfg(target_os = "macos")]
    let entry = entry.or_else(|default_err| {
        let mut paths = vec![std::path::PathBuf::from("/usr/local/lib/libvulkan.dylib")];
        if let Some(sdk) = std::env::var_os("VULKAN_SDK") {
            paths.push(std::path::Path::new(&sdk).join("lib/libvulkan.dylib"));
        }
        for path in paths {
            // SAFETY: as above.
            if let Ok(entry) = unsafe { ash::Entry::load_from(&path) } {
                log::debug!("loaded the Vulkan loader from {}", path.display());
                return Ok(entry);
            }
        }
        Err(default_err)
    });

    Ok(entry?)
}

/// Messenger settings shared by the instance-creation messenger and the persistent one. All
/// severities are requested and filtering is left to the `log` level, under the target `vulkan`
/// (e.g. `RUST_LOG=vulkan=trace,info`).
fn messenger_create_info() -> vk::DebugUtilsMessengerCreateInfoEXT<'static> {
    vk::DebugUtilsMessengerCreateInfoEXT::default()
        .message_severity(
            vk::DebugUtilsMessageSeverityFlagsEXT::ERROR
                | vk::DebugUtilsMessageSeverityFlagsEXT::WARNING
                | vk::DebugUtilsMessageSeverityFlagsEXT::INFO
                | vk::DebugUtilsMessageSeverityFlagsEXT::VERBOSE,
        )
        .message_type(
            vk::DebugUtilsMessageTypeFlagsEXT::GENERAL
                | vk::DebugUtilsMessageTypeFlagsEXT::VALIDATION
                | vk::DebugUtilsMessageTypeFlagsEXT::PERFORMANCE,
        )
        .pfn_user_callback(Some(debug_callback))
}

/// Called by the validation layer (and the loader) for each message, on whichever thread made
/// the Vulkan call. It must not unwind: a panic crossing an `extern "system"` boundary aborts.
unsafe extern "system" fn debug_callback(
    severity: vk::DebugUtilsMessageSeverityFlagsEXT,
    types: vk::DebugUtilsMessageTypeFlagsEXT,
    callback_data: *const vk::DebugUtilsMessengerCallbackDataEXT<'_>,
    _user_data: *mut c_void,
) -> vk::Bool32 {
    // SAFETY: the spec guarantees `callback_data` points to a valid struct for the duration of
    // the callback, and its strings are NUL-terminated or null.
    let (id, message) = unsafe {
        let data = &*callback_data;
        (
            data.message_id_name_as_c_str()
                .map(CStr::to_string_lossy)
                .unwrap_or_default(),
            data.message_as_c_str()
                .map(CStr::to_string_lossy)
                .unwrap_or_default(),
        )
    };

    // INFO maps to debug, not info: the loader reports every layer and driver it loads at INFO.
    let level = if severity.contains(vk::DebugUtilsMessageSeverityFlagsEXT::ERROR) {
        log::Level::Error
    } else if severity.contains(vk::DebugUtilsMessageSeverityFlagsEXT::WARNING) {
        log::Level::Warn
    } else if severity.contains(vk::DebugUtilsMessageSeverityFlagsEXT::INFO) {
        log::Level::Debug
    } else {
        log::Level::Trace
    };
    log::log!(target: "vulkan", level, "[{types:?}] {id}: {message}");

    // Returning TRUE would make the layer fail the call with VK_ERROR_VALIDATION_FAILED_EXT; the
    // spec reserves that for layer development, so applications always return FALSE.
    vk::FALSE
}
