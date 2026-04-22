use super::render::{CameraState, Primitive};
use super::Scene;
use acadrust::Handle;
use iced::widget::shader;
use iced::{mouse, Event, Rectangle};

// ── Mode ──────────────────────────────────────────────────────────────────

pub enum ViewportPaneMode {
    /// Full model space — fills whatever bounds Iced assigns.
    Model,
    /// Paper-space entities only (title blocks, frames, borders) using the
    /// paper-space camera. No viewport content projection.
    PaperSheet,
    /// Model-space content seen through a specific paper-space Viewport entity.
    Paper { handle: Handle },
}

// ── Widget struct ─────────────────────────────────────────────────────────

pub struct ViewportPane<'a> {
    pub scene: &'a Scene,
    pub mode: ViewportPaneMode,
}

impl<'a> ViewportPane<'a> {
    pub fn model(scene: &'a Scene) -> Self {
        Self { scene, mode: ViewportPaneMode::Model }
    }

    /// Paper-sheet layer: paper-space entities rendered with the paper camera.
    pub fn paper_sheet(scene: &'a Scene) -> Self {
        Self { scene, mode: ViewportPaneMode::PaperSheet }
    }

    /// One paper-space viewport: model content rendered through its own camera.
    pub fn paper(scene: &'a Scene, handle: Handle) -> Self {
        Self { scene, mode: ViewportPaneMode::Paper { handle } }
    }
}

// ── shader::Program impl ──────────────────────────────────────────────────

impl<'a, Msg: std::fmt::Debug + Clone> shader::Program<Msg> for ViewportPane<'a> {
    type State = CameraState;
    type Primitive = Primitive;

    fn draw(
        &self,
        state: &Self::State,
        _cursor: mouse::Cursor,
        bounds: Rectangle,
    ) -> Self::Primitive {
        match &self.mode {
            ViewportPaneMode::Model => {
                self.scene.build_primitive(state.hover_region, bounds)
            }
            ViewportPaneMode::PaperSheet => {
                self.scene.build_paper_sheet_primitive(state.hover_region, bounds)
            }
            ViewportPaneMode::Paper { handle } => {
                self.scene.build_viewport_primitive(*handle, state.hover_region, bounds)
            }
        }
    }

    fn update(
        &self,
        state: &mut Self::State,
        event: &Event,
        bounds: Rectangle,
        cursor: mouse::Cursor,
    ) -> Option<iced::widget::Action<Msg>> {
        // ViewCube hover only makes sense in the full model-space view.
        if matches!(self.mode, ViewportPaneMode::Model | ViewportPaneMode::PaperSheet) {
            self.scene.update_viewcube_state(state, bounds, cursor);
        }
        let _ = event;
        None
    }

    fn mouse_interaction(
        &self,
        state: &Self::State,
        _b: Rectangle,
        _c: mouse::Cursor,
    ) -> mouse::Interaction {
        if matches!(self.mode, ViewportPaneMode::Model | ViewportPaneMode::PaperSheet) {
            self.scene.viewcube_mouse_interaction(state)
        } else {
            mouse::Interaction::default()
        }
    }
}
