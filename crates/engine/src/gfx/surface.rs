//! `VkSurfaceKHR`: the instance-level object that represents a window as something Vulkan can
//! present to. The swapchain (step 1.4) is created from it.

use ash::{khr, vk};
use raw_window_handle::{RawDisplayHandle, RawWindowHandle};

use super::{Instance, Result, vk_err};

pub struct Surface {
    /// Function table for `VK_KHR_surface` (destroy, and the capability queries used in 1.4).
    loader: khr::surface::Instance,
    surface: vk::SurfaceKHR,
}

impl Surface {
    /// Creates a surface for a native window. On macOS, ash-window attaches a `CAMetalLayer` to
    /// the window's `NSView` and creates the surface from that layer (VK_EXT_metal_surface).
    ///
    /// # Safety
    ///
    /// `display` and `window` must be valid handles for a window that stays alive until this
    /// surface is dropped, and the surface must be dropped before `instance`.
    pub unsafe fn new(
        instance: &Instance,
        display: RawDisplayHandle,
        window: RawWindowHandle,
    ) -> Result<Self> {
        // SAFETY: the handles are valid and outlive the surface (caller's contract), and the
        // instance was created with the surface extensions for `display`.
        let surface = unsafe {
            ash_window::create_surface(instance.entry(), instance.raw(), display, window, None)
        }
        .map_err(vk_err("ash_window::create_surface"))?;
        let loader = khr::surface::Instance::new(instance.entry(), instance.raw());
        Ok(Self { loader, surface })
    }

    pub fn handle(&self) -> vk::SurfaceKHR {
        self.surface
    }
}

impl Drop for Surface {
    fn drop(&mut self) {
        // SAFETY: the surface was created by `new`, is destroyed only here, and the instance is
        // still alive (caller's contract on `new`).
        unsafe { self.loader.destroy_surface(self.surface, None) };
    }
}
