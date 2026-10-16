# Splats: a Rust rendering framework with Gaussian Splatting

This is the development plan. Read `AGENTS.md` first for how we work on this
project; this file is *what* we are building and in what order.

The plan is deliberately broken into small steps, each roughly the size of a
single working session and each ending in something that can be run and
looked at. Steps are ticked off as they are completed (`[x]`), and a short
note is left under a step if the implementation diverged from the plan.

## Goals

1. Build a small but real 3D rendering framework in Rust: a scenegraph with
   efficient culling and drawing, a PBR material system, glTF loading, and a
   Vulkan backend. "Real" means the design choices should be ones a production
   engine could live with, not toy shortcuts.
2. Use that framework as a test bed for Gaussian Splatting: load the common
   splat formats and render them as scenegraph nodes, with the heavy lifting
   done on the GPU.
3. Learn modern CUDA along the way, culminating (stretch goal) in training
   splats from images, which is the first step on a path towards implementing
   neural networks and transformers from scratch.
4. Keep the engine-side code portable enough that individual features
   (culling, splat rasterisation, the splat formats) could later be ported to
   a C++ engine.

## Platforms

Milestones 0 to 4, and the Vulkan-only parts of milestone 6, are developed
on an Apple Silicon Mac through MoltenVK. Milestone 5 onwards needs a Windows
PC with an NVIDIA GPU for CUDA. Both machines must keep building and running
everything except the CUDA-dependent features at all times. Linux should work
but is not tested routinely.

## Key technical decisions

These are decided up front so each step does not re-open them. Each can be
revisited if a step shows it was wrong, but note the change here.

| Area | Choice | Why |
|---|---|---|
| Graphics API | Vulkan 1.3 via the `ash` crate | `ash` is a thin, unsafe, one-to-one binding to the C API. Code written against it reads like C++ Vulkan code and so ports easily, and it teaches the actual API rather than a wrapper's idea of it. `vulkano` and `wgpu` hide too much for our purposes. |
| macOS support | MoltenVK, shipped with the LunarG Vulkan SDK | Apple only supports Metal natively. MoltenVK is a Vulkan implementation layered on Metal. It is a "portability" implementation, so a few extra flags are needed at instance and device creation (see Milestone 1). |
| Windowing | `winit` + `raw-window-handle` + `ash-window` | The standard combination; `ash-window` creates the right surface type per platform including `VK_EXT_metal_surface` on macOS. |
| GPU memory | `gpu-allocator` | Vulkan requires you to sub-allocate from large device allocations; this crate does that and is used by real engines. |
| Shaders | GLSL compiled to SPIR-V at build time with `glslc` (from the SDK) via `build.rs` | Simple, well documented, and GLSL is what most splat reference implementations use. Slang is a candidate to move to later. |
| Maths | `glam` | Fast SIMD-friendly linear algebra, `#[repr(C)]` types that can be uploaded to the GPU directly. |
| glTF loading | `gltf` crate | Mature, handles .gltf and .glb, images and all extensions we need. |
| Images | `image` crate (+ `ktx2`/`basis` later if needed) | Simple to start with. |
| CUDA | CUDA C++ kernels (`.cu` files) compiled with `nvcc` to PTX, loaded from Rust with `cudarc` | Writing kernels in CUDA C++ is the skill being learned and is what work code would use. `cudarc` gives safe-ish Rust bindings to the driver/runtime API and loads PTX at runtime. The Rust-CUDA project (kernels in Rust) is interesting but off-mission. |
| Vulkan/CUDA interop | External memory and external semaphores (`VK_KHR_external_memory_*`, `VK_KHR_external_semaphore_*`, `cudaImportExternalMemory`) | Zero-copy sharing of buffers between the two APIs, which is how a real engine would do it. |
| Debug UI | `egui` (via `egui-winit` and a small custom `ash` renderer, or `egui-ash`) | Added only once we have enough to need tweakable parameters; not in the first milestone. |
| Error handling | `anyhow` in the app, `thiserror` in library crates | Standard. |
| Logging | `log` macros for lines; `tracing-subscriber` + `tracing-log` as the sink, `tracing` spans for timing | Developer preference; spans feed Tracy later via `tracing-tracy`. |
| Crate layout | Cargo workspace: `crates/engine` (library), `crates/viewer` (binary), `crates/cuda` (feature-gated, PC only), `kernels/` (CUDA sources) | Keeps engine code separable from the app, and keeps CUDA optional so the Mac build still works. Start as a single crate if the workspace feels like overhead; split when the second crate is needed. |

