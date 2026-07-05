//! Adaptive ribbon panels.
//!
//! Lays the active module's panels on one row. When they don't all fit, panels
//! degrade **from the right**: first a panel's large buttons shrink to compact
//! icon columns, then — if it still doesn't fit — the panel collapses to a title
//! button whose click opens the full panel as a flyout overlay.

use std::cell::RefCell;

use iced::advanced::layout::{self, Layout};
use iced::advanced::widget::{self, Widget};
use iced::advanced::{mouse, overlay, renderer, Clipboard, Renderer as _, Shell};
use iced::{
    Background, Border, Color, Element, Event, Length, Point, Rectangle, Renderer, Shadow, Size,
    Theme, Vector,
};

use crate::app::Message;

/// One panel in its four renderings.
pub struct Panel<'a> {
    pub id: String,
    pub full: Element<'a, Message>,
    pub compact: Element<'a, Message>,
    pub button: Element<'a, Message>,
    pub flyout: Element<'a, Message>,
}

// Per-panel degradation level; also the offset of the shown element within the
// panel's 4 trees ([full, compact, button, flyout]).
const FULL: u8 = 0;
const COMPACT: u8 = 1;
const COLLAPSED: u8 = 2;

pub struct CollapsePanels<'a> {
    panels: Vec<Panel<'a>>,
    /// Title of the panel whose flyout is open (if any).
    open: Option<String>,
    /// Row height (a full ribbon tool-area height).
    row_h: f32,
    /// Colour of the 1px divider drawn between panels.
    divider: Color,
    /// Chosen degradation level per panel; set during layout.
    levels: RefCell<Vec<u8>>,
}

impl<'a> CollapsePanels<'a> {
    pub fn new(panels: Vec<Panel<'a>>, open: Option<String>, row_h: f32, divider: Color) -> Self {
        let n = panels.len();
        Self {
            panels,
            open,
            row_h,
            divider,
            levels: RefCell::new(vec![FULL; n]),
        }
    }

    fn shown(&self, i: usize, level: u8) -> &Element<'a, Message> {
        match level {
            FULL => &self.panels[i].full,
            COMPACT => &self.panels[i].compact,
            _ => &self.panels[i].button,
        }
    }

    fn shown_mut(&mut self, i: usize, level: u8) -> &mut Element<'a, Message> {
        match level {
            FULL => &mut self.panels[i].full,
            COMPACT => &mut self.panels[i].compact,
            _ => &mut self.panels[i].button,
        }
    }

    fn levels_snapshot(&self, n: usize) -> Vec<u8> {
        let mut v = self.levels.borrow().clone();
        v.resize(n, FULL);
        v
    }
}

impl<'a> Widget<Message, Theme, Renderer> for CollapsePanels<'a> {
    fn children(&self) -> Vec<widget::Tree> {
        let mut v = Vec::with_capacity(self.panels.len() * 4);
        for p in &self.panels {
            v.push(widget::Tree::new(&p.full));
            v.push(widget::Tree::new(&p.compact));
            v.push(widget::Tree::new(&p.button));
            v.push(widget::Tree::new(&p.flyout));
        }
        v
    }

