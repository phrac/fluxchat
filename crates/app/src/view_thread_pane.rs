//! The thread pane: a thread shown beside its room's timeline, with its own
//! composer. Live via the thread's own Timeline watcher.

use iced::widget::{column, container, row, scrollable, text};
use iced::{Alignment, Element, Fill, Length, Theme};

use crate::message::Message;
use crate::state::FluxChat;
use crate::theme;
use crate::ui::{self, Tone};
use crate::view::thin_scrollbar;

impl FluxChat {
    pub(crate) fn view_thread_pane<'a>(&'a self, key: &'a str) -> Element<'a, Message> {
        let room_id = fluxchat_core::timeline::real_room_id(key);
        let room_name = self.rooms.iter().find(|r| r.room_id == room_id)
            .map(|r| if r.is_direct { r.name.clone() } else { format!("#{}", r.name) })
            .unwrap_or_default();
        let msgs = self.messages_by_room.get(key);
        let count = msgs.map(|m| m.iter().filter(|m| !m.is_system() && m.thread_root.is_some()).count()).unwrap_or(0);
        let subtitle = match count {
            0 => room_name,
            1 => format!("{room_name}  \u{00b7}  1 reply"),
            n => format!("{room_name}  \u{00b7}  {n} replies"),
        };

        let header = container(
            row![
                ui::icon(crate::icons::SVG_THREAD, 16.0, theme::colors().accent),
                column![
                    text("Thread").font(self.sidebar_font()).size(14).color(theme::colors().text_bright),
                    text(subtitle).font(self.sidebar_font()).size(11).color(theme::colors().text_dim)
                        .wrapping(text::Wrapping::None),
                ].spacing(1).width(Fill).clip(true),
                ui::icon_only(crate::icons::SVG_MAXIMIZE, "Open full size", Tone::Ghost, Some(Message::ExpandThreadPane)),
                ui::icon_only(crate::icons::SVG_CLOSE, "Close thread", Tone::Ghost, Some(Message::CloseThreadPane)),
            ]
            .spacing(10)
            .align_y(Alignment::Center),
        )
        .padding([8, 12])
        .width(Fill)
        .style(|_: &Theme| container::Style {
            background: Some(theme::colors().bg_dark.into()),
            border: iced::Border { color: theme::colors().border, width: 1.0, radius: 0.0.into() },
            ..Default::default()
        });

        // Messages (threads are short: no virtualization)
        let body: Element<'a, Message> = match msgs {
            None => ui::empty_state(crate::icons::SVG_CLOCK, "Loading thread\u{2026}", None),
            Some(msgs) => {
                let rows = self.visible_rows(key, msgs);
                let mut list = column![].spacing(0).padding([4, 6]).width(Fill);
                for (i, msg) in rows.iter().enumerate() {
                    let continuation = self.prefs.group_messages && !self.compact_mode
                        && i > 0 && crate::view::is_continuation(rows[i - 1], msg);
                    let mut item = column![self.view_message(msg, continuation)].width(Fill);
                    // Divider under the root message.
                    if i == 0 && msg.thread_root.is_none() && rows.len() > 1 {
                        item = item.push(container(ui::divider()).padding([6, 0]));
                    }
                    list = list.push(item);
                }
                if rows.len() <= 1 {
                    list = list.push(ui::empty_state(crate::icons::SVG_THREAD, "No replies yet", Some("Start the thread below.".into())));
                }
                scrollable(list)
                    .id(crate::view::thread_scrollable_id())
                    .anchor_bottom()
                    .height(Fill)
                    .direction(iced::widget::scrollable::Direction::Vertical(
                        iced::widget::scrollable::Scrollbar::new().width(4).scroller_width(4),
                    ))
                    .style(thin_scrollbar)
                    .into()
            }
        };

        // Composer
        let ctrl_enter = self.prefs.ctrl_enter_to_send;
        let input = iced::widget::TextEditor::new(&self.thread_input)
            .id(crate::view::thread_input_id())
            .placeholder("Reply in thread\u{2026}")
            .on_action(Message::ThreadInputAction)
            .key_binding(move |key_press| {
                use iced::keyboard::Key;
                use iced::keyboard::key::Named;
                use iced::widget::text_editor::Binding;
                match key_press.key {
                    Key::Named(Named::Enter) if ctrl_enter
                        && (key_press.modifiers.command() || key_press.modifiers.control()) => Some(Binding::Custom(Message::ThreadSend)),
                    Key::Named(Named::Enter) if !ctrl_enter && !key_press.modifiers.shift() => Some(Binding::Custom(Message::ThreadSend)),
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
        let composer = container(
            container(
                row![
                    input,
                    ui::icon_only(crate::icons::SVG_FORWARD, "Send", Tone::Ghost,
                        (!self.thread_input.text().trim().is_empty()).then_some(Message::ThreadSend)),
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
        .padding([8, 10]);

        container(column![header, body, composer].height(Fill))
            .width(Fill)
            .height(Fill)
            .style(|_: &Theme| container::Style {
                background: Some(theme::colors().bg_mid.into()),
                ..Default::default()
            })
            .into()
    }
}
