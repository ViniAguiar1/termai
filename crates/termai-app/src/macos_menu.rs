//! Native macOS menu bar (App / Edit / View / Window), built with `muda`.
//!
//! winit doesn't populate the macOS `NSMenu`, so without this the menu bar shows
//! only the bold app name and nothing else (no About, no ⌘Q, no Edit). Items
//! that mirror the app's own keyboard shortcuts (Copy/Paste, font zoom) carry
//! the same accelerators and are dispatched back into the app via `MenuEvent`
//! using the `MENU_*` ids below; the predefined items (About, Hide, Quit,
//! Minimize, Zoom, Full Screen) are handled natively by AppKit.

use muda::{
    accelerator::{Accelerator, Code, Modifiers},
    AboutMetadata, Menu, MenuItem, PredefinedMenuItem, Submenu,
};

pub const MENU_COPY: &str = "copy";
pub const MENU_PASTE: &str = "paste";
pub const MENU_SELECT_ALL: &str = "select_all";
pub const MENU_ZOOM_IN: &str = "zoom_in";
pub const MENU_ZOOM_OUT: &str = "zoom_out";
pub const MENU_ZOOM_RESET: &str = "zoom_reset";
pub const MENU_NEW_TAB: &str = "new_tab";
pub const MENU_CLOSE_TAB: &str = "close_tab";
pub const MENU_SPLIT_V: &str = "split_v";
pub const MENU_SPLIT_H: &str = "split_h";
pub const MENU_CLEAR: &str = "clear";
pub const MENU_AI_EXPLAIN: &str = "ai_explain";
pub const MENU_AI_DISMISS: &str = "ai_dismiss";
pub const MENU_HELP_DOCS: &str = "help_docs";
pub const MENU_HELP_SITE: &str = "help_site";
pub const MENU_HELP_ISSUE: &str = "help_issue";

/// `⌘ + key` accelerator (SUPER maps to Command on macOS).
fn cmd(key: Code) -> Option<Accelerator> {
    Some(Accelerator::new(Some(Modifiers::SUPER), key))
}

/// `⌘⇧ + key` accelerator.
fn cmd_shift(key: Code) -> Option<Accelerator> {
    Some(Accelerator::new(
        Some(Modifiers::SUPER | Modifiers::SHIFT),
        key,
    ))
}

/// Build the full menu and install it as the application's main menu. Returns
/// the `Menu` so the caller can keep it alive — dropping it tears the native
/// menu back down.
pub fn build() -> Menu {
    let menu = Menu::new();

    // App menu — the first submenu becomes the bold application menu.
    let app = Submenu::new("termAI", true);
    let about = PredefinedMenuItem::about(
        Some("About termAI"),
        Some(AboutMetadata {
            name: Some("termAI".into()),
            version: Some(env!("CARGO_PKG_VERSION").into()),
            comments: Some("GPU-accelerated terminal with built-in AI.".into()),
            ..Default::default()
        }),
    );
    let _ = app.append_items(&[
        &about,
        &PredefinedMenuItem::separator(),
        &PredefinedMenuItem::hide(None),
        &PredefinedMenuItem::hide_others(None),
        &PredefinedMenuItem::show_all(None),
        &PredefinedMenuItem::separator(),
        &PredefinedMenuItem::quit(None),
    ]);

    // Terminal menu — tabs, splits, and clear (all routed via MenuEvent).
    let terminal = Submenu::new("Terminal", true);
    let new_tab = MenuItem::with_id(MENU_NEW_TAB, "New Tab", true, cmd(Code::KeyT));
    let close_tab = MenuItem::with_id(MENU_CLOSE_TAB, "Close Tab", true, cmd(Code::KeyW));
    let split_v = MenuItem::with_id(MENU_SPLIT_V, "Split Right", true, cmd(Code::KeyD));
    let split_h = MenuItem::with_id(MENU_SPLIT_H, "Split Down", true, cmd_shift(Code::KeyD));
    let clear = MenuItem::with_id(MENU_CLEAR, "Clear", true, cmd(Code::KeyK));
    let _ = terminal.append_items(&[
        &new_tab,
        &close_tab,
        &PredefinedMenuItem::separator(),
        &split_v,
        &split_h,
        &PredefinedMenuItem::separator(),
        &clear,
    ]);

    // Edit menu — custom items routed back to the app through MenuEvent so they
    // reuse the existing clipboard/selection logic on our wgpu surface.
    let edit = Submenu::new("Edit", true);
    let copy = MenuItem::with_id(MENU_COPY, "Copy", true, cmd(Code::KeyC));
    let paste = MenuItem::with_id(MENU_PASTE, "Paste", true, cmd(Code::KeyV));
    let select_all = MenuItem::with_id(MENU_SELECT_ALL, "Select All", true, cmd(Code::KeyA));
    let _ = edit.append_items(&[
        &copy,
        &PredefinedMenuItem::separator(),
        &paste,
        &select_all,
    ]);

    // View menu — font zoom (mirrors ⌘=/⌘−/⌘0) and native Full Screen.
    let view = Submenu::new("View", true);
    let zin = MenuItem::with_id(MENU_ZOOM_IN, "Increase Font Size", true, cmd(Code::Equal));
    let zout = MenuItem::with_id(MENU_ZOOM_OUT, "Decrease Font Size", true, cmd(Code::Minus));
    let zreset = MenuItem::with_id(MENU_ZOOM_RESET, "Reset Font Size", true, cmd(Code::Digit0));
    let _ = view.append_items(&[
        &zin,
        &zout,
        &zreset,
        &PredefinedMenuItem::separator(),
        &PredefinedMenuItem::fullscreen(None),
    ]);

    // AI menu — manual triggers for the built-in assistant.
    let ai = Submenu::new("AI", true);
    let ai_explain = MenuItem::with_id(
        MENU_AI_EXPLAIN,
        "Explain Last Error",
        true,
        cmd_shift(Code::KeyA),
    );
    let ai_dismiss = MenuItem::with_id(MENU_AI_DISMISS, "Dismiss Suggestion", true, None);
    let _ = ai.append_items(&[&ai_explain, &ai_dismiss]);

    // Window menu — native window management.
    let window = Submenu::new("Window", true);
    let _ = window.append_items(&[
        &PredefinedMenuItem::minimize(None),
        &PredefinedMenuItem::maximize(None), // shows as "Zoom"
        &PredefinedMenuItem::separator(),
        &PredefinedMenuItem::bring_all_to_front(None),
    ]);

    // Help menu — external links (opened via MenuEvent → `open <url>`).
    let help = Submenu::new("Help", true);
    let docs = MenuItem::with_id(MENU_HELP_DOCS, "termAI Documentation", true, None);
    let site = MenuItem::with_id(MENU_HELP_SITE, "termAI Website", true, None);
    let issue = MenuItem::with_id(MENU_HELP_ISSUE, "Report an Issue", true, None);
    let _ = help.append_items(&[&docs, &site, &PredefinedMenuItem::separator(), &issue]);

    let _ = menu.append_items(&[&app, &terminal, &edit, &view, &ai, &window, &help]);
    // Let AppKit manage the standard window list / Help search field.
    window.set_as_windows_menu_for_nsapp();
    help.set_as_help_menu_for_nsapp();
    menu.init_for_nsapp();
    menu
}
