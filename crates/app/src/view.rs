use iced::widget::{
    button, center, column, container, mouse_area, opaque, row, rule, scrollable, stack,
    text, text_input, Space,
};
use iced::{Color, Element, Fill, Font, Length, Subscription, Theme};

/// Fixed ID for the main message input so we can refocus it programmatically.
pub fn input_id() -> iced::widget::Id {
    iced::widget::Id::new("main_input")
}

/// The scrollable body of the open dialog / dialog tab.
pub fn dialog_scroll_id() -> iced::widget::Id {
    iced::widget::Id::new("dialog_scroll")
}

/// Scroll the open dialog's body back to the top (tab switches reuse the
/// same scrollable, which would otherwise keep the previous offset).
pub fn scroll_dialog_to_top<T>() -> iced::Task<T> {
    iced::widget::operation::snap_to(dialog_scroll_id(), iced::widget::scrollable::RelativeOffset::START)
}

/// Fixed ID for the GIF search input.
pub fn gif_search_id() -> iced::widget::Id {
    iced::widget::Id::new("gif_search_input")
}

/// Fixed ID for the emoji search input.
pub fn emoji_search_id() -> iced::widget::Id {
    iced::widget::Id::new("emoji_search_input")
}

/// Fixed ID for the search input.
pub fn search_input_id() -> iced::widget::Id {
    iced::widget::Id::new("search_input")
}

/// Fixed ID for the quick switcher input.
pub fn quick_switcher_id() -> iced::widget::Id {
    iced::widget::Id::new("quick_switcher_input")
}

/// Size of every icon in the spaces bar.
const SPACE_ICON_SIZE: f32 = 40.0;
/// Width of thread hover previews.
const THREAD_PREVIEW_WIDTH: f32 = 360.0;
/// Width of right-click menus.
const CONTEXT_MENU_WIDTH: f32 = 300.0;
/// Grab width of panel dividers.
pub(crate) const RESIZE_HANDLE_WIDTH: f32 = 3.0;
/// Width of the rail shown in place of a hidden member list.
pub(crate) const MEMBER_RAIL_WIDTH: f32 = 40.0;
/// Placeholder rows shown while a view's messages load.
pub(crate) const PLACEHOLDER_ROWS: usize = 8;
/// Pane title bars narrower than this get the core header actions only.
const HEADER_FULL_ACTIONS_WIDTH: f32 = 560.0;
/// Read-receipt avatars shown under a message before "+N".
const READ_RECEIPT_AVATARS: usize = 5;
/// Left offset of a thread entry's icon so it lines up with its parent
/// room's name: unread dot (8) + gap (6) + room icon (18) + gap (6).
const THREAD_ENTRY_OFFSET: f32 = 38.0;
/// Max height of a header-bar panel (threads, pins, files) before it scrolls.
const HUD_PANEL_MAX_HEIGHT: f32 = 300.0;
pub(crate) const EMOJI_GRID_COLUMNS: usize = 9;
pub(crate) const EMOJI_PANEL_HEIGHT: f32 = 300.0;
pub(crate) const EMOJI_CELL_SIZE: f32 = 38.0;
/// Picker width: 9 cells + gaps + padding + scrollbar room.
pub(crate) const EMOJI_PANEL_WIDTH: f32 = 400.0;
pub(crate) const EMOJI_FREQ_COUNT: usize = 24;

/// Fixed ID for the message scrollable so we can snap to bottom.
/// Returns the scrollable ID for the current room.
pub fn messages_scrollable_id() -> iced::widget::Id {
    iced::widget::Id::new("messages_scroll")
}

/// No-op kept for compatibility
pub fn set_scroll_room(_room_id: &str) {}

use crate::icons::*;
use crate::message::Message;
use crate::state::{FluxChat, Screen};
use crate::theme;

/// Thin subtle scrollbar style matching the dark theme.
pub(crate) fn thin_scrollbar(_theme: &Theme, status: iced::widget::scrollable::Status) -> iced::widget::scrollable::Style {
    use iced::widget::scrollable::{Style, Rail, Scroller, AutoScroll};
    let is_hovered = matches!(status, iced::widget::scrollable::Status::Hovered { .. } | iced::widget::scrollable::Status::Dragged { .. });
    let scroller_alpha = if is_hovered { 0.35 } else { 0.1 };
    let rail = Rail {
        background: None,
        border: iced::Border::default(),
        scroller: Scroller {
            background: theme::tint(scroller_alpha).into(),
            border: iced::Border { radius: 2.0.into(), ..Default::default() },
        },
    };
    Style {
        container: container::Style::default(),
        vertical_rail: rail,
        horizontal_rail: rail,
        gap: None,
        auto_scroll: AutoScroll {
            background: Color::TRANSPARENT.into(),
            border: iced::Border::default(),
            shadow: iced::Shadow::default(),
            icon: Color::TRANSPARENT,
        },
    }
}

// Default font constants (used as fallback and for non-customizable UI elements)
pub(crate) const MONO: Font = Font {
    family: iced::font::Family::Name("JetBrainsMono Nerd Font"),
    ..Font::MONOSPACE
};
pub(crate) const MONO_BOLD: Font = Font {
    family: iced::font::Family::Name("JetBrainsMono Nerd Font"),
    weight: iced::font::Weight::Bold,
    ..Font::MONOSPACE
};

/// Leaked font family names for 'static lifetime requirement.
/// We only leak unique names the user has actually selected.
static LEAKED_NAMES: std::sync::Mutex<Vec<&'static str>> = std::sync::Mutex::new(Vec::new());

pub(crate) fn font_by_name(name: &str) -> Font {
    let leaked = leaked_name(name);
    Font {
        family: iced::font::Family::Name(leaked),
        ..Font::MONOSPACE
    }
}

pub(crate) fn font_bold_by_name(name: &str) -> Font {
    let leaked = leaked_name(name);
    Font {
        family: iced::font::Family::Name(leaked),
        weight: iced::font::Weight::Bold,
        ..Font::MONOSPACE
    }
}

fn leaked_name(name: &str) -> &'static str {
    let mut names = LEAKED_NAMES.lock().unwrap_or_else(|e| e.into_inner());
    if let Some(existing) = names.iter().find(|n| **n == name) {
        return existing;
    }
    let leaked: &'static str = Box::leak(name.to_string().into_boxed_str());
    names.push(leaked);
    leaked
}

/// Discover available system fonts.
pub fn discover_system_fonts() -> Vec<String> {
    let mut db = fontdb::Database::new();
    db.load_system_fonts();
    let mut families: std::collections::HashSet<String> = std::collections::HashSet::new();
    for face in db.faces() {
        if let Some((name, _)) = face.families.first() {
            families.insert(name.clone());
        }
    }
    let mut sorted: Vec<String> = families.into_iter().collect();
    sorted.sort_by_key(|a| a.to_lowercase());
    sorted
}

impl FluxChat {
    /// Get the user's chosen message font.
    pub(crate) fn msg_font(&self) -> Font {
        font_by_name(&self.msg_font_family)
    }
    /// Get the user's chosen sidebar font.
    pub(crate) fn sidebar_font(&self) -> Font {
        font_by_name(&self.sidebar_font_family)
    }

    /// Window title, IRC-client style:
    /// `(3) #general › thread · Space — @me:server · 6 members — FluxChat`.
    pub fn title(&self) -> String {
        if self.screen == Screen::Login {
            return "FluxChat".to_string();
        }
        let unread: u64 = self.rooms.iter()
            .filter(|r| !self.read_rooms.contains(&r.room_id))
            .map(|r| r.unread_count)
            .sum();
        let mut parts: Vec<String> = Vec::new();
        if let Some(key) = self.selected_room_id.as_deref() {
            let real = fluxchat_core::timeline::real_room_id(key);
            if let Some(room) = self.rooms.iter().find(|r| r.room_id == real) {
                let mut where_ = if room.is_direct { room.name.clone() } else { format!("#{}", room.name) };
                if key.starts_with("thread::") {
                    where_.push_str(" \u{203a} thread");
                }
                if let Some(space) = &room.space_name {
                    where_.push_str(&format!(" \u{00b7} {space}"));
                }
                parts.push(where_);
            }
        }
        let mut who = self.user_id.clone();
        if let Some(n) = self.selected_room_id.as_ref()
            .and_then(|k| self.members_by_room.get(k))
            .map(|m| m.len())
            .filter(|n| *n > 0)
        {
            who.push_str(&format!(" \u{00b7} {n} members"));
        }
        if !who.is_empty() {
            parts.push(who);
        }
        if self.sync_status.is_some() {
            parts.push("reconnecting\u{2026}".into());
        }
        parts.push("FluxChat".into());
        let title = parts.join(" \u{2014} ");
        if unread > 0 { format!("({unread}) {title}") } else { title }
    }

    /// Top-level view — dispatches to login or main layout.
    pub fn view(&self) -> Element<Message> {
        match self.screen {
            Screen::Login => {
                if self.loading {
                    self.view_loading()
                } else {
                    self.view_login()
                }
            }
            Screen::Main => self.view_main(),
        }
    }

