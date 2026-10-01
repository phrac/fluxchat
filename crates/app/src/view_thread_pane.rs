//! Rendering of the tmux-style pane grid: the focused pane shows the full
//! main view; other panes show a lighter view (messages + their own
//! composer) of a room or thread. Empty panes offer to pick a room.

use iced::widget::{button, column, container, pane_grid, row, scrollable, text};
use iced::{Alignment, Element, Fill, Length, Theme};

use crate::message::Message;
use crate::state::FluxChat;
use crate::theme;
use crate::ui::{self, Tone};
use crate::view::thin_scrollbar;

/// Most recent rows a room shows in a non-focused pane (not virtualized).
const SIDE_PANE_ROWS: usize = 300;
/// Spaces bar width (see view_sidebar).
const SPACES_BAR_WIDTH: f32 = 56.0;
/// Gap between panes (pane_grid spacing).
const PANE_SPACING: f32 = 4.0;
/// Width taken by a title bar's controls, padding and borders: what's left
/// is the most the title may use.
const TITLE_BAR_RESERVED: f32 = 124.0;
/// Rooms offered in an empty pane's picker (the rest via search).
const EMPTY_PANE_ROOMS: usize = 8;
/// Width of the empty-pane room picker.
const EMPTY_PANE_WIDTH: f32 = 280.0;

