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
	anthropicModel = "claude-sonnet-4-20250514"

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

// New creates a new LLM client. Tries ANTHROPIC_API_KEY first, then OPENAI_API_KEY.
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
