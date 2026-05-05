mod colors;

use std::io::Read;
use std::sync::mpsc;
use std::sync::Arc;
use std::thread;
use std::time::Instant;

use winit::application::ApplicationHandler;
use winit::event::{ElementState, MouseButton, MouseScrollDelta, WindowEvent};
use winit::event_loop::{ActiveEventLoop, EventLoop};
use winit::keyboard::{Key, ModifiersState, NamedKey};
use winit::window::{Window, WindowId};

use termai_core::{CursorStyle, Terminal};
use termai_pty::PtySession;
use termai_renderer::{RenderCell, Renderer};

const MIN_FONT_SIZE: f32 = 10.0;
const MAX_FONT_SIZE: f32 = 60.0;
const ZOOM_STEP: f32 = 2.0;
const CURSOR_BLINK_MS: u128 = 530;

/// Selection state for mouse text selection.
#[derive(Clone)]
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
    window: Option<Arc<Window>>,
    renderer: Option<Renderer>,
    terminal: Terminal,
    pty: Option<PtySession>,
    pty_rx: Option<mpsc::Receiver<Vec<u8>>>,
    modifiers: ModifiersState,
    font_size: f32,
    scale_factor: f32,
    cursor_blink_start: Instant,
    selection: Option<Selection>,
    mouse_pressed: bool,
    clipboard: Option<arboard::Clipboard>,
}

impl App {
    fn new() -> Self {
        Self {
            window: None,
            renderer: None,
            terminal: Terminal::new(80, 24),
            pty: None,
            pty_rx: None,
            modifiers: ModifiersState::empty(),
            font_size: 14.0,
            scale_factor: 1.0,
            cursor_blink_start: Instant::now(),
            selection: None,
            mouse_pressed: false,
            clipboard: arboard::Clipboard::new().ok(),
        }
    }

    fn pixel_to_cell(&self, x: f64, y: f64) -> (usize, usize) {
        if let Some(ref renderer) = self.renderer {
            let (cw, ch) = renderer.cell_size();
            let col = (x as f32 * self.scale_factor / cw).floor() as usize;
            let row = (y as f32 * self.scale_factor / ch).floor() as usize;
            (
                col.min(self.terminal.cols.saturating_sub(1)),
                row.min(self.terminal.rows.saturating_sub(1)),
            )
        } else {
            (0, 0)
        }
    }

