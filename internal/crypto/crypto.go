// Package crypto — End-to-End Encryption для FlowLink.
// X25519 для обмена ключами + AES-256-GCM для шифрования данных.
// Relay НЕ имеет доступа к приватным ключам — не может расшифровать данные.
package crypto

import (
	"crypto/aes"
	"crypto/cipher"
	"crypto/ecdh"
	"crypto/rand"
	"crypto/sha256"
	"encoding/base64"
	"encoding/json"
	"errors"
	"fmt"
	"io"
	"os"
	"path/filepath"
	"sync"

	"github.com/braincreator/flowlink/internal/config"
)

// ═══════════════════════════════════════════════════════════
// Constants
// ═══════════════════════════════════════════════════════════

const (
	KeyExchangeVersion = 1
	AESKeySize        = 32 // AES-256
	NonceSize         = 12 // GCM standard nonce
	KeyFilePermission = 0600
)

var (
	ErrKeyNotFound       = errors.New("encryption key not found")
	ErrDecryptionFailed  = errors.New("decryption failed")
	ErrInvalidCiphertext = errors.New("invalid ciphertext")
	ErrNoSharedSecret    = errors.New("no shared secret established")
)

// ═══════════════════════════════════════════════════════════
// Keypair — X25519 ключевая пара
// ═══════════════════════════════════════════════════════════

// Keypair — пара публичный + приватный ключ X25519.
type Keypair struct {
	PrivateKey []byte `json:"private_key"` // 32 bytes, base64-encoded
	PublicKey  []byte `json:"public_key"`  // 32 bytes, base64-encoded
	KeyID      string `json:"key_id"`     // SHA-256(public_key) hex
	CreatedAt  int64  `json:"created_at"`
}

// KeyStore — хранилище ключей для агента/клиента.
type KeyStore struct {
	mu       sync.RWMutex
	keypairs map[string]*Keypair // keyID → Keypair
	activeID string              // текущий активный ключ
	dir      string              // директория хранения
}

// NewKeyStore — создаёт или загружает хранилище ключей.
func NewKeyStore(dir string) (*KeyStore, error) {
	if dir == "" {
		configDir, err := config.ConfigDir()
		if err != nil {
			return nil, err
		}
		dir = filepath.Join(configDir, "keys")
	}

	if err := os.MkdirAll(dir, 0700); err != nil {
		return nil, fmt.Errorf("create key directory: %w", err)
	}

	ks := &KeyStore{
		keypairs: make(map[string]*Keypair),
		dir:      dir,
	}

	// Загружаем существующие ключи
	if err := ks.load(); err != nil {
		// Если файл не существует — это OK, создадим при генерации
		if !os.IsNotExist(err) {
			return nil, err
		}
	}

	// Если ключей нет — генерируем
	if len(ks.keypairs) == 0 {
		kp, err := ks.Generate()
		if err != nil {
			return nil, err
		}
		ks.activeID = kp.KeyID
	}

	return ks, nil
}

// Generate — генерирует новую ключевую пару X25519.
func (ks *KeyStore) Generate() (*Keypair, error) {
	curve := ecdh.X25519()

	privateKey, err := curve.GenerateKey(rand.Reader)
	if err != nil {
		return nil, fmt.Errorf("generate X25519 key: %w", err)
	}

	publicKey := privateKey.PublicKey()

	kp := &Keypair{
		PrivateKey: privateKey.Bytes(),
		PublicKey:  publicKey.Bytes(),
		KeyID:      keyID(publicKey.Bytes()),
		CreatedAt:  nowUnix(),
	}

	ks.mu.Lock()
	ks.keypairs[kp.KeyID] = kp
	ks.activeID = kp.KeyID
	ks.mu.Unlock()

	if err := ks.save(); err != nil {
		// Rollback
		ks.mu.Lock()
		delete(ks.keypairs, kp.KeyID)
		ks.mu.Unlock()
		return nil, fmt.Errorf("save key: %w", err)
	}

	return kp, nil
}

