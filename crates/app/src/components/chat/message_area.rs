use dioxus::prelude::*;
use std::collections::HashMap;
use std::sync::Arc;
use fluxchat_core::AppState;

use crate::state::{UiState, RightPanel, nick_color_index, transform_mentions};

// Inline SVG icons — render identically on all platforms
const ICO_PIN: &str = r#"<svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><path d="M12 17v5"/><path d="M9 11V4a1 1 0 0 1 1-1h4a1 1 0 0 1 1 1v7"/><path d="M5 15h14l-1.5-4H6.5z"/></svg>"#;
const ICO_SEARCH: &str = r#"<svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><circle cx="11" cy="11" r="8"/><path d="m21 21-4.3-4.3"/></svg>"#;
const ICO_THREAD: &str = r#"<svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><path d="M3 6h18"/><path d="M3 12h18"/><path d="M3 18h18"/></svg>"#;
const ICO_MEMBERS: &str = r#"<svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><path d="M16 21v-2a4 4 0 0 0-4-4H6a4 4 0 0 0-4 4v2"/><circle cx="9" cy="7" r="4"/><path d="M22 21v-2a4 4 0 0 0-3-3.87"/><path d="M16 3.13a4 4 0 0 1 0 7.75"/></svg>"#;
const ICO_REACT: &str = r#"<svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><circle cx="12" cy="12" r="10"/><path d="M8 14s1.5 2 4 2 4-2 4-2"/><line x1="9" y1="9" x2="9.01" y2="9"/><line x1="15" y1="9" x2="15.01" y2="9"/></svg>"#;
const ICO_REPLY: &str = r#"<svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><polyline points="9 17 4 12 9 7"/><path d="M20 18v-2a4 4 0 0 0-4-4H4"/></svg>"#;
const ICO_EDIT: &str = r#"<svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><path d="M17 3a2.85 2.83 0 1 1 4 4L7.5 20.5 2 22l1.5-5.5Z"/></svg>"#;
const ICO_DELETE: &str = r#"<svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><path d="M3 6h18"/><path d="M19 6v14c0 1-1 2-2 2H7c-1 0-2-1-2-2V6"/><path d="M8 6V4c0-1 1-2 2-2h4c1 0 2 1 2 2v2"/></svg>"#;
const ICO_SETTINGS: &str = r#"<svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><circle cx="12" cy="12" r="3"/><path d="M19.4 15a1.65 1.65 0 0 0 .33 1.82l.06.06a2 2 0 0 1-2.83 2.83l-.06-.06a1.65 1.65 0 0 0-1.82-.33 1.65 1.65 0 0 0-1 1.51V21a2 2 0 0 1-4 0v-.09A1.65 1.65 0 0 0 9 19.4a1.65 1.65 0 0 0-1.82.33l-.06.06a2 2 0 0 1-2.83-2.83l.06-.06A1.65 1.65 0 0 0 4.68 15a1.65 1.65 0 0 0-1.51-1H3a2 2 0 0 1 0-4h.09A1.65 1.65 0 0 0 4.6 9a1.65 1.65 0 0 0-.33-1.82l-.06-.06a2 2 0 0 1 2.83-2.83l.06.06A1.65 1.65 0 0 0 9 4.68a1.65 1.65 0 0 0 1-1.51V3a2 2 0 0 1 4 0v.09a1.65 1.65 0 0 0 1 1.51 1.65 1.65 0 0 0 1.82-.33l.06-.06a2 2 0 0 1 2.83 2.83l-.06.06A1.65 1.65 0 0 0 19.4 9a1.65 1.65 0 0 0 1.51 1H21a2 2 0 0 1 0 4h-.09a1.65 1.65 0 0 0-1.51 1z"/></svg>"#;
use crate::components::chat::message_input::MessageInput;
use crate::components::chat::emoji_picker::EmojiPicker;

/// Nick color from a string hash.
pub fn nick_color_class(name: &str) -> String {
    format!("nick-{}", nick_color_index(name))
}

