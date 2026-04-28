use hyper::service::{make_service_fn, service_fn};
use hyper::Server;
use std::collections::HashMap;
use std::convert::Infallible;
use std::net::{IpAddr, SocketAddr};
use std::sync::{Arc, Mutex};
use std::time::Duration;
use tokio::net::TcpListener;
use tokio::sync::Semaphore;
use tokio::time::timeout;
use tokio_rustls::TlsAcceptor;

const GEMINI_TLS_HANDSHAKE_TIMEOUT: Duration = Duration::from_secs(10);
const GEMINI_MAX_CONCURRENT: usize = 256;
const GEMINI_MAX_PER_IP: usize = 4;

/// RAII guard that decrements a per-IP connection counter on drop.
struct PerIpGuard {
    table: Arc<Mutex<HashMap<IpAddr, usize>>>,
    ip: IpAddr,
}

impl PerIpGuard {
    /// Try to acquire a slot for `ip`. Returns `None` if the per-IP cap is hit.
    fn try_acquire(table: &Arc<Mutex<HashMap<IpAddr, usize>>>, ip: IpAddr) -> Option<Self> {
        let mut t = table.lock().unwrap();
        let count = t.entry(ip).or_insert(0);
        if *count >= GEMINI_MAX_PER_IP {
            return None;
        }
        *count += 1;
        Some(Self { table: Arc::clone(table), ip })
    }
}

impl Drop for PerIpGuard {
    fn drop(&mut self) {
        let mut t = self.table.lock().unwrap();
        if let Some(c) = t.get_mut(&self.ip) {
            *c -= 1;
            if *c == 0 {
                t.remove(&self.ip);
            }
        }
    }
}

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
        let cert_data = match std::env::var("STATE_DIRECTORY").ok() {
            Some(dir) if !dir.is_empty() => {
                let path = std::path::PathBuf::from(dir);
                acme::load_or_generate_persistent_certificate(&path, &domain)
                    .expect("Failed to load/generate persistent Gemini certificate")
            }
            _ => acme::generate_self_signed_certificate(&domain)
                .expect("Failed to generate self-signed certificate for Gemini"),
        };
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
    let semaphore = Arc::new(Semaphore::new(GEMINI_MAX_CONCURRENT));
    let per_ip: Arc<Mutex<HashMap<IpAddr, usize>>> = Arc::new(Mutex::new(HashMap::new()));

    loop {
        let (stream, peer_addr) = listener.accept().await?;
        let tls_acceptor = tls_acceptor.clone();

        // Drop connections over the cap rather than queuing unbounded work.
        let permit = match Arc::clone(&semaphore).try_acquire_owned() {
            Ok(p) => p,
            Err(_) => {
                if std::env::var("DEBUG_GEMINI").is_ok() {
                    eprintln!("Gemini connection dropped (at cap) from {}", peer_addr);
                }
                drop(stream);
                continue;
            }
        };

        // Per-IP cap: cheap defense against a single peer hogging permits.
        let ip_guard = match PerIpGuard::try_acquire(&per_ip, peer_addr.ip()) {
            Some(g) => g,
            None => {
                if std::env::var("DEBUG_GEMINI").is_ok() {
                    eprintln!("Gemini connection dropped (per-IP cap) from {}", peer_addr);
                }
                drop(stream);
                continue;
            }
        };

        tokio::spawn(async move {
            let _permit = permit;
            let _ip_guard = ip_guard;
            let tls_stream = match timeout(
                GEMINI_TLS_HANDSHAKE_TIMEOUT,
                tls_acceptor.accept(stream),
            )
            .await
            {
                Ok(Ok(s)) => s,
                Ok(Err(e)) => {
                    if std::env::var("DEBUG_GEMINI").is_ok() {
                        eprintln!("Gemini TLS error from {}: {}", peer_addr, e);
                    }
                    return;
                }
                Err(_) => {
                    if std::env::var("DEBUG_GEMINI").is_ok() {
                        eprintln!("Gemini TLS handshake timeout from {}", peer_addr);
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
