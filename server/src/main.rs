use hyper::service::{make_service_fn, service_fn};
use hyper::{Body, Request, Response, Server, StatusCode};
use std::convert::Infallible;
use std::io::Write;
use std::net::SocketAddr;
use std::sync::Arc;
use tokio::net::TcpListener;
use tokio_rustls::TlsAcceptor;

// Import rustls 0.23 as rustls23 for AWS SDK crypto provider
extern crate rustls23;

mod acme;
mod assets;
mod config;
mod gemini;
mod metrics;
mod router;
mod s3_storage;
mod websocket;

#[global_allocator]
static GLOBAL: mimalloc::MiMalloc = mimalloc::MiMalloc;

// Macro to ensure output is flushed before potentially crashing operations
macro_rules! debug_checkpoint {
    ($msg:expr) => {
        eprintln!("[DEBUG CHECKPOINT] {}", $msg);
        let _ = std::io::stderr().flush();
        let _ = std::io::stdout().flush();
    };
}

#[tokio::main]
async fn main() {
    // Set up panic hook to log panics before crashing
    std::panic::set_hook(Box::new(|panic_info| {
        eprintln!("\n!!! PANIC !!!");
        eprintln!("The application panicked: {}", panic_info);
        if let Some(location) = panic_info.location() {
            eprintln!("Panic occurred in file '{}' at line {}", location.file(), location.line());
        }
        eprintln!("This is a bug - the application should handle errors gracefully.");
        eprintln!("!!! END PANIC !!!\n");
        let _ = std::io::stderr().flush();
    }));

    // Install default crypto provider for rustls 0.23 (required for AWS SDK)
    // This must be done before any rustls 0.23 operations (AWS SDK uses rustls 0.23)
    // Note: We use rustls23 (0.23) for AWS SDK, rustls (0.19) for our own TLS
    let _ = rustls23::crypto::aws_lc_rs::default_provider()
        .install_default()
        .map_err(|e| {
            eprintln!("Warning: Failed to install default crypto provider: {:?}", e);
            eprintln!("This is usually OK if already installed by another part of the application");
        });

    debug_checkpoint!("Application starting");
    println!("=== Static Server Starting ===");
    let _ = std::io::stdout().flush();

    // Check if HTTPS is enabled via environment
    let https_enabled = std::env::var("ENABLE_HTTPS").unwrap_or_default() == "true";

    if https_enabled {
        println!("\n[HTTPS MODE ENABLED]");
        println!("Attempting to start HTTPS server...");

        match run_https_server().await {
            Ok(_) => {
                println!("HTTPS server exited normally");
            }
            Err(e) => {
                eprintln!("HTTPS server failed: {}", e);
                eprintln!("Falling back to simple HTTP server...");
                if let Err(e) = run_simple_server().await {
                    eprintln!("Simple HTTP server also failed: {}", e);
                    std::process::exit(1);
                }
            }
        }
    } else {
        println!("\n[SIMPLE HTTP MODE]");
        println!("HTTPS is disabled. Set ENABLE_HTTPS=true to enable.");

        if let Err(e) = run_simple_server().await {
            eprintln!("Server error: {}", e);
            std::process::exit(1);
        }
    }
}

async fn run_simple_server() -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    println!("\nStarting simple HTTP server...");

    let port = std::env::var("PORT")
        .ok()
        .and_then(|p| p.parse().ok())
        .unwrap_or(80);

    let addr = SocketAddr::from(([0, 0, 0, 0], port));

    // Initialize metrics collector
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

    let server = Server::bind(&addr)
        .serve(make_svc);

    println!("✓ HTTP server listening on http://{}", addr);
    println!("✓ Serving {} HTTP routes", router::route_count());

    // Start Gemini server with self-signed cert (if enabled)
    let gemini_enabled = std::env::var("ENABLE_GEMINI").unwrap_or_else(|_| "true".to_string()) == "true";

    if gemini_enabled && gemini::route_count() > 0 {
        println!("\nStarting Gemini server with self-signed certificate...");

        // Generate self-signed cert for Gemini
        let domain = std::env::var("DOMAIN").unwrap_or_else(|_| "localhost".to_string());
        let cert_data = acme::generate_self_signed_certificate(&domain).await?;
        let tls_config = acme::build_tls_config(&cert_data.cert_pem, &cert_data.privkey_pem)?;

        let gemini_port = 1965u16;
        tokio::spawn(async move {
            if let Err(e) = start_gemini_server(tls_config, gemini_port).await {
                eprintln!("Gemini server error: {}", e);
            }
        });

        println!("✓ Gemini server listening on gemini://{}:{}", domain, gemini_port);
        println!("✓ Serving {} Gemini routes", gemini::route_count());
        println!("  (Using self-signed certificate - clients will show trust warning)");
    } else if gemini_enabled {
        println!("\n(Gemini disabled - no .gmi content in gemini-content/)");
    }

    println!();

    server.await.map_err(|e| e.into())
}

