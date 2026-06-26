// Explode tool — ribbon definition + command implementation.
//
// Command:  EXPLODE (X)
//   EXPLODE: Breaks compound objects into their constituent simple entities.
//
//   Supported:
//     LwPolyline  → Lines (straight segments) + Arcs (bulge segments)
//     Polyline2D  → Lines + Arcs
//     Polyline3D  → Lines
//     Polyline    → Lines
//     Insert      → constituent entities (via acadrust explode_from_document)
//     MLine       → Lines (spine + offset lines per miter direction)
//     Dimension   → Lines (extension + dimension + arrows) + Text
//
//   Unsupported entity types are skipped silently.

use std::f64::consts::TAU;

use acadrust::entities::EntityCommon;
use acadrust::entities::{
    Arc as ArcEnt, Block, BlockEnd, Circle as CircleEnt, Dimension, Line as LineEnt, LwPolyline,
    MLine,
};
use acadrust::entities::{Polyline, Polyline2D};
use acadrust::tables::BlockRecord;
use acadrust::types::Vector3;
use acadrust::{CadDocument, EntityType, Handle};

use crate::command::{CadCommand, CmdResult};
use crate::modules::{IconKind, ModuleEvent, ToolDef};
use glam::DVec3;

// ── Ribbon definition ──────────────────────────────────────────────────────

pub fn tool() -> ToolDef {
    ToolDef {
        id: "EXPLODE",
        label: "Explode",
        icon: IconKind::Svg(include_bytes!("../../../../assets/icons/explode.svg")),
        event: ModuleEvent::Command("EXPLODE".to_string()),
    }
}

// ── Geometry helpers ────────────────────────────────────────────────────────

/// Explode just the polyline family (LwPolyline / Polyline / Polyline2D /
/// Polyline3D) into Line + Arc segments. No document needed — used where a
/// polyline must be treated as its constituent edges (e.g. TRIM boundaries).
/// Returns empty for any other entity type.
pub fn explode_polyline_segments(entity: &EntityType) -> Vec<EntityType> {
    match entity {
        EntityType::LwPolyline(p) => explode_lwpolyline(p),
        EntityType::Polyline2D(p) => explode_polyline2d(p),
        EntityType::Polyline(p) => explode_polyline(p),
        EntityType::Polyline3D(p) => explode_polyline3d(p),
        _ => vec![],
    }
}

/// Decompose an entity into its constituent simple entities.
/// Returns an empty vec if the entity cannot be exploded.
pub fn explode_entity(entity: &EntityType, document: &CadDocument) -> Vec<EntityType> {
    match entity {
        EntityType::LwPolyline(p) => explode_lwpolyline(p),
        EntityType::Polyline2D(p) => explode_polyline2d(p),
        EntityType::Polyline(p) => explode_polyline(p),
        EntityType::Polyline3D(p) => explode_polyline3d(p),
        EntityType::Insert(ins) => ins
            .explode_from_document(document)
            .into_iter()
            .map(normalize_insert_entity)
            .collect(),
        EntityType::MLine(ml) => explode_mline(ml),
        EntityType::Dimension(dim) => explode_dimension(dim),
        _ => vec![],
    }
}

fn explode_polyline(p: &Polyline) -> Vec<EntityType> {
    let n = p.vertices.len();
    if n < 2 {
        return vec![];
    }
    let closed = p.flags.is_closed();
    let n_segs = if closed { n } else { n - 1 };
    let mut result = Vec::new();
    for i in 0..n_segs {
        let v0 = &p.vertices[i];
        let v1 = &p.vertices[(i + 1) % n];
        let mut common = p.common.clone();
        common.handle = Handle::NULL;
        result.push(EntityType::Line(LineEnt {
            common,
            start: v0.location.clone(),
            end: v1.location.clone(),
            ..LineEnt::new()
        }));
    }
    result
}

fn explode_polyline3d(p: &acadrust::entities::Polyline3D) -> Vec<EntityType> {
    let n = p.vertices.len();
    if n < 2 {
        return vec![];
    }
    let closed = p.is_closed();
    let n_segs = if closed { n } else { n - 1 };
    let mut result = Vec::new();
    for i in 0..n_segs {
        let v0 = &p.vertices[i];
        let v1 = &p.vertices[(i + 1) % n];
        let mut common = p.common.clone();
        common.handle = Handle::NULL;
        result.push(EntityType::Line(LineEnt {
            common,
            start: v0.position.clone(),
            end: v1.position.clone(),
            ..LineEnt::new()
        }));
    }
    result
}

