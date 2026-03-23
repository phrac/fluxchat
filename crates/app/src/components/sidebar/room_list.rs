use dioxus::prelude::*;
use std::collections::HashSet;
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
    let collapsed = use_signal(|| HashSet::<String>::new());

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

    let collapsed_set = collapsed.read().clone();

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
                    SectionHeader { label: "Channels".to_string(), collapsed: collapsed, section_key: "channels".to_string() }
                    if !collapsed_set.contains("channels") {
                        for room in ungrouped.iter() {
                            RoomEntry { room: room.clone(), is_selected: selected.as_ref() == Some(&room.room_id) }
                        }
                    }
                }

                // Space-grouped channels
                for (space_name, space_rooms) in by_space.iter() {
                    SectionHeader { label: space_name.clone(), collapsed: collapsed, section_key: space_name.clone() }
                    if !collapsed_set.contains(space_name) {
                        for room in space_rooms.iter() {
                            RoomEntry { room: room.clone(), is_selected: selected.as_ref() == Some(&room.room_id) }
                        }
                    }
                }

                // DMs
                if !dms.is_empty() {
                    SectionHeader { label: "Direct Messages".to_string(), collapsed: collapsed, section_key: "dms".to_string() }
                    if !collapsed_set.contains("dms") {
                        for room in dms.iter() {
                            DmEntry { room: room.clone(), is_selected: selected.as_ref() == Some(&room.room_id) }
                        }
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
fn SectionHeader(label: String, mut collapsed: Signal<HashSet<String>>, section_key: String) -> Element {
    let is_collapsed = collapsed.read().contains(&section_key);
    let caret = if is_collapsed { "▸" } else { "▾" };
    let key = section_key.clone();

    rsx! {
        div {
            style: "color: var(--text-dim); font-size: 12px; font-weight: 700; text-transform: uppercase; padding: 6px 8px; margin: 4px -6px 2px; letter-spacing: 0.5px; display: flex; align-items: center; gap: 4px; cursor: pointer; user-select: none; background: rgba(255,255,255,0.03);",
            onclick: move |_| {
                let mut set = collapsed.write();
                if set.contains(&key) {
                    set.remove(&key);
                } else {
                    set.insert(key.clone());
                }
            },
            span { style: "font-size: 18px; width: 14px; flex-shrink: 0; line-height: 1;", "{caret}" }
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

    // Load messages via Timeline API
    if !loaded {
        spawn(async move {
            let client = match core.get_client().await { Ok(c) => c, Err(_) => return };
            let room_id_parsed = match fluxchat_core::ruma::RoomId::parse(&room_id) { Ok(r) => r, Err(_) => return };

            // Get or create timeline for this room
            let timeline = match fluxchat_core::timeline::get_or_create_timeline(&client, &core.timelines, &room_id_parsed).await {
                Ok(tl) => tl,
                Err(e) => { tracing::error!("Failed to create timeline: {e}"); return; }
            };

            // Subscribe to get initial items + live update stream
            let (initial_items, stream) = timeline.subscribe().await;

            // Convert initial items to MessageInfo
            let mut msgs = fluxchat_core::timeline::timeline_items_to_messages(&initial_items.into_iter().collect::<Vec<_>>());

            // Pre-calculate transformed_body
            {
                let media_cache = core.media_cache.lock().unwrap();
                let mention_map: std::collections::HashMap<String, String> = ui.members_by_room.read()
                    .get(&room_id)
                    .map(|members| members.iter().filter_map(|m| {
                        let mxc = m.avatar_url.as_ref()?;
                        Some((m.user_id.clone(), media_cache.get(mxc)?.clone()))
                    }).collect())
                    .unwrap_or_default();
                for m in msgs.iter_mut() {
                    if let Some(ref html) = m.formatted_body {
                        m.transformed_body = Some(crate::state::transform_mentions(html, &mention_map));
                    }
                }
            }
            ui.messages_by_room.write().insert(room_id.clone(), msgs);

            // Paginate backwards to fill viewport if we don't have enough messages
            let real_count = ui.messages_by_room.read().get(&room_id)
                .map(|m| m.iter().filter(|msg| !msg.event_id.starts_with("~state-")).count())
                .unwrap_or(0);
            if real_count < 30 {
                let _ = timeline.paginate_backwards(30).await;
            }

            // Mark as read
            let _ = timeline.mark_as_read(fluxchat_core::ruma::api::client::receipt::create_receipt::v3::ReceiptType::Read).await;

            // Spawn a task to watch the diff stream and update messages_by_room
            {
                use futures_util::StreamExt;
                let room_id = room_id.clone();
                let timeline = timeline.clone();
                let core = core.clone();
                spawn(async move {
                    futures_util::pin_mut!(stream);
                    while let Some(_diffs) = stream.next().await {
                        // On any diff, re-convert the full timeline items (Strategy A: simple)
                        let items = timeline.items().await;
                        let mut msgs = fluxchat_core::timeline::timeline_items_to_messages(
                            &items.into_iter().collect::<Vec<_>>()
                        );
                        // Pre-calculate transformed_body
                        {
                            let media_cache = core.media_cache.lock().unwrap();
                            let mut ui = consume_context::<UiState>();
                            let mention_map: std::collections::HashMap<String, String> = ui.members_by_room.read()
                                .get(&room_id)
                                .map(|members| members.iter().filter_map(|m| {
                                    let mxc = m.avatar_url.as_ref()?;
                                    Some((m.user_id.clone(), media_cache.get(mxc)?.clone()))
                                }).collect())
                                .unwrap_or_default();
                            for m in msgs.iter_mut() {
                                if let Some(ref html) = m.formatted_body {
                                    m.transformed_body = Some(crate::state::transform_mentions(html, &mention_map));
                                }
                            }
                            ui.messages_by_room.write().insert(room_id.clone(), msgs);
                        }
                    }
                });
            }

            if let Ok(members) = fluxchat_core::messages::get_room_members(&client, &room_id).await {
                ui.members_by_room.write().insert(room_id, members);
            }
        });
    }
}
