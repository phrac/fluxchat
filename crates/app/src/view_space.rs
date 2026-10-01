//! The space dialog: overview header, Rooms / Members / Settings tabs, and
//! room management (add existing, create, remove, join).

use iced::widget::{column, container, row, scrollable, text, Space};
use iced::{Alignment, Element, Fill, Length};

use crate::message::Message;
use crate::state::{FluxChat, SpaceTab};
use crate::theme;
use crate::ui::{self, Tone};
use crate::view::{thin_scrollbar, MONO};

/// Width of the space dialog.
const SPACE_DIALOG_WIDTH: f32 = 580.0;
/// Height of the tab body (fixed so switching tabs doesn't resize the dialog).
const SPACE_TAB_HEIGHT: f32 = 380.0;

impl FluxChat {
    pub(crate) fn view_space_details_modal<'a>(&'a self, space_id: &'a str) -> Element<'a, Message> {
        let space = self.spaces.iter().find(|s| s.room_id == space_id);
        let name = space.map(|s| s.name.as_str()).unwrap_or("Space");
        let topic = space.and_then(|s| s.topic.as_deref()).unwrap_or("");
        let perms = self.space_perms;
        let room_count = self.space_detail_rooms.as_ref().map(|r| r.len());

        // --- Header: avatar, name, stats, topic, actions ---
        let mut stats = vec![space_id.split(':').nth(1).unwrap_or("").to_string()];
        if !self.space_detail_members.is_empty() {
            stats.push(plural(self.space_detail_members.len(), "member"));
        }
        if let Some(n) = room_count {
            stats.push(plural(n, "room"));
        }
        let mut title_col = column![
            text(name).font(MONO).size(18).color(theme::colors().text_bright),
            text(stats.join("  \u{00b7}  ")).font(MONO).size(12).color(theme::colors().text_dim),
        ]
        .spacing(4);
        if !topic.is_empty() {
            let short: String = topic.lines().next().unwrap_or("").chars().take(160).collect();
            title_col = title_col.push(
                text(short).font(MONO).size(12).color(theme::colors().text_muted)
                    .wrapping(text::Wrapping::WordOrGlyph),
            );
        }
        let mut actions = row![].spacing(6).align_y(Alignment::Center);
        if perms.invite {
            actions = actions.push(ui::icon_only(
                crate::icons::SVG_USER_PLUS, "Invite people", Tone::Secondary,
                Some(Message::SpaceInvite(space_id.to_string())),
            ));
        }
        actions = actions.push(ui::close_button());
        let header = row![
            self.render_room_avatar(space.and_then(|s| s.avatar_url.as_deref()), name, 56.0),
            container(title_col).width(Fill),
            actions,
        ]
        .spacing(14)
        .align_y(Alignment::Start);

        // --- Tabs ---
        let rooms_label = match room_count {
            Some(n) => format!("Rooms  {n}"),
            None => "Rooms".into(),
        };
        let members_label = if self.space_detail_members.is_empty() {
            "Members".to_string()
        } else {
            format!("Members  {}", self.space_detail_members.len())
        };
        let tab_bar = ui::tabs(vec![
            (rooms_label, Message::SpaceTabChanged(SpaceTab::Rooms), self.space_tab == SpaceTab::Rooms),
            (members_label, Message::SpaceTabChanged(SpaceTab::Members), self.space_tab == SpaceTab::Members),
            ("Settings".into(), Message::SpaceTabChanged(SpaceTab::Settings), self.space_tab == SpaceTab::Settings),
        ]);

        let body: Element<'a, Message> = match self.space_tab {
            SpaceTab::Rooms if self.space_adding => self.view_space_add_rooms(space_id, name),
            SpaceTab::Rooms => self.view_space_rooms(space_id),
            SpaceTab::Members => self.view_space_members(),
            SpaceTab::Settings => self.view_space_settings(space_id, name),
        };