fn explode_polyline2d(p: &Polyline2D) -> Vec<EntityType> {
    let n = p.vertices.len();
    if n < 2 {
        return vec![];
    }
    let closed = p.is_closed();
    let n_segs = if closed { n } else { n - 1 };
    let elevation = p.elevation;

    let mut result = Vec::new();
    for i in 0..n_segs {
        let v0 = &p.vertices[i];
        let v1 = &p.vertices[(i + 1) % n];
        let p0 = [v0.location.x, v0.location.y];
        let p1 = [v1.location.x, v1.location.y];

        if v0.bulge.abs() < 1e-10 {
            let mut common = p.common.clone();
            common.handle = Handle::NULL;
            result.push(EntityType::Line(LineEnt {
                common,
                start: Vector3::new(p0[0], p0[1], elevation),
                end: Vector3::new(p1[0], p1[1], elevation),
                ..LineEnt::new()
            }));
        } else if let Some(arc) = bulge_to_arc(p0, p1, v0.bulge, elevation, &p.common) {
            result.push(arc);
        }
    }
    result
}

pub fn normalize_insert_entity(mut entity: EntityType) -> EntityType {
    match &mut entity {
        EntityType::Ellipse(ell) => {
            let major_len = ell.major_axis_length();
            let full_span = {
                let mut span = ell.end_parameter - ell.start_parameter;
                if span < 0.0 {
                    span += std::f64::consts::TAU;
                }
                (span - std::f64::consts::TAU).abs() < 1e-6
            };
            if (ell.minor_axis_ratio - 1.0).abs() < 1e-6 && full_span {
                let mut circle = CircleEnt::new();
                circle.common = ell.common.clone();
                circle.center = ell.center;
                circle.radius = major_len;
                circle.normal = ell.normal;
                entity = EntityType::Circle(circle);
            }
        }
        _ => {}
    }

    entity.common_mut().handle = Handle::NULL;
    entity.common_mut().owner_handle = Handle::NULL;
    entity
}

pub fn normalize_entity_for_block(entity: EntityType) -> EntityType {
    entity
}

fn explode_lwpolyline(p: &LwPolyline) -> Vec<EntityType> {
    let n = p.vertices.len();
    if n < 2 {
        return vec![];
    }

    let elevation = p.elevation;
    let n_segs = if p.is_closed { n } else { n - 1 };

    let mut result = Vec::new();
    for i in 0..n_segs {
        let v0 = &p.vertices[i];
        let v1 = &p.vertices[(i + 1) % n];

        let p0 = [v0.location.x, v0.location.y];
        let p1 = [v1.location.x, v1.location.y];

        if v0.bulge.abs() < 1e-10 {
            // Straight segment → Line
            let mut common = p.common.clone();
            common.handle = Handle::NULL;
            let line = LineEnt {
                common,
                start: Vector3::new(p0[0], p0[1], elevation),
                end: Vector3::new(p1[0], p1[1], elevation),
                ..LineEnt::new()
            };
            result.push(EntityType::Line(line));
        } else {
            // Arc segment from bulge
            if let Some(arc) = bulge_to_arc(p0, p1, v0.bulge, elevation, &p.common) {
                result.push(arc);
            }
        }
    }
    result
}

/// Convert a polyline bulge segment to an Arc entity.
///   Arc angles are measured from the +X axis.
fn bulge_to_arc(
    p0: [f64; 2],
    p1: [f64; 2],
    bulge: f64,
    elevation: f64,
    common_src: &EntityCommon,
) -> Option<EntityType> {
    let ba = crate::entities::common::BulgeArc::from_bulge(p0, p1, bulge)?;

    // acadrust Arc is always CCW from start_angle to end_angle. Negative
    // bulge means the polyline goes p0→p1 the CW way around the centre,
    // which is the same circular arc traversed p1→p0 the CCW way — so
    // swap endpoints when bulge < 0.
    let (start_angle, end_angle) = if bulge > 0.0 {
        (norm_angle(ba.start_angle), norm_angle(ba.end_angle))
    } else {
        (norm_angle(ba.end_angle), norm_angle(ba.start_angle))
    };

    let mut common = common_src.clone();
    common.handle = Handle::NULL;

    let arc = ArcEnt {
        common,
        center: Vector3::new(ba.center[0], ba.center[1], elevation),
        radius: ba.radius,
        start_angle,
        end_angle,
        ..ArcEnt::new()
    };
    Some(EntityType::Arc(arc))
}

