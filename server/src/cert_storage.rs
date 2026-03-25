use std::path::PathBuf;

use crate::s3_storage::CertificateData;

type BoxError = Box<dyn std::error::Error + Send + Sync>;

/// Trait for certificate persistence backends
#[async_trait::async_trait]
pub trait CertStorage: Send + Sync {
    fn name(&self) -> &str;

    async fn load_certificate(
        &self,
        domain: &str,
    ) -> Result<Option<CertificateData>, BoxError>;

    async fn save_certificate(
        &self,
        domain: &str,
        cert_pem: &str,
        privkey_pem: &str,
    ) -> Result<(), BoxError>;

    async fn test_storage(&self) -> Result<(), BoxError>;
}

/// Filesystem-based certificate storage
pub struct FilesystemStorage {
    base_dir: PathBuf,
    display_name: String,
}

impl FilesystemStorage {
    pub fn new(base_dir: impl Into<PathBuf>) -> Self {
        let base_dir = base_dir.into();
        let display_name = base_dir.to_string_lossy().into_owned();
        Self { base_dir, display_name }
    }

    fn cert_path(&self, domain: &str) -> PathBuf {
        self.base_dir.join("certs").join(domain).join("cert.pem")
    }

    fn key_path(&self, domain: &str) -> PathBuf {
        self.base_dir.join("certs").join(domain).join("privkey.pem")
    }
}

#[async_trait::async_trait]
impl CertStorage for FilesystemStorage {
    fn name(&self) -> &str {
        &self.display_name
    }

    async fn load_certificate(
        &self,
        domain: &str,
    ) -> Result<Option<CertificateData>, BoxError> {
        let cert_path = self.cert_path(domain);
        let key_path = self.key_path(domain);

        if !cert_path.exists() || !key_path.exists() {
            println!("No certificate found at {}", cert_path.display());
            return Ok(None);
        }

        let cert_pem = tokio::fs::read_to_string(&cert_path).await
            .map_err(|e| -> BoxError { format!("Failed to read {}: {}", cert_path.display(), e).into() })?;
        let privkey_pem = tokio::fs::read_to_string(&key_path).await
            .map_err(|e| -> BoxError { format!("Failed to read {}: {}", key_path.display(), e).into() })?;

        println!("Loaded certificate from {}", cert_path.display());
        Ok(Some(CertificateData { cert_pem, privkey_pem }))
    }

    async fn save_certificate(
        &self,
        domain: &str,
        cert_pem: &str,
        privkey_pem: &str,
    ) -> Result<(), BoxError> {
        let cert_path = self.cert_path(domain);
        let key_path = self.key_path(domain);

        // Create directory structure
        if let Some(parent) = cert_path.parent() {
            tokio::fs::create_dir_all(parent).await
                .map_err(|e| -> BoxError { format!("Failed to create {}: {}", parent.display(), e).into() })?;
        }

        tokio::fs::write(&cert_path, cert_pem).await
            .map_err(|e| -> BoxError { format!("Failed to write {}: {}", cert_path.display(), e).into() })?;

        tokio::fs::write(&key_path, privkey_pem).await
            .map_err(|e| -> BoxError { format!("Failed to write {}: {}", key_path.display(), e).into() })?;

        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let perms = std::fs::Permissions::from_mode(0o600);
            std::fs::set_permissions(&key_path, perms)
                .map_err(|e| -> BoxError { format!("Failed to set permissions on {}: {}", key_path.display(), e).into() })?;
        }

        println!("Saved certificate to {}", cert_path.display());
        println!("Saved private key to {}", key_path.display());
        Ok(())
    }

    async fn test_storage(&self) -> Result<(), BoxError> {
        tokio::fs::create_dir_all(&self.base_dir).await
            .map_err(|e| -> BoxError { format!("Cannot create cert directory {}: {}", self.base_dir.display(), e).into() })?;

        let test_path = self.base_dir.join(".test");
        tokio::fs::write(&test_path, "test").await
            .map_err(|e| -> BoxError { format!("Cannot write to {}: {}", self.base_dir.display(), e).into() })?;
        tokio::fs::remove_file(&test_path).await
            .map_err(|e| -> BoxError { format!("Cannot delete test file: {}", e).into() })?;

        Ok(())
    }
}
