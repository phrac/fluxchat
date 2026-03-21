use matrix_sdk::{
    room::MessagesOptions,
    ruma::events::room::message::{MessageType, RoomMessageEventContent},
    RoomMemberships,
};
use tauri::State;

use crate::state::AppState;

use super::{AppError, MemberInfo, MessageInfo, PaginatedMessages};

/// Clone the client and drop the lock immediately so other commands aren't blocked.
async fn get_client(state: &State<'_, AppState>) -> Result<matrix_sdk::Client, AppError> {
    let lock = state.client.lock().await;
    lock.as_ref().cloned().ok_or(AppError::NotLoggedIn)
}

#[tauri::command]
pub async fn get_room_messages(
    state: State<'_, AppState>,
    room_id: String,
    from_token: Option<String>,
) -> Result<PaginatedMessages, AppError> {
    let client = get_client(&state).await?;

    let room_id = matrix_sdk::ruma::RoomId::parse(&room_id)
        .map_err(|_| AppError::Other("Invalid room ID".into()))?;

    let room = client
        .get_room(&room_id)
        .ok_or(AppError::RoomNotFound(room_id.to_string()))?;

    let mut options = MessagesOptions::backward();
    options.limit = 50u32.into();
    if let Some(token) = from_token {
        options.from = Some(token);
    }

    let response = room.messages(options).await?;

    let mut messages = Vec::new();
    for event in &response.chunk {
        let raw = event.raw();
        let deserialized = match raw.deserialize() {
            Ok(ev) => ev,
            Err(_) => continue,
        };

        use matrix_sdk::ruma::events::AnySyncTimelineEvent;
        if let AnySyncTimelineEvent::MessageLike(
            matrix_sdk::ruma::events::AnySyncMessageLikeEvent::RoomMessage(
                matrix_sdk::ruma::events::SyncMessageLikeEvent::Original(msg),
            ),
        ) = deserialized
        {
            let (body, formatted_body, image_mxc) = match &msg.content.msgtype {
                MessageType::Text(t) => (
                    t.body.clone(),
                    t.formatted.as_ref().map(|f| f.body.clone()),
                    None,
                ),
                MessageType::Image(img) => {
                    let mxc = match &img.source {
                        matrix_sdk::ruma::events::room::MediaSource::Plain(uri) => {
                            Some(uri.to_string())
                        }
                        _ => None,
                    };
                    (img.body.clone(), None, mxc)
                }
                MessageType::Video(_) => ("[video]".into(), None, None),
                MessageType::Audio(_) => ("[audio]".into(), None, None),
                MessageType::File(_) => ("[file]".into(), None, None),
                MessageType::Notice(t) => (
                    t.body.clone(),
                    t.formatted.as_ref().map(|f| f.body.clone()),
                    None,
                ),
                MessageType::Emote(t) => (
                    format!("* {}", t.body),
                    t.formatted.as_ref().map(|f| format!("* {}", f.body)),
                    None,
                ),
                _ => ("[unsupported message]".into(), None, None),
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

            messages.push(MessageInfo {
                event_id: msg.event_id.to_string(),
                sender: msg.sender.to_string(),
                sender_name,
                body,
                formatted_body,
                image_mxc,
                timestamp: msg.origin_server_ts.as_secs().into(),
                avatar_url,
            });
        }
    }

    Ok(PaginatedMessages {
        messages,
        end_token: response.end,
    })
}

#[tauri::command]
pub async fn send_message(
    state: State<'_, AppState>,
    room_id: String,
    body: String,
) -> Result<String, AppError> {
    let client = get_client(&state).await?;

    let room_id = matrix_sdk::ruma::RoomId::parse(&room_id)
        .map_err(|_| AppError::Other("Invalid room ID".into()))?;

    let room = client
        .get_room(&room_id)
        .ok_or(AppError::RoomNotFound(room_id.to_string()))?;

    let content = RoomMessageEventContent::text_plain(&body);
    let response = room.send(content).await?;

    Ok(response.event_id.to_string())
}

#[tauri::command]
pub async fn get_room_members(
    state: State<'_, AppState>,
    room_id: String,
) -> Result<Vec<MemberInfo>, AppError> {
    let client = get_client(&state).await?;

    let room_id = matrix_sdk::ruma::RoomId::parse(&room_id)
        .map_err(|_| AppError::Other("Invalid room ID".into()))?;

    let room = client
        .get_room(&room_id)
        .ok_or(AppError::RoomNotFound(room_id.to_string()))?;

    let members = room.members(RoomMemberships::JOIN).await?;

    let mut result: Vec<MemberInfo> = members
        .iter()
        .map(|m| MemberInfo {
            user_id: m.user_id().to_string(),
            display_name: m
                .display_name()
                .map(|s| s.to_string())
                .unwrap_or_else(|| m.user_id().to_string()),
            avatar_url: m.avatar_url().map(|mxc| mxc.to_string()),
        })
        .collect();

    result.sort_by(|a, b| {
        a.display_name
            .to_lowercase()
            .cmp(&b.display_name.to_lowercase())
    });
    Ok(result)
}
