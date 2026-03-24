use dioxus::prelude::*;
use std::sync::Arc;
use fluxchat_core::AppState;
use crate::state::UiState;
use crate::components::chat::message_area::nick_color_class;

const BATCH_SIZE: usize = 30;

#[component]
pub fn MemberList() -> Element {
    let mut ui = use_context::<UiState>();
    let app_state = use_context::<Arc<AppState>>();
    let selected = ui.selected_room_id.read().clone();
    let mut visible_count = use_signal(|| BATCH_SIZE);

    // Reset visible count when room changes
    use_effect(move || {
        let _ = ui.selected_room_id.read();
        visible_count.set(BATCH_SIZE);
    });

    // Periodically refresh members for presence updates (every 30s)
    {
        let app_state = app_state.clone();
        use_future(move || {
            let app_state = app_state.clone();
            async move {
                loop {
                    tokio::time::sleep(std::time::Duration::from_secs(30)).await;
                    let ui = consume_context::<UiState>();
                    let room_id = ui.selected_room_id.peek().clone();
                    if let Some(rid) = room_id {
                        if let Ok(client) = app_state.get_client().await {
                            if let Ok(members) = fluxchat_core::messages::get_room_members(&client, &rid).await {
                                let mut ui = consume_context::<UiState>();
                                ui.members_by_room.write().insert(rid, members);
                            }
                        }
                    }
                }
            }
        });
    }

    let members = selected.as_ref()
        .and_then(|id| ui.members_by_room.read().get(id).cloned())
        .unwrap_or_default();

    let total = members.len();
    let show = *visible_count.read();

    // Only resolve avatars for visible members — sync cache lookup
    let visible_members: Vec<_> = members.into_iter().take(show).collect();
    let (avatar_map, uncached_mxcs): (std::collections::HashMap<String, String>, Vec<String>) = {
        let cache = app_state.media_cache.lock().unwrap();
        let mut amap = std::collections::HashMap::new();
        let mut uncached = Vec::new();
        for m in &visible_members {
            if let Some(mxc) = &m.avatar_url {
                if let Some(data_uri) = cache.get(mxc) {
                    amap.insert(m.user_id.clone(), data_uri.clone());
                } else {
                    uncached.push(mxc.clone());
                }
            }
        }
        // Dedup
        uncached.sort();
        uncached.dedup();
        (amap, uncached)
    };

    // Background-resolve uncached avatars — they'll appear on next render
    if !uncached_mxcs.is_empty() {
        let app_state = app_state.clone();
        spawn(async move {
            let client = match app_state.get_client().await { Ok(c) => c, Err(_) => return };
            for mxc in uncached_mxcs {
                let _ = fluxchat_core::media::get_media_url(&client, &app_state.db, &app_state.media_cache, &mxc).await;
            }
        });
    }

    let admins: Vec<_> = visible_members.iter().filter(|m| m.role == "Admin").cloned().collect();
    let mods: Vec<_> = visible_members.iter().filter(|m| m.role == "Moderator").cloned().collect();
    let regular: Vec<_> = visible_members.iter().filter(|m| m.role == "Member").cloned().collect();

    rsx! {
        div { style: "width: 180px; flex-shrink: 0; background: var(--bg-dark); border-left: 1px solid var(--border); display: flex; flex-direction: column;",
            div { style: "height: 40px; display: flex; align-items: center; justify-content: space-between; padding: 0 12px; border-bottom: 1px solid var(--border); flex-shrink: 0;",
                span { style: "font-size: 11px; font-weight: 700; text-transform: uppercase; color: var(--text-dim); letter-spacing: 0.5px;",
                    "Users — {total}"
                }
                span { style: "cursor: pointer; color: var(--text-dim); display: inline-flex; padding: 4px; border-radius: 3px;",
                    class: "room-action",
                    title: "Invite user",
                    onclick: move |_| { ui.room_settings_open.set(true); },
                    dangerous_inner_html: r#"<svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2.5" stroke-linecap="round" stroke-linejoin="round"><path d="M16 21v-2a4 4 0 0 0-4-4H6a4 4 0 0 0-4 4v2"/><circle cx="9" cy="7" r="4"/><line x1="19" y1="8" x2="19" y2="14"/><line x1="22" y1="11" x2="16" y2="11"/></svg>"#,
                }
            }
            div {
                style: "flex: 1; overflow-y: auto; padding: 8px 6px;",
                onscroll: move |_| {
                    if show < total {
                        visible_count.set((show + BATCH_SIZE).min(total));
                    }
                },
                if !admins.is_empty() {
                    div { style: "font-size: 10px; color: var(--text-dim); font-weight: 700; text-transform: uppercase; padding: 4px 8px; letter-spacing: 0.5px;",
                        "@ Admins — {admins.len()}"
                    }
                    for m in admins.iter() {
                        MemberRow { member: m.clone(), avatar_src: avatar_map.get(&m.user_id).cloned() }
                    }
                }
                if !mods.is_empty() {
                    div { style: "font-size: 10px; color: var(--text-dim); font-weight: 700; text-transform: uppercase; padding: 8px 8px 4px; letter-spacing: 0.5px;",
                        "% Mods — {mods.len()}"
                    }
                    for m in mods.iter() {
                        MemberRow { member: m.clone(), avatar_src: avatar_map.get(&m.user_id).cloned() }
                    }
                }
                if !regular.is_empty() {
                    div { style: "font-size: 10px; color: var(--text-dim); font-weight: 700; text-transform: uppercase; padding: 8px 8px 4px; letter-spacing: 0.5px;",
                        "Members — {regular.len()}"
                    }
                    for m in regular.iter() {
                        MemberRow { member: m.clone(), avatar_src: avatar_map.get(&m.user_id).cloned() }
                    }
                }
            }
        }
    }
}

