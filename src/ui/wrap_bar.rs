//! Adaptive two-block bar that wraps its trailing block onto a second row when
//! the width can't hold both blocks side by side. Used by the ribbon tab row
//! (lead = quick-access + undo/redo, trail = tabs) and the status bar
//! (lead = menu, trail = status pills, with the layout-tab strip as the Fill
//! `middle` filler between them).
//!
//! Layout, per row:
//!   • one row  → `lead` at the left; `trail` packed after it (`justify_end`
//!                false) or flush to the right edge (`justify_end` true);
//!                an optional Fill `middle` stretches through the gap.
//!   • two rows → `lead` (+ `middle` filling the rest) on row 1; `trail` on
//!                row 2, left- or right-aligned per `justify_end`.
//!
//! The measured total height is written to `height_out` (if set) so callers can
//! anchor overlays below the possibly-taller bar.

use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::Arc;

use iced::advanced::layout::{self, Layout};
use iced::advanced::widget::{self, Widget};
use iced::advanced::{mouse, overlay, renderer, Clipboard, Shell};
use iced::{Element, Event, Length, Point, Rectangle, Renderer, Size, Theme, Vector};

use crate::app::Message;

pub struct WrapBar<'a> {
    lead: Element<'a, Message>,
    /// Optional Fill filler occupying the gap between `lead` and `trail`.
    middle: Option<Element<'a, Message>>,
    trail: Element<'a, Message>,
    /// Gap between adjacent blocks on a shared row.
    spacing: f32,
    /// Minimum height of a single row.
    min_row_h: f32,
    /// When true, `trail` sits at the right edge (justified); otherwise it is
    /// packed immediately after `lead`.
    justify_end: bool,
    /// Receives the measured total height (bits of an `f32`).
    height_out: Option<Arc<AtomicU32>>,
}

impl<'a> WrapBar<'a> {
    pub fn new(lead: Element<'a, Message>, trail: Element<'a, Message>) -> Self {
        Self {
            lead,
            middle: None,
            trail,
            spacing: 0.0,
            min_row_h: 28.0,
            justify_end: false,
            height_out: None,
        }
    }

    pub fn spacing(mut self, spacing: f32) -> Self {
        self.spacing = spacing;
        self
    }

    pub fn min_row_h(mut self, h: f32) -> Self {
        self.min_row_h = h;
        self
    }

    pub fn justify_end(mut self, justify: bool) -> Self {
        self.justify_end = justify;
        self
    }

    pub fn middle(mut self, middle: Element<'a, Message>) -> Self {
        self.middle = Some(middle);
        self
    }

    pub fn report_height(mut self, out: Arc<AtomicU32>) -> Self {
        self.height_out = Some(out);
        self
    }

    /// Elements in row order: lead, [middle], trail.
    fn refs(&self) -> Vec<&Element<'a, Message>> {
        let mut v = Vec::with_capacity(3);
        v.push(&self.lead);
        if let Some(m) = &self.middle {
            v.push(m);
        }
        v.push(&self.trail);
        v
    }

    fn refs_mut(&mut self) -> Vec<&mut Element<'a, Message>> {
        let mut v = Vec::with_capacity(3);
        v.push(&mut self.lead);
        if let Some(m) = &mut self.middle {
            v.push(m);
        }
        v.push(&mut self.trail);
        v
    }
}

impl<'a> Widget<Message, Theme, Renderer> for WrapBar<'a> {
    fn children(&self) -> Vec<widget::Tree> {
        self.refs().iter().map(|e| widget::Tree::new(*e)).collect()
    }

    fn diff(&self, tree: &mut widget::Tree) {
        let refs: Vec<_> = self.refs().iter().map(|e| e.as_widget()).collect();
        tree.diff_children(&refs);
    }

    fn size(&self) -> Size<Length> {
        Size::new(Length::Fill, Length::Shrink)
    }

