use crate::error::AppError;
use crate::models::UrlPreview;

pub async fn get_url_preview(
    client: &matrix_sdk::Client,
    cache: &std::sync::Mutex<std::collections::HashMap<String, String>>,
    url: &str,
) -> Result<Option<UrlPreview>, AppError> {
    let cache_key = format!("preview:{}", url);
    { let c = cache.lock().unwrap(); if let Some(v) = c.get(&cache_key) { return Ok(serde_json::from_str(v).ok()); } }

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
    if let Ok(json) = serde_json::to_string(&preview) { cache.lock().unwrap().insert(cache_key, json); }
    Ok(Some(preview))
}
