use aws_sdk_s3::config::{Credentials, Region};
use aws_sdk_s3::Client;
use rustls::Certificate;
use rustls_pemfile;
use std::io::{Cursor, Write};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use crate::config::Config;

macro_rules! debug_checkpoint {
    ($msg:expr) => {
        eprintln!("[S3 DEBUG CHECKPOINT] {}", $msg);
        let _ = std::io::stderr().flush();
        let _ = std::io::stdout().flush();
    };
}

pub struct CertificateData {
    pub cert_pem: String,
    pub privkey_pem: String,
}

pub async fn init_s3_client(config: &Config) -> Result<Client, Box<dyn std::error::Error + Send + Sync>> {
    debug_checkpoint!("Entering init_s3_client");
    println!("Creating S3 credentials...");
    eprintln!("[DEBUG] S3 init: Creating credentials");

    debug_checkpoint!("About to create Credentials object");
    let credentials = Credentials::new(
        &config.s3_access_key,
        &config.s3_secret_key,
        None,
        None,
        "static"
    );
    debug_checkpoint!("Credentials object created");
    eprintln!("[DEBUG] S3 init: Credentials created");

    println!("Setting S3 region: {}", config.s3_region);
    debug_checkpoint!("About to create Region object");
    let region = Region::new(config.s3_region.clone());
    debug_checkpoint!("Region object created");
    eprintln!("[DEBUG] S3 init: Region set");

    println!("Building S3 config with endpoint: {}", config.s3_endpoint);
    debug_checkpoint!("About to build S3 config");
    let s3_config = aws_sdk_s3::Config::builder()
        .credentials_provider(credentials)
        .region(region)
        .endpoint_url(&config.s3_endpoint)
        .force_path_style(true)  // Required for Hetzner Object Storage
        .build();
    debug_checkpoint!("S3 config built successfully");
    eprintln!("[DEBUG] S3 init: Config built");

    println!("Creating S3 client...");
    debug_checkpoint!("About to create Client from config");
    let client = Client::from_conf(s3_config);
    debug_checkpoint!("S3 client created successfully");
    println!("✓ S3 client created");

    Ok(client)
}

pub async fn load_certificate(
    client: &Client,
    bucket: &str,
    domain: &str,
) -> Result<Option<CertificateData>, Box<dyn std::error::Error + Send + Sync>> {
    debug_checkpoint!("Loading certificate from S3");
    let cert_key = format!("certs/{}/cert.pem", domain);
    let privkey_key = format!("certs/{}/privkey.pem", domain);

    // Try to load certificate with timeout
    debug_checkpoint!(&format!("Fetching certificate from s3://{}/{}", bucket, cert_key));
    let cert_result = tokio::time::timeout(
        tokio::time::Duration::from_secs(10),
        client
            .get_object()
            .bucket(bucket)
            .key(&cert_key)
            .send()
    ).await;

    let cert_pem = match cert_result {
        Ok(Ok(output)) => {
            debug_checkpoint!("Certificate found in S3");
            let bytes = output.body.collect().await
                .map_err(|e| format!("Failed to read certificate body from S3: {}", e))?
                .into_bytes();
            String::from_utf8(bytes.to_vec())
                .map_err(|e| format!("Certificate is not valid UTF-8: {}", e))?
        }
        Ok(Err(err)) => {
            // If cert doesn't exist, return None
            eprintln!("Certificate not found in S3: {}", err);
            debug_checkpoint!("Certificate not found - will request new one");
            return Ok(None);
        }
        Err(_) => {
            eprintln!("S3 operation timed out while loading certificate");
            let _ = std::io::stderr().flush();
            return Err("S3 timeout while loading certificate - network may be unavailable".into());
        }
    };

    // Try to load private key with timeout
    debug_checkpoint!(&format!("Fetching private key from s3://{}/{}", bucket, privkey_key));
    let privkey_result = tokio::time::timeout(
        tokio::time::Duration::from_secs(10),
        client
            .get_object()
            .bucket(bucket)
            .key(&privkey_key)
            .send()
    ).await;

    let privkey_pem = match privkey_result {
        Ok(Ok(output)) => {
            debug_checkpoint!("Private key found in S3");
            let bytes = output.body.collect().await
                .map_err(|e| format!("Failed to read private key body from S3: {}", e))?
                .into_bytes();
            String::from_utf8(bytes.to_vec())
                .map_err(|e| format!("Private key is not valid UTF-8: {}", e))?
        }
        Ok(Err(err)) => {
            eprintln!("Private key not found in S3: {}", err);
            debug_checkpoint!("Private key not found - will request new certificate");
            return Ok(None);
        }
        Err(_) => {
            eprintln!("S3 operation timed out while loading private key");
            let _ = std::io::stderr().flush();
            return Err("S3 timeout while loading private key - network may be unavailable".into());
        }
    };

    debug_checkpoint!("Certificate and private key loaded successfully from S3");
    Ok(Some(CertificateData {
        cert_pem,
        privkey_pem,
    }))
}

