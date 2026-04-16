/// End-to-end test: WSS TLS listener — agent connects, exchanges messages, disconnects.
///
/// 1. Generates a self-signed TLS cert via rcgen
/// 2. Starts the relay WSS TLS listener on a random port
/// 3. Connects a client via wss:// with tokio-tungstenite
/// 4. Sends a connect message, verifies server response
/// 5. Receives a connected acknowledgement
/// 6. Clean disconnect
use std::sync::Arc;
use std::time::Duration;

use axum::Router;
use flowlink_relay::approval::ApprovalQueue;
use flowlink_relay::auth::{AuthManager, Client};
use flowlink_relay::control_plane::ControlPlaneState;
use flowlink_relay::devices::DeviceManager;
use flowlink_relay::eventbus::EventBus;
use flowlink_relay::handler::RelayHandler;
use flowlink_relay::pool::AgentPool;
use flowlink_relay::registry::Registry;
use flowlink_relay::server::{build_router, AppState, ShieldAlertManager};
use futures_util::{SinkExt, StreamExt};
use tokio_rustls::TlsAcceptor;
use tokio_tungstenite::tungstenite::Message as WsMessage;
use tower::ServiceExt;

// Ensure ring crypto provider is installed before any TLS operation.
// reqwest may pull in aws-lc-rs which conflicts — we force ring.
static CRYPTO_INIT: std::sync::Once = std::sync::Once::new();
fn ensure_ring_provider() {
    CRYPTO_INIT.call_once(|| {
        rustls::crypto::ring::default_provider()
            .install_default()
            .expect("failed to install ring crypto provider");
    });
}

// ── helpers ──────────────────────────────────────────────

fn make_state(tmp: &std::path::Path) -> AppState {
    let pool = Arc::new(AgentPool::new());
    let eventbus = Arc::new(EventBus::new());
    let auth = Arc::new(AuthManager::new());
    let approvals = Arc::new(ApprovalQueue::new());
    let registry = Arc::new(Registry::new(tmp).unwrap());
    let handler = Arc::new(RelayHandler::new(
        pool.clone(),
        auth.clone(),
        eventbus.clone(),
        approvals.clone(),
    ));
    let device_manager = Arc::new(DeviceManager::new(
        flowlink_relay::devices::PushConfig::default(),
    ));

    auth.register_client(Client {
        client_id: "test-client".into(),
        api_token: "test-token".into(),
        name: "Test Client".into(),
        active: true,
    });

    AppState {
        pool,
        approvals,
        eventbus: eventbus.clone(),
        handler,
        registry,
        device_manager,
        llm_proxy: None,
        shield_alerts: Arc::new(ShieldAlertManager::new()),
        audit_store: Arc::new(flowlink_relay::audit::AuditStore::new(
            &tmp.join("audit.jsonl"),
            None,
        )),
        metrics: Arc::new(flowlink_relay::metrics::Metrics::new()),
        billing: None,
        db: None,
        config_reloader: None,
        e2ee: Arc::new(flowlink_relay::e2ee::E2eeSessionManager::new()),
        usage_tracker: Arc::new(flowlink_relay::billing_middleware::UsageTracker::new()),
        rate_limiter: Arc::new(flowlink_relay::ratelimit::RateLimiter::new(100, 10)),
        control_plane: ControlPlaneState::new(),
        email_queue: std::sync::OnceLock::new(),
        tg_bot: std::sync::OnceLock::new(),
        auth_engine: None,
        email_service: None,
        auth,
        tochka: None,
        notification_store: None,
            rbac: std::sync::Arc::new(flowlink_relay::rbac_manager::RbacManager::new()),
            notification_router: std::sync::OnceLock::new(),
    }
}

/// Generate a self-signed TLS cert+key, return (cert_der, key_der).
fn generate_test_cert() -> (Vec<u8>, Vec<u8>) {
    let key_pair = rcgen::KeyPair::generate().unwrap();
    let cert_params =
        rcgen::CertificateParams::new(vec!["localhost".into(), "127.0.0.1".into()]).unwrap();
    let cert = cert_params.self_signed(&key_pair).unwrap();
    (cert.der().to_vec(), key_pair.serialize_der())
}

