use vte::{Params, Parser, Perform};

/// Terminal cell holding a character and its attributes.
#[derive(Clone, Debug)]
pub struct Cell {
    pub c: char,
    pub fg: Color,
    pub bg: Color,
    pub bold: bool,
}

impl Default for Cell {
    fn default() -> Self {
        Self {
            c: ' ',
            fg: Color::Default,
            bg: Color::Default,
            bold: false,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Color {
    Default,
    Indexed(u8),
    Rgb(u8, u8, u8),
}

/// Current text attributes applied to new characters.
#[derive(Clone, Debug)]
struct Attrs {
    fg: Color,
    bg: Color,
    bold: bool,
}

impl Default for Attrs {
    fn default() -> Self {
        Self {
            fg: Color::Default,
            bg: Color::Default,
            bold: false,
        }
    }
}

/// Minimal terminal state: grid of cells + cursor position.
pub struct Terminal {
    pub cols: usize,
    pub rows: usize,
    pub grid: Vec<Vec<Cell>>,
    pub cursor_x: usize,
    pub cursor_y: usize,
    attrs: Attrs,
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
            attrs: Attrs::default(),
        }
    }

    /// Feed raw bytes from PTY into the terminal state machine.
    pub fn feed(&mut self, bytes: &[u8]) {
        let mut parser = Parser::new();
        for &byte in bytes {
            parser.advance(self, byte);
        }
    }

    fn scroll_up(&mut self) {
        self.grid.remove(0);
        self.grid.push(vec![Cell::default(); self.cols]);
    }

    fn newline(&mut self) {
        self.cursor_x = 0;
        if self.cursor_y + 1 >= self.rows {
            self.scroll_up();
        } else {
            self.cursor_y += 1;
        }
    }

    fn apply_sgr(&mut self, params: &Params) {
        let mut iter = params.iter();

        // If no params, treat as reset (CSI 0 m)
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
                22 => self.attrs.bold = false,
                // Standard foreground colors (30-37)
                30..=37 => self.attrs.fg = Color::Indexed((code - 30) as u8),
                // Bright foreground colors (90-97)
                90..=97 => self.attrs.fg = Color::Indexed((code - 90 + 8) as u8),
                38 => {
                    self.parse_extended_color(&mut iter, &mut subparams, true);
                }
                39 => self.attrs.fg = Color::Default,
                // Standard background colors (40-47)
                40..=47 => self.attrs.bg = Color::Indexed((code - 40) as u8),
                // Bright background colors (100-107)
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
        // Next param determines the mode: 5 = indexed, 2 = RGB
        let mode = iter.next().and_then(|p| p.first().copied());
        match mode {
            Some(5) => {
                // 256-color: CSI 38;5;N m
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
                // True color: CSI 38;2;R;G;B m
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
        self.cursor_x += 1;
    }

    fn execute(&mut self, byte: u8) {
        match byte {
            b'\n' | 0x0B | 0x0C => self.newline(),
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
            _ => {}
        }
    }

    fn hook(&mut self, _params: &Params, _intermediates: &[u8], _ignore: bool, _action: char) {}
    fn put(&mut self, _byte: u8) {}
    fn unhook(&mut self) {}
    fn osc_dispatch(&mut self, _params: &[&[u8]], _bell_terminated: bool) {}

    fn csi_dispatch(
        &mut self,
        params: &Params,
        _intermediates: &[u8],
        _ignore: bool,
        action: char,
    ) {
        match action {
            // SGR - Select Graphic Rendition (colors/attributes)
            'm' => {
                self.apply_sgr(params);
                return;
            }
            _ => {}
        }

        let mut params_iter = params.iter();
        let first = params_iter
            .next()
            .and_then(|p| p.first().copied())
            .unwrap_or(1) as usize;

        match action {
            'A' => self.cursor_y = self.cursor_y.saturating_sub(first.max(1)),
            'B' => self.cursor_y = (self.cursor_y + first.max(1)).min(self.rows - 1),
            'C' => self.cursor_x = (self.cursor_x + first.max(1)).min(self.cols - 1),
            'D' => self.cursor_x = self.cursor_x.saturating_sub(first.max(1)),
            'H' | 'f' => {
                let row = first.saturating_sub(1).min(self.rows - 1);
                let col = params_iter
                    .next()
                    .and_then(|p| p.first().copied())
                    .unwrap_or(1) as usize;
                let col = col.saturating_sub(1).min(self.cols - 1);
                self.cursor_y = row;
                self.cursor_x = col;
            }
            'J' => match first {
                0 => {
                    for x in self.cursor_x..self.cols {
                        self.grid[self.cursor_y][x] = Cell::default();
                    }
                    for y in (self.cursor_y + 1)..self.rows {
                        self.grid[y] = vec![Cell::default(); self.cols];
                    }
                }
                2 | 3 => {
                    self.grid = vec![vec![Cell::default(); self.cols]; self.rows];
                    self.cursor_x = 0;
                    self.cursor_y = 0;
                }
                _ => {}
            },
            'K' => match first {
                0 => {
                    for x in self.cursor_x..self.cols {
                        self.grid[self.cursor_y][x] = Cell::default();
                    }
                }
                2 => {
                    self.grid[self.cursor_y] = vec![Cell::default(); self.cols];
                }
                _ => {}
            },
            _ => {}
        }
    }

    fn esc_dispatch(&mut self, _intermediates: &[u8], _ignore: bool, _byte: u8) {}
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_print_and_newline() {
        let mut term = Terminal::new(80, 24);
        term.feed(b"hello\nworld");
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
        // ESC[31m = red foreground, then print 'X'
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
        // ESC[38;5;208m = 256-color orange foreground
        term.feed(b"\x1b[38;5;208mX");
        assert_eq!(term.grid[0][0].fg, Color::Indexed(208));
    }

    #[test]
    fn test_sgr_truecolor() {
        let mut term = Terminal::new(80, 24);
        // ESC[38;2;255;128;0m = RGB foreground
        term.feed(b"\x1b[38;2;255;128;0mX");
        assert_eq!(term.grid[0][0].fg, Color::Rgb(255, 128, 0));
    }
}