    /// Render stacked read receipt avatars for a message, right-aligned.
    /// Sources per-event receipts directly from the SDK timeline item.
    fn view_read_receipts<'a>(
        &'a self,
        msg: &'a fluxchat_core::MessageInfo,
    ) -> Option<Element<'a, Message>> {
        if !self.prefs.show_read_receipts {
            return None;
        }
        let user_ids: Vec<&str> = msg.read_receipt_user_ids.iter()
            .filter(|uid| uid.as_str() != self.user_id)
            .map(String::as_str)
            .collect();
        if user_ids.is_empty() {
            return None;
        }
        let members = self.selected_room_id.as_ref()
            .and_then(|rid| self.members_by_room.get(rid));
        let name_of = |uid: &str| -> String {
            members.and_then(|m| m.iter().find(|m| m.user_id == uid))
                .map(|m| m.display_name.clone())
                .unwrap_or_else(|| uid.trim_start_matches('@').split(':').next().unwrap_or(uid).to_string())
        };
        let mut avatars_row = row![].spacing(2);
        let shown = user_ids.len().min(READ_RECEIPT_AVATARS);
        for uid in user_ids.iter().take(shown) {
            let avatar_mxc = members
                .and_then(|m| m.iter().find(|m| m.user_id == *uid))
                .and_then(|m| m.avatar_url.as_deref());
            avatars_row = avatars_row.push(self.render_avatar_with_initial(avatar_mxc, &name_of(uid), 16.0));
        }
        if user_ids.len() > shown {
            avatars_row = avatars_row.push(
                text(format!("+{}", user_ids.len() - shown))
                    .font(self.sidebar_font()).size(11).color(theme::colors().text_dim),
            );
        }
        // Hover: who has read up to here.
        let names: Vec<String> = user_ids.iter().map(|u| name_of(u)).collect();
        let tip = if names.len() <= 6 {
            format!("Read by {}", names.join(", "))
        } else {
            format!("Read by {} and {} others", names[..5].join(", "), names.len() - 5)
        };
        Some(
            row![
                Space::new().width(Fill),
                iced::widget::tooltip(
                    avatars_row,
                    text(tip).font(self.sidebar_font()).size(11).color(theme::colors().text_normal),
                    iced::widget::tooltip::Position::Left,
                )
                .delay(crate::ui::TOOLTIP_DELAY)
                .gap(6)
                .padding(4)
                .style(|_: &Theme| container::Style {
                    background: Some(theme::colors().bg_deep.into()),
                    border: iced::Border { color: theme::colors().border, width: 1.0, radius: 4.0.into() },
                    ..Default::default()
                }),
            ]
            .align_y(iced::Alignment::Center)
            .padding([1, 8])
            .into(),
        )
    }

    /// The thread summary under `msg` in room view `view_key`, if it is a
    /// thread root. The SDK's summary count (`thread_count`) covers replies
    /// that aren't loaded; loaded replies are counted too in case it lags.
    pub(crate) fn thread_summary_row<'a>(
        &'a self,
        view_key: &str,
        msg: &'a fluxchat_core::MessageInfo,
    ) -> Option<Element<'a, Message>> {
        if view_key.starts_with("thread::") {
            return None;
        }
        let loaded = self.thread_counts.get(view_key)
            .and_then(|tc| tc.get(msg.event_id.as_str()).copied())
            .unwrap_or(0);
        let count = loaded.max(msg.thread_count.unwrap_or(0));
        (count > 0).then(|| self.view_thread_summary(view_key, msg, count))
    }

    /// The row under a thread root: reply count plus the latest reply's
    /// sender and preview (from the SDK thread summary). Opens the thread.
    fn view_thread_summary<'a>(
        &'a self,
        room_id: &str,
        msg: &'a fluxchat_core::MessageInfo,
        count: u32,
    ) -> Element<'a, Message> {
        let label = if count == 1 { "1 reply".to_string() } else { format!("{count} replies") };
        let handle = iced::widget::svg::Handle::from_memory(crate::icons::SVG_THREAD);
        let mut summary = row![
            iced::widget::svg(handle)
                .width(12).height(12)
                .style(|_, _| iced::widget::svg::Style { color: Some(theme::colors().accent) }),
            text(label).font(self.sidebar_font()).size(12).color(theme::colors().accent),
        ].spacing(6).align_y(iced::Alignment::Center);
        // The agent's run in this thread: working / needs you / done / failed.
        let run = self.run_state(&fluxchat_core::timeline::thread_key(
            fluxchat_core::timeline::real_room_id(room_id), &msg.event_id));
        if let Some(state) = run {
            summary = summary.push(
                container(row![
                    crate::agents::state_dot(state, 6.0),
                    text(state.label()).font(self.sidebar_font()).size(11).color(state.color()),
                ].spacing(4).align_y(iced::Alignment::Center))
                .padding([0, 6])
                .style(move |_: &Theme| container::Style {
                    background: Some(theme::with_alpha(state.color(), 0.12).into()),
                    border: iced::Border { radius: 8.0.into(), ..Default::default() },
                    ..Default::default()
                }),
            );
        }
        if let Some(latest) = &msg.thread_latest {
            let preview: String = latest.body.lines().next().unwrap_or("").chars().take(80).collect();
            summary = summary
                .push(self.render_avatar_with_initial(latest.avatar_url.as_deref(), &latest.sender_name, 16.0))
                .push(text(latest.sender_name.as_str()).font(self.sidebar_font()).size(12)
                    .color(theme::nick_color(&latest.sender_name)))
                .push(text(preview).font(self.sidebar_font()).size(12)
                    .color(theme::colors().text_dim)
                    .wrapping(iced::widget::text::Wrapping::None));
        }
        summary = summary.push(
            text("\u{203A}").font(self.sidebar_font()).size(13).color(theme::colors().text_dim),
        );
        let open = button(summary)
            .on_press(Message::ListClick(
                fluxchat_core::timeline::thread_key(fluxchat_core::timeline::real_room_id(room_id), &msg.event_id),
                Box::new(Message::OpenThread(room_id.to_string(), msg.event_id.clone())),
            ))
            .padding([2, 6])
            .clip(true)
            .style(|_, status| button::Style {
                background: if matches!(status, button::Status::Hovered) {
                    Some(theme::with_alpha(theme::colors().accent, 0.1).into())
                } else {
                    None
                },
                border: iced::Border { radius: 4.0.into(), ..Default::default() },
                ..Default::default()
            });
        row![
            Space::new().width(Length::Fixed(self.msg_content_indent() - 6.0)),
            self.thread_menu_area(open, room_id, &msg.event_id, false),
        ].spacing(0).into()
    }

    /// Render a cached avatar image, or a blank space fallback.
    pub(crate) fn render_avatar<'a>(&self, mxc: Option<&str>, size: f32) -> Element<'a, Message> {
        if let Some(handle) = mxc.and_then(|m| self.avatar_cache.get(m)) {
            container(
                iced::widget::image(handle.clone())
                    .width(size).height(size)
                    .content_fit(iced::ContentFit::Cover)
                    .border_radius(size / 2.0),
            )
            .width(size).height(size)
            .into()
        } else {
            Space::new().width(size).height(size).into()
        }
    }

    /// Render a cached avatar with an initial letter fallback.
    pub(crate) fn render_avatar_with_initial<'a>(&self, mxc: Option<&str>, name: &str, size: f32) -> Element<'a, Message> {
        self.render_avatar_shaped(mxc, name, size, size / 2.0)
    }

    /// Wrap a thread row in a tooltip showing the whole root message and
    /// the latest reply.
    fn thread_hover_preview<'a>(
        &self,
        entry: impl Into<Element<'a, Message>>,
        t: &fluxchat_core::ThreadPreview,
        side: iced::widget::tooltip::Position,
    ) -> Element<'a, Message> {
        let mut tip = column![
            text(t.sender_name.clone()).font(self.sidebar_font()).size(12).color(theme::nick_color(&t.sender_name)),
            text(if t.full_body.is_empty() { t.body.clone() } else { t.full_body.clone() })
                .font(self.sidebar_font()).size(13).color(theme::colors().text_normal)
                .wrapping(iced::widget::text::Wrapping::WordOrGlyph),
        ]
        .spacing(4)
        .width(Length::Fixed(THREAD_PREVIEW_WIDTH));
        if let Some(latest) = &t.latest_body {
            tip = tip.push(crate::ui::divider());
            tip = tip.push(
                text(format!("Latest reply \u{00b7} {}", t.latest_sender_name.as_deref().unwrap_or("")))
                    .font(self.sidebar_font()).size(11).color(theme::colors().text_dim),
            );
            tip = tip.push(
                text(latest.clone()).font(self.sidebar_font()).size(13).color(theme::colors().text_muted)
                    .wrapping(iced::widget::text::Wrapping::WordOrGlyph),
            );
        }
        tip = tip.push(text(thread_meta(t)).font(self.sidebar_font()).size(11).color(theme::colors().text_dim));
        iced::widget::tooltip(entry, tip, side)
            .delay(crate::ui::TOOLTIP_DELAY)
            .gap(8)
            .padding(10)
            .style(crate::ui::dialog_style)
            .into()
    }

    /// Mentions inbox: messages that mentioned us or matched a keyword,
    /// across all rooms, newest first. Click to jump to the message.
    fn view_mentions_sidebar(&self) -> Element<'_, Message> {
        let header = row![
            text(format!("MENTIONS \u{2014} {}", self.mentions.len()))
                .font(self.sidebar_font()).size(12).color(theme::colors().text_dim),
            Space::new().width(Fill),
            button(crate::ui::icon(SVG_CLOSE, 14.0, theme::colors().text_dim))
                .on_press(Message::ToggleMentions)
                .padding(4)
                .style(crate::ui::button_style(crate::ui::Tone::Ghost)),
        ]
        .align_y(iced::Alignment::Center)
        .padding([8, 12]);
        let body: Element<Message> = if self.mentions.is_empty() {
            crate::ui::empty_state(SVG_AT_SIGN, "No mentions yet",
                Some("Messages that mention you or a keyword show up here.".into()))
        } else {
            let now = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_secs()).unwrap_or(0);
            let mut list = column![].spacing(2).padding([0, 6]);
            for m in &self.mentions {
                let room = self.rooms.iter().find(|r| r.room_id == m.room_id);
                let room_name = room.map(|r| if r.is_direct { r.name.clone() } else { format!("#{}", r.name) })
                    .unwrap_or_else(|| m.room_id.clone());
                let ago = now.saturating_sub(m.timestamp);
                let when = match ago {
                    0..=59 => "just now".to_string(),
                    60..=3599 => format!("{}m", ago / 60),
                    3600..=86399 => format!("{}h", ago / 3600),
                    _ => format!("{}d", ago / 86400),
                };
                let current = self.selected_room_id.as_deref() == Some(m.room_id.as_str())
                    && self.highlight_event_id.as_deref() == Some(m.event_id.as_str());
                list = list.push(
                    button(
                        column![
                            row![
                                self.render_room_avatar(room.and_then(|r| r.avatar_url.as_deref()), &room_name, 14.0),
                                text(room_name.clone()).font(self.sidebar_font()).size(11).color(theme::colors().accent)
                                    .wrapping(iced::widget::text::Wrapping::None).width(Fill),
                                text(when).font(self.sidebar_font()).size(11).color(theme::colors().text_dim),
                            ].spacing(6).align_y(iced::Alignment::Center),
                            text(m.sender_name.as_str()).font(self.sidebar_font()).size(12)
                                .color(theme::nick_color(&m.sender_name)),
                            text(m.body.as_str()).font(self.sidebar_font()).size(13).color(theme::colors().text_normal)
                                .wrapping(iced::widget::text::Wrapping::WordOrGlyph),
                        ].spacing(3),
                    )
                    .on_press(Message::OpenMention(m.room_id.clone(), m.event_id.clone()))
                    .padding([7, 8])
                    .width(Fill)
                    .clip(true)
                    .style(crate::ui::row_style(current)),
                );
            }
            scrollable(list)
                .height(Fill)
                .direction(iced::widget::scrollable::Direction::Vertical(
                    iced::widget::scrollable::Scrollbar::new().width(4).scroller_width(4),
                ))
                .style(thin_scrollbar)
                .into()
        };
        container(column![header, body].height(Fill))
            .width(Fill)
            .height(Fill)
            .style(|_: &Theme| container::Style {
                background: Some(theme::colors().bg_deep.into()),
                ..Default::default()
            })
            .into()
    }

    /// Button that brings back a hidden side list (room list / users).
    fn panel_restore_button(
        &self,
        icon: &'static [u8],
        message: Message,
        tip: &'static str,
        side: iced::widget::tooltip::Position,
    ) -> Element<'_, Message> {
        crate::ui::tooltip_at(
            button(crate::ui::icon(icon, 18.0, theme::colors().text_muted))
                .on_press(message)
                .padding(6)
                .style(crate::ui::button_style(crate::ui::Tone::Ghost)),
            tip,
            side,
        )
    }

    /// Pinned messages sidebar: the room's pins (in the room's pin order).
    /// Click to jump; the pin button unpins.
    fn view_pins_sidebar(&self) -> Element<'_, Message> {
        let header = row![
            text(format!("PINNED \u{2014} {}", self.pinned_messages.len()))
                .font(self.sidebar_font()).size(12).color(theme::colors().text_dim),
            Space::new().width(Fill),
            button(crate::ui::icon(SVG_CLOSE, 14.0, theme::colors().text_dim))
                .on_press(Message::TogglePins)
                .padding(4)
                .style(crate::ui::button_style(crate::ui::Tone::Ghost)),
        ]
        .align_y(iced::Alignment::Center)
        .padding([8, 12]);
        let body: Element<Message> = if self.pinned_messages.is_empty() {
            crate::ui::empty_state(SVG_PIN, "No pinned messages",
                Some("Pin a message from its hover bar to keep it here.".into()))
        } else {
            let mut list = column![].spacing(2).padding([0, 6]);
            for msg in &self.pinned_messages {
                let when = format!(
                    "{} {}",
                    format_day(msg.timestamp),
                    crate::view_messages::format_clock(local_time_of_day(msg.timestamp), false, self.prefs.twelve_hour),
                );
                let current = self.highlight_event_id.as_deref() == Some(msg.event_id.as_str());
                let unpin = crate::ui::tooltip_at(
                    button(crate::ui::icon(SVG_PIN, 12.0, theme::colors().text_dim))
                        .on_press(Message::PinMessage(msg.event_id.clone()))
                        .padding(3)
                        .style(crate::ui::button_style(crate::ui::Tone::Ghost)),
                    "Unpin",
                    iced::widget::tooltip::Position::Left,
                );
                list = list.push(
                    button(
                        column![
                            row![
                                self.render_avatar_with_initial(msg.avatar_url.as_deref(), &msg.sender_name, 16.0),
                                text(msg.sender_name.as_str()).font(self.sidebar_font()).size(12)
                                    .color(theme::nick_color(&msg.sender_name))
                                    .wrapping(iced::widget::text::Wrapping::None).width(Fill),
                                unpin,
                            ].spacing(6).align_y(iced::Alignment::Center),
                            text(ellipsize(&msg.body, PIN_PREVIEW_CHARS)).font(self.sidebar_font()).size(13)
                                .color(theme::colors().text_normal)
                                .wrapping(iced::widget::text::Wrapping::WordOrGlyph),
                            text(when).font(self.sidebar_font()).size(11).color(theme::colors().text_dim),
                        ].spacing(4),
                    )
                    .on_press(Message::JumpToEvent(msg.event_id.clone()))
                    .padding([7, 8])
                    .width(Fill)
                    .clip(true)
                    .style(crate::ui::row_style(current)),
                );
            }
            scrollable(list)
                .height(Fill)
                .direction(iced::widget::scrollable::Direction::Vertical(
                    iced::widget::scrollable::Scrollbar::new().width(4).scroller_width(4),
                ))
                .style(thin_scrollbar)
                .into()
        };
        container(column![header, body].height(Fill))
            .width(Fill)
            .height(Fill)
            .style(|_: &Theme| container::Style {
                background: Some(theme::colors().bg_deep.into()),
                ..Default::default()
            })
            .into()
    }

    /// Threads sidebar: the room's threads, most recently active first.
    fn view_threads_sidebar(&self) -> Element<'_, Message> {
        use crate::agents::RunState;
        let room_id = self.active_real_room().unwrap_or("").to_string();
        // With an agent here it's a task board: each thread is a run.
        let board = self.agent_in(&room_id).is_some();
        let run_of = |t: &fluxchat_core::ThreadPreview| self.run_info(&fluxchat_core::timeline::thread_key(&room_id, &t.root_id));
        let live = |t: &&fluxchat_core::ThreadPreview| run_of(t).is_some_and(|r| matches!(r.state, RunState::Working | RunState::NeedsYou));
        let mut threads: Vec<&fluxchat_core::ThreadPreview> = self.thread_list.iter()
            .filter(|t| !(board && self.threads_active_only) || live(t))
            .collect();
        if board {
            // Waiting on you, then working, then the rest by activity.
            threads.sort_by_key(|t| match run_of(t).map(|r| r.state) {
                Some(RunState::NeedsYou) => 0,
                Some(RunState::Working) => 1,
                _ => 2,
            });
        }
        let live_count = self.thread_list.iter().filter(live).count();
        let title = if board {
            format!("RUNS \u{2014} {}", self.thread_list.len())
        } else {
            format!("THREADS \u{2014} {}", self.thread_list.len())
        };
        let header = row![
            text(title).font(self.sidebar_font()).size(12).color(theme::colors().text_dim),
            Space::new().width(Fill),
            button(crate::ui::icon(SVG_CLOSE, 14.0, theme::colors().text_dim))
                .on_press(Message::ToggleThreads)
                .padding(4)
                .style(crate::ui::button_style(crate::ui::Tone::Ghost)),
        ]
        .align_y(iced::Alignment::Center)
        .padding([8, 12]);
        let filter = board.then(|| container(crate::ui::segmented(vec![
            (format!("Active {live_count}"), Some(Message::SetThreadsActiveOnly(true)), self.threads_active_only),
            ("All".to_string(), Some(Message::SetThreadsActiveOnly(false)), !self.threads_active_only),
        ])).padding(iced::Padding { top: 0.0, right: 12.0, bottom: 8.0, left: 12.0 }));

        let body: Element<Message> = if threads.is_empty() {
            if board && self.threads_active_only && !self.thread_list.is_empty() {
                crate::ui::empty_state(crate::icons::SVG_BOT, "Nothing running", Some("Runs show here while the agent works on them or waits for you.".into()))
            } else {
                crate::ui::empty_state(SVG_THREAD, "No threads yet", Some("Reply in thread on any message to start one.".into()))
            }
        } else {
            let mut list = column![].spacing(2).padding([0, 6]);
            for t in threads {
                let open = self.shown_in_pane(&fluxchat_core::timeline::thread_key(&room_id, &t.root_id));
                let entry = button(
                        column![
                            row![
                                crate::ui::icon(SVG_THREAD, 13.0, if open { theme::colors().accent } else { theme::colors().text_dim }),
                                text(t.sender_name.as_str()).font(self.sidebar_font()).size(12)
                                    .color(theme::nick_color(&t.sender_name))
                                    .wrapping(iced::widget::text::Wrapping::None),
                            ].spacing(6).align_y(iced::Alignment::Center),
                            text(t.body.as_str()).font(self.sidebar_font()).size(13).color(theme::colors().text_normal)
                                .wrapping(iced::widget::text::Wrapping::WordOrGlyph),
                        ]
                        // The run: state, how long, what it's on.
                        .push(run_of(t).filter(|_| board).map(|r| self.view_run_line(r, t)))
                        .push(text(thread_meta(t)).font(self.sidebar_font()).size(11).color(theme::colors().text_dim)
                            .wrapping(iced::widget::text::Wrapping::None))
                        .spacing(3),
                    )
                    .on_press(Message::ListClick(
                        fluxchat_core::timeline::thread_key(&room_id, &t.root_id),
                        Box::new(Message::OpenThread(room_id.clone(), t.root_id.clone())),
                    ))
                    .padding([7, 8])
                    .width(Fill)
                    .clip(true)
                    .style(crate::ui::row_style(open));
                let entry = self.thread_menu_area(entry, &room_id, &t.root_id, true);
                list = list.push(self.thread_hover_preview(entry, t, iced::widget::tooltip::Position::Left));
            }
            scrollable(list)
                .height(Fill)
                .direction(iced::widget::scrollable::Direction::Vertical(
                    iced::widget::scrollable::Scrollbar::new().width(4).scroller_width(4),
                ))
                .style(thin_scrollbar)
                .into()
        };
        container(column![header].push(filter).push(body).height(Fill))
            .width(Fill)
            .height(Fill)
            .style(|_: &Theme| container::Style {
                background: Some(theme::colors().bg_deep.into()),
                ..Default::default()
            })
            .into()
    }

    /// Draggable divider between panels (highlights while hovered/dragged).
    fn resize_handle(&self, target: crate::state::ResizeTarget) -> Element<'_, Message> {
        let active = self.resizing == Some(target);
        mouse_area(
            container(Space::new().width(RESIZE_HANDLE_WIDTH).height(Fill))
                .style(move |_: &Theme| container::Style {
                    background: Some(if active { theme::colors().accent } else { theme::colors().border }.into()),
                    ..Default::default()
                }),
        )
        .on_press(Message::ResizeStart(target))
        .interaction(iced::mouse::Interaction::ResizingHorizontally)
        .into()
    }

    /// A panel under the header bar: title + count + close, then a
    /// scrollable list (so long lists don't push the messages away).
    fn hud_panel<'a>(
        &'a self,
        title: &'a str,
        items: Vec<Element<'a, Message>>,
        on_close: Message,
        empty: &'a str,
    ) -> Element<'a, Message> {
        let count = items.len();
        let header = row![
            text(title.to_uppercase()).font(self.sidebar_font()).size(12).color(theme::colors().text_dim),
            text(if count > 0 { count.to_string() } else { String::new() })
                .font(self.sidebar_font()).size(12).color(theme::colors().text_dim),
            Space::new().width(Fill),
            button(crate::ui::icon(SVG_CLOSE, 14.0, theme::colors().text_dim))
                .on_press(on_close)
                .padding(4)
                .style(crate::ui::button_style(crate::ui::Tone::Ghost)),
        ]
        .spacing(8)
        .align_y(iced::Alignment::Center);
        let body: Element<'a, Message> = if items.is_empty() {
            text(empty).font(self.sidebar_font()).size(13).color(theme::colors().text_dim).into()
        } else {
            let mut list = column![].spacing(1);
            for item in items {
                list = list.push(item);
            }
            container(
                scrollable(container(list).padding(iced::Padding { right: 8.0, ..Default::default() }))
                    .direction(iced::widget::scrollable::Direction::Vertical(
                        iced::widget::scrollable::Scrollbar::new().width(4).scroller_width(4),
                    ))
                    .style(thin_scrollbar),
            )
            .max_height(HUD_PANEL_MAX_HEIGHT)
            .into()
        };
        container(column![header, body].spacing(4))
            .padding([8, 12])
            .width(Fill)
            .style(theme::container_panel)
            .into()
    }

    /// Command-palette card (search, quick switcher): icon + borderless
    /// input, divider, results, keyboard hint footer.
    fn palette<'a>(&self, input: iced::widget::TextInput<'a, Message>, body: Element<'a, Message>, hint: &'a str, width: f32) -> Element<'a, Message> {
        let input = input
            .font(self.sidebar_font())
            .size(16)
            .padding([6, 4])
            .style(|_: &Theme, _| iced::widget::text_input::Style {
                background: Color::TRANSPARENT.into(),
                border: iced::Border::default(),
                icon: theme::colors().text_dim,
                placeholder: theme::colors().text_dim,
                value: theme::colors().text_bright,
                selection: theme::with_alpha(theme::colors().accent, 0.35),
            });
        container(
            column![
                row![crate::ui::icon(SVG_SEARCH, 16.0, theme::colors().text_dim), input]
                    .spacing(10)
                    .align_y(iced::Alignment::Center)
                    .padding([8, 14]),
                crate::ui::divider(),
                container(
                    scrollable(container(body).padding(6))
                        .direction(iced::widget::scrollable::Direction::Vertical(
                            iced::widget::scrollable::Scrollbar::new().width(4).scroller_width(4),
                        ))
                        .style(thin_scrollbar),
                )
                .max_height(380.0),
                crate::ui::divider(),
                container(text(hint).font(self.sidebar_font()).size(11).color(theme::colors().text_dim))
                    .padding([6, 14]),
            ],
        )
        .width(Length::Fixed(width))
        .clip(true)
        .style(crate::ui::dialog_style)
        .into()
    }

    /// Dimmed full-window layer holding a palette near the top; clicking
    /// outside sends `on_dismiss`.
    fn palette_layer<'a>(&self, card: Element<'a, Message>, on_dismiss: Message) -> Element<'a, Message> {
        opaque(
            mouse_area(
                container(column![Space::new().height(90), opaque(card)])
                    .width(Fill)
                    .center_x(Fill)
                    .height(Fill)
                    .style(|_: &Theme| container::Style {
                        background: Some(Color::from_rgba(0.0, 0.0, 0.0, 0.45).into()),
                        ..Default::default()
                    }),
            )
            .on_press(on_dismiss),
        )
    }

    /// Rounded-square avatar for rooms and spaces.
    pub(crate) fn render_room_avatar<'a>(&self, mxc: Option<&str>, name: &str, size: f32) -> Element<'a, Message> {
        self.render_avatar_shaped(mxc, name, size, (size * 0.25).round())
    }

    /// Avatar image clipped to `radius`, or the name's initial on a tint of
    /// its nick colour.
    pub(crate) fn render_avatar_shaped<'a>(&self, mxc: Option<&str>, name: &str, size: f32, radius: f32) -> Element<'a, Message> {
        if let Some(handle) = mxc.and_then(|m| self.avatar_cache.get(m)) {
            container(
                iced::widget::image(handle.clone())
                    .width(size).height(size)
                    .content_fit(iced::ContentFit::Cover)
                    .border_radius(radius),
            )
            .width(size).height(size)
            .into()
        } else {
            let trimmed = name.trim_start_matches(['#', '@', '!']);
            let initial = trimmed.chars().next().unwrap_or('?').to_uppercase().to_string();
            let tint = theme::nick_color(name);
            container(
                text(initial).font(self.sidebar_font()).size(size * 0.45).color(tint)
            )
            .width(size).height(size)
            .center_x(size).center_y(size)
            .style(move |_: &Theme| container::Style {
                background: Some(theme::with_alpha(tint, 0.18).into()),
                border: iced::Border { radius: radius.into(), ..Default::default() },
                ..Default::default()
            })
            .into()
        }
    }

    /// Loading screen while restoring session.
    fn view_loading(&self) -> Element<Message> {
        center(
            text("loading...")
                .font(self.sidebar_font())
                .size(16)
                .color(theme::colors().text_muted),
        )
        .style(|_: &Theme| container::Style {
            background: Some(theme::colors().bg_deep.into()),
            ..Default::default()
        })
        .into()
    }

    /// Login screen with SSO + password support.
    fn view_login(&self) -> Element<Message> {
        let title = text("fluxchat")
            .size(22)
            .font(self.sidebar_font())
            .color(theme::colors().text_bright);

        let subtitle = text("Sign in with your Matrix account")
            .size(15)
            .font(self.sidebar_font())
            .color(theme::colors().text_muted);

        let homeserver = text_input("homeserver (e.g. matrix.org)", &self.login_homeserver)
            .on_input(Message::LoginHomeserverChanged)
            .font(self.sidebar_font())
            .size(14)
            .padding(8);

        let mut col = column![title, subtitle, homeserver].spacing(12).width(340);

        // Error display
        if let Some(err) = &self.login_error {
            col = col.push(
                container(text(err.as_str()).size(15).font(self.sidebar_font()).color(theme::colors().red))
                    .padding([8, 12])
                    .style(|_: &Theme| container::Style {
                        background: Some(iced::Color::from_rgba(0.94, 0.27, 0.27, 0.1).into()),
                        border: iced::Border {
                            radius: 4.0.into(),
                            ..Default::default()
                        },
                        ..Default::default()
                    }),
            );
        }

        // SSO buttons
        if let Some(caps) = &self.login_caps {
            if caps.has_sso {
                if caps.sso_providers.is_empty() {
                    let label = if self.login_submitting {
                        "Opening browser..."
                    } else {
                        "Sign in with SSO"
                    };
                    let mut btn = button(text(label).font(self.sidebar_font()).size(14).color(theme::colors().text_bright))
                        .width(Fill)
                        .padding([10, 0]);
                    if !self.login_submitting {
                        btn = btn.on_press(Message::LoginSso);
                    }
                    col = col.push(btn);
                } else {
                    for provider in &caps.sso_providers {
                        let label = if self.login_submitting {
                            "Opening browser...".to_string()
                        } else {
                            format!("Sign in with {}", provider.name)
                        };
                        let mut btn =
                            button(text(label).font(self.sidebar_font()).size(14).color(theme::colors().text_bright))
                                .width(Fill)
                                .padding([10, 0]);
                        if !self.login_submitting {
                            btn = btn.on_press(Message::LoginSso);
                        }
                        col = col.push(btn);
                    }
                }

                col = col.push(
                    text("Opens your browser to authenticate")
                        .size(13)
                        .font(self.sidebar_font())
                        .color(theme::colors().text_dim),
                );

                // Divider
                col = col.push(
                    row![
                        rule::horizontal(1),
                        text("or").size(14).font(self.sidebar_font()).color(theme::colors().text_dim),
                        rule::horizontal(1),
                    ]
                    .spacing(12)
                    .align_y(iced::Alignment::Center),
                );
            }
        }

        // Username + password fields
        let username = text_input("username", &self.login_username)
            .on_input(Message::LoginUsernameChanged)
            .font(self.sidebar_font())
            .size(14)
            .padding(8);

        let password = text_input("password", &self.login_password)
            .on_input(Message::LoginPasswordChanged)
            .on_submit(Message::LoginSubmit)
            .secure(true)
            .font(self.sidebar_font())
            .size(14)
            .padding(8);

        let login_label = if self.login_submitting {
            "Signing in..."
        } else {
            "Sign In"
        };
        let mut login_btn = button(
            text(login_label)
                .font(self.sidebar_font())
                .size(14)
                .color(theme::colors().bg_deep),
        )
        .width(Fill)
        .padding([10, 0]);
        if !self.login_submitting {
            login_btn = login_btn.on_press(Message::LoginSubmit);
        }

        col = col.push(username).push(password).push(login_btn);

        center(
            container(col)
                .padding(32)
                .style(|_: &Theme| container::Style {
                    background: Some(theme::colors().bg_dark.into()),
                    border: iced::Border {
                        color: theme::colors().border,
                        width: 1.0,
                        radius: 8.0.into(),
                    },
                    ..Default::default()
                }),
        )
        .style(|_: &Theme| container::Style {
            background: Some(theme::colors().bg_deep.into()),
            ..Default::default()
        })
        .into()
    }

    /// Main chat layout: sidebar | rooms | messages | status bar.
    fn view_main(&self) -> Element<Message> {
        let sidebar = self.view_sidebar();
        let room_list = self.view_room_list();
        let main_content = self.view_pane_grid();

        use crate::state::ResizeTarget;
        let mut main_area = row![sidebar];
        if !self.room_list_collapsed {
            main_area = main_area
                .push(container(room_list).width(Length::Fixed(self.sidebar_width)))
                .push(self.resize_handle(ResizeTarget::RoomList));
        }
        let mut main_area = main_area.push(main_content);

        // Pinned messages sidebar
        if self.pins_open && self.selected_room_id.is_some() {
            main_area = main_area
                .push(self.resize_handle(ResizeTarget::Pins))
                .push(container(self.view_pins_sidebar()).width(Length::Fixed(self.pins_width)));
        }

        // Threads list sidebar
        if self.threads_open && self.selected_room_id.is_some() {
            main_area = main_area
                .push(self.resize_handle(ResizeTarget::ThreadList))
                .push(container(self.view_threads_sidebar()).width(Length::Fixed(self.thread_list_width)));
        }

        // Mentions inbox sidebar
        if self.mentions_open {
            main_area = main_area
                .push(self.resize_handle(ResizeTarget::Mentions))
                .push(container(self.view_mentions_sidebar()).width(Length::Fixed(self.mentions_width)));
        }

        // Member list (right sidebar), or a slim rail to bring it back
        if self.show_member_list && self.selected_room_id.is_some() {
            main_area = main_area
                .push(self.resize_handle(ResizeTarget::MemberList))
                .push(self.view_member_list());
        } else if self.selected_room_id.is_some() {
            main_area = main_area.push(self.view_member_rail());
        }

        let main_area = main_area.width(Fill).height(Fill);

        let status_bar = self.view_status_bar();

        let base = container(column![main_area, status_bar])
            .width(Fill)
            .height(Fill)
            .style(|_: &Theme| container::Style {
                background: Some(theme::colors().bg_deep.into()),
                ..Default::default()
            });

        let mut layers: Vec<Element<Message>> = vec![base.into()];
        crate::view_gallery::set_lightbox_nav(crate::view_gallery::is_gallery_lightbox(self.modal.as_ref()));
        if self.modal.is_some() {
            layers.push(
                opaque(
                    mouse_area(
                        center(opaque(self.view_modal_content()))
                            .style(|_: &Theme| container::Style {
                                background: Some(
                                    Color::from_rgba(0.0, 0.0, 0.0, 0.6).into(),
                                ),
                                ..Default::default()
                            }),
                    )
                    .on_press(Message::CloseModal),
                ),
            );
        }
        // Search overlay (command-palette style, shared with the switcher)
        if self.search_open {
            let mut results_col = column![].spacing(1);
            for (i, result) in self.search_results.iter().take(20).enumerate() {
                let local_secs = local_time_of_day(result.timestamp);
                let time_str = crate::view_messages::format_clock(local_secs, false, self.prefs.twelve_hour);
                let body_line: String = result.body.lines().next().unwrap_or("").chars().take(120).collect();
                results_col = results_col.push(
                    button(
                        row![
                            text(time_str).font(self.sidebar_font()).size(12).color(theme::colors().text_dim),
                            text(&result.sender_name).font(self.sidebar_font()).size(13)
                                .color(theme::nick_color(&result.sender_name))
                                .wrapping(iced::widget::text::Wrapping::None),
                            text(body_line).font(self.sidebar_font()).size(13).color(theme::colors().text_normal)
                                .wrapping(iced::widget::text::Wrapping::None),
                        ].spacing(10).align_y(iced::Alignment::Center),
                    )
                    .on_press(Message::SearchSelectResult(i))
                    .padding([7, 10])
                    .width(Fill)
                    .style(crate::ui::row_style(self.search_selected == Some(i))),
                );
            }
            let body: Element<Message> = if !self.search_results.is_empty() {
                results_col.into()
            } else if self.search_done {
                crate::ui::empty_state(SVG_SEARCH, "No messages found", Some("Try different words.".into()))
            } else {
                crate::ui::empty_state(SVG_SEARCH, "Search this room",
                    Some("Type and press Enter to search the server.".into()))
            };
            let card = self.palette(
                text_input("Search messages", &self.search_query)
                    .id(search_input_id())
                    .on_input(Message::SearchQueryChanged)
                    .on_submit(Message::SearchSubmit),
                body,
                "\u{2191}\u{2193} select   \u{21b5} search / open   esc close",
                580.0,
            );
            layers.push(self.palette_layer(card, Message::ToggleSearch));
        }

        // Command palette (Ctrl/Cmd+K): rooms and actions
        if self.quick_switcher_open {
            let items = self.palette_items();
            let mut results_col = column![].spacing(1);
            let mut shown_actions_label = false;
            for (i, item) in items.iter().enumerate() {
                let selected = i == self.quick_switcher_selected;
                let activate = Message::PaletteActivate(Box::new(item.clone()));
                match item {
                    crate::state::PaletteItem::Room(room_id) => {
                        let Some(room) = self.rooms.iter().find(|r| r.room_id == *room_id) else { continue };
                        let detail = room.space_name.clone()
                            .unwrap_or_else(|| if room.is_direct { "Direct message".into() } else { String::new() });
                        let mut label = row![
                            text(&room.name).font(self.sidebar_font()).size(14).color(theme::colors().text_bright)
                                .wrapping(iced::widget::text::Wrapping::None),
                            text(detail).font(self.sidebar_font()).size(12).color(theme::colors().text_dim)
                                .wrapping(iced::widget::text::Wrapping::None),
                        ]
                        .spacing(10)
                        .align_y(iced::Alignment::Center);
                        if room.highlight_count > 0 || room.unread_count > 0 {
                            let (n, color) = if room.highlight_count > 0 {
                                (room.highlight_count, theme::colors().red)
                            } else {
                                (room.unread_count, theme::colors().accent)
                            };
                            label = label.push(Space::new().width(Fill)).push(crate::ui::badge(n.to_string(), color));
                        }
                        results_col = results_col.push(
                            button(
                                row![
                                    self.render_room_avatar(room.avatar_url.as_deref(), &room.name, 22.0),
                                    label,
                                ].spacing(10).align_y(iced::Alignment::Center),
                            )
                            .on_press(activate)
                            .padding([6, 10])
                            .width(Fill)
                            .style(crate::ui::row_style(selected)),
                        );
                    }
                    crate::state::PaletteItem::Prompt(prompt, snippet) => {
                        let one_line = crate::view::ellipsize(&prompt.replace('\n', " \u{21b5} "), 90);
                        results_col = results_col.push(
                            button(
                                row![
                                    container(crate::ui::icon(if snippet.is_some() { crate::icons::SVG_STAR } else { crate::icons::SVG_CLOCK }, 13.0, theme::colors().text_dim))
                                        .width(22).center_x(22),
                                ]
                                .push(snippet.as_ref().map(|n| text(n.clone()).font(self.sidebar_font()).size(13).color(theme::colors().accent)
                                    .wrapping(iced::widget::text::Wrapping::None)))
                                .push(text(one_line).font(self.sidebar_font()).size(13).color(theme::colors().text_normal)
                                    .wrapping(iced::widget::text::Wrapping::None))
                                .spacing(10).align_y(iced::Alignment::Center),
                            )
                            .on_press(activate)
                            .padding([6, 10])
                            .width(Fill)
                            .clip(true)
                            .style(crate::ui::row_style(selected)),
                        );
                    }
                    crate::state::PaletteItem::Action(action) => {
                        if !shown_actions_label {
                            shown_actions_label = true;
                            results_col = results_col.push(container(
                                text("ACTIONS").font(self.sidebar_font()).size(11).color(theme::colors().text_dim),
                            ).padding(iced::Padding { top: 8.0, right: 10.0, bottom: 2.0, left: 10.0 }));
                        }
                        results_col = results_col.push(
                            button(
                                row![
                                    container(crate::ui::icon(SVG_CHEVRON_RIGHT, 14.0, theme::colors().accent))
                                        .width(22).center_x(22),
                                    text(action.label.clone()).font(self.sidebar_font()).size(14).color(theme::colors().text_bright)
                                        .wrapping(iced::widget::text::Wrapping::None),
                                    text(action.hint).font(self.sidebar_font()).size(12).color(theme::colors().text_dim)
                                        .wrapping(iced::widget::text::Wrapping::None),
                                ].spacing(10).align_y(iced::Alignment::Center),
                            )
                            .on_press(activate)
                            .padding([6, 10])
                            .width(Fill)
                            .clip(true)
                            .style(crate::ui::row_style(selected)),
                        );
                    }
                }
            }
            let body: Element<Message> = if items.is_empty() {
                let hint = if self.quick_switcher_query.trim_start().starts_with('<') {
                    "Your sent messages and snippets show here. Save one with /snippet <name> <text>."
                } else {
                    "Type > to search actions only, < for your prompts."
                };
                crate::ui::empty_state(SVG_SEARCH, "Nothing matches", Some(hint.into()))
            } else {
                results_col.into()
            };
            let card = self.palette(
                text_input("Jump to a room \u{00b7} > actions \u{00b7} < your prompts (Ctrl+R)", &self.quick_switcher_query)
                    .id(quick_switcher_id())
                    .on_input(Message::QuickSwitcherQuery)
                    .on_submit(Message::QuickSwitcherSelect),
                body,
                "\u{2191}\u{2193} select   \u{21b5} open   > actions   esc close",
                560.0,
            );
            layers.push(self.palette_layer(card, Message::ToggleQuickSwitcher));
        }
        // Context menu overlay
        if let Some(ref ctx) = self.context_menu {
            let menu_items = self.view_context_menu(ctx);
            layers.push(
                opaque(
                    mouse_area(
                        container(
                            container(menu_items)
                                .padding(4)
                                .style(crate::ui::dialog_style),
                        )
                        .padding(iced::Padding { top: self.context_menu_position.1, left: self.context_menu_position.0, ..Default::default() })
                        .width(Fill)
                        .height(Fill),
                    )
                    .on_press(Message::CloseContextMenu),
                ),
            );
        }

        stack(layers).into()
    }

    /// Context menu items for rooms or spaces.
    fn view_context_menu(&self, ctx: &crate::state::ContextMenu) -> Element<'_, Message> {
        let mut col = column![].spacing(1).width(CONTEXT_MENU_WIDTH);

        match ctx {
            crate::state::ContextMenu::Room { room_id, is_dm } => {
                let rid = room_id.clone();
                // Recent threads (most recently active first), then "All threads".
                if let Some((menu_room, threads)) = &self.context_threads {
                    if menu_room == room_id {
                        match threads {
                            None => {
                                col = col.push(container(
                                    text("Loading threads\u{2026}").font(self.sidebar_font()).size(12).color(theme::colors().text_dim),
                                ).padding([6, 10]));
                                col = col.push(self.context_menu_divider());
                            }
                            Some(list) if !list.is_empty() => {
                                col = col.push(container(
                                    text("RECENT THREADS").font(self.sidebar_font()).size(11).color(theme::colors().text_dim),
                                ).padding(iced::Padding { top: 4.0, right: 10.0, bottom: 2.0, left: 10.0 }));
                                // Fit text to the menu: ~0.62em per character, minus
                                // padding and the icon column.
                                let fit = |size: f32, inset: f32| ((CONTEXT_MENU_WIDTH - inset) / (size * 0.62)) as usize;
                                for t in list.iter().take(3) {
                                    let entry = button(
                                            column![
                                                row![
                                                    crate::ui::icon(SVG_THREAD, 12.0, theme::colors().accent),
                                                    text(ellipsize(&crate::view_thread_pane::preview_title(t), fit(13.0, 46.0))).font(self.sidebar_font()).size(13)
                                                        .color(theme::colors().text_normal)
                                                        .wrapping(iced::widget::text::Wrapping::None),
                                                ].spacing(8).align_y(iced::Alignment::Center),
                                                row![
                                                    Space::new().width(20),
                                                    text(ellipsize(&thread_meta_short(t), fit(11.0, 46.0))).font(self.sidebar_font()).size(11)
                                                        .color(theme::colors().text_dim)
                                                        .wrapping(iced::widget::text::Wrapping::None),
                                                ],
                                            ].spacing(2),
                                        )
                                        .on_press(Message::Batch(vec![
                                            Message::CloseContextMenu,
                                            Message::OpenThread(room_id.clone(), t.root_id.clone()),
                                        ]))
                                        .padding([5, 10])
                                        .width(Fill)
                                        .clip(true)
                                        .style(crate::ui::row_style(false));
                                    col = col.push(self.thread_hover_preview(entry, t, iced::widget::tooltip::Position::Right));
                                }
                                col = col.push(
                                    button(
                                        row![
                                            crate::ui::icon(SVG_CHEVRON_RIGHT, 14.0, theme::colors().text_dim),
                                            text("All threads").font(self.sidebar_font()).size(13).color(theme::colors().accent),
                                        ].spacing(8).align_y(iced::Alignment::Center),
                                    )
                                    .on_press(Message::ShowRoomThreads(room_id.clone()))
                                    .padding([6, 10])
                                    .width(Fill)
                                    .style(crate::ui::row_style(false)),
                                );
                                col = col.push(self.context_menu_divider());
                            }
                            Some(_) => {} // no threads: nothing to show
                        }
                    }
                }
                let is_fav = self.rooms.iter()
                    .find(|r| r.room_id == *room_id)
                    .map(|r| r.is_favourite)
                    .unwrap_or(false);
                let fav_label = if is_fav { "Unfavourite" } else { "Favourite" };

                col = col.push(self.context_menu_item(
                    "Mark as Read", SVG_CHECK, rid.clone(), "mark_read",
                ));
                col = col.push(self.context_menu_item(
                    fav_label, SVG_STAR, rid.clone(), "favourite",
                ));
                col = col.push(self.context_menu_item(
                    "Copy Link", SVG_LINK, rid.clone(), "copy_link",
                ));
                {
                    use crate::panes::Placement;
                    let at = |p: Placement| Message::OpenAt(rid.clone(), p);
                    if self.locate(&rid).is_some() {
                        if self.selected_room_id.as_deref() != Some(rid.as_str()) {
                            col = col.push(self.context_menu_entry("Go to room", SVG_HASH, at(Placement::Auto)));
                        }
                    } else {
                        col = col.push(self.context_menu_entry("Open to the right", SVG_SPLIT_V, at(Placement::Right)));
                        col = col.push(self.context_menu_entry("Open below", SVG_SPLIT_H, at(Placement::Below)));
                        col = col.push(self.context_menu_entry("Open in new window", SVG_LAYERS, at(Placement::NewWindow)));
                    }
                }
                if !is_dm {
                    col = col.push(self.context_menu_item(
                        "Invite", SVG_USER_PLUS, rid.clone(), "invite",
                    ));
                    col = col.push(self.context_menu_item(
                        "Room Settings", SVG_SETTINGS, rid.clone(), "settings",
                    ));
                }
                col = col.push(self.context_menu_divider());
                col = col.push(self.context_menu_item_danger(
                    "Leave", SVG_CLOSE, rid, "leave",
                ));
            }
            crate::state::ContextMenu::Message { event_id } => {
                let eid = event_id.clone();
                let is_local = event_id.starts_with('~');
                if !is_local {
                    col = col.push(self.context_menu_item(
                        "Copy Message Link", SVG_LINK, eid.clone(), "msg_copy_link",
                    ));
                    col = col.push(self.context_menu_item(
                        "Copy Event ID", SVG_CLIPBOARD, eid.clone(), "msg_copy_id",
                    ));
                    col = col.push(self.context_menu_item(
                        "View Source", SVG_INFO, eid, "msg_source",
                    ));
                } else {
                    col = col.push(text("Not sent yet").font(self.sidebar_font()).size(12).color(theme::colors().text_dim));
                }
            }
            crate::state::ContextMenu::Thread { room_id, root_id } => {
                use crate::panes::Placement;
                let key = fluxchat_core::timeline::thread_key(room_id, root_id);
                let at = |p: Placement| Message::OpenAt(key.clone(), p);
                // Already open: it would only be focused, wherever asked.
                if self.locate(&key).is_some() {
                    col = col.push(self.context_menu_entry("Go to thread", SVG_THREAD, at(Placement::Auto)));
                } else {
                    col = col.push(self.context_menu_entry("Open thread", SVG_THREAD, at(Placement::Auto)));
                    col = col.push(self.context_menu_entry("Open to the right", SVG_SPLIT_V, at(Placement::Right)));
                    col = col.push(self.context_menu_entry("Open below", SVG_SPLIT_H, at(Placement::Below)));
                    col = col.push(self.context_menu_entry("Open in this pane", SVG_MAXIMIZE, at(Placement::Here)));
                    col = col.push(self.context_menu_entry("Open in new window", SVG_LAYERS, at(Placement::NewWindow)));
                }
            }
            crate::state::ContextMenu::Agent { key } => {
                for (label, icon, msg) in self.agent_menu_items(key) {
                    col = col.push(self.context_menu_entry(label, icon, Message::Batch(vec![Message::CloseContextMenu, msg])));
                }
            }
            crate::state::ContextMenu::Space { space_id } => {
                let sid = space_id.clone();
                col = col.push(self.context_menu_item(
                    "Manage Space", SVG_SETTINGS, sid.clone(), "space_details",
                ));
                col = col.push(self.context_menu_item(
                    "Invite to Space", SVG_USER_PLUS, sid.clone(), "space_invite",
                ));
                col = col.push(self.context_menu_divider());
                col = col.push(self.context_menu_item_danger(
                    "Leave Space", SVG_CLOSE, sid, "leave",
                ));
            }
        }

        col.into()
    }

    fn context_menu_item<'a>(&self, label: &'a str, icon: &'static [u8], target: String, action: &str) -> Element<'a, Message> {
        self.context_menu_entry(label, icon, Message::ContextMenuAction(action.to_string(), target))
    }

    fn context_menu_entry<'a>(&self, label: impl Into<String>, icon: &'static [u8], on_press: Message) -> Element<'a, Message> {
        let label: String = label.into();
        button(
            row![
                crate::ui::icon(icon, 14.0, theme::colors().text_dim),
                text(label).font(self.sidebar_font()).size(13).color(theme::colors().text_normal),
            ].spacing(8).align_y(iced::Alignment::Center),
        )
        .on_press(on_press)
        .padding([6, 10])
        .width(Fill)
        .style(crate::ui::row_style(false))
        .into()
    }

    /// Right-click on a thread row opens the thread menu (where to open it);
    /// middle-click opens it beside; list rows (`draggable`) drag onto panes.
    fn thread_menu_area<'a>(&self, row: impl Into<Element<'a, Message>>, room_id: &str, root_id: &str, draggable: bool) -> Element<'a, Message> {
        let key = fluxchat_core::timeline::thread_key(room_id, root_id);
        self.list_entry_area(row, key, crate::state::ContextMenu::Thread {
            room_id: room_id.to_string(),
            root_id: root_id.to_string(),
        }, draggable)
    }

    /// A room / thread row: right-click opens `menu`, middle-click opens
    /// `key` beside, and a `draggable` one can be dragged onto a pane
    /// (not rows inside panes: a press there is also a pane click).
    fn list_entry_area<'a>(
        &self,
        row: impl Into<Element<'a, Message>>,
        key: String,
        menu: crate::state::ContextMenu,
        draggable: bool,
    ) -> Element<'a, Message> {
        let area = iced::widget::mouse_area(row)
            .on_right_press(Message::OpenContextMenu(menu))
            .on_middle_press(Message::OpenAt(key.clone(), crate::panes::Placement::Right));
        if draggable { crate::dnd::source(key, area) } else { area.into() }
    }

    fn context_menu_item_danger<'a>(&self, label: &'a str, icon: &'static [u8], target: String, action: &str) -> Element<'a, Message> {
        let action = action.to_string();
        let handle = iced::widget::svg::Handle::from_memory(icon);
        button(
            row![
                iced::widget::svg(handle)
                    .width(14).height(14)
                    .style(|_, _| iced::widget::svg::Style { color: Some(theme::colors().red) }),
                text(label).font(self.sidebar_font()).size(13).color(theme::colors().red),
            ].spacing(8).align_y(iced::Alignment::Center),
        )
        .on_press(Message::ContextMenuAction(action, target))
        .padding([6, 10])
        .width(Fill)
        .style(|_, status| button::Style {
            background: matches!(status, button::Status::Hovered)
                .then(|| theme::with_alpha(theme::colors().red, 0.12).into()),
            border: iced::Border { radius: crate::ui::RADIUS.into(), ..Default::default() },
            ..Default::default()
        })
        .into()
    }

    fn context_menu_divider(&self) -> Element<'_, Message> {
        container(crate::ui::divider()).padding([4, 6]).into()
    }

    /// Left space sidebar (icons).
    fn view_sidebar(&self) -> Element<Message> {
        let is_home = self.selected_space_id.is_none();
        // Home button with SVG house icon
        let home_radius = if is_home { 12.0 } else { 20.0 };
        let home_bg_color = if is_home { theme::colors().accent } else { theme::colors().bg_mid };
        let home_handle = iced::widget::svg::Handle::from_memory(
            br#"<svg xmlns="http://www.w3.org/2000/svg" width="24" height="24" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><path d="m3 9 9-7 9 7v11a2 2 0 0 1-2 2H5a2 2 0 0 1-2-2z"/><polyline points="9 22 9 12 15 12 15 22"/></svg>"#.as_slice(),
        );
        let home_btn = button(
            iced::widget::svg(home_handle)
                .width(22).height(22)
                .style(|_, _| iced::widget::svg::Style { color: Some(theme::colors().text_bright) }),
        )
        .on_press(Message::SelectSpace(None))
        .padding(9)
        .style(move |_, _| button::Style {
            background: Some(home_bg_color.into()),
            border: iced::Border { radius: home_radius.into(), ..Default::default() },
            ..Default::default()
        });

        // Same outer size as the space icons (which reserve a 2px ring).
        let home_btn = container(home_btn).padding(2);
        let mut col = column![home_btn].spacing(4).padding([8, 4]).align_x(iced::Alignment::Center);

        // Separator
        col = col.push(
            container(Space::new())
                .width(24).height(2)
                .style(|_: &Theme| container::Style {
                    background: Some(theme::colors().border.into()),
                    border: iced::Border { radius: 1.0.into(), ..Default::default() },
                    ..Default::default()
                })
        );

        for node in &self.space_tree {
            let space = &node.info;
            let space_id = space.room_id.clone();
            let is_selected = self.selected_space_id.as_deref() == Some(&space.room_id);
            let radius = if is_selected { 12.0 } else { 20.0 };
            let opacity: f32 = if is_selected { 1.0 } else { 0.7 };

            // Every space icon is the same square; images and fallbacks alike.
            let icon_size = SPACE_ICON_SIZE;
            let tint = theme::nick_color(&space.name);
            let icon_content: Element<Message> = if let Some(handle) = space.avatar_url.as_ref().and_then(|mxc| self.avatar_cache.get(mxc)) {
                container(
                    iced::widget::image(handle.clone())
                        .width(icon_size).height(icon_size)
                        .content_fit(iced::ContentFit::Cover)
                        .border_radius(radius),
                )
                .width(icon_size).height(icon_size)
                .into()
            } else {
                let initials: String = space.name.split_whitespace()
                    .take(2)
                    .filter_map(|w| w.chars().find(|c| c.is_alphanumeric()))
                    .collect::<String>()
                    .to_uppercase();
                container(
                    text(initials).font(self.sidebar_font()).size(14).color(Color { a: opacity.max(0.85), ..tint }),
                )
                .width(icon_size).height(icon_size)
                .center_x(icon_size).center_y(icon_size)
                .style(move |_: &Theme| container::Style {
                    background: Some(theme::with_alpha(tint, if is_selected { 0.3 } else { 0.18 }).into()),
                    border: iced::Border { radius: radius.into(), ..Default::default() },
                    ..Default::default()
                })
                .into()
            };
            // Selected: accent ring around the icon (same outer size either way).
            let icon_content: Element<Message> = container(icon_content)
                .padding(2)
                .style(move |_: &Theme| container::Style {
                    border: iced::Border {
                        color: if is_selected { theme::colors().accent } else { Color::TRANSPARENT },
                        width: 2.0,
                        radius: (radius + 2.0).into(),
                    },
                    ..Default::default()
                })
                .into();

            let ctx_space_id = space.room_id.clone();
            let btn = mouse_area(
                iced::widget::tooltip(
                    button(icon_content)
                        .on_press(Message::SelectSpace(Some(space_id)))
                        .padding(0)
                        .style(theme::button_transparent),
                    text(&space.name).font(self.sidebar_font()).size(12).color(theme::colors().text_normal),
                    iced::widget::tooltip::Position::Right,
                )
                .delay(crate::ui::TOOLTIP_DELAY)
                .style(|_: &Theme| container::Style {
                    background: Some(theme::colors().bg_mid.into()),
                    border: iced::Border { color: theme::colors().border, width: 1.0, radius: 4.0.into() },
                    ..Default::default()
                })
                .padding(6)
            )
            .on_right_press(Message::OpenContextMenu(
                crate::state::ContextMenu::Space { space_id: ctx_space_id },
            ));

            col = col.push(btn);
        }

        let spaces = scrollable(col).height(Fill)
            .direction(iced::widget::scrollable::Direction::Vertical(iced::widget::scrollable::Scrollbar::new().width(4).scroller_width(4)))
            .style(thin_scrollbar);
        // Restore button at the top, matching the member list's rail.
        let mut bar = column![].align_x(iced::Alignment::Center);
        if self.room_list_collapsed {
            let tip = if cfg!(target_os = "macos") { "Show room list (\u{2318}[)" } else { "Show room list (Ctrl+[)" };
            bar = bar.push(container(self.panel_restore_button(
                crate::icons::SVG_PANEL_LEFT_OPEN, Message::ToggleRoomList, tip, iced::widget::tooltip::Position::Right,
            )).padding(iced::Padding { top: 8.0, right: 0.0, bottom: 0.0, left: 0.0 }));
        }
        let bar = bar.push(spaces);
        container(bar)
            .width(56)
            .height(Fill)
            .style(|_: &Theme| container::Style {
                background: Some(theme::colors().bg_deep.into()),
                border: iced::Border {
                    color: theme::colors().border,
                    width: 1.0,
                    radius: 0.0.into(),
                },
                ..Default::default()
            })
            .into()
    }

    /// The rows the message list would show for a view, in order — hidden
    /// senders and (outside thread views) thread replies removed.
    pub(crate) fn visible_rows<'m>(&self, room_id: &str, msgs: &'m [fluxchat_core::MessageInfo]) -> Vec<&'m fluxchat_core::MessageInfo> {
        use fluxchat_core::RowKind;
        let is_thread_view = room_id.starts_with("thread::");
        let p = &self.prefs;
        let rows = msgs.iter()
            .filter(|m| m.event_id != fluxchat_core::timeline::READ_MARKER_ID)
            .filter(|m| !self.ignored_users.contains(&m.sender))
            .filter(|m| is_thread_view || m.thread_root.is_none())
            .filter(|m| match m.row_kind {
                RowKind::JoinLeave => self.show_joinpart,
                RowKind::Profile => p.show_profile_changes,
                RowKind::RoomState => p.show_room_changes,
                RowKind::Redacted => p.show_redactions,
                RowKind::Message | RowKind::Membership => true,
            })
            .collect::<Vec<_>>();
        // A day whose events are all hidden (thread replies, edits) would
        // leave its divider over nothing.
        let mut out = Vec::with_capacity(rows.len());
        for (i, m) in rows.iter().enumerate() {
            if m.is_date_divider() && rows.get(i + 1).is_none_or(|n| n.is_date_divider()) {
                continue;
            }
            out.push(*m);
        }
        out
    }

    /// Measured height of a row, or the estimate if it hasn't been laid out yet.
    pub(crate) fn row_height(&self, msg: &fluxchat_core::MessageInfo) -> f32 {
        self.row_heights.borrow().get(&msg.event_id).copied().unwrap_or(ESTIMATED_ROW_HEIGHT)
    }

    /// Which rows to build widgets for, and the spacer heights above/below:
    /// `(start, end, top_spacer, bottom_spacer)`.
    fn virtual_window(&self, rows: &[&fluxchat_core::MessageInfo]) -> (usize, usize, f32, f32) {
        let heights: Vec<f32> = rows.iter().map(|m| self.row_height(m)).collect();
        let total: f32 = heights.iter().sum();
        let vh = self.messages_viewport_h.max(200.0);
        let (lo, hi) = if self.messages_at_bottom {
            (total - vh - VIRTUAL_OVERSCAN, total)
        } else {
            let offset = self.last_scroll_offset;
            (offset - VIRTUAL_OVERSCAN, offset + vh + VIRTUAL_OVERSCAN)
        };
        window_for(&heights, lo, hi)
    }

    /// The first row intersecting the viewport and its distance from the top
    /// of the list, using the current height map.
    fn viewport_anchor(&self, rows: &[&fluxchat_core::MessageInfo]) -> Option<(String, f32)> {
        let offset = if self.messages_at_bottom {
            let total: f32 = rows.iter().map(|m| self.row_height(m)).sum();
            (total - self.messages_viewport_h).max(0.0)
        } else {
            self.last_scroll_offset
        };
        let mut acc = 0.0f32;
        for msg in rows {
            let h = self.row_height(msg);
            if acc + h > offset {
                return Some((msg.event_id.clone(), acc));
            }
            acc += h;
        }
        None
    }

    /// Distance from the top of the current view's list to a row.
    pub(crate) fn row_top(&self, event_id: &str) -> Option<f32> {
        let room_id = self.selected_room_id.as_deref()?;
        let msgs = self.messages_by_room.get(room_id)?;
        let mut acc = 0.0f32;
        for msg in self.visible_rows(room_id, msgs) {
            if msg.event_id == event_id {
                return Some(acc);
            }
            acc += self.row_height(msg);
        }
        None
    }

    /// Split the rooms visible in the current space into sidebar sections.
    fn group_rooms(&self) -> RoomGroups<'_> {
        let mut groups = RoomGroups::default();
        for room in &self.rooms {
            // Filter by selected space (includes rooms from descendant sub-spaces)
            if let Some(ref selected_sid) = self.selected_space_id {
                let dominated = self.space_descendants.get(selected_sid.as_str())
                    .is_some_and(|desc| room.space_id.as_ref().is_some_and(|rid| desc.contains(rid)));
                if !dominated {
                    continue;
                }
            }
            if room.is_favourite {
                groups.favourites.push(room);
            }
            if room.is_direct {
                groups.dms.push(room);
            } else if let Some(ref sid) = room.space_id {
                groups.by_space_id.entry(sid.clone()).or_default().push(room);
            } else {
                groups.ungrouped.push(room);
            }
        }
        groups
    }

    /// Space sections for rooms whose space isn't in the hierarchy tree, in a
    /// stable (name-sorted) order — HashMap order would reshuffle every frame.
    fn orphan_space_sections<'m, 'r>(
        &self,
        by_space_id: &'m std::collections::HashMap<String, Vec<&'r fluxchat_core::RoomInfo>>,
    ) -> Vec<(&'m String, &'m Vec<&'r fluxchat_core::RoomInfo>)> {
        let mut sections: Vec<_> = by_space_id.iter()
            .filter(|(sid, _)| !self.space_tree.iter().any(|n| n.info.room_id == **sid))
            .collect();
        sections.sort_by_key(|(_, rooms)| rooms.first().and_then(|r| r.space_name.clone()));
        sections
    }

    /// Room IDs in the order the sidebar shows them (Alt+1-9 targets).
    pub(crate) fn sidebar_room_order(&self) -> Vec<String> {
        let groups = self.group_rooms();
        let mut order: Vec<String> = Vec::new();
        let mut push = |rooms: &[&fluxchat_core::RoomInfo]| {
            order.extend(rooms.iter().map(|r| r.room_id.clone()));
        };
        push(&groups.favourites);
        push(&groups.ungrouped);
        for node in &self.space_tree {
            if let Some(rooms) = groups.by_space_id.get(&node.info.room_id) {
                push(rooms);
            }
        }
        for (_, rooms) in self.orphan_space_sections(&groups.by_space_id) {
            push(rooms);
        }
        push(&groups.dms);
        order
    }

    /// Room list panel with grouped sections.
    fn view_room_list(&self) -> Element<Message> {
        let selected_space_name: Option<String> = self
            .selected_space_id
            .as_ref()
            .and_then(|sid| self.spaces.iter().find(|s| s.room_id == *sid))
            .map(|s| s.name.clone());

        let RoomGroups { favourites, ungrouped, by_space_id, dms } = self.group_rooms();

        // Header
        let header_text: String = selected_space_name
            .as_deref()
            .unwrap_or("fluxchat")
            .to_string();
        let title: Element<Message> = match &self.selected_space_id {
            // A space's name opens its dialog (rooms, members, settings).
            Some(space_id) => crate::ui::tooltip_at(
                button(
                    row![
                        text(header_text)
                            .font(self.sidebar_font())
                            .size(14)
                            .color(theme::colors().text_bright)
                            .wrapping(iced::widget::text::Wrapping::None),
                        crate::ui::icon(crate::icons::SVG_CHEVRON_DOWN, 12.0, theme::colors().text_dim),
                    ]
                    .spacing(6)
                    .align_y(iced::Alignment::Center),
                )
                .on_press(Message::OpenSpaceDetails(space_id.clone()))
                .padding([2, 6])
                .style(crate::ui::row_style(false)),
                "Manage space",
                iced::widget::tooltip::Position::Bottom,
            ),
            None => text(header_text)
                .font(self.sidebar_font())
                .size(14)
                .color(theme::colors().text_bright)
                .into(),
        };
        let mut header_row = row![
            container(title).width(Fill).clip(true),
        ]
        .align_y(iced::Alignment::Center);

        // Settings + invite buttons for spaces
        if let Some(space_id) = &self.selected_space_id {
            let sid = space_id.clone();
            header_row = header_row.push(
                button(
                    iced::widget::svg(iced::widget::svg::Handle::from_memory(SVG_SETTINGS))
                        .width(14).height(14)
                        .style(|_, _| iced::widget::svg::Style { color: Some(theme::colors().text_dim) }),
                )
                .on_press(Message::OpenSpaceDetails(sid.clone()))
                .padding([2, 4])
                .style(|_, status| button::Style {
                    background: if matches!(status, button::Status::Hovered) {
                        Some(theme::tint(0.08).into())
                    } else {
                        None
                    },
                    border: iced::Border { radius: 4.0.into(), ..Default::default() },
                    ..Default::default()
                }),
            );
            let invite_handle = iced::widget::svg::Handle::from_memory(SVG_USER_PLUS);
            header_row = header_row.push(
                button(
                    iced::widget::svg(invite_handle)
                        .width(14).height(14)
                        .style(|_, _| iced::widget::svg::Style { color: Some(theme::colors().text_dim) }),
                )
                .on_press(Message::SpaceInvite(sid))
                .padding([2, 4])
                .style(|_, status| button::Style {
                    background: if matches!(status, button::Status::Hovered) {
                        Some(theme::tint(0.08).into())
                    } else {
                        None
                    },
                    border: iced::Border { radius: 4.0.into(), ..Default::default() },
                    ..Default::default()
                }),
            );
        }

        let shortcut = if cfg!(target_os = "macos") { "Hide room list (\u{2318}[)" } else { "Hide room list (Ctrl+[)" };
        header_row = header_row.push(crate::ui::tooltip_at(
            button(crate::ui::icon(crate::icons::SVG_PANEL_LEFT_CLOSE, 14.0, theme::colors().text_dim))
                .on_press(Message::ToggleRoomList)
                .padding([2, 4])
                .style(crate::ui::button_style(crate::ui::Tone::Ghost)),
            shortcut,
            iced::widget::tooltip::Position::Bottom,
        ));
        header_row = header_row.push(
            button(
                iced::widget::svg(iced::widget::svg::Handle::from_memory(SVG_PLUS))
                    .width(14).height(14)
                    .style(|_, _| iced::widget::svg::Style { color: Some(theme::colors().accent) })
            )
                .on_press(Message::ToggleCreateMenu)
                .padding([2, 4])
                .style(|_, status| button::Style {
                    background: if matches!(status, button::Status::Hovered) {
                        Some(theme::tint(0.08).into())
                    } else { None },
                    border: iced::Border { radius: 4.0.into(), ..Default::default() },
                    ..Default::default()
                }),
        );

        let header = container(header_row)
        .padding([8, 12])
        .width(Fill)
        .style(|_: &Theme| container::Style {
            border: iced::Border {
                color: theme::colors().border,
                width: 1.0,
                radius: 0.0.into(),
            },
            ..Default::default()
        });

        let mut col = column![header].spacing(0);

        // Create menu popup
        if self.create_menu_open {
            let menu_item = |label: &'static str, icon: &'static [u8], msg: Message| -> Element<Message> {
                button(
                    row![
                        iced::widget::svg(iced::widget::svg::Handle::from_memory(icon))
                            .width(16).height(16)
                            .style(|_, _| iced::widget::svg::Style { color: Some(theme::colors().text_normal) }),
                        text(label).font(self.sidebar_font()).size(13).color(theme::colors().text_normal),
                    ].spacing(8).align_y(iced::Alignment::Center)
                )
                .on_press(msg)
                .padding([6, 12])
                .width(Fill)
                .style(|_, status| button::Style {
                    background: if matches!(status, button::Status::Hovered) {
                        Some(theme::tint(0.08).into())
                    } else { None },
                    ..Default::default()
                })
                .into()
            };

            col = col.push(
                container(
                    column![
                        menu_item("New Room", SVG_HASH, Message::OpenModal(crate::state::Modal::CreateRoom)),
                        menu_item("Direct Message", SVG_CHAT, Message::OpenModal(crate::state::Modal::CreateDm)),
                        menu_item("Explore Rooms", SVG_SEARCH, Message::OpenModal(crate::state::Modal::Directory)),
                    ].spacing(0)
                )
                .padding(4)
                .width(Fill)
                .style(|_: &Theme| container::Style {
                    background: Some(theme::colors().bg_dark.into()),
                    border: iced::Border {
                        color: theme::colors().border,
                        width: 1.0,
                        radius: 4.0.into(),
                    },
                    ..Default::default()
                })
            );
        }

        // Invites notification bar
        if !self.invited_rooms.is_empty() {
            let count = self.invited_rooms.len();
            let label = if count == 1 {
                "1 new invite".to_string()
            } else {
                format!("{count} new invites")
            };
            let bell_handle = iced::widget::svg::Handle::from_memory(SVG_BELL);
            col = col.push(
                button(
                    row![
                        iced::widget::svg(bell_handle)
                            .width(14).height(14)
                            .style(|_, _| iced::widget::svg::Style { color: Some(theme::colors().red) }),
                        text(label).font(self.sidebar_font()).size(12).color(theme::colors().red),
                        Space::new().width(Fill),
                        container(
                            text(count.to_string()).font(self.sidebar_font()).size(11).color(theme::colors().text_bright),
                        )
                        .padding([1, 5])
                        .style(|_: &Theme| container::Style {
                            background: Some(theme::colors().red.into()),
                            border: iced::Border { radius: 8.0.into(), ..Default::default() },
                            ..Default::default()
                        }),
                    ]
                    .spacing(6)
                    .align_y(iced::Alignment::Center),
                )
                .on_press(Message::OpenModal(crate::state::Modal::PendingInvites))
                .padding([6, 12])
                .width(Fill)
                .style(|_, status| button::Style {
                    background: if matches!(status, button::Status::Hovered) {
                        Some(theme::with_alpha(theme::colors().red, 0.08).into())
                    } else {
                        None
                    },
                    ..Default::default()
                }),
            );
        }

        // Knocked (pending join requests)
        if !self.knocked_rooms.is_empty() {
            col = col.push(self.view_section_header("Pending", self.knocked_rooms.len()));
            if !self.collapsed_sections.contains("Pending") {
                for room in &self.knocked_rooms {
                    let rid_cancel = room.room_id.clone();
                    col = col.push(
                        container(
                            row![
                                text(&room.name).font(self.sidebar_font()).size(13).color(theme::colors().text_dim),
                                Space::new().width(Fill),
                                button(text("\u{2717}").font(self.sidebar_font()).size(14).color(theme::colors().text_dim))
                                    .on_press(Message::CancelKnock(rid_cancel))
                                    .padding([2, 6])
                                    .style(|_, _| button::Style { background: None, text_color: theme::colors().text_dim, ..Default::default() }),
                            ]
                            .spacing(4)
                            .align_y(iced::Alignment::Center),
                        )
                        .padding(iced::Padding { top: 0.0, right: 4.0, bottom: 0.0, left: 12.0 })
                    );
                }
            }
        }

        // Favourites
        if !favourites.is_empty() {
            col = col.push(self.view_section_header("Favourites", favourites.len()));
            if !self.collapsed_sections.contains("Favourites") {
                for room in &favourites {
                    col = col.push(self.view_room_entry(room, false));
                    for el in self.view_thread_entries(&room.room_id, 0.0) { col = col.push(el); }
                }
            }
        }

        // Ungrouped channels
        if !ungrouped.is_empty() {
            col = col.push(self.view_section_header_with_icon("Channels", ungrouped.len(), Some(SVG_HASH), None));
            if !self.collapsed_sections.contains("Channels") {
                for room in &ungrouped {
                    col = col.push(self.view_room_entry(room, false));
                    for el in self.view_thread_entries(&room.room_id, 0.0) { col = col.push(el); }
                }
            }
        }

        // Space-grouped rooms — render in tree order with depth indentation
        // First, determine base depth (0 for home, or the selected space's depth)
        let base_depth = self.selected_space_id.as_ref()
            .and_then(|sid| self.space_tree.iter().find(|n| n.info.room_id == *sid))
            .map(|n| n.depth)
            .unwrap_or(0);

        // Determine which spaces have rooms (directly or via descendants)
        let mut spaces_with_rooms: std::collections::HashSet<String> = std::collections::HashSet::new();
        for sid in by_space_id.keys() {
            spaces_with_rooms.insert(sid.clone());
            // Also mark all ancestor spaces as having rooms
            for node in &self.space_tree {
                if let Some(desc) = self.space_descendants.get(&node.info.room_id) {
                    if desc.contains(sid) {
                        spaces_with_rooms.insert(node.info.room_id.clone());
                    }
                }
            }
        }

        // Walk the space tree in order; render sections for spaces that have rooms (directly or via descendants)
        // Track collapsed depth: if a space at depth N is collapsed, skip everything deeper
        let mut skip_below_depth: Option<usize> = None;

        for node in &self.space_tree {
            let sid = &node.info.room_id;

            // Check if we should skip this node because an ancestor is collapsed
            if let Some(skip_depth) = skip_below_depth {
                if node.depth > skip_depth {
                    continue;
                } else {
                    skip_below_depth = None;
                }
            }

            if !spaces_with_rooms.contains(sid.as_str()) {
                continue;
            }
            {
                let rooms = by_space_id.get(sid.as_str());
                let space_name = &node.info.name;
                let space_mxc = node.info.avatar_url.as_deref();
                let indent_depth = node.depth.saturating_sub(base_depth);
                let indent_px = (indent_depth as u16).min(4) * 8;

                let room_count = rooms.map_or(0, |r| r.len());
                let header = self.view_section_header_with_icon(space_name, room_count, Some(SVG_HASH), space_mxc);
                let header_el: Element<Message> = if indent_px > 0 {
                    container(header).padding(iced::Padding { top: 0.0, right: 0.0, bottom: 0.0, left: indent_px as f32 }).into()
                } else {
                    header
                };
                col = col.push(header_el);

                if self.collapsed_sections.contains(space_name) {
                    skip_below_depth = Some(node.depth);
                } else {
                    if let Some(rooms) = rooms {
                        let room_indent = (indent_px as f32 + 10.0).max(10.0); // always indent rooms under spaces
                        for room in rooms {
                            let entry: Element<Message> = container(self.view_room_entry(room, false))
                                .padding(iced::Padding { top: 0.0, right: 0.0, bottom: 0.0, left: room_indent })
                                .into();
                            col = col.push(entry);
                            for el in self.view_thread_entries(&room.room_id, room_indent) { col = col.push(el); }
                        }
                    }
                }
            }
        }

        // Render any rooms whose space_id wasn't in the tree (fallback)
        for (sid, rooms) in self.orphan_space_sections(&by_space_id) {
            let space_name = rooms.first()
                .and_then(|r| r.space_name.as_deref())
                .unwrap_or("Other");
            let space_mxc = self.spaces.iter()
                .find(|s| s.room_id == *sid)
                .and_then(|s| s.avatar_url.as_deref());
            col = col.push(self.view_section_header_with_icon(space_name, rooms.len(), Some(SVG_HASH), space_mxc));
            if !self.collapsed_sections.contains(space_name) {
                for room in rooms {
                    col = col.push(self.view_room_entry(room, false));
                    for el in self.view_thread_entries(&room.room_id, 0.0) { col = col.push(el); }
                }
            }
        }

        // DMs
        if !dms.is_empty() {
            col = col.push(self.view_section_header_with_icon("Direct Messages", dms.len(), Some(SVG_CHAT), None));
            if !self.collapsed_sections.contains("Direct Messages") {
                for room in &dms {
                    col = col.push(self.view_room_entry(room, true));
                    for el in self.view_thread_entries(&room.room_id, 0.0) { col = col.push(el); }
                }
            }
        }

        container(scrollable(col).height(Fill).direction(iced::widget::scrollable::Direction::Vertical(iced::widget::scrollable::Scrollbar::new().width(4).scroller_width(4))).style(thin_scrollbar))
            .width(Fill)
            .height(Fill)
            .style(|_: &Theme| container::Style {
                background: Some(theme::colors().bg_dark.into()),
                ..Default::default()
            })
            .into()
    }

    /// Collapsible section header with SVG icon.
    fn view_section_header(&self, name: &str, count: usize) -> Element<'_, Message> {
        self.view_section_header_with_icon(name, count, None, None)
    }

    /// Section header with optional SVG icon or space avatar.
    fn view_section_header_with_icon(&self, name: &str, count: usize, icon_svg: Option<&'static [u8]>, space_avatar_mxc: Option<&str>) -> Element<'_, Message> {
        let collapsed = self.collapsed_sections.contains(name);
        let arrow = if collapsed { "\u{25b8}" } else { "\u{25be}" };
        let name_owned = name.to_string();
        let name_label = name.to_uppercase();

        let mut header_row = row![
            text(arrow).font(self.sidebar_font()).size(14).color(theme::colors().text_dim),
        ]
        .spacing(5)
        .align_y(iced::Alignment::Center);

        // Space avatar or section icon
        if let Some(handle) = space_avatar_mxc.and_then(|mxc| self.avatar_cache.get(mxc)) {
            header_row = header_row.push(
                container(
                    iced::widget::image(handle.clone())
                        .width(16)
                        .height(16)
                        .content_fit(iced::ContentFit::Cover),
                )
                .width(16).height(16).clip(true)
                .style(|_: &Theme| container::Style {
                    border: iced::Border { radius: 3.0.into(), ..Default::default() },
                    ..Default::default()
                })
            );
        } else if let Some(svg_bytes) = icon_svg {
            let handle = iced::widget::svg::Handle::from_memory(svg_bytes);
            header_row = header_row.push(
                iced::widget::svg(handle)
                    .width(14)
                    .height(14)
                    .style(|_, _| iced::widget::svg::Style {
                        color: Some(theme::colors().text_dim),
                    }),
            );
        }

        header_row = header_row.push(
            text(name_label).font(MONO_BOLD).size(11).color(theme::colors().text_dim)
        );
        header_row = header_row.push(Space::new().width(Fill));
        header_row = header_row.push(
            text(count.to_string()).font(self.sidebar_font()).size(11).color(theme::colors().text_dim)
        );

        column![
            container(rule::horizontal(1).style(|_: &Theme| rule::Style {
                color: theme::tint(0.06),
                radius: 0.0.into(),
                fill_mode: rule::FillMode::Full,
                snap: true,
            })).padding([0, 8]),
            button(header_row)
                .on_press(Message::ToggleSection(name_owned))
                .padding([4, 10])
                .width(Fill)
                .style(|_, status| button::Style {
                    background: if matches!(status, button::Status::Hovered) {
                        Some(theme::tint(0.03).into())
                    } else {
                        None
                    },
                    text_color: theme::colors().text_dim,
                    ..Default::default()
                }),
        ].spacing(0).into()
    }

    /// Thread entries indented below their room in the sidebar (unless
    /// collapsed): its most recently active threads, then any other thread
    /// of it that's open in a pane (in any window), so an open thread can
    /// always be found under its room.
    fn view_thread_entries(&self, room_id: &str, indent: f32) -> Vec<Element<'_, Message>> {
        if self.collapsed_room_threads.contains(room_id) {
            return Vec::new();
        }
        // Text width left for the title: sidebar minus paddings, offset and icon.
        let avail = self.sidebar_width - 12.0 - indent - THREAD_ENTRY_OFFSET - 12.0 - 6.0 - 24.0;
        let max_chars = (avail / (12.0 * 0.62)).max(8.0) as usize;
        let recent: Vec<&fluxchat_core::ThreadPreview> = self.room_threads.get(room_id)
            .map(|ts| ts.iter().take(crate::update::SIDEBAR_THREADS).collect())
            .unwrap_or_default();
        let prefix = format!("thread::{room_id}::");
        let mut open: Vec<&str> = self.pane_keys()
            .filter(|k| k.starts_with(&prefix))
            .filter(|k| !recent.iter().any(|t| k.ends_with(t.root_id.as_str()) && k.len() == prefix.len() + t.root_id.len()))
            .collect();
        open.sort_unstable();
        open.dedup();
        let mut out: Vec<Element<'_, Message>> = recent.iter()
            .map(|t| self.view_sidebar_thread(room_id, &t.root_id, ellipsize(&crate::view_thread_pane::preview_title(t), max_chars), Some(t), indent))
            .collect();
        out.extend(open.into_iter().filter_map(|k| {
            let (_, root) = fluxchat_core::timeline::parse_thread_key(k)?;
            Some(self.view_sidebar_thread(room_id, root, ellipsize(&self.thread_title(k), max_chars), None, indent))
        }));
        out
    }

    /// One thread row under its room: highlighted like rooms are (selected /
    /// on screen in another pane), with the thread menu, middle-click and drag.
    fn view_sidebar_thread<'a>(
        &'a self,
        room_id: &str,
        root_id: &str,
        title: String,
        preview: Option<&'a fluxchat_core::ThreadPreview>,
        indent: f32,
    ) -> Element<'a, Message> {
        let key = fluxchat_core::timeline::thread_key(room_id, root_id);
        let is_selected = self.selected_room_id.as_deref() == Some(key.as_str());
        let on_screen = !is_selected && self.visible(&key);
        let lit = is_selected || on_screen;
        // An agent's run in it, while that's worth a look (not once done).
        let run = self.run_state(&key).filter(|s| s.active());
        let entry = button(
            row![
                // Same left offset as the parent room's name: unread
                // dot + gap + room icon + gap (see view_room_entry).
                Space::new().width(Length::Fixed(THREAD_ENTRY_OFFSET)),
                crate::ui::icon(SVG_THREAD, 12.0, if lit { theme::colors().accent } else { theme::colors().text_dim }),
                text(title).font(self.sidebar_font()).size(12)
                    .color(if lit { theme::colors().text_bright } else { theme::colors().text_muted })
                    .wrapping(iced::widget::text::Wrapping::None),
                Space::new().width(Fill),
            ]
            .push(run.map(|s| crate::ui::with_tooltip(crate::agents::state_dot(s, 7.0), s.label())))
            .spacing(6).align_y(iced::Alignment::Center),
        )
        .on_press(Message::ListClick(key, Box::new(Message::OpenThread(room_id.to_string(), root_id.to_string()))))
        .padding([3, 8])
        .width(Fill)
        .clip(true)
        .style(move |_, status| button::Style {
            background: if is_selected {
                Some(theme::colors().bg_light.into())
            } else if on_screen {
                Some(on_screen_bg().into())
            } else if matches!(status, button::Status::Hovered) {
                Some(theme::tint(0.06).into())
            } else {
                None
            },
            border: iced::Border { radius: crate::ui::RADIUS.into(), ..Default::default() },
            ..Default::default()
        });
        let entry = self.thread_menu_area(entry, room_id, root_id, true);
        let entry = match preview {
            Some(t) => self.thread_hover_preview(entry, t, iced::widget::tooltip::Position::Right),
            None => entry,
        };
        // Same outer padding as room entries (+ the space indent).
        container(entry)
            .padding(iced::Padding { top: 0.0, right: 4.0, bottom: 0.0, left: 12.0 + indent })
            .into()
    }

    /// Single room entry in the list.
    fn view_room_entry<'a>(&self, room: &'a fluxchat_core::RoomInfo, is_dm: bool) -> Element<'a, Message> {
        let is_selected = self.selected_room_id.as_deref() == Some(&room.room_id);
        // Shown in another pane of this window: a softer highlight.
        let on_screen = !is_selected && self.visible(&room.room_id);
        let has_unread = room.unread_count > 0 && !self.read_rooms.contains(&room.room_id);
        let name_color = if is_selected || on_screen || has_unread {
            theme::colors().text_bright
        } else {
            theme::colors().text_muted
        };
        let bg = if is_selected {
            theme::colors().bg_light
        } else if on_screen {
            on_screen_bg()
        } else {
            Color::TRANSPARENT
        };

        // Room icon: avatar > DM person > encrypted lock > hash
        // People round, rooms rounded squares.
        let room_icon: Element<Message> = if let Some(handle) = room.avatar_url.as_ref().and_then(|mxc| self.avatar_cache.get(mxc)) {
            iced::widget::image(handle.clone())
                .width(18)
                .height(18)
                .content_fit(iced::ContentFit::Cover)
                .border_radius(if is_dm { 9.0 } else { 4.5 })
                .into()
        } else if is_dm {
            let handle = iced::widget::svg::Handle::from_memory(SVG_PERSON);
            iced::widget::svg(handle)
                .width(14).height(14)
                .style(|_, _| iced::widget::svg::Style { color: Some(theme::colors().text_dim) })
                .into()
        } else if room.is_encrypted {
            let handle = iced::widget::svg::Handle::from_memory(SVG_LOCK);
            iced::widget::svg(handle)
                .width(14).height(14)
                .style(|_, _| iced::widget::svg::Style { color: Some(theme::colors().text_dim) })
                .into()
        } else {
            let handle = iced::widget::svg::Handle::from_memory(SVG_HASH);
            iced::widget::svg(handle)
                .width(14).height(14)
                .style(|_, _| iced::widget::svg::Style { color: Some(theme::colors().text_dim) })
                .into()
        };

        // Unread indicator: colored dot on the far left + badge count on the right
        let has_mentions = room.highlight_count > 0 && !self.read_rooms.contains(&room.room_id);
        let indicator_dot: Element<Message> = if has_mentions {
            // Red dot for mentions
            container(Space::new().width(0).height(0))
                .width(8).height(8)
                .style(|_: &Theme| container::Style {
                    background: Some(theme::colors().red.into()),
                    border: iced::Border { radius: 4.0.into(), ..Default::default() },
                    ..Default::default()
                })
                .into()
        } else if has_unread {
            // Accent dot for regular unread
            container(Space::new().width(0).height(0))
                .width(8).height(8)
                .style(|_: &Theme| container::Style {
                    background: Some(theme::colors().accent.into()),
                    border: iced::Border { radius: 4.0.into(), ..Default::default() },
                    ..Default::default()
                })
                .into()
        } else {
            // Invisible spacer to keep alignment
            Space::new().width(8).height(8).into()
        };

        let unread_badge: Option<Element<Message>> = if has_mentions {
            Some(container(
                text(room.highlight_count.to_string())
                    .font(self.sidebar_font())
                    .size(11)
                    .color(theme::colors().text_bright),
            )
            .padding([1, 4])
            .style(|_: &Theme| container::Style {
                background: Some(theme::colors().red.into()),
                border: iced::Border { radius: 8.0.into(), ..Default::default() },
                ..Default::default()
            })
            .into())
        } else if has_unread {
            Some(container(
                text(room.unread_count.to_string())
                    .font(self.sidebar_font())
                    .size(11)
                    .color(theme::colors().text_bright),
            )
            .padding([1, 4])
            .style(|_: &Theme| container::Style {
                background: Some(theme::colors().accent.into()),
                border: iced::Border { radius: 8.0.into(), ..Default::default() },
                ..Default::default()
            })
            .into())
        } else {
            None
        };

        let mut row_content = row![indicator_dot].spacing(6).align_y(iced::Alignment::Center);
        if let Some(badge) = unread_badge {
            row_content = row_content.push(badge);
        }
        // Fixed 18px slot so names line up whether the room has an avatar
        // (18px) or a glyph (14px).
        row_content = row_content.push(container(room_icon).width(18).center_x(18));
        row_content = row_content.push(
            text(&room.name)
                .font(self.sidebar_font())
                .size(13)
                .color(name_color)
                .wrapping(iced::widget::text::Wrapping::None),
        );

        // Collapse / expand this room's threads in the sidebar.
        let thread_count = self.room_threads.get(&room.room_id).map(|t| t.len()).unwrap_or(0);
        let has_threads = thread_count > 0;
        // Right-hand markers: an agent waiting for you, an active call.
        let needs_you = self.room_needs_you(&room.room_id);
        let in_call = self.rooms_with_calls.contains(&room.room_id);
        if needs_you || in_call {
            row_content = row_content.push(Space::new().width(Fill));
        }
        if needs_you {
            row_content = row_content.push(crate::ui::with_tooltip(
                crate::agents::state_dot(crate::agents::RunState::NeedsYou, 7.0),
                "An agent is waiting for your answer",
            ));
        }
        if in_call {
            row_content = row_content.push(
                iced::widget::svg(iced::widget::svg::Handle::from_memory(SVG_PHONE))
                    .width(12).height(12)
                    .style(|_, _| iced::widget::svg::Style { color: Some(theme::colors().green) }),
            );
        }

        let room_id = room.room_id.clone();
        let ctx_room_id = room.room_id.clone();
        let ctx_menu = crate::state::ContextMenu::Room { room_id: ctx_room_id, is_dm };
        let room_button = button(
            container(row_content)
                .width(Fill)
                .clip(true)
        )
        .on_press(Message::ListClick(room_id.clone(), Box::new(Message::SelectRoom(room_id.clone()))))
        .padding([5, 8])
        .width(Fill)
        .style(move |_, status| button::Style {
            background: Some(if !is_selected && matches!(status, button::Status::Hovered) {
                theme::tint(0.06)
            } else {
                bg
            }.into()),
            text_color: name_color,
            border: iced::Border { radius: crate::ui::RADIUS.into(), ..Default::default() },
            ..Default::default()
        });
        let mut entry = row![room_button].align_y(iced::Alignment::Center);
        if has_threads {
            let collapsed = self.collapsed_room_threads.contains(&room.room_id);
            entry = entry.push(crate::ui::with_tooltip(
                button(
                    row![
                        crate::ui::icon(if collapsed { SVG_CHEVRON_RIGHT } else { SVG_CHEVRON_DOWN }, 11.0, theme::colors().text_dim),
                        text(thread_count.to_string()).font(self.sidebar_font()).size(11).color(theme::colors().text_dim),
                    ].spacing(2).align_y(iced::Alignment::Center),
                )
                .on_press(Message::ToggleRoomThreads(room_id))
                .padding([3, 4])
                .style(crate::ui::button_style(crate::ui::Tone::Ghost)),
                if collapsed { "Show threads" } else { "Hide threads" },
            ));
        }
        self.list_entry_area(
            container(entry).padding(iced::Padding { top: 0.0, right: 4.0, bottom: 0.0, left: 12.0 }),
            room.room_id.clone(),
            ctx_menu,
            true,
        )
    }

    /// Main content area (tabs + messages + input).
    /// The room header: avatar, name, agent chips, actions. `title_width`:
    /// it's the focused pane's title bar in a split, that wide (slimmer, the
    /// split action is the title bar's own, and a narrow one keeps only the
    /// core actions so the name still shows).
    pub(crate) fn view_room_header<'a>(&'a self, room_id: &'a str, title_width: Option<f32>) -> Element<'a, Message> {
        let in_title = title_width.is_some();
        let narrow = title_width.is_some_and(|w| w < HEADER_FULL_ACTIONS_WIDTH);
        // Thread tabs: derive name from parent room
        let (current_room, room_name) = if room_id.starts_with("thread::") {
            let parts: Vec<&str> = room_id.splitn(3, "::").collect();
            let parent_room_id = parts.get(1).unwrap_or(&"");
            let parent = self.rooms.iter().find(|r| r.room_id == *parent_room_id);
            let parent_name = parent.map(|r| r.name.as_str()).unwrap_or("room");
            (parent, format!("Thread in {parent_name}"))
        } else {
            let room = self.rooms.iter().find(|r| r.room_id == *room_id);
            let name = room.map(|r| {
                match r.space_name {
                    Some(ref space) => format!("{space}/{}", r.name),
                    None => r.name.clone(),
                }
            }).unwrap_or_else(|| room_id.to_string());
            (room, name)
        };

        // Open panels read as pressed: accent icon on an accent fill.
        let make_action_btn = |svg_bytes: &'static [u8], msg: Message, active: bool, label: &'static str| -> Element<Message> {
            let icon_color = if active { theme::colors().accent } else { theme::colors().text_dim };
            let handle = iced::widget::svg::Handle::from_memory(svg_bytes);
            let btn = button(
                iced::widget::svg(handle)
                    .width(16).height(16)
                    .style(move |_, _| iced::widget::svg::Style { color: Some(icon_color) }),
            )
            .on_press(msg)
            .padding([4, 6])
            .style(move |_, status| button::Style {
                background: if active {
                    Some(theme::with_alpha(theme::colors().accent, 0.16).into())
                } else if matches!(status, button::Status::Hovered) {
                    Some(theme::tint(0.06).into())
                } else {
                    None
                },
                border: iced::Border { radius: 4.0.into(), ..Default::default() },
                ..Default::default()
            });
            iced::widget::tooltip(
                btn,
                text(label).font(self.sidebar_font()).size(11).color(theme::colors().text_normal),
                iced::widget::tooltip::Position::Bottom,
            )
            .delay(crate::ui::TOOLTIP_DELAY)
            .gap(4)
            .style(|_: &Theme| container::Style {
                background: Some(theme::colors().bg_dark.into()),
                border: iced::Border { color: theme::colors().border, width: 1.0, radius: 4.0.into() },
                ..Default::default()
            })
            .padding(4)
            .into()
        };

        // Call button — start/join or hang up (hidden if calls not supported)
        let selected_rid = self.selected_room_id.clone().unwrap_or_default();
        let in_call = self.call_room_id.as_deref() == Some(selected_rid.as_str())
            && self.call_state == fluxchat_core::rtc::CallConnectionState::Connected;
        // Call: start / join or hang up; only where calls work.
        let call_btn: Option<Element<Message>> = (self.calls_supported || in_call).then(|| {
            let call_msg = if in_call {
                Message::CallLeave
            } else {
                Message::CallJoin(selected_rid)
            };
            let handle = iced::widget::svg::Handle::from_memory(SVG_PHONE);
            let icon_color = if in_call { theme::colors().green } else { theme::tint(0.4) };
            button(
                iced::widget::svg(handle)
                    .width(14).height(14)
                    .style(move |_, _| iced::widget::svg::Style { color: Some(icon_color) })
            )
            .on_press(call_msg)
            .padding([4, 6])
            .style(move |_, _| button::Style {
                background: if in_call { Some(Color::from_rgba(0.0, 0.5, 0.0, 0.2).into()) } else { None },
                border: iced::Border { radius: 4.0.into(), ..Default::default() },
                ..Default::default()
            })
            .into()
        });

        // Pin button: red when room has pins, accent when panel open, dim otherwise
        let pin_color = if !self.pinned_messages.is_empty() {
            theme::colors().red
        } else if self.pins_open {
            theme::colors().accent
        } else {
            theme::colors().text_dim
        };
        let pin_active = self.pins_open || !self.pinned_messages.is_empty();
        let pin_btn: Element<Message> = {
            let handle = iced::widget::svg::Handle::from_memory(SVG_PIN);
            let btn = button(
                iced::widget::svg(handle)
                    .width(16).height(16)
                    .style(move |_, _| iced::widget::svg::Style { color: Some(pin_color) }),
            )
            .on_press(Message::TogglePins)
            .padding([4, 6])
            .style(move |_, status| button::Style {
                background: if pin_active || matches!(status, button::Status::Hovered) {
                    Some(theme::tint(0.06).into())
                } else {
                    None
                },
                border: iced::Border { radius: 4.0.into(), ..Default::default() },
                ..Default::default()
            });
            iced::widget::tooltip(
                btn,
                text("Pins").font(self.sidebar_font()).size(11),
                iced::widget::tooltip::Position::Bottom,
            )
            .delay(crate::ui::TOOLTIP_DELAY)
            .gap(4)
            .style(|_: &Theme| container::Style {
                background: Some(theme::colors().bg_dark.into()),
                border: iced::Border { color: theme::colors().border, width: 1.0, radius: 4.0.into() },
                ..Default::default()
            })
            .padding(4)
            .into()
        };

        // Grouped: call · find (search, pins, gallery) · split · panels
        // (threads, mentions, members) · the room (settings, favourite).
        // Narrow pane title bars: search, threads, members.
        let divider = || -> Element<Message> {
            container(container(Space::new().width(1).height(16)).style(|_: &Theme| container::Style {
                background: Some(theme::colors().border.into()),
                ..Default::default()
            }))
            .padding([0, 5])
            .into()
        };
        let actions = row![]
            .push(call_btn.filter(|_| !narrow).map(|c| row![c, divider()].align_y(iced::Alignment::Center)))
            .push(make_action_btn(SVG_SEARCH, Message::ToggleSearch, self.search_open, "Search"))
            .push((!narrow).then_some(pin_btn))
            .push((!narrow).then(|| make_action_btn(crate::icons::SVG_GALLERY, Message::OpenGallery(crate::state::GalleryTab::Media),
                matches!(self.modal, Some(crate::state::Modal::Gallery)), "Gallery (\u{2318}/Ctrl+Shift+G)")))
            .push((!in_title).then(|| row![
                divider(),
                make_action_btn(SVG_SPLIT_V, Message::PaneSplit(iced::widget::pane_grid::Axis::Vertical), false, "Split side by side (Ctrl+B %)"),
            ].align_y(iced::Alignment::Center)))
            .push((!narrow).then(divider))
            .push(make_action_btn(SVG_THREAD, Message::ToggleThreads, self.threads_open, "Threads"))
            .push((!narrow).then(|| make_action_btn(SVG_AT_SIGN, Message::ToggleMentions, self.mentions_open, "Mentions (\u{2318}/Ctrl+I)")))
            .push(make_action_btn(SVG_USERS, Message::ToggleMemberList, self.show_member_list, "Members"))
            .push((!narrow).then(divider))
            .push((!narrow).then(|| make_action_btn(SVG_INFO, Message::OpenModal(crate::state::Modal::RoomSettings), false, "Settings")))
            .push((!narrow).then(|| {
                let is_fav = current_room.map(|r| r.is_favourite).unwrap_or(false);
                let star_color = if is_fav { Color::from_rgb(1.0, 0.8, 0.0) } else { theme::colors().text_dim };
                let handle = iced::widget::svg::Handle::from_memory(SVG_STAR);
                let fav_btn: Element<Message> = button(
                    iced::widget::svg(handle)
                        .width(16).height(16)
                        .style(move |_, _| iced::widget::svg::Style { color: Some(star_color) }),
                )
                .on_press(Message::ToggleFavourite)
                .padding([4, 6])
                .style(move |_, status| button::Style {
                    background: if is_fav {
                        Some(Color::from_rgba(1.0, 0.8, 0.0, 0.1).into())
                    } else if matches!(status, button::Status::Hovered) {
                        Some(theme::tint(0.06).into())
                    } else {
                        None
                    },
                    border: iced::Border { radius: 4.0.into(), ..Default::default() },
                    ..Default::default()
                })
                .into();
                let tt: Element<Message> = iced::widget::tooltip(
                    fav_btn,
                    text("Favourite").font(self.sidebar_font()).size(11).color(theme::colors().text_normal),
                    iced::widget::tooltip::Position::Bottom,
                )
            .delay(crate::ui::TOOLTIP_DELAY)
                .gap(4)
                .style(|_: &Theme| container::Style {
                    background: Some(theme::colors().bg_dark.into()),
                    border: iced::Border { color: theme::colors().border, width: 1.0, radius: 4.0.into() },
                    ..Default::default()
                })
                .padding(4)
                .into();
                tt
            }))
        .spacing(2)
        .align_y(iced::Alignment::Center);

        // Room avatar for header
        let header_avatar: Element<Message> = if let Some(handle) = current_room
            .and_then(|r| r.avatar_url.as_ref())
            .and_then(|mxc| self.avatar_cache.get(mxc))
        {
            // People round, rooms rounded squares.
            let dm = current_room.is_some_and(|r| r.is_direct);
            iced::widget::image(handle.clone())
                .width(22).height(22)
                .content_fit(iced::ContentFit::Cover)
                .border_radius(if dm { 11.0 } else { 5.5 })
                .into()
        } else {
            let icon = if current_room.is_some_and(|r| r.is_direct) {
                SVG_PERSON
            } else if current_room.is_some_and(|r| r.is_encrypted) {
                SVG_LOCK
            } else {
                SVG_HASH
            };
            iced::widget::svg(iced::widget::svg::Handle::from_memory(icon))
                .width(16).height(16)
                .style(|_, _| iced::widget::svg::Style { color: Some(theme::colors().text_dim) })
                .into()
        };

        // Name and agent chips take what the actions leave (clipped in
        // narrow panes): the actions are laid out first.
        let identity = row![
            header_avatar,
            text(room_name).font(self.sidebar_font()).size(14).color(theme::colors().text_bright)
                .wrapping(iced::widget::text::Wrapping::None),
        ]
        // An agent here: who, and what it's up to.
        .push(self.view_agent_marker(room_id, narrow))
        .spacing(8)
        .width(Fill)
        .align_y(iced::Alignment::Center);
        let header_row = row![container(identity).width(Fill).clip(true)]
        .push(actions)
        .width(Fill)
        .spacing(8)
        .align_y(iced::Alignment::Center)
        .padding(if in_title { [0, 2] } else { [6, 12] });

        header_row.into()
    }

    pub(crate) fn view_content(&self) -> Element<'_, Message> {
        let mut content = column![].width(Fill).height(Fill);

        // Room header: in a split the focused pane's title bar carries it.
        let in_title = self.window().grid.len() > 1;
        if let Some(room_id) = self.selected_room_id.as_deref().filter(|_| !in_title) {
            content = content.push(
                container(self.view_room_header(room_id, None))
                    .width(Fill)
                    .style(|_: &Theme| container::Style {
                        background: Some(theme::colors().bg_dark.into()),
                        border: iced::Border {
                            color: theme::colors().border,
                            width: 1.0,
                            radius: 0.0.into(),
                        },
                        ..Default::default()
                    }),
            );
        }

        // (overflow menu rendered as overlay below)

        // Messages area
        if let Some(room_id) = &self.selected_room_id {
            // Topic bar
            if let Some(room) = self.rooms.iter().find(|r| r.room_id == *room_id) {
                if let Some(topic) = &room.topic {
                    if !topic.is_empty() {
                        content = content.push(
                            container(
                                text(topic.as_str())
                                    .font(self.sidebar_font())
                                    .size(14)
                                    .color(theme::colors().text_dim),
                            )
                            .padding([3, 12])
                            .width(Fill)
                            .style(|_: &Theme| container::Style {
                                background: Some(theme::colors().bg_dark.into()),
                                border: iced::Border {
                                    color: theme::colors().border,
                                    width: 1.0,
                                    radius: 0.0.into(),
                                },
                                ..Default::default()
                            }),
                        );
                    }
                }
            }

            // Collect all optional panels into a single column so the content
            // column always has the same number of children. This prevents Iced
            // from losing the messages scrollable's scroll state when panels
            // are toggled on/off.
            let mut panels_col: iced::widget::Column<'_, Message> = column![].spacing(0);

            // Search is now rendered as a modal overlay (like quick switcher)

            // People asking to join (knock), for those who can let them in.
            if !room_id.starts_with("thread::") {
                if let Some(bar) = self.view_join_requests(room_id) {
                    panels_col = panels_col.push(bar);
                }
            }

            // Panels (notices): fixed header, scrollable body capped at
            // HUD_PANEL_MAX_HEIGHT.
            if self.notices_open {
                panels_col = panels_col.push(self.hud_panel(
                    "Notices", Vec::new(), Message::ToggleNotices, "No server notices",
                ));
            }

            // Always push the panels column — empty when no panels are open
            content = content.push(panels_col.width(Fill));

            set_scroll_room(room_id);
            let messages = self.messages_by_room.get(room_id);
            let mut msg_col = column![].spacing(0).padding([2, 8]).width(Fill);
            let mut divider_shown = false;

            if self.paginating {
                msg_col = msg_col.push(
                    container(
                        text("Loading older messages...")
                            .font(self.sidebar_font())
                            .size(13)
                            .color(theme::colors().text_dim),
                    )
                    .width(Fill)
                    .center_x(Fill)
                    .padding([8, 0]),
                );
            }

            if let Some(msgs) = messages {
                // "NEW" divider after the message that was last read when
                // the room was opened (kept while the room stays open).
                let last_read = self.read_anchor.as_ref()
                    .filter(|(k, _)| k == room_id)
                    .map(|(_, e)| e.clone());
                // Don't show divider if the last read event is the very last message
                // Only when something real follows it.
                let show_divider = last_read.as_ref().is_some_and(|lr| {
                    msgs.iter().rev()
                        .find(|m| !m.event_id.starts_with('~'))
                        .map(|m| &m.event_id) != Some(lr)
                });
                // Virtualized list: only rows near the viewport become widgets;
                // everything else is a spacer sized from measured (or estimated)
                // row heights. See `virtual_window` / `MessagesScrolled`.
                let rows = self.visible_rows(room_id, msgs);
                let (start, end, top_spacer, bottom_spacer) = self.virtual_window(&rows);
                *self.virt_anchor.borrow_mut() = self.viewport_anchor(&rows);
                if top_spacer > 0.0 {
                    msg_col = msg_col.push(Space::new().width(Fill).height(top_spacer));
                }
                for (i, msg) in rows.iter().enumerate().take(end).skip(start) {
                    let continuation = self.prefs.group_messages && !self.compact_mode
                        && i > 0 && is_continuation(rows[i - 1], msg);
                    let mut row_col = column![self.view_message(room_id, msg, continuation)].spacing(0).width(Fill);
                    // Unread divider: show after the last-read message
                    let is_last_read = show_divider && !divider_shown
                        && last_read.as_deref() == Some(msg.event_id.as_str());
                    if let Some(summary) = self.thread_summary_row(room_id, msg) {
                        row_col = row_col.push(summary);
                    }
                    // Read receipts for this message
                    if let Some(receipt_row) = self.view_read_receipts(msg) {
                        row_col = row_col.push(receipt_row);
                    }
                    // Unread divider: show after the last-read message
                    if is_last_read {
                        divider_shown = true;
                        row_col = row_col.push(
                            container(
                                row![
                                    rule::horizontal(1).style(|_: &Theme| rule::Style {
                                        color: theme::colors().red,
                                        radius: 0.0.into(),
                                        fill_mode: rule::FillMode::Full,
                                        snap: true,
                                    }),
                                    text(" NEW ").font(self.sidebar_font()).size(11).color(theme::colors().red),
                                    rule::horizontal(1).style(|_: &Theme| rule::Style {
                                        color: theme::colors().red,
                                        radius: 0.0.into(),
                                        fill_mode: rule::FillMode::Full,
                                        snap: true,
                                    }),
                                ].spacing(8).align_y(iced::Alignment::Center)
                            ).padding([4.0, 8.0])
                        );
                    }
                    msg_col = msg_col.push(crate::measured::Measured::new(
                        msg.event_id.clone(), &self.row_heights, row_col.into(),
                    ));
                }
                if bottom_spacer > 0.0 {
                    msg_col = msg_col.push(Space::new().width(Fill).height(bottom_spacer));
                }
            } else {
                msg_col = msg_col.push(self.view_placeholder_rows(PLACEHOLDER_ROWS));
            }

            let msg_scrollable = mouse_area(
                scrollable(container(msg_col).width(Fill))
                    .id(messages_scrollable_id().clone())
                    .width(Fill)
                    .height(Fill)
                    .on_scroll(Message::MessagesScrolled)
                    .direction(iced::widget::scrollable::Direction::Vertical(iced::widget::scrollable::Scrollbar::new().width(4).scroller_width(4))).style(thin_scrollbar),
            )
            .on_exit(Message::HoverLeft(None));

            // Helper: wrap msg_scrollable in a stack with overlays (new-messages pill)
            let mut msg_stack = stack![
                container(msg_scrollable).width(Fill).height(Fill),
            ].width(Fill).height(Fill);

            // Floating "new messages" pill when new messages arrived while scrolled up
            if !self.messages_at_bottom && self.new_messages_below {
                {
                    let pill = button(
                        text("\u{2193} New messages")
                        .font(self.sidebar_font()).size(12).color(theme::colors().text_bright)
                    )
                    .on_press(Message::ScrollToBottom)
                    .padding([4, 12])
                    .style(|_, status| button::Style {
                        background: Some(if matches!(status, button::Status::Hovered) {
                            theme::with_alpha(theme::colors().accent, 0.9)
                        } else {
                            theme::with_alpha(theme::colors().accent, 0.75)
                        }.into()),
                        border: iced::Border { radius: 12.0.into(), ..Default::default() },
                        ..Default::default()
                    });
                    msg_stack = msg_stack.push(
                        container(container(pill).width(Fill).center_x(Fill).padding(8.0))
                            .width(Fill)
                            .height(Fill)
                            .align_y(iced::alignment::Vertical::Bottom),
                    );
                }
            }

            // Emoji picker overlay — anchored to bottom-right of message area
            if self.emoji_open {
                msg_stack = msg_stack.push(
                    container(self.view_emoji_picker())
                        .width(Fill)
                        .height(Fill)
                        .align_y(iced::alignment::Vertical::Bottom)
                        .align_x(iced::alignment::Horizontal::Right),
                );
            }

            content = content.push(msg_stack);

            // A replaced (upgraded) room takes no new messages: point to
            // the room that continues it instead of the input area.
            let real_room = fluxchat_core::timeline::real_room_id(room_id);
            let successor = self.rooms.iter().find(|r| r.room_id == real_room).and_then(|r| r.successor.clone());
            if let Some(successor) = successor {
                use crate::ui::{self, Tone};
                let via: Vec<String> = successor.split_once(':').map(|(_, s)| vec![s.to_string()]).unwrap_or_default();
                content = content.push(container(ui::card(row![
                    column![
                        text("This room has been replaced").font(MONO).size(13).color(theme::colors().text_normal),
                        ui::hint("It's no longer active. The conversation continues in the new room."),
                    ].spacing(2).width(Fill),
                    ui::btn("Go to the new room", Tone::Primary, Some(Message::OpenMatrixLink(successor, None, via))),
                ].spacing(12).align_y(iced::Alignment::Center))).padding([8, 12]));
            } else {
                // Input area: typing indicator, reply/edit preview, GIF panel,
                // upload preview, help text, autocomplete, input bar
                for element in self.view_input_area(room_id) {
                    content = content.push(element);
                }
            }
        } else {
            // Nothing selected: a short welcome with the common next steps.
            use crate::ui::{self, Tone};
            let shortcut = if cfg!(target_os = "macos") { "\u{2318}K" } else { "Ctrl+K" };
            content = content.push(
                center(
                    column![
                        ui::icon(SVG_CHAT, 40.0, theme::colors().text_dim),
                        text("Pick a conversation").font(self.sidebar_font()).size(18).color(theme::colors().text_bright),
                        text(format!("Choose a room from the sidebar, or press {shortcut} to jump to one."))
                            .font(self.sidebar_font()).size(13).color(theme::colors().text_muted),
                        Space::new().height(6),
                        row![
                            ui::icon_btn(SVG_SEARCH, format!("Jump to room  {shortcut}"), Tone::Secondary,
                                Some(Message::ToggleQuickSwitcher)),
                            ui::icon_btn(SVG_PERSON, "New message", Tone::Secondary,
                                Some(Message::OpenModal(crate::state::Modal::CreateDm))),
                            ui::icon_btn(SVG_GLOBE, "Explore rooms", Tone::Secondary,
                                Some(Message::OpenModal(crate::state::Modal::Directory))),
                        ]
                        .spacing(8),
                    ]
                    .spacing(10)
                    .align_x(iced::Alignment::Center),
                )
                .height(Fill),
            );
        }

        let base: Element<Message> = container(content)
            .width(Fill)
            .height(Fill)
            .style(|_: &Theme| container::Style {
                background: Some(theme::colors().bg_mid.into()),
                ..Default::default()
            })
            .into();

        base
    }

    /// IRC-style status bar at the bottom.
    fn view_status_bar(&self) -> Element<Message> {
        let mut bar = row![].spacing(0).align_y(iced::Alignment::Center);

        let sep = || text(" \u{2502} ").font(self.sidebar_font()).size(13).color(theme::colors().text_dim);

        let open_security = Message::OpenModal(crate::state::Modal::Settings { tab: "security".into() });
        // Status-bar text that opens a settings tab when clicked.
        let link = |label: String, color: Color, msg: Message| -> Element<Message> {
            button(text(label).font(self.sidebar_font()).size(13).color(color))
                .on_press(msg)
                .padding([1, 4])
                .style(crate::ui::row_style(false))
                .into()
        };
        bar = bar.push(crate::ui::with_tooltip(
            button(crate::ui::icon(SVG_SETTINGS, 14.0, theme::colors().text_dim))
                .on_press(Message::OpenModal(crate::state::Modal::Settings { tab: "account".into() }))
                .padding([3, 6])
                .style(crate::ui::button_style(crate::ui::Tone::Ghost)),
            "Settings",
        ));

        // User ID in green (like IRC)
        bar = bar.push(
            text(&self.user_id).font(self.sidebar_font()).size(13).color(theme::colors().green)
        );

        // Current room info
        if let Some(room_id) = &self.selected_room_id {
            let real_room = fluxchat_core::timeline::real_room_id(room_id);
            let room = self.rooms.iter().find(|r| r.room_id == real_room);
            let room_name = room.map(|r| r.name.as_str()).unwrap_or(real_room);
            let label = match room {
                Some(r) if r.is_direct => room_name.to_string(),
                _ => format!("#{room_name}"),
            };
            let label = if room_id.starts_with("thread::") {
                format!("{label} \u{203a} {}", ellipsize(&self.thread_title(room_id), 32))
            } else {
                label
            };

            bar = bar.push(sep());
            bar = bar.push(text(label).font(self.sidebar_font()).size(13).color(theme::colors().text_normal)
                .wrapping(iced::widget::text::Wrapping::None));

            // Your rank here, IRC-style (@ op, + voice), as a labelled pill.
            if let Some(me) = self.members_by_room.get(room_id).and_then(|m| m.iter().find(|m| m.user_id == self.user_id)) {
                let rank = if me.power_level >= 100 {
                    Some(("@ op", theme::colors().yellow, "You're an admin in this room"))
                } else if me.power_level >= 50 {
                    Some(("+ mod", theme::colors().accent, "You're a moderator in this room"))
                } else {
                    None
                };
                if let Some((label, color, tip)) = rank {
                    // Slimmer than ui::badge: it mustn't make the bar taller.
                    let pill = container(text(label).font(self.sidebar_font()).size(11).color(color))
                        .padding([0, 6])
                        .style(move |_: &Theme| container::Style {
                            background: Some(theme::with_alpha(color, 0.14).into()),
                            border: iced::Border { color: theme::with_alpha(color, 0.3), width: 1.0, radius: 8.0.into() },
                            ..Default::default()
                        });
                    bar = bar.push(Space::new().width(6));
                    bar = bar.push(crate::ui::tooltip_at(pill, tip, iced::widget::tooltip::Position::Top));
                }
            }

            // Nick count
            if let Some(members) = self.members_by_room.get(room_id) {
                bar = bar.push(sep());
                bar = bar.push(
                    text(format!("{} nicks", members.len()))
                        .font(self.sidebar_font()).size(13).color(theme::colors().text_muted),
                );
            }

            bar = bar.push(sep());

            // Encryption status with lock icon
            if let Some(r) = room {
                if r.is_encrypted {
                    let lock_handle = iced::widget::svg::Handle::from_memory(SVG_LOCK);
                    bar = bar.push(
                        row![
                            iced::widget::svg(lock_handle)
                                .width(11).height(11)
                                .style(|_, _| iced::widget::svg::Style { color: Some(theme::colors().green) }),
                            text(" encrypted").font(self.sidebar_font()).size(13).color(theme::colors().green),
                        ].spacing(2).align_y(iced::Alignment::Center)
                    );
                } else {
                    bar = bar.push(text("unencrypted").font(self.sidebar_font()).size(13).color(theme::colors().text_muted));
                }
            }

            // The agent's runtime (model · context · working dir), from its footer.
            if let Some(rt) = self.agent_runtime(room_id).filter(|_| self.agent_in(room_id).is_some()) {
                bar = bar.push(sep());
                bar = bar.push(row![
                    crate::ui::icon(crate::icons::SVG_BOT, 12.0, theme::colors().text_dim),
                    text(rt.summary()).font(self.sidebar_font()).size(13).color(theme::colors().text_muted)
                        .wrapping(iced::widget::text::Wrapping::None),
                ].spacing(4).align_y(iced::Alignment::Center));
            }
        }

        // tmux-style window list (and the armed prefix)
        if self.windows.len() > 1 || self.tmux_prefix {
            bar = bar.push(sep());
            for (i, w) in self.windows.iter().enumerate() {
                let name = w.focused_key()
                    .map(fluxchat_core::timeline::real_room_id)
                    .and_then(|r| self.rooms.iter().find(|x| x.room_id == r))
                    .map(|r| crate::view::ellipsize(&r.name, 14))
                    .unwrap_or_else(|| "empty".into());
                let active = i == self.active_window;
                bar = bar.push(
                    button(text(format!("{i}:{name}{}", if active { "*" } else { "" }))
                        .font(self.sidebar_font()).size(13)
                        .color(if active { theme::colors().accent } else { theme::colors().text_muted }))
                        .on_press(Message::WindowSelect(i))
                        .padding([1, 4])
                        .style(crate::ui::row_style(false)),
                );
            }
            if self.tmux_prefix {
                bar = bar.push(text("  [prefix]").font(self.sidebar_font()).size(13).color(theme::colors().yellow));
            }
        }

        // Agent runs everywhere: "2 running · 1 waiting" → the Runs dialog.
        let (working, waiting) = self.live_run_counts();
        if working + waiting > 0 {
            let color = if waiting > 0 { theme::colors().yellow } else { theme::colors().accent };
            let label = match (working, waiting) {
                (w, 0) => format!("{w} running"),
                (0, n) => format!("{n} waiting on you"),
                (w, n) => format!("{w} running \u{00b7} {n} waiting"),
            };
            bar = bar.push(sep());
            bar = bar.push(crate::ui::tooltip_at(
                button(row![
                    crate::ui::icon(crate::icons::SVG_BOT, 12.0, color),
                    text(label).font(self.sidebar_font()).size(13).color(color),
                ].spacing(4).align_y(iced::Alignment::Center))
                    .on_press(Message::OpenModal(crate::state::Modal::Runs))
                    .padding([1, 4])
                    .style(crate::ui::row_style(false)),
                "Agent runs",
                iced::widget::tooltip::Position::Top,
            ));
        }

        bar = bar.push(Space::new().width(Fill));

        // Sync lag / connection state
        let (lag_label, lag_color) = if let Some(status) = &self.sync_status {
            (status.clone(), theme::colors().red)
        } else if self.sync_lag_ms == 0 {
            ("synced".to_string(), theme::colors().green)
        } else {
            let color = if self.sync_lag_ms < 200 {
                theme::colors().green
            } else if self.sync_lag_ms < 1000 {
                theme::colors().yellow
            } else {
                theme::colors().red
            };
            (format!("{}ms", self.sync_lag_ms), color)
        };
        bar = bar.push(text(lag_label).font(self.sidebar_font()).size(13).color(lag_color));

        // Verification status
        if let Some(ref sec) = self.security_state {
            bar = bar.push(sep());
            if sec.is_verified {
                bar = bar.push(link("\u{2713} verified".into(), theme::colors().green, open_security.clone()));
            } else {
                bar = bar.push(link("\u{2717} unverified".into(), theme::colors().yellow, open_security.clone()));
            }
            // A server-side key backup this session isn't using means missing
            // keys for old messages that could simply be fetched.
            if sec.backup_exists && !sec.backup_enabled {
                bar = bar.push(sep());
                bar = bar.push(link("key backup not connected".into(), theme::colors().yellow, open_security));
            }
        }

        container(bar)
            .width(Fill)
            .padding([2, 8])
            .style(|_: &Theme| container::Style {
                background: Some(theme::colors().bg_dark.into()),
                border: iced::Border {
                    color: theme::colors().border,
                    width: 1.0,
                    radius: 0.0.into(),
                },
                ..Default::default()
            })
            .into()
    }

    /// Subscription: keyboard shortcuts, periodic tick.
    pub fn subscription(&self) -> Subscription<Message> {
        let tick = iced::time::every(std::time::Duration::from_secs(5)).map(|_| Message::Tick);

        // Fast tick for audio progress bar / recording timer (100ms when active)
        let audio_tick = if self.audio_playing.is_some() || self.voice_recording {
            iced::time::every(std::time::Duration::from_millis(100)).map(|_| Message::Noop)
        } else {
            Subscription::none()
        };

        let keys = iced::keyboard::listen().map(|event| {
            if let iced::keyboard::Event::KeyPressed {
                key, modifiers, ..
            } = event
            {
                // Alt+Up/Down: previous/next room; with Shift: previous/next unread
                if modifiers.alt() {
                    let dir = match key {
                        iced::keyboard::Key::Named(iced::keyboard::key::Named::ArrowUp) => Some(-1),
                        iced::keyboard::Key::Named(iced::keyboard::key::Named::ArrowDown) => Some(1),
                        _ => None,
                    };
                    if let Some(dir) = dir {
                        return if modifiers.shift() {
                            Message::SwitchUnread(dir)
                        } else {
                            Message::SwitchRoomRelative(dir)
                        };
                    }
                }
                // Alt+1-9: switch tabs
                if modifiers.alt() {
                    if let iced::keyboard::Key::Character(ref c) = key {
                        if let Ok(n) = c.as_str().parse::<usize>() {
                            if (1..=9).contains(&n) {
                                return Message::SwitchTab(n - 1);
                            }
                        }
                    }
                }

                // Arrow keys for search results navigation
                if key == iced::keyboard::Key::Named(iced::keyboard::key::Named::ArrowDown) && !modifiers.alt() {
                    return Message::SearchArrowDown;
                }
                if key == iced::keyboard::Key::Named(iced::keyboard::key::Named::ArrowUp) && !modifiers.alt() {
                    return Message::SearchArrowUp;
                }
            }
            Message::Noop
        });

        // File drag-drop + sidebar resize mouse tracking + window focus + global shortcuts
        let file_drop = iced::event::listen_with(|event, _status, _window| {
            // Intercept global shortcuts before widgets consume them
            if let iced::Event::Keyboard(iced::keyboard::Event::KeyPressed { key, modified_key, modifiers, repeat, .. }) = &event {
                // tmux: Ctrl+B arms the prefix, the next key is a pane command.
                // `key` ignores Shift, so commands are read from `modified_key`
                // (`%` rather than `5`, `"` rather than `'`). Key repeat from
                // holding Ctrl+B neither re-arms nor cancels the prefix.
                let ctrl_b = modifiers.control()
                    && matches!(key, iced::keyboard::Key::Character(c) if c.as_str().eq_ignore_ascii_case("b"));
                // The flag is flipped here, not in update(), so a quick
                // Ctrl+B → x can't arrive before the prefix is armed.
                if crate::panes::prefix_armed() {
                    if *repeat || ctrl_b {
                        return None;
                    }
                    let name = crate::panes::key_name(modified_key)?;
                    crate::panes::set_prefix_armed(false);
                    return Some(Message::TmuxKey(name));
                }
                if ctrl_b && !*repeat {
                    crate::panes::set_prefix_armed(true);
                    return Some(Message::TmuxPrefix);
                }
                // Ctrl/Cmd+K: quick switcher
                if modifiers.command() || modifiers.control() {
                    if let iced::keyboard::Key::Character(c) = key {
                        if modifiers.shift() && c.as_str().eq_ignore_ascii_case("g") {
                            return Some(Message::OpenGallery(crate::state::GalleryTab::Media));
                        }
                        if c.as_str() == "k" || c.as_str() == "K" {
                            return Some(Message::ToggleQuickSwitcher);
                        }
                        // Ctrl+R (as in a shell): search what you've sent.
                        if (c.as_str() == "r" || c.as_str() == "R") && !modifiers.shift() {
                            return Some(Message::OpenPromptHistory);
                        }
                        if c.as_str() == "[" {
                            return Some(Message::ToggleRoomList);
                        }
                        if c.as_str() == "]" {
                            return Some(Message::ToggleMemberList);
                        }
                        if c.as_str() == "i" || c.as_str() == "I" {
                            return Some(Message::ToggleMentions);
                        }
                        if c.as_str() == "f" || c.as_str() == "F" {
                            return Some(Message::ToggleSearch);
                        }
                        if c.as_str() == "c" || c.as_str() == "C" {
                            return Some(Message::CopySelection);
                        }
                        if c.as_str() == "u" || c.as_str() == "U" {
                            return Some(Message::ClearInput);
                        }
                        // Text paste is handled by the editor; this only picks
                        // up images (screenshots) on the clipboard.
                        if c.as_str() == "v" || c.as_str() == "V" {
                            return Some(Message::PasteClipboardImage);
                        }
                    }
                }
                // Escape: close the topmost modal/panel/completion. Handled
                // here (all events) rather than in `keyboard::listen` (ignored
                // events only) so it also works while a text input has focus —
                // and only here, so one press closes one thing.
                if *key == iced::keyboard::Key::Named(iced::keyboard::key::Named::Escape) {
                    return Some(Message::Escape);
                }
                // ←/→ browse the gallery from its lightbox.
                if crate::view_gallery::lightbox_nav() && modifiers.is_empty() {
                    match key {
                        iced::keyboard::Key::Named(iced::keyboard::key::Named::ArrowLeft) => return Some(Message::GalleryStep(-1)),
                        iced::keyboard::Key::Named(iced::keyboard::key::Named::ArrowRight) => return Some(Message::GalleryStep(1)),
                        _ => {}
                    }
                }
            }
            match event {
                iced::Event::Window(iced::window::Event::FileDropped(path)) => {
                    Some(Message::FileDropped(path))
                }
                iced::Event::Window(iced::window::Event::Resized(size)) => {
                    Some(Message::WindowResized(size.width, size.height))
                }
                iced::Event::Window(iced::window::Event::Focused) => {
                    Some(Message::WindowFocusChanged(true))
                }
                iced::Event::Window(iced::window::Event::Unfocused) => {
                    Some(Message::WindowFocusChanged(false))
                }
                iced::Event::Keyboard(iced::keyboard::Event::ModifiersChanged(m)) => {
                    MODIFIERS.store(m.bits(), std::sync::atomic::Ordering::Relaxed);
                    None
                }
                iced::Event::Mouse(iced::mouse::Event::CursorMoved { position }) => {
                    // Record without emitting a Message: every Message triggers
                    // a full view rebuild, and the cursor moves constantly.
                    set_cursor_position(position.x, position.y);
                    None
                }
                iced::Event::Mouse(iced::mouse::Event::ButtonReleased(iced::mouse::Button::Left)) => {
                    Some(Message::SidebarDragEnd)
                }
                iced::Event::Mouse(iced::mouse::Event::WheelScrolled { delta }) if wheel_up_at_timeline_top(&delta) => {
                    Some(Message::TimelineWheelUpAtTop)
                }
                _ => None,
            }
        });

        // Timeline updates from background diff watchers
        let timeline_sub = Subscription::run(timeline_update_stream);

        // Call events from LiveKit session
        let call_sub = Subscription::run(call_event_stream);

        // Only stream cursor movement into update() while resizing the sidebar.
        let sidebar_drag = if self.resizing.is_some() {
            iced::event::listen_with(|event, _status, _window| match event {
                iced::Event::Mouse(iced::mouse::Event::CursorMoved { position }) => {
                    Some(Message::CursorMoved(position.x, position.y))
                }
                _ => None,
            })
        } else {
            Subscription::none()
        };

        // Mic test level meter
        let mic_tick = if self.mic_test.is_some() {
            iced::time::every(std::time::Duration::from_millis(60)).map(|_| Message::MicLevelTick)
        } else {
            Subscription::none()
        };

        let close = iced::window::close_requests().map(|_| Message::CloseRequested);

        Subscription::batch([tick, keys, file_drop, timeline_sub, call_sub, audio_tick, sidebar_drag, mic_tick, close])
    }

    /// Application theme — dark.
    pub fn theme(&self) -> Theme {
        if theme::colors().dark {
            Theme::Dark
        } else {
            Theme::Light
        }
    }
}

