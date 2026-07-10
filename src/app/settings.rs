//! Persisted user preferences — DYN, ORTHO, POLAR, the polar increment, the
//! grid toggle, and the object-snap configuration (OSNAP on/off, which snap
//! modes are active, OTRACK). These are UI choices, not drawing data, so they
//! live in a per-user config file and survive across sessions.
//!
//! Plain `key=value` text, matching the recent-files / status-bar stores so we
//! don't pull in a serialization crate just for a handful of flags. Drawing
//! header settings such as LWT (lineweight display) are intentionally NOT here
//! — those belong to the file.

use crate::snap::SnapType;
use std::path::PathBuf;

/// Canonical order of the user-toggleable object-snap modes. Drives the
/// deterministic order when decoding the `$OSMODE` bitmask (see
/// [`snaps_from_osmode`]).
const SNAP_ORDER: &[SnapType] = &[
    SnapType::Endpoint,
    SnapType::Midpoint,
    SnapType::Center,
    SnapType::Node,
    SnapType::Quadrant,
    SnapType::Intersection,
    SnapType::Extension,
    SnapType::Insertion,
    SnapType::Perpendicular,
    SnapType::Tangent,
    SnapType::Nearest,
    SnapType::ApparentIntersection,
    SnapType::Parallel,
    // Grid snap (SNAPMODE) is a per-drawing view setting stored on the VPort,
    // not a global OSNAP preference, so it is deliberately excluded from the
    // persisted set. (#121)
];

/// `$OSMODE` bit for each running object-snap mode (standard AutoCAD bitmask).
/// `None` for OCS-only snaps (Grid, ObjectPick) that have no standard bit.
fn snap_bit(s: SnapType) -> Option<i32> {
    Some(match s {
        SnapType::Endpoint => 1,
        SnapType::Midpoint => 2,
        SnapType::Center => 4,
        SnapType::Node => 8,
        SnapType::Quadrant => 16,
        SnapType::Intersection => 32,
        SnapType::Insertion => 64,
        SnapType::Perpendicular => 128,
        SnapType::Tangent => 256,
        SnapType::Nearest => 512,
        SnapType::ApparentIntersection => 2048,
        SnapType::Extension => 4096,
        SnapType::Parallel => 8192,
        SnapType::Grid | SnapType::ObjectPick => return None,
    })
}

/// Bit 14 of `$OSMODE` — object snap turned off (master suppress).
const OSMODE_SUPPRESS: i32 = 16384;

/// Encode the running-snap set + master toggle into an `$OSMODE` bitmask for the
/// drawing header. OCS-only snaps (Grid, ObjectPick) have no bit and are
/// dropped. A cleared master toggle sets the suppress bit.
pub(crate) fn osmode_from_snaps<'a>(
    enabled: impl IntoIterator<Item = &'a SnapType>,
    snap_enabled: bool,
) -> i32 {
    let mut bits = 0;
    for t in enabled {
        if let Some(b) = snap_bit(*t) {
            bits |= b;
        }
    }
    if !snap_enabled {
        bits |= OSMODE_SUPPRESS;
    }
    bits
}

/// Decode an `$OSMODE` bitmask into `(running-snap set, master enabled)`. Only
/// the standard mappable modes are produced; the suppress bit maps (inverted) to
/// the master toggle.
pub(crate) fn snaps_from_osmode(osmode: i32) -> (Vec<SnapType>, bool) {
    let modes = SNAP_ORDER
        .iter()
        .copied()
        .filter(|t| snap_bit(*t).is_some_and(|b| osmode & b != 0))
        .collect();
    (modes, osmode & OSMODE_SUPPRESS == 0)
}

/// Parse a persisted `r,g,b` background triplet (each 0–255). Returns `None`
/// for an empty or malformed value so a missing/garbage key falls back to the
/// app default rather than a wrong colour.
fn parse_rgb(val: &str) -> Option<[u8; 3]> {
    let mut it = val.split(',').map(|t| t.trim().parse::<u8>());
    let r = it.next()?.ok()?;
    let g = it.next()?.ok()?;
    let b = it.next()?.ok()?;
    if it.next().is_some() {
        return None;
    }
    Some([r, g, b])
}

