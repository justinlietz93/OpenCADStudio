// Auto-split from scene/mod.rs. Pure text-move; behaviour unchanged.
use super::*;

impl Scene {
    // ── Layout management ─────────────────────────────────────────────────

    /// Rename a paper-space layout.  Updates the Layout object name in the document.
    pub fn rename_layout(&mut self, old_name: &str, new_name: &str) {
        for obj in self.document.objects.values_mut() {
            if let ObjectType::Layout(l) = obj {
                if l.name == old_name {
                    l.name = new_name.to_string();
                    return;
                }
            }
        }
    }

    /// Delete a paper-space layout and all entities owned by it.
    /// Returns `false` if the layout was not found or is "Model".
    pub fn delete_layout(&mut self, name: &str) -> bool {
        if name == "Model" {
            return false;
        }

        let layout_info = self.document.objects.values().find_map(|obj| {
            if let ObjectType::Layout(l) = obj {
                if l.name == name {
                    return Some((l.handle, l.block_record));
                }
            }
            None
        });

        let (layout_handle, block_handle) = match layout_info {
            Some(info) => info,
            None => return false,
        };

        // Remove all entities that belong to this layout's block record.
        let to_remove: Vec<Handle> = self
            .document
            .entities()
            .filter(|e| e.common().owner_handle == block_handle)
            .map(|e| e.common().handle)
            .collect();
        for h in &to_remove {
            self.hatches.remove(h);
            self.meshes.remove(h);
            self.solid_models.remove(h);
            self.document.remove_entity(*h);
        }

        // Remove the Layout object itself.
        self.document.objects.remove(&layout_handle);

        // Drop the layout's entry from the ACAD_LAYOUT dictionary so it does not
        // dangle (and so AutoCAD doesn't try to recover a now-missing layout).
        let dict_handle = self.document.header.acad_layout_dict_handle;
        if let Some(ObjectType::Dictionary(d)) = self.document.objects.get_mut(&dict_handle) {
            d.entries.retain(|(k, _)| k != name);
        }

        // Remove the now-empty paper-space block record.
        let block_name = self
            .document
            .block_records
            .iter()
            .find(|b| b.handle == block_handle)
            .map(|b| b.name.clone());
        if let Some(bn) = block_name {
            self.document.block_records.remove(&bn);
        }

        // Drop any standalone PlotSettings page setup tied to this layout.
        let ps_handles: Vec<Handle> = self
            .document
            .objects
            .iter()
            .filter_map(|(h, o)| match o {
                ObjectType::PlotSettings(ps) if ps.page_name == name => Some(*h),
                _ => None,
            })
            .collect();
        for h in ps_handles {
            self.document.objects.remove(&h);
        }

        // If the deleted layout was active, fall back to Model space.
        if self.current_layout == name {
            self.current_layout = "Model".to_string();
        }

        self.bump_geometry();
        true
    }

