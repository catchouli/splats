# Tooling setup

What each machine needs to build and run this project, and how to check it
is working. If a build fails because `glslc` or `nvcc` cannot be found, the
fix is here, not in `build.rs`.

| Tool | Mac | Windows PC | Linux / cloud | Used for |
|---|---|---|---|---|
| Rust (rustup) | yes | yes (MSVC toolchain) | yes | everything |
| LunarG Vulkan SDK | yes (includes MoltenVK) | yes | yes, or distro packages | `glslc`, validation layers, `vulkaninfo`, Vulkan loader |
| MSVC Build Tools | no | yes | no | Rust's linker on Windows, and `nvcc`'s host compiler |
| NVIDIA driver | no | yes | yes | Vulkan on the NVIDIA GPU, CUDA |
| CUDA Toolkit 12.8+ | no | yes | yes (CUDA machines only) | `nvcc`, the `cuda` feature |

## Rust (all machines)

Install rustup from <https://rustup.rs>. Nothing else is needed: the repo's
`rust-toolchain.toml` pins the exact compiler version, and rustup downloads
it (with `rustfmt` and `clippy`) the first time `cargo` runs in the repo.

On Windows, accept the default `x86_64-pc-windows-msvc` host. The GNU
toolchain will not work with `nvcc`.

## macOS (Apple Silicon)

1. **Vulkan SDK.** Download the macOS installer from
   <https://vulkan.lunarg.com/sdk/home>. It installs to
   `~/VulkanSDK/<version>/` and includes MoltenVK, the Vulkan loader, the
   validation layers, `glslc` and `vulkaninfo`.

   **Tick "System Global Installation"** in the installer. It copies the
   loader (`libvulkan.dylib`), the MoltenVK ICD, the validation layers and
   the tools (`glslc`, `vulkaninfo`, ...) into `/usr/local`, which is where
   the Vulkan loader and the dynamic linker look by default. No environment
   setup is then needed.

   Why not rely on the SDK's `setup-env.sh` instead: it makes the loader
   findable through `DYLD_LIBRARY_PATH`, and macOS (System Integrity
   Protection) strips `DYLD_*` variables whenever a protected system binary
   such as `/bin/zsh` launches a child process. `ash` loads `libvulkan.dylib`
   at runtime, so depending on how the viewer is started (Terminal, an IDE,
   a script) it would fail with "library not loaded" even though it compiled
   fine. `/usr/local/lib` is on dyld's default search path, so the global
   install works however the viewer is launched.

   Do not also source `setup-env.sh` from your shell profile. It is harmless,
   but the loader then sees every layer twice (once from the SDK folder, once
   from `/usr/local`) and prints "Removing layer ... duplicate" warnings.

   To undo a global install, `sudo ~/VulkanSDK/<version>/uninstall.sh`
   removes the `/usr/local` copies and the apps in `/Applications`, and leaves
   `~/VulkanSDK` alone. When upgrading the SDK, run the new installer with
   the same option ticked; it replaces the `/usr/local` files.

2. **Check** (in a new terminal):

   ```sh
   which glslc               # /usr/local/bin/glslc
   glslc --version
   vulkaninfo --summary      # should list one device, "Apple M<n>", driver MoltenVK
   ```

   Recent SDKs also install a second ICD, KosmicKrisp (Mesa's Vulkan on
   Metal). On macOS 14 it does not show up as a device; if a newer macOS
   lists two devices, the viewer must pick the MoltenVK one (record this in
   `docs/MACOS.md` if it happens).

   `vulkaninfo` reports the device's Vulkan API version. MoltenVK supports
   1.3 with the portability subset; known gaps are tracked in
   `docs/MACOS.md`.

## Windows (NVIDIA RTX PRO 2000 Blackwell laptop)

Install in this order, since CUDA's installer looks for Visual Studio:

1. **Visual Studio Build Tools** (or Visual Studio Community) from
   <https://visualstudio.microsoft.com/downloads/>, with the
   "Desktop development with C++" workload. This provides `link.exe` for
   Rust and `cl.exe`, which `nvcc` uses to compile the host-side code.
2. **NVIDIA driver.** The current Studio or Game Ready driver. It provides the
   Vulkan ICD and the CUDA driver (`nvcuda.dll`).
3. **Vulkan SDK.** The Windows installer from
   <https://vulkan.lunarg.com/sdk/home>. It sets `VULKAN_SDK` and adds the
   SDK's `Bin` folder to `PATH`; no script is needed.
4. **CUDA Toolkit 12.8 or newer** from
   <https://developer.nvidia.com/cuda-downloads>. 12.8 is the first release
   that can target Blackwell consumer and workstation parts (`sm_120`). The
   installer sets `CUDA_PATH` and adds `nvcc` to `PATH`.

**Check** (in a new terminal, so the updated `PATH` is picked up):

```powershell
rustc --version
glslc --version
vulkaninfo --summary   # should list the NVIDIA GPU and the integrated GPU
nvidia-smi             # driver version and the highest CUDA version it supports
nvcc --version         # 12.8 or newer
```

Two GPUs: `vulkaninfo --summary` lists both the NVIDIA GPU and the
integrated GPU. The engine prefers the discrete GPU when picking a device
(step 1.3). Windows may still run the *window* on the integrated GPU and
copy frames across, which is fine for development and matters only for
performance numbers. For those, set the viewer to "High performance" in
Windows Settings > Display > Graphics.

`nvcc` needs `cl.exe`. If `nvcc` complains that it cannot find it, either
run from the "x64 Native Tools Command Prompt for VS" or let `build.rs`
locate it (decided in step 5.1).

## Linux and cloud GPUs

Not routinely tested. Install the Vulkan SDK tarball from LunarG or the
distribution packages (`vulkan-tools`, `vulkan-validationlayers`,
`glslc` or `shaderc`), and on CUDA machines the NVIDIA driver and CUDA
Toolkit 12.8+.

Rented cloud GPUs (Colab and similar) are headless. Only the CUDA crate and
the trainer are expected to run there, and they must not need a window or a
Vulkan surface. Usually only the CUDA Toolkit is needed there, and it is
often already installed; check with `nvcc --version` and `nvidia-smi`.
