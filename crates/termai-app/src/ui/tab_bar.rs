//! Tab bar component — renders the strip at the top of the window.

use crate::theme::tokens;
use termai_renderer::{Renderer, Vertex};

/// A single tab's layout rectangle.
#[derive(Debug, Clone, PartialEq)]
pub struct TabRect {
    pub index: usize,
    pub x: f32,
    pub y: f32,
    pub w: f32,
    pub h: f32,
}

/// Compute the layout for N tabs given the strip's available width.
/// All length parameters and tokens are in PHYSICAL pixels — caller multiplies
/// logical dimensions by `scale` before passing.
pub fn layout_tabs(
    tab_count: usize,
    strip_width: f32,
    strip_height: f32,
    traffic_lights_reserve: f32,
    scale: f32,
) -> Vec<TabRect> {
    if tab_count == 0 {
        return vec![];
    }
    let available = strip_width
        - traffic_lights_reserve
        - tokens::CONNECTION_INDICATOR_SIZE * scale
        - tokens::CONNECTION_INDICATOR_RIGHT_PAD * scale;
    let mut per_tab = available / tab_count as f32;
    per_tab = per_tab.clamp(
        tokens::TAB_MIN_WIDTH_ABSOLUTE * scale,
        tokens::TAB_MAX_WIDTH * scale,
    );

    let mut out = Vec::with_capacity(tab_count);
    let mut x = traffic_lights_reserve;
    for i in 0..tab_count {
        out.push(TabRect {
            index: i,
            x,
            y: 0.0,
            w: per_tab,
            h: strip_height,
        });
        x += per_tab;
    }
    out
}

/// Hit test: return the tab index containing the given mouse position, if any.
pub fn hit_test(tabs: &[TabRect], px: f32, py: f32) -> Option<usize> {
    for tab in tabs {
        if px >= tab.x && px < tab.x + tab.w && py >= tab.y && py < tab.y + tab.h {
            return Some(tab.index);
        }
    }
    None
}

/// Action buttons drawn in the strip to the right of the tabs.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ButtonKind {
    NewTab,
    SplitRight,
    SplitDown,
}

#[derive(Debug, Clone)]
pub struct StripButton {
    pub kind: ButtonKind,
    pub x: f32,
    pub y: f32,
    pub w: f32,
    pub h: f32,
}

/// Lay out the action buttons immediately to the right of the last tab. Each is
/// a square hit area the height of the strip. All values are physical px.
pub fn layout_buttons(tabs: &[TabRect], strip_height: f32, scale: f32) -> Vec<StripButton> {
    let size = strip_height;
    let mut x = tabs.last().map(|t| t.x + t.w).unwrap_or(0.0) + 4.0 * scale;
    let mut out = Vec::with_capacity(3);
    for kind in [
        ButtonKind::NewTab,
        ButtonKind::SplitRight,
        ButtonKind::SplitDown,
    ] {
        out.push(StripButton {
            kind,
            x,
            y: 0.0,
            w: size,
            h: strip_height,
        });
        x += size;
    }
    out
}

/// Hit test the action buttons.
pub fn hit_test_button(buttons: &[StripButton], px: f32, py: f32) -> Option<ButtonKind> {
    buttons
        .iter()
        .find(|b| px >= b.x && px < b.x + b.w && py >= b.y && py < b.y + b.h)
        .map(|b| b.kind)
}

/// Draw the action-button icons (plus / split glyphs) as crisp rects so they
/// match the minimal chrome and don't depend on the font atlas.
pub fn render_buttons(
    buttons: &[StripButton],
    hovered: Option<ButtonKind>,
    scale: f32,
    renderer: &Renderer,
    vertices: &mut Vec<Vertex>,
) {
    for b in buttons {
        let color = if hovered == Some(b.kind) {
            tokens::TEXT_PRIMARY
        } else {
            tokens::TEXT_DIM
        };
        let icon = (b.h * 0.4).round();
        let th = (1.5 * scale).max(1.0);
        let cx = b.x + b.w / 2.0;
        let cy = b.y + b.h / 2.0;
        let half = icon / 2.0;
        match b.kind {
            ButtonKind::NewTab => {
                // Plus sign.
                renderer.build_rect(cx - half, cy - th / 2.0, icon, th, color, vertices);
                renderer.build_rect(cx - th / 2.0, cy - half, th, icon, color, vertices);
            }
            ButtonKind::SplitRight => {
                // Square with a vertical divider (two panes side by side).
                renderer.build_rect_outline(cx - half, cy - half, icon, icon, th, color, vertices);
                renderer.build_rect(cx - th / 2.0, cy - half, th, icon, color, vertices);
            }
            ButtonKind::SplitDown => {
                // Square with a horizontal divider (two panes stacked).
                renderer.build_rect_outline(cx - half, cy - half, icon, icon, th, color, vertices);
                renderer.build_rect(cx - half, cy - th / 2.0, icon, th, color, vertices);
            }
        }
    }
}

