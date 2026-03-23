use std::collections::HashMap;
use std::sync::Arc;
use matrix_sdk::ruma::OwnedRoomId;
use matrix_sdk_ui::timeline::{
    Timeline, TimelineBuilder, TimelineItem, TimelineItemKind, TimelineItemContent,
    EventTimelineItem,
};

use crate::models::{MessageInfo, ReactionInfo};

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
        TimelineItemContent::MsgLike(_) => {
            if content.is_redacted() {
                return None;
            }

            if let Some(message) = content.as_message() {
                let body = message.body().to_string();
                let (formatted_body, image_mxc) = match message.msgtype() {
                    matrix_sdk::ruma::events::room::message::MessageType::Text(t) => {
                        (t.formatted.as_ref().map(|f| f.body.clone()), None)
                    }
                    matrix_sdk::ruma::events::room::message::MessageType::Image(img) => {
                        let mxc = match &img.source {
                            matrix_sdk::ruma::events::room::MediaSource::Plain(uri) => Some(uri.to_string()),
                            matrix_sdk::ruma::events::room::MediaSource::Encrypted(enc) => Some(enc.url.to_string()),
                        };
                        (None, mxc)
                    }
                    matrix_sdk::ruma::events::room::message::MessageType::Notice(t) => {
                        (t.formatted.as_ref().map(|f| f.body.clone()), None)
                    }
                    matrix_sdk::ruma::events::room::message::MessageType::Emote(t) => {
                        (t.formatted.as_ref().map(|f| format!("* {}", f.body)), None)
                    }
                    _ => (None, None),
                };

                Some(MessageInfo {
                    event_id, sender, sender_name, body, formatted_body,
                    transformed_body: None, image_mxc, reactions, reply_to_id,
                    reply_to_sender: None, reply_to_body: None, timestamp, avatar_url,
                })
            } else {
                Some(MessageInfo {
                    event_id, sender, sender_name,
                    body: "[unsupported]".into(), formatted_body: None,
                    transformed_body: None, image_mxc: None, reactions, reply_to_id,
                    reply_to_sender: None, reply_to_body: None, timestamp, avatar_url,
                })
            }
        }

        TimelineItemContent::MembershipChange(mc) => {
            let display = mc.display_name().unwrap_or_else(|| sender_name.clone());
            Some(MessageInfo {
                event_id: format!("~state-{event_id}"), sender: String::new(), sender_name: String::new(),
                body: format!("{display} membership changed"),
                formatted_body: None, transformed_body: None, image_mxc: None,
                reactions: Vec::new(), reply_to_id: None, reply_to_sender: None,
                reply_to_body: None, timestamp, avatar_url: None,
            })
        }

        _ => None,
    }
}

/// Batch convert timeline items to MessageInfo vec.
pub fn timeline_items_to_messages(
    items: &[Arc<TimelineItem>],
) -> Vec<MessageInfo> {
    items.iter()
        .filter_map(|item| timeline_item_to_message_info(item))
        .collect()
}
