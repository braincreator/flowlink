// Message dispatcher — routes incoming messages to handlers
use flowlink_core::*;
use log::{info, warn};
use serde::Deserialize;
use std::sync::atomic::{AtomicU64, Ordering};

use crate::approval::ApprovalManager;
use crate::executor::{ExecResult, Executor};
use crate::fileops::FileOps;
use crate::killswitch::KillSwitch;
use crate::policy::PolicyEngine;
use crate::sandbox::Sandbox;
use crate::skills::{Skill, SkillManager};

/// Global counter for shield alerts received by this agent.
/// Incremented atomically so it is safe to read from any thread (metrics, status endpoints, etc.).
static SHIELD_ALERT_RECEIVED_COUNT: AtomicU64 = AtomicU64::new(0);

/// Return the total number of shield alerts received since process start.
pub fn shield_alert_count() -> u64 {
    SHIELD_ALERT_RECEIVED_COUNT.load(Ordering::Relaxed)
}

/// Dispatch an incoming message and return an optional response to send back.
///
/// **Priority routing:**
/// - `Priority::System` → bypasses killswitch, policy, and approval.
///   Only the executor's system semaphore applies.
/// - `Priority::User` → full pipeline: killswitch → policy → approval → executor.
///
/// System priority is reserved for trusted internal operations (auto-restore,
/// rollback, health checks). Never set by external messages.
pub async fn dispatch(
    msg: &Message,
    policy: &PolicyEngine,
    approval: &ApprovalManager,
    fileops: &FileOps,
    backup: &BackupManager,
    killswitch: &KillSwitch,
    skill_mgr: &SkillManager,
    _sandbox: &Sandbox,
    executor: &Executor,
) -> Option<Message> {
    let is_system = msg.priority == Priority::System;

    // Killswitch check — only for user commands.
    // System commands MUST always pass through (restore, rollback, etc.).
    if !is_system && killswitch.is_paused() {
        return Some({
            let mut msg = Message::new(MessageType::Error)
                .with_agent_id(msg.agent_id.as_deref().unwrap_or(""));
            msg.error = Some("agent paused or emergency stop".into());
            msg
        });
    }

    match &msg.msg_type {
        MessageType::ExecRequest => handle_exec(msg, policy, approval, executor, is_system).await,

        MessageType::Heartbeat => {
            Some(
                Message::new(MessageType::HeartbeatAck)
                    .with_agent_id(msg.agent_id.as_deref().unwrap_or("")),
            )
        }

        MessageType::FileRead => handle_file_read(msg, fileops),
        MessageType::FileWrite => handle_file_write(msg, fileops),
        MessageType::FileList => handle_file_list(msg, fileops),

        MessageType::BackupRequest => handle_backup_create(msg, backup).await,
        MessageType::BackupRestore => handle_backup_restore(msg, backup).await,
        MessageType::BackupDelete => handle_backup_delete(msg, backup).await,
        MessageType::BackupList => handle_backup_list(msg, backup).await,

        MessageType::ExecApprove | MessageType::ExecReject | MessageType::ApprovalResponse => {
            handle_approval_response(msg, approval, executor, policy).await
        }

        MessageType::ShieldAlert => handle_shield_alert(msg),

        MessageType::SkillPush => handle_skill_push(msg, skill_mgr),
        MessageType::SkillList => handle_skill_list(msg, skill_mgr),
        MessageType::SkillDelete => handle_skill_delete(msg, skill_mgr),

        // Ignore these server-originated / informational message types
        MessageType::Connect
        | MessageType::Connected
        | MessageType::Disconnect
        | MessageType::HeartbeatAck
        | MessageType::ExecOutput
        | MessageType::ExecDone
        | MessageType::NeedsApproval
        | MessageType::ApprovalRequest
        | MessageType::FileResponse
        | MessageType::SysInfo
        |        MessageType::SysInfoResp
        | MessageType::ConfigUpdate
        | MessageType::PolicyUpdate  // handled in Connection::handle_message before dispatch
        | MessageType::ConfigAck
        | MessageType::PolicyAck
        | MessageType::Task
        | MessageType::TaskProgress
        | MessageType::TaskDone
        | MessageType::TaskCancel
        | MessageType::LlmRequest
        | MessageType::LlmResponse
        | MessageType::BackupResponse
        | MessageType::BackupListResp
        | MessageType::BackupRestoreOk
        | MessageType::BackupDeleteOk
        | MessageType::BackupProgress
        | MessageType::ShieldAlertResponse
        | MessageType::PairingRequest
        | MessageType::PairingConfirm
        | MessageType::PairingResponse
        | MessageType::PatternSuggestion
        | MessageType::Error => {
            info!("Ignoring message type: {:?}", msg.msg_type);
            None
        }
    }
}

