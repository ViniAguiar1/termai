mod ai;
mod colors;
mod config;
mod pane;
mod tab;

use std::sync::Arc;
use std::time::{Duration, Instant};

use config::Config;

use winit::application::ApplicationHandler;
use winit::event::{ElementState, MouseButton, MouseScrollDelta, WindowEvent};
use winit::event_loop::{ActiveEventLoop, EventLoop};
use winit::keyboard::{Key, ModifiersState, NamedKey};
use winit::window::{Window, WindowId};

use termai_core::CursorStyle;
use termai_renderer::{RenderCell, Renderer, Vertex};

use pane::{PaneRect, SplitDir};
use tab::TabBar;

const MIN_FONT_SIZE: f32 = 10.0;
const MAX_FONT_SIZE: f32 = 60.0;
const ZOOM_STEP: f32 = 2.0;
const CURSOR_BLINK_MS: u128 = 530;
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
    /// `Some(t)` while a request is in flight (sent at `t`). Auto-expires after a
    /// timeout so a dropped IPC response doesn't permanently block autocomplete.
    pending_autocomplete: Option<Instant>,
    /// True after the user typed and we haven't fired an autocomplete request yet
    /// for that edit. Reset to true on every keystroke so each pause re-arms one fire.
    autocomplete_armed: bool,
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
            pending_autocomplete: None,
            autocomplete_armed: false,
        }
    }

    fn tab_bar_pixel_height(&self) -> f32 {
        if self.tab_bar.tab_count() <= 1 {
            return 0.0;
        }
        if let Some(ref renderer) = self.renderer {
            let (_, ch) = renderer.cell_size();
            ch + 4.0 // one row + padding
        } else {
            0.0
        }
    }

    fn search_bar_pixel_height(&self) -> f32 {
        if self.search.is_none() {
            return 0.0;
        }
        if let Some(ref renderer) = self.renderer {
            let (_, ch) = renderer.cell_size();
            ch + 4.0
        } else {
            0.0
        }
    }

    fn content_area(&self) -> (f32, f32, f32, f32) {
        if let Some(ref renderer) = self.renderer {
            let w = renderer.width() as f32;
            let h = renderer.height() as f32;
            let tab_h = self.tab_bar_pixel_height();
            let search_h = self.search_bar_pixel_height();
            (0.0, tab_h, w, h - tab_h - search_h)
        } else {
            (0.0, 0.0, 0.0, 0.0)
        }
    }

    fn pixel_to_cell_in_pane(&self, px: f64, py: f64, rect: &PaneRect) -> (usize, usize) {
        if let Some(ref renderer) = self.renderer {
            let (cw, ch) = renderer.cell_size();
            // winit 0.30 reports CursorMoved.position in physical pixels, so no
            // scale_factor multiplication is needed here. Renderer rects and
            // cell_size are also in physical pixels.
            let x = px as f32 - rect.x;
            let y = py as f32 - rect.y;
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
        let sx = px as f32;
        let sy = py as f32;
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
        let cursor_on = is_focused
            && pane.terminal.cursor_visible
            && pane.terminal.scroll_offset == 0
            && (self.cursor_blink_start.elapsed().as_millis() / CURSOR_BLINK_MS) % 2 == 0;

        let sel = if is_focused {
            self.selection.as_ref().map(|s| s.normalized())
        } else {
            None
        };

        // Build set of search-highlighted cells for this pane
        let search_highlight: Option<&SearchState> = if is_focused {
            self.search.as_ref().filter(|s| !s.query.is_empty())
        } else {
            None
        };

        // Detect URLs once per frame so we can underline them like hyperlinks.
        let urls = pane.terminal.detect_urls();

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

                        if let Some((sc, sr, ec, er)) = sel {
                            let in_sel = if sr == er {
                                row_idx == sr && col_idx >= sc && col_idx < ec
                            } else if row_idx == sr {
                                col_idx >= sc
                            } else if row_idx == er {
                                col_idx < ec
                            } else {
                                row_idx > sr && row_idx < er
                            };
                            if in_sel {
                                std::mem::swap(&mut fg, &mut bg);
                            }
                        }

                        // Search highlighting
                        if let Some(ref search) = search_highlight {
                            let qlen = search.query.len();
                            for (mi, &(abs_row, abs_col)) in search.matches.iter().enumerate() {
                                if let Some(vis_row) = pane.terminal.abs_row_to_visible(abs_row) {
                                    if vis_row == row_idx && col_idx >= abs_col && col_idx < abs_col + qlen {
                                        if mi == search.current {
                                            // Current match: bright orange
                                            fg = [0.0, 0.0, 0.0, 1.0];
                                            bg = [1.0, 0.6, 0.0, 1.0];
                                        } else {
                                            // Other matches: yellow
                                            fg = [0.0, 0.0, 0.0, 1.0];
                                            bg = [0.9, 0.9, 0.2, 1.0];
                                        }
                                        break;
                                    }
                                }
                            }
                        }

                        // URL detection: underline always, recolor on Cmd-hover.
                        let is_url = urls.iter().any(|&(ur, us, ue)| {
                            row_idx == ur && col_idx >= us && col_idx < ue
                        });
                        let mut underline = is_url;

                        if let Some((ur, us, ue)) = self.hovered_url {
                            if row_idx == ur && col_idx >= us && col_idx < ue {
                                fg = [0.4, 0.6, 1.0, 1.0]; // link blue
                            }
                        }

                        if cursor_on
                            && row_idx == pane.terminal.cursor_y
                            && col_idx == pane.terminal.cursor_x
                        {
                            match pane.terminal.cursor_style {
                                CursorStyle::Block => {
                                    fg = self.theme.bg;
                                    bg = self.theme.cursor;
                                }
                                CursorStyle::Underline | CursorStyle::Bar => {
                                    bg = self.theme.cursor_bar();
                                }
                            }
                            // Don't underline the cell that's already showing the cursor.
                            underline = false;
                        }

                        RenderCell { ch: cell.c, fg, bg, underline }
                    })
                    .collect()
            })
            .collect()
    }

    fn build_tab_bar_cells(&self) -> Vec<Vec<RenderCell>> {
        if self.tab_bar.tab_count() <= 1 {
            return vec![];
        }

        let renderer = match self.renderer {
            Some(ref r) => r,
            None => return vec![],
        };

        let (cols, _) = renderer.grid_size();
        let tab_bg = self.theme.tab_bg();
        let tab_fg = self.theme.tab_fg();
        let active_bg = self.theme.tab_active_bg();
        let active_fg = self.theme.tab_active_fg();
        let sep_fg = self.theme.tab_separator();

        let mut row = vec![
            RenderCell {
                ch: ' ',
                fg: tab_fg,
                bg: tab_bg,
                underline: false,
            };
            cols as usize
        ];

        let mut col = 0usize;
        for (i, tab) in self.tab_bar.tabs.iter().enumerate() {
            let label = format!(" {} {} ", i + 1, tab.title);
            let is_active = i == self.tab_bar.active;

            for ch in label.chars() {
                if col >= cols as usize {
                    break;
                }
                row[col] = RenderCell {
                    ch,
                    fg: if is_active { active_fg } else { tab_fg },
                    bg: if is_active { active_bg } else { tab_bg },
                    underline: false,
                };
                col += 1;
            }

            // Separator
            if col < cols as usize {
                row[col] = RenderCell {
                    ch: '|',
                    fg: sep_fg,
                    bg: tab_bg,
                    underline: false,
                };
                col += 1;
            }
        }

        vec![row]
    }

    /// Check if a click is in the tab bar and switch tabs if so. Returns true if handled.
    fn handle_tab_bar_click(&mut self, px: f64, py: f64) -> bool {
        let tab_h = self.tab_bar_pixel_height();
        if tab_h == 0.0 {
            return false;
        }

        // Mouse coords from winit 0.30 are already in physical pixels.
        let sy = py as f32;
        if sy >= tab_h {
            return false; // Click is below tab bar
        }

        // Determine which tab was clicked based on x position
        let renderer = match self.renderer {
            Some(ref r) => r,
            None => return false,
        };
        let (cw, _) = renderer.cell_size();
        let sx = px as f32;
        let click_col = (sx / cw).floor() as usize;

        let mut col = 0usize;
        for (i, tab) in self.tab_bar.tabs.iter().enumerate() {
            let label_len = format!(" {} {} ", i + 1, tab.title).len() + 1; // +1 for separator
            if click_col >= col && click_col < col + label_len {
                self.tab_bar.switch_to(i);
                return true;
            }
            col += label_len;
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

    fn build_search_bar_cells(&self) -> Vec<Vec<RenderCell>> {
        let search = match self.search {
            Some(ref s) => s,
            None => return vec![],
        };
        let renderer = match self.renderer {
            Some(ref r) => r,
            None => return vec![],
        };

        let (cols, _) = renderer.grid_size();
        let bg = self.theme.search_bg();
        let fg = self.theme.search_fg();
        let mut row = vec![RenderCell { ch: ' ', fg, bg, underline: false }; cols as usize];

        // "Find: <query>  N/M"
        let count_str = if search.matches.is_empty() {
            if search.query.is_empty() { String::new() } else { "0/0".to_string() }
        } else {
            format!("{}/{}", search.current + 1, search.matches.len())
        };

        let label = format!(" Find: {}  {}", search.query, count_str);

        for (i, ch) in label.chars().enumerate() {
            if i >= cols as usize {
                break;
            }
            row[i].ch = ch;
        }

        // Cursor position (blinking underscore after query)
        let cursor_pos = 7 + search.query.len(); // " Find: " is 7 chars
        if cursor_pos < cols as usize {
            row[cursor_pos].bg = self.theme.cursor_bar();
        }

        vec![row]
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

    fn build_ai_overlay_cells(&self) -> Vec<Vec<RenderCell>> {
        let suggestion = match self.ai_overlay {
            Some(ref s) => s,
            None => return vec![],
        };
        let renderer = match self.renderer {
            Some(ref r) => r,
            None => return vec![],
        };

        let (cols, _) = renderer.grid_size();
        let cols = cols as usize;
        let bg = self.theme.ai_overlay_bg();
        let title_fg = [1.0, 0.8, 0.2, 1.0]; // Gold — intentionally fixed for visibility
        let desc_fg = self.theme.fg;
        let action_fg = [0.4, 0.9, 0.4, 1.0]; // Green — intentionally fixed
        let hint_fg = self.theme.tab_fg();

        let mut rows: Vec<Vec<RenderCell>> = Vec::new();

        let make_row = |text: &str, fg: [f32; 4], bg: [f32; 4], cols: usize| -> Vec<RenderCell> {
            let mut row = vec![RenderCell { ch: ' ', fg, bg, underline: false }; cols];
            for (i, ch) in text.chars().enumerate() {
                if i >= cols {
                    break;
                }
                row[i] = RenderCell { ch, fg, bg, underline: false };
            }
            row
        };

        // Separator line
        rows.push(make_row(&"─".repeat(cols.min(80)), hint_fg, bg, cols));

        // Title
        let title_text = format!(" {} ", suggestion.title);
        rows.push(make_row(&title_text, title_fg, bg, cols));

        // Description
        if !suggestion.description.is_empty() {
            let desc_text = format!(" {}", suggestion.description);
            rows.push(make_row(&desc_text, desc_fg, bg, cols));
        }

        // Blank line
        rows.push(make_row("", desc_fg, bg, cols));

        // Actions
        for (i, action) in suggestion.actions.iter().enumerate() {
            let risk_indicator = match action.risk.as_str() {
                "high" => " [!]",
                "medium" => " [~]",
                _ => "",
            };
            let action_text = format!(" [{}] {}{}", i + 1, action.label, risk_indicator);
            rows.push(make_row(&action_text, action_fg, bg, cols));
        }

        // Hint
        rows.push(make_row("", desc_fg, bg, cols));
        rows.push(make_row(" Press 1-9 to execute, Esc to dismiss", hint_fg, bg, cols));

        rows
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

/// Strip a shell prompt prefix from a line, returning what the user typed.
/// Heuristic: cut after the last "$ ", "% ", "> " or "# " (common prompt endings).
fn strip_prompt(line: &str) -> &str {
    for marker in ["$ ", "% ", "# ", "> "] {
        if let Some(idx) = line.rfind(marker) {
            return line[idx + marker.len()..].trim_start();
        }
    }
    line.trim_start()
}

/// Pull the last `n` non-empty lines above `cursor_y` as plain strings — used as
/// LLM context for autocomplete. Trailing whitespace is trimmed per line.
fn recent_history(grid: &[Vec<termai_core::Cell>], cursor_y: usize, n: usize) -> String {
    let mut out: Vec<String> = Vec::new();
    let upper = cursor_y.min(grid.len());
    for row in grid[..upper].iter().rev() {
        let line: String = row.iter().map(|c| c.c).collect();
        let line = line.trim_end().to_string();
        if !line.is_empty() {
            out.push(line);
            if out.len() >= n {
                break;
            }
        }
    }
    out.reverse();
    out.join("\n")
}

impl ApplicationHandler for App {
    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        if self.window.is_some() {
            return;
        }

        let attrs = Window::default_attributes()
            .with_title(&self.config.window.title)
            .with_inner_size(winit::dpi::LogicalSize::new(
                self.config.window.width,
                self.config.window.height,
            ));

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
                        self.autocomplete_armed = true;
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
                                self.pending_autocomplete = None;
                            }
                            ai::AiMessage::NoCompletion => {
                                self.pending_autocomplete = None;
                            }
                        }
                    }
                }

                // Autocomplete: fire once, 300ms after the user stops typing.
                // `autocomplete_armed` is set on every keystroke and cleared after firing,
                // so we don't keep hammering the LLM after a NoCompletion.
                // Pending requests time out after 5s so a dropped response can't
                // permanently block future requests.
                let pending = matches!(
                    self.pending_autocomplete,
                    Some(t) if t.elapsed() < Duration::from_secs(5)
                );
                if !pending {
                    self.pending_autocomplete = None;
                }
                if self.autocomplete_armed
                    && self.ghost_text.is_none()
                    && !pending
                    && self.ghost_text_debounce.elapsed() > Duration::from_millis(300)
                {
                    if let Some(pane) = self.find_focused_pane_ref() {
                        let cx = pane.terminal.cursor_x;
                        let cy = pane.terminal.cursor_y;
                        if cx >= 1 && cy < pane.terminal.grid.len() {
                            let row = &pane.terminal.grid[cy];
                            let line: String = row.iter().take(cx).map(|c| c.c).collect();
                            let typed = strip_prompt(&line);
                            if !typed.is_empty() {
                                let typed = typed.to_string();
                                let cwd = std::env::current_dir()
                                    .map(|p| p.display().to_string())
                                    .unwrap_or_else(|_| ".".to_string());
                                let history = recent_history(&pane.terminal.grid, cy, 10);
                                if let Some(ref ai_client) = self.ai_client {
                                    ai_client.autocomplete(&typed, &cwd, &history);
                                    self.pending_autocomplete = Some(Instant::now());
                                    self.autocomplete_armed = false;
                                }
                            } else {
                                self.autocomplete_armed = false;
                            }
                        } else {
                            self.autocomplete_armed = false;
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
                let tab_bar_cells = self.build_tab_bar_cells();
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

                let search_bar_cells = self.build_search_bar_cells();
                let overlay_cells = self.build_ai_overlay_cells();

                // Ghost text (autocomplete suggestion) data
                let ghost_data = if let Some(ref ghost) = self.ghost_text {
                    if let Some(pane) = find_pane_ref(&tab.root, focused_id) {
                        if pane.terminal.scroll_offset == 0 {
                            let cursor_x = pane.terminal.cursor_x;
                            let cursor_y = pane.terminal.cursor_y;
                            let ghost_cells: Vec<Vec<RenderCell>> = vec![
                                ghost.chars().map(|c| RenderCell {
                                    ch: c,
                                    fg: [0.5, 0.5, 0.5, 0.8],
                                    bg: theme_bg,
                                    underline: false,
                                }).collect()
                            ];
                            Some((cursor_x, cursor_y, ghost_cells))
                        } else { None }
                    } else { None }
                } else { None };

                // Now borrow renderer mutably for vertex building
                let renderer = self.renderer.as_mut().unwrap();
                let mut vertices: Vec<Vertex> = Vec::new();

                // Tab bar
                if !tab_bar_cells.is_empty() {
                    renderer.build_vertices(&tab_bar_cells, 0.0, 0.0, &mut vertices);
                }

                // Pane content
                for rect in &rects {
                    renderer.build_rect(rect.x, rect.y, rect.w, rect.h, theme_bg, &mut vertices);
                }
                for (rect, cells) in &pane_cells {
                    renderer.build_vertices(cells, rect.x, rect.y, &mut vertices);
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

                // Ghost text (autocomplete)
                if let Some((cursor_x, cursor_y, ref ghost_cells)) = ghost_data {
                    if let Some(rect) = rects.iter().find(|r| r.id == focused_id) {
                        let (cw_px, ch_px) = renderer.cell_size();
                        let ghost_x = rect.x + (cursor_x as f32) * cw_px;
                        let ghost_y = rect.y + (cursor_y as f32) * ch_px;
                        renderer.build_vertices(ghost_cells, ghost_x, ghost_y, &mut vertices);
                    }
                }

                // Search bar
                if !search_bar_cells.is_empty() {
                    let search_y = cy + ch;
                    renderer.build_vertices(&search_bar_cells, 0.0, search_y, &mut vertices);
                }

                // AI suggestion overlay
                if !overlay_cells.is_empty() {
                    if let Some(rect) = rects.iter().find(|r| r.id == focused_id) {
                        let (_, cell_h) = renderer.cell_size();
                        let overlay_h = overlay_cells.len() as f32 * cell_h;
                        let overlay_y = (rect.y + rect.h - overlay_h).max(rect.y);
                        renderer.build_vertices(&overlay_cells, rect.x, overlay_y, &mut vertices);
                    }
                }

                // Re-upload atlas if new glyphs were rasterized
                if renderer.atlas_needs_reupload() {
                    renderer.reupload_atlas();
                }

                match renderer.submit_frame(&vertices) {
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