    fn build_render_cells(&self) -> Vec<Vec<RenderCell>> {
        let visible = self.terminal.visible_grid();
        let cursor_on = self.terminal.cursor_visible
            && self.terminal.scroll_offset == 0
            && (self.cursor_blink_start.elapsed().as_millis() / CURSOR_BLINK_MS) % 2 == 0;

        let sel = self.selection.as_ref().map(|s| s.normalized());

        visible
            .iter()
            .enumerate()
            .map(|(row_idx, row)| {
                row.iter()
                    .enumerate()
                    .map(|(col_idx, cell)| {
                        let mut fg = colors::resolve_fg(cell.fg, cell.bold);
                        let mut bg = colors::resolve_bg(cell.bg);

                        // Inverse video
                        if cell.inverse {
                            std::mem::swap(&mut fg, &mut bg);
                        }

                        // Selection highlight
                        if let Some((sc, sr, ec, er)) = sel {
                            let in_selection = if sr == er {
                                row_idx == sr && col_idx >= sc && col_idx < ec
                            } else if row_idx == sr {
                                col_idx >= sc
                            } else if row_idx == er {
                                col_idx < ec
                            } else {
                                row_idx > sr && row_idx < er
                            };
                            if in_selection {
                                // Invert colors for selection
                                std::mem::swap(&mut fg, &mut bg);
                            }
                        }

                        // Cursor
                        if cursor_on
                            && row_idx == self.terminal.cursor_y
                            && col_idx == self.terminal.cursor_x
                        {
                            match self.terminal.cursor_style {
                                CursorStyle::Block => {
                                    fg = colors::BG;
                                    bg = colors::FG;
                                }
                                CursorStyle::Underline => {
                                    // Rendered as a special underline — just highlight for now
                                    bg = [0.3, 0.3, 0.35, 1.0];
                                }
                                CursorStyle::Bar => {
                                    // Thin bar is hard with quad rendering — use highlight
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

    fn zoom(&mut self) {
        if let Some(ref mut renderer) = self.renderer {
            renderer.rebuild_atlas(self.font_size, self.scale_factor);
            let (cols, rows) = renderer.grid_size();
            let mut new_grid =
                vec![vec![termai_core::Cell::default(); cols as usize]; rows as usize];
            for (y, row) in self.terminal.grid.iter().enumerate() {
                if y >= rows as usize {
                    break;
                }
                for (x, cell) in row.iter().enumerate() {
                    if x >= cols as usize {
                        break;
                    }
                    new_grid[y][x] = cell.clone();
                }
            }
            self.terminal.cols = cols as usize;
            self.terminal.rows = rows as usize;
            self.terminal.grid = new_grid;
            self.terminal.cursor_x = self.terminal.cursor_x.min(cols as usize - 1);
            self.terminal.cursor_y = self.terminal.cursor_y.min(rows as usize - 1);
        }
    }

    fn copy_selection(&mut self) {
        if let Some(ref sel) = self.selection {
            let (sc, sr, ec, er) = sel.normalized();
            let text = self.terminal.get_text(sc, sr, ec, er);
            if !text.is_empty() {
                if let Some(ref mut clip) = self.clipboard {
                    let _ = clip.set_text(&text);
                }
            }
        }
    }

    fn paste(&mut self) {
        if let Some(ref mut clip) = self.clipboard {
            if let Ok(text) = clip.get_text() {
                if let Some(ref mut pty) = self.pty {
                    let _ = pty.write(text.as_bytes());
                }
            }
        }
    }
}

impl ApplicationHandler for App {
    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        if self.window.is_some() {
            return;
        }

        let attrs = Window::default_attributes()
            .with_title("termAI")
            .with_inner_size(winit::dpi::LogicalSize::new(1024, 640));

        let window = Arc::new(
            event_loop
                .create_window(attrs)
                .expect("Failed to create window"),
        );

        self.scale_factor = window.scale_factor() as f32;
        let renderer = Renderer::new(window.clone(), self.scale_factor, self.font_size);

        let (cols, rows) = renderer.grid_size();
        self.terminal = Terminal::new(cols as usize, rows as usize);

        let mut pty =
            PtySession::spawn(cols as u16, rows as u16).expect("Failed to spawn PTY");

        let (tx, rx) = mpsc::channel::<Vec<u8>>();
        let mut pty_reader = pty.take_reader();
        thread::spawn(move || {
            let mut buf = [0u8; 4096];
            loop {
                match pty_reader.read(&mut buf) {
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

        self.pty_rx = Some(rx);
        self.pty = Some(pty);
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
                    let (cols, rows) = renderer.grid_size();
                    self.terminal = Terminal::new(cols as usize, rows as usize);
                }
            }

            WindowEvent::ModifiersChanged(modifiers) => {
                self.modifiers = modifiers.state();
            }

            WindowEvent::MouseWheel { delta, .. } => {
                let lines = match delta {
                    MouseScrollDelta::LineDelta(_, y) => (y.abs() as usize).max(1) * if y < 0.0 { 1 } else { 0 },
                    MouseScrollDelta::PixelDelta(pos) => {
                        let l = (pos.y.abs() / 20.0) as usize;
                        if pos.y < 0.0 { l.max(1) } else { 0 }
                    }
                };
                let lines_up = match delta {
                    MouseScrollDelta::LineDelta(_, y) => if y > 0.0 { (y as usize).max(1) } else { 0 },
                    MouseScrollDelta::PixelDelta(pos) => if pos.y > 0.0 { ((pos.y / 20.0) as usize).max(1) } else { 0 },
                };
                if lines_up > 0 {
                    self.terminal.scroll_viewport_up(lines_up);
                }
                if lines > 0 {
                    self.terminal.scroll_viewport_down(lines);
                }
            }

            WindowEvent::MouseInput { state, button, .. } => {
                if button == MouseButton::Left {
                    match state {
                        ElementState::Pressed => {
                            self.mouse_pressed = true;
                            self.selection = None;
                        }
                        ElementState::Released => {
                            self.mouse_pressed = false;
                        }
                    }
                }
            }

            WindowEvent::CursorMoved { position, .. } => {
                if self.mouse_pressed {
                    let (col, row) = self.pixel_to_cell(position.x, position.y);
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

            WindowEvent::KeyboardInput { event, .. } => {
                if event.state != ElementState::Pressed {
                    return;
                }

                // Reset cursor blink on keypress
                self.cursor_blink_start = Instant::now();

                let is_super = self.modifiers.super_key();

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
                        // Copy
                        Key::Character(c) if c.as_str() == "c" => {
                            self.copy_selection();
                            return;
                        }
                        // Paste
                        Key::Character(c) if c.as_str() == "v" => {
                            self.paste();
                            return;
                        }
                        _ => {}
                    }
                }

                // Shift+PageUp/Down for scrollback
                let is_shift = self.modifiers.shift_key();
                if is_shift {
                    match &event.logical_key {
                        Key::Named(NamedKey::PageUp) => {
                            self.terminal.scroll_viewport_up(self.terminal.rows / 2);
                            return;
                        }
                        Key::Named(NamedKey::PageDown) => {
                            self.terminal.scroll_viewport_down(self.terminal.rows / 2);
                            return;
                        }
                        _ => {}
                    }
                }

                // Clear selection on any key
                self.selection = None;

                if let Some(ref mut pty) = self.pty {
                    let bytes = key_to_bytes(&event.logical_key, &event.text);
                    if !bytes.is_empty() {
                        let _ = pty.write(&bytes);
                    }
                }
            }

            WindowEvent::RedrawRequested => {
                // Drain PTY output
                if let Some(ref rx) = self.pty_rx {
                    while let Ok(data) = rx.try_recv() {
                        self.terminal.feed(&data);
                    }
                }

                let cells = self.build_render_cells();
                if let Some(ref mut renderer) = self.renderer {
                    match renderer.render(&cells) {
                        Ok(_) => {}
                        Err(wgpu::SurfaceError::Lost) => {
                            let (w, h) = (renderer.width(), renderer.height());
                            renderer.resize(w, h);
                        }
                        Err(wgpu::SurfaceError::OutOfMemory) => {
                            event_loop.exit();
                        }
                        Err(e) => {
                            log::warn!("Render error: {e:?}");
                        }
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

    let event_loop = EventLoop::new().expect("Failed to create event loop");
    let mut app = App::new();

    event_loop.run_app(&mut app).expect("Event loop failed");
}
