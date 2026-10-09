# Match the oldest distribution used for official Linux downloads.
FROM ubuntu:24.04
ARG DEBIAN_FRONTEND=noninteractive
RUN apt-get update && apt-get install -y --no-install-recommends \
    build-essential ca-certificates curl pkg-config clang cmake git python3 \
    libfontconfig1-dev libwayland-dev libxkbcommon-x11-dev libxcb-shape0-dev \
    libxcb-xfixes0-dev libssl-dev libvulkan-dev \
    && rm -rf /var/lib/apt/lists/*
ENV RUSTUP_HOME=/opt/rustup CARGO_HOME=/opt/cargo
RUN curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs \
    | sh -s -- -y --profile minimal --default-toolchain 1.99.0
# Invoke the installed toolchain directly so an unprivileged container user
# never needs to update the root-owned rustup installation.
ENV PATH="/opt/rustup/toolchains/1.99.0-x86_64-unknown-linux-gnu/bin:/opt/cargo/bin:${PATH}"
WORKDIR /workspace
