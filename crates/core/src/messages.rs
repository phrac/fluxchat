use matrix_sdk::{
    room::{edit::EditedContent, MessagesOptions},
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
        Some(Relation::Reply(reply)) => Some(reply.in_reply_to.event_id.to_string()),
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
                    timestamp: member_ev.origin_server_ts.as_secs().into(), avatar_url: None,
                    ..Default::default()
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
                timestamp: msg.origin_server_ts.as_secs().into(), avatar_url,
                ..Default::default()
            });
        }
    }
    Ok(PaginatedMessages { messages, end_token: response.end })
}

/// Send a text message to a room.
pub async fn send_message(client: &matrix_sdk::Client, room_id: &str, body: &str) -> Result<String, AppError> {
    let room_id_parsed = parse_room_id(room_id)?;
    let room = client.get_room(&room_id_parsed).ok_or(AppError::RoomNotFound(room_id.to_string()))?;

    // Handle /me emotes — strip prefix and send as emote message type
    if let Some(emote_body) = body.strip_prefix("/me ") {
        let content = build_emote_content(&room, emote_body).await;
        let result = room.send(content).await.map_err(|e| AppError::Other(format!("Failed to send: {e}")))?;
        return Ok(result.response.event_id.to_string());
    }

    let content = build_message_content(&room, body).await;
    let result = room.send(content).await.map_err(|e| AppError::Other(format!("Failed to send: {e}")))?;
    Ok(result.response.event_id.to_string())
}

/// Send a message as raw HTML (for /html command).
pub async fn send_html(client: &matrix_sdk::Client, room_id: &str, html: &str) -> Result<String, AppError> {
    let room_id_parsed = parse_room_id(room_id)?;
    let room = client.get_room(&room_id_parsed).ok_or(AppError::RoomNotFound(room_id.to_string()))?;
    // Strip HTML tags for the plain-text body fallback
    let plain = html.replace("<br>", "\n").replace("<br/>", "\n").replace("<br />", "\n");
    let mut plain_stripped = String::with_capacity(plain.len());
    let mut in_tag = false;
    for c in plain.chars() {
        if c == '<' { in_tag = true; }
        else if c == '>' { in_tag = false; }
        else if !in_tag { plain_stripped.push(c); }
    }
    let plain = plain_stripped;
    let content = RoomMessageEventContent::text_html(&plain, html);
    let result = room.send(content).await.map_err(|e| AppError::Other(format!("Failed to send: {e}")))?;
    Ok(result.response.event_id.to_string())
}

/// Send a plain text message without any markdown processing (for /plain command).
pub async fn send_plain(client: &matrix_sdk::Client, room_id: &str, text: &str) -> Result<String, AppError> {
    let room_id_parsed = parse_room_id(room_id)?;
    let room = client.get_room(&room_id_parsed).ok_or(AppError::RoomNotFound(room_id.to_string()))?;
    let content = RoomMessageEventContent::text_plain(text);
    let result = room.send(content).await.map_err(|e| AppError::Other(format!("Failed to send: {e}")))?;
    Ok(result.response.event_id.to_string())
}

/// Send a voice message as OGG Opus audio with MSC3245 voice marker.
pub async fn send_voice_message(client: &matrix_sdk::Client, room_id: &str, ogg_bytes: Vec<u8>, duration_ms: u64) -> Result<String, AppError> {
    use matrix_sdk::attachment::{AttachmentConfig, AttachmentInfo, BaseAudioInfo};

    let room_id_parsed = parse_room_id(room_id)?;
    let room = client.get_room(&room_id_parsed).ok_or(AppError::RoomNotFound(room_id.to_string()))?;

    let content_type: mime::Mime = "audio/ogg".parse().unwrap();
    let size = ogg_bytes.len() as u64;

    let config = AttachmentConfig::new()
        .info(AttachmentInfo::Voice(BaseAudioInfo {
            duration: Some(std::time::Duration::from_millis(duration_ms)),
            size: size.try_into().ok(),
            waveform: None,
        }));

    let result = room.send_attachment("Voice message", &content_type, ogg_bytes, config)
        .await
        .map_err(|e| AppError::Other(format!("Failed to send voice: {e}")))?;
    Ok(result.event_id.to_string())
}