/// Height assumed for a message row that hasn't been laid out yet.
pub(crate) const ESTIMATED_ROW_HEIGHT: f32 = 40.0;
/// Extra content built above and below the viewport so scrolling doesn't
/// reveal blank space before the next frame.
const VIRTUAL_OVERSCAN: f32 = 600.0;

/// Rows whose [top, bottom) range intersects the pixel range [lo, hi], plus
/// the summed heights of the rows before and after them.
/// Whether `msg` continues a run from the same sender started by `prev`
/// (shown without repeating avatar / with a dimmed nick).
pub(crate) fn is_continuation(prev: &fluxchat_core::MessageInfo, msg: &fluxchat_core::MessageInfo) -> bool {
    prev.sender == msg.sender
        && !prev.is_system() && !msg.is_system()
        && prev.row_kind != fluxchat_core::RowKind::Redacted
        && !prev.is_emote && !msg.is_emote
        && msg.reply_to_id.is_none()
        && msg.timestamp.saturating_sub(prev.timestamp) < crate::view_messages::MSG_GROUP_SECS
}

/// Compact thread line for narrow places: "3 replies · alice · 5m".
/// Characters of a pinned message shown in the pins sidebar.
const PIN_PREVIEW_CHARS: usize = 280;

fn thread_meta_short(t: &fluxchat_core::ThreadPreview) -> String {
    let mut parts = vec![if t.reply_count == 1 { "1 reply".to_string() } else { format!("{} replies", t.reply_count) }];
    if let Some(latest) = &t.latest_sender_name {
        parts.push(latest.clone());
    }
    if t.last_activity > 0 {
        let now = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_secs()).unwrap_or(0);
        let ago = now.saturating_sub(t.last_activity);
        parts.push(match ago {
            0..=59 => "now".into(),
            60..=3599 => format!("{}m", ago / 60),
            3600..=86399 => format!("{}h", ago / 3600),
            _ => format!("{}d", ago / 86400),
        });
    }
    parts.join(" \u{00b7} ")
}

