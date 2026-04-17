use hyper::{Body, Request, Response, StatusCode, header, upgrade::Upgraded};
use tokio_tungstenite::{
    WebSocketStream,
    tungstenite::{Message, protocol::WebSocketConfig},
};
use futures_util::{StreamExt, SinkExt};
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::sync::Semaphore;
use tokio::time::timeout;
use crate::metrics::Metrics;

// A metrics feed has no reason to receive anything bigger than control frames.
const WS_MAX_MESSAGE_SIZE: usize = 16 * 1024;
const WS_MAX_FRAME_SIZE: usize = 16 * 1024;
const WS_MAX_CLIENTS: usize = 64;
const WS_PING_INTERVAL: Duration = Duration::from_secs(30);
const WS_PONG_DEADLINE: Duration = Duration::from_secs(60);
const WS_SEND_TIMEOUT: Duration = Duration::from_secs(10);

lazy_static::lazy_static! {
    static ref WS_CLIENTS: Arc<Semaphore> = Arc::new(Semaphore::new(WS_MAX_CLIENTS));
}

pub async fn handle_websocket(
    req: Request<Body>,
    metrics: Arc<Metrics>,
) -> Result<Response<Body>, hyper::http::Error> {
    let headers = req.headers();
    let is_upgrade = headers
        .get(header::CONNECTION)
        .and_then(|v| v.to_str().ok())
        .map(|v| v.to_lowercase().contains("upgrade"))
        .unwrap_or(false);

    let is_websocket = headers
        .get(header::UPGRADE)
        .and_then(|v| v.to_str().ok())
        .map(|v| v.to_lowercase() == "websocket")
        .unwrap_or(false);

    if !is_upgrade || !is_websocket {
        return Response::builder()
            .status(StatusCode::BAD_REQUEST)
            .body(Body::from("Expected WebSocket upgrade"));
    }

    // RFC 6455 §4.2.1: Sec-WebSocket-Version must be 13.
    let version_ok = headers
        .get("Sec-WebSocket-Version")
        .and_then(|v| v.to_str().ok())
        .map(|v| v.trim() == "13")
        .unwrap_or(false);
    if !version_ok {
        return Response::builder()
            .status(StatusCode::BAD_REQUEST)
            .header("Sec-WebSocket-Version", "13")
            .body(Body::from("Unsupported WebSocket version"));
    }

    // Require a syntactically valid Sec-WebSocket-Key (16 bytes base64 = 24 chars).
    let accept_key = match compute_accept_key(headers) {
        Some(k) => k,
        None => {
            return Response::builder()
                .status(StatusCode::BAD_REQUEST)
                .body(Body::from("Missing or invalid Sec-WebSocket-Key"));
        }
    };

    // Cap concurrent clients. Refuse rather than queue.
    let permit = match Arc::clone(&WS_CLIENTS).try_acquire_owned() {
        Ok(p) => p,
        Err(_) => {
            return Response::builder()
                .status(StatusCode::SERVICE_UNAVAILABLE)
                .header(header::RETRY_AFTER, "30")
                .body(Body::from("WebSocket client limit reached"));
        }
    };

    tokio::spawn(async move {
        let _permit = permit;
        match hyper::upgrade::on(req).await {
            Ok(upgraded) => {
                if let Err(e) = websocket_loop(upgraded, metrics).await {
                    eprintln!("WebSocket error: {}", e);
                }
            }
            Err(e) => eprintln!("Upgrade error: {}", e),
        }
    });

    Response::builder()
        .status(StatusCode::SWITCHING_PROTOCOLS)
        .header(header::CONNECTION, "Upgrade")
        .header(header::UPGRADE, "websocket")
        .header("Sec-WebSocket-Accept", accept_key)
        .body(Body::empty())
}

async fn websocket_loop(
    upgraded: Upgraded,
    metrics: Arc<Metrics>,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    metrics.increment_ws_clients();

    let config = WebSocketConfig {
        max_message_size: Some(WS_MAX_MESSAGE_SIZE),
        max_frame_size: Some(WS_MAX_FRAME_SIZE),
        ..Default::default()
    };

    let ws_stream = WebSocketStream::from_raw_socket(
        upgraded,
        tokio_tungstenite::tungstenite::protocol::Role::Server,
        Some(config),
    ).await;

    let (mut tx, mut rx) = ws_stream.split();
    let mut broadcast = tokio::time::interval(Duration::from_secs(1));
    let mut ping = tokio::time::interval(WS_PING_INTERVAL);
    ping.tick().await; // skip the immediate first tick
    let mut last_pong = Instant::now();

    let result = async {
        loop {
            tokio::select! {
                _ = broadcast.tick() => {
                    let snapshot = metrics.snapshot();
                    let json = serde_json::to_string(&snapshot)?;

                    // If the client stops reading, the socket buffer fills,
                    // send() blocks — bail out instead of growing memory forever.
                    match timeout(WS_SEND_TIMEOUT, tx.send(Message::Text(json))).await {
                        Ok(Ok(())) => {}
                        Ok(Err(e)) => {
                            eprintln!("Failed to send metrics: {}", e);
                            break;
                        }
                        Err(_) => {
                            eprintln!("WebSocket send timeout; dropping client");
                            break;
                        }
                    }
                }

                _ = ping.tick() => {
                    if last_pong.elapsed() > WS_PONG_DEADLINE {
                        break;
                    }
                    match timeout(WS_SEND_TIMEOUT, tx.send(Message::Ping(Vec::new()))).await {
                        Ok(Ok(())) => {}
                        Ok(Err(_)) | Err(_) => break,
                    }
                }

                msg = rx.next() => {
                    match msg {
                        Some(Ok(Message::Ping(data))) => {
                            if tx.send(Message::Pong(data)).await.is_err() {
                                break;
                            }
                        }
                        Some(Ok(Message::Pong(_))) => {
                            last_pong = Instant::now();
                        }
                        Some(Ok(Message::Close(_))) | None => {
                            break;
                        }
                        Some(Err(e)) => {
                            eprintln!("WebSocket receive error: {}", e);
                            break;
                        }
                        _ => {}
                    }
                }
            }
        }
        Ok::<(), Box<dyn std::error::Error + Send + Sync>>(())
    }.await;

    metrics.decrement_ws_clients();
    result
}

fn compute_accept_key(headers: &hyper::HeaderMap) -> Option<String> {
    use sha1::{Sha1, Digest};
    use base64::Engine as _;

    const WS_GUID: &str = "258EAFA5-E914-47DA-95CA-C5AB0DC85B11";

    let key = headers
        .get("Sec-WebSocket-Key")
        .and_then(|v| v.to_str().ok())?
        .trim();

    // RFC 6455: the key is base64 of a 16-byte nonce — exactly 24 chars.
    if key.len() != 24 {
        return None;
    }
    let decoded = base64::engine::general_purpose::STANDARD.decode(key).ok()?;
    if decoded.len() != 16 {
        return None;
    }

    let mut hasher = Sha1::new();
    hasher.update(key.as_bytes());
    hasher.update(WS_GUID.as_bytes());
    let result = hasher.finalize();

    Some(base64::engine::general_purpose::STANDARD.encode(result))
}
