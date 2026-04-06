// Device management — pairing, listing, and push notification stubs.

use axum::{
    extract::{Path, Query, State},
    response::IntoResponse,
    Json,
};
use dashmap::DashMap;
use log::info;
use rand::Rng;
use serde::{Deserialize, Serialize};
use std::sync::Arc;

use crate::server::AppState;

// ═══════════════════════════════════════════════
// Types
// ═══════════════════════════════════════════════

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Device {
    pub id: String,
    pub user_id: String,
    pub name: String,
    pub device_type: String, // "ios", "android", "desktop"
    #[serde(skip_serializing_if = "Option::is_none")]
    pub push_token: Option<String>,
    pub paired_at: i64,
    pub last_seen: i64,
    pub active: bool,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct PairingCode {
    pub code: String,
    pub user_id: String,
    pub device_id: String,
    pub expires_at: i64,
}

#[derive(Debug, Clone)]
pub struct DeviceManager {
    devices: Arc<DashMap<String, Device>>,
    pairing_codes: Arc<DashMap<String, PairingCode>>,
}

impl DeviceManager {
    pub fn new() -> Self {
        Self {
            devices: Arc::new(DashMap::new()),
            pairing_codes: Arc::new(DashMap::new()),
        }
    }

    /// Generate a 6-digit pairing code with 5min expiry.
    pub fn generate_pairing_code(&self, user_id: &str) -> String {
        let code = format!("{:06}", rand::thread_rng().gen_range(100000..=999999));
        let device_id = uuid::Uuid::new_v4().to_string();
        let expires_at = chrono::Utc::now().timestamp() + 300; // 5 minutes

        self.pairing_codes.insert(code.clone(), PairingCode {
            code: code.clone(),
            user_id: user_id.to_string(),
            device_id,
            expires_at,
        });

        info!("Pairing code generated for user {user_id}");
        code
    }

    /// Confirm pairing — validates code, registers device.
    pub fn confirm_pairing(
        &self,
        code: &str,
        name: &str,
        device_type: &str,
        push_token: Option<String>,
    ) -> Result<Device, String> {
        let entry = self.pairing_codes.remove(code)
            .ok_or_else(|| "pairing code not found".to_string())?;

        let pc = entry.1;

        if chrono::Utc::now().timestamp() > pc.expires_at {
            return Err("pairing code expired".to_string());
        }

        let device = Device {
            id: pc.device_id,
            user_id: pc.user_id,
            name: name.to_string(),
            device_type: device_type.to_string(),
            push_token,
            paired_at: chrono::Utc::now().timestamp(),
            last_seen: chrono::Utc::now().timestamp(),
            active: true,
        };

        self.devices.insert(device.id.clone(), device.clone());
        info!("Device paired: {} ({})", device.name, device.id);
        Ok(device)
    }

    pub fn list_devices(&self, user_id: &str) -> Vec<Device> {
        self.devices.iter()
            .filter(|e| e.value().user_id == user_id)
            .map(|e| e.value().clone())
            .collect()
    }

    pub fn remove_device(&self, device_id: &str) -> Result<(), String> {
        if self.devices.remove(device_id).is_none() {
            return Err("device not found".to_string());
        }
        info!("Device removed: {device_id}");
        Ok(())
    }

    pub fn send_push(&self, device_id: &str, notification: &str) -> Result<(), String> {
        let device = self.devices.get(device_id)
            .ok_or_else(|| "device not found".to_string())?;

        if !device.active {
            return Err("device not active".to_string());
        }

        // Stub: in production, integrate with APNs/FCM
        info!("Push notification to {} ({}): {}", device.name, device_id, notification);
        Ok(())
    }
}

// ═══════════════════════════════════════════════
// Request/Response types
// ═══════════════════════════════════════════════

#[derive(Deserialize)]
pub struct PairRequest {
    user_id: String,
}

#[derive(Serialize)]
struct PairResponse {
    code: String,
    expires_in: i64,
}

#[derive(Deserialize)]
pub struct ConfirmRequest {
    code: String,
    name: String,
    device_type: String,
    #[serde(default)]
    push_token: Option<String>,
}

#[derive(Deserialize)]
pub struct DevicesQuery {
    user_id: String,
}

// ═══════════════════════════════════════════════
// Route Handlers
// ═══════════════════════════════════════════════

pub async fn pair_device(
    State(state): State<AppState>,
    Json(body): Json<PairRequest>,
) -> impl IntoResponse {
    let code = state.device_manager.generate_pairing_code(&body.user_id);
    Json(PairResponse { code, expires_in: 300 })
}

pub async fn confirm_pairing(
    State(state): State<AppState>,
    Json(body): Json<ConfirmRequest>,
) -> impl IntoResponse {
    match state.device_manager.confirm_pairing(&body.code, &body.name, &body.device_type, body.push_token) {
        Ok(device) => Json(device).into_response(),
        Err(e) => (
            axum::http::StatusCode::BAD_REQUEST,
            Json(serde_json::json!({ "error": e })),
        ).into_response(),
    }
}

pub async fn list_devices(
    State(state): State<AppState>,
    Query(query): Query<DevicesQuery>,
) -> Json<Vec<Device>> {
    Json(state.device_manager.list_devices(&query.user_id))
}

pub async fn remove_device(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> impl IntoResponse {
    match state.device_manager.remove_device(&id) {
        Ok(()) => axum::http::StatusCode::NO_CONTENT.into_response(),
        Err(e) => (
            axum::http::StatusCode::NOT_FOUND,
            Json(serde_json::json!({ "error": e })),
        ).into_response(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_generate_pairing_code() {
        let dm = DeviceManager::new();
        let code = dm.generate_pairing_code("user-1");
        assert_eq!(code.len(), 6);
        assert!(code.chars().all(|c| c.is_ascii_digit()));
    }

    #[test]
    fn test_confirm_pairing_valid() {
        let dm = DeviceManager::new();
        let code = dm.generate_pairing_code("user-1");
        let device = dm.confirm_pairing(&code, "iPhone", "ios", Some("push-tok".into())).unwrap();
        assert_eq!(device.user_id, "user-1");
        assert_eq!(device.name, "iPhone");
        assert!(device.active);
    }

    #[test]
    fn test_confirm_pairing_invalid_code() {
        let dm = DeviceManager::new();
        assert!(dm.confirm_pairing("000000", "x", "x", None).is_err());
    }

    #[test]
    fn test_pairing_code_single_use() {
        let dm = DeviceManager::new();
        let code = dm.generate_pairing_code("user-1");
        dm.confirm_pairing(&code, "Phone", "ios", None).unwrap();
        assert!(dm.confirm_pairing(&code, "Phone2", "ios", None).is_err());
    }

    #[test]
    fn test_list_devices() {
        let dm = DeviceManager::new();
        let code1 = dm.generate_pairing_code("user-1");
        dm.confirm_pairing(&code1, "Phone", "ios", None).unwrap();
        let code2 = dm.generate_pairing_code("user-1");
        dm.confirm_pairing(&code2, "Desktop", "desktop", None).unwrap();
        let code3 = dm.generate_pairing_code("user-2");
        dm.confirm_pairing(&code3, "Tablet", "android", None).unwrap();

        assert_eq!(dm.list_devices("user-1").len(), 2);
        assert_eq!(dm.list_devices("user-2").len(), 1);
        assert_eq!(dm.list_devices("user-3").len(), 0);
    }

    #[test]
    fn test_remove_device() {
        let dm = DeviceManager::new();
        let code = dm.generate_pairing_code("user-1");
        let device = dm.confirm_pairing(&code, "Phone", "ios", None).unwrap();
        assert!(dm.remove_device(&device.id).is_ok());
        assert!(dm.remove_device(&device.id).is_err()); // already removed
    }

    #[test]
    fn test_send_push_device_not_found() {
        let dm = DeviceManager::new();
        assert!(dm.send_push("nonexistent", "hello").is_err());
    }

    #[test]
    fn test_send_push_success() {
        let dm = DeviceManager::new();
        let code = dm.generate_pairing_code("user-1");
        let device = dm.confirm_pairing(&code, "Phone", "ios", None).unwrap();
        assert!(dm.send_push(&device.id, "notification").is_ok());
    }
}
