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
    #[serde(default, rename = "membershipID")]
    membership_id: String,
}

// --- JWT exchange ---

#[derive(Debug, Deserialize)]
struct JwtResponse {
    url: String,
    jwt: String,
}

/// Get the LiveKit service URL from the homeserver's .well-known.
pub async fn get_livekit_service_url(client: &matrix_sdk::Client) -> Result<String, AppError> {
    let user_id = client.user_id().ok_or(AppError::NotLoggedIn)?;
    let server = user_id.server_name().as_str();
    let well_known_url = format!("https://{server}/.well-known/matrix/client");

    let resp: serde_json::Value = reqwest::get(&well_known_url)
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

    let http = reqwest::Client::new();
    let response = http
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

    for raw_event in raw_events {
        // Extract the raw JSON from the enum
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

        let sender = json
            .get("state_key")
            .or_else(|| json.get("sender"))
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string();

        let content = match json.get("content") {
            Some(c) => c,
            None => continue,
        };

        // Check both legacy "memberships" array and newer flat format
        let memberships: Vec<CallMembership> = content
            .get("m.memberships")
            .or_else(|| content.get("memberships"))
            .and_then(|v| serde_json::from_value(v.clone()).ok())
            .unwrap_or_default();

        if !memberships.is_empty() {
            // Legacy format: memberships array
        } else if content.get("application").is_some() || content.get("call_id").is_some() {
            // Newer flat format: each state event IS a membership
            // expires is a duration in ms (not absolute timestamp) — if > 0, membership is active
            let is_active = content.get("expires")
                .and_then(|v| v.as_u64())
                .map(|exp| exp > 0)
                .unwrap_or(true);

            if is_active {
                let device_id = content.get("device_id")
                    .and_then(|v| v.as_str())
                    .unwrap_or("")
                    .to_string();
                // Extract user_id from composite state key: _@user:server_deviceId_callType
                let user_id_str = sender.split('_')
                    .find(|s| s.starts_with('@'))
                    .unwrap_or(&sender)
                    .to_string();

                let (display_name, avatar_url) =
                    if let Ok(uid) = matrix_sdk::ruma::UserId::parse(&user_id_str) {
                        if let Ok(Some(member)) = room.get_member_no_sync(&uid).await {
                            (
                                member.display_name().unwrap_or(uid.localpart()).to_string(),
                                member.avatar_url().map(|u| u.to_string()),
                            )
                        } else {
                            (user_id_str.trim_start_matches('@').split(':').next().unwrap_or(&user_id_str).to_string(), None)
                        }
                    } else {
                        (user_id_str.clone(), None)
                    };

                participants.push(CallParticipant {
                    user_id: user_id_str.clone(),
                    device_id,
                    display_name,
                    avatar_url,
                    is_local: user_id_str == my_user_id,
                });
                continue;
            } else {
                continue; // Expired membership
            }
        } else {
            continue; // Empty content
        }

        for membership in memberships {
            let (display_name, avatar_url) =
                if let Ok(user_id) = matrix_sdk::ruma::UserId::parse(&sender) {
                    if let Ok(Some(member)) = room.get_member_no_sync(&user_id).await {
                        (
                            member
                                .display_name()
                                .unwrap_or(user_id.localpart())
                                .to_string(),
                            member.avatar_url().map(|u| u.to_string()),
                        )
                    } else {
                        (sender.trim_start_matches('@').split(':').next().unwrap_or(&sender).to_string(), None)
                    }
                } else {
                    (sender.clone(), None)
                };

            participants.push(CallParticipant {
                user_id: sender.clone(),
                device_id: membership.device_id,
                display_name,
                avatar_url,
                is_local: sender == my_user_id,
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
            "expires": 3600000,
            "foci_active": [{
                "type": "livekit",
                "livekit_alias": room_id,
                "livekit_service_url": livekit_service_url,
            }],
            "membershipID": membership_id,
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
