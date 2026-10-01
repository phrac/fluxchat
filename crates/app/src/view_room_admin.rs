//! Room settings → Security & Privacy and Roles & Permissions tabs.

use iced::widget::{button, column, container, row, text, Space};
use iced::{Alignment, Border, Element, Fill, Length, Theme};

use fluxchat_core::room_admin::{HistoryMode, JoinMode, Permission};

use crate::message::Message;
use crate::state::FluxChat;
use crate::theme;
use crate::ui::{self, Tone};
use crate::view::MONO;

/// A selectable option card: radio dot, title, description.
fn choice<'a>(title: impl Into<String>, detail: impl Into<String>, selected: bool, on_press: Option<Message>) -> Element<'a, Message> {
    let (title, detail) = (title.into(), detail.into());
    let c = theme::colors();
    let dot = container(
        container(Space::new().width(8).height(8)).style(move |_: &Theme| container::Style {
            background: selected.then(|| theme::colors().accent.into()),
            border: Border { radius: 4.0.into(), ..Default::default() },
            ..Default::default()
        }),
    )
    .width(16)
    .height(16)
    .center_x(16)
    .center_y(16)
    .style(move |_: &Theme| container::Style {
        border: Border {
            color: if selected { theme::colors().accent } else { theme::colors().text_dim },
            width: 1.5,
            radius: 8.0.into(),
        },
        ..Default::default()
    });
    let enabled = on_press.is_some();
    button(
        row![
            dot,
            column![
                text(title).font(MONO).size(13).color(if enabled || selected { c.text_normal } else { c.text_dim }),
                text(detail).font(MONO).size(11).color(c.text_dim).wrapping(text::Wrapping::WordOrGlyph),
            ]
            .spacing(2)
            .width(Fill),
        ]
        .spacing(12)
        .align_y(Alignment::Center),
    )
    .on_press_maybe(if selected { None } else { on_press })
    .padding([8, 10])
    .width(Fill)
    .style(move |_, status| button::Style {
        background: Some(if selected {
            theme::with_alpha(theme::colors().accent, 0.1)
        } else if matches!(status, button::Status::Hovered) {
            theme::tint(0.05)
        } else {
            iced::Color::TRANSPARENT
        }.into()),
        border: Border {
            color: if selected { theme::with_alpha(theme::colors().accent, 0.5) } else { theme::colors().border },
            width: 1.0,
            radius: ui::RADIUS.into(),
        },
        ..Default::default()
    })
    .into()
}

/// Display name of a power level.
fn role_name(level: i64) -> String {
    match level {
        i64::MAX => "Owner".into(),
        100.. => "Admin".into(),
        50..=99 => "Moderator".into(),
        0 => "Member".into(),
        n => format!("Custom ({n})"),
    }
}

/// An entry in a level picker.
#[derive(Debug, Clone, PartialEq)]
struct LevelChoice(i64);

impl std::fmt::Display for LevelChoice {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self.0 {
            100 => f.write_str("Admin (100)"),
            50 => f.write_str("Moderator (50)"),
            0 => f.write_str("Member (0)"),
            n => write!(f, "Custom ({n})"),
        }
    }
}

/// Permissions shown in the Roles tab: (label, permission). Event types
/// fall back to the state/event defaults when not set explicitly.
fn permission_rows(is_space: bool) -> Vec<(&'static str, Permission)> {
    let ev = |t: &str| Permission::Event(t.to_string());
    let mut rows = vec![
        ("Default role for new members", Permission::UsersDefault),
        ("Send messages", ev("m.room.message")),
        ("Invite people", Permission::Invite),
        ("Remove people", Permission::Kick),
        ("Ban people", Permission::Ban),
        ("Delete others' messages", Permission::Redact),
        ("Notify everyone (@room)", Permission::NotifyRoom),
        ("Pin messages", ev("m.room.pinned_events")),
        ("Change the name", ev("m.room.name")),
        ("Change the topic", ev("m.room.topic")),
        ("Change the icon", ev("m.room.avatar")),
        ("Change the main address", ev("m.room.canonical_alias")),
        ("Change history visibility", ev("m.room.history_visibility")),
        ("Change permissions", ev("m.room.power_levels")),
        ("Enable encryption", ev("m.room.encryption")),
        ("Upgrade the room", ev("m.room.tombstone")),
        ("Other settings", Permission::StateDefault),
    ];
    if is_space {
        rows.insert(1, ("Add and remove rooms", ev("m.space.child")));
    }
    rows
}

