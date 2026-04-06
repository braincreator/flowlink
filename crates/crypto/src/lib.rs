// FlowLink Crypto — E2EE: X25519 key exchange + AES-256-GCM
// Port of internal/crypto/crypto.go

use aes_gcm::aead::{Aead, KeyInit, OsRng};
use aes_gcm::{Aes256Gcm, AeadCore, Nonce};
use anyhow::Result;
use rand::rngs::OsRng as RandRng;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use x25519_dalek::{PublicKey, StaticSecret};
use base64::{Engine, engine::general_purpose::STANDARD as B64};

// ═══════════════════════════════════════════════
// Constants
// ═══════════════════════════════════════════════

pub const AES_KEY_SIZE: usize = 32;
pub const NONCE_SIZE: usize = 12;

// ═══════════════════════════════════════════════
// Keypair
// ═══════════════════════════════════════════════

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct KeyPair {
    /// Base64-encoded X25519 private key (32 bytes)
    pub private_key: String,
    /// Base64-encoded X25519 public key (32 bytes)
    pub public_key: String,
    /// SHA-256(public_key) hex — unique key identifier
    pub key_id: String,
    /// Unix timestamp of creation
    pub created_at: i64,
}

impl KeyPair {
    /// Generate a new X25519 keypair
    pub fn generate() -> Self {
        let secret = StaticSecret::random_from_rng(RandRng);
        let public = PublicKey::from(&secret);
        let pub_bytes = public.as_bytes();
        let priv_bytes = secret.to_bytes();

        let key_id = {
            let mut hasher = Sha256::new();
            hasher.update(pub_bytes);
            format!("{:x}", hasher.finalize())
        };

        Self {
            private_key: B64.encode(priv_bytes),
            public_key: B64.encode(pub_bytes),
            key_id,
            created_at: chrono::Utc::now().timestamp(),
        }
    }

    fn secret(&self) -> Result<StaticSecret> {
        let bytes: [u8; 32] = B64
            .decode(&self.private_key)?
            .try_into()
            .map_err(|_| anyhow::anyhow!("Invalid private key length"))?;
        Ok(StaticSecret::from(bytes))
    }

    fn public_bytes(&self) -> Result<[u8; 32]> {
        let bytes: [u8; 32] = B64
            .decode(&self.public_key)?
            .try_into()
            .map_err(|_| anyhow::anyhow!("Invalid public key length"))?;
        Ok(bytes)
    }
}

// ═══════════════════════════════════════════════
// Encrypted Envelope
// ═══════════════════════════════════════════════

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EncryptedEnvelope {
    pub key_id: String,           // recipient key_id
    pub sender_key_id: String,    // sender key_id
    pub sender_public_key: String, // sender's actual public key (base64)
    pub nonce: String,
    pub ciphertext: String,
}

// ═══════════════════════════════════════════════
// Encrypt / Decrypt
// ═══════════════════════════════════════════════

/// Derive a shared AES-256 key from X25519 key exchange
fn derive_shared_key(
    my_secret: &StaticSecret,
    their_public: &PublicKey,
) -> [u8; AES_KEY_SIZE] {
    let shared = my_secret.diffie_hellman(their_public);
    let shared_bytes = shared.as_bytes();

    // HKDF-SHA256 to derive final key
    let hk = hkdf::Hkdf::<Sha256>::new(None, shared_bytes);
    let mut key = [0u8; AES_KEY_SIZE];
    hk.expand(b"flowlink-e2ee-v1", &mut key).expect("HKDF expand failed");
    key
}

/// Encrypt plaintext using our private key + their public key
pub fn encrypt(
    my_keypair: &KeyPair,
    their_public_b64: &str,
    plaintext: &[u8],
) -> Result<EncryptedEnvelope> {
    let my_secret = my_keypair.secret()?;
    let their_pub_bytes: [u8; 32] = B64
        .decode(their_public_b64)?
        .try_into()
        .map_err(|_| anyhow::anyhow!("Invalid their public key"))?;
    let their_public = PublicKey::from(their_pub_bytes);

    let shared_key = derive_shared_key(&my_secret, &their_public);

    let cipher = Aes256Gcm::new_from_slice(&shared_key)?;
    let nonce = Aes256Gcm::generate_nonce(&mut OsRng);

    let ciphertext = cipher
        .encrypt(&nonce, plaintext)
        .map_err(|e| anyhow::anyhow!("Encryption failed: {}", e))?;

    Ok(EncryptedEnvelope {
        key_id: their_public_b64.to_string(), // route to recipient
        sender_key_id: my_keypair.key_id.clone(),
        sender_public_key: my_keypair.public_key.clone(), // actual pubkey for DH
        nonce: B64.encode(nonce),
        ciphertext: B64.encode(ciphertext),
    })
}

