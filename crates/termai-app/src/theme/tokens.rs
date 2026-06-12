//! Design tokens for the termAI visual language.
//!
//! All colors are normalized RGBA in [0.0, 1.0].
//! Spacing unit is 4 pixels.

#![allow(dead_code)]

pub const WINDOW_BG: [f32; 4] = rgb(0x0a, 0x0a, 0x0a);
pub const CHROME_BG: [f32; 4] = rgb(0x1c, 0x1c, 0x1c);
pub const CHROME_BG_ACTIVE: [f32; 4] = rgb(0x26, 0x26, 0x26);
pub const CHROME_BORDER: [f32; 4] = rgb(0x2e, 0x2e, 0x2e);
pub const TEXT_PRIMARY: [f32; 4] = rgb(0xe6, 0xe6, 0xe6);
pub const TEXT_MUTED: [f32; 4] = rgb(0x8a, 0x8a, 0x8a);
pub const TEXT_DIM: [f32; 4] = rgb(0x5a, 0x5a, 0x5a);
pub const ACCENT: [f32; 4] = rgb(0xc4, 0x4d, 0xff);

pub const UNIT: f32 = 4.0;

// Spacing
pub const CONTENT_PADDING_LEFT: f32 = 12.0;
pub const CONTENT_PADDING_TOP: f32 = 8.0;
pub const CONTENT_PADDING_RIGHT: f32 = 8.0;
pub const CONTENT_PADDING_BOTTOM: f32 = 4.0;

// Tab strip
pub const TAB_STRIP_HEIGHT: f32 = 36.0;
pub const TAB_STRIP_BORDER: f32 = 1.0;
pub const TAB_MIN_WIDTH: f32 = 120.0;
pub const TAB_MIN_WIDTH_ABSOLUTE: f32 = 60.0;
pub const TAB_MAX_WIDTH: f32 = 240.0;
pub const TAB_ACTIVE_ACCENT_HEIGHT: f32 = 2.0;

// macOS: pixel reserve for traffic lights on the left of the strip.
#[cfg(target_os = "macos")]
pub const TRAFFIC_LIGHTS_RESERVE: f32 = 78.0;
#[cfg(not(target_os = "macos"))]
pub const TRAFFIC_LIGHTS_RESERVE: f32 = 0.0;

// Connection indicator (right side of strip)
pub const CONNECTION_INDICATOR_SIZE: f32 = 8.0;
pub const CONNECTION_INDICATOR_RIGHT_PAD: f32 = 8.0;

// Typography
pub const FONT_SIZE_CONTENT: f32 = 14.0;
pub const FONT_SIZE_CHROME: f32 = 12.0;

// Alpha
pub const SELECTION_ALPHA: f32 = 0.25;
pub const SEARCH_MATCH_ALPHA: f32 = 0.35;
pub const SEARCH_CURRENT_MATCH_ALPHA: f32 = 0.70;

// Animation
pub const HOVER_TRANSITION_MS: u128 = 120;
pub const OVERLAY_FADE_MS: u128 = 200;
pub const CURSOR_BLINK_MS: u128 = 530;
pub const CURSOR_FADE_MIN: f32 = 0.4;
pub const PULSE_PERIOD_MS: u128 = 1000;

const fn rgb(r: u8, g: u8, b: u8) -> [f32; 4] {
    [r as f32 / 255.0, g as f32 / 255.0, b as f32 / 255.0, 1.0]
}

pub const fn with_alpha(color: [f32; 4], alpha: f32) -> [f32; 4] {
    [color[0], color[1], color[2], alpha]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn accent_color_matches_spec() {
        // #c44dff = (196, 77, 255)
        assert_eq!(ACCENT, [196.0 / 255.0, 77.0 / 255.0, 255.0 / 255.0, 1.0]);
    }

    #[test]
    fn with_alpha_preserves_rgb() {
        let result = with_alpha(ACCENT, 0.25);
        assert_eq!(result[0], ACCENT[0]);
        assert_eq!(result[1], ACCENT[1]);
        assert_eq!(result[2], ACCENT[2]);
        assert_eq!(result[3], 0.25);
    }
}