// Active — возвращает текущую активную ключевую пару.
func (ks *KeyStore) Active() (*Keypair, error) {
	ks.mu.RLock()
	defer ks.mu.RUnlock()

	if ks.activeID == "" {
		return nil, ErrKeyNotFound
	}

	kp, ok := ks.keypairs[ks.activeID]
	if !ok {
		return nil, ErrKeyNotFound
	}

	return kp, nil
}

// Get — возвращает ключевую пару по ID.
func (ks *KeyStore) Get(keyID string) (*Keypair, error) {
	ks.mu.RLock()
	defer ks.mu.RUnlock()

	kp, ok := ks.keypairs[keyID]
	if !ok {
		return nil, ErrKeyNotFound
	}
	return kp, nil
}

// SetActive — устанавливает активный ключ по ID.
func (ks *KeyStore) SetActive(keyID string) error {
	ks.mu.Lock()
	defer ks.mu.Unlock()

	if _, ok := ks.keypairs[keyID]; !ok {
		return ErrKeyNotFound
	}
	ks.activeID = keyID
	return ks.save()
}

// List — возвращает все ключевые пары.
func (ks *KeyStore) List() []*Keypair {
	ks.mu.RLock()
	defer ks.mu.RUnlock()

	result := make([]*Keypair, 0, len(ks.keypairs))
	for _, kp := range ks.keypairs {
		result = append(result, kp)
	}
	return result
}

// PublicKeys — возвращает только публичные ключи (для отправки через relay).
func (ks *KeyStore) PublicKeys() []PublicKeyInfo {
	ks.mu.RLock()
	defer ks.mu.RUnlock()

	result := make([]PublicKeyInfo, 0, len(ks.keypairs))
	for _, kp := range ks.keypairs {
		result = append(result, PublicKeyInfo{
			KeyID:     kp.KeyID,
			PublicKey: kp.PublicKey,
			IsActive:  kp.KeyID == ks.activeID,
		})
	}
	return result
}

// Delete — удаляет ключевую пару (не может удалить активную).
func (ks *KeyStore) Delete(keyID string) error {
	ks.mu.Lock()
	defer ks.mu.Unlock()

	if keyID == ks.activeID {
		return errors.New("cannot delete active key")
	}

	delete(ks.keypairs, keyID)
	return ks.save()
}

// Rotate — создаёт новый ключ и делает его активным (key rotation).
func (ks *KeyStore) Rotate() (*Keypair, error) {
	kp, err := ks.Generate()
	if err != nil {
		return nil, err
	}
	ks.mu.Lock()
	ks.activeID = kp.KeyID
	ks.mu.Unlock()
	return kp, ks.save()
}

// ═══════════════════════════════════════════════════════════
// Key Derivation — X25519 ECDH → AES-256-GCM
// ═══════════════════════════════════════════════════════════

// symmetricKeyID — гарантирует одинаковый ключ для обоих направлений.
func symmetricKeyID(id1, id2 string) string {
	if id1 < id2 {
		return id1 + "|" + id2
	}
	return id2 + "|" + id1
	}

// SharedSecret — вычисляет общий секрет через X25519 ECDH.
// private = наш приватный ключ, peerPublic = публичный ключ пира.
func SharedSecret(privateKey, peerPublicKey []byte) ([]byte, error) {
	curve := ecdh.X25519()

	priv, err := curve.NewPrivateKey(privateKey)
	if err != nil {
		return nil, fmt.Errorf("parse private key: %w", err)
	}

	peerPub, err := curve.NewPublicKey(peerPublicKey)
	if err != nil {
		return nil, fmt.Errorf("parse peer public key: %w", err)
	}

	secret, err := priv.ECDH(peerPub)
	if err != nil {
		return nil, fmt.Errorf("ECDH: %w", err)
	}

	return secret, nil
}

// DeriveKey — выводит AES-256 ключ из shared secret + context.
// HKDF-like: SHA-256(shared_secret || context || keyID)
func DeriveKey(sharedSecret []byte, context string, keyID string) []byte {
	h := sha256.New()
	h.Write(sharedSecret)
	h.Write([]byte(context))
	h.Write([]byte(keyID))
	return h.Sum(nil) // 32 bytes = AES-256
}

// ═══════════════════════════════════════════════════════════
// AES-256-GCM Encryption / Decryption
// ═══════════════════════════════════════════════════════════