    /// Swap the `tab_order` of two paper layouts so they appear in swapped order.
    pub fn swap_layout_order(&mut self, name_a: &str, name_b: &str) {
        let mut order_a: Option<i16> = None;
        let mut order_b: Option<i16> = None;
        for obj in self.document.objects.values() {
            if let ObjectType::Layout(l) = obj {
                if l.name == name_a {
                    order_a = Some(l.tab_order);
                }
                if l.name == name_b {
                    order_b = Some(l.tab_order);
                }
            }
        }
        if let (Some(oa), Some(ob)) = (order_a, order_b) {
            for obj in self.document.objects.values_mut() {
                if let ObjectType::Layout(l) = obj {
                    if l.name == name_a {
                        l.tab_order = ob;
                    } else if l.name == name_b {
                        l.tab_order = oa;
                    }
                }
            }
        }
    }
    /// Discover the inner divider edges between Model tiles. Each entry
    /// is one draggable horizontal or vertical edge, with the span along
    /// the perpendicular axis that the edge actually covers (the union
    /// of touching tiles' extents). Coordinates are in normalized 0..1
    /// canvas space. Returns an empty list outside Model or for a
    /// single-tile layout.
    pub fn model_tile_edges(&self) -> Vec<TileEdge> {
        if self.current_layout != "Model" {
            return vec![];
        }
        let tiles = self.model_tiles.borrow();
        if tiles.len() < 2 {
            return vec![];
        }
        let mut out = Vec::new();
        // Collect candidate inner x's: any tile edge that's strictly
        // inside (0, 1). Dedup by epsilon.
        let mut xs: Vec<f32> = Vec::new();
        let mut ys: Vec<f32> = Vec::new();
        for t in tiles.iter() {
            for x in [t.rect.x, t.rect.x + t.rect.width] {
                if x > TILE_EPS && x < 1.0 - TILE_EPS {
                    xs.push(x);
                }
            }
            for y in [t.rect.y, t.rect.y + t.rect.height] {
                if y > TILE_EPS && y < 1.0 - TILE_EPS {
                    ys.push(y);
                }
            }
        }
        xs.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
        xs.dedup_by(|a, b| (*a - *b).abs() < TILE_EPS);
        ys.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
        ys.dedup_by(|a, b| (*a - *b).abs() < TILE_EPS);
        for x in xs {
            let mut y0 = f32::INFINITY;
            let mut y1 = f32::NEG_INFINITY;
            let mut has_left = false;
            let mut has_right = false;
            for t in tiles.iter() {
                if ((t.rect.x + t.rect.width) - x).abs() < TILE_EPS {
                    has_left = true;
                    y0 = y0.min(t.rect.y);
                    y1 = y1.max(t.rect.y + t.rect.height);
                }
                if (t.rect.x - x).abs() < TILE_EPS {
                    has_right = true;
                    y0 = y0.min(t.rect.y);
                    y1 = y1.max(t.rect.y + t.rect.height);
                }
            }
            if has_left && has_right && y1 > y0 {
                out.push(TileEdge {
                    orient: TileEdgeOrient::Vertical,
                    coord: x,
                    span: (y0, y1),
                });
            }
        }
        for y in ys {
            let mut x0 = f32::INFINITY;
            let mut x1 = f32::NEG_INFINITY;
            let mut has_top = false;
            let mut has_bot = false;
            for t in tiles.iter() {
                if ((t.rect.y + t.rect.height) - y).abs() < TILE_EPS {
                    has_top = true;
                    x0 = x0.min(t.rect.x);
                    x1 = x1.max(t.rect.x + t.rect.width);
                }
                if (t.rect.y - y).abs() < TILE_EPS {
                    has_bot = true;
                    x0 = x0.min(t.rect.x);
                    x1 = x1.max(t.rect.x + t.rect.width);
                }
            }
            if has_top && has_bot && x1 > x0 {
                out.push(TileEdge {
                    orient: TileEdgeOrient::Horizontal,
                    coord: y,
                    span: (x0, x1),
                });
            }
        }
        out
    }

    /// Hit-test the inner Model-tile dividers against a pixel cursor.
    /// `bounds` is the canvas pixel rectangle (origin = canvas top-left).
    /// Returns the closest edge within `tolerance_px` pixels of the cursor
    /// along its perpendicular axis, also requiring the cursor to lie
    /// within the edge's actual span.
    pub fn hit_model_tile_edge(
        &self,
        cursor_px: iced::Point,
        bounds: iced::Rectangle,
        tolerance_px: f32,
    ) -> Option<TileEdge> {
        if bounds.width <= 0.0 || bounds.height <= 0.0 {
            return None;
        }
        let cx = cursor_px.x - bounds.x;
        let cy = cursor_px.y - bounds.y;
        let nx = cx / bounds.width;
        let ny = cy / bounds.height;
        let tol_nx = tolerance_px / bounds.width;
        let tol_ny = tolerance_px / bounds.height;
        let mut best: Option<(f32, TileEdge)> = None;
        for e in self.model_tile_edges() {
            let (dist, in_span) = match e.orient {
                TileEdgeOrient::Vertical => (
                    (e.coord - nx).abs() / tol_nx.max(1e-9),
                    ny >= e.span.0 && ny <= e.span.1,
                ),
                TileEdgeOrient::Horizontal => (
                    (e.coord - ny).abs() / tol_ny.max(1e-9),
                    nx >= e.span.0 && nx <= e.span.1,
                ),
            };
            if in_span && dist <= 1.0 {
                if best.as_ref().map_or(true, |(d, _)| dist < *d) {
                    best = Some((dist, e));
                }
            }
        }
        best.map(|(_, e)| e)
    }

