use iced::widget::{
    button, column, container, row, scrollable, stack,
    text, Space,
};
use iced::{Color, Element, Fill, Theme};

use crate::icons::*;
use crate::message::Message;
use crate::state::FluxChat;
use crate::theme;
use crate::view::thin_scrollbar;

/// Most avatars shown on the hidden member list's rail (then "+N").
const MEMBER_RAIL_MAX: usize = 40;

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
                    .font(self.sidebar_font())
                    .size(14)
                    .color(theme::colors().text_bright),
                Space::new().width(Fill),
                button(
                    iced::widget::svg(invite_handle)
                        .width(14).height(14)
                        .style(|_, _| iced::widget::svg::Style { color: Some(theme::colors().text_dim) }),
                )
                .on_press(Message::OpenModal(crate::state::Modal::Invite))
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
                crate::ui::tooltip_at(
                    button(crate::ui::icon(crate::icons::SVG_PANEL_RIGHT_CLOSE, 14.0, theme::colors().text_dim))
                        .on_press(Message::ToggleMemberList)
                        .padding([2, 4])
                        .style(crate::ui::button_style(crate::ui::Tone::Ghost)),
                    if cfg!(target_os = "macos") { "Hide users (\u{2318}])" } else { "Hide users (Ctrl+])" },
                    iced::widget::tooltip::Position::Bottom,
                ),
            ]
            .spacing(2)
            .align_y(iced::Alignment::Center),
        )
        .padding([8, 12])
        .width(Fill);

        let mut col = column![header].spacing(0);

        // Filter box — big rooms are unusable without one
        col = col.push(
            container(
                iced::widget::text_input("Filter members…", &self.member_filter)
                    .on_input(Message::MemberFilterChanged)
                    .font(self.sidebar_font())
                    .size(12)
                    .padding([4, 8]),
            )
            .padding([0, 8])
            .width(Fill),
        );

        if let Some(members) = members {
            let filter = self.member_filter.trim().to_lowercase();
            let matches = |m: &&fluxchat_core::MemberInfo| {
                filter.is_empty()
                    || m.display_name.to_lowercase().contains(&filter)
                    || m.user_id.to_lowercase().contains(&filter)
            };
            // Group by role
            let admins: Vec<_> = members.iter().filter(|m| m.role == "Admin").filter(matches).collect();
            let mods: Vec<_> = members.iter().filter(|m| m.role == "Moderator").filter(matches).collect();
            let regular: Vec<_> = members.iter().filter(|m| m.role == "Member").filter(matches).collect();

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
            let mic_color = if self.call_mic_muted { theme::colors().red } else { theme::colors().green };
            let call_rid = self.call_room_id.clone().unwrap_or_default();

            panel = panel.push(container(
                column![
                    button(
                        row![
                            iced::widget::svg(iced::widget::svg::Handle::from_memory(SVG_PHONE))
                                .width(12).height(12)
                                .style(|_, _| iced::widget::svg::Style { color: Some(theme::colors().green) }),
                            text(call_room_name).font(self.sidebar_font()).size(11).color(theme::colors().green),
                        ].spacing(4).align_y(iced::Alignment::Center)
                    )
                    .on_press(Message::SelectRoom(call_rid))
                    .padding([2, 0])
                    .style(|_, _| button::Style { background: None, text_color: theme::colors().green, ..Default::default() }),
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
                                background: Some(theme::colors().bg_light.into()),
                                border: iced::Border { radius: 12.0.into(), ..Default::default() },
                                ..Default::default()
                            }),
                        button(
                            iced::widget::svg(iced::widget::svg::Handle::from_memory(SVG_PHONE_OFF))
                                .width(14).height(14)
                                .style(|_, _| iced::widget::svg::Style { color: Some(theme::colors().text_bright) })
                        )
                            .on_press(Message::CallLeave)
                            .padding([4, 8])
                            .style(|_, _| button::Style {
                                background: Some(theme::colors().red.into()),
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
                background: Some(theme::colors().bg_mid.into()),
                border: iced::Border { color: theme::colors().green, width: 1.0, ..Default::default() },
                ..Default::default()
            }));
        } else if is_connecting {
            panel = panel.push(container(
                text("Connecting...").font(self.sidebar_font()).size(11).color(theme::colors().accent),
            ).padding([8, 10]).width(Fill).style(|_: &Theme| container::Style {
                background: Some(theme::colors().bg_mid.into()),
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
                    text(err_msg).font(self.sidebar_font()).size(11).color(theme::colors().red),
                    Space::new().width(Fill),
                    button(
                        iced::widget::svg(iced::widget::svg::Handle::from_memory(SVG_CLOSE))
                            .width(10).height(10)
                            .style(|_, _| iced::widget::svg::Style { color: Some(theme::colors().text_dim) })
                    )
                    .on_press(Message::CallDisconnected("dismissed".into()))
                    .padding(2)
                    .style(|_, _| button::Style { background: None, ..Default::default() }),
                ].spacing(4).align_y(iced::Alignment::Center),
            ).padding([6, 10]).width(Fill).style(|_: &Theme| container::Style {
                background: Some(Color::from_rgba(0.8, 0.0, 0.0, 0.1).into()),
                border: iced::Border { color: theme::colors().red, width: 1.0, ..Default::default() },
                ..Default::default()
            }));
        } else if selected_has_call {
            // Room has an active call we can join
            let rid = room_id.to_string();
            panel = panel.push(container(
                row![
                    iced::widget::svg(iced::widget::svg::Handle::from_memory(SVG_PHONE))
                        .width(12).height(12)
                        .style(|_, _| iced::widget::svg::Style { color: Some(theme::colors().green) }),
                    text("Call active").font(self.sidebar_font()).size(11).color(theme::colors().text_normal),
                    Space::new().width(Fill),
                    button(text("Join").font(self.sidebar_font()).size(11).color(theme::colors().bg_deep))
                        .on_press(Message::CallJoin(rid))
                        .padding([3, 10])
                        .style(|_, _| button::Style {
                            background: Some(theme::colors().green.into()),
                            text_color: theme::colors().bg_deep,
                            border: iced::Border { radius: 4.0.into(), ..Default::default() },
                            ..Default::default()
                        }),
                ].spacing(6).align_y(iced::Alignment::Center),
            ).padding([6, 10]).width(Fill).style(|_: &Theme| container::Style {
                background: Some(theme::colors().bg_mid.into()),
                border: iced::Border { color: theme::colors().green, width: 1.0, ..Default::default() },
                ..Default::default()
            }));
        }

        container(panel)
            .width(self.member_list_width)
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

    /// Member list section (Admins, Mods, Members).
    pub(crate) fn view_member_section<'a>(
        &'a self,
        label: &str,
        members: &[&'a fluxchat_core::MemberInfo],
    ) -> Element<'a, Message> {
        let mut col = column![
            container(
                text(format!("{} \u{2014} {}", label.to_uppercase(), members.len()))
                    .font(self.sidebar_font())
                    .size(11)
                    .color(theme::colors().text_dim),
            )
            .padding([10.0, 14.0]),
        ]
        .spacing(0);

        for member in members.iter().take(30) {
            col = col.push(self.view_member_row(member));
        }

        col.into()
    }

    /// A member's avatar (or tinted initial) in a `box_size` square, with
    /// the presence dot at the bottom right.
    fn member_avatar<'a>(&'a self, member: &'a fluxchat_core::MemberInfo, size: f32, box_size: f32) -> Element<'a, Message> {
        let presence_color = match member.presence.as_str() {
            "online" => theme::colors().green,
            "idle" => theme::colors().yellow,
            _ => Color::TRANSPARENT,
        };
        let dot = (box_size * 0.3).round();
        let presence_dot = container(Space::new())
            .width(dot).height(dot)
            .style(move |_: &Theme| container::Style {
                background: Some(presence_color.into()),
                border: iced::Border {
                    radius: (dot / 2.0).into(),
                    color: theme::colors().bg_deep,
                    width: 2.0,
                },
                ..Default::default()
            });
        stack![
            container(self.render_avatar_with_initial(member.avatar_url.as_deref(), &member.display_name, size))
                .width(box_size).height(box_size),
            container(presence_dot)
                .width(box_size).height(box_size)
                .align_x(iced::alignment::Horizontal::Right)
                .align_y(iced::alignment::Vertical::Bottom),
        ].width(box_size).height(box_size).into()
    }

    /// The hidden member list's rail: restore button, then the members as
    /// small avatars (same order as the list) — hover for the name, click
    /// for the profile.
    pub(crate) fn view_member_rail(&self) -> Element<'_, Message> {
        let tip = if cfg!(target_os = "macos") { "Show users (\u{2318}])" } else { "Show users (Ctrl+])" };
        let restore = crate::ui::tooltip_at(
            button(crate::ui::icon(crate::icons::SVG_PANEL_RIGHT_OPEN, 18.0, theme::colors().text_muted))
                .on_press(Message::ToggleMemberList)
                .padding(6)
                .style(crate::ui::button_style(crate::ui::Tone::Ghost)),
            tip,
            iced::widget::tooltip::Position::Left,
        );
        let members = self.selected_room_id.as_ref().and_then(|r| self.members_by_room.get(r));
        let mut avatars = column![].spacing(6).align_x(iced::Alignment::Center);
        if let Some(members) = members {
            // Same order as the list: admins, moderators, members.
            let ordered = ["Admin", "Moderator", "Member"].into_iter()
                .flat_map(|role| members.iter().filter(move |m| m.role == role));
            for member in ordered.clone().take(MEMBER_RAIL_MAX) {
                let label = match member.role.as_str() {
                    "Admin" | "Moderator" => format!("{} \u{00b7} {}", member.display_name, member.role),
                    _ => member.display_name.clone(),
                };
                avatars = avatars.push(crate::ui::tooltip_at(
                    button(self.member_avatar(member, 24.0, 26.0))
                        .on_press(Message::OpenModal(crate::state::Modal::Profile { user_id: member.user_id.clone() }))
                        .padding(2)
                        .style(crate::ui::button_style(crate::ui::Tone::Ghost)),
                    label,
                    iced::widget::tooltip::Position::Left,
                ));
            }
            let more = members.len().saturating_sub(MEMBER_RAIL_MAX);
            if more > 0 {
                avatars = avatars.push(crate::ui::tooltip_at(
                    button(text(format!("+{more}")).font(self.sidebar_font()).size(11).color(theme::colors().text_dim))
                        .on_press(Message::ToggleMemberList)
                        .padding([4, 2])
                        .style(crate::ui::button_style(crate::ui::Tone::Ghost)),
                    "Show everyone",
                    iced::widget::tooltip::Position::Left,
                ));
            }
        }
        container(
            column![
                restore,
                container(crate::ui::divider()).width(22).padding([6, 0]),
                scrollable(avatars.width(Fill))
                    .height(Fill)
                    .direction(iced::widget::scrollable::Direction::Vertical(
                        iced::widget::scrollable::Scrollbar::new().width(0).scroller_width(0),
                    )),
            ]
            .align_x(iced::Alignment::Center),
        )
        .width(crate::view::MEMBER_RAIL_WIDTH)
        .height(Fill)
        .padding(iced::Padding { top: 8.0, right: 0.0, bottom: 8.0, left: 0.0 })
        .style(|_: &Theme| container::Style {
            background: Some(theme::colors().bg_deep.into()),
            border: iced::Border { color: theme::colors().border, width: 1.0, radius: 0.0.into() },
            ..Default::default()
        })
        .into()
    }

    /// Single member row.
    pub(crate) fn view_member_row<'a>(&'a self, member: &'a fluxchat_core::MemberInfo) -> Element<'a, Message> {
        let nick_col = theme::nick_color(&member.display_name);
        let user_id = member.user_id.clone();
        let avatar = self.member_avatar(member, 28.0, 32.0);

        let member_row = row![
            avatar,
            text(&member.display_name)
                .font(self.sidebar_font())
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
                    Some(theme::tint(0.04).into())
                } else {
                    None
                },
                text_color: theme::colors().text_normal,
                border: iced::Border { radius: 4.0.into(), ..Default::default() },
                ..Default::default()
            })
            .into()
    }
}