        let mut col = column![
            header,
            tab_bar,
            container(body).height(Length::Fixed(SPACE_TAB_HEIGHT)).width(Fill),
        ]
        .spacing(ui::GAP)
        .width(Length::Fixed(SPACE_DIALOG_WIDTH));
        if let Some(status) = &self.space_status {
            col = col.push(ui::status_line(status));
        }
        col.into()
    }

    /// Search box + optional trailing buttons.
    fn space_toolbar<'a>(&'a self, placeholder: &str, trailing: Vec<Element<'a, Message>>) -> Element<'a, Message> {
        let mut r = row![
            ui::input(placeholder, &self.space_filter, Message::SpaceFilterChanged).width(Fill),
        ]
        .spacing(8)
        .align_y(Alignment::Center);
        for t in trailing {
            r = r.push(t);
        }
        r.into()
    }

    fn view_space_rooms<'a>(&'a self, space_id: &'a str) -> Element<'a, Message> {
        let perms = self.space_perms;
        let mut trailing: Vec<Element<'a, Message>> = Vec::new();
        if perms.manage_children {
            trailing.push(ui::icon_btn(crate::icons::SVG_PLUS, "Add rooms", Tone::Secondary, Some(Message::SpaceToggleAdding)).into());
            trailing.push(ui::btn("Create room", Tone::Primary, Some(Message::SpaceCreateRoom(space_id.to_string()))).into());
        }
        let toolbar = self.space_toolbar("Filter rooms", trailing);

        let Some(children) = &self.space_detail_rooms else {
            return column![toolbar, ui::empty_state(crate::icons::SVG_CLOCK, "Loading rooms\u{2026}", None)]
                .spacing(10).into();
        };
        let q = self.space_filter.to_lowercase();
        let shown: Vec<&fluxchat_core::spaces::SpaceChild> = children.iter()
            .filter(|c| q.is_empty() || c.name.to_lowercase().contains(&q)
                || c.topic.as_deref().is_some_and(|t| t.to_lowercase().contains(&q)))
            .collect();

        if children.is_empty() {
            let detail = if perms.manage_children {
                "Add a room you're in, or create a new one."
            } else {
                "Rooms added to this space will show up here."
            };
            return column![
                toolbar,
                ui::empty_state(crate::icons::SVG_LAYERS, "No rooms in this space yet", Some(detail.into())),
            ]
            .spacing(10)
            .into();
        }

        let mut list = column![].spacing(2);
        for child in shown {
            list = list.push(self.view_space_child_row(space_id, child));
        }
        column![toolbar, scroll(list)].spacing(10).into()
    }

    fn view_space_child_row<'a>(
        &'a self,
        space_id: &'a str,
        child: &'a fluxchat_core::spaces::SpaceChild,
    ) -> Element<'a, Message> {
        let busy = self.space_busy.contains(&child.room_id);
        let confirming = self.space_confirm_remove.as_deref() == Some(child.room_id.as_str());

        let mut title = row![
            text(&child.name).font(MONO).size(13).color(theme::colors().text_bright)
                .wrapping(text::Wrapping::None),
        ]
        .spacing(6)
        .align_y(Alignment::Center);
        if child.is_space {
            title = title.push(ui::badge("space", theme::colors().purple));
        }
        if child.suggested {
            title = title.push(ui::badge("suggested", theme::colors().accent));
        }
        let detail = child.topic.as_deref()
            .and_then(|t| t.lines().next())
            .filter(|t| !t.trim().is_empty())
            .map(|t| t.chars().take(90).collect::<String>())
            .or_else(|| child.alias.clone())
            .unwrap_or_else(|| plural(child.num_members as usize, "member"));
        let info = column![
            title,
            text(detail).font(MONO).size(11).color(theme::colors().text_dim)
                .wrapping(text::Wrapping::None),
        ]
        .spacing(2);

        let mut actions = row![].spacing(6).align_y(Alignment::Center);
        if confirming {
            actions = actions
                .push(text("Remove from space?").font(MONO).size(12).color(theme::colors().text_muted))
                .push(ui::btn("Cancel", Tone::Ghost, Some(Message::SpaceAskRemove(None))))
                .push(ui::btn("Remove", Tone::Danger,
                    Some(Message::SpaceRemoveChild(space_id.to_string(), child.room_id.clone()))));
        } else {
            actions = actions.push(
                text(format!("{}", child.num_members)).font(MONO).size(11).color(theme::colors().text_dim),
            );
            actions = actions.push(ui::icon(crate::icons::SVG_USERS, 12.0, theme::colors().text_dim));
            actions = actions.push(Space::new().width(6));
            if child.joined {
                actions = actions.push(ui::btn("Open", Tone::Ghost, Some(Message::SpaceOpenChild(child.room_id.clone()))));
            } else {
                actions = actions.push(ui::btn(
                    if busy { "Joining\u{2026}" } else { "Join" },
                    Tone::Secondary,
                    (!busy).then(|| Message::SpaceJoinChild(space_id.to_string(), child.room_id.clone())),
                ));
            }
            if self.space_perms.manage_children {
                actions = actions.push(ui::icon_only(
                    crate::icons::SVG_TRASH, "Remove from space", Tone::Ghost,
                    (!busy).then(|| Message::SpaceAskRemove(Some(child.room_id.clone()))),
                ));
            }
        }

        container(
            row![
                self.render_room_avatar(child.avatar_url.as_deref(), &child.name, 34.0),
                container(info).width(Fill).clip(true),
                actions,
            ]
            .spacing(12)
            .align_y(Alignment::Center),
        )
        .padding([6, 8])
        .width(Fill)
        .style(move |_: &iced::Theme| iced::widget::container::Style {
            background: confirming.then(|| theme::with_alpha(theme::colors().red, 0.08).into()),
            border: iced::Border { radius: ui::RADIUS.into(), ..Default::default() },
            ..Default::default()
        })
        .into()
    }

    /// Picker of joined rooms that aren't in the space yet.
    fn view_space_add_rooms<'a>(&'a self, space_id: &'a str, space_name: &'a str) -> Element<'a, Message> {
        let toolbar = self.space_toolbar(
            "Search your rooms",
            vec![ui::btn("Done", Tone::Secondary, Some(Message::SpaceToggleAdding)).into()],
        );
        let in_space: std::collections::HashSet<&str> = self.space_detail_rooms.iter()
            .flatten()
            .map(|c| c.room_id.as_str())
            .collect();
        let q = self.space_filter.to_lowercase();
        let candidates: Vec<&fluxchat_core::RoomInfo> = self.rooms.iter()
            .filter(|r| !r.is_direct && !in_space.contains(r.room_id.as_str()))
            .filter(|r| q.is_empty() || r.name.to_lowercase().contains(&q))
            .collect();

        let heading = ui::hint(format!("Add rooms you're in to {space_name}. A room can belong to several spaces."));
        if candidates.is_empty() {
            let msg = if q.is_empty() { "Every room you're in is already here" } else { "No matching rooms" };
            return column![toolbar, heading, ui::empty_state(crate::icons::SVG_SEARCH, msg, None)]
                .spacing(10).into();
        }
        let mut list = column![].spacing(2);
        for room in candidates {
            let busy = self.space_busy.contains(&room.room_id);
            let detail = room.space_name.as_deref()
                .map(|s| format!("in {s}"))
                .or_else(|| room.topic.as_deref().and_then(|t| t.lines().next()).map(|t| t.chars().take(80).collect()))
                .unwrap_or_default();
            list = list.push(
                container(
                    row![
                        self.render_room_avatar(room.avatar_url.as_deref(), &room.name, 30.0),
                        container(column![
                            text(&room.name).font(MONO).size(13).color(theme::colors().text_bright)
                                .wrapping(text::Wrapping::None),
                            text(detail).font(MONO).size(11).color(theme::colors().text_dim)
                                .wrapping(text::Wrapping::None),
                        ].spacing(2)).width(Fill).clip(true),
                        ui::btn(
                            if busy { "Adding\u{2026}" } else { "Add" },
                            Tone::Secondary,
                            (!busy).then(|| Message::SpaceAddChild(space_id.to_string(), room.room_id.clone())),
                        ),
                    ]
                    .spacing(12)
                    .align_y(Alignment::Center),
                )
                .padding([6, 8])
                .width(Fill),
            );
        }
        column![toolbar, heading, scroll(list)].spacing(10).into()
    }

    fn view_space_members<'a>(&'a self) -> Element<'a, Message> {
        let toolbar = self.space_toolbar("Filter members", Vec::new());
        if self.space_detail_members.is_empty() {
            return column![toolbar, ui::empty_state(crate::icons::SVG_CLOCK, "Loading members\u{2026}", None)]
                .spacing(10).into();
        }
        let q = self.space_filter.to_lowercase();
        let mut members: Vec<&fluxchat_core::MemberInfo> = self.space_detail_members.iter()
            .filter(|m| q.is_empty() || m.display_name.to_lowercase().contains(&q)
                || m.user_id.to_lowercase().contains(&q))
            .collect();
        members.sort_by(|a, b| b.power_level.cmp(&a.power_level)
            .then_with(|| a.display_name.to_lowercase().cmp(&b.display_name.to_lowercase())));

        let mut list = column![].spacing(2);
        for m in members.into_iter().take(200) {
            let mut name_row = row![
                text(&m.display_name).font(MONO).size(13).color(theme::nick_color(&m.display_name))
                    .wrapping(text::Wrapping::None),
            ]
            .spacing(6)
            .align_y(Alignment::Center);
            if m.power_level >= 100 {
                name_row = name_row.push(ui::badge("admin", theme::colors().yellow));
            } else if m.power_level >= 50 {
                name_row = name_row.push(ui::badge("mod", theme::colors().accent));
            }
            list = list.push(
                iced::widget::button(
                    row![
                        self.render_avatar_with_initial(m.avatar_url.as_deref(), &m.display_name, 30.0),
                        container(column![
                            name_row,
                            text(&m.user_id).font(MONO).size(11).color(theme::colors().text_dim)
                                .wrapping(text::Wrapping::None),
                        ].spacing(2)).width(Fill).clip(true),
                        ui::icon(crate::icons::SVG_CHEVRON_RIGHT, 14.0, theme::colors().text_dim),
                    ]
                    .spacing(12)
                    .align_y(Alignment::Center),
                )
                .on_press(Message::OpenModal(crate::state::Modal::Profile { user_id: m.user_id.clone() }))
                .padding([6, 8])
                .width(Fill)
                .style(ui::row_style(false)),
            );
        }
        column![toolbar, scroll(list)].spacing(10).into()
    }

    fn view_space_settings<'a>(&'a self, space_id: &'a str, name: &'a str) -> Element<'a, Message> {
        let can_edit = self.space_perms.edit_details;
        let avatar_mxc = self.spaces.iter().find(|s| s.room_id == space_id).and_then(|s| s.avatar_url.as_deref());

        let mut avatar_actions = row![].spacing(8);
        if can_edit {
            avatar_actions = avatar_actions
                .push(ui::icon_btn(crate::icons::SVG_IMAGE, "Upload image", Tone::Secondary,
                    Some(Message::SpaceUploadAvatar(space_id.to_string()))))
                .push(ui::btn("Remove", Tone::Ghost,
                    avatar_mxc.is_some().then(|| Message::SpaceRemoveAvatar(space_id.to_string()))));
        }
        let avatar_row = row![
            self.render_room_avatar(avatar_mxc, name, 64.0),
            column![
                ui::section_label("Space icon"),
                if can_edit { ui::hint("Square images look best.") } else { ui::hint("Only admins can change the icon.") },
                avatar_actions,
            ]
            .spacing(6),
        ]
        .spacing(16)
        .align_y(Alignment::Center);

        let name_input = {
            let i = ui::input("Space name", &self.space_name_input, Message::SpaceNameChanged);
            if can_edit { i.on_submit(Message::SpaceSaveDetails(space_id.to_string())) } else { i.on_input_maybe(None::<fn(String) -> Message>) }
        };
        let topic_input = {
            let i = ui::input("What is this space about?", &self.space_topic_input, Message::SpaceTopicChanged);
            if can_edit { i.on_submit(Message::SpaceSaveDetails(space_id.to_string())) } else { i.on_input_maybe(None::<fn(String) -> Message>) }
        };

        let mut col = column![
            avatar_row,
            ui::field("Name", name_input),
            ui::field("Topic", topic_input),
        ]
        .spacing(ui::GAP);
        if can_edit {
            col = col.push(ui::footer(vec![
                ui::btn("Save changes", Tone::Primary, Some(Message::SpaceSaveDetails(space_id.to_string()))).into(),
            ]));
        }
        col = col.push(ui::divider());
        col = col.push(self.view_space_visibility());
        col = col.push(ui::divider());
        col = col.push(
            row![
                column![
                    text("Leave space").font(MONO).size(13).color(theme::colors().text_normal),
                    ui::hint("You'll stay in its rooms; leave those separately."),
                ]
                .spacing(2)
                .width(Fill),
                ui::icon_btn(crate::icons::SVG_LOG_OUT, "Leave", Tone::Danger,
                    Some(Message::AskLeaveRoom(space_id.to_string()))),
            ]
            .spacing(12)
            .align_y(Alignment::Center),
        );
        scroll(col)
    }
}

