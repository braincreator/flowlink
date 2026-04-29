//! FlowLink authentication crate — JWT tokens, OAuth, SAML, 2FA, email auth, rate limiting.
//!
//! This crate provides all authentication-related functionality extracted from the relay:
//! - `AuthEngine` — JWT creation/validation, session management, OAuth flows
//! - `AuthManager` — API token validation (legacy, DashMap-based)
//! - OAuth callbacks (VK, Yandex, GitHub)
//! - SAML 2.0 SP integration
//! - TOTP 2FA endpoints
//! - Email magic-link / code authentication
//! - Auth rate limiting (sliding-window brute-force protection)

pub mod rate_limiter;
pub mod rate_middleware;

pub mod email;
pub mod oauth;
pub mod saml;
pub mod two_factor;

// Re-export sub-module types
pub use email::*;
pub use oauth::*;
pub use rate_limiter::AuthRateLimiter;
pub use saml::SamlConfig;

// =========================================================================
// Core types from auth engine (originally in relay/src/auth.rs)
// =========================================================================

use anyhow::Result;
use chrono::{DateTime, Duration, Utc};
use dashmap::DashMap;
use jsonwebtoken::{DecodingKey, EncodingKey, Header, Validation, decode, encode};
use serde::{Deserialize, Serialize};
use sqlx::PgPool;
use std::sync::Arc;
use uuid::Uuid;

// ── Client (API token auth) ──

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Client {
    pub client_id: String,
    pub api_token: String,
    pub name: String,
    pub active: bool,
}

/// Auth manager for API token validation.
/// Tokens cached in-memory (DashMap) with optional PostgreSQL backing.
pub struct AuthManager {
    clients: Arc<DashMap<String, Client>>,
    token_hash_to_client: Arc<DashMap<String, String>>,
    db: Option<Arc<sqlx::PgPool>>,
}

fn hash_token(token: &str) -> String {
    use sha2::Digest;
    let mut hasher = sha2::Sha256::new();
    hasher.update(token.as_bytes());
    hex::encode(hasher.finalize())
}

impl Default for AuthManager {
    fn default() -> Self {
        Self::new(None)
    }
}

impl AuthManager {
    pub fn new(db: Option<Arc<sqlx::PgPool>>) -> Self {
        let mgr = Self {
            clients: Arc::new(DashMap::new()),
            token_hash_to_client: Arc::new(DashMap::new()),
            db: db.clone(),
        };
        if let Some(ref pool) = db {
            let rt = tokio::runtime::Handle::current();
            let pool = pool.clone();
            let clients = mgr.clients.clone();
            let token_map = mgr.token_hash_to_client.clone();
            rt.spawn_blocking(move || {
                let rt = match tokio::runtime::Runtime::new() {
                    Ok(r) => r,
                    Err(e) => {
                        log::error!("Failed to create runtime for DB token load: {e}");
                        return;
                    }
                };
                rt.block_on(async move {
                    if let Ok(rows) = sqlx::query_as::<_, (String, String, String, bool)>(
                        "SELECT agent_id, COALESCE(api_token, ''), name, (status = 'connected') FROM agents WHERE api_token IS NOT NULL"
                    )
                    .fetch_all(pool.as_ref())
                    .await
                    {
                        for (agent_id, api_token, name, active) in &rows {
                            if !api_token.is_empty() {
                                let token_hash = hash_token(api_token);
                                let client = Client {
                                    client_id: agent_id.clone(),
                                    api_token: String::new(),
                                    name: name.clone(),
                                    active: *active,
                                };
                                token_map.insert(token_hash, agent_id.clone());
                                clients.insert(agent_id.clone(), client);
                            }
                        }
                        let count = rows.iter().filter(|(_, t, _, _)| !t.is_empty()).count();
                        if count > 0 {
                            log::info!("AuthManager: loaded {} agent tokens from DB", count);
                        }
                    }
                });
            });
        }
        mgr
    }

