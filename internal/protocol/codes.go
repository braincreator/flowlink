package protocol

// =============================================================================
// Error Codes — machine-readable identifiers for all protocol errors.
// These are sent over the wire (JSON). NEVER change existing codes — only add new ones.
// Format: CATEGORY_SPECIFIC_DETAIL (SCREAMING_SNAKE_CASE)
// =============================================================================

// --- General ---
const (
	CodeOK                = "OK"
	CodeUnknownError      = "UNKNOWN_ERROR"
	CodeInvalidJSON       = "INVALID_JSON"
	CodeInvalidPayload    = "INVALID_PAYLOAD"
	CodeAgentNotFound     = "AGENT_NOT_FOUND"
	CodeAgentNotConnected = "AGENT_NOT_CONNECTED"
	CodeUnauthorized      = "UNAUTHORIZED"
	CodeForbidden         = "FORBIDDEN"
	CodeInternalError     = "INTERNAL_ERROR"
	CodeUnknownMessage    = "UNKNOWN_MESSAGE_TYPE"
)

// --- Authentication ---
const (
	CodeTokenMissing       = "TOKEN_MISSING"
	CodeTokenInvalid       = "TOKEN_INVALID"
	CodeTokenExpired       = "TOKEN_EXPIRED"
	CodeTokenRevoked       = "TOKEN_REVOKED"
	CodeTokenBlacklisted   = "TOKEN_BLACKLISTED"
	CodeSignatureInvalid   = "SIGNATURE_INVALID"
	CodeSecretNotFound     = "SECRET_NOT_FOUND"
	CodeTokenTypeInvalid   = "TOKEN_TYPE_INVALID"
	CodeTokenDecodeFailed  = "TOKEN_DECODE_FAILED"
	CodeTokenParseFailed   = "TOKEN_PARSE_FAILED"
	CodeTokenSerializeFail = "TOKEN_SERIALIZE_FAILED"
)

// --- Agent Lifecycle ---
const (
	CodeAgentConnectFailed = "AGENT_CONNECT_FAILED"
	CodeAgentDisconnected  = "AGENT_DISCONNECTED"
	CodeAgentReadFailed    = "AGENT_READ_FAILED"
	CodeAgentWriteFailed   = "AGENT_WRITE_FAILED"
	CodeAgentNotAuthorized = "AGENT_NOT_AUTHORIZED"
	CodeAgentLimitExceeded = "AGENT_LIMIT_EXCEEDED"
	CodeAgentPaused        = "AGENT_PAUSED"
	CodeAgentEmergencyStop = "AGENT_EMERGENCY_STOP"
)

// --- Execution ---
const (
	CodeExecSuccess         = "EXEC_SUCCESS"
	CodeExecTimeout         = "EXEC_TIMEOUT"
	CodeExecBlocked         = "EXEC_BLOCKED"
	CodeExecBlockedReadOnly = "EXEC_BLOCKED_READONLY"
	CodeExecBlockedSandbox  = "EXEC_BLOCKED_SANDBOX"
	CodeExecBlockedSudo     = "EXEC_BLOCKED_SUDO"
	CodeExecFailed          = "EXEC_FAILED"
	CodeExecNeedsApproval   = "EXEC_NEEDS_APPROVAL"
	CodeExecAwaitingApproval = "EXEC_AWAITING_APPROVAL"
	CodeExecRejected        = "EXEC_REJECTED"
	CodeExecApproved        = "EXEC_APPROVED"
)

// --- File Operations ---
const (
	CodeFileEmptyPath     = "FILE_EMPTY_PATH"
	CodeFileInvalidPath   = "FILE_INVALID_PATH"
	CodeFileNotFound      = "FILE_NOT_FOUND"
	CodeFileTooLarge      = "FILE_TOO_LARGE"
	CodeFileReadError     = "FILE_READ_ERROR"
	CodeFileWriteError    = "FILE_WRITE_ERROR"
	CodeFileDecodeError   = "FILE_DECODE_ERROR"
	CodeFileDirCreateError = "FILE_DIR_CREATE_ERROR"
	CodeFileParentDirError = "FILE_PARENT_DIR_ERROR"
	CodeFileDirReadError  = "FILE_DIR_READ_ERROR"
)

// --- Config ---
const (
	CodeConfigApplied   = "CONFIG_APPLIED"
	CodeConfigFailed    = "CONFIG_UPDATE_FAILED"
	CodeConfigLoadError = "CONFIG_LOAD_ERROR"
	CodeConfigSaveError = "CONFIG_SAVE_ERROR"
	CodeConfigParseError = "CONFIG_PARSE_ERROR"
)

// --- Skills ---
const (
	CodeSkillAlreadyExists = "SKILL_ALREADY_EXISTS"
	CodeSkillNotFound      = "SKILL_NOT_FOUND"
	CodeSkillSaveError     = "SKILL_SAVE_ERROR"
	CodeSkillDeleteError   = "SKILL_DELETE_ERROR"
	CodeSkillDirError      = "SKILL_DIR_CREATE_ERROR"
	CodeSkillSerializeError = "SKILL_SERIALIZE_ERROR"
)