/// Serialize an optional background triplet back to `r,g,b`, or empty when unset.
fn rgb_to_str(c: Option<[u8; 3]>) -> String {
    c.map(|[r, g, b]| format!("{r},{g},{b}")).unwrap_or_default()
}

/// A snapshot of the persisted preferences. Field defaults mirror the app's
/// in-code defaults so a missing key restores the same value the app boots
/// with.
#[derive(Clone, PartialEq)]
pub struct UserSettings {
    pub dyn_input: bool,
    pub polar: bool,
    pub polar_increment_deg: f32,
    pub otrack: bool,
    // Ortho ($ORTHOMODE) and the running OSNAP set ($OSMODE) are per-drawing —
    // stored in the document header, not here (they used to be persisted app-
    // globally, which duplicated the file's own state).
    /// Whether the one-time "make Open CAD Studio the default for .dwg/.dxf?"
    /// prompt has already been shown. Set once the user answers (either way),
    /// so we never nag again on subsequent launches.
    pub default_assoc_prompted: bool,
    /// Ids of plugins the user turned off in the Plugin Manager. Disabled
    /// plugins keep their manifest listed but drop their ribbon tab and command
    /// dispatch.
    pub disabled_plugins: Vec<String>,
    /// Linked plugin source repositories (`owner/repo`) the marketplace installs
    /// from.
    pub plugin_repos: Vec<String>,
    /// Controls whether the TEXTEDIT command repeats automatically (0 = Multiple, 1 = Single).
    pub texteditmode: bool,
    /// TEXTFILL: fill TrueType glyphs (true) or draw them hollow (false).
    pub textfill: bool,
    /// When true, saving over an existing file first copies it to a sibling
    /// `<name>.bak` so a faulty or accidental save can be recovered (#205).
    pub backup_on_save: bool,
    /// When true (default), the app (re)registers itself as a .dwg/.dxf/.bak
    /// handler on every launch. Toggle with the FILEASSOC command.
    pub file_assoc_enabled: bool,
    /// Minutes between autosaves to a `.sv$` recovery file (SAVETIME command).
    /// 0 disables autosave.
    pub savetime_min: i32,
    /// Persisted viewport background colours (0–255 RGB); `None` = app default
    /// (dark grey model / off-white paper). Applied to every drawing tab on
    /// launch and to tabs opened later, so a chosen background survives restarts
    /// (#188).
    pub bg_color: Option<[u8; 3]>,
    pub paper_bg_color: Option<[u8; 3]>,
}

impl Default for UserSettings {
    fn default() -> Self {
        Self {
            dyn_input: true,
            polar: false,
            polar_increment_deg: 45.0,
            otrack: false,
            default_assoc_prompted: false,
            disabled_plugins: Vec::new(),
            plugin_repos: Vec::new(),
            texteditmode: false,
            textfill: true,
            backup_on_save: true,
            file_assoc_enabled: true,
            savetime_min: 10,
            bg_color: None,
            paper_bg_color: None,
        }
    }
}

impl UserSettings {
    /// Read the saved preferences, or `None` when no settings file exists yet.
    /// Unknown / missing keys fall back to [`UserSettings::default`].
    pub fn load() -> Option<Self> {
        let path = config_path()?;
        let body = std::fs::read_to_string(path).ok()?;
        let mut s = UserSettings::default();
        for line in body.lines() {
            let line = line.trim();
            let Some((key, val)) = line.split_once('=') else { continue };
            let (key, val) = (key.trim(), val.trim());
            match key {
                "dyn" => s.dyn_input = val == "1",
                "polar" => s.polar = val == "1",
                "polar_increment_deg" => {
                    if let Ok(v) = val.parse::<f32>() {
                        s.polar_increment_deg = v;
                    }
                }
                "otrack" => s.otrack = val == "1",
                "bg_color" => s.bg_color = parse_rgb(val),
                "paper_bg_color" => s.paper_bg_color = parse_rgb(val),
                "default_assoc_prompted" => s.default_assoc_prompted = val == "1",
                "texteditmode" => {
                    if let Some(v) =
                        crate::modules::annotate::textedit::parse_texteditmode(val)
                    {
                        s.texteditmode = v;
                    }
                }
                "backup_on_save" => s.backup_on_save = val == "1",
                "textfill" => s.textfill = val == "1",
                "file_assoc_enabled" => s.file_assoc_enabled = val == "1",
                "savetime_min" => {
                    if let Ok(v) = val.parse::<i32>() {
                        s.savetime_min = v.max(0);
                    }
                }
                "disabled_plugins" => {
                    s.disabled_plugins = val
                        .split(',')
                        .map(|t| t.trim())
                        .filter(|t| !t.is_empty())
                        .map(|t| t.to_string())
                        .collect();
                }
                "plugin_repos" => {
                    s.plugin_repos = val
                        .split(',')
                        .map(|t| t.trim())
                        .filter(|t| !t.is_empty())
                        .map(|t| t.to_string())
                        .collect();
                }
                _ => {}
            }
        }
        Some(s)
    }

