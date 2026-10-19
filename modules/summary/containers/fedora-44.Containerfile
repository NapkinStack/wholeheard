# What summit-summary is built with for Fedora 44 (D27): that system's C compiler (for the TLS
# library's assembly), Rust as this repository uses it. Read by scripts/release.sh.
FROM registry.fedoraproject.org/fedora:44
RUN dnf install -y -q gcc && dnf clean all
ENV RUSTUP_HOME=/opt/rustup CARGO_HOME=/opt/cargo PATH=/opt/cargo/bin:$PATH
RUN curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y --profile minimal --default-toolchain 1.99.0 \
    && chmod -R a+rwX /opt/cargo /opt/rustup
