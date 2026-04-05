// Package relay — TLS функции для реле.
// Поддержка self-signed сертификатов, Let's Encrypt и certificate pinning.
package relay

import (
	"github.com/braincreator/flowlink/internal/protocol"
	"crypto/rand"
	"crypto/rsa"
	"crypto/sha256"
	"crypto/tls"
	"crypto/x509"
	"crypto/x509/pkix"
	"encoding/hex"
	"encoding/pem"
	"fmt"
	"log/slog"
	"math/big"
	"net"
	"os"
	"path/filepath"
	"time"

	"golang.org/x/crypto/acme/autocert"
)

// TLSMode — режим TLS.
type TLSMode string

const (
	TLSModeSelfSigned  TLSMode = "self-signed"  // самоподписанный (dev)
	TLSModeLetsEncrypt TLSMode = "letsencrypt"  // Let's Encrypt (production)
	TLSModeManual      TLSMode = "manual"       // ручное управление сертификатами
)

// CertManager — менеджер сертификатов.
type CertManager struct {
	mode      TLSMode
	certPath  string
	keyPath   string
	domain    string
	cacheDir  string
	logger    *slog.Logger

	// Для autocert
	autoCertManager *autocert.Manager
}

// NewCertManager — создаёт менеджер сертификатов.
func NewCertManager(mode TLSMode, certPath, keyPath, domain, cacheDir string) *CertManager {
	return &CertManager{
		mode:     mode,
		certPath: certPath,
		keyPath:  keyPath,
		domain:   domain,
		cacheDir: cacheDir,
		logger:   slog.Default(),
	}
}

// GenerateSelfSignedCert — генерирует самоподписанный сертификат для dev.
// RSA 2048, Valid: 365 дней, SAN: domain, localhost, 127.0.0.1
func GenerateSelfSignedCert(domain, certPath, keyPath string) error {
	// Генерируем RSA 2048
	priv, err := rsa.GenerateKey(rand.Reader, 2048)
	if err != nil {
		return protocol.ErrCause(protocol.CodeTLSKeyGenerateError, err)
	}

	// Серийный номер
	serialNumber, err := rand.Int(rand.Reader, new(big.Int).Lsh(big.NewInt(1), 128))
	if err != nil {
		return protocol.ErrCause(protocol.CodeTLSSerialGenerateError, err)
	}

	// Шаблон сертификата
	template := x509.Certificate{
		SerialNumber: serialNumber,
		Subject: pkix.Name{
			Organization: []string{"FlowLink Dev"},
			CommonName:   domain,
		},
		NotBefore: time.Now(),
		NotAfter:  time.Now().Add(365 * 24 * time.Hour), // 365 дней

		KeyUsage:              x509.KeyUsageKeyEncipherment | x509.KeyUsageDigitalSignature,
		ExtKeyUsage:           []x509.ExtKeyUsage{x509.ExtKeyUsageServerAuth},
		BasicConstraintsValid: true,

		// SAN — Subject Alternative Names
		DNSNames:    []string{domain, "localhost"},
		IPAddresses: []net.IP{net.IPv4(127, 0, 0, 1)},
	}

	// Создаём сертификат
	certDER, err := x509.CreateCertificate(rand.Reader, &template, &template, &priv.PublicKey, priv)
	if err != nil {
		return protocol.ErrCause(protocol.CodeTLSCertCreateError, err)
	}

	// Создаём директории если нужно
	if err := os.MkdirAll(filepath.Dir(certPath), 0700); err != nil {
		return protocol.ErrCause(protocol.CodeTLSCertDirError, err)
	}
	if err := os.MkdirAll(filepath.Dir(keyPath), 0700); err != nil {
		return protocol.ErrCause(protocol.CodeTLSCertDirError, err)
	}

	// Записываем сертификат
	certFile, err := os.Create(certPath)
	if err != nil {
		return protocol.ErrCause(protocol.CodeTLSCertCreateError, err)
	}
	defer certFile.Close()

	if err := pem.Encode(certFile, &pem.Block{
		Type:  "CERTIFICATE",
		Bytes: certDER,
	}); err != nil {
		return protocol.ErrCause(protocol.CodeTLSCertWriteError, err)
	}

	// Записываем ключ
	keyFile, err := os.OpenFile(keyPath, os.O_WRONLY|os.O_CREATE|os.O_TRUNC, 0600)
	if err != nil {
		return protocol.ErrCause(protocol.CodeTLSCertWriteError, err)
	}
	defer keyFile.Close()

	privBytes, err := x509.MarshalPKCS8PrivateKey(priv)
	if err != nil {
		return fmt.Errorf("marshal ключа: %w", err)
	}

	if err := pem.Encode(keyFile, &pem.Block{
		Type:  "PRIVATE KEY",
		Bytes: privBytes,
	}); err != nil {
		return protocol.ErrCause(protocol.CodeTLSCertWriteError, err)
	}

	return nil
}

