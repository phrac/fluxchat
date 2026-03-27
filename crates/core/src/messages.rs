use matrix_sdk::{
    attachment::AttachmentConfig,
    room::{edit::EditedContent, reply::Reply, MessagesOptions},
    ruma::events::{
        reaction::ReactionEventContent,
        relation::Annotation,
        room::{
            message::{MessageType, Relation, RoomMessageEventContent, RoomMessageEventContentWithoutRelation},
            pinned_events::RoomPinnedEventsEventContent,
            member::MembershipState,
        },
        AnySyncTimelineEvent, AnySyncStateEvent, AnySyncMessageLikeEvent, SyncStateEvent, SyncMessageLikeEvent,
    },
    RoomMemberships,
};
use std::collections::HashMap;

use crate::error::AppError;
use crate::models::*;
use crate::rooms::parse_room_id;

/// Default number of messages to load per page.
const MESSAGE_PAGE_SIZE: u32 = 20;
/// Maximum recent events to check for read receipts.
const RECEIPT_EVENT_LIMIT: usize = 30;

/// Extract body, formatted HTML, and image MXC from a message type.
pub(crate) fn extract_message_content(msgtype: &MessageType) -> (String, Option<String>, Option<String>) {
    match msgtype {
        MessageType::Text(t) => (t.body.clone(), t.formatted.as_ref().map(|f| f.body.clone()), None),
        MessageType::Image(img) => {
            let mxc = match &img.source {
                matrix_sdk::ruma::events::room::MediaSource::Plain(uri) => Some(uri.to_string()),
                matrix_sdk::ruma::events::room::MediaSource::Encrypted(enc) => Some(enc.url.to_string()),
            };
            (img.body.clone(), None, mxc)
        }
        MessageType::Notice(t) => (t.body.clone(), t.formatted.as_ref().map(|f| f.body.clone()), None),
        MessageType::Emote(t) => (t.body.clone(), t.formatted.as_ref().map(|f| format!("* {}", f.body)), None),
        MessageType::Video(vid) => {
            let mxc = match &vid.source {
                matrix_sdk::ruma::events::room::MediaSource::Plain(uri) => Some(uri.to_string()),
                matrix_sdk::ruma::events::room::MediaSource::Encrypted(enc) => Some(enc.url.to_string()),
            };
            (vid.body.clone(), None, mxc)
        }
        MessageType::Audio(a) => (a.body.clone(), None, None),
        MessageType::File(f) => (f.body.clone(), None, None),
        _ => (format!("[{}]", msgtype.msgtype()), None, None),
    }
}

fn extract_reply_info(relates_to: &Option<Relation<RoomMessageEventContentWithoutRelation>>) -> Option<String> {
    match relates_to {
        Some(Relation::Reply { in_reply_to }) => Some(in_reply_to.event_id.to_string()),
        Some(Relation::Thread(thread)) => thread.in_reply_to.as_ref().map(|r| r.event_id.to_string()),
        _ => None,
    }
}

