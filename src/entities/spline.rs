use acadrust::entities::Spline;
use truck_modeling::{
    base::{BoundedCurve, ParametricCurve, Vector4},
    builder, BSplineCurve, Curve, Edge, KnotVec, NurbsCurve, Point3, Wire,
};

use crate::command::EntityTransform;
use crate::entities::common::{edit_prop as edit, parse_f64, ro_prop as ro, square_grip};
use crate::entities::traits::TruckConvertible;
use crate::scene::convert::acad_to_truck::{TruckEntity, TruckObject};
use crate::scene::model::object::{GripApply, GripDef, PropSection};

fn to_truck(spl: &Spline) -> TruckEntity {
    let n = spl.control_points.len();
    if n < 2 {
        return TruckEntity {
            object: TruckObject::Point(builder::vertex(Point3::new(0.0, 0.0, 0.0))),
            snap_pts: vec![],
            tangent_geoms: vec![],
            key_vertices: vec![],
            fill_tris: vec![],
        };
    }

    let knot_vec = if !spl.knots.is_empty() {
        KnotVec::from(spl.knots.clone())
    } else {
        KnotVec::uniform_knot(spl.degree as usize, n - 1)
    };

    // Use rational NURBS when weights are provided (circles/conics stored as NURBS).
    let use_nurbs = !spl.weights.is_empty() && spl.weights.len() == n;
    let (p_start, p_end, curve) = if use_nurbs {
        let homo_pts: Vec<Vector4> = spl
            .control_points
            .iter()
            .zip(spl.weights.iter())
            .map(|(p, &w)| {
                let w = if w.abs() < 1e-12 { 1.0 } else { w };
                Vector4::new(p.x * w, p.y * w, p.z * w, w)
            })
            .collect();
        let nurbs = NurbsCurve::new(BSplineCurve::new(knot_vec, homo_pts));
        let (t0, t1) = nurbs.range_tuple();
        (nurbs.subs(t0), nurbs.subs(t1), Curve::NurbsCurve(nurbs))
    } else {
        let ctrl_pts: Vec<Point3> = spl
            .control_points
            .iter()
            .map(|p| Point3::new(p.x, p.y, p.z))
            .collect();
        let bspline = BSplineCurve::new(knot_vec, ctrl_pts);
        let (t0, t1) = bspline.range_tuple();
        (
            bspline.subs(t0),
            bspline.subs(t1),
            Curve::BSplineCurve(bspline),
        )
    };

    let snap_source = if !spl.fit_points.is_empty() {
        &spl.fit_points
    } else {
        &spl.control_points
    };
    let key_vertices: Vec<[f64; 3]> = snap_source.iter().map(|p| [p.x, p.y, p.z]).collect();

    let is_closed = spl.flags.closed || spl.flags.periodic;
    let gap = {
        let dx = (p_end.x - p_start.x) as f32;
        let dy = (p_end.y - p_start.y) as f32;
        let dz = (p_end.z - p_start.z) as f32;
        (dx * dx + dy * dy + dz * dz).sqrt()
    };

    let object = if is_closed && gap > 1e-6 {
        let v_start = builder::vertex(p_start);
        let v_end = builder::vertex(p_end);
        let v_close = builder::vertex(p_start);
        let main_edge = Edge::new(&v_start, &v_end, curve);
        let close_edge = builder::line(&v_end, &v_close);
        let wire: Wire = [main_edge, close_edge].into_iter().collect();
        TruckObject::Contour(wire)
    } else {
        let v_start = builder::vertex(p_start);
        let v_end = builder::vertex(p_end);
        let edge = Edge::new(&v_start, &v_end, curve);
        TruckObject::Curve(edge)
    };

    TruckEntity {
        object,
        snap_pts: vec![],
        tangent_geoms: vec![],
        key_vertices,
        fill_tris: vec![],
    }
}

fn grips(spline: &Spline) -> Vec<GripDef> {
    spline
        .control_points
        .iter()
        .enumerate()
        .map(|(i, p)| square_grip(i, glam::DVec3::new(p.x, p.y, p.z)))
        .collect()
}

