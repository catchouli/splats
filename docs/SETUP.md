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

   The installer offers a "System Global Installation" option, which copies
   the loader and MoltenVK into `/usr/local`. This project does not rely on
   it: use the environment script below so that the SDK version in use is
   explicit.

2. **Environment.** Add this to your shell profile (`~/.zshrc`):

   ```sh
   source ~/VulkanSDK/<version>/setup-env.sh
   ```

   The script sets `VULKAN_SDK` (pointing at the `macOS` subfolder), adds the
   SDK's `bin` to `PATH`, and sets the variables the Vulkan loader uses to
   find things at runtime:
   - `DYLD_LIBRARY_PATH`, so `libvulkan.dylib` can be found. `ash` loads the
     loader dynamically at startup, so without this the viewer fails with
     a "library not loaded" error even though it compiled fine.
   - `VK_DRIVER_FILES` / `VK_ICD_FILENAMES` (newer and older name), pointing
     at the MoltenVK ICD JSON. This is how the loader finds the "driver".
   - `VK_LAYER_PATH` / `VK_ADD_LAYER_PATH`, so the validation layers are found.

   Anything started outside that shell (an IDE launched from the Dock, an
   editor's built-in terminal that does not read `~/.zshrc`) will not have
   these variables. If `cargo run` works in Terminal but not elsewhere, this
   is why.

3. **Check:**

   ```sh
   echo $VULKAN_SDK          # .../VulkanSDK/<version>/macOS
   glslc --version
   vulkaninfo --summary      # should list one device, "Apple M<n>", driver MoltenVK
   ```

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
