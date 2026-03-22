use matrix_sdk::ruma::{
    api::client::room::create_room::v3::{Request as CreateRoomRequest, RoomPreset},
    events::space::child::SpaceChildEventContent,
    Int, RoomOrAliasId,
};
use std::collections::HashMap;

use crate::error::AppError;
use crate::models::RoomInfo;

/// Build a map of room_id -> space display name by scanning all joined spaces.
pub async fn build_space_map(client: &matrix_sdk::Client) -> HashMap<String, String> {
    let mut map = HashMap::new();
    for room in client.joined_rooms() {
        if !room.is_space() { continue; }
        let space_name = room.display_name().await
            .map(|n| n.to_string())
            .unwrap_or_default();
        let child_events = match room
            .get_state_events_static::<SpaceChildEventContent>().await
        {
            Ok(events) => events,
            Err(_) => continue,
        };
        for event in child_events {
            if let Ok(ev) = event.deserialize() {
                let child_id = ev.state_key().to_string();
                map.entry(child_id).or_insert_with(|| space_name.clone());
            }
        }
    }
    map
}

pub async fn room_to_info(room: &matrix_sdk::Room, space_name: Option<String>) -> RoomInfo {
    let name = room.display_name().await
        .map(|n| n.to_string())
        .unwrap_or_else(|_| room.room_id().to_string());
    RoomInfo {
        room_id: room.room_id().to_string(),
        name,
        is_encrypted: room.encryption_state().is_encrypted(),
        is_space: room.is_space(),
        is_direct: room.is_direct().await.unwrap_or(false),
        unread_count: room.unread_notification_counts().notification_count,
        avatar_url: room.avatar_url().map(|mxc| mxc.to_string()),
        topic: room.topic(),
        space_name,
    }
}

pub async fn get_rooms(client: &matrix_sdk::Client) -> Result<Vec<RoomInfo>, AppError> {
    let space_map = build_space_map(client).await;
    let mut rooms = Vec::new();
    for room in client.joined_rooms() {
        if !room.is_space() {
            let space_name = space_map.get(&room.room_id().to_string()).cloned();
            rooms.push(room_to_info(&room, space_name).await);
        }
    }
    rooms.sort_by(|a, b| a.is_direct.cmp(&b.is_direct)
        .then_with(|| a.space_name.cmp(&b.space_name))
        .then_with(|| a.name.to_lowercase().cmp(&b.name.to_lowercase())));
    Ok(rooms)
}

pub async fn get_spaces(client: &matrix_sdk::Client) -> Result<Vec<RoomInfo>, AppError> {
    let mut spaces = Vec::new();
    for room in client.joined_rooms() {
        if room.is_space() {
            spaces.push(room_to_info(&room, None).await);
        }
    }
    spaces.sort_by(|a, b| a.name.to_lowercase().cmp(&b.name.to_lowercase()));
    Ok(spaces)
}

pub async fn get_space_rooms(client: &matrix_sdk::Client, space_id: &str) -> Result<Vec<RoomInfo>, AppError> {
    let space_room_id = matrix_sdk::ruma::RoomId::parse(space_id)
        .map_err(|_| AppError::Other("Invalid space ID".into()))?;
    let space = client.get_room(&space_room_id)
        .ok_or(AppError::RoomNotFound(space_id.into()))?;
    let child_events = space.get_state_events_static::<SpaceChildEventContent>().await
        .map_err(|e| AppError::Other(e.to_string()))?;
    let mut rooms = Vec::new();
    for event in child_events {
        let ev = match event.deserialize() { Ok(e) => e, Err(_) => continue };
        let child_room_id = match matrix_sdk::ruma::OwnedRoomId::try_from(ev.state_key().to_string()) {
            Ok(id) => id, Err(_) => continue,
        };
        if let Some(room) = client.get_room(&child_room_id) {
            if !room.is_space() {
                rooms.push(room_to_info(&room, None).await);
            }
        }
    }
    rooms.sort_by(|a, b| a.name.to_lowercase().cmp(&b.name.to_lowercase()));
    Ok(rooms)
}

pub async fn join_room(client: &matrix_sdk::Client, room_id_or_alias: &str) -> Result<RoomInfo, AppError> {
    let room_or_alias = <&RoomOrAliasId>::try_from(room_id_or_alias)
        .map_err(|_| AppError::Other("Invalid room ID or alias".into()))?;
    let room = client.join_room_by_id_or_alias(room_or_alias, &[]).await?;
    Ok(room_to_info(&room, None).await)
}

pub async fn leave_room(client: &matrix_sdk::Client, room_id: &str) -> Result<(), AppError> {
    let room_id = matrix_sdk::ruma::RoomId::parse(room_id)
        .map_err(|_| AppError::Other("Invalid room ID".into()))?;
    let room = client.get_room(&room_id).ok_or(AppError::RoomNotFound(room_id.to_string()))?;
    room.leave().await?;
    Ok(())
}

pub async fn invite_user(client: &matrix_sdk::Client, room_id: &str, user_id: &str) -> Result<(), AppError> {
    let room_id = matrix_sdk::ruma::RoomId::parse(room_id).map_err(|_| AppError::Other("Invalid room ID".into()))?;
    let user_id = matrix_sdk::ruma::UserId::parse(user_id).map_err(|_| AppError::Other("Invalid user ID".into()))?;
    let room = client.get_room(&room_id).ok_or(AppError::RoomNotFound(room_id.to_string()))?;
    room.invite_user_by_id(&user_id).await?;
    Ok(())
}

