use anyhow::Result;
use chrono::{DateTime, Utc};
use sqlx::postgres::PgPool;
use std::sync::Arc;
use tokio::sync::RwLock;

use super::*;

pub struct OAuthStorage {
    pub pool: Arc<PgPool>,
}

impl OAuthStorage {
    pub fn new(pool: Arc<PgPool>) -> Self {
        Self { pool }
    }

    pub async fn create_tables(&self) -> Result<()> {
        let create_tables_sql = r#"
            CREATE TABLE IF NOT EXISTS oauth2_state_tokens (
                id SERIAL PRIMARY KEY,
                state TEXT UNIQUE NOT NULL,
                token TEXT UNIQUE NOT NULL,
                provider TEXT NOT NULL,
                user_id TEXT,
                created_at TIMESTAMP NOT NULL DEFAULT NOW(),
                expires_at TIMESTAMP NOT NULL,
                used BOOLEAN DEFAULT FALSE
            );

            CREATE INDEX IF NOT EXISTS idx_oauth2_state_tokens_state ON oauth2_state_tokens(state);
            CREATE INDEX IF NOT EXISTS idx_oauth2_state_tokens_token ON oauth2_state_tokens(token);
            CREATE INDEX IF NOT EXISTS idx_oauth2_state_tokens_provider ON oauth2_state_tokens(provider);
            CREATE INDEX IF NOT EXISTS idx_oauth2_state_tokens_user_id ON oauth2_state_tokens(user_id);
            CREATE INDEX IF NOT EXISTS idx_oauth2_state_tokens_expires_at ON oauth2_state_tokens(expires_at);

            CREATE TABLE IF NOT EXISTS oauth2_sessions (
                id TEXT PRIMARY KEY,
                user_id TEXT NOT NULL,
                provider TEXT NOT NULL,
                access_token TEXT NOT NULL,
                refresh_token TEXT,
                token_type TEXT NOT NULL,
                expires_at TIMESTAMP NOT NULL,
                scope TEXT,
                created_at TIMESTAMP NOT NULL DEFAULT NOW(),
                updated_at TIMESTAMP NOT NULL DEFAULT NOW(),
                metadata JSONB
            );

            CREATE INDEX IF NOT EXISTS idx_oauth2_sessions_user_id ON oauth2_sessions(user_id);
            CREATE INDEX IF NOT EXISTS idx_oauth2_sessions_provider ON oauth2_sessions(provider);
            CREATE INDEX IF NOT EXISTS idx_oauth2_sessions_expires_at ON oauth2_sessions(expires_at);

            CREATE TABLE IF NOT EXISTS oauth2_provider_configs (
                provider TEXT PRIMARY KEY,
                client_id TEXT NOT NULL,
                client_secret TEXT NOT NULL,
                redirect_uri TEXT NOT NULL,
                created_at TIMESTAMP NOT NULL DEFAULT NOW(),
                updated_at TIMESTAMP NOT NULL DEFAULT NOW()
            );
        "#;

        sqlx::query(create_tables_sql)
            .execute(self.pool.clone())
            .await?;

        log::info!("OAuth2 storage tables created successfully");
        Ok(())
    }

    pub async fn save_state_token(
        &self,
        provider: &str,
        state: &str,
        token: &str,
    ) -> Result<()> {
        let expires_at = Utc::now() + Duration::seconds(300); // 5 minutes

        let insert_sql = r#"
            INSERT INTO oauth2_state_tokens (state, token, provider, expires_at)
            VALUES ($1, $2, $3, $4)
            ON CONFLICT (state) DO UPDATE SET
                token = EXCLUDED.token,
                expires_at = EXCLUDED.expires_at,
                used = FALSE
        "#;

        sqlx::query(insert_sql)
            .bind(state)
            .bind(token)
            .bind(provider)
            .bind(expires_at)
            .execute(self.pool.clone())
            .await?;

        Ok(())
    }

