use vte::{Params, Parser, Perform};

/// Terminal cell holding a character and its attributes.
#[derive(Clone, Debug)]
pub struct Cell {
    pub c: char,
    pub fg: Color,
    pub bg: Color,
    pub bold: bool,
    pub underline: bool,
    pub inverse: bool,
}

impl Default for Cell {
    fn default() -> Self {
        Self {
            c: ' ',
            fg: Color::Default,
            bg: Color::Default,
            bold: false,
            underline: false,
            inverse: false,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Color {
    Default,
    Indexed(u8),
    Rgb(u8, u8, u8),
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum CursorStyle {
    Block,
    Underline,
    Bar,
}

/// Current text attributes applied to new characters.
#[derive(Clone, Debug)]
struct Attrs {
    fg: Color,
    bg: Color,
    bold: bool,
    underline: bool,
    inverse: bool,
}

impl Default for Attrs {
    fn default() -> Self {
        Self {
            fg: Color::Default,
            bg: Color::Default,
            bold: false,
            underline: false,
            inverse: false,
        }
    }
}

/// Terminal state machine with scrollback buffer.
pub struct Terminal {
    pub cols: usize,
    pub rows: usize,
    pub grid: Vec<Vec<Cell>>,
    pub cursor_x: usize,
    pub cursor_y: usize,
    pub cursor_visible: bool,
    pub cursor_style: CursorStyle,

    /// Scrollback buffer: lines that scrolled off the top.
    pub scrollback: Vec<Vec<Cell>>,
    pub max_scrollback: usize,
    /// How many lines the viewport is scrolled up (0 = at bottom).
    pub scroll_offset: usize,

    /// Alternate screen buffer (used by vim, htop, etc.)
    alt_grid: Vec<Vec<Cell>>,
    alt_cursor_x: usize,
    alt_cursor_y: usize,
    pub using_alt_screen: bool,

    /// Scroll region (top..=bottom inclusive row indices).
    scroll_top: usize,
    scroll_bottom: usize,

    /// Saved cursor position (for ESC 7 / ESC 8).
    saved_cursor_x: usize,
    saved_cursor_y: usize,
    saved_attrs: Attrs,

    attrs: Attrs,

    /// Working directory reported by the shell via OSC 7. None until first OSC 7 arrives.
    pub cwd: Option<std::path::PathBuf>,
}

impl Terminal {
    pub fn new(cols: usize, rows: usize) -> Self {
        let grid = vec![vec![Cell::default(); cols]; rows];
        Self {
            cols,
            rows,
            grid,
            cursor_x: 0,
            cursor_y: 0,
            cursor_visible: true,
            cursor_style: CursorStyle::Block,
            scrollback: Vec::new(),
            max_scrollback: 10_000,
            scroll_offset: 0,
            alt_grid: vec![vec![Cell::default(); cols]; rows],
            alt_cursor_x: 0,
            alt_cursor_y: 0,
            using_alt_screen: false,
            scroll_top: 0,
            scroll_bottom: rows.saturating_sub(1),
            saved_cursor_x: 0,
            saved_cursor_y: 0,
            saved_attrs: Attrs::default(),
            attrs: Attrs::default(),
            cwd: None,
        }
    }

    /// Feed raw bytes from PTY into the terminal state machine.
    pub fn feed(&mut self, bytes: &[u8]) {
        let mut parser = Parser::new();
        for &byte in bytes {
            parser.advance(self, byte);
        }
        // New output arrived — snap to bottom
        self.scroll_offset = 0;
    }

    /// Get the visible grid (accounting for scroll offset into scrollback).
    pub fn visible_grid(&self) -> Vec<&Vec<Cell>> {
        if self.scroll_offset == 0 || self.using_alt_screen {
            return self.grid.iter().collect();
        }

        let sb_len = self.scrollback.len();
        let offset = self.scroll_offset.min(sb_len);
        let sb_start = sb_len - offset;

        let mut visible = Vec::with_capacity(self.rows);

        // Lines from scrollback
        for i in sb_start..sb_len {
            if visible.len() >= self.rows {
                break;
            }
            visible.push(&self.scrollback[i]);
        }

        // Lines from current grid
        for row in &self.grid {
            if visible.len() >= self.rows {
                break;
            }
            visible.push(row);
        }

        visible
    }

    /// Scroll the viewport up by `lines` lines.
    pub fn scroll_viewport_up(&mut self, lines: usize) {
        let max = self.scrollback.len();
        self.scroll_offset = (self.scroll_offset + lines).min(max);
    }

    /// Scroll the viewport down by `lines` lines.
    pub fn scroll_viewport_down(&mut self, lines: usize) {
        self.scroll_offset = self.scroll_offset.saturating_sub(lines);
    }

    /// Extract text from a rectangular region (for copy).
    pub fn get_text(&self, start_col: usize, start_row: usize, end_col: usize, end_row: usize) -> String {
        let visible = self.visible_grid();
        let mut text = String::new();

        for row_idx in start_row..=end_row.min(visible.len() - 1) {
            let row = visible[row_idx];
            let col_start = if row_idx == start_row { start_col } else { 0 };
            let col_end = if row_idx == end_row {
                end_col.min(self.cols)
            } else {
                self.cols
            };

            for col in col_start..col_end {
                if col < row.len() {
                    text.push(row[col].c);
                }
            }

            // Trim trailing spaces and add newline between rows
            if row_idx < end_row {
                let trimmed = text.trim_end_matches(' ');
                text.truncate(trimmed.len());
                text.push('\n');
            }
        }

        let trimmed = text.trim_end_matches(' ');
        trimmed.to_string()
    }

    /// Detect URLs in the visible grid.
    /// Returns Vec of (row_index, start_col, end_col) where col indices are character positions.
    pub fn detect_urls(&self) -> Vec<(usize, usize, usize)> {
        let visible = self.visible_grid();
        let mut urls = Vec::new();
        let prefixes = ["https://", "http://", "file://"];

        for (row_idx, row) in visible.iter().enumerate() {
            let chars: Vec<char> = row.iter().map(|c| c.c).collect();
            for prefix in &prefixes {
                let mut search_col = 0usize;
                while search_col < chars.len() {
                    // Find prefix starting at search_col
                    let remaining: String = chars[search_col..].iter().collect();
                    let byte_pos = match remaining.find(prefix) {
                        Some(p) => p,
                        None => break,
                    };
                    // Convert byte position to char position
                    let char_offset = remaining[..byte_pos].chars().count();
                    let start_col = search_col + char_offset;

                    // Find end of URL by scanning chars
                    let mut end_col = start_col;
                    let mut paren_depth: i32 = 0;
                    for &ch in &chars[start_col..] {
                        match ch {
                            ' ' | '\t' | '"' | '\'' | '<' | '>' => break,
                            '(' => { paren_depth += 1; end_col += 1; }
                            ')' => {
                                if paren_depth > 0 {
                                    paren_depth -= 1;
                                    end_col += 1;
                                } else {
                                    break;
                                }
                            }
                            _ => { end_col += 1; }
                        }
                    }
                    if end_col > start_col + prefix.len() {
                        urls.push((row_idx, start_col, end_col));
                    }
                    search_col = end_col.max(start_col + 1);
                }
            }
        }
        urls
    }

    /// Search for a query string in scrollback + grid.
    /// Returns matches as (absolute_row, col) where absolute_row 0 is the first scrollback line.
    pub fn search(&self, query: &str) -> Vec<(usize, usize)> {
        if query.is_empty() {
            return vec![];
        }
        let query_lower = query.to_lowercase();
        let mut matches = Vec::new();

        // Search scrollback
        for (row_idx, row) in self.scrollback.iter().enumerate() {
            let line: String = row.iter().map(|c| c.c).collect();
            let line_lower = line.to_lowercase();
            let mut start = 0;
            while let Some(pos) = line_lower[start..].find(&query_lower) {
                matches.push((row_idx, start + pos));
                start += pos + 1;
            }
        }

        // Search grid
        let sb_len = self.scrollback.len();
        for (row_idx, row) in self.grid.iter().enumerate() {
            let line: String = row.iter().map(|c| c.c).collect();
            let line_lower = line.to_lowercase();
            let mut start = 0;
            while let Some(pos) = line_lower[start..].find(&query_lower) {
                matches.push((sb_len + row_idx, start + pos));
                start += pos + 1;
            }
        }

        matches
    }

    /// Convert an absolute row index (scrollback + grid) to a visible row index,
    /// given the current scroll_offset. Returns None if not visible.
    pub fn abs_row_to_visible(&self, abs_row: usize) -> Option<usize> {
        let sb_len = self.scrollback.len();
        let viewport_start_abs = if self.scroll_offset == 0 {
            sb_len
        } else {
            sb_len.saturating_sub(self.scroll_offset)
        };
        let viewport_end_abs = viewport_start_abs + self.rows;

        if abs_row >= viewport_start_abs && abs_row < viewport_end_abs {
            Some(abs_row - viewport_start_abs)
        } else {
            None
        }
    }

    fn scroll_up_region(&mut self) {
        let removed = self.grid.remove(self.scroll_top);

        // Only save to scrollback if scrolling the full screen from top
        if self.scroll_top == 0 && !self.using_alt_screen {
            self.scrollback.push(removed);
            if self.scrollback.len() > self.max_scrollback {
                self.scrollback.remove(0);
            }
        }

        self.grid
            .insert(self.scroll_bottom, vec![Cell::default(); self.cols]);
    }

    fn scroll_down_region(&mut self) {
        self.grid.remove(self.scroll_bottom);
        self.grid
            .insert(self.scroll_top, vec![Cell::default(); self.cols]);
    }

    fn newline(&mut self) {
        self.cursor_x = 0;
        if self.cursor_y == self.scroll_bottom {
            self.scroll_up_region();
        } else if self.cursor_y + 1 < self.rows {
            self.cursor_y += 1;
        }
    }

    fn linefeed(&mut self) {
        if self.cursor_y == self.scroll_bottom {
            self.scroll_up_region();
        } else if self.cursor_y + 1 < self.rows {
            self.cursor_y += 1;
        }
    }

    fn reverse_index(&mut self) {
        if self.cursor_y == self.scroll_top {
            self.scroll_down_region();
        } else if self.cursor_y > 0 {
            self.cursor_y -= 1;
        }
    }

    fn enter_alt_screen(&mut self) {
        if self.using_alt_screen {
            return;
        }
        self.alt_grid = self.grid.clone();
        self.alt_cursor_x = self.cursor_x;
        self.alt_cursor_y = self.cursor_y;
        self.grid = vec![vec![Cell::default(); self.cols]; self.rows];
        self.cursor_x = 0;
        self.cursor_y = 0;
        self.using_alt_screen = true;
    }

    fn exit_alt_screen(&mut self) {
        if !self.using_alt_screen {
            return;
        }
        self.grid = std::mem::take(&mut self.alt_grid);
        self.cursor_x = self.alt_cursor_x;
        self.cursor_y = self.alt_cursor_y;
        self.using_alt_screen = false;
    }

    fn save_cursor(&mut self) {
        self.saved_cursor_x = self.cursor_x;
        self.saved_cursor_y = self.cursor_y;
        self.saved_attrs = self.attrs.clone();
    }

    fn restore_cursor(&mut self) {
        self.cursor_x = self.saved_cursor_x.min(self.cols.saturating_sub(1));
        self.cursor_y = self.saved_cursor_y.min(self.rows.saturating_sub(1));
        self.attrs = self.saved_attrs.clone();
    }

    fn apply_sgr(&mut self, params: &Params) {
        let mut iter = params.iter();

        let first = match iter.next() {
            Some(p) => p,
            None => {
                self.attrs = Attrs::default();
                return;
            }
        };

        let mut subparams = first.iter().copied();
        loop {
            let code = match subparams.next() {
                Some(c) => c,
                None => match iter.next() {
                    Some(p) => match p.first() {
                        Some(&c) => c,
                        None => break,
                    },
                    None => break,
                },
            };

            match code {
                0 => self.attrs = Attrs::default(),
                1 => self.attrs.bold = true,
                4 => self.attrs.underline = true,
                7 => self.attrs.inverse = true,
                22 => self.attrs.bold = false,
                24 => self.attrs.underline = false,
                27 => self.attrs.inverse = false,
                30..=37 => self.attrs.fg = Color::Indexed((code - 30) as u8),
                90..=97 => self.attrs.fg = Color::Indexed((code - 90 + 8) as u8),
                38 => {
                    self.parse_extended_color(&mut iter, &mut subparams, true);
                }
                39 => self.attrs.fg = Color::Default,
                40..=47 => self.attrs.bg = Color::Indexed((code - 40) as u8),
                100..=107 => self.attrs.bg = Color::Indexed((code - 100 + 8) as u8),
                48 => {
                    self.parse_extended_color(&mut iter, &mut subparams, false);
                }
                49 => self.attrs.bg = Color::Default,
                _ => {}
            }
        }
    }

    fn parse_extended_color<'a>(
        &mut self,
        iter: &mut impl Iterator<Item = &'a [u16]>,
        _subparams: &mut impl Iterator<Item = u16>,
        is_fg: bool,
    ) {
        let mode = iter.next().and_then(|p| p.first().copied());
        match mode {
            Some(5) => {
                if let Some(idx) = iter.next().and_then(|p| p.first().copied()) {
                    let color = Color::Indexed(idx as u8);
                    if is_fg {
                        self.attrs.fg = color;
                    } else {
                        self.attrs.bg = color;
                    }
                }
            }
            Some(2) => {
                let r = iter.next().and_then(|p| p.first().copied()).unwrap_or(0) as u8;
                let g = iter.next().and_then(|p| p.first().copied()).unwrap_or(0) as u8;
                let b = iter.next().and_then(|p| p.first().copied()).unwrap_or(0) as u8;
                let color = Color::Rgb(r, g, b);
                if is_fg {
                    self.attrs.fg = color;
                } else {
                    self.attrs.bg = color;
                }
            }
            _ => {}
        }
    }
}

impl Perform for Terminal {
    fn print(&mut self, c: char) {
        if self.cursor_x >= self.cols {
            self.newline();
        }
        let cell = &mut self.grid[self.cursor_y][self.cursor_x];
        cell.c = c;
        cell.fg = self.attrs.fg;
        cell.bg = self.attrs.bg;
        cell.bold = self.attrs.bold;
        cell.underline = self.attrs.underline;
        cell.inverse = self.attrs.inverse;
        self.cursor_x += 1;
    }

    fn execute(&mut self, byte: u8) {
        match byte {
            b'\n' | 0x0B | 0x0C => self.linefeed(),
            b'\r' => self.cursor_x = 0,
            b'\t' => {
                let next_tab = (self.cursor_x / 8 + 1) * 8;
                self.cursor_x = next_tab.min(self.cols - 1);
            }
            b'\x08' => {
                if self.cursor_x > 0 {
                    self.cursor_x -= 1;
                }
            }
            b'\x07' => {} // BEL — ignore for now
            _ => {}
        }
    }

    fn hook(&mut self, _params: &Params, _intermediates: &[u8], _ignore: bool, _action: char) {}
    fn put(&mut self, _byte: u8) {}
    fn unhook(&mut self) {}
    fn osc_dispatch(&mut self, params: &[&[u8]], _bell_terminated: bool) {
        let Some(first) = params.first() else { return };
        let Ok(code) = std::str::from_utf8(first) else { return };
        if code != "7" { return; }
        let Some(uri_bytes) = params.get(1) else { return };
        let Ok(uri) = std::str::from_utf8(uri_bytes) else { return };
        // Format: file://hostname/path — strip the scheme and the hostname.
        let Some(after_scheme) = uri.strip_prefix("file://") else { return };
        // The first '/' after the hostname starts the actual path.
        if let Some(slash_idx) = after_scheme.find('/') {
            let path = &after_scheme[slash_idx..];
            // Percent-decode common encodings (space, etc.) — basic handling.
            let decoded = percent_decode(path);
            self.cwd = Some(std::path::PathBuf::from(decoded));
        }
    }

    fn csi_dispatch(
        &mut self,
        params: &Params,
        intermediates: &[u8],
        _ignore: bool,
        action: char,
    ) {
        // Handle private mode sequences (CSI ? Ps h/l)
        if intermediates.first() == Some(&b'?') {
            let mode = params
                .iter()
                .next()
                .and_then(|p| p.first().copied())
                .unwrap_or(0);

            match action {
                'h' => match mode {
                    25 => self.cursor_visible = true,
                    1049 => self.enter_alt_screen(),
                    1047 => self.enter_alt_screen(),
                    _ => {}
                },
                'l' => match mode {
                    25 => self.cursor_visible = false,
                    1049 => self.exit_alt_screen(),
                    1047 => self.exit_alt_screen(),
                    _ => {}
                },
                _ => {}
            }
            return;
        }

        // SGR
        if action == 'm' {
            self.apply_sgr(params);
            return;
        }

        let mut params_iter = params.iter();
        let first = params_iter
            .next()
            .and_then(|p| p.first().copied())
            .unwrap_or(0);

        match action {
            // Cursor movement
            'A' => {
                let n = (first as usize).max(1);
                self.cursor_y = self.cursor_y.saturating_sub(n);
            }
            'B' => {
                let n = (first as usize).max(1);
                self.cursor_y = (self.cursor_y + n).min(self.rows - 1);
            }
            'C' => {
                let n = (first as usize).max(1);
                self.cursor_x = (self.cursor_x + n).min(self.cols - 1);
            }
            'D' => {
                let n = (first as usize).max(1);
                self.cursor_x = self.cursor_x.saturating_sub(n);
            }
            // Cursor Next Line
            'E' => {
                let n = (first as usize).max(1);
                self.cursor_y = (self.cursor_y + n).min(self.rows - 1);
                self.cursor_x = 0;
            }
            // Cursor Previous Line
            'F' => {
                let n = (first as usize).max(1);
                self.cursor_y = self.cursor_y.saturating_sub(n);
                self.cursor_x = 0;
            }
            // Cursor Horizontal Absolute
            'G' => {
                let col = (first as usize).max(1).saturating_sub(1).min(self.cols - 1);
                self.cursor_x = col;
            }
            // Cursor Position
            'H' | 'f' => {
                let row = (first as usize).max(1).saturating_sub(1).min(self.rows - 1);
                let col = params_iter
                    .next()
                    .and_then(|p| p.first().copied())
                    .unwrap_or(1) as usize;
                let col = col.max(1).saturating_sub(1).min(self.cols - 1);
                self.cursor_y = row;
                self.cursor_x = col;
            }
            // Erase in Display
            'J' => match first {
                0 => {
                    for x in self.cursor_x..self.cols {
                        self.grid[self.cursor_y][x] = Cell::default();
                    }
                    for y in (self.cursor_y + 1)..self.rows {
                        self.grid[y] = vec![Cell::default(); self.cols];
                    }
                }
                1 => {
                    for y in 0..self.cursor_y {
                        self.grid[y] = vec![Cell::default(); self.cols];
                    }
                    for x in 0..=self.cursor_x.min(self.cols - 1) {
                        self.grid[self.cursor_y][x] = Cell::default();
                    }
                }
                2 | 3 => {
                    self.grid = vec![vec![Cell::default(); self.cols]; self.rows];
                }
                _ => {}
            },
            // Erase in Line
            'K' => match first {
                0 => {
                    for x in self.cursor_x..self.cols {
                        self.grid[self.cursor_y][x] = Cell::default();
                    }
                }
                1 => {
                    for x in 0..=self.cursor_x.min(self.cols - 1) {
                        self.grid[self.cursor_y][x] = Cell::default();
                    }
                }
                2 => {
                    self.grid[self.cursor_y] = vec![Cell::default(); self.cols];
                }
                _ => {}
            },
            // Insert Lines
            'L' => {
                let n = (first as usize).max(1);
                for _ in 0..n {
                    if self.cursor_y <= self.scroll_bottom {
                        self.grid.remove(self.scroll_bottom);
                        self.grid
                            .insert(self.cursor_y, vec![Cell::default(); self.cols]);
                    }
                }
            }
            // Delete Lines
            'M' => {
                let n = (first as usize).max(1);
                for _ in 0..n {
                    if self.cursor_y <= self.scroll_bottom {
                        self.grid.remove(self.cursor_y);
                        self.grid
                            .insert(self.scroll_bottom, vec![Cell::default(); self.cols]);
                    }
                }
            }
            // Delete Characters
            'P' => {
                let n = (first as usize).max(1).min(self.cols - self.cursor_x);
                let row = &mut self.grid[self.cursor_y];
                for _ in 0..n {
                    if self.cursor_x < row.len() {
                        row.remove(self.cursor_x);
                        row.push(Cell::default());
                    }
                }
            }
            // Scroll Up
            'S' => {
                let n = (first as usize).max(1);
                for _ in 0..n {
                    self.scroll_up_region();
                }
            }
            // Scroll Down
            'T' => {
                let n = (first as usize).max(1);
                for _ in 0..n {
                    self.scroll_down_region();
                }
            }
            // Erase Characters
            'X' => {
                let n = (first as usize).max(1);
                for i in 0..n {
                    let x = self.cursor_x + i;
                    if x < self.cols {
                        self.grid[self.cursor_y][x] = Cell::default();
                    }
                }
            }
            // Cursor Backward Tabulation
            'Z' => {
                let n = (first as usize).max(1);
                for _ in 0..n {
                    if self.cursor_x == 0 {
                        break;
                    }
                    self.cursor_x = ((self.cursor_x - 1) / 8) * 8;
                }
            }
            // Insert Characters
            '@' => {
                let n = (first as usize).max(1).min(self.cols - self.cursor_x);
                let row = &mut self.grid[self.cursor_y];
                for _ in 0..n {
                    row.insert(self.cursor_x, Cell::default());
                    row.pop();
                }
            }
            // Cursor position report
            'n' => {
                // We don't actually respond to the PTY here, but we handle the
                // sequence so it doesn't break anything.
            }
            // Set Scroll Region
            'r' => {
                let top = (first as usize).max(1).saturating_sub(1);
                let bottom = params_iter
                    .next()
                    .and_then(|p| p.first().copied())
                    .map(|b| (b as usize).max(1).saturating_sub(1))
                    .unwrap_or(self.rows - 1);
                self.scroll_top = top.min(self.rows - 1);
                self.scroll_bottom = bottom.min(self.rows - 1);
                if self.scroll_top >= self.scroll_bottom {
                    self.scroll_top = 0;
                    self.scroll_bottom = self.rows - 1;
                }
                self.cursor_x = 0;
                self.cursor_y = 0;
            }
            // Save cursor (ANSI.SYS)
            's' => self.save_cursor(),
            // Restore cursor (ANSI.SYS)
            'u' => self.restore_cursor(),
            // Cursor style (DECSCUSR)
            'q' if intermediates.first() == Some(&b' ') => {
                self.cursor_style = match first {
                    0 | 1 | 2 => CursorStyle::Block,
                    3 | 4 => CursorStyle::Underline,
                    5 | 6 => CursorStyle::Bar,
                    _ => CursorStyle::Block,
                };
            }
            _ => {}
        }
    }

    fn esc_dispatch(&mut self, intermediates: &[u8], _ignore: bool, byte: u8) {
        match byte {
            // Reverse Index (scroll down)
            b'M' => self.reverse_index(),
            // Save cursor
            b'7' => self.save_cursor(),
            // Restore cursor
            b'8' => self.restore_cursor(),
            // Reset
            b'c' => {
                *self = Terminal::new(self.cols, self.rows);
            }
            // Application/Normal keypad — handled at app level
            b'=' | b'>' => {}
            // Charset selection — ignore for now
            _ if intermediates.first() == Some(&b'(') => {}
            _ if intermediates.first() == Some(&b')') => {}
            _ => {}
        }
    }
}

/// Minimal percent-decoder for OSC 7 paths. Only handles %XX hex pairs.
/// Returns the input unchanged if no encoding is present.
fn percent_decode(s: &str) -> String {
    if !s.contains('%') {
        return s.to_string();
    }
    let bytes = s.as_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'%' && i + 2 < bytes.len() {
            if let (Some(h), Some(l)) = (
                from_hex(bytes[i + 1]),
                from_hex(bytes[i + 2]),
            ) {
                out.push((h << 4) | l);
                i += 3;
                continue;
            }
        }
        out.push(bytes[i]);
        i += 1;
    }
    String::from_utf8_lossy(&out).into_owned()
}

fn from_hex(b: u8) -> Option<u8> {
    match b {
        b'0'..=b'9' => Some(b - b'0'),
        b'a'..=b'f' => Some(b - b'a' + 10),
        b'A'..=b'F' => Some(b - b'A' + 10),
        _ => None,
    }
}

#[cfg(test)]
mod osc_tests {
    use super::*;

    #[test]
    fn osc_7_sets_cwd() {
        let mut term = Terminal::new(80, 24);
        let seq = b"\x1b]7;file://host/Users/vini/code\x07";
        term.feed(seq);
        assert_eq!(term.cwd.as_deref(), Some(std::path::Path::new("/Users/vini/code")));
    }

    #[test]
    fn osc_7_percent_decoding() {
        let mut term = Terminal::new(80, 24);
        let seq = b"\x1b]7;file://host/path%20with%20spaces\x07";
        term.feed(seq);
        assert_eq!(
            term.cwd.as_deref(),
            Some(std::path::Path::new("/path with spaces"))
        );
    }

    #[test]
    fn osc_7_ignored_without_file_scheme() {
        let mut term = Terminal::new(80, 24);
        let seq = b"\x1b]7;not-a-uri\x07";
        term.feed(seq);
        assert_eq!(term.cwd, None);
    }

    #[test]
    fn other_osc_codes_do_not_affect_cwd() {
        let mut term = Terminal::new(80, 24);
        // OSC 0 = window title — must NOT set cwd
        let seq = b"\x1b]0;some title\x07";
        term.feed(seq);
        assert_eq!(term.cwd, None);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_print_and_newline() {
        let mut term = Terminal::new(80, 24);
        term.feed(b"hello\r\nworld");
        assert_eq!(term.grid[0][0].c, 'h');
        assert_eq!(term.grid[0][4].c, 'o');
        assert_eq!(term.grid[1][0].c, 'w');
    }

    #[test]
    fn test_cursor_movement() {
        let mut term = Terminal::new(80, 24);
        term.feed(b"\x1b[5;10H*");
        assert_eq!(term.grid[4][9].c, '*');
    }

    #[test]
    fn test_sgr_foreground_color() {
        let mut term = Terminal::new(80, 24);
        term.feed(b"\x1b[31mX");
        assert_eq!(term.grid[0][0].c, 'X');
        assert_eq!(term.grid[0][0].fg, Color::Indexed(1));
    }

    #[test]
    fn test_sgr_reset() {
        let mut term = Terminal::new(80, 24);
        term.feed(b"\x1b[31mR\x1b[0mN");
        assert_eq!(term.grid[0][0].fg, Color::Indexed(1));
        assert_eq!(term.grid[0][1].fg, Color::Default);
    }

    #[test]
    fn test_sgr_256_color() {
        let mut term = Terminal::new(80, 24);
        term.feed(b"\x1b[38;5;208mX");
        assert_eq!(term.grid[0][0].fg, Color::Indexed(208));
    }

    #[test]
    fn test_sgr_truecolor() {
        let mut term = Terminal::new(80, 24);
        term.feed(b"\x1b[38;2;255;128;0mX");
        assert_eq!(term.grid[0][0].fg, Color::Rgb(255, 128, 0));
    }

    #[test]
    fn test_scrollback() {
        let mut term = Terminal::new(80, 3);
        term.feed(b"line1\r\nline2\r\nline3\r\nline4");
        // line1 should have scrolled into scrollback
        assert_eq!(term.scrollback.len(), 1);
        assert_eq!(term.scrollback[0][0].c, 'l');
        assert_eq!(term.scrollback[0][4].c, '1');
    }

    #[test]
    fn test_cursor_hide_show() {
        let mut term = Terminal::new(80, 24);
        assert!(term.cursor_visible);
        term.feed(b"\x1b[?25l");
        assert!(!term.cursor_visible);
        term.feed(b"\x1b[?25h");
        assert!(term.cursor_visible);
    }

    #[test]
    fn test_alt_screen() {
        let mut term = Terminal::new(80, 24);
        term.feed(b"hello");
        assert_eq!(term.grid[0][0].c, 'h');
        term.feed(b"\x1b[?1049h"); // enter alt screen
        assert!(term.using_alt_screen);
        assert_eq!(term.grid[0][0].c, ' '); // alt screen is blank
        term.feed(b"\x1b[?1049l"); // exit alt screen
        assert!(!term.using_alt_screen);
        assert_eq!(term.grid[0][0].c, 'h'); // original content restored
    }

    #[test]
    fn test_scroll_region() {
        let mut term = Terminal::new(80, 5);
        // Set scroll region to rows 2-4 (1-indexed: 2;4)
        term.feed(b"\x1b[2;4r");
        assert_eq!(term.scroll_top, 1);
        assert_eq!(term.scroll_bottom, 3);
    }

    #[test]
    fn test_insert_delete_lines() {
        let mut term = Terminal::new(80, 5);
        term.feed(b"AAAA\r\nBBBB\r\nCCCC\r\nDDDD\r\nEEEE");
        // Move to row 2 (0-indexed: 1) and insert a line
        term.feed(b"\x1b[2;1H\x1b[1L");
        assert_eq!(term.grid[1][0].c, ' '); // inserted blank line
        assert_eq!(term.grid[2][0].c, 'B'); // B moved down
    }

    #[test]
    fn test_erase_characters() {
        let mut term = Terminal::new(80, 24);
        term.feed(b"hello");
        term.feed(b"\x1b[1;1H\x1b[3X"); // erase 3 chars from position 0
        assert_eq!(term.grid[0][0].c, ' ');
        assert_eq!(term.grid[0][2].c, ' ');
        assert_eq!(term.grid[0][3].c, 'l'); // untouched
    }

    #[test]
    fn test_save_restore_cursor() {
        let mut term = Terminal::new(80, 24);
        term.feed(b"\x1b[5;10H"); // move to row 5, col 10
        term.feed(b"\x1b7"); // save cursor (ESC 7)
        term.feed(b"\x1b[1;1H"); // move to top-left
        term.feed(b"\x1b8"); // restore cursor (ESC 8)
        assert_eq!(term.cursor_y, 4);
        assert_eq!(term.cursor_x, 9);
    }

    #[test]
    fn test_get_text() {
        let mut term = Terminal::new(80, 24);
        term.feed(b"hello world");
        let text = term.get_text(0, 0, 5, 0);
        assert_eq!(text, "hello");
    }

    #[test]
    fn test_search() {
        let mut term = Terminal::new(80, 24);
        term.feed(b"hello world\r\nhello rust\r\ngoodbye");
        let matches = term.search("hello");
        assert_eq!(matches.len(), 2);
        assert_eq!(matches[0], (0, 0)); // row 0, col 0 (grid row since no scrollback)
        assert_eq!(matches[1], (1, 0)); // row 1, col 0
    }

    #[test]
    fn test_search_case_insensitive() {
        let mut term = Terminal::new(80, 24);
        term.feed(b"Hello HELLO hello");
        let matches = term.search("hello");
        assert_eq!(matches.len(), 3);
    }

    #[test]
    fn test_search_empty() {
        let term = Terminal::new(80, 24);
        let matches = term.search("");
        assert!(matches.is_empty());
    }

    #[test]
    fn test_search_in_scrollback() {
        let mut term = Terminal::new(80, 3);
        term.feed(b"findme\r\nline2\r\nline3\r\nline4");
        // "findme" should be in scrollback
        assert!(!term.scrollback.is_empty());
        let matches = term.search("findme");
        assert_eq!(matches.len(), 1);
        assert_eq!(matches[0].0, 0); // first scrollback line
    }
}
