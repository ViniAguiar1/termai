use termai_core::Color;

/// A complete terminal color theme.
#[derive(Debug, Clone)]
pub struct Theme {
    pub name: &'static str,
    pub bg: [f32; 4],
    pub fg: [f32; 4],
    pub cursor: [f32; 4],
    pub selection: [f32; 4],
    /// Standard 16 ANSI colors (0-7 normal, 8-15 bright).
    pub ansi: [[f32; 4]; 16],
}

/// Convert a terminal Color to RGBA using the active theme.
pub fn resolve_fg(theme: &Theme, color: Color, bold: bool) -> [f32; 4] {
    match color {
        Color::Default => {
            if bold {
                theme.ansi[15] // bright white for bold default
            } else {
                theme.fg
            }
        }
        Color::Indexed(idx) => {
            let idx = if bold && idx < 8 { idx + 8 } else { idx };
            indexed_to_rgb(theme, idx)
        }
        Color::Rgb(r, g, b) => [r as f32 / 255.0, g as f32 / 255.0, b as f32 / 255.0, 1.0],
    }
}

pub fn resolve_bg(theme: &Theme, color: Color) -> [f32; 4] {
    match color {
        Color::Default => theme.bg,
        Color::Indexed(idx) => indexed_to_rgb(theme, idx),
        Color::Rgb(r, g, b) => [r as f32 / 255.0, g as f32 / 255.0, b as f32 / 255.0, 1.0],
    }
}

