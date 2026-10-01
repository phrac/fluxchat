use matrix_sdk::media::{MediaFormat, MediaRequestParameters};

use crate::error::AppError;
use crate::rooms::parse_room_id;

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

/// Fetch raw media bytes from a room event (handles encrypted media).
/// Returns (bytes, mime_type). Used for video where data URIs are too large.
pub async fn get_event_media_bytes(
    client: &matrix_sdk::Client,
    room_id: &str,
    event_id: &str,
) -> Result<(Vec<u8>, String), AppError> {
    let room_id = parse_room_id(room_id)?;
    let room = client.get_room(&room_id).ok_or(AppError::RoomNotFound(room_id.to_string()))?;
    let event_id = matrix_sdk::ruma::EventId::parse(event_id).map_err(|_| AppError::Other("Invalid event ID".into()))?;
    let event = room.event(&event_id, None).await.map_err(|e| AppError::Other(format!("Failed to fetch event: {e}")))?;

    use matrix_sdk::ruma::events::{AnySyncTimelineEvent, AnySyncMessageLikeEvent, SyncMessageLikeEvent};
    use matrix_sdk::ruma::events::room::message::MessageType;

    let (source, mime) = match event.raw().deserialize().map_err(|e| AppError::Other(e.to_string()))? {
        AnySyncTimelineEvent::MessageLike(AnySyncMessageLikeEvent::RoomMessage(SyncMessageLikeEvent::Original(msg))) => {
            match msg.content.msgtype {
                MessageType::Image(img) => {
                    let mime = img.info.as_ref().and_then(|i| i.mimetype.clone()).unwrap_or_else(|| "image/png".into());
                    (img.source, mime)
                }
                MessageType::Video(vid) => {
                    let mime = vid.info.as_ref().and_then(|i| i.mimetype.clone()).unwrap_or_else(|| "video/mp4".into());
                    (vid.source, mime)
                }
                MessageType::Audio(audio) => {
                    let mime = audio.info.as_ref().and_then(|i| i.mimetype.clone()).unwrap_or_else(|| "audio/ogg".into());
                    (audio.source, mime)
                }
                MessageType::File(file) => {
                    let mime = file.info.as_ref().and_then(|i| i.mimetype.clone()).unwrap_or_else(|| "application/octet-stream".into());
                    (file.source, mime)
                }
                _ => return Err(AppError::Other("Not a media message".into())),
            }
        }
        AnySyncTimelineEvent::MessageLike(AnySyncMessageLikeEvent::Sticker(SyncMessageLikeEvent::Original(sticker))) => {
            let mime = sticker.content.info.mimetype.clone().unwrap_or_else(|| "image/png".into());
            match sticker.content.source {
                matrix_sdk::ruma::events::sticker::StickerMediaSource::Plain(uri) => {
                    (matrix_sdk::ruma::events::room::MediaSource::Plain(uri), mime)
                }
                matrix_sdk::ruma::events::sticker::StickerMediaSource::Encrypted(enc) => {
                    (matrix_sdk::ruma::events::room::MediaSource::Encrypted(enc), mime)
                }
                _ => return Err(AppError::Other("Unknown sticker source".into())),
            }
        }
        _ => return Err(AppError::Other("Not a message event".into())),
    };

    let request = MediaRequestParameters { source, format: MediaFormat::File };
    // Use false to skip SDK cache — ensures fresh decrypted download for encrypted media
    let data = client.media().get_media_content(&request, false).await
        .map_err(|e| AppError::Other(format!("Failed to download: {e}")))?;
    Ok((data, mime))
}
