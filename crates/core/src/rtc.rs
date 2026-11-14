//! MatrixRTC signaling — m.call.member state events, LiveKit JWT exchange.

use crate::error::AppError;
use serde::{Deserialize, Serialize};

/// A participant in a MatrixRTC call session.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CallParticipant {
    pub user_id: String,
    pub device_id: String,
    pub display_name: String,
    pub avatar_url: Option<String>,
    pub is_local: bool,
}

/// Connection state of our local call session.
#[derive(Debug, Clone, PartialEq)]
pub enum CallConnectionState {
    Disconnected,
    Connecting,
    Connected,
    Failed(String),
}

/// Info needed to connect to LiveKit after successful JWT exchange.
#[derive(Debug, Clone)]
pub struct CallJoinInfo {
    pub room_id: String,
    pub livekit_url: String,
    pub jwt: String,
}

/// OpenID token from the homeserver.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OpenIdToken {
    pub access_token: String,
    pub token_type: String,
    pub matrix_server_name: String,
    pub expires_in: u64,
}

// --- m.call.member event structures (MSC3401) ---

#[derive(Debug, Clone, Serialize, Deserialize)]
struct CallMembership {
    application: String,
    call_id: String,
    device_id: String,
    #[serde(default)]
    expires: u64,
    #[serde(default)]
    foci_active: Vec<serde_json::Value>,
    #[serde(default)]
    foci_preferred: Vec<serde_json::Value>,
    #[serde(default, rename = "membershipID")]
    membership_id: String,
}

// --- JWT exchange ---

#[derive(Debug, Deserialize)]
struct JwtResponse {
    url: String,
    jwt: String,
}

/// How long the .well-known and LiveKit token requests may take. reqwest has
/// no timeout of its own: a server that takes the connection and never
/// answers left a call at "connecting" (and the startup check for call
/// support pending) for good.
const HTTP_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(15);

static HTTP: std::sync::LazyLock<reqwest::Client> = std::sync::LazyLock::new(|| {
    reqwest::Client::builder().timeout(HTTP_TIMEOUT).build().unwrap_or_default()
});

/// Get the LiveKit service URL from the homeserver's .well-known.
pub async fn get_livekit_service_url(client: &matrix_sdk::Client) -> Result<String, AppError> {
    let user_id = client.user_id().ok_or(AppError::NotLoggedIn)?;
    let server = user_id.server_name().as_str();
    let well_known_url = format!("https://{server}/.well-known/matrix/client");

    let resp: serde_json::Value = HTTP.get(&well_known_url)
        .send()
        .await
        .map_err(|e| AppError::Other(format!("Failed to fetch .well-known: {e}")))?
        .json()
        .await
        .map_err(|e| AppError::Other(format!("Failed to parse .well-known: {e}")))?;

    let foci = resp
        .get("org.matrix.msc4143.rtc_foci")
        .and_then(|v| v.as_array())
        .ok_or_else(|| AppError::Other("No rtc_foci in .well-known".into()))?;

    for focus in foci {
        if focus.get("type").and_then(|v| v.as_str()) == Some("livekit") {
            if let Some(url) = focus.get("livekit_service_url").and_then(|v| v.as_str()) {
                return Ok(url.to_string());
            }
        }
    }

    Err(AppError::Other("No LiveKit focus found in .well-known".into()))
}

/// Get an OpenID token from the homeserver.
pub async fn get_openid_token(client: &matrix_sdk::Client) -> Result<OpenIdToken, AppError> {
    use matrix_sdk::ruma::api::client::account::request_openid_token::v3::Request;

    let user_id = client.user_id().ok_or(AppError::NotLoggedIn)?.to_owned();
    let request = Request::new(user_id.clone());
    let response = client
        .send(request)
        .await
        .map_err(|e| AppError::Other(format!("Failed to get OpenID token: {e}")))?;

    Ok(OpenIdToken {
        access_token: response.access_token,
        token_type: response.token_type.to_string(),
        matrix_server_name: user_id.server_name().to_string(),
        expires_in: response.expires_in.as_secs(),
    })
}

