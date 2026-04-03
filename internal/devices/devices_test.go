package devices

import (
	"os"
	"path/filepath"
	"testing"
	"time"

	"github.com/braincreator/flowlink/internal/crypto"
)

func tempDir(t *testing.T) string {
	t.Helper()
	dir := t.TempDir()
	return dir
}

// ═══════════════════════════════════════════════════════════
// DeviceRegistry Tests
// ═══════════════════════════════════════════════════════════

func TestDeviceRegistry_CreateAndLoad(t *testing.T) {
	dir := tempDir(t)

	// Create
	r1, err := NewDeviceRegistry(dir)
	if err != nil {
		t.Fatal(err)
	}

	device := &Device{
		ID:        "test-device-1",
		Name:      "my-server",
		PublicKey: []byte("test-public-key"),
		KeyID:     "key-123",
		OwnerID:   "owner-1",
		OS:        "linux",
		E2EE:      true,
	}

	if err := r1.Register(device); err != nil {
		t.Fatal(err)
	}

	if device.Status != StatusPending {
		t.Fatalf("expected pending, got %s", device.Status)
	}

	// Load from disk
	r2, err := NewDeviceRegistry(dir)
	if err != nil {
		t.Fatal(err)
	}

	d, err := r2.Get("test-device-1")
	if err != nil {
		t.Fatal(err)
	}
	if d.Name != "my-server" {
		t.Fatalf("expected my-server, got %s", d.Name)
	}
	if d.Status != StatusPending {
		t.Fatalf("expected pending, got %s", d.Status)
	}
	if !d.E2EE {
		t.Fatal("expected E2EE true")
	}
}

func TestDeviceRegistry_ApproveAndRevoke(t *testing.T) {
	dir := tempDir(t)
	r, err := NewDeviceRegistry(dir)
	if err != nil {
		t.Fatal(err)
	}

	device := &Device{
		ID:        "dev-1",
		Name:      "server-1",
		PublicKey: []byte("pubkey"),
		KeyID:     "k1",
		OwnerID:   "owner-1",
	}

	if err := r.Register(device); err != nil {
		t.Fatal(err)
	}

	// Approve
	if err := r.Approve("dev-1"); err != nil {
		t.Fatal(err)
	}

	if !r.IsApproved("dev-1") {
		t.Fatal("expected approved")
	}

	d, _ := r.Get("dev-1")
	if d.Status != StatusApproved {
		t.Fatalf("expected approved, got %s", d.Status)
	}

	// Revoke
	if err := r.Revoke("dev-1"); err != nil {
		t.Fatal(err)
	}

	if r.IsApproved("dev-1") {
		t.Fatal("should not be approved after revoke")
	}

	d, _ = r.Get("dev-1")
	if d.Status != StatusRevoked {
		t.Fatalf("expected revoked, got %s", d.Status)
	}

	// Not found
	if err := r.Approve("nonexistent"); err != ErrDeviceNotFound {
		t.Fatalf("expected ErrDeviceNotFound, got %v", err)
	}
}

func TestDeviceRegistry_List(t *testing.T) {
	dir := tempDir(t)
	r, err := NewDeviceRegistry(dir)
	if err != nil {
		t.Fatal(err)
	}

	r.Register(&Device{ID: "d1", Name: "s1", OwnerID: "owner-1"})
	r.Register(&Device{ID: "d2", Name: "s2", OwnerID: "owner-2"})
	r.Register(&Device{ID: "d3", Name: "s3", OwnerID: "owner-1"})

	list := r.List("owner-1")
	if len(list) != 2 {
		t.Fatalf("expected 2 devices, got %d", len(list))
	}

	all := r.List("")
	if len(all) != 3 {
		t.Fatalf("expected 3 devices, got %d", len(all))
	}
}

func TestDeviceRegistry_Duplicate(t *testing.T) {
	dir := tempDir(t)
	r, err := NewDeviceRegistry(dir)
	if err != nil {
		t.Fatal(err)
	}

	if err := r.Register(&Device{ID: "dup", Name: "s1", OwnerID: "o1"}); err != nil {
		t.Fatal(err)
	}

	if err := r.Register(&Device{ID: "dup", Name: "s2", OwnerID: "o2"}); err != ErrDeviceAlreadyExists {
		t.Fatalf("expected ErrDeviceAlreadyExists, got %v", err)
	}
}

