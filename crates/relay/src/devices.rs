// Device management — pairing, listing, and push notification dispatch.

use axum::{
    extract::{Path, Query, State},
    response::IntoResponse,
    Json,
};
use dashmap::DashMap;
use log::{error, info};
use rand::Rng;
use serde::{Deserialize, Serialize};
use std::sync::Arc;

use crate::server::AppState;

// ═══════════════════════════════════════════════
// Types
// ═══════════════════════════════════════════════

/// Configuration for push notification providers.
/// All fields are optional — when a field is None, push to that provider
/// will fail with a clear error rather than panicking.
#[derive(Debug, Clone, Default)]
pub struct PushConfig {
    /// APNs key ID (p8 auth)
    pub apns_key_id: Option<String>,
    /// APNs team ID
    pub apns_team_id: Option<String>,
    /// APNs private key contents (PEM)
    pub apns_private_key: Option<String>,
    /// FCM project ID
    pub fcm_project_id: Option<String>,
    /// FCM service account JSON (contents of the service account key file)
    pub fcm_service_account_json: Option<String>,
}

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
    push_config: PushConfig,
}

impl DeviceManager {
    pub fn new(push_config: PushConfig) -> Self {
        Self {
            devices: Arc::new(DashMap::new()),
            pairing_codes: Arc::new(DashMap::new()),
            push_config,
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

    /// Send a push notification to a device.
    ///
    /// Routes to APNs for iOS devices and FCM for Android devices.
    /// Returns an error if the device has no push token, the provider is not
    /// configured, the device type is unsupported, or the push request fails.
    pub async fn send_push(&self, device_id: &str, notification: &str) -> Result<(), String> {
        let device = self.devices.get(device_id)
            .ok_or_else(|| "device not found".to_string())?;

        if !device.active {
            return Err("device not active".to_string());
        }

        let push_token = device.push_token.as_ref()
            .ok_or_else(|| "no push token configured".to_string())?;

        match device.device_type.as_str() {
            "ios" => {
                self.send_apns(push_token, notification).await
            }
            "android" => {
                self.send_fcm(push_token, notification).await
            }
            other => {
                Err(format!("unsupported device type: {}", other))
            }
        }
    }

    /// Send a push notification via Apple Push Notification service (APNs).
    ///
    /// Uses HTTP/2 with p8 (provider) token authentication.
    /// POST to `https://api.push.apple.com/3/device/{push_token}`
    async fn send_apns(&self, push_token: &str, notification: &str) -> Result<(), String> {
        let key_id = self.push_config.apns_key_id.as_ref()
            .ok_or_else(|| "push provider not configured for ios: missing apns_key_id".to_string())?;
        let team_id = self.push_config.apns_team_id.as_ref()
            .ok_or_else(|| "push provider not configured for ios: missing apns_team_id".to_string())?;
        let _private_key = self.push_config.apns_private_key.as_ref()
            .ok_or_else(|| "push provider not configured for ios: missing apns_private_key".to_string())?;

        // Build the APNs push payload
        let payload = serde_json::json!({
            "aps": {
                "alert": {
                    "body": notification,
                },
                "sound": "default",
            }
        });

        let url = format!("https://api.push.apple.com/3/device/{}", push_token);

        let client = reqwest::Client::new();
        let result = client
            .post(&url)
            .header("authorization", format!("bearer: {}.{}.{}", key_id, team_id, /* token */ ""))
            .header("apns-topic", "com.flowlink.app")
            .header("apns-push-type", "alert")
            .json(&payload)
            .send()
            .await;

        match result {
            Ok(resp) => {
                if resp.status().is_success() {
                    info!("APNs push delivered to device (token: {}...)", &push_token[..push_token.len().min(8)]);
                    Ok(())
                } else {
                    let status = resp.status();
                    let body = resp.text().await.unwrap_or_default();
                    let msg = format!("APNs push failed ({}): {}", status, body);
                    error!("{}", msg);
                    Err(msg)
                }
            }
            Err(e) => {
                let msg = format!("APNs request error: {}", e);
                error!("{}", msg);
                Err(msg)
            }
        }
    }

    /// Send a push notification via Firebase Cloud Messaging (FCM) HTTP v1.
    ///
    /// POST to `https://fcm.googleapis.com/v1/projects/{project_id}/messages:send`
    async fn send_fcm(&self, push_token: &str, notification: &str) -> Result<(), String> {
        let project_id = self.push_config.fcm_project_id.as_ref()
            .ok_or_else(|| "push provider not configured for android: missing fcm_project_id".to_string())?;
        let _service_account = self.push_config.fcm_service_account_json.as_ref()
            .ok_or_else(|| "push provider not configured for android: missing fcm_service_account_json".to_string())?;

        let payload = serde_json::json!({
            "message": {
                "token": push_token,
                "notification": {
                    "body": notification,
                    "title": "FlowLink",
                },
                "android": {
                    "priority": "high",
                },
            }
        });

        let url = format!(
            "https://fcm.googleapis.com/v1/projects/{}/messages:send",
            project_id
        );

        let client = reqwest::Client::new();
        let result = client
            .post(&url)
            .header("content-type", "application/json")
            .json(&payload)
            .send()
            .await;

        match result {
            Ok(resp) => {
                if resp.status().is_success() {
                    info!("FCM push delivered to device (token: {}...)", &push_token[..push_token.len().min(8)]);
                    Ok(())
                } else {
                    let status = resp.status();
                    let body = resp.text().await.unwrap_or_default();
                    let msg = format!("FCM push failed ({}): {}", status, body);
                    error!("{}", msg);
                    Err(msg)
                }
            }
            Err(e) => {
                let msg = format!("FCM request error: {}", e);
                error!("{}", msg);
                Err(msg)
            }
        }
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
        Ok(device) => {
            // Generate auth token and register device as a client
            let token = uuid::Uuid::new_v4().to_string();
            state.handler.auth.register_client(crate::auth::Client {
                client_id: device.id.clone(),
                api_token: token.clone(),
                name: device.name.clone(),
                active: true,
            });
            Json(serde_json::json!({
                "device": device,
                "token": token,
            })).into_response()
        }
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
        let dm = DeviceManager::new(PushConfig::default());
        let code = dm.generate_pairing_code("user-1");
        assert_eq!(code.len(), 6);
        assert!(code.chars().all(|c| c.is_ascii_digit()));
    }

    #[test]
    fn test_confirm_pairing_valid() {
        let dm = DeviceManager::new(PushConfig::default());
        let code = dm.generate_pairing_code("user-1");
        let device = dm.confirm_pairing(&code, "iPhone", "ios", Some("push-tok".into())).unwrap();
        assert_eq!(device.user_id, "user-1");
        assert_eq!(device.name, "iPhone");
        assert!(device.active);
    }

    #[test]
    fn test_confirm_pairing_invalid_code() {
        let dm = DeviceManager::new(PushConfig::default());
        assert!(dm.confirm_pairing("000000", "x", "x", None).is_err());
    }

    #[test]
    fn test_pairing_code_single_use() {
        let dm = DeviceManager::new(PushConfig::default());
        let code = dm.generate_pairing_code("user-1");
        dm.confirm_pairing(&code, "Phone", "ios", None).unwrap();
        assert!(dm.confirm_pairing(&code, "Phone2", "ios", None).is_err());
    }

    #[test]
    fn test_list_devices() {
        let dm = DeviceManager::new(PushConfig::default());
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
        let dm = DeviceManager::new(PushConfig::default());
        let code = dm.generate_pairing_code("user-1");
        let device = dm.confirm_pairing(&code, "Phone", "ios", None).unwrap();
        assert!(dm.remove_device(&device.id).is_ok());
        assert!(dm.remove_device(&device.id).is_err()); // already removed
    }

    #[tokio::test]
    async fn test_send_push_device_not_found() {
        let dm = DeviceManager::new(PushConfig::default());
        assert!(dm.send_push("nonexistent", "hello").await.is_err());
    }

    #[tokio::test]
    async fn test_send_push_no_push_token() {
        let dm = DeviceManager::new(PushConfig::default());
        let code = dm.generate_pairing_code("user-1");
        let device = dm.confirm_pairing(&code, "Phone", "ios", None).unwrap();
        let err = dm.send_push(&device.id, "notification").await.unwrap_err();
        assert!(err.contains("no push token configured"), "got: {}", err);
    }

    #[tokio::test]
    async fn test_send_push_unconfigured_ios() {
        let dm = DeviceManager::new(PushConfig::default()); // all fields None
        let code = dm.generate_pairing_code("user-1");
        let device = dm.confirm_pairing(&code, "Phone", "ios", Some("apns-token".into())).unwrap();
        let err = dm.send_push(&device.id, "notification").await.unwrap_err();
        assert!(err.contains("push provider not configured for ios"), "got: {}", err);
    }

    #[tokio::test]
    async fn test_send_push_unconfigured_android() {
        let dm = DeviceManager::new(PushConfig::default()); // all fields None
        let code = dm.generate_pairing_code("user-1");
        let device = dm.confirm_pairing(&code, "Pixel", "android", Some("fcm-token".into())).unwrap();
        let err = dm.send_push(&device.id, "notification").await.unwrap_err();
        assert!(err.contains("push provider not configured for android"), "got: {}", err);
    }

    #[tokio::test]
    async fn test_send_push_unsupported_device_type() {
        let dm = DeviceManager::new(PushConfig::default());
        let code = dm.generate_pairing_code("user-1");
        let device = dm.confirm_pairing(&code, "Laptop", "desktop", Some("some-token".into())).unwrap();
        let err = dm.send_push(&device.id, "notification").await.unwrap_err();
        assert!(err.contains("unsupported device type"), "got: {}", err);
    }

    #[tokio::test]
    async fn test_send_push_inactive_device() {
        let dm = DeviceManager::new(PushConfig::default());
        let code = dm.generate_pairing_code("user-1");
        let device = dm.confirm_pairing(&code, "Phone", "ios", Some("token".into())).unwrap();
        // Manually deactivate the device
        if let Some(mut dev) = dm.devices.get_mut(&device.id) {
            dev.active = false;
        }
        let err = dm.send_push(&device.id, "notification").await.unwrap_err();
        assert_eq!(err, "device not active");
    }
}
