use iced::Task;

use fluxchat_core::room_admin;

use crate::message::Message;
use crate::state::{FluxChat, Modal};

impl FluxChat {
    /// Load the room's security settings and permissions for Room Settings.
    pub(crate) fn load_room_admin(&self, room_id: String) -> Task<Message> {
        let core = self.core.clone();
        let rid = room_id.clone();
        let security = Task::perform(
            async move {
                let r = match core.get_client().await {
                    Ok(client) => room_admin::room_security(&client, &rid).await.map_err(|e| e.to_string()),
                    Err(e) => Err(e.to_string()),
                };
                (rid, r)
            },
            |(rid, r)| Message::RoomSecurityLoaded(rid, r),
        );
        let core = self.core.clone();
        let rid = room_id.clone();
        let perms = Task::perform(
            async move {
                let r = match core.get_client().await {
                    Ok(client) => room_admin::room_permissions(&client, &rid).await.map_err(|e| e.to_string()),
                    Err(e) => Err(e.to_string()),
                };
                (rid, r)
            },
            |(rid, r)| Message::RoomPermsLoaded(rid, r),
        );
        let core = self.core.clone();
        let advanced = Task::perform(
            async move {
                let r = match core.get_client().await {
                    Ok(client) => room_admin::room_advanced(&client, &room_id).await.map(Box::new).map_err(|e| e.to_string()),
                    Err(e) => Err(e.to_string()),
                };
                (room_id, r)
            },
            |(rid, r)| Message::RoomAdvancedLoaded(rid, r),
        );
        Task::batch([security, perms, advanced])
    }

    /// Follow the join requests of the room in view (`room_id`, a real
    /// room): stops the previous room's watcher and starts this one's.
    pub(crate) fn watch_join_requests(&mut self, room_id: &str) -> Task<Message> {
        if self.join_requests_watch.as_ref().is_some_and(|(r, _)| r == room_id) {
            return Task::none();
        }
        self.join_requests_watch = None;
        self.join_requests = None;
        self.join_requests_open = false;
        // DMs and rooms we're not joined to have nobody to approve.
        if !self.rooms.iter().any(|r| r.room_id == room_id && !r.is_direct) {
            return Task::none();
        }
        let core = self.core.clone();
        let tx = self.timeline_tx.clone();
        let rid = room_id.to_string();
        Task::perform(
            async move {
                let client = core.get_client().await.ok()?;
                let r = rid.clone();
                let handle = room_admin::watch_join_requests(&client, &rid, move |requests| {
                    let _ = tx.send(Message::JoinRequestsUpdated(r.clone(), requests));
                }).await
                    .map_err(|e| tracing::warn!("Can't watch join requests in {rid}: {e}"))
                    .ok()?;
                Some((rid, crate::state::TaskGuard::new(&handle)))
            },
            |r| match r {
                Some((rid, guard)) => Message::JoinRequestsWatching(rid, guard),
                None => Message::Noop,
            },
        )
    }

    /// The room whose settings dialog is open.
    fn room_settings_room(&self) -> Option<String> {
        matches!(self.modal, Some(Modal::RoomSettings))
            .then(|| self.active_real_room().map(str::to_string))
            .flatten()
    }

    /// Run a room-admin change, then reload (errors go to the notice line).
    fn room_admin_change<F, Fut>(&self, change: F) -> Task<Message>
    where
        F: FnOnce(fluxchat_core::Client, String) -> Fut + Send + 'static,
        Fut: std::future::Future<Output = Result<(), fluxchat_core::AppError>> + Send + 'static,
    {
        let Some(room_id) = self.room_settings_room() else { return Task::none() };
        let core = self.core.clone();
        Task::perform(
            async move {
                let err = match core.get_client().await {
                    Ok(client) => change(client, room_id.clone()).await.err().map(|e| e.to_string()),
                    Err(e) => Some(e.to_string()),
                };
                (room_id, err)
            },
            |(rid, err)| Message::RoomAdminDone(rid, err),
        )
    }

