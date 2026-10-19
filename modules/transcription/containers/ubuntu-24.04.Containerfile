# What summit-transcription is built with for Ubuntu 24.04 (D27): that system's C++ compiler,
# CMake and clang for whisper.cpp, Vulkan's headers and shader compiler for the graphics card,
# Rust as this repository uses it. Read by scripts/release.sh.
FROM docker.io/library/ubuntu:24.04
ENV DEBIAN_FRONTEND=noninteractive
RUN apt-get update -qq && apt-get install -y -qq --no-install-recommends \
      ca-certificates curl build-essential pkg-config cmake clang libclang-dev libvulkan-dev glslc \
    && rm -rf /var/lib/apt/lists/*
ENV RUSTUP_HOME=/opt/rustup CARGO_HOME=/opt/cargo PATH=/opt/cargo/bin:$PATH
RUN curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y --profile minimal --default-toolchain 1.99.0 \
    && chmod -R a+rwX /opt/cargo /opt/rustup
# The processors built for, as .cargo/config.toml says — named here too, so that the image, and the
# build kept for it, change with them.
ENV GGML_NATIVE=OFF GGML_AVX=ON GGML_AVX2=ON GGML_FMA=ON GGML_F16C=ON
