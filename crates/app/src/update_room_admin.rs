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

