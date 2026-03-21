use matrix_sdk::ruma::RoomOrAliasId;
use tauri::State;

use crate::state::AppState;

use super::{AppError, RoomInfo};

async fn get_client(state: &State<'_, AppState>) -> Result<matrix_sdk::Client, AppError> {
    let lock = state.client.lock().await;
    lock.as_ref().cloned().ok_or(AppError::NotLoggedIn)
}

async fn room_to_info(room: &matrix_sdk::Room) -> RoomInfo {
    let name = room
        .display_name()
        .await
        .map(|n| n.to_string())
        .unwrap_or_else(|_| room.room_id().to_string());

    let topic = room.topic();
    let is_encrypted = room.encryption_state().is_encrypted();
    let is_space = room.is_space();
    let unread_count = room.unread_notification_counts().notification_count;
    let avatar_url = room.avatar_url().map(|mxc| mxc.to_string());

    RoomInfo {
        room_id: room.room_id().to_string(),
        name,
        is_encrypted,
        is_space,
        unread_count,
        avatar_url,
        topic,
    }
}

#[tauri::command]
pub async fn get_rooms(state: State<'_, AppState>) -> Result<Vec<RoomInfo>, AppError> {
    let client = get_client(&state).await?;

    let mut rooms = Vec::new();
    for room in client.joined_rooms() {
        if !room.is_space() {
            rooms.push(room_to_info(&room).await);
        }
    }

    rooms.sort_by(|a, b| a.name.to_lowercase().cmp(&b.name.to_lowercase()));
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

    Ok(room_to_info(&room).await)
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