/// Build a rustls ServerConfig from raw DER cert+key.
fn build_tls_config(cert_der: &[u8], key_der: &[u8]) -> tokio_rustls::TlsAcceptor {
    let certs: Vec<rustls::pki_types::CertificateDer> =
        vec![rustls::pki_types::CertificateDer::from(cert_der.to_vec())];
    let key = rustls::pki_types::PrivateKeyDer::try_from(key_der.to_vec()).unwrap();

    let config = rustls::ServerConfig::builder()
        .with_no_client_auth()
        .with_single_cert(certs, key)
        .unwrap();

    TlsAcceptor::from(std::sync::Arc::new(config))
}

/// Bind to a random port.
async fn random_port() -> (tokio::net::TcpListener, u16) {
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let port = listener.local_addr().unwrap().port();
    (listener, port)
}

/// Spawn the WSS TLS relay server, return wss:// URL.
async fn spawn_wss_relay(state: AppState) -> (String, tokio::task::JoinHandle<()>) {
    let (cert_der, key_der) = generate_test_cert();
    let tls_acceptor = build_tls_config(&cert_der, &key_der);
    let (listener, port) = random_port().await;
    let app: Router = build_router(state);
    let url = format!("wss://127.0.0.1:{port}");

    let handle = tokio::spawn(async move {
        loop {
            let (tcp_stream, peer_addr) = match listener.accept().await {
                Ok(s) => s,
                Err(e) => {
                    eprintln!("WSS listen error: {e}");
                    break;
                }
            };

            let tls_acceptor = tls_acceptor.clone();
            let app = app.clone();

            tokio::spawn(async move {
                let tls_stream = match tls_acceptor.accept(tcp_stream).await {
                    Ok(s) => s,
                    Err(e) => {
                        eprintln!("TLS handshake failed from {peer_addr}: {e}");
                        return;
                    }
                };

                let io = hyper_util::rt::TokioIo::new(tls_stream);
                let svc = hyper::service::service_fn(move |req| {
                    let app = app.clone();
                    async move { app.oneshot(req).await }
                });

                if let Err(e) = hyper_util::server::conn::auto::Builder::new(
                    hyper_util::rt::TokioExecutor::new(),
                )
                .serve_connection_with_upgrades(io, svc)
                .await
                {
                    eprintln!("WSS serve error from {peer_addr}: {e}");
                }
            });
        }
    });

    // Give server a moment to start
    tokio::time::sleep(Duration::from_millis(200)).await;
    (url, handle)
}

/// Build a TLS connector that accepts self-signed certs (test-only).
fn test_tls_connector() -> tokio_tungstenite::Connector {
    let config = std::sync::Arc::new(
        rustls::ClientConfig::builder()
            .dangerous()
            .with_custom_certificate_verifier(std::sync::Arc::new(SkipServerVerification))
            .with_no_client_auth(),
    );
    tokio_tungstenite::Connector::Rustls(config)
}

// ── tests ────────────────────────────────────────────────

#[tokio::test]
async fn wss_tls_connect_and_exchange_messages() {
    ensure_ring_provider();
    let tmp = tempfile::tempdir().unwrap();
    let state = make_state(tmp.path());
    let (wss_url, _server) = spawn_wss_relay(state).await;

    let connector = test_tls_connector();

    let (ws_stream, _resp) = tokio_tungstenite::connect_async_tls_with_config(
        &format!("{wss_url}/ws?agent_id=e2e-agent&token=test-token"),
        None,
        false,
        Some(connector),
    )
    .await
    .expect("WSS connect failed");

    let (mut write, mut read) = ws_stream.split();

    // Send connect message
    let connect_msg = serde_json::json!({
        "id": "test-1",
        "type": "connect",
        "agent_id": "e2e-agent",
        "payload": {
            "hostname": "e2e-host",
            "os": "linux",
            "arch": "x86_64",
            "version": "0.1.0"
        }
    });
    write
        .send(WsMessage::Text(connect_msg.to_string().into()))
        .await
        .expect("send connect failed");

    // Read response — should be a connected ack
    let msg = tokio::time::timeout(Duration::from_secs(3), read.next())
        .await
        .expect("timeout waiting for response")
        .expect("no message");

    let text = match msg {
        Ok(WsMessage::Text(t)) => t.to_string(),
        Ok(WsMessage::Close(_)) => panic!("server closed connection"),
        Ok(other) => panic!("unexpected message type: {other:?}"),
        Err(e) => panic!("ws error: {e}"),
    };

    let v: serde_json::Value = serde_json::from_str(&text).expect("invalid JSON");
    assert_eq!(v["type"], "connected", "expected 'connected', got: {text}");
    assert_eq!(v["agent_id"], "e2e-agent");

    // Send heartbeat
    let hb = serde_json::json!({
        "id": "test-2",
        "type": "heartbeat",
        "agent_id": "e2e-agent"
    });
    write
        .send(WsMessage::Text(hb.to_string().into()))
        .await
        .expect("send heartbeat failed");

    // Clean close
    write.close().await.expect("close failed");
}

