# termAI

GPU-accelerated terminal emulator with built-in AI assistance.

## Status

Early development. The terminal emulator is functional on macOS with GPU rendering, split panes, tabs, and ANSI color support. AI integration is next.

## Goal

Build a fast, cross-platform terminal emulator that can:

- Render text on the GPU for smooth, high-performance output
- Split panes and tabs like tmux, but built-in
- Analyze terminal output and detect errors
- Suggest corrective actions powered by AI
- Run on macOS, Linux, and Windows

## Tech Stack

- **Rust** — Terminal emulator (wgpu, winit, vte, portable-pty)
- **Go** — AI engine (Cobra CLI, error analysis, future LLM integration)

## Features

- GPU-accelerated text rendering via wgpu (Metal on macOS, Vulkan on Linux, DX12 on Windows)
- JetBrains Mono embedded font with HiDPI/Retina support
- ANSI color support (16 colors, 256-color, truecolor RGB)
- Split panes (vertical/horizontal) with independent PTYs
- Tabs with tab bar and click-to-switch
- Blinking cursor with style support (block, underline, bar)
- Scrollback buffer (10k lines) with mouse wheel and keyboard scrolling
- Text selection with mouse + Cmd+C/Cmd+V clipboard
- Zoom in/out with Cmd+/Cmd-
- Alternate screen buffer (vim, htop, tmux)
- VT100/xterm escape sequence support (cursor movement, scroll regions, insert/delete lines)
- Config file (`~/.config/termai/config.toml`)

## Quick Start

```bash
cargo run --release
```

## Build

```bash
# Debug
cargo build

# Release
cargo build --release

# Run
./target/release/termai
```

## Keyboard Shortcuts

| Shortcut | Action |
|----------|--------|
| Cmd+D | Split vertical |
| Cmd+Shift+D | Split horizontal |
| Cmd+[ / Cmd+] | Navigate panes |
| Cmd+T | New tab |
| Cmd+W | Close pane/tab |
| Cmd+1-9 | Switch tabs |
| Cmd++ / Cmd+- | Zoom in/out |
| Cmd+0 | Reset zoom |
| Cmd+C | Copy selection |
| Cmd+V | Paste |
| Shift+PageUp/Down | Scroll history |

## Config

Create `~/.config/termai/config.toml`:

```toml
[font]
size = 14.0

[window]
width = 1024
height = 640
title = "termAI"

[terminal]
scrollback_lines = 10000

[theme]
background = "#121216"
foreground = "#ccccce"
```

## Tests

```bash
# Rust
cargo test

# Go (AI engine)
cd ai && go test ./...
cd ai && go test -race ./...
cd ai && go vet ./...
cd ai && golangci-lint run
```

## Architecture

```
termai/
├── crates/                     # Rust workspace
│   ├── termai-app/             # Main binary: window, event loop, tabs, panes
│   ├── termai-core/            # VT100 state machine, terminal grid
│   ├── termai-renderer/        # wgpu GPU rendering, glyph atlas
│   └── termai-pty/             # Cross-platform PTY (Unix + Windows ConPTY)
├── ai/                         # Go AI engine
│   ├── cmd/                    # CLI, session, prompt
│   └── internal/               # Analyzer, executor
└── proto/                      # IPC protocol (planned)
```

## License

MIT License. See [LICENSE](LICENSE).

## Next Steps

- IPC between Rust emulator and Go AI engine
- AI-powered error analysis with LLM integration
- Cross-platform testing (Linux, Windows)
- Multi-language support (i18n)
