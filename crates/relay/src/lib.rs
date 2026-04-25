#![allow(unexpected_cfgs)]
pub mod pool;
pub mod auth;
pub mod auth_oauth;
pub mod auth_2fa;
pub mod handler;
pub mod eventbus;
pub mod approval;
pub mod ratelimit;
pub mod auth_rate_limiter;
pub mod auth_rate_middleware;
pub mod rate_limiter;
pub mod audit;
pub mod registry;
pub mod llm;
pub mod middleware;
pub mod tls;
pub mod server;
pub mod mcp;
pub mod devices;
pub mod rbac_manager;
pub mod metrics;
pub mod billing_api;
pub mod billing_persist;
pub mod billing_middleware;
pub mod plan_gate;
pub mod plan_enforcement;
pub mod email;
pub mod email_auth;
pub mod notifications;
pub mod email_queue;
pub mod preferences_api;
pub mod notifications_api;
pub mod account_deletion_api;
pub mod orgs_api;
pub mod webhooks_api;
pub mod dashboard;

pub mod config_reload;
pub mod e2ee;
pub mod control_plane;

#[cfg(feature = "tgbot")]
pub mod tgbot;

use std::sync::Arc;
use std::path::PathBuf;
use log::info;
use tokio::sync::RwLock;
use tokio_rustls::TlsAcceptor;
use flowlink_core::config::RelayConfig;

/// Returns the configured server base URL (from env or default).
/// Use this in places where RelayConfig is not directly accessible.
pub fn server_base_url() -> String {
    std::env::var("SERVER_URL").unwrap_or_else(|_| "https://flowlink.flow-masters.ru".to_string())
}

use crate::approval::ApprovalQueue;
use crate::auth::AuthManager;
use crate::eventbus::EventBus;
use crate::handler::RelayHandler;
use crate::pool::AgentPool;
use crate::registry::Registry;
use crate::devices::DeviceManager;
use crate::llm::LlmProxy;
use crate::server::AppState;
use crate::tls::{self as relay_tls};
use tower::ServiceExt;

pub struct Relay {
    config: RelayConfig,
    /// Optional path to the config file for hot-reload.
    /// If set, the relay will watch the file and auto-reload on changes.
    pub config_path: Option<PathBuf>,
}

impl Relay {
    pub fn new(config: RelayConfig) -> Self {
        Self { config, config_path: None }
    }

    /// Set the config file path for hot-reload.
    pub fn with_config_path(mut self, path: impl Into<PathBuf>) -> Self {
        self.config_path = Some(path.into());
        self
    }

