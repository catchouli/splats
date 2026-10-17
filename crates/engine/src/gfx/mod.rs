//! Vulkan backend. The `unsafe` Vulkan calls live under this module, wrapped in small types that
//! each own one or a few Vulkan objects and destroy them on drop.

mod device;
mod instance;
mod surface;

pub use device::Device;
pub use instance::Instance;
pub use surface::Surface;

use ash::vk;

#[derive(Debug, thiserror::Error)]
pub enum GfxError {
    #[error("could not load the Vulkan loader library (is the Vulkan SDK installed?): {0}")]
    Loading(#[from] ash::LoadingError),
    #[error("{call} failed: {result}")]
    Vulkan {
        call: &'static str,
        result: vk::Result,
    },
    #[error("required Vulkan instance extension {0} is not available")]
    MissingExtension(String),
    #[error("no suitable Vulkan device: {0}")]
    NoSuitableDevice(String),
}

pub type Result<T> = std::result::Result<T, GfxError>;

/// `map_err` adapter for a failed Vulkan call, naming the call in the error:
/// `unsafe { ... }.map_err(vk_err("vkCreateInstance"))?`.
fn vk_err(call: &'static str) -> impl FnOnce(vk::Result) -> GfxError {
    move |result| GfxError::Vulkan { call, result }
}

/// Formats a packed Vulkan version number as "major.minor.patch".
fn version_string(version: u32) -> String {
    format!(
        "{}.{}.{}",
        vk::api_version_major(version),
        vk::api_version_minor(version),
        vk::api_version_patch(version)
    )
}