/// Report a message to the homeserver.
pub async fn report_message(client: &matrix_sdk::Client, room_id: &str, event_id: &str, reason: &str) -> Result<(), AppError> {
    use matrix_sdk::ruma::api::client::room::report_content;
    let rid = parse_room_id(room_id)?;
    let eid = matrix_sdk::ruma::EventId::parse(event_id).map_err(|_| AppError::Other("Invalid event ID".into()))?;
    let mut request = report_content::v3::Request::new(rid, eid);
    request.reason = Some(reason.to_string());
    client.send(request).await.map_err(|e| AppError::Other(format!("Report failed: {e}")))?;
    Ok(())
}

/// Send a location message (MSC3488).
pub async fn send_location(client: &matrix_sdk::Client, room_id: &str, geo_uri: &str) -> Result<String, AppError> {
    use matrix_sdk::ruma::events::room::message::{LocationMessageEventContent, RoomMessageEventContent};
    let room_id_parsed = parse_room_id(room_id)?;
    let room = client.get_room(&room_id_parsed).ok_or(AppError::RoomNotFound(room_id.to_string()))?;
    let body = format!("Location: {geo_uri}");
    let content = RoomMessageEventContent::new(
        matrix_sdk::ruma::events::room::message::MessageType::Location(
            LocationMessageEventContent::new(body, geo_uri.to_string()),
        ),
    );
    let result = room.send(content).await.map_err(|e| AppError::Other(format!("Failed to send location: {e}")))?;
    Ok(result.response.event_id.to_string())
}

/// Build message content with mention resolution, markdown formatting,
/// and MSC3952 intentional mentions (m.mentions).
/// Build emote content (for /me commands). Uses m.emote msgtype.
pub async fn build_emote_content(_room: &matrix_sdk::Room, body: &str) -> RoomMessageEventContent {
    let has_markdown = body.contains("**") || body.contains('`') || body.contains("~~");

    

    if has_markdown {
        let escaped = html_escape(body);
        let html = simple_markdown(&escaped);
        if html != escaped {
            let mut final_html = html;
            if final_html.contains('\n') {
                final_html = final_html.replace('\n', "<br>");
            }
            RoomMessageEventContent::emote_html(body, &final_html)
        } else {
            RoomMessageEventContent::emote_plain(body)
        }
    } else {
        RoomMessageEventContent::emote_plain(body)
    }
}

pub async fn build_message_content(room: &matrix_sdk::Room, body: &str) -> RoomMessageEventContent {
    // Only attempt mention resolution if body contains @
    let has_mentions = body.contains('@');
    let has_headings = body.lines().any(parse_heading_line_is_heading);
    let has_markdown = body.contains("**") || body.contains('`') || body.contains("~~") || has_headings;

    if !has_mentions && !has_markdown {
        return RoomMessageEventContent::text_plain(body);
    }

    // Everything below builds HTML, so the raw text must be escaped first —
    // otherwise `Vec<u8>` or `a < b` corrupt the formatted body.
    let mut html = html_escape(body);
    let mut changed = false;
    let mut mentioned_user_ids: std::collections::BTreeSet<matrix_sdk::ruma::OwnedUserId> =
        std::collections::BTreeSet::new();
    let mut mentions_room = false;

    // Resolve @mentions
    if has_mentions {
        // Detect @room mention
        if body.contains("@room") {
            mentions_room = true;
        }

        // Use tokio timeout to prevent hanging
        let members_result = tokio::time::timeout(
            std::time::Duration::from_secs(3),
            room.members(matrix_sdk::RoomMemberships::JOIN),
        ).await;

        if let Ok(Ok(members)) = members_result {
            let mut member_names: Vec<(String, String)> = members.iter()
                .map(|m| {
                    let name = m.display_name().unwrap_or(m.user_id().localpart()).to_string();
                    let user_id = m.user_id().to_string();
                    (name, user_id)
                })
                .collect();
            member_names.sort_by_key(|(name, _)| std::cmp::Reverse(name.len()));

            for (display_name, user_id) in &member_names {
                let display_name = html_escape(display_name);
                let mention = format!("@{display_name}");
                if html.contains(&mention) {
                    let encoded_uid = user_id.replace('@', "%40").replace(':', "%3A");
                    let link = format!(
                        "<a href=\"https://matrix.to/#/{encoded_uid}\">{display_name}</a>"
                    );
                    html = html.replace(&mention, &link);
                    changed = true;
                    // Track for m.mentions (MSC3952)
                    if let Ok(uid) = matrix_sdk::ruma::UserId::parse(user_id) {
                        mentioned_user_ids.insert(uid);
                    }
                }
            }
        }
    }

    // Markdown
    if has_markdown {
        let before = html.clone();
        html = simple_markdown(&html);
        if html != before {
            changed = true;
        }
    }

    let mut content = if changed {
        let mut final_html = html;
        if final_html.contains('\n') {
            final_html = final_html.replace('\n', "<br>");
        }
        RoomMessageEventContent::text_html(body, &final_html)
    } else {
        RoomMessageEventContent::text_plain(body)
    };

    // Set m.mentions (MSC3952 intentional mentions)
    if !mentioned_user_ids.is_empty() || mentions_room {
        let mut mentions = matrix_sdk::ruma::events::Mentions::with_user_ids(mentioned_user_ids);
        mentions.room = mentions_room;
        content.mentions = Some(mentions);
    }

    content
}

