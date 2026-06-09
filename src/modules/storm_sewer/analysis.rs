// Bridge between Open CAD Studio and the `stormsewer` engine crate.
//
// Runs the hydrology/hydraulics engine on a network and turns the result into
// (a) acadrust entities to draw into the document and (b) a text report for the
// command line. Networks come from a `.ssn` file (see `stormsewer::parse`); a
// built-in sample backs the unit tests and the zero-argument fallbacks.

use acadrust::types::Vector3;
use acadrust::{Circle, EntityType, Line, MText};

use stormsewer::drawing::{draw_network, DrawConfig};
use stormsewer::idf::IdfCurve;
use stormsewer::network::{Analysis, AnalysisOptions, Network, Node, Pipe};
use stormsewer::parse::parse_ssn;
use stormsewer::report::format_analysis;

use super::data;

/// Default network-level analysis parameters used when analyzing a drawn
/// network (per-entity data carries geometry/area; rainfall is global).
fn default_params() -> (IdfCurve, AnalysisOptions) {
    (IdfCurve::new(60.0, 10.0, 0.8), AnalysisOptions::default())
}

/// Annotation labels only (flow + HGL), for overlaying on an already-drawn
/// network without re-drawing its geometry.
fn build_annotations(net: &Network, a: &Analysis) -> Vec<EntityType> {
    draw_network(net, a, &DrawConfig::default())
        .plan_labels
        .iter()
        .map(|l| label(l.x, l.y, l.text.clone(), l.height))
        .collect()
}

// ── Public API: analyze the network drawn in the active document ────────────

/// Reconstruct the network from drawn entities, analyze it, and return
/// annotation entities (flow/HGL labels) + the report.
pub fn analyze_doc<'a>(entities: impl Iterator<Item = &'a EntityType>) -> Result<(Vec<EntityType>, String), String> {
    let net = data::network_from_entities(entities)?;
    let (idf, opts) = default_params();
    let a = run_analysis(&net, &idf, &opts)?;
    Ok((build_annotations(&net, &a), format_analysis(&a)))
}

/// Reconstruct from drawn entities and return the HGL long-section entities.
pub fn profile_doc<'a>(entities: impl Iterator<Item = &'a EntityType>) -> Result<Vec<EntityType>, String> {
    let net = data::network_from_entities(entities)?;
    let (idf, opts) = default_params();
    let a = run_analysis(&net, &idf, &opts)?;
    Ok(build_profile(&net, &a))
}

/// Reconstruct from drawn entities and return the formatted report.
pub fn report_doc<'a>(entities: impl Iterator<Item = &'a EntityType>) -> Result<String, String> {
    let net = data::network_from_entities(entities)?;
    let (idf, opts) = default_params();
    let a = run_analysis(&net, &idf, &opts)?;
    Ok(format_analysis(&a))
}

/// The built-in demonstration network (properly sized, with plan coordinates).
fn demo() -> (Network, IdfCurve, AnalysisOptions) {
    let net = Network {
        nodes: vec![
            Node::inlet("N1", 104.0, 110.0, 1.0, 0.70).with_tc_inlet(12.0).at(0.0, 0.0),
            Node::inlet("N2", 102.5, 108.5, 1.0, 0.70).with_tc_inlet(10.0).at(300.0, 0.0),
            Node::junction("N3", 101.2, 107.0, 0.5, 0.80).with_tc_inlet(8.0).at(550.0, 0.0),
            Node::outfall("OUT", 100.0, 106.0).at(730.0, 0.0),
        ],
        pipes: vec![
            Pipe::new("P1", "N1", "N2", 300.0, 1.25, 0.013),
            Pipe::new("P2", "N2", "N3", 250.0, 1.50, 0.013),
            Pipe::new("P3", "N3", "OUT", 180.0, 1.75, 0.013),
        ],
    };
    let idf = IdfCurve::new(60.0, 10.0, 0.8);
    let opts = AnalysisOptions { tailwater: Some(100.5), ..Default::default() };
    (net, idf, opts)
}

fn v3(x: f64, y: f64) -> Vector3 {
    Vector3::new(x, y, 0.0)
}

fn seg(x1: f64, y1: f64, x2: f64, y2: f64) -> EntityType {
    EntityType::Line(Line::from_points(v3(x1, y1), v3(x2, y2)))
}

