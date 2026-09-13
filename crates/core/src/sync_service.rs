use matrix_sdk::Client;
use matrix_sdk::ruma::api::error::ErrorKind;
use matrix_sdk_ui::sync_service::{Error as SyncServiceError, SyncService};
use matrix_sdk_ui::room_list_service::Error as RoomListError;
use tracing::info;

use std::collections::HashMap;

use crate::error::AppError;

/// How long the room list watcher waits for further changes before rebuilding.
const ROOM_LIST_DEBOUNCE: std::time::Duration = std::time::Duration::from_millis(300);
use crate::models::RoomInfo;
use crate::rooms::SpaceNode;

/// Create and start the SyncService (uses Sliding Sync).
pub async fn create_sync_service(client: &Client) -> Result<SyncService, AppError> {
    let service = SyncService::builder(client.clone())
        .build()
        .await
        .map_err(|e| AppError::Other(format!("Failed to build SyncService: {e}")))?;

    info!("SyncService created, starting...");
    service.start().await;
    info!("SyncService started");

    Ok(service)
}

/// Returns true if a sync-service error was caused by the server rejecting
/// our access token (`M_UNKNOWN_TOKEN`). Such errors are terminal — no amount
/// of reconnecting will fix them; the session must be logged out.
pub fn is_invalid_token_error(err: &SyncServiceError) -> bool {
    let SyncServiceError::RoomList(RoomListError::SlidingSync(sdk_err)) = err else {
        return false;
    };
    matches!(sdk_err.client_api_error_kind(), Some(ErrorKind::UnknownToken(_)))
}

/// Upgrade the sliding-sync subscription for a specific room so we receive
/// more than the default 1 timeline event per sync response. Without this,
/// bursts of rapid events (e.g. a bot redacting a reaction and sending a
/// replacement in the same tick) get truncated — the redact gets dropped,
/// leaving stale aggregation state on the in-memory Timeline.
pub async fn subscribe_to_room(
    sync_service: &SyncService,
    room_id: &matrix_sdk::ruma::RoomId,
) {
    sync_service.room_list_service().subscribe_to_rooms(&[room_id]).await;
}

/// Spawn a background task that watches the RoomListService for changes
/// and calls the provided callback with updated room lists.
pub async fn spawn_room_list_watcher<F>(
    client: &Client,
    sync_service: &SyncService,
    on_update: F,
) where
    F: Fn(Vec<RoomInfo>, Vec<RoomInfo>, Vec<RoomInfo>, Vec<RoomInfo>, Vec<SpaceNode>, HashMap<String, Vec<String>>) + Send + 'static,
{
    let room_list_service = sync_service.room_list_service();
    let client = client.clone();

    tokio::spawn(async move {
        let all_rooms = match room_list_service.all_rooms().await {
            Ok(rooms) => rooms,
            Err(e) => {
                tracing::warn!("Failed to get RoomListService all_rooms: {e}");
                return;
            }
        };

        // The stream only yields after set_filter is called on the controller.
        // Use `new_filter_non_left` (everything we're still a member of) —
        // this is the natural "all rooms" set. The controller must be kept
        // alive for the lifetime of the stream.
        let (stream, controller) = all_rooms.entries_with_dynamic_adapters(100);
        controller.set_filter(Box::new(
            matrix_sdk_ui::room_list_service::filters::new_filter_non_left(),
        ));

        use futures_util::StreamExt;
        futures_util::pin_mut!(stream);

        while let Some(_diffs) = stream.next().await {
            // A single sync response usually yields several diff batches (and
            // the list re-sorts on every message in any room). Rebuilding reads
            // state for every room, so collect a burst and rebuild once.
            let debounce = tokio::time::sleep(ROOM_LIST_DEBOUNCE);
            tokio::pin!(debounce);
            let mut ended = false;
            loop {
                tokio::select! {
                    _ = &mut debounce => break,
                    more = stream.next() => if more.is_none() { ended = true; break; },
                }
            }

            // Rebuild from current client state
            let rooms = crate::rooms::get_rooms(&client).await.unwrap_or_default();
            let spaces = crate::rooms::get_spaces(&client).await.unwrap_or_default();
            let invited = crate::rooms::get_invited_rooms(&client).await.unwrap_or_default();
            let knocked = crate::rooms::get_knocked_rooms(&client).await.unwrap_or_default();
            let (space_tree, space_descendants) = crate::rooms::get_space_tree(&client).await;
            on_update(rooms, spaces, invited, knocked, space_tree, space_descendants);
            if ended {
                break;
            }
        }
        drop(controller);
        tracing::warn!("Room list watcher stream ended");
    });

    info!("Room list watcher spawned");
}
