use dioxus::prelude::*;
use std::sync::Arc;
use fluxchat_core::AppState;

use crate::state::UiState;
use crate::components::chat::emoji_picker::EmojiPicker;

const ICO_PLUS: &str = r#"<svg width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2.5" stroke-linecap="round"><line x1="12" y1="5" x2="12" y2="19"/><line x1="5" y1="12" x2="19" y2="12"/></svg>"#;

const SLASH_COMMANDS: &[(&str, &str, &str)] = &[
    ("/join", "<room>", "Join a room by alias or ID"),
    ("/leave", "", "Leave the current room"),
    ("/invite", "<user>", "Invite a user to this room"),
    ("/kick", "<user> [reason]", "Kick a user from this room"),
    ("/ban", "<user> [reason]", "Ban a user from this room"),
    ("/topic", "<text>", "Set the room topic"),
    ("/nick", "<name>", "Set the room display name"),
    ("/me", "<action>", "Send an action message"),
    ("/shrug", "[message]", "Append ¯\\_(ツ)_/¯"),
    ("/tableflip", "[message]", "Append (╯°□°)╯︵ ┻━┻"),
    ("/unflip", "[message]", "Append ┬─┬ノ( º _ ºノ)"),
    ("/lenny", "[message]", "Append ( ͡° ͜ʖ ͡°)"),
    ("/spoiler", "<text>", "Send a spoiler message"),
];

