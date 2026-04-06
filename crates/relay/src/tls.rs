// TLS — certificate loading and TLS acceptor for the relay
// Port of internal/relay/tls.go

use std::fs;
use std::io::BufReader;
use std::path::Path;
use std::sync::Arc;

use anyhow::{Context, Result};
use rustls::pki_types::{CertificateDer, PrivateKeyDer};
use rustls::ServerConfig;

#[derive(Debug, Clone, serde::Deserialize)]
pub struct TlsConfig {
    pub cert_path: String,
    pub key_path: String,
    /// Optional CA for client certificate verification (mTLS)
    pub ca_path: Option<String>,
}

/// Build a rustls ServerConfig from PEM cert + key files.
pub fn build_tls_server_config(config: &TlsConfig) -> Result<Arc<ServerConfig>> {
    let certs = load_certs(&config.cert_path)?;
    let key = load_key(&config.key_path)?;

    let mut builder = ServerConfig::builder()
        .with_no_client_auth()
        .with_single_cert(certs, key)
        .context("failed to build TLS server config")?;

    // mTLS: if CA path provided, verify client certs
    if let Some(ref ca_path) = config.ca_path {
        let ca_certs = load_certs(ca_path)?;
        let mut root_store = rustls::RootCertStore::empty();
        for cert in ca_certs {
            root_store.add(cert).context("failed to add CA cert")?;
        }
        let verifier = rustls::client::Verifier::new(Arc::new(root_store))?;
        // Rebuild with client auth
        builder = ServerConfig::builder()
            .with_client_cert_verifier(verifier)
            .with_single_cert(
                load_certs(&config.cert_path)?,
                load_key(&config.key_path)?,
            )
            .context("failed to build TLS server config with client auth")?;
    }

    Ok(Arc::new(builder))
}

fn load_certs(path: &str) -> Result<Vec<CertificateDer<'static>>> {
    let file = fs::File::open(path).with_context(|| format!("cannot open cert file: {path}"))?;
    let mut reader = BufReader::new(file);
    let certs = rustls_pemfile::certs(&mut reader)
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

    // Try PKCS8 first, then RSA, then EC
    if let Some(key) = rustls_pemfile::pkcs8_private_keys(&mut reader)
        .next()
        .transpose()
        .with_context(|| format!("failed to parse PKCS8 key from {path}"))?
    {
        return Ok(key.into());
    }

    let file = fs::File::open(path).with_context(|| format!("cannot open key file: {path}"))?;
    let mut reader = BufReader::new(file);

    if let Some(key) = rustls_pemfile::rsa_private_keys(&mut reader)
        .next()
        .transpose()
        .with_context(|| format!("failed to parse RSA key from {path}"))?
    {
        return Ok(key.into());
    }

    let file = fs::File::open(path).with_context(|| format!("cannot open key file: {path}"))?;
    let mut reader = BufReader::new(file);

    if let Some(key) = rustls_pemfile::ec_private_keys(&mut reader)
        .next()
        .transpose()
        .with_context(|| format!("failed to parse EC key from {path}"))?
    {
        return Ok(key.into());
    }

    anyhow::bail!("no private key found in {path}")
}
