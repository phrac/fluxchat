//! Shared building blocks for dialogs: header, buttons, inputs, cards, tabs,
//! badges. Every modal is assembled from these so spacing, radii, colours
//! and hover states stay consistent across the app.

use iced::widget::{button, column, container, row, rule, svg, text, text_input, tooltip, Space};
use iced::{Alignment, Border, Color, Element, Fill, Length, Shadow, Theme, Vector};

use crate::message::Message;
use crate::theme;
use crate::view::MONO;

/// Corner radius of buttons, inputs and list rows.
pub const RADIUS: f32 = 6.0;
/// Corner radius of the dialog frame.
pub const DIALOG_RADIUS: f32 = 10.0;
/// Inner padding of a dialog.
pub const DIALOG_PAD: u16 = 20;
/// Vertical gap between dialog sections.
pub const GAP: f32 = 14.0;

/// Visual weight of a button.
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Tone {
    /// The dialog's main action (solid accent).
    Primary,
    /// Neutral action (filled, bordered).
    Secondary,
    /// Destructive action (red tint; solid red on hover).
    Danger,
    /// Positive action (green tint).
    Success,
    /// Low-emphasis action (text only; tinted on hover).
    Ghost,
}

impl Tone {
    /// Foreground colour of the tone at rest — used for icons inside buttons.
    pub fn fg(self) -> Color {
        let c = theme::colors();
        match self {
            Tone::Primary => Color::WHITE,
            Tone::Secondary => c.text_normal,
            Tone::Danger => c.red,
            Tone::Success => c.green,
            Tone::Ghost => c.text_muted,
        }
    }
}

/// Style of the dialog frame itself.
pub fn dialog_style(_: &Theme) -> container::Style {
    let c = theme::colors();
    container::Style {
        background: Some(c.bg_dark.into()),
        border: Border { color: c.border, width: 1.0, radius: DIALOG_RADIUS.into() },
        shadow: Shadow {
            color: Color::from_rgba(0.0, 0.0, 0.0, 0.45),
            offset: Vector::new(0.0, 8.0),
            blur_radius: 28.0,
        },
        ..Default::default()
    }
}

fn mix(a: Color, b: Color, t: f32) -> Color {
    Color::from_rgba(
        a.r + (b.r - a.r) * t,
        a.g + (b.g - a.g) * t,
        a.b + (b.b - a.b) * t,
        a.a + (b.a - a.a) * t,
    )
}

/// Button style for a tone.
pub fn button_style(tone: Tone) -> impl Fn(&Theme, button::Status) -> button::Style {
    move |_, status| {
        let c = theme::colors();
        let hovered = matches!(status, button::Status::Hovered | button::Status::Pressed);
        let disabled = matches!(status, button::Status::Disabled);
        let (bg, fg, border) = match tone {
            Tone::Primary => {
                let bg = if hovered { mix(c.accent, Color::WHITE, 0.12) } else { c.accent };
                (Some(bg), Color::WHITE, Color::TRANSPARENT)
            }
            Tone::Secondary => (
                Some(if hovered { c.bg_hover } else { c.bg_light }),
                c.text_normal,
                c.border,
            ),
            Tone::Danger | Tone::Success => {
                let base = if tone == Tone::Danger { c.red } else { c.green };
                if hovered {
                    (Some(base), Color::WHITE, base)
                } else {
                    (Some(theme::with_alpha(base, 0.12)), base, theme::with_alpha(base, 0.35))
                }
            }
            Tone::Ghost => (
                hovered.then(|| theme::with_alpha(c.text_normal, 0.08)),
                if hovered { c.text_bright } else { c.text_muted },
                Color::TRANSPARENT,
            ),
        };
        let fade = |col: Color| if disabled { theme::with_alpha(col, col.a * 0.45) } else { col };
        button::Style {
            background: bg.map(|b| fade(b).into()),
            text_color: fade(fg),
            border: Border { color: fade(border), width: 1.0, radius: RADIUS.into() },
            ..Default::default()
        }
    }
}

/// A text button. `None` renders it disabled.
pub fn btn<'a>(label: impl Into<String>, tone: Tone, on_press: Option<Message>) -> button::Button<'a, Message> {
    button(
        text(label.into())
            .font(MONO)
            .size(13)
            // A Fill-width button sizes itself, not its child: the label must
            // fill it for align_x to center anything (a shrink button
            // collapses to the label, so Fill is a no-op there).
            .width(iced::Length::Fill)
            .align_x(Alignment::Center),
    )
        .on_press_maybe(on_press)
        .padding([7, 14])
        .style(button_style(tone))
}

