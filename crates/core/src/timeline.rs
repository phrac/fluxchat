use std::collections::HashMap;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use futures_util::{FutureExt, StreamExt};
use matrix_sdk::ruma::api::client::receipt::create_receipt::v3::ReceiptType;
use matrix_sdk_ui::eyeball_im::{Vector, VectorDiff};
use matrix_sdk_ui::timeline::{
    EventSendState, EventTimelineItem, Timeline, TimelineBuilder, TimelineDetails, TimelineFocus,
    TimelineItem, TimelineItemContent, TimelineItemKind, VirtualTimelineItem,
};
use tokio::sync::Notify;

use crate::models::{MessageInfo, PollInfo, ReactionInfo};

/// Back-paginate a freshly opened timeline until it has at least this many
/// non-state items, so a room never opens to a near-empty view.
const INITIAL_BACKFILL: u16 = 30;

/// Row id of the fully-read marker in a message list.
pub const READ_MARKER_ID: &str = "~readmarker";

/// Receives a view's complete, ordered message list every time its Timeline
/// changes. Called only from that view's watcher task, so updates for a view
/// are always delivered in order.
pub type TimelineSink = Arc<dyn Fn(String, Vec<MessageInfo>) + Send + Sync>;

/// Timelines are keyed by view: a room ID for the live room timeline, or
/// `thread::{room_id}::{root_event_id}` for a thread.
pub fn thread_key(room_id: &str, root_event_id: &str) -> String {
    format!("thread::{room_id}::{root_event_id}")
}

/// Split a thread key into (room_id, root_event_id).
pub fn parse_thread_key(key: &str) -> Option<(&str, &str)> {
    let rest = key.strip_prefix("thread::")?;
    rest.split_once("::")
}

/// The room a view key belongs to.
pub fn real_room_id(key: &str) -> &str {
    parse_thread_key(key).map(|(room, _)| room).unwrap_or(key)
}

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

/// Store of Timeline instances per view key. Every Timeline is created
/// together with exactly one watcher task that streams its changes to the
/// sink, and the watcher is aborted when the Timeline is removed — so there is
/// never a Timeline without a watcher, or two watchers for one Timeline.
#[derive(Default)]
pub struct RoomTimelines {
    map: tokio::sync::Mutex<HashMap<String, RoomTimeline>>,
    sink: std::sync::Mutex<Option<TimelineSink>>,
}

impl RoomTimelines {
    /// Set the callback that receives message-list updates for all views.
    pub fn set_sink(&self, sink: TimelineSink) {
        *self.sink.lock().unwrap_or_else(|e| e.into_inner()) = Some(sink);
    }

    /// Get a view's Timeline if one is open, without creating it.
    pub async fn get(&self, key: &str) -> Option<Arc<Timeline>> {
        self.map.lock().await.get(key).map(|rt| rt.timeline.clone())
    }

    /// Ask a view's watcher to re-send its current message list.
    /// Returns false if the view has no open Timeline.
    pub async fn request_snapshot(&self, key: &str) -> bool {
        match self.map.lock().await.get(key) {
            Some(rt) => {
                rt.resend.notify_one();
                true
            }
            None => false,
        }
    }

    /// Drop a view's Timeline and stop its watcher.
    pub async fn remove(&self, key: &str) {
        self.map.lock().await.remove(key);
    }

    /// Drop all Timelines and stop all watchers (logout).
    pub async fn clear(&self) {
        self.map.lock().await.clear();
    }
}