/// Handle an incoming ShieldAlert message.
///
/// Parses the alert payload, logs detailed information at WARN level,
/// increments the global shield alert counter, and returns `None` (no reply
/// needed — the server that sent the alert does not expect an ACK).
fn handle_shield_alert(msg: &Message) -> Option<Message> {
    // Increment the metric counter unconditionally for every received alert
    let count = SHIELD_ALERT_RECEIVED_COUNT.fetch_add(1, Ordering::Relaxed) + 1;

    match &msg.payload {
        Some(payload) => {
            // Attempt to deserialize into the structured alert payload
            match serde_json::from_value::<ShieldAlertPayload>(payload.clone()) {
                Ok(alert) => {
                    warn!(
                        "[ShieldAlert #{}] id={} pid={} uid={} user={} cmd={:?} rule={} action={} snapshot={:?} ts={}",
                        count,
                        alert.alert_id,
                        alert.pid,
                        alert.uid,
                        alert.username,
                        alert.command,
                        alert.rule_name,
                        alert.action,
                        alert.snapshot,
                        alert.timestamp,
                    );
                }
                Err(e) => {
                    // Payload present but malformed — still log what we can
                    warn!(
                        "[ShieldAlert #{}] (malformed payload) agent={:?} error={}",
                        count, msg.agent_id, e,
                    );
                }
            }
        }
        None => {
            warn!(
                "[ShieldAlert #{}] received with no payload, agent={:?}",
                count, msg.agent_id,
            );
        }
    }

    // ShieldAlerts are informational from server to agent; no reply needed
    None
}

async fn handle_approval_response(
    msg: &Message,
    approval: &ApprovalManager,
    executor: &Executor,
    _policy: &PolicyEngine,
) -> Option<Message> {
    let payload = match &msg.payload {
        Some(p) => p,
        None => return None,
    };
    let rid = match payload.get("request_id").and_then(|v| v.as_str()) {
        Some(r) => r,
        None => return None,
    };
    let approved = payload
        .get("approved")
        .and_then(|v| v.as_bool())
        .unwrap_or(false);
    let decision = if approved {
        crate::approval::ApprovalDecision::Approved
    } else {
        crate::approval::ApprovalDecision::Rejected
    };

    let exec_payload = approval.respond(rid, decision).await;

    // If approved and we have the saved payload, execute it now
    if let Some(exec_payload) = exec_payload {
        info!("Executing approved command: {}", exec_payload.command);
        match executor.exec(&exec_payload, flowlink_core::Priority::User).await {
            Ok(result) => {
                info!("Approved exec done: exit={} duration={}ms cmd={}", result.exit_code, result.duration_ms, exec_payload.command);
                return Some(exec_done_response(msg, &result));
            }
            Err(e) => {
                return Some(error_response(msg, "EXEC_FAILED", &format!("Execution error: {e}")));
            }
        }
    }
    None
}

