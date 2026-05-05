use vte::{Params, Parser, Perform};

/// Terminal cell holding a character and its attributes.
#[derive(Clone, Debug)]
pub struct Cell {
    pub c: char,
    pub fg: Color,
    pub bg: Color,
}

impl Default for Cell {
    fn default() -> Self {
        Self {
            c: ' ',
            fg: Color::Default,
            bg: Color::Default,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Color {
    Default,
    Indexed(u8),
    Rgb(u8, u8, u8),
}

/// Minimal terminal state: grid of cells + cursor position.
pub struct Terminal {
    pub cols: usize,
    pub rows: usize,
    pub grid: Vec<Vec<Cell>>,
    pub cursor_x: usize,
    pub cursor_y: usize,
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
}

impl Perform for Terminal {
    fn print(&mut self, c: char) {
        if self.cursor_x >= self.cols {
            self.newline();
        }
        self.grid[self.cursor_y][self.cursor_x].c = c;
        self.cursor_x += 1;
    }

    fn execute(&mut self, byte: u8) {
        match byte {
            b'\n' | 0x0B | 0x0C => self.newline(),
            b'\r' => self.cursor_x = 0,
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
        let mut params_iter = params.iter();
        let first = params_iter.next().and_then(|p| p.first().copied()).unwrap_or(1) as usize;

        match action {
            // Cursor Up
            'A' => self.cursor_y = self.cursor_y.saturating_sub(first.max(1)),
            // Cursor Down
            'B' => self.cursor_y = (self.cursor_y + first.max(1)).min(self.rows - 1),
            // Cursor Forward
            'C' => self.cursor_x = (self.cursor_x + first.max(1)).min(self.cols - 1),
            // Cursor Back
            'D' => self.cursor_x = self.cursor_x.saturating_sub(first.max(1)),
            // Cursor Position (H)
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
            // Erase in Display
            'J' => {
                match first {
                    0 => {
                        // Clear from cursor to end
                        for x in self.cursor_x..self.cols {
                            self.grid[self.cursor_y][x] = Cell::default();
                        }
                        for y in (self.cursor_y + 1)..self.rows {
                            self.grid[y] = vec![Cell::default(); self.cols];
                        }
                    }
                    2 | 3 => {
                        // Clear entire screen
                        self.grid = vec![vec![Cell::default(); self.cols]; self.rows];
                        self.cursor_x = 0;
                        self.cursor_y = 0;
                    }
                    _ => {}
                }
            }
            // Erase in Line
            'K' => {
                match first {
                    0 => {
                        for x in self.cursor_x..self.cols {
                            self.grid[self.cursor_y][x] = Cell::default();
                        }
                    }
                    2 => {
                        self.grid[self.cursor_y] = vec![Cell::default(); self.cols];
                    }
                    _ => {}
                }
            }
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
}
