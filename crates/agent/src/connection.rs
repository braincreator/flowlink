// WebSocket connection to relay with auto-reconnect + config hot-reload
// Port of internal/agent/connection.go

use flowlink_core::*;
use futures_util::{SinkExt, StreamExt};
use log::{info, warn, error};
use tokio::sync::RwLock;
use tokio_tungstenite::tungstenite::Message as WsMessage;

use crate::approval::ApprovalManager;
use crate::backup::BackupManager;
use crate::executor::Executor;
use crate::fileops::FileOps;
use crate::killswitch::KillSwitch;
use crate::policy::PolicyEngine;
use crate::skills::SkillManager;
use crate::sandbox::Sandbox;
use std::sync::Arc;

pub struct Connection {
    url: String,
    agent_id: String,
    token: String,
    policy: Arc<RwLock<PolicyEngine>>,
    approval: Arc<RwLock<ApprovalManager>>,
    fileops: FileOps,
    backup: BackupManager,
    killswitch: Arc<KillSwitch>,
    skill_mgr: SkillManager,
    sandbox: Arc<RwLock<Sandbox>>,
    executor: Executor,
    shutdown: Arc<tokio::sync::Notify>,
}

impl Connection {
    pub fn new(
        url: String,
        agent_id: String,
        token: String,
        policy: PolicyEngine,
        approval: ApprovalManager,
        fileops: FileOps,
        backup: BackupManager,
        killswitch: Arc<KillSwitch>,
        skill_mgr: SkillManager,
        sandbox: Sandbox,
        executor: Executor,
        shutdown: Arc<tokio::sync::Notify>,
    ) -> Self {
        Self {
            url, agent_id, token,
            policy: Arc::new(RwLock::new(policy)),
            approval: Arc::new(RwLock::new(approval)),
            fileops, backup, killswitch, skill_mgr,
            sandbox: Arc::new(RwLock::new(sandbox)),
            executor,
            shutdown,
        }
    }

    /// Connect, authenticate, run message loop with auto-reconnect + exponential backoff.
    pub async fn run(&self) -> anyhow::Result<()> {
        let mut backoff_secs: u64 = 1;
        const MAX_BACKOFF: u64 = 60;

        loop {
            tokio::select! {
                result = self.connect_and_loop() => {
                    match result {
                        Ok(()) => {
                            info!("Connection closed cleanly, reconnecting...");
                            backoff_secs = 1;
                        }
                        Err(e) => {
                            error!("Connection error: {e}, reconnecting in {backoff_secs}s...");
                        }
                    }

                    tokio::time::sleep(std::time::Duration::from_secs(backoff_secs)).await;
                    backoff_secs = (backoff_secs * 2).min(MAX_BACKOFF);
                }
                _ = self.shutdown.notified() => {
                    info!("Shutdown signal received, stopping agent connection");
                    break;
                }
            }
        }

        Ok(())
    }

    async fn connect_and_loop(&self) -> anyhow::Result<()> {
        let ws_url = format!("{}/ws?agent_id={}&token={}", self.url, self.agent_id, self.token);

        let (mut ws_stream, _resp) = tokio_tungstenite::connect_async(&ws_url).await?;
        info!("Connected to relay {}", self.url);

        // Send connect message
        let connect_payload = ConnectPayload {
            agent_id: self.agent_id.clone(),
            token: self.token.clone(),
            hostname: get_hostname(),
            os: std::env::consts::OS.into(),
            arch: std::env::consts::ARCH.into(),
            client_version: Some(env!("CARGO_PKG_VERSION").into()),
            public_key: None,
            protocol_version: Some(PROTOCOL_VERSION),
        };

        let connect_msg = Message::new(MessageType::Connect)
            .with_agent_id(&self.agent_id)
            .with_payload(connect_payload);

        let json = serde_json::to_string(&connect_msg)?;
        ws_stream.send(WsMessage::Text(json.into())).await?;

        // Message loop
        while let Some(msg) = ws_stream.next().await {
            match msg {
                Ok(WsMessage::Text(text)) => {
                    let response = self.handle_message(&text).await;
                    // Send the primary response
                    if let Some(resp) = response {
                        let json = serde_json::to_string(&resp)?;
                        if let Err(e) = ws_stream.send(WsMessage::Text(json.into())).await {
                            warn!("Failed to send response: {e}");
                        }
                    }
                    // Send any shield alerts queued during dispatch
                    for alert in self.collect_shield_alerts() {
                        let json = serde_json::to_string(&alert)?;
                        if let Err(e) = ws_stream.send(WsMessage::Text(json.into())).await {
                            warn!("Failed to send shield alert: {e}");
                        }
                        info!("Sent ShieldAlert to relay");
                    }
                }
                Ok(WsMessage::Ping(data)) => {
                    ws_stream.send(WsMessage::Pong(data)).await?;
                }
                Ok(WsMessage::Close(_)) => {
                    info!("Relay closed connection");
                    return Ok(());
                }
                Err(e) => return Err(e.into()),
                _ => {}
            }
        }

        Ok(())
    }