/// A button with a leading icon.
pub fn icon_btn<'a>(
    icon_svg: &'static [u8],
    label: impl Into<String>,
    tone: Tone,
    on_press: Option<Message>,
) -> button::Button<'a, Message> {
    let fg = if on_press.is_some() { tone.fg() } else { theme::with_alpha(tone.fg(), 0.45) };
    button(
        row![
            icon(icon_svg, 14.0, fg),
            text(label.into()).font(MONO).size(13),
        ]
        .spacing(7)
        .align_y(Alignment::Center),
    )
    .on_press_maybe(on_press)
    .padding([7, 12])
    .style(button_style(tone))
}

/// A square icon-only button with a hover tooltip.
pub fn icon_only<'a>(icon_svg: &'static [u8], tip: &'a str, tone: Tone, on_press: Option<Message>) -> Element<'a, Message> {
    let b = button(icon(icon_svg, 14.0, tone.fg()))
        .on_press_maybe(on_press)
        .padding(7)
        .style(button_style(tone));
    with_tooltip(b, tip)
}

/// How long the pointer must rest on something before its tooltip shows.
/// Instant tooltips flash while moving between buttons and cover the next
/// target; a short delay keeps them out of the way of clicks.
pub const TOOLTIP_DELAY: std::time::Duration = std::time::Duration::from_millis(600);

/// Wrap an element in the app's standard tooltip, shown above it.
pub fn with_tooltip<'a>(content: impl Into<Element<'a, Message>>, tip: &'a str) -> Element<'a, Message> {
    tooltip_at(content, tip, tooltip::Position::Top)
}

/// The app's standard tooltip on a chosen side. Use `Bottom` for anything
/// near the top edge: a tooltip that doesn't fit is pushed back into the
/// window and ends up covering the button it describes.
pub fn tooltip_at<'a>(
    content: impl Into<Element<'a, Message>>,
    tip: impl text::IntoFragment<'a>,
    position: tooltip::Position,
) -> Element<'a, Message> {
    styled_tooltip(content, text(tip).font(MONO).size(11).color(theme::colors().text_normal), position)
}

/// A delayed, themed tooltip with arbitrary content.
pub fn styled_tooltip<'a>(
    content: impl Into<Element<'a, Message>>,
    tip: impl Into<Element<'a, Message>>,
    position: tooltip::Position,
) -> Element<'a, Message> {
    tooltip(content, tip, position)
        .gap(6)
        .padding(4)
        .delay(TOOLTIP_DELAY)
        .style(|_: &Theme| container::Style {
            background: Some(theme::colors().bg_deep.into()),
            border: Border { color: theme::colors().border, width: 1.0, radius: 4.0.into() },
            ..Default::default()
        })
        .into()
}

/// An SVG icon tinted with `color`.
pub fn icon<'a>(icon_svg: &'static [u8], size: f32, color: Color) -> Element<'a, Message> {
    svg(svg::Handle::from_memory(icon_svg))
        .width(size)
        .height(size)
        .style(move |_, _| svg::Style { color: Some(color) })
        .into()
}

/// The dialog's close (×) button.
pub fn close_button<'a>() -> Element<'a, Message> {
    button(icon(crate::icons::SVG_CLOSE, 16.0, theme::colors().text_dim))
        .on_press(Message::CloseModal)
        .padding(6)
        .style(button_style(Tone::Ghost))
        .into()
}

/// Dialog header: title, optional subtitle, close button.
pub fn header<'a>(title: impl Into<String>, subtitle: Option<String>) -> Element<'a, Message> {
    let mut titles = column![
        text(title.into()).font(MONO).size(17).color(theme::colors().text_bright),
    ]
    .spacing(3);
    if let Some(sub) = subtitle {
        titles = titles.push(
            text(sub).font(MONO).size(12).color(theme::colors().text_dim)
                .wrapping(text::Wrapping::WordOrGlyph),
        );
    }
    row![container(titles).width(Fill), close_button()]
        .spacing(12)
        .align_y(Alignment::Start)
        .into()
}

/// Small uppercase label above a group of controls.
pub fn section_label<'a>(label: impl Into<String>) -> Element<'a, Message> {
    text(label.into().to_uppercase())
        .font(MONO)
        .size(11)
        .color(theme::colors().text_dim)
        .into()
}

/// Muted explanatory text.
pub fn hint<'a>(s: impl Into<String>) -> Element<'a, Message> {
    text(s.into())
        .font(MONO)
        .size(12)
        .color(theme::colors().text_muted)
        .wrapping(text::Wrapping::WordOrGlyph)
        .into()
}

/// Label + control, stacked.
pub fn field<'a>(label: impl Into<String>, control: impl Into<Element<'a, Message>>) -> Element<'a, Message> {
    column![section_label(label), control.into()].spacing(6).into()
}

