use dioxus::prelude::*;
use std::sync::Arc;
use fluxchat_core::AppState;
use crate::state::UiState;
use crate::components::chat::message_area::nick_color_class;

const BATCH_SIZE: usize = 30;

#[component]
pub fn MemberList() -> Element {
    let ui = use_context::<UiState>();
    let app_state = use_context::<Arc<AppState>>();
    let selected = ui.selected_room_id.read().clone();
    let mut visible_count = use_signal(|| BATCH_SIZE);

    // Reset visible count when room changes
    use_effect(move || {
        let _ = ui.selected_room_id.read();
        visible_count.set(BATCH_SIZE);
    });

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
            div { style: "height: 40px; display: flex; align-items: center; padding: 0 12px; font-size: 11px; font-weight: 700; text-transform: uppercase; color: var(--text-dim); border-bottom: 1px solid var(--border); flex-shrink: 0; letter-spacing: 0.5px;",
                "Users — {total}"
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
            span { class: "{nick_class}", style: "font-size: 13px; overflow: hidden; text-overflow: ellipsis; white-space: nowrap;",
                "{member.display_name}"
            }
        }
    }
}
