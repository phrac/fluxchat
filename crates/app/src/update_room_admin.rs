use iced::Task;

use fluxchat_core::room_admin;

use crate::message::Message;
use crate::state::{FluxChat, Modal};

/// Servers to join a directory entry through: the ones its address and its
/// ID name. Our server can only join a room it isn't in yet by asking one
/// that is; without these, joining anything listed by another server's
/// directory failed.
pub(crate) fn directory_via(room: &fluxchat_core::rooms::PublicRoomResult) -> Vec<String> {
    let server_of = |id: &str| id.split_once(':').map(|(_, server)| server.to_string());
    let mut via: Vec<String> = Vec::new();
    for server in [room.alias.as_deref().and_then(server_of), server_of(&room.room_id)].into_iter().flatten() {
        if !server.is_empty() && !via.contains(&server) {
            via.push(server);
        }
    }
    via
}

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
        // Its own task: this fetches the room's full state from the server.
        let core = self.core.clone();
        let rid = room_id.clone();
        let bridges = Task::perform(
            async move {
                let bridges = match core.get_client().await {
                    Ok(client) => room_admin::room_bridges(&client, &rid).await
                        .map_err(|e| tracing::warn!("Couldn't load bridges of {rid}: {e}"))
                        .unwrap_or_default(),
                    Err(_) => Vec::new(),
                };
                (rid, bridges)
            },
            |(rid, bridges)| Message::RoomBridgesLoaded(rid, bridges),
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
        Task::batch([security, perms, advanced, bridges])
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

    /// The room whose settings are open: Room Settings' room, or the space
    /// in the space dialog (a space is a room; visibility and addresses
    /// use the same controls).
    fn room_settings_room(&self) -> Option<String> {
        match &self.modal {
            Some(Modal::RoomSettings) => self.active_real_room().map(str::to_string),
            Some(Modal::SpaceDetails { space_id }) => Some(space_id.clone()),
            _ => None,
        }
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
            Message::RoomBridgesLoaded(room_id, bridges) => {
                if self.room_settings_room().as_deref() != Some(room_id.as_str()) {
                    return Some(Task::none());
                }
                let avatars = bridges.iter().filter_map(|b| b.protocol_avatar.clone()).collect();
                self.room_bridges = Some(bridges);
                self.resolve_avatars(avatars)
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
                        // An upgrade takes a while: only follow it from the
                        // room it was started in, not from wherever we are now.
                        let here = self.active_real_room() == Some(old_room.as_str());
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
                        if here {
                            self.update(Message::SelectRoom(new_room))
                        } else {
                            Task::none()
                        }
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
            space_id: None, is_favourite: false, successor: None, is_public: false,
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
    async fn unfavouriting_sticks_until_sync_catches_up() {
        use crate::message::SyncEvent;
        let list = |fav: bool| {
            let mut r = room("!a:x");
            r.is_favourite = fav;
            Message::SyncEvent(SyncEvent::RoomListUpdate(vec![r], vec![], vec![], vec![], vec![], Default::default()))
        };
        let mut app = app();
        let _ = app.update(list(true));
        let _ = app.update(Message::SelectRoom("!a:x".into()));
        let _ = app.update(Message::ToggleFavourite);
        assert!(!app.rooms[0].is_favourite);
        // A rebuild from before the server saw it doesn't star it again...
        let _ = app.update(list(true));
        assert!(!app.rooms[0].is_favourite);
        // ...and once sync shows it, ours stops overriding.
        let _ = app.update(list(false));
        assert!(app.pending_favourites.is_empty());
        // A failed request puts the star back.
        let _ = app.update(Message::ToggleFavourite);
        assert!(app.rooms[0].is_favourite);
        let _ = app.update(Message::FavouriteSet("!a:x".into(), true, Err("offline".into())));
        assert!(!app.rooms[0].is_favourite && app.pending_favourites.is_empty());
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

    #[tokio::test]
    async fn media_previews_hide_and_reveal() {
        use fluxchat_core::profile::{MediaPreviewConfig, MediaPreviews};
        let mut app = app();
        app.user_id = "@me:x".into();
        let mut public = room("!pub:x");
        public.is_public = true;
        app.rooms = vec![public, room("!priv:x")];
        let image = |sender: &str| fluxchat_core::MessageInfo {
            event_id: format!("$img-{sender}"), sender: sender.into(), image_mxc: Some("mxc://x/1".into()),
            ..Default::default()
        };
        let set = |media| Message::MediaPreviewsLoaded(MediaPreviewConfig { media, invite_avatars: true });

        assert!(!app.media_hidden("!pub:x", &image("@a:x")), "on by default");
        let _ = app.update(set(MediaPreviews::Private));
        assert!(app.media_hidden("!pub:x", &image("@a:x")));
        assert!(!app.media_hidden("!priv:x", &image("@a:x")));
        let _ = app.update(set(MediaPreviews::Off));
        assert!(app.media_hidden("!priv:x", &image("@a:x")));
        assert!(!app.media_hidden("!priv:x", &image("@me:x")), "our own media always shows");
        // A thread view follows its room.
        assert!(app.media_hidden("thread::!priv:x::$root", &image("@a:x")));

        let _ = app.update(Message::RevealMedia("!priv:x".into(), "$img-@a:x".into()));
        assert!(!app.media_hidden("!priv:x", &image("@a:x")));
    }

    #[tokio::test]
    async fn room_toggle_hides_and_shows_its_images() {
        let mut app = app();
        app.user_id = "@me:x".into();
        app.rooms = vec![room("!a:x"), room("!b:x")];
        let image = |sender: &str| fluxchat_core::MessageInfo {
            event_id: format!("$img-{sender}"), sender: sender.into(), image_mxc: Some("mxc://x/1".into()),
            ..Default::default()
        };
        app.messages_by_room.insert("!a:x".into(), vec![image("@a:x"), image("@me:x")]);
        assert!(!app.media_hidden("!a:x", &image("@a:x")), "shown by default");

        let _ = app.update(Message::ToggleRoomMedia(Some("!a:x".into())));
        assert!(app.media_hidden("!a:x", &image("@a:x")));
        assert!(app.media_hidden("!a:x", &image("@me:x")), "the room toggle hides ours too");
        assert!(app.media_hidden("thread::!a:x::$root", &image("@a:x")), "threads follow their room");
        assert!(!app.media_hidden("!b:x", &image("@a:x")), "other rooms are untouched");
        assert_eq!(app.core.db.get_setting("hidden_media_rooms").as_deref(), Some(r#"["!a:x"]"#));

        // One image can still be clicked open…
        let _ = app.update(Message::RevealMedia("!a:x".into(), "$img-@a:x".into()));
        assert!(!app.media_hidden("!a:x", &image("@a:x")));
        // …and hiding the room again covers it.
        let _ = app.update(Message::ToggleRoomMedia(Some("!a:x".into())));
        assert!(!app.media_hidden("!a:x", &image("@me:x")), "showing the room shows everything");
        let _ = app.update(Message::ToggleRoomMedia(Some("thread::!a:x::$root".into())));
        assert!(app.media_hidden("!a:x", &image("@a:x")), "a thread's toggle is its room's; revealed images hide again");

        // No argument: the room on screen.
        let _ = app.update(Message::SelectRoom("!b:x".into()));
        let _ = app.update(Message::ToggleRoomMedia(None));
        assert!(app.room_media_hidden("!b:x"));
        let _ = app.update(Message::ToggleRoomMedia(None));
        assert!(!app.room_media_hidden("!b:x"));
        assert_eq!(app.core.db.get_setting("hidden_media_rooms").as_deref(), Some(r#"["!a:x"]"#));

        // `/images`: no argument toggles; on/off only act when they change something.
        use crate::commands::{dispatch, CommandResult};
        assert!(matches!(dispatch("/images", &app), CommandResult::Messages(_)));
        assert!(matches!(dispatch("/images off", &app), CommandResult::Messages(_)));
        assert!(matches!(dispatch("/images on", &app), CommandResult::Noop), "already showing");
        assert!(matches!(dispatch("/images sideways", &app), CommandResult::Help(_)));
    }

    #[tokio::test]
    async fn room_settings_close_when_the_room_changes_under_them() {
        use crate::state::Modal;
        let mut app = app();
        app.rooms = vec![room("!a:x"), room("!b:x")];
        let _ = app.update(Message::SelectRoom("!a:x".into()));
        let _ = app.update(Message::OpenModal(Modal::RoomSettings));
        assert!(matches!(app.modal, Some(Modal::RoomSettings)));
        // Its own thread is still the same room.
        let _ = app.update(Message::SelectRoom("thread::!a:x::$root".into()));
        assert!(matches!(app.modal, Some(Modal::RoomSettings)));
        // Another room (a notification click, Alt+arrows): A's fields must
        // not be saved onto B.
        let _ = app.update(Message::SelectRoom("!b:x".into()));
        assert!(app.modal.is_none());
        // Dialogs that carry their own target stay.
        let _ = app.update(Message::OpenModal(Modal::Gallery));
        let _ = app.update(Message::SelectRoom("!a:x".into()));
        assert!(matches!(app.modal, Some(Modal::Gallery)));
    }

    #[tokio::test]
    async fn logout_forgets_the_accounts_state() {
        let mut app = app();
        app.rooms = vec![room("!a:x")];
        let _ = app.update(Message::SelectRoom("!a:x".into()));
        app.prompt_history = vec!["a secret prompt".into()];
        app.snippets = vec![("s".into(), "text".into())];
        app.drafts.insert("!a:x".into(), "half a message".into());
        app.input_text = "typing".into();
        app.call_room_id = Some("!a:x".into());
        let _ = app.update(Message::Logout);
        assert!(app.prompt_history.is_empty() && app.snippets.is_empty() && app.drafts.is_empty());
        assert!(app.input_text.is_empty() && app.run_history.is_empty() && app.mentions.is_empty());
        assert!(app.call_room_id.is_none(), "a call ends with the session");
        assert_eq!(app.windows.len(), 1);
    }

    #[tokio::test]
    async fn commands_parse_polls_and_refuse_to_toggle_ignores() {
        use crate::commands::{dispatch, CommandResult};
        let mut app = app();
        app.rooms = vec![room("!a:x")];
        let _ = app.update(Message::SelectRoom("!a:x".into()));
        match dispatch("/poll Lunch? | Pizza | Sushi |  ", &app) {
            CommandResult::Messages(m) => assert!(matches!(&m[..],
                [Message::PollCreate(q, a)] if q == "Lunch?" && a == &["Pizza".to_string(), "Sushi".to_string()])),
            _ => panic!("a poll"),
        }
        assert!(matches!(dispatch("/poll Lunch? | Pizza", &app), CommandResult::Help(_)), "one option isn't a poll");
        assert!(matches!(dispatch("/status in a meeting", &app), CommandResult::Messages(_)));
        // The message toggles, so the commands check first.
        assert!(matches!(dispatch("/unignore @a:x", &app), CommandResult::Help(_)));
        assert!(matches!(dispatch("/ignore @a:x", &app), CommandResult::Messages(_)));
        app.ignored_users.insert("@a:x".into());
        assert!(matches!(dispatch("/ignore @a:x", &app), CommandResult::Help(_)));
        assert!(matches!(dispatch("/unignore @a:x", &app), CommandResult::Messages(_)));
    }

    #[test]
    fn editing_an_attachment_edits_its_caption() {
        let image = fluxchat_core::MessageInfo {
            body: "cat.png".into(), image_mxc: Some("mxc://x/1".into()), caption: Some("look at her".into()),
            ..Default::default()
        };
        assert_eq!(crate::update::editable_text(&image), "look at her");
        let bare = fluxchat_core::MessageInfo { body: "report.pdf".into(), file_mxc: Some("mxc://x/2".into()), ..Default::default() };
        assert_eq!(crate::update::editable_text(&bare), "", "never the file name");
        let text = fluxchat_core::MessageInfo { body: "hello".into(), ..Default::default() };
        assert_eq!(crate::update::editable_text(&text), "hello");
    }

    #[tokio::test]
    async fn thread_view_takes_the_rooms_late_decrypted_reply() {
        use crate::message::SyncEvent;
        let mut app = app();
        app.user_id = "@me:x".into();
        app.rooms = vec![room("!a:x")];
        let thread = "thread::!a:x::$root";
        let reply = |body: &str, undecryptable: bool| fluxchat_core::MessageInfo {
            event_id: "$reply".into(), sender: "@agent:x".into(), body: body.into(),
            thread_root: Some("$root".into()), timestamp: 10,
            utd: undecryptable.then(|| fluxchat_core::UtdInfo {
                session_id: "s".into(), sender_key: None, device_id: None, cause: "Unknown".into(),
            }),
            ..Default::default()
        };
        let body_in = |app: &crate::state::FluxChat, key: &str| app.messages_by_room[key][0].body.clone();

        // The reply arrives before its key: undecryptable in both views.
        let _ = app.update(Message::SyncEvent(SyncEvent::MessagesUpdated("!a:x".into(), vec![reply("locked", true)])));
        let _ = app.update(Message::SyncEvent(SyncEvent::MessagesUpdated(thread.into(), vec![reply("locked", true)])));
        assert!(app.messages_by_room[thread][0].utd.is_some());

        // The key arrives: the SDK re-decrypts the room's copy only. The
        // thread's watcher re-sends its (still locked) list on request.
        let _ = app.update(Message::SyncEvent(SyncEvent::MessagesUpdated("!a:x".into(), vec![reply("the answer", false)])));
        let _ = app.update(Message::SyncEvent(SyncEvent::MessagesUpdated(thread.into(), vec![reply("locked", true)])));
        assert_eq!(body_in(&app, thread), "the answer");
        assert!(app.messages_by_room[thread][0].utd.is_none());
    }

    fn text(id: &str, body: &str) -> fluxchat_core::MessageInfo {
        fluxchat_core::MessageInfo {
            event_id: id.into(), sender: "@a:x".into(), body: body.into(), timestamp: 10, ..Default::default()
        }
    }

    #[tokio::test]
    async fn an_invite_for_this_room_is_not_sent_to_the_last_dialogs_target() {
        use crate::state::Modal;
        let mut app = app();
        app.rooms = vec![room("!a:x"), room("!b:x")];
        app.spaces = vec![room("!space:x")];
        let _ = app.update(Message::SelectRoom("!a:x".into()));
        // An invite to the space, closed without inviting anyone.
        let _ = app.update(Message::SpaceInvite("!space:x".into()));
        assert_eq!(app.invite_target_id.as_deref(), Some("!space:x"));
        let _ = app.update(Message::Escape);
        assert!(app.modal.is_none());
        // "Invite to this room" is for this room, not that space.
        let _ = app.update(Message::OpenModal(Modal::Invite));
        assert!(app.invite_target_id.is_none());
        // And, bound to the room on screen, it closes when that changes.
        let _ = app.update(Message::SelectRoom("!b:x".into()));
        assert!(app.modal.is_none());
    }

    #[tokio::test]
    async fn page_down_at_the_end_leaves_the_view_at_the_bottom() {
        let mut app = app();
        app.rooms = vec![room("!a:x")];
        let _ = app.update(Message::SelectRoom("!a:x".into()));
        app.messages_viewport_h = 800.0;
        app.messages_at_bottom = true;
        app.last_scroll_offset = 4200.0;
        // The scrollable can't move, so nothing would report back: the
        // offset must not run past the end (rows are built around it).
        let _ = app.update(Message::ScrollMessagesBy(0.9));
        let _ = app.update(Message::ScrollMessagesBy(0.9));
        assert!(app.messages_at_bottom);
        assert_eq!(app.last_scroll_offset, 4200.0);
        // Page Up still goes up.
        let _ = app.update(Message::ScrollMessagesBy(-0.5));
        assert!(!app.messages_at_bottom);
        assert_eq!(app.last_scroll_offset, 3800.0);
        // Nothing to scroll in a list that fits.
        app.messages_at_bottom = true;
        app.messages_underfilled = true;
        app.last_scroll_offset = 0.0;
        let _ = app.update(Message::ScrollMessagesBy(-0.9));
        assert!(app.messages_at_bottom);
    }

    #[tokio::test]
    async fn jumping_to_a_message_in_the_last_screenful_stays_at_the_bottom() {
        let mut app = app();
        app.rooms = vec![room("!a:x")];
        let _ = app.update(Message::SelectRoom("!a:x".into()));
        // 30 unmeasured rows of 40 px in an 800 px list: 400 px to scroll.
        let msgs: Vec<_> = (0..30).map(|i| text(&format!("$m{i}"), "hello")).collect();
        app.messages_by_room.insert("!a:x".into(), msgs);
        app.messages_viewport_h = 800.0;
        app.messages_at_bottom = true;
        app.last_scroll_offset = 400.0;

        // A reply's quote pointing at a message already on screen.
        let _ = app.update(Message::JumpToEvent("$m28".into()));
        assert_eq!(app.highlight_event_id.as_deref(), Some("$m28"));
        assert!(app.messages_at_bottom, "still following new messages");
        assert_eq!(app.last_scroll_offset, 400.0);

        // One further up is scrolled to, as before.
        let _ = app.update(Message::JumpToEvent("$m4".into()));
        assert!(!app.messages_at_bottom);
        assert_eq!(app.last_scroll_offset, 80.0);
    }

    #[tokio::test]
    async fn the_end_of_a_stopped_clip_does_not_end_the_next_one() {
        use std::sync::atomic::Ordering;
        let mut app = app();
        let _ = app.update(Message::PlayAudio("$a".into(), "!a:x".into()));
        let first = app.audio_token.load(Ordering::SeqCst);
        // Another clip: the first is stopped, and its player says so a moment later.
        let _ = app.update(Message::PlayAudio("$b".into(), "!a:x".into()));
        let _ = app.update(Message::AudioFinished(first));
        assert_eq!(app.audio_playing.as_deref(), Some("$b"));
        // Its own end (or a failed download) does end it.
        let second = app.audio_token.load(Ordering::SeqCst);
        let _ = app.update(Message::AudioFinished(second));
        assert!(app.audio_playing.is_none() && app.audio_start_time.is_none());
        // Stopping also disowns a download still on its way.
        let _ = app.update(Message::PlayAudio("$c".into(), "!a:x".into()));
        let loading = app.audio_token.load(Ordering::SeqCst);
        let _ = app.update(Message::StopAudio);
        assert_ne!(app.audio_token.load(Ordering::SeqCst), loading);
        assert!(app.audio_playing.is_none());
    }

    #[tokio::test]
    async fn a_link_without_a_preview_is_not_asked_for_on_every_update() {
        use crate::message::SyncEvent;
        let url = "https://example.org/page";
        let mut app = app();
        app.rooms = vec![room("!a:x")];
        let _ = app.update(Message::SelectRoom("!a:x".into()));
        let update = || Message::SyncEvent(SyncEvent::MessagesUpdated(
            "!a:x".into(), vec![text("$m", &format!("see {url}"))],
        ));
        let _ = app.update(update());
        assert!(app.url_previews_pending.contains(url), "asked for");
        // The server has no preview for it.
        let _ = app.update(Message::UrlPreviewFetched(url.into(), None));
        assert!(app.url_previews_pending.contains(url), "the next update won't ask again");
        let _ = app.update(Message::Tick);
        assert!(app.url_previews_pending.contains(url));
        // Much later it gets another try.
        if let Some(long_ago) = std::time::Instant::now().checked_sub(std::time::Duration::from_secs(3600)) {
            app.url_preview_misses.insert(url.into(), long_ago);
            let _ = app.update(Message::Tick);
            assert!(!app.url_previews_pending.contains(url) && app.url_preview_misses.is_empty());
        }
        // A preview that did arrive is kept, and no longer pending.
        let _ = app.update(update());
        let preview = fluxchat_core::UrlPreview {
            url: url.into(), title: Some("Page".into()), description: None, image_url: None, site_name: None,
        };
        let _ = app.update(Message::UrlPreviewFetched(url.into(), Some(preview)));
        assert!(app.url_previews.contains_key(url) && !app.url_previews_pending.contains(url));
    }

    #[tokio::test]
    async fn format_buttons_put_the_caret_between_their_markers() {
        use iced::widget::text_editor::{Action, Edit};
        let mut app = app();
        app.rooms = vec![room("!a:x")];
        let _ = app.update(Message::SelectRoom("!a:x".into()));
        let type_in = |app: &mut crate::state::FluxChat, s: &str| {
            for c in s.chars() {
                let _ = app.update(Message::InputAction(Action::Edit(Edit::Insert(c))));
            }
        };
        type_in(&mut app, "so ");
        let _ = app.update(Message::InsertFormat("**", "**"));
        type_in(&mut app, "bold");
        assert_eq!(app.input_text, "so **bold**");
        // Across the line break of a code block's closing fence too.
        let mut app = crate::panes::flow_tests::app();
        let _ = app.update(Message::SelectRoom("!a:x".into()));
        let _ = app.update(Message::InsertFormat("```\n", "\n```"));
        type_in(&mut app, "x");
        assert_eq!(app.input_text, "```\nx\n```");
    }

    #[tokio::test]
    async fn an_earlier_directory_searchs_results_are_dropped() {
        let listed = |id: &str| fluxchat_core::rooms::PublicRoomResult {
            room_id: id.into(), name: None, alias: None, topic: None, num_members: 1, avatar_url: None,
        };
        let mut app = app();
        let _ = app.update(Message::DirectorySearch);
        let slow = app.directory_request;
        // Back to the other server before the first one answered.
        let _ = app.update(Message::DirectoryServerChanged("matrixrooms.info".into()));
        let current = app.directory_request;
        let _ = app.update(Message::DirectoryResults(current, vec![listed("!new:x")], None));
        let _ = app.update(Message::DirectoryResults(slow, vec![listed("!old:x")], Some("token".into())));
        assert_eq!(app.directory_results.len(), 1);
        assert_eq!(app.directory_results[0].room_id, "!new:x");
        assert!(app.directory_next_batch.is_none(), "not the other server's page token");
        // A further page of the search on screen is added to it.
        let _ = app.update(Message::DirectoryResults(current, vec![listed("!more:x")], None));
        assert_eq!(app.directory_results.len(), 2);
    }

    #[tokio::test]
    async fn joining_from_the_directory_goes_through_the_rooms_servers() {
        use crate::state::Modal;
        let listed = fluxchat_core::rooms::PublicRoomResult {
            room_id: "!abc:far.example".into(), name: None, alias: Some("#talk:hub.example:8448".into()),
            topic: None, num_members: 1, avatar_url: None,
        };
        assert_eq!(super::directory_via(&listed), ["hub.example:8448", "far.example"]);
        // The same server twice is one; a room ID without a server has none.
        let same = fluxchat_core::rooms::PublicRoomResult {
            room_id: "!abc:far.example".into(), alias: Some("#talk:far.example".into()), ..listed.clone()
        };
        assert_eq!(super::directory_via(&same), ["far.example"]);
        let bare = fluxchat_core::rooms::PublicRoomResult { room_id: "!abc".into(), alias: None, ..listed.clone() };
        assert!(super::directory_via(&bare).is_empty());

        // The join is the ordinary one (reported, lands in the room): the
        // dialog doesn't wait for it.
        let mut app = app();
        app.modal = Some(Modal::Directory);
        app.directory_results = vec![listed];
        let _ = app.update(Message::DirectoryJoinRoom("!abc:far.example".into()));
        assert!(app.modal.is_none());
    }

    #[tokio::test]
    async fn a_panels_divider_counts_what_is_right_of_it() {
        use crate::state::ResizeTarget;
        let mut app = app();
        app.rooms = vec![room("!a:x")];
        app.window_size = (1200.0, 800.0);
        app.mentions_open = true;
        app.member_list_width = 200.0;
        let drag = |app: &mut crate::state::FluxChat, x: f32| {
            let _ = app.update(Message::ResizeStart(ResizeTarget::Mentions));
            let _ = app.update(Message::CursorMoved(x, 10.0));
            let _ = app.update(Message::SidebarDragEnd);
            app.mentions_width
        };
        // No room open: no member list (and no rail) is drawn beside it.
        app.show_member_list = true;
        assert_eq!(drag(&mut app, 900.0), 300.0);
        // Member list hidden: its rail is still there.
        let _ = app.update(Message::SelectRoom("!a:x".into()));
        app.show_member_list = false;
        assert_eq!(drag(&mut app, 900.0), 300.0 - crate::view::MEMBER_RAIL_WIDTH);
        // Shown: the list and its own divider.
        app.show_member_list = true;
        assert_eq!(drag(&mut app, 700.0), 300.0 - crate::view::RESIZE_HANDLE_WIDTH);
    }

    #[tokio::test]
    async fn a_link_to_a_space_we_are_in_shows_the_space() {
        let mut app = app();
        app.rooms = vec![room("!a:x")];
        app.spaces = vec![room("!space:x")];
        let _ = app.update(Message::OpenMatrixLink("!space:x".into(), None, vec![]));
        assert!(app.modal.is_none(), "nothing to join");
        assert_eq!(app.selected_space_id.as_deref(), Some("!space:x"));
        assert!(app.selected_room_id.is_none(), "a space isn't opened as a chat");
        // One we're not in still asks.
        let _ = app.update(Message::OpenMatrixLink("!elsewhere:x".into(), None, vec![]));
        assert!(matches!(app.modal, Some(crate::state::Modal::ConfirmJoin { .. })));
    }

    #[tokio::test]
    async fn a_place_off_the_map_is_not_shared() {
        let mut app = app();
        app.rooms = vec![room("!a:x")];
        let _ = app.update(Message::SelectRoom("!a:x".into()));
        let mut submit = |lat: &str, lon: &str| {
            app.help_text = None;
            app.location_open = true;
            app.location_lat = lat.into();
            app.location_lon = lon.into();
            let _ = app.update(Message::LocationSubmit);
            (app.location_open, app.help_text.is_some())
        };
        // Longitude first (GeoJSON order), and things that only parse as numbers.
        assert_eq!(submit("151.2093", "-33.8688"), (true, true));
        assert_eq!(submit("nan", "10"), (true, true));
        assert_eq!(submit("10", "inf"), (true, true));
        assert_eq!(submit("-33.8688", "151.2093"), (false, false), "sent");
    }

    #[tokio::test]
    async fn withdrawing_a_join_request_removes_it_at_once() {
        let mut app = app();
        app.knocked_rooms = vec![room("!a:x"), room("!b:x")];
        let _ = app.update(Message::CancelKnock("!a:x".into()));
        assert_eq!(app.knocked_rooms.len(), 1);
        assert_eq!(app.knocked_rooms[0].room_id, "!b:x");
    }

    #[tokio::test]
    async fn a_refused_display_name_is_put_back() {
        let mut app = app();
        app.display_name = Some("old".into());
        app.settings_nick_input = "  new  ".into();
        let _ = app.update(Message::SettingsSaveNick);
        assert_eq!(app.display_name.as_deref(), Some("new"));
        assert!(app.settings_status.is_none(), "not \"updated\" before the server said so");
        let _ = app.update(Message::NickSet(Err((Some("old".into()), "offline".into()))));
        assert_eq!(app.display_name.as_deref(), Some("old"));
        assert!(app.settings_status.as_deref().is_some_and(|s| s.contains("offline")));
        let _ = app.update(Message::SetNick("new".into()));
        let _ = app.update(Message::NickSet(Ok(())));
        assert_eq!(app.display_name.as_deref(), Some("new"));
        assert_eq!(app.settings_status.as_deref(), Some("Display name updated"));
    }

    #[tokio::test]
    async fn an_upgrade_that_ends_after_you_left_the_room_does_not_pull_you_back() {
        let mut app = app();
        app.rooms = vec![room("!a:x"), room("!b:x")];
        let _ = app.update(Message::SelectRoom("!a:x".into()));
        let _ = app.update(Message::SelectRoom("!b:x".into()));
        let _ = app.update(Message::RoomUpgraded("!a:x".into(), Ok("!a2:x".into())));
        assert_eq!(app.selected_room_id.as_deref(), Some("!b:x"));
        assert!(app.rooms.iter().any(|r| r.room_id == "!a2:x"), "the new room is listed");
        assert_eq!(app.rooms[0].successor.as_deref(), Some("!a2:x"));
        // Still in the room: on to its replacement.
        let _ = app.update(Message::SelectRoom("!b:x".into()));
        let _ = app.update(Message::RoomUpgraded("!b:x".into(), Ok("!b2:x".into())));
        assert_eq!(app.selected_room_id.as_deref(), Some("!b2:x"));
    }
}
