use iced::widget::{button, column, container, row, scrollable, text, text_input, Space};
use iced::{Color, Element, Fill, Length, Theme};

use crate::icons::*;
use crate::message::Message;
use crate::state::FluxChat;
use crate::theme;
use crate::view::{gif_search_id, input_id, thin_scrollbar, MONO};

impl FluxChat {
    /// Renders everything below the messages scrollable: typing indicator,
    /// reply/edit preview, GIF panel, emoji picker, upload preview, help text,
    /// command/mention autocomplete popups, and the input bar with formatting toolbar.
    pub(crate) fn view_input_area(&self, room_id: &str) -> Vec<Element<Message>> {
        let mut elements: Vec<Element<Message>> = Vec::new();

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
                elements.push(
                    container(
                        text(typing_text)
                            .font(MONO)
                            .size(13)
                            .color(theme::TEXT_DIM),
                    )
                    .padding([2, 12])
                    .into(),
                );
            }
        }

        // Reply/edit preview bar
        if let Some((_, sender, body)) = &self.reply_to {
            elements.push(
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
                })
                .into(),
            );
        } else if let Some((_, _)) = &self.edit_event {
            elements.push(
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
                })
                .into(),
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
                        .style(theme::button_transparent),
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
            } else if self.gif_results.is_empty() && self.gif_searched {
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

            elements.push(
                container(gif_col).padding([8, 12]).width(Fill)
                    .style(theme::container_panel)
                    .into(),
            );
        }

        // Location picker panel
        if self.location_open {
            let loc_col = column![
                row![
                    text("SHARE LOCATION").font(MONO).size(12).color(theme::TEXT_DIM),
                    Space::new().width(Fill),
                    button(text("\u{00d7}").font(MONO).size(16).color(theme::TEXT_DIM))
                        .on_press(Message::ToggleLocationPicker)
                        .padding([0, 6])
                        .style(theme::button_transparent),
                ].align_y(iced::Alignment::Center),
                row![
                    text("Lat").font(MONO).size(12).color(theme::TEXT_DIM),
                    text_input("40.80, -96.75 or paste pair", &self.location_lat)
                        .on_input(Message::LocationLatChanged)
                        .on_submit(Message::LocationSubmit)
                        .font(MONO).size(14).padding(4).width(Length::Fixed(200.0)),
                    text("Lon").font(MONO).size(12).color(theme::TEXT_DIM),
                    text_input("-96.75", &self.location_lon)
                        .on_input(Message::LocationLonChanged)
                        .on_submit(Message::LocationSubmit)
                        .font(MONO).size(14).padding(4).width(Length::Fixed(120.0)),
                    button(
                        text("Send").font(MONO).size(13).color(Color::WHITE)
                    )
                    .on_press(Message::LocationSubmit)
                    .padding([4, 12])
                    .style(|_, status| button::Style {
                        background: Some(if matches!(status, button::Status::Hovered) {
                            Color::from_rgba(0.23, 0.51, 0.96, 0.9)
                        } else {
                            Color::from_rgba(0.23, 0.51, 0.96, 0.7)
                        }.into()),
                        border: iced::Border { radius: 4.0.into(), ..Default::default() },
                        ..Default::default()
                    }),
                ].spacing(8).align_y(iced::Alignment::Center),
            ].spacing(8);
            elements.push(
                container(loc_col).padding([8, 12]).width(Fill)
                    .style(theme::container_panel)
                    .into(),
            );
        }

        // Emoji picker panel
        if self.emoji_open {
            elements.push(self.view_emoji_picker());
        }

        // Upload preview bar
        if let Some((ref filename, _, _)) = self.upload_pending {
            elements.push(
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
                })
                .into(),
            );
        }

        // Help text (command feedback)
        if let Some(ref help) = self.help_text {
            elements.push(
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
                })
                .into(),
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
            elements.push(
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
                .padding([4, 8])
                .into(),
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
            elements.push(
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
                .padding([4, 8])
                .into(),
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

        let loc_color = if self.location_open { theme::ACCENT } else { theme::TEXT_DIM };
        let loc_btn: Element<Message> = {
            let handle = iced::widget::svg::Handle::from_memory(crate::icons::SVG_MAP_PIN);
            iced::widget::tooltip(
                button(
                    iced::widget::svg(handle)
                        .width(16).height(16)
                        .style(move |_, _| iced::widget::svg::Style { color: Some(loc_color) }),
                )
                .on_press(Message::ToggleLocationPicker)
                .padding([2, 6])
                .style(|_, status| button::Style {
                    background: if matches!(status, button::Status::Hovered) {
                        Some(Color::from_rgba(1.0, 1.0, 1.0, 0.08).into())
                    } else {
                        None
                    },
                    border: iced::Border { radius: 4.0.into(), ..Default::default() },
                    ..Default::default()
                }),
                text("Location").font(MONO).size(12),
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

        // Mic button
        let mic_color = if self.voice_recording { theme::RED } else { theme::TEXT_DIM };
        let mic_btn: Element<Message> = {
            let handle = iced::widget::svg::Handle::from_memory(crate::icons::SVG_MIC);
            button(
                iced::widget::svg(handle)
                    .width(16).height(16)
                    .style(move |_, _| iced::widget::svg::Style { color: Some(mic_color) }),
            )
            .on_press(if self.voice_recording { Message::StopVoiceRecording } else { Message::StartVoiceRecording })
            .padding([2, 6])
            .style(|_, status| button::Style {
                background: if matches!(status, button::Status::Hovered) {
                    Some(Color::from_rgba(1.0, 1.0, 1.0, 0.08).into())
                } else {
                    None
                },
                border: iced::Border { radius: 4.0.into(), ..Default::default() },
                ..Default::default()
            })
            .into()
        };

        let input_row: Element<Message> = if let Some((_, duration_ms)) = &self.voice_ready {
            // Review bar: play preview + duration + discard/send
            let secs = duration_ms / 1000;
            let duration_str = format!("{}:{:02}", secs / 60, secs % 60);
            let play_handle = iced::widget::svg::Handle::from_memory(
                br#"<svg xmlns="http://www.w3.org/2000/svg" width="24" height="24" viewBox="0 0 24 24" fill="currentColor" stroke="none"><polygon points="5,3 19,12 5,21"/></svg>"#.as_slice(),
            );
            // Write to temp and play preview
            row![
                button(
                    iced::widget::svg(play_handle).width(16).height(16)
                        .style(|_, _| iced::widget::svg::Style { color: Some(theme::ACCENT) }),
                )
                .on_press(Message::PreviewVoiceMessage)
                .padding([4, 6])
                .style(|_, status| button::Style {
                    background: if matches!(status, button::Status::Hovered) {
                        Some(Color::from_rgba(1.0, 1.0, 1.0, 0.08).into())
                    } else { None },
                    border: iced::Border { radius: 4.0.into(), ..Default::default() },
                    ..Default::default()
                }),
                text(duration_str).font(MONO).size(16).color(theme::TEXT_BRIGHT),
                text("Voice message ready").font(MONO).size(14).color(theme::TEXT_DIM),
                Space::new().width(Fill),
                button(text("Discard").font(MONO).size(13).color(theme::TEXT_DIM))
                    .on_press(Message::DiscardVoiceMessage)
                    .padding([4, 10])
                    .style(|_, status| button::Style {
                        background: if matches!(status, button::Status::Hovered) {
                            Some(Color::from_rgba(1.0, 1.0, 1.0, 0.06).into())
                        } else { None },
                        border: iced::Border { radius: 4.0.into(), ..Default::default() },
                        ..Default::default()
                    }),
                button(text("Send").font(MONO).size(13).color(Color::WHITE))
                    .on_press(Message::SendVoiceMessage)
                    .padding([4, 12])
                    .style(|_, status| button::Style {
                        background: Some(if matches!(status, button::Status::Hovered) {
                            Color::from_rgba(0.23, 0.51, 0.96, 0.9)
                        } else {
                            Color::from_rgba(0.23, 0.51, 0.96, 0.7)
                        }.into()),
                        border: iced::Border { radius: 4.0.into(), ..Default::default() },
                        ..Default::default()
                    }),
            ]
            .spacing(10)
            .align_y(iced::Alignment::Center)
            .into()
        } else if self.voice_recording {
            // Recording bar: stop button + red dot + timer + cancel/send buttons
            let elapsed = self.voice_record_start
                .map(|s| s.elapsed().as_secs())
                .unwrap_or(0);
            let timer_str = format!("{}:{:02}", elapsed / 60, elapsed % 60);
            let stop_icon = iced::widget::svg::Handle::from_memory(
                br#"<svg xmlns="http://www.w3.org/2000/svg" width="24" height="24" viewBox="0 0 24 24" fill="currentColor" stroke="none"><rect x="5" y="4" width="14" height="16" rx="1"/></svg>"#.as_slice(),
            );
            row![
                // Red recording dot
                container(Space::new()).width(8).height(8)
                    .style(|_: &Theme| container::Style {
                        background: Some(theme::RED.into()),
                        border: iced::Border { radius: 4.0.into(), ..Default::default() },
                        ..Default::default()
                    }),
                text(timer_str).font(MONO).size(16).color(theme::RED),
                text("Recording...").font(MONO).size(14).color(theme::TEXT_DIM),
                Space::new().width(Fill),
                button(text("Cancel").font(MONO).size(13).color(theme::TEXT_DIM))
                    .on_press(Message::CancelVoiceRecording)
                    .padding([4, 10])
                    .style(|_, status| button::Style {
                        background: if matches!(status, button::Status::Hovered) {
                            Some(Color::from_rgba(1.0, 1.0, 1.0, 0.06).into())
                        } else { None },
                        border: iced::Border { radius: 4.0.into(), ..Default::default() },
                        ..Default::default()
                    }),
                button(
                    iced::widget::svg(stop_icon).width(18).height(18)
                        .style(|_, _| iced::widget::svg::Style { color: Some(theme::RED) }),
                )
                .on_press(Message::StopVoiceRecording)
                .padding([4, 6])
                .style(|_, status| button::Style {
                    background: if matches!(status, button::Status::Hovered) {
                        Some(Color::from_rgba(1.0, 0.0, 0.0, 0.15).into())
                    } else { None },
                    border: iced::Border { radius: 4.0.into(), ..Default::default() },
                    ..Default::default()
                }),
            ]
            .spacing(10)
            .align_y(iced::Alignment::Center)
            .into()
        } else {
            row![
                attach_btn,
                prompt,
                input,
                mic_btn,
                loc_btn,
                emoji_btn,
            ]
            .spacing(6)
            .align_y(iced::Alignment::Center)
            .into()
        };

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

        elements.push(
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
                })
                .into(),
        );

        elements
    }
}