/// Get or create the Timeline for a view key (room or thread). Creating one
/// also spawns its watcher.
pub async fn get_or_create_for_key(
    client: &matrix_sdk::Client,
    timelines: &RoomTimelines,
    key: &str,
) -> Result<Arc<Timeline>, crate::error::AppError> {
    let room_id = crate::rooms::parse_room_id(real_room_id(key))?;
    let root = match parse_thread_key(key) {
        Some((_, root)) => Some(matrix_sdk::ruma::EventId::parse(root)
            .map_err(|_| crate::error::AppError::Other(format!("Invalid thread root in {key}")))?),
        None => None,
    };
    let mut map = timelines.map.lock().await;
    if let Some(rt) = map.get(key) {
        if !rt.watcher.is_finished() {
            return Ok(rt.timeline.clone());
        }
        tracing::warn!("Timeline watcher for {key} had stopped — rebuilding timeline");
        map.remove(key);
    }
    let sink = timelines.sink.lock().unwrap_or_else(|e| e.into_inner()).clone()
        .ok_or_else(|| crate::error::AppError::Other("Timeline sink not set".into()))?;
    let room = client.get_room(&room_id)
        .ok_or_else(|| crate::error::AppError::RoomNotFound(room_id.to_string()))?;
    // Read receipts are off by default in the SDK; without this every
    // item's receipt list is empty and nothing shows who read what.
    let mut builder = TimelineBuilder::new(&room)
        .track_read_marker_and_receipts(matrix_sdk_ui::timeline::TimelineReadReceiptTracking::MessageLikeEvents);
    if let Some(root_event_id) = root {
        builder = builder.with_focus(TimelineFocus::Thread { root_event_id });
    }
    let timeline = Arc::new(builder.build().await
        .map_err(|e| crate::error::AppError::Other(format!("Failed to build timeline: {e}")))?);
    let resend = Arc::new(Notify::new());
    let watcher = tokio::spawn(watch_timeline(
        key.to_string(), timeline.clone(), resend.clone(), sink,
    )).abort_handle();
    map.insert(key.to_string(), RoomTimeline { timeline: timeline.clone(), resend, watcher });
    Ok(timeline)
}

/// Get or create the live Timeline for a room.
pub async fn get_or_create_timeline(
    client: &matrix_sdk::Client,
    timelines: &RoomTimelines,
    room_id: &matrix_sdk::ruma::RoomId,
) -> Result<Arc<Timeline>, crate::error::AppError> {
    get_or_create_for_key(client, timelines, room_id.as_str()).await
}

/// Send read receipt + fully-read marker up to the latest event of a view's
/// Timeline (threaded receipts for thread views). The SDK skips the request
/// when nothing new would be covered, so this is cheap to call often.
pub async fn mark_as_read(
    timelines: &RoomTimelines,
    key: &str,
    private: bool,
) -> Result<(), crate::error::AppError> {
    let Some(timeline) = timelines.get(key).await else { return Ok(()) };
    // A private receipt clears our unread state without telling others.
    let read = if private { ReceiptType::ReadPrivate } else { ReceiptType::Read };
    for receipt_type in [read, ReceiptType::FullyRead] {
        timeline.mark_as_read(receipt_type).await
            .map_err(|e| crate::error::AppError::Other(format!("Failed to send receipt: {e}")))?;
    }
    Ok(())
}

/// Send a text message (or `/me` emote) into a view through its Timeline, so
/// it gets a local echo, the send queue's retries, and — for thread views or
/// replies to threaded messages — the right `m.thread` relation.
pub async fn send_text(
    client: &matrix_sdk::Client,
    timelines: &RoomTimelines,
    key: &str,
    body: &str,
    reply_to: Option<&str>,
    markdown: bool,
) -> Result<(), crate::error::AppError> {
    let room_id = crate::rooms::parse_room_id(real_room_id(key))?;
    let room = client.get_room(&room_id)
        .ok_or_else(|| crate::error::AppError::RoomNotFound(room_id.to_string()))?;
    let mut content = match body.strip_prefix("/me ") {
        Some(emote) => crate::messages::build_emote_content(&room, emote).await,
        None => crate::messages::build_message_content(&room, body).await,
    };
    if !markdown {
        // Plain text as typed; keep the resolved intentional mentions.
        use matrix_sdk::ruma::events::room::message::RoomMessageEventContent;
        let mentions = content.mentions.take();
        content = match body.strip_prefix("/me ") {
            Some(emote) => RoomMessageEventContent::emote_plain(emote),
            None => RoomMessageEventContent::text_plain(body),
        };
        content.mentions = mentions;
    }
    let timeline = get_or_create_for_key(client, timelines, key).await?;
    let result = match reply_to {
        Some(event_id) => {
            let event_id = matrix_sdk::ruma::EventId::parse(event_id)
                .map_err(|_| crate::error::AppError::Other("Invalid event ID".into()))?;
            timeline.send_reply(content.into(), event_id).await
        }
        None => timeline.send(content.into()).await.map(|_| ()),
    };
    result.map_err(|e| crate::error::AppError::Other(format!("Failed to send: {e}")))
}

