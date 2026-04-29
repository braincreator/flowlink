// TLS support for agent↔relay connections
// Port of internal/agent/tls.go

use std::fs;
use std::sync::Arc;

use rustls::pki_types::{CertificateDer, PrivateKeyDer};

/// TLS configuration for mTLS agent↔relay connections.
pub struct TlsConfig {
    pub cert_path: String,
    pub key_path: String,
    pub ca_path: String,
}

impl TlsConfig {
    /// Build a tungstenite connector ready to use.
    pub fn build_connector(&self) -> anyhow::Result<tokio_tungstenite::tungstenite::Connector> {
        let config = rustls::ClientConfig::builder()
            .with_root_certificates(self.build_root_store()?)
            .with_client_auth_cert(self.load_certs()?, self.load_key()?)?;
        Ok(tokio_tungstenite::tungstenite::Connector::Rustls(Arc::new(config)))
    }

    fn load_certs(&self) -> anyhow::Result<Vec<CertificateDer<'static>>> {
        let cert = fs::read(&self.cert_path)?;
        Ok(rustls_pemfile::certs(&mut &*cert).collect::<Result<Vec<_>, _>>()?)
    }

    fn load_key(&self) -> anyhow::Result<PrivateKeyDer<'static>> {
        let key = fs::read(&self.key_path)?;
        let key_der = rustls_pemfile::pkcs8_private_keys(&mut &*key)
            .next()
            .ok_or_else(|| anyhow::anyhow!("no private key found"))??;
        Ok(PrivateKeyDer::Pkcs8(key_der))
    }

    fn build_root_store(&self) -> anyhow::Result<rustls::RootCertStore> {
        let ca = fs::read(&self.ca_path)?;
        let ca_certs: Vec<CertificateDer<'static>> =
            rustls_pemfile::certs(&mut &*ca).collect::<Result<Vec<_>, _>>()?;
        let mut root_store = rustls::RootCertStore::empty();
        for ca_cert in ca_certs {
            root_store.add(ca_cert)?;
        }
        Ok(root_store)
    }
}

/// Create an insecure TLS connector (dev mode). NOT FOR PRODUCTION.
pub fn insecure_tls_connector() -> anyhow::Result<tokio_tungstenite::tungstenite::Connector> {
    let config = rustls::ClientConfig::builder()
        .dangerous()
        .with_custom_certificate_verifier(Arc::new(InsecureVerifier))
        .with_no_client_auth();
    Ok(tokio_tungstenite::tungstenite::Connector::Rustls(Arc::new(config)))
}

/// A verifier that accepts any certificate. For dev mode only.
#[derive(Debug)]
struct InsecureVerifier;

