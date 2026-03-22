use dioxus::prelude::*;
use crate::state::UiState;
use crate::components::chat::message_area::nick_color_class;

#[component]
pub fn MemberList() -> Element {
    let ui = use_context::<UiState>();
    let selected = ui.selected_room_id.read().clone();
    let members = selected.as_ref()
        .and_then(|id| ui.members_by_room.read().get(id).cloned())
        .unwrap_or_default();

    let admins: Vec<_> = members.iter().filter(|m| m.role == "Admin").collect();
    let mods: Vec<_> = members.iter().filter(|m| m.role == "Moderator").collect();
    let regular: Vec<_> = members.iter().filter(|m| m.role == "Member").collect();

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
                        MemberEntry { name: m.display_name.clone(), user_id: m.user_id.clone(), presence: m.presence.clone() }
                    }
                }
                if !mods.is_empty() {
                    div { style: "font-size: 10px; color: var(--text-dim); font-weight: 700; text-transform: uppercase; padding: 8px 8px 4px; letter-spacing: 0.5px;",
                        "% Mods — {mods.len()}"
                    }
                    for m in mods.iter() {
                        MemberEntry { name: m.display_name.clone(), user_id: m.user_id.clone(), presence: m.presence.clone() }
                    }
                }
                if !regular.is_empty() {
                    div { style: "font-size: 10px; color: var(--text-dim); font-weight: 700; text-transform: uppercase; padding: 8px 8px 4px; letter-spacing: 0.5px;",
                        "Members — {regular.len()}"
                    }
                    for m in regular.iter() {
                        MemberEntry { name: m.display_name.clone(), user_id: m.user_id.clone(), presence: m.presence.clone() }
                    }
                }
            }
        }
    }
}

#[component]
fn MemberEntry(name: String, user_id: String, presence: String) -> Element {
    let nick_class = nick_color_class(&user_id);
    let presence_color = match presence.as_str() {
        "online" => "var(--green)",
        "idle" => "var(--yellow)",
        _ => "var(--text-dim)",
    };

    rsx! {
        div { style: "display: flex; align-items: center; gap: 6px; padding: 2px 8px; border-radius: 3px; cursor: pointer;",
            // Presence dot
            div { style: "width: 6px; height: 6px; border-radius: 50%; background: {presence_color}; flex-shrink: 0;" }
            // Nick
            span { class: "{nick_class}", style: "font-size: 13px; overflow: hidden; text-overflow: ellipsis; white-space: nowrap;",
                "{name}"
            }
        }
    }
}
