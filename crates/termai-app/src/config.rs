use serde::Deserialize;
use std::path::{Path, PathBuf};

#[derive(Deserialize, Debug)]
#[serde(default)]
pub struct Config {
    pub general: GeneralConfig,
    pub font: FontConfig,
    pub window: WindowConfig,
    pub terminal: TerminalConfig,
    pub theme: ThemeConfig,
}

#[derive(Deserialize, Debug, Default)]
#[serde(default)]
pub struct GeneralConfig {
    /// Paths to additional TOML files to merge underneath this one. Imports
    /// are loaded first, then the main file overrides them. Useful for
    /// distributing themes as standalone files.
    pub import: Vec<PathBuf>,
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
            general: GeneralConfig::default(),
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
    /// Honors `[general] import` by recursively merging the listed files
    /// underneath the main config (the main file wins on conflicts).
    pub fn load() -> Self {
        let path = config_path();
        match load_with_imports(&path) {
            Ok(value) => match value.try_into::<Config>() {
                Ok(config) => config,
                Err(e) => {
                    log::warn!("Failed to parse config at {}: {e}", path.display());
                    Config::default()
                }
            },
            Err(LoadError::ReadFailed) => Config::default(),
            Err(LoadError::ParseFailed(e)) => {
                log::warn!("Failed to parse config at {}: {e}", path.display());
                Config::default()
            }
        }
    }
}

enum LoadError {
    ReadFailed,
    ParseFailed(toml::de::Error),
}

/// Read a TOML file, recursively follow `[general] import = [...]`, and
/// produce a single merged `toml::Value`. Imports are layered first, then
/// the file at `path` is merged on top so its values win.
fn load_with_imports(path: &Path) -> Result<toml::Value, LoadError> {
    let mut visited = Vec::new();
    load_recursive(path, &mut visited)
}

fn load_recursive(path: &Path, visited: &mut Vec<PathBuf>) -> Result<toml::Value, LoadError> {
    let canonical = path.canonicalize().unwrap_or_else(|_| path.to_path_buf());
    if visited.iter().any(|p| p == &canonical) {
        log::warn!("Skipping cyclic config import: {}", path.display());
        return Ok(toml::Value::Table(toml::Table::new()));
    }
    visited.push(canonical);

    let contents = std::fs::read_to_string(path).map_err(|_| LoadError::ReadFailed)?;
    let mut value: toml::Value = toml::from_str(&contents).map_err(LoadError::ParseFailed)?;

    let imports: Vec<PathBuf> = value
        .get("general")
        .and_then(|g| g.get("import"))
        .and_then(|i| i.as_array())
        .map(|arr| {
            arr.iter()
                .filter_map(|v| v.as_str().map(expand_path))
                .collect()
        })
        .unwrap_or_default();

    if imports.is_empty() {
        return Ok(value);
    }

    let parent = path.parent().unwrap_or(Path::new("."));
    let mut base = toml::Value::Table(toml::Table::new());
    for import_path in imports {
        let resolved = if import_path.is_absolute() {
            import_path
        } else {
            parent.join(import_path)
        };
        match load_recursive(&resolved, visited) {
            Ok(imported) => merge_toml(&mut base, imported),
            Err(LoadError::ReadFailed) => {
                log::warn!("Config import not found: {}", resolved.display());
            }
            Err(LoadError::ParseFailed(e)) => {
                log::warn!("Failed to parse imported config {}: {e}", resolved.display());
            }
        }
    }

    merge_toml(&mut base, value);
    Ok(base)
}

/// Expand a leading `~` to the user's home directory.
fn expand_path(s: &str) -> PathBuf {
    if let Some(rest) = s.strip_prefix("~/") {
        if let Some(home) = dirs::home_dir() {
            return home.join(rest);
        }
    }
    PathBuf::from(s)
}