    pub fn register_client(&self, client: Client) {
        let token = client.api_token.clone();
        let token_hash = hash_token(&token);
        let id = client.client_id.clone();
        if let Some(_old_client) = self.clients.get(&id) {
            self.token_hash_to_client.retain(|_, v| v != &id);
        }
        self.token_hash_to_client.insert(token_hash, id.clone());
        let mut safe_client = client.clone();
        safe_client.api_token = String::new();
        self.clients.insert(id.clone(), safe_client);

        if let Some(ref pool) = self.db {
            let pool = pool.clone();
            let cid = id.clone();
            let name = client.name.clone();
            let api_token = token.clone();
            use std::collections::hash_map::DefaultHasher;
            use std::hash::{Hash, Hasher};
            let mut hasher = DefaultHasher::new();
            api_token.hash(&mut hasher);
            let token_hash = format!("{:016x}", hasher.finish());
            tokio::spawn(async move {
                let _ = sqlx::query(
                    "INSERT INTO agents (agent_id, api_token, api_token_hash, name, status) VALUES ($1, $2, $3, $4, 'connected') \
                     ON CONFLICT (agent_id) DO UPDATE SET api_token = EXCLUDED.api_token, api_token_hash = EXCLUDED.api_token_hash, name = EXCLUDED.name, status = 'connected'"
                )
                .bind(&cid)
                .bind(&api_token)
                .bind(&token_hash)
                .bind(&name)
                .execute(pool.as_ref())
                .await;
            });
        }
    }

    pub fn is_empty(&self) -> bool {
        self.clients.is_empty()
    }

    pub fn validate_token(&self, token: &str) -> Option<Client> {
        let token_hash = hash_token(token);
        let client_id = self.token_hash_to_client.get(&token_hash)?;
        let id: String = client_id.value().clone();
        self.clients.get(&id).map(|c| c.value().clone())
    }

    pub fn get_client(&self, client_id: &str) -> Option<Client> {
        self.clients.get(client_id).map(|c| c.value().clone())
    }
}

// ── JWT Claims ──

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct Claims {
    pub sub: String,
    pub account_id: String,
    pub email: Option<String>,
    pub name: Option<String>,
    pub avatar_url: Option<String>,
    pub is_admin: bool,
    pub org_id: Option<String>,
    pub iat: i64,
    pub exp: i64,
}

// ── Auth config ──

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AuthConfig {
    pub jwt_secret: String,
    pub access_token_ttl_min: i64,
    pub refresh_token_ttl_days: i64,
    pub vk: Option<flowlink_core::config::OAuthProviderConfig>,
    pub yandex: Option<flowlink_core::config::OAuthProviderConfig>,
    pub github: Option<flowlink_core::config::OAuthProviderConfig>,
}

impl Default for AuthConfig {
    fn default() -> Self {
        Self {
            jwt_secret: String::new(),
            access_token_ttl_min: 15,
            refresh_token_ttl_days: 30,
            vk: None,
            yandex: None,
            github: None,
        }
    }
}

// ── JWT token pair ──

#[derive(Debug, Serialize)]
pub struct TokenPair {
    pub access_token: String,
    pub refresh_token: String,
    pub expires_in: i64,
    pub session_id: String,
}

// ── Session info ──

#[derive(Debug, Clone, Serialize)]
pub struct SessionInfo {
    pub session_id: String,
    pub account_id: String,
    pub ip: Option<String>,
    pub user_agent: Option<String>,
    pub created_at: i64,
    pub last_seen: i64,
    pub email: Option<String>,
    pub name: Option<String>,
}

// ── AuthEngine ──

pub struct AuthEngine {
    config: AuthConfig,
    db: PgPool,
    pkce_store: Arc<DashMap<String, String>>,
    token_blacklist: Arc<DashMap<String, std::time::Instant>>,
    sessions: Arc<DashMap<String, Vec<SessionInfo>>>,
}

impl AuthEngine {
    pub fn new(config: AuthConfig, db: PgPool) -> Self {
        Self {
            config,
            db,
            pkce_store: Arc::new(DashMap::new()),
            token_blacklist: Arc::new(DashMap::new()),
            sessions: Arc::new(DashMap::new()),
        }
    }

