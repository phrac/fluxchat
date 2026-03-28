#!/usr/bin/env bash
set -euo pipefail

echo "Building fluxchat for macOS..."
cargo build -p fluxchat --release

BINARY="target/release/fluxchat"
if [ -f "$BINARY" ]; then
    echo "Build successful: $BINARY"
    ls -lh "$BINARY"
else
    echo "Build failed: binary not found"
    exit 1
fi