impl rustls::client::danger::ServerCertVerifier for InsecureVerifier {
    fn verify_server_cert(
        &self,
        _end_entity: &CertificateDer<'_>,
        _intermediates: &[CertificateDer<'_>],
        _server_name: &rustls::pki_types::ServerName<'_>,
        _ocsp_response: &[u8],
        _now: rustls::pki_types::UnixTime,
    ) -> Result<rustls::client::danger::ServerCertVerified, rustls::Error> {
        Ok(rustls::client::danger::ServerCertVerified::assertion())
    }

    fn verify_tls12_signature(
        &self,
        _message: &[u8],
        _cert: &CertificateDer<'_>,
        _dss: &rustls::DigitallySignedStruct,
    ) -> Result<rustls::client::danger::HandshakeSignatureValid, rustls::Error> {
        Ok(rustls::client::danger::HandshakeSignatureValid::assertion())
    }

    fn verify_tls13_signature(
        &self,
        _message: &[u8],
        _cert: &CertificateDer<'_>,
        _dss: &rustls::DigitallySignedStruct,
    ) -> Result<rustls::client::danger::HandshakeSignatureValid, rustls::Error> {
        Ok(rustls::client::danger::HandshakeSignatureValid::assertion())
    }

    fn supported_verify_schemes(&self) -> Vec<rustls::SignatureScheme> {
        vec![
            rustls::SignatureScheme::RSA_PKCS1_SHA256,
            rustls::SignatureScheme::RSA_PKCS1_SHA384,
            rustls::SignatureScheme::RSA_PKCS1_SHA512,
            rustls::SignatureScheme::ECDSA_NISTP256_SHA256,
            rustls::SignatureScheme::ECDSA_NISTP384_SHA384,
            rustls::SignatureScheme::ECDSA_NISTP521_SHA512,
            rustls::SignatureScheme::RSA_PSS_SHA256,
            rustls::SignatureScheme::RSA_PSS_SHA384,
            rustls::SignatureScheme::RSA_PSS_SHA512,
            rustls::SignatureScheme::ED25519,
        ]
    }
}

/// Parse a SHA256 fingerprint string into normalized "sha256:hex" form.
pub fn parse_fingerprint(fp: &str) -> anyhow::Result<String> {
    let fp = fp.trim();
    let hex_part = if fp.to_lowercase().starts_with("sha256:") {
        &fp[7..]
    } else {
        fp
    };
    for c in hex_part.chars() {
        if !c.is_ascii_hexdigit() {
            anyhow::bail!("invalid hex in fingerprint");
        }
    }
    Ok(format!("sha256:{}", hex_part.to_lowercase()))
}

/// Compute SHA256 fingerprint of a certificate's DER bytes.
pub fn cert_fingerprint(der_bytes: &[u8]) -> String {
    use sha2::{Digest, Sha256};
    let hash = Sha256::digest(der_bytes);
    format!("sha256:{}", hex_encode(&hash))
}

fn hex_encode(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{:02x}", b)).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_fingerprint_with_prefix() {
        let result = parse_fingerprint(
            "SHA256:ABCDEF0123456789ABCDEF0123456789ABCDEF0123456789ABCDEF0123456789",
        )
        .unwrap();
        assert_eq!(
            result,
            "sha256:abcdef0123456789abcdef0123456789abcdef0123456789abcdef0123456789"
        );
    }

    #[test]
    fn test_parse_fingerprint_without_prefix() {
        let result =
            parse_fingerprint("ABCDEF0123456789ABCDEF0123456789ABCDEF0123456789ABCDEF0123456789")
                .unwrap();
        assert_eq!(
            result,
            "sha256:abcdef0123456789abcdef0123456789abcdef0123456789abcdef0123456789"
        );
    }

    #[test]
    fn test_parse_fingerprint_lowercase() {
        let result =
            parse_fingerprint("abcdef0123456789abcdef0123456789abcdef0123456789abcdef0123456789")
                .unwrap();
        assert_eq!(
            result,
            "sha256:abcdef0123456789abcdef0123456789abcdef0123456789abcdef0123456789"
        );
    }

    #[test]
    fn test_parse_fingerprint_invalid_chars() {
        let result = parse_fingerprint("sha256:ZZZZ");
        assert!(result.is_err());
    }

    #[test]
    fn test_parse_fingerprint_trimming() {
        let result = parse_fingerprint("  sha256:ABCDEF  ").unwrap();
        assert_eq!(result, "sha256:abcdef");
    }

    #[test]
    fn test_cert_fingerprint() {
        let data = b"some certificate data";
        let fp = cert_fingerprint(data);
        assert!(fp.starts_with("sha256:"));
        assert_eq!(fp.len(), 7 + 64); // "sha256:" + 64 hex chars
    }

    #[test]
    fn test_cert_fingerprint_deterministic() {
        let data = b"test cert";
        assert_eq!(cert_fingerprint(data), cert_fingerprint(data));
    }

    #[test]
    fn test_cert_fingerprint_different_data() {
        assert_ne!(cert_fingerprint(b"a"), cert_fingerprint(b"b"));
    }

    #[test]
    fn test_insecure_tls_connector() {
        let connector = insecure_tls_connector();
        assert!(connector.is_ok());
    }
}
