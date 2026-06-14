mod ai;
mod colors;
mod config;
mod pane;
mod tab;
mod theme;
mod ui;

use std::sync::Arc;
use std::time::{Duration, Instant};

use config::Config;

use winit::application::ApplicationHandler;
use winit::event::{ElementState, MouseButton, MouseScrollDelta, WindowEvent};
use winit::event_loop::{ActiveEventLoop, EventLoop};
use winit::keyboard::{Key, ModifiersState, NamedKey};
use winit::window::{Window, WindowId};

#[cfg(target_os = "macos")]
use winit::platform::macos::WindowAttributesExtMacOS;

use termai_core::CursorStyle;
use termai_renderer::{RenderCell, Renderer, Vertex};

use pane::{PaneRect, SplitDir};
use tab::TabBar;

const MIN_FONT_SIZE: f32 = 10.0;
const MAX_FONT_SIZE: f32 = 60.0;
const ZOOM_STEP: f32 = 2.0;
const TAB_BAR_HEIGHT: f32 = 0.0; // Will be set based on cell height

struct Selection {
    start_col: usize,
    start_row: usize,
    end_col: usize,
    end_row: usize,
}

impl Selection {
    fn normalized(&self) -> (usize, usize, usize, usize) {
        if self.start_row < self.end_row
            || (self.start_row == self.end_row && self.start_col <= self.end_col)
        {
            (self.start_col, self.start_row, self.end_col, self.end_row)
        } else {
            (self.end_col, self.end_row, self.start_col, self.start_row)
        }
    }
}

struct SearchState {
    query: String,
    /// Matches as (absolute_row, col) from Terminal::search()
    matches: Vec<(usize, usize)>,
    /// Index into matches for the current highlighted match
    current: usize,
}

/// Error patterns that trigger AI analysis.
/// Includes both the Go analyzer's patterns and general error indicators.
const ERROR_PATTERNS: &[&str] = &[
    "command not found",
    "no space left",
    "enospc",
    "address already in use",
    "eaddrinuse",
    "module not found",
    "cannot find module",
    "permission denied",
    "no such file or directory",
    "is not recognized",
    "segmentation fault",
    "killed",
    "abort",
    "panic:",
    "error:",
    "fatal:",
    "traceback",
    "exception",
    "failed to",
    "cannot find",
    "not found",
    "denied",
    "refused",
    "timeout",
    "connection reset",
];

struct App {
    config: Config,
    theme: colors::Theme,
    window: Option<Arc<Window>>,
    renderer: Option<Renderer>,
    tab_bar: TabBar,
    modifiers: ModifiersState,
    font_size: f32,
    scale_factor: f32,
    cursor_blink_start: Instant,
    selection: Option<Selection>,
    mouse_pressed: bool,
    mouse_just_pressed: bool,
    mouse_pos: (f64, f64),
    clipboard: Option<arboard::Clipboard>,
    search: Option<SearchState>,
    ai_client: Option<ai::AiClient>,
    ai_overlay: Option<ai::AiSuggestion>,
    /// Cooldown: don't send another analysis until this time passes.
    ai_last_analysis: Instant,
    /// URL currently under the mouse cursor (when Cmd is held).
    hovered_url: Option<(usize, usize, usize)>, // (row, start_col, end_col)
    /// Multi-click tracking for double/triple click.
    last_click_time: Instant,
    click_count: u32,
    last_click_pos: (usize, usize),
    /// Ghost text for AI autocomplete.
    ghost_text: Option<String>,
    ghost_text_debounce: Instant,
    pending_autocomplete: bool,
    hovered_tab: Option<usize>,
    hover_started: Instant,
}

impl App {
    fn new() -> Self {
        Self {
            config: Config::default(),
            theme: colors::DEFAULT.clone(),
            window: None,
            renderer: None,
            tab_bar: TabBar::new(80, 24),
            modifiers: ModifiersState::empty(),
            font_size: 14.0,
            scale_factor: 1.0,
            cursor_blink_start: Instant::now(),
            selection: None,
            mouse_pressed: false,
            mouse_just_pressed: false,
            mouse_pos: (0.0, 0.0),
            clipboard: arboard::Clipboard::new().ok(),
            search: None,
            ai_client: Some(ai::AiClient::new()),
            ai_overlay: None,
            ai_last_analysis: Instant::now(),
            hovered_url: None,
            last_click_time: Instant::now(),
            click_count: 0,
            last_click_pos: (0, 0),
            ghost_text: None,
            ghost_text_debounce: Instant::now(),
            pending_autocomplete: false,
            hovered_tab: None,
            hover_started: Instant::now(),
        }
    }

    fn tab_bar_pixel_height(&self) -> f32 {
        if self.tab_bar.tab_count() <= 1 {
            return 0.0;
        }
        theme::tokens::TAB_STRIP_HEIGHT + theme::tokens::TAB_STRIP_BORDER
    }

    /// Smooth fade opacity for the cursor, sine-cycling between CURSOR_FADE_MIN and 1.0.
    fn cursor_opacity(&self) -> f32 {
        let elapsed = self.cursor_blink_start.elapsed().as_millis();
        let phase = (elapsed % theme::tokens::CURSOR_BLINK_MS) as f32
            / theme::tokens::CURSOR_BLINK_MS as f32;
        // sine wave: 0 → 1 → 0 → -1 → 0 across phase 0..1
        // Map to opacity range [CURSOR_FADE_MIN, 1.0]
        let s = ((phase * std::f32::consts::TAU).sin() * 0.5) + 0.5; // 0..1
        theme::tokens::CURSOR_FADE_MIN + (1.0 - theme::tokens::CURSOR_FADE_MIN) * s
    }

    fn tab_titles(&self) -> Vec<String> {
        let home = dirs::home_dir();
        self.tab_bar.tabs.iter().map(|tab| {
            let cwd = find_pane_ref(&tab.root, tab.focused_pane_id)
                .and_then(|p| p.terminal.cwd.clone())
                .or_else(|| std::env::current_dir().ok());
            match cwd {
                Some(p) => ui::path_shorten::shorten(p, home.as_deref(), 20),
                None => tab.title.clone(),
            }
        }).collect()
    }

    fn search_bar_pixel_height(&self) -> f32 {
        0.0
    }

    fn content_area(&self) -> (f32, f32, f32, f32) {
        if let Some(ref renderer) = self.renderer {
            let w = renderer.width() as f32;
            let h = renderer.height() as f32;
            let tab_h = self.tab_bar_pixel_height();
            let search_h = self.search_bar_pixel_height();
            let x = theme::tokens::CONTENT_PADDING_LEFT;
            let y = tab_h + theme::tokens::CONTENT_PADDING_TOP;
            let cw = w - x - theme::tokens::CONTENT_PADDING_RIGHT;
            let ch = h - y - search_h - theme::tokens::CONTENT_PADDING_BOTTOM;
            (x, y, cw, ch)
        } else {
            (0.0, 0.0, 0.0, 0.0)
        }
    }

