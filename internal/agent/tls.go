// Package agent — TLS функции для агента.
// Поддержка insecure mode (dev), стандартной верификации и certificate pinning.
package agent

import (
	"github.com/braincreator/flowlink/internal/protocol"
	"crypto/sha256"
	"crypto/tls"
	"crypto/x509"
	"encoding/hex"
	"fmt"
	"strings"
)

// TLSConfig — создаёт tls.Config для агента.
// Параметры:
// - insecure: true = InsecureSkipVerify (dev режим)
// - pinFingerprint: "" = стандартная верификация, "sha256:..." = certificate pinning
func TLSConfig(insecure bool, pinFingerprint string) *tls.Config {
	// Dev режим — пропускаем верификацию
	if insecure {
		return &tls.Config{
			InsecureSkipVerify: true,
			MinVersion:         tls.VersionTLS12,
		}
	}

	// Certificate pinning
	if pinFingerprint != "" {
		return &tls.Config{
			InsecureSkipVerify: false,
			MinVersion:         tls.VersionTLS12,
			VerifyConnection: func(state tls.ConnectionState) error {
				return verifyPinning(pinFingerprint, state.PeerCertificates)
			},
		}
	}

	// Стандартная верификация
	return &tls.Config{
		InsecureSkipVerify: false,
		MinVersion:         tls.VersionTLS12,
	}
}

// verifyPinning — проверяет fingerprint сертификата.
func verifyPinning(expectedFingerprint string, certs []*x509.Certificate) error {
	if len(certs) == 0 {
		return protocol.Err(protocol.CodeTLSCertMissing)
	}

	// Проверяем первый сертификат (leaf)
	cert := certs[0]
	hash := sha256.Sum256(cert.Raw)
	actualFingerprint := "sha256:" + hex.EncodeToString(hash[:])

	// Нормализуем fingerprint (lowercase, без пробелов)
	expectedFingerprint = strings.ToLower(strings.TrimSpace(expectedFingerprint))
	actualFingerprint = strings.ToLower(strings.TrimSpace(actualFingerprint))

	if actualFingerprint != expectedFingerprint {
		return fmt.Errorf("certificate pinning failed: ожидается %s, получен %s",
			expectedFingerprint, actualFingerprint)
	}

	return nil
}

// TLSPinnedVerify — создаёт VerifyConnection для pinning.
// Используется для production с certificate pinning.
func TLSPinnedVerify(pinFingerprint string) func(state tls.ConnectionState) error {
	return func(state tls.ConnectionState) error {
		return verifyPinning(pinFingerprint, state.PeerCertificates)
	}
}

// TLSInsecureSkip — создаёт tls.Config для dev (пропуск верификации).
// НЕ ИСПОЛЬЗОВАТЬ В PRODUCTION!
func TLSInsecureSkip() *tls.Config {
	return &tls.Config{
		InsecureSkipVerify: true,
		MinVersion:         tls.VersionTLS12,
	}
}

// GetCertFingerprint — возвращает SHA256 fingerprint сертификата.
// Формат: "sha256:HEX..."
func GetCertFingerprint(cert *x509.Certificate) string {
	if cert == nil {
		return ""
	}

	hash := sha256.Sum256(cert.Raw)
	return "sha256:" + hex.EncodeToString(hash[:])
}

// ParseFingerprint — парсит fingerprint из строки.
// Поддерживает форматы: "sha256:HEX...", "SHA256:HEX...", "HEX..."
func ParseFingerprint(fingerprint string) (string, error) {
	fingerprint = strings.TrimSpace(fingerprint)

	// Если нет префикса sha256, добавляем
	if !strings.HasPrefix(strings.ToLower(fingerprint), "sha256:") {
		// Проверяем что это hex строка
		if _, err := hex.DecodeString(fingerprint); err != nil {
			return "", protocol.ErrCause(protocol.CodeSignatureInvalid, err)
		}
		fingerprint = "sha256:" + fingerprint
	}

	// Нормализуем в lowercase
	return strings.ToLower(fingerprint), nil
}
