use matrix_sdk::ruma::{
    api::client::room::create_room::v3::{Request as CreateRoomRequest, RoomPreset},
    events::space::child::SpaceChildEventContent,
    Int, RoomOrAliasId,
};
use std::collections::{HashMap, HashSet};

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
            if let Some(child_id) = event.deserialize().ok().as_ref().and_then(crate::spaces::active_child_id) {
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
        unread_count: {
            // Prefer client-side computed counts (more accurate for encrypted rooms)
            let client_notifs = room.num_unread_notifications();
            let client_msgs = room.num_unread_messages();
            if client_notifs > 0 || client_msgs > 0 {
                client_notifs.max(client_msgs)
            } else {
                room.unread_notification_counts().notification_count
            }
        },
        highlight_count: {
            let client_mentions = room.num_unread_mentions();
            if client_mentions > 0 {
                client_mentions
            } else {
                room.unread_notification_counts().highlight_count
            }
        },
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
    spaces.sort_by_key(|a| a.name.to_lowercase());
    Ok(spaces)
}

/// A space with its depth in the hierarchy (0 = top-level).
#[derive(Debug, Clone)]
pub struct SpaceNode {
    pub info: RoomInfo,
    pub depth: usize,
}

/// Build a hierarchical list of spaces (depth-annotated, pre-order DFS)
/// and a map from each space_id to all its descendant space IDs (including itself).
pub async fn get_space_tree(client: &matrix_sdk::Client)
    -> (Vec<SpaceNode>, HashMap<String, Vec<String>>)
{
    // Collect all joined spaces into a map
    let mut all_spaces: HashMap<String, RoomInfo> = HashMap::new();
    for room in client.joined_rooms() {
        if room.is_space() {
            let info = room_to_info(&room, None).await;
            all_spaces.insert(info.room_id.clone(), info);
        }
    }

    // Build parent->children map (only space children that are themselves spaces)
    let mut children_map: HashMap<String, Vec<String>> = HashMap::new();
    let mut is_child: HashSet<String> = HashSet::new();

    for room in client.joined_rooms() {
        if !room.is_space() { continue; }
        let parent_id = room.room_id().to_string();
        let child_events = match room
            .get_state_events_static::<SpaceChildEventContent>().await
        {
            Ok(events) => events,
            Err(_) => continue,
        };
        for event in child_events {
            if let Some(child_id) = event.deserialize().ok().as_ref().and_then(crate::spaces::active_child_id) {
                // Only track children that are themselves joined spaces
                if all_spaces.contains_key(&child_id) && child_id != parent_id {
                    children_map.entry(parent_id.clone()).or_default().push(child_id.clone());
                    is_child.insert(child_id);
                }
            }
        }
    }

    // Find top-level spaces (not a child of any other space)
    let mut top_level: Vec<&str> = all_spaces.keys()
        .filter(|id| !is_child.contains(id.as_str()))
        .map(|s| s.as_str())
        .collect();
    top_level.sort_by(|a, b| {
        let na = all_spaces.get(*a).map(|s| s.name.to_lowercase()).unwrap_or_default();
        let nb = all_spaces.get(*b).map(|s| s.name.to_lowercase()).unwrap_or_default();
        na.cmp(&nb)
    });

    // DFS traversal
    let mut result: Vec<SpaceNode> = Vec::new();
    let mut descendants: HashMap<String, Vec<String>> = HashMap::new();
    let mut visited: HashSet<String> = HashSet::new();

    fn dfs(
        space_id: &str,
        depth: usize,
        all_spaces: &HashMap<String, RoomInfo>,
        children_map: &HashMap<String, Vec<String>>,
        visited: &mut HashSet<String>,
        result: &mut Vec<SpaceNode>,
        descendants: &mut HashMap<String, Vec<String>>,
    ) {
        if !visited.insert(space_id.to_string()) {
            return; // cycle detected
        }
        if let Some(info) = all_spaces.get(space_id) {
            result.push(SpaceNode { info: info.clone(), depth });
        }
        // This space is its own descendant (for room filtering)
        let mut desc = vec![space_id.to_string()];

        if let Some(children) = children_map.get(space_id) {
            let mut sorted = children.clone();
            sorted.sort_by(|a, b| {
                let na = all_spaces.get(a).map(|s| s.name.to_lowercase()).unwrap_or_default();
                let nb = all_spaces.get(b).map(|s| s.name.to_lowercase()).unwrap_or_default();
                na.cmp(&nb)
            });
            for child in &sorted {
                dfs(child, depth + 1, all_spaces, children_map, visited, result, descendants);
                // Collect child's descendants
                if let Some(child_desc) = descendants.get(child) {
                    desc.extend(child_desc.iter().cloned());
                }
            }
        }
        descendants.insert(space_id.to_string(), desc);
    }

    for &root in &top_level {
        dfs(root, 0, &all_spaces, &children_map, &mut visited, &mut result, &mut descendants);
    }

    (result, descendants)
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

/// Get rooms the user has knocked on (pending join requests).
pub async fn get_knocked_rooms(client: &matrix_sdk::Client) -> Result<Vec<RoomInfo>, AppError> {
    let mut rooms = Vec::new();
    for room in client.rooms().into_iter().filter(|r| r.state() == matrix_sdk::RoomState::Knocked) {
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

/// Knock on a room (request to join).
pub async fn knock_room(client: &matrix_sdk::Client, room_id_or_alias: &str, reason: Option<&str>) -> Result<String, AppError> {
    let id = matrix_sdk::ruma::RoomOrAliasId::parse(room_id_or_alias)
        .map_err(|_| AppError::Other("Invalid room ID or alias".into()))?;
    let room = client.knock(id, reason.map(|s| s.to_string()), Vec::new())
        .await
        .map_err(|e| AppError::Other(format!("Failed to knock: {e}")))?;
    Ok(room.room_id().to_string())
}

/// Cancel a knock (leave a room in knocked state).
pub async fn cancel_knock(client: &matrix_sdk::Client, room_id: &str) -> Result<(), AppError> {
    let rid = matrix_sdk::ruma::RoomId::parse(room_id)
        .map_err(|_| AppError::Other("Invalid room ID".into()))?;
    let room = client.get_room(&rid)
        .ok_or(AppError::Other("Room not found".into()))?;
    room.leave().await.map_err(|e| AppError::Other(format!("Failed to cancel knock: {e}")))?;
    Ok(())
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
        let Some(child_id) = event.deserialize().ok().as_ref().and_then(crate::spaces::active_child_id) else { continue };
        let child_room_id = match matrix_sdk::ruma::OwnedRoomId::try_from(child_id) {
            Ok(id) => id, Err(_) => continue,
        };
        if let Some(room) = client.get_room(&child_room_id) {
            if !room.is_space() {
                rooms.push(room_to_info(&room, None).await);
            }
        }
    }
    rooms.sort_by_key(|a| a.name.to_lowercase());
    Ok(rooms)
}

/// Join a room by ID or alias.
pub async fn join_room(client: &matrix_sdk::Client, room_id_or_alias: &str) -> Result<RoomInfo, AppError> {
    join_room_via(client, room_id_or_alias, &[]).await
}

/// Join through the given servers (from a link's `via=`): needed to join a
/// room on another server by its ID.
pub async fn join_room_via(client: &matrix_sdk::Client, room_id_or_alias: &str, via: &[String]) -> Result<RoomInfo, AppError> {
    let room_or_alias = <&RoomOrAliasId>::try_from(room_id_or_alias)
        .map_err(|_| AppError::Other("Invalid room ID or alias".into()))?;
    let servers: Vec<matrix_sdk::ruma::OwnedServerName> = via.iter()
        .filter_map(|s| matrix_sdk::ruma::ServerName::parse(s).ok())
        .collect();
    let room = client.join_room_by_id_or_alias(room_or_alias, &servers).await?;
    Ok(room_to_info(&room, None).await)
}

/// Resolve a room alias to its room ID and the servers that know it.
pub async fn resolve_alias(client: &matrix_sdk::Client, alias: &str) -> Result<(String, Vec<String>), AppError> {
    let alias = matrix_sdk::ruma::RoomAliasId::parse(alias)
        .map_err(|_| AppError::Other("Invalid room alias".into()))?;
    let response = client.resolve_room_alias(&alias).await
        .map_err(|e| AppError::Other(format!("Couldn't resolve {alias}: {e}")))?;
    Ok((response.room_id.to_string(), response.servers.iter().map(|s| s.to_string()).collect()))
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

/// Unban a user from a room.
pub async fn unban_user(client: &matrix_sdk::Client, room_id: &str, user_id: &str) -> Result<(), AppError> {
    let room_id = parse_room_id(room_id)?;
    let user_id = parse_user_id(user_id)?;
    let room = client.get_room(&room_id).ok_or(AppError::RoomNotFound(room_id.to_string()))?;
    room.unban_user(&user_id, None).await.map_err(|e| AppError::Other(format!("Failed to unban: {e}")))?;
    Ok(())
}

/// Upgrade a room to a new version.
pub async fn upgrade_room(client: &matrix_sdk::Client, room_id: &str, version: &str) -> Result<(), AppError> {
    use matrix_sdk::ruma::api::client::room::upgrade_room::v3::Request;
    use matrix_sdk::ruma::RoomVersionId;
    let room_id = parse_room_id(room_id)?;
    let new_version: RoomVersionId = version.try_into()
        .map_err(|_| AppError::Other(format!("Invalid room version: {version}")))?;
    let request = Request::new(room_id, new_version);
    client.send(request).await.map_err(|e| AppError::Other(format!("Failed to upgrade room: {e}")))?;
    Ok(())
}

/// Get all room state events as (event_type, json_string) pairs for dev tools.
pub async fn get_room_state_events(client: &matrix_sdk::Client, room_id: &str) -> Result<Vec<(String, String)>, AppError> {
    use matrix_sdk::ruma::api::client::state::get_state_events;
    let room_id = parse_room_id(room_id)?;
    let request = get_state_events::v3::Request::new(room_id);
    let response = client.send(request).await
        .map_err(|e| AppError::Other(format!("Failed to get state: {e}")))?;
    let mut events: Vec<(String, String)> = response.room_state.iter()
        .filter_map(|raw| {
            let val: serde_json::Value = serde_json::from_str(raw.json().get()).ok()?;
            let event_type = val.get("type")?.as_str()?.to_string();
            let pretty = serde_json::to_string_pretty(&val).ok()?;
            Some((event_type, pretty))
        })
        .collect();
    events.sort_by(|a, b| a.0.cmp(&b.0));
    Ok(events)
}

/// Send a raw JSON event to a room (state or message).
pub async fn send_raw_event(
    client: &matrix_sdk::Client,
    room_id: &str,
    event_type: &str,
    state_key: Option<&str>,
    body_json: &str,
) -> Result<(), AppError> {
    let room_id_parsed = parse_room_id(room_id)?;
    let room = client.get_room(&room_id_parsed).ok_or(AppError::RoomNotFound(room_id.to_string()))?;
    let body: serde_json::Value = serde_json::from_str(body_json)
        .map_err(|e| AppError::Other(format!("Invalid JSON: {e}")))?;
    if let Some(key) = state_key {
        room.send_state_event_raw(event_type, key, body).await
            .map_err(|e| AppError::Other(format!("Failed to send state event: {e}")))?;
    } else {
        room.send_raw(event_type, body).await
            .map_err(|e| AppError::Other(format!("Failed to send event: {e}")))?;
    }
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
#[allow(clippy::too_many_arguments)]
pub async fn create_room(
    client: &matrix_sdk::Client,
    name: &str,
    topic: Option<&str>,
    is_public: bool,
    is_encrypted: bool,
    is_space: bool,
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
    if is_space {
        // Set creation_content.type = "m.space" to create a space.
        let mut creation = serde_json::Map::new();
        creation.insert("type".into(), serde_json::Value::String("m.space".into()));
        let json_str = serde_json::to_string(&creation).unwrap();
        let raw_value = serde_json::value::RawValue::from_string(json_str).unwrap();
        request.creation_content = Some(
            matrix_sdk::ruma::serde::Raw::from_json(raw_value),
        );
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
        if let Err(e) = crate::spaces::add_child(client, sid, room.room_id().as_str()).await {
            tracing::warn!("Failed to associate room with space: {e}");
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
    crate::spaces::add_child(client, space_id, room_id).await
}

/// Remove a room from a space.
pub async fn remove_room_from_space(client: &matrix_sdk::Client, room_id: &str, space_id: &str) -> Result<(), AppError> {
    crate::spaces::remove_child(client, space_id, room_id).await
}

/// Open or create a DM room with a user without sending a message.
pub async fn open_dm(client: &matrix_sdk::Client, user_id: &str) -> Result<String, AppError> {
    let target = matrix_sdk::ruma::UserId::parse(user_id)
        .map_err(|_| AppError::Other(format!("Invalid user ID: {user_id}")))?;

    // Reuse an existing DM: the m.direct mapping is authoritative, and a
    // two-person DM the other side hasn't left also counts.
    for room in client.joined_rooms() {
        if room.direct_targets().iter().any(|t| t.as_str() == target.as_str()) {
            return Ok(room.room_id().to_string());
        }
        if room.is_direct().await.unwrap_or(false) && room.active_members_count() <= 2 {
            if let Ok(Some(member)) = room.get_member_no_sync(&target).await {
                use matrix_sdk::ruma::events::room::member::MembershipState;
                if matches!(member.membership(), MembershipState::Join | MembershipState::Invite) {
                    return Ok(room.room_id().to_string());
                }
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
    let room_id = open_dm(client, user_id).await?;
    let room_id = parse_room_id(&room_id)?;
    let room = client.get_room(&room_id).ok_or(AppError::RoomNotFound(room_id.to_string()))?;

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
/// When `server` is provided, queries that server's directory via federation.
pub async fn list_public_rooms(
    client: &matrix_sdk::Client,
    search: Option<&str>,
    limit: Option<u32>,
    since: Option<&str>,
    server: Option<&str>,
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
    if let Some(srv) = server {
        if let Ok(name) = matrix_sdk::ruma::ServerName::parse(srv) {
            request.server = Some(name.to_owned());
        }
    }

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

/// Upload a room avatar image.
pub async fn upload_room_avatar(client: &matrix_sdk::Client, room_id: &str, mime_type: &str, data: Vec<u8>) -> Result<String, AppError> {
    let room_id = parse_room_id(room_id)?;
    let room = client.get_room(&room_id).ok_or(AppError::RoomNotFound(room_id.to_string()))?;
    let ct: mime::Mime = mime_type.parse().unwrap_or(mime::IMAGE_PNG);
    let mxc = client.media().upload(&ct, data, None).await
        .map_err(|e| AppError::Other(format!("Failed to upload: {e}")))?.content_uri;
    use matrix_sdk::ruma::events::room::avatar::RoomAvatarEventContent;
    let mut content = RoomAvatarEventContent::new();
    content.url = Some(mxc.clone());
    room.send_state_event(content).await.map_err(|e| AppError::Other(format!("Failed to set avatar: {e}")))?;
    Ok(mxc.to_string())
}

/// Remove the room avatar.
pub async fn remove_room_avatar(client: &matrix_sdk::Client, room_id: &str) -> Result<(), AppError> {
    let room_id = parse_room_id(room_id)?;
    let room = client.get_room(&room_id).ok_or(AppError::RoomNotFound(room_id.to_string()))?;
    use matrix_sdk::ruma::events::room::avatar::RoomAvatarEventContent;
    let content = RoomAvatarEventContent::new();
    room.send_state_event(content).await.map_err(|e| AppError::Other(format!("Failed to remove avatar: {e}")))?;
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