#[component]
pub fn MessageArea() -> Element {
    let _app_state = use_context::<Arc<AppState>>();
    let ui = use_context::<UiState>();
    let selected = ui.selected_room_id.read().clone();
    let tabs = ui.open_tabs.read().clone();


    // Set up JS→Rust channel for mention pill clicks and drag-drop
    use_future(move || async move {
        let mut eval = document::eval(r#"
            window.__fluxchat_dioxus = dioxus;

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

            // Paste image handler
            document.addEventListener('paste', async (e) => {
                let items = e.clipboardData && e.clipboardData.items;
                if (!items) return;
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
        loop {
            match eval.recv::<String>().await {
                Ok(msg) => {
                    if msg.starts_with("file:") {
                        // File drop or paste
                        let json = &msg[5..];
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
                    } else if msg.starts_with("paginate:") {
                        // Pagination via Timeline API — just call paginate_backwards
                        // The diff stream watcher (spawned in select_room) will update messages_by_room
                        let room_id = msg[9..].to_string();
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
                            let el = document.getElementById('{sid}');
                            if (el) el.__fluxLoading = false;
                        "#));
                    } else if !msg.is_empty() {
                        // Mention pill click
                        let mut ui = consume_context::<UiState>();
                        ui.profile_user_id.set(Some(msg));
                    }
                }
                Err(_) => break,
            }
        }
    });

    rsx! {
        div { style: "flex: 1; display: flex; flex-direction: column; min-width: 0; background: var(--bg-mid);",

            // Channel tabs + room actions
            div { style: "display: flex; align-items: center; height: 36px; border-bottom: 1px solid var(--border); flex-shrink: 0; background: var(--bg-dark);",
                // Tabs
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
                // Room action icons — right-aligned
                if selected.is_some() {
                    RoomActions {}
                }
            }

            // Search HUD — drops down from top
            if *ui.search_open.read() {
                SearchHud {}
            }

            // Pins HUD — drops down from top
            if *ui.pins_open.read() {
                PinsHud {}
            }

            // Render ALL open tabs' messages — show/hide via CSS for instant switching
            // but wrapped in a component that only updates when its room is active.
            for (tab_room_id, _tab_name, _tab_space, _tab_mxc) in tabs.iter() {
                { let is_active = selected.as_ref() == Some(tab_room_id);
                  rsx! {
                    div {
                        key: "{tab_room_id}",
                        style: if is_active { "flex: 1; display: flex; flex-direction: column; min-height: 0;" } else { "display: none;" },
                        RoomMessages { room_id: tab_room_id.clone(), is_active: is_active }
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

            // Typing indicator
            { let typing = selected.as_ref()
                .and_then(|id| ui.typing_by_room.read().get(id).cloned())
                .unwrap_or_default();
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

            // Message input
            if selected.is_some() {
                MessageInput {}
            }
        }
    }
}

/// Per-room message list — stays in DOM, shown/hidden via CSS.
#[component]
fn RoomMessages(room_id: String, is_active: bool) -> Element {
    let mut ui = use_context::<UiState>();
    let app_state = use_context::<Arc<AppState>>();
    let my_user_id = ui.user_id.read().clone();
    let mut react_menu_event = use_signal(|| Option::<String>::None);

    // Only read messages signal if we are the active tab, or if we have no messages yet.
    // This prevents background tabs from re-rendering the whole message list on every sync event.
    let messages = if is_active {
        ui.messages_by_room.read().get(&room_id).cloned().unwrap_or_default()
    } else {
        ui.messages_by_room.peek().get(&room_id).cloned().unwrap_or_default()
    };
    let _has_more = ui.pagination_tokens.read().contains_key(&room_id);

    // Build avatar maps with one cache lock — only when active or first render
    let (avatar_map, mention_avatars, uncached) = {
        let cache = app_state.media_cache.lock().unwrap();
        let mut amap = HashMap::new();
        let mut uncached = Vec::new();
        for m in &messages {
            if let Some(mxc) = &m.avatar_url {
                if let Some(data_uri) = cache.get(mxc) {
                    amap.insert(mxc.clone(), data_uri.clone());
                } else if is_active {
                    uncached.push(mxc.clone());
                }
            }
        }
        let mmap: HashMap<String, String> = if is_active {
            ui.members_by_room.read()
                .get(&room_id)
                .map(|members| members.iter().filter_map(|m| {
                    let mxc = m.avatar_url.as_ref()?;
                    Some((m.user_id.clone(), cache.get(mxc)?.clone()))
                }).collect())
                .unwrap_or_default()
        } else { HashMap::new() };
        uncached.sort();
        uncached.dedup();
        (amap, mmap, uncached)
    };

    // Background-resolve uncached message avatars
    if is_active && !uncached.is_empty() {
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
            let _ = is_active;
            if !is_active { return; }
            // Set up a scroll listener for pagination via JS
            let sid = sid.clone();
            let room_id = room_id_pg.clone();
            document::eval(&format!(r#"
                setTimeout(() => {{
                    let el = document.getElementById('{sid}');
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
                            if (window.__fluxchat_dioxus) window.__fluxchat_dioxus.send('paginate:{room_id}');
                        }}
                    }});
                }}, 50);
            "#));
        });
    }

    rsx! {
        div {
            id: "{scroll_id}",
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
                { let is_system = msg.event_id.starts_with("~state-") || msg.event_id.starts_with("~system-");
                  let eid = msg.event_id.clone();
                  if is_system {
                    rsx! {
                        div { key: "{eid}", style: "display: flex; padding: 2px 0; font-size: 12px; color: var(--text-dim); font-family: var(--font-mono); align-items: center;",
                            span { class: "msg-time", "" }
                            span { style: "width: 120px; flex-shrink: 0;" }
                            div { style: "width: 1px; align-self: stretch; background: var(--border); margin: 0 8px; flex-shrink: 0;" }
                            span { style: "color: var(--green); margin-right: 4px;", "-->" }
                            "{msg.body}"
                        }
                    }
                  } else {
                    let nick_class = nick_color_class(&msg.sender);
                    let time = {
                        let ts = msg.timestamp;
                        let secs = ts % 86400;
                        let h = secs / 3600;
                        let m = (secs % 3600) / 60;
                        format!("{:02}:{:02}", h, m)
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
                    let content_rsx = if msg.image_mxc.is_some() {
                        rsx! { InlineImage { room_id: room_id.clone(), event_id: msg.event_id.clone(), body: msg.body.clone() } }
                    } else if let Some(ref transformed) = msg.transformed_body {
                        rsx! { span { class: "msg-body", dangerous_inner_html: "{transformed}" } }
                    } else if let Some(ref html) = msg.formatted_body {
                        // Compute and save back to the message object if possible (requires write lock, so maybe just compute once)
                        let transformed = transform_mentions(html, &mention_avatars);
                        rsx! { span { class: "msg-body", dangerous_inner_html: "{transformed}" } }
                    } else {
                        rsx! { span { class: "msg-body", "{msg.body}" } }
                    };

                    // Look up reply context
                    let reply_context = msg.reply_to_id.as_ref().and_then(|rid| {
                        messages.iter().find(|m| m.event_id == *rid)
                            .map(|m| (m.sender_name.clone(), m.body.clone()))
                    });

                    rsx! {
                        div { key: "{eid}",
                            if let Some((ref reply_sender, ref reply_preview)) = reply_context {
                                div { style: "display: flex; padding: 0 0 0 72px; font-size: 11px; font-family: var(--font-mono); color: var(--text-dim); align-items: center; gap: 4px;",
                                    span { style: "color: var(--green);", "╭─" }
                                    span { style: "color: var(--text-muted);", "{reply_sender}:" }
                                    span { style: "overflow: hidden; text-overflow: ellipsis; white-space: nowrap; max-width: 400px;",
                                        { reply_preview.chars().take(80).collect::<String>() }
                                    }
                                }
                            }
                            div { class: "msg-row", style: "display: flex; padding: 1px 0; align-items: flex-start;",
                                span { class: "msg-time", style: "align-self: baseline;", "{time}" }
                                if let Some(src) = avatar_src {
                                    img { src: "{src}", style: "width: 16px; height: 16px; border-radius: 50%; object-fit: cover; flex-shrink: 0; margin-right: 4px;" }
                                } else {
                                    div { class: "{nick_class}", style: "width: 16px; height: 16px; border-radius: 50%; display: flex; align-items: center; justify-content: center; font-size: 8px; font-weight: 700; color: white; flex-shrink: 0; margin-right: 4px; background: var(--bg-light);", "{initial}" }
                                }
                                span { style: "width: 100px; text-align: right; flex-shrink: 0; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; align-self: baseline;",
                                    span { class: "{nick_class}", style: "font-weight: 600; font-family: var(--font-mono); font-size: 13px;", "{msg.sender_name}" }
                                }
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
                                } } }
                                // Hover actions
                                if !is_local && is_active {
                                    div { class: if showing_react { "msg-actions has-picker" } else { "msg-actions" },
                                        span { class: "msg-action", onclick: move |_| { react_menu_event.set(if react_menu_event.read().as_ref() == Some(&eid_react) { None } else { Some(eid_react.clone()) }); }, dangerous_inner_html: ICO_REACT }
                                        span { class: "msg-action", onclick: { let name = reply_name.clone(); let body = reply_body.clone(); let eid = eid_reply.clone(); move |_| { ui.reply_to.set(Some((eid.clone(), name.clone(), body.clone()))); ui.edit_event.set(None); } }, dangerous_inner_html: ICO_REPLY }
                                        span { class: "msg-action", onclick: { let eid = eid_pin.clone(); let rid = rid_pin.clone(); let app_state = app_state.clone(); move |_| { let eid = eid.clone(); let rid = rid.clone(); let app_state = app_state.clone(); spawn(async move { let client = match app_state.get_client().await { Ok(c) => c, Err(_) => return }; let _ = fluxchat_core::messages::pin_message(&client, &rid, &eid).await; }); } }, dangerous_inner_html: ICO_PIN }
                                        if is_own {
                                            span { class: "msg-action", onclick: { let body = edit_body.clone(); let eid = eid_edit.clone(); move |_| { ui.edit_event.set(Some((eid.clone(), body.clone()))); ui.reply_to.set(None); } }, dangerous_inner_html: ICO_EDIT }
                                            span { class: "msg-action", onclick: { let eid = eid_del.clone(); let rid = rid_del.clone(); let app_state = app_state.clone(); move |_| { if let Some(msgs) = ui.messages_by_room.write().get_mut(&rid) { msgs.retain(|m| m.event_id != eid); } let eid = eid.clone(); let rid = rid.clone(); let app_state = app_state.clone(); spawn(async move { let client = match app_state.get_client().await { Ok(c) => c, Err(_) => return }; let _ = fluxchat_core::messages::delete_message(&client, &rid, &eid).await; }); } }, dangerous_inner_html: ICO_DELETE }
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
                                                                let client = match app_state.get_client().await { Ok(c) => c, Err(_) => return };
                                                                let _ = fluxchat_core::messages::send_reaction(&client, &rid, &eid, &emoji).await;
                                                            });
                                                        }
                                                    },
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
            }
            } // inner div
        } // scroll container
    }
}

#[component]
fn ChannelTab(room_id: String, name: String, space_name: Option<String>, space_avatar_mxc: Option<String>, is_active: bool) -> Element {
    let mut ui = use_context::<UiState>();
    let core = use_context::<Arc<AppState>>();

    let tab_class = if is_active { "channel-tab active" } else { "channel-tab" };
    let label = format!("#{name}");

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
                    move |e: Event<MouseData>| {
                        e.stop_propagation();
                        let mut tabs = ui.open_tabs.write();
                        tabs.retain(|(id, _, _, _)| id != &cid);
                        if ui.selected_room_id.read().as_ref() == Some(&cid) {
                            let next = tabs.last().map(|(id, _, _, _)| id.clone());
                            drop(tabs);
                            ui.selected_room_id.set(next);
                        }
                        // Free memory for closed tab
                        ui.messages_by_room.write().remove(&cid);
                        ui.members_by_room.write().remove(&cid);
                        ui.pagination_tokens.write().remove(&cid);
                    }
                },
                "×"
            }
        }
    }
}

