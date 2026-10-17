//! The renderer: owns all GPU state and draws a frame when asked.
//!
//! The host app owns the window and the event loop and calls `resize` and `render`; the renderer
//! never sees windowing events directly, so it can be driven by any host (our viewer, another
//! engine). So far it creates the Vulkan instance, the window surface and the device.

use raw_window_handle::{RawDisplayHandle, RawWindowHandle};

use crate::gfx::{self, Device, Instance, Surface};

pub struct Renderer {
    // Rust drops fields in declaration order (C++ destroys members in the reverse order), and
    // Vulkan objects must be destroyed before the instance they came from. So objects created
    // from the instance are declared above it. The device and the surface do not depend on each
    // other, but the swapchain (1.4) depends on both and goes above them.
    #[expect(dead_code, reason = "used from step 1.4 (swapchain)")]
    device: Device,
    #[expect(dead_code, reason = "used from step 1.4 (swapchain)")]
    surface: Surface,
    #[expect(dead_code, reason = "kept alive for the objects above")]
    instance: Instance,

    /// Size of the drawable area in physical pixels. Can be 0x0 when the window is minimised.
    width: u32,
    height: u32,
    /// Frames rendered since creation.
    frame_index: u64,
}

impl Renderer {
    /// Creates the renderer for a native window, given its raw handles and drawable size in
    /// physical pixels.
    ///
    /// This takes raw handles and is `unsafe` rather than borrowing the window (`Renderer<'w>`
    /// holding `&'w Window`) because the host typically stores the window and the renderer side
    /// by side in one struct, and Rust does not allow a struct field to borrow another field of
    /// the same struct. The lifetime rule is still there; it moves from the type system into this
    /// contract, which is also how a C++ host would see it.
    ///
    /// # Safety
    ///
    /// `display` and `window` must be valid handles for a window that outlives the renderer.
    pub unsafe fn new(
        display: RawDisplayHandle,
        window: RawWindowHandle,
        width: u32,
        height: u32,
    ) -> gfx::Result<Self> {
        let instance = Instance::new(display)?;
        // SAFETY: the window outlives the renderer (this function's contract), and `surface` is
        // declared before `instance` in `Renderer`, so it is dropped first.
        let surface = unsafe { Surface::new(&instance, display, window)? };
        let device = Device::new(&instance, &surface)?;
        log::info!("renderer created at {width}x{height}");
        Ok(Self {
            device,
            surface,
            instance,
            width,
            height,
            frame_index: 0,
        })
    }

    /// Called when the window's drawable size changes. From step 1.4 this is where the swapchain is
    /// marked out of date.
    pub fn resize(&mut self, width: u32, height: u32) {
        if (width, height) != (self.width, self.height) {
            log::debug!("resize {width}x{height}");
            self.width = width;
            self.height = height;
        }
    }

    /// Whether there is anything to draw into. A minimised window has a 0x0 drawable, and Vulkan
    /// cannot create a swapchain of that size, so the host should skip `render` while this is
    /// false.
    pub fn can_render(&self) -> bool {
        self.width > 0 && self.height > 0
    }

    /// Draw one frame. `dt` is the time since the previous frame, in seconds.
    pub fn render(&mut self, dt: f32) {
        log::trace!("frame {} dt {:.3} ms", self.frame_index, dt * 1000.0);
        self.frame_index += 1;
    }

    pub fn frame_index(&self) -> u64 {
        self.frame_index
    }
}
