//! Collapsing ribbon panels.
//!
//! Given a list of panels (each with an inline element, a collapsed title
//! button, and a full flyout), this widget lays out as many inline panels as
//! fit the width from the left and collapses the rest into their title buttons.
//! Clicking a collapsed button opens that panel's flyout as an overlay anchored
//! just below the button.

use std::cell::Cell;

use iced::advanced::layout::{self, Layout};
use iced::advanced::widget::{self, Widget};
use iced::advanced::{mouse, overlay, renderer, Clipboard, Shell};
use iced::{Element, Event, Length, Point, Rectangle, Renderer, Size, Theme, Vector};

use crate::app::Message;

/// One ribbon panel in its three renderings.
pub struct Panel<'a> {
    pub id: String,
    pub inline: Element<'a, Message>,
    pub button: Element<'a, Message>,
    pub flyout: Element<'a, Message>,
}

pub struct CollapsePanels<'a> {
    panels: Vec<Panel<'a>>,
    /// Title of the panel whose flyout is open (if any).
    open: Option<String>,
    /// Row height (a full ribbon tool-area height).
    row_h: f32,
    /// Number of panels shown inline from the left; set during layout.
    split: Cell<usize>,
}

impl<'a> CollapsePanels<'a> {
    pub fn new(panels: Vec<Panel<'a>>, open: Option<String>, row_h: f32) -> Self {
        Self {
            panels,
            open,
            row_h,
            split: Cell::new(0),
        }
    }

    /// Element used inline for panel `i` given the split `k`.
    fn shown(&self, i: usize, k: usize) -> &Element<'a, Message> {
        if i < k {
            &self.panels[i].inline
        } else {
            &self.panels[i].button
        }
    }

    fn shown_mut(&mut self, i: usize, k: usize) -> &mut Element<'a, Message> {
        if i < k {
            &mut self.panels[i].inline
        } else {
            &mut self.panels[i].button
        }
    }

    /// Tree index of the element shown inline for panel `i` given split `k`.
    fn shown_tree(i: usize, k: usize) -> usize {
        // trees are laid out [inline_0, button_0, flyout_0, inline_1, …]
        if i < k {
            3 * i
        } else {
            3 * i + 1
        }
    }
}

impl<'a> Widget<Message, Theme, Renderer> for CollapsePanels<'a> {
    fn children(&self) -> Vec<widget::Tree> {
        let mut v = Vec::with_capacity(self.panels.len() * 3);
        for p in &self.panels {
            v.push(widget::Tree::new(&p.inline));
            v.push(widget::Tree::new(&p.button));
            v.push(widget::Tree::new(&p.flyout));
        }
        v
    }

    fn diff(&self, tree: &mut widget::Tree) {
        let mut refs: Vec<&dyn Widget<Message, Theme, Renderer>> = Vec::new();
        for p in &self.panels {
            refs.push(p.inline.as_widget());
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
        let natural =
            layout::Limits::new(Size::ZERO, Size::new(f32::INFINITY, f32::INFINITY));
        let n = self.panels.len();

        // Measure every inline panel and every collapsed button.
        let mut inline_w = vec![0.0f32; n];
        let mut button_w = vec![0.0f32; n];
        for i in 0..n {
            let inline_node = self.panels[i].inline.as_widget_mut().layout(
                &mut tree.children[3 * i],
                renderer,
                &natural,
            );
            let button_node = self.panels[i].button.as_widget_mut().layout(
                &mut tree.children[3 * i + 1],
                renderer,
                &natural,
            );
            inline_w[i] = inline_node.size().width;
            button_w[i] = button_node.size().width;
        }

        // Expand a prefix of panels from the left while the row still fits with
        // the remaining panels collapsed to buttons.
        let all_buttons: f32 = button_w.iter().sum();
        let mut used = all_buttons;
        let mut k = 0;
        for i in 0..n {
            let candidate = used - button_w[i] + inline_w[i];
            if candidate <= max_w {
                used = candidate;
                k = i + 1;
            } else {
                break;
            }
        }
        self.split.set(k);

        // Place the chosen element for each panel left-to-right.
        let mut children: Vec<layout::Node> = Vec::with_capacity(n);
        let mut x = 0.0f32;
        for i in 0..n {
            let tree_idx = Self::shown_tree(i, k);
            let node = self.shown_mut(i, k).as_widget_mut().layout(
                &mut tree.children[tree_idx],
                renderer,
                &natural,
            );
            let h = node.size().height;
            let y = ((self.row_h - h) / 2.0).max(0.0);
            let w = node.size().width;
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
        let k = self.split.get();
        for (i, child_layout) in layout.children().enumerate() {
            let tree_idx = Self::shown_tree(i, k);
            self.shown_mut(i, k).as_widget_mut().update(
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
        let k = self.split.get();
        let mut interaction = mouse::Interaction::default();
        for (i, child_layout) in layout.children().enumerate() {
            let tree_idx = Self::shown_tree(i, k);
            let it = self.shown(i, k).as_widget().mouse_interaction(
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
        let k = self.split.get();
        for (i, child_layout) in layout.children().enumerate() {
            let tree_idx = Self::shown_tree(i, k);
            self.shown_mut(i, k).as_widget_mut().operate(
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
        let k = self.split.get();
        for (i, child_layout) in layout.children().enumerate() {
            let tree_idx = Self::shown_tree(i, k);
            self.shown(i, k).as_widget().draw(
                &tree.children[tree_idx],
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
        _renderer: &Renderer,
        _viewport: &Rectangle,
        translation: Vector,
    ) -> Option<overlay::Element<'b, Message, Theme, Renderer>> {
        let k = self.split.get();
        let open_id = self.open.clone()?;
        let p = self.panels.iter().position(|pan| pan.id == open_id)?;
        // Only collapsed panels (index >= k) show a flyout.
        if p < k {
            return None;
        }

        let child_layout = layout.children().nth(p)?;
        let b = child_layout.bounds();
        let anchor = Point::new(b.x + translation.x, b.y + b.height + translation.y);

        Some(overlay::Element::new(Box::new(FlyoutOverlay {
            flyout: &mut self.panels[p].flyout,
            tree: &mut tree.children[3 * p + 2],
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

        // Close when a press lands outside the flyout. Capture the event so the
        // press doesn't also reach the collapsed button underneath (which would
        // immediately re-open the flyout).
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