// LoadOrGenerate — загружает существующий сертификат или генерирует новый.
func LoadOrGenerate(certPath, keyPath, domain string) (*tls.Certificate, error) {
	// Проверяем существуют ли файлы
	certExists := fileExists(certPath)
	keyExists := fileExists(keyPath)

	// Если оба существуют — загружаем
	if certExists && keyExists {
		cert, err := tls.LoadX509KeyPair(certPath, keyPath)
		if err != nil {
			return nil, protocol.ErrCause(protocol.CodeTLSCertLoadError, err)
		}
		return &cert, nil
	}

	// Иначе генерируем
	if err := GenerateSelfSignedCert(domain, certPath, keyPath); err != nil {
		return nil, protocol.ErrCause(protocol.CodeTLSCertCreateError, err)
	}

	// Загружаем только что созданный
	cert, err := tls.LoadX509KeyPair(certPath, keyPath)
	if err != nil {
		return nil, protocol.ErrCause(protocol.CodeTLSCertLoadError, err)
	}

	return &cert, nil
}

// AutoTLSConfig — создаёт конфигурацию TLS для Let's Encrypt.
// Использует golang.org/x/crypto/acme/autocert
func AutoTLSConfig(domain, cacheDir string) (*tls.Config, error) {
	if domain == "" {
		return nil, fmt.Errorf("domain обязателен для Let's Encrypt")
	}

	// Создаём директорию для кэша
	if err := os.MkdirAll(cacheDir, 0700); err != nil {
		return nil, protocol.ErrCause(protocol.CodeTLSCertDirError, err)
	}

	// Создаём autocert manager
	m := &autocert.Manager{
		Prompt:     autocert.AcceptTOS,
		HostPolicy: autocert.HostWhitelist(domain),
		Cache:      autocert.DirCache(cacheDir),
	}

	// Возвращаем TLS конфиг
	tlsConfig := &tls.Config{
		GetCertificate: m.GetCertificate,
		MinVersion:     tls.VersionTLS12,
	}

	return tlsConfig, nil
}

// CertFingerprint — возвращает SHA256 fingerprint сертификата.
// Формат: "sha256:HEX..."
func CertFingerprint(cert *x509.Certificate) string {
	if cert == nil {
		return ""
	}

	hash := sha256.Sum256(cert.Raw)
	return "sha256:" + hex.EncodeToString(hash[:])
}

// ValidateCertPinning — проверяет что fingerprint сертификата совпадает с ожидаемым.
func ValidateCertPinning(expectedFingerprint string, cert *x509.Certificate) error {
	if expectedFingerprint == "" {
		return nil // pinning отключён
	}

	if cert == nil {
		return protocol.Err(protocol.CodeTLSCertMissing)
	}

	actualFingerprint := CertFingerprint(cert)
	if actualFingerprint != expectedFingerprint {
		return fmt.Errorf("fingerprint не совпадает: ожидается %s, получен %s",
			expectedFingerprint, actualFingerprint)
	}

	return nil
}

// GetTLSConfig — возвращает tls.Config в зависимости от режима.
func (cm *CertManager) GetTLSConfig() (*tls.Config, error) {
	switch cm.mode {
	case TLSModeSelfSigned:
		return cm.getSelfSignedConfig()
	case TLSModeLetsEncrypt:
		return cm.getLetsEncryptConfig()
	case TLSModeManual:
		return cm.getManualConfig()
	default:
		return nil, protocol.Err(protocol.CodeTLSModeUnknown, cm.mode)
	}
}

// getSelfSignedConfig — конфиг для self-signed сертификатов.
func (cm *CertManager) getSelfSignedConfig() (*tls.Config, error) {
	cert, err := LoadOrGenerate(cm.certPath, cm.keyPath, cm.domain)
	if err != nil {
		return nil, err
	}

	cm.logger.Info("using self-signed certificate",
		"cert", cm.certPath,
		"fingerprint", cm.getCertFingerprint(cert))

	return &tls.Config{
		Certificates: []tls.Certificate{*cert},
		MinVersion:   tls.VersionTLS12,
	}, nil
}

// getLetsEncryptConfig — конфиг для Let's Encrypt.
func (cm *CertManager) getLetsEncryptConfig() (*tls.Config, error) {
	tlsConfig, err := AutoTLSConfig(cm.domain, cm.cacheDir)
	if err != nil {
		return nil, err
	}

	cm.logger.Info("using Let's Encrypt", "domain", cm.domain)

	return tlsConfig, nil
}

// getManualConfig — конфиг для ручного управления сертификатами.
func (cm *CertManager) getManualConfig() (*tls.Config, error) {
	cert, err := tls.LoadX509KeyPair(cm.certPath, cm.keyPath)
	if err != nil {
		return nil, protocol.ErrCause(protocol.CodeTLSCertLoadError, err)
	}

	cm.logger.Info("using manual certificate", "cert", cm.certPath)

	return &tls.Config{
		Certificates: []tls.Certificate{cert},
		MinVersion:   tls.VersionTLS12,
	}, nil
}

// GetAutoCertManager — возвращает autocert.Manager для Let's Encrypt.
func (cm *CertManager) GetAutoCertManager() *autocert.Manager {
	return cm.autoCertManager
}

// getCertFingerprint — возвращает fingerprint из tls.Certificate.
func (cm *CertManager) getCertFingerprint(cert *tls.Certificate) string {
	if cert == nil || cert.Leaf == nil {
		// Парсим сертификат если Leaf не заполнен
		if len(cert.Certificate) > 0 {
			parsed, err := x509.ParseCertificate(cert.Certificate[0])
			if err != nil {
				return ""
			}
			return CertFingerprint(parsed)
		}
		return ""
	}
	return CertFingerprint(cert.Leaf)
}

// fileExists — проверяет существует ли файл.
func fileExists(path string) bool {
	_, err := os.Stat(path)
	return err == nil
}
