mod colors;
mod config;
mod pane;
mod tab;

use std::sync::Arc;
use std::time::Instant;

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

struct App {
    config: Config,
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
}

impl App {
    fn new() -> Self {
        Self {
            config: Config::default(),
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

    fn content_area(&self) -> (f32, f32, f32, f32) {
        if let Some(ref renderer) = self.renderer {
            let w = renderer.width() as f32;
            let h = renderer.height() as f32;
            let tab_h = self.tab_bar_pixel_height();
            (0.0, tab_h, w, h - tab_h)
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
        let cursor_on = is_focused
            && pane.terminal.cursor_visible
            && pane.terminal.scroll_offset == 0
            && (self.cursor_blink_start.elapsed().as_millis() / CURSOR_BLINK_MS) % 2 == 0;

        let sel = if is_focused {
            self.selection.as_ref().map(|s| s.normalized())
        } else {
            None
        };

        visible
            .iter()
            .enumerate()
            .map(|(row_idx, row)| {
                row.iter()
                    .enumerate()
                    .map(|(col_idx, cell)| {
                        let mut fg = colors::resolve_fg(cell.fg, cell.bold);
                        let mut bg = colors::resolve_bg(cell.bg);

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

                        if cursor_on
                            && row_idx == pane.terminal.cursor_y
                            && col_idx == pane.terminal.cursor_x
                        {
                            match pane.terminal.cursor_style {
                                CursorStyle::Block => {
                                    fg = colors::BG;
                                    bg = colors::FG;
                                }
                                CursorStyle::Underline | CursorStyle::Bar => {
                                    bg = [0.3, 0.3, 0.35, 1.0];
                                }
                            }
                        }

                        RenderCell { ch: cell.c, fg, bg }
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
        let mut row = vec![
            RenderCell {
                ch: ' ',
                fg: [0.5, 0.5, 0.5, 1.0],
                bg: [0.15, 0.15, 0.17, 1.0],
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
                    fg: if is_active {
                        [1.0, 1.0, 1.0, 1.0]
                    } else {
                        [0.5, 0.5, 0.5, 1.0]
                    },
                    bg: if is_active {
                        [0.25, 0.25, 0.28, 1.0]
                    } else {
                        [0.15, 0.15, 0.17, 1.0]
                    },
                };
                col += 1;
            }

            // Separator
            if col < cols as usize {
                row[col] = RenderCell {
                    ch: '|',
                    fg: [0.3, 0.3, 0.3, 1.0],
                    bg: [0.15, 0.15, 0.17, 1.0],
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

        let sy = py as f32 * self.scale_factor;
        if sy >= tab_h {
            return false; // Click is below tab bar
        }

        // Determine which tab was clicked based on x position
        let renderer = match self.renderer {
            Some(ref r) => r,
            None => return false,
        };
        let (cw, _) = renderer.cell_size();
        let sx = px as f32 * self.scale_factor;
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

    fn zoom(&mut self) {
        if let Some(ref mut renderer) = self.renderer {
            renderer.rebuild_atlas(self.font_size, self.scale_factor);
        }
        // Panes will be re-created with correct sizes on next layout
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
        let renderer = Renderer::new(window.clone(), self.scale_factor, self.font_size);

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
                    // Pane terminals will be resized on next render via layout
                }
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

                // Handle click on tab bar (on first move after press)
                if self.mouse_just_pressed {
                    self.mouse_just_pressed = false;
                    if self.handle_tab_bar_click(position.x, position.y) {
                        self.mouse_pressed = false;
                        return;
                    }
                }
                // Click to focus pane
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

                self.selection = None;

                // Send key to focused pane
                let tab = self.tab_bar.active_tab();
                let id = tab.focused_pane_id;
                if let Some(pane) = tab.root.find_pane(id) {
                    let bytes = key_to_bytes(&event.logical_key, &event.text);
                    if !bytes.is_empty() {
                        pane.write(&bytes);
                    }
                }
            }

            WindowEvent::RedrawRequested => {
                // Poll all panes in active tab
                self.tab_bar.active_tab().poll();

                let renderer = match self.renderer {
                    Some(ref r) => r,
                    None => return,
                };

                let mut vertices: Vec<Vertex> = Vec::new();

                // Tab bar
                let tab_bar_cells = self.build_tab_bar_cells();
                if !tab_bar_cells.is_empty() {
                    renderer.build_vertices(&tab_bar_cells, 0.0, 0.0, &mut vertices);
                }

                // Pane content
                let (cx, cy, cw, ch) = self.content_area();
                let tab = &self.tab_bar.tabs[self.tab_bar.active];
                let rects = tab.layout(cx, cy, cw, ch);
                let focused_id = tab.focused_pane_id;

                for rect in &rects {
                    // Fill pane background
                    renderer.build_rect(rect.x, rect.y, rect.w, rect.h, colors::BG, &mut vertices);

                    if let Some(pane) = find_pane_ref(&tab.root, rect.id) {
                        let is_focused = rect.id == focused_id;
                        let cells = self.build_pane_cells(pane, is_focused);
                        renderer.build_vertices(&cells, rect.x, rect.y, &mut vertices);
                    }
                }

                // Dividers between panes
                if rects.len() > 1 {
                    // Draw divider lines between adjacent panes
                    for i in 0..rects.len() {
                        for j in (i + 1)..rects.len() {
                            let a = &rects[i];
                            let b = &rects[j];
                            // Vertical divider (side by side)
                            if (a.x + a.w - b.x).abs() < 2.0 {
                                let div_x = a.x + a.w;
                                let div_y = a.y.min(b.y);
                                let div_h = a.h.max(b.h);
                                renderer.build_divider(div_x, div_y, 1.0, div_h, &mut vertices);
                            }
                            // Horizontal divider (stacked)
                            if (a.y + a.h - b.y).abs() < 2.0 {
                                let div_x = a.x.min(b.x);
                                let div_y = a.y + a.h;
                                let div_w = a.w.max(b.w);
                                renderer.build_divider(div_x, div_y, div_w, 1.0, &mut vertices);
                            }
                        }
                    }
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
    app.config = config;

    event_loop.run_app(&mut app).expect("Event loop failed");
}