Rust edition 2024, stable toolchain. No nightly features.

## Conventions

- Right-handed, Y-up world space (matches glTF). Vulkan's clip space is Y-down
  with depth 0..1; handle this in the projection matrix, not by flipping
  viewports, and note it in a comment the first time it bites.
- Reverse-Z depth buffer (depth 1 = near, 0 = far, `VK_COMPARE_OP_GREATER`)
  from the start, because it is free and retrofitting it is annoying.
- Dynamic rendering (`VK_KHR_dynamic_rendering`, core in 1.3) instead of render
  passes and framebuffers. Synchronisation2 (`VK_KHR_synchronization2`, core in
  1.3) for barriers. Both are supported by MoltenVK.
- Bindless-style descriptor usage (one large descriptor set of textures indexed
  by material id) once the material system exists; a plain descriptor set per
  material in the very first version.
- Validation layers always on in debug builds. A validation error is a bug.

---

## Milestone 0: Project skeleton

Goal: a repo that builds on both machines with the tooling installed.

- [ ] **0.1 Repo and workspace.** `git init`, `.gitignore` (target/, SPIR-V
      outputs, `.vscode/` local settings, any downloaded assets), `Cargo.toml`
      workspace with `crates/engine` and `crates/viewer`, `rust-toolchain.toml`
      pinning stable, `rustfmt.toml`, `clippy` clean at default settings.
      A `README.md` that is one paragraph plus a link to this file.
- [ ] **0.2 Tooling checklist (per machine).** Document in `docs/SETUP.md`:
      Rust via rustup; the LunarG Vulkan SDK (gives `glslc`, `vulkaninfo`,
      validation layers, and on macOS MoltenVK); on the PC, the CUDA Toolkit
      and MSVC Build Tools; environment variables (`VULKAN_SDK`, and on macOS
      the SDK's `setup-env.sh` which sets `VK_ICD_FILENAMES`/`VK_DRIVER_FILES`,
      `VK_LAYER_PATH`, `DYLD_LIBRARY_PATH`). Verify with `vulkaninfo --summary`
      and `nvcc --version`.
- [ ] **0.3 Assets folder.** `assets/` with a `README` describing where the
      sample models and splat files come from (Khronos glTF sample models;
      public 3DGS scenes), and a `.gitignore` so large files are not committed.
      A tiny script (`scripts/fetch-assets.sh` / `.ps1`) that downloads them.

## Milestone 1: A triangle with Vulkan

Goal: the full Vulkan boilerplate written once, understood, and organised so
later milestones do not have to touch it much. This is the longest milestone
in terms of lines of code and the most "learn the modern API" heavy. On macOS
this is also where MoltenVK is set up.

Reference: vulkan-tutorial.com (updated for dynamic rendering), the
`ash` examples, and the Vulkan Guide (vkguide.dev) for the "modern" way.

- [ ] **1.1 Window and event loop.** `winit` window, resize and close events,
      a frame loop that calls an empty `render()` with delta time. `log` macros
      for log lines, `tracing-subscriber` (plus `tracing-log`) as the sink.
- [ ] **1.2 Instance, validation, and surface.** Create a `VkInstance` with
      the validation layer and the debug-utils messenger in debug builds.
      Create the surface with `ash-window`.
      *macOS specifics, which go in `docs/MACOS.md` as they are learned:*
      enable `VK_KHR_portability_enumeration` and set
      `VK_INSTANCE_CREATE_ENUMERATE_PORTABILITY_BIT_KHR`, otherwise MoltenVK's
      device is not even enumerated; use `VK_EXT_metal_surface` (ash-window
      does this); expect `VK_KHR_get_physical_device_properties2` to be
      required as an instance extension.
- [ ] **1.3 Physical and logical device.** Pick a physical device (prefer
      discrete), find a queue family with graphics + present, create the
      device with the 1.3 features we rely on (dynamic rendering,
      synchronization2, buffer device address, descriptor indexing).
      *macOS:* enable the `VK_KHR_portability_subset` device extension when
      it is present (it is mandatory to enable it if offered), and check the
      MoltenVK feature list for anything we rely on that is missing. Check
      the actual Vulkan version MoltenVK reports; if 1.3 is not available, use
      the equivalent `KHR` extensions on 1.2.