fn indexed_to_rgb(theme: &Theme, idx: u8) -> [f32; 4] {
    if (idx as usize) < 16 {
        return theme.ansi[idx as usize];
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

// ---------------------------------------------------------------------------
// UI color helpers — derived from the active theme
// ---------------------------------------------------------------------------

impl Theme {
    /// Tab bar background (inactive tabs).
    pub fn tab_bg(&self) -> [f32; 4] {
        blend(self.bg, [1.0, 1.0, 1.0, 1.0], 0.06)
    }

    /// Tab bar background (active tab).
    pub fn tab_active_bg(&self) -> [f32; 4] {
        blend(self.bg, [1.0, 1.0, 1.0, 1.0], 0.15)
    }

    /// Tab bar foreground (inactive tabs).
    pub fn tab_fg(&self) -> [f32; 4] {
        blend(self.fg, self.bg, 0.4)
    }

    /// Tab bar foreground (active tab).
    pub fn tab_active_fg(&self) -> [f32; 4] {
        self.fg
    }

    /// Tab separator color.
    pub fn tab_separator(&self) -> [f32; 4] {
        blend(self.bg, self.fg, 0.2)
    }

    /// Pane divider color.
    pub fn divider(&self) -> [f32; 4] {
        blend(self.bg, self.fg, 0.25)
    }

    /// Search bar background.
    pub fn search_bg(&self) -> [f32; 4] {
        blend(self.bg, [1.0, 1.0, 1.0, 1.0], 0.10)
    }

    /// Search bar foreground.
    pub fn search_fg(&self) -> [f32; 4] {
        self.fg
    }

    /// AI overlay background.
    pub fn ai_overlay_bg(&self) -> [f32; 4] {
        blend(self.bg, [0.0, 0.2, 0.5, 1.0], 0.15)
    }

    /// Cursor color for underline/bar styles.
    pub fn cursor_bar(&self) -> [f32; 4] {
        blend(self.bg, self.fg, 0.35)
    }
}

/// Linear blend between two colors: result = a * (1 - t) + b * t.
fn blend(a: [f32; 4], b: [f32; 4], t: f32) -> [f32; 4] {
    [
        a[0] * (1.0 - t) + b[0] * t,
        a[1] * (1.0 - t) + b[1] * t,
        a[2] * (1.0 - t) + b[2] * t,
        1.0,
    ]
}

// ---------------------------------------------------------------------------
// Helper to build [f32; 4] from hex at compile time
// ---------------------------------------------------------------------------

const fn hex(r: u8, g: u8, b: u8) -> [f32; 4] {
    [r as f32 / 255.0, g as f32 / 255.0, b as f32 / 255.0, 1.0]
}

// ---------------------------------------------------------------------------
// Built-in themes
// ---------------------------------------------------------------------------

pub const DEFAULT: Theme = Theme {
    name: "termAI Dark",
    bg: hex(0x0a, 0x0a, 0x0a),
    fg: hex(0xe6, 0xe6, 0xe6),
    cursor: hex(0xc4, 0x4d, 0xff),
    selection: [0xc4 as f32 / 255.0, 0x4d as f32 / 255.0, 0xff as f32 / 255.0, 0.25],
    ansi: [
        hex(0x0a, 0x0a, 0x0a), // 0: black
        hex(0xff, 0x5c, 0x57), // 1: red
        hex(0x5a, 0xf7, 0x8e), // 2: green
        hex(0xf3, 0xf9, 0x9d), // 3: yellow
        hex(0x57, 0xc7, 0xff), // 4: blue
        hex(0xc4, 0x4d, 0xff), // 5: magenta (matches accent)
        hex(0x9a, 0xed, 0xfe), // 6: cyan
        hex(0xe6, 0xe6, 0xe6), // 7: white
        hex(0x33, 0x33, 0x33), // 8: bright black
        hex(0xff, 0x7c, 0x77), // 9: bright red
        hex(0x7a, 0xff, 0xae), // 10: bright green
        hex(0xff, 0xff, 0xbd), // 11: bright yellow
        hex(0x77, 0xe7, 0xff), // 12: bright blue
        hex(0xe4, 0x6d, 0xff), // 13: bright magenta
        hex(0xba, 0xff, 0xff), // 14: bright cyan
        hex(0xff, 0xff, 0xff), // 15: bright white
    ],
};

pub const DRACULA: Theme = Theme {
    name: "dracula",
    bg: hex(0x28, 0x2A, 0x36),
    fg: hex(0xF8, 0xF8, 0xF2),
    cursor: hex(0xF8, 0xF8, 0xF2),
    selection: hex(0x44, 0x47, 0x5A),
    ansi: [
        hex(0x21, 0x22, 0x2C), // black
        hex(0xFF, 0x55, 0x55), // red
        hex(0x50, 0xFA, 0x7B), // green
        hex(0xF1, 0xFA, 0x8C), // yellow
        hex(0xBD, 0x93, 0xF9), // blue
        hex(0xFF, 0x79, 0xC6), // magenta
        hex(0x8B, 0xE9, 0xFD), // cyan
        hex(0xF8, 0xF8, 0xF2), // white
        hex(0x62, 0x72, 0xA4), // bright black
        hex(0xFF, 0x6E, 0x6E), // bright red
        hex(0x69, 0xFF, 0x94), // bright green
        hex(0xFF, 0xFF, 0xA5), // bright yellow
        hex(0xD6, 0xAC, 0xFF), // bright blue
        hex(0xFF, 0x92, 0xDF), // bright magenta
        hex(0xA4, 0xFF, 0xFF), // bright cyan
        hex(0xFF, 0xFF, 0xFF), // bright white
    ],
};

pub const CATPPUCCIN_MOCHA: Theme = Theme {
    name: "catppuccin-mocha",
    bg: hex(0x1E, 0x1E, 0x2E),
    fg: hex(0xCD, 0xD6, 0xF4),
    cursor: hex(0xF5, 0xE0, 0xDC),
    selection: hex(0x45, 0x47, 0x5A),
    ansi: [
        hex(0x45, 0x47, 0x5A), // black (surface1)
        hex(0xF3, 0x8B, 0xA8), // red
        hex(0xA6, 0xE3, 0xA1), // green
        hex(0xF9, 0xE2, 0xAF), // yellow
        hex(0x89, 0xB4, 0xFA), // blue
        hex(0xF5, 0xC2, 0xE7), // magenta (pink)
        hex(0x94, 0xE2, 0xD5), // cyan (teal)
        hex(0xBA, 0xC2, 0xDE), // white (subtext1)
        hex(0x58, 0x5B, 0x70), // bright black (surface2)
        hex(0xF3, 0x8B, 0xA8), // bright red
        hex(0xA6, 0xE3, 0xA1), // bright green
        hex(0xF9, 0xE2, 0xAF), // bright yellow
        hex(0x89, 0xB4, 0xFA), // bright blue
        hex(0xF5, 0xC2, 0xE7), // bright magenta
        hex(0x94, 0xE2, 0xD5), // bright cyan
        hex(0xA6, 0xAD, 0xC8), // bright white (subtext0)
    ],
};

pub const CATPPUCCIN_LATTE: Theme = Theme {
    name: "catppuccin-latte",
    bg: hex(0xEF, 0xF1, 0xF5),
    fg: hex(0x4C, 0x4F, 0x69),
    cursor: hex(0xDC, 0x8A, 0x78),
    selection: hex(0xAC, 0xB0, 0xBE),
    ansi: [
        hex(0x5C, 0x5F, 0x77), // black (subtext1)
        hex(0xD2, 0x0F, 0x39), // red
        hex(0x40, 0xA0, 0x2B), // green
        hex(0xDF, 0x8E, 0x1D), // yellow
        hex(0x1E, 0x66, 0xF5), // blue
        hex(0xEA, 0x76, 0xCB), // magenta (pink)
        hex(0x17, 0x92, 0x99), // cyan (teal)
        hex(0xAC, 0xB0, 0xBE), // white (surface2)
        hex(0x6C, 0x6F, 0x85), // bright black (subtext0)
        hex(0xD2, 0x0F, 0x39), // bright red
        hex(0x40, 0xA0, 0x2B), // bright green
        hex(0xDF, 0x8E, 0x1D), // bright yellow
        hex(0x1E, 0x66, 0xF5), // bright blue
        hex(0xEA, 0x76, 0xCB), // bright magenta
        hex(0x17, 0x92, 0x99), // bright cyan
        hex(0x4C, 0x4F, 0x69), // bright white (text)
    ],
};

pub const CATPPUCCIN_FRAPPE: Theme = Theme {
    name: "catppuccin-frappe",
    bg: hex(0x30, 0x34, 0x46),
    fg: hex(0xC6, 0xD0, 0xF5),
    cursor: hex(0xF2, 0xD5, 0xCF),
    selection: hex(0x51, 0x57, 0x6D),
    ansi: [
        hex(0x51, 0x57, 0x6D), // black
        hex(0xE7, 0x82, 0x84), // red
        hex(0xA6, 0xD1, 0x89), // green
        hex(0xE5, 0xC8, 0x90), // yellow
        hex(0x8C, 0xAA, 0xEE), // blue
        hex(0xF4, 0xB8, 0xE4), // magenta
        hex(0x81, 0xC8, 0xBE), // cyan
        hex(0xB5, 0xBF, 0xE2), // white
        hex(0x62, 0x68, 0x80), // bright black
        hex(0xE7, 0x82, 0x84), // bright red
        hex(0xA6, 0xD1, 0x89), // bright green
        hex(0xE5, 0xC8, 0x90), // bright yellow
        hex(0x8C, 0xAA, 0xEE), // bright blue
        hex(0xF4, 0xB8, 0xE4), // bright magenta
        hex(0x81, 0xC8, 0xBE), // bright cyan
        hex(0xA5, 0xAD, 0xCE), // bright white
    ],
};

pub const CATPPUCCIN_MACCHIATO: Theme = Theme {
    name: "catppuccin-macchiato",
    bg: hex(0x24, 0x27, 0x3A),
    fg: hex(0xCA, 0xD3, 0xF5),
    cursor: hex(0xF4, 0xDB, 0xD6),
    selection: hex(0x49, 0x4D, 0x64),
    ansi: [
        hex(0x49, 0x4D, 0x64), // black
        hex(0xED, 0x87, 0x96), // red
        hex(0xA6, 0xDA, 0x95), // green
        hex(0xEE, 0xD4, 0x9F), // yellow
        hex(0x8A, 0xAD, 0xF4), // blue
        hex(0xF5, 0xBD, 0xE6), // magenta
        hex(0x8B, 0xD5, 0xCA), // cyan
        hex(0xB8, 0xC0, 0xE0), // white
        hex(0x5B, 0x60, 0x78), // bright black
        hex(0xED, 0x87, 0x96), // bright red
        hex(0xA6, 0xDA, 0x95), // bright green
        hex(0xEE, 0xD4, 0x9F), // bright yellow
        hex(0x8A, 0xAD, 0xF4), // bright blue
        hex(0xF5, 0xBD, 0xE6), // bright magenta
        hex(0x8B, 0xD5, 0xCA), // bright cyan
        hex(0xA5, 0xAD, 0xCB), // bright white
    ],
};

pub const GRUVBOX_DARK: Theme = Theme {
    name: "gruvbox-dark",
    bg: hex(0x28, 0x28, 0x28),
    fg: hex(0xEB, 0xDB, 0xB2),
    cursor: hex(0xEB, 0xDB, 0xB2),
    selection: hex(0x50, 0x49, 0x45),
    ansi: [
        hex(0x28, 0x28, 0x28), // black
        hex(0xCC, 0x24, 0x1D), // red
        hex(0x98, 0x97, 0x1A), // green
        hex(0xD7, 0x99, 0x21), // yellow
        hex(0x45, 0x85, 0x88), // blue
        hex(0xB1, 0x62, 0x86), // magenta
        hex(0x68, 0x9D, 0x6A), // cyan
        hex(0xA8, 0x99, 0x84), // white
        hex(0x92, 0x83, 0x74), // bright black
        hex(0xFB, 0x49, 0x34), // bright red
        hex(0xB8, 0xBB, 0x26), // bright green
        hex(0xFA, 0xBD, 0x2F), // bright yellow
        hex(0x83, 0xA5, 0x98), // bright blue
        hex(0xD3, 0x86, 0x9B), // bright magenta
        hex(0x8E, 0xC0, 0x7C), // bright cyan
        hex(0xEB, 0xDB, 0xB2), // bright white
    ],
};

pub const GRUVBOX_LIGHT: Theme = Theme {
    name: "gruvbox-light",
    bg: hex(0xFB, 0xF1, 0xC7),
    fg: hex(0x3C, 0x38, 0x36),
    cursor: hex(0x3C, 0x38, 0x36),
    selection: hex(0xD5, 0xC4, 0xA1),
    ansi: [
        hex(0xFB, 0xF1, 0xC7), // black
        hex(0xCC, 0x24, 0x1D), // red
        hex(0x98, 0x97, 0x1A), // green
        hex(0xD7, 0x99, 0x21), // yellow
        hex(0x45, 0x85, 0x88), // blue
        hex(0xB1, 0x62, 0x86), // magenta
        hex(0x68, 0x9D, 0x6A), // cyan
        hex(0x7C, 0x6F, 0x64), // white
        hex(0x92, 0x83, 0x74), // bright black
        hex(0x9D, 0x00, 0x06), // bright red
        hex(0x79, 0x74, 0x0E), // bright green
        hex(0xB5, 0x76, 0x14), // bright yellow
        hex(0x07, 0x66, 0x78), // bright blue
        hex(0x8F, 0x3F, 0x71), // bright magenta
        hex(0x42, 0x7B, 0x58), // bright cyan
        hex(0x3C, 0x38, 0x36), // bright white
    ],
};

pub const NORD: Theme = Theme {
    name: "nord",
    bg: hex(0x2E, 0x34, 0x40),
    fg: hex(0xD8, 0xDE, 0xE9),
    cursor: hex(0xD8, 0xDE, 0xE9),
    selection: hex(0x43, 0x4C, 0x5E),
    ansi: [
        hex(0x3B, 0x42, 0x52), // black
        hex(0xBF, 0x61, 0x6A), // red
        hex(0xA3, 0xBE, 0x8C), // green
        hex(0xEB, 0xCB, 0x8B), // yellow
        hex(0x81, 0xA1, 0xC1), // blue
        hex(0xB4, 0x8E, 0xAD), // magenta
        hex(0x88, 0xC0, 0xD0), // cyan
        hex(0xE5, 0xE9, 0xF0), // white
        hex(0x4C, 0x56, 0x6A), // bright black
        hex(0xBF, 0x61, 0x6A), // bright red
        hex(0xA3, 0xBE, 0x8C), // bright green
        hex(0xEB, 0xCB, 0x8B), // bright yellow
        hex(0x81, 0xA1, 0xC1), // bright blue
        hex(0xB4, 0x8E, 0xAD), // bright magenta
        hex(0x8F, 0xBC, 0xBB), // bright cyan
        hex(0xEC, 0xEF, 0xF4), // bright white
    ],
};

pub const TOKYO_NIGHT: Theme = Theme {
    name: "tokyo-night",
    bg: hex(0x1A, 0x1B, 0x26),
    fg: hex(0xC0, 0xCA, 0xF5),
    cursor: hex(0xC0, 0xCA, 0xF5),
    selection: hex(0x33, 0x46, 0x7C),
    ansi: [
        hex(0x15, 0x16, 0x1E), // black
        hex(0xF7, 0x76, 0x8E), // red
        hex(0x9E, 0xCE, 0x6A), // green
        hex(0xE0, 0xAF, 0x68), // yellow
        hex(0x7A, 0xA2, 0xF7), // blue
        hex(0xBB, 0x9A, 0xF7), // magenta
        hex(0x7D, 0xCF, 0xFF), // cyan
        hex(0xA9, 0xB1, 0xD6), // white
        hex(0x41, 0x44, 0x68), // bright black
        hex(0xF7, 0x76, 0x8E), // bright red
        hex(0x9E, 0xCE, 0x6A), // bright green
        hex(0xE0, 0xAF, 0x68), // bright yellow
        hex(0x7A, 0xA2, 0xF7), // bright blue
        hex(0xBB, 0x9A, 0xF7), // bright magenta
        hex(0x7D, 0xCF, 0xFF), // bright cyan
        hex(0xC0, 0xCA, 0xF5), // bright white
    ],
};

pub const SOLARIZED_DARK: Theme = Theme {
    name: "solarized-dark",
    bg: hex(0x00, 0x2B, 0x36),
    fg: hex(0x83, 0x94, 0x96),
    cursor: hex(0x83, 0x94, 0x96),
    selection: hex(0x07, 0x36, 0x42),
    ansi: [
        hex(0x07, 0x36, 0x42), // black
        hex(0xDC, 0x32, 0x2F), // red
        hex(0x85, 0x99, 0x00), // green
        hex(0xB5, 0x89, 0x00), // yellow
        hex(0x26, 0x8B, 0xD2), // blue
        hex(0xD3, 0x36, 0x82), // magenta
        hex(0x2A, 0xA1, 0x98), // cyan
        hex(0xEE, 0xE8, 0xD5), // white
        hex(0x00, 0x2B, 0x36), // bright black
        hex(0xCB, 0x4B, 0x16), // bright red (orange)
        hex(0x58, 0x6E, 0x75), // bright green (base01)
        hex(0x65, 0x7B, 0x83), // bright yellow (base00)
        hex(0x83, 0x94, 0x96), // bright blue (base1)
        hex(0x6C, 0x71, 0xC4), // bright magenta (violet)
        hex(0x93, 0xA1, 0xA1), // bright cyan (base2)
        hex(0xFD, 0xF6, 0xE3), // bright white
    ],
};

pub const SOLARIZED_LIGHT: Theme = Theme {
    name: "solarized-light",
    bg: hex(0xFD, 0xF6, 0xE3),
    fg: hex(0x65, 0x7B, 0x83),
    cursor: hex(0x65, 0x7B, 0x83),
    selection: hex(0xEE, 0xE8, 0xD5),
    ansi: [
        hex(0xEE, 0xE8, 0xD5), // black
        hex(0xDC, 0x32, 0x2F), // red
        hex(0x85, 0x99, 0x00), // green
        hex(0xB5, 0x89, 0x00), // yellow
        hex(0x26, 0x8B, 0xD2), // blue
        hex(0xD3, 0x36, 0x82), // magenta
        hex(0x2A, 0xA1, 0x98), // cyan
        hex(0x07, 0x36, 0x42), // white
        hex(0xFD, 0xF6, 0xE3), // bright black
        hex(0xCB, 0x4B, 0x16), // bright red (orange)
        hex(0x93, 0xA1, 0xA1), // bright green
        hex(0x83, 0x94, 0x96), // bright yellow
        hex(0x65, 0x7B, 0x83), // bright blue
        hex(0x6C, 0x71, 0xC4), // bright magenta (violet)
        hex(0x58, 0x6E, 0x75), // bright cyan
        hex(0x00, 0x2B, 0x36), // bright white
    ],
};

pub const ONE_DARK: Theme = Theme {
    name: "one-dark",
    bg: hex(0x28, 0x2C, 0x34),
    fg: hex(0xAB, 0xB2, 0xBF),
    cursor: hex(0x52, 0x8B, 0xFF),
    selection: hex(0x3E, 0x44, 0x51),
    ansi: [
        hex(0x28, 0x2C, 0x34), // black
        hex(0xE0, 0x6C, 0x75), // red
        hex(0x98, 0xC3, 0x79), // green
        hex(0xE5, 0xC0, 0x7B), // yellow
        hex(0x61, 0xAF, 0xEF), // blue
        hex(0xC6, 0x78, 0xDD), // magenta
        hex(0x56, 0xB6, 0xC2), // cyan
        hex(0xAB, 0xB2, 0xBF), // white
        hex(0x54, 0x58, 0x62), // bright black
        hex(0xE0, 0x6C, 0x75), // bright red
        hex(0x98, 0xC3, 0x79), // bright green
        hex(0xE5, 0xC0, 0x7B), // bright yellow
        hex(0x61, 0xAF, 0xEF), // bright blue
        hex(0xC6, 0x78, 0xDD), // bright magenta
        hex(0x56, 0xB6, 0xC2), // bright cyan
        hex(0xBE, 0xC5, 0xD4), // bright white
    ],
};

/// All available built-in themes.
pub const THEMES: &[&Theme] = &[
    &DEFAULT,
    &DRACULA,
    &CATPPUCCIN_MOCHA,
    &CATPPUCCIN_LATTE,
    &CATPPUCCIN_FRAPPE,
    &CATPPUCCIN_MACCHIATO,
    &GRUVBOX_DARK,
    &GRUVBOX_LIGHT,
    &NORD,
    &TOKYO_NIGHT,
    &SOLARIZED_DARK,
    &SOLARIZED_LIGHT,
    &ONE_DARK,
];

/// Look up a built-in theme by name (case-insensitive).
pub fn theme_by_name(name: &str) -> Option<&'static Theme> {
    THEMES.iter().find(|t| t.name.eq_ignore_ascii_case(name)).copied()
}
