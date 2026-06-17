//! Headless automation server (`OpenCADStudio --serve`).
//!
//! Drives the app without a GUI over a line-based JSON protocol: one request
//! object per line on stdin, one response object per line on stdout. State (the
//! active document) persists across requests, so an external process — a script
//! or an AI agent — can act, observe, and act again.
//!
//! Operations:
//! - `{"op":"new"}`                          — start an empty document
//! - `{"op":"open","path":"file.dwg"}`       — load a drawing
//! - `{"op":"run","cmd":"LAYER Walls"}`      — run a command (the same dispatcher
//!                                             the GUI command line uses)
//! - `{"op":"entities"}`                     — summary count by entity type
//! - `{"op":"save","path":"out.dwg"}`        — write the document (path optional
//!                                             once opened/saved)

use std::io::{BufRead, Write};
use std::path::PathBuf;

use serde_json::{json, Value};

use super::OpenCADStudio;

/// Run the headless JSON server until stdin closes.
pub fn serve() {
    let mut app = OpenCADStudio::new();
    let stdin = std::io::stdin();
    let stdout = std::io::stdout();

    emit(
        &stdout,
        json!({"ok": true, "ready": true, "version": env!("CARGO_PKG_VERSION")}),
    );

    for line in stdin.lock().lines() {
        let Ok(line) = line else { break };
        let line = line.trim();
        if line.is_empty() {
            continue;
        }
        let resp = app.automation_op(line);
        emit(&stdout, resp);
    }
}

fn emit(stdout: &std::io::Stdout, value: Value) {
    let mut o = stdout.lock();
    let _ = writeln!(o, "{value}");
    let _ = o.flush();
}

fn err(msg: impl std::fmt::Display) -> Value {
    json!({ "ok": false, "error": msg.to_string() })
}

impl OpenCADStudio {
    /// Handle one JSON request line and return the JSON response.
    pub(crate) fn automation_op(&mut self, line: &str) -> Value {
        let req: Value = match serde_json::from_str(line) {
            Ok(v) => v,
            Err(e) => return err(format!("invalid JSON: {e}")),
        };
        match req["op"].as_str().unwrap_or("") {
            "new" => {
                let i = self.active_tab;
                self.tabs[i].scene.document = acadrust::CadDocument::new();
                self.tabs[i].current_path = None;
                self.tabs[i].scene.bump_geometry();
                self.entity_summary()
            }
            "open" => {
                let Some(path) = req["path"].as_str() else {
                    return err("open: missing \"path\"");
                };
                let bytes = match std::fs::read(path) {
                    Ok(b) => b,
                    Err(e) => return err(format!("open: {e}")),
                };
                let name = PathBuf::from(path)
                    .file_name()
                    .map(|n| n.to_string_lossy().into_owned())
                    .unwrap_or_else(|| path.to_string());
                match crate::io::load_bytes(&name, bytes) {
                    Ok(doc) => {
                        let i = self.active_tab;
                        self.tabs[i].scene.document = doc;
                        self.tabs[i].current_path = Some(PathBuf::from(path));
                        self.tabs[i].scene.bump_geometry();
                        self.entity_summary()
                    }
                    Err(e) => err(format!("open: {e}")),
                }
            }
            "run" => {
                let cmd = req["cmd"].as_str().unwrap_or("").to_string();
                if cmd.is_empty() {
                    return err("run: missing \"cmd\"");
                }
                let i = self.active_tab;
                let before = self.tabs[i].scene.document.entities().count();
                // The returned Task drives GUI follow-up; synchronous commands
                // (system variables, layer ops, …) have already applied. Pick-
                // based interactive commands need coordinate feeding — not yet
                // wired headless.
                let _ = self.dispatch_command(&cmd);
                let after = self.tabs[i].scene.document.entities().count();
                json!({
                    "ok": true,
                    "cmd": cmd,
                    "entities": after,
                    "added": after as i64 - before as i64,
                })
            }
            "entities" => self.entity_summary(),
            "save" => {
                let i = self.active_tab;
                let path = req["path"]
                    .as_str()
                    .map(PathBuf::from)
                    .or_else(|| self.tabs[i].current_path.clone());
                let Some(path) = path else {
                    return err("save: no \"path\" and the document has none");
                };
                match crate::io::save(&self.tabs[i].scene.document, &path) {
                    Ok(()) => {
                        self.tabs[i].current_path = Some(path.clone());
                        json!({ "ok": true, "saved": path.to_string_lossy() })
                    }
                    Err(e) => err(format!("save: {e}")),
                }
            }
            "" => err("missing \"op\""),
            other => err(format!("unknown op: {other}")),
        }
    }

    /// Count of entities in the active document, total and by type.
    fn entity_summary(&self) -> Value {
        let i = self.active_tab;
        let mut by_type: std::collections::BTreeMap<String, u64> = Default::default();
        let mut total = 0u64;
        for e in self.tabs[i].scene.document.entities() {
            *by_type
                .entry(crate::entities::names::ui_name(e).to_string())
                .or_default() += 1;
            total += 1;
        }
        json!({ "ok": true, "total": total, "by_type": by_type })
    }
}

#[cfg(test)]
mod tests {
    use crate::app::OpenCADStudio;

    #[test]
    fn automation_ops_round_trip() {
        let mut app = OpenCADStudio::new_for_test();

        let r = app.automation_op(r#"{"op":"new"}"#);
        assert_eq!(r["ok"], true);
        assert_eq!(r["total"], 0);

        // A synchronous command runs through the real dispatcher.
        let r = app.automation_op(r#"{"op":"run","cmd":"PDMODE 3"}"#);
        assert_eq!(r["ok"], true);
        assert_eq!(r["cmd"], "PDMODE 3");

        let r = app.automation_op(r#"{"op":"entities"}"#);
        assert_eq!(r["ok"], true);

        // Errors are reported, never panics.
        assert_eq!(app.automation_op(r#"{"op":"bogus"}"#)["ok"], false);
        assert_eq!(app.automation_op("not json")["ok"], false);
        assert_eq!(app.automation_op(r#"{"op":"run"}"#)["ok"], false);
    }

    #[test]
    fn save_then_open_round_trips() {
        let mut app = OpenCADStudio::new_for_test();
        let path = std::env::temp_dir().join("ocs_automation_test.dxf");
        let p = path.to_string_lossy();
        app.automation_op(r#"{"op":"new"}"#);
        assert_eq!(
            app.automation_op(&format!(r#"{{"op":"save","path":"{p}"}}"#))["ok"],
            true
        );
        assert_eq!(
            app.automation_op(&format!(r#"{{"op":"open","path":"{p}"}}"#))["ok"],
            true
        );
        let _ = std::fs::remove_file(&path);
    }
}
