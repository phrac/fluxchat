//! A throwaway HTTP server on localhost, for tests of code that talks to a
//! homeserver or downloads something.

use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpListener;

/// Serve every connection one response — whatever `respond` returns for the
/// request line (`GET /path HTTP/1.1`), written as is — and return the
/// server's base URL. Requests are expected to have no body.
pub(crate) async fn serve(respond: impl Fn(&str) -> Vec<u8> + Send + Sync + 'static) -> String {
    let listener = TcpListener::bind("127.0.0.1:0").await.expect("a local port");
    let addr = listener.local_addr().expect("its address");
    run(listener, respond);
    format!("http://{addr}")
}

/// Start serving at `base`, a URL `nowhere` gave out: the server that
/// couldn't be reached comes up.
pub(crate) async fn serve_at(base: &str, respond: impl Fn(&str) -> Vec<u8> + Send + Sync + 'static) {
    let addr = base.strip_prefix("http://").expect("a URL from `nowhere`");
    run(TcpListener::bind(addr).await.expect("the port is still free"), respond);
}

fn run(listener: TcpListener, respond: impl Fn(&str) -> Vec<u8> + Send + Sync + 'static) {
    let respond = std::sync::Arc::new(respond);
    tokio::spawn(async move {
        while let Ok((mut socket, _)) = listener.accept().await {
            let respond = respond.clone();
            tokio::spawn(async move {
                let mut head = Vec::new();
                let mut chunk = [0u8; 4096];
                while !head.windows(4).any(|w| w == b"\r\n\r\n") {
                    match socket.read(&mut chunk).await {
                        Ok(0) | Err(_) => return,
                        Ok(n) => head.extend_from_slice(&chunk[..n]),
                    }
                }
                let head = String::from_utf8_lossy(&head).into_owned();
                let response = respond(head.lines().next().unwrap_or(""));
                let _ = socket.write_all(&response).await;
                let _ = socket.shutdown().await;
            });
        }
    });
}

/// A complete response with a JSON body.
pub(crate) fn json(status: &str, body: &str) -> Vec<u8> {
    format!(
        "HTTP/1.1 {status}\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
        body.len(),
    )
    .into_bytes()
}

/// A server that takes connections and never answers; returns its base URL.
pub(crate) async fn stalled() -> String {
    let listener = TcpListener::bind("127.0.0.1:0").await.expect("a local port");
    let addr = listener.local_addr().expect("its address");
    tokio::spawn(async move {
        let mut held = Vec::new();
        while let Ok((socket, _)) = listener.accept().await {
            held.push(socket);
        }
    });
    format!("http://{addr}")
}

/// A base URL nothing listens on (connections are refused).
pub(crate) async fn nowhere() -> String {
    let listener = TcpListener::bind("127.0.0.1:0").await.expect("a local port");
    let addr = listener.local_addr().expect("its address");
    drop(listener);
    format!("http://{addr}")
}
