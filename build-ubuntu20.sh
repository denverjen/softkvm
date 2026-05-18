#!/bin/bash
set -e

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"

docker run --rm -v "$SCRIPT_DIR":/src -w /src ubuntu:20.04 bash -c "\
  DEBIAN_FRONTEND=noninteractive apt-get update && \
  DEBIAN_FRONTEND=noninteractive apt-get install -y curl build-essential pkg-config libudev-dev && \
  curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y && \
  . /root/.cargo/env && \
  cargo build --release -p softkvm-client"

echo ""
echo "Binary: $SCRIPT_DIR/target/release/softkvm-client"
