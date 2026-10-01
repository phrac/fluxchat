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

/// Advanced settings: version, upgrade links and addresses.
#[derive(Debug, Clone)]
pub struct RoomAdvanced {
    pub version: String,
    /// The server's version for new rooms, when it differs from this room's
    /// (what Upgrade moves to).
    pub upgrade_to: Option<String>,
    /// The room this one replaced, and the room that replaced it.
    pub predecessor: Option<String>,
    pub successor: Option<String>,
    /// Published addresses (`m.room.canonical_alias`).
    pub main_alias: Option<String>,
    pub alt_aliases: Vec<String>,
    /// Addresses on our server that point here, published or not.
    pub local_aliases: Vec<String>,
    /// Our homeserver, for `#name:server` addresses.
    pub server: String,
    pub can_publish_aliases: bool,
    pub can_upgrade: bool,
}

/// Read a room's version, upgrade links and addresses.
pub async fn room_advanced(client: &matrix_sdk::Client, room_id: &str) -> Result<RoomAdvanced, AppError> {
    use matrix_sdk::ruma::api::client::room::aliases;
    let room = room(client, room_id)?;
    let user_id = client.user_id().ok_or_else(|| err("Not logged in"))?.to_owned();
    let pl = room.power_levels().await.map_err(err)?;
    let version = room.version().map(|v| v.to_string()).unwrap_or_else(|| "1".into());
    // Capabilities and local aliases are best-effort: the tab still works
    // (without Upgrade / the local list) if the server won't say.
    let upgrade_to = client.homeserver_capabilities().room_versions().await.ok()
        .map(|c| c.default.to_string())
        .filter(|v| *v != version);
    let local_aliases = client.send(aliases::v3::Request::new(room.room_id().to_owned())).await
        .map(|r| r.aliases.iter().map(|a| a.to_string()).collect())
        .unwrap_or_default();
    Ok(RoomAdvanced {
        version,
        upgrade_to,
        predecessor: room.predecessor_room().map(|p| p.room_id.to_string()),
        successor: room.successor_room().map(|s| s.room_id.to_string()),
        main_alias: room.canonical_alias().map(|a| a.to_string()),
        alt_aliases: room.alt_aliases().iter().map(|a| a.to_string()).collect(),
        local_aliases,
        server: user_id.server_name().to_string(),
        can_publish_aliases: pl.user_can_send_state(&user_id, StateEventType::RoomCanonicalAlias),
        can_upgrade: pl.user_can_send_state(&user_id, StateEventType::RoomTombstone),
    })
}

/// `#name:server` from what was typed: a full address, or just the name
/// (then on `server`).
pub fn full_alias(input: &str, server: &str) -> Option<String> {
    let input = input.trim();
    let name = input.strip_prefix('#').unwrap_or(input);
    if name.is_empty() || name.chars().any(char::is_whitespace) {
        return None;
    }
    let full = if name.contains(':') { format!("#{name}") } else { format!("#{name}:{server}") };
    matrix_sdk::ruma::RoomAliasId::parse(&full).ok().map(|a| a.to_string())
}

/// Rewrite the published addresses: `main` (None = no main address) and
/// the alternatives, keeping `main` out of them.
pub async fn set_published_aliases(client: &matrix_sdk::Client, room_id: &str, main: Option<&str>, alts: &[String]) -> Result<(), AppError> {
    use matrix_sdk::ruma::events::room::canonical_alias::RoomCanonicalAliasEventContent;
    use matrix_sdk::ruma::RoomAliasId;
    let room = room(client, room_id)?;
    let parse = |a: &str| RoomAliasId::parse(a).map_err(|_| err(format!("Invalid address: {a}")));
    let mut content = RoomCanonicalAliasEventContent::new();
    content.alias = main.map(parse).transpose()?;
    for alt in alts.iter().filter(|a| Some(a.as_str()) != main) {
        let alt = parse(alt)?;
        if !content.alt_aliases.contains(&alt) {
            content.alt_aliases.push(alt);
        }
    }
    room.send_state_event(content).await.map_err(err)?;
    Ok(())
}

