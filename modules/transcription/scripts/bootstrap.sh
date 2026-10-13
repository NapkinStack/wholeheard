#!/usr/bin/env bash
# Makes the module buildable from a fresh clone: the Rust toolchain's components, and what
# the engine's C++ library is built with — a compiler and CMake.
set -euo pipefail

command -v cargo >/dev/null || { echo "bootstrap: install Rust first — https://rustup.rs"; exit 1; }
rustup component add rustfmt clippy >/dev/null 2>&1 || true
command -v c++ >/dev/null || { echo "bootstrap: install a C++ compiler (g++ or clang++)"; exit 1; }
if ! command -v cmake >/dev/null; then
  if command -v apt-get >/dev/null; then sudo apt-get update -qq && sudo apt-get install -y -qq cmake
  elif command -v uv >/dev/null; then uv tool install cmake
  else echo "bootstrap: install CMake"; exit 1
  fi
fi
