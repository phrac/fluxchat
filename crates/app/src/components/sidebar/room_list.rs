use dioxus::prelude::*;
use std::collections::HashSet;
use std::sync::Arc;
use fluxchat_core::{AppState, RoomInfo};

use crate::state::UiState;
use crate::components::chat::message_area::nick_color_class;

const ICO_INVITE: &str = r#"<svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2.5" stroke-linecap="round" stroke-linejoin="round"><path d="M16 21v-2a4 4 0 0 0-4-4H6a4 4 0 0 0-4 4v2"/><circle cx="9" cy="7" r="4"/><line x1="19" y1="8" x2="19" y2="14"/><line x1="22" y1="11" x2="16" y2="11"/></svg>"#;
const ICO_HASH: &str = r#"<svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2.5" stroke-linecap="round"><line x1="4" y1="9" x2="20" y2="9"/><line x1="4" y1="15" x2="20" y2="15"/><line x1="10" y1="3" x2="8" y2="21"/><line x1="16" y1="3" x2="14" y2="21"/></svg>"#;
const ICO_DM: &str = r#"<svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2.5" stroke-linecap="round" stroke-linejoin="round"><path d="M21 15a2 2 0 0 1-2 2H7l-4 4V5a2 2 0 0 1 2-2h14a2 2 0 0 1 2 2z"/></svg>"#;
const ICO_SPACE: &str = r#"<svg width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><rect x="3" y="3" width="7" height="7"/><rect x="14" y="3" width="7" height="7"/><rect x="3" y="14" width="7" height="7"/><rect x="14" y="14" width="7" height="7"/></svg>"#;

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

    let core = use_context::<Arc<AppState>>();

    let selected_space_info = selected_space.as_ref()
        .and_then(|sid| ui.spaces.read().iter().find(|s| s.room_id == *sid).cloned());
    let header = selected_space_info.as_ref().map(|s| s.name.clone())
        .unwrap_or_else(|| "fluxchat".to_string());
    let header_avatar_mxc = selected_space_info.as_ref().and_then(|s| s.avatar_url.clone());

    // Resolve header avatar
    let header_avatar = use_resource({
        let mxc = header_avatar_mxc.clone();
        let core = core.clone();
        move || {
            let mxc = mxc.clone();
            let core = core.clone();
            async move {
                let mxc = mxc?;
                let client = core.get_client().await.ok()?;
                fluxchat_core::media::get_media_url(&client, &core.db, &core.media_cache, &mxc).await.ok()
            }
        }
    });
    let header_avatar_src = header_avatar.read().clone().flatten();

    // Build a map of space_name -> avatar_mxc for section headers
    let space_avatars: std::collections::HashMap<String, Option<String>> = ui.spaces.read().iter()
        .map(|s| (s.name.clone(), s.avatar_url.clone()))
        .collect();

    let collapsed_set = collapsed.read().clone();

    rsx! {
        div { style: "width: 200px; flex-shrink: 0; background: var(--bg-dark); display: flex; flex-direction: column; border-right: 1px solid var(--border);",
            // Header
            div { style: "height: 40px; display: flex; align-items: center; justify-content: space-between; padding: 0 12px; font-weight: 600; font-size: 14px; color: var(--text-bright); border-bottom: 1px solid var(--border); flex-shrink: 0;",
                div { style: "display: flex; align-items: center; gap: 8px; overflow: hidden;",
                    if let Some(ref src) = header_avatar_src {
                        img { src: "{src}", style: "width: 20px; height: 20px; border-radius: 4px; object-fit: cover; flex-shrink: 0;" }
                    } else if selected_space.is_some() {
                        // Space without avatar — generic space icon
                        span { style: "color: var(--text-dim); display: inline-flex; flex-shrink: 0;",
                            dangerous_inner_html: ICO_SPACE,
                        }
                    }
                    span { style: "overflow: hidden; text-overflow: ellipsis; white-space: nowrap;", "{header}" }
                }
                span {
                    style: "color: var(--text-dim); cursor: pointer; font-size: 18px; line-height: 1; padding: 2px 4px; border-radius: 3px;",
                    title: "Create room or join",
                    onclick: move |_| { ui.create_room_open.set(true); },
                    "+"
                }
            }

            // Room list
            div { style: "flex: 1; overflow-y: auto; padding: 8px 6px;",
                // Invitations
                { let invites = ui.invited_rooms.read().clone();
                  if !invites.is_empty() {
                    rsx! {
                        SectionHeader { label: "Invites".to_string(), collapsed: collapsed, section_key: "invites".to_string(), icon: Some(ICO_INVITE.to_string()), avatar_mxc: None }
                        if !collapsed_set.contains("invites") {
                            for inv in invites.iter() {
                                InviteEntry { room: inv.clone() }
                            }
                        }
                    }
                  } else { rsx! {} }
                }

                // Ungrouped channels
                if !ungrouped.is_empty() {
                    SectionHeader { label: "Channels".to_string(), collapsed: collapsed, section_key: "channels".to_string(), icon: Some(ICO_HASH.to_string()), avatar_mxc: None }
                    if !collapsed_set.contains("channels") {
                        for room in ungrouped.iter() {
                            RoomEntry { room: room.clone(), is_selected: selected.as_ref() == Some(&room.room_id) }
                        }
                    }
                }

                // Space-grouped channels
                for (space_name, space_rooms) in by_space.iter() {
                    { let mxc = space_avatars.get(space_name).cloned().flatten();
                      rsx! {
                        SectionHeader { label: space_name.clone(), collapsed: collapsed, section_key: space_name.clone(), icon: Some(ICO_SPACE.to_string()), avatar_mxc: mxc }
                      }
                    }
                    if !collapsed_set.contains(space_name) {
                        for room in space_rooms.iter() {
                            RoomEntry { room: room.clone(), is_selected: selected.as_ref() == Some(&room.room_id) }
                        }
                    }
                }

                // DMs
                if !dms.is_empty() {
                    SectionHeader { label: "Direct Messages".to_string(), collapsed: collapsed, section_key: "dms".to_string(), icon: Some(ICO_DM.to_string()), avatar_mxc: None }
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
fn SectionHeader(label: String, mut collapsed: Signal<HashSet<String>>, section_key: String, icon: Option<String>, avatar_mxc: Option<String>) -> Element {
    let core = use_context::<Arc<AppState>>();
    let is_collapsed = collapsed.read().contains(&section_key);
    let caret = if is_collapsed { "▸" } else { "▾" };
    let key = section_key.clone();

    // Resolve space avatar if provided
    let avatar_src = use_resource({
        let mxc = avatar_mxc.clone();
        let core = core.clone();
        move || {
            let mxc = mxc.clone();
            let core = core.clone();
            async move {
                let mxc = mxc?;
                let client = core.get_client().await.ok()?;
                fluxchat_core::media::get_media_url(&client, &core.db, &core.media_cache, &mxc).await.ok()
            }
        }
    });
    let resolved_avatar = avatar_src.read().clone().flatten();

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
            // Avatar or fallback SVG icon
            if let Some(ref src) = resolved_avatar {
                img { src: "{src}", style: "width: 16px; height: 16px; border-radius: 3px; object-fit: cover; flex-shrink: 0;" }
            } else if let Some(ref ico) = icon {
                span { style: "display: inline-flex; align-items: center; flex-shrink: 0; opacity: 0.5;",
                    dangerous_inner_html: "{ico}",
                }
            }
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
                span { style: "width: 8px; height: 8px; border-radius: 50%; background: var(--accent); flex-shrink: 0;" }
            }
        }
    }
}

#[component]
fn DmEntry(room: RoomInfo, is_selected: bool) -> Element {
    let core = use_context::<Arc<AppState>>();
    let ui = use_context::<UiState>();
    let has_unread = room.unread_count > 0;
    let room_id = room.room_id.clone();
    let room_name = room.name.clone();
    let nick_class = nick_color_class(&room.name);

    let bg = if is_selected { "var(--bg-light)" } else { "transparent" };
    let color = if is_selected || has_unread { "var(--text-bright)" } else { "var(--text-muted)" };
    let weight = if has_unread { "600" } else { "400" };
    let style = format!("padding: 3px 8px; border-radius: 4px; cursor: pointer; font-size: 13px; display: flex; align-items: center; gap: 6px; background: {bg}; color: {color}; font-weight: {weight};");

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
            span { style: "overflow: hidden; text-overflow: ellipsis; white-space: nowrap; flex: 1;",
                "{room.name}"
            }
            if has_unread {
                span { style: "width: 8px; height: 8px; border-radius: 50%; background: var(--accent); flex-shrink: 0;" }
            }
        }
    }
}

