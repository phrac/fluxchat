use dioxus::prelude::*;
use std::collections::HashMap;
use std::sync::Arc;
use fluxchat_core::AppState;

use crate::state::{UiState, nick_color_index, transform_mentions, highlight_code_blocks, html_escape};

/// Escape a string for safe interpolation into JavaScript string literals.
fn js_escape(s: &str) -> String {
    s.replace('\\', "\\\\")
     .replace('\'', "\\'")
     .replace('"', "\\\"")
     .replace('\n', "\\n")
     .replace('\r', "\\r")
}

// Inline SVG icons — render identically on all platforms
const ICO_PIN: &str = r#"<svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><path d="M12 17v5"/><path d="M9 11V4a1 1 0 0 1 1-1h4a1 1 0 0 1 1 1v7"/><path d="M5 15h14l-1.5-4H6.5z"/></svg>"#;
const ICO_SEARCH: &str = r#"<svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><circle cx="11" cy="11" r="8"/><path d="m21 21-4.3-4.3"/></svg>"#;
const ICO_THREAD: &str = r#"<svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><path d="M21 15a2 2 0 0 1-2 2H7l-4 4V5a2 2 0 0 1 2-2h14a2 2 0 0 1 2 2z"/><path d="M8 10h8"/><path d="M8 14h4"/></svg>"#;
const ICO_MEMBERS: &str = r#"<svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><path d="M16 21v-2a4 4 0 0 0-4-4H6a4 4 0 0 0-4 4v2"/><circle cx="9" cy="7" r="4"/><path d="M22 21v-2a4 4 0 0 0-3-3.87"/><path d="M16 3.13a4 4 0 0 1 0 7.75"/></svg>"#;
const ICO_REACT: &str = r#"<svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><circle cx="12" cy="12" r="10"/><path d="M8 14s1.5 2 4 2 4-2 4-2"/><line x1="9" y1="9" x2="9.01" y2="9"/><line x1="15" y1="9" x2="15.01" y2="9"/></svg>"#;
const ICO_REPLY: &str = r#"<svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><polyline points="9 17 4 12 9 7"/><path d="M20 18v-2a4 4 0 0 0-4-4H4"/></svg>"#;
const ICO_EDIT: &str = r#"<svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><path d="M17 3a2.85 2.83 0 1 1 4 4L7.5 20.5 2 22l1.5-5.5Z"/></svg>"#;
const ICO_DELETE: &str = r#"<svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><path d="M3 6h18"/><path d="M19 6v14c0 1-1 2-2 2H7c-1 0-2-1-2-2V6"/><path d="M8 6V4c0-1 1-2 2-2h4c1 0 2 1 2 2v2"/></svg>"#;
const ICO_FORWARD: &str = r#"<svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><polyline points="15 17 20 12 15 7"/><path d="M4 18v-2a4 4 0 0 1 4-4h12"/></svg>"#;
const ICO_FILE: &str = r#"<svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><path d="M15 2H6a2 2 0 0 0-2 2v16a2 2 0 0 0 2 2h12a2 2 0 0 0 2-2V7Z"/><path d="M14 2v4a2 2 0 0 0 2 2h4"/></svg>"#;
const ICO_STAR: &str = r#"<svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><polygon points="12 2 15.09 8.26 22 9.27 17 14.14 18.18 21.02 12 17.77 5.82 21.02 7 14.14 2 9.27 8.91 8.26 12 2"/></svg>"#;
const ICO_LINK: &str = r#"<svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><path d="M10 13a5 5 0 0 0 7.54.54l3-3a5 5 0 0 0-7.07-7.07l-1.72 1.71"/><path d="M14 11a5 5 0 0 0-7.54-.54l-3 3a5 5 0 0 0 7.07 7.07l1.71-1.71"/></svg>"#;
const ICO_INFO: &str = r#"<svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><circle cx="12" cy="12" r="10"/><path d="M12 16v-4"/><path d="M12 8h.01"/></svg>"#;
const ICO_GIF: &str = r#"<svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><rect x="2" y="4" width="20" height="16" rx="2"/><text x="12" y="15" text-anchor="middle" font-size="8" font-weight="bold" fill="currentColor" stroke="none">GIF</text></svg>"#;
use crate::components::chat::message_input::MessageInput;
use crate::components::chat::emoji_picker::EmojiPicker;

/// Nick color from a string hash.
pub fn nick_color_class(name: &str) -> String {
    format!("nick-{}", nick_color_index(name))
}

