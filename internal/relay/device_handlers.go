// Package relay — HTTP API handlers for device management (E2EE).
package relay

import (
	"encoding/json"
	"net/http"
	"strings"
	"time"

	"github.com/braincreator/flowlink/internal/protocol"
)

// handleDevicesList — GET /api/v1/devices — список устройств с E2EE статусом.
func (r *Relay) handleDevicesList(w http.ResponseWriter, req *http.Request) {
	if r.deviceRegistry == nil {
		writeDeviceError(w, http.StatusServiceUnavailable, "devices not configured")
		return
	}
	ownerID := req.URL.Query().Get("owner_id")
	list := r.deviceRegistry.List(ownerID)
	w.Header().Set("Content-Type", "application/json")
	json.NewEncoder(w).Encode(list)
}

// handleDeviceByID — GET/DELETE /api/v1/devices/{id}
func (r *Relay) handleDeviceByID(w http.ResponseWriter, req *http.Request) {
	if r.deviceRegistry == nil {
		writeDeviceError(w, http.StatusServiceUnavailable, "devices not configured")
		return
	}
	id := strings.TrimPrefix(req.URL.Path, "/api/v1/devices/")
	if id == "" {
		writeDeviceError(w, http.StatusBadRequest, "device id required")
		return
	}

	switch req.Method {
	case http.MethodGet:
		device, err := r.deviceRegistry.Get(id)
		if err != nil {
			writeDeviceError(w, http.StatusNotFound, "device not found")
			return
		}
		w.Header().Set("Content-Type", "application/json")
		json.NewEncoder(w).Encode(device)

	case http.MethodDelete:
		if err := r.deviceRegistry.Revoke(id); err != nil {
			writeDeviceError(w, http.StatusInternalServerError, "revoke failed")
			return
		}
		w.WriteHeader(http.StatusNoContent)

	default:
		writeDeviceError(w, http.StatusMethodNotAllowed, "method not allowed")
	}
}

// handlePairingAction — POST /api/v1/pairing/{action}/{code}
// Actions: approve, reject
func (r *Relay) handlePairingAction(w http.ResponseWriter, req *http.Request) {
	if r.pairingManager == nil {
		writeDeviceError(w, http.StatusServiceUnavailable, "pairing not configured")
		return
	}
	if req.Method != http.MethodPost {
		writeDeviceError(w, http.StatusMethodNotAllowed, "use POST")
		return
	}

	// Parse: /api/v1/pairing/{action}/{code}
	path := strings.TrimPrefix(req.URL.Path, "/api/v1/pairing/")
	parts := strings.Split(path, "/")
	if len(parts) < 2 {
		writeDeviceError(w, http.StatusBadRequest, "format: /api/v1/pairing/{action}/{code}")
		return
	}
	action := parts[0]
	code := parts[1]

	ownerID := "default" // TODO: get from auth context

	switch action {
	case "approve":
		device, err := r.pairingManager.ApprovePairing(code, ownerID)
		if err != nil {
			writeDeviceError(w, http.StatusBadRequest, err.Error())
			return
		}
		r.logEvent("device_approved", "device_id", device.ID, "code", code)
		w.Header().Set("Content-Type", "application/json")
		json.NewEncoder(w).Encode(device)

	case "reject":
		if err := r.pairingManager.RejectPairing(code); err != nil {
			writeDeviceError(w, http.StatusNotFound, "pairing request not found")
			return
		}
		r.logEvent("device_rejected", "code", code)
		w.WriteHeader(http.StatusNoContent)

	default:
		writeDeviceError(w, http.StatusBadRequest, "invalid action: use approve or reject")
	}
}

// handleKeysList — GET /api/v1/keys — список публичных ключей E2EE.
func (r *Relay) handleKeysList(w http.ResponseWriter, req *http.Request) {
	if r.keystore == nil {
		writeDeviceError(w, http.StatusServiceUnavailable, "keys not configured")
		return
	}
	keys := r.keystore.PublicKeys()
	w.Header().Set("Content-Type", "application/json")
	json.NewEncoder(w).Encode(keys)
}

// handleKeysRotate — POST /api/v1/keys/rotate — ротация ключей E2EE.
func (r *Relay) handleKeysRotate(w http.ResponseWriter, req *http.Request) {
	if r.keystore == nil {
		writeDeviceError(w, http.StatusServiceUnavailable, "keys not configured")
		return
	}
	if req.Method != http.MethodPost {
		writeDeviceError(w, http.StatusMethodNotAllowed, "use POST")
		return
	}

	kp, err := r.keystore.Rotate()
	if err != nil {
		writeDeviceError(w, http.StatusInternalServerError, "key rotation failed")
		return
	}

	r.logEvent("keys_rotated", "key_id", kp.KeyID)
	w.Header().Set("Content-Type", "application/json")
	json.NewEncoder(w).Encode(map[string]any{
		"key_id":     kp.KeyID,
		"public_key": kp.PublicKey,
		"created_at": kp.CreatedAt,
		"active":     true,
	})
}

// writeDeviceError — helper for error responses (wrapper for relay's writeErrorCustom).
func writeDeviceError(w http.ResponseWriter, code int, message string) {
	writeErrorCustom(w, code, protocol.CodeInternalError, message)
}

// logEvent — helper for audit logging in device handlers.
func (r *Relay) logEvent(action string, fields ...string) {
	if r.audit == nil {
		return
	}
	entry := AuditEntry{
		Action:    action,
		Timestamp: time.Now(),
		Result:    "success",
	}
	// Parse key-value pairs
	for i := 0; i < len(fields)-1; i += 2 {
		switch fields[i] {
		case "device_id":
			entry.AgentID = fields[i+1]
		case "code":
			entry.Command = fields[i+1]
		case "key_id":
			entry.BackupID = fields[i+1]
		}
	}
	r.audit.Log(entry)
}