    fn layout(
        &mut self,
        tree: &mut widget::Tree,
        renderer: &Renderer,
        limits: &layout::Limits,
    ) -> layout::Node {
        let max = limits.max();
        // Measure blocks at their natural (unwrapped) width so the fit decision
        // and one-row placement use true content widths. A flex-wrap `trail`
        // (WrapFlow) only wraps when it is later laid out with a bounded width.
        let natural =
            layout::Limits::new(Size::ZERO, Size::new(f32::INFINITY, f32::INFINITY));

        let has_middle = self.middle.is_some();
        let trail_idx = if has_middle { 2 } else { 1 };

        let mut lead_node =
            self.lead
                .as_widget_mut()
                .layout(&mut tree.children[0], renderer, &natural);
        let mut trail_node = self.trail.as_widget_mut().layout(
            &mut tree.children[trail_idx],
            renderer,
            &natural,
        );

        let mut lead_sz = lead_node.size();
        let mut trail_sz = trail_node.size();

        let width = if max.width.is_finite() {
            max.width
        } else {
            lead_sz.width + self.spacing + trail_sz.width
        };

        let fits = max.width.is_finite()
            && lead_sz.width + self.spacing + trail_sz.width <= max.width;

        let row_h = lead_sz.height.max(trail_sz.height).max(self.min_row_h);
        let bounded = layout::Limits::new(Size::ZERO, Size::new(width, f32::INFINITY));

        let (lead_pos, trail_pos, middle_x, middle_gap, total_h);
        if fits {
            // One row: lead left, trail packed after it or flush right.
            let trail_x = if self.justify_end {
                (width - trail_sz.width).max(lead_sz.width + self.spacing)
            } else {
                lead_sz.width + self.spacing
            };
            lead_pos = Point::new(0.0, (row_h - lead_sz.height) / 2.0);
            trail_pos = Point::new(trail_x, (row_h - trail_sz.height) / 2.0);
            middle_x = lead_sz.width + self.spacing;
            middle_gap = (trail_x - self.spacing - middle_x).max(0.0);
            total_h = row_h;
        } else if has_middle {
            // 3-slot justified bar: lead on row 1, middle fills the rest of it,
            // trail (flex-wrap) drops onto the row(s) below.
            lead_pos = Point::new(0.0, (row_h - lead_sz.height) / 2.0);
            middle_x = lead_sz.width + self.spacing;
            middle_gap = (width - middle_x).max(0.0);

            trail_node = self.trail.as_widget_mut().layout(
                &mut tree.children[trail_idx],
                renderer,
                &bounded,
            );
            trail_sz = trail_node.size();
            trail_pos = Point::new(0.0, row_h);
            total_h = row_h + trail_sz.height;
        } else {
            // 2-slot dual-wrap: lead and trail each wrap within their OWN row
            // band, stacked so a wrapped lead item never lands on a trail row.
            lead_node =
                self.lead
                    .as_widget_mut()
                    .layout(&mut tree.children[0], renderer, &bounded);
            lead_sz = lead_node.size();
            let lead_h = lead_sz.height.max(self.min_row_h);

            trail_node = self.trail.as_widget_mut().layout(
                &mut tree.children[trail_idx],
                renderer,
                &bounded,
            );
            trail_sz = trail_node.size();
            let trail_h = trail_sz.height.max(self.min_row_h);

            lead_pos = Point::new(0.0, (lead_h - lead_sz.height) / 2.0);
            trail_pos = Point::new(0.0, lead_h + (trail_h - trail_sz.height) / 2.0);
            middle_x = 0.0;
            middle_gap = 0.0;
            total_h = lead_h + trail_h;
        }

        let mut children: Vec<layout::Node> = Vec::with_capacity(3);
        children.push(lead_node.move_to(lead_pos));

        if has_middle {
            let mid_limits =
                layout::Limits::new(Size::new(middle_gap, 0.0), Size::new(middle_gap, row_h));
            let mut mid_node = self.middle.as_mut().unwrap().as_widget_mut().layout(
                &mut tree.children[1],
                renderer,
                &mid_limits,
            );
            let mid_y = ((row_h - mid_node.size().height) / 2.0).max(0.0);
            mid_node = mid_node.move_to(Point::new(middle_x, mid_y));
            children.push(mid_node);
        }

        children.push(trail_node.move_to(trail_pos));

        if let Some(out) = &self.height_out {
            out.store(total_h.to_bits(), Ordering::Relaxed);
        }

        layout::Node::with_children(Size::new(width, total_h), children)
    }

    fn update(
        &mut self,
        tree: &mut widget::Tree,
        event: &Event,
        layout: Layout<'_>,
        cursor: mouse::Cursor,
        renderer: &Renderer,
        clipboard: &mut dyn Clipboard,
        shell: &mut Shell<'_, Message>,
        viewport: &Rectangle,
    ) {
        for ((child, state), child_layout) in self
            .refs_mut()
            .into_iter()
            .zip(tree.children.iter_mut())
            .zip(layout.children())
        {
            child.as_widget_mut().update(
                state,
                event,
                child_layout,
                cursor,
                renderer,
                clipboard,
                shell,
                viewport,
            );
        }
    }

