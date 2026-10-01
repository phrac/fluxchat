//! Account-wide notification rules stored as push rules on the server, so
//! they apply on every device: default levels for DMs and group rooms,
//! mentions, @room, keywords, and the invite / call / bot-notice rules.

use matrix_sdk::notification_settings::{IsEncrypted, IsOneToOne, RoomNotificationMode};
use matrix_sdk::ruma::push::{PredefinedContentRuleId, PredefinedOverrideRuleId, PredefinedUnderrideRuleId, RuleKind};

use crate::error::AppError;

/// Default level for a kind of room.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DefaultLevel {
    AllMessages,
    MentionsAndKeywords,
}

/// A toggleable rule.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NotifyToggle {
    /// Being mentioned (by ID, display name or username).
    MentionMe,
    /// `@room` mentions.
    MentionRoom,
    Invites,
    Calls,
    /// Messages from bots (`m.notice`).
    BotNotices,
    RoomUpgrades,
}

/// Current state of the account-wide rules.
#[derive(Debug, Clone)]
pub struct NotifyRules {
    pub dms: DefaultLevel,
    pub groups: DefaultLevel,
    pub mention_me: bool,
    pub mention_room: bool,
    pub invites: bool,
    pub calls: bool,
    pub bot_notices: bool,
    pub room_upgrades: bool,
    pub keywords: Vec<String>,
}

fn err(e: impl std::fmt::Display) -> AppError {
    AppError::Other(e.to_string())
}

/// The push rules behind each toggle. A toggle is on when its first
/// rule is enabled (for bot notices, when the suppress rule is *off*).
/// The legacy display-name / username / roomnotif rules are deprecated but
/// still on servers and still fire for messages without `m.mentions`
/// (bridges, older clients), so they're switched together with the new ones.
#[allow(deprecated)]
fn rules_for(t: NotifyToggle) -> Vec<(RuleKind, String)> {
    let o = |r: PredefinedOverrideRuleId| (RuleKind::Override, r.as_str().to_string());
    let u = |r: PredefinedUnderrideRuleId| (RuleKind::Underride, r.as_str().to_string());
    match t {
        NotifyToggle::MentionMe => vec![
            o(PredefinedOverrideRuleId::IsUserMention),
            o(PredefinedOverrideRuleId::ContainsDisplayName),
            (RuleKind::Content, PredefinedContentRuleId::ContainsUserName.as_str().to_string()),
        ],
        NotifyToggle::MentionRoom => vec![o(PredefinedOverrideRuleId::IsRoomMention), o(PredefinedOverrideRuleId::RoomNotif)],
        NotifyToggle::Invites => vec![o(PredefinedOverrideRuleId::InviteForMe)],
        NotifyToggle::Calls => vec![u(PredefinedUnderrideRuleId::Call)],
        NotifyToggle::BotNotices => vec![o(PredefinedOverrideRuleId::SuppressNotices)],
        NotifyToggle::RoomUpgrades => vec![o(PredefinedOverrideRuleId::Tombstone)],
    }
}

fn level(mode: RoomNotificationMode) -> DefaultLevel {
    match mode {
        RoomNotificationMode::AllMessages => DefaultLevel::AllMessages,
        _ => DefaultLevel::MentionsAndKeywords,
    }
}

/// Read the account-wide rules.
pub async fn notify_rules(client: &matrix_sdk::Client) -> Result<NotifyRules, AppError> {
    let s = client.notification_settings().await;
    let enabled = |t: NotifyToggle| {
        let s = &s;
        async move {
            // First rule that exists on this server decides.
            for (kind, id) in rules_for(t) {
                if let Ok(on) = s.is_push_rule_enabled(kind, &id).await {
                    return Some(on);
                }
            }
            None
        }
    };
    Ok(NotifyRules {
        dms: level(s.get_default_room_notification_mode(IsEncrypted::No, IsOneToOne::Yes).await),
        groups: level(s.get_default_room_notification_mode(IsEncrypted::No, IsOneToOne::No).await),
        mention_me: enabled(NotifyToggle::MentionMe).await.unwrap_or(true),
        mention_room: enabled(NotifyToggle::MentionRoom).await.unwrap_or(true),
        invites: enabled(NotifyToggle::Invites).await.unwrap_or(true),
        calls: enabled(NotifyToggle::Calls).await.unwrap_or(true),
        // The predefined rule *suppresses* notices.
        bot_notices: !enabled(NotifyToggle::BotNotices).await.unwrap_or(true),
        room_upgrades: enabled(NotifyToggle::RoomUpgrades).await.unwrap_or(true),
        keywords: s.enabled_keywords().await.into_iter().collect(),
    })
}

/// Set the default level for DMs (`dm = true`) or group rooms, for both
/// encrypted and unencrypted rooms.
pub async fn set_default_level(client: &matrix_sdk::Client, dm: bool, lvl: DefaultLevel) -> Result<(), AppError> {
    let s = client.notification_settings().await;
    let mode = match lvl {
        DefaultLevel::AllMessages => RoomNotificationMode::AllMessages,
        DefaultLevel::MentionsAndKeywords => RoomNotificationMode::MentionsAndKeywordsOnly,
    };
    let one_to_one = if dm { IsOneToOne::Yes } else { IsOneToOne::No };
    for enc in [IsEncrypted::No, IsEncrypted::Yes] {
        s.set_default_room_notification_mode(enc, one_to_one, mode).await.map_err(err)?;
    }
    Ok(())
}

/// Turn a rule group on or off. Rules this server doesn't have are skipped.
pub async fn set_toggle(client: &matrix_sdk::Client, t: NotifyToggle, on: bool) -> Result<(), AppError> {
    let s = client.notification_settings().await;
    let enable = if t == NotifyToggle::BotNotices { !on } else { on };
    let mut any = false;
    for (kind, id) in rules_for(t) {
        if s.is_push_rule_enabled(kind.clone(), &id).await.is_err() {
            continue; // not on this server
        }
        s.set_push_rule_enabled(kind, &id, enable).await.map_err(err)?;
        any = true;
    }
    if any { Ok(()) } else { Err(err("Your server doesn't support this setting")) }
}

/// Add a keyword that notifies (and highlights) wherever it appears.
pub async fn add_keyword(client: &matrix_sdk::Client, keyword: &str) -> Result<(), AppError> {
    client.notification_settings().await.add_keyword(keyword.trim().to_string()).await.map_err(err)
}

/// Remove a keyword.
pub async fn remove_keyword(client: &matrix_sdk::Client, keyword: &str) -> Result<(), AppError> {
    client.notification_settings().await.remove_keyword(keyword).await.map_err(err)
}
