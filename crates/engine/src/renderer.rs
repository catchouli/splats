//! The renderer: owns all GPU state and draws a frame when asked.
//!
//! For now (step 1.1) it only counts frames. From step 1.2 it takes the window's raw handles,
//! creates the Vulkan instance and surface, and grows from there. The host app owns the window and
//! the event loop and calls `resize` and `render`; the renderer never sees windowing events
//! directly, so it can be driven by any host (our viewer, a headless test, another engine).

pub struct Renderer {
    /// Size of the drawable area in physical pixels. Can be 0x0 when the window is minimised.
    width: u32,
    height: u32,
    /// Frames rendered since creation.
    frame_index: u64,
}

impl Renderer {
    pub fn new(width: u32, height: u32) -> Self {
        log::info!("renderer created at {width}x{height}");
        Self {
            width,
            height,
            frame_index: 0,
        }
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