    /// Best-effort persist; silent on failure (read-only home, full disk).
    pub fn save(&self) {
        let Some(path) = config_path() else { return };
        if let Some(dir) = path.parent() {
            let _ = std::fs::create_dir_all(dir);
        }
        let b = |v: bool| if v { "1" } else { "0" };
        let body = format!(
            "dyn={}\npolar={}\npolar_increment_deg={}\notrack={}\ndefault_assoc_prompted={}\ndisabled_plugins={}\nplugin_repos={}\ntexteditmode={}\ntextfill={}\nbackup_on_save={}\nfile_assoc_enabled={}\nsavetime_min={}\nbg_color={}\npaper_bg_color={}\n",
            b(self.dyn_input),
            b(self.polar),
            self.polar_increment_deg,
            b(self.otrack),
            b(self.default_assoc_prompted),
            self.disabled_plugins.join(","),
            self.plugin_repos.join(","),
            self.texteditmode,
            b(self.textfill),
            b(self.backup_on_save),
            b(self.file_assoc_enabled),
            self.savetime_min,
            rgb_to_str(self.bg_color),
            rgb_to_str(self.paper_bg_color),
        );
        let _ = std::fs::write(path, body);
    }
}

/// `<config-dir>/OpenCADStudio/settings.txt`, matching the recent-files store.
fn config_path() -> Option<PathBuf> {
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
    Some(p.join("settings.txt"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn osmode_encodes_bits_and_suppress() {
        // Endpoint(1) + Midpoint(2) + Intersection(32) = 35, master on.
        let on = [SnapType::Endpoint, SnapType::Midpoint, SnapType::Intersection];
        assert_eq!(osmode_from_snaps(on.iter(), true), 35);
        // Master off sets the suppress bit (16384).
        assert_eq!(osmode_from_snaps(on.iter(), false), 35 | 16384);
        // OCS-only snaps carry no bit and are dropped.
        assert_eq!(osmode_from_snaps([SnapType::Grid, SnapType::ObjectPick].iter(), true), 0);
    }

    #[test]
    fn osmode_decodes_bits_and_suppress() {
        let (modes, enabled) = snaps_from_osmode(35);
        let set: std::collections::HashSet<_> = modes.into_iter().collect();
        assert!(enabled);
        assert_eq!(set.len(), 3);
        assert!(set.contains(&SnapType::Endpoint));
        assert!(set.contains(&SnapType::Midpoint));
        assert!(set.contains(&SnapType::Intersection));
        // Suppress bit → master off; the mode bits still decode.
        let (_m, en) = snaps_from_osmode(35 | 16384);
        assert!(!en);
    }

    #[test]
    fn osmode_round_trips_every_mappable_mode() {
        // All 13 running snaps + master on encode and decode back to the same set.
        let all: Vec<SnapType> = SNAP_ORDER.to_vec();
        let bits = osmode_from_snaps(all.iter(), true);
        let (back, enabled) = snaps_from_osmode(bits);
        assert!(enabled);
        let a: std::collections::HashSet<_> = all.into_iter().collect();
        let b: std::collections::HashSet<_> = back.into_iter().collect();
        assert_eq!(a, b);
    }
}