impl FluxChat {
    pub(crate) fn view_room_security_tab(&self, room_id: &str) -> Element<'_, Message> {
        let Some(sec) = &self.room_security else {
            return ui::empty_state(crate::icons::SVG_CLOCK, "Loading\u{2026}", None);
        };
        let in_space = self.room_parent_spaces.as_ref().is_some_and(|p| !p.is_empty());
        let space_names: Vec<String> = self.room_parent_spaces.iter().flatten()
            .filter_map(|id| self.spaces.iter().find(|s| s.room_id == *id).map(|s| s.name.clone()))
            .collect();
        let space_detail = if space_names.is_empty() {
            "Add this room to a space first".to_string()
        } else {
            format!("Anyone in {} can find and join", space_names.join(", "))
        };
        let join = |mode: JoinMode, enabled: bool| sec.can_change_join.then_some(Message::SetJoinMode(mode)).filter(|_| enabled);

        let access = column![
            choice("Private (invite only)", "Only people who are invited can join",
                sec.join == JoinMode::Invite, join(JoinMode::Invite, true)),
            choice("Ask to join", "Anyone can request to join; members approve",
                sec.join == JoinMode::Knock, join(JoinMode::Knock, true)),
            choice("Space members", space_detail,
                sec.join == JoinMode::SpaceMembers, join(JoinMode::SpaceMembers, in_space)),
            choice("Public", "Anyone can find and join",
                sec.join == JoinMode::Public, join(JoinMode::Public, true)),
        ]
        .spacing(6);

        let mut col = column![ui::field("Who can join", access)].spacing(ui::GAP + 4.0);
        if !sec.can_change_join {
            col = col.push(ui::hint("You don't have permission to change who can join."));
        }

        if sec.join == JoinMode::Public {
            col = col.push(ui::card(column![
                ui::toggle_row("List in the room directory", "Let people find it in Explore rooms",
                    sec.in_directory, Message::SetInDirectory),
                ui::divider(),
                ui::toggle_row("Allow guests", "Guest accounts can join without registering",
                    sec.guest_access, Message::SetGuestAccess),
            ].spacing(10)));
        }

        let hist = |mode: HistoryMode| sec.can_change_history.then_some(Message::SetHistoryMode(mode));
        let mut history = column![].spacing(6);
        if sec.join == JoinMode::Public || sec.history == HistoryMode::WorldReadable {
            history = history.push(choice("Anyone", "Readable without joining",
                sec.history == HistoryMode::WorldReadable, hist(HistoryMode::WorldReadable)));
        }
        history = history
            .push(choice("Members (all history)", "New members can read messages from before they joined",
                sec.history == HistoryMode::Shared, hist(HistoryMode::Shared)))
            .push(choice("Members, since they were invited", "Messages from before their invite stay hidden",
                sec.history == HistoryMode::Invited, hist(HistoryMode::Invited)))
            .push(choice("Members, since they joined", "Messages from before they joined stay hidden",
                sec.history == HistoryMode::Joined, hist(HistoryMode::Joined)));
        col = col.push(ui::field("Who can read history", history));
        col = col.push(ui::hint("History changes only apply to future messages."));

