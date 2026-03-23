use dioxus::prelude::*;
use std::sync::Arc;
use fluxchat_core::AppState;

use crate::state::UiState;
use crate::components::chat::emoji_picker::EmojiPicker;
use crate::components::chat::commands::{self, CommandAction};

const ICO_PLUS: &str = r#"<svg width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2.5" stroke-linecap="round"><line x1="12" y1="5" x2="12" y2="19"/><line x1="5" y1="12" x2="19" y2="12"/></svg>"#;
const ICO_BOLD: &str = r#"<svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2.5" stroke-linecap="round" stroke-linejoin="round"><path d="M6 4h8a4 4 0 0 1 4 4 4 4 0 0 1-4 4H6z"/><path d="M6 12h9a4 4 0 0 1 4 4 4 4 0 0 1-4 4H6z"/></svg>"#;
const ICO_ITALIC: &str = r#"<svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2.5" stroke-linecap="round" stroke-linejoin="round"><line x1="19" y1="4" x2="10" y2="4"/><line x1="14" y1="20" x2="5" y2="20"/><line x1="15" y1="4" x2="9" y2="20"/></svg>"#;
const ICO_STRIKE: &str = r#"<svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2.5" stroke-linecap="round" stroke-linejoin="round"><path d="M16 4H9a3 3 0 0 0-3 3c0 2 1.5 3 3 3h6"/><path d="M4 12h16"/><path d="M15 12a3 3 0 0 1-3 3H8"/></svg>"#;
const ICO_CODE: &str = r#"<svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2.5" stroke-linecap="round" stroke-linejoin="round"><polyline points="16 18 22 12 16 6"/><polyline points="8 6 2 12 8 18"/></svg>"#;
const ICO_CODEBLOCK: &str = r#"<svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2.5" stroke-linecap="round" stroke-linejoin="round"><rect x="3" y="3" width="18" height="18" rx="2"/><polyline points="10 8 6 12 10 16"/><polyline points="14 8 18 12 14 16"/></svg>"#;
const ICO_LINK: &str = r#"<svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2.5" stroke-linecap="round" stroke-linejoin="round"><path d="M10 13a5 5 0 0 0 7.54.54l3-3a5 5 0 0 0-7.07-7.07l-1.72 1.71"/><path d="M14 11a5 5 0 0 0-7.54-.54l-3 3a5 5 0 0 0 7.07 7.07l1.71-1.71"/></svg>"#;
const ICO_EMOJI: &str = r#"<svg width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><circle cx="12" cy="12" r="10"/><path d="M8 14s1.5 2 4 2 4-2 4-2"/><line x1="9" y1="9" x2="9.01" y2="9"/><line x1="15" y1="9" x2="15.01" y2="9"/></svg>"#;

