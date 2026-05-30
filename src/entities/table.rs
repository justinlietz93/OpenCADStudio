use acadrust::entities::Table;
use glam::Vec3;

use crate::command::EntityTransform;
use crate::entities::common::{ro_prop as ro, square_grip};
use crate::entities::text_support::{layout_mtext, MTextRenderOpts, MTextVAnchor, ResolvedTextStyle};
use crate::entities::traits::{Grippable, PropertyEditable, Transformable, TruckConvertible};
use crate::scene::acad_to_truck::{TruckEntity, TruckObject};
use crate::scene::object::{GripApply, GripDef, PropSection};
use crate::scene::wire_model::SnapHint;
use crate::scene::transform;

fn v3(v: &acadrust::types::Vector3) -> Vec3 {
    Vec3::new(v.x as f32, v.y as f32, v.z as f32)
}

impl TruckConvertible for Table {
    fn to_truck(&self, document: &acadrust::CadDocument) -> Option<TruckEntity> {
        if self.rows.is_empty() || self.columns.is_empty() {
            return None;
        }

        let origin = v3(&self.insertion_point);
        let h_raw = v3(&self.horizontal_direction);
        let h = if h_raw.length_squared() > 1e-10 {
            h_raw.normalize()
        } else {
            Vec3::X
        };
        // Perpendicular "down" direction in the drawing plane (tables grow downward)
        let v_down = Vec3::new(h.y, -h.x, 0.0);

        let col_offsets: Vec<f32> = {
            let mut off = 0.0f32;
            let mut v = vec![0.0f32];
            for col in &self.columns {
                off += col.width as f32;
                v.push(off);
            }
            v
        };
        let total_w = *col_offsets.last().unwrap_or(&0.0);

        let row_offsets: Vec<f32> = {
            let mut off = 0.0f32;
            let mut v = vec![0.0f32];
            for row in &self.rows {
                off += row.height as f32;
                v.push(off);
            }
            v
        };
        let total_h = *row_offsets.last().unwrap_or(&0.0);

        let mut pts: Vec<[f32; 3]> = Vec::new();

        // Per-cell borders. When a cell carries a CellStyle, honour the
        // visibility / `invisible` flag of each of its four borders so
        // hidden borders disappear from the grid. Cells with no style still
        // emit the standard four borders. To avoid drawing each shared edge
        // twice we coalesce the segments by their (start, end) coordinates.
        use std::collections::HashSet;
        let mut emitted: HashSet<(i32, i32, i32, i32)> = HashSet::new();
        let try_add = |a: Vec3, b: Vec3, vis: bool, emitted: &mut HashSet<(i32, i32, i32, i32)>, pts: &mut Vec<[f32; 3]>| {
            if !vis {
                return;
            }
            let key = (
                (a.x * 1_000.0) as i32,
                (a.y * 1_000.0) as i32,
                (b.x * 1_000.0) as i32,
                (b.y * 1_000.0) as i32,
            );
            let key_rev = (key.2, key.3, key.0, key.1);
            if emitted.contains(&key) || emitted.contains(&key_rev) {
                return;
            }
            emitted.insert(key);
            if !pts.is_empty() {
                pts.push([f32::NAN; 3]);
            }
            pts.push([a.x, a.y, a.z]);
            pts.push([b.x, b.y, b.z]);
        };
        for (ri, row) in self.rows.iter().enumerate() {
            let row_top = row_offsets[ri];
            let row_bot = row_offsets
                .get(ri + 1)
                .copied()
                .unwrap_or(row_top + row.height as f32);
            for (ci, cell) in row.cells.iter().enumerate() {
                let col_left = col_offsets[ci];
                let col_right = col_offsets
                    .get(ci + 1)
                    .copied()
                    .unwrap_or(col_left
                        + self.columns.get(ci).map(|c| c.width as f32).unwrap_or(1.0));
                // Default to visible when no style override is present.
                let (top_vis, right_vis, bottom_vis, left_vis) = cell
                    .style
                    .as_ref()
                    .map(|s| {
                        (
                            !s.top_border.invisible,
                            !s.right_border.invisible,
                            !s.bottom_border.invisible,
                            !s.left_border.invisible,
                        )
                    })
                    .unwrap_or((true, true, true, true));
                let tl = origin + h * col_left + v_down * row_top;
                let tr = origin + h * col_right + v_down * row_top;
                let br_ = origin + h * col_right + v_down * row_bot;
                let bl = origin + h * col_left + v_down * row_bot;
                try_add(tl, tr, top_vis, &mut emitted, &mut pts);
                try_add(tr, br_, right_vis, &mut emitted, &mut pts);
                try_add(bl, br_, bottom_vis, &mut emitted, &mut pts);
                try_add(tl, bl, left_vis, &mut emitted, &mut pts);
            }
        }
        // Suppress unused-variable warnings now that the simple grid-pass
        // is gone — col/row offsets still feed cell drawing below.
        let _ = (total_w, total_h);

        // Cell text — resolve defaults via TableStyle, then layer per-cell
        // overrides on top. Resolution order (text height, text style, alignment):
        //   1. CellContent.* (per-content explicit override)
        //   2. CellStyle.*   (per-cell explicit override)
        //   3. TableStyle.<row_kind>_row_style.* (table-wide default for this row class)
        //   4. compiled-in fallback (0.18 / "txt" / MiddleCenter)
        //
        // Row classification: row 0 is Title (when not suppressed), row 1 is
        // Header (when not suppressed), everything else is Data. The two
        // suppressed flags shift the leading rows down to Data.
        let lookup_style = |h: acadrust::Handle| -> Option<&acadrust::tables::TextStyle> {
            document.text_styles.iter().find(|s| s.handle == h)
        };
        let table_style: Option<&acadrust::objects::TableStyle> =
            self.table_style_handle.and_then(|h| {
                document.objects.get(&h).and_then(|obj| match obj {
                    acadrust::objects::ObjectType::TableStyle(ts) => Some(ts),
                    _ => None,
                })
            });
        let title_suppressed = table_style.map(|t| t.title_suppressed).unwrap_or(false);
        let header_suppressed = table_style.map(|t| t.header_suppressed).unwrap_or(false);

        let font_for_handle = |handle: Option<acadrust::Handle>| -> Option<String> {
            handle.and_then(|h| lookup_style(h)).and_then(|s| {
                let file = s.font_file.trim();
                if !file.is_empty() {
                    let basename = file.rsplit(['/', '\\']).next().unwrap_or(file);
                    let stem = basename.split('.').next().unwrap_or(basename).trim();
                    if !stem.is_empty() {
                        return Some(stem.to_string());
                    }
                }
                None
            })
        };
        // Build a ResolvedTextStyle for the cell — needed by the shared MText
        // pipeline so inline `\W`, `\Q`, etc. compose with the style baseline.
        let resolved_style_for_handle = |handle: Option<acadrust::Handle>,
                                         font_name: String|
         -> ResolvedTextStyle {
            let style = handle.and_then(|h| lookup_style(h));
            ResolvedTextStyle {
                font_name,
                width_factor: style.map(|s| s.width_factor as f32).unwrap_or(1.0),
                oblique_angle: style.map(|s| s.oblique_angle as f32).unwrap_or(0.0),
                is_backward: style.map(|s| s.is_backward()).unwrap_or(false),
                is_upside_down: style.map(|s| s.is_upside_down()).unwrap_or(false),
            }
        };

        for (ri, row) in self.rows.iter().enumerate() {
            let row_top = row_offsets[ri];
            let row_bot = row_offsets
                .get(ri + 1)
                .copied()
                .unwrap_or(row_top + row.height as f32);
            let row_mid = (row_top + row_bot) * 0.5;

            // Pick the appropriate row_style from TableStyle for this row's role.
            let row_style: Option<&acadrust::objects::RowCellStyle> = table_style
                .map(|ts| {
                    let kind = match (title_suppressed, header_suppressed, ri) {
                        (false, _, 0) => 0,            // title
                        (false, false, 1) => 1,        // header
                        (true, false, 0) => 1,        // header pulled up
                        _ => 2,                       // data
                    };
                    match kind {
                        0 => &ts.title_row_style,
                        1 => &ts.header_row_style,
                        _ => &ts.data_row_style,
                    }
                });

            for (ci, cell) in row.cells.iter().enumerate() {
                let text = cell.text_value();
                if text.is_empty() {
                    continue;
                }

                let col_left = col_offsets[ci];
                let col_width = self.columns.get(ci).map(|c| c.width as f32).unwrap_or(1.0);
                let col_right = col_left + col_width;

                // Resolve text height: content → cell-style → row-style → 0.18.
                let content = cell.contents.first();
                let cell_h = content
                    .map(|c| c.text_height)
                    .filter(|h| *h > 1e-6)
                    .or_else(|| cell.style.as_ref().map(|s| s.text_height).filter(|h| *h > 1e-6))
                    .or_else(|| row_style.map(|s| s.text_height).filter(|h| *h > 1e-6))
                    .map(|h| h as f32)
                    .unwrap_or(0.18);
                let margin = cell_h * 0.5_f32;

                // Resolve text-style handle: content → cell-style → row-style.
                let style_handle = content
                    .and_then(|c| c.text_style_handle)
                    .or_else(|| cell.style.as_ref().and_then(|s| s.text_style_handle))
                    .or_else(|| row_style.and_then(|s| s.text_style_handle));
                let font_owned =
                    font_for_handle(style_handle).unwrap_or_else(|| "txt".to_string());
                let resolved = resolved_style_for_handle(style_handle, font_owned);

                // Alignment resolution: cell.style.alignment (1-9) overrides;
                // otherwise fall back to row_style.alignment, then MiddleCenter.
                let align = cell
                    .style
                    .as_ref()
                    .map(|s| s.alignment)
                    .filter(|a| *a != 0)
                    .or_else(|| row_style.map(|s| s.alignment as i32))
                    .unwrap_or(5);
                let horiz = ((align - 1).rem_euclid(3)) + 1; // 1=left, 2=center, 3=right
                let vert = ((align - 1) / 3) + 1; // 1=top, 2=middle, 3=bottom

                // Position the cell's MText block anchor at the requested
                // alignment corner / midpoint of the cell's content area.
                let (x_offset, attach_h_anchor) = match horiz {
                    1 => (col_left + margin, 0.0_f32),
                    3 => (col_right - margin, 1.0_f32),
                    _ => (col_left + col_width * 0.5, 0.5_f32),
                };
                let (y_offset, v_anchor) = match vert {
                    1 => (row_top + margin, MTextVAnchor::Top),
                    3 => (row_bot - margin, MTextVAnchor::Bottom),
                    _ => (row_mid, MTextVAnchor::Middle),
                };
                let text_origin = origin + h * x_offset + v_down * y_offset;

                // Content rotation (radians) on top of table cell rotation.
                let rot = content.map(|c| c.rotation as f32).unwrap_or(0.0)
                    + cell.rotation as f32;
                let layout = layout_mtext(&MTextRenderOpts {
                    value: text,
                    insertion: [text_origin.x as f64, text_origin.y as f64, origin.z as f64],
                    height: cell_h,
                    rect_w: 0.0,
                    rotation: rot,
                    style: &resolved,
                    attach_h_anchor,
                    v_anchor,
                    line_spacing_factor: 1.0,
                    vertical_text: false,
                    want_glyph_boxes: false,
                });
                // Flatten TextStroke groups into the table's Lines buffer.
                // Per-run inline `\C` / `\c` colour is dropped here because the
                // table emits a single TruckObject::Lines for borders + text;
                // tracking it would require splitting the table into multiple
                // WireModels per cell colour. Borders + uniform-coloured runs
                // honour the entity's outer colour.
                for ts in &layout.strokes {
                    let ox = ts.origin[0] as f32;
                    let oy = ts.origin[1] as f32;
                    for stroke in &ts.strokes {
                        if stroke.len() < 2 {
                            continue;
                        }
                        if !pts.is_empty() {
                            pts.push([f32::NAN; 3]);
                        }
                        for &[x, y] in stroke {
                            pts.push([x + ox, y + oy, origin.z]);
                        }
                    }
                }
            }
        }

        // Table currently does its layout in glam::Vec3 (f32). The world_offset
        // subtraction in tessellate.rs needs f64, so widen at the boundary —
        // precision is already limited by the f32 math above (separate fix-up).
        let pts_f64: Vec<[f64; 3]> = pts
            .into_iter()
            .map(|[x, y, z]| {
                if x.is_nan() {
                    [f64::NAN, f64::NAN, f64::NAN]
                } else {
                    [x as f64, y as f64, z as f64]
                }
            })
            .collect();
        Some(TruckEntity {
            object: TruckObject::Lines(pts_f64),
            snap_pts: vec![(origin, SnapHint::Insertion)],
            tangent_geoms: vec![],
            key_vertices: vec![],
            fill_tris: vec![],
        })
    }
}

