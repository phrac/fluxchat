//! A thread's text from the server (decrypted where we have the keys), for
//! keeping agent run transcripts searchable when the thread was never
//! opened — runs in background rooms finish without their thread loaded.

use matrix_sdk::{
    room::{IncludeRelations, RelationsOptions},
    ruma::{
        api::Direction,
        events::{
            relation::RelationType,
            room::message::{MessageType, Relation},
            AnySyncMessageLikeEvent, AnySyncTimelineEvent, SyncMessageLikeEvent,
        },
        EventId, UInt,
    },
};

use crate::error::AppError;
use crate::gallery::to_items;
use crate::models::MessageInfo;
use crate::rooms::parse_room_id;

const PAGE_SIZE: u32 = 100;
/// Pages read at most (agent threads are rarely longer).
const MAX_PAGES: usize = 5;

/// The thread's root and replies with text (body, sender, sender name,
/// timestamp), oldest first. Edits aren't applied: originals only.
pub async fn thread_messages(
    client: &matrix_sdk::Client,
    room_id: &str,
    root: &str,
) -> Result<Vec<MessageInfo>, AppError> {
    let rid = parse_room_id(room_id)?;
    let room = client.get_room(&rid).ok_or(AppError::RoomNotFound(rid.to_string()))?;
    let root_id = EventId::parse(root).map_err(|_| AppError::Other("Invalid event ID".into()))?;

    let mut events = Vec::new();
    if let Ok(ev) = room.event(&root_id, None).await {
        events.push(ev);
    }
    let mut from = None;
    for _ in 0..MAX_PAGES {
        let options = RelationsOptions {
            from: from.take(),
            dir: Direction::Backward,
            limit: Some(UInt::from(PAGE_SIZE)),
            include_relations: IncludeRelations::RelationsOfType(RelationType::Thread),
            recurse: false,
        };
        let response = room.relations(root_id.clone(), options).await
            .map_err(|e| AppError::Other(format!("Couldn't load the thread: {e}")))?;
        let done = response.chunk.is_empty() || response.next_batch_token.is_none();
        events.extend(response.chunk);
        if done {
            break;
        }
        from = response.next_batch_token;
    }
    let mut items = to_items(&room, &events, text_info).await;
    items.sort_by_key(|m| m.timestamp);
    items.dedup_by(|a, b| a.event_id == b.event_id);
    Ok(items)
}

/// A message's text (`None` for edits, state, redacted or undecryptable
/// events). Media count by their file name or caption.
fn text_info(event: &AnySyncTimelineEvent) -> Option<MessageInfo> {
    let AnySyncTimelineEvent::MessageLike(AnySyncMessageLikeEvent::RoomMessage(SyncMessageLikeEvent::Original(msg))) = event else {
        return None;
    };
    if matches!(msg.content.relates_to, Some(Relation::Replacement(_))) {
        return None;
    }
    let body = match &msg.content.msgtype {
        MessageType::Text(t) => t.body.clone(),
        MessageType::Notice(n) => n.body.clone(),
        MessageType::Emote(e) => format!("* {}", e.body),
        other => other.body().to_string(),
    };
    Some(MessageInfo {
        event_id: event.event_id().to_string(),
        sender: event.sender().to_string(),
        timestamp: event.origin_server_ts().as_secs().into(),
        body,
        ..Default::default()
    })
}
