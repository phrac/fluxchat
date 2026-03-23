use dioxus::prelude::*;
use std::collections::HashMap;
use std::sync::Arc;
use fluxchat_core::AppState;

use crate::state::UiState;
use crate::components::chat::message_input::MessageInput;


/// Nick color index from a string hash.
fn nick_color_index(name: &str) -> u32 {
    let mut hash: u32 = 0;
    for b in name.bytes() { hash = b as u32 + ((hash << 5).wrapping_sub(hash)); }
    hash % 15
}

/// Nick color from a string hash.
pub fn nick_color_class(name: &str) -> String {
    format!("nick-{}", nick_color_index(name))
}

/// Transform Matrix mention links into styled mention pills.
/// `avatar_cache` maps user_id -> resolved data URI for avatars.
fn transform_mentions(html: &str, avatar_cache: &HashMap<String, String>) -> String {
    let mut result = String::with_capacity(html.len());
    let mut remaining = html;
    let prefix = r#"<a href="https://matrix.to/#/"#;

    while let Some(start) = remaining.find(prefix) {
        result.push_str(&remaining[..start]);
        let after_prefix = &remaining[start + prefix.len()..];

        if let Some(quote_end) = after_prefix.find("\">") {
            let user_id_raw = &after_prefix[..quote_end];
            let user_id = user_id_raw.replace("%40", "@");
            let after_quote = &after_prefix[quote_end + 2..];

            if user_id.starts_with('@') {
                if let Some(end_tag) = after_quote.find("</a>") {
                    let display_name = &after_quote[..end_tag];
                    remaining = &after_quote[end_tag + 4..];

                    let color_idx = nick_color_index(display_name);

                    // Build avatar HTML — real image or colored initial
                    let avatar_html = if let Some(data_uri) = avatar_cache.get(&user_id) {
                        format!(r#"<img src="{data_uri}" style="width:14px;height:14px;border-radius:50%;object-fit:cover;vertical-align:middle;">"#)
                    } else {
                        let initial = display_name.chars().next()
                            .unwrap_or('?').to_uppercase().to_string();
                        format!(r#"<span class="mention-avatar nick-{color_idx}">{initial}</span>"#)
                    };

                    result.push_str(&format!(
                        r#"<span class="mention-pill" data-userid="{user_id}" onclick="if(window.__fluxchat_dioxus)window.__fluxchat_dioxus.send(this.dataset.userid);">{avatar_html}{display_name}</span>"#
                    ));
                    continue;
                }
            }
            // Not a user mention — keep original link
            result.push_str(prefix);
            remaining = after_prefix;
        } else {
            result.push_str(prefix);
            remaining = after_prefix;
        }
    }
    result.push_str(remaining);
    result
}

#[component]
pub fn MessageArea() -> Element {
    let app_state = use_context::<Arc<AppState>>();
    let mut ui = use_context::<UiState>();
    let selected = ui.selected_room_id.read().clone();
    let tabs = ui.open_tabs.read().clone();

    // Scroll active room to bottom on room switch
    use_effect(move || {
        let sel = ui.selected_room_id.read().clone();
        if let Some(id) = sel {
            let scroll_id = format!("msg-scroll-{}", id);
            document::eval(&format!(r#"
                requestAnimationFrame(() => requestAnimationFrame(() => {{
                    let el = document.getElementById('{scroll_id}');
                    if (el) el.scrollTop = el.scrollHeight;
                }}));
            "#));
        }
    });

    // Set up JS→Rust channel for mention pill clicks
    use_future(move || async move {
        let mut eval = document::eval(r#"
            window.__fluxchat_dioxus = dioxus;
        "#);
        loop {
            match eval.recv::<String>().await {
                Ok(user_id) => {
                    if !user_id.is_empty() {
                        let mut ui = consume_context::<UiState>();
                        ui.profile_user_id.set(Some(user_id));
                    }
                }
                Err(_) => break,
            }
        }
    });

    rsx! {
        div { style: "flex: 1; display: flex; flex-direction: column; min-width: 0; background: var(--bg-mid);",

            // Channel tabs
            div { style: "display: flex; align-items: center; height: 36px; border-bottom: 1px solid var(--border); flex-shrink: 0; padding: 0 8px; gap: 0; overflow-x: auto; background: var(--bg-dark);",
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

            // Render ALL open tabs' messages — show/hide via CSS for instant switching
            for (tab_room_id, _tab_name, _tab_space, _tab_mxc) in tabs.iter() {
                { let is_active = selected.as_ref() == Some(tab_room_id);
                  let display = if is_active { "flex: 1; display: flex; flex-direction: column; min-height: 0;" } else { "display: none;" };
                  rsx! {
                    div { style: "{display}",
                        RoomMessages { room_id: tab_room_id.clone(), is_active: is_active }
                    }
                  }
                }
            }

            if selected.is_none() {
                div { style: "flex: 1; display: flex; align-items: center; justify-content: center; color: var(--text-dim);",
                    "Select a channel to start chatting"
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
    let mut loading_older = use_signal(|| false);

    // Only read signals for the active tab to avoid subscribing inactive tabs to message changes
    let messages = if is_active {
        ui.messages_by_room.read().get(&room_id).cloned().unwrap_or_default()
    } else {
        // For inactive tabs, still need to render but use a lighter read
        ui.messages_by_room.peek().get(&room_id).cloned().unwrap_or_default()
    };
    let has_more = ui.pagination_tokens.read().contains_key(&room_id);

    // Build avatar maps with one cache lock + background resolve uncached
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
        let mmap: HashMap<String, String> = if is_active {
            ui.members_by_room.read()
                .get(&room_id)
                .map(|members| members.iter().filter_map(|m| {
                    let mxc = m.avatar_url.as_ref()?;
                    Some((m.user_id.clone(), cache.get(mxc)?.clone()))
                }).collect())
                .unwrap_or_default()
        } else {
            HashMap::new()
        };
        // Dedup uncached
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

    rsx! {
        div {
            id: "{scroll_id}",
            style: "flex: 1; overflow-y: auto; overflow-x: hidden; padding: 8px 12px;",
            onscroll: {
                let scroll_id = scroll_id.clone();
                let room_id = room_id.clone();
                let app_state = app_state.clone();
                move |_| {
                    if *loading_older.read() || !has_more { return; }
                    let id = scroll_id.clone();
                    let room_id = room_id.clone();
                    let app_state = app_state.clone();
                    spawn(async move {
                        let mut eval = document::eval(&format!(r#"
                            let el = document.getElementById('{id}');
                            dioxus.send(el ? el.scrollTop : 9999);
                        "#));
                        if let Ok(top) = eval.recv::<f64>().await {
                            if top < 150.0 {
                                loading_older.set(true);
                                let mut ui = consume_context::<UiState>();
                                let token = ui.pagination_tokens.read().get(&room_id).cloned();
                                if let Some(token) = token {
                                    let client = match app_state.get_client().await { Ok(c) => c, Err(_) => { loading_older.set(false); return; } };
                                    if let Ok(paginated) = fluxchat_core::messages::get_room_messages(&client, &room_id, Some(&token)).await {
                                        let older: Vec<_> = paginated.messages.into_iter().rev().collect();
                                        if let Some(new_token) = paginated.end_token {
                                            ui.pagination_tokens.write().insert(room_id.clone(), new_token);
                                        } else {
                                            ui.pagination_tokens.write().remove(&room_id);
                                        }
                                        let mut msgs = ui.messages_by_room.write();
                                        if let Some(existing) = msgs.get_mut(&room_id) {
                                            let mut combined = older;
                                            combined.append(existing);
                                            *existing = combined;
                                        }
                                    }
                                }
                                loading_older.set(false);
                            }
                        }
                    });
                }
            },
            if *loading_older.read() {
                div { style: "display: flex; justify-content: center; padding: 4px; font-family: var(--font-mono); font-size: 11px; color: var(--text-dim);",
                    "loading..."
                }
            }
            if messages.is_empty() {
                div { style: "display: flex; align-items: center; justify-content: center; height: 100%; color: var(--text-dim);",
                    "No messages yet"
                }
            }
            for msg in messages.iter() {
                { let is_system = msg.event_id.starts_with("~state-") || msg.event_id.starts_with("~system-");
                  if is_system {
                    rsx! {
                        div { style: "display: flex; padding: 2px 0; font-size: 12px; color: var(--text-dim); font-family: var(--font-mono); align-items: center;",
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
                    let eid = msg.event_id.clone();
                    let eid_react = msg.event_id.clone();
                    let eid_reply = msg.event_id.clone();
                    let eid_edit = msg.event_id.clone();
                    let eid_del = msg.event_id.clone();
                    let reply_name = msg.sender_name.clone();
                    let reply_body = msg.body.clone();
                    let edit_body = msg.body.clone();
                    let rid_del = room_id.clone();
                    let rid_react = room_id.clone();
                    let showing_react = react_menu_event.read().as_ref() == Some(&eid);
                    // Look up reply context from loaded messages
                    let reply_context = msg.reply_to_id.as_ref().and_then(|rid| {
                        messages.iter().find(|m| m.event_id == *rid)
                            .map(|m| (m.sender_name.clone(), m.body.clone()))
                    });
                    { rsx! {
                        // Reply indicator line
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
                                img {
                                    src: "{src}",
                                    style: "width: 16px; height: 16px; border-radius: 50%; object-fit: cover; flex-shrink: 0; margin-right: 4px;",
                                }
                            } else {
                                div { class: "{nick_class}", style: "width: 16px; height: 16px; border-radius: 50%; display: flex; align-items: center; justify-content: center; font-size: 8px; font-weight: 700; color: white; flex-shrink: 0; margin-right: 4px; background: var(--bg-light);",
                                    "{initial}"
                                }
                            }
                            span { style: "width: 100px; text-align: right; flex-shrink: 0; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; align-self: baseline;",
                                span { class: "{nick_class}", style: "font-weight: 600; font-family: var(--font-mono); font-size: 13px;",
                                    "{msg.sender_name}"
                                }
                            }
                            div { style: "width: 1px; align-self: stretch; background: var(--border); margin: 0 8px; flex-shrink: 0;" }
                            // Message content + reactions in a column
                            div { style: "min-width: 0; flex: 1;",
                                if msg.image_mxc.is_some() {
                                    InlineImage {
                                        room_id: room_id.clone(),
                                        event_id: msg.event_id.clone(),
                                        body: msg.body.clone(),
                                    }
                                } else if let Some(ref html) = msg.formatted_body {
                                    { let transformed = transform_mentions(html, &mention_avatars);
                                    rsx! { span { class: "msg-body", dangerous_inner_html: "{transformed}" } } }
                                } else {
                                    span { class: "msg-body", "{msg.body}" }
                                }
                                // Reactions below message
                                if !msg.reactions.is_empty() {
                                    div { style: "display: flex; flex-wrap: wrap; gap: 4px; padding: 2px 0;",
                                        for r in msg.reactions.iter() {
                                            { let tooltip = r.users.iter()
                                                .map(|u| u.split(':').next().unwrap_or(u).trim_start_matches('@').to_string())
                                                .collect::<Vec<_>>().join(", ");
                                            rsx! {
                                                span { style: "font-size: 12px; background: rgba(59,130,246,0.15); border-radius: 10px; padding: 1px 6px; color: var(--text-muted); cursor: default; font-family: var(--font-mono);",
                                                    title: "{tooltip}",
                                                    "{r.emoji} {r.count}"
                                                }
                                            } }
                                        }
                                    }
                                }
                            }
                            // Hover actions — IRC style
                            if !is_local {
                                { let actions_class = if showing_react { "msg-actions has-picker" } else { "msg-actions" };
                                rsx! { div { class: "{actions_class}",
                                    // React
                                    span { class: "msg-action",
                                        onclick: move |_| {
                                            let current = react_menu_event.read().clone();
                                            if current.as_ref() == Some(&eid_react) {
                                                react_menu_event.set(None);
                                            } else {
                                                react_menu_event.set(Some(eid_react.clone()));
                                            }
                                        },
                                        "+1"
                                    }
                                    // Reply
                                    span { class: "msg-action",
                                        onclick: {
                                            let name = reply_name.clone();
                                            let body = reply_body.clone();
                                            let eid = eid_reply.clone();
                                            move |_| {
                                                ui.reply_to.set(Some((eid.clone(), name.clone(), body.clone())));
                                                ui.edit_event.set(None);
                                            }
                                        },
                                        "re"
                                    }
                                    // Edit (own only)
                                    if is_own {
                                        span { class: "msg-action",
                                            onclick: {
                                                let body = edit_body.clone();
                                                let eid = eid_edit.clone();
                                                move |_| {
                                                    ui.edit_event.set(Some((eid.clone(), body.clone())));
                                                    ui.reply_to.set(None);
                                                }
                                            },
                                            "ed"
                                        }
                                    }
                                    // Delete (own only)
                                    if is_own {
                                        span { class: "msg-action",
                                            onclick: {
                                                let eid = eid_del.clone();
                                                let rid = rid_del.clone();
                                                let app_state = app_state.clone();
                                                move |_| {
                                                    // Remove locally immediately
                                                    if let Some(msgs) = ui.messages_by_room.write().get_mut(&rid) {
                                                        msgs.retain(|m| m.event_id != eid);
                                                    }
                                                    let eid = eid.clone();
                                                    let rid = rid.clone();
                                                    let app_state = app_state.clone();
                                                    spawn(async move {
                                                        let client = match app_state.get_client().await { Ok(c) => c, Err(_) => return };
                                                        let _ = fluxchat_core::messages::delete_message(&client, &rid, &eid).await;
                                                    });
                                                }
                                            },
                                            "rm"
                                        }
                                    }
                                    // Reaction picker popup
                                    if showing_react {
                                        div { class: "react-picker",
                                            for emoji in ["👍", "❤️", "😂", "🎉", "😮", "😢"].iter() {
                                                span {
                                                    onclick: {
                                                        let emoji = emoji.to_string();
                                                        let eid = eid.clone();
                                                        let rid = rid_react.clone();
                                                        let app_state = app_state.clone();
                                                        move |_| {
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
                                                    "{emoji}"
                                                }
                                            }
                                        }
                                    }
                                } }
                            } }
                        }
                    }
                    }
                  }
                }
            }
        }
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
        Some(Some(data_uri)) => rsx! {
            div { style: "display: flex; flex-direction: column;",
                img {
                    src: "{data_uri}",
                    alt: "{body}",
                    style: "max-width: 400px; max-height: 300px; border-radius: 4px; margin: 2px 0; object-fit: contain;",
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

