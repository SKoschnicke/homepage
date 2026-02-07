use s3::creds::Credentials;
use s3::region::Region;
use s3::Bucket;
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

pub async fn init_s3_bucket(config: &Config) -> Result<Bucket, Box<dyn std::error::Error + Send + Sync>> {
    debug_checkpoint!("Entering init_s3_bucket");
    println!("Creating S3 credentials...");
    eprintln!("[DEBUG] S3 init: Creating credentials");

    debug_checkpoint!("About to create Credentials object");
    let credentials = Credentials::new(
        Some(&config.s3_access_key),
        Some(&config.s3_secret_key),
        None,
        None,
        None,
    )?;
    debug_checkpoint!("Credentials object created");
    eprintln!("[DEBUG] S3 init: Credentials created");

    println!("Setting S3 region: {}", config.s3_region);
    debug_checkpoint!("About to create Region object");
    let region = Region::Custom {
        region: config.s3_region.clone(),
        endpoint: config.s3_endpoint.clone(),
    };
    debug_checkpoint!("Region object created");
    eprintln!("[DEBUG] S3 init: Region set");

    println!("Building S3 bucket with endpoint: {}", config.s3_endpoint);
    debug_checkpoint!("About to build S3 bucket");
    let bucket = Bucket::new(
        &config.s3_bucket,
        region,
        credentials,
    )?.with_path_style();  // Required for Hetzner Object Storage
    debug_checkpoint!("S3 bucket created successfully");
    eprintln!("[DEBUG] S3 init: Bucket ready");

    println!("✓ S3 bucket initialized");

    Ok(*bucket)
}

pub async fn load_certificate(
    bucket: &Bucket,
    domain: &str,
) -> Result<Option<CertificateData>, Box<dyn std::error::Error + Send + Sync>> {
    debug_checkpoint!("Loading certificate from S3");
    let cert_key = format!("certs/{}/cert.pem", domain);
    let privkey_key = format!("certs/{}/privkey.pem", domain);

    // Try to load certificate with timeout
    debug_checkpoint!(&format!("Fetching certificate from s3://{}/{}", bucket.name(), cert_key));
    let cert_result = tokio::time::timeout(
        tokio::time::Duration::from_secs(10),
        bucket.get_object(&cert_key)
    ).await;

    let cert_pem = match cert_result {
        Ok(Ok(response)) => {
            if response.status_code() == 200 {
                debug_checkpoint!("Certificate found in S3");
                String::from_utf8(response.to_vec())
                    .map_err(|e| format!("Certificate is not valid UTF-8: {}", e))?
            } else {
                eprintln!("Certificate not found in S3 (status {})", response.status_code());
                debug_checkpoint!("Certificate not found - will request new one");
                return Ok(None);
            }
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
    debug_checkpoint!(&format!("Fetching private key from s3://{}/{}", bucket.name(), privkey_key));
    let privkey_result = tokio::time::timeout(
        tokio::time::Duration::from_secs(10),
        bucket.get_object(&privkey_key)
    ).await;

    let privkey_pem = match privkey_result {
        Ok(Ok(response)) => {
            if response.status_code() == 200 {
                debug_checkpoint!("Private key found in S3");
                String::from_utf8(response.to_vec())
                    .map_err(|e| format!("Private key is not valid UTF-8: {}", e))?
            } else {
                eprintln!("Private key not found in S3 (status {})", response.status_code());
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
    bucket: &Bucket,
    domain: &str,
    cert_pem: &str,
    privkey_pem: &str,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    debug_checkpoint!("Saving certificate to S3");
    let cert_key = format!("certs/{}/cert.pem", domain);
    let privkey_key = format!("certs/{}/privkey.pem", domain);

    // Save certificate with timeout
    debug_checkpoint!(&format!("Uploading certificate to s3://{}/{}", bucket.name(), cert_key));
    let cert_result = tokio::time::timeout(
        tokio::time::Duration::from_secs(10),
        bucket.put_object(&cert_key, cert_pem.as_bytes())
    ).await;

    match cert_result {
        Ok(Ok(response)) => {
            if response.status_code() >= 200 && response.status_code() < 300 {
                println!("Saved certificate to S3: {}", cert_key);
                debug_checkpoint!("Certificate saved successfully");
            } else {
                let _ = std::io::stderr().flush();
                return Err(format!("S3 error saving certificate: status {}", response.status_code()).into());
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
    debug_checkpoint!(&format!("Uploading private key to s3://{}/{}", bucket.name(), privkey_key));
    let key_result = tokio::time::timeout(
        tokio::time::Duration::from_secs(10),
        bucket.put_object(&privkey_key, privkey_pem.as_bytes())
    ).await;

    match key_result {
        Ok(Ok(response)) => {
            if response.status_code() >= 200 && response.status_code() < 300 {
                println!("Saved private key to S3: {}", privkey_key);
                debug_checkpoint!("Private key saved successfully");
            } else {
                let _ = std::io::stderr().flush();
                return Err(format!("S3 error saving private key: status {}", response.status_code()).into());
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
    bucket: &Bucket,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    debug_checkpoint!("Testing S3 storage connectivity");
    let test_key = ".test/connectivity-check";
    let test_content = b"S3 storage connectivity test";

    // Test 1: Write
    debug_checkpoint!("S3 test: Writing test object");
    let write_result = tokio::time::timeout(
        Duration::from_secs(10),
        bucket.put_object(test_key, test_content)
    ).await;

    match write_result {
        Ok(Ok(response)) => {
            if response.status_code() >= 200 && response.status_code() < 300 {
                debug_checkpoint!("S3 test: Write successful");
            } else {
                let _ = std::io::stderr().flush();
                return Err(format!("S3 write test failed: status {}", response.status_code()).into());
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
    let read_result = tokio::time::timeout(
        Duration::from_secs(10),
        bucket.get_object(test_key)
    ).await;

    let body = match read_result {
        Ok(Ok(response)) => {
            if response.status_code() == 200 {
                debug_checkpoint!("S3 test: Read successful");
                response.to_vec()
            } else {
                let _ = std::io::stderr().flush();
                return Err(format!("S3 read test failed: status {}", response.status_code()).into());
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

    if &body[..] != test_content {
        let _ = std::io::stderr().flush();
        return Err("S3 read-back verification failed: content mismatch".into());
    }
    debug_checkpoint!("S3 test: Content verification successful");

    // Test 3: Delete
    debug_checkpoint!("S3 test: Deleting test object");
    let delete_result = tokio::time::timeout(
        Duration::from_secs(10),
        bucket.delete_object(test_key)
    ).await;

    match delete_result {
        Ok(Ok(response)) => {
            // S3 returns 204 for successful deletes
            if response.status_code() >= 200 && response.status_code() < 300 {
                debug_checkpoint!("S3 test: Delete successful");
            } else {
                let _ = std::io::stderr().flush();
                return Err(format!("S3 delete test failed: status {}", response.status_code()).into());
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
