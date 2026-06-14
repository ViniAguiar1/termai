//! AI suggestion overlay — appears at the bottom of the focused pane.

use crate::theme::tokens;
use termai_renderer::{Renderer, Vertex};

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Risk {
    Low,
    Medium,
    High,
}

impl Risk {
    pub fn from_str(s: &str) -> Self {
        match s {
            "high" => Risk::High,
            "medium" => Risk::Medium,
            _ => Risk::Low,
        }
    }
}

pub struct ActionView<'a> {
    pub label: &'a str,
    pub risk: Risk,
}

pub struct AiOverlayInput<'a> {
    pub title: &'a str,
    pub description: &'a str,
    pub actions: &'a [ActionView<'a>],
    /// (x, y, w, h) of the focused pane (pixel coords).
    pub pane_rect: (f32, f32, f32, f32),
    /// 0.0..1.0 — multiplied into every color's alpha for fade-out.
    pub fade_alpha: f32,
}

pub fn render(
    input: &AiOverlayInput,
    renderer: &mut Renderer,
    main_vertices: &mut Vec<Vertex>,
    chrome_vertices: &mut Vec<Vertex>,
) {
    let (px, py, pw, ph) = input.pane_rect;
    let pad_y = 12.0;
    let pad_x = 16.0;
    let line_h = 18.0;
    let title_line_h = 22.0;
    let action_count = input.actions.len() as f32;
    let total_h = pad_y * 2.0 + title_line_h + line_h + action_count * line_h;

    let oy = py + ph - total_h;
    let ox = px;

    // Background.
    let mut bg = tokens::CHROME_BG;
    bg[3] = input.fade_alpha;
    renderer.build_rect(ox, oy, pw, total_h, bg, main_vertices);

    // Top border (accent, 2px).
    let mut accent = tokens::ACCENT;
    accent[3] = input.fade_alpha;
    renderer.build_rect(ox, oy, pw, 2.0, accent, main_vertices);

    let mut cursor_y = oy + pad_y;
    let mut primary = tokens::TEXT_PRIMARY;
    let mut muted = tokens::TEXT_MUTED;
    primary[3] = input.fade_alpha;
    muted[3] = input.fade_alpha;

    // Title.
    renderer.build_chrome_text_run(input.title, ox + pad_x, cursor_y, primary, chrome_vertices);
    cursor_y += title_line_h;

    // Description (single-line, truncated to fit).
    let (cw, _) = renderer.chrome_cell_size();
    let max_chars = ((pw - 2.0 * pad_x) / cw) as usize;
    let truncated: String = if input.description.chars().count() > max_chars {
        let kept: String = input
            .description
            .chars()
            .take(max_chars.saturating_sub(1))
            .collect();
        format!("{}…", kept)
    } else {
        input.description.to_string()
    };
    renderer.build_chrome_text_run(&truncated, ox + pad_x, cursor_y, muted, chrome_vertices);
    cursor_y += line_h;

    // Actions.
    for (i, action) in input.actions.iter().enumerate() {
        let num = format!("[{}]", i + 1);
        let num_w = renderer.measure_chrome_text(&num);
        renderer.build_chrome_text_run(&num, ox + pad_x, cursor_y, accent, chrome_vertices);
        renderer.build_chrome_text_run(
            action.label,
            ox + pad_x + num_w + cw,
            cursor_y,
            primary,
            chrome_vertices,
        );

        // Risk dot on the right.
        let dot_color = match action.risk {
            Risk::Low => [
                0x5a_u8 as f32 / 255.0,
                0xf7_u8 as f32 / 255.0,
                0x8e_u8 as f32 / 255.0,
                input.fade_alpha,
            ],
            Risk::Medium => [
                0xf3_u8 as f32 / 255.0,
                0xf9_u8 as f32 / 255.0,
                0x9d_u8 as f32 / 255.0,
                input.fade_alpha,
            ],
            Risk::High => [
                0xff_u8 as f32 / 255.0,
                0x5c_u8 as f32 / 255.0,
                0x57_u8 as f32 / 255.0,
                input.fade_alpha,
            ],
        };
        let dot_x = ox + pw - pad_x - 6.0;
        let dot_y = cursor_y + 4.0;
        renderer.build_rect(dot_x, dot_y, 6.0, 6.0, dot_color, main_vertices);

        if matches!(action.risk, Risk::High) {
            let label_w = 4.0 * cw + cw; // "high" + spacing approx
            renderer.build_chrome_text_run("high", dot_x - label_w, cursor_y, muted, chrome_vertices);
        }

        cursor_y += line_h;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn risk_from_str_maps_correctly() {
        assert_eq!(Risk::from_str("low"), Risk::Low);
        assert_eq!(Risk::from_str("medium"), Risk::Medium);
        assert_eq!(Risk::from_str("high"), Risk::High);
        assert_eq!(Risk::from_str("unknown"), Risk::Low); // default
    }
}
