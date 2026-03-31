use iced::widget::{
    button, column, container, mouse_area, rich_text, row, scrollable, stack,
    text, text_input, Space,
};
use iced::{Color, Element, Fill, Length, Theme};

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
        let msg_avatar: Element<'a, Message> = self.render_avatar(msg.avatar_url.as_deref(), 16.0);

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
            // Parse basic markdown in plain text bodies
            let spans = crate::html::plain_text_to_spans(&msg.body);
            rich_text(spans)
                .font(MONO)
                .size(15)
                .wrapping(iced::widget::text::Wrapping::WordOrGlyph)
                .width(Fill)
                .on_link_click(Message::OpenUrl)
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
}