/// Recursively merge `overlay` into `base`. Tables are merged key-by-key;
/// any other value type in the overlay replaces the corresponding entry in
/// the base. The overlay always wins on leaf conflicts.
pub(crate) fn merge_toml(base: &mut toml::Value, overlay: toml::Value) {
    match (base, overlay) {
        (toml::Value::Table(base_tbl), toml::Value::Table(overlay_tbl)) => {
            for (key, overlay_val) in overlay_tbl {
                match base_tbl.get_mut(&key) {
                    Some(base_val) => merge_toml(base_val, overlay_val),
                    None => {
                        base_tbl.insert(key, overlay_val);
                    }
                }
            }
        }
        (slot, overlay_val) => {
            *slot = overlay_val;
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

#[cfg(test)]
mod tests {
    use super::*;

    fn parse(s: &str) -> toml::Value {
        toml::from_str(s).unwrap()
    }

    #[test]
    fn merge_overlay_wins_on_leaves() {
        let mut base = parse(r#"name = "base""#);
        let overlay = parse(r#"name = "overlay""#);
        merge_toml(&mut base, overlay);
        assert_eq!(base["name"].as_str(), Some("overlay"));
    }

    #[test]
    fn merge_keeps_base_keys_not_in_overlay() {
        let mut base = parse(r#"a = 1
b = 2"#);
        let overlay = parse(r#"b = 99"#);
        merge_toml(&mut base, overlay);
        assert_eq!(base["a"].as_integer(), Some(1));
        assert_eq!(base["b"].as_integer(), Some(99));
    }

    #[test]
    fn merge_recurses_into_tables() {
        let mut base = parse(r##"
[colors.primary]
background = "#000000"
foreground = "#ffffff"
"##);
        let overlay = parse(r##"
[colors.primary]
background = "#1d1f21"
"##);
        merge_toml(&mut base, overlay);
        assert_eq!(base["colors"]["primary"]["background"].as_str(), Some("#1d1f21"));
        assert_eq!(base["colors"]["primary"]["foreground"].as_str(), Some("#ffffff"));
    }

    #[test]
    fn merge_overlay_replaces_arrays_outright() {
        // Arrays are leaf values for our purposes; replacing rather than
        // concatenating matches Alacritty's documented behavior and avoids
        // surprising "everything appended forever" semantics.
        let mut base = parse(r#"import = ["a.toml"]"#);
        let overlay = parse(r#"import = ["b.toml", "c.toml"]"#);
        merge_toml(&mut base, overlay);
        let arr = base["import"].as_array().unwrap();
        assert_eq!(arr.len(), 2);
        assert_eq!(arr[0].as_str(), Some("b.toml"));
    }

    #[test]
    fn config_loads_imports_and_main_overrides() {
        let dir = tempdir();
        let theme_path = dir.join("theme.toml");
        let main_path = dir.join("config.toml");

        std::fs::write(&theme_path, r##"
[theme]
name = "dracula"
background = "#282a36"
foreground = "#f8f8f2"
"##).unwrap();

        std::fs::write(&main_path, format!(r##"
[general]
import = ["{}"]

[theme]
name = "my-override"
"##, theme_path.display())).unwrap();

        let value = load_with_imports(&main_path).ok().unwrap();
        let cfg: Config = value.try_into().unwrap();
        // Overlay (main) overrides the theme name...
        assert_eq!(cfg.theme.name, "my-override");
        // ...but the theme file's bg/fg flow through.
        assert_eq!(cfg.theme.background, "#282a36");
        assert_eq!(cfg.theme.foreground, "#f8f8f2");

        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn cyclic_imports_dont_loop_forever() {
        let dir = tempdir();
        let a = dir.join("a.toml");
        let b = dir.join("b.toml");
        std::fs::write(&a, format!(r#"
[general]
import = ["{}"]
[theme]
name = "from-a"
"#, b.display())).unwrap();
        std::fs::write(&b, format!(r#"
[general]
import = ["{}"]
[font]
size = 17.0
"#, a.display())).unwrap();

        let value = load_with_imports(&a).ok().unwrap();
        let cfg: Config = value.try_into().unwrap();
        assert_eq!(cfg.theme.name, "from-a");
        assert_eq!(cfg.font.size, 17.0);

        std::fs::remove_dir_all(&dir).ok();
    }

    fn tempdir() -> PathBuf {
        let dir = std::env::temp_dir().join(format!(
            "termai-config-test-{}",
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }
}
