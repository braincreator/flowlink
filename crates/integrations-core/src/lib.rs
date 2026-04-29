//! FlowLink Integrations Core — traits, types, event bus, and lifecycle management.
//!
//! This crate provides the foundation for the integration marketplace:
//! - `Integration` trait — every integration implements it
//! - `OAuthIntegration` trait — for integrations requiring OAuth2 flow
//! - `IntegrationEvent` — events dispatched from relay to integrations
//! - `IntegrationManager` — runtime registry that manages active integrations
//! - OAuth2 token lifecycle (authorize → callback → exchange → refresh)

use std::sync::Arc;
use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

// ═══════════════════════════════════════════════
// Integration Types
// ═══════════════════════════════════════════════

/// Unique identifier for an integration type (e.g. "telegram", "slack", "discord")
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Hash)]
pub struct IntegrationKind(pub String);

impl std::fmt::Display for IntegrationKind {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl From<&str> for IntegrationKind {
    fn from(s: &str) -> Self { Self(s.to_string()) }
}

/// Unique instance ID for a specific integration installation
pub type IntegrationId = String;

/// Status of an integration instance
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum IntegrationStatus {
    /// Installed but waiting for OAuth authorization
    PendingAuth,
    /// Installed but not configured
    Installed,
    /// Configured and ready to start
    Configured,
    /// Currently running
    Active,
    /// Temporarily stopped
    Paused,
    /// OAuth token expired, needs re-authorization
    TokenExpired,
    /// Error state
    Error(String),
    /// Uninstalled
    Uninstalled,
}

// ═══════════════════════════════════════════════
// OAuth2 Support
// ═══════════════════════════════════════════════

/// OAuth2 configuration for an integration type
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OAuthConfig {
    /// OAuth2 authorization URL (e.g. "https://slack.com/oauth/v2/authorize")
    pub authorize_url: String,
    /// OAuth2 token exchange URL (e.g. "https://slack.com/api/oauth.v2.access")
    pub token_url: String,
    /// OAuth2 client ID
    pub client_id: String,
    /// OAuth2 client secret (stored securely, not sent to frontend)
    pub client_secret: String,
    /// OAuth2 scopes (space-separated)
    pub scopes: String,
    /// Additional query params for authorize URL
    pub extra_params: Option<HashMap<String, String>>,
}

/// OAuth2 tokens stored per integration instance
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OAuthTokens {
    /// Access token
    pub access_token: String,
    /// Refresh token (if provider supports)
    pub refresh_token: Option<String>,
    /// Token type (usually "Bearer")
    pub token_type: String,
    /// Scopes granted
    pub scope: String,
    /// Expires at (Unix timestamp, None = never expires)
    pub expires_at: Option<i64>,
    /// When tokens were obtained
    pub obtained_at: chrono::DateTime<chrono::Utc>,
}

impl OAuthTokens {
    /// Check if the access token is expired or about to expire (5 min buffer)
    pub fn is_expired(&self) -> bool {
        match self.expires_at {
            Some(exp) => chrono::Utc::now().timestamp() + 300 > exp,
            None => false,
        }
    }

    /// Seconds until expiry
    pub fn expires_in_seconds(&self) -> Option<i64> {
        self.expires_at.map(|exp| exp - chrono::Utc::now().timestamp())
    }
}

/// State for pending OAuth2 authorization flows
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PendingOAuthState {
    /// Random state parameter for CSRF protection
    pub state: String,
    /// Integration instance ID this flow is for
    pub integration_id: IntegrationId,
    /// Account ID that initiated the flow
    pub account_id: String,
    /// Integration type
    pub kind: IntegrationKind,
    /// Redirect URL after OAuth completion
    pub redirect_after: Option<String>,
    /// When this state was created (for expiry)
    pub created_at: chrono::DateTime<chrono::Utc>,
}

impl PendingOAuthState {
    /// Check if this pending state has expired (10 minutes)
    pub fn is_expired(&self) -> bool {
        let age = chrono::Utc::now() - self.created_at;
        age > chrono::Duration::minutes(10)
    }
}

// ═══════════════════════════════════════════════
// Events
// ═══════════════════════════════════════════════

/// Events that relay dispatches to integrations
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", content = "payload")]
pub enum IntegrationEvent {
    // Agent events
    AgentConnected { agent_id: String, hostname: String },
    AgentDisconnected { agent_id: String, hostname: String },

    // Security events
    ShieldAlert { agent_id: String, risk: String, command: String },

