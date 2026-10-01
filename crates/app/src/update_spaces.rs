use iced::Task;

use crate::message::Message;
use crate::state::{FluxChat, Modal, SpaceTab};

/// Let the user pick an image file; returns (mime, bytes).
pub(crate) async fn pick_image() -> Option<(&'static str, Vec<u8>)> {
    let handle = rfd::AsyncFileDialog::new()
        .add_filter("Image", &["png", "jpg", "jpeg", "gif", "webp"])
        .pick_file()
        .await?;
    let data = handle.read().await;
    let name = handle.file_name().to_lowercase();
    let mime = if name.ends_with(".png") { "image/png" }
        else if name.ends_with(".gif") { "image/gif" }
        else if name.ends_with(".webp") { "image/webp" }
        else { "image/jpeg" };
    Some((mime, data))
}

impl FluxChat {
    /// Fetch a space's children, members and our permissions for the dialog.
    fn load_space_details(&self, space_id: String) -> Task<Message> {
        let core = self.core.clone();
        Task::perform(
            async move {
                let client = match core.get_client().await {
                    Ok(c) => c,
                    Err(e) => return (space_id, Err(e.to_string()), Vec::new(), Default::default()),
                };
                let (children, members, perms) = tokio::join!(
                    fluxchat_core::spaces::space_children(&client, &space_id),
                    fluxchat_core::messages::get_room_members(&client, &space_id),
                    fluxchat_core::spaces::space_permissions(&client, &space_id),
                );
                (
                    space_id,
                    children.map_err(|e| e.to_string()),
                    members.unwrap_or_default(),
                    perms.unwrap_or_default(),
                )
            },
            |(sid, children, members, perms)| Message::SpaceDetailsLoaded(sid, children, members, perms),
        )
    }

    /// The space whose dialog is open, if any.
    fn open_space_id(&self) -> Option<&str> {
        match &self.modal {
            Some(Modal::SpaceDetails { space_id }) => Some(space_id),
            _ => None,
        }
    }

