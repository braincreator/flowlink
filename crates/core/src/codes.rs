// FlowLink Core — Error codes
// Port of internal/protocol/codes.go
// NEVER change existing codes — only add new ones.

pub mod codes {
    // General
    pub const OK: &str = "OK";
    pub const UNKNOWN_ERROR: &str = "UNKNOWN_ERROR";
    pub const INVALID_JSON: &str = "INVALID_JSON";
    pub const INVALID_PAYLOAD: &str = "INVALID_PAYLOAD";
    pub const AGENT_NOT_FOUND: &str = "AGENT_NOT_FOUND";
    pub const AGENT_NOT_CONNECTED: &str = "AGENT_NOT_CONNECTED";
    pub const UNAUTHORIZED: &str = "UNAUTHORIZED";
    pub const FORBIDDEN: &str = "FORBIDDEN";
    pub const INTERNAL_ERROR: &str = "INTERNAL_ERROR";
    pub const UNKNOWN_MESSAGE_TYPE: &str = "UNKNOWN_MESSAGE_TYPE";

    // Authentication
    pub const TOKEN_MISSING: &str = "TOKEN_MISSING";
    pub const TOKEN_INVALID: &str = "TOKEN_INVALID";
    pub const TOKEN_EXPIRED: &str = "TOKEN_EXPIRED";
    pub const TOKEN_REVOKED: &str = "TOKEN_REVOKED";
    pub const TOKEN_BLACKLISTED: &str = "TOKEN_BLACKLISTED";
    pub const SIGNATURE_INVALID: &str = "SIGNATURE_INVALID";
    pub const SECRET_NOT_FOUND: &str = "SECRET_NOT_FOUND";

    // Agent Lifecycle
    pub const AGENT_CONNECT_FAILED: &str = "AGENT_CONNECT_FAILED";
    pub const AGENT_DISCONNECTED: &str = "AGENT_DISCONNECTED";
    pub const AGENT_NOT_AUTHORIZED: &str = "AGENT_NOT_AUTHORIZED";
    pub const AGENT_LIMIT_EXCEEDED: &str = "AGENT_LIMIT_EXCEEDED";
    pub const AGENT_PAUSED: &str = "AGENT_PAUSED";
    pub const AGENT_EMERGENCY_STOP: &str = "AGENT_EMERGENCY_STOP";
    pub const PROTOCOL_VERSION_MISMATCH: &str = "PROTOCOL_VERSION_MISMATCH";

    // Execution
    pub const EXEC_SUCCESS: &str = "EXEC_SUCCESS";
    pub const EXEC_TIMEOUT: &str = "EXEC_TIMEOUT";
    pub const EXEC_BLOCKED: &str = "EXEC_BLOCKED";
    pub const EXEC_BLOCKED_READONLY: &str = "EXEC_BLOCKED_READONLY";
    pub const EXEC_BLOCKED_SANDBOX: &str = "EXEC_BLOCKED_SANDBOX";
    pub const EXEC_BLOCKED_SUDO: &str = "EXEC_BLOCKED_SUDO";
    pub const EXEC_FAILED: &str = "EXEC_FAILED";
    pub const EXEC_NEEDS_APPROVAL: &str = "EXEC_NEEDS_APPROVAL";
    pub const EXEC_AWAITING_APPROVAL: &str = "EXEC_AWAITING_APPROVAL";
    pub const EXEC_REJECTED: &str = "EXEC_REJECTED";
    pub const EXEC_APPROVED: &str = "EXEC_APPROVED";

    // File Operations
    pub const FILE_EMPTY_PATH: &str = "FILE_EMPTY_PATH";
    pub const FILE_INVALID_PATH: &str = "FILE_INVALID_PATH";
    pub const FILE_NOT_FOUND: &str = "FILE_NOT_FOUND";
    pub const FILE_TOO_LARGE: &str = "FILE_TOO_LARGE";
    pub const FILE_READ_ERROR: &str = "FILE_READ_ERROR";
    pub const FILE_WRITE_ERROR: &str = "FILE_WRITE_ERROR";

