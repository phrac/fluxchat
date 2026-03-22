use matrix_sdk::{
    media::{MediaFormat, MediaRequestParameters},
    ruma::{events::room::MediaSource, MxcUri},
};
use tauri::State;

use crate::db;
use crate::state::AppState;

use super::AppError;

/// Download media via the Matrix client and return it as a base64 data URI.
/// Three-tier cache: in-memory HashMap → FluxDB on disk → network fetch.
#[tauri::command]
pub async fn get_media_url(
    state: State<'_, AppState>,
    mxc_uri: String,
) -> Result<String, AppError> {
    // L1: in-memory cache
    {
        let cache = state.media_cache.lock().unwrap();
        if let Some(data_uri) = cache.get(&mxc_uri) {
            return Ok(data_uri.clone());
        }
    }

    // L2: FluxDB disk cache
    if let Some(data_uri) = db::get_cached_media(&state.db, &mxc_uri) {
        let mut cache = state.media_cache.lock().unwrap();
        cache.insert(mxc_uri, data_uri.clone());
        return Ok(data_uri);
    }

    // L3: network fetch
    let client = {
        let lock = state.client.lock().await;
        lock.as_ref().cloned().ok_or(AppError::NotLoggedIn)?
    };

    let mxc = <&MxcUri>::try_from(mxc_uri.as_str())
        .map_err(|_| AppError::Other("Invalid MXC URI".into()))?;

    let request = MediaRequestParameters {
        source: MediaSource::Plain(mxc.to_owned()),
        format: MediaFormat::File,
    };

    let data = client
        .media()
        .get_media_content(&request, true)
        .await
        .map_err(|e| AppError::Other(format!("Failed to download media: {e}")))?;

    let data_uri = bytes_to_data_uri(&data);

    // Store in L1 (memory)
    {
        let mut cache = state.media_cache.lock().unwrap();
        cache.insert(mxc_uri.clone(), data_uri.clone());
    }

    // Store in L2 (FluxDB disk)
    db::cache_media(&state.db, &mxc_uri, &data_uri);

    Ok(data_uri)
}

/// Fetch an image from a room event, handling both encrypted and unencrypted media.
#[tauri::command]
pub async fn get_event_media(
    state: State<'_, AppState>,
    room_id: String,
    event_id: String,
) -> Result<String, AppError> {
    // Check cache first
    let cache_key = format!("event:{}", event_id);
    {
        let cache = state.media_cache.lock().unwrap();
        if let Some(data_uri) = cache.get(&cache_key) {
            return Ok(data_uri.clone());
        }
    }

    if let Some(data_uri) = db::get_cached_media(&state.db, &cache_key) {
        let mut cache = state.media_cache.lock().unwrap();
        cache.insert(cache_key, data_uri.clone());
        return Ok(data_uri);
    }

    let client = {
        let lock = state.client.lock().await;
        lock.as_ref().cloned().ok_or(AppError::NotLoggedIn)?
    };

    let room_id = matrix_sdk::ruma::RoomId::parse(&room_id)
        .map_err(|_| AppError::Other("Invalid room ID".into()))?;
    let room = client
        .get_room(&room_id)
        .ok_or(AppError::RoomNotFound(room_id.to_string()))?;

    let event_id = matrix_sdk::ruma::EventId::parse(&event_id)
        .map_err(|_| AppError::Other("Invalid event ID".into()))?;

    let event = room
        .event(&event_id, None)
        .await
        .map_err(|e| AppError::Other(format!("Failed to fetch event: {e}")))?;

    let raw = event.raw();
    let deserialized = raw
        .deserialize()
        .map_err(|e| AppError::Other(format!("Failed to deserialize: {e}")))?;

    use matrix_sdk::ruma::events::{AnySyncTimelineEvent, AnySyncMessageLikeEvent, SyncMessageLikeEvent};
    use matrix_sdk::ruma::events::room::message::MessageType;

    let source = match deserialized {
        AnySyncTimelineEvent::MessageLike(AnySyncMessageLikeEvent::RoomMessage(
            SyncMessageLikeEvent::Original(msg),
        )) => match msg.content.msgtype {
            MessageType::Image(img) => img.source,
            _ => return Err(AppError::Other("Not an image message".into())),
        },
        _ => return Err(AppError::Other("Not a message event".into())),
    };

    let request = MediaRequestParameters {
        source,
        format: MediaFormat::File,
    };

    let data = client
        .media()
        .get_media_content(&request, true)
        .await
        .map_err(|e| AppError::Other(format!("Failed to download media: {e}")))?;

    let data_uri = bytes_to_data_uri(&data);

    {
        let mut cache = state.media_cache.lock().unwrap();
        cache.insert(cache_key.clone(), data_uri.clone());
    }
    db::cache_media(&state.db, &cache_key, &data_uri);

    Ok(data_uri)
}

fn bytes_to_data_uri(data: &[u8]) -> String {
    let content_type = if data.starts_with(&[0x89, 0x50, 0x4E, 0x47]) {
        "image/png"
    } else if data.starts_with(&[0xFF, 0xD8, 0xFF]) {
        "image/jpeg"
    } else if data.starts_with(b"GIF") {
        "image/gif"
    } else if data.starts_with(b"RIFF") && data.len() > 12 && &data[8..12] == b"WEBP" {
        "image/webp"
    } else {
        "image/png"
    };

    use base64::Engine;
    let b64 = base64::engine::general_purpose::STANDARD.encode(data);
    format!("data:{};base64,{}", content_type, b64)
}
