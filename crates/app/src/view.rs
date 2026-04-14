use iced::widget::{
    button, center, column, container, mouse_area, opaque, row, rule, scrollable, stack,
    text, text_input, Space,
};
use iced::{Color, Element, Fill, Font, Length, Subscription, Theme};

/// Fixed ID for the main message input so we can refocus it programmatically.
pub fn input_id() -> iced::widget::Id {
    iced::widget::Id::new("main_input")
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

pub(crate) const EMOJI_GRID_COLUMNS: usize = 8;
pub(crate) const EMOJI_PANEL_HEIGHT: f32 = 280.0;
pub(crate) const EMOJI_CELL_SIZE: f32 = 36.0;
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
            background: Color::from_rgba(1.0, 1.0, 1.0, scroller_alpha).into(),
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
pub(crate) const SANS: Font = Font::DEFAULT;
pub(crate) const SANS_BOLD: Font = Font { weight: iced::font::Weight::Bold, ..Font::DEFAULT };

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
    let mut names = LEAKED_NAMES.lock().unwrap();
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
    sorted.sort_by(|a, b| a.to_lowercase().cmp(&b.to_lowercase()));
    sorted
}

impl FluxChat {
    /// Get the user's chosen message font.
    pub(crate) fn msg_font(&self) -> Font {
        font_by_name(&self.msg_font_family)
    }
    /// Get the user's chosen message font (bold).
    pub(crate) fn msg_font_bold(&self) -> Font {
        font_bold_by_name(&self.msg_font_family)
    }
    /// Get the user's chosen sidebar font.
    pub(crate) fn sidebar_font(&self) -> Font {
        font_by_name(&self.sidebar_font_family)
    }
    /// Get the user's chosen sidebar font (bold).
    pub(crate) fn sidebar_font_bold(&self) -> Font {
        font_bold_by_name(&self.sidebar_font_family)
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
        let user_ids: Vec<&str> = msg.read_receipt_user_ids.iter()
            .filter(|uid| uid.as_str() != self.user_id)
            .map(String::as_str)
            .collect();
        if user_ids.is_empty() {
            return None;
        }
        let members = self.selected_room_id.as_ref()
            .and_then(|rid| self.members_by_room.get(rid));
        let mut avatars_row = row![].spacing(2);
        let shown = user_ids.len().min(5);
        for uid in user_ids.iter().take(shown) {
            let member = members.and_then(|m| m.iter().find(|m| m.user_id == *uid));
            let avatar_mxc = member.and_then(|m| m.avatar_url.as_deref());
            let display = member.map(|m| m.display_name.as_str())
                .unwrap_or_else(|| uid.trim_start_matches('@').split(':').next().unwrap_or(uid));
            avatars_row = avatars_row.push(
                self.render_avatar_with_initial(avatar_mxc, display, 14.0),
            );
        }
        if user_ids.len() > 5 {
            avatars_row = avatars_row.push(
                text(format!("+{}", user_ids.len() - 5))
                    .font(self.sidebar_font()).size(10).color(theme::colors().text_dim),
            );
        }
        // Offset to align with message content area (timestamp + nick + avatar + separator)
        Some(
            row![
                Space::new().width(Length::Fixed(170.0)),
                Space::new().width(Fill),
                avatars_row,
            ]
            .align_y(iced::Alignment::Center)
            .padding([0, 8])
            .into(),
        )
    }

    /// Render a cached avatar image, or a blank space fallback.
    pub(crate) fn render_avatar<'a>(&self, mxc: Option<&str>, size: f32) -> Element<'a, Message> {
        if let Some(handle) = mxc.and_then(|m| self.avatar_cache.get(m)) {
            container(
                iced::widget::image(handle.clone())
                    .width(size).height(size)
                    .content_fit(iced::ContentFit::Cover),
            )
            .width(size).height(size).clip(true)
            .style(move |_: &Theme| container::Style {
                border: iced::Border { radius: (size / 2.0).into(), ..Default::default() },
                ..Default::default()
            })
            .into()
        } else {
            Space::new().width(size).height(size).into()
        }
    }

