use std::collections::HashMap;
use std::sync::Arc;
use matrix_sdk::ruma::OwnedRoomId;
use matrix_sdk_ui::timeline::{
    Timeline, TimelineBuilder, TimelineItem, TimelineItemKind, TimelineItemContent,
    EventTimelineItem,
};

use crate::models::{MessageInfo, PollInfo, ReactionInfo};

/// Store of Timeline instances per room.
pub type RoomTimelines = tokio::sync::Mutex<HashMap<OwnedRoomId, Arc<Timeline>>>;

/// Get or create a Timeline for a room.
pub async fn get_or_create_timeline(
    client: &matrix_sdk::Client,
    timelines: &RoomTimelines,
    room_id: &matrix_sdk::ruma::RoomId,
) -> Result<Arc<Timeline>, crate::error::AppError> {
    let mut map = timelines.lock().await;
    if let Some(tl) = map.get(room_id) {
        return Ok(tl.clone());
    }
    let room = client.get_room(room_id)
        .ok_or_else(|| crate::error::AppError::RoomNotFound(room_id.to_string()))?;
    let timeline = TimelineBuilder::new(&room).build().await
        .map_err(|e| crate::error::AppError::Other(format!("Failed to build timeline: {e}")))?;
    let tl = Arc::new(timeline);
    map.insert(room_id.to_owned(), tl.clone());
    Ok(tl)
}

/// Remove a timeline when a room tab is closed.
pub async fn remove_timeline(
    timelines: &RoomTimelines,
    room_id: &matrix_sdk::ruma::RoomId,
) {
    timelines.lock().await.remove(room_id);
}

/// Convert a TimelineItem to our MessageInfo. Returns None for items we skip (date dividers, etc).
pub fn timeline_item_to_message_info(item: &TimelineItem) -> Option<MessageInfo> {
    match item.kind() {
        TimelineItemKind::Event(ev) => event_to_message_info(ev),
        TimelineItemKind::Virtual(_) => None, // skip date dividers etc for now
    }
}

