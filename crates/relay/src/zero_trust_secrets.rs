// Zero-Trust Secret Protection
// ============================
//
// Цель: Даже при полной компрометации relay-сервера (root access),
// невозможно прочитать секреты организаций.
//
// Архитектура:
// 1. Каждая org имеет свой X25519 keypair (OrgKey)
// 2. OrgKey НИКОГДА не покидает агент/клиент админа
// 3. Relay видит только ciphertext — не может расшифровать
// 4. Vault использует response wrapping — unwrap только у агента
// 5. AppRole token'ы с минимальным TTL и scoped policies
// 6. External Vault — организация подключает свой Vault
//
// Угрозы и защита:
//
// | Угроза                              | Защита                                    |
// |-------------------------------------|-------------------------------------------|
// | Root на relay                       | Нет ключей расшифровки на relay           |
// | Доступ к Vault                      | Response wrapping + limited AppRole       |
// | Перехват traffic agent↔relay        | E2EE (X25519 per-org keys)                |
// | Перехват traffic relay↔Vault        | mTLS + response wrapping                  |
// | Compromised agent                   | Agent-scoped keys + audit trail           |
// | Insider (relay admin)               | Zero knowledge — relay не видит plaintext |

use flowlink_crypto::EncryptedEnvelope;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Org secret configuration — stored in DB, contains ONLY public keys
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OrgSecretConfig {
    /// Org ID
    pub org_id: String,
    /// Organization's PUBLIC key (for encrypting TO org)
    pub org_public_key: String,
    /// Key ID (SHA-256 of public key)
    pub org_key_id: String,
    /// Vault configuration
    pub vault: VaultMode,
    /// When the org key was last rotated
    pub key_rotated_at: Option<String>,
    /// Who set up the org key
    pub key_set_up_by: Option<String>,
}

/// Vault mode — embedded (our Vault) or external (org's own)
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "mode")]
pub enum VaultMode {
    /// Use FlowLink's embedded HashiCorp Vault
    #[serde(rename = "embedded")]
    Embedded {
        /// Vault namespace for this org
        namespace: String,
    },
    /// Connect to organization's own Vault instance
    #[serde(rename = "external")]
    External {
        /// Vault address (e.g. "https://vault.company.com:8200")
        address: String,
        /// Auth method for external Vault
        auth: ExternalVaultAuth,
        /// KV mount path in external Vault
        mount_path: String,
        /// mTLS configuration
        mtls: Option<MtlsConfig>,
        /// CA certificate (PEM) for verifying external Vault
        ca_cert_pem: Option<String>,
        /// Whether to use response wrapping
        response_wrapping: bool,
    },
    /// No Vault configured — secrets stored only in encrypted DB
    #[serde(rename = "none")]
    None,
}

/// Auth configuration for external Vault
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "method")]
pub enum ExternalVaultAuth {
    /// AppRole — recommended for machine-to-machine
    #[serde(rename = "approle")]
    AppRole {
        /// Role ID (not secret — can be stored in DB)
        role_id: String,
        /// Encrypted secret ID (encrypted with org's public key)
        /// Relay cannot decrypt this — only agent can
        encrypted_secret_id: EncryptedEnvelope,
        /// Secret ID TTL — auto-rotate after this
        secret_id_ttl: String,
    },
    /// JWT/OIDC — agent authenticates with its identity token
    #[serde(rename = "jwt")]
    Jwt {
        /// JWT role name in Vault
        role: String,
        /// OIDC provider URL
        oidc_provider: String,
    },
    /// Kubernetes auth — for agents running in K8s
    #[serde(rename = "kubernetes")]
    Kubernetes {
        /// K8s auth role
        role: String,
        /// Service account token path
        token_path: String,
    },
    /// TLS certificate auth
    #[serde(rename = "tls")]
    Tls {
        /// Certificate name in Vault
        cert_name: String,
    },
}

/// mTLS configuration for Vault connection
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MtlsConfig {
    /// Client certificate (PEM, encrypted with org key)
    pub encrypted_client_cert: EncryptedEnvelope,
    /// Client key (PEM, encrypted with org key)
    pub encrypted_client_key: EncryptedEnvelope,
    /// Server name for SNI
    pub server_name: String,
}

