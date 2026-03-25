use std::env;

#[derive(Clone)]
pub struct Config {
    pub domain: String,
    pub local_dev: bool,
    pub acme_contact: String,
    pub acme_staging: bool,
    /// Directory for filesystem certificate storage (when not using S3)
    pub cert_dir: Option<String>,
    pub s3_endpoint: Option<String>,
    pub s3_bucket: Option<String>,
    pub s3_access_key: Option<String>,
    pub s3_secret_key: Option<String>,
    pub s3_region: String,
}

impl Config {
    pub fn has_s3(&self) -> bool {
        self.s3_endpoint.is_some()
            && self.s3_bucket.is_some()
            && self.s3_access_key.is_some()
            && self.s3_secret_key.is_some()
    }

    pub fn load_from_env() -> Result<Self, String> {
        let domain = env::var("DOMAIN")
            .map_err(|_| "DOMAIN environment variable required for HTTPS")?;

        let local_dev = env::var("LOCAL_DEV")
            .map(|v| v == "true" || v == "1")
            .unwrap_or(false);

        let acme_contact = env::var("ACME_CONTACT_EMAIL")
            .map_err(|_| "ACME_CONTACT_EMAIL environment variable required for Let's Encrypt")?;

        let acme_staging = env::var("ACME_STAGING")
            .map(|v| v == "true" || v == "1")
            .unwrap_or(false);

        let cert_dir = env::var("CERT_DIR").ok();

        // S3 configuration - check if any S3 vars are set
        let s3_endpoint = env::var("S3_ENDPOINT").ok();
        let s3_bucket = env::var("S3_BUCKET").ok();
        let s3_access_key = env::var("S3_ACCESS_KEY").ok();
        let s3_secret_key = env::var("S3_SECRET_KEY").ok();
        let s3_region = env::var("S3_REGION")
            .unwrap_or_else(|_| "us-east-1".to_string());

        // In local dev mode, default S3 to MinIO if no cert_dir and no explicit S3
        let (s3_endpoint, s3_bucket, s3_access_key, s3_secret_key) = if local_dev
            && cert_dir.is_none()
            && s3_endpoint.is_none()
        {
            (
                Some("http://localhost:9000".to_string()),
                Some("local-certs".to_string()),
                Some("minioadmin".to_string()),
                Some("minioadmin".to_string()),
            )
        } else {
            (s3_endpoint, s3_bucket, s3_access_key, s3_secret_key)
        };

        // Validate: need either cert_dir or S3 for HTTPS (unless we'll fall back to HTTP)
        let has_s3 = s3_endpoint.is_some()
            && s3_bucket.is_some()
            && s3_access_key.is_some()
            && s3_secret_key.is_some();

        if !local_dev && cert_dir.is_none() && !has_s3 {
            return Err(
                "Certificate storage required: set CERT_DIR for filesystem storage, \
                 or S3_ENDPOINT/S3_BUCKET/S3_ACCESS_KEY/S3_SECRET_KEY for S3 storage"
                    .to_string(),
            );
        }

        // Validate S3 endpoint format if provided
        if let Some(ref endpoint) = s3_endpoint {
            if !endpoint.starts_with("https://") && !endpoint.starts_with("http://") {
                return Err("S3_ENDPOINT must start with http:// or https://".to_string());
            }
        }

        // Validate domain format
        if domain.is_empty() || domain.contains(' ') {
            return Err("Invalid DOMAIN format".to_string());
        }

        // Validate email format in production
        if !local_dev && !acme_contact.contains('@') {
            return Err("Invalid ACME_CONTACT_EMAIL format".to_string());
        }

        Ok(Config {
            domain,
            local_dev,
            acme_contact,
            acme_staging,
            cert_dir,
            s3_endpoint,
            s3_bucket,
            s3_access_key,
            s3_secret_key,
            s3_region,
        })
    }
}
