use matrix_sdk::ruma::{
    api::client::room::create_room::v3::{Request as CreateRoomRequest, RoomPreset},
    events::space::child::SpaceChildEventContent,
    RoomOrAliasId,
};
use std::collections::HashMap;
use tauri::State;

use crate::state::AppState;

use super::{AppError, RoomInfo};

async fn get_client(state: &State<'_, AppState>) -> Result<matrix_sdk::Client, AppError> {
    let lock = state.client.lock().await;
    lock.as_ref().cloned().ok_or(AppError::NotLoggedIn)
}

/// Build a map of room_id -> space display name by scanning all joined spaces.
async fn build_space_map(client: &matrix_sdk::Client) -> HashMap<String, String> {
    let mut map = HashMap::new();

    for room in client.joined_rooms() {
        if !room.is_space() {
            continue;
        }

        let space_name = room
            .display_name()
            .await
            .map(|n| n.to_string())
            .unwrap_or_default();

        // Read m.space.child state events to find child room IDs
        let child_events = match room
            .get_state_events_static::<SpaceChildEventContent>()
            .await
        {
            Ok(events) => events,
            Err(_) => continue,
        };

        for event in child_events {
            if let Ok(ev) = event.deserialize() {
                let child_id = ev.state_key().to_string();
                // Only set if not already mapped (first space wins)
                map.entry(child_id).or_insert_with(|| space_name.clone());
            }
        }
    }

    map
}

async fn room_to_info(
    room: &matrix_sdk::Room,
    space_name: Option<String>,
) -> RoomInfo {
    let name = room
        .display_name()
        .await
        .map(|n| n.to_string())
        .unwrap_or_else(|_| room.room_id().to_string());

    let topic = room.topic();
    let is_encrypted = room.encryption_state().is_encrypted();
    let is_space = room.is_space();
    let is_direct = room.is_direct().await.unwrap_or(false);
    let unread_count = room.unread_notification_counts().notification_count;
    let avatar_url = room.avatar_url().map(|mxc| mxc.to_string());

    RoomInfo {
        room_id: room.room_id().to_string(),
        name,
        is_encrypted,
        is_space,
        is_direct,
        unread_count,
        avatar_url,
        topic,
        space_name,
    }
}

#[tauri::command]
pub async fn get_rooms(state: State<'_, AppState>) -> Result<Vec<RoomInfo>, AppError> {
    let client = get_client(&state).await?;

    let space_map = build_space_map(&client).await;

    let mut rooms = Vec::new();
    for room in client.joined_rooms() {
        if !room.is_space() {
            let space_name = space_map.get(&room.room_id().to_string()).cloned();
            rooms.push(room_to_info(&room, space_name).await);
        }
    }

    // Sort: DMs last, then by space name (None first), then by room name
    rooms.sort_by(|a, b| {
        a.is_direct
            .cmp(&b.is_direct)
            .then_with(|| a.space_name.cmp(&b.space_name))
            .then_with(|| a.name.to_lowercase().cmp(&b.name.to_lowercase()))
    });

    Ok(rooms)
}

#[tauri::command]
pub async fn join_room(
    state: State<'_, AppState>,
    room_id_or_alias: String,
) -> Result<RoomInfo, AppError> {
    let client = get_client(&state).await?;

    let room_or_alias = <&RoomOrAliasId>::try_from(room_id_or_alias.as_str())
        .map_err(|_| AppError::Other("Invalid room ID or alias".into()))?;

    let room = client.join_room_by_id_or_alias(room_or_alias, &[]).await?;

    Ok(room_to_info(&room, None).await)
}

#[tauri::command]
pub async fn leave_room(state: State<'_, AppState>, room_id: String) -> Result<(), AppError> {
    let client = get_client(&state).await?;

    let room_id = matrix_sdk::ruma::RoomId::parse(&room_id)
        .map_err(|_| AppError::Other("Invalid room ID".into()))?;

    let room = client
        .get_room(&room_id)
        .ok_or(AppError::RoomNotFound(room_id.to_string()))?;

    room.leave().await?;
    Ok(())
}

#[tauri::command]
pub async fn invite_user(
    state: State<'_, AppState>,
    room_id: String,
    user_id: String,
) -> Result<(), AppError> {
    let client = get_client(&state).await?;

    let room_id = matrix_sdk::ruma::RoomId::parse(&room_id)
        .map_err(|_| AppError::Other("Invalid room ID".into()))?;

    let user_id = matrix_sdk::ruma::UserId::parse(&user_id)
        .map_err(|_| AppError::Other("Invalid user ID".into()))?;

    let room = client
        .get_room(&room_id)
        .ok_or(AppError::RoomNotFound(room_id.to_string()))?;

    room.invite_user_by_id(&user_id).await?;
    Ok(())
}

