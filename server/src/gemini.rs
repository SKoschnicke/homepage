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
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpStream;
use tokio_rustls::server::TlsStream;

use crate::assets::{get_gemini_routes, GeminiAsset};

lazy_static::lazy_static! {
    static ref GEMINI_ROUTES: HashMap<&'static str, &'static GeminiAsset> = get_gemini_routes();
}

const MAX_REQUEST_SIZE: usize = 1024;

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

    loop {
        let n = stream.read(&mut buf[pos..]).await?;
        if n == 0 {
            // Connection closed before complete request
            return Ok(());
        }
        pos += n;

        // Check for CRLF
        if pos >= 2 && &buf[pos - 2..pos] == b"\r\n" {
            break;
        }

        if pos >= MAX_REQUEST_SIZE + 2 {
            // Request too long
            stream.write_all(b"59 Request exceeds maximum size\r\n").await?;
            return Ok(());
        }
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

    // Route to content
    let response = route(path);
    stream.write_all(&response).await?;

    Ok(())
}

/// Route a Gemini request to the appropriate content
fn route(path: &str) -> Vec<u8> {
    // Normalize path
    let path = if path.is_empty() { "/" } else { path };

    // Try exact match
    if let Some(asset) = GEMINI_ROUTES.get(path) {
        return success_response(asset.content);
    }

    // Try without trailing slash
    if path.len() > 1 && path.ends_with('/') {
        let without_slash = &path[..path.len() - 1];
        if let Some(asset) = GEMINI_ROUTES.get(without_slash) {
            return success_response(asset.content);
        }
    }

    // Try with trailing slash (directory)
    if !path.ends_with('/') {
        let with_slash = format!("{}/", path);
        if let Some(asset) = GEMINI_ROUTES.get(with_slash.as_str()) {
            return success_response(asset.content);
        }
    }

    // Not found
    not_found_response()
}

/// Build a success response (status 20)
fn success_response(content: &[u8]) -> Vec<u8> {
    let mut response = b"20 text/gemini\r\n".to_vec();
    response.extend_from_slice(content);
    response
}

/// Build a not found response (status 51)
fn not_found_response() -> Vec<u8> {
    b"51 Not found\r\n".to_vec()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_success_response() {
        let response = success_response(b"# Hello\nWorld");
        assert!(response.starts_with(b"20 text/gemini\r\n"));
        assert!(response.ends_with(b"# Hello\nWorld"));
    }

    #[test]
    fn test_not_found_response() {
        let response = not_found_response();
        assert_eq!(response, b"51 Not found\r\n");
    }
}
