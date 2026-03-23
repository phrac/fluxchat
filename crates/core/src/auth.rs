use matrix_sdk::{
    authentication::{matrix::MatrixSession, SessionTokens},
    ruma::OwnedUserId,
    Client, SessionMeta,
};
use serde_json::json;
use std::path::PathBuf;
use tracing::{info, warn};

use crate::error::AppError;
use crate::models::LoginResult;
use crate::state::AppState;

fn data_dir() -> PathBuf {
    dirs::data_dir().unwrap_or_else(|| PathBuf::from(".")).join("fluxchat")
}

pub async fn login(
    state: &AppState,
    homeserver: &str,
    username: &str,
    password: &str,
) -> Result<(Client, LoginResult), AppError> {
    let data_dir = data_dir();
    let matrix_store = data_dir.join("matrix_store");

    if matrix_store.exists() {
        info!("Clearing old matrix store for fresh login");
        let _ = std::fs::remove_dir_all(&matrix_store);
    }

    let client = Client::builder()
        .server_name_or_homeserver_url(homeserver)
        .sqlite_store(&matrix_store, None)
        .build()
        .await?;

    client.matrix_auth()
        .login_username(username, password)
        .initial_device_display_name("FluxChat")
        .await?;

    let user_id = client.user_id()
        .ok_or(AppError::Other("No user ID after login".into()))?.to_string();

    if let Some(session) = client.matrix_auth().session() {
        let session_json = json!({
            "homeserver": homeserver,
            "user_id": session.meta.user_id.as_str(),
            "device_id": session.meta.device_id.as_str(),
            "access_token": session.tokens.access_token,
            "refresh_token": session.tokens.refresh_token,
        });
        state.db.save_session(&session_json.to_string())
            .map_err(|e| AppError::Other(e.to_string()))?;
        info!("Session saved for {}", user_id);
    }

    // Enable event cache for Timeline API
    if let Err(e) = client.event_cache().subscribe() {
        warn!("Failed to subscribe event cache: {e}");
    }

    let display_name = client.account().get_display_name().await.ok().flatten();

    state.set_client(client.clone()).await;

    Ok((client, LoginResult { user_id, display_name }))
}

pub async fn logout(state: &AppState) -> Result<(), AppError> {
    let mut lock = state.client.lock().await;
    if let Some(client) = lock.take() {
        let _ = client.logout().await;
    }
    state.db.delete_session();
    let matrix_store = data_dir().join("matrix_store");
    if matrix_store.exists() {
        let _ = std::fs::remove_dir_all(&matrix_store);
    }
    Ok(())
}

/// Discover login types supported by a homeserver.
pub async fn get_login_types(homeserver: &str) -> Result<Vec<String>, AppError> {
    let client = Client::builder()
        .server_name_or_homeserver_url(homeserver)
        .build()
        .await?;

    let response = client.matrix_auth().get_login_types().await
        .map_err(|e| AppError::Other(format!("Failed to get login types: {e}")))?;

    use matrix_sdk::ruma::api::client::session::get_login_types::v3::LoginType;
    let mut types = Vec::new();
    for flow in &response.flows {
        match flow {
            LoginType::Password(_) => types.push("password".to_string()),
            LoginType::Sso(sso) => {
                types.push("sso".to_string());
                for provider in &sso.identity_providers {
                    types.push(format!("sso:{}", provider.name));
                }
            }
            LoginType::Token(_) => types.push("token".to_string()),
            _ => {}
        }
    }
    Ok(types)
}

/// SSO login — opens browser, waits for callback, completes login.
pub async fn login_sso(
    state: &AppState,
    homeserver: &str,
) -> Result<(Client, LoginResult), AppError> {
    let data_dir = data_dir();
    let matrix_store = data_dir.join("matrix_store");

    if matrix_store.exists() {
        info!("Clearing old matrix store for fresh login");
        let _ = std::fs::remove_dir_all(&matrix_store);
    }

    let client = Client::builder()
        .server_name_or_homeserver_url(homeserver)
        .sqlite_store(&matrix_store, None)
        .build()
        .await?;

    client.matrix_auth()
        .login_sso(|url| async move {
            // Open the SSO URL in the system browser
            if let Err(e) = open::that(&url) {
                warn!("Failed to open browser: {e}");
            }
            Ok(())
        })
        .initial_device_display_name("FluxChat")
        .await
        .map_err(|e| AppError::Other(format!("SSO login failed: {e}")))?;

    let user_id = client.user_id()
        .ok_or(AppError::Other("No user ID after SSO login".into()))?.to_string();

    if let Some(session) = client.matrix_auth().session() {
        let session_json = serde_json::json!({
            "homeserver": homeserver,
            "user_id": session.meta.user_id.as_str(),
            "device_id": session.meta.device_id.as_str(),
            "access_token": session.tokens.access_token,
            "refresh_token": session.tokens.refresh_token,
        });
        state.db.save_session(&session_json.to_string())
            .map_err(|e| AppError::Other(e.to_string()))?;
        info!("SSO session saved for {}", user_id);
    }

    // Enable event cache for Timeline API
    if let Err(e) = client.event_cache().subscribe() {
        warn!("Failed to subscribe event cache: {e}");
    }

    let display_name = client.account().get_display_name().await.ok().flatten();

    state.set_client(client.clone()).await;

    Ok((client, LoginResult { user_id, display_name }))
}

pub async fn restore_session(state: &AppState) -> Result<Option<(Client, LoginResult)>, AppError> {
    let session_str = match state.db.load_session() {
        Some(s) => s,
        None => { info!("No saved session"); return Ok(None); }
    };

    let session_doc: serde_json::Value = serde_json::from_str(&session_str)
        .map_err(|e| AppError::Other(format!("Invalid session data: {e}")))?;

    let homeserver = session_doc["homeserver"].as_str()
        .ok_or(AppError::Other("Missing homeserver".into()))?;
    let session = &session_doc;

    let user_id: OwnedUserId = session["user_id"].as_str()
        .ok_or(AppError::Other("Missing user_id".into()))?
        .try_into().map_err(|_| AppError::Other("Invalid user_id".into()))?;

    let device_id: matrix_sdk::ruma::OwnedDeviceId = session["device_id"].as_str()
        .ok_or(AppError::Other("Missing device_id".into()))?.into();

    let access_token = session["access_token"].as_str()
        .ok_or(AppError::Other("Missing access_token".into()))?.to_string();
    let refresh_token = session["refresh_token"].as_str().map(|s| s.to_string());

    let matrix_store = data_dir().join("matrix_store");

    info!("Restoring session for {} (device {})", user_id, device_id);

    let client = Client::builder()
        .server_name_or_homeserver_url(homeserver)
        .sqlite_store(&matrix_store, None)
        .build().await?;

    let matrix_session = MatrixSession {
        meta: SessionMeta { user_id: user_id.clone(), device_id },
        tokens: SessionTokens { access_token, refresh_token },
    };

    match client.restore_session(matrix_session).await {
        Ok(()) => info!("Session restored"),
        Err(e) => {
            warn!("Session restore failed: {e}, clearing stale data");
            state.db.delete_session();
            if matrix_store.exists() { let _ = std::fs::remove_dir_all(&matrix_store); }
            return Ok(None);
        }
    }

    // Enable event cache for Timeline API
    if let Err(e) = client.event_cache().subscribe() {
        warn!("Failed to subscribe event cache: {e}");
    }

    let display_name = client.account().get_display_name().await.ok().flatten();
    state.set_client(client.clone()).await;

    Ok(Some((client, LoginResult { user_id: user_id.to_string(), display_name })))
}
