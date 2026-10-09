# Runtime validation image: no Rust toolchain or development libraries.
FROM ubuntu:24.04
ARG DEBIAN_FRONTEND=noninteractive
RUN apt-get update && apt-get install -y --no-install-recommends \
    ca-certificates python3 libfontconfig1 fonts-dejavu-core libwayland-client0 \
    libwayland-cursor0 libxkbcommon-x11-0 libxcb-shape0 libxcb-xfixes0 \
    libvulkan1 mesa-vulkan-drivers xvfb xdotool imagemagick xdg-utils \
    shared-mime-info desktop-file-utils \
    && rm -rf /var/lib/apt/lists/*