// ═══════════════════════════════════════════════════════════
// PairingManager Tests
// ═══════════════════════════════════════════════════════════

func setupPairing(t *testing.T) (*PairingManager, *DeviceRegistry) {
	t.Helper()
	dir := t.TempDir()

	keyDir := filepath.Join(dir, "keys")
	os.MkdirAll(keyDir, 0700)

	ks, err := crypto.NewKeyStore(keyDir)
	if err != nil {
		t.Fatal(err)
	}

	reg, err := NewDeviceRegistry(dir)
	if err != nil {
		t.Fatal(err)
	}

	e2ee := crypto.NewE2EELayer(ks)
	pm := NewPairingManager(reg, ks, e2ee)
	return pm, reg
}

func TestPairingManager_InitiateAndApprove(t *testing.T) {
	pm, reg := setupPairing(t)

	req, err := pm.InitiatePairing("test-server", []byte("pub-key-data"), "key-abc")
	if err != nil {
		t.Fatal(err)
	}

	if len(req.Code) != pairingCodeLength {
		t.Fatalf("expected code length %d, got %d", pairingCodeLength, len(req.Code))
	}

	if req.Device.Name != "test-server" {
		t.Fatalf("expected test-server, got %s", req.Device.Name)
	}

	// Approve
	device, err := pm.ApprovePairing(req.Code, "owner-42")
	if err != nil {
		t.Fatal(err)
	}

	if device.OwnerID != "owner-42" {
		t.Fatalf("expected owner-42, got %s", device.OwnerID)
	}

	if !reg.IsApproved(device.ID) {
		t.Fatal("device should be approved in registry")
	}

	// Code should be consumed
	if _, ok := pm.GetPending(req.Code); ok {
		t.Fatal("code should be consumed after approval")
	}
}

func TestPairingManager_ExpiredCode(t *testing.T) {
	pm, _ := setupPairing(t)

	req, err := pm.InitiatePairing("server", []byte("key"), "kid")
	if err != nil {
		t.Fatal(err)
	}

	// Manually expire
	pm.mu.Lock()
	req.ExpiresAt = time.Now().Add(-time.Second)
	pm.mu.Unlock()

	_, err = pm.ApprovePairing(req.Code, "owner")
	if err != ErrPairingExpired {
		t.Fatalf("expected ErrPairingExpired, got %v", err)
	}
}

func TestPairingManager_Reject(t *testing.T) {
	pm, reg := setupPairing(t)

	req, err := pm.InitiatePairing("server", []byte("key"), "kid")
	if err != nil {
		t.Fatal(err)
	}

	if err := pm.RejectPairing(req.Code); err != nil {
		t.Fatal(err)
	}

	// Code consumed
	if _, ok := pm.GetPending(req.Code); ok {
		t.Fatal("code should be consumed after rejection")
	}

	// Device NOT in registry
	if _, err := reg.Get(req.Device.ID); err != ErrDeviceNotFound {
		t.Fatalf("expected ErrDeviceNotFound, got %v", err)
	}

	// Reject nonexistent
	if err := pm.RejectPairing("000000"); err != ErrPairingNotFound {
		t.Fatalf("expected ErrPairingNotFound, got %v", err)
	}
}

func TestPairingManager_VerifyDevice(t *testing.T) {
	pm, _ := setupPairing(t)

	pubKey := []byte("my-public-key-32bytes!!")
	req, _ := pm.InitiatePairing("server", pubKey, "kid")
	pm.ApprovePairing(req.Code, "owner")

	if !pm.VerifyDevice(req.Device.ID, pubKey) {
		t.Fatal("verify should succeed with correct key")
	}

	if pm.VerifyDevice(req.Device.ID, []byte("wrong-key")) {
		t.Fatal("verify should fail with wrong key")
	}

	if pm.VerifyDevice("nonexistent", pubKey) {
		t.Fatal("verify should fail for nonexistent device")
	}
}