    pub async fn get_state_token(&self, state: &str) -> Result<Option<StateToken>> {
        let query = r#"
            SELECT id, state, token, provider, user_id, created_at, expires_at, used
            FROM oauth2_state_tokens
            WHERE state = $1
        "#;

        let row = sqlx::query(query)
            .bind(state)
            .fetch_optional(self.pool.clone())
            .await?;

        match row {
            Some(row) => {
                Ok(Some(StateToken {
                    id: row.try_get("id")?,
                    state: row.try_get("state")?,
                    token: row.try_get("token")?,
                    provider: row.try_get("provider")?,
                    user_id: row.try_get("user_id")?,
                    created_at: row.try_get("created_at")?,
                    expires_at: row.try_get("expires_at")?,
                    used: row.try_get("used")?,
                }))
            }
            None => Ok(None),
        }
    }

    pub async fn verify_state_token(&self, provider: &str, token: &str) -> Result<Option<String>> {
        let query = r#"
            SELECT id, state, token, provider, user_id, created_at, expires_at, used
            FROM oauth2_state_tokens
            WHERE token = $1 AND provider = $2 AND expires_at > NOW()
        "#;

        let row = sqlx::query(query)
            .bind(token)
            .bind(provider)
            .fetch_optional(self.pool.clone())
            .await?;

        match row {
            Some(row) => {
                let state: String = row.try_get("state")?;
                Ok(Some(state))
            }
            None => Ok(None),
        }
    }

    pub async fn mark_state_token_used(&self, token: &str) -> Result<()> {
        let update_sql = r#"
            UPDATE oauth2_state_tokens
            SET used = TRUE
            WHERE token = $1
        "#;

        sqlx::query(update_sql)
            .bind(token)
            .execute(self.pool.clone())
            .await?;

        Ok(())
    }

    pub async fn cleanup_expired_state_tokens(&self) -> Result<i64> {
        let query = r#"
            DELETE FROM oauth2_state_tokens
            WHERE expires_at < NOW()
        "#;

        let result = sqlx::query(query)
            .execute(self.pool.clone())
            .await?;

        let deleted_count = result.rows_affected();
        log::info!("Cleaned up {} expired OAuth2 state tokens", deleted_count);

        Ok(deleted_count)
    }

    pub async fn save_session(&self, session: &OAuthSession) -> Result<()> {
        let insert_sql = r#"
            INSERT INTO oauth2_sessions (
                id, user_id, provider, access_token, refresh_token,
                token_type, expires_at, scope, metadata
            )
            VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9)
            ON CONFLICT (id) DO UPDATE SET
                user_id = EXCLUDED.user_id,
                provider = EXCLUDED.provider,
                access_token = EXCLUDED.access_token,
                refresh_token = EXCLUDED.refresh_token,
                token_type = EXCLUDED.token_type,
                expires_at = EXCLUDED.expires_at,
                scope = EXCLUDED.scope,
                metadata = EXCLUDED.metadata,
                updated_at = NOW()
        "#;

        let metadata_json = serde_json::to_value(&session.metadata).unwrap_or(serde_json::Value::Null);

        sqlx::query(insert_sql)
            .bind(&session.id)
            .bind(&session.user_id)
            .bind(&session.provider)
            .bind(&session.access_token)
            .bind(&session.refresh_token)
            .bind(&session.token_type)
            .bind(session.expires_at)
            .bind(&session.scope)
            .bind(&metadata_json)
            .execute(self.pool.clone())
            .await?;