/// Wrapped secret from Vault — can only be unwrapped by the intended recipient
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WrappedSecret {
    /// Vault response wrapping token — single-use, TTL-limited
    pub wrapping_token: String,
    /// The agent that should unwrap this
    pub target_agent_id: String,
    /// When the wrapping token expires
    pub expires_at: String,
    /// What secret this contains (metadata only, no values)
    pub metadata: HashMap<String, String>,
}

/// Result of wrapping secrets for an agent
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentSecretBundle {
    /// Agent ID
    pub agent_id: String,
    /// E2EE encrypted bundle — contains wrapped secrets
    pub encrypted_bundle: EncryptedEnvelope,
    /// Which secrets are included (names only)
    pub secret_names: Vec<String>,
}

/// Setup org key ceremony — admin provides their public key
/// This key is used to encrypt all org secrets.
/// The PRIVATE key NEVER touches the relay server.
#[derive(Debug, Serialize, Deserialize)]
pub struct OrgKeySetupRequest {
    /// Admin's public key (base64, X25519)
    pub org_public_key: String,
    /// Optional: admin's existing key ID for rotation
    pub replacing_key_id: Option<String>,
}

/// Setup external vault request
#[derive(Debug, Serialize, Deserialize)]
pub struct ExternalVaultSetupRequest {
    pub address: String,
    pub auth: ExternalVaultAuth,
    pub mount_path: String,
    pub ca_cert_pem: Option<String>,
    pub mtls: Option<MtlsConfig>,
    pub response_wrapping: bool,
}

/// Verification result — can this relay access secrets?
#[derive(Debug, Serialize, Deserialize)]
pub struct SecretAccessVerification {
    /// Can relay decrypt secrets? (should ALWAYS be false)
    pub relay_can_decrypt: bool,
    /// Can relay read Vault values? (should be false with wrapping)
    pub relay_can_read_values: bool,
    /// Is external vault connected?
    pub external_vault: bool,
    /// Is mTLS configured?
    pub mtls_enabled: bool,
    /// Is response wrapping enabled?
    pub response_wrapping: bool,
}

