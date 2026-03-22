use dioxus::prelude::*;
use std::sync::Arc;
use fluxchat_core::{AppState, RoomInfo};

use crate::state::UiState;
use crate::components::chat::message_area::nick_color_class;

#[component]
pub fn RoomList() -> Element {
    let core = use_context::<Arc<AppState>>();
    let mut ui = use_context::<UiState>();
    let rooms = ui.rooms.read().clone();
    let selected = ui.selected_room_id.read().clone();
    let selected_space = ui.selected_space_id.read().clone();

    // Filter rooms by space if one is selected
    let filtered: Vec<_> = if let Some(ref sid) = selected_space {
        let space_name = ui.spaces.read().iter()
            .find(|s| s.room_id == *sid)
            .map(|s| s.name.clone());
        rooms.iter().filter(|r| r.space_name == space_name).cloned().collect()
    } else {
        rooms.clone()
    };

    // Group into sections
    let mut ungrouped: Vec<RoomInfo> = Vec::new();
    let mut by_space: std::collections::BTreeMap<String, Vec<RoomInfo>> = std::collections::BTreeMap::new();
    let mut dms: Vec<RoomInfo> = Vec::new();

    for room in filtered.iter() {
        if room.is_direct {
            dms.push(room.clone());
        } else if let Some(ref sn) = room.space_name {
            by_space.entry(sn.clone()).or_default().push(room.clone());
        } else {
            ungrouped.push(room.clone());
        }
    }

    let header = selected_space.as_ref()
        .and_then(|sid| ui.spaces.read().iter().find(|s| s.room_id == *sid).map(|s| s.name.clone()))
        .unwrap_or_else(|| "fluxchat".to_string());

    rsx! {
        div { style: "width: 200px; flex-shrink: 0; background: var(--bg-dark); display: flex; flex-direction: column; border-right: 1px solid var(--border);",
            // Header
            div { style: "height: 40px; display: flex; align-items: center; padding: 0 12px; font-weight: 600; font-size: 14px; color: var(--text-bright); border-bottom: 1px solid var(--border); flex-shrink: 0;",
                "{header}"
            }

            // Room list
            div { style: "flex: 1; overflow-y: auto; padding: 8px 6px;",
                // Ungrouped channels
                if !ungrouped.is_empty() {
                    SectionHeader { label: "Channels".to_string() }
                    for room in ungrouped.iter() {
                        RoomEntry { room: room.clone(), is_selected: selected.as_ref() == Some(&room.room_id) }
                    }
                }

                // Space-grouped channels
                for (space_name, space_rooms) in by_space.iter() {
                    SectionHeader { label: space_name.clone() }
                    for room in space_rooms.iter() {
                        RoomEntry { room: room.clone(), is_selected: selected.as_ref() == Some(&room.room_id) }
                    }
                }

                // DMs
                if !dms.is_empty() {
                    SectionHeader { label: "Direct Messages".to_string() }
                    for room in dms.iter() {
                        DmEntry { room: room.clone(), is_selected: selected.as_ref() == Some(&room.room_id) }
                    }
                }

                if filtered.is_empty() {
                    div { style: "padding: 16px; text-align: center; color: var(--text-dim); font-size: 13px;",
                        "No rooms"
                    }
                }
            }

        }
    }
}

#[component]
fn SectionHeader(label: String) -> Element {
    rsx! {
        div { style: "color: var(--text-dim); font-size: 10px; font-weight: 700; text-transform: uppercase; padding: 10px 8px 4px; letter-spacing: 0.5px;",
            "{label}"
        }
    }
}

