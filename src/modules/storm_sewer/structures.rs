// Interactive storm-sewer drafting commands: place structure markers and draw
// pipe runs on the canvas.
//
// These create geometry (circle markers, pipe lines). Attaching hydraulic data
// (invert, rim, area, C, diameter) to placed structures — so SS_ANALYZE reads
// the drawn network instead of the built-in sample — is the next step; see
// INTEGRATION.md.

use acadrust::types::Vector3;
use acadrust::{Circle, EntityType, Line};
use glam::Vec3;

use crate::command::{CadCommand, CmdResult};

fn v3(p: Vec3) -> Vector3 {
    Vector3::new(p.x as f64, p.y as f64, 0.0)
}

/// Repeatedly place a structure marker (a circle) at picked points.
pub struct PlaceStructure {
    label: &'static str,
    radius: f64,
}

impl PlaceStructure {
    pub fn inlet() -> Self {
        Self { label: "inlet", radius: 3.0 }
    }
    pub fn junction() -> Self {
        Self { label: "junction", radius: 4.0 }
    }
    pub fn outfall() -> Self {
        Self { label: "outfall", radius: 6.0 }
    }
}

impl CadCommand for PlaceStructure {
    fn name(&self) -> &'static str {
        "SS_STRUCTURE"
    }
    fn prompt(&self) -> String {
        format!("Storm sewer: pick {} location  [Esc = done]", self.label)
    }
    fn on_point(&mut self, pt: Vec3) -> CmdResult {
        let marker = Circle { center: v3(pt), radius: self.radius, ..Default::default() };
        // CommitEntity keeps the command active so several can be dropped.
        CmdResult::CommitEntity(EntityType::Circle(marker))
    }
    fn on_enter(&mut self) -> CmdResult {
        CmdResult::Cancel
    }
}

/// Draw pipe runs as connected line segments (chains like LINE).
pub struct PlacePipe {
    last: Option<Vec3>,
}

impl PlacePipe {
    pub fn new() -> Self {
        Self { last: None }
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
        if self.last.is_none() {
            "Storm sewer pipe: pick start structure".into()
        } else {
            "Storm sewer pipe: pick next structure  [Esc = done]".into()
        }
    }
    fn on_point(&mut self, pt: Vec3) -> CmdResult {
        match self.last {
            None => {
                self.last = Some(pt);
                CmdResult::NeedPoint
            }
            Some(prev) => {
                let pipe = Line::from_points(v3(prev), v3(pt));
                self.last = Some(pt);
                CmdResult::CommitEntity(EntityType::Line(pipe))
            }
        }
    }
    fn on_enter(&mut self) -> CmdResult {
        CmdResult::Cancel
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn structure_commits_a_circle_and_stays_active() {
        let mut cmd = PlaceStructure::inlet();
        match cmd.on_point(Vec3::new(10.0, 20.0, 0.0)) {
            CmdResult::CommitEntity(EntityType::Circle(c)) => {
                assert_eq!(c.center.x, 10.0);
                assert_eq!(c.center.y, 20.0);
                assert!(c.radius > 0.0);
            }
            _ => panic!("expected CommitEntity(Circle)"),
        }
    }

    #[test]
    fn pipe_needs_two_points_then_commits_a_line() {
        let mut cmd = PlacePipe::new();
        assert!(matches!(cmd.on_point(Vec3::new(0.0, 0.0, 0.0)), CmdResult::NeedPoint));
        match cmd.on_point(Vec3::new(100.0, 0.0, 0.0)) {
            CmdResult::CommitEntity(EntityType::Line(l)) => {
                assert_eq!(l.start.x, 0.0);
                assert_eq!(l.end.x, 100.0);
            }
            _ => panic!("expected CommitEntity(Line)"),
        }
    }
}