/// Decrypt ciphertext using our private key + their public key
pub fn decrypt(
    my_keypair: &KeyPair,
    envelope: &EncryptedEnvelope,
) -> Result<Vec<u8>> {
    let my_secret = my_keypair.secret()?;

    // Use sender's actual public key for DH
    let their_pub_bytes: [u8; 32] = B64
        .decode(&envelope.sender_public_key)?
        .try_into()
        .map_err(|_| anyhow::anyhow!("Invalid sender public key"))?;
    let their_public = PublicKey::from(their_pub_bytes);

    let shared_key = derive_shared_key(&my_secret, &their_public);

    let cipher = Aes256Gcm::new_from_slice(&shared_key)?;

    let nonce_bytes: [u8; NONCE_SIZE] = B64
        .decode(&envelope.nonce)?
        .try_into()
        .map_err(|_| anyhow::anyhow!("Invalid nonce"))?;
    let nonce = Nonce::from(nonce_bytes);

    let ciphertext = B64.decode(&envelope.ciphertext)?;

    let plaintext = cipher
        .decrypt(&nonce, ciphertext.as_ref())
        .map_err(|_| anyhow::anyhow!("Decryption failed — invalid key or corrupted data"))?;

    Ok(plaintext)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_keypair_generate() {
        let kp = KeyPair::generate();
        assert!(!kp.private_key.is_empty());
        assert!(!kp.public_key.is_empty());
        assert!(!kp.key_id.is_empty());
        assert_eq!(kp.key_id.len(), 64); // SHA-256 hex
    }

    #[test]
    fn test_encrypt_decrypt_roundtrip() {
        let alice = KeyPair::generate();
        let bob = KeyPair::generate();

        let plaintext = b"Hello, FlowLink E2EE!";

        // Alice encrypts for Bob
        let envelope = encrypt(&alice, &bob.public_key, plaintext).unwrap();

        // Bob decrypts using Alice's public key as key_id
        let decrypted = decrypt(&bob, &envelope).unwrap();

        assert_eq!(decrypted, plaintext);
    }

    #[test]
    fn test_different_keypairs_fail() {
        let alice = KeyPair::generate();
        let bob = KeyPair::generate();
        let eve = KeyPair::generate();

        let plaintext = b"Secret message";

        // Alice encrypts for Bob
        let envelope = encrypt(&alice, &bob.public_key, plaintext).unwrap();

        // Eve tries to decrypt — should fail
        let result = decrypt(&eve, &envelope);
        assert!(result.is_err());
    }
}

    #[test]
    fn test_different_keypairs_produced() {
        let a = KeyPair::generate();
        let b = KeyPair::generate();
        assert_ne!(a.public_key, b.public_key);
        assert_ne!(a.key_id, b.key_id);
    }

    #[test]
    fn test_encrypt_empty_message() {
        let alice = KeyPair::generate();
        let bob = KeyPair::generate();
        let envelope = encrypt(&alice, &bob.public_key, b"").unwrap();
        let decrypted = decrypt(&bob, &envelope).unwrap();
        assert_eq!(decrypted, b"");
    }

    #[test]
    fn test_encrypt_large_message_1mb() {
        let alice = KeyPair::generate();
        let bob = KeyPair::generate();
        let data = vec![0xAB_u8; 1024 * 1024];
        let envelope = encrypt(&alice, &bob.public_key, &data).unwrap();
        let decrypted = decrypt(&bob, &envelope).unwrap();
        assert_eq!(decrypted.len(), 1024 * 1024);
    }

    #[test]
    fn test_tampered_ciphertext_fails() {
        let alice = KeyPair::generate();
        let bob = KeyPair::generate();
        let mut envelope = encrypt(&alice, &bob.public_key, b"secret").unwrap();
        // tamper ciphertext
        envelope.ciphertext = "AAAA".into();
        assert!(decrypt(&bob, &envelope).is_err());
    }

    #[test]
    fn test_tampered_nonce_fails() {
        let alice = KeyPair::generate();
        let bob = KeyPair::generate();
        let mut envelope = encrypt(&alice, &bob.public_key, b"secret").unwrap();
        envelope.nonce = base64::Engine::encode(&base64::engine::general_purpose::STANDARD, [0u8; 12]);
        assert!(decrypt(&bob, &envelope).is_err());
    }

    #[test]
    fn test_multiple_messages_same_key() {
        let alice = KeyPair::generate();
        let bob = KeyPair::generate();
        for i in 0..10 {
            let msg = format!("message {}", i);
            let envelope = encrypt(&alice, &bob.public_key, msg.as_bytes()).unwrap();
            let decrypted = decrypt(&bob, &envelope).unwrap();
            assert_eq!(String::from_utf8(decrypted).unwrap(), msg);
        }
    }

    #[test]
    fn test_public_key_extraction() {
        let kp = KeyPair::generate();
        let pub_bytes = kp.public_bytes().unwrap();
        assert_eq!(pub_bytes.len(), 32);
        // key_id is SHA-256 of public key bytes
        let mut hasher = sha2::Sha256::new();
        hasher.update(pub_bytes);
        let expected = format!("{:x}", hasher.finalize());
        assert_eq!(kp.key_id, expected);
    }

    #[test]
    fn test_bidirectional_encryption() {
        let alice = KeyPair::generate();
        let bob = KeyPair::generate();

        // Alice → Bob
        let env1 = encrypt(&alice, &bob.public_key, b"hello bob").unwrap();
        let dec1 = decrypt(&bob, &env1).unwrap();
        assert_eq!(dec1, b"hello bob");

        // Bob → Alice
        let env2 = encrypt(&bob, &alice.public_key, b"hello alice").unwrap();
        let dec2 = decrypt(&alice, &env2).unwrap();
        assert_eq!(dec2, b"hello alice");
    }