/// Send a file into a view through its Timeline (local echo, send queue,
/// thread relation in thread views), with `caption` — formatted like a typed
/// message, mentions included — on the same event, so text + file is one send.
#[allow(clippy::too_many_arguments)]
pub async fn send_attachment(
    client: &matrix_sdk::Client,
    timelines: &RoomTimelines,
    key: &str,
    filename: &str,
    mime_type: &str,
    data: Vec<u8>,
    caption: Option<&str>,
    reply_to: Option<&str>,
) -> Result<(), crate::error::AppError> {
    use matrix_sdk::ruma::events::room::message::MessageType;
    use matrix_sdk_ui::timeline::{AttachmentConfig, AttachmentSource};

    let room_id = crate::rooms::parse_room_id(real_room_id(key))?;
    let room = client.get_room(&room_id)
        .ok_or_else(|| crate::error::AppError::RoomNotFound(room_id.to_string()))?;
    let content_type: mime::Mime = mime_type.parse().unwrap_or(mime::APPLICATION_OCTET_STREAM);

    let mut config = AttachmentConfig::default();
    if mime_type.starts_with("image/") {
        // Decoding + resizing a large image is CPU-bound: keep it off the
        // async worker (block_in_place avoids copying the image to move it).
        let make = || crate::media::generate_thumbnail(&data, 320, 240);
        let multi_thread = tokio::runtime::Handle::try_current()
            .is_ok_and(|h| h.runtime_flavor() == tokio::runtime::RuntimeFlavor::MultiThread);
        let thumb = if multi_thread { tokio::task::block_in_place(make) } else { make() };
        if let Some((thumb, w, h)) = thumb {
            use matrix_sdk::ruma::UInt;
            config.thumbnail = Some(matrix_sdk::attachment::Thumbnail {
                size: UInt::try_from(thumb.len()).unwrap_or_default(),
                data: thumb,
                content_type: mime::IMAGE_PNG,
                height: UInt::from(h),
                width: UInt::from(w),
            });
        }
    }
    if let Some(text) = caption.map(str::trim).filter(|t| !t.is_empty()) {
        let content = crate::messages::build_message_content(&room, text).await;
        config.mentions = content.mentions.clone();
        if let MessageType::Text(t) = content.msgtype {
            config.caption = Some(t);
        }
    }
    if let Some(event_id) = reply_to {
        config.in_reply_to = Some(matrix_sdk::ruma::EventId::parse(event_id)
            .map_err(|_| crate::error::AppError::Other("Invalid event ID".into()))?);
    }

    let timeline = get_or_create_for_key(client, timelines, key).await?;
    timeline
        .send_attachment(AttachmentSource::Data { bytes: data, filename: filename.to_string() }, content_type, config)
        .use_send_queue()
        .await
        .map_err(|e| crate::error::AppError::Other(format!("Failed to upload: {e}")))
}