    /// Render a cached avatar with an initial letter fallback.
    pub(crate) fn render_avatar_with_initial<'a>(&self, mxc: Option<&str>, name: &str, size: f32) -> Element<'a, Message> {
        if let Some(handle) = mxc.and_then(|m| self.avatar_cache.get(m)) {
            container(
                iced::widget::image(handle.clone())
                    .width(size).height(size)
                    .content_fit(iced::ContentFit::Cover),
            )
            .width(size).height(size).clip(true)
            .style(move |_: &Theme| container::Style {
                border: iced::Border { radius: (size / 2.0).into(), ..Default::default() },
                ..Default::default()
            })
            .into()
        } else {
            let initial = name.chars().next().unwrap_or('?').to_uppercase().to_string();
            container(
                text(initial).font(self.sidebar_font()).size(size * 0.45).color(theme::colors().text_muted)
            )
            .width(size).height(size)
            .center_x(size).center_y(size)
            .style(move |_: &Theme| container::Style {
                background: Some(theme::colors().bg_light.into()),
                border: iced::Border { radius: (size / 2.0).into(), ..Default::default() },
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
        let main_content = self.view_content();

        // Drag handle between room list and content
        let drag_handle: Element<Message> = mouse_area(
            container(Space::new().width(3).height(Fill))
                .style(|_: &Theme| container::Style {
                    background: Some(theme::colors().border.into()),
                    ..Default::default()
                })
        )
        .on_press(Message::SidebarDragStart)
        .interaction(iced::mouse::Interaction::ResizingHorizontally)
        .into();

        let mut main_area = row![
            sidebar,
            container(room_list).width(Length::Fixed(self.sidebar_width)),
            drag_handle,
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
                background: Some(theme::colors().bg_deep.into()),
                ..Default::default()
            });

        let mut layers: Vec<Element<Message>> = vec![base.into()];
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
        // Search modal overlay (same style as quick switcher)
        if self.search_open {
            let mut results_col = column![].spacing(0);
            let selected_idx = self.search_selected;
            for (i, result) in self.search_results.iter().take(20).enumerate() {
                let local_secs = local_time_of_day(result.timestamp);
                let h = local_secs / 3600;
                let m = (local_secs % 3600) / 60;
                let time_str = format!("{h:02}:{m:02}");
                let nick_col = theme::nick_color(&result.sender_name);
                let body_line = result.body.lines().next().unwrap_or("");
                let body_trunc = if body_line.len() > 100 {
                    let mut end = 97;
                    while !body_line.is_char_boundary(end) { end -= 1; }
                    format!("{}...", &body_line[..end])
                } else {
                    body_line.to_string()
                };
                let is_selected = selected_idx == Some(i);
                let bg = if is_selected {
                    Some(Color::from_rgba(1.0, 1.0, 1.0, 0.1).into())
                } else {
                    None
                };
                results_col = results_col.push(
                    button(
                        row![
                            text(time_str).font(self.sidebar_font()).size(12).color(theme::colors().text_dim),
                            text(&result.sender_name).font(self.sidebar_font()).size(13).color(nick_col)
                                .wrapping(iced::widget::text::Wrapping::None),
                            text(body_trunc).font(self.sidebar_font()).size(13).color(theme::colors().text_normal)
                                .wrapping(iced::widget::text::Wrapping::None),
                        ].spacing(8).align_y(iced::Alignment::Center),
                    )
                    .on_press(Message::SearchSelectResult(i))
                    .padding([6, 14])
                    .width(Fill)
                    .style(move |_, status| button::Style {
                        background: if matches!(status, button::Status::Hovered) {
                            Some(Color::from_rgba(1.0, 1.0, 1.0, 0.08).into())
                        } else {
                            bg
                        },
                        ..Default::default()
                    }),
                );
            }

            let search_card = container(
                column![
                    text_input("Search messages...", &self.search_query)
                        .id(search_input_id())
                        .on_input(Message::SearchQueryChanged)
                        .on_submit(Message::SearchSubmit)
                        .font(self.sidebar_font())
                        .size(16)
                        .padding(10),
                    scrollable(results_col)
                        .height(Length::Shrink)
                        .direction(iced::widget::scrollable::Direction::Vertical(
                            iced::widget::scrollable::Scrollbar::new().width(4).scroller_width(4),
                        ))
                        .style(thin_scrollbar),
                ].spacing(0),
            )
            .max_height(400.0)
            .width(Length::Fixed(560.0))
            .style(|_: &Theme| container::Style {
                background: Some(theme::colors().bg_dark.into()),
                border: iced::Border {
                    color: theme::colors().accent,
                    width: 1.0,
                    radius: 8.0.into(),
                },
                shadow: iced::Shadow {
                    color: Color::from_rgba(0.0, 0.0, 0.0, 0.5),
                    offset: iced::Vector::new(0.0, 4.0),
                    blur_radius: 20.0,
                },
                ..Default::default()
            });

            layers.push(
                opaque(
                    mouse_area(
                        container(
                            column![
                                Space::new().height(80),
                                search_card,
                            ]
                        )
                        .width(Fill)
                        .center_x(Fill)
                        .height(Fill)
                        .style(|_: &Theme| container::Style {
                            background: Some(Color::from_rgba(0.0, 0.0, 0.0, 0.4).into()),
                            ..Default::default()
                        }),
                    )
                    .on_press(Message::ToggleSearch),
                ),
            );
        }

        // Quick switcher overlay
        if self.quick_switcher_open {
            let matches = self.quick_switcher_matches();
            let mut results_col = column![].spacing(0);
            for (i, room_id) in matches.iter().enumerate() {
                if let Some(room) = self.rooms.iter().find(|r| r.room_id == *room_id) {
                    let is_selected = i == self.quick_switcher_selected;
                    let bg = if is_selected {
                        Some(Color::from_rgba(1.0, 1.0, 1.0, 0.1).into())
                    } else {
                        None
                    };
                    let space_prefix = room.space_name.as_ref()
                        .map(|s| format!("{s}/"))
                        .unwrap_or_default();
                    let rid = room_id.clone();
                    let avatar_el: Element<Message> = if let Some(handle) = room.avatar_url.as_ref().and_then(|mxc| self.avatar_cache.get(mxc)) {
                        container(
                            iced::widget::image(handle.clone())
                                .width(20).height(20)
                                .content_fit(iced::ContentFit::Cover),
                        )
                        .width(20).height(20).clip(true)
                        .style(|_: &Theme| container::Style {
                            border: iced::Border { radius: 4.0.into(), ..Default::default() },
                            ..Default::default()
                        })
                        .into()
                    } else {
                        let initial = room.name.chars().next().unwrap_or('#').to_uppercase().to_string();
                        container(
                            text(initial).font(self.sidebar_font()).size(11).color(theme::colors().text_dim),
                        )
                        .width(20).height(20)
                        .center_x(20).center_y(20)
                        .style(|_: &Theme| container::Style {
                            background: Some(theme::colors().bg_light.into()),
                            border: iced::Border { radius: 4.0.into(), ..Default::default() },
                            ..Default::default()
                        })
                        .into()
                    };
                    results_col = results_col.push(
                        button(
                            row![
                                avatar_el,
                                text(format!("{space_prefix}{}", room.name))
                                    .font(self.sidebar_font()).size(14).color(theme::colors().text_normal),
                            ].spacing(10).align_y(iced::Alignment::Center),
                        )
                        .on_press(Message::SelectRoom(rid))
                        .padding([8, 14])
                        .width(Fill)
                        .style(move |_, status| button::Style {
                            background: if matches!(status, button::Status::Hovered) {
                                Some(Color::from_rgba(1.0, 1.0, 1.0, 0.08).into())
                            } else {
                                bg
                            },
                            ..Default::default()
                        }),
                    );
                }
            }
            let switcher = container(
                column![
                    text_input("Switch to room...", &self.quick_switcher_query)
                        .id(quick_switcher_id())
                        .on_input(Message::QuickSwitcherQuery)
                        .on_submit(Message::QuickSwitcherSelect)
                        .font(self.sidebar_font())
                        .size(16)
                        .padding(10),
                    scrollable(results_col)
                        .height(Length::Shrink)
                        .direction(iced::widget::scrollable::Direction::Vertical(
                            iced::widget::scrollable::Scrollbar::new().width(4).scroller_width(4),
                        ))
                        .style(thin_scrollbar),
                ].spacing(0),
            )
            .max_height(400.0)
            .width(Length::Fixed(500.0))
            .style(|_: &Theme| container::Style {
                background: Some(theme::colors().bg_dark.into()),
                border: iced::Border {
                    color: theme::colors().accent,
                    width: 1.0,
                    radius: 8.0.into(),
                },
                shadow: iced::Shadow {
                    color: Color::from_rgba(0.0, 0.0, 0.0, 0.5),
                    offset: iced::Vector::new(0.0, 4.0),
                    blur_radius: 20.0,
                },
                ..Default::default()
            });

            layers.push(
                opaque(
                    mouse_area(
                        container(
                            column![
                                Space::new().height(80),
                                switcher,
                            ]
                        )
                        .width(Fill)
                        .center_x(Fill)
                        .height(Fill)
                        .style(|_: &Theme| container::Style {
                            background: Some(Color::from_rgba(0.0, 0.0, 0.0, 0.4).into()),
                            ..Default::default()
                        }),
                    )
                    .on_press(Message::ToggleQuickSwitcher),
                ),
            );
        }
        // Context menu overlay
        if let Some(ref ctx) = self.context_menu {
            let menu_items = self.view_context_menu(ctx);
            layers.push(
                opaque(
                    mouse_area(
                        container(
                            container(menu_items)
                                .padding([4, 0])
                                .style(|_: &Theme| container::Style {
                                    background: Some(theme::colors().bg_dark.into()),
                                    border: iced::Border {
                                        color: theme::colors().border,
                                        width: 1.0,
                                        radius: 6.0.into(),
                                    },
                                    shadow: iced::Shadow {
                                        color: Color::from_rgba(0.0, 0.0, 0.0, 0.5),
                                        offset: iced::Vector::new(0.0, 2.0),
                                        blur_radius: 12.0,
                                    },
                                    ..Default::default()
                                }),
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
        let mut col = column![].spacing(0).width(180);

        match ctx {
            crate::state::ContextMenu::Room { room_id, is_dm } => {
                let rid = room_id.clone();
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
            crate::state::ContextMenu::Space { space_id } => {
                let sid = space_id.clone();
                col = col.push(self.context_menu_item(
                    "Space Details", SVG_INFO, sid.clone(), "space_details",
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
        let action = action.to_string();
        let handle = iced::widget::svg::Handle::from_memory(icon);
        button(
            row![
                iced::widget::svg(handle)
                    .width(14).height(14)
                    .style(|_, _| iced::widget::svg::Style { color: Some(theme::colors().text_dim) }),
                text(label).font(self.sidebar_font()).size(13).color(theme::colors().text_normal),
            ].spacing(8).align_y(iced::Alignment::Center),
        )
        .on_press(Message::ContextMenuAction(action, target))
        .padding([6, 12])
        .width(Fill)
        .style(|_, status| button::Style {
            background: if matches!(status, button::Status::Hovered) {
                Some(Color::from_rgba(1.0, 1.0, 1.0, 0.08).into())
            } else {
                None
            },
            ..Default::default()
        })
        .into()
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
        .padding([6, 12])
        .width(Fill)
        .style(|_, status| button::Style {
            background: if matches!(status, button::Status::Hovered) {
                Some(Color::from_rgba(1.0, 0.2, 0.2, 0.1).into())
            } else {
                None
            },
            ..Default::default()
        })
        .into()
    }

    fn context_menu_divider(&self) -> Element<'_, Message> {
        container(rule::horizontal(1).style(|_: &Theme| rule::Style {
            color: Color::from_rgba(1.0, 1.0, 1.0, 0.08),
            radius: 0.0.into(),
            fill_mode: rule::FillMode::Full,
            snap: true,
        }))
        .padding([2, 8])
        .into()
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
            let depth = node.depth;
            let space_id = space.room_id.clone();
            let is_selected = self.selected_space_id.as_deref() == Some(&space.room_id);
            let radius = if is_selected { 12.0 } else { 20.0 };
            let opacity = if is_selected { 1.0 } else { 0.7 };
            let glow = if is_selected {
                iced::Shadow {
                    color: Color::from_rgba(0.35, 0.55, 0.9, 0.6),
                    offset: iced::Vector::ZERO,
                    blur_radius: 10.0,
                }
            } else {
                iced::Shadow::default()
            };

            // Scale icon size down for sub-spaces
            let icon_size: u32 = if depth == 0 { 40 } else { 32 };
            let font_size: u32 = if depth == 0 { 13 } else { 11 };
            let indent: u32 = (depth as u32).min(3) * 4;

            let icon_content: Element<Message> = if let Some(handle) = space.avatar_url.as_ref().and_then(|mxc| self.avatar_cache.get(mxc)) {
                container(
                    iced::widget::image(handle.clone())
                        .width(icon_size).height(icon_size)
                        .content_fit(iced::ContentFit::Cover),
                )
                .width(icon_size).height(icon_size)
                .clip(true)
                .style(move |_: &Theme| container::Style {
                    border: iced::Border {
                        radius: radius.into(),
                        ..Default::default()
                    },
                    shadow: glow,
                    ..Default::default()
                })
                .into()
            } else {
                let initials: String = space.name.split_whitespace()
                    .take(2)
                    .map(|w| w.chars().next().unwrap_or('?'))
                    .collect::<String>()
                    .to_uppercase();
                container(
                    text(initials).font(self.sidebar_font()).size(font_size).color(Color { a: opacity, ..theme::colors().text_muted }),
                )
                .width(icon_size).height(icon_size)
                .center(Fill)
                .style(move |_: &Theme| container::Style {
                    background: Some(theme::colors().bg_mid.into()),
                    border: iced::Border {
                        radius: radius.into(),
                        ..Default::default()
                    },
                    shadow: glow,
                    ..Default::default()
                })
                .into()
            };

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

            let btn_el: Element<Message> = if indent > 0 {
                row![Space::new().width(indent), btn].into()
            } else {
                btn.into()
            };

            col = col.push(btn_el);
        }

        container(scrollable(col).direction(iced::widget::scrollable::Direction::Vertical(iced::widget::scrollable::Scrollbar::new().width(4).scroller_width(4))).style(thin_scrollbar))
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
        let mut by_space_id: std::collections::HashMap<String, Vec<&fluxchat_core::RoomInfo>> =
            std::collections::HashMap::new();
        let mut dms: Vec<&fluxchat_core::RoomInfo> = Vec::new();

        for room in &self.rooms {
            // Filter by selected space (includes rooms from descendant sub-spaces)
            if let Some(ref selected_sid) = self.selected_space_id {
                let dominated = self.space_descendants.get(selected_sid.as_str())
                    .map_or(false, |desc| room.space_id.as_ref().map_or(false, |rid| desc.contains(rid)));
                if !dominated {
                    continue;
                }
            }
            if room.is_favourite {
                favourites.push(room);
            }
            if room.is_direct {
                dms.push(room);
            } else if let Some(ref sid) = room.space_id {
                by_space_id.entry(sid.clone()).or_default().push(room);
            } else {
                ungrouped.push(room);
            }
        }

        // Header
        let header_text: String = selected_space_name
            .as_deref()
            .unwrap_or("fluxchat")
            .to_string();
        let mut header_row = row![
            text(header_text)
                .font(self.sidebar_font())
                .size(14)
                .color(theme::colors().text_bright),
            Space::new().width(Fill),
        ]
        .align_y(iced::Alignment::Center);

        // Invite button for spaces
        if let Some(space_id) = &self.selected_space_id {
            let sid = space_id.clone();
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
                        Some(Color::from_rgba(1.0, 1.0, 1.0, 0.08).into())
                    } else {
                        None
                    },
                    border: iced::Border { radius: 4.0.into(), ..Default::default() },
                    ..Default::default()
                }),
            );
        }

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
                        Some(Color::from_rgba(1.0, 1.0, 1.0, 0.08).into())
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
                        Some(Color::from_rgba(1.0, 1.0, 1.0, 0.08).into())
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
                            text(count.to_string()).font(self.sidebar_font()).size(10).color(theme::colors().text_bright),
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
                        Some(Color::from_rgba(1.0, 0.2, 0.2, 0.08).into())
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
                    for el in self.view_thread_entries(&room.room_id) { col = col.push(el); }
                }
            }
        }

        // Ungrouped channels
        if !ungrouped.is_empty() {
            col = col.push(self.view_section_header_with_icon("Channels", ungrouped.len(), Some(SVG_HASH), None));
            if !self.collapsed_sections.contains("Channels") {
                for room in &ungrouped {
                    col = col.push(self.view_room_entry(room, false));
                    for el in self.view_thread_entries(&room.room_id) { col = col.push(el); }
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
                    header.into()
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
                            for el in self.view_thread_entries(&room.room_id) { col = col.push(el); }
                        }
                    }
                }
            }
        }

        // Render any rooms whose space_id wasn't in the tree (fallback)
        for (sid, rooms) in &by_space_id {
            if self.space_tree.iter().any(|n| n.info.room_id == *sid) {
                continue; // already rendered above
            }
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
                    for el in self.view_thread_entries(&room.room_id) { col = col.push(el); }
                }
            }
        }