- [ ] **1.4 Swapchain.** Surface format and present mode selection, image
      views, recreation on resize. Prefer `MAILBOX`, fall back to `FIFO`
      (MoltenVK typically only offers FIFO).
- [ ] **1.5 Frame synchronisation.** Frames-in-flight (2), per-frame command
      pool and buffer, image-available and render-finished semaphores, in-flight
      fences. Write this as a `FrameContext` struct, since everything later
      hangs off it.
- [ ] **1.6 Shaders and pipeline.** `build.rs` that compiles every file in
      `shaders/` to SPIR-V with `glslc` and embeds the output with
      `include_bytes!`. Triangle vertex and fragment shaders (positions
      hard-coded in the vertex shader, no vertex buffer yet). Graphics
      pipeline with dynamic viewport/scissor and dynamic rendering.
- [ ] **1.7 Draw the triangle.** Record: transition swapchain image, begin
      rendering, bind pipeline, draw 3 verts, end, transition to present,
      submit, present. **Checkpoint: a coloured triangle on both machines.**
- [ ] **1.8 Buffers and memory.** Integrate `gpu-allocator`. A `Buffer`
      type, a staging upload path, and a `Mesh` with vertex + index buffers.
      Replace the hard-coded triangle with an indexed quad. Add a uniform
      buffer with a view-projection matrix and a simple orbit camera.
- [ ] **1.9 Depth buffer and images.** An `Image` type, a depth attachment
      with reverse-Z, and a textured quad (sampler, descriptor set layout,
      descriptor pool). **Checkpoint: textured spinning cube with depth.**
- [ ] **1.10 Tidy.** Pull everything above into `crates/engine/src/gfx/`:
      `instance.rs`, `device.rs`, `swapchain.rs`, `frame.rs`, `buffer.rs`,
      `image.rs`, `pipeline.rs`, `shader.rs`. Write the deletion-queue /
      RAII pattern for resource destruction. `Drop` order in Rust is a real
      concern with Vulkan handles; decide the pattern here and stick to it.

## Milestone 2: Scenegraph with efficient cull and draw

Goal: a scenegraph whose performance characteristics would hold up in a real
engine, with the design explained so it can be ported.

Design direction: a flat, data-oriented scene rather than a tree of boxed
nodes. Nodes live in arrays indexed by handle; the hierarchy is parent indices
plus a topologically-sorted order so world transforms can be updated in a
single linear pass. Renderable data (mesh, material, bounds, world matrix)
lives in separate flat arrays so culling and sorting touch only what they need.

- [ ] **2.1 Node storage and transforms.** `Scene` with `Node { parent, local
      transform, world transform, dirty flag, name }` in a generational arena
      (`slotmap` or hand-rolled). Add/remove/reparent. World-transform update
      as one pass over nodes in parent-before-child order. Unit tests.
- [ ] **2.2 Renderables and bounds.** A `MeshInstance` component referencing a
      mesh and a material handle, with a local AABB. World-space bounding
      sphere/AABB computed in the transform pass. Resource storage for meshes
      and materials as separate handle-indexed pools.
- [ ] **2.3 Camera and frustum culling.** `Camera` node, frustum extraction
      from the view-projection matrix, sphere-then-AABB test against the six
      planes. Produce a visible list per frame. Count and display culled vs
      drawn in the window title (egui comes later).
- [ ] **2.4 Draw list sorting and batching.** Sort visible items by
      (pipeline, material, mesh) to minimise state changes; front-to-back
      within opaque for early-Z. Per-object data (world matrix) goes in a
      per-frame storage buffer indexed by `gl_InstanceIndex` or a push
      constant, so a draw is just `vkCmdDrawIndexed` with an instance offset.
- [ ] **2.5 Indirect drawing.** Move draw parameters into a
      `VkDrawIndexedIndirectCommand` buffer and issue one
      `vkCmdDrawIndexedIndirect` per batch. This is the stepping stone to
      GPU-driven rendering and is where the design stops being "a scenegraph"
      and starts being "a render queue".
- [ ] **2.6 Stress test.** A scene of ~100k instanced cubes/spheres in a grid.
      Profile the CPU side (`tracy` via the `tracing-tracy` crate, or
      `puffin`). Target: transform update + cull + sort well under 1 ms for
      that count on the PC. Record numbers in `docs/PERF.md`.