/// Send a voice message (Ogg/Opus, MSC3245 voice flag) to a view through
/// its Timeline: in a thread view it goes into the thread, with local echo.
pub async fn send_voice(
    client: &matrix_sdk::Client,
    timelines: &RoomTimelines,
    key: &str,
    ogg_bytes: Vec<u8>,
    duration_ms: u64,
) -> Result<(), crate::error::AppError> {
    use matrix_sdk::attachment::{AttachmentInfo, BaseAudioInfo};
    use matrix_sdk_ui::timeline::{AttachmentConfig, AttachmentSource};
    let size = ogg_bytes.len() as u64;
    let config = AttachmentConfig {
        info: Some(AttachmentInfo::Voice(BaseAudioInfo {
            duration: Some(std::time::Duration::from_millis(duration_ms)),
            size: size.try_into().ok(),
            waveform: None,
        })),
        ..Default::default()
    };
    let content_type: mime::Mime = "audio/ogg".parse().unwrap_or(mime::APPLICATION_OCTET_STREAM);
    let timeline = get_or_create_for_key(client, timelines, key).await?;
    timeline
        .send_attachment(AttachmentSource::Data { bytes: ogg_bytes, filename: "Voice message.ogg".into() }, content_type, config)
        .use_send_queue()
        .await
        .map_err(|e| crate::error::AppError::Other(format!("Failed to send voice: {e}")))
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

    let backfill = Arc::new(Backfill::default());
    backfill.maybe_run(&timeline, &room_id, &items);

    futures_util::pin_mut!(stream);
    loop {
        tokio::select! {
            batch = stream.next() => {
                let Some(batch) = batch else { break };
                let mut reset = apply_diffs(&mut items, batch);
                // Coalesce diffs that are already queued into one emit.
                let mut ended = false;
                loop {
                    match stream.next().now_or_never() {
                        Some(Some(more)) => reset |= apply_diffs(&mut items, more),
                        Some(None) => { ended = true; break; }
                        None => break,
                    }
                }
                sink(room_id.clone(), flatten_messages(&items));
                if ended { break; }
                // The SDK clears a timeline when its event cache resets (gappy
                // sync, lagged updates); that starts a fresh fill.
                if reset {
                    backfill.restart();
                }
                // Each completed page lands here as diffs, so this keeps
                // paginating until the view has enough rows to show.
                backfill.maybe_run(&timeline, &room_id, &items);
            }
            _ = resend.notified() => {
                sink(room_id.clone(), flatten_messages(&items));
            }
            _ = backfill.finished.notified() => {
                backfill.maybe_run(&timeline, &room_id, &items);
            }
        }
    }
    tracing::warn!("Timeline diff stream ended for {room_id}");
}

/// Keeps a view filled with at least `INITIAL_BACKFILL` rows it will actually
/// display. Counting raw events isn't enough: a room whose recent history is
/// all thread replies, edits and reactions has nothing to show in its main
/// view, so pages are fetched one at a time until visible rows appear, the
/// start of the room is reached, a page fails `MAX_BACKFILL_RETRIES` times in
/// a row, or `MAX_BACKFILL_PAGES` runs out (until the next `restart`).
#[derive(Default)]
struct Backfill {
    running: AtomicBool,
    done: AtomicBool,
    pages: std::sync::atomic::AtomicU32,
    /// Consecutive failed pages.
    failures: std::sync::atomic::AtomicU32,
    /// Signalled when a page finishes, so the watcher re-checks even if the
    /// page only added events it doesn't show.
    finished: Notify,
}

/// Upper bound on automatic back-pagination per fill (pages of
/// `INITIAL_BACKFILL` events), so a huge thread-only stretch can't page forever.
const MAX_BACKFILL_PAGES: u32 = 20;
/// Attempts per page before giving up. A page that times out (often a burst
/// of other requests on a slow link) would otherwise leave a thread or room
/// empty until it's reopened.
const MAX_BACKFILL_RETRIES: u32 = 4;
/// Wait before the first retry; doubles each time.
const BACKFILL_RETRY_DELAY: std::time::Duration = std::time::Duration::from_secs(2);

impl Backfill {
    fn restart(&self) {
        self.done.store(false, Ordering::Release);
        self.pages.store(0, Ordering::Release);
        self.failures.store(0, Ordering::Release);
    }

