//! Keeps the SDK send queue moving.
//!
//! The SDK disables a room's send queue after *any* failed request (a
//! message, an edit, a reaction, a redaction) and leaves turning it back on to
//! the app. Until then every later message in that room stays "sending…". So:
//! unsent requests from earlier sessions are respawned at start; after an
//! unrecoverable error (that request is parked as failed and never picked
//! again) the room's queue is re-enabled at once; after a recoverable one
//! (network, server busy), after a backoff that grows while the room keeps
//! failing.

use std::collections::HashMap;
use std::time::{Duration, Instant};

use matrix_sdk::Client;
use matrix_sdk_ui::timeline::TimelineEventItemId;
use tokio::sync::broadcast::error::RecvError;

use crate::error::AppError;

const FIRST_RETRY: Duration = Duration::from_secs(5);
const MAX_RETRY: Duration = Duration::from_secs(120);
/// A room that hasn't failed for this long starts again at `FIRST_RETRY`.
const RETRY_RESET_AFTER: Duration = Duration::from_secs(600);

/// Respawn unsent requests and keep re-enabling room queues after errors.
/// Runs until the client is dropped.
pub fn spawn_send_queue_keeper(client: &Client) {
    let client = client.clone();
    tokio::spawn(async move {
        let queue = client.send_queue();
        let mut errors = queue.subscribe_errors();
        queue.respawn_tasks_for_rooms_with_unsent_requests().await;
        // Per room: next delay and when it last failed.
        let mut backoff: HashMap<String, (Duration, Instant)> = HashMap::new();
        loop {
            let err = match errors.recv().await {
                Ok(err) => err,
                Err(RecvError::Lagged(_)) => continue,
                Err(RecvError::Closed) => break,
            };
            let Some(room) = client.get_room(&err.room_id) else { continue };
            let delay = if err.is_recoverable {
                let entry = backoff.entry(err.room_id.to_string()).or_insert((FIRST_RETRY, Instant::now()));
                if entry.1.elapsed() > RETRY_RESET_AFTER {
                    entry.0 = FIRST_RETRY;
                }
                let delay = entry.0;
                *entry = ((delay * 2).min(MAX_RETRY), Instant::now());
                delay
            } else {
                Duration::ZERO
            };
            tracing::warn!(
                "Send failed in {} ({}): {}; resuming its queue in {}s",
                err.room_id, if err.is_recoverable { "will retry" } else { "parked" }, err.error, delay.as_secs()
            );
            tokio::spawn(async move {
                tokio::time::sleep(delay).await;
                room.send_queue().set_enabled(true);
            });
        }
    });
}

/// Abort a message that hasn't been sent yet (or failed to send): the local
/// echo with transaction ID `txn_id` in the view's timeline (`view_key`: a
/// room or thread key, whichever timeline it was sent through).
pub async fn cancel_send(timelines: &crate::timeline::RoomTimelines, view_key: &str, txn_id: &str) -> Result<(), AppError> {
    let timeline = timelines.get(view_key).await
        .ok_or_else(|| AppError::Other("That view isn't loaded".into()))?;
    let id = TimelineEventItemId::TransactionId(txn_id.into());
    timeline.redact(&id, None).await
        .map_err(|e| AppError::Other(format!("Couldn't cancel sending: {e}")))
}
