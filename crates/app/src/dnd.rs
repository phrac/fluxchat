//! Drag a room or thread from the lists onto a pane.
//!
//! `DragSource` wraps a list row and remembers a left press on it;
//! once the cursor has moved `DRAG_THRESHOLD` with the button down (the global
//! listener calls `cursor_moved`), a drag is active; `SidebarDragEnd` (the
//! global left-release) ends it. `DropTarget` wraps a
//! pane: while a drag is over it, it shades the zone the drop would fill
//! (an edge → split that side, the middle → show it in this pane) and on
//! release sends `Message::DropOnPane`. Both are transparent wrappers (same
//! tree tag/state as their child) and keep the drag in statics, so moving the
//! cursor redraws the shading without a Message (every Message rebuilds the view).

use std::sync::Mutex;

use iced::advanced::layout;
use iced::advanced::mouse;
use iced::advanced::overlay;
use iced::advanced::renderer::{self, Renderer as _};
use iced::advanced::widget::tree::{self, Tree};
use iced::advanced::widget::Operation;
use iced::advanced::{Clipboard, Layout, Shell, Widget};
use iced::widget::pane_grid;
use iced::{Element, Event, Length, Point, Rectangle, Size, Vector};

use crate::message::Message;

/// Pixels the cursor must travel with the button down before a press on a
/// row becomes a drag (so clicks with a little jitter stay clicks).
const DRAG_THRESHOLD: f32 = 8.0;

/// Fraction of a pane's width / height from each edge that splits on drop;
/// the rest is the middle (show it in this pane).
const EDGE_ZONE: f32 = 0.25;

/// A click this soon after a drag ended is the drag's release landing back
/// on the row it started from, not a click. (The release reaches update()
/// through the global listener, in no fixed order with the row's message.)
const DRAG_END_GRACE: std::time::Duration = std::time::Duration::from_millis(300);

/// Where in a pane a drop lands.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DropZone {
    Center,
    Left,
    Right,
    Top,
    Bottom,
}

#[derive(Default)]
struct Drag {
    /// The pressed row's view key and where the press was.
    source: Option<(String, Point)>,
    active: bool,
    /// The pane and zone under the cursor while active.
    over: Option<(pane_grid::Pane, DropZone)>,
    /// When the last drag ended (see `DRAG_END_GRACE`).
    ended: Option<std::time::Instant>,
}

static DRAG: Mutex<Drag> = Mutex::new(Drag { source: None, active: false, over: None, ended: None });

fn drag() -> std::sync::MutexGuard<'static, Drag> {
    DRAG.lock().unwrap_or_else(|e| e.into_inner())
}

/// The cursor moved (global listener): start the drag once past the threshold.
pub fn cursor_moved(position: Point) {
    let mut d = drag();
    if let Some((_, start)) = &d.source {
        if !d.active && start.distance(position) > DRAG_THRESHOLD {
            d.active = true;
        }
    }
}

/// The left button was released (`SidebarDragEnd`; drop targets have
/// already seen the release by then).
pub fn released() {
    let mut d = drag();
    if d.active {
        d.ended = Some(std::time::Instant::now());
    }
    d.source = None;
    d.active = false;
    d.over = None;
}

/// Whether the click being handled is really the end of a drag.
pub fn was_drag() -> bool {
    let d = drag();
    d.active || d.ended.is_some_and(|t| t.elapsed() < DRAG_END_GRACE)
}

/// The zone of `bounds` that `p` is in.
pub fn zone(bounds: Rectangle, p: Point) -> DropZone {
    let x = ((p.x - bounds.x) / bounds.width.max(1.0)).clamp(0.0, 1.0);
    let y = ((p.y - bounds.y) / bounds.height.max(1.0)).clamp(0.0, 1.0);
    let edges = [(x, DropZone::Left), (1.0 - x, DropZone::Right), (y, DropZone::Top), (1.0 - y, DropZone::Bottom)];
    let (dist, edge) = edges.into_iter().fold((f32::MAX, DropZone::Center), |best, e| if e.0 < best.0 { e } else { best });
    if dist < EDGE_ZONE { edge } else { DropZone::Center }
}

/// The part of `bounds` a drop in `zone` would fill.
fn zone_rect(bounds: Rectangle, zone: DropZone) -> Rectangle {
    let (w, h) = (bounds.width / 2.0, bounds.height / 2.0);
    match zone {
        DropZone::Center => bounds,
        DropZone::Left => Rectangle { width: w, ..bounds },
        DropZone::Right => Rectangle { x: bounds.x + w, width: w, ..bounds },
        DropZone::Top => Rectangle { height: h, ..bounds },
        DropZone::Bottom => Rectangle { y: bounds.y + h, height: h, ..bounds },
    }
}

/// Forward every `Widget` method to `self.content` except the ones listed.
macro_rules! transparent {
    () => {
        fn size(&self) -> Size<Length> {
            self.content.as_widget().size()
        }

        fn size_hint(&self) -> Size<Length> {
            self.content.as_widget().size_hint()
        }

        fn tag(&self) -> tree::Tag {
            self.content.as_widget().tag()
        }

        fn state(&self) -> tree::State {
            self.content.as_widget().state()
        }

        fn children(&self) -> Vec<Tree> {
            self.content.as_widget().children()
        }

        fn diff(&self, tree: &mut Tree) {
            self.content.as_widget().diff(tree)
        }

        fn layout(&mut self, tree: &mut Tree, renderer: &iced::Renderer, limits: &layout::Limits) -> layout::Node {
            self.content.as_widget_mut().layout(tree, renderer, limits)
        }

        fn operate(&mut self, tree: &mut Tree, layout: Layout<'_>, renderer: &iced::Renderer, operation: &mut dyn Operation) {
            self.content.as_widget_mut().operate(tree, layout, renderer, operation)
        }

        fn overlay<'b>(
            &'b mut self,
            tree: &'b mut Tree,
            layout: Layout<'b>,
            renderer: &iced::Renderer,
            viewport: &Rectangle,
            translation: Vector,
        ) -> Option<overlay::Element<'b, Message, iced::Theme, iced::Renderer>> {
            self.content.as_widget_mut().overlay(tree, layout, renderer, viewport, translation)
        }
    };
}