pub struct TabBarRenderInput<'a> {
    pub tabs: &'a [TabRect],
    pub active_index: usize,
    pub hovered_index: Option<usize>,
    /// 0.0..1.0 — animation progress for the hover bg interpolation.
    pub hover_progress: f32,
    pub titles: &'a [String],
    pub strip_width: f32,
    /// Display scale (physical pixels per logical pixel).
    pub scale: f32,
}

pub fn render_tab_bar(
    input: &TabBarRenderInput,
    renderer: &mut Renderer,
    main_vertices: &mut Vec<Vertex>,
    chrome_vertices: &mut Vec<Vertex>,
) {
    let s = input.scale;
    let strip_h = tokens::TAB_STRIP_HEIGHT * s;
    let border_h = tokens::TAB_STRIP_BORDER * s;

    // 1. Strip background (just the tab row — traffic lights overlay it).
    renderer.build_rect(
        0.0,
        0.0,
        input.strip_width,
        strip_h,
        tokens::CHROME_BG,
        main_vertices,
    );

    // 2. Bottom border of the strip.
    renderer.build_rect(
        0.0,
        strip_h,
        input.strip_width,
        border_h,
        tokens::CHROME_BORDER,
        main_vertices,
    );

    // 3. Each tab.
    for tab in input.tabs {
        let is_active = tab.index == input.active_index;
        let bg = if is_active {
            tokens::CHROME_BG_ACTIVE
        } else if input.hovered_index == Some(tab.index) {
            let target = [
                0x22 as f32 / 255.0,
                0x22 as f32 / 255.0,
                0x22 as f32 / 255.0,
                1.0,
            ];
            interpolate(tokens::CHROME_BG, target, input.hover_progress)
        } else {
            tokens::CHROME_BG
        };

        renderer.build_rect(tab.x, tab.y, tab.w, tab.h, bg, main_vertices);

        // Vertical separator on the right edge of the tab, except for the last tab.
        if tab.index < input.tabs.len() - 1 {
            renderer.build_rect(
                tab.x + tab.w - border_h,
                tab.y + 6.0 * s,
                border_h,
                tab.h - 12.0 * s,
                tokens::CHROME_BORDER,
                main_vertices,
            );
        }

        // Title text (centered horizontally and vertically using chrome atlas metrics).
        if let Some(title) = input.titles.get(tab.index) {
            let (cw, ch) = renderer.chrome_cell_size();
            let text_w = title.chars().count() as f32 * cw;
            let text_x = tab.x + (tab.w - text_w) / 2.0;
            let text_y = tab.y + (tab.h - ch) / 2.0;
            let color = if is_active {
                tokens::TEXT_PRIMARY
            } else {
                tokens::TEXT_MUTED
            };
            renderer.build_chrome_text_run(title, text_x, text_y, color, chrome_vertices);
        }

        // Active tab accent line (bottom 2px) — single colorful detail in the strip.
        if is_active {
            let accent_h = tokens::TAB_ACTIVE_ACCENT_HEIGHT * s;
            renderer.build_rect(
                tab.x,
                tab.y + tab.h - accent_h,
                tab.w,
                accent_h,
                tokens::ACCENT,
                main_vertices,
            );
        }
    }
}

