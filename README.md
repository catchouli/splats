# splats

A small Rust 3D rendering framework (Vulkan via `ash`, a data-oriented
scenegraph, PBR materials, glTF loading) built as a test bed for Gaussian
splatting, with CUDA for the heavy GPU work. Runs on Windows and on macOS
through MoltenVK; CUDA features are Windows/Linux only. See [PLAN.md](PLAN.md)
for the roadmap and [AGENTS.md](AGENTS.md) for how AI agents work on this repo.
