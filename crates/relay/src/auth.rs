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

    pub fn validate_token(&self, token: &str) -> Option<Client> {
        let client_id = self.token_to_client.get(token)?;
        let id: String = client_id.value().clone();
        self.clients.get(&id).map(|c| c.value().clone())
    }

    pub fn get_client(&self, client_id: &str) -> Option<Client> {
        self.clients.get(client_id).map(|c| c.value().clone())
    }
}