/// Fetch a page of messages from a room using the REST messages API.
pub async fn get_room_messages(client: &matrix_sdk::Client, room_id: &str, from_token: Option<&str>) -> Result<PaginatedMessages, AppError> {
    let room_id = parse_room_id(room_id)?;
    let room = client.get_room(&room_id).ok_or(AppError::RoomNotFound(room_id.to_string()))?;

    let mut options = MessagesOptions::backward();
    options.limit = MESSAGE_PAGE_SIZE.into();
    if let Some(token) = from_token { options.from = Some(token.to_string()); }

    let response = room.messages(options).await?;

    // Collect reactions
    let mut reaction_map: HashMap<String, Vec<(String, String)>> = HashMap::new();
    for event in &response.chunk {
        if let Ok(AnySyncTimelineEvent::MessageLike(AnySyncMessageLikeEvent::Reaction(SyncMessageLikeEvent::Original(reaction)))) = event.raw().deserialize() {
            reaction_map.entry(reaction.content.relates_to.event_id.to_string())
                .or_default().push((reaction.content.relates_to.key.clone(), reaction.sender.to_string()));
        }
    }

    let mut member_cache: HashMap<String, (String, Option<String>)> = HashMap::new();
    let mut messages = Vec::new();

    for event in &response.chunk {
        let deserialized = match event.raw().deserialize() { Ok(ev) => ev, Err(_) => continue };

        // State events (membership)
        if let AnySyncTimelineEvent::State(AnySyncStateEvent::RoomMember(SyncStateEvent::Original(member_ev))) = &deserialized {
            let action = match member_ev.content.membership {
                MembershipState::Join => if member_ev.prev_content().map(|p| &p.membership) == Some(&MembershipState::Invite) { "accepted the invitation" } else { "joined the room" },
                MembershipState::Leave => if member_ev.state_key.as_str() == member_ev.sender.as_str() { "left the room" } else { "was removed" },
                MembershipState::Invite => "was invited",
                MembershipState::Ban => "was banned",
                _ => "",
            };
            if !action.is_empty() {
                let name = member_ev.content.displayname.as_deref().unwrap_or(member_ev.state_key.as_str());
                let body = if member_ev.state_key.as_str() != member_ev.sender.as_str() && !action.starts_with("accepted") && !action.starts_with("joined") {
                    format!("{} {} {}", member_ev.sender, action.trim_start_matches("was "), name)
                } else { format!("{} {}", name, action) };
                messages.push(MessageInfo {
                    event_id: format!("~state-{}", member_ev.event_id), sender: String::new(), sender_name: String::new(),
                    body, formatted_body: None, transformed_body: None, image_mxc: None, reactions: Vec::new(),
                    reply_to_id: None, reply_to_sender: None, reply_to_body: None,
                    timestamp: member_ev.origin_server_ts.as_secs().into(), avatar_url: None, thread_root: None, thread_count: None, image_width: None, image_height: None, poll: None, is_emote: false, is_video: false,
                });
            }
        }

        // Messages
        if let AnySyncTimelineEvent::MessageLike(AnySyncMessageLikeEvent::RoomMessage(SyncMessageLikeEvent::Original(msg))) = deserialized {
            let (body, formatted_body, image_mxc) = extract_message_content(&msg.content.msgtype);

            let sender_key = msg.sender.to_string();
            let (sender_name, avatar_url) = if let Some(cached) = member_cache.get(&sender_key) {
                cached.clone()
            } else {
                let member = room.get_member_no_sync(&msg.sender).await.ok().flatten();
                let name = member.as_ref().and_then(|m| m.display_name().map(|s| s.to_string())).unwrap_or_else(|| msg.sender.to_string());
                let avatar = member.as_ref().and_then(|m| m.avatar_url()).map(|mxc| mxc.to_string());
                member_cache.insert(sender_key, (name.clone(), avatar.clone()));
                (name, avatar)
            };

            let reply_to_id = extract_reply_info(&msg.content.relates_to);
            let event_id_str = msg.event_id.to_string();
            let reactions = if let Some(raw) = reaction_map.get(&event_id_str) {
                let mut by_emoji: HashMap<String, (u32, Vec<String>)> = HashMap::new();
                for (emoji, sender) in raw { let e = by_emoji.entry(emoji.clone()).or_insert((0, Vec::new())); e.0 += 1; e.1.push(sender.clone()); }
                by_emoji.into_iter().map(|(emoji, (count, users))| ReactionInfo { emoji, count, users }).collect()
            } else { Vec::new() };

            messages.push(MessageInfo {
                event_id: event_id_str, sender: msg.sender.to_string(), sender_name, body, formatted_body,
                transformed_body: None, // Will be populated by UI if needed
                image_mxc,
                reactions, reply_to_id, reply_to_sender: None, reply_to_body: None,
                timestamp: msg.origin_server_ts.as_secs().into(), avatar_url, thread_root: None, thread_count: None, image_width: None, image_height: None, poll: None, is_emote: false, is_video: false,
            });
        }
    }
    Ok(PaginatedMessages { messages, end_token: response.end })
}

/// Send a text message to a room.
pub async fn send_message(client: &matrix_sdk::Client, room_id: &str, body: &str) -> Result<String, AppError> {
    let room_id = parse_room_id(room_id)?;
    let room = client.get_room(&room_id).ok_or(AppError::RoomNotFound(room_id.to_string()))?;
    let content = RoomMessageEventContent::text_plain(body);
    Ok(room.send(content).await?.event_id.to_string())
}

