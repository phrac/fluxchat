use matrix_sdk::{
    authentication::{matrix::MatrixSession, SessionTokens},
    ruma::OwnedUserId,
    Client, SessionMeta,
};
use serde::Serialize;
use serde_json::json;
use std::path::PathBuf;
use tauri::State;
use tracing::{info, warn};

use crate::db;
use crate::events::spawn_sync;
use crate::state::AppState;

use super::AppError;

fn data_dir() -> PathBuf {
    dirs::data_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join("fluxchat")
}

#[derive(Debug, Serialize)]
pub struct LoginResult {
    pub user_id: String,
    pub display_name: Option<String>,
}

#[tauri::command]
pub async fn login(
    state: State<'_, AppState>,
    app_handle: tauri::AppHandle,
    homeserver: String,
    username: String,
    password: String,
) -> Result<LoginResult, AppError> {
    let data_dir = data_dir();
    let matrix_store = data_dir.join("matrix_store");

    // Clear old crypto store on fresh login to avoid device ID mismatch
    if matrix_store.exists() {
        info!("Clearing old matrix store for fresh login");
        let _ = std::fs::remove_dir_all(&matrix_store);
    }

    let client = Client::builder()
        .server_name_or_homeserver_url(&homeserver)
        .sqlite_store(&matrix_store, None)
        .build()
        .await?;

    client
        .matrix_auth()
        .login_username(&username, &password)
        .initial_device_display_name("fluxchat")
        .await?;

    let user_id = client
        .user_id()
        .ok_or(AppError::Other("No user ID after login".into()))?
        .to_string();

    // Persist session to FluxDB
    if let Some(session) = client.matrix_auth().session() {
        let session_json = json!({
            "user_id": session.meta.user_id.as_str(),
            "device_id": session.meta.device_id.as_str(),
            "access_token": session.tokens.access_token,
            "refresh_token": session.tokens.refresh_token,
        });
        db::save_session(&state.db, &homeserver, session_json)
            .map_err(|e| AppError::Other(e.to_string()))?;
        info!("Session saved to FluxDB for {}", user_id);
    }

    let display_name = client.account().get_display_name().await.ok().flatten();

    let client_clone = client.clone();
    {
        let mut lock = state.client.lock().await;
        *lock = Some(client);
    }
    spawn_sync(client_clone, app_handle);

    Ok(LoginResult {
        user_id,
        display_name,
    })
}

#[tauri::command]
pub async fn logout(state: State<'_, AppState>) -> Result<(), AppError> {
    let mut lock = state.client.lock().await;
    if let Some(client) = lock.take() {
        let _ = client.logout().await;
    }
    db::delete_session(&state.db);

    // Also clear the matrix crypto store
    let matrix_store = data_dir().join("matrix_store");
    if matrix_store.exists() {
        let _ = std::fs::remove_dir_all(&matrix_store);
    }

    Ok(())
}

#[tauri::command]
pub async fn restore_session(
    state: State<'_, AppState>,
    app_handle: tauri::AppHandle,
) -> Result<Option<LoginResult>, AppError> {
    let session_doc = match db::load_session(&state.db) {
        Some(doc) => doc,
        None => {
            info!("No saved session found in FluxDB");
            return Ok(None);
        }
    };

    let homeserver = session_doc["homeserver"]
        .as_str()
        .ok_or(AppError::Other("Invalid session: missing homeserver".into()))?;
    let session = &session_doc["session"];

    let user_id: OwnedUserId = session["user_id"]
        .as_str()
        .ok_or(AppError::Other("Invalid session: missing user_id".into()))?
        .try_into()
        .map_err(|_| AppError::Other("Invalid user_id format".into()))?;

    let device_id: matrix_sdk::ruma::OwnedDeviceId = session["device_id"]
        .as_str()
        .ok_or(AppError::Other("Invalid session: missing device_id".into()))?
        .into();

    let access_token = session["access_token"]
        .as_str()
        .ok_or(AppError::Other("Invalid session: missing access_token".into()))?
        .to_string();

    let refresh_token = session["refresh_token"].as_str().map(|s| s.to_string());

    let data_dir = data_dir();
    let matrix_store = data_dir.join("matrix_store");

    info!("Restoring session for {} (device {})", user_id, device_id);

    let client = Client::builder()
        .server_name_or_homeserver_url(homeserver)
        .sqlite_store(&matrix_store, None)
        .build()
        .await?;

    let matrix_session = MatrixSession {
        meta: SessionMeta {
            user_id: user_id.clone(),
            device_id,
        },
        tokens: SessionTokens {
            access_token,
            refresh_token,
        },
    };

    match client.restore_session(matrix_session).await {
        Ok(()) => {
            info!("Session restored successfully");
        }
        Err(e) => {
            warn!("Session restore failed: {e}, clearing stale data");
            db::delete_session(&state.db);
            if matrix_store.exists() {
                let _ = std::fs::remove_dir_all(&matrix_store);
            }
            return Ok(None);
        }
    }

    let display_name = client.account().get_display_name().await.ok().flatten();

    let client_clone = client.clone();
    {
        let mut lock = state.client.lock().await;
        *lock = Some(client);
    }
    spawn_sync(client_clone, app_handle);

    Ok(Some(LoginResult {
        user_id: user_id.to_string(),
        display_name,
    }))
}