    pub fn create_tokens(
        &self,
        user_id: &str,
        account_id: &str,
        email: Option<&str>,
        name: Option<&str>,
        avatar_url: Option<&str>,
        is_admin: bool,
        org_id: Option<&str>,
    ) -> Result<TokenPair> {
        let now = Utc::now();
        let access_exp = now + Duration::minutes(self.config.access_token_ttl_min);
        let refresh_exp = now + Duration::days(self.config.refresh_token_ttl_days);

        let access_claims = Claims {
            sub: user_id.to_string(),
            account_id: account_id.to_string(),
            email: email.map(|s| s.to_string()),
            name: name.map(|s| s.to_string()),
            avatar_url: avatar_url.map(|s| s.to_string()),
            is_admin,
            org_id: org_id.map(|s| s.to_string()),
            iat: now.timestamp(),
            exp: access_exp.timestamp(),
        };

        let access_token = encode(
            &Header::default(),
            &access_claims,
            &EncodingKey::from_secret(self.config.jwt_secret.as_bytes()),
        )?;

        let refresh_claims = Claims {
            sub: user_id.to_string(),
            account_id: account_id.to_string(),
            email: None,
            name: None,
            avatar_url: None,
            is_admin,
            org_id: org_id.map(|s| s.to_string()),
            iat: now.timestamp(),
            exp: refresh_exp.timestamp(),
        };

        let refresh_token = encode(
            &Header::default(),
            &refresh_claims,
            &EncodingKey::from_secret(self.config.jwt_secret.as_bytes()),
        )?;

        let session_id = Uuid::new_v4().to_string();
        Ok(TokenPair {
            access_token,
            refresh_token,
            expires_in: self.config.access_token_ttl_min * 60,
            session_id,
        })
    }

    pub fn create_org_tokens(
        &self,
        user_id: &str,
        account_id: &str,
        email: Option<&str>,
        name: Option<&str>,
        avatar_url: Option<&str>,
        is_admin: bool,
        org_id: &str,
        _role: &str,
    ) -> Result<TokenPair> {
        self.create_tokens(user_id, account_id, email, name, avatar_url, is_admin, Some(org_id))
    }

    pub fn validate_access_token(&self, token: &str) -> Result<Claims> {
        if self.token_blacklist.contains_key(token) {
            return Err(anyhow::anyhow!("Token has been revoked"));
        }
        let token_data = decode::<Claims>(
            token,
            &DecodingKey::from_secret(self.config.jwt_secret.as_bytes()),
            &Validation::default(),
        )?;
        Ok(token_data.claims)
    }

    pub fn validate_refresh_token(&self, token: &str) -> Result<Claims> {
        if self.token_blacklist.contains_key(token) {
            return Err(anyhow::anyhow!("Token has been revoked"));
        }
        let token_data = decode::<Claims>(
            token,
            &DecodingKey::from_secret(self.config.jwt_secret.as_bytes()),
            &Validation::default(),
        )?;
        Ok(token_data.claims)
    }

    pub fn blacklist_token(&self, token: &str) {
        self.token_blacklist.insert(
            token.to_string(),
            std::time::Instant::now() + std::time::Duration::from_secs(30 * 24 * 3600),
        );
        if self.token_blacklist.len() > 10000 {
            let now = std::time::Instant::now();
            self.token_blacklist.retain(|_, exp| *exp > now);
        }
    }

    // ── Session management ──

    pub fn create_session(
        &self,
        account_id: &str,
        session_id: &str,
        ip: Option<&str>,
        user_agent: Option<&str>,
        email: Option<&str>,
        name: Option<&str>,
    ) {
        let now = Utc::now().timestamp();
        let session = SessionInfo {
            session_id: session_id.to_string(),
            account_id: account_id.to_string(),
            ip: ip.map(|s| s.to_string()),
            user_agent: user_agent.map(|s| s.to_string()),
            created_at: now,
            last_seen: now,
            email: email.map(|s| s.to_string()),
            name: name.map(|s| s.to_string()),
        };
        let mut sessions = self.sessions.entry(account_id.to_string()).or_default();
        sessions.retain(|s| s.session_id != session_id);
        sessions.push(session);
        sessions.sort_by(|a, b| b.last_seen.cmp(&a.last_seen));
        sessions.truncate(20);
    }

    pub fn list_sessions(&self, account_id: &str) -> Vec<SessionInfo> {
        self.sessions
            .get(account_id)
            .map(|s| s.clone())
            .unwrap_or_default()
    }

