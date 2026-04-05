package protocol

import (
	"strings"
	"testing"
)

func TestAllErrorCodesAreEnglish(t *testing.T) {
	// Every error code must have an English translation
	for code, msg := range enMessages {
		if msg == "" {
			t.Errorf("empty English message for code %q", code)
		}
		// Code should be SCREAMING_SNAKE_CASE
		if strings.ToLower(code) == code {
			t.Errorf("code %q should be SCREAMING_SNAKE_CASE", code)
		}
	}
}

func TestAllErrorCodesHaveRussianTranslation(t *testing.T) {
	// Every English code must have a Russian translation
	for code := range enMessages {
		if _, ok := ruMessages[code]; !ok {
			t.Errorf("missing Russian translation for code %q", code)
		}
	}
	// No extra Russian codes without English
	for code := range ruMessages {
		if _, ok := enMessages[code]; !ok {
			t.Errorf("extra Russian code without English: %q", code)
		}
	}
}

func TestTFallback(t *testing.T) {
	// Non-existent code returns the code itself
	got := T("NONEXISTENT_CODE")
	if got != "NONEXISTENT_CODE" {
		t.Errorf("expected code itself, got %q", got)
	}
}

func TestSetLocale(t *testing.T) {
	// Reset after test
	defer SetLocale("en")

	SetLocale("ru")
	got := T(CodeAgentNotConnected)
	if got != "Агент не подключён" {
		t.Errorf("expected Russian message, got %q", got)
	}

	SetLocale("en")
	got = T(CodeAgentNotConnected)
	if got != "Agent not connected" {
		t.Errorf("expected English message, got %q", got)
	}
}

func TestTf(t *testing.T) {
	got := Tf(CodeExecTimeout, 60)
	expected := "Command timed out after 60 seconds"
	if got != expected {
		t.Errorf("expected %q, got %q", expected, got)
	}
}

func TestProtoError(t *testing.T) {
	err := Err(CodeAgentNotConnected)
	if err.Code != CodeAgentNotConnected {
		t.Errorf("expected code %q, got %q", CodeAgentNotConnected, err.Code)
	}
	if err.Error() == "" {
		t.Error("expected non-empty error string")
	}
}

func TestErrCause(t *testing.T) {
	inner := Err(CodeInvalidJSON)
	outer := ErrCause(CodeInternalError, inner)
	if !strings.Contains(outer.Error(), CodeInternalError) {
		t.Error("outer error should contain its code")
	}
	if outer.Unwrap() != inner {
		t.Error("Unwrap should return inner error")
	}
}

func TestErrorPayloadFromCode(t *testing.T) {
	p := ErrorPayloadFromCode(CodeAgentNotConnected)
	if p.Code != CodeAgentNotConnected {
		t.Errorf("expected code %q, got %q", CodeAgentNotConnected, p.Code)
	}
	if p.Message == "" {
		t.Error("expected non-empty message")
	}
}

func TestErrorPayloadFromError(t *testing.T) {
	p := ErrorPayloadFromError(CodeFileNotFound, Err(CodeFileNotFound, "/tmp/test"))
	if p.Code != CodeFileNotFound {
		t.Errorf("expected code %q, got %q", CodeFileNotFound, p.Code)
	}
}

func TestConvenienceConstructors(t *testing.T) {
	tests := []struct {
		name string
		err  *ProtoError
		code string
	}{
		{"ExecBlockedReadOnly", ErrExecBlockedReadOnly("rm -rf /"), CodeExecBlockedReadOnly},
		{"ExecBlockedSandbox", ErrExecBlockedSandbox("cmd", "rm"), CodeExecBlockedSandbox},
		{"ExecBlockedSudo", ErrExecBlockedSudo("sudo rm"), CodeExecBlockedSudo},
		{"ExecTimeout", ErrExecTimeout(120), CodeExecTimeout},
		{"SkillExists", ErrSkillExists("test"), CodeSkillAlreadyExists},
		{"SkillNotFound", ErrSkillNotFound("test"), CodeSkillNotFound},
		{"FileNotFound", ErrFileNotFound("/tmp/x"), CodeFileNotFound},
		{"FileTooLarge", ErrFileTooLarge("/tmp/x", 999, 100), CodeFileTooLarge},
		{"ClientNotFound", ErrClientNotFound("c1"), CodeClientNotFound},
		{"AgentNotFound", ErrAgentNotFound("a1"), CodeAgentNotFound},
	}
	for _, tt := range tests {
		t.Run(tt.name, func(t *testing.T) {
			if tt.err.Code != tt.code {
				t.Errorf("expected code %q, got %q", tt.code, tt.err.Code)
			}
		})
	}
}

func TestRegisterLocale(t *testing.T) {
	defer SetLocale("en")

	RegisterLocale("de", map[string]string{
		CodeAgentNotConnected: "Agent nicht verbunden",
	})
	SetLocale("de")
	got := T(CodeAgentNotConnected)
	if got != "Agent nicht verbunden" {
		t.Errorf("expected German message, got %q", got)
	}
}
