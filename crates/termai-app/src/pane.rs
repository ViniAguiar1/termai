use std::io::Read;
use std::sync::mpsc;
use std::thread;

use termai_core::Terminal;
use termai_pty::PtySession;

/// A single pane containing a terminal + PTY.
pub struct Pane {
    pub terminal: Terminal,
    pub pty: PtySession,
    pub pty_rx: mpsc::Receiver<Vec<u8>>,
    pub id: u64,
    /// Rolling buffer of recent PTY output (last ~4KB) for error detection.
    pub recent_output: String,
}

static NEXT_PANE_ID: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(1);

impl Pane {
    pub fn new(cols: usize, rows: usize) -> Self {
        let terminal = Terminal::new(cols, rows);
        let mut pty = PtySession::spawn(cols as u16, rows as u16)
            .expect("Failed to spawn PTY");

        let (tx, rx) = mpsc::channel::<Vec<u8>>();
        let mut reader = pty.take_reader();
        thread::spawn(move || {
            let mut buf = [0u8; 4096];
            loop {
                match reader.read(&mut buf) {
                    Ok(0) => break,
                    Ok(n) => {
                        if tx.send(buf[..n].to_vec()).is_err() {
                            break;
                        }
                    }
                    Err(_) => break,
                }
            }
        });

        let id = NEXT_PANE_ID.fetch_add(1, std::sync::atomic::Ordering::Relaxed);

        Self {
            terminal,
            pty,
            pty_rx: rx,
            id,
            recent_output: String::new(),
        }
    }

    /// Drain any pending PTY output into the terminal.
    /// Returns true if new data was received.
    pub fn poll(&mut self) -> bool {
        let mut got_data = false;
        while let Ok(data) = self.pty_rx.try_recv() {
            self.terminal.feed(&data);
            // Append to recent output buffer (lossy UTF-8 for error detection)
            self.recent_output.push_str(&String::from_utf8_lossy(&data));
            // Keep only last 4KB
            if self.recent_output.len() > 4096 {
                let start = self.recent_output.len() - 4096;
                self.recent_output = self.recent_output[start..].to_string();
            }
            got_data = true;
        }
        got_data
    }

    /// Clear the recent output buffer (after sending to AI for analysis).
    pub fn clear_recent_output(&mut self) {
        self.recent_output.clear();
    }

    /// Write bytes to the PTY (keyboard input).
    pub fn write(&mut self, data: &[u8]) {
        let _ = self.pty.write(data);
    }

    /// Resize both the terminal grid and PTY to new dimensions.
    pub fn resize(&mut self, cols: usize, rows: usize) {
        if cols == self.terminal.cols && rows == self.terminal.rows {
            return;
        }
        resize_terminal(&mut self.terminal, cols, rows);
        let _ = self.pty.resize(cols as u16, rows as u16);
    }
}

/// Split direction.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum SplitDir {
    Vertical,   // side by side (left | right)
    Horizontal, // stacked (top / bottom)
}

/// A tree node: either a leaf (Pane) or a split containing two children.
pub enum PaneNode {
    Leaf(Pane),
    Split {
        dir: SplitDir,
        ratio: f32, // 0.0..1.0, how much space the first child gets
        first: Box<PaneNode>,
        second: Box<PaneNode>,
    },
}

impl PaneNode {
    /// Create a leaf node with a new pane.
    pub fn new_leaf(cols: usize, rows: usize) -> Self {
        PaneNode::Leaf(Pane::new(cols, rows))
    }

    /// Poll all panes in the tree. Returns true if any pane received data.
    pub fn poll_all(&mut self) -> bool {
        match self {
            PaneNode::Leaf(pane) => pane.poll(),
            PaneNode::Split { first, second, .. } => {
                let a = first.poll_all();
                let b = second.poll_all();
                a || b
            }
        }
    }

    /// Find a pane by ID.
    pub fn find_pane(&mut self, id: u64) -> Option<&mut Pane> {
        match self {
            PaneNode::Leaf(pane) => {
                if pane.id == id {
                    Some(pane)
                } else {
                    None
                }
            }
            PaneNode::Split { first, second, .. } => {
                first.find_pane(id).or_else(|| second.find_pane(id))
            }
        }
    }

    /// Get all pane IDs in order (left-to-right / top-to-bottom).
    pub fn pane_ids(&self) -> Vec<u64> {
        match self {
            PaneNode::Leaf(pane) => vec![pane.id],
            PaneNode::Split { first, second, .. } => {
                let mut ids = first.pane_ids();
                ids.extend(second.pane_ids());
                ids
            }
        }
    }

    /// Count total panes.
    pub fn pane_count(&self) -> usize {
        match self {
            PaneNode::Leaf(_) => 1,
            PaneNode::Split { first, second, .. } => {
                first.pane_count() + second.pane_count()
            }
        }
    }

