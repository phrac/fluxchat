//! Room administration: who can join and read history (security & privacy),
//! and the power levels each action needs (roles & permissions). Works for
//! spaces too — they're rooms.

use matrix_sdk::ruma::api::client::room::Visibility;
use matrix_sdk::ruma::events::room::guest_access::{GuestAccess, RoomGuestAccessEventContent};
use matrix_sdk::ruma::events::room::history_visibility::HistoryVisibility;
use matrix_sdk::ruma::events::room::join_rules::{AllowRule, JoinRule, Restricted};
use matrix_sdk::ruma::events::room::power_levels::RoomPowerLevelsEventContent;
use matrix_sdk::ruma::events::{StateEventType, TimelineEventType};
use matrix_sdk::ruma::Int;
use matrix_sdk::RoomMemberships;

use crate::error::AppError;
use crate::rooms::parse_room_id;

/// Who may join.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum JoinMode {
    /// Invite only.
    Invite,
    /// Anyone may ask to join; members approve.
    Knock,
    /// Members of the listed spaces may join.
    SpaceMembers,
    /// Anyone may join.
    Public,
}

/// Who may read history.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HistoryMode {
    /// Anyone, without joining (public rooms).
    WorldReadable,
    /// Members, including messages from before they joined.
    Shared,
    /// Members, from when they were invited.
    Invited,
    /// Members, from when they joined.
    Joined,
}

/// Security & privacy settings of a room, plus what we may change.
#[derive(Debug, Clone)]
pub struct RoomSecurity {
    pub join: JoinMode,
    /// Spaces whose members may join (for `JoinMode::SpaceMembers`).
    pub allowed_spaces: Vec<String>,
    pub history: HistoryMode,
    pub guest_access: bool,
    pub in_directory: bool,
    pub encrypted: bool,
    pub can_change_join: bool,
    pub can_change_history: bool,
    pub can_change_guests: bool,
    pub can_enable_encryption: bool,
    pub can_publish: bool,
}

fn room(client: &matrix_sdk::Client, room_id: &str) -> Result<matrix_sdk::Room, AppError> {
    let id = parse_room_id(room_id)?;
    client.get_room(&id).ok_or_else(|| AppError::RoomNotFound(room_id.into()))
}

fn err(e: impl std::fmt::Display) -> AppError {
    AppError::Other(e.to_string())
}

/// Read a room's security & privacy settings.
pub async fn room_security(client: &matrix_sdk::Client, room_id: &str) -> Result<RoomSecurity, AppError> {
    let room = room(client, room_id)?;
    let user_id = client.user_id().ok_or_else(|| err("Not logged in"))?.to_owned();
    let pl = room.power_levels().await.map_err(err)?;
    let can = |t: StateEventType| pl.user_can_send_state(&user_id, t);

    let (join, allowed_spaces) = match room.join_rule() {
        Some(JoinRule::Public) => (JoinMode::Public, Vec::new()),
        Some(JoinRule::Knock) | Some(JoinRule::KnockRestricted(_)) => (JoinMode::Knock, Vec::new()),
        Some(JoinRule::Restricted(r)) => (
            JoinMode::SpaceMembers,
            r.allow.iter().filter_map(|a| match a {
                AllowRule::RoomMembership(m) => Some(m.room_id.to_string()),
                _ => None,
            }).collect(),
        ),
        _ => (JoinMode::Invite, Vec::new()),
    };
    let history = match room.history_visibility_or_default() {
        HistoryVisibility::WorldReadable => HistoryMode::WorldReadable,
        HistoryVisibility::Invited => HistoryMode::Invited,
        HistoryVisibility::Joined => HistoryMode::Joined,
        _ => HistoryMode::Shared,
    };
    let in_directory = matches!(room.privacy_settings().get_room_visibility().await, Ok(Visibility::Public));
    Ok(RoomSecurity {
        join,
        allowed_spaces,
        history,
        guest_access: room.guest_access() == GuestAccess::CanJoin,
        in_directory,
        encrypted: room.latest_encryption_state().await.map(|s| s.is_encrypted()).unwrap_or(false),
        can_change_join: can(StateEventType::RoomJoinRules),
        can_change_history: can(StateEventType::RoomHistoryVisibility),
        can_change_guests: can(StateEventType::RoomGuestAccess),
        can_enable_encryption: can(StateEventType::RoomEncryption),
        // Directory listing needs moderator rights on most servers; let the
        // server decide and surface its error.
        can_publish: can(StateEventType::RoomCanonicalAlias),
    })
}