    async fn handle_message(&self, text: &str) -> Option<Message> {
        let mut msg: Message = match serde_json::from_str(text) {
            Ok(m) => m,
            Err(e) => {
                warn!("Failed to parse message: {e}");
                return None;
            }
        };

        // SECURITY: Never trust priority from external clients.
        // Priority::System is reserved for internal operations (auto-restore,
        // health engine) that create Messages programmatically — never from
        // the WebSocket. Force all inbound messages to User priority.
        if msg.priority == Priority::System {
            warn!(
                "Inbound message had system priority (id={} type={:?}) — forced to user. \
                 System priority is only for internal operations.",
                msg.id, msg.msg_type
            );
            msg.priority = Priority::User;
        }

        // EXCEPTION: ConfigUpdate from relay is trusted (sent with System priority by reloader).
        // Allow it through to update agent config at runtime.
        if msg.msg_type == MessageType::ConfigUpdate {
            info!("Received ConfigUpdate from relay");
            return self.handle_config_update(&msg).await;
        }

        info!("Received: {:?}", msg.msg_type);
        let response = crate::dispatch::dispatch(
            &msg,
            &*self.policy.read().await,
            &*self.approval.read().await,
            &self.fileops,
            &self.backup,
            &self.killswitch,
            &self.skill_mgr,
            &*self.sandbox.read().await,
            &self.executor,
        ).await;
        response
    }

    /// Handle ConfigUpdate from relay — apply new config and send ConfigAck.
    ///
    /// Updates: read_only mode, sandbox params, approval mode.
    /// Fields that require reconnect (relay_url, agent_id, token) are logged but not applied.
    async fn handle_config_update(&self, msg: &Message) -> Option<Message> {
        let payload = match &msg.payload {
            Some(p) => p,
            None => {
                warn!("ConfigUpdate received with no payload");
                return Some(Message::new(MessageType::ConfigAck)
                    .with_agent_id(&self.agent_id)
                    .with_payload(serde_json::json!({
                        "status": "error",
                        "reason": "no payload"
                    })));
            }
        };

        let mut applied = Vec::new();
        let mut warnings = Vec::new();

        // Update read_only mode
        if let Some(read_only) = payload.get("read_only").and_then(|v| v.as_bool()) {
            {
                let mut policy = self.policy.write().await;
                policy.set_read_only(read_only);
            }
            applied.push(format!("read_only={read_only}"));
        }

        // Update sandbox allowed_dirs
        if let Some(dirs) = payload.get("sandbox_allowed_dirs").and_then(|v| v.as_array()) {
            let dirs: Vec<String> = dirs.iter()
                .filter_map(|v| v.as_str().map(String::from))
                .collect();
            {
                let mut sb = self.sandbox.write().await;
                sb.set_allowed_dirs(dirs.clone());
            }
            applied.push(format!("sandbox_allowed_dirs=[{} items]", dirs.len()));
        }

        // Update sandbox blocked_patterns
        if let Some(patterns) = payload.get("sandbox_blocked_patterns").and_then(|v| v.as_array()) {
            let patterns: Vec<String> = patterns.iter()
                .filter_map(|v| v.as_str().map(String::from))
                .collect();
            {
                let mut sb = self.sandbox.write().await;
                sb.set_blocked_patterns(patterns.clone());
            }
            applied.push(format!("sandbox_blocked_patterns=[{} items]", patterns.len()));
        }

        // Update sandbox allow_sudo
        if let Some(allow) = payload.get("sandbox_allow_sudo").and_then(|v| v.as_bool()) {
            {
                let mut sb = self.sandbox.write().await;
                sb.set_allow_sudo(allow);
            }
            applied.push(format!("sandbox_allow_sudo={allow}"));
        }

        // Update sandbox max_exec_timeout
        if let Some(timeout) = payload.get("sandbox_max_exec_timeout").and_then(|v| v.as_u64()) {
            {
                let mut sb = self.sandbox.write().await;
                sb.set_max_exec_timeout(timeout as u32);
            }
            applied.push(format!("sandbox_max_exec_timeout={timeout}"));
        }

        // Update approval mode
        if let Some(mode) = payload.get("approval_mode").and_then(|v| v.as_str()) {
            use crate::approval::ApprovalMode;
            let new_mode = match mode {
                "soft_ask" => ApprovalMode::SoftAsk,
                "hard_ask" => ApprovalMode::HardAsk,
                "auto" => ApprovalMode::Auto,
                other => {
                    warnings.push(format!("unknown approval_mode '{other}', ignored"));
                    return Some(Message::new(MessageType::ConfigAck)
                        .with_agent_id(&self.agent_id)
                        .with_payload(serde_json::json!({
                            "status": "partial",
                            "applied": applied,
                            "warnings": warnings,
                        })));
                }
            };
            {
                let mut approval = self.approval.write().await;
                approval.set_mode(new_mode);
            }
            applied.push(format!("approval_mode={mode}"));
        }

        // Log fields that require reconnect (cannot be applied at runtime)
        for field in &["relay_url", "agent_id", "token"] {
            if payload.get(*field).is_some() {
                warnings.push(format!("{field} changed — requires agent restart to take effect"));
            }
        }

        let status = if warnings.is_empty() { "applied" } else { "partial" };
        info!(
            "Config update: status={status}, applied=[{}], warnings=[{}]",
            applied.join(", "),
            warnings.join(", "),
        );

        Some(Message::new(MessageType::ConfigAck)
            .with_agent_id(&self.agent_id)
            .with_payload(serde_json::json!({
                "status": status,
                "applied": applied,
                "warnings": warnings,
            })))
    }