/// A list row that can be dragged onto a pane.
pub struct DragSource<'a> {
    key: String,
    content: Element<'a, Message>,
}

pub fn source<'a>(key: String, content: impl Into<Element<'a, Message>>) -> Element<'a, Message> {
    Element::new(DragSource { key, content: content.into() })
}

impl Widget<Message, iced::Theme, iced::Renderer> for DragSource<'_> {
    transparent!();

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
        self.content.as_widget().draw(tree, renderer, theme, style, layout, cursor, viewport)
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
        // Before the row's button sees (and captures) the press.
        if let Event::Mouse(mouse::Event::ButtonPressed(mouse::Button::Left)) = event {
            if let Some(p) = cursor.position_over(layout.bounds()) {
                let mut d = drag();
                d.source = Some((self.key.clone(), p));
                d.active = false;
                d.over = None;
            }
        }
        self.content.as_widget_mut().update(tree, event, layout, cursor, renderer, clipboard, shell, viewport)
    }

    fn mouse_interaction(
        &self,
        tree: &Tree,
        layout: Layout<'_>,
        cursor: mouse::Cursor,
        viewport: &Rectangle,
        renderer: &iced::Renderer,
    ) -> mouse::Interaction {
        if drag().active {
            return mouse::Interaction::Grabbing;
        }
        self.content.as_widget().mouse_interaction(tree, layout, cursor, viewport, renderer)
    }
}

/// A pane that takes drops.
pub struct DropTarget<'a> {
    pane: pane_grid::Pane,
    content: Element<'a, Message>,
}

pub fn target<'a>(pane: pane_grid::Pane, content: impl Into<Element<'a, Message>>) -> Element<'a, Message> {
    Element::new(DropTarget { pane, content: content.into() })
}

impl Widget<Message, iced::Theme, iced::Renderer> for DropTarget<'_> {
    transparent!();

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
        self.content.as_widget().draw(tree, renderer, theme, style, layout, cursor, viewport);
        let over = {
            let d = drag();
            d.active.then_some(d.over).flatten()
        };
        if let Some((pane, zone)) = over {
            if pane == self.pane {
                let bounds = layout.bounds();
                let accent = crate::theme::colors().accent;
                renderer.with_layer(bounds, |r| {
                    r.fill_quad(
                        renderer::Quad {
                            bounds: zone_rect(bounds, zone).shrink(4.0),
                            border: iced::Border {
                                color: crate::theme::with_alpha(accent, 0.8),
                                width: 2.0,
                                radius: crate::ui::RADIUS.into(),
                            },
                            ..renderer::Quad::default()
                        },
                        crate::theme::with_alpha(accent, 0.18),
                    );
                });
            }
        }
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
        match event {
            Event::Mouse(mouse::Event::CursorMoved { .. }) => {
                let mut d = drag();
                if d.active {
                    let here = cursor.position_over(layout.bounds()).map(|p| (self.pane, zone(layout.bounds(), p)));
                    let mine = d.over.is_some_and(|(p, _)| p == self.pane);
                    // Take over when the cursor is here; let go when it left.
                    if here.is_some() && here != d.over {
                        d.over = here;
                        shell.request_redraw();
                    } else if here.is_none() && mine {
                        d.over = None;
                        shell.request_redraw();
                    }
                }
            }
            Event::Mouse(mouse::Event::ButtonReleased(mouse::Button::Left)) => {
                let d = drag();
                if let (true, Some((key, _)), Some(p)) = (d.active, d.source.as_ref(), cursor.position_over(layout.bounds())) {
                    shell.publish(Message::DropOnPane(key.clone(), self.pane, zone(layout.bounds(), p)));
                }
            }
            _ => {}
        }
        self.content.as_widget_mut().update(tree, event, layout, cursor, renderer, clipboard, shell, viewport)
    }

    fn mouse_interaction(
        &self,
        tree: &Tree,
        layout: Layout<'_>,
        cursor: mouse::Cursor,
        viewport: &Rectangle,
        renderer: &iced::Renderer,
    ) -> mouse::Interaction {
        if drag().active && cursor.is_over(layout.bounds()) {
            return mouse::Interaction::Grabbing;
        }
        self.content.as_widget().mouse_interaction(tree, layout, cursor, viewport, renderer)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn zones_split_at_the_edges_and_fill_in_the_middle() {
        let b = Rectangle { x: 100.0, y: 0.0, width: 400.0, height: 200.0 };
        assert_eq!(zone(b, Point::new(300.0, 100.0)), DropZone::Center);
        assert_eq!(zone(b, Point::new(110.0, 100.0)), DropZone::Left);
        assert_eq!(zone(b, Point::new(490.0, 100.0)), DropZone::Right);
        assert_eq!(zone(b, Point::new(300.0, 10.0)), DropZone::Top);
        assert_eq!(zone(b, Point::new(300.0, 195.0)), DropZone::Bottom);
        // A corner goes to the nearer edge.
        assert_eq!(zone(b, Point::new(105.0, 40.0)), DropZone::Left);
    }
}