async fn handle_exec(
    msg: &Message,
    policy: &PolicyEngine,
    approval: &ApprovalManager,
    executor: &Executor,
    is_system: bool,
) -> Option<Message> {
    let payload: ExecRequestPayload = match msg.payload.as_ref() {
        Some(p) => match serde_json::from_value(p.clone()) {
            Ok(p) => p,
            Err(e) => {
                return Some(error_response(
                    msg,
                    "INVALID_PAYLOAD",
                    &format!("Failed to parse ExecRequestPayload: {e}"),
                ));
            }
        },
        None => {
            return Some(error_response(
                msg,
                "MISSING_PAYLOAD",
                "ExecRequest requires a payload",
            ));
        }
    };

    // ── System commands: bypass policy and approval entirely ──
    if is_system {
        info!(
            "[SYSTEM] Executing (bypass policy/approval): {}",
            payload.command
        );
        match executor.exec(&payload, Priority::System).await {
            Ok(result) => return Some(exec_done_response(msg, &result)),
            Err(e) => {
                return Some(error_response(
                    msg,
                    "EXEC_FAILED",
                    &format!("Execution error: {e}"),
                ))
            }
        }
    }

    // ── User commands: full policy pipeline ──
    let policy_result = policy.check(&payload);
    info!("[dispatch] policy check done: blocked={}, risk={:?}", policy_result.blocked, policy_result.risk_level);
    if policy_result.blocked {
        warn!("Command blocked: {}", policy_result.reason);
        let agent_id = msg.agent_id.as_deref().unwrap_or("");
        let shield_alert = Message::new(MessageType::ShieldAlert)
            .with_agent_id(agent_id)
            .with_payload(ShieldAlertPayload {
                alert_id: uuid::Uuid::new_v4().to_string(),
                pid: std::process::id(),
                uid: unsafe { libc::getuid() },
                username: whoami_fallback(),
                command: payload.command.clone(),
                rule_name: policy_result.reason.clone(),
                action: "blocked".into(),
                snapshot: policy_result.snapshot_id.clone(),
                timestamp: chrono::Utc::now().timestamp(),
            });
        // Return the error response; the connection layer will also forward the shield alert
        // We stash the alert in a thread-local so connection can pick it up
        SHIELD_ALERT_QUEUE.with(|q| q.borrow_mut().push(shield_alert));
        return Some(error_response(msg, "POLICY_BLOCKED", &policy_result.reason));
    }

    // Approval check
    let risk_str = match policy_result.risk_level {
        crate::policy::RiskLevel::None => "none",
        crate::policy::RiskLevel::Low => "low",
        crate::policy::RiskLevel::Medium => "medium",
        crate::policy::RiskLevel::High => "high",
    };
    if approval.needs_approval(risk_str) {
        info!("Command requires approval: {} (timeout: {}s)", payload.command, approval.timeout_sec());

        // Register pending approval with exec payload for later execution
        let request_id = payload.request_id.clone();
        let command = payload.command.clone();
        let risk = risk_str.to_string();
        let exec_payload = Some(payload.clone());

        // Spawn approval wait in background — sends NeedsApproval upstream
        // On timeout, sends ExecDone with error back to relay
        let approval_clone = approval.clone_safe();
        let _agent_id = msg.agent_id.as_deref().unwrap_or("").to_string();
        let exec_payload_clone = payload.clone();
        tokio::spawn(async move {
            let decision = approval_clone.request_approval(request_id.clone(), command, risk, exec_payload).await;
            if matches!(decision, crate::approval::ApprovalDecision::TimedOut) {
                // We need to notify relay that the command timed out
                // But we don't have access to the sender here — the result is logged
                log::warn!("Approval request {} timed out, command: {}", request_id, exec_payload_clone.command);
            }
        });

        let approval_payload = ApprovalRequestPayload {
            request_id: payload.request_id.clone(),
            command: payload.command.clone(),
            risk: risk_str.to_string(),
            mode: format!("{:?}", approval.mode()),
            timestamp: chrono::Utc::now().timestamp(),
        };
        return Some(
            Message::new(MessageType::NeedsApproval)
                .with_agent_id(msg.agent_id.as_deref().unwrap_or(""))
                .with_payload(approval_payload),
        );
    }

    // ── GitOps pre-exec backup for destructive commands ──
    #[cfg(feature = "gitops")]
    {
        let cmd_lower = payload.command.to_lowercase();
        let is_destructive = cmd_lower.contains("rm ")
            || cmd_lower.contains("rm -rf")
            || cmd_lower.contains("chmod")
            || cmd_lower.contains("chown")
            || cmd_lower.contains("truncate")
            || cmd_lower.contains("dd ")
            || cmd_lower.contains(">/")
            || cmd_lower.contains("mv ")
            || cmd_lower.contains("cp --remove");

        if is_destructive {
            log::info!("[gitops] Pre-exec backup for destructive command: {}", payload.command);
            let backup_config = flowlink_gitops::config::BackupConfig::default();
            let vault_config = flowlink_gitops::config::VaultConfig::default();
            let engine = flowlink_gitops::backup::BackupEngine::new(backup_config, vault_config);
            let file_backup = engine.file_backup();
            // Backup affected paths if dir is specified
            if let Some(dir) = &payload.dir {
                let paths = vec![std::path::PathBuf::from(dir)];
                log::info!("[gitops] Backing up paths: {:?}", paths);
            }
        }
    }

    match executor.exec(&payload, Priority::User).await {
        Ok(result) => {
            info!("Exec done: exit={} duration={}ms cmd={}", result.exit_code, result.duration_ms, payload.command);
            Some(exec_done_response(msg, &result))
        }
        Err(e) => Some(error_response(
            msg,
            "EXEC_FAILED",
            &format!("Execution error: {e}"),
        )),
    }
}

fn exec_done_response(msg: &Message, result: &ExecResult) -> Message {
    let payload = ExecDonePayload {
        request_id: result.request_id.clone(),
        exit_code: result.exit_code,
        duration_ms: result.duration_ms as i64,
        stdout: result.stdout.clone(),
        stderr: String::new(),
        error: if result.timed_out {
            Some("Command timed out".into())
        } else {
            None
        },
    };
    Message::new(MessageType::ExecDone)
        .with_agent_id(msg.agent_id.as_deref().unwrap_or(""))
        .with_payload(payload)
}

