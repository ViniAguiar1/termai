use serde::Deserialize;
use std::path::PathBuf;

#[derive(Deserialize, Debug)]
#[serde(default)]
pub struct Config {
    pub font: FontConfig,
    pub window: WindowConfig,
    pub terminal: TerminalConfig,
    pub theme: ThemeConfig,
    pub cursor: CursorConfig,
    pub keys: KeysConfig,
}

#[derive(Deserialize, Debug)]
#[serde(default)]
pub struct KeysConfig {
    /// tmux-style split leader, e.g. "ctrl+b". Followed by `|` (vertical) or
    /// `-` (horizontal). Only `ctrl+<char>` is supported.
    pub leader: String,
}

impl Default for KeysConfig {
    fn default() -> Self {
        Self { leader: "ctrl+b".to_string() }
    }
}

impl KeysConfig {
    /// The leader's character (the part after `ctrl+`), lowercased. Defaults to 'b'.
    pub fn leader_char(&self) -> char {
        self.leader
            .rsplit('+')
            .next()
            .and_then(|s| s.trim().chars().next())
            .unwrap_or('b')
            .to_ascii_lowercase()
    }

    /// The control byte the leader maps to (e.g. ctrl+b → 0x02), for pass-through.
    pub fn leader_byte(&self) -> u8 {
        (self.leader_char() as u8) & 0x1f
    }
}

#[derive(Deserialize, Debug)]
#[serde(default)]
pub struct FontConfig {
    pub size: f32,
    /// Optional font family name (e.g. "JetBrainsMono Nerd Font").
    /// If not set, uses the embedded JetBrains Mono.
    pub family: Option<String>,
}

#[derive(Deserialize, Debug)]
#[serde(default)]
pub struct WindowConfig {
    pub width: u32,
    pub height: u32,
    pub title: String,
}

#[derive(Deserialize, Debug)]
#[serde(default)]
pub struct TerminalConfig {
    pub shell: Option<String>,
    pub scrollback_lines: usize,
}

#[derive(Deserialize, Debug)]
#[serde(default)]
pub struct ThemeConfig {
    /// Built-in theme name (e.g. "dracula", "catppuccin-mocha", "nord").
    pub name: String,
    pub background: String,
    pub foreground: String,
}

#[derive(Deserialize, Debug)]
#[serde(default)]
pub struct CursorConfig {
    pub style: String,
}

impl Default for CursorConfig {
    fn default() -> Self {
        Self { style: "block".to_string() }
    }
}

impl Default for Config {
    fn default() -> Self {
        Self {
            font: FontConfig::default(),
            window: WindowConfig::default(),
            terminal: TerminalConfig::default(),
            theme: ThemeConfig::default(),
            cursor: CursorConfig::default(),
            keys: KeysConfig::default(),
        }
    }
}

impl Default for FontConfig {
    fn default() -> Self {
        Self {
            size: 14.0,
            family: None,
        }
    }
}

impl Default for WindowConfig {
    fn default() -> Self {
        Self {
            width: 1024,
            height: 640,
            title: "termAI".to_string(),
        }
    }
}

impl Default for TerminalConfig {
    fn default() -> Self {
        Self {
            shell: None,
            scrollback_lines: 10_000,
        }
    }
}

impl Default for ThemeConfig {
    fn default() -> Self {
        Self {
            name: "default".to_string(),
            background: String::new(),
            foreground: String::new(),
        }
    }
}

impl Config {
    /// Load config from ~/.config/termai/config.toml, falling back to defaults.
    pub fn load() -> Self {
        let path = config_path();
        match std::fs::read_to_string(&path) {
            Ok(contents) => match toml::from_str(&contents) {
                Ok(config) => config,
                Err(e) => {
                    log::warn!("Failed to parse config at {}: {e}", path.display());
                    Config::default()
                }
            },
            Err(_) => Config::default(),
        }
    }
}

fn config_path() -> PathBuf {
    dirs::config_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join("termai")
        .join("config.toml")
}

impl ThemeConfig {
    /// Resolve this config into a concrete Theme.
    /// Starts from the named built-in theme, then applies any explicit
    /// background/foreground overrides.
    pub fn resolve(&self) -> crate::colors::Theme {
        let mut theme = crate::colors::theme_by_name(&self.name)
            .cloned()
            .unwrap_or_else(|| {
                if self.name != "default" {
                    log::warn!("Unknown theme '{}', falling back to default", self.name);
                }
                crate::colors::DEFAULT.clone()
            });

        if !self.background.is_empty() {
            if let Some(c) = parse_hex_color(&self.background) {
                theme.bg = c;
            }
        }
        if !self.foreground.is_empty() {
            if let Some(c) = parse_hex_color(&self.foreground) {
                theme.fg = c;
            }
        }

        theme
    }
}

/// Parse a hex color string (#RRGGBB) to [f32; 4].
pub fn parse_hex_color(hex: &str) -> Option<[f32; 4]> {
    let hex = hex.trim_start_matches('#');
    if hex.len() != 6 {
        return None;
    }
    let r = u8::from_str_radix(&hex[0..2], 16).ok()? as f32 / 255.0;
    let g = u8::from_str_radix(&hex[2..4], 16).ok()? as f32 / 255.0;
    let b = u8::from_str_radix(&hex[4..6], 16).ok()? as f32 / 255.0;
    Some([r, g, b, 1.0])
}