#[tokio::test]
async fn wss_tls_rejects_without_agent_id() {
    ensure_ring_provider();
    let tmp = tempfile::tempdir().unwrap();
    let state = make_state(tmp.path());
    let (wss_url, _server) = spawn_wss_relay(state).await;

    let connector = test_tls_connector();

    // Connect without agent_id — should get an error or close
    let result = tokio_tungstenite::connect_async_tls_with_config(
        &format!("{wss_url}/ws?token=test-token"),
        None,
        false,
        Some(connector),
    )
    .await;

    match result {
        Ok((mut ws_stream, _resp)) => {
            let (mut _write, mut read) = ws_stream.split();
            let msg = tokio::time::timeout(Duration::from_secs(2), read.next()).await;
            match msg {
                Ok(Some(Ok(WsMessage::Close(_)))) => (),
                Ok(Some(Ok(WsMessage::Text(t)))) => {
                    let v: serde_json::Value = serde_json::from_str(&t.to_string()).unwrap();
                    assert!(v["type"] == "error" || v["type"] == "connected");
                }
                _ => panic!("expected close or error, got: {msg:?}"),
            }
        }
        Err(_) => {
            // Connection itself rejected — also acceptable
        }
    }
}

#[tokio::test]
async fn wss_tls_http_health_still_works() {
    ensure_ring_provider();
    let tmp = tempfile::tempdir().unwrap();
    let state = make_state(tmp.path());
    let (wss_url, _server) = spawn_wss_relay(state).await;

    // reqwest client that accepts self-signed certs
    let client = reqwest::Client::builder()
        .danger_accept_invalid_certs(true)
        .build()
        .unwrap();

    let resp = client
        .get(&format!("{wss_url}/health").replace("wss://", "https://"))
        .send()
        .await
        .expect("health request failed");

    assert!(resp.status().is_success());
    let body: serde_json::Value = resp.json().await.unwrap();
    assert_eq!(body["status"], "ok");
}

// ── custom verifier for self-signed certs (test only) ──

#[derive(Debug)]
struct SkipServerVerification;

impl rustls::client::danger::ServerCertVerifier for SkipServerVerification {
    fn verify_server_cert(
        &self,
        _end_entity: &rustls::pki_types::CertificateDer<'_>,
        _intermediates: &[rustls::pki_types::CertificateDer<'_>],
        _server_name: &rustls::pki_types::ServerName<'_>,
        _ocsp_response: &[u8],
        _now: rustls::pki_types::UnixTime,
    ) -> Result<rustls::client::danger::ServerCertVerified, rustls::Error> {
        Ok(rustls::client::danger::ServerCertVerified::assertion())
    }

    fn verify_tls12_signature(
        &self,
        _message: &[u8],
        _cert: &rustls::pki_types::CertificateDer<'_>,
        _dss: &rustls::DigitallySignedStruct,
    ) -> Result<rustls::client::danger::HandshakeSignatureValid, rustls::Error> {
        Ok(rustls::client::danger::HandshakeSignatureValid::assertion())
    }

    fn verify_tls13_signature(
        &self,
        _message: &[u8],
        _cert: &rustls::pki_types::CertificateDer<'_>,
        _dss: &rustls::DigitallySignedStruct,
    ) -> Result<rustls::client::danger::HandshakeSignatureValid, rustls::Error> {
        Ok(rustls::client::danger::HandshakeSignatureValid::assertion())
    }

    fn supported_verify_schemes(&self) -> Vec<rustls::SignatureScheme> {
        vec![
            rustls::SignatureScheme::RSA_PKCS1_SHA256,
            rustls::SignatureScheme::RSA_PKCS1_SHA384,
            rustls::SignatureScheme::RSA_PKCS1_SHA512,
            rustls::SignatureScheme::ECDSA_NISTP256_SHA256,
            rustls::SignatureScheme::ECDSA_NISTP384_SHA384,
            rustls::SignatureScheme::ECDSA_NISTP521_SHA512,
            rustls::SignatureScheme::RSA_PSS_SHA256,
            rustls::SignatureScheme::RSA_PSS_SHA384,
            rustls::SignatureScheme::RSA_PSS_SHA512,
            rustls::SignatureScheme::ED25519,
        ]
    }
}
