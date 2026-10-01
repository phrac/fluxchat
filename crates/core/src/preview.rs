use crate::error::AppError;
use crate::models::UrlPreview;

/// Previews kept in memory; when full, half (arbitrary entries) is dropped.
const PREVIEW_CACHE_LIMIT: usize = 500;

/// Fetch an OpenGraph URL preview from the homeserver, with caching.
pub async fn get_url_preview(
    client: &matrix_sdk::Client,
    cache: &std::sync::Mutex<std::collections::HashMap<String, String>>,
    url: &str,
) -> Result<Option<UrlPreview>, AppError> {
    let cache_key = format!("preview:{}", url);
    { let c = cache.lock().unwrap_or_else(|e| e.into_inner()); if let Some(v) = c.get(&cache_key) { return Ok(serde_json::from_str(v).ok()); } }

    use matrix_sdk::ruma::api::client::authenticated_media::get_media_preview::v1::Request;
    let request = Request::new(url.to_string());
    let response = match client.send(request).await { Ok(r) => r, Err(_) => return Ok(None) };

    let og: serde_json::Value = match &response.data { Some(raw) => serde_json::from_str(raw.get()).unwrap_or_default(), None => return Ok(None) };

    let title = og.get("og:title").and_then(|v| v.as_str()).map(String::from);
    let description = og.get("og:description").and_then(|v| v.as_str()).map(String::from);
    let image_url = og.get("og:image").and_then(|v| v.as_str()).map(String::from);
    let site_name = og.get("og:site_name").and_then(|v| v.as_str()).map(String::from);

    if title.is_none() && description.is_none() { return Ok(None); }

    let preview = UrlPreview { url: url.to_string(), title, description, image_url, site_name };
    if let Ok(json) = serde_json::to_string(&preview) {
        let mut c = cache.lock().unwrap_or_else(|e| e.into_inner());
        if c.len() >= PREVIEW_CACHE_LIMIT {
            let drop: Vec<String> = c.keys().take(PREVIEW_CACHE_LIMIT / 2).cloned().collect();
            for k in drop { c.remove(&k); }
        }
        c.insert(cache_key, json);
    }
    Ok(Some(preview))
}
