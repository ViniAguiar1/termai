//! Lightweight git branch lookup for the tab strip.
//!
//! Reads `.git/HEAD` directly instead of shelling out to `git`, so it's cheap
//! enough to call as the working directory changes. Walks up from the given
//! directory to find the repository root (handles both a `.git` directory and
//! the `.git` file used by worktrees/submodules).

use std::path::Path;

/// Return the current branch name for the repo containing `start`, or a short
/// commit hash when in detached-HEAD state. `None` if not inside a repo.
pub fn branch(start: &Path) -> Option<String> {
    let mut cur = start.to_path_buf();
    loop {
        let git = cur.join(".git");
        if git.is_dir() {
            return read_head(&git);
        }
        if git.is_file() {
            // Worktree/submodule: the `.git` file points at the real gitdir.
            let content = std::fs::read_to_string(&git).ok()?;
            let path = content.strip_prefix("gitdir:")?.trim();
            return read_head(Path::new(path));
        }
        if !cur.pop() {
            return None;
        }
    }
}

/// Parse a gitdir's `HEAD` into a branch name or short hash.
fn read_head(gitdir: &Path) -> Option<String> {
    let head = std::fs::read_to_string(gitdir.join("HEAD")).ok()?;
    let head = head.trim();
    if let Some(rest) = head.strip_prefix("ref: refs/heads/") {
        return Some(rest.to_string());
    }
    // Detached HEAD — show a short hash.
    if head.len() >= 7 && head.chars().all(|c| c.is_ascii_hexdigit()) {
        return Some(head[..7].to_string());
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_branch_from_symbolic_head() {
        let dir = std::env::temp_dir().join("termai_git_branch_test");
        let gitdir = dir.join(".git");
        std::fs::create_dir_all(&gitdir).unwrap();
        std::fs::write(gitdir.join("HEAD"), "ref: refs/heads/feature/top-bar\n").unwrap();
        assert_eq!(branch(&dir), Some("feature/top-bar".to_string()));
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn reads_short_hash_when_detached() {
        let dir = std::env::temp_dir().join("termai_git_detached_test");
        let gitdir = dir.join(".git");
        std::fs::create_dir_all(&gitdir).unwrap();
        std::fs::write(gitdir.join("HEAD"), "a1b2c3d4e5f6\n").unwrap();
        assert_eq!(branch(&dir), Some("a1b2c3d".to_string()));
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn finds_repo_from_subdirectory() {
        let root = std::env::temp_dir().join("termai_git_walkup_test");
        let gitdir = root.join(".git");
        let sub = root.join("a").join("b");
        std::fs::create_dir_all(&gitdir).unwrap();
        std::fs::create_dir_all(&sub).unwrap();
        std::fs::write(gitdir.join("HEAD"), "ref: refs/heads/main\n").unwrap();
        assert_eq!(branch(&sub), Some("main".to_string()));
        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn none_outside_repo() {
        let dir = std::env::temp_dir().join("termai_git_norepo_test");
        std::fs::create_dir_all(&dir).unwrap();
        // Ensure no stray .git from a previous run.
        let _ = std::fs::remove_dir_all(dir.join(".git"));
        assert_eq!(branch(&dir), None);
        let _ = std::fs::remove_dir_all(&dir);
    }
}
