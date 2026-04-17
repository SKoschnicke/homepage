use rustls::pki_types::{CertificateDer, PrivateKeyDer, PrivatePkcs8KeyDer};
use rustls::ServerConfig;
use std::fs;
use std::io::Cursor;
use std::path::Path;
use std::sync::Arc;

pub struct CertificateData {
    pub cert_pem: String,
    pub privkey_pem: String,
}

pub fn generate_self_signed_certificate(
    domain: &str,
) -> Result<CertificateData, Box<dyn std::error::Error + Send + Sync>> {
    let mut params = rcgen::CertificateParams::new(vec![domain.to_string()])?;
    params.distinguished_name = rcgen::DistinguishedName::new();
    params
        .distinguished_name
        .push(rcgen::DnType::CommonName, domain.to_string());

    let key_pair = rcgen::KeyPair::generate()?;
    let cert = params.self_signed(&key_pair)?;

    Ok(CertificateData {
        cert_pem: cert.pem(),
        privkey_pem: key_pair.serialize_pem(),
    })
}

/// Load a cert+key PEM pair from `dir`, generating and persisting a fresh one
/// if either file is missing. Gemini clients pin on TOFU, so the on-disk pair
/// must survive restarts.
pub fn load_or_generate_persistent_certificate(
    dir: &Path,
    domain: &str,
) -> Result<CertificateData, Box<dyn std::error::Error + Send + Sync>> {
    let cert_path = dir.join("gemini.crt");
    let key_path = dir.join("gemini.key");

    if cert_path.exists() && key_path.exists() {
        let cert_pem = fs::read_to_string(&cert_path)?;
        let privkey_pem = fs::read_to_string(&key_path)?;
        return Ok(CertificateData { cert_pem, privkey_pem });
    }

    fs::create_dir_all(dir)?;
    let data = generate_self_signed_certificate(domain)?;

    // Write key with 0600 before cert so a partial failure leaves no key on disk
    // readable by group/other.
    write_private(&key_path, &data.privkey_pem)?;
    fs::write(&cert_path, &data.cert_pem)?;

    Ok(data)
}

#[cfg(unix)]
fn write_private(path: &Path, contents: &str) -> std::io::Result<()> {
    use std::os::unix::fs::OpenOptionsExt;
    use std::io::Write;
    let mut f = fs::OpenOptions::new()
        .write(true)
        .create(true)
        .truncate(true)
        .mode(0o600)
        .open(path)?;
    f.write_all(contents.as_bytes())
}

#[cfg(not(unix))]
fn write_private(path: &Path, contents: &str) -> std::io::Result<()> {
    fs::write(path, contents)
}

pub fn build_tls_config(
    cert_pem: &str,
    privkey_pem: &str,
) -> Result<Arc<ServerConfig>, Box<dyn std::error::Error + Send + Sync>> {
    let mut cert_cursor = Cursor::new(cert_pem.as_bytes());
    let cert_chain: Vec<CertificateDer<'static>> = rustls_pemfile::certs(&mut cert_cursor)
        .collect::<Result<Vec<_>, _>>()?;

    if cert_chain.is_empty() {
        return Err("No certificates found in PEM".into());
    }

    let mut key_cursor = Cursor::new(privkey_pem.as_bytes());
    let mut pkcs8_keys: Vec<PrivatePkcs8KeyDer<'static>> =
        rustls_pemfile::pkcs8_private_keys(&mut key_cursor)
            .collect::<Result<Vec<_>, _>>()?;
    if pkcs8_keys.is_empty() {
        return Err("No private key found in PEM".into());
    }
    let private_key = PrivateKeyDer::Pkcs8(pkcs8_keys.remove(0));

    let mut config = ServerConfig::builder()
        .with_no_client_auth()
        .with_single_cert(cert_chain, private_key)?;
    config.alpn_protocols = vec![b"h2".to_vec(), b"http/1.1".to_vec()];

    Ok(Arc::new(config))
}
