//! Selectable rich text widget for Iced 0.14.
//!
//! Drop-in replacement for `rich_text` that supports mouse text selection
//! and Ctrl+C / Cmd+C copy. Renders spans with full styling (colors, bold,
//! italic, code blocks, mentions, links) and allows users to select and
//! copy message text.

use iced::advanced::graphics::text::Paragraph as ConcreteParagraph;
use iced::advanced::layout;
use iced::advanced::mouse;
use iced::advanced::renderer;
use iced::advanced::text::{self, Paragraph, Span};
use iced::advanced::widget::tree::{self, Tree};
use iced::advanced::{Clipboard, Layout, Renderer as _, Shell, Widget};
use iced::widget::text::{Alignment, LineHeight, Shaping, Wrapping};
use iced::{Element, Event, Font, Length, Pixels, Point, Rectangle, Size, Vector};

/// Selection state: anchor and cursor as (line, byte_index) in the cosmic_text buffer.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct SelectionAnchor {
    pub line: usize,
    pub index: usize,
}

/// Identifies a specific SelectableRichText widget instance.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct SelectionWidgetId {
    pub event_id: String,
    pub part_index: usize,
}

/// A position within a specific widget.
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct GlobalSelectionAnchor {
    pub widget_id: SelectionWidgetId,
    pub line: usize,
    pub index: usize,
}

/// What portion of a widget's text is selected.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum SelectionRange {
    /// No selection in this widget.
    None,
    /// Partial selection between start and end (line, byte_index).
    Partial { start: (usize, usize), end: (usize, usize) },
    /// Entire widget text is selected.
    Full,
}

impl Default for SelectionRange {
    fn default() -> Self { SelectionRange::None }
}

/// Events emitted by SelectableRichText for parent-managed selection.
pub enum SelectionEvent {
    Started(SelectionWidgetId, usize, usize),
    Moved(SelectionWidgetId, usize, usize),
    Ended,
}

/// Global cross-message selection state, stored in FluxChat.
#[derive(Debug, Clone, Default)]
pub(crate) struct CrossMessageSelection {
    pub anchor: Option<GlobalSelectionAnchor>,
    pub cursor: Option<GlobalSelectionAnchor>,
    pub active: bool,
}