/// Reply to a specific message in a room.
pub async fn send_reply(client: &matrix_sdk::Client, room_id: &str, body: &str, reply_to_event_id: &str) -> Result<String, AppError> {
    let room_id = parse_room_id(room_id)?;
    let room = client.get_room(&room_id).ok_or(AppError::RoomNotFound(room_id.to_string()))?;
    let event_id = matrix_sdk::ruma::EventId::parse(reply_to_event_id).map_err(|_| AppError::Other("Invalid event ID".into()))?;
    let content = RoomMessageEventContentWithoutRelation::text_plain(body);
    let reply = Reply { event_id, enforce_thread: matrix_sdk::room::reply::EnforceThread::Unthreaded };
    let reply_content = room.make_reply_event(content, reply).await.map_err(|e| AppError::Other(format!("Failed to create reply: {e}")))?;
    Ok(room.send(reply_content).await?.event_id.to_string())
}

/// Edit an existing message in a room.
pub async fn edit_message(client: &matrix_sdk::Client, room_id: &str, event_id: &str, new_body: &str) -> Result<(), AppError> {
    let room_id = parse_room_id(room_id)?;
    let room = client.get_room(&room_id).ok_or(AppError::RoomNotFound(room_id.to_string()))?;
    let event_id = matrix_sdk::ruma::EventId::parse(event_id).map_err(|_| AppError::Other("Invalid event ID".into()))?;
    let content = RoomMessageEventContentWithoutRelation::text_plain(new_body);
    let edit = room.make_edit_event(&event_id, EditedContent::RoomMessage(content)).await.map_err(|e| AppError::Other(format!("Failed to edit: {e}")))?;
    room.send(edit).await?;
    Ok(())
}

/// Send or cancel a typing indicator in a room.
pub async fn send_typing(client: &matrix_sdk::Client, room_id: &str, typing: bool) -> Result<(), AppError> {
    let room_id = parse_room_id(room_id)?;
    let room = client.get_room(&room_id).ok_or(AppError::RoomNotFound(room_id.to_string()))?;
    room.typing_notice(typing).await?;
    Ok(())
}

/// Toggle a reaction emoji on a message.
pub async fn send_reaction(client: &matrix_sdk::Client, room_id: &str, event_id: &str, emoji: &str) -> Result<(), AppError> {
    let room_id = parse_room_id(room_id)?;
    let room = client.get_room(&room_id).ok_or(AppError::RoomNotFound(room_id.to_string()))?;
    let event_id = matrix_sdk::ruma::EventId::parse(event_id).map_err(|_| AppError::Other("Invalid event ID".into()))?;
    room.send(ReactionEventContent::from(Annotation::new(event_id, emoji.to_string()))).await?;
    Ok(())
}

/// Delete (redact) a message from a room.
pub async fn delete_message(client: &matrix_sdk::Client, room_id: &str, event_id: &str) -> Result<(), AppError> {
    let room_id = parse_room_id(room_id)?;
    let room = client.get_room(&room_id).ok_or(AppError::RoomNotFound(room_id.to_string()))?;
    let event_id = matrix_sdk::ruma::EventId::parse(event_id).map_err(|_| AppError::Other("Invalid event ID".into()))?;
    room.redact(&event_id, None, None).await.map_err(|e| AppError::Other(format!("Failed to delete: {e}")))?;
    Ok(())
}