pub async fn save_certificate(
    client: &Client,
    bucket: &str,
    domain: &str,
    cert_pem: &str,
    privkey_pem: &str,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    debug_checkpoint!("Saving certificate to S3");
    let cert_key = format!("certs/{}/cert.pem", domain);
    let privkey_key = format!("certs/{}/privkey.pem", domain);

    // Save certificate with timeout
    debug_checkpoint!(&format!("Uploading certificate to s3://{}/{}", bucket, cert_key));
    let cert_result = tokio::time::timeout(
        tokio::time::Duration::from_secs(10),
        client
            .put_object()
            .bucket(bucket)
            .key(&cert_key)
            .body(cert_pem.as_bytes().to_vec().into())
            .send()
    ).await;

    match cert_result {
        Ok(Ok(_)) => {
            println!("Saved certificate to S3: {}", cert_key);
            debug_checkpoint!("Certificate saved successfully");
        }
        Ok(Err(e)) => {
            eprintln!("Failed to save certificate to S3: {:?}", e);
            let _ = std::io::stderr().flush();
            return Err(format!("S3 error while saving certificate: {}", e).into());
        }
        Err(_) => {
            eprintln!("S3 operation timed out while saving certificate");
            let _ = std::io::stderr().flush();
            return Err("S3 timeout while saving certificate - network may be unavailable".into());
        }
    }

    // Save private key with timeout
    debug_checkpoint!(&format!("Uploading private key to s3://{}/{}", bucket, privkey_key));
    let key_result = tokio::time::timeout(
        tokio::time::Duration::from_secs(10),
        client
            .put_object()
            .bucket(bucket)
            .key(&privkey_key)
            .body(privkey_pem.as_bytes().to_vec().into())
            .send()
    ).await;

    match key_result {
        Ok(Ok(_)) => {
            println!("Saved private key to S3: {}", privkey_key);
            debug_checkpoint!("Private key saved successfully");
        }
        Ok(Err(e)) => {
            eprintln!("Failed to save private key to S3: {:?}", e);
            let _ = std::io::stderr().flush();
            return Err(format!("S3 error while saving private key: {}", e).into());
        }
        Err(_) => {
            eprintln!("S3 operation timed out while saving private key");
            let _ = std::io::stderr().flush();
            return Err("S3 timeout while saving private key - network may be unavailable".into());
        }
    }

    debug_checkpoint!("Certificate and private key saved to S3");
    Ok(())
}

pub fn check_cert_expiry(cert_pem: &str) -> Result<Duration, Box<dyn std::error::Error + Send + Sync>> {
    // Parse the certificate to check expiry
    let mut cursor = Cursor::new(cert_pem.as_bytes());
    let certs = rustls_pemfile::certs(&mut cursor)?;

    if certs.is_empty() {
        return Err("No certificates found in PEM".into());
    }

    // Parse the first certificate
    let cert = Certificate(certs[0].clone());

    // Use x509-parser to extract expiry date
    let parsed = x509_parser::parse_x509_certificate(&cert.0)?;
    let not_after = parsed.1.validity().not_after;

    // Convert ASN1Time to SystemTime
    let expiry_timestamp = not_after.timestamp() as u64;
    let expiry_time = UNIX_EPOCH + Duration::from_secs(expiry_timestamp);

    // Calculate time until expiry
    let now = SystemTime::now();
    match expiry_time.duration_since(now) {
        Ok(duration) => Ok(duration),
        Err(_) => Ok(Duration::from_secs(0)), // Already expired
    }
}

pub fn cert_is_valid(cert_pem: &str, min_days_remaining: u64) -> bool {
    match check_cert_expiry(cert_pem) {
        Ok(duration) => {
            let days_remaining = duration.as_secs() / 86400;
            days_remaining >= min_days_remaining
        }
        Err(e) => {
            eprintln!("Failed to check cert expiry: {}", e);
            false
        }
    }
}
