use std::collections::HashMap;
use tokio::sync::RwLock;
use flowlink_crypto::{KeyPair, EncryptedEnvelope, encrypt, decrypt};

/// Manages E2EE sessions for connected agents.
/// Stores each agent's public key, handles encryption/decryption.
pub struct E2eeSessionManager {
    /// agent_id -> their public key (base64)
    agent_keys: RwLock<HashMap<String, String>>,
    /// Relay's own keypair for encryption
    relay_keypair: KeyPair,
}

impl E2eeSessionManager {
    pub fn new() -> Self {
        Self {
            agent_keys: RwLock::new(HashMap::new()),
            relay_keypair: KeyPair::generate(),
        }
    }

    /// Get relay's public key (base64) and key_id for distribution
    pub fn relay_public_key(&self) -> &str {
        &self.relay_keypair.public_key
    }
    pub fn relay_key_id(&self) -> &str {
        &self.relay_keypair.key_id
    }

    /// Register an agent's public key (from ConnectPayload.public_key)
    pub async fn register_agent_key(&self, agent_id: &str, public_key: &str) {
        self.agent_keys.write().await.insert(agent_id.to_string(), public_key.to_string());
    }

    /// Remove agent key on disconnect
    pub async fn remove_agent_key(&self, agent_id: &str) {
        self.agent_keys.write().await.remove(agent_id);
    }

    /// Encrypt a message for a specific agent using E2EE.
    /// Returns JSON string of EncryptedEnvelope if agent has a registered key.
    /// Returns None if no key registered (falls back to plaintext).
    pub async fn encrypt_for_agent(&self, agent_id: &str, plaintext: &[u8]) -> Option<String> {
        let peer_key = self.agent_keys.read().await.get(agent_id)?.clone();
        let envelope = encrypt(&self.relay_keypair, &peer_key, plaintext).ok()?;
        Some(serde_json::to_string(&envelope).ok()?)
    }

    /// Decrypt a message from an agent using E2EE.
    /// Input is JSON string of EncryptedEnvelope.
    /// Returns decrypted plaintext bytes.
    pub fn decrypt_from_agent(&self, envelope_json: &str) -> Option<Vec<u8>> {
        let envelope: EncryptedEnvelope = serde_json::from_str(envelope_json).ok()?;
        decrypt(&self.relay_keypair, &envelope).ok()
    }