    /// After dispatch returns, drain any queued shield alerts. Called from connect_and_loop.
    fn collect_shield_alerts(&self) -> Vec<Message> {
        crate::dispatch::drain_shield_alerts()
    }
}

fn get_hostname() -> String {
    std::process::Command::new("hostname")
        .output()
        .map(|o| String::from_utf8_lossy(&o.stdout).trim().into())
        .unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::approval::{ApprovalManager, ApprovalMode};
    use crate::policy::PolicyEngine;
    use crate::sandbox::Sandbox;
    use flowlink_core::{Message, MessageType, Priority};

    fn test_connection() -> Connection {
        Connection::new(
            "ws://localhost:9090".into(),
            "test-agent".into(),
            "test-token".into(),
            PolicyEngine::new(false, false),
            ApprovalManager::new(ApprovalMode::Auto),
            crate::fileops::FileOps::new(vec![], 1024),
            crate::backup::BackupManager::new("/tmp".into(), 5, 7),
            std::sync::Arc::new(KillSwitch::new()),
            crate::skills::SkillManager::new("/tmp").unwrap(),
            Sandbox::new(vec![], vec![], 1024, 300, false),
            crate::executor::Executor::default_executor(),
            Arc::new(tokio::sync::Notify::new()),
        )
    }

    #[tokio::test]
    async fn test_config_update_read_only() {
        let conn = test_connection();
        let msg = Message::new(MessageType::ConfigUpdate)
            .with_agent_id("test-agent")
            .with_payload(serde_json::json!({"read_only": true}));

        let resp = conn.handle_config_update(&msg).await.unwrap();
        assert_eq!(resp.msg_type, MessageType::ConfigAck);

        // Verify policy was updated
        let policy = conn.policy.read().await;
        assert!(policy.is_read_only());
    }

    #[tokio::test]
    async fn test_config_update_sandbox() {
        let conn = test_connection();
        let msg = Message::new(MessageType::ConfigUpdate)
            .with_agent_id("test-agent")
            .with_payload(serde_json::json!({
                "sandbox_allowed_dirs": ["/home", "/tmp"],
                "sandbox_allow_sudo": true,
            }));

        let resp = conn.handle_config_update(&msg).await.unwrap();
        let payload = resp.payload.unwrap();
        assert_eq!(payload["status"], "applied");
    }

    #[tokio::test]
    async fn test_config_update_approval_mode() {
        let conn = test_connection();
        let msg = Message::new(MessageType::ConfigUpdate)
            .with_agent_id("test-agent")
            .with_payload(serde_json::json!({"approval_mode": "hard_ask"}));

        let resp = conn.handle_config_update(&msg).await.unwrap();
        let payload = resp.payload.unwrap();
        assert_eq!(payload["status"], "applied");
    }

    #[tokio::test]
    async fn test_config_update_no_payload() {
        let conn = test_connection();
        let msg = Message::new(MessageType::ConfigUpdate)
            .with_agent_id("test-agent");

        let resp = conn.handle_config_update(&msg).await.unwrap();
        let payload = resp.payload.unwrap();
        assert_eq!(payload["status"], "error");
    }

    #[tokio::test]
    async fn test_config_update_relay_url_warns() {
        let conn = test_connection();
        let msg = Message::new(MessageType::ConfigUpdate)
            .with_agent_id("test-agent")
            .with_payload(serde_json::json!({"relay_url": "wss://new:9090"}));

        let resp = conn.handle_config_update(&msg).await.unwrap();
        let payload = resp.payload.unwrap();
        assert_eq!(payload["status"], "partial");
        assert!(payload["warnings"].as_array().unwrap().iter().any(|w| w.as_str().unwrap().contains("relay_url")));
    }
}
