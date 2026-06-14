//! Floating search bar (top-right corner of content area).

use crate::theme::tokens;
use termai_renderer::{Renderer, Vertex};

pub const SEARCH_BAR_WIDTH: f32 = 280.0;
pub const SEARCH_BAR_HEIGHT: f32 = 32.0;
pub const SEARCH_BAR_OFFSET_TOP: f32 = 8.0;
pub const SEARCH_BAR_OFFSET_RIGHT: f32 = 8.0;
pub const SEARCH_BAR_PADDING_X: f32 = 12.0;
pub const SEARCH_ICON: &str = "⌕";

pub struct SearchBarInput<'a> {
    pub query: &'a str,
    pub match_count: usize,
    pub current_match: usize,
    pub strip_width: f32,
    pub content_top: f32,
}

pub fn render(
    input: &SearchBarInput,
    renderer: &mut Renderer,
    main_vertices: &mut Vec<Vertex>,
    chrome_vertices: &mut Vec<Vertex>,
) {
    let x = input.strip_width - SEARCH_BAR_WIDTH - SEARCH_BAR_OFFSET_RIGHT;
    let y = input.content_top + SEARCH_BAR_OFFSET_TOP;

    // Drop shadow (single offset rect with low alpha — no real blur).
    renderer.build_rect(
        x + 2.0, y + 4.0, SEARCH_BAR_WIDTH, SEARCH_BAR_HEIGHT,
        [0.0, 0.0, 0.0, 0.4],
        main_vertices,
    );

    // Background.
    renderer.build_rect(x, y, SEARCH_BAR_WIDTH, SEARCH_BAR_HEIGHT, tokens::CHROME_BG_ACTIVE, main_vertices);

    // 1px border.
    renderer.build_rect_outline(x, y, SEARCH_BAR_WIDTH, SEARCH_BAR_HEIGHT, 1.0, tokens::CHROME_BORDER, main_vertices);

    let (cw, ch) = renderer.chrome_cell_size();
    let text_y = y + (SEARCH_BAR_HEIGHT - ch) / 2.0;

    // Icon.
    renderer.build_chrome_text_run(SEARCH_ICON, x + SEARCH_BAR_PADDING_X, text_y, tokens::TEXT_MUTED, chrome_vertices);

    // Query text (or placeholder).
    let query_x = x + SEARCH_BAR_PADDING_X + 2.0 * cw;
    if input.query.is_empty() {
        renderer.build_chrome_text_run("Buscar...", query_x, text_y, tokens::TEXT_DIM, chrome_vertices);
    } else {
        renderer.build_chrome_text_run(input.query, query_x, text_y, tokens::TEXT_PRIMARY, chrome_vertices);
    }

    // Counter on the right.
    if input.match_count > 0 {
        let counter = format!("{}/{}", input.current_match + 1, input.match_count);
        let counter_w = counter.chars().count() as f32 * cw;
        renderer.build_chrome_text_run(
            &counter,
            x + SEARCH_BAR_WIDTH - SEARCH_BAR_PADDING_X - counter_w,
            text_y,
            tokens::TEXT_MUTED,
            chrome_vertices,
        );
    }
}
