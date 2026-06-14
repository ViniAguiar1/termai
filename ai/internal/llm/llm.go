package llm

import (
	"bytes"
	"encoding/json"
	"fmt"
	"io"
	"net/http"
	"os"
	"strings"
	"time"

	"github.com/ViniAguiar1/termai/ai/internal/analyzer"
)

const (
	anthropicURL   = "https://api.anthropic.com/v1/messages"
	// Sonnet 4.6 — best speed/intelligence balance, used for error analysis.
	// (The old claude-sonnet-4-20250514 was retired → 404.)
	anthropicModel = "claude-sonnet-4-6"
	// Haiku 4.5 — fastest model, used for autocomplete where latency matters
	// most (ghost text should feel near-instant).
	anthropicFastModel = "claude-haiku-4-5"

	openaiURL   = "https://api.openai.com/v1/chat/completions"
	openaiModel = "gpt-4o-mini"

	maxTokens = 1024
	timeout   = 15 * time.Second
)

// Provider identifies which LLM backend is in use.
type Provider string

const (
	ProviderAnthropic Provider = "anthropic"
	ProviderOpenAI    Provider = "openai"
)

const systemPrompt = `You are an AI assistant embedded in a terminal emulator called termAI. Your job is to analyze command errors and suggest fixes.

When the user runs a command that fails, you receive the command and error output. You must respond with a JSON object containing:
- "title": short title describing the problem (in Portuguese, pt-BR)
- "description": brief explanation of what went wrong (in Portuguese, pt-BR)
- "actions": array of suggested fixes, each with:
  - "label": what the action does (in Portuguese, pt-BR)
  - "command": the shell command to run (empty string if it's just guidance)
  - "risk": "low", "medium", or "high"

Rules:
- Always respond in Portuguese (pt-BR)
- Only suggest commands that are safe and relevant
- Mark destructive commands (rm, kill, sudo) as "high" risk
- Mark install/modify commands as "medium" risk
- Mark read-only/diagnostic commands as "low" risk
- Provide 1-4 actions maximum
- If the error is trivial (typo), suggest the corrected command
- ONLY output valid JSON, no markdown, no explanation outside the JSON

Example response:
{"title":"Comando não encontrado","description":"O comando 'gi' não existe. Você quis dizer 'git'?","actions":[{"label":"Executar o comando correto","command":"git status","risk":"low"}]}`

// Client handles LLM API calls.
type Client struct {
	provider   Provider
	apiKey     string
	httpClient *http.Client
}

// NewWithKey creates a client with an explicit provider and key.
func NewWithKey(provider, apiKey string) *Client {
	if apiKey == "" {
		return nil
	}

	var p Provider
	switch provider {
	case "anthropic":
		p = ProviderAnthropic
	default:
		p = ProviderOpenAI
	}

	return &Client{
		provider:   p,
		apiKey:     apiKey,
		httpClient: &http.Client{Timeout: timeout},
	}
}

// New creates a new LLM client from environment variables.
// Tries ANTHROPIC_API_KEY first, then OPENAI_API_KEY.
// Returns nil if no API key is configured.
func New() *Client {
	if key := os.Getenv("ANTHROPIC_API_KEY"); key != "" {
		return &Client{
			provider:   ProviderAnthropic,
			apiKey:     key,
			httpClient: &http.Client{Timeout: timeout},
		}
	}

	if key := os.Getenv("OPENAI_API_KEY"); key != "" {
		return &Client{
			provider:   ProviderOpenAI,
			apiKey:     key,
			httpClient: &http.Client{Timeout: timeout},
		}
	}

	return nil
}

// Provider returns which provider is active.
func (c *Client) ProviderName() string {
	return string(c.provider)
}

// Analyze sends the command and error to the LLM and returns a suggestion.
func (c *Client) Analyze(command, errorOutput string) (*analyzer.Suggestion, error) {
	switch c.provider {
	case ProviderAnthropic:
		return c.analyzeAnthropic(command, errorOutput)
	case ProviderOpenAI:
		return c.analyzeOpenAI(command, errorOutput)
	default:
		return nil, fmt.Errorf("unknown provider: %s", c.provider)
	}
}

func (c *Client) analyzeAnthropic(command, errorOutput string) (*analyzer.Suggestion, error) {
	userMessage := fmt.Sprintf("Command: %s\nError output:\n%s", command, errorOutput)

	reqBody := map[string]interface{}{
		"model":      anthropicModel,
		"max_tokens": maxTokens,
		"system":     systemPrompt,
		"messages": []map[string]string{
			{"role": "user", "content": userMessage},
		},
	}

	bodyBytes, err := json.Marshal(reqBody)
	if err != nil {
		return nil, fmt.Errorf("marshal request: %w", err)
	}

	req, err := http.NewRequest("POST", anthropicURL, bytes.NewReader(bodyBytes))
	if err != nil {
		return nil, fmt.Errorf("create request: %w", err)
	}

	req.Header.Set("Content-Type", "application/json")
	req.Header.Set("x-api-key", c.apiKey)
	req.Header.Set("anthropic-version", "2023-06-01")

	resp, err := c.httpClient.Do(req)
	if err != nil {
		return nil, fmt.Errorf("api call: %w", err)
	}
	defer resp.Body.Close()

	respBody, err := io.ReadAll(resp.Body)
	if err != nil {
		return nil, fmt.Errorf("read response: %w", err)
	}

	if resp.StatusCode != 200 {
		return nil, fmt.Errorf("api error %d: %s", resp.StatusCode, string(respBody))
	}

	return parseAnthropicResponse(respBody)
}

