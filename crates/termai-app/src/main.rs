mod colors;

use std::io::Read;
use std::sync::mpsc;
use std::sync::Arc;
use std::thread;

use winit::application::ApplicationHandler;
use winit::event::{ElementState, WindowEvent};
use winit::event_loop::{ActiveEventLoop, EventLoop};
use winit::keyboard::{Key, ModifiersState, NamedKey};
use winit::window::{Window, WindowId};

use termai_core::Terminal;
use termai_pty::PtySession;
use termai_renderer::{RenderCell, Renderer};

const MIN_FONT_SIZE: f32 = 10.0;
const MAX_FONT_SIZE: f32 = 60.0;
const ZOOM_STEP: f32 = 2.0;

struct App {
    window: Option<Arc<Window>>,
    renderer: Option<Renderer>,
    terminal: Terminal,
    pty: Option<PtySession>,
    pty_rx: Option<mpsc::Receiver<Vec<u8>>>,
    modifiers: ModifiersState,
    font_size: f32,
    scale_factor: f32,
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
        }
    }

    fn build_render_cells(&self) -> Vec<Vec<RenderCell>> {
        self.terminal
            .grid
            .iter()
            .map(|row| {
                row.iter()
                    .map(|cell| RenderCell {
                        ch: cell.c,
                        fg: colors::resolve_fg(cell.fg, cell.bold),
                        bg: colors::resolve_bg(cell.bg),
                    })
                    .collect()
            })
            .collect()
    }

    fn rebuild_renderer(&mut self) {
        let window = match self.window {
            Some(ref w) => w.clone(),
            None => return,
        };

        let renderer = Renderer::new(window, self.scale_factor, self.font_size);
        let (cols, rows) = renderer.grid_size();
        self.terminal = Terminal::new(cols as usize, rows as usize);
        self.renderer = Some(renderer);

        // TODO: resize PTY to match new grid
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

            WindowEvent::KeyboardInput { event, .. } => {
                if event.state != ElementState::Pressed {
                    return;
                }

                let is_super = self.modifiers.super_key();

                // Zoom: Cmd+ / Cmd-
                if is_super {
                    match &event.logical_key {
                        Key::Character(c) if c.as_str() == "=" || c.as_str() == "+" => {
                            self.font_size = (self.font_size + ZOOM_STEP).min(MAX_FONT_SIZE);
                            self.rebuild_renderer();
                            return;
                        }
                        Key::Character(c) if c.as_str() == "-" => {
                            self.font_size = (self.font_size - ZOOM_STEP).max(MIN_FONT_SIZE);
                            self.rebuild_renderer();
                            return;
                        }
                        Key::Character(c) if c.as_str() == "0" => {
                            self.font_size = 14.0;
                            self.rebuild_renderer();
                            return;
                        }
                        _ => {}
                    }
                }

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
            NamedKey::ArrowUp => b"\x1b[A".to_vec(),
            NamedKey::ArrowDown => b"\x1b[B".to_vec(),
            NamedKey::ArrowRight => b"\x1b[C".to_vec(),
            NamedKey::ArrowLeft => b"\x1b[D".to_vec(),
            NamedKey::Home => b"\x1b[H".to_vec(),
            NamedKey::End => b"\x1b[F".to_vec(),
            NamedKey::Delete => b"\x1b[3~".to_vec(),
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
