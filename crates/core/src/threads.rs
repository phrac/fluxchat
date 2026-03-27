use matrix_sdk::{
    room::{RelationsOptions, IncludeRelations},
    ruma::api::Direction,
};

use crate::error::AppError;
use crate::messages::extract_message_content;
use crate::models::MessageInfo;

/// Fetch all messages in a thread, including the root event.
pub async fn get_thread_messages(client: &matrix_sdk::Client, room_id: &str, thread_root_id: &str) -> Result<Vec<MessageInfo>, AppError> {
    let room_id = matrix_sdk::ruma::RoomId::parse(room_id).map_err(|_| AppError::Other("Invalid room ID".into()))?;
    let room = client.get_room(&room_id).ok_or(AppError::RoomNotFound(room_id.to_string()))?;
    let event_id = matrix_sdk::ruma::EventId::parse(thread_root_id).map_err(|_| AppError::Other("Invalid event ID".into()))?;

    let mut messages = Vec::new();

    // Root event
    if let Ok(root) = room.event(&event_id, None).await {
        if let Ok(ev) = root.raw().deserialize() {
            if let Some(info) = extract_msg(&room, ev).await { messages.push(info); }
        }
    }

    // Thread replies
    let options = RelationsOptions { from: None, dir: Direction::Forward, limit: Some(100u32.into()), include_relations: IncludeRelations::AllRelations, recurse: false };
    if let Ok(relations) = room.relations(event_id, options).await {
        for event in &relations.chunk {
            if let Ok(ev) = event.raw().deserialize() {
                if let Some(info) = extract_msg(&room, ev).await { messages.push(info); }
            }
        }
    }
    Ok(messages)
}

async fn extract_msg(room: &matrix_sdk::Room, event: matrix_sdk::ruma::events::AnySyncTimelineEvent) -> Option<MessageInfo> {
    use matrix_sdk::ruma::events::{AnySyncTimelineEvent, AnySyncMessageLikeEvent, SyncMessageLikeEvent};
    if let AnySyncTimelineEvent::MessageLike(AnySyncMessageLikeEvent::RoomMessage(SyncMessageLikeEvent::Original(msg))) = event {
        let (body, formatted_body, image_mxc) = extract_message_content(&msg.content.msgtype);
        let member = room.get_member_no_sync(&msg.sender).await.ok().flatten();
        Some(MessageInfo {
            event_id: msg.event_id.to_string(), sender: msg.sender.to_string(),
            sender_name: member.as_ref().and_then(|m| m.display_name().map(|s| s.to_string())).unwrap_or_else(|| msg.sender.to_string()),
            body, formatted_body, transformed_body: None, image_mxc, reactions: Vec::new(),
            reply_to_id: None, reply_to_sender: None, reply_to_body: None,
            timestamp: msg.origin_server_ts.as_secs().into(),
            avatar_url: member.as_ref().and_then(|m| m.avatar_url()).map(|mxc| mxc.to_string()),
            thread_root: None, thread_count: None, image_width: None, image_height: None, poll: None, is_emote: false, is_video: false,
        })
    } else { None }
}
