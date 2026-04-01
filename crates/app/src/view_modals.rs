use iced::widget::{
    button, column, container, row, rule, scrollable,
    text, text_input, Space,
};
use iced::{Color, Element, Fill, Length, Theme};

use crate::message::Message;
use crate::state::FluxChat;
use crate::theme;
use crate::view::{thin_scrollbar, MONO};

impl FluxChat {
    /// Render the active modal content.
    pub(crate) fn view_modal_content(&self) -> Element<Message> {
        let modal = match &self.modal {
            Some(m) => m,
            None => return text("").into(),
        };

        let modal_body: Element<Message> = match modal {
            crate::state::Modal::Settings { tab } => self.view_settings_modal(tab),
            crate::state::Modal::CreateRoom => self.view_create_room_modal(),
            crate::state::Modal::Profile { user_id } => self.view_profile_modal(user_id),
            crate::state::Modal::Verification { info } => self.view_verification_modal(info),
            crate::state::Modal::Lightbox { url } => self.view_lightbox_modal(url),
            crate::state::Modal::Forward { body, .. } => self.view_forward_modal(body),
            crate::state::Modal::RoomSettings => self.view_room_settings_modal(),
            crate::state::Modal::Invite => self.view_invite_modal(),
            crate::state::Modal::Report { .. } => {
                let col = column![
                    text("Report Message").font(MONO).size(16).color(theme::TEXT_BRIGHT),
                    rule::horizontal(1),
                    text("Why are you reporting this message?").font(MONO).size(13).color(theme::TEXT_DIM),
                    text_input("Reason...", &self.report_reason)
                        .on_input(Message::ReportReasonChanged)
                        .on_submit(Message::SubmitReport)
                        .font(MONO).size(14).padding(8),
                    row![
                        Space::new().width(Fill),
                        button(text("Cancel").font(MONO).size(13).color(theme::TEXT_DIM))
                            .on_press(Message::CloseModal)
                            .padding([6, 14])
                            .style(|_, status| button::Style {
                                background: if matches!(status, button::Status::Hovered) {
                                    Some(iced::Color::from_rgba(1.0, 1.0, 1.0, 0.06).into())
                                } else { None },
                                border: iced::Border { radius: 4.0.into(), ..Default::default() },
                                ..Default::default()
                            }),
                        button(text("Report").font(MONO).size(13).color(iced::Color::WHITE))
                            .on_press(Message::SubmitReport)
                            .padding([6, 14])
                            .style(|_, status| button::Style {
                                background: Some(if matches!(status, button::Status::Hovered) {
                                    iced::Color::from_rgba(0.9, 0.2, 0.2, 0.9)
                                } else {
                                    iced::Color::from_rgba(0.9, 0.2, 0.2, 0.7)
                                }.into()),
                                border: iced::Border { radius: 4.0.into(), ..Default::default() },
                                ..Default::default()
                            }),
                    ].spacing(8),
                ].spacing(12).width(350);
                col.into()
            }
        };

        let pad = if matches!(modal, crate::state::Modal::Profile { .. }) { 0 } else { 24 };
        container(modal_body)
            .padding(pad)
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

    fn view_settings_modal(&self, tab: &str) -> Element<Message> {
        let tab_owned = tab.to_owned();

        let make_tab_btn = |label: String, tab_id: String, current: &str| -> Element<Message> {
            let is_active = current == tab_id;
            let bg = if is_active {
                theme::BG_LIGHT
            } else {
                iced::Color::TRANSPARENT
            };
            let color = if is_active {
                theme::TEXT_BRIGHT
            } else {
                theme::TEXT_MUTED
            };
            button(text(label).font(MONO).size(14).color(color))
                .on_press(Message::OpenModal(crate::state::Modal::Settings {
                    tab: tab_id,
                }))
                .padding([6, 10])
                .width(Fill)
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

        let sidebar = column![
            make_tab_btn("Account".into(), "account".into(), &tab_owned),
            make_tab_btn("Appearance".into(), "appearance".into(), &tab_owned),
            make_tab_btn("Notifications".into(), "notifications".into(), &tab_owned),
            make_tab_btn("Sessions".into(), "sessions".into(), &tab_owned),
            make_tab_btn("Security".into(), "security".into(), &tab_owned),
        ]
        .spacing(4)
        .width(120);

        let tab_content: Element<Message> = match tab_owned.as_str() {
            "account" => {
                let display = self.display_name.as_deref().unwrap_or(&self.user_id);

                // Avatar section (use cached handle to avoid flicker)
                let avatar_element: Element<Message> = if let Some(ref handle) = self.settings_avatar_handle {
                    container(
                        iced::widget::image(handle.clone())
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

                // Banner at top (use cached handle to avoid flicker)
                const BANNER_HEIGHT: f32 = 150.0;
                let banner_element: Element<Message> = if let Some(ref handle) = self.settings_banner_handle {
                    container(
                        iced::widget::image(handle.clone())
                            .width(Fill)
                            .height(BANNER_HEIGHT)
                            .content_fit(iced::ContentFit::Cover),
                    )
                    .width(Fill)
                    .height(BANNER_HEIGHT)
                    .clip(true)
                    .style(|_: &Theme| container::Style {
                        border: iced::Border { radius: 6.0.into(), ..Default::default() },
                        ..Default::default()
                    })
                    .into()
                } else {
                    container(
                        text("no banner").font(MONO).size(13).color(theme::TEXT_DIM),
                    )
                    .width(Fill)
                    .height(BANNER_HEIGHT)
                    .center(Fill)
                    .style(|_: &Theme| container::Style {
                        background: Some(theme::BG_LIGHT.into()),
                        border: iced::Border { radius: 6.0.into(), ..Default::default() },
                        ..Default::default()
                    })
                    .into()
                };

                // Banner actions + position slider
                let mut banner_actions = row![
                    button(text("change").font(MONO).size(12).color(theme::ACCENT))
                        .on_press(Message::SettingsPickBanner)
                        .padding([2, 0])
                        .style(|_, _| button::Style { background: None, text_color: theme::ACCENT, ..Default::default() }),
                    button(text("remove").font(MONO).size(12).color(theme::TEXT_DIM))
                        .on_press(Message::SettingsRemoveBanner)
                        .padding([2, 0])
                        .style(|_, _| button::Style { background: None, text_color: theme::TEXT_DIM, ..Default::default() }),
                ].spacing(8).align_y(iced::Alignment::Center);

                // Show position slider + save if banner needs cropping
                if self.settings_banner_handle.is_some() {
                    let (img_w, img_h) = self.settings_banner_dimensions.unwrap_or((800, 200));
                    let target_h = (img_w as f32 / 4.0) as u32;
                    if img_h > target_h {
                        banner_actions = banner_actions.push(Space::new().width(Fill));
                        banner_actions = banner_actions.push(
                            text("crop").font(MONO).size(11).color(theme::TEXT_DIM),
                        );
                        banner_actions = banner_actions.push(
                            iced::widget::slider(0.0..=1.0, self.settings_banner_offset, Message::SettingsBannerOffsetChanged)
                                .step(0.01)
                                .width(80),
                        );
                        banner_actions = banner_actions.push(
                            button(text("save").font(MONO).size(12).color(theme::GREEN))
                                .on_press(Message::SettingsSaveBanner)
                                .padding([2, 6])
                                .style(|_, _| button::Style { background: None, text_color: theme::GREEN, ..Default::default() }),
                        );
                    } else {
                        // Already good aspect ratio, save directly
                        banner_actions = banner_actions.push(Space::new().width(Fill));
                        banner_actions = banner_actions.push(
                            button(text("save").font(MONO).size(12).color(theme::GREEN))
                                .on_press(Message::SettingsSaveBanner)
                                .padding([2, 6])
                                .style(|_, _| button::Style { background: None, text_color: theme::GREEN, ..Default::default() }),
                        );
                    }
                }

                let mut col = column![
                    // Banner first, like other clients
                    banner_element,
                    banner_actions,
                    Space::new().height(2),
                    // Avatar + profile info
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
                ].spacing(10);

                if let Some(ref status) = self.settings_status {
                    col = col.push(text(status.as_str()).font(MONO).size(13).color(theme::GREEN));
                }

                // Password change section
                col = col.push(Space::new().height(4));
                col = col.push(rule::horizontal(1));
                col = col.push(text("PASSWORD").font(MONO).size(12).color(theme::TEXT_DIM));
                col = col.push(
                    text_input("Current password", &self.settings_old_password)
                        .on_input(Message::SettingsOldPasswordChanged)
                        .font(MONO).size(13).padding(6).secure(true),
                );
                col = col.push(
                    text_input("New password", &self.settings_new_password)
                        .on_input(Message::SettingsNewPasswordChanged)
                        .font(MONO).size(13).padding(6).secure(true),
                );
                col = col.push(
                    row![
                        text_input("Confirm new password", &self.settings_confirm_password)
                            .on_input(Message::SettingsConfirmPasswordChanged)
                            .on_submit(Message::SettingsChangePassword)
                            .font(MONO).size(13).padding(6).secure(true),
                        button(text("Change").font(MONO).size(13).color(theme::TEXT_BRIGHT))
                            .on_press(Message::SettingsChangePassword)
                            .padding([6, 12])
                            .style(|_, _| button::Style {
                                background: Some(theme::ACCENT.into()),
                                text_color: theme::TEXT_BRIGHT,
                                border: iced::Border { radius: 4.0.into(), ..Default::default() },
                                ..Default::default()
                            }),
                    ].spacing(6).align_y(iced::Alignment::Center),
                );

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
                scrollable(container(col).padding([0, 12]).width(Fill)).height(Fill).direction(iced::widget::scrollable::Direction::Vertical(iced::widget::scrollable::Scrollbar::new().width(3).scroller_width(3))).style(thin_scrollbar).into()
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
                scrollable(container(col).padding([0, 12]).width(Fill)).height(Fill).direction(iced::widget::scrollable::Direction::Vertical(iced::widget::scrollable::Scrollbar::new().width(3).scroller_width(3))).style(thin_scrollbar).into()
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

                scrollable(container(col).padding([0, 12]).width(Fill)).height(Fill).direction(iced::widget::scrollable::Direction::Vertical(iced::widget::scrollable::Scrollbar::new().width(3).scroller_width(3))).style(thin_scrollbar).into()
            }
            "appearance" => {
                let fs = self.msg_font_size;
                let col = column![
                    text("Appearance").font(MONO).size(16).color(theme::TEXT_BRIGHT),
                    rule::horizontal(1),
                    text("MESSAGE FONT SIZE").font(MONO).size(11).color(theme::TEXT_DIM),
                    row![
                        iced::widget::slider(10.0..=24.0, fs, Message::SetMsgFontSize)
                            .step(1.0)
                            .width(200),
                        text(format!("{:.0}px", fs)).font(MONO).size(13).color(theme::TEXT_NORMAL),
                    ].spacing(12).align_y(iced::Alignment::Center),
                    Space::new().height(8),
                    // Preview
                    text("PREVIEW").font(MONO).size(11).color(theme::TEXT_DIM),
                    container(
                        column![
                            text("The quick brown fox jumps over the lazy dog.")
                                .font(MONO).size(fs).color(theme::TEXT_NORMAL),
                            text("**bold** *italic* `code` ~~strike~~")
                                .font(MONO).size(fs).color(theme::TEXT_MUTED),
                        ].spacing(4)
                    )
                    .padding([8, 12])
                    .width(Fill)
                    .style(|_: &Theme| container::Style {
                        background: Some(theme::BG_MID.into()),
                        border: iced::Border { radius: 4.0.into(), ..Default::default() },
                        ..Default::default()
                    }),
                ].spacing(10);
                scrollable(container(col).padding([0, 12]).width(Fill)).height(Fill).direction(iced::widget::scrollable::Direction::Vertical(iced::widget::scrollable::Scrollbar::new().width(3).scroller_width(3))).style(thin_scrollbar).into()
            }
            "notifications" => {
                let make_toggle = |label: &'static str, enabled: bool, msg: fn(bool) -> Message| -> Element<Message> {
                    row![
                        text(label).font(MONO).size(14).color(theme::TEXT_NORMAL),
                        Space::new().width(Fill),
                        iced::widget::toggler(enabled)
                            .on_toggle(msg)
                            .size(18)
                            .style(|_: &Theme, status| {
                                use iced::widget::toggler;
                                let is_on = match status {
                                    toggler::Status::Active { is_toggled } | toggler::Status::Hovered { is_toggled } | toggler::Status::Disabled { is_toggled } => is_toggled,
                                };
                                toggler::Style {
                                    background: if is_on { theme::ACCENT.into() } else { theme::BG_LIGHT.into() },
                                    foreground: iced::Color::WHITE.into(),
                                    background_border_width: 1.0,
                                    background_border_color: if is_on { theme::ACCENT } else { theme::BORDER },
                                    foreground_border_width: 0.0,
                                    foreground_border_color: iced::Color::TRANSPARENT,
                                    text_color: None,
                                    border_radius: None,
                                    padding_ratio: 0.2,
                                }
                            }),
                    ]
                    .spacing(12)
                    .align_y(iced::Alignment::Center)
                    .padding([8, 0])
                    .into()
                };

                let col = column![
                    text("Notifications").font(MONO).size(16).color(theme::TEXT_BRIGHT),
                    rule::horizontal(1),
                    make_toggle("Enable notifications", self.notifications_enabled, Message::SetNotificationsEnabled),
                    make_toggle("Mentions only", self.notify_mentions_only, Message::SetNotifyMentionsOnly),
                    make_toggle("Show message preview", self.notify_show_preview, Message::SetNotifyShowPreview),
                    make_toggle("Notification sound", self.notify_sound, Message::SetNotifySound),
                    Space::new().height(12),
                    text("Per-room mute: use /mute and /unmute commands").font(MONO).size(12).color(theme::TEXT_DIM),
                ].spacing(6);
                scrollable(container(col).padding([0, 12]).width(Fill)).height(Fill).direction(iced::widget::scrollable::Direction::Vertical(iced::widget::scrollable::Scrollbar::new().width(3).scroller_width(3))).style(thin_scrollbar).into()
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
            rule::horizontal(1),
            row![
                sidebar,
                rule::vertical(1),
                container(tab_content).width(Fill),
            ]
            .spacing(8)
            .height(Fill),
        ]
        .spacing(12)
        .width(700)
        .height(500)
        .into()
    }

    fn view_create_room_modal(&self) -> Element<Message> {
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
                    .style(theme::button_transparent),
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
                    .style(theme::button_transparent),
                button(text(encrypted_label).font(MONO).size(14).color(theme::TEXT_MUTED))
                    .on_press(Message::CreateRoomToggleEncrypted)
                    .padding([4, 8])
                    .style(theme::button_transparent),
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

    fn view_profile_modal<'a>(&'a self, user_id: &'a str) -> Element<'a, Message> {
        // Look up member info from the current room
        let member = self
            .selected_room_id
            .as_ref()
            .and_then(|rid| self.members_by_room.get(rid))
            .and_then(|members| members.iter().find(|m| m.user_id == *user_id));

        let display_name = member
            .map(|m| m.display_name.as_str())
            .unwrap_or(user_id);
        let nick_col = theme::nick_color(display_name);
        let power = member.map(|m| m.power_level).unwrap_or(0);
        let role = member.map(|m| m.role.as_str()).unwrap_or("Member");
        let is_ignored = self.ignored_users.contains(user_id);
        let uid = user_id.to_owned();

        // My power level in this room
        let my_power = self.selected_room_id.as_ref()
            .and_then(|rid| self.members_by_room.get(rid))
            .and_then(|members| members.iter().find(|m| m.user_id == self.user_id))
            .map(|m| m.power_level)
            .unwrap_or(0);
        let can_set_power = my_power > power && user_id != self.user_id;

        // Role color + badge
        let role_color = match power {
            100.. => theme::YELLOW,
            50..=99 => theme::ACCENT,
            _ => theme::TEXT_NORMAL,
        };

        // --- Colored banner strip ---
        let banner_color = iced::Color { a: 0.5, ..nick_col };
        let banner = container(Space::new())
            .width(Fill)
            .height(60)
            .style(move |_: &Theme| container::Style {
                background: Some(banner_color.into()),
                ..Default::default()
            });

        // --- Avatar + name row ---
        let avatar_el: Element<Message> = self.render_avatar_with_initial(
            member.and_then(|m| m.avatar_url.as_deref()),
            display_name,
            48.0,
        );

        // --- Header: avatar + name + close ---
        let header = row![
            avatar_el,
            column![
                text(display_name).font(MONO).size(16).color(nick_col),
                text(user_id).font(MONO).size(11).color(theme::TEXT_MUTED),
            ].spacing(2),
            Space::new().width(Fill),
            button(text("x").font(MONO).size(14).color(theme::TEXT_DIM))
                .on_press(Message::CloseModal)
                .padding([2, 6])
                .style(|_, _| button::Style {
                    background: None,
                    ..Default::default()
                }),
        ]
        .spacing(10)
        .padding([12, 16])
        .align_y(iced::Alignment::Center);

        // --- About section ---
        let role_badge: Element<Message> = container(
            text(role).font(MONO).size(11).color(role_color),
        )
        .padding([2, 8])
        .style(move |_: &Theme| container::Style {
            background: Some(iced::Color { a: 0.15, ..role_color }.into()),
            border: iced::Border { radius: 10.0.into(), ..Default::default() },
            ..Default::default()
        })
        .into();

        let about_section = container(
            column![
                text("ABOUT").font(MONO).size(10).color(theme::TEXT_DIM),
                Space::new().height(4),
                row![
                    role_badge,
                    Space::new().width(Fill),
                    text(format!("Power {power}")).font(MONO).size(11).color(theme::TEXT_DIM),
                ].align_y(iced::Alignment::Center),
            ].spacing(0),
        )
        .padding([10, 16])
        .width(Fill)
        .style(|_: &Theme| container::Style {
            background: Some(theme::BG_MID.into()),
            border: iced::Border { radius: 6.0.into(), ..Default::default() },
            ..Default::default()
        });

        // --- Roles section (only if can edit) ---
        let roles_section: Option<Element<Message>> = if can_set_power {
            let room_id = self.selected_room_id.clone().unwrap_or_default();
            let mut role_col = column![].spacing(4);
            for (label, desc, level, color) in [
                ("Member", "Default permissions", 0i64, theme::TEXT_NORMAL),
                ("Moderator", "Kick, ban, mute", 50, theme::ACCENT),
                ("Admin", "Full room control", 100, theme::YELLOW),
            ] {
                let is_active = power == level;
                let bg = if is_active { theme::BG_LIGHT } else { theme::BG_DEEP };
                let fg = if is_active { color } else { theme::TEXT_MUTED };
                let border_c = if is_active { color } else { theme::BG_LIGHT };
                role_col = role_col.push(
                    button(
                        row![
                            text(label).font(MONO).size(12).color(fg),
                            Space::new().width(Fill),
                            text(desc).font(MONO).size(10).color(theme::TEXT_DIM),
                        ].align_y(iced::Alignment::Center),
                    )
                    .on_press(Message::SetPowerLevel(room_id.clone(), uid.clone(), level))
                    .padding([6, 12])
                    .width(Fill)
                    .style(move |_, _| button::Style {
                        background: Some(bg.into()),
                        text_color: fg,
                        border: iced::Border {
                            color: border_c,
                            width: 1.0,
                            radius: 4.0.into(),
                        },
                        ..Default::default()
                    }),
                );
            }

            // Custom level inline
            role_col = role_col.push(
                button(
                    row![
                        text("Custom").font(MONO).size(12).color(theme::TEXT_MUTED),
                        Space::new().width(Fill),
                        text_input("0-100", &self.profile_power_input)
                            .on_input(Message::ProfilePowerInputChanged)
                            .on_submit(Message::ProfileSetPowerLevel)
                            .font(MONO).size(11).padding(3).width(60),
                        button(text("Set").font(MONO).size(11).color(theme::TEXT_BRIGHT))
                            .on_press(Message::ProfileSetPowerLevel)
                            .padding([3, 8])
                            .style(|_, _| button::Style {
                                background: Some(theme::ACCENT.into()),
                                text_color: theme::TEXT_BRIGHT,
                                border: iced::Border { radius: 4.0.into(), ..Default::default() },
                                ..Default::default()
                            }),
                    ].spacing(6).align_y(iced::Alignment::Center),
                )
                .padding([4, 12])
                .width(Fill)
                .style(|_, _| button::Style {
                    background: Some(theme::BG_DEEP.into()),
                    border: iced::Border {
                        color: theme::BG_LIGHT,
                        width: 1.0,
                        radius: 4.0.into(),
                    },
                    ..Default::default()
                }),
            );

            Some(container(
                column![
                    text("SET ROLE").font(MONO).size(10).color(theme::TEXT_DIM),
                    Space::new().height(6),
                    role_col,
                ].spacing(0),
            )
            .padding([10, 16])
            .width(Fill)
            .style(|_: &Theme| container::Style {
                background: Some(theme::BG_MID.into()),
                border: iced::Border { radius: 6.0.into(), ..Default::default() },
                ..Default::default()
            })
            .into())
        } else {
            None
        };

        // --- Action buttons ---
        let ignore_label = if is_ignored { "Unignore" } else { "Ignore" };
        let ignore_color = if is_ignored { theme::GREEN } else { theme::RED };
        let mut actions_row = row![].spacing(6);
        if uid != self.user_id {
            actions_row = actions_row.push(
                button(text("Message").font(MONO).size(12).color(theme::ACCENT))
                    .on_press(Message::SendDmToUser(uid.clone()))
                    .padding([5, 14])
                    .width(Fill)
                    .style(|_, _| button::Style {
                        background: Some(theme::BG_LIGHT.into()),
                        text_color: theme::ACCENT,
                        border: iced::Border { radius: 4.0.into(), ..Default::default() },
                        ..Default::default()
                    }),
            );
        }
        actions_row = actions_row.push(
            button(text(ignore_label).font(MONO).size(12).color(ignore_color))
                .on_press(Message::ToggleIgnoreUser(uid))
                .padding([5, 14])
                .width(Fill)
                .style(move |_, _| button::Style {
                    background: Some(theme::BG_LIGHT.into()),
                    text_color: ignore_color,
                    border: iced::Border { radius: 4.0.into(), ..Default::default() },
                    ..Default::default()
                }),
        );

        // --- Assemble ---
        let mut card = column![
            banner,
            header,
            container(rule::horizontal(1)).padding([0, 16]),
            container(about_section).padding([6, 12]),
        ].spacing(0).width(340);

        if let Some(roles) = roles_section {
            card = card.push(container(roles).padding([6, 12]));
        }

        card = card.push(container(rule::horizontal(1)).padding([2, 16]));
        card = card.push(container(actions_row).padding([8, 12]));

        if is_ignored {
            card = card.push(
                container(text("(ignored)").font(MONO).size(11).color(theme::RED)).padding([0, 16]),
            );
        }

        card = card.push(Space::new().height(6));
        card.into()
    }

    fn view_verification_modal<'a>(&'a self, info: &'a fluxchat_core::VerificationInfo) -> Element<'a, Message> {
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

    fn view_lightbox_modal(&self, url: &str) -> Element<Message> {
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

    fn view_forward_modal(&self, body: &str) -> Element<Message> {
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

    fn view_room_settings_modal(&self) -> Element<Message> {
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
                    if room.is_encrypted {
                        info_col = info_col.push(
                            row![
                                text("Encrypted:").font(MONO).size(13).color(theme::TEXT_DIM),
                                text("Yes").font(MONO).size(14).color(theme::GREEN),
                            ].spacing(8)
                        );
                    } else {
                        let rid = room.room_id.clone();
                        info_col = info_col.push(
                            row![
                                text("Encrypted:").font(MONO).size(13).color(theme::TEXT_DIM),
                                text("No").font(MONO).size(14).color(theme::TEXT_MUTED),
                                Space::new().width(Fill),
                                button(text("Enable Encryption").font(MONO).size(12).color(theme::ACCENT))
                                    .on_press(Message::EnableEncryption(rid))
                                    .padding([4, 10])
                                    .style(|_, _| button::Style {
                                        background: Some(theme::BG_LIGHT.into()),
                                        text_color: theme::ACCENT,
                                        border: iced::Border { radius: 4.0.into(), ..Default::default() },
                                        ..Default::default()
                                    }),
                            ].spacing(8).align_y(iced::Alignment::Center)
                        );
                    }
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

    fn view_invite_modal(&self) -> Element<Message> {
        let mut col = column![
            row![
                text("Invite User").font(MONO).size(16).color(theme::TEXT_BRIGHT),
                Space::new().width(Fill),
                button(text("\u{00d7}").font(MONO).size(18).color(theme::TEXT_DIM))
                    .on_press(Message::CloseModal)
                    .padding([2, 6])
                    .style(theme::button_transparent),
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

    /// Single device/session row for the sessions settings tab.
    pub(crate) fn view_device_row<'a>(&self, device: &'a fluxchat_core::devices::DeviceInfo) -> Element<'a, Message> {
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
}
