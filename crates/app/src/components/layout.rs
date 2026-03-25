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

    let user_id = ui.user_id.read().clone();
    let selected_room = ui.selected_room_id.read().clone();
    let room_name = selected_room.as_ref()
        .and_then(|id| ui.rooms.read().iter().find(|r| r.room_id == *id).map(|r| r.name.clone()))
        .unwrap_or_default();
    let member_count = selected_room.as_ref()
        .and_then(|id| ui.members_by_room.read().get(id).map(|m| m.len()))
        .unwrap_or(0);
    let my_power = selected_room.as_ref()
        .and_then(|id| {
            let my_id = ui.user_id.peek().clone();
            ui.members_by_room.read().get(id)
                .and_then(|members| members.iter().find(|m| m.user_id == my_id).map(|m| m.power_level))
        })
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

        // Forward message modal
        { let fwd = ui.forward_event.read().clone();
          if fwd.is_some() {
            rsx! { ForwardModal {} }
          } else { rsx! {} }
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
                                                    .set_file_name(format!("image.{ext}"))
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
                onclick: move |_| { let v = *ui.settings_open.peek(); ui.settings_open.set(!v); },
                span { style: "display: inline-flex; opacity: 0.6;", dangerous_inner_html: r#"<svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><circle cx="12" cy="12" r="3"/><path d="M19.4 15a1.65 1.65 0 0 0 .33 1.82l.06.06a2 2 0 0 1-2.83 2.83l-.06-.06a1.65 1.65 0 0 0-1.82-.33 1.65 1.65 0 0 0-1 1.51V21a2 2 0 0 1-4 0v-.09A1.65 1.65 0 0 0 9 19.4a1.65 1.65 0 0 0-1.82.33l-.06.06a2 2 0 0 1-2.83-2.83l.06-.06A1.65 1.65 0 0 0 4.68 15a1.65 1.65 0 0 0-1.51-1H3a2 2 0 0 1 0-4h.09A1.65 1.65 0 0 0 4.6 9a1.65 1.65 0 0 0-.33-1.82l-.06-.06a2 2 0 0 1 2.83-2.83l.06.06A1.65 1.65 0 0 0 9 4.68a1.65 1.65 0 0 0 1-1.51V3a2 2 0 0 1 4 0v.09a1.65 1.65 0 0 0 1 1.51 1.65 1.65 0 0 0 1.82-.33l.06-.06a2 2 0 0 1 2.83 2.83l-.06.06A1.65 1.65 0 0 0 19.4 9a1.65 1.65 0 0 0 1.51 1H21a2 2 0 0 1 0 4h-.09a1.65 1.65 0 0 0-1.51 1z"/></svg>"# }
                UserAvatar {}
                span { style: "font-family: var(--font-mono); color: var(--green);", "{user_id}" }
            }
            span { class: "sep", "│" }
            if let Some(_) = &selected_room {
                span { "#{room_name}" }
                if my_power >= 100 {
                    span { style: "color: var(--yellow); font-family: var(--font-mono); font-size: 12px; margin-left: 4px;", "@" }
                } else if my_power >= 50 {
                    span { style: "color: var(--accent-muted); font-family: var(--font-mono); font-size: 12px; margin-left: 4px;", "+" }
                }
                span { class: "sep", "│" }
                span { "{member_count} nicks" }
                span { class: "sep", "│" }
                if is_encrypted {
                    span { style: "color: var(--green); display: inline-flex; align-items: center; gap: 4px;",
                        span { style: "display: inline-flex; opacity: 0.7;", dangerous_inner_html: r#"<svg width="11" height="11" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><rect x="3" y="11" width="18" height="11" rx="2" ry="2"/><path d="M7 11V7a5 5 0 0 1 10 0v4"/></svg>"# }
                        "encrypted"
                    }
                } else {
                    span { "unencrypted" }
                }
            } else {
                span { "no channel selected" }
            }
            span { class: "sep", "│" }
            SyncLag {}
            span { class: "sep", "│" }
            DeviceStatus {}
        }
    }
}

