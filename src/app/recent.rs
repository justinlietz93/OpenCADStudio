//! Persistent recent-files store backing the Start page's Recent Documents
//! panel. Stored as plain text on disk (one path per line, newest first) next
//! to the other per-user config, so no serialization crate is pulled in just
//! for this.

use super::OpenCADStudio;
use std::path::{Path, PathBuf};

impl OpenCADStudio {
    /// Record a freshly opened file at the top of the recents list.
    pub(super) fn push_recent(&mut self, path: PathBuf) {
        self.recent_files.retain(|r| r != &path);
        self.recent_files.insert(0, path);
        self.recent_files.truncate(20);
        // Best-effort persist; silent on failure (read-only home, full disk).
        let _ = save_recents(&self.recent_files);
    }

    /// Drop a path from the recents list (manual removal from the Start page).
    pub(super) fn remove_recent(&mut self, path: &Path) {
        self.recent_files.retain(|r| r.as_path() != path);
        let _ = save_recents(&self.recent_files);
    }
}

/// Rehydrate the recents list from disk. Call once at app boot.
pub(super) fn load_recent_files() -> Vec<PathBuf> {
    let Some(path) = recents_file_path() else {
        return vec![];
    };
    let Ok(body) = std::fs::read_to_string(path) else {
        return vec![];
    };
    body.lines()
        .filter(|l| !l.trim().is_empty())
        .map(PathBuf::from)
        .collect()
}

fn recents_file_path() -> Option<PathBuf> {
    let base: PathBuf = if cfg!(target_os = "windows") {
        std::env::var_os("APPDATA").map(PathBuf::from)?
    } else if cfg!(target_os = "macos") {
        let home = std::env::var_os("HOME")?;
        let mut p = PathBuf::from(home);
        p.push("Library");
        p.push("Application Support");
        p
    } else if let Some(d) = std::env::var_os("XDG_CONFIG_HOME") {
        PathBuf::from(d)
    } else {
        let home = std::env::var_os("HOME")?;
        let mut p = PathBuf::from(home);
        p.push(".config");
        p
    };
    let mut p = base;
    p.push("OpenCADStudio");
    Some(p.join("recent.txt"))
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
