//! Persistent recent-files store backing the Start page's Recent Documents
//! panel. Stored as plain text on disk (one path per line, newest first) next
//! to the other per-user config, so no serialization crate is pulled in just
//! for this.

use super::OpenCADStudio;
use std::path::{Path, PathBuf};

/// Bounds and default for how many recent files are kept.
pub(super) const RECENT_MIN: usize = 5;
pub(super) const RECENT_MAX: usize = 100;
const RECENT_DEFAULT: usize = 20;

impl OpenCADStudio {
    /// Record a freshly opened file at the top of the recents list.
    pub(super) fn push_recent(&mut self, path: PathBuf) {
        self.recent_files.retain(|r| r != &path);
        self.recent_files.insert(0, path);
        self.recent_files.truncate(self.recent_limit);
        // Best-effort persist; silent on failure (read-only home, full disk).
        let _ = save_recents(&self.recent_files);
    }

    /// Drop a path from the recents list (manual removal from the Start page).
    pub(super) fn remove_recent(&mut self, path: &Path) {
        self.recent_files.retain(|r| r.as_path() != path);
        let _ = save_recents(&self.recent_files);
    }

    /// Set how many recent files are kept, trim the current list to fit, and
    /// persist both the new limit and the trimmed list.
    pub(super) fn set_recent_limit(&mut self, limit: usize) {
        self.recent_limit = limit.clamp(RECENT_MIN, RECENT_MAX);
        self.recent_files.truncate(self.recent_limit);
        save_recent_limit(self.recent_limit);
        let _ = save_recents(&self.recent_files);
    }
}

/// Rehydrate the recents list from disk, trimmed to the saved limit. Call once
/// at app boot.
pub(super) fn load_recent_files() -> Vec<PathBuf> {
    let Some(path) = recents_file_path() else {
        return vec![];
    };
    let Ok(body) = std::fs::read_to_string(path) else {
        return vec![];
    };
    let mut list: Vec<PathBuf> = body
        .lines()
        .filter(|l| !l.trim().is_empty())
        .map(PathBuf::from)
        .collect();
    list.truncate(load_recent_limit());
    list
}

/// Load the saved recent-file limit (clamped), defaulting to `RECENT_DEFAULT`.
pub(super) fn load_recent_limit() -> usize {
    limit_file_path()
        .and_then(|p| std::fs::read_to_string(p).ok())
        .and_then(|s| s.trim().parse::<usize>().ok())
        .map(|n| n.clamp(RECENT_MIN, RECENT_MAX))
        .unwrap_or(RECENT_DEFAULT)
}

fn save_recent_limit(limit: usize) {
    let Some(path) = limit_file_path() else { return };
    if let Some(dir) = path.parent() {
        let _ = std::fs::create_dir_all(dir);
    }
    let _ = std::fs::write(path, limit.to_string());
}

fn recents_file_path() -> Option<PathBuf> {
    Some(crate::config::config_dir()?.join("recent.txt"))
}

fn limit_file_path() -> Option<PathBuf> {
    Some(crate::config::config_dir()?.join("recent_limit.txt"))
}

fn save_recents(list: &[PathBuf]) -> std::io::Result<()> {
    let Some(path) = recents_file_path() else {
        return Ok(());
    };
    if let Some(dir) = path.parent() {
        std::fs::create_dir_all(dir)?;
    }
    let body: String = list
        .iter()
        .map(|p| p.to_string_lossy().into_owned())
        .collect::<Vec<_>>()
        .join("\n");
    std::fs::write(path, body)
}
