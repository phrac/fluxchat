//! Media gallery: every image, video and file in a room or thread, fetched
//! from the server page by page (newest first) and decrypted where we have
//! the keys, so the gallery isn't limited to the history that's loaded.

use std::collections::HashMap;

use matrix_sdk::{
    deserialized_responses::TimelineEvent,
    room::{IncludeRelations, MessagesOptions, RelationsOptions},
    ruma::{
        api::{client::filter::UrlFilter, Direction},
        events::{
            relation::RelationType,
            room::{
                message::{MessageType, Relation},
                MediaSource,
            },
            sticker::StickerMediaSource,
            AnySyncMessageLikeEvent, AnySyncTimelineEvent, SyncMessageLikeEvent,
        },
        EventId, UInt,
    },
};

use crate::error::AppError;
use crate::models::MessageInfo;
use crate::rooms::parse_room_id;

/// Events requested per page. Encrypted rooms can't be filtered by the
/// server, so most of a page there is ordinary messages.
const GALLERY_PAGE_SIZE: u32 = 100;

/// One page of gallery items.
#[derive(Debug, Clone, Default)]
pub struct MediaPage {
    /// Media and file messages in the page, newest first.
    pub items: Vec<MessageInfo>,
    /// Token for the next (older) page; `None` once the start is reached.
    pub next: Option<String>,
    /// Events the server returned, media or not.
    pub scanned: usize,
}

/// A page of a room's media, older than `from` (or the newest, without it).
/// Thread replies are included.
pub async fn room_media_page(
    client: &matrix_sdk::Client,
    room_id: &str,
    from: Option<String>,
) -> Result<MediaPage, AppError> {
    let rid = parse_room_id(room_id)?;
    let room = client.get_room(&rid).ok_or(AppError::RoomNotFound(rid.to_string()))?;

    let mut options = MessagesOptions::backward();
    options.limit = GALLERY_PAGE_SIZE.into();
    options.from = from;
    // The server can only see attachments in unencrypted events.
    if room.encryption_state().is_encrypted() {
        options.filter.types = Some(vec!["m.room.encrypted".into(), "m.room.message".into(), "m.sticker".into()]);
    } else {
        options.filter.types = Some(vec!["m.room.message".into(), "m.sticker".into()]);
        options.filter.url_filter = Some(UrlFilter::EventsWithUrl);
    }
    let response = room.messages(options).await
        .map_err(|e| AppError::Other(format!("Couldn't load media: {e}")))?;

    let scanned = response.chunk.len();
    let items = to_items(&room, &response.chunk, media_info).await;
    // An empty page means the start of the room, whatever the token says.
    let next = if scanned == 0 { None } else { response.end };
    Ok(MediaPage { items, next, scanned })
}

/// A page of a thread's media (replies, plus the root on the first page).
pub async fn thread_media_page(
    client: &matrix_sdk::Client,
    room_id: &str,
    root: &str,
    from: Option<String>,
) -> Result<MediaPage, AppError> {
    let rid = parse_room_id(room_id)?;
    let room = client.get_room(&rid).ok_or(AppError::RoomNotFound(rid.to_string()))?;
    let root_id = EventId::parse(root).map_err(|_| AppError::Other("Invalid event ID".into()))?;

    let first = from.is_none();
    let options = RelationsOptions {
        from,
        dir: Direction::Backward,
        limit: Some(UInt::from(GALLERY_PAGE_SIZE)),
        include_relations: IncludeRelations::RelationsOfType(RelationType::Thread),
        recurse: false,
    };
    let response = room.relations(root_id.clone(), options).await
        .map_err(|e| AppError::Other(format!("Couldn't load media: {e}")))?;

    let mut events = response.chunk;
    let scanned = events.len();
    let next = response.next_batch_token.filter(|_| scanned > 0);
    // The root is the oldest message: it belongs on the last page, but the
    // first is the only one we know to fetch it on (threads are short).
    if first {
        if let Ok(ev) = room.event(&root_id, None).await {
            events.push(ev);
        }
    }
    let items = to_items(&room, &events, media_info).await;
    Ok(MediaPage { items, next, scanned })
}

