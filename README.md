# termAI

AI-powered terminal assistant built in Go.

## 🚀 Status

Early development, with a working CLI foundation.

## 🧠 Goal

Build a developer-focused terminal that can:

- Execute commands
- Analyze terminal output
- Detect errors
- Suggest actions
- Evolve into an AI-powered assistant

## 🛠 Tech Stack

- Go
- Cobra CLI
- golangci-lint

## ✅ Current Features

- Interactive CLI loop
- Command execution with stdout, stderr and exit code capture
- Local error analysis for common terminal failures
- Contextual guidance for `nvm: command not found`
- Suggested actions with risk levels
- Confirmation before running sensitive actions
- Placeholder detection for commands that need manual editing
- Unit tests for analyzer, executor and CLI safety helpers
- Linting configuration
- GitHub Actions CI for tests, vet, race detection and linting

## 📦 Setup

```bash
go run main.go
```

Build with an injected version:

```bash
go build -ldflags "-X github.com/ViniAguiar1/termai/cmd.appVersion=v0.1.0" -o termai
```

By default, local builds fall back to Go build metadata and show versions like `dev-<commit>` automatically.

## 🧪 Tests

```bash
go test ./...
go vet ./...
golangci-lint run
```

For race detection:

```bash
go test -race ./...
```

## 🧪 Manual Testing

Run termAI:

```bash
go run main.go
```

Try a known command-not-found case:

```bash
nvm use 24
```

termAI should detect that `nvm` is not loaded in the current executor and suggest loading `~/.nvm/nvm.sh` before running `nvm use 24`.

## 📄 License

This project is licensed under the MIT License. See [LICENSE](/Users/viniciusaguiar/termai/LICENSE:1).

## 📌 Next Steps

- Replace raw stdin scanning with a readline-style input layer for history and arrow-key support
- Add OpenAI-powered analysis for unknown or complex errors
- Expand analyzer rules and safety metadata
- Improve command suggestions that require user-provided values
