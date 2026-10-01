//! A message row with its hover action bar floating over it.
//!
//! The bar used to be a second `stack` layer, but a stack sizes every layer
//! to its first one: on a single-line row (shorter than the bar) the bar was
//! cut off. Here the bar is drawn as an overlay at its natural size, anchored
//! to the row's top-right corner and kept inside the visible list.
//!
//! While the cursor is on the overlay, iced gives the widgets underneath a
//! "levitating" cursor, so the row's `mouse_area` reports an exit: the caller
//! wraps the bar in its own `mouse_area` and treats the row's exit as
//! "still hovered" while the bar has the cursor (`Message::HoverBar`).
//!
//! Both children are always in the widget tree (the bar only shows when
//! `show`), so hovering never reshapes the tree and resets text selection.

use iced::advanced::layout::{self, Layout};
use iced::advanced::mouse;
use iced::advanced::overlay;
use iced::advanced::renderer;
use iced::advanced::widget::tree::Tree;
use iced::advanced::widget::Operation;
use iced::advanced::{Clipboard, Shell, Widget};
use iced::{Element, Event, Length, Point, Rectangle, Size, Vector};

pub struct HoverBar<'a, Message> {
    base: Element<'a, Message>,
    bar: Element<'a, Message>,
    show: bool,
    /// Gap between the bar and the row's right edge.
    inset: f32,
}

impl<'a, Message> HoverBar<'a, Message> {
    pub fn new(base: impl Into<Element<'a, Message>>, bar: impl Into<Element<'a, Message>>, show: bool) -> Self {
        Self { base: base.into(), bar: bar.into(), show, inset: 0.0 }
    }

    pub fn inset(mut self, inset: f32) -> Self {
        self.inset = inset;
        self
    }
}

impl<'a, Message> Widget<Message, iced::Theme, iced::Renderer> for HoverBar<'a, Message> {
    fn size(&self) -> Size<Length> {
        self.base.as_widget().size()
    }

    fn size_hint(&self) -> Size<Length> {
        self.base.as_widget().size_hint()
    }

    fn children(&self) -> Vec<Tree> {
        vec![Tree::new(&self.base), Tree::new(&self.bar)]
    }

    fn diff(&self, tree: &mut Tree) {
        tree.diff_children(&[&self.base, &self.bar]);
    }

    fn layout(&mut self, tree: &mut Tree, renderer: &iced::Renderer, limits: &layout::Limits) -> layout::Node {
        // Only the row takes space; the bar is laid out in its overlay.
        let base = self.base.as_widget_mut().layout(&mut tree.children[0], renderer, limits);
        layout::Node::with_children(base.size(), vec![base])
    }

    fn draw(
        &self,
        tree: &Tree,
        renderer: &mut iced::Renderer,
        theme: &iced::Theme,
        style: &renderer::Style,
        layout: Layout<'_>,
        cursor: mouse::Cursor,
        viewport: &Rectangle,
    ) {
        let base = layout.children().next().unwrap();
        self.base.as_widget().draw(&tree.children[0], renderer, theme, style, base, cursor, viewport);
    }

    fn operate(&mut self, tree: &mut Tree, layout: Layout<'_>, renderer: &iced::Renderer, operation: &mut dyn Operation) {
        let base = layout.children().next().unwrap();
        self.base.as_widget_mut().operate(&mut tree.children[0], base, renderer, operation);
    }

    fn update(
        &mut self,
        tree: &mut Tree,
        event: &Event,
        layout: Layout<'_>,
        cursor: mouse::Cursor,
        renderer: &iced::Renderer,
        clipboard: &mut dyn Clipboard,
        shell: &mut Shell<'_, Message>,
        viewport: &Rectangle,
    ) {
        let base = layout.children().next().unwrap();
        self.base.as_widget_mut().update(&mut tree.children[0], event, base, cursor, renderer, clipboard, shell, viewport);
    }

    fn mouse_interaction(
        &self,
        tree: &Tree,
        layout: Layout<'_>,
        cursor: mouse::Cursor,
        viewport: &Rectangle,
        renderer: &iced::Renderer,
    ) -> mouse::Interaction {
        let base = layout.children().next().unwrap();
        self.base.as_widget().mouse_interaction(&tree.children[0], base, cursor, viewport, renderer)
    }

    fn overlay<'b>(
        &'b mut self,
        tree: &'b mut Tree,
        layout: Layout<'b>,
        renderer: &iced::Renderer,
        viewport: &Rectangle,
        translation: Vector,
    ) -> Option<overlay::Element<'b, Message, iced::Theme, iced::Renderer>> {
        let base_layout = layout.children().next().unwrap();
        let (base_tree, bar_tree) = tree.children.split_at_mut(1);
        let base = self.base.as_widget_mut().overlay(&mut base_tree[0], base_layout, renderer, viewport, translation);
        if !self.show {
            return base;
        }
        let bar = overlay::Element::new(Box::new(BarOverlay {
            content: &mut self.bar,
            tree: &mut bar_tree[0],
            anchor: layout.bounds() + translation,
            // The list's visible area (scrollables pass theirs down).
            visible: *viewport,
            inset: self.inset,
            window: Size::ZERO,
        }));
        Some(match base {
            Some(base) => overlay::Group::with_children(vec![base, bar]).overlay(),
            None => bar,
        })
    }
}