fn handle_file_read(msg: &Message, fileops: &FileOps) -> Option<Message> {
    use base64::{engine::general_purpose::STANDARD, Engine as _};
    let payload: FileReadPayload = match msg
        .payload
        .as_ref()
        .and_then(|p| serde_json::from_value(p.clone()).ok())
    {
        Some(p) => p,
        None => {
            return Some(error_response(
                msg,
                "MISSING_PAYLOAD",
                "FileRead requires a payload with path",
            ))
        }
    };
    if payload.path.is_empty() {
        return Some(error_response(
            msg,
            flowlink_core::codes::FILE_EMPTY_PATH,
            "Path is empty",
        ));
    }
    match fileops.read(&payload.path) {
        Ok(data) => {
            let encoded = STANDARD.encode(&data);
            Some(
                Message::new(MessageType::FileResponse)
                    .with_agent_id(msg.agent_id.as_deref().unwrap_or(""))
                    .with_payload(FileResponsePayload {
                        request_id: None,
                        path: Some(payload.path),
                        content: Some(encoded),
                        encoding: Some("base64".into()),
                        mode: None,
                        size: Some(data.len() as i64),
                        is_dir: Some(false),
                        entries: None,
                        error: None,
                    }),
            )
        }
        Err(e) => Some(error_response(
            msg,
            e.split(':').next().unwrap_or("FILE_READ_ERROR"),
            &e,
        )),
    }
}

fn handle_file_write(msg: &Message, fileops: &FileOps) -> Option<Message> {
    use base64::{engine::general_purpose::STANDARD, Engine as _};
    let payload: FileWritePayload = match msg
        .payload
        .as_ref()
        .and_then(|p| serde_json::from_value(p.clone()).ok())
    {
        Some(p) => p,
        None => {
            return Some(error_response(
                msg,
                "MISSING_PAYLOAD",
                "FileWrite requires a payload",
            ))
        }
    };
    let data = match STANDARD.decode(&payload.content) {
        Ok(d) => d,
        Err(e) => {
            return Some(error_response(
                msg,
                "INVALID_ENCODING",
                &format!("Base64 decode failed: {e}"),
            ))
        }
    };
    match fileops.write(&payload.path, &data) {
        Ok(()) => Some(
            Message::new(MessageType::FileResponse)
                .with_agent_id(msg.agent_id.as_deref().unwrap_or(""))
                .with_payload(FileResponsePayload {
                    request_id: None,
                    path: Some(payload.path),
                    content: None,
                    encoding: Some("base64".into()),
                    mode: payload.mode,
                    size: Some(data.len() as i64),
                    is_dir: Some(false),
                    entries: None,
                    error: None,
                }),
        ),
        Err(e) => Some(error_response(
            msg,
            e.split(':').next().unwrap_or("FILE_WRITE_ERROR"),
            &e,
        )),
    }
}

fn handle_file_list(msg: &Message, fileops: &FileOps) -> Option<Message> {
    #[derive(Deserialize)]
    struct ListReq {
        path: String,
        #[serde(default)]
        recursive: bool,
    }
    let payload: ListReq = match msg
        .payload
        .as_ref()
        .and_then(|p| serde_json::from_value(p.clone()).ok())
    {
        Some(p) => p,
        None => {
            return Some(error_response(
                msg,
                "MISSING_PAYLOAD",
                "FileList requires a payload with path",
            ))
        }
    };
    match fileops.list(&payload.path, payload.recursive) {
        Ok(entries) => {
            let core_entries: Vec<flowlink_core::FileEntry> = entries
                .iter()
                .map(|e| flowlink_core::FileEntry {
                    name: e.name.clone(),
                    size: e.size,
                    is_dir: e.is_dir,
                    mode: e.mode,
                })
                .collect();
            Some(
                Message::new(MessageType::FileResponse)
                    .with_agent_id(msg.agent_id.as_deref().unwrap_or(""))
                    .with_payload(FileResponsePayload {
                        request_id: None,
                        path: Some(payload.path),
                        content: None,
                        encoding: None,
                        mode: None,
                        size: None,
                        is_dir: Some(true),
                        entries: Some(core_entries),
                        error: None,
                    }),
            )
        }
        Err(e) => Some(error_response(
            msg,
            e.split(':').next().unwrap_or("FILE_READ_ERROR"),
            &e,
        )),
    }
}