/// Exchange an OpenID token for a LiveKit JWT via the lk-jwt-service.
pub async fn get_livekit_jwt(
    service_url: &str,
    room_id: &str,
    openid_token: &OpenIdToken,
    device_id: &str,
) -> Result<CallJoinInfo, AppError> {
    let url = format!("{}/sfu/get", service_url.trim_end_matches('/'));

    let body = serde_json::json!({
        "room": room_id,
        "openid_token": {
            "access_token": openid_token.access_token,
            "token_type": openid_token.token_type,
            "matrix_server_name": openid_token.matrix_server_name,
            "expires_in": openid_token.expires_in,
        },
        "device_id": device_id,
    });

    let response = HTTP
        .post(&url)
        .json(&body)
        .send()
        .await
        .map_err(|e| AppError::Other(format!("Failed to request LiveKit JWT: {e}")))?;

    let status = response.status();
    let body_text = response.text().await
        .map_err(|e| AppError::Other(format!("Failed to read JWT response: {e}")))?;

    if !status.is_success() {
        return Err(AppError::Other(format!("LiveKit JWT request failed ({status}): {body_text}")));
    }

    let resp: JwtResponse = serde_json::from_str(&body_text)
        .map_err(|e| AppError::Other(format!("Failed to parse LiveKit JWT response: {e}")))?;

    Ok(CallJoinInfo {
        room_id: room_id.to_string(),
        livekit_url: resp.url,
        jwt: resp.jwt,
    })
}

/// The devices with a live membership in one call member event, as
/// (user ID, device ID).
fn live_memberships(event: &serde_json::Value, now_ms: u64) -> Vec<(String, String)> {
    let Some(content) = event.get("content") else { return Vec::new() };
    // Whose membership it is: the sender's. The state key is `@user:server`
    // in the older format but `_@user:server_DEVICE_m.call` in the per-device
    // one, which can't be taken apart at `_` (user IDs contain them:
    // `@first_last:server` came out as "@first") and is whatever its sender
    // chose — a key naming someone else must not put *them* in the call.
    let user = event.get("sender").and_then(|v| v.as_str()).filter(|s| !s.is_empty())
        .or_else(|| event.get("state_key").and_then(|v| v.as_str()))
        .unwrap_or("")
        .to_string();
    let origin_ts = event.get("origin_server_ts").and_then(|v| v.as_u64()).unwrap_or(0);
    // `expires` (ms after the event) is the sender's number as well: a huge
    // one must not wrap around into "expired long ago" (or, in a debug
    // build, panic the task that scans every room for calls).
    let live = |expires: u64| origin_ts.saturating_add(expires) > now_ms;

    // Legacy format: a "memberships" array, each entry with its own expiry.
    let memberships: Vec<CallMembership> = content
        .get("m.memberships")
        .or_else(|| content.get("memberships"))
        .and_then(|v| serde_json::from_value(v.clone()).ok())
        .unwrap_or_default();
    if !memberships.is_empty() {
        return memberships.into_iter()
            // No expiry info — treat as active
            .filter(|m| m.expires == 0 || origin_ts == 0 || live(m.expires))
            .map(|m| (user.clone(), m.device_id))
            .collect();
    }
    // Newer flat format: each state event IS a membership. Empty content
    // means they left.
    if content.get("application").is_none() && content.get("call_id").is_none() {
        return Vec::new();
    }
    let expires = content.get("expires").and_then(|v| v.as_u64()).unwrap_or(0);
    let active = if expires > 0 && origin_ts > 0 {
        live(expires)
    } else {
        // No expiry info — check if content has active fields
        content.get("application").is_some()
    };
    if !active {
        return Vec::new();
    }
    let device = content.get("device_id").and_then(|v| v.as_str()).unwrap_or("").to_string();
    vec![(user, device)]
}