pub async fn kick_user(client: &matrix_sdk::Client, room_id: &str, user_id: &str, reason: Option<&str>) -> Result<(), AppError> {
    let room_id = matrix_sdk::ruma::RoomId::parse(room_id).map_err(|_| AppError::Other("Invalid room ID".into()))?;
    let user_id = matrix_sdk::ruma::UserId::parse(user_id).map_err(|_| AppError::Other("Invalid user ID".into()))?;
    let room = client.get_room(&room_id).ok_or(AppError::RoomNotFound(room_id.to_string()))?;
    room.kick_user(&user_id, reason).await.map_err(|e| AppError::Other(format!("Failed to kick: {e}")))?;
    Ok(())
}

pub async fn ban_user(client: &matrix_sdk::Client, room_id: &str, user_id: &str, reason: Option<&str>) -> Result<(), AppError> {
    let room_id = matrix_sdk::ruma::RoomId::parse(room_id).map_err(|_| AppError::Other("Invalid room ID".into()))?;
    let user_id = matrix_sdk::ruma::UserId::parse(user_id).map_err(|_| AppError::Other("Invalid user ID".into()))?;
    let room = client.get_room(&room_id).ok_or(AppError::RoomNotFound(room_id.to_string()))?;
    room.ban_user(&user_id, reason).await.map_err(|e| AppError::Other(format!("Failed to ban: {e}")))?;
    Ok(())
}

pub async fn set_user_power_level(client: &matrix_sdk::Client, room_id: &str, user_id: &str, power_level: i64) -> Result<(), AppError> {
    let room_id = matrix_sdk::ruma::RoomId::parse(room_id).map_err(|_| AppError::Other("Invalid room ID".into()))?;
    let user_id = matrix_sdk::ruma::UserId::parse(user_id).map_err(|_| AppError::Other("Invalid user ID".into()))?;
    let room = client.get_room(&room_id).ok_or(AppError::RoomNotFound(room_id.to_string()))?;
    room.update_power_levels(vec![(&user_id, Int::new(power_level).unwrap_or(Int::MIN))]).await?;
    Ok(())
}

pub async fn set_room_name(client: &matrix_sdk::Client, room_id: &str, name: &str) -> Result<(), AppError> {
    let room_id = matrix_sdk::ruma::RoomId::parse(room_id).map_err(|_| AppError::Other("Invalid room ID".into()))?;
    let room = client.get_room(&room_id).ok_or(AppError::RoomNotFound(room_id.to_string()))?;
    room.set_name(name.to_string()).await.map_err(|e| AppError::Other(format!("Failed to set name: {e}")))?;
    Ok(())
}

pub async fn set_room_topic(client: &matrix_sdk::Client, room_id: &str, topic: &str) -> Result<(), AppError> {
    let room_id = matrix_sdk::ruma::RoomId::parse(room_id).map_err(|_| AppError::Other("Invalid room ID".into()))?;
    let room = client.get_room(&room_id).ok_or(AppError::RoomNotFound(room_id.to_string()))?;
    room.set_room_topic(topic).await.map_err(|e| AppError::Other(format!("Failed to set topic: {e}")))?;
    Ok(())
}

pub async fn create_room(
    client: &matrix_sdk::Client,
    name: &str,
    topic: Option<&str>,
    is_public: bool,
    is_encrypted: bool,
    invite_user_ids: &[String],
    space_id: Option<&str>,
) -> Result<String, AppError> {
    let mut request = CreateRoomRequest::new();
    request.name = Some(name.to_string());
    request.topic = topic.map(|s| s.to_string());
    request.preset = Some(if is_public { RoomPreset::PublicChat } else { RoomPreset::PrivateChat });
    if is_public {
        request.visibility = matrix_sdk::ruma::api::client::room::Visibility::Public;
    }
    for uid in invite_user_ids {
        if let Ok(user_id) = matrix_sdk::ruma::UserId::parse(uid) {
            request.invite.push(user_id);
        }
    }
    let room = client.create_room(request).await
        .map_err(|e| AppError::Other(format!("Failed to create room: {e}")))?;

    // Add to space if specified
    if let Some(sid) = space_id {
        if let Ok(space_room_id) = matrix_sdk::ruma::RoomId::parse(sid) {
            if let Some(space) = client.get_room(&space_room_id) {
                let mut content = SpaceChildEventContent::new(Vec::new());
                if let Some(host) = client.homeserver().host_str() {
                    if let Ok(server) = matrix_sdk::ruma::ServerName::parse(host) {
                        content.via = vec![server.to_owned()];
                    }
                }
                let _ = space.send_state_event_for_key(room.room_id(), content).await;
            }
        }
    }

    // Enable encryption if requested
    if is_encrypted {
        use matrix_sdk::ruma::events::room::encryption::RoomEncryptionEventContent;
        let _ = room.send_state_event(RoomEncryptionEventContent::with_recommended_defaults()).await;
    }

    Ok(room.room_id().to_string())
}
