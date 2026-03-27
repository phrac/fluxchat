/// Local HTTP server for video playback in WKWebView.
/// Data URIs are too large for video; this serves cached files via HTTP with Range support.
use std::io::{Read, Write, Seek, BufRead, BufReader};
use std::net::TcpListener;
use std::sync::atomic::{AtomicU16, Ordering};

static MEDIA_PORT: AtomicU16 = AtomicU16::new(0);

/// Get the media server base URL.
pub fn base_url() -> String {
    format!("http://127.0.0.1:{}", MEDIA_PORT.load(Ordering::Relaxed))
}

/// Start the media server on a random port (127.0.0.1 only).
pub fn start() {
    let listener = TcpListener::bind("127.0.0.1:0").expect("Failed to bind media server");
    MEDIA_PORT.store(listener.local_addr().unwrap().port(), Ordering::Relaxed);

    std::thread::spawn(move || {
        let cache_dir = std::env::temp_dir().join("fluxchat_video");
        for stream in listener.incoming().flatten() {
            let cache_dir = cache_dir.clone();
            std::thread::spawn(move || { let _ = handle(stream, &cache_dir); });
        }
    });
}

fn handle(mut stream: std::net::TcpStream, cache_dir: &std::path::Path) -> std::io::Result<()> {
    let mut reader = BufReader::new(stream.try_clone()?);
    let mut request_line = String::new();
    reader.read_line(&mut request_line)?;

    let path = request_line.split_whitespace().nth(1).unwrap_or("/");
    let filename = path.trim_start_matches('/');

    // Collect headers
    let mut range_header = None;
    loop {
        let mut line = String::new();
        reader.read_line(&mut line)?;
        if line.trim().is_empty() { break; }
        if let Some(val) = line.strip_prefix("Range: ") {
            range_header = Some(val.trim().to_string());
        }
    }

    let file_path = cache_dir.join(filename);
    let mut file = match std::fs::File::open(&file_path) {
        Ok(f) => f,
        Err(_) => { stream.write_all(b"HTTP/1.1 404 Not Found\r\nContent-Length: 0\r\n\r\n")?; return Ok(()); }
    };

    let total = file.metadata()?.len() as usize;
    let mime = if filename.ends_with(".webm") { "video/webm" }
        else if filename.ends_with(".mov") { "video/quicktime" }
        else { "video/mp4" };

    // Parse Range header
    let (status, start, end) = if let Some(range) = range_header.as_deref().and_then(|s| s.strip_prefix("bytes=")) {
        let parts: Vec<&str> = range.split('-').collect();
        let s: usize = parts.first().and_then(|p| p.parse().ok()).unwrap_or(0);
        let e: usize = parts.get(1).and_then(|p| if p.is_empty() { None } else { p.parse().ok() }).unwrap_or(total - 1).min(total - 1);
        (206, s, e)
    } else {
        (200, 0, total - 1)
    };

    let len = end - start + 1;
    file.seek(std::io::SeekFrom::Start(start as u64))?;

    let mut header = format!("HTTP/1.1 {status} {}\r\nContent-Type: {mime}\r\nContent-Length: {len}\r\nAccept-Ranges: bytes\r\n",
        if status == 206 { "Partial Content" } else { "OK" });
    if status == 206 { header.push_str(&format!("Content-Range: bytes {start}-{end}/{total}\r\n")); }
    header.push_str("\r\n");
    stream.write_all(header.as_bytes())?;

    let mut remaining = len;
    let mut buf = [0u8; 65536];
    while remaining > 0 {
        let to_read = remaining.min(buf.len());
        let n = file.read(&mut buf[..to_read])?;
        if n == 0 { break; }
        stream.write_all(&buf[..n])?;
        remaining -= n;
    }
    Ok(())
}
