use iced::widget::{
    button, column, container, row, scrollable, stack,
    text, Space,
};
use iced::{Color, Element, Fill, Theme};

use crate::icons::*;
use crate::message::Message;
use crate::state::FluxChat;
use crate::theme;
use crate::view::{thin_scrollbar, MONO};

impl FluxChat {
    /// Member list panel (right sidebar).
    pub(crate) fn view_member_list(&self) -> Element<Message> {
        let room_id = match &self.selected_room_id {
            Some(id) => id,
            None => {
                return container(Space::new())
                    .width(0)
                    .into();
            }
        };

        let members = self.members_by_room.get(room_id);
        let total = members.map(|m| m.len()).unwrap_or(0);

        let invite_handle = iced::widget::svg::Handle::from_memory(
            br#"<svg xmlns="http://www.w3.org/2000/svg" width="24" height="24" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><path d="M16 21v-2a4 4 0 0 0-4-4H5a4 4 0 0 0-4 4v2"/><circle cx="8.5" cy="7" r="4"/><line x1="20" y1="8" x2="20" y2="14"/><line x1="23" y1="11" x2="17" y2="11"/></svg>"#.as_slice(),
        );
        let header = container(
            row![
                text(format!("USERS \u{2014} {total}"))
                    .font(MONO)
                    .size(14)
                    .color(theme::TEXT_BRIGHT),
                Space::new().width(Fill),
                button(
                    iced::widget::svg(invite_handle)
                        .width(14).height(14)
                        .style(|_, _| iced::widget::svg::Style { color: Some(theme::TEXT_DIM) }),
                )
                .on_press(Message::OpenModal(crate::state::Modal::Invite))
                .padding([2, 4])
                .style(|_, status| button::Style {
                    background: if matches!(status, button::Status::Hovered) {
                        Some(Color::from_rgba(1.0, 1.0, 1.0, 0.08).into())
                    } else {
                        None
                    },
                    border: iced::Border { radius: 4.0.into(), ..Default::default() },
                    ..Default::default()
                }),
            ]
            .align_y(iced::Alignment::Center),
        )
        .padding([8, 12])
        .width(Fill);

        let mut col = column![header].spacing(0);

        if let Some(members) = members {
            // Group by role
            let admins: Vec<_> = members.iter().filter(|m| m.role == "Admin").collect();
            let mods: Vec<_> = members.iter().filter(|m| m.role == "Moderator").collect();
            let regular: Vec<_> = members.iter().filter(|m| m.role == "Member").collect();

            if !admins.is_empty() {
                col = col.push(self.view_member_section("@ Admins", &admins));
            }
            if !mods.is_empty() {
                col = col.push(self.view_member_section("% Mods", &mods));
            }
            if !regular.is_empty() {
                col = col.push(self.view_member_section("Members", &regular));
            }
        }

        let member_scroll = scrollable(col).height(Fill).direction(iced::widget::scrollable::Direction::Vertical(iced::widget::scrollable::Scrollbar::new().width(4).scroller_width(4))).style(thin_scrollbar);

        let mut panel = column![member_scroll].height(Fill);

        // Call widget — always at bottom of member list
        let is_connected = self.call_state == fluxchat_core::rtc::CallConnectionState::Connected;
        let is_connecting = self.call_state == fluxchat_core::rtc::CallConnectionState::Connecting;
        let is_failed = matches!(self.call_state, fluxchat_core::rtc::CallConnectionState::Failed(_));
        let selected_has_call = self.rooms_with_calls.contains(room_id);

        if is_connected {
            // In a call — show room name, participants, controls
            let call_room_name = self.call_room_id.as_ref()
                .and_then(|rid| self.rooms.iter().find(|r| r.room_id == *rid))
                .map(|r| {
                    match r.space_name {
                        Some(ref s) => format!("{s}/{}", r.name),
                        None => r.name.clone(),
                    }
                })
                .unwrap_or_else(|| "Call".into());

            let call_room_members = self.call_room_id.as_ref()
                .and_then(|rid| self.members_by_room.get(rid));

            let mut pip_participants = row![].spacing(3).align_y(iced::Alignment::Center);
            let my_avatar_mxc = call_room_members
                .and_then(|m| m.iter().find(|m| m.user_id == self.user_id))
                .and_then(|m| m.avatar_url.as_deref());
            pip_participants = pip_participants.push(
                container(self.render_avatar(my_avatar_mxc, 18.0)).width(18).height(18),
            );
            for p in self.call_participants.iter().filter(|p| !p.is_local).take(3) {
                let member_avatar = call_room_members
                    .and_then(|members| members.iter().find(|m| p.user_id.starts_with(&m.user_id) || m.user_id == p.user_id))
                    .and_then(|m| m.avatar_url.as_deref());
                let avatar_mxc = p.avatar_url.as_deref().or(member_avatar);
                pip_participants = pip_participants.push(
                    container(self.render_avatar(avatar_mxc, 18.0)).width(18).height(18),
                );
            }

            let mic_icon = if self.call_mic_muted { SVG_MIC_OFF } else { SVG_MIC };
            let mic_color = if self.call_mic_muted { theme::RED } else { theme::GREEN };
            let call_rid = self.call_room_id.clone().unwrap_or_default();

            panel = panel.push(container(
                column![
                    button(
                        row![
                            iced::widget::svg(iced::widget::svg::Handle::from_memory(SVG_PHONE))
                                .width(12).height(12)
                                .style(|_, _| iced::widget::svg::Style { color: Some(theme::GREEN) }),
                            text(call_room_name).font(MONO).size(11).color(theme::GREEN),
                        ].spacing(4).align_y(iced::Alignment::Center)
                    )
                    .on_press(Message::SelectRoom(call_rid))
                    .padding([2, 0])
                    .style(|_, _| button::Style { background: None, text_color: theme::GREEN, ..Default::default() }),
                    pip_participants,
                    row![
                        button(
                            iced::widget::svg(iced::widget::svg::Handle::from_memory(mic_icon))
                                .width(14).height(14)
                                .style(move |_, _| iced::widget::svg::Style { color: Some(mic_color) })
                        )
                            .on_press(Message::CallToggleMic)
                            .padding([4, 8])
                            .style(move |_, _| button::Style {
                                background: Some(theme::BG_LIGHT.into()),
                                border: iced::Border { radius: 12.0.into(), ..Default::default() },
                                ..Default::default()
                            }),
                        button(
                            iced::widget::svg(iced::widget::svg::Handle::from_memory(SVG_PHONE_OFF))
                                .width(14).height(14)
                                .style(|_, _| iced::widget::svg::Style { color: Some(theme::TEXT_BRIGHT) })
                        )
                            .on_press(Message::CallLeave)
                            .padding([4, 8])
                            .style(|_, _| button::Style {
                                background: Some(theme::RED.into()),
                                border: iced::Border { radius: 12.0.into(), ..Default::default() },
                                ..Default::default()
                            }),
                    ].spacing(4),
                ]
                .spacing(4)
                .padding([8, 10]),
            )
            .width(Fill)
            .style(|_: &Theme| container::Style {
                background: Some(theme::BG_MID.into()),
                border: iced::Border { color: theme::GREEN, width: 1.0, ..Default::default() },
                ..Default::default()
            }));
        } else if is_connecting {
            panel = panel.push(container(
                text("Connecting...").font(MONO).size(11).color(theme::ACCENT),
            ).padding([8, 10]).width(Fill).style(|_: &Theme| container::Style {
                background: Some(theme::BG_MID.into()),
                ..Default::default()
            }));
        } else if is_failed {
            let err_msg = if let fluxchat_core::rtc::CallConnectionState::Failed(ref e) = self.call_state {
                if e.contains("M_FORBIDDEN") { "Not allowed" }
                else if e.contains("No LiveKit") || e.contains("rtc_foci") { "Not supported" }
                else { "Call failed" }
            } else { "Error" };
            panel = panel.push(container(
                row![
                    text(err_msg).font(MONO).size(11).color(theme::RED),
                    Space::new().width(Fill),
                    button(
                        iced::widget::svg(iced::widget::svg::Handle::from_memory(SVG_CLOSE))
                            .width(10).height(10)
                            .style(|_, _| iced::widget::svg::Style { color: Some(theme::TEXT_DIM) })
                    )
                    .on_press(Message::CallDisconnected("dismissed".into()))
                    .padding(2)
                    .style(|_, _| button::Style { background: None, ..Default::default() }),
                ].spacing(4).align_y(iced::Alignment::Center),
            ).padding([6, 10]).width(Fill).style(|_: &Theme| container::Style {
                background: Some(Color::from_rgba(0.8, 0.0, 0.0, 0.1).into()),
                border: iced::Border { color: theme::RED, width: 1.0, ..Default::default() },
                ..Default::default()
            }));
        } else if selected_has_call {
            // Room has an active call we can join
            let rid = room_id.to_string();
            panel = panel.push(container(
                row![
                    iced::widget::svg(iced::widget::svg::Handle::from_memory(SVG_PHONE))
                        .width(12).height(12)
                        .style(|_, _| iced::widget::svg::Style { color: Some(theme::GREEN) }),
                    text("Call active").font(MONO).size(11).color(theme::TEXT_NORMAL),
                    Space::new().width(Fill),
                    button(text("Join").font(MONO).size(11).color(theme::BG_DEEP))
                        .on_press(Message::CallJoin(rid))
                        .padding([3, 10])
                        .style(|_, _| button::Style {
                            background: Some(theme::GREEN.into()),
                            text_color: theme::BG_DEEP,
                            border: iced::Border { radius: 4.0.into(), ..Default::default() },
                            ..Default::default()
                        }),
                ].spacing(6).align_y(iced::Alignment::Center),
            ).padding([6, 10]).width(Fill).style(|_: &Theme| container::Style {
                background: Some(theme::BG_MID.into()),
                border: iced::Border { color: theme::GREEN, width: 1.0, ..Default::default() },
                ..Default::default()
            }));
        }

        container(panel)
            .width(200)
            .height(Fill)
            .style(|_: &Theme| container::Style {
                background: Some(theme::BG_DEEP.into()),
                border: iced::Border {
                    color: theme::BORDER,
                    width: 1.0,
                    radius: 0.0.into(),
                },
                ..Default::default()
            })
            .into()
    }

