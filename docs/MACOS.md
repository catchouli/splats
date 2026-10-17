# macOS and MoltenVK notes

What we have learned about running the engine on Apple Silicon through
MoltenVK, in the order we hit it. Tested on an M3 Max, macOS 14, Vulkan SDK
1.4.363 (MoltenVK 1.4.2) unless a section says otherwise.

## Loading the Vulkan loader (step 1.2)

`ash::Entry::load()` calls `dlopen("libvulkan.dylib")`. For a bare name like
that, dyld searches `DYLD_LIBRARY_PATH` and then its default fallback path,
which on current macOS is only `/usr/lib`, *not* `/usr/local/lib` (older
documentation says it includes `/usr/local/lib`; the error message's list
of tried paths shows it does not). So even with the SDK's global install,
the plain load fails with:

```
dlopen(libvulkan.dylib, 0x0005): tried: 'libvulkan.dylib' (no such file), ...
'/usr/lib/libvulkan.dylib' (no such file, not in dyld cache) ...
```

Setting `DYLD_LIBRARY_PATH` or `DYLD_FALLBACK_LIBRARY_PATH` is not a fix:
SIP strips `DYLD_*` variables when a protected binary (the shell) starts a
process. The engine instead falls back to explicit paths on macOS
(`load_entry` in `crates/engine/src/gfx/instance.rs`):
`/usr/local/lib/libvulkan.dylib`, then `$VULKAN_SDK/lib/libvulkan.dylib`.
`vulkaninfo` is not affected because it links the loader instead of opening
it at runtime, which is why the setup check in step 0.2 did not catch this.

A packaged `.app` would instead ship `libvulkan.dylib` and `libMoltenVK.dylib`
in `Contents/Frameworks` and load from there. Not needed yet.

## Instance creation (step 1.2)

- **Portability enumeration is required.** MoltenVK is a "portability"
  driver (it does not implement all of Vulkan), and loaders since 1.3.216
  hide such drivers unless the instance enables
  `VK_KHR_portability_enumeration` and sets
  `VK_INSTANCE_CREATE_ENUMERATE_PORTABILITY_BIT_KHR`. Without it,
  `vkEnumeratePhysicalDevices` returns no devices. The engine enables it
  whenever the loader offers it, on every platform; elsewhere it is harmless.
- **`VK_KHR_get_physical_device_properties2` is not needed** as an instance
  extension: it is core since Vulkan 1.1 and we request API 1.3. (Older
  guides list it because they target 1.0.) Confirmed in step 1.3: enabling
  `VK_KHR_portability_subset` on the device raises no validation message
  without it.
- **Surface:** `VK_EXT_metal_surface`, which `ash-window` picks for an AppKit
  window. It attaches a `CAMetalLayer` to the window's `NSView`, so the
  surface must be created on the main thread (winit's callbacks run there).
- **Reported API version is clamped.** `vulkaninfo` shows the M3 Max as
  Vulkan 1.4.357, but with our instance requesting 1.3 MoltenVK reports the
  device as 1.3.357 ("Created VkInstance for Vulkan version 1.3.357, as
  requested by app").
- **KosmicKrisp.** The SDK also installs a second driver, KosmicKrisp. On
  macOS 14 the loader drops it ("Removing driver ... due to not having any
  physical devices"), so MoltenVK is the only device.
- **Duplicate layer warnings.** If the SDK's `setup-env.sh` variables
  (`VK_ADD_LAYER_PATH`, `VK_ICD_FILENAMES`) are set alongside the global
  install, the loader logs a "Removing layer ... duplicate" warning per
  layer. Harmless; unset the variables (see `docs/SETUP.md`).

## Device creation (step 1.3)

- **`VK_KHR_portability_subset` must be enabled** whenever the device lists
  it. Its features are parts of core Vulkan that MoltenVK may not do. The
  engine enables every one the device supports and logs the rest. On the
  M3 Max the unsupported ones are `pointPolygons`, `samplerMipLodBias`,
  `tessellationIsolines` and `tessellationPointMode`. None of these matter
  to us; `samplerMipLodBias` means sampler LOD bias must stay 0.
- **Everything the plan relies on is supported:** dynamic rendering,
  synchronization2, buffer device address, and the descriptor-indexing
  features for bindless textures (runtime arrays, partially bound,
  variable count, non-uniform sampled-image indexing, update-after-bind).
  `timelineSemaphore` and `scalarBlockLayout` are also available.
- **Not supported:** `drawIndirectCount` (so no
  `vkCmdDrawIndexedIndirectCount`; this matters for step 2.7),
  `geometryShader`, `wideLines`.
- **Queues:** 4 queue families, each with 1 queue, all
  graphics + compute + transfer. Family 0 presents.
- The device is an `INTEGRATED_GPU`, so the "prefer discrete" rule has no
  effect here.
- The validation layer logs `WARNING-cache-file-error` at INFO severity at
  device creation the first time it runs (there is no
  `~/.cache/shader_validation_cache.bin` yet). It is not a problem.

## Build warnings

- `cargo build` reports that `block v0.1.6` "contains code that will be
  rejected by a future version of Rust". It comes from `ash-window` 0.13 →
  `raw-window-metal` 0.4 → `cocoa` → `block`, macOS only. Upstream, nothing
  to do on our side until `ash-window` moves to the `objc2` crates.
