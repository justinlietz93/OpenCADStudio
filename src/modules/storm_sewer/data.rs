// Storm-sewer data carried on drawing entities via XDATA, and reconstruction
// of a `stormsewer::Network` from the drawn entities.
//
// Structures are circles tagged with the `STORMSEWER_STRUCT` app record
// [kind, invert, rim, area, C]; pipes are lines tagged with `STORMSEWER_PIPE`
// [diameter, n, from-handle, to-handle]. Connectivity is by entity handle, so
// the drawn network round-trips to DWG/DXF and is analyzable directly.

use std::collections::HashMap;

use acadrust::xdata::{ExtendedDataRecord, XDataValue};
use acadrust::{EntityType, Handle};

use stormsewer::network::{Network, Node, NodeKind, Pipe};

pub const APP_STRUCT: &str = "STORMSEWER_STRUCT";
pub const APP_PIPE: &str = "STORMSEWER_PIPE";

pub fn kind_str(k: NodeKind) -> &'static str {
    match k {
        NodeKind::Inlet => "inlet",
        NodeKind::Junction => "junction",
        NodeKind::Outfall => "outfall",
    }
}

fn parse_kind(s: &str) -> NodeKind {
    match s {
        "outfall" => NodeKind::Outfall,
        "junction" => NodeKind::Junction,
        _ => NodeKind::Inlet,
    }
}

/// XDATA record for a structure marker.
pub fn structure_xdata(kind: NodeKind, invert: f64, rim: f64, area: f64, c: f64) -> ExtendedDataRecord {
    let mut r = ExtendedDataRecord::new(APP_STRUCT);
    r.add_value(XDataValue::String(kind_str(kind).to_string()));
    r.add_value(XDataValue::Real(invert));
    r.add_value(XDataValue::Real(rim));
    r.add_value(XDataValue::Real(area));
    r.add_value(XDataValue::Real(c));
    r
}

/// XDATA record for a pipe, linking the two structures it connects by handle.
pub fn pipe_xdata(diameter: f64, n: f64, from: Handle, to: Handle) -> ExtendedDataRecord {
    let mut r = ExtendedDataRecord::new(APP_PIPE);
    r.add_value(XDataValue::Real(diameter));
    r.add_value(XDataValue::Real(n));
    r.add_value(XDataValue::Handle(from));
    r.add_value(XDataValue::Handle(to));
    r
}

fn real(v: &XDataValue) -> Option<f64> {
    if let XDataValue::Real(x) = v {
        Some(*x)
    } else {
        None
    }
}

fn handle(v: &XDataValue) -> Option<Handle> {
    if let XDataValue::Handle(h) = v {
        Some(*h)
    } else {
        None
    }
}

struct StructRec {
    handle: Handle,
    kind: NodeKind,
    invert: f64,
    rim: f64,
    area: f64,
    c: f64,
    x: f64,
    y: f64,
}

struct PipeRec {
    diameter: f64,
    n: f64,
    from: Handle,
    to: Handle,
    length: f64,
}

fn read_structure(e: &EntityType) -> Option<StructRec> {
    let rec = e.common().extended_data.get_record(APP_STRUCT)?;
    if rec.values.len() < 5 {
        return None;
    }
    let kind = match &rec.values[0] {
        XDataValue::String(s) => parse_kind(s),
        _ => return None,
    };
    let (x, y) = match e {
        EntityType::Circle(c) => (c.center.x, c.center.y),
        _ => return None,
    };
    Some(StructRec {
        handle: e.common().handle,
        kind,
        invert: real(&rec.values[1])?,
        rim: real(&rec.values[2])?,
        area: real(&rec.values[3])?,
        c: real(&rec.values[4])?,
        x,
        y,
    })
}

fn read_pipe(e: &EntityType) -> Option<PipeRec> {
    let rec = e.common().extended_data.get_record(APP_PIPE)?;
    if rec.values.len() < 4 {
        return None;
    }
    let length = match e {
        EntityType::Line(l) => {
            let dx = l.end.x - l.start.x;
            let dy = l.end.y - l.start.y;
            (dx * dx + dy * dy).sqrt()
        }
        _ => return None,
    };
    Some(PipeRec {
        diameter: real(&rec.values[0])?,
        n: real(&rec.values[1])?,
        from: handle(&rec.values[2])?,
        to: handle(&rec.values[3])?,
        length,
    })
}