    /// Handle Room Settings security/roles messages. Returns Some(task) if handled.
    pub(crate) fn handle_room_admin_message(&mut self, message: Message) -> Option<Task<Message>> {
        let task = match message {
            Message::RoomSettingsTabChanged(tab) => {
                self.room_settings_tab = tab;
                crate::view::scroll_dialog_to_top()
            }
            Message::RoomSecurityLoaded(room_id, result) => {
                if self.room_settings_room().as_deref() == Some(room_id.as_str()) {
                    match result {
                        Ok(sec) => self.room_security = Some(sec),
                        Err(e) => return Some(self.update(Message::ShowError(format!("Couldn't load room security: {e}")))),
                    }
                }
                Task::none()
            }
            Message::RoomPermsLoaded(room_id, result) => {
                if self.room_settings_room().as_deref() == Some(room_id.as_str()) {
                    match result {
                        Ok(perms) => self.room_perms = Some(perms),
                        Err(e) => return Some(self.update(Message::ShowError(format!("Couldn't load permissions: {e}")))),
                    }
                }
                Task::none()
            }
            Message::SetJoinMode(mode) => {
                // "Space members" allows the spaces this room is in.
                let spaces = self.room_parent_spaces.clone().unwrap_or_default();
                if let Some(sec) = self.room_security.as_mut() {
                    sec.join = mode;
                }
                self.room_admin_change(move |client, rid| async move {
                    room_admin::set_join_mode(&client, &rid, mode, &spaces).await
                })
            }
            Message::SetHistoryMode(mode) => {
                if let Some(sec) = self.room_security.as_mut() {
                    sec.history = mode;
                }
                self.room_admin_change(move |client, rid| async move {
                    room_admin::set_history_mode(&client, &rid, mode).await
                })
            }
            Message::SetGuestAccess(on) => {
                if let Some(sec) = self.room_security.as_mut() {
                    sec.guest_access = on;
                }
                self.room_admin_change(move |client, rid| async move {
                    room_admin::set_guest_access(&client, &rid, on).await
                })
            }
            Message::SetInDirectory(on) => {
                if let Some(sec) = self.room_security.as_mut() {
                    sec.in_directory = on;
                }
                self.room_admin_change(move |client, rid| async move {
                    room_admin::set_in_directory(&client, &rid, on).await
                })
            }
            Message::SetPermission(permission, level) => {
                if let Some((perms, _)) = self.room_perms.as_mut() {
                    perms.levels.insert(permission.clone(), level);
                }
                self.room_admin_change(move |client, rid| async move {
                    room_admin::set_permission(&client, &rid, &permission, level).await
                })
            }
            Message::JoinRequestsWatching(room_id, guard) => {
                // Kept only if we're still on that room (else dropped: stops).
                if self.active_real_room() == Some(room_id.as_str()) && self.join_requests_watch.is_none() {
                    self.join_requests_watch = Some((room_id, guard));
                }
                Task::none()
            }
            Message::JoinRequestsUpdated(room_id, requests) => {
                if self.join_requests_watch.as_ref().is_some_and(|(r, _)| *r == room_id) {
                    if requests.requests.is_empty() {
                        self.join_requests_open = false;
                    }
                    self.join_requests = Some((room_id, requests));
                }
                Task::none()
            }
            Message::ToggleJoinRequests => {
                self.join_requests_open = !self.join_requests_open;
                Task::none()
            }
            Message::RespondJoinRequest(user_id, response) => {
                let Some((room_id, reqs)) = self.join_requests.as_mut() else { return Some(Task::none()) };
                // Gone at once; the watcher re-sends the list if it fails.
                reqs.requests.retain(|r| r.user_id != user_id);
                if reqs.requests.is_empty() {
                    self.join_requests_open = false;
                }
                let room_id = room_id.clone();
                let core = self.core.clone();
                Task::perform(
                    async move {
                        match core.get_client().await {
                            Ok(client) => room_admin::respond_join_request(&client, &room_id, &user_id, response).await.err().map(|e| e.to_string()),
                            Err(e) => Some(e.to_string()),
                        }
                    },
                    |err| match err {
                        Some(e) => Message::ShowError(format!("Couldn't answer the join request: {e}")),
                        None => Message::Noop,
                    },
                )
            }
            Message::RoomAdvancedLoaded(room_id, result) => {
                if self.room_settings_room().as_deref() == Some(room_id.as_str()) {
                    match result {
                        Ok(adv) => self.room_advanced = Some(*adv),
                        Err(e) => return Some(self.update(Message::ShowError(format!("Couldn't load room details: {e}")))),
                    }
                }
                Task::none()
            }
            Message::RoomAliasInputChanged(value) => {
                self.room_alias_input = value;
                Task::none()
            }
            Message::AddRoomAlias => {
                let Some(adv) = &self.room_advanced else { return Some(Task::none()) };
                let Some(alias) = room_admin::full_alias(&self.room_alias_input, &adv.server) else {
                    return Some(self.update(Message::ShowError("Addresses look like #name:server, with no spaces".into())));
                };
                self.room_alias_input.clear();
                // `add_room_alias` also makes it the main address if there's none.
                self.room_admin_change(move |client, rid| async move {
                    fluxchat_core::rooms::add_room_alias(&client, &rid, &alias).await
                })
            }
            Message::RemoveRoomAlias(alias) => {
                if let Some(adv) = self.room_advanced.as_mut() {
                    adv.local_aliases.retain(|a| *a != alias);
                }
                self.room_admin_change(move |client, rid| async move {
                    room_admin::remove_local_alias(&client, &rid, &alias).await
                })
            }
            Message::SetMainAlias(main) => {
                let Some(adv) = self.room_advanced.as_mut() else { return Some(Task::none()) };
                // The old main address stays published, as an alternative.
                let mut alts = adv.alt_aliases.clone();
                if let Some(old) = adv.main_alias.take().filter(|old| Some(old) != main.as_ref()) {
                    alts.insert(0, old);
                }
                alts.retain(|a| Some(a) != main.as_ref());
                adv.main_alias = main.clone();
                adv.alt_aliases = alts.clone();
                self.room_admin_change(move |client, rid| async move {
                    room_admin::set_published_aliases(&client, &rid, main.as_deref(), &alts).await
                })
            }
            Message::SetAliasPublished(alias, publish) => {
                let Some(adv) = self.room_advanced.as_mut() else { return Some(Task::none()) };
                if publish {
                    if !adv.alt_aliases.contains(&alias) {
                        adv.alt_aliases.push(alias);
                    }
                } else {
                    adv.alt_aliases.retain(|a| *a != alias);
                }
                let (main, alts) = (adv.main_alias.clone(), adv.alt_aliases.clone());
                self.room_admin_change(move |client, rid| async move {
                    room_admin::set_published_aliases(&client, &rid, main.as_deref(), &alts).await
                })
            }
            Message::ConfirmRoomUpgrade(ask) => {
                self.room_upgrade_confirm = ask;
                Task::none()
            }
            Message::RoomUpgraded(old_room, result) => {
                self.room_upgrading = false;
                self.room_upgrade_confirm = false;
                match result {
                    Ok(new_room) => {
                        // Go to the new room; the old one now points there.
                        // We're already in it, but the room list hasn't
                        // caught up: show it as the old room until it does.
                        if self.room_settings_room().as_deref() == Some(old_room.as_str()) {
                            self.modal = None;
                        }
                        if let Some(old) = self.rooms.iter_mut().find(|r| r.room_id == old_room) {
                            old.successor = Some(new_room.clone());
                            let mut info = old.clone();
                            info.room_id = new_room.clone();
                            info.successor = None;
                            info.unread_count = 0;
                            info.highlight_count = 0;
                            if !self.rooms.iter().any(|r| r.room_id == new_room) {
                                self.rooms.push(info);
                            }
                        }
                        self.update(Message::SelectRoom(new_room))
                    }
                    Err(e) => self.update(Message::ShowError(format!("Couldn't upgrade the room: {e}"))),
                }
            }
            Message::RoomAdminDone(room_id, err) => {
                // Reload either way: on failure this puts the controls back.
                let reload = if self.room_settings_room().as_deref() == Some(room_id.as_str()) {
                    self.load_room_admin(room_id)
                } else {
                    Task::none()
                };
                match err {
                    Some(e) => Task::batch([reload, self.update(Message::ShowError(format!("Couldn't save: {e}")))]),
                    None => reload,
                }
            }
            _ => return None,
        };
        Some(task)
    }
}