func (c *Client) analyzeOpenAI(command, errorOutput string) (*analyzer.Suggestion, error) {
	userMessage := fmt.Sprintf("Command: %s\nError output:\n%s", command, errorOutput)

	reqBody := map[string]interface{}{
		"model":      openaiModel,
		"max_tokens": maxTokens,
		"messages": []map[string]interface{}{
			{"role": "system", "content": systemPrompt},
			{"role": "user", "content": userMessage},
		},
	}

	bodyBytes, err := json.Marshal(reqBody)
	if err != nil {
		return nil, fmt.Errorf("marshal request: %w", err)
	}

	req, err := http.NewRequest("POST", openaiURL, bytes.NewReader(bodyBytes))
	if err != nil {
		return nil, fmt.Errorf("create request: %w", err)
	}

	req.Header.Set("Content-Type", "application/json")
	req.Header.Set("Authorization", "Bearer "+c.apiKey)

	resp, err := c.httpClient.Do(req)
	if err != nil {
		return nil, fmt.Errorf("api call: %w", err)
	}
	defer resp.Body.Close()

	respBody, err := io.ReadAll(resp.Body)
	if err != nil {
		return nil, fmt.Errorf("read response: %w", err)
	}

	if resp.StatusCode != 200 {
		return nil, fmt.Errorf("api error %d: %s", resp.StatusCode, string(respBody))
	}

	return parseOpenAIResponse(respBody)
}

const autocompleteSystemPrompt = `You are a shell command autocomplete engine. Suggest the most likely completion for the user's partial command.

Rules:
- Return ONLY the suffix to append to the partial command. No prefix, no markdown, no quotes, no explanation.
- The "Recent commands" context may include error messages or output from past commands — IGNORE errors there. Past failures do not mean the user wants something different now; they are still typing.
- Always try to suggest something plausible based on the partial command alone. Common commands (git, npm, docker, cd, ls, etc.) have well-known completions.
- Only return an empty string if the partial command is gibberish that cannot be reasonably completed.
- Examples (partial → completion):
  "git ch" → "eckout"
  "git st" → "atus"
  "npm i" → "nstall"
  "docker p" → "s"`

// Autocomplete sends a partial command to the LLM and returns the completion text.
func (c *Client) Autocomplete(partialCmd, cwd, history string) (string, error) {
	switch c.provider {
	case ProviderAnthropic:
		return c.autocompleteAnthropic(partialCmd, cwd, history)
	case ProviderOpenAI:
		return c.autocompleteOpenAI(partialCmd, cwd, history)
	default:
		return "", fmt.Errorf("unknown provider: %s", c.provider)
	}
}

func (c *Client) autocompleteAnthropic(partialCmd, cwd, history string) (string, error) {
	userMessage := fmt.Sprintf("Working directory: %s\nRecent commands:\n%s\n\nPartial command: %s", cwd, history, partialCmd)

	reqBody := map[string]interface{}{
		"model":      anthropicFastModel,
		"max_tokens": 100,
		"system":     autocompleteSystemPrompt,
		"messages": []map[string]string{
			{"role": "user", "content": userMessage},
		},
	}

	bodyBytes, err := json.Marshal(reqBody)
	if err != nil {
		return "", fmt.Errorf("marshal request: %w", err)
	}

	req, err := http.NewRequest("POST", anthropicURL, bytes.NewReader(bodyBytes))
	if err != nil {
		return "", fmt.Errorf("create request: %w", err)
	}

	req.Header.Set("Content-Type", "application/json")
	req.Header.Set("x-api-key", c.apiKey)
	req.Header.Set("anthropic-version", "2023-06-01")

	resp, err := c.httpClient.Do(req)
	if err != nil {
		return "", fmt.Errorf("api call: %w", err)
	}
	defer resp.Body.Close()

	respBody, err := io.ReadAll(resp.Body)
	if err != nil {
		return "", fmt.Errorf("read response: %w", err)
	}

	if resp.StatusCode != 200 {
		return "", fmt.Errorf("api error %d: %s", resp.StatusCode, string(respBody))
	}

	return parseAnthropicText(respBody)
}