/// Draw the focused pane's git branch right-aligned in the strip, just left of
/// the connection indicator: a small accent dot followed by the branch name.
#[allow(clippy::too_many_arguments)]
pub fn render_branch(
    branch: &str,
    strip_width: f32,
    scale: f32,
    renderer: &mut Renderer,
    shape_vertices: &mut Vec<Vertex>,
    chrome_vertices: &mut Vec<Vertex>,
) {
    if branch.is_empty() {
        return;
    }
    let strip_h = tokens::TAB_STRIP_HEIGHT * scale;
    let (cw, ch) = renderer.chrome_cell_size();
    let dot = (ch * 0.42).round();
    let gap = 6.0 * scale;
    let text_w = branch.chars().count() as f32 * cw;
    let total = dot + gap + text_w;

    // Right edge sits left of the connection indicator with a little padding.
    let indicator_reserve = (tokens::CONNECTION_INDICATOR_SIZE
        + tokens::CONNECTION_INDICATOR_RIGHT_PAD)
        * scale;
    let right_edge = strip_width - indicator_reserve - 10.0 * scale;
    let start_x = right_edge - total;

    let dot_y = (strip_h - dot) / 2.0;
    renderer.build_rect(start_x, dot_y, dot, dot, tokens::ACCENT, shape_vertices);

    let text_x = start_x + dot + gap;
    let text_y = (strip_h - ch) / 2.0;
    renderer.build_chrome_text_run(branch, text_x, text_y, tokens::TEXT_MUTED, chrome_vertices);
}

fn interpolate(a: [f32; 4], b: [f32; 4], t: f32) -> [f32; 4] {
    let t = t.clamp(0.0, 1.0);
    [
        a[0] + (b[0] - a[0]) * t,
        a[1] + (b[1] - a[1]) * t,
        a[2] + (b[2] - a[2]) * t,
        a[3] + (b[3] - a[3]) * t,
    ]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn three_tabs_split_evenly_with_reserve() {
        let tabs = layout_tabs(3, 1000.0, 36.0, 78.0, 1.0);
        assert_eq!(tabs.len(), 3);
        // Available = 1000 - 78 - 8 (indicator) - 8 (pad) = 906; per_tab = 302, clamped to 240
        assert_eq!(tabs[0].w, tokens::TAB_MAX_WIDTH);
        assert_eq!(tabs[0].x, 78.0);
        assert_eq!(tabs[1].x, 78.0 + tokens::TAB_MAX_WIDTH);
    }

    #[test]
    fn many_tabs_shrink_to_min() {
        let tabs = layout_tabs(20, 400.0, 36.0, 0.0, 1.0);
        let available = 400.0 - 0.0 - tokens::CONNECTION_INDICATOR_SIZE - tokens::CONNECTION_INDICATOR_RIGHT_PAD;
        let expected = (available / 20.0).clamp(tokens::TAB_MIN_WIDTH_ABSOLUTE, tokens::TAB_MAX_WIDTH);
        assert_eq!(tabs[0].w, expected);
    }

    #[test]
    fn hit_test_returns_correct_index() {
        let tabs = layout_tabs(3, 1000.0, 36.0, 78.0, 1.0);
        // Click in middle of tab 1 — y must fall inside the tab row, which
        // starts at TITLE_BAR_RESERVE on macOS / 0 elsewhere.
        let mid_x = tabs[1].x + tabs[1].w / 2.0;
        let mid_y = tabs[1].y + tabs[1].h / 2.0;
        assert_eq!(hit_test(&tabs, mid_x, mid_y), Some(1));
        // Click in traffic lights reserve area (left of all tabs)
        assert_eq!(hit_test(&tabs, 40.0, mid_y), None);
        // Click well below the strip
        assert_eq!(hit_test(&tabs, mid_x, tabs[1].y + tabs[1].h + 50.0), None);
    }

    #[test]
    fn zero_tabs_returns_empty() {
        assert!(layout_tabs(0, 1000.0, 36.0, 78.0, 1.0).is_empty());
    }

    #[test]
    fn interpolate_clamps_extremes() {
        let a = [0.0, 0.0, 0.0, 1.0];
        let b = [1.0, 1.0, 1.0, 1.0];
        assert_eq!(interpolate(a, b, 0.0), a);
        assert_eq!(interpolate(a, b, 1.0), b);
        assert_eq!(interpolate(a, b, -1.0), a);
        assert_eq!(interpolate(a, b, 2.0), b);
    }
}
