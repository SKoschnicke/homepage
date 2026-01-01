use hyper::{Body, Request, Response, StatusCode, header, upgrade::Upgraded};
use tokio_tungstenite::{WebSocketStream, tungstenite::Message};
use futures_util::{StreamExt, SinkExt};
use std::sync::Arc;
use std::time::Duration;
use crate::metrics::Metrics;

pub async fn handle_websocket(
    req: Request<Body>,
    metrics: Arc<Metrics>,
) -> Result<Response<Body>, hyper::http::Error> {
    // Check if this is a WebSocket upgrade request
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
            .body(Body::from("Expected WebSocket upgrade"))
    }

    // Compute accept key before moving req
    let accept_key = compute_accept_key(headers);

    // Spawn task to handle WebSocket upgrade
    tokio::spawn(async move {
        match hyper::upgrade::on(req).await {
            Ok(upgraded) => {
                if let Err(e) = websocket_loop(upgraded, metrics).await {
                    eprintln!("WebSocket error: {}", e);
                }
            }
            Err(e) => eprintln!("Upgrade error: {}", e),
        }
    });

    // Return 101 Switching Protocols response
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
    // Increment WebSocket client count on connect
    metrics.increment_ws_clients();

    let ws_stream = WebSocketStream::from_raw_socket(
        upgraded,
        tokio_tungstenite::tungstenite::protocol::Role::Server,
        None,
    ).await;

    let (mut tx, mut rx) = ws_stream.split();
    let mut interval = tokio::time::interval(Duration::from_secs(1));

    let result = async {
        loop {
        tokio::select! {
            // Broadcast metrics every second
            _ = interval.tick() => {
                let snapshot = metrics.snapshot();
                let json = serde_json::to_string(&snapshot)?;

                if let Err(e) = tx.send(Message::Text(json)).await {
                    eprintln!("Failed to send metrics: {}", e);
                    break;
                }
            }

            // Handle incoming messages (ping/pong, close)
            msg = rx.next() => {
                match msg {
                    Some(Ok(Message::Ping(data))) => {
                        tx.send(Message::Pong(data)).await?;
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

    // Decrement WebSocket client count on disconnect
    metrics.decrement_ws_clients();

    result
}

fn compute_accept_key(headers: &hyper::HeaderMap) -> String {
    use sha1::{Sha1, Digest};
    use base64::Engine as _;

    const WS_GUID: &str = "258EAFA5-E914-47DA-95CA-C5AB0DC85B11";

    let key = headers
        .get("Sec-WebSocket-Key")
        .and_then(|v| v.to_str().ok())
        .unwrap_or("");

    let mut hasher = Sha1::new();
    hasher.update(key.as_bytes());
    hasher.update(WS_GUID.as_bytes());
    let result = hasher.finalize();

    base64::engine::general_purpose::STANDARD.encode(result)
}
