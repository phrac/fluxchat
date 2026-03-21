use matrix_sdk::{
    room::{reply::Reply, MessagesOptions},
    ruma::events::{
        reaction::ReactionEventContent,
        relation::Annotation,
        room::{
            message::{
                MessageType, Relation, RoomMessageEventContent,
                RoomMessageEventContentWithoutRelation,
            },
            pinned_events::RoomPinnedEventsEventContent,
        },
    },
    RoomMemberships,
};
use tauri::State;

use crate::state::AppState;

use super::{AppError, MemberInfo, MessageInfo, PaginatedMessages};

async fn get_client(state: &State<'_, AppState>) -> Result<matrix_sdk::Client, AppError> {
    let lock = state.client.lock().await;
    lock.as_ref().cloned().ok_or(AppError::NotLoggedIn)
}

/// Extract reply info from a message's relates_to field
fn extract_reply_info(
    relates_to: &Option<Relation<RoomMessageEventContentWithoutRelation>>,
) -> Option<String> {
    match relates_to {
        Some(Relation::Reply { in_reply_to }) => Some(in_reply_to.event_id.to_string()),
        Some(Relation::Thread(thread)) => {
            thread.in_reply_to.as_ref().map(|r| r.event_id.to_string())
        }
        _ => None,
    }
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

            // Extract reply info
            let reply_to_id = extract_reply_info(&msg.content.relates_to);

            messages.push(MessageInfo {
                event_id: msg.event_id.to_string(),
                sender: msg.sender.to_string(),
                sender_name,
                body,
                formatted_body,
                image_mxc,
                reply_to_id,
                reactions: Vec::new(),
                reply_to_sender: None,
                reply_to_body: None,
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
pub async fn send_reply(
    state: State<'_, AppState>,
    room_id: String,
    body: String,
    reply_to_event_id: String,
) -> Result<String, AppError> {
    let client = get_client(&state).await?;

    let room_id = matrix_sdk::ruma::RoomId::parse(&room_id)
        .map_err(|_| AppError::Other("Invalid room ID".into()))?;

    let room = client
        .get_room(&room_id)
        .ok_or(AppError::RoomNotFound(room_id.to_string()))?;

    let event_id = matrix_sdk::ruma::EventId::parse(&reply_to_event_id)
        .map_err(|_| AppError::Other("Invalid event ID".into()))?;

    let content = RoomMessageEventContentWithoutRelation::text_plain(&body);
    let reply = Reply {
        event_id,
        enforce_thread: matrix_sdk::room::reply::EnforceThread::Unthreaded,
    };

    let reply_content = room
        .make_reply_event(content, reply)
        .await
        .map_err(|e| AppError::Other(format!("Failed to create reply: {e}")))?;

    let response = room.send(reply_content).await?;
    Ok(response.event_id.to_string())
}

#[tauri::command]
pub async fn send_typing(
    state: State<'_, AppState>,
    room_id: String,
    typing: bool,
) -> Result<(), AppError> {
    let client = get_client(&state).await?;

    let room_id = matrix_sdk::ruma::RoomId::parse(&room_id)
        .map_err(|_| AppError::Other("Invalid room ID".into()))?;

    let room = client
        .get_room(&room_id)
        .ok_or(AppError::RoomNotFound(room_id.to_string()))?;

    room.typing_notice(typing).await?;
    Ok(())
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

    let store = client.state_store();

    let mut result: Vec<MemberInfo> = Vec::new();
    for m in &members {
        let power_level = match m.power_level() {
            matrix_sdk::ruma::events::room::power_levels::UserPowerLevel::Infinite => 100,
            matrix_sdk::ruma::events::room::power_levels::UserPowerLevel::Int(n) => {
                i64::from(n)
            }
            _ => 0,
        };

        let role = if power_level >= 100 {
            "Admin".to_string()
        } else if power_level >= 50 {
            "Moderator".to_string()
        } else {
            "Member".to_string()
        };

        let presence = match store.get_presence_event(m.user_id()).await {
            Ok(Some(raw)) => raw
                .deserialize()
                .ok()
                .map(|ev| match ev.content.presence {
                    matrix_sdk::ruma::presence::PresenceState::Online => "online",
                    matrix_sdk::ruma::presence::PresenceState::Unavailable => "idle",
                    matrix_sdk::ruma::presence::PresenceState::Offline => "offline",
                    _ => "offline",
                })
                .unwrap_or("offline")
                .to_string(),
            _ => "offline".to_string(),
        };

        let server_name = m.user_id().server_name().to_string();

        result.push(MemberInfo {
            user_id: m.user_id().to_string(),
            display_name: m
                .display_name()
                .map(|s| s.to_string())
                .unwrap_or_else(|| m.user_id().to_string()),
            avatar_url: m.avatar_url().map(|mxc| mxc.to_string()),
            power_level,
            role,
            presence,
            server_name,
        });
    }

    result.sort_by(|a, b| {
        b.power_level
            .cmp(&a.power_level)
            .then_with(|| {
                a.display_name
                    .to_lowercase()
                    .cmp(&b.display_name.to_lowercase())
            })
    });
    Ok(result)
}

#[tauri::command]
pub async fn send_reaction(
    state: State<'_, AppState>,
    room_id: String,
    event_id: String,
    emoji: String,
) -> Result<(), AppError> {
    let client = get_client(&state).await?;

    let room_id = matrix_sdk::ruma::RoomId::parse(&room_id)
        .map_err(|_| AppError::Other("Invalid room ID".into()))?;

    let room = client
        .get_room(&room_id)
        .ok_or(AppError::RoomNotFound(room_id.to_string()))?;

    let event_id = matrix_sdk::ruma::EventId::parse(&event_id)
        .map_err(|_| AppError::Other("Invalid event ID".into()))?;

    let annotation = Annotation::new(event_id, emoji);
    let content = ReactionEventContent::from(annotation);
    room.send(content).await?;

    Ok(())
}

#[tauri::command]
pub async fn delete_message(
    state: State<'_, AppState>,
    room_id: String,
    event_id: String,
) -> Result<(), AppError> {
    let client = get_client(&state).await?;

    let room_id = matrix_sdk::ruma::RoomId::parse(&room_id)
        .map_err(|_| AppError::Other("Invalid room ID".into()))?;

    let room = client
        .get_room(&room_id)
        .ok_or(AppError::RoomNotFound(room_id.to_string()))?;

    let event_id = matrix_sdk::ruma::EventId::parse(&event_id)
        .map_err(|_| AppError::Other("Invalid event ID".into()))?;

    room.redact(&event_id, None, None)
        .await
        .map_err(|e| AppError::Other(format!("Failed to delete message: {e}")))?;

    Ok(())
}

#[tauri::command]
pub async fn get_pinned_messages(
    state: State<'_, AppState>,
    room_id: String,
) -> Result<Vec<MessageInfo>, AppError> {
    let client = get_client(&state).await?;

    let room_id = matrix_sdk::ruma::RoomId::parse(&room_id)
        .map_err(|_| AppError::Other("Invalid room ID".into()))?;

    let room = client
        .get_room(&room_id)
        .ok_or(AppError::RoomNotFound(room_id.to_string()))?;

    // Get pinned event IDs
    let pinned_ids: Vec<matrix_sdk::ruma::OwnedEventId> = room
        .get_state_events_static::<RoomPinnedEventsEventContent>()
        .await
        .ok()
        .and_then(|events| {
            events.into_iter().next().and_then(|e| {
                e.deserialize().ok().and_then(|ev| {
                    ev.as_sync().map(|s| match s {
                        matrix_sdk::ruma::events::SyncStateEvent::Original(o) => {
                            o.content.pinned.clone()
                        }
                        _ => Vec::new(),
                    })
                })
            })
        })
        .unwrap_or_default();

    let mut messages = Vec::new();
    for event_id in &pinned_ids {
        // Fetch each pinned event
        let event = match room.event(event_id, None).await {
            Ok(e) => e,
            Err(_) => continue,
        };

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
                MessageType::Notice(t) => (
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
                _ => ("[message]".into(), None, None),
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
                reactions: Vec::new(),
                reply_to_id: None,
                reply_to_sender: None,
                reply_to_body: None,
                timestamp: msg.origin_server_ts.as_secs().into(),
                avatar_url,
            });
        }
    }

    Ok(messages)
}

#[tauri::command]
pub async fn pin_message(
    state: State<'_, AppState>,
    room_id: String,
    event_id: String,
) -> Result<(), AppError> {
    let client = get_client(&state).await?;

    let room_id = matrix_sdk::ruma::RoomId::parse(&room_id)
        .map_err(|_| AppError::Other("Invalid room ID".into()))?;

    let room = client
        .get_room(&room_id)
        .ok_or(AppError::RoomNotFound(room_id.to_string()))?;

    let event_id = matrix_sdk::ruma::EventId::parse(&event_id)
        .map_err(|_| AppError::Other("Invalid event ID".into()))?;

    // Get current pinned events
    let mut pinned: Vec<matrix_sdk::ruma::OwnedEventId> = room
        .get_state_events_static::<RoomPinnedEventsEventContent>()
        .await
        .ok()
        .and_then(|events| {
            events.into_iter().next().and_then(|e| {
                e.deserialize().ok().and_then(|ev| {
                    ev.as_sync().map(|s| match s {
                        matrix_sdk::ruma::events::SyncStateEvent::Original(o) => {
                            o.content.pinned.clone()
                        }
                        _ => Vec::new(),
                    })
                })
            })
        })
        .unwrap_or_default();

    // Toggle pin
    if pinned.contains(&event_id) {
        pinned.retain(|id| id != &event_id);
    } else {
        pinned.push(event_id);
    }

    let content = RoomPinnedEventsEventContent::new(pinned);
    room.send_state_event(content)
        .await
        .map_err(|e| AppError::Other(format!("Failed to pin message: {e}")))?;

    Ok(())
}
