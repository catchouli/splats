# Instructions for AI agents working on this project

This file is for Claude Code and any other coding agent working in this
repository. `PLAN.md` says what we are building; this file says how we work.

## What this project is

A Rust 3D rendering framework (Vulkan via `ash`, scenegraph, PBR, glTF) built
as a test bed for Gaussian Splatting, with CUDA used for the heavy GPU work and
Vulkan/CUDA interop to display the results. See `PLAN.md` for the milestones
and the technical decisions already taken.

## Who you are working with

The developer is a professional 3D graphics programmer. They understand
rendering, scenegraphs, GPU architecture and the maths; there is no need to
explain what a BRDF or a frustum is. What they are deliberately refreshing
and learning is:

- **Modern Vulkan** (1.3-era: dynamic rendering, synchronization2, bindless
  descriptors, GPU-driven rendering). Their API knowledge predates Vulkan.
- **Modern CUDA.** They did GPU volume rendering in CUDA over a decade ago
  (persistent threads era) so the fundamentals are there, but cooperative
  groups, warp intrinsics, unified memory, CUB, Nsight, and the current best
  practices are new. CUDA is a stepping stone towards implementing machine
  learning models from scratch, so when there is a choice, prefer doing GPU
  work explicitly (writing the kernel, the sort, the reduction) over pulling
  in a library that hides it, and say what the library equivalent would be.
- **Rust** in a systems/graphics context. Assume comfortable but not expert.
  Explain borrow-checker-driven design choices when they shape an API.

Some of what is built here may later be ported to a C++ engine, so engine
code should be straightforward to translate: plain data structures, explicit
control flow, few trait-heavy abstractions, and comments that describe the
algorithm rather than the Rust idiom.

## How to work

**One step at a time.** Work on one `PLAN.md` step per request unless told
otherwise. Before starting a step, say in a few lines what you are about to
build and any design decision you are making. After finishing it, summarise
what was done, what to look at, and how to run it. The developer wants to
follow the development closely, so prefer a sequence of small, reviewable
changes over one big one.

**Never commit automatically.** The developer reviews all code before it is
committed. Do not run `git commit`, `git push`, or anything that rewrites
history unless explicitly asked in that conversation. Staging (`git add`) is
fine if it helps the review. When a step is done, say so and stop; the
developer will review and commit.

**Explain as you go.** When you touch something the developer is learning
(Vulkan, CUDA), include short comments in the code explaining *why* the API
is used that way, especially where it differs from older APIs (OpenGL, old
CUDA), and mention gotchas in your summary. Do not pad the code with comments
on things a graphics programmer already knows.

**Keep the plan current.** When a step is completed, tick it in `PLAN.md`.
If the implementation diverged from the plan, add a one-line note under the
step. If a technical decision changes, update the decisions table. Add a line
to the progress log at the end of `PLAN.md` at the end of each session.

**Ask when the plan is ambiguous, not when it is merely hard.** Design choices
that the plan already settles should be followed. Choices the plan leaves
open with a clear conventional answer should be made and stated. Only stop
and ask when different answers would lead to materially different code.

## Build, test, run

- `cargo build` / `cargo run -p viewer` for the viewer. Release builds
  (`--release`) for anything performance related.
- `cargo test` for the engine's unit tests (scenegraph, formats, maths).
- `cargo clippy --all-targets` and `cargo fmt` must be clean before a step is
  declared done.
- CUDA code is behind the `cuda` cargo feature: `cargo run -p viewer
  --features cuda`. The default build must compile and run on a machine
  without CUDA (the macOS laptop). Never make `cuda` a default feature.
- Validation layers are enabled in debug builds. A validation message is a
  bug to fix, not a warning to ignore. If a validation error is MoltenVK
  specific, say so and record it in `docs/MACOS.md`.
- Shaders live in `shaders/` (GLSL) and `kernels/` (CUDA) and are compiled by
  `build.rs`. If the build fails because `glslc` or `nvcc` is missing, the
  fix is the tooling setup in `docs/SETUP.md`, not removing the build step.

## Platforms

- **Apple Silicon Mac via MoltenVK**: where milestones 0 to 4 and the
  Vulkan parts of 6 are developed. Everything except the CUDA feature must
  keep working here.
- **Windows PC with NVIDIA GPU**: needed from milestone 5 onwards and for
  performance numbers. All milestones must work here. Do not break the Mac build to make the PC path easier; use
  feature gates. MoltenVK-specific requirements (portability enumeration,
  portability subset, missing features) are documented in `docs/MACOS.md`.
- Linux: should work, not routinely tested.

When writing platform-specific code, use `#[cfg(target_os = ...)]` at the
smallest scope possible and keep the platform-neutral path readable.

## Code conventions

- Rust 2024 edition, stable toolchain, no nightly features.
- `unsafe` is unavoidable with `ash`. Keep it contained in the `gfx` module,
  wrap it in small safe-ish types, and put a `// SAFETY:` comment on each
  unsafe block stating what invariant makes it sound.
- GPU-visible structs are `#[repr(C)]` and use `glam` types or explicit
  `[f32; N]` arrays; check alignment against the shader's layout (std140 /
  std430 / scalar) in a comment next to the struct.
- Resource handles (meshes, materials, textures, nodes) are typed indices
  into pools, not `Rc`/`Arc` of objects. This is both faster and closer to
  how it would be done in C++.
- Errors: `anyhow` in binaries, `thiserror` enums in library crates.
- Logging: `log` macros for actual log lines everywhere. The binary installs
  a `tracing-subscriber` (with `tracing-log` bridging the `log` facade) as the
  sink, and uses `tracing` spans for per-frame/per-pass timing. Not
  `env_logger`.
- Naming: Vulkan concepts keep their Vulkan names (`Swapchain`, `Pipeline`,
  `DescriptorSet`) so the code reads alongside the spec. Engine concepts use
  plain names (`Scene`, `Node`, `Mesh`, `Material`, `SplatNode`).
- Tests go next to the code (`#[cfg(test)]`) for units; integration tests in
  `crates/*/tests/`. Rendering is not unit tested, but any pure function
  (frustum extraction, covariance projection, format parsing, SH evaluation)
  should be.

## Documentation layout

- `README.md`: one paragraph and a pointer here and to `PLAN.md`.
- `PLAN.md`: milestones, steps, decisions, progress log.
- `AGENTS.md`: this file.
- `docs/SETUP.md`: tooling per platform.
- `docs/MACOS.md`: everything learned about MoltenVK.
- `docs/CUDA.md`: toolchain, interop and profiling notes.
- `docs/PERF.md`: measurements, with machine and date.
- `docs/NOTES/*.md`: longer write-ups of anything worth keeping (e.g. the
  EWA projection derivation, the radix sort design). Written when a step is
  done, not before.

## Things not to do

- Do not commit, push, tag, or rewrite git history unless asked.
- Do not add dependencies beyond those in the `PLAN.md` decisions table
  without saying why in the summary. Prefer writing the small thing over
  adding a crate for it, especially for GPU algorithms we want to learn.
- Do not swap `ash` for `vulkano`/`wgpu`, or CUDA C++ for Rust-GPU, however
  tempting; those are deliberate choices.
- Do not commit large binary assets. `assets/` is gitignored except for its
  README and fetch script.
- Do not put anything personal or private in the repository. This is a public
  repo. No local paths outside the repo, no credentials, no employer-specific
  details.
- Do not silently skip a failing part of a step. Finish what can be finished,
  then say clearly what was left and why.
