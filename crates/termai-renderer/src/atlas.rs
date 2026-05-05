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

/// A texture atlas containing rasterized glyphs for ASCII characters.
pub struct GlyphAtlas {
    pub texture_data: Vec<u8>,
    pub texture_width: u32,
    pub texture_height: u32,
    pub cell_width: f32,
    pub cell_height: f32,
    glyphs: Vec<Option<GlyphInfo>>,
}

impl GlyphAtlas {
    /// Build an atlas from embedded font bytes at the given pixel size.
    pub fn new(font_bytes: &[u8], font_size: f32) -> Self {
        let font = FontRef::try_from_slice(font_bytes).expect("Failed to parse font");
        let scaled = font.as_scaled(font_size);

        let cell_width = scaled.h_advance(font.glyph_id('M')).ceil();
        let cell_height = (scaled.ascent() - scaled.descent()).ceil();

        // We'll pack ASCII 32..127 in a grid
        let glyph_count = 95; // printable ASCII
        let cols = 16u32;
        let rows = ((glyph_count as u32) + cols - 1) / cols;

        let cell_w = cell_width.ceil() as u32 + 2; // padding
        let cell_h = cell_height.ceil() as u32 + 2;

        let tex_width = cols * cell_w;
        let tex_height = rows * cell_h;

        let mut texture_data = vec![0u8; (tex_width * tex_height) as usize];
        let mut glyphs: Vec<Option<GlyphInfo>> = vec![None; 128];

        for i in 0..glyph_count {
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

                let col = (i as u32) % cols;
                let row = (i as u32) / cols;
                let base_x = col * cell_w + 1;
                let base_y = row * cell_h + 1;

                outlined.draw(|x, y, coverage| {
                    let px = base_x + x;
                    let py = base_y + y;
                    if px < tex_width && py < tex_height {
                        let idx = (py * tex_width + px) as usize;
                        texture_data[idx] = (coverage * 255.0) as u8;
                    }
                });

                glyphs[ch as usize] = Some(GlyphInfo {
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
        }

        Self {
            texture_data,
            texture_width: tex_width,
            texture_height: tex_height,
            cell_width,
            cell_height,
            glyphs,
        }
    }

    /// Get glyph info for a character. Returns None for non-printable chars.
    pub fn get(&self, ch: char) -> Option<&GlyphInfo> {
        let idx = ch as usize;
        if idx < self.glyphs.len() {
            self.glyphs[idx].as_ref()
        } else {
            None
        }
    }
}
