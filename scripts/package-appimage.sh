#!/usr/bin/env bash
set -euo pipefail

VERSION="${1:-0.1.0}"
ARCH=$(uname -m)
APP_DIR="target/appimage/termAI.AppDir"

echo "=== Packaging termAI as AppImage (v${VERSION}, ${ARCH}) ==="

if [ ! -f "target/release/termai" ]; then
    echo "Error: target/release/termai not found. Run scripts/build-linux.sh first."
    exit 1
fi

# Create AppDir structure
rm -rf "$APP_DIR"
mkdir -p "$APP_DIR/usr/bin"

# Copy binaries
cp target/release/termai "$APP_DIR/usr/bin/"
if [ -f "target/release/termai-ai" ]; then
    cp target/release/termai-ai "$APP_DIR/usr/bin/"
fi

# Desktop file (required by AppImage)
cat > "$APP_DIR/termai.desktop" << EOF
[Desktop Entry]
Name=termAI
Comment=GPU-accelerated terminal with AI assistance
Exec=termai
Icon=termai
Terminal=false
Type=Application
Categories=System;TerminalEmulator;
EOF

# AppRun script
cat > "$APP_DIR/AppRun" << 'APPRUN'
#!/usr/bin/env bash
SELF=$(readlink -f "$0")
HERE=${SELF%/*}
export PATH="${HERE}/usr/bin:${PATH}"
export LD_LIBRARY_PATH="${HERE}/usr/lib:${LD_LIBRARY_PATH:-}"
exec "${HERE}/usr/bin/termai" "$@"
APPRUN
chmod +x "$APP_DIR/AppRun"

# Check for appimagetool
if command -v appimagetool &>/dev/null; then
    appimagetool "$APP_DIR" "target/appimage/termAI-${VERSION}-${ARCH}.AppImage"
    echo "=== AppImage created: target/appimage/termAI-${VERSION}-${ARCH}.AppImage ==="
else
    echo "=== AppDir created at ${APP_DIR} ==="
    echo "Install appimagetool to create the final .AppImage file:"
    echo "  wget https://github.com/AppImage/AppImageKit/releases/download/continuous/appimagetool-x86_64.AppImage"
fi