        // DMs
        if !dms.is_empty() {
            col = col.push(self.view_section_header_with_icon("Direct Messages", dms.len(), Some(SVG_CHAT), None));
            if !self.collapsed_sections.contains("Direct Messages") {
                for room in &dms {
                    col = col.push(self.view_room_entry(room, true));
                    for el in self.view_thread_entries(&room.room_id) { col = col.push(el); }
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
            text(count.to_string()).font(self.sidebar_font()).size(10).color(theme::colors().text_dim)
        );

        column![
            container(rule::horizontal(1).style(|_: &Theme| rule::Style {
                color: Color::from_rgba(1.0, 1.0, 1.0, 0.06),
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
                        Some(Color::from_rgba(1.0, 1.0, 1.0, 0.03).into())
                    } else {
                        None
                    },
                    text_color: theme::colors().text_dim,
                    ..Default::default()
                }),
        ].spacing(0).into()
    }

    /// Thread entries indented below their parent room in the sidebar.
    fn view_thread_entries(&self, room_id: &str) -> Vec<Element<Message>> {
        let prefix = format!("thread::{room_id}::");
        self.open_tabs.iter()
            .filter(|(tid, _, _, _)| tid.starts_with(&prefix))
            .map(|(tid, tname, _, _)| {
                let is_selected = self.selected_room_id.as_deref() == Some(tid.as_str());
                let name_color = if is_selected { theme::colors().text_bright } else { theme::colors().text_dim };
                let bg = if is_selected { theme::colors().bg_light } else { Color::TRANSPARENT };
                let tid_owned = tid.clone();
                button(
                    row![
                        Space::new().width(Length::Fixed(20.0)),
                        iced::widget::svg(iced::widget::svg::Handle::from_memory(SVG_THREAD))
                            .width(12).height(12)
                            .style(move |_, _| iced::widget::svg::Style { color: Some(name_color) }),
                        text(tname).font(self.sidebar_font()).size(12).color(name_color)
                            .wrapping(iced::widget::text::Wrapping::None),
                    ].spacing(4).align_y(iced::Alignment::Center),
                )
                .on_press(Message::SelectRoom(tid_owned))
                .padding([3, 8])
                .width(Fill)
                .style(move |_, status| button::Style {
                    background: if matches!(status, button::Status::Hovered) {
                        Some(Color::from_rgba(1.0, 1.0, 1.0, 0.06).into())
                    } else if bg != Color::TRANSPARENT {
                        Some(bg.into())
                    } else {
                        None
                    },
                    ..Default::default()
                })
                .into()
            })
            .collect()
    }

    /// Single room entry in the list.
    fn view_room_entry<'a>(&self, room: &'a fluxchat_core::RoomInfo, is_dm: bool) -> Element<'a, Message> {
        let is_selected = self.selected_room_id.as_deref() == Some(&room.room_id);
        let has_unread = room.unread_count > 0 && !self.read_rooms.contains(&room.room_id);
        let name_color = if is_selected || has_unread {
            theme::colors().text_bright
        } else {
            theme::colors().text_muted
        };
        let bg = if is_selected {
            theme::colors().bg_light
        } else {
            Color::TRANSPARENT
        };