/// Pin or unpin a message in a room.
pub async fn pin_message(client: &matrix_sdk::Client, room_id: &str, event_id: &str) -> Result<(), AppError> {
    let room_id = parse_room_id(room_id)?;
    let room = client.get_room(&room_id).ok_or(AppError::RoomNotFound(room_id.to_string()))?;
    let event_id = matrix_sdk::ruma::EventId::parse(event_id).map_err(|_| AppError::Other("Invalid event ID".into()))?;

    let mut pinned: Vec<matrix_sdk::ruma::OwnedEventId> = room.get_state_events_static::<RoomPinnedEventsEventContent>().await.ok()
        .and_then(|events| events.into_iter().next().and_then(|e| e.deserialize().ok().and_then(|ev| ev.as_sync().map(|s| match s {
            matrix_sdk::ruma::events::SyncStateEvent::Original(o) => o.content.pinned.clone(), _ => Vec::new(),
        })))).unwrap_or_default();

    if pinned.contains(&event_id) { pinned.retain(|id| id != &event_id); } else { pinned.push(event_id); }
    room.send_state_event(RoomPinnedEventsEventContent::new(pinned)).await.map_err(|e| AppError::Other(format!("Failed to pin: {e}")))?;
    Ok(())
}

/// Fetch pinned messages for a room.
pub async fn get_pinned_messages(client: &matrix_sdk::Client, room_id: &str) -> Result<Vec<MessageInfo>, AppError> {
    let room_id = parse_room_id(room_id)?;
    let room = client.get_room(&room_id).ok_or(AppError::RoomNotFound(room_id.to_string()))?;

    let pinned_ids: Vec<matrix_sdk::ruma::OwnedEventId> = room.get_state_events_static::<RoomPinnedEventsEventContent>().await.ok()
        .and_then(|events| events.into_iter().next().and_then(|e| e.deserialize().ok().and_then(|ev| ev.as_sync().map(|s| match s {
            matrix_sdk::ruma::events::SyncStateEvent::Original(o) => o.content.pinned.clone(), _ => Vec::new(),
        })))).unwrap_or_default();

    let mut messages = Vec::new();
    for eid in &pinned_ids {
        let event = match room.event(eid, None).await { Ok(e) => e, Err(_) => continue };
        if let Ok(AnySyncTimelineEvent::MessageLike(AnySyncMessageLikeEvent::RoomMessage(SyncMessageLikeEvent::Original(msg)))) = event.raw().deserialize() {
            let (body, formatted_body, image_mxc) = extract_message_content(&msg.content.msgtype);
            let member = room.get_member_no_sync(&msg.sender).await.ok().flatten();
            messages.push(MessageInfo {
                event_id: msg.event_id.to_string(), sender: msg.sender.to_string(),
                sender_name: member.as_ref().and_then(|m| m.display_name().map(|s| s.to_string())).unwrap_or_else(|| msg.sender.to_string()),
                body, formatted_body, transformed_body: None, image_mxc, reactions: Vec::new(),
                reply_to_id: None, reply_to_sender: None, reply_to_body: None,
                timestamp: msg.origin_server_ts.as_secs().into(),
                avatar_url: member.as_ref().and_then(|m| m.avatar_url()).map(|mxc| mxc.to_string()),
                thread_root: None, thread_count: None, image_width: None, image_height: None, poll: None, is_emote: false, is_video: false,
            });
        }
    }
    Ok(messages)
}

/// Fetch the member list for a room with presence and power level info.
pub async fn get_room_members(client: &matrix_sdk::Client, room_id: &str) -> Result<Vec<MemberInfo>, AppError> {
    let room_id = parse_room_id(room_id)?;
    let room = client.get_room(&room_id).ok_or(AppError::RoomNotFound(room_id.to_string()))?;
    let members = room.members(RoomMemberships::JOIN).await?;
    let store = client.state_store();

    let mut result = Vec::new();
    for m in &members {
        let power_level = match m.power_level() {
            matrix_sdk::ruma::events::room::power_levels::UserPowerLevel::Infinite => 100,
            matrix_sdk::ruma::events::room::power_levels::UserPowerLevel::Int(n) => i64::from(n),
            _ => 0,
        };
        let role = if power_level >= 100 { "Admin" } else if power_level >= 50 { "Moderator" } else { "Member" }.to_string();
        let (presence, status_msg) = match store.get_presence_event(m.user_id()).await {
            Ok(Some(raw)) => {
                let (p, s) = raw.deserialize().ok().map(|ev| {
                    let p = match ev.content.presence {
                        matrix_sdk::ruma::presence::PresenceState::Online => "online",
                        matrix_sdk::ruma::presence::PresenceState::Unavailable => "idle",
                        _ => "offline",
                    }.to_string();
                    let s = ev.content.status_msg.clone();
                    (p, s)
                }).unwrap_or(("offline".to_string(), None));
                (p, s)
            }
            _ => ("offline".to_string(), None),
        };
        result.push(MemberInfo {
            user_id: m.user_id().to_string(),
            display_name: m.display_name().map(|s| s.to_string()).unwrap_or_else(|| m.user_id().to_string()),
            avatar_url: m.avatar_url().map(|mxc| mxc.to_string()),
            power_level, role, presence, status_msg,
            server_name: m.user_id().server_name().to_string(),
        });
    }
    result.sort_by(|a, b| b.power_level.cmp(&a.power_level).then_with(|| a.display_name.to_lowercase().cmp(&b.display_name.to_lowercase())));
    Ok(result)
}

