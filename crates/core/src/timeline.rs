use std::collections::HashMap;
use std::sync::Arc;
use futures_util::{FutureExt, StreamExt};
use matrix_sdk::ruma::api::client::receipt::create_receipt::v3::ReceiptType;
use matrix_sdk::ruma::OwnedRoomId;
use matrix_sdk_ui::eyeball_im::{Vector, VectorDiff};
use matrix_sdk_ui::timeline::{
    Timeline, TimelineBuilder, TimelineItem, TimelineItemKind, TimelineItemContent,
    EventTimelineItem,
};
use tokio::sync::Notify;

use crate::models::{MessageInfo, PollInfo, ReactionInfo};

/// Back-paginate a freshly opened timeline until it has at least this many
/// non-state items, so a room never opens to a near-empty view.
const INITIAL_BACKFILL: u16 = 30;

/// Receives a room's complete, ordered message list every time its Timeline
/// changes. Called only from that room's watcher task, so updates for a room
/// are always delivered in order.
pub type TimelineSink = Arc<dyn Fn(String, Vec<MessageInfo>) + Send + Sync>;

struct RoomTimeline {
    timeline: Arc<Timeline>,
    /// Wakes the watcher to re-emit its current snapshot.
    resend: Arc<Notify>,
    watcher: tokio::task::AbortHandle,
}

impl Drop for RoomTimeline {
    fn drop(&mut self) {
        self.watcher.abort();
    }
}

/// Store of Timeline instances per room. Every Timeline is created together
/// with exactly one watcher task that streams its changes to the sink, and the
/// watcher is aborted when the Timeline is removed — so there is never a
/// Timeline without a watcher, or two watchers for one Timeline.
#[derive(Default)]
pub struct RoomTimelines {
    map: tokio::sync::Mutex<HashMap<OwnedRoomId, RoomTimeline>>,
    sink: std::sync::Mutex<Option<TimelineSink>>,
}

impl RoomTimelines {
    /// Set the callback that receives message-list updates for all rooms.
    pub fn set_sink(&self, sink: TimelineSink) {
        *self.sink.lock().unwrap_or_else(|e| e.into_inner()) = Some(sink);
    }

    /// Get a room's Timeline if one is open, without creating it.
    pub async fn get(&self, room_id: &matrix_sdk::ruma::RoomId) -> Option<Arc<Timeline>> {
        self.map.lock().await.get(room_id).map(|rt| rt.timeline.clone())
    }

    /// Ask a room's watcher to re-send its current message list.
    /// Returns false if the room has no open Timeline.
    pub async fn request_snapshot(&self, room_id: &matrix_sdk::ruma::RoomId) -> bool {
        match self.map.lock().await.get(room_id) {
            Some(rt) => {
                rt.resend.notify_one();
                true
            }
            None => false,
        }
    }

    /// Drop a room's Timeline and stop its watcher.
    pub async fn remove(&self, room_id: &matrix_sdk::ruma::RoomId) {
        self.map.lock().await.remove(room_id);
    }

    /// Drop all Timelines and stop all watchers (logout).
    pub async fn clear(&self) {
        self.map.lock().await.clear();
    }
}

/// Get or create a Timeline for a room. Creating one also spawns its watcher.
pub async fn get_or_create_timeline(
    client: &matrix_sdk::Client,
    timelines: &RoomTimelines,
    room_id: &matrix_sdk::ruma::RoomId,
) -> Result<Arc<Timeline>, crate::error::AppError> {
    let mut map = timelines.map.lock().await;
    if let Some(rt) = map.get(room_id) {
        if !rt.watcher.is_finished() {
            return Ok(rt.timeline.clone());
        }
        tracing::warn!("Timeline watcher for {room_id} had stopped — rebuilding timeline");
        map.remove(room_id);
    }
    let sink = timelines.sink.lock().unwrap_or_else(|e| e.into_inner()).clone()
        .ok_or_else(|| crate::error::AppError::Other("Timeline sink not set".into()))?;
    let room = client.get_room(room_id)
        .ok_or_else(|| crate::error::AppError::RoomNotFound(room_id.to_string()))?;
    let timeline = Arc::new(TimelineBuilder::new(&room).build().await
        .map_err(|e| crate::error::AppError::Other(format!("Failed to build timeline: {e}")))?);
    let resend = Arc::new(Notify::new());
    let watcher = tokio::spawn(watch_timeline(
        room_id.to_string(), timeline.clone(), resend.clone(), sink,
    )).abort_handle();
    map.insert(room_id.to_owned(), RoomTimeline { timeline: timeline.clone(), resend, watcher });
    Ok(timeline)
}