    pub async fn run(&self) -> anyhow::Result<()> {
        let pool = Arc::new(AgentPool::new());
        let eventbus = Arc::new(EventBus::new());
        let approvals = Arc::new(ApprovalQueue::new());

        // AuthEngine for JWT tokens (requires jwt_secret + database)
        // Initialized after db setup — see below
        let _auth_engine: Option<Arc<crate::auth::AuthEngine>>;

        let data_dir = shellexpand::tilde(&self.config.registry.data_path).to_string();
        let registry = Arc::new(Registry::new(&data_dir)?);

        let llm_proxy = if self.config.llm.enabled {
            Some(Arc::new(LlmProxy::new(
                self.config.llm.backends.clone(),
                self.config.llm.timeout_sec,
            )))
        } else {
            None
        };

        // Database (optional — PostgreSQL)
        let db_config = &self.config.database;
        let db = if let Some(url) = &db_config.primary {
            match flowlink_db::DbPool::open(url, &db_config.replicas).await {
                Ok(pool) => {
                    if db_config.migrate_on_start {
                        if let Err(e) = pool.run_migrations().await {
                            log::warn!("Database migrations failed: {e}. Continuing without DB.");
                            None
                        } else {
                            log::info!("Database connected & migrations applied");
                            Some(Arc::new(pool))
                        }
                    } else {
                        log::info!("Database connected (migrations skipped)");
                        Some(Arc::new(pool))
                    }
                }
                Err(e) => {
                    log::warn!("Database connection failed: {e}. Running without DB.");
                    None
                }
            }
        } else {
            None
        };

        // AuthManager — loads agent tokens from DB on startup
        let auth = Arc::new(AuthManager::new(db.as_ref().map(|d| Arc::new(d.write_pool.clone()))));

        let handler = Arc::new(RelayHandler::new(
            pool.clone(), auth.clone(), eventbus.clone(), approvals.clone(),
        ));

        // Initialize AuthEngine now that db is available
        let auth_engine = if !self.config.auth.jwt_secret.is_empty() {
            match &db {
                Some(db_pool) => {
                    let engine = Arc::new(crate::auth::AuthEngine::new(
                        crate::auth::AuthConfig {
                            jwt_secret: self.config.auth.jwt_secret.clone(),
                            access_token_ttl_min: self.config.auth.access_token_ttl_min,
                            refresh_token_ttl_days: self.config.auth.refresh_token_ttl_days,
                            vk: None,
                            yandex: None,
                            github: None,
                        },
                        db_pool.pool().clone(),
                    ));
                    log::info!("🔐 JWT auth engine initialized");
                    Some(engine)
                }
                None => {
                    log::warn!("JWT auth disabled: database not configured");
                    None
                }
            }
        } else {
            None
        };

        // Config hot-reload (optional — requires config path)
        let metrics = Arc::new(metrics::Metrics::new());
        let rate_limits_config = Arc::new(std::sync::RwLock::new(crate::rate_limiter::RateLimitsConfig::default()));
        let config_reloader = if let Some(config_path) = &self.config_path {
            if config_path.exists() {
                let shared_config = Arc::new(RwLock::new(self.config.clone()));
                let reloader = Arc::new(crate::config_reload::ConfigReloader::new(
                    config_path.clone(),
                    shared_config,
                    handler.clone(),
                    metrics.clone(),
                    rate_limits_config.clone(),
                ));
                // Start file watcher in background
                match reloader.clone().start_watcher() {
                    Ok(_handle) => {
                        info!("Config hot-reload enabled, watching {}", config_path.display());
                        Some(reloader)
                    }
                    Err(e) => {
                        log::warn!("Config watcher failed to start: {e}. Manual reload still available.");
                        Some(reloader)
                    }
                }
            } else {
                log::warn!("Config path {} not found, hot-reload disabled", config_path.display());
                None
            }
        } else {
            None
        };

        let state = AppState {
            start_time: std::time::Instant::now(),
            pool, approvals, eventbus, handler, registry,
            device_manager: Arc::new(DeviceManager::new(devices::PushConfig::default())),
            llm_proxy,
            shield_alerts: Arc::new(server::ShieldAlertManager::new()),
            audit_store: Arc::new(audit::AuditStore::new(
                std::path::Path::new(&shellexpand::tilde("~/.flowlink/audit.jsonl").to_string()),
                db.clone(),
            )),
            metrics,
            tg_bot: std::sync::OnceLock::new(),
            billing: if self.config.billing.enabled {
                let payment_config = {
                    let bc = &self.config.billing;
                    let mut pc = flowlink_billing::payment::PaymentConfig::default();
                    pc.enabled = bc.enabled;
                    if let (Some(client_id), Some(secret_key)) = (&bc.tochka_client_id, &bc.tochka_webhook_secret) {
                        pc.sbp = Some(flowlink_billing::payment::SbpConfig {
                            terminal_key: client_id.clone(),
                            secret_key: secret_key.clone(),
                            payment_type_id: "SBP".to_string(),
                            callback_url: format!("{}/api/billing/webhook/tochka", server_base_url()),
                            success_url: format!("{}/billing/success", server_base_url()),
                            fail_url: format!("{}/billing/fail", server_base_url()),
                        });
                    }
                    pc
                };
                let engine = if let Some(ref db_pool) = db {
                    // Use DbPersist for real DB-backed billing
                    let persist = Arc::new(crate::billing_persist::DbPersist::new(db_pool.pool().clone()));
                    Arc::new(flowlink_billing::BillingEngine::with_persist(payment_config, persist))
                } else {
                    Arc::new(flowlink_billing::BillingEngine::new(payment_config))
                };
                // Load plans from DB if available
                if let Some(ref db_pool) = db {
                    engine.plans().load_from_db(db_pool).await;
                }
                // Load all account billing data from DB
                engine.load_all().await.ok();
                Some(engine)
            } else {
                None
            },
            db,
            tochka: if let (Some(jwt_token), Some(customer_code), Some(merchant_id)) = (&self.config.billing.tochka_jwt_token, &self.config.billing.tochka_customer_code, &self.config.billing.tochka_merchant_id) {
                Some(Arc::new(flowlink_billing::tochka::TochkaClient::new(
                    flowlink_billing::payment::SbpConfig {
                        terminal_key: customer_code.clone(), // customer_code for API
                        secret_key: jwt_token.clone(), // JWT token for auth
                        payment_type_id: merchant_id.clone(), // merchantId for acquiring
                        callback_url: format!("{}/api/billing/webhook/tochka", server_base_url()),
                        success_url: format!("{}/billing/success", server_base_url()),
                        fail_url: format!("{}/billing/fail", server_base_url()),
                    },
                )))
            } else {
                None
            },
            auth: auth.clone(),
            auth_engine,
            email_service: if !self.config.smtp.host.is_empty() && !self.config.smtp.username.is_empty() {
                match crate::email::EmailService::new(
                    &self.config.smtp.host,
                    self.config.smtp.port,
                    &self.config.smtp.username,
                    &self.config.smtp.password,
                    &self.config.smtp.from,
                ) {
                    Ok(svc) => Some(Arc::new(svc)),
                    Err(e) => {
                        log::warn!("Failed to create email service: {e}");
                        None
                    }
                }
            } else {
                None
            },
            config_reloader,
            e2ee: Arc::new(crate::e2ee::E2eeSessionManager::new()),
            usage_tracker: Arc::new(crate::billing_middleware::UsageTracker::new()),
            rate_limiter: Arc::new(ratelimit::RateLimiter::new(100, 10)),
            control_plane: crate::control_plane::ControlPlaneState::new(),
            email_queue: std::sync::OnceLock::new(),
            notification_router: std::sync::OnceLock::new(),
            notification_store: Some(Arc::new(crate::preferences_api::NotificationStore::new())),
            rbac: Arc::new(crate::rbac_manager::RbacManager::new()),
            auth_rate_limiter: Arc::new(crate::auth_rate_limiter::AuthRateLimiter::new()),
            tiered_rate_limiter: Arc::new(crate::rate_limiter::TieredRateLimiter::new()),
            rate_limits_config: rate_limits_config.clone(),
            key_rate_limiter: Arc::new(crate::api_keys::KeyRateLimiter::new(100, 60)), // 100 req/min per key
            saml_config: None,
            rusiem_config: None,
            vault: None,
            http_client: reqwest::Client::new(),
            cors_origins: self.config.cors_allowed_origins.clone(),
        };

        // Email queue worker (requires both email_service and db)
        if let (Some(ref email_svc), Some(ref db_pool)) = (&state.email_service, &state.db) {
            let queue = Arc::new(crate::email_queue::EmailQueue::new(
                email_svc.clone(),
                db_pool.pool().clone(),
            ));
            let tiered_rl = state.tiered_rate_limiter.clone();
            queue.clone().start_worker(move || tiered_rl.cleanup());
            let _ = state.email_queue.set(queue);
            log::info!("📧 Email queue initialized");
        }

        // ── Approval timeout background task ──
        // Scans pending approvals every 10s and marks timed-out ones.
        {
            let approvals_bg = state.approvals.clone();
            let handler_bg = state.handler.clone();
            let db_bg = state.db.clone();
            tokio::spawn(async move {
                let mut interval = tokio::time::interval(std::time::Duration::from_secs(10));
                interval.tick().await; // skip first
                loop {
                    interval.tick().await;
                    let timed_out = approvals_bg.take_timed_out(300); // 5 min default
                    for req in timed_out {
                        log::warn!("Approval timed out: {} agent={} cmd={:?}", req.id, req.agent_id, req.command);
                        // Notify agent via WS
                        let _ = handler_bg.send_to_agent(&req.agent_id,
                            flowlink_core::Message::new(flowlink_core::MessageType::ExecReject)
                                .with_agent_id(&req.agent_id)
                                .with_payload(serde_json::json!({
                                    "request_id": req.id,
                                    "decision": "timed_out",
                                    "approved": false,
                                    "reason": "approval request timed out (300s)",
                                }))
                        ).await;
                        // Update DB
                        if let Some(ref db) = db_bg {
                            let _ = sqlx::query(
                                "UPDATE approval_log SET status = 'timed_out', resolved_at = NOW() WHERE id = $1 AND status = 'pending'"
                            )
                            .bind(&req.id)
                            .execute(db.write_pool())
                            .await;
                        }
                    }
                }
            });
        }

        let app = server::build_router(state.clone());

        // ── Periodic maintenance tasks ──
        {
            let rate_limiter = state.rate_limiter.clone();
            let audit_store = state.audit_store.clone();
            let db_bg = state.db.clone();
            let agent_pool = state.pool.clone();
            let eventbus = state.eventbus.clone();
            tokio::spawn(async move {
                let mut interval = tokio::time::interval(std::time::Duration::from_secs(600)); // every 10 min
                interval.tick().await; // skip first
                loop {
                    interval.tick().await;
                    // Prune stale rate limiter buckets
                    rate_limiter.prune();
                    // Prune audit events older than 90 days
                    let pruned = audit_store.prune(std::time::Duration::from_secs(90 * 24 * 3600));
                    if pruned > 0 {
                        log::info!("Maintenance: pruned {} old audit events", pruned);
                    }
                    // Check subscription expiry (if DB available)
                    if let Some(ref db) = db_bg {
                        if let Err(e) = crate::billing_api::check_expiry_bg(db.pool()).await {
                            log::warn!("Maintenance: subscription expiry check failed: {e}");
                        }
                    }
                    // Prune agents offline for >7 days
                    let pruned_agents = agent_pool.prune_offline(7 * 24 * 3600);
                    if pruned_agents > 0 {
                        log::info!("Maintenance: pruned {} stale offline agents", pruned_agents);
                    }
                    // Prune expired tokens/codes from DB
                    if let Some(ref db) = db_bg {
                        let _ = sqlx::query("DELETE FROM linking_codes WHERE expires_at < NOW()").execute(db.pool()).await;
                        let _ = sqlx::query("DELETE FROM email_verification_codes WHERE expires_at < NOW()").execute(db.pool()).await;
                        let _ = sqlx::query("DELETE FROM org_invitations WHERE expires_at < NOW() AND status = 'pending'").execute(db.pool()).await;
                    }
                    // Prune empty event bus channels
                    eventbus.prune_empty();
                }
            });
        }

        let addr = self.config.http_addr;
        info!("relay listening on {addr} (HTTP API)");

        let http_listener = tokio::net::TcpListener::bind(addr).await?;
        let http_server = axum::serve(http_listener, app)
            .with_graceful_shutdown(shutdown_signal());

        // ── WSS TLS listener (required) ──
        // Realtime endpoint for agents. Cert+key are mandatory — fail fast if missing.
        if !self.config.wss_tls.is_enabled() {
            return Err(anyhow::anyhow!(
                "wss_tls: cert_path and key_path are required (wss_addr={})",
                self.config.wss_addr
            ));
        }
        let cert_path = self.config.wss_tls.cert_path.as_deref().unwrap();
        let key_path = self.config.wss_tls.key_path.as_deref().unwrap();
        let wss_addr = self.config.wss_addr;

        let tls_config = relay_tls::build_tls_server_config(&relay_tls::TlsConfig {
                cert_path: cert_path.to_string(),
                key_path: key_path.to_string(),
                ca_path: None,
            })?;

        let tls_acceptor = TlsAcceptor::from(tls_config);

        info!("WSS TLS listener on {wss_addr} (cert: {cert_path})");

        let wss_listener = tokio::net::TcpListener::bind(wss_addr).await?;
        let wss_app = server::build_router(state.clone());

        tokio::spawn(async move {
            loop {
                let acceptor = tls_acceptor.clone();
                let app = wss_app.clone();
                tokio::select! {
                    result = wss_listener.accept() => {
                        match result {
                            Ok((tcp_stream, peer_addr)) => {
                                info!("WSS TLS connection from {peer_addr}");
                                match acceptor.accept(tcp_stream).await {
                                    Ok(tls_stream) => {
                                        let app = app.clone();
                                        tokio::spawn(async move {
                                            let io = hyper_util::rt::TokioIo::new(tls_stream);
                                            let svc = hyper::service::service_fn(move |req| {
                                                let app = app.clone();
                                                async move {
                                                    app.oneshot(req).await
                                                }
                                            });
                                            if let Err(e) = hyper_util::server::conn::auto::Builder::new(hyper_util::rt::TokioExecutor::new())
                                                .serve_connection_with_upgrades(io, svc)
                                                .await
                                            {
                                                log::warn!("WSS TLS serve error: {e}");
                                            }
                                        });
                                    }
                                    Err(e) => {
                                        log::warn!("WSS TLS handshake failed from {peer_addr}: {e}");
                                    }
                                }
                            }
                            Err(e) => {
                                log::error!("WSS listener accept error: {e}");
                            }
                        }
                    }
                    _ = shutdown_signal() => {
                        info!("WSS TLS listener shutting down");
                        break;
                    }
                }
            }
        });

        // ── Telegram Bot (optional) ──
        #[cfg(feature = "tgbot")]
        if let Some(tg_token) = &self.config.tg_bot_token {
            if !tg_token.is_empty() {
                let bot_state = Arc::new(state.clone());
                let token = tg_token.clone();
                let bot_config = crate::tgbot::bot::BotConfig {
                    mode: crate::tgbot::bot::BotMode::Polling,
                    webhook_url: None,
                    polling_interval: std::time::Duration::from_secs(5),
                    auto_recovery_enabled: true,
                };
                // Store bot instance for notifications
                let _ = bot_state.tg_bot.set(teloxide::Bot::new(&token));
                crate::tgbot::start_tgbot(bot_state, token, bot_config).await;
                info!("Telegram bot started");

                // Initialize notification router from env with DB pool
                let pool_clone = state.db.as_ref().map(|d| d.pool().clone());
                let router = crate::notifications::NotificationRouter::from_env(pool_clone);
                if router.channel_count() > 0 {
                    let router = std::sync::Arc::new(router);
                    let _ = state.notification_router.set(router.clone());

                    // Start plan reload listener — admin changes → hot reload billing cache
                    let reload_state = state.clone();
                    tokio::spawn(async move {
                        let mut rx = reload_state.eventbus.subscribe("plans:updated");
                        log::info!("\u{1f4e6} Plan reload listener active");
                        while let Ok(msg) = rx.recv().await {
                            if let Some(ref billing) = reload_state.billing {
                                if let Some(ref db) = reload_state.db {
                                    billing.plans().load_from_db(&*db).await;
                                    log::info!("\u{1f4e6} Plans reloaded: {}", msg);
                                }
                            }
                        }
                        log::warn!("\u{1f4e6} Plan reload listener exited");
                    });

                    // Start shield alert → notification channel forwarder
                    let notify_state = state.clone();
                    let notify_router = router.clone();
                    tokio::spawn(async move {
                        let mut rx = notify_state.eventbus.subscribe("shield_alert");
                        log::info!("🛡️ Shield → notification channels active ({} channel(s))", notify_router.channel_count());
                        while let Ok(msg) = rx.recv().await {
                            if let Ok(v) = serde_json::from_str::<serde_json::Value>(&msg) {
                                // Resolve agent_id → account_id via registry
                                let account_id = v.get("agent_id")
                                    .and_then(|a| a.as_str())
                                    .and_then(|agent_id| {
                                        notify_state.registry.get_agent(agent_id)
                                            .map(|agent| agent.client_id)
                                    })
                                    .unwrap_or_default();

                                let mut notification = crate::notifications::Notification::shield_alert(
                                    &account_id,
                                    v.get("pid").and_then(|p| p.as_i64()).unwrap_or(0) as i32,
                                    v.get("username").and_then(|u| u.as_str()).unwrap_or("?"),
                                    v.get("command").and_then(|c| c.as_str()).unwrap_or("?"),
                                    v.get("rule_name").and_then(|r| r.as_str()).unwrap_or("?"),
                                    v.get("action").and_then(|a| a.as_str()).unwrap_or("?"),
                                );

                                // If no account resolved, also mark for global (admin) delivery
                                if account_id.is_empty() {
                                    notification.tags.push("global_fallback".into());
                                }

                                notify_router.send(&notification).await;
                            }
                        }
                    });

                    // Start approval → notification channel forwarder
                    let approve_state = state.clone();
                    let approve_router = router.clone();
                    tokio::spawn(async move {
                        let mut rx = approve_state.eventbus.subscribe("approval_request");
                        log::info!("📋 Approval → notification channels active");
                        while let Ok(msg) = rx.recv().await {
                            if let Ok(v) = serde_json::from_str::<serde_json::Value>(&msg) {
                                let account_id = v.get("agent_id")
                                    .and_then(|a| a.as_str())
                                    .and_then(|agent_id| {
                                        approve_state.registry.get_agent(agent_id)
                                            .map(|agent| agent.client_id)
                                    })
                                    .unwrap_or_default();

                                let body = format!(
                                    "<b>📋 Approval Request</b>\nAgent: {}\nCommand: <code>{}</code>\n{}",
                                    v.get("agent_id").and_then(|a| a.as_str()).unwrap_or("?"),
                                    v.get("command").and_then(|c| c.as_str()).unwrap_or("?"),
                                    v.get("reason").and_then(|r| r.as_str()).unwrap_or(""),
                                );

                                let notification = crate::notifications::Notification {
                                    id: uuid::Uuid::new_v4().to_string(),
                                    account_id,
                                    severity: crate::notifications::Severity::Warning,
                                    category: crate::notifications::Category::Audit,
                                    subject: "Approval Required".into(),
                                    body,
                                    data: std::collections::HashMap::new(),
                                    timestamp: chrono::Utc::now().to_rfc3339(),
                                    tags: vec!["approval".into(), "audit".into()],
                                };

                                approve_router.send(&notification).await;
                            }
                        }
                    });
                }
            }
        }

        http_server.await?;

        info!("relay shut down");
        Ok(())
    }
}

async fn shutdown_signal() {
    let ctrl_c = async {
        tokio::signal::ctrl_c()
            .await
            .expect("failed to listen for ctrl-c");
    };
    #[cfg(unix)]
    let terminate = async {
        tokio::signal::unix::signal(tokio::signal::unix::SignalKind::terminate())
            .expect("failed to install SIGTERM handler")
            .recv()
            .await;
    };
    #[cfg(not(unix))]
    let terminate = std::future::pending::<()>();

    tokio::select! {
        _ = ctrl_c => {},
        _ = terminate => {},
    }
    info!("Shutting down...");
}
pub mod playground;
pub mod policy_db;
pub mod api_keys;
pub mod saml;
pub mod rusiem;
pub mod custom_roles_api;
pub mod agent_tags_api;
pub mod command_history_api;
pub mod agent_health_api;
pub mod webhook_delivery;
pub mod sessions_api;
pub mod secrets_api;
pub mod secret_mappings_api;
pub mod compliance_api;
pub mod fstek;
pub mod discovery_api;
pub mod vault_client;
pub mod zero_trust_secrets;
pub mod zero_trust_api;
pub mod infra_map;
pub mod infra_map_api;
pub mod health_monitor;
pub mod health_monitor_api;