    fn pixel_to_cell_in_pane(&self, px: f64, py: f64, rect: &PaneRect) -> (usize, usize) {
        if let Some(ref renderer) = self.renderer {
            let (cw, ch) = renderer.cell_size();
            let x = px as f32 * self.scale_factor - rect.x;
            let y = py as f32 * self.scale_factor - rect.y;
            let col = (x / cw).floor().max(0.0) as usize;
            let row = (y / ch).floor().max(0.0) as usize;
            (col, row)
        } else {
            (0, 0)
        }
    }

    fn find_pane_at(&self, px: f64, py: f64) -> Option<PaneRect> {
        let (cx, cy, cw, ch) = self.content_area();
        let rects = self.tab_bar.tabs[self.tab_bar.active]
            .layout(cx, cy, cw, ch);
        let sx = px as f32 * self.scale_factor;
        let sy = py as f32 * self.scale_factor;
        rects.into_iter().find(|r| {
            sx >= r.x && sx < r.x + r.w && sy >= r.y && sy < r.y + r.h
        })
    }

    fn build_pane_cells(
        &self,
        pane: &pane::Pane,
        is_focused: bool,
    ) -> Vec<Vec<RenderCell>> {
        let visible = pane.terminal.visible_grid();
        let cursor_shown = is_focused
            && pane.terminal.cursor_visible
            && pane.terminal.scroll_offset == 0;
        let cursor_alpha = if cursor_shown { self.cursor_opacity() } else { 0.0 };

        visible
            .iter()
            .enumerate()
            .map(|(row_idx, row)| {
                row.iter()
                    .enumerate()
                    .map(|(col_idx, cell)| {
                        let mut fg = colors::resolve_fg(&self.theme, cell.fg, cell.bold);
                        let mut bg = colors::resolve_bg(&self.theme, cell.bg);

                        if cell.inverse {
                            std::mem::swap(&mut fg, &mut bg);
                        }

                        // URL hover highlight
                        if let Some((ur, us, ue)) = self.hovered_url {
                            if row_idx == ur && col_idx >= us && col_idx < ue {
                                fg = [0.4, 0.6, 1.0, 1.0]; // link blue
                            }
                        }

                        if cursor_shown
                            && row_idx == pane.terminal.cursor_y
                            && col_idx == pane.terminal.cursor_x
                        {
                            match pane.terminal.cursor_style {
                                CursorStyle::Block => {
                                    fg = self.theme.bg;
                                    let mut c = self.theme.cursor;
                                    c[3] = cursor_alpha;
                                    bg = c;
                                }
                                CursorStyle::Underline | CursorStyle::Bar => {
                                    let mut c = self.theme.cursor_bar();
                                    c[3] = cursor_alpha;
                                    bg = c;
                                }
                            }
                        }

                        RenderCell { ch: cell.c, fg, bg }
                    })
                    .collect()
            })
            .collect()
    }

    /// Check if a click is in the tab bar and switch tabs if so. Returns true if handled.
    fn handle_tab_bar_click(&mut self, px: f64, py: f64) -> bool {
        let tab_h = self.tab_bar_pixel_height();
        if tab_h == 0.0 {
            return false;
        }
        let sy = py as f32 * self.scale_factor;
        if sy >= tab_h {
            return false;
        }
        let strip_width = self.renderer.as_ref().map(|r| r.width() as f32).unwrap_or(0.0);
        let tab_layout = ui::tab_bar::layout_tabs(
            self.tab_bar.tab_count(),
            strip_width,
            theme::tokens::TAB_STRIP_HEIGHT,
            theme::tokens::TRAFFIC_LIGHTS_RESERVE,
        );
        let sx = px as f32 * self.scale_factor;
        if let Some(idx) = ui::tab_bar::hit_test(&tab_layout, sx, sy) {
            self.tab_bar.switch_to(idx);
            return true;
        }
        false
    }

    /// Resize all panes in the active tab to match the current layout.
    fn resize_panes(&mut self) {
        if let Some(ref renderer) = self.renderer {
            let (cx, cy, cw, ch) = self.content_area();
            let (cell_w, cell_h) = renderer.cell_size();
            let tab = self.tab_bar.active_tab();
            let rects = tab.root.layout(cx, cy, cw, ch);
            tab.root.resize_all(&rects, cell_w, cell_h);
        }
    }

    fn zoom(&mut self) {
        if let Some(ref mut renderer) = self.renderer {
            renderer.rebuild_atlas(self.font_size, self.scale_factor);
        }
        self.resize_panes();
    }

    fn copy_selection(&mut self) {
        let tab = &self.tab_bar.tabs[self.tab_bar.active];
        if let Some(ref sel) = self.selection {
            let (sc, sr, ec, er) = sel.normalized();
            // Find focused pane and extract text
            if let Some(pane) = self.find_focused_pane_ref() {
                let text = pane.terminal.get_text(sc, sr, ec, er);
                if !text.is_empty() {
                    if let Some(ref mut clip) = self.clipboard {
                        let _ = clip.set_text(&text);
                    }
                }
            }
        }
    }

    fn find_focused_pane_ref(&self) -> Option<&pane::Pane> {
        let tab = &self.tab_bar.tabs[self.tab_bar.active];
        find_pane_ref(&tab.root, tab.focused_pane_id)
    }

    fn update_search(&mut self) {
        if let Some(ref mut search) = self.search {
            let tab = &self.tab_bar.tabs[self.tab_bar.active];
            if let Some(pane) = find_pane_ref(&tab.root, tab.focused_pane_id) {
                search.matches = pane.terminal.search(&search.query);
                if search.matches.is_empty() {
                    search.current = 0;
                } else {
                    search.current = search.current.min(search.matches.len() - 1);
                }
            }
        }
    }

    fn search_jump_to_current(&mut self) {
        if let Some(ref search) = self.search {
            if let Some(&(abs_row, _col)) = search.matches.get(search.current) {
                let tab = self.tab_bar.active_tab();
                if let Some(pane) = tab.root.find_pane(tab.focused_pane_id) {
                    let sb_len = pane.terminal.scrollback.len();
                    if abs_row < sb_len {
                        // In scrollback: set offset so this row is visible
                        pane.terminal.scroll_offset = sb_len - abs_row;
                    } else {
                        // In grid: snap to bottom
                        pane.terminal.scroll_offset = 0;
                    }
                }
            }
        }
    }