fn label(x: f64, y: f64, text: String, height: f64) -> EntityType {
    EntityType::MText(MText { value: text, insertion_point: v3(x, y), height, ..Default::default() })
}

fn circle(x: f64, y: f64, r: f64) -> EntityType {
    EntityType::Circle(Circle { center: v3(x, y), radius: r, ..Default::default() })
}

fn run_analysis(net: &Network, idf: &IdfCurve, opts: &AnalysisOptions) -> Result<Analysis, String> {
    net.analyze(idf, opts).map_err(|e| e.to_string())
}

/// Plan-view entities (pipes, structure markers, flow/HGL labels) + the report.
fn build_plan(net: &Network, a: &Analysis) -> (Vec<EntityType>, String) {
    let d = draw_network(net, a, &DrawConfig::default());
    let mut ents = Vec::new();
    for p in &d.plan_pipes {
        ents.push(seg(p.x1, p.y1, p.x2, p.y2));
    }
    for n in &d.plan_nodes {
        ents.push(circle(n.x, n.y, n.radius));
    }
    for l in &d.plan_labels {
        ents.push(label(l.x, l.y, l.text.clone(), l.height));
    }
    (ents, format_analysis(a))
}

/// HGL long-section entities (ground / invert / HGL polylines + labels).
fn build_profile(net: &Network, a: &Analysis) -> Vec<EntityType> {
    let d = draw_network(net, a, &DrawConfig::default());
    let mut ents = Vec::new();
    for pl in &d.profile_lines {
        for w in pl.pts.windows(2) {
            ents.push(seg(w[0].0, w[0].1, w[1].0, w[1].1));
        }
    }
    for l in &d.profile_labels {
        ents.push(label(l.x, l.y, l.text.clone(), l.height));
    }
    ents
}

// ── Public API: from a `.ssn` document ──────────────────────────────────────

/// Open a file dialog and read a `.ssn` network file.
///
/// Returns `None` if the user cancels, `Some(Ok(text))` on success, or
/// `Some(Err(msg))` if the chosen file cannot be read.
pub fn pick_ssn_file() -> Option<Result<String, String>> {
    let path = rfd::FileDialog::new()
        .add_filter("Storm Sewer Network", &["ssn"])
        .set_title("Open storm-sewer network (.ssn)")
        .pick_file()?;
    Some(std::fs::read_to_string(&path).map_err(|e| format!("cannot read {}: {e}", path.display())))
}

/// Parse a `.ssn` document, analyze it, and return (plan entities, report).
pub fn analyze_text(text: &str) -> Result<(Vec<EntityType>, String), String> {
    let p = parse_ssn(text)?;
    let a = run_analysis(&p.network, &p.idf, &p.options)?;
    Ok(build_plan(&p.network, &a))
}

/// Parse a `.ssn` document, analyze it, and return the HGL profile entities.
pub fn profile_text(text: &str) -> Result<Vec<EntityType>, String> {
    let p = parse_ssn(text)?;
    let a = run_analysis(&p.network, &p.idf, &p.options)?;
    Ok(build_profile(&p.network, &a))
}

/// Parse a `.ssn` document, analyze it, and return the formatted report.
pub fn report_text_from(text: &str) -> Result<String, String> {
    let p = parse_ssn(text)?;
    let a = run_analysis(&p.network, &p.idf, &p.options)?;
    Ok(format_analysis(&a))
}

// ── Built-in sample fallbacks (used by tests) ───────────────────────────────

/// Plan entities + report for the built-in sample network.
pub fn analyze_plan() -> Result<(Vec<EntityType>, String), String> {
    let (net, idf, opts) = demo();
    let a = run_analysis(&net, &idf, &opts)?;
    Ok(build_plan(&net, &a))
}

/// Profile entities for the built-in sample network.
pub fn analyze_profile() -> Result<Vec<EntityType>, String> {
    let (net, idf, opts) = demo();
    let a = run_analysis(&net, &idf, &opts)?;
    Ok(build_profile(&net, &a))
}

/// Formatted report for the built-in sample network.
pub fn report_text() -> String {
    let (net, idf, opts) = demo();
    match run_analysis(&net, &idf, &opts) {
        Ok(a) => format_analysis(&a),
        Err(e) => format!("storm-sewer analysis error: {e}"),
    }
}

/// Back-compat alias used by the module's integration test.
#[allow(dead_code)]
pub fn demo_report() -> String {
    report_text()
}
