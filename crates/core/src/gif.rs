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

/// Download GIF bytes from a URL.
pub async fn download_gif(url: &str) -> Result<Vec<u8>, AppError> {
    let resp = reqwest::get(url)
        .await
        .map_err(|e| AppError::Other(format!("GIF download failed: {e}")))?;

    resp.bytes()
        .await
        .map(|b| b.to_vec())
        .map_err(|e| AppError::Other(format!("GIF read failed: {e}")))
}