    pub async fn find_or_create_by_email(
        &self,
        email: &str,
        name: &str,
        groups: &[String],
    ) -> Result<(String, bool)> {
        let row = sqlx::query_as::<_, (String, bool)>(
            "SELECT account_id, is_admin FROM accounts WHERE email = $1 AND active = true",
        )
        .bind(email)
        .fetch_optional(&self.db)
        .await
        .map_err(|e| anyhow::anyhow!("DB error: {e}"))?;

        if let Some((account_id, is_admin)) = row {
            return Ok((account_id, is_admin));
        }

        let account_id = uuid::Uuid::new_v4().to_string();
        let is_admin = groups.iter().any(|g| g == "admin" || g == "Administrators");

        sqlx::query(
            "INSERT INTO accounts (account_id, email, name, is_admin, active, plan_id, created_at) VALUES ($1, $2, $3, $4, true, 'free', NOW())",
        )
        .bind(&account_id)
        .bind(email)
        .bind(name)
        .bind(is_admin)
        .execute(&self.db)
        .await
        .map_err(|e| anyhow::anyhow!("Account create error: {e}"))?;

        log::info!("SAML: created account {} for {}", account_id, email);
        Ok((account_id, is_admin))
    }

    pub fn revoke_session(&self, account_id: &str, session_id: &str) -> bool {
        let mut sessions = match self.sessions.get_mut(account_id) {
            Some(s) => s,
            None => return false,
        };
        let before = sessions.len();
        sessions.retain(|s| s.session_id != session_id);
        sessions.len() < before
    }

    pub fn revoke_other_sessions(&self, account_id: &str, keep_session_id: &str) -> usize {
        let mut sessions = match self.sessions.get_mut(account_id) {
            Some(s) => s,
            None => return 0,
        };
        let before = sessions.len();
        sessions.retain(|s| s.session_id == keep_session_id);
        before - sessions.len()
    }

    // ── OAuth flows ──

    pub fn vk_auth_url(&self) -> Option<String> {
        let vk = self.config.vk.as_ref()?;
        let code_verifier: String = std::iter::repeat_with(|| {
            let b = rand::random::<u8>();
            match b % 3 {
                0 => char::from(b'A' + (b % 26)),
                1 => char::from(b'a' + (b % 26)),
                _ => char::from(b'0' + (b % 10)),
            }
        })
        .take(64)
        .collect();

        use sha2::{Digest, Sha256};
        let mut hasher = Sha256::new();
        hasher.update(code_verifier.as_bytes());
        let hash = hasher.finalize();
        let code_challenge = base64_encode_url_safe(&hash);

        let state: String = std::iter::repeat_with(|| {
            let b = rand::random::<u8>();
            match b % 3 {
                0 => char::from(b'A' + (b % 26)),
                1 => char::from(b'a' + (b % 26)),
                _ => char::from(b'0' + (b % 10)),
            }
        })
        .take(32)
        .collect();

        self.pkce_store.insert(state.clone(), code_verifier);

        Some(format!(
            "https://id.vk.ru/authorize?response_type=code&client_id={}&redirect_uri={}&state={}&code_challenge={}&code_challenge_method=S256&scope=email+phone",
            vk.client_id,
            urlencoding::encode(&vk.redirect_uri),
            state,
            code_challenge,
        ))
    }

    pub fn yandex_auth_url(&self, state: &str) -> Option<String> {
        let yandex = self.config.yandex.as_ref()?;
        Some(format!(
            "https://oauth.yandex.ru/authorize?client_id={}&response_type=code&redirect_uri={}&scope=login:email login:info&state={}",
            yandex.client_id,
            urlencoding::encode(&yandex.redirect_uri),
            state,
        ))
    }

