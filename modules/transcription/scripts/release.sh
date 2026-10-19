#!/usr/bin/env bash
# What this module ships: summit-transcription, into $NSTACK_DIST. Built for the system SUMMIT_TARGET
# names — in a container of that system (containers/<target>.Containerfile), so that it links the
# libraries that system has, never this machine's (D27); unset, for this machine.
set -euo pipefail
here=$(cd "$(dirname "$0")/.." && pwd)
dist=${NSTACK_DIST:?run through nstack release, which names this module\'s artefacts folder}
target=${SUMMIT_TARGET:-}
# The processors it is built for: .cargo/config.toml, read by every build of the module.
# On the graphics card through Vulkan where the machine has it — a tenth of the meeting's
# length on the decider's laptop —, on the processor where it has not (ADR-0003).
features=(--features vulkan)
if [ -z "$target" ]; then
  (cd "$here" && cargo build --locked --release "${features[@]}")
  # whisper-rs-sys does not build again when the flags change: a whisper.cpp built before them,
  # tuned to this processor, would be shipped as it is — built again, then.
  out=${CARGO_TARGET_DIR:-$here/target}/release
  if grep -qs '^GGML_NATIVE:BOOL=ON' "$out"/build/whisper-rs-sys-*/out/build/CMakeCache.txt; then
    (cd "$here" && cargo clean --release -p whisper-rs-sys && cargo build --locked --release "${features[@]}")
  fi
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
# No relabelling of what is mounted (`:Z` would relabel the files in place): labels left alone;
# the source seen through a throwaway layer (`:O`): what a build writes there never reaches it.
podman run --rm --security-opt label=disable -v "$here:/src:O" -v "$built:/target" \
  -v "$cache/registry:/opt/cargo/registry" -e CARGO_TARGET_DIR=/target -w /src "$image" \
  cargo build --locked --release -q "${features[@]}"
cp "$built/release/summit-transcription" "$dist/"