    fn maybe_run(self: &Arc<Self>, timeline: &Arc<Timeline>, key: &str, items: &Vector<Option<MessageInfo>>) {
        if self.done.load(Ordering::Acquire) {
            return;
        }
        let is_thread = parse_thread_key(key).is_some();
        let visible = items.iter().flatten()
            .filter(|m| !m.is_system() && (is_thread || m.thread_root.is_none()))
            .count();
        if visible >= INITIAL_BACKFILL as usize {
            self.done.store(true, Ordering::Release);
            return;
        }
        if self.running.swap(true, Ordering::AcqRel) {
            return;
        }
        if self.pages.fetch_add(1, Ordering::AcqRel) >= MAX_BACKFILL_PAGES {
            self.done.store(true, Ordering::Release);
            self.running.store(false, Ordering::Release);
            return;
        }
        let this = self.clone();
        let timeline = timeline.clone();
        let key = key.to_string();
        tokio::spawn(async move {
            match timeline.paginate_backwards(INITIAL_BACKFILL).await {
                Ok(true) => this.done.store(true, Ordering::Release),
                Ok(false) => this.failures.store(0, Ordering::Release),
                Err(e) => {
                    let failures = this.failures.fetch_add(1, Ordering::AcqRel) + 1;
                    if failures >= MAX_BACKFILL_RETRIES {
                        tracing::warn!("Back-pagination failed for {key}, giving up: {e}");
                        this.done.store(true, Ordering::Release);
                    } else {
                        tracing::warn!("Back-pagination failed for {key} (attempt {failures}), retrying: {e}");
                        // A failed page doesn't use up the page budget.
                        this.pages.fetch_sub(1, Ordering::AcqRel);
                        tokio::time::sleep(BACKFILL_RETRY_DELAY * 2u32.pow(failures - 1)).await;
                    }
                }
            }
            this.running.store(false, Ordering::Release);
            this.finished.notify_one();
        });
    }
}

/// Apply a batch of diffs to the mirror. Returns true if the batch cleared or
/// replaced the whole list.
fn apply_diffs(items: &mut Vector<Option<MessageInfo>>, diffs: Vec<VectorDiff<Arc<TimelineItem>>>) -> bool {
    let mut reset = false;
    for diff in diffs {
        reset |= matches!(diff, VectorDiff::Clear | VectorDiff::Reset { .. });
        diff.map(|item| timeline_item_to_message_info(&item)).apply(items);
    }
    reset
}

/// Short user-facing explanation for an SDK `UtdCause` (by its Debug name).
fn utd_cause_text(cause: &str) -> &'static str {
    match cause {
        "SentBeforeWeJoined" => "sent before you joined",
        "VerificationViolation" | "UnsignedDevice" | "UnknownDevice" =>
            "sender's client won't share keys with an unverified session",
        "HistoricalMessageAndBackupIsDisabled" | "HistoricalMessageAndDeviceIsUnverified" =>
            "older message; key backup isn't set up",
        "WithheldForUnverifiedOrInsecureDevice" => "sender withheld the key: verify this session",
        "WithheldBySender" => "sender withheld the key",
        _ => "missing session key",
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
        TimelineItemKind::Virtual(VirtualTimelineItem::DateDivider(ts)) => Some(MessageInfo {
            event_id: format!("~date-{}", ts.0),
            timestamp: ts.as_secs().into(),
            ..Default::default()
        }),
        // Where our fully-read marker sits — the app opens rooms here.
        TimelineItemKind::Virtual(VirtualTimelineItem::ReadMarker) => Some(MessageInfo {
            event_id: READ_MARKER_ID.to_string(),
            ..Default::default()
        }),
        TimelineItemKind::Virtual(_) => None, // timeline start
    }
}

