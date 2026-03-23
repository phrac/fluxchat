use dioxus::prelude::*;
use std::sync::Arc;
use fluxchat_core::AppState;

use crate::state::UiState;
use crate::components::sidebar::space_sidebar::SpaceSidebar;
use crate::components::sidebar::room_list::RoomList;
use crate::components::chat::message_area::{MessageArea, nick_color_class};
use crate::components::members::member_list::MemberList;

#[component]
pub fn MainLayout() -> Element {
    let mut ui = use_context::<UiState>();

    let user_id = ui.user_id.read().clone();
    let server = user_id.split(':').nth(1).unwrap_or("matrix.org");
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

            // Member list
            if selected_room.is_some() {
                MemberList {}
            }
        }

        // Profile modal
        { let profile_uid = ui.profile_user_id.read().clone();
          if let Some(uid) = profile_uid {
            rsx! { ProfileModal { user_id: uid } }
          } else { rsx! {} }
        }

        // Status bar — IRC style
        div { class: "status-bar",
            span { style: "font-family: var(--font-mono); color: var(--green);", "{user_id}" }
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
