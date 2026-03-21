use matrix_sdk::ruma::{events::space::child::SpaceChildEventContent, RoomOrAliasId};
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