/// Build a `stormsewer::Network` from drawn entities. Structures become nodes
/// (named N1, N2, … in encounter order); pipes become links, mapped to nodes
/// by the handles stored in their XDATA.
pub fn network_from_entities<'a>(entities: impl Iterator<Item = &'a EntityType>) -> Result<Network, String> {
    let mut structs: Vec<StructRec> = Vec::new();
    let mut pipes_raw: Vec<PipeRec> = Vec::new();
    for e in entities {
        if let Some(s) = read_structure(e) {
            structs.push(s);
        } else if let Some(p) = read_pipe(e) {
            pipes_raw.push(p);
        }
    }
    if structs.is_empty() {
        return Err("No storm-sewer structures in the drawing — place Inlet/Junction/Outfall first.".into());
    }

    let mut id_of: HashMap<u64, String> = HashMap::new();
    let mut nodes = Vec::with_capacity(structs.len());
    for (idx, s) in structs.iter().enumerate() {
        let id = format!("N{}", idx + 1);
        id_of.insert(s.handle.value(), id.clone());
        let node = match s.kind {
            NodeKind::Inlet => Node::inlet(&id, s.invert, s.rim, s.area, s.c),
            NodeKind::Junction => Node::junction(&id, s.invert, s.rim, s.area, s.c),
            NodeKind::Outfall => Node::outfall(&id, s.invert, s.rim),
        }
        .at(s.x, s.y);
        nodes.push(node);
    }

    let mut pipes = Vec::new();
    let mut dropped = 0;
    for (k, p) in pipes_raw.iter().enumerate() {
        match (id_of.get(&p.from.value()), id_of.get(&p.to.value())) {
            (Some(f), Some(t)) => {
                pipes.push(Pipe::new(&format!("P{}", k + 1), f, t, p.length, p.diameter, p.n));
            }
            _ => dropped += 1,
        }
    }
    if pipes.is_empty() {
        return Err(format!(
            "No connected storm-sewer pipes ({} structure(s) found, {} dangling pipe(s)). Use Pipe Run to connect structures.",
            structs.len(), dropped
        ));
    }
    Ok(Network { nodes, pipes })
}

#[cfg(test)]
mod tests {
    use super::*;
    use acadrust::types::Vector3;
    use acadrust::{Circle, Line};

    fn structure(h: u64, kind: NodeKind, x: f64, invert: f64) -> EntityType {
        let mut e = EntityType::Circle(Circle { center: Vector3::new(x, 0.0, 0.0), radius: 3.0, ..Default::default() });
        e.common_mut().handle = Handle::new(h);
        e.common_mut().extended_data.add_record(structure_xdata(kind, invert, invert + 6.0, 1.0, 0.7));
        e
    }
    fn pipe(from: u64, to: u64, x1: f64, x2: f64) -> EntityType {
        let mut e = EntityType::Line(Line::from_points(Vector3::new(x1, 0.0, 0.0), Vector3::new(x2, 0.0, 0.0)));
        e.common_mut().extended_data.add_record(pipe_xdata(1.5, 0.013, Handle::new(from), Handle::new(to)));
        e
    }

    #[test]
    fn reconstructs_network_from_tagged_entities() {
        let ents = vec![
            structure(1, NodeKind::Inlet, 0.0, 104.0),
            structure(2, NodeKind::Outfall, 100.0, 100.0),
            pipe(1, 2, 0.0, 100.0),
        ];
        let net = network_from_entities(ents.iter()).unwrap();
        assert_eq!(net.nodes.len(), 2);
        assert_eq!(net.pipes.len(), 1);
        assert_eq!(net.pipes[0].from, "N1");
        assert_eq!(net.pipes[0].to, "N2");
        assert!((net.pipes[0].length - 100.0).abs() < 1e-6);
        assert!((net.pipes[0].diameter - 1.5).abs() < 1e-9);
    }

    #[test]
    fn errors_when_no_structures() {
        let ents: Vec<EntityType> = vec![];
        assert!(network_from_entities(ents.iter()).is_err());
    }

    #[test]
    fn dangling_pipe_is_reported() {
        let ents = vec![
            structure(1, NodeKind::Inlet, 0.0, 104.0),
            pipe(1, 99, 0.0, 100.0), // 99 doesn't exist
        ];
        assert!(network_from_entities(ents.iter()).is_err());
    }
}
