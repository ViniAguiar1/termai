#!/bin/bash
set -e

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
ROOT_DIR="$(cd "$SCRIPT_DIR/../.." && pwd)"
APP_NAME="termAI"
BUNDLE_NAME="$APP_NAME.app"

# Single source of truth for the version: explicit arg > git tag > Cargo.toml.
# Pass e.g. `package.sh 0.2.0`, otherwise derive from the latest `vX.Y.Z` tag,
# falling back to the workspace package version.
VERSION="${1:-}"
if [ -z "$VERSION" ]; then
    VERSION="$(git -C "$ROOT_DIR" describe --tags --abbrev=0 2>/dev/null | sed 's/^v//')"
fi
if [ -z "$VERSION" ]; then
    VERSION="$(awk -F'"' '/^version/ {print $2; exit}' "$ROOT_DIR/Cargo.toml")"
fi
ARCH="$(uname -m)" # arm64 or x86_64

DMG_NAME="termAI-${VERSION}-macos-${ARCH}.dmg"
BUILD_DIR="$ROOT_DIR/target/release"
DIST_DIR="$ROOT_DIR/dist"
APP_DIR="$DIST_DIR/$BUNDLE_NAME"

echo "Version: $VERSION  Arch: $ARCH"

# Code signing (set these env vars before running)
SIGN_IDENTITY="${APPLE_SIGN_IDENTITY:?Set APPLE_SIGN_IDENTITY (e.g. 'Developer ID Application: Name (TEAMID)')}"
TEAM_ID="${APPLE_TEAM_ID:?Set APPLE_TEAM_ID}"

echo "=== Building termAI for macOS ==="
echo ""

# 1. Build Go AI engine (inject version so `--version` and update-check match)
echo "[1/6] Building Go AI engine..."
cd "$ROOT_DIR/ai"
CGO_ENABLED=0 go build \
    -ldflags "-s -w -X github.com/ViniAguiar1/termai/ai/cmd.appVersion=v${VERSION}" \
    -o "$BUILD_DIR/termai-ai" .

# 2. Build Rust terminal (release, Apple Silicon)
echo "[2/6] Building Rust terminal..."
cd "$ROOT_DIR"
cargo build --release

# 3. Create .app bundle
echo "[3/6] Creating $BUNDLE_NAME..."
rm -rf "$APP_DIR"
mkdir -p "$APP_DIR/Contents/MacOS"
mkdir -p "$APP_DIR/Contents/Resources"

# Copy Info.plist and stamp the version from VERSION
cp "$SCRIPT_DIR/Info.plist" "$APP_DIR/Contents/"
/usr/libexec/PlistBuddy -c "Set :CFBundleVersion $VERSION" "$APP_DIR/Contents/Info.plist"
/usr/libexec/PlistBuddy -c "Set :CFBundleShortVersionString $VERSION" "$APP_DIR/Contents/Info.plist"

# Copy binaries
cp "$BUILD_DIR/termai" "$APP_DIR/Contents/MacOS/"
cp "$BUILD_DIR/termai-ai" "$APP_DIR/Contents/MacOS/"

# Create launcher script that sets up PATH so termai finds termai-ai
cat > "$APP_DIR/Contents/MacOS/termai-launcher" << 'LAUNCHER'
#!/bin/bash
DIR="$(cd "$(dirname "$0")" && pwd)"
export PATH="$DIR:$PATH"

# The AI engine reads its API key from ~/.config/termai/config.toml (preferred)
# or from ANTHROPIC_API_KEY / OPENAI_API_KEY. Source the shell rc as a fallback
# for users who export the key there.
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

# 4. Code sign
echo "[4/6] Signing app..."
codesign --force --options runtime --sign "$SIGN_IDENTITY" "$APP_DIR/Contents/MacOS/termai"
codesign --force --options runtime --sign "$SIGN_IDENTITY" "$APP_DIR/Contents/MacOS/termai-ai"
codesign --force --options runtime --sign "$SIGN_IDENTITY" "$APP_DIR/Contents/MacOS/termai-launcher"
codesign --force --options runtime --sign "$SIGN_IDENTITY" "$APP_DIR"
echo "  Signed with: $SIGN_IDENTITY"

# Verify signature
codesign --verify --deep --strict "$APP_DIR"
echo "  Signature verified OK"

# 5. Create DMG
echo "[5/6] Creating $DMG_NAME..."
mkdir -p "$DIST_DIR"
rm -f "$DIST_DIR/$DMG_NAME"

DMG_TMP="$DIST_DIR/dmg-tmp"
rm -rf "$DMG_TMP"
mkdir -p "$DMG_TMP"
cp -R "$APP_DIR" "$DMG_TMP/"
ln -s /Applications "$DMG_TMP/Applications"

hdiutil create -volname "$APP_NAME" \
    -srcfolder "$DMG_TMP" \
    -ov -format UDZO \
    "$DIST_DIR/$DMG_NAME"

rm -rf "$DMG_TMP"

# Sign the DMG too
codesign --force --sign "$SIGN_IDENTITY" "$DIST_DIR/$DMG_NAME"

# 6. Notarize
echo "[6/6] Notarizing (this may take a few minutes)..."
xcrun notarytool submit "$DIST_DIR/$DMG_NAME" \
    --team-id "$TEAM_ID" \
    --keychain-profile "notarytool" \
    --wait

# Staple the notarization ticket
xcrun stapler staple "$DIST_DIR/$DMG_NAME"
echo "  Notarization complete"

echo ""
echo "=== Done! ==="
echo ""
echo "  App:  $APP_DIR"
echo "  DMG:  $DIST_DIR/$DMG_NAME"
echo "  Status: Signed + Notarized (opens without warnings)"
echo ""
echo "For AI features, set your API key:"
echo "  export ANTHROPIC_API_KEY=\"sk-ant-...\""
echo "  export OPENAI_API_KEY=\"sk-...\"      # alternative"
