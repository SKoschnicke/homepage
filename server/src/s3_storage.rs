use rusty_s3::{Bucket, Credentials, S3Action, UrlStyle};
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

/// Lightweight S3 client using rusty-s3 for signing and reqwest for HTTP
pub struct S3Client {
    bucket: Bucket,
    credentials: Credentials,
    http: reqwest::Client,
    bucket_name: String,
}

impl S3Client {
    pub fn name(&self) -> &str {
        &self.bucket_name
    }
}

pub async fn init_s3_bucket(config: &Config) -> Result<S3Client, Box<dyn std::error::Error + Send + Sync>> {
    debug_checkpoint!("Entering init_s3_bucket");
    println!("Creating S3 credentials...");
    eprintln!("[DEBUG] S3 init: Creating credentials");

    debug_checkpoint!("About to create Credentials object");
    let credentials = Credentials::new(
        config.s3_access_key.clone(),
        config.s3_secret_key.clone(),
    );
    debug_checkpoint!("Credentials object created");
    eprintln!("[DEBUG] S3 init: Credentials created");

    println!("Setting S3 region: {}", config.s3_region);
    debug_checkpoint!("About to create Bucket object");

    let endpoint: url::Url = config.s3_endpoint.parse()
        .map_err(|e| format!("Invalid S3 endpoint URL: {}", e))?;

    // Use path-style URLs (required for Hetzner Object Storage and MinIO)
    // Pass owned strings to avoid lifetime issues
    let bucket = Bucket::new(
        endpoint,
        UrlStyle::Path,
        config.s3_bucket.clone(),
        config.s3_region.clone(),
    ).map_err(|e| format!("Failed to create S3 bucket: {}", e))?;

    debug_checkpoint!("Bucket object created");
    eprintln!("[DEBUG] S3 init: Bucket ready");

    // Create HTTP client
    let http = reqwest::Client::builder()
        .timeout(Duration::from_secs(30))
        .build()
        .map_err(|e| format!("Failed to create HTTP client: {}", e))?;

    println!("✓ S3 bucket initialized");

    Ok(S3Client {
        bucket,
        credentials,
        http,
        bucket_name: config.s3_bucket.clone(),
    })
}

