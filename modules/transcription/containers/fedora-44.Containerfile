# What wholeheard-transcription is built with for Fedora 44 (D27): that system's C++ compiler, CMake
# and clang for whisper.cpp, Vulkan's headers and shader compiler for the graphics card, Rust as
# this repository uses it. Read by scripts/release.sh.
FROM registry.fedoraproject.org/fedora:44
RUN dnf install -y -q gcc gcc-c++ make cmake pkgconf-pkg-config clang clang-devel \
      vulkan-headers vulkan-loader-devel glslc && dnf clean all
ENV RUSTUP_HOME=/opt/rustup CARGO_HOME=/opt/cargo PATH=/opt/cargo/bin:$PATH
RUN curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y --profile minimal --default-toolchain 1.99.0 \
    && chmod -R a+rwX /opt/cargo /opt/rustup
# The processors built for, as .cargo/config.toml says — named here too, so that the image, and the
# build kept for it, change with them.
ENV GGML_NATIVE=OFF GGML_AVX=ON GGML_AVX2=ON GGML_FMA=ON GGML_F16C=ON
