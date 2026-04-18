use iced::Task;
use gstreamer::prelude::*;
use gstreamer_pbutils::prelude::*;

use crate::message::{LoginSuccess, Message, SyncEvent};
use crate::state::{FluxChat, Screen};

/// Cache size limits
const AVATAR_CACHE_LIMIT: usize = 1000;
const IMAGE_CACHE_LIMIT: usize = 500;
const URL_PREVIEW_LIMIT: usize = 200;
const GIF_CACHE_LIMIT: usize = 100;

/// Evict entries from a HashMap when it exceeds the limit.
fn evict_cache<K: std::hash::Hash + Eq + Clone, V>(cache: &mut std::collections::HashMap<K, V>, limit: usize) {
    if cache.len() > limit {
        let to_remove: Vec<K> = cache.keys().take(cache.len() - limit).cloned().collect();
        for key in to_remove {
            cache.remove(&key);
        }
    }
}

/// Transcode animated WebP bytes into GIF bytes so they can be fed into
/// `iced_gif::Frames`, which only understands GIF. Returns None if the input
/// isn't an animated WebP (static WebPs stay on the static image path).
/// Heavy CPU work — always run under `spawn_blocking`.
fn webp_to_gif_bytes(data: &[u8]) -> Option<Vec<u8>> {
    use image_webp::WebPDecoder;
    use gif::{Encoder, Frame, Repeat};

    let mut decoder = match WebPDecoder::new(std::io::Cursor::new(data)) {
        Ok(d) => d,
        Err(e) => { tracing::warn!("webp decode init failed: {e}"); return None; }
    };
    if !decoder.is_animated() || decoder.num_frames() < 2 {
        return None;
    }
    let (w_u32, h_u32) = decoder.dimensions();
    let (w, h): (u16, u16) = (w_u32.try_into().ok()?, h_u32.try_into().ok()?);
    let has_alpha = decoder.has_alpha();
    let buf_size = match decoder.output_buffer_size() {
        Some(s) => s,
        None => { tracing::warn!("webp output_buffer_size returned None"); return None; }
    };

    let mut out = Vec::new();
    {
        let mut encoder = match Encoder::new(&mut out, w, h, &[]) {
            Ok(e) => e,
            Err(e) => { tracing::warn!("gif encoder init failed: {e}"); return None; }
        };
        let _ = encoder.set_repeat(Repeat::Infinite);

        for i in 0..decoder.num_frames() {
            let mut frame_buf = vec![0u8; buf_size];
            let duration_ms = match decoder.read_frame(&mut frame_buf) {
                Ok(d) => d,
                Err(e) => { tracing::warn!("webp read_frame {i} failed: {e}"); return None; }
            };
            let mut rgba = if has_alpha {
                frame_buf
            } else {
                let mut r = Vec::with_capacity(w as usize * h as usize * 4);
                for chunk in frame_buf.chunks_exact(3) {
                    r.extend_from_slice(chunk);
                    r.push(255);
                }
                r
            };
            // Speed 30 = fastest NeuQuant quantization (default is 1 = slowest).
            // For animated stickers we want snappy loading, color fidelity can
            // take the hit. Large animated WebPs (600x400, 20+ frames) go from
            // tens of seconds to sub-second with speed=30.
            let mut frame = Frame::from_rgba_speed(w, h, &mut rgba, 30);
            // GIF delay is in units of 10ms; floor at 2 to avoid 0-delay frames.
            frame.delay = ((duration_ms / 10) as u16).max(2);
            if let Err(e) = encoder.write_frame(&frame) {
                tracing::warn!("gif write_frame {i} failed: {e}");
                return None;
            }
        }
    }
    Some(out)
}

/// Global voice recording handle (cpal::Stream isn't Send, can't store in async state).
/// Safety: only accessed from the main UI thread.
struct SendHandle(Option<fluxchat_core::voice::RecordingHandle>);
unsafe impl Send for SendHandle {}
unsafe impl Sync for SendHandle {}
static VOICE_HANDLE: std::sync::Mutex<SendHandle> = std::sync::Mutex::new(SendHandle(None));

impl FluxChat {
    /// Create state and return an initial task (attempt session restore).
    pub fn new_with_init(
        core: std::sync::Arc<fluxchat_core::AppState>,
    ) -> (Self, Task<Message>) {
        let state = Self::new(core.clone());

        // Try to restore a saved session on startup
        let task = Task::perform(
            async move {
                match fluxchat_core::auth::restore_session(&core).await {
                    Ok(Some((_, result))) => Some(LoginSuccess {
                        user_id: result.user_id,
                        display_name: result.display_name,
                    }),
                    _ => None,
                }
            },
            |result| match result {
                Some(success) => Message::SessionRestored(success),
                None => Message::Noop,
            },
        );

        (state, task)
    }

