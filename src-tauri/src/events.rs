use std::time::Duration;
use matrix_sdk::{
    config::SyncSettings,
    ruma::events::{
        reaction::OriginalSyncReactionEvent,
        room::message::{MessageType, OriginalSyncRoomMessageEvent, Relation},
        typing::SyncTypingEvent,
    },
    Client, Room,
};
use tauri::{AppHandle, Emitter};
use tracing::{error, info};

use crate::commands::MessageInfo;

#[derive(serde::Serialize, Clone)]
struct NewMessagePayload {
    room_id: String,
    message: MessageInfo,
}

#[derive(serde::Serialize, Clone)]
struct ReactionPayload {
    room_id: String,
    event_id: String,
    emoji: String,
    sender: String,
}

#[derive(serde::Serialize, Clone)]
struct TypingPayload {
    room_id: String,
    user_ids: Vec<String>,
}

pub fn spawn_sync(client: Client, app_handle: AppHandle) {
    tokio::spawn(async move {
        // Register verification handlers
        crate::commands::verification::register_verification_handlers(&client, app_handle.clone());

        // Register message handler
        client.add_event_handler({
            let handle = app_handle.clone();
            move |ev: OriginalSyncRoomMessageEvent, room: Room| {
                let handle = handle.clone();
                async move {
                    let (body, formatted_body, image_mxc) = match &ev.content.msgtype {
                        MessageType::Text(t) => (
                            t.body.clone(),
                            t.formatted.as_ref().map(|f| f.body.clone()),
                            None,
                        ),
                        MessageType::Image(img) => {
                            let mxc = match &img.source {
                                matrix_sdk::ruma::events::room::MediaSource::Plain(uri) => {
                                    Some(uri.to_string())
                                }
                                _ => None,
                            };
                            (img.body.clone(), None, mxc)
                        }
                        MessageType::Video(_) => ("[video]".into(), None, None),
                        MessageType::Audio(_) => ("[audio]".into(), None, None),
                        MessageType::File(_) => ("[file]".into(), None, None),
                        MessageType::Notice(t) => (
                            t.body.clone(),
                            t.formatted.as_ref().map(|f| f.body.clone()),
                            None,
                        ),
                        MessageType::Emote(t) => (
                            format!("* {}", t.body),
                            t.formatted.as_ref().map(|f| format!("* {}", f.body)),
                            None,
                        ),
                        _ => ("[unsupported message]".into(), None, None),
                    };

                    let member = room
                        .get_member_no_sync(&ev.sender)
                        .await
                        .ok()
                        .flatten();

                    let sender_name = member
                        .as_ref()
                        .and_then(|m| m.display_name().map(|s| s.to_string()))
                        .unwrap_or_else(|| ev.sender.to_string());

                    let avatar_url = member
                        .as_ref()
                        .and_then(|m| m.avatar_url())
                        .map(|mxc| mxc.to_string());

                    // Extract reply info
                    let reply_to_id = match &ev.content.relates_to {
                        Some(Relation::Reply { in_reply_to }) => {
                            Some(in_reply_to.event_id.to_string())
                        }
                        Some(Relation::Thread(thread)) => {
                            thread.in_reply_to.as_ref().map(|r| r.event_id.to_string())
                        }
                        _ => None,
                    };

                    let msg = MessageInfo {
                        event_id: ev.event_id.to_string(),
                        sender: ev.sender.to_string(),
                        sender_name,
                        body,
                        formatted_body,
                        image_mxc,
                        reactions: Vec::new(),
                        reply_to_id,
                        reply_to_sender: None,
                        reply_to_body: None,
                        timestamp: ev.origin_server_ts.as_secs().into(),
                        avatar_url,
                    };

                    let payload = NewMessagePayload {
                        room_id: room.room_id().to_string(),
                        message: msg,
                    };

                    if let Err(e) = handle.emit("matrix://sync-room-message", &payload) {
                        error!("Failed to emit message event: {e}");
                    }
                }
            }
        });

        // Register typing handler
        let own_user_id = client.user_id().map(|u| u.to_owned());
        client.add_event_handler({
            let handle = app_handle.clone();
            move |ev: SyncTypingEvent, room: Room| {
                let handle = handle.clone();
                let own_id = own_user_id.clone();
                async move {
                    let user_ids: Vec<String> = ev
                        .content
                        .user_ids
                        .iter()
                        .filter(|u| own_id.as_ref().map_or(true, |own| *u != own))
                        .map(|u| u.to_string())
                        .collect();
                    let payload = TypingPayload {
                        room_id: room.room_id().to_string(),
                        user_ids,
                    };
                    let _ = handle.emit("matrix://typing-indicator", &payload);
                }
            }
        });

        // Register reaction handler
        client.add_event_handler({
            let handle = app_handle.clone();
            move |ev: OriginalSyncReactionEvent, room: Room| {
                let handle = handle.clone();
                async move {
                    let payload = ReactionPayload {
                        room_id: room.room_id().to_string(),
                        event_id: ev.content.relates_to.event_id.to_string(),
                        emoji: ev.content.relates_to.key.clone(),
                        sender: ev.sender.to_string(),
                    };
                    let _ = handle.emit("matrix://reaction", &payload);
                }
            }
        });

        info!("Starting initial sync...");

        let sync_settings = SyncSettings::default().timeout(Duration::from_secs(10));

        match client.sync_once(sync_settings.clone()).await {
            Ok(_) => {
                info!("Initial sync complete");
                let _ = app_handle.emit("matrix://room-list-update", ());
            }
            Err(e) => {
                error!("Initial sync failed: {e}");
                return;
            }
        }

        info!("Starting continuous sync...");
        client
            .sync(sync_settings)
            .await
            .unwrap_or_else(|e| {
                error!("Sync loop ended with error: {e}");
            });
    });
}
