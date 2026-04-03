use iced::widget::{
    button, column, container, mouse_area, row, scrollable, stack,
    text, text_input, Space,
};
use iced::{Color, Element, Fill, Length, Theme};

use crate::selectable_text::selectable_rich_text;

use crate::icons::*;
use crate::message::Message;
use crate::state::FluxChat;
use crate::theme;
use crate::update::extract_urls;
use crate::view::{
    emoji_search_id, local_time_of_day, thin_scrollbar, EMOJI_CELL_SIZE, EMOJI_FREQ_COUNT,
    EMOJI_GRID_COLUMNS, EMOJI_PANEL_HEIGHT, MONO,
};

impl FluxChat {
    /// Render a single message row: [time] [nick] | [body]
    pub(crate) fn view_message<'a>(&'a self, msg: &'a fluxchat_core::MessageInfo) -> Element<'a, Message> {
        let fs = self.msg_font_size;
        // Timestamp
        let time_str = if self.timestamp_format == "off" {
            String::new()
        } else {
            // Convert Unix timestamp to local time using libc
            let local_secs = local_time_of_day(msg.timestamp);
            let h = local_secs / 3600;
            let m = (local_secs % 3600) / 60;
            if self.timestamp_format == "full" {
                let s = local_secs % 60;
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
                    .size(fs)
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
                .size(fs)
                .color(nick_col),
        )
        .align_x(iced::alignment::Horizontal::Right)
        .width(Length::Fixed(100.0));

        // Avatar
        let msg_avatar: Element<'a, Message> = self.render_avatar(msg.avatar_url.as_deref(), 16.0);

        // Separator — vertical line spanning full message height
        let make_sep = || -> Element<'a, Message> {
            container(Space::new())
                .width(1)
                .height(Fill)
                .style(|_: &Theme| container::Style {
                    background: Some(theme::TEXT_DIM.into()),
                    ..Default::default()
                })
                .into()
        };

        // Poll messages
        if let Some(ref poll) = msg.poll {
            let mut poll_col = column![
                text(&poll.question).font(MONO).size(fs).color(theme::TEXT_BRIGHT),
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

            return row![time, nick, msg_avatar, make_sep(), poll_col].spacing(6).width(Fill).align_y(iced::Alignment::Start).into();
        }

        // Build the content element — image, video, or text
        let mut body_element: Element<'a, Message> = if let Some(ref geo) = msg.geo_uri {
            // Location message — display as a location card
            let coords = geo.strip_prefix("geo:").unwrap_or(geo);
            let parts: Vec<&str> = coords.split(',').collect();
            let (lat, lon) = if parts.len() >= 2 {
                (parts[0].trim(), parts[1].split(';').next().unwrap_or("").trim())
            } else {
                (coords, "")
            };
            let map_url = format!("https://www.openstreetmap.org/?mlat={lat}&mlon={lon}#map=16/{lat}/{lon}");
            let description = if msg.body.starts_with("Location:") {
                None
            } else {
                Some(msg.body.clone())
            };
            let pin_handle = iced::widget::svg::Handle::from_memory(crate::icons::SVG_MAP_PIN);
            let mut card_col = column![
                row![
                    iced::widget::svg(pin_handle).width(20).height(20)
                        .style(|_, _| iced::widget::svg::Style { color: Some(theme::RED) }),
                    text(format!("{lat}, {lon}")).font(MONO).size(fs).color(theme::TEXT_BRIGHT),
                ].spacing(8).align_y(iced::Alignment::Center),
            ].spacing(4);
            if let Some(desc) = description {
                card_col = card_col.push(
                    text(desc).font(MONO).size(fs - 1.0).color(theme::TEXT_MUTED)
                );
            }
            card_col = card_col.push(
                text("Open in OpenStreetMap").font(MONO).size(12).color(theme::ACCENT)
            );
            button(
                container(card_col)
                    .padding([8, 12])
                    .width(Length::Fixed(300.0))
                    .style(|_: &Theme| container::Style {
                        background: Some(theme::BG_DARK.into()),
                        border: iced::Border {
                            color: theme::BORDER,
                            width: 1.0,
                            radius: 6.0.into(),
                        },
                        ..Default::default()
                    }),
            )
            .on_press(Message::OpenUrl(map_url))
            .padding(0)
            .style(|_, _| button::Style { background: None, ..Default::default() })
            .into()
        } else if msg.audio_mxc.is_some() {
            // Audio/voice message — display as playback card
            let total_ms = msg.duration_ms.unwrap_or(0);
            let is_playing = self.audio_playing.as_deref() == Some(&msg.event_id);

            // Compute progress from elapsed time
            let progress = if is_playing {
                if let Some(start) = self.audio_start_time {
                    let elapsed = start.elapsed().as_millis() as f32;
                    let total = self.audio_duration_ms.max(1) as f32;
                    (elapsed / total).min(1.0)
                } else {
                    0.0
                }
            } else {
                0.0
            };

            // Time display: elapsed if playing, total if not
            let display_ms = if is_playing {
                self.audio_start_time.map(|s| s.elapsed().as_millis() as u64).unwrap_or(0).min(total_ms)
            } else {
                total_ms
            };
            let display_secs = display_ms / 1000;
            let duration_str = format!("{}:{:02}", display_secs / 60, display_secs % 60);

            let label = if msg.is_voice || msg.body.contains("voice_message") { "Voice message" } else { &msg.body };

            // Play/stop icon
            let icon_svg = if is_playing {
                br#"<svg xmlns="http://www.w3.org/2000/svg" width="24" height="24" viewBox="0 0 24 24" fill="currentColor" stroke="none"><rect x="5" y="4" width="14" height="16" rx="1"/></svg>"#.as_slice()
            } else {
                br#"<svg xmlns="http://www.w3.org/2000/svg" width="24" height="24" viewBox="0 0 24 24" fill="currentColor" stroke="none"><polygon points="5,3 19,12 5,21"/></svg>"#.as_slice()
            };
            let icon_handle = iced::widget::svg::Handle::from_memory(icon_svg);

            let bar_width = 120.0_f32;
            let filled_width = (bar_width * progress).max(0.0);
            let play_event_id = msg.event_id.clone();
            let play_room_id = self.selected_room_id.clone().unwrap_or_default();
            let icon_color = if is_playing { theme::RED } else { theme::ACCENT };
            button(
                container(
                    row![
                        iced::widget::svg(icon_handle).width(16).height(16)
                            .style(move |_, _| iced::widget::svg::Style { color: Some(icon_color) }),
                        // Progress bar: filled portion + remaining
                        container(
                            row![
                                container(Space::new())
                                    .width(Length::Fixed(filled_width))
                                    .height(4)
                                    .style(|_: &Theme| container::Style {
                                        background: Some(theme::ACCENT.into()),
                                        border: iced::Border { radius: 2.0.into(), ..Default::default() },
                                        ..Default::default()
                                    }),
                                container(Space::new())
                                    .width(Length::Fixed(bar_width - filled_width))
                                    .height(4)
                                    .style(|_: &Theme| container::Style {
                                        background: Some(theme::BG_LIGHT.into()),
                                        border: iced::Border { radius: 2.0.into(), ..Default::default() },
                                        ..Default::default()
                                    }),
                            ].spacing(0),
                        ),
                        text(duration_str).font(MONO).size(12).color(theme::TEXT_DIM),
                        text(label).font(MONO).size(12).color(theme::TEXT_MUTED)
                            .wrapping(iced::widget::text::Wrapping::None),
                    ].spacing(8).align_y(iced::Alignment::Center),
                )
                .padding([6, 10])
                .style(|_: &Theme| container::Style {
                    background: Some(theme::BG_DARK.into()),
                    border: iced::Border { color: theme::BORDER, width: 1.0, radius: 6.0.into() },
                    ..Default::default()
                }),
            )
            .on_press(if is_playing {
                Message::StopAudio
            } else {
                Message::PlayAudio(play_event_id, play_room_id)
            })
            .padding(0)
            .style(|_, _| button::Style { background: None, ..Default::default() })
            .into()
        } else if let Some(ref mxc) = msg.image_mxc {
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
                    let label = if msg.body.trim().is_empty() { "Loading image..." } else { &msg.body };
                    text(format!("[image: {label}]"))
                        .font(MONO).size(fs).color(theme::TEXT_DIM)
                        .into()
                }
            } else {
                // Video messages
                text(format!("[video: {}]", msg.body))
                    .font(MONO).size(fs).color(theme::PURPLE).width(Fill)
                    .into()
            }
        } else if msg.is_video {
            text(format!("[video: {}]", msg.body))
                .font(MONO).size(fs).color(theme::PURPLE).width(Fill)
                .into()
        } else if let Some(ref formatted) = msg.formatted_body {
            let has_code_block = formatted.contains("<pre>");
            let is_expanded = self.expanded_code_blocks.contains(&msg.event_id);

            // Collapse code blocks: truncate <pre> content to ~5 lines if not expanded
            let display_html = if has_code_block && !is_expanded {
                collapse_code_blocks(formatted, 5)
            } else {
                formatted.clone()
            };

            let spans = crate::html::html_to_spans(&display_html);
            let body_el: Element<'a, Message> = selectable_rich_text(spans)
                .font(MONO)
                .size(fs)
                .wrapping(iced::widget::text::Wrapping::WordOrGlyph)
                .width(Fill)
                .on_link_click(Message::OpenUrl)
                .into();

            if has_code_block {
                let eid = msg.event_id.clone();
                let label = if is_expanded { "Show less" } else { "Show more" };
                column![
                    body_el,
                    button(text(label).font(MONO).size(11).color(theme::ACCENT))
                        .on_press(Message::ToggleCodeBlock(eid))
                        .padding([2, 8])
                        .style(|_, status| button::Style {
                            background: if matches!(status, button::Status::Hovered) {
                                Some(Color::from_rgba(1.0, 1.0, 1.0, 0.06).into())
                            } else { None },
                            ..Default::default()
                        }),
                ].spacing(2).width(Fill).into()
            } else {
                body_el
            }
        } else {
            // Parse basic markdown in plain text bodies
            let spans = crate::html::plain_text_to_spans(&msg.body);
            selectable_rich_text(spans)
                .font(MONO)
                .size(fs)
                .wrapping(iced::widget::text::Wrapping::WordOrGlyph)
                .width(Fill)
                .on_link_click(Message::OpenUrl)
                .into()
        };

        // Reply context — colored left bar with sender name + quoted text
        // If SDK didn't resolve the reply, try local message cache
        let (resolved_reply_sender, resolved_reply_body) = if msg.reply_to_body.is_some() {
            (msg.reply_to_sender.clone(), msg.reply_to_body.clone())
        } else if let Some(ref reply_id) = msg.reply_to_id {
            // Look up in local messages
            let room_id = self.selected_room_id.as_deref().unwrap_or("");
            let found = self.messages_by_room.get(room_id)
                .and_then(|msgs| msgs.iter().find(|m| m.event_id == *reply_id));
            if let Some(original) = found {
                (Some(original.sender_name.clone()), Some(original.body.chars().take(120).collect()))
            } else {
                (Some("...".into()), Some("Reply".into()))
            }
        } else {
            (None, None)
        };

        if let Some(reply_body) = resolved_reply_body {
            let reply_sender: String = resolved_reply_sender.unwrap_or_else(|| "?".into());
            let sender_color = theme::nick_color(&reply_sender);

            // Truncate long reply bodies
            let preview_text = if reply_body.len() > 100 {
                format!("{}...", &reply_body[..100])
            } else {
                reply_body
            };

            let jump_to_id = msg.reply_to_id.clone().unwrap_or_default();
            let reply_preview: Element<'a, Message> = button(
                container(
                    row![
                        text("\u{2503}").size(20).color(sender_color), // vertical bar character
                        column![
                            text(reply_sender).font(MONO).size(12).color(sender_color),
                            text(preview_text).font(MONO).size(12).color(theme::TEXT_DIM)
                                .wrapping(iced::widget::text::Wrapping::WordOrGlyph),
                        ].spacing(1),
                    ].spacing(4).align_y(iced::Alignment::Center)
                )
                .padding([2, 6])
                .style(|_: &Theme| container::Style {
                    background: Some(Color::from_rgba(1.0, 1.0, 1.0, 0.03).into()),
                    border: iced::Border { radius: 4.0.into(), ..Default::default() },
                    ..Default::default()
                })
            )
            .on_press(Message::JumpToEvent(jump_to_id))
            .padding(0)
            .style(|_, _| button::Style { background: None, ..Default::default() })
            .into();

            // Combine reply preview + body into the message body area
            let combined_body: Element<'a, Message> = column![
                reply_preview,
                body_element,
            ].spacing(4).width(Fill).into();

            // Replace body_element with combined version — fall through to normal rendering
            body_element = combined_body;
        }

        // Build message row + reactions + action buttons
        // Message row with hover action bar overlay
        let is_hovered = self.hovered_message.as_deref() == Some(&msg.event_id);
        let event_id = msg.event_id.clone();
        let eid_hover = msg.event_id.clone();

        let base_row: Element<'a, Message> = row![time, nick, msg_avatar, make_sep(), body_element]
            .spacing(6).width(Fill).align_y(iced::Alignment::Start).into();

        // Wrap in container with hover highlight
        let hover_bg = if is_hovered {
            Color::from_rgba(1.0, 1.0, 1.0, 0.03)
        } else {
            Color::TRANSPARENT
        };
        let base_row: Element<'a, Message> = container(base_row)
            .width(Fill)
            .padding([2, 0])
            .style(move |_: &Theme| container::Style {
                background: Some(hover_bg.into()),
                ..Default::default()
            })
            .into();

        let msg_row: Element<'a, Message> = if is_hovered {
            // Size action buttons to fit within the message row height
            let btn_size = (fs + 4.0).max(18.0);
            let icon_sz = btn_size - 6.0;
            let btn_pad = ((btn_size - icon_sz) / 2.0).floor() as u16;
            let make_msg_action = move |svg_bytes: &'static [u8], msg_action: Message| -> Element<'a, Message> {
                let handle = iced::widget::svg::Handle::from_memory(svg_bytes);
                button(
                    iced::widget::svg(handle)
                        .width(Length::Fixed(icon_sz)).height(Length::Fixed(icon_sz))
                        .content_fit(iced::ContentFit::Fill)
                        .style(|_, _| iced::widget::svg::Style { color: Some(theme::TEXT_MUTED) }),
                )
                .on_press(msg_action)
                .padding(btn_pad)
                .width(Length::Fixed(btn_size))
                .height(Length::Fixed(btn_size))
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
            // Reply in thread
            if let Some(room_id) = &self.selected_room_id {
                if !room_id.starts_with("thread::") {
                    // Use existing thread_root if this message is in a thread, otherwise start new thread on this message
                    let thread_root = msg.thread_root.as_deref().unwrap_or(&msg.event_id);
                    action_row = action_row.push(make_msg_action(SVG_THREAD, Message::OpenThread(
                        room_id.clone(),
                        thread_root.to_string(),
                    )));
                }
            }

            // Pin/unpin
            action_row = action_row.push(make_msg_action(SVG_PIN, Message::PinMessage(event_id.clone())));

            if is_own {
                action_row = action_row.push(make_msg_action(SVG_EDIT, Message::StartEdit(
                    event_id.clone(),
                    msg.body.clone(),
                )));
                action_row = action_row.push(make_msg_action(SVG_DELETE, Message::DeleteMessage(event_id.clone())));
            } else {
                // Report button (other people's messages only)
                let report_room = self.selected_room_id.clone().unwrap_or_default();
                let report_room = if report_room.starts_with("thread::") {
                    report_room.splitn(3, "::").nth(1).unwrap_or(&report_room).to_string()
                } else {
                    report_room
                };
                action_row = action_row.push(make_msg_action(SVG_FLAG, Message::OpenModal(
                    crate::state::Modal::Report { event_id: event_id.clone(), room_id: report_room },
                )));
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

        // URL preview cards (max 2 unless expanded)
        let indent = time_width + 100.0 + 22.0 + 18.0;
        let urls = extract_urls(&msg.body);
        let all_previews: Vec<&fluxchat_core::UrlPreview> = urls
            .iter()
            .filter_map(|url| self.url_previews.get(url))
            .collect();
        let is_expanded = self.expanded_previews.contains(&msg.event_id);
        let max_show = if is_expanded { all_previews.len() } else { 2 };
        let extra_count = all_previews.len().saturating_sub(max_show);
        let mut preview_elements: Vec<Element<'a, Message>> = all_previews
            .into_iter()
            .take(max_show)
            .map(|preview| self.view_url_preview(preview, indent))
            .collect();
        if extra_count > 0 {
            let eid = msg.event_id.clone();
            preview_elements.push(
                row![
                    Space::new().width(Length::Fixed(indent)),
                    button(
                        text(format!("Show {extra_count} more preview{}", if extra_count == 1 { "" } else { "s" }))
                            .font(MONO).size(11).color(theme::ACCENT)
                    )
                    .on_press(Message::TogglePreviewExpand(eid))
                    .padding([2, 0])
                    .style(|_, _| button::Style { background: None, text_color: theme::ACCENT, ..Default::default() }),
                ].into()
            );
        } else if is_expanded {
            let eid = msg.event_id.clone();
            preview_elements.push(
                row![
                    Space::new().width(Length::Fixed(indent)),
                    button(
                        text("Show less").font(MONO).size(11).color(theme::TEXT_DIM)
                    )
                    .on_press(Message::TogglePreviewExpand(eid))
                    .padding([2, 0])
                    .style(|_, _| button::Style { background: None, text_color: theme::TEXT_DIM, ..Default::default() }),
                ].into()
            );
        }

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

            // Build tooltip with user names
            let room_members = self.selected_room_id.as_ref()
                .and_then(|rid| self.members_by_room.get(rid));
            let tooltip_text: String = reaction.users.iter()
                .map(|uid| {
                    room_members
                        .and_then(|m| m.iter().find(|m| m.user_id == *uid))
                        .map(|m| m.display_name.as_str())
                        .unwrap_or_else(|| uid.split(':').next().unwrap_or(uid).trim_start_matches('@'))
                        .to_string()
                })
                .collect::<Vec<_>>()
                .join(", ");

            let reaction_btn = button(
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
                });

            reactions_row = reactions_row.push(
                iced::widget::tooltip(
                    reaction_btn,
                    text(tooltip_text).font(MONO).size(11),
                    iced::widget::tooltip::Position::Top,
                )
                .style(|_: &Theme| container::Style {
                    background: Some(theme::BG_DARK.into()),
                    border: iced::Border {
                        color: theme::BORDER,
                        width: 1.0,
                        radius: 4.0.into(),
                    },
                    ..Default::default()
                })
                .padding(6),
            );
        }

        let mut col = column![msg_row].spacing(2).width(Fill);
        for el in preview_elements {
            col = col.push(el);
        }
        col = col.push(reactions_row);

        // Thread indicator rendered in view.rs message loop (using derived counts)

        // Highlight searched-to message with a tinted background
        if self.highlight_event_id.as_deref() == Some(&msg.event_id) {
            container(col)
                .width(Fill)
                .style(|_: &Theme| container::Style {
                    background: Some(Color::from_rgba(0.23, 0.51, 0.96, 0.15).into()),
                    ..Default::default()
                })
                .into()
        } else {
            col.into()
        }
    }

    /// Render the emoji picker panel.
    pub(crate) fn view_emoji_picker(&self) -> Element<Message> {
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
                .style(theme::button_transparent),
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

    pub(crate) fn view_url_preview<'a>(&self, preview: &'a fluxchat_core::UrlPreview, indent: f32) -> Element<'a, Message> {
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

        let mut card_row = row![].spacing(8);

        // OG image thumbnail (on the left)
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

        card_row = card_row.push(container(content_col).width(Fill).clip(true));

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
}

