package protocol

import "fmt"

// =============================================================================
// Error Builder — typed protocol errors with machine-readable codes.
// Use these instead of free-text error strings in payloads.
// =============================================================================

// ProtoError represents a structured protocol error.
// It carries a machine-readable Code and a human-readable Message.
type ProtoError struct {
	Code    string            `json:"code"`
	Message string            `json:"message,omitempty"`
	Details map[string]any    `json:"details,omitempty"`
	Cause   error             `json:"-"`
}

func (e *ProtoError) Error() string {
	if e.Cause != nil {
		return fmt.Sprintf("%s: %s: %v", e.Code, e.Message, e.Cause)
	}
	return fmt.Sprintf("%s: %s", e.Code, e.Message)
}

func (e *ProtoError) Unwrap() error { return e.Cause }

// --- Constructors ---

// Err creates a new ProtoError with the given code and optional format args for message.
// The message template is looked up from i18n (if available), or used as-is.
func Err(code string, args ...any) *ProtoError {
	msg := T(code)
	if len(args) > 0 {
		msg = fmt.Sprintf(msg, args...)
	}
	return &ProtoError{Code: code, Message: msg}
}

// ErrCause creates a ProtoError wrapping an underlying error.
func ErrCause(code string, cause error, args ...any) *ProtoError {
	msg := T(code)
	if len(args) > 0 {
		msg = fmt.Sprintf(msg, args...)
	}
	return &ProtoError{Code: code, Message: msg, Cause: cause}
}

// ErrDetails creates a ProtoError with structured details.
func ErrDetails(code string, details map[string]any, args ...any) *ProtoError {
	msg := T(code)
	if len(args) > 0 {
		msg = fmt.Sprintf(msg, args...)
	}
	return &ProtoError{Code: code, Message: msg, Details: details}
}

// --- Payload Helpers ---

// ErrorPayloadFromCode creates an ErrorPayload (for wire format) from a code.
func ErrorPayloadFromCode(code string, args ...any) ErrorPayload {
	msg := T(code)
	if len(args) > 0 {
		msg = fmt.Sprintf(msg, args...)
	}
	return ErrorPayload{Code: code, Message: msg}
}

// ErrorPayloadFromError creates an ErrorPayload from code + wrapped error.
func ErrorPayloadFromError(code string, cause error, args ...any) ErrorPayload {
	msg := T(code)
	if len(args) > 0 {
		msg = fmt.Sprintf(msg, args...)
	}
	if msg == "" {
		msg = cause.Error()
	}
	return ErrorPayload{Code: code, Message: msg}
}

// --- Convenience constructors for common patterns ---

// ErrInvalidPayload — shorthand for INVALID_PAYLOAD with wrapped error.
func ErrInvalidPayload(err error) *ProtoError {
	return ErrCause(CodeInvalidPayload, err)
}

// ErrAgentNotConnected — shorthand for AGENT_NOT_CONNECTED.
func ErrAgentNotConnected(agentID string) *ProtoError {
	return Err(CodeAgentNotConnected, agentID)
}

// ErrExecBlockedReadOnly — shorthand with details.
func ErrExecBlockedReadOnly(command string) *ProtoError {
	return ErrDetails(CodeExecBlockedReadOnly, map[string]any{
		"command": command,
		"mode":    "read_only",
	})
}

// ErrExecBlockedSandbox — shorthand with details.
func ErrExecBlockedSandbox(command, pattern string) *ProtoError {
	return ErrDetails(CodeExecBlockedSandbox, map[string]any{
		"command": command,
		"pattern": pattern,
	})
}

// ErrExecBlockedSudo — shorthand.
func ErrExecBlockedSudo(command string) *ProtoError {
	return ErrDetails(CodeExecBlockedSudo, map[string]any{
		"command": command,
		"rule":    "sudo_not_allowed",
	})
}

// ErrExecTimeout — shorthand with timeout value.
func ErrExecTimeout(timeoutSec int) *ProtoError {
	return Err(CodeExecTimeout, timeoutSec)
}

// ErrSkillExists — shorthand.
func ErrSkillExists(skillID string) *ProtoError {
	return Err(CodeSkillAlreadyExists, skillID)
}

// ErrSkillNotFound — shorthand.
func ErrSkillNotFound(skillID string) *ProtoError {
	return Err(CodeSkillNotFound, skillID)
}

// ErrFileNotFound — shorthand.
func ErrFileNotFound(path string) *ProtoError {
	return ErrCause(CodeFileNotFound, fmt.Errorf("%s", path))
}

// ErrFileTooLarge — shorthand.
func ErrFileTooLarge(path string, size int64, maxBytes int64) *ProtoError {
	return ErrDetails(CodeFileTooLarge, map[string]any{
		"path":      path,
		"size":      size,
		"max_bytes": maxBytes,
	})
}

// ErrSnapshotNotFound — shorthand.
func ErrSnapshotNotFound(id string) *ProtoError {
	return ErrCause(CodeBackupSnapshotNotFound, fmt.Errorf("%s", id))
}

// ErrClientNotFound — shorthand.
func ErrClientNotFound(id string) *ProtoError {
	return Err(CodeClientNotFound, id)
}

// ErrAgentNotFound — shorthand.
func ErrAgentNotFound(id string) *ProtoError {
	return Err(CodeAgentNotFound, id)
}