fn norm_angle(a: f64) -> f64 {
    ((a % TAU) + TAU) % TAU
}

fn explode_mline(ml: &MLine) -> Vec<EntityType> {
    let n = ml.vertices.len();
    if n < 2 {
        return vec![];
    }
    let closed = ml.flags.contains(acadrust::entities::MLineFlags::CLOSED);
    let scale = ml.scale_factor;
    let n_segs = if closed { n } else { n - 1 };
    let mut result = Vec::new();

    // Helper: build a Line from two Vector3 positions.
    let make_line = |common: &acadrust::entities::EntityCommon,
                     s: &acadrust::types::Vector3,
                     e: &acadrust::types::Vector3|
     -> EntityType {
        let mut c = common.clone();
        c.handle = Handle::NULL;
        EntityType::Line(LineEnt {
            common: c,
            start: s.clone(),
            end: e.clone(),
            ..LineEnt::new()
        })
    };

    // For each segment, emit the center-spine line and the two ±scale/2 offset lines.
    for i in 0..n_segs {
        let v0 = &ml.vertices[i];
        let v1 = &ml.vertices[(i + 1) % n];

        // Spine line
        result.push(make_line(&ml.common, &v0.position, &v1.position));

        if scale.abs() > 1e-9 {
            let half = scale * 0.5;
            for &sign in &[-1.0_f64, 1.0_f64] {
                let off = half * sign;
                // Use miter direction at each vertex to offset the endpoints.
                let s = Vector3::new(
                    v0.position.x + v0.miter.x * off,
                    v0.position.y + v0.miter.y * off,
                    v0.position.z + v0.miter.z * off,
                );
                let e = Vector3::new(
                    v1.position.x + v1.miter.x * off,
                    v1.position.y + v1.miter.y * off,
                    v1.position.z + v1.miter.z * off,
                );
                result.push(make_line(&ml.common, &s, &e));
            }
        }
    }

    result
}

// ── Dimension explode ──────────────────────────────────────────────────────