    fn diff(&self, tree: &mut widget::Tree) {
        let mut refs: Vec<&dyn Widget<Message, Theme, Renderer>> = Vec::new();
        for p in &self.panels {
            refs.push(p.full.as_widget());
            refs.push(p.compact.as_widget());
            refs.push(p.button.as_widget());
            refs.push(p.flyout.as_widget());
        }
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
        let natural = layout::Limits::new(Size::ZERO, Size::new(f32::INFINITY, f32::INFINITY));
        let n = self.panels.len();

        // Measure each panel at all three densities.
        let mut full_w = vec![0.0f32; n];
        let mut compact_w = vec![0.0f32; n];
        let mut button_w = vec![0.0f32; n];
        for i in 0..n {
            full_w[i] = self.panels[i].full.as_widget_mut()
                .layout(&mut tree.children[4 * i], renderer, &natural)
                .size()
                .width;
            compact_w[i] = self.panels[i].compact.as_widget_mut()
                .layout(&mut tree.children[4 * i + 1], renderer, &natural)
                .size()
                .width;
            button_w[i] = self.panels[i].button.as_widget_mut()
                .layout(&mut tree.children[4 * i + 2], renderer, &natural)
                .size()
                .width;
        }

        let width_of = |lv: u8, i: usize| -> f32 {
            match lv {
                FULL => full_w[i],
                COMPACT => compact_w[i],
                _ => button_w[i],
            }
        };
        let total = |levels: &[u8]| -> f32 { (0..n).map(|i| width_of(levels[i], i)).sum() };

        // Start all full; degrade from the RIGHT. Phase 1 shrinks large panels
        // to compact one at a time; phase 2 (only if compact still overflows)
        // collapses them to buttons.
        let mut levels = vec![FULL; n];
        for i in (0..n).rev() {
            if total(&levels) <= max_w {
                break;
            }
            levels[i] = COMPACT;
        }
        for i in (0..n).rev() {
            if total(&levels) <= max_w {
                break;
            }
            levels[i] = COLLAPSED;
        }
        *self.levels.borrow_mut() = levels.clone();

        // Place the chosen element for each panel left-to-right.
        let mut children: Vec<layout::Node> = Vec::with_capacity(n);
        let mut x = 0.0f32;
        for i in 0..n {
            let level = levels[i];
            let tree_idx = 4 * i + level as usize;
            let node = self.shown_mut(i, level).as_widget_mut().layout(
                &mut tree.children[tree_idx],
                renderer,
                &natural,
            );
            let h = node.size().height;
            let w = node.size().width;
            let y = ((self.row_h - h) / 2.0).max(0.0);
            children.push(node.move_to(Point::new(x, y)));
            x += w;
        }

        layout::Node::with_children(Size::new(x, self.row_h), children)
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
        let levels = self.levels_snapshot(self.panels.len());
        for (i, child_layout) in layout.children().enumerate() {
            let level = levels[i];
            let tree_idx = 4 * i + level as usize;
            self.shown_mut(i, level).as_widget_mut().update(
                &mut tree.children[tree_idx],
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
        let levels = self.levels_snapshot(self.panels.len());
        let mut interaction = mouse::Interaction::default();
        for (i, child_layout) in layout.children().enumerate() {
            let level = levels[i];
            let tree_idx = 4 * i + level as usize;
            let it = self.shown(i, level).as_widget().mouse_interaction(
                &tree.children[tree_idx],
                child_layout,
                cursor,
                viewport,
                renderer,
            );
            if it != mouse::Interaction::default() {
                interaction = it;
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
        let levels = self.levels_snapshot(self.panels.len());
        for (i, child_layout) in layout.children().enumerate() {
            let level = levels[i];
            let tree_idx = 4 * i + level as usize;
            self.shown_mut(i, level).as_widget_mut().operate(
                &mut tree.children[tree_idx],
                child_layout,
                renderer,
                operation,
            );
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
        let levels = self.levels_snapshot(self.panels.len());
        for (i, child_layout) in layout.children().enumerate() {
            let level = levels[i];
            let tree_idx = 4 * i + level as usize;
            self.shown(i, level).as_widget().draw(
                &tree.children[tree_idx],
                renderer,
                theme,
                style,
                child_layout,
                cursor,
                viewport,
            );
        }

        // 1px divider between adjacent panels, except between two collapsed
        // panels (whose buttons read better with no line between them).
        let bounds: Vec<Rectangle> = layout.children().map(|l| l.bounds()).collect();
        let wb = layout.bounds();
        for i in 0..self.panels.len().saturating_sub(1) {
            if levels[i] == COLLAPSED && levels[i + 1] == COLLAPSED {
                continue;
            }
            let x = bounds[i + 1].x;
            renderer.fill_quad(
                renderer::Quad {
                    bounds: Rectangle {
                        x,
                        y: wb.y,
                        width: 1.0,
                        height: wb.height,
                    },
                    border: Border::default(),
                    shadow: Shadow::default(),
                    snap: true,
                },
                Background::Color(self.divider),
            );
        }
    }

    fn overlay<'b>(
        &'b mut self,
        tree: &'b mut widget::Tree,
        layout: Layout<'b>,
        _renderer: &Renderer,
        _viewport: &Rectangle,
        translation: Vector,
    ) -> Option<overlay::Element<'b, Message, Theme, Renderer>> {
        let levels = self.levels_snapshot(self.panels.len());
        let open_id = self.open.clone()?;
        let p = self.panels.iter().position(|pan| pan.id == open_id)?;
        // Only a collapsed panel shows a flyout.
        if levels.get(p).copied().unwrap_or(FULL) != COLLAPSED {
            return None;
        }

        let child_layout = layout.children().nth(p)?;
        let b = child_layout.bounds();
        let anchor = Point::new(b.x + translation.x, b.y + b.height + translation.y);

        Some(overlay::Element::new(Box::new(FlyoutOverlay {
            flyout: &mut self.panels[p].flyout,
            tree: &mut tree.children[4 * p + 3],
            anchor,
        })))
    }
}

impl<'a> From<CollapsePanels<'a>> for Element<'a, Message> {
    fn from(w: CollapsePanels<'a>) -> Self {
        Element::new(w)
    }
}

/// Overlay that renders an open panel's flyout anchored below its button and
/// closes it when the user presses outside.
struct FlyoutOverlay<'a, 'b> {
    flyout: &'b mut Element<'a, Message>,
    tree: &'b mut widget::Tree,
    anchor: Point,
}

impl overlay::Overlay<Message, Theme, Renderer> for FlyoutOverlay<'_, '_> {
    fn layout(&mut self, renderer: &Renderer, bounds: Size) -> layout::Node {
        let viewport = Rectangle::with_size(bounds);
        let limits = layout::Limits::new(Size::ZERO, viewport.size());
        let node = self
            .flyout
            .as_widget_mut()
            .layout(self.tree, renderer, &limits);
        let size = node.size();
        let mut x = self.anchor.x;
        let mut y = self.anchor.y;
        if x + size.width > viewport.width {
            x = (viewport.width - size.width).max(0.0);
        }
        if y + size.height > viewport.height {
            y = (self.anchor.y - size.height).max(0.0);
        }
        layout::Node::with_children(size, vec![node]).translate(Vector::new(x, y))
    }

    fn draw(
        &self,
        renderer: &mut Renderer,
        theme: &Theme,
        style: &renderer::Style,
        layout: Layout<'_>,
        cursor: mouse::Cursor,
    ) {
        let child = layout.children().next().unwrap();
        self.flyout.as_widget().draw(
            self.tree,
            renderer,
            theme,
            style,
            child,
            cursor,
            &child.bounds(),
        );
    }

    fn update(
        &mut self,
        event: &Event,
        layout: Layout<'_>,
        cursor: mouse::Cursor,
        renderer: &Renderer,
        clipboard: &mut dyn Clipboard,
        shell: &mut Shell<'_, Message>,
    ) {
        let child = layout.children().next().unwrap();
        let vp = child.bounds();

        if let Event::Mouse(mouse::Event::ButtonPressed(_)) = event {
            if !cursor.is_over(vp) {
                shell.publish(Message::CloseRibbonDropdown);
                shell.capture_event();
                return;
            }
        }

        self.flyout
            .as_widget_mut()
            .update(self.tree, event, child, cursor, renderer, clipboard, shell, &vp);
    }

    fn operate(
        &mut self,
        layout: Layout<'_>,
        renderer: &Renderer,
        operation: &mut dyn widget::Operation,
    ) {
        let child = layout.children().next().unwrap();
        self.flyout
            .as_widget_mut()
            .operate(self.tree, child, renderer, operation);
    }

    fn mouse_interaction(
        &self,
        layout: Layout<'_>,
        cursor: mouse::Cursor,
        renderer: &Renderer,
    ) -> mouse::Interaction {
        let child = layout.children().next().unwrap();
        self.flyout
            .as_widget()
            .mouse_interaction(self.tree, child, cursor, &child.bounds(), renderer)
    }
}
