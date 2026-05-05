package cmd

import (
	"encoding/json"
	"fmt"
	"net"
	"os"
	"os/signal"
	"syscall"

	"github.com/ViniAguiar1/termai/ai/internal/analyzer"
	"github.com/ViniAguiar1/termai/ai/internal/llm"
	"github.com/spf13/cobra"
)

// Global LLM client (nil if no API key)
var llmClient *llm.Client

const defaultSocketPath = "/tmp/termai-ai.sock"

// AnalyzeRequest is the JSON request from the Rust terminal.
type AnalyzeRequest struct {
	Type     string `json:"type"`
	Command  string `json:"command"`
	Output   string `json:"output"`
	ExitCode int    `json:"exit_code"`
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
	// Initialize LLM client
	llmClient = llm.New()
	if llmClient != nil {
		fmt.Fprintln(os.Stderr, "LLM enabled (ANTHROPIC_API_KEY found)")
	} else {
		fmt.Fprintln(os.Stderr, "LLM disabled (set ANTHROPIC_API_KEY to enable)")
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
		var req AnalyzeRequest
		if err := decoder.Decode(&req); err != nil {
			return // Connection closed or invalid JSON
		}

		if req.Type != "analyze" {
			continue
		}

		errorOutput := req.Output
		suggestion := analyzer.AnalyzeCommand(req.Command, errorOutput)

		// Fall back to LLM if pattern matching found nothing
		if suggestion == nil && llmClient != nil {
			llmSuggestion, err := llmClient.Analyze(req.Command, errorOutput)
			if err != nil {
				fmt.Fprintf(os.Stderr, "LLM error: %v\n", err)
			} else {
				suggestion = llmSuggestion
			}
		}

		if suggestion == nil {
			resp := AnalyzeResponse{Type: "no_suggestion"}
			_ = encoder.Encode(resp)
			continue
		}

		actions := make([]ActionResponse, 0, len(suggestion.Actions))
		for _, a := range suggestion.Actions {
			if a.Command == "" {
				continue // Skip guidance-only actions
			}
			actions = append(actions, ActionResponse{
				Label:   a.Label,
				Command: a.Command,
				Risk:    string(a.Risk),
			})
		}

		resp := AnalyzeResponse{
			Type:        "suggestion",
			Title:       suggestion.Title,
			Description: suggestion.Description,
			Actions:     actions,
		}
		_ = encoder.Encode(resp)
	}
}
