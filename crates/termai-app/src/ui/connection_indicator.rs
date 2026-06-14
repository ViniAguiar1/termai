//! Connection indicator — small dot on the right edge of the tab strip.

use crate::theme::tokens;
use termai_renderer::{Renderer, Vertex};

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum State {
    Disconnected,
    Connected,
    Analyzing,
}

/// Render the indicator. `pulse_t` is 0.0..1.0 — only used by `Analyzing`.
/// `scale` is the display scale (physical pixels per logical pixel).
pub fn render(
    state: State,
    strip_width: f32,
    pulse_t: f32,
    scale: f32,
    renderer: &Renderer,
    vertices: &mut Vec<Vertex>,
) {
    let size = tokens::CONNECTION_INDICATOR_SIZE * scale;
    let x = strip_width - size - tokens::CONNECTION_INDICATOR_RIGHT_PAD * scale;
    // Center the dot vertically within the strip.
    let y = (tokens::TAB_STRIP_HEIGHT * scale - size) / 2.0;

    match state {
        State::Connected => {
            renderer.build_rect(x, y, size, size, tokens::TEXT_DIM, vertices);
        }
        State::Disconnected => {
            renderer.build_rect_outline(x, y, size, size, 1.0, tokens::TEXT_DIM, vertices);
        }
        State::Analyzing => {
            let alpha = tokens::CURSOR_FADE_MIN + (1.0 - tokens::CURSOR_FADE_MIN) * pulse_t;
            renderer.build_rect(
                x,
                y,
                size,
                size,
                tokens::with_alpha(tokens::ACCENT, alpha),
                vertices,
            );
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn state_is_copy() {
        // Compile-check: ensures `State` derives `Copy`.
        let s = State::Connected;
        let _t = s;
        let _u = s;
    }
}
