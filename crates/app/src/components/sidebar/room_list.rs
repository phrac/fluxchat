use dioxus::prelude::*;
use std::sync::Arc;
use fluxchat_core::{AppState, RoomInfo};

use crate::state::UiState;
use crate::components::chat::message_area::nick_color_class;

#[component]
pub fn RoomList() -> Element {
    let _core = use_context::<Arc<AppState>>();
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
            div { style: "height: 40px; display: flex; align-items: center; justify-content: space-between; padding: 0 12px; font-weight: 600; font-size: 14px; color: var(--text-bright); border-bottom: 1px solid var(--border); flex-shrink: 0;",
                "{header}"
                span {
                    style: "color: var(--text-dim); cursor: pointer; font-size: 18px; line-height: 1; padding: 2px 4px; border-radius: 3px;",
                    title: "Create room or join",
                    onclick: move |_| { ui.create_room_open.set(true); },
                    "+"
                }
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
    let ui = use_context::<UiState>();
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
    let ui = use_context::<UiState>();
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

/// Public entry point for selecting a room from outside this module.
pub fn select_room_public(core: Arc<AppState>, ui: UiState, room_id: String, room_name: String) {
    select_room(core, ui, room_id, room_name);
}

fn select_room(core: Arc<AppState>, mut ui: UiState, room_id: String, room_name: String) {
    ui.selected_room_id.set(Some(room_id.clone()));

    // Mark this room as read and persist
    ui.read_rooms.write().insert(room_id.clone());
    if let Some(r) = ui.rooms.write().iter_mut().find(|r| r.room_id == room_id) {
        r.unread_count = 0;
    }
    // Persist read rooms to disk
    {
        let read = ui.read_rooms.read().clone();
        let json = serde_json::to_string(&read.into_iter().collect::<Vec<_>>()).unwrap_or_default();
        let _ = core.db.set_setting("read_rooms", &json);
    }

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

    // Send read receipt for already-loaded rooms and clear unread count
    let loaded = ui.messages_by_room.read().contains_key(&room_id);
    if loaded {
        let last_eid = ui.messages_by_room.read().get(&room_id)
            .and_then(|msgs| msgs.last())
            .map(|m| m.event_id.clone())
            .filter(|eid| !eid.starts_with("~"));
        if let Some(eid) = last_eid {
            let core = core.clone();
            let room_id = room_id.clone();
            // Clear unread count locally immediately
            let mut rooms = ui.rooms.write();
            if let Some(r) = rooms.iter_mut().find(|r| r.room_id == room_id) {
                r.unread_count = 0;
            }
            drop(rooms);
            spawn(async move {
                let client = match core.get_client().await { Ok(c) => c, Err(_) => return };
                let _ = fluxchat_core::messages::send_read_receipt(&client, &room_id, &eid).await;
            });
        }
    }

    // Load messages if needed
    if !loaded {
        spawn(async move {
            let client = match core.get_client().await { Ok(c) => c, Err(_) => return };
            // Fetch batches until we have at least 5 real messages or run out of history
            let mut all_msgs = Vec::new();
            let mut token: Option<String> = None;
            for _ in 0..5 {
                let from = token.as_deref();
                if let Ok(paginated) = fluxchat_core::messages::get_room_messages(&client, &room_id, from).await {
                    let batch: Vec<_> = paginated.messages.into_iter().rev().collect();
                    let has_more = paginated.end_token.is_some();
                    token = paginated.end_token;

                    let mut combined = batch;
                    combined.append(&mut all_msgs);
                    all_msgs = combined;

                    let real_count = all_msgs.iter().filter(|m| !m.event_id.starts_with("~state-") && !m.event_id.starts_with("~system-")).count();
                    if real_count >= 5 || !has_more { break; }
                } else { break; }
            }
            if let Some(t) = token {
                ui.pagination_tokens.write().insert(room_id.clone(), t);
            }
            ui.messages_by_room.write().insert(room_id.clone(), all_msgs);
            // Scroll to bottom — multiple attempts to handle heavy DOM renders
            let scroll_id = format!("msg-scroll-{}", room_id);
            document::eval(&format!(r#"
                function sb() {{
                    let el = document.getElementById('{scroll_id}');
                    if (el) el.scrollTop = el.scrollHeight;
                }}
                requestAnimationFrame(() => requestAnimationFrame(sb));
                setTimeout(sb, 100);
                setTimeout(sb, 300);
            "#));
            // Send read receipt and clear unread count
            if let Some(last_eid) = ui.messages_by_room.read().get(&room_id)
                .and_then(|msgs| msgs.last())
                .map(|m| m.event_id.clone())
                .filter(|eid| !eid.starts_with("~"))
            {
                let mut rooms = ui.rooms.write();
                if let Some(r) = rooms.iter_mut().find(|r| r.room_id == room_id) {
                    r.unread_count = 0;
                }
                drop(rooms);
                let _ = fluxchat_core::messages::send_read_receipt(&client, &room_id, &last_eid).await;
            }
            if let Ok(members) = fluxchat_core::messages::get_room_members(&client, &room_id).await {
                ui.members_by_room.write().insert(room_id, members);
            }
        });
    }
}