/// Change who may join. `spaces` is used for `JoinMode::SpaceMembers`.
pub async fn set_join_mode(client: &matrix_sdk::Client, room_id: &str, mode: JoinMode, spaces: &[String]) -> Result<(), AppError> {
    let room = room(client, room_id)?;
    let rule = match mode {
        JoinMode::Invite => JoinRule::Invite,
        JoinMode::Knock => JoinRule::Knock,
        JoinMode::Public => JoinRule::Public,
        JoinMode::SpaceMembers => {
            let allow: Vec<AllowRule> = spaces.iter()
                .filter_map(|s| parse_room_id(s).ok())
                .map(AllowRule::room_membership)
                .collect();
            if allow.is_empty() {
                return Err(err("This room isn't in a space yet — add it to one first"));
            }
            JoinRule::Restricted(Restricted::new(allow))
        }
    };
    room.privacy_settings().update_join_rule(rule).await.map_err(err)
}

/// Change who may read history.
pub async fn set_history_mode(client: &matrix_sdk::Client, room_id: &str, mode: HistoryMode) -> Result<(), AppError> {
    let vis = match mode {
        HistoryMode::WorldReadable => HistoryVisibility::WorldReadable,
        HistoryMode::Shared => HistoryVisibility::Shared,
        HistoryMode::Invited => HistoryVisibility::Invited,
        HistoryMode::Joined => HistoryVisibility::Joined,
    };
    room(client, room_id)?.privacy_settings().update_room_history_visibility(vis).await.map_err(err)
}

/// Allow or forbid guest accounts.
pub async fn set_guest_access(client: &matrix_sdk::Client, room_id: &str, allowed: bool) -> Result<(), AppError> {
    let access = if allowed { GuestAccess::CanJoin } else { GuestAccess::Forbidden };
    room(client, room_id)?.send_state_event(RoomGuestAccessEventContent::new(access)).await.map_err(err)?;
    Ok(())
}

/// List or unlist the room in the server's public room directory.
pub async fn set_in_directory(client: &matrix_sdk::Client, room_id: &str, listed: bool) -> Result<(), AppError> {
    let vis = if listed { Visibility::Public } else { Visibility::Private };
    room(client, room_id)?.privacy_settings().update_room_visibility(vis).await.map_err(err)
}

/// A setting in the power-level event.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum Permission {
    /// Level new members get.
    UsersDefault,
    /// Send ordinary messages (unless overridden per type).
    EventsDefault,
    /// Change settings without a specific level.
    StateDefault,
    Invite,
    Kick,
    Ban,
    /// Delete other people's messages.
    Redact,
    /// Notify everyone with @room.
    NotifyRoom,
    /// A specific event type (`m.room.name`, ...).
    Event(String),
}

/// Power levels of a room: users with explicit levels, every action's
/// required level, and what we may change.
#[derive(Debug, Clone)]
pub struct RoomPermissions {
    /// (user_id, display name, level), highest first.
    pub users: Vec<(String, String, i64)>,
    pub levels: std::collections::HashMap<Permission, i64>,
    pub my_level: i64,
    pub can_edit: bool,
}

impl RoomPermissions {
    /// Level required for a permission (event types fall back to the
    /// state/event defaults like the server does).
    pub fn level(&self, p: &Permission) -> i64 {
        if let Some(l) = self.levels.get(p) {
            return *l;
        }
        match p {
            Permission::Event(t) if is_state_type(t) => self.levels.get(&Permission::StateDefault).copied().unwrap_or(50),
            _ => self.levels.get(&Permission::EventsDefault).copied().unwrap_or(0),
        }
    }
}

/// Event types configured as state in the permissions UI.
fn is_state_type(t: &str) -> bool {
    t != "m.room.message" && t != "m.reaction" && t != "m.room.redaction" && t != "m.sticker"
}