#[component]
fn InviteEntry(room: RoomInfo) -> Element {
    let app_state = use_context::<Arc<AppState>>();
    let room_id = room.room_id.clone();
    let name = if room.name.is_empty() || room.name == room.room_id { "Unnamed room".to_string() } else { room.name.clone() };

    rsx! {
        div { style: "padding: 4px 8px; border-radius: 4px; font-size: 12px; margin-bottom: 2px; background: rgba(59,130,246,0.06); border: 1px solid rgba(59,130,246,0.15);",
            div { style: "color: var(--text-bright); font-size: 13px; margin-bottom: 4px; overflow: hidden; text-overflow: ellipsis; white-space: nowrap;",
                "{name}"
            }
            if let Some(ref topic) = room.topic {
                div { style: "color: var(--text-dim); font-size: 11px; margin-bottom: 4px; overflow: hidden; text-overflow: ellipsis; white-space: nowrap;",
                    "{topic}"
                }
            }
            div { style: "display: flex; gap: 6px;",
                span {
                    style: "font-family: var(--font-mono); font-size: 11px; color: var(--green); cursor: pointer; padding: 2px 8px; border-radius: 3px; border: 1px solid var(--green);",
                    onclick: {
                        let app_state = app_state.clone();
                        let rid = room_id.clone();
                        move |_| {
                            let app_state = app_state.clone();
                            let rid = rid.clone();
                            spawn(async move {
                                let client = match app_state.get_client().await { Ok(c) => c, Err(_) => return };
                                let _ = fluxchat_core::rooms::accept_invite(&client, &rid).await;
                            });
                        }
                    },
                    "accept"
                }
                span {
                    style: "font-family: var(--font-mono); font-size: 11px; color: var(--red); cursor: pointer; padding: 2px 8px; border-radius: 3px; border: 1px solid var(--border);",
                    onclick: {
                        let app_state = app_state.clone();
                        let rid = room_id.clone();
                        move |_| {
                            let app_state = app_state.clone();
                            let rid = rid.clone();
                            spawn(async move {
                                let client = match app_state.get_client().await { Ok(c) => c, Err(_) => return };
                                let _ = fluxchat_core::rooms::reject_invite(&client, &rid).await;
                            });
                        }
                    },
                    "reject"
                }
            }
        }
    }
}