    pub async fn vk_callback(&self, code: &str, state: &str) -> Result<OAuthUser> {
        let vk = self.config.vk.as_ref().ok_or_else(|| anyhow::anyhow!("VK OAuth not configured"))?;
        let code_verifier = self
            .pkce_store
            .get(state)
            .map(|v| v.value().clone())
            .ok_or_else(|| anyhow::anyhow!("VK: invalid or expired state/PKCE"))?;
        self.pkce_store.remove(state);

        let client = reqwest::Client::new();
        let token_resp: serde_json::Value = client
            .post("https://id.vk.ru/oauth2/auth")
            .form(&[
                ("grant_type", "authorization_code"),
                ("code", code),
                ("client_id", &vk.client_id),
                ("redirect_uri", &vk.redirect_uri),
                ("code_verifier", &code_verifier),
                ("state", state),
                ("device_id", "1"),
            ])
            .send()
            .await?
            .json()
            .await?;

        let access_token = token_resp["access_token"]
            .as_str()
            .ok_or_else(|| anyhow::anyhow!("VK: no access_token in response: {:?}", token_resp))?
            .to_string();
        let vk_user_id = token_resp["user_id"]
            .as_str()
            .ok_or_else(|| anyhow::anyhow!("VK: no user_id"))?;
        let email = token_resp["email"].as_str().map(|s| s.to_string());

        let info_resp: serde_json::Value = client
            .get("https://id.vk.ru/oauth2/user_info")
            .header("Authorization", format!("Bearer {}", access_token))
            .send()
            .await?
            .json()
            .await?;

        let user_data = &info_resp["user"];
        let first_name = user_data["first_name"].as_str().unwrap_or("");
        let last_name = user_data["last_name"].as_str().unwrap_or("");
        let avatar = user_data["avatar"].as_str().map(|s| s.to_string());

        let full_name = format!("{} {}", first_name, last_name).trim().to_string();

        let user = self
            .find_or_create_user(
                Some(format!("vk:{}", vk_user_id)),
                None,
                None,
                email.as_deref(),
                if full_name.is_empty() { None } else { Some(&full_name) },
                avatar.as_deref(),
            )
            .await?;

        Ok(user)
    }

    pub async fn yandex_callback(&self, code: &str) -> Result<OAuthUser> {
        let yandex = self.config.yandex.as_ref().ok_or_else(|| anyhow::anyhow!("Yandex OAuth not configured"))?;

        let client = reqwest::Client::new();
        let token_resp: serde_json::Value = client
            .post("https://oauth.yandex.ru/token")
            .form(&[
                ("grant_type", "authorization_code"),
                ("code", code),
                ("client_id", &yandex.client_id),
                ("client_secret", &yandex.client_secret),
            ])
            .send()
            .await?
            .json()
            .await?;

        let access_token = token_resp["access_token"]
            .as_str()
            .ok_or_else(|| anyhow::anyhow!("Yandex: no access_token"))?
            .to_string();

        let info_resp: serde_json::Value = client
            .get("https://login.yandex.ru/info?format=json")
            .header("Authorization", format!("OAuth {}", access_token))
            .send()
            .await?
            .json()
            .await?;

        let yandex_id = info_resp["id"].as_str().ok_or_else(|| anyhow::anyhow!("Yandex: no id"))?;
        let email = info_resp["default_email"].as_str().map(|s| s.to_string());
        let name = info_resp["real_name"]
            .as_str()
            .or_else(|| info_resp["display_name"].as_str());
        let avatar = info_resp["default_avatar_id"]
            .as_str()
            .map(|id| format!("https://avatars.yandex.net/get-yapic/{}/islands-200", id));

        let user = self
            .find_or_create_user(
                None,
                Some(format!("yandex:{}", yandex_id)),
                None,
                email.as_deref(),
                name,
                avatar.as_deref(),
            )
            .await?;

        Ok(user)
    }

    pub fn github_auth_url(&self) -> Option<String> {
        let gh = self.config.github.as_ref()?;
        let state: String = std::iter::repeat_with(|| {
            let b = rand::random::<u8>();
            match b % 3 {
                0 => char::from(b'A' + (b % 26)),
                1 => char::from(b'a' + (b % 26)),
                _ => char::from(b'0' + (b % 10)),
            }
        })
        .take(32)
        .collect();
        Some(format!(
            "https://github.com/login/oauth/authorize?client_id={}&redirect_uri={}&scope=user:email+read:user&state={}",
            gh.client_id,
            urlencoding::encode(&gh.redirect_uri),
            state,
        ))
    }

