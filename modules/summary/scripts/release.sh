#!/usr/bin/env bash
# What this module ships: summit-summary, into $NSTACK_DIST. Built for the system SUMMIT_TARGET
# names — in a container of that system (containers/<target>.Containerfile), so that it links the
# libraries that system has, never this machine's (D27); unset, for this machine.
set -euo pipefail
here=$(cd "$(dirname "$0")/.." && pwd)
dist=${NSTACK_DIST:?run through nstack release, which names this module\'s artefacts folder}
target=${SUMMIT_TARGET:-}
if [ -z "$target" ]; then
  (cd "$here" && cargo build --locked --release)
  cp "${CARGO_TARGET_DIR:-$here/target}/release/summit-summary" "$dist/"
  exit 0
fi
recipe=$here/containers/$target.Containerfile
[ -f "$recipe" ] || { echo "release: summit-summary is not built for '$target' (containers/*.Containerfile)" >&2; exit 2; }
image=summit-build-summary:$target
podman build -q -t "$image" -f "$recipe" "$here/containers" >/dev/null
# Kept between releases, outside the repository: the build — one per image, so that another
# recipe (headers, compilers) builds again from nothing — and the crates fetched.
cache=${XDG_CACHE_HOME:-$HOME/.cache}/summit-build/$target
built=$cache/summary-$(podman image inspect -f '{{.Id}}' "$image" | cut -c1-12)
mkdir -p "$built" "$cache/registry"
# No relabelling of what is mounted (`:Z` would relabel the files in place): labels left alone.
podman run --rm --security-opt label=disable -v "$here:/src:ro" -v "$built:/target" \
  -v "$cache/registry:/opt/cargo/registry" -e CARGO_TARGET_DIR=/target -w /src "$image" \
  cargo build --locked --release -q
cp "$built/release/summit-summary" "$dist/"