#[component]
pub fn MessageInput() -> Element {
    let core = use_context::<Arc<AppState>>();
    let mut ui = use_context::<UiState>();
    let mut text = use_signal(|| String::new());
    let mut sending = use_signal(|| false);
    let mut uploading = use_signal(|| false);
    let mut typing_sent = use_signal(|| false);
    let mut emoji_open = use_signal(|| false);

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

        // Handle slash commands
        if body.starts_with('/') {
            let room_id = match ui.selected_room_id.read().clone() { Some(id) => id, None => return };
            let core = core.clone();
            let parts: Vec<&str> = body.splitn(2, ' ').collect();
            let cmd = parts[0];
            let args = parts.get(1).unwrap_or(&"").trim().to_string();

            match cmd {
                "/join" => {
                    if !args.is_empty() {
                        text.set(String::new());
                        let core = core.clone();
                        spawn(async move {
                            let client = match core.get_client().await { Ok(c) => c, Err(_) => return };
                            let _ = fluxchat_core::rooms::join_room(&client, &args).await;
                        });
                        return;
                    }
                }
                "/leave" => {
                    text.set(String::new());
                    let core = core.clone();
                    let rid = room_id.clone();
                    spawn(async move {
                        let client = match core.get_client().await { Ok(c) => c, Err(_) => return };
                        let _ = fluxchat_core::rooms::leave_room(&client, &rid).await;
                    });
                    return;
                }
                "/invite" => {
                    if !args.is_empty() {
                        text.set(String::new());
                        let core = core.clone();
                        let rid = room_id.clone();
                        spawn(async move {
                            let client = match core.get_client().await { Ok(c) => c, Err(_) => return };
                            let _ = fluxchat_core::rooms::invite_user(&client, &rid, &args).await;
                        });
                        return;
                    }
                }
                "/kick" => {
                    if !args.is_empty() {
                        text.set(String::new());
                        let core = core.clone();
                        let rid = room_id.clone();
                        let kparts: Vec<&str> = args.splitn(2, ' ').collect();
                        let user = kparts[0].to_string();
                        let reason = kparts.get(1).map(|s| s.to_string());
                        spawn(async move {
                            let client = match core.get_client().await { Ok(c) => c, Err(_) => return };
                            let _ = fluxchat_core::rooms::kick_user(&client, &rid, &user, reason.as_deref()).await;
                        });
                        return;
                    }
                }
                "/ban" => {
                    if !args.is_empty() {
                        text.set(String::new());
                        let core = core.clone();
                        let rid = room_id.clone();
                        let bparts: Vec<&str> = args.splitn(2, ' ').collect();
                        let user = bparts[0].to_string();
                        let reason = bparts.get(1).map(|s| s.to_string());
                        spawn(async move {
                            let client = match core.get_client().await { Ok(c) => c, Err(_) => return };
                            let _ = fluxchat_core::rooms::ban_user(&client, &rid, &user, reason.as_deref()).await;
                        });
                        return;
                    }
                }
                "/topic" => {
                    if !args.is_empty() {
                        text.set(String::new());
                        let core = core.clone();
                        let rid = room_id.clone();
                        spawn(async move {
                            let client = match core.get_client().await { Ok(c) => c, Err(_) => return };
                            let _ = fluxchat_core::rooms::set_room_topic(&client, &rid, &args).await;
                        });
                        return;
                    }
                }
                "/nick" => {
                    if !args.is_empty() {
                        text.set(String::new());
                        let core = core.clone();
                        spawn(async move {
                            let client = match core.get_client().await { Ok(c) => c, Err(_) => return };
                            let _ = fluxchat_core::profile::set_display_name(&client, &args).await;
                        });
                        return;
                    }
                }
                "/me" => {
                    body = format!("* {args}");
                    // Fall through to send as normal message
                }
                "/shrug" => {
                    body = if args.is_empty() { "¯\\_(ツ)_/¯".into() } else { format!("{args} ¯\\_(ツ)_/¯") };
                }
                "/tableflip" => {
                    body = if args.is_empty() { "(╯°□°)╯︵ ┻━┻".into() } else { format!("{args} (╯°□°)╯︵ ┻━┻") };
                }
                "/unflip" => {
                    body = if args.is_empty() { "┬─┬ノ( º _ ºノ)".into() } else { format!("{args} ┬─┬ノ( º _ ºノ)") };
                }
                "/lenny" => {
                    body = if args.is_empty() { "( ͡° ͜ʖ ͡°)".into() } else { format!("{args} ( ͡° ͜ʖ ͡°)") };
                }
                "/spoiler" => {
                    if !args.is_empty() {
                        body = args;
                        // TODO: wrap in spoiler tags when formatted_body is supported for sending
                    }
                }
                _ => {
                    // Unknown command — send as plain text
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
        ("ed>", "var(--yellow)")
    } else if reply_to.is_some() {
        ("re>", "var(--green)")
    } else {
        (">", "var(--accent)")
    };

    rsx! {
        div { style: "flex-shrink: 0;",
            // Reply/edit context bar
            if let Some((_, ref name, ref body)) = reply_to {
                div { style: "display: flex; align-items: center; padding: 2px 12px; font-size: 12px; font-family: var(--font-mono); color: var(--text-dim); background: var(--bg-deep); border-left: 2px solid var(--green); margin: 0 12px;",
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
                div { style: "display: flex; align-items: center; padding: 2px 12px; font-size: 12px; font-family: var(--font-mono); color: var(--text-dim); background: var(--bg-deep); border-left: 2px solid var(--yellow); margin: 0 12px;",
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
                div { style: "display: flex; align-items: center; padding: 2px 12px; font-size: 12px; font-family: var(--font-mono); color: var(--text-dim); background: var(--bg-deep); border-left: 2px solid var(--accent); margin: 0 12px;",
                    span { style: "color: var(--accent); margin-right: 6px;",
                        if *uploading.read() { "uploading:" } else { "attach:" }
                    }
                    span { style: "flex: 1; overflow: hidden; text-overflow: ellipsis; white-space: nowrap;",
                        "{filename}"
                    }
                    if !*uploading.read() {
                        // Send the upload
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
              let cmd_filter = if show_cmds { &current_text[..] } else { "" };
              let filtered: Vec<_> = if show_cmds {
                  SLASH_COMMANDS.iter()
                      .filter(|(name, _, _)| name.starts_with(cmd_filter) || cmd_filter == "/")
                      .collect()
              } else { Vec::new() };
              if !filtered.is_empty() {
                rsx! {
                    div { style: "margin: 0 12px; background: var(--bg-deep); border: 1px solid var(--border); border-radius: 6px 6px 0 0; border-bottom: none; max-height: 200px; overflow-y: auto; padding: 4px 0;",
                        for (_i, (name, usage, desc)) in filtered.iter().enumerate() {
                            {
                              let cmd_text = format!("{name} {usage}");
                              rsx! {
                                div {
                                    class: "cmd-item",
                                    style: "display: flex; align-items: center; gap: 8px; padding: 4px 12px; cursor: pointer;",
                                    onclick: move |_| {
                                        text.set(format!("{name} "));
                                    },
                                    span { style: "font-family: var(--font-mono); font-size: 13px; color: var(--accent); font-weight: 600; flex-shrink: 0;",
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
            // Input row
            div { style: "display: flex; align-items: center; height: 40px; margin: 4px 12px 8px; border-radius: 6px; background: var(--bg-deep); border: 1px solid var(--border);",
                // Attach button
                span {
                    style: "color: var(--text-dim); cursor: pointer; padding: 0 8px; display: inline-flex; align-items: center; flex-shrink: 0;",
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
                span { style: "color: {prefix_color}; font-family: var(--font-mono); font-size: 15px; font-weight: 700; flex-shrink: 0; opacity: 0.6; padding-right: 8px;", "{prefix}" }
                input {
                    style: "flex: 1; background: transparent; border: none; outline: none; color: var(--text-bright); font-size: 14px; font-family: var(--font-mono); padding-right: 8px;",
                    r#type: "text",
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
                                // Accept first matching command on Tab
                                let filtered: Vec<_> = SLASH_COMMANDS.iter()
                                    .filter(|(name, _, _)| name.starts_with(&val[..]) || val == "/")
                                    .collect();
                                if !filtered.is_empty() {
                                    text.set(format!("{} ", filtered[0].0));
                                }
                                return;
                            }

                            if e.key() == Key::Enter && !e.modifiers().shift() {
                                // If in autocomplete, accept highlighted command
                                if in_autocomplete {
                                    let filtered: Vec<_> = SLASH_COMMANDS.iter()
                                        .filter(|(name, _, _)| name.starts_with(&val[..]) || val == "/")
                                        .collect();
                                    if filtered.len() == 1 {
                                        // Exact single match — accept it
                                        text.set(format!("{} ", filtered[0].0));
                                        return;
                                    }
                                }
                                handle_send(());
                            } else if e.key() == Key::Escape {
                                ui.reply_to.set(None);
                                ui.edit_event.set(None);
                                text.set(String::new());
                            }
                        }
                    },
                }
                // Emoji button
                div { style: "position: relative; flex-shrink: 0;",
                    span {
                        style: "color: var(--text-dim); cursor: pointer; padding: 0 8px; display: inline-flex; align-items: center; font-size: 16px;",
                        onclick: move |_| { let v = *emoji_open.read(); emoji_open.set(!v); },
                        "☺"
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
        }
    }
}
