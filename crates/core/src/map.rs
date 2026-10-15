//! OSM slippy-map tiles for location messages (MSC3488 `geo:` cards).
//!
//! Tiles are fetched from tile.openstreetmap.org. The OSM tile usage policy
//! requires a descriptive User-Agent (set below) and visible attribution for
//! rendered maps — the desktop card draws "© OpenStreetMap contributors"
//! under every preview. Composed previews are cached in the shared media
//! cache, so each distinct location hits the tile server at most once.

use image::ImageFormat;
use image::{GenericImage, GenericImageView};
use std::io::Cursor;

/// Zoom level for previews (building level; matches the OSM web deep link).
pub const MAP_ZOOM: u8 = 16;
/// Composed preview size — rendered at 2x and displayed at half size so it
/// stays crisp on hidpi screens (shown at 256x160).
pub const MAP_W: u32 = 512;
pub const MAP_H: u32 = 320;
/// Slippy tile edge in pixels.
const TILE: i64 = 256;

/// Per the tile usage policy: identify the app and where to find it.
const TILE_USER_AGENT: &str = "fluxchat-desktop (https://github.com/phrac/fluxchat)";

static TILE_CLIENT: std::sync::LazyLock<reqwest::Client> = std::sync::LazyLock::new(|| {
    reqwest::Client::builder()
        .user_agent(TILE_USER_AGENT)
        .timeout(std::time::Duration::from_secs(10))
        .build()
        .unwrap_or_default()
});

/// Parse a `geo:` URI (MSC3488) into (lat, lon), dropping any `;params`.
pub fn parse_geo_uri(geo: &str) -> Option<(f64, f64)> {
    let coords = geo.strip_prefix("geo:")?;
    let mut parts = coords.split(',');
    let lat: f64 = parts.next()?.trim().parse().ok()?;
    let lon: f64 = parts.next()?.split(';').next()?.trim().parse().ok()?;
    if (-90.0..=90.0).contains(&lat) && (-180.0..=180.0).contains(&lon) {
        Some((lat, lon))
    } else {
        None
    }
}

/// Media-cache key for a location's composed preview (rounded so identical
/// locations from different events dedupe). None for unparseable geo URIs.
pub fn map_cache_key(geo: &str) -> Option<String> {
    let (lat, lon) = parse_geo_uri(geo)?;
    Some(format!("map:{:.5},{:.5}", lat, lon))
}

/// Fractional slippy-tile coordinates (x east, y south) at zoom `z`.
pub fn tile_frac(lat: f64, lon: f64, z: u8) -> (f64, f64) {
    let n = (1u64 << z) as f64;
    let x = (lon + 180.0) / 360.0 * n;
    let rad = lat.to_radians();
    let y = (1.0 - (rad.tan() + 1.0 / rad.cos()).ln() / std::f64::consts::PI) / 2.0 * n;
    (x, y)
}

/// Fetch the tiles covering a MAP_W x MAP_H window centered on (lat, lon),
/// stitch them, crop the window and encode PNG. None if any tile fails —
/// the caller treats that like any failed media fetch (retry backoff).
pub async fn fetch_map_preview(lat: f64, lon: f64) -> Option<(Vec<u8>, u32, u32)> {
    let n = (1u64 << MAP_ZOOM) as i64;
    let (fx, fy) = tile_frac(lat, lon, MAP_ZOOM);
    // Center pixel in zoom-space, clamped so the window stays on the map.
    let cx = (fx * TILE as f64).round().clamp(MAP_W as i64 / 2, n * TILE - MAP_W as i64 / 2);
    let cy = (fy * TILE as f64).round().clamp(MAP_H as i64 / 2, n * TILE - MAP_H as i64 / 2);
    let ox = cx - MAP_W as i64 / 2;
    let oy = cy - MAP_H as i64 / 2;
    let (tx0, tx1) = (ox.div_euclid(TILE), (ox + MAP_W as i64 - 1).div_euclid(TILE));
    let (ty0, ty1) = (oy.div_euclid(TILE), (oy + MAP_H as i64 - 1).div_euclid(TILE));

    let client = TILE_CLIENT.clone();
    let mut tiles: Vec<(i64, i64, Vec<u8>)> = Vec::new();
    for ty in ty0..=ty1 {
        if ty < 0 || ty >= n {
            return None; // beyond the map edge (poles): no preview
        }
        for tx in tx0..=tx1 {
            let x = tx.rem_euclid(n) as u32; // wrap the antimeridian
            let bytes = fetch_tile(&client, MAP_ZOOM, x, ty as u32).await.ok()??;
            tiles.push((tx, ty, bytes));
        }
    }
    let stitched = tokio::task::spawn_blocking(move || compose(tiles, tx0, ty0, ox, oy))
        .await
        .ok()?;
    stitched.map(|png| (png, MAP_W, MAP_H))
}