    // Config
    pub const CONFIG_APPLIED: &str = "CONFIG_APPLIED";
    pub const CONFIG_FAILED: &str = "CONFIG_UPDATE_FAILED";
    pub const CONFIG_LOAD_ERROR: &str = "CONFIG_LOAD_ERROR";

    // Skills
    pub const SKILL_ALREADY_EXISTS: &str = "SKILL_ALREADY_EXISTS";
    pub const SKILL_NOT_FOUND: &str = "SKILL_NOT_FOUND";

    // Tasks (Autonomous)
    pub const TASK_ACCEPTED: &str = "TASK_ACCEPTED";
    pub const TASK_ERROR: &str = "TASK_ERROR";
    pub const TASK_DONE: &str = "TASK_DONE";

    // Kill Switch
    pub const KILL_SWITCH_DISK_FULL: &str = "KILL_SWITCH_DISK_FULL";
    pub const KILL_SWITCH_CPU_HIGH: &str = "KILL_SWITCH_CPU_HIGH";
    pub const KILL_SWITCH_PAUSED: &str = "KILL_SWITCH_PAUSED";
    pub const KILL_SWITCH_RESUMED: &str = "KILL_SWITCH_RESUMED";
    pub const KILL_SWITCH_EMERGENCY: &str = "KILL_SWITCH_EMERGENCY";

    // Backup
    pub const BACKUP_CREATE_ERROR: &str = "BACKUP_CREATE_ERROR";
    pub const BACKUP_SNAPSHOT_NOT_FOUND: &str = "BACKUP_SNAPSHOT_NOT_FOUND";
    pub const BACKUP_RESTORE_ERROR: &str = "BACKUP_RESTORE_ERROR";
    pub const BACKUP_DELETE_ERROR: &str = "BACKUP_DELETE_ERROR";
    pub const BACKUP_CHECKSUM_MISMATCH: &str = "BACKUP_CHECKSUM_MISMATCH";

    // MCP
    pub const MCP_AGENT_NOT_FOUND: &str = "MCP_AGENT_NOT_FOUND";
    pub const MCP_TIMEOUT: &str = "MCP_TIMEOUT";

    // LLM Proxy
    pub const LLM_ALL_BACKENDS_DOWN: &str = "LLM_ALL_BACKENDS_DOWN";
    pub const LLM_REQUEST_ERROR: &str = "LLM_REQUEST_ERROR";

    // Shield (NEW v2)
    pub const SHIELD_BLOCKED: &str = "SHIELD_BLOCKED";
    pub const SHIELD_SNAPSHOT_CREATED: &str = "SHIELD_SNAPSHOT_CREATED";
    pub const SHIELD_PROCESS_STOPPED: &str = "SHIELD_PROCESS_STOPPED";
    pub const SHIELD_PROCESS_KILLED: &str = "SHIELD_PROCESS_KILLED";
    pub const SHIELD_TIMEOUT: &str = "SHIELD_TIMEOUT";
    pub const SHIELD_ALERT_SENT: &str = "SHIELD_ALERT_SENT";
}

#[cfg(test)]
mod tests {
    use super::*;

