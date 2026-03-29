use matrix_sdk::ruma::{
    api::client::room::create_room::v3::{Request as CreateRoomRequest, RoomPreset},
    events::space::child::SpaceChildEventContent,
    Int, RoomOrAliasId,
};
use std::collections::HashMap;

use crate::error::AppError;
use crate::models::RoomInfo;

/// Parse a room ID string, returning AppError on failure.
pub fn parse_room_id(room_id: &str) -> Result<matrix_sdk::ruma::OwnedRoomId, AppError> {
    matrix_sdk::ruma::RoomId::parse(room_id).map_err(|_| AppError::Other(format!("Invalid room ID: {room_id}")))
}

/// Parse a user ID string, returning AppError on failure.
pub fn parse_user_id(user_id: &str) -> Result<matrix_sdk::ruma::OwnedUserId, AppError> {
    matrix_sdk::ruma::UserId::parse(user_id).map_err(|_| AppError::Other(format!("Invalid user ID: {user_id}")))
}

/// Build a map of room_id -> space display name by scanning all joined spaces.
pub async fn build_space_map(client: &matrix_sdk::Client) -> HashMap<String, (String, String)> {
    let mut map = HashMap::new();
    for room in client.joined_rooms() {
        if !room.is_space() { continue; }
        let space_name = room.display_name().await
            .map(|n| n.to_string())
            .unwrap_or_default();
        let space_id = room.room_id().to_string();
        let child_events = match room
            .get_state_events_static::<SpaceChildEventContent>().await
        {
            Ok(events) => events,
            Err(_) => continue,
        };
        for event in child_events {
            if let Ok(ev) = event.deserialize() {
                let child_id = ev.state_key().to_string();
                map.entry(child_id).or_insert_with(|| (space_name.clone(), space_id.clone()));
            }
        }
    }
    map
}

/// Convert a Matrix Room into our RoomInfo model.
pub async fn room_to_info(room: &matrix_sdk::Room, space_info: Option<(String, String)>) -> RoomInfo {
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
        highlight_count: room.unread_notification_counts().highlight_count,
        avatar_url: room.avatar_url().map(|mxc| mxc.to_string()),
        topic: room.topic(),
        space_name: space_info.as_ref().map(|(name, _)| name.clone()),
        space_id: space_info.as_ref().map(|(_, id)| id.clone()),
        is_favourite: room.is_favourite(),
    }
}

/// Get all joined rooms (excluding spaces), sorted by category.
pub async fn get_rooms(client: &matrix_sdk::Client) -> Result<Vec<RoomInfo>, AppError> {
    let space_map = build_space_map(client).await;
    let mut rooms = Vec::new();
    for room in client.joined_rooms() {
        if !room.is_space() {
            let space_info = space_map.get(&room.room_id().to_string()).cloned();
            rooms.push(room_to_info(&room, space_info).await);
        }
    }
    rooms.sort_by(|a, b| a.is_direct.cmp(&b.is_direct)
        .then_with(|| b.is_favourite.cmp(&a.is_favourite))
        .then_with(|| a.space_name.cmp(&b.space_name))
        .then_with(|| a.name.to_lowercase().cmp(&b.name.to_lowercase())));
    Ok(rooms)
}

/// Get all joined spaces, sorted alphabetically.
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

/// Get rooms the user has been invited to.
pub async fn get_invited_rooms(client: &matrix_sdk::Client) -> Result<Vec<RoomInfo>, AppError> {
    let mut rooms = Vec::new();
    for room in client.invited_rooms() {
        let name = room.cached_display_name()
            .map(|n| n.to_string())
            .unwrap_or_else(|| room.room_id().to_string());
        let avatar_url = room.avatar_url().map(|u| u.to_string());
        let topic = room.topic();
        rooms.push(RoomInfo {
            room_id: room.room_id().to_string(),
            name,
            is_encrypted: false,
            is_space: room.is_space(),
            is_direct: false,
            unread_count: 0,
            highlight_count: 0,
            avatar_url,
            topic,
            space_name: None,
            space_id: None,
            is_favourite: false,
        });
    }
    Ok(rooms)
}

