use dioxus::prelude::*;
use std::sync::Arc;
use fluxchat_core::AppState;

use crate::state::UiState;

const ICO_PLUS: &str = r#"<svg width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2.5" stroke-linecap="round"><line x1="12" y1="5" x2="12" y2="19"/><line x1="5" y1="12" x2="19" y2="12"/></svg>"#;

#[component]
pub fn MessageInput() -> Element {
    let core = use_context::<Arc<AppState>>();
    let mut ui = use_context::<UiState>();
    let mut text = use_signal(|| String::new());
    let mut sending = use_signal(|| false);
    let mut uploading = use_signal(|| false);
    let mut typing_sent = use_signal(|| false);

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
        let body = text().trim().to_string();
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

        if let Some((event_id, _)) = edit {
            let body = body.clone();
            spawn(async move {
                sending.set(true);
                let client = match core.get_client().await { Ok(c) => c, Err(_) => { sending.set(false); return; } };
                let _ = fluxchat_core::messages::edit_message(&client, &room_id, &event_id, &body).await;
                sending.set(false);
            });
        } else if let Some((event_id, _, _)) = reply {
            let mut ui = ui.clone();
            let user_id = ui.user_id.read().clone();
            let temp_id = format!("~local-{}", std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap_or_default().as_millis());
            let optimistic = fluxchat_core::MessageInfo {
                event_id: temp_id, sender: user_id.clone(),
                sender_name: user_id.split(':').next().unwrap_or("").trim_start_matches('@').to_string(),
                body: body.clone(), formatted_body: None, transformed_body: None, image_mxc: None, reactions: Vec::new(),
                reply_to_id: Some(event_id.clone()), reply_to_sender: None, reply_to_body: None,
                timestamp: std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap_or_default().as_secs(),
                avatar_url: None,
            };
            ui.messages_by_room.write().entry(room_id.clone()).or_insert_with(Vec::new).push(optimistic);
            spawn(async move {
                sending.set(true);
                let client = match core.get_client().await { Ok(c) => c, Err(_) => { sending.set(false); return; } };
                let _ = fluxchat_core::messages::send_reply(&client, &room_id, &body, &event_id).await;
                sending.set(false);
            });
        } else {
            let mut ui = ui.clone();
            let user_id = ui.user_id.read().clone();
            let temp_id = format!("~local-{}", std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap_or_default().as_millis());
            let optimistic = fluxchat_core::MessageInfo {
                event_id: temp_id, sender: user_id.clone(),
                sender_name: user_id.split(':').next().unwrap_or("").trim_start_matches('@').to_string(),
                body: body.clone(), formatted_body: None, transformed_body: None, image_mxc: None, reactions: Vec::new(),
                reply_to_id: None, reply_to_sender: None, reply_to_body: None,
                timestamp: std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap_or_default().as_secs(),
                avatar_url: None,
            };
            ui.messages_by_room.write().entry(room_id.clone()).or_insert_with(Vec::new).push(optimistic);
            spawn(async move {
                sending.set(true);
                let client = match core.get_client().await { Ok(c) => c, Err(_) => { sending.set(false); return; } };
                let _ = fluxchat_core::messages::send_message(&client, &room_id, &body).await;
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
                            if e.key() == Key::Enter && !e.modifiers().shift() {
                                handle_send(());
                            } else if e.key() == Key::Escape {
                                ui.reply_to.set(None);
                                ui.edit_event.set(None);
                                text.set(String::new());
                            }
                        }
                    },
                }
            }
        }
    }
}