    /// Handle space dialog messages. Returns Some(task) if handled.
    pub(crate) fn handle_space_message(&mut self, message: Message) -> Option<Task<Message>> {
        let task = match message {
            Message::OpenSpaceDetails(space_id) => {
                self.context_menu = None;
                self.space_detail_rooms = None;
                self.space_detail_members.clear();
                self.space_perms = Default::default();
                self.space_tab = SpaceTab::Rooms;
                self.space_filter.clear();
                self.space_adding = false;
                self.space_confirm_remove = None;
                self.space_busy.clear();
                self.space_status = None;
                let space = self.spaces.iter().find(|s| s.room_id == space_id);
                self.space_name_input = space.map(|s| s.name.clone()).unwrap_or_default();
                self.space_topic_input = space.and_then(|s| s.topic.clone()).unwrap_or_default();
                // Visibility and addresses use Room Settings' controls.
                self.room_security = None;
                self.room_perms = None;
                self.room_advanced = None;
                self.room_alias_input.clear();
                self.modal = Some(Modal::SpaceDetails { space_id: space_id.clone() });
                Task::batch([self.load_space_details(space_id.clone()), self.load_room_admin(space_id)])
            }
            Message::SpaceDetailsLoaded(space_id, children, members, perms) => {
                if self.open_space_id() != Some(space_id.as_str()) {
                    return Some(Task::none());
                }
                let mut avatars: Vec<String> = members.iter().filter_map(|m| m.avatar_url.clone()).collect();
                match children {
                    Ok(children) => {
                        avatars.extend(children.iter().filter_map(|c| c.avatar_url.clone()));
                        self.space_detail_rooms = Some(children);
                    }
                    Err(e) => {
                        self.space_detail_rooms = Some(Vec::new());
                        self.space_status = Some(format!("Couldn't load rooms: {e}"));
                    }
                }
                self.space_detail_members = members;
                self.space_perms = perms;
                self.resolve_avatars(avatars)
            }
            Message::SpaceTabChanged(tab) => {
                self.space_tab = tab;
                self.space_filter.clear();
                self.space_adding = false;
                self.space_confirm_remove = None;
                crate::view::scroll_dialog_to_top()
            }
            Message::SpaceFilterChanged(q) => {
                self.space_filter = q;
                Task::none()
            }
            Message::SpaceToggleAdding => {
                self.space_adding = !self.space_adding;
                self.space_filter.clear();
                self.space_confirm_remove = None;
                crate::view::scroll_dialog_to_top()
            }
            Message::SpaceAskRemove(room_id) => {
                self.space_confirm_remove = room_id;
                Task::none()
            }
            Message::SpaceAddChild(space_id, room_id) => {
                self.space_busy.insert(room_id.clone());
                let core = self.core.clone();
                Task::perform(
                    async move {
                        let err = match core.get_client().await {
                            Ok(client) => fluxchat_core::spaces::add_child(&client, &space_id, &room_id).await
                                .err().map(|e| e.to_string()),
                            Err(e) => Some(e.to_string()),
                        };
                        (space_id, room_id, err)
                    },
                    |(s, r, e)| Message::SpaceChildOpDone(s, r, e),
                )
            }
            Message::SpaceRemoveChild(space_id, room_id) => {
                self.space_confirm_remove = None;
                self.space_busy.insert(room_id.clone());
                let core = self.core.clone();
                Task::perform(
                    async move {
                        let err = match core.get_client().await {
                            Ok(client) => fluxchat_core::spaces::remove_child(&client, &space_id, &room_id).await
                                .err().map(|e| e.to_string()),
                            Err(e) => Some(e.to_string()),
                        };
                        (space_id, room_id, err)
                    },
                    |(s, r, e)| Message::SpaceChildOpDone(s, r, e),
                )
            }
            Message::SpaceJoinChild(space_id, room_id) => {
                self.space_busy.insert(room_id.clone());
                let via = self.space_detail_rooms.iter().flatten()
                    .find(|c| c.room_id == room_id)
                    .map(|c| c.via.clone())
                    .unwrap_or_default();
                let core = self.core.clone();
                Task::perform(
                    async move {
                        let err = match core.get_client().await {
                            Ok(client) => fluxchat_core::spaces::join_child(&client, &room_id, &via).await
                                .err().map(|e| e.to_string()),
                            Err(e) => Some(e.to_string()),
                        };
                        (space_id, room_id, err)
                    },
                    |(s, r, e)| Message::SpaceChildOpDone(s, r, e),
                )
            }
            Message::SpaceChildOpDone(space_id, room_id, err) => {
                self.space_busy.remove(&room_id);
                self.space_status = err;
                let reload = if self.open_space_id() == Some(space_id.as_str()) {
                    self.load_space_details(space_id)
                } else {
                    Task::none()
                };
                Task::batch([reload, Task::done(Message::RefreshRoomList)])
            }
            Message::SpaceNameChanged(s) => {
                self.space_name_input = s;
                Task::none()
            }
            Message::SpaceTopicChanged(s) => {
                self.space_topic_input = s;
                Task::none()
            }
            Message::SpaceSaveDetails(space_id) => {
                let current = self.spaces.iter().find(|s| s.room_id == space_id);
                let name = self.space_name_input.trim().to_string();
                let topic = self.space_topic_input.trim().to_string();
                let name_changed = !name.is_empty() && current.map(|s| s.name.as_str()) != Some(name.as_str());
                let topic_changed = current.and_then(|s| s.topic.as_deref()).unwrap_or("") != topic;
                if !name_changed && !topic_changed {
                    self.space_status = Some("Nothing to save".into());
                    return Some(Task::none());
                }
                self.space_status = None;
                let core = self.core.clone();
                Task::perform(
                    async move {
                        let client = match core.get_client().await {
                            Ok(c) => c,
                            Err(e) => return (space_id, Some(e.to_string())),
                        };
                        if name_changed {
                            if let Err(e) = fluxchat_core::rooms::set_room_name(&client, &space_id, &name).await {
                                return (space_id, Some(e.to_string()));
                            }
                        }
                        if topic_changed {
                            if let Err(e) = fluxchat_core::rooms::set_room_topic(&client, &space_id, &topic).await {
                                return (space_id, Some(e.to_string()));
                            }
                        }
                        (space_id, None)
                    },
                    |(sid, err)| Message::SpaceDetailsSaved(sid, err),
                )
            }
            Message::SpaceUploadAvatar(space_id) => {
                let core = self.core.clone();
                Task::perform(
                    async move {
                        let (mime, data) = pick_image().await?;
                        let client = match core.get_client().await {
                            Ok(c) => c,
                            Err(e) => return Some((space_id, Some(e.to_string()))),
                        };
                        let err = fluxchat_core::rooms::upload_room_avatar(&client, &space_id, mime, data).await
                            .err().map(|e| e.to_string());
                        Some((space_id, err))
                    },
                    |r| match r {
                        Some((sid, err)) => Message::SpaceDetailsSaved(sid, err),
                        None => Message::Noop, // picker cancelled
                    },
                )
            }
            Message::SpaceRemoveAvatar(space_id) => {
                let core = self.core.clone();
                Task::perform(
                    async move {
                        let err = match core.get_client().await {
                            Ok(client) => fluxchat_core::rooms::remove_room_avatar(&client, &space_id).await
                                .err().map(|e| e.to_string()),
                            Err(e) => Some(e.to_string()),
                        };
                        (space_id, err)
                    },
                    |(sid, err)| Message::SpaceDetailsSaved(sid, err),
                )
            }
            Message::SpaceDetailsSaved(_space_id, err) => {
                self.space_status = Some(match err {
                    Some(e) => format!("Save failed: {e}"),
                    None => "Saved".into(),
                });
                // The room list rebuild carries the new name/topic/avatar.
                Task::done(Message::RefreshRoomList)
            }
            Message::SpaceCreateRoom(space_id) => {
                let open = self.update(Message::OpenModal(Modal::CreateRoom));
                self.create_room_is_space = false;
                self.create_room_space = Some(space_id);
                open
            }
            Message::SpaceOpenChild(room_id) => {
                self.modal = None;
                self.update(Message::SelectRoom(room_id))
            }
            _ => return None,
        };
        Some(task)
    }
}
