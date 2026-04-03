package crypto

import (
	"crypto/ecdh"
	"crypto/rand"
	"encoding/json"
	"testing"
)

func TestKeypairGeneration(t *testing.T) {
	dir := t.TempDir()
	ks, err := NewKeyStore(dir)
	if err != nil {
		t.Fatalf("create keystore: %v", err)
	}

	kp, err := ks.Active()
	if err != nil {
		t.Fatalf("get active key: %v", err)
	}

	if len(kp.PrivateKey) != 32 {
		t.Errorf("private key should be 32 bytes, got %d", len(kp.PrivateKey))
	}
	if len(kp.PublicKey) != 32 {
		t.Errorf("public key should be 32 bytes, got %d", len(kp.PublicKey))
	}
	if kp.KeyID == "" {
		t.Error("key ID should not be empty")
	}
}

func TestKeyStorePersistence(t *testing.T) {
	dir := t.TempDir()

	// Create and generate
	ks1, err := NewKeyStore(dir)
	if err != nil {
		t.Fatalf("create keystore: %v", err)
	}
	kp1, _ := ks1.Active()

	// Reload
	ks2, err := NewKeyStore(dir)
	if err != nil {
		t.Fatalf("reload keystore: %v", err)
	}
	kp2, _ := ks2.Active()

	if kp1.KeyID != kp2.KeyID {
		t.Errorf("key ID mismatch after reload: %s != %s", kp1.KeyID, kp2.KeyID)
	}
	if string(kp1.PublicKey) != string(kp2.PublicKey) {
		t.Error("public key mismatch after reload")
	}
}

func TestKeyRotation(t *testing.T) {
	dir := t.TempDir()
	ks, err := NewKeyStore(dir)
	if err != nil {
		t.Fatalf("create keystore: %v", err)
	}

	oldKey, _ := ks.Active()
	oldID := oldKey.KeyID

	newKey, err := ks.Rotate()
	if err != nil {
		t.Fatalf("rotate: %v", err)
	}

	if newKey.KeyID == oldID {
		t.Error("new key should have different ID")
	}

	keys := ks.List()
	if len(keys) != 2 {
		t.Errorf("expected 2 keys after rotation, got %d", len(keys))
	}
}

func TestECDHSharedSecret(t *testing.T) {
	curve := ecdh.X25519()

	priv1, _ := curve.GenerateKey(rand.Reader)
	priv2, _ := curve.GenerateKey(rand.Reader)

	pub1 := priv1.PublicKey()
	pub2 := priv2.PublicKey()

	secret1, err := SharedSecret(priv1.Bytes(), pub2.Bytes())
	if err != nil {
		t.Fatalf("ECDH 1: %v", err)
	}

	secret2, err := SharedSecret(priv2.Bytes(), pub1.Bytes())
	if err != nil {
		t.Fatalf("ECDH 2: %v", err)
	}

	if string(secret1) != string(secret2) {
		t.Error("shared secrets should be identical")
	}
}

func TestAESEncryptDecrypt(t *testing.T) {
	key := make([]byte, 32)
	for i := range key {
		key[i] = byte(i)
	}

	tests := []struct {
		name string
		data []byte
	}{
		{"empty", []byte{}},
		{"short", []byte("hello")},
		{"json", []byte(`{"command": "ls -la", "timeout": 30}`)},
		{"binary", make([]byte, 1024)},
	}

	for _, tt := range tests {
		t.Run(tt.name, func(t *testing.T) {
			ciphertext, err := Encrypt(key, tt.data)
			if err != nil {
				t.Fatalf("encrypt: %v", err)
			}

			plaintext, err := Decrypt(key, ciphertext)
			if err != nil {
				t.Fatalf("decrypt: %v", err)
			}

			if string(plaintext) != string(tt.data) {
				t.Errorf("mismatch: got %q, want %q", plaintext, tt.data)
			}
		})
	}
}

func TestAESWrongKey(t *testing.T) {
	key1 := make([]byte, 32)
	key2 := make([]byte, 32)
	key2[0] = 0xFF // Different key

	ciphertext, err := Encrypt(key1, []byte("secret"))
	if err != nil {
		t.Fatalf("encrypt: %v", err)
	}

	_, err = Decrypt(key2, ciphertext)
	if err == nil {
		t.Error("expected decryption failure with wrong key")
	}
}