async fn run_https_server() -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    debug_checkpoint!("Entering run_https_server");
    println!("\n=== HTTPS Setup ===");
    eprintln!("[DEBUG] run_https_server: Starting");

    // Step 1: Load configuration
    debug_checkpoint!("Loading configuration");
    println!("[1/5] Loading configuration...");
    eprintln!("[DEBUG] About to load config from env");

    let config = match config::Config::load_from_env() {
        Ok(c) => {
            eprintln!("[DEBUG] Config loaded successfully");
            c
        }
        Err(e) => {
            eprintln!("[FATAL] Failed to load configuration: {}", e);
            eprintln!("[DEBUG] Config error details: {}", e);
            let _ = std::io::stderr().flush();
            return Err(e.into());
        }
    };

    println!("  ✓ Domain: {}", config.domain);
    println!("  ✓ Local Dev Mode: {}", config.local_dev);
    println!("  ✓ ACME Staging: {}", config.acme_staging);
    println!("  ✓ S3 Bucket: {}", config.s3_bucket);
    debug_checkpoint!("Config loaded and printed successfully");

    // Step 2: Initialize S3 client (always - needed for both dev and production)
    debug_checkpoint!("Initializing S3 client");
    println!("\n[2/5] Initializing S3 storage...");
    eprintln!("[DEBUG] About to initialize S3 client");

    let s3_client = match s3_storage::init_s3_client(&config).await {
        Ok(client) => {
            debug_checkpoint!("S3 client initialized successfully");
            eprintln!("[DEBUG] S3 client initialized successfully");
            println!("  ✓ S3 client initialized");
            client
        }
        Err(e) => {
            eprintln!("Failed to initialize S3 client: {}", e);
            eprintln!("Cannot use HTTPS without S3 storage.");
            eprintln!("Falling back to simple HTTP mode...");
            let _ = std::io::stderr().flush();
            return run_simple_server().await;
        }
    };

    // Step 2b: Test S3 storage connectivity
    debug_checkpoint!("Testing S3 storage");
    println!("  Testing S3 storage...");
    eprintln!("[DEBUG] About to test S3 storage");

    if let Err(e) = s3_storage::test_s3_storage(&s3_client, &config.s3_bucket).await {
        eprintln!("S3 storage test failed: {}", e);
        eprintln!("Cannot persist certificates without working S3 storage.");
        eprintln!("Falling back to simple HTTP mode...");
        let _ = std::io::stderr().flush();
        return run_simple_server().await;
    }
    println!("  ✓ S3 storage is working");
    debug_checkpoint!("S3 storage test passed");

    // Step 3: Get TLS certificate (safe now - S3 verified)
    debug_checkpoint!("Starting TLS certificate acquisition");
    println!("\n[3/5] Obtaining TLS certificate...");
    eprintln!("[DEBUG] About to obtain TLS certificate");

    let tls_config = if config.local_dev {
        debug_checkpoint!("Using local dev mode - self-signed cert with S3 caching");
        println!("  Local dev mode: self-signed certificate with S3 caching...");
        eprintln!("[DEBUG] Calling get_or_create_self_signed_certificate");

        match acme::get_or_create_self_signed_certificate(
            &config.domain,
            &s3_client,
            &config.s3_bucket,
        ).await {
            Ok(cfg) => {
                eprintln!("[DEBUG] Self-signed certificate ready");
                cfg
            }
            Err(e) => {
                eprintln!("[FATAL] Failed to get/create self-signed certificate: {}", e);
                let _ = std::io::stderr().flush();
                return Err(e);
            }
        }
    } else {
        debug_checkpoint!("Using production mode - Let's Encrypt with S3 caching");
        println!("  Production mode: Let's Encrypt with S3 caching...");
        eprintln!("[DEBUG] Calling get_or_create_certificate");

        match acme::get_or_create_certificate(
            &config.domain,
            &config.acme_contact,
            config.acme_staging,
            &s3_client,
            &config.s3_bucket,
        ).await {
            Ok(cert) => {
                debug_checkpoint!("Certificate obtained successfully");
                eprintln!("[DEBUG] Certificate obtained successfully");
                cert
            }
            Err(e) => {
                eprintln!("[FATAL] Failed to obtain certificate: {}", e);
                let _ = std::io::stderr().flush();
                return Err(e);
            }
        }
    };
    debug_checkpoint!("TLS certificate ready");
    println!("  ✓ TLS certificate ready");
    eprintln!("[DEBUG] TLS config ready");

    // Step 4: Initialize metrics
    println!("\n[4/5] Initializing metrics...");
    let metrics = metrics::Metrics::new();
    println!("  ✓ Metrics initialized");

    // Determine ports based on local dev mode
    let (http_port, https_port) = if config.local_dev {
        (8080, 8443)
    } else {
        (80, 443)
    };

    // Step 5: Start HTTP redirect server
    println!("\n[5/7] Starting HTTP redirect server (port {})...", http_port);
    let http_metrics = Arc::clone(&metrics);
    let http_domain = config.domain.clone();
    let http_server = tokio::spawn(async move {
        start_http_redirect_server(http_metrics, http_domain, http_port).await
    });
    println!("  ✓ HTTP redirect server started");

    // Step 6: Start HTTPS server
    println!("\n[6/7] Starting HTTPS server (port {})...", https_port);
    let https_tls_config = Arc::clone(&tls_config);
    let https_metrics = Arc::clone(&metrics);
    let https_server = tokio::spawn(async move {
        start_https_server(https_tls_config, https_metrics, https_port).await
    });
    println!("  ✓ HTTPS server started");

    // Step 7: Start Gemini server (reuses TLS config)
    let gemini_enabled = std::env::var("ENABLE_GEMINI").unwrap_or_else(|_| "true".to_string()) == "true";
    let gemini_port = 1965u16;

    let gemini_server = if gemini_enabled {
        println!("\n[7/7] Starting Gemini server (port {})...", gemini_port);
        let gemini_tls_config = Arc::clone(&tls_config);
        let handle = tokio::spawn(async move {
            start_gemini_server(gemini_tls_config, gemini_port).await
        });
        println!("  ✓ Gemini server started");
        Some(handle)
    } else {
        println!("\n[7/7] Gemini server disabled (set ENABLE_GEMINI=true to enable)");
        None
    };

    println!("\n========================================");
    println!("✓ Server Ready!");
    println!("  HTTP:   http://0.0.0.0:{} (→ HTTPS)", http_port);
    println!("  HTTPS:  https://{}:{}", config.domain, https_port);
    if gemini_enabled {
        println!("  Gemini: gemini://{}:{}", config.domain, gemini_port);
    }
    println!("  HTTP Routes:   {}", router::route_count());
    println!("  Gemini Routes: {}", gemini::route_count());
    println!("========================================\n");

    // Wait for servers
    if let Some(gemini_handle) = gemini_server {
        let (http_result, https_result, gemini_result) = tokio::try_join!(
            http_server,
            https_server,
            gemini_handle
        ).map_err(|e| format!("Server task failed: {}", e))?;

        http_result?;
        https_result?;
        gemini_result?;
    } else {
        let (http_result, https_result) = tokio::try_join!(http_server, https_server)
            .map_err(|e| format!("Server task failed: {}", e))?;

        http_result?;
        https_result?;
    }

    Ok(())
}