        let encryption: Element<Message> = if sec.encrypted {
            row![
                ui::icon(crate::icons::SVG_LOCK, 16.0, theme::colors().green),
                column![
                    text("End-to-end encrypted").font(MONO).size(13).color(theme::colors().text_normal),
                    ui::hint("Messages can only be read by members' devices. This can't be turned off."),
                ].spacing(2).width(Fill),
            ].spacing(10).align_y(Alignment::Center).into()
        } else {
            row![
                ui::icon(crate::icons::SVG_LOCK, 16.0, theme::colors().text_dim),
                column![
                    text("Not encrypted").font(MONO).size(13).color(theme::colors().text_normal),
                    ui::hint("Once enabled, encryption can't be turned off. Bots and bridges may stop working."),
                ].spacing(2).width(Fill),
                ui::btn("Enable", Tone::Secondary,
                    sec.can_enable_encryption.then(|| Message::EnableEncryption(room_id.to_string()))),
            ].spacing(10).align_y(Alignment::Center).into()
        };
        col = col.push(ui::field("Encryption", ui::card(encryption)));
        col.into()
    }

    pub(crate) fn view_room_roles_tab(&self, room_id: &str) -> Element<'_, Message> {
        let Some((perms, bans)) = &self.room_perms else {
            return ui::empty_state(crate::icons::SVG_CLOCK, "Loading\u{2026}", None);
        };
        let is_space = self.spaces.iter().any(|s| s.room_id == room_id);
        let mut col = column![].spacing(ui::GAP + 4.0);

        // --- Privileged users ---
        let mut people = column![].spacing(2);
        let privileged: Vec<_> = perms.users.iter()
            .filter(|(_, _, level)| *level != perms.level(&Permission::UsersDefault))
            .collect();
        if privileged.is_empty() {
            people = people.push(ui::hint("Nobody has a special role."));
        }
        for (uid, name, level) in privileged {
            let badge_color = match *level {
                100.. => theme::colors().yellow,
                50..=99 => theme::colors().accent,
                _ => theme::colors().text_muted,
            };
            let member_avatar = self.find_member(uid).0.and_then(|m| m.avatar_url.as_deref());
            let can_change = perms.can_edit && *level < perms.my_level && *uid != self.user_id;
            let mut r = row![
                self.render_avatar_with_initial(member_avatar, name, 26.0),
                column![
                    text(name.as_str()).font(MONO).size(13).color(theme::nick_color(name)),
                    text(uid.as_str()).font(MONO).size(11).color(theme::colors().text_dim),
                ].spacing(1).width(Fill),
                ui::badge(role_name(*level), badge_color),
            ]
            .spacing(10)
            .align_y(Alignment::Center);
            if can_change {
                r = r.push(ui::with_tooltip(
                    button(ui::icon(crate::icons::SVG_USER_MINUS, 14.0, theme::colors().text_dim))
                        .on_press(Message::SetPowerLevel(room_id.to_string(), uid.clone(), perms.level(&Permission::UsersDefault)))
                        .padding(6)
                        .style(ui::button_style(Tone::Ghost)),
                    "Reset to member",
                ));
            }
            people = people.push(container(r).padding([4, 6]));
        }
        col = col.push(ui::field("People with roles", column![
            people,
            ui::hint("To give someone a role, open their profile from the member list."),
        ].spacing(6)));

        // --- Banned ---
        if !bans.is_empty() {
            let can_unban = perms.my_level >= perms.level(&Permission::Ban);
            let mut list = column![].spacing(2);
            for b in bans {
                let mut r = row![
                    column![
                        text(b.display_name.as_str()).font(MONO).size(13).color(theme::colors().text_normal),
                        text(b.reason.as_deref().map(|r| format!("{} \u{00b7} {r}", b.user_id)).unwrap_or_else(|| b.user_id.clone()))
                            .font(MONO).size(11).color(theme::colors().text_dim),
                    ].spacing(1).width(Fill),
                ]
                .spacing(10)
                .align_y(Alignment::Center);
                if can_unban {
                    r = r.push(ui::btn("Unban", Tone::Secondary, Some(Message::UnbanUser(b.user_id.clone()))));
                }
                list = list.push(container(r).padding([4, 6]));
            }
            col = col.push(ui::field(format!("Banned \u{2014} {}", bans.len()), list));
        }

        // --- Permissions ---
        let mut grid = column![].spacing(8);
        for (label, permission) in permission_rows(is_space) {
            let level = perms.level(&permission);
            let mut options = vec![LevelChoice(0), LevelChoice(50), LevelChoice(100)];
            if !options.contains(&LevelChoice(level)) {
                options.push(LevelChoice(level));
            }
            // We can only set levels up to our own, and only change ones we reach.
            let editable = perms.can_edit && level <= perms.my_level;
            let my_level = perms.my_level;
            options.retain(|o| o.0 <= my_level || o.0 == level);
            let control: Element<Message> = if editable {
                let p = permission.clone();
                iced::widget::pick_list(options, Some(LevelChoice(level)), move |c: LevelChoice| {
                    Message::SetPermission(p.clone(), c.0)
                })
                .font(MONO)
                .text_size(12)
                .padding([5, 8])
                .width(Length::Fixed(170.0))
                .style(ui::pick_list_style)
                .into()
            } else {
                container(text(LevelChoice(level).to_string()).font(MONO).size(12).color(theme::colors().text_muted))
                    .width(Length::Fixed(170.0))
                    .padding([5, 8])
                    .into()
            };
            grid = grid.push(
                row![
                    text(label).font(MONO).size(13).color(theme::colors().text_normal).width(Fill),
                    control,
                ]
                .spacing(12)
                .align_y(Alignment::Center),
            );
        }
        let perm_hint = if perms.can_edit {
            "Minimum role needed for each action."
        } else {
            "Minimum role needed for each action. You can't change these."
        };
        col = col.push(ui::field("Permissions", column![ui::hint(perm_hint), ui::card(grid)].spacing(8)));
        col.into()
    }

    pub(crate) fn view_room_advanced_tab(&self, room_id: &str) -> Element<'_, Message> {
        let c = theme::colors();
        let id_row = row![
            text(room_id.to_string()).font(MONO).size(12).color(c.text_muted).width(Fill),
            ui::icon_only(crate::icons::SVG_COPY, "Copy room ID", Tone::Ghost,
                Some(Message::CopyMessageText(room_id.to_string()))),
        ]
        .spacing(8)
        .align_y(Alignment::Center);
        let mut col = column![ui::field("Room ID", id_row)].spacing(ui::GAP + 4.0);
        let Some(adv) = &self.room_advanced else {
            return col.push(ui::empty_state(crate::icons::SVG_CLOCK, "Loading\u{2026}", None)).into();
        };

        // --- Version & upgrade ---
        let mut version = column![
            row![
                text(format!("Room version {}", adv.version)).font(MONO).size(13).color(c.text_normal).width(Fill),
            ]
            .align_y(Alignment::Center),
        ]
        .spacing(10);
        match (&adv.upgrade_to, self.room_upgrade_confirm) {
            (Some(to), false) => {
                version = version.push(row![
                    ui::hint(format!("Your server makes new rooms with version {to}.")),
                    Space::new().width(Fill),
                    ui::btn(format!("Upgrade to {to}"), Tone::Secondary,
                        adv.can_upgrade.then_some(Message::ConfirmRoomUpgrade(true))),
                ].spacing(10).align_y(Alignment::Center));
                if !adv.can_upgrade {
                    version = version.push(ui::hint("You don't have permission to upgrade this room."));
                }
            }
            (Some(to), true) => {
                version = version.push(column![
                    text("Upgrade this room?").font(MONO).size(13).color(c.yellow),
                    ui::hint("This makes a new room and closes this one: members see a link to move over, \
                        history stays here, and bridges may need to be set up again. It can't be undone."),
                    ui::footer(vec![
                        ui::btn("Cancel", Tone::Ghost, (!self.room_upgrading).then_some(Message::ConfirmRoomUpgrade(false))).into(),
                        ui::btn(if self.room_upgrading { "Upgrading\u{2026}".to_string() } else { format!("Upgrade to {to}") },
                            Tone::Danger, (!self.room_upgrading).then(|| Message::UpgradeRoom(to.clone()))).into(),
                    ]),
                ].spacing(8));
            }
            (None, _) => version = version.push(ui::hint("This is the version your server uses for new rooms.")),
        }
        let link = |room: &str, label: &'static str, detail: &'static str| -> Element<'_, Message> {
            // Room IDs before v12 name a server that's in the room.
            let via: Vec<String> = room.split_once(':').map(|(_, s)| vec![s.to_string()]).unwrap_or_default();
            row![
                column![
                    text(label).font(MONO).size(13).color(c.text_normal),
                    ui::hint(detail),
                ].spacing(2).width(Fill),
                ui::btn("Open", Tone::Secondary, Some(Message::OpenMatrixLink(room.to_string(), None, via))),
            ].spacing(10).align_y(Alignment::Center).into()
        };
        if let Some(pred) = &adv.predecessor {
            version = version.push(ui::divider()).push(link(pred, "Continues an older room", "Older messages are in the room this one replaced."));
        }
        if let Some(succ) = &adv.successor {
            version = version.push(ui::divider()).push(link(succ, "Replaced by a newer room", "This room was upgraded; the conversation continues there."));
        }
        col = col.push(ui::field("Version", ui::card(version)));

        // --- Addresses: published (main + alternatives) and local ---
        let mut addresses: Vec<&String> = adv.main_alias.iter().chain(&adv.alt_aliases).chain(&adv.local_aliases).collect();
        addresses.dedup_by(|a, b| a == b);
        let mut seen = std::collections::HashSet::new();
        addresses.retain(|a| seen.insert(*a));
        let mut list = column![].spacing(4);
        if addresses.is_empty() {
            list = list.push(ui::hint("No addresses yet. Add one so people can join with a link like #name:server."));
        }
        let can = adv.can_publish_aliases;
        for alias in addresses {
            let is_main = adv.main_alias.as_ref() == Some(alias);
            let is_alt = adv.alt_aliases.contains(alias);
            let is_local = adv.local_aliases.contains(alias);
            let mut r = row![text(alias.as_str()).font(MONO).size(13).color(c.text_normal)]
                .spacing(8)
                .align_y(Alignment::Center);
            if is_main {
                r = r.push(ui::badge("Main", c.accent));
            } else if !is_alt {
                r = r.push(ui::badge("Not published", c.text_muted));
            }
            r = r.push(Space::new().width(Fill));
            if !is_main {
                r = r.push(ui::btn("Make main", Tone::Ghost, can.then(|| Message::SetMainAlias(Some(alias.clone())))));
            }
            r = r.push(if is_main {
                ui::btn("Unpublish", Tone::Ghost, can.then_some(Message::SetMainAlias(None)))
            } else if is_alt {
                ui::btn("Unpublish", Tone::Ghost, can.then(|| Message::SetAliasPublished(alias.clone(), false)))
            } else {
                ui::btn("Publish", Tone::Ghost, can.then(|| Message::SetAliasPublished(alias.clone(), true)))
            });
            if is_local {
                r = r.push(ui::icon_only(crate::icons::SVG_TRASH, "Delete address", Tone::Ghost,
                    Some(Message::RemoveRoomAlias(alias.clone()))));
            }
            list = list.push(container(r).padding([2, 4]));
        }
        let add_row = row![
            ui::input(&format!("name  (becomes #name:{})", adv.server), &self.room_alias_input, Message::RoomAliasInputChanged)
                .on_submit(Message::AddRoomAlias)
                .width(Fill),
            ui::btn("Add", Tone::Secondary,
                (!self.room_alias_input.trim().is_empty()).then_some(Message::AddRoomAlias)),
        ]
        .spacing(8)
        .align_y(Alignment::Center);
        let mut addr = column![ui::card(list), add_row].spacing(8);
        addr = addr.push(ui::hint(format!(
            "Published addresses show in the room's details; the main one is used in links. \
            Addresses you add live on {} (delete them with the bin).", adv.server)));
        if !can {
            addr = addr.push(ui::hint("You don't have permission to change which addresses are published."));
        }
        col = col.push(ui::field("Addresses", addr));
        col.into()
    }
}