func TestE2EELayer(t *testing.T) {
	// Create two keystores (simulating client and agent)
	dir1 := t.TempDir()
	dir2 := t.TempDir()

	ks1, _ := NewKeyStore(dir1) // Client
	ks2, _ := NewKeyStore(dir2) // Agent

	e2ee1 := NewE2EELayer(ks1) // Client E2EE
	e2ee2 := NewE2EELayer(ks2) // Agent E2EE

	// Get each other's public keys
	pub1 := ks1.PublicKeys()[0]
	pub2 := ks2.PublicKeys()[0]

	// Client → Agent
	message := map[string]any{
		"command": "ls -la",
		"timeout": 30,
	}

	// Client seals
	wrapped, err := e2ee1.Wrap(pub2.PublicKey, pub2.KeyID, message)
	if err != nil {
		t.Fatalf("wrap: %v", err)
	}
	if !wrapped.Encrypted {
		t.Error("payload should be encrypted")
	}

	// Agent opens
	raw, err := e2ee2.Unwrap(wrapped)
	if err != nil {
		t.Fatalf("unwrap: %v", err)
	}

	var decoded map[string]any
	if err := json.Unmarshal(raw, &decoded); err != nil {
		t.Fatalf("unmarshal: %v", err)
	}

	if decoded["command"] != "ls -la" {
		t.Errorf("command mismatch: got %v", decoded["command"])
	}

	// Agent → Client (reply)
	reply := map[string]any{
		"exit_code": 0,
		"output":    "file1.txt\nfile2.txt",
	}

	replyWrapped, err := e2ee2.Wrap(pub1.PublicKey, pub1.KeyID, reply)
	if err != nil {
		t.Fatalf("wrap reply: %v", err)
	}

	replyRaw, err := e2ee1.Unwrap(replyWrapped)
	if err != nil {
		t.Fatalf("unwrap reply: %v", err)
	}

	var decodedReply map[string]any
	if err := json.Unmarshal(replyRaw, &decodedReply); err != nil {
		t.Fatalf("unmarshal reply: %v", err)
	}

	if decodedReply["output"] != "file1.txt\nfile2.txt" {
		t.Errorf("output mismatch: got %v", decodedReply["output"])
	}
}

func TestE2EE_CachedSealOpen(t *testing.T) {
	dir1 := t.TempDir()
	dir2 := t.TempDir()

	ks1, _ := NewKeyStore(dir1)
	ks2, _ := NewKeyStore(dir2)

	e2ee1 := NewE2EELayer(ks1)

	pub2 := ks2.PublicKeys()[0]

	// First message establishes shared secret
	_, err := e2ee1.Wrap(pub2.PublicKey, pub2.KeyID, "init")
	if err != nil {
		t.Fatalf("init: %v", err)
	}

	// Second message should use cached secret
	envelope, err := e2ee1.SealWithCached(pub2.KeyID, []byte("cached message"))
	if err != nil {
		t.Fatalf("seal cached: %v", err)
	}

	// Verify no ephemeral key in cached mode
	if envelope.EphemeralPubKey != nil {
		t.Error("cached seal should not include ephemeral public key")
	}

	// Agent needs to have received the first message to have the cache
	// In real code this happens through the relay
}

func TestE2EE_RelayCannotDecrypt(t *testing.T) {
	dir1 := t.TempDir()
	dir2 := t.TempDir()
	dirRelay := t.TempDir()

	ks1, _ := NewKeyStore(dir1)     // Client
	ks2, _ := NewKeyStore(dir2)     // Agent
	ksRelay, _ := NewKeyStore(dirRelay) // Relay (has its OWN keys, not client's)

	e2ee1 := NewE2EELayer(ks1)
	e2eeRelay := NewE2EELayer(ksRelay)

	pub2 := ks2.PublicKeys()[0]

	// Client encrypts for agent
	wrapped, _ := e2ee1.Wrap(pub2.PublicKey, pub2.KeyID, map[string]string{
		"command": "cat /etc/shadow",
	})

	// Relay tries to decrypt — should FAIL
	_, err := e2eeRelay.Unwrap(wrapped)
	if err == nil {
		t.Error("relay should NOT be able to decrypt client→agent messages")
	}
}

func TestEncryptDecryptFile(t *testing.T) {
	key := make([]byte, 32)
	data := []byte(`{"timestamp": 123, "command": "rm -rf /", "exit_code": 0}`)

	path := t.TempDir() + "/audit.json.enc"

	err := EncryptFile(key, data, path)
	if err != nil {
		t.Fatalf("encrypt file: %v", err)
	}

	decrypted, err := DecryptFile(key, path)
	if err != nil {
		t.Fatalf("decrypt file: %v", err)
	}

	if string(decrypted) != string(data) {
		t.Error("file content mismatch")
	}
}