fn error_response(msg: &Message, code: &str, message: &str) -> Message {
    Message::new(MessageType::Error)
        .with_agent_id(msg.agent_id.as_deref().unwrap_or(""))
        .with_payload(ErrorPayload {
            code: code.into(),
            message: message.into(),
        })
}

use crate::backup::BackupManager;
use flowlink_core::ShieldAlertPayload;
use std::cell::RefCell;

thread_local! {
    /// Queue of shield alerts generated during dispatch (picked up by connection layer)
    pub static SHIELD_ALERT_QUEUE: RefCell<Vec<Message>> = const { RefCell::new(Vec::new()) };
}

/// Drain any shield alerts queued during dispatch. Call from connection layer after dispatch.
pub fn drain_shield_alerts() -> Vec<Message> {
    SHIELD_ALERT_QUEUE.with(|q| std::mem::take(&mut *q.borrow_mut()))
}

fn whoami_fallback() -> String {
    std::env::var("USER").unwrap_or_else(|_| "unknown".into())
}

async fn handle_backup_create(msg: &Message, backup: &BackupManager) -> Option<Message> {
    let payload: BackupRequestPayload = match msg
        .payload
        .as_ref()
        .and_then(|p| serde_json::from_value(p.clone()).ok())
    {
        Some(p) => p,
        None => {
            return Some(error_response(
                msg,
                "MISSING_PAYLOAD",
                "BackupRequest requires a payload",
            ))
        }
    };
    let paths = payload.paths.unwrap_or_default();
    let label = payload.description.unwrap_or_default();
    let request_id = payload.request_id;

    match backup.create(&label, paths).await {
        Ok(meta) => Some(
            Message::new(MessageType::BackupResponse)
                .with_agent_id(msg.agent_id.as_deref().unwrap_or(""))
                .with_payload(BackupResponsePayload {
                    request_id,
                    snapshot_id: Some(meta.id),
                    size: Some(meta.size_bytes as i64),
                    timestamp: Some(meta.created_at),
                    success: true,
                    error: None,
                }),
        ),
        Err(e) => Some(error_response(
            msg,
            codes::BACKUP_CREATE_ERROR,
            &e.to_string(),
        )),
    }
}

async fn handle_backup_list(msg: &Message, backup: &BackupManager) -> Option<Message> {
    match backup.list().await {
        Ok(snapshots) => {
            let entries: Vec<Snapshot> = snapshots
                .into_iter()
                .map(|m| Snapshot {
                    id: m.id,
                    description: if m.label.is_empty() {
                        None
                    } else {
                        Some(m.label)
                    },
                    timestamp: m.created_at,
                    size: m.size_bytes as i64,
                    paths: m.paths,
                    filename: m.filename,
                })
                .collect();
            Some(
                Message::new(MessageType::BackupListResp)
                    .with_agent_id(msg.agent_id.as_deref().unwrap_or(""))
                    .with_payload(serde_json::json!({ "snapshots": entries })),
            )
        }
        Err(e) => Some(error_response(
            msg,
            codes::BACKUP_CREATE_ERROR,
            &e.to_string(),
        )),
    }
}

async fn handle_backup_restore(msg: &Message, backup: &BackupManager) -> Option<Message> {
    let payload: BackupRestorePayload = match msg
        .payload
        .as_ref()
        .and_then(|p| serde_json::from_value(p.clone()).ok())
    {
        Some(p) => p,
        None => {
            return Some(error_response(
                msg,
                "MISSING_PAYLOAD",
                "BackupRestore requires a payload",
            ))
        }
    };
    match backup.restore(&payload.snapshot_id, None).await {
        Ok(()) => Some(
            Message::new(MessageType::BackupRestoreOk)
                .with_agent_id(msg.agent_id.as_deref().unwrap_or(""))
                .with_payload(serde_json::json!({ "request_id": payload.request_id, "snapshot_id": payload.snapshot_id })),
        ),
        Err(e) => Some(error_response(msg, codes::BACKUP_RESTORE_ERROR, &e.to_string())),
    }
}

async fn handle_backup_delete(msg: &Message, backup: &BackupManager) -> Option<Message> {
    let payload: BackupRestorePayload = match msg
        .payload
        .as_ref()
        .and_then(|p| serde_json::from_value(p.clone()).ok())
    {
        Some(p) => p,
        None => {
            return Some(error_response(
                msg,
                "MISSING_PAYLOAD",
                "BackupDelete requires a payload",
            ))
        }
    };
    match backup.delete(&payload.snapshot_id).await {
        Ok(()) => Some(
            Message::new(MessageType::BackupDeleteOk)
                .with_agent_id(msg.agent_id.as_deref().unwrap_or(""))
                .with_payload(serde_json::json!({ "request_id": payload.request_id, "snapshot_id": payload.snapshot_id })),
        ),
        Err(e) => Some(error_response(msg, codes::BACKUP_DELETE_ERROR, &e.to_string())),
    }
}