/// Cut `s` to at most `max` characters, ending in "…" when shortened.
pub(crate) fn ellipsize(s: &str, max: usize) -> String {
    if s.chars().count() <= max {
        return s.to_string();
    }
    let mut out: String = s.chars().take(max.saturating_sub(1)).collect();
    out = out.trim_end().to_string();
    out.push('\u{2026}');
    out
}

/// "3 replies · alice · 5m ago" for a thread list entry.
fn thread_meta(t: &fluxchat_core::ThreadPreview) -> String {
    let mut parts = vec![if t.reply_count == 1 { "1 reply".to_string() } else { format!("{} replies", t.reply_count) }];
    if let Some(latest) = &t.latest_sender_name {
        parts.push(format!("last: {latest}"));
    }
    if t.last_activity > 0 {
        let now = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_secs()).unwrap_or(0);
        let ago = now.saturating_sub(t.last_activity);
        parts.push(match ago {
            0..=59 => "just now".into(),
            60..=3599 => format!("{}m ago", ago / 60),
            3600..=86399 => format!("{}h ago", ago / 3600),
            _ => format!("{}d ago", ago / 86400),
        });
    }
    parts.join("  \u{00b7}  ")
}

fn window_for(heights: &[f32], lo: f32, hi: f32) -> (usize, usize, f32, f32) {
    let mut start = heights.len();
    let mut end = heights.len();
    let mut acc = 0.0f32;
    for (i, h) in heights.iter().enumerate() {
        let row_end = acc + h;
        if start == heights.len() && row_end > lo {
            start = i;
        }
        if acc > hi {
            end = i;
            break;
        }
        acc = row_end;
    }
    let end = end.max(start);
    let top: f32 = heights[..start].iter().sum();
    let bottom: f32 = heights[end..].iter().sum();
    (start, end, top, bottom)
}

