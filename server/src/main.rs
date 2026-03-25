use hyper::service::{make_service_fn, service_fn};
use hyper::Server;
use std::convert::Infallible;
use std::net::SocketAddr;
use std::sync::Arc;
use tokio::net::TcpListener;
use tokio_rustls::TlsAcceptor;

mod acme;
mod assets;
mod gemini;
mod metrics;
mod router;
mod websocket;

#[global_allocator]
static GLOBAL: mimalloc::MiMalloc = mimalloc::MiMalloc;

#[tokio::main]
async fn main() {
    let port: u16 = std::env::var("PORT")
        .ok()
        .and_then(|p| p.parse().ok())
        .unwrap_or(8080);

    let addr = SocketAddr::from(([127, 0, 0, 1], port));

    let metrics = metrics::Metrics::new();

    let make_svc = make_service_fn(move |_conn| {
        let metrics = Arc::clone(&metrics);
        async move {
            Ok::<_, Infallible>(service_fn(move |req| {
                let metrics = Arc::clone(&metrics);
                router::route(req, metrics)
            }))
        }
    });

    let server = Server::bind(&addr).serve(make_svc);

    println!("HTTP server listening on http://{}", addr);
    println!("Serving {} routes", router::route_count());

    // Start Gemini server if content exists
    let gemini_enabled = std::env::var("ENABLE_GEMINI")
        .unwrap_or_else(|_| "true".to_string()) == "true";

    if gemini_enabled && gemini::route_count() > 0 {
        let domain = std::env::var("DOMAIN").unwrap_or_else(|_| "localhost".to_string());
        let cert_data = acme::generate_self_signed_certificate(&domain)
            .expect("Failed to generate self-signed certificate for Gemini");
        let tls_config = acme::build_tls_config(&cert_data.cert_pem, &cert_data.privkey_pem)
            .expect("Failed to build TLS config for Gemini");

        let gemini_port = 1965u16;
        tokio::spawn(async move {
            if let Err(e) = start_gemini_server(tls_config, gemini_port).await {
                eprintln!("Gemini server error: {}", e);
            }
        });

        println!("Gemini server listening on gemini://{}:{}", domain, gemini_port);
        println!("Serving {} Gemini routes", gemini::route_count());
    }

    if let Err(e) = server.await {
        eprintln!("Server error: {}", e);
        std::process::exit(1);
    }
}

async fn start_gemini_server(
    tls_config: Arc<tokio_rustls::rustls::ServerConfig>,
    port: u16,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let addr = SocketAddr::from(([0, 0, 0, 0], port));
    let listener = TcpListener::bind(&addr).await?;
    let tls_acceptor = TlsAcceptor::from(tls_config);

    loop {
        let (stream, peer_addr) = listener.accept().await?;
        let tls_acceptor = tls_acceptor.clone();

        tokio::spawn(async move {
            let tls_stream = match tls_acceptor.accept(stream).await {
                Ok(s) => s,
                Err(e) => {
                    if std::env::var("DEBUG_GEMINI").is_ok() {
                        eprintln!("Gemini TLS error from {}: {}", peer_addr, e);
                    }
                    return;
                }
            };

            if let Err(e) = gemini::handle_connection(tls_stream).await {
                eprintln!("Gemini connection error from {}: {}", peer_addr, e);
            }
        });
    }
}
