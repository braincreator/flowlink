// TLS support for agent↔relay connections
// Port of internal/agent/tls.go

use std::fs;

/// TLS configuration for mTLS agent↔relay connections.
pub struct TlsConfig {
    pub cert_path: String,
    pub key_path: String,
    pub ca_path: String,
}

impl TlsConfig {
    /// Build a native-tls TLS connector for tungstenite.
    pub fn build_tls_connector(&self) -> anyhow::Result<native_tls::TlsConnector> {
        let cert = fs::read(&self.cert_path)?;
        let key = fs::read(&self.key_path)?;
        let ca = fs::read(&self.ca_path)?;

        let identity = native_tls::Identity::from_pkcs8(&cert, &key)?;

        let mut builder = native_tls::TlsConnector::builder();
        builder.identity(identity);

        // Add CA certificate
        let ca_cert = native_tls::Certificate::from_pem(&ca)?;
        builder.add_root_certificate(ca_cert);

        // Enforce TLS 1.2+
        builder.min_protocol_version(Some(native_tls::Protocol::Tlsv12));

        Ok(builder.build()?)
    }

    /// Build a tungstenite connector ready to use.
    pub fn build_connector(&self) -> anyhow::Result<tokio_tungstenite::tungstenite::Connector> {
        let tls = self.build_tls_connector()?;
        Ok(tokio_tungstenite::tungstenite::Connector::NativeTls(tls))
    }
}

/// Create an insecure TLS connector (dev mode). NOT FOR PRODUCTION.
pub fn insecure_tls_connector() -> anyhow::Result<tokio_tungstenite::tungstenite::Connector> {
    let tls = native_tls::TlsConnector::builder()
        .danger_accept_invalid_certs(true)
        .min_protocol_version(Some(native_tls::Protocol::Tlsv12))
        .build()?;
    Ok(tokio_tungstenite::tungstenite::Connector::NativeTls(tls))
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
        let result = parse_fingerprint("SHA256:ABCDEF0123456789ABCDEF0123456789ABCDEF0123456789ABCDEF0123456789").unwrap();
        assert_eq!(result, "sha256:abcdef0123456789abcdef0123456789abcdef0123456789abcdef0123456789");
    }

    #[test]
    fn test_parse_fingerprint_without_prefix() {
        let result = parse_fingerprint("ABCDEF0123456789ABCDEF0123456789ABCDEF0123456789ABCDEF0123456789").unwrap();
        assert_eq!(result, "sha256:abcdef0123456789abcdef0123456789abcdef0123456789abcdef0123456789");
    }

    #[test]
    fn test_parse_fingerprint_lowercase() {
        let result = parse_fingerprint("abcdef0123456789abcdef0123456789abcdef0123456789abcdef0123456789").unwrap();
        assert_eq!(result, "sha256:abcdef0123456789abcdef0123456789abcdef0123456789abcdef0123456789");
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