#[cfg(test)]
mod tests {
    use fluxchat_core::room_admin::{JoinRequest, JoinRequests, JoinResponse};

    use crate::message::Message;
    use crate::panes::flow_tests::app;

    fn room(id: &str) -> fluxchat_core::RoomInfo {
        fluxchat_core::RoomInfo {
            room_id: id.into(), name: id.into(), is_encrypted: false, is_space: false, is_direct: false,
            unread_count: 0, highlight_count: 0, avatar_url: None, topic: None, space_name: None,
            space_id: None, is_favourite: false, successor: None,
        }
    }

    fn requests(users: &[&str]) -> JoinRequests {
        JoinRequests {
            can_respond: true,
            requests: users.iter().map(|u| JoinRequest {
                user_id: u.to_string(), display_name: None, avatar_url: None, reason: None, timestamp: None,
            }).collect(),
        }
    }

    #[tokio::test]
    async fn join_requests_follow_the_room_in_view() {
        let mut app = app();
        app.rooms = vec![room("!a:x"), room("!b:x")];
        let _ = app.update(Message::SelectRoom("!a:x".into()));
        // Updates before the watcher is registered are dropped.
        let _ = app.update(Message::JoinRequestsUpdated("!a:x".into(), requests(&["@u:x"])));
        assert!(app.join_requests.is_none());

        let guard = crate::state::TaskGuard::new(&tokio::spawn(async {}));
        let _ = app.update(Message::JoinRequestsWatching("!a:x".into(), guard));
        let _ = app.update(Message::JoinRequestsUpdated("!a:x".into(), requests(&["@u:x", "@v:x"])));
        let _ = app.update(Message::ToggleJoinRequests);
        assert!(app.join_requests_open);

        // Answering removes the row at once; the last one closes the list.
        let _ = app.update(Message::RespondJoinRequest("@u:x".into(), JoinResponse::Approve));
        let _ = app.update(Message::RespondJoinRequest("@v:x".into(), JoinResponse::Deny));
        assert!(app.join_requests.as_ref().unwrap().1.requests.is_empty());
        assert!(!app.join_requests_open);

        // Another room: the old watcher and its requests go.
        let _ = app.update(Message::SelectRoom("!b:x".into()));
        assert!(app.join_requests_watch.is_none() && app.join_requests.is_none());
        // A late update from the old room is ignored.
        let _ = app.update(Message::JoinRequestsUpdated("!a:x".into(), requests(&["@w:x"])));
        assert!(app.join_requests.is_none());
    }