/// Remove a timeline when a room tab is closed.
pub async fn remove_timeline(
    timelines: &RoomTimelines,
    room_id: &matrix_sdk::ruma::RoomId,
) {
    timelines.remove(room_id).await;
}

/// Send read receipt + fully-read marker up to the latest event of a room's
/// Timeline. The SDK skips the request when nothing new would be covered, so
/// this is cheap to call often.
pub async fn mark_as_read(
    timelines: &RoomTimelines,
    room_id: &matrix_sdk::ruma::RoomId,
) -> Result<(), crate::error::AppError> {
    let Some(timeline) = timelines.get(room_id).await else { return Ok(()) };
    for receipt_type in [ReceiptType::Read, ReceiptType::FullyRead] {
        timeline.mark_as_read(receipt_type).await
            .map_err(|e| crate::error::AppError::Other(format!("Failed to send receipt: {e}")))?;
    }
    Ok(())
}

/// The single consumer of a Timeline's diff stream. Keeps a converted mirror of
/// the SDK's item list by applying each diff to it (so only changed items are
/// re-converted), and emits the full message list after each burst of diffs.
async fn watch_timeline(
    room_id: String,
    timeline: Arc<Timeline>,
    resend: Arc<Notify>,
    sink: TimelineSink,
) {
    let (initial, stream) = timeline.subscribe().await;
    let mut items: Vector<Option<MessageInfo>> = initial.iter()
        .map(|item| timeline_item_to_message_info(item))
        .collect();
    sink(room_id.clone(), flatten_messages(&items));

    let loaded = items.iter().flatten().filter(|m| !m.event_id.starts_with("~state-")).count();
    if loaded < INITIAL_BACKFILL as usize {
        let timeline = timeline.clone();
        let room_id = room_id.clone();
        tokio::spawn(async move {
            if let Err(e) = timeline.paginate_backwards(INITIAL_BACKFILL).await {
                tracing::warn!("Initial back-pagination failed for {room_id}: {e}");
            }
        });
    }

    futures_util::pin_mut!(stream);
    loop {
        tokio::select! {
            batch = stream.next() => {
                let Some(batch) = batch else { break };
                apply_diffs(&mut items, batch);
                // Coalesce diffs that are already queued into one emit.
                let mut ended = false;
                loop {
                    match stream.next().now_or_never() {
                        Some(Some(more)) => apply_diffs(&mut items, more),
                        Some(None) => { ended = true; break; }
                        None => break,
                    }
                }
                sink(room_id.clone(), flatten_messages(&items));
                if ended { break; }
            }
            _ = resend.notified() => {
                sink(room_id.clone(), flatten_messages(&items));
            }
        }
    }
    tracing::warn!("Timeline diff stream ended for {room_id}");
}

fn apply_diffs(items: &mut Vector<Option<MessageInfo>>, diffs: Vec<VectorDiff<Arc<TimelineItem>>>) {
    for diff in diffs {
        diff.map(|item| timeline_item_to_message_info(&item)).apply(items);
    }
}

/// Collect the displayable messages and derive thread reply counts from the
/// replies present in the timeline.
fn flatten_messages(items: &Vector<Option<MessageInfo>>) -> Vec<MessageInfo> {
    let mut msgs: Vec<MessageInfo> = items.iter().flatten().cloned().collect();
    let mut thread_counts: HashMap<String, u32> = HashMap::new();
    for msg in &msgs {
        if let Some(ref root) = msg.thread_root {
            *thread_counts.entry(root.clone()).or_insert(0) += 1;
        }
    }
    for msg in &mut msgs {
        if let Some(&count) = thread_counts.get(&msg.event_id) {
            msg.thread_count = Some(msg.thread_count.unwrap_or(0).max(count));
        }
    }
    msgs
}

/// Convert a TimelineItem to our MessageInfo. Returns None for items we skip (date dividers, etc).
pub fn timeline_item_to_message_info(item: &TimelineItem) -> Option<MessageInfo> {
    match item.kind() {
        TimelineItemKind::Event(ev) => event_to_message_info(ev),
        TimelineItemKind::Virtual(_) => None, // skip date dividers etc for now
    }
}

