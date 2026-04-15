pub mod providers;
pub mod models;
pub mod token_manager;
pub mod storage;
pub mod error;
pub mod utils;

pub use providers::*;
pub use models::*;
pub use token_manager::*;
pub use storage::*;
pub use error::*;
pub use utils::*;

use anyhow::Result;
use std::sync::Arc;
use tokio::sync::RwLock;

// Main OAuth2 orchestrator
pub struct OAuth2Manager {
    pub providers: Arc<RwLock<HashMap<String, Arc<dyn OAuthProvider + Send + Sync>>>>,
    pub storage: Arc<OAuthStorage>,
    pub token_manager: Arc<TokenManager>,
}

impl OAuth2Manager {
    pub fn new(storage: Arc<OAuthStorage>, token_manager: Arc<TokenManager>) -> Self {
        Self {
            providers: Arc::new(RwLock::new(HashMap::new())),
            storage,
            token_manager,
        }
    }

    pub async fn register_provider(&self, provider: OAuthProviderConfig) -> Result<()> {
        let provider_impl = match provider.provider_type.as_str() {
            "google" => Arc::new(GoogleOAuthProvider::new(provider)?),
            "github" => Arc::new(GithubOAuthProvider::new(provider)?),
            "microsoft" => Arc::new(MicrosoftOAuthProvider::new(provider)?),
            _ => {
                return Err(OAuthError::UnknownProvider(provider.provider_type));
            }
        };

        self.providers.write().await.insert(provider.provider_type, provider_impl);

        log::info!("Registered OAuth2 provider: {}", provider.provider_type);
        Ok(())
    }

    pub async fn get_provider(&self, provider_type: &str) -> Result<Arc<dyn OAuthProvider + Send + Sync>> {
        let providers = self.providers.read().await;

        providers.get(provider_type)
            .cloned()
            .ok_or_else(|| OAuthError::ProviderNotFound(provider_type.to_string()))
    }

    pub async fn authorize_url(
        &self,
        provider_type: &str,
        client_id: &str,
        redirect_uri: &str,
        scope: Option<Vec<String>>,
    ) -> Result<String> {
        let provider = self.get_provider(provider_type).await?;

        let state = generate_state();
        let code_verifier = generate_code_verifier();

        let config = OAuthProviderConfig {
            provider_type: provider_type.to_string(),
            client_id: client_id.to_string(),
            client_secret: String::new(), // Get from storage
            redirect_uri: redirect_uri.to_string(),
            scope: scope.unwrap_or_default(),
            state,
            code_verifier,
            auto_approve: false,
        };

        provider.get_authorization_url(&config)
    }

    pub async fn exchange_code_for_token(
        &self,
        provider_type: &str,
        code: &str,
    ) -> Result<OAuthTokenResponse> {
        let provider = self.get_provider(provider_type).await?;
        let config = self.storage.get_provider_config(provider_type).await?;

        provider.exchange_code_for_token(&config, code).await
    }

    pub async fn refresh_token(
        &self,
        provider_type: &str,
        refresh_token: &str,
    ) -> Result<OAuthTokenResponse> {
        let provider = self.get_provider(provider_type).await?;
        let config = self.storage.get_provider_config(provider_type).await?;

        provider.refresh_token(&config, refresh_token).await
    }

    pub async fn revoke_token(
        &self,
        provider_type: &str,
        token: &str,
    ) -> Result<()> {
        let provider = self.get_provider(provider_type).await?;
        let config = self.storage.get_provider_config(provider_type).await?;

        provider.revoke_token(&config, token).await
    }

    pub async fn get_user_info(
        &self,
        provider_type: &str,
        access_token: &str,
    ) -> Result<OAuthUserInfo> {
        let provider = self.get_provider(provider_type).await?;

        provider.get_user_info(access_token).await
    }

    pub async fn generate_state_token(&self, provider_type: &str, user_id: &str) -> Result<String> {
        let state = generate_state();
        let token = generate_state_token(state.clone(), user_id);

        self.storage.save_state_token(provider_type, &state, &token).await?;

        Ok(token)
    }

    pub async fn verify_state_token(&self, provider_type: &str, token: &str) -> Result<Option<String>> {
        self.storage.verify_state_token(provider_type, token).await
    }

    pub async fn cleanup_expired_tokens(&self) -> Result<usize> {
        self.storage.cleanup_expired_tokens().await
    }

    pub async fn get_stats(&self) -> OAuthStats {
        let providers = self.providers.read().await;

        OAuthStats {
            registered_providers: providers.len(),
            providers: providers.iter()
                .map(|(name, provider)| {
                    let info = provider.get_info();
                    (name.clone(), info)
                })
                .collect(),
        }
    }
}

// Generate a random state token
fn generate_state() -> String {
    use rand::Rng;
    let mut rng = rand::thread_rng();
    let bytes: [u8; 32] = rng.gen();
    hex::encode(bytes)
}

// Generate a state token with user association
fn generate_state_token(state: String, user_id: &str) -> String {
    use base64::{Engine as _, engine::general_purpose::STANDARD};
    use ring::hmac;

    let data = format!("{}:{}", state, user_id);

    let key = hmac::Key::new(hmac::HMAC_SHA256, b"flowlink-oauth2-state");
    let signature = hmac::sign(&key, data.as_bytes());
    let signature_b64 = STANDARD.encode(signature);

    format!("{}.{}", state, signature_b64)
}

pub type OAuthProviderBox = Arc<dyn OAuthProvider + Send + Sync>;

#[async_trait::async_trait]
pub trait OAuthProvider: Send + Sync {
    async fn get_authorization_url(&self, config: &OAuthProviderConfig) -> Result<String>;
    async fn exchange_code_for_token(&self, config: &OAuthProviderConfig, code: &str) -> Result<OAuthTokenResponse>;
    async fn refresh_token(&self, config: &OAuthProviderConfig, refresh_token: &str) -> Result<OAuthTokenResponse>;
    async fn revoke_token(&self, config: &OAuthProviderConfig, token: &str) -> Result<()>;
    async fn get_user_info(&self, access_token: &str) -> Result<OAuthUserInfo>;

    fn get_provider_type(&self) -> &str;
    fn get_info(&self) -> OAuthProviderInfo;
}

// Custom error types
#[derive(Debug, thiserror::Error)]
pub enum OAuthError {
    #[error("Unknown provider type: {0}")]
    UnknownProvider(String),

    #[error("Provider not found: {0}")]
    ProviderNotFound(String),

    #[error("OAuth2 error: {0}")]
    OAuth2Error(#[from] oauth2::reqwest::Error),

    #[error("Invalid authorization code: {0}")]
    InvalidAuthorizationCode(String),

    #[error("Invalid refresh token: {0}")]
    InvalidRefreshToken(String),

    #[error("Access token expired: {0}")]
    AccessTokenExpired(String),

    #[error("State token mismatch: {0}")]
    StateTokenMismatch(String),

    #[error("User authentication failed: {0}")]
    UserAuthenticationFailed(String),

    #[error("Invalid state token")]
    InvalidStateToken,

    #[error("Token expired")]
    TokenExpired,

    #[error("Storage error: {0}")]
    StorageError(#[from] Box<dyn std::error::Error + Send + Sync>),

    #[error("Configuration error: {0}")]
    ConfigurationError(String),
}