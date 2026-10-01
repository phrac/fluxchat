use iced::widget::{
    button, column, container, mouse_area, row, scrollable, stack,
    text, Space,
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
    EMOJI_GRID_COLUMNS, EMOJI_PANEL_HEIGHT, EMOJI_PANEL_WIDTH,
};

/// Selection-widget part index for a media caption (after any body parts).
const CAPTION_PART_INDEX: usize = 10_000;

/// Size multiplier for emoji-only messages.
const BIG_EMOJI_SCALE: f32 = 2.2;
/// Lines a collapsed code block shows before "Show more".
const CODE_BLOCK_LINES: usize = 5;

/// Gap between the columns of a message row (time, nick, avatar, separator, text).
pub(crate) const MSG_COL_GAP: f32 = 6.0;
/// Messages from the same sender within this many seconds are grouped.
pub(crate) const MSG_GROUP_SECS: u64 = 300;

/// Column geometry of a message row (see `FluxChat::msg_columns`).
pub(crate) struct MsgColumns {
    pub time_size: f32,
    pub time_w: f32,
    pub nick_w: f32,
    pub avatar: f32,
}

/// Emoji picker tabs: (icon, name). Index 0 is "frequently used"; 1..=9
/// map to `emojis::Group` in order.
const EMOJI_CATEGORIES: [(&[u8], &str); 10] = [
    (SVG_CLOCK, "Frequently used"),
    (SVG_SMILE, "Smileys & emotion"),
    (SVG_HAND, "People & body"),
    (SVG_LEAF, "Animals & nature"),
    (SVG_COFFEE, "Food & drink"),
    (SVG_PLANE, "Travel & places"),
    (SVG_TROPHY, "Activities"),
    (SVG_LIGHTBULB, "Objects"),
    (SVG_SHAPES, "Symbols"),
    (SVG_FLAG, "Flags"),
];

impl FluxChat {
    /// Compute selection range for a specific widget, based on global selection state.
    fn selection_range_for(&self, widget_id: &crate::selectable_text::SelectionWidgetId) -> crate::selectable_text::SelectionRange {
        use crate::selectable_text::{SelectionRange, GlobalSelectionAnchor};

        let anchor = match &self.selection.anchor {
            Some(a) => a,
            None => return SelectionRange::None,
        };
        let cursor = match &self.selection.cursor {
            Some(c) => c,
            None => return SelectionRange::None,
        };

        // Get message list for position ordering
        let room_id = match &self.selected_room_id {
            Some(r) => r,
            None => return SelectionRange::None,
        };
        let messages = match self.messages_by_room.get(room_id) {
            Some(m) => m,
            None => return SelectionRange::None,
        };

        // Find message indices
        let find_idx = |a: &GlobalSelectionAnchor| -> Option<usize> {
            messages.iter().position(|m| m.event_id == a.widget_id.event_id)
        };
        let anchor_idx = match find_idx(anchor) { Some(i) => i, None => return SelectionRange::None };
        let cursor_idx = match find_idx(cursor) { Some(i) => i, None => return SelectionRange::None };

        // Normalize direction (start <= end)
        let (start, end, start_idx, end_idx) = if anchor_idx < cursor_idx
            || (anchor_idx == cursor_idx && (anchor.widget_id.part_index < cursor.widget_id.part_index
                || (anchor.widget_id.part_index == cursor.widget_id.part_index
                    && (anchor.line < cursor.line || (anchor.line == cursor.line && anchor.index <= cursor.index)))))
        {
            (anchor, cursor, anchor_idx, cursor_idx)
        } else {
            (cursor, anchor, cursor_idx, anchor_idx)
        };

        let widget_msg_idx = match messages.iter().position(|m| m.event_id == widget_id.event_id) {
            Some(i) => i,
            None => return SelectionRange::None,
        };

        // Check if widget is in range
        if widget_msg_idx < start_idx || widget_msg_idx > end_idx {
            return SelectionRange::None;
        }

        let is_start_widget = widget_id == &start.widget_id;
        let is_end_widget = widget_id == &end.widget_id;

        // Same message: check part_index ordering
        if widget_msg_idx == start_idx && widget_msg_idx == end_idx {
            if widget_id.part_index < start.widget_id.part_index || widget_id.part_index > end.widget_id.part_index {
                return SelectionRange::None;
            }
            if is_start_widget && is_end_widget {
                return SelectionRange::Partial {
                    start: (start.line, start.index),
                    end: (end.line, end.index),
                };
            }
            if widget_id.part_index == start.widget_id.part_index {
                return SelectionRange::Partial {
                    start: (start.line, start.index),
                    end: (usize::MAX, usize::MAX),
                };
            }
            if widget_id.part_index == end.widget_id.part_index {
                return SelectionRange::Partial {
                    start: (0, 0),
                    end: (end.line, end.index),
                };
            }
            return SelectionRange::Full;
        }

        if is_start_widget {
            SelectionRange::Partial {
                start: (start.line, start.index),
                end: (usize::MAX, usize::MAX),
            }
        } else if is_end_widget {
            SelectionRange::Partial {
                start: (0, 0),
                end: (end.line, end.index),
            }
        } else {
            SelectionRange::Full
        }
    }

    /// Render a single message row: [time] [nick] | [body]
    /// Whether a message should be highlighted: it mentions us (name,
    /// username, ID or pill), @room, or one of our keywords (server push-rule
    /// keywords plus local `/highlight` words). Never our own messages.
    pub(crate) fn is_highlight(&self, msg: &fluxchat_core::MessageInfo) -> bool {
        if msg.sender == self.user_id || msg.row_kind != fluxchat_core::RowKind::Message || msg.body.is_empty() {
            return false;
        }
        let body = msg.body.to_lowercase();
        let html = msg.formatted_body.as_deref().unwrap_or("");
        let rules = self.notify_rules.as_ref();
        let mention_me = rules.is_none_or(|r| r.mention_me);
        if mention_me {
            let localpart = self.user_id.trim_start_matches('@').split(':').next().unwrap_or("").to_lowercase();
            let display = self.display_name.as_deref().unwrap_or("").to_lowercase();
            let mentions = |needle: &str| !needle.is_empty() && contains_word(&body, needle);
            if mentions(&localpart) || mentions(&display) || html.contains(&self.user_id)
                || html.contains(&self.user_id.replace('@', "%40"))
            {
                return true;
            }
        }
        if rules.is_none_or(|r| r.mention_room) && contains_word(&body, "@room") {
            return true;
        }
        let server_keywords = rules.map(|r| r.keywords.as_slice()).unwrap_or(&[]);
        server_keywords.iter().chain(self.highlight_words.iter())
            .any(|k| !k.trim().is_empty() && contains_word(&body, &k.to_lowercase()))
    }

    /// Column geometry of a message row, scaled to the message font size.
    pub(crate) fn msg_columns(&self) -> MsgColumns {
        let fs = self.msg_font_size;
        let time_size = (fs * 0.85).max(11.0);
        let time_chars = match self.timestamp_format.as_str() {
            "off" => 0.0,
            "full" => 8.0,
            _ => 5.0,
        } + if self.prefs.twelve_hour && self.timestamp_format != "off" { 3.0 } else { 0.0 };
        MsgColumns {
            time_size,
            // Digits are ~0.6em in most fonts; a little slack avoids clipping.
            time_w: if time_chars == 0.0 { 0.0 } else { (time_chars * time_size * 0.62 + 6.0).ceil() },
            nick_w: (fs * 7.4).max(96.0).round(),
            // Compact mode: no avatar column.
            avatar: if self.compact_mode { 0.0 } else { (fs + 4.0).round() },
        }
    }

    /// Distance from the row's left edge to the message text.
    pub(crate) fn msg_content_indent(&self) -> f32 {
        let c = self.msg_columns();
        // time + nick + avatar + 1px separator, with a 6px gap after each.
        c.time_w + c.nick_w + c.avatar + 1.0 + 4.0 * MSG_COL_GAP
    }

