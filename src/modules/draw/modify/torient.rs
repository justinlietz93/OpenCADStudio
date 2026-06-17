// TORIENT tool — interactive command.
//
// Command:  TORIENT
//   Requires at least one entity selected before starting.
//   Step 1: Wait for an angle (numerical input), Enter (Most Readable), or pick first point.
//   Step 2: If first point picked, pick second point to define angle vector.

use acadrust::Handle;
use glam::Vec3;

use crate::command::{CadCommand, CmdResult, DynField};
use crate::modules::{IconKind, ModuleEvent, ToolDef};
use crate::scene::model::wire_model::WireModel;
use acadrust::EntityType;

// ── Command implementation ─────────────────────────────────────────────────

enum Step {
    AngleOrFirstPoint,
    SecondPoint { first_point: Vec3 },
}

pub struct TorientCommand {
    handles: Vec<Handle>,
    entities: Vec<(Handle, EntityType)>,
    step: Step,
}

impl TorientCommand {
    pub fn new(handles: Vec<Handle>, entities: Vec<(Handle, EntityType)>) -> Self {
        Self {
            handles,
            entities,
            step: Step::AngleOrFirstPoint,
        }
    }

    fn commit_angle(&self, new_angle_rad: Option<f64>) -> CmdResult {
        let mut replacements = Vec::new();

        for (handle, entity) in &self.entities {
            let mut new_entity = entity.clone();
            let mut changed = false;

            match &mut new_entity {
                EntityType::Text(text) => {
                    let angle = new_angle_rad.unwrap_or_else(|| most_readable_angle(text.rotation));
                    text.rotation = angle;
                    changed = true;
                }
                EntityType::MText(mtext) => {
                    let angle = new_angle_rad.unwrap_or_else(|| most_readable_angle(mtext.rotation as f64));
                    mtext.rotation = angle as f64;
                    changed = true;
                }
                EntityType::AttributeDefinition(attdef) => {
                    let angle = new_angle_rad.unwrap_or_else(|| most_readable_angle(attdef.rotation));
                    attdef.rotation = angle;
                    changed = true;
                }
                EntityType::Insert(insert) => {
                    let mut block_changed = false;
                    for attr in &mut insert.attributes {
                        let angle = new_angle_rad.unwrap_or_else(|| most_readable_angle(attr.rotation));
                        attr.rotation = angle;
                        block_changed = true;
                    }
                    if block_changed {
                        changed = true;
                    }
                }
                _ => {}
            }

            if changed {
                replacements.push((*handle, vec![new_entity]));
            }
        }

        if replacements.is_empty() {
            CmdResult::Cancel
        } else {
            CmdResult::ReplaceMany(replacements, vec![])
        }
    }
}

fn most_readable_angle(mut angle_rad: f64) -> f64 {
    let two_pi = 2.0 * std::f64::consts::PI;
    angle_rad %= two_pi;
    if angle_rad < 0.0 {
        angle_rad += two_pi;
    }

    // "Most Readable" logic: if upside down (i.e. angle > 90 and <= 270 deg), rotate 180 deg
    let half_pi = std::f64::consts::PI / 2.0;
    let three_half_pi = 3.0 * std::f64::consts::PI / 2.0;

    if angle_rad > half_pi + 1e-6 && angle_rad <= three_half_pi + 1e-6 {
        angle_rad += std::f64::consts::PI;
        angle_rad %= two_pi;
    }
    
    angle_rad
}

impl CadCommand for TorientCommand {
    fn name(&self) -> &'static str {
        "TORIENT"
    }

    fn prompt(&self) -> String {
        match &self.step {
            Step::AngleOrFirstPoint => "TORIENT  New absolute rotation <Most Readable>:".into(),
            Step::SecondPoint { .. } => "TORIENT  Specify second point:".into(),
        }
    }

    fn on_point(&mut self, pt: Vec3) -> CmdResult {
        match self.step {
            Step::AngleOrFirstPoint => {
                self.step = Step::SecondPoint { first_point: pt };
                CmdResult::NeedPoint
            }
            Step::SecondPoint { first_point } => {
                let dx = pt.x - first_point.x;
                let dy = pt.y - first_point.y;
                let angle = dy.atan2(dx) as f64;
                self.commit_angle(Some(angle))
            }
        }
    }

    fn on_enter(&mut self) -> CmdResult {
        match self.step {
            Step::AngleOrFirstPoint => self.commit_angle(None),
            Step::SecondPoint { .. } => CmdResult::Cancel,
        }
    }

    fn on_escape(&mut self) -> CmdResult {
        CmdResult::Cancel
    }

    fn on_text_input(&mut self, text: &str) -> Option<CmdResult> {
        if let Step::AngleOrFirstPoint = self.step {
            let deg: f64 = text.trim().replace(',', ".").parse().ok()?;
            Some(self.commit_angle(Some(deg.to_radians())))
        } else {
            None
        }
    }

    fn on_preview_wires(&mut self, pt: Vec3) -> Vec<WireModel> {
        if let Step::SecondPoint { first_point } = self.step {
            vec![WireModel::solid(
                "rubber_band".into(),
                vec![[first_point.x, first_point.y, first_point.z], [pt.x, pt.y, pt.z]],
                WireModel::CYAN,
                false,
            )]
        } else {
            vec![]
        }
    }

    fn dyn_field(&self) -> DynField {
        match self.step {
            Step::AngleOrFirstPoint => DynField::Angle,
            Step::SecondPoint { .. } => DynField::Point,
        }
    }

    fn dyn_spec(&self) -> Option<crate::command::DynSpec> {
        None
    }
}