#[cfg(test)]
mod tests {
    use super::window_for;

    #[test]
    fn window_covers_requested_range_with_exact_spacers() {
        let heights = vec![10.0; 100]; // 1000px of rows
        let (start, end, top, bottom) = window_for(&heights, 250.0, 400.0);
        assert_eq!((start, end), (25, 41));
        assert_eq!(top, 250.0);
        assert_eq!(bottom, (100 - 41) as f32 * 10.0);
        assert_eq!(top + heights[start..end].iter().sum::<f32>() + bottom, 1000.0);
    }

    #[test]
    fn window_clamps_to_list_edges() {
        let heights = vec![30.0, 50.0, 20.0];
        assert_eq!(window_for(&heights, -600.0, 5000.0), (0, 3, 0.0, 0.0));
        // Range entirely below the content: nothing rendered, all in the top spacer.
        assert_eq!(window_for(&heights, 500.0, 900.0), (3, 3, 100.0, 0.0));
        assert_eq!(window_for(&[], 0.0, 100.0), (0, 0, 0.0, 0.0));
    }
}

/// Rooms of the current space bucketed into sidebar sections.
#[derive(Default)]
struct RoomGroups<'a> {
    favourites: Vec<&'a fluxchat_core::RoomInfo>,
    ungrouped: Vec<&'a fluxchat_core::RoomInfo>,
    by_space_id: std::collections::HashMap<String, Vec<&'a fluxchat_core::RoomInfo>>,
    dms: Vec<&'a fluxchat_core::RoomInfo>,
}

