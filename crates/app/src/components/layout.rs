use dioxus::prelude::*;
use std::sync::Arc;
use fluxchat_core::AppState;

use crate::state::{UiState, RightPanel};
use crate::components::sidebar::space_sidebar::SpaceSidebar;
use crate::components::sidebar::room_list::RoomList;
use crate::components::chat::message_area::{MessageArea, nick_color_class};
use crate::components::members::member_list::MemberList;

#[component]
pub fn MainLayout() -> Element {
    let mut ui = use_context::<UiState>();

    let _app_state = use_context::<Arc<AppState>>();
    let user_id = ui.user_id.read().clone();
    let selected_room = ui.selected_room_id.read().clone();
    let room_name = selected_room.as_ref()
        .and_then(|id| ui.rooms.read().iter().find(|r| r.room_id == *id).map(|r| r.name.clone()))
        .unwrap_or_default();
    let member_count = selected_room.as_ref()
        .and_then(|id| ui.members_by_room.read().get(id).map(|m| m.len()))
        .unwrap_or(0);
    let is_encrypted = selected_room.as_ref()
        .and_then(|id| ui.rooms.read().iter().find(|r| r.room_id == *id).map(|r| r.is_encrypted))
        .unwrap_or(false);

    rsx! {
        // Main content area
        div { style: "display: flex; flex: 1; min-height: 0;",
            // Space sidebar
            SpaceSidebar {}

            // Room list
            RoomList {}

            // Message area (with channel tabs)
            MessageArea {}

            // Right panel (pins/search/threads) or member list
            { let panel = ui.right_panel.read().clone();
              let show_members = *ui.show_member_list.read();
              if let Some(ref p) = panel {
                rsx! { RightPanelView { panel: p.clone() } }
              } else if selected_room.is_some() && show_members {
                rsx! { MemberList {} }
              } else { rsx! {} }
            }
        }

        // Profile modal
        { let profile_uid = ui.profile_user_id.read().clone();
          if let Some(uid) = profile_uid {
            rsx! { ProfileModal { user_id: uid } }
          } else { rsx! {} }
        }

        // Settings modal
        if *ui.settings_open.read() {
            SettingsModal {}
        }

        // Create room modal
        if *ui.create_room_open.read() {
            CreateRoomModal {}
        }

        // Room settings modal
        if *ui.room_settings_open.read() {
            RoomSettingsModal {}
        }

        // Image lightbox
        { let lb = ui.lightbox_url.read().clone();
          if let Some(ref url) = lb {
            let url_for_save = url.clone();
            rsx! {
                div {
                    style: "position: fixed; inset: 0; background: rgba(0,0,0,0.85); z-index: 2000; display: flex; align-items: center; justify-content: center; cursor: pointer;",
                    tabindex: "0",
                    autofocus: true,
                    onclick: move |_| { ui.lightbox_url.set(None); },
                    onkeydown: move |e: Event<KeyboardData>| {
                        if e.key() == Key::Escape { ui.lightbox_url.set(None); }
                    },
                    // Close button
                    div { style: "position: absolute; top: 16px; right: 16px; display: flex; gap: 8px; z-index: 2001;",
                        // Save button (disk icon)
                        span {
                            style: "color: var(--text-muted); cursor: pointer; background: rgba(0,0,0,0.6); padding: 6px 8px; border-radius: 4px; display: inline-flex; align-items: center;",
                            title: "Save image",
                            onclick: {
                                let url = url_for_save.clone();
                                move |e: Event<MouseData>| {
                                    e.stop_propagation();
                                    let url = url.clone();
                                    std::thread::spawn(move || {
                                        if let Some(comma) = url.find(',') {
                                            let header = &url[..comma];
                                            let b64 = &url[comma + 1..];
                                            use base64::Engine;
                                            if let Ok(bytes) = base64::engine::general_purpose::STANDARD.decode(b64) {
                                                let ext = if header.contains("image/png") { "png" }
                                                    else if header.contains("image/jpeg") { "jpg" }
                                                    else if header.contains("image/gif") { "gif" }
                                                    else if header.contains("image/webp") { "webp" }
                                                    else { "png" };

                                                let dialog = rfd::FileDialog::new()
                                                    .set_file_name(&format!("image.{ext}"))
                                                    .add_filter("Image", &[ext]);
                                                if let Some(path) = dialog.save_file() {
                                                    let _ = std::fs::write(&path, &bytes);
                                                }
                                            }
                                        }
                                    });
                                }
                            },
                            dangerous_inner_html: r#"<svg width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><path d="M19 21H5a2 2 0 0 1-2-2V5a2 2 0 0 1 2-2h11l5 5v11a2 2 0 0 1-2 2z"/><polyline points="17 21 17 13 7 13 7 21"/><polyline points="7 3 7 8 15 8"/></svg>"#,
                        }
                        // Close button
                        span {
                            style: "color: var(--text-muted); cursor: pointer; font-size: 20px; background: rgba(0,0,0,0.6); padding: 4px 10px; border-radius: 4px; line-height: 1;",
                            onclick: move |_| { ui.lightbox_url.set(None); },
                            "×"
                        }
                    }
                    img {
                        src: "{url}",
                        style: "max-width: 90vw; max-height: 90vh; object-fit: contain; border-radius: 4px;",
                        onclick: move |e| { e.stop_propagation(); },
                    }
                }
            }
          } else { rsx! {} }
        }

        // Verification modal
        { let vr = ui.verification_request.read().clone();
          if let Some(info) = vr {
            rsx! { VerificationModal { info: info } }
          } else { rsx! {} }
        }

        // Status bar — IRC style
        div { class: "status-bar",
            // Gear + avatar + name — all clickable to open settings
            div { style: "display: flex; align-items: center; gap: 6px; cursor: pointer; padding: 0 2px; border-radius: 3px;",
                onclick: move |_| { let v = *ui.settings_open.read(); ui.settings_open.set(!v); },
                span { style: "font-size: 15px; opacity: 0.6;", "⚙" }
                UserAvatar {}
                span { style: "font-family: var(--font-mono); color: var(--green);", "{user_id}" }
            }
            span { class: "sep", "│" }
            if let Some(_) = &selected_room {
                span { "#{room_name}" }
                span { class: "sep", "│" }
                span { "{member_count} nicks" }
                span { class: "sep", "│" }
                if is_encrypted {
                    span { style: "color: var(--green);", "🔒 encrypted" }
                } else {
                    span { "unencrypted" }
                }
            } else {
                span { "no channel selected" }
            }
            span { class: "sep", "│" }
            span { style: "color: var(--green);", "synced" }
            span { class: "sep", "│" }
            DeviceStatus {}
        }
    }
}

