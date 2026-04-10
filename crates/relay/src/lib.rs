pub mod pool;
pub mod auth;
pub mod handler;
pub mod eventbus;
pub mod approval;
pub mod ratelimit;
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

pub mod config_reload;
pub mod e2ee;
pub mod control_plane;

use std::sync::Arc;
use std::path::PathBuf;
use log::info;
use tokio::sync::RwLock;
use tokio_rustls::TlsAcceptor;
use flowlink_core::config::RelayConfig;

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
        let auth = Arc::new(AuthManager::new());
        let eventbus = Arc::new(EventBus::new());
        let approvals = Arc::new(ApprovalQueue::new());

        let data_dir = shellexpand::tilde(&self.config.registry.data_path).to_string();
        let registry = Arc::new(Registry::new(&data_dir)?);

        let handler = Arc::new(RelayHandler::new(
            pool.clone(), auth.clone(), eventbus.clone(), approvals.clone(),
        ));

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

        // Config hot-reload (optional — requires config path)
        let metrics = Arc::new(metrics::Metrics::new());
        let config_reloader = if let Some(config_path) = &self.config_path {
            if config_path.exists() {
                let shared_config = Arc::new(RwLock::new(self.config.clone()));
                let reloader = Arc::new(crate::config_reload::ConfigReloader::new(
                    config_path.clone(),
                    shared_config,
                    handler.clone(),
                    metrics.clone(),
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
            pool, approvals, eventbus, handler, registry,
            device_manager: Arc::new(DeviceManager::new(devices::PushConfig::default())),
            llm_proxy,
            shield_alerts: Arc::new(server::ShieldAlertManager::new()),
            audit_store: Arc::new(audit::AuditStore::new(
                std::path::Path::new(&shellexpand::tilde("~/.flowlink/audit.jsonl").to_string()),
                db.clone(),
            )),
            metrics,
            billing: if self.config.billing.enabled {
                let engine = Arc::new(flowlink_billing::BillingEngine::new(
                    flowlink_billing::payment::PaymentConfig::default(),
                ));
                // Load plans from DB if available
                if let Some(ref db_pool) = db {
                    engine.plans().load_from_db(db_pool).await;
                }
                Some(engine)
            } else {
                None
            },
            db,
            config_reloader,
            e2ee: Arc::new(crate::e2ee::E2eeSessionManager::new()),
            usage_tracker: Arc::new(crate::billing_middleware::UsageTracker::new()),
            rate_limiter: Arc::new(ratelimit::RateLimiter::new(100, 10)),
            control_plane: crate::control_plane::ControlPlaneState::new(),
        };

        let app = server::build_router(state.clone());

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
                                            if let Err(e) = hyper::server::conn::http1::Builder::new()
                                                .serve_connection(io, svc)
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