    /// Elm-style update: process a message and return a task.
    pub fn update(&mut self, message: Message) -> Task<Message> {
        match message {
            // --- Auth ---
            Message::LoginHomeserverChanged(s) => {
                self.login_homeserver = s.clone();
                self.login_caps = None;
                // Fetch login capabilities for this homeserver
                return Task::perform(
                    async move {
                        fluxchat_core::auth::get_login_capabilities(&s).await.ok()
                    },
                    Message::LoginCapsLoaded,
                );
            }
            Message::LoginUsernameChanged(s) => {
                self.login_username = s;
            }
            Message::LoginPasswordChanged(s) => {
                self.login_password = s;
            }
            Message::LoginCapsLoaded(caps) => {
                self.login_caps = caps;
            }
            Message::LoginSubmit => {
                self.login_error = None;
                self.login_submitting = true;
                let core = self.core.clone();
                let homeserver = self.login_homeserver.clone();
                let username = self.login_username.clone();
                let password = self.login_password.clone();

                return Task::perform(
                    async move {
                        match fluxchat_core::auth::login(&core, &homeserver, &username, &password)
                            .await
                        {
                            Ok((_, result)) => Ok(LoginSuccess {
                                user_id: result.user_id,
                                display_name: result.display_name,
                            }),
                            Err(e) => Err(format!("{e}")),
                        }
                    },
                    Message::LoginResult,
                );
            }
            Message::LoginSso => {
                self.login_error = None;
                self.login_submitting = true;
                let core = self.core.clone();
                let homeserver = self.login_homeserver.clone();

                return Task::perform(
                    async move {
                        match fluxchat_core::auth::login_sso(&core, &homeserver).await {
                            Ok((_, result)) => Ok(LoginSuccess {
                                user_id: result.user_id,
                                display_name: result.display_name,
                            }),
                            Err(e) => Err(format!("{e}")),
                        }
                    },
                    Message::LoginResult,
                );
            }
            Message::LoginResult(Ok(success)) => {
                self.login_submitting = false;
                return self.finish_login(success);
            }
            Message::LoginResult(Err(e)) => {
                self.login_error = Some(e);
                self.login_submitting = false;
            }
            Message::SessionRestored(success) => {
                return self.finish_login(success);
            }
            Message::Logout => {
                self.screen = Screen::Login;
                self.user_id.clear();
                self.display_name = None;
                self.rooms.clear();
                self.spaces.clear();
                self.messages_by_room.clear();
                self.open_tabs.clear();
                self.sync_running = false;
                let core = self.core.clone();
                return Task::perform(
                    async move { let _ = fluxchat_core::auth::logout(&core).await; },
                    |_| Message::Noop,
                );
            }

            // --- Post-login startup ---
            Message::SyncStarted => {
                self.sync_running = true;
            }
            Message::SyncReconnected => {
                tracing::info!("Sync reconnected — clearing cached timelines");
                // Clear cached timelines so they get re-created with fresh subscriptions
                let core = self.core.clone();
                let clear_task = Task::perform(
                    async move {
                        core.timelines.lock().await.clear();
                    },
                    |_| Message::Noop,
                );
                // Clear cached messages so the active room reloads from fresh timeline
                self.messages_by_room.clear();
                // Re-select the active room to re-subscribe its timeline
                if let Some(ref room_id) = self.selected_room_id.clone() {
                    return Task::batch([
                        clear_task,
                        Task::done(Message::SelectRoom(room_id.clone())),
                    ]);
                }
                return clear_task;
            }
            Message::InitialRoomsLoaded(rooms, spaces, invited, knocked, space_tree, space_descendants) => {
                let mut mxc_uris: Vec<String> = spaces.iter()
                    .filter_map(|s| s.avatar_url.clone())
                    .collect();
                mxc_uris.extend(rooms.iter().filter_map(|r| r.avatar_url.clone()));
                self.rooms = rooms;
                self.spaces = spaces;
                self.space_tree = space_tree;
                self.space_descendants = space_descendants;
                self.invited_rooms = invited;
                self.knocked_rooms = knocked;
                self.sync_running = true;
                // Load security state for status bar + resolve space/room avatars
                let sec_task = self.update(Message::SecurityLoadState);
                let avatar_task = self.resolve_avatars(mxc_uris);
                // Check if LiveKit/calls are supported + scan all rooms for active calls
                let core_lk = self.core.clone();
                let room_ids: Vec<String> = self.rooms.iter().map(|r| r.room_id.clone()).collect();
                let lk_check = Task::perform(
                    async move {
                        let client = core_lk.get_client().await.ok()?;
                        if fluxchat_core::rtc::get_livekit_service_url(&client).await.is_err() {
                            return None;
                        }
                        let mut rooms_with_calls = std::collections::HashSet::new();
                        for rid in &room_ids {
                            if fluxchat_core::rtc::has_active_call(&client, &rid).await {
                                rooms_with_calls.insert(rid.clone());
                            }
                        }
                        Some(rooms_with_calls)
                    },
                    |result| match result {
                        Some(rooms) => Message::CallRoomsWithCallsUpdated(rooms),
                        _ => Message::Noop,
                    },
                );
                return Task::batch([sec_task, avatar_task, lk_check]);
            }
            Message::TimelineLoaded {
                room_id,
                messages,
                members,
            } => {
                // Collect MXC URIs for avatar resolution
                let mut mxc_uris: Vec<String> = messages
                    .iter()
                    .filter_map(|m| m.avatar_url.clone())
                    .collect();
                if let Some(ref members) = members {
                    mxc_uris.extend(members.iter().filter_map(|m| m.avatar_url.clone()));
                }
                let preview_task = self.resolve_url_previews(&messages);
                let image_task = self.resolve_images(&messages);
                // Track last read event for read receipts
                if let Some(last) = messages.last() {
                    self.last_read_event.insert(room_id.clone(), last.event_id.clone());
                }
                self.messages_by_room.insert(room_id.clone(), messages);
                if let Some(members) = members {
                    self.members_by_room.insert(room_id, members);
                }
                let read_task = self.update(Message::SendReadReceipt);
                let snap = iced::widget::operation::snap_to_end(
                    crate::view::messages_scrollable_id(),
                );
                return Task::batch([self.resolve_avatars(mxc_uris), preview_task, image_task, read_task, snap]);
            }

            // --- Sync events ---
            Message::SyncEvent(event) => match event {
                SyncEvent::RoomListUpdate(rooms, spaces, invited, knocked, space_tree, space_descendants) => {
                    // Zero unread for rooms we've marked as read or the currently selected room
                    let selected = self.selected_room_id.clone();
                    self.rooms = rooms
                        .into_iter()
                        .filter(|r| !self.leaving_rooms.contains(&r.room_id))
                        .map(|mut r| {
                            if self.read_rooms.contains(&r.room_id)
                                || selected.as_deref() == Some(&r.room_id)
                            {
                                r.unread_count = 0;
                                r.highlight_count = 0;
                            }
                            r
                        })
                        .collect();
                    // Clean up selected room / tabs if room was removed (left/pruned)
                    if let Some(ref sel) = self.selected_room_id {
                        if !self.rooms.iter().any(|r| r.room_id == *sel) {
                            self.open_tabs.retain(|(id, _, _, _)| self.rooms.iter().any(|r| r.room_id == *id));
                            self.selected_room_id = self.open_tabs.last().map(|(id, _, _, _)| id.clone());
                        }
                    }

                    // Resolve space and room avatars
                    let mut mxc_uris: Vec<String> = spaces.iter()
                        .filter_map(|s| s.avatar_url.clone())
                        .collect();
                    mxc_uris.extend(self.rooms.iter().filter_map(|r| r.avatar_url.clone()));
                    self.spaces = spaces;
                    self.space_tree = space_tree;
                    self.space_descendants = space_descendants;

                    // Detect new invites and notify
                    let old_invite_ids: std::collections::HashSet<&str> = self.invited_rooms
                        .iter().map(|r| r.room_id.as_str()).collect();
                    let new_invites: Vec<&fluxchat_core::models::RoomInfo> = invited.iter()
                        .filter(|r| !old_invite_ids.contains(r.room_id.as_str()))
                        .collect();
                    if !new_invites.is_empty() {
                        // Auto-expand the Invites section
                        self.collapsed_sections.remove("Invites");
                        // Fire desktop notifications for new invites
                        if self.notifications_enabled {
                            for invite in &new_invites {
                                let _ = notify_rust::Notification::new()
                                    .summary("Room Invite")
                                    .body(&format!("You've been invited to {}", invite.name))
                                    .timeout(5000)
                                    .show();
                            }
                        }
                    }

                    self.invited_rooms = invited;
                    self.knocked_rooms = knocked;
                    if !mxc_uris.is_empty() {
                        return self.resolve_avatars(mxc_uris);
                    }
                }
                SyncEvent::MessagesUpdated(room_id, new_messages) => {
                    // Detect truly new messages for notifications
                    // Detect new messages by comparing against the last seen timestamp
                    let last_ts = self.last_seen_timestamp.get(&room_id).copied().unwrap_or(0);
                    let brand_new: Vec<&fluxchat_core::MessageInfo> = new_messages.iter()
                        .filter(|m| m.timestamp > last_ts
                            && !m.event_id.starts_with("~")
                            && m.sender != self.user_id)
                        .collect();
                    // Update last seen timestamp
                    if let Some(max_ts) = new_messages.iter().map(|m| m.timestamp).max() {
                        self.last_seen_timestamp.insert(room_id.clone(), max_ts);
                    }

                    // Re-mark room as unread if new messages arrive while not viewing it
                    let is_active = self.selected_room_id.as_deref() == Some(&room_id);
                    if !brand_new.is_empty() && (!is_active || !self.app_focused) {
                        self.read_rooms.remove(&room_id);
                    }

                    // Track new messages arriving while scrolled up
                    if is_active && !brand_new.is_empty() && !self.messages_at_bottom {
                        self.new_messages_below = true;
                    }

                    // Fire desktop notifications for new messages
                    if self.notifications_enabled && !brand_new.is_empty()
                        && (!is_active || !self.app_focused)
                    {
                        let room_name = self.rooms.iter()
                            .find(|r| r.room_id == room_id)
                            .map(|r| r.name.clone())
                            .unwrap_or_else(|| room_id.clone());

                        for msg in &brand_new {
                            // Skip if mentions-only mode and not a mention
                            if self.notify_mentions_only {
                                let is_mention = msg.body.to_lowercase().contains(
                                    &self.display_name.as_deref().unwrap_or("").to_lowercase()
                                ) || msg.formatted_body.as_ref().is_some_and(|f|
                                    f.contains(&self.user_id)
                                );
                                if !is_mention { continue; }
                            }

                            let title = format!("{} - {}", msg.sender_name, room_name);
                            let body_text = if self.notify_show_preview {
                                msg.body.chars().take(200).collect::<String>()
                            } else {
                                "New message".into()
                            };

                            let _ = notify_rust::Notification::new()
                                .summary(&title)
                                .body(&body_text)
                                .timeout(5000)
                                .show();
                        }
                    }

                    // Filter out empty messages upfront instead of per-render.
                    let new_messages: Vec<fluxchat_core::MessageInfo> = new_messages.into_iter()
                        .filter(|m| {
                            // Always keep system events, emotes, images, videos, polls, reactions
                            if m.event_id.starts_with("~") || m.is_emote || m.image_mxc.is_some()
                                || m.is_video || m.poll.is_some() || !m.reactions.is_empty() {
                                return true;
                            }
                            // Check body
                            if !m.body.trim().is_empty() {
                                return true;
                            }
                            // Check formatted body — strip HTML tags
                            if let Some(ref f) = m.formatted_body {
                                let text_only: String = f.split('<')
                                    .flat_map(|s| s.split('>').skip(1))
                                    .collect();
                                if !text_only.trim().is_empty() {
                                    return true;
                                }
                            }
                            false
                        })
                        .collect();

                    // Detect Timeline drift: if the SDK's state.items has shrunk
                    // substantially vs. what it reported last time (e.g. due to
                    // a lagged-subscriber recovery that replaced items with just
                    // the current event-cache memory window), older messages lose
                    // their aggregation tracking and reactions on them get stuck.
                    // Trigger an automatic rebuild when this happens.
                    let new_real_count = new_messages.iter()
                        .filter(|m| !m.event_id.starts_with("~"))
                        .count();
                    let last_count = self.last_timeline_real_count.get(&room_id).copied().unwrap_or(0);
                    // Require new_real_count > 0: a transient empty snapshot
                    // (e.g. during room setup or bridge churn) is not drift.
                    let shrunk = last_count >= 10
                        && new_real_count > 0
                        && new_real_count * 2 < last_count;
                    let is_active = self.selected_room_id.as_deref() == Some(&room_id);
                    // Only remember counts we'd actually compare against, so a
                    // transient 0 doesn't become the new baseline.
                    if new_real_count > 0 {
                        self.last_timeline_real_count.insert(room_id.clone(), new_real_count);
                    }
                    if shrunk && is_active {
                        tracing::warn!(
                            "Timeline drift detected for {} ({} -> {} real items) — auto-rebuilding",
                            room_id, last_count, new_real_count,
                        );
                        return Task::done(Message::RefreshTimeline);
                    }

                    // Merge: keep old messages the SDK trimmed from its window,
                    // but always use SDK version for matching IDs (edits applied).
                    let merged = if let Some(existing) = self.messages_by_room.get(&room_id) {
                        if new_messages.is_empty() {
                            // SDK sent empty timeline (e.g. bridge state event) — keep existing
                            existing.clone()
                        } else {
                            let new_ids: std::collections::HashSet<&str> = new_messages
                                .iter()
                                .map(|m| m.event_id.as_str())
                                .collect();
                            let real_msgs = || new_messages.iter()
                                .filter(|m| !m.event_id.starts_with("~"));
                            let oldest_new_ts = real_msgs()
                                .map(|m| m.timestamp)
                                .min()
                                .unwrap_or(0);
                            let newest_new_ts = real_msgs()
                                .map(|m| m.timestamp)
                                .max()
                                .unwrap_or(u64::MAX);
                            // Keep old messages that are:
                            // - older than the SDK window (back-pagination preservation), OR
                            // - newer than the SDK window (protects against stale snapshots
                            //   from Tick/diff-stream races overwriting recent messages)
                            // Messages within the SDK window that aren't in new_ids are
                            // intentionally dropped (redactions, edits replacing event IDs).
                            let mut merged: Vec<fluxchat_core::MessageInfo> = existing
                                .iter()
                                .filter(|m| {
                                    !new_ids.contains(m.event_id.as_str())
                                        && (m.timestamp < oldest_new_ts
                                            || m.timestamp > newest_new_ts)
                                })
                                .cloned()
                                .collect();
                            merged.extend(new_messages);
                            merged.sort_by_key(|m| m.timestamp);
                            merged
                        }
                    } else {
                        new_messages
                    };

                    // Compute thread reply counts once here instead of per-render
                    let mut tc: std::collections::HashMap<String, u32> = std::collections::HashMap::new();
                    for m in &merged {
                        if let Some(ref root) = m.thread_root {
                            *tc.entry(root.clone()).or_insert(0) += 1;
                        }
                    }
                    self.thread_counts.insert(room_id.clone(), tc);
                    // Advance last_read_event if this is the active room
                    if self.selected_room_id.as_deref() == Some(&room_id) {
                        if let Some(last) = merged.last() {
                            self.last_read_event
                                .insert(room_id.clone(), last.event_id.clone());
                        }
                    }
                    let mxc_uris: Vec<String> = merged
                        .iter()
                        .filter_map(|m| m.avatar_url.clone())
                        .collect();
                    let preview_task = self.resolve_url_previews(&merged);
                    let image_task = self.resolve_images(&merged);
                    let new_count = merged.len();
                    self.messages_by_room.insert(room_id, merged);
                    // Clear paginating when messages arrive from diff stream
                    if is_active && self.paginating && new_count != self.paginate_msg_count {
                        let added = new_count - self.paginate_msg_count;
                        // Keep paginating=true until scroll correction settles
                        self.paginate_msg_count = new_count;
                        self.paginate_added_count = added;
                    }
                    let mut tasks = vec![self.resolve_avatars(mxc_uris), preview_task, image_task];
                    if is_active && self.paginate_added_count > 0 {
                        // Scroll down to keep old messages in view after prepending
                        let estimated_height = self.paginate_added_count as f32 * 40.0;
                        let new_offset = self.pre_paginate_offset + estimated_height;
                        self.paginate_added_count = 0;
                        tasks.push(iced::widget::operation::scroll_to(
                            crate::view::messages_scrollable_id(),
                            iced::widget::scrollable::AbsoluteOffset { x: 0.0, y: new_offset },
                        ));
                    } else if is_active && self.messages_at_bottom {
                        tasks.push(iced::widget::operation::snap_to_end(
                            crate::view::messages_scrollable_id(),
                        ));
                    }
                    // Send read receipt for active room when at bottom
                    if is_active && self.messages_at_bottom {
                        tasks.push(self.update(Message::SendReadReceipt));
                    }
                    // Refocus input bar (new messages can steal focus) — but
                    // not while a modal is open, or we steal focus from modal
                    // text inputs (settings password prompts, forward picker,
                    // room create dialog, etc.).
                    if is_active && self.modal.is_none() {
                        tasks.push(iced::widget::operation::focus(crate::view::input_id()));
                    }
                    return Task::batch(tasks);
                }
                SyncEvent::MembersUpdated(room_id, members) => {
                    let mxc_uris: Vec<String> = members
                        .iter()
                        .filter_map(|m| m.avatar_url.clone())
                        .collect();
                    self.members_by_room.insert(room_id, members);
                    return self.resolve_avatars(mxc_uris);
                }
                SyncEvent::TypingUpdate(room_id, user_ids) => {
                    if user_ids.is_empty() {
                        self.typing_by_room.remove(&room_id);
                    } else {
                        self.typing_by_room.insert(room_id, (user_ids, std::time::Instant::now()));
                    }
                }
                SyncEvent::VerificationRequest(info) => {
                    self.modal = Some(crate::state::Modal::Verification { info });
                }
                SyncEvent::SyncLag(ms) => {
                    self.sync_lag_ms = ms;
                }
            },

            // --- Room navigation ---
            Message::SelectRoom(room_id) => {
                let _already_loaded = self.messages_by_room.contains_key(&room_id);
                self.quick_switcher_open = false;
                // When navigating to a non-thread room, remove thread tabs
                if !room_id.starts_with("thread::") {
                    self.open_tabs.retain(|(id, _, _, _)| !id.starts_with("thread::"));
                }
                self.selected_room_id = Some(room_id.clone());
                self.messages_at_bottom = true;
                self.pinned_messages.clear();
                crate::view::set_scroll_room(&room_id);
                self.paginating = false;
                self.paginate_msg_count = 0;
                // Mark as read (session-only, SDK handles server-side)
                self.read_rooms.insert(room_id.clone());
                // Load pins in background for pin icon state
                let pin_load = {
                    let core = self.core.clone();
                    let rid = if room_id.starts_with("thread::") {
                        room_id.splitn(3, "::").nth(1).unwrap_or(&room_id).to_string()
                    } else {
                        room_id.clone()
                    };
                    Task::perform(
                        async move {
                            let client = core.get_client().await.ok()?;
                            fluxchat_core::messages::get_pinned_messages(&client, &rid).await.ok()
                        },
                        |result| match result {
                            Some(pins) => Message::PinsLoaded(pins),
                            None => Message::PinsLoaded(Vec::new()),
                        },
                    )
                };

                // Load timeline + members if not cached
                let needs_messages = !self.messages_by_room.contains_key(&room_id);
                let needs_members = !self.members_by_room.contains_key(&room_id);

                let focus = iced::widget::operation::focus(crate::view::input_id());
                // Check for active calls in this room immediately
                let call_check = {
                    let core = self.core.clone();
                    let rid = room_id.clone();
                    Task::perform(
                        async move {
                            let client = core.get_client().await.ok()?;
                            let participants = fluxchat_core::rtc::get_call_members(&client, &rid).await.ok()?;
                            Some((rid, participants))
                        },
                        |result| match result {
                            Some((rid, participants)) => Message::CallParticipantsUpdated(rid, participants),
                            None => Message::Noop,
                        },
                    )
                };

                if needs_messages || needs_members {
                    let core = self.core.clone();
                    let rid = room_id.clone();
                    let tx = self.timeline_tx.clone();
                    let load = Task::perform(
                        async move {
                            let client = core.get_client().await.ok()?;
                            let room_id_parsed = fluxchat_core::ruma::RoomId::parse(&rid).ok()?;

                            // Load members
                            let members = fluxchat_core::messages::get_room_members(&client, &rid)
                                .await
                                .ok();

                            // Get or create timeline
                            let timeline = fluxchat_core::timeline::get_or_create_timeline(
                                &client,
                                &core.timelines,
                                &room_id_parsed,
                            )
                            .await
                            .ok()?;

                            // Subscribe to get initial items + live stream
                            let (initial_items, stream) = timeline.subscribe().await;

                            // Convert initial items
                            let msgs = fluxchat_core::timeline::timeline_items_to_messages(
                                &initial_items.into_iter().collect::<Vec<_>>(),
                            );

                            // Paginate backwards if not enough messages
                            let real_count = msgs.iter()
                                .filter(|m| !m.event_id.starts_with("~state-"))
                                .count();
                            if real_count < 30 {
                                let _ = timeline.paginate_backwards(30).await;
                            }

                            // Spawn a background task to watch the diff stream
                            {
                                let timeline = timeline.clone();
                                let rid = rid.clone();
                                let tx = tx.clone();
                                tokio::spawn(async move {
                                    use futures_util::StreamExt;
                                    futures_util::pin_mut!(stream);
                                    while let Some(_diffs) = stream.next().await {
                                        let items = timeline.items().await;
                                        let msgs = fluxchat_core::timeline::timeline_items_to_messages(
                                            &items.into_iter().collect::<Vec<_>>(),
                                        );
                                        if tx.send(Message::SyncEvent(
                                            SyncEvent::MessagesUpdated(rid.clone(), msgs),
                                        )).is_err() {
                                            break; // Channel closed
                                        }
                                    }
                                    tracing::warn!("Diff stream watcher ended for room {}", rid);
                                });
                            }

                            Some((rid, msgs, members))
                        },
                        |result| match result {
                            Some((room_id, msgs, members)) => {
                                Message::TimelineLoaded {
                                    room_id,
                                    messages: msgs,
                                    members,
                                }
                            }
                            None => Message::Noop,
                        },
                    );
                    return Task::batch([load, focus, call_check, pin_load]);
                }
                // Re-resolve images for already-cached messages (clear pending to allow retries)
                self.image_cache_pending.clear();
                let cached_msgs: Vec<fluxchat_core::MessageInfo> = self.messages_by_room
                    .get(&room_id)
                    .cloned()
                    .unwrap_or_default();
                let image_task = self.resolve_images(&cached_msgs);
                // Use ScrollToBottom message instead of direct snap_to_end —
                // goes through the update loop, giving iced a frame to render first
                return Task::batch([focus, call_check, image_task, Task::done(Message::ScrollToBottom), pin_load]);
            }
            Message::SelectSpace(space_id) => {
                self.selected_space_id = space_id;
            }
            Message::CloseTab(_) | Message::SwitchTab(_) => {
                // Tabs removed — these are no-ops now
            }
            Message::ToggleSection(section) => {
                if !self.collapsed_sections.remove(&section) {
                    self.collapsed_sections.insert(section);
                }
                // Persist collapsed sections
                let sections: Vec<&str> = self.collapsed_sections.iter().map(|s| s.as_str()).collect();
                if let Ok(json) = serde_json::to_string(&sections) {
                    let _ = self.core.db.set_setting("collapsed_sections", &json);
                }
            }

            // --- Message input ---
            Message::InputChanged(text) => {
                // Handle special key signals
                if text == "/\t" {
                    // Tab cycling through command completions
                    if !self.cmd_completions.is_empty() {
                        self.cmd_completion_idx = (self.cmd_completion_idx + 1) % self.cmd_completions.len();
                        if let Some((name, _)) = self.cmd_completions.get(self.cmd_completion_idx) {
                            self.input_text = format!("{name} ");
                            self.input_content = iced::widget::text_editor::Content::with_text(&self.input_text);
                            // Move cursor to end
                            self.input_content.perform(iced::widget::text_editor::Action::Move(
                                iced::widget::text_editor::Motion::DocumentEnd,
                            ));
                        }
                    }
                    return iced::widget::operation::focus(crate::view::input_id());
                }
                if text == "@\t" {
                    // Tab cycling through mention completions
                    if !self.mention_completions.is_empty() {
                        self.mention_completion_idx = (self.mention_completion_idx + 1) % self.mention_completions.len();
                        if let Some((_, display_name)) = self.mention_completions.get(self.mention_completion_idx) {
                            if let Some(at_pos) = self.input_text.rfind('@') {
                                self.input_text = format!("{}{display_name} ", &self.input_text[..at_pos + 1]);
                                self.input_content = iced::widget::text_editor::Content::with_text(&self.input_text);
                                self.input_content.perform(iced::widget::text_editor::Action::Move(
                                    iced::widget::text_editor::Motion::DocumentEnd,
                                ));
                                // Dismiss the autocomplete HUD after accepting
                                self.mention_completions.clear();
                                self.mention_completion_idx = 0;
                            }
                        }
                    }
                    return iced::widget::operation::focus(crate::view::input_id());
                }
                if text == "\x1b[A" {
                    // Up arrow on empty input — edit last own message
                    if let Some(room_id) = &self.selected_room_id {
                        if let Some(msgs) = self.messages_by_room.get(room_id) {
                            if let Some(msg) = msgs.iter().rev().find(|m| m.sender == self.user_id && !m.event_id.starts_with('~')) {
                                return self.update(Message::StartEdit(msg.event_id.clone(), msg.body.clone()));
                            }
                        }
                    }
                    return Task::none();
                }

                self.input_text = text.clone();

                // Update command autocomplete (skip escaped `//` literal messages)
                if text.starts_with('/') && !text.starts_with("//") && !text.contains(' ') {
                    self.cmd_completions = crate::commands::autocomplete_entries(&text);
                    self.cmd_completion_idx = 0;
                } else {
                    self.cmd_completions.clear();
                }

                // Update @mention autocomplete
                if let Some(at_pos) = text.rfind('@') {
                    let after_at = &text[at_pos + 1..];
                    if !after_at.contains(' ') && !after_at.is_empty() {
                        let query = after_at.to_lowercase();
                        if let Some(room_id) = &self.selected_room_id {
                            if let Some(members) = self.members_by_room.get(room_id) {
                                self.mention_completions = members
                                    .iter()
                                    .filter(|m| m.display_name.to_lowercase().contains(&query))
                                    .take(8)
                                    .map(|m| (m.user_id.clone(), m.display_name.clone()))
                                    .collect();
                                self.mention_completion_idx = 0;
                            }
                        }
                    } else {
                        self.mention_completions.clear();
                    }
                } else {
                    self.mention_completions.clear();
                }
                // Always keep input focused
                return iced::widget::operation::focus(crate::view::input_id());
            }
            Message::InputAction(action) => {
                self.input_content.perform(action);
                self.help_text = None;
                // Sync plain text mirror
                let new_text = self.input_content.text();
                // Trim trailing newline that Content always adds
                let new_text = new_text.trim_end_matches('\n').to_string();
                if new_text != self.input_text {
                    // Update text but don't refocus (InputChanged handles it)
                    self.input_text = new_text.clone();

                    // Update command autocomplete (skip escaped `//` literal messages)
                    if new_text.starts_with('/') && !new_text.starts_with("//") && !new_text.contains(' ') {
                        self.cmd_completions = crate::commands::autocomplete_entries(&new_text);
                        self.cmd_completion_idx = 0;
                    } else {
                        self.cmd_completions.clear();
                    }

                    // Update @mention autocomplete
                    if let Some(at_pos) = new_text.rfind('@') {
                        let after_at = &new_text[at_pos + 1..];
                        if !after_at.contains(' ') && !after_at.is_empty() {
                            let query = after_at.to_lowercase();
                            if let Some(room_id) = &self.selected_room_id {
                                if let Some(members) = self.members_by_room.get(room_id) {
                                    self.mention_completions = members
                                        .iter()
                                        .filter(|m| m.display_name.to_lowercase().contains(&query))
                                        .take(8)
                                        .map(|m| (m.user_id.clone(), m.display_name.clone()))
                                        .collect();
                                    self.mention_completion_idx = 0;
                                }
                            }
                        } else {
                            self.mention_completions.clear();
                        }
                    } else {
                        self.mention_completions.clear();
                    }

                    // Send typing indicator + refocus
                    let focus = iced::widget::operation::focus(crate::view::input_id());
                    if let Some(room_id) = self.selected_room_id.clone() {
                        let core = self.core.clone();
                        let typing = !new_text.is_empty();
                        let typing_task = Task::perform(
                            async move {
                                let client = core.get_client().await.ok()?;
                                fluxchat_core::messages::send_typing(&client, &room_id, typing).await.ok()
                            },
                            |_| Message::Noop,
                        );
                        return Task::batch([typing_task, focus]);
                    }
                    return focus;
                }
                // Even if text didn't change, keep focus
                return iced::widget::operation::focus(crate::view::input_id());
            }
            Message::SendMessage => {
                if self.input_text.trim().is_empty() {
                    return Task::none();
                }

                // Handle slash commands. `//foo` escapes to a literal `/foo` message.
                if self.input_text.starts_with('/') && !self.input_text.starts_with("//") {
                    let result = crate::commands::dispatch(&self.input_text, self);
                    self.input_text.clear();
                    self.input_content = iced::widget::text_editor::Content::new();
                    self.cmd_completions.clear();

                    let focus = iced::widget::operation::focus(crate::view::input_id());
                    match result {
                        crate::commands::CommandResult::Messages(msgs) => {
                            let mut task = focus;
                            for msg in msgs {
                                task = Task::batch([task, self.update(msg)]);
                            }
                            return task;
                        }
                        crate::commands::CommandResult::Rewrite(text) => {
                            // Send the rewritten text directly (don't re-enter command dispatch)
                            self.help_text = None;
                            if let Some(room_id) = self.selected_room_id.clone() {
                                let core = self.core.clone();
                                return Task::perform(
                                    async move {
                                        let client = core.get_client().await.ok()?;
                                        let _ = fluxchat_core::messages::send_message(&client, &room_id, &text).await;
                                        Some(())
                                    },
                                    |_| Message::Noop,
                                );
                            }
                            return Task::none();
                        }
                        crate::commands::CommandResult::Help(text) => {
                            self.help_text = Some(text);
                            return focus;
                        }
                        crate::commands::CommandResult::Noop => {
                            return focus;
                        }
                    }
                }

                let mut body = std::mem::take(&mut self.input_text);
                if body.starts_with("//") {
                    body.remove(0);
                }
                self.input_content = iced::widget::text_editor::Content::new();
                self.help_text = None;
                self.cmd_completions.clear();
                self.mention_completions.clear();

                if let Some(room_id) = self.selected_room_id.clone() {
                    let core = self.core.clone();
                    let reply_to = self.reply_to.take();
                    let edit = self.edit_event.take();
                    let tx_thread = self.timeline_tx.clone();
                    // Detect thread context: synthetic ID is thread::room_id::thread_root_id
                    let thread_ctx = if room_id.starts_with("thread::") {
                        let parts: Vec<&str> = room_id.splitn(3, "::").collect();
                        if parts.len() == 3 {
                            Some((parts[1].to_string(), parts[2].to_string()))
                        } else {
                            None
                        }
                    } else {
                        None
                    };
                    let send = Task::perform(
                        async move {
                            let client = core.get_client().await.ok()?;
                            if let Some((event_id, _)) = edit {
                                // Edit uses the real room_id
                                let real_room = thread_ctx.as_ref().map(|(r, _)| r.as_str()).unwrap_or(&room_id);
                                let _ = fluxchat_core::messages::edit_message(
                                    &client, real_room, &event_id, &body,
                                )
                                .await;
                            } else if let Some((event_id, _, _)) = reply_to {
                                let real_room = thread_ctx.as_ref().map(|(r, _)| r.as_str()).unwrap_or(&room_id);
                                let _ = fluxchat_core::messages::send_reply(
                                    &client, real_room, &body, &event_id,
                                )
                                .await;
                            } else if let Some((real_room, thread_root)) = &thread_ctx {
                                // In a thread tab — send as threaded reply
                                let _ = fluxchat_core::messages::send_thread_reply(
                                    &client, real_room, &body, thread_root,
                                )
                                .await;
                                // Re-fetch thread messages after sending
                                if let Ok(msgs) = fluxchat_core::threads::get_thread_messages(
                                    &client, real_room, thread_root,
                                ).await {
                                    let _ = tx_thread.send(crate::message::Message::SyncEvent(
                                        SyncEvent::MessagesUpdated(room_id.clone(), msgs),
                                    ));
                                }
                            } else {
                                let _ =
                                    fluxchat_core::messages::send_message(&client, &room_id, &body)
                                        .await;
                            }
                            Some(())
                        },
                        |_| Message::Noop,
                    );
                    let snap = iced::widget::operation::snap_to_end(
                        crate::view::messages_scrollable_id(),
                    );
                    return Task::batch([send, iced::widget::operation::focus(crate::view::input_id()), snap]);
                }
            }
            Message::CompleteCommand(idx) => {
                if let Some((name, _)) = self.cmd_completions.get(idx) {
                    self.input_text = format!("{name} ");
                    self.input_content = iced::widget::text_editor::Content::with_text(&self.input_text);
                    self.cmd_completions.clear();
                }
                return iced::widget::operation::focus(crate::view::input_id());
            }
            Message::CompleteMention(idx) => {
                if let Some((_, display_name)) = self.mention_completions.get(idx) {
                    // Replace @partial with @display_name
                    if let Some(at_pos) = self.input_text.rfind('@') {
                        self.input_text = format!("{}{display_name} ", &self.input_text[..at_pos + 1]);
                        self.input_content = iced::widget::text_editor::Content::with_text(&self.input_text);
                    }
                    self.mention_completions.clear();
                }
                return iced::widget::operation::focus(crate::view::input_id());
            }
            Message::StartReply(event_id, sender, body) => {
                self.reply_to = Some((event_id, sender, body));
            }
            Message::CancelReply => {
                self.reply_to = None;
            }
            Message::StartEdit(event_id, body) => {
                self.edit_event = Some((event_id, body.clone()));
                self.input_text = body.clone();
                self.input_content = iced::widget::text_editor::Content::with_text(&body);
            }
            Message::CancelEdit => {
                self.edit_event = None;
                self.input_text.clear();
                self.input_content = iced::widget::text_editor::Content::new();
            }

            // --- Message actions ---
            Message::CopyMessageText(body) => {
                return iced::clipboard::write(body);
            }
            Message::SelectionStarted(widget_id, line, index) => {
                use crate::selectable_text::{GlobalSelectionAnchor, CrossMessageSelection};
                let anchor = GlobalSelectionAnchor { widget_id, line, index };
                self.selection = CrossMessageSelection {
                    cursor: Some(anchor.clone()),
                    anchor: Some(anchor),
                    active: true,
                };
            }
            Message::SelectionMoved(widget_id, line, index) => {
                if self.selection.active {
                    self.selection.cursor = Some(crate::selectable_text::GlobalSelectionAnchor {
                        widget_id, line, index,
                    });
                }
            }
            Message::SelectionEnded => {
                self.selection.active = false;
                // Clear if no actual drag occurred (click without movement)
                if self.selection.anchor == self.selection.cursor {
                    self.selection = Default::default();
                } else if let Some(text) = self.extract_selection_text() {
                    // Mirror selection into the X11/Wayland PRIMARY selection so
                    // Shift+Insert / middle-click paste works in terminals.
                    crate::primary_clipboard::set(text);
                }
            }
            Message::CopySelection => {
                if let Some(text) = self.extract_selection_text() {
                    return iced::clipboard::write(text);
                }
            }
            Message::ClearInput => {
                self.input_text.clear();
                self.input_content = iced::widget::text_editor::Content::new();
                self.reply_to = None;
                self.edit_event = None;
                self.cmd_completions.clear();
                self.mention_completions.clear();
            }
            Message::ToggleReaction(event_id, emoji) => {
                if let Some(room_id) = self.selected_room_id.clone() {
                    let core = self.core.clone();
                    return Task::perform(
                        async move {
                            let client = core.get_client().await.ok()?;
                            let room_id_parsed = fluxchat_core::ruma::RoomId::parse(&room_id).ok()?;
                            let timeline = fluxchat_core::timeline::get_or_create_timeline(
                                &client, &core.timelines, &room_id_parsed,
                            ).await.ok()?;
                            let eid = fluxchat_core::ruma::EventId::parse(&event_id).ok()?.to_owned();
                            let item_id = fluxchat_core::TimelineEventItemId::EventId(eid);
                            // If the Timeline has drifted and no longer has this
                            // event in its state.items, toggle_reaction returns
                            // FailedToToggleReaction — signal up so we can rebuild.
                            Some(timeline.toggle_reaction(&item_id, &emoji).await.is_err())
                        },
                        |failed| if failed.unwrap_or(false) {
                            tracing::warn!("toggle_reaction failed — Timeline drift, auto-rebuilding");
                            Message::RefreshTimeline
                        } else {
                            Message::Noop
                        },
                    );
                }
            }
            Message::DeleteMessage(event_id) => {
                if let Some(room_id) = self.selected_room_id.clone() {
                    // Remove from local state immediately
                    if let Some(msgs) = self.messages_by_room.get_mut(&room_id) {
                        msgs.retain(|m| m.event_id != event_id);
                    }
                    let core = self.core.clone();
                    return Task::perform(
                        async move {
                            let client = core.get_client().await.ok()?;
                            let _ = fluxchat_core::messages::delete_message(
                                &client, &room_id, &event_id,
                            )
                            .await;
                            Some(())
                        },
                        |_| Message::Noop,
                    );
                }
            }
            Message::PinMessage(event_id) => {
                if let Some(room_id) = self.selected_room_id.clone() {
                    // Strip thread prefix for pin operations
                    let real_room = if room_id.starts_with("thread::") {
                        room_id.splitn(3, "::").nth(1).unwrap_or(&room_id).to_string()
                    } else {
                        room_id.clone()
                    };
                    let core = self.core.clone();
                    return Task::perform(
                        async move {
                            let client = core.get_client().await.ok()?;
                            let _ = fluxchat_core::messages::pin_message(
                                &client, &real_room, &event_id,
                            )
                            .await;
                            // Re-fetch pins so the icon updates
                            let pins = fluxchat_core::messages::get_pinned_messages(&client, &real_room).await.ok();
                            Some(pins.unwrap_or_default())
                        },
                        |result| match result {
                            Some(pins) => Message::PinsLoaded(pins),
                            None => Message::Noop,
                        },
                    );
                }
            }
            Message::ForwardMessage(body, formatted, sender) => {
                self.modal = Some(crate::state::Modal::Forward {
                    body,
                    formatted_body: formatted,
                    sender_name: sender,
                });
            }

            // --- Search ---
            Message::SearchQueryChanged(query) => {
                self.search_query = query;
                self.search_selected = None;
                self.highlight_event_id = None;
            }
            Message::SearchSubmit => {
                // If a result is selected, navigate to it on Enter
                if let Some(idx) = self.search_selected {
                    return self.update(Message::SearchSelectResult(idx));
                }
                let query = self.search_query.clone();
                if query.trim().is_empty() {
                    return Task::none();
                }
                if let Some(room_id) = self.selected_room_id.clone() {
                    // Strip synthetic thread:: prefix for search
                    let search_room_id = if room_id.starts_with("thread::") {
                        room_id.split("::").nth(1).unwrap_or(&room_id).to_string()
                    } else {
                        room_id.clone()
                    };
                    let core = self.core.clone();
                    // Grab local messages for client-side fallback
                    let local_msgs: Vec<fluxchat_core::MessageInfo> = self.messages_by_room
                        .get(&room_id)
                        .cloned()
                        .unwrap_or_default();
                    return Task::perform(
                        async move {
                            // Try server-side search first
                            if let Some(client) = core.get_client().await.ok() {
                                match fluxchat_core::messages::search_messages(
                                    &client, &search_room_id, &query, None,
                                )
                                .await
                                {
                                    Ok((results, _next)) if !results.is_empty() => {
                                        return results;
                                    }
                                    Ok(_) => {
                                        tracing::info!("Server search returned 0 results, trying client-side");
                                    }
                                    Err(e) => {
                                        tracing::warn!("Server search failed: {e}, trying client-side");
                                    }
                                }
                            }
                            // Client-side fallback: search loaded messages
                            let query_lower = query.to_lowercase();
                            local_msgs.iter()
                                .filter(|m| m.body.to_lowercase().contains(&query_lower))
                                .map(|m| fluxchat_core::messages::SearchResult {
                                    event_id: m.event_id.clone(),
                                    sender: m.sender.clone(),
                                    sender_name: m.sender_name.clone(),
                                    body: m.body.clone(),
                                    timestamp: m.timestamp,
                                    room_id: search_room_id.clone(),
                                })
                                .collect()
                        },
                        Message::SearchResults,
                    );
                }
            }
            Message::SearchResults(results) => {
                self.search_results = results;
                self.search_selected = None;
            }
            Message::SearchArrowDown => {
                // Quick switcher takes priority
                if self.quick_switcher_open {
                    return self.update(Message::QuickSwitcherDown);
                }
                // Command completions navigation
                if !self.cmd_completions.is_empty() {
                    self.cmd_completion_idx = (self.cmd_completion_idx + 1) % self.cmd_completions.len().min(12);
                    return Task::none();
                }
                let len = self.search_results.len().min(20);
                if len > 0 {
                    self.search_selected = Some(
                        self.search_selected.map(|i| (i + 1).min(len - 1)).unwrap_or(0)
                    );
                }
            }
            Message::SearchArrowUp => {
                if self.quick_switcher_open {
                    return self.update(Message::QuickSwitcherUp);
                }
                if !self.cmd_completions.is_empty() {
                    let len = self.cmd_completions.len().min(12);
                    self.cmd_completion_idx = if self.cmd_completion_idx == 0 { len - 1 } else { self.cmd_completion_idx - 1 };
                    return Task::none();
                }
                if !self.search_results.is_empty() {
                    self.search_selected = Some(
                        self.search_selected.map(|i| i.saturating_sub(1)).unwrap_or(0)
                    );
                }
            }
            Message::SearchSelectResult(index) => {
                if let Some(result) = self.search_results.get(index) {
                    let event_id = result.event_id.clone();
                    self.highlight_event_id = Some(event_id.clone());
                    // Close search UI
                    self.search_open = false;
                    self.search_results.clear();
                    self.search_query.clear();
                    self.search_selected = None;
                    // Scroll to the message if it exists in loaded messages
                    if let Some(room_id) = &self.selected_room_id {
                        if let Some(msgs) = self.messages_by_room.get(room_id) {
                            if let Some(pos) = msgs.iter().position(|m| m.event_id == event_id) {
                                // Estimate scroll offset: each message ~40px
                                let offset = (pos as f32) * 40.0;
                                return iced::widget::operation::scroll_to(
                                    crate::view::messages_scrollable_id(),
                                    iced::widget::scrollable::AbsoluteOffset { x: 0.0, y: offset },
                                );
                            }
                        }
                    }
                }
            }

            // --- Threads ---
            Message::OpenThread(room_id, thread_root_id) => {
                let synthetic_id = format!("thread::{room_id}::{thread_root_id}");
                self.threads_open = false;
                self.overflow_menu_open = false;
                // Track open thread in sidebar
                if !self.open_tabs.iter().any(|(id, _, _, _)| id == &synthetic_id) {
                    let parent_name = self.rooms.iter()
                        .find(|r| r.room_id == room_id)
                        .map(|r| r.name.clone())
                        .unwrap_or_else(|| "thread".into());
                    // Use first line of thread root message as thread name
                    let thread_name = self.messages_by_room
                        .get(&room_id)
                        .and_then(|msgs| msgs.iter().find(|m| m.event_id == thread_root_id))
                        .map(|m| {
                            let preview: String = m.body.chars().take(100).collect();
                            if m.body.chars().count() > 100 { format!("{preview}...") } else { preview }
                        })
                        .unwrap_or_else(|| "Thread".into());
                    self.open_tabs.push((synthetic_id.clone(), thread_name, Some(parent_name), None));
                }
                // If already loaded, just select it
                if self.messages_by_room.contains_key(&synthetic_id) {
                    return self.update(Message::SelectRoom(synthetic_id));
                }
                // Load thread messages via relations API (SDK timeline thread_root is always None in v0.16)
                let core = self.core.clone();
                let sid = synthetic_id.clone();
                let rid = room_id.clone();
                let trid = thread_root_id.clone();
                self.selected_room_id = Some(synthetic_id.clone());
                self.messages_at_bottom = true;
                return Task::perform(
                    async move {
                        let client = core.get_client().await.ok()?;
                        let msgs = fluxchat_core::threads::get_thread_messages(&client, &rid, &trid)
                            .await
                            .unwrap_or_else(|e| {
                                tracing::warn!("Failed to load thread: {e}");
                                Vec::new()
                            });
                        let members = fluxchat_core::messages::get_room_members(&client, &rid).await.ok();
                        Some((sid, msgs, members))
                    },
                    |result| match result {
                        Some((room_id, msgs, members)) => {
                            Message::TimelineLoaded { room_id, messages: msgs, members }
                        }
                        None => Message::Noop,
                    },
                );
            }

            // --- UI panels ---
            Message::ToggleMemberList => {
                self.show_member_list = !self.show_member_list;
            }
            Message::ToggleSearch => {
                self.search_open = !self.search_open;
                if !self.search_open {
                    self.search_results.clear();
                    self.search_selected = None;
                    self.highlight_event_id = None;
                }
                if self.search_open {
                    return iced::widget::operation::focus(crate::view::search_input_id());
                }
            }
            Message::TogglePins => {
                self.pins_open = !self.pins_open;
                self.overflow_menu_open = false;
                if self.pins_open {
                    if let Some(room_id) = self.selected_room_id.clone() {
                        let core = self.core.clone();
                        return Task::perform(
                            async move {
                                let client = core.get_client().await.ok()?;
                                fluxchat_core::messages::get_pinned_messages(&client, &room_id).await.ok()
                            },
                            |result| match result {
                                Some(msgs) => Message::PinsLoaded(msgs),
                                None => Message::PinsLoaded(Vec::new()),
                            },
                        );
                    }
                }
            }
            Message::ToggleThreads => {
                self.threads_open = !self.threads_open;
                self.overflow_menu_open = false;
                if self.threads_open {
                    // Collect thread roots from loaded messages.
                    // Find them two ways: messages with thread_count > 0, OR
                    // messages referenced as thread_root by other messages.
                    if let Some(room_id) = &self.selected_room_id {
                        if let Some(msgs) = self.messages_by_room.get(room_id) {
                            // Collect all referenced thread root IDs
                            let mut root_ids: std::collections::HashSet<String> = std::collections::HashSet::new();
                            for m in msgs {
                                if let Some(ref root) = m.thread_root {
                                    root_ids.insert(root.clone());
                                }
                                if m.thread_count.map(|c| c > 0).unwrap_or(false) {
                                    root_ids.insert(m.event_id.clone());
                                }
                            }
                            self.thread_messages = msgs.iter()
                                .filter(|m| root_ids.contains(&m.event_id))
                                .cloned()
                                .collect();
                        }
                    }
                }
            }
            Message::ToggleFiles => {
                self.files_open = !self.files_open;
                self.overflow_menu_open = false;
            }
            Message::ToggleNotices => {
                self.notices_open = !self.notices_open;
                self.overflow_menu_open = false;
            }
            Message::ToggleGifs => {
                self.gifs_open = !self.gifs_open;
                self.emoji_open = false;
                self.overflow_menu_open = false;
                if self.gifs_open {
                    return iced::widget::operation::focus(crate::view::gif_search_id());
                }
            }
            Message::ToggleEmojiPicker => {
                self.emoji_open = !self.emoji_open;
                self.gifs_open = false;
                self.overflow_menu_open = false;
                if self.emoji_open {
                    self.emoji_mode = crate::state::EmojiPickerMode::Insert;
                    self.emoji_query.clear();
                    self.emoji_category = 0;
                    return iced::widget::operation::focus(crate::view::emoji_search_id());
                }
            }
            Message::OpenEmojiReact(event_id) => {
                self.emoji_open = true;
                self.emoji_mode = crate::state::EmojiPickerMode::React(event_id);
                self.emoji_query.clear();
                self.emoji_category = 0;
                self.gifs_open = false;
                self.overflow_menu_open = false;
                return iced::widget::operation::focus(crate::view::emoji_search_id());
            }
            Message::EmojiQueryChanged(query) => {
                self.emoji_query = query;
            }
            Message::EmojiCategoryChanged(idx) => {
                self.emoji_category = idx;
            }
            Message::EmojiSelect(emoji) => {
                // Update frequency
                *self.emoji_freq.entry(emoji.clone()).or_insert(0) += 1;
                if let Ok(json) = serde_json::to_string(&self.emoji_freq) {
                    let _ = self.core.db.set_setting("emoji_freq", &json);
                }
                match &self.emoji_mode {
                    crate::state::EmojiPickerMode::Insert => {
                        self.input_text.push_str(&emoji);
                        self.input_content = iced::widget::text_editor::Content::with_text(&self.input_text);
                        return iced::widget::operation::focus(crate::view::input_id());
                    }
                    crate::state::EmojiPickerMode::React(event_id) => {
                        let eid = event_id.clone();
                        self.emoji_open = false;
                        return self.update(Message::ToggleReaction(eid, emoji));
                    }
                }
            }
            Message::ToggleOverflowMenu => {
                self.overflow_menu_open = !self.overflow_menu_open;
            }
            Message::InvitesRefreshed(invited) => {
                self.invited_rooms = invited;
            }
            Message::RefreshRoomList => {
                self.leaving_rooms.clear();
                let core = self.core.clone();
                return Task::perform(
                    async move {
                        let client = core.get_client().await.ok()?;
                        let rooms = fluxchat_core::rooms::get_rooms(&client).await.ok()?;
                        let spaces = fluxchat_core::rooms::get_spaces(&client).await.ok()?;
                        let invited = fluxchat_core::rooms::get_invited_rooms(&client).await.ok()?;
                        let knocked = fluxchat_core::rooms::get_knocked_rooms(&client).await.ok()?;
                        let (space_tree, space_descendants) = fluxchat_core::rooms::get_space_tree(&client).await;
                        Some((rooms, spaces, invited, knocked, space_tree, space_descendants))
                    },
                    |result| match result {
                        Some((rooms, spaces, invited, knocked, tree, desc)) =>
                            Message::SyncEvent(SyncEvent::RoomListUpdate(rooms, spaces, invited, knocked, tree, desc)),
                        None => Message::Noop,
                    },
                );
            }
            Message::ToggleCreateMenu => {
                self.create_menu_open = !self.create_menu_open;
            }
            Message::DmSearchChanged(query) => {
                self.dm_search = query;
            }
            Message::DmSearchSubmit => {
                let query = self.dm_search.clone();
                if query.is_empty() { return Task::none(); }
                let core = self.core.clone();
                return Task::perform(
                    async move {
                        let client = core.get_client().await.ok()?;
                        fluxchat_core::rooms::search_users(&client, &query, 20).await.ok()
                    },
                    |result| match result {
                        Some(results) => Message::DmSearchResults(results),
                        None => Message::DmSearchResults(Vec::new()),
                    },
                );
            }
            Message::DmSearchResults(mut results) => {
                // If query looks like a user ID and isn't in results, add it
                if self.dm_search.starts_with('@') && self.dm_search.contains(':') {
                    let uid = self.dm_search.clone();
                    if !results.iter().any(|r| r.user_id == uid) {
                        results.insert(0, fluxchat_core::rooms::UserSearchResult {
                            user_id: uid,
                            display_name: None,
                            avatar_url: None,
                        });
                    }
                }
                self.dm_results = results;
            }
            Message::OpenContextMenu(menu) => {
                self.context_menu_position = self.cursor_position;
                self.context_menu = Some(menu);
            }
            Message::CloseContextMenu => {
                self.context_menu = None;
            }
            Message::ContextMenuAction(action, target_id) => {
                self.context_menu = None;
                match action.as_str() {
                    "settings" => {
                        // Select the room first, then open room settings via OpenModal path
                        self.selected_room_id = Some(target_id);
                        if let Some(room) = self.selected_room_id.as_ref()
                            .and_then(|rid| self.rooms.iter().find(|r| r.room_id == *rid))
                        {
                            self.room_settings_name = room.name.clone();
                            self.room_settings_topic = room.topic.clone().unwrap_or_default();
                        }
                        self.modal = Some(crate::state::Modal::RoomSettings);
                    }
                    "invite" => {
                        self.invite_target_id = Some(target_id);
                        self.modal = Some(crate::state::Modal::Invite);
                    }
                    "favourite" => {
                        let core = self.core.clone();
                        let rid = target_id;
                        return Task::perform(
                            async move {
                                let client = core.get_client().await.ok()?;
                                let is_fav = fluxchat_core::rooms::is_room_favourite(&client, &rid).await.ok()?;
                                fluxchat_core::rooms::set_room_favourite(&client, &rid, !is_fav).await.ok()
                            },
                            |_| Message::Noop,
                        );
                    }
                    "leave" => {
                        return self.update(Message::LeaveRoom(target_id));
                    }
                    "mark_read" => {
                        self.read_rooms.insert(target_id);
                    }
                    "space_invite" => {
                        self.invite_target_id = Some(target_id);
                        self.modal = Some(crate::state::Modal::Invite);
                    }
                    "space_details" => {
                        self.space_detail_rooms.clear();
                        self.space_detail_members.clear();
                        self.modal = Some(crate::state::Modal::SpaceDetails { space_id: target_id.clone() });
                        let core = self.core.clone();
                        let sid = target_id;
                        return Task::perform(
                            async move {
                                let client = core.get_client().await.ok()?;
                                let rooms = fluxchat_core::rooms::get_space_rooms(&client, &sid).await.unwrap_or_default();
                                let members = fluxchat_core::messages::get_room_members(&client, &sid).await.unwrap_or_default();
                                Some((rooms, members))
                            },
                            |result| match result {
                                Some((rooms, members)) => Message::SpaceDetailsLoaded(rooms, members),
                                None => Message::Noop,
                            },
                        );
                    }
                    _ => {}
                }
            }
            Message::SetRightPanel(panel) => {
                self.right_panel = panel;
            }

            // --- Modals ---
            Message::OpenModal(modal) => {
                self.create_menu_open = false;
                // Clicking own profile opens settings instead
                if let crate::state::Modal::Profile { ref user_id } = modal {
                    if *user_id == self.user_id {
                        return self.update(Message::OpenModal(crate::state::Modal::Settings {
                            tab: "account".into(),
                        }));
                    }
                }
                // Load devices when opening sessions tab
                let task = if matches!(&modal, crate::state::Modal::Settings { tab } if tab == "sessions") {
                    self.settings_nick_input = self.display_name.clone().unwrap_or_default();
                    self.update(Message::SettingsLoadDevices)
                } else if matches!(&modal, crate::state::Modal::Settings { tab } if tab == "account") {
                    self.settings_nick_input = self.display_name.clone().unwrap_or_default();
                    self.settings_status = None;
                    self.update(Message::SettingsLoadProfile)
                } else if matches!(&modal, crate::state::Modal::Settings { tab } if tab == "security") {
                    self.settings_status = None;
                    self.update(Message::SecurityLoadState)
                } else if matches!(&modal, crate::state::Modal::CreateRoom) {
                    // Pre-select current space
                    self.create_room_space = self.selected_space_id.clone();
                    self.create_room_name.clear();
                    self.create_room_topic.clear();
                    self.create_room_public = false;
                    self.create_room_encrypted = false;
                    Task::none()
                } else if matches!(&modal, crate::state::Modal::RoomSettings) {
                    // Pre-populate with current room data
                    if let Some(room) = self.selected_room_id.as_ref()
                        .and_then(|rid| self.rooms.iter().find(|r| r.room_id == *rid))
                    {
                        self.room_settings_name = room.name.clone();
                        self.room_settings_topic = room.topic.clone().unwrap_or_default();
                    }
                    Task::none()
                } else if let crate::state::Modal::Profile { ref user_id } = modal {
                    // Initialize power input with current power level
                    let power = self.selected_room_id.as_ref()
                        .and_then(|rid| self.members_by_room.get(rid))
                        .and_then(|members| members.iter().find(|m| m.user_id == *user_id))
                        .map(|m| m.power_level)
                        .unwrap_or(0);
                    self.profile_power_input = power.to_string();
                    // Load verification status
                    self.profile_user_verified = None;
                    let core = self.core.clone();
                    let uid = user_id.clone();
                    let uid2 = user_id.clone();
                    Task::perform(
                        async move {
                            let client = core.get_client().await.ok()?;
                            fluxchat_core::verification::is_user_verified(&client, &uid).await.ok()
                        },
                        move |result| match result {
                            Some(verified) => Message::UserVerifiedStatus(uid2, verified),
                            None => Message::Noop,
                        },
                    )
                } else if matches!(&modal, crate::state::Modal::CreateDm) {
                    self.dm_search.clear();
                    self.dm_results.clear();
                    Task::none()
                } else if matches!(&modal, crate::state::Modal::Report { .. }) {
                    self.report_reason.clear();
                    Task::none()
                } else if matches!(&modal, crate::state::Modal::Directory) {
                    self.directory_results.clear();
                    self.directory_next_batch = None;
                    self.directory_loading = true;
                    // Defer to DirectorySearch which handles MRS vs federation
                    self.modal = Some(modal);
                    return self.update(Message::DirectorySearch);
                } else if matches!(&modal, crate::state::Modal::DevTools) {
                    self.devtools_state_events.clear();
                    self.devtools_tab = "state".into();
                    if let Some(room_id) = self.selected_room_id.clone() {
                        let core = self.core.clone();
                        Task::perform(
                            async move {
                                let client = core.get_client().await.ok()?;
                                fluxchat_core::rooms::get_room_state_events(&client, &room_id).await.ok()
                            },
                            |result| match result {
                                Some(events) => Message::DevToolsLoaded(events),
                                None => Message::Noop,
                            },
                        )
                    } else {
                        Task::none()
                    }
                } else {
                    Task::none()
                };
                self.modal = Some(modal);
                return task;
            }
            Message::StartVerification(user_id) => {
                let core = self.core.clone();
                let tx = self.timeline_tx.clone();
                let tx2 = self.timeline_tx.clone();
                // Show a "waiting" verification modal immediately
                self.modal = Some(crate::state::Modal::Verification {
                    info: fluxchat_core::VerificationInfo {
                        flow_id: String::new(),
                        other_user_id: user_id.clone(),
                        is_self_verification: false,
                        state: "requesting...".into(),
                        emojis: None,
                    },
                });
                return Task::perform(
                    async move {
                        let client = core.get_client().await.ok()?;
                        fluxchat_core::verification::request_verification(
                            &client, &user_id,
                            move |info| {
                                let _ = tx.send(Message::SyncEvent(SyncEvent::VerificationRequest(info)));
                            },
                            move || {
                                let _ = tx2.send(Message::CloseModal);
                            },
                        ).await.ok()
                    },
                    |_| Message::Noop,
                );
            }
            Message::StartDeviceVerification(device_id) => {
                let core = self.core.clone();
                let tx = self.timeline_tx.clone();
                let tx2 = self.timeline_tx.clone();
                self.modal = Some(crate::state::Modal::Verification {
                    info: fluxchat_core::VerificationInfo {
                        flow_id: String::new(),
                        other_user_id: self.user_id.clone(),
                        is_self_verification: true,
                        state: "requesting...".into(),
                        emojis: None,
                    },
                });
                return Task::perform(
                    async move {
                        let client = core.get_client().await.ok()?;
                        fluxchat_core::verification::request_device_verification(
                            &client, &device_id,
                            move |info| {
                                let _ = tx.send(Message::SyncEvent(SyncEvent::VerificationRequest(info)));
                            },
                            move || {
                                let _ = tx2.send(Message::CloseModal);
                            },
                        ).await.ok()
                    },
                    |_| Message::Noop,
                );
            }
            Message::VerificationAccept(user_id, flow_id) => {
                let core = self.core.clone();
                let tx = self.timeline_tx.clone();
                let tx2 = self.timeline_tx.clone();
                // Update modal state to show we're accepting
                if let Some(crate::state::Modal::Verification { ref mut info }) = self.modal {
                    info.state = "accepting...".into();
                }
                return Task::perform(
                    async move {
                        let client = core.get_client().await.ok()?;
                        fluxchat_core::verification::accept_verification(
                            &client, &user_id, &flow_id,
                            move |info| {
                                let _ = tx.send(Message::SyncEvent(SyncEvent::VerificationRequest(info)));
                            },
                            move || {
                                let _ = tx2.send(Message::CloseModal);
                            },
                        ).await.ok()
                    },
                    |_| Message::Noop,
                );
            }
            Message::VerificationConfirm(user_id, flow_id) => {
                if let Some(crate::state::Modal::Verification { ref mut info }) = self.modal {
                    info.state = "confirming...".into();
                }
                let core = self.core.clone();
                return Task::perform(
                    async move {
                        let client = core.get_client().await.ok()?;
                        fluxchat_core::verification::confirm_verification(&client, &user_id, &flow_id).await.ok()
                    },
                    |_| Message::CloseModal,
                );
            }
            Message::VerificationCancel(user_id, flow_id) => {
                self.modal = None;
                let core = self.core.clone();
                return Task::perform(
                    async move {
                        let client = core.get_client().await.ok()?;
                        fluxchat_core::verification::cancel_verification(&client, &user_id, &flow_id).await.ok()
                    },
                    |_| Message::Noop,
                );
            }
            Message::CloseModal => {
                self.modal = None;
            }

            // --- Prefs ---
            Message::SetTimestampFormat(fmt) => {
                self.timestamp_format = fmt.clone();
                let _ = self.core.db.set_setting("timestamp_format", &fmt);
            }
            Message::ToggleCompactMode => {
                self.compact_mode = !self.compact_mode;
                let _ = self.core.db.set_setting(
                    "compact_mode",
                    if self.compact_mode { "true" } else { "false" },
                );
            }
            Message::ToggleJoinPart => {
                self.show_joinpart = !self.show_joinpart;
                let _ = self.core.db.set_setting(
                    "show_joinpart",
                    if self.show_joinpart { "true" } else { "false" },
                );
            }
            Message::SetZoomLevel(level) => {
                self.zoom_level = level;
                let _ = self.core.db.set_setting("zoom_level", &level.to_string());
            }
            Message::SetMsgFontSize(size) => {
                self.msg_font_size = size.clamp(10.0, 24.0);
                let _ = self.core.db.set_setting("msg_font_size", &self.msg_font_size.to_string());
            }
            Message::SetMsgFontFamily(name) => {
                self.msg_font_family = name.clone();
                let _ = self.core.db.set_setting("msg_font_family", &name);
            }
            Message::SetSidebarFontFamily(name) => {
                self.sidebar_font_family = name.clone();
                let _ = self.core.db.set_setting("sidebar_font_family", &name);
            }
            Message::SetTheme(name) => {
                if let Some(theme) = self.available_themes.iter().find(|t| t.name == name) {
                    crate::theme::set_active_theme(theme.clone());
                    self.active_theme_name = name.clone();
                    let _ = self.core.db.set_setting("theme", &name);
                }
            }

            // --- Notifications ---
            Message::SetNotificationsEnabled(enabled) => {
                self.notifications_enabled = enabled;
                let _ = self.core.db.set_setting("notifications_enabled", if enabled { "true" } else { "false" });
            }
            Message::SetNotifyMentionsOnly(val) => {
                self.notify_mentions_only = val;
                let _ = self.core.db.set_setting("notify_mentions_only", if val { "true" } else { "false" });
            }
            Message::SetNotifyShowPreview(val) => {
                self.notify_show_preview = val;
                let _ = self.core.db.set_setting("notify_show_preview", if val { "true" } else { "false" });
            }
            Message::SetNotifySound(val) => {
                self.notify_sound = val;
                let _ = self.core.db.set_setting("notify_sound", if val { "true" } else { "false" });
            }
            Message::WindowFocusChanged(focused) => {
                self.app_focused = focused;
            }

            // --- Room commands ---
            Message::JoinRoom(room_or_alias) => {
                let core = self.core.clone();
                return Task::perform(
                    async move {
                        let client = core.get_client().await.ok()?;
                        let result = fluxchat_core::rooms::join_room(&client, &room_or_alias).await.ok()?;
                        tokio::time::sleep(std::time::Duration::from_millis(500)).await;
                        Some(result)
                    },
                    |result| match result {
                        Some(room) => Message::SelectRoom(room.room_id),
                        None => Message::Noop,
                    },
                ).chain(Task::done(Message::RefreshRoomList));
            }
            Message::LeaveRoom(room_id) => {
                self.modal = None;
                self.open_tabs.retain(|(id, _, _, _)| id != &room_id);
                if self.selected_room_id.as_deref() == Some(&room_id) {
                    self.selected_room_id = self.open_tabs.last().map(|(id, _, _, _)| id.clone());
                }
                // Remove from sidebar immediately and mark as leaving
                self.leaving_rooms.insert(room_id.clone());
                self.rooms.retain(|r| r.room_id != room_id);
                self.messages_by_room.remove(&room_id);
                self.members_by_room.remove(&room_id);
                self.thread_counts.remove(&room_id);
                let core = self.core.clone();
                return Task::perform(
                    async move {
                        let client = core.get_client().await.ok()?;
                        fluxchat_core::rooms::leave_room(&client, &room_id).await.ok()?;
                        tokio::time::sleep(std::time::Duration::from_millis(500)).await;
                        Some(())
                    },
                    |_| Message::RefreshRoomList,
                );
            }
            Message::SpaceInvite(space_id) => {
                self.invite_target_id = Some(space_id);
                self.modal = Some(crate::state::Modal::Invite);
            }
            Message::SpaceDetailsLoaded(rooms, members) => {
                self.space_detail_rooms = rooms;
                self.space_detail_members = members;
            }
            Message::InviteUser(room_id, user_id) => {
                let core = self.core.clone();
                return Task::perform(
                    async move {
                        let client = core.get_client().await.ok()?;
                        fluxchat_core::rooms::invite_user(&client, &room_id, &user_id).await.ok()
                    },
                    |_| Message::Noop,
                );
            }
            Message::KickUser(room_id, user_id, reason) => {
                let core = self.core.clone();
                return Task::perform(
                    async move {
                        let client = core.get_client().await.ok()?;
                        fluxchat_core::rooms::kick_user(&client, &room_id, &user_id, reason.as_deref()).await.ok()
                    },
                    |_| Message::Noop,
                );
            }
            Message::BanUser(room_id, user_id, reason) => {
                let core = self.core.clone();
                return Task::perform(
                    async move {
                        let client = core.get_client().await.ok()?;
                        fluxchat_core::rooms::ban_user(&client, &room_id, &user_id, reason.as_deref()).await.ok()
                    },
                    |_| Message::Noop,
                );
            }
            Message::UnbanUser(user_input) => {
                if let Some(room_id) = self.selected_room_id.clone() {
                    let core = self.core.clone();
                    // Resolve display name to user_id via members list
                    let user_id = if user_input.starts_with('@') {
                        user_input
                    } else if let Some(members) = self.members_by_room.get(&room_id) {
                        let lower = user_input.to_lowercase();
                        members.iter().find(|m| m.display_name.to_lowercase() == lower)
                            .map(|m| m.user_id.clone())
                            .unwrap_or(user_input)
                    } else {
                        user_input
                    };
                    return Task::perform(
                        async move {
                            let client = core.get_client().await.ok()?;
                            fluxchat_core::rooms::unban_user(&client, &room_id, &user_id).await.ok()
                        },
                        |_| Message::Noop,
                    );
                }
            }
            Message::SetRoomName(name) => {
                if let Some(room_id) = self.selected_room_id.clone() {
                    let core = self.core.clone();
                    return Task::perform(
                        async move {
                            let client = core.get_client().await.ok()?;
                            fluxchat_core::rooms::set_room_name(&client, &room_id, &name).await.ok()
                        },
                        |_| Message::Noop,
                    );
                }
            }
            Message::SendHtml(html) => {
                if let Some(room_id) = self.selected_room_id.clone() {
                    let core = self.core.clone();
                    return Task::perform(
                        async move {
                            let client = core.get_client().await.ok()?;
                            fluxchat_core::messages::send_html(&client, &room_id, &html).await.ok()
                        },
                        |_| Message::Noop,
                    );
                }
            }
            Message::SendPlain(text) => {
                if let Some(room_id) = self.selected_room_id.clone() {
                    let core = self.core.clone();
                    return Task::perform(
                        async move {
                            let client = core.get_client().await.ok()?;
                            fluxchat_core::messages::send_plain(&client, &room_id, &text).await.ok()
                        },
                        |_| Message::Noop,
                    );
                }
            }
            Message::UpgradeRoom(version) => {
                if let Some(room_id) = self.selected_room_id.clone() {
                    let core = self.core.clone();
                    return Task::perform(
                        async move {
                            let client = core.get_client().await.ok()?;
                            fluxchat_core::rooms::upgrade_room(&client, &room_id, &version).await.ok()
                        },
                        |_| Message::Noop,
                    );
                }
            }
            Message::DevToolsLoaded(events) => {
                self.devtools_state_events = events;
            }
            Message::DevToolsTabChanged(tab) => {
                self.devtools_tab = tab;
            }
            Message::DevToolsSendTypeChanged(t) => {
                self.devtools_send_type = t;
            }
            Message::DevToolsSendStateKeyChanged(k) => {
                self.devtools_send_state_key = k;
            }
            Message::DevToolsSendBodyChanged(b) => {
                self.devtools_send_body = b;
            }
            Message::DevToolsToggleIsState => {
                self.devtools_is_state = !self.devtools_is_state;
            }
            Message::DevToolsSend => {
                if let Some(room_id) = self.selected_room_id.clone() {
                    let core = self.core.clone();
                    let event_type = self.devtools_send_type.clone();
                    let body = self.devtools_send_body.clone();
                    let state_key = if self.devtools_is_state {
                        Some(self.devtools_send_state_key.clone())
                    } else {
                        None
                    };
                    return Task::perform(
                        async move {
                            let client = core.get_client().await.ok()?;
                            fluxchat_core::rooms::send_raw_event(
                                &client, &room_id, &event_type,
                                state_key.as_deref(), &body,
                            ).await.ok()
                        },
                        |_| Message::Noop,
                    );
                }
            }
            Message::DirectorySearchChanged(q) => {
                self.directory_search = q;
            }
            Message::DirectoryServerChanged(s) => {
                self.directory_server = s;
                return self.update(Message::DirectorySearch);
            }
            Message::DirectorySearch => {
                self.directory_results.clear();
                self.directory_next_batch = None;
                self.directory_loading = true;
                let core = self.core.clone();
                let query = self.directory_search.clone();
                let server = self.directory_server.clone();
                return Task::perform(
                    async move {
                        let client = core.get_client().await.ok()?;
                        let search = if query.is_empty() { None } else { Some(query.as_str()) };
                        let srv = if server.is_empty() { None } else { Some(server.as_str()) };
                        fluxchat_core::rooms::list_public_rooms(&client, search, Some(20), None, srv).await.ok()
                    },
                    |result| match result {
                        Some((rooms, next)) => Message::DirectoryResults(rooms, next),
                        None => Message::DirectoryResults(vec![], None),
                    },
                );
            }
            Message::DirectoryResults(rooms, next_batch) => {
                self.directory_loading = false;
                let mxc_uris: Vec<String> = rooms.iter()
                    .filter_map(|r| r.avatar_url.clone())
                    .collect();
                self.directory_results.extend(rooms);
                self.directory_next_batch = next_batch;
                if !mxc_uris.is_empty() {
                    return self.resolve_avatars(mxc_uris);
                }
            }
            Message::DirectoryLoadMore => {
                if let Some(since) = self.directory_next_batch.clone() {
                    self.directory_loading = true;
                    let core = self.core.clone();
                    let query = self.directory_search.clone();
                    let server = self.directory_server.clone();
                    return Task::perform(
                        async move {
                            let client = core.get_client().await.ok()?;
                            let search = if query.is_empty() { None } else { Some(query.as_str()) };
                            let srv = if server.is_empty() { None } else { Some(server.as_str()) };
                            fluxchat_core::rooms::list_public_rooms(&client, search, Some(20), Some(&since), srv).await.ok()
                        },
                        |result| match result {
                            Some((rooms, next)) => Message::DirectoryResults(rooms, next),
                            None => Message::DirectoryResults(vec![], None),
                        },
                    );
                }
            }
            Message::DirectoryJoinRoom(room_id) => {
                let core = self.core.clone();
                return Task::perform(
                    async move {
                        let client = core.get_client().await.ok()?;
                        fluxchat_core::rooms::join_room(&client, &room_id).await.ok()
                    },
                    |_| Message::CloseModal,
                );
            }
            Message::SetTopic(room_id, topic) => {
                let core = self.core.clone();
                return Task::perform(
                    async move {
                        let client = core.get_client().await.ok()?;
                        fluxchat_core::rooms::set_room_topic(&client, &room_id, &topic).await.ok()
                    },
                    |_| Message::Noop,
                );
            }
            Message::SetNick(name) => {
                self.display_name = Some(name.clone());
                let core = self.core.clone();
                return Task::perform(
                    async move {
                        let client = core.get_client().await.ok()?;
                        fluxchat_core::profile::set_display_name(&client, &name).await.ok()
                    },
                    |_| Message::Noop,
                );
            }
            Message::SendDm(user_id, body) => {
                let core = self.core.clone();
                return Task::perform(
                    async move {
                        let client = core.get_client().await.ok()?;
                        fluxchat_core::rooms::send_dm(&client, &user_id, &body).await.ok()
                    },
                    |_| Message::Noop,
                );
            }

            // --- File upload ---
            Message::PickFile => {
                return Task::perform(
                    async {
                        let handle = rfd::AsyncFileDialog::new()
                            .set_title("Upload file")
                            .pick_file()
                            .await?;
                        let name = handle.file_name();
                        let data = handle.read().await;
                        let mime = mime_from_extension(&name);
                        Some((name, mime, data))
                    },
                    |result| match result {
                        Some((name, mime, data)) => Message::FileSelected(name, mime, data),
                        None => Message::Noop,
                    },
                );
            }
            Message::FileDropped(path) => {
                return Task::perform(
                    async move {
                        let data = tokio::fs::read(&path).await.ok()?;
                        let name = path.file_name()?.to_string_lossy().to_string();
                        let mime = mime_from_extension(&name);
                        Some((name, mime, data))
                    },
                    |result| match result {
                        Some((name, mime, data)) => Message::FileSelected(name, mime, data),
                        None => Message::Noop,
                    },
                );
            }
            Message::FileSelected(name, mime, data) => {
                self.upload_pending = Some((name, mime, data));
            }
            Message::CancelUpload => {
                self.upload_pending = None;
            }
            Message::UploadFile => {
                if let Some((name, mime, data)) = self.upload_pending.take() {
                    if let Some(room_id) = self.selected_room_id.clone() {
                        let core = self.core.clone();
                        return Task::perform(
                            async move {
                                let client = core.get_client().await.ok()?;
                                fluxchat_core::messages::upload_file(
                                    &client, &room_id, &name, &mime, data,
                                )
                                .await
                                .ok()
                            },
                            |_| Message::Noop,
                        );
                    }
                }
            }

            // --- Settings / Security / Profile / Power levels ---
            msg @ (Message::SettingsNickChanged(_) | Message::SettingsSaveNick
                | Message::SettingsLoadDevices | Message::SettingsDevicesLoaded(_)
                | Message::SettingsSignOutDevice(_) | Message::SettingsSignOutPasswordChanged(_)
                | Message::SettingsSignOutConfirm | Message::SettingsStatus(_)
                | Message::SettingsLoadProfile | Message::SettingsProfileLoaded { .. }
                | Message::SettingsUploadAvatar | Message::SettingsRemoveAvatar
                | Message::SettingsPickBanner | Message::SettingsBannerPicked(_)
                | Message::SettingsSaveBanner | Message::SettingsRemoveBanner
                | Message::SettingsBannerOffsetChanged(_)
                | Message::SettingsOldPasswordChanged(_) | Message::SettingsNewPasswordChanged(_)
                | Message::SettingsConfirmPasswordChanged(_) | Message::SettingsChangePassword
                | Message::RotateRoomKeys(_) | Message::RotateRoomKeysResult(_)
                | Message::SecurityLoadState | Message::SecurityStateLoaded(_)
                | Message::SecurityEnableBackup | Message::SecurityBackupEnabled(_)
                | Message::SecurityRecoveryKeyInputChanged(_) | Message::SecurityRecover
                | Message::SecurityRecoverResult(_)
                | Message::SecurityResetIdentityConfirm | Message::SecurityResetIdentityCancel
                | Message::SecurityResetPasswordChanged(_) | Message::SecurityResetIdentitySubmit
                | Message::SecurityResetIdentityResult(_)
                | Message::ToggleIgnoreUser(_) | Message::ProfilePowerInputChanged(_)
                | Message::ProfileSetPowerLevel | Message::SendDmToUser(_)
                | Message::DmCreated(_)
                | Message::ManualVerifyUser(_) | Message::ManualVerifyResult(_, _)
                | Message::UserVerifiedStatus(_, _)
                | Message::SetPowerLevel(_, _, _)) => {
                if let Some(task) = self.handle_settings_message(msg) {
                    return task;
                }
            }

            // --- Avatars ---
            Message::AvatarsResolved(entries) => {
                for (mxc, data) in entries {
                    let handle = iced::widget::image::Handle::from_bytes(data);
                    self.avatar_cache.insert(mxc, handle);
                }
                evict_cache(&mut self.avatar_cache, AVATAR_CACHE_LIMIT);
            }

            // --- Inline images ---
            Message::ImageResolved(mxc, data) => {
                self.image_cache_pending.remove(&mxc);
                // Persist to SQLite
                use base64::Engine;
                let b64 = base64::engine::general_purpose::STANDARD.encode(&data);
                self.core.db.cache_media(&format!("iced_img:{mxc}"), &b64);
                // Try to decode as animated GIF (native) or animated WebP (via transcode).
                let is_gif = data.len() >= 3 && &data[..3] == b"GIF";
                let is_webp = data.len() >= 12
                    && &data[..4] == b"RIFF" && &data[8..12] == b"WEBP";
                if is_gif || is_webp {
                    let mxc_clone = mxc.clone();
                    let data_clone = data.clone();
                    let core_task = self.core.clone();
                    let handle = iced::widget::image::Handle::from_bytes(data);
                    self.image_cache.insert(mxc.clone(), handle);
                    return Task::perform(
                        async move {
                            let mxc_log = mxc_clone.clone();
                            let mxc_inner = mxc_clone.clone();
                            let frames = match tokio::task::spawn_blocking(move || {
                                let gif_bytes = if is_webp {
                                    let gb = webp_to_gif_bytes(&data_clone)?;
                                    // Cache the transcoded GIF so we don't repeat
                                    // the expensive NeuQuant quantization on restart.
                                    let encoded = base64::engine::general_purpose::STANDARD.encode(&gb);
                                    core_task.db.cache_media(&format!("iced_gif:{mxc_inner}"), &encoded);
                                    gb
                                } else {
                                    data_clone
                                };
                                match iced_gif::Frames::from_bytes(gif_bytes) {
                                    Ok(f) => Some(f),
                                    Err(e) => {
                                        tracing::warn!("iced_gif Frames::from_bytes failed: {e}");
                                        None
                                    }
                                }
                            }).await {
                                Ok(Some(f)) => f,
                                Ok(None) => return None,
                                Err(e) => {
                                    tracing::warn!("transcode task for {mxc_log} failed: {e}");
                                    return None;
                                }
                            };
                            Some((mxc_clone, std::sync::Arc::new(frames)))
                        },
                        |result| match result {
                            Some((mxc, frames)) => Message::GifFramesResolved(mxc, frames),
                            None => Message::Noop,
                        },
                    );
                }
                let handle = iced::widget::image::Handle::from_bytes(data);
                self.image_cache.insert(mxc, handle);
                evict_cache(&mut self.image_cache, IMAGE_CACHE_LIMIT);
            }
            Message::GifFramesResolved(mxc, frames) => {
                self.gif_frames_cache.insert(mxc, frames);
                evict_cache(&mut self.gif_frames_cache, GIF_CACHE_LIMIT);
            }

            // --- URL Previews ---
            Message::UrlPreviewFetched(url, preview) => {
                self.url_previews_pending.remove(&url);
                if let Some(preview) = preview {
                    // Fetch og:image if present
                    let task = if let Some(ref image_url) = preview.image_url {
                        if !self.url_preview_images.contains_key(image_url) && image_url.starts_with("mxc://") {
                            let core = self.core.clone();
                            let img_url = image_url.clone();
                            Task::perform(
                                async move {
                                    use matrix_sdk::media::{MediaRequestParameters, MediaFormat};
                                    use matrix_sdk::ruma::events::room::MediaSource;
                                    use matrix_sdk::ruma::MxcUri;
                                    let client = core.get_client().await.ok()?;
                                    let mxc_uri = <&MxcUri>::from(img_url.as_str());
                                    let req = MediaRequestParameters {
                                        source: MediaSource::Plain(mxc_uri.to_owned()),
                                        format: MediaFormat::File,
                                    };
                                    let data = client.media().get_media_content(&req, true).await.ok()?;
                                    Some((img_url, data))
                                },
                                |result| match result {
                                    Some((url, data)) => Message::UrlPreviewImageResolved(url, data),
                                    None => Message::Noop,
                                },
                            )
                        } else {
                            Task::none()
                        }
                    } else {
                        Task::none()
                    };
                    self.url_previews.insert(url, preview);
                    evict_cache(&mut self.url_previews, URL_PREVIEW_LIMIT);
                    return task;
                }
            }
            Message::UrlPreviewImageResolved(url, data) => {
                let handle = iced::widget::image::Handle::from_bytes(data);
                self.url_preview_images.insert(url, handle);
                evict_cache(&mut self.url_preview_images, URL_PREVIEW_LIMIT);
            }

            // --- Message hover ---
            Message::HoverMessage(id) => {
                self.hovered_message = id;
            }
            Message::TogglePreviewExpand(event_id) => {
                if !self.expanded_previews.remove(&event_id) {
                    self.expanded_previews.insert(event_id);
                }
            }

            // --- Formatting ---
            Message::InsertFormat(prefix, suffix) => {
                self.input_text.push_str(prefix);
                self.input_text.push_str(suffix);
                self.input_content = iced::widget::text_editor::Content::with_text(&self.input_text);
                return iced::widget::operation::focus(crate::view::input_id());
            }

            // --- Overflow menu actions ---
            Message::ToggleFavourite => {
                self.overflow_menu_open = false;
                if let Some(room_id) = self.selected_room_id.clone() {
                    // Toggle local state immediately for snappy UI
                    if let Some(room) = self.rooms.iter_mut().find(|r| r.room_id == room_id) {
                        room.is_favourite = !room.is_favourite;
                    }
                    let core = self.core.clone();
                    let new_fav = self.rooms.iter().find(|r| r.room_id == room_id)
                        .map(|r| r.is_favourite).unwrap_or(false);
                    return Task::perform(
                        async move {
                            let client = core.get_client().await.ok()?;
                            fluxchat_core::rooms::set_room_favourite(&client, &room_id, new_fav).await.ok()
                        },
                        |_| Message::Noop,
                    );
                }
            }
            Message::CopyRoomLink => {
                self.overflow_menu_open = false;
                if let Some(room_id) = &self.selected_room_id {
                    let link = format!("https://matrix.to/#/{room_id}");
                    return iced::clipboard::write(link);
                }
            }

            // --- Invites ---
            Message::AcceptInvite(room_id) => {
                // Remove from invite list immediately for snappy UI
                self.invited_rooms.retain(|r| r.room_id != room_id);
                let core = self.core.clone();
                let rid = room_id.clone();
                return Task::perform(
                    async move {
                        let client = core.get_client().await.ok()?;
                        match fluxchat_core::rooms::accept_invite(&client, &rid).await {
                            Ok(()) => {
                                // Brief delay for server to process the join
                                tokio::time::sleep(std::time::Duration::from_millis(500)).await;
                                Some(rid)
                            }
                            Err(e) => {
                                tracing::warn!("Failed to accept invite for {rid}: {e}");
                                None
                            }
                        }
                    },
                    |result| match result {
                        Some(room_id) => Message::SelectRoom(room_id),
                        None => Message::Noop,
                    },
                ).chain(Task::done(Message::RefreshRoomList));
            }
            Message::RejectInvite(room_id) => {
                // Remove from invite list immediately for snappy UI
                self.invited_rooms.retain(|r| r.room_id != room_id);
                let core = self.core.clone();
                let rid = room_id.clone();
                return Task::perform(
                    async move {
                        let client = core.get_client().await.ok()?;
                        if let Err(e) = fluxchat_core::rooms::reject_invite(&client, &rid).await {
                            tracing::warn!("Failed to reject invite for {rid}: {e}");
                        }
                        tokio::time::sleep(std::time::Duration::from_millis(500)).await;
                        Some(())
                    },
                    |_| Message::RefreshRoomList,
                );
            }

            // --- Knocking ---
            Message::KnockRoom(room_id_or_alias, reason) => {
                let core = self.core.clone();
                return Task::perform(
                    async move {
                        let client = core.get_client().await.ok()?;
                        fluxchat_core::rooms::knock_room(&client, &room_id_or_alias, reason.as_deref()).await.ok()
                    },
                    |_| Message::Noop,
                );
            }
            Message::CancelKnock(room_id) => {
                let core = self.core.clone();
                return Task::perform(
                    async move {
                        let client = core.get_client().await.ok()?;
                        fluxchat_core::rooms::cancel_knock(&client, &room_id).await.ok()
                    },
                    |_| Message::Noop,
                );
            }

            // --- Audio playback ---
            Message::PlayAudio(event_id, room_id) => {
                // If already playing this audio, stop it
                if self.audio_playing.as_deref() == Some(&event_id) {
                    return self.update(Message::StopAudio);
                }
                // Stop any current playback first
                if self.audio_playing.is_some() {
                    let _ = self.update(Message::StopAudio);
                }
                // Get duration from the message
                let duration = self.messages_by_room
                    .get(&room_id)
                    .and_then(|msgs| msgs.iter().find(|m| m.event_id == event_id))
                    .and_then(|m| m.duration_ms)
                    .unwrap_or(5000);
                self.audio_playing = Some(event_id.clone());
                self.audio_start_time = Some(std::time::Instant::now());
                self.audio_duration_ms = duration;
                self.audio_progress = 0.0;

                let core = self.core.clone();
                let tx = self.timeline_tx.clone();
                let real_room = if room_id.starts_with("thread::") {
                    room_id.splitn(3, "::").nth(1).unwrap_or(&room_id).to_string()
                } else {
                    room_id
                };
                return Task::perform(
                    async move {
                        let client = core.get_client().await.ok()?;
                        let (bytes, _mime) = fluxchat_core::media::get_event_media_bytes(
                            &client, &real_room, &event_id,
                        ).await.ok()?;
                        // Write to temp file and play with system audio player
                        let ext = if _mime.contains("ogg") { "ogg" }
                            else if _mime.contains("mp3") || _mime.contains("mpeg") { "mp3" }
                            else if _mime.contains("wav") { "wav" }
                            else if _mime.contains("mp4") || _mime.contains("m4a") { "m4a" }
                            else { "ogg" };
                        let path = std::env::temp_dir().join(format!("fluxchat_audio.{ext}"));
                        std::fs::write(&path, &bytes).ok()?;

                        let tx2 = tx.clone();
                        std::thread::spawn(move || {
                            #[cfg(target_os = "macos")]
                            let mut child = {
                                std::process::Command::new("ffplay")
                                    .args(["-nodisp", "-autoexit", "-loglevel", "quiet"])
                                    .arg(&path)
                                    .spawn()
                                    .or_else(|_| std::process::Command::new("afplay").arg(&path).spawn())
                                    .or_else(|_| std::process::Command::new("mpv").arg("--no-video").arg(&path).spawn())
                            };
                            #[cfg(target_os = "linux")]
                            let mut child = {
                                std::process::Command::new("ffplay")
                                    .args(["-nodisp", "-autoexit", "-loglevel", "quiet"])
                                    .arg(&path)
                                    .spawn()
                                    .or_else(|_| std::process::Command::new("pw-play").arg(&path).spawn())
                                    .or_else(|_| std::process::Command::new("paplay").arg(&path).spawn())
                                    .or_else(|_| std::process::Command::new("mpv").arg("--no-video").arg(&path).spawn())
                            };
                            #[cfg(target_os = "windows")]
                            let mut child = {
                                std::process::Command::new("cmd")
                                    .args(["/C", "start", "/wait", "", &path.to_string_lossy()])
                                    .spawn()
                            };
                            if let Ok(ref mut c) = child {
                                let _ = c.wait();
                            }
                            let _ = std::fs::remove_file(&path);
                            let _ = tx2.send(Message::AudioFinished);
                        });

                        Some(())
                    },
                    |_| Message::Noop,
                );
            }
            Message::StopAudio => {
                // Kill audio player processes playing our temp file
                #[cfg(unix)]
                { let _ = std::process::Command::new("pkill").arg("-f").arg("fluxchat_audio").status(); }
                #[cfg(windows)]
                { let _ = std::process::Command::new("taskkill").args(["/IM", "cmd.exe", "/F"]).status(); }
                self.audio_playing = None;
                self.audio_progress = 0.0;
                self.audio_start_time = None;
                self.audio_pid = None;
            }
            Message::DownloadFile(event_id, room_id) => {
                // Get the filename from the message
                let filename = self.messages_by_room
                    .get(&room_id)
                    .and_then(|msgs| msgs.iter().find(|m| m.event_id == event_id))
                    .map(|m| m.body.clone())
                    .unwrap_or_else(|| "file".to_string());
                let core = self.core.clone();
                let real_room = if room_id.starts_with("thread::") {
                    room_id.splitn(3, "::").nth(1).unwrap_or(&room_id).to_string()
                } else {
                    room_id
                };
                return Task::perform(
                    async move {
                        let client = core.get_client().await.ok()?;
                        let (data, _mime) = fluxchat_core::media::get_event_media_bytes(&client, &real_room, &event_id).await.ok()?;
                        // Open save dialog
                        let handle = rfd::AsyncFileDialog::new()
                            .set_file_name(&filename)
                            .save_file()
                            .await?;
                        handle.write(&data).await.ok()?;
                        Some(())
                    },
                    |_| Message::Noop,
                );
            }
            Message::PlayVideo(event_id, room_id) => {
                let core = self.core.clone();
                let real_room = if room_id.starts_with("thread::") {
                    room_id.splitn(3, "::").nth(1).unwrap_or(&room_id).to_string()
                } else {
                    room_id
                };
                let filename = self.messages_by_room
                    .get(&real_room)
                    .and_then(|msgs| msgs.iter().find(|m| m.event_id == event_id))
                    .map(|m| m.body.clone())
                    .unwrap_or_else(|| "video.mp4".to_string());
                let eid = event_id.clone();
                return Task::perform(
                    async move {
                        // Lazy GStreamer init (safe to call multiple times)
                        if let Err(e) = gstreamer::init() {
                            tracing::warn!("GStreamer init failed: {e}");
                            return None;
                        }
                        let client = core.get_client().await.ok()?;
                        let (data, _mime) = fluxchat_core::media::get_event_media_bytes(&client, &real_room, &eid).await.ok()?;
                        let tmp = std::env::temp_dir().join(format!("fluxchat_video_{filename}"));
                        std::fs::write(&tmp, &data).ok()?;
                        Some((event_id, tmp))
                    },
                    |result| match result {
                        Some((eid, path)) => Message::VideoReady(eid, path),
                        None => Message::Noop,
                    },
                );
            }
            Message::VideoReady(event_id, path) => {
                match url::Url::from_file_path(&path) {
                    Ok(file_url) => {
                        match create_video_player(&file_url) {
                            Ok(mut video) => {
                                video.set_volume(1.0);
                                self.video_player = Some(video);
                                self.video_playing_event = Some(event_id);
                                self.video_temp_path = Some(path);
                                self.video_position = 0.0;
                                self.video_dragging = false;
                            }
                            Err(e) => {
                                tracing::warn!("Failed to create video player: {e}");
                                let _ = open::that(&path);
                            }
                        }
                    }
                    Err(_) => {
                        tracing::warn!("Invalid video file path");
                        let _ = open::that(&path);
                    }
                }
            }
            Message::VideoTogglePause => {
                if let Some(ref mut video) = self.video_player {
                    video.set_paused(!video.paused());
                }
            }
            Message::VideoSeek(secs) => {
                self.video_dragging = true;
                self.video_position = secs;
                if let Some(ref mut video) = self.video_player {
                    video.set_paused(true);
                }
            }
            Message::VideoSeekRelease => {
                self.video_dragging = false;
                if let Some(ref mut video) = self.video_player {
                    let _ = video.seek(std::time::Duration::from_secs_f64(self.video_position), false);
                    video.set_paused(false);
                }
            }
            Message::VideoNewFrame => {
                if !self.video_dragging {
                    if let Some(ref video) = self.video_player {
                        self.video_position = video.position().as_secs_f64();
                    }
                }
            }
            Message::VideoEndOfStream => {
                if let Some(ref mut video) = self.video_player {
                    video.set_paused(true);
                }
            }
            Message::VideoClose => {
                self.video_player = None;
                self.video_playing_event = None;
                self.video_position = 0.0;
                self.video_dragging = false;
                if let Some(path) = self.video_temp_path.take() {
                    let _ = std::fs::remove_file(&path);
                }
            }
            Message::VideoThumbResolved(key, data, w, h) => {
                self.image_cache_pending.remove(&key);
                use base64::Engine;
                let b64 = base64::engine::general_purpose::STANDARD.encode(&data);
                self.core.db.cache_media(&format!("iced_img:{key}"), &b64);
                let handle = iced::widget::image::Handle::from_bytes(data);
                self.image_cache.insert(key.clone(), handle);
                self.video_thumb_sizes.insert(key, (w, h));
            }
            Message::AudioFinished => {
                self.audio_playing = None;
                self.audio_progress = 0.0;
                self.audio_start_time = None;
            }

            // --- Voice recording ---
            Message::StartVoiceRecording => {
                match fluxchat_core::voice::start_recording() {
                    Ok(handle) => {
                        self.voice_recording = true;
                        self.voice_record_start = Some(std::time::Instant::now());
                        // Store handle in a global — cpal::Stream isn't Send
                        VOICE_HANDLE.lock().unwrap().0 = Some(handle);
                    }
                    Err(e) => {
                        tracing::warn!("Failed to start recording: {e}");
                    }
                }
            }
            Message::StopVoiceRecording => {
                self.voice_recording = false;
                self.voice_record_start = None;
                if let Some(handle) = VOICE_HANDLE.lock().unwrap().0.take() {
                    // Encode on main thread (fast for short recordings)
                    match handle.stop_and_encode() {
                        Ok((bytes, duration)) => {
                            return self.update(Message::VoiceRecordingReady(bytes, duration));
                        }
                        Err(e) => tracing::warn!("Voice encode failed: {e}"),
                    }
                }
            }
            Message::CancelVoiceRecording => {
                self.voice_recording = false;
                self.voice_record_start = None;
                // Drop the handle to stop recording
                let _ = VOICE_HANDLE.lock().unwrap().0.take();
            }
            Message::VoiceRecordingReady(bytes, duration_ms) => {
                self.voice_ready = Some((bytes, duration_ms));
            }
            Message::SendVoiceMessage => {
                if let Some((bytes, duration_ms)) = self.voice_ready.take() {
                    if let Some(room_id) = self.selected_room_id.clone() {
                        let real_room = if room_id.starts_with("thread::") {
                            room_id.splitn(3, "::").nth(1).unwrap_or(&room_id).to_string()
                        } else {
                            room_id
                        };
                        let core = self.core.clone();
                        return Task::perform(
                            async move {
                                let client = core.get_client().await.ok()?;
                                fluxchat_core::messages::send_voice_message(
                                    &client, &real_room, bytes, duration_ms,
                                ).await.ok()
                            },
                            |_| Message::Noop,
                        );
                    }
                }
            }
            Message::DiscardVoiceMessage => {
                self.voice_ready = None;
            }
            Message::PreviewVoiceMessage => {
                if let Some((ref bytes, _)) = self.voice_ready {
                    let bytes = bytes.clone();
                    std::thread::spawn(move || {
                        let path = std::env::temp_dir().join("fluxchat_voice_preview.ogg");
                        if std::fs::write(&path, &bytes).is_ok() {
                            #[cfg(target_os = "macos")]
                            {
                                let _ = std::process::Command::new("ffplay")
                                    .args(["-nodisp", "-autoexit", "-loglevel", "quiet"])
                                    .arg(&path).status()
                                    .or_else(|_| std::process::Command::new("mpv").arg("--no-video").arg(&path).status());
                            }
                            #[cfg(target_os = "linux")]
                            {
                                let _ = std::process::Command::new("ffplay")
                                    .args(["-nodisp", "-autoexit", "-loglevel", "quiet"])
                                    .arg(&path).status()
                                    .or_else(|_| std::process::Command::new("pw-play").arg(&path).status())
                                    .or_else(|_| std::process::Command::new("mpv").arg("--no-video").arg(&path).status());
                            }
                            let _ = std::fs::remove_file(&path);
                        }
                    });
                }
            }

            // --- Navigation ---
            Message::JumpToEvent(event_id) => {
                self.highlight_event_id = Some(event_id.clone());
                if let Some(room_id) = &self.selected_room_id {
                    if let Some(msgs) = self.messages_by_room.get(room_id) {
                        if let Some(pos) = msgs.iter().position(|m| m.event_id == event_id) {
                            let offset = (pos as f32) * 40.0;
                            return iced::widget::operation::scroll_to(
                                crate::view::messages_scrollable_id(),
                                iced::widget::scrollable::AbsoluteOffset { x: 0.0, y: offset },
                            );
                        }
                    }
                }
            }

            // --- Code blocks ---
            Message::ToggleCodeBlock(event_id) => {
                if !self.expanded_code_blocks.remove(&event_id) {
                    self.expanded_code_blocks.insert(event_id);
                }
            }

            // --- Reporting ---
            Message::ReportReasonChanged(reason) => {
                self.report_reason = reason;
            }
            Message::SubmitReport => {
                if let Some(crate::state::Modal::Report { event_id, room_id }) = self.modal.take() {
                    let reason = std::mem::take(&mut self.report_reason);
                    let core = self.core.clone();
                    return Task::perform(
                        async move {
                            let client = core.get_client().await.ok()?;
                            fluxchat_core::messages::report_message(&client, &room_id, &event_id, &reason).await.ok()
                        },
                        |_| Message::Noop,
                    );
                }
            }

            // --- Quick switcher ---
            Message::ToggleQuickSwitcher => {
                self.quick_switcher_open = !self.quick_switcher_open;
                self.quick_switcher_query.clear();
                self.quick_switcher_selected = 0;
                if self.quick_switcher_open {
                    return iced::widget::operation::focus(crate::view::quick_switcher_id());
                }
            }
            Message::QuickSwitcherQuery(query) => {
                self.quick_switcher_query = query;
                self.quick_switcher_selected = 0;
            }
            Message::QuickSwitcherDown => {
                if self.quick_switcher_open {
                    let count = self.quick_switcher_matches().len();
                    if count > 0 {
                        self.quick_switcher_selected = (self.quick_switcher_selected + 1).min(count - 1);
                    }
                }
            }
            Message::QuickSwitcherUp => {
                if self.quick_switcher_open {
                    self.quick_switcher_selected = self.quick_switcher_selected.saturating_sub(1);
                }
            }
            Message::QuickSwitcherSelect => {
                if self.quick_switcher_open {
                    let matches = self.quick_switcher_matches();
                    if let Some(room_id) = matches.get(self.quick_switcher_selected).cloned() {
                        self.quick_switcher_open = false;
                        self.quick_switcher_query.clear();
                        return self.update(Message::SelectRoom(room_id));
                    }
                }
            }

            // --- Location ---
            Message::ToggleLocationPicker => {
                self.location_open = !self.location_open;
                if self.location_open {
                    self.location_lat.clear();
                    self.location_lon.clear();
                }
            }
            Message::LocationLatChanged(val) => {
                // Detect pasted "lat, lon" pair and auto-split
                if let Some((lat, lon)) = val.split_once(',') {
                    let lat = lat.trim().to_string();
                    let lon = lon.trim().to_string();
                    if lat.parse::<f64>().is_ok() && lon.parse::<f64>().is_ok() {
                        self.location_lat = lat;
                        self.location_lon = lon;
                        return Task::none();
                    }
                }
                self.location_lat = val;
            }
            Message::LocationLonChanged(val) => {
                self.location_lon = val;
            }
            Message::LocationSubmit => {
                let lat = self.location_lat.trim().to_string();
                let lon = self.location_lon.trim().to_string();
                if lat.is_empty() || lon.is_empty() {
                    return Task::none();
                }
                if lat.parse::<f64>().is_err() || lon.parse::<f64>().is_err() {
                    return Task::none();
                }
                let geo_uri = format!("geo:{lat},{lon}");
                self.location_open = false;
                return self.update(Message::SendLocation(geo_uri));
            }
            Message::SendLocation(geo_uri) => {
                if let Some(room_id) = self.selected_room_id.clone() {
                    let real_room = if room_id.starts_with("thread::") {
                        room_id.splitn(3, "::").nth(1).unwrap_or(&room_id).to_string()
                    } else {
                        room_id
                    };
                    let core = self.core.clone();
                    return Task::perform(
                        async move {
                            let client = core.get_client().await.ok()?;
                            fluxchat_core::messages::send_location(&client, &real_room, &geo_uri).await.ok()
                        },
                        |_| Message::Noop,
                    );
                }
            }

            // --- Panels data ---
            Message::PinsLoaded(msgs) => {
                self.pinned_messages = msgs;
            }
            Message::ThreadsLoaded(msgs) => {
                self.thread_messages = msgs;
            }

            // --- Room creation ---
            Message::CreateRoomNameChanged(name) => {
                self.create_room_name = name;
            }
            Message::CreateRoomTopicChanged(topic) => {
                self.create_room_topic = topic;
            }
            Message::CreateRoomTogglePublic => {
                self.create_room_public = !self.create_room_public;
            }
            Message::CreateRoomToggleEncrypted => {
                self.create_room_encrypted = !self.create_room_encrypted;
            }
            Message::CreateRoomToggleSpace => {
                self.create_room_is_space = !self.create_room_is_space;
            }
            Message::CreateRoomSelectSpace(space_id) => {
                self.create_room_space = space_id;
            }
            Message::CreateRoomSubmit => {
                let name = self.create_room_name.clone();
                if name.trim().is_empty() {
                    return Task::none();
                }
                let topic = if self.create_room_topic.trim().is_empty() { None } else { Some(self.create_room_topic.clone()) };
                let is_public = self.create_room_public;
                let encrypted = self.create_room_encrypted;
                let is_space = self.create_room_is_space;
                let space_id = self.create_room_space.clone();
                self.modal = None;
                self.create_room_name.clear();
                self.create_room_topic.clear();
                self.create_room_public = false;
                self.create_room_encrypted = false;
                self.create_room_is_space = false;
                self.create_room_space = None;
                let core = self.core.clone();
                return Task::perform(
                    async move {
                        let client = core.get_client().await.ok()?;
                        let room_id = fluxchat_core::rooms::create_room(
                            &client, &name, topic.as_deref(), is_public, encrypted, is_space, &[], space_id.as_deref(),
                        ).await.ok()?;
                        Some((room_id, name, encrypted, is_space, space_id))
                    },
                    |result| match result {
                        Some((room_id, name, encrypted, is_space, space_id)) => Message::RoomCreated(room_id, name, encrypted, is_space, space_id),
                        None => Message::Noop,
                    },
                );
            }
            Message::RoomCreated(room_id, name, encrypted, is_space, space_id) => {
                // Add to local room list immediately so sidebar updates
                let space_name = space_id.as_ref().and_then(|sid|
                    self.spaces.iter().find(|s| s.room_id == *sid).map(|s| s.name.clone())
                );
                if is_space {
                    let info = fluxchat_core::RoomInfo {
                        room_id: room_id.clone(),
                        name,
                        is_encrypted: encrypted,
                        is_space: true,
                        is_direct: false,
                        unread_count: 0,
                        highlight_count: 0,
                        avatar_url: None,
                        topic: None,
                        space_name: None,
                        space_id: None,
                        is_favourite: false,
                    };
                    if !self.spaces.iter().any(|s| s.room_id == room_id) {
                        self.spaces.push(info.clone());
                    }
                    if !self.space_tree.iter().any(|n| n.info.room_id == room_id) {
                        self.space_tree.push(fluxchat_core::rooms::SpaceNode {
                            info,
                            depth: 0,
                        });
                    }
                } else if !self.rooms.iter().any(|r| r.room_id == room_id) {
                    self.rooms.push(fluxchat_core::RoomInfo {
                        room_id: room_id.clone(),
                        name,
                        is_encrypted: encrypted,
                        is_space: false,
                        is_direct: false,
                        unread_count: 0,
                        highlight_count: 0,
                        avatar_url: None,
                        topic: None,
                        space_name,
                        space_id,
                        is_favourite: false,
                    });
                }
                if !is_space {
                    return self.update(Message::SelectRoom(room_id));
                }
            }

            // --- Room settings ---
            Message::RoomSettingsNameChanged(name) => {
                self.room_settings_name = name;
            }
            Message::RoomSettingsTopicChanged(topic) => {
                self.room_settings_topic = topic;
            }
            Message::EnableEncryption(room_id) => {
                // Update local state immediately
                if let Some(room) = self.rooms.iter_mut().find(|r| r.room_id == room_id) {
                    room.is_encrypted = true;
                }
                let core = self.core.clone();
                return Task::perform(
                    async move {
                        let client = core.get_client().await.ok()?;
                        let rid = fluxchat_core::ruma::RoomId::parse(&room_id).ok()?;
                        let room = client.get_room(&rid)?;
                        use matrix_sdk::ruma::events::room::encryption::RoomEncryptionEventContent;
                        room.send_state_event(RoomEncryptionEventContent::with_recommended_defaults()).await.ok()
                    },
                    |_| Message::Noop,
                );
            }
            Message::RoomSettingsSaveName => {
                if let Some(room_id) = self.selected_room_id.clone() {
                    let name = self.room_settings_name.clone();
                    // Update local state immediately
                    if let Some(room) = self.rooms.iter_mut().find(|r| r.room_id == room_id) {
                        room.name = name.clone();
                    }
                    let core = self.core.clone();
                    return Task::perform(
                        async move {
                            let client = core.get_client().await.ok()?;
                            fluxchat_core::rooms::set_room_name(&client, &room_id, &name).await.ok()
                        },
                        |_| Message::Noop,
                    );
                }
            }
            Message::RoomSettingsSaveTopic => {
                if let Some(room_id) = self.selected_room_id.clone() {
                    let topic = self.room_settings_topic.clone();
                    // Update local state immediately
                    if let Some(room) = self.rooms.iter_mut().find(|r| r.room_id == room_id) {
                        room.topic = if topic.is_empty() { None } else { Some(topic.clone()) };
                    }
                    let core = self.core.clone();
                    return Task::perform(
                        async move {
                            let client = core.get_client().await.ok()?;
                            fluxchat_core::rooms::set_room_topic(&client, &room_id, &topic).await.ok()
                        },
                        |_| Message::Noop,
                    );
                }
            }
            Message::RoomSettingsUploadAvatar => {
                if let Some(room_id) = self.selected_room_id.clone() {
                    let core = self.core.clone();
                    return Task::perform(
                        async move {
                            let handle = rfd::AsyncFileDialog::new()
                                .add_filter("Image", &["png", "jpg", "jpeg", "gif", "webp"])
                                .pick_file()
                                .await?;
                            let data = handle.read().await;
                            let name = handle.file_name();
                            let mime = if name.ends_with(".png") { "image/png" }
                                else if name.ends_with(".gif") { "image/gif" }
                                else if name.ends_with(".webp") { "image/webp" }
                                else { "image/jpeg" };
                            let client = core.get_client().await.ok()?;
                            let mxc = fluxchat_core::rooms::upload_room_avatar(&client, &room_id, mime, data.to_vec()).await.ok()?;
                            Some((room_id, Some(mxc)))
                        },
                        |result| match result {
                            Some((room_id, mxc)) => Message::RoomAvatarUpdated(room_id, mxc),
                            None => Message::Noop,
                        },
                    );
                }
            }
            Message::RoomAvatarUpdated(room_id, mxc) => {
                // Update avatar_url in local room list
                if let Some(room) = self.rooms.iter_mut().find(|r| r.room_id == room_id) {
                    room.avatar_url = mxc.clone();
                }
                // Resolve into avatar cache
                if let Some(mxc) = mxc {
                    return self.resolve_avatars(vec![mxc]);
                }
            }
            Message::RoomSettingsRemoveAvatar => {
                if let Some(room_id) = self.selected_room_id.clone() {
                    let core = self.core.clone();
                    return Task::perform(
                        async move {
                            let client = core.get_client().await.ok()?;
                            fluxchat_core::rooms::remove_room_avatar(&client, &room_id).await.ok()?;
                            Some(room_id)
                        },
                        |result| match result {
                            Some(room_id) => Message::RoomAvatarUpdated(room_id, None),
                            None => Message::Noop,
                        },
                    );
                }
            }

            // --- Forward ---
            Message::ForwardSearchChanged(query) => {
                self.forward_search = query;
            }
            Message::ForwardToRoom(target_room_id) => {
                if let Some(crate::state::Modal::Forward { body, formatted_body, .. }) = &self.modal {
                    let body = body.clone();
                    let formatted = if formatted_body.is_empty() { None } else { Some(formatted_body.clone()) };
                    let core = self.core.clone();
                    let rid = target_room_id.clone();
                    self.modal = None;
                    return Task::perform(
                        async move {
                            let client = core.get_client().await.ok()?;
                            fluxchat_core::messages::forward_message(&client, &rid, &body, formatted.as_deref()).await.ok()
                        },
                        |_| Message::Noop,
                    );
                }
            }

            // --- Invite ---
            Message::InviteSearchChanged(query) => {
                self.invite_search = query;
            }
            Message::InviteSearchSubmit => {
                let query = self.invite_search.clone();
                if query.trim().is_empty() {
                    return Task::none();
                }
                let core = self.core.clone();
                return Task::perform(
                    async move {
                        let client = core.get_client().await.ok()?;
                        fluxchat_core::rooms::search_users(&client, &query, 10).await.ok()
                    },
                    |result| match result {
                        Some(results) => Message::InviteSearchResults(results),
                        None => Message::InviteSearchResults(Vec::new()),
                    },
                );
            }
            Message::InviteSearchResults(results) => {
                self.invite_results = results;
            }
            Message::InviteUserFromModal(user_id) => {
                if let Some(room_id) = self.invite_target_id.take().or_else(|| self.selected_room_id.clone()) {
                    let core = self.core.clone();
                    self.modal = None;
                    self.invite_search.clear();
                    self.invite_results.clear();
                    return Task::perform(
                        async move {
                            let client = core.get_client().await.ok()?;
                            fluxchat_core::rooms::invite_user(&client, &room_id, &user_id).await.ok()
                        },
                        |_| Message::Noop,
                    );
                }
            }

            // --- Space management ---
            Message::AddRoomToSpace(space_id) => {
                if let Some(room_id) = self.selected_room_id.clone() {
                    let core = self.core.clone();
                    return Task::perform(
                        async move {
                            let client = core.get_client().await.ok()?;
                            fluxchat_core::rooms::add_room_to_space(&client, &room_id, &space_id).await.ok()
                        },
                        |_| Message::Noop,
                    );
                }
            }
            Message::RemoveRoomFromSpace(space_id) => {
                if let Some(room_id) = self.selected_room_id.clone() {
                    let core = self.core.clone();
                    return Task::perform(
                        async move {
                            let client = core.get_client().await.ok()?;
                            fluxchat_core::rooms::remove_room_from_space(&client, &room_id, &space_id).await.ok()
                        },
                        |_| Message::Noop,
                    );
                }
            }

            // --- Highlight words ---
            Message::AddHighlight(word) => {
                if !self.highlight_words.contains(&word) {
                    self.highlight_words.push(word);
                    if let Ok(json) = serde_json::to_string(&self.highlight_words) {
                        let _ = self.core.db.set_setting("highlight_words", &json);
                    }
                }
            }
            Message::RemoveHighlight(word) => {
                self.highlight_words.retain(|w| w != &word);
                if let Ok(json) = serde_json::to_string(&self.highlight_words) {
                    let _ = self.core.db.set_setting("highlight_words", &json);
                }
            }

            // --- Room notifications ---
            Message::MuteRoom => {
                if let Some(room_id) = self.selected_room_id.clone() {
                    let core = self.core.clone();
                    return Task::perform(
                        async move {
                            let client = core.get_client().await.ok()?;
                            fluxchat_core::rooms::set_notification_mode(&client, &room_id, "mute").await.ok()
                        },
                        |_| Message::Noop,
                    );
                }
            }
            Message::UnmuteRoom => {
                if let Some(room_id) = self.selected_room_id.clone() {
                    let core = self.core.clone();
                    return Task::perform(
                        async move {
                            let client = core.get_client().await.ok()?;
                            fluxchat_core::rooms::set_notification_mode(&client, &room_id, "all").await.ok()
                        },
                        |_| Message::Noop,
                    );
                }
            }
            Message::RefreshTimeline => {
                // Rebuild the Timeline in-place without clearing the view:
                // drop the cached Timeline, build a fresh one from the current
                // event cache, subscribe + paginate + respawn the diff watcher,
                // then swap messages_by_room atomically via TimelineLoaded.
                // Works around SDK state drift (e.g. lagged-subscriber recovery
                // dropping old messages from Timeline.state.items, leaving
                // aggregations unapplicable and reactions stuck).
                if let Some(room_id) = self.selected_room_id.clone() {
                    if room_id.starts_with("thread::") {
                        return Task::none();
                    }
                    // Reset drift counter so the post-refresh MessagesUpdated
                    // doesn't falsely re-trigger a rebuild.
                    self.last_timeline_real_count.remove(&room_id);
                    let core = self.core.clone();
                    let rid = room_id.clone();
                    let tx = self.timeline_tx.clone();
                    return Task::perform(
                        async move {
                            let client = core.get_client().await.ok()?;
                            let room_id_parsed = fluxchat_core::ruma::RoomId::parse(&rid).ok()?;

                            // Drop the cached Timeline so get_or_create builds fresh.
                            fluxchat_core::timeline::remove_timeline(
                                &core.timelines, &room_id_parsed,
                            ).await;

                            let members = fluxchat_core::messages::get_room_members(&client, &rid)
                                .await
                                .ok();

                            let timeline = fluxchat_core::timeline::get_or_create_timeline(
                                &client, &core.timelines, &room_id_parsed,
                            ).await.ok()?;

                            let (initial_items, stream) = timeline.subscribe().await;
                            let msgs = fluxchat_core::timeline::timeline_items_to_messages(
                                &initial_items.into_iter().collect::<Vec<_>>(),
                            );

                            let real_count = msgs.iter()
                                .filter(|m| !m.event_id.starts_with("~state-"))
                                .count();
                            if real_count < 30 {
                                let _ = timeline.paginate_backwards(30).await;
                            }

                            // Respawn the diff watcher against the new Timeline.
                            {
                                let timeline = timeline.clone();
                                let rid = rid.clone();
                                let tx = tx.clone();
                                tokio::spawn(async move {
                                    use futures_util::StreamExt;
                                    futures_util::pin_mut!(stream);
                                    while let Some(_diffs) = stream.next().await {
                                        let items = timeline.items().await;
                                        let msgs = fluxchat_core::timeline::timeline_items_to_messages(
                                            &items.into_iter().collect::<Vec<_>>(),
                                        );
                                        if tx.send(Message::SyncEvent(
                                            SyncEvent::MessagesUpdated(rid.clone(), msgs),
                                        )).is_err() {
                                            break;
                                        }
                                    }
                                    tracing::warn!("Diff stream watcher ended for room {}", rid);
                                });
                            }

                            // Re-read post-pagination so TimelineLoaded carries
                            // the full restored set.
                            let items = timeline.items().await;
                            let final_msgs = fluxchat_core::timeline::timeline_items_to_messages(
                                &items.into_iter().collect::<Vec<_>>(),
                            );

                            Some((rid, final_msgs, members))
                        },
                        |result| match result {
                            Some((room_id, messages, members)) => Message::TimelineLoaded {
                                room_id, messages, members,
                            },
                            None => Message::Noop,
                        },
                    );
                }
            }

            // --- Giphy ---
            Message::SetGiphyKey(key) => {
                self.giphy_api_key = key.clone();
                let _ = self.core.db.set_setting("giphy_api_key", &key);
            }
            Message::GifSearch(query) => {
                self.gifs_open = true;
                self.gif_query = query;
                let search = self.update(Message::GifSearchSubmit);
                let focus = iced::widget::operation::focus(crate::view::gif_search_id());
                return Task::batch([search, focus]);
            }
            Message::GifQueryChanged(query) => {
                self.gif_query = query;
                self.gif_searched = false;
            }
            Message::GifSearchSubmit => {
                let query = self.gif_query.clone();
                if query.trim().is_empty() {
                    self.gif_results.clear();
                    return Task::none();
                }
                self.gif_searching = true;
                let api_key = if self.giphy_api_key.is_empty() { None } else { Some(self.giphy_api_key.clone()) };
                return Task::perform(
                    async move {
                        fluxchat_core::gif::search_gifs(&query, api_key.as_deref()).await.unwrap_or_default()
                    },
                    Message::GifResultsLoaded,
                );
            }
            Message::GifResultsLoaded(results) => {
                self.gif_searching = false;
                self.gif_searched = true;
                // Fetch preview images
                let mut tasks = Vec::new();
                for gif in &results {
                    let url = gif.preview_url.clone();
                    if !self.gif_preview_cache.contains_key(&url) {
                        let url_clone = url.clone();
                        tasks.push(Task::perform(
                            async move {
                                let bytes = fluxchat_core::gif::download_gif(&url_clone).await.ok()?;
                                let frames = iced_gif::Frames::from_bytes(bytes).ok()?;
                                Some((url_clone, std::sync::Arc::new(frames)))
                            },
                            |result| match result {
                                Some((url, frames)) => Message::GifPreviewLoaded(url, frames),
                                None => Message::Noop,
                            },
                        ));
                    }
                }
                self.gif_results = results;
                if !tasks.is_empty() {
                    return Task::batch(tasks);
                }
            }
            Message::GifPreviewLoaded(url, frames) => {
                self.gif_preview_cache.insert(url, frames);
                evict_cache(&mut self.gif_preview_cache, GIF_CACHE_LIMIT);
            }
            Message::GifSelect(gif_url, description) => {
                self.gifs_open = false;
                self.gif_results.clear();
                self.gif_query.clear();
                if let Some(room_id) = self.selected_room_id.clone() {
                    let core = self.core.clone();
                    return Task::perform(
                        async move {
                            // Download the GIF and upload as image
                            let data = fluxchat_core::gif::download_gif(&gif_url).await.ok()?;
                            let client = core.get_client().await.ok()?;
                            fluxchat_core::messages::upload_file(
                                &client, &room_id, &format!("{description}.gif"), "image/gif", data,
                            ).await.ok()
                        },
                        |_| Message::Noop,
                    );
                }
            }

            // --- Read receipts ---
            Message::SendReadReceipt => {
                if let Some(room_id) = self.selected_room_id.clone() {
                    if let Some(event_id) = self.last_read_event.get(&room_id).cloned() {
                        let core = self.core.clone();
                        return Task::perform(
                            async move {
                                let client = core.get_client().await.ok()?;
                                fluxchat_core::messages::send_read_receipt(&client, &room_id, &event_id).await.ok()
                            },
                            |_| Message::Noop,
                        );
                    }
                }
            }

            // --- Misc ---
            Message::PaginateBack(room_id) => {
                let core = self.core.clone();
                return Task::perform(
                    async move {
                        let timeline = {
                            let timelines = core.timelines.lock().await;
                            let room_id_parsed = fluxchat_core::ruma::RoomId::parse(&room_id).ok()?;
                            timelines.get(&*room_id_parsed)?.clone()
                        };
                        // First call may just adjust lazy-loading window; second call
                        // actually fetches from server if needed.
                        let r1 = timeline.paginate_backwards(20).await;
                        if matches!(r1, Ok(false)) {
                            let _ = timeline.paginate_backwards(20).await;
                        }
                        Some(())
                    },
                    |result| match result {
                        Some(()) => Message::PaginateBackDone,
                        None => Message::PaginateBackDone,
                    },
                );
            }
            Message::PaginateBackDone => {
                // Don't clear paginating here. Messages arrive via diff stream
                // (MessagesUpdated), which will clear it when count changes.
                // Set a timeout to clear it in case no messages arrive (end of history).
                return Task::perform(
                    async { tokio::time::sleep(std::time::Duration::from_secs(3)).await },
                    |_| Message::PaginateTimeout,
                );
            }
            Message::PaginateTimeout => {
                if self.paginating {
                    self.paginating = false;
                }
            }
            Message::ScrollToBottom => {
                self.messages_at_bottom = true;
                self.new_messages_below = false;
                return iced::widget::operation::snap_to_end(
                    crate::view::messages_scrollable_id(),
                );
            }
            Message::MessagesScrolled(viewport) => {
                // absolute_offset().y = 0 at bottom, increases scrolling up
                // No anchor_bottom: offset=0 at top (oldest), increases scrolling down
                let offset = viewport.absolute_offset().y;
                let content_h = viewport.content_bounds().height;
                let viewport_h = viewport.bounds().height;
                let max_scroll = (content_h - viewport_h).max(0.0);
                let near_top = offset < 100.0;
                let near_bottom = (max_scroll - offset) < 50.0;
                // Distinguish user scroll-up from content growth (image/preview loads).
                // Content growth: offset stays same or increases, max_scroll grows.
                // User scroll-up: offset decreases.
                // If we were at bottom and content grew, snap back instead of
                // marking messages_at_bottom = false (which would stop auto-scroll).
                if self.messages_at_bottom && !near_bottom
                    && offset >= self.last_scroll_offset - 1.0
                {
                    // Content grew while we were at the bottom — snap back
                    self.last_scroll_offset = offset;
                    return iced::widget::operation::snap_to_end(
                        crate::view::messages_scrollable_id(),
                    );
                }
                self.messages_at_bottom = near_bottom;
                self.last_scroll_offset = offset;
                if near_bottom {
                    self.new_messages_below = false;
                }
                // Clear paginating once scroll correction has settled
                if self.paginating && !near_top {
                    self.paginating = false;
                }
                // When near the top (oldest messages) and not already paginating
                if near_top && !self.paginating {
                    if let Some(room_id) = self.selected_room_id.clone() {
                        let msg_count = self.messages_by_room
                            .get(&room_id).map(|m| m.len()).unwrap_or(0);
                        self.paginate_msg_count = msg_count;
                        self.pre_paginate_offset = offset;
                        self.paginating = true;
                        return self.update(Message::PaginateBack(room_id));
                    }
                }
            }
            Message::SidebarDragStart => {
                self.sidebar_dragging = true;
            }
            Message::SidebarDragEnd => {
                self.sidebar_dragging = false;
            }
            Message::CursorMoved(x, y) => {
                self.cursor_position = (x, y);
                if self.sidebar_dragging {
                    let new_width = (x - 56.0).clamp(150.0, 400.0);
                    self.sidebar_width = new_width;
                }
            }
            Message::SidebarDrag(_) => {
                // Legacy — now handled by CursorMoved
            }
            Message::OpenUrl(url) => {
                // Handle matrix.to room links by navigating to the room
                if let Some(rest) = url.strip_prefix("https://matrix.to/#/") {
                    let decoded = rest.replace("%3A", ":").replace("%23", "#");
                    let id = decoded.split('?').next().unwrap_or(&decoded);
                    if id.starts_with('!') {
                        // Room ID — navigate directly
                        return self.update(Message::SelectRoom(id.to_string()));
                    } else if id.starts_with('#') {
                        // Room alias — find matching room by alias-like name
                        if let Some(room) = self.rooms.iter().find(|r| {
                            // Check if any part of the alias matches the room name
                            let alias_name = id.split(':').next().unwrap_or(id).trim_start_matches('#');
                            r.name.eq_ignore_ascii_case(alias_name)
                        }) {
                            return self.update(Message::SelectRoom(room.room_id.clone()));
                        }
                    } else if id.starts_with('@') {
                        // User mention — open profile modal
                        return self.update(Message::OpenModal(crate::state::Modal::Profile { user_id: id.to_string() }));
                    }
                }
                let _ = open::that(&url);
            }
            Message::Escape => {
                // Priority: quick switcher > overflow > completions > modal > panels > reply/edit
                if self.quick_switcher_open {
                    self.quick_switcher_open = false;
                } else if self.search_open {
                    self.search_open = false;
                    self.search_results.clear();
                    self.search_selected = None;
                    self.highlight_event_id = None;
                } else if self.selection.anchor.is_some() {
                    self.selection = Default::default();
                } else if self.context_menu.is_some() {
                    self.context_menu = None;
                } else if self.overflow_menu_open {
                    self.overflow_menu_open = false;
                } else if !self.cmd_completions.is_empty() {
                    self.cmd_completions.clear();
                } else if !self.mention_completions.is_empty() {
                    self.mention_completions.clear();
                } else if self.modal.is_some() {
                    self.modal = None;
                } else if self.pins_open {
                    self.pins_open = false;
                } else if self.threads_open {
                    self.threads_open = false;
                } else if self.files_open {
                    self.files_open = false;
                } else if self.notices_open {
                    self.notices_open = false;
                } else if self.gifs_open {
                    self.gifs_open = false;
                } else if self.emoji_open {
                    self.emoji_open = false;
                } else if self.help_text.is_some() {
                    self.help_text = None;
                } else if self.reply_to.is_some() {
                    self.reply_to = None;
                } else if self.edit_event.is_some() {
                    self.edit_event = None;
                    self.input_text.clear();
                    self.input_content = iced::widget::text_editor::Content::new();
                }
            }
            Message::Tick => {
                if self.sync_running {
                    let mut tasks = Vec::new();

                    // Room list updates are now reactive via RoomListService watcher
                    // Tick only handles read receipts and call presence polling

                    // Refresh invites (sliding sync watcher may not trigger for invite state changes)
                    {
                        let core_inv = self.core.clone();
                        tasks.push(Task::perform(
                            async move {
                                let client = core_inv.get_client().await.ok()?;
                                let invited = fluxchat_core::rooms::get_invited_rooms(&client).await.ok()?;
                                Some(invited)
                            },
                            |result| match result {
                                Some(invited) => Message::InvitesRefreshed(invited),
                                None => Message::Noop,
                            },
                        ));
                    }

                    // Clear stale typing indicators (timeout after 10s)
                    self.typing_by_room.retain(|_, (_, updated)| updated.elapsed().as_secs() < 10);

                    // Periodic timeline refresh for the active room.
                    // The diff stream watcher handles live updates, but this
                    // acts as a reliable fallback that also picks up reaction
                    // changes and read receipt movements.
                    if let Some(ref room_id) = self.selected_room_id {
                        if !room_id.starts_with("thread::") {
                            let core_tl = self.core.clone();
                            let rid_tl = room_id.clone();
                            tasks.push(Task::perform(
                                async move {
                                    let client = core_tl.get_client().await.ok()?;
                                    let room_id_parsed = fluxchat_core::ruma::RoomId::parse(&rid_tl).ok()?;
                                    let timeline = fluxchat_core::timeline::get_or_create_timeline(
                                        &client, &core_tl.timelines, &room_id_parsed,
                                    ).await.ok()?;
                                    let items = timeline.items().await;
                                    let msgs = fluxchat_core::timeline::timeline_items_to_messages(
                                        &items.into_iter().collect::<Vec<_>>(),
                                    );
                                    Some((rid_tl, msgs))
                                },
                                |result| match result {
                                    Some((rid, msgs)) => Message::SyncEvent(SyncEvent::MessagesUpdated(rid, msgs)),
                                    None => Message::Noop,
                                },
                            ));
                        }
                    }

                    // Refresh member list for active room
                    if let Some(ref room_id) = self.selected_room_id {
                        if !room_id.starts_with("thread::") {
                            let core_m = self.core.clone();
                            let rid_m = room_id.clone();
                            tasks.push(Task::perform(
                                async move {
                                    let client = core_m.get_client().await.ok()?;
                                    let members = fluxchat_core::messages::get_room_members(&client, &rid_m).await.ok()?;
                                    Some((rid_m, members))
                                },
                                |result| match result {
                                    Some((rid, members)) => Message::SyncEvent(SyncEvent::MembersUpdated(rid, members)),
                                    None => Message::Noop,
                                },
                            ));
                        }
                    }

                    // Scan all rooms for active calls (sidebar indicators)
                    {
                        let core3 = self.core.clone();
                        let room_ids: Vec<String> = self.rooms.iter().map(|r| r.room_id.clone()).collect();
                        tasks.push(Task::perform(
                            async move {
                                let client = core3.get_client().await.ok()?;
                                let mut rooms_with_calls = std::collections::HashSet::new();
                                for rid in &room_ids {
                                    if fluxchat_core::rtc::has_active_call(&client, rid).await {
                                        rooms_with_calls.insert(rid.clone());
                                    }
                                }
                                Some(rooms_with_calls)
                            },
                            |result| match result {
                                Some(rooms) => Message::CallRoomsWithCallsUpdated(rooms),
                                None => Message::Noop,
                            },
                        ));
                    }

                    // Load participants for selected room (join banner)
                    if let Some(ref room_id) = self.selected_room_id {
                        let core4 = self.core.clone();
                        let rid = room_id.clone();
                        let is_connected = self.call_state == fluxchat_core::rtc::CallConnectionState::Connected;
                        if !is_connected {
                            tasks.push(Task::perform(
                                async move {
                                    let client = core4.get_client().await.ok()?;
                                    let participants = fluxchat_core::rtc::get_call_members(&client, &rid).await.ok()?;
                                    Some((rid, participants))
                                },
                                |result| match result {
                                    Some((rid, participants)) => Message::CallParticipantsUpdated(rid, participants),
                                    None => Message::Noop,
                                },
                            ));
                        }
                    }

                    return Task::batch(tasks);
                }
            }
            // --- Calls/RTC (delegated to update_call.rs) ---
            msg @ (Message::CallJoin(_) | Message::CallJoinResult(_) | Message::CallConnected
                | Message::CallDisconnected(_) | Message::CallError(_) | Message::CallLeave
                | Message::CallParticipantJoined(_) | Message::CallParticipantLeft(_)
                | Message::CallParticipantsUpdated(_, _) | Message::CallToggleMic
                | Message::CallRoomsWithCallsUpdated(_)) => {
                if let Some(task) = self.handle_call_message(msg) {
                    return task;
                }
            }

            Message::Noop => {}
        }

        Task::none()
    }