- [ ] **2.7 (Optional) GPU culling.** Compute shader that does the frustum
      test and writes the indirect buffer, with a count buffer and
      `vkCmdDrawIndexedIndirectCount`. Keep the CPU path as the reference.

## Milestone 3: PBR material system

Goal: a glTF-compatible metallic-roughness PBR pipeline with image-based
lighting, structured so materials are data and shader permutations are
controlled.

Reference: the glTF 2.0 spec Appendix B (BRDF), Khronos glTF-Sample-Viewer
shaders, "Real Shading in Unreal Engine 4" (Karis), learnopengl's PBR
chapters for the IBL precomputation.

- [ ] **3.1 Material data model.** `Material { base colour factor + texture,
      metallic/roughness factors + texture, normal texture, occlusion,
      emissive, alpha mode/cutoff, double-sided }`. GPU-side material buffer
      (one struct per material in a storage buffer), textures in a large
      descriptor array indexed by material (descriptor indexing). A default
      material and a 1x1 white/flat-normal fallback texture.
- [ ] **3.2 Vertex format.** Position, normal, tangent (with handedness),
      UV0, (UV1, colour optional). Decide the packed layout now; it is what
      glTF loading will produce.
- [ ] **3.3 Direct lighting.** Cook-Torrance with GGX NDF, Smith-GGX
      height-correlated visibility, Schlick Fresnel. Directional light plus a
      few point lights in a lights uniform buffer. Normal mapping. Linear
      workflow with sRGB swapchain format or manual encode.
      **Checkpoint: the classic metallic/roughness sphere grid.**
- [ ] **3.4 Image-based lighting.** Load an equirectangular HDR, convert to
      a cubemap, prefilter the specular cubemap by roughness (GGX importance
      sampling), compute the diffuse irradiance map, and the split-sum BRDF
      LUT. All done in compute shaders on first load and cached to disk.
      Skybox pass.
- [ ] **3.5 Tone mapping and post.** Render to an HDR (`R16G16B16A16_SFLOAT`)
      colour target, then a fullscreen tone-map pass (ACES or AgX) into the
      swapchain. This establishes the render-graph-lite structure: a list of
      passes with declared inputs/outputs and the barriers between them.
- [ ] **3.6 Transparency.** Alpha-blend material mode as a separate sorted
      back-to-front pass after opaque. Needed anyway for splats.
- [ ] **3.7 Shadows (optional, but cheap to add here).** A single cascade
      directional shadow map with PCF. Adds a depth-only pipeline variant,
      which forces the "pipeline permutation" question to be answered.

## Milestone 4: glTF scenegraph node

Goal: drop a `.gltf`/`.glb` into the scene and see it rendered correctly
through the systems from milestones 2 and 3.

- [ ] **4.1 Loader.** Using the `gltf` crate: meshes/primitives into our
      vertex format (generate tangents with `mikktspace` when absent, generate
      flat normals when absent), materials into milestone 3's model, textures
      uploaded through a texture cache keyed by image index.
- [ ] **4.2 Hierarchy.** glTF nodes become scenegraph nodes under a root
      `ModelNode`; TRS and matrix transforms; multiple scenes handled by
      picking the default. The model node is just an ordinary subtree once
      loaded, so culling and sorting work unchanged.
- [ ] **4.3 Test against the sample models.** Render `DamagedHelmet`,
      `FlightHelmet`, `Sponza`, `MetalRoughSpheres`, `NormalTangentMirrorTest`,
      `AlphaBlendModeTest` from the Khronos sample set. Compare to the Khronos
      sample viewer and fix discrepancies. Screenshots go in `docs/` for the
      record.
- [ ] **4.4 Scene file and viewer polish.** A tiny scene description (RON or
      TOML) listing models, transforms and the environment map, so the viewer
      is driven by data. Add `egui` now: scene tree, material inspector,
      frame stats. Fly camera.
- [ ] **4.5 (Optional) Animation.** Skinning and node animation from glTF.
      Only if it is wanted; splats do not need it.

## Milestone 5: CUDA

Goal: get CUDA kernels compiling and running from the Rust app, share GPU
memory with Vulkan, and render something with CUDA to prove the whole path
works. This milestone is PC-only; everything in it is behind a `cuda` cargo
feature so the Mac build is untouched.