func (c *Client) autocompleteOpenAI(partialCmd, cwd, history string) (string, error) {
	userMessage := fmt.Sprintf("Working directory: %s\nRecent commands:\n%s\n\nPartial command: %s", cwd, history, partialCmd)

	reqBody := map[string]interface{}{
		"model":      openaiModel,
		"max_tokens": 100,
		"temperature": 0,
		"messages": []map[string]interface{}{
			{"role": "system", "content": autocompleteSystemPrompt},
			{"role": "user", "content": userMessage},
		},
	}

	bodyBytes, err := json.Marshal(reqBody)
	if err != nil {
		return "", fmt.Errorf("marshal request: %w", err)
	}

	req, err := http.NewRequest("POST", openaiURL, bytes.NewReader(bodyBytes))
	if err != nil {
		return "", fmt.Errorf("create request: %w", err)
	}

	req.Header.Set("Content-Type", "application/json")
	req.Header.Set("Authorization", "Bearer "+c.apiKey)

	resp, err := c.httpClient.Do(req)
	if err != nil {
		return "", fmt.Errorf("api call: %w", err)
	}
	defer resp.Body.Close()

	respBody, err := io.ReadAll(resp.Body)
	if err != nil {
		return "", fmt.Errorf("read response: %w", err)
	}

	if resp.StatusCode != 200 {
		return "", fmt.Errorf("api error %d: %s", resp.StatusCode, string(respBody))
	}

	return parseOpenAIText(respBody)
}

func parseAnthropicText(body []byte) (string, error) {
	var apiResp struct {
		Content []struct {
			Type string `json:"type"`
			Text string `json:"text"`
		} `json:"content"`
	}
	if err := json.Unmarshal(body, &apiResp); err != nil {
		return "", fmt.Errorf("parse response: %w", err)
	}
	for _, block := range apiResp.Content {
		if block.Type == "text" {
			return strings.TrimSpace(block.Text), nil
		}
	}
	return "", nil
}

func parseOpenAIText(body []byte) (string, error) {
	var apiResp struct {
		Choices []struct {
			Message struct {
				Content string `json:"content"`
			} `json:"message"`
		} `json:"choices"`
	}
	if err := json.Unmarshal(body, &apiResp); err != nil {
		return "", fmt.Errorf("parse response: %w", err)
	}
	if len(apiResp.Choices) > 0 {
		return strings.TrimSpace(apiResp.Choices[0].Message.Content), nil
	}
	return "", nil
}

func parseAnthropicResponse(body []byte) (*analyzer.Suggestion, error) {
	var apiResp struct {
		Content []struct {
			Type string `json:"type"`
			Text string `json:"text"`
		} `json:"content"`
	}

	if err := json.Unmarshal(body, &apiResp); err != nil {
		return nil, fmt.Errorf("parse api response: %w", err)
	}

	if len(apiResp.Content) == 0 {
		return nil, fmt.Errorf("empty response")
	}

	text := ""
	for _, block := range apiResp.Content {
		if block.Type == "text" {
			text = block.Text
			break
		}
	}

	return parseSuggestionJSON(text)
}

func parseOpenAIResponse(body []byte) (*analyzer.Suggestion, error) {
	var apiResp struct {
		Choices []struct {
			Message struct {
				Content string `json:"content"`
			} `json:"message"`
		} `json:"choices"`
	}

	if err := json.Unmarshal(body, &apiResp); err != nil {
		return nil, fmt.Errorf("parse api response: %w", err)
	}

	if len(apiResp.Choices) == 0 {
		return nil, fmt.Errorf("empty response")
	}

	return parseSuggestionJSON(apiResp.Choices[0].Message.Content)
}

func parseSuggestionJSON(text string) (*analyzer.Suggestion, error) {
	if text == "" {
		return nil, fmt.Errorf("no text in response")
	}

	// Strip markdown code fences if present
	text = strings.TrimSpace(text)
	text = strings.TrimPrefix(text, "```json")
	text = strings.TrimPrefix(text, "```")
	text = strings.TrimSuffix(text, "```")
	text = strings.TrimSpace(text)

	var result struct {
		Title       string `json:"title"`
		Description string `json:"description"`
		Actions     []struct {
			Label   string `json:"label"`
			Command string `json:"command"`
			Risk    string `json:"risk"`
		} `json:"actions"`
	}

	if err := json.Unmarshal([]byte(text), &result); err != nil {
		return nil, fmt.Errorf("parse suggestion json: %w (raw: %s)", err, text)
	}

	suggestion := &analyzer.Suggestion{
		Title:       result.Title,
		Description: result.Description,
	}

	for _, a := range result.Actions {
		risk := analyzer.RiskLevel(a.Risk)
		if risk != analyzer.RiskLow && risk != analyzer.RiskMedium && risk != analyzer.RiskHigh {
			risk = analyzer.RiskLow
		}

		suggestion.Actions = append(suggestion.Actions, analyzer.Action{
			Label:                a.Label,
			Command:              a.Command,
			Risk:                 risk,
			RequiresConfirmation: risk == analyzer.RiskMedium || risk == analyzer.RiskHigh,
		})
	}

	return suggestion, nil
}