/// Returns true if a line is an ATX-style heading (`#` ... `######` followed by space).
fn parse_heading_line_is_heading(line: &str) -> bool {
    let hashes = line.chars().take_while(|c| *c == '#').count();
    (1..=6).contains(&hashes) && line[hashes..].starts_with(' ')
}

/// Wrap heading lines (`# ` ... `###### `) in `<hN>` tags. Skips lines inside `<pre>` blocks.
fn apply_headings(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    let mut in_pre = false;
    let mut first = true;
    for line in text.split('\n') {
        if !first {
            out.push('\n');
        }
        first = false;
        if !in_pre && parse_heading_line_is_heading(line) {
            let level = line.chars().take_while(|c| *c == '#').count();
            let rest = &line[level + 1..];
            out.push_str(&format!("<h{level}>{rest}</h{level}>"));
        } else {
            out.push_str(line);
        }
        if line.contains("<pre") {
            in_pre = true;
        }
        if line.contains("</pre>") {
            in_pre = false;
        }
    }
    out
}

fn simple_markdown(text: &str) -> String {
    let mut result = text.to_string();
    // Code blocks (``` ... ```)
    while let Some(start) = result.find("```") {
        if let Some(end) = result[start + 3..].find("```") {
            let code = &result[start + 3..start + 3 + end];
            let replacement = format!("<pre><code>{code}</code></pre>");
            result = format!("{}{}{}", &result[..start], replacement, &result[start + 6 + end..]);
        } else {
            break;
        }
    }
    // Inline code (` ... `)
    while let Some(start) = result.find('`') {
        if let Some(end) = result[start + 1..].find('`') {
            let code = &result[start + 1..start + 1 + end];
            let replacement = format!("<code>{code}</code>");
            result = format!("{}{}{}", &result[..start], replacement, &result[start + 2 + end..]);
        } else {
            break;
        }
    }
    // Bold (**...**)
    while let Some(start) = result.find("**") {
        if let Some(end) = result[start + 2..].find("**") {
            let bold = &result[start + 2..start + 2 + end];
            let replacement = format!("<strong>{bold}</strong>");
            result = format!("{}{}{}", &result[..start], replacement, &result[start + 4 + end..]);
        } else {
            break;
        }
    }
    // Strikethrough (~~...~~)
    while let Some(start) = result.find("~~") {
        if let Some(end) = result[start + 2..].find("~~") {
            let strike = &result[start + 2..start + 2 + end];
            let replacement = format!("<del>{strike}</del>");
            result = format!("{}{}{}", &result[..start], replacement, &result[start + 4 + end..]);
        } else {
            break;
        }
    }
    // Italic (*...*) — after bold so `**` pairs are already consumed.
    result = apply_italics(&result);
    // Headings (`# ` … `###### ` at line start) — applied last so we can skip <pre> regions
    result = apply_headings(&result);
    result
}

/// Escape `&`, `<`, `>` and `"` for safe inclusion in a formatted body.
pub fn html_escape(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    for c in text.chars() {
        match c {
            '&' => out.push_str("&amp;"),
            '<' => out.push_str("&lt;"),
            '>' => out.push_str("&gt;"),
            '"' => out.push_str("&quot;"),
            _ => out.push(c),
        }
    }
    out
}