fn properties(spline: &Spline) -> Vec<PropSection> {
    let show = if spline.fit_points.is_empty() {
        "Control Vertices"
    } else {
        "Fit Points"
    };
    let cp0 = spline.control_points.first();
    let fp0 = spline.fit_points.first();
    let w0 = spline.weights.first().copied();

    // Polyline-approximation length over the effective defining points.
    let pts = if spline.fit_points.is_empty() {
        &spline.control_points
    } else {
        &spline.fit_points
    };
    let mut length = 0.0;
    for w in pts.windows(2) {
        let dx = w[1].x - w[0].x;
        let dy = w[1].y - w[0].y;
        let dz = w[1].z - w[0].z;
        length += (dx * dx + dy * dy + dz * dz).sqrt();
    }

    let closed = spline.flags.closed || spline.flags.periodic;
    let yes_no = |b: bool| if b { "Yes" } else { "No" };

    vec![
        PropSection {
            title: "Data Points".into(),
            props: vec![
                ro("Show", "show", show),
                edit("Degree", "degree", spline.degree as f64),
                ro(
                    "Control Point Count",
                    "ctrl_pt_count",
                    spline.control_points.len().to_string(),
                ),
                ro("Control Point", "ctrl_pt_index", "0"),
                ro(
                    "Control Point X",
                    "ctrl_pt_x",
                    format!("{:.4}", cp0.map(|p| p.x).unwrap_or(0.0)),
                ),
                ro(
                    "Control Point Y",
                    "ctrl_pt_y",
                    format!("{:.4}", cp0.map(|p| p.y).unwrap_or(0.0)),
                ),
                ro(
                    "Control Point Z",
                    "ctrl_pt_z",
                    format!("{:.4}", cp0.map(|p| p.z).unwrap_or(0.0)),
                ),
                ro("Weight", "weight", format!("{:.4}", w0.unwrap_or(1.0))),
                ro("Knot Parameterization", "knot_param", String::new()),
                ro(
                    "Fit Point Count",
                    "fit_pt_count",
                    spline.fit_points.len().to_string(),
                ),
                ro("Fit Point", "fit_pt_index", "0"),
                ro(
                    "Fit Point X",
                    "fit_pt_x",
                    format!("{:.4}", fp0.map(|p| p.x).unwrap_or(0.0)),
                ),
                ro(
                    "Fit Point Y",
                    "fit_pt_y",
                    format!("{:.4}", fp0.map(|p| p.y).unwrap_or(0.0)),
                ),
                ro(
                    "Fit Point Z",
                    "fit_pt_z",
                    format!("{:.4}", fp0.map(|p| p.z).unwrap_or(0.0)),
                ),
                ro("Fit Tolerance", "fit_tolerance", format!("{:.6}", spline.fit_tolerance)),
                ro("Start Tangent X", "start_tan_x", format!("{:.4}", spline.begin_tangent.x)),
                ro("Start Tangent Y", "start_tan_y", format!("{:.4}", spline.begin_tangent.y)),
                ro("Start Tangent Z", "start_tan_z", format!("{:.4}", spline.begin_tangent.z)),
                ro("End Tangent X", "end_tan_x", format!("{:.4}", spline.end_tangent.x)),
                ro("End Tangent Y", "end_tan_y", format!("{:.4}", spline.end_tangent.y)),
                ro("End Tangent Z", "end_tan_z", format!("{:.4}", spline.end_tangent.z)),
            ],
        },
        PropSection {
            title: "Misc".into(),
            props: vec![
                ro("Closed", "closed", yes_no(closed)),
                ro("Planar", "planar", yes_no(spline.flags.planar)),
                ro("Length", "length", format!("{length:.4}")),
                ro("Area", "area", String::new()),
            ],
        },
    ]
}

fn apply_geom_prop(spline: &mut Spline, field: &str, value: &str) {
    if field == "degree" {
        if let Some(v) = parse_f64(value) {
            spline.degree = v.round() as i32;
        }
    }
}

fn apply_grip(spline: &mut Spline, grip_id: usize, apply: GripApply) {
    if let Some(cp) = spline.control_points.get_mut(grip_id) {
        match apply {
            GripApply::Absolute(p) => {
                cp.x = p.x as f64;
                cp.y = p.y as f64;
                cp.z = p.z as f64;
            }
            GripApply::Translate(d) => {
                cp.x += d.x as f64;
                cp.y += d.y as f64;
                cp.z += d.z as f64;
            }
        }
    }
}

