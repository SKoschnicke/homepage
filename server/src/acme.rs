use aws_sdk_s3::Client as S3Client;
use hyper::service::{make_service_fn, service_fn};
use hyper::{Body, Request, Response, Server, StatusCode};
use instant_acme::{
    Account, AuthorizationStatus, ChallengeType, Identifier, LetsEncrypt,
    NewAccount, NewOrder, OrderStatus,
};
use rustls::{Certificate, PrivateKey, ServerConfig};
use rustls_pemfile;
use std::io::Cursor;
use std::net::SocketAddr;
use std::sync::Arc;
use tokio::sync::RwLock;

use crate::s3_storage::{self, CertificateData};

const MIN_DAYS_REMAINING: u64 = 30;

pub async fn generate_self_signed_certificate(
    domain: &str,
) -> Result<CertificateData, Box<dyn std::error::Error + Send + Sync>> {
    println!("Generating self-signed certificate for local development...");

    // Generate self-signed certificate using rcgen
    let mut params = rcgen::CertificateParams::new(vec![domain.to_string()]);
    params.distinguished_name = rcgen::DistinguishedName::new();
    params.distinguished_name.push(
        rcgen::DnType::CommonName,
        domain.to_string(),
    );
    params.distinguished_name.push(
        rcgen::DnType::OrganizationName,
        "Local Development".to_string(),
    );

    let cert = rcgen::Certificate::from_params(params)?;
    let cert_pem = cert.serialize_pem()?;
    let privkey_pem = cert.serialize_private_key_pem();

    println!("Self-signed certificate generated for {}", domain);
    println!("WARNING: This certificate is NOT trusted by browsers!");
    println!("         Accept the security warning in your browser to proceed.");

    Ok(CertificateData {
        cert_pem,
        privkey_pem,
    })
}

pub async fn get_or_create_self_signed_certificate(
    domain: &str,
    s3_client: &aws_sdk_s3::Client,
    bucket: &str,
) -> Result<Arc<ServerConfig>, Box<dyn std::error::Error + Send + Sync>> {
    println!("Checking S3 for existing certificate...");
    println!("  Bucket: {}", bucket);
    println!("  Domain: {}", domain);

    // Try to load existing certificate from S3
    match s3_storage::load_certificate(s3_client, bucket, domain).await {
        Ok(Some(cert_data)) => {
            println!("Found certificate in S3, checking expiry...");

            // Check if certificate is still valid
            if s3_storage::cert_is_valid(&cert_data.cert_pem, MIN_DAYS_REMAINING) {
                println!("Certificate is valid (> {} days remaining), using cached cert", MIN_DAYS_REMAINING);
                return build_tls_config(&cert_data.cert_pem, &cert_data.privkey_pem);
            } else {
                println!("Certificate expires soon (< {} days), generating new one", MIN_DAYS_REMAINING);
            }
        }
        Ok(None) => {
            println!("No certificate found in S3, generating new one");
        }
        Err(e) => {
            eprintln!("Error checking S3 for certificate: {}", e);
            eprintln!("Will generate new certificate");
        }
    }

    // Generate new self-signed certificate
    println!("Generating new self-signed certificate...");
    let cert_data = match generate_self_signed_certificate(domain).await {
        Ok(data) => {
            println!("Self-signed certificate generated successfully");
            data
        }
        Err(e) => {
            eprintln!("Failed to generate self-signed certificate: {}", e);
            return Err(e);
        }
    };

    // Save to S3
    println!("Saving certificate to S3...");
    match s3_storage::save_certificate(
        s3_client,
        bucket,
        domain,
        &cert_data.cert_pem,
        &cert_data.privkey_pem,
    ).await {
        Ok(_) => println!("Certificate saved to S3 successfully"),
        Err(e) => {
            eprintln!("Warning: Failed to save certificate to S3: {}", e);
            eprintln!("Continuing with certificate anyway...");
        }
    }

    // Build TLS config
    build_tls_config(&cert_data.cert_pem, &cert_data.privkey_pem)
}

