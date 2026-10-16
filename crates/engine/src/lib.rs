//! Rendering framework library.
//!
//! Everything engine-side lives here so it stays separable from the viewer app (and portable to
//! other hosts). Modules are added as `PLAN.md` steps land: `gfx` for the Vulkan backend in
//! milestone 1, `scene` in milestone 2.

pub mod gfx;
mod renderer;

pub use gfx::GfxError;
pub use renderer::Renderer;

/// Crate version, so the viewer can report which engine it was built with.
pub const VERSION: &str = env!("CARGO_PKG_VERSION");
