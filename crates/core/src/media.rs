use matrix_sdk::media::{MediaFormat, MediaRequestParameters};

use crate::error::AppError;
use crate::rooms::parse_room_id;

/// Generate a thumbnail from image data. Returns (png_bytes, width, height) or None.
pub fn generate_thumbnail(data: &[u8], max_w: u32, max_h: u32) -> Option<(Vec<u8>, u32, u32)> {
    let img = image::load_from_memory(data).ok()?;
    let thumb = img.thumbnail(max_w, max_h);
    let (w, h) = (thumb.width(), thumb.height());
    let mut buf = std::io::Cursor::new(Vec::new());
    thumb.write_to(&mut buf, image::ImageFormat::Png).ok()?;
    Some((buf.into_inner(), w, h))
}

/// Get original image dimensions without full decode.
/// A PDF's first page as a PNG `width` px wide (pure-Rust rendering, no
/// system libraries), with its size. None for anything it can't render —
/// encrypted PDFs, broken files (a panic inside the renderer included).
pub fn pdf_preview(data: &[u8], width: u32) -> Option<(Vec<u8>, u32, u32)> {
    use hayro::hayro_interpret::InterpreterSettings;
    use hayro::hayro_syntax::Pdf;
    use hayro::vello_cpu::color::palette::css::WHITE;
    use hayro::{PixmapSettings, RenderCache, RenderSettings};
    let data = data.to_vec();
    std::panic::catch_unwind(move || {
        let pdf = Pdf::new(data).ok()?;
        let pages = pdf.pages();
        let page = pages.first()?;
        let (w, h) = page.render_dimensions();
        if w <= 0.0 || h <= 0.0 {
            return None;
        }
        // Scale to the width; very tall pages are capped at 3x it.
        let scale = (width as f32 / w).min(width as f32 * 3.0 / h).clamp(0.05, 4.0);
        let cache = RenderCache::new();
        let pixmap = hayro::render(page, &cache, &InterpreterSettings::default(), &RenderSettings::default(),
            &PixmapSettings { x_scale: scale, y_scale: scale, bg_color: WHITE });
        let (pw, ph) = (pixmap.width() as u32, pixmap.height() as u32);
        let png = pixmap.into_png().ok()?;
        Some((png, pw, ph))
    }).ok().flatten()
}

pub fn get_image_dimensions(data: &[u8]) -> Option<(u32, u32)> {
    let img = image::load_from_memory(data).ok()?;
    Some((img.width(), img.height()))
}

