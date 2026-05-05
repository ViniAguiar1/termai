use crate::pane::{PaneNode, PaneRect, SplitDir};

/// A tab containing a tree of panes.
pub struct Tab {
    pub root: PaneNode,
    pub focused_pane_id: u64,
    pub title: String,
}

impl Tab {
    pub fn new(cols: usize, rows: usize) -> Self {
        let root = PaneNode::new_leaf(cols, rows);
        let id = root.pane_ids()[0];
        Self {
            root,
            focused_pane_id: id,
            title: String::from("shell"),
        }
    }

    /// Split the focused pane.
    pub fn split(&mut self, dir: SplitDir, cols: usize, rows: usize) -> Option<u64> {
        let new_id = self.root.split(self.focused_pane_id, dir, cols, rows)?;
        self.focused_pane_id = new_id;
        Some(new_id)
    }

    /// Close the focused pane. Returns false if it's the last pane.
    pub fn close_focused(&mut self) -> bool {
        if self.root.pane_count() <= 1 {
            return false;
        }

        let ids = self.root.pane_ids();
        let current_idx = ids.iter().position(|&id| id == self.focused_pane_id);

        if self.root.remove(self.focused_pane_id) {
            // Focus the next available pane
            let new_ids = self.root.pane_ids();
            if let Some(idx) = current_idx {
                let next = if idx < new_ids.len() { idx } else { new_ids.len() - 1 };
                self.focused_pane_id = new_ids[next];
            } else {
                self.focused_pane_id = new_ids[0];
            }
            true
        } else {
            false
        }
    }

    /// Focus the next pane in order.
    pub fn focus_next(&mut self) {
        let ids = self.root.pane_ids();
        if let Some(idx) = ids.iter().position(|&id| id == self.focused_pane_id) {
            let next = (idx + 1) % ids.len();
            self.focused_pane_id = ids[next];
        }
    }

    /// Focus the previous pane in order.
    pub fn focus_prev(&mut self) {
        let ids = self.root.pane_ids();
        if let Some(idx) = ids.iter().position(|&id| id == self.focused_pane_id) {
            let prev = if idx == 0 { ids.len() - 1 } else { idx - 1 };
            self.focused_pane_id = ids[prev];
        }
    }

    /// Get layout rectangles for all panes.
    pub fn layout(&self, x: f32, y: f32, w: f32, h: f32) -> Vec<PaneRect> {
        self.root.layout(x, y, w, h)
    }

    /// Poll all panes.
    pub fn poll(&mut self) {
        self.root.poll_all();
    }
}

/// Manages multiple tabs.
pub struct TabBar {
    pub tabs: Vec<Tab>,
    pub active: usize,
}

impl TabBar {
    pub fn new(cols: usize, rows: usize) -> Self {
        Self {
            tabs: vec![Tab::new(cols, rows)],
            active: 0,
        }
    }

    pub fn active_tab(&mut self) -> &mut Tab {
        &mut self.tabs[self.active]
    }

    pub fn new_tab(&mut self, cols: usize, rows: usize) {
        self.tabs.push(Tab::new(cols, rows));
        self.active = self.tabs.len() - 1;
    }

    /// Close the active tab. Returns false if it's the last tab.
    pub fn close_active_tab(&mut self) -> bool {
        if self.tabs.len() <= 1 {
            return false;
        }
        self.tabs.remove(self.active);
        if self.active >= self.tabs.len() {
            self.active = self.tabs.len() - 1;
        }
        true
    }

    pub fn switch_to(&mut self, index: usize) {
        if index < self.tabs.len() {
            self.active = index;
        }
    }

    pub fn tab_count(&self) -> usize {
        self.tabs.len()
    }
}