#[component]
fn InlineImage(room_id: String, event_id: String, body: String) -> Element {
    let app_state = use_context::<Arc<AppState>>();
    let mut ui = use_context::<UiState>();

    let image_data = use_resource(move || {
        let app_state = app_state.clone();
        let room_id = room_id.clone();
        let event_id = event_id.clone();
        async move {
            let client = app_state.get_client().await.ok()?;
            fluxchat_core::media::get_event_media(
                &client, &app_state.db, &app_state.media_cache, &room_id, &event_id
            ).await.ok()
        }
    });

    let resolved = image_data.read().clone();
    match resolved {
        Some(Some(data_uri)) => {
            let uri_for_click = data_uri.clone();
            rsx! {
                div { style: "display: flex; flex-direction: column;",
                    img {
                        src: "{data_uri}",
                        alt: "{body}",
                        style: "max-width: 400px; max-height: 300px; border-radius: 4px; margin: 2px 0; object-fit: contain; cursor: pointer;",
                        onclick: move |_| { ui.lightbox_url.set(Some(uri_for_click.clone())); },
                    }
                }
            }
        },
        Some(None) => rsx! {
            span { class: "msg-body", style: "color: var(--text-dim);", "[image: {body}]" }
        },
        None => rsx! {
            span { class: "msg-body", style: "color: var(--text-dim);", "[loading {body}...]" }
        },
    }
}