#[component]
pub fn MessageArea() -> Element {
    let ui = use_context::<UiState>();
    // NOTE: Do NOT read open_tabs or selected_room_id here — those are read
    // by child components (TabBar, ActiveRoomContent) to avoid cascading re-renders.

    // Set up JS→Rust channel for mention pill clicks and drag-drop
    use_future(move || async move {
        let mut eval = document::eval(r#"
            window.__fluxchat_dioxus = dioxus;

            // Suppress all WebView sounds
            document.addEventListener('keydown', (e) => {
                if (e.key === 'Escape' || e.key === 'Tab') e.preventDefault();
                // Alt+1-9 (or Cmd+1-9 on Mac) to switch tabs
                if ((e.altKey || e.metaKey) && e.key >= '1' && e.key <= '9') {
                    e.preventDefault();
                    dioxus.send('switchtab:' + e.key);
                }
            });
            window.Audio = function() { return { play:()=>{}, pause:()=>{}, load:()=>{} }; };
            window.alert = () => {};
            // Override Notification API to prevent any native notifications
            window.Notification = { permission: 'denied', requestPermission: () => Promise.resolve('denied') };

            // Intercept link clicks — open in system browser, not the WebView
            document.addEventListener('click', (e) => {
                let a = e.target.closest('a[href]');
                if (a && !a.classList.contains('mention-pill')) {
                    let href = a.getAttribute('href');
                    if (href && (href.startsWith('http://') || href.startsWith('https://'))) {
                        e.preventDefault();
                        e.stopPropagation();
                        dioxus.send('openurl:' + href);
                    }
                }
            });

            // Drag-drop file handler
            document.addEventListener('dragover', (e) => { e.preventDefault(); });
            document.addEventListener('drop', async (e) => {
                e.preventDefault();
                if (e.dataTransfer && e.dataTransfer.files.length > 0) {
                    let file = e.dataTransfer.files[0];
                    let reader = new FileReader();
                    reader.onload = () => {
                        let base64 = reader.result.split(',')[1];
                        dioxus.send('file:' + JSON.stringify({
                            name: file.name,
                            type: file.type || 'application/octet-stream',
                            data: base64
                        }));
                    };
                    reader.readAsDataURL(file);
                }
            });

            // Paste handler — images go to file upload, text goes to input
            document.addEventListener('paste', async (e) => {
                let items = e.clipboardData && e.clipboardData.items;
                if (!items) return;
                let hasImage = false;
                for (let item of items) {
                    if (item.type.startsWith('image/')) { hasImage = true; break; }
                }
                if (!hasImage) {
                    // Text paste — ensure contenteditable input is focused
                    let el = document.getElementById('flux-input');
                    if (el && document.activeElement !== el) {
                        e.preventDefault();
                        el.focus();
                        let t = e.clipboardData.getData('text/plain');
                        document.execCommand('insertText', false, t);
                        if (window.__fluxExtractText) {
                            dioxus.send('input:' + window.__fluxExtractText());
                        }
                    }
                    return;
                }
                for (let item of items) {
                    if (item.type.startsWith('image/')) {
                        e.preventDefault();
                        let file = item.getAsFile();
                        if (!file) continue;
                        let reader = new FileReader();
                        reader.onload = () => {
                            let base64 = reader.result.split(',')[1];
                            dioxus.send('file:' + JSON.stringify({
                                name: file.name || 'pasted-image.png',
                                type: file.type,
                                data: base64
                            }));
                        };
                        reader.readAsDataURL(file);
                        break;
                    }
                }
            });

        "#);
        while let Ok(msg) = eval.recv::<String>().await {
                    if let Some(json) = msg.strip_prefix("file:") {
                        // File drop or paste
                        if let Ok(parsed) = serde_json::from_str::<serde_json::Value>(json) {
                            let name = parsed["name"].as_str().unwrap_or("file").to_string();
                            let mime = parsed["type"].as_str().unwrap_or("application/octet-stream").to_string();
                            if let Some(b64) = parsed["data"].as_str() {
                                use base64::Engine;
                                if let Ok(data) = base64::engine::general_purpose::STANDARD.decode(b64) {
                                    let mut ui = consume_context::<UiState>();
                                    ui.upload_pending.set(Some((name, mime, data)));
                                }
                            }
                        }
                    } else if let Some(url) = msg.strip_prefix("openurl:") {
                        // Open URL in system browser
                        let url = url.to_string();
                        let _ = open::that(&url);
                    } else if let Some(room_id_str) = msg.strip_prefix("paginate:") {
                        // Pagination via Timeline API — just call paginate_backwards
                        // The diff stream watcher (spawned in select_room) will update messages_by_room
                        let room_id = room_id_str.to_string();
                        let app_state = consume_context::<Arc<AppState>>();
                        if let Ok(room_id_parsed) = fluxchat_core::ruma::RoomId::parse(&room_id) {
                            let timelines = app_state.timelines.lock().await;
                            if let Some(timeline) = timelines.get(&*room_id_parsed) {
                                let _ = timeline.paginate_backwards(20).await;
                            }
                        }
                        // Release the JS loading lock after a delay
                        let sid = format!("msg-scroll-{}", room_id);
                        tokio::time::sleep(std::time::Duration::from_millis(500)).await;
                        document::eval(&format!(r#"
                            let el = document.getElementById('{}');
                            if (el) el.__fluxLoading = false;
                        "#, js_escape(&sid)));
                    } else if let Some(idx_str) = msg.strip_prefix("switchtab:") {
                        // Alt/Cmd+1-9 tab switching
                        if let Ok(idx) = idx_str.parse::<usize>() {
                            let idx = idx - 1; // Convert 1-based to 0-based
                            let mut ui = consume_context::<UiState>();
                            let tabs = ui.open_tabs.peek().clone();
                            if let Some((room_id, _, _, _)) = tabs.get(idx) {
                                ui.selected_room_id.set(Some(room_id.clone()));
                            }
                        }
                    } else if !msg.is_empty() {
                        // Mention pill click
                        let mut ui = consume_context::<UiState>();
                        ui.profile_user_id.set(Some(msg));
                    }
        }
    });

    rsx! {
        div { style: "flex: 1; display: flex; flex-direction: column; min-width: 0; background: var(--bg-mid);",
            // Tab bar — separate component, only re-renders on tab changes
            TabBar {}
            TopicBar {}

            // HUDs
            if *ui.search_open.read() { SearchHud {} }
            if *ui.pins_open.read() { PinsHud {} }
            if *ui.threads_open.read() { ThreadsHud {} }
            if *ui.files_open.read() { FilesHud {} }
            if *ui.notices_open.read() { NoticesHud {} }

            // Message content — separate component, only re-renders on selected room changes
            ActiveRoomContent {}
        }
    }
}

/// Tab bar — isolated from message rendering.
#[component]
fn TabBar() -> Element {
    let ui = use_context::<UiState>();
    let tabs = ui.open_tabs.read().clone();
    let selected = ui.selected_room_id.read().clone();

    rsx! {
        div { style: "display: flex; align-items: center; height: 40px; border-bottom: 1px solid var(--border); flex-shrink: 0; background: var(--bg-dark);",
            div { style: "display: flex; align-items: center; flex: 1; min-width: 0; padding: 0 8px; gap: 0; overflow-x: auto;",
                for (id, name, space, space_mxc) in tabs.iter() {
                    ChannelTab {
                        room_id: id.clone(),
                        name: name.clone(),
                        space_name: space.clone(),
                        space_avatar_mxc: space_mxc.clone(),
                        is_active: selected.as_ref() == Some(id),
                    }
                }
            }
            if selected.is_some() {
                RoomActions {}
            }
        }
    }
}

/// Topic bar — slim IRC-style topic display below tabs.
#[component]
fn TopicBar() -> Element {
    let ui = use_context::<UiState>();
    let selected = ui.selected_room_id.read().clone();
    let topic = selected.as_ref().and_then(|id| {
        ui.rooms.peek().iter().find(|r| r.room_id == *id).and_then(|r| r.topic.clone())
    });

    if let Some(topic) = topic {
        if !topic.is_empty() {
            rsx! {
                div { style: "padding: 3px 12px; font-family: var(--font-mono); font-size: 11px; color: var(--text-dim); background: var(--bg-dark); border-bottom: 1px solid var(--border); overflow: hidden; text-overflow: ellipsis; white-space: nowrap;",
                    "{topic}"
                }
            }
        } else {
            rsx! {}
        }
    } else {
        rsx! {}
    }
}

/// Room content area — renders all open rooms, shows/hides via CSS.
/// RoomMessages components stay alive and their DOM is preserved across tab switches.
#[component]
fn ActiveRoomContent() -> Element {
    let ui = use_context::<UiState>();
    // Read both signals — this component re-renders on tab/room changes,
    // but it only updates CSS classes, NOT child component props.
    let selected = ui.selected_room_id.read().clone();
    let tabs = ui.open_tabs.read().clone();

    rsx! {
        // Keep ALL room message components alive — CSS toggles visibility.
        // RoomMessages only receives room_id which never changes, so Dioxus
        // memoization prevents re-rendering on tab switch.
        for (tab_room_id, _, _, _) in tabs.iter() {
            { let is_active = selected.as_ref() == Some(tab_room_id);
              let style = if is_active {
                  "flex: 1; display: flex; flex-direction: column; min-height: 0;"
              } else {
                  "display: none;"
              };
              rsx! {
                div { key: "{tab_room_id}", style: "{style}",
                    RoomMessages { room_id: tab_room_id.clone() }
                }
              }
            }
        }

        if selected.is_none() {
            { let space_id = ui.selected_space_id.read().clone();
              if let Some(sid) = space_id {
                rsx! { SpaceOverview { space_id: sid } }
              } else {
                rsx! {
                    div { style: "flex: 1; display: flex; align-items: center; justify-content: center; color: var(--text-dim);",
                        "Select a channel to start chatting"
                    }
                }
              }
            }
        }

        // Typing indicator for active room
        if let Some(ref active_id) = selected {
            { let typing = ui.typing_by_room.read().get(active_id).cloned().unwrap_or_default();
              if !typing.is_empty() {
                let names: Vec<_> = typing.iter().map(|u| u.split(':').next().unwrap_or(u).trim_start_matches('@').to_string()).collect();
                let text = if names.len() == 1 { format!("{} is typing...", names[0]) }
                    else { format!("{} are typing...", names.join(", ")) };
                rsx! {
                    div { style: "padding: 2px 12px; font-size: 12px; color: var(--text-dim); font-family: var(--font-mono); flex-shrink: 0;",
                        "{text}"
                    }
                }
              } else { rsx! {} }
            }
        }

        if *ui.gifs_open.read() { GifsHud {} }

        // Input always visible — supports /settings, /sessions etc. without a room
        MessageInput {}
    }
}

/// Per-room message list — stays in DOM, shown/hidden via CSS.
#[component]
fn RoomMessages(room_id: String) -> Element {
    let mut ui = use_context::<UiState>();
    let app_state = use_context::<Arc<AppState>>();
    let mut react_menu_event = use_signal(|| Option::<String>::None);

    // Check for room tombstone (upgrade)
    let tombstone = use_resource({
        let app_state = app_state.clone();
        let rid = room_id.clone();
        move || {
            let app_state = app_state.clone();
            let rid = rid.clone();
            async move {
                let client = app_state.get_client().await.ok()?;
                fluxchat_core::rooms::check_tombstone(&client, &rid).await
            }
        }
    });

    let my_user_id = ui.user_id.peek().clone();
    // Use peek() to avoid subscribing to the entire HashMap signal.
    // The diff stream watcher (spawned in select_room) directly writes to this
    // signal, which is the only trigger for re-render.
    let all_messages = ui.messages_by_room.read().get(&room_id).cloned().unwrap_or_default();
    let ignored = ui.ignored_users.peek().clone();
    let ts_format = ui.timestamp_format.read().clone();
    let compact = *ui.compact_mode.read();
    let show_joinpart = *ui.show_joinpart.read();
    let highlight_words = ui.highlight_words.read().clone();

    // Build receipt map: event_id → Vec<ReadReceipt> (excluding self)
    let receipt_map: HashMap<String, Vec<fluxchat_core::ReadReceipt>> = {
        let receipts = ui.receipts_by_room.read().get(&room_id).cloned().unwrap_or_default();
        let mut map: HashMap<String, Vec<fluxchat_core::ReadReceipt>> = HashMap::new();
        for r in receipts {
            if r.user_id != my_user_id {
                map.entry(r.event_id.clone()).or_default().push(r);
            }
        }
        map
    };

    // Build thread reply counts BEFORE filtering (need all messages to count)
    let thread_counts: HashMap<String, u32> = {
        let mut counts = HashMap::new();
        for m in &all_messages {
            if let Some(ref root) = m.thread_root {
                *counts.entry(root.clone()).or_insert(0) += 1;
            }
        }
        counts
    };

    // Filter: hide ignored users, join/part if toggled off. For regular rooms, also hide thread replies.
    let is_thread_view = room_id.starts_with("thread::");
    let messages: Vec<_> = all_messages.into_iter()
        .filter(|m| !ignored.contains(&m.sender))
        .filter(|m| is_thread_view || m.thread_root.is_none())
        .filter(|m| show_joinpart || !(m.event_id.starts_with("~state-") || m.event_id.starts_with("~system-")))
        .collect();

    // Find the last-read event to show the "new messages" divider
    let last_read_eid = ui.last_read_event.peek().get(&room_id).cloned();
    // Find the index of the last-read message — divider goes after it
    let unread_divider_idx = last_read_eid.as_ref().and_then(|eid| {
        messages.iter().position(|m| m.event_id == *eid)
    }).filter(|idx| *idx < messages.len().saturating_sub(1)); // Only show if there are messages after it

    // Count new messages after the divider for the floating pill
    let new_msg_count = unread_divider_idx
        .map(|idx| messages.len() - idx - 1)
        .unwrap_or(0);

    // Build avatar maps with one cache lock
    let (avatar_map, mention_avatars, uncached) = {
        let cache = app_state.media_cache.lock().unwrap();
        let mut amap = HashMap::new();
        let mut uncached = Vec::new();
        for m in &messages {
            if let Some(mxc) = &m.avatar_url {
                if let Some(data_uri) = cache.get(mxc) {
                    amap.insert(mxc.clone(), data_uri.clone());
                } else {
                    uncached.push(mxc.clone());
                }
            }
        }
        let mmap: HashMap<String, String> = ui.members_by_room.read()
            .get(&room_id)
            .map(|members| members.iter().filter_map(|m| {
                let mxc = m.avatar_url.as_ref()?;
                Some((m.user_id.clone(), cache.get(mxc)?.clone()))
            }).collect())
            .unwrap_or_default();
        uncached.sort();
        uncached.dedup();
        (amap, mmap, uncached)
    };

    // Background-resolve uncached message avatars
    if !uncached.is_empty() {
        let app_state = app_state.clone();
        spawn(async move {
            let client = match app_state.get_client().await { Ok(c) => c, Err(_) => return };
            for mxc in uncached {
                let _ = fluxchat_core::media::get_media_url(&client, &app_state.db, &app_state.media_cache, &mxc).await;
            }
        });
    }

    let scroll_id = format!("msg-scroll-{}", room_id);

    // Pagination: detect scroll near top via JS on the column-reverse container.
    // In column-reverse, scrollTop is 0 at the bottom (newest). Scrolling up makes
    // scrollTop go negative (WebKit) or the scroll range inverts. We detect "near top"
    // by checking if the user has scrolled far enough from the start.
    {
        let sid = scroll_id.clone();
        let room_id_pg = room_id.clone();
        let _app_state_pg = app_state.clone();
        use_effect(move || {
            // Set up a scroll listener for pagination via JS
            let sid = sid.clone();
            let room_id = room_id_pg.clone();
            document::eval(&format!(r#"
                setTimeout(() => {{
                    let el = document.getElementById('{}');
                    if (!el || el.__fluxPgSetup) return;
                    el.__fluxPgSetup = true;
                    el.addEventListener('scroll', () => {{
                        // In column-reverse, scrollTop=0 is bottom. Scrolling up goes negative.
                        // "Near top" = scrollTop is at the far negative end.
                        // scrollTop + scrollHeight - clientHeight ≈ 0 means at the visual top.
                        let atTop = (el.scrollTop + el.clientHeight - el.scrollHeight) > -100;
                        // Alternative: some browsers use abs(scrollTop) near scrollHeight
                        let atTop2 = (Math.abs(el.scrollTop) + el.clientHeight + 100) >= el.scrollHeight;
                        if ((atTop || atTop2) && !el.__fluxLoading) {{
                            el.__fluxLoading = true;
                            if (window.__fluxchat_dioxus) window.__fluxchat_dioxus.send('paginate:{}');
                        }}
                    }});
                }}, 50);
            "#, js_escape(&sid), js_escape(&room_id)));
        });
    }

    rsx! {
        div { style: "flex: 1; position: relative; display: flex; flex-direction: column; min-height: 0;",
        // Room tombstone banner
        { let ts = tombstone.read().clone().flatten();
          if let Some((body, replacement_id)) = ts {
            let rid = replacement_id.clone();
            rsx! {
                div { style: "padding: 8px 12px; background: rgba(234,179,8,0.1); border-bottom: 1px solid rgba(234,179,8,0.2); font-family: var(--font-mono); font-size: 12px; color: var(--yellow); display: flex; align-items: center; gap: 8px; flex-shrink: 0;",
                    span { style: "flex: 1;",
                        if body.is_empty() { "This room has been upgraded." } else { "{body}" }
                    }
                    span {
                        style: "cursor: pointer; padding: 2px 10px; border-radius: 4px; border: 1px solid var(--yellow); font-size: 11px; flex-shrink: 0;",
                        onclick: {
                            let app_state = app_state.clone();
                            move |_| {
                                let rid = rid.clone();
                                let app_state = app_state.clone();
                                spawn(async move {
                                    let client = match app_state.get_client().await { Ok(c) => c, Err(_) => return };
                                    let _ = fluxchat_core::rooms::join_room(&client, &rid).await;
                                });
                            }
                        },
                        "follow upgrade"
                    }
                }
            }
          } else { rsx! {} }
        }
        // Floating "new messages" pill
        if new_msg_count > 0 {
            div { style: "position: absolute; bottom: 8px; left: 50%; transform: translateX(-50%); z-index: 5;",
                div {
                    style: "background: var(--accent-muted); color: var(--bg-deep); font-family: var(--font-mono); font-size: 11px; font-weight: 600; padding: 3px 12px; border-radius: 12px; cursor: pointer; white-space: nowrap; box-shadow: 0 2px 8px rgba(0,0,0,0.3);",
                    onclick: {
                        let scroll_id = scroll_id.clone();
                        let rid = room_id.clone();
                        move |_| {
                            // Scroll to bottom (which is scroll top in column-reverse)
                            let sid = scroll_id.clone();
                            document::eval(&format!(r#"
                                let el = document.getElementById('{}');
                                if (el) el.scrollTop = 0;
                            "#, js_escape(&sid)));
                            // Clear the divider
                            ui.last_read_event.write().remove(&rid);
                        }
                    },
                    { let s = if new_msg_count == 1 { "message" } else { "messages" };
                      format!("{new_msg_count} new {s}")
                    }
                }
            }
        }
        div {
            id: "{scroll_id}",
            class: if compact { "compact" } else { "" },
            // column-reverse: browser naturally shows newest messages (bottom) first.
            // No JS scroll-to-bottom needed. Scroll position preserved across re-renders.
            style: "flex: 1; overflow-y: auto; overflow-x: hidden; padding: 8px 12px; display: flex; flex-direction: column-reverse;",

            // Single inner div holds messages in chronological order.
            // column-reverse on parent makes this appear at the bottom of the scroll area.
            div {
            if messages.is_empty() {
                { let is_loaded = ui.messages_by_room.peek().contains_key(&room_id);
                rsx! {
                    div { style: "display: flex; align-items: center; justify-content: center; height: 100%; color: var(--text-dim); font-family: var(--font-mono); font-size: 13px; gap: 8px;",
                        if is_loaded {
                            "No messages yet"
                        } else {
                            span { style: "animation: pulse 1.5s ease-in-out infinite;", "loading messages..." }
                        }
                    }
                } }
            }
            for (_idx, msg) in messages.iter().enumerate() {
                // Unread divider — "new messages" line
                if unread_divider_idx == Some(_idx) {
                    div { style: "display: flex; align-items: center; gap: 8px; padding: 6px 0; margin: 4px 0;",
                        div { style: "flex: 1; height: 1px; background: var(--accent-muted);" }
                        span { style: "font-family: var(--font-mono); font-size: 11px; color: var(--accent-muted); flex-shrink: 0; text-transform: uppercase; letter-spacing: 0.5px;",
                            "new messages"
                        }
                        div { style: "flex: 1; height: 1px; background: var(--accent-muted);" }
                    }
                }
                { let is_system = msg.event_id.starts_with("~state-") || msg.event_id.starts_with("~system-");
                  let eid = msg.event_id.clone();
                  let is_highlighted = !highlight_words.is_empty() && highlight_words.iter().any(|w| msg.body.to_lowercase().contains(w));
                  if is_system {
                    let (arrow, arrow_color) = if msg.body.contains("has joined") {
                        ("-->", "var(--green)")
                    } else if msg.body.contains("has left") || msg.body.contains("was kicked") || msg.body.contains("was banned") {
                        ("<--", "var(--red)")
                    } else if msg.body.contains("was invited") {
                        ("-->", "var(--accent-muted)")
                    } else {
                        ("---", "var(--text-dim)")
                    };
                    rsx! {
                        div { key: "{eid}", style: "display: flex; padding: 2px 0; font-size: 12px; color: var(--text-dim); font-family: var(--font-mono); align-items: center;",
                            span { class: "msg-time", "" }
                            span { style: "width: 120px; flex-shrink: 0;" }
                            div { style: "width: 1px; align-self: stretch; background: var(--border); margin: 0 8px; flex-shrink: 0;" }
                            span { style: "color: {arrow_color}; margin-right: 4px;", "{arrow}" }
                            "{msg.body}"
                        }
                    }
                  } else if msg.is_emote {
                    // IRC-style /me action: * nick does something
                    let nick_class = nick_color_class(&msg.sender);
                    let time = {
                        let ts = msg.timestamp;
                        let secs = ts % 86400;
                        let h = secs / 3600;
                        let m = (secs % 3600) / 60;
                        let s = secs % 60;
                        match ts_format.as_str() {
                            "full" => format!("{:02}:{:02}:{:02}", h, m, s),
                            "off" => String::new(),
                            _ => format!("{:02}:{:02}", h, m),
                        }
                    };
                    let emote_text = if let Some(ref html) = msg.formatted_body {
                        let transformed = highlight_code_blocks(&transform_mentions(html, &mention_avatars));
                        format!("* <span class=\"{nick_class}\" style=\"font-weight: 600;\">{}</span> {}", html_escape(&msg.sender_name), transformed)
                    } else {
                        format!("* <span class=\"{nick_class}\" style=\"font-weight: 600;\">{}</span> {}", html_escape(&msg.sender_name), html_escape(&msg.body))
                    };
                    rsx! {
                        div { key: "{eid}", "data-event-id": "{eid}",
                            div { class: "msg-row", style: "display: flex; padding: 1px 0; align-items: flex-start;",
                                span { class: "msg-time", style: "line-height: 20px;", "{time}" }
                                span { style: "width: 120px; flex-shrink: 0;" }
                                div { style: "width: 1px; align-self: stretch; background: var(--border); margin: 0 8px; flex-shrink: 0;" }
                                span { class: "msg-body", style: "font-style: italic; color: var(--text-muted);", dangerous_inner_html: "{emote_text}" }
                            }
                        }
                    }
                  } else {
                    let nick_class = nick_color_class(&msg.sender);
                    let time = {
                        let ts = msg.timestamp;
                        let secs = ts % 86400;
                        let h = secs / 3600;
                        let m = (secs % 3600) / 60;
                        let s = secs % 60;
                        match ts_format.as_str() {
                            "full" => format!("{:02}:{:02}:{:02}", h, m, s),
                            "off" => String::new(),
                            _ => format!("{:02}:{:02}", h, m),
                        }
                    };
                    let avatar_src = msg.avatar_url.as_ref().and_then(|mxc| avatar_map.get(mxc));
                    let initial = msg.sender_name.chars().next().unwrap_or('?').to_uppercase().to_string();
                    let is_own = msg.sender == my_user_id;
                    let is_local = msg.event_id.starts_with("~local-");
                    let eid_react = msg.event_id.clone();
                    let eid_reply = msg.event_id.clone();
                    let eid_edit = msg.event_id.clone();
                    let eid_del = msg.event_id.clone();
                    let eid_pin = msg.event_id.clone();
                    let reply_name = msg.sender_name.clone();
                    let reply_body = msg.body.clone();
                    let edit_body = msg.body.clone();
                    let rid_del = room_id.clone();
                    let rid_react = room_id.clone();
                    let rid_pin = room_id.clone();
                    let showing_react = react_menu_event.read().as_ref() == Some(&eid);

                    // Optimization: Check if we have pre-computed transformed_body
                    let content_rsx = if msg.poll.is_some() {
                        let poll = msg.poll.clone().unwrap();
                        rsx! { PollDisplay { poll: poll, event_id: msg.event_id.clone(), room_id: room_id.clone(), my_user_id: my_user_id.clone() } }
                    } else if msg.is_video && msg.image_mxc.is_some() {
                        rsx! { InlineVideo { room_id: room_id.clone(), event_id: msg.event_id.clone(), body: msg.body.clone(), image_width: msg.image_width, image_height: msg.image_height } }
                    } else if msg.image_mxc.is_some() {
                        rsx! { InlineImage { room_id: room_id.clone(), event_id: msg.event_id.clone(), body: msg.body.clone(), image_mxc: msg.image_mxc.clone(), image_width: msg.image_width, image_height: msg.image_height } }
                    } else if let Some(ref transformed) = msg.transformed_body {
                        let highlighted = highlight_code_blocks(transformed);
                        rsx! { span { class: "msg-body", dangerous_inner_html: "{highlighted}" } }
                    } else if let Some(ref html) = msg.formatted_body {
                        let transformed = highlight_code_blocks(&transform_mentions(html, &mention_avatars));
                        rsx! { span { class: "msg-body", dangerous_inner_html: "{transformed}" } }
                    } else if msg.body.contains('\n') {
                        let body_html = html_escape(&msg.body).replace('\n', "<br>");
                        rsx! { span { class: "msg-body", dangerous_inner_html: "{body_html}" } }
                    } else {
                        rsx! { span { class: "msg-body", "{msg.body}" } }
                    };

                    // Look up reply context (skip in thread views — thread fallback replies are noise)
                    let reply_context = if is_thread_view { None } else {
                        msg.reply_to_id.as_ref().and_then(|rid| {
                            messages.iter().find(|m| m.event_id == *rid)
                                .map(|m| (m.sender_name.clone(), m.body.clone()))
                        })
                    };

                    rsx! {
                        div { key: "{eid}", "data-event-id": "{eid}",
                            if let Some((ref reply_sender, ref reply_preview)) = reply_context {
                                div { style: "display: flex; padding: 0 0 0 72px; font-size: 11px; font-family: var(--font-mono); color: var(--text-dim); align-items: center; gap: 4px;",
                                    span { style: "color: var(--green);", "╭─" }
                                    span { style: "color: var(--text-muted);", "{reply_sender}:" }
                                    span { style: "overflow: hidden; text-overflow: ellipsis; white-space: nowrap; max-width: 400px;",
                                        { reply_preview.chars().take(80).collect::<String>() }
                                    }
                                }
                            }
                            div { class: if is_highlighted { "msg-row msg-highlight" } else { "msg-row" }, style: "display: flex; padding: 1px 0; align-items: flex-start;",
                                span { class: "msg-time", style: "line-height: 20px;", "{time}" }
                                { let sender_id = msg.sender.clone();
                                  let sender_id2 = msg.sender.clone();
                                rsx! {
                                if let Some(src) = avatar_src {
                                    img { src: "{src}", style: "width: 16px; height: 16px; border-radius: 50%; object-fit: cover; flex-shrink: 0; margin-right: 4px; margin-top: 2px; cursor: pointer;",
                                        onclick: move |_| { consume_context::<UiState>().profile_user_id.set(Some(sender_id.clone())); },
                                    }
                                } else {
                                    div { class: "{nick_class}", style: "width: 16px; height: 16px; border-radius: 50%; display: flex; align-items: center; justify-content: center; font-size: 8px; font-weight: 700; color: white; flex-shrink: 0; margin-right: 4px; margin-top: 2px; background: var(--bg-light); cursor: pointer;",
                                        onclick: move |_| { consume_context::<UiState>().profile_user_id.set(Some(sender_id.clone())); },
                                        "{initial}"
                                    }
                                }
                                span { style: "width: 100px; text-align: right; flex-shrink: 0; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; line-height: 20px; cursor: pointer;",
                                    onclick: move |_| { consume_context::<UiState>().profile_user_id.set(Some(sender_id2.clone())); },
                                    span { class: "{nick_class}", style: "font-weight: 600; font-family: var(--font-mono); font-size: 13px;", "{msg.sender_name}" }
                                }
                                } }
                                div { style: "width: 1px; align-self: stretch; background: var(--border); margin: 0 8px; flex-shrink: 0;" }
                                { let urls = extract_urls(&msg.body);
                                rsx! {
                                div { style: "min-width: 0; flex: 1;",
                                    {content_rsx}
                                    for url in urls.iter() {
                                        UrlPreviewCard { url: url.clone() }
                                    }
                                    if !msg.reactions.is_empty() {
                                        div { style: "display: flex; flex-wrap: wrap; gap: 4px; padding: 2px 0;",
                                            for r in msg.reactions.iter() {
                                                span { style: "font-size: 12px; background: rgba(59,130,246,0.15); border-radius: 10px; padding: 1px 6px; color: var(--text-muted); cursor: default; font-family: var(--font-mono);",
                                                    title: "{r.users.join(\", \")}",
                                                    "{r.emoji} {r.count}"
                                                }
                                            }
                                        }
                                    }
                                    // Thread indicator — show on messages that are thread roots (not in thread views)
                                    { let tc = if is_thread_view { 0 } else { thread_counts.get(&eid).copied().unwrap_or(0) };
                                      if tc > 0 {
                                        let label = if tc == 1 { "1 reply".to_string() } else { format!("{tc} replies") };
                                        let thread_eid = eid.clone();
                                        let thread_body = msg.body.clone();
                                        let thread_room_id = room_id.clone();
                                        rsx! {
                                            div {
                                                style: "display: inline-flex; align-items: center; gap: 5px; padding: 3px 10px; margin-top: 4px; cursor: pointer; background: rgba(59,130,246,0.08); border: 1px solid rgba(59,130,246,0.15); border-radius: 4px; transition: background 0.1s, border-color 0.1s;",
                                                class: "cmd-item",
                                                onclick: move |_| {
                                                    let ui = consume_context::<UiState>();
                                                    crate::components::layout::open_thread_tab(ui, thread_room_id.clone(), thread_eid.clone(), thread_body.clone());
                                                },
                                                span { style: "display: inline-flex; color: var(--accent-muted);",
                                                    dangerous_inner_html: ICO_THREAD,
                                                }
                                                span { style: "font-family: var(--font-mono); font-size: 11px; color: var(--accent-muted); font-weight: 600;",
                                                    "{label}"
                                                }
                                                span { style: "font-family: var(--font-mono); font-size: 11px; color: var(--text-dim);",
                                                    "→"
                                                }
                                            }
                                        }
                                      } else { rsx! {} }
                                    }
                                } } }
                                // Hover actions
                                if !is_local {
                                    div { class: if showing_react { "msg-actions has-picker" } else { "msg-actions" },
                                        span { class: "msg-action", onclick: move |_| { react_menu_event.set(if react_menu_event.peek().as_ref() == Some(&eid_react) { None } else { Some(eid_react.clone()) }); }, dangerous_inner_html: ICO_REACT }
                                        span { class: "msg-action", onclick: { let name = reply_name.clone(); let body = reply_body.clone(); let eid = eid_reply.clone(); move |_| { ui.reply_to.set(Some((eid.clone(), name.clone(), body.clone()))); ui.edit_event.set(None); } }, dangerous_inner_html: ICO_REPLY }
                                        span { class: "msg-action", onclick: { let body = msg.body.clone(); let fb = msg.formatted_body.clone(); let sn = msg.sender_name.clone(); move |_| { let mut ui = consume_context::<UiState>(); ui.forward_event.set(Some((body.clone(), fb.clone().unwrap_or_default(), Some(sn.clone())))); } }, dangerous_inner_html: ICO_FORWARD }
                                        span { class: "msg-action", onclick: { let eid = eid_pin.clone(); let rid = rid_pin.clone(); let app_state = app_state.clone(); move |_| { let eid = eid.clone(); let rid = rid.clone(); let app_state = app_state.clone(); spawn(async move { let client = match app_state.get_client().await { Ok(c) => c, Err(_) => return }; let _ = fluxchat_core::messages::pin_message(&client, &rid, &eid).await; }); } }, dangerous_inner_html: ICO_PIN }
                                        if is_own {
                                            span { class: "msg-action", onclick: { let body = edit_body.clone(); let eid = eid_edit.clone(); move |_| { ui.edit_event.set(Some((eid.clone(), body.clone()))); ui.reply_to.set(None); } }, dangerous_inner_html: ICO_EDIT }
                                            span { class: "msg-action", onclick: { let eid = eid_del.clone(); let rid = rid_del.clone(); let app_state = app_state.clone(); move |_| { let eid = eid.clone(); let rid = rid.clone(); let app_state = app_state.clone(); spawn(async move { if let Ok(rid_parsed) = fluxchat_core::ruma::RoomId::parse(&rid) { let timelines = app_state.timelines.lock().await; if let Some(timeline) = timelines.get(&*rid_parsed) { if let Ok(eid_parsed) = fluxchat_core::ruma::EventId::parse(&eid) { let item_id = matrix_sdk_ui::timeline::TimelineEventItemId::EventId(eid_parsed); let _ = timeline.redact(&item_id, None).await; } } } }); } }, dangerous_inner_html: ICO_DELETE }
                                        }
                                        if showing_react {
                                            div { style: "position: absolute; right: 0; bottom: 100%; margin-bottom: 4px; z-index: 100;",
                                                EmojiPicker {
                                                    on_select: {
                                                        let eid = eid.clone();
                                                        let rid = rid_react.clone();
                                                        let app_state = app_state.clone();
                                                        move |emoji: String| {
                                                            react_menu_event.set(None);
                                                            let emoji = emoji.clone();
                                                            let eid = eid.clone();
                                                            let rid = rid.clone();
                                                            let app_state = app_state.clone();
                                                            spawn(async move {
                                                                if let Ok(rid_parsed) = fluxchat_core::ruma::RoomId::parse(&rid) {
                                                                    let timelines = app_state.timelines.lock().await;
                                                                    if let Some(timeline) = timelines.get(&*rid_parsed) {
                                                                        if let Ok(eid_parsed) = fluxchat_core::ruma::EventId::parse(&eid) {
                                                                            let item_id = matrix_sdk_ui::timeline::TimelineEventItemId::EventId(eid_parsed);
                                                                            let _ = timeline.toggle_reaction(&item_id, &emoji).await;
                                                                        }
                                                                    }
                                                                }
                                                            });
                                                        }
                                                    },
                                                }
                                            }
                                        }
                                    }
                                }
                            }
                            // Read receipts — right-aligned below the message row
                            { if let Some(readers) = receipt_map.get(&eid) {
                                let readers = readers.clone();
                                rsx! {
                                    div { style: "display: flex; justify-content: flex-end; padding: 0 12px 2px 0;",
                                        for reader in readers.iter().take(8) {
                                            { let initial = reader.display_name.chars().next().unwrap_or('?').to_uppercase().to_string();
                                              let avatar = reader.avatar_url.as_ref().and_then(|mxc| {
                                                  app_state.media_cache.lock().unwrap().get(mxc).cloned()
                                              });
                                            rsx! {
                                                if let Some(ref src) = avatar {
                                                    img { src: "{src}", title: "{reader.display_name}", style: "width: 14px; height: 14px; border-radius: 50%; object-fit: cover; margin-left: -2px; border: 1.5px solid var(--bg-main);" }
                                                } else {
                                                    div { title: "{reader.display_name}", style: "width: 14px; height: 14px; border-radius: 50%; background: var(--bg-hover); display: flex; align-items: center; justify-content: center; font-size: 7px; font-weight: 600; color: var(--text-dim); margin-left: -2px; border: 1.5px solid var(--bg-main);",
                                                        "{initial}"
                                                    }
                                                }
                                            } }
                                        }
                                    }
                                }
                            } else { rsx! {} } }
                        }
                    }
                  }
                }
            }
            } // inner div
        } // scroll container
        } // position wrapper
    }
}

#[component]
fn ChannelTab(room_id: String, name: String, space_name: Option<String>, space_avatar_mxc: Option<String>, is_active: bool) -> Element {
    let mut ui = use_context::<UiState>();
    let core = use_context::<Arc<AppState>>();

    let tab_class = if is_active { "channel-tab active" } else { "channel-tab" };
    let is_dm = ui.rooms.peek().iter().any(|r| r.room_id == room_id && r.is_direct);
    let label = if is_dm { name.clone() } else { format!("#{name}") };

    // Resolve space avatar
    let avatar_src = use_resource({
        let mxc = space_avatar_mxc.clone();
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
    let avatar_resolved = avatar_src.read().clone().flatten();

    let tab_id = room_id.clone();
    let close_id = room_id.clone();

    rsx! {
        div {
            class: "{tab_class}",
            style: "display: flex; align-items: center; gap: 4px;",
            onclick: move |_| { ui.selected_room_id.set(Some(tab_id.clone())); },

            // Space avatar
            if let Some(ref src) = avatar_resolved {
                img {
                    src: "{src}",
                    style: "width: 14px; height: 14px; border-radius: 3px; object-fit: cover; flex-shrink: 0;",
                }
            } else if space_name.is_some() {
                span { style: "font-size: 10px; opacity: 0.5; font-family: var(--font-mono); flex-shrink: 0;",
                    {space_name.as_ref().map(|s| s.chars().take(2).collect::<String>().to_uppercase()).unwrap_or_default()}
                }
            }

            span { class: "tab-title", "{label}" }

            span {
                class: "tab-close",
                onclick: {
                    let cid = close_id.clone();
                    let core2 = core.clone();
                    move |e: Event<MouseData>| {
                        e.stop_propagation();
                        // Compute new state before any signal writes
                        let is_selected = ui.selected_room_id.read().as_ref() == Some(&cid);
                        let next = if is_selected {
                            ui.open_tabs.read().iter()
                                .rfind(|(id, _, _, _)| id != &cid)
                                .map(|(id, _, _, _)| id.clone())
                        } else { None };

                        // Single batch of signal updates
                        ui.open_tabs.write().retain(|(id, _, _, _)| id != &cid);
                        if is_selected {
                            ui.selected_room_id.set(next);
                        }

                        // All cleanup in background
                        let cid = cid.clone();
                        let core2 = core2.clone();
                        spawn(async move {
                            let mut ui = consume_context::<UiState>();
                            ui.messages_by_room.write().remove(&cid);
                            ui.members_by_room.write().remove(&cid);
                            if let Ok(rid) = fluxchat_core::ruma::RoomId::parse(&cid) {
                                fluxchat_core::timeline::remove_timeline(&core2.timelines, &rid).await;
                            }
                        });
                    }
                },
                "×"
            }
        }
    }
}

#[component]
fn PollDisplay(poll: fluxchat_core::PollInfo, event_id: String, room_id: String, my_user_id: String) -> Element {
    let app_state = use_context::<Arc<AppState>>();

    // Determine which answer the current user voted for
    let my_vote = poll.answers.iter().find(|(aid, _)| {
        poll.votes.get(aid).map(|voters| voters.contains(&my_user_id)).unwrap_or(false)
    }).map(|(aid, _)| aid.clone());

    rsx! {
        div { style: "background: var(--bg-deep); border: 1px solid var(--border); border-radius: 6px; padding: 10px 14px; max-width: 400px; margin: 2px 0;",
            // Question
            div { style: "font-family: var(--font-mono); font-size: 13px; font-weight: 600; color: var(--text-bright); margin-bottom: 8px;",
                "\u{1f4ca} {poll.question}"
            }
            // Answers
            for (aid, atext) in poll.answers.iter() {
                { let aid = aid.clone();
                  let atext = atext.clone();
                  let vote_count = poll.votes.get(&aid).map(|v| v.len()).unwrap_or(0);
                  let pct = if poll.total_votes > 0 { (vote_count as f64 / poll.total_votes as f64 * 100.0) as u32 } else { 0 };
                  let is_my_vote = my_vote.as_ref() == Some(&aid);
                  let is_ended = poll.is_ended;
                  let border_color = if is_my_vote { "var(--accent)" } else { "var(--border)" };
                  let cursor = if is_ended { "default" } else { "pointer" };
                  let poll_eid = event_id.clone();
                  let poll_rid = room_id.clone();
                  let app = app_state.clone();
                rsx! {
                    div {
                        style: "position: relative; padding: 6px 10px; margin-bottom: 4px; border-radius: 4px; border: 1px solid {border_color}; cursor: {cursor}; overflow: hidden;",
                        onclick: move |_| {
                            if !is_ended {
                                let aid = aid.clone();
                                let eid = poll_eid.clone();
                                let rid = poll_rid.clone();
                                let app = app.clone();
                                spawn(async move {
                                    use matrix_sdk::ruma::events::poll::unstable_response::UnstablePollResponseEventContent;
                                    use matrix_sdk::ruma::events::AnyMessageLikeEventContent;
                                    let _client = match app.get_client().await { Ok(c) => c, Err(_) => return };
                                    if let Ok(rid_parsed) = matrix_sdk::ruma::RoomId::parse(&rid) {
                                        let timelines = app.timelines.lock().await;
                                        if let Some(timeline) = timelines.get(&*rid_parsed) {
                                            if let Ok(eid_parsed) = matrix_sdk::ruma::EventId::parse(&eid) {
                                                let content = UnstablePollResponseEventContent::new(
                                                    vec![aid],
                                                    eid_parsed,
                                                );
                                                let _ = timeline.send(AnyMessageLikeEventContent::UnstablePollResponse(content)).await;
                                            }
                                        }
                                    }
                                });
                            }
                        },
                        // Progress bar background
                        div { style: "position: absolute; top: 0; left: 0; bottom: 0; width: {pct}%; background: rgba(59, 130, 246, 0.1); transition: width 0.3s;" }
                        // Content
                        div { style: "position: relative; display: flex; justify-content: space-between; align-items: center;",
                            div { style: "display: flex; align-items: center; gap: 6px;",
                                if is_my_vote {
                                    span { style: "color: var(--accent); font-size: 12px;", "\u{25cf}" }
                                }
                                span { style: "font-family: var(--font-mono); font-size: 12px; color: var(--text-normal);", "{atext}" }
                            }
                            span { style: "font-family: var(--font-mono); font-size: 11px; color: var(--text-dim);",
                                { if poll.total_votes > 0 { format!("{vote_count} ({pct}%)") } else { format!("{vote_count}") } }
                            }
                        }
                    }
                } }
            }
            // Footer: total votes + ended status
            div { style: "font-family: var(--font-mono); font-size: 11px; color: var(--text-dim); margin-top: 4px;",
                if poll.is_ended {
                    "Poll ended \u{00b7} {poll.total_votes} votes"
                } else {
                    "{poll.total_votes} votes"
                }
            }
        }
    }
}

#[component]
fn InlineImage(room_id: String, event_id: String, body: String, image_mxc: Option<String>, image_width: Option<u32>, image_height: Option<u32>) -> Element {
    let app_state = use_context::<Arc<AppState>>();
    let mut ui = use_context::<UiState>();

    // Compute display size (constrain to 400x300 while maintaining aspect ratio)
    let (display_w, display_h) = match (image_width, image_height) {
        (Some(w), Some(h)) if w > 0 && h > 0 => {
            let scale = f64::min(400.0 / w as f64, 300.0 / h as f64).min(1.0);
            ((w as f64 * scale) as u32, (h as f64 * scale) as u32)
        }
        _ => (0, 0),
    };

    // Clone for use in onclick (use_resource moves the originals)
    let room_id_click = room_id.clone();
    let event_id_click = event_id.clone();

    // Load image via event (handles both encrypted and unencrypted media)
    let thumbnail_data = use_resource(move || {
        let app_state = app_state.clone();
        let room_id = room_id.clone();
        let event_id = event_id.clone();
        async move {
            let client = app_state.get_client().await.ok()?;
            let real_room_id = if room_id.starts_with("thread::") {
                room_id.split("::").nth(1).unwrap_or(&room_id).to_string()
            } else {
                room_id.clone()
            };
            let data = fluxchat_core::media::get_event_media(
                &client, &app_state.db, &app_state.media_cache, &real_room_id, &event_id
            ).await.ok()?;
            Some((data, true))
        }
    });

    let resolved = thumbnail_data.read().clone();
    let size_style = if display_w > 0 && display_h > 0 {
        format!("width: {}px; height: {}px;", display_w, display_h)
    } else {
        String::new()
    };

    match resolved {
        Some(Some((data_uri, is_full))) if data_uri.starts_with("data:image/") => {
            let uri_for_click = data_uri.clone();
            let is_full_res = is_full;
            rsx! {
                div { style: "display: flex; flex-direction: column;",
                    img {
                        src: "{data_uri}",
                        alt: "{body}",
                        style: "max-width: 400px; max-height: 300px; {size_style} border-radius: 4px; margin: 2px 0; object-fit: contain; cursor: pointer;",
                        onclick: move |_| {
                            if is_full_res {
                                // Already have full res
                                ui.lightbox_url.set(Some(uri_for_click.clone()));
                            } else {
                                // Load full-res for lightbox
                                let uri = uri_for_click.clone();
                                let app = consume_context::<Arc<AppState>>();
                                let room_id = room_id_click.clone();
                                let event_id = event_id_click.clone();
                                spawn(async move {
                                    let real_room_id = if room_id.starts_with("thread::") {
                                        room_id.split("::").nth(1).unwrap_or(&room_id).to_string()
                                    } else { room_id.clone() };
                                    let client = match app.get_client().await { Ok(c) => c, Err(_) => { consume_context::<UiState>().lightbox_url.set(Some(uri)); return; } };
                                    match fluxchat_core::media::get_event_media(&client, &app.db, &app.media_cache, &real_room_id, &event_id).await {
                                        Ok(full) => consume_context::<UiState>().lightbox_url.set(Some(full)),
                                        Err(_) => consume_context::<UiState>().lightbox_url.set(Some(uri)),
                                    }
                                });
                            }
                        },
                    }
                }
            }
        },
        Some(Some(_)) | Some(None) => rsx! {
            span { class: "msg-body", style: "color: var(--text-dim);", "[image: {body}]" }
        },
        None => {
            let placeholder_style = if size_style.is_empty() {
                "width: 200px; height: 150px;".to_string()
            } else {
                size_style.clone()
            };
            rsx! {
                div { style: "max-width: 400px; max-height: 300px; {placeholder_style} border-radius: 4px; margin: 2px 0; background: var(--bg-hover); display: flex; align-items: center; justify-content: center; color: var(--text-dim); font-size: 12px;",
                    "[loading {body}...]"
                }
            }
        },
    }
}

/// Inline video player for m.video messages.
/// Downloads to disk cache, served via local HTTP server (data URIs too large for video).
#[component]
fn InlineVideo(room_id: String, event_id: String, body: String, image_width: Option<u32>, image_height: Option<u32>) -> Element {
    let app_state = use_context::<Arc<AppState>>();

    let (display_w, display_h) = match (image_width, image_height) {
        (Some(w), Some(h)) if w > 0 && h > 0 => {
            let scale = f64::min(400.0 / w as f64, 300.0 / h as f64).min(1.0);
            ((w as f64 * scale) as u32, (h as f64 * scale) as u32)
        }
        _ => (320, 240),
    };

    let video_data = use_resource(move || {
        let app_state = app_state.clone();
        let room_id = room_id.clone();
        let event_id = event_id.clone();
        async move {
            let safe_id: String = event_id.chars().filter(|c| c.is_alphanumeric()).collect();
            let cache_dir = std::env::temp_dir().join("fluxchat_video");

            // Check disk cache first
            if let Ok(entries) = std::fs::read_dir(&cache_dir) {
                for entry in entries.flatten() {
                    let name = entry.file_name().to_string_lossy().to_string();
                    if name.starts_with(&safe_id) {
                        return Some(format!("{}/{}", crate::media_server::base_url(), name));
                    }
                }
            }

            // Download from Matrix server
            let client = app_state.get_client().await.ok()?;
            let real_room_id = if room_id.starts_with("thread::") {
                room_id.split("::").nth(1).unwrap_or(&room_id).to_string()
            } else {
                room_id.clone()
            };
            let (bytes, mime) = fluxchat_core::media::get_event_media_bytes(
                &client, &real_room_id, &event_id
            ).await.ok()?;

            let ext = match mime.as_str() {
                "video/webm" => "webm",
                "video/quicktime" => "mov",
                _ => "mp4",
            };
            let _ = std::fs::create_dir_all(&cache_dir);
            let filename = format!("{safe_id}.{ext}");
            let _ = std::fs::write(cache_dir.join(&filename), &bytes);
            Some(format!("{}/{}", crate::media_server::base_url(), filename))
        }
    });

    let resolved = video_data.read().clone();

    match resolved {
        Some(Some(url)) => {
            rsx! {
                div { style: "display: flex; flex-direction: column;",
                    video {
                        src: "{url}",
                        controls: true,
                        preload: "metadata",
                        style: "max-width: 400px; max-height: 300px; width: {display_w}px; height: {display_h}px; border-radius: 4px; margin: 2px 0; object-fit: contain;",
                    }
                }
            }
        },
        Some(None) => rsx! {
            span { class: "msg-body", style: "color: var(--text-dim);", "[video: {body}]" }
        },
        None => {
            let placeholder_style = format!("width: {display_w}px; height: {display_h}px;");
            rsx! {
                div { style: "max-width: 400px; max-height: 300px; {placeholder_style} border-radius: 4px; margin: 2px 0; background: var(--bg-hover); display: flex; align-items: center; justify-content: center; color: var(--text-dim); font-size: 12px;",
                    "[loading video...]"
                }
            }
        },
    }
}

/// Pinned messages HUD — drops down from the tab bar.
#[component]
/// Reusable HUD panel wrapper — consistent chrome for all dropdown panels.
#[component]
fn HudPanel(title: String, count: String, on_close: EventHandler<()>, children: Element) -> Element {
    rsx! {
        div {
            class: "search-hud",
            style: "flex-direction: column; align-items: stretch; max-height: 300px; overflow-y: auto;",
            tabindex: "0",
            onmounted: move |e| { std::mem::drop(e.data().set_focus(true)); },
            onkeydown: move |e: Event<KeyboardData>| {
                if e.key() == Key::Escape { on_close.call(()); }
            },
            div { style: "display: flex; align-items: center; justify-content: space-between; margin-bottom: 4px;",
                span { style: "font-family: var(--font-mono); font-size: 11px; font-weight: 700; text-transform: uppercase; color: var(--text-dim); letter-spacing: 0.5px;",
                    "{title} — {count}"
                }
                span { style: "color: var(--text-dim); cursor: pointer; font-size: 14px; padding: 0 2px;",
                    onclick: move |_| { on_close.call(()); },
                    "×"
                }
            }
            {children}
        }
    }
}

#[component]
fn PinsHud() -> Element {
    let mut ui = use_context::<UiState>();
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
    let count = match &resolved { Some(msgs) => msgs.len().to_string(), None => "...".to_string() };

    rsx! {
        HudPanel { title: "pinned".to_string(), count: count, on_close: move |_| { ui.pins_open.set(false); },
            match resolved {
                Some(msgs) if msgs.is_empty() => rsx! {
                    div { style: "color: var(--text-dim); font-size: 12px; font-family: var(--font-mono); padding: 8px 0;",
                        "no pinned messages"
                    }
                },
                Some(msgs) => rsx! {
                    for msg in msgs.iter() {
                        div { style: "display: flex; gap: 8px; padding: 4px 0; border-top: 1px solid var(--border); font-family: var(--font-mono); font-size: 12px;",
                            span { style: "color: var(--text-muted); flex-shrink: 0; width: 80px; text-align: right; overflow: hidden; text-overflow: ellipsis; white-space: nowrap;",
                                "{msg.sender_name}"
                            }
                            span { style: "color: var(--text-normal); overflow: hidden; text-overflow: ellipsis; white-space: nowrap;",
                                "{msg.body}"
                            }
                        }
                    }
                },
                None => rsx! {
                    div { style: "color: var(--text-dim); font-size: 12px; font-family: var(--font-mono); padding: 8px 0;",
                        "loading..."
                    }
                },
            }
        }
    }
}

/// Threads HUD — dropdown listing threads in the current room.
#[component]
fn ThreadsHud() -> Element {
    let mut ui = use_context::<UiState>();
    let selected = ui.selected_room_id.read().clone();

    // Derive thread roots: (eid, sender, body, count, last_reply_sender, last_reply_body)
    let threads: Vec<(String, String, String, u32, String, String)> = if let Some(ref room_id) = selected {
        let msgs = ui.messages_by_room.read();
        if let Some(messages) = msgs.get(room_id) {
            let mut counts: std::collections::HashMap<String, u32> = std::collections::HashMap::new();
            let mut last_reply: std::collections::HashMap<String, (String, String)> = std::collections::HashMap::new();
            for m in messages.iter() {
                if let Some(ref root) = m.thread_root {
                    *counts.entry(root.clone()).or_insert(0) += 1;
                    last_reply.insert(root.clone(), (m.sender_name.clone(), m.body.clone()));
                }
            }
            counts.iter()
                .filter_map(|(root_eid, count)| {
                    messages.iter()
                        .find(|m| m.event_id == *root_eid)
                        .map(|m| {
                            let (lr_sender, lr_body) = last_reply.get(root_eid).cloned().unwrap_or_default();
                            (m.event_id.clone(), m.sender_name.clone(), m.body.clone(), *count, lr_sender, lr_body)
                        })
                })
                .collect()
        } else { Vec::new() }
    } else { Vec::new() };

    let total = threads.len();

    rsx! {
        HudPanel { title: "threads".to_string(), count: total.to_string(), on_close: move |_| { ui.threads_open.set(false); },
            if threads.is_empty() {
                div { style: "color: var(--text-dim); font-size: 12px; font-family: var(--font-mono); padding: 8px 0;",
                    "no threads in this room"
                }
            }
            if total > 50 {
                div { style: "font-size: 10px; color: var(--text-dim); font-family: var(--font-mono); padding: 2px 0;",
                    { format!("showing 50 of {}", total) }
                }
            }
            for (eid, sender, body, count, lr_sender, lr_body) in threads.iter().take(50) {
                { let eid = eid.clone();
                  let body = body.clone();
                  let room_id = selected.clone();
                  let root_preview = if body.len() > 60 { format!("{}...", &body[..60]) } else { body.clone() };
                  let reply_preview = if lr_body.len() > 50 { format!("{}...", &lr_body[..50]) } else { lr_body.clone() };
                  let replies = if *count == 1 { "1 reply".to_string() } else { format!("{count} replies") };
                  rsx! {
                    div {
                        style: "padding: 8px; border-top: 1px solid var(--border); font-family: var(--font-mono); cursor: pointer;",
                        class: "cmd-item",
                        onclick: move |_| {
                            if let Some(ref rid) = room_id {
                                let ui = consume_context::<UiState>();
                                crate::components::layout::open_thread_tab(ui, rid.clone(), eid.clone(), body.clone());
                                consume_context::<UiState>().threads_open.set(false);
                            }
                        },
                        // Root message
                        div { style: "display: flex; align-items: center; gap: 6px; margin-bottom: 4px;",
                            span { style: "color: var(--accent-muted); display: inline-flex; flex-shrink: 0;",
                                dangerous_inner_html: ICO_THREAD,
                            }
                            span { style: "font-size: 12px; font-weight: 600; color: var(--text-bright); flex-shrink: 0;",
                                "{sender}"
                            }
                            span { style: "font-size: 12px; color: var(--text-muted); overflow: hidden; text-overflow: ellipsis; white-space: nowrap; flex: 1;",
                                "{root_preview}"
                            }
                        }
                        // Last reply + count
                        div { style: "display: flex; align-items: center; gap: 6px; padding-left: 20px;",
                            span { style: "font-size: 11px; color: var(--accent-muted); flex-shrink: 0;",
                                "{replies}"
                            }
                            if !lr_sender.is_empty() {
                                span { style: "font-size: 11px; color: var(--text-dim);", "·" }
                                span { style: "font-size: 11px; color: var(--text-dim); overflow: hidden; text-overflow: ellipsis; white-space: nowrap; flex: 1;",
                                    "last: {lr_sender}: {reply_preview}"
                                }
                            }
                        }
                    }
                  }
                }
            }
        }
    }
}

/// Files HUD — dropdown listing files/media shared in the current room.
#[component]
fn FilesHud() -> Element {
    let mut ui = use_context::<UiState>();
    let selected = ui.selected_room_id.read().clone();

    let app_state = use_context::<Arc<AppState>>();

    // Collect files from loaded messages (messages with image_mxc set)
    let files: Vec<(String, String, String, String, u64)> = selected.as_ref().map(|room_id| {
        ui.messages_by_room.read()
            .get(room_id)
            .map(|msgs| msgs.iter()
                .filter(|m| m.image_mxc.is_some())
                .map(|m| (m.event_id.clone(), m.sender_name.clone(), m.body.clone(), m.image_mxc.clone().unwrap_or_default(), m.timestamp))
                .collect())
            .unwrap_or_default()
    }).unwrap_or_default();

    let total = files.len();

    rsx! {
        HudPanel { title: "files".to_string(), count: total.to_string(), on_close: move |_| { ui.files_open.set(false); },
            if files.is_empty() {
                div { style: "color: var(--text-dim); font-size: 12px; font-family: var(--font-mono); padding: 8px 0;",
                    "no files in loaded messages"
                }
            }
            if total > 50 {
                div { style: "font-size: 10px; color: var(--text-dim); font-family: var(--font-mono); padding: 2px 0;",
                    { format!("showing 50 of {}", total) }
                }
            }
            for (eid, sender, body, mxc, ts) in files.iter().rev().take(50) {
                { let eid_scroll = eid.clone();
                  let nick_class = nick_color_class(sender);
                  let time = {
                      let secs = *ts % 86400;
                      let h = secs / 3600;
                      let m = (secs % 3600) / 60;
                      format!("{:02}:{:02}", h, m)
                  };
                  // Resolve thumbnail from media cache
                  let thumb = {
                      let cache = app_state.media_cache.lock().unwrap();
                      cache.get(mxc).cloned().or_else(|| cache.get(&format!("event:{}", eid)).cloned())
                  };
                rsx! {
                    div {
                        style: "display: flex; gap: 8px; padding: 6px 0; border-top: 1px solid var(--border); font-family: var(--font-mono); font-size: 12px; cursor: pointer; align-items: center;",
                        class: "cmd-item",
                        onclick: move |_| {
                            let eid = eid_scroll.clone();
                            spawn(async move {
                                let js = format!(r#"
                                    document.querySelectorAll('.search-highlight').forEach(el => el.classList.remove('search-highlight'));
                                    let el = document.querySelector('[data-event-id="{}"]');
                                    if (el) {{
                                        el.classList.add('search-highlight');
                                        el.scrollIntoView({{ behavior: 'smooth', block: 'center' }});
                                    }}
                                "#, js_escape(&eid));
                                let _ = document::eval(&js).await;
                            });
                        },
                        if let Some(ref src) = thumb {
                            img { src: "{src}", style: "width: 36px; height: 36px; border-radius: 4px; object-fit: cover; flex-shrink: 0;" }
                        } else {
                            div { style: "width: 36px; height: 36px; border-radius: 4px; background: var(--bg-hover); display: flex; align-items: center; justify-content: center; flex-shrink: 0; opacity: 0.5;",
                                dangerous_inner_html: ICO_FILE,
                            }
                        }
                        div { style: "min-width: 0; flex: 1; overflow: hidden;",
                            div { style: "display: flex; gap: 6px; align-items: baseline;",
                                span { class: "{nick_class}", style: "font-size: 11px; font-weight: 600; flex-shrink: 0;",
                                    "{sender}"
                                }
                                span { style: "font-size: 10px; color: var(--text-dim); flex-shrink: 0;",
                                    "{time}"
                                }
                            }
                            div { style: "font-size: 11px; color: var(--text-muted); overflow: hidden; text-overflow: ellipsis; white-space: nowrap;",
                                "{body}"
                            }
                        }
                    }
                } }
            }
        }
    }
}

/// Notices HUD — system/state messages aggregated in one panel.
#[component]
fn NoticesHud() -> Element {
    let mut ui = use_context::<UiState>();
    let selected = ui.selected_room_id.read().clone();

    let notices: Vec<(String, String)> = selected.as_ref().map(|room_id| {
        ui.messages_by_room.read()
            .get(room_id)
            .map(|msgs| msgs.iter()
                .filter(|m| m.event_id.starts_with("~state-") || m.event_id.starts_with("~system-"))
                .map(|m| {
                    let time = {
                        let secs = m.timestamp % 86400;
                        let h = secs / 3600;
                        let min = (secs % 3600) / 60;
                        format!("{:02}:{:02}", h, min)
                    };
                    (time, m.body.clone())
                })
                .collect())
            .unwrap_or_default()
    }).unwrap_or_default();

    let total = notices.len();

    rsx! {
        HudPanel { title: "notices".to_string(), count: total.to_string(), on_close: move |_| { ui.notices_open.set(false); },
            if notices.is_empty() {
                div { style: "color: var(--text-dim); font-size: 12px; font-family: var(--font-mono); padding: 8px 0;",
                    "no system messages"
                }
            }
            if total > 50 {
                div { style: "font-size: 10px; color: var(--text-dim); font-family: var(--font-mono); padding: 2px 0;",
                    { format!("showing 50 of {}", total) }
                }
            }
            for (time, body) in notices.iter().rev().take(50) {
                div { style: "display: flex; gap: 8px; padding: 3px 0; border-top: 1px solid var(--border); font-family: var(--font-mono); font-size: 11px;",
                    span { style: "color: var(--text-dim); flex-shrink: 0;", "{time}" }
                    span { style: "color: var(--text-muted);", "{body}" }
                }
            }
        }
    }
}

/// GIF picker HUD — search Tenor and click to send.
#[component]
fn GifsHud() -> Element {
    let mut ui = use_context::<UiState>();
    let app_state = use_context::<Arc<AppState>>();
    let initial_query = ui.gif_query.peek().clone();
    let mut query = use_signal(move || initial_query);
    let results = use_signal(Vec::<fluxchat_core::gif::GifResult>::new);
    let loading = use_signal(|| false);
    let error_msg = use_signal(|| Option::<String>::None);
    let sending = use_signal(|| false);

    let selected = ui.selected_room_id.read().clone();
    let send_room_id = selected.as_ref().map(|id| {
        if id.starts_with("thread::") { id.split("::").nth(1).unwrap_or(id).to_string() } else { id.clone() }
    });

    // Shared search logic
    let do_search = move |q: String| {
        if q.is_empty() { return; }
        let api_key = ui.giphy_api_key.peek().clone();
        let mut loading = loading;
        let mut results = results;
        let mut error_msg = error_msg;
        spawn(async move {
            loading.set(true);
            error_msg.set(None);
            let key_ref = if api_key.is_empty() { None } else { Some(api_key.as_str()) };
            match fluxchat_core::gif::search_gifs(&q, key_ref).await {
                Ok(gifs) => results.set(gifs),
                Err(e) => error_msg.set(Some(format!("{e}"))),
            }
            loading.set(false);
        });
    };

    // Auto-search on mount if query is pre-filled
    let mut auto_searched = use_signal(|| false);
    if !*auto_searched.peek() && !query.peek().is_empty() {
        auto_searched.set(true);
        do_search(query.peek().clone());
    }

    let result_count = results.read().len();
    let count_str = if *loading.read() { "...".to_string() } else { result_count.to_string() };

    rsx! {
        div {
            class: "search-hud",
            style: "flex-direction: column; align-items: stretch; max-height: 50vh; overflow-y: auto;",
            tabindex: "0",
            onmounted: move |e| { std::mem::drop(e.data().set_focus(true)); },
            onkeydown: move |e: Event<KeyboardData>| {
                if e.key() == Key::Escape { ui.gifs_open.set(false); }
            },
            div { style: "display: flex; align-items: center; justify-content: space-between; margin-bottom: 4px;",
                span { style: "font-family: var(--font-mono); font-size: 11px; font-weight: 700; text-transform: uppercase; color: var(--text-dim); letter-spacing: 0.5px;",
                    "gif search — {count_str}"
                }
                span { style: "color: var(--text-dim); cursor: pointer; font-size: 14px; padding: 0 2px;",
                    onclick: move |_| { ui.gifs_open.set(false); },
                    "×"
                }
            }
            // Search input
            div { style: "display: flex; gap: 6px; margin-bottom: 6px;",
                input {
                    r#type: "text",
                    placeholder: "Search GIFs...",
                    value: "{query}",
                    style: "flex: 1; background: var(--bg-input); color: var(--text-normal); border: 1px solid var(--border); border-radius: 3px; padding: 4px 8px; font-family: var(--font-mono); font-size: 12px; outline: none;",
                    oninput: move |e: Event<FormData>| { query.set(e.value()); },
                    onkeydown: move |e: Event<KeyboardData>| {
                        if e.key() == Key::Enter { do_search(query.peek().trim().to_string()); }
                    },
                }
                button {
                    style: "background: var(--bg-input); color: var(--text-normal); border: 1px solid var(--border); border-radius: 3px; padding: 4px 10px; font-family: var(--font-mono); font-size: 12px; cursor: pointer;",
                    onclick: move |_| { do_search(query.peek().trim().to_string()); },
                    "Search"
                }
            }
            // Error
            if let Some(err) = error_msg.read().as_ref() {
                div { style: "color: #e06060; font-size: 11px; font-family: var(--font-mono); padding: 4px 0;",
                    "{err}"
                }
            }
            // Sending indicator
            if *sending.read() {
                div { style: "color: var(--text-dim); font-size: 11px; font-family: var(--font-mono); padding: 4px 0;",
                    "Sending GIF..."
                }
            }
            // Results grid
            if result_count > 0 {
                div { style: "display: grid; grid-template-columns: repeat(3, 1fr); gap: 6px; padding: 4px 0;",
                    for gif in results.read().iter().take(12) {
                        {
                            let gif_url = gif.url.clone();
                            let gif_desc = gif.description.clone();
                            let preview = gif.preview_url.clone();
                            let room_id = send_room_id.clone();
                            let app_state = app_state.clone();
                            let mut sending = sending;
                            let mut ui_close = ui.clone();
                            rsx! {
                                img {
                                    src: "{preview}",
                                    alt: "{gif_desc}",
                                    title: "{gif_desc}",
                                    style: "width: 100%; aspect-ratio: 4/3; object-fit: contain; border-radius: 4px; cursor: pointer; border: 1px solid var(--border); background: #111;",
                                    onclick: move |_| {
                                        let gif_url = gif_url.clone();
                                        let room_id = room_id.clone();
                                        let app_state = app_state.clone();
                                        spawn(async move {
                                            let Some(room_id) = room_id else { return };
                                            sending.set(true);
                                            match fluxchat_core::gif::download_gif(&gif_url).await {
                                                Ok(data) => {
                                                    let client = match app_state.get_client().await {
                                                        Ok(c) => c,
                                                        Err(_) => { sending.set(false); return; }
                                                    };
                                                    let _ = fluxchat_core::messages::upload_file(
                                                        &client, &room_id, "tenor.gif", "image/gif", data
                                                    ).await;
                                                    sending.set(false);
                                                    ui_close.gifs_open.set(false);
                                                }
                                                Err(e) => {
                                                    tracing::warn!("GIF download failed: {e}");
                                                    sending.set(false);
                                                }
                                            }
                                        });
                                    },
                                }
                            }
                        }
                    }
                }
            } else if !*loading.read() && result_count == 0 && *auto_searched.read() {
                div { style: "color: var(--text-dim); font-size: 12px; font-family: var(--font-mono); padding: 8px 0;",
                    "no results"
                }
            }
            // Giphy attribution
            div { style: "font-size: 9px; color: var(--text-dim); font-family: var(--font-mono); text-align: right; padding-top: 4px;",
                "Powered by GIPHY"
            }
        }
    }
}

/// Search HUD — server-side message search.
#[component]
fn SearchHud() -> Element {
    let mut ui = use_context::<UiState>();
    let app_state = use_context::<Arc<AppState>>();
    let mut query = use_signal(String::new);
    let mut results = use_signal(Vec::<(String, String, String)>::new); // (event_id, sender, body)
    let mut current_idx = use_signal(|| 0usize);
    let mut searching = use_signal(|| false);

    let selected = ui.selected_room_id.read().clone();
    // Resolve thread IDs to real room IDs for search
    let search_room_id = selected.as_ref().map(|id| {
        if id.starts_with("thread::") { id.split("::").nth(1).unwrap_or(id).to_string() } else { id.clone() }
    });

    let result_count = results.read().len();
    let idx = *current_idx.read();

    // Scroll to and highlight current search result
    use_effect(move || {
        let results = results.read();
        let idx = *current_idx.read();
        if let Some((ref event_id, _, _)) = results.get(idx) {
            let eid = event_id.clone();
            spawn(async move {
                let js = format!(r#"
                    document.querySelectorAll('.search-highlight').forEach(el => el.classList.remove('search-highlight'));
                    let el = document.querySelector('[data-event-id="{}"]');
                    if (el) {{
                        el.classList.add('search-highlight');
                        el.scrollIntoView({{ behavior: 'smooth', block: 'center' }});
                    }}
                "#, js_escape(&eid));
                let _ = document::eval(&js).await;
            });
        }
    });

    // Trigger search helper
    let do_search = {
        let app_state = app_state.clone();
        let search_room_id = search_room_id.clone();
        let selected_raw = selected.clone();
        move |_: ()| {
            let q = query().trim().to_string();
            if q.is_empty() { results.set(Vec::new()); return; }
            if let Some(ref room_id) = search_room_id {
                let app_state = app_state.clone();
                let room_id = room_id.clone();
                let selected_key = selected_raw.clone().unwrap_or_default();
                let q2 = q.clone();
                searching.set(true);
                spawn(async move {
                    let client = match app_state.get_client().await { Ok(c) => c, Err(_) => { searching.set(false); return; } };
                    match fluxchat_core::messages::search_messages(&client, &room_id, &q, None).await {
                        Ok((search_results, _)) => {
                            let matches: Vec<_> = search_results.iter()
                                .map(|r| (r.event_id.clone(), r.sender_name.clone(), r.body.clone()))
                                .collect();
                            current_idx.set(0);
                            results.set(matches);
                        }
                        Err(_) => {
                            let ui = consume_context::<UiState>();
                            let matches: Vec<_> = ui.messages_by_room.read()
                                .get(&selected_key)
                                .map(|msgs| msgs.iter()
                                    .filter(|m| m.body.to_lowercase().contains(&q2.to_lowercase()))
                                    .map(|m| (m.event_id.clone(), m.sender_name.clone(), m.body.clone()))
                                    .collect())
                                .unwrap_or_default();
                            current_idx.set(0);
                            results.set(matches);
                        }
                    }
                    searching.set(false);
                });
            }
        }
    };

    rsx! {
        div { style: "position: relative;",
            div { class: "search-hud",
                span { style: "color: var(--text-dim); display: inline-flex;", dangerous_inner_html: r#"<svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><circle cx="11" cy="11" r="8"/><path d="m21 21-4.3-4.3"/></svg>"# }
                input {
                    r#type: "text",
                    placeholder: "search messages...",
                    autofocus: true,
                    value: "{query}",
                    oninput: move |e| {
                        query.set(e.value());
                    },
                    onkeydown: {
                        let mut ui = ui.clone();
                        let mut do_search = do_search.clone();
                        move |e: Event<KeyboardData>| {
                            if e.key() == Key::Escape {
                                if result_count > 0 {
                                    results.set(Vec::new());
                                } else {
                                    ui.search_open.set(false);
                                }
                            } else if e.key() == Key::Enter {
                                if result_count > 0 {
                                    // Cycle to next result
                                    let new_idx = if idx + 1 < result_count { idx + 1 } else { 0 };
                                    current_idx.set(new_idx);
                                } else {
                                    do_search(());
                                }
                            } else if e.key() == Key::ArrowDown && result_count > 0 {
                                let new_idx = if idx + 1 < result_count { idx + 1 } else { 0 };
                                current_idx.set(new_idx);
                            } else if e.key() == Key::ArrowUp && result_count > 0 {
                                let new_idx = if idx > 0 { idx - 1 } else { result_count - 1 };
                                current_idx.set(new_idx);
                            }
                        }
                    },
                }
                if *searching.read() {
                    span { class: "search-count", style: "animation: pulse 1s ease-in-out infinite;", "searching..." }
                } else if result_count > 0 {
                    span { class: "search-count", "{result_count} results" }
                } else if !query().is_empty() {
                    span { class: "search-count", "enter to search" }
                }
                span { style: "color: var(--text-dim); cursor: pointer; font-size: 14px; padding: 0 2px;",
                    onclick: move |_| { ui.search_open.set(false); },
                    "×"
                }
            }
            // Results dropdown
            if result_count > 0 {
                div { style: "position: absolute; top: 100%; left: 0; right: 0; background: var(--bg-dark); border: 1px solid var(--border); border-top: none; max-height: 300px; overflow-y: auto; z-index: 50;",
                    for (i, (eid, sender, body)) in results.read().iter().enumerate() {
                        { let _eid = eid.clone();
                          let is_active = i == idx;
                          let bg = if is_active { "var(--bg-light)" } else { "transparent" };
                          let nick_class = nick_color_class(sender);
                          let preview: String = body.chars().take(120).collect();
                        rsx! {
                            div {
                                style: "display: flex; gap: 8px; padding: 6px 12px; cursor: pointer; background: {bg}; border-bottom: 1px solid var(--border);",
                                class: "cmd-item",
                                onclick: move |_| {
                                    current_idx.set(i);
                                },
                                span { class: "{nick_class}", style: "font-family: var(--font-mono); font-size: 12px; font-weight: 600; flex-shrink: 0; min-width: 80px; text-align: right; overflow: hidden; text-overflow: ellipsis; white-space: nowrap;",
                                    "{sender}"
                                }
                                span { style: "font-family: var(--font-mono); font-size: 12px; color: var(--text-muted); overflow: hidden; text-overflow: ellipsis; white-space: nowrap; min-width: 0;",
                                    "{preview}"
                                }
                            }
                        } }
                    }
                }
            }
        }
    }
}

const ICO_MORE: &str = r#"<svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><circle cx="12" cy="12" r="1"/><circle cx="12" cy="5" r="1"/><circle cx="12" cy="19" r="1"/></svg>"#;

/// Room action icons in the tab bar header.
#[component]
fn RoomActions() -> Element {
    let mut ui = use_context::<UiState>();
    let app_state = consume_context::<Arc<AppState>>();
    let members_visible = *ui.show_member_list.read();
    let mut menu_open = use_signal(|| false);

    let selected_rid = ui.selected_room_id.read().clone();
    let is_fav = selected_rid.as_ref().map(|id| ui.rooms.read().iter().any(|r| r.room_id == *id && r.is_favourite)).unwrap_or(false);

    rsx! {
        div { style: "display: flex; align-items: center; gap: 1px; padding: 0 8px; flex-shrink: 0;",
            // Search
            span {
                class: if *ui.search_open.read() { "room-action active" } else { "room-action" },
                title: "Search",
                onclick: move |_| { let open = *ui.search_open.peek(); ui.search_open.set(!open); },
                dangerous_inner_html: ICO_SEARCH,
            }
            // Toggle member list
            span {
                class: if members_visible { "room-action active" } else { "room-action" },
                title: "Toggle member list",
                onclick: move |_| { ui.show_member_list.set(!members_visible); },
                dangerous_inner_html: ICO_MEMBERS,
            }
            // Room info
            span {
                class: if *ui.room_settings_open.read() { "room-action active" } else { "room-action" },
                title: "Room info",
                onclick: move |_| { let v = *ui.room_settings_open.peek(); ui.room_settings_open.set(!v); },
                dangerous_inner_html: ICO_INFO,
            }
            // Overflow menu
            div { style: "position: relative;",
                span {
                    class: if *menu_open.read() { "room-action active" } else { "room-action" },
                    title: "More",
                    onclick: move |_| { let v = *menu_open.peek(); menu_open.set(!v); },
                    dangerous_inner_html: ICO_MORE,
                }
                if *menu_open.read() {
                    // Backdrop to close menu
                    div {
                        style: "position: fixed; inset: 0; z-index: 49;",
                        onclick: move |_| { menu_open.set(false); },
                    }
                    div { style: "position: absolute; right: 0; top: 100%; margin-top: 4px; background: var(--bg-dark); border: 1px solid var(--border); border-radius: 6px; min-width: 180px; z-index: 50; padding: 4px 0; box-shadow: 0 4px 12px rgba(0,0,0,0.3);",
                        // Favourite
                        div {
                            class: "cmd-item",
                            style: "display: flex; align-items: center; gap: 8px; padding: 6px 12px; font-family: var(--font-mono); font-size: 12px; color: var(--text-normal); cursor: pointer;",
                            onclick: move |_| {
                                menu_open.set(false);
                                let rid = ui.selected_room_id.peek().clone();
                                if let Some(rid) = rid {
                                    let app = app_state.clone();
                                    let new_fav = !is_fav;
                                    spawn(async move {
                                        let client = match app.get_client().await { Ok(c) => c, Err(_) => return };
                                        let _ = fluxchat_core::rooms::set_room_favourite(&client, &rid, new_fav).await;
                                    });
                                }
                            },
                            span { style: "display: inline-flex; opacity: 0.6;", dangerous_inner_html: ICO_STAR }
                            if is_fav { "Remove favourite" } else { "Favourite" }
                        }
                        // Copy link
                        div {
                            class: "cmd-item",
                            style: "display: flex; align-items: center; gap: 8px; padding: 6px 12px; font-family: var(--font-mono); font-size: 12px; color: var(--text-normal); cursor: pointer;",
                            onclick: move |_| {
                                menu_open.set(false);
                                if let Some(ref rid) = *ui.selected_room_id.peek() {
                                    let link = format!("https://matrix.to/#/{}", rid);
                                    spawn(async move {
                                        let js = format!("navigator.clipboard.writeText('{}');", js_escape(&link));
                                        let _ = document::eval(&js).await;
                                    });
                                }
                            },
                            span { style: "display: inline-flex; opacity: 0.6;", dangerous_inner_html: ICO_LINK }
                            "Copy link"
                        }
                        div { style: "height: 1px; background: var(--border); margin: 4px 0;" }
                        // Pins
                        div {
                            class: "cmd-item",
                            style: "display: flex; align-items: center; gap: 8px; padding: 6px 12px; font-family: var(--font-mono); font-size: 12px; color: var(--text-normal); cursor: pointer;",
                            onclick: move |_| { menu_open.set(false); let open = *ui.pins_open.peek(); ui.pins_open.set(!open); },
                            span { style: "display: inline-flex; opacity: 0.6;", dangerous_inner_html: ICO_PIN }
                            "Pinned messages"
                        }
                        // Threads
                        div {
                            class: "cmd-item",
                            style: "display: flex; align-items: center; gap: 8px; padding: 6px 12px; font-family: var(--font-mono); font-size: 12px; color: var(--text-normal); cursor: pointer;",
                            onclick: move |_| { menu_open.set(false); let open = *ui.threads_open.peek(); ui.threads_open.set(!open); },
                            span { style: "display: inline-flex; opacity: 0.6;", dangerous_inner_html: ICO_THREAD }
                            "Threads"
                        }
                        // Files
                        div {
                            class: "cmd-item",
                            style: "display: flex; align-items: center; gap: 8px; padding: 6px 12px; font-family: var(--font-mono); font-size: 12px; color: var(--text-normal); cursor: pointer;",
                            onclick: move |_| { menu_open.set(false); let open = *ui.files_open.peek(); ui.files_open.set(!open); },
                            span { style: "display: inline-flex; opacity: 0.6;", dangerous_inner_html: ICO_FILE }
                            "Files"
                        }
                        // GIFs
                        div {
                            class: "cmd-item",
                            style: "display: flex; align-items: center; gap: 8px; padding: 6px 12px; font-family: var(--font-mono); font-size: 12px; color: var(--text-normal); cursor: pointer;",
                            onclick: move |_| { menu_open.set(false); ui.gif_query.set(String::new()); let open = *ui.gifs_open.peek(); ui.gifs_open.set(!open); },
                            span { style: "display: inline-flex; opacity: 0.6;", dangerous_inner_html: ICO_GIF }
                            "GIFs"
                        }
                        // Notices
                        div {
                            class: "cmd-item",
                            style: "display: flex; align-items: center; gap: 8px; padding: 6px 12px; font-family: var(--font-mono); font-size: 12px; color: var(--text-normal); cursor: pointer;",
                            onclick: move |_| { menu_open.set(false); let open = *ui.notices_open.peek(); ui.notices_open.set(!open); },
                            span { style: "display: inline-flex; opacity: 0.6;", dangerous_inner_html: r#"<svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><path d="M18 8A6 6 0 0 0 6 8c0 7-3 9-3 9h18s-3-2-3-9"/><path d="M13.73 21a2 2 0 0 1-3.46 0"/></svg>"# }
                            "Notices"
                        }
                    }
                }
            }
        }
    }
}

/// Extract URLs from message text.
fn extract_urls(text: &str) -> Vec<String> {
    text.split_whitespace()
        .map(|w| w.trim_matches(|c: char| c == '`' || c == '<' || c == '>' || c == '"' || c == '\''))
        .filter(|w| (w.starts_with("http://") || w.starts_with("https://")) && w.len() > 10)
        .take(2)
        .map(|w| w.trim_end_matches(['.', ',', ')', ']']).to_string())
        .collect()
}

/// Compact URL preview card.
#[component]
fn UrlPreviewCard(url: String) -> Element {
    let app_state = use_context::<Arc<AppState>>();

    // Fetch preview + resolve og:image in a single resource
    let preview = use_resource({
        let app_state = app_state.clone();
        let url = url.clone();
        move || {
            let app_state = app_state.clone();
            let url = url.clone();
            async move {
                let client = app_state.get_client().await.ok()?;
                let p = fluxchat_core::preview::get_url_preview(&client, &app_state.media_cache, &url).await.ok()??;
                // Resolve og:image if present
                let image_data_uri = if let Some(ref mxc) = p.image_url {
                    if mxc.starts_with("mxc://") {
                        fluxchat_core::media::get_media_url(&client, &app_state.db, &app_state.media_cache, mxc).await.ok()
                    } else if mxc.starts_with("http://") || mxc.starts_with("https://") {
                        Some(mxc.clone())
                    } else {
                        None
                    }
                } else {
                    None
                };
                Some((p, image_data_uri))
            }
        }
    });

    let resolved = preview.read().clone().flatten();

    if let Some((p, image_data_uri)) = resolved {
        rsx! {
            div { style: "display: flex; gap: 8px; padding: 4px 8px; margin: 4px 0; border-left: 2px solid var(--border); max-width: 400px; cursor: pointer;",
                onclick: { let url = url.clone(); move |_| { let _ = open::that(&url); } },
                div { style: "min-width: 0; flex: 1;",
                    if let Some(ref site) = p.site_name {
                        div { style: "font-size: 10px; color: var(--text-dim); font-family: var(--font-mono); text-transform: uppercase;",
                            "{site}"
                        }
                    }
                    if let Some(ref title) = p.title {
                        div { style: "font-size: 12px; color: var(--accent); font-family: var(--font-mono); overflow: hidden; text-overflow: ellipsis; white-space: nowrap;",
                            "{title}"
                        }
                    }
                    if let Some(ref desc) = p.description {
                        div { style: "font-size: 11px; color: var(--text-muted); font-family: var(--font-mono); overflow: hidden; text-overflow: ellipsis; display: -webkit-box; -webkit-line-clamp: 2; -webkit-box-orient: vertical;",
                            { desc.chars().take(150).collect::<String>() }
                        }
                    }
                }
                if let Some(ref src) = image_data_uri {
                    img {
                        src: "{src}",
                        style: "width: 60px; height: 60px; border-radius: 4px; object-fit: cover; flex-shrink: 0; align-self: center;",
                    }
                }
            }
        }
    } else {
        rsx! {}
    }
}

/// Space overview — shown when a space is selected but no room is open.
#[component]
fn SpaceOverview(space_id: String) -> Element {
    let ui = use_context::<UiState>();
    let app_state = use_context::<Arc<AppState>>();

    let space_info = ui.spaces.read().iter().find(|s| s.room_id == space_id).cloned();
    let space = match space_info { Some(s) => s, None => return rsx! {} };

    // Fetch space rooms
    let space_rooms = use_resource({
        let app_state = app_state.clone();
        let sid = space_id.clone();
        move || {
            let app_state = app_state.clone();
            let sid = sid.clone();
            async move {
                let client = app_state.get_client().await.ok()?;
                fluxchat_core::rooms::get_space_rooms(&client, &sid).await.ok()
            }
        }
    });

    let rooms = space_rooms.read().clone().flatten().unwrap_or_default();
    let mut invite_user = use_signal(String::new);
    let mut status_msg = use_signal(|| Option::<String>::None);

    rsx! {
        div { style: "flex: 1; display: flex; flex-direction: column; padding: 24px; overflow-y: auto;",
            // Space header
            div { style: "margin-bottom: 20px;",
                div { style: "font-size: 20px; font-weight: 700; color: var(--text-bright); font-family: var(--font-mono); margin-bottom: 4px;",
                    "{space.name}"
                }
                if let Some(ref topic) = space.topic {
                    div { style: "font-size: 13px; color: var(--text-muted); font-family: var(--font-mono);",
                        "{topic}"
                    }
                }
                div { style: "font-size: 11px; color: var(--text-dim); font-family: var(--font-mono); margin-top: 4px;",
                    "{space_id}"
                }
            }

            // Rooms in space
            div { style: "margin-bottom: 20px;",
                div { style: "font-size: 11px; font-weight: 700; text-transform: uppercase; color: var(--text-dim); letter-spacing: 0.5px; margin-bottom: 8px; font-family: var(--font-mono);",
                    "channels — {rooms.len()}"
                }
                for room in rooms.iter() {
                    { let rid = room.room_id.clone();
                      let rname = room.name.clone();
                      let encrypted = room.is_encrypted;
                      let unread = room.unread_count;
                      rsx! {
                        div {
                            style: "display: flex; align-items: center; gap: 8px; padding: 6px 8px; border-radius: 4px; cursor: pointer; font-family: var(--font-mono); font-size: 13px; color: var(--text-muted);",
                            onmouseenter: move |_| {},
                            onclick: {
                                let core = app_state.clone();
                                let ui = ui.clone();
                                let rid = rid.clone();
                                let rname = rname.clone();
                                move |_| {
                                    crate::components::sidebar::room_list::select_room_public(core.clone(), ui.clone(), rid.clone(), rname.clone());
                                }
                            },
                            span { style: "color: var(--text-dim); font-size: 12px;",
                                if encrypted {
                                    span { style: "display: inline-flex; opacity: 0.5;", dangerous_inner_html: r#"<svg width="12" height="12" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><rect x="3" y="11" width="18" height="11" rx="2" ry="2"/><path d="M7 11V7a5 5 0 0 1 10 0v4"/></svg>"# }
                                } else {
                                    "#"
                                }
                            }
                            span { style: "flex: 1;", "{rname}" }
                            if unread > 0 {
                                span { style: "background: var(--red); color: white; font-size: 9px; padding: 0 4px; border-radius: 6px; font-weight: 700;",
                                    "{unread}"
                                }
                            }
                        }
                    } }
                }
            }

            // Invite to space
            div {
                div { style: "font-size: 11px; font-weight: 700; text-transform: uppercase; color: var(--text-dim); letter-spacing: 0.5px; margin-bottom: 4px; font-family: var(--font-mono);", "invite to space" }
                div { style: "display: flex; gap: 6px; max-width: 400px;",
                    input {
                        style: "flex: 1; background: var(--bg-deep); border: 1px solid var(--border); border-radius: 4px; padding: 6px 8px; color: var(--text-bright); font-family: var(--font-mono); font-size: 13px; outline: none;",
                        placeholder: "@user:server.org",
                        value: "{invite_user}",
                        oninput: move |e| invite_user.set(e.value()),
                    }
                    span { style: "font-family: var(--font-mono); font-size: 12px; color: var(--green); cursor: pointer; padding: 6px 8px; border-radius: 4px; border: 1px solid var(--border); flex-shrink: 0;",
                        onclick: {
                            let app_state = app_state.clone();
                            let sid = space_id.clone();
                            move |_| {
                                let u = invite_user().trim().to_string();
                                if u.is_empty() { return; }
                                let app_state = app_state.clone();
                                let sid = sid.clone();
                                spawn(async move {
                                    let client = match app_state.get_client().await { Ok(c) => c, Err(_) => return };
                                    match fluxchat_core::rooms::invite_user(&client, &sid, &u).await {
                                        Ok(_) => { invite_user.set(String::new()); status_msg.set(Some(format!("invited {u}"))); }
                                        Err(e) => { status_msg.set(Some(format!("{e}"))); }
                                    }
                                });
                            }
                        },
                        "invite"
                    }
                }
                if let Some(ref msg) = *status_msg.read() {
                    div { style: "font-size: 12px; color: var(--text-muted); font-family: var(--font-mono); margin-top: 4px;", "{msg}" }
                }
            }
        }
    }
}
