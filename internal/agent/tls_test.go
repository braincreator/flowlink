package agent

import (
	"crypto/tls"
	"crypto/x509"
	"crypto/x509/pkix"
	"encoding/hex"
	"math/big"
	"strings"
	"testing"
	"time"
)

// TestTLSConfig_Insecure tests insecure TLS config
func TestTLSConfig_Insecure(t *testing.T) {
	cfg := TLSConfig(true, "")

	if cfg == nil {
		t.Fatal("expected non-nil config")
	}

	if !cfg.InsecureSkipVerify {
		t.Error("expected InsecureSkipVerify to be true")
	}

	if cfg.MinVersion != tls.VersionTLS12 {
		t.Error("expected TLS 1.2 minimum")
	}
}

// TestTLSConfig_Standard tests standard TLS config
func TestTLSConfig_Standard(t *testing.T) {
	cfg := TLSConfig(false, "")

	if cfg == nil {
		t.Fatal("expected non-nil config")
	}

	if cfg.InsecureSkipVerify {
		t.Error("expected InsecureSkipVerify to be false")
	}

	if cfg.MinVersion != tls.VersionTLS12 {
		t.Error("expected TLS 1.2 minimum")
	}
}

// TestTLSConfig_PinFingerprint tests certificate pinning config
func TestTLSConfig_PinFingerprint(t *testing.T) {
	fingerprint := "sha256:" + strings.Repeat("a", 64)
	cfg := TLSConfig(false, fingerprint)

	if cfg == nil {
		t.Fatal("expected non-nil config")
	}

	if cfg.InsecureSkipVerify {
		t.Error("expected InsecureSkipVerify to be false")
	}

	if cfg.VerifyConnection == nil {
		t.Error("expected VerifyConnection to be set for pinning")
	}
}

// TestTLSInsecureSkip tests insecure skip helper
func TestTLSInsecureSkip(t *testing.T) {
	cfg := TLSInsecureSkip()

	if cfg == nil {
		t.Fatal("expected non-nil config")
	}

	if !cfg.InsecureSkipVerify {
		t.Error("expected InsecureSkipVerify to be true")
	}

	if cfg.MinVersion != tls.VersionTLS12 {
		t.Error("expected TLS 1.2 minimum")
	}
}

// TestTLSPinnedVerify tests pinning verify function
func TestTLSPinnedVerify(t *testing.T) {
	// Create a test certificate
	template := &x509.Certificate{
		SerialNumber: big.NewInt(1),
		Subject:      pkix.Name{CommonName: "test"},
		NotBefore:    time.Now(),
		NotAfter:     time.Now().Add(time.Hour),
	}

	// Generate a self-signed cert (simplified)
	certBytes := make([]byte, 100)
	template.Raw = certBytes

	// Calculate fingerprint
	fingerprint := GetCertFingerprint(template)
	if fingerprint == "" {
		t.Error("expected non-empty fingerprint")
	}

	if !strings.HasPrefix(fingerprint, "sha256:") {
		t.Errorf("expected fingerprint to start with 'sha256:', got %s", fingerprint)
	}

	// Test with nil cert
	nilFingerprint := GetCertFingerprint(nil)
	if nilFingerprint != "" {
		t.Error("expected empty fingerprint for nil cert")
	}
}