    /// Get room IDs matching the quick switcher query (fuzzy match on room name).
    pub(crate) fn quick_switcher_matches(&self) -> Vec<String> {
        let query = self.quick_switcher_query.to_lowercase();
        if query.is_empty() {
            // Show recent/all rooms when query is empty
            return self.rooms.iter()
                .take(10)
                .map(|r| r.room_id.clone())
                .collect();
        }
        self.rooms.iter()
            .filter(|r| {
                r.name.to_lowercase().contains(&query)
                || r.space_name.as_ref().is_some_and(|s| s.to_lowercase().contains(&query))
            })
            .take(10)
            .map(|r| r.room_id.clone())
            .collect()
    }

    /// Spawn background avatar resolution for any uncached MXC URIs.
    fn resolve_avatars(&self, mxc_uris: Vec<String>) -> Task<Message> {
        let uncached: Vec<String> = mxc_uris
            .into_iter()
            .filter(|mxc| !self.avatar_cache.contains_key(mxc))
            .collect();
        if uncached.is_empty() {
            return Task::none();
        }
        let core = self.core.clone();
        Task::perform(
            async move {
                use matrix_sdk::media::{MediaRequestParameters, MediaFormat};
                use matrix_sdk::ruma::events::room::MediaSource;
                use matrix_sdk::ruma::MxcUri;

                let client = match core.get_client().await {
                    Ok(c) => c,
                    Err(_) => return Vec::new(),
                };
                let mut resolved = Vec::new();
                for mxc in uncached {
                    let mxc_uri = <&MxcUri>::from(mxc.as_str());
                    let req = MediaRequestParameters {
                        source: MediaSource::Plain(mxc_uri.to_owned()),
                        format: MediaFormat::Thumbnail(matrix_sdk::media::MediaThumbnailSettings::new(
                            matrix_sdk::ruma::UInt::new(40).unwrap(),
                            matrix_sdk::ruma::UInt::new(40).unwrap(),
                        )),
                    };
                    // Try thumbnail first, fall back to full file
                    let data = match client.media().get_media_content(&req, true).await {
                        Ok(d) => d,
                        Err(_) => {
                            let req_full = MediaRequestParameters {
                                source: MediaSource::Plain(mxc_uri.to_owned()),
                                format: MediaFormat::File,
                            };
                            match client.media().get_media_content(&req_full, true).await {
                                Ok(d) => d,
                                Err(_) => continue,
                            }
                        }
                    };
                    resolved.push((mxc.clone(), data));
                }
                resolved
            },
            Message::AvatarsResolved,
        )
    }

