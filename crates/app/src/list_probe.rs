//! Wrapper around a message list's scrollable that reports what iced's
//! scrollable keeps to itself.
//!
//! * **Fit** — `on_scroll` is only called while the content overflows
//!   (`notify_viewport` returns early otherwise), so a list whose rows don't
//!   fill it never says a word: the app can't tell it's short, and "load
//!   older history" (driven by scroll events) never starts. `on_fit` reports
//!   such a list (its bounds and content height), once per change.
//! * **Mount** — widget state lives in the widget tree by position. When the
//!   list lands on a new tree node (its pane changed, the layout around it
//!   changed) the scrollable starts over at the top while the app still
//!   believes it's wherever it was. `on_mount` says so, before the new
//!   scrollable's first report, so the app can put it back.
//! * **Wheel up at the top** — a scroll-up the scrollable couldn't use (it's
//!   at the top, or nothing overflows): `on_wheel_up`, the cue to load more.

use iced::advanced::layout;
use iced::advanced::mouse;
use iced::advanced::overlay;
use iced::advanced::renderer;
use iced::advanced::widget::tree::{self, Tree};
use iced::advanced::widget::Operation;
use iced::advanced::{Clipboard, Layout, Shell, Widget};
use iced::{Element, Event, Length, Rectangle, Size, Vector};

type OnFit<'a, Message> = Box<dyn Fn(Rectangle, f32) -> Message + 'a>;

pub struct ListProbe<'a, Message> {
    /// The scrollable (its first layout child is its content).
    content: Element<'a, Message>,
    /// The view the list shows: a list that fits is reported again when this
    /// changes, even at the same size.
    key: String,
    on_mount: Option<Message>,
    on_fit: Option<OnFit<'a, Message>>,
    on_wheel_up: Option<Message>,
}

#[derive(Default)]
struct State {
    mounted: bool,
    /// The last fit reported: view, list bounds, content height.
    fit: Option<(String, Rectangle, f32)>,
}

impl<'a, Message> ListProbe<'a, Message> {
    pub fn new(key: impl Into<String>, scrollable: impl Into<Element<'a, Message>>) -> Self {
        Self { content: scrollable.into(), key: key.into(), on_mount: None, on_fit: None, on_wheel_up: None }
    }

    /// Sent once when the list's widget state is new.
    pub fn on_mount(mut self, message: Message) -> Self {
        self.on_mount = Some(message);
        self
    }

    /// Sent when the content doesn't overflow the list: (list bounds, content height).
    pub fn on_fit(mut self, f: impl Fn(Rectangle, f32) -> Message + 'a) -> Self {
        self.on_fit = Some(Box::new(f));
        self
    }

    /// Sent for a scroll-up over the list that the scrollable had no use for.
    pub fn on_wheel_up(mut self, message: Message) -> Self {
        self.on_wheel_up = Some(message);
        self
    }
}

/// Whether content of `content_height` leaves a list of `list_height`
/// unfilled — the case iced's scrollable never reports.
pub(crate) fn fits(list_height: f32, content_height: f32) -> bool {
    content_height <= list_height
}

/// A wheel / trackpad movement towards older messages.
fn scrolls_up(delta: &mouse::ScrollDelta) -> bool {
    match delta {
        mouse::ScrollDelta::Lines { y, .. } | mouse::ScrollDelta::Pixels { y, .. } => *y > 0.0,
    }
}

impl<'a, Message: Clone> Widget<Message, iced::Theme, iced::Renderer> for ListProbe<'a, Message> {
    fn size(&self) -> Size<Length> {
        self.content.as_widget().size()
    }

    fn size_hint(&self) -> Size<Length> {
        self.content.as_widget().size_hint()
    }

    fn tag(&self) -> tree::Tag {
        tree::Tag::of::<State>()
    }

    fn state(&self) -> tree::State {
        tree::State::new(State::default())
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
        // Before the scrollable sees anything: its first report must find
        // the app already knowing it's a new one.
        let state = tree.state.downcast_mut::<State>();
        if !state.mounted {
            state.mounted = true;
            if let Some(message) = &self.on_mount {
                shell.publish(message.clone());
            }
        }

        let list = layout.children().next().unwrap();
        let was_captured = shell.is_event_captured();
        self.content.as_widget_mut().update(
            &mut tree.children[0], event, list, cursor, renderer, clipboard, shell, viewport,
        );

        match event {
            Event::Window(iced::window::Event::RedrawRequested(_)) => {
                let Some(on_fit) = &self.on_fit else { return };
                let bounds = list.bounds();
                let content_height = list.children().next().map(|c| c.bounds().height).unwrap_or(0.0);
                let state = tree.state.downcast_mut::<State>();
                if !fits(bounds.height, content_height) {
                    // Overflowing: the scrollable reports from here on.
                    state.fit = None;
                    return;
                }
                let now = (self.key.clone(), bounds, content_height);
                if state.fit.as_ref() != Some(&now) {
                    shell.publish(on_fit(bounds, content_height));
                    state.fit = Some(now);
                }
            }
            Event::Mouse(mouse::Event::WheelScrolled { delta })
                if scrolls_up(delta) && !was_captured && !shell.is_event_captured()
                    && cursor.is_over(list.bounds()) =>
            {
                if let Some(message) = &self.on_wheel_up {
                    shell.publish(message.clone());
                }
            }
            _ => {}
        }
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

impl<'a, Message: Clone + 'a> From<ListProbe<'a, Message>> for Element<'a, Message> {
    fn from(probe: ListProbe<'a, Message>) -> Self {
        Element::new(probe)
    }
}

/// Test access: visit the "has been mounted" flag of every probe in a widget tree.
#[cfg(test)]
pub(crate) fn each_probe(tree: &mut Tree, f: &mut dyn FnMut(&mut bool)) {
    if tree.tag == tree::Tag::of::<State>() {
        f(&mut tree.state.downcast_mut::<State>().mounted);
    }
    for child in &mut tree.children {
        each_probe(child, f);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_list_fits_until_its_content_is_taller() {
        assert!(fits(600.0, 0.0));
        assert!(fits(600.0, 599.5));
        assert!(fits(600.0, 600.0), "exactly full still can't scroll, so iced says nothing");
        assert!(!fits(600.0, 600.5));
    }

    #[test]
    fn only_a_scroll_towards_older_messages_counts() {
        assert!(scrolls_up(&mouse::ScrollDelta::Lines { x: 0.0, y: 1.0 }));
        assert!(scrolls_up(&mouse::ScrollDelta::Pixels { x: 0.0, y: 12.0 }));
        assert!(!scrolls_up(&mouse::ScrollDelta::Lines { x: 0.0, y: -1.0 }));
        assert!(!scrolls_up(&mouse::ScrollDelta::Pixels { x: 5.0, y: 0.0 }));
    }
}