/// Convert an EventTimelineItem to MessageInfo.
/// The `org.fluxchat.agent` object of an event (its latest edit's
/// `m.new_content` first), if it has one. Only events that mention the key
/// are parsed.
fn agent_meta(ev: &EventTimelineItem) -> Option<serde_json::Value> {
    const KEY: &str = "org.fluxchat.agent";
    let from = |raw: &str| -> Option<serde_json::Value> {
        if !raw.contains(KEY) {
            return None;
        }
        let val: serde_json::Value = serde_json::from_str(raw).ok()?;
        let content = val.get("content")?;
        content.get("m.new_content").and_then(|c| c.get(KEY))
            .or_else(|| content.get(KEY))
            .filter(|v| v.is_object())
            .cloned()
    };
    ev.latest_json().and_then(|r| from(r.json().get()))
        .or_else(|| ev.original_json().and_then(|r| from(r.json().get())))
}

fn event_to_message_info(ev: &EventTimelineItem) -> Option<MessageInfo> {
    let mut info = convert_event(ev)?;
    if let Some(message) = ev.content().as_message() {
        use matrix_sdk::ruma::events::room::message::MessageType;
        let parts = match message.msgtype() {
            MessageType::Image(c) => Some((c.caption(), c.formatted_caption(), c.filename.as_deref())),
            MessageType::File(c) => Some((c.caption(), c.formatted_caption(), c.filename.as_deref())),
            MessageType::Video(c) => Some((c.caption(), c.formatted_caption(), c.filename.as_deref())),
            MessageType::Audio(c) => Some((c.caption(), c.formatted_caption(), c.filename.as_deref())),
            _ => None,
        };
        if let Some((Some(caption), formatted, filename)) = parts {
            info.caption = Some(caption.to_string());
            info.caption_html = formatted
                .filter(|f| f.format == matrix_sdk::ruma::events::room::message::MessageFormat::Html)
                .map(|f| f.body.clone());
            if let Some(name) = filename {
                info.body = name.to_string();
            }
        }
    }
    info.thread_latest = ev.content().thread_summary().and_then(|summary| {
        let TimelineDetails::Ready(latest) = summary.latest_event else { return None };
        let sender = latest.sender.to_string();
        let (sender_name, avatar_url) = match &latest.sender_profile {
            TimelineDetails::Ready(p) => (
                p.display_name.clone().unwrap_or_else(|| latest.sender.localpart().to_string()),
                p.avatar_url.as_ref().map(|u| u.to_string()),
            ),
            _ => (latest.sender.localpart().to_string(), None),
        };
        let body = latest.content.as_message().map(|m| m.body().to_string()).unwrap_or_default();
        Some(crate::models::ThreadLatest { sender, sender_name, avatar_url, body })
    });
    Some(info)
}