/// Right panel for pins, search, threads.
#[component]
fn RightPanelView(panel: RightPanel) -> Element {
    let mut ui = use_context::<UiState>();
    let _app_state = use_context::<Arc<AppState>>();
    let _selected = ui.selected_room_id.read().clone();

    let title = match panel {
        RightPanel::Pins => "Pinned Messages",
        RightPanel::Search => "Search",
        RightPanel::Threads => "Threads",
        RightPanel::Invites => "Invites",
    };

    rsx! {
        div { style: "width: 280px; flex-shrink: 0; background: var(--bg-dark); border-left: 1px solid var(--border); display: flex; flex-direction: column;",
            // Header
            div { style: "height: 40px; display: flex; align-items: center; justify-content: space-between; padding: 0 12px; border-bottom: 1px solid var(--border); flex-shrink: 0;",
                span { style: "font-size: 12px; font-weight: 700; text-transform: uppercase; color: var(--text-dim); letter-spacing: 0.5px; font-family: var(--font-mono);",
                    "{title}"
                }
                span { style: "cursor: pointer; color: var(--text-dim); font-size: 14px;",
                    onclick: move |_| { ui.right_panel.set(None); },
                    "×"
                }
            }
            // Content
            div { style: "flex: 1; overflow-y: auto; padding: 8px 12px;",
                match panel {
                    RightPanel::Pins => rsx! {},
                    RightPanel::Search => rsx! {},
                    RightPanel::Threads => rsx! {
                        div { style: "color: var(--text-dim); font-size: 13px; font-family: var(--font-mono); text-align: center; padding: 20px;",
                            "Threads coming soon"
                        }
                    },
                    RightPanel::Invites => rsx! {
                        div { style: "color: var(--text-dim); font-size: 13px; font-family: var(--font-mono); text-align: center; padding: 20px;",
                            "Invites coming soon"
                        }
                    },
                }
            }
        }
    }
}

/// Pinned messages panel.
#[component]
fn PinnedMessagesPanel() -> Element {
    let ui = use_context::<UiState>();
    let app_state = use_context::<Arc<AppState>>();
    let selected = ui.selected_room_id.read().clone();

    let pinned = use_resource({
        let app_state = app_state.clone();
        let room_id = selected.clone();
        move || {
            let app_state = app_state.clone();
            let room_id = room_id.clone();
            async move {
                let room_id = room_id?;
                let client = app_state.get_client().await.ok()?;
                fluxchat_core::messages::get_pinned_messages(&client, &room_id).await.ok()
            }
        }
    });

    let resolved = pinned.read().clone().flatten();

    match resolved {
        Some(msgs) if msgs.is_empty() => rsx! {
            div { style: "color: var(--text-dim); font-size: 13px; font-family: var(--font-mono); text-align: center; padding: 20px;",
                "No pinned messages"
            }
        },
        Some(msgs) => rsx! {
            for msg in msgs.iter() {
                div { style: "padding: 6px 0; border-bottom: 1px solid var(--border);",
                    div { style: "font-size: 11px; color: var(--text-dim); font-family: var(--font-mono); margin-bottom: 2px;",
                        "{msg.sender_name}"
                    }
                    div { style: "font-size: 13px; color: var(--text-normal); font-family: var(--font-mono);",
                        "{msg.body}"
                    }
                }
            }
        },
        None => rsx! {
            div { style: "color: var(--text-dim); font-size: 13px; font-family: var(--font-mono); text-align: center; padding: 20px;",
                "Loading..."
            }
        },
    }
}

/// Device/session verification status in the status bar.
#[component]
fn DeviceStatus() -> Element {
    let app_state = use_context::<Arc<AppState>>();
    let ui = use_context::<UiState>();

    let status = use_resource({
        let app_state = app_state.clone();
        move || {
            // Re-check when verification modal closes
            let _ = ui.verification_request.read();
            let app_state = app_state.clone();
            async move {
                // Small delay to let crypto state settle after verification
                tokio::time::sleep(std::time::Duration::from_millis(500)).await;
                let client = app_state.get_client().await.ok()?;
                let device_id = client.device_id()?.to_string();
                let user_id = client.user_id()?;

                // Check if our own device is cross-signing verified
                let own_device = client.encryption()
                    .get_device(user_id, client.device_id()?)
                    .await.ok()??;

                let is_verified = own_device.is_verified_with_cross_signing();

                // Count total own devices
                let devices = client.encryption()
                    .get_user_devices(user_id)
                    .await.ok()?;
                let total = devices.devices().count();
                let verified_count = devices.devices()
                    .filter(|d| d.is_verified_with_cross_signing())
                    .count();

                Some((device_id, is_verified, verified_count, total))
            }
        }
    });

    let resolved = status.read().clone().flatten();

    if let Some((device_id, is_verified, verified, total)) = resolved {
        let (icon, color) = if is_verified {
            ("✓", "var(--green)")
        } else {
            ("✗", "var(--yellow)")
        };
        let label = if is_verified { "verified" } else { "unverified" };
        let short_id = if device_id.len() > 10 { &device_id[..10] } else { &device_id };
        let tooltip = format!("{} ({}/{} devices verified)", device_id, verified, total);
        rsx! {
            span {
                style: "color: {color}; font-family: var(--font-mono);",
                title: "{tooltip}",
                "{icon} {label}"
            }
            span { style: "color: var(--text-dim); font-family: var(--font-mono); margin-left: 4px; font-size: 10px;",
                title: "{device_id}",
                "[{short_id}]"
            }
        }
    } else {
        rsx! {
            span { style: "color: var(--text-dim); font-family: var(--font-mono);", "..." }
        }
    }
}