    /// Member list section (Admins, Mods, Members).
    pub(crate) fn view_member_section<'a>(
        &'a self,
        label: &str,
        members: &[&'a fluxchat_core::MemberInfo],
    ) -> Element<'a, Message> {
        let mut col = column![
            container(
                text(format!("{} \u{2014} {}", label.to_uppercase(), members.len()))
                    .font(MONO)
                    .size(11)
                    .color(theme::TEXT_DIM),
            )
            .padding([10.0, 14.0]),
        ]
        .spacing(0);

        for member in members.iter().take(30) {
            col = col.push(self.view_member_row(member));
        }

        col.into()
    }

    /// Single member row.
    pub(crate) fn view_member_row<'a>(&'a self, member: &'a fluxchat_core::MemberInfo) -> Element<'a, Message> {
        let nick_col = theme::nick_color(&member.display_name);
        let presence_color = match member.presence.as_str() {
            "online" => theme::GREEN,
            "idle" => theme::YELLOW,
            _ => Color::TRANSPARENT,
        };
        let user_id = member.user_id.clone();

        // Avatar — use cached image or fallback to initial
        let avatar_img: Element<'a, Message> = self.render_avatar_with_initial(member.avatar_url.as_deref(), &member.display_name, 28.0);

        // Presence dot overlaid on avatar (bottom-right)
        let presence_dot = container(Space::new())
            .width(10).height(10)
            .style(move |_: &Theme| container::Style {
                background: Some(presence_color.into()),
                border: iced::Border {
                    radius: 5.0.into(),
                    color: theme::BG_DEEP,
                    width: 2.0,
                },
                ..Default::default()
            });
        let avatar: Element<'a, Message> = stack![
            container(avatar_img).width(32).height(32),
            container(presence_dot)
                .width(32).height(32)
                .align_x(iced::alignment::Horizontal::Right)
                .align_y(iced::alignment::Vertical::Bottom),
        ].width(32).height(32).into();

        let member_row = row![
            avatar,
            text(&member.display_name)
                .font(MONO)
                .size(13)
                .color(nick_col)
                .wrapping(iced::widget::text::Wrapping::None),
        ]
        .spacing(8)
        .align_y(iced::Alignment::Center);

        button(member_row)
            .on_press(Message::OpenModal(crate::state::Modal::Profile {
                user_id,
            }))
            .padding([4, 12])
            .width(Fill)
            .style(|_, status| button::Style {
                background: if matches!(status, button::Status::Hovered) {
                    Some(Color::from_rgba(1.0, 1.0, 1.0, 0.04).into())
                } else {
                    None
                },
                text_color: theme::TEXT_NORMAL,
                border: iced::Border { radius: 4.0.into(), ..Default::default() },
                ..Default::default()
            })
            .into()
    }
}