    /// Move the inner divider edge from `old_coord` to `new_coord`, both
    /// in normalized 0..1 space. Adjusts every tile that touches the
    /// edge on either side. `min_size` clamps the new coordinate so no
    /// tile on either side can shrink below the minimum — dragging a
    /// divider to the screen edge stops at that minimum instead of
    /// closing the pane (use the close button for that).
    pub fn move_model_tile_edge(
        &self,
        orient: TileEdgeOrient,
        old_coord: f32,
        new_coord: f32,
        min_size: f32,
    ) {
        let mut tiles = self.model_tiles.borrow_mut();
        // Clamp the new coordinate so no tile becomes ≤ 0 wide / tall.
        // (Sub-`min_size` results are still allowed — the collapse pass
        // handles those.)
        let new_coord = match orient {
            TileEdgeOrient::Vertical => {
                let mut lo = 0.0_f32;
                let mut hi = 1.0_f32;
                for t in tiles.iter() {
                    if ((t.rect.x + t.rect.width) - old_coord).abs() < TILE_EPS {
                        lo = lo.max(t.rect.x + min_size);
                    }
                    if (t.rect.x - old_coord).abs() < TILE_EPS {
                        hi = hi.min(t.rect.x + t.rect.width - min_size);
                    }
                }
                new_coord.clamp(lo, hi.max(lo))
            }
            TileEdgeOrient::Horizontal => {
                let mut lo = 0.0_f32;
                let mut hi = 1.0_f32;
                for t in tiles.iter() {
                    if ((t.rect.y + t.rect.height) - old_coord).abs() < TILE_EPS {
                        lo = lo.max(t.rect.y + min_size);
                    }
                    if (t.rect.y - old_coord).abs() < TILE_EPS {
                        hi = hi.min(t.rect.y + t.rect.height - min_size);
                    }
                }
                new_coord.clamp(lo, hi.max(lo))
            }
        };
        for t in tiles.iter_mut() {
            match orient {
                TileEdgeOrient::Vertical => {
                    if ((t.rect.x + t.rect.width) - old_coord).abs() < TILE_EPS {
                        t.rect.width = (new_coord - t.rect.x).max(0.0);
                    } else if (t.rect.x - old_coord).abs() < TILE_EPS {
                        let old_right = t.rect.x + t.rect.width;
                        t.rect.x = new_coord;
                        t.rect.width = (old_right - new_coord).max(0.0);
                    }
                }
                TileEdgeOrient::Horizontal => {
                    if ((t.rect.y + t.rect.height) - old_coord).abs() < TILE_EPS {
                        t.rect.height = (new_coord - t.rect.y).max(0.0);
                    } else if (t.rect.y - old_coord).abs() < TILE_EPS {
                        let old_bottom = t.rect.y + t.rect.height;
                        t.rect.y = new_coord;
                        t.rect.height = (old_bottom - new_coord).max(0.0);
                    }
                }
            }
        }
    }

    /// Close the active Model tile, absorbing its area into the
    /// neighbour that shares the longest contact edge and rebinding the
    /// live camera to that neighbour. No-op with fewer than two tiles.
    pub fn close_active_model_tile(&self) {
        let mut tiles = self.model_tiles.borrow_mut();
        if tiles.len() < 2 {
            return;
        }
        let idx = self.active_model_tile.get().min(tiles.len() - 1);
        self.absorb_model_tile(&mut tiles, idx);
    }

