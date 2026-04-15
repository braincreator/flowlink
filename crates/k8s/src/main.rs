use anyhow::Result;
use axum::{
    extract::State,
    routing::{get, post},
    Json, Router,
};
use base64::Engine;
use clap::Parser;
use kube::Client;
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use tokio::signal;

use flowlink_k8s::config::K8sConfig;
use flowlink_k8s::operator::ShieldOperator;
use flowlink_k8s::webhook::{
    AdmissionRequest, AdmissionResponse, AdmissionResponseStatus, AdmissionWebhook,
};

#[derive(Parser, Debug)]
#[command(name = "flowlink-k8s", about = "FlowLink Shield Kubernetes Operator")]
struct Cli {
    /// Namespace to run in
    #[arg(long, env = "FLOWLINK_NAMESPACE")]
    namespace: Option<String>,

    /// FlowLink Relay URL
    #[arg(long, env = "FLOWLINK_RELAY_URL")]
    relay_url: Option<String>,

    /// Shield mode (monitor|enforce)
    #[arg(long, env = "FLOWLINK_MODE")]
    mode: Option<String>,

    /// Webhook port
    #[arg(long, env = "FLOWLINK_WEBHOOK_PORT", default_value = "9443")]
    webhook_port: u16,

    /// Directory for TLS certs
    #[arg(long, env = "FLOWLINK_CERT_DIR")]
    cert_dir: Option<String>,

    /// Sidecar image
    #[arg(long, env = "FLOWLINK_SIDECAR_IMAGE")]
    sidecar_image: Option<String>,
}

#[derive(Clone)]
struct AppState {
    webhook: Arc<AdmissionWebhook>,
    #[allow(dead_code)]
    config: K8sConfig,
}

#[allow(non_snake_case)]
#[derive(Debug, Serialize, Deserialize)]
struct AdmissionReview {
    apiVersion: String,
    kind: String,
    request: Option<AdmissionRequest>,
    response: Option<AdmissionResponse>,
}

async fn healthz() -> &'static str {
    "ok"
}

async fn mutate_handler(
    State(state): State<AppState>,
    Json(review): Json<AdmissionReview>,
) -> Json<AdmissionReview> {
    let uid = review
        .request
        .as_ref()
        .map(|r| r.uid.clone())
        .unwrap_or_default();

    match &review.request {
        Some(req) => {
            let (resp, patch) = state.webhook.handle_review(req);

            let (patch_type, patch_b64) = match patch {
                Some(p) => {
                    let patch_json = serde_json::to_string(&p).unwrap_or_default();
                    let b64 = base64::engine::general_purpose::STANDARD.encode(patch_json);
                    (Some("JSONPatch".to_string()), Some(b64))
                }
                None => (None, None),
            };

            Json(AdmissionReview {
                apiVersion: "admission.k8s.io/v1".into(),
                kind: "AdmissionReview".into(),
                request: None,
                response: Some(AdmissionResponse {
                    uid: resp.uid,
                    allowed: resp.allowed,
                    status: resp.status,
                    patch_type,
                    patch: patch_b64,
                }),
            })
        }
        None => Json(AdmissionReview {
            apiVersion: "admission.k8s.io/v1".into(),
            kind: "AdmissionReview".into(),
            request: None,
            response: Some(AdmissionResponse {
                uid,
                allowed: false,
                status: Some(AdmissionResponseStatus {
                    code: Some(400),
                    message: Some("No request in review".into()),
                }),
                patch_type: None,
                patch: None,
            }),
        }),
    }
}

async fn validate_handler(
    State(state): State<AppState>,
    Json(review): Json<AdmissionReview>,
) -> Json<AdmissionReview> {
    // Validation uses the same logic, just no mutation
    let uid = review
        .request
        .as_ref()
        .map(|r| r.uid.clone())
        .unwrap_or_default();

    match &review.request {
        Some(req) => {
            let (resp, _) = state.webhook.handle_review(req);
            Json(AdmissionReview {
                apiVersion: "admission.k8s.io/v1".into(),
                kind: "AdmissionReview".into(),
                request: None,
                response: Some(resp),
            })
        }
        None => Json(AdmissionReview {
            apiVersion: "admission.k8s.io/v1".into(),
            kind: "AdmissionReview".into(),
            request: None,
            response: Some(AdmissionResponse {
                uid,
                allowed: false,
                status: Some(AdmissionResponseStatus {
                    code: Some(400),
                    message: Some("No request in review".into()),
                }),
                patch_type: None,
                patch: None,
            }),
        }),
    }
}

#[tokio::main]
async fn main() -> Result<()> {
    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("info")).init();

    let cli = Cli::parse();

    let mut config = K8sConfig::default();
    if let Some(ns) = cli.namespace {
        config.namespace = ns;
    }
    if let Some(url) = cli.relay_url {
        config.relay_url = url;
    }
    if let Some(mode) = cli.mode {
        config.mode = match mode.to_lowercase().as_str() {
            "enforce" => flowlink_k8s::crd::ShieldMode::Enforce,
            _ => flowlink_k8s::crd::ShieldMode::Monitor,
        };
    }
    config.webhook_port = cli.webhook_port;
    if let Some(dir) = cli.cert_dir {
        config.cert_dir = dir;
    }
    if let Some(img) = cli.sidecar_image {
        config.sidecar_image = img;
    }

    log::info!(
        "FlowLink K8s Operator starting in namespace {}",
        config.namespace
    );

    // Try to connect to Kubernetes — fall back to webhook-only mode if unavailable
    let k8s_client: Option<Client> = match Client::try_default().await {
        Ok(c) => {
            log::info!("Connected to Kubernetes cluster");
            Some(c)
        }
        Err(e) => {
            log::warn!(
                "Cannot connect to Kubernetes ({}), running in webhook-only mode",
                e
            );
            None
        }
    };

    // Spawn the operator in a background task if K8s is available
    if let Some(client) = k8s_client {
        let operator = ShieldOperator::new(client, config.clone());
        tokio::spawn(async move {
            if let Err(e) = operator.run().await {
                log::error!("Operator failed: {}", e);
            }
        });
        log::info!("ShieldOperator spawned");
    } else {
        log::warn!("ShieldOperator not started — K8s unavailable, webhook-only mode");
    }

    let webhook = Arc::new(AdmissionWebhook::new(config.clone(), vec![]));
    let state = AppState {
        webhook,
        config: config.clone(),
    };

    let app = Router::new()
        .route("/healthz", get(healthz))
        .route("/mutate", post(mutate_handler))
        .route("/validate", post(validate_handler))
        .with_state(state);

    let addr = format!("0.0.0.0:{}", config.webhook_port);
    log::info!("Webhook server listening on {}", addr);

    let listener = tokio::net::TcpListener::bind(&addr).await?;
    axum::serve(listener, app)
        .with_graceful_shutdown(shutdown_signal())
        .await?;

    log::info!("Shutdown complete");
    Ok(())
}

async fn shutdown_signal() {
    signal::ctrl_c()
        .await
        .expect("Failed to install CTRL+C signal handler");
    log::info!("Received shutdown signal");
}
