use serde::{Deserialize, Serialize};
use tauri::State;

use crate::state::AppState;
use super::AppError;

async fn get_client(state: &State<'_, AppState>) -> Result<matrix_sdk::Client, AppError> {
    let lock = state.client.lock().await;
    lock.as_ref().cloned().ok_or(AppError::NotLoggedIn)
}

#[derive(Debug, Serialize)]
pub struct UserProfile {
    pub user_id: String,
    pub display_name: Option<String>,
    pub avatar_url: Option<String>,
    pub banner_url: Option<String>,
}

#[tauri::command]
pub async fn get_profile(state: State<'_, AppState>) -> Result<UserProfile, AppError> {
    let client = get_client(&state).await?;

    let user_id = client
        .user_id()
        .ok_or(AppError::NotLoggedIn)?
        .to_string();

    let display_name = client.account().get_display_name().await.ok().flatten();
    let avatar_url = client
        .account()
        .get_avatar_url()
        .await
        .ok()
        .flatten()
        .map(|u| u.to_string());

    // Get banner from account data
    let banner_url = client
        .account()
        .account_data_raw(matrix_sdk::ruma::events::GlobalAccountDataEventType::from("io.element.banner"))
        .await
        .ok()
        .flatten()
        .and_then(|raw| {
            serde_json::from_str::<serde_json::Value>(raw.json().get()).ok()
        })
        .and_then(|v| v.get("url").and_then(|u| u.as_str()).map(String::from));

    Ok(UserProfile {
        user_id,
        display_name,
        avatar_url,
        banner_url,
    })
}

#[tauri::command]
pub async fn set_display_name(
    state: State<'_, AppState>,
    name: String,
) -> Result<(), AppError> {
    let client = get_client(&state).await?;
    client
        .account()
        .set_display_name(Some(&name))
        .await
        .map_err(|e| AppError::Other(format!("Failed to set display name: {e}")))?;
    Ok(())
}

#[tauri::command]
pub async fn upload_avatar(
    state: State<'_, AppState>,
    mime_type: String,
    data: Vec<u8>,
) -> Result<String, AppError> {
    let client = get_client(&state).await?;

    let content_type: mime::Mime = mime_type
        .parse()
        .unwrap_or("image/png".parse().unwrap());

    let mxc_url = client
        .account()
        .upload_avatar(&content_type, data)
        .await
        .map_err(|e| AppError::Other(format!("Failed to upload avatar: {e}")))?;

    Ok(mxc_url.to_string())
}

#[tauri::command]
pub async fn remove_avatar(state: State<'_, AppState>) -> Result<(), AppError> {
    let client = get_client(&state).await?;
    client
        .account()
        .set_avatar_url(None)
        .await
        .map_err(|e| AppError::Other(format!("Failed to remove avatar: {e}")))?;
    Ok(())
}

#[tauri::command]
pub async fn upload_banner(
    state: State<'_, AppState>,
    mime_type: String,
    data: Vec<u8>,
) -> Result<String, AppError> {
    let client = get_client(&state).await?;

    let content_type: mime::Mime = mime_type
        .parse()
        .unwrap_or("image/png".parse().unwrap());

    // Upload the image
    let response = client
        .media()
        .upload(&content_type, data, None)
        .await
        .map_err(|e| AppError::Other(format!("Failed to upload banner: {e}")))?;

    let mxc_url = response.content_uri.to_string();

    // Store the MXC URL in account data
    let banner_data = serde_json::json!({ "url": mxc_url });
    let raw = matrix_sdk::ruma::serde::Raw::from_json(
        serde_json::value::to_raw_value(&banner_data)
            .map_err(|e| AppError::Other(format!("Failed to serialize: {e}")))?,
    );

    client
        .account()
        .set_account_data_raw(matrix_sdk::ruma::events::GlobalAccountDataEventType::from("io.element.banner"), raw)
        .await
        .map_err(|e| AppError::Other(format!("Failed to save banner: {e}")))?;

    Ok(mxc_url)
}

#[tauri::command]
pub async fn remove_banner(state: State<'_, AppState>) -> Result<(), AppError> {
    let client = get_client(&state).await?;

    let banner_data = serde_json::json!({});
    let raw = matrix_sdk::ruma::serde::Raw::from_json(
        serde_json::value::to_raw_value(&banner_data)
            .map_err(|e| AppError::Other(format!("Failed to serialize: {e}")))?,
    );

    client
        .account()
        .set_account_data_raw(matrix_sdk::ruma::events::GlobalAccountDataEventType::from("io.element.banner"), raw)
        .await
        .map_err(|e| AppError::Other(format!("Failed to remove banner: {e}")))?;

    Ok(())
}