/// Public entry point for selecting a room from outside this module.
pub fn select_room_public(core: Arc<AppState>, ui: UiState, room_id: String, room_name: String) {
    select_room(core, ui, room_id, room_name);
}

fn select_room(core: Arc<AppState>, mut ui: UiState, room_id: String, room_name: String) {
    ui.selected_room_id.set(Some(room_id.clone()));

    // Record the last event_id so we can show the "new messages" divider
    if let Some(last_eid) = ui.messages_by_room.peek().get(&room_id)
        .and_then(|msgs| msgs.last())
        .map(|m| m.event_id.clone())
    {
        ui.last_read_event.write().insert(room_id.clone(), last_eid);
    }

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
                            let mut mbr = ui.messages_by_room.write();
                            mbr.insert(room_id.clone(), msgs);

                            // Sync open thread tabs for this room
                            let thread_keys: Vec<String> = mbr.keys()
                                .filter(|k| k.starts_with(&format!("thread::{room_id}::")))
                                .cloned()
                                .collect();
                            for tk in thread_keys {
                                // Extract thread root eid from key "thread::{room_id}::{eid}"
                                if let Some(root_eid) = tk.splitn(3, "::").nth(2) {
                                    let root_eid = root_eid.to_string();
                                    if let Some(room_msgs) = mbr.get(&room_id) {
                                        let thread_msgs: Vec<_> = room_msgs.iter()
                                            .filter(|m| m.event_id == root_eid || m.thread_root.as_deref() == Some(&root_eid))
                                            .cloned()
                                            .collect();
                                        mbr.insert(tk, thread_msgs);
                                    }
                                }
                            }
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