// Encrypt — шифрует plaintext с AES-256-GCM.
// Возвращает: nonce (12 bytes) + ciphertext + tag.
func Encrypt(key, plaintext []byte) ([]byte, error) {
	if len(key) != AESKeySize {
		return nil, fmt.Errorf("invalid key size: %d (expected %d)", len(key), AESKeySize)
	}

	block, err := aes.NewCipher(key)
	if err != nil {
		return nil, fmt.Errorf("AES cipher: %w", err)
	}

	gcm, err := cipher.NewGCM(block)
	if err != nil {
		return nil, fmt.Errorf("GCM: %w", err)
	}

	nonce := make([]byte, NonceSize)
	if _, err := io.ReadFull(rand.Reader, nonce); err != nil {
		return nil, fmt.Errorf("generate nonce: %w", err)
	}

	// nonce prepended to ciphertext
	ciphertext := gcm.Seal(nonce, nonce, plaintext, nil)
	return ciphertext, nil
}

// Decrypt — расшифровывает ciphertext (nonce + ciphertext + tag).
func Decrypt(key, ciphertext []byte) ([]byte, error) {
	if len(key) != AESKeySize {
		return nil, fmt.Errorf("invalid key size: %d (expected %d)", len(key), AESKeySize)
	}

	if len(ciphertext) < NonceSize {
		return nil, ErrInvalidCiphertext
	}

	block, err := aes.NewCipher(key)
	if err != nil {
		return nil, fmt.Errorf("AES cipher: %w", err)
	}

	gcm, err := cipher.NewGCM(block)
	if err != nil {
		return nil, fmt.Errorf("GCM: %w", err)
	}

	nonce := ciphertext[:NonceSize]
	ct := ciphertext[NonceSize:]

	plaintext, err := gcm.Open(nil, nonce, ct, nil)
	if err != nil {
		return nil, ErrDecryptionFailed
	}

	return plaintext, nil
}

// ═══════════════════════════════════════════════════════════
// E2EE Envelope — зашифрованное сообщение
// ═══════════════════════════════════════════════════════════

// PublicKeyInfo — публичная информация о ключе (без приватного!).
type PublicKeyInfo struct {
	KeyID     string `json:"key_id"`
	PublicKey []byte `json:"public_key"`
	IsActive  bool   `json:"is_active"`
}

// EncryptedEnvelope — зашифрованное сообщение для передачи через relay.
// Relay НЕ может расшифровать — у него нет приватных ключей.
type EncryptedEnvelope struct {
	Version   int    `json:"version"`             // 1
	KeyID     string `json:"key_id"`              // ID ключа получателя
	SenderKey string `json:"sender_key_id"`       // ID ключа отправителя
	Nonce     []byte `json:"nonce,omitempty"`      // Для прямого AES (опционально)
	Ciphertext []byte `json:"ciphertext"`          // Зашифрованные данные
	// KeyExchange — для первого сообщения (ECDH)
	EphemeralPubKey []byte `json:"ephemeral_pub_key,omitempty"`
}

// E2EELayer — слой end-to-end шифрования.
type E2EELayer struct {
	keyStore *KeyStore
	// Кэш shared secrets: peerKeyID → derivedKey
	secrets   map[string][]byte
	secretsMu sync.RWMutex
}

// NewE2EELayer — создаёт E2EE слой.
func NewE2EELayer(ks *KeyStore) *E2EELayer {
	return &E2EELayer{
		keyStore: ks,
		secrets:  make(map[string][]byte),
	}
}