    /// Check if agent has E2EE enabled
    pub async fn is_encrypted(&self, agent_id: &str) -> bool {
        self.agent_keys.read().await.contains_key(agent_id)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_encrypt_for_agent_roundtrip() {
        let manager = E2eeSessionManager::new();

        // Generate a mock agent keypair and register
        let agent_keypair = KeyPair::generate();
        manager.register_agent_key("agent-1", &agent_keypair.public_key).await;

        assert!(manager.is_encrypted("agent-1").await);
        assert!(!manager.is_encrypted("unknown").await);

        let plaintext = b"Hello, E2EE relay!";

        // Relay encrypts for agent-1
        let encrypted = manager.encrypt_for_agent("agent-1", plaintext).await;
        assert!(encrypted.is_some());

        // Verify the envelope structure
        let envelope: EncryptedEnvelope = serde_json::from_str(&encrypted.unwrap()).unwrap();
        assert_eq!(envelope.sender_key_id, manager.relay_key_id());

        // Agent decrypts the relay's message using their own keypair
        let decrypted = decrypt(&agent_keypair, &envelope).unwrap();
        assert_eq!(decrypted, plaintext);
    }

    #[tokio::test]
    async fn test_decrypt_from_agent_roundtrip() {
        let manager = E2eeSessionManager::new();

        // Agent generates keypair, registers public key, and encrypts a message for relay
        let agent_keypair = KeyPair::generate();
        manager.register_agent_key("agent-1", &agent_keypair.public_key).await;

        let plaintext = b"Agent says hello to relay";
        let envelope = encrypt(&agent_keypair, manager.relay_public_key(), plaintext).unwrap();
        let envelope_json = serde_json::to_string(&envelope).unwrap();

        // Relay decrypts the agent's message
        let decrypted = manager.decrypt_from_agent(&envelope_json).unwrap();
        assert_eq!(decrypted, plaintext);
    }

    #[tokio::test]
    async fn test_bidirectional_communication() {
        let manager = E2eeSessionManager::new();
        let agent_keypair = KeyPair::generate();
        manager.register_agent_key("agent-1", &agent_keypair.public_key).await;

        // Relay -> Agent
        let relay_to_agent = manager.encrypt_for_agent("agent-1", b"relay to agent").await.unwrap();
        let env1: EncryptedEnvelope = serde_json::from_str(&relay_to_agent).unwrap();
        let dec1 = decrypt(&agent_keypair, &env1).unwrap();
        assert_eq!(dec1, b"relay to agent");

        // Agent -> Relay
        let env2 = encrypt(&agent_keypair, manager.relay_public_key(), b"agent to relay").unwrap();
        let dec2 = manager.decrypt_from_agent(&serde_json::to_string(&env2).unwrap()).unwrap();
        assert_eq!(dec2, b"agent to relay");
    }

    #[tokio::test]
    async fn test_encrypt_no_key_returns_none() {
        let manager = E2eeSessionManager::new();
        let result = manager.encrypt_for_agent("nonexistent", b"hello").await;
        assert!(result.is_none());
    }

    #[tokio::test]
    async fn test_decrypt_invalid_json_returns_none() {
        let manager = E2eeSessionManager::new();
        let result = manager.decrypt_from_agent("not json");
        assert!(result.is_none());
    }

    #[tokio::test]
    async fn test_decrypt_plaintext_message_returns_none() {
        let manager = E2eeSessionManager::new();
        // A normal FlowLink message (not an encrypted envelope) should fail decryption
        let result = manager.decrypt_from_agent(r#"{"msg_type":"heartbeat","version":1}"#);
        assert!(result.is_none());
    }

    #[tokio::test]
    async fn test_register_and_remove_agent_key() {
        let manager = E2eeSessionManager::new();
        let kp = KeyPair::generate();

        manager.register_agent_key("agent-x", &kp.public_key).await;
        assert!(manager.is_encrypted("agent-x").await);

        manager.remove_agent_key("agent-x").await;
        assert!(!manager.is_encrypted("agent-x").await);
    }

    #[tokio::test]
    async fn test_relay_keypair_properties() {
        let manager = E2eeSessionManager::new();
        assert!(!manager.relay_public_key().is_empty());
        assert!(!manager.relay_key_id().is_empty());
        // key_id is SHA-256 hex = 64 chars
        assert_eq!(manager.relay_key_id().len(), 64);
    }

    #[tokio::test]
    async fn test_multiple_agents_isolated() {
        let manager = E2eeSessionManager::new();
        let kp1 = KeyPair::generate();
        let kp2 = KeyPair::generate();

        manager.register_agent_key("alice", &kp1.public_key).await;
        manager.register_agent_key("bob", &kp2.public_key).await;

        let msg = b"secret for alice";
        let encrypted_alice = manager.encrypt_for_agent("alice", msg).await.unwrap();
        let encrypted_bob = manager.encrypt_for_agent("bob", msg).await.unwrap();

        // Different ciphertexts (different recipient keys)
        let env_alice: EncryptedEnvelope = serde_json::from_str(&encrypted_alice).unwrap();
        let env_bob: EncryptedEnvelope = serde_json::from_str(&encrypted_bob).unwrap();
        assert_ne!(env_alice.ciphertext, env_bob.ciphertext);

        // Each agent can decrypt their own message
        let dec_alice = decrypt(&kp1, &env_alice).unwrap();
        let dec_bob = decrypt(&kp2, &env_bob).unwrap();
        assert_eq!(dec_alice, msg);
        assert_eq!(dec_bob, msg);
    }

    #[tokio::test]
    async fn test_decrypt_tampered_envelope_fails() {
        let manager = E2eeSessionManager::new();
        let kp = KeyPair::generate();
        manager.register_agent_key("agent-1", &kp.public_key).await;

        // Agent sends a valid encrypted message to relay
        let envelope = encrypt(&kp, manager.relay_public_key(), b"hello").unwrap();
        let mut envelope_json = serde_json::to_string(&envelope).unwrap();

        // Tamper with the JSON
        envelope_json = envelope_json.replace(envelope.ciphertext.as_str(), "AAAA");

        let result = manager.decrypt_from_agent(&envelope_json);
        assert!(result.is_none());
    }
}