pub async fn get_or_create_certificate(
    domain: &str,
    email: &str,
    staging: bool,
    s3_client: &S3Client,
    bucket: &str,
) -> Result<Arc<ServerConfig>, Box<dyn std::error::Error + Send + Sync>> {
    println!("Checking S3 for existing certificate...");
    println!("  Bucket: {}", bucket);
    println!("  Domain: {}", domain);

    // Try to load existing certificate from S3
    match s3_storage::load_certificate(s3_client, bucket, domain).await {
        Ok(Some(cert_data)) => {
            println!("Found certificate in S3, checking expiry...");

            // Check if certificate is still valid
            if s3_storage::cert_is_valid(&cert_data.cert_pem, MIN_DAYS_REMAINING) {
                println!("Certificate is valid (> {} days remaining), using cached cert", MIN_DAYS_REMAINING);
                return build_tls_config(&cert_data.cert_pem, &cert_data.privkey_pem);
            } else {
                println!("Certificate expires soon (< {} days), requesting new one", MIN_DAYS_REMAINING);
            }
        }
        Ok(None) => {
            println!("No certificate found in S3, requesting new one");
        }
        Err(e) => {
            eprintln!("Error checking S3 for certificate: {}", e);
            eprintln!("Will attempt to request new certificate from Let's Encrypt");
        }
    }

    // Request new certificate
    println!("Requesting new certificate from Let's Encrypt{}...",
        if staging { " (STAGING)" } else { "" });

    let cert_data = match request_new_certificate(domain, email, staging).await {
        Ok(data) => {
            println!("Certificate obtained successfully from Let's Encrypt");
            data
        }
        Err(e) => {
            eprintln!("Failed to obtain certificate from Let's Encrypt: {}", e);
            return Err(e);
        }
    };

    // Save to S3
    println!("Saving certificate to S3...");
    match s3_storage::save_certificate(
        s3_client,
        bucket,
        domain,
        &cert_data.cert_pem,
        &cert_data.privkey_pem,
    ).await {
        Ok(_) => println!("Certificate saved to S3 successfully"),
        Err(e) => {
            eprintln!("Warning: Failed to save certificate to S3: {}", e);
            eprintln!("Continuing with certificate anyway...");
        }
    }

    // Build TLS config
    build_tls_config(&cert_data.cert_pem, &cert_data.privkey_pem)
}