Reference: the CUDA C++ Programming Guide (the "Programming Model" and
"Performance Guidelines" chapters are what changed least and matter most),
the CUDA samples repo (`simpleVulkan` is exactly the interop we want), and
the `cudarc` examples.

- [ ] **5.1 Toolchain.** `crates/cuda` with a `build.rs` that runs `nvcc
      -ptx` on everything in `kernels/` (via the `cc` crate or a direct
      command), embeds the PTX, and loads it at runtime with `cudarc`. A
      "hello" kernel (vector add) with a test. Decide the target `sm_` /
      `compute_` version based on the PC's GPU and record it.
- [ ] **5.2 Kernel skill-up exercises.** Not engine features; small kernels
      in `kernels/exercises/` with timings, to refresh the fundamentals and
      learn what is new since ~2014: grid-stride loops, shared memory tiling,
      warp shuffles and cooperative groups (which replace a lot of old
      hand-rolled persistent-thread tricks), `__ldg` and the read-only cache,
      unified memory, streams and events, occupancy calculator. A reduction,
      a prefix sum (scan), and a radix sort, because splat rendering needs
      exactly those. Compare against CUB/Thrust equivalents for speed.
- [ ] **5.3 Vulkan/CUDA memory interop.** Allocate a Vulkan buffer with
      `VK_KHR_external_memory` (win32 handle on Windows, fd on Linux), import
      it into CUDA with `cudaImportExternalMemory`, write to it from a kernel,
      read it in a Vulkan shader. Same for a `VkImage` if needed (mipmapped
      arrays). Synchronise with external semaphores so there are no CPU
      round-trips.
- [ ] **5.4 Render something with CUDA.** A `CudaRenderNode` scenegraph node
      that owns a shared image, runs a kernel each frame, and is composited
      into the scene as a textured quad. Suggested kernel: a ray-marched
      volume render of a procedural density field (a nod to prior volume
      rendering work, and the same structure as a splat rasteriser's
      per-pixel loop). Pass the scene camera matrices into the kernel.
      **Checkpoint: CUDA-rendered volume floating in the PBR scene.**
- [ ] **5.5 Profiling.** Nsight Systems for the timeline, Nsight Compute for
      a kernel. Note how to run each in `docs/CUDA.md`.

## Milestone 6: Gaussian splats

Goal: load the common splat file formats and render them as scenegraph nodes,
first with a straightforward Vulkan path (works on the Mac), then with a
high-performance tile-based rasteriser in CUDA.

Reference: Kerbl et al. 2023 "3D Gaussian Splatting for Real-Time Radiance
Field Rendering" and the official `diff-gaussian-rasterization` CUDA code;
Zwicker et al. 2001 "EWA Splatting" for the projected-covariance maths;
the mkkellogg GaussianSplats3D and antimatter15 splat viewers for the
simpler sort-and-blend approach.

- [ ] **6.1 Splat data model.** In-memory representation: position, scale
      (log), rotation (quaternion), opacity (logit), spherical-harmonics
      colour coefficients (degree 0..3). Structure-of-arrays. Decide on the
      GPU-side packed format (e.g. covariance precomputed to 6 floats, SH
      packed as half floats) separately from the file formats.
- [ ] **6.2 Formats.** A `splat_formats` module with a reader per format:
      - `.ply` as written by the original INRIA training code (binary little
        endian, `f_dc_*`, `f_rest_*`, `scale_*`, `rot_*`, `opacity`);
      - `.splat` (antimatter15: 32 bytes per splat, no SH beyond degree 0);
      - `.ksplat` (mkkellogg, compressed, SH optional);
      - `.spz` (Niantic, gzip-compressed, quantised);
      - compressed `.ply` variants (PlayCanvas/SuperSplat "compressed.ply")
        if time allows.
      Each reader converts to the in-memory model. Round-trip tests where a
      format supports writing. These readers are a prime candidate for a C++
      port, so keep them free of clever Rust.
- [ ] **6.3 Simple renderer (Vulkan, cross-platform).** `SplatNode`:
      per-frame CPU or GPU sort of splats by view depth, then one instanced
      draw of a quad per splat. Vertex shader projects the 3D covariance to
      2D (the EWA Jacobian), computes the quad extent (3 sigma), evaluates SH
      for the view direction; fragment shader evaluates the Gaussian and
      alpha-blends back to front. Depth test against the opaque scene, no
      depth write. Start with a CPU sort (radix on quantised depth) because
      it is simple; measure.
      **Checkpoint: a public 3DGS scene (e.g. the "bicycle" or "garden"
      scene) rendered in the viewer on both machines.**
