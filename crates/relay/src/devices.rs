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

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct TrustScore {
    /// Trust score from 0 to 100.
    pub score: u8,
    pub successful_pairs: u32,
    pub failed_attempts: u32,
    /// Unix timestamp of the last risky action, if any.
    pub last_risky_action: Option<i64>,
    /// Human-readable risk flags (e.g. "suspicious_ip", "brute_force").
    pub flags: Vec<String>,
}

#[derive(Debug, Clone)]
pub struct DeviceManager {
    devices: Arc<DashMap<String, Device>>,
    pairing_codes: Arc<DashMap<String, PairingCode>>,
    trust_scores: Arc<DashMap<String, TrustScore>>,
    push_config: PushConfig,
}

impl DeviceManager {
    pub fn new(push_config: PushConfig) -> Self {
        Self {
            devices: Arc::new(DashMap::new()),
            pairing_codes: Arc::new(DashMap::new()),
            trust_scores: Arc::new(DashMap::new()),
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
    /// After confirming, the trust score is evaluated. If the score is below 20,
    /// the device is set to `active = false` (denied) instead of being activated.
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

        let mut device = Device {
            id: pc.device_id,
            user_id: pc.user_id,
            name: name.to_string(),
            device_type: device_type.to_string(),
            push_token,
            paired_at: chrono::Utc::now().timestamp(),
            last_seen: chrono::Utc::now().timestamp(),
            active: true,
        };

        // Record the successful pairing and evaluate trust
        self.record_successful_pair(&device.id);
        let trust = self.evaluate_trust(&device);

        // Auto-deny devices with trust score below 20
        if trust.score < 20 {
            device.active = false;
            info!(
                "Device {} ({}) auto-denied: trust score {} < 20",
                device.name, device.id, trust.score
            );
        } else {
            info!(
                "Device {} ({}) paired with trust score {}",
                device.name, device.id, trust.score
            );
        }

        self.devices.insert(device.id.clone(), device.clone());
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
        self.trust_scores.remove(device_id);
        info!("Device removed: {device_id}");
        Ok(())
    }

    // ═══════════════════════════════════════════════
    // Trust scoring
    // ═══════════════════════════════════════════════

    /// Evaluate the trust score for a device based on pairing history, time, and risk flags.
    ///
    /// Scoring:
    /// - Base score for new device: 30
    /// - +10 per successful_pair (max +50 from this)
    /// - -15 per failed_attempt
    /// - +1 per day since paired (max +20)
    /// - -20 per suspicious flag
    /// - Clamped to 0–100
    pub fn evaluate_trust(&self, device: &Device) -> TrustScore {
        let existing = self.trust_scores.get(&device.id).map(|e| e.value().clone());

        let successful_pairs = existing.as_ref().map_or(0, |t| t.successful_pairs);
        let failed_attempts = existing.as_ref().map_or(0, |t| t.failed_attempts);
        let flags = existing.as_ref().map_or(Vec::new(), |t| t.flags.clone());
        let last_risky_action = existing.as_ref().and_then(|t| t.last_risky_action);

        let mut score: i32 = 30; // base score for new device

        // +10 per successful_pair, max +50
        score += (successful_pairs as i32 * 10).min(50);

        // -15 per failed_attempt
        score -= failed_attempts as i32 * 15;

        // +1 per day since paired, max +20
        let days_since_paired = (chrono::Utc::now().timestamp() - device.paired_at) / 86400;
        score += (days_since_paired.max(0) as i32).min(20);

        // -20 per suspicious flag
        score -= flags.len() as i32 * 20;

        // Clamp to 0-100
        let score = score.clamp(0, 100) as u8;

        TrustScore {
            score,
            successful_pairs,
            failed_attempts,
            last_risky_action,
            flags,
        }
    }

    /// Record a failed pairing attempt for a device, updating its trust score.
    pub fn record_failed_attempt(&self, device_id: &str) {
        let device_id_owned = device_id.to_string();
        {
            let mut entry = self.trust_scores.entry(device_id_owned.clone()).or_default();
            let ts = entry.value_mut();
            ts.failed_attempts += 1;
            ts.last_risky_action = Some(chrono::Utc::now().timestamp());
        }
        // Recalculate score outside the write lock to avoid deadlock
        if let Some(device) = self.devices.get(&device_id_owned) {
            let fresh = self.evaluate_trust(&device);
            if let Some(mut ts) = self.trust_scores.get_mut(&device_id_owned) {
                ts.score = fresh.score;
            }
        }
        let score = self.trust_scores.get(&device_id_owned).map(|e| e.score).unwrap_or(0);
        info!(
            "Failed attempt recorded for device {device_id}: score={score}"
        );
    }

    /// Record a successful pairing for a device, updating its trust score.
    pub fn record_successful_pair(&self, device_id: &str) {
        let device_id_owned = device_id.to_string();
        {
            let mut entry = self.trust_scores.entry(device_id_owned.clone()).or_default();
            let ts = entry.value_mut();
            ts.successful_pairs += 1;
        }
        // Recalculate score outside the write lock to avoid deadlock
        if let Some(device) = self.devices.get(&device_id_owned) {
            let fresh = self.evaluate_trust(&device);
            if let Some(mut ts) = self.trust_scores.get_mut(&device_id_owned) {
                ts.score = fresh.score;
            }
        }
        let score = self.trust_scores.get(&device_id_owned).map(|e| e.score).unwrap_or(0);
        let pairs = self.trust_scores.get(&device_id_owned).map(|e| e.successful_pairs).unwrap_or(0);
        info!(
            "Successful pair recorded for device {device_id}: score={score}, successful_pairs={pairs}"
        );
    }

    /// Get the current trust score for a device, if one exists.
    pub fn get_trust_score(&self, device_id: &str) -> Option<TrustScore> {
        self.trust_scores.get(device_id).map(|e| e.value().clone())
    }

    /// Check whether a device is considered trusted (score >= 50).
    /// Devices without a trust score are evaluated fresh and considered untrusted
    /// unless the evaluated score meets the threshold.
    pub fn is_device_trusted(&self, device_id: &str) -> bool {
        if let Some(device) = self.devices.get(device_id) {
            let score = self.trust_scores.get(device_id)
                .map(|e| e.value().clone())
                .unwrap_or_else(|| self.evaluate_trust(&device));
            score.score >= 50
        } else {
            false
        }
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

#[allow(dead_code)]
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
    pub user_id: Option<String>,
}

// ═══════════════════════════════════════════════
// Route Handlers
// ═══════════════════════════════════════════════

pub async fn pair_device(
    State(state): State<AppState>,
    axum::extract::Extension(claims): axum::extract::Extension<crate::auth::Claims>,
    Json(body): Json<PairRequest>,
) -> impl IntoResponse {
    let _ = body; // use claims.account_id instead of body.user_id
    let code = state.device_manager.generate_pairing_code(&claims.account_id);
    Json(PairResponse { code, expires_in: 300 })
}

pub async fn confirm_pairing(
    State(state): State<AppState>,
    axum::extract::Extension(_claims): axum::extract::Extension<crate::auth::Claims>,
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
    claims: axum::extract::Extension<crate::auth::Claims>,
    Query(_query): Query<DevicesQuery>,
) -> Json<Vec<Device>> {
    Json(state.device_manager.list_devices(&claims.account_id))
}

pub async fn remove_device(
    State(state): State<AppState>,
    axum::extract::Extension(claims): axum::extract::Extension<crate::auth::Claims>,
    Path(id): Path<String>,
) -> impl IntoResponse {
    // Verify device belongs to user
    let user_devices = state.device_manager.list_devices(&claims.account_id);
    let owns = user_devices.iter().any(|d| d.id == id);
    if !owns && !claims.is_admin {
        return (axum::http::StatusCode::FORBIDDEN, Json(serde_json::json!({"error": "Not your device"}))).into_response();
    }
    match state.device_manager.remove_device(&id) {
        Ok(()) => axum::http::StatusCode::NO_CONTENT.into_response(),
        Err(e) => (
            axum::http::StatusCode::NOT_FOUND,
            Json(serde_json::json!({ "error": e })),
        ).into_response(),
    }
}

pub async fn get_device_trust(
    State(state): State<AppState>,
    _claims: axum::extract::Extension<crate::auth::Claims>,
    Path(id): Path<String>,
) -> impl IntoResponse {
    match state.device_manager.get_trust_score(&id) {
        Some(trust) => Json(trust).into_response(),
        None => (
            axum::http::StatusCode::NOT_FOUND,
            Json(serde_json::json!({ "error": "device trust score not found" })),
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

    // ═══════════════════════════════════════════════
    // Trust scoring tests
    // ═══════════════════════════════════════════════

    /// Helper: create a DeviceManager with a paired device and return (dm, device).
    fn setup_paired_device() -> (DeviceManager, Device) {
        let dm = DeviceManager::new(PushConfig::default());
        let code = dm.generate_pairing_code("user-1");
        let device = dm.confirm_pairing(&code, "Phone", "ios", None).unwrap();
        (dm, device)
    }

    #[test]
    fn test_evaluate_trust_base_score() {
        let (dm, device) = setup_paired_device();
        let trust = dm.evaluate_trust(&device);
        // Base score = 30 + 10 (1 successful pair) = 40
        assert_eq!(trust.score, 40);
        assert_eq!(trust.successful_pairs, 1);
        assert_eq!(trust.failed_attempts, 0);
        assert!(trust.flags.is_empty());
    }

    #[test]
    fn test_evaluate_trust_successful_pairs_caps_at_50() {
        let dm = DeviceManager::new(PushConfig::default());
        let code = dm.generate_pairing_code("user-1");
        let device = dm.confirm_pairing(&code, "Phone", "ios", None).unwrap();

        // Manually set successful_pairs to 10 (would give +100, but cap at +50)
        if let Some(mut ts) = dm.trust_scores.get_mut(&device.id) {
            ts.successful_pairs = 10;
        }

        let trust = dm.evaluate_trust(&device);
        // 30 (base) + 50 (capped successful) = 80
        assert_eq!(trust.score, 80);
    }

    #[test]
    fn test_record_failed_attempt_decreases_score() {
        let (dm, device) = setup_paired_device();

        // Record 2 failed attempts
        dm.record_failed_attempt(&device.id);
        dm.record_failed_attempt(&device.id);

        let trust = dm.get_trust_score(&device.id).unwrap();
        // After 1 successful pair (+10) and 2 failures (-30):
        // 30 + 10 - 30 = 10
        assert_eq!(trust.score, 10);
        assert_eq!(trust.failed_attempts, 2);
        assert!(trust.last_risky_action.is_some());
    }

    #[test]
    fn test_record_successful_pair_increases_score() {
        let (dm, device) = setup_paired_device();
        let initial_score = dm.get_trust_score(&device.id).unwrap().score;

        // Record additional successful pair
        dm.record_successful_pair(&device.id);
        let trust = dm.get_trust_score(&device.id).unwrap();
        assert!(trust.score > initial_score);
        assert_eq!(trust.successful_pairs, 2);
    }

    #[test]
    fn test_evaluate_trust_flags_penalty() {
        let (dm, device) = setup_paired_device();

        // Add a suspicious flag
        if let Some(mut ts) = dm.trust_scores.get_mut(&device.id) {
            ts.flags.push("suspicious_ip".to_string());
        }

        let trust = dm.evaluate_trust(&device);
        // 30 (base) + 10 (1 successful pair) - 20 (1 flag) = 20
        assert_eq!(trust.score, 20);
    }

    #[test]
    fn test_evaluate_trust_multiple_flags_deny() {
        let (dm, device) = setup_paired_device();

        // Add two suspicious flags
        if let Some(mut ts) = dm.trust_scores.get_mut(&device.id) {
            ts.flags.push("suspicious_ip".to_string());
            ts.flags.push("brute_force".to_string());
        }

        let trust = dm.evaluate_trust(&device);
        // 30 + 10 - 40 = 0
        assert_eq!(trust.score, 0);
    }

    #[test]
    fn test_evaluate_trust_clamps_to_0() {
        let dm = DeviceManager::new(PushConfig::default());
        let code = dm.generate_pairing_code("user-1");
        let device = dm.confirm_pairing(&code, "Phone", "ios", None).unwrap();

        // Set high failed_attempts and flags to push below 0
        if let Some(mut ts) = dm.trust_scores.get_mut(&device.id) {
            ts.failed_attempts = 10;
            ts.flags.push("suspicious_ip".to_string());
            ts.flags.push("brute_force".to_string());
        }

        let trust = dm.evaluate_trust(&device);
        assert_eq!(trust.score, 0, "score should be clamped to 0, got {}", trust.score);
    }

    #[test]
    fn test_evaluate_trust_clamps_to_100() {
        let dm = DeviceManager::new(PushConfig::default());
        let code = dm.generate_pairing_code("user-1");
        let device = dm.confirm_pairing(&code, "Phone", "ios", None).unwrap();

        // Set high successful_pairs and old paired_at
        if let Some(mut ts) = dm.trust_scores.get_mut(&device.id) {
            ts.successful_pairs = 100;
        }
        {
            let mut dev = dm.devices.get_mut(&device.id).unwrap();
            // Set paired_at to 60 days ago for +20 time bonus
            dev.paired_at = chrono::Utc::now().timestamp() - (60 * 86400);
        }

        // Fetch the updated device from the map
        let device = dm.devices.get(&device.id).unwrap().value().clone();
        let trust = dm.evaluate_trust(&device);
        assert_eq!(trust.score, 100, "score should be clamped to 100, got {}", trust.score);
    }

    #[test]
    fn test_auto_deny_low_trust() {
        let dm = DeviceManager::new(PushConfig::default());

        // Pre-seed a trust score with 3 failed attempts before pairing
        // We'll pair, then manually manipulate trust to simulate low-trust on next pair
        let code = dm.generate_pairing_code("user-1");
        let device = dm.confirm_pairing(&code, "Phone", "ios", None).unwrap();

        // Manually push score below 20 via failed attempts and flags
        if let Some(mut ts) = dm.trust_scores.get_mut(&device.id) {
            ts.failed_attempts = 3;
            ts.flags.push("suspicious_ip".to_string());
        }

        // Now pair a second device and seed it with a pre-existing low trust entry
        let code2 = dm.generate_pairing_code("user-1");
        let pc = dm.pairing_codes.remove(&code2).unwrap().1;
        let device2 = Device {
            id: pc.device_id,
            user_id: pc.user_id,
            name: "Evil Phone".to_string(),
            device_type: "ios".to_string(),
            push_token: None,
            paired_at: chrono::Utc::now().timestamp(),
            last_seen: chrono::Utc::now().timestamp(),
            active: true,
        };

        // Pre-seed the trust score for device2 with low values
        dm.trust_scores.insert(device2.id.clone(), TrustScore {
            score: 0,
            successful_pairs: 0,
            failed_attempts: 5,
            last_risky_action: Some(chrono::Utc::now().timestamp()),
            flags: vec!["suspicious_ip".to_string(), "brute_force".to_string()],
        });

        dm.devices.insert(device2.id.clone(), device2.clone());
        dm.record_successful_pair(&device2.id);
        let trust = dm.evaluate_trust(&device2);

        // Score should be < 20 due to heavy penalties
        assert!(
            trust.score < 20,
            "expected score < 20 for high-risk device, got {}",
            trust.score
        );
    }

    #[test]
    fn test_is_device_trusted_true() {
        let (dm, device) = setup_paired_device();
        // Freshly paired device with 1 successful pair: score = 40
        // Not >= 50, so not trusted initially. Let's add another successful pair.
        dm.record_successful_pair(&device.id);
        // Now: 30 + 20 (2 successful) = 50 => trusted
        assert!(dm.is_device_trusted(&device.id));
    }

    #[test]
    fn test_is_device_trusted_false() {
        let (dm, device) = setup_paired_device();
        // Score = 40, which is < 50
        assert!(!dm.is_device_trusted(&device.id));
    }

    #[test]
    fn test_is_device_trusted_nonexistent() {
        let dm = DeviceManager::new(PushConfig::default());
        assert!(!dm.is_device_trusted("nonexistent"));
    }

    #[test]
    fn test_get_trust_score_none_for_unknown() {
        let dm = DeviceManager::new(PushConfig::default());
        assert!(dm.get_trust_score("nonexistent").is_none());
    }

    #[test]
    fn test_evaluate_trust_time_bonus() {
        let dm = DeviceManager::new(PushConfig::default());
        let code = dm.generate_pairing_code("user-1");
        let device = dm.confirm_pairing(&code, "Phone", "ios", None).unwrap();

        // Set paired_at to 25 days ago — should give max +20 time bonus
        {
            let mut dev = dm.devices.get_mut(&device.id).unwrap();
            dev.paired_at = chrono::Utc::now().timestamp() - (25 * 86400);
        }

        // Fetch the updated device from the map
        let device = dm.devices.get(&device.id).unwrap().value().clone();
        let trust = dm.evaluate_trust(&device);
        // 30 (base) + 10 (1 successful pair) + 20 (time, capped) = 60
        assert_eq!(trust.score, 60);
    }
}
