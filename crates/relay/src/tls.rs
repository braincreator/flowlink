// TLS — certificate loading for the relay
// Port of internal/relay/tls.go

use std::fs;
use std::io::BufReader;
use std::sync::Arc;

use anyhow::{Context, Result};
use rustls::pki_types::{CertificateDer, PrivateKeyDer};
use rustls::server::danger::{ClientCertVerified, ClientCertVerifier};
use rustls::server::WebPkiClientVerifier;
use rustls::client::danger::HandshakeSignatureValid;
use rustls::DistinguishedName;
use rustls::ServerConfig;

#[derive(Debug, Clone, serde::Deserialize)]
pub struct TlsConfig {
    pub cert_path: String,
    pub key_path: String,
    pub ca_path: Option<String>,
}

/// Build a rustls ServerConfig from PEM cert + key files.
///
/// When `ca_path` is provided, builds a verifier that requires clients to
/// present a certificate signed by one of the loaded CAs (mTLS).
/// When `ca_path` is `None`, client authentication is disabled (backward compat).
pub fn build_tls_server_config(config: &TlsConfig) -> Result<Arc<ServerConfig>> {
    let certs = load_certs(&config.cert_path)?;
    let key = load_key(&config.key_path)?;

    let server_config = match &config.ca_path {
        Some(ca_path) => {
            // mTLS: require client certificates signed by our CA
            let ca_certs = load_certs(ca_path)
                .with_context(|| format!("failed to load CA certs from {ca_path}"))?;
            let verifier = Arc::new(MtlsClientVerifier::new(ca_certs)?);
            ServerConfig::builder()
                .with_client_cert_verifier(verifier)
                .with_single_cert(certs, key)
                .context("failed to build TLS server config with mTLS")?
        }
        None => {
            // No client auth (backward compat)
            ServerConfig::builder()
                .with_no_client_auth()
                .with_single_cert(certs, key)
                .context("failed to build TLS server config")?
        }
    };

    Ok(Arc::new(server_config))
}

/// Custom `ClientCertVerifier` that validates client certificates against a
/// set of trusted CA certificates (mTLS).
///
/// This wraps `rustls::server::WebPkiClientVerifier` which does the
/// heavy lifting of building a certificate trust anchor and verifying client
/// certificate chains using the `ring` cryptographic provider.
#[derive(Debug)]
struct MtlsClientVerifier {
    inner: Arc<dyn ClientCertVerifier>,
}

impl MtlsClientVerifier {
    fn new(ca_certs: Vec<CertificateDer<'static>>) -> Result<Self> {
        use rustls::RootCertStore;

        let mut root_store = RootCertStore::empty();
        for cert in ca_certs {
            root_store
                .add(cert)
                .context("failed to add CA certificate to root store")?;
        }

        // WebPkiClientVerifier::builder() requires at least one trust anchor.
        // It builds a ClientCertVerifier that validates client cert chains.
        let verifier = WebPkiClientVerifier::builder(root_store.into())
            .build()
            .context("failed to build WebPkiClientVerifier")?;

        Ok(Self {
            inner: verifier,
        })
    }
}

// Delegate all ClientCertVerifier trait methods to the inner WebPkiClientVerifier.
impl ClientCertVerifier for MtlsClientVerifier {
    fn verify_tls12_signature(
        &self,
        message: &[u8],
        cert: &CertificateDer<'_>,
        dss: &rustls::DigitallySignedStruct,
    ) -> std::result::Result<HandshakeSignatureValid, rustls::Error> {
        self.inner.verify_tls12_signature(message, cert, dss)
    }

    fn verify_tls13_signature(
        &self,
        message: &[u8],
        cert: &CertificateDer<'_>,
        dss: &rustls::DigitallySignedStruct,
    ) -> std::result::Result<HandshakeSignatureValid, rustls::Error> {
        self.inner.verify_tls13_signature(message, cert, dss)
    }

    fn supported_verify_schemes(&self) -> Vec<rustls::SignatureScheme> {
        self.inner.supported_verify_schemes()
    }

    fn verify_client_cert(
        &self,
        end_entity: &CertificateDer<'_>,
        intermediates: &[CertificateDer<'_>],
        now: rustls::pki_types::UnixTime,
    ) -> std::result::Result<ClientCertVerified, rustls::Error> {
        self.inner.verify_client_cert(end_entity, intermediates, now)
    }

    fn root_hint_subjects(&self) -> &[DistinguishedName] {
        self.inner.root_hint_subjects()
    }

    fn offer_client_auth(&self) -> bool {
        self.inner.offer_client_auth()
    }
}

fn load_certs(path: &str) -> Result<Vec<CertificateDer<'static>>> {
    let file = fs::File::open(path).with_context(|| format!("cannot open cert file: {path}"))?;
    let mut reader = BufReader::new(file);
    let certs: Vec<CertificateDer<'static>> = rustls_pemfile::certs(&mut reader)
        .collect::<Result<Vec<_>, _>>()
        .with_context(|| format!("failed to parse certs from {path}"))?;
    if certs.is_empty() {
        anyhow::bail!("no certificates found in {path}");
    }
    Ok(certs)
}

fn load_key(path: &str) -> Result<PrivateKeyDer<'static>> {
    let file = fs::File::open(path).with_context(|| format!("cannot open key file: {path}"))?;
    let mut reader = BufReader::new(file);
    rustls_pemfile::private_key(&mut reader)
        .with_context(|| format!("failed to parse private key from {path}"))?
        .context("no private key found")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_tls_config_deserialize() {
        let json = serde_json::json!({
            "cert_path": "/tmp/cert.pem",
            "key_path": "/tmp/key.pem",
        });
        let config: TlsConfig = serde_json::from_value(json).unwrap();
        assert_eq!(config.cert_path, "/tmp/cert.pem");
        assert!(config.ca_path.is_none());
    }

    #[test]
    fn test_tls_config_with_ca() {
        let json = serde_json::json!({
            "cert_path": "/tmp/cert.pem",
            "key_path": "/tmp/key.pem",
            "ca_path": "/tmp/ca.pem",
        });
        let config: TlsConfig = serde_json::from_value(json).unwrap();
        assert_eq!(config.ca_path.as_deref(), Some("/tmp/ca.pem"));
    }

    #[test]
    fn test_load_certs_missing_file() {
        assert!(load_certs("/nonexistent/path.pem").is_err());
    }

    #[test]
    fn test_load_key_missing_file() {
        assert!(load_key("/nonexistent/path.pem").is_err());
    }

    #[test]
    fn test_build_tls_missing_files() {
        let config = TlsConfig {
            cert_path: "/nonexistent/cert.pem".into(),
            key_path: "/nonexistent/key.pem".into(),
            ca_path: None,
        };
        assert!(build_tls_server_config(&config).is_err());
    }

    #[test]
    fn test_build_tls_with_ca_missing_files() {
        let config = TlsConfig {
            cert_path: "/nonexistent/cert.pem".into(),
            key_path: "/nonexistent/key.pem".into(),
            ca_path: Some("/nonexistent/ca.pem".into()),
        };
        assert!(build_tls_server_config(&config).is_err());
    }
}