/// Fetch raw media bytes from a room event (handles encrypted media).
/// Returns (bytes, mime_type). Used for video where data URIs are too large.
pub async fn get_event_media_bytes(
    client: &matrix_sdk::Client,
    room_id: &str,
    event_id: &str,
) -> Result<(Vec<u8>, String), AppError> {
    let room_id = parse_room_id(room_id)?;
    let room = client.get_room(&room_id).ok_or(AppError::RoomNotFound(room_id.to_string()))?;
    let event_id = matrix_sdk::ruma::EventId::parse(event_id).map_err(|_| AppError::Other("Invalid event ID".into()))?;
    let event = room.event(&event_id, None).await.map_err(|e| AppError::Other(format!("Failed to fetch event: {e}")))?;

    use matrix_sdk::ruma::events::{AnySyncTimelineEvent, AnySyncMessageLikeEvent, SyncMessageLikeEvent};
    use matrix_sdk::ruma::events::room::message::MessageType;

    let (source, mime) = match event.raw().deserialize().map_err(|e| AppError::Other(e.to_string()))? {
        AnySyncTimelineEvent::MessageLike(AnySyncMessageLikeEvent::RoomMessage(SyncMessageLikeEvent::Original(msg))) => {
            match msg.content.msgtype {
                MessageType::Image(img) => {
                    let mime = img.info.as_ref().and_then(|i| i.mimetype.clone()).unwrap_or_else(|| "image/png".into());
                    (img.source, mime)
                }
                MessageType::Video(vid) => {
                    let mime = vid.info.as_ref().and_then(|i| i.mimetype.clone()).unwrap_or_else(|| "video/mp4".into());
                    (vid.source, mime)
                }
                MessageType::Audio(audio) => {
                    let mime = audio.info.as_ref().and_then(|i| i.mimetype.clone()).unwrap_or_else(|| "audio/ogg".into());
                    (audio.source, mime)
                }
                MessageType::File(file) => {
                    let mime = file.info.as_ref().and_then(|i| i.mimetype.clone()).unwrap_or_else(|| "application/octet-stream".into());
                    (file.source, mime)
                }
                _ => return Err(AppError::Other("Not a media message".into())),
            }
        }
        AnySyncTimelineEvent::MessageLike(AnySyncMessageLikeEvent::Sticker(SyncMessageLikeEvent::Original(sticker))) => {
            let mime = sticker.content.info.mimetype.clone().unwrap_or_else(|| "image/png".into());
            match sticker.content.source {
                matrix_sdk::ruma::events::sticker::StickerMediaSource::Plain(uri) => {
                    (matrix_sdk::ruma::events::room::MediaSource::Plain(uri), mime)
                }
                matrix_sdk::ruma::events::sticker::StickerMediaSource::Encrypted(enc) => {
                    (matrix_sdk::ruma::events::room::MediaSource::Encrypted(enc), mime)
                }
                _ => return Err(AppError::Other("Unknown sticker source".into())),
            }
        }
        _ => return Err(AppError::Other("Not a message event".into())),
    };

    let request = MediaRequestParameters { source, format: MediaFormat::File };
    // Use false to skip SDK cache — ensures fresh decrypted download for encrypted media
    let data = client.media().get_media_content(&request, false).await
        .map_err(|e| AppError::Other(format!("Failed to download: {e}")))?;
    Ok((data, mime))
}

#[cfg(test)]
mod pdf_tests {
    /// A one-page PDF saying "Hello" in Helvetica (a standard font: not
    /// embedded, so it only draws with hayro's bundled fonts).
    fn hello_pdf() -> Vec<u8> {
        let objects = [
            "<< /Type /Catalog /Pages 2 0 R >>",
            "<< /Type /Pages /Kids [3 0 R] /Count 1 >>",
            "<< /Type /Page /Parent 2 0 R /MediaBox [0 0 200 100] /Contents 4 0 R /Resources << /Font << /F1 5 0 R >> >> >>",
            "<< /Length 41 >>\nstream\nBT /F1 36 Tf 20 35 Td (Hello) Tj ET\nendstream",
            "<< /Type /Font /Subtype /Type1 /BaseFont /Helvetica >>",
        ];
        let mut pdf = b"%PDF-1.4\n".to_vec();
        let mut offsets = Vec::new();
        for (i, body) in objects.iter().enumerate() {
            offsets.push(pdf.len());
            pdf.extend(format!("{} 0 obj\n{body}\nendobj\n", i + 1).as_bytes());
        }
        let xref = pdf.len();
        pdf.extend(format!("xref\n0 {}\n0000000000 65535 f \n", objects.len() + 1).as_bytes());
        for o in offsets {
            pdf.extend(format!("{o:010} 00000 n \n").as_bytes());
        }
        pdf.extend(format!("trailer\n<< /Size {} /Root 1 0 R >>\nstartxref\n{xref}\n%%EOF\n", objects.len() + 1).as_bytes());
        pdf
    }

    #[test]
    fn renders_the_first_page_with_its_text() {
        let (png, w, h) = super::pdf_preview(&hello_pdf(), 400).expect("renders");
        assert_eq!((w, h), (400, 200));
        let img = image::load_from_memory(&png).unwrap().to_rgba8();
        let dark = img.pixels().filter(|p| p.0[0] < 128).count();
        assert!(dark > 200, "the text drew ({dark} dark pixels)");
        assert!(super::pdf_preview(b"not a pdf", 400).is_none());
    }
}