/// Convert an EventTimelineItem to MessageInfo.
fn event_to_message_info(ev: &EventTimelineItem) -> Option<MessageInfo> {
    let event_id = if let Some(eid) = ev.event_id() {
        eid.to_string()
    } else if let Some(txn_id) = ev.transaction_id() {
        format!("~local-{}", txn_id)
    } else {
        return None;
    };

    let sender = ev.sender().to_string();
    let (sender_name, avatar_url) = match ev.sender_profile() {
        matrix_sdk_ui::timeline::TimelineDetails::Ready(profile) => {
            let name = profile.display_name.clone()
                .unwrap_or_else(|| sender.split(':').next().unwrap_or(&sender).trim_start_matches('@').to_string());
            let avatar = profile.avatar_url.as_ref().map(|u| u.to_string());
            (name, avatar)
        }
        _ => {
            let name = sender.split(':').next().unwrap_or(&sender).trim_start_matches('@').to_string();
            (name, None)
        }
    };

    let timestamp = ev.timestamp().as_secs().into();
    let content = ev.content();

    // Authoritative per-event read-receipt list from the SDK timeline item.
    // The SDK keeps this live; no polling needed.
    let read_receipt_user_ids: Vec<String> = ev.read_receipts()
        .keys()
        .map(|u| u.to_string())
        .collect();

    // Extract reactions (on TimelineItemContent)
    let reactions: Vec<ReactionInfo> = if let Some(rxns) = content.reactions() {
        rxns.iter().map(|(emoji, senders)| {
            let users: Vec<String> = senders.keys().map(|u| u.to_string()).collect();
            ReactionInfo {
                emoji: emoji.to_string(),
                count: users.len() as u32,
                users,
            }
        }).collect()
    } else {
        Vec::new()
    };

    // Extract reply info (on TimelineItemContent)
    let reply_details = content.in_reply_to();
    let reply_to_id = reply_details.as_ref().map(|r| r.event_id.to_string());
    let (reply_to_sender, reply_to_body) = reply_details
        .and_then(|r| {
            if let matrix_sdk_ui::timeline::TimelineDetails::Ready(ref event) = r.event {
                let sender = event.sender.localpart().to_string();
                let body = event.content.as_message()
                    .map(|m| m.body().to_string())
                    .unwrap_or_default();
                if body.is_empty() { None } else { Some((sender, body)) }
            } else {
                None
            }
        })
        .map(|(s, b)| (Some(s), Some(b)))
        .unwrap_or((None, None));

    match content {
        TimelineItemContent::MsgLike(msg_like) => {
            if content.is_redacted() {
                return None;
            }

            let thread_root = msg_like.thread_root.as_ref().map(|id| id.to_string());
            let thread_count = msg_like.thread_summary.as_ref().map(|s| s.num_replies);


            if let Some(message) = content.as_message() {
                let body = message.body().to_string();
                let is_emote = matches!(message.msgtype(), matrix_sdk::ruma::events::room::message::MessageType::Emote(_));
                let is_video = matches!(message.msgtype(), matrix_sdk::ruma::events::room::message::MessageType::Video(_));
                // Extract type-specific fields
                let mut formatted_body = None;
                let mut image_mxc = None;
                let mut image_width = None;
                let mut image_height = None;
                let mut geo_uri = None;
                let mut audio_mxc = None;
                let mut duration_ms = None;
                let mut is_voice_msg = false;
                let mut file_mxc = None;
                let mut file_mimetype = None;
                let mut file_size = None;
                let mut video_mxc = None;

                match message.msgtype() {
                    matrix_sdk::ruma::events::room::message::MessageType::Text(t) => {
                        formatted_body = t.formatted.as_ref().map(|f| f.body.clone());
                    }
                    matrix_sdk::ruma::events::room::message::MessageType::Image(img) => {
                        image_mxc = match &img.source {
                            matrix_sdk::ruma::events::room::MediaSource::Plain(uri) => Some(uri.to_string()),
                            matrix_sdk::ruma::events::room::MediaSource::Encrypted(enc) => Some(enc.url.to_string()),
                        };
                        image_width = img.info.as_ref().and_then(|i| i.width).and_then(|v| u32::try_from(v).ok());
                        image_height = img.info.as_ref().and_then(|i| i.height).and_then(|v| u32::try_from(v).ok());
                    }
                    matrix_sdk::ruma::events::room::message::MessageType::Notice(t) => {
                        formatted_body = t.formatted.as_ref().map(|f| f.body.clone());
                    }
                    matrix_sdk::ruma::events::room::message::MessageType::Emote(t) => {
                        formatted_body = t.formatted.as_ref().map(|f| f.body.clone());
                    }
                    matrix_sdk::ruma::events::room::message::MessageType::Video(vid) => {
                        video_mxc = match &vid.source {
                            matrix_sdk::ruma::events::room::MediaSource::Plain(uri) => Some(uri.to_string()),
                            matrix_sdk::ruma::events::room::MediaSource::Encrypted(enc) => Some(enc.url.to_string()),
                        };
                        // Use thumbnail for preview if available (don't fall back to video source)
                        image_mxc = vid.info.as_ref()
                            .and_then(|i| i.thumbnail_source.as_ref())
                            .map(|s| match s {
                                matrix_sdk::ruma::events::room::MediaSource::Plain(uri) => uri.to_string(),
                                matrix_sdk::ruma::events::room::MediaSource::Encrypted(enc) => enc.url.to_string(),
                            });
                        image_width = vid.info.as_ref().and_then(|i| i.width).and_then(|v| u32::try_from(v).ok());
                        image_height = vid.info.as_ref().and_then(|i| i.height).and_then(|v| u32::try_from(v).ok());
                    }
                    matrix_sdk::ruma::events::room::message::MessageType::File(file) => {
                        let mimetype = file.info.as_ref()
                            .and_then(|i| i.mimetype.as_ref()).cloned();
                        let is_image = mimetype.as_ref().is_some_and(|m| m.starts_with("image/"));
                        let mxc = match &file.source {
                            matrix_sdk::ruma::events::room::MediaSource::Plain(uri) => Some(uri.to_string()),
                            matrix_sdk::ruma::events::room::MediaSource::Encrypted(enc) => Some(enc.url.to_string()),
                        };
                        if is_image {
                            image_mxc = mxc;
                        } else {
                            file_mxc = mxc;
                            file_mimetype = mimetype;
                            file_size = file.info.as_ref().and_then(|i| i.size).and_then(|s| u64::try_from(s).ok());
                        }
                    }
                    matrix_sdk::ruma::events::room::message::MessageType::Audio(audio) => {
                        audio_mxc = match &audio.source {
                            matrix_sdk::ruma::events::room::MediaSource::Plain(uri) => Some(uri.to_string()),
                            matrix_sdk::ruma::events::room::MediaSource::Encrypted(enc) => Some(enc.url.to_string()),
                        };
                        duration_ms = audio.info.as_ref()
                            .and_then(|i| i.duration)
                            .map(|d| d.as_millis() as u64);
                        // Check raw JSON for m.voice marker (MSC3245)
                        if let Some(raw) = ev.original_json().or_else(|| ev.latest_json()) {
                            if let Ok(val) = serde_json::from_str::<serde_json::Value>(raw.json().get()) {
                                if val.get("content").and_then(|c| c.get("org.matrix.msc3245.voice")).is_some() {
                                    is_voice_msg = true;
                                }
                            }
                        }
                    }
                    matrix_sdk::ruma::events::room::message::MessageType::Location(loc) => {
                        geo_uri = Some(loc.geo_uri.clone());
                    }
                    _ => {}
                }

                Some(MessageInfo {
                    event_id, sender, sender_name, body, formatted_body,
                    transformed_body: None, image_mxc, reactions, reply_to_id,
                    reply_to_sender: reply_to_sender.clone(), reply_to_body: reply_to_body.clone(), timestamp, avatar_url,
                    thread_root, thread_count, image_width, image_height, poll: None, geo_uri,
                    audio_mxc, duration_ms, file_mxc, file_mimetype, file_size, video_mxc, is_emote, is_video, is_voice: is_voice_msg,
                    read_receipt_user_ids: read_receipt_user_ids.clone(),
                })
            } else if let Some(sticker) = content.as_sticker() {
                let sc = sticker.content();
                let body = sc.body.clone();
                let mxc = match &sc.source {
                    matrix_sdk::ruma::events::sticker::StickerMediaSource::Plain(uri) => Some(uri.to_string()),
                    _ => None,
                };
                let w = sc.info.width.and_then(|v| u32::try_from(v).ok());
                let h = sc.info.height.and_then(|v| u32::try_from(v).ok());
                Some(MessageInfo {
                    event_id, sender, sender_name, body, formatted_body: None,
                    transformed_body: None, image_mxc: mxc, reactions, reply_to_id,
                    reply_to_sender: reply_to_sender.clone(), reply_to_body: reply_to_body.clone(), timestamp, avatar_url,
                    thread_root, thread_count, image_width: w, image_height: h, poll: None,
                    geo_uri: None, audio_mxc: None, duration_ms: None, file_mxc: None, file_mimetype: None, file_size: None, video_mxc: None, is_emote: false, is_video: false, is_voice: false,
                    read_receipt_user_ids: read_receipt_user_ids.clone(),
                })
            } else if let Some(poll_state) = content.as_poll() {
                let result = poll_state.results();
                let question = result.question.clone();
                let answers: Vec<(String, String)> = result.answers.iter()
                    .map(|a| (a.id.clone(), a.text.clone()))
                    .collect();
                let votes: std::collections::HashMap<String, Vec<String>> = result.votes;
                let total_votes = votes.values().map(|v| v.len() as u32).sum();
                let is_ended = result.end_time.is_some();

                let body = format!("Poll: {}", question);

                Some(MessageInfo {
                    event_id, sender, sender_name, body, formatted_body: None,
                    transformed_body: None, image_mxc: None, reactions, reply_to_id,
                    reply_to_sender: reply_to_sender.clone(), reply_to_body: reply_to_body.clone(), timestamp, avatar_url,
                    thread_root, thread_count, image_width: None, image_height: None,
                    poll: Some(PollInfo {
                        question,
                        answers,
                        votes,
                        is_ended,
                        total_votes,
                        my_vote: None,
                    }),
                    geo_uri: None, audio_mxc: None, duration_ms: None, file_mxc: None, file_mimetype: None, file_size: None, video_mxc: None, is_emote: false, is_video: false, is_voice: false,
                    read_receipt_user_ids: read_receipt_user_ids.clone(),
                })
            } else {
                // Distinguish encryption failures (missing key) from genuinely
                // unknown content types — users have a shot at fixing UTDs
                // (recover backup, etc.), but "[unsupported]" is dead-end.
                let body = match &msg_like.kind {
                    matrix_sdk_ui::timeline::MsgLikeKind::UnableToDecrypt(enc) => {
                        use matrix_sdk_ui::timeline::EncryptedMessage;
                        #[allow(deprecated)]
                        match enc {
                            EncryptedMessage::MegolmV1AesSha2 { session_id, cause, .. } => {
                                // Log each unique session at most once per process
                                // to avoid spamming on every timeline re-read.
                                static SEEN: std::sync::OnceLock<std::sync::Mutex<std::collections::HashSet<String>>> =
                                    std::sync::OnceLock::new();
                                let seen = SEEN.get_or_init(|| std::sync::Mutex::new(std::collections::HashSet::new()));
                                let first_time = seen.lock().map(|mut s| s.insert(session_id.clone())).unwrap_or(true);
                                if first_time {
                                    tracing::warn!("UTD: session={session_id} cause={cause:?}");
                                }
                                format!("🔒 [can't decrypt — missing session key {}]",
                                    session_id.chars().take(8).collect::<String>())
                            }
                            _ => "🔒 [can't decrypt]".into(),
                        }
                    }
                    _ => {
                        if let Some(raw) = ev.original_json().or_else(|| ev.latest_json()) {
                            let preview: String = raw.json().get().chars().take(400).collect();
                            tracing::warn!("unsupported msglike content: {preview}");
                        }
                        "[unsupported]".into()
                    }
                };
                Some(MessageInfo {
                    event_id, sender, sender_name,
                    body, formatted_body: None,
                    transformed_body: None, image_mxc: None, reactions, reply_to_id,
                    reply_to_sender: reply_to_sender.clone(), reply_to_body: reply_to_body.clone(), timestamp, avatar_url,
                    thread_root, thread_count, image_width: None, image_height: None, poll: None,
                    geo_uri: None, audio_mxc: None, duration_ms: None, file_mxc: None, file_mimetype: None, file_size: None, video_mxc: None, is_emote: false, is_video: false, is_voice: false,
                    read_receipt_user_ids,
                })
            }
        }

        TimelineItemContent::MembershipChange(mc) => {
            use matrix_sdk_ui::timeline::MembershipChange as MChange;
            let display = mc.display_name().unwrap_or_else(|| sender_name.clone());
            let body = match mc.change() {
                Some(MChange::Joined) | Some(MChange::InvitationAccepted) => format!("{display} has joined"),
                Some(MChange::Left) => format!("{display} has left"),
                Some(MChange::Banned) => format!("{display} was banned"),
                Some(MChange::Unbanned) => format!("{display} was unbanned"),
                Some(MChange::Kicked) => format!("{display} was kicked"),
                Some(MChange::KickedAndBanned) => format!("{display} was kicked and banned"),
                Some(MChange::Invited) => format!("{display} was invited"),
                Some(MChange::InvitationRejected) => format!("{display} rejected the invite"),
                Some(MChange::InvitationRevoked) => format!("{display}'s invite was revoked"),
                _ => format!("{display} membership changed"),
            };
            Some(MessageInfo {
                event_id: format!("~state-{event_id}"), sender: String::new(), sender_name: String::new(),
                body,
                formatted_body: None, transformed_body: None, image_mxc: None,
                reactions: Vec::new(), reply_to_id: None, reply_to_sender: None,
                reply_to_body: None, timestamp, avatar_url: None,
                thread_root: None, thread_count: None, image_width: None, image_height: None, poll: None,
                geo_uri: None, audio_mxc: None, duration_ms: None, file_mxc: None, file_mimetype: None, file_size: None, video_mxc: None, is_emote: false, is_video: false, is_voice: false,
                read_receipt_user_ids: Vec::new(),
            })
        }

        TimelineItemContent::ProfileChange(pc) => {
            let display = pc.displayname_change().and_then(|c| c.new.clone())
                .unwrap_or_else(|| sender_name.clone());
            Some(MessageInfo {
                event_id: format!("~state-{event_id}"), sender: String::new(), sender_name: String::new(),
                body: format!("{display} changed their profile"),
                formatted_body: None, transformed_body: None, image_mxc: None,
                reactions: Vec::new(), reply_to_id: None, reply_to_sender: None,
                reply_to_body: None, timestamp, avatar_url: None,
                thread_root: None, thread_count: None, image_width: None, image_height: None, poll: None,
                geo_uri: None, audio_mxc: None, duration_ms: None, file_mxc: None, file_mimetype: None, file_size: None, video_mxc: None, is_emote: false, is_video: false, is_voice: false,
                read_receipt_user_ids: Vec::new(),
            })
        }

        TimelineItemContent::OtherState(state) => {
            use matrix_sdk_ui::timeline::AnyOtherStateEventContentChange;
            let body = match state.content() {
                AnyOtherStateEventContentChange::RoomPowerLevels(_) => {
                    format!("{sender_name} changed room power levels")
                }
                AnyOtherStateEventContentChange::RoomName(_) => {
                    format!("{sender_name} changed the room name")
                }
                AnyOtherStateEventContentChange::RoomTopic(_) => {
                    format!("{sender_name} changed the room topic")
                }
                AnyOtherStateEventContentChange::RoomAvatar(_) => {
                    format!("{sender_name} changed the room avatar")
                }
                AnyOtherStateEventContentChange::_Custom { ref event_type }
                    if event_type.contains("call") || event_type.contains("rtc") =>
                {
                    return None; // Suppress call/RTC state events
                }
                _ => format!("{sender_name} updated room settings"),
            };
            Some(MessageInfo {
                event_id: format!("~state-{event_id}"), sender: String::new(), sender_name: String::new(),
                body,
                formatted_body: None, transformed_body: None, image_mxc: None,
                reactions: Vec::new(), reply_to_id: None, reply_to_sender: None,
                reply_to_body: None, timestamp, avatar_url: None,
                thread_root: None, thread_count: None, image_width: None, image_height: None, poll: None,
                geo_uri: None, audio_mxc: None, duration_ms: None, file_mxc: None, file_mimetype: None, file_size: None, video_mxc: None, is_emote: false, is_video: false, is_voice: false,
                read_receipt_user_ids: Vec::new(),
            })
        }

        _ => None,
    }
}

/// Create a thread-focused timeline for a specific thread root.
pub async fn create_thread_timeline(
    client: &matrix_sdk::Client,
    room_id: &matrix_sdk::ruma::RoomId,
    thread_root_id: &matrix_sdk::ruma::EventId,
) -> Result<Arc<Timeline>, crate::error::AppError> {
    use matrix_sdk_ui::timeline::TimelineFocus;
    let room = client.get_room(room_id)
        .ok_or_else(|| crate::error::AppError::RoomNotFound(room_id.to_string()))?;
    let timeline = TimelineBuilder::new(&room)
        .with_focus(TimelineFocus::Thread { root_event_id: thread_root_id.to_owned() })
        .build()
        .await
        .map_err(|e| crate::error::AppError::Other(format!("Failed to build thread timeline: {e}")))?;
    Ok(Arc::new(timeline))
}
