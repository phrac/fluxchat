use iced::Task;

use crate::message::Message;
use crate::state::FluxChat;

impl FluxChat {
    /// Handle settings, security, profile, and power level messages. Returns Some(task) if handled.
    pub(crate) fn handle_settings_message(&mut self, message: Message) -> Option<Task<Message>> {
        match message {
            // --- Settings ---
            Message::SettingsNickChanged(s) => {
                self.settings_nick_input = s;
                Some(Task::none())
            }
            Message::SettingsSaveNick => {
                let nick = self.settings_nick_input.clone();
                if nick.trim().is_empty() {
                    return Some(Task::none());
                }
                self.display_name = Some(nick.clone());
                let core = self.core.clone();
                Some(Task::perform(
                    async move {
                        let client = core.get_client().await.ok()?;
                        fluxchat_core::profile::set_display_name(&client, &nick).await.ok();
                        Some(())
                    },
                    |_| Message::SettingsStatus("Display name updated".into()),
                ))
            }
            Message::SettingsLoadDevices => {
                let core = self.core.clone();
                Some(Task::perform(
                    async move {
                        let client = core.get_client().await.ok()?;
                        fluxchat_core::devices::list_devices(&client).await.ok()
                    },
                    |result| match result {
                        Some(devices) => Message::SettingsDevicesLoaded(devices),
                        None => Message::Noop,
                    },
                ))
            }
            Message::SettingsDevicesLoaded(devices) => {
                self.settings_devices = devices;
                Some(Task::none())
            }
            Message::SettingsSignOutDevice(device_id) => {
                self.settings_signout_device = Some(device_id);
                self.settings_signout_password.clear();
                Some(Task::none())
            }
            Message::SettingsSignOutPasswordChanged(s) => {
                self.settings_signout_password = s;
                Some(Task::none())
            }
            Message::SettingsSignOutConfirm => {
                if let Some(device_id) = self.settings_signout_device.take() {
                    let password = std::mem::take(&mut self.settings_signout_password);
                    let core = self.core.clone();
                    Some(Task::perform(
                        async move {
                            let client = core.get_client().await.ok()?;
                            fluxchat_core::devices::delete_devices(&client, &[device_id], &password).await.ok()
                        },
                        |_| Message::SettingsLoadDevices,
                    ))
                } else {
                    Some(Task::none())
                }
            }
            Message::SettingsStatus(s) => {
                self.settings_status = Some(s);
                Some(Task::none())
            }
            Message::SettingsLoadProfile => {
                let core = self.core.clone();
                Some(Task::perform(
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
                ))
            }
            Message::SettingsProfileLoaded { avatar_data, banner_data } => {
                self.settings_avatar_handle = avatar_data.as_ref()
                    .map(|d| iced::widget::image::Handle::from_bytes(d.clone()));
                self.settings_avatar_data = avatar_data;
                self.settings_banner_dimensions = banner_data.as_ref()
                    .and_then(|d| fluxchat_core::media::get_image_dimensions(d));
                self.settings_banner_handle = banner_data.as_ref()
                    .map(|d| iced::widget::image::Handle::from_bytes(d.clone()));
                self.settings_banner_data = banner_data;
                self.settings_banner_offset = 0.5;
                // Apply initial center crop
                let _ = self.update(Message::SettingsBannerOffsetChanged(0.5));
                Some(Task::none())
            }
            Message::SettingsUploadAvatar => {
                let core = self.core.clone();
                Some(Task::perform(
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
                        Some(()) => Message::SettingsLoadProfile,
                        None => Message::Noop,
                    },
                ))
            }
            Message::SettingsRemoveAvatar => {
                self.settings_avatar_data = None;
                self.settings_avatar_handle = None;
                let core = self.core.clone();
                Some(Task::perform(
                    async move {
                        let client = core.get_client().await.ok()?;
                        fluxchat_core::profile::remove_avatar(&client).await.ok()
                    },
                    |result| match result {
                        Some(()) => Message::SettingsLoadProfile,
                        None => Message::Noop,
                    },
                ))
            }
            Message::SettingsPickBanner => {
                Some(Task::perform(
                    async move {
                        let handle = rfd::AsyncFileDialog::new()
                            .add_filter("Image", &["png", "jpg", "jpeg", "gif", "webp"])
                            .pick_file()
                            .await?;
                        Some(handle.read().await.to_vec())
                    },
                    |result| match result {
                        Some(data) => Message::SettingsBannerPicked(data),
                        None => Message::Noop,
                    },
                ))
            }
            Message::SettingsBannerPicked(data) => {
                self.settings_banner_dimensions = fluxchat_core::media::get_image_dimensions(&data);
                self.settings_banner_handle = Some(
                    iced::widget::image::Handle::from_bytes(data.clone())
                );
                self.settings_banner_data = Some(data);
                self.settings_banner_offset = 0.5;
                let _ = self.update(Message::SettingsBannerOffsetChanged(0.5));
                Some(Task::none())
            }
            Message::SettingsSaveBanner => {
                if let Some(ref data) = self.settings_banner_data {
                    let core = self.core.clone();
                    let data = data.clone();
                    let offset = self.settings_banner_offset;
                    let dims = self.settings_banner_dimensions;
                    Some(Task::perform(
                        async move {
                            // Crop to 4:1 aspect at the selected offset
                            let upload_data = if let Some((w, h)) = dims {
                                let target_h = (w as f32 / 4.0) as u32;
                                if h > target_h {
                                    if let Ok(img) = image::load_from_memory(&data) {
                                        let max_y = h.saturating_sub(target_h);
                                        let y = (max_y as f32 * offset) as u32;
                                        let cropped = image::DynamicImage::from(
                                            image::imageops::crop_imm(&img, 0, y, w, target_h.min(h - y)).to_image()
                                        );
                                        let mut buf = std::io::Cursor::new(Vec::new());
                                        if cropped.write_to(&mut buf, image::ImageFormat::Png).is_ok() {
                                            buf.into_inner()
                                        } else {
                                            data
                                        }
                                    } else {
                                        data
                                    }
                                } else {
                                    data
                                }
                            } else {
                                data
                            };
                            let client = core.get_client().await.ok()?;
                            fluxchat_core::profile::upload_banner(&client, "image/png", upload_data).await.ok()?;
                            Some(())
                        },
                        |result| match result {
                            Some(()) => Message::SettingsLoadProfile,
                            None => Message::SettingsStatus("Failed to upload banner".into()),
                        },
                    ))
                } else {
                    Some(Task::none())
                }
            }
            Message::SettingsRemoveBanner => {
                self.settings_banner_data = None;
                self.settings_banner_handle = None;
                let core = self.core.clone();
                Some(Task::perform(
                    async move {
                        let client = core.get_client().await.ok()?;
                        fluxchat_core::profile::remove_banner(&client).await.ok()
                    },
                    |result| match result {
                        Some(()) => Message::SettingsLoadProfile,
                        None => Message::Noop,
                    },
                ))
            }

            Message::SettingsBannerOffsetChanged(val) => {
                self.settings_banner_offset = val;
                // Re-crop banner preview from original data
                if let Some(ref data) = self.settings_banner_data {
                    if let Some((w, h)) = self.settings_banner_dimensions {
                        // Target aspect ratio ~4:1 for banner display
                        let target_h = (w as f32 / 4.0) as u32;
                        if h > target_h {
                            // Crop vertically based on offset
                            if let Ok(img) = image::load_from_memory(data) {
                                let max_y = h.saturating_sub(target_h);
                                let y = (max_y as f32 * val) as u32;
                                let cropped = image::DynamicImage::from(
                                    image::imageops::crop_imm(&img, 0, y, w, target_h.min(h - y)).to_image()
                                );
                                let mut buf = std::io::Cursor::new(Vec::new());
                                if cropped.write_to(&mut buf, image::ImageFormat::Png).is_ok() {
                                    self.settings_banner_handle = Some(
                                        iced::widget::image::Handle::from_bytes(buf.into_inner())
                                    );
                                }
                            }
                        }
                    }
                }
                Some(Task::none())
            }
            // --- Password change ---
            Message::SettingsOldPasswordChanged(val) => { self.settings_old_password = val; Some(Task::none()) }
            Message::SettingsNewPasswordChanged(val) => { self.settings_new_password = val; Some(Task::none()) }
            Message::SettingsConfirmPasswordChanged(val) => { self.settings_confirm_password = val; Some(Task::none()) }
            Message::SettingsChangePassword => {
                if self.settings_new_password != self.settings_confirm_password {
                    self.settings_status = Some("Passwords don't match".into());
                    return Some(Task::none());
                }
                if self.settings_new_password.is_empty() || self.settings_old_password.is_empty() {
                    self.settings_status = Some("Fill in all password fields".into());
                    return Some(Task::none());
                }
                let core = self.core.clone();
                let old_pw = self.settings_old_password.clone();
                let new_pw = self.settings_new_password.clone();
                self.settings_old_password.clear();
                self.settings_new_password.clear();
                self.settings_confirm_password.clear();
                Some(Task::perform(
                    async move {
                        let client = core.get_client().await.ok()?;
                        fluxchat_core::profile::change_password(&client, &old_pw, &new_pw).await.ok()
                    },
                    |result| match result {
                        Some(()) => Message::SettingsStatus("Password changed".into()),
                        None => Message::SettingsStatus("Failed to change password".into()),
                    },
                ))
            }

            // --- Encryption key rotation ---
            Message::RotateRoomKeys(room_id) => {
                let core = self.core.clone();
                let rid = room_id.clone();
                Some(Task::perform(
                    async move {
                        let client = core.get_client().await
                            .map_err(|e| format!("{e}"))?;
                        let room_id_parsed = matrix_sdk::ruma::RoomId::parse(&rid)
                            .map_err(|e| format!("Invalid room ID: {e}"))?;
                        let room = client.get_room(&room_id_parsed)
                            .ok_or_else(|| format!("Room not found: {rid}"))?;
                        room.discard_room_key().await
                            .map_err(|e| format!("Failed to rotate keys: {e}"))?;
                        Ok(rid)
                    },
                    Message::RotateRoomKeysResult,
                ))
            }
            Message::RotateRoomKeysResult(result) => {
                match result {
                    Ok(room_id) => {
                        tracing::info!("Rotated encryption keys for room {room_id}");
                        self.help_text = Some("Encryption keys rotated. Next message will use a new session.".into());
                    }
                    Err(e) => {
                        tracing::warn!("Failed to rotate keys: {e}");
                        self.help_text = Some(format!("Failed to rotate keys: {e}"));
                    }
                }
                None
            }

            // --- Security ---
            Message::SecurityLoadState => {
                self.security_state = None;
                let core = self.core.clone();
                Some(Task::perform(
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
                ))
            }
            Message::SecurityStateLoaded(state) => {
                self.security_state = Some(state);
                Some(Task::none())
            }
            Message::SecurityEnableBackup => {
                self.settings_status = Some("enabling backup...".into());
                let core = self.core.clone();
                Some(Task::perform(
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
                ))
            }
            Message::SecurityBackupEnabled(key) => {
                if key.is_some() {
                    self.settings_status = Some("backup enabled! save your recovery key".into());
                } else {
                    self.settings_status = Some("backup enabled".into());
                }
                self.security_shown_recovery_key = key;
                // Refresh security state
                Some(self.update(Message::SecurityLoadState))
            }
            Message::SecurityRecoveryKeyInputChanged(s) => {
                self.security_recovery_key_input = s;
                Some(Task::none())
            }
            Message::SecurityRecover => {
                let key = self.security_recovery_key_input.trim().to_string();
                if key.is_empty() {
                    return Some(Task::none());
                }
                self.settings_status = Some("recovering...".into());
                let core = self.core.clone();
                Some(Task::perform(
                    async move {
                        let client = core.get_client().await.map_err(|e| format!("{e}"))?;
                        client.encryption().recovery().recover(&key).await.map_err(|e| format!("{e}"))
                    },
                    Message::SecurityRecoverResult,
                ))
            }
            Message::SecurityRecoverResult(result) => {
                match result {
                    Ok(()) => {
                        self.settings_status = Some("recovery successful! keys restored.".into());
                        self.security_recovery_key_input.clear();
                        Some(self.update(Message::SecurityLoadState))
                    }
                    Err(e) => {
                        self.settings_status = Some(format!("recovery failed: {e}"));
                        Some(Task::none())
                    }
                }
            }

            // --- Profile actions ---
            Message::ToggleIgnoreUser(user_id) => {
                if self.ignored_users.contains(&user_id) {
                    self.ignored_users.remove(&user_id);
                } else {
                    self.ignored_users.insert(user_id);
                }
                Some(Task::none())
            }
            Message::ProfilePowerInputChanged(val) => {
                self.profile_power_input = val;
                Some(Task::none())
            }
            Message::ProfileSetPowerLevel => {
                if let Some(crate::state::Modal::Profile { ref user_id }) = self.modal {
                    if let Ok(level) = self.profile_power_input.trim().parse::<i64>() {
                        if let Some(ref room_id) = self.selected_room_id {
                            return Some(self.update(Message::SetPowerLevel(
                                room_id.clone(),
                                user_id.clone(),
                                level,
                            )));
                        }
                    }
                }
                Some(Task::none())
            }
            Message::SendDmToUser(user_id) => {
                self.modal = None;
                let core = self.core.clone();
                Some(Task::perform(
                    async move {
                        let client = core.get_client().await.ok()?;
                        let room_id = fluxchat_core::rooms::open_dm(&client, &user_id).await.ok()?;
                        let rid = fluxchat_core::ruma::RoomId::parse(&room_id).ok()?;
                        let room = client.get_room(&rid)?;
                        Some(fluxchat_core::rooms::room_to_info(&room, None).await)
                    },
                    |result| match result {
                        Some(info) => Message::DmCreated(Box::new(info)),
                        None => Message::Noop,
                    },
                ))
            }
            Message::DmCreated(info) => {
                let room_id = info.room_id.clone();
                if !self.rooms.iter().any(|r| r.room_id == room_id) {
                    self.rooms.push(*info);
                }
                Some(Task::done(Message::SelectRoom(room_id)))
            }

            // --- Manual verification ---
            Message::ManualVerifyUser(user_id) => {
                let core = self.core.clone();
                let uid = user_id.clone();
                Some(Task::perform(
                    async move {
                        let client = core.get_client().await.map_err(|e| e.to_string())?;
                        fluxchat_core::verification::manual_verify_user(&client, &uid).await
                            .map_err(|e| e.to_string())?;
                        Ok(uid)
                    },
                    move |result: Result<String, String>| match result {
                        Ok(uid) => Message::ManualVerifyResult(uid, Ok(())),
                        Err(e) => Message::ManualVerifyResult(user_id, Err(e)),
                    },
                ))
            }
            Message::ManualVerifyResult(user_id, result) => {
                match &result {
                    Ok(()) => {
                        self.profile_user_verified = Some(true);
                        self.help_text = Some(format!("Verified {user_id}"));
                    }
                    Err(e) => {
                        self.help_text = Some(format!("Verify failed: {e}"));
                    }
                }
                Some(Task::none())
            }
            Message::UserVerifiedStatus(_user_id, verified) => {
                self.profile_user_verified = Some(verified);
                Some(Task::none())
            }

            // --- Power levels ---
            Message::SetPowerLevel(room_id, user_id, level) => {
                let core = self.core.clone();
                Some(Task::perform(
                    async move {
                        let client = core.get_client().await.ok()?;
                        fluxchat_core::rooms::set_user_power_level(&client, &room_id, &user_id, level).await.ok()
                    },
                    |_| Message::Noop,
                ))
            }

            _ => None,
        }
    }
}
