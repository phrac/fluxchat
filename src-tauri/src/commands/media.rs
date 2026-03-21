use matrix_sdk::{
    media::{MediaFormat, MediaRequestParameters},
    ruma::{events::room::MediaSource, MxcUri},
};
use tauri::State;

use crate::db;
use crate::state::AppState;

use super::AppError;

/// Download media via the Matrix client and return it as a base64 data URI.
/// Three-tier cache: in-memory HashMap → FluxDB on disk → network fetch.
#[tauri::command]
pub async fn get_media_url(
    state: State<'_, AppState>,
    mxc_uri: String,
) -> Result<String, AppError> {
    // L1: in-memory cache
    {
        let cache = state.media_cache.lock().unwrap();
        if let Some(data_uri) = cache.get(&mxc_uri) {
            return Ok(data_uri.clone());
        }
    }

    // L2: FluxDB disk cache
    if let Some(data_uri) = db::get_cached_media(&state.db, &mxc_uri) {
        // Promote to L1
        let mut cache = state.media_cache.lock().unwrap();
        cache.insert(mxc_uri, data_uri.clone());
        return Ok(data_uri);
    }

    // L3: network fetch
    let client = {
        let lock = state.client.lock().await;
        lock.as_ref().cloned().ok_or(AppError::NotLoggedIn)?
    };

    let mxc = <&MxcUri>::try_from(mxc_uri.as_str())
        .map_err(|_| AppError::Other("Invalid MXC URI".into()))?;

    let request = MediaRequestParameters {
        source: MediaSource::Plain(mxc.to_owned()),
        format: MediaFormat::File,
    };

    let data = client
        .media()
        .get_media_content(&request, true)
        .await
        .map_err(|e| AppError::Other(format!("Failed to download media: {e}")))?;

    // Detect content type from magic bytes
    let content_type = if data.starts_with(&[0x89, 0x50, 0x4E, 0x47]) {
        "image/png"
    } else if data.starts_with(&[0xFF, 0xD8, 0xFF]) {
        "image/jpeg"
    } else if data.starts_with(b"GIF") {
        "image/gif"
    } else if data.starts_with(b"RIFF") && data.len() > 12 && &data[8..12] == b"WEBP" {
        "image/webp"
    } else {
        "image/png"
    };

    use base64::Engine;
    let b64 = base64::engine::general_purpose::STANDARD.encode(&data);
    let data_uri = format!("data:{};base64,{}", content_type, b64);

    // Store in L1 (memory)
    {
        let mut cache = state.media_cache.lock().unwrap();
        cache.insert(mxc_uri.clone(), data_uri.clone());
    }

    // Store in L2 (FluxDB disk) — fire and forget
    db::cache_media(&state.db, &mxc_uri, &data_uri);

    Ok(data_uri)
}
