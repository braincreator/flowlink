// Auth Manager — token validation, client registry
// Port of internal/relay/auth.go

use dashmap::DashMap;
use std::sync::Arc;

#[derive(Debug, Clone)]
pub struct Client {
    pub client_id: String,
    pub api_token: String,
    pub name: String,
    pub active: bool,
}

pub struct AuthManager {
    clients: Arc<DashMap<String, Client>>,
    token_to_client: Arc<DashMap<String, String>>, // token -> client_id
}

impl Default for AuthManager {
    fn default() -> Self {
        Self::new()
    }
}

impl AuthManager {
    pub fn new() -> Self {
        Self {
            clients: Arc::new(DashMap::new()),
            token_to_client: Arc::new(DashMap::new()),
        }
    }

    pub fn register_client(&self, client: Client) {
        let token = client.api_token.clone();
        let id = client.client_id.clone();
        self.token_to_client.insert(token, id.clone());
        self.clients.insert(id, client);
    }

    pub fn is_empty(&self) -> bool {
        self.clients.is_empty()
    }

    pub fn validate_token(&self, token: &str) -> Option<Client> {
        let client_id = self.token_to_client.get(token)?;
        let id: String = client_id.value().clone();
        self.clients.get(&id).map(|c| c.value().clone())
    }

    pub fn get_client(&self, client_id: &str) -> Option<Client> {
        self.clients.get(client_id).map(|c| c.value().clone())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn test_client(id: &str, token: &str) -> Client {
        Client { client_id: id.into(), api_token: token.into(), name: id.into(), active: true }
    }

    #[test]
    fn test_register_and_validate() {
        let auth = AuthManager::new();
        auth.register_client(test_client("c1", "tok1"));
        let c = auth.validate_token("tok1").unwrap();
        assert_eq!(c.client_id, "c1");
    }

    #[test]
    fn test_invalid_token() {
        let auth = AuthManager::new();
        assert!(auth.validate_token("nope").is_none());
    }

    #[test]
    fn test_get_client() {
        let auth = AuthManager::new();
        auth.register_client(test_client("c1", "tok1"));
        assert!(auth.get_client("c1").is_some());
        assert!(auth.get_client("c2").is_none());
    }

    #[test]
    fn test_inactive_client() {
        let auth = AuthManager::new();
        auth.register_client(Client { client_id: "c1".into(), api_token: "tok1".into(), name: "c1".into(), active: false });
        // validate_token still returns the client (doesn't check active flag)
        let c = auth.validate_token("tok1").unwrap();
        assert_eq!(c.client_id, "c1");
    }

    #[test]
    fn test_register_duplicate_overwrites() {
        let auth = AuthManager::new();
        auth.register_client(test_client("c1", "tok1"));
        auth.register_client(Client { client_id: "c1".into(), api_token: "tok2".into(), name: "updated".into(), active: true });
        assert!(auth.validate_token("tok1").is_none()); // old token gone
        assert!(auth.validate_token("tok2").is_some());
    }

    #[test]
    fn test_is_empty() {
        let auth = AuthManager::new();
        assert!(auth.is_empty());
        auth.register_client(test_client("c1", "tok1"));
        assert!(!auth.is_empty());
    }

    #[test]
    fn test_default() {
        let auth = AuthManager::default();
        assert!(auth.is_empty());
    }

    #[test]
    fn test_multiple_clients() {
        let auth = AuthManager::new();
        for i in 0..10 {
            auth.register_client(test_client(&format!("c{i}"), &format!("tok{i}")));
        }
        for i in 0..10 {
            assert!(auth.validate_token(&format!("tok{i}")).is_some());
        }
    }
}