/// Wrap `*text*` in `<em>`. Requires non-space characters directly inside
/// the markers and no newline between them, so `5 * 3 * 2` is left alone.
fn apply_italics(text: &str) -> String {
    let bytes = text.as_bytes();
    let mut out = String::with_capacity(text.len());
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'*' {
            let inner_ok = |b: u8| !b.is_ascii_whitespace() && b != b'*';
            if let Some(len) = text[i + 1..].find('*') {
                let inner = &text[i + 1..i + 1 + len];
                if len > 0
                    && inner_ok(inner.as_bytes()[0])
                    && inner_ok(inner.as_bytes()[len - 1])
                    && !inner.contains('\n')
                {
                    out.push_str("<em>");
                    out.push_str(inner);
                    out.push_str("</em>");
                    i += len + 2;
                    continue;
                }
            }
        }
        let ch = text[i..].chars().next().unwrap();
        out.push(ch);
        i += ch.len_utf8();
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn code_spans_keep_angle_brackets_intact() {
        let html = simple_markdown(&html_escape("use `Vec<u8>` here"));
        assert_eq!(html, "use <code>Vec&lt;u8&gt;</code> here");
    }

    #[test]
    fn bold_strike_italic_and_headings() {
        assert_eq!(simple_markdown("**b** ~~s~~ *i*"), "<strong>b</strong> <del>s</del> <em>i</em>");
        assert_eq!(simple_markdown("# Title\nbody"), "<h1>Title</h1>\nbody");
    }

    #[test]
    fn italics_ignore_arithmetic_and_unclosed() {
        assert_eq!(apply_italics("5 * 3 * 2"), "5 * 3 * 2");
        assert_eq!(apply_italics("a *b"), "a *b");
        assert_eq!(apply_italics("**"), "**");
    }

    #[test]
    fn headings_skip_code_blocks() {
        let html = simple_markdown("```\n# not a heading\n```");
        assert_eq!(html, "<pre><code>\n# not a heading\n</code></pre>");
    }
}

/// Edit an existing message in a room.
pub async fn edit_message(client: &matrix_sdk::Client, room_id: &str, event_id: &str, new_body: &str) -> Result<(), AppError> {
    let room_id = parse_room_id(room_id)?;
    let room = client.get_room(&room_id).ok_or(AppError::RoomNotFound(room_id.to_string()))?;
    let event_id = matrix_sdk::ruma::EventId::parse(event_id).map_err(|_| AppError::Other("Invalid event ID".into()))?;
    // Same markdown / mention processing as a new message.
    let content: RoomMessageEventContentWithoutRelation = build_message_content(&room, new_body).await.into();
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
                ..Default::default()
            });
        }
    }
    Ok(messages)
}

/// Rooms with at most this many members get presence fetched per member
/// (sliding sync delivers no presence, so the store has none).
const PRESENCE_FETCH_LIMIT: usize = 50;
/// Concurrent presence requests when filling a member list.
const PRESENCE_FETCH_CONCURRENCY: usize = 8;
/// Per-request timeout for presence lookups.
const PRESENCE_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(5);

fn presence_label(state: &matrix_sdk::ruma::presence::PresenceState) -> &'static str {
    match state {
        matrix_sdk::ruma::presence::PresenceState::Online => "online",
        matrix_sdk::ruma::presence::PresenceState::Unavailable => "idle",
        _ => "offline",
    }
}

/// Ask the user's homeserver for their current presence. Fails if the
/// server has presence disabled or hides it from us.
pub async fn fetch_presence(client: &matrix_sdk::Client, user_id: &str) -> Result<crate::models::Presence, AppError> {
    use matrix_sdk::ruma::api::client::presence::get_presence;
    let uid = crate::rooms::parse_user_id(user_id)?;
    let resp = tokio::time::timeout(PRESENCE_TIMEOUT, client.send(get_presence::v3::Request::new(uid)))
        .await
        .map_err(|_| AppError::Other("Presence request timed out".into()))?
        .map_err(|e| AppError::Other(format!("Presence unavailable: {e}")))?;
    Ok(crate::models::Presence {
        state: presence_label(&resp.presence).to_string(),
        status_msg: resp.status_msg.filter(|s| !s.is_empty()),
        last_active_secs: resp.last_active_ago.map(|d| d.as_secs()),
        currently_active: resp.currently_active.unwrap_or(false),
    })
}

