use iced::widget::{
    button, column, container, row, scrollable,
    text, Space,
};
use iced::{Color, Element, Fill, Length, Theme};

use crate::message::Message;
use crate::state::FluxChat;
use crate::theme;
use crate::view::{thin_scrollbar, MONO};
use crate::ui::{self, Tone};

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
            crate::state::Modal::Forward { body, sender_name, .. } => self.view_forward_modal(body, sender_name.as_deref()),
            crate::state::Modal::RoomSettings => self.view_room_settings_modal(),
            crate::state::Modal::Invite => self.view_invite_modal(),
            crate::state::Modal::CreateDm => self.view_create_dm_modal(),
            crate::state::Modal::SpaceDetails { space_id } => self.view_space_details_modal(space_id),
            crate::state::Modal::PendingInvites => self.view_pending_invites_modal(),
            crate::state::Modal::DevTools => self.view_devtools_modal(),
            crate::state::Modal::Directory => self.view_directory_modal(),
            crate::state::Modal::EventSource { event_id, json } => {
                column![
                    ui::header("Event source", Some(event_id.clone())),
                    container(
                        scrollable(
                            container(
                                text(json.as_str()).font(MONO).size(12).color(theme::colors().code_text)
                            ).padding(12).width(Fill)
                        )
                        .direction(iced::widget::scrollable::Direction::Both {
                            vertical: iced::widget::scrollable::Scrollbar::new().width(4).scroller_width(4),
                            horizontal: iced::widget::scrollable::Scrollbar::new().width(4).scroller_width(4),
                        })
                        .style(thin_scrollbar)
                    )
                    .height(Length::Fixed(420.0))
                    .style(|_: &Theme| container::Style {
                        background: Some(theme::colors().code_bg.into()),
                        border: iced::Border { color: theme::colors().border, width: 1.0, radius: 8.0.into() },
                        ..Default::default()
                    }),
                    ui::footer(vec![
                        ui::icon_btn(crate::icons::SVG_COPY, "Copy JSON", Tone::Secondary,
                            Some(Message::CopyMessageText(json.clone()))).into(),
                    ]),
                ].spacing(ui::GAP).width(Length::Fixed(680.0)).into()
            }
            crate::state::Modal::ConfirmLeave { room_id, name } => {
                let is_space = self.spaces.iter().any(|s| s.room_id == *room_id);
                let detail = if is_space {
                    "You'll leave the space but stay in its rooms. You'll need an invite or a public link to rejoin."
                } else {
                    "You'll stop receiving messages and need an invite or a public link to rejoin."
                };
                column![
                    ui::header(format!("Leave {name}?"), None),
                    ui::hint(detail),
                    ui::footer(vec![
                        ui::btn("Cancel", Tone::Ghost, Some(Message::CloseModal)).into(),
                        ui::btn(if is_space { "Leave space" } else { "Leave room" }, Tone::Danger,
                            Some(Message::LeaveRoom(room_id.clone()))).into(),
                    ]),
                ].spacing(ui::GAP).width(Length::Fixed(400.0)).into()
            }
            crate::state::Modal::ConfirmJoin { target, via } => {
                column![
                    ui::header(format!("Join {target}?"), None),
                    ui::hint("This link points to a room you're not in. Joining lets its members see you."),
                    ui::footer(vec![
                        ui::btn("Cancel", Tone::Ghost, Some(Message::CloseModal)).into(),
                        ui::btn("Join room", Tone::Primary,
                            Some(Message::Batch(vec![Message::CloseModal, Message::JoinRoomVia(target.clone(), via.clone())]))).into(),
                    ]),
                ].spacing(ui::GAP).width(Length::Fixed(400.0)).into()
            }
            crate::state::Modal::Report { .. } => {
                column![
                    ui::header("Report message", Some("Reports go to your homeserver's administrators.".into())),
                    ui::field("Reason", ui::input("What's wrong with this message?", &self.report_reason, Message::ReportReasonChanged)
                        .on_submit(Message::SubmitReport)),
                    ui::footer(vec![
                        ui::btn("Cancel", Tone::Ghost, Some(Message::CloseModal)).into(),
                        ui::btn("Report", Tone::Danger, Some(Message::SubmitReport)).into(),
                    ]),
                ].spacing(ui::GAP).width(Length::Fixed(420.0)).into()
            }
        };

        // The profile card (banner) and settings (sidebar) draw edge to edge;
        // everything else gets the standard dialog padding.
        let pad = if matches!(modal, crate::state::Modal::Profile { .. } | crate::state::Modal::Settings { .. }) {
            0
        } else {
            crate::ui::DIALOG_PAD
        };
        container(modal_body)
            .padding(pad)
            .clip(true)
            .style(crate::ui::dialog_style)
            .into()
    }

    fn view_settings_modal(&self, tab: &str) -> Element<Message> {
        const TABS: [(&str, &str, &[u8]); 7] = [
            ("account", "Account", crate::icons::SVG_PERSON),
            ("preferences", "Preferences", crate::icons::SVG_SLIDERS),
            ("appearance", "Appearance", crate::icons::SVG_PALETTE),
            ("notifications", "Notifications", crate::icons::SVG_BELL),
            ("voice", "Voice & Video", crate::icons::SVG_HEADPHONES),
            ("sessions", "Sessions", crate::icons::SVG_MONITOR),
            ("security", "Security", crate::icons::SVG_SHIELD),
        ];

        // --- Sidebar navigation ---
        let mut nav = column![
            container(text("Settings").font(MONO).size(15).color(theme::colors().text_bright))
                .padding(iced::Padding { top: 2.0, right: 8.0, bottom: 10.0, left: 8.0 }),
        ]
        .spacing(2);
        for (id, label, icon_svg) in TABS {
            let active = tab == id;
            let fg = if active { theme::colors().text_bright } else { theme::colors().text_muted };
            nav = nav.push(
                button(
                    row![
                        ui::icon(icon_svg, 15.0, if active { theme::colors().accent } else { theme::colors().text_dim }),
                        text(label).font(MONO).size(13).color(fg),
                    ]
                    .spacing(10)
                    .align_y(iced::Alignment::Center),
                )
                .on_press(Message::OpenModal(crate::state::Modal::Settings { tab: id.into() }))
                .padding([7, 10])
                .width(Fill)
                .style(ui::row_style(active)),
            );
        }
        nav = nav.push(Space::new().height(Fill));
        nav = nav.push(
            ui::icon_btn(crate::icons::SVG_LOG_OUT, "Log out", Tone::Danger, Some(Message::Logout)).width(Fill),
        );
        let sidebar = container(nav.height(Fill))
            .width(Length::Fixed(176.0))
            .height(Fill)
            .padding(12)
            .style(|_: &Theme| container::Style {
                background: Some(theme::colors().bg_deep.into()),
                border: iced::Border {
                    radius: iced::border::Radius { top_left: ui::DIALOG_RADIUS, bottom_left: ui::DIALOG_RADIUS, ..Default::default() },
                    ..Default::default()
                },
                ..Default::default()
            });

        let (title, subtitle) = match tab {
            "account" => ("Account", "Your profile and sign-in"),
            "preferences" => ("Preferences", "How the timeline, composer and privacy behave"),
            "voice" => ("Voice & Video", "Devices and audio processing for calls"),
            "appearance" => ("Appearance", "Theme, fonts and text size"),
            "notifications" => ("Notifications", "Desktop alerts for new messages"),
            "sessions" => ("Sessions", "Devices signed in to your account"),
            "security" => ("Security", "Encryption keys and backup"),
            _ => ("Settings", ""),
        };

        let body: Element<Message> = match tab {
            "account" => self.view_settings_account(),
            "preferences" => self.view_settings_preferences(),
            "voice" => self.view_settings_voice(),
            "sessions" => self.view_settings_sessions(),
            "security" => self.view_settings_security(),
            "appearance" => self.view_settings_appearance(),
            "notifications" => self.view_settings_notifications(),
            _ => ui::empty_state(crate::icons::SVG_SETTINGS, "Unknown tab", None),
        };

        let content = column![
            ui::header(title, Some(subtitle.into())),
            scrollable(container(body).padding(iced::Padding { right: 12.0, bottom: 4.0, ..Default::default() }))
                .id(crate::view::dialog_scroll_id())
                .height(Fill)
                .direction(iced::widget::scrollable::Direction::Vertical(
                    iced::widget::scrollable::Scrollbar::new().width(4).scroller_width(4),
                ))
                .style(thin_scrollbar),
        ]
        .spacing(ui::GAP)
        .padding(iced::Padding { top: 18.0, right: 14.0, bottom: 18.0, left: 22.0 });

        // Drawn edge to edge (the frame adds no padding for Settings).
        container(row![sidebar, content.width(Fill)].height(Fill))
            .width(Length::Fixed(800.0))
            .height(Length::Fixed(560.0))
            .into()
    }

    fn view_settings_account(&self) -> Element<Message> {
        let display = self.display_name.as_deref().unwrap_or(&self.user_id);

        // Banner (cached handle avoids flicker)
        const BANNER_HEIGHT: f32 = 140.0;
        let banner_img: Element<Message> = if let Some(ref handle) = self.settings_banner_handle {
            iced::widget::image(handle.clone())
                .width(Fill)
                .height(BANNER_HEIGHT)
                .content_fit(iced::ContentFit::Cover)
                .into()
        } else {
            container(ui::hint("No banner"))
                .width(Fill)
                .height(BANNER_HEIGHT)
                .center(Fill)
                .style(|_: &Theme| container::Style {
                    background: Some(theme::colors().bg_mid.into()),
                    ..Default::default()
                })
                .into()
        };
        let banner = container(banner_img)
            .width(Fill)
            .height(BANNER_HEIGHT)
            .clip(true)
            .style(|_: &Theme| container::Style {
                border: iced::Border { color: theme::colors().border, width: 1.0, radius: 8.0.into() },
                ..Default::default()
            });

        let mut banner_actions = row![
            ui::icon_btn(crate::icons::SVG_IMAGE, "Change banner", Tone::Secondary, Some(Message::SettingsPickBanner)),
            ui::btn("Remove", Tone::Ghost, Some(Message::SettingsRemoveBanner)),
            Space::new().width(Fill),
        ]
        .spacing(8)
        .align_y(iced::Alignment::Center);
        if self.settings_banner_handle.is_some() {
            let (img_w, img_h) = self.settings_banner_dimensions.unwrap_or((800, 200));
            if img_h > img_w / 4 {
                banner_actions = banner_actions
                    .push(text("Position").font(MONO).size(11).color(theme::colors().text_dim))
                    .push(
                        iced::widget::slider(0.0..=1.0, self.settings_banner_offset, Message::SettingsBannerOffsetChanged)
                            .step(0.01_f32)
                            .width(90),
                    );
            }
            banner_actions = banner_actions.push(ui::btn("Save banner", Tone::Primary, Some(Message::SettingsSaveBanner)));
        }

        // Avatar + identity
        let avatar: Element<Message> = if let Some(ref handle) = self.settings_avatar_handle {
            container(iced::widget::image(handle.clone()).width(72).height(72).content_fit(iced::ContentFit::Cover))
                .width(72).height(72).clip(true)
                .style(|_: &Theme| container::Style {
                    border: iced::Border { radius: 36.0.into(), ..Default::default() },
                    ..Default::default()
                })
                .into()
        } else {
            self.render_avatar_with_initial(None, display, 72.0)
        };
        let identity = row![
            avatar,
            column![
                text(display).font(MONO).size(17).color(theme::colors().text_bright),
                row![
                    text(&self.user_id).font(MONO).size(12).color(theme::colors().text_dim),
                    button(ui::icon(crate::icons::SVG_COPY, 12.0, theme::colors().text_dim))
                        .on_press(Message::CopyMessageText(self.user_id.clone()))
                        .padding(3)
                        .style(ui::button_style(Tone::Ghost)),
                ].spacing(4).align_y(iced::Alignment::Center),
                row![
                    ui::btn("Change avatar", Tone::Secondary, Some(Message::SettingsUploadAvatar)),
                    ui::btn("Remove", Tone::Ghost, Some(Message::SettingsRemoveAvatar)),
                ].spacing(8),
            ]
            .spacing(6),
        ]
        .spacing(16)
        .align_y(iced::Alignment::Center);

        let name_row = row![
            ui::input("Display name", &self.settings_nick_input, Message::SettingsNickChanged)
                .on_submit(Message::SettingsSaveNick)
                .width(Fill),
            ui::btn("Save", Tone::Primary,
                (!self.settings_nick_input.trim().is_empty()
                    && Some(self.settings_nick_input.trim()) != self.display_name.as_deref())
                    .then_some(Message::SettingsSaveNick)),
        ]
        .spacing(8)
        .align_y(iced::Alignment::Center);

        let can_change_pw = !self.settings_old_password.is_empty()
            && !self.settings_new_password.is_empty()
            && self.settings_new_password == self.settings_confirm_password;
        let mismatch = !self.settings_confirm_password.is_empty()
            && self.settings_new_password != self.settings_confirm_password;
        let mut password = column![
            ui::input("Current password", &self.settings_old_password, Message::SettingsOldPasswordChanged).secure(true),
            ui::input("New password", &self.settings_new_password, Message::SettingsNewPasswordChanged).secure(true),
            ui::input("Confirm new password", &self.settings_confirm_password, Message::SettingsConfirmPasswordChanged)
                .on_submit(Message::SettingsChangePassword)
                .secure(true),
        ]
        .spacing(8);
        if mismatch {
            password = password.push(text("Passwords don't match").font(MONO).size(12).color(theme::colors().red));
        }
        password = password.push(ui::footer(vec![
            ui::btn("Change password", Tone::Secondary, can_change_pw.then_some(Message::SettingsChangePassword)).into(),
        ]));

        let mut col = column![
            column![banner, banner_actions].spacing(8),
            identity,
            ui::field("Display name", name_row),
        ]
        .spacing(ui::GAP + 4.0);
        if let Some(ref status) = self.settings_status {
            col = col.push(ui::status_line(status));
        }
        col = col.push(ui::divider());
        col = col.push(ui::field("Password", password));

        // Deactivation: armed first, then confirmed with the password.
        col = col.push(ui::divider());
        let mut deactivate = column![
            row![
                column![
                    text("Deactivate account").font(MONO).size(13).color(theme::colors().text_normal),
                    ui::hint("Permanently disables this account: you're removed from every room and can't log in again."),
                ].spacing(2).width(Fill),
                ui::btn("Deactivate\u{2026}", Tone::Danger,
                    (!self.deactivate_armed).then_some(Message::SettingsDeactivateArm(true))),
            ]
            .spacing(12)
            .align_y(iced::Alignment::Center),
        ]
        .spacing(10);
        if self.deactivate_armed {
            deactivate = deactivate.push(ui::card(column![
                text("This can't be undone.").font(MONO).size(13).color(theme::colors().red),
                ui::hint(format!("{} can never be used again, by you or anyone else. \
                    Your messages stay visible to people who already have them.", self.user_id)),
                ui::input("Password", &self.deactivate_password, Message::SettingsDeactivatePasswordChanged)
                    .on_submit(Message::SettingsDeactivate)
                    .secure(true),
                ui::footer(vec![
                    ui::btn("Cancel", Tone::Ghost, (!self.deactivating).then_some(Message::SettingsDeactivateArm(false))).into(),
                    ui::btn(if self.deactivating { "Deactivating\u{2026}" } else { "Deactivate account" }, Tone::Danger,
                        (!self.deactivating && !self.deactivate_password.is_empty()).then_some(Message::SettingsDeactivate)).into(),
                ]),
            ].spacing(10)));
        }
        col = col.push(deactivate);
        col.into()
    }

    fn view_settings_sessions(&self) -> Element<Message> {
        if self.settings_devices.is_empty() {
            return ui::empty_state(crate::icons::SVG_MONITOR, "Loading sessions\u{2026}", None);
        }
        let others: Vec<_> = self.settings_devices.iter().filter(|d| !d.is_current).collect();
        let mut col = column![ui::section_label("This session")].spacing(8);
        for device in self.settings_devices.iter().filter(|d| d.is_current) {
            col = col.push(self.view_device_row(device));
        }
        col = col.push(Space::new().height(8));
        col = col.push(ui::section_label(format!("Other sessions \u{2014} {}", others.len())));
        if others.is_empty() {
            col = col.push(ui::hint("You're not signed in anywhere else."));
        }
        for device in others {
            col = col.push(self.view_device_row(device));
        }
        col.into()
    }

    fn view_settings_security(&self) -> Element<Message> {
        let mut col = column![].spacing(ui::GAP);
        let status_row = |icon_svg: &'static [u8], color: Color, title: String, detail: String| -> Element<Message> {
            row![
                ui::icon(icon_svg, 18.0, color),
                column![
                    text(title).font(MONO).size(13).color(theme::colors().text_normal),
                    text(detail).font(MONO).size(11).color(theme::colors().text_dim)
                        .wrapping(iced::widget::text::Wrapping::WordOrGlyph),
                ].spacing(2).width(Fill),
            ]
            .spacing(12)
            .align_y(iced::Alignment::Center)
            .into()
        };

        match &self.security_state {
            None => {
                col = col.push(ui::empty_state(crate::icons::SVG_SHIELD, "Loading\u{2026}", None));
            }
            Some(sec) => {
                // Cross-signing
                col = col.push(ui::card(if sec.is_verified {
                    status_row(crate::icons::SVG_SHIELD_CHECK, theme::colors().green,
                        "This session is verified".into(), "It's cross-signed and trusted by your other devices.".into())
                } else {
                    status_row(crate::icons::SVG_SHIELD, theme::colors().yellow,
                        "This session isn't verified".into(),
                        "Verify it from another session (Sessions tab) or recover with your recovery key below.".into())
                }));

                // Key backup
                let backup_detail = format!(
                    "Server backup: {}  \u{00b7}  this device: {}  \u{00b7}  recovery: {}",
                    if sec.backup_exists { "exists" } else { "none" },
                    if sec.backup_enabled { "backing up" } else { "off" },
                    sec.recovery_state,
                );
                let mut backup = column![
                    status_row(crate::icons::SVG_KEY,
                        if sec.backup_enabled { theme::colors().green } else { theme::colors().text_dim },
                        "Key backup".into(), backup_detail),
                ]
                .spacing(10);
                if !sec.backup_enabled {
                    backup = backup.push(ui::footer(vec![
                        ui::btn("Enable backup", Tone::Primary, Some(Message::SecurityEnableBackup)).into(),
                    ]));
                }
                col = col.push(ui::card(backup));
            }
        }

        if let Some(ref key) = self.security_shown_recovery_key {
            col = col.push(
                container(
                    column![
                        text("Save your recovery key").font(MONO).size(13).color(theme::colors().yellow),
                        ui::hint("You'll need it to read encrypted history on new devices. Store it somewhere safe."),
                        container(
                            text(key.as_str()).font(MONO).size(14).color(theme::colors().text_bright)
                                .wrapping(iced::widget::text::Wrapping::WordOrGlyph),
                        )
                        .padding(12)
                        .width(Fill)
                        .style(|_: &Theme| container::Style {
                            background: Some(theme::colors().bg_deep.into()),
                            border: iced::Border { radius: 6.0.into(), ..Default::default() },
                            ..Default::default()
                        }),
                        ui::footer(vec![
                            ui::icon_btn(crate::icons::SVG_COPY, "Copy key", Tone::Secondary,
                                Some(Message::CopyMessageText(key.clone()))).into(),
                        ]),
                    ]
                    .spacing(10),
                )
                .padding(14)
                .width(Fill)
                .style(|_: &Theme| container::Style {
                    background: Some(theme::with_alpha(theme::colors().yellow, 0.06).into()),
                    border: iced::Border { color: theme::with_alpha(theme::colors().yellow, 0.5), width: 1.0, radius: 8.0.into() },
                    ..Default::default()
                }),
            );
        }

        // Recover
        col = col.push(ui::field(
            "Recover from backup",
            row![
                ui::input("Recovery key or passphrase", &self.security_recovery_key_input, Message::SecurityRecoveryKeyInputChanged)
                    .on_submit(Message::SecurityRecover)
                    .secure(true)
                    .width(Fill),
                ui::btn("Recover", Tone::Secondary,
                    (!self.security_recovery_key_input.trim().is_empty()).then_some(Message::SecurityRecover)),
            ]
            .spacing(8)
            .align_y(iced::Alignment::Center),
        ));

        if let Some(ref status) = self.settings_status {
            col = col.push(ui::status_line(status));
        }

        // Ignored users (the account's m.ignored_user_list)
        let mut ignored: Vec<&String> = self.ignored_users.iter().collect();
        ignored.sort();
        let mut list = column![].spacing(4);
        if ignored.is_empty() {
            list = list.push(ui::hint("You're not ignoring anyone. Ignore someone from their profile, or with /ignore."));
        }
        for user_id in ignored {
            let member = self.find_member(user_id).0;
            let name = member.map(|m| m.display_name.as_str()).filter(|n| !n.is_empty()).unwrap_or(user_id.as_str());
            list = list.push(container(row![
                self.render_avatar_with_initial(member.and_then(|m| m.avatar_url.as_deref()), name, 26.0),
                column![
                    text(name.to_string()).font(MONO).size(13).color(theme::colors().text_normal),
                    text(user_id.as_str()).font(MONO).size(11).color(theme::colors().text_dim),
                ].spacing(1).width(Fill),
                ui::btn("Unignore", Tone::Ghost, Some(Message::ToggleIgnoreUser(user_id.clone()))),
            ].spacing(10).align_y(iced::Alignment::Center)).padding([2, 4]));
        }
        col = col.push(ui::divider());
        col = col.push(ui::field("Ignored users", column![
            list,
            ui::hint("You won't see their messages or invites. Applies on all your devices."),
        ].spacing(6)));

        // Reset identity
        col = col.push(ui::divider());
        let mut reset = column![
            text("Lost your recovery key?").font(MONO).size(13).color(theme::colors().text_normal),
            ui::hint("Resetting your identity replaces the server key backup and creates a new recovery key. \
                Encrypted history that isn't on this device stays unreadable, and your other devices must verify again."),
        ]
        .spacing(6);
        if !self.security_reset_confirm {
            reset = reset.push(ui::footer(vec![
                ui::btn("Reset identity\u{2026}", Tone::Danger, Some(Message::SecurityResetIdentityConfirm)).into(),
            ]));
        } else {
            reset = reset.push(
                row![
                    ui::input("Account password", &self.security_reset_password, Message::SecurityResetPasswordChanged)
                        .on_submit(Message::SecurityResetIdentitySubmit)
                        .secure(true)
                        .width(Fill),
                    ui::btn("Cancel", Tone::Ghost, Some(Message::SecurityResetIdentityCancel)),
                    ui::btn(
                        if self.security_resetting { "Resetting\u{2026}" } else { "Reset" },
                        Tone::Danger,
                        (!self.security_resetting && !self.security_reset_password.is_empty())
                            .then_some(Message::SecurityResetIdentitySubmit),
                    ),
                ]
                .spacing(8)
                .align_y(iced::Alignment::Center),
            );
        }
        col = col.push(reset);
        col.into()
    }

    fn view_settings_preferences(&self) -> Element<Message> {
        // Each toggle sends a full updated copy of the preferences.
        let toggle = |label: &'static str, detail: &'static str, on: bool, set: fn(&mut crate::prefs::Prefs, bool)| -> Element<Message> {
            let base = self.prefs.clone();
            ui::toggle_row(label, detail, on, move |v| {
                let mut p = base.clone();
                set(&mut p, v);
                Message::SetPrefs(Box::new(p))
            })
        };
        let section = |items: Vec<Element<'static, Message>>| -> Element<Message> {
            let mut col = column![].spacing(10);
            for (i, item) in items.into_iter().enumerate() {
                if i > 0 {
                    col = col.push(ui::divider());
                }
                col = col.push(item);
            }
            ui::card(col).into()
        };
        let p = &self.prefs;
        let fmt = self.timestamp_format.as_str();
        let clock = |secs: bool| crate::view_messages::format_clock(14 * 3600 + 5 * 60 + 9, secs, p.twelve_hour);

        let timestamps = column![
            ui::segmented(vec![
                ("Hidden".to_string(), Some(Message::SetTimestampFormat("off".into())), fmt == "off"),
                (clock(false), Some(Message::SetTimestampFormat("short".into())), fmt != "off" && fmt != "full"),
                (clock(true), Some(Message::SetTimestampFormat("full".into())), fmt == "full"),
            ]),
            toggle("12-hour clock", "2:05 PM instead of 14:05", p.twelve_hour, |p, v| p.twelve_hour = v),
        ]
        .spacing(10);

        let layout = vec![
            ui::toggle_row("Compact layout", "Tighter rows, no avatars", self.compact_mode, Message::SetCompactMode),
            toggle("Group consecutive messages", "Fade the name and hide the avatar on follow-ups from the same person",
                p.group_messages, |p, v| p.group_messages = v),
            toggle("Big emoji", "Show messages that are only emoji at a larger size", p.big_emoji, |p, v| p.big_emoji = v),
            toggle("Link previews", "Show a card for links in messages", p.url_previews, |p, v| p.url_previews = v),
            toggle("Autoplay GIFs", "Animate GIFs in the timeline (off shows a still frame)", p.autoplay_gifs, |p, v| p.autoplay_gifs = v),
            toggle("Expand code blocks", "Show long code blocks in full instead of collapsed", p.expand_code_blocks, |p, v| p.expand_code_blocks = v),
        ];
        let events = vec![
            ui::toggle_row("Joins and leaves", "People joining or leaving the room", self.show_joinpart, Message::SetShowJoinPart),
            toggle("Name and avatar changes", "When someone changes their display name or avatar",
                p.show_profile_changes, |p, v| p.show_profile_changes = v),
            toggle("Room changes", "Name, topic, icon and permission changes", p.show_room_changes, |p, v| p.show_room_changes = v),
            toggle("Deleted messages", "Leave a \u{201c}Message deleted\u{201d} placeholder", p.show_redactions, |p, v| p.show_redactions = v),
            toggle("Read receipts", "Show who has read up to each message", p.show_read_receipts, |p, v| p.show_read_receipts = v),
            toggle("Typing indicators", "Show when others are typing", p.show_typing, |p, v| p.show_typing = v),
        ];
        let send_shortcut = if cfg!(target_os = "macos") { "\u{2318}+Enter sends; Enter adds a new line" } else { "Ctrl+Enter sends; Enter adds a new line" };
        let composer = vec![
            toggle("Send with Ctrl+Enter", send_shortcut, p.ctrl_enter_to_send, |p, v| p.ctrl_enter_to_send = v),
            toggle("Markdown", "Format **bold**, *italic*, `code` and lists when sending", p.use_markdown, |p, v| p.use_markdown = v),
            toggle("Emoji suggestions", "Complete :shortcodes: as you type", p.emoji_suggestions, |p, v| p.emoji_suggestions = v),
            toggle("Replace emoticons", "Turn :) and <3 into emoji when sending", p.auto_replace_emoji, |p, v| p.auto_replace_emoji = v),
        ];
        let privacy = vec![
            toggle("Send read receipts", "Let others see what you've read (off: your unread state still syncs, privately)",
                p.send_read_receipts, |p, v| p.send_read_receipts = v),
            toggle("Send typing notifications", "Let others see when you're typing", p.send_typing, |p, v| p.send_typing = v),
            toggle("Share presence", "Show others when you're online or away", p.share_presence, |p, v| p.share_presence = v),
        ];

        // Media previews (account data, shared with other clients).
        use fluxchat_core::profile::{MediaPreviewConfig, MediaPreviews};
        let mp = self.media_previews;
        let set = |c: MediaPreviewConfig| Some(Message::SetMediaPreviews(c));
        let media = column![
            ui::segmented(vec![
                ("Always".to_string(), set(MediaPreviewConfig { media: MediaPreviews::On, ..mp }), mp.media == MediaPreviews::On),
                ("In private rooms".to_string(), set(MediaPreviewConfig { media: MediaPreviews::Private, ..mp }), mp.media == MediaPreviews::Private),
                ("Never".to_string(), set(MediaPreviewConfig { media: MediaPreviews::Off, ..mp }), mp.media == MediaPreviews::Off),
            ]),
            ui::hint("Where images and videos show without a click. Hidden media isn't downloaded until you show it; yours always shows."),
            ui::divider(),
            ui::toggle_row("Invite avatars", "Show the icons of rooms you're invited to",
                mp.invite_avatars, move |v| Message::SetMediaPreviews(MediaPreviewConfig { invite_avatars: v, ..mp })),
        ]
        .spacing(10);

        column![
            ui::field("Timestamps", ui::card(timestamps)),
            ui::field("Timeline", section(layout)),
            ui::field("Media", ui::card(media)),
            ui::field("Show in the timeline", section(events)),
            ui::field("Composer", section(composer)),
            ui::field("Privacy", section(privacy)),
        ]
        .spacing(ui::GAP + 4.0)
        .into()
    }

    fn view_settings_voice(&self) -> Element<Message> {
        const DEFAULT: &str = "System default";
        // Device picker: "System default" first, then the device names.
        let picker = |devices: &[String], chosen: Option<&String>, set: fn(&mut crate::prefs::Prefs, Option<String>)| {
            let mut options = vec![DEFAULT.to_string()];
            options.extend(devices.iter().cloned());
            // A remembered device that's unplugged still shows as selected.
            if let Some(c) = chosen {
                if !options.contains(c) {
                    options.push(c.clone());
                }
            }
            let selected = chosen.cloned().unwrap_or_else(|| DEFAULT.to_string());
            let base = self.prefs.clone();
            iced::widget::pick_list(options, Some(selected), move |name: String| {
                let mut p = base.clone();
                set(&mut p, (name != DEFAULT).then_some(name));
                Message::SetPrefs(Box::new(p))
            })
            .font(MONO)
            .text_size(13)
            .padding([7, 10])
            .width(Fill)
            .style(ui::pick_list_style)
        };
        let toggle = |label: &'static str, detail: &'static str, on: bool, set: fn(&mut crate::prefs::Prefs, bool)| -> Element<Message> {
            let base = self.prefs.clone();
            ui::toggle_row(label, detail, on, move |v| {
                let mut p = base.clone();
                set(&mut p, v);
                Message::SetPrefs(Box::new(p))
            })
        };

        // Mic test: level meter + start/stop
        let level = self.mic_level.clamp(0.0, 1.0);
        let filled = ((level * 1000.0) as u16).clamp(1, 999);
        let meter = container(
            row![
                container(Space::new().height(8))
                    .width(Length::FillPortion(filled))
                    .style(move |_: &Theme| container::Style {
                        background: Some(if level > 0.85 { theme::colors().yellow } else { theme::colors().green }.into()),
                        border: iced::Border { radius: 4.0.into(), ..Default::default() },
                        ..Default::default()
                    }),
                Space::new().width(Length::FillPortion(1000 - filled)),
            ],
        )
        .padding(2)
        .width(Fill)
        .style(|_: &Theme| container::Style {
            background: Some(theme::colors().bg_deep.into()),
            border: iced::Border { color: theme::colors().border, width: 1.0, radius: 6.0.into() },
            ..Default::default()
        });
        let testing = self.mic_test.is_some();
        let mic_test = row![
            if testing { meter.into() } else { Element::from(ui::hint("Check your microphone level before a call.")) },
            ui::btn(if testing { "Stop test" } else { "Test microphone" }, Tone::Secondary, Some(Message::ToggleMicTest)),
        ]
        .spacing(12)
        .align_y(iced::Alignment::Center);

        let devices_loading = self.audio_inputs.is_empty() && self.audio_outputs.is_empty();
        let mut mic_col = column![
            picker(&self.audio_inputs, self.prefs.audio_input.as_ref(), |p, v| p.audio_input = v),
            mic_test,
        ].spacing(10);
        if testing && self.mic_blocked {
            let fix = if cfg!(target_os = "macos") {
                "No sound is reaching FluxChat. Allow it in System Settings \u{2192} Privacy & Security \u{2192} Microphone, then restart FluxChat. (Or pick another microphone.)"
            } else {
                "No sound is reaching FluxChat. Check that the microphone isn't muted, or pick another one."
            };
            mic_col = mic_col.push(text(fix).font(MONO).size(12).color(theme::colors().yellow));
        }
        let mut col = column![
            ui::field("Microphone", mic_col),
            ui::field("Speaker", picker(&self.audio_outputs, self.prefs.audio_output.as_ref(), |p, v| p.audio_output = v)),
        ]
        .spacing(ui::GAP + 4.0);
        if devices_loading {
            col = col.push(ui::hint("Looking for audio devices\u{2026}"));
        }
        col = col.push(ui::field("Voice processing", ui::card(column![
            toggle("Echo cancellation", "Stop others hearing themselves through your speakers",
                self.prefs.echo_cancellation, |p, v| p.echo_cancellation = v),
            ui::divider(),
            toggle("Noise suppression", "Filter out background noise", self.prefs.noise_suppression, |p, v| p.noise_suppression = v),
            ui::divider(),
            toggle("Automatic gain", "Keep your voice at an even volume", self.prefs.auto_gain, |p, v| p.auto_gain = v),
        ].spacing(10))));
        col = col.push(ui::hint(if cfg!(target_os = "linux") {
            "Changes apply to the next call. On Linux, call audio uses the PulseAudio default devices; the choices here apply to voice messages."
        } else {
            "Changes apply to the next call and voice message."
        }));
        col.into()
    }

    fn view_settings_appearance(&self) -> Element<Message> {
        let fs = self.msg_font_size;
        let msg_font = crate::view::font_by_name(&self.msg_font_family);
        let msg_font_bold = crate::view::font_bold_by_name(&self.msg_font_family);
        let sidebar_font = crate::view::font_by_name(&self.sidebar_font_family);
        let theme_names: Vec<String> = self.available_themes.iter().map(|t| t.name.clone()).collect();

        let pick = |options: Vec<String>, selected: String, on: fn(String) -> Message| {
            iced::widget::pick_list(options, Some(selected), on)
                .font(MONO)
                .text_size(13)
                .padding([7, 10])
                .width(Fill)
                .style(ui::pick_list_style)
        };

        let preview = container(
            column![
                row![
                    text("12:04").font(msg_font).size(fs * 0.85).color(theme::colors().text_dim),
                    text("alice").font(msg_font_bold).size(fs).color(theme::nick_color("alice")),
                    text("\u{2502}").font(msg_font).size(fs).color(theme::colors().border),
                    text("The quick brown fox jumps over the lazy dog.").font(msg_font).size(fs).color(theme::colors().text_normal),
                ].spacing(10).align_y(iced::Alignment::Center),
                row![
                    text("12:05").font(msg_font).size(fs * 0.85).color(theme::colors().text_dim),
                    text("bob").font(msg_font_bold).size(fs).color(theme::nick_color("bob")),
                    text("\u{2502}").font(msg_font).size(fs).color(theme::colors().border),
                    text("bold, italic and code look like this").font(msg_font).size(fs).color(theme::colors().text_normal),
                ].spacing(10).align_y(iced::Alignment::Center),
                ui::divider(),
                text("# general    # random    @ alice").font(sidebar_font).size(13).color(theme::colors().text_muted),
            ]
            .spacing(8),
        )
        .padding(14)
        .width(Fill)
        .style(|_: &Theme| container::Style {
            background: Some(theme::colors().bg_deep.into()),
            border: iced::Border { color: theme::colors().border, width: 1.0, radius: 8.0.into() },
            ..Default::default()
        });

        column![
            ui::field("Theme", column![
                pick(theme_names, self.active_theme_name.clone(), Message::SetTheme),
                ui::hint("Add your own: drop .toml theme files into data_dir/fluxchat/themes/."),
            ].spacing(6)),
            row![
                container(ui::field("Message font", pick(self.available_fonts.clone(), self.msg_font_family.clone(), Message::SetMsgFontFamily))).width(Fill),
                container(ui::field("Sidebar font", pick(self.available_fonts.clone(), self.sidebar_font_family.clone(), Message::SetSidebarFontFamily))).width(Fill),
            ].spacing(12),
            ui::field("Message text size", row![
                iced::widget::slider(10.0..=24.0, fs, Message::SetMsgFontSize).step(1.0_f32).width(Fill),
                text(format!("{fs:.0}px")).font(MONO).size(13).color(theme::colors().text_normal).width(48),
            ].spacing(12).align_y(iced::Alignment::Center)),
            ui::field("Preview", preview),
        ]
        .spacing(ui::GAP + 4.0)
        .into()
    }

    fn view_settings_notifications(&self) -> Element<Message> {
        use fluxchat_core::notify_rules::{DefaultLevel, NotifyToggle};
        let enabled = self.notifications_enabled;
        let sub = |label: &'static str, detail: &'static str, on: bool, msg: fn(bool) -> Message| -> Element<Message> {
            if enabled {
                ui::toggle_row(label, detail, on, msg)
            } else {
                // Dimmed and inert while desktop notifications are off.
                row![
                    column![
                        text(label).font(MONO).size(13).color(theme::colors().text_dim),
                        text(detail).font(MONO).size(11).color(theme::colors().text_dim),
                    ].spacing(2).width(Fill),
                    iced::widget::toggler(on).size(18).style(ui::toggle_style),
                ]
                .spacing(12)
                .align_y(iced::Alignment::Center)
                .into()
            }
        };
        let desktop = column![
            ui::card(ui::toggle_row(
                "Desktop notifications", "Show a system notification on this device",
                enabled, Message::SetNotificationsEnabled,
            )),
            ui::card(column![
                sub("Only for mentions", "Only alert on this device when you're mentioned or a keyword matches",
                    self.notify_mentions_only, Message::SetNotifyMentionsOnly),
                ui::divider(),
                sub("Show message preview", "Include the message text in the notification",
                    self.notify_show_preview, Message::SetNotifyShowPreview),
                ui::divider(),
                sub("Play sound", "Use the system notification sound",
                    self.notify_sound, Message::SetNotifySound),
            ].spacing(12)),
        ]
        .spacing(10);

        let mut col = column![ui::field("This device", desktop)].spacing(ui::GAP + 4.0);

        let Some(rules) = &self.notify_rules else {
            col = col.push(ui::field("Your account", ui::hint("Loading your notification rules\u{2026}")));
            return col.into();
        };

        let level_picker = |dm: bool, current: DefaultLevel| ui::segmented(vec![
            ("All messages".to_string(), Some(Message::SetDefaultNotify(dm, DefaultLevel::AllMessages)), current == DefaultLevel::AllMessages),
            ("Mentions & keywords".to_string(), Some(Message::SetDefaultNotify(dm, DefaultLevel::MentionsAndKeywords)), current == DefaultLevel::MentionsAndKeywords),
        ]);
        let toggle = |label: &'static str, detail: &'static str, on: bool, t: NotifyToggle| -> Element<Message> {
            ui::toggle_row(label, detail, on, move |v| Message::SetNotifyToggle(t, v))
        };

        let defaults = ui::card(column![
            text("Direct messages").font(MONO).size(13).color(theme::colors().text_normal),
            level_picker(true, rules.dms),
            Space::new().height(4),
            text("Group rooms").font(MONO).size(13).color(theme::colors().text_normal),
            level_picker(false, rules.groups),
        ].spacing(8));

        let mentions = ui::card(column![
            toggle("Mentions of me", "Your name, username or a mention pill", rules.mention_me, NotifyToggle::MentionMe),
            ui::divider(),
            toggle("@room", "When someone notifies the whole room", rules.mention_room, NotifyToggle::MentionRoom),
        ].spacing(10));

        // Keywords: chips + add field
        let mut chips = row![].spacing(6);
        for kw in &rules.keywords {
            chips = chips.push(
                container(
                    row![
                        text(kw.as_str()).font(MONO).size(12).color(theme::colors().text_normal),
                        button(ui::icon(crate::icons::SVG_CLOSE, 11.0, theme::colors().text_dim))
                            .on_press(Message::RemoveKeyword(kw.clone()))
                            .padding(2)
                            .style(ui::button_style(Tone::Ghost)),
                    ]
                    .spacing(4)
                    .align_y(iced::Alignment::Center),
                )
                .padding(iced::Padding { top: 3.0, right: 4.0, bottom: 3.0, left: 10.0 })
                .style(|_: &Theme| container::Style {
                    background: Some(theme::colors().bg_light.into()),
                    border: iced::Border { color: theme::colors().border, width: 1.0, radius: 12.0.into() },
                    ..Default::default()
                }),
            );
        }
        let keywords = ui::card(column![
            if rules.keywords.is_empty() {
                Element::from(ui::hint("No keywords yet. Messages containing a keyword notify you and are highlighted."))
            } else {
                iced::widget::scrollable(chips)
                    .direction(iced::widget::scrollable::Direction::Horizontal(
                        iced::widget::scrollable::Scrollbar::new().width(3).scroller_width(3),
                    ))
                    .style(thin_scrollbar)
                    .into()
            },
            row![
                ui::input("Add a keyword", &self.keyword_input, Message::KeywordInputChanged)
                    .on_submit(Message::AddKeyword)
                    .width(Fill),
                ui::btn("Add", Tone::Secondary, (!self.keyword_input.trim().is_empty()).then_some(Message::AddKeyword)),
            ].spacing(8).align_y(iced::Alignment::Center),
        ].spacing(10));

        let other = ui::card(column![
            toggle("Invites", "When you're invited to a room", rules.invites, NotifyToggle::Invites),
            ui::divider(),
            toggle("Calls", "Incoming calls", rules.calls, NotifyToggle::Calls),
            ui::divider(),
            toggle("Bot messages", "Notices sent by bots", rules.bot_notices, NotifyToggle::BotNotices),
            ui::divider(),
            toggle("Room upgrades", "When a room moves to a new version", rules.room_upgrades, NotifyToggle::RoomUpgrades),
        ].spacing(10));

        col = col
            .push(ui::field("Notify me for", defaults))
            .push(ui::field("Mentions", mentions))
            .push(ui::field("Keywords", keywords))
            .push(ui::field("Other", other))
            .push(ui::hint("Account settings are saved on your server and apply on all your devices. Per-room overrides live in each room's settings."));
        col.into()
    }

    fn view_create_room_modal(&self) -> Element<Message> {
        let is_space = self.create_room_is_space;
        let kind = ui::segmented(vec![
            ("Room".to_string(), is_space.then_some(Message::CreateRoomToggleSpace), !is_space),
            ("Space".to_string(), (!is_space).then_some(Message::CreateRoomToggleSpace), is_space),
        ]);

        let mut col = column![
            ui::header(
                if is_space { "Create a space" } else { "Create a room" },
                Some(if is_space {
                    "Spaces group rooms and people together.".into()
                } else {
                    "Rooms are where conversations happen.".into()
                }),
            ),
            kind,
            ui::field("Name", ui::input(
                if is_space { "e.g. My Community" } else { "e.g. general" },
                &self.create_room_name,
                Message::CreateRoomNameChanged,
            ).on_submit(Message::CreateRoomSubmit)),
            ui::field("Topic (optional)", ui::input(
                if is_space { "What is this space about?" } else { "What is this room about?" },
                &self.create_room_topic,
                Message::CreateRoomTopicChanged,
            )),
        ]
        .spacing(ui::GAP)
        .width(480);

        // Parent space (rooms only)
        if !is_space && !self.spaces.is_empty() {
            let mut chips = row![].spacing(6);
            let chip = |label: String, active: bool, msg: Message| {
                button(text(label).font(MONO).size(12))
                    .on_press(msg)
                    .padding([5, 10])
                    .style(move |_, status| {
                        let c = theme::colors();
                        button::Style {
                            background: Some(if active {
                                theme::with_alpha(c.accent, 0.2)
                            } else if matches!(status, button::Status::Hovered) {
                                c.bg_hover
                            } else {
                                c.bg_deep
                            }.into()),
                            text_color: if active { c.text_bright } else { c.text_muted },
                            border: iced::Border { color: if active { c.accent } else { c.border }, width: 1.0, radius: 14.0.into() },
                            ..Default::default()
                        }
                    })
            };
            chips = chips.push(chip("None".into(), self.create_room_space.is_none(), Message::CreateRoomSelectSpace(None)));
            for space in &self.spaces {
                let active = self.create_room_space.as_deref() == Some(space.room_id.as_str());
                chips = chips.push(chip(space.name.clone(), active, Message::CreateRoomSelectSpace(Some(space.room_id.clone()))));
            }
            col = col.push(ui::field(
                "Add to space",
                scrollable(chips)
                    .direction(iced::widget::scrollable::Direction::Horizontal(
                        iced::widget::scrollable::Scrollbar::new().width(3).scroller_width(3),
                    ))
                    .style(thin_scrollbar),
            ));
        }

        // Options
        let toggle = |label: &'static str, detail: &'static str, on: bool, msg: Message| -> Element<Message> {
            ui::toggle_row(label, detail, on, move |_| msg.clone())
        };
        let mut options = column![
            toggle("Public", "Anyone can find and join it", self.create_room_public, Message::CreateRoomTogglePublic),
        ]
        .spacing(12);
        if !is_space {
            options = options.push(toggle(
                "End-to-end encryption", "Can't be turned off later",
                self.create_room_encrypted, Message::CreateRoomToggleEncrypted,
            ));
        }
        col = col.push(ui::card(options));
        col = col.push(ui::footer(vec![
            ui::btn("Cancel", Tone::Ghost, Some(Message::CloseModal)).into(),
            ui::btn(
                if is_space { "Create space" } else { "Create room" },
                Tone::Primary,
                (!self.create_room_name.trim().is_empty()).then_some(Message::CreateRoomSubmit),
            ).into(),
        ]));
        col.into()
    }

    fn view_verification_modal<'a>(&'a self, info: &'a fluxchat_core::VerificationInfo) -> Element<'a, Message> {
        let uid = info.other_user_id.clone();
        let fid = info.flow_id.clone();
        let who = if info.is_self_verification {
            "your other session".to_string()
        } else {
            self.find_member(&info.other_user_id).0
                .map(|m| m.display_name.clone())
                .unwrap_or_else(|| info.other_user_id.clone())
        };
        let (title, subtitle) = match info.state.as_str() {
            "requested" => ("Verification request".to_string(), format!("{who} wants to verify with you.")),
            "emojis_ready" => ("Compare emoji".to_string(),
                format!("Confirm the emoji below appear, in the same order, on {who}'s screen.")),
            s if s.starts_with("requesting") => ("Verify".to_string(),
                format!("Waiting for {who} to accept the request\u{2026}")),
            other => ("Verify".to_string(), other.to_string()),
        };
        let mut col = column![ui::header(title, Some(subtitle))].spacing(ui::GAP);

        if let Some(ref emojis) = info.emojis {
            let mut grid = column![].spacing(10);
            for chunk in emojis.chunks(4) {
                let mut r = row![].spacing(10);
                for emoji in chunk {
                    r = r.push(
                        container(
                            column![
                                text(&emoji.symbol).size(34),
                                text(&emoji.description).font(MONO).size(11).color(theme::colors().text_muted),
                            ]
                            .spacing(6)
                            .align_x(iced::Alignment::Center),
                        )
                        .padding([12, 4])
                        .width(Fill)
                        .center_x(Fill)
                        .style(|_: &Theme| container::Style {
                            background: Some(theme::colors().bg_mid.into()),
                            border: iced::Border { color: theme::colors().border, width: 1.0, radius: 8.0.into() },
                            ..Default::default()
                        }),
                    );
                }
                grid = grid.push(r);
            }
            col = col.push(grid);
        } else if info.state != "requested" {
            col = col.push(ui::empty_state(crate::icons::SVG_SHIELD, "Waiting\u{2026}", None));
        }

        let actions: Vec<Element<Message>> = if info.state == "requested" {
            vec![
                ui::btn("Decline", Tone::Ghost, Some(Message::VerificationCancel(uid.clone(), fid.clone()))).into(),
                ui::btn("Accept", Tone::Primary, Some(Message::VerificationAccept(uid, fid))).into(),
            ]
        } else if info.emojis.is_some() {
            vec![
                ui::btn("They don't match", Tone::Danger, Some(Message::VerificationCancel(uid.clone(), fid.clone()))).into(),
                ui::btn("They match", Tone::Success, Some(Message::VerificationConfirm(uid, fid))).into(),
            ]
        } else {
            vec![ui::btn("Cancel", Tone::Ghost, Some(Message::VerificationCancel(uid, fid))).into()]
        };
        col.push(ui::footer(actions)).width(440).into()
    }

    fn view_lightbox_modal(&self, url: &str) -> Element<Message> {
        // The message the image belongs to (for its name, sender and time).
        let msg = self.selected_room_id.as_ref()
            .and_then(|rid| self.messages_by_room.get(rid))
            .and_then(|msgs| msgs.iter().rev().find(|m| m.image_mxc.as_deref() == Some(url)));
        let title = msg.map(|m| m.body.clone()).filter(|b| !b.trim().is_empty())
            .unwrap_or_else(|| "Image".into());
        let subtitle = msg.map(|m| {
            let secs = crate::view::local_time_of_day(m.timestamp);
            format!(
                "{}  \u{00b7}  {}, {}",
                m.sender_name,
                crate::view_messages::local_day_label(m.timestamp),
                crate::view_messages::format_clock(secs, false, self.prefs.twelve_hour),
            )
        });

        let image_el: Element<Message> = if let Some(handle) = self.image_cache.get(url) {
            iced::widget::image(handle.clone())
                .content_fit(iced::ContentFit::Contain)
                .width(Fill)
                .height(Fill)
                .into()
        } else {
            ui::empty_state(crate::icons::SVG_IMAGE, "Loading image\u{2026}", None)
        };

        let header = row![
            column![
                text(title).font(MONO).size(16).color(theme::colors().text_bright)
                    .wrapping(iced::widget::text::Wrapping::None),
                text(subtitle.unwrap_or_default()).font(MONO).size(12).color(theme::colors().text_dim),
            ]
            .spacing(3)
            .width(Fill)
            .clip(true),
            ui::icon_btn(crate::icons::SVG_COPY, "Copy", Tone::Secondary, Some(Message::CopyImage(url.to_string()))),
            ui::icon_btn(crate::icons::SVG_FILE, "Save", Tone::Secondary, Some(Message::SaveImage(url.to_string()))),
            ui::close_button(),
        ]
        .spacing(8)
        .align_y(iced::Alignment::Center);

        // Clicking the enlarged image closes it again (as do the backdrop,
        // Escape and the close button).
        let stage = iced::widget::mouse_area(
            container(image_el)
                .width(Fill)
                .height(Fill)
                .center(Fill)
                .padding(8)
                .clip(true)
                .style(|_: &Theme| container::Style {
                    background: Some(theme::colors().bg_deep.into()),
                    border: iced::Border { color: theme::colors().border, width: 1.0, radius: 8.0.into() },
                    ..Default::default()
                }),
        )
        .on_press(Message::CloseModal)
        .interaction(iced::mouse::Interaction::ZoomOut);

        column![
            header,
            stage,
            container(text("Click the image or press Esc to close").font(MONO).size(11).color(theme::colors().text_dim))
                .width(Fill)
                .center_x(Fill),
        ]
        .spacing(ui::GAP)
        .width(Length::FillPortion(9))
        .height(Length::FillPortion(9))
        .into()
    }

    fn view_forward_modal(&self, body: &str, sender_name: Option<&str>) -> Element<Message> {
        let subtitle = match sender_name {
            Some(name) => format!("Message from {name}"),
            None => "Pick a room to send it to".to_string(),
        };
        let preview: String = body.lines().next().unwrap_or("").chars().take(140).collect();
        let quote = container(
            text(preview).font(MONO).size(12).color(theme::colors().text_muted)
                .wrapping(iced::widget::text::Wrapping::WordOrGlyph),
        )
        .padding([8, 12])
        .width(Fill)
        .style(|_: &Theme| container::Style {
            background: Some(theme::colors().bg_mid.into()),
            border: iced::Border { color: theme::colors().border, width: 1.0, radius: 6.0.into() },
            ..Default::default()
        });

        let query = self.forward_search.to_lowercase();
        let mut list = column![].spacing(2);
        let mut any = false;
        for room in self.rooms.iter()
            .filter(|r| query.is_empty() || r.name.to_lowercase().contains(&query))
            .take(40)
        {
            any = true;
            let detail = room.space_name.clone().unwrap_or_else(|| if room.is_direct { "Direct message".into() } else { String::new() });
            list = list.push(
                button(
                    row![
                        self.render_room_avatar(room.avatar_url.as_deref(), &room.name, 28.0),
                        column![
                            text(&room.name).font(MONO).size(13).color(theme::colors().text_bright)
                                .wrapping(iced::widget::text::Wrapping::None),
                            text(detail).font(MONO).size(11).color(theme::colors().text_dim),
                        ].spacing(1).width(Fill),
                        ui::icon(crate::icons::SVG_FORWARD, 14.0, theme::colors().text_dim),
                    ]
                    .spacing(10)
                    .align_y(iced::Alignment::Center),
                )
                .on_press(Message::ForwardToRoom(room.room_id.clone()))
                .padding([6, 8])
                .width(Fill)
                .style(ui::row_style(false)),
            );
        }
        let results: Element<Message> = if any {
            scrollable(list)
                .height(Length::Fixed(280.0))
                .direction(iced::widget::scrollable::Direction::Vertical(
                    iced::widget::scrollable::Scrollbar::new().width(4).scroller_width(4),
                ))
                .style(thin_scrollbar)
                .into()
        } else {
            ui::empty_state(crate::icons::SVG_SEARCH, "No matching rooms", None)
        };
        column![
            ui::header("Forward message", Some(subtitle)),
            quote,
            ui::input("Search rooms", &self.forward_search, Message::ForwardSearchChanged),
            results,
        ]
        .spacing(ui::GAP)
        .width(460)
        .into()
    }

    fn view_room_settings_modal(&self) -> Element<Message> {
        let Some(room) = self.active_real_room()
            .and_then(|rid| self.rooms.iter().find(|r| r.room_id == rid))
        else {
            return column![
                ui::header("Room settings", None),
                ui::empty_state(crate::icons::SVG_HASH, "No room selected", None),
            ]
            .spacing(ui::GAP)
            .width(520)
            .into();
        };

        // --- General ---
        let uploading = self.room_avatar_status.as_deref().is_some_and(|s| s.starts_with("Uploading"));
        let mut icon_col = column![
            ui::section_label("Room icon"),
            row![
                ui::icon_btn(crate::icons::SVG_IMAGE, "Upload image", Tone::Secondary,
                    (!uploading).then_some(Message::RoomSettingsUploadAvatar)),
                ui::btn("Remove", Tone::Ghost,
                    (room.avatar_url.is_some() && !uploading).then_some(Message::RoomSettingsRemoveAvatar)),
            ]
            .spacing(8),
        ]
        .spacing(8);
        if let Some(status) = &self.room_avatar_status {
            let color = if uploading { theme::colors().text_muted } else { theme::colors().red };
            icon_col = icon_col.push(text(status.as_str()).font(MONO).size(12).color(color));
        }
        let avatar_row = row![
            self.render_room_avatar(room.avatar_url.as_deref(), &room.name, 64.0),
            icon_col,
        ]
        .spacing(16)
        .align_y(iced::Alignment::Center);
        let name_dirty = self.room_settings_name.trim() != room.name && !self.room_settings_name.trim().is_empty();
        let topic_dirty = self.room_settings_topic.trim() != room.topic.as_deref().unwrap_or("");
        let name_row = row![
            ui::input("Room name", &self.room_settings_name, Message::RoomSettingsNameChanged)
                .on_submit(Message::RoomSettingsSaveName)
                .width(Fill),
            ui::btn("Save", Tone::Secondary, name_dirty.then_some(Message::RoomSettingsSaveName)),
        ]
        .spacing(8)
        .align_y(iced::Alignment::Center);
        let topic_row = row![
            ui::input("What's this room about?", &self.room_settings_topic, Message::RoomSettingsTopicChanged)
                .on_submit(Message::RoomSettingsSaveTopic)
                .width(Fill),
            ui::btn("Save", Tone::Secondary, topic_dirty.then_some(Message::RoomSettingsSaveTopic)),
        ]
        .spacing(8)
        .align_y(iced::Alignment::Center);

        // --- Notifications ---
        let mode = self.room_notify_mode.as_deref();
        let notify = ui::segmented(
            [("all", "All messages"), ("mentions", "Mentions"), ("mute", "Mute")]
                .into_iter()
                .map(|(key, label)| (
                    label.to_string(),
                    mode.is_some().then(|| Message::SetRoomNotifyMode(key.to_string())),
                    mode == Some(key),
                ))
                .collect(),
        );

        // --- Spaces: the ones this room is in, plus an "add to" picker
        // limited to spaces we're allowed to edit ---
        let mut spaces_col = column![].spacing(6);
        match &self.room_parent_spaces {
            None => spaces_col = spaces_col.push(ui::hint("Loading\u{2026}")),
            Some(parents) => {
                let in_spaces: Vec<&fluxchat_core::RoomInfo> = self.spaces.iter()
                    .filter(|sp| parents.contains(&sp.room_id))
                    .collect();
                if in_spaces.is_empty() {
                    spaces_col = spaces_col.push(ui::hint("This room isn't in any space."));
                }
                for space in in_spaces {
                    let can_remove = self.manageable_spaces.contains(&space.room_id);
                    spaces_col = spaces_col.push(
                        row![
                            self.render_room_avatar(space.avatar_url.as_deref(), &space.name, 22.0),
                            text(&space.name).font(MONO).size(13).color(theme::colors().text_normal).width(Fill),
                            ui::btn("Remove", Tone::Ghost,
                                can_remove.then(|| Message::RemoveRoomFromSpace(space.room_id.clone()))),
                        ]
                        .spacing(10)
                        .align_y(iced::Alignment::Center),
                    );
                }
                let choices: Vec<SpaceChoice> = self.spaces.iter()
                    .filter(|sp| self.manageable_spaces.contains(&sp.room_id) && !parents.contains(&sp.room_id))
                    .map(|sp| SpaceChoice { id: sp.room_id.clone(), name: sp.name.clone() })
                    .collect();
                if !choices.is_empty() {
                    let selected = self.room_settings_add_space.as_ref()
                        .and_then(|id| choices.iter().find(|c| c.id == *id).cloned());
                    let add_msg = selected.as_ref().map(|c| Message::AddRoomToSpace(c.id.clone()));
                    spaces_col = spaces_col.push(
                        row![
                            iced::widget::pick_list(choices, selected, |c: SpaceChoice| Message::RoomSettingsPickSpace(c.id))
                                .placeholder("Add to a space\u{2026}")
                                .font(MONO)
                                .text_size(13)
                                .padding([7, 10])
                                .width(Fill)
                                .style(ui::pick_list_style),
                            ui::btn("Add", Tone::Secondary, add_msg),
                        ]
                        .spacing(8)
                        .align_y(iced::Alignment::Center),
                    );
                }
            }
        }

        let body = column![
            avatar_row,
            ui::field("Name", name_row),
            ui::field("Topic", topic_row),
            ui::divider(),
            ui::field("Notify me in this room", column![
                notify,
                ui::hint("Only affects this room. Desktop alerts are set in Settings \u{2192} Notifications."),
            ].spacing(6)),
            ui::field("Spaces", spaces_col),
            ui::divider(),
            row![
                column![
                    text("Leave room").font(MONO).size(13).color(theme::colors().text_normal),
                    ui::hint("You'll need an invite or a public link to rejoin."),
                ].spacing(2).width(Fill),
                ui::icon_btn(crate::icons::SVG_LOG_OUT, "Leave", Tone::Danger, Some(Message::AskLeaveRoom(room.room_id.clone()))),
            ]
            .spacing(12)
            .align_y(iced::Alignment::Center),
        ]
        .spacing(ui::GAP);

        use crate::state::RoomSettingsTab;
        let tab = self.room_settings_tab;
        let body: Element<Message> = match tab {
            RoomSettingsTab::General => body.into(),
            RoomSettingsTab::Security => self.view_room_security_tab(&room.room_id),
            RoomSettingsTab::Roles => self.view_room_roles_tab(&room.room_id),
            RoomSettingsTab::Advanced => self.view_room_advanced_tab(&room.room_id),
            RoomSettingsTab::Bridges => self.view_room_bridges_tab(),
        };
        let mut tabs = vec![
            ("General".into(), Message::RoomSettingsTabChanged(RoomSettingsTab::General), tab == RoomSettingsTab::General),
            ("Security & Privacy".into(), Message::RoomSettingsTabChanged(RoomSettingsTab::Security), tab == RoomSettingsTab::Security),
            ("Roles & Permissions".into(), Message::RoomSettingsTabChanged(RoomSettingsTab::Roles), tab == RoomSettingsTab::Roles),
            ("Advanced".into(), Message::RoomSettingsTabChanged(RoomSettingsTab::Advanced), tab == RoomSettingsTab::Advanced),
        ];
        // Like Element: only bridged rooms get the tab.
        if self.room_bridges.as_ref().is_some_and(|b| !b.is_empty()) {
            tabs.push(("Bridges".into(), Message::RoomSettingsTabChanged(RoomSettingsTab::Bridges), tab == RoomSettingsTab::Bridges));
        }
        column![
            ui::header("Room settings", Some(room.name.clone())),
            ui::tabs(tabs),
            container(
                scrollable(container(body).padding(iced::Padding { right: 10.0, ..Default::default() }))
                    .id(crate::view::dialog_scroll_id())
                    .direction(iced::widget::scrollable::Direction::Vertical(
                        iced::widget::scrollable::Scrollbar::new().width(4).scroller_width(4),
                    ))
                    .style(thin_scrollbar),
            )
            .height(Length::Fixed(520.0)),
        ]
        .spacing(ui::GAP)
        // Wide enough for all five tab labels (Bridges included).
        .width(640)
        .into()
    }

    fn view_invite_modal(&self) -> Element<Message> {
        let target = self.invite_target_id.as_deref()
            .or(self.active_real_room());
        let target_name = target
            .and_then(|t| self.rooms.iter().chain(self.spaces.iter()).find(|r| r.room_id == t))
            .map(|r| r.name.clone());
        let subtitle = match target_name {
            Some(n) => format!("Invite people to {n}"),
            None => "Invite people".into(),
        };
        self.view_user_picker(
            "Invite",
            subtitle,
            &self.invite_search,
            &self.invite_results,
            Message::InviteSearchChanged,
            Message::InviteSearchSubmit,
            Message::InviteUserFromModal,
            "Invite",
        )
    }

    fn view_create_dm_modal(&self) -> Element<Message> {
        self.view_user_picker(
            "New message",
            "Start a direct conversation".into(),
            &self.dm_search,
            &self.dm_results,
            Message::DmSearchChanged,
            Message::DmSearchSubmit,
            Message::SendDmToUser,
            "Message",
        )
    }

    /// Search-the-directory dialog shared by Invite and New Message.
    #[allow(clippy::too_many_arguments)]
    fn view_user_picker<'a>(
        &'a self,
        title: &'a str,
        subtitle: String,
        search: &'a str,
        results: &'a [fluxchat_core::rooms::UserSearchResult],
        on_change: fn(String) -> Message,
        on_submit: Message,
        on_pick: fn(String) -> Message,
        cta: &'a str,
    ) -> Element<'a, Message> {
        let search_row = row![
            ui::input("Name or @user:server", search, on_change).on_submit(on_submit.clone()).width(Fill),
            ui::icon_btn(crate::icons::SVG_SEARCH, "Search", Tone::Secondary, Some(on_submit)),
        ]
        .spacing(8)
        .align_y(iced::Alignment::Center);

        let looks_like_id = search.starts_with('@') && search.contains(':');
        let mut list = column![].spacing(2);
        for user in results {
            let name = user.display_name.as_deref().unwrap_or(&user.user_id);
            list = list.push(
                container(
                    row![
                        self.render_avatar_with_initial(user.avatar_url.as_deref(), name, 32.0),
                        column![
                            text(name).font(MONO).size(13).color(theme::nick_color(name))
                                .wrapping(iced::widget::text::Wrapping::None),
                            text(&user.user_id).font(MONO).size(11).color(theme::colors().text_dim)
                                .wrapping(iced::widget::text::Wrapping::None),
                        ].spacing(2).width(Fill),
                        ui::btn(cta, Tone::Secondary, Some(on_pick(user.user_id.clone()))),
                    ]
                    .spacing(10)
                    .align_y(iced::Alignment::Center),
                )
                .padding([6, 8])
                .width(Fill),
            );
        }
        let body: Element<Message> = if results.is_empty() {
            if looks_like_id {
                container(
                    row![
                        self.render_avatar_with_initial(None, search.trim_start_matches('@'), 32.0),
                        text(search).font(MONO).size(13).color(theme::colors().text_bright).width(Fill),
                        ui::btn(cta, Tone::Primary, Some(on_pick(search.to_string()))),
                    ]
                    .spacing(10)
                    .align_y(iced::Alignment::Center),
                )
                .padding([6, 8])
                .into()
            } else if search.is_empty() {
                ui::empty_state(crate::icons::SVG_USERS, "Search the user directory",
                    Some("Type a name, or paste a full Matrix ID.".into()))
            } else {
                ui::empty_state(crate::icons::SVG_SEARCH, "No users found",
                    Some("Press Enter to search, or use a full @user:server ID.".into()))
            }
        } else {
            scrollable(list)
                .height(Length::Shrink)
                .direction(iced::widget::scrollable::Direction::Vertical(
                    iced::widget::scrollable::Scrollbar::new().width(4).scroller_width(4),
                ))
                .style(thin_scrollbar)
                .into()
        };
        column![
            ui::header(title, Some(subtitle)),
            search_row,
            container(body).max_height(320.0),
        ]
        .spacing(ui::GAP)
        .width(460)
        .into()
    }

    /// Single device/session row for the sessions settings tab.
    pub(crate) fn view_device_row<'a>(&self, device: &'a fluxchat_core::devices::DeviceInfo) -> Element<'a, Message> {
        let name = device.display_name.as_deref().unwrap_or("Unknown device");
        let last_seen = device.last_seen_ts.map(|ts| {
            let now = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default()
                .as_millis() as u64;
            let ago = now.saturating_sub(ts) / 1000;
            if ago < 60 { "active now".to_string() }
            else if ago < 3600 { format!("active {}m ago", ago / 60) }
            else if ago < 86400 { format!("active {}h ago", ago / 3600) }
            else { format!("active {}d ago", ago / 86400) }
        }).unwrap_or_else(|| "last activity unknown".to_string());
        let mut details = vec![device.device_id.clone(), last_seen];
        if let Some(ref ip) = device.last_seen_ip {
            details.push(ip.clone());
        }

        let mut title = row![
            text(name).font(MONO).size(13).color(theme::colors().text_bright),
        ]
        .spacing(8)
        .align_y(iced::Alignment::Center);
        if device.is_verified {
            title = title.push(ui::badge("Verified", theme::colors().green));
        } else {
            title = title.push(ui::badge("Unverified", theme::colors().yellow));
        }

        let mut col = column![
            row![
                ui::icon(crate::icons::SVG_MONITOR, 18.0,
                    if device.is_current { theme::colors().accent } else { theme::colors().text_dim }),
                column![
                    title,
                    text(details.join("  \u{00b7}  ")).font(MONO).size(11).color(theme::colors().text_dim),
                ].spacing(3).width(Fill),
            ]
            .spacing(12)
            .align_y(iced::Alignment::Center),
        ]
        .spacing(10);

        if !device.is_current {
            if self.settings_signout_device.as_deref() == Some(&device.device_id) {
                col = col.push(
                    row![
                        ui::input("Your password", &self.settings_signout_password, Message::SettingsSignOutPasswordChanged)
                            .on_submit(Message::SettingsSignOutConfirm)
                            .secure(true)
                            .width(Fill),
                        ui::btn("Sign out", Tone::Danger, Some(Message::SettingsSignOutConfirm)),
                    ]
                    .spacing(8)
                    .align_y(iced::Alignment::Center),
                );
            } else {
                let mut actions = vec![];
                if !device.is_verified {
                    actions.push(ui::btn("Verify", Tone::Secondary,
                        Some(Message::StartDeviceVerification(device.device_id.clone()))).into());
                }
                actions.push(ui::btn("Sign out", Tone::Ghost,
                    Some(Message::SettingsSignOutDevice(device.device_id.clone()))).into());
                col = col.push(ui::footer(actions));
            }
        }

        let current = device.is_current;
        container(col)
            .padding([12, 14])
            .width(Fill)
            .style(move |_: &Theme| container::Style {
                background: Some(theme::colors().bg_mid.into()),
                border: iced::Border {
                    color: if current { theme::with_alpha(theme::colors().accent, 0.5) } else { theme::colors().border },
                    width: 1.0,
                    radius: 8.0.into(),
                },
                ..Default::default()
            })
            .into()
    }

    fn view_pending_invites_modal(&self) -> Element<Message> {
        let count = self.invited_rooms.len();
        let subtitle = match count {
            0 => "You're all caught up".to_string(),
            1 => "1 invitation".to_string(),
            n => format!("{n} invitations"),
        };
        let mut col = column![ui::header("Invites", Some(subtitle))].spacing(ui::GAP);
        if self.invited_rooms.is_empty() {
            col = col.push(ui::empty_state(crate::icons::SVG_BELL, "No pending invites", None));
        } else {
            let mut list = column![].spacing(8);
            for room in &self.invited_rooms {
                list = list.push(ui::card(
                    row![
                        self.render_room_avatar(room.avatar_url.as_deref().filter(|_| self.media_previews.invite_avatars), &room.name, 36.0),
                        column![
                            text(&room.name).font(MONO).size(13).color(theme::colors().text_bright),
                            text(if room.is_direct { "Direct message" } else if room.is_space { "Space" } else { "Room" })
                                .font(MONO).size(11).color(theme::colors().text_dim),
                        ].spacing(2).width(Fill),
                        ui::btn("Decline", Tone::Ghost, Some(Message::RejectInvite(room.room_id.clone()))),
                        ui::btn("Accept", Tone::Primary, Some(Message::AcceptInvite(room.room_id.clone()))),
                    ]
                    .spacing(10)
                    .align_y(iced::Alignment::Center),
                ));
            }
            col = col.push(
                scrollable(list)
                    .height(Length::Shrink)
                    .direction(iced::widget::scrollable::Direction::Vertical(
                        iced::widget::scrollable::Scrollbar::new().width(4).scroller_width(4),
                    ))
                    .style(thin_scrollbar),
            );
            if count > 1 {
                col = col.push(if self.decline_all_armed {
                    ui::card(row![
                        text(format!("Decline all {count} invites?")).font(MONO).size(13).color(theme::colors().text_normal).width(Fill),
                        ui::btn("Cancel", Tone::Ghost, Some(Message::ArmDeclineAllInvites(false))),
                        ui::btn("Decline all", Tone::Danger, Some(Message::DeclineAllInvites)),
                    ].spacing(8).align_y(iced::Alignment::Center)).into()
                } else {
                    ui::footer(vec![
                        ui::btn("Decline all", Tone::Ghost, Some(Message::ArmDeclineAllInvites(true))).into(),
                        ui::btn("Accept all", Tone::Primary, Some(Message::AcceptAllInvites)).into(),
                    ])
                });
            }
        }
        col.width(460).into()
    }

    fn view_devtools_modal(&self) -> Element<Message> {
        let room_name = self.active_real_room()
            .and_then(|rid| self.rooms.iter().find(|r| r.room_id == rid))
            .map(|r| r.name.clone())
            .unwrap_or_else(|| "No room".into());

        let tab_bar = ui::tabs(vec![
            ("Room state".into(), Message::DevToolsTabChanged("state".into()), self.devtools_tab == "state"),
            ("Send event".into(), Message::DevToolsTabChanged("send".into()), self.devtools_tab == "send"),
        ]);

        let body: Element<Message> = if self.devtools_tab == "state" {
            if self.devtools_state_events.is_empty() {
                ui::empty_state(crate::icons::SVG_CLOCK, "Loading state events\u{2026}", None)
            } else {
                let mut col = column![].spacing(6);
                for (event_type, json) in &self.devtools_state_events {
                    let preview = if json.len() > 500 {
                        let mut end = 500;
                        while !json.is_char_boundary(end) { end -= 1; }
                        format!("{}\u{2026}", &json[..end])
                    } else {
                        json.clone()
                    };
                    col = col.push(
                        container(
                            column![
                                row![
                                    text(event_type).font(MONO).size(12).color(theme::colors().accent).width(Fill),
                                    ui::icon_only(crate::icons::SVG_COPY, "Copy JSON", Tone::Ghost,
                                        Some(Message::CopyMessageText(json.clone()))),
                                ].align_y(iced::Alignment::Center),
                                text(preview).font(MONO).size(11).color(theme::colors().code_text)
                                    .wrapping(iced::widget::text::Wrapping::WordOrGlyph),
                            ]
                            .spacing(4),
                        )
                        .padding([8, 10])
                        .width(Fill)
                        .style(|_: &Theme| container::Style {
                            background: Some(theme::colors().code_bg.into()),
                            border: iced::Border { color: theme::colors().border, width: 1.0, radius: 6.0.into() },
                            ..Default::default()
                        }),
                    );
                }
                scrollable(container(col).padding(iced::Padding { right: 8.0, ..Default::default() }))
                    .height(Fill)
                    .direction(iced::widget::scrollable::Direction::Vertical(
                        iced::widget::scrollable::Scrollbar::new().width(4).scroller_width(4),
                    ))
                    .style(thin_scrollbar)
                    .into()
            }
        } else {
            let mut form = column![
                ui::toggle_row("State event", "Send as room state instead of a timeline event",
                    self.devtools_is_state, |_| Message::DevToolsToggleIsState),
                ui::field("Event type", ui::input("m.room.message", &self.devtools_send_type, Message::DevToolsSendTypeChanged)),
            ]
            .spacing(ui::GAP);
            if self.devtools_is_state {
                form = form.push(ui::field("State key", ui::input("(empty)", &self.devtools_send_state_key, Message::DevToolsSendStateKeyChanged)));
            }
            let json_ok = serde_json::from_str::<serde_json::Value>(&self.devtools_send_body)
                .map(|v| v.is_object())
                .unwrap_or(false);
            form = form.push(ui::field("Content (JSON)", ui::input("{\"body\": \"hello\", \"msgtype\": \"m.text\"}", &self.devtools_send_body, Message::DevToolsSendBodyChanged)));
            if !self.devtools_send_body.trim().is_empty() && !json_ok {
                form = form.push(text("Content must be a JSON object").font(MONO).size(12).color(theme::colors().red));
            }
            form = form.push(ui::footer(vec![
                ui::btn("Send", Tone::Primary,
                    (json_ok && !self.devtools_send_type.trim().is_empty()).then_some(Message::DevToolsSend)).into(),
            ]));
            form.into()
        };

        column![
            ui::header("Developer tools", Some(room_name)),
            tab_bar,
            container(body).height(Length::Fixed(440.0)),
        ]
        .spacing(ui::GAP)
        .width(700)
        .into()
    }

    fn view_directory_modal(&self) -> Element<Message> {
        let servers = [("", "My server"), ("matrixrooms.info", "matrixrooms.info")];
        let source = ui::segmented(
            servers.iter().map(|(srv, label)| (
                label.to_string(),
                Some(Message::DirectoryServerChanged(srv.to_string())),
                self.directory_server == *srv,
            )).collect(),
        );

        let search_bar = row![
            ui::input("Search public rooms", &self.directory_search, Message::DirectorySearchChanged)
                .on_submit(Message::DirectorySearch)
                .width(Fill),
            ui::icon_btn(crate::icons::SVG_SEARCH, "Search", Tone::Primary, Some(Message::DirectorySearch)),
        ]
        .spacing(8)
        .align_y(iced::Alignment::Center);

        let mut results = column![].spacing(4);
        for room in &self.directory_results {
            let name = room.name.as_deref().unwrap_or("(unnamed)");
            let already_joined = self.rooms.iter().any(|r| r.room_id == room.room_id);
            let mut info = column![
                text(name).font(MONO).size(13).color(theme::colors().text_bright)
                    .wrapping(iced::widget::text::Wrapping::None),
            ]
            .spacing(2);
            if let Some(alias) = room.alias.as_deref().filter(|a| !a.is_empty()) {
                info = info.push(text(alias).font(MONO).size(11).color(theme::colors().text_dim));
            }
            if let Some(topic) = room.topic.as_deref() {
                let short: String = topic.replace('\n', " ").chars().take(120).collect();
                if !short.trim().is_empty() {
                    info = info.push(
                        text(short).font(MONO).size(11).color(theme::colors().text_muted)
                            .wrapping(iced::widget::text::Wrapping::WordOrGlyph),
                    );
                }
            }
            let members = room.num_members;
            let member_label = if members >= 1000 { format!("{:.1}k", members as f64 / 1000.0) } else { members.to_string() };
            let action: Element<Message> = if already_joined {
                ui::btn("Open", Tone::Ghost, Some(Message::Batch(vec![
                    Message::CloseModal,
                    Message::SelectRoom(room.room_id.clone()),
                ]))).into()
            } else {
                ui::btn("Join", Tone::Secondary, Some(Message::DirectoryJoinRoom(room.room_id.clone()))).into()
            };
            results = results.push(
                container(
                    row![
                        self.render_room_avatar(room.avatar_url.as_deref(), name, 40.0),
                        container(info).width(Fill),
                        column![
                            action,
                            row![
                                ui::icon(crate::icons::SVG_USERS, 11.0, theme::colors().text_dim),
                                text(member_label).font(MONO).size(11).color(theme::colors().text_dim),
                            ].spacing(4).align_y(iced::Alignment::Center),
                        ]
                        .spacing(6)
                        .align_x(iced::Alignment::End),
                    ]
                    .spacing(12)
                    .align_y(iced::Alignment::Center),
                )
                .padding([10, 12])
                .width(Fill)
                .style(|_: &Theme| container::Style {
                    background: Some(theme::colors().bg_mid.into()),
                    border: iced::Border { color: theme::colors().border, width: 1.0, radius: 8.0.into() },
                    ..Default::default()
                }),
            );
        }
        if self.directory_loading {
            results = results.push(ui::empty_state(crate::icons::SVG_CLOCK, "Searching\u{2026}", None));
        } else if self.directory_next_batch.is_some() {
            results = results.push(
                container(ui::btn("Load more", Tone::Ghost, Some(Message::DirectoryLoadMore)))
                    .width(Fill).center_x(Fill).padding([6, 0]),
            );
        } else if self.directory_results.is_empty() {
            results = results.push(ui::empty_state(crate::icons::SVG_GLOBE, "No rooms found",
                Some("Try another search or server.".into())));
        }

        let subtitle = if self.directory_results.is_empty() {
            "Find public rooms to join".to_string()
        } else {
            format!("{} rooms", self.directory_results.len())
        };
        column![
            ui::header("Explore rooms", Some(subtitle)),
            source,
            search_bar,
            scrollable(container(results).padding(iced::Padding { right: 8.0, ..Default::default() }))
                .height(440)
                .direction(iced::widget::scrollable::Direction::Vertical(
                    iced::widget::scrollable::Scrollbar::new().width(4).scroller_width(4),
                ))
                .style(thin_scrollbar),
        ]
        .spacing(ui::GAP)
        .width(620)
        .into()
    }

}

/// A space offered in Room Settings' "Add to space" picker.
#[derive(Debug, Clone)]
struct SpaceChoice {
    id: String,
    name: String,
}

impl PartialEq for SpaceChoice {
    fn eq(&self, other: &Self) -> bool {
        self.id == other.id
    }
}

impl std::fmt::Display for SpaceChoice {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.name)
    }
}
