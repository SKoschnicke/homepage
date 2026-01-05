use std::env;

#[derive(Clone)]
pub struct Config {
    pub domain: String,
    pub local_dev: bool,
    pub acme_contact: String,
    pub acme_staging: bool,
    pub s3_endpoint: String,
    pub s3_bucket: String,
    pub s3_access_key: String,
    pub s3_secret_key: String,
    pub s3_region: String,
}

impl Config {
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

        // S3 configuration is optional in local dev mode
        let s3_endpoint = if local_dev {
            env::var("S3_ENDPOINT").unwrap_or_else(|_| "http://localhost:9000".to_string())
        } else {
            env::var("S3_ENDPOINT")
                .map_err(|_| "S3_ENDPOINT environment variable required for certificate storage")?
        };

        let s3_bucket = if local_dev {
            env::var("S3_BUCKET").unwrap_or_else(|_| "local-certs".to_string())
        } else {
            env::var("S3_BUCKET")
                .map_err(|_| "S3_BUCKET environment variable required for certificate storage")?
        };

        let s3_access_key = if local_dev {
            env::var("S3_ACCESS_KEY").unwrap_or_else(|_| "minioadmin".to_string())
        } else {
            env::var("S3_ACCESS_KEY")
                .map_err(|_| "S3_ACCESS_KEY environment variable required for certificate storage")?
        };

        let s3_secret_key = if local_dev {
            env::var("S3_SECRET_KEY").unwrap_or_else(|_| "minioadmin".to_string())
        } else {
            env::var("S3_SECRET_KEY")
                .map_err(|_| "S3_SECRET_KEY environment variable required for certificate storage")?
        };

        let s3_region = env::var("S3_REGION")
            .unwrap_or_else(|_| "us-east-1".to_string());

        // Validate domain format (basic check)
        if domain.is_empty() || domain.contains(' ') {
            return Err("Invalid DOMAIN format".to_string());
        }

        // Validate email format (basic check)
        if !acme_contact.contains('@') || !acme_contact.contains('.') {
            return Err("Invalid ACME_CONTACT_EMAIL format".to_string());
        }

        // Validate S3 endpoint format
        if !s3_endpoint.starts_with("https://") && !s3_endpoint.starts_with("http://") {
            return Err("S3_ENDPOINT must start with http:// or https://".to_string());
        }

        Ok(Config {
            domain,
            local_dev,
            acme_contact,
            acme_staging,
            s3_endpoint,
            s3_bucket,
            s3_access_key,
            s3_secret_key,
            s3_region,
        })
    }
}
