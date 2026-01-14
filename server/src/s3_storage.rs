use aws_sdk_s3::config::{Credentials, Region};
use aws_sdk_s3::Client;
use rustls::Certificate;
use rustls_pemfile;
use std::io::Cursor;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use crate::config::Config;

pub struct CertificateData {
    pub cert_pem: String,
    pub privkey_pem: String,
}

pub async fn init_s3_client(config: &Config) -> Result<Client, Box<dyn std::error::Error>> {
    println!("Creating S3 credentials...");
    let credentials = Credentials::new(
        &config.s3_access_key,
        &config.s3_secret_key,
        None,
        None,
        "static"
    );

    println!("Setting S3 region: {}", config.s3_region);
    let region = Region::new(config.s3_region.clone());

    println!("Building S3 config with endpoint: {}", config.s3_endpoint);
    let s3_config = aws_sdk_s3::Config::builder()
        .credentials_provider(credentials)
        .region(region)
        .endpoint_url(&config.s3_endpoint)
        .build();

    println!("Creating S3 client...");
    let client = Client::from_conf(s3_config);

    // Test connectivity with a simple list_buckets call
    println!("Testing S3 connectivity...");
    match client.list_buckets().send().await {
        Ok(_) => {
            println!("S3 connectivity test passed");
            Ok(client)
        }
        Err(e) => {
            eprintln!("S3 connectivity test failed: {:?}", e);
            Err(format!("Failed to connect to S3: {}", e).into())
        }
    }
}

pub async fn load_certificate(
    client: &Client,
    bucket: &str,
    domain: &str,
) -> Result<Option<CertificateData>, Box<dyn std::error::Error>> {
    let cert_key = format!("certs/{}/cert.pem", domain);
    let privkey_key = format!("certs/{}/privkey.pem", domain);

    // Try to load certificate
    let cert_result = client
        .get_object()
        .bucket(bucket)
        .key(&cert_key)
        .send()
        .await;

    let cert_pem = match cert_result {
        Ok(output) => {
            let bytes = output.body.collect().await?.into_bytes();
            String::from_utf8(bytes.to_vec())?
        }
        Err(err) => {
            // If cert doesn't exist, return None
            eprintln!("Certificate not found in S3: {}", err);
            return Ok(None);
        }
    };

    // Try to load private key
    let privkey_result = client
        .get_object()
        .bucket(bucket)
        .key(&privkey_key)
        .send()
        .await;

    let privkey_pem = match privkey_result {
        Ok(output) => {
            let bytes = output.body.collect().await?.into_bytes();
            String::from_utf8(bytes.to_vec())?
        }
        Err(err) => {
            eprintln!("Private key not found in S3: {}", err);
            return Ok(None);
        }
    };

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
) -> Result<(), Box<dyn std::error::Error>> {
    let cert_key = format!("certs/{}/cert.pem", domain);
    let privkey_key = format!("certs/{}/privkey.pem", domain);

    // Save certificate
    client
        .put_object()
        .bucket(bucket)
        .key(&cert_key)
        .body(cert_pem.as_bytes().to_vec().into())
        .send()
        .await?;

    println!("Saved certificate to S3: {}", cert_key);

    // Save private key
    client
        .put_object()
        .bucket(bucket)
        .key(&privkey_key)
        .body(privkey_pem.as_bytes().to_vec().into())
        .send()
        .await?;

    println!("Saved private key to S3: {}", privkey_key);

    Ok(())
}

pub fn check_cert_expiry(cert_pem: &str) -> Result<Duration, Box<dyn std::error::Error>> {
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