impl Grippable for Table {
    fn grips(&self) -> Vec<GripDef> {
        vec![square_grip(0, v3(&self.insertion_point))]
    }

    fn apply_grip(&mut self, grip_id: usize, apply: GripApply) {
        if grip_id == 0 {
            match apply {
                GripApply::Translate(d) => {
                    self.insertion_point.x += d.x as f64;
                    self.insertion_point.y += d.y as f64;
                    self.insertion_point.z += d.z as f64;
                }
                GripApply::Absolute(p) => {
                    self.insertion_point.x = p.x as f64;
                    self.insertion_point.y = p.y as f64;
                    self.insertion_point.z = p.z as f64;
                }
            }
        }
    }
}

impl PropertyEditable for Table {
    fn geometry_properties(&self, _text_style_names: &[String]) -> PropSection {
        let fmt_h = |oh: &Option<acadrust::types::Handle>| -> String {
            match oh {
                Some(h) if !h.is_null() => format!("{:X}", h.value()),
                _ => "(none)".to_string(),
            }
        };
        PropSection {
            title: "Table".into(),
            props: vec![
                ro("Rows", "tbl_rows", self.rows.len().to_string()),
                ro("Columns", "tbl_cols", self.columns.len().to_string()),
                ro(
                    "Insert X",
                    "tbl_ix",
                    format!("{:.4}", self.insertion_point.x),
                ),
                ro(
                    "Insert Y",
                    "tbl_iy",
                    format!("{:.4}", self.insertion_point.y),
                ),
                ro(
                    "Insert Z",
                    "tbl_iz",
                    format!("{:.4}", self.insertion_point.z),
                ),
                ro(
                    "Table Style",
                    "tbl_style_handle",
                    fmt_h(&self.table_style_handle),
                ),
                ro(
                    "Block Record",
                    "tbl_block_rec_handle",
                    fmt_h(&self.block_record_handle),
                ),
                ro("Data Version", "tbl_data_version", self.data_version.to_string()),
                ro(
                    "Value Flags",
                    "tbl_value_flags",
                    format!("{:#010x}", self.value_flags),
                ),
                ro(
                    "Override Flag",
                    "tbl_override_flag",
                    if self.override_flag { "Yes" } else { "No" },
                ),
                ro(
                    "Override Border Color",
                    "tbl_override_border_color",
                    if self.override_border_color { "Yes" } else { "No" },
                ),
                ro(
                    "Override Border LW",
                    "tbl_override_border_lw",
                    if self.override_border_line_weight {
                        "Yes"
                    } else {
                        "No"
                    },
                ),
                ro(
                    "Override Border Vis",
                    "tbl_override_border_vis",
                    if self.override_border_visibility {
                        "Yes"
                    } else {
                        "No"
                    },
                ),
                ro(
                    "Break Spacing",
                    "tbl_break_spacing",
                    format!("{:.4}", self.break_spacing),
                ),
                ro(
                    "Break Flow",
                    "tbl_break_flow",
                    format!("{:?}", self.break_flow_direction),
                ),
                ro(
                    "Break Options",
                    "tbl_break_options",
                    format!("{:#018b}", self.break_options.bits()),
                ),
                ro(
                    "Normal",
                    "tbl_normal",
                    format!(
                        "{:.3}, {:.3}, {:.3}",
                        self.normal.x, self.normal.y, self.normal.z
                    ),
                ),
            ],
        }
    }

    fn apply_geom_prop(&mut self, _field: &str, _value: &str) {}
}

impl Transformable for Table {
    fn apply_transform(&mut self, t: &EntityTransform) {
        transform::apply_standard_entity_transform(self, t, |entity, p1, p2| {
            transform::reflect_xy_point(
                &mut entity.insertion_point.x,
                &mut entity.insertion_point.y,
                p1,
                p2,
            );
            // Reflect the horizontal direction by reflecting a tip point
            let mut tip_x = entity.insertion_point.x + entity.horizontal_direction.x;
            let mut tip_y = entity.insertion_point.y + entity.horizontal_direction.y;
            transform::reflect_xy_point(&mut tip_x, &mut tip_y, p1, p2);
            entity.horizontal_direction.x = tip_x - entity.insertion_point.x;
            entity.horizontal_direction.y = tip_y - entity.insertion_point.y;
        });
    }
}
