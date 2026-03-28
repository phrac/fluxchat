use iced::widget::{
    button, center, column, container, mouse_area, opaque, rich_text, row, rule, scrollable, stack,
    text, text_input, Space,
};
use iced::{Color, Element, Fill, Font, Length, Subscription, Theme};

/// Fixed ID for the main message input so we can refocus it programmatically.
pub fn input_id() -> iced::widget::Id {
    iced::widget::Id::new("main_input")
}

use crate::message::Message;
use crate::state::{FluxChat, Screen};
use crate::theme;
use crate::update::extract_urls;

// Lucide-style SVG icons for formatting toolbar
const SVG_BOLD: &[u8] = br#"<svg xmlns="http://www.w3.org/2000/svg" width="24" height="24" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2.5" stroke-linecap="round" stroke-linejoin="round"><path d="M6 4h8a4 4 0 0 1 4 4 4 4 0 0 1-4 4H6z"/><path d="M6 12h9a4 4 0 0 1 4 4 4 4 0 0 1-4 4H6z"/></svg>"#;
const SVG_ITALIC: &[u8] = br#"<svg xmlns="http://www.w3.org/2000/svg" width="24" height="24" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2.5" stroke-linecap="round" stroke-linejoin="round"><line x1="19" y1="4" x2="10" y2="4"/><line x1="14" y1="20" x2="5" y2="20"/><line x1="15" y1="4" x2="9" y2="20"/></svg>"#;
const SVG_STRIKE: &[u8] = br#"<svg xmlns="http://www.w3.org/2000/svg" width="24" height="24" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2.5" stroke-linecap="round" stroke-linejoin="round"><path d="M16 4H9a3 3 0 0 0-3 3c0 2 1.5 3 3 3h6"/><path d="M4 12h16"/><path d="M15 12a3 3 0 0 1-3 3H8"/></svg>"#;
const SVG_CODE: &[u8] = br#"<svg xmlns="http://www.w3.org/2000/svg" width="24" height="24" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2.5" stroke-linecap="round" stroke-linejoin="round"><polyline points="16 18 22 12 16 6"/><polyline points="8 6 2 12 8 18"/></svg>"#;
const SVG_CODEBLOCK: &[u8] = br#"<svg xmlns="http://www.w3.org/2000/svg" width="24" height="24" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2.5" stroke-linecap="round" stroke-linejoin="round"><rect x="3" y="3" width="18" height="18" rx="2"/><polyline points="10 8 6 12 10 16"/><polyline points="14 8 18 12 14 16"/></svg>"#;
const SVG_LINK: &[u8] = br#"<svg xmlns="http://www.w3.org/2000/svg" width="24" height="24" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2.5" stroke-linecap="round" stroke-linejoin="round"><path d="M10 13a5 5 0 0 0 7.54.54l3-3a5 5 0 0 0-7.07-7.07l-1.72 1.71"/><path d="M14 11a5 5 0 0 0-7.54-.54l-3 3a5 5 0 0 0 7.07 7.07l1.71-1.71"/></svg>"#;
const SVG_EMOJI: &[u8] = br#"<svg xmlns="http://www.w3.org/2000/svg" width="24" height="24" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><circle cx="12" cy="12" r="10"/><path d="M8 14s1.5 2 4 2 4-2 4-2"/><line x1="9" y1="9" x2="9.01" y2="9"/><line x1="15" y1="9" x2="15.01" y2="9"/></svg>"#;

const MONO: Font = Font {
    family: iced::font::Family::Name("JetBrainsMono Nerd Font"),
    ..Font::MONOSPACE
};

impl FluxChat {
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

    /// Loading screen while restoring session.
    fn view_loading(&self) -> Element<Message> {
        center(
            text("loading...")
                .font(MONO)
                .size(16)
                .color(theme::TEXT_MUTED),
        )
        .style(|_: &Theme| container::Style {
            background: Some(theme::BG_DEEP.into()),
            ..Default::default()
        })
        .into()
    }

    /// Login screen with SSO + password support.
    fn view_login(&self) -> Element<Message> {
        let title = text("fluxchat")
            .size(22)
            .font(MONO)
            .color(theme::TEXT_BRIGHT);

        let subtitle = text("Sign in with your Matrix account")
            .size(15)
            .font(MONO)
            .color(theme::TEXT_MUTED);

        let homeserver = text_input("homeserver (e.g. matrix.org)", &self.login_homeserver)
            .on_input(Message::LoginHomeserverChanged)
            .font(MONO)
            .size(14)
            .padding(8);

        let mut col = column![title, subtitle, homeserver].spacing(12).width(340);

        // Error display
        if let Some(err) = &self.login_error {
            col = col.push(
                container(text(err.as_str()).size(15).font(MONO).color(theme::RED))
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
                    let mut btn = button(text(label).font(MONO).size(14).color(theme::TEXT_BRIGHT))
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
                            button(text(label).font(MONO).size(14).color(theme::TEXT_BRIGHT))
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
                        .font(MONO)
                        .color(theme::TEXT_DIM),
                );

                // Divider
                col = col.push(
                    row![
                        rule::horizontal(1),
                        text("or").size(14).font(MONO).color(theme::TEXT_DIM),
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
            .font(MONO)
            .size(14)
            .padding(8);

        let password = text_input("password", &self.login_password)
            .on_input(Message::LoginPasswordChanged)
            .on_submit(Message::LoginSubmit)
            .secure(true)
            .font(MONO)
            .size(14)
            .padding(8);

        let login_label = if self.login_submitting {
            "Signing in..."
        } else {
            "Sign In"
        };
        let mut login_btn = button(
            text(login_label)
                .font(MONO)
                .size(14)
                .color(theme::BG_DEEP),
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
                    background: Some(theme::BG_DARK.into()),
                    border: iced::Border {
                        color: theme::BORDER,
                        width: 1.0,
                        radius: 8.0.into(),
                    },
                    ..Default::default()
                }),
        )
        .style(|_: &Theme| container::Style {
            background: Some(theme::BG_DEEP.into()),
            ..Default::default()
        })
        .into()
    }

    /// Main chat layout: sidebar | rooms | messages | status bar.
    fn view_main(&self) -> Element<Message> {
        let sidebar = self.view_sidebar();
        let room_list = self.view_room_list();
        let main_content = self.view_content();

        let mut main_area = row![
            sidebar,
            container(room_list).width(200),
            main_content,
        ];

        // Member list (right sidebar)
        if self.show_member_list && self.selected_room_id.is_some() {
            main_area = main_area.push(self.view_member_list());
        }

        let main_area = main_area.width(Fill).height(Fill);

        let status_bar = self.view_status_bar();

        let base = container(column![main_area, status_bar])
            .width(Fill)
            .height(Fill)
            .style(|_: &Theme| container::Style {
                background: Some(theme::BG_DEEP.into()),
                ..Default::default()
            });

        if self.modal.is_some() {
            stack(vec![
                base.into(),
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
            ])
            .into()
        } else {
            base.into()
        }
    }

    /// Left space sidebar (icons).
    fn view_sidebar(&self) -> Element<Message> {
        let is_home = self.selected_space_id.is_none();
        let home_bg = if is_home { theme::BG_LIGHT } else { theme::BG_DARK };

        let home_btn = button(text("H").font(MONO).size(14).color(theme::TEXT_BRIGHT))
            .on_press(Message::SelectSpace(None))
            .padding([8, 10])
            .style(move |_, _| button::Style {
                background: Some(home_bg.into()),
                text_color: theme::TEXT_BRIGHT,
                ..Default::default()
            });

        let mut col = column![home_btn].spacing(4).padding(4);

        for space in &self.spaces {
            let label = space
                .name
                .chars()
                .next()
                .unwrap_or('?')
                .to_uppercase()
                .to_string();
            let space_id = space.room_id.clone();
            let is_selected = self.selected_space_id.as_deref() == Some(&space.room_id);
            let bg = if is_selected {
                theme::BG_LIGHT
            } else {
                theme::BG_DARK
            };
            let btn = button(text(label).font(MONO).size(14).color(theme::TEXT_NORMAL))
                .on_press(Message::SelectSpace(Some(space_id)))
                .padding([8, 10])
                .style(move |_, _| button::Style {
                    background: Some(bg.into()),
                    text_color: theme::TEXT_NORMAL,
                    ..Default::default()
                });
            col = col.push(btn);
        }

        container(scrollable(col))
            .width(48)
            .height(Fill)
            .style(|_: &Theme| container::Style {
                background: Some(theme::BG_DARK.into()),
                ..Default::default()
            })
            .into()
    }

    /// Room list panel with grouped sections.
    fn view_room_list(&self) -> Element<Message> {
        let selected_space_name: Option<String> = self
            .selected_space_id
            .as_ref()
            .and_then(|sid| self.spaces.iter().find(|s| s.room_id == *sid))
            .map(|s| s.name.clone());

        // Group rooms into sections
        let mut favourites: Vec<&fluxchat_core::RoomInfo> = Vec::new();
        let mut ungrouped: Vec<&fluxchat_core::RoomInfo> = Vec::new();
        let mut by_space: std::collections::BTreeMap<String, Vec<&fluxchat_core::RoomInfo>> =
            std::collections::BTreeMap::new();
        let mut dms: Vec<&fluxchat_core::RoomInfo> = Vec::new();

        for room in &self.rooms {
            // Filter by selected space
            if let Some(ref filter) = selected_space_name {
                if room.space_name.as_deref() != Some(filter) {
                    continue;
                }
            }
            if room.is_favourite {
                favourites.push(room);
            }
            if room.is_direct {
                dms.push(room);
            } else if let Some(ref sn) = room.space_name {
                by_space.entry(sn.clone()).or_default().push(room);
            } else {
                ungrouped.push(room);
            }
        }

        // Header
        let header_text: String = selected_space_name
            .as_deref()
            .unwrap_or("fluxchat")
            .to_string();
        let header = container(
            row![
                text(header_text)
                    .font(MONO)
                    .size(14)
                    .color(theme::TEXT_BRIGHT),
                Space::new().width(Fill),
                button(text("+").font(MONO).size(16).color(theme::TEXT_DIM))
                    .on_press(Message::OpenModal(crate::state::Modal::CreateRoom))
                    .padding([0, 4])
                    .style(|_, _| button::Style {
                        background: None,
                        text_color: theme::TEXT_DIM,
                        ..Default::default()
                    }),
            ]
            .align_y(iced::Alignment::Center),
        )
        .padding([8, 12])
        .width(Fill)
        .style(|_: &Theme| container::Style {
            border: iced::Border {
                color: theme::BORDER,
                width: 1.0,
                radius: 0.0.into(),
            },
            ..Default::default()
        });

        let mut col = column![header].spacing(0);

        // Invites
        if !self.invited_rooms.is_empty() {
            col = col.push(self.view_section_header("Invites", self.invited_rooms.len()));
            if !self.collapsed_sections.contains("Invites") {
                for room in &self.invited_rooms {
                    col = col.push(self.view_room_entry(room, false));
                }
            }
        }

        // Favourites
        if !favourites.is_empty() {
            col = col.push(self.view_section_header("Favourites", favourites.len()));
            if !self.collapsed_sections.contains("Favourites") {
                for room in &favourites {
                    col = col.push(self.view_room_entry(room, false));
                }
            }
        }

        // Ungrouped channels
        if !ungrouped.is_empty() {
            col = col.push(self.view_section_header("Channels", ungrouped.len()));
            if !self.collapsed_sections.contains("Channels") {
                for room in &ungrouped {
                    col = col.push(self.view_room_entry(room, false));
                }
            }
        }

        // Space-grouped rooms
        for (space_name, rooms) in &by_space {
            col = col.push(self.view_section_header(space_name, rooms.len()));
            if !self.collapsed_sections.contains(space_name) {
                for room in rooms {
                    col = col.push(self.view_room_entry(room, false));
                }
            }
        }

        // DMs
        if !dms.is_empty() {
            col = col.push(self.view_section_header("Direct Messages", dms.len()));
            if !self.collapsed_sections.contains("Direct Messages") {
                for room in &dms {
                    col = col.push(self.view_room_entry(room, true));
                }
            }
        }

        container(scrollable(col).height(Fill))
            .width(Fill)
            .height(Fill)
            .style(|_: &Theme| container::Style {
                background: Some(theme::BG_DARK.into()),
                ..Default::default()
            })
            .into()
    }

    /// Collapsible section header.
    fn view_section_header(&self, name: &str, count: usize) -> Element<'_, Message> {
        let collapsed = self.collapsed_sections.contains(name);
        let arrow = if collapsed { "\u{25b8}" } else { "\u{25be}" };
        let name_owned = name.to_string();
        let name_label = name.to_uppercase();

        button(
            row![
                text(arrow)
                    .font(MONO)
                    .size(12)
                    .color(theme::TEXT_DIM),
                text(name_label)
                    .font(MONO)
                    .size(13)
                    .color(theme::TEXT_MUTED),
                Space::new().width(Fill),
                text(count.to_string())
                    .font(MONO)
                    .size(12)
                    .color(theme::TEXT_DIM),
            ]
            .spacing(6)
            .align_y(iced::Alignment::Center),
        )
        .on_press(Message::ToggleSection(name_owned))
        .padding([4, 12])
        .width(Fill)
        .style(|_, _| button::Style {
            background: None,
            text_color: theme::TEXT_MUTED,
            ..Default::default()
        })
        .into()
    }

