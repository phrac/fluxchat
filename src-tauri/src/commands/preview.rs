use serde::{Deserialize, Serialize};
use tauri::State;

use crate::state::AppState;
use super::AppError;

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct UrlPreview {
    pub url: String,
    pub title: Option<String>,
    pub description: Option<String>,
    pub image_url: Option<String>,
    pub site_name: Option<String>,
}

/// Fetch OpenGraph metadata for a URL via the Matrix server's preview API.
#[tauri::command]
pub async fn get_url_preview(
    state: State<'_, AppState>,
    url: String,
) -> Result<Option<UrlPreview>, AppError> {
    // Check cache
    {
        let cache = state.media_cache.lock().unwrap();
        if let Some(cached) = cache.get(&format!("preview:{}", url)) {
            if let Ok(preview) = serde_json::from_str::<UrlPreview>(cached) {
                return Ok(Some(preview));
            }
        }
    }

    let client = {
        let lock = state.client.lock().await;
        lock.as_ref().cloned().ok_or(AppError::NotLoggedIn)?
    };

    use matrix_sdk::ruma::api::client::authenticated_media::get_media_preview::v1::Request;

    let request = Request::new(url.clone());
    let response = match client.send(request).await {
        Ok(r) => r,
        Err(_) => return Ok(None),
    };

    // The response data is an Option<Box<RawValue>> — parse it as JSON
    let og_data: serde_json::Value = match &response.data {
        Some(raw) => serde_json::from_str(raw.get()).unwrap_or_default(),
        None => return Ok(None),
    };

    let title = og_data.get("og:title").and_then(|v| v.as_str()).map(String::from);
    let description = og_data.get("og:description").and_then(|v| v.as_str()).map(String::from);
    let image_url = og_data.get("og:image").and_then(|v| v.as_str()).map(String::from);
    let site_name = og_data.get("og:site_name").and_then(|v| v.as_str()).map(String::from);

    if title.is_none() && description.is_none() {
        return Ok(None);
    }

    let preview = UrlPreview {
        url,
        title,
        description,
        image_url,
        site_name,
    };

    // Cache it
    if let Ok(json) = serde_json::to_string(&preview) {
        let mut cache = state.media_cache.lock().unwrap();
        cache.insert(format!("preview:{}", preview.url), json);
    }

    Ok(Some(preview))
}