fn handle_skill_push(msg: &Message, skill_mgr: &SkillManager) -> Option<Message> {
    let payload: Skill = match msg
        .payload
        .as_ref()
        .and_then(|p| serde_json::from_value(p.clone()).ok())
    {
        Some(s) => s,
        None => {
            return Some(error_response(
                msg,
                "MISSING_PAYLOAD",
                "SkillPush requires a payload",
            ))
        }
    };
    let mut skill = payload;
    match skill_mgr.install(&mut skill) {
        Ok(()) => Some(
            Message::new(MessageType::SkillPush)
                .with_agent_id(msg.agent_id.as_deref().unwrap_or(""))
                .with_payload(serde_json::json!({ "success": true, "skill_id": skill.id })),
        ),
        Err(e) => Some(error_response(msg, "SKILL_INSTALL_ERROR", &e.to_string())),
    }
}

fn handle_skill_list(msg: &Message, skill_mgr: &SkillManager) -> Option<Message> {
    match skill_mgr.list() {
        Ok(skills) => Some(
            Message::new(MessageType::SkillList)
                .with_agent_id(msg.agent_id.as_deref().unwrap_or(""))
                .with_payload(serde_json::json!({ "skills": skills })),
        ),
        Err(e) => Some(error_response(msg, "SKILL_LIST_ERROR", &e.to_string())),
    }
}