fn apply_transform(spline: &mut Spline, t: &EntityTransform) {
    crate::scene::view::transform::apply_standard_entity_transform(spline, t, |entity, p1, p2| {
        for cp in &mut entity.control_points {
            crate::scene::view::transform::reflect_xy_point(&mut cp.x, &mut cp.y, p1, p2);
        }
        for fp in &mut entity.fit_points {
            crate::scene::view::transform::reflect_xy_point(&mut fp.x, &mut fp.y, p1, p2);
        }
    });
}

impl TruckConvertible for Spline {
    fn to_truck(&self, _document: &acadrust::CadDocument) -> Option<TruckEntity> {
        Some(to_truck(self))
    }
}

impl crate::entities::traits::Grippable for Spline {
    fn grips(&self) -> Vec<GripDef> {
        grips(self)
    }
    fn apply_grip(&mut self, grip_id: usize, apply: GripApply) {
        apply_grip(self, grip_id, apply);
    }
    fn grip_menu(&self, _grip_id: usize) -> Vec<crate::scene::model::object::GripMenuItem> {
        use crate::scene::model::object::{GripMenuAction, GripMenuItem};
        vec![
            GripMenuItem {
                label: "Stretch",
                action: GripMenuAction::Stretch,
            },
            GripMenuItem {
                label: "Add Vertex",
                action: GripMenuAction::AddVertex,
            },
            GripMenuItem {
                label: "Remove Vertex",
                action: GripMenuAction::RemoveVertex,
            },
            GripMenuItem {
                label: "Refine Vertices",
                action: GripMenuAction::RefineVertices,
            },
        ]
    }
    fn apply_grip_menu(&mut self, grip_id: usize, action: crate::scene::model::object::GripMenuAction) {
        use crate::scene::model::object::GripMenuAction as A;
        let n = self.control_points.len();
        let min_cv = (self.degree as usize).saturating_add(1).max(2);
        match action {
            A::AddVertex if grip_id < n => {
                let i1 = (grip_id + 1).min(n - 1);
                if i1 == grip_id {
                    return;
                }
                let p0 = &self.control_points[grip_id];
                let p1 = &self.control_points[i1];
                let mid = acadrust::types::Vector3::new(
                    (p0.x + p1.x) * 0.5,
                    (p0.y + p1.y) * 0.5,
                    (p0.z + p1.z) * 0.5,
                );
                self.control_points.insert(i1, mid);
                if !self.weights.is_empty() && self.weights.len() == n {
                    let w = (self.weights[grip_id] + self.weights[i1.min(self.weights.len() - 1)])
                        * 0.5;
                    self.weights.insert(i1, w);
                }
                // Clear knots so to_truck rebuilds a uniform knot vector
                // for the new CV count.
                self.knots.clear();
            }
            A::RemoveVertex if grip_id < n && n > min_cv => {
                self.control_points.remove(grip_id);
                if grip_id < self.weights.len() {
                    self.weights.remove(grip_id);
                }
                self.knots.clear();
            }
            A::RefineVertices => {
                // Insert a CV between every adjacent pair (chord midpoints)
                // and rebuild a uniform knot vector.
                if n >= 2 {
                    let mut refined = Vec::with_capacity(n * 2 - 1);
                    let mut refined_w = Vec::with_capacity(n * 2 - 1);
                    let has_w = !self.weights.is_empty() && self.weights.len() == n;
                    for i in 0..n {
                        refined.push(self.control_points[i].clone());
                        if has_w {
                            refined_w.push(self.weights[i]);
                        }
                        if i + 1 < n {
                            let a = &self.control_points[i];
                            let b = &self.control_points[i + 1];
                            refined.push(acadrust::types::Vector3::new(
                                (a.x + b.x) * 0.5,
                                (a.y + b.y) * 0.5,
                                (a.z + b.z) * 0.5,
                            ));
                            if has_w {
                                refined_w.push((self.weights[i] + self.weights[i + 1]) * 0.5);
                            }
                        }
                    }
                    self.control_points = refined;
                    if has_w {
                        self.weights = refined_w;
                    }
                    self.knots.clear();
                }
            }
            _ => {}
        }
    }
}

impl crate::entities::traits::PropertyEditable for Spline {
    fn geometry_properties(&self, _text_style_names: &[String]) -> Vec<PropSection> {
        properties(self)
    }
    fn apply_geom_prop(&mut self, field: &str, value: &str) {
        apply_geom_prop(self, field, value);
    }
}

impl crate::entities::traits::Transformable for Spline {
    fn apply_transform(&mut self, t: &EntityTransform) {
        apply_transform(self, t);
    }
}