#[component]
fn MemberRow(member: fluxchat_core::MemberInfo, avatar_src: Option<String>) -> Element {
    let mut ui = use_context::<UiState>();
    let nick_class = nick_color_class(&member.user_id);
    let initial = member.display_name.chars().next().unwrap_or('?').to_uppercase().to_string();
    let presence_color = match member.presence.as_str() {
        "online" => "var(--green)", "idle" => "var(--yellow)", _ => "var(--text-dim)",
    };
    let uid = member.user_id.clone();

    rsx! {
        div {
            style: "display: flex; align-items: center; gap: 6px; padding: 2px 8px; border-radius: 3px; cursor: pointer;",
            onclick: move |_| { ui.profile_user_id.set(Some(uid.clone())); },
            div { style: "position: relative; flex-shrink: 0; width: 20px; height: 20px;",
                if let Some(ref src) = avatar_src {
                    img { src: "{src}", style: "width: 20px; height: 20px; border-radius: 50%; object-fit: cover;" }
                } else {
                    div { style: "width: 20px; height: 20px; border-radius: 50%; background: var(--bg-light); display: flex; align-items: center; justify-content: center; font-size: 9px; font-weight: 600; color: var(--text-muted);",
                        "{initial}"
                    }
                }
                div { style: "position: absolute; bottom: -1px; right: -1px; width: 7px; height: 7px; border-radius: 50%; background: {presence_color}; border: 1.5px solid var(--bg-dark);" }
            }
            div { style: "min-width: 0; flex: 1; overflow: hidden;",
                span { class: "{nick_class}", style: "font-size: 13px; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; display: block;",
                    "{member.display_name}"
                }
                if let Some(ref status) = member.status_msg {
                    span { style: "font-size: 10px; color: var(--text-dim); font-family: var(--font-mono); overflow: hidden; text-overflow: ellipsis; white-space: nowrap; display: block;",
                        "{status}"
                    }
                }
            }
            if member.power_level >= 100 {
                span { style: "width: 8px; height: 8px; border-radius: 50%; background: var(--yellow); flex-shrink: 0; opacity: 0.8;", title: "Admin ({member.power_level})" }
            } else if member.power_level >= 50 {
                span { style: "width: 8px; height: 8px; border-radius: 50%; background: var(--accent-muted); flex-shrink: 0; opacity: 0.8;", title: "Mod ({member.power_level})" }
            }
        }
    }
}