/// Convert an EventTimelineItem to MessageInfo.
fn event_to_message_info(ev: &EventTimelineItem) -> Option<MessageInfo> {
    let event_id = if let Some(eid) = ev.event_id() {
        eid.to_string()
    } else if let Some(txn_id) = ev.transaction_id() {
        format!("~local-{}", txn_id)
    } else {
        return None;
    };

    let sender = ev.sender().to_string();
    let (sender_name, avatar_url) = match ev.sender_profile() {
        matrix_sdk_ui::timeline::TimelineDetails::Ready(profile) => {
            let name = profile.display_name.clone()
                .unwrap_or_else(|| sender.split(':').next().unwrap_or(&sender).trim_start_matches('@').to_string());
            let avatar = profile.avatar_url.as_ref().map(|u| u.to_string());
            (name, avatar)
        }
        _ => {
            let name = sender.split(':').next().unwrap_or(&sender).trim_start_matches('@').to_string();
            (name, None)
        }
    };

    let timestamp = ev.timestamp().as_secs().into();
    let content = ev.content();

    // Extract reactions (on TimelineItemContent)
    let reactions: Vec<ReactionInfo> = if let Some(rxns) = content.reactions() {
        rxns.iter().map(|(emoji, senders)| {
            let users: Vec<String> = senders.keys().map(|u| u.to_string()).collect();
            ReactionInfo {
                emoji: emoji.to_string(),
                count: users.len() as u32,
                users,
            }
        }).collect()
    } else {
        Vec::new()
    };

    // Extract reply info (on TimelineItemContent)
    let reply_to_id = content.in_reply_to().map(|r| r.event_id.to_string());

    match content {
        TimelineItemContent::MsgLike(msg_like) => {
            if content.is_redacted() {
                return None;
            }

            // Extract thread info
            let thread_root = msg_like.thread_root.as_ref().map(|id| id.to_string());
            let thread_count = msg_like.thread_summary.as_ref().map(|s| s.num_replies);


            if let Some(message) = content.as_message() {
                let body = message.body().to_string();
                let is_emote = matches!(message.msgtype(), matrix_sdk::ruma::events::room::message::MessageType::Emote(_));
                let is_video = matches!(message.msgtype(), matrix_sdk::ruma::events::room::message::MessageType::Video(_));
                let (formatted_body, image_mxc, image_width, image_height) = match message.msgtype() {
                    matrix_sdk::ruma::events::room::message::MessageType::Text(t) => {
                        (t.formatted.as_ref().map(|f| f.body.clone()), None, None, None)
                    }
                    matrix_sdk::ruma::events::room::message::MessageType::Image(img) => {
                        let mxc = match &img.source {
                            matrix_sdk::ruma::events::room::MediaSource::Plain(uri) => Some(uri.to_string()),
                            matrix_sdk::ruma::events::room::MediaSource::Encrypted(enc) => Some(enc.url.to_string()),
                        };
                        let w = img.info.as_ref().and_then(|i| i.width).and_then(|v| u32::try_from(v).ok());
                        let h = img.info.as_ref().and_then(|i| i.height).and_then(|v| u32::try_from(v).ok());
                        (None, mxc, w, h)
                    }
                    matrix_sdk::ruma::events::room::message::MessageType::Notice(t) => {
                        (t.formatted.as_ref().map(|f| f.body.clone()), None, None, None)
                    }
                    matrix_sdk::ruma::events::room::message::MessageType::Emote(t) => {
                        (t.formatted.as_ref().map(|f| f.body.clone()), None, None, None)
                    }
                    matrix_sdk::ruma::events::room::message::MessageType::Video(vid) => {
                        let mxc = match &vid.source {
                            matrix_sdk::ruma::events::room::MediaSource::Plain(uri) => Some(uri.to_string()),
                            matrix_sdk::ruma::events::room::MediaSource::Encrypted(enc) => Some(enc.url.to_string()),
                        };
                        let w = vid.info.as_ref().and_then(|i| i.width).and_then(|v| u32::try_from(v).ok());
                        let h = vid.info.as_ref().and_then(|i| i.height).and_then(|v| u32::try_from(v).ok());
                        (None, mxc, w, h)
                    }
                    matrix_sdk::ruma::events::room::message::MessageType::File(file) => {
                        // Show files with image MIME types as inline images (e.g. GIFs sent as files)
                        let is_image = file.info.as_ref()
                            .and_then(|i| i.mimetype.as_ref())
                            .is_some_and(|m| m.starts_with("image/"));
                        if is_image {
                            let mxc = match &file.source {
                                matrix_sdk::ruma::events::room::MediaSource::Plain(uri) => Some(uri.to_string()),
                                matrix_sdk::ruma::events::room::MediaSource::Encrypted(enc) => Some(enc.url.to_string()),
                            };
                            (None, mxc, None, None)
                        } else {
                            (None, None, None, None)
                        }
                    }
                    _ => (None, None, None, None),
                };

                Some(MessageInfo {
                    event_id, sender, sender_name, body, formatted_body,
                    transformed_body: None, image_mxc, reactions, reply_to_id,
                    reply_to_sender: None, reply_to_body: None, timestamp, avatar_url,
                    thread_root, thread_count, image_width, image_height, poll: None,
                    is_emote, is_video,
                })
            } else if let Some(sticker) = content.as_sticker() {
                let sc = sticker.content();
                let body = sc.body.clone();
                let mxc = match &sc.source {
                    matrix_sdk::ruma::events::sticker::StickerMediaSource::Plain(uri) => Some(uri.to_string()),
                    _ => None,
                };
                let w = sc.info.width.and_then(|v| u32::try_from(v).ok());
                let h = sc.info.height.and_then(|v| u32::try_from(v).ok());
                Some(MessageInfo {
                    event_id, sender, sender_name, body, formatted_body: None,
                    transformed_body: None, image_mxc: mxc, reactions, reply_to_id,
                    reply_to_sender: None, reply_to_body: None, timestamp, avatar_url,
                    thread_root, thread_count, image_width: w, image_height: h, poll: None,
                    is_emote: false, is_video: false,
                })
            } else if let Some(poll_state) = content.as_poll() {
                let result = poll_state.results();
                let question = result.question.clone();
                let answers: Vec<(String, String)> = result.answers.iter()
                    .map(|a| (a.id.clone(), a.text.clone()))
                    .collect();
                let votes: std::collections::HashMap<String, Vec<String>> = result.votes;
                let total_votes = votes.values().map(|v| v.len() as u32).sum();
                let is_ended = result.end_time.is_some();

                let body = format!("Poll: {}", question);

                Some(MessageInfo {
                    event_id, sender, sender_name, body, formatted_body: None,
                    transformed_body: None, image_mxc: None, reactions, reply_to_id,
                    reply_to_sender: None, reply_to_body: None, timestamp, avatar_url,
                    thread_root, thread_count, image_width: None, image_height: None,
                    poll: Some(PollInfo {
                        question,
                        answers,
                        votes,
                        is_ended,
                        total_votes,
                        my_vote: None,
                    }),
                    is_emote: false, is_video: false,
                })
            } else {
                Some(MessageInfo {
                    event_id, sender, sender_name,
                    body: "[unsupported]".into(), formatted_body: None,
                    transformed_body: None, image_mxc: None, reactions, reply_to_id,
                    reply_to_sender: None, reply_to_body: None, timestamp, avatar_url,
                    thread_root, thread_count, image_width: None, image_height: None, poll: None,
                    is_emote: false, is_video: false,
                })
            }
        }

        TimelineItemContent::MembershipChange(mc) => {
            use matrix_sdk_ui::timeline::MembershipChange as MChange;
            let display = mc.display_name().unwrap_or_else(|| sender_name.clone());
            let body = match mc.change() {
                Some(MChange::Joined) | Some(MChange::InvitationAccepted) => format!("{display} has joined"),
                Some(MChange::Left) => format!("{display} has left"),
                Some(MChange::Banned) => format!("{display} was banned"),
                Some(MChange::Unbanned) => format!("{display} was unbanned"),
                Some(MChange::Kicked) => format!("{display} was kicked"),
                Some(MChange::KickedAndBanned) => format!("{display} was kicked and banned"),
                Some(MChange::Invited) => format!("{display} was invited"),
                Some(MChange::InvitationRejected) => format!("{display} rejected the invite"),
                Some(MChange::InvitationRevoked) => format!("{display}'s invite was revoked"),
                _ => format!("{display} membership changed"),
            };
            Some(MessageInfo {
                event_id: format!("~state-{event_id}"), sender: String::new(), sender_name: String::new(),
                body,
                formatted_body: None, transformed_body: None, image_mxc: None,
                reactions: Vec::new(), reply_to_id: None, reply_to_sender: None,
                reply_to_body: None, timestamp, avatar_url: None,
                thread_root: None, thread_count: None, image_width: None, image_height: None, poll: None,
                is_emote: false, is_video: false,
            })
        }

        TimelineItemContent::ProfileChange(pc) => {
            let display = pc.displayname_change().and_then(|c| c.new.clone())
                .unwrap_or_else(|| sender_name.clone());
            Some(MessageInfo {
                event_id: format!("~state-{event_id}"), sender: String::new(), sender_name: String::new(),
                body: format!("{display} changed their profile"),
                formatted_body: None, transformed_body: None, image_mxc: None,
                reactions: Vec::new(), reply_to_id: None, reply_to_sender: None,
                reply_to_body: None, timestamp, avatar_url: None,
                thread_root: None, thread_count: None, image_width: None, image_height: None, poll: None,
                is_emote: false, is_video: false,
            })
        }

        TimelineItemContent::OtherState(state) => {
            use matrix_sdk_ui::timeline::AnyOtherFullStateEventContent;
            let body = match state.content() {
                AnyOtherFullStateEventContent::RoomPowerLevels(_) => {
                    format!("{sender_name} changed room power levels")
                }
                AnyOtherFullStateEventContent::RoomName(_) => {
                    format!("{sender_name} changed the room name")
                }
                AnyOtherFullStateEventContent::RoomTopic(_) => {
                    format!("{sender_name} changed the room topic")
                }
                AnyOtherFullStateEventContent::RoomAvatar(_) => {
                    format!("{sender_name} changed the room avatar")
                }
                AnyOtherFullStateEventContent::_Custom { ref event_type }
                    if event_type.contains("call") || event_type.contains("rtc") =>
                {
                    return None; // Suppress call/RTC state events
                }
                _ => format!("{sender_name} updated room settings"),
            };
            Some(MessageInfo {
                event_id: format!("~state-{event_id}"), sender: String::new(), sender_name: String::new(),
                body,
                formatted_body: None, transformed_body: None, image_mxc: None,
                reactions: Vec::new(), reply_to_id: None, reply_to_sender: None,
                reply_to_body: None, timestamp, avatar_url: None,
                thread_root: None, thread_count: None, image_width: None, image_height: None, poll: None,
                is_emote: false, is_video: false,
            })
        }

        _ => None,
    }
}

/// Create a thread-focused timeline for a specific thread root.
pub async fn create_thread_timeline(
    client: &matrix_sdk::Client,
    room_id: &matrix_sdk::ruma::RoomId,
    thread_root_id: &matrix_sdk::ruma::EventId,
) -> Result<Arc<Timeline>, crate::error::AppError> {
    use matrix_sdk_ui::timeline::TimelineFocus;
    let room = client.get_room(room_id)
        .ok_or_else(|| crate::error::AppError::RoomNotFound(room_id.to_string()))?;
    let timeline = TimelineBuilder::new(&room)
        .with_focus(TimelineFocus::Thread { root_event_id: thread_root_id.to_owned() })
        .build()
        .await
        .map_err(|e| crate::error::AppError::Other(format!("Failed to build thread timeline: {e}")))?;
    Ok(Arc::new(timeline))
}

/// Batch convert timeline items to MessageInfo vec.
pub fn timeline_items_to_messages(
    items: &[Arc<TimelineItem>],
) -> Vec<MessageInfo> {
    items.iter()
        .filter_map(|item| timeline_item_to_message_info(item))
        .collect()
}