async fn request_new_certificate(
    domain: &str,
    email: &str,
    staging: bool,
) -> Result<CertificateData, Box<dyn std::error::Error + Send + Sync>> {
    // Choose ACME directory (staging or production)
    let url = if staging {
        LetsEncrypt::Staging.url()
    } else {
        LetsEncrypt::Production.url()
    };

    // Create account
    println!("Creating ACME account...");
    let (account, _credentials) = Account::create(
        &NewAccount {
            contact: &[&format!("mailto:{}", email)],
            terms_of_service_agreed: true,
            only_return_existing: false,
        },
        url,
        None,
    )
    .await?;

    // Create order for domain
    println!("Creating certificate order for {}...", domain);
    let identifier = Identifier::Dns(domain.to_string());
    let mut order = account
        .new_order(&NewOrder {
            identifiers: &[identifier],
        })
    .await?;

    // Get authorizations
    let authorizations = order.authorizations().await?;
    let mut challenges = Vec::new();

    for authz in &authorizations {
        // Find HTTP-01 challenge
        match authz.status {
            AuthorizationStatus::Pending => {}
            AuthorizationStatus::Valid => continue,
            _ => {
                return Err(format!("Unexpected authorization status: {:?}", authz.status).into());
            }
        }

        let challenge = authz
            .challenges
            .iter()
            .find(|c| c.r#type == ChallengeType::Http01)
            .ok_or("No HTTP-01 challenge found")?;

        let Identifier::Dns(domain) = &authz.identifier;
        challenges.push((domain.clone(), challenge));
    }

    // Set up HTTP-01 challenge server
    if !challenges.is_empty() {
        println!("Setting up HTTP-01 challenge server on port 80...");

        let challenge_data: Vec<(String, String)> = challenges
            .iter()
            .map(|(_domain, challenge)| {
                let token = challenge.token.clone();
                let key_auth = order.key_authorization(challenge).as_str().to_string();
                (token, key_auth)
            })
            .collect();

        // Store challenge responses
        let challenge_map = Arc::new(RwLock::new(challenge_data));

        // Spawn challenge server
        let challenge_server = {
            let challenge_map = Arc::clone(&challenge_map);
            tokio::spawn(async move {
                run_challenge_server(challenge_map).await
            })
        };

        // Give server time to start
        tokio::time::sleep(tokio::time::Duration::from_secs(1)).await;

        // Trigger validation
        println!("Triggering ACME validation...");
        for (_, challenge) in &challenges {
            order.set_challenge_ready(&challenge.url).await?;
        }

        // Poll for validation
        println!("Waiting for validation...");
        let mut tries = 0;
        let mut delay = tokio::time::Duration::from_millis(250);

        loop {
            tokio::time::sleep(delay).await;
            let state = order.refresh().await?;

            if let OrderStatus::Ready | OrderStatus::Valid = state.status {
                println!("Validation successful!");
                break;
            } else if let OrderStatus::Invalid = state.status {
                return Err("Order became invalid".into());
            }

            tries += 1;
            if tries > 20 {
                return Err("Validation timeout".into());
            }

            delay = delay.min(tokio::time::Duration::from_secs(5));
        }

        // Stop challenge server
        challenge_server.abort();
    }

    // Generate private key and CSR
    println!("Generating private key...");

    // Create certificate params with the correct domain
    let mut params = rcgen::CertificateParams::new(vec![domain.to_string()]);
    params.distinguished_name = rcgen::DistinguishedName::new();
    params.distinguished_name.push(
        rcgen::DnType::CommonName,
        domain.to_string(),
    );

    // Generate key pair
    let key_pair = rcgen::KeyPair::generate(&rcgen::PKCS_ECDSA_P256_SHA256)?;
    let key_pair_der = key_pair.serialize_der();

    // Set the key pair in params before creating certificate
    params.key_pair = Some(key_pair);

    // Generate certificate with the configured params
    let cert = rcgen::Certificate::from_params(params)?;

    use base64::Engine;
    let private_key_pem = format!(
        "-----BEGIN PRIVATE KEY-----\n{}\n-----END PRIVATE KEY-----\n",
        base64::engine::general_purpose::STANDARD.encode(&key_pair_der)
    );

    // Finalize order with properly configured CSR
    println!("Finalizing certificate order...");
    let csr_der = cert.serialize_request_der()?;
    order.finalize(&csr_der).await?;

    // Download certificate
    println!("Downloading certificate...");
    let cert_chain_pem = loop {
        match order.certificate().await? {
            Some(cert) => break cert,
            None => {
                tokio::time::sleep(tokio::time::Duration::from_secs(1)).await;
            }
        }
    };

    println!("Certificate obtained successfully!");

    Ok(CertificateData {
        cert_pem: cert_chain_pem,
        privkey_pem: private_key_pem,
    })
}

async fn run_challenge_server(
    challenge_map: Arc<RwLock<Vec<(String, String)>>>,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let addr = SocketAddr::from(([0, 0, 0, 0], 80));

    let make_svc = make_service_fn(move |_conn| {
        let challenge_map = Arc::clone(&challenge_map);
        async move {
            Ok::<_, hyper::Error>(service_fn(move |req| {
                let challenge_map = Arc::clone(&challenge_map);
                async move { handle_challenge_request(req, challenge_map).await }
            }))
        }
    });

    let server = Server::bind(&addr).serve(make_svc);

    println!("HTTP-01 challenge server listening on {}", addr);

    server.await?;
    Ok(())
}

async fn handle_challenge_request(
    req: Request<Body>,
    challenge_map: Arc<RwLock<Vec<(String, String)>>>,
) -> Result<Response<Body>, hyper::Error> {
    let path = req.uri().path();

    // Check if this is an ACME challenge request
    if path.starts_with("/.well-known/acme-challenge/") {
        let token = path.trim_start_matches("/.well-known/acme-challenge/");

        let challenges = challenge_map.read().await;
        for (challenge_token, key_auth) in challenges.iter() {
            if challenge_token == token {
                return Ok(Response::builder()
                    .status(StatusCode::OK)
                    .header("content-type", "text/plain")
                    .body(Body::from(key_auth.clone()))
                    .unwrap());
            }
        }
    }

    // Return 404 for all other requests
    Ok(Response::builder()
        .status(StatusCode::NOT_FOUND)
        .body(Body::from("Not Found"))
        .unwrap())
}

pub fn build_tls_config(
    cert_pem: &str,
    privkey_pem: &str,
) -> Result<Arc<ServerConfig>, Box<dyn std::error::Error + Send + Sync>> {
    // Parse certificates
    let mut cert_cursor = Cursor::new(cert_pem.as_bytes());
    let cert_chain: Vec<Certificate> = rustls_pemfile::certs(&mut cert_cursor)?
        .into_iter()
        .map(Certificate)
        .collect();

    if cert_chain.is_empty() {
        return Err("No certificates found in PEM".into());
    }

    // Parse private key
    let mut key_cursor = Cursor::new(privkey_pem.as_bytes());
    let private_key = if let Ok(keys) = rustls_pemfile::pkcs8_private_keys(&mut key_cursor) {
        if !keys.is_empty() {
            PrivateKey(keys[0].clone())
        } else {
            return Err("No private key found in PEM".into());
        }
    } else {
        return Err("Failed to parse private key".into());
    };

    // Build TLS config (rustls 0.19 API)
    use rustls::NoClientAuth;
    let mut config = ServerConfig::new(NoClientAuth::new());
    config.set_single_cert(cert_chain, private_key)?;

    // Enable HTTP/2 and HTTP/1.1
    config.alpn_protocols = vec![b"h2".to_vec(), b"http/1.1".to_vec()];

    Ok(Arc::new(config))
}
