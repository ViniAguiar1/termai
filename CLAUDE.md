# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Project Overview

termAI is an AI-powered terminal assistant built in Go. It runs an interactive shell session that executes commands, detects errors in output, and suggests corrective actions with risk levels and confirmation prompts.

## Commands

```bash
# Run
go run main.go

# Build with version injection
go build -ldflags "-X github.com/ViniAguiar1/termai/cmd.appVersion=v0.1.0" -o termai

# Test
go test ./...
go test -race ./...

# Single test
go test ./internal/analyzer/ -run TestAnalyze

# Lint
golangci-lint run

# Vet
go vet ./...
```

## Architecture

The app is a Cobra CLI with a single root command that starts an interactive REPL session.

- **`cmd/`** — CLI layer: session loop, prompt rendering, user interaction
  - `session.go` — Main REPL loop: reads input, executes commands, displays suggestions
  - `prompt.go` — Builds the shell prompt with git branch/status info
  - `root.go` — Cobra root command, action confirmation helpers, risk labeling
  - `input.go` — `lineReader` interface (wraps `chzyer/readline`)
  - `version.go` — Version string logic (supports ldflags injection, falls back to Go build info)
- **`internal/executor/`** — Command execution with three modes:
  - `Run()` — Simple stdout/stderr capture
  - `RunWithPTY()` — PTY-based execution for interactive commands
  - `RunAndUpdateSession()` — Captures post-execution env vars to persist session state (e.g., after `nvm use`)
- **`internal/analyzer/`** — Pattern-matching error analyzer that returns `Suggestion` structs with labeled `Action`s (command, risk level, confirmation requirement)

## Key Design Decisions

- Commands run via `sh -c` with a merged environment (OS env + session-accumulated env vars)
- Session env is updated by parsing null-delimited `env -0` output after command execution
- The analyzer is rule-based (string matching on stderr/output), not AI-powered yet
- UI text is in Portuguese (Brazilian)
- Actions have `Risk` levels (low/medium/high) that gate confirmation prompts
- Actions with `<placeholder>` tokens are shown but not auto-executed

## CI

GitHub Actions runs on push to main and PRs: `go vet`, `go test`, `go test -race`, and `golangci-lint` (v2.11). Linters enabled: govet, errcheck, staticcheck, unused, ineffassign.