/// Accept a room invitation.
pub async fn accept_invite(client: &matrix_sdk::Client, room_id: &str) -> Result<(), AppError> {
    let rid = matrix_sdk::ruma::RoomId::parse(room_id)
        .map_err(|_| AppError::Other("Invalid room ID".into()))?;
    let room = client.get_room(&rid)
        .ok_or(AppError::Other("Room not found".into()))?;
    room.join().await.map_err(|e| AppError::Other(format!("Failed to join: {e}")))?;
    Ok(())
}

/// Reject a room invitation.
pub async fn reject_invite(client: &matrix_sdk::Client, room_id: &str) -> Result<(), AppError> {
    let rid = matrix_sdk::ruma::RoomId::parse(room_id)
        .map_err(|_| AppError::Other("Invalid room ID".into()))?;
    let room = client.get_room(&rid)
        .ok_or(AppError::Other("Room not found".into()))?;
    room.leave().await.map_err(|e| AppError::Other(format!("Failed to reject: {e}")))?;
    Ok(())
}

/// Check if a room has been upgraded (tombstoned). Returns (body, replacement_room_id) if so.
pub async fn check_tombstone(client: &matrix_sdk::Client, room_id: &str) -> Option<(String, String)> {
    use matrix_sdk::ruma::events::room::tombstone::RoomTombstoneEventContent;
    let rid = matrix_sdk::ruma::RoomId::parse(room_id).ok()?;
    let room = client.get_room(&rid)?;
    let tombstone = room.get_state_event_static::<RoomTombstoneEventContent>().await.ok()??;
    let content = tombstone.deserialize().ok()?;
    let original = content.as_sync().and_then(|s| s.as_original())?;
    Some((original.content.body.clone(), original.content.replacement_room.to_string()))
}

/// Get child rooms of a space.
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

/// Join a room by ID or alias.
pub async fn join_room(client: &matrix_sdk::Client, room_id_or_alias: &str) -> Result<RoomInfo, AppError> {
    let room_or_alias = <&RoomOrAliasId>::try_from(room_id_or_alias)
        .map_err(|_| AppError::Other("Invalid room ID or alias".into()))?;
    let room = client.join_room_by_id_or_alias(room_or_alias, &[]).await?;
    Ok(room_to_info(&room, None).await)
}

/// Leave a room.
pub async fn leave_room(client: &matrix_sdk::Client, room_id: &str) -> Result<(), AppError> {
    let room_id = matrix_sdk::ruma::RoomId::parse(room_id)
        .map_err(|_| AppError::Other("Invalid room ID".into()))?;
    let room = client.get_room(&room_id).ok_or(AppError::RoomNotFound(room_id.to_string()))?;
    room.leave().await?;
    Ok(())
}

/// Invite a user to a room.
pub async fn invite_user(client: &matrix_sdk::Client, room_id: &str, user_id: &str) -> Result<(), AppError> {
    let room_id = parse_room_id(room_id)?;
    let user_id = parse_user_id(user_id)?;
    let room = client.get_room(&room_id).ok_or(AppError::RoomNotFound(room_id.to_string()))?;
    room.invite_user_by_id(&user_id).await?;
    Ok(())
}

/// Kick a user from a room with an optional reason.
pub async fn kick_user(client: &matrix_sdk::Client, room_id: &str, user_id: &str, reason: Option<&str>) -> Result<(), AppError> {
    let room_id = parse_room_id(room_id)?;
    let user_id = parse_user_id(user_id)?;
    let room = client.get_room(&room_id).ok_or(AppError::RoomNotFound(room_id.to_string()))?;
    room.kick_user(&user_id, reason).await.map_err(|e| AppError::Other(format!("Failed to kick: {e}")))?;
    Ok(())
}

/// Ban a user from a room with an optional reason.
pub async fn ban_user(client: &matrix_sdk::Client, room_id: &str, user_id: &str, reason: Option<&str>) -> Result<(), AppError> {
    let room_id = parse_room_id(room_id)?;
    let user_id = parse_user_id(user_id)?;
    let room = client.get_room(&room_id).ok_or(AppError::RoomNotFound(room_id.to_string()))?;
    room.ban_user(&user_id, reason).await.map_err(|e| AppError::Other(format!("Failed to ban: {e}")))?;
    Ok(())
}

