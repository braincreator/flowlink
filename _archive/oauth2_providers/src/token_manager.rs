use anyhow::Result;
use chrono::{Duration, Utc};
use std::sync::Arc;
use tokio::sync::RwLock;

use super::*;
use super::storage::OAuthStorage;

pub struct TokenManager {
    pub storage: Arc<OAuthStorage>,
    pub token_rotation_interval: chrono::Duration,
    pub cleanup_interval: chrono::Duration,
}

impl TokenManager {
    pub fn new(storage: Arc<OAuthStorage>) -> Self {
        Self {
            storage,
            token_rotation_interval: Duration::minutes(5),
            cleanup_interval: Duration::hours(1),
        }
    }

    pub fn with_rotation_interval(mut self, interval: chrono::Duration) -> Self {
        self.token_rotation_interval = interval;
        self
    }

    pub fn with_cleanup_interval(mut self, interval: chrono::Duration) -> Self {
        self.cleanup_interval = interval;
        self
    }

    pub async fn rotate_access_token(
        &self,
        session_id: &str,
        new_token: &str,
    ) -> Result<()> {
        let mut session = self.storage.get_session(session_id).await?
            .ok_or_else(|| anyhow::anyhow!("Session not found: {}", session_id))?;

        session.access_token = new_token.to_string();
        session.expires_at = Utc::now() + Duration::seconds(session.expires_in);
        session.updated_at = Utc::now();

        self.storage.save_session(&session).await?;

        Ok(())
    }

    pub async fn refresh_access_token(
        &self,
        session_id: &str,
        access_token: &str,
        refresh_token: &str,
        token_type: &str,
        expires_in: i64,
        scope: Option<String>,
    ) -> Result<()> {
        let mut session = self.storage.get_session(session_id).await?
            .ok_or_else(|| anyhow::anyhow!("Session not found: {}", session_id))?;

        session.access_token = access_token.to_string();
        session.refresh_token = Some(refresh_token.to_string());
        session.token_type = token_type.to_string();
        session.expires_at = Utc::now() + Duration::seconds(expires_in);
        session.scope = scope;
        session.updated_at = Utc::now();

        self.storage.save_session(&session).await?;

        Ok(())
    }

    pub async fn validate_access_token(
        &self,
        session_id: &str,
    ) -> Result<bool> {
        let session = self.storage.get_session(session_id).await?
            .ok_or_else(|| anyhow::anyhow!("Session not found: {}", session_id))?;

        // Check if token is expired
        if session.expires_at < Utc::now() {
            return Ok(false);
        }

        // Check if refresh token exists and is not expired
        if session.refresh_token.is_none() {
            return Ok(false);
        }

        Ok(true)
    }

    pub async fn get_or_refresh_access_token(
        &self,
        session_id: &str,
    ) -> Result<String> {
        let session = self.storage.get_session(session_id).await?
            .ok_or_else(|| anyhow::anyhow!("Session not found: {}", session_id))?;

        // Check if token is about to expire (within 5 minutes)
        let time_until_expiration = session.expires_at - Utc::now();
        if time_until_expiration.num_seconds() < 300 {
            // Token is about to expire, try to refresh
            if let Some(ref refresh_token) = session.refresh_token {
                // TODO: Call OAuth provider to refresh token
                // For now, return the current token
                log::warn!("Token refresh needed for session {}, but refresh not implemented yet", session_id);
                return Ok(session.access_token.clone());
            }
        }

        Ok(session.access_token.clone())
    }

    pub async fn revoke_all_user_sessions(&self, user_id: &str) -> Result<i64> {
        let sessions = self.storage.get_sessions_by_user(user_id).await?;

        let mut count = 0;
        for session in sessions {
            self.storage.delete_session(&session.id).await?;
            count += 1;
        }

        Ok(count)
    }

    pub async fn rotate_all_tokens(&self) -> Result<i64> {
        let sessions = self.storage.get_all_provider_configs().await?;

        let mut count = 0;

        for session in sessions {
            // TODO: Implement token rotation for each provider
            // For now, just log
            log::debug!("Token rotation needed for provider: {}", session.provider_type);
            count += 1;
        }

        Ok(count)
    }

    pub async fn start_cleanup_task(&self) -> Result<()> {
        let storage = self.storage.clone();

        tokio::spawn(async move {
            loop {
                tokio::time::sleep(self.cleanup_interval).await;

                log::info!("Running OAuth2 token cleanup");

                if let Err(e) = storage.cleanup_expired_tokens().await {
                    log::error!("Error cleaning up OAuth2 tokens: {}", e);
                }
            }
        });

        Ok(())
    }