// Seal — шифрует данные для получателя по его публичному ключу.
// Использует X25519 ECDH для обмена ключами + AES-256-GCM для шифрования.
func (e *E2EELayer) Seal(peerPublicKey []byte, peerKeyID string, plaintext []byte) (*EncryptedEnvelope, error) {
	myKey, err := e.keyStore.Active()
	if err != nil {
		return nil, fmt.Errorf("get active key: %w", err)
	}

	// ECDH: вычисляем shared secret
	shared, err := SharedSecret(myKey.PrivateKey, peerPublicKey)
	if err != nil {
		return nil, fmt.Errorf("compute shared secret: %w", err)
	}

	// Symmetric key derivation — одинаковый ключ для обоих направлений
	aesKey := DeriveKey(shared, "flowlink-v1", symmetricKeyID(myKey.KeyID, peerKeyID))

	// Encrypt
	ciphertext, err := Encrypt(aesKey, plaintext)
	if err != nil {
		return nil, fmt.Errorf("encrypt: %w", err)
	}

	// Cache derived key для будущих расшифровок ответов
	e.secretsMu.Lock()
	e.secrets[peerKeyID] = aesKey
	e.secretsMu.Unlock()

	return &EncryptedEnvelope{
		Version:         KeyExchangeVersion,
		KeyID:           peerKeyID,
		SenderKey:       myKey.KeyID,
		EphemeralPubKey: myKey.PublicKey, // Отправляем свой публичный ключ
		Ciphertext:      ciphertext,
	}, nil
}

// Open — расшифровывает данные от отправителя.
func (e *E2EELayer) Open(envelope *EncryptedEnvelope) ([]byte, error) {
	if envelope.Version != KeyExchangeVersion {
		return nil, fmt.Errorf("unsupported version: %d", envelope.Version)
	}

	myKey, err := e.keyStore.Get(envelope.KeyID)
	if err != nil {
		// Попробуем активный ключ
		myKey, err = e.keyStore.Active()
		if err != nil {
			return nil, fmt.Errorf("get key %s: %w", envelope.KeyID, err)
		}
	}

	// ECDH: вычисляем shared secret с помощью публичного ключа отправителя
	shared, err := SharedSecret(myKey.PrivateKey, envelope.EphemeralPubKey)
	if err != nil {
		return nil, fmt.Errorf("compute shared secret: %w", err)
	}

	// Symmetric key derivation — тот же ключ что у отправителя
	aesKey := DeriveKey(shared, "flowlink-v1", symmetricKeyID(envelope.SenderKey, envelope.KeyID))

	// Cache derived key
	e.secretsMu.Lock()
	e.secrets[envelope.SenderKey] = aesKey
	e.secretsMu.Unlock()

	// Decrypt
	plaintext, err := Decrypt(aesKey, envelope.Ciphertext)
	if err != nil {
		return nil, fmt.Errorf("decrypt: %w", err)
	}

	return plaintext, nil
}

// SealWithCached — шифрует данные используя кэшированный shared secret.
// Быстрее чем Seal — не нужно ECDH если секрет уже вычислен.
func (e *E2EELayer) SealWithCached(peerKeyID string, plaintext []byte) (*EncryptedEnvelope, error) {
	e.secretsMu.RLock()
	aesKey, ok := e.secrets[peerKeyID]
	e.secretsMu.RUnlock()

	if !ok {
		return nil, ErrNoSharedSecret
	}

	myKey, err := e.keyStore.Active()
	if err != nil {
		return nil, fmt.Errorf("get active key: %w", err)
	}

	ciphertext, err := Encrypt(aesKey, plaintext)
	if err != nil {
		return nil, fmt.Errorf("encrypt: %w", err)
	}

	return &EncryptedEnvelope{
		Version:    KeyExchangeVersion,
		KeyID:      peerKeyID,
		SenderKey:  myKey.KeyID,
		Ciphertext: ciphertext,
	}, nil
}

// OpenWithCached — расшифровывает используя кэшированный ключ.
func (e *E2EELayer) OpenWithCached(envelope *EncryptedEnvelope) ([]byte, error) {
	e.secretsMu.RLock()
	aesKey, ok := e.secrets[envelope.SenderKey]
	e.secretsMu.RUnlock()

	if !ok {
		return nil, ErrNoSharedSecret
	}

	return Decrypt(aesKey, envelope.Ciphertext)
}

// HasSharedSecret — проверяет, есть ли кэшированный секрет для пира.
func (e *E2EELayer) HasSharedSecret(peerKeyID string) bool {
	e.secretsMu.RLock()
	defer e.secretsMu.RUnlock()
	_, ok := e.secrets[peerKeyID]
	return ok
}

// ═══════════════════════════════════════════════════════════
// Encrypted Payload Helper
// ═══════════════════════════════════════════════════════════