    /// Spawn full-size image fetches for message images.
    /// Extract selected text across messages for Ctrl+C copy.
    fn extract_selection_text(&self) -> Option<String> {
        let anchor = self.selection.anchor.as_ref()?;
        let cursor = self.selection.cursor.as_ref()?;

        let room_id = self.selected_room_id.as_ref()?;
        let messages = self.messages_by_room.get(room_id)?;

        let anchor_idx = messages.iter().position(|m| m.event_id == anchor.widget_id.event_id)?;
        let cursor_idx = messages.iter().position(|m| m.event_id == cursor.widget_id.event_id)?;

        let (start, end, start_idx, end_idx) = if anchor_idx < cursor_idx
            || (anchor_idx == cursor_idx && (anchor.line < cursor.line || (anchor.line == cursor.line && anchor.index <= cursor.index)))
        {
            (anchor, cursor, anchor_idx, cursor_idx)
        } else {
            (cursor, anchor, cursor_idx, anchor_idx)
        };

        let mut result = String::new();
        for i in start_idx..=end_idx {
            let msg = &messages[i];
            let text = &msg.body;

            if i == start_idx && i == end_idx {
                // Single message
                let s = line_index_to_offset(text, start.line, start.index);
                let e = line_index_to_offset(text, end.line, end.index);
                if s < e && e <= text.len() {
                    result.push_str(&text[s..e]);
                }
            } else if i == start_idx {
                let off = line_index_to_offset(text, start.line, start.index);
                if off < text.len() {
                    result.push_str(&text[off..]);
                }
                result.push('\n');
            } else if i == end_idx {
                let off = line_index_to_offset(text, end.line, end.index);
                result.push_str(&text[..off.min(text.len())]);
            } else {
                result.push_str(text);
                result.push('\n');
            }
        }

        if result.is_empty() { None } else { Some(result) }
    }

