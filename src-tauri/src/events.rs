use matrix_sdk::{
    config::SyncSettings,
    ruma::events::room::message::{MessageType, OriginalSyncRoomMessageEvent},
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
                                matrix_sdk::ruma::events::room::MediaSource::Plain(uri) => Some(uri.to_string()),
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

                    let msg = MessageInfo {
                        event_id: ev.event_id.to_string(),
                        sender: ev.sender.to_string(),
                        sender_name,
                        body,
                        formatted_body,
                        image_mxc,
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

        info!("Starting initial sync...");

        // Initial sync
        match client.sync_once(SyncSettings::default()).await {
            Ok(_) => {
                info!("Initial sync complete");
                let _ = app_handle.emit("matrix://room-list-update", ());
            }
            Err(e) => {
                error!("Initial sync failed: {e}");
                return;
            }
        }

        // Continuous sync — sync() manages the token internally
        info!("Starting continuous sync...");
        client
            .sync(SyncSettings::default())
            .await
            .unwrap_or_else(|e| {
                error!("Sync loop ended with error: {e}");
            });
    });
}