    fn search_next(&mut self) {
        if let Some(ref mut search) = self.search {
            if !search.matches.is_empty() {
                search.current = (search.current + 1) % search.matches.len();
            }
        }
        self.search_jump_to_current();
    }

    fn search_prev(&mut self) {
        if let Some(ref mut search) = self.search {
            if !search.matches.is_empty() {
                search.current = if search.current == 0 {
                    search.matches.len() - 1
                } else {
                    search.current - 1
                };
            }
        }
        self.search_jump_to_current();
    }

    /// Check if the focused pane's recent output contains an error pattern.
    /// If so, send it to the AI engine for analysis.
    fn check_for_errors(&mut self) {
        // Cooldown: wait at least 2 seconds between analyses
        if self.ai_last_analysis.elapsed() < Duration::from_secs(2) {
            return;
        }

        // Don't analyze if overlay is already showing
        if self.ai_overlay.is_some() {
            return;
        }

        let ai_client = match self.ai_client {
            Some(ref c) => c,
            None => return,
        };

        let tab = &mut self.tab_bar.tabs[self.tab_bar.active];
        let focused_id = tab.focused_pane_id;
        let pane = match tab.root.find_pane(focused_id) {
            Some(p) => p,
            None => return,
        };

        if pane.recent_output.is_empty() {
            return;
        }

        let output_lower = pane.recent_output.to_lowercase();

        // Check for any known error pattern
        let has_error = ERROR_PATTERNS.iter().any(|p| output_lower.contains(p));
        if !has_error {
            return;
        }

        // Try to extract the last command line from the output.
        // Heuristic: look for the line containing the error and the line before it.
        let lines: Vec<&str> = pane.recent_output.lines().collect();
        let mut error_line = "";
        let mut command_line = "";

        for (i, line) in lines.iter().enumerate() {
            let lower = line.to_lowercase();
            if ERROR_PATTERNS.iter().any(|p| lower.contains(p)) {
                error_line = line;
                // The command is often the line before the error, or embedded in it
                // e.g., "zsh: command not found: gi" — extract "gi"
                // e.g., "bash: nvm: command not found" — extract "nvm"
                if i > 0 {
                    command_line = lines[i - 1];
                }
                break;
            }
        }

        // Extract the command from shell error messages
        let command = extract_command_from_error(error_line, command_line);

        ai_client.analyze(&command, error_line, 127);
        self.ai_last_analysis = Instant::now();
        pane.clear_recent_output();
    }

    fn paste(&mut self) {
        if let Some(ref mut clip) = self.clipboard {
            if let Ok(text) = clip.get_text() {
                let tab = self.tab_bar.active_tab();
                if let Some(pane) = tab.root.find_pane(tab.focused_pane_id) {
                    pane.write(text.as_bytes());
                }
            }
        }
    }
}

/// Try to extract the failed command from error output.
/// e.g., "zsh: command not found: gi" → "gi"
/// e.g., "bash: nvm: command not found" → "nvm"
fn extract_command_from_error(error_line: &str, command_line: &str) -> String {
    // "zsh: command not found: <cmd>"
    if let Some(idx) = error_line.find("command not found: ") {
        return error_line[idx + "command not found: ".len()..].trim().to_string();
    }
    // "bash: <cmd>: command not found"
    if error_line.contains("command not found") {
        let parts: Vec<&str> = error_line.split(':').collect();
        if parts.len() >= 2 {
            let cmd = parts[1].trim();
            if !cmd.is_empty() && cmd != "command not found" {
                return cmd.to_string();
            }
        }
    }
    // Fall back to the command line (often the prompt line before the error)
    // Strip common prompt patterns: "user@host path % <cmd>" or "$ <cmd>"
    let trimmed = command_line.trim();
    if let Some(idx) = trimmed.rfind("% ") {
        return trimmed[idx + 2..].to_string();
    }
    if let Some(idx) = trimmed.rfind("$ ") {
        return trimmed[idx + 2..].to_string();
    }
    trimmed.to_string()
}

/// Find word boundaries around the given column in a line of characters.
fn find_word_bounds(line: &[char], col: usize) -> (usize, usize) {
    if col >= line.len() {
        return (col, col);
    }

    let is_delimiter = |c: char| -> bool {
        c.is_whitespace()
            || matches!(
                c,
                '/' | ':' | '.' | ',' | ';' | '(' | ')' | '[' | ']' | '{' | '}' | '<' | '>'
                    | '"' | '\'' | '|' | '&' | '=' | '+' | '-' | '*' | '!' | '?' | '#' | '@'
                    | '$' | '%' | '^' | '~' | '`'
            )
    };

    if is_delimiter(line[col]) {
        return (col, col + 1);
    }

    let mut start = col;
    while start > 0 && !is_delimiter(line[start - 1]) {
        start -= 1;
    }

    let mut end = col + 1;
    while end < line.len() && !is_delimiter(line[end]) {
        end += 1;
    }

    (start, end)
}

fn find_pane_ref(node: &pane::PaneNode, id: u64) -> Option<&pane::Pane> {
    match node {
        pane::PaneNode::Leaf(pane) => {
            if pane.id == id {
                Some(pane)
            } else {
                None
            }
        }
        pane::PaneNode::Split { first, second, .. } => {
            find_pane_ref(first, id).or_else(|| find_pane_ref(second, id))
        }
    }
}

impl ApplicationHandler for App {
    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        if self.window.is_some() {
            return;
        }

        let mut attrs = Window::default_attributes()
            .with_title(&self.config.window.title)
            .with_inner_size(winit::dpi::LogicalSize::new(
                self.config.window.width,
                self.config.window.height,
            ));

        #[cfg(target_os = "macos")]
        {
            attrs = attrs
                .with_titlebar_transparent(true)
                .with_fullsize_content_view(true);
        }

        let window = Arc::new(
            event_loop
                .create_window(attrs)
                .expect("Failed to create window"),
        );

        self.scale_factor = window.scale_factor() as f32;
        let mut renderer = Renderer::new(window.clone(), self.scale_factor, self.font_size);
        renderer.clear_color = self.theme.bg;