/// The main timeline's bounds while it's scrolled to the top of what's
/// loaded (None otherwise). A scroll-up gesture there can't move the view,
/// so the scrollable reports nothing; the event listener uses this to turn
/// that gesture into "load older messages" without a Message per wheel tick.
static TIMELINE_AT_TOP: std::sync::Mutex<Option<iced::Rectangle>> = std::sync::Mutex::new(None);

pub(crate) fn set_timeline_at_top(bounds: Option<iced::Rectangle>) {
    *TIMELINE_AT_TOP.lock().unwrap_or_else(|e| e.into_inner()) = bounds;
}

/// A wheel/trackpad scroll up over the timeline while it's at the top.
fn wheel_up_at_timeline_top(delta: &iced::mouse::ScrollDelta) -> bool {
    let up = match delta {
        iced::mouse::ScrollDelta::Lines { y, .. } | iced::mouse::ScrollDelta::Pixels { y, .. } => *y > 0.0,
    };
    if !up {
        return false;
    }
    let (x, y) = cursor_position();
    TIMELINE_AT_TOP.lock().unwrap_or_else(|e| e.into_inner())
        .is_some_and(|b| b.contains(iced::Point::new(x, y)))
}

/// Sidebar background of a room / thread shown in an unfocused pane: half
/// way between nothing and the selected row's.
fn on_screen_bg() -> Color {
    theme::with_alpha(theme::colors().bg_light, 0.5)
}

