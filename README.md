# termAI

GPU-accelerated terminal emulator with built-in AI assistance.

## Status

Active development. The terminal emulator is functional on macOS with GPU rendering, split panes, tabs, scrollback search, instant autocomplete, a native menu bar, and AI-powered error analysis (Claude / OpenAI).

## Goal

Build a fast, cross-platform terminal emulator that can:

- Render text on the GPU for smooth, high-performance output
- Split panes and tabs like tmux, but built-in
- Analyze terminal output and detect errors automatically
- Suggest corrective actions powered by LLM (Claude or OpenAI)
- Fall back to offline pattern matching when no API key is configured
- Run on macOS, Linux, and Windows

## Tech Stack

- **Rust** — Terminal emulator (wgpu, winit, vte, portable-pty)
- **Go** — AI engine (Cobra CLI, error analysis, Claude/OpenAI LLM integration)

## Features

- GPU-accelerated text rendering via wgpu (Metal on macOS, Vulkan on Linux, DX12 on Windows)
- JetBrains Mono embedded font with HiDPI/Retina support
- ANSI color support (16 colors, 256-color, truecolor RGB)
- Split panes (vertical/horizontal) with independent PTYs
- Tabs with tab bar, click-to-switch, and new-tab / split buttons in the strip
- Working directory and git branch shown in the tab strip
- Native macOS menu bar (App, Terminal, Edit, View, AI, Window, Help)
- Instant autocomplete: history-based ghost text (local, zero-latency), with an LLM fallback — accept with Tab or →
- Clickable URLs (Cmd+click to open)
- Launches a login shell (sources `~/.zprofile`, etc.), matching Terminal.app
- Blinking cursor with style support (block, underline, bar)
- Scrollback buffer (10k lines) with mouse wheel and keyboard scrolling
- Text selection with mouse + Cmd+C/Cmd+V clipboard
- Zoom in/out with Cmd+/Cmd-
- Alternate screen buffer (vim, htop, tmux)
- VT100/xterm escape sequence support (cursor movement, scroll regions, insert/delete lines)
- Search in scrollback (Cmd+F) with match highlighting and navigation
- PTY resize notification (programs re-render correctly on split/resize)
- AI-powered error analysis via IPC (Rust ↔ Go over Unix socket)
- LLM support (Claude / OpenAI) with offline pattern matching fallback
- Config file (`~/.config/termai/config.toml`)

## Quick Start

```bash
./build.sh
./target/release/termai
```

## Build

```bash
# Build both Rust terminal + Go AI engine
./build.sh

# Or build separately:
cargo build --release
cd ai && go build -o ../target/release/termai-ai .
```

## AI Setup

The AI engine works offline with pattern matching for common errors. For full LLM-powered analysis, set one of:

```bash
# Anthropic (recommended)
export ANTHROPIC_API_KEY="sk-ant-..."

# Or OpenAI
export OPENAI_API_KEY="sk-..."
```

Alternatively, set the provider and key in the config file (see the `[ai]`
section under [Config](#config)). Error analysis uses Claude Sonnet; the
instant autocomplete uses Claude Haiku for low latency.

When a command fails, the AI overlay appears automatically with suggestions. Press 1-9 to execute an action, Escape to dismiss.

## Keyboard Shortcuts

| Shortcut | Action |
|----------|--------|
| Cmd+D | Split vertical |
| Cmd+Shift+D | Split horizontal |
| Cmd+[ / Cmd+] | Navigate panes |
| Cmd+T | New tab |
| Cmd+W | Close pane/tab |
| Cmd+1-9 | Switch tabs |
| Tab / → | Accept autocomplete suggestion |
| Cmd++ / Cmd+- | Zoom in/out |
| Cmd+0 | Reset zoom |
| Cmd+C | Copy selection |
| Cmd+V | Paste |
| Shift+PageUp/Down | Scroll history |
| Cmd+F | Search in scrollback |
| Cmd+G | Next search match |
| Cmd+Shift+G | Previous search match |
| Escape | Close search / dismiss AI overlay |

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

# Optional — AI provider/key (an alternative to the env vars above).
[ai]
provider = "anthropic"   # "anthropic" or "openai"
api_key = "sk-ant-..."
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
│   ├── cmd/                    # CLI, session, prompt, IPC server
│   └── internal/               # Analyzer, executor, LLM client
└── build.sh                    # Build script (Rust + Go)
```

## License

MIT License. See [LICENSE](LICENSE).

## Next Steps

- Cross-platform testing (Linux, Windows)
- Themes support (Dracula, Catppuccin, etc.)
- Ligatures and Nerd Font icons