async fn start_http_redirect_server(
    metrics: Arc<metrics::Metrics>,
    domain: String,
    port: u16,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let addr = SocketAddr::from(([0, 0, 0, 0], port));

    let make_svc = make_service_fn(move |_conn| {
        let metrics = Arc::clone(&metrics);
        let domain = domain.clone();
        async move {
            Ok::<_, Infallible>(service_fn(move |req: Request<Body>| {
                let metrics = Arc::clone(&metrics);
                let domain = domain.clone();
                async move {
                    let start = std::time::Instant::now();

                    // Check for ACME challenge
                    let response = if req.uri().path().starts_with("/.well-known/acme-challenge/") {
                        Response::builder()
                            .status(StatusCode::NOT_FOUND)
                            .body(Body::from("Not found"))
                            .unwrap()
                    } else {
                        // Redirect to HTTPS
                        let location = format!("https://{}{}", domain, req.uri());
                        Response::builder()
                            .status(StatusCode::MOVED_PERMANENTLY)
                            .header("Location", location)
                            .body(Body::empty())
                            .unwrap()
                    };

                    metrics.record_request(start.elapsed());
                    Ok::<_, Infallible>(response)
                }
            }))
        }
    });

    Server::bind(&addr)
        .serve(make_svc)
        .await
        .map_err(|e| e.into())
}

async fn start_https_server(
    tls_config: Arc<tokio_rustls::rustls::ServerConfig>,
    metrics: Arc<metrics::Metrics>,
    port: u16,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let addr = SocketAddr::from(([0, 0, 0, 0], port));
    let listener = TcpListener::bind(&addr).await?;
    let tls_acceptor = TlsAcceptor::from(tls_config);

    loop {
        let (stream, _peer_addr) = listener.accept().await?;
        let tls_acceptor = tls_acceptor.clone();
        let metrics = Arc::clone(&metrics);

        tokio::spawn(async move {
            let tls_stream = match tls_acceptor.accept(stream).await {
                Ok(s) => s,
                Err(e) => {
                    eprintln!("TLS accept error: {}", e);
                    return;
                }
            };

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
                    // TLS errors are common with probes/scanners - log at debug level
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
