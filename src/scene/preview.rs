// Auto-split from scene/mod.rs. Pure text-move; behaviour unchanged.
use super::*;

impl Scene {
    // ── Preview wire ──────────────────────────────────────────────────────

    pub fn set_preview_wires(&mut self, wires: Vec<WireModel>) {
        // Preview wires are an overlay appended to the cached base wire set in
        // `build_primitive`; they are NOT part of the tessellation cache. So a
        // preview update must NOT bump `geometry_epoch` — that would re-
        // tessellate the whole model on every rubber-band frame. The overlay
        // forces a GPU wire re-upload on its own (the `has_overlay` content-id
        // path), and iced redraws after the message that set the preview.
        self.preview_wires = wires;
    }

    pub fn clear_preview_wire(&mut self) {
        // No geometry bump — see `set_preview_wires`. Dropping the overlay
        // flips the wire content id back to the base tessellation id, which
        // re-uploads the base wires (without the preview) on the next frame.
        self.preview_wires = vec![];
        self.interim_wire = None;
    }

    pub fn wire_models_for(&self, handles: &[acadrust::Handle]) -> Vec<WireModel> {
        handles
            .iter()
            .flat_map(|h| {
                match self.document.entities().find(|e| e.common().handle == *h) {
                    // Hatches carry no outline in the normal wire set, but an
                    // edit preview (move / copy / array / grip-drag) needs to
                    // show the shape following the cursor. Build a live boundary
                    // from the current HatchModel — `apply_grip` keeps it in
                    // step, so the preview tracks a dragged grip in real time.
                    Some(EntityType::Hatch(_)) => {
                        self.hatch_outline_wire(*h).into_iter().collect()
                    }
                    Some(e) => self.tessellate_one(e),
                    None => Vec::new(),
                }
            })
            .collect()
    }

    /// Boundary outline wire for a hatch, reconstructed from its cached
    /// `HatchModel` (offsets from `world_origin`). Used only for edit previews —
    /// the normal render shows the fill, not this outline.
    fn hatch_outline_wire(&self, handle: Handle) -> Option<WireModel> {
        let m = self.hatches.get(&handle)?;
        let (wx, wy) = (m.world_origin[0], m.world_origin[1]);
        let pts: Vec<[f64; 3]> = m
            .boundary
            .iter()
            .map(|&[x, y]| {
                if x.is_finite() && y.is_finite() {
                    [wx + x as f64, wy + y as f64, 0.0]
                } else {
                    [f64::NAN; 3]
                }
            })
            .collect();
        if pts.len() < 2 {
            return None;
        }
        Some(WireModel::solid_f64(
            handle.value().to_string(),
            pts,
            m.color,
            false,
        ))
    }

    /// Build wire models for an arbitrary slice of entities (e.g. clipboard contents).
    /// Entities need not be in the document — they are tessellated directly.
    pub fn wires_for_entities(&self, entities: &[acadrust::EntityType]) -> Vec<WireModel> {
        entities
            .iter()
            .flat_map(|e| self.tessellate_one(e))
            .collect()
    }

    pub fn set_interim_wire(&mut self, w: WireModel) {
        // Overlay wire — same reasoning as `set_preview_wires`: no geometry
        // bump, so the model isn't re-tessellated on every interim update.
        self.interim_wire = Some(w);
    }
}