    // Approval events
    ApprovalRequested { approval_id: String, agent_id: String, command: String, risk: String },
    ApprovalResolved { approval_id: String, decision: String, resolved_by: String },

    // Billing events
    PaymentReceived { account_id: String, amount_kopecks: u64, description: String },
    SubscriptionChanged { account_id: String, plan: String },
    PlanExpiring { account_id: String, days_left: u32 },

    // System events
    SystemAlert { level: String, message: String },

    // Custom
    Custom { name: String, data: serde_json::Value },
}

// ═══════════════════════════════════════════════
// Configuration & Metadata
// ═══════════════════════════════════════════════

/// Configuration for a single integration instance (stored in DB)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IntegrationConfig {
    pub id: IntegrationId,
    pub kind: IntegrationKind,
    pub account_id: String,
    pub org_id: Option<String>,
    /// Integration-specific config (bot_token, webhook_url, etc.)
    pub config: serde_json::Value,
    /// OAuth2 tokens (if integration uses OAuth)
    pub oauth_tokens: Option<OAuthTokens>,
    pub subscribed_events: Vec<String>,
    pub status: IntegrationStatus,
    pub name: String,
    pub created_at: chrono::DateTime<chrono::Utc>,
    pub updated_at: chrono::DateTime<chrono::Utc>,
}

/// Metadata about an integration type (for marketplace catalog)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IntegrationMeta {
    pub kind: IntegrationKind,
    pub display_name: String,
    pub description: String,
    pub icon: String,
    pub category: IntegrationCategory,
    /// Configuration schema (JSON Schema)
    pub config_schema: serde_json::Value,
    /// Available events this integration can subscribe to
    pub available_events: Vec<EventDescriptor>,
    pub supports_user_instances: bool,
    pub supports_org_instances: bool,
    /// Does this integration require OAuth2 flow?
    pub requires_oauth: bool,
    /// OAuth2 config (if requires_oauth) — client_secret is NOT sent to frontend
    pub oauth_config: Option<OAuthPublicConfig>,
    pub author: String,
    pub version: String,
}