    /// Single room entry in the list.
    fn view_room_entry(&self, room: &fluxchat_core::RoomInfo, is_dm: bool) -> Element<Message> {
        let is_selected = self.selected_room_id.as_deref() == Some(&room.room_id);
        let name_color = if is_selected {
            theme::TEXT_BRIGHT
        } else {
            theme::TEXT_NORMAL
        };
        let bg = if is_selected {
            theme::BG_LIGHT
        } else {
            theme::BG_DARK
        };

        let prefix = if is_dm { "" } else { "#" };
        let icon: Element<Message> = if is_dm {
            // Person icon for DMs
            text("\u{25cf}").font(MONO).size(8).color(theme::TEXT_DIM).into()
        } else if room.is_encrypted {
            // Lock icon for encrypted rooms
            text("\u{1f512}").size(13).color(theme::TEXT_DIM).into()
        } else {
            Space::new().width(0).into()
        };
        let mut row_content = row![
            icon,
            text(format!("{prefix}{}", room.name))
                .font(MONO)
                .size(15)
                .color(name_color),
        ]
        .spacing(4)
        .align_y(iced::Alignment::Center);

        // Unread badge
        if room.highlight_count > 0 {
            row_content = row_content.push(Space::new().width(Fill));
            row_content = row_content.push(
                container(
                    text(room.highlight_count.to_string())
                        .font(MONO)
                        .size(12)
                        .color(theme::TEXT_BRIGHT),
                )
                .padding([1, 5])
                .style(|_: &Theme| container::Style {
                    background: Some(theme::RED.into()),
                    border: iced::Border {
                        radius: 8.0.into(),
                        ..Default::default()
                    },
                    ..Default::default()
                }),
            );
        } else if room.unread_count > 0 && !self.read_rooms.contains(&room.room_id) {
            row_content = row_content.push(Space::new().width(Fill));
            row_content = row_content.push(
                container(
                    text(room.unread_count.to_string())
                        .font(MONO)
                        .size(12)
                        .color(theme::TEXT_BRIGHT),
                )
                .padding([1, 5])
                .style(|_: &Theme| container::Style {
                    background: Some(theme::ACCENT.into()),
                    border: iced::Border {
                        radius: 8.0.into(),
                        ..Default::default()
                    },
                    ..Default::default()
                }),
            );
        }

        let room_id = room.room_id.clone();
        button(row_content)
            .on_press(Message::SelectRoom(room_id))
            .padding([3, 12])
            .width(Fill)
            .style(move |_, _| button::Style {
                background: Some(bg.into()),
                text_color: name_color,
                border: iced::Border::default(),
                ..Default::default()
            })
            .into()
    }

    /// Main content area (tabs + messages + input).
    fn view_content(&self) -> Element<'_, Message> {
        let mut content = column![].width(Fill).height(Fill);

        // Tab bar
        if !self.open_tabs.is_empty() {
            let mut tabs = row![].spacing(0);
            for (i, (room_id, name, space_name, _)) in self.open_tabs.iter().enumerate() {
                if i > 0 {
                    tabs = tabs.push(
                        container(Space::new())
                            .width(1)
                            .height(28)
                            .style(|_: &Theme| container::Style {
                                background: Some(theme::BORDER.into()),
                                ..Default::default()
                            }),
                    );
                }
                let is_active = self.selected_room_id.as_deref() == Some(room_id);
                let tab_bg = if is_active {
                    theme::BG_MID
                } else {
                    theme::BG_DARK
                };
                let tab_color = if is_active {
                    theme::TEXT_BRIGHT
                } else {
                    theme::TEXT_MUTED
                };
                let bottom_color = if is_active {
                    theme::ACCENT
                } else {
                    theme::BG_DARK
                };

                let label = match space_name {
                    Some(s) => format!("{s}/{name}"),
                    None => name.clone(),
                };

                let room_id_close = room_id.clone();
                let close_btn = button(
                    text("\u{00d7}").font(MONO).size(16).color(theme::TEXT_DIM)
                )
                .on_press(Message::CloseTab(room_id_close))
                .padding([0, 6])
                .style(|_, status| button::Style {
                    background: if matches!(status, button::Status::Hovered) {
                        Some(Color::from_rgba(1.0, 1.0, 1.0, 0.1).into())
                    } else {
                        None
                    },
                    text_color: theme::TEXT_DIM,
                    border: iced::Border { radius: 4.0.into(), ..Default::default() },
                    ..Default::default()
                });

                let tab_btn = button(
                    row![
                        container(
                            text(label).font(MONO).size(14).color(tab_color)
                                .wrapping(iced::widget::text::Wrapping::None)
                        ).clip(true).width(Fill),
                        close_btn,
                    ]
                    .spacing(4)
                    .align_y(iced::Alignment::Center),
                )
                .on_press(Message::SwitchTab(i))
                .padding([6, 8])
                .width(Fill)
                .style(move |_, _| button::Style {
                    background: Some(tab_bg.into()),
                    text_color: tab_color,
                    ..Default::default()
                });

                // Tab with accent underline for active
                let tab = column![
                    tab_btn,
                    container(Space::new())
                        .width(Fill)
                        .height(2)
                        .style(move |_: &Theme| container::Style {
                            background: Some(bottom_color.into()),
                            ..Default::default()
                        }),
                ]
                .width(Length::FillPortion(1));

                tabs = tabs.push(tab);
            }

            content = content.push(
                container(tabs)
                    .width(Fill)
                    .style(|_: &Theme| container::Style {
                        background: Some(theme::BG_DARK.into()),
                        border: iced::Border {
                            color: theme::BORDER,
                            width: 1.0,
                            radius: 0.0.into(),
                        },
                        ..Default::default()
                    }),
            );
        }