    fn mouse_interaction(
        &self,
        tree: &widget::Tree,
        layout: Layout<'_>,
        cursor: mouse::Cursor,
        viewport: &Rectangle,
        renderer: &Renderer,
    ) -> mouse::Interaction {
        let mut interaction = mouse::Interaction::default();
        for ((child, state), child_layout) in self
            .refs()
            .into_iter()
            .zip(tree.children.iter())
            .zip(layout.children())
        {
            let i = child.as_widget().mouse_interaction(
                state,
                child_layout,
                cursor,
                viewport,
                renderer,
            );
            if i != mouse::Interaction::default() {
                interaction = i;
            }
        }
        interaction
    }

    fn operate(
        &mut self,
        tree: &mut widget::Tree,
        layout: Layout<'_>,
        renderer: &Renderer,
        operation: &mut dyn widget::Operation,
    ) {
        for ((child, state), child_layout) in self
            .refs_mut()
            .into_iter()
            .zip(tree.children.iter_mut())
            .zip(layout.children())
        {
            child
                .as_widget_mut()
                .operate(state, child_layout, renderer, operation);
        }
    }

    fn draw(
        &self,
        tree: &widget::Tree,
        renderer: &mut Renderer,
        theme: &Theme,
        style: &renderer::Style,
        layout: Layout<'_>,
        cursor: mouse::Cursor,
        viewport: &Rectangle,
    ) {
        for ((child, state), child_layout) in self
            .refs()
            .into_iter()
            .zip(tree.children.iter())
            .zip(layout.children())
        {
            child.as_widget().draw(
                state,
                renderer,
                theme,
                style,
                child_layout,
                cursor,
                viewport,
            );
        }
    }

    fn overlay<'b>(
        &'b mut self,
        tree: &'b mut widget::Tree,
        layout: Layout<'b>,
        renderer: &Renderer,
        viewport: &Rectangle,
        translation: Vector,
    ) -> Option<overlay::Element<'b, Message, Theme, Renderer>> {
        // At most one child (the hovered tooltip, if any) yields an overlay;
        // return the first. Borrow fields and tree slots disjointly so the
        // returned overlay can outlive this call.
        let layouts: Vec<Layout<'b>> = layout.children().collect();
        let has_middle = self.middle.is_some();

        // Split the tree children into disjoint mutable slots.
        let (lead_tree, rest) = tree.children.split_at_mut(1);
        if let Some(ll) = layouts.first() {
            if let Some(ov) = self.lead.as_widget_mut().overlay(
                &mut lead_tree[0],
                *ll,
                renderer,
                viewport,
                translation,
            ) {
                return Some(ov);
            }
        }

        if has_middle {
            let (mid_tree, trail_tree) = rest.split_at_mut(1);
            if let (Some(m), Some(ml)) = (self.middle.as_mut(), layouts.get(1)) {
                if let Some(ov) = m.as_widget_mut().overlay(
                    &mut mid_tree[0],
                    *ml,
                    renderer,
                    viewport,
                    translation,
                ) {
                    return Some(ov);
                }
            }
            if let Some(tl) = layouts.get(2) {
                return self.trail.as_widget_mut().overlay(
                    &mut trail_tree[0],
                    *tl,
                    renderer,
                    viewport,
                    translation,
                );
            }
        } else if let Some(tl) = layouts.get(1) {
            return self.trail.as_widget_mut().overlay(
                &mut rest[0],
                *tl,
                renderer,
                viewport,
                translation,
            );
        }
        None
    }
}

impl<'a> From<WrapBar<'a>> for Element<'a, Message> {
    fn from(w: WrapBar<'a>) -> Self {
        Element::new(w)
    }
}

/// Flex-wrap row: lays its items left-to-right and wraps onto a new row when
/// the next item would exceed the available width. Each row is `row_h` tall and
/// items are vertically centred. Used for the status-bar pills so they spread
/// across multiple rows when the width can't hold them on one line.
pub struct WrapFlow<'a> {
    items: Vec<Element<'a, Message>>,
    spacing_x: f32,
    spacing_y: f32,
    row_h: f32,
}

impl<'a> WrapFlow<'a> {
    pub fn new(items: Vec<Element<'a, Message>>) -> Self {
        Self {
            items,
            spacing_x: 2.0,
            spacing_y: 0.0,
            row_h: 28.0,
        }
    }

    pub fn spacing_x(mut self, s: f32) -> Self {
        self.spacing_x = s;
        self
    }

    pub fn row_h(mut self, h: f32) -> Self {
        self.row_h = h;
        self
    }
}

impl<'a> Widget<Message, Theme, Renderer> for WrapFlow<'a> {
    fn children(&self) -> Vec<widget::Tree> {
        self.items.iter().map(widget::Tree::new).collect()
    }

