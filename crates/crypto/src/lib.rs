// FlowLink Crypto — E2EE: X25519 key exchange + AES-256-GCM
// Hash utilities: SHA-256, HMAC-SHA256
// Port of internal/crypto/crypto.go

use aes_gcm::aead::{Aead, KeyInit, OsRng};
use aes_gcm::{AeadCore, Aes256Gcm, Nonce};
use anyhow::Result;
use base64::{engine::general_purpose::STANDARD as B64, Engine};
use rand::rngs::OsRng as RandRng;
use serde::{Deserialize, Serialize};
use x25519_dalek::{PublicKey, StaticSecret};

// Re-export sha2 for streaming hash use cases
pub use sha2::{Digest, Sha256};

// ═══════════════════════════════════════════════
// Constants
// ═══════════════════════════════════════════════

pub const AES_KEY_SIZE: usize = 32;
pub const NONCE_SIZE: usize = 12;

// ═══════════════════════════════════════════════
// Hash & HMAC Utilities
// ═══════════════════════════════════════════════

/// Compute SHA-256 hash, returning raw 32 bytes.
pub fn sha256(data: &[u8]) -> [u8; 32] {
    let mut hasher = Sha256::new();
    hasher.update(data);
    hasher.finalize().into()
}

/// Compute SHA-256 hash as a lowercase hex string (64 chars).
pub fn sha256_hex(data: &[u8]) -> String {
    let mut hasher = Sha256::new();
    hasher.update(data);
    format!("{:x}", hasher.finalize())
}

/// Compute HMAC-SHA256 as a lowercase hex string (64 chars).
///
/// # Panics
/// Panics if the key length exceeds HMAC's maximum (512 bytes).
pub fn hmac_sha256_hex(key: &[u8], data: &[u8]) -> String {
    use hmac::{Hmac, Mac};
    type HmacSha256 = Hmac<Sha256>;
    let mut mac = <HmacSha256 as Mac>::new_from_slice(key).expect("HMAC key length invalid");
    mac.update(data);
    let result = mac.finalize().into_bytes();
    hex::encode(result)
}

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

    #[cfg(test)]
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
    pub key_id: String,            // recipient key_id
    pub sender_key_id: String,     // sender key_id
    pub sender_public_key: String, // sender's actual public key (base64)
    pub nonce: String,
    pub ciphertext: String,
}

// ═══════════════════════════════════════════════
// Encrypt / Decrypt
// ═══════════════════════════════════════════════