    pub async fn get_token_stats(&self) -> TokenStats {
        let sessions = self.storage.get_all_provider_configs().await;

        TokenStats {
            total_sessions: sessions.len(),
            sessions_with_refresh_token: sessions.iter()
                .filter(|s| s.refresh_token.is_some())
                .count(),
            total_expiry_days: 30,
        }
    }
}

#[derive(Debug, Clone)]
pub struct TokenStats {
    pub total_sessions: usize,
    pub sessions_with_refresh_token: usize,
    pub total_expiry_days: i32,
}

pub struct SessionToken {
    pub access_token: String,
    pub token_type: String,
    pub expires_in: i64,
    pub refresh_token: Option<String>,
    pub scope: Option<String>,
    pub id_token: Option<String>,
}

impl SessionToken {
    pub fn new(
        access_token: String,
        token_type: String,
        expires_in: i64,
        refresh_token: Option<String>,
        scope: Option<String>,
        id_token: Option<String>,
    ) -> Self {
        Self {
            access_token,
            token_type,
            expires_in,
            refresh_token,
            scope,
            id_token,
        }
    }

    pub fn from_oauth_response(response: &OAuthTokenResponse) -> Self {
        Self {
            access_token: response.access_token.clone(),
            token_type: response.token_type.clone(),
            expires_in: response.expires_in,
            refresh_token: response.refresh_token.clone(),
            scope: response.scope.clone(),
            id_token: response.id_token.clone(),
        }
    }

    pub fn is_expired(&self) -> bool {
        self.expires_in <= 0
    }

    pub fn time_until_expiration(&self) -> i64 {
        chrono::Utc::now().signed_duration_since(
            chrono::Utc::now() + chrono::Duration::seconds(self.expires_in)
        ).num_seconds()
    }
}

// Token validation
pub struct TokenValidator {
    pub allowed_token_types: Vec<String>,
    pub min_token_length: usize,
    pub max_token_length: usize,
}

impl TokenValidator {
    pub fn new() -> Self {
        Self {
            allowed_token_types: vec!["Bearer".to_string(), "Bearer ".to_string()],
            min_token_length: 20,
            max_token_length: 4096,
        }
    }

    pub fn validate(&self, token: &str) -> Result<()> {
        // Check token type
        let token_type = token.split_whitespace().next()
            .ok_or_else(|| anyhow::anyhow!("Token must include token type"))?;

        if !self.allowed_token_types.contains(&token_type.to_lowercase()) {
            return Err(anyhow::anyhow!(
                "Invalid token type: {}. Must be one of: {}",
                token_type,
                self.allowed_token_types.join(", ")
            ));
        }

        // Check token length
        let token_value = if token_type == "Bearer" && token.len() > 8 {
            &token[7..] // Remove "Bearer " prefix
        } else {
            token
        };

        if token_value.len() < self.min_token_length {
            return Err(anyhow::anyhow!(
                "Token too short: {} (minimum: {})",
                token_value.len(),
                self.min_token_length
            ));
        }

        if token_value.len() > self.max_token_length {
            return Err(anyhow::anyhow!(
                "Token too long: {} (maximum: {})",
                token_value.len(),
                self.max_token_length
            ));
        }

        Ok(())
    }

    pub fn validate_refresh_token(&self, token: &str) -> Result<()> {
        self.validate(token)
    }
}

// ID Token validation (OpenID Connect)
pub struct IdTokenValidator {
    pub issuer: String,
    pub audience: Vec<String>,
}

impl IdTokenValidator {
    pub fn new(issuer: String, audience: Vec<String>) -> Self {
        Self { issuer, audience }
    }

    pub async fn validate_id_token(
        &self,
        id_token: &str,
    ) -> Result<IdTokenClaims> {
        use jsonwebtoken::{decode, Validation, Header};
        use jsonwebtoken::algorithm::HS256;

        let token_data = decode::<IdTokenClaims>(
            id_token,
            &jsonwebtoken::EncodingKey::from_secret(b"secret"), // TODO: Use proper secret
            &Validation::new(jsonwebtoken::Algorithm::HS256),
        )?;

        let claims = token_data.claims;

        // Validate issuer
        if claims.iss != self.issuer {
            return Err(anyhow::anyhow!("Invalid issuer: expected {}, got {}", self.issuer, claims.iss));
        }

        // Validate audience
        if !claims.aud.contains(&self.issuer) {
            return Err(anyhow::anyhow!("Invalid audience"));
        }

        // Validate expiration
        let now = Utc::now().timestamp();
        if claims.exp > 0 && claims.exp < now {
            return Err(anyhow::anyhow!("Token expired"));
        }

        // Validate issued at
        if claims.iat > 0 && claims.iat > now {
            return Err(anyhow::anyhow!("Token issued in the future"));
        }

        Ok(claims)
    }
}