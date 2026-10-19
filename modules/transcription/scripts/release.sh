#!/usr/bin/env bash
# What this module ships: summit-transcription, into $NSTACK_DIST. Built for the system SUMMIT_TARGET
# names — in a container of that system (containers/<target>.Containerfile), so that it links the
# libraries that system has, never this machine's (D27); unset, for this machine.
set -euo pipefail
here=$(cd "$(dirname "$0")/.." && pwd)
dist=${NSTACK_DIST:?run through nstack release, which names this module\'s artefacts folder}
target=${SUMMIT_TARGET:-}
# whisper.cpp tunes itself to the processor that builds it, unless told otherwise: a program
# that would stop on a processor without this one's instructions (AVX-512). Built for the
# processors of the last ten years instead — AVX2, FMA, F16C (x86-64-v3) —, here as in the
# container.
export GGML_NATIVE=OFF GGML_AVX=ON GGML_AVX2=ON GGML_FMA=ON GGML_F16C=ON
if [ -z "$target" ]; then
  (cd "$here" && cargo build --locked --release)
  cp "${CARGO_TARGET_DIR:-$here/target}/release/summit-transcription" "$dist/"
  exit 0
fi
recipe=$here/containers/$target.Containerfile
[ -f "$recipe" ] || { echo "release: summit-transcription is not built for '$target' (containers/*.Containerfile)" >&2; exit 2; }
image=summit-build-transcription:$target
podman build -q -t "$image" -f "$recipe" "$here/containers" >/dev/null
# Kept between releases, outside the repository: the build — one per image, so that another
# recipe (headers, compilers) builds again from nothing — and the crates fetched.
cache=${XDG_CACHE_HOME:-$HOME/.cache}/summit-build/$target
built=$cache/transcription-$(podman image inspect -f '{{.Id}}' "$image" | cut -c1-12)
mkdir -p "$built" "$cache/registry"
# No relabelling of what is mounted (`:Z` would relabel the files in place): labels left alone.
podman run --rm --security-opt label=disable -v "$here:/src:ro" -v "$built:/target" \
  -v "$cache/registry:/opt/cargo/registry" -e CARGO_TARGET_DIR=/target -e GGML_NATIVE -e GGML_AVX -e GGML_AVX2 -e GGML_FMA -e GGML_F16C -w /src "$image" \
  cargo build --locked --release -q
cp "$built/release/summit-transcription" "$dist/"