    pub async fn github_callback(&self, code: &str) -> Result<OAuthUser> {
        let gh = self.config.github.as_ref().ok_or_else(|| anyhow::anyhow!("GitHub OAuth not configured"))?;
        let client = reqwest::Client::new();

        let token_resp: serde_json::Value = client
            .post("https://github.com/login/oauth/access_token")
            .header("Accept", "application/json")
            .form(&[
                ("client_id", &gh.client_id),
                ("client_secret", &gh.client_secret),
                ("code", &code.to_string()),
            ])
            .send()
            .await?
            .json()
            .await?;

        let access_token = token_resp["access_token"]
            .as_str()
            .ok_or_else(|| anyhow::anyhow!("GitHub: no access_token: {:?}", token_resp))?
            .to_string();

        let user_resp: serde_json::Value = client
            .get("https://api.github.com/user")
            .header("Authorization", format!("Bearer {}", access_token))
            .header("User-Agent", "FlowLink")
            .send()
            .await?
            .json()
            .await?;

        let gh_id = user_resp["id"].as_u64().ok_or_else(|| anyhow::anyhow!("GitHub: no id"))?;
        let login = user_resp["login"].as_str().unwrap_or("");
        let name = user_resp["name"].as_str().unwrap_or(login);
        let avatar = user_resp["avatar_url"].as_str().map(|s| s.to_string());
        let email = user_resp["email"].as_str().map(|s| s.to_string());

        let email = if email.is_none() {
            let emails_resp: serde_json::Value = client
                .get("https://api.github.com/user/emails")
                .header("Authorization", format!("Bearer {}", access_token))
                .header("User-Agent", "FlowLink")
                .send()
                .await?
                .json()
                .await?;
            emails_resp
                .as_array()
                .and_then(|arr| arr.iter().find(|e| e["primary"].as_bool() == Some(true)))
                .and_then(|e| e["email"].as_str())
                .map(|s| s.to_string())
        } else {
            email
        };

        let user = self
            .find_or_create_user(
                None,
                None,
                Some(format!("github:{}", gh_id)),
                email.as_deref(),
                if name.is_empty() { None } else { Some(name) },
                avatar.as_deref(),
            )
            .await?;

        Ok(user)
    }

    async fn find_or_create_user(
        &self,
        vk_id: Option<String>,
        yandex_id: Option<String>,
        github_id: Option<String>,
        email: Option<&str>,
        name: Option<&str>,
        avatar_url: Option<&str>,
    ) -> Result<OAuthUser> {
        if let Some(ref vk) = vk_id {
            let row = sqlx::query_as::<_, UserRow>(
                "SELECT id, email, name, avatar_url, oauth_vk_id, oauth_yandex_id, oauth_github_id, account_id, created_at, last_login FROM users WHERE oauth_vk_id = $1",
            )
            .bind(vk)
            .fetch_optional(&self.db)
            .await?;
            if let Some(user) = row {
                sqlx::query("UPDATE users SET last_login = now() WHERE id = $1")
                    .bind(user.id)
                    .execute(&self.db)
                    .await?;
                return Ok(user.into());
            }
        }

        if let Some(ref yandex) = yandex_id {
            let row = sqlx::query_as::<_, UserRow>(
                "SELECT id, email, name, avatar_url, oauth_vk_id, oauth_yandex_id, oauth_github_id, account_id, created_at, last_login FROM users WHERE oauth_yandex_id = $1",
            )
            .bind(yandex)
            .fetch_optional(&self.db)
            .await?;
            if let Some(user) = row {
                sqlx::query("UPDATE users SET last_login = now() WHERE id = $1")
                    .bind(user.id)
                    .execute(&self.db)
                    .await?;
                return Ok(user.into());
            }
        }

        if let Some(ref gh) = github_id {
            let row = sqlx::query_as::<_, UserRow>(
                "SELECT id, email, name, avatar_url, oauth_vk_id, oauth_yandex_id, oauth_github_id, account_id, created_at, last_login FROM users WHERE oauth_github_id = $1",
            )
            .bind(gh)
            .fetch_optional(&self.db)
            .await?;
            if let Some(user) = row {
                sqlx::query("UPDATE users SET last_login = now() WHERE id = $1")
                    .bind(user.id)
                    .execute(&self.db)
                    .await?;
                return Ok(user.into());
            }
        }

        if let Some(em) = email {
            let row = sqlx::query_as::<_, UserRow>(
                "SELECT id, email, name, avatar_url, oauth_vk_id, oauth_yandex_id, account_id, created_at, last_login FROM users WHERE email = $1",
            )
            .bind(em)
            .fetch_optional(&self.db)
            .await?;

            if let Some(user) = row {
                if let Some(ref vk) = vk_id {
                    sqlx::query("UPDATE users SET oauth_vk_id = $1, last_login = now() WHERE id = $2 AND oauth_vk_id IS NULL")
                        .bind(vk)
                        .bind(user.id)
                        .execute(&self.db)
                        .await?;
                }
                if let Some(ref yandex) = yandex_id {
                    sqlx::query("UPDATE users SET oauth_yandex_id = $1, last_login = now() WHERE id = $2 AND oauth_yandex_id IS NULL")
                        .bind(yandex)
                        .bind(user.id)
                        .execute(&self.db)
                        .await?;
                }
                if let Some(ref gh) = github_id {
                    sqlx::query("UPDATE users SET oauth_github_id = $1, last_login = now() WHERE id = $2 AND oauth_github_id IS NULL")
                        .bind(gh)
                        .bind(user.id)
                        .execute(&self.db)
                        .await?;
                }
                return Ok(user.into());
            }
        }

        let user_id = Uuid::new_v4();
        let account_id = format!("user:{}", user_id);

        sqlx::query("INSERT INTO accounts (account_id, plan_id, active) VALUES ($1, 'trial', true)")
            .bind(&account_id)
            .execute(&self.db)
            .await?;

        sqlx::query(
            "INSERT INTO users (id, email, name, avatar_url, oauth_vk_id, oauth_yandex_id, oauth_github_id, account_id) VALUES ($1, $2, $3, $4, $5, $6, $7, $8)",
        )
        .bind(user_id)
        .bind(email)
        .bind(name.unwrap_or(""))
        .bind(avatar_url)
        .bind(&vk_id)
        .bind(&yandex_id)
        .bind(&github_id)
        .bind(&account_id)
        .execute(&self.db)
        .await?;

        Ok(OAuthUser {
            id: user_id.to_string(),
            email: email.map(|s| s.to_string()),
            name: name.unwrap_or("").to_string(),
            avatar_url: avatar_url.map(|s| s.to_string()),
            account_id,
        })
    }