- [ ] **6.4 GPU sort.** Move the depth sort to a compute-shader radix sort
      (Vulkan, so it works on the Mac). Key = quantised view depth, value =
      splat index. This is a well-known but fiddly piece of GPU code and is
      worth doing properly; it will be reused.
- [ ] **6.5 Culling and LOD-ish things.** Frustum cull splats in the same
      compute pass that generates sort keys; skip splats whose projected size
      is below a pixel or whose opacity rounds to zero. Integrate with the
      scenegraph's bounds so a `SplatNode` is culled as a whole when offscreen.
- [ ] **6.6 Tile-based rasteriser (CUDA).** Port the original paper's
      approach: project splats, compute the tile range each touches, generate
      (tile id | depth) keys, radix sort (CUB or 5.2's sort), find per-tile
      ranges, then one thread block per tile that walks its sorted list
      front-to-back with early termination. Output to the shared Vulkan image
      from 5.3. Compare quality and speed against 6.3/6.4.
      **Checkpoint: both splat renderers selectable at runtime, with timings.**
- [ ] **6.7 (Optional) Tile-based rasteriser in Vulkan compute.** Same
      algorithm as 6.6 as a compute shader, so the fast path also works on
      the Mac. Valuable for the C++ port if the work engine is Vulkan/Metal.

## Milestone 7 (stretch): Generating splats

Goal: train a Gaussian splat scene from posed images. This is where the
project turns into a machine learning project: a differentiable renderer,
gradient descent, and an optimiser, all in CUDA.

Reference: the paper and `gaussian-splatting` repo for the algorithm; the
`gsplat` library (Nerfstudio) for a cleaner, well-documented reimplementation
of the backward pass.

- [ ] **7.1 Inputs.** Read COLMAP output (`cameras.bin`, `images.bin`,
      `points3D.bin`) for camera intrinsics/extrinsics and the sparse point
      cloud used to initialise splats. Load the training images.
- [ ] **7.2 Forward pass as a loss.** Render a training view with the 6.6
      rasteriser, compute L1 + D-SSIM loss against the photo on the GPU.
- [ ] **7.3 Backward pass.** Gradients of the loss with respect to each
      splat's position, scale, rotation, opacity and SH coefficients, back
      through alpha compositing, the 2D Gaussian, the projection, and the
      covariance construction. This is the heart of it and is done by hand
      (no autograd). Check with finite differences on a tiny scene.
- [ ] **7.4 Optimiser and training loop.** Adam with per-parameter learning
      rates and the position learning-rate schedule from the paper.
      Adaptive density control: clone/split splats with large view-space
      gradients, prune low-opacity ones, periodic opacity reset. Live view of
      training progress in the viewer.
- [ ] **7.5 Evaluate.** PSNR/SSIM on held-out views for a standard scene
      against published numbers. Export `.ply` and load it back through 6.2.

Beyond this, the natural next steps towards the AI goal are a small tensor
library on top of the same CUDA kernels, then an MLP, then attention, but
those are a different project.

---

## Order of work and where the risk is

Milestones 0 to 4 are sequential and are done on the Mac. Milestone 5 (CUDA)
needs the PC and can start any time after 1.10; it only strictly needs 5.3
done before 6.6. Milestones 6.1 to 6.5 need only milestones 1 to 3 and also
work on the Mac, so a sensible order is 0-4, 6.1-6.5, then move to the PC for
5 and 6.6.

Known risk areas, worth a spike before committing to a design:

- MoltenVK feature coverage: verify dynamic rendering, synchronization2,
  descriptor indexing and buffer device address early (step 1.3).
- `gpu-allocator` plus external memory for CUDA interop: external memory
  needs a dedicated allocation with export flags, so the allocator needs a
  side path (step 5.3).
- Drop/lifetime design for Vulkan handles in Rust (step 1.10). Get it wrong
  and every later step fights the borrow checker.
- The GPU radix sort (6.4) and the backward pass (7.3) are the two pieces
  most likely to eat several sessions.

## Progress log

Add an entry per session: date, steps touched, anything learned that changes
the plan.

- 2026-10-16: Plan written. No code yet.