    fn resolve_images(&mut self, messages: &[fluxchat_core::MessageInfo]) -> Task<Message> {
        let mut tasks = Vec::new();
        for msg in messages {
            if let Some(ref mxc) = msg.image_mxc {
                if self.image_cache.contains_key(mxc) || self.image_cache_pending.contains(mxc) {
                    continue;
                }
                // Check SQLite cache first
                if let Some(b64) = self.core.db.get_cached_media(&format!("iced_img:{mxc}")) {
                    use base64::Engine;
                    if let Ok(data) = base64::engine::general_purpose::STANDARD.decode(&b64) {
                        // Validate it's actual image data (not garbage from a failed fetch)
                        let is_valid = data.len() > 100 && (
                            data.starts_with(&[0x89, 0x50, 0x4E, 0x47]) || // PNG
                            data.starts_with(&[0xFF, 0xD8, 0xFF]) ||       // JPEG
                            data.starts_with(b"GIF") ||                     // GIF
                            data.starts_with(b"RIFF") ||                    // WebP
                            (data.len() > 12 && &data[4..8] == b"ftyp")    // MP4/HEIF
                        );
                        if !is_valid {
                            tracing::warn!("Cached image data for {mxc} is invalid ({} bytes), re-fetching", data.len());
                            // Remove bad cache entry
                            self.core.db.cache_media(&format!("iced_img:{mxc}"), "");
                        } else {
                        let is_gif = data.len() >= 3 && &data[..3] == b"GIF";
                        let is_webp = data.len() >= 12
                            && &data[..4] == b"RIFF" && &data[8..12] == b"WEBP";
                        let handle = iced::widget::image::Handle::from_bytes(data.clone());
                        self.image_cache.insert(mxc.clone(), handle);
                        if (is_gif || is_webp) && !self.gif_frames_cache.contains_key(mxc) {
                            let mxc_clone = mxc.clone();
                            // Fast path for previously-transcoded WebPs: cached
                            // GIF bytes skip the NeuQuant pass entirely.
                            let cached_gif_b64 = if is_webp {
                                self.core.db.get_cached_media(&format!("iced_gif:{mxc}"))
                            } else {
                                None
                            };
                            let core_task = self.core.clone();
                            tasks.push(Task::perform(
                                async move {
                                    let mxc_log = mxc_clone.clone();
                                    let mxc_inner = mxc_clone.clone();
                                    let frames = match tokio::task::spawn_blocking(move || {
                                        let gif_bytes = if is_webp {
                                            if let Some(b64) = cached_gif_b64 {
                                                match base64::Engine::decode(
                                                    &base64::engine::general_purpose::STANDARD, &b64
                                                ) {
                                                    Ok(bytes) if bytes.len() >= 3
                                                        && &bytes[..3] == b"GIF" => bytes,
                                                    _ => {
                                                        let gb = webp_to_gif_bytes(&data)?;
                                                        let enc = base64::Engine::encode(
                                                            &base64::engine::general_purpose::STANDARD, &gb
                                                        );
                                                        core_task.db.cache_media(&format!("iced_gif:{mxc_inner}"), &enc);
                                                        gb
                                                    }
                                                }
                                            } else {
                                                let gb = webp_to_gif_bytes(&data)?;
                                                let enc = base64::Engine::encode(
                                                    &base64::engine::general_purpose::STANDARD, &gb
                                                );
                                                core_task.db.cache_media(&format!("iced_gif:{mxc_inner}"), &enc);
                                                gb
                                            }
                                        } else {
                                            data
                                        };
                                        match iced_gif::Frames::from_bytes(gif_bytes) {
                                            Ok(f) => Some(f),
                                            Err(e) => {
                                                tracing::warn!("iced_gif Frames::from_bytes failed: {e}");
                                                None
                                            }
                                        }
                                    }).await {
                                        Ok(Some(f)) => f,
                                        Ok(None) => return None,
                                        Err(e) => {
                                            tracing::warn!("transcode task for {mxc_log} failed: {e}");
                                            return None;
                                        }
                                    };
                                    Some((mxc_clone, std::sync::Arc::new(frames)))
                                },
                                |result| match result {
                                    Some((mxc, frames)) => Message::GifFramesResolved(mxc, frames),
                                    None => Message::Noop,
                                },
                            ));
                        }
                        continue;
                    } // is_valid
                    }
                }
                self.image_cache_pending.insert(mxc.clone());
                let core = self.core.clone();
                let mxc_clone = mxc.clone();
                let event_id = msg.event_id.clone();
                let room_id = self.selected_room_id.clone().unwrap_or_default();
                let real_room = if room_id.starts_with("thread::") {
                    room_id.splitn(3, "::").nth(1).unwrap_or(&room_id).to_string()
                } else {
                    room_id
                };
                tasks.push(Task::perform(
                    async move {
                        let client = core.get_client().await.ok()?;
                        // Try event-based fetch first (handles encrypted media)
                        let event_result = fluxchat_core::media::get_event_media_bytes(
                            &client, &real_room, &event_id,
                        ).await;
                        let data = match event_result {
                            Ok((bytes, _mime)) => bytes,
                            Err(_) => {
                                use matrix_sdk::media::{MediaRequestParameters, MediaFormat};
                                use matrix_sdk::ruma::events::room::MediaSource;
                                use matrix_sdk::ruma::MxcUri;
                                let mxc_uri = <&MxcUri>::from(mxc_clone.as_str());
                                let req = MediaRequestParameters {
                                    source: MediaSource::Plain(mxc_uri.to_owned()),
                                    format: MediaFormat::File,
                                };
                                client.media().get_media_content(&req, true).await.ok()?
                            }
                        };
                        Some((mxc_clone, data))
                    },
                    |result| match result {
                        Some((mxc, data)) => Message::ImageResolved(mxc, data),
                        None => Message::Noop,
                    },
                ));
            }
            // Generate thumbnails for videos without server-provided thumbnails
            if msg.is_video && msg.image_mxc.is_none() {
                if let Some(ref vmxc) = msg.video_mxc {
                    let thumb_key = format!("video_thumb:{vmxc}");
                    if self.image_cache.contains_key(&thumb_key) || self.image_cache_pending.contains(&thumb_key) {
                        continue;
                    }
                    // Check SQLite cache
                    if let Some(b64) = self.core.db.get_cached_media(&format!("iced_img:{thumb_key}")) {
                        use base64::Engine;
                        if let Ok(data) = base64::engine::general_purpose::STANDARD.decode(&b64) {
                            if data.len() > 100 {
                                // Read PNG dimensions from header
                                if let Ok(reader) = image::ImageReader::new(std::io::Cursor::new(&data)).with_guessed_format() {
                                    if let Ok((w, h)) = reader.into_dimensions() {
                                        self.video_thumb_sizes.insert(thumb_key.clone(), (w, h));
                                    }
                                }
                                let handle = iced::widget::image::Handle::from_bytes(data);
                                self.image_cache.insert(thumb_key, handle);
                                continue;
                            }
                        }
                    }
                    self.image_cache_pending.insert(thumb_key.clone());
                    let core = self.core.clone();
                    let event_id = msg.event_id.clone();
                    let room_id = self.selected_room_id.clone().unwrap_or_default();
                    let real_room = if room_id.starts_with("thread::") {
                        room_id.splitn(3, "::").nth(1).unwrap_or(&room_id).to_string()
                    } else {
                        room_id
                    };
                    tasks.push(Task::perform(
                        async move {
                            let client = core.get_client().await.ok()?;
                            let (data, _mime) = fluxchat_core::media::get_event_media_bytes(
                                &client, &real_room, &event_id,
                            ).await.ok()?;
                            // Write to temp file for GStreamer
                            let tmp = std::env::temp_dir().join(format!("fluxchat_thumb_{event_id}.tmp"));
                            std::fs::write(&tmp, &data).ok()?;
                            let result = extract_video_thumbnail(&tmp);
                            let _ = std::fs::remove_file(&tmp);
                            let (data, w, h) = result?;
                            Some((thumb_key, data, w, h))
                        },
                        |result| match result {
                            Some((key, data, w, h)) => Message::VideoThumbResolved(key, data, w, h),
                            None => Message::Noop,
                        },
                    ));
                }
            }
        }
        if tasks.is_empty() { Task::none() } else { Task::batch(tasks) }
    }