// TestParseFingerprint tests fingerprint parsing
func TestParseFingerprint(t *testing.T) {
	tests := []struct {
		name    string
		input   string
		wantErr bool
		want    string
	}{
		{
			name:    "valid with sha256 prefix",
			input:   "sha256:" + strings.Repeat("ab", 32),
			wantErr: false,
			want:    "sha256:" + strings.Repeat("ab", 32),
		},
		{
			name:    "valid uppercase prefix",
			input:   "SHA256:" + strings.Repeat("ab", 32),
			wantErr: false,
			want:    "sha256:" + strings.Repeat("ab", 32),
		},
		{
			name:    "valid without prefix",
			input:   strings.Repeat("ab", 32),
			wantErr: false,
			want:    "sha256:" + strings.Repeat("ab", 32),
		},
		{
			name:    "invalid hex",
			input:   "ZZZZ",
			wantErr: true,
		},
		{
			name:    "empty string",
			input:   "",
			wantErr: false, // Empty string gets "sha256:" prefix
			want:    "sha256:",
		},
		{
			name:    "with whitespace",
			input:   "  " + strings.Repeat("ab", 32) + "  ",
			wantErr: false,
			want:    "sha256:" + strings.Repeat("ab", 32),
		},
	}

	for _, tt := range tests {
		t.Run(tt.name, func(t *testing.T) {
			result, err := ParseFingerprint(tt.input)

			if tt.wantErr {
				if err == nil {
					t.Error("expected error, got nil")
				}
			} else {
				if err != nil {
					t.Errorf("unexpected error: %v", err)
				}
				if result != tt.want {
					t.Errorf("expected %s, got %s", tt.want, result)
				}
			}
		})
	}
}

// TestVerifyPinning tests certificate pinning verification
func TestVerifyPinning(t *testing.T) {
	// Create test certificate
	template := &x509.Certificate{
		SerialNumber: big.NewInt(1),
		Subject:      pkix.Name{CommonName: "test"},
		NotBefore:    time.Now(),
		NotAfter:     time.Now().Add(time.Hour),
	}

	// Generate some bytes for Raw
	certBytes := make([]byte, 100)
	for i := range certBytes {
		certBytes[i] = byte(i)
	}
	template.Raw = certBytes

	// Calculate correct fingerprint
	correctFingerprint := GetCertFingerprint(template)

	// Test with correct fingerprint
	err := verifyPinning(correctFingerprint, []*x509.Certificate{template})
	if err != nil {
		t.Errorf("expected no error with correct fingerprint, got: %v", err)
	}

	// Test with wrong fingerprint
	wrongFingerprint := "sha256:" + strings.Repeat("ff", 32)
	err = verifyPinning(wrongFingerprint, []*x509.Certificate{template})
	if err == nil {
		t.Error("expected error with wrong fingerprint")
	}

	// Test with empty certs
	err = verifyPinning(correctFingerprint, []*x509.Certificate{})
	if err == nil {
		t.Error("expected error with empty certs")
	}

	// Test with nil certs
	err = verifyPinning(correctFingerprint, nil)
	if err == nil {
		t.Error("expected error with nil certs")
	}
}

// TestTLSPinnedVerifyFunction tests the returned verify function
func TestTLSPinnedVerifyFunction(t *testing.T) {
	template := &x509.Certificate{
		SerialNumber: big.NewInt(1),
		Subject:      pkix.Name{CommonName: "test"},
		NotBefore:    time.Now(),
		NotAfter:     time.Now().Add(time.Hour),
	}

	certBytes := make([]byte, 100)
	for i := range certBytes {
		certBytes[i] = byte(i)
	}
	template.Raw = certBytes

	fingerprint := GetCertFingerprint(template)

	verifyFn := TLSPinnedVerify(fingerprint)
	if verifyFn == nil {
		t.Fatal("expected non-nil verify function")
	}

	// Test with correct cert
	state := tls.ConnectionState{
		PeerCertificates: []*x509.Certificate{template},
	}

	err := verifyFn(state)
	if err != nil {
		t.Errorf("expected no error with correct cert, got: %v", err)
	}
}

// TestHexEncodingRoundTrip tests hex encoding consistency
func TestHexEncodingRoundTrip(t *testing.T) {
	original := []byte("test certificate data for encoding")

	encoded := hex.EncodeToString(original)
	decoded, err := hex.DecodeString(encoded)
	if err != nil {
		t.Fatalf("unexpected decode error: %v", err)
	}

	if string(decoded) != string(original) {
		t.Errorf("round-trip failed: expected %s, got %s", string(original), string(decoded))
	}
}


