//! The user profile card: banner, identity, status, verification, actions,
//! and (with permission) role / moderation controls for the current room.

use iced::widget::{button, column, container, row, stack, text, Space};
use iced::{Alignment, Background, Border, Color, Element, Fill, Gradient, Length, Theme};

use crate::message::Message;
use crate::state::FluxChat;
use crate::theme;
use crate::ui::{self, Tone};
use crate::view::MONO;

const PROFILE_WIDTH: f32 = 380.0;
const BANNER_HEIGHT: f32 = 92.0;
const AVATAR_SIZE: f32 = 80.0;
/// Ring drawn around the avatar where it overlaps the banner.
const AVATAR_RING: f32 = 5.0;

impl FluxChat {
    /// Member info for a user: the open view's member list first (power
    /// levels are per room), then any other loaded list.
    pub(crate) fn find_member(&self, user_id: &str) -> (Option<&fluxchat_core::MemberInfo>, bool) {
        let in_room = self.selected_room_id.as_ref()
            .and_then(|rid| self.members_by_room.get(rid))
            .and_then(|ms| ms.iter().find(|m| m.user_id == user_id));
        if in_room.is_some() {
            return (in_room, true);
        }
        let elsewhere = self.space_detail_members.iter().find(|m| m.user_id == user_id)
            .or_else(|| self.members_by_room.values().flatten().find(|m| m.user_id == user_id));
        (elsewhere, false)
    }