        // Messages area
        if let Some(room_id) = &self.selected_room_id {
            // Topic bar
            if let Some(room) = self.rooms.iter().find(|r| r.room_id == *room_id) {
                if let Some(topic) = &room.topic {
                    if !topic.is_empty() {
                        content = content.push(
                            container(
                                text(topic.as_str())
                                    .font(MONO)
                                    .size(14)
                                    .color(theme::TEXT_DIM),
                            )
                            .padding([3, 12])
                            .width(Fill)
                            .style(|_: &Theme| container::Style {
                                background: Some(theme::BG_DARK.into()),
                                border: iced::Border {
                                    color: theme::BORDER,
                                    width: 1.0,
                                    radius: 0.0.into(),
                                },
                                ..Default::default()
                            }),
                        );
                    }
                }
            }

            // Search bar
            if self.search_open {
                let search_input =
                    text_input("search messages...", &self.search_query)
                        .on_input(Message::SearchQueryChanged)
                        .on_submit(Message::SearchSubmit)
                        .font(MONO)
                        .size(15)
                        .padding(6);

                let search_bar = row![
                    search_input,
                    button(text("x").font(MONO).size(13).color(theme::TEXT_DIM))
                        .on_press(Message::ToggleSearch)
                        .padding([4, 8])
                        .style(|_, _| button::Style {
                            background: None,
                            ..Default::default()
                        }),
                ]
                .spacing(4)
                .align_y(iced::Alignment::Center);

                content = content.push(
                    container(search_bar)
                        .width(Fill)
                        .padding([4, 8])
                        .style(|_: &Theme| container::Style {
                            background: Some(theme::BG_DARK.into()),
                            border: iced::Border {
                                color: theme::ACCENT,
                                width: 1.0,
                                radius: 0.0.into(),
                            },
                            ..Default::default()
                        }),
                );

                // Search results
                if !self.search_results.is_empty() {
                    let mut results_col = column![].spacing(1);
                    for result in self.search_results.iter().take(20) {
                        let secs = (result.timestamp / 1000) % 86400;
                        let h = secs / 3600;
                        let m = (secs % 3600) / 60;
                        let time_str = format!("{h:02}:{m:02}");
                        let nick_col = theme::nick_color(&result.sender_name);
                        results_col = results_col.push(
                            container(
                                row![
                                    text(time_str)
                                        .font(MONO)
                                        .size(13)
                                        .color(theme::TEXT_DIM),
                                    text(&result.sender_name)
                                        .font(MONO)
                                        .size(14)
                                        .color(nick_col),
                                    text(&result.body)
                                        .font(MONO)
                                        .size(14)
                                        .color(theme::TEXT_NORMAL),
                                ]
                                .spacing(6),
                            )
                            .padding([2, 8]),
                        );
                    }
                    content = content.push(
                        container(scrollable(results_col).height(Length::Fixed(200.0)))
                            .width(Fill)
                            .style(|_: &Theme| container::Style {
                                background: Some(theme::BG_DARK.into()),
                                border: iced::Border {
                                    color: theme::BORDER,
                                    width: 1.0,
                                    radius: 0.0.into(),
                                },
                                ..Default::default()
                            }),
                    );
                }
            }

            let messages = self.messages_by_room.get(room_id);
            let mut msg_col = column![].spacing(0).padding([2, 8]).width(Fill);

            if let Some(msgs) = messages {
                let skip = msgs.len().saturating_sub(50);
                for msg in msgs.iter().skip(skip) {
                    if self.ignored_users.contains(&msg.sender) {
                        continue;
                    }
                    // Skip empty/whitespace-only messages that aren't system/emote/image/video/poll
                    let has_content = !msg.body.trim().is_empty()
                        || msg.formatted_body.as_ref().is_some_and(|f| {
                            // Strip HTML tags and check if any text content remains
                            let stripped = f.replace(|c: char| c == '\n' || c == '\r', "");
                            let text_only = stripped
                                .split('<')
                                .flat_map(|s| s.split('>').skip(1))
                                .collect::<String>();
                            !text_only.trim().is_empty()
                        });
                    if !has_content
                        && !msg.event_id.starts_with("~")
                        && !msg.is_emote
                        && msg.image_mxc.is_none()
                        && !msg.is_video
                        && msg.poll.is_none()
                    {
                        continue;
                    }
                    msg_col = msg_col.push(self.view_message(msg));
                }
            } else {
                msg_col = msg_col.push(
                    text("Loading messages...")
                        .font(MONO)
                        .size(15)
                        .color(theme::TEXT_DIM),
                );
            }

            content = content.push(
                scrollable(container(msg_col).width(Fill))
                    .width(Fill)
                    .height(Fill)
                    .anchor_bottom(),
            );

            // Typing indicator
            if let Some(typers) = self.typing_by_room.get(room_id) {
                if !typers.is_empty() {
                    let typing_text = if typers.len() == 1 {
                        format!("{} is typing...", typers[0])
                    } else if typers.len() == 2 {
                        format!("{} and {} are typing...", typers[0], typers[1])
                    } else {
                        format!("{} people are typing...", typers.len())
                    };
                    content = content.push(
                        container(
                            text(typing_text)
                                .font(MONO)
                                .size(13)
                                .color(theme::TEXT_DIM),
                        )
                        .padding([2, 12]),
                    );
                }
            }

            // Reply/edit preview bar
            if let Some((_, sender, body)) = &self.reply_to {
                content = content.push(
                    container(
                        row![
                            text(format!("replying to {sender}: {body}"))
                                .font(MONO)
                                .size(14)
                                .color(theme::TEXT_MUTED),
                            Space::new().width(Fill),
                            button(text("x").font(MONO).size(13).color(theme::TEXT_DIM))
                                .on_press(Message::CancelReply)
                                .padding([0, 4])
                                .style(|_, _| button::Style {
                                    background: None,
                                    ..Default::default()
                                }),
                        ]
                        .align_y(iced::Alignment::Center),
                    )
                    .padding([4, 12])
                    .style(|_: &Theme| container::Style {
                        background: Some(theme::BG_DARK.into()),
                        border: iced::Border {
                            color: theme::ACCENT,
                            width: 2.0,
                            radius: 0.0.into(),
                        },
                        ..Default::default()
                    }),
                );
            } else if let Some((_, _)) = &self.edit_event {
                content = content.push(
                    container(
                        row![
                            text("editing message")
                                .font(MONO)
                                .size(14)
                                .color(theme::YELLOW),
                            Space::new().width(Fill),
                            button(text("x").font(MONO).size(13).color(theme::TEXT_DIM))
                                .on_press(Message::CancelEdit)
                                .padding([0, 4])
                                .style(|_, _| button::Style {
                                    background: None,
                                    ..Default::default()
                                }),
                        ]
                        .align_y(iced::Alignment::Center),
                    )
                    .padding([4, 12])
                    .style(|_: &Theme| container::Style {
                        background: Some(theme::BG_DARK.into()),
                        ..Default::default()
                    }),
                );
            }

            // Upload preview bar
            if let Some((ref filename, _, _)) = self.upload_pending {
                content = content.push(
                    container(
                        row![
                            text(format!("attach: {filename}"))
                                .font(MONO)
                                .size(14)
                                .color(theme::ACCENT),
                            Space::new().width(Fill),
                            button(text("send").font(MONO).size(14).color(theme::GREEN))
                                .on_press(Message::UploadFile)
                                .padding([0, 6])
                                .style(|_, _| button::Style {
                                    background: None,
                                    text_color: theme::GREEN,
                                    ..Default::default()
                                }),
                            button(text("\u{00d7}").font(MONO).size(16).color(theme::TEXT_DIM))
                                .on_press(Message::CancelUpload)
                                .padding([0, 4])
                                .style(|_, _| button::Style {
                                    background: None,
                                    ..Default::default()
                                }),
                        ]
                        .spacing(8)
                        .align_y(iced::Alignment::Center),
                    )
                    .padding([4, 12])
                    .style(|_: &Theme| container::Style {
                        background: Some(theme::BG_DEEP.into()),
                        border: iced::Border {
                            color: theme::ACCENT,
                            width: 2.0,
                            radius: 0.0.into(),
                        },
                        ..Default::default()
                    }),
                );
            }

            // Help text (command feedback)
            if let Some(ref help) = self.help_text {
                content = content.push(
                    container(
                        text(help.as_str())
                            .font(MONO)
                            .size(14)
                            .color(theme::TEXT_MUTED),
                    )
                    .padding([4, 12])
                    .width(Fill)
                    .style(|_: &Theme| container::Style {
                        background: Some(theme::BG_DARK.into()),
                        ..Default::default()
                    }),
                );
            }

            // Command autocomplete popup
            if !self.cmd_completions.is_empty() {
                let mut popup_col = column![].spacing(0);
                for (i, (name, desc)) in self.cmd_completions.iter().take(12).enumerate() {
                    let is_selected = i == self.cmd_completion_idx;
                    let bg = if is_selected {
                        theme::BG_MID
                    } else {
                        theme::BG_DEEP
                    };
                    popup_col = popup_col.push(
                        button(
                            row![
                                text(*name)
                                    .font(MONO)
                                    .size(15)
                                    .color(theme::ACCENT),
                                text(*desc)
                                    .font(MONO)
                                    .size(14)
                                    .color(theme::TEXT_DIM),
                            ]
                            .spacing(8),
                        )
                        .on_press(Message::CompleteCommand(i))
                        .width(Fill)
                        .padding([3, 12])
                        .style(move |_, _| button::Style {
                            background: Some(bg.into()),
                            text_color: theme::TEXT_NORMAL,
                            ..Default::default()
                        }),
                    );
                }
                content = content.push(
                    container(
                        container(
                            popup_col,
                        )
                        .max_height(300)
                        .padding([4, 0])
                        .style(|_: &Theme| container::Style {
                            background: Some(theme::BG_DEEP.into()),
                            border: iced::Border {
                                color: theme::ACCENT,
                                width: 2.0,
                                radius: 6.0.into(),
                            },
                            shadow: iced::Shadow {
                                color: Color::from_rgba(0.0, 0.0, 0.0, 0.5),
                                offset: iced::Vector::new(0.0, -2.0),
                                blur_radius: 12.0,
                            },
                            ..Default::default()
                        }),
                    )
                    .width(Fill)
                    .padding([4, 8]),
                );
            }

            // @mention autocomplete popup
            if !self.mention_completions.is_empty() {
                let mut popup_col = column![].spacing(0);
                for (i, (user_id, display_name)) in self.mention_completions.iter().take(8).enumerate() {
                    let is_selected = i == self.mention_completion_idx;
                    let bg = if is_selected {
                        theme::BG_MID
                    } else {
                        theme::BG_DEEP
                    };
                    let nick_col = theme::nick_color(display_name);
                    popup_col = popup_col.push(
                        button(
                            row![
                                text(display_name.as_str())
                                    .font(MONO)
                                    .size(15)
                                    .color(nick_col),
                                text(user_id.as_str())
                                    .font(MONO)
                                    .size(13)
                                    .color(theme::TEXT_DIM),
                            ]
                            .spacing(8),
                        )
                        .on_press(Message::CompleteMention(i))
                        .width(Fill)
                        .padding([3, 12])
                        .style(move |_, _| button::Style {
                            background: Some(bg.into()),
                            text_color: theme::TEXT_NORMAL,
                            ..Default::default()
                        }),
                    );
                }
                content = content.push(
                    container(
                        container(popup_col)
                            .max_height(250)
                            .padding([4, 0])
                            .style(|_: &Theme| container::Style {
                                background: Some(theme::BG_DEEP.into()),
                                border: iced::Border {
                                    color: theme::ACCENT,
                                    width: 2.0,
                                    radius: 6.0.into(),
                                },
                                shadow: iced::Shadow {
                                    color: Color::from_rgba(0.0, 0.0, 0.0, 0.5),
                                    offset: iced::Vector::new(0.0, -2.0),
                                    blur_radius: 12.0,
                                },
                                ..Default::default()
                            }),
                    )
                    .width(Fill)
                    .padding([4, 8]),
                );
            }

            // Input bar
            let attach_btn = button(
                text("+").font(MONO).size(18).color(theme::TEXT_MUTED),
            )
            .on_press(Message::PickFile)
            .padding([2, 8])
            .style(|_, status| button::Style {
                background: if matches!(status, button::Status::Hovered) {
                    Some(Color::from_rgba(1.0, 1.0, 1.0, 0.08).into())
                } else {
                    None
                },
                text_color: theme::TEXT_MUTED,
                border: iced::Border { radius: 4.0.into(), ..Default::default() },
                ..Default::default()
            });

            let prompt = text("\u{276f}").font(MONO).size(16).color(theme::ACCENT);

            let input = text_input("Type a message or /command...", &self.input_text)
                .id(input_id())
                .on_input(Message::InputChanged)
                .on_submit(Message::SendMessage)
                .font(MONO)
                .size(14)
                .padding([8, 4])
                .style(|_, _| text_input::Style {
                    background: iced::Background::Color(Color::TRANSPARENT),
                    border: iced::Border::default(),
                    icon: theme::TEXT_DIM,
                    placeholder: theme::TEXT_DIM,
                    value: theme::TEXT_BRIGHT,
                    selection: Color::from_rgba(0.23, 0.51, 0.99, 0.3),
                });

            let emoji_handle = iced::widget::svg::Handle::from_memory(SVG_EMOJI);
            let emoji_btn: Element<Message> = iced::widget::tooltip(
                button(
                    iced::widget::svg(emoji_handle)
                        .width(18)
                        .height(18)
                        .style(|_, _| iced::widget::svg::Style {
                            color: Some(theme::TEXT_DIM),
                        }),
                )
                .on_press(Message::Noop) // TODO: emoji picker
                .padding([2, 8])
                .style(|_, status| button::Style {
                    background: if matches!(status, button::Status::Hovered) {
                        Some(Color::from_rgba(1.0, 1.0, 1.0, 0.08).into())
                    } else {
                        None
                    },
                    text_color: theme::TEXT_DIM,
                    border: iced::Border { radius: 4.0.into(), ..Default::default() },
                    ..Default::default()
                }),
                text("Emoji").font(MONO).size(12),
                iced::widget::tooltip::Position::Top,
            )
            .style(|_: &Theme| container::Style {
                background: Some(theme::BG_MID.into()),
                border: iced::Border { color: theme::BORDER, width: 1.0, radius: 4.0.into() },
                ..Default::default()
            })
            .padding(6)
            .into();

            let input_row = row![
                attach_btn,
                prompt,
                input,
                emoji_btn,
            ]
            .spacing(6)
            .align_y(iced::Alignment::Center);

            // Formatting toolbar with SVG icons
            let make_fmt_btn = |svg_bytes: &'static [u8], prefix: &'static str, suffix: &'static str, title: &'static str| -> Element<Message> {
                let handle = iced::widget::svg::Handle::from_memory(svg_bytes);
                iced::widget::tooltip(
                    button(
                        iced::widget::svg(handle)
                            .width(14)
                            .height(14)
                            .style(|_, _| iced::widget::svg::Style {
                                color: Some(theme::TEXT_DIM),
                            }),
                    )
                    .on_press(Message::InsertFormat(prefix, suffix))
                    .padding([3, 6])
                    .style(|_, status| button::Style {
                        background: if matches!(status, button::Status::Hovered) {
                            Some(Color::from_rgba(1.0, 1.0, 1.0, 0.08).into())
                        } else {
                            None
                        },
                        text_color: theme::TEXT_DIM,
                        border: iced::Border { radius: 3.0.into(), ..Default::default() },
                        ..Default::default()
                    }),
                    text(title).font(MONO).size(12),
                    iced::widget::tooltip::Position::Top,
                )
                .style(|_: &Theme| container::Style {
                    background: Some(theme::BG_MID.into()),
                    border: iced::Border { color: theme::BORDER, width: 1.0, radius: 4.0.into() },
                    ..Default::default()
                })
                .padding(6)
                .into()
            };

            let fmt_bar = row![
                make_fmt_btn(SVG_BOLD, "**", "**", "Bold"),
                make_fmt_btn(SVG_ITALIC, "*", "*", "Italic"),
                make_fmt_btn(SVG_STRIKE, "~~", "~~", "Strikethrough"),
                make_fmt_btn(SVG_CODE, "`", "`", "Code"),
                make_fmt_btn(SVG_CODEBLOCK, "```\n", "\n```", "Code block"),
                make_fmt_btn(SVG_LINK, "[", "](url)", "Link"),
            ]
            .spacing(2)
            .align_y(iced::Alignment::Center);

            content = content.push(
                container(
                    container(
                        column![
                            input_row,
                            container(fmt_bar)
                                .padding(iced::Padding { top: 4.0, right: 0.0, bottom: 0.0, left: 0.0 })
                                .style(|_: &Theme| container::Style {
                                    border: iced::Border {
                                        color: Color::from_rgba(1.0, 1.0, 1.0, 0.04),
                                        width: 1.0,
                                        radius: 0.0.into(),
                                    },
                                    ..Default::default()
                                }),
                        ].spacing(4)
                    )
                    .width(Fill)
                    .padding([10, 16])
                    .style(|_: &Theme| container::Style {
                        background: Some(Color::from_rgba(0.12, 0.13, 0.15, 0.95).into()),
                        border: iced::Border {
                            color: Color::from_rgba(1.0, 1.0, 1.0, 0.08),
                            width: 1.0,
                            radius: 12.0.into(),
                        },
                        shadow: iced::Shadow {
                            color: Color::from_rgba(0.0, 0.0, 0.0, 0.5),
                            offset: iced::Vector::new(0.0, 4.0),
                            blur_radius: 20.0,
                        },
                        ..Default::default()
                    })
                )
                    .width(Fill)
                    .padding(iced::Padding { top: 6.0, right: 14.0, bottom: 10.0, left: 14.0 })
                    .style(|_: &Theme| container::Style {
                        ..Default::default()
                    }),
            );
        } else {
            content = content.push(
                center(
                    text("select a room")
                        .font(MONO)
                        .size(16)
                        .color(theme::TEXT_DIM),
                )
                .height(Fill),
            );
        }