        // Room icon: avatar > DM person > encrypted lock > hash
        let room_icon: Element<Message> = if let Some(handle) = room.avatar_url.as_ref().and_then(|mxc| self.avatar_cache.get(mxc)) {
            container(
                iced::widget::image(handle.clone())
                    .width(18)
                    .height(18)
                    .content_fit(iced::ContentFit::Cover),
            )
            .width(18).height(18).clip(true)
            .style(|_: &Theme| container::Style {
                border: iced::Border { radius: 4.0.into(), ..Default::default() },
                ..Default::default()
            })
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
                    .size(10)
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
                    .size(10)
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
        row_content = row_content.push(room_icon);
        row_content = row_content.push(
            text(&room.name)
                .font(self.sidebar_font())
                .size(13)
                .color(name_color)
                .wrapping(iced::widget::text::Wrapping::None),
        );

        // Call indicator
        if self.rooms_with_calls.contains(&room.room_id) {
            row_content = row_content.push(Space::new().width(Fill));
            row_content = row_content.push(
                iced::widget::svg(iced::widget::svg::Handle::from_memory(SVG_PHONE))
                    .width(12).height(12)
                    .style(|_, _| iced::widget::svg::Style { color: Some(theme::colors().green) }),
            );
        }

        let room_id = room.room_id.clone();
        let ctx_room_id = room.room_id.clone();
        let ctx_menu = crate::state::ContextMenu::Room { room_id: ctx_room_id, is_dm };
        mouse_area(
            container(
                button(
                    container(row_content)
                        .width(Fill)
                        .clip(true)
                )
                .on_press(Message::SelectRoom(room_id))
                .padding([5, 8])
                .width(Fill)
                .style(move |_, _| button::Style {
                    background: Some(bg.into()),
                    text_color: name_color,
                    border: iced::Border { radius: 4.0.into(), ..Default::default() },
                    ..Default::default()
                })
            )
            .padding(iced::Padding { top: 0.0, right: 4.0, bottom: 0.0, left: 12.0 })
        )
        .on_right_press(Message::OpenContextMenu(ctx_menu))
        .into()
    }