fn convert_event(ev: &EventTimelineItem) -> Option<MessageInfo> {
    let event_id = if let Some(eid) = ev.event_id() {
        eid.to_string()
    } else {
        let txn_id = ev.transaction_id()?;
        format!("~local-{}", txn_id)
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
    let send_state = ev.send_state().and_then(|state| match state {
        EventSendState::NotSentYet { .. } => Some(crate::models::SendState::Sending),
        EventSendState::SendingFailed { is_recoverable, .. } =>
            Some(crate::models::SendState::Failed { recoverable: *is_recoverable }),
        EventSendState::Sent { .. } => None,
    });
    let is_edited = content.as_message().is_some_and(|m| m.is_edited());

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
                // Kept as a placeholder row; hidden unless the user wants
                // "message deleted" placeholders.
                return Some(MessageInfo {
                    event_id, sender, sender_name, avatar_url, timestamp,
                    body: "Message deleted".into(),
                    thread_root: msg_like.thread_root.as_ref().map(|id| id.to_string()),
                    row_kind: crate::models::RowKind::Redacted,
                    ..Default::default()
                });
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
                            file_size = file.info.as_ref().and_then(|i| i.size).map(u64::from);
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
                    utd: None,
                    is_edited,
                    send_state: send_state.clone(),
                    thread_latest: None,
                    caption: None,
                    caption_html: None,
                    row_kind: Default::default(),
                    agent: agent_meta(ev),
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
                    utd: None,
                    is_edited,
                    send_state: send_state.clone(),
                    thread_latest: None,
                    caption: None,
                    caption_html: None,
                    row_kind: Default::default(),
                    agent: None,
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
                    utd: None,
                    is_edited,
                    send_state: send_state.clone(),
                    thread_latest: None,
                    caption: None,
                    caption_html: None,
                    row_kind: Default::default(),
                    agent: None,
                })
            } else {
                // Distinguish encryption failures (missing key) from genuinely
                // unknown content types — users have a shot at fixing UTDs
                // (recover backup, etc.), but "[unsupported]" is dead-end.
                let mut utd = None;
                let body = match &msg_like.kind {
                    matrix_sdk_ui::timeline::MsgLikeKind::UnableToDecrypt(enc) => {
                        use matrix_sdk_ui::timeline::EncryptedMessage;
                        #[allow(deprecated)]
                        match enc {
                            EncryptedMessage::MegolmV1AesSha2 { session_id, cause, sender_key, device_id } => {
                                // Log each unique session at most once per process
                                // to avoid spamming on every timeline re-read.
                                static SEEN: std::sync::OnceLock<std::sync::Mutex<std::collections::HashSet<String>>> =
                                    std::sync::OnceLock::new();
                                let seen = SEEN.get_or_init(|| std::sync::Mutex::new(std::collections::HashSet::new()));
                                let first_time = seen.lock().map(|mut s| s.insert(session_id.clone())).unwrap_or(true);
                                if first_time {
                                    tracing::warn!("UTD: session={session_id} cause={cause:?} device={device_id:?}");
                                }
                                let cause_str = format!("{cause:?}");
                                utd = Some(crate::models::UtdInfo {
                                    session_id: session_id.clone(),
                                    sender_key: sender_key.clone(),
                                    device_id: device_id.as_ref().map(|d| d.to_string()),
                                    cause: cause_str.clone(),
                                });
                                let short: String = session_id.chars().take(8).collect();
                                format!("🔒 [can't decrypt — {} ({short})]", utd_cause_text(&cause_str))
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
                    utd,
                    is_edited: false,
                    send_state,
                    thread_latest: None,
                    caption: None,
                    caption_html: None,
                    row_kind: Default::default(),
                    agent: None,
                })
            }
        }

        TimelineItemContent::MembershipChange(mc) => {
            use matrix_sdk_ui::timeline::MembershipChange as MChange;
            let display = mc.display_name().unwrap_or_else(|| sender_name.clone());
            let row_kind = match mc.change() {
                Some(MChange::Joined) | Some(MChange::InvitationAccepted) | Some(MChange::Left) =>
                    crate::models::RowKind::JoinLeave,
                _ => crate::models::RowKind::Membership,
            };
            let body = match mc.change() {
                // A re-sent join with nothing changed (bots "ensure joined" on
                // every restart) — nothing to tell the user.
                Some(MChange::None) => return None,
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
                utd: None,
            is_edited: false,
            send_state: None,
            thread_latest: None,
            caption: None,
            caption_html: None,
            row_kind,
            agent: None,
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
                utd: None,
            is_edited: false,
            send_state: None,
            thread_latest: None,
            caption: None,
            caption_html: None,
            row_kind: crate::models::RowKind::Profile,
            agent: None,
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
                utd: None,
            is_edited: false,
            send_state: None,
            thread_latest: None,
            caption: None,
            caption_html: None,
            row_kind: crate::models::RowKind::RoomState,
            agent: None,
            })
        }

        _ => None,
    }
}


#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn thread_keys_round_trip() {
        let key = thread_key("!room:example.org", "$root");
        assert_eq!(parse_thread_key(&key), Some(("!room:example.org", "$root")));
        assert_eq!(real_room_id(&key), "!room:example.org");
        assert_eq!(real_room_id("!room:example.org"), "!room:example.org");
        assert_eq!(parse_thread_key("!room:example.org"), None);
    }
}