    /// Split the pane with the given ID, creating a new pane as the second child.
    /// Returns the new pane's ID, or None if the pane wasn't found.
    pub fn split(&mut self, target_id: u64, dir: SplitDir, cols: usize, rows: usize) -> Option<u64> {
        match self {
            PaneNode::Leaf(pane) if pane.id == target_id => {
                // Calculate child sizes
                let (c1, r1, c2, r2) = match dir {
                    SplitDir::Vertical => {
                        let half = cols / 2;
                        (half.max(1), rows, (cols - half).max(1), rows)
                    }
                    SplitDir::Horizontal => {
                        let half = rows / 2;
                        (cols, half.max(1), cols, (rows - half).max(1))
                    }
                };

                // Resize existing pane's terminal and PTY preserving content
                resize_terminal(&mut pane.terminal, c1, r1);
                let _ = pane.pty.resize(c1 as u16, r1 as u16);

                // Take ownership of the current leaf
                let old = std::mem::replace(self, PaneNode::new_leaf(1, 1)); // placeholder
                let new_pane = PaneNode::new_leaf(c2, r2);
                let new_id = match &new_pane {
                    PaneNode::Leaf(p) => p.id,
                    _ => unreachable!(),
                };

                *self = PaneNode::Split {
                    dir,
                    ratio: 0.5,
                    first: Box::new(old),
                    second: Box::new(new_pane),
                };

                Some(new_id)
            }
            PaneNode::Leaf(_) => None,
            PaneNode::Split { first, second, .. } => {
                first.split(target_id, dir, cols, rows)
                    .or_else(|| second.split(target_id, dir, cols, rows))
            }
        }
    }

    /// Remove a pane by ID. Returns true if the pane was removed.
    /// The tree restructures so the sibling takes the parent's place.
    pub fn remove(&mut self, target_id: u64) -> bool {
        match self {
            PaneNode::Leaf(_) => false, // Can't remove the last pane from itself
            PaneNode::Split { first, second, .. } => {
                // Check if first child is the target leaf
                if matches!(first.as_ref(), PaneNode::Leaf(p) if p.id == target_id) {
                    let sibling = std::mem::replace(second.as_mut(), PaneNode::new_leaf(1, 1));
                    *self = sibling;
                    return true;
                }
                // Check if second child is the target leaf
                if matches!(second.as_ref(), PaneNode::Leaf(p) if p.id == target_id) {
                    let sibling = std::mem::replace(first.as_mut(), PaneNode::new_leaf(1, 1));
                    *self = sibling;
                    return true;
                }
                // Recurse
                first.remove(target_id) || second.remove(target_id)
            }
        }
    }

    /// Resize all panes based on their layout rects and cell size.
    pub fn resize_all(&mut self, rects: &[PaneRect], cell_w: f32, cell_h: f32) {
        for rect in rects {
            if let Some(pane) = self.find_pane(rect.id) {
                let cols = (rect.w / cell_w).floor().max(1.0) as usize;
                let rows = (rect.h / cell_h).floor().max(1.0) as usize;
                pane.resize(cols, rows);
            }
        }
    }

    /// Collect all panes with their pixel-space rectangles.
    /// Returns (pane_id, x, y, width, height) for each pane.
    pub fn layout(&self, x: f32, y: f32, w: f32, h: f32) -> Vec<PaneRect> {
        match self {
            PaneNode::Leaf(pane) => {
                vec![PaneRect {
                    id: pane.id,
                    x,
                    y,
                    w,
                    h,
                }]
            }
            PaneNode::Split {
                dir,
                ratio,
                first,
                second,
            } => {
                let mut rects = Vec::new();
                match dir {
                    SplitDir::Vertical => {
                        let divider = 1.0; // 1px divider
                        let first_w = (w * ratio - divider / 2.0).max(0.0);
                        let second_w = (w * (1.0 - ratio) - divider / 2.0).max(0.0);
                        rects.extend(first.layout(x, y, first_w, h));
                        rects.extend(second.layout(x + first_w + divider, y, second_w, h));
                    }
                    SplitDir::Horizontal => {
                        let divider = 1.0;
                        let first_h = (h * ratio - divider / 2.0).max(0.0);
                        let second_h = (h * (1.0 - ratio) - divider / 2.0).max(0.0);
                        rects.extend(first.layout(x, y, w, first_h));
                        rects.extend(second.layout(x, y + first_h + divider, w, second_h));
                    }
                }
                rects
            }
        }
    }
}

/// A pane's position and size in pixel coordinates.
#[derive(Clone, Debug)]
pub struct PaneRect {
    pub id: u64,
    pub x: f32,
    pub y: f32,
    pub w: f32,
    pub h: f32,
}

/// Resize a terminal grid, preserving as much content as possible.
fn resize_terminal(term: &mut Terminal, new_cols: usize, new_rows: usize) {
    let mut new_grid = vec![vec![termai_core::Cell::default(); new_cols]; new_rows];
    for (y, row) in term.grid.iter().enumerate() {
        if y >= new_rows {
            break;
        }
        for (x, cell) in row.iter().enumerate() {
            if x >= new_cols {
                break;
            }
            new_grid[y][x] = cell.clone();
        }
    }
    term.cols = new_cols;
    term.rows = new_rows;
    term.grid = new_grid;
    term.cursor_x = term.cursor_x.min(new_cols.saturating_sub(1));
    term.cursor_y = term.cursor_y.min(new_rows.saturating_sub(1));
}