async fn fetch_tile(client: &reqwest::Client, z: u8, x: u32, y: u32) -> Result<Option<Vec<u8>>, reqwest::Error> {
    let resp = client
        .get(format!("https://tile.openstreetmap.org/{z}/{x}/{y}.png"))
        .send()
        .await?;
    if !resp.status().is_success() {
        return Ok(None);
    }
    Ok(Some(resp.bytes().await?.to_vec()))
}

/// Paste the fetched tiles into a canvas aligned to (tx0, ty0) and crop the
/// window starting at window origin (ox, oy) in zoom-space pixels.
fn compose(tiles: Vec<(i64, i64, Vec<u8>)>, tx0: i64, ty0: i64, ox: i64, oy: i64) -> Option<Vec<u8>> {
    let cols = tiles.iter().map(|(tx, _, _)| *tx).max()? - tx0 + 1;
    let rows = tiles.iter().map(|(_, ty, _)| *ty).max()? - ty0 + 1;
    let mut canvas = image::RgbImage::new((cols * TILE) as u32, (rows * TILE) as u32);
    for (tx, ty, data) in &tiles {
        let tile = image::load_from_memory(data).ok()?;
        image::imageops::overlay(&mut canvas, &tile.to_rgb8(), ((tx - tx0) * TILE), ((ty - ty0) * TILE));
    }
    let crop = canvas.crop_imm((ox - tx0 * TILE) as u32, (oy - ty0 * TILE) as u32, MAP_W, MAP_H);
    let mut out = Vec::new();
    crop.write_to(&mut Cursor::new(&mut out), ImageFormat::Png).ok()?;
    Some(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_geo_uris_with_and_without_params() {
        assert_eq!(parse_geo_uri("geo:40.80,-96.75"), Some((40.80, -96.75)));
        assert_eq!(parse_geo_uri("geo:40.801,-96.752;u=25"), Some((40.801, -96.752)));
        assert_eq!(parse_geo_uri("geo: 40.8 , -96.7 "), Some((40.8, -96.7)));
        assert_eq!(parse_geo_uri("40.8,-96.7"), None);
        assert_eq!(parse_geo_uri("geo:91,0"), None);
        assert_eq!(parse_geo_uri("geo:0"), None);
    }

    #[test]
    fn cache_keys_round_and_dedupe() {
        assert_eq!(
            map_cache_key("geo:40.8,-96.75"),
            Some("map:40.80000,-96.75000".into())
        );
        assert_eq!(map_cache_key("geo:40.8,-96.75;u=5"), map_cache_key("geo:40.8,-96.75"));
        assert_eq!(map_cache_key("nonsense"), None);
    }

    #[test]
    fn tile_frac_matches_known_slippy_values() {
        // z=0: the whole world is one tile, so every point lands in [0,1).
        let (x, y) = tile_frac(0.0, 0.0, 0);
        assert!((x - 0.5).abs() < 1e-9 && (y - 0.5).abs() < 1e-9);
        // Lincoln, NE at z=16 (the app's zoom): well-known slippy formula check.
        let (x, y) = tile_frac(40.80, -96.75, 16);
        let n = 65536.0;
        assert!((x - (180.0 - 96.75) / 360.0 * n).abs() < 1e-6);
        assert!(y > 0.0 && y < n);
        // Equator maps to the exact vertical middle at any zoom.
        let (_, y) = tile_frac(0.0, 0.0, 16);
        assert!((y - 32768.0).abs() < 1e-6);
    }

    #[test]
    fn window_layout_stays_inside_the_canvas() {
        // For any center pixel the compose() crop origin must be non-negative
        // and the window must fit — check the clamp math at the edges.
        let n = (1i64 << MAP_ZOOM) * TILE;
        for cx in [MAP_W as i64 / 2, n / 2, n - MAP_W as i64 / 2] {
            let ox = cx - MAP_W as i64 / 2;
            assert!(ox >= 0 && ox + MAP_W as i64 <= n);
        }
    }

    #[test]
    fn compose_crops_centered_window() {
        // Tiles whose first column is red and the rest blue: the crop must
        // show the tile boundary exactly where it falls inside the window.
        // Window origin 128px into tile column 0 → boundary lands at x=128.
        let mut tiles = Vec::new();
        for tx in 0..=2 {
            for ty in 0..=1 {
                tiles.push((tx, ty, png_solid(if tx == 0 { 255 } else { 0 }, 0, if tx == 0 { 0 } else { 255 })));
            }
        }
        let png = compose(tiles, 0, 0, 128, 0).expect("compose");
        let img = image::load_from_memory(&png).unwrap().to_rgb8();
        assert_eq!((img.width(), img.height()), (MAP_W, MAP_H));
        assert_eq!(img.pixel(120, 160)[0], 255); // left of the boundary: red
        assert_eq!(img.pixel(136, 160)[2], 255); // right of it: blue
    }

    fn png_solid(r: u8, g: u8, b: u8) -> Vec<u8> {
        let img = image::RgbImage::from_pixel(256, 256, image::Rgb([r, g, b]));
        let mut out = Vec::new();
        img.write_to(&mut Cursor::new(&mut out), ImageFormat::Png).unwrap();
        out
    }
}
