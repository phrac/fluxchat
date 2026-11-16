//! Giphy GIF search API.
//!
//! Requires a Giphy API key via `/giphykey`, `GIPHY_API_KEY` env var,
//! or the settings database. Get a free key at <https://developers.giphy.com/dashboard/>.

use crate::AppError;
use serde::Deserialize;

/// Maximum number of GIF results to fetch per search.
const GIF_SEARCH_LIMIT: u8 = 12;

/// A single GIF search result.
#[derive(Clone, Debug)]
pub struct GifResult {
    /// Preview thumbnail URL (small, for the picker grid).
    pub preview_url: String,
    /// Full-size GIF URL (sent to the room).
    pub url: String,
    /// Width of the full-size GIF.
    pub width: u32,
    /// Height of the full-size GIF.
    pub height: u32,
    /// Short description.
    pub description: String,
}

#[derive(Deserialize)]
struct GiphyResponse {
    data: Vec<GiphyGif>,
}

#[derive(Deserialize)]
struct GiphyGif {
    #[serde(default)]
    title: String,
    images: GiphyImages,
}

#[derive(Deserialize)]
struct GiphyImages {
    original: Option<GiphyImage>,
    fixed_height_small: Option<GiphyImage>,
    fixed_height: Option<GiphyImage>,
}

#[derive(Deserialize)]
struct GiphyImage {
    #[serde(default)]
    url: String,
    #[serde(default, deserialize_with = "deserialize_str_u32")]
    width: u32,
    #[serde(default, deserialize_with = "deserialize_str_u32")]
    height: u32,
}

/// Giphy returns dimensions as strings; parse them to u32.
fn deserialize_str_u32<'de, D: serde::Deserializer<'de>>(d: D) -> Result<u32, D::Error> {
    let s = String::deserialize(d)?;
    s.parse().map_err(|_| serde::de::Error::custom("invalid number"))
}

/// Search Giphy for GIFs matching the query.
/// If `api_key` is empty/None, falls back to `GIPHY_API_KEY` env var.
pub async fn search_gifs(query: &str, api_key: Option<&str>) -> Result<Vec<GifResult>, AppError> {
    let api_key = api_key
        .filter(|k| !k.is_empty())
        .map(String::from)
        .or_else(|| std::env::var("GIPHY_API_KEY").ok())
        .ok_or_else(|| AppError::Other(
            "No Giphy API key set. Use /giphykey <key> to set one (free at developers.giphy.com/dashboard)".into()
        ))?;

    let client = reqwest::Client::new();
    let resp = client
        .get("https://api.giphy.com/v1/gifs/search")
        .query(&[
            ("q", query),
            ("api_key", &*api_key),
            ("limit", &GIF_SEARCH_LIMIT.to_string()),
            ("rating", "pg-13"),
        ])
        .timeout(SEARCH_TIMEOUT)
        .send()
        .await
        .map_err(|e| AppError::Other(format!("Giphy request failed: {e}")))?;

    if !resp.status().is_success() {
        let status = resp.status();
        let body: String = resp.text().await.unwrap_or_default();
        return Err(AppError::Other(format!("Giphy API error {status}: {body}")));
    }

    let data: GiphyResponse = resp
        .json()
        .await
        .map_err(|e| AppError::Other(format!("Giphy parse failed: {e}")))?;

    let results = data
        .data
        .into_iter()
        .filter_map(|g| {
            let original = g.images.original?;
            let preview = g.images.fixed_height_small
                .or(g.images.fixed_height)
                .map(|i| i.url)
                .unwrap_or_else(|| original.url.clone());
            Some(GifResult {
                preview_url: preview,
                url: original.url,
                width: original.width,
                height: original.height,
                description: g.title,
            })
        })
        .collect();

    Ok(results)
}

/// Maximum GIF download size (50 MB).
const MAX_GIF_SIZE: u64 = 50 * 1024 * 1024;

/// How long a search, and a download, may take. reqwest has no timeout of
/// its own: on a connection that went quiet the picker stayed at
/// "searching" and a picked GIF was never sent, with no error either way.
const SEARCH_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(15);
const DOWNLOAD_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(60);

/// Download GIF bytes from a URL.
pub async fn download_gif(url: &str) -> Result<Vec<u8>, AppError> {
    download_up_to(url, MAX_GIF_SIZE, DOWNLOAD_TIMEOUT).await
}

async fn download_up_to(url: &str, max: u64, timeout: std::time::Duration) -> Result<Vec<u8>, AppError> {
    let mut resp = reqwest::Client::new()
        .get(url)
        .timeout(timeout)
        .send()
        .await
        .map_err(|e| AppError::Other(format!("GIF download failed: {e}")))?;

    // An error page isn't a GIF (it was uploaded to the room as one).
    if !resp.status().is_success() {
        return Err(AppError::Other(format!("GIF download failed: HTTP {}", resp.status())));
    }
    let too_large = |len: u64| AppError::Other(format!("GIF too large: {len} bytes (max {max})"));
    if let Some(len) = resp.content_length() {
        if len > max {
            return Err(too_large(len));
        }
    }

    // The length header is optional, and only a claim: count what arrives.
    let mut data = Vec::new();
    while let Some(chunk) = resp.chunk().await.map_err(|e| AppError::Other(format!("GIF read failed: {e}")))? {
        let len = (data.len() + chunk.len()) as u64;
        if len > max {
            return Err(too_large(len));
        }
        data.extend_from_slice(&chunk);
    }
    Ok(data)
}

#[cfg(test)]
mod tests {
    use super::download_up_to;
    use crate::test_http::{json, serve, stalled};
    use std::time::Duration;

    /// A response whose body is `len` bytes with no length header (it ends
    /// when the connection closes).
    fn unsized_body(len: usize) -> Vec<u8> {
        let mut response = b"HTTP/1.1 200 OK\r\nContent-Type: image/gif\r\nConnection: close\r\n\r\n".to_vec();
        response.resize(response.len() + len, b'x');
        response
    }

    #[tokio::test]
    async fn a_download_without_a_length_is_still_capped() {
        let base = serve(|_| unsized_body(5_000)).await;
        let data = download_up_to(&base, 10_000, Duration::from_secs(10)).await.expect("fits");
        assert_eq!(data.len(), 5_000);
        let err = download_up_to(&base, 1_000, Duration::from_secs(10)).await.unwrap_err();
        assert!(err.to_string().contains("too large"), "{err}");
    }

    #[tokio::test]
    async fn an_error_page_is_not_a_gif() {
        let base = serve(|_| json("404 Not Found", r#"{"message":"gone"}"#)).await;
        assert!(download_up_to(&base, 10_000, Duration::from_secs(10)).await.is_err());
    }

    #[tokio::test]
    async fn a_server_that_never_answers_is_given_up_on() {
        let base = stalled().await;
        let started = std::time::Instant::now();
        assert!(download_up_to(&base, 10_000, Duration::from_millis(300)).await.is_err());
        assert!(started.elapsed() < Duration::from_secs(5));
    }
}
