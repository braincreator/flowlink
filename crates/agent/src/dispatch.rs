// Message dispatcher — routes incoming messages to handlers
use flowlink_core::*;
use log::{info, warn};
use serde::Deserialize;

use crate::approval::ApprovalManager;
use crate::executor::{Executor, ExecResult};
use crate::fileops::FileOps;
use crate::policy::PolicyEngine;

/// Dispatch an incoming message and return an optional response to send back.
pub async fn dispatch(
    msg: &Message,
    policy: &PolicyEngine,
    approval: &ApprovalManager,
    fileops: &FileOps,
    backup: &BackupManager,
) -> Option<Message> {
    match &msg.msg_type {
        MessageType::ExecRequest => handle_exec(msg, policy, approval).await,

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
            handle_approval_response(msg, approval).await;
            None
        }

        MessageType::ShieldAlert => {
            info!("Shield alert received (no-op for now)");
            None
        }

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
        | MessageType::SysInfoResp
        | MessageType::ConfigUpdate
        | MessageType::ConfigAck
        | MessageType::Task
        | MessageType::TaskProgress
        | MessageType::TaskDone
        | MessageType::TaskCancel
        | MessageType::SkillPush
        | MessageType::SkillList
        | MessageType::SkillDelete
        | MessageType::LlmRequest
        | MessageType::LlmResponse
        | MessageType::BackupResponse
        | MessageType::BackupListResp
        | MessageType::BackupRestoreOk
        | MessageType::BackupDeleteOk
        | MessageType::BackupProgress
        | MessageType::ShieldAlertResponse
        | MessageType::Error => {
            info!("Ignoring message type: {:?}", msg.msg_type);
            None
        }
    }
}

async fn handle_approval_response(msg: &Message, approval: &ApprovalManager) {
    let payload = match &msg.payload {
        Some(p) => p,
        None => return,
    };
    let rid = match payload.get("request_id").and_then(|v| v.as_str()) {
        Some(r) => r,
        None => return,
    };
    let approved = payload.get("approved").and_then(|v| v.as_bool()).unwrap_or(false);
    let decision = if approved {
        crate::approval::ApprovalDecision::Approved
    } else {
        crate::approval::ApprovalDecision::Rejected
    };
    approval.respond(rid, decision).await;
}

async fn handle_exec(
    msg: &Message,
    policy: &PolicyEngine,
    approval: &ApprovalManager,
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
            return Some(error_response(msg, "MISSING_PAYLOAD", "ExecRequest requires a payload"));
        }
    };

    // Policy check
    let policy_result = policy.check(&payload);
    if policy_result.blocked {
        warn!("Command blocked: {}", policy_result.reason);
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
        info!("Command requires approval: {}", payload.command);
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

    // Execute
    match Executor::exec(&payload).await {
        Ok(result) => Some(exec_done_response(msg, &result)),
        Err(e) => Some(error_response(msg, "EXEC_FAILED", &format!("Execution error: {e}"))),
    }
}