impl<'a, Message: 'a> From<HoverBar<'a, Message>> for Element<'a, Message> {
    fn from(widget: HoverBar<'a, Message>) -> Self {
        Element::new(widget)
    }
}

struct BarOverlay<'a, 'b, Message> {
    content: &'a mut Element<'b, Message>,
    tree: &'a mut Tree,
    /// The row, in window coordinates.
    anchor: Rectangle,
    visible: Rectangle,
    inset: f32,
    /// Window size, recorded at layout for nested overlays (tooltips).
    window: Size,
}

impl<Message> overlay::Overlay<Message, iced::Theme, iced::Renderer> for BarOverlay<'_, '_, Message> {
    fn layout(&mut self, renderer: &iced::Renderer, bounds: Size) -> layout::Node {
        self.window = bounds;
        let limits = layout::Limits::new(Size::ZERO, Size::new(self.anchor.width, bounds.height));
        let node = self.content.as_widget_mut().layout(self.tree, renderer, &limits);
        let size = node.size();
        let x = (self.anchor.x + self.anchor.width - size.width - self.inset).max(self.anchor.x);
        // Top-aligned with the row, but never past the list's edges (the
        // last row's bar would otherwise cover the composer).
        let bottom = self.visible.y + self.visible.height - size.height;
        let y = self.anchor.y.min(bottom).max(self.visible.y);
        node.move_to(Point::new(x, y))
    }

    fn draw(
        &self,
        renderer: &mut iced::Renderer,
        theme: &iced::Theme,
        style: &renderer::Style,
        layout: Layout<'_>,
        cursor: mouse::Cursor,
    ) {
        self.content.as_widget().draw(self.tree, renderer, theme, style, layout, cursor, &layout.bounds());
    }

    fn operate(&mut self, layout: Layout<'_>, renderer: &iced::Renderer, operation: &mut dyn Operation) {
        self.content.as_widget_mut().operate(self.tree, layout, renderer, operation);
    }

    fn update(
        &mut self,
        event: &Event,
        layout: Layout<'_>,
        cursor: mouse::Cursor,
        renderer: &iced::Renderer,
        clipboard: &mut dyn Clipboard,
        shell: &mut Shell<'_, Message>,
    ) {
        let bounds = layout.bounds();
        self.content.as_widget_mut().update(self.tree, event, layout, cursor, renderer, clipboard, shell, &bounds);
    }

    fn mouse_interaction(&self, layout: Layout<'_>, cursor: mouse::Cursor, renderer: &iced::Renderer) -> mouse::Interaction {
        self.content.as_widget().mouse_interaction(self.tree, layout, cursor, &layout.bounds(), renderer)
    }

    fn overlay<'c>(
        &'c mut self,
        layout: Layout<'c>,
        renderer: &iced::Renderer,
    ) -> Option<overlay::Element<'c, Message, iced::Theme, iced::Renderer>> {
        // The buttons' tooltips.
        let window = Rectangle::with_size(self.window);
        self.content.as_widget_mut().overlay(self.tree, layout, renderer, &window, Vector::ZERO)
    }
}

#[cfg(test)]
mod tests {
    use crate::message::Message;
    use crate::panes::flow_tests::app;

    fn hovered(app: &crate::state::FluxChat) -> Option<&str> {
        app.hovered_message.as_deref()
    }

    #[tokio::test]
    async fn moving_onto_the_bar_keeps_the_hover() {
        let mut app = app();
        let _ = app.update(Message::HoverMessage(Some("$a".into())));
        // Onto the bar: the bar's enter comes first, then the row and the
        // list underneath report the cursor gone.
        let _ = app.update(Message::HoverBar(Some("$a".into())));
        let _ = app.update(Message::HoverLeft(Some("$a".into())));
        let _ = app.update(Message::HoverLeft(None));
        assert_eq!(hovered(&app), Some("$a"));
        // Back onto the row: the bar's exit, then the row's enter.
        let _ = app.update(Message::HoverBar(None));
        let _ = app.update(Message::HoverMessage(Some("$a".into())));
        assert_eq!(hovered(&app), Some("$a"));
        // Off the bar to somewhere else entirely.
        let _ = app.update(Message::HoverBar(Some("$a".into())));
        let _ = app.update(Message::HoverBar(None));
        assert_eq!(hovered(&app), None);
    }

    #[tokio::test]
    async fn leaving_a_row_ends_its_hover_only() {
        let mut app = app();
        let _ = app.update(Message::HoverMessage(Some("$a".into())));
        let _ = app.update(Message::HoverMessage(Some("$b".into())));
        // $a's exit can arrive after $b's enter.
        let _ = app.update(Message::HoverLeft(Some("$a".into())));
        assert_eq!(hovered(&app), Some("$b"));
        let _ = app.update(Message::HoverLeft(Some("$b".into())));
        assert_eq!(hovered(&app), None);
    }
}