/// Pinned messages HUD — drops down from the tab bar.
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

    rsx! {
        div { class: "search-hud", style: "flex-direction: column; align-items: stretch; max-height: 300px; overflow-y: auto;",
            // Header row
            div { style: "display: flex; align-items: center; justify-content: space-between; margin-bottom: 4px;",
                span { style: "font-family: var(--font-mono); font-size: 11px; font-weight: 700; text-transform: uppercase; color: var(--text-dim); letter-spacing: 0.5px;",
                    { match &resolved {
                        Some(msgs) => format!("pinned — {}", msgs.len()),
                        None => "pinned — ...".to_string(),
                    }}
                }
                span { style: "color: var(--text-dim); cursor: pointer; font-size: 14px; padding: 0 2px;",
                    onclick: move |_| { ui.pins_open.set(false); },
                    "×"
                }
            }
            // Pinned messages
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

/// Search HUD — drops down from the tab bar.
#[component]
fn SearchHud() -> Element {
    let mut ui = use_context::<UiState>();
    let mut query = use_signal(|| String::new());
    let mut results = use_signal(|| Vec::<(String, String, String)>::new()); // (event_id, sender, body)
    let mut current_idx = use_signal(|| 0usize);

    let selected = ui.selected_room_id.read().clone();

    let mut do_search = move |_: ()| {
        let q = query().trim().to_lowercase();
        if q.is_empty() {
            results.set(Vec::new());
            return;
        }
        if let Some(ref room_id) = selected {
            let matches: Vec<_> = ui.messages_by_room.read()
                .get(room_id)
                .map(|msgs| msgs.iter()
                    .filter(|m| m.body.to_lowercase().contains(&q))
                    .map(|m| (m.event_id.clone(), m.sender_name.clone(), m.body.clone()))
                    .collect())
                .unwrap_or_default();
            current_idx.set(if matches.is_empty() { 0 } else { matches.len() - 1 });
            results.set(matches);
        }
    };

    let result_count = results.read().len();
    let idx = *current_idx.read();

    rsx! {
        div { class: "search-hud",
            span { style: "color: var(--text-dim); font-size: 14px;", "⌕" }
            input {
                r#type: "text",
                placeholder: "search messages...",
                value: "{query}",
                oninput: move |e| {
                    query.set(e.value());
                    do_search(());
                },
                onkeydown: {
                    let mut ui = ui.clone();
                    move |e: Event<KeyboardData>| {
                        if e.key() == Key::Escape {
                            ui.search_open.set(false);
                        } else if e.key() == Key::Enter {
                            if result_count > 0 {
                                let new_idx = if idx > 0 { idx - 1 } else { result_count - 1 };
                                current_idx.set(new_idx);
                            }
                        }
                    }
                },
            }
            if result_count > 0 {
                span { class: "search-count", "{idx + 1}/{result_count}" }
                span { class: "search-nav",
                    onclick: move |_| { if idx > 0 { current_idx.set(idx - 1); } },
                    "▲"
                }
                span { class: "search-nav",
                    onclick: move |_| { if idx < result_count - 1 { current_idx.set(idx + 1); } },
                    "▼"
                }
            } else if !query().is_empty() {
                span { class: "search-count", "no results" }
            }
            span { style: "color: var(--text-dim); cursor: pointer; font-size: 14px; padding: 0 2px;",
                onclick: move |_| { ui.search_open.set(false); },
                "×"
            }
        }
    }
}

/// Room action icons in the tab bar header.
#[component]
fn RoomActions() -> Element {
    let mut ui = use_context::<UiState>();
    let panel = ui.right_panel.read().clone();
    let members_visible = *ui.show_member_list.read();

    rsx! {
        div { style: "display: flex; align-items: center; gap: 1px; padding: 0 8px; flex-shrink: 0;",
            // Pinned messages
            span {
                class: if *ui.pins_open.read() { "room-action active" } else { "room-action" },
                title: "Pinned messages",
                onclick: move |_| { let open = *ui.pins_open.read(); ui.pins_open.set(!open); },
                dangerous_inner_html: ICO_PIN,
            }
            // Search
            span {
                class: if *ui.search_open.read() { "room-action active" } else { "room-action" },
                title: "Search",
                onclick: move |_| { let open = *ui.search_open.read(); ui.search_open.set(!open); },
                dangerous_inner_html: ICO_SEARCH,
            }
            // Threads
            span {
                class: if panel == Some(RightPanel::Threads) { "room-action active" } else { "room-action" },
                title: "Threads",
                onclick: move |_| {
                    let current = ui.right_panel.read().clone();
                    if current == Some(RightPanel::Threads) { ui.right_panel.set(None); }
                    else { ui.right_panel.set(Some(RightPanel::Threads)); }
                },
                dangerous_inner_html: ICO_THREAD,
            }
            // Toggle member list
            span {
                class: if members_visible { "room-action active" } else { "room-action" },
                title: "Toggle member list",
                onclick: move |_| { ui.show_member_list.set(!members_visible); },
                dangerous_inner_html: ICO_MEMBERS,
            }
            // Room settings
            span {
                class: if *ui.room_settings_open.read() { "room-action active" } else { "room-action" },
                title: "Room settings",
                onclick: move |_| { let v = *ui.room_settings_open.read(); ui.room_settings_open.set(!v); },
                dangerous_inner_html: ICO_SETTINGS,
            }
        }
    }
}

/// Extract URLs from message text.
fn extract_urls(text: &str) -> Vec<String> {
    text.split_whitespace()
        .filter(|w| (w.starts_with("http://") || w.starts_with("https://")) && w.len() > 10)
        .take(2)
        .map(|w| w.trim_end_matches(|c: char| c == '.' || c == ',' || c == ')' || c == ']').to_string())
        .collect()
}

/// Compact URL preview card.
#[component]
fn UrlPreviewCard(url: String) -> Element {
    let app_state = use_context::<Arc<AppState>>();

    let preview = use_resource({
        let app_state = app_state.clone();
        let url = url.clone();
        move || {
            let app_state = app_state.clone();
            let url = url.clone();
            async move {
                let client = app_state.get_client().await.ok()?;
                fluxchat_core::preview::get_url_preview(&client, &app_state.media_cache, &url).await.ok()?
            }
        }
    });

    let resolved = preview.read().clone().flatten();

    if let Some(p) = resolved {
        rsx! {
            div { style: "display: flex; gap: 8px; padding: 4px 8px; margin: 4px 0; border-left: 2px solid var(--border); max-width: 400px;",
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
    let mut invite_user = use_signal(|| String::new());
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
                                if encrypted { "🔒" } else { "#" }
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