/// Upload a file attachment to a room, generating a thumbnail for images.
pub async fn upload_file(client: &matrix_sdk::Client, room_id: &str, filename: &str, mime_type: &str, data: Vec<u8>) -> Result<String, AppError> {
    let room_id = parse_room_id(room_id)?;
    let room = client.get_room(&room_id).ok_or(AppError::RoomNotFound(room_id.to_string()))?;
    let content_type: mime::Mime = mime_type.parse().unwrap_or(mime::APPLICATION_OCTET_STREAM);

    // Generate thumbnail for images
    let config = if mime_type.starts_with("image/") {
        if let Some((thumb_data, thumb_w, thumb_h)) = crate::media::generate_thumbnail(&data, 320, 240) {
            use matrix_sdk::attachment::Thumbnail;
            use matrix_sdk::ruma::UInt;
            AttachmentConfig::new().thumbnail(Some(Thumbnail {
                data: thumb_data.clone(),
                content_type: mime::IMAGE_PNG,
                height: UInt::from(thumb_h),
                width: UInt::from(thumb_w),
                size: UInt::try_from(thumb_data.len()).unwrap_or(UInt::from(0u32)),
            }))
        } else {
            AttachmentConfig::new()
        }
    } else {
        AttachmentConfig::new()
    };

    let response = room.send_attachment(filename, &content_type, data, config).await.map_err(|e| AppError::Other(format!("Failed to upload: {e}")))?;
    Ok(response.event_id.to_string())
}

/// Send a read receipt and fully-read marker for a message.
pub async fn send_read_receipt(client: &matrix_sdk::Client, room_id: &str, event_id: &str) -> Result<(), AppError> {
    let room_id = parse_room_id(room_id)?;
    let room = client.get_room(&room_id).ok_or(AppError::RoomNotFound(room_id.to_string()))?;
    let event_id = matrix_sdk::ruma::EventId::parse(event_id).map_err(|_| AppError::Other("Invalid event ID".into()))?;
    let receipts = matrix_sdk::room::Receipts::new().fully_read_marker(Some(event_id.clone())).public_read_receipt(Some(event_id));
    room.send_multiple_receipts(receipts).await.map_err(|e| AppError::Other(format!("Failed to send receipt: {e}")))?;
    Ok(())
}

/// Fetch read receipts for a room by checking recent event IDs.
pub async fn get_room_read_receipts(
    client: &matrix_sdk::Client,
    room_id: &str,
    recent_event_ids: &[String],
) -> Result<Vec<crate::models::ReadReceipt>, AppError> {
    use matrix_sdk::ruma::events::receipt::{ReceiptType, ReceiptThread};

    let room_id = parse_room_id(room_id)?;
    let room = client.get_room(&room_id).ok_or(AppError::RoomNotFound(room_id.to_string()))?;

    let mut receipts = Vec::new();
    let mut seen_users = std::collections::HashSet::new();

    for eid_str in recent_event_ids.iter().rev().take(RECEIPT_EVENT_LIMIT) {
        let event_id = match matrix_sdk::ruma::EventId::parse(eid_str) {
            Ok(eid) => eid,
            Err(_) => continue,
        };
        // Try both Main and Unthreaded thread types
        for thread in [ReceiptThread::Main, ReceiptThread::Unthreaded] {
            if let Ok(event_receipts) = room.load_event_receipts(ReceiptType::Read, thread, &event_id).await {
                for (user_id, _receipt) in event_receipts {
                    let uid_str = user_id.to_string();
                    if seen_users.contains(&uid_str) { continue; }
                    seen_users.insert(uid_str.clone());

                    let (display_name, avatar_url) = if let Some(member) = room.get_member_no_sync(&user_id).await.ok().flatten() {
                        let name = member.display_name().unwrap_or(user_id.localpart()).to_string();
                        let avatar = member.avatar_url().map(|u| u.to_string());
                        (name, avatar)
                    } else {
                        (user_id.localpart().to_string(), None)
                    };
                    receipts.push(crate::models::ReadReceipt {
                        user_id: uid_str,
                        event_id: eid_str.clone(),
                        display_name,
                        avatar_url,
                    });
                }
            }
        }
    }
    Ok(receipts)
}

