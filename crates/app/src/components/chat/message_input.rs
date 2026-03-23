use dioxus::prelude::*;
use std::sync::Arc;
use fluxchat_core::AppState;

use crate::state::UiState;

#[component]
pub fn MessageInput() -> Element {
    let core = use_context::<Arc<AppState>>();
    let mut ui = use_context::<UiState>();
    let mut text = use_signal(|| String::new());
    let mut sending = use_signal(|| false);

    let reply_to = ui.reply_to.read().clone();
    let edit_event = ui.edit_event.read().clone();

    // When entering edit mode, populate the input with the original text
    use_effect(move || {
        let edit = ui.edit_event.read().clone();
        if let Some((_, ref body)) = edit {
            text.set(body.clone());
        }
    });

    let mut ui_cancel = ui.clone();
    let mut ui_cancel2 = ui.clone();
    let mut ui_cancel3 = ui.clone();
    let mut handle_send = move |_: ()| {
        let body = text().trim().to_string();
        if body.is_empty() || *sending.read() { return; }
        let room_id = match ui.selected_room_id.read().clone() { Some(id) => id, None => return };
        let core = core.clone();
        let reply = ui.reply_to.read().clone();
        let edit = ui.edit_event.read().clone();

        text.set(String::new());
        ui.reply_to.set(None);
        ui.edit_event.set(None);

        if let Some((event_id, _)) = edit {
            // Edit mode
            let body = body.clone();
            spawn(async move {
                sending.set(true);
                let client = match core.get_client().await { Ok(c) => c, Err(_) => { sending.set(false); return; } };
                let _ = fluxchat_core::messages::edit_message(&client, &room_id, &event_id, &body).await;
                sending.set(false);
            });
        } else if let Some((event_id, _, _)) = reply {
            // Reply mode
            let mut ui = ui.clone();
            let user_id = ui.user_id.read().clone();
            let temp_id = format!("~local-{}", std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap_or_default().as_millis());
            let optimistic = fluxchat_core::MessageInfo {
                event_id: temp_id, sender: user_id.clone(),
                sender_name: user_id.split(':').next().unwrap_or("").trim_start_matches('@').to_string(),
                body: body.clone(), formatted_body: None, image_mxc: None, reactions: Vec::new(),
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
            // Normal send
            let mut ui = ui.clone();
            let user_id = ui.user_id.read().clone();
            let temp_id = format!("~local-{}", std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap_or_default().as_millis());
            let optimistic = fluxchat_core::MessageInfo {
                event_id: temp_id, sender: user_id.clone(),
                sender_name: user_id.split(':').next().unwrap_or("").trim_start_matches('@').to_string(),
                body: body.clone(), formatted_body: None, image_mxc: None, reactions: Vec::new(),
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
            // Input row
            div { style: "display: flex; align-items: center; height: 40px; margin: 4px 12px 8px; border-radius: 6px; background: var(--bg-deep); border: 1px solid var(--border);",
                span { style: "color: {prefix_color}; font-family: var(--font-mono); font-size: 15px; font-weight: 700; padding: 0 10px; flex-shrink: 0; opacity: 0.6;", "{prefix}" }
                input {
                    style: "flex: 1; background: transparent; border: none; outline: none; color: var(--text-bright); font-size: 14px; font-family: var(--font-mono); padding-right: 12px;",
                    r#type: "text",
                    value: "{text}",
                    oninput: move |e| text.set(e.value()),
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