/// Set a user's power level in a room.
pub async fn set_user_power_level(client: &matrix_sdk::Client, room_id: &str, user_id: &str, power_level: i64) -> Result<(), AppError> {
    let room_id = parse_room_id(room_id)?;
    let user_id = parse_user_id(user_id)?;
    let room = client.get_room(&room_id).ok_or(AppError::RoomNotFound(room_id.to_string()))?;
    room.update_power_levels(vec![(&user_id, Int::new(power_level).unwrap_or(Int::MIN))]).await?;
    Ok(())
}

/// Set the display name of a room.
pub async fn set_room_name(client: &matrix_sdk::Client, room_id: &str, name: &str) -> Result<(), AppError> {
    let room_id = parse_room_id(room_id)?;
    let room = client.get_room(&room_id).ok_or(AppError::RoomNotFound(room_id.to_string()))?;
    room.set_name(name.to_string()).await.map_err(|e| AppError::Other(format!("Failed to set name: {e}")))?;
    Ok(())
}

/// Set the topic of a room.
pub async fn set_room_topic(client: &matrix_sdk::Client, room_id: &str, topic: &str) -> Result<(), AppError> {
    let room_id = parse_room_id(room_id)?;
    let room = client.get_room(&room_id).ok_or(AppError::RoomNotFound(room_id.to_string()))?;
    room.set_room_topic(topic).await.map_err(|e| AppError::Other(format!("Failed to set topic: {e}")))?;
    Ok(())
}

/// Create a new room with optional encryption, space association, and invites.
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
                if let Err(e) = space.send_state_event_for_key(room.room_id(), content).await {
                    tracing::warn!("Failed to associate room with space: {e}");
                }
            }
        }
    }

    // Enable encryption if requested
    if is_encrypted {
        use matrix_sdk::ruma::events::room::encryption::RoomEncryptionEventContent;
        if let Err(e) = room.send_state_event(RoomEncryptionEventContent::with_recommended_defaults()).await {
            tracing::warn!("Failed to enable encryption on new room: {e}");
        }
    }

    Ok(room.room_id().to_string())
}

/// Add an existing room to a space.
pub async fn add_room_to_space(client: &matrix_sdk::Client, room_id: &str, space_id: &str) -> Result<(), AppError> {
    let room_id_parsed = parse_room_id(room_id)?;
    let space_room_id = parse_room_id(space_id)?;
    let space = client.get_room(&space_room_id)
        .ok_or(AppError::RoomNotFound(space_id.into()))?;
    let mut content = SpaceChildEventContent::new(Vec::new());
    if let Some(host) = client.homeserver().host_str() {
        if let Ok(server) = matrix_sdk::ruma::ServerName::parse(host) {
            content.via = vec![server.to_owned()];
        }
    }
    space.send_state_event_for_key(&room_id_parsed, content).await
        .map_err(|e| AppError::Other(format!("Failed to add room to space: {e}")))?;
    Ok(())
}

/// Remove a room from a space.
pub async fn remove_room_from_space(client: &matrix_sdk::Client, room_id: &str, space_id: &str) -> Result<(), AppError> {
    let room_id_parsed = parse_room_id(room_id)?;
    let space_room_id = parse_room_id(space_id)?;
    let space = client.get_room(&space_room_id)
        .ok_or(AppError::RoomNotFound(space_id.into()))?;
    let content = SpaceChildEventContent::new(Vec::new());
    space.send_state_event_for_key(&room_id_parsed, content).await
        .map_err(|e| AppError::Other(format!("Failed to remove room from space: {e}")))?;
    Ok(())
}

/// Open or create a DM room with a user without sending a message.
pub async fn open_dm(client: &matrix_sdk::Client, user_id: &str) -> Result<String, AppError> {
    let target = matrix_sdk::ruma::UserId::parse(user_id)
        .map_err(|_| AppError::Other(format!("Invalid user ID: {user_id}")))?;

    for room in client.joined_rooms() {
        if room.is_direct().await.unwrap_or(false) && room.active_members_count() <= 2 {
            if let Ok(Some(_)) = room.get_member_no_sync(&target).await {
                return Ok(room.room_id().to_string());
            }
        }
    }

    let mut request = CreateRoomRequest::new();
    request.is_direct = true;
    request.invite = vec![target];
    request.preset = Some(RoomPreset::PrivateChat);
    let room = client.create_room(request).await
        .map_err(|e| AppError::Other(format!("Failed to create DM: {e}")))?;

    Ok(room.room_id().to_string())
}

