// Storm Sewer module — gravity storm-drain network design & analysis.
//
// Implements the standard public-domain methods (Rational method, Manning,
// HGL backwater) via the external `stormsewer` engine crate. The ribbon tab
// here is the UI surface; the actual command handlers (place structure, draw
// pipe, run analysis) are dispatched by the host command system — see
// INTEGRATION.md for where each `SS_*` command plugs in.

pub mod analysis;
pub mod structures;

use crate::modules::{CadModule, IconKind, ModuleEvent, RibbonGroup, RibbonItem, ToolDef};

pub struct StormSewerModule;

// Register the SS_* command names for command-line autocomplete.
inventory::submit!(crate::command::CommandRegistration {
    names: &[
        "SS_INLET",
        "SS_JUNCTION",
        "SS_OUTFALL",
        "SS_PIPE",
        "SS_ANALYZE",
        "SS_REPORT",
        "SS_PROFILE",
    ]
});

const IC_INLET: &[u8] = include_bytes!("icons/inlet.svg");
const IC_JUNCTION: &[u8] = include_bytes!("icons/junction.svg");
const IC_OUTFALL: &[u8] = include_bytes!("icons/outfall.svg");
const IC_PIPE: &[u8] = include_bytes!("icons/pipe.svg");
const IC_ANALYZE: &[u8] = include_bytes!("icons/analyze.svg");
const IC_REPORT: &[u8] = include_bytes!("icons/report.svg");
const IC_PROFILE: &[u8] = include_bytes!("icons/profile.svg");

/// Convenience: an SVG-icon tool that fires a named `SS_*` command.
fn tool(id: &'static str, label: &'static str, icon: &'static [u8]) -> ToolDef {
    ToolDef {
        id,
        label,
        icon: IconKind::Svg(icon),
        event: ModuleEvent::Command(id.to_string()),
    }
}

impl CadModule for StormSewerModule {
    fn id(&self) -> &'static str {
        "storm_sewer"
    }
    fn title(&self) -> &'static str {
        "Storm Sewer"
    }

    fn ribbon_groups(&self) -> Vec<RibbonGroup> {
        vec![
            // ── Network: place structures and connect them with pipes ───────
            RibbonGroup {
                title: "Network",
                tools: vec![
                    RibbonItem::LargeTool(tool("SS_INLET", "Inlet", IC_INLET)),
                    RibbonItem::LargeTool(tool("SS_JUNCTION", "Junction", IC_JUNCTION)),
                    RibbonItem::LargeTool(tool("SS_OUTFALL", "Outfall", IC_OUTFALL)),
                    RibbonItem::LargeTool(tool("SS_PIPE", "Pipe\nRun", IC_PIPE)),
                ],
            },
            // ── Analysis: run the engine and review results ─────────────────
            RibbonGroup {
                title: "Analysis",
                tools: vec![
                    RibbonItem::LargeTool(tool("SS_ANALYZE", "Analyze", IC_ANALYZE)),
                    RibbonItem::Tool(tool("SS_REPORT", "Report", IC_REPORT)),
                    RibbonItem::Tool(tool("SS_PROFILE", "Profile", IC_PROFILE)),
                ],
            },
        ]
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::modules::registry;

    #[test]
    fn module_is_registered_in_ribbon() {
        let titles: Vec<&str> = registry::all_modules().iter().map(|m| m.title()).collect();
        assert!(titles.contains(&"Storm Sewer"), "ribbon tabs: {titles:?}");
    }

    #[test]
    fn ribbon_exposes_core_tools() {
        let mut ids = Vec::new();
        for g in StormSewerModule.ribbon_groups() {
            for item in g.tools {
                if let RibbonItem::Tool(t) | RibbonItem::LargeTool(t) = item {
                    ids.push(t.id);
                }
            }
        }
        for needed in ["SS_INLET", "SS_PIPE", "SS_ANALYZE", "SS_REPORT", "SS_PROFILE"] {
            assert!(ids.contains(&needed), "missing {needed}; have {ids:?}");
        }
    }

    #[test]
    fn engine_bridge_runs_inside_ocs() {
        // The stormsewer crate, called from within the OCS binary, produces a
        // full analysis report for the properly-sized sample network.
        let report = super::analysis::demo_report();
        assert!(report.contains("STORM SEWER ANALYSIS"), "no header:\n{report}");
        assert!(report.contains("P1") && report.contains("HGL"), "no pipe/HGL:\n{report}");
        assert!(report.contains("no surface flooding"), "expected ok design:\n{report}");
    }

    #[test]
    fn analyze_plan_emits_entities_and_report() {
        let (ents, report) = super::analysis::analyze_plan().expect("analysis runs");
        // 3 pipes + 4 structures + labels  → comfortably more than 7 entities.
        assert!(ents.len() >= 7, "too few entities: {}", ents.len());
        assert!(report.contains("STORM SEWER ANALYSIS"));
    }

    #[test]
    fn analyze_profile_emits_entities() {
        let ents = super::analysis::analyze_profile().expect("profile runs");
        // Ground + invert + HGL line segments along a 4-node stem.
        assert!(ents.len() >= 6, "too few profile entities: {}", ents.len());
    }

    const SAMPLE_SSN: &str = "\
IDF 60 10 0.8
TAILWATER 100.5
NODE N1 inlet 0 0 104 110 1 0.7 12
NODE N2 inlet 300 0 102.5 108.5 1 0.7
NODE OUT outfall 600 0 100 106
PIPE P1 N1 N2 300 1.25 0.013
PIPE P2 N2 OUT 300 1.5 0.013
";

    #[test]
    fn analyze_text_parses_and_emits() {
        let (ents, report) = super::analysis::analyze_text(SAMPLE_SSN).expect("parse + analyze");
        assert!(ents.len() >= 5, "too few entities: {}", ents.len());
        assert!(report.contains("STORM SEWER ANALYSIS"));
    }

    #[test]
    fn profile_text_parses_and_emits() {
        let ents = super::analysis::profile_text(SAMPLE_SSN).expect("parse + profile");
        assert!(ents.len() >= 4, "too few profile entities: {}", ents.len());
    }

    #[test]
    fn bad_ssn_reports_error() {
        let err = super::analysis::analyze_text("NODE X inlet 0 0 oops 1 1 1").unwrap_err();
        assert!(err.contains("line 1"), "{err}");
    }
}
