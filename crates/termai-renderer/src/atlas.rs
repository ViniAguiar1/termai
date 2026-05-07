use std::collections::HashMap;

use ab_glyph::{point, Font, FontRef, Glyph, ScaleFont};

/// Represents a single glyph's position in the atlas texture.
#[derive(Clone, Copy, Debug)]
pub struct GlyphInfo {
    /// UV coordinates (normalized 0..1) in the atlas texture.
    pub uv_x: f32,
    pub uv_y: f32,
    pub uv_w: f32,
    pub uv_h: f32,
    /// Pixel offset from cell origin when rendering.
    pub offset_x: f32,
    pub offset_y: f32,
    /// Pixel dimensions of the rasterized glyph.
    pub width: f32,
    pub height: f32,
}

/// A texture atlas that rasterizes glyphs on demand, supporting full Unicode.
pub struct GlyphAtlas {
    pub texture_data: Vec<u8>,
    pub texture_width: u32,
    pub texture_height: u32,
    pub cell_width: f32,
    pub cell_height: f32,
    glyphs: HashMap<char, GlyphInfo>,
    /// Packing cursor: next free position in the atlas texture.
    next_x: u32,
    next_y: u32,
    /// Height of the current row of glyphs being packed.
    row_height: u32,
    /// Font data stored for on-demand rasterization.
    font_data: Vec<u8>,
    font_size: f32,
    /// True when texture_data has changed and needs GPU re-upload.
    dirty: bool,
}

impl GlyphAtlas {
    /// Build an atlas from embedded font bytes at the given pixel size.
    /// Pre-populates ASCII 32..127 for fast startup.
    pub fn new(font_bytes: &[u8], font_size: f32) -> Self {
        let font = FontRef::try_from_slice(font_bytes).expect("Failed to parse font");
        let scaled = font.as_scaled(font_size);

        let cell_width = scaled.h_advance(font.glyph_id('M')).ceil();
        // Add line spacing (20%) like modern terminals (iTerm, Alacritty, WezTerm)
        let cell_height = ((scaled.ascent() - scaled.descent()) * 1.2).ceil();

        let cell_w = cell_width.ceil() as u32 + 2; // padding
        let cell_h = cell_height.ceil() as u32 + 2;

        // Start with a 1024x1024 atlas to hold many glyphs
        let tex_width = 1024u32;
        let tex_height = 1024u32;

        let mut texture_data = vec![0u8; (tex_width * tex_height) as usize];
        let mut glyphs = HashMap::new();

        let mut next_x = 0u32;
        let mut next_y = 0u32;
        let row_height = cell_h;

        // Pre-populate ASCII 32..127
        for i in 0..95u32 {
            let ch = (i + 32) as u8 as char;
            let glyph_id = font.glyph_id(ch);
            let glyph = Glyph {
                id: glyph_id,
                scale: font_size.into(),
                position: point(0.0, scaled.ascent()),
            };

            if let Some(outlined) = font.outline_glyph(glyph) {
                let bounds = outlined.px_bounds();
                let gw = bounds.width() as u32;
                let gh = bounds.height() as u32;

                // Check if we need to wrap to next row
                if next_x + cell_w > tex_width {
                    next_x = 0;
                    next_y += row_height;
                }

                let base_x = next_x + 1;
                let base_y = next_y + 1;

                if base_y + gh < tex_height {
                    outlined.draw(|x, y, coverage| {
                        let px = base_x + x;
                        let py = base_y + y;
                        if px < tex_width && py < tex_height {
                            let idx = (py * tex_width + px) as usize;
                            texture_data[idx] = (coverage * 255.0) as u8;
                        }
                    });

                    glyphs.insert(ch, GlyphInfo {
                        uv_x: base_x as f32 / tex_width as f32,
                        uv_y: base_y as f32 / tex_height as f32,
                        uv_w: gw as f32 / tex_width as f32,
                        uv_h: gh as f32 / tex_height as f32,
                        offset_x: bounds.min.x,
                        offset_y: bounds.min.y,
                        width: gw as f32,
                        height: gh as f32,
                    });
                }

                next_x += cell_w;
            }
        }

        Self {
            texture_data,
            texture_width: tex_width,
            texture_height: tex_height,
            cell_width,
            cell_height,
            glyphs,
            next_x,
            next_y,
            row_height,
            font_data: font_bytes.to_vec(),
            font_size,
            dirty: false,
        }
    }

    /// Get glyph info for a character (immutable). Returns None if not yet rasterized.
    pub fn get(&self, ch: char) -> Option<&GlyphInfo> {
        self.glyphs.get(&ch)
    }

    /// Get glyph info, rasterizing on demand if needed.
    pub fn get_or_insert(&mut self, ch: char) -> Option<&GlyphInfo> {
        if self.glyphs.contains_key(&ch) {
            return self.glyphs.get(&ch);
        }

        // Rasterize the glyph
        let font = match FontRef::try_from_slice(&self.font_data) {
            Ok(f) => f,
            Err(_) => return None,
        };
        let scaled = font.as_scaled(self.font_size);

        let glyph_id = font.glyph_id(ch);
        let glyph = Glyph {
            id: glyph_id,
            scale: self.font_size.into(),
            position: point(0.0, scaled.ascent()),
        };

        let outlined = font.outline_glyph(glyph)?;
        let bounds = outlined.px_bounds();
        let gw = bounds.width() as u32;
        let gh = bounds.height() as u32;

        let cell_w = self.cell_width.ceil() as u32 + 2;
        let slot_w = cell_w.max(gw + 2);

        // Check if we need to wrap to next row
        if self.next_x + slot_w > self.texture_width {
            self.next_x = 0;
            self.next_y += self.row_height;
        }

        // Grow atlas if needed
        if self.next_y + self.row_height > self.texture_height {
            let new_height = self.texture_height * 2;
            let mut new_data = vec![0u8; (self.texture_width * new_height) as usize];
            new_data[..self.texture_data.len()].copy_from_slice(&self.texture_data);
            self.texture_data = new_data;
            self.texture_height = new_height;
            // UV coordinates of existing glyphs are now wrong — recalculate them
            let old_height = self.texture_height / 2;
            for info in self.glyphs.values_mut() {
                info.uv_y *= old_height as f32 / new_height as f32;
                info.uv_h *= old_height as f32 / new_height as f32;
            }
            self.dirty = true;
        }

        let base_x = self.next_x + 1;
        let base_y = self.next_y + 1;

        outlined.draw(|x, y, coverage| {
            let px = base_x + x;
            let py = base_y + y;
            if px < self.texture_width && py < self.texture_height {
                let idx = (py * self.texture_width + px) as usize;
                self.texture_data[idx] = (coverage * 255.0) as u8;
            }
        });

        let info = GlyphInfo {
            uv_x: base_x as f32 / self.texture_width as f32,
            uv_y: base_y as f32 / self.texture_height as f32,
            uv_w: gw as f32 / self.texture_width as f32,
            uv_h: gh as f32 / self.texture_height as f32,
            offset_x: bounds.min.x,
            offset_y: bounds.min.y,
            width: gw as f32,
            height: gh as f32,
        };

        self.next_x += slot_w;
        self.dirty = true;
        self.glyphs.insert(ch, info);
        self.glyphs.get(&ch)
    }

    /// Whether the texture data has changed since last GPU upload.
    pub fn is_dirty(&self) -> bool {
        self.dirty
    }

    /// Mark the texture as clean (after GPU re-upload).
    pub fn clear_dirty(&mut self) {
        self.dirty = false;
    }
}
