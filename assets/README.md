# Assets

Sample models and splat scenes used for testing. Nothing in this folder is
committed except this README (see `.gitignore`). Download with:

```sh
scripts/fetch-assets.sh [gltf] [gltf-all] [splats] [splats-large] [all]
```

With no arguments it fetches `gltf`. Re-running skips what is already there,
and fetching one group never removes another.

| Group | Contents | Download | On disk | Needed from |
|---|---|---|---|---|
| `gltf` | The 6 Khronos sample models step 4.3 uses, plain glTF variant | 120 MB | 230 MB | Milestone 4 |
| `gltf-all` | All 150 Khronos sample models, plain glTF variant | 630 MB | ~1.2 GB | Optional |
| `splats` | `bonsai` 3DGS scene: INRIA `.ply` (30k iterations) and antimatter15 `.splat` | 350 MB | 350 MB | Milestone 6 |
| `splats-large` | `bicycle` 3DGS scene, INRIA `.ply` (30k iterations) | 1.5 GB | 1.5 GB | Milestone 6 checkpoint |
| `all` | `gltf-all`, `splats` and `splats-large` | 2.5 GB | ~3.1 GB | |

The glTF models take about twice their download size on disk because they
are a git checkout: `.git` keeps a compressed copy of every file, and
textures and `.bin` buffers barely compress. Deleting
`assets/gltf-sample-assets/.git` reclaims that space, at the cost of the
script no longer being able to add models to that folder.

## Layout

```
assets/
  gltf-sample-assets/Models/<Name>/glTF/<Name>.gltf   (sparse git checkout)
  splats/bonsai.ply
  splats/bonsai-7k.splat
  splats/bicycle.ply
```

## Sources and licences

**glTF models**: [KhronosGroup/glTF-Sample-Assets](https://github.com/KhronosGroup/glTF-Sample-Assets),
the maintained successor to glTF-Sample-Models. The `gltf` group is
`DamagedHelmet`, `FlightHelmet`, `Sponza`, `MetalRoughSpheres`,
`NormalTangentMirrorTest` and `AlphaBlendModeTest` (step 4.3 of `PLAN.md`).
`Models/model-index.json` lists every model with a description.
Licences vary per model (mostly CC-BY 4.0 or CC0); each model folder has
its own `LICENSE.md`.

**Splat scenes**: pretrained scenes from the original 3D Gaussian Splatting
paper (Kerbl et al. 2023), trained on the Mip-NeRF 360 dataset. The
official download is INRIA's
[pretrained models archive](https://repo-sam.inria.fr/fungraph/3d-gaussian-splatting/),
a single 14.6 GB zip of every scene. The script uses a per-scene mirror on
Hugging Face ([dylanebert/3dgs](https://huggingface.co/datasets/dylanebert/3dgs))
instead. That mirror is third party; the files are the INRIA training
output (`point_cloud.ply`), and `.splat` conversions of them. The INRIA
models are released for research and evaluation only, not commercial use.

Formats the readers in step 6.2 also need (`.ksplat`, `.spz`, compressed
`.ply`) will be added to the script when that step is reached.