        Ok(())
    }

    pub async fn get_session(&self, session_id: &str) -> Result<Option<OAuthSession>> {
        let query = r#"
            SELECT id, user_id, provider, access_token, refresh_token,
                   token_type, expires_at, scope, created_at, updated_at, metadata
            FROM oauth2_sessions
            WHERE id = $1
        "#;

        let row = sqlx::query(query)
            .bind(session_id)
            .fetch_optional(self.pool.clone())
            .await?;

        match row {
            Some(row) => {
                let metadata_json: serde_json::Value = row.try_get("metadata")?;
                let metadata: std::collections::HashMap<String, serde_json::Value> =
                    serde_json::from_value(metadata_json)?;

                Ok(Some(OAuthSession {
                    id: row.try_get("id")?,
                    user_id: row.try_get("user_id")?,
                    provider: row.try_get("provider")?,
                    access_token: row.try_get("access_token")?,
                    refresh_token: row.try_get("refresh_token")?,
                    token_type: row.try_get("token_type")?,
                    expires_at: row.try_get("expires_at")?,
                    scope: row.try_get("scope")?,
                    created_at: row.try_get("created_at")?,
                    updated_at: row.try_get("updated_at")?,
                    metadata,
                }))
            }
            None => Ok(None),
        }
    }

    pub async fn get_sessions_by_user(&self, user_id: &str) -> Result<Vec<OAuthSession>> {
        let query = r#"
            SELECT id, user_id, provider, access_token, refresh_token,
                   token_type, expires_at, scope, created_at, updated_at, metadata
            FROM oauth2_sessions
            WHERE user_id = $1
            ORDER BY created_at DESC
        "#;

        let rows = sqlx::query(query)
            .bind(user_id)
            .fetch_all(self.pool.clone())
            .await?;

        let mut sessions = Vec::new();

        for row in rows {
            let metadata_json: serde_json::Value = row.try_get("metadata")?;
            let metadata: std::collections::HashMap<String, serde_json::Value> =
                serde_json::from_value(metadata_json)?;

            sessions.push(OAuthSession {
                id: row.try_get("id")?,
                user_id: row.try_get("user_id")?,
                provider: row.try_get("provider")?,
                access_token: row.try_get("access_token")?,
                refresh_token: row.try_get("refresh_token")?,
                token_type: row.try_get("token_type")?,
                expires_at: row.try_get("expires_at")?,
                scope: row.try_get("scope")?,
                created_at: row.try_get("created_at")?,
                updated_at: row.try_get("updated_at")?,
                metadata,
            });
        }

        Ok(sessions)
    }

    pub async fn delete_session(&self, session_id: &str) -> Result<()> {
        let query = "DELETE FROM oauth2_sessions WHERE id = $1";

        sqlx::query(query)
            .bind(session_id)
            .execute(self.pool.clone())
            .await?;

        Ok(())
    }

    pub async fn delete_sessions_by_user(&self, user_id: &str) -> Result<()> {
        let query = "DELETE FROM oauth2_sessions WHERE user_id = $1";

        sqlx::query(query)
            .bind(user_id)
            .execute(self.pool.clone())
            .await?;

        Ok(())
    }

    pub async fn cleanup_expired_sessions(&self) -> Result<i64> {
        let query = r#"
            DELETE FROM oauth2_sessions
            WHERE expires_at < NOW()
        "#;

        let result = sqlx::query(query)
            .execute(self.pool.clone())
            .await?;

        let deleted_count = result.rows_affected();
        log::info!("Cleaned up {} expired OAuth2 sessions", deleted_count);

        Ok(deleted_count)
    }

    pub async fn update_provider_config(&self, provider: &str, client_id: &str, client_secret: &str, redirect_uri: &str) -> Result<()> {
        let query = r#"
            INSERT INTO oauth2_provider_configs (provider, client_id, client_secret, redirect_uri)
            VALUES ($1, $2, $3, $4)
            ON CONFLICT (provider) DO UPDATE SET
                client_id = EXCLUDED.client_id,
                client_secret = EXCLUDED.client_secret,
                redirect_uri = EXCLUDED.redirect_uri,
                updated_at = NOW()
        "#;

        sqlx::query(query)
            .bind(provider)
            .bind(client_id)
            .bind(client_secret)
            .bind(redirect_uri)
            .execute(self.pool.clone())
            .await?;

        Ok(())
    }

    pub async fn get_provider_config(&self, provider: &str) -> Result<OAuthProviderConfig> {
        let query = r#"
            SELECT client_id, client_secret, redirect_uri
            FROM oauth2_provider_configs
            WHERE provider = $1
        "#;

        let row = sqlx::query(query)
            .bind(provider)
            .fetch_optional(self.pool.clone())
            .await?;

        match row {
            Some(row) => {
                Ok(OAuthProviderConfig {
                    provider_type: provider.to_string(),
                    client_id: row.try_get("client_id")?,
                    client_secret: row.try_get("client_secret")?,
                    redirect_uri: row.try_get("redirect_uri")?,
                    scope: vec![],
                    state: "".to_string(),
                    code_verifier: "".to_string(),
                    auto_approve: false,
                })
            }
            None => Err(anyhow::anyhow!("Provider configuration not found: {}", provider)),
        }
    }

    pub async fn get_all_provider_configs(&self) -> Result<Vec<OAuthProviderConfig>> {
        let query = r#"
            SELECT provider, client_id, client_secret, redirect_uri
            FROM oauth2_provider_configs
        "#;

        let rows = sqlx::query(query)
            .fetch_all(self.pool.clone())
            .await?;

        let mut configs = Vec::new();

        for row in rows {
            configs.push(OAuthProviderConfig {
                provider_type: row.try_get("provider")?,
                client_id: row.try_get("client_id")?,
                client_secret: row.try_get("client_secret")?,
                redirect_uri: row.try_get("redirect_uri")?,
                scope: vec![],
                state: "".to_string(),
                code_verifier: "".to_string(),
                auto_approve: false,
            });
        }

        Ok(configs)
    }

    pub async fn cleanup_expired_tokens(&self) -> Result<i64> {
        let sessions_deleted = self.cleanup_expired_sessions().await?;

        let states_deleted = self.cleanup_expired_state_tokens().await?;

        Ok(sessions_deleted + states_deleted)
    }
}