/// Tiny avatar for the current user in the status bar.
#[component]
fn UserAvatar() -> Element {
    let _ui = use_context::<UiState>();
    let app_state = use_context::<Arc<AppState>>();

    // Resolve own avatar
    let avatar_data = use_resource({
        let app_state = app_state.clone();
        move || {
            let app_state = app_state.clone();
            async move {
                let client = app_state.get_client().await.ok()?;
                let avatar_mxc = client.account().get_avatar_url().await.ok()??.to_string();
                fluxchat_core::media::get_media_url(&client, &app_state.db, &app_state.media_cache, &avatar_mxc).await.ok()
            }
        }
    });
    let resolved = avatar_data.read().clone().flatten();

    if let Some(src) = resolved {
        rsx! {
            img {
                src: "{src}",
                style: "width: 16px; height: 16px; border-radius: 50%; object-fit: cover; margin-right: 4px;",
            }
        }
    } else {
        rsx! {}
    }
}

/// Settings modal.
#[component]
fn SettingsModal() -> Element {
    let mut ui = use_context::<UiState>();
    let app_state = use_context::<Arc<AppState>>();
    let user_id = ui.user_id.read().clone();
    let display_name = ui.display_name.read().clone().unwrap_or_else(|| user_id.clone());
    let mut edit_name = use_signal(|| display_name.clone());
    let mut status_msg = use_signal(|| Option::<String>::None);

    // Fetch current profile with avatar
    let profile = use_resource({
        let app_state = app_state.clone();
        move || {
            let app_state = app_state.clone();
            async move {
                let client = app_state.get_client().await.ok()?;
                let p = fluxchat_core::profile::get_profile(&client).await.ok()?;
                let avatar_data = if let Some(ref mxc) = p.avatar_url {
                    fluxchat_core::media::get_media_url(&client, &app_state.db, &app_state.media_cache, mxc).await.ok()
                } else { None };
                let banner_data = if let Some(ref mxc) = p.banner_url {
                    fluxchat_core::media::get_media_url(&client, &app_state.db, &app_state.media_cache, mxc).await.ok()
                } else { None };
                Some((p, avatar_data, banner_data))
            }
        }
    });
    let profile_data = profile.read().clone().flatten();
    let avatar_resolved = profile_data.as_ref().and_then(|(_, a, _)| a.clone());
    let banner_resolved = profile_data.as_ref().and_then(|(_, _, b)| b.clone());

    rsx! {
        div {
            style: "position: fixed; inset: 0; background: rgba(0,0,0,0.5); z-index: 1000; display: flex; align-items: center; justify-content: center;",
            onclick: move |_| { ui.settings_open.set(false); },

            div {
                style: "background: var(--bg-dark); border: 1px solid var(--border); border-radius: 8px; min-width: 400px; max-width: 500px; max-height: 80vh; overflow-y: auto;",
                onclick: move |e| { e.stop_propagation(); },

                // Header
                div { style: "display: flex; justify-content: space-between; align-items: center; padding: 16px 20px; border-bottom: 1px solid var(--border);",
                    span { style: "font-size: 16px; font-weight: 600; color: var(--text-bright); font-family: var(--font-mono);", "settings" }
                    span {
                        style: "cursor: pointer; color: var(--text-dim); font-size: 18px; padding: 4px;",
                        onclick: move |_| { ui.settings_open.set(false); },
                        "×"
                    }
                }

                // Content
                div { style: "padding: 20px; display: flex; flex-direction: column; gap: 16px;",

                    // Profile section
                    div {
                        div { style: "font-size: 11px; font-weight: 700; text-transform: uppercase; color: var(--text-dim); letter-spacing: 0.5px; margin-bottom: 8px; font-family: var(--font-mono);",
                            "profile"
                        }
                        // Avatar
                        div { style: "display: flex; align-items: center; gap: 12px; margin-bottom: 12px;",
                            if let Some(ref src) = avatar_resolved {
                                img { src: "{src}", style: "width: 64px; height: 64px; border-radius: 50%; object-fit: cover;" }
                            } else {
                                div { style: "width: 64px; height: 64px; border-radius: 50%; background: var(--bg-light); display: flex; align-items: center; justify-content: center; font-size: 24px; font-weight: 700; color: var(--text-muted);",
                                    { display_name.chars().next().unwrap_or('?').to_uppercase().to_string() }
                                }
                            }
                            div { style: "display: flex; flex-direction: column; gap: 4px;",
                                // Upload avatar
                                span { style: "font-family: var(--font-mono); font-size: 12px; color: var(--accent); cursor: pointer;",
                                    onclick: {
                                        let app_state = app_state.clone();
                                        move |_| {
                                            let app_state = app_state.clone();
                                            spawn(async move {
                                                let mut eval = document::eval(r#"
                                                    let input = document.createElement('input');
                                                    input.type = 'file';
                                                    input.accept = 'image/*';
                                                    input.onchange = async () => {
                                                        if (input.files.length > 0) {
                                                            let file = input.files[0];
                                                            let reader = new FileReader();
                                                            reader.onload = () => {
                                                                let base64 = reader.result.split(',')[1];
                                                                dioxus.send(JSON.stringify({ type: file.type, data: base64 }));
                                                            };
                                                            reader.readAsDataURL(file);
                                                        }
                                                    };
                                                    input.click();
                                                "#);
                                                if let Ok(json) = eval.recv::<String>().await {
                                                    if let Ok(parsed) = serde_json::from_str::<serde_json::Value>(&json) {
                                                        let mime = parsed["type"].as_str().unwrap_or("image/png").to_string();
                                                        if let Some(b64) = parsed["data"].as_str() {
                                                            use base64::Engine;
                                                            if let Ok(data) = base64::engine::general_purpose::STANDARD.decode(b64) {
                                                                let client = match app_state.get_client().await { Ok(c) => c, Err(_) => return };
                                                                match fluxchat_core::profile::upload_avatar(&client, &mime, data).await {
                                                                    Ok(_) => { status_msg.set(Some("avatar updated".into())); }
                                                                    Err(e) => { status_msg.set(Some(format!("{e}"))); }
                                                                }
                                                            }
                                                        }
                                                    }
                                                }
                                            });
                                        }
                                    },
                                    "change avatar"
                                }
                                // Remove avatar
                                span { style: "font-family: var(--font-mono); font-size: 12px; color: var(--text-dim); cursor: pointer;",
                                    onclick: {
                                        let app_state = app_state.clone();
                                        move |_| {
                                            let app_state = app_state.clone();
                                            spawn(async move {
                                                let client = match app_state.get_client().await { Ok(c) => c, Err(_) => return };
                                                match fluxchat_core::profile::remove_avatar(&client).await {
                                                    Ok(_) => { status_msg.set(Some("avatar removed".into())); }
                                                    Err(e) => { status_msg.set(Some(format!("{e}"))); }
                                                }
                                            });
                                        }
                                    },
                                    "remove avatar"
                                }
                            }
                        }
                        // User ID
                        div { style: "font-size: 11px; color: var(--text-dim); font-family: var(--font-mono); margin-bottom: 8px;", "{user_id}" }
                        // Display name
                        div { style: "display: flex; gap: 6px;",
                            input {
                                style: "flex: 1; background: var(--bg-deep); border: 1px solid var(--border); border-radius: 4px; padding: 6px 8px; color: var(--text-bright); font-family: var(--font-mono); font-size: 13px; outline: none;",
                                value: "{edit_name}",
                                oninput: move |e| edit_name.set(e.value()),
                            }
                            span { style: "font-family: var(--font-mono); font-size: 12px; color: var(--accent); cursor: pointer; padding: 6px 8px; border-radius: 4px; border: 1px solid var(--border); flex-shrink: 0;",
                                onclick: {
                                    let app_state = app_state.clone();
                                    move |_| {
                                        let new_name = edit_name().trim().to_string();
                                        if new_name.is_empty() { return; }
                                        let app_state = app_state.clone();
                                        spawn(async move {
                                            let client = match app_state.get_client().await { Ok(c) => c, Err(_) => return };
                                            match fluxchat_core::profile::set_display_name(&client, &new_name).await {
                                                Ok(_) => {
                                                    let mut ui = consume_context::<UiState>();
                                                    ui.display_name.set(Some(new_name));
                                                    status_msg.set(Some("name updated".into()));
                                                }
                                                Err(e) => { status_msg.set(Some(format!("{e}"))); }
                                            }
                                        });
                                    }
                                },
                                "save"
                            }
                        }
                    }

                    // Banner section
                    div {
                        div { style: "font-size: 11px; font-weight: 700; text-transform: uppercase; color: var(--text-dim); letter-spacing: 0.5px; margin-bottom: 8px; font-family: var(--font-mono);",
                            "banner"
                        }
                        if let Some(ref src) = banner_resolved {
                            img { src: "{src}", style: "width: 100%; height: 80px; object-fit: cover; border-radius: 4px; margin-bottom: 8px;" }
                        } else {
                            div { style: "width: 100%; height: 80px; background: var(--bg-light); border-radius: 4px; margin-bottom: 8px; display: flex; align-items: center; justify-content: center; color: var(--text-dim); font-size: 12px; font-family: var(--font-mono);",
                                "no banner"
                            }
                        }
                        div { style: "display: flex; gap: 8px;",
                            span { style: "font-family: var(--font-mono); font-size: 12px; color: var(--accent); cursor: pointer;",
                                onclick: {
                                    let app_state = app_state.clone();
                                    move |_| {
                                        let app_state = app_state.clone();
                                        spawn(async move {
                                            let mut eval = document::eval(r#"
                                                let input = document.createElement('input');
                                                input.type = 'file';
                                                input.accept = 'image/*';
                                                input.onchange = async () => {
                                                    if (input.files.length > 0) {
                                                        let file = input.files[0];
                                                        let reader = new FileReader();
                                                        reader.onload = () => {
                                                            let base64 = reader.result.split(',')[1];
                                                            dioxus.send(JSON.stringify({ type: file.type, data: base64 }));
                                                        };
                                                        reader.readAsDataURL(file);
                                                    }
                                                };
                                                input.click();
                                            "#);
                                            if let Ok(json) = eval.recv::<String>().await {
                                                if let Ok(parsed) = serde_json::from_str::<serde_json::Value>(&json) {
                                                    let mime = parsed["type"].as_str().unwrap_or("image/png").to_string();
                                                    if let Some(b64) = parsed["data"].as_str() {
                                                        use base64::Engine;
                                                        if let Ok(data) = base64::engine::general_purpose::STANDARD.decode(b64) {
                                                            let client = match app_state.get_client().await { Ok(c) => c, Err(_) => return };
                                                            match fluxchat_core::profile::upload_banner(&client, &mime, data).await {
                                                                Ok(_) => { status_msg.set(Some("banner updated".into())); }
                                                                Err(e) => { status_msg.set(Some(format!("{e}"))); }
                                                            }
                                                        }
                                                    }
                                                }
                                            }
                                        });
                                    }
                                },
                                "change banner"
                            }
                            span { style: "font-family: var(--font-mono); font-size: 12px; color: var(--text-dim); cursor: pointer;",
                                onclick: {
                                    let app_state = app_state.clone();
                                    move |_| {
                                        let app_state = app_state.clone();
                                        spawn(async move {
                                            let client = match app_state.get_client().await { Ok(c) => c, Err(_) => return };
                                            match fluxchat_core::profile::remove_banner(&client).await {
                                                Ok(_) => { status_msg.set(Some("banner removed".into())); }
                                                Err(e) => { status_msg.set(Some(format!("{e}"))); }
                                            }
                                        });
                                    }
                                },
                                "remove banner"
                            }
                        }
                    }

                    // Status
                    if let Some(ref msg) = *status_msg.read() {
                        div { style: "font-size: 12px; color: var(--text-muted); font-family: var(--font-mono);", "{msg}" }
                    }

                    // Session section
                    div {
                        div { style: "font-size: 11px; font-weight: 700; text-transform: uppercase; color: var(--text-dim); letter-spacing: 0.5px; margin-bottom: 8px; font-family: var(--font-mono);",
                            "session"
                        }
                        span {
                            style: "font-size: 13px; color: var(--red); cursor: pointer; font-family: var(--font-mono);",
                            onclick: {
                                let app_state = app_state.clone();
                                move |_| {
                                    let app_state = app_state.clone();
                                    spawn(async move {
                                        let mut ui = consume_context::<UiState>();
                                        if let Ok(client) = app_state.get_client().await {
                                            let _ = client.matrix_auth().logout().await;
                                        }
                                        app_state.db.delete_session();
                                        ui.logged_in.set(false);
                                        ui.settings_open.set(false);
                                    });
                                }
                            },
                            "logout"
                        }
                    }
                }
            }
        }
    }
}

#[component]
fn ProfileModal(user_id: String) -> Element {
    let mut ui = use_context::<UiState>();
    let app_state = use_context::<Arc<AppState>>();

    // Look up member info from any loaded room
    let member = ui.members_by_room.read().values()
        .flat_map(|members| members.iter())
        .find(|m| m.user_id == user_id)
        .cloned();

    let display = member.as_ref().map(|m| m.display_name.clone())
        .unwrap_or_else(|| user_id.split(':').next().unwrap_or(&user_id).trim_start_matches('@').to_string());
    let role = member.as_ref().map(|m| m.role.clone()).unwrap_or_else(|| "Unknown".into());
    let presence = member.as_ref().map(|m| m.presence.clone()).unwrap_or_else(|| "offline".into());
    let server = user_id.split(':').nth(1).unwrap_or("unknown").to_string();
    let nick_class = nick_color_class(&display);
    let initial = display.chars().next().unwrap_or('?').to_uppercase().to_string();
    let presence_color = match presence.as_str() {
        "online" => "var(--green)",
        "idle" => "var(--yellow)",
        _ => "var(--text-dim)",
    };

    // Resolve avatar
    let avatar_mxc = member.as_ref().and_then(|m| m.avatar_url.clone());
    let avatar_data = use_resource({
        let app_state = app_state.clone();
        let mxc = avatar_mxc.clone();
        move || {
            let app_state = app_state.clone();
            let mxc = mxc.clone();
            async move {
                let mxc = mxc?;
                let client = app_state.get_client().await.ok()?;
                fluxchat_core::media::get_media_url(&client, &app_state.db, &app_state.media_cache, &mxc).await.ok()
            }
        }
    });
    let avatar_resolved = avatar_data.read().clone().flatten();

    rsx! {
        // Backdrop
        div {
            style: "position: fixed; inset: 0; background: rgba(0,0,0,0.5); z-index: 1000; display: flex; align-items: center; justify-content: center;",
            onclick: move |_| { ui.profile_user_id.set(None); },

            // Modal
            div {
                style: "background: var(--bg-dark); border: 1px solid var(--border); border-radius: 8px; min-width: 320px; max-width: 400px; overflow: hidden;",
                onclick: move |e| { e.stop_propagation(); },

                // Close button
                div { style: "display: flex; justify-content: flex-end; padding: 8px 12px 0;",
                    span {
                        style: "cursor: pointer; color: var(--text-dim); font-size: 18px; line-height: 1; padding: 4px;",
                        onclick: move |_| { ui.profile_user_id.set(None); },
                        "×"
                    }
                }

                // Content
                div { style: "padding: 0 24px 24px;",

                    // Avatar + name
                    div { style: "display: flex; align-items: center; gap: 12px; margin-bottom: 16px;",
                        if let Some(ref src) = avatar_resolved {
                            img {
                                src: "{src}",
                                style: "width: 48px; height: 48px; border-radius: 50%; object-fit: cover;",
                            }
                        } else {
                            div { class: "{nick_class}", style: "width: 48px; height: 48px; border-radius: 50%; display: flex; align-items: center; justify-content: center; font-size: 20px; font-weight: 700; color: white; background: var(--bg-light);",
                                "{initial}"
                            }
                        }
                        div {
                            div { class: "{nick_class}", style: "font-size: 18px; font-weight: 600;", "{display}" }
                            div { style: "font-size: 12px; color: var(--text-dim); font-family: var(--font-mono);", "{user_id}" }
                        }
                    }

                    // Info rows
                    div { style: "display: flex; flex-direction: column; gap: 8px; font-size: 13px;",
                        div { style: "display: flex; justify-content: space-between;",
                            span { style: "color: var(--text-muted);", "Role" }
                            span { style: "color: var(--text-normal);", "{role}" }
                        }
                        div { style: "display: flex; justify-content: space-between;",
                            span { style: "color: var(--text-muted);", "Status" }
                            span { style: "color: {presence_color};", "{presence}" }
                        }
                        div { style: "display: flex; justify-content: space-between;",
                            span { style: "color: var(--text-muted);", "Server" }
                            span { style: "color: var(--text-normal); font-family: var(--font-mono);", "{server}" }
                        }
                    }
                }
            }
        }
    }
}

/// Verification modal — shown when a verification request comes in.
#[component]
fn VerificationModal(info: fluxchat_core::VerificationInfo) -> Element {
    let mut ui = use_context::<UiState>();
    let app_state = use_context::<Arc<AppState>>();

    let label = if info.is_self_verification {
        "Self-verification request"
    } else {
        "Verification request"
    };

    rsx! {
        div {
            style: "position: fixed; inset: 0; background: rgba(0,0,0,0.5); z-index: 1000; display: flex; align-items: center; justify-content: center;",
            onclick: move |_| { ui.verification_request.set(None); },

            div {
                style: "background: var(--bg-dark); border: 1px solid var(--border); border-radius: 8px; min-width: 360px; max-width: 440px;",
                onclick: move |e| { e.stop_propagation(); },

                // Header
                div { style: "display: flex; justify-content: space-between; align-items: center; padding: 16px 20px; border-bottom: 1px solid var(--border);",
                    span { style: "font-size: 14px; font-weight: 600; color: var(--text-bright); font-family: var(--font-mono);", "{label}" }
                    span {
                        style: "cursor: pointer; color: var(--text-dim); font-size: 18px; padding: 4px;",
                        onclick: move |_| { ui.verification_request.set(None); },
                        "×"
                    }
                }

                div { style: "padding: 20px; display: flex; flex-direction: column; gap: 12px;",
                    div { style: "font-size: 13px; color: var(--text-muted); font-family: var(--font-mono);",
                        "From: {info.other_user_id}"
                    }
                    div { style: "font-size: 12px; color: var(--text-dim); font-family: var(--font-mono);",
                        "Flow: {info.flow_id}"
                    }

                    // Show emojis if available
                    if let Some(ref emojis) = info.emojis {
                        div { style: "font-size: 12px; color: var(--text-muted); font-family: var(--font-mono); margin-top: 4px;",
                            "Verify these emojis match on both devices:"
                        }
                        div { style: "display: flex; flex-wrap: wrap; gap: 8px; justify-content: center; padding: 12px 0;",
                            for e in emojis.iter() {
                                div { style: "display: flex; flex-direction: column; align-items: center; gap: 2px; width: 60px;",
                                    span { style: "font-size: 24px;", "{e.symbol}" }
                                    span { style: "font-size: 9px; color: var(--text-dim); text-align: center; font-family: var(--font-mono);", "{e.description}" }
                                }
                            }
                        }
                    }

                    // Action buttons
                    div { style: "display: flex; gap: 8px; justify-content: flex-end; margin-top: 8px;",
                        // Cancel
                        span { style: "font-family: var(--font-mono); font-size: 13px; color: var(--text-muted); cursor: pointer; padding: 4px 12px; border-radius: 4px; border: 1px solid var(--border);",
                            onclick: {
                                let flow_id = info.flow_id.clone();
                                let user_id = info.other_user_id.clone();
                                let app_state = app_state.clone();
                                move |_| {
                                    ui.verification_request.set(None);
                                    let flow_id = flow_id.clone();
                                    let user_id = user_id.clone();
                                    let app_state = app_state.clone();
                                    spawn(async move {
                                        let client = match app_state.get_client().await { Ok(c) => c, Err(_) => return };
                                        let _ = fluxchat_core::verification::cancel_verification(&client, &user_id, &flow_id).await;
                                    });
                                }
                            },
                            "cancel"
                        }
                        // Accept / Confirm
                        { let has_emojis = info.emojis.is_some();
                          let btn_label = if has_emojis { "confirm match" } else { "accept" };
                          let flow_id = info.flow_id.clone();
                          let user_id = info.other_user_id.clone();
                          rsx! {
                            span { style: "font-family: var(--font-mono); font-size: 13px; color: var(--green); cursor: pointer; padding: 4px 12px; border-radius: 4px; border: 1px solid var(--green);",
                                onclick: {
                                    let flow_id = flow_id.clone();
                                    let user_id = user_id.clone();
                                    let app_state = app_state.clone();
                                    move |_| {
                                        let flow_id = flow_id.clone();
                                        let user_id = user_id.clone();
                                        let app_state = app_state.clone();
                                        spawn(async move {
                                            let client = match app_state.get_client().await { Ok(c) => c, Err(_) => return };
                                            if has_emojis {
                                                let _ = fluxchat_core::verification::confirm_verification(&client, &user_id, &flow_id).await;
                                            } else {
                                                let _ = fluxchat_core::verification::accept_verification(
                                                    &client, &user_id, &flow_id,
                                                    |info| {
                                                        if let Some(tx) = crate::app::SYNC_TX.get() {
                                                            let _ = tx.send(crate::app::SyncEvent::Verification(info));
                                                        }
                                                    },
                                                    || {},
                                                ).await;
                                            }
                                            let mut ui = consume_context::<UiState>();
                                            if has_emojis {
                                                ui.verification_request.set(None);
                                            }
                                        });
                                    }
                                },
                                "{btn_label}"
                            }
                          }
                        }
                    }
                }
            }
        }
    }
}

/// Create Room modal.
#[component]
fn CreateRoomModal() -> Element {
    let mut ui = use_context::<UiState>();
    let app_state = use_context::<Arc<AppState>>();

    let mut name = use_signal(|| String::new());
    let mut topic = use_signal(|| String::new());
    let mut is_public = use_signal(|| false);
    let mut encrypted = use_signal(|| true);
    let mut join_input = use_signal(|| String::new());
    let mut creating = use_signal(|| false);
    let mut error_msg = use_signal(|| Option::<String>::None);

    rsx! {
        div {
            style: "position: fixed; inset: 0; background: rgba(0,0,0,0.5); z-index: 1000; display: flex; align-items: center; justify-content: center;",
            onclick: move |_| { ui.create_room_open.set(false); },
            div {
                style: "background: var(--bg-dark); border: 1px solid var(--border); border-radius: 8px; min-width: 400px; max-width: 480px;",
                onclick: move |e| { e.stop_propagation(); },
                div { style: "display: flex; justify-content: space-between; align-items: center; padding: 16px 20px; border-bottom: 1px solid var(--border);",
                    span { style: "font-size: 14px; font-weight: 600; color: var(--text-bright); font-family: var(--font-mono);", "create room" }
                    span { style: "cursor: pointer; color: var(--text-dim); font-size: 18px; padding: 4px;",
                        onclick: move |_| { ui.create_room_open.set(false); }, "×"
                    }
                }
                div { style: "padding: 20px; display: flex; flex-direction: column; gap: 12px;",
                    div {
                        div { style: "font-size: 11px; font-weight: 700; text-transform: uppercase; color: var(--text-dim); letter-spacing: 0.5px; margin-bottom: 4px; font-family: var(--font-mono);", "name" }
                        input {
                            style: "width: 100%; background: var(--bg-deep); border: 1px solid var(--border); border-radius: 4px; padding: 6px 8px; color: var(--text-bright); font-family: var(--font-mono); font-size: 13px; outline: none;",
                            placeholder: "room-name", value: "{name}", oninput: move |e| name.set(e.value()),
                        }
                    }
                    div {
                        div { style: "font-size: 11px; font-weight: 700; text-transform: uppercase; color: var(--text-dim); letter-spacing: 0.5px; margin-bottom: 4px; font-family: var(--font-mono);", "topic" }
                        input {
                            style: "width: 100%; background: var(--bg-deep); border: 1px solid var(--border); border-radius: 4px; padding: 6px 8px; color: var(--text-bright); font-family: var(--font-mono); font-size: 13px; outline: none;",
                            placeholder: "optional topic", value: "{topic}", oninput: move |e| topic.set(e.value()),
                        }
                    }
                    div { style: "display: flex; gap: 16px; font-size: 13px; font-family: var(--font-mono);",
                        label { style: "display: flex; align-items: center; gap: 4px; color: var(--text-muted); cursor: pointer;",
                            input { r#type: "checkbox", checked: *is_public.read(), onchange: move |e| is_public.set(e.checked()) }
                            "public"
                        }
                        label { style: "display: flex; align-items: center; gap: 4px; color: var(--text-muted); cursor: pointer;",
                            input { r#type: "checkbox", checked: *encrypted.read(), onchange: move |e| encrypted.set(e.checked()) }
                            "encrypted"
                        }
                    }
                    div { style: "border-top: 1px solid var(--border); margin: 4px 0;" }
                    div {
                        div { style: "font-size: 11px; font-weight: 700; text-transform: uppercase; color: var(--text-dim); letter-spacing: 0.5px; margin-bottom: 4px; font-family: var(--font-mono);", "or join room" }
                        input {
                            style: "width: 100%; background: var(--bg-deep); border: 1px solid var(--border); border-radius: 4px; padding: 6px 8px; color: var(--text-bright); font-family: var(--font-mono); font-size: 13px; outline: none;",
                            placeholder: "#room:server.org or !roomid:server.org", value: "{join_input}", oninput: move |e| join_input.set(e.value()),
                        }
                    }
                    if let Some(ref err) = *error_msg.read() {
                        div { style: "font-size: 12px; color: var(--red); font-family: var(--font-mono);", "{err}" }
                    }
                    div { style: "display: flex; gap: 8px; justify-content: flex-end; margin-top: 4px;",
                        span { style: "font-family: var(--font-mono); font-size: 13px; color: var(--text-muted); cursor: pointer; padding: 4px 12px; border-radius: 4px; border: 1px solid var(--border);",
                            onclick: move |_| { ui.create_room_open.set(false); }, "cancel"
                        }
                        if !join_input().trim().is_empty() {
                            span { style: "font-family: var(--font-mono); font-size: 13px; color: var(--accent); cursor: pointer; padding: 4px 12px; border-radius: 4px; border: 1px solid var(--accent);",
                                onclick: {
                                    let app_state = app_state.clone();
                                    move |_| {
                                        let r = join_input().trim().to_string();
                                        let app_state = app_state.clone();
                                        creating.set(true); error_msg.set(None);
                                        spawn(async move {
                                            let client = match app_state.get_client().await { Ok(c) => c, Err(_) => { creating.set(false); return; } };
                                            match fluxchat_core::rooms::join_room(&client, &r).await {
                                                Ok(_) => { consume_context::<UiState>().create_room_open.set(false); }
                                                Err(e) => { error_msg.set(Some(format!("{e}"))); }
                                            }
                                            creating.set(false);
                                        });
                                    }
                                },
                                if *creating.read() { "joining..." } else { "join" }
                            }
                        }
                        if !name().trim().is_empty() {
                            span { style: "font-family: var(--font-mono); font-size: 13px; color: var(--green); cursor: pointer; padding: 4px 12px; border-radius: 4px; border: 1px solid var(--green);",
                                onclick: {
                                    let app_state = app_state.clone();
                                    move |_| {
                                        let rn = name().trim().to_string();
                                        let rt = topic().trim().to_string();
                                        let public = *is_public.read(); let enc = *encrypted.read();
                                        let app_state = app_state.clone();
                                        creating.set(true); error_msg.set(None);
                                        spawn(async move {
                                            let client = match app_state.get_client().await { Ok(c) => c, Err(_) => { creating.set(false); return; } };
                                            let t = if rt.is_empty() { None } else { Some(rt.as_str()) };
                                            match fluxchat_core::rooms::create_room(&client, &rn, t, public, enc, &[], None).await {
                                                Ok(_) => { consume_context::<UiState>().create_room_open.set(false); }
                                                Err(e) => { error_msg.set(Some(format!("{e}"))); }
                                            }
                                            creating.set(false);
                                        });
                                    }
                                },
                                if *creating.read() { "creating..." } else { "create" }
                            }
                        }
                    }
                }
            }
        }
    }
}

/// Room Settings modal.
#[component]
fn RoomSettingsModal() -> Element {
    let mut ui = use_context::<UiState>();
    let app_state = use_context::<Arc<AppState>>();
    let selected = ui.selected_room_id.read().clone();
    let room_info = selected.as_ref()
        .and_then(|id| ui.rooms.read().iter().find(|r| r.room_id == *id).cloned());
    let room = match room_info { Some(r) => r, None => { ui.room_settings_open.set(false); return rsx! {}; } };

    let mut edit_name = use_signal(|| room.name.clone());
    let mut edit_topic = use_signal(|| room.topic.clone().unwrap_or_default());
    let mut invite_user = use_signal(|| String::new());
    let mut status_msg = use_signal(|| Option::<String>::None);
    let room_id = room.room_id.clone();

    rsx! {
        div {
            style: "position: fixed; inset: 0; background: rgba(0,0,0,0.5); z-index: 1000; display: flex; align-items: center; justify-content: center;",
            onclick: move |_| { ui.room_settings_open.set(false); },
            div {
                style: "background: var(--bg-dark); border: 1px solid var(--border); border-radius: 8px; min-width: 400px; max-width: 480px;",
                onclick: move |e| { e.stop_propagation(); },
                div { style: "display: flex; justify-content: space-between; align-items: center; padding: 16px 20px; border-bottom: 1px solid var(--border);",
                    span { style: "font-size: 14px; font-weight: 600; color: var(--text-bright); font-family: var(--font-mono);", "room settings" }
                    span { style: "cursor: pointer; color: var(--text-dim); font-size: 18px; padding: 4px;",
                        onclick: move |_| { ui.room_settings_open.set(false); }, "×"
                    }
                }
                div { style: "padding: 20px; display: flex; flex-direction: column; gap: 12px;",
                    div { style: "font-size: 11px; color: var(--text-dim); font-family: var(--font-mono);", "{room_id}" }
                    // Name
                    div {
                        div { style: "font-size: 11px; font-weight: 700; text-transform: uppercase; color: var(--text-dim); letter-spacing: 0.5px; margin-bottom: 4px; font-family: var(--font-mono);", "name" }
                        div { style: "display: flex; gap: 6px;",
                            input { style: "flex: 1; background: var(--bg-deep); border: 1px solid var(--border); border-radius: 4px; padding: 6px 8px; color: var(--text-bright); font-family: var(--font-mono); font-size: 13px; outline: none;",
                                value: "{edit_name}", oninput: move |e| edit_name.set(e.value()),
                            }
                            span { style: "font-family: var(--font-mono); font-size: 12px; color: var(--accent); cursor: pointer; padding: 6px 8px; border-radius: 4px; border: 1px solid var(--border); flex-shrink: 0;",
                                onclick: { let app_state = app_state.clone(); let rid = room_id.clone();
                                    move |_| { let n = edit_name().trim().to_string(); let app_state = app_state.clone(); let rid = rid.clone();
                                        spawn(async move { let client = match app_state.get_client().await { Ok(c) => c, Err(_) => return };
                                            match fluxchat_core::rooms::set_room_name(&client, &rid, &n).await { Ok(_) => { status_msg.set(Some("name updated".into())); } Err(e) => { status_msg.set(Some(format!("{e}"))); } }
                                        });
                                    }
                                }, "save"
                            }
                        }
                    }
                    // Topic
                    div {
                        div { style: "font-size: 11px; font-weight: 700; text-transform: uppercase; color: var(--text-dim); letter-spacing: 0.5px; margin-bottom: 4px; font-family: var(--font-mono);", "topic" }
                        div { style: "display: flex; gap: 6px;",
                            input { style: "flex: 1; background: var(--bg-deep); border: 1px solid var(--border); border-radius: 4px; padding: 6px 8px; color: var(--text-bright); font-family: var(--font-mono); font-size: 13px; outline: none;",
                                value: "{edit_topic}", oninput: move |e| edit_topic.set(e.value()),
                            }
                            span { style: "font-family: var(--font-mono); font-size: 12px; color: var(--accent); cursor: pointer; padding: 6px 8px; border-radius: 4px; border: 1px solid var(--border); flex-shrink: 0;",
                                onclick: { let app_state = app_state.clone(); let rid = room_id.clone();
                                    move |_| { let t = edit_topic().trim().to_string(); let app_state = app_state.clone(); let rid = rid.clone();
                                        spawn(async move { let client = match app_state.get_client().await { Ok(c) => c, Err(_) => return };
                                            match fluxchat_core::rooms::set_room_topic(&client, &rid, &t).await { Ok(_) => { status_msg.set(Some("topic updated".into())); } Err(e) => { status_msg.set(Some(format!("{e}"))); } }
                                        });
                                    }
                                }, "save"
                            }
                        }
                    }
                    div { style: "display: flex; gap: 16px; font-size: 12px; font-family: var(--font-mono); color: var(--text-muted);",
                        if room.is_encrypted { span { style: "color: var(--green);", "encrypted" } } else { span { "unencrypted" } }
                        if room.is_direct { span { "direct message" } }
                    }
                    // Invite
                    div {
                        div { style: "font-size: 11px; font-weight: 700; text-transform: uppercase; color: var(--text-dim); letter-spacing: 0.5px; margin-bottom: 4px; font-family: var(--font-mono);", "invite user" }
                        div { style: "display: flex; gap: 6px;",
                            input { style: "flex: 1; background: var(--bg-deep); border: 1px solid var(--border); border-radius: 4px; padding: 6px 8px; color: var(--text-bright); font-family: var(--font-mono); font-size: 13px; outline: none;",
                                placeholder: "@user:server.org", value: "{invite_user}", oninput: move |e| invite_user.set(e.value()),
                            }
                            span { style: "font-family: var(--font-mono); font-size: 12px; color: var(--green); cursor: pointer; padding: 6px 8px; border-radius: 4px; border: 1px solid var(--border); flex-shrink: 0;",
                                onclick: { let app_state = app_state.clone(); let rid = room_id.clone();
                                    move |_| { let u = invite_user().trim().to_string(); if u.is_empty() { return; }
                                        let app_state = app_state.clone(); let rid = rid.clone();
                                        spawn(async move { let client = match app_state.get_client().await { Ok(c) => c, Err(_) => return };
                                            match fluxchat_core::rooms::invite_user(&client, &rid, &u).await { Ok(_) => { invite_user.set(String::new()); status_msg.set(Some(format!("invited {u}"))); } Err(e) => { status_msg.set(Some(format!("{e}"))); } }
                                        });
                                    }
                                }, "invite"
                            }
                        }
                    }
                    if let Some(ref msg) = *status_msg.read() {
                        div { style: "font-size: 12px; color: var(--text-muted); font-family: var(--font-mono);", "{msg}" }
                    }
                    div { style: "border-top: 1px solid var(--border); padding-top: 12px; margin-top: 4px;",
                        span { style: "font-family: var(--font-mono); font-size: 13px; color: var(--red); cursor: pointer;",
                            onclick: { let app_state = app_state.clone(); let rid = room_id.clone();
                                move |_| { let app_state = app_state.clone(); let rid = rid.clone();
                                    spawn(async move { let client = match app_state.get_client().await { Ok(c) => c, Err(_) => return };
                                        let _ = fluxchat_core::rooms::leave_room(&client, &rid).await;
                                        let mut ui = consume_context::<UiState>(); ui.room_settings_open.set(false); ui.selected_room_id.set(None);
                                    });
                                }
                            }, "leave room"
                        }
                    }
                }
            }
        }
    }
}
