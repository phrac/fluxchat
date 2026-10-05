//! An indeterminate progress bar: a segment sliding along a thin track.
//!
//! It animates by asking for redraws (`request_redraw_at`) while it's on
//! screen — no Messages, so no view rebuilds, and it stops by itself when
//! it's gone from the view. A frame every `FRAME` keeps the cost down.

use std::sync::OnceLock;
use std::time::Duration;

use iced::advanced::layout;
use iced::advanced::mouse;
use iced::advanced::renderer::{self, Renderer as _};
use iced::advanced::widget::tree::{self, Tree};
use iced::advanced::{Clipboard, Layout, Shell, Widget};
use iced::time::Instant;
use iced::{window, Color, Element, Event, Length, Rectangle, Size};

/// Time between frames (20 fps is plenty for a sliding bar).
const FRAME: Duration = Duration::from_millis(50);
/// One sweep across the track.
const PERIOD_SECS: f32 = 1.6;
/// Segment length, as a share of the track.
const SEGMENT: f32 = 0.3;

/// Shared start, so every bar slides in step.
fn epoch() -> Instant {
    static EPOCH: OnceLock<Instant> = OnceLock::new();
    *EPOCH.get_or_init(Instant::now)
}

pub struct Working {
    height: f32,
    color: Color,
}

#[derive(Default)]
struct State {
    now: Option<Instant>,
}

/// A sliding bar `height` px tall in `color`, as wide as it's given.
pub fn working<'a, Message: 'a>(height: f32, color: Color) -> Element<'a, Message> {
    Element::new(Working { height, color })
}

impl<Message> Widget<Message, iced::Theme, iced::Renderer> for Working {
    fn size(&self) -> Size<Length> {
        Size::new(Length::Fill, Length::Fixed(self.height))
    }

    fn tag(&self) -> tree::Tag {
        tree::Tag::of::<State>()
    }

    fn state(&self) -> tree::State {
        tree::State::new(State::default())
    }

    fn layout(&mut self, _tree: &mut Tree, _renderer: &iced::Renderer, limits: &layout::Limits) -> layout::Node {
        layout::Node::new(limits.resolve(Length::Fill, Length::Fixed(self.height), Size::ZERO))
    }

    fn update(
        &mut self,
        tree: &mut Tree,
        event: &Event,
        _layout: Layout<'_>,
        _cursor: mouse::Cursor,
        _renderer: &iced::Renderer,
        _clipboard: &mut dyn Clipboard,
        shell: &mut Shell<'_, Message>,
        _viewport: &Rectangle,
    ) {
        let state = tree.state.downcast_mut::<State>();
        match event {
            Event::Window(window::Event::RedrawRequested(now)) => {
                state.now = Some(*now);
                shell.request_redraw_at(*now + FRAME);
            }
            // Just appeared: get the first frame going.
            _ if state.now.is_none() => shell.request_redraw(),
            _ => {}
        }
    }

    fn draw(
        &self,
        tree: &Tree,
        renderer: &mut iced::Renderer,
        _theme: &iced::Theme,
        _style: &renderer::Style,
        layout: Layout<'_>,
        _cursor: mouse::Cursor,
        _viewport: &Rectangle,
    ) {
        let b = layout.bounds();
        let radius = (self.height / 2.0).into();
        renderer.fill_quad(
            renderer::Quad { bounds: b, border: iced::Border { radius, ..Default::default() }, ..Default::default() },
            Color { a: self.color.a * 0.18, ..self.color },
        );
        let now = tree.state.downcast_ref::<State>().now.unwrap_or_else(epoch);
        let t = (now.saturating_duration_since(epoch()).as_secs_f32() % PERIOD_SECS) / PERIOD_SECS;
        let seg = b.width * SEGMENT;
        // From fully left of the track to fully right of it, clipped to it.
        let x0 = b.x - seg + t * (b.width + seg);
        let left = x0.max(b.x);
        let right = (x0 + seg).min(b.x + b.width);
        if right > left {
            renderer.fill_quad(
                renderer::Quad {
                    bounds: Rectangle { x: left, width: right - left, ..b },
                    border: iced::Border { radius, ..Default::default() },
                    ..Default::default()
                },
                self.color,
            );
        }
    }
}
