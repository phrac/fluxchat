//! Transparent wrapper whose widget-tree node has its own tag.
//!
//! iced keeps widget state (scroll offsets, text selection, ..) by tree
//! position and tag: when a pane switches between the full view and its
//! compact side-pane body, the new widgets would pick up the old ones' state
//! — a bottom-anchored side-pane list read the main list's "at the bottom"
//! offset as "at the top". Wrapping the side-pane body in `Isolated` makes the
//! tags differ at its root, so either side starts with fresh state.

use iced::advanced::layout;
use iced::advanced::mouse;
use iced::advanced::overlay;
use iced::advanced::renderer;
use iced::advanced::widget::tree::{self, Tree};
use iced::advanced::widget::Operation;
use iced::advanced::{Clipboard, Layout, Shell, Widget};
use iced::{Element, Event, Length, Rectangle, Size, Vector};

/// Tag type of the wrapper's tree node.
struct Marker;

pub struct Isolated<'a, Message> {
    content: Element<'a, Message>,
}

impl<'a, Message> Isolated<'a, Message> {
    pub fn new(content: impl Into<Element<'a, Message>>) -> Self {
        Self { content: content.into() }
    }
}

impl<'a, Message> Widget<Message, iced::Theme, iced::Renderer> for Isolated<'a, Message> {
    fn size(&self) -> Size<Length> {
        self.content.as_widget().size()
    }

    fn size_hint(&self) -> Size<Length> {
        self.content.as_widget().size_hint()
    }

    fn tag(&self) -> tree::Tag {
        tree::Tag::of::<Marker>()
    }

    fn state(&self) -> tree::State {
        tree::State::None
    }

    fn children(&self) -> Vec<Tree> {
        vec![Tree::new(&self.content)]
    }

    fn diff(&self, tree: &mut Tree) {
        tree.diff_children(std::slice::from_ref(&self.content));
    }

    fn layout(
        &mut self,
        tree: &mut Tree,
        renderer: &iced::Renderer,
        limits: &layout::Limits,
    ) -> layout::Node {
        let child = self.content.as_widget_mut().layout(&mut tree.children[0], renderer, limits);
        layout::Node::with_children(child.size(), vec![child])
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
        let child = layout.children().next().unwrap();
        self.content.as_widget().draw(&tree.children[0], renderer, theme, style, child, cursor, viewport)
    }

    fn operate(
        &mut self,
        tree: &mut Tree,
        layout: Layout<'_>,
        renderer: &iced::Renderer,
        operation: &mut dyn Operation,
    ) {
        let child = layout.children().next().unwrap();
        self.content.as_widget_mut().operate(&mut tree.children[0], child, renderer, operation)
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
        let child = layout.children().next().unwrap();
        self.content.as_widget_mut().update(
            &mut tree.children[0], event, child, cursor, renderer, clipboard, shell, viewport,
        )
    }

    fn mouse_interaction(
        &self,
        tree: &Tree,
        layout: Layout<'_>,
        cursor: mouse::Cursor,
        viewport: &Rectangle,
        renderer: &iced::Renderer,
    ) -> mouse::Interaction {
        let child = layout.children().next().unwrap();
        self.content.as_widget().mouse_interaction(&tree.children[0], child, cursor, viewport, renderer)
    }

    fn overlay<'b>(
        &'b mut self,
        tree: &'b mut Tree,
        layout: Layout<'b>,
        renderer: &iced::Renderer,
        viewport: &Rectangle,
        translation: Vector,
    ) -> Option<overlay::Element<'b, Message, iced::Theme, iced::Renderer>> {
        let child = layout.children().next().unwrap();
        self.content.as_widget_mut().overlay(&mut tree.children[0], child, renderer, viewport, translation)
    }
}

impl<'a, Message: 'a> From<Isolated<'a, Message>> for Element<'a, Message> {
    fn from(isolated: Isolated<'a, Message>) -> Self {
        Element::new(isolated)
    }
}