    fn all_codes() -> Vec<&'static str> {
        vec![
            OK, UNKNOWN_ERROR, INVALID_JSON, INVALID_PAYLOAD, AGENT_NOT_FOUND,
            AGENT_NOT_CONNECTED, UNAUTHORIZED, FORBIDDEN, INTERNAL_ERROR, UNKNOWN_MESSAGE_TYPE,
            TOKEN_MISSING, TOKEN_INVALID, TOKEN_EXPIRED, TOKEN_REVOKED, TOKEN_BLACKLISTED,
            SIGNATURE_INVALID, SECRET_NOT_FOUND,
            AGENT_CONNECT_FAILED, AGENT_DISCONNECTED, AGENT_NOT_AUTHORIZED,
            AGENT_LIMIT_EXCEEDED, AGENT_PAUSED, AGENT_EMERGENCY_STOP, PROTOCOL_VERSION_MISMATCH,
            EXEC_SUCCESS, EXEC_TIMEOUT, EXEC_BLOCKED, EXEC_BLOCKED_READONLY, EXEC_BLOCKED_SANDBOX,
            EXEC_BLOCKED_SUDO, EXEC_FAILED, EXEC_NEEDS_APPROVAL, EXEC_AWAITING_APPROVAL,
            EXEC_REJECTED, EXEC_APPROVED,
            FILE_EMPTY_PATH, FILE_INVALID_PATH, FILE_NOT_FOUND, FILE_TOO_LARGE,
            FILE_READ_ERROR, FILE_WRITE_ERROR,
            CONFIG_APPLIED, CONFIG_FAILED, CONFIG_LOAD_ERROR,
            SKILL_ALREADY_EXISTS, SKILL_NOT_FOUND,
            TASK_ACCEPTED, TASK_ERROR, TASK_DONE,
            KILL_SWITCH_DISK_FULL, KILL_SWITCH_CPU_HIGH, KILL_SWITCH_PAUSED,
            KILL_SWITCH_RESUMED, KILL_SWITCH_EMERGENCY,
            BACKUP_CREATE_ERROR, BACKUP_SNAPSHOT_NOT_FOUND, BACKUP_RESTORE_ERROR,
            BACKUP_DELETE_ERROR, BACKUP_CHECKSUM_MISMATCH,
            MCP_AGENT_NOT_FOUND, MCP_TIMEOUT,
            LLM_ALL_BACKENDS_DOWN, LLM_REQUEST_ERROR,
            SHIELD_BLOCKED, SHIELD_SNAPSHOT_CREATED, SHIELD_PROCESS_STOPPED,
            SHIELD_PROCESS_KILLED, SHIELD_TIMEOUT, SHIELD_ALERT_SENT,
        ]
    }

    #[test]
    fn test_all_codes_are_non_empty() {
        for code in all_codes() {
            assert!(!code.is_empty(), "Code should not be empty");
        }
    }

    #[test]
    fn test_no_duplicate_codes() {
        let codes = all_codes();
        let mut seen = std::collections::HashSet::new();
        for code in &codes {
            assert!(seen.insert(*code), "Duplicate code: {}", code);
        }
    }

    #[test]
    fn test_auth_codes_exist() {
        assert_eq!(TOKEN_MISSING, "TOKEN_MISSING");
        assert_eq!(TOKEN_INVALID, "TOKEN_INVALID");
        assert_eq!(TOKEN_EXPIRED, "TOKEN_EXPIRED");
        assert_eq!(TOKEN_REVOKED, "TOKEN_REVOKED");
    }

    #[test]
    fn test_exec_codes_exist() {
        assert_eq!(EXEC_SUCCESS, "EXEC_SUCCESS");
        assert_eq!(EXEC_TIMEOUT, "EXEC_TIMEOUT");
        assert_eq!(EXEC_BLOCKED, "EXEC_BLOCKED");
        assert_eq!(EXEC_FAILED, "EXEC_FAILED");
    }

    #[test]
    fn test_file_codes_exist() {
        assert_eq!(FILE_NOT_FOUND, "FILE_NOT_FOUND");
        assert_eq!(FILE_TOO_LARGE, "FILE_TOO_LARGE");
        assert_eq!(FILE_EMPTY_PATH, "FILE_EMPTY_PATH");
    }

    #[test]
    fn test_backup_codes_exist() {
        assert_eq!(BACKUP_CREATE_ERROR, "BACKUP_CREATE_ERROR");
        assert_eq!(BACKUP_RESTORE_ERROR, "BACKUP_RESTORE_ERROR");
        assert_eq!(BACKUP_CHECKSUM_MISMATCH, "BACKUP_CHECKSUM_MISMATCH");
    }
}
