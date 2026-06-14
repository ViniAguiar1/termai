package cmd

import (
	"encoding/json"
	"fmt"
	"net"
	"os"
	"os/signal"
	"syscall"

	"github.com/ViniAguiar1/termai/ai/internal/analyzer"
	"github.com/ViniAguiar1/termai/ai/internal/config"
	"github.com/ViniAguiar1/termai/ai/internal/llm"
	"github.com/spf13/cobra"
)

// Global LLM client (nil if no API key)
var llmClient *llm.Client

const defaultSocketPath = "/tmp/termai-ai.sock"

// BaseRequest extracts just the type field to route requests.
type BaseRequest struct {
	Type string `json:"type"`
}

// AnalyzeRequest is the JSON request from the Rust terminal.
type AnalyzeRequest struct {
	Type     string `json:"type"`
	Command  string `json:"command"`
	Output   string `json:"output"`
	ExitCode int    `json:"exit_code"`
}

// AutocompleteRequest is a request for command completion.
type AutocompleteRequest struct {
	Type       string `json:"type"`
	PartialCmd string `json:"partial_cmd"`
	Cwd        string `json:"cwd"`
	History    string `json:"history"`
}

// AutocompleteResponse is returned for completion requests.
type AutocompleteResponse struct {
	Type       string `json:"type"`
	Completion string `json:"completion,omitempty"`
}

// ActionResponse is a single suggested action.
type ActionResponse struct {
	Label   string `json:"label"`
	Command string `json:"command"`
	Risk    string `json:"risk"`
}

// AnalyzeResponse is the JSON response to the Rust terminal.
type AnalyzeResponse struct {
	Type        string           `json:"type"`
	Title       string           `json:"title"`
	Description string           `json:"description"`
	Actions     []ActionResponse `json:"actions"`
}

var serveCmd = &cobra.Command{
	Use:   "serve",
	Short: "Start AI engine as a Unix socket server for IPC",
	Run: func(cmd *cobra.Command, args []string) {
		socketPath, _ := cmd.Flags().GetString("socket")
		if socketPath == "" {
			socketPath = defaultSocketPath
		}

		if err := runServer(socketPath); err != nil {
			fmt.Fprintln(os.Stderr, err)
			os.Exit(1)
		}
	},
}

func init() {
	serveCmd.Flags().String("socket", defaultSocketPath, "Unix socket path")
	rootCmd.AddCommand(serveCmd)
}

func runServer(socketPath string) error {
	// Load config and initialize LLM client
	cfg := config.Load()
	provider := cfg.Provider()
	if provider != "" {
		apiKey := cfg.APIKey(provider)
		llmClient = llm.NewWithKey(provider, apiKey)
	}
	if llmClient == nil {
		llmClient = llm.New() // fall back to env vars
	}
	if llmClient != nil {
		fmt.Fprintf(os.Stderr, "LLM enabled (provider: %s)\n", llmClient.ProviderName())
	} else {
		fmt.Fprintln(os.Stderr, "LLM disabled (set api_key in ~/.config/termai/config.toml or ANTHROPIC_API_KEY/OPENAI_API_KEY env var)")
	}

	// Clean up stale socket
	_ = os.Remove(socketPath)

	listener, err := net.Listen("unix", socketPath)
	if err != nil {
		return fmt.Errorf("failed to listen on %s: %w", socketPath, err)
	}
	defer func() {
		listener.Close()
		_ = os.Remove(socketPath)
	}()

	// Handle signals for clean shutdown
	sigCh := make(chan os.Signal, 1)
	signal.Notify(sigCh, syscall.SIGINT, syscall.SIGTERM)
	go func() {
		<-sigCh
		listener.Close()
		_ = os.Remove(socketPath)
		os.Exit(0)
	}()

	fmt.Fprintf(os.Stderr, "termai-ai server listening on %s\n", socketPath)

	for {
		conn, err := listener.Accept()
		if err != nil {
			// Listener closed by signal
			return nil
		}
		go handleConnection(conn)
	}
}

func handleConnection(conn net.Conn) {
	defer conn.Close()

	decoder := json.NewDecoder(conn)
	encoder := json.NewEncoder(conn)

	for {
		var raw json.RawMessage
		if err := decoder.Decode(&raw); err != nil {
			return
		}

		var base BaseRequest
		if err := json.Unmarshal(raw, &base); err != nil {
			continue
		}

		switch base.Type {
		case "analyze":
			var req AnalyzeRequest
			if err := json.Unmarshal(raw, &req); err != nil {
				continue
			}
			handleAnalyze(encoder, req)

		case "autocomplete":
			var req AutocompleteRequest
			if err := json.Unmarshal(raw, &req); err != nil {
				continue
			}
			handleAutocomplete(encoder, req)

		case "update_check":
			var req UpdateCheckRequest
			if err := json.Unmarshal(raw, &req); err != nil {
				continue
			}
			handleUpdateCheck(encoder, req)
		}
	}
}

func handleAnalyze(encoder *json.Encoder, req AnalyzeRequest) {
	errorOutput := req.Output
	var suggestion *analyzer.Suggestion

	if llmClient != nil {
		llmSuggestion, err := llmClient.Analyze(req.Command, errorOutput)
		if err != nil {
			fmt.Fprintf(os.Stderr, "LLM error: %v\n", err)
		} else {
			suggestion = llmSuggestion
		}
	}

	if suggestion == nil {
		suggestion = analyzer.AnalyzeCommand(req.Command, errorOutput)
	}

	if suggestion == nil {
		_ = encoder.Encode(AnalyzeResponse{Type: "no_suggestion"})
		return
	}

	actions := make([]ActionResponse, 0, len(suggestion.Actions))
	for _, a := range suggestion.Actions {
		if a.Command == "" {
			continue
		}
		actions = append(actions, ActionResponse{
			Label:   a.Label,
			Command: a.Command,
			Risk:    string(a.Risk),
		})
	}

	_ = encoder.Encode(AnalyzeResponse{
		Type:        "suggestion",
		Title:       suggestion.Title,
		Description: suggestion.Description,
		Actions:     actions,
	})
}

func handleAutocomplete(encoder *json.Encoder, req AutocompleteRequest) {
	if llmClient == nil {
		_ = encoder.Encode(AutocompleteResponse{Type: "no_completion"})
		return
	}

	completion, err := llmClient.Autocomplete(req.PartialCmd, req.Cwd, req.History)
	if err != nil {
		fmt.Fprintf(os.Stderr, "Autocomplete error: %v\n", err)
		_ = encoder.Encode(AutocompleteResponse{Type: "no_completion"})
		return
	}

	if completion == "" {
		_ = encoder.Encode(AutocompleteResponse{Type: "no_completion"})
		return
	}

	_ = encoder.Encode(AutocompleteResponse{
		Type:       "completion",
		Completion: completion,
	})
}