fn exec_done_response(msg: &Message, result: &ExecResult) -> Message {
    let payload = ExecDonePayload {
        request_id: result.request_id.clone(),
        exit_code: result.exit_code,
        duration_ms: result.duration_ms as i64,
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
    use base64::{Engine as _, engine::general_purpose::STANDARD};
    let payload: FileReadPayload = match msg.payload.as_ref().and_then(|p| serde_json::from_value(p.clone()).ok()) {
        Some(p) => p,
        None => return Some(error_response(msg, "MISSING_PAYLOAD", "FileRead requires a payload with path")),
    };
    if payload.path.is_empty() {
        return Some(error_response(msg, flowlink_core::codes::codes::FILE_EMPTY_PATH, "Path is empty"));
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
        Err(e) => Some(error_response(msg, &e.split(':').next().unwrap_or("FILE_READ_ERROR"), &e)),
    }
}

fn handle_file_write(msg: &Message, fileops: &FileOps) -> Option<Message> {
    use base64::{Engine as _, engine::general_purpose::STANDARD};
    let payload: FileWritePayload = match msg.payload.as_ref().and_then(|p| serde_json::from_value(p.clone()).ok()) {
        Some(p) => p,
        None => return Some(error_response(msg, "MISSING_PAYLOAD", "FileWrite requires a payload")),
    };
    let data = match STANDARD.decode(&payload.content) {
        Ok(d) => d,
        Err(e) => return Some(error_response(msg, "INVALID_ENCODING", &format!("Base64 decode failed: {e}"))),
    };
    match fileops.write(&payload.path, &data) {
        Ok(()) => {
            Some(
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
            )
        }
        Err(e) => Some(error_response(msg, &e.split(':').next().unwrap_or("FILE_WRITE_ERROR"), &e)),
    }
}

fn handle_file_list(msg: &Message, fileops: &FileOps) -> Option<Message> {
    #[derive(Deserialize)]
    struct ListReq { path: String, #[serde(default)] recursive: bool }
    let payload: ListReq = match msg.payload.as_ref().and_then(|p| serde_json::from_value(p.clone()).ok()) {
        Some(p) => p,
        None => return Some(error_response(msg, "MISSING_PAYLOAD", "FileList requires a payload with path")),
    };
    match fileops.list(&payload.path, payload.recursive) {
        Ok(entries) => {
            let core_entries: Vec<flowlink_core::FileEntry> = entries.iter().map(|e| flowlink_core::FileEntry {
                name: e.name.clone(),
                size: e.size,
                is_dir: e.is_dir,
                mode: e.mode,
            }).collect();
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
        Err(e) => Some(error_response(msg, &e.split(':').next().unwrap_or("FILE_READ_ERROR"), &e)),
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

async fn handle_backup_create(msg: &Message, backup: &BackupManager) -> Option<Message> {
    let payload: BackupRequestPayload = match msg.payload.as_ref().and_then(|p| serde_json::from_value(p.clone()).ok()) {
        Some(p) => p,
        None => return Some(error_response(msg, "MISSING_PAYLOAD", "BackupRequest requires a payload")),
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
        Err(e) => Some(error_response(msg, codes::codes::BACKUP_CREATE_ERROR, &e.to_string())),
    }
}

async fn handle_backup_list(msg: &Message, backup: &BackupManager) -> Option<Message> {
    match backup.list().await {
        Ok(snapshots) => {
            let entries: Vec<Snapshot> = snapshots.into_iter().map(|m| Snapshot {
                id: m.id,
                description: if m.label.is_empty() { None } else { Some(m.label) },
                timestamp: m.created_at,
                size: m.size_bytes as i64,
                paths: m.paths,
                filename: m.filename,
            }).collect();
            Some(
                Message::new(MessageType::BackupListResp)
                    .with_agent_id(msg.agent_id.as_deref().unwrap_or(""))
                    .with_payload(serde_json::json!({ "snapshots": entries })),
            )
        }
        Err(e) => Some(error_response(msg, codes::codes::BACKUP_CREATE_ERROR, &e.to_string())),
    }
}

async fn handle_backup_restore(msg: &Message, backup: &BackupManager) -> Option<Message> {
    let payload: BackupRestorePayload = match msg.payload.as_ref().and_then(|p| serde_json::from_value(p.clone()).ok()) {
        Some(p) => p,
        None => return Some(error_response(msg, "MISSING_PAYLOAD", "BackupRestore requires a payload")),
    };
    match backup.restore(&payload.snapshot_id, None).await {
        Ok(()) => Some(
            Message::new(MessageType::BackupRestoreOk)
                .with_agent_id(msg.agent_id.as_deref().unwrap_or(""))
                .with_payload(serde_json::json!({ "request_id": payload.request_id, "snapshot_id": payload.snapshot_id })),
        ),
        Err(e) => Some(error_response(msg, codes::codes::BACKUP_RESTORE_ERROR, &e.to_string())),
    }
}

async fn handle_backup_delete(msg: &Message, backup: &BackupManager) -> Option<Message> {
    let payload: BackupRestorePayload = match msg.payload.as_ref().and_then(|p| serde_json::from_value(p.clone()).ok()) {
        Some(p) => p,
        None => return Some(error_response(msg, "MISSING_PAYLOAD", "BackupDelete requires a payload")),
    };
    match backup.delete(&payload.snapshot_id).await {
        Ok(()) => Some(
            Message::new(MessageType::BackupDeleteOk)
                .with_agent_id(msg.agent_id.as_deref().unwrap_or(""))
                .with_payload(serde_json::json!({ "request_id": payload.request_id, "snapshot_id": payload.snapshot_id })),
        ),
        Err(e) => Some(error_response(msg, codes::codes::BACKUP_DELETE_ERROR, &e.to_string())),
    }
}
