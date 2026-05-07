#!/usr/bin/env bash
set -euo pipefail

echo "=== Building termAI for Linux ==="

# Build Rust terminal emulator
echo "Building Rust binary..."
cargo build --release

# Build Go AI engine
echo "Building Go AI engine..."
cd ai
go build -ldflags "-X github.com/ViniAguiar1/termai/ai/cmd.appVersion=v0.1.0" -o ../target/release/termai-ai
cd ..

echo "=== Build complete ==="
echo "Binaries at:"
echo "  target/release/termai"
echo "  target/release/termai-ai"
