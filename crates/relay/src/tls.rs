// TLS — certificate loading for the relay
// Port of internal/relay/tls.go

use std::fs;
use std::io::BufReader;
use std::sync::Arc;

use anyhow::{Context, Result};
use rustls::pki_types::{CertificateDer, PrivateKeyDer};
use rustls::ServerConfig;

#[derive(Debug, Clone, serde::Deserialize)]
pub struct TlsConfig {
    pub cert_path: String,
    pub key_path: String,
    pub ca_path: Option<String>,
}

/// Build a rustls ServerConfig from PEM cert + key files.
pub fn build_tls_server_config(config: &TlsConfig) -> Result<Arc<ServerConfig>> {
    let certs = load_certs(&config.cert_path)?;
    let key = load_key(&config.key_path)?;

    let server_config = ServerConfig::builder()
        .with_no_client_auth()
        .with_single_cert(certs, key)
        .context("failed to build TLS server config")?;

    Ok(Arc::new(server_config))
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
}
