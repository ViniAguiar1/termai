//! Path shortening for tab titles.
//!
//! `/Users/vini/code/termai` → `~/code/termai` (home expansion only) if ≤ max chars.
//! Otherwise shortens middle segments to their first letter: `~/code/termai` → `~/c/termai`.
//! Final fallback: ellipsis truncation `~/code/te…`.

use std::path::Path;

pub fn shorten<P: AsRef<Path>>(path: P, home: Option<&Path>, max_chars: usize) -> String {
    let path = path.as_ref();
    let mut s = path.to_string_lossy().into_owned();

    if let Some(home) = home {
        if let Ok(rel) = path.strip_prefix(home) {
            s = if rel.as_os_str().is_empty() {
                "~".to_string()
            } else {
                format!("~/{}", rel.to_string_lossy())
            };
        }
    }

    if s.chars().count() <= max_chars {
        return s;
    }

    // Shorten middle segments to first char, one at a time from left, stopping when we fit.
    let parts: Vec<&str> = s.split('/').collect();
    if parts.len() > 2 {
        let mut shortened: Vec<String> = parts.iter().map(|p| p.to_string()).collect();
        // Middle indices: 1 .. len-1 (skip first and last)
        for i in 1..parts.len() - 1 {
            if !shortened[i].is_empty() && shortened[i].chars().count() > 1 {
                let first_char = shortened[i].chars().next().unwrap();
                shortened[i] = first_char.to_string();
                let candidate = shortened.join("/");
                if candidate.chars().count() <= max_chars {
                    return candidate;
                }
            }
        }
        // All middle segments shortened; use the result regardless.
        let acc = shortened.join("/");
        if acc.chars().count() <= max_chars {
            return acc;
        }
        s = acc;
    }

    // Final fallback: ellipsis truncate.
    if s.chars().count() > max_chars {
        let kept: String = s.chars().take(max_chars.saturating_sub(1)).collect();
        return format!("{}…", kept);
    }
    s
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    #[test]
    fn home_replaced_with_tilde() {
        let home = PathBuf::from("/Users/vini");
        assert_eq!(shorten("/Users/vini", Some(&home), 20), "~");
        assert_eq!(shorten("/Users/vini/code", Some(&home), 20), "~/code");
    }

    #[test]
    fn short_path_unchanged() {
        let home = PathBuf::from("/Users/vini");
        assert_eq!(shorten("/Users/vini/code/termai", Some(&home), 20), "~/code/termai");
    }

    #[test]
    fn middle_segments_shortened_when_over_max() {
        let home = PathBuf::from("/Users/vini");
        assert_eq!(
            shorten("/Users/vini/code/projects/termai/crates", Some(&home), 20),
            "~/c/p/termai/crates"
        );
    }

    #[test]
    fn ellipsis_when_still_too_long() {
        let home = PathBuf::from("/Users/vini");
        let result = shorten(
            "/Users/vini/aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa",
            Some(&home),
            10,
        );
        assert!(result.ends_with('…'));
        assert!(result.chars().count() == 10);
    }

    #[test]
    fn no_home_match_keeps_absolute_path() {
        let home = PathBuf::from("/Users/vini");
        assert_eq!(shorten("/etc/hosts", Some(&home), 20), "/etc/hosts");
    }
}
