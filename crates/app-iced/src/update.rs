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
                self.messages_by_room.insert(room_id.clone(), messages);
                if let Some(members) = members {
                    self.members_by_room.insert(room_id, members);
                }
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
                    self.spaces = spaces;
                    self.invited_rooms = invited;
                }
                SyncEvent::MessagesUpdated(room_id, messages) => {
                    // Advance last_read_event if this is the active room
                    if self.selected_room_id.as_deref() == Some(&room_id) {
                        if let Some(last) = messages.last() {
                            self.last_read_event
                                .insert(room_id.clone(), last.event_id.clone());
                        }
                    }
                    self.messages_by_room.insert(room_id, messages);
                }
                SyncEvent::MembersUpdated(room_id, members) => {
                    self.members_by_room.insert(room_id, members);
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

                if needs_messages || needs_members {
                    let core = self.core.clone();
                    let rid = room_id.clone();
                    let tx = self.timeline_tx.clone();
                    return Task::perform(
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
                                // Return a batch: messages + members
                                Message::TimelineLoaded {
                                    room_id,
                                    messages: msgs,
                                    members,
                                }
                            }
                            None => Message::Noop,
                        },
                    );
                }
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

                    match result {
                        crate::commands::CommandResult::Messages(msgs) => {
                            let mut task = Task::none();
                            for msg in msgs {
                                task = Task::batch([task, self.update(msg)]);
                            }
                            return task;
                        }
                        crate::commands::CommandResult::Rewrite(text) => {
                            // Fall through to send as message
                            self.input_text = text;
                            return self.update(Message::SendMessage);
                        }
                        crate::commands::CommandResult::Help(text) => {
                            self.help_text = Some(text);
                            return Task::none();
                        }
                        crate::commands::CommandResult::Noop => {
                            return Task::none();
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
                    return Task::perform(
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
                }
            }
            Message::CompleteCommand(idx) => {
                if let Some((name, _)) = self.cmd_completions.get(idx) {
                    self.input_text = format!("{name} ");
                    self.cmd_completions.clear();
                }
            }
            Message::CompleteMention(idx) => {
                if let Some((_, display_name)) = self.mention_completions.get(idx) {
                    // Replace @partial with @display_name
                    if let Some(at_pos) = self.input_text.rfind('@') {
                        self.input_text = format!("{}{display_name} ", &self.input_text[..at_pos + 1]);
                    }
                    self.mention_completions.clear();
                }
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
                self.modal = Some(modal);
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
