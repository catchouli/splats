#!/usr/bin/env bash
# Download the sample assets into assets/. See assets/README.md for what each
# group contains and where it comes from.
#
#   scripts/fetch-assets.sh                 # gltf (the default)
#   scripts/fetch-assets.sh gltf splats     # several groups
#   scripts/fetch-assets.sh gltf-all        # every Khronos sample model
#   scripts/fetch-assets.sh all             # everything (gltf-all and all splats)
#
# Safe to re-run: anything already downloaded is skipped.

set -euo pipefail

ASSETS="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)/assets"

# Khronos glTF sample models used by step 4.3. Only the plain glTF variant
# (.gltf + .bin + images) of each is checked out.
GLTF_REPO="https://github.com/KhronosGroup/glTF-Sample-Assets.git"
GLTF_MODELS=(
    DamagedHelmet
    FlightHelmet
    Sponza
    MetalRoughSpheres
    NormalTangentMirrorTest
    AlphaBlendModeTest
)

# Pretrained 3DGS scenes (INRIA training code output), mirrored one scene at a
# time on Hugging Face. The official INRIA archive bundles every scene into a
# single 14.6 GB zip.
SPLAT_BASE="https://huggingface.co/datasets/dylanebert/3dgs/resolve/main"
# "remote path in the dataset" "local file under assets/splats/"
SPLATS=(
    "bonsai/point_cloud/iteration_30000/point_cloud.ply" "bonsai.ply"
    "bonsai/bonsai-7k.splat"                              "bonsai-7k.splat"
)
SPLATS_LARGE=(
    "bicycle/point_cloud/iteration_30000/point_cloud.ply" "bicycle.ply"
)

GLTF_DIR="$ASSETS/gltf-sample-assets"

clone_gltf_repo() {
    if [ ! -d "$GLTF_DIR/.git" ]; then
        echo "Cloning glTF-Sample-Assets (sparse, shallow)..."
        # --filter=blob:none and --sparse: fetch the commit and tree but no file
        # contents, so only the folders added below are actually downloaded.
        git clone --depth 1 --filter=blob:none --sparse "$GLTF_REPO" "$GLTF_DIR"
    fi
}

# fetch_gltf_models <name> [<name> ...]
fetch_gltf_models() {
    local paths=()
    for m in "$@"; do
        paths+=("Models/$m/glTF")
    done
    # "add" rather than "set", so fetching one group never removes another.
    # Cone mode also checks out the files directly inside each parent folder,
    # so every model's LICENSE.md and README.md come along.
    git -C "$GLTF_DIR" sparse-checkout add "${paths[@]}"
    echo "glTF models in $GLTF_DIR/Models"
}

fetch_gltf() {
    clone_gltf_repo
    fetch_gltf_models "${GLTF_MODELS[@]}"
}

fetch_gltf_all() {
    clone_gltf_repo
    # The clone already has every directory listing (only file contents were
    # filtered out), so the model list can be read locally.
    local models=()
    while IFS= read -r m; do
        models+=("$m")
    done < <(git -C "$GLTF_DIR" ls-tree -d --name-only HEAD Models/ | sed 's|^Models/||')
    fetch_gltf_models "${models[@]}"
}

# fetch_files <remote> <local> [<remote> <local> ...]
fetch_files() {
    mkdir -p "$ASSETS/splats"
    while [ $# -gt 0 ]; do
        local url="$SPLAT_BASE/$1"
        local out="$ASSETS/splats/$2"
        shift 2
        if [ -f "$out" ]; then
            echo "Have $out"
            continue
        fi
        echo "Downloading $url"
        # Download to .part and rename at the end, so an interrupted download is
        # never mistaken for a finished one; -C - resumes a partial .part.
        curl -fL --retry 3 -C - -o "$out.part" "$url"
        mv "$out.part" "$out"
    done
}

if [ $# -eq 0 ]; then
    set -- gltf
fi

for group in "$@"; do
    case "$group" in
        gltf)         fetch_gltf ;;
        gltf-all)     fetch_gltf_all ;;
        splats)       fetch_files "${SPLATS[@]}" ;;
        splats-large) fetch_files "${SPLATS_LARGE[@]}" ;;
        all)
            fetch_gltf_all
            fetch_files "${SPLATS[@]}"
            fetch_files "${SPLATS_LARGE[@]}"
            ;;
        *)
            echo "Unknown group '$group'. Groups: gltf, gltf-all, splats, splats-large, all" >&2
            exit 1
            ;;
    esac
done