        let (cols, rows) = renderer.grid_size();
        self.tab_bar = TabBar::new(cols as usize, rows as usize);
        self.renderer = Some(renderer);
        self.window = Some(window);
    }

    fn window_event(&mut self, event_loop: &ActiveEventLoop, _id: WindowId, event: WindowEvent) {
        match event {
            WindowEvent::CloseRequested => {
                event_loop.exit();
            }

            WindowEvent::Resized(size) => {
                if let Some(ref mut renderer) = self.renderer {
                    renderer.resize(size.width, size.height);
                }
                self.resize_panes();
            }

            WindowEvent::ModifiersChanged(modifiers) => {
                self.modifiers = modifiers.state();
            }

            WindowEvent::MouseWheel { delta, .. } => {
                let tab = self.tab_bar.active_tab();
                let focused_id = tab.focused_pane_id;
                if let Some(pane) = tab.root.find_pane(focused_id) {
                    match delta {
                        MouseScrollDelta::LineDelta(_, y) => {
                            let n = (y.abs() as usize).max(1);
                            if y > 0.0 {
                                pane.terminal.scroll_viewport_up(n);
                            } else {
                                pane.terminal.scroll_viewport_down(n);
                            }
                        }
                        MouseScrollDelta::PixelDelta(pos) => {
                            let n = ((pos.y.abs() / 20.0) as usize).max(1);
                            if pos.y > 0.0 {
                                pane.terminal.scroll_viewport_up(n);
                            } else {
                                pane.terminal.scroll_viewport_down(n);
                            }
                        }
                    }
                }
            }

            WindowEvent::MouseInput { state, button, .. } => {
                if button == MouseButton::Left {
                    match state {
                        ElementState::Pressed => {
                            // Try tab bar click with current mouse pos
                            if self.handle_tab_bar_click(self.mouse_pos.0, self.mouse_pos.1) {
                                return;
                            }

                            // Cmd+click on URL: detect URL at click position and open it
                            if self.modifiers.super_key() {
                                if let Some(rect) = self.find_pane_at(self.mouse_pos.0, self.mouse_pos.1) {
                                    let (col, row) = self.pixel_to_cell_in_pane(
                                        self.mouse_pos.0, self.mouse_pos.1, &rect,
                                    );
                                    let tab = &self.tab_bar.tabs[self.tab_bar.active];
                                    if let Some(pane) = find_pane_ref(&tab.root, rect.id) {
                                        let urls = pane.terminal.detect_urls();
                                        for (ur, us, ue) in &urls {
                                            if row == *ur && col >= *us && col < *ue {
                                                let visible = pane.terminal.visible_grid();
                                                if row < visible.len() {
                                                    let url: String = visible[row].iter()
                                                        .skip(*us).take(*ue - *us)
                                                        .map(|c| c.c).collect();
                                                    let url = url.trim().to_string();
                                                    if !url.is_empty() {
                                                        #[cfg(target_os = "macos")]
                                                        let opener = "open";
                                                        #[cfg(target_os = "linux")]
                                                        let opener = "xdg-open";
                                                        #[cfg(not(any(target_os = "macos", target_os = "linux")))]
                                                        let opener = "open";
                                                        let _ = std::process::Command::new(opener)
                                                            .arg(&url).spawn();
                                                    }
                                                }
                                                return;
                                            }
                                        }
                                    }
                                }
                            }

                            // Multi-click detection
                            let now = Instant::now();
                            if let Some(rect) = self.find_pane_at(self.mouse_pos.0, self.mouse_pos.1) {
                                let (col, row) = self.pixel_to_cell_in_pane(
                                    self.mouse_pos.0, self.mouse_pos.1, &rect,
                                );
                                // Allow nearby cells (within 2) to handle Retina subpixel jitter
                                let col_diff = (col as i64 - self.last_click_pos.0 as i64).unsigned_abs();
                                let is_nearby = col_diff <= 2
                                    && row == self.last_click_pos.1;
                                let is_fast = now.duration_since(self.last_click_time)
                                    < Duration::from_millis(400);

                                if is_fast && is_nearby && self.click_count >= 1 && self.click_count < 3 {
                                    self.click_count += 1;
                                } else {
                                    self.click_count = 1;
                                }
                                self.last_click_time = now;
                                self.last_click_pos = (col, row);

                                match self.click_count {
                                    2 => {
                                        // Double-click: select word
                                        self.tab_bar.active_tab().focused_pane_id = rect.id;
                                        let tab = &self.tab_bar.tabs[self.tab_bar.active];
                                        if let Some(pane) = find_pane_ref(&tab.root, rect.id) {
                                            let visible = pane.terminal.visible_grid();
                                            if row < visible.len() {
                                                let line: Vec<char> = visible[row]
                                                    .iter().map(|c| c.c).collect();
                                                let (ws, we) = find_word_bounds(&line, col);
                                                self.selection = Some(Selection {
                                                    start_col: ws,
                                                    start_row: row,
                                                    end_col: we,
                                                    end_row: row,
                                                });
                                                self.copy_selection();
                                            }
                                        }
                                        self.mouse_pressed = false;
                                        return;
                                    }
                                    3 => {
                                        // Triple-click: select line
                                        self.tab_bar.active_tab().focused_pane_id = rect.id;
                                        let tab = &self.tab_bar.tabs[self.tab_bar.active];
                                        if let Some(pane) = find_pane_ref(&tab.root, rect.id) {
                                            self.selection = Some(Selection {
                                                start_col: 0,
                                                start_row: row,
                                                end_col: pane.terminal.cols,
                                                end_row: row,
                                            });
                                            self.copy_selection();
                                        }
                                        self.mouse_pressed = false;
                                        return;
                                    }
                                    _ => {} // single click: fall through
                                }
                            }

                            self.mouse_pressed = true;
                            self.mouse_just_pressed = true;
                            self.selection = None;
                        }
                        ElementState::Released => {
                            self.mouse_pressed = false;
                        }
                    }
                }
            }

            WindowEvent::CursorMoved { position, .. } => {
                self.mouse_pos = (position.x, position.y);

                // URL hover detection when Cmd is held
                self.hovered_url = None;
                if self.modifiers.super_key() {
                    if let Some(rect) = self.find_pane_at(position.x, position.y) {
                        let (col, row) = self.pixel_to_cell_in_pane(position.x, position.y, &rect);
                        let tab = &self.tab_bar.tabs[self.tab_bar.active];
                        if let Some(pane) = find_pane_ref(&tab.root, rect.id) {
                            let urls = pane.terminal.detect_urls();
                            for (ur, us, ue) in &urls {
                                if row == *ur && col >= *us && col < *ue {
                                    self.hovered_url = Some((*ur, *us, *ue));
                                    break;
                                }
                            }
                        }
                    }
                }

                // Tab bar hover tracking
                let new_hover = {
                    let sx = position.x as f32 * self.scale_factor;
                    let sy = position.y as f32 * self.scale_factor;
                    let tab_h = self.tab_bar_pixel_height();
                    if tab_h > 0.0 && sy < tab_h {
                        let strip_width = self.renderer.as_ref().map(|r| r.width() as f32).unwrap_or(0.0);
                        let tab_layout = ui::tab_bar::layout_tabs(
                            self.tab_bar.tab_count(),
                            strip_width,
                            theme::tokens::TAB_STRIP_HEIGHT,
                            theme::tokens::TRAFFIC_LIGHTS_RESERVE,
                        );
                        ui::tab_bar::hit_test(&tab_layout, sx, sy)
                    } else {
                        None
                    }
                };
                if new_hover != self.hovered_tab {
                    self.hovered_tab = new_hover;
                    if new_hover.is_some() {
                        self.hover_started = Instant::now();
                    }
                    if let Some(ref window) = self.window {
                        window.request_redraw();
                    }
                }

                // Handle click on tab bar (on first move after press)
                if self.mouse_just_pressed {
                    self.mouse_just_pressed = false;
                    if self.handle_tab_bar_click(position.x, position.y) {
                        self.mouse_pressed = false;
                        return;
                    }
                }
                // Click to focus pane + drag selection
                if self.mouse_pressed {
                    if let Some(rect) = self.find_pane_at(position.x, position.y) {
                        self.tab_bar.active_tab().focused_pane_id = rect.id;
                        let (col, row) = self.pixel_to_cell_in_pane(position.x, position.y, &rect);
                        if let Some(ref mut sel) = self.selection {
                            sel.end_col = col;
                            sel.end_row = row;
                        } else {
                            self.selection = Some(Selection {
                                start_col: col,
                                start_row: row,
                                end_col: col,
                                end_row: row,
                            });
                        }
                    }
                }
            }

            WindowEvent::KeyboardInput { event, .. } => {
                if event.state != ElementState::Pressed {
                    return;
                }

                self.cursor_blink_start = Instant::now();

                // AI overlay interaction: number keys to execute, Escape to dismiss
                if self.ai_overlay.is_some() {
                    match &event.logical_key {
                        Key::Named(NamedKey::Escape) => {
                            self.ai_overlay = None;
                            return;
                        }
                        Key::Character(c) if c.len() == 1 && c.as_bytes()[0] >= b'1' && c.as_bytes()[0] <= b'9' => {
                            let idx = (c.as_bytes()[0] - b'1') as usize;
                            let command = self.ai_overlay.as_ref()
                                .and_then(|s| s.actions.get(idx))
                                .map(|a| a.command.clone());
                            if let Some(cmd) = command {
                                self.ai_overlay = None;
                                let tab = self.tab_bar.active_tab();
                                let id = tab.focused_pane_id;
                                if let Some(pane) = tab.root.find_pane(id) {
                                    // Write command + Enter to PTY
                                    pane.write(cmd.as_bytes());
                                    pane.write(b"\r");
                                }
                            }
                            return;
                        }
                        _ => {
                            // Any other key dismisses the overlay
                            self.ai_overlay = None;
                        }
                    }
                }

                let is_super = self.modifiers.super_key();
                let is_shift = self.modifiers.shift_key();

                if is_super {
                    match &event.logical_key {
                        // Zoom
                        Key::Character(c) if c.as_str() == "=" || c.as_str() == "+" => {
                            self.font_size = (self.font_size + ZOOM_STEP).min(MAX_FONT_SIZE);
                            self.zoom();
                            return;
                        }
                        Key::Character(c) if c.as_str() == "-" => {
                            self.font_size = (self.font_size - ZOOM_STEP).max(MIN_FONT_SIZE);
                            self.zoom();
                            return;
                        }
                        Key::Character(c) if c.as_str() == "0" => {
                            self.font_size = 14.0;
                            self.zoom();
                            return;
                        }
                        // Split vertical: Cmd+D
                        Key::Character(c) if c.as_str() == "d" && !is_shift => {
                            let (cx, cy, cw, ch) = self.content_area();
                            if let Some(ref renderer) = self.renderer {
                                let (cols, rows) = renderer.grid_size_for(cw / 2.0, ch);
                                self.tab_bar.active_tab().split(
                                    SplitDir::Vertical,
                                    cols as usize,
                                    rows as usize,
                                );
                            }
                            self.resize_panes();
                            return;
                        }
                        // Split horizontal: Cmd+Shift+D
                        Key::Character(c) if (c.as_str() == "d" || c.as_str() == "D") && is_shift => {
                            let (cx, cy, cw, ch) = self.content_area();
                            if let Some(ref renderer) = self.renderer {
                                let (cols, rows) = renderer.grid_size_for(cw, ch / 2.0);
                                self.tab_bar.active_tab().split(
                                    SplitDir::Horizontal,
                                    cols as usize,
                                    rows as usize,
                                );
                            }
                            self.resize_panes();
                            return;
                        }
                        // New tab: Cmd+T
                        Key::Character(c) if c.as_str() == "t" => {
                            if let Some(ref renderer) = self.renderer {
                                let (cols, rows) = renderer.grid_size();
                                self.tab_bar.new_tab(cols as usize, rows as usize);
                            }
                            return;
                        }
                        // Close pane/tab: Cmd+W
                        Key::Character(c) if c.as_str() == "w" => {
                            let tab = self.tab_bar.active_tab();
                            if !tab.close_focused() {
                                // Last pane in tab — close tab
                                if !self.tab_bar.close_active_tab() {
                                    // Last tab — exit
                                    event_loop.exit();
                                }
                            }
                            return;
                        }
                        // Focus next pane: Cmd+]
                        Key::Character(c) if c.as_str() == "]" => {
                            self.tab_bar.active_tab().focus_next();
                            return;
                        }
                        // Focus prev pane: Cmd+[
                        Key::Character(c) if c.as_str() == "[" => {
                            self.tab_bar.active_tab().focus_prev();
                            return;
                        }
                        // Search: Cmd+F
                        Key::Character(c) if c.as_str() == "f" => {
                            if self.search.is_none() {
                                self.search = Some(SearchState {
                                    query: String::new(),
                                    matches: Vec::new(),
                                    current: 0,
                                });
                            }
                            return;
                        }
                        // Next match: Cmd+G
                        Key::Character(c) if c.as_str() == "g" && !is_shift => {
                            self.search_next();
                            return;
                        }
                        // Prev match: Cmd+Shift+G
                        Key::Character(c) if (c.as_str() == "g" || c.as_str() == "G") && is_shift => {
                            self.search_prev();
                            return;
                        }
                        // Copy: Cmd+C
                        Key::Character(c) if c.as_str() == "c" => {
                            self.copy_selection();
                            return;
                        }
                        // Paste: Cmd+V
                        Key::Character(c) if c.as_str() == "v" => {
                            self.paste();
                            return;
                        }
                        // Tab switching: Cmd+1-9
                        Key::Character(c)
                            if c.len() == 1 && c.as_bytes()[0] >= b'1' && c.as_bytes()[0] <= b'9' =>
                        {
                            let idx = (c.as_bytes()[0] - b'1') as usize;
                            self.tab_bar.switch_to(idx);
                            return;
                        }
                        _ => {}
                    }
                }

                // Shift+PageUp/Down for scrollback
                if is_shift {
                    match &event.logical_key {
                        Key::Named(NamedKey::PageUp) => {
                            let tab = self.tab_bar.active_tab();
                            let id = tab.focused_pane_id;
                            if let Some(pane) = tab.root.find_pane(id) {
                                let half = pane.terminal.rows / 2;
                                pane.terminal.scroll_viewport_up(half.max(1));
                            }
                            return;
                        }
                        Key::Named(NamedKey::PageDown) => {
                            let tab = self.tab_bar.active_tab();
                            let id = tab.focused_pane_id;
                            if let Some(pane) = tab.root.find_pane(id) {
                                let half = pane.terminal.rows / 2;
                                pane.terminal.scroll_viewport_down(half.max(1));
                            }
                            return;
                        }
                        _ => {}
                    }
                }

                // Search mode: route keyboard to search query
                if self.search.is_some() {
                    match &event.logical_key {
                        Key::Named(NamedKey::Escape) => {
                            self.search = None;
                        }
                        Key::Named(NamedKey::Enter) => {
                            if is_shift {
                                self.search_prev();
                            } else {
                                self.search_next();
                            }
                        }
                        Key::Named(NamedKey::Backspace) => {
                            if let Some(ref mut search) = self.search {
                                search.query.pop();
                            }
                            self.update_search();
                        }
                        Key::Character(_) => {
                            if let Some(text) = &event.text {
                                if let Some(ref mut search) = self.search {
                                    search.query.push_str(text.as_str());
                                }
                                self.update_search();
                                self.search_jump_to_current();
                            }
                        }
                        _ => {}
                    }
                    return;
                }

                self.selection = None;

                // Ghost text acceptance: Tab or Right arrow accepts the suggestion
                if self.ghost_text.is_some() {
                    match &event.logical_key {
                        Key::Named(NamedKey::Tab) | Key::Named(NamedKey::ArrowRight) => {
                            if let Some(text) = self.ghost_text.take() {
                                let tab = self.tab_bar.active_tab();
                                let id = tab.focused_pane_id;
                                if let Some(pane) = tab.root.find_pane(id) {
                                    pane.write(text.as_bytes());
                                }
                            }
                            return;
                        }
                        _ => {
                            self.ghost_text = None;
                        }
                    }
                }

                // Send key to focused pane
                let tab = self.tab_bar.active_tab();
                let id = tab.focused_pane_id;
                if let Some(pane) = tab.root.find_pane(id) {
                    let bytes = key_to_bytes(&event.logical_key, &event.text);
                    if !bytes.is_empty() {
                        // Dismiss AI overlay on new input
                        self.ai_overlay = None;
                        self.ghost_text_debounce = Instant::now();
                        pane.write(&bytes);
                    }
                }
            }

            WindowEvent::RedrawRequested => {
                // Poll all panes in active tab
                let got_data = self.tab_bar.active_tab().poll();

                // Check for errors in PTY output and send to AI
                if got_data {
                    self.check_for_errors();
                }

                // Poll AI client for suggestions and completions
                if let Some(ref ai_client) = self.ai_client {
                    if let Some(msg) = ai_client.poll() {
                        match msg {
                            ai::AiMessage::Suggestion(s) => {
                                self.ai_overlay = Some(s);
                            }
                            ai::AiMessage::NoSuggestion => {}
                            ai::AiMessage::Completion(text) => {
                                self.ghost_text = Some(text);
                                self.pending_autocomplete = false;
                            }
                            ai::AiMessage::NoCompletion => {
                                self.pending_autocomplete = false;
                            }
                        }
                    }
                }

                // Autocomplete debounce: request after 300ms of idle typing
                if self.ghost_text.is_none() && !self.pending_autocomplete {
                    let elapsed = self.ghost_text_debounce.elapsed();
                    if elapsed > Duration::from_millis(300) && elapsed < Duration::from_millis(500) {
                        if let Some(pane) = self.find_focused_pane_ref() {
                            if pane.terminal.cursor_x > 2 {
                                let row = &pane.terminal.grid[pane.terminal.cursor_y];
                                let line: String = row.iter()
                                    .take(pane.terminal.cursor_x)
                                    .map(|c| c.c).collect();
                                let line = line.trim().to_string();
                                if line.len() > 2 {
                                    if let Some(ref ai_client) = self.ai_client {
                                        ai_client.autocomplete(&line, ".", "");
                                        self.pending_autocomplete = true;
                                    }
                                }
                            }
                        }
                    }
                }

                // Auto-dismiss overlay after 10 seconds
                if let Some(ref overlay) = self.ai_overlay {
                    if overlay.created.elapsed() > Duration::from_secs(10) {
                        self.ai_overlay = None;
                    }
                }

                if self.renderer.is_none() {
                    return;
                }

                // Build all cell data first (before borrowing renderer mutably)
                let titles = self.tab_titles();
                let tab_layout = if self.tab_bar.tab_count() > 1 {
                    let strip_width = self.renderer.as_ref().map(|r| r.width() as f32).unwrap_or(0.0);
                    ui::tab_bar::layout_tabs(
                        self.tab_bar.tab_count(),
                        strip_width,
                        theme::tokens::TAB_STRIP_HEIGHT,
                        theme::tokens::TRAFFIC_LIGHTS_RESERVE,
                    )
                } else {
                    vec![]
                };
                let (cx, cy, cw, ch) = self.content_area();
                let tab = &self.tab_bar.tabs[self.tab_bar.active];
                let rects = tab.layout(cx, cy, cw, ch);
                let focused_id = tab.focused_pane_id;
                let theme_bg = self.theme.bg;
                let div_color = self.theme.divider();

                let mut pane_cells: Vec<(PaneRect, Vec<Vec<RenderCell>>)> = Vec::new();
                for rect in &rects {
                    if let Some(pane) = find_pane_ref(&tab.root, rect.id) {
                        let is_focused = rect.id == focused_id;
                        let cells = self.build_pane_cells(pane, is_focused);
                        pane_cells.push((rect.clone(), cells));
                    }
                }

                // Ghost text (autocomplete suggestion) data
                let ghost_data = if let Some(ref ghost) = self.ghost_text {
                    if let Some(pane) = find_pane_ref(&tab.root, focused_id) {
                        if pane.terminal.scroll_offset == 0 {
                            let cursor_x = pane.terminal.cursor_x;
                            let cursor_y = pane.terminal.cursor_y;
                            let ghost_cells: Vec<Vec<RenderCell>> = vec![
                                ghost.chars().map(|c| RenderCell {
                                    ch: c,
                                    fg: theme::tokens::TEXT_DIM,
                                    bg: theme_bg,
                                }).collect()
                            ];
                            Some((cursor_x, cursor_y, ghost_cells))
                        } else { None }
                    } else { None }
                } else { None };

                // Pre-compute selection geometry before mutably borrowing renderer
                // (sx, sy, ex, ey, pane_cols) for the translucent overlay
                let sel_data: Option<(usize, usize, usize, usize, usize)> =
                    self.selection.as_ref().and_then(|sel_obj| {
                        let (sx, sy, ex, ey) = sel_obj.normalized();
                        let pane_cols = find_pane_ref(&tab.root, focused_id)
                            .map(|p| p.terminal.cols)
                            .unwrap_or(80);
                        Some((sx, sy, ex, ey, pane_cols))
                    });

                // Pre-compute values that require &self before we mutably borrow self.renderer
                let tab_bar_h = self.tab_bar_pixel_height();

                // Now borrow renderer mutably for vertex building
                let renderer = self.renderer.as_mut().unwrap();
                let mut vertices: Vec<Vertex> = Vec::new();
                let mut chrome_vertices: Vec<Vertex> = Vec::new();

                if !tab_layout.is_empty() {
                    let hover_progress = if self.hovered_tab.is_some() {
                        let elapsed = self.hover_started.elapsed().as_millis() as f32;
                        (elapsed / theme::tokens::HOVER_TRANSITION_MS as f32).min(1.0)
                    } else {
                        0.0
                    };
                    let strip_width = renderer.width() as f32;
                    let input = ui::tab_bar::TabBarRenderInput {
                        tabs: &tab_layout,
                        active_index: self.tab_bar.active,
                        hovered_index: self.hovered_tab,
                        hover_progress,
                        titles: &titles,
                        strip_width,
                    };
                    ui::tab_bar::render_tab_bar(&input, renderer, &mut vertices, &mut chrome_vertices);

                    if self.tab_bar.tab_count() > 1 {
                        let state = match self.ai_client.as_ref() {
                            Some(c) if c.is_analyzing() => ui::connection_indicator::State::Analyzing,
                            Some(c) if c.is_connected() => ui::connection_indicator::State::Connected,
                            _ => ui::connection_indicator::State::Disconnected,
                        };

                        let pulse_t = if matches!(state, ui::connection_indicator::State::Analyzing) {
                            let elapsed = self.cursor_blink_start.elapsed().as_millis();
                            let phase = (elapsed % theme::tokens::PULSE_PERIOD_MS) as f32
                                / theme::tokens::PULSE_PERIOD_MS as f32;
                            (phase * std::f32::consts::TAU).sin() * 0.5 + 0.5
                        } else {
                            0.0
                        };

                        ui::connection_indicator::render(
                            state,
                            renderer.width() as f32,
                            pulse_t,
                            renderer,
                            &mut vertices,
                        );
                    }
                }

                // Pane content
                for rect in &rects {
                    renderer.build_rect(rect.x, rect.y, rect.w, rect.h, theme_bg, &mut vertices);
                }
                for (rect, cells) in &pane_cells {
                    renderer.build_vertices(cells, rect.x, rect.y, &mut vertices);
                }

                // Outline cursor for unfocused panes.
                for rect in &rects {
                    if rect.id == focused_id {
                        continue;
                    }
                    let pane = match find_pane_ref(&tab.root, rect.id) {
                        Some(p) => p,
                        None => continue,
                    };
                    if !pane.terminal.cursor_visible || pane.terminal.scroll_offset != 0 {
                        continue;
                    }
                    let (cw_px, ch_px) = renderer.cell_size();
                    let cx_pos = rect.x + pane.terminal.cursor_x as f32 * cw_px;
                    let cy_pos = rect.y + pane.terminal.cursor_y as f32 * ch_px;
                    renderer.build_rect_outline(cx_pos, cy_pos, cw_px, ch_px, 1.0, self.theme.cursor, &mut vertices);
                }

                // Dividers between panes
                if rects.len() > 1 {
                    for i in 0..rects.len() {
                        for j in (i + 1)..rects.len() {
                            let a = &rects[i];
                            let b = &rects[j];
                            if (a.x + a.w - b.x).abs() < 2.0 {
                                let div_x = a.x + a.w;
                                let div_y = a.y.min(b.y);
                                let div_h = a.h.max(b.h);
                                renderer.build_divider(div_x, div_y, 1.0, div_h, div_color, &mut vertices);
                            }
                            if (a.y + a.h - b.y).abs() < 2.0 {
                                let div_x = a.x.min(b.x);
                                let div_y = a.y + a.h;
                                let div_w = a.w.max(b.w);
                                renderer.build_divider(div_x, div_y, div_w, 1.0, div_color, &mut vertices);
                            }
                        }
                    }
                }

                // Focused pane border (above dividers, below overlays)
                if let Some(rect) = rects.iter().find(|r| r.id == focused_id) {
                    renderer.build_rect_outline(
                        rect.x,
                        rect.y,
                        rect.w,
                        rect.h,
                        1.0,
                        theme::tokens::ACCENT,
                        &mut vertices,
                    );
                }

                // Selection overlay (alpha 25% accent)
                if let Some((sx, sy, ex, ey, pane_cols)) = sel_data {
                    if let Some(rect) = rects.iter().find(|r| r.id == focused_id) {
                        let (cw_px, ch_px) = renderer.cell_size();
                        let sel_color = theme::tokens::with_alpha(
                            theme::tokens::ACCENT,
                            theme::tokens::SELECTION_ALPHA,
                        );
                        if sy == ey {
                            let w = ((ex.saturating_sub(sx)) as f32).max(1.0) * cw_px;
                            renderer.build_rect(
                                rect.x + sx as f32 * cw_px,
                                rect.y + sy as f32 * ch_px,
                                w,
                                ch_px,
                                sel_color,
                                &mut vertices,
                            );
                        } else {
                            // First row: from sx to end of pane width
                            renderer.build_rect(
                                rect.x + sx as f32 * cw_px,
                                rect.y + sy as f32 * ch_px,
                                (pane_cols - sx) as f32 * cw_px,
                                ch_px,
                                sel_color,
                                &mut vertices,
                            );
                            // Middle rows: full pane width
                            for row in (sy + 1)..ey {
                                renderer.build_rect(
                                    rect.x,
                                    rect.y + row as f32 * ch_px,
                                    pane_cols as f32 * cw_px,
                                    ch_px,
                                    sel_color,
                                    &mut vertices,
                                );
                            }
                            // Last row: from 0 to ex
                            renderer.build_rect(
                                rect.x,
                                rect.y + ey as f32 * ch_px,
                                ex as f32 * cw_px,
                                ch_px,
                                sel_color,
                                &mut vertices,
                            );
                        }
                    }
                }

                // Ghost text (autocomplete)
                if let Some((cursor_x, cursor_y, ref ghost_cells)) = ghost_data {
                    if let Some(rect) = rects.iter().find(|r| r.id == focused_id) {
                        let (cw_px, ch_px) = renderer.cell_size();
                        let ghost_x = rect.x + (cursor_x as f32) * cw_px;
                        let ghost_y = rect.y + (cursor_y as f32) * ch_px;
                        renderer.build_vertices(ghost_cells, ghost_x, ghost_y, &mut vertices);
                    }
                }

                // Search match highlights (alpha overlays)
                if let Some(ref search) = self.search {
                    if let Some(rect) = rects.iter().find(|r| r.id == focused_id) {
                        let (cw_px, ch_px) = renderer.cell_size();
                        let pane = find_pane_ref(&tab.root, focused_id);
                        let qlen = search.query.chars().count() as f32;
                        for (mi, &(abs_row, abs_col)) in search.matches.iter().enumerate() {
                            if let Some(p) = pane {
                                if let Some(vis_row) = p.terminal.abs_row_to_visible(abs_row) {
                                    let alpha = if mi == search.current {
                                        theme::tokens::SEARCH_CURRENT_MATCH_ALPHA
                                    } else {
                                        theme::tokens::SEARCH_MATCH_ALPHA
                                    };
                                    let color = theme::tokens::with_alpha(theme::tokens::ACCENT, alpha);
                                    renderer.build_rect(
                                        rect.x + abs_col as f32 * cw_px,
                                        rect.y + vis_row as f32 * ch_px,
                                        qlen * cw_px,
                                        ch_px,
                                        color,
                                        &mut vertices,
                                    );
                                }
                            }
                        }
                    }
                }

                // Floating search bar (top-right, over content)
                if let Some(ref search) = self.search {
                    let input = ui::search_bar::SearchBarInput {
                        query: &search.query,
                        match_count: search.matches.len(),
                        current_match: search.current,
                        strip_width: renderer.width() as f32,
                        content_top: tab_bar_h,
                    };
                    ui::search_bar::render(&input, renderer, &mut vertices, &mut chrome_vertices);
                }

                // AI suggestion overlay
                if let Some(ref suggestion) = self.ai_overlay {
                    if let Some(rect) = rects.iter().find(|r| r.id == focused_id) {
                        // Fade alpha: 1.0 until last 200ms of 10s lifetime, then linear fade.
                        let elapsed = suggestion.created.elapsed().as_millis();
                        let total = 10_000u128;
                        let fade_start = total.saturating_sub(theme::tokens::OVERLAY_FADE_MS);
                        let fade_alpha = if elapsed < fade_start {
                            1.0
                        } else {
                            let remaining = total.saturating_sub(elapsed) as f32;
                            (remaining / theme::tokens::OVERLAY_FADE_MS as f32).max(0.0)
                        };

                        let actions: Vec<ui::ai_overlay::ActionView> = suggestion
                            .actions
                            .iter()
                            .map(|a| ui::ai_overlay::ActionView {
                                label: &a.label,
                                risk: ui::ai_overlay::Risk::from_str(&a.risk),
                            })
                            .collect();

                        let input = ui::ai_overlay::AiOverlayInput {
                            title: &suggestion.title,
                            description: &suggestion.description,
                            actions: &actions,
                            pane_rect: (rect.x, rect.y, rect.w, rect.h),
                            fade_alpha,
                        };
                        ui::ai_overlay::render(
                            &input,
                            renderer,
                            &mut vertices,
                            &mut chrome_vertices,
                        );
                    }
                }

                // Re-upload atlas if new glyphs were rasterized
                if renderer.atlas_needs_reupload() {
                    renderer.reupload_atlas();
                }
                if renderer.chrome_atlas_needs_reupload() {
                    renderer.reupload_chrome_atlas();
                }

                match renderer.submit_frame(&vertices, &chrome_vertices) {
                    Ok(_) => {}
                    Err(wgpu::SurfaceError::Lost) => {
                        if let Some(ref mut r) = self.renderer {
                            let (w, h) = (r.width(), r.height());
                            r.resize(w, h);
                        }
                    }
                    Err(wgpu::SurfaceError::OutOfMemory) => {
                        event_loop.exit();
                    }
                    Err(e) => {
                        log::warn!("Render error: {e:?}");
                    }
                }

                if let Some(ref window) = self.window {
                    window.request_redraw();
                }
            }

            _ => {}
        }
    }
}