/// Derive a shared AES-256 key from X25519 key exchange
fn derive_shared_key(my_secret: &StaticSecret, their_public: &PublicKey) -> [u8; AES_KEY_SIZE] {
    let shared = my_secret.diffie_hellman(their_public);
    let shared_bytes = shared.as_bytes();

    // HKDF-SHA256 to derive final key
    let hk = hkdf::Hkdf::<Sha256>::new(None, shared_bytes);
    let mut key = [0u8; AES_KEY_SIZE];
    hk.expand(b"flowlink-e2ee-v1", &mut key)
        .expect("HKDF expand failed");
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
pub fn decrypt(my_keypair: &KeyPair, envelope: &EncryptedEnvelope) -> Result<Vec<u8>> {
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
        envelope.nonce =
            base64::Engine::encode(&base64::engine::general_purpose::STANDARD, [0u8; 12]);
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

    // ── Key generation ──

    #[test]
    fn test_multiple_keypairs_all_different() {
        let keys: Vec<_> = (0..20).map(|_| KeyPair::generate()).collect();
        for i in 0..keys.len() {
            for j in (i + 1)..keys.len() {
                assert_ne!(keys[i].public_key, keys[j].public_key);
                assert_ne!(keys[i].private_key, keys[j].private_key);
                assert_ne!(keys[i].key_id, keys[j].key_id);
            }
        }
    }

    #[test]
    fn test_public_key_derivation_consistency() {
        let kp = KeyPair::generate();
        let b1 = kp.public_bytes().unwrap();
        let b2 = kp.public_bytes().unwrap();
        assert_eq!(b1, b2);
    }

    #[test]
    fn test_key_serialization_roundtrip() {
        let kp = KeyPair::generate();
        let priv_bytes: Vec<u8> = B64.decode(&kp.private_key).unwrap();
        let pub_bytes: Vec<u8> = B64.decode(&kp.public_key).unwrap();
        assert_eq!(priv_bytes.len(), 32);
        assert_eq!(pub_bytes.len(), 32);
    }

    #[test]
    fn test_invalid_public_key_too_short() {
        let short = B64.encode([0u8; 16]);
        let result: Result<[u8; 32], _> = B64.decode(&short).unwrap().try_into();
        assert!(result.is_err());
    }

    #[test]
    fn test_invalid_public_key_too_long() {
        let long = B64.encode([0u8; 64]);
        let result: Result<[u8; 32], _> = B64.decode(&long).unwrap().try_into();
        assert!(result.is_err());
    }

    // ── Encryption / Decryption ──

    #[test]
    fn test_encrypt_one_byte() {
        let alice = KeyPair::generate();
        let bob = KeyPair::generate();
        let envelope = encrypt(&alice, &bob.public_key, &[0x42]).unwrap();
        let decrypted = decrypt(&bob, &envelope).unwrap();
        assert_eq!(decrypted, vec![0x42]);
    }

    #[test]
    fn test_encrypt_1kb() {
        let alice = KeyPair::generate();
        let bob = KeyPair::generate();
        let data = vec![0xCC; 1024];
        let envelope = encrypt(&alice, &bob.public_key, &data).unwrap();
        let decrypted = decrypt(&bob, &envelope).unwrap();
        assert_eq!(decrypted, data);
    }

    #[test]
    fn test_encrypt_10kb() {
        let alice = KeyPair::generate();
        let bob = KeyPair::generate();
        let data = vec![0xDD; 10_240];
        let envelope = encrypt(&alice, &bob.public_key, &data).unwrap();
        let decrypted = decrypt(&bob, &envelope).unwrap();
        assert_eq!(decrypted, data);
    }

    #[test]
    fn test_encrypt_100kb() {
        let alice = KeyPair::generate();
        let bob = KeyPair::generate();
        let data = vec![0xEE; 102_400];
        let envelope = encrypt(&alice, &bob.public_key, &data).unwrap();
        let decrypted = decrypt(&bob, &envelope).unwrap();
        assert_eq!(decrypted, data);
    }

    #[test]
    fn test_encrypt_same_message_different_ciphertexts() {
        let alice = KeyPair::generate();
        let bob = KeyPair::generate();
        let msg = b"deterministic is bad";
        let env1 = encrypt(&alice, &bob.public_key, msg).unwrap();
        let env2 = encrypt(&alice, &bob.public_key, msg).unwrap();
        assert_ne!(env1.ciphertext, env2.ciphertext);
        assert_ne!(env1.nonce, env2.nonce);
    }

    #[test]
    fn test_decrypt_with_wrong_key() {
        let alice = KeyPair::generate();
        let bob = KeyPair::generate();
        let eve = KeyPair::generate();
        let env = encrypt(&alice, &bob.public_key, b"secret").unwrap();
        assert!(decrypt(&eve, &env).is_err());
    }

    #[test]
    fn test_corrupted_ciphertext_fails() {
        let alice = KeyPair::generate();
        let bob = KeyPair::generate();
        let mut env = encrypt(&alice, &bob.public_key, b"data").unwrap();
        let mut ct = B64.decode(&env.ciphertext).unwrap();
        ct[0] ^= 0xFF;
        env.ciphertext = B64.encode(&ct);
        assert!(decrypt(&bob, &env).is_err());
    }

    #[test]
    fn test_corrupted_nonce_fails() {
        let alice = KeyPair::generate();
        let bob = KeyPair::generate();
        let mut env = encrypt(&alice, &bob.public_key, b"data").unwrap();
        let mut nonce = B64.decode(&env.nonce).unwrap();
        nonce[0] ^= 0xFF;
        env.nonce = B64.encode(&nonce);
        assert!(decrypt(&bob, &env).is_err());
    }

    #[test]
    fn test_ciphertext_too_short() {
        let alice = KeyPair::generate();
        let bob = KeyPair::generate();
        let mut env = encrypt(&alice, &bob.public_key, b"x").unwrap();
        env.ciphertext = B64.encode([0u8; 4]); // too short for GCM tag
        assert!(decrypt(&bob, &env).is_err());
    }

    #[test]
    fn test_truncated_ciphertext_fails() {
        let alice = KeyPair::generate();
        let bob = KeyPair::generate();
        let mut env = encrypt(&alice, &bob.public_key, b"truncation test").unwrap();
        let ct = B64.decode(&env.ciphertext).unwrap();
        env.ciphertext = B64.encode(&ct[..ct.len().saturating_sub(5)]);
        assert!(decrypt(&bob, &env).is_err());
    }

    #[test]
    fn test_invalid_nonce_base64() {
        let alice = KeyPair::generate();
        let bob = KeyPair::generate();
        let mut env = encrypt(&alice, &bob.public_key, b"x").unwrap();
        env.nonce = "not-valid-base64!!!".into();
        assert!(decrypt(&bob, &env).is_err());
    }

    #[test]
    fn test_invalid_sender_public_key() {
        let alice = KeyPair::generate();
        let bob = KeyPair::generate();
        let mut env = encrypt(&alice, &bob.public_key, b"x").unwrap();
        env.sender_public_key = "invalid!!".into();
        assert!(decrypt(&bob, &env).is_err());
    }

    // ── HKDF / Key Derivation ──

    #[test]
    fn test_hkdf_deterministic() {
        let alice = KeyPair::generate();
        let bob = KeyPair::generate();
        let secret1 = alice
            .secret()
            .unwrap()
            .diffie_hellman(&PublicKey::from(bob.public_bytes().unwrap()));
        let secret2 = alice
            .secret()
            .unwrap()
            .diffie_hellman(&PublicKey::from(bob.public_bytes().unwrap()));
        assert_eq!(secret1.as_bytes(), secret2.as_bytes());
    }

    #[test]
    fn test_hkdf_different_info_different_keys() {
        let alice = KeyPair::generate();
        let bob = KeyPair::generate();
        let shared = alice
            .secret()
            .unwrap()
            .diffie_hellman(&PublicKey::from(bob.public_bytes().unwrap()));

        let hk1 = hkdf::Hkdf::<Sha256>::new(None, shared.as_bytes());
        let hk2 = hkdf::Hkdf::<Sha256>::new(None, shared.as_bytes());
        let mut k1 = [0u8; 32];
        let mut k2 = [0u8; 32];
        hk1.expand(b"info-a", &mut k1).unwrap();
        hk2.expand(b"info-b", &mut k2).unwrap();
        assert_ne!(k1, k2);
    }

    #[test]
    fn test_hkdf_empty_info() {
        let alice = KeyPair::generate();
        let bob = KeyPair::generate();
        let shared = alice
            .secret()
            .unwrap()
            .diffie_hellman(&PublicKey::from(bob.public_bytes().unwrap()));
        let hk = hkdf::Hkdf::<Sha256>::new(None, shared.as_bytes());
        let mut key = [0u8; 32];
        hk.expand(b"", &mut key).unwrap();
        assert_ne!(key, [0u8; 32]);
    }

    #[test]
    fn test_hkdf_long_info() {
        let alice = KeyPair::generate();
        let bob = KeyPair::generate();
        let shared = alice
            .secret()
            .unwrap()
            .diffie_hellman(&PublicKey::from(bob.public_bytes().unwrap()));
        let hk = hkdf::Hkdf::<Sha256>::new(None, shared.as_bytes());
        let long_info = vec![b'x'; 1024];
        let mut key = [0u8; 32];
        hk.expand(&long_info, &mut key).unwrap();
        assert_ne!(key, [0u8; 32]);
    }

    #[test]
    fn test_derived_key_length() {
        let alice = KeyPair::generate();
        let bob = KeyPair::generate();
        let shared = alice
            .secret()
            .unwrap()
            .diffie_hellman(&PublicKey::from(bob.public_bytes().unwrap()));
        let hk = hkdf::Hkdf::<Sha256>::new(None, shared.as_bytes());
        let mut key = [0u8; 32];
        hk.expand(b"flowlink-e2ee-v1", &mut key).unwrap();
        assert_eq!(key.len(), AES_KEY_SIZE);
    }

    #[test]
    fn test_hkdf_different_shared_secrets_different_keys() {
        let alice = KeyPair::generate();
        let bob = KeyPair::generate();
        let carol = KeyPair::generate();

        let s1 = alice
            .secret()
            .unwrap()
            .diffie_hellman(&PublicKey::from(bob.public_bytes().unwrap()));
        let s2 = alice
            .secret()
            .unwrap()
            .diffie_hellman(&PublicKey::from(carol.public_bytes().unwrap()));

        let mut k1 = [0u8; 32];
        let mut k2 = [0u8; 32];
        hkdf::Hkdf::<Sha256>::new(None, s1.as_bytes())
            .expand(b"test", &mut k1)
            .unwrap();
        hkdf::Hkdf::<Sha256>::new(None, s2.as_bytes())
            .expand(b"test", &mut k2)
            .unwrap();
        assert_ne!(k1, k2);
    }

    // ── End-to-end scenarios ──

    #[test]
    fn test_full_e2ee_flow() {
        let alice = KeyPair::generate();
        let bob = KeyPair::generate();
        let plaintext = b"Alice's secret message to Bob";

        let envelope = encrypt(&alice, &bob.public_key, plaintext).unwrap();
        assert_eq!(envelope.sender_key_id, alice.key_id);
        assert_eq!(envelope.sender_public_key, alice.public_key);

        let decrypted = decrypt(&bob, &envelope).unwrap();
        assert_eq!(decrypted, plaintext);
    }

    #[test]
    fn test_multi_recipient() {
        let alice = KeyPair::generate();
        let bob = KeyPair::generate();
        let charlie = KeyPair::generate();
        let msg = b"Multi-cast message";

        let env_bob = encrypt(&alice, &bob.public_key, msg).unwrap();
        let env_charlie = encrypt(&alice, &charlie.public_key, msg).unwrap();

        assert_ne!(env_bob.ciphertext, env_charlie.ciphertext);
        assert_eq!(decrypt(&bob, &env_bob).unwrap(), msg);
        assert_eq!(decrypt(&charlie, &env_charlie).unwrap(), msg);
        assert!(decrypt(&bob, &env_charlie).is_err());
        assert!(decrypt(&charlie, &env_bob).is_err());
    }

    #[test]
    fn test_key_rotation() {
        let alice = KeyPair::generate();
        let bob_old = KeyPair::generate();
        let bob_new = KeyPair::generate();
        let msg = b"rotate me";

        let env_old = encrypt(&alice, &bob_old.public_key, msg).unwrap();
        let env_new = encrypt(&alice, &bob_new.public_key, msg).unwrap();

        assert_eq!(decrypt(&bob_old, &env_old).unwrap(), msg);
        assert!(decrypt(&bob_new, &env_old).is_err());
        assert_eq!(decrypt(&bob_new, &env_new).unwrap(), msg);
        assert!(decrypt(&bob_old, &env_new).is_err());
    }

    #[test]
    fn test_session_key_derivation_with_nonce() {
        let alice = KeyPair::generate();
        let bob = KeyPair::generate();
        let shared = alice
            .secret()
            .unwrap()
            .diffie_hellman(&PublicKey::from(bob.public_bytes().unwrap()));

        // Derive session key with nonce
        let nonce = b"session-nonce-12345";
        let mut preimage = shared.as_bytes().to_vec();
        preimage.extend_from_slice(nonce);
        let hk = hkdf::Hkdf::<Sha256>::new(None, &preimage);
        let mut session_key = [0u8; 32];
        hk.expand(b"session-key", &mut session_key).unwrap();

        // Use session key to encrypt
        let cipher = Aes256Gcm::new_from_slice(&session_key).unwrap();
        let nonce12 = Aes256Gcm::generate_nonce(&mut OsRng);
        let ct = cipher.encrypt(&nonce12, b"session data".as_ref()).unwrap();
        let pt = cipher.decrypt(&nonce12, ct.as_ref()).unwrap();
        assert_eq!(pt, b"session data");
    }

    // ── Edge cases ──

    #[test]
    fn test_binary_data_non_utf8() {
        let alice = KeyPair::generate();
        let bob = KeyPair::generate();
        let data: Vec<u8> = (0u8..=255).collect();
        let env = encrypt(&alice, &bob.public_key, &data).unwrap();
        let decrypted = decrypt(&bob, &env).unwrap();
        assert_eq!(decrypted, data);
    }

    #[test]
    fn test_unicode_emoji() {
        let alice = KeyPair::generate();
        let bob = KeyPair::generate();
        let msg = "Hello 🌍 مرحبا 안녕하세요 🚀✨";
        let env = encrypt(&alice, &bob.public_key, msg.as_bytes()).unwrap();
        let decrypted = decrypt(&bob, &env).unwrap();
        assert_eq!(String::from_utf8(decrypted).unwrap(), msg);
    }

    #[test]
    fn test_null_bytes_in_message() {
        let alice = KeyPair::generate();
        let bob = KeyPair::generate();
        let data = b"before\x00null\x00after";
        let env = encrypt(&alice, &bob.public_key, data).unwrap();
        let decrypted = decrypt(&bob, &env).unwrap();
        assert_eq!(decrypted, data);
    }

    #[test]
    fn test_repeated_encryption_100_times() {
        let alice = KeyPair::generate();
        let bob = KeyPair::generate();
        let msg = b"repeated 100x";
        for i in 0..100 {
            let env = encrypt(&alice, &bob.public_key, msg).unwrap();
            let decrypted = decrypt(&bob, &env).unwrap();
            assert_eq!(decrypted, msg, "failed on iteration {}", i);
        }
    }

    #[test]
    fn test_envelope_serialization_roundtrip() {
        let alice = KeyPair::generate();
        let bob = KeyPair::generate();
        let env = encrypt(&alice, &bob.public_key, b"serialize me").unwrap();
        let json = serde_json::to_string(&env).unwrap();
        let deserialized: EncryptedEnvelope = serde_json::from_str(&json).unwrap();
        assert_eq!(deserialized.nonce, env.nonce);
        assert_eq!(deserialized.ciphertext, env.ciphertext);
        let decrypted = decrypt(&bob, &deserialized).unwrap();
        assert_eq!(decrypted, b"serialize me");
    }

    #[test]
    fn test_keypair_serialization_roundtrip() {
        let kp = KeyPair::generate();
        let json = serde_json::to_string(&kp).unwrap();
        let deserialized: KeyPair = serde_json::from_str(&json).unwrap();
        assert_eq!(deserialized.private_key, kp.private_key);
        assert_eq!(deserialized.public_key, kp.public_key);
        assert_eq!(deserialized.key_id, kp.key_id);
    }

    #[test]
    fn test_zero_plaintext_still_has_nonce_and_ciphertext() {
        let alice = KeyPair::generate();
        let bob = KeyPair::generate();
        let env = encrypt(&alice, &bob.public_key, b"").unwrap();
        assert!(!env.nonce.is_empty());
        assert!(!env.ciphertext.is_empty());
    }

    #[test]
    fn test_ciphertext_longer_than_plaintext() {
        let alice = KeyPair::generate();
        let bob = KeyPair::generate();
        let pt = b"short";
        let env = encrypt(&alice, &bob.public_key, pt).unwrap();
        let ct = B64.decode(&env.ciphertext).unwrap();
        assert!(ct.len() > pt.len()); // GCM adds 16-byte tag
    }

    #[test]
    fn test_sender_key_id_matches_keypair() {
        let alice = KeyPair::generate();
        let bob = KeyPair::generate();
        let env = encrypt(&alice, &bob.public_key, b"check").unwrap();
        assert_eq!(env.sender_key_id, alice.key_id);
        assert_eq!(env.sender_public_key, alice.public_key);
    }

    #[test]
    fn test_different_plaintexts_different_ciphertexts() {
        let alice = KeyPair::generate();
        let bob = KeyPair::generate();
        let env1 = encrypt(&alice, &bob.public_key, b"message one").unwrap();
        let env2 = encrypt(&alice, &bob.public_key, b"message two").unwrap();
        assert_ne!(env1.ciphertext, env2.ciphertext);
    }

    #[test]
    fn test_all_zeros_message() {
        let alice = KeyPair::generate();
        let bob = KeyPair::generate();
        let data = vec![0u8; 512];
        let env = encrypt(&alice, &bob.public_key, &data).unwrap();
        let decrypted = decrypt(&bob, &env).unwrap();
        assert_eq!(decrypted, data);
    }

    #[test]
    fn test_all_ones_message() {
        let alice = KeyPair::generate();
        let bob = KeyPair::generate();
        let data = vec![0xFF; 512];
        let env = encrypt(&alice, &bob.public_key, &data).unwrap();
        let decrypted = decrypt(&bob, &env).unwrap();
        assert_eq!(decrypted, data);
    }

    #[test]
    fn test_created_at_is_recent() {
        let before = chrono::Utc::now().timestamp();
        let kp = KeyPair::generate();
        let after = chrono::Utc::now().timestamp();
        assert!(kp.created_at >= before && kp.created_at <= after);
    }

    // ── Hash & HMAC Utilities ──

    #[test]
    fn test_sha256_known_vector() {
        // SHA-256("abc") = ba7816bf 8f01cfea 414140de 5dae2223 b00361a3 96177a9c b410ff61 f20015ad
        let hash = sha256(b"abc");
        assert_eq!(
            hex::encode(hash),
            "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad"
        );
    }

    #[test]
    fn test_sha256_hex_known_vector() {
        let hex_str = sha256_hex(b"abc");
        assert_eq!(
            hex_str,
            "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad"
        );
        assert_eq!(hex_str.len(), 64);
    }

    #[test]
    fn test_sha256_empty() {
        // SHA-256("") = e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855
        let hex_str = sha256_hex(b"");
        assert_eq!(
            hex_str,
            "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855"
        );
    }

    #[test]
    fn test_sha256_deterministic() {
        let h1 = sha256_hex(b"hello world");
        let h2 = sha256_hex(b"hello world");
        assert_eq!(h1, h2);
    }

    #[test]
    fn test_sha256_different_inputs() {
        let h1 = sha256_hex(b"foo");
        let h2 = sha256_hex(b"bar");
        assert_ne!(h1, h2);
    }

    #[test]
    fn test_sha256_bytes_matches_hex() {
        let bytes = sha256(b"test");
        let hex_str = sha256_hex(b"test");
        assert_eq!(hex::encode(bytes), hex_str);
    }

    #[test]
    fn test_hmac_sha256_hex_known_vector() {
        // RFC 4231 Test Case 2: key="Jefe", data="what do ya want for nothing?"
        let hex_str = hmac_sha256_hex(b"Jefe", b"what do ya want for nothing?");
        assert_eq!(
            hex_str,
            "5bdcc146bf60754e6a042426089575c75a003f089d2739839dec58b964ec3843"
        );
    }

    #[test]
    fn test_hmac_sha256_hex_deterministic() {
        let h1 = hmac_sha256_hex(b"key", b"data");
        let h2 = hmac_sha256_hex(b"key", b"data");
        assert_eq!(h1, h2);
    }

    #[test]
    fn test_hmac_sha256_hex_different_keys() {
        let h1 = hmac_sha256_hex(b"key1", b"data");
        let h2 = hmac_sha256_hex(b"key2", b"data");
        assert_ne!(h1, h2);
    }

    #[test]
    fn test_hmac_sha256_hex_different_data() {
        let h1 = hmac_sha256_hex(b"key", b"data1");
        let h2 = hmac_sha256_hex(b"key", b"data2");
        assert_ne!(h1, h2);
    }

    #[test]
    fn test_hmac_sha256_hex_length() {
        let hex_str = hmac_sha256_hex(b"key", b"data");
        assert_eq!(hex_str.len(), 64); // 32 bytes = 64 hex chars
    }
}