/// Keyboard modifiers held right now (bits), kept by the global listener.
static MODIFIERS: std::sync::atomic::AtomicU32 = std::sync::atomic::AtomicU32::new(0);

/// Cmd (macOS) / Ctrl held: a click on a room or thread row opens it beside.
pub(crate) fn open_modifier_held() -> bool {
    iced::keyboard::Modifiers::from_bits_truncate(MODIFIERS.load(std::sync::atomic::Ordering::Relaxed)).command()
}

static CURSOR_X: std::sync::atomic::AtomicU32 = std::sync::atomic::AtomicU32::new(0);
static CURSOR_Y: std::sync::atomic::AtomicU32 = std::sync::atomic::AtomicU32::new(0);

fn set_cursor_position(x: f32, y: f32) {
    CURSOR_X.store(x.to_bits(), std::sync::atomic::Ordering::Relaxed);
    CURSOR_Y.store(y.to_bits(), std::sync::atomic::Ordering::Relaxed);
}

/// Last known window cursor position, tracked outside the update loop.
pub(crate) fn cursor_position() -> (f32, f32) {
    (
        f32::from_bits(CURSOR_X.load(std::sync::atomic::Ordering::Relaxed)),
        f32::from_bits(CURSOR_Y.load(std::sync::atomic::Ordering::Relaxed)),
    )
}

