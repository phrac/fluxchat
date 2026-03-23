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

    let app_state = use_context::<Arc<AppState>>();
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
    let app_state = use_context::<Arc<AppState>>();
    let selected = ui.selected_room_id.read().clone();

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

    let status = use_resource({
        let app_state = app_state.clone();
        move || {
            let app_state = app_state.clone();
            async move {
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
        let tooltip = format!("{} ({}/{} devices verified)", device_id, verified, total);
        rsx! {
            span {
                style: "color: {color}; font-family: var(--font-mono);",
                title: "{tooltip}",
                "{icon} {label}"
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
    let ui = use_context::<UiState>();
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

                    // Account section
                    div {
                        div { style: "font-size: 11px; font-weight: 700; text-transform: uppercase; color: var(--text-dim); letter-spacing: 0.5px; margin-bottom: 8px; font-family: var(--font-mono);",
                            "account"
                        }
                        div { style: "display: flex; flex-direction: column; gap: 6px; font-size: 13px;",
                            div { style: "display: flex; justify-content: space-between;",
                                span { style: "color: var(--text-muted);", "User ID" }
                                span { style: "color: var(--text-normal); font-family: var(--font-mono);", "{user_id}" }
                            }
                            div { style: "display: flex; justify-content: space-between;",
                                span { style: "color: var(--text-muted);", "Display Name" }
                                span { style: "color: var(--text-normal);", "{display_name}" }
                            }
                        }
                    }

                    // Appearance section
                    div {
                        div { style: "font-size: 11px; font-weight: 700; text-transform: uppercase; color: var(--text-dim); letter-spacing: 0.5px; margin-bottom: 8px; font-family: var(--font-mono);",
                            "appearance"
                        }
                        div { style: "font-size: 13px; color: var(--text-muted);",
                            "Theme and font settings coming soon."
                        }
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