/// Read m.call.member state events to get current call participants.
pub async fn get_call_members(
    client: &matrix_sdk::Client,
    room_id: &str,
) -> Result<Vec<CallParticipant>, AppError> {
    use crate::rooms::parse_room_id;
    use matrix_sdk::ruma::events::StateEventType;

    let room_id_parsed = parse_room_id(room_id)?;
    let room = client
        .get_room(&room_id_parsed)
        .ok_or(AppError::RoomNotFound(room_id.to_string()))?;

    let my_user_id = client
        .user_id()
        .map(|u| u.to_string())
        .unwrap_or_default();

    let mut participants = Vec::new();

    // Check multiple call member event types (MSC3401 unstable + stable)
    let mut raw_events = room
        .get_state_events(StateEventType::from("org.matrix.msc3401.call.member"))
        .await
        .unwrap_or_default();
    // Also check the stable event type
    raw_events.extend(room
        .get_state_events(StateEventType::from("m.call.member"))
        .await
        .unwrap_or_default());

    // Parse all events and deduplicate by state_key, keeping the latest by
    // origin_server_ts. Ordered, so the list doesn't reshuffle on every look.
    let mut events_by_state_key: std::collections::BTreeMap<String, serde_json::Value> = std::collections::BTreeMap::new();
    for raw_event in raw_events {
        let json_bytes = match &raw_event {
            matrix_sdk::deserialized_responses::RawAnySyncOrStrippedState::Sync(raw) => {
                raw.json().get().to_string()
            }
            matrix_sdk::deserialized_responses::RawAnySyncOrStrippedState::Stripped(raw) => {
                raw.json().get().to_string()
            }
        };

        let json: serde_json::Value = match serde_json::from_str(&json_bytes) {
            Ok(v) => v,
            Err(_) => continue,
        };

        let state_key = json.get("state_key").and_then(|v| v.as_str()).unwrap_or("").to_string();
        let origin_ts = json.get("origin_server_ts").and_then(|v| v.as_u64()).unwrap_or(0);

        let replace = match events_by_state_key.get(&state_key) {
            Some(existing) => {
                let existing_ts = existing.get("origin_server_ts").and_then(|v| v.as_u64()).unwrap_or(0);
                origin_ts > existing_ts
            }
            None => true,
        };
        if replace {
            events_by_state_key.insert(state_key, json);
        }
    }

    let now_ms = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as u64;
    for json in events_by_state_key.values() {
        for (user_id, device_id) in live_memberships(json, now_ms) {
            let (display_name, avatar_url) = match matrix_sdk::ruma::UserId::parse(&user_id) {
                Ok(uid) => match room.get_member_no_sync(&uid).await {
                    Ok(Some(member)) => (
                        member.display_name().unwrap_or(uid.localpart()).to_string(),
                        member.avatar_url().map(|u| u.to_string()),
                    ),
                    _ => (uid.localpart().to_string(), None),
                },
                Err(_) => (user_id.clone(), None),
            };
            participants.push(CallParticipant {
                is_local: user_id == my_user_id,
                user_id,
                device_id,
                display_name,
                avatar_url,
            });
        }
    }

    // Deduplicate by user_id
    let mut seen = std::collections::HashSet::new();
    participants.retain(|p| seen.insert(p.user_id.clone()));

    Ok(participants)
}

/// Check if a room has an active call.
pub async fn has_active_call(client: &matrix_sdk::Client, room_id: &str) -> bool {
    get_call_members(client, room_id)
        .await
        .map(|p| !p.is_empty())
        .unwrap_or(false)
}

/// Send m.call.member state event to join a call.
pub async fn send_call_member_event(
    client: &matrix_sdk::Client,
    room_id: &str,
    livekit_service_url: &str,
) -> Result<(), AppError> {
    use crate::rooms::parse_room_id;

    let room_id_parsed = parse_room_id(room_id)?;
    let room = client
        .get_room(&room_id_parsed)
        .ok_or(AppError::RoomNotFound(room_id.to_string()))?;

    let user_id = client.user_id().ok_or(AppError::NotLoggedIn)?;
    let device_id = client.device_id().ok_or(AppError::NotLoggedIn)?;
    let membership_id = uuid::Uuid::new_v4().to_string();

    let content = serde_json::json!({
        "memberships": [{
            "application": "m.call",
            "call_id": "",
            "device_id": device_id.as_str(),
            "expires": 14400000,
            "membershipID": membership_id,
            // Legacy field — older clients (pre-MSC4143)
            "foci_active": [{
                "type": "livekit",
                "livekit_alias": room_id,
                "livekit_service_url": livekit_service_url,
            }],
            // Current fields — Element and newer clients
            "foci_preferred": [{
                "type": "livekit",
                "livekit_service_url": livekit_service_url,
            }],
            "focus_active": {
                "type": "livekit",
                "focus_selection": "oldest_membership",
            },
        }]
    });

    room.send_state_event_raw(
        "org.matrix.msc3401.call.member",
        user_id.as_str(),
        content,
    )
    .await
    .map_err(|e| AppError::Other(format!("Failed to send call member event: {e}")))?;

    Ok(())
}

/// Remove m.call.member state event to leave a call.
pub async fn remove_call_member_event(
    client: &matrix_sdk::Client,
    room_id: &str,
) -> Result<(), AppError> {
    use crate::rooms::parse_room_id;

    let room_id_parsed = parse_room_id(room_id)?;
    let room = client
        .get_room(&room_id_parsed)
        .ok_or(AppError::RoomNotFound(room_id.to_string()))?;

    let user_id = client.user_id().ok_or(AppError::NotLoggedIn)?;

    let content = serde_json::json!({ "memberships": [] });

    room.send_state_event_raw(
        "org.matrix.msc3401.call.member",
        user_id.as_str(),
        content,
    )
    .await
    .map_err(|e| AppError::Other(format!("Failed to send leave event: {e}")))?;

    Ok(())
}

