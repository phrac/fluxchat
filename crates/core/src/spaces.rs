//! Space management: listing a space's children (including rooms we haven't
//! joined), adding/removing children, and the current user's permissions.

use matrix_sdk::deserialized_responses::SyncOrStrippedState;
use matrix_sdk::ruma::events::space::child::SpaceChildEventContent;
use matrix_sdk::ruma::events::{StateEventType, SyncStateEvent};
use matrix_sdk::RoomState;
use matrix_sdk_ui::spaces::room_list::SpaceRoomListPaginationState;
use matrix_sdk_ui::spaces::{SpaceRoomList, SpaceService};

use crate::error::AppError;
use crate::rooms::parse_room_id;

/// Upper bound on `/hierarchy` pages fetched for one space.
const MAX_HIERARCHY_PAGES: usize = 10;

/// A direct child of a space, as shown in the space dialog.
#[derive(Debug, Clone)]
pub struct SpaceChild {
    pub room_id: String,
    pub name: String,
    pub topic: Option<String>,
    pub avatar_url: Option<String>,
    pub alias: Option<String>,
    pub num_members: u64,
    pub is_space: bool,
    pub joined: bool,
    pub suggested: bool,
    /// Servers to join through (from the space's `m.space.child`).
    pub via: Vec<String>,
}

/// What the current user may change in a space.
#[derive(Debug, Clone, Copy, Default)]
pub struct SpacePermissions {
    /// May add/remove children (`m.space.child`).
    pub manage_children: bool,
    /// May change name / topic / avatar.
    pub edit_details: bool,
    pub invite: bool,
}

/// The state key of a live `m.space.child` event. A child whose `via` is
/// empty (or that was redacted) has been removed from the space.
pub(crate) fn active_child_id(ev: &SyncOrStrippedState<SpaceChildEventContent>) -> Option<String> {
    match ev {
        SyncOrStrippedState::Sync(SyncStateEvent::Original(e)) =>
            (!e.content.via.is_empty()).then(|| e.state_key.to_string()),
        SyncOrStrippedState::Sync(SyncStateEvent::Redacted(_)) => None,
        SyncOrStrippedState::Stripped(e) =>
            e.content.via.as_ref().is_some_and(|v| !v.is_empty()).then(|| e.state_key.to_string()),
    }
}

/// The direct children of a space. Uses the server's `/hierarchy` so rooms
/// we haven't joined are listed with their names; falls back to the space's
/// local `m.space.child` state if that fails.
pub async fn space_children(client: &matrix_sdk::Client, space_id: &str) -> Result<Vec<SpaceChild>, AppError> {
    let space_room_id = parse_room_id(space_id)?;
    let list = SpaceRoomList::new(client.clone(), space_room_id.clone()).await;
    let mut hierarchy_ok = true;
    for _ in 0..MAX_HIERARCHY_PAGES {
        if let Err(e) = list.paginate().await {
            tracing::warn!("Space hierarchy for {space_id} failed: {e}");
            hierarchy_ok = false;
            break;
        }
        if matches!(list.pagination_state(), SpaceRoomListPaginationState::Idle { end_reached: true }) {
            break;
        }
    }
    let mut children: Vec<SpaceChild> = if hierarchy_ok {
        list.rooms().await.into_iter().map(|r| {
            // Prefer live local state for rooms we're in (fresher than the
            // server's hierarchy cache).
            let local = client.get_room(&r.room_id);
            SpaceChild {
                room_id: r.room_id.to_string(),
                name: local.as_ref().and_then(|l| l.cached_display_name().map(|n| n.to_string()))
                    .unwrap_or(r.display_name),
                topic: local.as_ref().and_then(|l| l.topic()).or(r.topic),
                avatar_url: r.avatar_url.map(|u| u.to_string()),
                alias: r.canonical_alias.map(|a| a.to_string()),
                num_members: r.num_joined_members,
                is_space: r.room_type.as_ref().is_some_and(|t| *t == matrix_sdk::ruma::room::RoomType::Space),
                joined: matches!(r.state, Some(RoomState::Joined)),
                suggested: r.suggested,
                via: r.via.iter().map(|v| v.to_string()).collect(),
            }
        }).collect()
    } else {
        local_children(client, space_id).await?
    };
    // Joined rooms first, then by name.
    children.sort_by(|a, b| b.joined.cmp(&a.joined)
        .then_with(|| a.is_space.cmp(&b.is_space))
        .then_with(|| a.name.to_lowercase().cmp(&b.name.to_lowercase())));
    Ok(children)
}

/// Children from the space's local state (rooms we know about only).
async fn local_children(client: &matrix_sdk::Client, space_id: &str) -> Result<Vec<SpaceChild>, AppError> {
    let space = client.get_room(&parse_room_id(space_id)?)
        .ok_or_else(|| AppError::RoomNotFound(space_id.into()))?;
    let events = space.get_state_events_static::<SpaceChildEventContent>().await
        .map_err(|e| AppError::Other(e.to_string()))?;
    let mut out = Vec::new();
    for raw in events {
        let Ok(ev) = raw.deserialize() else { continue };
        let Some(child_id) = active_child_id(&ev) else { continue };
        let Ok(rid) = parse_room_id(&child_id) else { continue };
        let room = client.get_room(&rid);
        out.push(SpaceChild {
            name: room.as_ref().and_then(|r| r.cached_display_name().map(|n| n.to_string()))
                .unwrap_or_else(|| child_id.clone()),
            topic: room.as_ref().and_then(|r| r.topic()),
            avatar_url: room.as_ref().and_then(|r| r.avatar_url()).map(|u| u.to_string()),
            alias: room.as_ref().and_then(|r| r.canonical_alias()).map(|a| a.to_string()),
            num_members: room.as_ref().map(|r| r.joined_members_count()).unwrap_or(0),
            is_space: room.as_ref().is_some_and(|r| r.is_space()),
            joined: room.as_ref().is_some_and(|r| r.state() == RoomState::Joined),
            suggested: false,
            via: Vec::new(),
            room_id: child_id,
        });
    }
    Ok(out)
}