impl FluxChat {
    /// Who can find and join the space, its directory listing and addresses
    /// (Room Settings' security/address state, loaded for the space).
    fn view_space_visibility(&self) -> Element<'_, Message> {
        use fluxchat_core::room_admin::JoinMode;
        use crate::view_room_admin::choice;
        let Some(sec) = &self.room_security else {
            return ui::field("Visibility", ui::hint("Loading\u{2026}"));
        };
        let join = |mode: JoinMode| sec.can_change_join.then_some(Message::SetJoinMode(mode));
        let mut visibility = column![
            choice("Private (invite only)", "Only people who are invited can find and join",
                sec.join == JoinMode::Invite, join(JoinMode::Invite)),
            choice("Ask to join", "Anyone can request to join; members approve",
                sec.join == JoinMode::Knock, join(JoinMode::Knock)),
            choice("Public", "Anyone can find and join",
                sec.join == JoinMode::Public, join(JoinMode::Public)),
        ]
        .spacing(6);
        if sec.join == JoinMode::SpaceMembers {
            // Set elsewhere (a sub-space open to its parent's members).
            visibility = visibility.push(choice("Members of parent spaces", "Set by another client",
                true, None));
        }
        let mut col = column![ui::field("Visibility", visibility)].spacing(ui::GAP);
        if !sec.can_change_join {
            col = col.push(ui::hint("Only admins can change who can join."));
        }
        if sec.join == JoinMode::Public {
            col = col.push(ui::card(ui::toggle_row("List in the directory",
                "Let people find it in Explore rooms", sec.in_directory, Message::SetInDirectory)));
        }
        if let Some(adv) = &self.room_advanced {
            col = col.push(ui::field("Address", self.view_addresses(adv, "space")));
        }
        col.into()
    }
}

/// Vertical scroller with the thin scrollbar used across the app.
fn scroll<'a>(content: impl Into<Element<'a, Message>>) -> Element<'a, Message> {
    scrollable(container(content).padding(iced::Padding { right: 8.0, ..Default::default() }))
        .id(crate::view::dialog_scroll_id())
        .height(Fill)
        .direction(iced::widget::scrollable::Direction::Vertical(
            iced::widget::scrollable::Scrollbar::new().width(4).scroller_width(4),
        ))
        .style(thin_scrollbar)
        .into()
}

/// "1 room", "3 rooms".
fn plural(n: usize, noun: &str) -> String {
    if n == 1 { format!("1 {noun}") } else { format!("{n} {noun}s") }
}
