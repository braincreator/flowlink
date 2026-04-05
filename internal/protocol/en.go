package protocol

// enMessages — English translations for all protocol error codes.
// This is the default locale.
var enMessages = map[string]string{
	// --- General ---
	CodeOK:                "OK",
	CodeUnknownError:      "Unknown error",
	CodeInvalidJSON:       "Invalid JSON",
	CodeInvalidPayload:    "Invalid payload: %v",
	CodeAgentNotFound:     "Agent not found: %s",
	CodeAgentNotConnected: "Agent not connected",
	CodeUnauthorized:      "Unauthorized",
	CodeForbidden:         "Forbidden",
	CodeInternalError:     "Internal error",
	CodeUnknownMessage:    "Unknown message type: %s",

	// --- Authentication ---
	CodeTokenMissing:       "Token missing",
	CodeTokenInvalid:       "Invalid token",
	CodeTokenExpired:       "Token expired",
	CodeTokenRevoked:       "Token revoked",
	CodeTokenBlacklisted:   "Token blacklisted",
	CodeSignatureInvalid:   "Invalid signature",
	CodeSecretNotFound:     "Secret not found",
	CodeTokenTypeInvalid:   "Invalid token type",
	CodeTokenDecodeFailed:  "Token decode failed",
	CodeTokenParseFailed:   "Token parse failed",
	CodeTokenSerializeFail: "Token serialize failed",

	// --- Agent Lifecycle ---
	CodeAgentConnectFailed: "Agent connect failed",
	CodeAgentDisconnected:  "Agent disconnected",
	CodeAgentReadFailed:    "Agent read failed",
	CodeAgentWriteFailed:   "Agent write failed",
	CodeAgentNotAuthorized: "Agent not authorized",
	CodeAgentLimitExceeded: "Agent limit exceeded (%d/%d)",
	CodeAgentPaused:        "Agent paused: %s",
	CodeAgentEmergencyStop: "Emergency stop: commands not executed",
	CodeProtocolVersionMismatch: "Protocol version mismatch: client %d, server %d",

	// --- Execution ---
	CodeExecSuccess:          "Command executed successfully",
	CodeExecTimeout:          "Command timed out after %d seconds",
	CodeExecBlocked:          "Command blocked",
	CodeExecBlockedReadOnly:  "Command blocked: agent is in read-only mode",
	CodeExecBlockedSandbox:   "Command blocked: matches sandbox pattern",
	CodeExecBlockedSudo:     "Command blocked: sudo not allowed",
	CodeExecFailed:           "Command failed",
	CodeExecNeedsApproval:    "Command needs approval",
	CodeExecAwaitingApproval: "Command awaiting approval",
	CodeExecRejected:         "Command rejected by user",
	CodeExecApproved:         "Command approved",

	// --- File Operations ---
	CodeFileEmptyPath:     "Empty path",
	CodeFileInvalidPath:   "Invalid path: %v",
	CodeFileNotFound:      "File not found: %s",
	CodeFileTooLarge:      "File too large: %d bytes (max %d bytes)",
	CodeFileReadError:     "File read error: %v",
	CodeFileWriteError:    "File write error: %v",
	CodeFileDecodeError:   "File decode error: %v",
	CodeFileDirCreateError: "Directory create error: %v",
	CodeFileParentDirError: "Parent directory create error: %v",
	CodeFileDirReadError:  "Directory read error: %v",

	// --- Config ---
	CodeConfigApplied:    "Configuration applied",
	CodeConfigFailed:     "Configuration update failed",
	CodeConfigLoadError:  "Config load error: %v",
	CodeConfigSaveError:  "Config save error: %v",
	CodeConfigParseError: "Config parse error: %v",

	// --- Skills ---
	CodeSkillAlreadyExists: "Skill %s already exists (use force_update)",
	CodeSkillNotFound:      "Skill %s not found",
	CodeSkillSaveError:     "Skill save error: %v",
	CodeSkillDeleteError:   "Skill delete error: %v",
	CodeSkillDirError:      "Skill directory create error: %v",
	CodeSkillSerializeError: "Skill serialize error: %v",

	// --- Tasks ---
	CodeTaskAccepted:    "Task accepted",
	CodeTaskError:       "Task error: %v",
	CodeTaskCancelError: "Task cancel error: %v",
	CodeTaskStepStart:   "Task step started",
	CodeTaskStepDone:    "Task step completed",
	CodeTaskDone:        "Task completed",
	CodeTaskErrorDone:   "Task completed with error",

	// --- Kill Switch ---
	CodeKillSwitchDiskFull:  "Disk almost full: %.1f%%",
	CodeKillSwitchCPUHigh:   "High CPU usage",
	CodeKillSwitchPause:     "Agent paused: %s",
	CodeKillSwitchResume:    "Agent resumed",
	CodeKillSwitchEmergency: "Emergency stop",

	// --- Backup ---
	CodeBackupEmptyPaths:       "Empty backup paths",
	CodeBackupCreateError:      "Backup create error: %v",
	CodeBackupArchiveError:     "Archive create error: %v",
	CodeBackupSnapshotNotFound: "Snapshot not found: %s",
	CodeBackupRestoreError:     "Backup restore error: %v",
	CodeBackupRestoreOpenError: "Backup open error: %v",
	CodeBackupRestoreGzipError: "Backup gzip error: %v",
	CodeBackupRestoreReadError: "Backup archive read error: %v",
	CodeBackupRestoreDirError:  "Backup directory create error: %v",
	CodeBackupRestoreFileError: "Backup file write error: %v",
	CodeBackupDeleteError:      "Backup delete error: %v",
	CodeBackupMetadataError:    "Backup metadata error: %v",
	CodeBackupDirCreateError:   "Backup directory create error: %v",
	CodeBackupSerializeError:   "Backup metadata serialize error: %v",
	CodeBackupGlobError:        "Backup glob error: %v",
	CodeBackupFileAddError:     "Backup file add error: %v",
	CodeBackupCleanup:          "Backup cleanup completed",
	CodeBackupChecksumCompute:  "Backup checksum compute error: %v",
	CodeBackupChecksumMismatch: "Backup checksum mismatch: expected %s, got %s",

	// --- Audit ---
	CodeAuditDirCreateError:    "Audit directory create error: %v",
	CodeAuditFileOpenError:     "Audit file open error: %v",
	CodeAuditSerializeError:    "Audit entry serialize error: %v",
	CodeAuditWriteError:        "Audit entry write error: %v",
	CodeAuditFormatUnsupported: "Unsupported audit format: %s",

	// --- Registry ---
	CodeClientNotFound:    "Client not found: %s",
	CodeClientDeactivated: "Client not found or deactivated: %s",
	CodeClientCreateError: "Client create error: %v",
	CodeClientSaveError:   "Client save error: %v",
	CodeClientLoadError:   "Client load error: %v",
	CodeRegistryLoadError: "Registry load error: %v",
	CodeRegistrySaveError: "Registry save error: %v",
	CodeTokenGenerateError: "Token generate error: %v",

	// --- TLS ---
	CodeTLSKeyGenerateError:    "TLS key generate error: %v",
	CodeTLSSerialGenerateError: "TLS serial generate error: %v",
	CodeTLSCertCreateError:     "TLS certificate create error: %v",
	CodeTLSCertDirError:        "TLS certificate directory create error: %v",
	CodeTLSCertWriteError:      "TLS certificate write error: %v",
	CodeTLSCertLoadError:       "TLS certificate load error: %v",
	CodeTLSCertMissing:         "TLS certificate missing",
	CodeTLSModeUnknown:         "Unknown TLS mode: %s",

	// --- MCP ---
	CodeMCPAgentNotFound: "Agent '%s' not found (connected: %d)",
	CodeMCPTimeout:       "Agent response timeout (%v)",
	CodeMCPAgentError:    "Agent error: %s",

	// --- LLM ---
	CodeLLMAllBackendsDown: "All LLM backends unavailable",
	CodeLLMRequestError:    "LLM request error: %v",
	CodeLLMResponseError:   "LLM response error: %v",
	CodeLLMEmptyResponse:   "Empty response from LLM",
	CodeLLMParseError:      "LLM response parse error: %v",
}
