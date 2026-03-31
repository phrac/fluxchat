use iced::Task;

use crate::message::{LoginSuccess, Message, SyncEvent};
use crate::state::{FluxChat, Screen};

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
            Message::InitialRoomsLoaded(rooms, spaces, invited) => {
                let mut mxc_uris: Vec<String> = spaces.iter()
                    .filter_map(|s| s.avatar_url.clone())
                    .collect();
                mxc_uris.extend(rooms.iter().filter_map(|r| r.avatar_url.clone()));
                self.rooms = rooms;
                self.spaces = spaces;
                self.invited_rooms = invited;
                self.sync_running = true;
                // Load security state for status bar + resolve space/room avatars
                let sec_task = self.update(Message::SecurityLoadState);
                let avatar_task = self.resolve_avatars(mxc_uris);
                return Task::batch([sec_task, avatar_task]);
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
                SyncEvent::RoomListUpdate(rooms, spaces, invited) => {
                    // Zero unread for rooms we've marked as read
                    self.rooms = rooms
                        .into_iter()
                        .map(|mut r| {
                            if self.read_rooms.contains(&r.room_id) {
                                r.unread_count = 0;
                                r.highlight_count = 0;
                            }
                            r
                        })
                        .collect();
                    // Resolve space and room avatars
                    let mut mxc_uris: Vec<String> = spaces.iter()
                        .filter_map(|s| s.avatar_url.clone())
                        .collect();
                    mxc_uris.extend(self.rooms.iter().filter_map(|r| r.avatar_url.clone()));
                    self.spaces = spaces;
                    self.invited_rooms = invited;
                    if !mxc_uris.is_empty() {
                        return self.resolve_avatars(mxc_uris);
                    }
                }
                SyncEvent::MessagesUpdated(room_id, new_messages) => {
                    // Merge new messages with existing ones to preserve history
                    // that the SDK timeline may have trimmed from its buffer.
                    let merged = if let Some(existing) = self.messages_by_room.get(&room_id) {
                        let new_ids: std::collections::HashSet<&str> = new_messages
                            .iter()
                            .map(|m| m.event_id.as_str())
                            .collect();
                        // Keep old messages not present in the new set (trimmed by SDK),
                        // then append all new messages (which include updates to existing ones)
                        let mut merged: Vec<fluxchat_core::MessageInfo> = existing
                            .iter()
                            .filter(|m| !new_ids.contains(m.event_id.as_str()))
                            .cloned()
                            .collect();
                        merged.extend(new_messages);
                        // Sort by timestamp to maintain chronological order
                        merged.sort_by_key(|m| m.timestamp);
                        merged
                    } else {
                        new_messages
                    };
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
                    let is_active = self.selected_room_id.as_deref() == Some(&room_id);
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
                    self.typing_by_room.insert(room_id, user_ids);
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
                let already_loaded = self.messages_by_room.contains_key(&room_id);
                self.selected_room_id = Some(room_id.clone());
                if !already_loaded {
                    self.messages_at_bottom = true;
                }
                self.paginating = false;
                self.paginate_msg_count = 0;
                // Mark as read
                self.read_rooms.insert(room_id.clone());
                // Add to tabs if not already present
                if !self.open_tabs.iter().any(|(id, _, _, _)| id == &room_id) {
                    let room = self.rooms.iter().find(|r| r.room_id == room_id);
                    let name = room
                        .map(|r| r.name.clone())
                        .unwrap_or_else(|| room_id.clone());
                    let space_name = room.and_then(|r| r.space_name.clone());
                    self.open_tabs.push((room_id.clone(), name, space_name, None));
                }

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
                                        // Re-read full timeline items on any diff
                                        let items = timeline.items().await;
                                        let msgs = fluxchat_core::timeline::timeline_items_to_messages(
                                            &items.into_iter().collect::<Vec<_>>(),
                                        );
                                        let _ = tx.send(Message::SyncEvent(
                                            SyncEvent::MessagesUpdated(rid.clone(), msgs),
                                        ));
                                    }
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
                    return Task::batch([load, focus, call_check]);
                }
                return Task::batch([focus, call_check]);
            }
            Message::SelectSpace(space_id) => {
                self.selected_space_id = space_id;
            }
            Message::CloseTab(room_id) => {
                self.open_tabs.retain(|(id, _, _, _)| id != &room_id);
                if self.selected_room_id.as_deref() == Some(&room_id) {
                    self.selected_room_id = self.open_tabs.last().map(|(id, _, _, _)| id.clone());
                }
                self.messages_by_room.remove(&room_id);
                self.members_by_room.remove(&room_id);
            }
            Message::SwitchTab(idx) => {
                if let Some((id, _, _, _)) = self.open_tabs.get(idx) {
                    self.selected_room_id = Some(id.clone());
                }
            }
            Message::ToggleSection(section) => {
                if !self.collapsed_sections.remove(&section) {
                    self.collapsed_sections.insert(section);
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

                // Update command autocomplete
                if text.starts_with('/') && !text.contains(' ') {
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
                // Sync plain text mirror
                let new_text = self.input_content.text();
                // Trim trailing newline that Content always adds
                let new_text = new_text.trim_end_matches('\n').to_string();
                if new_text != self.input_text {
                    // Update text but don't refocus (InputChanged handles it)
                    self.input_text = new_text.clone();

                    // Update command autocomplete
                    if new_text.starts_with('/') && !new_text.contains(' ') {
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

                // Handle slash commands
                if self.input_text.starts_with('/') {
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

                let body = std::mem::take(&mut self.input_text);
                self.input_content = iced::widget::text_editor::Content::new();
                self.help_text = None;
                self.cmd_completions.clear();
                self.mention_completions.clear();

                if let Some(room_id) = self.selected_room_id.clone() {
                    let core = self.core.clone();
                    let reply_to = self.reply_to.take();
                    let edit = self.edit_event.take();
                    let send = Task::perform(
                        async move {
                            let client = core.get_client().await.ok()?;
                            if let Some((event_id, _)) = edit {
                                let _ = fluxchat_core::messages::edit_message(
                                    &client, &room_id, &event_id, &body,
                                )
                                .await;
                            } else if let Some((event_id, _, _)) = reply_to {
                                let _ = fluxchat_core::messages::send_reply(
                                    &client, &room_id, &body, &event_id,
                                )
                                .await;
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
            Message::ToggleReaction(event_id, emoji) => {
                if let Some(room_id) = self.selected_room_id.clone() {
                    let core = self.core.clone();
                    return Task::perform(
                        async move {
                            let client = core.get_client().await.ok()?;
                            let _ = fluxchat_core::messages::send_reaction(
                                &client, &room_id, &event_id, &emoji,
                            )
                            .await;
                            Some(())
                        },
                        |_| Message::Noop,
                    );
                }
            }
            Message::DeleteMessage(event_id) => {
                if let Some(room_id) = self.selected_room_id.clone() {
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
                    let core = self.core.clone();
                    return Task::perform(
                        async move {
                            let client = core.get_client().await.ok()?;
                            let _ = fluxchat_core::messages::pin_message(
                                &client, &room_id, &event_id,
                            )
                            .await;
                            Some(())
                        },
                        |_| Message::Noop,
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
            }
            Message::SearchSubmit => {
                let query = self.search_query.clone();
                if query.trim().is_empty() {
                    return Task::none();
                }
                if let Some(room_id) = self.selected_room_id.clone() {
                    let core = self.core.clone();
                    return Task::perform(
                        async move {
                            let client = core.get_client().await.ok()?;
                            let (results, _next) =
                                fluxchat_core::messages::search_messages(
                                    &client, &room_id, &query, None,
                                )
                                .await
                                .ok()?;
                            Some(results)
                        },
                        |result| match result {
                            Some(results) => Message::SearchResults(results),
                            None => Message::SearchResults(Vec::new()),
                        },
                    );
                }
            }
            Message::SearchResults(results) => {
                self.search_results = results;
            }

            // --- UI panels ---
            Message::ToggleMemberList => {
                self.show_member_list = !self.show_member_list;
            }
            Message::ToggleSearch => {
                self.search_open = !self.search_open;
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
            Message::SetRightPanel(panel) => {
                self.right_panel = panel;
            }

            // --- Modals ---
            Message::OpenModal(modal) => {
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
                    Task::none()
                } else {
                    Task::none()
                };
                self.modal = Some(modal);
                return task;
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

            // --- Room commands ---
            Message::JoinRoom(room_or_alias) => {
                let core = self.core.clone();
                return Task::perform(
                    async move {
                        let client = core.get_client().await.ok()?;
                        fluxchat_core::rooms::join_room(&client, &room_or_alias).await.ok()
                    },
                    |result| match result {
                        Some(room) => Message::SelectRoom(room.room_id),
                        None => Message::Noop,
                    },
                );
            }
            Message::LeaveRoom(room_id) => {
                self.modal = None;
                self.open_tabs.retain(|(id, _, _, _)| id != &room_id);
                if self.selected_room_id.as_deref() == Some(&room_id) {
                    self.selected_room_id = self.open_tabs.last().map(|(id, _, _, _)| id.clone());
                }
                self.messages_by_room.remove(&room_id);
                self.members_by_room.remove(&room_id);
                let core = self.core.clone();
                return Task::perform(
                    async move {
                        let client = core.get_client().await.ok()?;
                        fluxchat_core::rooms::leave_room(&client, &room_id).await.ok()
                    },
                    |_| Message::Noop,
                );
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
                | Message::SecurityLoadState | Message::SecurityStateLoaded(_)
                | Message::SecurityEnableBackup | Message::SecurityBackupEnabled(_)
                | Message::SecurityRecoveryKeyInputChanged(_) | Message::SecurityRecover
                | Message::SecurityRecoverResult(_)
                | Message::ToggleIgnoreUser(_) | Message::ProfilePowerInputChanged(_)
                | Message::ProfileSetPowerLevel | Message::SendDmToUser(_)
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
            }

            // --- Inline images ---
            Message::ImageResolved(mxc, data) => {
                self.image_cache_pending.remove(&mxc);
                // Persist to SQLite
                use base64::Engine;
                let b64 = base64::engine::general_purpose::STANDARD.encode(&data);
                self.core.db.cache_media(&format!("iced_img:{mxc}"), &b64);
                // Try to decode as animated GIF
                let is_gif = data.len() >= 3 && &data[..3] == b"GIF";
                if is_gif {
                    let mxc_clone = mxc.clone();
                    let data_clone = data.clone();
                    let handle = iced::widget::image::Handle::from_bytes(data);
                    self.image_cache.insert(mxc.clone(), handle);
                    return Task::perform(
                        async move {
                            let frames = iced_gif::Frames::from_bytes(data_clone).ok()?;
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
            }
            Message::GifFramesResolved(mxc, frames) => {
                self.gif_frames_cache.insert(mxc, frames);
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
                    return task;
                }
            }
            Message::UrlPreviewImageResolved(url, data) => {
                let handle = iced::widget::image::Handle::from_bytes(data);
                self.url_preview_images.insert(url, handle);
            }

            // --- Message hover ---
            Message::HoverMessage(id) => {
                self.hovered_message = id;
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
                    let core = self.core.clone();
                    return Task::perform(
                        async move {
                            let client = core.get_client().await.ok()?;
                            let is_fav = fluxchat_core::rooms::is_room_favourite(&client, &room_id).await.ok()?;
                            fluxchat_core::rooms::set_room_favourite(&client, &room_id, !is_fav).await.ok()
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
                let core = self.core.clone();
                return Task::perform(
                    async move {
                        let client = core.get_client().await.ok()?;
                        fluxchat_core::rooms::accept_invite(&client, &room_id).await.ok()
                    },
                    |_| Message::Noop,
                );
            }
            Message::RejectInvite(room_id) => {
                let core = self.core.clone();
                return Task::perform(
                    async move {
                        let client = core.get_client().await.ok()?;
                        fluxchat_core::rooms::reject_invite(&client, &room_id).await.ok()
                    },
                    |_| Message::Noop,
                );
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
                let space_id = self.create_room_space.clone();
                self.modal = None;
                self.create_room_name.clear();
                self.create_room_topic.clear();
                self.create_room_public = false;
                self.create_room_encrypted = false;
                self.create_room_space = None;
                let core = self.core.clone();
                return Task::perform(
                    async move {
                        let client = core.get_client().await.ok()?;
                        let room_id = fluxchat_core::rooms::create_room(
                            &client, &name, topic.as_deref(), is_public, encrypted, &[], space_id.as_deref(),
                        ).await.ok()?;
                        Some(room_id)
                    },
                    |result| match result {
                        Some(room_id) => Message::SelectRoom(room_id),
                        None => Message::Noop,
                    },
                );
            }

            // --- Room settings ---
            Message::RoomSettingsNameChanged(name) => {
                self.room_settings_name = name;
            }
            Message::RoomSettingsTopicChanged(topic) => {
                self.room_settings_topic = topic;
            }
            Message::RoomSettingsSaveName => {
                if let Some(room_id) = self.selected_room_id.clone() {
                    let name = self.room_settings_name.clone();
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
                            fluxchat_core::rooms::upload_room_avatar(&client, &room_id, mime, data.to_vec()).await.ok()
                        },
                        |_| Message::Noop,
                    );
                }
            }
            Message::RoomSettingsRemoveAvatar => {
                if let Some(room_id) = self.selected_room_id.clone() {
                    let core = self.core.clone();
                    return Task::perform(
                        async move {
                            let client = core.get_client().await.ok()?;
                            fluxchat_core::rooms::remove_room_avatar(&client, &room_id).await.ok()
                        },
                        |_| Message::Noop,
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
                if let Some(room_id) = self.selected_room_id.clone() {
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

            Message::ReadReceiptsLoaded(room_id, receipts) => {
                self.receipts_by_room.insert(room_id, receipts);
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
                self.messages_at_bottom = near_bottom;
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
            Message::SidebarDrag(x) => {
                if self.sidebar_dragging {
                    // x is the cursor position; subtract the space sidebar width (~56px)
                    let new_width = (x - 56.0).clamp(150.0, 400.0);
                    self.sidebar_width = new_width;
                }
            }
            Message::OpenUrl(url) => {
                let _ = open::that(&url);
            }
            Message::Escape => {
                // Priority: overflow > completions > modal > panels > reply/edit
                if self.overflow_menu_open {
                    self.overflow_menu_open = false;
                } else if !self.cmd_completions.is_empty() {
                    self.cmd_completions.clear();
                } else if !self.mention_completions.is_empty() {
                    self.mention_completions.clear();
                } else if self.modal.is_some() {
                    self.modal = None;
                } else if self.search_open {
                    self.search_open = false;
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

                    // Periodic room list refresh
                    let core = self.core.clone();
                    tasks.push(Task::perform(
                        async move {
                            let client = core.get_client().await.ok()?;
                            let rooms = fluxchat_core::rooms::get_rooms(&client).await.ok()?;
                            let spaces = fluxchat_core::rooms::get_spaces(&client).await.ok()?;
                            let invited =
                                fluxchat_core::rooms::get_invited_rooms(&client).await.ok()?;
                            Some((rooms, spaces, invited))
                        },
                        |result| match result {
                            Some((rooms, spaces, invited)) => {
                                Message::SyncEvent(SyncEvent::RoomListUpdate(
                                    rooms, spaces, invited,
                                ))
                            }
                            None => Message::Noop,
                        },
                    ));

                    // Load read receipts for current room
                    if let Some(ref room_id) = self.selected_room_id {
                        let core2 = self.core.clone();
                        let rid = room_id.clone();
                        let event_ids: Vec<String> = self.messages_by_room
                            .get(room_id)
                            .map(|msgs| msgs.iter().map(|m| m.event_id.clone()).collect())
                            .unwrap_or_default();
                        if !event_ids.is_empty() {
                            tasks.push(Task::perform(
                                async move {
                                    let client = core2.get_client().await.ok()?;
                                    let receipts = fluxchat_core::messages::get_room_read_receipts(
                                        &client, &rid, &event_ids,
                                    ).await.ok()?;
                                    Some((rid, receipts))
                                },
                                |result| match result {
                                    Some((rid, receipts)) => Message::ReadReceiptsLoaded(rid, receipts),
                                    None => Message::Noop,
                                },
                            ));
                        }
                    }

                    // Scan selected room for active calls (for join banner)
                    if let Some(ref room_id) = self.selected_room_id {
                        let core3 = self.core.clone();
                        let rid = room_id.clone();
                        let is_connected = self.call_state == fluxchat_core::rtc::CallConnectionState::Connected;
                        tasks.push(Task::perform(
                            async move {
                                let client = core3.get_client().await.ok()?;
                                let participants = fluxchat_core::rtc::get_call_members(&client, &rid).await.ok()?;
                                Some((rid, participants, is_connected))
                            },
                            |result| match result {
                                Some((rid, participants, is_connected)) => {
                                    if is_connected {
                                        // Don't overwrite participants when connected — just update room indicator
                                        let mut rooms = std::collections::HashSet::new();
                                        if !participants.is_empty() {
                                            rooms.insert(rid);
                                        }
                                        Message::CallRoomsWithCallsUpdated(rooms)
                                    } else {
                                        Message::CallParticipantsUpdated(rid, participants)
                                    }
                                }
                                None => Message::Noop,
                            },
                        ));
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
                        let is_gif = data.len() >= 3 && &data[..3] == b"GIF";
                        let handle = iced::widget::image::Handle::from_bytes(data.clone());
                        self.image_cache.insert(mxc.clone(), handle);
                        if is_gif && !self.gif_frames_cache.contains_key(mxc) {
                            let mxc_clone = mxc.clone();
                            tasks.push(Task::perform(
                                async move {
                                    let frames = iced_gif::Frames::from_bytes(data).ok()?;
                                    Some((mxc_clone, std::sync::Arc::new(frames)))
                                },
                                |result| match result {
                                    Some((mxc, frames)) => Message::GifFramesResolved(mxc, frames),
                                    None => Message::Noop,
                                },
                            ));
                        }
                        continue;
                    }
                }
                self.image_cache_pending.insert(mxc.clone());
                let core = self.core.clone();
                let mxc_clone = mxc.clone();
                tasks.push(Task::perform(
                    async move {
                        use matrix_sdk::media::{MediaRequestParameters, MediaFormat};
                        use matrix_sdk::ruma::events::room::MediaSource;
                        use matrix_sdk::ruma::MxcUri;
                        let client = core.get_client().await.ok()?;
                        let mxc_uri = <&MxcUri>::from(mxc_clone.as_str());
                        let req = MediaRequestParameters {
                            source: MediaSource::Plain(mxc_uri.to_owned()),
                            format: MediaFormat::File,
                        };
                        let data = client.media().get_media_content(&req, true).await.ok()?;
                        Some((mxc_clone, data))
                    },
                    |result| match result {
                        Some((mxc, data)) => Message::ImageResolved(mxc, data),
                        None => Message::Noop,
                    },
                ));
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

                // Start sync service
                match fluxchat_core::sync_service::create_sync_service(&client).await {
                    Ok(sync_service) => {
                        *core.sync_service.lock().await = Some(sync_service);
                    }
                    Err(e) => {
                        tracing::warn!("SyncService failed ({e}), using basic sync");
                    }
                }

                // Also start the legacy sync for typing/verification callbacks
                // (using a no-op bridge since Iced subscription handles room list)
                fluxchat_core::sync::spawn_sync(client.clone(), NoopSyncBridge);

                // Load initial rooms
                let rooms = fluxchat_core::rooms::get_rooms(&client).await.ok()?;
                let spaces = fluxchat_core::rooms::get_spaces(&client).await.ok()?;
                let invited = fluxchat_core::rooms::get_invited_rooms(&client).await.ok()?;

                Some((rooms, spaces, invited))
            },
            |result| match result {
                Some((rooms, spaces, invited)) => {
                    // We'll handle this with a batch: mark sync started + load rooms
                    Message::InitialRoomsLoaded(rooms, spaces, invited)
                }
                None => Message::SyncStarted,
            },
        )
    }

    /// Load persisted settings from SQLite into state.
    fn load_persisted_settings(&mut self) {
        if let Some(json) = self.core.db.get_setting("read_rooms") {
            if let Ok(ids) = serde_json::from_str::<Vec<String>>(&json) {
                self.read_rooms = ids.into_iter().collect();
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
        if let Some(key) = self.core.db.get_setting("giphy_api_key") {
            self.giphy_api_key = key;
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

/// No-op sync bridge — Iced subscription handles events directly.
/// We only need spawn_sync running for typing/verification callbacks via the SDK.
struct NoopSyncBridge;

impl fluxchat_core::sync::SyncCallbacks for NoopSyncBridge {
    fn on_message(&self, _: String, _: fluxchat_core::MessageInfo) {}
    fn on_edit(&self, _: String, _: String, _: String, _: Option<String>) {}
    fn on_reaction(&self, _: String, _: String, _: String, _: String) {}
    fn on_typing(&self, _: String, _: Vec<String>) {}
    fn on_membership_change(&self, _: String, _: fluxchat_core::MessageInfo) {}
    fn on_room_list_update(&self) {}
    fn on_verification_request(&self, _: fluxchat_core::VerificationInfo) {}
}

/// Extract URLs from message text (max 2 per message).
pub(crate) fn extract_urls(text: &str) -> Vec<String> {
    text.split_whitespace()
        .map(|w| w.trim_matches(|c: char| c == '`' || c == '<' || c == '>' || c == '"' || c == '\''))
        .filter(|w| (w.starts_with("http://") || w.starts_with("https://")) && w.len() > 10)
        .take(2)
        .map(|w| w.trim_end_matches(['.', ',', ')', ']']).to_string())
        .collect()
}
