package config

import (
	"os"
	"path/filepath"

	"github.com/BurntSushi/toml"
)

type AIConfig struct {
	Provider string `toml:"provider"` // "openai" or "anthropic"
	APIKey   string `toml:"api_key"`
}

type Config struct {
	AI AIConfig `toml:"ai"`
}

// Load reads ~/.config/termai/config.toml and returns the config.
func Load() Config {
	var cfg Config

	home, err := os.UserHomeDir()
	if err != nil {
		return cfg
	}

	path := filepath.Join(home, ".config", "termai", "config.toml")
	_, err = toml.DecodeFile(path, &cfg)
	if err != nil {
		return cfg
	}

	return cfg
}

// APIKey returns the API key for the given provider.
// Config takes priority over environment variables.
func (c *Config) APIKey(provider string) string {
	// If config has a key and matches the provider, use it
	if c.AI.APIKey != "" {
		if c.AI.Provider == "" || c.AI.Provider == provider {
			return c.AI.APIKey
		}
	}

	// Fall back to env vars
	switch provider {
	case "anthropic":
		return os.Getenv("ANTHROPIC_API_KEY")
	case "openai":
		return os.Getenv("OPENAI_API_KEY")
	}

	return ""
}

// Provider returns the configured provider name.
// Auto-detects from config or env vars.
func (c *Config) Provider() string {
	if c.AI.Provider != "" && c.AI.APIKey != "" {
		return c.AI.Provider
	}

	// Auto-detect from env vars
	if c.AI.APIKey != "" {
		// Key in config but no provider specified — default to openai
		return "openai"
	}

	if os.Getenv("ANTHROPIC_API_KEY") != "" {
		return "anthropic"
	}
	if os.Getenv("OPENAI_API_KEY") != "" {
		return "openai"
	}

	return ""
}