/// The messages among `events` that `pick` makes an item of, with sender
/// names.
pub(crate) async fn to_items(
    room: &matrix_sdk::Room,
    events: &[TimelineEvent],
    pick: fn(&AnySyncTimelineEvent) -> Option<MessageInfo>,
) -> Vec<MessageInfo> {
    let mut names: HashMap<String, String> = HashMap::new();
    let mut items = Vec::new();
    for event in events {
        let Ok(parsed) = event.raw().deserialize() else { continue };
        let Some(mut info) = pick(&parsed) else { continue };
        info.sender_name = match names.get(&info.sender) {
            Some(name) => name.clone(),
            None => {
                let name = match matrix_sdk::ruma::UserId::parse(&info.sender) {
                    Ok(uid) => room.get_member_no_sync(&uid).await.ok().flatten()
                        .and_then(|m| m.display_name().map(str::to_owned)),
                    Err(_) => None,
                }
                .unwrap_or_else(|| info.sender.clone());
                names.insert(info.sender.clone(), name.clone());
                name
            }
        };
        items.push(info);
    }
    items
}

/// The URL a media source is known by — the same key the timeline uses, so
/// images it already loaded are reused.
fn source_key(source: &MediaSource) -> String {
    match source {
        MediaSource::Plain(uri) => uri.to_string(),
        MediaSource::Encrypted(file) => file.url.to_string(),
    }
}

/// A gallery item for a media or file message, `None` for anything else
/// (text, edits, redacted or undecryptable events).
pub fn media_info(event: &AnySyncTimelineEvent) -> Option<MessageInfo> {
    let AnySyncTimelineEvent::MessageLike(msg_like) = event else { return None };
    let mut info = MessageInfo {
        event_id: event.event_id().to_string(),
        sender: event.sender().to_string(),
        timestamp: event.origin_server_ts().as_secs().into(),
        ..Default::default()
    };
    match msg_like {
        AnySyncMessageLikeEvent::RoomMessage(SyncMessageLikeEvent::Original(msg)) => {
            match &msg.content.relates_to {
                // An edit's new content belongs to the original event.
                Some(Relation::Replacement(_)) => return None,
                Some(Relation::Thread(thread)) => info.thread_root = Some(thread.event_id.to_string()),
                _ => {}
            }
            let dims = |w: Option<UInt>, h: Option<UInt>| {
                (w.and_then(|v| u32::try_from(v).ok()), h.and_then(|v| u32::try_from(v).ok()))
            };
            match &msg.content.msgtype {
                MessageType::Image(img) => {
                    info.body = file_name(img.filename.as_deref(), &img.body);
                    info.image_mxc = Some(source_key(&img.source));
                    (info.image_width, info.image_height) =
                        img.info.as_ref().map(|i| dims(i.width, i.height)).unwrap_or_default();
                }
                MessageType::Video(vid) => {
                    info.body = file_name(vid.filename.as_deref(), &vid.body);
                    info.is_video = true;
                    info.video_mxc = Some(source_key(&vid.source));
                    info.image_mxc = vid.info.as_ref()
                        .and_then(|i| i.thumbnail_source.as_ref())
                        .map(source_key);
                    (info.image_width, info.image_height) =
                        vid.info.as_ref().map(|i| dims(i.width, i.height)).unwrap_or_default();
                    info.file_size = vid.info.as_ref().and_then(|i| i.size).map(u64::from);
                }
                MessageType::File(file) => {
                    info.body = file_name(file.filename.as_deref(), &file.body);
                    let mimetype = file.info.as_ref().and_then(|i| i.mimetype.clone());
                    // Images sent as files show as images, like in the timeline.
                    if mimetype.as_deref().is_some_and(|m| m.starts_with("image/")) {
                        info.image_mxc = Some(source_key(&file.source));
                    } else {
                        info.file_mxc = Some(source_key(&file.source));
                        info.file_mimetype = mimetype;
                        info.file_size = file.info.as_ref().and_then(|i| i.size).map(u64::from);
                    }
                }
                MessageType::Audio(audio) => {
                    info.body = file_name(audio.filename.as_deref(), &audio.body);
                    info.audio_mxc = Some(source_key(&audio.source));
                    info.file_mimetype = audio.info.as_ref().and_then(|i| i.mimetype.clone());
                    info.file_size = audio.info.as_ref().and_then(|i| i.size).map(u64::from);
                    info.duration_ms = audio.info.as_ref().and_then(|i| i.duration).map(|d| d.as_millis() as u64);
                    info.is_voice = audio.voice.is_some();
                }
                _ => return None,
            }
        }
        AnySyncMessageLikeEvent::Sticker(SyncMessageLikeEvent::Original(sticker)) => {
            info.body = sticker.content.body.clone();
            info.image_mxc = match &sticker.content.source {
                StickerMediaSource::Plain(uri) => Some(uri.to_string()),
                StickerMediaSource::Encrypted(file) => Some(file.url.to_string()),
                _ => return None,
            };
            info.image_width = sticker.content.info.width.and_then(|v| u32::try_from(v).ok());
            info.image_height = sticker.content.info.height.and_then(|v| u32::try_from(v).ok());
        }
        _ => return None,
    }
    Some(info)
}

