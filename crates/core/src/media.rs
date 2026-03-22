use matrix_sdk::{
    media::{MediaFormat, MediaRequestParameters},
    ruma::{events::room::MediaSource, MxcUri},
};

use crate::db::AppDb;
use crate::error::AppError;

pub fn bytes_to_data_uri(data: &[u8]) -> String {
    let content_type = if data.starts_with(&[0x89, 0x50, 0x4E, 0x47]) { "image/png" }
    else if data.starts_with(&[0xFF, 0xD8, 0xFF]) { "image/jpeg" }
    else if data.starts_with(b"GIF") { "image/gif" }
    else if data.starts_with(b"RIFF") && data.len() > 12 && &data[8..12] == b"WEBP" { "image/webp" }
    else { "image/png" };
    use base64::Engine;
    let b64 = base64::engine::general_purpose::STANDARD.encode(data);
    format!("data:{};base64,{}", content_type, b64)
}

/// Download media with 3-tier cache: memory → SQLite → network.
pub async fn get_media_url(
    client: &matrix_sdk::Client,
    db: &AppDb,
    cache: &std::sync::Mutex<std::collections::HashMap<String, String>>,
    mxc_uri: &str,
) -> Result<String, AppError> {
    // L1: memory
    { let c = cache.lock().unwrap(); if let Some(v) = c.get(mxc_uri) { return Ok(v.clone()); } }
    // L2: SQLite
    if let Some(v) = db.get_cached_media(mxc_uri) {
        cache.lock().unwrap().insert(mxc_uri.to_string(), v.clone());
        return Ok(v);
    }
    // L3: network
    let mxc = <&MxcUri>::try_from(mxc_uri).map_err(|_| AppError::Other("Invalid MXC URI".into()))?;
    let request = MediaRequestParameters { source: MediaSource::Plain(mxc.to_owned()), format: MediaFormat::File };
    let data = client.media().get_media_content(&request, true).await
        .map_err(|e| AppError::Other(format!("Failed to download media: {e}")))?;
    let data_uri = bytes_to_data_uri(&data);
    cache.lock().unwrap().insert(mxc_uri.to_string(), data_uri.clone());
    db.cache_media(mxc_uri, &data_uri);
    Ok(data_uri)
}

/// Fetch media from a room event (handles encrypted media).
pub async fn get_event_media(
    client: &matrix_sdk::Client,
    db: &AppDb,
    cache: &std::sync::Mutex<std::collections::HashMap<String, String>>,
    room_id: &str,
    event_id: &str,
) -> Result<String, AppError> {
    let cache_key = format!("event:{}", event_id);
    { let c = cache.lock().unwrap(); if let Some(v) = c.get(&cache_key) { return Ok(v.clone()); } }
    if let Some(v) = db.get_cached_media(&cache_key) {
        cache.lock().unwrap().insert(cache_key, v.clone());
        return Ok(v);
    }

    let room_id = matrix_sdk::ruma::RoomId::parse(room_id).map_err(|_| AppError::Other("Invalid room ID".into()))?;
    let room = client.get_room(&room_id).ok_or(AppError::RoomNotFound(room_id.to_string()))?;
    let event_id = matrix_sdk::ruma::EventId::parse(event_id).map_err(|_| AppError::Other("Invalid event ID".into()))?;
    let event = room.event(&event_id, None).await.map_err(|e| AppError::Other(format!("Failed to fetch event: {e}")))?;

    use matrix_sdk::ruma::events::{AnySyncTimelineEvent, AnySyncMessageLikeEvent, SyncMessageLikeEvent};
    use matrix_sdk::ruma::events::room::message::MessageType;

    let source = match event.raw().deserialize().map_err(|e| AppError::Other(e.to_string()))? {
        AnySyncTimelineEvent::MessageLike(AnySyncMessageLikeEvent::RoomMessage(SyncMessageLikeEvent::Original(msg))) => {
            match msg.content.msgtype { MessageType::Image(img) => img.source, _ => return Err(AppError::Other("Not an image".into())) }
        }
        _ => return Err(AppError::Other("Not a message event".into())),
    };

    let request = MediaRequestParameters { source, format: MediaFormat::File };
    let data = client.media().get_media_content(&request, true).await
        .map_err(|e| AppError::Other(format!("Failed to download: {e}")))?;
    let data_uri = bytes_to_data_uri(&data);
    cache.lock().unwrap().insert(cache_key.clone(), data_uri.clone());
    db.cache_media(&cache_key, &data_uri);
    Ok(data_uri)
}