pub async fn load_certificate(
    client: &S3Client,
    domain: &str,
) -> Result<Option<CertificateData>, Box<dyn std::error::Error + Send + Sync>> {
    debug_checkpoint!("Loading certificate from S3");
    let cert_key = format!("certs/{}/cert.pem", domain);
    let privkey_key = format!("certs/{}/privkey.pem", domain);

    // Try to load certificate with timeout
    debug_checkpoint!(&format!("Fetching certificate from s3://{}/{}", client.name(), cert_key));

    let cert_url = client.bucket.get_object(Some(&client.credentials), &cert_key)
        .sign(Duration::from_secs(60));

    let cert_result = tokio::time::timeout(
        tokio::time::Duration::from_secs(10),
        client.http.get(cert_url).send()
    ).await;

    let cert_pem = match cert_result {
        Ok(Ok(response)) => {
            if response.status().is_success() {
                debug_checkpoint!("Certificate found in S3");
                response.text().await
                    .map_err(|e| format!("Failed to read certificate body: {}", e))?
            } else if response.status().as_u16() == 404 {
                eprintln!("Certificate not found in S3 (status 404)");
                debug_checkpoint!("Certificate not found - will request new one");
                return Ok(None);
            } else {
                eprintln!("Certificate not found in S3 (status {})", response.status());
                debug_checkpoint!("Certificate not found - will request new one");
                return Ok(None);
            }
        }
        Ok(Err(err)) => {
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
    debug_checkpoint!(&format!("Fetching private key from s3://{}/{}", client.name(), privkey_key));

    let privkey_url = client.bucket.get_object(Some(&client.credentials), &privkey_key)
        .sign(Duration::from_secs(60));

    let privkey_result = tokio::time::timeout(
        tokio::time::Duration::from_secs(10),
        client.http.get(privkey_url).send()
    ).await;

    let privkey_pem = match privkey_result {
        Ok(Ok(response)) => {
            if response.status().is_success() {
                debug_checkpoint!("Private key found in S3");
                response.text().await
                    .map_err(|e| format!("Failed to read private key body: {}", e))?
            } else if response.status().as_u16() == 404 {
                eprintln!("Private key not found in S3 (status 404)");
                debug_checkpoint!("Private key not found - will request new certificate");
                return Ok(None);
            } else {
                eprintln!("Private key not found in S3 (status {})", response.status());
                debug_checkpoint!("Private key not found - will request new certificate");
                return Ok(None);
            }
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
    client: &S3Client,
    domain: &str,
    cert_pem: &str,
    privkey_pem: &str,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    debug_checkpoint!("Saving certificate to S3");
    let cert_key = format!("certs/{}/cert.pem", domain);
    let privkey_key = format!("certs/{}/privkey.pem", domain);

    // Save certificate with timeout
    debug_checkpoint!(&format!("Uploading certificate to s3://{}/{}", client.name(), cert_key));

    let cert_url = client.bucket.put_object(Some(&client.credentials), &cert_key)
        .sign(Duration::from_secs(60));

    let cert_result = tokio::time::timeout(
        Duration::from_secs(10),
        client.http.put(cert_url)
            .body(cert_pem.to_string())
            .send()
    ).await;

    match cert_result {
        Ok(Ok(response)) => {
            if response.status().is_success() {
                println!("Saved certificate to S3: {}", cert_key);
                debug_checkpoint!("Certificate saved successfully");
            } else {
                let _ = std::io::stderr().flush();
                return Err(format!("S3 error saving certificate: status {}", response.status()).into());
            }
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
    debug_checkpoint!(&format!("Uploading private key to s3://{}/{}", client.name(), privkey_key));

    let key_url = client.bucket.put_object(Some(&client.credentials), &privkey_key)
        .sign(Duration::from_secs(60));

    let key_result = tokio::time::timeout(
        Duration::from_secs(10),
        client.http.put(key_url)
            .body(privkey_pem.to_string())
            .send()
    ).await;

    match key_result {
        Ok(Ok(response)) => {
            if response.status().is_success() {
                println!("Saved private key to S3: {}", privkey_key);
                debug_checkpoint!("Private key saved successfully");
            } else {
                let _ = std::io::stderr().flush();
                return Err(format!("S3 error saving private key: status {}", response.status()).into());
            }
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

pub async fn test_s3_storage(
    client: &S3Client,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    debug_checkpoint!("Testing S3 storage connectivity");
    let test_key = ".test/connectivity-check";
    let test_content = "S3 storage connectivity test";

    // Test 1: Write
    debug_checkpoint!("S3 test: Writing test object");

    let write_url = client.bucket.put_object(Some(&client.credentials), test_key)
        .sign(Duration::from_secs(60));

    let write_result = tokio::time::timeout(
        Duration::from_secs(10),
        client.http.put(write_url)
            .body(test_content)
            .send()
    ).await;

    match write_result {
        Ok(Ok(response)) => {
            if response.status().is_success() {
                debug_checkpoint!("S3 test: Write successful");
            } else {
                let _ = std::io::stderr().flush();
                return Err(format!("S3 write test failed: status {}", response.status()).into());
            }
        }
        Ok(Err(e)) => {
            let _ = std::io::stderr().flush();
            return Err(format!("S3 write test failed: {}", e).into());
        }
        Err(_) => {
            let _ = std::io::stderr().flush();
            return Err("S3 write test timed out after 10 seconds".into());
        }
    }

    // Test 2: Read
    debug_checkpoint!("S3 test: Reading test object");

    let read_url = client.bucket.get_object(Some(&client.credentials), test_key)
        .sign(Duration::from_secs(60));

    let read_result = tokio::time::timeout(
        Duration::from_secs(10),
        client.http.get(read_url).send()
    ).await;

    let body = match read_result {
        Ok(Ok(response)) => {
            if response.status().is_success() {
                debug_checkpoint!("S3 test: Read successful");
                response.text().await
                    .map_err(|e| format!("Failed to read test object: {}", e))?
            } else {
                let _ = std::io::stderr().flush();
                return Err(format!("S3 read test failed: status {}", response.status()).into());
            }
        }
        Ok(Err(e)) => {
            let _ = std::io::stderr().flush();
            return Err(format!("S3 read test failed: {}", e).into());
        }
        Err(_) => {
            let _ = std::io::stderr().flush();
            return Err("S3 read test timed out after 10 seconds".into());
        }
    };

    if body != test_content {
        let _ = std::io::stderr().flush();
        return Err("S3 read-back verification failed: content mismatch".into());
    }
    debug_checkpoint!("S3 test: Content verification successful");

    // Test 3: Delete
    debug_checkpoint!("S3 test: Deleting test object");

    let delete_url = client.bucket.delete_object(Some(&client.credentials), test_key)
        .sign(Duration::from_secs(60));

    let delete_result = tokio::time::timeout(
        Duration::from_secs(10),
        client.http.delete(delete_url).send()
    ).await;

    match delete_result {
        Ok(Ok(response)) => {
            // S3 returns 204 for successful deletes
            if response.status().is_success() {
                debug_checkpoint!("S3 test: Delete successful");
            } else {
                let _ = std::io::stderr().flush();
                return Err(format!("S3 delete test failed: status {}", response.status()).into());
            }
        }
        Ok(Err(e)) => {
            let _ = std::io::stderr().flush();
            return Err(format!("S3 delete test failed: {}", e).into());
        }
        Err(_) => {
            let _ = std::io::stderr().flush();
            return Err("S3 delete test timed out after 10 seconds".into());
        }
    }

    debug_checkpoint!("S3 storage test completed successfully");
    Ok(())
}