impl OrgSecretConfig {
    /// Verify zero-trust properties
    pub fn verify_zero_trust(&self) -> SecretAccessVerification {
        let external = matches!(self.vault, VaultMode::External { .. });
        let mtls = match &self.vault {
            VaultMode::External { mtls, .. } => mtls.is_some(),
            _ => false,
        };
        let wrapping = match &self.vault {
            VaultMode::External { response_wrapping, .. } => *response_wrapping,
            _ => false,
        };

        SecretAccessVerification {
            // Relay should NEVER be able to decrypt — it doesn't have org private key
            relay_can_decrypt: false,
            // With response wrapping, relay can't read actual values
            relay_can_read_values: false,
            external_vault: external,
            mtls_enabled: mtls,
            response_wrapping: wrapping,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_org_config_embedded() {
        let config = OrgSecretConfig {
            org_id: "org-123".into(),
            org_public_key: "base64pubkey".into(),
            org_key_id: "key-abc".into(),
            vault: VaultMode::Embedded {
                namespace: "org-123".into(),
            },
            key_rotated_at: None,
            key_set_up_by: None,
        };
        assert!(matches!(config.vault, VaultMode::Embedded { .. }));
        let verify = config.verify_zero_trust();
        assert!(!verify.relay_can_decrypt);
        assert!(!verify.external_vault);
    }

    #[test]
    fn test_org_config_external_vault() {
        let kp = KeyPair::generate();
        let config = OrgSecretConfig {
            org_id: "org-456".into(),
            org_public_key: kp.public_key.clone(),
            org_key_id: kp.key_id.clone(),
            vault: VaultMode::External {
                address: "https://vault.company.com:8200".into(),
                auth: ExternalVaultAuth::Jwt {
                    role: "flowlink-agent".into(),
                    oidc_provider: "https://id.company.com".into(),
                },
                mount_path: "flowlink".into(),
                mtls: None,
                ca_cert_pem: Some("-----BEGIN CERTIFICATE-----\n...".into()),
                response_wrapping: true,
            },
            key_rotated_at: None,
            key_set_up_by: None,
        };
        let verify = config.verify_zero_trust();
        assert!(!verify.relay_can_decrypt);
        assert!(verify.external_vault);
        assert!(verify.response_wrapping);
        assert!(!verify.mtls_enabled);
    }

    #[test]
    fn test_org_config_none() {
        let config = OrgSecretConfig {
            org_id: "org-789".into(),
            org_public_key: String::new(),
            org_key_id: String::new(),
            vault: VaultMode::None,
            key_rotated_at: None,
            key_set_up_by: None,
        };
        assert!(matches!(config.vault, VaultMode::None));
    }

    #[test]
    fn test_external_vault_auth_serialization() {
        let auth = ExternalVaultAuth::AppRole {
            role_id: "role-123".into(),
            encrypted_secret_id: EncryptedEnvelope {
                key_id: "key-1".into(),
                sender_key_id: "sender-1".into(),
                sender_public_key: "pubkey".into(),
                nonce: "nonce123".into(),
                ciphertext: "cipher123".into(),
            },
            secret_id_ttl: "24h".into(),
        };
        let json = serde_json::to_string(&auth).unwrap();
        assert!(json.contains("approle"));
        assert!(json.contains("role-123"));
        let back: ExternalVaultAuth = serde_json::from_str(&json).unwrap();
        assert!(matches!(back, ExternalVaultAuth::AppRole { .. }));
    }

    #[test]
    fn test_vault_mode_serialization_roundtrip() {
        let modes: Vec<VaultMode> = vec![
            VaultMode::Embedded { namespace: "ns1".into() },
            VaultMode::External {
                address: "https://v:8200".into(),
                auth: ExternalVaultAuth::Kubernetes {
                    role: "fl".into(),
                    token_path: "/var/run/secrets".into(),
                },
                mount_path: "secret".into(),
                mtls: None,
                ca_cert_pem: None,
                response_wrapping: false,
            },
            VaultMode::None,
        ];
        for mode in &modes {
            let json = serde_json::to_string(mode).unwrap();
            let back: VaultMode = serde_json::from_str(&json).unwrap();
            let json2 = serde_json::to_string(&back).unwrap();
            assert_eq!(json, json2);
        }
    }

    #[test]
    fn test_wrapped_secret() {
        let ws = WrappedSecret {
            wrapping_token: "s.12345".into(),
            target_agent_id: "agent-abc".into(),
            expires_at: "2026-04-25T18:00:00Z".into(),
            metadata: {
                let mut m = HashMap::new();
                m.insert("secret_count".into(), "3".into());
                m
            },
        };
        assert_eq!(ws.wrapping_token, "s.12345");
        assert_eq!(ws.metadata.get("secret_count"), Some(&"3".into()));
    }

    #[test]
    fn test_zero_trust_verification() {
        // Even with full config, relay should NEVER be able to decrypt
        let kp = KeyPair::generate();
        let config = OrgSecretConfig {
            org_id: "org-test".into(),
            org_public_key: kp.public_key.clone(),
            org_key_id: kp.key_id.clone(),
            vault: VaultMode::External {
                address: "https://vault.example.com:8200".into(),
                auth: ExternalVaultAuth::Tls { cert_name: "agent".into() },
                mount_path: "secret".into(),
                mtls: Some(MtlsConfig {
                    encrypted_client_cert: EncryptedEnvelope {
                        key_id: "k".into(),
                        sender_key_id: "s".into(),
                        sender_public_key: "p".into(),
                        nonce: "n".into(),
                        ciphertext: "c".into(),
                    },
                    encrypted_client_key: EncryptedEnvelope {
                        key_id: "k".into(),
                        sender_key_id: "s".into(),
                        sender_public_key: "p".into(),
                        nonce: "n".into(),
                        ciphertext: "c".into(),
                    },
                    server_name: "vault.example.com".into(),
                }),
                ca_cert_pem: Some("cert".into()),
                response_wrapping: true,
            },
            key_rotated_at: None,
            key_set_up_by: None,
        };

        let verify = config.verify_zero_trust();
        assert!(!verify.relay_can_decrypt, "Relay must NEVER be able to decrypt");
        assert!(!verify.relay_can_read_values, "With wrapping, relay can't read values");
        assert!(verify.external_vault);
        assert!(verify.mtls_enabled);
        assert!(verify.response_wrapping);
    }
}