    /// One message row. `continuation`: same sender as the row above within a
    /// few minutes — the nick is dimmed and the avatar left out so runs of
    /// messages read as one block.
    /// One timeline row. `view_key` is the view it's shown in (the main view
    /// or a side pane), used for every room-scoped lookup and action.
    pub(crate) fn view_message<'a>(&'a self, view_key: &'a str, msg: &'a fluxchat_core::MessageInfo, continuation: bool) -> Element<'a, Message> {
        let fs = self.msg_font_size;
        let cols = self.msg_columns();
        // Timestamp
        let time_str = if self.timestamp_format == "off" {
            String::new()
        } else {
            // Convert Unix timestamp to local time using libc
            format_clock(
                local_time_of_day(msg.timestamp),
                self.timestamp_format == "full",
                self.prefs.twelve_hour,
            )
        };

        // Date divider
        if msg.is_date_divider() {
            let line = || iced::widget::rule::horizontal(1).style(|_: &Theme| iced::widget::rule::Style {
                color: theme::colors().border,
                radius: 0.0.into(),
                fill_mode: iced::widget::rule::FillMode::Full,
                snap: true,
            });
            return container(
                row![
                    line(),
                    text(local_day_label(msg.timestamp)).font(self.sidebar_font()).size(12).color(theme::colors().text_muted),
                    line(),
                ].spacing(10).align_y(iced::Alignment::Center)
            ).padding([8, 8]).width(Fill).into();
        }

        // System messages (join/part/state changes)
        if msg.is_system() {
            return row![
                text(time_str)
                    .font(self.msg_font())
                    .size(cols.time_size)
                    .color(theme::colors().text_dim)
                    .width(Length::Fixed(cols.time_w)),
                Space::new().width(Length::Fixed(self.msg_content_indent() - cols.time_w - MSG_COL_GAP)),
                text(&msg.body)
                    .font(self.msg_font())
                    .size((fs - 1.0).max(11.0))
                    .color(theme::colors().text_dim)
                    .wrapping(iced::widget::text::Wrapping::WordOrGlyph)
                    .width(Fill),
            ]
            .align_y(iced::Alignment::Center)
            .padding([1, 0])
            .width(Fill)
            .into();
        }

        // Emote messages
        if msg.is_emote {
            let nick_col = theme::nick_color(&msg.sender_name);
            return row![
                text(time_str)
                    .font(self.msg_font())
                    .size(cols.time_size)
                    .color(theme::colors().text_dim)
                    .width(Length::Fixed(cols.time_w)),
                container(text("*").font(self.msg_font()).size(fs).color(nick_col))
                    .align_x(iced::alignment::Horizontal::Right)
                    .width(Length::Fixed(cols.nick_w + MSG_COL_GAP + cols.avatar)),
                Space::new().width(Length::Fixed(1.0)),
                text(format!("{} {}", msg.sender_name, msg.body))
                    .font(self.msg_font())
                    .size(fs)
                    .color(nick_col)
                    .wrapping(iced::widget::text::Wrapping::WordOrGlyph)
                    .width(Fill),
            ]
            .spacing(6)
            .width(Fill)
            .into();
        }

        let time = text(time_str)
            .font(self.msg_font())
            .size(cols.time_size)
            .color(theme::colors().text_dim)
            .width(Length::Fixed(cols.time_w));

        // Nick (right-aligned, truncated to the column; dimmed on continuations)
        let nick_col = theme::nick_color(&msg.sender_name);
        let nick = container(
            text(&msg.sender_name)
                .font(self.msg_font())
                .size(fs)
                .color(if continuation { theme::with_alpha(nick_col, 0.35) } else { nick_col })
                .wrapping(iced::widget::text::Wrapping::None),
        )
        .align_x(iced::alignment::Horizontal::Right)
        .width(Length::Fixed(cols.nick_w))
        .clip(true);

        // Avatar (initial on a colour tint when they have none); blank on continuations
        let msg_avatar: Element<'a, Message> = if continuation || self.compact_mode {
            Space::new().width(cols.avatar).height(1).into()
        } else {
            self.render_avatar_with_initial(msg.avatar_url.as_deref(), &msg.sender_name, cols.avatar)
        };

        // Separator — vertical line spanning full message height
        let make_sep = || -> Element<'a, Message> {
            container(Space::new())
                .width(1)
                .height(Fill)
                .style(|_: &Theme| container::Style {
                    background: Some(theme::with_alpha(theme::colors().text_dim, 0.45).into()),
                    ..Default::default()
                })
                .into()
        };

        // Deleted message placeholder
        if msg.row_kind == fluxchat_core::RowKind::Redacted {
            let placeholder = row![
                crate::ui::icon(SVG_TRASH, (fs - 2.0).max(11.0), theme::colors().text_dim),
                text("Message deleted").font(self.msg_font()).size((fs - 1.0).max(11.0))
                    .color(theme::colors().text_dim),
            ]
            .spacing(6)
            .align_y(iced::Alignment::Center);
            return container(
                row![time, nick, msg_avatar, make_sep(), placeholder]
                    .spacing(MSG_COL_GAP).width(Fill).align_y(iced::Alignment::Start),
            )
            .padding(iced::Padding { top: if continuation { 1.0 } else { 6.0 }, right: 0.0, bottom: 1.0, left: 0.0 })
            .width(Fill)
            .into();
        }

        // Poll messages
        if let Some(ref poll) = msg.poll {
            let mut poll_col = column![
                text(&poll.question).font(self.msg_font()).size(fs).color(theme::colors().text_bright),
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
                let label_color = if is_my_vote { theme::colors().accent } else { theme::colors().text_normal };

                poll_col = poll_col.push(
                    row![
                        text(format!("{answer_text} ({votes})"))
                            .font(self.msg_font())
                            .size(14)
                            .color(label_color),
                        text(format!("{pct}%"))
                            .font(self.msg_font())
                            .size(13)
                            .color(theme::colors().text_dim),
                    ]
                    .spacing(8),
                );
            }

            if poll.is_ended {
                poll_col = poll_col.push(
                    text("Poll ended")
                        .font(self.msg_font())
                        .size(13)
                        .color(theme::colors().text_dim),
                );
            }

            return row![time, nick, msg_avatar, make_sep(), poll_col].spacing(MSG_COL_GAP).width(Fill).align_y(iced::Alignment::Start).into();
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
                        .style(|_, _| iced::widget::svg::Style { color: Some(theme::colors().red) }),
                    text(format!("{lat}, {lon}")).font(self.msg_font()).size(fs).color(theme::colors().text_bright),
                ].spacing(8).align_y(iced::Alignment::Center),
            ].spacing(4);
            if let Some(desc) = description {
                card_col = card_col.push(
                    text(desc).font(self.msg_font()).size(fs - 1.0).color(theme::colors().text_muted)
                );
            }
            card_col = card_col.push(
                text("Open in OpenStreetMap").font(self.msg_font()).size(12).color(theme::colors().accent)
            );
            button(
                container(card_col)
                    .padding([8, 12])
                    .width(Length::Fixed(300.0))
                    .style(|_: &Theme| container::Style {
                        background: Some(theme::colors().bg_dark.into()),
                        border: iced::Border {
                            color: theme::colors().border,
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
            let play_room_id = view_key.to_string();
            let icon_color = if is_playing { theme::colors().red } else { theme::colors().accent };
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
                                        background: Some(theme::colors().accent.into()),
                                        border: iced::Border { radius: 2.0.into(), ..Default::default() },
                                        ..Default::default()
                                    }),
                                container(Space::new())
                                    .width(Length::Fixed(bar_width - filled_width))
                                    .height(4)
                                    .style(|_: &Theme| container::Style {
                                        background: Some(theme::colors().bg_light.into()),
                                        border: iced::Border { radius: 2.0.into(), ..Default::default() },
                                        ..Default::default()
                                    }),
                            ].spacing(0),
                        ),
                        text(duration_str).font(self.msg_font()).size(12).color(theme::colors().text_dim),
                        text(label).font(self.msg_font()).size(12).color(theme::colors().text_muted)
                            .wrapping(iced::widget::text::Wrapping::None),
                    ].spacing(8).align_y(iced::Alignment::Center),
                )
                .padding([6, 10])
                .style(|_: &Theme| container::Style {
                    background: Some(theme::colors().bg_dark.into()),
                    border: iced::Border { color: theme::colors().border, width: 1.0, radius: 6.0.into() },
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
                if let Some(frames) = self.gif_frames_cache.get(mxc).filter(|_| self.prefs.autoplay_gifs) {
                    let img_w = msg.image_width.unwrap_or(320).min(400) as f32;
                    let mxc_click = mxc.clone();
                    mouse_area(
                        iced_gif::gif(frames.as_ref())
                            .width(img_w)
                            .content_fit(iced::ContentFit::ScaleDown)
                    )
                    .on_press(Message::OpenModal(crate::state::Modal::Lightbox { url: mxc_click }))
                    .interaction(iced::mouse::Interaction::ZoomIn)
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
                    .interaction(iced::mouse::Interaction::ZoomIn)
                    .into()
                } else {
                    let label = if msg.body.trim().is_empty() { "Loading image..." } else { &msg.body };
                    text(format!("[image: {label}]"))
                        .font(self.msg_font()).size(fs).color(theme::colors().text_dim)
                        .into()
                }
            } else {
                // Video messages
                text(format!("[video: {}]", msg.body))
                    .font(self.msg_font()).size(fs).color(theme::colors().purple).width(Fill)
                    .into()
            }
        } else if msg.is_video {
            let eid = msg.event_id.clone();
            let rid = view_key.to_string();
            let is_playing = self.video_playing_event.as_deref() == Some(&msg.event_id);

            if is_playing {
                if let Some(ref video) = self.video_player {
                    // Use actual video dimensions, accounting for possible rotation
                    let (raw_w, raw_h) = video.size();
                    let (raw_w, raw_h) = if raw_w > 0 && raw_h > 0 {
                        (raw_w as f32, raw_h as f32)
                    } else {
                        (480.0, 270.0)
                    };
                    // Fit within 480w x 500h, minimum 300w for readability
                    let scale = (480.0 / raw_w).min(500.0 / raw_h).min(1.0);
                    let vid_w = (raw_w * scale).max(300.0);
                    let vid_h = (raw_h * scale).max(200.0);

                    let duration = video.duration().as_secs_f64();
                    let paused = video.paused();

                    let play_pause_icon = if paused {
                        br#"<svg xmlns="http://www.w3.org/2000/svg" width="24" height="24" viewBox="0 0 24 24" fill="currentColor" stroke="none"><polygon points="5,3 19,12 5,21"/></svg>"#.as_slice()
                    } else {
                        br#"<svg xmlns="http://www.w3.org/2000/svg" width="24" height="24" viewBox="0 0 24 24" fill="currentColor" stroke="none"><rect x="6" y="4" width="4" height="16"/><rect x="14" y="4" width="4" height="16"/></svg>"#.as_slice()
                    };

                    let pos_mins = (self.video_position / 60.0) as u32;
                    let pos_secs = (self.video_position % 60.0) as u32;
                    let dur_mins = (duration / 60.0) as u32;
                    let dur_secs = (duration % 60.0) as u32;
                    let time_str = format!("{pos_mins}:{pos_secs:02} / {dur_mins}:{dur_secs:02}");

                    let player_widget: Element<Message> = iced_video_player::VideoPlayer::new(video)
                        .width(vid_w)
                        .height(vid_h)
                        .content_fit(iced::ContentFit::Contain)
                        .on_new_frame(Message::VideoNewFrame)
                        .on_end_of_stream(Message::VideoEndOfStream)
                        .into();

                    let controls = container(
                        row![
                            button(
                                iced::widget::svg(iced::widget::svg::Handle::from_memory(play_pause_icon))
                                    .width(18).height(18)
                                    .style(|_, _| iced::widget::svg::Style { color: Some(Color::WHITE) }),
                            )
                            .on_press(Message::VideoTogglePause)
                            .padding([4, 8])
                            .style(|_, _| button::Style { background: None, ..Default::default() }),
                            iced::widget::slider(0.0..=duration.max(0.001), self.video_position, Message::VideoSeek)
                                .on_release(Message::VideoSeekRelease)
                                .width(Fill)
                                .height(16),
                            text(time_str)
                                .size(11).color(Color::WHITE)
                                .font(self.msg_font()),
                            button(
                                iced::widget::svg(iced::widget::svg::Handle::from_memory(SVG_CLOSE))
                                    .width(16).height(16)
                                    .style(|_, _| iced::widget::svg::Style { color: Some(Color::WHITE) }),
                            )
                            .on_press(Message::VideoClose)
                            .padding([4, 8])
                            .style(|_, _| button::Style { background: None, ..Default::default() }),
                        ]
                        .spacing(6)
                        .align_y(iced::Alignment::Center),
                    )
                    .padding([4, 8])
                    .style(|_: &Theme| container::Style {
                        background: Some(Color::from_rgba(0.0, 0.0, 0.0, 0.7).into()),
                        ..Default::default()
                    });

                    column![
                        container(player_widget)
                            .style(|_: &Theme| container::Style {
                                background: Some(Color::BLACK.into()),
                                border: iced::Border { radius: 8.0.into(), ..Default::default() },
                                ..Default::default()
                            })
                            .clip(true),
                        controls,
                    ]
                    .width(vid_w)
                    .into()
                } else {
                    // Loading state
                    text(format!("[loading video: {}]", msg.body))
                        .font(self.msg_font()).size(fs).color(theme::colors().purple)
                        .into()
                }
            } else {
                // Thumbnail with play overlay (not currently playing)
                let thumbnail: Element<Message> = if let Some(mxc) = msg.image_mxc.as_ref() {
                    if let Some(handle) = self.image_cache.get(mxc) {
                        let img_w = msg.image_width.unwrap_or(320).min(400) as f32;
                        let img_h = if let (Some(w), Some(h)) = (msg.image_width, msg.image_height) {
                            (img_w * h as f32 / w as f32).min(300.0)
                        } else { 200.0 };
                        iced::widget::stack![
                            iced::widget::image(handle.clone())
                                .width(img_w).height(img_h)
                                .content_fit(iced::ContentFit::Cover),
                            // Play button overlay
                            container(
                                container(
                                    iced::widget::svg(iced::widget::svg::Handle::from_memory(
                                        br#"<svg xmlns="http://www.w3.org/2000/svg" width="24" height="24" viewBox="0 0 24 24" fill="currentColor" stroke="none"><polygon points="5,3 19,12 5,21"/></svg>"#.as_slice(),
                                    ))
                                    .width(32).height(32)
                                    .style(|_, _| iced::widget::svg::Style { color: Some(Color::WHITE) }),
                                )
                                .padding(12)
                                .style(|_: &Theme| container::Style {
                                    background: Some(Color::from_rgba(0.0, 0.0, 0.0, 0.5).into()),
                                    border: iced::Border { radius: 28.0.into(), ..Default::default() },
                                    ..Default::default()
                                }),
                            )
                            .width(img_w).height(img_h)
                            .center(Fill),
                        ].width(img_w).height(img_h).into()
                    } else {
                        text(format!("[video: {}]", msg.body))
                            .font(self.msg_font()).size(fs).color(theme::colors().purple)
                            .into()
                    }
                } else {
                    // Check for generated thumbnail via video_thumb key
                    let thumb_key = msg.video_mxc.as_ref().map(|v| format!("video_thumb:{v}"));
                    let gen_handle = thumb_key.as_ref().and_then(|k| self.image_cache.get(k));
                    if let Some(handle) = gen_handle {
                        let (tw, th) = thumb_key.as_ref()
                            .and_then(|k| self.video_thumb_sizes.get(k))
                            .copied()
                            .unwrap_or((320, 240));
                        let img_w = tw as f32;
                        let img_h = th as f32;
                        let play_svg = br#"<svg xmlns="http://www.w3.org/2000/svg" width="24" height="24" viewBox="0 0 24 24" fill="currentColor" stroke="none"><polygon points="5,3 19,12 5,21"/></svg>"#.as_slice();
                        button(
                            iced::widget::stack![
                                iced::widget::image(handle.clone())
                                    .width(img_w).height(img_h)
                                    .content_fit(iced::ContentFit::Cover),
                                container(
                                    container(
                                        iced::widget::svg(iced::widget::svg::Handle::from_memory(play_svg))
                                            .width(32).height(32)
                                            .style(|_, _| iced::widget::svg::Style { color: Some(Color::WHITE) }),
                                    )
                                    .padding(12)
                                    .style(|_: &Theme| container::Style {
                                        background: Some(Color::from_rgba(0.0, 0.0, 0.0, 0.5).into()),
                                        border: iced::Border { radius: 28.0.into(), ..Default::default() },
                                        ..Default::default()
                                    }),
                                )
                                .width(img_w).height(img_h)
                                .center(Fill),
                            ].width(img_w).height(img_h)
                        )
                        .on_press(Message::PlayVideo(eid.clone(), rid.clone()))
                        .padding(0)
                        .style(|_, _| button::Style { background: None, ..Default::default() })
                        .into()
                    } else {
                        // No thumbnail yet (loading or unavailable)
                        container(
                            row![
                                iced::widget::svg(iced::widget::svg::Handle::from_memory(
                                    br#"<svg xmlns="http://www.w3.org/2000/svg" width="24" height="24" viewBox="0 0 24 24" fill="currentColor" stroke="none"><polygon points="5,3 19,12 5,21"/></svg>"#.as_slice(),
                                ))
                                .width(20).height(20)
                                .style(|_, _| iced::widget::svg::Style { color: Some(theme::colors().accent) }),
                                text(&msg.body).font(self.msg_font()).size(fs).color(theme::colors().text_bright),
                            ].spacing(8).align_y(iced::Alignment::Center),
                        )
                        .padding([8, 12])
                        .style(|_: &Theme| container::Style {
                            background: Some(theme::colors().bg_mid.into()),
                            border: iced::Border { color: theme::colors().border, width: 1.0, radius: 8.0.into() },
                            ..Default::default()
                        })
                        .into()
                    }
                };
                button(thumbnail)
                    .on_press(Message::PlayVideo(eid, rid))
                    .padding(0)
                    .style(|_, _| button::Style { background: None, ..Default::default() })
                    .into()
            }
        } else if msg.file_mxc.is_some() {
            // File attachment card
            let file_icon = iced::widget::svg::Handle::from_memory(crate::icons::SVG_FILE);
            let size_str = msg.file_size.map(|s| {
                if s >= 1_048_576 { format!("{:.1} MB", s as f64 / 1_048_576.0) }
                else if s >= 1024 { format!("{:.0} KB", s as f64 / 1024.0) }
                else { format!("{s} B") }
            }).unwrap_or_default();
            let mime_str = msg.file_mimetype.as_deref().unwrap_or("file");
            let eid = msg.event_id.clone();
            let rid = view_key.to_string();
            button(
                container(
                    row![
                        iced::widget::svg(file_icon)
                            .width(24).height(24)
                            .style(|_, _| iced::widget::svg::Style { color: Some(theme::colors().accent) }),
                        column![
                            text(&msg.body).font(self.msg_font()).size(fs).color(theme::colors().text_bright),
                            text(format!("{mime_str}  {size_str}")).font(self.msg_font()).size(11).color(theme::colors().text_dim),
                        ].spacing(2),
                    ]
                    .spacing(10)
                    .align_y(iced::Alignment::Center),
                )
                .padding([10, 14])
                .style(|_: &Theme| container::Style {
                    background: Some(theme::colors().bg_mid.into()),
                    border: iced::Border { color: theme::colors().border, width: 1.0, radius: 8.0.into() },
                    ..Default::default()
                }),
            )
            .on_press(Message::DownloadFile(eid, rid))
            .padding(0)
            .style(|_, _| button::Style { background: None, ..Default::default() })
            .into()
        } else if self.prefs.big_emoji && crate::prefs::is_emoji_only(&msg.body) {
            // Emoji-only message (formatted or not): shown large.
            text(msg.body.trim()).size(fs * BIG_EMOJI_SCALE).into()
        } else if let Some(ref formatted) = msg.formatted_body {
            let has_code_block = formatted.contains("<pre>");
            let has_table = formatted.contains("<table");
            // Per code block: long enough to collapse, and expanded? The set
            // holds blocks toggled away from the default.
            let long_blocks = if has_code_block { long_code_blocks(formatted, CODE_BLOCK_LINES) } else { Vec::new() };
            let expanded: Vec<bool> = (0..long_blocks.len())
                .map(|i| self.expanded_code_blocks.contains(&(msg.event_id.clone(), i)) != self.prefs.expand_code_blocks)
                .collect();

            let rendered = self.body_cache.get_or_insert(
                ("html", formatted.as_str(), &expanded, fs.to_bits()),
                || {
                    let mut display_html = if has_code_block {
                        collapse_code_blocks(formatted, CODE_BLOCK_LINES, &expanded)
                    } else {
                        formatted.clone()
                    };

                    // Convert markdown-style bullets and headings to HTML
                    // (some bots send * bullets and ### headings in formatted_body)
                    if !display_html.contains("<ul>") && !display_html.contains("<ol>") {
                        display_html = convert_markdown_bullets(&display_html);
                    }
                    display_html = convert_markdown_headings(&display_html);

                    if has_code_block || has_table {
                        crate::html::RenderedBody::Parts(crate::html::html_to_parts(&display_html, &self.rooms, fs))
                    } else {
                        crate::html::RenderedBody::Spans(crate::html::html_to_spans_with_rooms(&display_html, &self.rooms, fs))
                    }
                },
            );

            if let crate::html::RenderedBody::Parts(parts) = &*rendered {
                // Use structured parts to render code blocks in containers
                let parts = parts.clone();
                let mut body_col = column![].spacing(4).width(Fill);
                let mut part_idx = 0usize;
                let mut block_idx = 0usize;

                for part in parts {
                    match part {
                        crate::html::MessagePart::Text(spans) => {
                            if spans.iter().all(|s| s.text.as_ref().trim().is_empty()) {
                                continue;
                            }
                            let wid = crate::selectable_text::SelectionWidgetId {
                                event_id: msg.event_id.clone(), part_index: part_idx,
                            };
                            let sel_range = self.selection_range_for(&wid);
                            let sel_active = self.selection.active;
                            body_col = body_col.push(
                                selectable_rich_text(spans)
                                    .font(self.msg_font())
                                    .size(fs)
                                    .wrapping(iced::widget::text::Wrapping::WordOrGlyph)
                                    .width(Fill)
                                    .on_link_click(Message::OpenUrl)
                                    .widget_id(wid)
                                    .selection_range(sel_range)
                                    .selection_active(sel_active)
                                    .on_selection(|evt| match evt {
                                        crate::selectable_text::SelectionEvent::Started(id, l, i) => Message::SelectionStarted(id, l, i),
                                        crate::selectable_text::SelectionEvent::Moved(id, l, i) => Message::SelectionMoved(id, l, i),
                                        crate::selectable_text::SelectionEvent::Ended => Message::SelectionEnded,
                                    }),
                            );
                            part_idx += 1;
                        }
                        crate::html::MessagePart::CodeBlock(spans) => {
                            let block = block_idx;
                            block_idx += 1;
                            let wid = crate::selectable_text::SelectionWidgetId {
                                event_id: msg.event_id.clone(), part_index: part_idx,
                            };
                            let sel_range = self.selection_range_for(&wid);
                            let sel_active = self.selection.active;
                            let code_col = column![
                                selectable_rich_text(spans)
                                    .font(self.msg_font())
                                    .size(fs)
                                    .wrapping(iced::widget::text::Wrapping::WordOrGlyph)
                                    .width(Fill)
                                    .on_link_click(Message::OpenUrl)
                                    .widget_id(wid)
                                    .selection_range(sel_range)
                                    .selection_active(sel_active)
                                    .on_selection(|evt| match evt {
                                        crate::selectable_text::SelectionEvent::Started(id, l, i) => Message::SelectionStarted(id, l, i),
                                        crate::selectable_text::SelectionEvent::Moved(id, l, i) => Message::SelectionMoved(id, l, i),
                                        crate::selectable_text::SelectionEvent::Ended => Message::SelectionEnded,
                                    }),
                            ].spacing(4).width(Fill);
                            // Only blocks with lines to hide get the toggle.
                            let code_col = if long_blocks.get(block).copied().unwrap_or(false) {
                                let label = if expanded[block] { "Show less" } else { "Show more" };
                                code_col.push(
                                    button(text(label).font(self.msg_font()).size(11).color(theme::colors().accent))
                                        .on_press(Message::ToggleCodeBlock(msg.event_id.clone(), block))
                                        .padding([2, 8])
                                        .style(|_, status| button::Style {
                                            background: if matches!(status, button::Status::Hovered) {
                                                Some(theme::tint(0.06).into())
                                            } else { None },
                                            ..Default::default()
                                        }),
                                )
                            } else {
                                code_col
                            };

                            let code_container_bg = theme::colors().code_bg;
                            body_col = body_col.push(
                                container(code_col)
                                    .padding([8, 12])
                                    .width(Fill)
                                    .style(move |_| container::Style {
                                        background: Some(code_container_bg.into()),
                                        border: iced::Border { radius: 6.0.into(), ..Default::default() },
                                        ..Default::default()
                                    }),
                            );
                            part_idx += 1;
                        }
                        crate::html::MessagePart::Table { headers, rows } => {
                            let border_color = theme::with_alpha(theme::colors().text_normal, 0.15);
                            let header_bg = theme::colors().code_bg;
                            let col_count = headers.len().max(rows.iter().map(|r| r.len()).max().unwrap_or(0));
                            if col_count == 0 {
                                continue;
                            }

                            let mut table_col = column![].width(Fill);

                            // Header row
                            if !headers.is_empty() {
                                let mut header_row = row![].width(Fill);
                                for col in 0..col_count {
                                    let spans = headers.get(col).cloned().unwrap_or_default();
                                    let wid = crate::selectable_text::SelectionWidgetId {
                                        event_id: msg.event_id.clone(), part_index: part_idx,
                                    };
                                    let sel_range = self.selection_range_for(&wid);
                                    let sel_active = self.selection.active;
                                    let cell = selectable_rich_text(spans)
                                        .font(self.msg_font())
                                        .size(fs)
                                        .wrapping(iced::widget::text::Wrapping::WordOrGlyph)
                                        .width(Fill)
                                        .on_link_click(Message::OpenUrl)
                                        .widget_id(wid)
                                        .selection_range(sel_range)
                                        .selection_active(sel_active)
                                        .on_selection(|evt| match evt {
                                            crate::selectable_text::SelectionEvent::Started(id, l, i) => Message::SelectionStarted(id, l, i),
                                            crate::selectable_text::SelectionEvent::Moved(id, l, i) => Message::SelectionMoved(id, l, i),
                                            crate::selectable_text::SelectionEvent::Ended => Message::SelectionEnded,
                                        });
                                    let bg = header_bg;
                                    header_row = header_row.push(
                                        container(cell)
                                            .padding([6, 10])
                                            .width(Length::FillPortion(1))
                                            .style(move |_| container::Style {
                                                background: Some(bg.into()),
                                                border: iced::Border {
                                                    color: border_color,
                                                    width: 1.0,
                                                    radius: 0.0.into(),
                                                },
                                                ..Default::default()
                                            }),
                                    );
                                    part_idx += 1;
                                }
                                table_col = table_col.push(header_row);
                            }

                            // Body rows
                            for row_cells in rows {
                                let mut body_row = row![].width(Fill);
                                for col in 0..col_count {
                                    let spans = row_cells.get(col).cloned().unwrap_or_default();
                                    let wid = crate::selectable_text::SelectionWidgetId {
                                        event_id: msg.event_id.clone(), part_index: part_idx,
                                    };
                                    let sel_range = self.selection_range_for(&wid);
                                    let sel_active = self.selection.active;
                                    let cell = selectable_rich_text(spans)
                                        .font(self.msg_font())
                                        .size(fs)
                                        .wrapping(iced::widget::text::Wrapping::WordOrGlyph)
                                        .width(Fill)
                                        .on_link_click(Message::OpenUrl)
                                        .widget_id(wid)
                                        .selection_range(sel_range)
                                        .selection_active(sel_active)
                                        .on_selection(|evt| match evt {
                                            crate::selectable_text::SelectionEvent::Started(id, l, i) => Message::SelectionStarted(id, l, i),
                                            crate::selectable_text::SelectionEvent::Moved(id, l, i) => Message::SelectionMoved(id, l, i),
                                            crate::selectable_text::SelectionEvent::Ended => Message::SelectionEnded,
                                        });
                                    body_row = body_row.push(
                                        container(cell)
                                            .padding([6, 10])
                                            .width(Length::FillPortion(1))
                                            .style(move |_| container::Style {
                                                border: iced::Border {
                                                    color: border_color,
                                                    width: 1.0,
                                                    radius: 0.0.into(),
                                                },
                                                ..Default::default()
                                            }),
                                    );
                                    part_idx += 1;
                                }
                                table_col = table_col.push(body_row);
                            }

                            body_col = body_col.push(
                                container(table_col)
                                    .width(Fill)
                                    .padding([4, 0]),
                            );
                        }
                    }
                }

                body_col.into()
            } else {
                let wid = crate::selectable_text::SelectionWidgetId {
                    event_id: msg.event_id.clone(), part_index: 0,
                };
                let sel_range = self.selection_range_for(&wid);
                let sel_active = self.selection.active;
                let spans = match &*rendered {
                    crate::html::RenderedBody::Spans(spans) => spans.clone(),
                    crate::html::RenderedBody::Parts(_) => Vec::new(),
                };
                selectable_rich_text(spans)
                    .font(self.msg_font())
                    .size(fs)
                    .wrapping(iced::widget::text::Wrapping::WordOrGlyph)
                    .width(Fill)
                    .on_link_click(Message::OpenUrl)
                    .widget_id(wid)
                    .selection_range(sel_range)
                    .selection_active(sel_active)
                    .on_selection(|evt| match evt {
                        crate::selectable_text::SelectionEvent::Started(id, l, i) => Message::SelectionStarted(id, l, i),
                        crate::selectable_text::SelectionEvent::Moved(id, l, i) => Message::SelectionMoved(id, l, i),
                        crate::selectable_text::SelectionEvent::Ended => Message::SelectionEnded,
                    })
                    .into()
            }
        } else {
            let wid = crate::selectable_text::SelectionWidgetId {
                event_id: msg.event_id.clone(), part_index: 0,
            };
            let sel_range = self.selection_range_for(&wid);
            let sel_active = self.selection.active;
            // Parse basic markdown in plain text bodies
            let rendered = self.body_cache.get_or_insert(("plain", msg.body.as_str()), || {
                crate::html::RenderedBody::Spans(crate::html::plain_text_to_spans(&msg.body))
            });
            let spans = match &*rendered {
                crate::html::RenderedBody::Spans(spans) => spans.clone(),
                crate::html::RenderedBody::Parts(_) => Vec::new(),
            };
            selectable_rich_text(spans)
                .font(self.msg_font())
                .size(fs)
                .wrapping(iced::widget::text::Wrapping::WordOrGlyph)
                .width(Fill)
                .on_link_click(Message::OpenUrl)
                .widget_id(wid)
                .selection_range(sel_range)
                .selection_active(sel_active)
                .on_selection(|evt| match evt {
                    crate::selectable_text::SelectionEvent::Started(id, l, i) => Message::SelectionStarted(id, l, i),
                    crate::selectable_text::SelectionEvent::Moved(id, l, i) => Message::SelectionMoved(id, l, i),
                    crate::selectable_text::SelectionEvent::Ended => Message::SelectionEnded,
                })
                .into()
        };

        // Caption under media (image / file / video / audio sent with text)
        if let Some(ref caption) = msg.caption {
            let rendered = match msg.caption_html {
                Some(ref html) => self.body_cache.get_or_insert(("caption-html", html.as_str(), fs.to_bits()), || {
                    crate::html::RenderedBody::Spans(crate::html::html_to_spans_with_rooms(html, &self.rooms, fs))
                }),
                None => self.body_cache.get_or_insert(("plain", caption.as_str()), || {
                    crate::html::RenderedBody::Spans(crate::html::plain_text_to_spans(caption))
                }),
            };
            let spans = match &*rendered {
                crate::html::RenderedBody::Spans(spans) => spans.clone(),
                crate::html::RenderedBody::Parts(_) => Vec::new(),
            };
            // Part index past any body parts so selection ids don't collide.
            let wid = crate::selectable_text::SelectionWidgetId {
                event_id: msg.event_id.clone(), part_index: CAPTION_PART_INDEX,
            };
            let caption_el = selectable_rich_text(spans)
                .font(self.msg_font())
                .size(fs)
                .wrapping(iced::widget::text::Wrapping::WordOrGlyph)
                .width(Fill)
                .on_link_click(Message::OpenUrl)
                .selection_range(self.selection_range_for(&wid))
                .selection_active(self.selection.active)
                .widget_id(wid)
                .on_selection(|evt| match evt {
                    crate::selectable_text::SelectionEvent::Started(id, l, i) => Message::SelectionStarted(id, l, i),
                    crate::selectable_text::SelectionEvent::Moved(id, l, i) => Message::SelectionMoved(id, l, i),
                    crate::selectable_text::SelectionEvent::Ended => Message::SelectionEnded,
                });
            body_element = column![body_element, caption_el].spacing(6).width(Fill).into();
        }

        // Edited marker and local-echo delivery state
        let mut status_bits: Vec<Element<'a, Message>> = Vec::new();
        if msg.is_edited {
            status_bits.push(text("(edited)").font(self.msg_font()).size((fs - 3.0).max(9.0)).color(theme::colors().text_dim).into());
        }
        match msg.send_state {
            Some(fluxchat_core::SendState::Sending) => {
                status_bits.push(text("sending…").font(self.msg_font()).size((fs - 3.0).max(9.0)).color(theme::colors().text_dim).into());
            }
            Some(fluxchat_core::SendState::Failed { recoverable }) => {
                status_bits.push(text("failed to send").font(self.msg_font()).size((fs - 3.0).max(9.0)).color(theme::colors().red).into());
                if recoverable {
                    if let Some(room) = self.active_real_room() {
                        status_bits.push(
                            button(text("retry").font(self.msg_font()).size((fs - 3.0).max(9.0)).color(theme::colors().accent))
                                .on_press(Message::RetrySend(room.to_string()))
                                .padding([0, 4])
                                .style(|_, _| button::Style { background: None, ..Default::default() })
                                .into(),
                        );
                    }
                }
            }
            None => {}
        }
        if !status_bits.is_empty() {
            let mut status_row = row![].spacing(6).align_y(iced::Alignment::Center);
            for bit in status_bits {
                status_row = status_row.push(bit);
            }
            body_element = column![body_element, status_row].spacing(2).width(Fill).into();
        }

        // Decryption diagnostics under undecryptable messages
        if let Some(hint) = msg.utd.as_ref().and_then(|u| self.utd_hints.get(&u.session_id)) {
            body_element = column![
                body_element,
                text(hint.clone()).font(self.msg_font()).size((fs - 2.0).max(10.0)).color(theme::colors().text_dim)
                    .wrapping(iced::widget::text::Wrapping::WordOrGlyph),
            ].spacing(2).width(Fill).into();
        }

        // Reply context — colored left bar with sender name + quoted text
        // If SDK didn't resolve the reply, try local message cache
        let (resolved_reply_sender, resolved_reply_body) = if msg.reply_to_body.is_some() {
            (msg.reply_to_sender.clone(), msg.reply_to_body.clone())
        } else if let Some(ref reply_id) = msg.reply_to_id {
            // Look up in local messages
            let found = self.messages_by_room.get(view_key)
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
                let mut end = 100;
                while !reply_body.is_char_boundary(end) { end -= 1; }
                format!("{}...", &reply_body[..end])
            } else {
                reply_body
            };

            let jump_to_id = msg.reply_to_id.clone().unwrap_or_default();
            let reply_preview: Element<'a, Message> = button(
                container(
                    row![
                        text("\u{2503}").size(20).color(sender_color), // vertical bar character
                        column![
                            text(reply_sender).font(self.msg_font()).size(12).color(sender_color),
                            text(preview_text).font(self.msg_font()).size(12).color(theme::colors().text_dim)
                                .wrapping(iced::widget::text::Wrapping::WordOrGlyph),
                        ].spacing(1),
                    ].spacing(4).align_y(iced::Alignment::Center)
                )
                .padding([2, 6])
                .style(|_: &Theme| container::Style {
                    background: Some(theme::tint(0.03).into()),
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
            .spacing(MSG_COL_GAP).width(Fill).align_y(iced::Alignment::Start).into();

        // Wrap in container with hover highlight; mentions/keywords get a
        // red wash (a little stronger on hover).
        let highlighted = self.is_highlight(msg);
        let hover_bg = match (highlighted, is_hovered) {
            (true, true) => theme::with_alpha(theme::colors().red, 0.13),
            (true, false) => theme::with_alpha(theme::colors().red, 0.08),
            (false, true) => theme::tint(0.03),
            (false, false) => Color::TRANSPARENT,
        };
        let base_row: Element<'a, Message> = container(base_row)
            .width(Fill)
            .padding(iced::Padding {
                top: if continuation || self.compact_mode { 1.0 } else { 6.0 },
                right: 0.0,
                bottom: if self.compact_mode { 0.0 } else { 1.0 },
                left: 0.0,
            })
            .style(move |_: &Theme| container::Style {
                background: Some(hover_bg.into()),
                ..Default::default()
            })
            .into();

        // Always build the action bar to keep widget tree structure stable
        // (changing tree structure on hover loses SelectableRichText selection state)
        let btn_size = (fs + 4.0).max(18.0);
        let icon_sz = btn_size - 6.0;
        let btn_pad = ((btn_size - icon_sz) / 2.0).floor() as u16;
        let tooltip_font = self.sidebar_font();
        let make_msg_action = move |svg_bytes: &'static [u8], label: &'static str, msg_action: Message| -> Element<'a, Message> {
            let handle = iced::widget::svg::Handle::from_memory(svg_bytes);
            let btn = button(
                iced::widget::svg(handle)
                    .width(Length::Fixed(icon_sz)).height(Length::Fixed(icon_sz))
                    .content_fit(iced::ContentFit::Fill)
                    .style(|_, status| iced::widget::svg::Style {
                        color: Some(if matches!(status, iced::widget::svg::Status::Hovered) {
                            theme::colors().text_bright
                        } else {
                            theme::colors().text_muted
                        }),
                    }),
            )
            .on_press(msg_action)
            .padding(btn_pad)
            .width(Length::Fixed(btn_size))
            .height(Length::Fixed(btn_size))
            .style(|_, status| button::Style {
                background: if matches!(status, button::Status::Hovered) {
                    Some(theme::tint(0.12).into())
                } else {
                    None
                },
                border: iced::Border { radius: crate::ui::RADIUS.into(), ..Default::default() },
                ..Default::default()
            });
            iced::widget::tooltip(
                btn,
                text(label).font(tooltip_font).size(11).color(theme::colors().text_normal),
                iced::widget::tooltip::Position::Top,
            )
                .delay(crate::ui::TOOLTIP_DELAY)
            .gap(4)
            .style(|_: &Theme| container::Style {
                background: Some(theme::colors().bg_deep.into()),
                border: iced::Border { color: theme::colors().border, width: 1.0, radius: 4.0.into() },
                ..Default::default()
            })
            .padding(4)
            .into()
        };

        let is_own = msg.sender == self.user_id;
        let mut action_row = row![].spacing(2).align_y(iced::Alignment::Center);

        action_row = action_row.push(make_msg_action(SVG_REACT, "React", Message::OpenEmojiReact(event_id.clone())));
        action_row = action_row.push(make_msg_action(SVG_CLIPBOARD, "Copy text", Message::CopyMessageText(msg.body.clone())));
        action_row = action_row.push(make_msg_action(SVG_REPLY, "Reply", Message::StartReply(
            event_id.clone(),
            msg.sender_name.clone(),
            msg.body.chars().take(80).collect(),
        )));
        if let Some(room_id) = &self.selected_room_id {
            if !room_id.starts_with("thread::") {
                let thread_root = msg.thread_root.as_deref().unwrap_or(&msg.event_id);
                action_row = action_row.push(make_msg_action(SVG_THREAD, "Reply in thread", Message::OpenThread(
                    room_id.clone(),
                    thread_root.to_string(),
                )));
            }
        }
        action_row = action_row.push(make_msg_action(SVG_FORWARD, "Forward", Message::ForwardMessage(
            msg.body.clone(),
            msg.formatted_body.clone().unwrap_or_default(),
            Some(msg.sender_name.clone()),
        )));
        action_row = action_row.push(make_msg_action(SVG_PIN, "Pin", Message::PinMessage(event_id.clone())));

        if is_own {
            action_row = action_row.push(make_msg_action(SVG_EDIT, "Edit", Message::StartEdit(
                event_id.clone(),
                msg.body.clone(),
            )));
            action_row = action_row.push(make_msg_action(SVG_DELETE, "Delete", Message::DeleteMessage(event_id.clone())));
        } else {
            let report_room = fluxchat_core::timeline::real_room_id(view_key).to_string();
            action_row = action_row.push(make_msg_action(SVG_FLAG, "Report", Message::OpenModal(
                crate::state::Modal::Report { event_id: event_id.clone(), room_id: report_room },
            )));
        }

        // Only show action bar when hovered (opacity 0 when not, preserving tree structure)
        let action_bar: Element<'a, Message> = if is_hovered {
            container(
                row![
                    Space::new().width(Fill),
                    container(action_row)
                        .padding([2, 4])
                        .style(|_: &Theme| container::Style {
                            background: Some(theme::colors().bg_dark.into()),
                            border: iced::Border {
                                color: theme::colors().border,
                                width: 1.0,
                                radius: 8.0.into(),
                            },
                            shadow: iced::Shadow {
                                color: Color::from_rgba(0.0, 0.0, 0.0, 0.3),
                                offset: iced::Vector::new(0.0, 3.0),
                                blur_radius: 10.0,
                            },
                            ..Default::default()
                        }),
                ]
                .align_y(iced::Alignment::Start)
            )
            .width(Fill)
            .height(Length::Shrink)
            .padding([0, 8])
            .into()
        } else {
            // Invisible placeholder — same tree shape, zero height
            container(Space::new().height(0)).width(Fill).into()
        };

        let ctx_menu = crate::state::ContextMenu::Message { event_id: msg.event_id.clone() };
        let msg_row: Element<'a, Message> = mouse_area(
            stack![base_row, action_bar].width(Fill)
        )
        .on_enter(Message::HoverMessage(Some(eid_hover)))
        .on_exit(Message::HoverMessage(None))
        .on_right_press(Message::OpenContextMenu(ctx_menu))
        .into();

        // URL preview cards (max 2 unless expanded)
        let indent = self.msg_content_indent();
        let urls = if self.prefs.url_previews { extract_urls(&msg.body) } else { Vec::new() };
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
                            .font(self.msg_font()).size(11).color(theme::colors().accent)
                    )
                    .on_press(Message::TogglePreviewExpand(eid))
                    .padding([2, 0])
                    .style(|_, _| button::Style { background: None, text_color: theme::colors().accent, ..Default::default() }),
                ].into()
            );
        } else if is_expanded {
            let eid = msg.event_id.clone();
            preview_elements.push(
                row![
                    Space::new().width(Length::Fixed(indent)),
                    button(
                        text("Show less").font(self.msg_font()).size(11).color(theme::colors().text_dim)
                    )
                    .on_press(Message::TogglePreviewExpand(eid))
                    .padding([2, 0])
                    .style(|_, _| button::Style { background: None, text_color: theme::colors().text_dim, ..Default::default() }),
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
            Space::new().width(Length::Fixed(self.msg_content_indent())),
        ]
        .spacing(4);

        for reaction in &msg.reactions {
            let event_id = msg.event_id.clone();
            let emoji = reaction.emoji.clone();
            let is_mine = reaction.users.iter().any(|u| u == &self.user_id);
            let bg = if is_mine {
                theme::with_alpha(theme::colors().accent, 0.15) // ACCENT tint
            } else {
                theme::tint(0.06)
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
                    text(reaction.count.to_string()).font(self.msg_font()).size(12).color(theme::colors().text_muted),
                ].spacing(4).align_y(iced::Alignment::Center)
            )
                .on_press(Message::ToggleReaction(event_id, emoji))
                .padding([2, 8])
                .style(move |_, status| button::Style {
                    background: Some(if matches!(status, button::Status::Hovered) {
                        theme::tint(0.12)
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
                    text(tooltip_text).font(self.msg_font()).size(11),
                    iced::widget::tooltip::Position::Top,
                )
                .delay(crate::ui::TOOLTIP_DELAY)
                .style(|_: &Theme| container::Style {
                    background: Some(theme::colors().bg_dark.into()),
                    border: iced::Border {
                        color: theme::colors().border,
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
                    background: Some(theme::with_alpha(theme::colors().accent, 0.15).into()),
                    ..Default::default()
                })
                .into()
        } else {
            col.into()
        }
    }

    /// Render the emoji picker panel.
    pub(crate) fn view_emoji_picker(&self) -> Element<Message> {
        use crate::ui::{self, Tone};
        let query_lower = self.emoji_query.to_lowercase();
        let searching = !query_lower.is_empty();
        let reacting = matches!(self.emoji_mode, crate::state::EmojiPickerMode::React(_));

        // Header: search + close
        let header = row![
            ui::input(
                if reacting { "Search reactions\u{2026}" } else { "Search emoji\u{2026}" },
                &self.emoji_query,
                Message::EmojiQueryChanged,
            )
            .id(emoji_search_id())
            .width(Fill),
            button(ui::icon(SVG_CLOSE, 14.0, theme::colors().text_dim))
                .on_press(Message::ToggleEmojiPicker)
                .padding(7)
                .style(ui::button_style(Tone::Ghost)),
        ]
        .spacing(6)
        .align_y(iced::Alignment::Center);

        // Category tabs: an icon per category, equal widths so all ten fit.
        let mut tabs = row![].spacing(2);
        for (i, (icon_svg, label)) in EMOJI_CATEGORIES.iter().enumerate() {
            let active = !searching && self.emoji_category == i as u8;
            let color = if active { theme::colors().accent } else { theme::colors().text_dim };
            let tab = button(
                column![
                    container(ui::icon(icon_svg, 16.0, color)).center_x(Fill),
                    container(Space::new().height(2)).width(Fill).style(move |_: &Theme| container::Style {
                        background: active.then(|| theme::colors().accent.into()),
                        border: iced::Border { radius: 1.0.into(), ..Default::default() },
                        ..Default::default()
                    }),
                ]
                .spacing(5),
            )
            .on_press(Message::EmojiCategoryChanged(i as u8))
            .padding(iced::Padding { top: 6.0, right: 0.0, bottom: 0.0, left: 0.0 })
            .width(Fill)
            .style(move |_, status| button::Style {
                background: (matches!(status, button::Status::Hovered) && !active)
                    .then(|| theme::with_alpha(theme::colors().text_normal, 0.06).into()),
                border: iced::Border { radius: 4.0.into(), ..Default::default() },
                ..Default::default()
            });
            tabs = tabs.push(container(ui::with_tooltip(tab, label)).width(Fill));
        }

        // Collect emojis to display
        let emojis_to_show: Vec<&str> = if searching {
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

        let heading = if searching {
            if emojis_to_show.len() == 1 { "1 result".to_string() } else { format!("{} results", emojis_to_show.len()) }
        } else {
            EMOJI_CATEGORIES.get(self.emoji_category as usize).map(|(_, l)| l.to_string()).unwrap_or_default()
        };

        // Emoji grid
        let mut grid = column![].spacing(2);
        if emojis_to_show.is_empty() {
            let (title, detail) = if !searching && self.emoji_category == 0 {
                ("Nothing here yet", Some("Emoji you use will show up here.".to_string()))
            } else {
                ("No emoji found", None)
            };
            grid = grid.push(ui::empty_state(SVG_SMILE, title, detail));
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
                            background: matches!(status, button::Status::Hovered)
                                .then(|| theme::with_alpha(theme::colors().text_normal, 0.1).into()),
                            border: iced::Border { radius: 6.0.into(), ..Default::default() },
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
            ui::divider(),
            ui::section_label(heading),
            scrollable(grid)
                .height(Length::Fixed(EMOJI_PANEL_HEIGHT))
                .direction(iced::widget::scrollable::Direction::Vertical(
                    iced::widget::scrollable::Scrollbar::new().width(4).scroller_width(4),
                ))
                .style(thin_scrollbar),
        ].spacing(8);

        let picker = container(panel)
            .padding(12)
            .width(Length::Fixed(EMOJI_PANEL_WIDTH))
            .style(ui::dialog_style);
        // Right-align the picker
        container(picker).width(Fill).align_x(iced::alignment::Horizontal::Right).padding(8)
            .into()
    }

    pub(crate) fn view_url_preview<'a>(&self, preview: &'a fluxchat_core::UrlPreview, indent: f32) -> Element<'a, Message> {
        let mut content_col = column![].spacing(2);

        if let Some(ref site_name) = preview.site_name {
            content_col = content_col.push(
                text(site_name.as_str().to_uppercase())
                    .font(self.msg_font()).size(11).color(theme::colors().text_dim)
            );
        }

        if let Some(ref title) = preview.title {
            content_col = content_col.push(
                text(title.as_str())
                    .font(self.msg_font()).size(14).color(theme::colors().accent)
                    .wrapping(iced::widget::text::Wrapping::None)
            );
        }

        if let Some(ref desc) = preview.description {
            let short = if desc.len() > 150 {
                let mut end = 150;
                while !desc.is_char_boundary(end) { end -= 1; }
                &desc[..end]
            } else { desc };
            content_col = content_col.push(
                text(short)
                    .font(self.msg_font()).size(13).color(theme::colors().text_muted)
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
                    background: Some(theme::colors().bg_dark.into()),
                    border: iced::Border {
                        color: theme::colors().border,
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

/// Convert markdown-style bullet lines (`* text` or `- text`) into `<ul><li>` HTML.
fn convert_markdown_bullets(html: &str) -> String {
    // Split on <br> variants and newlines, preserving separators as empty entries
    let lines: Vec<&str> = html.split("<br>")
        .flat_map(|s| s.split("<br/>"))
        .flat_map(|s| s.split("<br />"))
        .flat_map(|s| s.split('\n'))
        .collect();
    // Quick check: any bullet lines at all?
    if !lines.iter().any(|l| {
        let t = l.trim();
        t.starts_with("* ") || t.starts_with("- ")
    }) {
        return html.to_string();
    }
    let mut result = String::new();
    let mut in_list = false;
    for (i, line) in lines.iter().enumerate() {
        let trimmed = line.trim();
        if let Some(rest) = trimmed.strip_prefix("* ").or_else(|| trimmed.strip_prefix("- ")) {
            if !in_list {
                result.push_str("<ul>");
                in_list = true;
            }
            result.push_str("<li>");
            result.push_str(rest);
            result.push_str("</li>");
        } else {
            if in_list {
                result.push_str("</ul>");
                in_list = false;
            }
            // Preserve line breaks between non-bullet lines
            if i > 0 {
                result.push_str("<br>");
            }
            result.push_str(line);
        }
    }
    if in_list {
        result.push_str("</ul>");
    }
    result
}

/// Convert markdown-style headings (`#` through `######`) to HTML heading tags.
fn convert_markdown_headings(html: &str) -> String {
    if !html.contains('#') {
        return html.to_string();
    }
    let mut result = String::with_capacity(html.len());
    for part in html.split("<br>").flat_map(|s| s.split("<br/>")).flat_map(|s| s.split('\n')) {
        let trimmed = part.trim();
        let (level, rest) = if let Some(r) = trimmed.strip_prefix("###### ") {
            (6, r)
        } else if let Some(r) = trimmed.strip_prefix("##### ") {
            (5, r)
        } else if let Some(r) = trimmed.strip_prefix("#### ") {
            (4, r)
        } else if let Some(r) = trimmed.strip_prefix("### ") {
            (3, r)
        } else if let Some(r) = trimmed.strip_prefix("## ") {
            (2, r)
        } else if let Some(r) = trimmed.strip_prefix("# ") {
            (1, r)
        } else {
            if !result.is_empty() {
                result.push_str("<br>");
            }
            result.push_str(part);
            continue;
        };
        if !result.is_empty() {
            result.push_str("<br>");
        }
        result.push_str(&format!("<h{level}>{rest}</h{level}>"));
    }
    result
}

/// Byte range of each `<pre>` block's code (inside `<pre>` / `<pre><code ..>`),
/// in document order — the order `html_to_parts` yields `CodeBlock`s.
fn code_block_ranges(html: &str) -> Vec<std::ops::Range<usize>> {
    let mut ranges = Vec::new();
    let mut pos = 0;
    while let Some(found) = html[pos..].find("<pre") {
        let tag = pos + found;
        // `<pre>` or `<pre attr..>`, not e.g. `<prefix>`.
        if !html[tag + 4..].starts_with(['>', ' ', '\t', '\n']) {
            pos = tag + 4;
            continue;
        }
        let Some(gt) = html[tag..].find('>') else { break };
        let content_start = tag + gt + 1;
        let (code_start, end_marker) = if html[content_start..].starts_with("<code") {
            match html[content_start..].find('>') {
                Some(i) => (content_start + i + 1, "</code></pre>"),
                None => break,
            }
        } else {
            (content_start, "</pre>")
        };
        let Some(len) = html[code_start..].find(end_marker) else { break };
        ranges.push(code_start..code_start + len);
        pos = code_start + len + end_marker.len();
    }
    ranges
}

/// Whether each code block is longer than `max_lines` (worth a Show more).
fn long_code_blocks(html: &str, max_lines: usize) -> Vec<bool> {
    code_block_ranges(html).into_iter()
        .map(|r| html[r].split('\n').nth(max_lines).is_some())
        .collect()
}

/// Truncate code blocks to `max_lines` lines, except those `expanded[i]`.
fn collapse_code_blocks(html: &str, max_lines: usize, expanded: &[bool]) -> String {
    let mut result = String::with_capacity(html.len());
    let mut copied = 0;
    for (i, range) in code_block_ranges(html).into_iter().enumerate() {
        if expanded.get(i).copied().unwrap_or(false) {
            continue;
        }
        let lines: Vec<&str> = html[range.clone()].split('\n').collect();
        if lines.len() > max_lines {
            result.push_str(&html[copied..range.start]);
            result.push_str(&lines[..max_lines].join("\n"));
            result.push_str(&format!("\n... ({} more lines)", lines.len() - max_lines));
            copied = range.end;
        }
    }
    result.push_str(&html[copied..]);
    result
}

/// Day label for a date divider row.
pub(crate) fn local_day_label(unix_secs: u64) -> String {
    crate::view::format_day(unix_secs)
}

/// Clock text for seconds-since-midnight: "14:05", "14:05:09", "2:05 PM".
pub(crate) fn format_clock(secs_of_day: u64, seconds: bool, twelve_hour: bool) -> String {
    let (h, m, s) = (secs_of_day / 3600, (secs_of_day % 3600) / 60, secs_of_day % 60);
    let tail = if seconds { format!(":{s:02}") } else { String::new() };
    if twelve_hour {
        let (h12, ampm) = match h { 0 => (12, "AM"), 1..=11 => (h, "AM"), 12 => (12, "PM"), _ => (h - 12, "PM") };
        format!("{h12}:{m:02}{tail} {ampm}")
    } else {
        format!("{h:02}:{m:02}{tail}")
    }
}


/// `needle` appears in `haystack` as a whole word (both lowercase).
fn contains_word(haystack: &str, needle: &str) -> bool {
    let is_word = |c: char| c.is_alphanumeric() || c == '_';
    haystack.match_indices(needle).any(|(i, m)| {
        let before = haystack[..i].chars().next_back();
        let after = haystack[i + m.len()..].chars().next();
        before.is_none_or(|c| !is_word(c)) && after.is_none_or(|c| !is_word(c))
    })
}

#[cfg(test)]
mod tests {
    use super::{collapse_code_blocks, contains_word, format_clock, long_code_blocks};

    #[test]
    fn code_blocks_collapse_one_at_a_time() {
        let long = "a\nb\nc\nd";
        let html = format!("<p>x</p><pre><code class=\"language-sh\">{long}</code></pre><pre>one</pre><pre class=\"y\">{long}</pre>");
        // Only blocks longer than the limit offer Show more.
        assert_eq!(long_code_blocks(&html, 2), vec![true, false, true]);
        // Expanding the first leaves the last one collapsed.
        let out = collapse_code_blocks(&html, 2, &[true, false, false]);
        assert!(out.contains(&format!(">{long}</code></pre>")));
        assert!(out.contains("<pre class=\"y\">a\nb\n... (2 more lines)</pre>"));
        assert!(out.contains("<pre>one</pre>"));
        // Nothing to collapse: unchanged.
        assert_eq!(collapse_code_blocks("<prefix>a\nb\nc</prefix>", 1, &[]), "<prefix>a\nb\nc</prefix>");
    }

    #[test]
    fn whole_word_matching() {
        assert!(contains_word("hey phrac, look", "phrac"));
        assert!(contains_word("@room deploy now", "@room"));
        assert!(!contains_word("phracking around", "phrac"));
        assert!(!contains_word("myroomie", "room"));
        assert!(contains_word("seerr!", "seerr"));
    }

    #[test]
    fn clock_formats() {
        let t = 14 * 3600 + 5 * 60 + 9;
        assert_eq!(format_clock(t, false, false), "14:05");
        assert_eq!(format_clock(t, true, false), "14:05:09");
        assert_eq!(format_clock(t, false, true), "2:05 PM");
        assert_eq!(format_clock(5 * 60, false, true), "12:05 AM");
        assert_eq!(format_clock(12 * 3600, false, true), "12:00 PM");
    }
}