    /// Spawn URL preview fetches for any uncached URLs in the given messages.
    fn resolve_url_previews(&mut self, messages: &[fluxchat_core::MessageInfo]) -> Task<Message> {
        let mut tasks = Vec::new();
        for msg in messages {
            for url in extract_urls(&msg.body) {
                if self.url_previews.contains_key(&url) || self.url_previews_pending.contains(&url) {
                    continue;
                }
                self.url_previews_pending.insert(url.clone());
                let core = self.core.clone();
                let url_clone = url.clone();
                tasks.push(Task::perform(
                    async move {
                        let client = core.get_client().await.ok()?;
                        let preview = fluxchat_core::preview::get_url_preview(
                            &client,
                            &core.media_cache,
                            &url_clone,
                        ).await.ok()?;
                        Some((url_clone, preview))
                    },
                    |result| match result {
                        Some((url, preview)) => Message::UrlPreviewFetched(url, preview),
                        None => Message::Noop,
                    },
                ));
            }
        }
        if tasks.is_empty() {
            Task::none()
        } else {
            Task::batch(tasks)
        }
    }

    /// Complete login: set user info, load settings, start sync.
    fn finish_login(&mut self, success: LoginSuccess) -> Task<Message> {
        self.user_id = success.user_id;
        self.display_name = success.display_name;
        self.screen = Screen::Main;
        self.loading = false;
        self.login_submitting = false;
        self.load_persisted_settings();

        // Start sync and load initial rooms
        let core = self.core.clone();
        let tx = self.timeline_tx.clone();
        Task::perform(
            async move {
                let client = match core.get_client().await {
                    Ok(c) => c,
                    Err(_) => return None,
                };

                // Store device ID
                if let Some(did) = client.device_id() {
                    let _ = core.db.set_setting("device_id", &did.to_string());
                }

                // Enable event cache
                let _ = client.event_cache().subscribe();

                // Register typing handler — fires under sliding sync
                {
                    let tx = tx.clone();
                    let own_user_id = client.user_id().map(|u| u.to_owned());
                    client.add_event_handler(move |ev: matrix_sdk::ruma::events::typing::SyncTypingEvent, room: matrix_sdk::Room| {
                        let tx = tx.clone();
                        let own_id = own_user_id.clone();
                        async move {
                            let user_ids: Vec<String> = ev.content.user_ids.iter()
                                .filter(|u| own_id.as_ref() != Some(*u))
                                .map(|u| u.to_string())
                                .collect();
                            let _ = tx.send(Message::SyncEvent(SyncEvent::TypingUpdate(
                                room.room_id().to_string(), user_ids,
                            )));
                        }
                    });
                }

                // Register verification handler — fires under sliding sync
                {
                    let tx = tx.clone();
                    let client_clone = client.clone();
                    client.add_event_handler(move |ev: matrix_sdk::ruma::events::key::verification::request::ToDeviceKeyVerificationRequestEvent| {
                        let tx = tx.clone();
                        let client = client_clone.clone();
                        async move {
                            let request = client.encryption()
                                .get_verification_request(&ev.sender, &ev.content.transaction_id)
                                .await;
                            if let Some(request) = request {
                                let _ = tx.send(Message::SyncEvent(SyncEvent::VerificationRequest(
                                    fluxchat_core::VerificationInfo {
                                        flow_id: request.flow_id().to_string(),
                                        other_user_id: request.other_user_id().to_string(),
                                        is_self_verification: request.is_self_verification(),
                                        state: "requested".into(),
                                        emojis: None,
                                    },
                                )));
                            }
                        }
                    });
                }

                // Start sync service (sliding sync)
                match fluxchat_core::sync_service::create_sync_service(&client).await {
                    Ok(sync_service) => {
                        // Spawn room list watcher for reactive updates
                        {
                            let tx = tx.clone();
                            fluxchat_core::sync_service::spawn_room_list_watcher(
                                &client, &sync_service, move |rooms, spaces, invited, knocked, space_tree, space_descendants| {
                                    let _ = tx.send(Message::SyncEvent(SyncEvent::RoomListUpdate(
                                        rooms, spaces, invited, knocked, space_tree, space_descendants,
                                    )));
                                },
                            ).await;
                        }
                        // Spawn reconnection watcher
                        {
                            let mut state_sub = sync_service.state();
                            let core_reconnect = core.clone();
                            let tx_reconnect = tx.clone();
                            tokio::spawn(async move {
                                use fluxchat_core::SyncState;
                                loop {
                                    let next: Option<SyncState> = state_sub.next().await;
                                    if next.is_none() { break; }
                                    let current = state_sub.get();
                                    match current {
                                        SyncState::Error(ref e) => {
                                            tracing::warn!("Sync error: {e}, reconnecting in 5s...");
                                            tokio::time::sleep(std::time::Duration::from_secs(5)).await;
                                            if let Some(ref svc) = *core_reconnect.sync_service.lock().await {
                                                svc.start().await;
                                                tracing::info!("Sync restarted after error");
                                                let _ = tx_reconnect.send(Message::SyncReconnected);
                                            }
                                        }
                                        SyncState::Terminated => {
                                            tracing::warn!("Sync terminated, reconnecting in 5s...");
                                            tokio::time::sleep(std::time::Duration::from_secs(5)).await;
                                            if let Some(ref svc) = *core_reconnect.sync_service.lock().await {
                                                svc.start().await;
                                                tracing::info!("Sync restarted after termination");
                                                let _ = tx_reconnect.send(Message::SyncReconnected);
                                            }
                                        }
                                        _ => {}
                                    }
                                }
                            });
                        }

                        *core.sync_service.lock().await = Some(sync_service);
                    }
                    Err(e) => {
                        tracing::warn!("SyncService failed: {e}");
                    }
                }

                // Load initial rooms
                let rooms = fluxchat_core::rooms::get_rooms(&client).await.ok()?;
                let spaces = fluxchat_core::rooms::get_spaces(&client).await.ok()?;
                let invited = fluxchat_core::rooms::get_invited_rooms(&client).await.ok()?;
                let knocked = fluxchat_core::rooms::get_knocked_rooms(&client).await.ok()?;
                let (space_tree, space_descendants) = fluxchat_core::rooms::get_space_tree(&client).await;

                Some((rooms, spaces, invited, knocked, space_tree, space_descendants))
            },
            |result| match result {
                Some((rooms, spaces, invited, knocked, space_tree, space_descendants)) => {
                    Message::InitialRoomsLoaded(rooms, spaces, invited, knocked, space_tree, space_descendants)
                }
                None => Message::SyncStarted,
            },
        )
    }

