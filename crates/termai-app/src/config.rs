use serde::Deserialize;
use std::path::PathBuf;

#[derive(Deserialize, Debug)]
#[serde(default)]
pub struct Config {
    pub font: FontConfig,
    pub window: WindowConfig,
    pub terminal: TerminalConfig,
    pub theme: ThemeConfig,
}

#[derive(Deserialize, Debug)]
#[serde(default)]
pub struct FontConfig {
    pub size: f32,
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

impl Default for Config {
    fn default() -> Self {
        Self {
            font: FontConfig::default(),
            window: WindowConfig::default(),
            terminal: TerminalConfig::default(),
            theme: ThemeConfig::default(),
        }
    }
}

impl Default for FontConfig {
    fn default() -> Self {
        Self { size: 14.0 }
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
