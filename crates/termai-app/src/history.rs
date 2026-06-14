//! Local command history for instant, zero-latency autocomplete.
//!
//! Mirrors the behavior of fish / zsh-autosuggestions: keep a recency-ordered,
//! deduplicated list of commands the user has run (seeded from their shell
//! history file) and, given a typed prefix, return the suffix of the most
//! recent matching command. This is a pure in-memory prefix lookup — no IPC,
//! no network — so ghost text can appear within a single frame.

use std::path::PathBuf;

/// Cap on retained entries, to bound memory on huge history files.
const MAX_ENTRIES: usize = 5000;

#[derive(Default)]
pub struct CommandHistory {
    /// Commands, most-recent first, deduplicated (each command appears once).
    entries: Vec<String>,
}

impl CommandHistory {
    /// Build a history seeded from the user's shell history file. Best-effort:
    /// a missing or unreadable file yields an empty history (autocomplete then
    /// fills in as the user runs commands this session).
    pub fn load() -> Self {
        let mut h = CommandHistory::default();
        if let Some(path) = shell_history_path() {
            if let Ok(contents) = std::fs::read_to_string(&path) {
                h.seed_from_file(&contents);
            }
        }
        h
    }

    /// Parse a shell-history file body, most-recent-last on disk, into entries
    /// (most-recent-first, deduplicated). Handles zsh extended history
    /// (`: <ts>:<elapsed>;<cmd>`) and plain bash lines.
    pub fn seed_from_file(&mut self, contents: &str) {
        let mut seen = std::collections::HashSet::new();
        // Walk oldest→newest, dropping older duplicates, then reverse so the
        // newest occurrence wins and ends up first.
        let mut ordered: Vec<String> = Vec::new();
        for raw in contents.lines() {
            let cmd = parse_history_line(raw);
            if cmd.is_empty() {
                continue;
            }
            ordered.push(cmd.to_string());
        }
        for cmd in ordered.into_iter().rev() {
            if seen.insert(cmd.clone()) {
                self.entries.push(cmd);
                if self.entries.len() >= MAX_ENTRIES {
                    break;
                }
            }
        }
    }

    /// Record a command the user just ran, moving it to the front (most recent).
    pub fn record(&mut self, cmd: &str) {
        let cmd = cmd.trim();
        if cmd.is_empty() {
            return;
        }
        self.entries.retain(|e| e != cmd);
        self.entries.insert(0, cmd.to_string());
        self.entries.truncate(MAX_ENTRIES);
    }

    /// Given what the user has typed, return the completion *suffix* of the most
    /// recent command that starts with it (excluding the prefix itself). Returns
    /// `None` when nothing extends the prefix.
    pub fn suggest(&self, typed: &str) -> Option<String> {
        if typed.is_empty() {
            return None;
        }
        self.entries
            .iter()
            .find(|e| e.len() > typed.len() && e.starts_with(typed))
            .map(|e| e[typed.len()..].to_string())
    }
}

/// Extract the command from a single history-file line. zsh extended-history
/// lines look like `: 1700000000:0;git status`; everything else is taken as-is.
fn parse_history_line(raw: &str) -> &str {
    let line = raw.trim_end();
    if let Some(rest) = line.strip_prefix(": ") {
        // `<ts>:<elapsed>;<cmd>` — the command is after the first ';'.
        if let Some(idx) = rest.find(';') {
            return rest[idx + 1..].trim();
        }
    }
    line.trim()
}

/// Resolve the shell history file: `$HISTFILE` if set, else `~/.zsh_history`,
/// else `~/.bash_history`.
fn shell_history_path() -> Option<PathBuf> {
    if let Ok(hf) = std::env::var("HISTFILE") {
        if !hf.is_empty() {
            return Some(PathBuf::from(hf));
        }
    }
    let home = std::env::var("HOME").ok()?;
    let zsh = PathBuf::from(&home).join(".zsh_history");
    if zsh.exists() {
        return Some(zsh);
    }
    let bash = PathBuf::from(&home).join(".bash_history");
    if bash.exists() {
        return Some(bash);
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn suggests_most_recent_match_suffix() {
        let mut h = CommandHistory::default();
        h.record("git status");
        h.record("git checkout main");
        // Most recent matching "git ch" is "git checkout main".
        assert_eq!(h.suggest("git ch"), Some("eckout main".to_string()));
    }

    #[test]
    fn no_suggestion_when_nothing_extends_prefix() {
        let mut h = CommandHistory::default();
        h.record("ls -la");
        assert_eq!(h.suggest("git"), None);
        // Exact equality is not a suggestion (nothing left to complete).
        assert_eq!(h.suggest("ls -la"), None);
        assert_eq!(h.suggest(""), None);
    }

    #[test]
    fn record_moves_existing_to_front() {
        let mut h = CommandHistory::default();
        h.record("git pull");
        h.record("git push");
        h.record("git pull"); // re-run an old command
        // "git p" should now resolve to the freshest: "git pull".
        assert_eq!(h.suggest("git p"), Some("ull".to_string()));
        // And "git pull" appears only once.
        assert_eq!(h.entries.iter().filter(|e| *e == "git pull").count(), 1);
    }

    #[test]
    fn parses_zsh_extended_history() {
        let body = ": 1700000000:0;git status\n: 1700000001:0;cargo build\n";
        let mut h = CommandHistory::default();
        h.seed_from_file(body);
        assert_eq!(h.suggest("car"), Some("go build".to_string()));
        assert_eq!(h.suggest("git"), Some(" status".to_string()));
    }

    #[test]
    fn parses_plain_bash_history() {
        let body = "npm install\nnpm run dev\n";
        let mut h = CommandHistory::default();
        h.seed_from_file(body);
        // Newest on disk (last line) wins for "npm ".
        assert_eq!(h.suggest("npm "), Some("run dev".to_string()));
    }

    #[test]
    fn seed_dedupes_keeping_newest() {
        let body = "ls\ncd foo\nls\n";
        let mut h = CommandHistory::default();
        h.seed_from_file(body);
        assert_eq!(h.entries.iter().filter(|e| *e == "ls").count(), 1);
    }
}
