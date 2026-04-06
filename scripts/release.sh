#!/bin/bash
set -e

VERSION=$(grep '^version' crates/cli/Cargo.toml | head -1 | sed 's/.*"\(.*\)".*/\1/')
echo "Building FlowLink v${VERSION}..."

cargo build --release

mkdir -p dist
ARCH=$(uname -m)
OS=$(uname -s)

tar -czf "dist/flowlink-${VERSION}-${OS}-${ARCH}.tar.gz" \
  -C target/release flowlink

echo "Built: dist/flowlink-${VERSION}-${OS}-${ARCH}.tar.gz"
ls -lh dist/*.tar.gz