    pub(crate) fn view_profile_modal<'a>(&'a self, user_id: &'a str) -> Element<'a, Message> {
        let (member, in_room) = self.find_member(user_id);
        let localpart = user_id.trim_start_matches('@').split(':').next().unwrap_or(user_id);
        let display_name = member.map(|m| m.display_name.as_str()).filter(|n| !n.is_empty()).unwrap_or(localpart);
        let nick_col = theme::nick_color(display_name);
        let power = if in_room { member.map(|m| m.power_level).unwrap_or(0) } else { 0 };
        let is_self = user_id == self.user_id;
        let is_ignored = self.ignored_users.contains(user_id);
        let verified = self.profile_user_verified;

        // --- Banner + overlapping avatar ---
        let banner = container(
            row![Space::new().width(Fill), banner_close()].padding(8),
        )
        .width(Fill)
        .height(Length::Fixed(BANNER_HEIGHT))
        .style(move |_: &Theme| container::Style {
            background: Some(Background::Gradient(Gradient::Linear(
                iced::gradient::Linear::new(iced::Radians(2.4))
                    .add_stop(0.0, theme::with_alpha(nick_col, 0.75))
                    .add_stop(1.0, theme::with_alpha(theme::colors().accent, 0.35)),
            ))),
            border: Border {
                radius: iced::border::Radius { top_left: ui::DIALOG_RADIUS, top_right: ui::DIALOG_RADIUS, ..Default::default() },
                ..Default::default()
            },
            ..Default::default()
        });
        let avatar = container(self.render_avatar_with_initial(
            member.and_then(|m| m.avatar_url.as_deref()), display_name, AVATAR_SIZE,
        ))
        .padding(AVATAR_RING)
        .style(|_: &Theme| container::Style {
            background: Some(theme::colors().bg_dark.into()),
            border: Border { radius: ((AVATAR_SIZE / 2.0) + AVATAR_RING).into(), ..Default::default() },
            ..Default::default()
        });
        // Quick actions sit to the right of the avatar, under the banner.
        let mut quick = row![].spacing(6).align_y(Alignment::Center);
        if !is_self {
            quick = quick.push(ui::icon_only(
                crate::icons::SVG_AT_SIGN, "Mention", Tone::Secondary,
                self.selected_room_id.is_some().then(|| Message::MentionUser(display_name.to_string())),
            ));
            quick = quick.push(ui::icon_btn(
                crate::icons::SVG_CHAT, "Message", Tone::Primary,
                Some(Message::SendDmToUser(user_id.to_string())),
            ));
        }
        let top = stack![
            column![banner, Space::new().height(AVATAR_SIZE / 2.0 + AVATAR_RING)],
            container(row![avatar, Space::new().width(Fill), container(quick).padding([0, 0])]
                .align_y(Alignment::End))
                .padding(iced::Padding { top: BANNER_HEIGHT - AVATAR_SIZE / 2.0 - AVATAR_RING, left: 16.0, right: 18.0, bottom: 0.0 })
                .width(Fill),
        ];

        // --- Identity ---
        let mut name_row = row![
            text(display_name).font(MONO).size(19).color(theme::colors().text_bright),
        ]
        .spacing(8)
        .align_y(Alignment::Center);
        if verified == Some(true) {
            name_row = name_row.push(ui::with_tooltip(
                ui::icon(crate::icons::SVG_SHIELD_CHECK, 16.0, theme::colors().green), "Verified",
            ));
        }
        let id_row = row![
            text(user_id).font(MONO).size(12).color(theme::colors().text_dim),
            button(ui::icon(crate::icons::SVG_COPY, 12.0, theme::colors().text_dim))
                .on_press(Message::CopyMessageText(user_id.to_string()))
                .padding(3)
                .style(ui::button_style(Tone::Ghost)),
        ]
        .spacing(4)
        .align_y(Alignment::Center);

        let mut identity = column![name_row, id_row].spacing(2);

        // Presence + status message: the server's answer when it arrives,
        // else the member list's. Nothing is shown if neither knows.
        let fresh = self.profile_presence.as_ref();
        let (state, status_msg, last_active) = match fresh {
            Some(Some(p)) => (Some(p.state.as_str()), p.status_msg.as_deref(), p.last_active_secs),
            _ => match member {
                Some(m) if m.presence != "offline" || m.last_active_secs.is_some() =>
                    (Some(m.presence.as_str()), m.status_msg.as_deref(), m.last_active_secs),
                Some(m) => (None, m.status_msg.as_deref(), None),
                None => (None, None, None),
            },
        };
        if state.is_some() || status_msg.is_some_and(|s| !s.is_empty()) {
            let mut presence = row![].spacing(6).align_y(Alignment::Center);
            if let Some(state) = state {
                let (dot, label) = match state {
                    "online" => (theme::colors().green, "Online".to_string()),
                    "idle" => (theme::colors().yellow, "Away".to_string()),
                    _ => (theme::colors().text_dim, match last_active {
                        Some(secs) => format!("Offline \u{00b7} last seen {}", ago(secs)),
                        None => "Offline".to_string(),
                    }),
                };
                presence = presence
                    .push(container(Space::new().width(8).height(8)).style(move |_: &Theme| container::Style {
                        background: Some(dot.into()),
                        border: Border { radius: 4.0.into(), ..Default::default() },
                        ..Default::default()
                    }))
                    .push(text(label).font(MONO).size(12).color(theme::colors().text_muted));
            }
            if let Some(status) = status_msg.filter(|s| !s.is_empty()) {
                if state.is_some() {
                    presence = presence.push(text("\u{2014}").font(MONO).size(12).color(theme::colors().text_dim));
                }
                presence = presence.push(
                    text(status).font(MONO).size(12).color(theme::colors().text_normal)
                        .wrapping(text::Wrapping::WordOrGlyph),
                );
            }
            identity = identity.push(Space::new().height(6));
            identity = identity.push(presence);
        }

        // Badges
        let mut badges = row![].spacing(6);
        let mut any_badge = false;
        if in_room {
            if power >= 100 {
                badges = badges.push(ui::badge("Admin", theme::colors().yellow));
                any_badge = true;
            } else if power >= 50 {
                badges = badges.push(ui::badge("Moderator", theme::colors().accent));
                any_badge = true;
            }
        }
        if is_ignored {
            badges = badges.push(ui::badge("Ignored", theme::colors().red));
            any_badge = true;
        }
        if any_badge {
            identity = identity.push(Space::new().height(4));
            identity = identity.push(badges);
        }

        let mut body = column![identity].spacing(ui::GAP);

        // --- Verification ---
        if !is_self {
            let (icon_svg, color, title, detail) = match verified {
                Some(true) => (crate::icons::SVG_SHIELD_CHECK, theme::colors().green, "Verified",
                    "Messages from their devices are trusted."),
                Some(false) => (crate::icons::SVG_SHIELD, theme::colors().yellow, "Not verified",
                    "Compare emoji to confirm their identity."),
                None => (crate::icons::SVG_SHIELD, theme::colors().text_dim, "Checking\u{2026}", ""),
            };
            let mut sec = column![
                row![
                    ui::icon(icon_svg, 18.0, color),
                    column![
                        text(title).font(MONO).size(13).color(color),
                        text(detail).font(MONO).size(11).color(theme::colors().text_dim)
                            .wrapping(text::Wrapping::WordOrGlyph),
                    ].spacing(2).width(Fill),
                ]
                .spacing(10)
                .align_y(Alignment::Center),
            ]
            .spacing(10);
            if verified == Some(false) {
                sec = sec.push(
                    row![
                        ui::btn("Verify", Tone::Secondary, Some(Message::StartVerification(user_id.to_string())))
                            .width(Fill),
                        ui::with_tooltip(
                            ui::btn("Trust manually", Tone::Ghost, Some(Message::ManualVerifyUser(user_id.to_string()))),
                            "Sign their identity without an emoji check (for bots)",
                        ),
                    ]
                    .spacing(8),
                );
            }
            body = body.push(ui::card(sec));
        }

        // --- Moderation (current room, with permission) ---
        if in_room && !is_self {
            if let Some(section) = self.view_profile_moderation(user_id, power) {
                body = body.push(section);
            }
        }

        // --- Footer ---
        if !is_self {
            body = body.push(ui::divider());
            let ignore = if is_ignored {
                ui::icon_btn(crate::icons::SVG_EYE_OFF, "Unignore", Tone::Ghost, Some(Message::ToggleIgnoreUser(user_id.to_string())))
            } else {
                ui::icon_btn(crate::icons::SVG_EYE_OFF, "Ignore", Tone::Ghost, Some(Message::ToggleIgnoreUser(user_id.to_string())))
            };
            body = body.push(row![
                ignore,
                Space::new().width(Fill),
                text(member.map(|m| m.server_name.as_str()).unwrap_or("")).font(MONO).size(11).color(theme::colors().text_dim),
            ].align_y(Alignment::Center));
        }

        column![
            top,
            container(body).padding(iced::Padding { top: 10.0, right: 20.0, bottom: 18.0, left: 20.0 }),
        ]
        .width(Length::Fixed(PROFILE_WIDTH))
        .into()
    }