/// Truncate code inside `<pre>` blocks to `max_lines` lines.
fn collapse_code_blocks(html: &str, max_lines: usize) -> String {
    let mut result = String::new();
    let mut remaining = html;

    while let Some(pre_start) = remaining.find("<pre>").or_else(|| remaining.find("<pre><code")) {
        // Copy everything before <pre>
        result.push_str(&remaining[..pre_start]);

        // Find the opening tag end
        let tag_and_rest = &remaining[pre_start..];
        let content_start = tag_and_rest.find('>').map(|i| i + 1).unwrap_or(0);

        // Handle nested <code> tag
        let after_pre = &tag_and_rest[content_start..];
        let (code_start_offset, end_marker) = if after_pre.starts_with("<code") {
            let inner_end = after_pre.find('>').map(|i| i + 1).unwrap_or(0);
            (content_start + inner_end, "</code></pre>")
        } else {
            (content_start, "</pre>")
        };

        let code_region = &tag_and_rest[code_start_offset..];
        if let Some(end_pos) = code_region.find(end_marker) {
            let full_code = &code_region[..end_pos];
            let lines: Vec<&str> = full_code.split('\n').collect();

            // Copy the opening tags
            result.push_str(&tag_and_rest[..code_start_offset]);

            if lines.len() > max_lines {
                // Truncate
                result.push_str(&lines[..max_lines].join("\n"));
                result.push_str(&format!("\n... ({} more lines)", lines.len() - max_lines));
            } else {
                result.push_str(full_code);
            }

            result.push_str(end_marker);
            remaining = &code_region[end_pos + end_marker.len()..];
        } else {
            // No closing tag found, copy as-is
            result.push_str(tag_and_rest);
            break;
        }
    }
    result.push_str(remaining);
    result
}