/// Text input style shared by all dialogs.
pub fn input_style(_: &Theme, status: text_input::Status) -> text_input::Style {
    let c = theme::colors();
    let border = match status {
        text_input::Status::Focused { .. } => c.accent,
        text_input::Status::Hovered => mix(c.border, c.text_dim, 0.4),
        _ => c.border,
    };
    text_input::Style {
        background: c.bg_deep.into(),
        border: Border { color: border, width: 1.0, radius: RADIUS.into() },
        icon: c.text_dim,
        placeholder: c.text_dim,
        value: c.text_normal,
        selection: theme::with_alpha(c.accent, 0.35),
    }
}

/// A styled single-line text input.
pub fn input<'a>(
    placeholder: &str,
    value: &str,
    on_input: impl Fn(String) -> Message + 'a,
) -> text_input::TextInput<'a, Message> {
    text_input(placeholder, value)
        .on_input(on_input)
        .font(MONO)
        .size(13)
        .padding([8, 10])
        .style(input_style)
}

/// Raised surface for grouping content inside a dialog.
pub fn card<'a>(content: impl Into<Element<'a, Message>>) -> container::Container<'a, Message> {
    container(content)
        .padding([12, 14])
        .width(Fill)
        .style(|_: &Theme| container::Style {
            background: Some(theme::colors().bg_mid.into()),
            border: Border { color: theme::colors().border, width: 1.0, radius: 8.0.into() },
            ..Default::default()
        })
}

/// Thin horizontal separator.
pub fn divider<'a>() -> Element<'a, Message> {
    rule::horizontal(1)
        .style(|_: &Theme| rule::Style {
            color: theme::colors().border,
            radius: 0.0.into(),
            fill_mode: rule::FillMode::Full,
            snap: true,
        })
        .into()
}

/// Right-aligned row of dialog actions.
pub fn footer<'a>(actions: Vec<Element<'a, Message>>) -> Element<'a, Message> {
    let mut r = row![Space::new().width(Fill)].spacing(8).align_y(Alignment::Center);
    for a in actions {
        r = r.push(a);
    }
    r.into()
}

/// Underlined tab strip. Each item: (label, message, active).
pub fn tabs<'a>(items: Vec<(String, Message, bool)>) -> Element<'a, Message> {
    let mut r = row![].spacing(4);
    for (label, msg, active) in items {
        let c = theme::colors();
        let fg = if active { c.text_bright } else { c.text_muted };
        let tab = column![
            button(text(label).font(MONO).size(13).color(fg))
                .on_press(msg)
                .padding([6, 12])
                .style(move |_, status| button::Style {
                    background: (matches!(status, button::Status::Hovered) && !active)
                        .then(|| theme::with_alpha(theme::colors().text_normal, 0.06).into()),
                    border: Border { radius: RADIUS.into(), ..Default::default() },
                    ..Default::default()
                }),
            container(Space::new().height(2))
                .width(Fill)
                .style(move |_: &Theme| container::Style {
                    background: active.then(|| theme::colors().accent.into()),
                    border: Border { radius: 1.0.into(), ..Default::default() },
                    ..Default::default()
                }),
        ]
        .width(Length::Shrink);
        r = r.push(tab);
    }
    column![r, divider()].spacing(0).into()
}

/// Small rounded pill label.
pub fn badge<'a>(label: impl Into<String>, color: Color) -> Element<'a, Message> {
    container(text(label.into()).font(MONO).size(11).color(color))
        .padding([2, 8])
        .style(move |_: &Theme| container::Style {
            background: Some(theme::with_alpha(color, 0.14).into()),
            border: Border { color: theme::with_alpha(color, 0.3), width: 1.0, radius: 10.0.into() },
            ..Default::default()
        })
        .into()
}

/// Style for clickable list rows (hover tint; `active` = selected).
pub fn row_style(active: bool) -> impl Fn(&Theme, button::Status) -> button::Style {
    move |_, status| {
        let c = theme::colors();
        let bg = if active {
            Some(theme::with_alpha(c.accent, 0.14))
        } else if matches!(status, button::Status::Hovered) {
            Some(theme::with_alpha(c.text_normal, 0.06))
        } else {
            None
        };
        button::Style {
            background: bg.map(Into::into),
            text_color: c.text_normal,
            border: Border { radius: RADIUS.into(), ..Default::default() },
            ..Default::default()
        }
    }
}