#[tauri::command]
pub async fn create_room(
    state: State<'_, AppState>,
    name: String,
    topic: Option<String>,
    is_public: bool,
    is_encrypted: bool,
    invite_user_ids: Vec<String>,
) -> Result<String, AppError> {
    let client = get_client(&state).await?;

    let mut request = CreateRoomRequest::new();
    request.name = Some(name);
    request.topic = topic;
    request.preset = Some(if is_public {
        RoomPreset::PublicChat
    } else {
        RoomPreset::PrivateChat
    });

    if is_public {
        request.visibility = matrix_sdk::ruma::api::client::room::Visibility::Public;
    }

    // Parse invite user IDs
    for uid in &invite_user_ids {
        if let Ok(user_id) = matrix_sdk::ruma::UserId::parse(uid) {
            request.invite.push(user_id);
        }
    }

    let room = client
        .create_room(request)
        .await
        .map_err(|e| AppError::Other(format!("Failed to create room: {e}")))?;

    // Enable encryption after creation if requested
    if is_encrypted {
        use matrix_sdk::ruma::events::room::encryption::RoomEncryptionEventContent;
        let _ = room.send_state_event(RoomEncryptionEventContent::with_recommended_defaults()).await;
    }

    Ok(room.room_id().to_string())
}

#[tauri::command]
pub async fn set_room_name(
    state: State<'_, AppState>,
    room_id: String,
    name: String,
) -> Result<(), AppError> {
    let client = get_client(&state).await?;

    let room_id = matrix_sdk::ruma::RoomId::parse(&room_id)
        .map_err(|_| AppError::Other("Invalid room ID".into()))?;

    let room = client
        .get_room(&room_id)
        .ok_or(AppError::RoomNotFound(room_id.to_string()))?;

    room.set_name(name)
        .await
        .map_err(|e| AppError::Other(format!("Failed to set name: {e}")))?;

    Ok(())
}

#[tauri::command]
pub async fn set_room_topic(
    state: State<'_, AppState>,
    room_id: String,
    topic: String,
) -> Result<(), AppError> {
    let client = get_client(&state).await?;

    let room_id = matrix_sdk::ruma::RoomId::parse(&room_id)
        .map_err(|_| AppError::Other("Invalid room ID".into()))?;

    let room = client
        .get_room(&room_id)
        .ok_or(AppError::RoomNotFound(room_id.to_string()))?;

    room.set_room_topic(&topic)
        .await
        .map_err(|e| AppError::Other(format!("Failed to set topic: {e}")))?;

    Ok(())
}

#[tauri::command]
pub async fn kick_user(
    state: State<'_, AppState>,
    room_id: String,
    user_id: String,
    reason: Option<String>,
) -> Result<(), AppError> {
    let client = get_client(&state).await?;
    let room_id = matrix_sdk::ruma::RoomId::parse(&room_id)
        .map_err(|_| AppError::Other("Invalid room ID".into()))?;
    let user_id = matrix_sdk::ruma::UserId::parse(&user_id)
        .map_err(|_| AppError::Other("Invalid user ID".into()))?;
    let room = client.get_room(&room_id)
        .ok_or(AppError::RoomNotFound(room_id.to_string()))?;

    room.kick_user(&user_id, reason.as_deref())
        .await
        .map_err(|e| AppError::Other(format!("Failed to kick: {e}")))?;
    Ok(())
}

#[tauri::command]
pub async fn ban_user(
    state: State<'_, AppState>,
    room_id: String,
    user_id: String,
    reason: Option<String>,
) -> Result<(), AppError> {
    let client = get_client(&state).await?;
    let room_id = matrix_sdk::ruma::RoomId::parse(&room_id)
        .map_err(|_| AppError::Other("Invalid room ID".into()))?;
    let user_id = matrix_sdk::ruma::UserId::parse(&user_id)
        .map_err(|_| AppError::Other("Invalid user ID".into()))?;
    let room = client.get_room(&room_id)
        .ok_or(AppError::RoomNotFound(room_id.to_string()))?;

    room.ban_user(&user_id, reason.as_deref())
        .await
        .map_err(|e| AppError::Other(format!("Failed to ban: {e}")))?;
    Ok(())
}

#[tauri::command]
pub async fn set_user_power_level(
    state: State<'_, AppState>,
    room_id: String,
    user_id: String,
    power_level: i64,
) -> Result<(), AppError> {
    let client = get_client(&state).await?;
    let room_id = matrix_sdk::ruma::RoomId::parse(&room_id)
        .map_err(|_| AppError::Other("Invalid room ID".into()))?;
    let user_id = matrix_sdk::ruma::UserId::parse(&user_id)
        .map_err(|_| AppError::Other("Invalid user ID".into()))?;
    let room = client.get_room(&room_id)
        .ok_or(AppError::RoomNotFound(room_id.to_string()))?;

    use matrix_sdk::ruma::Int;

    room.update_power_levels(vec![(&user_id, Int::new(power_level).unwrap_or(Int::MIN))])
        .await?;

    Ok(())
}