    #[tokio::test]
    async fn decline_all_needs_confirmation() {
        let mut app = app();
        app.invited_rooms = vec![room("!a:x"), room("!b:x")];
        let _ = app.update(Message::DeclineAllInvites);
        assert_eq!(app.invited_rooms.len(), 2, "not armed: nothing declined");
        let _ = app.update(Message::ArmDeclineAllInvites(true));
        let _ = app.update(Message::DeclineAllInvites);
        assert!(app.invited_rooms.is_empty());
        assert!(!app.decline_all_armed);
    }

    #[tokio::test]
    async fn accept_all_clears_the_list() {
        let mut app = app();
        app.invited_rooms = vec![room("!a:x"), room("!b:x")];
        let _ = app.update(Message::AcceptAllInvites);
        assert!(app.invited_rooms.is_empty());
        assert!(app.selected_room_id.is_none(), "accepting in bulk doesn't switch rooms");
    }

    #[tokio::test]
    async fn new_room_stays_in_its_space_until_sync_catches_up() {
        use crate::message::SyncEvent;
        let list = |rooms: Vec<fluxchat_core::RoomInfo>| Message::SyncEvent(SyncEvent::RoomListUpdate(
            rooms, vec![room("!space:x")], vec![], vec![], vec![], Default::default(),
        ));
        let mut app = app();
        // Sync lists the new room before the space's m.space.child is back.
        let _ = app.update(list(vec![room("!new:x")]));
        let _ = app.update(Message::RoomCreated("!new:x".into(), "new".into(), false, false, Some("!space:x".into())));
        assert_eq!(app.rooms[0].space_id.as_deref(), Some("!space:x"));
        // Rebuilds that still lack the link don't undo it...
        let _ = app.update(list(vec![room("!new:x")]));
        assert_eq!(app.rooms[0].space_id.as_deref(), Some("!space:x"));
        // ...and once the server's list has it, ours stops overriding.
        let mut synced = room("!new:x");
        synced.space_id = Some("!space:x".into());
        let _ = app.update(list(vec![synced]));
        assert!(app.pending_space_links.is_empty());
    }
}
