use iced::widget::{
    button, center, column, container, mouse_area, opaque, rich_text, row, rule, scrollable, stack,
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

const EMOJI_GRID_COLUMNS: usize = 8;
const EMOJI_PANEL_HEIGHT: f32 = 280.0;
const EMOJI_CELL_SIZE: f32 = 36.0;
const EMOJI_FREQ_COUNT: usize = 24;

/// Fixed ID for the message scrollable so we can snap to bottom.
pub fn messages_scrollable_id() -> iced::widget::Id {
    iced::widget::Id::new("messages_scroll")
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

/// Thin subtle scrollbar style matching the dark theme.
fn thin_scrollbar(_theme: &Theme, status: iced::widget::scrollable::Status) -> iced::widget::scrollable::Style {
    use iced::widget::scrollable::{Style, Rail, Scroller, AutoScroll};
    let is_hovered = matches!(status, iced::widget::scrollable::Status::Hovered { .. } | iced::widget::scrollable::Status::Dragged { .. });
    let scroller_alpha = if is_hovered { 0.4 } else { 0.15 };
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

// Sidebar icons
const SVG_HASH: &[u8] = br#"<svg xmlns="http://www.w3.org/2000/svg" width="24" height="24" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2.5" stroke-linecap="round"><line x1="4" y1="9" x2="20" y2="9"/><line x1="4" y1="15" x2="20" y2="15"/><line x1="10" y1="3" x2="8" y2="21"/><line x1="16" y1="3" x2="14" y2="21"/></svg>"#;
const SVG_LOCK: &[u8] = br#"<svg xmlns="http://www.w3.org/2000/svg" width="24" height="24" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><rect x="3" y="11" width="18" height="11" rx="2" ry="2"/><path d="M7 11V7a5 5 0 0 1 10 0v4"/></svg>"#;
const SVG_PERSON: &[u8] = br#"<svg xmlns="http://www.w3.org/2000/svg" width="24" height="24" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><path d="M20 21v-2a4 4 0 0 0-4-4H8a4 4 0 0 0-4 4v2"/><circle cx="12" cy="7" r="4"/></svg>"#;
const SVG_CHAT: &[u8] = br#"<svg xmlns="http://www.w3.org/2000/svg" width="24" height="24" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2.5" stroke-linecap="round" stroke-linejoin="round"><path d="M21 15a2 2 0 0 1-2 2H7l-4 4V5a2 2 0 0 1 2-2h14a2 2 0 0 1 2 2z"/></svg>"#;

// Room action bar icons
const SVG_SEARCH: &[u8] = br#"<svg xmlns="http://www.w3.org/2000/svg" width="24" height="24" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><circle cx="11" cy="11" r="8"/><line x1="21" y1="21" x2="16.65" y2="16.65"/></svg>"#;
const SVG_USERS: &[u8] = br#"<svg xmlns="http://www.w3.org/2000/svg" width="24" height="24" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><path d="M17 21v-2a4 4 0 0 0-4-4H5a4 4 0 0 0-4 4v2"/><circle cx="9" cy="7" r="4"/><path d="M23 21v-2a4 4 0 0 0-3-3.87"/><path d="M16 3.13a4 4 0 0 1 0 7.75"/></svg>"#;
const SVG_INFO: &[u8] = br#"<svg xmlns="http://www.w3.org/2000/svg" width="24" height="24" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><circle cx="12" cy="12" r="10"/><line x1="12" y1="16" x2="12" y2="12"/><line x1="12" y1="8" x2="12.01" y2="8"/></svg>"#;
const SVG_MORE: &[u8] = br#"<svg xmlns="http://www.w3.org/2000/svg" width="24" height="24" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><circle cx="12" cy="12" r="1"/><circle cx="12" cy="5" r="1"/><circle cx="12" cy="19" r="1"/></svg>"#;
const SVG_STAR: &[u8] = br#"<svg xmlns="http://www.w3.org/2000/svg" width="24" height="24" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><polygon points="12 2 15.09 8.26 22 9.27 17 14.14 18.18 21.02 12 17.77 5.82 21.02 7 14.14 2 9.27 8.91 8.26 12 2"/></svg>"#;
const SVG_COPY: &[u8] = br#"<svg xmlns="http://www.w3.org/2000/svg" width="24" height="24" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><path d="M10 13a5 5 0 0 0 7.54.54l3-3a5 5 0 0 0-7.07-7.07l-1.72 1.71"/><path d="M14 11a5 5 0 0 0-7.54-.54l-3 3a5 5 0 0 0 7.07 7.07l1.71-1.71"/></svg>"#;
const SVG_CLIPBOARD: &[u8] = br#"<svg xmlns="http://www.w3.org/2000/svg" width="24" height="24" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><rect width="8" height="4" x="8" y="2" rx="1" ry="1"/><path d="M16 4h2a2 2 0 0 1 2 2v14a2 2 0 0 1-2 2H6a2 2 0 0 1-2-2V6a2 2 0 0 1 2-2h2"/></svg>"#;
const SVG_PIN: &[u8] = br#"<svg xmlns="http://www.w3.org/2000/svg" width="24" height="24" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><line x1="12" y1="17" x2="12" y2="22"/><path d="M5 17h14v-1.76a2 2 0 0 0-1.11-1.79l-1.78-.9A2 2 0 0 1 15 10.76V6h1a2 2 0 0 0 0-4H8a2 2 0 0 0 0 4h1v4.76a2 2 0 0 1-1.11 1.79l-1.78.9A2 2 0 0 0 5 15.24Z"/></svg>"#;
const SVG_THREAD: &[u8] = br#"<svg xmlns="http://www.w3.org/2000/svg" width="24" height="24" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><path d="M21 15a2 2 0 0 1-2 2H7l-4 4V5a2 2 0 0 1 2-2h14a2 2 0 0 1 2 2z"/><line x1="9" y1="10" x2="15" y2="10"/></svg>"#;
const SVG_FILE: &[u8] = br#"<svg xmlns="http://www.w3.org/2000/svg" width="24" height="24" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><path d="M14 2H6a2 2 0 0 0-2 2v16a2 2 0 0 0 2 2h12a2 2 0 0 0 2-2V8z"/><polyline points="14 2 14 8 20 8"/></svg>"#;
const SVG_GIF: &[u8] = br#"<svg xmlns="http://www.w3.org/2000/svg" width="24" height="24" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><rect x="2" y="4" width="20" height="16" rx="2"/></svg>"#;
const SVG_BELL: &[u8] = br#"<svg xmlns="http://www.w3.org/2000/svg" width="24" height="24" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><path d="M18 8A6 6 0 0 0 6 8c0 7-3 9-3 9h18s-3-2-3-9"/><path d="M13.73 21a2 2 0 0 1-3.46 0"/></svg>"#;

// Message action bar icons
const SVG_REACT: &[u8] = br#"<svg xmlns="http://www.w3.org/2000/svg" width="24" height="24" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><circle cx="12" cy="12" r="10"/><path d="M8 14s1.5 2 4 2 4-2 4-2"/><line x1="9" y1="9" x2="9.01" y2="9"/><line x1="15" y1="9" x2="15.01" y2="9"/></svg>"#;
const SVG_REPLY: &[u8] = br#"<svg xmlns="http://www.w3.org/2000/svg" width="24" height="24" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><polyline points="9 17 4 12 9 7"/><path d="M20 18v-2a4 4 0 0 0-4-4H4"/></svg>"#;
const SVG_EDIT: &[u8] = br#"<svg xmlns="http://www.w3.org/2000/svg" width="24" height="24" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><path d="M17 3a2.85 2.83 0 1 1 4 4L7.5 20.5 2 22l1.5-5.5Z"/></svg>"#;
const SVG_DELETE: &[u8] = br#"<svg xmlns="http://www.w3.org/2000/svg" width="24" height="24" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><path d="M3 6h18"/><path d="M19 6v14c0 1-1 2-2 2H7c-1 0-2-1-2-2V6"/><path d="M8 6V4c0-1 1-2 2-2h4c1 0 2 1 2 2v2"/></svg>"#;

const MONO: Font = Font {
    family: iced::font::Family::Name("JetBrainsMono Nerd Font"),
    ..Font::MONOSPACE
};
const SANS: Font = Font::DEFAULT;
const SANS_BOLD: Font = Font { weight: iced::font::Weight::Bold, ..Font::DEFAULT };

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

        // Drag handle between room list and content
        let drag_handle: Element<Message> = mouse_area(
            container(Space::new().width(3).height(Fill))
                .style(|_: &Theme| container::Style {
                    background: Some(theme::BORDER.into()),
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
                background: Some(theme::BG_DEEP.into()),
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
        stack(layers).into()
    }

    /// Left space sidebar (icons).
    fn view_sidebar(&self) -> Element<Message> {
        let is_home = self.selected_space_id.is_none();
        // Home button with SVG house icon
        let home_radius = if is_home { 12.0 } else { 20.0 };
        let home_bg_color = if is_home { theme::ACCENT } else { theme::BG_MID };
        let home_handle = iced::widget::svg::Handle::from_memory(
            br#"<svg xmlns="http://www.w3.org/2000/svg" width="24" height="24" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><path d="m3 9 9-7 9 7v11a2 2 0 0 1-2 2H5a2 2 0 0 1-2-2z"/><polyline points="9 22 9 12 15 12 15 22"/></svg>"#.as_slice(),
        );
        let home_btn = button(
            iced::widget::svg(home_handle)
                .width(22).height(22)
                .style(|_, _| iced::widget::svg::Style { color: Some(theme::TEXT_BRIGHT) }),
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
                    background: Some(theme::BORDER.into()),
                    border: iced::Border { radius: 1.0.into(), ..Default::default() },
                    ..Default::default()
                })
        );

        for space in &self.spaces {
            let space_id = space.room_id.clone();
            let is_selected = self.selected_space_id.as_deref() == Some(&space.room_id);
            let radius = if is_selected { 12.0 } else { 20.0 };
            let border_color = if is_selected { theme::ACCENT } else { Color::TRANSPARENT };
            let opacity = if is_selected { 1.0 } else { 0.7 };

            let icon_content: Element<Message> = if let Some(handle) = space.avatar_url.as_ref().and_then(|mxc| self.avatar_cache.get(mxc)) {
                container(
                    iced::widget::image(handle.clone())
                        .width(36).height(36)
                        .content_fit(iced::ContentFit::Cover),
                )
                .width(40).height(40)
                .center(Fill)
                .clip(true)
                .style(move |_: &Theme| container::Style {
                    border: iced::Border {
                        color: border_color,
                        width: 2.0,
                        radius: radius.into(),
                    },
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
                    text(initials).font(MONO).size(13).color(Color { a: opacity, ..theme::TEXT_MUTED }),
                )
                .width(40).height(40)
                .center(Fill)
                .style(move |_: &Theme| container::Style {
                    background: Some(theme::BG_MID.into()),
                    border: iced::Border {
                        color: border_color,
                        width: 2.0,
                        radius: radius.into(),
                    },
                    ..Default::default()
                })
                .into()
            };

            let btn = iced::widget::tooltip(
                button(icon_content)
                    .on_press(Message::SelectSpace(Some(space_id)))
                    .padding(0)
                    .style(|_, _| button::Style { background: None, ..Default::default() }),
                text(&space.name).font(MONO).size(12),
                iced::widget::tooltip::Position::Right,
            )
            .style(|_: &Theme| container::Style {
                background: Some(theme::BG_MID.into()),
                border: iced::Border { color: theme::BORDER, width: 1.0, radius: 4.0.into() },
                ..Default::default()
            })
            .padding(6);

            col = col.push(btn);
        }

        container(scrollable(col).direction(iced::widget::scrollable::Direction::Vertical(iced::widget::scrollable::Scrollbar::new().width(4).scroller_width(4))).style(thin_scrollbar))
            .width(56)
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
                    let rid_accept = room.room_id.clone();
                    let rid_reject = room.room_id.clone();
                    col = col.push(
                        container(
                            row![
                                text(&room.name).font(MONO).size(13).color(theme::TEXT_NORMAL),
                                Space::new().width(Fill),
                                button(text("\u{2713}").font(MONO).size(14).color(theme::GREEN))
                                    .on_press(Message::AcceptInvite(rid_accept))
                                    .padding([2, 6])
                                    .style(|_, _| button::Style { background: None, text_color: theme::GREEN, ..Default::default() }),
                                button(text("\u{2717}").font(MONO).size(14).color(theme::RED))
                                    .on_press(Message::RejectInvite(rid_reject))
                                    .padding([2, 6])
                                    .style(|_, _| button::Style { background: None, text_color: theme::RED, ..Default::default() }),
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
                }
            }
        }

        // Ungrouped channels
        if !ungrouped.is_empty() {
            col = col.push(self.view_section_header_with_icon("Channels", ungrouped.len(), Some(SVG_HASH), None));
            if !self.collapsed_sections.contains("Channels") {
                for room in &ungrouped {
                    col = col.push(self.view_room_entry(room, false));
                }
            }
        }

        // Space-grouped rooms
        for (space_name, rooms) in &by_space {
            // Find space avatar MXC
            let space_mxc = self.spaces.iter()
                .find(|s| s.name == *space_name)
                .and_then(|s| s.avatar_url.as_deref());
            col = col.push(self.view_section_header_with_icon(space_name, rooms.len(), Some(SVG_HASH), space_mxc));
            if !self.collapsed_sections.contains(space_name) {
                for room in rooms {
                    col = col.push(self.view_room_entry(room, false));
                }
            }
        }

        // DMs
        if !dms.is_empty() {
            col = col.push(self.view_section_header_with_icon("Direct Messages", dms.len(), Some(SVG_CHAT), None));
            if !self.collapsed_sections.contains("Direct Messages") {
                for room in &dms {
                    col = col.push(self.view_room_entry(room, true));
                }
            }
        }

        container(scrollable(col).height(Fill).direction(iced::widget::scrollable::Direction::Vertical(iced::widget::scrollable::Scrollbar::new().width(4).scroller_width(4))).style(thin_scrollbar))
            .width(Fill)
            .height(Fill)
            .style(|_: &Theme| container::Style {
                background: Some(theme::BG_DARK.into()),
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
            text(arrow).font(MONO).size(14).color(theme::TEXT_DIM),
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
                        color: Some(theme::TEXT_DIM),
                    }),
            );
        }

        header_row = header_row.push(
            text(name_label).font(MONO).size(14).color(theme::TEXT_BRIGHT)
        );
        header_row = header_row.push(Space::new().width(Fill));
        header_row = header_row.push(
            text(count.to_string()).font(MONO).size(12).color(theme::TEXT_MUTED)
        );

        button(header_row)
            .on_press(Message::ToggleSection(name_owned))
            .padding([6, 10])
            .width(Fill)
            .style(|_, _| button::Style {
                background: Some(Color::from_rgba(1.0, 1.0, 1.0, 0.02).into()),
                text_color: theme::TEXT_MUTED,
                ..Default::default()
            })
            .into()
    }

    /// Single room entry in the list.
    fn view_room_entry<'a>(&self, room: &'a fluxchat_core::RoomInfo, is_dm: bool) -> Element<'a, Message> {
        let is_selected = self.selected_room_id.as_deref() == Some(&room.room_id);
        let has_unread = room.unread_count > 0 && !self.read_rooms.contains(&room.room_id);
        let name_color = if is_selected || has_unread {
            theme::TEXT_BRIGHT
        } else {
            theme::TEXT_MUTED
        };
        let bg = if is_selected {
            theme::BG_LIGHT
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
                .style(|_, _| iced::widget::svg::Style { color: Some(Color::from_rgba(1.0, 1.0, 1.0, 0.3)) })
                .into()
        } else if room.is_encrypted {
            let handle = iced::widget::svg::Handle::from_memory(SVG_LOCK);
            iced::widget::svg(handle)
                .width(14).height(14)
                .style(|_, _| iced::widget::svg::Style { color: Some(Color::from_rgba(1.0, 1.0, 1.0, 0.3)) })
                .into()
        } else {
            let handle = iced::widget::svg::Handle::from_memory(SVG_HASH);
            iced::widget::svg(handle)
                .width(14).height(14)
                .style(|_, _| iced::widget::svg::Style { color: Some(Color::from_rgba(1.0, 1.0, 1.0, 0.3)) })
                .into()
        };

        let mut row_content = row![
            room_icon,
            text(&room.name)
                .font(MONO)
                .size(15)
                .color(name_color)
                .wrapping(iced::widget::text::Wrapping::None),
        ]
        .spacing(6)
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

            // Action buttons in tab bar
            let make_action_btn = |svg_bytes: &'static [u8], msg: Message, active: bool| -> Element<Message> {
                let icon_color = if active { theme::ACCENT } else { theme::TEXT_DIM };
                let handle = iced::widget::svg::Handle::from_memory(svg_bytes);
                button(
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
                })
                .into()
            };

            let actions = row![
                make_action_btn(SVG_SEARCH, Message::ToggleSearch, self.search_open),
                make_action_btn(SVG_USERS, Message::ToggleMemberList, self.show_member_list),
                make_action_btn(SVG_INFO, Message::OpenModal(crate::state::Modal::RoomSettings), false),
                make_action_btn(SVG_MORE, Message::ToggleOverflowMenu, self.overflow_menu_open),
            ]
            .spacing(2)
            .align_y(iced::Alignment::Center);

            let tab_row = row![
                tabs,
                actions,
            ].width(Fill).align_y(iced::Alignment::Center);

            content = content.push(
                container(tab_row)
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
                        container(scrollable(results_col).height(Length::Fixed(200.0)).direction(iced::widget::scrollable::Direction::Vertical(iced::widget::scrollable::Scrollbar::new().width(4).scroller_width(4))).style(thin_scrollbar))
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

            // Panels (pinned messages, threads, files, notices)
            if self.pins_open {
                let mut panel_col = column![
                    row![
                        text("PINNED MESSAGES").font(MONO).size(12).color(theme::TEXT_DIM),
                        Space::new().width(Fill),
                        button(text("\u{00d7}").font(MONO).size(16).color(theme::TEXT_DIM))
                            .on_press(Message::TogglePins)
                            .padding([0, 6])
                            .style(|_, _| button::Style { background: None, ..Default::default() }),
                    ].align_y(iced::Alignment::Center),
                ].spacing(4);
                if self.pinned_messages.is_empty() {
                    panel_col = panel_col.push(text("No pinned messages").font(MONO).size(13).color(theme::TEXT_DIM));
                } else {
                    for msg in self.pinned_messages.iter().take(20) {
                        panel_col = panel_col.push(
                            text(format!("{}: {}", msg.sender_name, msg.body))
                                .font(MONO).size(13).color(theme::TEXT_MUTED)
                        );
                    }
                }
                content = content.push(
                    container(panel_col).padding([8, 12]).width(Fill)
                        .style(|_: &Theme| container::Style {
                            background: Some(theme::BG_DARK.into()),
                            border: iced::Border { color: theme::BORDER, width: 1.0, radius: 0.0.into() },
                            ..Default::default()
                        })
                );
            }

            if self.threads_open {
                let panel_col = column![
                    row![
                        text("THREADS").font(MONO).size(12).color(theme::TEXT_DIM),
                        Space::new().width(Fill),
                        button(text("\u{00d7}").font(MONO).size(16).color(theme::TEXT_DIM))
                            .on_press(Message::ToggleThreads)
                            .padding([0, 6])
                            .style(|_, _| button::Style { background: None, ..Default::default() }),
                    ].align_y(iced::Alignment::Center),
                    text("Thread view coming soon").font(MONO).size(13).color(theme::TEXT_DIM),
                ].spacing(4);
                content = content.push(
                    container(panel_col).padding([8, 12]).width(Fill)
                        .style(|_: &Theme| container::Style {
                            background: Some(theme::BG_DARK.into()),
                            border: iced::Border { color: theme::BORDER, width: 1.0, radius: 0.0.into() },
                            ..Default::default()
                        })
                );
            }

            if self.files_open {
                let mut panel_col = column![
                    row![
                        text("FILES").font(MONO).size(12).color(theme::TEXT_DIM),
                        Space::new().width(Fill),
                        button(text("\u{00d7}").font(MONO).size(16).color(theme::TEXT_DIM))
                            .on_press(Message::ToggleFiles)
                            .padding([0, 6])
                            .style(|_, _| button::Style { background: None, ..Default::default() }),
                    ].align_y(iced::Alignment::Center),
                ].spacing(4);
                // Show images from messages
                if let Some(msgs) = self.messages_by_room.get(room_id) {
                    let files: Vec<_> = msgs.iter().filter(|m| m.image_mxc.is_some()).take(20).collect();
                    if files.is_empty() {
                        panel_col = panel_col.push(text("No files").font(MONO).size(13).color(theme::TEXT_DIM));
                    } else {
                        for msg in files {
                            panel_col = panel_col.push(
                                text(format!("\u{1f4ce} {}", msg.body)).font(MONO).size(13).color(theme::ACCENT)
                            );
                        }
                    }
                }
                content = content.push(
                    container(panel_col).padding([8, 12]).width(Fill)
                        .style(|_: &Theme| container::Style {
                            background: Some(theme::BG_DARK.into()),
                            border: iced::Border { color: theme::BORDER, width: 1.0, radius: 0.0.into() },
                            ..Default::default()
                        })
                );
            }

            if self.notices_open {
                let panel_col = column![
                    row![
                        text("NOTICES").font(MONO).size(12).color(theme::TEXT_DIM),
                        Space::new().width(Fill),
                        button(text("\u{00d7}").font(MONO).size(16).color(theme::TEXT_DIM))
                            .on_press(Message::ToggleNotices)
                            .padding([0, 6])
                            .style(|_, _| button::Style { background: None, ..Default::default() }),
                    ].align_y(iced::Alignment::Center),
                    text("No server notices").font(MONO).size(13).color(theme::TEXT_DIM),
                ].spacing(4);
                content = content.push(
                    container(panel_col).padding([8, 12]).width(Fill)
                        .style(|_: &Theme| container::Style {
                            background: Some(theme::BG_DARK.into()),
                            border: iced::Border { color: theme::BORDER, width: 1.0, radius: 0.0.into() },
                            ..Default::default()
                        })
                );
            }

            let messages = self.messages_by_room.get(room_id);
            let mut msg_col = column![].spacing(0).padding([2, 8]).width(Fill);
            let mut divider_shown = false;

            if self.paginating {
                msg_col = msg_col.push(
                    container(
                        text("Loading older messages...")
                            .font(MONO)
                            .size(13)
                            .color(theme::TEXT_DIM),
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
                for msg in msgs.iter() {
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
                    // Unread divider: show after the last-read message
                    if show_divider && !divider_shown {
                        if let Some(ref lr) = last_read {
                            if msg.event_id == *lr {
                                divider_shown = true;
                                msg_col = msg_col.push(self.view_message(msg));
                                msg_col = msg_col.push(
                                    container(
                                        row![
                                            rule::horizontal(1).style(|_: &Theme| rule::Style {
                                                color: theme::RED,
                                                radius: 0.0.into(),
                                                fill_mode: rule::FillMode::Full,
                                                snap: true,
                                            }),
                                            text(" NEW ").font(MONO).size(11).color(theme::RED),
                                            rule::horizontal(1).style(|_: &Theme| rule::Style {
                                                color: theme::RED,
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
                }
            } else {
                msg_col = msg_col.push(
                    text("Loading messages...")
                        .font(MONO)
                        .size(15)
                        .color(theme::TEXT_DIM),
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

            // Floating "new messages" pill when scrolled up and new messages arrived
            if !self.messages_at_bottom {
                let unread = self.rooms.iter()
                    .find(|r| Some(&r.room_id) == self.selected_room_id.as_ref())
                    .map(|r| r.unread_count)
                    .unwrap_or(0);
                if unread > 0 || divider_shown {
                    let pill = button(
                        text(if unread > 0 {
                            format!("\u{2193} {unread} new message{}", if unread == 1 { "" } else { "s" })
                        } else {
                            "\u{2193} New messages".to_string()
                        })
                        .font(MONO).size(12).color(Color::WHITE)
                    )
                    .on_press(Message::ScrollToBottom)
                    .padding([4, 12])
                    .style(|_, status| button::Style {
                        background: Some(if matches!(status, button::Status::Hovered) {
                            Color::from_rgba(0.23, 0.51, 0.96, 0.9)
                        } else {
                            Color::from_rgba(0.23, 0.51, 0.96, 0.75)
                        }.into()),
                        border: iced::Border { radius: 12.0.into(), ..Default::default() },
                        ..Default::default()
                    });
                    let overlay = container(pill)
                        .width(Fill)
                        .center_x(Fill)
                        .padding(8.0);
                    content = content.push(
                        stack![
                            container(msg_scrollable).width(Fill).height(Fill),
                            container(Space::new()).width(Fill).height(Fill), // spacer
                            container(overlay).width(Fill).align_y(iced::alignment::Vertical::Bottom),
                        ].width(Fill).height(Fill)
                    );
                } else {
                    content = content.push(msg_scrollable);
                }
            } else {
                content = content.push(msg_scrollable);
            }

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

            // GIF search panel
            if self.gifs_open {
                let mut gif_col = column![
                    row![
                        text("GIF SEARCH").font(MONO).size(12).color(theme::TEXT_DIM),
                        Space::new().width(Fill),
                        button(text("\u{00d7}").font(MONO).size(16).color(theme::TEXT_DIM))
                            .on_press(Message::ToggleGifs)
                            .padding([0, 6])
                            .style(|_, _| button::Style { background: None, ..Default::default() }),
                    ].align_y(iced::Alignment::Center),
                    row![
                        text_input("Search GIFs...", &self.gif_query)
                            .id(gif_search_id())
                            .on_input(Message::GifQueryChanged)
                            .on_submit(Message::GifSearchSubmit)
                            .font(MONO).size(14).padding(6),
                    ],
                ].spacing(4);

                if self.gif_searching {
                    gif_col = gif_col.push(text("Searching...").font(MONO).size(13).color(theme::TEXT_DIM));
                } else if self.gif_results.is_empty() && !self.gif_query.is_empty() {
                    gif_col = gif_col.push(text("No results").font(MONO).size(13).color(theme::TEXT_DIM));
                } else {
                    // Grid of GIF results (3 columns)
                    let mut grid = column![].spacing(4);
                    for chunk in self.gif_results.chunks(3) {
                        let mut row_items = row![].spacing(4);
                        for gif in chunk {
                            let url = gif.url.clone();
                            let desc = gif.description.clone();
                            let content: Element<Message> = if let Some(frames) = self.gif_preview_cache.get(&gif.preview_url) {
                                iced_gif::gif(frames.as_ref())
                                    .width(Fill)
                                    .content_fit(iced::ContentFit::ScaleDown)
                                    .into()
                            } else {
                                text(&gif.description)
                                    .font(MONO).size(11).color(theme::TEXT_DIM)
                                    .into()
                            };
                            row_items = row_items.push(
                                button(content)
                                    .on_press(Message::GifSelect(url, desc))
                                    .padding(2)
                                    .width(Length::FillPortion(1))
                                    .style(|_, status| button::Style {
                                        background: Some(if matches!(status, button::Status::Hovered) {
                                            Color::from_rgba(1.0, 1.0, 1.0, 0.1)
                                        } else {
                                            theme::BG_DARK
                                        }.into()),
                                        border: iced::Border { radius: 4.0.into(), ..Default::default() },
                                        ..Default::default()
                                    }),
                            );
                        }
                        grid = grid.push(row_items);
                    }
                    gif_col = gif_col.push(
                        scrollable(grid)
                            .height(Length::Fixed(200.0))
                            .direction(iced::widget::scrollable::Direction::Vertical(
                                iced::widget::scrollable::Scrollbar::new().width(4).scroller_width(4),
                            ))
                            .style(thin_scrollbar)
                    );
                }

                content = content.push(
                    container(gif_col).padding([8, 12]).width(Fill)
                        .style(|_: &Theme| container::Style {
                            background: Some(theme::BG_DARK.into()),
                            border: iced::Border { color: theme::BORDER, width: 1.0, radius: 0.0.into() },
                            ..Default::default()
                        })
                );
            }

            // Emoji picker panel
            if self.emoji_open {
                content = content.push(self.view_emoji_picker());
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

            let input = iced::widget::TextEditor::new(&self.input_content)
                .id(input_id())
                .placeholder("Type a message or /command...")
                .on_action(Message::InputAction)
                .key_binding({
                    let has_completions = !self.cmd_completions.is_empty();
                    let has_mentions = !self.mention_completions.is_empty();
                    let input_empty = self.input_text.trim().is_empty();
                    move |key_press| {
                        use iced::keyboard::Key;
                        use iced::keyboard::key::Named;
                        use iced::widget::text_editor::Binding;
                        match key_press.key {
                            // Enter without Shift = send message
                            Key::Named(Named::Enter) if !key_press.modifiers.shift() => {
                                Some(Binding::Custom(Message::SendMessage))
                            }
                            // Tab = cycle autocomplete
                            Key::Named(Named::Tab) if has_completions => {
                                Some(Binding::Custom(Message::InputChanged("/\t".into())))
                            }
                            Key::Named(Named::Tab) if has_mentions => {
                                Some(Binding::Custom(Message::InputChanged("@\t".into())))
                            }
                            // Up arrow on empty input = edit last message
                            Key::Named(Named::ArrowUp) if input_empty => {
                                Some(Binding::Custom(Message::InputChanged("\x1b[A".into())))
                            }
                            _ => Binding::from_key_press(key_press),
                        }
                    }
                })
                .font(MONO)
                .size(14)
                .height(Length::Shrink)
                .max_height(120)
                .padding([8, 4])
                .style(|_: &Theme, _| iced::widget::text_editor::Style {
                    background: iced::Background::Color(Color::TRANSPARENT),
                    border: iced::Border::default(),
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
                .on_press(Message::ToggleEmojiPicker)
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

        let base: Element<Message> = container(content)
            .width(Fill)
            .height(Fill)
            .style(|_: &Theme| container::Style {
                background: Some(theme::BG_MID.into()),
                ..Default::default()
            })
            .into();

        if !self.overflow_menu_open {
            return base;
        }

        // Overflow menu as floating overlay
        let make_menu_item = |svg_bytes: &'static [u8], label: &'static str, msg: Message| -> Element<Message> {
            let handle = iced::widget::svg::Handle::from_memory(svg_bytes);
            button(
                row![
                    iced::widget::svg(handle)
                        .width(14).height(14)
                        .style(|_, _| iced::widget::svg::Style { color: Some(theme::TEXT_DIM) }),
                    text(label).font(MONO).size(14).color(theme::TEXT_NORMAL),
                ]
                .spacing(10)
                .align_y(iced::Alignment::Center),
            )
            .on_press(msg)
            .padding([6, 14])
            .width(Fill)
            .style(|_, status| button::Style {
                background: if matches!(status, button::Status::Hovered) {
                    Some(Color::from_rgba(1.0, 1.0, 1.0, 0.06).into())
                } else {
                    None
                },
                text_color: theme::TEXT_NORMAL,
                ..Default::default()
            })
            .into()
        };

        let menu = container(
            column![
                make_menu_item(SVG_STAR, "Favourite", Message::ToggleFavourite),
                make_menu_item(SVG_COPY, "Copy link", Message::CopyRoomLink),
                container(Space::new()).width(Fill).height(1)
                    .style(|_: &Theme| container::Style {
                        background: Some(theme::BORDER.into()),
                        ..Default::default()
                    }),
                make_menu_item(SVG_PIN, "Pinned messages", Message::TogglePins),
                make_menu_item(SVG_THREAD, "Threads", Message::ToggleThreads),
                make_menu_item(SVG_FILE, "Files", Message::ToggleFiles),
                make_menu_item(SVG_GIF, "GIFs", Message::ToggleGifs),
                make_menu_item(SVG_BELL, "Notices", Message::ToggleNotices),
            ]
            .spacing(0)
        )
        .width(180)
        .padding([6, 0])
        .style(|_: &Theme| container::Style {
            background: Some(theme::BG_MID.into()),
            border: iced::Border {
                color: theme::BORDER,
                width: 1.0,
                radius: 6.0.into(),
            },
            shadow: iced::Shadow {
                color: Color::from_rgba(0.0, 0.0, 0.0, 0.5),
                offset: iced::Vector::new(0.0, 4.0),
                blur_radius: 16.0,
            },
            ..Default::default()
        });

        let overlay: Element<Message> = mouse_area(
            container(
                column![
                    row![
                        Space::new().width(Fill),
                        menu,
                    ],
                    Space::new().height(Fill),
                ]
            )
            .width(Fill)
            .height(Fill)
            .padding(iced::Padding { top: 32.0, right: 8.0, bottom: 0.0, left: 0.0 })
        )
        .on_press(Message::ToggleOverflowMenu)
        .into();

        stack![base, overlay].width(Fill).height(Fill).into()
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

        // Build the content element — image, video, or text
        let body_element: Element<'a, Message> = if let Some(ref mxc) = msg.image_mxc {
            if !msg.is_video {
                // Image messages
                if let Some(frames) = self.gif_frames_cache.get(mxc) {
                    let img_w = msg.image_width.unwrap_or(320).min(400) as f32;
                    let mxc_click = mxc.clone();
                    mouse_area(
                        iced_gif::gif(frames.as_ref())
                            .width(img_w)
                            .content_fit(iced::ContentFit::ScaleDown)
                    )
                    .on_press(Message::OpenModal(crate::state::Modal::Lightbox { url: mxc_click }))
                    .into()
                } else if let Some(handle) = self.image_cache.get(mxc) {
                    let img_w = msg.image_width.unwrap_or(320).min(400) as f32;
                    let img_h = if let (Some(w), Some(h)) = (msg.image_width, msg.image_height) {
                        (img_w * h as f32 / w as f32).min(300.0)
                    } else {
                        240.0
                    };
                    let mxc_click = mxc.clone();
                    mouse_area(
                        container(
                            iced::widget::image(handle.clone())
                                .width(img_w)
                                .height(img_h)
                                .content_fit(iced::ContentFit::Contain),
                        )
                        .clip(true)
                        .style(|_: &Theme| container::Style {
                            border: iced::Border { radius: 4.0.into(), ..Default::default() },
                            ..Default::default()
                        })
                    )
                    .on_press(Message::OpenModal(crate::state::Modal::Lightbox { url: mxc_click }))
                    .into()
                } else {
                    text(format!("[image: {}]", msg.body))
                        .font(MONO).size(15).color(theme::TEXT_DIM)
                        .into()
                }
            } else {
                // Video messages
                text(format!("[video: {}]", msg.body))
                    .font(MONO).size(15).color(theme::PURPLE).width(Fill)
                    .into()
            }
        } else if msg.is_video {
            text(format!("[video: {}]", msg.body))
                .font(MONO).size(15).color(theme::PURPLE).width(Fill)
                .into()
        } else if let Some(ref formatted) = msg.formatted_body {
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
        // Message row with hover action bar overlay
        let is_hovered = self.hovered_message.as_deref() == Some(&msg.event_id);
        let event_id = msg.event_id.clone();
        let eid_hover = msg.event_id.clone();

        let base_row: Element<'a, Message> = row![time, nick, msg_avatar, sep, body_element]
            .spacing(6).width(Fill).align_y(iced::Alignment::Start).into();

        // Wrap in container with hover highlight
        let hover_bg = if is_hovered {
            Color::from_rgba(1.0, 1.0, 1.0, 0.03)
        } else {
            Color::TRANSPARENT
        };
        let base_row: Element<'a, Message> = container(base_row)
            .width(Fill)
            .padding([5, 0])
            .style(move |_: &Theme| container::Style {
                background: Some(hover_bg.into()),
                ..Default::default()
            })
            .into();

        let msg_row: Element<'a, Message> = if is_hovered {
            let make_msg_action = |svg_bytes: &'static [u8], msg_action: Message| -> Element<'a, Message> {
                let handle = iced::widget::svg::Handle::from_memory(svg_bytes);
                button(
                    iced::widget::svg(handle)
                        .width(Length::Fixed(16.0)).height(Length::Fixed(16.0))
                        .content_fit(iced::ContentFit::Fill)
                        .style(|_, _| iced::widget::svg::Style { color: Some(theme::TEXT_MUTED) }),
                )
                .on_press(msg_action)
                .padding(5)
                .width(Length::Fixed(26.0))
                .height(Length::Fixed(26.0))
                .style(|_, status| button::Style {
                    background: if matches!(status, button::Status::Hovered) {
                        Some(Color::from_rgba(1.0, 1.0, 1.0, 0.12).into())
                    } else {
                        None
                    },
                    border: iced::Border { radius: 4.0.into(), ..Default::default() },
                    ..Default::default()
                })
                .into()
            };

            let is_own = msg.sender == self.user_id;
            let mut action_row = row![].spacing(2).align_y(iced::Alignment::Center);

            action_row = action_row.push(make_msg_action(SVG_REACT, Message::OpenEmojiReact(event_id.clone())));
            action_row = action_row.push(make_msg_action(SVG_CLIPBOARD, Message::CopyMessageText(msg.body.clone())));
            action_row = action_row.push(make_msg_action(SVG_REPLY, Message::StartReply(
                event_id.clone(),
                msg.sender_name.clone(),
                msg.body.chars().take(80).collect(),
            )));

            if is_own {
                action_row = action_row.push(make_msg_action(SVG_EDIT, Message::StartEdit(
                    event_id.clone(),
                    msg.body.clone(),
                )));
                action_row = action_row.push(make_msg_action(SVG_DELETE, Message::DeleteMessage(event_id.clone())));
            }

            let action_bar: Element<'a, Message> = container(
                row![
                    Space::new().width(Fill),
                    container(action_row)
                        .padding([2, 4])
                        .style(|_: &Theme| container::Style {
                            background: Some(theme::BG_DARK.into()),
                            border: iced::Border {
                                color: theme::BORDER,
                                width: 1.0,
                                radius: 5.0.into(),
                            },
                            shadow: iced::Shadow {
                                color: Color::from_rgba(0.0, 0.0, 0.0, 0.3),
                                offset: iced::Vector::new(0.0, 2.0),
                                blur_radius: 8.0,
                            },
                            ..Default::default()
                        }),
                ]
                .align_y(iced::Alignment::Start)
            )
            .width(Fill)
            .height(Length::Shrink)
            .padding([0, 8])
            .into();

            let stacked = mouse_area(
                stack![base_row, action_bar].width(Fill)
            )
            .on_enter(Message::HoverMessage(Some(eid_hover)));

            stacked.into()
        } else {
            mouse_area(base_row)
                .on_enter(Message::HoverMessage(Some(eid_hover)))
                .on_exit(Message::HoverMessage(None))
                .into()
        };

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
            let is_mine = reaction.users.iter().any(|u| u == &self.user_id);
            let bg = if is_mine {
                Color::from_rgba(0.23, 0.51, 0.96, 0.15) // ACCENT tint
            } else {
                Color::from_rgba(1.0, 1.0, 1.0, 0.06)
            };

            reactions_row = reactions_row.push(
                button(
                    row![
                        text(&reaction.emoji).size(16),
                        text(reaction.count.to_string()).font(MONO).size(12).color(theme::TEXT_MUTED),
                    ].spacing(4).align_y(iced::Alignment::Center)
                )
                    .on_press(Message::ToggleReaction(event_id, emoji))
                    .padding([2, 8])
                    .style(move |_, status| button::Style {
                        background: Some(if matches!(status, button::Status::Hovered) {
                            Color::from_rgba(1.0, 1.0, 1.0, 0.12)
                        } else {
                            bg
                        }.into()),
                        border: iced::Border {
                            radius: 10.0.into(),
                            ..Default::default()
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
    /// Render the emoji picker panel.
    fn view_emoji_picker(&self) -> Element<Message> {
        let query_lower = self.emoji_query.to_lowercase();

        // Header row
        let title = if matches!(self.emoji_mode, crate::state::EmojiPickerMode::React(_)) {
            "REACT"
        } else {
            "EMOJI"
        };
        let header = row![
            text(title).font(MONO).size(12).color(theme::TEXT_DIM),
            text_input("Search emoji...", &self.emoji_query)
                .id(emoji_search_id())
                .on_input(Message::EmojiQueryChanged)
                .font(MONO).size(14).padding(6)
                .width(Fill),
            button(text("\u{00d7}").font(MONO).size(16).color(theme::TEXT_DIM))
                .on_press(Message::ToggleEmojiPicker)
                .padding([0, 6])
                .style(|_, _| button::Style { background: None, ..Default::default() }),
        ].spacing(8).align_y(iced::Alignment::Center);

        // Category tabs
        let category_labels = [
            "\u{2605}", // ★ frequent
            "\u{1f600}", // 😀 SmileysAndEmotion
            "\u{1f464}", // 👤 PeopleAndBody
            "\u{1f43e}", // 🐾 AnimalsAndNature
            "\u{1f34e}", // 🍎 FoodAndDrink
            "\u{2708}",  // ✈ TravelAndPlaces
            "\u{26bd}",  // ⚽ Activities
            "\u{1f4a1}", // 💡 Objects
            "\u{2660}",  // ♠ Symbols
            "\u{1f3f4}", // 🏴 Flags
        ];
        let mut tabs = row![].spacing(2);
        for (i, label) in category_labels.iter().enumerate() {
            let is_active = self.emoji_category == i as u8;
            tabs = tabs.push(
                button(text(*label).size(18).center().width(EMOJI_CELL_SIZE))
                    .on_press(Message::EmojiCategoryChanged(i as u8))
                    .padding([2, 0])
                    .style(move |_, status| button::Style {
                        background: Some(if is_active {
                            Color::from_rgba(1.0, 1.0, 1.0, 0.12)
                        } else if matches!(status, button::Status::Hovered) {
                            Color::from_rgba(1.0, 1.0, 1.0, 0.06)
                        } else {
                            Color::TRANSPARENT
                        }.into()),
                        border: iced::Border { radius: 4.0.into(), ..Default::default() },
                        ..Default::default()
                    }),
            );
        }

        // Collect emojis to display
        let emojis_to_show: Vec<&str> = if !query_lower.is_empty() {
            // Search mode
            emojis::iter()
                .filter(|e| e.skin_tone().is_none())
                .filter(|e| {
                    e.name().to_lowercase().contains(&query_lower)
                        || e.shortcode().is_some_and(|s| s.to_lowercase().contains(&query_lower))
                })
                .map(|e| e.as_str())
                .take(200)
                .collect()
        } else if self.emoji_category == 0 {
            // Frequently used
            let mut freq: Vec<_> = self.emoji_freq.iter().collect();
            freq.sort_by(|a, b| b.1.cmp(a.1));
            freq.into_iter()
                .take(EMOJI_FREQ_COUNT)
                .map(|(emoji, _)| emoji.as_str())
                .collect()
        } else {
            // Category
            let group = match self.emoji_category {
                1 => emojis::Group::SmileysAndEmotion,
                2 => emojis::Group::PeopleAndBody,
                3 => emojis::Group::AnimalsAndNature,
                4 => emojis::Group::FoodAndDrink,
                5 => emojis::Group::TravelAndPlaces,
                6 => emojis::Group::Activities,
                7 => emojis::Group::Objects,
                8 => emojis::Group::Symbols,
                _ => emojis::Group::Flags,
            };
            group.emojis()
                .filter(|e| e.skin_tone().is_none())
                .map(|e| e.as_str())
                .collect()
        };

        // Build emoji grid
        let mut grid = column![].spacing(2);
        if emojis_to_show.is_empty() {
            let hint = if self.emoji_category == 0 && query_lower.is_empty() {
                "No frequently used emoji yet"
            } else {
                "No results"
            };
            grid = grid.push(text(hint).font(MONO).size(13).color(theme::TEXT_DIM));
        } else {
            for chunk in emojis_to_show.chunks(EMOJI_GRID_COLUMNS) {
                let mut row_items = row![].spacing(2);
                for emoji in chunk {
                    let e = emoji.to_string();
                    row_items = row_items.push(
                        button(
                            text(*emoji).size(22).center()
                                .width(EMOJI_CELL_SIZE).height(EMOJI_CELL_SIZE)
                        )
                        .on_press(Message::EmojiSelect(e))
                        .padding(0)
                        .style(|_, status| button::Style {
                            background: Some(if matches!(status, button::Status::Hovered) {
                                Color::from_rgba(1.0, 1.0, 1.0, 0.1)
                            } else {
                                Color::TRANSPARENT
                            }.into()),
                            border: iced::Border { radius: 4.0.into(), ..Default::default() },
                            ..Default::default()
                        }),
                    );
                }
                grid = grid.push(row_items);
            }
        }

        let panel = column![
            header,
            tabs,
            scrollable(grid)
                .height(Length::Fixed(EMOJI_PANEL_HEIGHT))
                .direction(iced::widget::scrollable::Direction::Vertical(
                    iced::widget::scrollable::Scrollbar::new().width(4).scroller_width(4),
                ))
                .style(thin_scrollbar),
        ].spacing(4);

        let picker = container(panel).padding([8, 12]).width(Length::Fixed(380.0))
            .style(|_: &Theme| container::Style {
                background: Some(theme::BG_DARK.into()),
                border: iced::Border { color: theme::BORDER, width: 1.0, radius: 4.0.into() },
                ..Default::default()
            });
        // Right-align the picker
        container(picker).width(Fill).align_x(iced::alignment::Horizontal::Right)
            .into()
    }

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

        container(scrollable(col).height(Fill).direction(iced::widget::scrollable::Direction::Vertical(iced::widget::scrollable::Scrollbar::new().width(4).scroller_width(4))).style(thin_scrollbar))
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
    fn view_member_section<'a>(
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
    fn view_member_row<'a>(&'a self, member: &'a fluxchat_core::MemberInfo) -> Element<'a, Message> {
        let nick_col = theme::nick_color(&member.display_name);
        let presence_color = match member.presence.as_str() {
            "online" => theme::GREEN,
            "idle" => theme::YELLOW,
            _ => Color::TRANSPARENT,
        };
        let user_id = member.user_id.clone();

        // Avatar — use cached image or fallback to initial
        let avatar_img: Element<'a, Message> = if let Some(handle) = member.avatar_url.as_ref().and_then(|mxc| self.avatar_cache.get(mxc)) {
            let handle = handle.clone();
            container(
                iced::widget::image(handle)
                    .width(28)
                    .height(28)
                    .content_fit(iced::ContentFit::Cover),
            )
            .width(28)
            .height(28)
            .clip(true)
            .style(|_: &Theme| container::Style {
                border: iced::Border { radius: 14.0.into(), ..Default::default() },
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
                    .size(12)
                    .color(theme::TEXT_MUTED),
            )
            .width(28)
            .height(28)
            .center_x(Fill)
            .center_y(Fill)
            .style(|_: &Theme| container::Style {
                background: Some(theme::BG_LIGHT.into()),
                border: iced::Border { radius: 14.0.into(), ..Default::default() },
                ..Default::default()
            })
            .into()
        };

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
                        scrollable(col).height(Length::Shrink).direction(iced::widget::scrollable::Direction::Vertical(iced::widget::scrollable::Scrollbar::new().width(4).scroller_width(4))).style(thin_scrollbar).into()
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
                        scrollable(col).height(Length::Shrink).direction(iced::widget::scrollable::Direction::Vertical(iced::widget::scrollable::Scrollbar::new().width(4).scroller_width(4))).style(thin_scrollbar).into()
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

                        scrollable(col).height(Length::Shrink).direction(iced::widget::scrollable::Direction::Vertical(iced::widget::scrollable::Scrollbar::new().width(4).scroller_width(4))).style(thin_scrollbar).into()
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
                let public_label = if self.create_room_public { "\u{25c9} Public" } else { "\u{25cb} Public" };
                let encrypted_label = if self.create_room_encrypted { "\u{25c9} Encrypted" } else { "\u{25cb} Encrypted" };

                // Space selector
                let space_label = self.create_room_space.as_ref()
                    .and_then(|sid| self.spaces.iter().find(|s| s.room_id == *sid))
                    .map(|s| s.name.as_str())
                    .unwrap_or("None (top-level)");

                let mut space_buttons = row![
                    button(text("None").font(MONO).size(13).color(
                        if self.create_room_space.is_none() { theme::ACCENT } else { theme::TEXT_MUTED }
                    ))
                    .on_press(Message::CreateRoomSelectSpace(None))
                    .padding([3, 8])
                    .style(move |_, _| button::Style {
                        background: None,
                        border: iced::Border { radius: 4.0.into(), ..Default::default() },
                        ..Default::default()
                    }),
                ].spacing(4);

                for space in &self.spaces {
                    let sid = space.room_id.clone();
                    let is_sel = self.create_room_space.as_deref() == Some(&space.room_id);
                    let color = if is_sel { theme::ACCENT } else { theme::TEXT_MUTED };
                    space_buttons = space_buttons.push(
                        button(text(&space.name).font(MONO).size(13).color(color))
                            .on_press(Message::CreateRoomSelectSpace(Some(sid)))
                            .padding([3, 8])
                            .style(move |_, _| button::Style {
                                background: None,
                                border: iced::Border { radius: 4.0.into(), ..Default::default() },
                                ..Default::default()
                            }),
                    );
                }

                column![
                    row![
                        text("Create Room").font(MONO).size(16).color(theme::TEXT_BRIGHT),
                        Space::new().width(Fill),
                        button(text("\u{00d7}").font(MONO).size(18).color(theme::TEXT_DIM))
                            .on_press(Message::CloseModal)
                            .padding([2, 6])
                            .style(|_, _| button::Style { background: None, ..Default::default() }),
                    ].align_y(iced::Alignment::Center),
                    text("Room Name").font(MONO).size(13).color(theme::TEXT_DIM),
                    text_input("e.g. my-room", &self.create_room_name)
                        .on_input(Message::CreateRoomNameChanged)
                        .on_submit(Message::CreateRoomSubmit)
                        .font(MONO).size(14).padding(8),
                    text("Topic (optional)").font(MONO).size(13).color(theme::TEXT_DIM),
                    text_input("Room topic...", &self.create_room_topic)
                        .on_input(Message::CreateRoomTopicChanged)
                        .font(MONO).size(14).padding(8),
                    text(format!("Space: {space_label}")).font(MONO).size(13).color(theme::TEXT_DIM),
                    scrollable(space_buttons)
                        .direction(iced::widget::scrollable::Direction::Horizontal(
                            iced::widget::scrollable::Scrollbar::new().width(0).scroller_width(0),
                        )),
                    row![
                        button(text(public_label).font(MONO).size(14).color(theme::TEXT_MUTED))
                            .on_press(Message::CreateRoomTogglePublic)
                            .padding([4, 8])
                            .style(|_, _| button::Style { background: None, ..Default::default() }),
                        button(text(encrypted_label).font(MONO).size(14).color(theme::TEXT_MUTED))
                            .on_press(Message::CreateRoomToggleEncrypted)
                            .padding([4, 8])
                            .style(|_, _| button::Style { background: None, ..Default::default() }),
                    ].spacing(12),
                    button(text("Create").font(MONO).size(14).color(theme::BG_DEEP))
                        .on_press(Message::CreateRoomSubmit)
                        .padding([8, 24])
                        .style(|_, _| button::Style {
                            background: Some(theme::ACCENT.into()),
                            text_color: theme::BG_DEEP,
                            border: iced::Border { radius: 4.0.into(), ..Default::default() },
                            ..Default::default()
                        }),
                ]
                .spacing(10)
                .width(450)
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

                // Action buttons
                profile_col = profile_col.push(rule::horizontal(1));

                let uid = user_id.clone();
                let ignore_label = if is_ignored { "Unignore" } else { "Ignore" };
                let ignore_color = if is_ignored { theme::GREEN } else { theme::RED };

                let mut actions_row = row![
                    button(text(ignore_label).font(MONO).size(13).color(ignore_color))
                        .on_press(Message::ToggleIgnoreUser(uid.clone()))
                        .padding([4, 12])
                        .style(move |_, _| button::Style {
                            background: Some(theme::BG_LIGHT.into()),
                            text_color: ignore_color,
                            border: iced::Border { radius: 4.0.into(), ..Default::default() },
                            ..Default::default()
                        }),
                ].spacing(8);

                if uid != self.user_id {
                    actions_row = actions_row.push(
                        button(text("Message").font(MONO).size(13).color(theme::ACCENT))
                            .on_press(Message::SendDmToUser(uid))
                            .padding([4, 12])
                            .style(|_, _| button::Style {
                                background: Some(theme::BG_LIGHT.into()),
                                text_color: theme::ACCENT,
                                border: iced::Border { radius: 4.0.into(), ..Default::default() },
                                ..Default::default()
                            }),
                    );
                }

                profile_col = profile_col.push(actions_row);

                if is_ignored {
                    profile_col = profile_col.push(
                        text("(ignored)").font(MONO).size(14).color(theme::RED),
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
                let image_el: Element<Message> = if let Some(handle) = self.image_cache.get(url) {
                    iced::widget::image(handle.clone())
                        .content_fit(iced::ContentFit::Contain)
                        .width(Fill)
                        .height(Fill)
                        .into()
                } else {
                    text("Loading...").font(MONO).size(14).color(theme::TEXT_DIM).into()
                };

                column![
                    row![
                        Space::new().width(Fill),
                        button(text("\u{00d7}").font(MONO).size(20).color(theme::TEXT_BRIGHT))
                            .on_press(Message::CloseModal)
                            .padding([4, 10])
                            .style(|_, status| button::Style {
                                background: if matches!(status, button::Status::Hovered) {
                                    Some(Color::from_rgba(1.0, 1.0, 1.0, 0.1).into())
                                } else {
                                    None
                                },
                                border: iced::Border { radius: 4.0.into(), ..Default::default() },
                                ..Default::default()
                            }),
                    ]
                    .align_y(iced::Alignment::Center),
                    container(image_el)
                        .width(Fill)
                        .height(Fill)
                        .center(Fill)
                        .clip(true),
                ]
                .spacing(4)
                .width(Length::FillPortion(9))
                .height(Length::FillPortion(9))
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
                    text(format!("Message: {}", &body.chars().take(80).collect::<String>()))
                        .font(MONO).size(13).color(theme::TEXT_MUTED),
                    rule::horizontal(1),
                    text("Send to:").font(MONO).size(13).color(theme::TEXT_DIM),
                    text_input("Search rooms...", &self.forward_search)
                        .on_input(Message::ForwardSearchChanged)
                        .font(MONO).size(14).padding(6),
                    {
                        let query = self.forward_search.to_lowercase();
                        let mut room_list = column![].spacing(2);
                        for room in self.rooms.iter()
                            .filter(|r| query.is_empty() || r.name.to_lowercase().contains(&query))
                            .take(10)
                        {
                            let rid = room.room_id.clone();
                            room_list = room_list.push(
                                button(text(&room.name).font(MONO).size(14).color(theme::TEXT_NORMAL))
                                    .on_press(Message::ForwardToRoom(rid))
                                    .padding([4, 8])
                                    .width(Fill)
                                    .style(|_, status| button::Style {
                                        background: if matches!(status, button::Status::Hovered) {
                                            Some(Color::from_rgba(1.0, 1.0, 1.0, 0.06).into())
                                        } else { None },
                                        border: iced::Border { radius: 4.0.into(), ..Default::default() },
                                        ..Default::default()
                                    }),
                            );
                        }
                        room_list
                    },
                ]
                .spacing(8)
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
                        button(text("\u{00d7}").font(MONO).size(18).color(theme::TEXT_DIM))
                            .on_press(Message::CloseModal)
                            .padding([2, 6])
                            .style(|_, _| button::Style {
                                background: None,
                                ..Default::default()
                            }),
                    ]
                    .align_y(iced::Alignment::Center),
                    {
                        let room = self.selected_room_id.as_ref()
                            .and_then(|rid| self.rooms.iter().find(|r| r.room_id == *rid));
                        let mut info_col = column![].spacing(6);
                        if let Some(room) = room {
                            // Avatar row
                            let avatar_el: Element<Message> = if let Some(handle) = room.avatar_url.as_ref().and_then(|mxc| self.avatar_cache.get(mxc)) {
                                container(
                                    iced::widget::image(handle.clone())
                                        .width(48).height(48)
                                        .content_fit(iced::ContentFit::Cover),
                                ).width(48).height(48).clip(true)
                                .style(|_: &Theme| container::Style {
                                    border: iced::Border { radius: 8.0.into(), ..Default::default() },
                                    ..Default::default()
                                }).into()
                            } else {
                                container(text("#").font(MONO).size(20).color(theme::TEXT_DIM))
                                    .width(48).height(48)
                                    .center_x(Length::Fixed(48.0)).center_y(Length::Fixed(48.0))
                                    .style(|_: &Theme| container::Style {
                                        background: Some(theme::BG_LIGHT.into()),
                                        border: iced::Border { radius: 8.0.into(), ..Default::default() },
                                        ..Default::default()
                                    }).into()
                            };
                            info_col = info_col.push(
                                row![
                                    avatar_el,
                                    button(text("Upload").font(MONO).size(12).color(theme::ACCENT))
                                        .on_press(Message::RoomSettingsUploadAvatar)
                                        .padding([4, 8])
                                        .style(|_, _| button::Style {
                                            background: Some(theme::BG_LIGHT.into()),
                                            text_color: theme::ACCENT,
                                            border: iced::Border { radius: 4.0.into(), ..Default::default() },
                                            ..Default::default()
                                        }),
                                    button(text("Remove").font(MONO).size(12).color(theme::TEXT_DIM))
                                        .on_press(Message::RoomSettingsRemoveAvatar)
                                        .padding([4, 8])
                                        .style(|_, _| button::Style {
                                            background: None, text_color: theme::TEXT_DIM,
                                            border: iced::Border { radius: 4.0.into(), ..Default::default() },
                                            ..Default::default()
                                        }),
                                ].spacing(8).align_y(iced::Alignment::Center)
                            );

                            // Editable name
                            info_col = info_col.push(text("Name").font(MONO).size(11).color(theme::TEXT_DIM));
                            info_col = info_col.push(
                                row![
                                    text_input("Room name", &self.room_settings_name)
                                        .on_input(Message::RoomSettingsNameChanged)
                                        .on_submit(Message::RoomSettingsSaveName)
                                        .font(MONO).size(13).padding(6),
                                    button(text("Save").font(MONO).size(12).color(theme::ACCENT))
                                        .on_press(Message::RoomSettingsSaveName)
                                        .padding([6, 10])
                                        .style(|_, _| button::Style {
                                            background: Some(theme::BG_LIGHT.into()),
                                            text_color: theme::ACCENT,
                                            border: iced::Border { radius: 4.0.into(), ..Default::default() },
                                            ..Default::default()
                                        }),
                                ].spacing(6)
                            );

                            // Editable topic
                            info_col = info_col.push(text("Topic").font(MONO).size(11).color(theme::TEXT_DIM));
                            info_col = info_col.push(
                                row![
                                    text_input("Room topic", &self.room_settings_topic)
                                        .on_input(Message::RoomSettingsTopicChanged)
                                        .on_submit(Message::RoomSettingsSaveTopic)
                                        .font(MONO).size(13).padding(6),
                                    button(text("Save").font(MONO).size(12).color(theme::ACCENT))
                                        .on_press(Message::RoomSettingsSaveTopic)
                                        .padding([6, 10])
                                        .style(|_, _| button::Style {
                                            background: Some(theme::BG_LIGHT.into()),
                                            text_color: theme::ACCENT,
                                            border: iced::Border { radius: 4.0.into(), ..Default::default() },
                                            ..Default::default()
                                        }),
                                ].spacing(6)
                            );

                            info_col = info_col.push(rule::horizontal(1));
                            info_col = info_col.push(
                                row![
                                    text("Room ID:").font(MONO).size(13).color(theme::TEXT_DIM),
                                    text(&room.room_id).font(MONO).size(12).color(theme::TEXT_DIM),
                                ].spacing(8)
                            );
                            info_col = info_col.push(
                                row![
                                    text("Encrypted:").font(MONO).size(13).color(theme::TEXT_DIM),
                                    text(if room.is_encrypted { "Yes" } else { "No" }).font(MONO).size(14)
                                        .color(if room.is_encrypted { theme::GREEN } else { theme::TEXT_MUTED }),
                                ].spacing(8)
                            );
                            // Space membership
                            info_col = info_col.push(rule::horizontal(1));
                            info_col = info_col.push(
                                text("SPACES").font(MONO).size(12).color(theme::TEXT_DIM)
                            );

                            let current_space = room.space_id.as_deref();
                            for space in &self.spaces {
                                let is_in = current_space == Some(&space.room_id);
                                let sid = space.room_id.clone();
                                let (label, color, msg) = if is_in {
                                    (format!("\u{2713} {}", space.name), theme::GREEN,
                                     Message::RemoveRoomFromSpace(sid))
                                } else {
                                    (format!("+ {}", space.name), theme::TEXT_MUTED,
                                     Message::AddRoomToSpace(sid))
                                };
                                info_col = info_col.push(
                                    button(text(label).font(MONO).size(14).color(color))
                                        .on_press(msg)
                                        .padding([3, 8])
                                        .width(Fill)
                                        .style(move |_, status| button::Style {
                                            background: if matches!(status, button::Status::Hovered) {
                                                Some(Color::from_rgba(1.0, 1.0, 1.0, 0.04).into())
                                            } else { None },
                                            border: iced::Border { radius: 4.0.into(), ..Default::default() },
                                            ..Default::default()
                                        }),
                                );
                            }

                            info_col = info_col.push(rule::horizontal(1));
                            let rid = room.room_id.clone();
                            info_col = info_col.push(
                                button(text("Leave Room").font(MONO).size(14).color(theme::RED))
                                    .on_press(Message::LeaveRoom(rid))
                                    .padding([6, 16])
                                    .style(|_, _| button::Style {
                                        background: Some(theme::BG_LIGHT.into()),
                                        text_color: theme::RED,
                                        border: iced::Border { radius: 4.0.into(), ..Default::default() },
                                        ..Default::default()
                                    }),
                            );
                        } else {
                            info_col = info_col.push(text("No room selected").font(MONO).size(14).color(theme::TEXT_DIM));
                        }
                        info_col
                    },
                ]
                .spacing(12)
                .width(450)
                .into()
            }

            crate::state::Modal::Invite => {
                let mut col = column![
                    row![
                        text("Invite User").font(MONO).size(16).color(theme::TEXT_BRIGHT),
                        Space::new().width(Fill),
                        button(text("\u{00d7}").font(MONO).size(18).color(theme::TEXT_DIM))
                            .on_press(Message::CloseModal)
                            .padding([2, 6])
                            .style(|_, _| button::Style { background: None, ..Default::default() }),
                    ].align_y(iced::Alignment::Center),
                    text("Search for a user to invite").font(MONO).size(13).color(theme::TEXT_DIM),
                    row![
                        text_input("@user:server or display name...", &self.invite_search)
                            .on_input(Message::InviteSearchChanged)
                            .on_submit(Message::InviteSearchSubmit)
                            .font(MONO).size(14).padding(8),
                        button(text("Search").font(MONO).size(14).color(theme::ACCENT))
                            .on_press(Message::InviteSearchSubmit)
                            .padding([8, 16])
                            .style(|_, _| button::Style {
                                background: Some(theme::BG_LIGHT.into()),
                                text_color: theme::ACCENT,
                                border: iced::Border { radius: 4.0.into(), ..Default::default() },
                                ..Default::default()
                            }),
                    ].spacing(8),
                ].spacing(10);

                if self.invite_results.is_empty() && !self.invite_search.is_empty() {
                    col = col.push(text("No users found").font(MONO).size(13).color(theme::TEXT_DIM));
                }

                for user in &self.invite_results {
                    let uid = user.user_id.clone();
                    let name = user.display_name.as_deref().unwrap_or(&user.user_id);
                    col = col.push(
                        button(
                            row![
                                text(name).font(MONO).size(14).color(theme::TEXT_NORMAL),
                                Space::new().width(Fill),
                                text(&user.user_id).font(MONO).size(12).color(theme::TEXT_DIM),
                            ].spacing(8).align_y(iced::Alignment::Center)
                        )
                        .on_press(Message::InviteUserFromModal(uid))
                        .padding([6, 10])
                        .width(Fill)
                        .style(|_, status| button::Style {
                            background: if matches!(status, button::Status::Hovered) {
                                Some(Color::from_rgba(1.0, 1.0, 1.0, 0.06).into())
                            } else { None },
                            border: iced::Border { radius: 4.0.into(), ..Default::default() },
                            ..Default::default()
                        }),
                    );
                }

                // Also allow direct invite by typing a user ID
                if self.invite_search.starts_with('@') && self.invite_search.contains(':') {
                    let uid = self.invite_search.clone();
                    col = col.push(rule::horizontal(1));
                    col = col.push(
                        button(
                            text(format!("Invite {}", &self.invite_search)).font(MONO).size(14).color(theme::GREEN)
                        )
                        .on_press(Message::InviteUserFromModal(uid))
                        .padding([6, 10])
                        .width(Fill)
                        .style(|_, _| button::Style {
                            background: Some(theme::BG_LIGHT.into()),
                            text_color: theme::GREEN,
                            border: iced::Border { radius: 4.0.into(), ..Default::default() },
                            ..Default::default()
                        }),
                    );
                }

                col.spacing(8).width(450).into()
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

        let sep = || text(" \u{2502} ").font(MONO).size(13).color(theme::TEXT_DIM);

        // Settings gear SVG
        let gear_handle = iced::widget::svg::Handle::from_memory(
            br#"<svg xmlns="http://www.w3.org/2000/svg" width="24" height="24" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><circle cx="12" cy="12" r="3"/><path d="M19.4 15a1.65 1.65 0 0 0 .33 1.82l.06.06a2 2 0 0 1-2.83 2.83l-.06-.06a1.65 1.65 0 0 0-1.82-.33 1.65 1.65 0 0 0-1 1.51V21a2 2 0 0 1-4 0v-.09A1.65 1.65 0 0 0 9 19.4a1.65 1.65 0 0 0-1.82.33l-.06.06a2 2 0 0 1-2.83-2.83l.06-.06A1.65 1.65 0 0 0 4.68 15a1.65 1.65 0 0 0-1.51-1H3a2 2 0 0 1 0-4h.09A1.65 1.65 0 0 0 4.6 9a1.65 1.65 0 0 0-.33-1.82l-.06-.06a2 2 0 0 1 2.83-2.83l.06.06A1.65 1.65 0 0 0 9 4.68a1.65 1.65 0 0 0 1-1.51V3a2 2 0 0 1 4 0v.09a1.65 1.65 0 0 0 1 1.51 1.65 1.65 0 0 0 1.82-.33l.06-.06a2 2 0 0 1 2.83 2.83l-.06.06A1.65 1.65 0 0 0 19.4 9a1.65 1.65 0 0 0 1.51 1H21a2 2 0 0 1 0 4h-.09a1.65 1.65 0 0 0-1.51 1z"/></svg>"#.as_slice(),
        );
        bar = bar.push(
            button(
                iced::widget::svg(gear_handle)
                    .width(14).height(14)
                    .style(|_, _| iced::widget::svg::Style { color: Some(theme::TEXT_DIM) }),
            )
            .on_press(Message::OpenModal(crate::state::Modal::Settings {
                tab: "account".into(),
            }))
            .padding([2, 6])
            .style(|_, _| button::Style { background: None, ..Default::default() }),
        );

        // User ID in green (like IRC)
        bar = bar.push(
            text(&self.user_id).font(MONO).size(13).color(theme::GREEN)
        );

        // Current room info
        if let Some(room_id) = &self.selected_room_id {
            let room = self.rooms.iter().find(|r| r.room_id == *room_id);
            let room_name = room.map(|r| r.name.as_str()).unwrap_or(room_id);

            bar = bar.push(sep());
            bar = bar.push(text(format!("#{room_name}")).font(MONO).size(13).color(theme::TEXT_NORMAL));

            // Power level badge
            if let Some(members) = self.members_by_room.get(room_id) {
                if let Some(me) = members.iter().find(|m| m.user_id == self.user_id) {
                    if me.power_level >= 100 {
                        bar = bar.push(text(" @").font(MONO).size(12).color(theme::YELLOW));
                    } else if me.power_level >= 50 {
                        bar = bar.push(text(" +").font(MONO).size(12).color(theme::ACCENT));
                    }
                }
            }

            // Nick count
            if let Some(members) = self.members_by_room.get(room_id) {
                bar = bar.push(sep());
                bar = bar.push(
                    text(format!("{} nicks", members.len()))
                        .font(MONO).size(13).color(theme::TEXT_MUTED),
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
                                .style(|_, _| iced::widget::svg::Style { color: Some(theme::GREEN) }),
                            text(" encrypted").font(MONO).size(13).color(theme::GREEN),
                        ].spacing(2).align_y(iced::Alignment::Center)
                    );
                } else {
                    bar = bar.push(text("unencrypted").font(MONO).size(13).color(theme::TEXT_MUTED));
                }
            }
        }

        bar = bar.push(Space::new().width(Fill));

        // Sync lag
        let lag_label = if self.sync_lag_ms == 0 { "synced".to_string() } else { format!("{}ms", self.sync_lag_ms) };
        let lag_color = if self.sync_lag_ms < 200 {
            theme::GREEN
        } else if self.sync_lag_ms < 1000 {
            theme::YELLOW
        } else {
            theme::RED
        };
        bar = bar.push(text(lag_label).font(MONO).size(13).color(lag_color));

        // Verification status
        if let Some(ref sec) = self.security_state {
            bar = bar.push(sep());
            if sec.is_verified {
                bar = bar.push(
                    text("\u{2713} verified").font(MONO).size(13).color(theme::GREEN)
                );
            } else {
                bar = bar.push(
                    text("\u{2717} unverified").font(MONO).size(13).color(theme::YELLOW)
                );
            }
        }

        // Device ID
        if let Some(ref did) = self.device_id {
            bar = bar.push(sep());
            let short = if did.len() > 10 { &did[..10] } else { did };
            bar = bar.push(text(short).font(MONO).size(13).color(theme::TEXT_DIM));
        }

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

        // File drag-drop + sidebar resize mouse tracking
        let file_drop = iced::event::listen_with(|event, _status, _window| {
            match event {
                iced::Event::Window(iced::window::Event::FileDropped(path)) => {
                    Some(Message::FileDropped(path))
                }
                iced::Event::Mouse(iced::mouse::Event::CursorMoved { position }) => {
                    Some(Message::SidebarDrag(position.x))
                }
                iced::Event::Mouse(iced::mouse::Event::ButtonReleased(iced::mouse::Button::Left)) => {
                    Some(Message::SidebarDragEnd)
                }
                _ => None,
            }
        });

        // Timeline updates from background diff watchers
        let timeline_sub = Subscription::run(timeline_update_stream);

        Subscription::batch([tick, keys, file_drop, timeline_sub])
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