/// The current user's permissions in a space.
pub async fn space_permissions(client: &matrix_sdk::Client, space_id: &str) -> Result<SpacePermissions, AppError> {
    let space = client.get_room(&parse_room_id(space_id)?)
        .ok_or_else(|| AppError::RoomNotFound(space_id.into()))?;
    let user_id = client.user_id().ok_or_else(|| AppError::Other("Not logged in".into()))?;
    let pl = space.power_levels().await.map_err(|e| AppError::Other(e.to_string()))?;
    Ok(SpacePermissions {
        manage_children: pl.user_can_send_state(user_id, StateEventType::SpaceChild),
        edit_details: pl.user_can_send_state(user_id, StateEventType::RoomName),
        invite: pl.user_can_invite(user_id),
    })
}

/// Add a room we're in to a space (also sets `m.space.parent` on the room
/// when we're allowed to).
pub async fn add_child(client: &matrix_sdk::Client, space_id: &str, child_id: &str) -> Result<(), AppError> {
    SpaceService::new(client.clone()).await
        .add_child_to_space(parse_room_id(child_id)?, parse_room_id(space_id)?)
        .await
        .map_err(|e| AppError::Other(format!("Couldn't add room to space: {e}")))
}

/// Remove a child from a space (and its `m.space.parent`, when allowed).
pub async fn remove_child(client: &matrix_sdk::Client, space_id: &str, child_id: &str) -> Result<(), AppError> {
    SpaceService::new(client.clone()).await
        .remove_child_from_space(parse_room_id(child_id)?, parse_room_id(space_id)?)
        .await
        .map_err(|e| AppError::Other(format!("Couldn't remove room from space: {e}")))
}

/// IDs of the joined spaces that list `room_id` as a child.
pub async fn parent_spaces(client: &matrix_sdk::Client, room_id: &str) -> Vec<String> {
    let mut out = Vec::new();
    for space in client.joined_rooms().into_iter().filter(|r| r.is_space()) {
        let Ok(events) = space.get_state_events_static::<SpaceChildEventContent>().await else { continue };
        let contains = events.into_iter()
            .filter_map(|raw| raw.deserialize().ok())
            .any(|ev| active_child_id(&ev).as_deref() == Some(room_id));
        if contains {
            out.push(space.room_id().to_string());
        }
    }
    out
}

/// Join a space's child room, routing through the servers the space lists.
pub async fn join_child(client: &matrix_sdk::Client, room_id: &str, via: &[String]) -> Result<(), AppError> {
    let rid = parse_room_id(room_id)?;
    let servers: Vec<matrix_sdk::ruma::OwnedServerName> = via.iter()
        .filter_map(|v| matrix_sdk::ruma::ServerName::parse(v).ok())
        .collect();
    let target: &matrix_sdk::ruma::RoomOrAliasId = (&*rid).into();
    client.join_room_by_id_or_alias(target, &servers).await
        .map_err(|e| AppError::Other(format!("Couldn't join: {e}")))?;
    Ok(())
}

/// Joined spaces the current user may add children to.
pub async fn manageable_spaces(client: &matrix_sdk::Client) -> Vec<String> {
    let Some(user_id) = client.user_id() else { return Vec::new() };
    let mut out = Vec::new();
    for space in client.joined_rooms().into_iter().filter(|r| r.is_space()) {
        if let Ok(pl) = space.power_levels().await {
            if pl.user_can_send_state(user_id, StateEventType::SpaceChild) {
                out.push(space.room_id().to_string());
            }
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::active_child_id;
    use matrix_sdk::deserialized_responses::RawSyncOrStrippedState;
    use matrix_sdk::ruma::events::space::child::SpaceChildEventContent;
    use matrix_sdk::ruma::serde::Raw;

    fn child_event(content: &str) -> RawSyncOrStrippedState<SpaceChildEventContent> {
        let json = format!(
            r#"{{"type":"m.space.child","state_key":"!child:example.org","sender":"@a:example.org",
                "event_id":"$e","origin_server_ts":1,"content":{content}}}"#
        );
        RawSyncOrStrippedState::Sync(Raw::from_json_string(json).unwrap())
    }

    #[test]
    fn child_with_via_is_active() {
        let ev = child_event(r#"{"via":["example.org"]}"#).deserialize().unwrap();
        assert_eq!(active_child_id(&ev).as_deref(), Some("!child:example.org"));
    }

    #[test]
    fn emptied_child_counts_as_removed() {
        // Removal sends `{}` (or an empty `via`); neither is a child any more.
        for content in [r#"{}"#, r#"{"via":[]}"#] {
            let ev = child_event(content).deserialize();
            assert!(ev.map(|e| active_child_id(&e)).ok().flatten().is_none(), "{content}");
        }
    }
}
