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
    /// Top-level shortcut: if set, applied to `normal` (and inherited by the
    /// other variants) when no per-variant family is given. Lets a user write
    /// `[font] family = "..."` without spelling out every variant.
    pub family: Option<String>,
    pub normal: FontVariant,
    pub bold: FontVariant,
    pub italic: FontVariant,
    pub bold_italic: FontVariant,
}

#[derive(Deserialize, Debug, Default, Clone)]
#[serde(default)]
pub struct FontVariant {
    /// Family name passed to the system font matcher (e.g. "JetBrainsMono Nerd Font").
    pub family: Option<String>,
    /// Style hint forwarded to the matcher (e.g. "Regular", "Bold", "Italic").
    pub style: Option<String>,
}

impl FontConfig {
    /// Resolve the effective family for the regular weight, falling back from
    /// `font.normal.family` to the top-level `font.family` shortcut.
    pub fn normal_family(&self) -> Option<&str> {
        self.normal
            .family
            .as_deref()
            .or(self.family.as_deref())
    }
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
        Self {
            size: 14.0,
            family: None,
            normal: FontVariant::default(),
            bold: FontVariant::default(),
            italic: FontVariant::default(),
            bold_italic: FontVariant::default(),
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