/// Public OAuth config (safe to send to frontend — no secrets)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OAuthPublicConfig {
    pub authorize_url: String,
    pub client_id: String,
    pub scopes: String,
    pub extra_params: Option<HashMap<String, String>>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum IntegrationCategory {
    Messenger,
    Monitoring,
    CiCd,
    Productivity,
    Storage,
    Custom,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EventDescriptor {
    pub event_type: String,
    pub display_name: String,
    pub description: String,
}

#[derive(Debug)]
pub enum IntegrationAction {
    SendMessage {
        chat_id: Option<String>,
        text: String,
        buttons: Vec<Vec<(String, String)>>,
    },
    Noop,
}

#[derive(Debug, thiserror::Error)]
pub enum IntegrationError {
    #[error("Configuration error: {0}")]
    ConfigError(String),
    #[error("Connection error: {0}")]
    ConnectionError(String),
    #[error("Authentication error: {0}")]
    AuthError(String),
    #[error("OAuth token expired: {0}")]
    TokenExpired(String),
    #[error("OAuth state invalid: {0}")]
    InvalidOAuthState(String),
    #[error("Rate limited: {0}")]
    RateLimited(String),
    #[error("Not found: {0}")]
    NotFound(String),
    #[error("Internal error: {0}")]
    Internal(String),
}

// ═══════════════════════════════════════════════
// Core Trait
// ═══════════════════════════════════════════════

/// Base trait for ALL integrations.
#[async_trait]
pub trait Integration: Send + Sync {
    fn kind(&self) -> IntegrationKind;
    fn meta(&self) -> IntegrationMeta;

    /// Does this integration require OAuth2?
    fn requires_oauth(&self) -> bool { false }

    /// OAuth2 config (if requires_oauth)
    fn oauth_config(&self) -> Option<OAuthConfig> { None }

    /// Validate static config before starting
    async fn validate_config(&self, config: &serde_json::Value) -> Result<(), IntegrationError>;

    /// Exchange OAuth2 authorization code for tokens
    async fn exchange_oauth_code(
        &self,
        code: &str,
        redirect_uri: &str,
    ) -> Result<OAuthTokens, IntegrationError> {
        // Default: use standard OAuth2 code exchange
        let oauth = self.oauth_config()
            .ok_or_else(|| IntegrationError::ConfigError("No OAuth config".into()))?;

        let client = reqwest::Client::new();
        let resp = client.post(&oauth.token_url)
            .form(&[
                ("grant_type", "authorization_code"),
                ("code", code),
                ("redirect_uri", redirect_uri),
                ("client_id", &oauth.client_id),
                ("client_secret", &oauth.client_secret),
            ])
            .send().await
            .map_err(|e| IntegrationError::ConnectionError(e.to_string()))?;

        let body: serde_json::Value = resp.json().await
            .map_err(|e| IntegrationError::AuthError(e.to_string()))?;

        self.parse_token_response(&body)
    }

    /// Refresh an expired access token
    async fn refresh_oauth_token(
        &self,
        refresh_token: &str,
    ) -> Result<OAuthTokens, IntegrationError> {
        let oauth = self.oauth_config()
            .ok_or_else(|| IntegrationError::ConfigError("No OAuth config".into()))?;

        let client = reqwest::Client::new();
        let resp = client.post(&oauth.token_url)
            .form(&[
                ("grant_type", "refresh_token"),
                ("refresh_token", refresh_token),
                ("client_id", &oauth.client_id),
                ("client_secret", &oauth.client_secret),
            ])
            .send().await
            .map_err(|e| IntegrationError::ConnectionError(e.to_string()))?;

        let body: serde_json::Value = resp.json().await
            .map_err(|e| IntegrationError::AuthError(e.to_string()))?;

        self.parse_token_response(&body)
    }

    /// Parse standard OAuth2 token response (override for non-standard providers)
    fn parse_token_response(&self, body: &serde_json::Value) -> Result<OAuthTokens, IntegrationError> {
        let access_token = body.get("access_token")
            .and_then(|v| v.as_str())
            .ok_or_else(|| IntegrationError::AuthError("No access_token in response".into()))?
            .to_string();

        let refresh_token = body.get("refresh_token").and_then(|v| v.as_str()).map(|s| s.to_string());
        let expires_in = body.get("expires_in").and_then(|v| v.as_i64());
        let scope = body.get("scope").and_then(|v| v.as_str()).unwrap_or("").to_string();
        let token_type = body.get("token_type").and_then(|v| v.as_str()).unwrap_or("Bearer").to_string();

        let now = chrono::Utc::now();
        let expires_at = expires_in.map(|secs| now.timestamp() + secs);

        Ok(OAuthTokens {
            access_token,
            refresh_token,
            token_type,
            scope,
            expires_at,
            obtained_at: now,
        })
    }

    /// Build the authorization URL for this integration
    fn build_authorize_url(&self, state: &str, redirect_uri: &str) -> Result<String, IntegrationError> {
        let oauth = self.oauth_config()
            .ok_or_else(|| IntegrationError::ConfigError("No OAuth config".into()))?;

        let mut url = format!(
            "{}?response_type=code&client_id={}&redirect_uri={}&scope={}&state={}",
            oauth.authorize_url,
            oauth.client_id,
            urlencoding::encode(redirect_uri),
            urlencoding::encode(&oauth.scopes),
            state,
        );

        if let Some(ref extra) = oauth.extra_params {
            for (k, v) in extra {
                url.push_str(&format!("&{}={}", k, urlencoding::encode(v)));
            }
        }

        Ok(url)
    }

    /// Start the integration with given config
    async fn start(
        &self,
        id: IntegrationId,
        config: IntegrationConfig,
        event_rx: tokio::sync::broadcast::Receiver<IntegrationEvent>,
    ) -> Result<(), IntegrationError>;

    /// Stop the integration
    async fn stop(&self, id: &IntegrationId) -> Result<(), IntegrationError>;

    /// Handle an incoming event
    async fn handle_event(
        &self,
        event: &IntegrationEvent,
        config: &IntegrationConfig,
    ) -> Vec<IntegrationAction>;

    /// Handle a command from the integration's platform
    async fn handle_command(
        &self,
        command: &str,
        args: &serde_json::Value,
        config: &IntegrationConfig,
    ) -> Result<serde_json::Value, IntegrationError>;

    /// Health check
    async fn health_check(&self, id: &IntegrationId) -> Result<bool, IntegrationError>;
}

// ═══════════════════════════════════════════════
// Integration Manager
// ═══════════════════════════════════════════════

pub struct IntegrationManager {
    factories: HashMap<IntegrationKind, Arc<dyn Integration>>,
    instances: HashMap<IntegrationId, IntegrationHandle>,
    event_tx: tokio::sync::broadcast::Sender<IntegrationEvent>,
    /// Pending OAuth states (state → PendingOAuthState)
    pending_oauth: HashMap<String, PendingOAuthState>,
}

struct IntegrationHandle {
    kind: IntegrationKind,
    config: IntegrationConfig,
    shutdown_tx: tokio::sync::oneshot::Sender<()>,
}

impl IntegrationManager {
    pub fn new() -> Self {
        let (event_tx, _) = tokio::sync::broadcast::channel(1024);
        Self {
            factories: HashMap::new(),
            instances: HashMap::new(),
            event_tx,
            pending_oauth: HashMap::new(),
        }
    }

    pub fn register(&mut self, integration: Arc<dyn Integration>) {
        let kind = integration.kind();
        log::info!("📦 Registered integration type: {} (oauth={})", kind, integration.requires_oauth());
        self.factories.insert(kind, integration);
    }

    pub fn list_available(&self) -> Vec<IntegrationMeta> {
        self.factories.values().map(|f| f.meta()).collect()
    }

    pub fn get_meta(&self, kind: &IntegrationKind) -> Option<IntegrationMeta> {
        self.factories.get(kind).map(|f| f.meta())
    }

    /// Initiate OAuth2 flow — returns the authorization URL to redirect user to
    pub fn begin_oauth(
        &mut self,
        kind: &IntegrationKind,
        integration_id: IntegrationId,
        account_id: String,
        redirect_uri: &str,
        redirect_after: Option<String>,
    ) -> Result<String, IntegrationError> {
        let factory = self.factories.get(kind)
            .ok_or_else(|| IntegrationError::NotFound(format!("Unknown: {}", kind)))?;

        if !factory.requires_oauth() {
            return Err(IntegrationError::ConfigError("Integration doesn't require OAuth".into()));
        }

        // Generate random state for CSRF protection
        let state = uuid::Uuid::new_v4().to_string();

        // Store pending state
        self.pending_oauth.insert(state.clone(), PendingOAuthState {
            state: state.clone(),
            integration_id,
            account_id,
            kind: kind.clone(),
            redirect_after,
            created_at: chrono::Utc::now(),
        });

        // Build authorize URL
        factory.build_authorize_url(&state, redirect_uri)
    }

    /// Complete OAuth2 flow — exchange code for tokens
    pub async fn complete_oauth(
        &mut self,
        state: &str,
        code: &str,
        redirect_uri: &str,
    ) -> Result<(IntegrationId, OAuthTokens), IntegrationError> {
        // Find and remove pending state
        let pending = self.pending_oauth.remove(state)
            .ok_or_else(|| IntegrationError::InvalidOAuthState("Unknown or expired state".into()))?;

        if pending.is_expired() {
            return Err(IntegrationError::InvalidOAuthState("OAuth state expired".into()));
        }

        let factory = self.factories.get(&pending.kind)
            .ok_or_else(|| IntegrationError::NotFound(format!("Unknown: {}", pending.kind)))?;

        // Exchange code for tokens
        let tokens = factory.exchange_oauth_code(code, redirect_uri).await?;

        log::info!("🔑 OAuth tokens obtained for integration {} ({})", pending.integration_id, pending.kind);

        Ok((pending.integration_id, tokens))
    }

    /// Refresh expired OAuth tokens for an integration
    pub async fn refresh_tokens(
        &self,
        kind: &IntegrationKind,
        refresh_token: &str,
    ) -> Result<OAuthTokens, IntegrationError> {
        let factory = self.factories.get(kind)
            .ok_or_else(|| IntegrationError::NotFound(format!("Unknown: {}", kind)))?;
        factory.refresh_oauth_token(refresh_token).await
    }

    /// Install and start an integration
    pub async fn install(
        &mut self,
        kind: &IntegrationKind,
        config: IntegrationConfig,
    ) -> Result<IntegrationId, IntegrationError> {
        let factory = self.factories.get(kind)
            .ok_or_else(|| IntegrationError::NotFound(format!("Unknown: {}", kind)))?;

        // Skip validate_config for OAuth integrations waiting for tokens
        if !factory.requires_oauth() || config.oauth_tokens.is_some() {
            factory.validate_config(&config.config).await?;
        }

        let id = config.id.clone();
        let (shutdown_tx, shutdown_rx) = tokio::sync::oneshot::channel();
        let event_rx = self.event_tx.subscribe();

        let factory_clone = factory.clone();
        let config_clone = config.clone();
        let id_clone = id.clone();
        tokio::spawn(async move {
            tokio::select! {
                result = factory_clone.start(id_clone.clone(), config_clone, event_rx) => {
                    if let Err(e) = result {
                        log::error!("Integration {} failed: {}", id_clone, e);
                    }
                }
                _ = shutdown_rx => {
                    log::info!("Integration {} shutting down", id_clone);
                }
            }
        });

        self.instances.insert(id.clone(), IntegrationHandle {
            kind: kind.clone(),
            config,
            shutdown_tx,
        });

        log::info!("✅ Integration installed: {} ({})", id, kind);
        Ok(id)
    }

    /// Update OAuth tokens for an existing instance
    pub fn update_oauth_tokens(&mut self, id: &IntegrationId, tokens: OAuthTokens) {
        if let Some(handle) = self.instances.get_mut(id) {
            handle.config.oauth_tokens = Some(tokens);
        }
    }

    pub async fn uninstall(&mut self, id: &IntegrationId) -> Result<(), IntegrationError> {
        if let Some(handle) = self.instances.remove(id) {
            let _ = handle.shutdown_tx.send(());
            if let Some(factory) = self.factories.get(&handle.kind) {
                factory.stop(id).await?;
            }
            log::info!("🗑 Integration uninstalled: {}", id);
        }
        Ok(())
    }

    pub fn dispatch_event(&self, event: IntegrationEvent) {
        if let Err(e) = self.event_tx.send(event) {
            log::warn!("Failed to dispatch event: {}", e);
        }
    }

    pub fn list_user_integrations(&self, account_id: &str) -> Vec<&IntegrationConfig> {
        self.instances.values()
            .filter(|h| h.config.account_id == account_id)
            .map(|h| &h.config)
            .collect()
    }

    pub fn active_count(&self) -> usize { self.instances.len() }
}

impl Default for IntegrationManager {
    fn default() -> Self { Self::new() }
}

// ═══════════════════════════════════════════════
// Marketplace Catalog
// ═══════════════════════════════════════════════

pub fn builtin_catalog() -> Vec<IntegrationMeta> {
    vec![
        IntegrationMeta {
            kind: IntegrationKind("telegram".into()),
            display_name: "Telegram Bot".into(),
            description: "Connect your own Telegram bot for notifications, commands, and approvals.".into(),
            icon: "🤖".into(),
            category: IntegrationCategory::Messenger,
            config_schema: serde_json::json!({
                "type": "object",
                "required": ["bot_token"],
                "properties": {
                    "bot_token": { "type": "string", "title": "Bot Token", "description": "Get it from @BotFather" },
                    "admin_chat_id": { "type": "integer", "title": "Admin Chat ID" },
                    "webhook_url": { "type": "string", "title": "Webhook URL (optional)" }
                }
            }),
            available_events: EventDescriptor::all_events(),
            supports_user_instances: true,
            supports_org_instances: true,
            requires_oauth: false,
            oauth_config: None,
            author: "FlowLink".into(),
            version: "1.0.0".into(),
        },
        IntegrationMeta {
            kind: IntegrationKind("slack".into()),
            display_name: "Slack".into(),
            description: "Connect your Slack workspace. Requires OAuth2 installation.".into(),
            icon: "💬".into(),
            category: IntegrationCategory::Messenger,
            config_schema: serde_json::json!({
                "type": "object",
                "properties": {
                    "channel": { "type": "string", "title": "Default Channel" }
                }
            }),
            available_events: EventDescriptor::all_events(),
            supports_user_instances: false,
            supports_org_instances: true,
            requires_oauth: true,
            oauth_config: Some(OAuthPublicConfig {
                authorize_url: "https://slack.com/oauth/v2/authorize".into(),
                client_id: "${SLACK_CLIENT_ID}".into(),
                scopes: "chat:write chat:write.public channels:read groups:read im:write".into(),
                extra_params: None,
            }),
            author: "FlowLink".into(),
            version: "1.0.0".into(),
        },
        IntegrationMeta {
            kind: IntegrationKind("discord".into()),
            display_name: "Discord".into(),
            description: "Connect your Discord server via OAuth2 bot invite.".into(),
            icon: "🎮".into(),
            category: IntegrationCategory::Messenger,
            config_schema: serde_json::json!({
                "type": "object",
                "properties": {
                    "channel_id": { "type": "string", "title": "Notification Channel ID" }
                }
            }),
            available_events: EventDescriptor::all_events(),
            supports_user_instances: false,
            supports_org_instances: true,
            requires_oauth: true,
            oauth_config: Some(OAuthPublicConfig {
                authorize_url: "https://discord.com/api/oauth2/authorize".into(),
                client_id: "${DISCORD_CLIENT_ID}".into(),
                scopes: "bot identify webhooks.incoming".into(),
                extra_params: Some(HashMap::from([
                    ("permissions".into(), "2048".into()), // SEND_MESSAGES
                ])),
            }),
            author: "FlowLink".into(),
            version: "1.0.0".into(),
        },
        IntegrationMeta {
            kind: IntegrationKind("github".into()),
            display_name: "GitHub".into(),
            description: "Get alerts as GitHub issues, link commits to approvals.".into(),
            icon: "🐙".into(),
            category: IntegrationCategory::CiCd,
            config_schema: serde_json::json!({
                "type": "object",
                "properties": {
                    "repository": { "type": "string", "title": "Repository (owner/repo)" }
                }
            }),
            available_events: EventDescriptor::all_events(),
            supports_user_instances: true,
            supports_org_instances: true,
            requires_oauth: true,
            oauth_config: Some(OAuthPublicConfig {
                authorize_url: "https://github.com/login/oauth/authorize".into(),
                client_id: "${GITHUB_CLIENT_ID}".into(),
                scopes: "repo read:org".into(),
                extra_params: None,
            }),
            author: "FlowLink".into(),
            version: "1.0.0".into(),
        },
        IntegrationMeta {
            kind: IntegrationKind("max".into()),
            display_name: "MAX Messenger".into(),
            description: "Connect your MAX messenger bot for notifications and commands.".into(),
            icon: "📱".into(),
            category: IntegrationCategory::Messenger,
            config_schema: serde_json::json!({
                "type": "object",
                "required": ["access_token"],
                "properties": {
                    "access_token": { "type": "string", "title": "Access Token", "description": "Bot token from MAX business platform" },
                    "chat_id": { "type": "integer", "title": "Chat ID", "description": "Default notification chat ID" },
                    "webhook_url": { "type": "string", "title": "Webhook URL", "description": "URL for receiving MAX updates" }
                }
            }),
            available_events: EventDescriptor::all_events(),
            supports_user_instances: true,
            supports_org_instances: true,
            requires_oauth: false,
            oauth_config: None,
            author: "FlowLink".into(),
            version: "1.0.0".into(),
        },
        IntegrationMeta {
            kind: IntegrationKind("webhook".into()),
            display_name: "Custom Webhook".into(),
            description: "Forward events to any HTTP endpoint.".into(),
            icon: "🔗".into(),
            category: IntegrationCategory::Custom,
            config_schema: serde_json::json!({
                "type": "object",
                "required": ["url"],
                "properties": {
                    "url": { "type": "string", "title": "Webhook URL" },
                    "secret": { "type": "string", "title": "Signing Secret" },
                    "headers": { "type": "object", "title": "Custom Headers" }
                }
            }),
            available_events: EventDescriptor::all_events(),
            supports_user_instances: true,
            supports_org_instances: true,
            requires_oauth: false,
            oauth_config: None,
            author: "FlowLink".into(),
            version: "1.0.0".into(),
        },
    ]
}

impl EventDescriptor {
    pub fn all_events() -> Vec<EventDescriptor> {
        vec![
            EventDescriptor { event_type: "agent_connected".into(), display_name: "Agent Connected".into(), description: "When a server agent connects".into() },
            EventDescriptor { event_type: "agent_disconnected".into(), display_name: "Agent Disconnected".into(), description: "When a server agent disconnects".into() },
            EventDescriptor { event_type: "shield_alert".into(), display_name: "Security Alert".into(), description: "When Shield detects a risky command".into() },
            EventDescriptor { event_type: "approval_requested".into(), display_name: "Approval Requested".into(), description: "When a command requires approval".into() },
            EventDescriptor { event_type: "approval_resolved".into(), display_name: "Approval Resolved".into(), description: "When an approval is resolved".into() },
            EventDescriptor { event_type: "payment_received".into(), display_name: "Payment Received".into(), description: "When a payment is processed".into() },
            EventDescriptor { event_type: "subscription_changed".into(), display_name: "Subscription Changed".into(), description: "When plan changes".into() },
            EventDescriptor { event_type: "system_alert".into(), display_name: "System Alert".into(), description: "System alerts and warnings".into() },
        ]
    }
}

// ═══════════════════════════════════════════════
// Tests
// ═══════════════════════════════════════════════


// ═══════════════════════════════════════════════
// Tests
// ═══════════════════════════════════════════════

