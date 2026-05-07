#!/usr/bin/env bash
set -euo pipefail

VERSION="${1:-0.1.0}"
ARCH=$(dpkg --print-architecture 2>/dev/null || echo "amd64")
PKG_NAME="termai"
PKG_DIR="target/deb/${PKG_NAME}_${VERSION}_${ARCH}"

echo "=== Packaging termAI as .deb (v${VERSION}, ${ARCH}) ==="

if [ ! -f "target/release/termai" ]; then
    echo "Error: target/release/termai not found. Run scripts/build-linux.sh first."
    exit 1
fi

# Create directory structure
rm -rf "$PKG_DIR"
mkdir -p "$PKG_DIR/DEBIAN"
mkdir -p "$PKG_DIR/usr/bin"
mkdir -p "$PKG_DIR/usr/share/applications"

# Copy binaries
cp target/release/termai "$PKG_DIR/usr/bin/"
if [ -f "target/release/termai-ai" ]; then
    cp target/release/termai-ai "$PKG_DIR/usr/bin/"
fi

# Control file
cat > "$PKG_DIR/DEBIAN/control" << EOF
Package: ${PKG_NAME}
Version: ${VERSION}
Section: x11
Priority: optional
Architecture: ${ARCH}
Depends: libvulkan1, libfontconfig1, libxkbcommon0
Maintainer: Vinicius Aguiar <vini@termai.dev>
Description: GPU-accelerated terminal emulator with built-in AI assistance
 termAI is a modern terminal emulator featuring GPU-accelerated rendering
 via wgpu/Vulkan, built-in AI error analysis, and command suggestions.
Homepage: https://github.com/ViniAguiar1/termai
EOF

# Desktop entry
cat > "$PKG_DIR/usr/share/applications/termai.desktop" << EOF
[Desktop Entry]
Name=termAI
Comment=GPU-accelerated terminal with AI assistance
Exec=termai
Icon=termai
Terminal=false
Type=Application
Categories=System;TerminalEmulator;
Keywords=terminal;console;command line;AI;
EOF

# Build .deb
dpkg-deb --build "$PKG_DIR"
echo "=== Package created: ${PKG_DIR}.deb ==="
