#!/bin/bash
set -e

echo "Building termai..."

# Build Go AI engine
echo "  [1/2] Go AI engine"
cd ai
go build -o ../target/release/termai-ai .
cd ..

# Build Rust terminal
echo "  [2/2] Rust terminal"
cargo build --release

echo ""
echo "Done! Both binaries are in target/release/"
echo "  target/release/termai      (terminal emulator)"
echo "  target/release/termai-ai   (AI engine)"
echo ""
echo "Run with: ./target/release/termai"
