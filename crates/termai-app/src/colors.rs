use termai_core::Color;

/// Dark terminal color palette (similar to Alacritty defaults).
pub const BG: [f32; 4] = [0.07, 0.07, 0.09, 1.0];
pub const FG: [f32; 4] = [0.80, 0.80, 0.82, 1.0];

/// Standard ANSI 16 colors (dark theme).
const ANSI_COLORS: [[f32; 4]; 16] = [
    // Normal colors (0-7)
    [0.10, 0.10, 0.12, 1.0], // 0: black
    [0.80, 0.22, 0.22, 1.0], // 1: red
    [0.36, 0.72, 0.36, 1.0], // 2: green
    [0.80, 0.68, 0.28, 1.0], // 3: yellow
    [0.36, 0.50, 0.82, 1.0], // 4: blue
    [0.68, 0.40, 0.72, 1.0], // 5: magenta
    [0.34, 0.70, 0.72, 1.0], // 6: cyan
    [0.72, 0.72, 0.74, 1.0], // 7: white
    // Bright colors (8-15)
    [0.40, 0.40, 0.42, 1.0], // 8: bright black
    [0.92, 0.36, 0.36, 1.0], // 9: bright red
    [0.48, 0.86, 0.48, 1.0], // 10: bright green
    [0.94, 0.82, 0.40, 1.0], // 11: bright yellow
    [0.50, 0.64, 0.94, 1.0], // 12: bright blue
    [0.82, 0.54, 0.86, 1.0], // 13: bright magenta
    [0.48, 0.84, 0.86, 1.0], // 14: bright cyan
    [0.92, 0.92, 0.94, 1.0], // 15: bright white
];

/// Convert a terminal Color to RGBA f32 values.
pub fn resolve_fg(color: Color, bold: bool) -> [f32; 4] {
    match color {
        Color::Default => {
            if bold {
                ANSI_COLORS[15] // bright white for bold default
            } else {
                FG
            }
        }
        Color::Indexed(idx) => {
            let idx = if bold && idx < 8 { idx + 8 } else { idx };
            indexed_to_rgb(idx)
        }
        Color::Rgb(r, g, b) => [r as f32 / 255.0, g as f32 / 255.0, b as f32 / 255.0, 1.0],
    }
}

pub fn resolve_bg(color: Color) -> [f32; 4] {
    match color {
        Color::Default => BG,
        Color::Indexed(idx) => indexed_to_rgb(idx),
        Color::Rgb(r, g, b) => [r as f32 / 255.0, g as f32 / 255.0, b as f32 / 255.0, 1.0],
    }
}

fn indexed_to_rgb(idx: u8) -> [f32; 4] {
    if (idx as usize) < 16 {
        return ANSI_COLORS[idx as usize];
    }

    // 256-color: 16-231 = 6x6x6 color cube
    if idx < 232 {
        let idx = idx - 16;
        let r = (idx / 36) % 6;
        let g = (idx / 6) % 6;
        let b = idx % 6;
        let to_f = |v: u8| {
            if v == 0 {
                0.0
            } else {
                (55.0 + 40.0 * v as f32) / 255.0
            }
        };
        return [to_f(r), to_f(g), to_f(b), 1.0];
    }

    // 232-255 = grayscale ramp
    let level = (8 + 10 * (idx - 232) as u32) as f32 / 255.0;
    [level, level, level, 1.0]
}
