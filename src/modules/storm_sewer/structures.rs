// Interactive storm-sewer drafting commands.
//
// PlaceStructure: pick a location, then prompt invert/rim/area/C on the command
// line; commits a circle marker carrying StormSewer XDATA.
// PlacePipe: pick a start structure and an end structure (snaps to their
// centers), prompt diameter/n; commits a line carrying XDATA with connectivity.

use acadrust::types::Vector3;
use acadrust::{Circle, EntityType, Handle, Line};
use glam::Vec3;

use stormsewer::network::NodeKind;

use super::data;
use crate::command::{CadCommand, CmdResult};

fn parse_num(text: &str) -> Option<f64> {
    text.trim().replace(',', ".").parse::<f64>().ok()
}

// ── Structure placement ─────────────────────────────────────────────────────

enum SStep {
    Point,
    Invert,
    Rim,
    Area,
    C,
}

pub struct PlaceStructure {
    kind: NodeKind,
    radius: f64,
    x: f64,
    y: f64,
    invert: f64,
    rim: f64,
    area: f64,
    c: f64,
    step: SStep,
}

impl PlaceStructure {
    pub fn inlet() -> Self {
        Self::new(NodeKind::Inlet, 3.0)
    }
    pub fn junction() -> Self {
        Self::new(NodeKind::Junction, 4.0)
    }
    pub fn outfall() -> Self {
        Self::new(NodeKind::Outfall, 6.0)
    }
    fn new(kind: NodeKind, radius: f64) -> Self {
        Self { kind, radius, x: 0.0, y: 0.0, invert: 100.0, rim: 105.0, area: 1.0, c: 0.70, step: SStep::Point }
    }
    fn commit(&self) -> CmdResult {
        let circ = Circle { center: Vector3::new(self.x, self.y, 0.0), radius: self.radius, ..Default::default() };
        let mut ent = EntityType::Circle(circ);
        let (area, c) = if self.kind == NodeKind::Outfall { (0.0, 0.0) } else { (self.area, self.c) };
        ent.common_mut()
            .extended_data
            .add_record(data::structure_xdata(self.kind, self.invert, self.rim, area, c));
        CmdResult::CommitAndExit(ent)
    }
}

impl CadCommand for PlaceStructure {
    fn name(&self) -> &'static str {
        "SS_STRUCTURE"
    }
    fn prompt(&self) -> String {
        match self.step {
            SStep::Point => format!("Storm {}: pick location:", data::kind_str(self.kind)),
            SStep::Invert => format!("Invert elevation <{:.2}>:", self.invert),
            SStep::Rim => format!("Rim elevation <{:.2}>:", self.rim),
            SStep::Area => format!("Drainage area, ac <{:.2}>:", self.area),
            SStep::C => format!("Runoff coefficient C <{:.2}>:", self.c),
        }
    }
    fn on_point(&mut self, pt: Vec3) -> CmdResult {
        if let SStep::Point = self.step {
            self.x = pt.x as f64;
            self.y = pt.y as f64;
            self.step = SStep::Invert;
        }
        CmdResult::NeedPoint
    }
    fn wants_text_input(&self) -> bool {
        !matches!(self.step, SStep::Point)
    }
    fn on_text_input(&mut self, text: &str) -> Option<CmdResult> {
        let v = parse_num(text);
        match self.step {
            SStep::Point => None,
            SStep::Invert => {
                if let Some(x) = v {
                    self.invert = x;
                }
                self.step = SStep::Rim;
                None
            }
            SStep::Rim => {
                if let Some(x) = v {
                    self.rim = x;
                }
                if self.kind == NodeKind::Outfall {
                    Some(self.commit())
                } else {
                    self.step = SStep::Area;
                    None
                }
            }
            SStep::Area => {
                if let Some(x) = v {
                    self.area = x;
                }
                self.step = SStep::C;
                None
            }
            SStep::C => {
                if let Some(x) = v {
                    self.c = x;
                }
                Some(self.commit())
            }
        }
    }
    fn on_enter(&mut self) -> CmdResult {
        CmdResult::Cancel
    }
}

// ── Pipe placement ──────────────────────────────────────────────────────────

enum PStep {
    PickStart,
    PickEnd,
    Diameter,
    N,
}

pub struct PlacePipe {
    step: PStep,
    start_handle: Option<Handle>,
    start_xy: (f64, f64),
    end_handle: Option<Handle>,
    end_xy: (f64, f64),
    pending: Option<Handle>,
    diameter: f64,
    n: f64,
}

impl PlacePipe {
    pub fn new() -> Self {
        Self {
            step: PStep::PickStart,
            start_handle: None,
            start_xy: (0.0, 0.0),
            end_handle: None,
            end_xy: (0.0, 0.0),
            pending: None,
            diameter: 1.25,
            n: 0.013,
        }
    }
    fn commit(&self) -> CmdResult {
        let line = Line::from_points(
            Vector3::new(self.start_xy.0, self.start_xy.1, 0.0),
            Vector3::new(self.end_xy.0, self.end_xy.1, 0.0),
        );
        let mut ent = EntityType::Line(line);
        if let (Some(f), Some(t)) = (self.start_handle, self.end_handle) {
            ent.common_mut().extended_data.add_record(data::pipe_xdata(self.diameter, self.n, f, t));
        }
        CmdResult::CommitAndExit(ent)
    }
}