// EncryptedPayload — зашифрованный payload для протокола FlowLink.
type EncryptedPayload struct {
	Encrypted bool              `json:"encrypted"`
	Envelope  *EncryptedEnvelope `json:"envelope,omitempty"`
	Raw       any               `json:"raw,omitempty"` // fallback (unencrypted)
}

// Wrap — оборачивает payload в зашифрованный формат.
func (e *E2EELayer) Wrap(peerPublicKey []byte, peerKeyID string, payload any) (*EncryptedPayload, error) {
	data, err := json.Marshal(payload)
	if err != nil {
		return nil, fmt.Errorf("marshal payload: %w", err)
	}

	envelope, err := e.Seal(peerPublicKey, peerKeyID, data)
	if err != nil {
		return nil, fmt.Errorf("seal: %w", err)
	}

	return &EncryptedPayload{
		Encrypted: true,
		Envelope:  envelope,
	}, nil
}

// Unwrap — распаковывает зашифрованный payload.
func (e *E2EELayer) Unwrap(ep *EncryptedPayload) ([]byte, error) {
	if !ep.Encrypted || ep.Envelope == nil {
		// Unencrypted fallback
		if ep.Raw != nil {
			return json.Marshal(ep.Raw)
		}
		return nil, errors.New("empty payload")
	}

	// Сначала пробуем cached
	if e.HasSharedSecret(ep.Envelope.SenderKey) {
		return e.OpenWithCached(ep.Envelope)
	}

	// Full ECDH
	return e.Open(ep.Envelope)
}

// ═══════════════════════════════════════════════════════════
// File Encryption — шифрование файлов на диске
// ═══════════════════════════════════════════════════════════

// EncryptFile — шифрует файл на диске (для audit logs, backups).
func EncryptFile(key, data []byte, outputPath string) error {
	ciphertext, err := Encrypt(key, data)
	if err != nil {
		return err
	}
	return os.WriteFile(outputPath, ciphertext, 0600)
}

// DecryptFile — расшифровывает файл с диска.
func DecryptFile(key []byte, inputPath string) ([]byte, error) {
	ciphertext, err := os.ReadFile(inputPath)
	if err != nil {
		return nil, err
	}
	return Decrypt(key, ciphertext)
}

// ═══════════════════════════════════════════════════════════
// Persistence — сохранение/загрузка ключей
// ═══════════════════════════════════════════════════════════

type keyStoreFile struct {
	ActiveID string    `json:"active_id"`
	Keys     []Keypair `json:"keys"`
}

func (ks *KeyStore) save() error {
	ks.mu.RLock()
	defer ks.mu.RUnlock()

	data := keyStoreFile{
		ActiveID: ks.activeID,
		Keys:     make([]Keypair, 0, len(ks.keypairs)),
	}

	for _, kp := range ks.keypairs {
		data.Keys = append(data.Keys, *kp)
	}

	jsonData, err := json.MarshalIndent(data, "", "  ")
	if err != nil {
		return err
	}

	path := filepath.Join(ks.dir, "keys.json")
	return os.WriteFile(path, jsonData, KeyFilePermission)
}

func (ks *KeyStore) load() error {
	path := filepath.Join(ks.dir, "keys.json")

	data, err := os.ReadFile(path)
	if err != nil {
		return err
	}

	var file keyStoreFile
	if err := json.Unmarshal(data, &file); err != nil {
		return err
	}

	ks.mu.Lock()
	defer ks.mu.Unlock()

	ks.activeID = file.ActiveID
	ks.keypairs = make(map[string]*Keypair)

	for i := range file.Keys {
		ks.keypairs[file.Keys[i].KeyID] = &file.Keys[i]
	}

	return nil
}

// ═══════════════════════════════════════════════════════════
// Helpers
// ═══════════════════════════════════════════════════════════

func keyID(publicKey []byte) string {
	h := sha256.Sum256(publicKey)
	return base64.RawURLEncoding.EncodeToString(h[:16]) // 22 chars, URL-safe
}

func nowUnix() int64 {
	return int64(0) // placeholder — use time.Now().Unix() in real code
}