/// An attachment's file name: `filename` when the body is a caption
/// (Matrix v1.10), otherwise the body.
fn file_name(filename: Option<&str>, body: &str) -> String {
    filename.filter(|f| !f.trim().is_empty()).unwrap_or(body).to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn parse(json: serde_json::Value) -> AnySyncTimelineEvent {
        serde_json::from_value(json).unwrap()
    }

    fn message(content: serde_json::Value) -> AnySyncTimelineEvent {
        parse(serde_json::json!({
            "type": "m.room.message",
            "event_id": "$e:x",
            "sender": "@a:x",
            "origin_server_ts": 1_700_000_000_000u64,
            "content": content,
        }))
    }

    #[test]
    fn image_with_caption_uses_file_name() {
        let info = media_info(&message(serde_json::json!({
            "msgtype": "m.image", "body": "look at this", "filename": "cat.png",
            "url": "mxc://x/cat", "info": { "w": 640, "h": 480 },
        }))).unwrap();
        assert_eq!(info.body, "cat.png");
        assert_eq!(info.image_mxc.as_deref(), Some("mxc://x/cat"));
        assert_eq!((info.image_width, info.image_height), (Some(640), Some(480)));
        assert_eq!(info.timestamp, 1_700_000_000);
    }

    #[test]
    fn files_and_images_sent_as_files() {
        let pdf = media_info(&message(serde_json::json!({
            "msgtype": "m.file", "body": "doc.pdf", "url": "mxc://x/pdf",
            "info": { "mimetype": "application/pdf", "size": 1234 },
        }))).unwrap();
        assert_eq!(pdf.file_mxc.as_deref(), Some("mxc://x/pdf"));
        assert_eq!(pdf.file_size, Some(1234));
        assert!(pdf.image_mxc.is_none());

        let png = media_info(&message(serde_json::json!({
            "msgtype": "m.file", "body": "shot.png", "url": "mxc://x/png",
            "info": { "mimetype": "image/png" },
        }))).unwrap();
        assert_eq!(png.image_mxc.as_deref(), Some("mxc://x/png"));
        assert!(png.file_mxc.is_none());
    }

    #[test]
    fn video_uses_its_thumbnail_and_thread_is_kept() {
        let info = media_info(&message(serde_json::json!({
            "msgtype": "m.video", "body": "clip.mp4", "url": "mxc://x/vid",
            "info": { "thumbnail_url": "mxc://x/thumb" },
            "m.relates_to": { "rel_type": "m.thread", "event_id": "$root:x" },
        }))).unwrap();
        assert!(info.is_video);
        assert_eq!(info.video_mxc.as_deref(), Some("mxc://x/vid"));
        assert_eq!(info.image_mxc.as_deref(), Some("mxc://x/thumb"));
        assert_eq!(info.thread_root.as_deref(), Some("$root:x"));
    }

    #[test]
    fn text_and_edits_are_skipped() {
        assert!(media_info(&message(serde_json::json!({ "msgtype": "m.text", "body": "hi" }))).is_none());
        assert!(media_info(&message(serde_json::json!({
            "msgtype": "m.image", "body": "* new", "url": "mxc://x/new",
            "m.new_content": { "msgtype": "m.image", "body": "new", "url": "mxc://x/new" },
            "m.relates_to": { "rel_type": "m.replace", "event_id": "$old:x" },
        }))).is_none());
    }
}
