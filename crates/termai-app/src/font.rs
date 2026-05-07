//! System font resolution.
//!
//! Looks up a family name (e.g. "JetBrainsMono Nerd Font") through the OS font
//! backend (CoreText / fontconfig / DirectWrite) and returns the raw bytes for
//! the renderer's glyph atlas. Falls back to `None` when nothing matches so
//! callers can use the embedded font.

use font_kit::family_name::FamilyName;
use font_kit::handle::Handle;
use font_kit::properties::{Properties, Style, Weight};
use font_kit::source::SystemSource;

/// Read a font from the system by family name. Returns `None` if the family
/// can't be matched or the file can't be read.
pub fn load_system_font(family: &str, style_hint: Option<&str>) -> Option<Vec<u8>> {
    let mut props = Properties::new();
    if let Some(style) = style_hint {
        let lower = style.to_lowercase();
        if lower.contains("bold") {
            props.weight = Weight::BOLD;
        }
        if lower.contains("italic") || lower.contains("oblique") {
            props.style = Style::Italic;
        }
    }

    let handle = SystemSource::new()
        .select_best_match(&[FamilyName::Title(family.to_string())], &props)
        .ok()?;

    match handle {
        Handle::Path { path, .. } => std::fs::read(&path).ok(),
        Handle::Memory { bytes, .. } => Some((*bytes).clone()),
    }
}