/// Send a direct message to a user. Creates a DM room if one doesn't exist.
pub async fn send_dm(client: &matrix_sdk::Client, user_id: &str, body: &str) -> Result<(), AppError> {
    let target = matrix_sdk::ruma::UserId::parse(user_id)
        .map_err(|_| AppError::Other(format!("Invalid user ID: {user_id}")))?;

    // Look for an existing DM room with this user
    let mut existing = None;
    for room in client.joined_rooms() {
        if room.is_direct().await.unwrap_or(false) && room.active_members_count() <= 2 {
            if let Ok(Some(_)) = room.get_member_no_sync(&target).await {
                existing = Some(room);
                break;
            }
        }
    }

    let room = if let Some(r) = existing {
        r
    } else {
        // Create a new DM room
        let mut request = CreateRoomRequest::new();
        request.is_direct = true;
        request.invite = vec![target.clone()];
        request.preset = Some(RoomPreset::PrivateChat);
        client.create_room(request).await
            .map_err(|e| AppError::Other(format!("Failed to create DM: {e}")))?
    };

    // Send the message
    use matrix_sdk::ruma::events::room::message::RoomMessageEventContent;
    room.send(RoomMessageEventContent::text_plain(body)).await
        .map_err(|e| AppError::Other(format!("Failed to send DM: {e}")))?;

    Ok(())
}

/// User search result.
#[derive(Clone, Debug)]
pub struct UserSearchResult {
    pub user_id: String,
    pub display_name: Option<String>,
    pub avatar_url: Option<String>,
}

/// Search the user directory.
pub async fn search_users(client: &matrix_sdk::Client, query: &str, limit: u64) -> Result<Vec<UserSearchResult>, AppError> {
    let response = client.search_users(query, limit).await
        .map_err(|e| AppError::Other(format!("User search failed: {e}")))?;
    Ok(response.results.iter().map(|u| UserSearchResult {
        user_id: u.user_id.to_string(),
        display_name: u.display_name.clone(),
        avatar_url: u.avatar_url.as_ref().map(|u| u.to_string()),
    }).collect())
}

/// Public room listing result.
#[derive(Clone, Debug)]
pub struct PublicRoomResult {
    pub room_id: String,
    pub name: Option<String>,
    pub alias: Option<String>,
    pub topic: Option<String>,
    pub num_members: u64,
    pub avatar_url: Option<String>,
}

/// List public rooms, optionally filtered by search term.
pub async fn list_public_rooms(
    client: &matrix_sdk::Client,
    search: Option<&str>,
    limit: Option<u32>,
    since: Option<&str>,
) -> Result<(Vec<PublicRoomResult>, Option<String>), AppError> {
    use matrix_sdk::ruma::api::client::directory::get_public_rooms_filtered;
    use matrix_sdk::ruma::directory::Filter;

    let mut request = get_public_rooms_filtered::v3::Request::new();
    if let Some(term) = search {
        request.filter = Filter::new();
        request.filter.generic_search_term = Some(term.to_string());
    }
    if let Some(l) = limit {
        request.limit = Some(l.into());
    }
    request.since = since.map(|s| s.to_string());

    let response = client.send(request).await
        .map_err(|e| AppError::Other(format!("Public rooms listing failed: {e}")))?;

    let rooms: Vec<PublicRoomResult> = response.chunk.iter().map(|r| PublicRoomResult {
        room_id: r.room_id.to_string(),
        name: r.name.clone(),
        alias: r.canonical_alias.as_ref().map(|a| a.to_string()),
        topic: r.topic.clone(),
        num_members: r.num_joined_members.into(),
        avatar_url: r.avatar_url.as_ref().map(|u| u.to_string()),
    }).collect();

    Ok((rooms, response.next_batch))
}

/// Get room aliases (canonical + alt)
pub async fn get_room_aliases(client: &matrix_sdk::Client, room_id: &str) -> Result<Vec<String>, AppError> {
    let room_id = parse_room_id(room_id)?;
    let room = client.get_room(&room_id).ok_or(AppError::RoomNotFound(room_id.to_string()))?;
    let mut aliases = Vec::new();
    if let Some(canonical) = room.canonical_alias() {
        aliases.push(canonical.to_string());
    }
    for alt in room.alt_aliases() {
        aliases.push(alt.to_string());
    }
    Ok(aliases)
}

