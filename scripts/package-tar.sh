#!/usr/bin/env bash
set -euo pipefail

VERSION="${1:-0.1.0}"
ARCH=$(uname -m)
PKG_NAME="termai-${VERSION}-linux-${ARCH}"
PKG_DIR="target/tar/${PKG_NAME}"

echo "=== Packaging termAI as .tar.gz (v${VERSION}, ${ARCH}) ==="

if [ ! -f "target/release/termai" ]; then
    echo "Error: target/release/termai not found. Run scripts/build-linux.sh first."
    exit 1
fi

rm -rf "$PKG_DIR"
mkdir -p "$PKG_DIR"

# Copy binaries
cp target/release/termai "$PKG_DIR/"
if [ -f "target/release/termai-ai" ]; then
    cp target/release/termai-ai "$PKG_DIR/"
fi

# Install script
cat > "$PKG_DIR/install.sh" << 'INSTALL'
#!/usr/bin/env bash
set -euo pipefail

PREFIX="${1:-/usr/local}"
echo "Installing termAI to ${PREFIX}/bin..."

sudo install -Dm755 termai "${PREFIX}/bin/termai"
if [ -f termai-ai ]; then
    sudo install -Dm755 termai-ai "${PREFIX}/bin/termai-ai"
fi

echo "termAI installed successfully!"
echo "Run 'termai' to start."
INSTALL
chmod +x "$PKG_DIR/install.sh"

# Create tarball
cd target/tar
tar czf "${PKG_NAME}.tar.gz" "${PKG_NAME}"
cd ../..

echo "=== Tarball created: target/tar/${PKG_NAME}.tar.gz ==="