/// A banned member.
#[derive(Debug, Clone)]
pub struct BannedUser {
    pub user_id: String,
    pub display_name: String,
    pub reason: Option<String>,
}

fn int(i: Int) -> i64 {
    i64::from(i)
}

/// Read a room's power levels and ban list.
pub async fn room_permissions(client: &matrix_sdk::Client, room_id: &str) -> Result<(RoomPermissions, Vec<BannedUser>), AppError> {
    let room = room(client, room_id)?;
    let user_id = client.user_id().ok_or_else(|| err("Not logged in"))?.to_owned();
    let pl = room.power_levels().await.map_err(err)?;

    let mut levels = std::collections::HashMap::new();
    levels.insert(Permission::UsersDefault, int(pl.users_default));
    levels.insert(Permission::EventsDefault, int(pl.events_default));
    levels.insert(Permission::StateDefault, int(pl.state_default));
    levels.insert(Permission::Invite, int(pl.invite));
    levels.insert(Permission::Kick, int(pl.kick));
    levels.insert(Permission::Ban, int(pl.ban));
    levels.insert(Permission::Redact, int(pl.redact));
    levels.insert(Permission::NotifyRoom, int(pl.notifications.room));
    for (t, l) in &pl.events {
        levels.insert(Permission::Event(t.to_string()), int(*l));
    }

    let mut users = Vec::new();
    for (uid, level) in &pl.users {
        let name = room.get_member_no_sync(uid).await.ok().flatten()
            .and_then(|m| m.display_name().map(str::to_string))
            .unwrap_or_else(|| uid.localpart().to_string());
        users.push((uid.to_string(), name, int(*level)));
    }
    // Creators of v12+ rooms have unlimited power and aren't in `users`.
    for creator in room.creators().unwrap_or_default() {
        if !users.iter().any(|(u, _, _)| *u == creator.as_str()) {
            let name = room.get_member_no_sync(&creator).await.ok().flatten()
                .and_then(|m| m.display_name().map(str::to_string))
                .unwrap_or_else(|| creator.localpart().to_string());
            users.push((creator.to_string(), name, i64::MAX));
        }
    }
    users.sort_by(|a, b| b.2.cmp(&a.2).then_with(|| a.1.to_lowercase().cmp(&b.1.to_lowercase())));

    let my_level = match pl.for_user(&user_id) {
        matrix_sdk::ruma::events::room::power_levels::UserPowerLevel::Int(i) => int(i),
        _ => i64::MAX,
    };
    let can_edit = pl.user_can_send_state(&user_id, StateEventType::RoomPowerLevels);

    let mut bans = Vec::new();
    for m in room.members(RoomMemberships::BAN).await.map_err(err)? {
        bans.push(BannedUser {
            user_id: m.user_id().to_string(),
            display_name: m.display_name().map(str::to_string).unwrap_or_else(|| m.user_id().localpart().to_string()),
            reason: m.event().reason().map(str::to_string),
        });
    }
    Ok((RoomPermissions { users, levels, my_level, can_edit }, bans))
}

/// Change the level a permission requires.
pub async fn set_permission(client: &matrix_sdk::Client, room_id: &str, permission: &Permission, level: i64) -> Result<(), AppError> {
    let room = room(client, room_id)?;
    let mut pl = room.power_levels().await.map_err(err)?;
    let level = Int::new(level).ok_or_else(|| err("Invalid level"))?;
    match permission {
        Permission::UsersDefault => pl.users_default = level,
        Permission::EventsDefault => pl.events_default = level,
        Permission::StateDefault => pl.state_default = level,
        Permission::Invite => pl.invite = level,
        Permission::Kick => pl.kick = level,
        Permission::Ban => pl.ban = level,
        Permission::Redact => pl.redact = level,
        Permission::NotifyRoom => pl.notifications.room = level,
        Permission::Event(t) => {
            pl.events.insert(TimelineEventType::from(t.as_str()), level);
        }
    }
    let content = RoomPowerLevelsEventContent::try_from(pl).map_err(err)?;
    room.send_state_event(content).await.map_err(err)?;
    Ok(())
}
