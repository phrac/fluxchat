use matrix_sdk::ruma::events::space::child::SpaceChildEventContent;
use tauri::State;

use crate::state::AppState;

use super::{AppError, RoomInfo};

async fn get_client(state: &State<'_, AppState>) -> Result<matrix_sdk::Client, AppError> {
    let lock = state.client.lock().await;
    lock.as_ref().cloned().ok_or(AppError::NotLoggedIn)
}

#[tauri::command]
pub async fn get_spaces(state: State<'_, AppState>) -> Result<Vec<RoomInfo>, AppError> {
    let client = get_client(&state).await?;

    let mut spaces = Vec::new();
    for room in client.joined_rooms() {
        if room.is_space() {
            let name = room
                .display_name()
                .await
                .map(|n| n.to_string())
                .unwrap_or_else(|_| room.room_id().to_string());

            spaces.push(RoomInfo {
                room_id: room.room_id().to_string(),
                name,
                is_encrypted: false,
                is_space: true,
                is_direct: false,
                unread_count: 0,
                avatar_url: room.avatar_url().map(|mxc| mxc.to_string()),
                topic: room.topic(),
                space_name: None,
            });
        }
    }

    spaces.sort_by(|a, b| a.name.to_lowercase().cmp(&b.name.to_lowercase()));
    Ok(spaces)
}

#[tauri::command]
pub async fn get_space_rooms(
    state: State<'_, AppState>,
    space_id: String,
) -> Result<Vec<RoomInfo>, AppError> {
    let client = get_client(&state).await?;

    let space_room_id = matrix_sdk::ruma::RoomId::parse(&space_id)
        .map_err(|_| AppError::Other("Invalid space ID".into()))?;

    let space = client
        .get_room(&space_room_id)
        .ok_or(AppError::RoomNotFound(space_id))?;

    let child_events = space
        .get_state_events_static::<SpaceChildEventContent>()
        .await
        .map_err(|e| AppError::Other(e.to_string()))?;

    let mut rooms = Vec::new();
    for event in child_events {
        let event = match event.deserialize() {
            Ok(e) => e,
            Err(_) => continue,
        };

        let child_room_id =
            match matrix_sdk::ruma::OwnedRoomId::try_from(event.state_key().to_string()) {
                Ok(id) => id,
                Err(_) => continue,
            };

        if let Some(room) = client.get_room(&child_room_id) {
            if !room.is_space() {
                let name = room
                    .display_name()
                    .await
                    .map(|n| n.to_string())
                    .unwrap_or_else(|_| room.room_id().to_string());

                let is_encrypted = room.encryption_state().is_encrypted();

                let is_direct = room.is_direct().await.unwrap_or(false);
                rooms.push(RoomInfo {
                    room_id: room.room_id().to_string(),
                    name,
                    is_encrypted,
                    is_space: false,
                    is_direct,
                    unread_count: room.unread_notification_counts().notification_count,
                    avatar_url: room.avatar_url().map(|mxc| mxc.to_string()),
                    topic: room.topic(),
                    space_name: None,
                });
            }
        }
    }

    rooms.sort_by(|a, b| a.name.to_lowercase().cmp(&b.name.to_lowercase()));
    Ok(rooms)
}
