use dioxus::prelude::*;
use std::sync::Arc;
use fluxchat_core::AppState;
use crate::state::UiState;
use crate::components::chat::message_area::nick_color_class;

#[component]
pub fn MemberList() -> Element {
    let ui = use_context::<UiState>();
    let selected = ui.selected_room_id.read().clone();
    let members = selected.as_ref()
        .and_then(|id| ui.members_by_room.read().get(id).cloned())
        .unwrap_or_default();

    let admins: Vec<_> = members.iter().filter(|m| m.role == "Admin").cloned().collect();
    let mods: Vec<_> = members.iter().filter(|m| m.role == "Moderator").cloned().collect();
    let regular: Vec<_> = members.iter().filter(|m| m.role == "Member").cloned().collect();

    rsx! {
        div { style: "width: 180px; flex-shrink: 0; background: var(--bg-dark); border-left: 1px solid var(--border); display: flex; flex-direction: column;",
            div { style: "height: 40px; display: flex; align-items: center; padding: 0 12px; font-size: 11px; font-weight: 700; text-transform: uppercase; color: var(--text-dim); border-bottom: 1px solid var(--border); flex-shrink: 0; letter-spacing: 0.5px;",
                "Users — {members.len()}"
            }
            div { style: "flex: 1; overflow-y: auto; padding: 8px 6px;",
                if !admins.is_empty() {
                    div { style: "font-size: 10px; color: var(--text-dim); font-weight: 700; text-transform: uppercase; padding: 4px 8px; letter-spacing: 0.5px;",
                        "@ Admins — {admins.len()}"
                    }
                    for m in admins.iter() {
                        MemberEntry { name: m.display_name.clone(), user_id: m.user_id.clone(), presence: m.presence.clone(), avatar_mxc: m.avatar_url.clone() }
                    }
                }
                if !mods.is_empty() {
                    div { style: "font-size: 10px; color: var(--text-dim); font-weight: 700; text-transform: uppercase; padding: 8px 8px 4px; letter-spacing: 0.5px;",
                        "% Mods — {mods.len()}"
                    }
                    for m in mods.iter() {
                        MemberEntry { name: m.display_name.clone(), user_id: m.user_id.clone(), presence: m.presence.clone(), avatar_mxc: m.avatar_url.clone() }
                    }
                }
                if !regular.is_empty() {
                    div { style: "font-size: 10px; color: var(--text-dim); font-weight: 700; text-transform: uppercase; padding: 8px 8px 4px; letter-spacing: 0.5px;",
                        "Members — {regular.len()}"
                    }
                    for m in regular.iter() {
                        MemberEntry { name: m.display_name.clone(), user_id: m.user_id.clone(), presence: m.presence.clone(), avatar_mxc: m.avatar_url.clone() }
                    }
                }
            }
        }
    }
}

#[component]
fn MemberEntry(name: String, user_id: String, presence: String, avatar_mxc: Option<String>) -> Element {
    let core = use_context::<Arc<AppState>>();
    let nick_class = nick_color_class(&user_id);
    let presence_color = match presence.as_str() {
        "online" => "var(--green)",
        "idle" => "var(--yellow)",
        _ => "var(--text-dim)",
    };

    // Resolve avatar
    let avatar_src = use_resource({
        let mxc = avatar_mxc.clone();
        let core = core.clone();
        move || {
            let mxc = mxc.clone();
            let core = core.clone();
            async move {
                if let Some(mxc) = mxc {
                    let client = core.get_client().await.ok()?;
                    fluxchat_core::media::get_media_url(&client, &core.db, &core.media_cache, &mxc).await.ok()
                } else { None }
            }
        }
    });
    let has_avatar = avatar_src.read().as_ref().and_then(|v| v.as_ref()).is_some();

    let initial = name.chars().next().unwrap_or('?').to_uppercase().to_string();

    rsx! {
        div { style: "display: flex; align-items: center; gap: 6px; padding: 2px 8px; border-radius: 3px; cursor: pointer;",
            // Avatar with presence dot
            div { style: "position: relative; flex-shrink: 0; width: 20px; height: 20px;",
                if has_avatar {
                    img {
                        src: "{avatar_src.read().as_ref().and_then(|v| v.as_ref()).unwrap()}",
                        style: "width: 20px; height: 20px; border-radius: 50%; object-fit: cover;",
                    }
                } else {
                    div { style: "width: 20px; height: 20px; border-radius: 50%; background: var(--bg-light); display: flex; align-items: center; justify-content: center; font-size: 9px; font-weight: 600; color: var(--text-muted); line-height: 1;",
                        "{initial}"
                    }
                }
                // Presence dot
                div { style: "position: absolute; bottom: -1px; right: -1px; width: 7px; height: 7px; border-radius: 50%; background: {presence_color}; border: 1.5px solid var(--bg-dark);" }
            }
            // Nick
            span { class: "{nick_class}", style: "font-size: 13px; overflow: hidden; text-overflow: ellipsis; white-space: nowrap;",
                "{name}"
            }
        }
    }
}
