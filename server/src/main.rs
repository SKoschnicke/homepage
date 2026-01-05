use hyper::service::{make_service_fn, service_fn};
use hyper::{Body, Request, Response, Server, StatusCode};
use std::convert::Infallible;
use std::net::SocketAddr;
use std::sync::Arc;
use tokio::net::TcpListener;
use tokio_rustls::TlsAcceptor;

mod acme;
mod assets;
mod config;
mod metrics;
mod router;
mod s3_storage;
mod websocket;

#[global_allocator]
static GLOBAL: mimalloc::MiMalloc = mimalloc::MiMalloc;

#[tokio::main]
async fn main() {
    // 1. Parse environment config
    let config = match config::Config::load_from_env() {
        Ok(cfg) => cfg,
        Err(e) => {
            eprintln!("FATAL: Configuration error: {}", e);
            eprintln!("\nRequired environment variables:");
            eprintln!("  DOMAIN - Domain for TLS certificate (e.g., sven.guru)");
            eprintln!("  ACME_CONTACT_EMAIL - Let's Encrypt contact email");
            eprintln!("  S3_ENDPOINT - S3-compatible endpoint (e.g., https://fsn1.your-objectstorage.com)");
            eprintln!("  S3_BUCKET - S3 bucket name for certificates");
            eprintln!("  S3_ACCESS_KEY - S3 access key");
            eprintln!("  S3_SECRET_KEY - S3 secret key");
            eprintln!("\nOptional:");
            eprintln!("  ACME_STAGING - Set to 'true' to use Let's Encrypt staging (default: false)");
            eprintln!("  S3_REGION - S3 region (default: us-east-1)");
            std::process::exit(1);
        }
    };

    println!("Configuration loaded:");
    println!("  Domain: {}", config.domain);
    println!("  Local Dev Mode: {}", config.local_dev);
    println!("  ACME Contact: {}", config.acme_contact);
    println!("  ACME Staging: {}", config.acme_staging);
    println!("  S3 Bucket: {}", config.s3_bucket);

    // 2. Get TLS certificate (local dev mode or production)
    let tls_config = if config.local_dev {
        // Local development: Use self-signed certificate
        println!("\n[LOCAL DEV MODE] Generating self-signed certificate...");
        match acme::generate_self_signed_certificate(&config.domain).await {
            Ok(cfg) => cfg,
            Err(e) => {
                eprintln!("\nFATAL: Failed to generate self-signed certificate: {}", e);
                std::process::exit(1);
            }
        }
    } else {
        // Production: Use Let's Encrypt with S3 storage
        println!("\nInitializing S3 client...");
        let s3_client = s3_storage::init_s3_client(&config).await;

        println!("\nObtaining TLS certificate...");
        match acme::get_or_create_certificate(
            &config.domain,
            &config.acme_contact,
            config.acme_staging,
            &s3_client,
            &config.s3_bucket,
        )
        .await
        {
            Ok(cfg) => cfg,
            Err(e) => {
                eprintln!("\nFATAL: Failed to obtain TLS certificate: {}", e);
                eprintln!("\nPossible causes:");
                eprintln!("  1. DNS record for {} does not point to this server", config.domain);
                eprintln!("  2. Port 80 is blocked by firewall or already in use");
                eprintln!("  3. S3 bucket '{}' doesn't exist or is inaccessible", config.s3_bucket);
                eprintln!("  4. Let's Encrypt rate limit reached (try ACME_STAGING=true)");
                eprintln!("\nVerify DNS with: dig {} +short", config.domain);
                std::process::exit(1);
            }
        }
    };

    // 4. Initialize metrics
    let metrics = metrics::Metrics::new();

    // 5. Start HTTP server (port 80) - redirects to HTTPS
    let http_metrics = Arc::clone(&metrics);
    let http_domain = config.domain.clone();
    let http_server = tokio::spawn(async move {
        start_http_redirect_server(http_metrics, http_domain).await
    });

    // 6. Start HTTPS server (port 443) - main content
    let https_metrics = Arc::clone(&metrics);
    let https_server = tokio::spawn(async move {
        start_https_server(tls_config, https_metrics).await
    });

    // 7. Display startup message
    println!("\n========================================");
    println!("Server running:");
    println!("  HTTP:  http://0.0.0.0:80 (redirects to HTTPS)");
    println!("  HTTPS: https://{}:443", config.domain);
    println!("  Routes: {}", router::route_count());
    println!("========================================\n");

    // 8. Run both servers concurrently
    match tokio::try_join!(http_server, https_server) {
        Ok(_) => {}
        Err(e) => {
            eprintln!("Server error: {}", e);
            std::process::exit(1);
        }
    }
}

async fn start_http_redirect_server(_metrics: Arc<metrics::Metrics>, domain: String) {
    let addr = SocketAddr::from(([0, 0, 0, 0], 80));

    let make_svc = make_service_fn(move |_conn| {
        let domain = domain.clone();
        async move {
            Ok::<_, Infallible>(service_fn(move |req: Request<Body>| {
                let domain = domain.clone();
                async move { handle_http_redirect(req, domain).await }
            }))
        }
    });

    let server = Server::bind(&addr).serve(make_svc);

    println!("HTTP redirect server listening on {}", addr);

    if let Err(e) = server.await {
        eprintln!("HTTP server error: {}", e);
        std::process::exit(1);
    }
}

async fn handle_http_redirect(
    req: Request<Body>,
    domain: String,
) -> Result<Response<Body>, Infallible> {
    let path = req.uri().path();
    let query = req.uri().query().map(|q| format!("?{}", q)).unwrap_or_default();

    // Allow ACME HTTP-01 challenges (for future cert renewal)
    if path.starts_with("/.well-known/acme-challenge/") {
        return Ok(Response::builder()
            .status(StatusCode::NOT_FOUND)
            .body(Body::from("Not Found"))
            .unwrap());
    }

    // Redirect all other requests to HTTPS
    let https_url = format!("https://{}{}{}", domain, path, query);

    Ok(Response::builder()
        .status(StatusCode::MOVED_PERMANENTLY)
        .header("location", https_url)
        .body(Body::from("Redirecting to HTTPS"))
        .unwrap())
}

async fn start_https_server(
    tls_config: Arc<rustls::ServerConfig>,
    metrics: Arc<metrics::Metrics>,
) {
    let addr = SocketAddr::from(([0, 0, 0, 0], 443));
    let listener = TcpListener::bind(&addr).await.unwrap();

    println!("HTTPS server listening on {}", addr);

    let tls_acceptor = TlsAcceptor::from(tls_config);

    loop {
        let (stream, _peer_addr) = match listener.accept().await {
            Ok(conn) => conn,
            Err(e) => {
                eprintln!("TCP accept error: {}", e);
                continue;
            }
        };

        let tls_acceptor = tls_acceptor.clone();
        let metrics = Arc::clone(&metrics);

        tokio::spawn(async move {
            // Wrap in TLS
            let tls_stream = match tls_acceptor.accept(stream).await {
                Ok(stream) => stream,
                Err(e) => {
                    eprintln!("TLS accept error: {}", e);
                    return;
                }
            };

            // Serve HTTP over TLS
            let service = service_fn(move |req| {
                let metrics = Arc::clone(&metrics);
                router::route(req, metrics)
            });

            if let Err(e) = hyper::server::conn::Http::new()
                .serve_connection(tls_stream, service)
                .with_upgrades()
                .await
            {
                eprintln!("HTTPS connection error: {}", e);
            }
        });
    }
}