#[component]
fn RoomEntry(room: RoomInfo, is_selected: bool) -> Element {
    let core = use_context::<Arc<AppState>>();
    let mut ui = use_context::<UiState>();
    let has_unread = room.unread_count > 0;
    let name = room.name.to_lowercase().replace(' ', "-");
    let room_id = room.room_id.clone();
    let room_name = room.name.clone();

    let bg = if is_selected { "var(--bg-light)" } else { "transparent" };
    let color = if is_selected || has_unread { "var(--text-bright)" } else { "var(--text-muted)" };
    let weight = if has_unread { "600" } else { "400" };
    let style = format!("padding: 3px 8px; border-radius: 4px; cursor: pointer; font-size: 13px; display: flex; align-items: center; gap: 6px; background: {bg}; color: {color}; font-weight: {weight};");

    // Resolve room avatar
    let avatar_src = use_resource({
        let mxc = room.avatar_url.clone();
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

    rsx! {
        div {
            style: "{style}",
            onclick: move |_| {
                select_room(core.clone(), ui.clone(), room_id.clone(), room_name.clone());
            },
            // Avatar or icon
            if has_avatar {
                img {
                    src: "{avatar_src.read().as_ref().and_then(|v| v.as_ref()).unwrap()}",
                    style: "width: 18px; height: 18px; border-radius: 4px; object-fit: cover; flex-shrink: 0;",
                }
            } else {
                span { style: "font-family: var(--font-mono); opacity: 0.4; font-size: 12px; width: 18px; text-align: center; flex-shrink: 0;",
                    if room.is_encrypted { "🔒" } else { "#" }
                }
            }
            span { style: "overflow: hidden; text-overflow: ellipsis; white-space: nowrap; flex: 1;",
                "{name}"
            }
            if room.unread_count > 0 {
                span { style: "background: var(--red); color: white; font-size: 9px; padding: 0 4px; border-radius: 6px; font-weight: 700; flex-shrink: 0;",
                    "{room.unread_count}"
                }
            }
        }
    }
}

#[component]
fn DmEntry(room: RoomInfo, is_selected: bool) -> Element {
    let core = use_context::<Arc<AppState>>();
    let mut ui = use_context::<UiState>();
    let room_id = room.room_id.clone();
    let room_name = room.name.clone();
    let nick_class = nick_color_class(&room.name);

    let bg = if is_selected { "var(--bg-light)" } else { "transparent" };
    let color = if is_selected { "var(--text-bright)" } else { "var(--text-muted)" };
    let style = format!("padding: 3px 8px; border-radius: 4px; cursor: pointer; font-size: 13px; display: flex; align-items: center; gap: 6px; background: {bg}; color: {color};");

    // Resolve DM avatar
    let avatar_src = use_resource({
        let mxc = room.avatar_url.clone();
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
    let initial = room.name.chars().next().unwrap_or('?').to_uppercase().to_string();

    rsx! {
        div {
            style: "{style}",
            onclick: move |_| {
                select_room(core.clone(), ui.clone(), room_id.clone(), room_name.clone());
            },
            if has_avatar {
                img {
                    src: "{avatar_src.read().as_ref().and_then(|v| v.as_ref()).unwrap()}",
                    style: "width: 18px; height: 18px; border-radius: 50%; object-fit: cover; flex-shrink: 0;",
                }
            } else {
                div { class: "{nick_class}", style: "width: 18px; height: 18px; border-radius: 50%; background: var(--bg-light); display: flex; align-items: center; justify-content: center; font-size: 9px; font-weight: 600; flex-shrink: 0; line-height: 1;",
                    "{initial}"
                }
            }
            "{room.name}"
        }
    }
}

fn select_room(core: Arc<AppState>, mut ui: UiState, room_id: String, room_name: String) {
    ui.selected_room_id.set(Some(room_id.clone()));

    // Find space info
    let space_name = ui.rooms.read().iter()
        .find(|r| r.room_id == room_id)
        .and_then(|r| r.space_name.clone());
    let space_avatar = space_name.as_ref().and_then(|sn| {
        ui.spaces.read().iter().find(|s| s.name == *sn).and_then(|s| s.avatar_url.clone())
    });

    // Add to tabs
    let mut tabs = ui.open_tabs.write();
    if !tabs.iter().any(|(id, _, _, _)| id == &room_id) {
        if tabs.len() >= 8 { tabs.remove(0); }
        tabs.push((room_id.clone(), room_name, space_name, space_avatar));
    }
    drop(tabs);

    // Load messages if needed
    let loaded = ui.messages_by_room.read().contains_key(&room_id);
    if !loaded {
        spawn(async move {
            let client = match core.get_client().await { Ok(c) => c, Err(_) => return };
            if let Ok(paginated) = fluxchat_core::messages::get_room_messages(&client, &room_id, None).await {
                let msgs: Vec<_> = paginated.messages.into_iter().rev().collect();

                ui.messages_by_room.write().insert(room_id.clone(), msgs);
            }
            if let Ok(members) = fluxchat_core::messages::get_room_members(&client, &room_id).await {
                ui.members_by_room.write().insert(room_id, members);
            }
        });
    }
}
