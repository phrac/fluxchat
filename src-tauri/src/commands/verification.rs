use matrix_sdk::ruma::events::key::verification::request::ToDeviceKeyVerificationRequestEvent;
use serde::Serialize;
use tauri::{AppHandle, Emitter, State};
use tracing::info;

use crate::state::AppState;
use super::AppError;

#[derive(Debug, Serialize, Clone)]
pub struct EmojiInfo {
    pub symbol: String,
    pub description: String,
}

#[derive(Debug, Serialize, Clone)]
pub struct VerificationInfo {
    pub flow_id: String,
    pub other_user_id: String,
    pub is_self_verification: bool,
    pub state: String,
    pub emojis: Option<Vec<EmojiInfo>>,
}

/// Register handlers for incoming verification requests.
/// Called from the sync setup.
pub fn register_verification_handlers(client: &matrix_sdk::Client, app_handle: AppHandle) {
    client.add_event_handler({
        let handle = app_handle.clone();
        let client = client.clone();
        move |ev: ToDeviceKeyVerificationRequestEvent| {
            let handle = handle.clone();
            let client = client.clone();
            async move {
                info!("Incoming verification request from {}", ev.sender);

                let request = client
                    .encryption()
                    .get_verification_request(&ev.sender, &ev.content.transaction_id)
                    .await;

                if let Some(request) = request {
                    let info = VerificationInfo {
                        flow_id: request.flow_id().to_string(),
                        other_user_id: request.other_user_id().to_string(),
                        is_self_verification: request.is_self_verification(),
                        state: "requested".into(),
                        emojis: None,
                    };
                    let _ = handle.emit("matrix://verification-request", &info);
                }
            }
        }
    });
}

/// Accept an incoming verification request, start SAS, and drive the flow.
#[tauri::command]
pub async fn accept_verification(
    state: State<'_, AppState>,
    app_handle: tauri::AppHandle,
    user_id: String,
    flow_id: String,
) -> Result<(), AppError> {
    let lock = state.client.lock().await;
    let client = lock.as_ref().ok_or(AppError::NotLoggedIn)?;

    let user_id = matrix_sdk::ruma::UserId::parse(&user_id)
        .map_err(|_| AppError::Other("Invalid user ID".into()))?;

    let request = client
        .encryption()
        .get_verification_request(&user_id, &flow_id)
        .await
        .ok_or(AppError::Other("Verification request not found".into()))?;

    // Accept the request
    request.accept().await?;

    // Start SAS verification
    let sas = request
        .start_sas()
        .await?
        .ok_or(AppError::Other("Failed to start SAS verification".into()))?;

    // Spawn a task to monitor the SAS state and emit emoji when ready
    let handle = app_handle.clone();
    let flow_id_clone = flow_id.clone();
    let other_user = user_id.to_string();

    tokio::spawn(async move {
        // Poll until emojis are available
        for _ in 0..60 {
            tokio::time::sleep(std::time::Duration::from_millis(500)).await;

            if sas.can_be_presented() {
                if let Some(emojis) = sas.emoji() {
                    let emoji_info: Vec<EmojiInfo> = emojis
                        .iter()
                        .map(|e| EmojiInfo {
                            symbol: e.symbol.to_string(),
                            description: e.description.to_string(),
                        })
                        .collect();

                    let info = VerificationInfo {
                        flow_id: flow_id_clone.clone(),
                        other_user_id: other_user.clone(),
                        is_self_verification: sas.is_done(),
                        state: "emojis_ready".into(),
                        emojis: Some(emoji_info),
                    };
                    let _ = handle.emit("matrix://verification-emojis", &info);
                }
                break;
            }

            if sas.is_cancelled() {
                let info = VerificationInfo {
                    flow_id: flow_id_clone.clone(),
                    other_user_id: other_user.clone(),
                    is_self_verification: false,
                    state: "cancelled".into(),
                    emojis: None,
                };
                let _ = handle.emit("matrix://verification-cancelled", &info);
                return;
            }
        }
    });

    Ok(())
}

/// Confirm the emoji match.
#[tauri::command]
pub async fn confirm_verification(
    state: State<'_, AppState>,
    user_id: String,
    flow_id: String,
) -> Result<(), AppError> {
    let lock = state.client.lock().await;
    let client = lock.as_ref().ok_or(AppError::NotLoggedIn)?;

    let user_id = matrix_sdk::ruma::UserId::parse(&user_id)
        .map_err(|_| AppError::Other("Invalid user ID".into()))?;

    let verification = client
        .encryption()
        .get_verification(&user_id, &flow_id)
        .await
        .ok_or(AppError::Other("Verification not found".into()))?;

    if let Some(sas) = verification.sas() {
        sas.confirm().await?;
        info!("Verification confirmed for {}", user_id);
    } else {
        return Err(AppError::Other("Not a SAS verification".into()));
    }

    Ok(())
}

/// Cancel a verification.
#[tauri::command]
pub async fn cancel_verification(
    state: State<'_, AppState>,
    user_id: String,
    flow_id: String,
) -> Result<(), AppError> {
    let lock = state.client.lock().await;
    let client = lock.as_ref().ok_or(AppError::NotLoggedIn)?;

    let user_id = matrix_sdk::ruma::UserId::parse(&user_id)
        .map_err(|_| AppError::Other("Invalid user ID".into()))?;

    let verification = client
        .encryption()
        .get_verification(&user_id, &flow_id)
        .await;

    if let Some(v) = verification {
        if let Some(sas) = v.sas() {
            sas.cancel().await?;
        }
    } else {
        // Try cancelling the request instead
        let request = client
            .encryption()
            .get_verification_request(&user_id, &flow_id)
            .await;
        if let Some(req) = request {
            req.cancel().await?;
        }
    }

    Ok(())
}