/// Convert a Dimension entity into Lines (geometry) + Text (label).
fn explode_dimension(dim: &Dimension) -> Vec<EntityType> {
    use acadrust::entities::Text;

    let base = dim.base();
    let common = base.common.clone();
    let mut result: Vec<EntityType> = Vec::new();

    // Helper: make a line segment
    let make_seg = |a: &Vector3, b: &Vector3, common: &EntityCommon| -> EntityType {
        let mut c = common.clone();
        c.handle = Handle::NULL;
        EntityType::Line(LineEnt {
            common: c,
            start: a.clone(),
            end: b.clone(),
            ..LineEnt::new()
        })
    };

    let v3 = |x: f64, y: f64, z: f64| Vector3::new(x, y, z);

    match dim {
        Dimension::Aligned(d) => {
            let fx = d.first_point.x;
            let fy = d.first_point.y;
            let sx = d.second_point.x;
            let sy = d.second_point.y;
            let dx_s = sx - fx;
            let dy_s = sy - fy;
            let len = (dx_s * dx_s + dy_s * dy_s).sqrt().max(1e-12);
            let axis_angle = dy_s.atan2(dx_s);
            let perp_x = -(axis_angle.sin());
            let perp_y = axis_angle.cos();
            let offset =
                (d.definition_point.x - fx) * perp_x + (d.definition_point.y - fy) * perp_y;
            let d1 = v3(fx + perp_x * offset, fy + perp_y * offset, d.first_point.z);
            let d2 = v3(sx + perp_x * offset, sy + perp_y * offset, d.second_point.z);
            result.push(make_seg(&d.first_point, &d1, &common));
            result.push(make_seg(&d.second_point, &d2, &common));
            result.push(make_seg(&d1, &d2, &common));
            let _ = len;
        }
        Dimension::Linear(d) => {
            let angle = d.rotation.to_radians();
            let perp_x = -(angle.sin());
            let perp_y = angle.cos();
            let fx = d.first_point.x;
            let fy = d.first_point.y;
            let sx = d.second_point.x;
            let sy = d.second_point.y;
            let offset =
                (d.definition_point.x - fx) * perp_x + (d.definition_point.y - fy) * perp_y;
            let d1 = v3(fx + perp_x * offset, fy + perp_y * offset, d.first_point.z);
            let d2 = v3(sx + perp_x * offset, sy + perp_y * offset, d.second_point.z);
            result.push(make_seg(&d.first_point, &d1, &common));
            result.push(make_seg(&d.second_point, &d2, &common));
            result.push(make_seg(&d1, &d2, &common));
        }
        Dimension::Radius(d) => {
            result.push(make_seg(&d.angle_vertex, &d.definition_point, &common));
        }
        Dimension::Diameter(d) => {
            result.push(make_seg(&d.angle_vertex, &d.definition_point, &common));
        }
        Dimension::Angular2Ln(d) => {
            result.push(make_seg(&d.first_point, &d.angle_vertex, &common));
            result.push(make_seg(&d.second_point, &d.angle_vertex, &common));
        }
        Dimension::Angular3Pt(d) => {
            result.push(make_seg(&d.first_point, &d.angle_vertex, &common));
            result.push(make_seg(&d.second_point, &d.angle_vertex, &common));
        }
        Dimension::Ordinate(d) => {
            result.push(make_seg(&d.feature_location, &d.definition_point, &common));
            result.push(make_seg(&d.definition_point, &d.leader_endpoint, &common));
        }
    }

    // Text entity for the dimension label
    let text_val = if let Some(u) = &base.user_text {
        if !u.trim().is_empty() {
            u.clone()
        } else {
            format!("{:.4}", dim.measurement())
        }
    } else if !base.text.trim().is_empty() {
        base.text.clone()
    } else {
        match dim {
            Dimension::Radius(_) => format!("R{:.4}", dim.measurement()),
            Dimension::Diameter(_) => format!("Ø{:.4}", dim.measurement()),
            Dimension::Angular2Ln(_) | Dimension::Angular3Pt(_) => {
                format!("{:.2}°", dim.measurement())
            }
            _ => format!("{:.4}", dim.measurement()),
        }
    };

    let mut text = Text::with_value(text_val, base.text_middle_point.clone())
        .with_height(base.line_spacing_factor.abs().max(0.1))
        .with_rotation(base.text_rotation);
    text.common = common.clone();
    text.common.handle = Handle::NULL;
    result.push(EntityType::Text(text));

    result
}

// ── Dimension block baking (DWG/DXF interop) ────────────────────────────────

/// Smallest free `*D<n>` anonymous block name in `doc`.
fn next_dimension_block_name(doc: &CadDocument) -> String {
    let mut n = 0u64;
    loop {
        let cand = format!("*D{n}");
        if doc.block_records.get(&cand).is_none() {
            return cand;
        }
        n += 1;
    }
}