#[component]
pub fn MessageInput() -> Element {
    let core = use_context::<Arc<AppState>>();
    let mut ui = use_context::<UiState>();
    let mut text = use_signal(|| String::new());
    let mut sending = use_signal(|| false);
    let mut uploading = use_signal(|| false);
    let mut typing_sent = use_signal(|| false);
    let mut emoji_open = use_signal(|| false);
    let mut help_text = use_signal(|| Option::<String>::None);

    let reply_to = ui.reply_to.read().clone();
    let edit_event = ui.edit_event.read().clone();
    let upload_pending = ui.upload_pending.read().clone();

    // When entering edit mode, populate the input with the original text
    use_effect(move || {
        let edit = ui.edit_event.read().clone();
        if let Some((_, ref body)) = edit {
            text.set(body.clone());
        }
    });

    let mut ui_cancel = ui.clone();
    let mut ui_cancel2 = ui.clone();
    let ui_cancel3 = ui.clone();
    let ui_upload = ui.clone();
    let ui_typing = ui.clone();
    let core_typing = core.clone();

    // Upload handler
    let handle_upload = {
        let core = core.clone();
        move |filename: String, mime: String, data: Vec<u8>| {
            let room_id = match ui_upload.selected_room_id.read().clone() { Some(id) => id, None => return };
            let core = core.clone();
            spawn(async move {
                uploading.set(true);
                let client = match core.get_client().await { Ok(c) => c, Err(_) => { uploading.set(false); return; } };
                let _ = fluxchat_core::messages::upload_file(&client, &room_id, &filename, &mime, data).await;
                uploading.set(false);
            });
        }
    };

    let mut handle_send = move |_: ()| {
        let mut body = text().trim().to_string();

        // Handle slash commands via modular registry
        if body.starts_with('/') {
            let room_id = ui.selected_room_id.read().clone();
            match commands::dispatch(&body, room_id.as_deref(), core.clone(), ui.clone()) {
                CommandAction::Handled => {
                    text.set(String::new());
                    help_text.set(None);
                    return;
                }
                CommandAction::Rewrite(new_body) => {
                    body = new_body;
                    help_text.set(None);
                    // Fall through to normal send
                }
                CommandAction::ShowHelp(msg) => {
                    text.set(String::new());
                    help_text.set(Some(msg));
                    return;
                }
                CommandAction::Noop => {
                    // Unknown or incomplete command — send as plain text
                }
            }
        }

        let body = body; // rebind as immutable
        let pending_upload = {
            let val = ui.upload_pending.read().clone();
            val
        };
        if body.is_empty() && pending_upload.is_none() || *sending.read() { return; }
        let room_id = match ui.selected_room_id.read().clone() { Some(id) => id, None => return };
        let core = core.clone();
        let reply = ui.reply_to.read().clone();
        let edit = ui.edit_event.read().clone();

        text.set(String::new());
        ui.reply_to.set(None);
        ui.edit_event.set(None);
        ui.upload_pending.set(None);
        typing_sent.set(false);
        // Stop typing indicator
        {
            let core = core.clone();
            let room_id = room_id.clone();
            spawn(async move {
                let client = match core.get_client().await { Ok(c) => c, Err(_) => return };
                let _ = fluxchat_core::messages::send_typing(&client, &room_id, false).await;
            });
        }

        // If there's a file pending, upload it (and send text after if any)
        if let Some((filename, mime, data)) = pending_upload {
            let core2 = core.clone();
            let body2 = body.clone();
            let room_id2 = room_id.clone();
            spawn(async move {
                sending.set(true);
                let client = match core2.get_client().await { Ok(c) => c, Err(_) => { sending.set(false); return; } };
                let _ = fluxchat_core::messages::upload_file(&client, &room_id2, &filename, &mime, data).await;
                // Also send text message if user typed something
                if !body2.is_empty() {
                    let _ = fluxchat_core::messages::send_message(&client, &room_id2, &body2).await;
                }
                sending.set(false);
            });
            return;
        }

        // Use Timeline API for send/reply/edit — handles local echo automatically
        if let Some((event_id, _)) = edit {
            let body = body.clone();
            spawn(async move {
                sending.set(true);
                if let Ok(rid) = fluxchat_core::ruma::RoomId::parse(&room_id) {
                    let timelines = core.timelines.lock().await;
                    if let Some(timeline) = timelines.get(&*rid) {
                        if let Ok(eid) = fluxchat_core::ruma::EventId::parse(&event_id) {
                            use matrix_sdk::room::edit::EditedContent;
                            use matrix_sdk::ruma::events::room::message::RoomMessageEventContentWithoutRelation;
                            let content = RoomMessageEventContentWithoutRelation::text_plain(&body);
                            let item_id = matrix_sdk_ui::timeline::TimelineEventItemId::EventId(eid);
                            let _ = timeline.edit(&item_id, EditedContent::RoomMessage(content)).await;
                        }
                    } else {
                        let client = match core.get_client().await { Ok(c) => c, Err(_) => { sending.set(false); return; } };
                        let _ = fluxchat_core::messages::edit_message(&client, &room_id, &event_id, &body).await;
                    }
                }
                sending.set(false);
            });
        } else if let Some((event_id, _, _)) = reply {
            spawn(async move {
                sending.set(true);
                if let Ok(rid) = fluxchat_core::ruma::RoomId::parse(&room_id) {
                    let timelines = core.timelines.lock().await;
                    if let Some(timeline) = timelines.get(&*rid) {
                        if let Ok(eid) = fluxchat_core::ruma::EventId::parse(&event_id) {
                            use matrix_sdk::ruma::events::room::message::RoomMessageEventContentWithoutRelation;
                            let content = RoomMessageEventContentWithoutRelation::text_plain(&body);
                            let _ = timeline.send_reply(content, eid).await;
                        }
                    } else {
                        let client = match core.get_client().await { Ok(c) => c, Err(_) => { sending.set(false); return; } };
                        let _ = fluxchat_core::messages::send_reply(&client, &room_id, &body, &event_id).await;
                    }
                }
                sending.set(false);
            });
        } else {
            // Normal send via Timeline
            spawn(async move {
                sending.set(true);
                if let Ok(rid) = fluxchat_core::ruma::RoomId::parse(&room_id) {
                    let timelines = core.timelines.lock().await;
                    if let Some(timeline) = timelines.get(&*rid) {
                        use matrix_sdk::ruma::events::room::message::RoomMessageEventContent;
                        let content = RoomMessageEventContent::text_plain(&body);
                        let _ = timeline.send(content.into()).await;
                    } else {
                        let client = match core.get_client().await { Ok(c) => c, Err(_) => { sending.set(false); return; } };
                        let _ = fluxchat_core::messages::send_message(&client, &room_id, &body).await;
                    }
                }
                sending.set(false);
            });
        }
    };

    // Prompt prefix changes based on mode
    let (prefix, prefix_color) = if edit_event.is_some() {
        ("ed\u{276f}", "var(--yellow)")
    } else if reply_to.is_some() {
        ("re\u{276f}", "var(--green)")
    } else {
        ("\u{276f}", "var(--accent-muted)")
    };

    // Format button helper — wraps selection or appends markup
    let make_fmt_handler = |pfx: &'static str, sfx: &'static str| {
        move |_| {
            let pfx = pfx.to_string();
            let sfx = sfx.to_string();
            spawn(async move {
                let mut eval = document::eval(r#"
                    let ta = document.getElementById('flux-input');
                    if (ta) { dioxus.send(ta.selectionStart + ',' + ta.selectionEnd); }
                    else { dioxus.send('-1,-1'); }
                "#);
                if let Ok(data) = eval.recv::<String>().await {
                    let parts: Vec<&str> = data.split(',').collect();
                    if parts.len() == 2 {
                        let start: usize = parts[0].parse().unwrap_or(0);
                        let end: usize = parts[1].parse().unwrap_or(0);
                        let current = text();
                        if start != end && start < current.len() && end <= current.len() {
                            let before = &current[..start];
                            let selected = &current[start..end];
                            let after = &current[end..];
                            text.set(format!("{before}{pfx}{selected}{sfx}{after}"));
                        } else {
                            text.set(format!("{current}{pfx}{sfx}"));
                        }
                    }
                }
            });
        }
    };

    rsx! {
        div { style: "flex-shrink: 0;",
            // Reply/edit context bar
            if let Some((_, ref name, ref body)) = reply_to {
                div { style: "display: flex; align-items: center; padding: 4px 16px; font-size: 12px; font-family: var(--font-mono); color: var(--text-dim); background: var(--bg-deep); border-left: 2px solid var(--green); margin: 0 12px;",
                    span { style: "color: var(--green); margin-right: 6px;", "replying to {name}:" }
                    span { style: "flex: 1; overflow: hidden; text-overflow: ellipsis; white-space: nowrap;",
                        { body.chars().take(80).collect::<String>() }
                    }
                    span { style: "cursor: pointer; padding: 0 4px; color: var(--text-dim);",
                        onclick: move |_| { ui_cancel.reply_to.set(None); },
                        "×"
                    }
                }
            }
            if let Some((_, ref body)) = edit_event {
                div { style: "display: flex; align-items: center; padding: 4px 16px; font-size: 12px; font-family: var(--font-mono); color: var(--text-dim); background: var(--bg-deep); border-left: 2px solid var(--yellow); margin: 0 12px;",
                    span { style: "color: var(--yellow); margin-right: 6px;", "editing:" }
                    span { style: "flex: 1; overflow: hidden; text-overflow: ellipsis; white-space: nowrap;",
                        { body.chars().take(80).collect::<String>() }
                    }
                    span { style: "cursor: pointer; padding: 0 4px; color: var(--text-dim);",
                        onclick: move |_| { ui_cancel2.edit_event.set(None); text.set(String::new()); },
                        "×"
                    }
                }
            }
            // Upload preview bar
            if let Some((ref filename, _, _)) = upload_pending {
                div { style: "display: flex; align-items: center; padding: 4px 16px; font-size: 12px; font-family: var(--font-mono); color: var(--text-dim); background: var(--bg-deep); border-left: 2px solid var(--accent); margin: 0 12px;",
                    span { style: "color: var(--accent); margin-right: 6px;",
                        if *uploading.read() { "uploading:" } else { "attach:" }
                    }
                    span { style: "flex: 1; overflow: hidden; text-overflow: ellipsis; white-space: nowrap;",
                        "{filename}"
                    }
                    if !*uploading.read() {
                        span { style: "cursor: pointer; padding: 0 6px; color: var(--green); font-weight: 600;",
                            onclick: {
                                let handle_upload = handle_upload.clone();
                                move |_| {
                                    let mut ui = consume_context::<UiState>();
                                    let pending = {
                                        let val = ui.upload_pending.read().clone();
                                        val
                                    };
                                    if let Some((filename, mime, data)) = pending {
                                        ui.upload_pending.set(None);
                                        handle_upload(filename, mime, data);
                                    }
                                }
                            },
                            "send"
                        }
                        span { style: "cursor: pointer; padding: 0 4px; color: var(--text-dim);",
                            onclick: move |_| {
                                consume_context::<UiState>().upload_pending.set(None);
                            },
                            "×"
                        }
                    }
                }
            }
            // Slash command autocomplete popup
            { let current_text = text();
              let show_cmds = current_text.starts_with('/') && !current_text.contains(' ');
              let filtered: Vec<(String, String, String)> = if show_cmds {
                  commands::autocomplete_entries().into_iter()
                      .filter(|(name, _, _)| name.starts_with(&current_text[..]) || current_text == "/")
                      .collect()
              } else { Vec::new() };
              if !filtered.is_empty() {
                rsx! {
                    div { style: "margin: 0 12px; background: var(--bg-deep); border: 1px solid var(--border); border-radius: 6px 6px 0 0; border-bottom: none; max-height: 200px; overflow-y: auto; padding: 4px 0;",
                        for (name, usage, desc) in filtered.iter() {
                            {
                              let cmd_text = format!("{name} {usage}");
                              let name = name.clone();
                              rsx! {
                                div {
                                    class: "cmd-item",
                                    style: "display: flex; align-items: center; gap: 8px; padding: 4px 12px; cursor: pointer;",
                                    onclick: move |_| {
                                        text.set(format!("{name} "));
                                    },
                                    span { style: "font-family: var(--font-mono); font-size: 13px; color: var(--accent-muted); font-weight: 600; flex-shrink: 0;",
                                        "{cmd_text}"
                                    }
                                    span { style: "font-family: var(--font-mono); font-size: 12px; color: var(--text-dim);",
                                        "{desc}"
                                    }
                                }
                            } }
                        }
                    }
                }
              } else { rsx! {} }
            }
            // Help text display
            if let Some(ref msg) = *help_text.read() {
                div { style: "margin: 0 12px 4px; background: var(--bg-deep); border: 1px solid var(--border); border-radius: 4px; padding: 8px 12px; font-family: var(--font-mono); font-size: 12px; color: var(--text-muted); white-space: pre-wrap; line-height: 1.6; position: relative;",
                    span {
                        style: "position: absolute; top: 4px; right: 8px; cursor: pointer; color: var(--text-dim); font-size: 14px;",
                        onclick: move |_| { help_text.set(None); },
                        "×"
                    }
                    "{msg}"
                }
            }

            // ── The command bar ──────────────────────────────────────
            div { style: "margin: 0 12px 8px; border-radius: 8px; background: var(--bg-deep); border: 1px solid var(--border); transition: border-color 0.15s, box-shadow 0.15s;",
                class: "command-bar",

                // Input row
                div { style: "display: flex; align-items: center; min-height: 44px; padding: 0 4px;",
                    // Attach button
                    span {
                        class: "cmd-btn",
                        style: "color: var(--text-dim); cursor: pointer; padding: 6px 8px; display: inline-flex; align-items: center; flex-shrink: 0; border-radius: 4px; transition: color 0.1s, background 0.1s;",
                        title: "Attach file",
                        onclick: move |_| {
                            spawn(async move {
                                let mut eval = document::eval(r#"
                                    let input = document.createElement('input');
                                    input.type = 'file';
                                    input.accept = 'image/*,video/*,audio/*,application/*';
                                    input.onchange = async () => {
                                        if (input.files.length > 0) {
                                            let file = input.files[0];
                                            let reader = new FileReader();
                                            reader.onload = () => {
                                                let base64 = reader.result.split(',')[1];
                                                dioxus.send(JSON.stringify({
                                                    name: file.name,
                                                    type: file.type || 'application/octet-stream',
                                                    data: base64
                                                }));
                                            };
                                            reader.readAsDataURL(file);
                                        }
                                    };
                                    input.click();
                                "#);
                                if let Ok(json) = eval.recv::<String>().await {
                                    if let Ok(parsed) = serde_json::from_str::<serde_json::Value>(&json) {
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
                                }
                            });
                        },
                        dangerous_inner_html: ICO_PLUS,
                    }

                    // Prompt
                    span { style: "color: {prefix_color}; font-family: var(--font-mono); font-size: 18px; font-weight: 700; flex-shrink: 0; padding: 0 6px; line-height: 1;", "{prefix}" }

                    // Text input
                    input {
                        id: "flux-input",
                        style: "flex: 1; background: transparent; border: none; outline: none; color: var(--text-bright); font-size: 14px; font-family: var(--font-mono); padding: 8px 0; min-width: 0;",
                        r#type: "text",
                        placeholder: "Type a message or /command...",
                        value: "{text}",
                        oninput: {
                            let core = core_typing.clone();
                            let ui = ui_typing.clone();
                            move |e: Event<FormData>| {
                                let val = e.value();
                                text.set(val.clone());
                                if !val.is_empty() && !*typing_sent.read() {
                                    typing_sent.set(true);
                                    let core = core.clone();
                                    if let Some(room_id) = ui.selected_room_id.peek().clone() {
                                        spawn(async move {
                                            let client = match core.get_client().await { Ok(c) => c, Err(_) => return };
                                            let _ = fluxchat_core::messages::send_typing(&client, &room_id, true).await;
                                        });
                                    }
                                } else if val.is_empty() && *typing_sent.read() {
                                    typing_sent.set(false);
                                    let core = core.clone();
                                    if let Some(room_id) = ui.selected_room_id.peek().clone() {
                                        spawn(async move {
                                            let client = match core.get_client().await { Ok(c) => c, Err(_) => return };
                                            let _ = fluxchat_core::messages::send_typing(&client, &room_id, false).await;
                                        });
                                    }
                                }
                            }
                        },
                        onkeydown: {
                            let mut ui = ui_cancel3.clone();
                            move |e: Event<KeyboardData>| {
                                let val = text();
                                let in_autocomplete = val.starts_with('/') && !val.contains(' ');

                                if e.key() == Key::Tab && in_autocomplete {
                                    let all = commands::autocomplete_entries();
                                    let filtered: Vec<_> = all.iter()
                                        .filter(|(name, _, _)| name.starts_with(&val[..]) || val == "/")
                                        .collect();
                                    if !filtered.is_empty() {
                                        text.set(format!("{} ", filtered[0].0));
                                    }
                                    return;
                                }

                                if e.key() == Key::Enter && !e.modifiers().shift() {
                                    if in_autocomplete {
                                        let all = commands::autocomplete_entries();
                                        let filtered: Vec<_> = all.iter()
                                            .filter(|(name, _, _)| name.starts_with(&val[..]) || val == "/")
                                            .collect();
                                        if filtered.len() == 1 {
                                            text.set(format!("{} ", filtered[0].0));
                                            return;
                                        }
                                    }
                                    handle_send(());
                                } else if e.key() == Key::Escape {
                                    ui.reply_to.set(None);
                                    ui.edit_event.set(None);
                                    text.set(String::new());
                                    help_text.set(None);
                                }
                            }
                        },
                    }

                    // Emoji button
                    div { style: "position: relative; flex-shrink: 0;",
                        span {
                            class: "cmd-btn",
                            style: "color: var(--text-dim); cursor: pointer; padding: 6px 8px; display: inline-flex; align-items: center; border-radius: 4px; transition: color 0.1s, background 0.1s;",
                            title: "Emoji",
                            onclick: move |_| { let v = *emoji_open.read(); emoji_open.set(!v); },
                            dangerous_inner_html: ICO_EMOJI,
                        }
                        if *emoji_open.read() {
                            div { style: "position: absolute; bottom: 100%; right: 0; margin-bottom: 8px; z-index: 100;",
                                EmojiPicker {
                                    on_select: move |emoji: String| {
                                        let current = text();
                                        text.set(format!("{current}{emoji}"));
                                        emoji_open.set(false);
                                    },
                                }
                            }
                        }
                    }
                }

                // Formatting toolbar — inside the bar, subtle divider
                div { style: "display: flex; align-items: center; gap: 1px; padding: 0 8px 4px; border-top: 1px solid rgba(255,255,255,0.03);",
                    span {
                        class: "cmd-btn",
                        style: "color: var(--text-dim); cursor: pointer; padding: 3px 6px; display: inline-flex; align-items: center; border-radius: 3px; transition: color 0.1s, background 0.1s;",
                        title: "Bold",
                        onclick: make_fmt_handler("**", "**"),
                        dangerous_inner_html: ICO_BOLD,
                    }
                    span {
                        class: "cmd-btn",
                        style: "color: var(--text-dim); cursor: pointer; padding: 3px 6px; display: inline-flex; align-items: center; border-radius: 3px; transition: color 0.1s, background 0.1s;",
                        title: "Italic",
                        onclick: make_fmt_handler("*", "*"),
                        dangerous_inner_html: ICO_ITALIC,
                    }
                    span {
                        class: "cmd-btn",
                        style: "color: var(--text-dim); cursor: pointer; padding: 3px 6px; display: inline-flex; align-items: center; border-radius: 3px; transition: color 0.1s, background 0.1s;",
                        title: "Strikethrough",
                        onclick: make_fmt_handler("~~", "~~"),
                        dangerous_inner_html: ICO_STRIKE,
                    }
                    span {
                        class: "cmd-btn",
                        style: "color: var(--text-dim); cursor: pointer; padding: 3px 6px; display: inline-flex; align-items: center; border-radius: 3px; transition: color 0.1s, background 0.1s;",
                        title: "Inline code",
                        onclick: make_fmt_handler("`", "`"),
                        dangerous_inner_html: ICO_CODE,
                    }
                    span {
                        class: "cmd-btn",
                        style: "color: var(--text-dim); cursor: pointer; padding: 3px 6px; display: inline-flex; align-items: center; border-radius: 3px; transition: color 0.1s, background 0.1s;",
                        title: "Code block",
                        onclick: make_fmt_handler("```\n", "\n```"),
                        dangerous_inner_html: ICO_CODEBLOCK,
                    }
                    span {
                        class: "cmd-btn",
                        style: "color: var(--text-dim); cursor: pointer; padding: 3px 6px; display: inline-flex; align-items: center; border-radius: 3px; transition: color 0.1s, background 0.1s;",
                        title: "Link",
                        onclick: make_fmt_handler("[", "](url)"),
                        dangerous_inner_html: ICO_LINK,
                    }
                }
            }
        }
    }
}