/// Delete an address from our server. If it was published, unpublish it
/// too, so the room doesn't advertise a dead address.
pub async fn remove_local_alias(client: &matrix_sdk::Client, room_id: &str, alias: &str) -> Result<(), AppError> {
    crate::rooms::remove_room_alias(client, alias).await?;
    let room = room(client, room_id)?;
    let main = room.canonical_alias().map(|a| a.to_string());
    let alts: Vec<String> = room.alt_aliases().iter().map(|a| a.to_string()).collect();
    if main.as_deref() == Some(alias) || alts.iter().any(|a| a == alias) {
        let main = main.filter(|m| m != alias);
        let alts: Vec<String> = alts.into_iter().filter(|a| a != alias).collect();
        // Best effort: the server usually unpublishes it itself, and the
        // delete (what was asked) already succeeded.
        if let Err(e) = set_published_aliases(client, room_id, main.as_deref(), &alts).await {
            tracing::warn!("Couldn't unpublish deleted address {alias}: {e}");
        }
    }
    Ok(())
}

/// Someone asking to join a room (knock join rule).
#[derive(Debug, Clone, PartialEq)]
pub struct JoinRequest {
    pub user_id: String,
    pub display_name: Option<String>,
    pub avatar_url: Option<String>,
    pub reason: Option<String>,
    /// Seconds since the epoch.
    pub timestamp: Option<u64>,
}

/// The pending join requests of a room, and whether we may answer them
/// (approving is an invite).
#[derive(Debug, Clone, PartialEq, Default)]
pub struct JoinRequests {
    pub can_respond: bool,
    pub requests: Vec<JoinRequest>,
}

/// How to answer a join request.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum JoinResponse {
    /// Invite them in.
    Approve,
    /// Turn them away (they can ask again).
    Deny,
    /// Turn them away and ban them.
    DenyAndBan,
}

/// Watch a room's join requests: `on_update` gets the full list now and
/// whenever it changes. Abort the returned task to stop.
pub async fn watch_join_requests<F>(client: &matrix_sdk::Client, room_id: &str, on_update: F) -> Result<tokio::task::JoinHandle<()>, AppError>
where
    F: Fn(JoinRequests) + Send + 'static,
{
    use futures_util::StreamExt;
    let room = room(client, room_id)?;
    let user_id = client.user_id().ok_or_else(|| err("Not logged in"))?.to_owned();
    let (stream, cleanup) = room.subscribe_to_knock_requests().await.map_err(err)?;
    // The SDK's helper task lives as long as our watcher.
    struct Cleanup(matrix_sdk::executor::JoinHandle<()>);
    impl Drop for Cleanup {
        fn drop(&mut self) {
            self.0.abort();
        }
    }
    Ok(tokio::spawn(async move {
        let _cleanup = Cleanup(cleanup);
        let mut stream = std::pin::pin!(stream);
        let mut last = None;
        while let Some(requests) = stream.next().await {
            let can_respond = match room.power_levels().await {
                Ok(pl) => pl.user_can_invite(&user_id),
                Err(_) => false,
            };
            let update = JoinRequests {
                can_respond,
                requests: requests.into_iter().map(|r| JoinRequest {
                    user_id: r.member_info.user_id.to_string(),
                    display_name: r.member_info.display_name,
                    avatar_url: r.member_info.avatar_url.map(|u| u.to_string()),
                    reason: r.member_info.reason.filter(|s| !s.trim().is_empty()),
                    timestamp: r.timestamp.map(|t| u64::from(t) / 1000),
                }).collect(),
            };
            if last.as_ref() != Some(&update) {
                last = Some(update.clone());
                on_update(update);
            }
        }
    }))
}

/// Answer a join request.
pub async fn respond_join_request(client: &matrix_sdk::Client, room_id: &str, user_id: &str, response: JoinResponse) -> Result<(), AppError> {
    let room = room(client, room_id)?;
    let user_id = crate::rooms::parse_user_id(user_id)?;
    match response {
        JoinResponse::Approve => room.invite_user_by_id(&user_id).await,
        JoinResponse::Deny => room.kick_user(&user_id, None).await,
        JoinResponse::DenyAndBan => room.ban_user(&user_id, None).await,
    }
    .map_err(err)
}

#[cfg(test)]
mod tests {
    use super::full_alias;

    #[test]
    fn alias_from_input() {
        assert_eq!(full_alias("lobby", "example.org").as_deref(), Some("#lobby:example.org"));
        assert_eq!(full_alias(" #lobby ", "example.org").as_deref(), Some("#lobby:example.org"));
        assert_eq!(full_alias("#lobby:other.net", "example.org").as_deref(), Some("#lobby:other.net"));
        assert_eq!(full_alias("", "example.org"), None);
        assert_eq!(full_alias("two words", "example.org"), None);
    }
}