/// Right panel for pins, search, threads.
#[component]
fn RightPanelView(panel: RightPanel) -> Element {
    let mut ui = use_context::<UiState>();

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
                    RightPanel::Threads => rsx! { ThreadsPanel {} },
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

/// Threads panel — shows thread roots from the current room.
#[component]
fn ThreadsPanel() -> Element {
    let ui = use_context::<UiState>();
    let selected = ui.selected_room_id.read().clone();

    // Derive thread roots by scanning thread_root references in replies
    let threads: Vec<_> = if let Some(ref room_id) = selected {
        let msgs = ui.messages_by_room.read();
        let messages = msgs.get(room_id);
        if let Some(messages) = messages {
            // Count replies per thread root
            let mut counts: std::collections::HashMap<String, u32> = std::collections::HashMap::new();
            for m in messages.iter() {
                if let Some(ref root) = m.thread_root {
                    *counts.entry(root.clone()).or_insert(0) += 1;
                }
            }
            // Find the root messages and pair with counts
            counts.iter()
                .filter_map(|(root_eid, count)| {
                    messages.iter()
                        .find(|m| m.event_id == *root_eid)
                        .map(|m| (m.event_id.clone(), m.sender_name.clone(), m.body.clone(), *count, m.timestamp))
                })
                .collect()
        } else { Vec::new() }
    } else {
        Vec::new()
    };

    if threads.is_empty() {
        return rsx! {
            div { style: "color: var(--text-dim); font-size: 13px; font-family: var(--font-mono); text-align: center; padding: 20px;",
                "No threads in this room"
            }
        };
    }

    rsx! {
        div { style: "display: flex; flex-direction: column; gap: 4px;",
            for (eid, sender, body, count, ts) in threads.iter() {
                { let label = if *count == 1 { "1 reply".to_string() } else { format!("{count} replies") };
                  let time = {
                      let secs = ts % 86400;
                      let h = secs / 3600;
                      let m = (secs % 3600) / 60;
                      format!("{:02}:{:02}", h, m)
                  };
                  let preview = if body.len() > 60 { format!("{}...", &body[..60]) } else { body.clone() };
                  { let thread_eid = eid.clone();
                    let thread_body = body.clone();
                    let room_id = selected.clone();
                    let ui_thread = ui.clone();
                  rsx! {
                    div { style: "background: var(--bg-mid); border-radius: 4px; padding: 8px; cursor: pointer; transition: background 0.1s;",
                        class: "cmd-item",
                        onclick: {
                            move |_| {
                                if let Some(ref room_id) = room_id {
                                    open_thread_tab(ui_thread.clone(), room_id.clone(), thread_eid.clone(), thread_body.clone());
                                }
                            }
                        },
                        div { style: "display: flex; justify-content: space-between; align-items: center; margin-bottom: 4px;",
                            span { style: "font-family: var(--font-mono); font-size: 12px; font-weight: 600; color: var(--text-bright);", "{sender}" }
                            span { style: "font-family: var(--font-mono); font-size: 10px; color: var(--text-dim);", "{time}" }
                        }
                        div { style: "font-family: var(--font-mono); font-size: 12px; color: var(--text-muted); overflow: hidden; text-overflow: ellipsis; white-space: nowrap; margin-bottom: 4px;",
                            "{preview}"
                        }
                        div { style: "font-family: var(--font-mono); font-size: 11px; color: var(--accent-muted);",
                            "{label}"
                        }
                    }
                  }
                }
            } }
        }
    }
}

/// Open a thread as a new tab with thread messages loaded.
pub fn open_thread_tab(mut ui: UiState, room_id: String, thread_root_eid: String, root_body: String) {
    let thread_tab_id = format!("thread::{room_id}::{thread_root_eid}");

    // Gather thread messages: root + all replies referencing this root
    let thread_msgs: Vec<_> = ui.messages_by_room.read()
        .get(&room_id)
        .map(|msgs| msgs.iter()
            .filter(|m| m.event_id == thread_root_eid || m.thread_root.as_deref() == Some(&thread_root_eid))
            .cloned()
            .collect())
        .unwrap_or_default();

    // Store thread messages under the synthetic key
    ui.messages_by_room.write().insert(thread_tab_id.clone(), thread_msgs.clone());

    // Tab name: root body if meaningful, else sender context
    let sender = thread_msgs.first().map(|m| m.sender_name.clone()).unwrap_or_default();
    let tab_name = if root_body.len() > 3 {
        let preview = if root_body.len() > 30 { format!("{}...", &root_body[..30]) } else { root_body };
        format!("\u{1F5E8} {preview}")
    } else {
        let room_name = ui.rooms.read().iter()
            .find(|r| r.room_id == room_id)
            .map(|r| r.name.to_lowercase().replace(' ', "-"))
            .unwrap_or_default();
        format!("\u{1F5E8} {sender} in #{room_name}")
    };

    let mut tabs = ui.open_tabs.write();
    if !tabs.iter().any(|(id, _, _, _)| id == &thread_tab_id) {
        if tabs.len() >= 8 { tabs.remove(0); }
        tabs.push((thread_tab_id.clone(), tab_name, None, None));
    }
    drop(tabs);

    ui.selected_room_id.set(Some(thread_tab_id));
    ui.right_panel.set(None);
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

/// Sync lag meter — periodically pings the homeserver and displays latency.
#[component]
fn SyncLag() -> Element {
    let mut ui = use_context::<UiState>();
    let app_state = use_context::<Arc<AppState>>();

    // Periodically measure sync lag with a lightweight server request
    use_future({
        let app_state = app_state.clone();
        move || {
            let app_state = app_state.clone();
            async move {
                loop {
                    if let Ok(client) = app_state.get_client().await {
                        let start = std::time::Instant::now();
                        // Use get_display_name as a lightweight server ping
                        let _ = client.account().get_display_name().await;
                        let lag = start.elapsed().as_millis() as u64;
                        ui.sync_lag_ms.set(lag);
                    }
                    tokio::time::sleep(std::time::Duration::from_secs(30)).await;
                }
            }
        }
    });

    let lag = *ui.sync_lag_ms.read();
    let color = if lag < 200 {
        "var(--green)"
    } else if lag < 1000 {
        "var(--yellow)"
    } else {
        "var(--red)"
    };
    let label = if lag == 0 { "synced".to_string() } else { format!("{lag}ms") };

    rsx! {
        span { style: "color: {color}; font-family: var(--font-mono);", "{label}" }
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
        let (icon_svg, color) = if is_verified {
            (r#"<svg width="11" height="11" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="3" stroke-linecap="round" stroke-linejoin="round"><polyline points="20 6 9 17 4 12"/></svg>"#, "var(--green)")
        } else {
            (r#"<svg width="11" height="11" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="3" stroke-linecap="round" stroke-linejoin="round"><line x1="18" y1="6" x2="6" y2="18"/><line x1="6" y1="6" x2="18" y2="18"/></svg>"#, "var(--yellow)")
        };
        let label = if is_verified { "verified" } else { "unverified" };
        let short_id = if device_id.len() > 10 { &device_id[..10] } else { &device_id };
        let tooltip = format!("{} ({}/{} devices verified)", device_id, verified, total);
        let mut ui = consume_context::<UiState>();
        rsx! {
            span {
                style: "color: {color}; font-family: var(--font-mono); cursor: pointer; display: inline-flex; align-items: center; gap: 4px;",
                title: "{tooltip}",
                onclick: move |_| { ui.settings_tab.set("security".into()); ui.settings_open.set(true); },
                span { style: "display: inline-flex;", dangerous_inner_html: icon_svg }
                "{label}"
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

/// Settings modal — sidebar nav with /account, /sessions sections.
#[component]
fn SettingsModal() -> Element {
    let mut ui = use_context::<UiState>();
    let tab = ui.settings_tab.read().clone();

    rsx! {
        div {
            style: "position: fixed; inset: 0; background: rgba(0,0,0,0.5); z-index: 1000; display: flex; align-items: center; justify-content: center;",
            tabindex: "0",
            autofocus: true,
            onclick: move |_| { ui.settings_open.set(false); },
            onkeydown: move |e: Event<KeyboardData>| {
                if e.key() == Key::Escape { ui.settings_open.set(false); }
            },

            div {
                style: "background: var(--bg-dark); border: 1px solid var(--border); border-radius: 8px; width: 700px; max-width: 90vw; height: 520px; max-height: 80vh; display: flex; overflow: hidden;",
                onclick: move |e| { e.stop_propagation(); },

                // Sidebar nav
                div { style: "width: 160px; flex-shrink: 0; background: var(--bg-deep); border-right: 1px solid var(--border); padding: 16px 0; display: flex; flex-direction: column;",
                    div { style: "padding: 0 12px 12px; font-size: 11px; font-weight: 700; text-transform: uppercase; color: var(--text-dim); letter-spacing: 0.5px; font-family: var(--font-mono);",
                        "settings"
                    }

                    SettingsNavItem { label: "/account", tab_id: "account", active_tab: tab.clone() }
                    SettingsNavItem { label: "/sessions", tab_id: "sessions", active_tab: tab.clone() }
                    SettingsNavItem { label: "/security", tab_id: "security", active_tab: tab.clone() }

                    // Spacer
                    div { style: "flex: 1;" }

                    // Close
                    div { style: "padding: 8px 12px;",
                        span {
                            style: "font-family: var(--font-mono); font-size: 12px; color: var(--text-dim); cursor: pointer; display: block; padding: 6px 8px; border-radius: 4px;",
                            onclick: move |_| { ui.settings_open.set(false); },
                            "esc close"
                        }
                    }
                }

                // Content area
                div { style: "flex: 1; overflow-y: auto; display: flex; flex-direction: column;",
                    // Header
                    div { style: "display: flex; justify-content: space-between; align-items: center; padding: 16px 20px; border-bottom: 1px solid var(--border); flex-shrink: 0;",
                        span { style: "font-size: 14px; font-weight: 600; color: var(--text-bright); font-family: var(--font-mono);",
                            match tab.as_str() {
                                "sessions" => "sessions",
                                "security" => "security",
                                _ => "account",
                            }
                        }
                        span {
                            style: "cursor: pointer; color: var(--text-dim); font-size: 18px; padding: 4px;",
                            onclick: move |_| { ui.settings_open.set(false); },
                            "×"
                        }
                    }

                    // Page content
                    div { style: "flex: 1; padding: 20px; overflow-y: auto;",
                        match tab.as_str() {
                            "sessions" => rsx! { SettingsSessionsPage {} },
                            "security" => rsx! { SettingsSecurityPage {} },
                            _ => rsx! { SettingsAccountPage {} },
                        }
                    }
                }
            }
        }
    }
}

#[component]
fn SettingsNavItem(label: String, tab_id: String, active_tab: String) -> Element {
    let mut ui = use_context::<UiState>();
    let is_active = active_tab == tab_id;
    let bg = if is_active { "var(--bg-mid)" } else { "transparent" };
    let color = if is_active { "var(--text-bright)" } else { "var(--text-muted)" };
    let tid = tab_id.clone();

    rsx! {
        div {
            style: "padding: 6px 12px; cursor: pointer; font-family: var(--font-mono); font-size: 13px; color: {color}; background: {bg}; transition: background 0.1s, color 0.1s;",
            onclick: move |_| { ui.settings_tab.set(tid.clone()); },
            "{label}"
        }
    }
}

/// Account settings page — profile, banner, logout.
#[component]
fn SettingsAccountPage() -> Element {
    let ui = use_context::<UiState>();
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
        div { style: "display: flex; flex-direction: column; gap: 16px;",

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

            // Logout
            div { style: "border-top: 1px solid var(--border); padding-top: 16px; margin-top: 4px;",
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

/// Sessions settings page — list all devices/sessions.
#[component]
fn SettingsSessionsPage() -> Element {
    let app_state = use_context::<Arc<AppState>>();
    let mut status_msg = use_signal(|| Option::<String>::None);
    let mut pw_input = use_signal(String::new);
    let mut signing_out = use_signal(|| Option::<String>::None); // device_id being signed out
    let mut confirm_signout = use_signal(|| Option::<String>::None); // device_id awaiting password

    // Fetch device list
    let mut devices = use_resource({
        let app_state = app_state.clone();
        move || {
            let app_state = app_state.clone();
            async move {
                let client = app_state.get_client().await.ok()?;
                fluxchat_core::devices::list_devices(&client).await.ok()
            }
        }
    });

    let device_list = devices.read().clone().flatten().unwrap_or_default();
    let current_device = device_list.iter().find(|d| d.is_current).cloned();
    let other_devices: Vec<_> = device_list.iter().filter(|d| !d.is_current).cloned().collect();

    rsx! {
        div { style: "display: flex; flex-direction: column; gap: 16px;",

            // Current session
            div {
                div { style: "font-size: 11px; font-weight: 700; text-transform: uppercase; color: var(--text-dim); letter-spacing: 0.5px; margin-bottom: 8px; font-family: var(--font-mono);",
                    "current session"
                }
                if let Some(ref dev) = current_device {
                    SessionRow { device: dev.clone(), is_current: true }
                } else {
                    div { style: "font-family: var(--font-mono); font-size: 12px; color: var(--text-dim); animation: pulse 1.5s ease-in-out infinite;",
                        "loading..."
                    }
                }
            }

            // Other sessions
            div {
                div { style: "display: flex; justify-content: space-between; align-items: center; margin-bottom: 8px;",
                    div { style: "font-size: 11px; font-weight: 700; text-transform: uppercase; color: var(--text-dim); letter-spacing: 0.5px; font-family: var(--font-mono);",
                        "other sessions ({other_devices.len()})"
                    }
                }

                if other_devices.is_empty() && !device_list.is_empty() {
                    div { style: "font-family: var(--font-mono); font-size: 12px; color: var(--text-dim); padding: 8px 0;",
                        "no other sessions"
                    }
                }

                div { style: "display: flex; flex-direction: column; gap: 2px;",
                    for dev in other_devices.iter() {
                        div { style: "display: flex; align-items: center; gap: 8px;",
                            div { style: "flex: 1;",
                                SessionRow { device: dev.clone(), is_current: false }
                            }

                            // Sign out button or password prompt
                            if confirm_signout.read().as_ref() == Some(&dev.device_id) {
                                // Password input for UIAA
                                div { style: "display: flex; gap: 4px; align-items: center; flex-shrink: 0;",
                                    input {
                                        style: "width: 120px; background: var(--bg-deep); border: 1px solid var(--border); border-radius: 3px; padding: 3px 6px; color: var(--text-bright); font-family: var(--font-mono); font-size: 11px; outline: none;",
                                        r#type: "password",
                                        placeholder: "password",
                                        value: "{pw_input}",
                                        oninput: move |e| pw_input.set(e.value()),
                                        onkeydown: {
                                            let app_state = app_state.clone();
                                            let dev_id = dev.device_id.clone();
                                            move |e: Event<KeyboardData>| {
                                                if e.key() == Key::Enter {
                                                    let pw = pw_input().clone();
                                                    let dev_id = dev_id.clone();
                                                    let app_state = app_state.clone();
                                                    signing_out.set(Some(dev_id.clone()));
                                                    spawn(async move {
                                                        let client = match app_state.get_client().await { Ok(c) => c, Err(_) => { signing_out.set(None); return; } };
                                                        match fluxchat_core::devices::delete_devices(&client, std::slice::from_ref(&dev_id), &pw).await {
                                                            Ok(_) => {
                                                                status_msg.set(Some(format!("signed out {}", &dev_id[..dev_id.len().min(10)])));
                                                                confirm_signout.set(None);
                                                                pw_input.set(String::new());
                                                                devices.restart();
                                                            }
                                                            Err(e) => { status_msg.set(Some(format!("{e}"))); }
                                                        }
                                                        signing_out.set(None);
                                                    });
                                                } else if e.key() == Key::Escape {
                                                    confirm_signout.set(None);
                                                    pw_input.set(String::new());
                                                }
                                            }
                                        },
                                    }
                                    span { style: "font-family: var(--font-mono); font-size: 11px; color: var(--text-dim); cursor: pointer;",
                                        onclick: move |_| { confirm_signout.set(None); pw_input.set(String::new()); },
                                        "×"
                                    }
                                }
                            } else if signing_out.read().as_ref() == Some(&dev.device_id) {
                                span { style: "font-family: var(--font-mono); font-size: 11px; color: var(--text-dim); flex-shrink: 0;",
                                    "..."
                                }
                            } else {
                                span {
                                    style: "font-family: var(--font-mono); font-size: 11px; color: var(--red); cursor: pointer; padding: 3px 6px; border-radius: 3px; flex-shrink: 0; opacity: 0.7; transition: opacity 0.1s;",
                                    onclick: {
                                        let dev_id = dev.device_id.clone();
                                        move |_| { confirm_signout.set(Some(dev_id.clone())); }
                                    },
                                    "sign out"
                                }
                            }
                        }
                    }
                }
            }

            // Status message
            if let Some(ref msg) = *status_msg.read() {
                div { style: "font-size: 12px; color: var(--text-muted); font-family: var(--font-mono);", "{msg}" }
            }
        }
    }
}

/// Single session/device row.
#[component]
fn SessionRow(device: fluxchat_core::devices::DeviceInfo, is_current: bool) -> Element {
    let name = device.display_name.clone().unwrap_or_else(|| "Unknown device".into());
    let short_id = if device.device_id.len() > 10 {
        &device.device_id[..10]
    } else {
        &device.device_id
    };

    let (verified_icon_svg, verified_color, verified_label) = if device.is_verified {
        (r#"<svg width="11" height="11" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="3" stroke-linecap="round" stroke-linejoin="round"><polyline points="20 6 9 17 4 12"/></svg>"#, "var(--green)", "verified")
    } else {
        (r#"<svg width="11" height="11" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="3" stroke-linecap="round" stroke-linejoin="round"><line x1="18" y1="6" x2="6" y2="18"/><line x1="6" y1="6" x2="18" y2="18"/></svg>"#, "var(--yellow)", "unverified")
    };

    // Format last seen time
    let last_seen = if let Some(ts) = device.last_seen_ts {
        let secs = ts;
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();
        let diff = now.saturating_sub(secs);
        if diff < 60 { "just now".to_string() }
        else if diff < 3600 { format!("{}m ago", diff / 60) }
        else if diff < 86400 { format!("{}h ago", diff / 3600) }
        else { format!("{}d ago", diff / 86400) }
    } else {
        "unknown".to_string()
    };

    let ip = device.last_seen_ip.clone().unwrap_or_default();
    let border_color = if is_current { "var(--accent)" } else { "var(--border)" };

    rsx! {
        div {
            style: "background: var(--bg-mid); border: 1px solid {border_color}; border-radius: 4px; padding: 10px 12px; font-family: var(--font-mono);",
            // Top row: name + verified badge
            div { style: "display: flex; align-items: center; gap: 8px; margin-bottom: 4px;",
                span { style: "font-size: 13px; font-weight: 600; color: var(--text-bright);",
                    "{name}"
                }
                if is_current {
                    span { style: "font-size: 10px; color: var(--accent); background: rgba(59,130,246,0.1); padding: 1px 6px; border-radius: 3px;",
                        "this device"
                    }
                }
                span { style: "font-size: 11px; color: {verified_color}; display: inline-flex; align-items: center; gap: 4px;",
                    span { style: "display: inline-flex;", dangerous_inner_html: verified_icon_svg }
                    "{verified_label}"
                }
            }
            // Bottom row: details
            div { style: "display: flex; gap: 12px; font-size: 11px; color: var(--text-dim);",
                span { "{short_id}" }
                if !ip.is_empty() {
                    span { "{ip}" }
                }
                span { "last seen {last_seen}" }
            }
        }
    }
}

/// Security settings — key backup, recovery, cross-signing.
#[component]
fn SettingsSecurityPage() -> Element {
    let app_state = use_context::<Arc<AppState>>();
    let mut status_msg = use_signal(|| Option::<String>::None);
    let mut recovery_key_input = use_signal(String::new);
    let mut show_recovery_key = use_signal(|| Option::<String>::None);

    // Fetch backup/recovery state
    let security_state = use_resource({
        let app_state = app_state.clone();
        move || {
            let app_state = app_state.clone();
            async move {
                let client = app_state.get_client().await.ok()?;
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

                Some((backup_exists, backup_enabled, format!("{recovery_state:?}"), is_verified))
            }
        }
    });

    let state = security_state.read().clone().flatten();

    rsx! {
        div { style: "display: flex; flex-direction: column; gap: 16px;",

            // Cross-signing status
            div {
                div { style: "font-size: 11px; font-weight: 700; text-transform: uppercase; color: var(--text-dim); letter-spacing: 0.5px; margin-bottom: 8px; font-family: var(--font-mono);",
                    "cross-signing"
                }
                if let Some((_, _, _, is_verified)) = &state {
                    if *is_verified {
                        div { style: "display: flex; align-items: center; gap: 8px; font-family: var(--font-mono); font-size: 13px;",
                            span { style: "color: var(--green); display: inline-flex; align-items: center; gap: 4px;",
                                span { style: "display: inline-flex;", dangerous_inner_html: r#"<svg width="11" height="11" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="3" stroke-linecap="round" stroke-linejoin="round"><polyline points="20 6 9 17 4 12"/></svg>"# }
                                "verified"
                            }
                            span { style: "color: var(--text-dim);", "This device is cross-signed" }
                        }
                    } else {
                        div { style: "display: flex; align-items: center; gap: 8px; font-family: var(--font-mono); font-size: 13px;",
                            span { style: "color: var(--yellow); display: inline-flex; align-items: center; gap: 4px;",
                                span { style: "display: inline-flex;", dangerous_inner_html: r#"<svg width="11" height="11" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="3" stroke-linecap="round" stroke-linejoin="round"><line x1="18" y1="6" x2="6" y2="18"/><line x1="6" y1="6" x2="18" y2="18"/></svg>"# }
                                "unverified"
                            }
                            span { style: "color: var(--text-dim);", "Verify this device for secure messaging" }
                        }
                    }
                } else {
                    div { style: "font-family: var(--font-mono); font-size: 12px; color: var(--text-dim); animation: pulse 1.5s ease-in-out infinite;",
                        "loading..."
                    }
                }
            }

            // Key backup status
            div {
                div { style: "font-size: 11px; font-weight: 700; text-transform: uppercase; color: var(--text-dim); letter-spacing: 0.5px; margin-bottom: 8px; font-family: var(--font-mono);",
                    "key backup"
                }
                if let Some((backup_exists, backup_enabled, recovery_state, _)) = &state {
                    { let server_color = if *backup_exists { "var(--green)" } else { "var(--text-dim)" };
                      let local_color = if *backup_enabled { "var(--green)" } else { "var(--text-dim)" };
                    rsx! {
                    div { style: "display: flex; flex-direction: column; gap: 6px; font-family: var(--font-mono); font-size: 13px;",
                        div { style: "display: flex; gap: 12px;",
                            span { style: "color: var(--text-muted);", "Server backup:" }
                            span { style: "color: {server_color};",
                                if *backup_exists { "exists" } else { "none" }
                            }
                        }
                        div { style: "display: flex; gap: 12px;",
                            span { style: "color: var(--text-muted);", "Local backup:" }
                            span { style: "color: {local_color};",
                                if *backup_enabled { "enabled" } else { "disabled" }
                            }
                        }
                        div { style: "display: flex; gap: 12px;",
                            span { style: "color: var(--text-muted);", "Recovery:" }
                            span { style: "color: var(--text-normal);", "{recovery_state}" }
                        }
                    }

                    // Actions
                    div { style: "display: flex; gap: 8px; margin-top: 8px; flex-wrap: wrap;",
                        if !*backup_enabled {
                            span {
                                style: "font-family: var(--font-mono); font-size: 12px; color: var(--green); cursor: pointer; padding: 4px 12px; border-radius: 4px; border: 1px solid var(--green);",
                                onclick: {
                                    let app_state = app_state.clone();
                                    move |_| {
                                        let app_state = app_state.clone();
                                        status_msg.set(Some("enabling backup...".into()));
                                        spawn(async move {
                                            let client = match app_state.get_client().await { Ok(c) => c, Err(_) => return };
                                            match client.encryption().recovery().enable().await {
                                                Ok(key) => {
                                                    show_recovery_key.set(Some(key));
                                                    status_msg.set(Some("backup enabled! save your recovery key".into()));
                                                }
                                                Err(e) => {
                                                    // Try just enabling backup without full recovery
                                                    match client.encryption().recovery().enable_backup().await {
                                                        Ok(_) => status_msg.set(Some("backup enabled".into())),
                                                        Err(_) => status_msg.set(Some(format!("failed: {e}"))),
                                                    }
                                                }
                                            }
                                        });
                                    }
                                },
                                "enable backup"
                            }
                        }
                    }
                } } }
            }

            // Recovery key display (shown after enabling)
            if let Some(ref key) = *show_recovery_key.read() {
                div {
                    div { style: "font-size: 11px; font-weight: 700; text-transform: uppercase; color: var(--yellow); letter-spacing: 0.5px; margin-bottom: 8px; font-family: var(--font-mono);",
                        "recovery key — save this!"
                    }
                    div { style: "background: var(--bg-deep); border: 1px solid var(--yellow); border-radius: 4px; padding: 12px; font-family: var(--font-mono); font-size: 13px; color: var(--text-bright); word-break: break-all; -webkit-user-select: text; user-select: text;",
                        "{key}"
                    }
                    div { style: "font-size: 11px; color: var(--text-dim); font-family: var(--font-mono); margin-top: 4px;",
                        "Store this key safely. You'll need it to restore encrypted messages on new devices."
                    }
                }
            }

            // Recover from backup
            div {
                div { style: "font-size: 11px; font-weight: 700; text-transform: uppercase; color: var(--text-dim); letter-spacing: 0.5px; margin-bottom: 8px; font-family: var(--font-mono);",
                    "recover from backup"
                }
                div { style: "display: flex; gap: 6px;",
                    input {
                        style: "flex: 1; background: var(--bg-deep); border: 1px solid var(--border); border-radius: 4px; padding: 6px 8px; color: var(--text-bright); font-family: var(--font-mono); font-size: 13px; outline: none;",
                        placeholder: "Recovery key or passphrase",
                        value: "{recovery_key_input}",
                        oninput: move |e| recovery_key_input.set(e.value()),
                    }
                    span {
                        style: "font-family: var(--font-mono); font-size: 12px; color: var(--accent-muted); cursor: pointer; padding: 6px 12px; border-radius: 4px; border: 1px solid var(--border); flex-shrink: 0;",
                        onclick: {
                            let app_state = app_state.clone();
                            move |_| {
                                let key = recovery_key_input().trim().to_string();
                                if key.is_empty() { return; }
                                let app_state = app_state.clone();
                                status_msg.set(Some("recovering...".into()));
                                spawn(async move {
                                    let client = match app_state.get_client().await { Ok(c) => c, Err(_) => return };
                                    match client.encryption().recovery().recover(&key).await {
                                        Ok(_) => {
                                            status_msg.set(Some("recovery successful! keys restored.".into()));
                                            recovery_key_input.set(String::new());
                                        }
                                        Err(e) => status_msg.set(Some(format!("recovery failed: {e}"))),
                                    }
                                });
                            }
                        },
                        "recover"
                    }
                }
            }

            // Status
            if let Some(ref msg) = *status_msg.read() {
                div { style: "font-size: 12px; color: var(--text-muted); font-family: var(--font-mono);", "{msg}" }
            }
        }
    }
}

#[component]
fn ProfileModal(user_id: String) -> Element {
    let mut ui = use_context::<UiState>();
    let app_state = use_context::<Arc<AppState>>();

    // Look up member info — prefer current room, fall back to any room
    let member = {
        let members = ui.members_by_room.read();
        let current_room = ui.selected_room_id.peek().clone();
        current_room.as_ref()
            .and_then(|rid| members.get(rid))
            .and_then(|m| m.iter().find(|m| m.user_id == user_id))
            .cloned()
            .or_else(|| members.values().flat_map(|m| m.iter()).find(|m| m.user_id == user_id).cloned())
    };

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
                        if let Some(ref m) = member {
                            div { style: "display: flex; justify-content: space-between;",
                                span { style: "color: var(--text-muted);", "Power Level" }
                                span { style: "color: var(--text-normal); font-family: var(--font-mono);", "{m.power_level}" }
                            }
                        }
                        div { style: "display: flex; justify-content: space-between;",
                            span { style: "color: var(--text-muted);", "Status" }
                            span { style: "color: {presence_color};", "{presence}" }
                        }
                        if let Some(ref m) = member {
                            if let Some(ref status) = m.status_msg {
                                div { style: "display: flex; justify-content: space-between;",
                                    span { style: "color: var(--text-muted);", "Status Message" }
                                    span { style: "color: var(--text-normal); font-family: var(--font-mono); font-size: 12px; max-width: 180px; overflow: hidden; text-overflow: ellipsis; white-space: nowrap;",
                                        "{status}"
                                    }
                                }
                            }
                        }
                        div { style: "display: flex; justify-content: space-between;",
                            span { style: "color: var(--text-muted);", "Server" }
                            span { style: "color: var(--text-normal); font-family: var(--font-mono);", "{server}" }
                        }
                    }

                    // Power level actions (promote/demote)
                    { let is_self = ui.user_id.peek().as_str() == user_id;
                      let my_id = ui.user_id.peek().clone();
                      let target_pl = member.as_ref().map(|m| m.power_level).unwrap_or(0);
                      let my_pl = ui.members_by_room.peek().values()
                          .flat_map(|members| members.iter())
                          .find(|m| m.user_id == my_id)
                          .map(|m| m.power_level).unwrap_or(0);
                      let can_change = !is_self && my_pl > target_pl;
                      if can_change {
                          let uid_op = user_id.clone();
                          let app_op = app_state.clone();
                          rsx! {
                              div { style: "margin-top: 8px; padding-top: 8px; border-top: 1px solid var(--border); display: flex; gap: 12px; flex-wrap: wrap;",
                                  if target_pl < 50 {
                                      { let uid = uid_op.clone(); let app = app_op.clone();
                                      rsx! {
                                          span { style: "font-family: var(--font-mono); font-size: 12px; color: var(--accent-muted); cursor: pointer;",
                                              onclick: move |_| {
                                                  let uid = uid.clone(); let app = app.clone();
                                                  spawn(async move {
                                                      let client = match app.get_client().await { Ok(c) => c, Err(_) => return };
                                                      let _ = fluxchat_core::rooms::set_user_power_level(&client, &consume_context::<UiState>().selected_room_id.peek().clone().unwrap_or_default(), &uid, 50).await;
                                                  });
                                              },
                                              "promote to mod"
                                          }
                                      } }
                                  }
                                  if target_pl < 100 && my_pl >= 100 {
                                      { let uid = uid_op.clone(); let app = app_op.clone();
                                      rsx! {
                                          span { style: "font-family: var(--font-mono); font-size: 12px; color: var(--yellow); cursor: pointer;",
                                              onclick: move |_| {
                                                  let uid = uid.clone(); let app = app.clone();
                                                  spawn(async move {
                                                      let client = match app.get_client().await { Ok(c) => c, Err(_) => return };
                                                      let _ = fluxchat_core::rooms::set_user_power_level(&client, &consume_context::<UiState>().selected_room_id.peek().clone().unwrap_or_default(), &uid, 100).await;
                                                  });
                                              },
                                              "promote to admin"
                                          }
                                      } }
                                  }
                                  if target_pl > 0 {
                                      { let uid = uid_op.clone(); let app = app_op.clone();
                                      rsx! {
                                          span { style: "font-family: var(--font-mono); font-size: 12px; color: var(--red); cursor: pointer;",
                                              onclick: move |_| {
                                                  let uid = uid.clone(); let app = app.clone();
                                                  spawn(async move {
                                                      let client = match app.get_client().await { Ok(c) => c, Err(_) => return };
                                                      let _ = fluxchat_core::rooms::set_user_power_level(&client, &consume_context::<UiState>().selected_room_id.peek().clone().unwrap_or_default(), &uid, 0).await;
                                                  });
                                              },
                                              "demote to member"
                                          }
                                      } }
                                  }
                              }
                          }
                      } else { rsx! {} }
                    }

                    // Ignore/unignore button (don't show for own user)
                    { let is_self = ui.user_id.peek().as_str() == user_id;
                      let is_ignored = ui.ignored_users.read().contains(&user_id);
                      if !is_self {
                        let uid = user_id.clone();
                        if is_ignored {
                            rsx! {
                                div { style: "margin-top: 8px; padding-top: 8px; border-top: 1px solid var(--border);",
                                    span {
                                        style: "font-family: var(--font-mono); font-size: 12px; color: var(--accent-muted); cursor: pointer;",
                                        onclick: {
                                            let uid = uid.clone();
                                            let app_state = app_state.clone();
                                            move |_| {
                                                let uid = uid.clone();
                                                let app_state = app_state.clone();
                                                consume_context::<UiState>().ignored_users.write().remove(&uid);
                                                spawn(async move {
                                                    let client = match app_state.get_client().await { Ok(c) => c, Err(_) => return };
                                                    if let Ok(parsed) = matrix_sdk::ruma::UserId::parse(&uid) {
                                                        let _ = client.account().unignore_user(&parsed).await;
                                                    }
                                                });
                                            }
                                        },
                                        "unignore user"
                                    }
                                }
                            }
                        } else {
                            rsx! {
                                div { style: "margin-top: 8px; padding-top: 8px; border-top: 1px solid var(--border);",
                                    span {
                                        style: "font-family: var(--font-mono); font-size: 12px; color: var(--red); cursor: pointer;",
                                        onclick: {
                                            let uid = uid.clone();
                                            let app_state = app_state.clone();
                                            move |_| {
                                                let uid = uid.clone();
                                                let app_state = app_state.clone();
                                                consume_context::<UiState>().ignored_users.write().insert(uid.clone());
                                                spawn(async move {
                                                    let client = match app_state.get_client().await { Ok(c) => c, Err(_) => return };
                                                    if let Ok(parsed) = matrix_sdk::ruma::UserId::parse(&uid) {
                                                        let _ = client.account().ignore_user(&parsed).await;
                                                    }
                                                });
                                            }
                                        },
                                        "ignore user"
                                    }
                                }
                            }
                        }
                      } else { rsx! {} }
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

    let mut name = use_signal(String::new);
    let mut topic = use_signal(String::new);
    let mut is_public = use_signal(|| false);
    let mut encrypted = use_signal(|| true);
    let mut join_input = use_signal(String::new);
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
    let mut invite_user = use_signal(String::new);
    let mut status_msg = use_signal(|| Option::<String>::None);
    let mut new_alias = use_signal(String::new);
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
                    // Aliases
                    div {
                        div { style: "font-size: 11px; font-weight: 700; text-transform: uppercase; color: var(--text-dim); letter-spacing: 0.5px; margin-bottom: 4px; font-family: var(--font-mono);", "aliases" }
                        { let rid_alias = room_id.clone();
                          let app_alias = app_state.clone();
                          let aliases = use_resource({
                              let app = app_alias.clone();
                              let rid = rid_alias.clone();
                              move || {
                                  let app = app.clone();
                                  let rid = rid.clone();
                                  async move {
                                      let client = app.get_client().await.ok()?;
                                      fluxchat_core::rooms::get_room_aliases(&client, &rid).await.ok()
                                  }
                              }
                          });
                          let alias_list = aliases.read().clone().flatten().unwrap_or_default();
                        rsx! {
                            if alias_list.is_empty() {
                                div { style: "font-size: 12px; color: var(--text-dim); font-family: var(--font-mono);", "no aliases" }
                            }
                            for alias in alias_list.iter() {
                                div { style: "display: flex; justify-content: space-between; align-items: center; padding: 2px 0;",
                                    span { style: "font-size: 12px; color: var(--accent); font-family: var(--font-mono);", "{alias}" }
                                    span { style: "font-size: 11px; color: var(--red); cursor: pointer; font-family: var(--font-mono);",
                                        onclick: { let app = app_alias.clone(); let a = alias.clone();
                                            move |_| { let app = app.clone(); let a = a.clone();
                                                spawn(async move { let client = match app.get_client().await { Ok(c) => c, Err(_) => return };
                                                    let _ = fluxchat_core::rooms::remove_room_alias(&client, &a).await;
                                                    status_msg.set(Some(format!("removed {a}")));
                                                });
                                            }
                                        }, "remove"
                                    }
                                }
                            }
                            div { style: "display: flex; gap: 6px; margin-top: 4px;",
                                input { style: "flex: 1; background: var(--bg-deep); border: 1px solid var(--border); border-radius: 4px; padding: 6px 8px; color: var(--text-bright); font-family: var(--font-mono); font-size: 13px; outline: none;",
                                    placeholder: "#alias:server.org", value: "{new_alias}", oninput: move |e| new_alias.set(e.value()),
                                }
                                span { style: "font-family: var(--font-mono); font-size: 12px; color: var(--accent); cursor: pointer; padding: 6px 8px; border-radius: 4px; border: 1px solid var(--border); flex-shrink: 0;",
                                    onclick: { let app = app_alias.clone(); let rid = rid_alias.clone();
                                        move |_| { let a = new_alias().trim().to_string(); if a.is_empty() { return; }
                                            let app = app.clone(); let rid = rid.clone();
                                            spawn(async move { let client = match app.get_client().await { Ok(c) => c, Err(_) => return };
                                                match fluxchat_core::rooms::add_room_alias(&client, &rid, &a).await {
                                                    Ok(_) => { new_alias.set(String::new()); status_msg.set(Some(format!("added {a}"))); }
                                                    Err(e) => { status_msg.set(Some(format!("{e}"))); }
                                                }
                                            });
                                        }
                                    }, "add"
                                }
                            }
                        } }
                    }
                    div { style: "display: flex; gap: 16px; font-size: 12px; font-family: var(--font-mono); color: var(--text-muted);",
                        if room.is_encrypted { span { style: "color: var(--green);", "encrypted" } } else { span { "unencrypted" } }
                        if room.is_direct { span { "direct message" } }
                    }
                    // Notifications
                    div {
                        div { style: "font-size: 11px; font-weight: 700; text-transform: uppercase; color: var(--text-dim); letter-spacing: 0.5px; margin-bottom: 4px; font-family: var(--font-mono);", "notifications" }
                        {
                            let rid_notif = room_id.clone();
                            let app_notif = app_state.clone();
                            let notif_mode = use_resource({
                                let app = app_notif.clone();
                                let rid = rid_notif.clone();
                                move || {
                                    let app = app.clone();
                                    let rid = rid.clone();
                                    async move {
                                        let client = app.get_client().await.ok()?;
                                        Some(fluxchat_core::rooms::get_notification_mode(&client, &rid).await.unwrap_or("all".into()))
                                    }
                                }
                            });
                            let current = notif_mode.read().clone().flatten().unwrap_or("all".into());
                            rsx! {
                                div { style: "display: flex; gap: 8px; align-items: center;",
                                    span {
                                        style: if current == "all" { "font-family: var(--font-mono); font-size: 12px; color: var(--green); cursor: pointer; padding: 4px 10px; border-radius: 4px; border: 1px solid var(--green); background: rgba(34,197,94,0.1);" } else { "font-family: var(--font-mono); font-size: 12px; color: var(--text-muted); cursor: pointer; padding: 4px 10px; border-radius: 4px; border: 1px solid var(--border);" },
                                        onclick: { let app = app_notif.clone(); let rid = rid_notif.clone();
                                            move |_| { let app = app.clone(); let rid = rid.clone(); let mut notif_mode = notif_mode;
                                                spawn(async move { let client = match app.get_client().await { Ok(c) => c, Err(_) => return };
                                                    let _ = fluxchat_core::rooms::set_notification_mode(&client, &rid, "all").await;
                                                    notif_mode.restart();
                                                });
                                            }
                                        }, "all"
                                    }
                                    span {
                                        style: if current == "mentions" { "font-family: var(--font-mono); font-size: 12px; color: var(--accent); cursor: pointer; padding: 4px 10px; border-radius: 4px; border: 1px solid var(--accent); background: rgba(99,102,241,0.1);" } else { "font-family: var(--font-mono); font-size: 12px; color: var(--text-muted); cursor: pointer; padding: 4px 10px; border-radius: 4px; border: 1px solid var(--border);" },
                                        onclick: { let app = app_notif.clone(); let rid = rid_notif.clone();
                                            move |_| { let app = app.clone(); let rid = rid.clone(); let mut notif_mode = notif_mode;
                                                spawn(async move { let client = match app.get_client().await { Ok(c) => c, Err(_) => return };
                                                    let _ = fluxchat_core::rooms::set_notification_mode(&client, &rid, "mentions").await;
                                                    notif_mode.restart();
                                                });
                                            }
                                        }, "mentions"
                                    }
                                    span {
                                        style: if current == "mute" { "font-family: var(--font-mono); font-size: 12px; color: var(--red); cursor: pointer; padding: 4px 10px; border-radius: 4px; border: 1px solid var(--red); background: rgba(239,68,68,0.1);" } else { "font-family: var(--font-mono); font-size: 12px; color: var(--text-muted); cursor: pointer; padding: 4px 10px; border-radius: 4px; border: 1px solid var(--border);" },
                                        onclick: { let app = app_notif.clone(); let rid = rid_notif.clone();
                                            move |_| { let app = app.clone(); let rid = rid.clone(); let mut notif_mode = notif_mode;
                                                spawn(async move { let client = match app.get_client().await { Ok(c) => c, Err(_) => return };
                                                    let _ = fluxchat_core::rooms::set_notification_mode(&client, &rid, "mute").await;
                                                    notif_mode.restart();
                                                });
                                            }
                                        }, "mute"
                                    }
                                }
                            }
                        }
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

/// Forward message modal — pick a room to forward to.
#[component]
fn ForwardModal() -> Element {
    let mut ui = use_context::<UiState>();
    let app_state = use_context::<Arc<AppState>>();
    let rooms = ui.rooms.read().clone();
    let mut search = use_signal(String::new);

    let (body, formatted, sender) = match ui.forward_event.read().clone() {
        Some((b, f, s)) => (b, if f.is_empty() { None } else { Some(f) }, s),
        None => return rsx! {},
    };

    let query = search().to_lowercase();
    let filtered: Vec<_> = rooms.iter()
        .filter(|r| !r.is_space && (query.is_empty() || r.name.to_lowercase().contains(&query)))
        .take(20)
        .collect();

    rsx! {
        div {
            style: "position: fixed; inset: 0; background: rgba(0,0,0,0.5); z-index: 1000; display: flex; align-items: center; justify-content: center;",
            onclick: move |_| { ui.forward_event.set(None); },
            div {
                style: "background: var(--bg-dark); border: 1px solid var(--border); border-radius: 8px; min-width: 360px; max-width: 420px; max-height: 500px; display: flex; flex-direction: column;",
                onclick: move |e| { e.stop_propagation(); },
                div { style: "padding: 16px 20px; border-bottom: 1px solid var(--border); display: flex; justify-content: space-between; align-items: center;",
                    span { style: "font-size: 14px; font-weight: 600; color: var(--text-bright); font-family: var(--font-mono);", "forward message" }
                    span { style: "cursor: pointer; color: var(--text-dim); font-size: 18px; padding: 4px;",
                        onclick: move |_| { ui.forward_event.set(None); }, "\u{00d7}"
                    }
                }
                div { style: "padding: 12px 20px;",
                    if let Some(ref sn) = sender {
                        div { style: "font-size: 11px; color: var(--text-dim); font-family: var(--font-mono); margin-bottom: 4px;", "from {sn}:" }
                    }
                    div { style: "font-size: 12px; color: var(--text-muted); font-family: var(--font-mono); overflow: hidden; text-overflow: ellipsis; white-space: nowrap; margin-bottom: 8px;",
                        "{body}"
                    }
                    input {
                        style: "width: 100%; background: var(--bg-deep); border: 1px solid var(--border); border-radius: 4px; padding: 6px 8px; color: var(--text-bright); font-family: var(--font-mono); font-size: 13px; outline: none;",
                        placeholder: "search rooms...",
                        value: "{search}",
                        oninput: move |e| search.set(e.value()),
                    }
                }
                div { style: "flex: 1; overflow-y: auto; padding: 0 20px 16px;",
                    for room in filtered.iter() {
                        { let rid = room.room_id.clone();
                          let rname = room.name.clone();
                          let body = body.clone();
                          let formatted = formatted.clone();
                          let app = app_state.clone();
                        rsx! {
                            div {
                                style: "padding: 6px 8px; border-radius: 4px; cursor: pointer; font-size: 13px; font-family: var(--font-mono); color: var(--text-normal); display: flex; align-items: center; gap: 6px;",
                                class: "cmd-item",
                                onclick: move |_| {
                                    let rid = rid.clone();
                                    let body = body.clone();
                                    let formatted = formatted.clone();
                                    let app = app.clone();
                                    consume_context::<UiState>().forward_event.set(None);
                                    spawn(async move {
                                        let client = match app.get_client().await { Ok(c) => c, Err(_) => return };
                                        let _ = fluxchat_core::messages::forward_message(&client, &rid, &body, formatted.as_deref()).await;
                                    });
                                },
                                span { style: "font-family: var(--font-mono); opacity: 0.4; font-size: 12px;", "#" }
                                "{rname}"
                            }
                        } }
                    }
                    if filtered.is_empty() {
                        div { style: "font-size: 12px; color: var(--text-dim); font-family: var(--font-mono); padding: 8px;", "no rooms found" }
                    }
                }
            }
        }
    }
}