// --- Tasks (Autonomous) ---
const (
	CodeTaskAccepted  = "TASK_ACCEPTED"
	CodeTaskError     = "TASK_ERROR"
	CodeTaskCancelError = "TASK_CANCEL_ERROR"
	CodeTaskStepStart = "TASK_STEP_START"
	CodeTaskStepDone  = "TASK_STEP_DONE"
	CodeTaskDone      = "TASK_DONE"
	CodeTaskErrorDone = "TASK_ERROR_DONE"
)

// --- Kill Switch ---
const (
	CodeKillSwitchDiskFull = "KILL_SWITCH_DISK_FULL"
	CodeKillSwitchCPUHigh  = "KILL_SWITCH_CPU_HIGH"
	CodeKillSwitchPause    = "KILL_SWITCH_PAUSED"
	CodeKillSwitchResume   = "KILL_SWITCH_RESUMED"
	CodeKillSwitchEmergency = "KILL_SWITCH_EMERGENCY"
)

// --- Backup ---
const (
	CodeBackupEmptyPaths     = "BACKUP_EMPTY_PATHS"
	CodeBackupCreateError    = "BACKUP_CREATE_ERROR"
	CodeBackupArchiveError   = "BACKUP_ARCHIVE_ERROR"
	CodeBackupSnapshotNotFound = "BACKUP_SNAPSHOT_NOT_FOUND"
	CodeBackupRestoreError   = "BACKUP_RESTORE_ERROR"
	CodeBackupRestoreOpenError = "BACKUP_RESTORE_OPEN_ERROR"
	CodeBackupRestoreGzipError = "BACKUP_RESTORE_GZIP_ERROR"
	CodeBackupRestoreReadError = "BACKUP_RESTORE_READ_ERROR"
	CodeBackupRestoreDirError  = "BACKUP_RESTORE_DIR_ERROR"
	CodeBackupRestoreFileError = "BACKUP_RESTORE_FILE_ERROR"
	CodeBackupDeleteError   = "BACKUP_DELETE_ERROR"
	CodeBackupMetadataError  = "BACKUP_METADATA_ERROR"
	CodeBackupDirCreateError = "BACKUP_DIR_CREATE_ERROR"
	CodeBackupSerializeError = "BACKUP_SERIALIZE_ERROR"
	CodeBackupGlobError     = "BACKUP_GLOB_ERROR"
	CodeBackupFileAddError  = "BACKUP_FILE_ADD_ERROR"
	CodeBackupCleanup       = "BACKUP_CLEANUP"
)

// --- Audit ---
const (
	CodeAuditDirCreateError = "AUDIT_DIR_CREATE_ERROR"
	CodeAuditFileOpenError  = "AUDIT_FILE_OPEN_ERROR"
	CodeAuditSerializeError = "AUDIT_SERIALIZE_ERROR"
	CodeAuditWriteError     = "AUDIT_WRITE_ERROR"
	CodeAuditFormatUnsupported = "AUDIT_FORMAT_UNSUPPORTED"
)

// --- Registry ---
const (
	CodeClientNotFound    = "CLIENT_NOT_FOUND"
	CodeClientDeactivated = "CLIENT_DEACTIVATED"
	CodeClientCreateError = "CLIENT_CREATE_ERROR"
	CodeClientSaveError   = "CLIENT_SAVE_ERROR"
	CodeClientLoadError   = "CLIENT_LOAD_ERROR"
	CodeRegistryLoadError = "REGISTRY_LOAD_ERROR"
	CodeRegistrySaveError = "REGISTRY_SAVE_ERROR"
	CodeTokenGenerateError = "TOKEN_GENERATE_ERROR"
)

// --- TLS ---
const (
	CodeTLSKeyGenerateError    = "TLS_KEY_GENERATE_ERROR"
	CodeTLSSerialGenerateError = "TLS_SERIAL_GENERATE_ERROR"
	CodeTLSCertCreateError     = "TLS_CERT_CREATE_ERROR"
	CodeTLSCertDirError        = "TLS_CERT_DIR_CREATE_ERROR"
	CodeTLSCertWriteError      = "TLS_CERT_WRITE_ERROR"
	CodeTLSCertLoadError       = "TLS_CERT_LOAD_ERROR"
	CodeTLSCertMissing         = "TLS_CERT_MISSING"
	CodeTLSModeUnknown         = "TLS_MODE_UNKNOWN"
)

// --- MCP ---
const (
	CodeMCPAgentNotFound  = "MCP_AGENT_NOT_FOUND"
	CodeMCPTimeout        = "MCP_TIMEOUT"
	CodeMCPAgentError     = "MCP_AGENT_ERROR"
)

// --- LLM Proxy ---
const (
	CodeLLMAllBackendsDown = "LLM_ALL_BACKENDS_DOWN"
	CodeLLMRequestError    = "LLM_REQUEST_ERROR"
	CodeLLMResponseError   = "LLM_RESPONSE_ERROR"
	CodeLLMEmptyResponse   = "LLM_EMPTY_RESPONSE"
	CodeLLMParseError      = "LLM_PARSE_ERROR"
)