/// Add a room alias
pub async fn add_room_alias(client: &matrix_sdk::Client, room_id: &str, alias: &str) -> Result<(), AppError> {
    let room_id = parse_room_id(room_id)?;
    let alias_id = matrix_sdk::ruma::RoomAliasId::parse(alias).map_err(|_| AppError::Other("Invalid alias (use #name:server format)".into()))?;
    // Create the alias on the server
    use matrix_sdk::ruma::api::client::alias::create_alias::v3::Request;
    let request = Request::new(alias_id.to_owned(), room_id.clone());
    client.send(request).await.map_err(|e| AppError::Other(format!("Failed to create alias: {e}")))?;
    // Set as canonical alias if none exists
    let room = client.get_room(&room_id).ok_or(AppError::RoomNotFound(room_id.to_string()))?;
    if room.canonical_alias().is_none() {
        use matrix_sdk::ruma::events::room::canonical_alias::RoomCanonicalAliasEventContent;
        let mut content = RoomCanonicalAliasEventContent::new();
        content.alias = Some(alias_id.to_owned());
        room.send_state_event(content).await.map_err(|e| AppError::Other(format!("Failed to set canonical alias: {e}")))?;
    }
    Ok(())
}

/// Remove a room alias
pub async fn remove_room_alias(client: &matrix_sdk::Client, alias: &str) -> Result<(), AppError> {
    let alias_id = matrix_sdk::ruma::RoomAliasId::parse(alias).map_err(|_| AppError::Other("Invalid alias".into()))?;
    use matrix_sdk::ruma::api::client::alias::delete_alias::v3::Request;
    let request = Request::new(alias_id.to_owned());
    client.send(request).await.map_err(|e| AppError::Other(format!("Failed to remove alias: {e}")))?;
    Ok(())
}

/// Get room notification mode: "all", "mentions", or "mute"
pub async fn get_notification_mode(client: &matrix_sdk::Client, room_id: &str) -> Result<String, AppError> {
    let room_id = parse_room_id(room_id)?;
    let room = client.get_room(&room_id).ok_or(AppError::RoomNotFound(room_id.to_string()))?;
    let mode = room.notification_mode().await;
    Ok(match mode {
        Some(matrix_sdk::notification_settings::RoomNotificationMode::AllMessages) => "all",
        Some(matrix_sdk::notification_settings::RoomNotificationMode::MentionsAndKeywordsOnly) => "mentions",
        Some(matrix_sdk::notification_settings::RoomNotificationMode::Mute) => "mute",
        None => "all",
    }.to_string())
}

/// Set room notification mode: "all", "mentions", or "mute"
pub async fn set_notification_mode(client: &matrix_sdk::Client, room_id: &str, mode: &str) -> Result<(), AppError> {
    let room_id = parse_room_id(room_id)?;
    let _room = client.get_room(&room_id).ok_or(AppError::RoomNotFound(room_id.to_string()))?;
    let settings = client.notification_settings().await;
    use matrix_sdk::notification_settings::RoomNotificationMode;
    let notification_mode = match mode {
        "mentions" => RoomNotificationMode::MentionsAndKeywordsOnly,
        "mute" => RoomNotificationMode::Mute,
        _ => RoomNotificationMode::AllMessages,
    };
    settings.set_room_notification_mode(&room_id, notification_mode).await
        .map_err(|e| AppError::Other(format!("Failed to set notification mode: {e}")))?;
    Ok(())
}

/// Check if a room is favourited.
pub async fn is_room_favourite(client: &matrix_sdk::Client, room_id: &str) -> Result<bool, AppError> {
    let room_id = parse_room_id(room_id)?;
    let room = client.get_room(&room_id).ok_or(AppError::RoomNotFound(room_id.to_string()))?;
    Ok(room.is_favourite())
}

/// Toggle favourite status on a room.
pub async fn set_room_favourite(client: &matrix_sdk::Client, room_id: &str, favourite: bool) -> Result<(), AppError> {
    let room_id = parse_room_id(room_id)?;
    let room = client.get_room(&room_id).ok_or(AppError::RoomNotFound(room_id.to_string()))?;
    room.set_is_favourite(favourite, None).await.map_err(|e| AppError::Other(format!("Failed to set favourite: {e}")))?;
    Ok(())
}
