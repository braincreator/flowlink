pub mod models;
pub mod handlers;
pub mod router;
pub mod verification;
pub mod storage;
pub mod metrics;

pub use models::*;
pub use handlers::*;
pub use router::*;
pub use verification::*;
pub use storage::*;
pub use metrics::*;

use anyhow::Result;
use std::sync::Arc;
use tokio::sync::RwLock;
use hyper::{Body, Request, Response, Server};
use tower_http::cors::CorsLayer;
use tower_http::trace::TraceLayer;

// Main orchestrator for webhook receiver
pub struct WebhookReceiver {
    pub config: WebhookReceiverConfig,
    pub router: Arc<WebhookRouter>,
    pub storage: Arc<WebhookStorage>,
    pub metrics: Arc<WebhookMetrics>,
    pub server: Option<Server<hyper::body::Incoming, hyper::body::Incoming>>,
}

#[derive(Debug, Clone, serde::Deserialize, serde::Serialize)]
pub struct WebhookReceiverConfig {
    pub port: u16,
    pub public_url: String,
    pub max_webhook_size: usize,
    pub retention_days: i32,
    pub allowed_origins: Vec<String>,
    pub hmac_secrets: Vec<WebhookHmacSecret>,
    pub enable_metrics: bool,
    pub enable_storage: bool,
    pub routing_rules: Vec<RoutingRule>,
}

#[derive(Debug, Clone, serde::Deserialize, serde::Serialize)]
pub struct WebhookHmacSecret {
    pub service: String,
    pub secret: String,
}

#[derive(Debug, Clone, serde::Deserialize, serde::Serialize)]
pub struct RoutingRule {
    pub service: String,
    pub enabled: bool,
    pub target: RoutingTarget,
    pub filters: Vec<RoutingFilter>,
    pub rate_limit: Option<RateLimit>,
}

#[derive(Debug, Clone, serde::Deserialize, serde::Serialize)]
pub enum RoutingTarget {
    FlowLink,
    Discord { channel: String },
    Slack { channel: String },
    Webhook { url: String },
    Local { handler: String },
}

#[derive(Debug, Clone, serde::Deserialize, serde::Serialize)]
pub struct RoutingFilter {
    pub field: String,
    pub operator: FilterOperator,
    pub value: String,
}

#[derive(Debug, Clone, serde::Deserialize, serde::Serialize)]
pub enum FilterOperator {
    Equals,
    NotEquals,
    Contains,
    NotContains,
    StartsWith,
    EndsWith,
    Regex,
}

#[derive(Debug, Clone, serde::Deserialize, serde::Serialize)]
pub struct RateLimit {
    pub requests_per_minute: i32,
    pub burst_size: i32,
}

impl WebhookReceiver {
    pub async fn new(config: WebhookReceiverConfig) -> Result<Self> {
        let router = Arc::new(WebhookRouter::new(config.routing_rules.clone()));
        let storage = Arc::new(WebhookStorage::new(config.clone()));
        let metrics = Arc::new(WebhookMetrics::new(config.enable_metrics));
        
        Ok(Self {
            config,
            router,
            storage,
            metrics,
            server: None,
        })
    }
    
    pub async fn start(&self) -> Result<()> {
        log::info!("Starting webhook receiver on port {}", self.config.port);
        
        let port = self.config.port;
        let config = self.config.clone();
        let router = self.router.clone();
        let storage = self.storage.clone();
        let metrics = self.metrics.clone();
        
        // Setup CORS and tracing middleware
        let app = Router::new()
            .route("/webhook/:service", hyper::service::service_fn(move |req| {
                handle_webhook(req, router.clone(), storage.clone(), metrics.clone(), config.clone())
            }))
            .route("/health", hyper::service::service_fn(|_| async {
                Response::builder()
                    .status(200)
                    .header("content-type", "application/json")
                    .body(Body::from(r#"{"status": "ok", "service": "webhook-receiver"}"#))
                    .unwrap()
            }))
            .route("/metrics", hyper::service::service_fn(|_| async {
                // TODO: Return actual metrics
                Response::builder()
                    .status(200)
                    .header("content-type", "application/json")
                    .body(Body::from(r#"{"webhooks_received": 0}"#))
                    .unwrap()
            }))
            .layer(CorsLayer::permissive())
            .layer(TraceLayer::new_for_http());
        
        let addr = format!("0.0.0.0:{}", port).parse().unwrap();
        
        let server = Server::bind(&addr).serve(app.into_make_service());
        self.server = Some(server);
        
        tokio::spawn(async move {
            if let Err(e) = server.await {
                log::error!("Webhook receiver server error: {}", e);
            }
        });
        
        Ok(())
    }
    
    pub async fn stop(&self) -> Result<()> {
        log::info!("Stopping webhook receiver");
        
        if let Some(server) = self.server.take() {
            server.graceful_shutdown().await;
        }
        
        Ok(())
    }
    
    pub async fn get_stats(&self) -> WebhookStats {
        self.metrics.get_stats().await
    }
}

// Main webhook handler
async fn handle_webhook(
    req: Request<Body>,
    router: Arc<WebhookRouter>,
    storage: Arc<WebhookStorage>,
    metrics: Arc<WebhookMetrics>,
    config: WebhookReceiverConfig,
) -> Result<Response<Body>, hyper::Error> {
    let method = req.method();
    let path = req.uri().path();
    let headers = req.headers();
    
    // Only accept POST requests
    if method != hyper::Method::POST {
        return Ok(Response::builder()
            .status(405)
            .header("content-type", "application/json")
            .body(Body::from(r#"{"error": "Method not allowed"}"#))
            .unwrap());
    }
    
    // Extract service from path
    let service_path = path.strip_prefix("/webhook/").unwrap_or("");
    let service = service_path.split('/').next().unwrap_or("");
    
    if service.is_empty() {
        return Ok(Response::builder()
            .status(400)
            .header("content-type", "application/json")
            .body(Body::from(r#"{"error": "Service not specified"}"#))
            .unwrap());
    }
    
    // Read webhook body
    let body = hyper::body::to_bytes(req.into_body()).await?;
    let webhook_data = String::from_utf8_lossy(&body);
    
    // Log webhook
    log::debug!("Received webhook from service {}: {} bytes", service, body.len());
    
    // Parse and process webhook
    let webhook = Webhook {
        id: uuid::Uuid::new_v4().to_string(),
        service: service.to_string(),
        data: webhook_data.to_string(),
        timestamp: chrono::Utc::now(),
        headers: headers.clone(),
        ip_address: None, // TODO: Extract from req
    };
    
    // Store webhook
    if config.enable_storage {
        if let Err(e) = storage.store_webhook(&webhook).await {
            log::error!("Failed to store webhook: {}", e);
        }
    }
    
    // Increment metrics
    metrics.increment_received(service).await;
    
    // Route webhook
    let routing_result = router.route_webhook(&webhook).await;
    
    match routing_result {
        Ok(_) => {
            metrics.increment_routed(service).await;
            Ok(Response::builder()
                .status(200)
                .header("content-type", "application/json")
                .body(Body::from(r#"{"status": "accepted"}"#))
                .unwrap())
        }
        Err(e) => {
            log::error!("Failed to route webhook: {}", e);
            metrics.increment_failed(service).await;
            Ok(Response::builder()
                .status(400)
                .header("content-type", "application/json")
                .body(Body::from(format!(r#"{{"error": "Routing failed: {}"}}"#, e)))
                .unwrap())
        }
    }
}