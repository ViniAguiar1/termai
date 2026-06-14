//! Design tokens for the termAI visual language.
//!
//! All colors are normalized RGBA in [0.0, 1.0].
//! Spacing unit is 4 pixels.

#![allow(dead_code)]

pub const WINDOW_BG: [f32; 4] = rgb(0x0a, 0x0a, 0x0a);
// Chrome BG matches WINDOW_BG so the strip blends with the content (no
// visible gray bar). The active tab still differentiates via CHROME_BG_ACTIVE
// and the magenta accent line.
pub const CHROME_BG: [f32; 4] = rgb(0x0a, 0x0a, 0x0a);
pub const CHROME_BG_ACTIVE: [f32; 4] = rgb(0x1c, 0x1c, 0x1c);
pub const CHROME_BORDER: [f32; 4] = rgb(0x1c, 0x1c, 0x1c);
pub const TEXT_PRIMARY: [f32; 4] = rgb(0xe6, 0xe6, 0xe6);
pub const TEXT_MUTED: [f32; 4] = rgb(0x8a, 0x8a, 0x8a);
pub const TEXT_DIM: [f32; 4] = rgb(0x5a, 0x5a, 0x5a);
pub const ACCENT: [f32; 4] = rgb(0xc4, 0x4d, 0xff);
/// Amber, for warning states (e.g. the AI engine is up but the LLM is failing).
pub const WARN: [f32; 4] = rgb(0xf0, 0xa8, 0x30);

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

// macOS: vertical reserve at the top of content when there's no tab strip,
// so the traffic-lights row doesn't overlap terminal output.
#[cfg(target_os = "macos")]
pub const TITLE_BAR_RESERVE: f32 = 28.0;
#[cfg(not(target_os = "macos"))]
pub const TITLE_BAR_RESERVE: f32 = 0.0;

// Connection indicator (right side of strip)
pub const CONNECTION_INDICATOR_SIZE: f32 = 8.0;
pub const CONNECTION_INDICATOR_RIGHT_PAD: f32 = 8.0;

// Splits / panes (logical pixels; multiply by scale)
/// Gap reserved between sibling panes for the draggable divider.
pub const PANE_GUTTER: f32 = 6.0;
/// Extra hit-test slop around the divider so the thin gutter is easy to grab.
pub const DIVIDER_GRAB_SLOP: f32 = 4.0;
/// Resting divider color (sits in the gutter between panes).
pub const DIVIDER: [f32; 4] = rgb(0x1c, 0x1c, 0x1c);
/// Divider color when hovered or being dragged.
pub const DIVIDER_HOVER: [f32; 4] = ACCENT;
/// Translucent black laid over inactive panes to focus the active one.
pub const INACTIVE_PANE_DIM: [f32; 4] = [0.0, 0.0, 0.0, 0.38];

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