/// Centered placeholder for empty lists.
pub fn empty_state<'a>(icon_svg: &'static [u8], title: impl Into<String>, detail: Option<String>) -> Element<'a, Message> {
    let mut col = column![
        icon(icon_svg, 28.0, theme::colors().text_dim),
        text(title.into()).font(MONO).size(13).color(theme::colors().text_muted),
    ]
    .spacing(8)
    .align_x(Alignment::Center);
    if let Some(d) = detail {
        col = col.push(text(d).font(MONO).size(12).color(theme::colors().text_dim).align_x(Alignment::Center));
    }
    container(col).width(Fill).padding([24, 12]).center_x(Fill).into()
}

/// Status line under a form (success or error).
pub fn status_line<'a>(message: &str) -> Element<'a, Message> {
    let lower = message.to_lowercase();
    let is_error = ["fail", "error", "invalid", "couldn't", "can't", "denied"]
        .iter()
        .any(|w| lower.contains(w));
    let color = if is_error { theme::colors().red } else { theme::colors().green };
    text(message.to_string()).font(MONO).size(12).color(color).into()
}

/// Segmented control: joined buttons, one active. Each item:
/// (label, message — `None` disables it, active).
pub fn segmented<'a>(items: Vec<(String, Option<Message>, bool)>) -> Element<'a, Message> {
    let n = items.len();
    let mut r = row![].spacing(0);
    for (i, (label, msg, active)) in items.into_iter().enumerate() {
        let (first, last) = (i == 0, i + 1 == n);
        let enabled = msg.is_some();
        r = r.push(
            button(text(label).font(MONO).size(12).align_x(Alignment::Center).width(Fill))
                .on_press_maybe(if active { None } else { msg })
                .padding([7, 4])
                .width(Fill)
                .style(move |_, status| {
                    let c = theme::colors();
                    let hovered = matches!(status, button::Status::Hovered);
                    button::Style {
                        background: Some(if active {
                            theme::with_alpha(c.accent, 0.22)
                        } else if hovered {
                            c.bg_hover
                        } else {
                            c.bg_deep
                        }.into()),
                        text_color: if active {
                            c.text_bright
                        } else if enabled {
                            c.text_muted
                        } else {
                            theme::with_alpha(c.text_dim, 0.5)
                        },
                        border: Border {
                            color: if active { c.accent } else { c.border },
                            width: 1.0,
                            radius: iced::border::Radius {
                                top_left: if first { RADIUS } else { 0.0 },
                                bottom_left: if first { RADIUS } else { 0.0 },
                                top_right: if last { RADIUS } else { 0.0 },
                                bottom_right: if last { RADIUS } else { 0.0 },
                            },
                        },
                        ..Default::default()
                    }
                }),
        );
    }
    r.into()
}

/// Toggle switch style shared by settings and dialogs.
pub fn toggle_style(_: &Theme, status: iced::widget::toggler::Status) -> iced::widget::toggler::Style {
    use iced::widget::toggler;
    let c = theme::colors();
    let on = match status {
        toggler::Status::Active { is_toggled }
        | toggler::Status::Hovered { is_toggled }
        | toggler::Status::Disabled { is_toggled } => is_toggled,
    };
    toggler::Style {
        background: if on { c.accent.into() } else { c.bg_light.into() },
        background_border_width: 1.0,
        background_border_color: if on { c.accent } else { c.border },
        foreground: Color::WHITE.into(),
        foreground_border_width: 0.0,
        foreground_border_color: Color::TRANSPARENT,
        text_color: None,
        border_radius: None,
        padding_ratio: 0.2,
    }
}

/// A labelled toggle row: title, description, switch.
pub fn toggle_row<'a>(
    label: impl Into<String>,
    detail: impl Into<String>,
    on: bool,
    on_toggle: impl Fn(bool) -> Message + 'a,
) -> Element<'a, Message> {
    row![
        column![
            text(label.into()).font(MONO).size(13).color(theme::colors().text_normal),
            text(detail.into()).font(MONO).size(11).color(theme::colors().text_dim)
                .wrapping(text::Wrapping::WordOrGlyph),
        ]
        .spacing(2)
        .width(Fill),
        iced::widget::toggler(on).on_toggle(on_toggle).size(18).style(toggle_style),
    ]
    .spacing(12)
    .align_y(Alignment::Center)
    .into()
}

/// Pick list style matching `input_style`.
pub fn pick_list_style(_: &Theme, status: iced::widget::pick_list::Status) -> iced::widget::pick_list::Style {
    let c = theme::colors();
    let border = match status {
        iced::widget::pick_list::Status::Opened { .. } => c.accent,
        iced::widget::pick_list::Status::Hovered => mix(c.border, c.text_dim, 0.4),
        _ => c.border,
    };
    iced::widget::pick_list::Style {
        text_color: c.text_normal,
        placeholder_color: c.text_dim,
        handle_color: c.text_dim,
        background: c.bg_deep.into(),
        border: Border { color: border, width: 1.0, radius: RADIUS.into() },
    }
}
