#!/usr/bin/env bash
# What this module ships, into $NSTACK_DIST: summit, its desktop entry and icon under the
# application's id, and its version. The program is built for the system SUMMIT_TARGET names —
# in a container of that system (containers/<target>.Containerfile), so that it links the
# libraries that system has, never this machine's (D27); unset, for this machine.
set -euo pipefail
here=$(cd "$(dirname "$0")/.." && pwd)
dist=${NSTACK_DIST:?run through nstack release, which names this module\'s artefacts folder}
target=${SUMMIT_TARGET:-}
if [ -z "$target" ]; then
  (cd "$here" && cargo build --locked --release)
  built=$here/target/release
else
  recipe=$here/containers/$target.Containerfile
  [ -f "$recipe" ] || { echo "release: summit is not built for '$target' (containers/*.Containerfile)" >&2; exit 2; }
  image=summit-build-shell:$target
  podman build -q -t "$image" -f "$recipe" "$here/containers" >/dev/null
  # Kept between releases, outside the repository: the build and the crates fetched.
  cache=${XDG_CACHE_HOME:-$HOME/.cache}/summit-build/$target
  mkdir -p "$cache/shell" "$cache/registry"
  # No relabelling of what is mounted (`:Z` would relabel the files in place): labels left alone.
  podman run --rm --security-opt label=disable -v "$here:/src:ro" -v "$cache/shell:/target" \
    -v "$cache/registry:/opt/cargo/registry" -e CARGO_TARGET_DIR=/target -w /src "$image" \
    cargo build --locked --release -q
  built=$cache/shell/release
fi
cp "$built/summit" "$here/linux/dev.summit.Summit.desktop" "$dist/"
cp "$here/icons/icon.png" "$dist/dev.summit.Summit.png"
python3 -c 'import json, sys; print(json.load(open(sys.argv[1]))["version"])' "$here/tauri.conf.json" > "$dist/VERSION"