/// Recent messages that highlighted us (mentions / keywords), across all
/// rooms, newest first — the server's `/notifications?only=highlight`.
/// Encrypted ones are decrypted locally where we have the keys. (The server
/// can't evaluate mentions inside encrypted messages; the app adds those
/// from live notifications and loaded timelines.)
pub async fn fetch_highlights(client: &matrix_sdk::Client, limit: u32) -> Result<Vec<crate::models::MentionItem>, AppError> {
    use matrix_sdk::ruma::api::client::push::get_notifications;
    use matrix_sdk::ruma::events::room::encrypted::OriginalSyncRoomEncryptedEvent;
    let mut request = get_notifications::v3::Request::new();
    request.limit = Some(matrix_sdk::ruma::UInt::from(limit));
    request.only = Some("highlight".into());
    let response = client.send(request).await
        .map_err(|e| AppError::Other(format!("Couldn't load mentions: {e}")))?;

    let mut out = Vec::new();
    for n in response.notifications {
        let Some(room) = client.get_room(&n.room_id) else { continue };
        let Ok(mut json) = n.event.deserialize_as::<serde_json::Value>() else { continue };
        if json["type"] == "m.room.encrypted" {
            let raw = matrix_sdk::ruma::serde::Raw::<OriginalSyncRoomEncryptedEvent>::from_json(n.event.json().to_owned());
            match room.decrypt_event(&raw, None).await {
                Ok(ev) => match ev.raw().deserialize_as::<serde_json::Value>() {
                    Ok(j) => json = j,
                    Err(_) => continue,
                },
                Err(_) => continue, // no key: nothing to show
            }
        }
        if json["type"] != "m.room.message" {
            continue;
        }
        let body = json["content"]["body"].as_str().unwrap_or_default();
        if body.trim().is_empty() {
            continue;
        }
        let sender = json["sender"].as_str().unwrap_or_default().to_string();
        let sender_name = match matrix_sdk::ruma::UserId::parse(&sender) {
            Ok(uid) => room.get_member_no_sync(&uid).await.ok().flatten()
                .and_then(|m| m.display_name().map(str::to_string))
                .unwrap_or_else(|| uid.localpart().to_string()),
            Err(_) => sender.clone(),
        };
        out.push(crate::models::MentionItem {
            room_id: n.room_id.to_string(),
            event_id: json["event_id"].as_str().unwrap_or_default().to_string(),
            sender,
            sender_name,
            body: body.lines().next().unwrap_or("").chars().take(200).collect(),
            timestamp: json["origin_server_ts"].as_u64().map(|ms| ms / 1000)
                .unwrap_or_else(|| u64::from(n.ts.as_secs())),
        });
    }
    Ok(out)
}

/// Longest text kept for hover previews.
const PREVIEW_TEXT_LIMIT: usize = 600;

/// A room's threads, most recently active first (server `/threads`, with
/// each root's bundled thread summary).
pub async fn recent_threads(client: &matrix_sdk::Client, room_id: &str, limit: u32) -> Result<Vec<crate::models::ThreadPreview>, AppError> {
    use matrix_sdk::room::ListThreadsOptions;
    let rid = parse_room_id(room_id)?;
    let room = client.get_room(&rid).ok_or_else(|| AppError::RoomNotFound(room_id.into()))?;
    let opts = ListThreadsOptions {
        limit: Some(matrix_sdk::ruma::UInt::from(limit)),
        ..Default::default()
    };
    let roots = room.list_threads(opts).await.map_err(|e| AppError::Other(format!("Couldn't load threads: {e}")))?;
    let name_of = |uid: &str| {
        let room = room.clone();
        let uid = uid.to_string();
        async move {
            let Ok(user) = matrix_sdk::ruma::UserId::parse(&uid) else { return uid };
            room.get_member_no_sync(&user).await.ok().flatten()
                .and_then(|m| m.display_name().map(str::to_string))
                .unwrap_or_else(|| user.localpart().to_string())
        }
    };
    let mut out = Vec::new();
    for ev in roots.chunk {
        let Ok(json) = ev.raw().deserialize_as::<serde_json::Value>() else { continue };
        let Some(root_id) = json["event_id"].as_str() else { continue };
        let sender = json["sender"].as_str().unwrap_or_default().to_string();
        let full = json["content"]["body"].as_str().unwrap_or("").trim().to_string();
        let body = full.lines().next().map(|l| l.chars().take(120).collect::<String>())
            .filter(|b| !b.trim().is_empty())
            .unwrap_or_else(|| "(message)".into());
        let thread = &json["unsigned"]["m.relations"]["m.thread"];
        let latest = &thread["latest_event"];
        let ts = |v: &serde_json::Value| v["origin_server_ts"].as_u64().map(|ms| ms / 1000);
        let latest_sender = latest["sender"].as_str().map(str::to_string);
        out.push(crate::models::ThreadPreview {
            root_id: root_id.to_string(),
            sender_name: name_of(&sender).await,
            sender,
            body,
            reply_count: thread["count"].as_u64().unwrap_or(0) as u32,
            last_activity: ts(latest).or_else(|| ts(&json)).unwrap_or(0),
            latest_sender_name: match latest_sender {
                Some(s) => Some(name_of(&s).await),
                None => None,
            },
            full_body: full.chars().take(PREVIEW_TEXT_LIMIT).collect(),
            latest_body: latest["content"]["body"].as_str()
                .map(|b| b.trim().chars().take(PREVIEW_TEXT_LIMIT).collect::<String>())
                .filter(|b| !b.is_empty()),
        });
    }
    // The server orders by latest activity; keep that, newest first.
    out.sort_by_key(|a| std::cmp::Reverse(a.last_activity));
    Ok(out)
}

