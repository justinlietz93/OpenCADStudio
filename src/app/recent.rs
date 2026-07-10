//! Recent-files list backing the Start page's Recent Documents panel. The list
//! itself lives in the consolidated app config (`settings.json`, the "recent"
//! section); this module just mutates the in-memory list and persists via
//! `save_config`.

use super::OpenCADStudio;
use std::path::{Path, PathBuf};

/// Bounds and default for how many recent files are kept.
pub(super) const RECENT_MIN: usize = 5;
pub(super) const RECENT_MAX: usize = 100;
pub(super) const RECENT_DEFAULT: usize = 20;

impl OpenCADStudio {
    /// Record a freshly opened file at the top of the recents list.
    pub(super) fn push_recent(&mut self, path: PathBuf) {
        self.recent_files.retain(|r| r != &path);
        self.recent_files.insert(0, path);
        self.recent_files.truncate(self.recent_limit);
        self.save_config();
    }

    /// Drop a path from the recents list (manual removal from the Start page).
    pub(super) fn remove_recent(&mut self, path: &Path) {
        self.recent_files.retain(|r| r.as_path() != path);
        self.save_config();
    }

    /// Set how many recent files are kept, trim the current list to fit, and
    /// persist both.
    pub(super) fn set_recent_limit(&mut self, limit: usize) {
        self.recent_limit = limit.clamp(RECENT_MIN, RECENT_MAX);
        self.recent_files.truncate(self.recent_limit);
        self.save_config();
    }
}
