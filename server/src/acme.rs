use rustls::{Certificate, PrivateKey, ServerConfig};
use rustls_pemfile;
use std::io::Cursor;
use std::sync::Arc;

pub struct CertificateData {
    pub cert_pem: String,
    pub privkey_pem: String,
}

pub fn generate_self_signed_certificate(
    domain: &str,
) -> Result<CertificateData, Box<dyn std::error::Error + Send + Sync>> {
    let mut params = rcgen::CertificateParams::new(vec![domain.to_string()]);
    params.distinguished_name = rcgen::DistinguishedName::new();
    params
        .distinguished_name
        .push(rcgen::DnType::CommonName, domain.to_string());

    let cert = rcgen::Certificate::from_params(params)?;
    let cert_pem = cert.serialize_pem()?;
    let privkey_pem = cert.serialize_private_key_pem();

    Ok(CertificateData {
        cert_pem,
        privkey_pem,
    })
}

pub fn build_tls_config(
    cert_pem: &str,
    privkey_pem: &str,
) -> Result<Arc<ServerConfig>, Box<dyn std::error::Error + Send + Sync>> {
    let mut cert_cursor = Cursor::new(cert_pem.as_bytes());
    let cert_chain: Vec<Certificate> = rustls_pemfile::certs(&mut cert_cursor)?
        .into_iter()
        .map(Certificate)
        .collect();

    if cert_chain.is_empty() {
        return Err("No certificates found in PEM".into());
    }

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

    use rustls::NoClientAuth;
    let mut config = ServerConfig::new(NoClientAuth::new());
    config.set_single_cert(cert_chain, private_key)?;
    config.alpn_protocols = vec![b"h2".to_vec(), b"http/1.1".to_vec()];

    Ok(Arc::new(config))
}