/// Human-readable day for a date divider: "Today", "Yesterday", or the full date.
pub(crate) fn format_day(unix_secs: u64) -> String {
    use chrono::{Datelike, TimeZone};
    let Some(day) = chrono::Utc.timestamp_opt(unix_secs as i64, 0).single()
        .map(|utc| utc.with_timezone(&chrono::Local).date_naive()) else {
        return String::new();
    };
    let today = chrono::Local::now().date_naive();
    if day == today {
        "Today".to_string()
    } else if today.pred_opt() == Some(day) {
        "Yesterday".to_string()
    } else if day.year() == today.year() {
        day.format("%A, %-d %B").to_string()
    } else {
        day.format("%A, %-d %B %Y").to_string()
    }
}

/// Convert a Unix timestamp (seconds) to local time-of-day in seconds.
pub(crate) fn local_time_of_day(unix_secs: u64) -> u64 {
    use chrono::{TimeZone, Timelike};
    let dt = chrono::Utc.timestamp_opt(unix_secs as i64, 0)
        .single()
        .map(|utc| utc.with_timezone(&chrono::Local));
    match dt {
        Some(local) => (local.hour() * 3600 + local.minute() * 60 + local.second()) as u64,
        None => unix_secs % 86400,
    }
}

/// Stream that drains the global timeline update channel.
/// Used as an Iced `Subscription::run` source — function pointer for identity.
fn timeline_update_stream() -> impl futures_util::Stream<Item = Message> {
    async_stream::stream! {
        let mut rx = {
            let lock = crate::state::TIMELINE_RX.get();
            match lock {
                Some(mutex) => {
                    let mut guard = mutex.lock().unwrap_or_else(|e| e.into_inner());
                    match guard.take() {
                        Some(rx) => rx,
                        None => return,
                    }
                }
                None => return,
            }
        };
        loop {
            match rx.recv().await {
                Some(msg) => yield msg,
                None => return,
            }
        }
    }
}

fn call_event_stream() -> impl futures_util::Stream<Item = Message> {
    async_stream::stream! {
        let mut rx = {
            let lock = crate::state::CALL_RX.get();
            match lock {
                Some(mutex) => {
                    let mut guard = mutex.lock().unwrap_or_else(|e| e.into_inner());
                    match guard.take() {
                        Some(rx) => rx,
                        None => return,
                    }
                }
                None => return,
            }
        };
        loop {
            match rx.recv().await {
                Some(msg) => yield msg,
                None => return,
            }
        }
    }
}

#[cfg(test)]
mod ellipsize_tests {
    use super::ellipsize;

    #[test]
    fn shortens_with_ellipsis() {
        assert_eq!(ellipsize("short", 10), "short");
        assert_eq!(ellipsize("add heart of the beast to seerr", 12), "add heart o\u{2026}");
        assert_eq!(ellipsize("ab cd", 4), "ab\u{2026}");
    }
}