        container(content)
            .width(Fill)
            .height(Fill)
            .style(|_: &Theme| container::Style {
                background: Some(theme::BG_MID.into()),
                ..Default::default()
            })
            .into()
    }

    /// Render a single message row: [time] [nick] | [body]
    fn view_message<'a>(&'a self, msg: &'a fluxchat_core::MessageInfo) -> Element<'a, Message> {
        // Timestamp
        let time_str = if self.timestamp_format == "off" {
            String::new()
        } else {
            let secs = (msg.timestamp / 1000) % 86400;
            let h = secs / 3600;
            let m = (secs % 3600) / 60;
            if self.timestamp_format == "full" {
                let s = secs % 60;
                format!("{h:02}:{m:02}:{s:02}")
            } else {
                format!("{h:02}:{m:02}")
            }
        };

        // System messages (join/part/state changes)
        if msg.event_id.starts_with("~") {
            return row![
                text(time_str)
                    .font(MONO)
                    .size(14)
                    .color(theme::TEXT_DIM)
                    .width(Length::Fixed(50.0)),
                text(&msg.body)
                    .font(MONO)
                    .size(14)
                    .color(theme::TEXT_DIM)
                    .wrapping(iced::widget::text::Wrapping::WordOrGlyph)
                    .width(Fill),
            ]
            .spacing(6)
            .width(Fill)
            .into();
        }

        // Emote messages
        if msg.is_emote {
            let nick_col = theme::nick_color(&msg.sender_name);
            return row![
                text(time_str)
                    .font(MONO)
                    .size(14)
                    .color(theme::TEXT_DIM)
                    .width(Length::Fixed(50.0)),
                text(format!("* {} {}", msg.sender_name, msg.body))
                    .font(MONO)
                    .size(15)
                    .color(nick_col)
                    .wrapping(iced::widget::text::Wrapping::WordOrGlyph)
                    .width(Fill),
            ]
            .spacing(6)
            .width(Fill)
            .into();
        }

        let time_width = if self.timestamp_format == "off" {
            0.0
        } else {
            50.0
        };

        let time = text(time_str)
            .font(MONO)
            .size(14)
            .color(theme::TEXT_DIM)
            .width(Length::Fixed(time_width));

        // Nick (right-aligned in fixed width)
        let nick_col = theme::nick_color(&msg.sender_name);
        let nick = container(
            text(&msg.sender_name)
                .font(MONO)
                .size(15)
                .color(nick_col),
        )
        .align_x(iced::alignment::Horizontal::Right)
        .width(Length::Fixed(100.0));

        // Avatar
        let msg_avatar: Element<'a, Message> = if let Some(handle) = msg.avatar_url.as_ref().and_then(|mxc| self.avatar_cache.get(mxc)) {
            let handle = handle.clone();
            container(
                iced::widget::image(handle)
                    .width(16)
                    .height(16)
                    .content_fit(iced::ContentFit::Cover),
            )
            .width(16)
            .height(16)
            .clip(true)
            .style(|_: &Theme| container::Style {
                border: iced::Border { radius: 8.0.into(), ..Default::default() },
                ..Default::default()
            })
            .into()
        } else {
            Space::new().width(16).height(16).into()
        };

        // Separator
        let sep = text("|").font(MONO).size(15).color(theme::TEXT_DIM);

        // Poll messages
        if let Some(ref poll) = msg.poll {
            let mut poll_col = column![
                text(&poll.question).font(MONO).size(15).color(theme::TEXT_BRIGHT),
            ]
            .spacing(4);

            for (answer_id, answer_text) in &poll.answers {
                let votes = poll.votes.get(answer_id).map(|v| v.len() as u32).unwrap_or(0);
                let pct = if poll.total_votes > 0 {
                    (votes as f32 / poll.total_votes as f32 * 100.0) as u32
                } else {
                    0
                };
                let is_my_vote = poll.my_vote.as_deref() == Some(answer_id);
                let label_color = if is_my_vote { theme::ACCENT } else { theme::TEXT_NORMAL };

                poll_col = poll_col.push(
                    row![
                        text(format!("{answer_text} ({votes})"))
                            .font(MONO)
                            .size(14)
                            .color(label_color),
                        text(format!("{pct}%"))
                            .font(MONO)
                            .size(13)
                            .color(theme::TEXT_DIM),
                    ]
                    .spacing(8),
                );
            }

            if poll.is_ended {
                poll_col = poll_col.push(
                    text("Poll ended")
                        .font(MONO)
                        .size(13)
                        .color(theme::TEXT_DIM),
                );
            }

            return row![time, nick, msg_avatar, sep, poll_col].spacing(6).width(Fill).align_y(iced::Alignment::Start).into();
        }

        // Image messages — show as block element (actual image loading comes later)
        if msg.image_mxc.is_some() && !msg.is_video {
            let image_label = format!("[image: {}]", msg.body);
            let img_element = text(image_label)
                .font(MONO)
                .size(15)
                .color(theme::ACCENT)
                .width(Fill);

            return row![time, nick, msg_avatar, sep, img_element].spacing(6).width(Fill).align_y(iced::Alignment::Center).into();
        }

        // Video messages
        if msg.is_video {
            let video_label = format!("[video: {}]", msg.body);
            let vid_element = text(video_label)
                .font(MONO)
                .size(15)
                .color(theme::PURPLE)
                .width(Fill);

            return row![time, nick, msg_avatar, sep, vid_element].spacing(6).width(Fill).align_y(iced::Alignment::Center).into();
        }

        // Body — use rich_text with HTML parsing when formatted body is available
        let body_element: Element<'a, Message> = if let Some(ref formatted) = msg.formatted_body {
            let spans = crate::html::html_to_spans(formatted);
            rich_text(spans)
                .font(MONO)
                .size(15)
                .wrapping(iced::widget::text::Wrapping::WordOrGlyph)
                .width(Fill)
                .on_link_click(Message::OpenUrl)
                .into()
        } else {
            text(&msg.body)
                .font(MONO)
                .size(15)
                .color(theme::TEXT_NORMAL)
                .wrapping(iced::widget::text::Wrapping::WordOrGlyph)
                .width(Fill)
                .into()
        };

        // Reply context
        if let Some(ref reply_body) = msg.reply_to_body {
            let reply_sender = msg.reply_to_sender.as_deref().unwrap_or("?");
            let reply_preview = container(
                text(format!("{reply_sender}: {reply_body}"))
                    .font(MONO)
                    .size(13)
                    .color(theme::TEXT_DIM),
            )
            .padding([2, 8])
            .style(|_: &Theme| container::Style {
                border: iced::Border {
                    color: theme::TEXT_DIM,
                    width: 1.0,
                    radius: 2.0.into(),
                },
                ..Default::default()
            });

            return column![
                row![time.width(Length::Fixed(time_width + 100.0 + 22.0 + 18.0)), reply_preview].spacing(6).width(Fill),
                row![
                    Space::new().width(time_width),
                    nick,
                    msg_avatar,
                    sep,
                    body_element,
                ]
                .spacing(6)
                .width(Fill)
                .align_y(iced::Alignment::Start),
            ]
            .spacing(2)
            .width(Fill)
            .into();
        }

        // Build message row + reactions + action buttons
        let msg_row: Element<'a, Message> = row![time, nick, msg_avatar, sep, body_element].spacing(6).width(Fill).align_y(iced::Alignment::Start).into();

        // URL preview cards
        let indent = time_width + 100.0 + 22.0 + 18.0;
        let urls = extract_urls(&msg.body);
        let preview_elements: Vec<Element<'a, Message>> = urls
            .iter()
            .filter_map(|url| self.url_previews.get(url))
            .map(|preview| self.view_url_preview(preview, indent))
            .collect();

        // Reactions
        if msg.reactions.is_empty() && preview_elements.is_empty() {
            return msg_row;
        }
        if msg.reactions.is_empty() {
            let mut col = column![msg_row].spacing(2).width(Fill);
            for el in preview_elements {
                col = col.push(el);
            }
            return col.into();
        }

        let mut reactions_row = row![
            Space::new().width(Length::Fixed(time_width + 100.0 + 22.0 + 18.0)),
        ]
        .spacing(4);

        for reaction in &msg.reactions {
            let event_id = msg.event_id.clone();
            let emoji = reaction.emoji.clone();
            let label = format!("{} {}", reaction.emoji, reaction.count);
            let is_mine = reaction.users.iter().any(|u| u == &self.user_id);
            let border_color = if is_mine { theme::ACCENT } else { theme::BORDER };

            reactions_row = reactions_row.push(
                button(text(label).font(MONO).size(13).color(theme::TEXT_MUTED))
                    .on_press(Message::ToggleReaction(event_id, emoji))
                    .padding([1, 6])
                    .style(move |_, _| button::Style {
                        background: Some(theme::BG_DARK.into()),
                        text_color: theme::TEXT_MUTED,
                        border: iced::Border {
                            color: border_color,
                            width: 1.0,
                            radius: 4.0.into(),
                        },
                        ..Default::default()
                    }),
            );
        }

        let mut col = column![msg_row].spacing(2).width(Fill);
        for el in preview_elements {
            col = col.push(el);
        }
        col = col.push(reactions_row);
        col.into()
    }

    /// URL preview card rendered below a message.
    fn view_url_preview<'a>(&self, preview: &'a fluxchat_core::UrlPreview, indent: f32) -> Element<'a, Message> {
        let mut content_col = column![].spacing(2);

        if let Some(ref site_name) = preview.site_name {
            content_col = content_col.push(
                text(site_name.as_str().to_uppercase())
                    .font(MONO).size(11).color(theme::TEXT_DIM)
            );
        }

        if let Some(ref title) = preview.title {
            content_col = content_col.push(
                text(title.as_str())
                    .font(MONO).size(14).color(theme::ACCENT)
                    .wrapping(iced::widget::text::Wrapping::None)
            );
        }

        if let Some(ref desc) = preview.description {
            let short = if desc.len() > 150 { &desc[..150] } else { desc };
            content_col = content_col.push(
                text(short)
                    .font(MONO).size(13).color(theme::TEXT_MUTED)
                    .wrapping(iced::widget::text::Wrapping::WordOrGlyph)
            );
        }

        let mut card_row = row![container(content_col).width(Fill).clip(true)].spacing(8);

        // OG image thumbnail
        if let Some(ref image_url) = preview.image_url {
            if let Some(handle) = self.url_preview_images.get(image_url) {
                card_row = card_row.push(
                    container(
                        iced::widget::image(handle.clone())
                            .width(60)
                            .height(60)
                            .content_fit(iced::ContentFit::Cover),
                    )
                    .width(60)
                    .height(60)
                    .clip(true)
                    .style(|_: &Theme| container::Style {
                        border: iced::Border { radius: 4.0.into(), ..Default::default() },
                        ..Default::default()
                    })
                );
            }
        }

        let url = preview.url.clone();
        let card = button(
            container(card_row)
                .padding([8, 12])
                .max_width(450)
                .style(|_: &Theme| container::Style {
                    background: Some(theme::BG_DARK.into()),
                    border: iced::Border {
                        color: theme::BORDER,
                        width: 1.0,
                        radius: 4.0.into(),
                    },
                    ..Default::default()
                }),
        )
        .on_press(Message::OpenUrl(url))
        .padding(0)
        .style(|_, _| button::Style {
            background: None,
            ..Default::default()
        });

        row![
            Space::new().width(Length::Fixed(indent)),
            card,
        ].spacing(6).width(Fill).into()
    }

    /// Single device/session row for the sessions settings tab.
    fn view_device_row<'a>(&self, device: &'a fluxchat_core::devices::DeviceInfo) -> Element<'a, Message> {
        let name = device.display_name.as_deref().unwrap_or("Unknown device");
        let verified_color = if device.is_verified { theme::GREEN } else { theme::YELLOW };
        let verified_label = if device.is_verified { "verified" } else { "unverified" };
        let device_id_short = if device.device_id.len() > 10 {
            &device.device_id[..10]
        } else {
            &device.device_id
        };

        let last_seen = device.last_seen_ts.map(|ts| {
            let now = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default()
                .as_millis() as u64;
            let ago = now.saturating_sub(ts) / 1000;
            if ago < 60 { "just now".to_string() }
            else if ago < 3600 { format!("{}m ago", ago / 60) }
            else if ago < 86400 { format!("{}h ago", ago / 3600) }
            else { format!("{}d ago", ago / 86400) }
        }).unwrap_or_else(|| "unknown".to_string());

        let border_color = if device.is_current { theme::ACCENT } else { theme::BORDER };
        let mut device_col = column![
            row![
                text(name).font(MONO).size(14).color(theme::TEXT_BRIGHT),
                if device.is_current {
                    text(" (this device)").font(MONO).size(12).color(theme::ACCENT)
                } else {
                    text("").size(1)
                },
            ].spacing(4),
            row![
                text(verified_label).font(MONO).size(12).color(verified_color),
                text(format!(" | {device_id_short}")).font(MONO).size(12).color(theme::TEXT_DIM),
                if let Some(ref ip) = device.last_seen_ip {
                    text(format!(" | {ip}")).font(MONO).size(12).color(theme::TEXT_DIM)
                } else {
                    text("").size(1)
                },
                text(format!(" | {last_seen}")).font(MONO).size(12).color(theme::TEXT_DIM),
            ].spacing(0),
        ].spacing(2);

        // Sign out button (not for current device)
        if !device.is_current {
            if self.settings_signout_device.as_deref() == Some(&device.device_id) {
                device_col = device_col.push(
                    row![
                        text_input("password", &self.settings_signout_password)
                            .on_input(Message::SettingsSignOutPasswordChanged)
                            .on_submit(Message::SettingsSignOutConfirm)
                            .secure(true)
                            .font(MONO).size(13).padding(4).width(200),
                        button(text("Confirm").font(MONO).size(12).color(theme::RED))
                            .on_press(Message::SettingsSignOutConfirm)
                            .padding([4, 8])
                            .style(|_, _| button::Style {
                                background: Some(theme::BG_LIGHT.into()),
                                text_color: theme::RED,
                                border: iced::Border { radius: 4.0.into(), ..Default::default() },
                                ..Default::default()
                            }),
                    ].spacing(8).align_y(iced::Alignment::Center),
                );
            } else {
                let did = device.device_id.clone();
                device_col = device_col.push(
                    button(text("Sign out").font(MONO).size(12).color(theme::RED))
                        .on_press(Message::SettingsSignOutDevice(did))
                        .padding([2, 8])
                        .style(|_, _| button::Style {
                            background: None,
                            text_color: theme::RED,
                            ..Default::default()
                        }),
                );
            }
        }

        container(device_col)
            .padding([8, 12])
            .width(Fill)
            .style(move |_: &Theme| container::Style {
                background: Some(theme::BG_MID.into()),
                border: iced::Border {
                    color: border_color,
                    width: 1.0,
                    radius: 4.0.into(),
                },
                ..Default::default()
            })
            .into()
    }

    /// Member list panel (right sidebar).
    fn view_member_list(&self) -> Element<Message> {
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

        let header = container(
            text(format!("Users -- {total}"))
                .font(MONO)
                .size(13)
                .color(theme::TEXT_DIM),
        )
        .padding([8, 12])
        .width(Fill)
        .style(|_: &Theme| container::Style {
            border: iced::Border {
                color: theme::BORDER,
                width: 1.0,
                radius: 0.0.into(),
            },
            ..Default::default()
        });

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

        container(scrollable(col).height(Fill))
            .width(180)
            .height(Fill)
            .style(|_: &Theme| container::Style {
                background: Some(theme::BG_DARK.into()),
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
    fn view_member_section<'a>(
        &'a self,
        label: &str,
        members: &[&'a fluxchat_core::MemberInfo],
    ) -> Element<'a, Message> {
        let mut col = column![
            container(
                text(format!("{label} -- {}", members.len()))
                    .font(MONO)
                    .size(12)
                    .color(theme::TEXT_DIM),
            )
            .padding([4, 12]),
        ]
        .spacing(0);

        for member in members.iter().take(30) {
            col = col.push(self.view_member_row(member));
        }

        col.into()
    }

    /// Single member row.
    fn view_member_row<'a>(&'a self, member: &'a fluxchat_core::MemberInfo) -> Element<'a, Message> {
        let nick_col = theme::nick_color(&member.display_name);
        let presence_color = match member.presence.as_str() {
            "online" => theme::GREEN,
            "idle" => theme::YELLOW,
            _ => theme::TEXT_DIM,
        };
        let user_id = member.user_id.clone();

        // Avatar — use cached image or fallback to initial
        let avatar: Element<'a, Message> = if let Some(handle) = member.avatar_url.as_ref().and_then(|mxc| self.avatar_cache.get(mxc)) {
            let handle = handle.clone();
            container(
                iced::widget::image(handle)
                    .width(20)
                    .height(20)
                    .content_fit(iced::ContentFit::Cover),
            )
            .width(20)
            .height(20)
            .clip(true)
            .style(|_: &Theme| container::Style {
                border: iced::Border { radius: 10.0.into(), ..Default::default() },
                ..Default::default()
            })
            .into()
        } else {
            let initial = member
                .display_name
                .chars()
                .next()
                .unwrap_or('?')
                .to_uppercase()
                .to_string();
            container(
                text(initial)
                    .font(MONO)
                    .size(11)
                    .color(theme::TEXT_MUTED),
            )
            .width(20)
            .height(20)
            .center_x(Fill)
            .center_y(Fill)
            .style(|_: &Theme| container::Style {
                background: Some(theme::BG_LIGHT.into()),
                border: iced::Border { radius: 10.0.into(), ..Default::default() },
                ..Default::default()
            })
            .into()
        };

        let mut member_row = row![
            avatar,
            text(&member.display_name)
                .font(MONO)
                .size(15)
                .color(nick_col),
            Space::new().width(Fill),
            container(Space::new())
                .width(8)
                .height(8)
                .style(move |_: &Theme| container::Style {
                    background: Some(presence_color.into()),
                    border: iced::Border {
                        radius: 4.0.into(),
                        ..Default::default()
                    },
                    ..Default::default()
                }),
        ]
        .spacing(4)
        .align_y(iced::Alignment::Center);

        // Power level badge
        if member.power_level >= 100 {
            member_row = member_row.push(
                Space::new().width(Fill),
            );
            member_row = member_row.push(
                container(Space::new())
                    .width(8)
                    .height(8)
                    .style(|_: &Theme| container::Style {
                        background: Some(theme::YELLOW.into()),
                        border: iced::Border {
                            radius: 4.0.into(),
                            ..Default::default()
                        },
                        ..Default::default()
                    }),
            );
        } else if member.power_level >= 50 {
            member_row = member_row.push(
                Space::new().width(Fill),
            );
            member_row = member_row.push(
                container(Space::new())
                    .width(8)
                    .height(8)
                    .style(|_: &Theme| container::Style {
                        background: Some(theme::ACCENT.into()),
                        border: iced::Border {
                            radius: 4.0.into(),
                            ..Default::default()
                        },
                        ..Default::default()
                    }),
            );
        }

        button(member_row)
            .on_press(Message::OpenModal(crate::state::Modal::Profile {
                user_id,
            }))
            .padding([2, 8])
            .width(Fill)
            .style(|_, _| button::Style {
                background: None,
                text_color: theme::TEXT_NORMAL,
                ..Default::default()
            })
            .into()
    }

    /// Render the active modal content.
    fn view_modal_content(&self) -> Element<Message> {
        let modal = match &self.modal {
            Some(m) => m,
            None => return text("").into(),
        };

        let modal_body: Element<Message> = match modal {
            crate::state::Modal::Settings { tab } => {
                let tab_owned = tab.clone();

                let make_tab_btn = |label: String, tab_id: String, current: &str| -> Element<Message> {
                    let is_active = current == tab_id;
                    let bg = if is_active {
                        theme::BG_LIGHT
                    } else {
                        theme::BG_MID
                    };
                    let color = if is_active {
                        theme::TEXT_BRIGHT
                    } else {
                        theme::TEXT_MUTED
                    };
                    button(text(label).font(MONO).size(15).color(color))
                        .on_press(Message::OpenModal(crate::state::Modal::Settings {
                            tab: tab_id,
                        }))
                        .padding([6, 16])
                        .style(move |_, _| button::Style {
                            background: Some(bg.into()),
                            text_color: color,
                            border: iced::Border {
                                radius: 4.0.into(),
                                ..Default::default()
                            },
                            ..Default::default()
                        })
                        .into()
                };

                let tabs = row![
                    make_tab_btn("Account".into(), "account".into(), &tab_owned),
                    make_tab_btn("Sessions".into(), "sessions".into(), &tab_owned),
                    make_tab_btn("Security".into(), "security".into(), &tab_owned),
                ]
                .spacing(4);

                let tab_content: Element<Message> = match tab_owned.as_str() {
                    "account" => {
                        let display = self.display_name.as_deref().unwrap_or(&self.user_id);

                        // Avatar section
                        let avatar_element: Element<Message> = if let Some(ref data) = self.settings_avatar_data {
                            let handle = iced::widget::image::Handle::from_bytes(data.clone());
                            container(
                                iced::widget::image(handle)
                                    .width(64)
                                    .height(64)
                                    .content_fit(iced::ContentFit::Cover),
                            )
                            .width(64)
                            .height(64)
                            .clip(true)
                            .style(|_: &Theme| container::Style {
                                border: iced::Border { radius: 32.0.into(), ..Default::default() },
                                ..Default::default()
                            })
                            .into()
                        } else {
                            let initial = display.chars().next().unwrap_or('?').to_uppercase().to_string();
                            container(
                                text(initial).font(MONO).size(24).color(theme::TEXT_MUTED),
                            )
                            .width(64)
                            .height(64)
                            .center(Fill)
                            .style(|_: &Theme| container::Style {
                                background: Some(theme::BG_LIGHT.into()),
                                border: iced::Border { radius: 32.0.into(), ..Default::default() },
                                ..Default::default()
                            })
                            .into()
                        };

                        let avatar_row = row![
                            avatar_element,
                            column![
                                button(text("change avatar").font(MONO).size(13).color(theme::ACCENT))
                                    .on_press(Message::SettingsUploadAvatar)
                                    .padding([2, 0])
                                    .style(|_, _| button::Style { background: None, text_color: theme::ACCENT, ..Default::default() }),
                                button(text("remove avatar").font(MONO).size(13).color(theme::TEXT_DIM))
                                    .on_press(Message::SettingsRemoveAvatar)
                                    .padding([2, 0])
                                    .style(|_, _| button::Style { background: None, text_color: theme::TEXT_DIM, ..Default::default() }),
                            ].spacing(2),
                        ].spacing(12).align_y(iced::Alignment::Center);

                        // Banner section
                        let banner_element: Element<Message> = if let Some(ref data) = self.settings_banner_data {
                            let handle = iced::widget::image::Handle::from_bytes(data.clone());
                            container(
                                iced::widget::image(handle)
                                    .width(Fill)
                                    .height(80)
                                    .content_fit(iced::ContentFit::Cover),
                            )
                            .width(Fill)
                            .height(80)
                            .clip(true)
                            .style(|_: &Theme| container::Style {
                                border: iced::Border { radius: 4.0.into(), ..Default::default() },
                                ..Default::default()
                            })
                            .into()
                        } else {
                            container(
                                text("no banner").font(MONO).size(13).color(theme::TEXT_DIM),
                            )
                            .width(Fill)
                            .height(80)
                            .center(Fill)
                            .style(|_: &Theme| container::Style {
                                background: Some(theme::BG_LIGHT.into()),
                                border: iced::Border { radius: 4.0.into(), ..Default::default() },
                                ..Default::default()
                            })
                            .into()
                        };

                        let banner_actions = row![
                            button(text("change banner").font(MONO).size(13).color(theme::ACCENT))
                                .on_press(Message::SettingsUploadBanner)
                                .padding([2, 0])
                                .style(|_, _| button::Style { background: None, text_color: theme::ACCENT, ..Default::default() }),
                            button(text("remove banner").font(MONO).size(13).color(theme::TEXT_DIM))
                                .on_press(Message::SettingsRemoveBanner)
                                .padding([2, 0])
                                .style(|_, _| button::Style { background: None, text_color: theme::TEXT_DIM, ..Default::default() }),
                        ].spacing(8);

                        let mut col = column![
                            text("Account Settings").font(MONO).size(16).color(theme::TEXT_BRIGHT),
                            rule::horizontal(1),
                            // Profile
                            text("PROFILE").font(MONO).size(12).color(theme::TEXT_DIM),
                            avatar_row,
                            text(format!("User ID: {}", self.user_id)).font(MONO).size(13).color(theme::TEXT_DIM),
                            // Display name
                            text("Display Name").font(MONO).size(13).color(theme::TEXT_DIM),
                            row![
                                text_input("display name", &self.settings_nick_input)
                                    .on_input(Message::SettingsNickChanged)
                                    .on_submit(Message::SettingsSaveNick)
                                    .font(MONO).size(14).padding(6),
                                button(text("Save").font(MONO).size(14).color(theme::TEXT_BRIGHT))
                                    .on_press(Message::SettingsSaveNick)
                                    .padding([6, 16])
                                    .style(|_, _| button::Style {
                                        background: Some(theme::ACCENT.into()),
                                        text_color: theme::TEXT_BRIGHT,
                                        border: iced::Border { radius: 4.0.into(), ..Default::default() },
                                        ..Default::default()
                                    }),
                            ].spacing(8).align_y(iced::Alignment::Center),
                            Space::new().height(4),
                            // Banner
                            text("BANNER").font(MONO).size(12).color(theme::TEXT_DIM),
                            banner_element,
                            banner_actions,
                        ].spacing(10);

                        if let Some(ref status) = self.settings_status {
                            col = col.push(text(status.as_str()).font(MONO).size(13).color(theme::GREEN));
                        }

                        col = col.push(Space::new().height(8));
                        col = col.push(rule::horizontal(1));
                        col = col.push(
                            button(text("Log Out").font(MONO).size(15).color(theme::RED))
                                .on_press(Message::Logout)
                                .padding([6, 16])
                                .style(|_, _| button::Style {
                                    background: Some(theme::BG_LIGHT.into()),
                                    text_color: theme::RED,
                                    border: iced::Border { radius: 4.0.into(), ..Default::default() },
                                    ..Default::default()
                                }),
                        );
                        scrollable(col).height(Length::Shrink).into()
                    }
                    "sessions" => {
                        let mut col = column![
                            text("Active Sessions").font(MONO).size(16).color(theme::TEXT_BRIGHT),
                            rule::horizontal(1),
                        ].spacing(10);

                        if self.settings_devices.is_empty() {
                            col = col.push(text("Loading devices...").font(MONO).size(14).color(theme::TEXT_DIM));
                        } else {
                            let other_count = self.settings_devices.iter().filter(|d| !d.is_current).count();

                            // Current session first
                            col = col.push(text("CURRENT SESSION").font(MONO).size(12).color(theme::TEXT_DIM));
                            for device in self.settings_devices.iter().filter(|d| d.is_current) {
                                col = col.push(self.view_device_row(device));
                            }

                            col = col.push(Space::new().height(4));
                            col = col.push(text(format!("OTHER SESSIONS ({other_count})")).font(MONO).size(12).color(theme::TEXT_DIM));

                            if other_count == 0 {
                                col = col.push(text("no other sessions").font(MONO).size(14).color(theme::TEXT_DIM));
                            }

                            for device in self.settings_devices.iter().filter(|d| !d.is_current) {
                                col = col.push(self.view_device_row(device));
                            }
                        }
                        scrollable(col).height(Length::Shrink).into()
                    }
                    "security" => {
                        let mut col = column![
                            text("Security & Encryption").font(MONO).size(16).color(theme::TEXT_BRIGHT),
                            rule::horizontal(1),
                        ].spacing(10);

                        // Cross-signing section
                        col = col.push(text("CROSS-SIGNING").font(MONO).size(12).color(theme::TEXT_DIM));
                        if let Some(ref sec) = self.security_state {
                            if sec.is_verified {
                                col = col.push(
                                    row![
                                        text("\u{2713}").font(MONO).size(14).color(theme::GREEN),
                                        text("verified").font(MONO).size(14).color(theme::GREEN),
                                        text("This device is cross-signed").font(MONO).size(14).color(theme::TEXT_DIM),
                                    ].spacing(8)
                                );
                            } else {
                                col = col.push(
                                    row![
                                        text("\u{2717}").font(MONO).size(14).color(theme::YELLOW),
                                        text("unverified").font(MONO).size(14).color(theme::YELLOW),
                                        text("Verify this device for secure messaging").font(MONO).size(14).color(theme::TEXT_DIM),
                                    ].spacing(8)
                                );
                            }
                        } else {
                            col = col.push(text("loading...").font(MONO).size(14).color(theme::TEXT_DIM));
                        }

                        col = col.push(Space::new().height(8));

                        // Key backup section
                        col = col.push(text("KEY BACKUP").font(MONO).size(12).color(theme::TEXT_DIM));
                        if let Some(ref sec) = self.security_state {
                            let server_color = if sec.backup_exists { theme::GREEN } else { theme::TEXT_DIM };
                            let local_color = if sec.backup_enabled { theme::GREEN } else { theme::TEXT_DIM };
                            col = col.push(
                                row![
                                    text("Server backup:").font(MONO).size(14).color(theme::TEXT_MUTED),
                                    text(if sec.backup_exists { "exists" } else { "none" })
                                        .font(MONO).size(14).color(server_color),
                                ].spacing(8)
                            );
                            col = col.push(
                                row![
                                    text("Local backup:").font(MONO).size(14).color(theme::TEXT_MUTED),
                                    text(if sec.backup_enabled { "enabled" } else { "disabled" })
                                        .font(MONO).size(14).color(local_color),
                                ].spacing(8)
                            );
                            col = col.push(
                                row![
                                    text("Recovery:").font(MONO).size(14).color(theme::TEXT_MUTED),
                                    text(sec.recovery_state.as_str()).font(MONO).size(14).color(theme::TEXT_NORMAL),
                                ].spacing(8)
                            );

                            // Enable backup button
                            if !sec.backup_enabled {
                                col = col.push(Space::new().height(4));
                                col = col.push(
                                    button(text("enable backup").font(MONO).size(14).color(theme::GREEN))
                                        .on_press(Message::SecurityEnableBackup)
                                        .padding([4, 12])
                                        .style(|_, _| button::Style {
                                            background: None,
                                            text_color: theme::GREEN,
                                            border: iced::Border {
                                                color: theme::GREEN,
                                                width: 1.0,
                                                radius: 4.0.into(),
                                            },
                                            ..Default::default()
                                        }),
                                );
                            }
                        } else {
                            col = col.push(text("loading...").font(MONO).size(14).color(theme::TEXT_DIM));
                        }

                        // Recovery key display (shown after enabling backup)
                        if let Some(ref key) = self.security_shown_recovery_key {
                            col = col.push(Space::new().height(8));
                            col = col.push(text("RECOVERY KEY \u{2014} SAVE THIS!").font(MONO).size(12).color(theme::YELLOW));
                            col = col.push(
                                container(
                                    text(key.as_str())
                                        .font(MONO).size(14).color(theme::TEXT_BRIGHT)
                                        .wrapping(iced::widget::text::Wrapping::WordOrGlyph)
                                )
                                .padding([12, 12])
                                .width(Fill)
                                .style(|_: &Theme| container::Style {
                                    background: Some(theme::BG_DEEP.into()),
                                    border: iced::Border {
                                        color: theme::YELLOW,
                                        width: 1.0,
                                        radius: 4.0.into(),
                                    },
                                    ..Default::default()
                                })
                            );
                            col = col.push(
                                text("Store this key safely. You'll need it to restore encrypted messages on new devices.")
                                    .font(MONO).size(12).color(theme::TEXT_DIM)
                            );
                        }

                        col = col.push(Space::new().height(8));

                        // Recover from backup
                        col = col.push(text("RECOVER FROM BACKUP").font(MONO).size(12).color(theme::TEXT_DIM));
                        col = col.push(
                            row![
                                text_input("Recovery key or passphrase", &self.security_recovery_key_input)
                                    .on_input(Message::SecurityRecoveryKeyInputChanged)
                                    .on_submit(Message::SecurityRecover)
                                    .font(MONO).size(14).padding(6),
                                button(text("recover").font(MONO).size(14).color(theme::ACCENT))
                                    .on_press(Message::SecurityRecover)
                                    .padding([6, 12])
                                    .style(|_, _| button::Style {
                                        background: None,
                                        text_color: theme::ACCENT,
                                        border: iced::Border {
                                            color: theme::BORDER,
                                            width: 1.0,
                                            radius: 4.0.into(),
                                        },
                                        ..Default::default()
                                    }),
                            ].spacing(6)
                        );

                        // Status message
                        if let Some(ref status) = self.settings_status {
                            col = col.push(text(status.as_str()).font(MONO).size(13).color(theme::TEXT_MUTED));
                        }

                        scrollable(col).height(Length::Shrink).into()
                    }
                    _ => text("Unknown tab")
                        .font(MONO)
                        .size(15)
                        .color(theme::TEXT_DIM)
                        .into(),
                };

                column![
                    row![
                        text("Settings")
                            .font(MONO)
                            .size(16)
                            .color(theme::TEXT_BRIGHT),
                        Space::new().width(Fill),
                        button(text("x").font(MONO).size(14).color(theme::TEXT_DIM))
                            .on_press(Message::CloseModal)
                            .padding([2, 6])
                            .style(|_, _| button::Style {
                                background: None,
                                ..Default::default()
                            }),
                    ]
                    .align_y(iced::Alignment::Center),
                    tabs,
                    rule::horizontal(1),
                    tab_content,
                ]
                .spacing(12)
                .width(500)
                .into()
            }

            crate::state::Modal::CreateRoom => {
                column![
                    row![
                        text("Create Room")
                            .font(MONO)
                            .size(16)
                            .color(theme::TEXT_BRIGHT),
                        Space::new().width(Fill),
                        button(text("x").font(MONO).size(14).color(theme::TEXT_DIM))
                            .on_press(Message::CloseModal)
                            .padding([2, 6])
                            .style(|_, _| button::Style {
                                background: None,
                                ..Default::default()
                            }),
                    ]
                    .align_y(iced::Alignment::Center),
                    text("Room Name")
                        .font(MONO)
                        .size(14)
                        .color(theme::TEXT_MUTED),
                    text_input("e.g. my-room", "")
                        .font(MONO)
                        .size(14)
                        .padding(8),
                    row![
                        text("Public").font(MONO).size(14).color(theme::TEXT_MUTED),
                        Space::new().width(8),
                        text("[toggle]")
                            .font(MONO)
                            .size(14)
                            .color(theme::TEXT_DIM),
                        Space::new().width(16),
                        text("Encrypted")
                            .font(MONO)
                            .size(14)
                            .color(theme::TEXT_MUTED),
                        Space::new().width(8),
                        text("[toggle]")
                            .font(MONO)
                            .size(14)
                            .color(theme::TEXT_DIM),
                    ],
                    button(
                        text("Create")
                            .font(MONO)
                            .size(14)
                            .color(theme::BG_DEEP),
                    )
                    .padding([8, 24])
                    .style(|_, _| button::Style {
                        background: Some(theme::ACCENT.into()),
                        text_color: theme::BG_DEEP,
                        border: iced::Border {
                            radius: 4.0.into(),
                            ..Default::default()
                        },
                        ..Default::default()
                    }),
                ]
                .spacing(12)
                .width(400)
                .into()
            }

            crate::state::Modal::Profile { user_id } => {
                // Look up member info from the current room
                let member = self
                    .selected_room_id
                    .as_ref()
                    .and_then(|rid| self.members_by_room.get(rid))
                    .and_then(|members| members.iter().find(|m| m.user_id == *user_id));

                let display_name = member
                    .map(|m| m.display_name.as_str())
                    .unwrap_or(user_id.as_str());
                let nick_col = theme::nick_color(display_name);
                let power = member.map(|m| m.power_level).unwrap_or(0);
                let role = member.map(|m| m.role.as_str()).unwrap_or("Member");
                let is_ignored = self.ignored_users.contains(user_id);

                let mut profile_col = column![
                    row![
                        text(display_name)
                            .font(MONO)
                            .size(16)
                            .color(nick_col),
                        Space::new().width(Fill),
                        button(text("x").font(MONO).size(14).color(theme::TEXT_DIM))
                            .on_press(Message::CloseModal)
                            .padding([2, 6])
                            .style(|_, _| button::Style {
                                background: None,
                                ..Default::default()
                            }),
                    ]
                    .align_y(iced::Alignment::Center),
                    text(user_id.as_str())
                        .font(MONO)
                        .size(14)
                        .color(theme::TEXT_MUTED),
                    rule::horizontal(1),
                    row![
                        text("Role: ")
                            .font(MONO)
                            .size(14)
                            .color(theme::TEXT_DIM),
                        text(role)
                            .font(MONO)
                            .size(14)
                            .color(theme::TEXT_NORMAL),
                    ],
                    row![
                        text("Power: ")
                            .font(MONO)
                            .size(14)
                            .color(theme::TEXT_DIM),
                        text(power.to_string())
                            .font(MONO)
                            .size(14)
                            .color(theme::TEXT_NORMAL),
                    ],
                ]
                .spacing(8);

                if is_ignored {
                    profile_col = profile_col.push(
                        text("(ignored)")
                            .font(MONO)
                            .size(14)
                            .color(theme::RED),
                    );
                }

                profile_col.width(350).into()
            }

            crate::state::Modal::Verification { info } => {
                let mut col = column![
                    row![
                        text("Verify Device")
                            .font(MONO)
                            .size(16)
                            .color(theme::TEXT_BRIGHT),
                        Space::new().width(Fill),
                        button(text("x").font(MONO).size(14).color(theme::TEXT_DIM))
                            .on_press(Message::CloseModal)
                            .padding([2, 6])
                            .style(|_, _| button::Style {
                                background: None,
                                ..Default::default()
                            }),
                    ]
                    .align_y(iced::Alignment::Center),
                    text(format!("User: {}", info.other_user_id))
                        .font(MONO)
                        .size(15)
                        .color(theme::TEXT_NORMAL),
                    text(format!("State: {}", info.state))
                        .font(MONO)
                        .size(14)
                        .color(theme::TEXT_MUTED),
                    rule::horizontal(1),
                    text("Compare these emoji:")
                        .font(MONO)
                        .size(15)
                        .color(theme::TEXT_NORMAL),
                ]
                .spacing(8);

                // Emoji grid (3 columns)
                if let Some(ref emojis) = info.emojis {
                    let mut emoji_rows: Vec<Element<Message>> = Vec::new();
                    for chunk in emojis.chunks(3) {
                        let mut r = row![].spacing(16);
                        for emoji in chunk {
                            r = r.push(
                                column![
                                    text(&emoji.symbol).size(28),
                                    text(&emoji.description)
                                        .font(MONO)
                                        .size(12)
                                        .color(theme::TEXT_DIM),
                                ]
                                .spacing(2)
                                .align_x(iced::Alignment::Center),
                            );
                        }
                        emoji_rows.push(r.into());
                    }
                    for emoji_row in emoji_rows {
                        col = col.push(emoji_row);
                    }
                }

                col.width(400).into()
            }

            crate::state::Modal::Lightbox { url } => {
                column![
                    row![
                        text("Image Viewer")
                            .font(MONO)
                            .size(16)
                            .color(theme::TEXT_BRIGHT),
                        Space::new().width(Fill),
                        button(text("x").font(MONO).size(14).color(theme::TEXT_DIM))
                            .on_press(Message::CloseModal)
                            .padding([2, 6])
                            .style(|_, _| button::Style {
                                background: None,
                                ..Default::default()
                            }),
                    ]
                    .align_y(iced::Alignment::Center),
                    text("[image viewer]")
                        .font(MONO)
                        .size(14)
                        .color(theme::TEXT_DIM),
                    text(url.as_str())
                        .font(MONO)
                        .size(13)
                        .color(theme::TEXT_MUTED),
                ]
                .spacing(12)
                .width(600)
                .into()
            }

            crate::state::Modal::Forward { body, .. } => {
                column![
                    row![
                        text("Forward Message")
                            .font(MONO)
                            .size(16)
                            .color(theme::TEXT_BRIGHT),
                        Space::new().width(Fill),
                        button(text("x").font(MONO).size(14).color(theme::TEXT_DIM))
                            .on_press(Message::CloseModal)
                            .padding([2, 6])
                            .style(|_, _| button::Style {
                                background: None,
                                ..Default::default()
                            }),
                    ]
                    .align_y(iced::Alignment::Center),
                    text("[forward modal]")
                        .font(MONO)
                        .size(14)
                        .color(theme::TEXT_DIM),
                    text(body.as_str())
                        .font(MONO)
                        .size(14)
                        .color(theme::TEXT_MUTED),
                ]
                .spacing(12)
                .width(450)
                .into()
            }

            crate::state::Modal::RoomSettings => {
                column![
                    row![
                        text("Room Settings")
                            .font(MONO)
                            .size(16)
                            .color(theme::TEXT_BRIGHT),
                        Space::new().width(Fill),
                        button(text("x").font(MONO).size(14).color(theme::TEXT_DIM))
                            .on_press(Message::CloseModal)
                            .padding([2, 6])
                            .style(|_, _| button::Style {
                                background: None,
                                ..Default::default()
                            }),
                    ]
                    .align_y(iced::Alignment::Center),
                    text("[room settings]")
                        .font(MONO)
                        .size(14)
                        .color(theme::TEXT_DIM),
                ]
                .spacing(12)
                .width(450)
                .into()
            }
        };

        container(modal_body)
            .padding(24)
            .style(|_: &Theme| container::Style {
                background: Some(theme::BG_DARK.into()),
                border: iced::Border {
                    color: theme::BORDER,
                    width: 1.0,
                    radius: 8.0.into(),
                },
                ..Default::default()
            })
            .into()
    }

    /// IRC-style status bar at the bottom.
    fn view_status_bar(&self) -> Element<Message> {
        let mut bar = row![].spacing(0).align_y(iced::Alignment::Center);

        let sep = || text(" | ").font(MONO).size(14).color(theme::TEXT_DIM);

        // Settings gear
        bar = bar.push(
            button(text("*").font(MONO).size(15).color(theme::TEXT_DIM))
                .on_press(Message::OpenModal(crate::state::Modal::Settings {
                    tab: "account".into(),
                }))
                .padding([2, 8])
                .style(|_, _| button::Style {
                    background: None,
                    ..Default::default()
                }),
        );

        // User display name
        let display = self
            .display_name
            .as_deref()
            .unwrap_or(&self.user_id);
        bar = bar.push(text(display).font(MONO).size(14).color(theme::TEXT_MUTED));

        // Current room info
        if let Some(room_id) = &self.selected_room_id {
            let room = self.rooms.iter().find(|r| r.room_id == *room_id);
            let room_name = room.map(|r| r.name.as_str()).unwrap_or(room_id);

            bar = bar.push(sep());
            bar = bar.push(text(format!("#{room_name}")).font(MONO).size(14).color(theme::TEXT_NORMAL));

            // Nick count
            if let Some(members) = self.members_by_room.get(room_id) {
                bar = bar.push(sep());
                bar = bar.push(
                    text(format!("{} nicks", members.len()))
                        .font(MONO)
                        .size(14)
                        .color(theme::TEXT_MUTED),
                );
            }

            // Encryption status
            if let Some(r) = room {
                if r.is_encrypted {
                    bar = bar.push(sep());
                    bar = bar.push(
                        text("encrypted")
                            .font(MONO)
                            .size(14)
                            .color(theme::GREEN),
                    );
                }
            }
        }

        bar = bar.push(Space::new().width(Fill));

        // Sync lag
        let lag_color = if self.sync_lag_ms < 200 {
            theme::GREEN
        } else if self.sync_lag_ms < 1000 {
            theme::YELLOW
        } else {
            theme::RED
        };
        bar = bar.push(
            text(format!("{}ms", self.sync_lag_ms))
                .font(MONO)
                .size(13)
                .color(lag_color),
        );

        container(bar)
            .width(Fill)
            .padding([2, 8])
            .style(|_: &Theme| container::Style {
                background: Some(theme::BG_DARK.into()),
                border: iced::Border {
                    color: theme::BORDER,
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

        let keys = iced::keyboard::listen().map(|event| {
            if let iced::keyboard::Event::KeyPressed {
                key, modifiers, ..
            } = event
            {
                // Alt+1-9: switch tabs
                if modifiers.alt() {
                    if let iced::keyboard::Key::Character(ref c) = key {
                        if let Ok(n) = c.as_str().parse::<usize>() {
                            if n >= 1 && n <= 9 {
                                return Message::SwitchTab(n - 1);
                            }
                        }
                    }
                }

                // Escape: close modals/panels/completions
                if key == iced::keyboard::Key::Named(iced::keyboard::key::Named::Escape) {
                    return Message::Escape;
                }
            }
            Message::Noop
        });

        // Timeline updates from background diff watchers
        let timeline_sub = Subscription::run(timeline_update_stream);

        Subscription::batch([tick, keys, timeline_sub])
    }

    /// Application theme — dark.
    pub fn theme(&self) -> Theme {
        Theme::Dark
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
                    let mut guard = mutex.lock().unwrap();
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
