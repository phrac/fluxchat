use matrix_sdk::{
    media::{MediaFormat, MediaRequestParameters, MediaThumbnailSettings},
    ruma::{events::room::MediaSource, MxcUri},
};

use crate::db::AppDb;
use crate::error::AppError;
use crate::rooms::parse_room_id;

/// Maximum entries in the in-memory media cache before eviction.
const MEDIA_CACHE_LIMIT: usize = 500;
/// Number of entries to evict when cache is full.
const MEDIA_CACHE_EVICT: usize = 100;

/// Generate a thumbnail from image data. Returns (png_bytes, width, height) or None.
pub fn generate_thumbnail(data: &[u8], max_w: u32, max_h: u32) -> Option<(Vec<u8>, u32, u32)> {
    let img = image::load_from_memory(data).ok()?;
    let thumb = img.thumbnail(max_w, max_h);
    let (w, h) = (thumb.width(), thumb.height());
    let mut buf = std::io::Cursor::new(Vec::new());
    thumb.write_to(&mut buf, image::ImageFormat::Png).ok()?;
    Some((buf.into_inner(), w, h))
}

/// Get original image dimensions without full decode.
pub fn get_image_dimensions(data: &[u8]) -> Option<(u32, u32)> {
    let img = image::load_from_memory(data).ok()?;
    Some((img.width(), img.height()))
}

/// Convert raw image bytes to a base64 data URI, detecting format from magic bytes.
pub fn bytes_to_data_uri(data: &[u8]) -> String {
    let content_type = if data.starts_with(&[0x89, 0x50, 0x4E, 0x47]) { "image/png" }
    else if data.starts_with(&[0xFF, 0xD8, 0xFF]) { "image/jpeg" }
    else if data.starts_with(b"GIF") { "image/gif" }
    else if data.starts_with(b"RIFF") && data.len() > 12 && &data[8..12] == b"WEBP" { "image/webp" }
    else if data.len() > 8 && &data[4..8] == b"ftyp" { "video/mp4" }
    else if data.starts_with(&[0x1A, 0x45, 0xDF, 0xA3]) { "video/webm" }
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
    let mxc = <&MxcUri>::from(mxc_uri);
    let request = MediaRequestParameters { source: MediaSource::Plain(mxc.to_owned()), format: MediaFormat::File };
    let data = client.media().get_media_content(&request, true).await
        .map_err(|e| AppError::Other(format!("Failed to download media: {e}")))?;
    let data_uri = bytes_to_data_uri(&data);
    let mut c = cache.lock().unwrap();
    // Cap memory cache at 500 entries — evict oldest when full
    if c.len() >= MEDIA_CACHE_LIMIT {
        let keys: Vec<String> = c.keys().take(MEDIA_CACHE_EVICT).cloned().collect();
        for k in keys { c.remove(&k); }
    }
    c.insert(mxc_uri.to_string(), data_uri.clone());
    drop(c);
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

    let room_id = parse_room_id(room_id)?;
    let room = client.get_room(&room_id).ok_or(AppError::RoomNotFound(room_id.to_string()))?;
    let event_id = matrix_sdk::ruma::EventId::parse(event_id).map_err(|_| AppError::Other("Invalid event ID".into()))?;
    let event = room.event(&event_id, None).await.map_err(|e| AppError::Other(format!("Failed to fetch event: {e}")))?;

    use matrix_sdk::ruma::events::{AnySyncTimelineEvent, AnySyncMessageLikeEvent, SyncMessageLikeEvent};
    use matrix_sdk::ruma::events::room::message::MessageType;

    let source = match event.raw().deserialize().map_err(|e| AppError::Other(e.to_string()))? {
        AnySyncTimelineEvent::MessageLike(AnySyncMessageLikeEvent::RoomMessage(SyncMessageLikeEvent::Original(msg))) => {
            match msg.content.msgtype {
                MessageType::Image(img) => img.source,
                MessageType::Video(vid) => vid.source,
                _ => return Err(AppError::Other("Not a media message".into())),
            }
        }
        AnySyncTimelineEvent::MessageLike(AnySyncMessageLikeEvent::Sticker(SyncMessageLikeEvent::Original(sticker))) => {
            match sticker.content.source {
                matrix_sdk::ruma::events::sticker::StickerMediaSource::Plain(uri) => MediaSource::Plain(uri),
                _ => return Err(AppError::Other("Encrypted stickers not supported".into())),
            }
        }
        _ => return Err(AppError::Other("Not a message event".into())),
    };

    let request = MediaRequestParameters { source, format: MediaFormat::File };
    let data = client.media().get_media_content(&request, true).await
        .map_err(|e| AppError::Other(format!("Failed to download: {e}")))?;
    let data_uri = bytes_to_data_uri(&data);
    let mut c = cache.lock().unwrap();
    if c.len() >= MEDIA_CACHE_LIMIT {
        let keys: Vec<String> = c.keys().take(MEDIA_CACHE_EVICT).cloned().collect();
        for k in keys { c.remove(&k); }
    }
    c.insert(cache_key.clone(), data_uri.clone());
    drop(c);
    db.cache_media(&cache_key, &data_uri);
    Ok(data_uri)
}

/// Download a thumbnail of media with 3-tier cache.
pub async fn get_media_thumbnail(
    client: &matrix_sdk::Client,
    db: &AppDb,
    cache: &std::sync::Mutex<std::collections::HashMap<String, String>>,
    mxc_uri: &str,
    width: u32,
    height: u32,
) -> Result<String, AppError> {
    let cache_key = format!("thumb:{}x{}:{}", width, height, mxc_uri);
    { let c = cache.lock().unwrap(); if let Some(v) = c.get(&cache_key) { return Ok(v.clone()); } }
    if let Some(v) = db.get_cached_media(&cache_key) {
        cache.lock().unwrap().insert(cache_key, v.clone());
        return Ok(v);
    }
    let mxc = <&MxcUri>::from(mxc_uri);
    let settings = MediaThumbnailSettings::new(width.into(), height.into());
    let request = MediaRequestParameters { source: MediaSource::Plain(mxc.to_owned()), format: MediaFormat::Thumbnail(settings) };
    // Don't fallback to full file here — let the caller handle encrypted media via get_event_media
    let data = client.media().get_media_content(&request, true).await
        .map_err(|e| AppError::Other(format!("Failed to download thumbnail: {e}")))?;
    let data_uri = bytes_to_data_uri(&data);
    let mut c = cache.lock().unwrap();
    if c.len() >= MEDIA_CACHE_LIMIT {
        let keys: Vec<String> = c.keys().take(MEDIA_CACHE_EVICT).cloned().collect();
        for k in keys { c.remove(&k); }
    }
    c.insert(cache_key.clone(), data_uri.clone());
    drop(c);
    db.cache_media(&cache_key, &data_uri);
    Ok(data_uri)
}
