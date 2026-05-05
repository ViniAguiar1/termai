# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Project Overview

termAI is a GPU-accelerated terminal emulator with built-in AI assistance. The emulator is written in Rust (wgpu + winit) and the AI engine is in Go. They communicate via IPC.

## Commands

```bash
# Rust (terminal emulator)
cargo check                       # Type-check all crates
cargo build                       # Build debug binary
cargo build --release             # Build release binary
cargo test                        # Run all Rust tests
cargo test -p termai-core         # Run tests for a specific crate
cargo run                         # Run the terminal emulator

# Go (AI engine, from ai/ directory)
cd ai && go build ./...
cd ai && go test ./...
cd ai && go test -race ./...
cd ai && go test ./internal/analyzer/ -run TestAnalyze   # Single test
cd ai && go vet ./...
cd ai && golangci-lint run

# Go build with version injection
cd ai && go build -ldflags "-X github.com/ViniAguiar1/termai/ai/cmd.appVersion=v0.1.0" -o termai-ai
```

## Architecture

### Rust — Terminal Emulator (`crates/`)

A workspace of 4 crates:

- **`termai-app`** — Main binary: window creation (winit), event loop, orchestrates all other crates
- **`termai-core`** — VT100/xterm state machine using the `vte` crate. Maintains a grid of `Cell`s with cursor position. Implements `vte::Perform` for escape sequence handling
- **`termai-renderer`** — GPU text rendering via `wgpu` (placeholder, to be implemented)
- **`termai-pty`** — Cross-platform PTY management using `portable-pty`. Spawns shell, provides read/write interface

### Go — AI Engine (`ai/`)

- **`ai/cmd/`** — CLI layer: session loop, prompt rendering, user interaction
  - `session.go` — REPL loop: reads input, executes commands, displays suggestions
  - `prompt.go` — Shell prompt with git branch/status info
  - `root.go` — Cobra root command, action confirmation, risk labeling
  - `input.go` — `lineReader` interface (wraps `chzyer/readline`)
  - `version.go` — Version string (ldflags injection + Go build info fallback)
- **`ai/internal/executor/`** — Command execution (simple, PTY, env-capturing modes)
- **`ai/internal/analyzer/`** — Rule-based error pattern matching, returns suggestions with risk-labeled actions

### IPC (`proto/`)

Communication protocol between Rust emulator and Go AI engine (to be defined).

## Key Design Decisions

- Terminal rendering will use wgpu for GPU-accelerated text (glyph atlas approach)
- PTY is cross-platform via `portable-pty` (Unix pty + Windows ConPTY)
- VT parser uses the `vte` crate (zero-copy, state-machine based)
- Go AI engine is rule-based for now, will integrate LLM APIs for complex errors
- UI text in the AI engine is in Portuguese (Brazilian)
- Actions have `Risk` levels (low/medium/high) that gate confirmation prompts

## CI

GitHub Actions runs on push to main and PRs: `go vet`, `go test`, `go test -race`, and `golangci-lint` (v2.11). Linters enabled: govet, errcheck, staticcheck, unused, ineffassign.
