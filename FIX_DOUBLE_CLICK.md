# Bug Fix: Double-click word selection selects wrong row

## Problem

When double-clicking on a word (e.g., "hello" in the output of `echo hello world`), the selection highlights a **different row** — always one row below where the user clicked. The click detection calculates the wrong row index.

## Debug findings

Logs from `RUST_LOG=info` show:

```
DBLCLICK row=2 col=7 cell_h=28.0 y_phys=66.8 mouse_y=33 rect_y=0 line="viniciusaguiar@192 termai %"
  row_above=1: "hello world"
```

The user clicks on "hello world" (visible grid row 1), but `pixel_to_cell_in_pane` returns row=2. The selection then highlights row 2 (the prompt line below).

### Environment
- macOS, Retina display (scale_factor = 2.0)
- Font: JetBrains Mono embedded, config font.size = 16.0
- Pixel font size = 16 * 2 = 32px
- cell_height from atlas = 28.0 physical pixels (was 28, now ~34 after 1.2x line spacing was added but issue persists)

### Coordinate math
- `mouse_y` = 33 logical pixels (from winit `CursorMoved`)
- `y_physical` = 33 * scale_factor(2.0) = 66
- `row` = floor(66 / cell_h) = floor(66 / 34) = 1 (with 1.2x spacing) — **but it's still returning 2**

The 1.2x line spacing fix was applied to `atlas.rs` but the bug persists, which suggests the rendering and click detection may use **different coordinate systems** or there's an offset not accounted for.

## Root cause hypothesis

There's a mismatch between where glyphs are visually rendered on screen (via the GPU shader + vertex positions) and where the click detection logic thinks each row is. Both use `renderer.cell_size()` for the cell height, but something is introducing an offset. Possible causes:

1. **The glyph atlas cell packing adds padding** that shifts where text appears visually but isn't accounted for in click detection
2. **The vertex position of glyphs includes an `offset_y`** from `GlyphInfo` that shifts text downward within the cell — this visual offset isn't in the click math
3. **macOS title bar or window chrome** adding an invisible offset to mouse coordinates
4. **DPI scaling rounding** — `mouse_y` might be fractional but `as f32` truncates

## Files to investigate

### `crates/termai-renderer/src/atlas.rs`
- `GlyphAtlas::new()` — cell_height calculation (line ~52): `((scaled.ascent() - scaled.descent()) * 1.2).ceil()`
- `GlyphInfo.offset_y` — the vertical offset used when rendering each glyph. This offset shifts the glyph within the cell but is NOT used in click detection

### `crates/termai-renderer/src/lib.rs`
- `push_glyph_quad()` (line ~527) — uses `cell_y + glyph.offset_y` to position the glyph. This `offset_y` could cause visible text to appear lower in the cell than the cell boundary
- `build_vertices()` (line ~395) — background quads use `offset_y + row_idx * cell_h`, glyph quads add `glyph.offset_y` on top
- `cell_size()` (line ~376) — returns `(atlas.cell_width, atlas.cell_height)`, used by both rendering and click detection

### `crates/termai-app/src/main.rs`
- `pixel_to_cell_in_pane()` (line ~169) — converts mouse logical pixels to cell (col, row):
  ```rust
  let y = py as f32 * self.scale_factor - rect.y;
  let row = (y / ch).floor().max(0.0) as usize;
  ```
- `MouseInput` handler (line ~828) — double-click detection and word selection at lines ~893-920
- `build_pane_cells()` (line ~193) — selection highlighting uses `row_idx` from visible grid iteration
- `find_pane_at()` (line ~182) — finds which pane the mouse is over

### `crates/termai-renderer/src/shader.wgsl`
- Vertex shader converts pixel coords to clip space:
  ```wgsl
  let x = (in.position.x / uniforms.screen_size.x) * 2.0 - 1.0;
  let y = 1.0 - (in.position.y / uniforms.screen_size.y) * 2.0;
  ```
- `uniforms.screen_size` = physical window size from `inner_size()`

## Suggested investigation approach

1. **Add a visual debug overlay**: render a 1px colored line at each row boundary (`y = row * cell_h` for each row) to see if the row boundaries align with the text. This will immediately show if the rendering and click math agree.

2. **Check `glyph.offset_y`**: Log or print the `offset_y` values from GlyphInfo. If offset_y is significant (e.g., 5-10px), text is rendered lower in the cell than the cell's top boundary, making users click "on the text" but in the cell above.

3. **Test with scale_factor = 1.0**: Run on an external non-Retina display or force scale_factor to 1.0 to see if the issue is DPI-specific.

4. **Compare with single-click drag selection**: Does single-click drag selection also have the same 1-row offset? If yes, the issue is in `pixel_to_cell_in_pane()`. If no, it's specific to the double-click handler.

## Quick test to verify the fix

After fixing, run:
```bash
./dev.sh
```
Then in the terminal:
1. `echo hello world` + Enter
2. Double-click on "hello" in the output line
3. Expected: only "hello" is highlighted (inverted colors)
4. Triple-click on the same line
5. Expected: entire "hello world" line is highlighted

## Additional context

- The double-click **detection** works correctly (click_count reaches 2)
- The `find_word_bounds()` function works correctly (returns proper word boundaries)
- The issue is purely that `row` from `pixel_to_cell_in_pane()` doesn't match the visual row the user clicked on
- winit 0.30, wgpu 24, macOS with Retina display
- The selection rendering in `build_pane_cells` uses `row_idx` from `visible_grid()` iteration — same source as the click handler