    pub async fn get_user(&self, user_id: &str) -> Result<Option<OAuthUser>> {
        let row = sqlx::query_as::<_, UserRow>(
            "SELECT id, email, name, avatar_url, oauth_vk_id, oauth_yandex_id, oauth_github_id, account_id, created_at, last_login FROM users WHERE id = $1",
        )
        .bind(Uuid::parse_str(user_id)?)
        .fetch_optional(&self.db)
        .await?;
        Ok(row.map(|r| r.into()))
    }
}

// ── User types ──

#[allow(dead_code)]
#[derive(Debug, sqlx::FromRow)]
struct UserRow {
    id: Uuid,
    email: Option<String>,
    name: String,
    avatar_url: Option<String>,
    oauth_vk_id: Option<String>,
    oauth_yandex_id: Option<String>,
    oauth_github_id: Option<String>,
    account_id: String,
    created_at: DateTime<Utc>,
    last_login: DateTime<Utc>,
}

impl From<UserRow> for OAuthUser {
    fn from(r: UserRow) -> Self {
        Self {
            id: r.id.to_string(),
            email: r.email,
            name: r.name,
            avatar_url: r.avatar_url,
            account_id: r.account_id,
        }
    }
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct OAuthUser {
    pub id: String,
    pub email: Option<String>,
    pub name: String,
    pub avatar_url: Option<String>,
    pub account_id: String,
}

// Simple URL encoding (no external dependency)
pub(crate) mod urlencoding {
    pub fn encode(s: &str) -> String {
        let mut encoded = String::with_capacity(s.len() * 2);
        for b in s.bytes() {
            if b.is_ascii_alphanumeric() || b == b'-' || b == b'_' || b == b'.' || b == b'~' {
                encoded.push(b as char);
            } else {
                encoded.push_str(&format!("%{:02X}", b));
            }
        }
        encoded
    }
}

fn base64_encode_url_safe(data: &[u8]) -> String {
    const ALPHABET: &[u8] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789-_";
    let mut result = String::new();
    for chunk in data.chunks(3) {
        let b0 = chunk[0] as u32;
        let b1 = if chunk.len() > 1 { chunk[1] as u32 } else { 0 };
        let b2 = if chunk.len() > 2 { chunk[2] as u32 } else { 0 };
        let triple = (b0 << 16) | (b1 << 8) | b2;
        result.push(ALPHABET[((triple >> 18) & 0x3F) as usize] as char);
        result.push(ALPHABET[((triple >> 12) & 0x3F) as usize] as char);
        if chunk.len() > 1 {
            result.push(ALPHABET[((triple >> 6) & 0x3F) as usize] as char);
        }
        if chunk.len() > 2 {
            result.push(ALPHABET[(triple & 0x3F) as usize] as char);
        }
    }
    result
}

// =========================================================================
// AuthState — shared state for auth handlers
// =========================================================================

/// Trait for hot-reloading config. Implemented by relay's ConfigReloader.
#[async_trait::async_trait]
pub trait ConfigProvider: Send + Sync {
    async fn get_config(&self) -> flowlink_core::config::RelayConfig;
}

/// Trait for sending verification emails. Implemented by relay's EmailService.
#[async_trait::async_trait]
pub trait EmailSender: Send + Sync {
    fn send_verification_code(
        &self,
        email: &str,
        code: &str,
        lang: &str,
    ) -> std::pin::Pin<Box<dyn std::future::Future<Output = Result<()>> + Send + '_>>;
}

/// Trait for scheduling notification emails. Implemented by relay's EmailQueue.
#[async_trait::async_trait]
pub trait EmailQueueSender: Send + Sync {
    fn schedule_welcome_series(
        &self,
        account_id: &str,
        email: &str,
    ) -> std::pin::Pin<Box<dyn std::future::Future<Output = Result<()>> + Send + '_>>;