impl Default for PlacePipe {
    fn default() -> Self {
        Self::new()
    }
}

impl CadCommand for PlacePipe {
    fn name(&self) -> &'static str {
        "SS_PIPE"
    }
    fn prompt(&self) -> String {
        match self.step {
            PStep::PickStart => "Pipe: pick START structure:".into(),
            PStep::PickEnd => "Pipe: pick END structure:".into(),
            PStep::Diameter => format!("Pipe diameter, ft <{:.2}>:", self.diameter),
            PStep::N => format!("Manning n <{:.3}>:", self.n),
        }
    }
    fn needs_entity_pick(&self) -> bool {
        matches!(self.step, PStep::PickStart | PStep::PickEnd)
    }
    fn on_entity_pick(&mut self, handle: Handle, _pt: Vec3) -> CmdResult {
        // Stash the handle; inject_picked_entity validates it and reads center.
        self.pending = Some(handle);
        CmdResult::NeedPoint
    }
    fn inject_picked_entity(&mut self, entity: EntityType) {
        let is_struct = entity.common().extended_data.get_record(data::APP_STRUCT).is_some();
        let center = match &entity {
            EntityType::Circle(c) => Some((c.center.x, c.center.y)),
            _ => None,
        };
        if !is_struct || center.is_none() {
            self.pending = None;
            return;
        }
        let center = center.unwrap();
        match self.step {
            PStep::PickStart => {
                self.start_handle = self.pending.take();
                self.start_xy = center;
                self.step = PStep::PickEnd;
            }
            PStep::PickEnd => {
                self.end_handle = self.pending.take();
                self.end_xy = center;
                self.step = PStep::Diameter;
            }
            _ => {}
        }
    }
    fn wants_text_input(&self) -> bool {
        matches!(self.step, PStep::Diameter | PStep::N)
    }
    fn on_text_input(&mut self, text: &str) -> Option<CmdResult> {
        let v = parse_num(text);
        match self.step {
            PStep::Diameter => {
                if let Some(x) = v {
                    self.diameter = x;
                }
                self.step = PStep::N;
                None
            }
            PStep::N => {
                if let Some(x) = v {
                    self.n = x;
                }
                Some(self.commit())
            }
            _ => None,
        }
    }
    fn on_point(&mut self, _pt: Vec3) -> CmdResult {
        CmdResult::NeedPoint
    }
    fn on_enter(&mut self) -> CmdResult {
        CmdResult::Cancel
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn structure_prompts_then_commits_tagged_circle() {
        let mut cmd = PlaceStructure::inlet();
        // pick a point
        assert!(matches!(cmd.on_point(Vec3::new(10.0, 20.0, 0.0)), CmdResult::NeedPoint));
        assert!(cmd.wants_text_input());
        // invert, rim, area, then C completes
        assert!(cmd.on_text_input("104").is_none()); // invert -> rim
        assert!(cmd.on_text_input("110").is_none()); // rim -> area
        assert!(cmd.on_text_input("2.0").is_none()); // area -> C
        match cmd.on_text_input("0.8") {
            Some(CmdResult::CommitAndExit(EntityType::Circle(c))) => {
                assert_eq!(c.center.x, 10.0);
                let rec = EntityType::Circle(c.clone());
                assert!(rec.common().extended_data.get_record(data::APP_STRUCT).is_some());
            }
            _ => panic!("expected CommitAndExit(Circle) with XDATA"),
        }
    }

    #[test]
    fn outfall_skips_area_and_c() {
        let mut cmd = PlaceStructure::outfall();
        cmd.on_point(Vec3::new(0.0, 0.0, 0.0));
        assert!(cmd.on_text_input("100").is_none()); // invert -> rim
        // rim completes immediately for an outfall
        assert!(matches!(cmd.on_text_input("105"), Some(CmdResult::CommitAndExit(_))));
    }

    #[test]
    fn pipe_connects_two_structures() {
        let mut cmd = PlacePipe::new();
        assert!(cmd.needs_entity_pick());
        // simulate picking the start structure
        let mut s1 = EntityType::Circle(Circle { center: Vector3::new(0.0, 0.0, 0.0), radius: 3.0, ..Default::default() });
        s1.common_mut().extended_data.add_record(data::structure_xdata(NodeKind::Inlet, 100.0, 105.0, 1.0, 0.7));
        cmd.on_entity_pick(Handle::default(), Vec3::ZERO);
        cmd.inject_picked_entity(s1);
        // now picking the end structure
        let mut s2 = EntityType::Circle(Circle { center: Vector3::new(100.0, 0.0, 0.0), radius: 3.0, ..Default::default() });
        s2.common_mut().extended_data.add_record(data::structure_xdata(NodeKind::Outfall, 99.0, 104.0, 0.0, 0.0));
        cmd.on_entity_pick(Handle::default(), Vec3::ZERO);
        cmd.inject_picked_entity(s2);
        assert!(cmd.wants_text_input(), "should be prompting diameter");
        assert!(cmd.on_text_input("1.5").is_none()); // diameter -> n
        match cmd.on_text_input("0.013") {
            Some(CmdResult::CommitAndExit(EntityType::Line(l))) => {
                assert_eq!(l.start.x, 0.0);
                assert_eq!(l.end.x, 100.0);
            }
            _ => panic!("expected CommitAndExit(Line)"),
        }
    }
}