/// Set our own presence (online, or offline to stop sharing activity).
pub async fn set_own_presence(client: &matrix_sdk::Client, online: bool) -> Result<(), AppError> {
    use matrix_sdk::ruma::api::client::presence::set_presence;
    use matrix_sdk::ruma::presence::PresenceState;
    let user_id = client.user_id().ok_or_else(|| AppError::Other("Not logged in".into()))?.to_owned();
    let state = if online { PresenceState::Online } else { PresenceState::Offline };
    client.send(set_presence::v3::Request::new(user_id, state)).await
        .map_err(|e| AppError::Other(format!("Couldn't set presence: {e}")))?;
    Ok(())
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
            Ok(Some(raw)) => raw.deserialize().ok()
                .map(|ev| (presence_label(&ev.content.presence).to_string(), ev.content.status_msg.clone()))
                .unwrap_or(("offline".to_string(), None)),
            _ => ("offline".to_string(), None),
        };
        result.push(MemberInfo {
            user_id: m.user_id().to_string(),
            display_name: m.display_name().map(|s| s.to_string()).unwrap_or_else(|| m.user_id().to_string()),
            avatar_url: m.avatar_url().map(|mxc| mxc.to_string()),
            power_level, role, presence, status_msg,
            server_name: m.user_id().server_name().to_string(),
            last_active_secs: None,
        });
    }
    // Sliding sync carries no presence, so the store is usually empty: ask
    // the servers directly for small rooms.
    if result.len() <= PRESENCE_FETCH_LIMIT {
        use futures_util::StreamExt;
        let ids: Vec<(usize, String)> = result.iter().map(|m| m.user_id.clone()).enumerate().collect();
        let fetched: Vec<Option<(usize, crate::models::Presence)>> = futures_util::stream::iter(ids)
            .map(|(i, uid)| {
                let client = client.clone();
                async move { fetch_presence(&client, &uid).await.ok().map(|p| (i, p)) }
            })
            .buffer_unordered(PRESENCE_FETCH_CONCURRENCY)
            .collect()
            .await;
        let fetched = fetched.into_iter().flatten();
        for (i, p) in fetched {
            let m = &mut result[i];
            m.presence = p.state;
            m.last_active_secs = p.last_active_secs;
            if p.status_msg.is_some() {
                m.status_msg = p.status_msg;
            }
        }
    }
    result.sort_by(|a, b| b.power_level.cmp(&a.power_level).then_with(|| a.display_name.to_lowercase().cmp(&b.display_name.to_lowercase())));
    Ok(result)
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

/// Fetch an event's raw JSON (pretty-printed) for "View source".
pub async fn get_event_source(client: &matrix_sdk::Client, room_id: &str, event_id: &str) -> Result<String, AppError> {
    let room_id = parse_room_id(room_id)?;
    let room = client.get_room(&room_id).ok_or(AppError::RoomNotFound(room_id.to_string()))?;
    let event_id = matrix_sdk::ruma::EventId::parse(event_id).map_err(|_| AppError::Other("Invalid event ID".into()))?;
    let event = room.event(&event_id, None).await.map_err(|e| AppError::Other(format!("Failed to fetch event: {e}")))?;
    let value: serde_json::Value = serde_json::from_str(event.raw().json().get())
        .map_err(|e| AppError::Other(format!("Invalid event JSON: {e}")))?;
    serde_json::to_string_pretty(&value).map_err(|e| AppError::Other(e.to_string()))
}