    /// Drop tile `idx`, growing the neighbour with the longest shared
    /// contact edge to cover the vacated area. Fixes up
    /// `active_model_tile` so the live camera stays bound to a real tile
    /// (preferring the neighbour that absorbed it). Falls back to
    /// stretching the first remaining tile to fill the canvas if the
    /// tile has no axis-aligned neighbour.
    fn absorb_model_tile(&self, tiles: &mut Vec<ModelTile>, idx: usize) {
        let removed = tiles[idx].rect;
        // Find the neighbour with the longest shared contact edge.
        let mut best: Option<(usize, f32, ContactSide)> = None;
        for (j, t) in tiles.iter().enumerate() {
            if j == idx {
                continue;
            }
            let probes = [
                (
                    ContactSide::Left,
                    ((t.rect.x + t.rect.width) - removed.x).abs() < TILE_EPS,
                    overlap_len(
                        (t.rect.y, t.rect.y + t.rect.height),
                        (removed.y, removed.y + removed.height),
                    ),
                ),
                (
                    ContactSide::Right,
                    (t.rect.x - (removed.x + removed.width)).abs() < TILE_EPS,
                    overlap_len(
                        (t.rect.y, t.rect.y + t.rect.height),
                        (removed.y, removed.y + removed.height),
                    ),
                ),
                (
                    ContactSide::Top,
                    ((t.rect.y + t.rect.height) - removed.y).abs() < TILE_EPS,
                    overlap_len(
                        (t.rect.x, t.rect.x + t.rect.width),
                        (removed.x, removed.x + removed.width),
                    ),
                ),
                (
                    ContactSide::Bottom,
                    (t.rect.y - (removed.y + removed.height)).abs() < TILE_EPS,
                    overlap_len(
                        (t.rect.x, t.rect.x + t.rect.width),
                        (removed.x, removed.x + removed.width),
                    ),
                ),
            ];
            for (side, touches, c) in probes {
                if touches && c > 0.0 {
                    if best.map_or(true, |(_, len, _)| c > len) {
                        best = Some((j, c, side));
                    }
                }
            }
        }
        if let Some((nbr_idx, _, side)) = best {
            match side {
                ContactSide::Left => {
                    tiles[nbr_idx].rect.width =
                        (removed.x + removed.width) - tiles[nbr_idx].rect.x;
                }
                ContactSide::Right => {
                    let old_right =
                        tiles[nbr_idx].rect.x + tiles[nbr_idx].rect.width;
                    tiles[nbr_idx].rect.x = removed.x;
                    tiles[nbr_idx].rect.width = old_right - removed.x;
                }
                ContactSide::Top => {
                    tiles[nbr_idx].rect.height =
                        (removed.y + removed.height) - tiles[nbr_idx].rect.y;
                }
                ContactSide::Bottom => {
                    let old_bottom =
                        tiles[nbr_idx].rect.y + tiles[nbr_idx].rect.height;
                    tiles[nbr_idx].rect.y = removed.y;
                    tiles[nbr_idx].rect.height = old_bottom - removed.y;
                }
            }
            let active = self.active_model_tile.get();
            let new_active = if active == idx {
                if nbr_idx > idx { nbr_idx - 1 } else { nbr_idx }
            } else if active > idx {
                active - 1
            } else {
                active
            };
            tiles.remove(idx);
            self.active_model_tile
                .set(new_active.min(tiles.len().saturating_sub(1)));
        } else {
            // Isolated tile (shouldn't happen with axis-aligned
            // splits) — drop it and stretch the first remaining
            // tile to fill the canvas so we don't leave a hole.
            tiles.remove(idx);
            let active = self.active_model_tile.get();
            self.active_model_tile
                .set(active.saturating_sub(if active > idx { 1 } else { 0 }).min(tiles.len().saturating_sub(1)));
            if let Some(first) = tiles.first_mut() {
                first.rect = iced::Rectangle {
                    x: 0.0,
                    y: 0.0,
                    width: 1.0,
                    height: 1.0,
                };
            }
        }
    }

    /// Split the active Model tile in two. `horizontal` → a horizontal
    /// divider (top / bottom halves); otherwise a vertical divider (left /
    /// right). Both halves inherit the active tile's current camera; the
    /// active tile stays the first half. No-op outside the Model layout.
    pub fn split_active_model_tile(&self, horizontal: bool) {
        if self.current_layout != "Model" {
            return;
        }
        let cam_now = self.camera.borrow().clone();
        let mut tiles = self.model_tiles.borrow_mut();
        let active = self.active_model_tile.get().min(tiles.len().saturating_sub(1));
        let r = tiles[active].rect;
        let (a, b) = if horizontal {
            (
                iced::Rectangle { height: r.height / 2.0, ..r },
                iced::Rectangle {
                    y: r.y + r.height / 2.0,
                    height: r.height / 2.0,
                    ..r
                },
            )
        } else {
            (
                iced::Rectangle { width: r.width / 2.0, ..r },
                iced::Rectangle {
                    x: r.x + r.width / 2.0,
                    width: r.width / 2.0,
                    ..r
                },
            )
        };
        let mode = tiles[active].render_mode;
        let (grid_on, snap_on) = (tiles[active].grid_on, tiles[active].snap_on);
        tiles[active] = ModelTile {
            rect: a,
            camera: cam_now.clone(),
            render_mode: mode,
            grid_on,
            snap_on,
        };
        tiles.insert(
            active + 1,
            ModelTile {
                rect: b,
                camera: cam_now,
                render_mode: mode,
                grid_on,
                snap_on,
            },
        );
    }