/// Bake an anonymous `*D<n>` geometry block for every DIMENSION that doesn't
/// already own one, so the file is valid for AutoCAD-family readers.
///
/// OCS renders dimensions by re-tessellating them on the fly and never
/// materialises the `*D` block that a DWG `DIMENSION` is supposed to reference
/// (the lines / arrows / text that AutoCAD actually draws). A dimension created
/// in OCS therefore goes out referencing a block that doesn't exist, and the
/// writer emits a null block handle — strict readers (DWG TrueView, QCAD) drop
/// the dimension or demand a recovery, and lenient ones (BricsCAD) regenerate it
/// at a different position. Call this on the document about to be written so each
/// such dimension gets a real block built from its exploded geometry (extension
/// lines + dimension line + measurement text, the same decomposition EXPLODE
/// uses) and its `block_name` points at it.
///
/// Dimensions that already reference an existing block (e.g. imported from a real
/// DWG, or copied via the `*D`-cloning copy path) are left untouched so their
/// original graphics are preserved.
pub fn bake_dimension_blocks(doc: &mut CadDocument) {
    // Handles of dimensions whose block reference is missing or dangling.
    let pending: Vec<Handle> = doc
        .entities()
        .filter_map(|e| match e {
            EntityType::Dimension(d) => {
                let bn = &d.base().block_name;
                if bn.trim().is_empty() || doc.block_records.get(bn).is_none() {
                    Some(d.base().common.handle)
                } else {
                    None
                }
            }
            _ => None,
        })
        .collect();

    for handle in pending {
        let dim = match doc.get_entity(handle) {
            Some(EntityType::Dimension(d)) => d.clone(),
            _ => continue,
        };
        let subs = explode_dimension(&dim);
        if subs.is_empty() {
            continue;
        }

        let name = next_dimension_block_name(doc);
        // Reserve three consecutive handles for the record / block / endblk.
        // Adding the block + endblk (which carry explicit handles) advances the
        // document's handle counter past them, so the NULL-handle sub-entities
        // added afterwards get fresh handles without colliding.
        let next = doc.next_handle();
        let br_handle = Handle::new(next);
        let block_handle = Handle::new(next + 1);
        let end_handle = Handle::new(next + 2);

        let mut br = BlockRecord::new(&name);
        br.handle = br_handle;
        br.block_entity_handle = block_handle;
        br.block_end_handle = end_handle;
        br.flags.anonymous = true;
        if doc.block_records.add(br).is_err() {
            continue;
        }

        let mut block = Block::new(&name, Vector3::new(0.0, 0.0, 0.0));
        block.common.handle = block_handle;
        block.common.owner_handle = br_handle;
        let _ = doc.add_entity(EntityType::Block(block));

        let mut block_end = BlockEnd::new();
        block_end.common.handle = end_handle;
        block_end.common.owner_handle = br_handle;
        let _ = doc.add_entity(EntityType::BlockEnd(block_end));

        for mut sub in subs {
            sub.common_mut().handle = Handle::NULL;
            sub.common_mut().owner_handle = br_handle;
            let _ = doc.add_entity(sub);
        }

        if let Some(EntityType::Dimension(d)) = doc.get_entity_mut(handle) {
            d.base_mut().block_name = name;
        }
    }
}

// ── Command stub (kept for future interactive selection mode) ───────────────

pub struct ExplodeCommand;

impl ExplodeCommand {
    #[allow(dead_code)]
    pub fn new() -> Self {
        Self
    }
}

impl CadCommand for ExplodeCommand {
    fn name(&self) -> &'static str {
        "EXPLODE"
    }
    fn prompt(&self) -> String {
        "EXPLODE  Select objects to explode:".into()
    }

    fn on_point(&mut self, _pt: DVec3) -> CmdResult {
        CmdResult::Cancel
    }
    fn on_enter(&mut self) -> CmdResult {
        CmdResult::Cancel
    }
    fn on_escape(&mut self) -> CmdResult {
        CmdResult::Cancel
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use acadrust::entities::DimensionLinear;

    /// A dimension created without a geometry block gets a real `*D` block on
    /// bake, its `block_name` resolves to that block, and a second bake is a
    /// no-op (already has a valid block).
    #[test]
    fn bakes_block_for_blockless_dimension_and_is_idempotent() {
        let mut doc = CadDocument::new();

        let mut d = DimensionLinear::new(Vector3::new(0.0, 0.0, 0.0), Vector3::new(10.0, 0.0, 0.0));
        d.definition_point = Vector3::new(0.0, 5.0, 0.0);
        d.base.text_middle_point = Vector3::new(5.0, 5.0, 0.0);
        // block_name is left empty — exactly what OCS-created dimensions carry.
        let handle = doc
            .add_entity(EntityType::Dimension(Dimension::Linear(d)))
            .unwrap();

        bake_dimension_blocks(&mut doc);

        let block_name = match doc.get_entity(handle) {
            Some(EntityType::Dimension(d)) => d.base().block_name.clone(),
            _ => panic!("dimension missing"),
        };
        assert!(!block_name.trim().is_empty(), "block_name should be set");
        assert!(
            doc.block_records.get(&block_name).is_some(),
            "baked block must exist in the block table"
        );

        // Second pass must not create another block for the same dimension.
        let before = doc.block_records.len();
        bake_dimension_blocks(&mut doc);
        assert_eq!(
            doc.block_records.len(),
            before,
            "a dimension that already owns a block must not be re-baked"
        );
    }
}
