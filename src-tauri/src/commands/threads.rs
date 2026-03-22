use matrix_sdk::{
    room::{RelationsOptions, IncludeRelations},
    ruma::{
        api::Direction,
        events::room::message::MessageType,
    },
};
use tauri::State;

use crate::state::AppState;
use super::{AppError, MessageInfo};

async fn get_client(state: &State<'_, AppState>) -> Result<matrix_sdk::Client, AppError> {
    let lock = state.client.lock().await;
    lock.as_ref().cloned().ok_or(AppError::NotLoggedIn)
}

/// Get messages in a thread (replies to a root event).
#[tauri::command]
pub async fn get_thread_messages(
    state: State<'_, AppState>,
    room_id: String,
    thread_root_id: String,
) -> Result<Vec<MessageInfo>, AppError> {
    let client = get_client(&state).await?;

    let room_id = matrix_sdk::ruma::RoomId::parse(&room_id)
        .map_err(|_| AppError::Other("Invalid room ID".into()))?;

    let room = client
        .get_room(&room_id)
        .ok_or(AppError::RoomNotFound(room_id.to_string()))?;

    let event_id = matrix_sdk::ruma::EventId::parse(&thread_root_id)
        .map_err(|_| AppError::Other("Invalid event ID".into()))?;

    // First, get the root event itself
    let mut messages = Vec::new();

    if let Ok(root_event) = room.event(&event_id, None).await {
        if let Ok(deserialized) = root_event.raw().deserialize() {
            if let Some(msg_info) = extract_message_info(&room, deserialized).await {
                messages.push(msg_info);
            }
        }
    }

    // Then get thread replies
    let options = RelationsOptions {
        from: None,
        dir: Direction::Forward,
        limit: Some(100u32.into()),
        include_relations: IncludeRelations::AllRelations,
        recurse: false,
    };

    match room.relations(event_id.into(), options).await {
        Ok(relations) => {
            for event in &relations.chunk {
                if let Ok(deserialized) = event.raw().deserialize() {
                    if let Some(msg_info) = extract_message_info(&room, deserialized).await {
                        messages.push(msg_info);
                    }
                }
            }
        }
        Err(e) => {
            tracing::warn!("Failed to fetch thread relations: {e}");
        }
    }

    Ok(messages)
}

async fn extract_message_info(
    room: &matrix_sdk::Room,
    event: matrix_sdk::ruma::events::AnySyncTimelineEvent,
) -> Option<MessageInfo> {
    use matrix_sdk::ruma::events::{AnySyncTimelineEvent, AnySyncMessageLikeEvent, SyncMessageLikeEvent};

    if let AnySyncTimelineEvent::MessageLike(
        AnySyncMessageLikeEvent::RoomMessage(
            SyncMessageLikeEvent::Original(msg),
        ),
    ) = event
    {
        let (body, formatted_body, image_mxc) = match &msg.content.msgtype {
            MessageType::Text(t) => (
                t.body.clone(),
                t.formatted.as_ref().map(|f| f.body.clone()),
                None,
            ),
            MessageType::Image(img) => {
                let mxc = match &img.source {
                    matrix_sdk::ruma::events::room::MediaSource::Plain(uri) => Some(uri.to_string()),
                    matrix_sdk::ruma::events::room::MediaSource::Encrypted(enc) => Some(enc.url.to_string()),
                    _ => None,
                };
                (img.body.clone(), None, mxc)
            }
            MessageType::Notice(t) => (
                t.body.clone(),
                t.formatted.as_ref().map(|f| f.body.clone()),
                None,
            ),
            _ => return None,
        };

        let member = room.get_member_no_sync(&msg.sender).await.ok().flatten();
        let sender_name = member
            .as_ref()
            .and_then(|m| m.display_name().map(|s| s.to_string()))
            .unwrap_or_else(|| msg.sender.to_string());
        let avatar_url = member
            .as_ref()
            .and_then(|m| m.avatar_url())
            .map(|mxc| mxc.to_string());

        Some(MessageInfo {
            event_id: msg.event_id.to_string(),
            sender: msg.sender.to_string(),
            sender_name,
            body,
            formatted_body,
            image_mxc,
            reactions: Vec::new(),
            reply_to_id: None,
            reply_to_sender: None,
            reply_to_body: None,
            timestamp: msg.origin_server_ts.as_secs().into(),
            avatar_url,
        })
    } else {
        None
    }
}