    /// Make the Model tile containing normalized point `(nx, ny)` active,
    /// swapping cameras so the live `Scene::camera` follows the new tile.
    /// Returns `true` when the active tile changed. No-op outside Model.
    pub fn set_active_model_tile_at(&self, nx: f32, ny: f32) -> bool {
        if self.current_layout != "Model" {
            return false;
        }
        let new = {
            let tiles = self.model_tiles.borrow();
            tiles.iter().position(|t| {
                nx >= t.rect.x
                    && nx < t.rect.x + t.rect.width
                    && ny >= t.rect.y
                    && ny < t.rect.y + t.rect.height
            })
        };
        let Some(new) = new else { return false };
        let old = self.active_model_tile.get();
        if new == old {
            return false;
        }
        // Stash the live camera into the outgoing tile, load the incoming.
        let incoming = {
            let mut tiles = self.model_tiles.borrow_mut();
            if let Some(t) = tiles.get_mut(old) {
                t.camera = self.camera.borrow().clone();
            }
            tiles.get(new).map(|t| t.camera.clone())
        };
        if let Some(cam) = incoming {
            *self.camera.borrow_mut() = cam;
        }
        self.active_model_tile.set(new);
        // Caller bumps camera_generation (it needs &mut Scene).
        true
    }

    /// Replace the Model tiled layout with the given normalized rectangles
    /// (each in 0..1). Every tile inherits the current camera; the first
    /// tile becomes active. Used by VPORTS presets and `reset_model_tiles`.
    pub fn set_model_tile_layout(&self, rects: Vec<iced::Rectangle>) {
        let cam_now = self.camera.borrow().clone();
        // Every new pane inherits the active tile's current visual style.
        let (mode, grid_on, snap_on) = {
            let tiles = self.model_tiles.borrow();
            let active = self.active_model_tile.get().min(tiles.len().saturating_sub(1));
            tiles
                .get(active)
                .map(|t| (t.render_mode, t.grid_on, t.snap_on))
                .unwrap_or((
                    acadrust::entities::ViewportRenderMode::Wireframe2D,
                    false,
                    false,
                ))
        };
        let tiles: Vec<ModelTile> = rects
            .into_iter()
            .map(|rect| ModelTile {
                rect,
                camera: cam_now.clone(),
                render_mode: mode,
                grid_on,
                snap_on,
            })
            .collect();
        *self.model_tiles.borrow_mut() = if tiles.is_empty() {
            vec![ModelTile {
                rect: iced::Rectangle {
                    x: 0.0,
                    y: 0.0,
                    width: 1.0,
                    height: 1.0,
                },
                camera: cam_now,
                render_mode: mode,
                grid_on,
                snap_on,
            }]
        } else {
            tiles
        };
        self.active_model_tile.set(0);
    }

    /// Screen-pixel rectangle of the active Model tile within a canvas of
    /// `(vw, vh)`. Full canvas outside the Model layout or for a single
    /// tile. Used to map cursor coordinates into the active tile so pick /
    /// pan / ViewCube work per-pane in a tiled layout.
    /// Canvas bounds + camera for every Model tile whose grid display is on.
    /// Each pane renders its own grid independently of which tile is active or
    /// hovered, so the grid never flickers as the cursor crosses panes. The
    /// active tile uses the live camera (mid-orbit/pan); others use their
    /// stored camera. (#121)
    /// Screen rect + camera for every grid-on sub-view in the current layout —
    /// model tiles in model space, the sheet plus each floating viewport
    /// (clipped to its rectangle) in paper space. Derived from the same
    /// `active_viewports` enumeration the renderer uses, so the grid overlay can
    /// never drift from the views actually on screen (issue #121). The grid

    pub fn active_model_tile_bounds(&self, vw: f32, vh: f32) -> iced::Rectangle {
        if self.current_layout != "Model" {
            return iced::Rectangle { x: 0.0, y: 0.0, width: vw, height: vh };
        }
        let tiles = self.model_tiles.borrow();
        let active = self.active_model_tile.get().min(tiles.len().saturating_sub(1));
        match tiles.get(active) {
            Some(t) => iced::Rectangle {
                x: t.rect.x * vw,
                y: t.rect.y * vh,
                width: (t.rect.width * vw).max(1.0),
                height: (t.rect.height * vh).max(1.0),
            },
            None => iced::Rectangle { x: 0.0, y: 0.0, width: vw, height: vh },
        }
    }
}

#[derive(Copy, Clone, Debug)]
enum ContactSide {
    Left,
    Right,
    Top,
    Bottom,
}

fn overlap_len(a: (f32, f32), b: (f32, f32)) -> f32 {
    (a.1.min(b.1) - a.0.max(b.0)).max(0.0)
}