impl FluxChat {
    /// The active window's panes.
    pub(crate) fn view_pane_grid(&self) -> Element<'_, Message> {
        let window = self.window();
        let multiple = window.grid.len() > 1;
        let maximized = window.grid.maximized().is_some();
        let widths = self.pane_widths();
        pane_grid(&window.grid, move |pane, slot, _is_max| {
            let focused = pane == window.focus;
            let body: Element<Message> = match slot.key.as_deref() {
                // A lone empty pane is the welcome view; in a split it's a picker.
                None if multiple => self.view_empty_pane(pane),
                _ if focused => self.view_content(),
                Some(key) => self.view_pane_body(key),
                None => self.view_empty_pane(pane),
            };
            let framed = container(body)
                .width(Fill)
                .height(Fill)
                .style(move |_: &Theme| container::Style {
                    border: iced::Border {
                        color: if focused && multiple { theme::with_alpha(theme::colors().accent, 0.7) } else { theme::colors().border },
                        width: if multiple { 1.0 } else { 0.0 },
                        radius: 0.0.into(),
                    },
                    ..Default::default()
                });
            let mut content = pane_grid::Content::new(framed);
            if multiple || maximized {
                content = content.title_bar(
                    pane_grid::TitleBar::new(
                        // iced hides a title that doesn't fit beside the
                        // controls (instead of shortening it): cap and clip it.
                        container(self.view_pane_title(slot.key.as_deref(), focused))
                            .max_width((widths.get(&pane).copied().unwrap_or(0.0) - TITLE_BAR_RESERVED).max(40.0))
                            .clip(true),
                    )
                        .controls(pane_grid::Controls::new(self.view_pane_controls(pane, maximized)))
                        .always_show_controls()
                        .padding([4, 8])
                        .style(move |_: &Theme| container::Style {
                            background: Some(if focused {
                                theme::with_alpha(theme::colors().accent, 0.12)
                            } else {
                                theme::colors().bg_dark
                            }.into()),
                            ..Default::default()
                        }),
                );
            }
            content
        })
        .width(Fill)
        .height(Fill)
        .spacing(PANE_SPACING)
        .on_click(Message::PaneClicked)
        .on_drag(Message::PaneDragged)
        .on_resize(8, Message::PaneResized)
        .into()
    }

    /// Width of each pane in the active window: the window minus the panels
    /// around the grid, split as the grid's layout says.
    fn pane_widths(&self) -> std::collections::BTreeMap<pane_grid::Pane, f32> {
        let handle = crate::view::RESIZE_HANDLE_WIDTH;
        let room_open = self.selected_room_id.is_some();
        let mut grid_w = self.window_size.0 - SPACES_BAR_WIDTH;
        if !self.room_list_collapsed {
            grid_w -= self.sidebar_width + handle;
        }
        if self.pins_open && room_open {
            grid_w -= self.pins_width + handle;
        }
        if self.threads_open && room_open {
            grid_w -= self.thread_list_width + handle;
        }
        if self.mentions_open {
            grid_w -= self.mentions_width + handle;
        }
        if self.show_member_list && room_open {
            grid_w -= self.member_list_width + handle;
        }
        let grid_w = grid_w.max(0.0);
        let grid = &self.window().grid;
        if let Some(max) = grid.maximized() {
            return [(max, grid_w)].into_iter().collect();
        }
        grid.layout()
            .pane_regions(PANE_SPACING, 0.0, iced::Size::new(grid_w, self.window_size.1))
            .into_iter()
            .map(|(pane, r)| (pane, r.width))
            .collect()
    }

    /// Pane title: icon/avatar, name, short context. Dragging it moves the pane.
    fn view_pane_title<'a>(&'a self, key: Option<&str>, focused: bool) -> Element<'a, Message> {
        let color = if focused { theme::colors().text_bright } else { theme::colors().text_muted };
        let Some(key) = key else {
            return row![
                ui::icon(crate::icons::SVG_PLUS, 13.0, theme::colors().text_dim),
                text("Empty pane").font(self.sidebar_font()).size(12).color(color),
            ].spacing(8).align_y(Alignment::Center).into();
        };
        let room_id = fluxchat_core::timeline::real_room_id(key);
        let room = self.rooms.iter().find(|r| r.room_id == room_id);
        let room_name = room
            .map(|r| if r.is_direct { r.name.clone() } else { format!("#{}", r.name) })
            .unwrap_or_else(|| room_id.to_string());
        let (lead, label): (Element<Message>, String) = if key.starts_with("thread::") {
            let root = fluxchat_core::timeline::parse_thread_key(key).map(|(_, e)| e).unwrap_or("");
            let preview = self.messages_by_room.get(key)
                .and_then(|m| m.iter().find(|m| m.event_id == root))
                .map(|m| crate::view::ellipsize(m.body.lines().next().unwrap_or(""), 40))
                .unwrap_or_else(|| "Thread".into());
            (ui::icon(crate::icons::SVG_THREAD, 13.0, theme::colors().accent), format!("{preview}  \u{00b7}  {room_name}"))
        } else {
            (self.render_room_avatar(room.and_then(|r| r.avatar_url.as_deref()), &room_name, 16.0), room_name)
        };
        row![
            lead,
            text(label).font(self.sidebar_font()).size(12).color(color)
                .wrapping(iced::widget::text::Wrapping::None),
        ]
        .spacing(8)
        .align_y(Alignment::Center)
        .into()
    }

    /// Split / zoom / close buttons in a pane's title bar.
    fn view_pane_controls(&self, pane: pane_grid::Pane, maximized: bool) -> Element<'_, Message> {
        let ctl = |icon: &'static [u8], tip: &'static str, msg: Message| -> Element<'_, Message> {
            ui::tooltip_at(
                button(ui::icon(icon, 13.0, theme::colors().text_dim))
                    .on_press(msg)
                    .padding(4)
                    .style(ui::button_style(Tone::Ghost)),
                tip,
                iced::widget::tooltip::Position::Bottom,
            )
        };
        row![
            ctl(crate::icons::SVG_SPLIT_V, "Split side by side (Ctrl+B %)",
                Message::Batch(vec![Message::PaneFocus(pane), Message::PaneSplit(pane_grid::Axis::Vertical)])),
            ctl(crate::icons::SVG_SPLIT_H, "Split stacked (Ctrl+B \")",
                Message::Batch(vec![Message::PaneFocus(pane), Message::PaneSplit(pane_grid::Axis::Horizontal)])),
            if maximized {
                ctl(crate::icons::SVG_MINIMIZE, "Restore (Ctrl+B z)", Message::PaneZoom)
            } else {
                ctl(crate::icons::SVG_MAXIMIZE, "Zoom (Ctrl+B z)", Message::Batch(vec![Message::PaneFocus(pane), Message::PaneZoom]))
            },
            ctl(crate::icons::SVG_CLOSE, "Close pane (Ctrl+B x)", Message::PaneClose(Some(pane))),
        ]
        .spacing(2)
        .into()
    }

    /// An empty pane: a short room list that fills *this* pane, plus the
    /// full search (which also targets this pane).
    fn view_empty_pane(&self, pane: pane_grid::Pane) -> Element<'_, Message> {
        let shortcut = if cfg!(target_os = "macos") { "\u{2318}K" } else { "Ctrl+K" };
        let shown: std::collections::HashSet<&str> = self.window().grid.iter()
            .filter_map(|(_, s)| s.key.as_deref())
            .collect();
        let mut list = column![].spacing(2).width(Length::Fixed(EMPTY_PANE_WIDTH));
        for room_id in self.sidebar_room_order().into_iter()
            .filter(|r| !shown.contains(r.as_str()))
            .take(EMPTY_PANE_ROOMS)
        {
            let Some(room) = self.rooms.iter().find(|r| r.room_id == room_id) else { continue };
            let name = if room.is_direct { room.name.clone() } else { format!("#{}", room.name) };
            list = list.push(
                button(
                    row![
                        self.render_room_avatar(room.avatar_url.as_deref(), &room.name, 18.0),
                        text(name).font(self.sidebar_font()).size(13).color(theme::colors().text_normal)
                            .wrapping(iced::widget::text::Wrapping::None),
                    ].spacing(8).align_y(Alignment::Center),
                )
                .on_press(Message::FillPane(pane, room_id.clone()))
                .padding([5, 10])
                .width(Fill)
                .clip(true)
                .style(ui::row_style(false)),
            );
        }
        container(
            column![
                ui::icon(crate::icons::SVG_SPLIT_V, 28.0, theme::colors().text_dim),
                text("Choose a room for this pane").font(self.sidebar_font()).size(15).color(theme::colors().text_bright),
                list,
                ui::btn(format!("Search all rooms  {shortcut}"), Tone::Secondary,
                    Some(Message::Batch(vec![Message::PaneFocus(pane), Message::ToggleQuickSwitcher]))),
            ]
            .spacing(12)
            .align_x(Alignment::Center),
        )
        .width(Fill)
        .height(Fill)
        .center(Fill)
        .style(|_: &Theme| container::Style {
            background: Some(theme::colors().bg_mid.into()),
            ..Default::default()
        })
        .into()
    }

    /// A non-focused pane: its messages and its own composer.
    pub(crate) fn view_pane_body<'a>(&'a self, key: &'a str) -> Element<'a, Message> {
        let is_thread = key.starts_with("thread::");
        let room_id = fluxchat_core::timeline::real_room_id(key);
        let room_name = self.rooms.iter().find(|r| r.room_id == room_id)
            .map(|r| if r.is_direct { r.name.clone() } else { format!("#{}", r.name) })
            .unwrap_or_default();

        let body: Element<'a, Message> = match self.messages_by_room.get(key) {
            None => ui::empty_state(crate::icons::SVG_CLOCK, "Loading\u{2026}", None),
            Some(msgs) => {
                let mut rows = self.visible_rows(key, msgs);
                if rows.len() > SIDE_PANE_ROWS {
                    rows.drain(..rows.len() - SIDE_PANE_ROWS);
                }
                let mut list = column![].spacing(0).padding([4, 6]).width(Fill);
                for (i, msg) in rows.iter().enumerate() {
                    let continuation = self.prefs.group_messages && !self.compact_mode
                        && i > 0 && crate::view::is_continuation(rows[i - 1], msg);
                    // Actions act on this pane's view (it gets focus first).
                    let row = self.view_message(key, msg, continuation)
                        .map(move |m| crate::panes::in_view(key, m));
                    let mut item = column![row].width(Fill);
                    if is_thread && i == 0 && msg.thread_root.is_none() && rows.len() > 1 {
                        item = item.push(container(ui::divider()).padding([6, 0]));
                    }
                    list = list.push(item);
                }
                if is_thread && rows.len() <= 1 {
                    list = list.push(ui::empty_state(crate::icons::SVG_THREAD, "No replies yet", Some("Start the thread below.".into())));
                }
                scrollable(list)
                    .id(Self::pane_scroll_id(key))
                    .anchor_bottom()
                    .height(Fill)
                    .direction(iced::widget::scrollable::Direction::Vertical(
                        iced::widget::scrollable::Scrollbar::new().width(4).scroller_width(4),
                    ))
                    .style(thin_scrollbar)
                    .into()
            }
        };

        let composer: Element<'a, Message> = match self.pane_inputs.get(key) {
            None => column![].into(),
            Some(content) => {
                let ctrl_enter = self.prefs.ctrl_enter_to_send;
                let send_key = key.to_string();
                let input = iced::widget::TextEditor::new(content)
                    .placeholder(if is_thread { "Reply in thread\u{2026}".to_string() } else { format!("Message {room_name}") })
                    .on_action({
                        let k = key.to_string();
                        move |a| Message::PaneInputAction(k.clone(), a)
                    })
                    .key_binding(move |key_press| {
                        use iced::keyboard::Key;
                        use iced::keyboard::key::Named;
                        use iced::widget::text_editor::Binding;
                        if let Some(b) = crate::view_input::tmux_binding(&key_press) {
                            return Some(b);
                        }
                        match key_press.key {
                            Key::Named(Named::Enter) if ctrl_enter
                                && (key_press.modifiers.command() || key_press.modifiers.control()) =>
                                Some(Binding::Custom(Message::PaneSend(send_key.clone()))),
                            Key::Named(Named::Enter) if !ctrl_enter && !key_press.modifiers.shift() =>
                                Some(Binding::Custom(Message::PaneSend(send_key.clone()))),
                            _ => Binding::from_key_press(key_press),
                        }
                    })
                    .font(self.msg_font())
                    .size(14)
                    .height(Length::Shrink)
                    .max_height(120)
                    .padding([8, 4])
                    .style(|_: &Theme, _| iced::widget::text_editor::Style {
                        background: iced::Background::Color(iced::Color::TRANSPARENT),
                        border: iced::Border::default(),
                        placeholder: theme::colors().text_dim,
                        value: theme::colors().text_bright,
                        selection: theme::with_alpha(theme::colors().accent, 0.35),
                    });
                container(
                    container(
                        row![
                            input,
                            ui::icon_only(crate::icons::SVG_FORWARD, "Send", Tone::Ghost,
                                (!content.text().trim().is_empty()).then(|| Message::PaneSend(key.to_string()))),
                        ]
                        .spacing(6)
                        .align_y(Alignment::Center),
                    )
                    .padding([4, 10])
                    .width(Fill)
                    .style(|_: &Theme| container::Style {
                        background: Some(theme::colors().bg_dark.into()),
                        border: iced::Border { color: theme::colors().border, width: 1.0, radius: 10.0.into() },
                        ..Default::default()
                    }),
                )
                .padding([8, 10])
                .into()
            }
        };

        container(column![body, composer].height(Fill))
            .width(Fill)
            .height(Fill)
            .style(|_: &Theme| container::Style {
                background: Some(theme::colors().bg_mid.into()),
                ..Default::default()
            })
            .into()
    }
}
