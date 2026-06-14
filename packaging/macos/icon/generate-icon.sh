#!/bin/bash
# Generate AppIcon.icns (and a window-icon PNG) from AppIcon.svg.
# Requires: rsvg-convert (brew install librsvg) + iconutil (macOS).
set -e

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
SVG="$SCRIPT_DIR/AppIcon.svg"
ICONSET="$SCRIPT_DIR/AppIcon.iconset"
OUT_ICNS="$SCRIPT_DIR/../AppIcon.icns"          # packaging/macos/AppIcon.icns
OUT_PNG="$SCRIPT_DIR/../../../crates/termai-app/assets/icon.png"  # 256px for runtime window icon

command -v rsvg-convert >/dev/null || { echo "rsvg-convert not found (brew install librsvg)"; exit 1; }
command -v iconutil   >/dev/null || { echo "iconutil not found (macOS only)"; exit 1; }

rm -rf "$ICONSET"
mkdir -p "$ICONSET"

render() { rsvg-convert -w "$1" -h "$1" "$SVG" -o "$ICONSET/$2"; }

render 16   icon_16x16.png
render 32   icon_16x16@2x.png
render 32   icon_32x32.png
render 64   icon_32x32@2x.png
render 128  icon_128x128.png
render 256  icon_128x128@2x.png
render 256  icon_256x256.png
render 512  icon_256x256@2x.png
render 512  icon_512x512.png
render 1024 icon_512x512@2x.png

iconutil -c icns "$ICONSET" -o "$OUT_ICNS"
echo "  Wrote $OUT_ICNS"

# Runtime window icon (macOS Dock uses the .icns from the bundle; this PNG is for
# winit's set_window_icon on Linux/Windows and `cargo run`).
mkdir -p "$(dirname "$OUT_PNG")"
rsvg-convert -w 256 -h 256 "$SVG" -o "$OUT_PNG"
echo "  Wrote $OUT_PNG"

rm -rf "$ICONSET"
echo "Done."
