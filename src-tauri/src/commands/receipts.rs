use tauri::State;

use crate::state::AppState;
use super::AppError;

async fn get_client(state: &State<'_, AppState>) -> Result<matrix_sdk::Client, AppError> {
    let lock = state.client.lock().await;
    lock.as_ref().cloned().ok_or(AppError::NotLoggedIn)
}

/// Send a read receipt for the given event in the given room.
#[tauri::command]
pub async fn send_read_receipt(
    state: State<'_, AppState>,
    room_id: String,
    event_id: String,
) -> Result<(), AppError> {
    let client = get_client(&state).await?;

    let room_id = matrix_sdk::ruma::RoomId::parse(&room_id)
        .map_err(|_| AppError::Other("Invalid room ID".into()))?;

    let room = client
        .get_room(&room_id)
        .ok_or(AppError::RoomNotFound(room_id.to_string()))?;

    let event_id = matrix_sdk::ruma::EventId::parse(&event_id)
        .map_err(|_| AppError::Other("Invalid event ID".into()))?;

    // Send both public read receipt and fully-read marker
    let receipts = matrix_sdk::room::Receipts::new()
        .fully_read_marker(Some(event_id.clone()))
        .public_read_receipt(Some(event_id));

    room.send_multiple_receipts(receipts)
        .await
        .map_err(|e| AppError::Other(format!("Failed to send receipt: {e}")))?;

    Ok(())
}