    /// Role selector and kick/ban for the current room, if we outrank them.
    fn view_profile_moderation<'a>(&'a self, user_id: &'a str, power: i64) -> Option<Element<'a, Message>> {
        let room_id = self.active_real_room()?.to_string();
        let my_power = self.find_member(&self.user_id).0.map(|m| m.power_level).unwrap_or(0);
        if my_power <= power || my_power < 50 {
            return None;
        }
        let room_name = self.rooms.iter().find(|r| r.room_id == room_id)
            .map(|r| r.name.as_str()).unwrap_or("this room");

        // Role presets (levels above our own are disabled).
        let seg = ui::segmented(
            [("Member", 0i64), ("Moderator", 50), ("Admin", 100)].into_iter().map(|(label, level)| {
                let allowed = level < my_power || (level == my_power && my_power == 100);
                (
                    label.to_string(),
                    allowed.then(|| Message::SetPowerLevel(room_id.clone(), user_id.to_string(), level)),
                    power == level,
                )
            }).collect(),
        );
        let custom = row![
            text("Custom level").font(MONO).size(12).color(theme::colors().text_muted).width(Fill),
            ui::input("0\u{2013}100", &self.profile_power_input, Message::ProfilePowerInputChanged)
                .on_submit(Message::ProfileSetPowerLevel)
                .width(Length::Fixed(72.0)),
            ui::btn("Set", Tone::Secondary, Some(Message::ProfileSetPowerLevel)),
        ]
        .spacing(8)
        .align_y(Alignment::Center);

        let actions: Element<'a, Message> = match self.profile_confirm.as_deref() {
            Some(action @ ("kick" | "ban")) => {
                let verb = if action == "kick" { "Kick" } else { "Ban" };
                let msg = if action == "kick" {
                    Message::KickUser(room_id.clone(), user_id.to_string(), None)
                } else {
                    Message::BanUser(room_id.clone(), user_id.to_string(), None)
                };
                row![
                    text(format!("{verb} from {room_name}?")).font(MONO).size(12).color(theme::colors().text_normal).width(Fill),
                    ui::btn("Cancel", Tone::Ghost, Some(Message::ProfileConfirm(None))),
                    ui::btn(verb, Tone::Danger, Some(msg)),
                ]
                .spacing(6)
                .align_y(Alignment::Center)
                .into()
            }
            _ => row![
                ui::icon_btn(crate::icons::SVG_USER_MINUS, "Kick", Tone::Danger, Some(Message::ProfileConfirm(Some("kick".into()))))
                    .width(Fill),
                ui::icon_btn(crate::icons::SVG_BAN, "Ban", Tone::Danger, Some(Message::ProfileConfirm(Some("ban".into()))))
                    .width(Fill),
            ]
            .spacing(8)
            .into(),
        };

        Some(
            ui::card(
                column![
                    row![
                        ui::section_label("Role"),
                        Space::new().width(Fill),
                        text(format!("power {power}")).font(MONO).size(11).color(theme::colors().text_dim),
                    ],
                    seg,
                    custom,
                    ui::divider(),
                    actions,
                ]
                .spacing(10),
            )
            .into(),
        )
    }
}

/// Close button drawn on the banner (light icon on a dark hover).
fn banner_close<'a>() -> Element<'a, Message> {
    button(ui::icon(crate::icons::SVG_CLOSE, 16.0, Color::from_rgba(1.0, 1.0, 1.0, 0.85)))
        .on_press(Message::CloseModal)
        .padding(6)
        .style(|_, status| button::Style {
            background: Some(Color::from_rgba(0.0, 0.0, 0.0, if matches!(status, button::Status::Hovered) { 0.45 } else { 0.25 }).into()),
            border: Border { radius: 14.0.into(), ..Default::default() },
            ..Default::default()
        })
        .into()
}

/// "just now", "5m ago", "3h ago", "2d ago".
fn ago(secs: u64) -> String {
    match secs {
        0..=59 => "just now".into(),
        60..=3599 => format!("{}m ago", secs / 60),
        3600..=86399 => format!("{}h ago", secs / 3600),
        _ => format!("{}d ago", secs / 86400),
    }
}