/// A selectable rich text widget.
///
/// Uses concrete Iced types (Font, Renderer, Theme) rather than being fully
/// generic, since this widget relies on accessing the cosmic_text buffer
/// inside the concrete `iced::advanced::graphics::text::Paragraph`.
pub struct SelectableRichText<'a, Link, Message>
where
    Link: Clone + 'static,
{
    spans: Box<dyn AsRef<[Span<'a, Link, Font>]> + 'a>,
    size: Option<Pixels>,
    line_height: LineHeight,
    width: Length,
    height: Length,
    font: Option<Font>,
    wrapping: Wrapping,
    on_link_click: Option<Box<dyn Fn(Link) -> Message + 'a>>,
    /// Widget identity for cross-message selection.
    widget_id: Option<SelectionWidgetId>,
    /// What portion of this widget is selected (driven by parent).
    selection_range: SelectionRange,
    /// Whether a selection drag is globally active.
    selection_active: bool,
    /// Callback for selection events.
    on_selection: Option<Box<dyn Fn(SelectionEvent) -> Message + 'a>>,
}

impl<'a, Link, Message> SelectableRichText<'a, Link, Message>
where
    Link: Clone + 'static,
{
    /// Creates a new [`SelectableRichText`] with the given spans.
    pub fn new(spans: impl AsRef<[Span<'a, Link, Font>]> + 'a) -> Self {
        Self {
            spans: Box::new(spans),
            size: None,
            line_height: LineHeight::default(),
            width: Length::Shrink,
            height: Length::Shrink,
            font: None,
            wrapping: Wrapping::default(),
            on_link_click: None,
            widget_id: None,
            selection_range: SelectionRange::None,
            selection_active: false,
            on_selection: None,
        }
    }

    pub fn size(mut self, size: impl Into<Pixels>) -> Self {
        self.size = Some(size.into());
        self
    }

    #[allow(dead_code)]
    pub fn line_height(mut self, line_height: impl Into<LineHeight>) -> Self {
        self.line_height = line_height.into();
        self
    }

    pub fn font(mut self, font: impl Into<Font>) -> Self {
        self.font = Some(font.into());
        self
    }

    pub fn width(mut self, width: impl Into<Length>) -> Self {
        self.width = width.into();
        self
    }

    #[allow(dead_code)]
    pub fn height(mut self, height: impl Into<Length>) -> Self {
        self.height = height.into();
        self
    }

    pub fn wrapping(mut self, wrapping: Wrapping) -> Self {
        self.wrapping = wrapping;
        self
    }

    pub fn on_link_click(
        mut self,
        on_link_click: impl Fn(Link) -> Message + 'a,
    ) -> Self {
        self.on_link_click = Some(Box::new(on_link_click));
        self
    }

    pub fn widget_id(mut self, id: SelectionWidgetId) -> Self {
        self.widget_id = Some(id);
        self
    }

    pub fn selection_range(mut self, range: SelectionRange) -> Self {
        self.selection_range = range;
        self
    }

    pub fn selection_active(mut self, active: bool) -> Self {
        self.selection_active = active;
        self
    }

    pub fn on_selection(
        mut self,
        on_selection: impl Fn(SelectionEvent) -> Message + 'a,
    ) -> Self {
        self.on_selection = Some(Box::new(on_selection));
        self
    }
}

/// Widget state stored in the tree.
struct State<Link> {
    spans: Vec<Span<'static, Link, Font>>,
    paragraph: ConcreteParagraph,
    /// Whether the mouse button is currently pressed (for link click vs drag detection).
    mouse_pressed: bool,
    /// The span index that was pressed (for link click detection).
    span_pressed: Option<usize>,
    /// Currently hovered link span index.
    hovered_link: Option<usize>,
}

impl<Link, Message> Widget<Message, iced::Theme, iced::Renderer>
    for SelectableRichText<'_, Link, Message>
where
    Link: Clone + 'static,
    Message: Clone + 'static,
{
    fn tag(&self) -> tree::Tag {
        tree::Tag::of::<State<Link>>()
    }

    fn state(&self) -> tree::State {
        tree::State::new(State::<Link> {
            spans: Vec::new(),
            paragraph: ConcreteParagraph::default(),
            mouse_pressed: false,
            span_pressed: None,
            hovered_link: None,
        })
    }

    fn size(&self) -> Size<Length> {
        Size {
            width: self.width,
            height: self.height,
        }
    }

    fn layout(
        &mut self,
        tree: &mut Tree,
        renderer: &iced::Renderer,
        limits: &layout::Limits,
    ) -> layout::Node {
        let state = tree.state.downcast_mut::<State<Link>>();
        let spans = self.spans.as_ref().as_ref();

        layout::sized(limits, self.width, self.height, |limits| {
            let bounds = limits.max();
            let size = self
                .size
                .unwrap_or_else(|| text::Renderer::default_size(renderer));
            let font = self
                .font
                .unwrap_or_else(|| text::Renderer::default_font(renderer));

            let text_with_spans = || iced::advanced::text::Text {
                content: spans,
                bounds,
                size,
                line_height: self.line_height,
                font,
                align_x: Alignment::Default,
                align_y: iced::alignment::Vertical::Top,
                shaping: Shaping::Advanced,
                wrapping: self.wrapping,
            };

            if state.spans != spans {
                state.paragraph =
                    ConcreteParagraph::with_spans(text_with_spans());
                state.spans =
                    spans.iter().cloned().map(Span::to_static).collect();
            } else {
                match state.paragraph.compare(iced::advanced::text::Text {
                    content: (),
                    bounds,
                    size,
                    line_height: self.line_height,
                    font,
                    align_x: Alignment::Default,
                    align_y: iced::alignment::Vertical::Top,
                    shaping: Shaping::Advanced,
                    wrapping: self.wrapping,
                }) {
                    text::Difference::None => {}
                    text::Difference::Bounds => {
                        state.paragraph.resize(bounds);
                    }
                    text::Difference::Shape => {
                        state.paragraph =
                            ConcreteParagraph::with_spans(text_with_spans());
                    }
                }
            }

            state.paragraph.min_bounds()
        })
    }

    fn draw(
        &self,
        tree: &Tree,
        renderer: &mut iced::Renderer,
        theme: &iced::Theme,
        defaults: &renderer::Style,
        layout: Layout<'_>,
        _cursor: mouse::Cursor,
        viewport: &Rectangle,
    ) {
        if !layout.bounds().intersects(viewport) {
            return;
        }

        let state = tree.state.downcast_ref::<State<Link>>();
        let spans = self.spans.as_ref().as_ref();
        let style =
            <iced::Theme as iced::widget::text::Catalog>::style(
                theme,
                &<iced::Theme as iced::widget::text::Catalog>::default(),
            );
        let translation = layout.position() - Point::ORIGIN;

        // Draw selection highlights first (behind text)
        let sel_anchors: Option<(SelectionAnchor, SelectionAnchor)> = match self.selection_range {
            SelectionRange::None => None,
            SelectionRange::Partial { start, end } => {
                Some((
                    SelectionAnchor { line: start.0, index: start.1 },
                    SelectionAnchor { line: end.0, index: end.1 },
                ))
            }
            SelectionRange::Full => select_all_anchors(&state.paragraph),
        };
        if let Some((anchor, cursor)) = sel_anchors {
            let selection_rects = compute_selection_rects(&state.paragraph, anchor, cursor);
            let selection_color = crate::theme::with_alpha(crate::theme::colors().accent, 0.3);
            for rect in selection_rects {
                renderer.fill_quad(
                    renderer::Quad {
                        bounds: rect + translation,
                        ..Default::default()
                    },
                    selection_color,
                );
            }
        }

        // Draw span backgrounds, underlines, strikethroughs (same as Rich)
        for (index, span) in spans.iter().enumerate() {
            let is_hovered_link =
                self.on_link_click.is_some() && Some(index) == state.hovered_link;

            if span.highlight.is_some()
                || span.underline
                || span.strikethrough
                || is_hovered_link
            {
                let regions = state.paragraph.span_bounds(index);

                if let Some(highlight) = span.highlight {
                    for bounds in &regions {
                        let bounds = Rectangle::new(
                            bounds.position()
                                - Vector::new(span.padding.left, span.padding.top),
                            bounds.size()
                                + Size::new(span.padding.x(), span.padding.y()),
                        );

                        renderer.fill_quad(
                            renderer::Quad {
                                bounds: bounds + translation,
                                border: highlight.border,
                                ..Default::default()
                            },
                            highlight.background,
                        );
                    }
                }

                if span.underline || span.strikethrough || is_hovered_link {
                    let size = span
                        .size
                        .or(self.size)
                        .unwrap_or(text::Renderer::default_size(renderer));
                    let line_height = span
                        .line_height
                        .unwrap_or(self.line_height)
                        .to_absolute(size);
                    let color = span
                        .color
                        .or(style.color)
                        .unwrap_or(defaults.text_color);

                    let baseline = translation
                        + Vector::new(0.0, size.0 + (line_height.0 - size.0) / 2.0);

                    if span.underline || is_hovered_link {
                        for bounds in &regions {
                            renderer.fill_quad(
                                renderer::Quad {
                                    bounds: Rectangle::new(
                                        bounds.position() + baseline
                                            - Vector::new(0.0, size.0 * 0.08),
                                        Size::new(bounds.width, 1.0),
                                    ),
                                    ..Default::default()
                                },
                                color,
                            );
                        }
                    }

                    if span.strikethrough {
                        for bounds in &regions {
                            renderer.fill_quad(
                                renderer::Quad {
                                    bounds: Rectangle::new(
                                        bounds.position() + baseline
                                            - Vector::new(0.0, size.0 / 2.0),
                                        Size::new(bounds.width, 1.0),
                                    ),
                                    ..Default::default()
                                },
                                color,
                            );
                        }
                    }
                }
            }
        }

        // Draw the text paragraph
        iced::advanced::widget::text::draw(
            renderer,
            defaults,
            layout.bounds(),
            &state.paragraph,
            style,
            viewport,
        );
    }

    fn update(
        &mut self,
        tree: &mut Tree,
        event: &Event,
        layout: Layout<'_>,
        cursor: mouse::Cursor,
        _renderer: &iced::Renderer,
        _clipboard: &mut dyn Clipboard,
        shell: &mut Shell<'_, Message>,
        _viewport: &Rectangle,
    ) {
        let state = tree.state.downcast_mut::<State<Link>>();
        let bounds = layout.bounds();
        let spans = self.spans.as_ref().as_ref();

        // Update hovered link
        let was_hovered = state.hovered_link.is_some();
        if let Some(position) = cursor.position_in(bounds) {
            state.hovered_link =
                state.paragraph.hit_span(position).and_then(|span| {
                    if spans.get(span)?.link.is_some() {
                        Some(span)
                    } else {
                        None
                    }
                });
        } else {
            state.hovered_link = None;
        }
        if was_hovered != state.hovered_link.is_some() {
            shell.request_redraw();
        }

        match event {
            Event::Mouse(mouse::Event::ButtonPressed(mouse::Button::Left)) => {
                if let Some(position) = cursor.position_in(bounds) {
                    if let Some(hit) = hit_cursor(&state.paragraph, position) {
                        state.mouse_pressed = true;

                        // Track span press for link click detection
                        state.span_pressed = state
                            .paragraph
                            .hit_span(position)
                            .and_then(|span| {
                                if spans.get(span)?.link.is_some() {
                                    Some(span)
                                } else {
                                    None
                                }
                            });

                        // Emit selection started event
                        if let (Some(on_sel), Some(wid)) = (&self.on_selection, &self.widget_id) {
                            shell.publish(on_sel(SelectionEvent::Started(
                                wid.clone(), hit.line, hit.index,
                            )));
                        }

                        shell.request_redraw();
                    }
                }
            }
            Event::Mouse(mouse::Event::CursorMoved { .. }) => {
                if let Some(position) = cursor.position_in(bounds) {
                    // Cursor is within this widget — emit if drag is active
                    if state.mouse_pressed || self.selection_active {
                        if let Some(hit) = hit_cursor(&state.paragraph, position) {
                            if let (Some(on_sel), Some(wid)) = (&self.on_selection, &self.widget_id) {
                                state.span_pressed = None;
                                shell.publish(on_sel(SelectionEvent::Moved(
                                    wid.clone(), hit.line, hit.index,
                                )));
                                shell.request_redraw();
                            }
                        }
                    }
                } else if state.mouse_pressed {
                    // Cursor outside bounds but THIS widget started the press — clamp to edges
                    // (only for the originating widget, not others)
                    if let Some(pos) = cursor.position() {
                        let clamped = Point::new(
                            (pos.x - bounds.x).max(0.0).min(bounds.width),
                            (pos.y - bounds.y).max(0.0).min(bounds.height),
                        );
                        if let Some(hit) = hit_cursor(&state.paragraph, clamped) {
                            if let (Some(on_sel), Some(wid)) = (&self.on_selection, &self.widget_id) {
                                state.span_pressed = None;
                                shell.publish(on_sel(SelectionEvent::Moved(
                                    wid.clone(), hit.line, hit.index,
                                )));
                                shell.request_redraw();
                            }
                        }
                    }
                }
            }
            Event::Mouse(mouse::Event::ButtonReleased(mouse::Button::Left)) => {
                if state.mouse_pressed {
                    state.mouse_pressed = false;

                    // If a link span was pressed and no drag occurred, treat as link click
                    if self.selection_range == SelectionRange::None {
                        if let Some(span_idx) = state.span_pressed.take() {
                            if let Some(on_link_click) = &self.on_link_click {
                                if let Some(link) = spans
                                    .get(span_idx)
                                    .and_then(|s| s.link.clone())
                                {
                                    shell.publish(on_link_click(link));
                                }
                            }
                        }
                    }
                    state.span_pressed = None;

                    // Emit selection ended
                    if let Some(on_sel) = &self.on_selection {
                        shell.publish(on_sel(SelectionEvent::Ended));
                    }
                }
            }
            // Ctrl+C and Ctrl+A now handled at the app level
            _ => {}
        }
    }

    fn mouse_interaction(
        &self,
        tree: &Tree,
        layout: Layout<'_>,
        cursor: mouse::Cursor,
        _viewport: &Rectangle,
        _renderer: &iced::Renderer,
    ) -> mouse::Interaction {
        let state = tree.state.downcast_ref::<State<Link>>();

        if state.hovered_link.is_some() {
            mouse::Interaction::Pointer
        } else if cursor.is_over(layout.bounds()) {
            mouse::Interaction::Text
        } else {
            mouse::Interaction::None
        }
    }
}

/// Hit-test the cosmic_text buffer to get a (line, byte_index) cursor position.
pub(crate) fn hit_cursor(
    paragraph: &ConcreteParagraph,
    position: Point,
) -> Option<SelectionAnchor> {
    let buffer = paragraph.buffer();
    let cursor = buffer.hit(position.x, position.y)?;
    Some(SelectionAnchor {
        line: cursor.line,
        index: cursor.index,
    })
}

/// Compute selection rectangles between two anchors in the cosmic_text buffer.
pub(crate) fn compute_selection_rects(
    paragraph: &ConcreteParagraph,
    anchor: SelectionAnchor,
    cursor: SelectionAnchor,
) -> Vec<Rectangle> {
    let (start, end) = if anchor.line < cursor.line
        || (anchor.line == cursor.line && anchor.index <= cursor.index)
    {
        (anchor, cursor)
    } else {
        (cursor, anchor)
    };

    let buffer = paragraph.buffer();
    let mut rects = Vec::new();

    for run in buffer.layout_runs() {
        let line_idx = run.line_i;
        let line_top = run.line_top;
        let line_height = run.line_height;

        // Skip lines entirely before or after the selection
        if line_idx < start.line || line_idx > end.line {
            continue;
        }

        let line_text_len = buffer
            .lines
            .get(line_idx)
            .map(|l| l.text().len())
            .unwrap_or(0);

        let sel_start_idx = if line_idx == start.line {
            start.index
        } else {
            0
        };
        let sel_end_idx = if line_idx == end.line {
            end.index
        } else {
            line_text_len
        };

        if sel_start_idx >= sel_end_idx {
            continue;
        }

        // Find glyph bounds within the selection range on this visual line
        let mut min_x = f32::MAX;
        let mut max_x = f32::MIN;

        for glyph in run.glyphs.iter() {
            let glyph_start = glyph.start;
            let glyph_end = glyph.end;

            // Check if glyph overlaps with selection
            if glyph_end > sel_start_idx && glyph_start < sel_end_idx {
                min_x = min_x.min(glyph.x);
                max_x = max_x.max(glyph.x + glyph.w);
            }
        }

        if min_x < max_x {
            rects.push(Rectangle {
                x: min_x,
                y: line_top,
                width: max_x - min_x,
                height: line_height,
            });
        }
    }

    rects
}

/// Extract the selected text between two anchors.
pub(crate) fn extract_selected_text(
    paragraph: &ConcreteParagraph,
    anchor: SelectionAnchor,
    cursor: SelectionAnchor,
) -> String {
    let (start, end) = if anchor.line < cursor.line
        || (anchor.line == cursor.line && anchor.index <= cursor.index)
    {
        (anchor, cursor)
    } else {
        (cursor, anchor)
    };

    let buffer = paragraph.buffer();
    let mut result = String::new();

    for (i, line) in buffer.lines.iter().enumerate() {
        if i < start.line {
            continue;
        }
        if i > end.line {
            break;
        }

        let text = line.text();
        let line_start = if i == start.line { start.index } else { 0 };
        let line_end = if i == end.line {
            end.index.min(text.len())
        } else {
            text.len()
        };

        if line_start < line_end && line_start < text.len() {
            // Ensure we don't slice in the middle of a UTF-8 character
            let safe_start = text
                .char_indices()
                .map(|(idx, _)| idx)
                .find(|&idx| idx >= line_start)
                .unwrap_or(text.len());
            let safe_end = text
                .char_indices()
                .map(|(idx, _)| idx)
                .find(|&idx| idx >= line_end)
                .unwrap_or(text.len());

            if safe_start < safe_end {
                result.push_str(&text[safe_start..safe_end]);
            }
        }

        if i < end.line {
            result.push('\n');
        }
    }

    result
}

/// Get anchors representing the entire text (for Ctrl+A).
pub(crate) fn select_all_anchors(
    paragraph: &ConcreteParagraph,
) -> Option<(SelectionAnchor, SelectionAnchor)> {
    let buffer = paragraph.buffer();
    let last_line = buffer.lines.len().checked_sub(1)?;
    let last_len = buffer.lines.get(last_line)?.text().len();

    Some((
        SelectionAnchor { line: 0, index: 0 },
        SelectionAnchor {
            line: last_line,
            index: last_len,
        },
    ))
}

/// Helper function to create a [`SelectableRichText`] widget.
pub fn selectable_rich_text<'a, Link, Message>(
    spans: impl AsRef<[Span<'a, Link, Font>]> + 'a,
) -> SelectableRichText<'a, Link, Message>
where
    Link: Clone + 'static,
{
    SelectableRichText::new(spans)
}

impl<'a, Link, Message> From<SelectableRichText<'a, Link, Message>>
    for Element<'a, Message>
where
    Message: Clone + 'a + 'static,
    Link: Clone + 'a,
{
    fn from(
        text: SelectableRichText<'a, Link, Message>,
    ) -> Element<'a, Message> {
        Element::new(text)
    }
}