    /// Main content area (tabs + messages + input).
    fn view_content(&self) -> Element<'_, Message> {
        let mut content = column![].width(Fill).height(Fill);

        // Room header bar (room name + action buttons)
        if let Some(ref room_id) = self.selected_room_id {
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
                }).unwrap_or_else(|| room_id.clone());
                (room, name)
            };

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
                        Some(Color::from_rgba(1.0, 1.0, 1.0, 0.06).into())
                    } else if matches!(status, button::Status::Hovered) {
                        Some(Color::from_rgba(1.0, 1.0, 1.0, 0.06).into())
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
            let call_btn: Element<Message> = if self.calls_supported || in_call {
                let call_msg = if in_call {
                    Message::CallLeave
                } else {
                    Message::CallJoin(selected_rid)
                };
                let handle = iced::widget::svg::Handle::from_memory(SVG_PHONE);
                let icon_color = if in_call { theme::colors().green } else { Color::from_rgba(1.0, 1.0, 1.0, 0.4) };
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
            } else {
                // Dimmed/disabled phone icon when calls not supported
                let handle = iced::widget::svg::Handle::from_memory(SVG_PHONE);
                container(
                    iced::widget::svg(handle)
                        .width(14).height(14)
                        .style(|_, _| iced::widget::svg::Style { color: Some(Color::from_rgba(1.0, 1.0, 1.0, 0.15)) })
                )
                .padding([4, 6])
                .into()
            };

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
                    background: if pin_active {
                        Some(Color::from_rgba(1.0, 1.0, 1.0, 0.06).into())
                    } else if matches!(status, button::Status::Hovered) {
                        Some(Color::from_rgba(1.0, 1.0, 1.0, 0.06).into())
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
                .gap(4)
                .style(|_: &Theme| container::Style {
                    background: Some(theme::colors().bg_dark.into()),
                    border: iced::Border { color: theme::colors().border, width: 1.0, radius: 4.0.into() },
                    ..Default::default()
                })
                .padding(4)
                .into()
            };

            let actions = row![
                call_btn,
                make_action_btn(SVG_SEARCH, Message::ToggleSearch, self.search_open, "Search"),
                pin_btn,
                make_action_btn(SVG_THREAD, Message::ToggleThreads, self.threads_open, "Threads"),
                make_action_btn(SVG_USERS, Message::ToggleMemberList, self.show_member_list, "Members"),
                make_action_btn(SVG_INFO, Message::OpenModal(crate::state::Modal::RoomSettings), false, "Settings"),
                {
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
                            Some(Color::from_rgba(1.0, 1.0, 1.0, 0.06).into())
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
                    .gap(4)
                    .style(|_: &Theme| container::Style {
                        background: Some(theme::colors().bg_dark.into()),
                        border: iced::Border { color: theme::colors().border, width: 1.0, radius: 4.0.into() },
                        ..Default::default()
                    })
                    .padding(4)
                    .into();
                    tt
                },
            ]
            .spacing(2)
            .align_y(iced::Alignment::Center);

            // Room avatar for header
            let header_avatar: Element<Message> = if let Some(handle) = current_room
                .and_then(|r| r.avatar_url.as_ref())
                .and_then(|mxc| self.avatar_cache.get(mxc))
            {
                container(
                    iced::widget::image(handle.clone())
                        .width(22).height(22)
                        .content_fit(iced::ContentFit::Cover),
                )
                .width(22).height(22).clip(true)
                .style(|_: &Theme| container::Style {
                    border: iced::Border { radius: 4.0.into(), ..Default::default() },
                    ..Default::default()
                })
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

            let header_row = row![
                header_avatar,
                text(room_name).font(self.sidebar_font()).size(14).color(theme::colors().text_bright)
                    .wrapping(iced::widget::text::Wrapping::None),
                Space::new().width(Fill),
                actions,
            ]
            .spacing(8)
            .align_y(iced::Alignment::Center)
            .padding([6, 12]);

            content = content.push(
                container(header_row)
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

            // Thread topic bar: show thread root message preview
            if room_id.starts_with("thread::") {
                if let Some((_, tname, _, _)) = self.open_tabs.iter().find(|(id, _, _, _)| id == room_id) {
                    content = content.push(
                        container(
                            text(tname.as_str())
                                .font(self.sidebar_font())
                                .size(13)
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

            // Collect all optional panels into a single column so the content
            // column always has the same number of children. This prevents Iced
            // from losing the messages scrollable's scroll state when panels
            // are toggled on/off.
            let mut panels_col: iced::widget::Column<'_, Message> = column![].spacing(0);

            // Search is now rendered as a modal overlay (like quick switcher)

            // Panels (pinned messages, threads, files, notices)
            if self.pins_open {
                let mut panel_col = column![
                    row![
                        text("PINNED MESSAGES").font(self.sidebar_font()).size(12).color(theme::colors().text_dim),
                        Space::new().width(Fill),
                        button(text("\u{00d7}").font(self.sidebar_font()).size(16).color(theme::colors().text_dim))
                            .on_press(Message::TogglePins)
                            .padding([0, 6])
                            .style(theme::button_transparent),
                    ].align_y(iced::Alignment::Center),
                ].spacing(4);
                if self.pinned_messages.is_empty() {
                    panel_col = panel_col.push(text("No pinned messages").font(self.sidebar_font()).size(13).color(theme::colors().text_dim));
                } else {
                    for msg in self.pinned_messages.iter().take(20) {
                        panel_col = panel_col.push(
                            text(format!("{}: {}", msg.sender_name, msg.body))
                                .font(self.sidebar_font()).size(13).color(theme::colors().text_muted)
                        );
                    }
                }
                panels_col = panels_col.push(
                    container(panel_col).padding([8, 12]).width(Fill)
                        .style(theme::container_panel)
                );
            }

            if self.threads_open {
                let mut panel_col = column![
                    row![
                        text("THREADS").font(self.sidebar_font()).size(12).color(theme::colors().text_dim),
                        Space::new().width(Fill),
                        button(text("\u{00d7}").font(self.sidebar_font()).size(16).color(theme::colors().text_dim))
                            .on_press(Message::ToggleThreads)
                            .padding([0, 6])
                            .style(theme::button_transparent),
                    ].align_y(iced::Alignment::Center),
                ].spacing(4);
                if self.thread_messages.is_empty() {
                    panel_col = panel_col.push(text("No threads").font(self.sidebar_font()).size(13).color(theme::colors().text_dim));
                } else {
                    for tmsg in self.thread_messages.iter().take(50) {
                        let nick_col = theme::nick_color(&tmsg.sender_name);
                        let count = tmsg.thread_count.unwrap_or(0);
                        let count_label = if count == 1 { "1 reply".to_string() } else { format!("{count} replies") };
                        let body_preview: String = tmsg.body.lines().next().unwrap_or("").chars().take(80).collect();
                        let thread_room_id = self.selected_room_id.clone().unwrap_or_default();
                        let thread_event_id = tmsg.event_id.clone();
                        panel_col = panel_col.push(
                            button(
                                column![
                                    row![
                                        text(&tmsg.sender_name).font(self.sidebar_font()).size(13).color(nick_col),
                                        text(count_label).font(self.sidebar_font()).size(12).color(theme::colors().text_dim),
                                    ].spacing(8).align_y(iced::Alignment::Center),
                                    text(body_preview).font(self.sidebar_font()).size(13).color(theme::colors().text_muted)
                                        .wrapping(iced::widget::text::Wrapping::None),
                                ].spacing(2),
                            )
                            .on_press(Message::OpenThread(thread_room_id, thread_event_id))
                            .padding([4, 6])
                            .width(Fill)
                            .style(|_, status| button::Style {
                                background: if matches!(status, button::Status::Hovered) {
                                    Some(Color::from_rgba(1.0, 1.0, 1.0, 0.06).into())
                                } else {
                                    None
                                },
                                ..Default::default()
                            }),
                        );
                    }
                }
                panels_col = panels_col.push(
                    container(panel_col).padding([8, 12]).width(Fill)
                        .style(theme::container_panel)
                );
            }

            if self.files_open {
                let mut panel_col = column![
                    row![
                        text("FILES").font(self.sidebar_font()).size(12).color(theme::colors().text_dim),
                        Space::new().width(Fill),
                        button(text("\u{00d7}").font(self.sidebar_font()).size(16).color(theme::colors().text_dim))
                            .on_press(Message::ToggleFiles)
                            .padding([0, 6])
                            .style(theme::button_transparent),
                    ].align_y(iced::Alignment::Center),
                ].spacing(4);
                // Show images from messages
                if let Some(msgs) = self.messages_by_room.get(room_id) {
                    let files: Vec<_> = msgs.iter().filter(|m| m.image_mxc.is_some()).take(20).collect();
                    if files.is_empty() {
                        panel_col = panel_col.push(text("No files").font(self.sidebar_font()).size(13).color(theme::colors().text_dim));
                    } else {
                        for msg in files {
                            panel_col = panel_col.push(
                                text(format!("\u{1f4ce} {}", msg.body)).font(self.sidebar_font()).size(13).color(theme::colors().accent)
                            );
                        }
                    }
                }
                panels_col = panels_col.push(
                    container(panel_col).padding([8, 12]).width(Fill)
                        .style(theme::container_panel)
                );
            }

            if self.notices_open {
                let panel_col = column![
                    row![
                        text("NOTICES").font(self.sidebar_font()).size(12).color(theme::colors().text_dim),
                        Space::new().width(Fill),
                        button(text("\u{00d7}").font(self.sidebar_font()).size(16).color(theme::colors().text_dim))
                            .on_press(Message::ToggleNotices)
                            .padding([0, 6])
                            .style(theme::button_transparent),
                    ].align_y(iced::Alignment::Center),
                    text("No server notices").font(self.sidebar_font()).size(13).color(theme::colors().text_dim),
                ].spacing(4);
                panels_col = panels_col.push(
                    container(panel_col).padding([8, 12]).width(Fill)
                        .style(theme::container_panel)
                );
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
                let last_read = self.last_read_event.get(room_id).cloned();
                // Don't show divider if the last read event is the very last message
                let show_divider = last_read.as_ref().is_some_and(|lr| {
                    msgs.last().map(|m| &m.event_id) != Some(lr)
                });
                let is_thread_view = room_id.starts_with("thread::");
                // Thread counts precomputed in update.rs
                let empty_tc = std::collections::HashMap::new();
                let thread_counts = if !is_thread_view {
                    self.thread_counts.get(room_id).unwrap_or(&empty_tc)
                } else {
                    &empty_tc
                };
                for msg in msgs.iter() {
                    if self.ignored_users.contains(&msg.sender) {
                        continue;
                    }
                    // Hide thread replies from the main room (they show in thread view)
                    if !is_thread_view && msg.thread_root.is_some() {
                        continue;
                    }
                    // Unread divider: show after the last-read message
                    if show_divider && !divider_shown {
                        if let Some(ref lr) = last_read {
                            if msg.event_id == *lr {
                                divider_shown = true;
                                msg_col = msg_col.push(self.view_message(msg));
                                // Read receipts for this message
                                if let Some(receipt_row) = self.view_read_receipts(msg) {
                                    msg_col = msg_col.push(receipt_row);
                                }
                                msg_col = msg_col.push(
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
                                continue;
                            }
                        }
                    }
                    msg_col = msg_col.push(self.view_message(msg));
                    // Thread indicator from derived counts (not relying on msg.thread_count)
                    if !is_thread_view {
                        if let Some(&count) = thread_counts.get(msg.event_id.as_str()) {
                            let label = if count == 1 { "1 reply".to_string() } else { format!("{count} replies") };
                            let handle = iced::widget::svg::Handle::from_memory(crate::icons::SVG_THREAD);
                            let current_room = room_id.to_string();
                            let eid = msg.event_id.clone();
                            msg_col = msg_col.push(
                                row![
                                    Space::new().width(Length::Fixed(196.0)),
                                    button(
                                        row![
                                            iced::widget::svg(handle)
                                                .width(12).height(12)
                                                .style(|_, _| iced::widget::svg::Style { color: Some(theme::colors().accent) }),
                                            text(label).font(self.sidebar_font()).size(12).color(theme::colors().accent),
                                        ].spacing(4).align_y(iced::Alignment::Center),
                                    )
                                    .on_press(Message::OpenThread(current_room, eid))
                                    .padding([2, 6])
                                    .style(|_, status| button::Style {
                                        background: if matches!(status, button::Status::Hovered) {
                                            Some(theme::with_alpha(theme::colors().accent, 0.1).into())
                                        } else {
                                            None
                                        },
                                        border: iced::Border { radius: 4.0.into(), ..Default::default() },
                                        ..Default::default()
                                    }),
                                ].spacing(0),
                            );
                        }
                    }
                    // Read receipts for this message
                    if let Some(receipt_row) = self.view_read_receipts(msg) {
                        msg_col = msg_col.push(receipt_row);
                    }
                }
            } else {
                msg_col = msg_col.push(
                    text("Loading messages...")
                        .font(self.sidebar_font())
                        .size(15)
                        .color(theme::colors().text_dim),
                );
            }

            let msg_scrollable = mouse_area(
                scrollable(container(msg_col).width(Fill))
                    .id(messages_scrollable_id().clone())
                    .width(Fill)
                    .height(Fill)
                    .on_scroll(Message::MessagesScrolled)
                    .direction(iced::widget::scrollable::Direction::Vertical(iced::widget::scrollable::Scrollbar::new().width(4).scroller_width(4))).style(thin_scrollbar),
            )
            .on_exit(Message::HoverMessage(None));

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

            // Input area: typing indicator, reply/edit preview, GIF panel,
            // upload preview, help text, autocomplete, input bar
            for element in self.view_input_area(room_id) {
                content = content.push(element);
            }
        } else {
            content = content.push(
                center(
                    text("select a room")
                        .font(self.sidebar_font())
                        .size(16)
                        .color(theme::colors().text_dim),
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

        // Settings gear SVG
        let gear_handle = iced::widget::svg::Handle::from_memory(
            br#"<svg xmlns="http://www.w3.org/2000/svg" width="24" height="24" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><circle cx="12" cy="12" r="3"/><path d="M19.4 15a1.65 1.65 0 0 0 .33 1.82l.06.06a2 2 0 0 1-2.83 2.83l-.06-.06a1.65 1.65 0 0 0-1.82-.33 1.65 1.65 0 0 0-1 1.51V21a2 2 0 0 1-4 0v-.09A1.65 1.65 0 0 0 9 19.4a1.65 1.65 0 0 0-1.82.33l-.06.06a2 2 0 0 1-2.83-2.83l.06-.06A1.65 1.65 0 0 0 4.68 15a1.65 1.65 0 0 0-1.51-1H3a2 2 0 0 1 0-4h.09A1.65 1.65 0 0 0 4.6 9a1.65 1.65 0 0 0-.33-1.82l-.06-.06a2 2 0 0 1 2.83-2.83l.06.06A1.65 1.65 0 0 0 9 4.68a1.65 1.65 0 0 0 1-1.51V3a2 2 0 0 1 4 0v.09a1.65 1.65 0 0 0 1 1.51 1.65 1.65 0 0 0 1.82-.33l.06-.06a2 2 0 0 1 2.83 2.83l-.06.06A1.65 1.65 0 0 0 19.4 9a1.65 1.65 0 0 0 1.51 1H21a2 2 0 0 1 0 4h-.09a1.65 1.65 0 0 0-1.51 1z"/></svg>"#.as_slice(),
        );
        bar = bar.push(
            button(
                iced::widget::svg(gear_handle)
                    .width(14).height(14)
                    .style(|_, _| iced::widget::svg::Style { color: Some(theme::colors().text_dim) }),
            )
            .on_press(Message::OpenModal(crate::state::Modal::Settings {
                tab: "account".into(),
            }))
            .padding([2, 6])
            .style(theme::button_transparent),
        );

        // User ID in green (like IRC)
        bar = bar.push(
            text(&self.user_id).font(self.sidebar_font()).size(13).color(theme::colors().green)
        );

        // Current room info
        if let Some(room_id) = &self.selected_room_id {
            let room = self.rooms.iter().find(|r| r.room_id == *room_id);
            let room_name = room.map(|r| r.name.as_str()).unwrap_or(room_id);

            bar = bar.push(sep());
            bar = bar.push(text(format!("#{room_name}")).font(self.sidebar_font()).size(13).color(theme::colors().text_normal));

            // Power level badge
            if let Some(members) = self.members_by_room.get(room_id) {
                if let Some(me) = members.iter().find(|m| m.user_id == self.user_id) {
                    if me.power_level >= 100 {
                        bar = bar.push(text(" @").font(self.sidebar_font()).size(12).color(theme::colors().yellow));
                    } else if me.power_level >= 50 {
                        bar = bar.push(text(" +").font(self.sidebar_font()).size(12).color(theme::colors().accent));
                    }
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
        }

        bar = bar.push(Space::new().width(Fill));

        // Sync lag
        let lag_label = if self.sync_lag_ms == 0 { "synced".to_string() } else { format!("{}ms", self.sync_lag_ms) };
        let lag_color = if self.sync_lag_ms < 200 {
            theme::colors().green
        } else if self.sync_lag_ms < 1000 {
            theme::colors().yellow
        } else {
            theme::colors().red
        };
        bar = bar.push(text(lag_label).font(self.sidebar_font()).size(13).color(lag_color));

        // Verification status
        if let Some(ref sec) = self.security_state {
            bar = bar.push(sep());
            if sec.is_verified {
                bar = bar.push(
                    text("\u{2713} verified").font(self.sidebar_font()).size(13).color(theme::colors().green)
                );
            } else {
                bar = bar.push(
                    text("\u{2717} unverified").font(self.sidebar_font()).size(13).color(theme::colors().yellow)
                );
            }
        }

        // Device ID
        if let Some(ref did) = self.device_id {
            bar = bar.push(sep());
            let short = if did.len() > 10 { &did[..10] } else { did };
            bar = bar.push(text(short).font(self.sidebar_font()).size(13).color(theme::colors().text_dim));
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

                // Arrow keys for search results navigation
                if key == iced::keyboard::Key::Named(iced::keyboard::key::Named::ArrowDown) && !modifiers.alt() {
                    return Message::SearchArrowDown;
                }
                if key == iced::keyboard::Key::Named(iced::keyboard::key::Named::ArrowUp) && !modifiers.alt() {
                    return Message::SearchArrowUp;
                }

                // Escape: close modals/panels/completions
                if key == iced::keyboard::Key::Named(iced::keyboard::key::Named::Escape) {
                    return Message::Escape;
                }
            }
            Message::Noop
        });

        // File drag-drop + sidebar resize mouse tracking + window focus + global shortcuts
        let file_drop = iced::event::listen_with(|event, _status, _window| {
            // Intercept global shortcuts before widgets consume them
            if let iced::Event::Keyboard(iced::keyboard::Event::KeyPressed { key, modifiers, .. }) = &event {
                // Ctrl/Cmd+K: quick switcher
                if modifiers.command() || modifiers.control() {
                    if let iced::keyboard::Key::Character(c) = key {
                        if c.as_str() == "k" || c.as_str() == "K" {
                            return Some(Message::ToggleQuickSwitcher);
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
                    }
                }
                // Escape while quick switcher is open — intercept before text_input consumes it
                if *key == iced::keyboard::Key::Named(iced::keyboard::key::Named::Escape) {
                    // Always emit Escape — the handler will check quick_switcher_open
                    return Some(Message::Escape);
                }
            }
            match event {
                iced::Event::Window(iced::window::Event::FileDropped(path)) => {
                    Some(Message::FileDropped(path))
                }
                iced::Event::Window(iced::window::Event::Focused) => {
                    Some(Message::WindowFocusChanged(true))
                }
                iced::Event::Window(iced::window::Event::Unfocused) => {
                    Some(Message::WindowFocusChanged(false))
                }
                iced::Event::Mouse(iced::mouse::Event::CursorMoved { position }) => {
                    Some(Message::CursorMoved(position.x, position.y))
                }
                iced::Event::Mouse(iced::mouse::Event::ButtonReleased(iced::mouse::Button::Left)) => {
                    Some(Message::SidebarDragEnd)
                }
                _ => None,
            }
        });

        // Timeline updates from background diff watchers
        let timeline_sub = Subscription::run(timeline_update_stream);

        // Call events from LiveKit session
        let call_sub = Subscription::run(call_event_stream);

        Subscription::batch([tick, keys, file_drop, timeline_sub, call_sub, audio_tick])
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

fn call_event_stream() -> impl futures_util::Stream<Item = Message> {
    async_stream::stream! {
        let mut rx = {
            let lock = crate::state::CALL_RX.get();
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
