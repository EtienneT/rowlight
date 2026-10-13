#!/usr/bin/env bash
# Isolated release cache: never replace the fast host development cache.
set -euo pipefail
project_root=$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")/.." && pwd)
cd "$project_root"
docker build -t tablelamp-linux-builder:ubuntu-24.04 -f packaging/linux/Dockerfile .
mkdir -p target/linux-ubuntu-24.04/cargo target/linux-ubuntu-24.04/build
docker run --rm --user "$(id -u):$(id -g)" \
    -e CARGO_HOME=/workspace/target/linux-ubuntu-24.04/cargo \
    -e CARGO_TARGET_DIR=/workspace/target/linux-ubuntu-24.04/build \
    -e CARGO_BUILD_JOBS=4 \
    -v "$project_root:/workspace" \
    tablelamp-linux-builder:ubuntu-24.04 ./scripts/package-linux.sh "$@"
