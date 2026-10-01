//! Animated image frames (GIF, animated WebP) with a memory budget.
//!
//! `iced_gif::Frames` decodes every frame up front to a full-canvas RGBA
//! buffer, so a few-KB GIF declaring a huge canvas or thousands of frames
//! would allocate gigabytes — anyone could post one and take down everyone
//! viewing the room. `decode` measures the decoded size from the headers
//! first (no pixel data is decoded) and refuses anything over
//! `ANIMATION_MAX_BYTES`; those show as a still image instead.

use std::collections::{HashMap, VecDeque};
use std::sync::Arc;

/// Largest decoded size of one animation (a 480×480, 140-frame GIF).
pub(crate) const ANIMATION_MAX_BYTES: u64 = 128 * 1024 * 1024;
/// Decoded frames kept for timeline / gallery / lightbox animations.
pub(crate) const FRAMES_CACHE_BYTES: u64 = 512 * 1024 * 1024;
/// Decoded frames kept for the GIF search grid (small previews).
pub(crate) const PREVIEW_CACHE_BYTES: u64 = 128 * 1024 * 1024;

/// Decoded animation frames and their size in memory.
#[derive(Debug, Clone)]
pub(crate) struct Anim {
    pub frames: Arc<iced_gif::Frames>,
    pub bytes: u64,
}

/// Decoded size of a GIF the way `iced_gif` holds it (one full-canvas RGBA
/// buffer per frame), read from the headers without decoding pixels. Stops
/// counting frames once over `limit`.
fn gif_decoded_bytes(data: &[u8], limit: u64) -> Option<u64> {
    let mut opts = gif::DecodeOptions::new();
    opts.set_color_output(gif::ColorOutput::Indexed);
    let mut decoder = opts.read_info(data).ok()?;
    let canvas = decoder.width() as u64 * decoder.height() as u64 * 4;
    let mut total = 0u64;
    while decoder.next_frame_info().ok()?.is_some() {
        total += canvas;
        if total > limit {
            break;
        }
    }
    Some(total)
}

/// Decoded size of an animated WebP's frames (RGBA, full canvas each).
pub(crate) fn webp_decoded_bytes(width: u32, height: u32, frames: u32) -> u64 {
    width as u64 * height as u64 * 4 * frames as u64
}

/// Decode a GIF's frames, unless that would exceed `ANIMATION_MAX_BYTES`.
/// Blocking: run under `spawn_blocking`.
pub(crate) fn decode(gif_bytes: Vec<u8>) -> Option<Anim> {
    let bytes = gif_decoded_bytes(&gif_bytes, ANIMATION_MAX_BYTES)?;
    if bytes == 0 || bytes > ANIMATION_MAX_BYTES {
        tracing::info!("animation not decoded: {bytes} bytes decoded exceeds the limit");
        return None;
    }
    let frames = iced_gif::Frames::from_bytes(gif_bytes)
        .map_err(|e| tracing::warn!("iced_gif Frames::from_bytes failed: {e}"))
        .ok()?;
    Some(Anim { frames: Arc::new(frames), bytes })
}

/// Decoded animations, evicted oldest-first to stay within a byte budget.
pub(crate) struct FramesCache {
    map: HashMap<String, Anim>,
    order: VecDeque<String>,
    bytes: u64,
    budget: u64,
}

impl FramesCache {
    pub(crate) fn new(budget: u64) -> Self {
        Self { map: HashMap::new(), order: VecDeque::new(), bytes: 0, budget }
    }

    pub(crate) fn get(&self, key: &str) -> Option<&Arc<iced_gif::Frames>> {
        self.map.get(key).map(|a| &a.frames)
    }

    pub(crate) fn contains_key(&self, key: &str) -> bool {
        self.map.contains_key(key)
    }

    /// Whether adding a large animation could evict others.
    pub(crate) fn nearly_full(&self) -> bool {
        self.bytes + ANIMATION_MAX_BYTES > self.budget
    }

    pub(crate) fn insert(&mut self, key: String, anim: Anim) {
        if let Some(old) = self.map.remove(&key) {
            self.bytes -= old.bytes;
            self.order.retain(|k| k != &key);
        }
        while self.bytes + anim.bytes > self.budget {
            let Some(oldest) = self.order.pop_front() else { break };
            if let Some(old) = self.map.remove(&oldest) {
                self.bytes -= old.bytes;
            }
        }
        self.bytes += anim.bytes;
        self.order.push_back(key.clone());
        self.map.insert(key, anim);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn gif(width: u16, height: u16, frames: usize) -> Vec<u8> {
        let mut out = Vec::new();
        {
            let mut enc = gif::Encoder::new(&mut out, width, height, &[0, 0, 0, 255, 255, 255]).unwrap();
            for _ in 0..frames {
                // A 1×1 frame: tiny on the wire whatever the canvas size.
                let frame = gif::Frame { width: 1, height: 1, buffer: std::borrow::Cow::Borrowed(&[0]), ..Default::default() };
                enc.write_frame(&frame).unwrap();
            }
        }
        out
    }

    #[test]
    fn measures_decoded_size_from_headers() {
        assert_eq!(gif_decoded_bytes(&gif(10, 20, 3), u64::MAX), Some(10 * 20 * 4 * 3));
        assert_eq!(gif_decoded_bytes(b"not a gif", u64::MAX), None);
    }

    #[test]
    fn refuses_decompression_bombs() {
        // ~1 KB on the wire, 4 GB decoded.
        let bomb = gif(16_000, 16_000, 4);
        assert!(bomb.len() < 4096);
        assert!(decode(bomb).is_none());
        assert!(decode(gif(8, 8, 2)).is_some());
    }

    #[test]
    fn cache_evicts_oldest_to_stay_in_budget() {
        let anim = |bytes| Anim { frames: Arc::new(iced_gif::Frames::from_bytes(gif(2, 2, 2)).unwrap()), bytes };
        let mut cache = FramesCache::new(100);
        cache.insert("a".into(), anim(40));
        cache.insert("b".into(), anim(40));
        cache.insert("c".into(), anim(40));
        assert!(!cache.contains_key("a"));
        assert!(cache.contains_key("b") && cache.contains_key("c"));
        cache.insert("c".into(), anim(50));
        assert_eq!(cache.bytes, 90);
        assert!(cache.contains_key("b"));
    }
}