/// Full join flow: get service URL → get OpenID token → get JWT → send state event.
pub async fn join_call(client: &matrix_sdk::Client, room_id: &str) -> Result<CallJoinInfo, AppError> {
    let service_url = get_livekit_service_url(client).await?;
    let openid_token = get_openid_token(client).await?;
    let device_id = client
        .device_id()
        .ok_or(AppError::NotLoggedIn)?
        .as_str()
        .to_string();

    let join_info = get_livekit_jwt(&service_url, room_id, &openid_token, &device_id).await?;
    send_call_member_event(client, room_id, &service_url).await?;

    Ok(join_info)
}

/// Full leave flow: remove state event.
pub async fn leave_call(client: &matrix_sdk::Client, room_id: &str) -> Result<(), AppError> {
    remove_call_member_event(client, room_id).await
}

#[cfg(test)]
mod tests {
    use super::live_memberships;
    use serde_json::json;

    const NOW: u64 = 1_800_000_000_000;

    fn pair(user: &str, device: &str) -> (String, String) {
        (user.to_string(), device.to_string())
    }

    #[test]
    fn a_per_device_membership_belongs_to_its_sender() {
        // The user ID has underscores, like the state key around it.
        let event = json!({
            "type": "org.matrix.msc3401.call.member",
            "state_key": "_@first_last:example.org_DEVICE_m.call",
            "sender": "@first_last:example.org",
            "origin_server_ts": NOW - 1_000,
            "content": { "application": "m.call", "call_id": "", "device_id": "DEVICE", "expires": 14_400_000u64 },
        });
        assert_eq!(live_memberships(&event, NOW), vec![pair("@first_last:example.org", "DEVICE")]);

        // A key that names someone else doesn't put them in the call.
        let forged = json!({
            "state_key": "_@victim:example.org_DEVICE_m.call",
            "sender": "@mallory:example.org",
            "origin_server_ts": NOW - 1_000,
            "content": { "application": "m.call", "call_id": "", "device_id": "DEVICE", "expires": 14_400_000u64 },
        });
        assert_eq!(live_memberships(&forged, NOW), vec![pair("@mallory:example.org", "DEVICE")]);
    }

    #[test]
    fn a_huge_expiry_does_not_wrap_around() {
        let flat = json!({
            "state_key": "_@a:x_D_m.call",
            "sender": "@a:x",
            "origin_server_ts": NOW - 1_000,
            "content": { "application": "m.call", "call_id": "", "device_id": "D", "expires": u64::MAX },
        });
        assert_eq!(live_memberships(&flat, NOW), vec![pair("@a:x", "D")]);
        let legacy = json!({
            "state_key": "@a:x",
            "sender": "@a:x",
            "origin_server_ts": NOW - 1_000,
            "content": { "memberships": [
                { "application": "m.call", "call_id": "", "device_id": "D", "expires": u64::MAX },
            ] },
        });
        assert_eq!(live_memberships(&legacy, NOW), vec![pair("@a:x", "D")]);
    }

    #[test]
    fn expired_and_emptied_memberships_are_not_in_the_call() {
        let membership = |device: &str, expires: u64| json!({
            "application": "m.call", "call_id": "", "device_id": device, "expires": expires,
        });
        // Sent five hours ago: the 4 h one is over, the 6 h one isn't.
        let legacy = json!({
            "state_key": "@a:x",
            "sender": "@a:x",
            "origin_server_ts": NOW - 5 * 3_600_000,
            "content": { "memberships": [membership("OLD", 4 * 3_600_000), membership("NEW", 6 * 3_600_000)] },
        });
        assert_eq!(live_memberships(&legacy, NOW), vec![pair("@a:x", "NEW")]);

        let flat_expired = json!({
            "state_key": "_@a:x_D_m.call",
            "sender": "@a:x",
            "origin_server_ts": NOW - 5 * 3_600_000,
            "content": { "application": "m.call", "call_id": "", "device_id": "D", "expires": 4 * 3_600_000 },
        });
        assert!(live_memberships(&flat_expired, NOW).is_empty());

        // Leaving empties the content.
        for content in [json!({}), json!({ "memberships": [] })] {
            let left = json!({ "state_key": "@a:x", "sender": "@a:x", "origin_server_ts": NOW, "content": content });
            assert!(live_memberships(&left, NOW).is_empty());
        }
    }
}
