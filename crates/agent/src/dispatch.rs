// Message dispatcher — routes incoming messages to handlers
use flowlink_core::*;
use log::{info, warn};

use crate::approval::ApprovalManager;
use crate::executor::{Executor, ExecResult};
use crate::policy::PolicyEngine;

/// Dispatch an incoming message and return an optional response to send back.
pub async fn dispatch(
    msg: &Message,
    policy: &PolicyEngine,
    approval: &ApprovalManager,
) -> Option<Message> {
    match &msg.msg_type {
        MessageType::ExecRequest => handle_exec(msg, policy, approval).await,

        MessageType::Heartbeat => {
            Some(
                Message::new(MessageType::HeartbeatAck)
                    .with_agent_id(msg.agent_id.as_deref().unwrap_or("")),
            )
        }

        MessageType::FileRead | MessageType::FileWrite | MessageType::FileList => {
            Some(error_response(msg, "FILE_NOT_IMPLEMENTED", "File operations not yet implemented"))
        }

        MessageType::BackupRequest
        | MessageType::BackupRestore
        | MessageType::BackupDelete
        | MessageType::BackupList => {
            Some(error_response(msg, "BACKUP_NOT_IMPLEMENTED", "Backup operations not yet implemented"))
        }

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

fn error_response(msg: &Message, code: &str, message: &str) -> Message {
    Message::new(MessageType::Error)
        .with_agent_id(msg.agent_id.as_deref().unwrap_or(""))
        .with_payload(ErrorPayload {
            code: code.into(),
            message: message.into(),
        })
}