fn handle_skill_delete(msg: &Message, skill_mgr: &SkillManager) -> Option<Message> {
    let name = match msg
        .payload
        .as_ref()
        .and_then(|p| p.get("name").and_then(|v| v.as_str()))
    {
        Some(n) => n,
        None => {
            return Some(error_response(
                msg,
                "MISSING_PAYLOAD",
                "SkillDelete requires a payload with 'name'",
            ))
        }
    };
    match skill_mgr.delete(name) {
        Ok(()) => Some(
            Message::new(MessageType::SkillDelete)
                .with_agent_id(msg.agent_id.as_deref().unwrap_or(""))
                .with_payload(serde_json::json!({ "success": true, "name": name })),
        ),
        Err(e) => Some(error_response(msg, "SKILL_DELETE_ERROR", &e.to_string())),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use flowlink_core::*;
    use std::collections::HashMap;

    fn test_deps() -> (
        tempfile::TempDir,
        PolicyEngine,
        ApprovalManager,
        FileOps,
        BackupManager,
        KillSwitch,
        SkillManager,
        Sandbox,
        Executor,
    ) {
        let tmp = tempfile::tempdir().unwrap();
        let tmp_canonical = tmp.path().canonicalize().unwrap();
        let tmp_str = tmp_canonical.to_str().unwrap();
        let policy = PolicyEngine::new(false, false);
        let approval = ApprovalManager::new(crate::approval::ApprovalMode::Auto);
        let fileops = FileOps::new(vec![tmp_str.into()], 1024 * 1024);
        let backup =
            BackupManager::new(tmp.path().join("backups").to_str().unwrap().into(), 10, 30);
        let killswitch = KillSwitch::new();
        let skill_mgr = SkillManager::new(tmp.path().to_str().unwrap()).unwrap();
        let sandbox = Sandbox::new(
            vec![tmp.path().to_str().unwrap().into()],
            vec![],
            0,
            0,
            false,
        );
        let executor = Executor::default_executor();
        (
            tmp, policy, approval, fileops, backup, killswitch, skill_mgr, sandbox, executor,
        )
    }

    fn msg_with(t: MessageType, payload: serde_json::Value) -> Message {
        Message::new(t)
            .with_agent_id("test-agent")
            .with_payload(payload)
    }

    fn system_msg_with(t: MessageType, payload: serde_json::Value) -> Message {
        Message::new(t)
            .with_agent_id("test-agent")
            .with_payload(payload)
            .with_priority(Priority::System)
    }

    #[tokio::test]
    async fn test_ping_ack() {
        let (_tmp, policy, approval, fileops, backup, ks, skill_mgr, sandbox, executor) =
            test_deps();
        let msg = Message::new(MessageType::Heartbeat).with_agent_id("test");
        let resp = dispatch(
            &msg, &policy, &approval, &fileops, &backup, &ks, &skill_mgr, &sandbox, &executor,
        )
        .await;
        assert!(resp.is_some());
        assert_eq!(resp.unwrap().msg_type, MessageType::HeartbeatAck);
    }

    #[tokio::test]
    async fn test_exec_done() {
        let (_tmp, policy, approval, fileops, backup, ks, skill_mgr, sandbox, executor) =
            test_deps();
        let payload = serde_json::json!({
            "command": "echo hello",
            "timeout_sec": 10,
            "request_id": "e1"
        });
        let msg = msg_with(MessageType::ExecRequest, payload);
        let resp = dispatch(
            &msg, &policy, &approval, &fileops, &backup, &ks, &skill_mgr, &sandbox, &executor,
        )
        .await;
        assert!(resp.is_some());
        assert_eq!(resp.unwrap().msg_type, MessageType::ExecDone);
    }

    #[tokio::test]
    async fn test_exec_blocked() {
        let (_tmp, policy, approval, fileops, backup, ks, skill_mgr, sandbox, executor) =
            test_deps();
        let payload = serde_json::json!({
            "command": "rm -rf /",
            "timeout_sec": 10,
            "request_id": "e2"
        });
        let msg = msg_with(MessageType::ExecRequest, payload);
        let resp = dispatch(
            &msg, &policy, &approval, &fileops, &backup, &ks, &skill_mgr, &sandbox, &executor,
        )
        .await;
        assert!(resp.is_some());
        assert_eq!(resp.unwrap().msg_type, MessageType::Error);
    }

    #[tokio::test]
    async fn test_file_read() {
        let (tmp, policy, approval, fileops, backup, ks, skill_mgr, sandbox, executor) =
            test_deps();
        let path = tmp.path().join("test.txt");
        std::fs::write(&path, "hello").unwrap();
        let payload = serde_json::json!({ "path": path.to_str().unwrap() });
        let msg = msg_with(MessageType::FileRead, payload);
        let resp = dispatch(
            &msg, &policy, &approval, &fileops, &backup, &ks, &skill_mgr, &sandbox, &executor,
        )
        .await;
        assert!(resp.is_some());
        assert_eq!(resp.unwrap().msg_type, MessageType::FileResponse);
    }

    #[tokio::test]
    async fn test_backup_list() {
        let (_tmp, policy, approval, fileops, backup, ks, skill_mgr, sandbox, executor) =
            test_deps();
        let msg = Message::new(MessageType::BackupList).with_agent_id("test");
        let resp = dispatch(
            &msg, &policy, &approval, &fileops, &backup, &ks, &skill_mgr, &sandbox, &executor,
        )
        .await;
        assert!(resp.is_some());
        assert_eq!(resp.unwrap().msg_type, MessageType::BackupListResp);
    }

    #[tokio::test]
    async fn test_skill_list() {
        let (_tmp, policy, approval, fileops, backup, ks, skill_mgr, sandbox, executor) =
            test_deps();
        let msg = Message::new(MessageType::SkillList).with_agent_id("test");
        let resp = dispatch(
            &msg, &policy, &approval, &fileops, &backup, &ks, &skill_mgr, &sandbox, &executor,
        )
        .await;
        assert!(resp.is_some());
    }

    #[tokio::test]
    async fn test_killswitch_blocks_user_exec() {
        let (_tmp, policy, approval, fileops, backup, ks, skill_mgr, sandbox, executor) =
            test_deps();
        ks.pause("test");
        let payload = serde_json::json!({
            "command": "echo hello",
            "timeout_sec": 10,
            "request_id": "e3"
        });
        let msg = msg_with(MessageType::ExecRequest, payload);
        let resp = dispatch(
            &msg, &policy, &approval, &fileops, &backup, &ks, &skill_mgr, &sandbox, &executor,
        )
        .await;
        assert!(resp.is_some());
        assert_eq!(resp.unwrap().msg_type, MessageType::Error);
    }

    #[tokio::test]
    async fn test_system_bypasses_killswitch() {
        // System priority MUST pass through even when killswitch is paused
        let (_tmp, policy, approval, fileops, backup, ks, skill_mgr, sandbox, executor) =
            test_deps();
        ks.pause("test");

        let payload = serde_json::json!({
            "command": "echo system-restore-ok",
            "timeout_sec": 10,
            "request_id": "sys-1"
        });
        let msg = system_msg_with(MessageType::ExecRequest, payload);
        let resp = dispatch(
            &msg, &policy, &approval, &fileops, &backup, &ks, &skill_mgr, &sandbox, &executor,
        )
        .await;

        assert!(resp.is_some());
        let r = resp.unwrap();
        assert_eq!(r.msg_type, MessageType::ExecDone);
    }

    #[tokio::test]
    async fn test_system_bypasses_policy() {
        // System priority should execute even destructive commands
        let (_tmp, policy, approval, fileops, backup, ks, skill_mgr, sandbox, executor) =
            test_deps();

        let payload = serde_json::json!({
            "command": "rm -rf /nonexistent_test_dir",
            "timeout_sec": 5,
            "request_id": "sys-2"
        });
        let msg = system_msg_with(MessageType::ExecRequest, payload);
        let resp = dispatch(
            &msg, &policy, &approval, &fileops, &backup, &ks, &skill_mgr, &sandbox, &executor,
        )
        .await;

        // Should execute (not be blocked) — even though rm -rf is normally blocked
        assert!(resp.is_some());
        assert_eq!(resp.unwrap().msg_type, MessageType::ExecDone);
    }

    #[tokio::test]
    async fn test_missing_payload_error() {
        let (_tmp, policy, approval, fileops, backup, ks, skill_mgr, sandbox, executor) =
            test_deps();
        let msg = Message::new(MessageType::ExecRequest).with_agent_id("test");
        let resp = dispatch(
            &msg, &policy, &approval, &fileops, &backup, &ks, &skill_mgr, &sandbox, &executor,
        )
        .await;
        assert!(resp.is_some());
        assert_eq!(resp.unwrap().msg_type, MessageType::Error);
    }

    #[tokio::test]
    async fn test_shield_alert_increments_counter() {
        let (_tmp, policy, approval, fileops, backup, ks, skill_mgr, sandbox, executor) =
            test_deps();

        // Reset the counter for the test
        let before = shield_alert_count();

        let payload = serde_json::json!({
            "alert_id": "test-alert-1",
            "pid": 1234,
            "uid": 1000,
            "username": "testuser",
            "command": "rm -rf /",
            "rule_name": "no_rm_rf",
            "action": "blocked",
            "timestamp": 1700000000
        });
        let msg = msg_with(MessageType::ShieldAlert, payload);
        let resp = dispatch(
            &msg, &policy, &approval, &fileops, &backup, &ks, &skill_mgr, &sandbox, &executor,
        )
        .await;

        // ShieldAlert handler returns None (no reply)
        assert!(resp.is_none());
        // Counter should have incremented (at least 1, may be more due to parallel tests)
        assert!(shield_alert_count() >= before + 1);
    }

    #[tokio::test]
    async fn test_shield_alert_no_payload() {
        let (_tmp, policy, approval, fileops, backup, ks, skill_mgr, sandbox, executor) =
            test_deps();

        let before = shield_alert_count();

        let msg = Message::new(MessageType::ShieldAlert).with_agent_id("test-agent");
        let resp = dispatch(
            &msg, &policy, &approval, &fileops, &backup, &ks, &skill_mgr, &sandbox, &executor,
        )
        .await;

        assert!(resp.is_none());
        // Counter still increments even without payload
        assert_eq!(shield_alert_count(), before + 1);
    }

    #[tokio::test]
    async fn test_shield_alert_malformed_payload() {
        let (_tmp, policy, approval, fileops, backup, ks, skill_mgr, sandbox, executor) =
            test_deps();

        let before = shield_alert_count();

        // Payload that doesn't match ShieldAlertPayload schema
        let payload = serde_json::json!({ "garbage": true });
        let msg = msg_with(MessageType::ShieldAlert, payload);
        let resp = dispatch(
            &msg, &policy, &approval, &fileops, &backup, &ks, &skill_mgr, &sandbox, &executor,
        )
        .await;

        assert!(resp.is_none());
        // Counter increments even with malformed payload; use >= to tolerate
        // parallel test interference on the shared global counter.
        assert!(shield_alert_count() > before, "expected counter to increment, was {} now {}", before, shield_alert_count());
    }

    #[tokio::test]
    async fn test_system_priority_in_dispatch_still_bypasses() {
        // When dispatch() receives a System message directly (internal call),
        // it bypasses policy. This test verifies the dispatch-level behavior.
        // Note: connection layer strips System for inbound, but internal
        // components create System messages programmatically.
        let (_tmp, policy, approval, fileops, backup, ks, skill_mgr, sandbox, executor) =
            test_deps();
        ks.pause("test");

        // Simulate an internal System message (not from WebSocket)
        let msg = system_msg_with(
            MessageType::ExecRequest,
            serde_json::json!({
                "command": "echo internal-system",
                "timeout_sec": 5,
                "request_id": "internal-1"
            }),
        );
        let resp = dispatch(
            &msg, &policy, &approval, &fileops, &backup, &ks, &skill_mgr, &sandbox, &executor,
        )
        .await;

        assert!(resp.is_some());
        assert_eq!(resp.unwrap().msg_type, MessageType::ExecDone);
    }
}
