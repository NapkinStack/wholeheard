#!/usr/bin/env bash
# Makes the module buildable from a fresh clone: the Rust toolchain's components, and what
# the engine's C++ library is built with — a compiler and CMake.
set -euo pipefail

command -v cargo >/dev/null || { echo "bootstrap: install Rust first — https://rustup.rs"; exit 1; }
rustup component add rustfmt clippy >/dev/null 2>&1 || true
command -v c++ >/dev/null || { echo "bootstrap: install a C++ compiler (g++ or clang++)"; exit 1; }
# CMake comes without administrator rights where it can: a bootstrap that asks for a password
# stops whoever runs it without a terminal, and a continuous-integration runner has it already.
if ! command -v cmake >/dev/null; then
  if command -v uv >/dev/null; then uv tool install cmake
  else echo "bootstrap: install CMake — your system's package, or https://cmake.org/download"; exit 1
  fi
  command -v cmake >/dev/null || { echo "bootstrap: CMake is installed but not on the PATH — run: uv tool update-shell, then open a new terminal"; exit 1; }
fi
