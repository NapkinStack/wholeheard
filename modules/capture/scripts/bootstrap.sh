#!/usr/bin/env bash
# Makes the module buildable from a fresh clone: the Rust toolchain, and on Linux the
# headers of the two sound systems the audio library is built against.
set -euo pipefail

command -v cargo >/dev/null || { echo "bootstrap: install Rust first — https://rustup.rs"; exit 1; }
rustup component add rustfmt clippy >/dev/null 2>&1 || true

if [ "$(uname -s)" = "Linux" ]; then
  if pkg-config --exists libpipewire-0.3 alsa 2>/dev/null && command -v clang >/dev/null; then
    exit 0
  fi
  if command -v apt-get >/dev/null; then
    sudo apt-get update -qq
    sudo apt-get install -y -qq libpipewire-0.3-dev libasound2-dev libclang-dev clang pkg-config
  elif command -v dnf >/dev/null; then
    sudo dnf install -y pipewire-devel alsa-lib-devel clang pkgconf-pkg-config
  else
    echo "bootstrap: install the PipeWire and ALSA development headers, and clang"; exit 1
  fi
fi