// In-memory storage for testing
pub struct InMemoryOAuthStorage {
    pub state_tokens: Arc<RwLock<HashMap<String, StateToken>>>,
    pub sessions: Arc<RwLock<HashMap<String, OAuthSession>>>,
    pub provider_configs: Arc<RwLock<HashMap<String, OAuthProviderConfig>>>,
}

impl InMemoryOAuthStorage {
    pub fn new() -> Self {
        Self {
            state_tokens: Arc::new(RwLock::new(HashMap::new())),
            sessions: Arc::new(RwLock::new(HashMap::new())),
            provider_configs: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    pub async fn save_state_token(
        &self,
        provider: &str,
        state: &str,
        token: &str,
    ) -> Result<()> {
        let expires_at = Utc::now() + chrono::Duration::seconds(300);

        self.state_tokens.write().await.insert(
            state.to_string(),
            StateToken {
                id: uuid::Uuid::new_v4().to_string(),
                state: state.to_string(),
                token: token.to_string(),
                provider: provider.to_string(),
                user_id: None,
                created_at: Utc::now(),
                expires_at,
                used: false,
            },
        );

        Ok(())
    }

    pub async fn get_state_token(&self, state: &str) -> Result<Option<StateToken>> {
        Ok(self.state_tokens.read().await.get(state).cloned())
    }

    pub async fn verify_state_token(&self, provider: &str, token: &str) -> Result<Option<String>> {
        let state_tokens = self.state_tokens.read().await;

        if let Some(state_token) = state_tokens.values().find(|st| {
            st.token == token && st.provider == provider && st.expires_at > Utc::now()
        }) {
            Ok(Some(state_token.state.clone()))
        } else {
            Ok(None)
        }
    }

    pub async fn mark_state_token_used(&self, token: &str) -> Result<()> {
        let mut state_tokens = self.state_tokens.write().await;

        if let Some(state_token) = state_tokens.values_mut().find(|st| st.token == token) {
            state_token.used = true;
        }

        Ok(())
    }

    pub async fn save_session(&self, session: &OAuthSession) -> Result<()> {
        self.sessions.write().await.insert(session.id.clone(), session.clone());
        Ok(())
    }

    pub async fn get_session(&self, session_id: &str) -> Result<Option<OAuthSession>> {
        Ok(self.sessions.read().await.get(session_id).cloned())
    }

    pub async fn delete_session(&self, session_id: &str) -> Result<()> {
        self.sessions.write().await.remove(session_id);
        Ok(())
    }

    pub async fn cleanup_expired_tokens(&self) -> Result<i64> {
        let mut count = 0;

        let mut state_tokens = self.state_tokens.write().await;
        state_tokens.retain(|_, st| st.expires_at > Utc::now());
        count += state_tokens.len();

        let mut sessions = self.sessions.write().await;
        sessions.retain(|_, s| s.expires_at > Utc::now());
        count += sessions.len();

        Ok(count as i64)
    }
}