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
        let perms = Task::perform(
            async move {
                let r = match core.get_client().await {
                    Ok(client) => room_admin::room_permissions(&client, &room_id).await.map_err(|e| e.to_string()),
                    Err(e) => Err(e.to_string()),
                };
                (room_id, r)
            },
            |(rid, r)| Message::RoomPermsLoaded(rid, r),
        );
        Task::batch([security, perms])
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
                Task::none()
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