    /// Load persisted settings from SQLite into state.
    fn load_persisted_settings(&mut self) {
        // read_rooms is session-only — SDK notification_count is authoritative on startup
        if let Some(json) = self.core.db.get_setting("collapsed_sections") {
            if let Ok(sections) = serde_json::from_str::<Vec<String>>(&json) {
                self.collapsed_sections = sections.into_iter().collect();
            }
        }
        if let Some(val) = self.core.db.get_setting("timestamp_format") {
            self.timestamp_format = val;
        }
        if let Some(val) = self.core.db.get_setting("compact_mode") {
            self.compact_mode = val == "true";
        }
        if let Some(val) = self.core.db.get_setting("show_joinpart") {
            self.show_joinpart = val == "true";
        }
        if let Some(val) = self.core.db.get_setting("zoom_level") {
            if let Ok(level) = val.parse::<u32>() {
                self.zoom_level = level;
            }
        }
        if let Some(val) = self.core.db.get_setting("msg_font_size") {
            if let Ok(size) = val.parse::<f32>() {
                self.msg_font_size = size.clamp(10.0, 24.0);
            }
        }
        if let Some(val) = self.core.db.get_setting("msg_font_family") {
            self.msg_font_family = val;
        }
        if let Some(val) = self.core.db.get_setting("sidebar_font_family") {
            self.sidebar_font_family = val;
        }
        self.available_fonts = crate::view::discover_system_fonts();
        self.available_themes = crate::theme::discover_themes();
        if let Some(val) = self.core.db.get_setting("theme") {
            self.active_theme_name = val.clone();
            if let Some(theme) = self.available_themes.iter().find(|t| t.name == val) {
                crate::theme::set_active_theme(theme.clone());
            }
        }
        if let Some(key) = self.core.db.get_setting("giphy_api_key") {
            self.giphy_api_key = key;
        }
        if let Some(val) = self.core.db.get_setting("notifications_enabled") {
            self.notifications_enabled = val != "false";
        }
        if let Some(val) = self.core.db.get_setting("notify_mentions_only") {
            self.notify_mentions_only = val == "true";
        }
        if let Some(val) = self.core.db.get_setting("notify_show_preview") {
            self.notify_show_preview = val != "false";
        }
        if let Some(val) = self.core.db.get_setting("notify_sound") {
            self.notify_sound = val != "false";
        }
        if let Some(words) = self.core.db.get_setting("highlight_words") {
            self.highlight_words = serde_json::from_str(&words).unwrap_or_default();
        }
        if let Some(json) = self.core.db.get_setting("emoji_freq") {
            self.emoji_freq = serde_json::from_str(&json).unwrap_or_default();
        }
        if let Some(did) = self.core.db.get_setting("device_id") {
            self.device_id = Some(did);
        }
    }
}

