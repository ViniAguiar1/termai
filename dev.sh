#!/usr/bin/env bash
set -euo pipefail

echo "=== Building AI engine ==="
cd ai
go build -o ../target/debug/termai-ai
cd ..

echo "=== Starting termAI ==="
cargo run