fn key_to_bytes(key: &Key, text: &Option<winit::keyboard::SmolStr>) -> Vec<u8> {
    match key {
        Key::Named(named) => match named {
            NamedKey::Enter => vec![b'\r'],
            NamedKey::Backspace => vec![0x7f],
            NamedKey::Tab => vec![b'\t'],
            NamedKey::Escape => vec![0x1b],
            NamedKey::Space => vec![b' '],
            NamedKey::ArrowUp => b"\x1b[A".to_vec(),
            NamedKey::ArrowDown => b"\x1b[B".to_vec(),
            NamedKey::ArrowRight => b"\x1b[C".to_vec(),
            NamedKey::ArrowLeft => b"\x1b[D".to_vec(),
            NamedKey::Home => b"\x1b[H".to_vec(),
            NamedKey::End => b"\x1b[F".to_vec(),
            NamedKey::Delete => b"\x1b[3~".to_vec(),
            NamedKey::PageUp => b"\x1b[5~".to_vec(),
            NamedKey::PageDown => b"\x1b[6~".to_vec(),
            NamedKey::Insert => b"\x1b[2~".to_vec(),
            _ => vec![],
        },
        Key::Character(_) => {
            if let Some(text) = text {
                text.as_bytes().to_vec()
            } else {
                vec![]
            }
        }
        _ => vec![],
    }
}

fn main() {
    env_logger::init();

    let config = Config::load();

    let event_loop = EventLoop::new().expect("Failed to create event loop");
    let mut app = App::new();
    app.font_size = config.font.size;
    app.theme = config.theme.resolve();
    log::info!("Using theme: {}", app.theme.name);
    app.config = config;

    event_loop.run_app(&mut app).expect("Event loop failed");
}