/// Convert (line, byte_index) in a multi-line string to a flat byte offset.
fn line_index_to_offset(text: &str, line: usize, index: usize) -> usize {
    let mut offset = 0;
    for (i, text_line) in text.split('\n').enumerate() {
        if i == line {
            return (offset + index).min(text.len());
        }
        offset += text_line.len() + 1; // +1 for the \n
    }
    text.len()
}

/// Create a video player with rotation handling and framerate fallback.
/// Always uses a custom pipeline with `videoflip method=automatic` for correct orientation.
fn create_video_player(uri: &url::Url) -> Result<iced_video_player::Video, iced_video_player::Error> {
    // Always use custom pipeline with videoflip for rotation support
    let pipeline_str = format!(
        "playbin uri=\"{}\" video-sink=\"videoflip method=automatic ! videorate ! videoscale ! videoconvert ! appsink name=iced_video drop=true caps=video/x-raw,format=NV12,pixel-aspect-ratio=1/1\"",
        uri.as_str()
    );
    let pipeline = gstreamer::parse::launch(&pipeline_str)?
        .downcast::<gstreamer::Pipeline>()
        .map_err(|_| iced_video_player::Error::Cast)?;

    let video_sink: gstreamer::Element = pipeline.property("video-sink");
    let pad = video_sink.pads().first().cloned().unwrap();
    let pad = pad.dynamic_cast::<gstreamer::GhostPad>().unwrap();
    let bin = pad
        .parent_element()
        .unwrap()
        .downcast::<gstreamer::Bin>()
        .unwrap();
    let video_sink = bin.by_name("iced_video").unwrap();
    let video_sink = video_sink
        .downcast::<gstreamer_app::AppSink>()
        .map_err(|_| iced_video_player::Error::Cast)?;

    iced_video_player::Video::from_gst_pipeline(pipeline, video_sink, None)
}

/// Extract the first frame of a video as a PNG thumbnail using GStreamer.
/// Reads rotation metadata via GstDiscoverer for reliable orientation.
/// Fits within a 400x400 bounding box, preserving aspect ratio.
fn extract_video_thumbnail(path: &std::path::Path) -> Option<(Vec<u8>, u32, u32)> {
    gstreamer::init().ok()?;
    let uri = url::Url::from_file_path(path).ok()?;

    // Read rotation from container metadata via Discoverer
    let mut rotation = 0u32;
    if let Ok(discoverer) = gstreamer_pbutils::Discoverer::new(gstreamer::ClockTime::from_seconds(5)) {
        if let Ok(info) = discoverer.discover_uri(uri.as_str()) {
            let check_tags = |tags: &gstreamer::TagListRef| -> Option<u32> {
                let val = tags.get::<gstreamer::tags::ImageOrientation>()?;
                let s = val.get();
                tracing::info!("Video orientation tag: {s}");
                match s {
                    "rotate-90" => Some(90),
                    "rotate-180" => Some(180),
                    "rotate-270" => Some(270),
                    _ => None,
                }
            };
            if let Some(tags) = info.tags() {
                if let Some(r) = check_tags(&tags) {
                    rotation = r;
                }
            }
            if rotation == 0 {
                for stream in info.video_streams() {
                    if let Some(tags) = stream.tags() {
                        if let Some(r) = check_tags(&tags) {
                            rotation = r;
                            break;
                        }
                    }
                }
            }
        }
    }

    // Extract raw frame (no videoflip — we rotate manually for reliability)
    let pipeline_str = format!(
        "uridecodebin uri=\"{}\" ! videoconvert ! video/x-raw,format=RGB ! appsink name=sink max-buffers=1 drop=true",
        uri.as_str()
    );
    let pipeline = gstreamer::parse::launch(&pipeline_str).ok()?
        .downcast::<gstreamer::Pipeline>().ok()?;

    let sink = pipeline.by_name("sink")?
        .downcast::<gstreamer_app::AppSink>().ok()?;

    pipeline.set_state(gstreamer::State::Playing).ok()?;
    pipeline.state(gstreamer::ClockTime::from_seconds(5)).0.ok()?;

    let sample = sink.try_pull_sample(gstreamer::ClockTime::from_seconds(5))?;
    let caps = sample.caps()?;
    let s = caps.structure(0)?;
    let width = s.get::<i32>("width").ok()? as u32;
    let height = s.get::<i32>("height").ok()? as u32;

    let buffer = sample.buffer()?;
    let map = buffer.map_readable().ok()?;
    let rgb_data = map.as_slice();

    pipeline.set_state(gstreamer::State::Null).ok()?;

    let img = image::RgbImage::from_raw(width, height, rgb_data.to_vec())?;

    // Apply rotation from metadata
    let rotated = match rotation {
        90 => image::DynamicImage::ImageRgb8(img).rotate90().to_rgb8(),
        180 => image::DynamicImage::ImageRgb8(img).rotate180().to_rgb8(),
        270 => image::DynamicImage::ImageRgb8(img).rotate270().to_rgb8(),
        _ => img,
    };
    let (rw, rh) = rotated.dimensions();

    // Fit within 350x500 CSS-point box
    let max_w = 350u32;
    let max_h = 500u32;
    let scale = (max_w as f64 / rw as f64).min(max_h as f64 / rh as f64).min(1.0);
    let new_w = (rw as f64 * scale) as u32;
    let new_h = (rh as f64 * scale) as u32;

    let resized = image::imageops::resize(&rotated, new_w, new_h, image::imageops::FilterType::Triangle);

    let mut png_buf = Vec::new();
    {
        let encoder = image::codecs::png::PngEncoder::new(&mut png_buf);
        use image::ImageEncoder;
        encoder.write_image(resized.as_raw(), new_w, new_h, image::ExtendedColorType::Rgb8).ok()?;
    }

    tracing::info!("Video thumbnail generated: {new_w}x{new_h} pixels (from {rw}x{rh} rotated frame)");
    Some((png_buf, new_w, new_h))
}

/// Guess MIME type from file extension.
fn mime_from_extension(filename: &str) -> String {
    let ext = filename.rsplit('.').next().unwrap_or("").to_lowercase();
    match ext.as_str() {
        "png" => "image/png",
        "jpg" | "jpeg" => "image/jpeg",
        "gif" => "image/gif",
        "webp" => "image/webp",
        "svg" => "image/svg+xml",
        "mp4" => "video/mp4",
        "webm" => "video/webm",
        "mp3" => "audio/mpeg",
        "ogg" => "audio/ogg",
        "wav" => "audio/wav",
        "pdf" => "application/pdf",
        "txt" => "text/plain",
        _ => "application/octet-stream",
    }
    .to_string()
}

/// Extract URLs from message text (max 2 per message).
pub(crate) fn extract_urls(text: &str) -> Vec<String> {
    text.split_whitespace()
        .map(|w| w.trim_matches(|c: char| c == '`' || c == '<' || c == '>' || c == '"' || c == '\''))
        .filter(|w| (w.starts_with("http://") || w.starts_with("https://")) && w.len() > 10)
        .take(5)
        .map(|w| w.trim_end_matches(['.', ',', ')', ']']).to_string())
        .collect()
}
