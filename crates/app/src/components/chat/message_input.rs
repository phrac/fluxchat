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

    let room_name = ui.selected_room_id.read().as_ref()
        .and_then(|id| ui.rooms.read().iter().find(|r| r.room_id == *id).map(|r| r.name.clone()))
        .unwrap_or_default();

    let mut handle_send = move |_: ()| {
        let body = text().trim().to_string();
        if body.is_empty() || *sending.read() { return; }
        let room_id = match ui.selected_room_id.read().clone() { Some(id) => id, None => return };
        let core = core.clone();
        let mut ui = ui.clone();

        // Optimistic insert
        let temp_id = format!("~local-{}", std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap_or_default().as_millis());
        let user_id = ui.user_id.read().clone();
        let optimistic = fluxchat_core::MessageInfo {
            event_id: temp_id, sender: user_id.clone(),
            sender_name: user_id.split(':').next().unwrap_or("").trim_start_matches('@').to_string(),
            body: body.clone(), formatted_body: None, image_mxc: None, reactions: Vec::new(),
            reply_to_id: None, reply_to_sender: None, reply_to_body: None,
            timestamp: std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap_or_default().as_secs(),
            avatar_url: None,
        };
        ui.messages_by_room.write().entry(room_id.clone()).or_insert_with(Vec::new).push(optimistic);
        text.set(String::new());

        spawn(async move {
            sending.set(true);
            let client = match core.get_client().await { Ok(c) => c, Err(_) => { sending.set(false); return; } };
            let _ = fluxchat_core::messages::send_message(&client, &room_id, &body).await;
            sending.set(false);
        });
    };

    rsx! {
        div { style: "display: flex; align-items: center; height: 40px; margin: 4px 12px 8px; border-radius: 6px; background: var(--bg-deep); border: 1px solid var(--border); flex-shrink: 0;",
            span { style: "color: var(--accent); font-family: var(--font-mono); font-size: 15px; font-weight: 700; padding: 0 10px; flex-shrink: 0; opacity: 0.6;", ">" }
            input {
                style: "flex: 1; background: transparent; border: none; outline: none; color: var(--text-bright); font-size: 14px; font-family: var(--font-mono); padding-right: 12px;",
                r#type: "text",
                value: "{text}",
                oninput: move |e| text.set(e.value()),
                onkeydown: move |e| {
                    if e.key() == Key::Enter && !e.modifiers().shift() {
                        handle_send(());
                    }
                },
            }
        }
    }
}
