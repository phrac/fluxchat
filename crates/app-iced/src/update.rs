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
                self.rooms = rooms;
                self.spaces = spaces;
                self.invited_rooms = invited;
                self.sync_running = true;
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
                self.messages_by_room.insert(room_id.clone(), messages);
                if let Some(members) = members {
                    self.members_by_room.insert(room_id, members);
                }
                return Task::batch([self.resolve_avatars(mxc_uris), preview_task, image_task]);
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
                SyncEvent::MessagesUpdated(room_id, messages) => {
                    // Advance last_read_event if this is the active room
                    if self.selected_room_id.as_deref() == Some(&room_id) {
                        if let Some(last) = messages.last() {
                            self.last_read_event
                                .insert(room_id.clone(), last.event_id.clone());
                        }
                    }
                    let mxc_uris: Vec<String> = messages
                        .iter()
                        .filter_map(|m| m.avatar_url.clone())
                        .collect();
                    let preview_task = self.resolve_url_previews(&messages);
                    let image_task = self.resolve_images(&messages);
                    self.messages_by_room.insert(room_id, messages);
                    return Task::batch([self.resolve_avatars(mxc_uris), preview_task, image_task]);
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
                self.selected_room_id = Some(room_id.clone());
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
                    return Task::batch([load, focus]);
                }
                return focus;
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
                // Keep input focused when completions popup appears
                if !self.cmd_completions.is_empty() || !self.mention_completions.is_empty() {
                    return iced::widget::operation::focus(crate::view::input_id());
                }
            }
            Message::SendMessage => {
                if self.input_text.trim().is_empty() {
                    return Task::none();
                }

                // Handle slash commands
                if self.input_text.starts_with('/') {
                    let result = crate::commands::dispatch(&self.input_text, self);
                    self.input_text.clear();
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
                    return Task::batch([send, iced::widget::operation::focus(crate::view::input_id())]);
                }
            }
            Message::CompleteCommand(idx) => {
                if let Some((name, _)) = self.cmd_completions.get(idx) {
                    self.input_text = format!("{name} ");
                    self.cmd_completions.clear();
                }
                return iced::widget::operation::focus(crate::view::input_id());
            }
            Message::CompleteMention(idx) => {
                if let Some((_, display_name)) = self.mention_completions.get(idx) {
                    // Replace @partial with @display_name
                    if let Some(at_pos) = self.input_text.rfind('@') {
                        self.input_text = format!("{}{display_name} ", &self.input_text[..at_pos + 1]);
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
                self.input_text = body;
            }
            Message::CancelEdit => {
                self.edit_event = None;
                self.input_text.clear();
            }

            // --- Message actions ---
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
            }
            Message::ToggleThreads => {
                self.threads_open = !self.threads_open;
            }
            Message::ToggleFiles => {
                self.files_open = !self.files_open;
            }
            Message::ToggleNotices => {
                self.notices_open = !self.notices_open;
            }
            Message::ToggleGifs => {
                self.gifs_open = !self.gifs_open;
            }
            Message::SetRightPanel(panel) => {
                self.right_panel = panel;
            }

            // --- Modals ---
            Message::OpenModal(modal) => {
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

            // --- Settings ---
            Message::SettingsNickChanged(s) => {
                self.settings_nick_input = s;
            }
            Message::SettingsSaveNick => {
                let nick = self.settings_nick_input.clone();
                if nick.trim().is_empty() {
                    return Task::none();
                }
                self.display_name = Some(nick.clone());
                let core = self.core.clone();
                return Task::perform(
                    async move {
                        let client = core.get_client().await.ok()?;
                        fluxchat_core::profile::set_display_name(&client, &nick).await.ok();
                        Some(())
                    },
                    |_| Message::SettingsStatus("Display name updated".into()),
                );
            }
            Message::SettingsLoadDevices => {
                let core = self.core.clone();
                return Task::perform(
                    async move {
                        let client = core.get_client().await.ok()?;
                        fluxchat_core::devices::list_devices(&client).await.ok()
                    },
                    |result| match result {
                        Some(devices) => Message::SettingsDevicesLoaded(devices),
                        None => Message::Noop,
                    },
                );
            }
            Message::SettingsDevicesLoaded(devices) => {
                self.settings_devices = devices;
            }
            Message::SettingsSignOutDevice(device_id) => {
                self.settings_signout_device = Some(device_id);
                self.settings_signout_password.clear();
            }
            Message::SettingsSignOutPasswordChanged(s) => {
                self.settings_signout_password = s;
            }
            Message::SettingsSignOutConfirm => {
                if let Some(device_id) = self.settings_signout_device.take() {
                    let password = std::mem::take(&mut self.settings_signout_password);
                    let core = self.core.clone();
                    return Task::perform(
                        async move {
                            let client = core.get_client().await.ok()?;
                            fluxchat_core::devices::delete_devices(&client, &[device_id], &password).await.ok()
                        },
                        |_| Message::SettingsLoadDevices,
                    );
                }
            }
            Message::SettingsStatus(s) => {
                self.settings_status = Some(s);
            }
            Message::SettingsLoadProfile => {
                let core = self.core.clone();
                return Task::perform(
                    async move {
                        use matrix_sdk::media::{MediaRequestParameters, MediaFormat};
                        use matrix_sdk::ruma::events::room::MediaSource;
                        use matrix_sdk::ruma::MxcUri;
                        let client = core.get_client().await.ok()?;
                        let profile = fluxchat_core::profile::get_profile(&client).await.ok()?;
                        let avatar_data = if let Some(ref mxc) = profile.avatar_url {
                            let mxc_uri = <&MxcUri>::from(mxc.as_str());
                            let req = MediaRequestParameters { source: MediaSource::Plain(mxc_uri.to_owned()), format: MediaFormat::File };
                            client.media().get_media_content(&req, true).await.ok()
                        } else { None };
                        let banner_data = if let Some(ref mxc) = profile.banner_url {
                            let mxc_uri = <&MxcUri>::from(mxc.as_str());
                            let req = MediaRequestParameters { source: MediaSource::Plain(mxc_uri.to_owned()), format: MediaFormat::File };
                            client.media().get_media_content(&req, true).await.ok()
                        } else { None };
                        Some((avatar_data, banner_data))
                    },
                    |result| match result {
                        Some((avatar_data, banner_data)) => Message::SettingsProfileLoaded { avatar_data, banner_data },
                        None => Message::Noop,
                    },
                );
            }
            Message::SettingsProfileLoaded { avatar_data, banner_data } => {
                self.settings_avatar_data = avatar_data;
                self.settings_banner_data = banner_data;
            }
            Message::SettingsUploadAvatar => {
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
                        fluxchat_core::profile::upload_avatar(&client, mime, data.to_vec()).await.ok()?;
                        Some(())
                    },
                    |result| match result {
                        Some(()) => Message::SettingsStatus("avatar updated".into()),
                        None => Message::Noop,
                    },
                );
            }
            Message::SettingsRemoveAvatar => {
                let core = self.core.clone();
                return Task::perform(
                    async move {
                        let client = core.get_client().await.ok()?;
                        fluxchat_core::profile::remove_avatar(&client).await.ok()
                    },
                    |result| match result {
                        Some(()) => Message::SettingsStatus("avatar removed".into()),
                        None => Message::Noop,
                    },
                );
            }
            Message::SettingsUploadBanner => {
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
                        fluxchat_core::profile::upload_banner(&client, mime, data.to_vec()).await.ok()?;
                        Some(())
                    },
                    |result| match result {
                        Some(()) => Message::SettingsStatus("banner updated".into()),
                        None => Message::Noop,
                    },
                );
            }
            Message::SettingsRemoveBanner => {
                let core = self.core.clone();
                return Task::perform(
                    async move {
                        let client = core.get_client().await.ok()?;
                        fluxchat_core::profile::remove_banner(&client).await.ok()
                    },
                    |result| match result {
                        Some(()) => Message::SettingsStatus("banner removed".into()),
                        None => Message::Noop,
                    },
                );
            }

            // --- Security ---
            Message::SecurityLoadState => {
                self.security_state = None;
                let core = self.core.clone();
                return Task::perform(
                    async move {
                        let client = core.get_client().await.ok()?;
                        let encryption = client.encryption();

                        let backup_exists = encryption.backups().exists_on_server().await.unwrap_or(false);
                        let backup_enabled = encryption.backups().are_enabled().await;
                        let recovery_state = encryption.recovery().state();

                        let own_device = {
                            let user_id = client.user_id()?;
                            let device_id = client.device_id()?;
                            encryption.get_device(user_id, device_id).await.ok()??
                        };
                        let is_verified = own_device.is_verified_with_cross_signing();

                        Some(crate::state::SecurityState {
                            is_verified,
                            backup_exists,
                            backup_enabled,
                            recovery_state: format!("{recovery_state:?}"),
                        })
                    },
                    |result| match result {
                        Some(state) => Message::SecurityStateLoaded(state),
                        None => Message::Noop,
                    },
                );
            }
            Message::SecurityStateLoaded(state) => {
                self.security_state = Some(state);
            }
            Message::SecurityEnableBackup => {
                self.settings_status = Some("enabling backup...".into());
                let core = self.core.clone();
                return Task::perform(
                    async move {
                        let client = core.get_client().await.ok()?;
                        match client.encryption().recovery().enable().await {
                            Ok(key) => Some(Some(key)),
                            Err(_) => {
                                // Try just enabling backup without full recovery
                                match client.encryption().recovery().enable_backup().await {
                                    Ok(_) => Some(None),
                                    Err(_) => None,
                                }
                            }
                        }
                    },
                    |result| match result {
                        Some(key) => Message::SecurityBackupEnabled(key),
                        None => Message::SettingsStatus("failed to enable backup".into()),
                    },
                );
            }
            Message::SecurityBackupEnabled(key) => {
                if key.is_some() {
                    self.settings_status = Some("backup enabled! save your recovery key".into());
                } else {
                    self.settings_status = Some("backup enabled".into());
                }
                self.security_shown_recovery_key = key;
                // Refresh security state
                return self.update(Message::SecurityLoadState);
            }
            Message::SecurityRecoveryKeyInputChanged(s) => {
                self.security_recovery_key_input = s;
            }
            Message::SecurityRecover => {
                let key = self.security_recovery_key_input.trim().to_string();
                if key.is_empty() {
                    return Task::none();
                }
                self.settings_status = Some("recovering...".into());
                let core = self.core.clone();
                return Task::perform(
                    async move {
                        let client = core.get_client().await.map_err(|e| format!("{e}"))?;
                        client.encryption().recovery().recover(&key).await.map_err(|e| format!("{e}"))
                    },
                    Message::SecurityRecoverResult,
                );
            }
            Message::SecurityRecoverResult(result) => {
                match result {
                    Ok(()) => {
                        self.settings_status = Some("recovery successful! keys restored.".into());
                        self.security_recovery_key_input.clear();
                        return self.update(Message::SecurityLoadState);
                    }
                    Err(e) => {
                        self.settings_status = Some(format!("recovery failed: {e}"));
                    }
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
                let handle = iced::widget::image::Handle::from_bytes(data);
                self.image_cache.insert(mxc, handle);
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

            // --- Formatting ---
            Message::InsertFormat(prefix, suffix) => {
                self.input_text.push_str(prefix);
                self.input_text.push_str(suffix);
                return iced::widget::operation::focus(crate::view::input_id());
            }

            // --- Misc ---
            Message::PaginateBack(_room_id) => { /* TODO */ }
            Message::OpenUrl(url) => {
                let _ = open::that(&url);
            }
            Message::Escape => {
                // Priority: completions > modal > panels > reply/edit
                if !self.cmd_completions.is_empty() {
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
                } else if self.help_text.is_some() {
                    self.help_text = None;
                } else if self.reply_to.is_some() {
                    self.reply_to = None;
                } else if self.edit_event.is_some() {
                    self.edit_event = None;
                    self.input_text.clear();
                }
            }
            Message::Tick => {
                // Periodic room list refresh
                if self.sync_running {
                    let core = self.core.clone();
                    return Task::perform(
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
                    );
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
                        let handle = iced::widget::image::Handle::from_bytes(data);
                        self.image_cache.insert(mxc.clone(), handle);
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
