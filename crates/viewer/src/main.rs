//! Test-bed viewer: owns the window and the event loop, and drives the engine's `Renderer`.
//!
//! winit 0.30 structure: instead of passing a closure to `EventLoop::run` (the pre-0.30 API most
//! tutorials show), the app implements the `ApplicationHandler` trait and winit calls its methods.
//! Windows can only be created from inside those callbacks (they need an `ActiveEventLoop`), which
//! is why `window` and `renderer` start as `None` and are filled in by `resumed`. On desktop
//! `resumed` fires once at startup; on mobile it fires every time the app comes back to the
//! foreground, when the surface has to be recreated.

use std::time::Instant;

use anyhow::Result;
use engine::Renderer;
use tracing_subscriber::EnvFilter;
use winit::application::ApplicationHandler;
use winit::dpi::LogicalSize;
use winit::event::{ElementState, KeyEvent, WindowEvent};
use winit::event_loop::{ActiveEventLoop, ControlFlow, EventLoop};
use winit::keyboard::{Key, NamedKey};
use winit::raw_window_handle::{HasDisplayHandle, HasWindowHandle};
use winit::window::{Window, WindowId};

/// Longest frame time passed to `render`. After a stall (a breakpoint, the window being dragged on
/// Windows, which blocks the loop) a huge `dt` would make anything time-based jump; clamp it
/// instead.
const MAX_DT: f32 = 0.25;

struct App {
    // Declared before `window` so it is dropped first if `App` is dropped without `exiting` having
    // run: the renderer's Vulkan surface must not outlive the native window.
    renderer: Option<Renderer>,
    window: Option<Window>,
    last_frame: Instant,
    /// Frames and time accumulated since the last frame-rate log line.
    stats_frames: u32,
    stats_start: Instant,
    /// The trait callbacks cannot return errors, so a fatal error is stored here, the loop is asked
    /// to exit, and `main` returns it.
    error: Option<anyhow::Error>,
}

impl App {
    fn new() -> Self {
        let now = Instant::now();
        Self {
            renderer: None,
            window: None,
            last_frame: now,
            stats_frames: 0,
            stats_start: now,
            error: None,
        }
    }

    fn create_window(&mut self, event_loop: &ActiveEventLoop) -> Result<()> {
        let attributes = Window::default_attributes()
            .with_title(format!("splats viewer (engine {})", engine::VERSION))
            .with_inner_size(LogicalSize::new(1280.0, 720.0));
        let window = event_loop.create_window(attributes)?;

        // Logical size is in points; the renderer works in physical pixels. On a Retina display
        // these differ by the scale factor (2.0).
        let size = window.inner_size();
        log::info!(
            "window {}x{} px, scale factor {}",
            size.width,
            size.height,
            window.scale_factor()
        );

        let display = window.display_handle()?.as_raw();
        let handle = window.window_handle()?.as_raw();
        // SAFETY: the handles come from `window`, which `App` keeps alive for as long as the
        // renderer: the renderer is dropped first in `exiting` and in `App`'s field order.
        let renderer = unsafe { Renderer::new(display, handle, size.width, size.height)? };

        self.renderer = Some(renderer);
        self.window = Some(window);
        self.last_frame = Instant::now();
        self.stats_start = self.last_frame;
        Ok(())
    }

    fn redraw(&mut self) {
        let Some(renderer) = self.renderer.as_mut() else {
            return;
        };

        let now = Instant::now();
        let dt = (now - self.last_frame).as_secs_f32().min(MAX_DT);
        self.last_frame = now;

        if renderer.can_render() {
            let _frame = tracing::trace_span!("frame", index = renderer.frame_index()).entered();
            renderer.render(dt);
            self.stats_frames += 1;
        }

        let elapsed = (now - self.stats_start).as_secs_f32();
        if elapsed >= 1.0 {
            log::debug!(
                "{:.1} fps ({:.2} ms/frame)",
                self.stats_frames as f32 / elapsed,
                elapsed * 1000.0 / self.stats_frames.max(1) as f32
            );
            self.stats_frames = 0;
            self.stats_start = now;
        }
    }
}

impl ApplicationHandler for App {
    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        if self.window.is_some() {
            return;
        }
        if let Err(e) = self.create_window(event_loop) {
            self.error = Some(e);
            event_loop.exit();
        }
    }

    fn window_event(&mut self, event_loop: &ActiveEventLoop, _id: WindowId, event: WindowEvent) {
        match event {
            WindowEvent::CloseRequested
            | WindowEvent::KeyboardInput {
                event:
                    KeyEvent {
                        logical_key: Key::Named(NamedKey::Escape),
                        state: ElementState::Pressed,
                        ..
                    },
                ..
            } => {
                log::info!("exit requested");
                event_loop.exit();
            }
            WindowEvent::Resized(size) => {
                if let Some(renderer) = self.renderer.as_mut() {
                    renderer.resize(size.width, size.height);
                }
            }
            WindowEvent::RedrawRequested => self.redraw(),
            _ => {}
        }
    }

    /// Called once the event queue has been drained. Asking for a redraw here gives a continuous
    /// frame loop: process all input, then draw one frame. Until the swapchain exists (step 1.4)
    /// nothing paces this, so with `ControlFlow::Poll` the loop runs as fast as the OS delivers
    /// redraws.
    fn about_to_wait(&mut self, _event_loop: &ActiveEventLoop) {
        if let Some(window) = self.window.as_ref() {
            window.request_redraw();
        }
    }

    fn exiting(&mut self, _event_loop: &ActiveEventLoop) {
        if let Some(renderer) = self.renderer.as_ref() {
            log::info!("exiting after {} frames", renderer.frame_index());
        }
        // Drop the renderer before the window: it holds a Vulkan surface that must be destroyed
        // while the native window still exists.
        self.renderer = None;
        self.window = None;
    }
}

fn main() -> Result<()> {
    // The subscriber is the sink for both `tracing` events and, through the `tracing-log` bridge
    // that `init()` installs, `log` macros. Filter with RUST_LOG, e.g. `RUST_LOG=debug` or
    // `RUST_LOG=engine=trace,info`.
    tracing_subscriber::fmt()
        .with_env_filter(
            EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info")),
        )
        .init();

    let event_loop = EventLoop::new()?;
    // Poll: run the loop continuously rather than sleeping until an OS event arrives (`Wait`),
    // which is what an editor-style app would use.
    event_loop.set_control_flow(ControlFlow::Poll);

    let mut app = App::new();
    event_loop.run_app(&mut app)?;

    match app.error {
        Some(e) => Err(e),
        None => Ok(()),
    }
}
