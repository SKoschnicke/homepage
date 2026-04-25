//! Gemini protocol handler
//!
//! Gemini is a lightweight protocol that sits between Gopher and HTTP.
//! Request: just the URL followed by CRLF (max 1024 bytes)
//! Response: status code + space + meta + CRLF + body
//!
//! Status codes:
//! - 20: Success (meta is MIME type)
//! - 30: Redirect (meta is new URL)
//! - 40: Temporary failure
//! - 50: Permanent failure
//! - 51: Not found
//! - 59: Bad request

use std::collections::HashMap;
use std::time::Duration;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpStream;
use tokio::time::timeout;
use tokio_rustls::server::TlsStream;

use crate::assets::{get_gemini_routes, GeminiAsset};

lazy_static::lazy_static! {
    static ref GEMINI_ROUTES: HashMap<&'static str, &'static GeminiAsset> = get_gemini_routes();
}

const MAX_REQUEST_SIZE: usize = 1024;
const REQUEST_READ_TIMEOUT: Duration = Duration::from_secs(10);

/// Return the number of Gemini routes
pub fn route_count() -> usize {
    GEMINI_ROUTES.len()
}

/// Handle a single Gemini connection
pub async fn handle_connection(
    mut stream: TlsStream<TcpStream>,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    // Read request (URL + CRLF, max 1024 bytes per spec)
    let mut buf = [0u8; MAX_REQUEST_SIZE + 2]; // +2 for CRLF
    let mut pos = 0;

    let read_result = timeout(REQUEST_READ_TIMEOUT, async {
        loop {
            let n = stream.read(&mut buf[pos..]).await?;
            if n == 0 {
                // Connection closed before complete request
                return Ok::<Option<usize>, std::io::Error>(None);
            }
            pos += n;

            // Check for CRLF
            if pos >= 2 && &buf[pos - 2..pos] == b"\r\n" {
                return Ok(Some(pos));
            }

            if pos >= MAX_REQUEST_SIZE + 2 {
                // Request too long
                return Ok(Some(0));
            }
        }
    })
    .await;

    match read_result {
        Err(_) => {
            // Timed out — best-effort status then drop
            let _ = stream.write_all(b"59 Request timeout\r\n").await;
            return Ok(());
        }
        Ok(Ok(None)) => return Ok(()),
        Ok(Ok(Some(0))) => {
            stream.write_all(b"59 Request exceeds maximum size\r\n").await?;
            return Ok(());
        }
        Ok(Ok(Some(_))) => {}
        Ok(Err(e)) => return Err(e.into()),
    }

    // Parse URL (strip CRLF)
    let request = match std::str::from_utf8(&buf[..pos - 2]) {
        Ok(s) => s,
        Err(_) => {
            stream.write_all(b"59 Invalid UTF-8 in request\r\n").await?;
            return Ok(());
        }
    };

    // Parse as URL
    let url = match url::Url::parse(request) {
        Ok(u) => u,
        Err(_) => {
            stream.write_all(b"59 Invalid URL\r\n").await?;
            return Ok(());
        }
    };

    // Only handle gemini:// scheme
    if url.scheme() != "gemini" {
        stream.write_all(b"59 Only gemini:// URLs are supported\r\n").await?;
        return Ok(());
    }

    let path = url.path();

    // Route to content. Write header then body in two calls so we don't
    // allocate a Vec just to concatenate a static header with static content.
    match lookup(path) {
        Some(asset) => {
            let header = format!("20 {}\r\n", asset.content_type);
            stream.write_all(header.as_bytes()).await?;
            stream.write_all(asset.content).await?;
        }
        None => {
            stream.write_all(b"51 Not found\r\n").await?;
        }
    }

    // Flush any TLS buffer and send close_notify. Without this, large
    // bodies (e.g. images) get truncated when the stream drops mid-flush.
    let _ = stream.shutdown().await;

    Ok(())
}

/// Look up the static asset for a Gemini path, if any.
fn lookup(path: &str) -> Option<&'static GeminiAsset> {
    let path = if path.is_empty() { "/" } else { path };

    if let Some(asset) = GEMINI_ROUTES.get(path) {
        return Some(asset);
    }

    if path.len() > 1 && path.ends_with('/') {
        let without_slash = &path[..path.len() - 1];
        if let Some(asset) = GEMINI_ROUTES.get(without_slash) {
            return Some(asset);
        }
    }

    if !path.ends_with('/') {
        let with_slash = format!("{}/", path);
        if let Some(asset) = GEMINI_ROUTES.get(with_slash.as_str()) {
            return Some(asset);
        }
    }

    None
}
