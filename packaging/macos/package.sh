#!/bin/bash
set -e

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
ROOT_DIR="$(cd "$SCRIPT_DIR/../.." && pwd)"
APP_NAME="termAI"
BUNDLE_NAME="$APP_NAME.app"
DMG_NAME="termAI-0.1.0-beta-macos-arm64.dmg"
BUILD_DIR="$ROOT_DIR/target/release"
DIST_DIR="$ROOT_DIR/dist"
APP_DIR="$DIST_DIR/$BUNDLE_NAME"

echo "=== Building termAI for macOS ==="
echo ""

# 1. Build Go AI engine
echo "[1/4] Building Go AI engine..."
cd "$ROOT_DIR/ai"
CGO_ENABLED=0 go build -ldflags "-s -w" -o "$BUILD_DIR/termai-ai" .

# 2. Build Rust terminal (release, Apple Silicon)
echo "[2/4] Building Rust terminal..."
cd "$ROOT_DIR"
cargo build --release

# 3. Create .app bundle
echo "[3/4] Creating $BUNDLE_NAME..."
rm -rf "$APP_DIR"
mkdir -p "$APP_DIR/Contents/MacOS"
mkdir -p "$APP_DIR/Contents/Resources"

# Copy Info.plist
cp "$SCRIPT_DIR/Info.plist" "$APP_DIR/Contents/"

# Copy binaries
cp "$BUILD_DIR/termai" "$APP_DIR/Contents/MacOS/"
cp "$BUILD_DIR/termai-ai" "$APP_DIR/Contents/MacOS/"

# Create launcher script that sets up PATH so termai finds termai-ai
cat > "$APP_DIR/Contents/MacOS/termai-launcher" << 'LAUNCHER'
#!/bin/bash
DIR="$(cd "$(dirname "$0")" && pwd)"
export PATH="$DIR:$PATH"

# Pass through user's shell environment for API keys
if [ -f "$HOME/.zshrc" ]; then
    source "$HOME/.zshrc" 2>/dev/null || true
elif [ -f "$HOME/.bashrc" ]; then
    source "$HOME/.bashrc" 2>/dev/null || true
fi

exec "$DIR/termai"
LAUNCHER
chmod +x "$APP_DIR/Contents/MacOS/termai-launcher"

# Update Info.plist to use launcher
/usr/libexec/PlistBuddy -c "Set :CFBundleExecutable termai-launcher" "$APP_DIR/Contents/Info.plist"

# Copy icon if it exists
if [ -f "$SCRIPT_DIR/AppIcon.icns" ]; then
    cp "$SCRIPT_DIR/AppIcon.icns" "$APP_DIR/Contents/Resources/"
fi

echo "  Created: $APP_DIR"

# 4. Create DMG
echo "[4/4] Creating $DMG_NAME..."
mkdir -p "$DIST_DIR"
rm -f "$DIST_DIR/$DMG_NAME"

# Create temporary DMG directory
DMG_TMP="$DIST_DIR/dmg-tmp"
rm -rf "$DMG_TMP"
mkdir -p "$DMG_TMP"
cp -R "$APP_DIR" "$DMG_TMP/"

# Create symlink to Applications
ln -s /Applications "$DMG_TMP/Applications"

# Create DMG
hdiutil create -volname "$APP_NAME" \
    -srcfolder "$DMG_TMP" \
    -ov -format UDZO \
    "$DIST_DIR/$DMG_NAME"

# Cleanup
rm -rf "$DMG_TMP"

echo ""
echo "=== Done! ==="
echo ""
echo "  App:  $APP_DIR"
echo "  DMG:  $DIST_DIR/$DMG_NAME"
echo ""
echo "To install:"
echo "  1. Open the .dmg"
echo "  2. Drag termAI to Applications"
echo "  3. Right-click > Open (first time, to bypass Gatekeeper)"
echo ""
echo "For AI features, set your API key:"
echo "  export ANTHROPIC_API_KEY=\"sk-ant-...\""
echo "  export OPENAI_API_KEY=\"sk-...\"      # alternative"