    fn diff(&self, tree: &mut widget::Tree) {
        let refs: Vec<_> = self.items.iter().map(|e| e.as_widget()).collect();
        tree.diff_children(&refs);
    }

    fn size(&self) -> Size<Length> {
        Size::new(Length::Shrink, Length::Shrink)
    }

    fn layout(
        &mut self,
        tree: &mut widget::Tree,
        renderer: &Renderer,
        limits: &layout::Limits,
    ) -> layout::Node {
        let max_w = limits.max().width;
        let natural =
            layout::Limits::new(Size::ZERO, Size::new(f32::INFINITY, f32::INFINITY));

        let mut nodes: Vec<layout::Node> = Vec::with_capacity(self.items.len());
        let mut x = 0.0f32;
        let mut y = 0.0f32;
        let mut used_w = 0.0f32;

        for (item, state) in self.items.iter_mut().zip(tree.children.iter_mut()) {
            let node = item.as_widget_mut().layout(state, renderer, &natural);
            let sz = node.size();
            if x > 0.0 && x + sz.width > max_w {
                x = 0.0;
                y += self.row_h + self.spacing_y;
            }
            let cy = y + ((self.row_h - sz.height) / 2.0).max(0.0);
            nodes.push(node.move_to(Point::new(x, cy)));
            x += sz.width + self.spacing_x;
            used_w = used_w.max(x - self.spacing_x);
        }

        let total_h = if self.items.is_empty() {
            self.row_h
        } else {
            y + self.row_h
        };
        layout::Node::with_children(Size::new(used_w.max(0.0), total_h), nodes)
    }

    fn update(
        &mut self,
        tree: &mut widget::Tree,
        event: &Event,
        layout: Layout<'_>,
        cursor: mouse::Cursor,
        renderer: &Renderer,
        clipboard: &mut dyn Clipboard,
        shell: &mut Shell<'_, Message>,
        viewport: &Rectangle,
    ) {
        for ((item, state), child_layout) in self
            .items
            .iter_mut()
            .zip(tree.children.iter_mut())
            .zip(layout.children())
        {
            item.as_widget_mut().update(
                state,
                event,
                child_layout,
                cursor,
                renderer,
                clipboard,
                shell,
                viewport,
            );
        }
    }

    fn mouse_interaction(
        &self,
        tree: &widget::Tree,
        layout: Layout<'_>,
        cursor: mouse::Cursor,
        viewport: &Rectangle,
        renderer: &Renderer,
    ) -> mouse::Interaction {
        let mut interaction = mouse::Interaction::default();
        for ((item, state), child_layout) in self
            .items
            .iter()
            .zip(tree.children.iter())
            .zip(layout.children())
        {
            let i = item.as_widget().mouse_interaction(
                state,
                child_layout,
                cursor,
                viewport,
                renderer,
            );
            if i != mouse::Interaction::default() {
                interaction = i;
            }
        }
        interaction
    }

    fn operate(
        &mut self,
        tree: &mut widget::Tree,
        layout: Layout<'_>,
        renderer: &Renderer,
        operation: &mut dyn widget::Operation,
    ) {
        for ((item, state), child_layout) in self
            .items
            .iter_mut()
            .zip(tree.children.iter_mut())
            .zip(layout.children())
        {
            item.as_widget_mut()
                .operate(state, child_layout, renderer, operation);
        }
    }

    fn draw(
        &self,
        tree: &widget::Tree,
        renderer: &mut Renderer,
        theme: &Theme,
        style: &renderer::Style,
        layout: Layout<'_>,
        cursor: mouse::Cursor,
        viewport: &Rectangle,
    ) {
        for ((item, state), child_layout) in self
            .items
            .iter()
            .zip(tree.children.iter())
            .zip(layout.children())
        {
            item.as_widget()
                .draw(state, renderer, theme, style, child_layout, cursor, viewport);
        }
    }

    fn overlay<'b>(
        &'b mut self,
        tree: &'b mut widget::Tree,
        layout: Layout<'b>,
        renderer: &Renderer,
        viewport: &Rectangle,
        translation: Vector,
    ) -> Option<overlay::Element<'b, Message, Theme, Renderer>> {
        overlay::from_children(
            &mut self.items,
            tree,
            layout,
            renderer,
            viewport,
            translation,
        )
    }
}

impl<'a> From<WrapFlow<'a>> for Element<'a, Message> {
    fn from(w: WrapFlow<'a>) -> Self {
        Element::new(w)
    }
}