    fn schedule_login_notification(
        &self,
        account_id: &str,
        email: &str,
        vars: std::collections::HashMap<String, String>,
    ) -> std::pin::Pin<Box<dyn std::future::Future<Output = Result<()>> + Send + '_>>;
}

/// Shared state for all auth handlers.
/// Constructed by relay from AppState fields and injected as `State<Arc<AuthState>>`.
#[derive(Clone)]
pub struct AuthState {
    pub auth_engine: Option<Arc<AuthEngine>>,
    pub auth_manager: Arc<AuthManager>,
    pub db: Option<Arc<flowlink_db::DbPool>>,
    pub config_reloader: Option<Arc<dyn ConfigProvider>>,
    pub saml_config: Option<Arc<tokio::sync::Mutex<SamlConfig>>>,
    pub email_service: Option<Arc<dyn EmailSender>>,
    pub email_queue: Option<Arc<dyn EmailQueueSender>>,
    pub http_client: reqwest::Client,
    /// Rate limiter for auth endpoints (from relay)
    pub auth_rate_limiter: Arc<AuthRateLimiter>,
    /// Tiered rate limiter (from relay) for plan-aware limits
    pub tiered_rate_limiter: Option<Arc<dyn TieredRateLimitProvider>>,
}

/// Trait for the tiered rate limiter used by auth middleware.
/// Implemented by relay's TieredRateLimiter wrapper.
#[async_trait::async_trait]
pub trait TieredRateLimitProvider: Send + Sync {
    fn check_tiered(
        &self,
        key: &str,
        category: RateLimitCategory,
        tier: &RateLimitTier,
    ) -> Result<(), u64>;
}

/// Re-exported rate limit types for the auth middleware.
/// These are copied from relay/src/rate_limiter.rs to avoid circular dependency.
#[derive(Debug, Clone)]
pub struct RateLimitTier {
    pub name: &'static str,
    pub burst: u32,
    pub sustained: u32,
    pub window_secs: u64,
}

pub const FREE_TIER: RateLimitTier = RateLimitTier {
    name: "free",
    burst: 5,
    sustained: 20,
    window_secs: 60,
};

pub const STARTER_TIER: RateLimitTier = RateLimitTier {
    name: "starter",
    burst: 10,
    sustained: 50,
    window_secs: 60,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum RateLimitCategory {
    AuthLogin,
    EmailChange,
}

/// Extract account_id from request extensions (set by JWT auth middleware).
/// Simplified version that extracts from the `AccountId` extension.
#[derive(Clone)]
pub struct AccountIdExtractor(pub String);

impl<S: Send + Sync> axum::extract::FromRequestParts<S> for AccountIdExtractor {
    type Rejection = axum::http::StatusCode;

    fn from_request_parts(
        parts: &mut axum::http::request::Parts,
        _state: &S,
    ) -> impl std::future::Future<Output = Result<Self, Self::Rejection>> + Send {
        /// Internal marker type for account ID extension
        pub struct AccountId(pub String);

        let account = parts
            .extensions
            .get::<AccountId>()
            .map(|a| a.0.clone())
            .unwrap_or_else(|| "default".to_string());
        std::future::ready(Ok(AccountIdExtractor(account)))
    }
}