/// Forward a message to another room by sending its content there.
pub async fn forward_message(client: &matrix_sdk::Client, target_room_id: &str, body: &str, formatted_body: Option<&str>) -> Result<(), AppError> {
    let target_id = matrix_sdk::ruma::RoomId::parse(target_room_id)
        .map_err(|_| AppError::Other("Invalid target room ID".into()))?;
    let room = client.get_room(&target_id)
        .ok_or(AppError::RoomNotFound(target_id.to_string()))?;

    let content = if let Some(html) = formatted_body {
        RoomMessageEventContent::text_html(body, html)
    } else {
        RoomMessageEventContent::text_plain(body)
    };
    room.send(content).await.map_err(|e| AppError::Other(format!("Failed to forward: {e}")))?;
    Ok(())
}

/// Server-side message search result.
#[derive(Clone, Debug)]
pub struct SearchResult {
    pub event_id: String,
    pub sender: String,
    pub sender_name: String,
    pub body: String,
    pub timestamp: u64,
    pub room_id: String,
}

/// Search messages server-side via the Matrix search API.
pub async fn search_messages(
    client: &matrix_sdk::Client,
    room_id: &str,
    query: &str,
    next_batch: Option<&str>,
) -> Result<(Vec<SearchResult>, Option<String>), AppError> {
    use matrix_sdk::ruma::api::client::search::search_events;

    let rid = matrix_sdk::ruma::RoomId::parse(room_id)
        .map_err(|_| AppError::Other("Invalid room ID".into()))?;

    let mut criteria = search_events::v3::Criteria::new(query.to_string());

    // Filter to this room only
    let mut filter = matrix_sdk::ruma::api::client::filter::RoomEventFilter::default();
    filter.rooms = Some(vec![rid.to_owned()]);
    criteria.filter = filter;
    criteria.order_by = Some(search_events::v3::OrderBy::Recent);

    let mut categories = search_events::v3::Categories::new();
    categories.room_events = Some(criteria);

    let mut request = search_events::v3::Request::new(categories);
    request.next_batch = next_batch.map(|s| s.to_string());

    let response = client.send(request).await
        .map_err(|e| AppError::Other(format!("Search failed: {e}")))?;

    let mut results = Vec::new();
    let room_events = &response.search_categories.room_events;
    {
        for item in &room_events.results {
            if let Some(ref raw_event) = item.result {
                // Parse from raw JSON to extract fields
                let json = raw_event.json();
                let val: serde_json::Value = match serde_json::from_str(json.get()) {
                    Ok(v) => v,
                    Err(_) => continue,
                };
                let event_id = val["event_id"].as_str().unwrap_or_default().to_string();
                let sender = val["sender"].as_str().unwrap_or_default().to_string();
                let sender_name = sender.split(':').next().unwrap_or(&sender)
                    .trim_start_matches('@').to_string();
                let body = val["content"]["body"].as_str().unwrap_or_default().to_string();
                let timestamp = val["origin_server_ts"].as_u64().unwrap_or(0) / 1000;
                if !event_id.is_empty() && !body.is_empty() {
                    results.push(SearchResult {
                        event_id,
                        sender,
                        sender_name,
                        body,
                        timestamp,
                        room_id: room_id.to_string(),
                    });
                }
            }
        }
    }

    Ok((results, room_events.next_batch.clone()))
}
