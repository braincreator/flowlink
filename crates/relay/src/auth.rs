//! Authentication module — JWT tokens + OAuth (VK, Yandex) + API token auth

use anyhow::Result;
use chrono::{DateTime, Duration, Utc};
use jsonwebtoken::{DecodingKey, EncodingKey, Header, Validation, decode, encode};
use serde::{Deserialize, Serialize};
use sqlx::PgPool;
use std::sync::Arc;
use uuid::Uuid;

// =========================================================================
// AuthManager — API token validation (legacy, used by handler/middleware)
// =========================================================================

use dashmap::DashMap;

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
    token_to_client: Arc<DashMap<String, String>>,
    db: Option<Arc<sqlx::PgPool>>,
}

impl Default for AuthManager {
    fn default() -> Self { Self::new(None) }
}

impl AuthManager {
    pub fn new(db: Option<Arc<sqlx::PgPool>>) -> Self {
        let mgr = Self {
            clients: Arc::new(DashMap::new()),
            token_to_client: Arc::new(DashMap::new()),
            db: db.clone(),
        };
        if let Some(ref pool) = db {
            let rt = tokio::runtime::Handle::current();
            let pool = pool.clone();
            let clients = mgr.clients.clone();
            let token_map = mgr.token_to_client.clone();
            rt.spawn_blocking(move || {
                let rt = tokio::runtime::Runtime::new().unwrap();
                rt.block_on(async move {
                    if let Ok(rows) = sqlx::query_as::<_, (String, String, String, bool)>(
                        "SELECT agent_id, COALESCE(api_token, ''), name, (status = 'connected') FROM agents WHERE api_token IS NOT NULL"
                    ).fetch_all(pool.as_ref()).await {
                        for (agent_id, api_token, name, active) in &rows {
                            if !api_token.is_empty() {
                                let client = Client {
                                    client_id: agent_id.clone(),
                                    api_token: api_token.clone(),
                                    name: name.clone(),
                                    active: *active,
                                };
                                token_map.insert(api_token.clone(), agent_id.clone());
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
        let id = client.client_id.clone();
        if let Some(old) = self.clients.get(&id) {
            self.token_to_client.remove(&old.api_token);
        }
        self.token_to_client.insert(token.clone(), id.clone());
        self.clients.insert(id.clone(), client.clone());

        // Persist to DB
        if let Some(ref pool) = self.db {
            let pool = pool.clone();
            let cid = id.clone();
            let name = client.name.clone();
            let api_token = token.clone();
            // SHA-256 hash of token for verification without storing plaintext
            use std::collections::hash_map::DefaultHasher;
            use std::hash::{Hash, Hasher};
            let mut hasher = DefaultHasher::new();
            api_token.hash(&mut hasher);
            let token_hash = format!("{:016x}", hasher.finish());
            tokio::spawn(async move {
                let _ = sqlx::query(
                    "INSERT INTO agents (agent_id, api_token, api_token_hash, name, status) VALUES ($1, $2, $3, $4, 'connected') \
                     ON CONFLICT (agent_id) DO UPDATE SET api_token = EXCLUDED.api_token, api_token_hash = EXCLUDED.api_token_hash, name = EXCLUDED.name, status = 'connected'"
                ).bind(&cid).bind(&api_token).bind(&token_hash).bind(&name).execute(pool.as_ref()).await;
            });
        }
    }

    pub fn is_empty(&self) -> bool { self.clients.is_empty() }

    pub fn validate_token(&self, token: &str) -> Option<Client> {
        // Fast path: in-memory cache
        let client_id = self.token_to_client.get(token)?;
        let id: String = client_id.value().clone();
        self.clients.get(&id).map(|c| c.value().clone())
    }

    pub fn get_client(&self, client_id: &str) -> Option<Client> {
        self.clients.get(client_id).map(|c| c.value().clone())
    }
}

// =========================================================================
// AuthEngine — JWT + OAuth
// =========================================================================

// ---------------------------------------------------------------------------
// JWT Claims
// ---------------------------------------------------------------------------

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct Claims {
    /// Subject — user UUID
    pub sub: String,
    /// Account ID (billing)
    pub account_id: String,
    /// Email
    pub email: Option<String>,
    /// Name
    pub name: Option<String>,
    /// Is admin
    pub is_admin: bool,
    pub org_id: Option<String>,
    /// Issued at
    pub iat: i64,
    /// Expiration
    pub exp: i64,
}

// ---------------------------------------------------------------------------
// Auth config
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AuthConfig {
    /// JWT secret key (HS256)
    pub jwt_secret: String,
    /// Access token TTL in minutes (default 15)
    pub access_token_ttl_min: i64,
    /// Refresh token TTL in days (default 30)
    pub refresh_token_ttl_days: i64,
    /// VK OAuth
    pub vk: Option<flowlink_core::config::OAuthProviderConfig>,
    /// Yandex OAuth
    pub yandex: Option<flowlink_core::config::OAuthProviderConfig>,
    /// GitHub OAuth
    pub github: Option<flowlink_core::config::OAuthProviderConfig>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OAuthProvider {
    pub client_id: String,
    pub client_secret: String,
    pub redirect_uri: String,
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

// ---------------------------------------------------------------------------
// JWT token pair
// ---------------------------------------------------------------------------

#[derive(Debug, Serialize)]
pub struct TokenPair {
    pub access_token: String,
    pub refresh_token: String,
    pub expires_in: i64,
    pub session_id: String,
}

// ---------------------------------------------------------------------------
// Auth engine
// ---------------------------------------------------------------------------

/// Active session tracking
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

pub struct AuthEngine {
    config: AuthConfig,
    db: PgPool,
    /// PKCE code_verifier storage: state -> code_verifier
    pkce_store: Arc<DashMap<String, String>>,
    /// Token blacklist: token -> expiry instant
    token_blacklist: Arc<DashMap<String, std::time::Instant>>,
    /// Active sessions: account_id -> Vec<SessionInfo>
    sessions: Arc<DashMap<String, Vec<SessionInfo>>>,
}

impl AuthEngine {
    pub fn new(config: AuthConfig, db: PgPool) -> Self {
        Self { config, db, pkce_store: Arc::new(DashMap::new()), token_blacklist: Arc::new(DashMap::new()), sessions: Arc::new(DashMap::new()) }
    }

    /// Generate JWT access + refresh token pair.
    /// If is_admin is None, queries the DB.
    pub fn create_tokens(&self, user_id: &str, account_id: &str, email: Option<&str>, name: Option<&str>, is_admin: bool, org_id: Option<&str>) -> Result<TokenPair> {
        let now = Utc::now();
        let access_exp = now + Duration::minutes(self.config.access_token_ttl_min);
        let refresh_exp = now + Duration::days(self.config.refresh_token_ttl_days);

        let access_claims = Claims {
            sub: user_id.to_string(),
            account_id: account_id.to_string(),
            email: email.map(|s| s.to_string()),
            name: name.map(|s| s.to_string()),
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

    /// Generate JWT token pair scoped to an organization.
    pub fn create_org_tokens(&self, user_id: &str, account_id: &str, email: Option<&str>, name: Option<&str>, is_admin: bool, org_id: &str, _role: &str) -> Result<TokenPair> {
        self.create_tokens(user_id, account_id, email, name, is_admin, Some(org_id))
    }

    /// Validate access token (with blacklist check)
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

    /// Validate refresh token (with blacklist check)
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

    /// Add token to blacklist (revokes it)
    pub fn blacklist_token(&self, token: &str) {
        self.token_blacklist.insert(token.to_string(), std::time::Instant::now() + std::time::Duration::from_secs(30 * 24 * 3600));
        if self.token_blacklist.len() > 10000 {
            let now = std::time::Instant::now();
            self.token_blacklist.retain(|_, exp| *exp > now);
        }
    }

    // ---- Session management ----

    /// Register a new session for an account
    pub fn create_session(&self, account_id: &str, session_id: &str, ip: Option<&str>, user_agent: Option<&str>, email: Option<&str>, name: Option<&str>) {
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
        // Keep max 20 sessions per account
        sessions.retain(|s| s.session_id != session_id);
        sessions.push(session);
        sessions.sort_by(|a, b| b.last_seen.cmp(&a.last_seen));
        sessions.truncate(20);
    }

    /// List sessions for an account
    pub fn list_sessions(&self, account_id: &str) -> Vec<SessionInfo> {
        self.sessions.get(account_id).map(|s| s.clone()).unwrap_or_default()
    }

    /// Revoke a specific session
    pub fn revoke_session(&self, account_id: &str, session_id: &str) -> bool {
        let mut sessions = match self.sessions.get_mut(account_id) {
            Some(s) => s,
            None => return false,
        };
        let before = sessions.len();
        sessions.retain(|s| s.session_id != session_id);
        sessions.len() < before
    }

    /// Revoke all sessions except the current one
    pub fn revoke_other_sessions(&self, account_id: &str, keep_session_id: &str) -> usize {
        let mut sessions = match self.sessions.get_mut(account_id) {
            Some(s) => s,
            None => return 0,
        };
        let before = sessions.len();
        sessions.retain(|s| s.session_id == keep_session_id);
        before - sessions.len()
    }

    // ---- OAuth flows ----

    /// Get VK ID OAuth authorize URL (OAuth 2.1 with PKCE)
    pub fn vk_auth_url(&self) -> Option<String> {
        let vk = self.config.vk.as_ref()?;
        
        // Generate PKCE code_verifier (43-128 chars)
        let code_verifier: String = std::iter::repeat_with(|| {
            let b = rand::random::<u8>();
            match b % 3 {
                0 => char::from(b'A' + (b % 26)),
                1 => char::from(b'a' + (b % 26)),
                _ => char::from(b'0' + (b % 10)),
            }
        }).take(64).collect();
        
        // Compute code_challenge = base64url(sha256(code_verifier))
        use sha2::{Sha256, Digest};
        let mut hasher = Sha256::new();
        hasher.update(code_verifier.as_bytes());
        let hash = hasher.finalize();
        let code_challenge = base64_encode_url_safe(&hash);
        
        // Generate random state and store code_verifier
        let state: String = std::iter::repeat_with(|| {
            let b = rand::random::<u8>();
            match b % 3 {
                0 => char::from(b'A' + (b % 26)),
                1 => char::from(b'a' + (b % 26)),
                _ => char::from(b'0' + (b % 10)),
            }
        }).take(32).collect();
        
        self.pkce_store.insert(state.clone(), code_verifier);
        
        Some(format!(
            "https://id.vk.ru/authorize?response_type=code&client_id={}&redirect_uri={}&state={}&code_challenge={}&code_challenge_method=S256&scope=email+phone",
            vk.client_id,
            urlencoding::encode(&vk.redirect_uri),
            state,
            code_challenge,
        ))
    }

    /// Get Yandex OAuth authorize URL
    pub fn yandex_auth_url(&self, state: &str) -> Option<String> {
        let yandex = self.config.yandex.as_ref()?;
        Some(format!(
            "https://oauth.yandex.ru/authorize?client_id={}&response_type=code&redirect_uri={}&scope=login:email login:info&state={}",
            yandex.client_id,
            urlencoding::encode(&yandex.redirect_uri),
            state,
        ))
    }

    /// Exchange VK ID code for tokens, fetch user info, find or create user
    pub async fn vk_callback(&self, code: &str, state: &str) -> Result<OAuthUser> {
        let vk = self.config.vk.as_ref().ok_or_else(|| anyhow::anyhow!("VK OAuth not configured"))?;
        
        // Get code_verifier from PKCE store
        let code_verifier = self.pkce_store.get(state)
            .map(|v| v.value().clone())
            .ok_or_else(|| anyhow::anyhow!("VK: invalid or expired state/PKCE"))?
        ;
        self.pkce_store.remove(state);

        // Exchange code for access token (VK ID OAuth 2.1)
        // Note: VK ID does NOT use client_secret, only PKCE code_verifier
        let client = reqwest::Client::new();
        let token_resp: serde_json::Value = client.post("https://id.vk.ru/oauth2/auth")
            .form(&[
                ("grant_type", "authorization_code"),
                ("code", code),
                ("client_id", &vk.client_id),
                ("redirect_uri", &vk.redirect_uri),
                ("code_verifier", &code_verifier),
                ("state", state),
                ("device_id", "1"),
            ])
            .send().await?
            .json().await?;

        let access_token = token_resp["access_token"]
            .as_str()
            .ok_or_else(|| anyhow::anyhow!("VK: no access_token in response: {:?}", token_resp))?
            .to_string();
        let vk_user_id = token_resp["user_id"]
            .as_str()
            .ok_or_else(|| anyhow::anyhow!("VK: no user_id"))?;
        let email = token_resp["email"].as_str().map(|s| s.to_string());

        // Fetch user info via VK ID API
        let info_resp: serde_json::Value = client.get("https://id.vk.ru/oauth2/user_info")
            .header("Authorization", format!("Bearer {}", access_token))
            .send().await?
            .json().await?;
        
        let user_data = &info_resp["user"];
        let first_name = user_data["first_name"].as_str().unwrap_or("");
        let last_name = user_data["last_name"].as_str().unwrap_or("");
        let avatar = user_data["avatar"].as_str().map(|s| s.to_string());
        let _phone = user_data["phone"].as_str().map(|s| s.to_string());

        let full_name = format!("{} {}", first_name, last_name).trim().to_string();

        // Find or create user
        let user = self.find_or_create_user(
            Some(format!("vk:{}", vk_user_id)),
            None, None,
            email.as_deref(),
            if full_name.is_empty() { None } else { Some(&full_name) },
            avatar.as_deref(),
        ).await?;

        Ok(user)
    }

    /// Exchange Yandex code for tokens, fetch user info, find or create user
    pub async fn yandex_callback(&self, code: &str) -> Result<OAuthUser> {
        let yandex = self.config.yandex.as_ref().ok_or_else(|| anyhow::anyhow!("Yandex OAuth not configured"))?;

        // Exchange code for token
        let client = reqwest::Client::new();
        let token_resp: serde_json::Value = client
            .post("https://oauth.yandex.ru/token")
            .form(&[
                ("grant_type", "authorization_code"),
                ("code", code),
                ("client_id", &yandex.client_id),
                ("client_secret", &yandex.client_secret),
            ])
            .send().await?
            .json().await?;

        let access_token = token_resp["access_token"]
            .as_str()
            .ok_or_else(|| anyhow::anyhow!("Yandex: no access_token"))?
            .to_string();

        // Fetch user info
        let info_resp: serde_json::Value = client
            .get("https://login.yandex.ru/info?format=json")
            .header("Authorization", format!("OAuth {}", access_token))
            .send().await?
            .json().await?;

        let yandex_id = info_resp["id"]
            .as_str()
            .ok_or_else(|| anyhow::anyhow!("Yandex: no id"))?;
        let email = info_resp["default_email"].as_str().map(|s| s.to_string());
        let name = info_resp["real_name"].as_str().or_else(|| info_resp["display_name"].as_str());
        let avatar = info_resp["default_avatar_id"].as_str().map(|id| {
            format!("https://avatars.yandex.net/get-yapic/{}/islands-200", id)
        });

        // Find or create user
        let user = self.find_or_create_user(
            None,
            Some(format!("yandex:{}", yandex_id)),
            None,
            email.as_deref(),
            name,
            avatar.as_deref(),
        ).await?;

        Ok(user)
    }

    /// Get GitHub OAuth authorize URL
    pub fn github_auth_url(&self) -> Option<String> {
        let gh = self.config.github.as_ref()?;
        let state: String = std::iter::repeat_with(|| {
            let b = rand::random::<u8>();
            match b % 3 {
                0 => char::from(b'A' + (b % 26)),
                1 => char::from(b'a' + (b % 26)),
                _ => char::from(b'0' + (b % 10)),
            }
        }).take(32).collect();
        Some(format!(
            "https://github.com/login/oauth/authorize?client_id={}&redirect_uri={}&scope=user:email+read:user&state={}",
            gh.client_id, urlencoding::encode(&gh.redirect_uri), state,
        ))
    }

    /// Exchange GitHub code for tokens, fetch user info, find or create user
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
            .send().await?.json().await?;

        let access_token = token_resp["access_token"]
            .as_str().ok_or_else(|| anyhow::anyhow!("GitHub: no access_token: {:?}", token_resp))?
            .to_string();

        let user_resp: serde_json::Value = client
            .get("https://api.github.com/user")
            .header("Authorization", format!("Bearer {}", access_token))
            .header("User-Agent", "FlowLink")
            .send().await?.json().await?;

        let gh_id = user_resp["id"].as_u64().ok_or_else(|| anyhow::anyhow!("GitHub: no id"))?;
        let login = user_resp["login"].as_str().unwrap_or("");
        let name = user_resp["name"].as_str().unwrap_or(login);
        let avatar = user_resp["avatar_url"].as_str().map(|s| s.to_string());
        let email = user_resp["email"].as_str().map(|s| s.to_string());

        // If no public email, fetch from emails API
        let email = if email.is_none() {
            let emails_resp: serde_json::Value = client
                .get("https://api.github.com/user/emails")
                .header("Authorization", format!("Bearer {}", access_token))
                .header("User-Agent", "FlowLink")
                .send().await?.json().await?;
            emails_resp.as_array()
                .and_then(|arr| arr.iter().find(|e| e["primary"].as_bool() == Some(true)))
                .and_then(|e| e["email"].as_str())
                .map(|s| s.to_string())
        } else { email };

        let user = self.find_or_create_user(
            None, None, Some(format!("github:{}", gh_id)),
            email.as_deref(),
            if name.is_empty() { None } else { Some(name) },
            avatar.as_deref(),
        ).await?;

        Ok(user)
    }

    /// Find existing user by OAuth ID or create new one with Trial account
    async fn find_or_create_user(
        &self,
        vk_id: Option<String>,
        yandex_id: Option<String>,
        github_id: Option<String>,
        email: Option<&str>,
        name: Option<&str>,
        avatar_url: Option<&str>,
    ) -> Result<OAuthUser> {
        // Try to find by VK ID
        if let Some(ref vk) = vk_id {
            let row = sqlx::query_as::<_, UserRow>(
                "SELECT id, email, name, avatar_url, oauth_vk_id, oauth_yandex_id, oauth_github_id, account_id, created_at, last_login FROM users WHERE oauth_vk_id = $1"
            )
            .bind(vk)
            .fetch_optional(&self.db)
            .await?;
            if let Some(user) = row {
                sqlx::query("UPDATE users SET last_login = now() WHERE id = $1")
                    .bind(&user.id).execute(&self.db).await?;
                return Ok(user.into());
            }
        }

        // Try to find by Yandex ID
        if let Some(ref yandex) = yandex_id {
            let row = sqlx::query_as::<_, UserRow>(
                "SELECT id, email, name, avatar_url, oauth_vk_id, oauth_yandex_id, oauth_github_id, account_id, created_at, last_login FROM users WHERE oauth_yandex_id = $1"
            )
            .bind(yandex)
            .fetch_optional(&self.db)
            .await?;
            if let Some(user) = row {
                sqlx::query("UPDATE users SET last_login = now() WHERE id = $1")
                    .bind(&user.id).execute(&self.db).await?;
                return Ok(user.into());
            }
        }

        // Try to find by GitHub ID
        if let Some(ref gh) = github_id {
            let row = sqlx::query_as::<_, UserRow>(
                "SELECT id, email, name, avatar_url, oauth_vk_id, oauth_yandex_id, oauth_github_id, account_id, created_at, last_login FROM users WHERE oauth_github_id = $1"
            )
            .bind(gh)
            .fetch_optional(&self.db)
            .await?;
            if let Some(user) = row {
                sqlx::query("UPDATE users SET last_login = now() WHERE id = $1")
                    .bind(&user.id).execute(&self.db).await?;
                return Ok(user.into());
            }
        }

        // Try to find by email
        if let Some(em) = email {
            let row = sqlx::query_as::<_, UserRow>(
                "SELECT id, email, name, avatar_url, oauth_vk_id, oauth_yandex_id, account_id, created_at, last_login FROM users WHERE email = $1"
            )
            .bind(em)
            .fetch_optional(&self.db)
            .await?;

            if let Some(user) = row {
                // Link OAuth provider
                if let Some(ref vk) = vk_id {
                    sqlx::query("UPDATE users SET oauth_vk_id = $1, last_login = now() WHERE id = $2 AND oauth_vk_id IS NULL")
                        .bind(vk)
                        .bind(&user.id)
                        .execute(&self.db)
                        .await?;
                }
                if let Some(ref yandex) = yandex_id {
                    sqlx::query("UPDATE users SET oauth_yandex_id = $1, last_login = now() WHERE id = $2 AND oauth_yandex_id IS NULL")
                        .bind(yandex)
                        .bind(&user.id)
                        .execute(&self.db)
                        .await?;
                }
                if let Some(ref gh) = github_id {
                    sqlx::query("UPDATE users SET oauth_github_id = $1, last_login = now() WHERE id = $2 AND oauth_github_id IS NULL")
                        .bind(gh)
                        .bind(&user.id)
                        .execute(&self.db)
                        .await?;
                }
                return Ok(user.into());
            }
        }

        // Create new user
        let user_id = Uuid::new_v4();
        let account_id = format!("user:{}", user_id);

        sqlx::query("INSERT INTO accounts (account_id, plan_id, active) VALUES ($1, 'trial', true)")
            .bind(&account_id).execute(&self.db).await?;

        sqlx::query(
            "INSERT INTO users (id, email, name, avatar_url, oauth_vk_id, oauth_yandex_id, oauth_github_id, account_id) VALUES ($1, $2, $3, $4, $5, $6, $7, $8)"
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

    /// Get user by ID
    pub async fn get_user(&self, user_id: &str) -> Result<Option<OAuthUser>> {
        let row = sqlx::query_as::<_, UserRow>(
            "SELECT id, email, name, avatar_url, oauth_vk_id, oauth_yandex_id, oauth_github_id, account_id, created_at, last_login FROM users WHERE id = $1"
        )
        .bind(Uuid::parse_str(user_id)?)
        .fetch_optional(&self.db)
        .await?;
        Ok(row.map(|r| r.into()))
    }
}

// ---------------------------------------------------------------------------
// User types
// ---------------------------------------------------------------------------

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
