use chrono::Utc;
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;

use super::*;

pub struct WebhookMetrics {
    pub stats: Arc<RwLock<MetricsData>>,
    pub enable_metrics: bool,
    pub start_time: chrono::DateTime<Utc>,
}

#[derive(Debug, Clone)]
struct MetricsData {
    total_received: i64,
    total_routed: i64,
    total_failed: i64,
    service_stats: HashMap<String, ServiceMetrics>,
    last_received: Option<chrono::DateTime<Utc>>,
}

#[derive(Debug, Clone)]
struct ServiceMetrics {
    received: i64,
    routed: i64,
    failed: i64,
    last_received: Option<chrono::DateTime<Utc>>,
}

impl WebhookMetrics {
    pub fn new(enable_metrics: bool) -> Self {
        Self {
            stats: Arc::new(RwLock::new(MetricsData {
                total_received: 0,
                total_routed: 0,
                total_failed: 0,
                service_stats: HashMap::new(),
                last_received: None,
            })),
            enable_metrics,
            start_time: Utc::now(),
        }
    }
    
    pub async fn increment_received(&self, service: &str) {
        if !self.enable_metrics {
            return;
        }
        
        let mut stats = self.stats.write().await;
        
        stats.total_received += 1;
        stats.last_received = Some(Utc::now());
        
        let service_metrics = stats.service_stats.entry(service.to_string()).or_insert(ServiceMetrics {
            received: 0,
            routed: 0,
            failed: 0,
            last_received: None,
        });
        service_metrics.received += 1;
        service_metrics.last_received = Some(Utc::now());
    }
    
    pub async fn increment_routed(&self, service: &str) {
        if !self.enable_metrics {
            return;
        }
        
        let mut stats = self.stats.write().await;
        stats.total_routed += 1;
        
        if let Some(service_metrics) = stats.service_stats.get_mut(service) {
            service_metrics.routed += 1;
        }
    }
    
    pub async fn increment_failed(&self, service: &str) {
        if !self.enable_metrics {
            return;
        }
        
        let mut stats = self.stats.write().await;
        stats.total_failed += 1;
        
        let service_metrics = stats.service_stats.entry(service.to_string()).or_insert(ServiceMetrics {
            received: 0,
            routed: 0,
            failed: 0,
            last_received: None,
        });
        service_metrics.failed += 1;
    }
    
    pub async fn get_stats(&self) -> WebhookStats {
        let stats = self.stats.read().await;
        
        WebhookStats {
            total_received: stats.total_received,
            total_routed: stats.total_routed,
            total_failed: stats.total_failed,
            service_stats: stats.service_stats.iter()
                .map(|(service, m)| (service.clone(), ServiceStats {
                    received: m.received,
                    routed: m.routed,
                    failed: m.failed,
                    last_received: m.last_received,
                }))
                .collect(),
            last_received: stats.last_received,
            uptime_seconds: Utc::now()
                .signed_duration_since(self.start_time)
                .num_seconds(),
        }
    }
    
    pub async fn reset_stats(&self) {
        let mut stats = self.stats.write().await;
        
        stats.total_received = 0;
        stats.total_routed = 0;
        stats.total_failed = 0;
        stats.service_stats.clear();
        stats.last_received = None;
    }
    
    pub async fn record_webhook(&self, webhook: &Webhook, success: bool, routed: bool) {
        if !self.enable_metrics {
            return;
        }
        
        self.increment_received(webhook.service.as_str()).await;
        
        if routed {
            self.increment_routed(webhook.service.as_str()).await;
        } else {
            self.increment_failed(webhook.service.as_str()).await;
        }
    }
}

// Webhook verification module
pub mod verification {
    use anyhow::Result;
    use base64::{Engine as _, engine::general_purpose::STANDARD};
    
    use super::*;
    
    pub async fn verify_webhook_signature(
        webhook: &Webhook,
        signature: &str,
        secret: &str,
    ) -> Result<bool> {
        // Different services use different signature formats
        match webhook.service.as_str() {
            "github" | "gitlab" => {
                // GitHub and GitLab use SHA-256 with hex encoding
                verify_hmac_sha256_hex(webhook.data.as_bytes(), signature, secret)
            }
            "discord" => {
                // Discord uses HMAC with base64 encoding
                verify_hmac_sha256_base64(webhook.data.as_bytes(), signature, secret)
            }
            "stripe" => {
                // Stripe uses SHA-256 with base64 encoding
                verify_hmac_sha256_base64(webhook.data.as_bytes(), signature, secret)
            }
            _ => {
                // Unknown service - skip verification
                Ok(true)
            }
        }
    }
    
    pub fn verify_hmac_sha256_hex(data: &[u8], signature_hex: &str, secret: &str) -> bool {
        use hmac::{Hmac, Mac};
        use sha2::Sha256;
        
        type HmacSha256 = Hmac<Sha256>;
        
        let key = secret.as_bytes();
        let mut mac = HmacSha256::new_from_slice(key)
            .expect("Failed to create HMAC key");
        
        mac.update(data);
        
        let expected_hex = hex::encode(mac.finalize().into_bytes());
        
        // Compare case-insensitively
        expected_hex.eq_ignore_ascii_case(signature_hex)
    }
    
    pub fn verify_hmac_sha256_base64(data: &[u8], signature_b64: &str, secret: &str) -> bool {
        use hmac::{Hmac, Mac};
        use sha2::Sha256;
        
        type HmacSha256 = Hmac<Sha256>;
        
        let key = secret.as_bytes();
        let mut mac = HmacSha256::new_from_slice(key)
            .expect("Failed to create HMAC key");
        
        mac.update(data);
        
        let expected_b64 = STANDARD.encode(mac.finalize().into_bytes());
        
        expected_b64 == signature_b64
    }
    
    pub async fn verify_webhook_timestamp(
        webhook: &Webhook,
        max_age_seconds: i64,
    ) -> Result<bool> {
        let age = Utc::now()
            .signed_duration_since(webhook.timestamp)
            .num_seconds();
        
        if age > max_age_seconds {
            log::warn!("Webhook {} is too old: {} seconds", webhook.id, age);
            return Ok(false);
        }
        
        Ok(true)
    }
}

// Rate limiting module
pub mod rate_limit {
    use std::collections::HashMap;
    use std::sync::Arc;
    use tokio::sync::RwLock;
    use chrono::{Utc, Duration};

    use super::*;

    #[derive(Debug, Clone)]
    pub struct RateLimitConfig {
        pub requests_per_minute: i32,
        pub requests_per_hour: i32,
        pub requests_per_day: i32,
    }

    #[derive(Debug, Clone)]
    struct RateLimitRecord {
        pub request_count: i32,
        pub minute_window: Vec<i64>,
        pub hour_window: Vec<i64>,
        pub day_window: Vec<i64>,
    }

    pub struct RateLimiter {
        pub limits: HashMap<String, RateLimitConfig>,
        pub records: Arc<RwLock<HashMap<String, RateLimitRecord>>>,
    }

    impl RateLimiter {
        pub fn new() -> Self {
            Self {
                limits: HashMap::new(),
                records: Arc::new(RwLock::new(HashMap::new())),
            }
        }

        pub fn register_service(&mut self, service: String, config: RateLimitConfig) {
            self.limits.insert(service, config);
        }

        pub async fn check_request(&self, service: &str) -> (bool, Option<String>) {
            let now = Utc::now().timestamp();

            if let Some(config) = self.limits.get(service) {
                let mut records = self.records.write().await;

                if let Some(record) = records.get_mut(service) {
                    // Remove old timestamps from windows
                    record.minute_window.retain(|&ts| now - ts < 60);
                    record.hour_window.retain(|&ts| now - ts < 3600);
                    record.day_window.retain(|&ts| now - ts < 86400);

                    // Check limits
                    let minute_exceeded = record.minute_window.len() >= config.requests_per_minute as usize;
                    let hour_exceeded = record.hour_window.len() >= config.requests_per_hour as usize;
                    let day_exceeded = record.day_window.len() >= config.requests_per_day as usize;

                    if minute_exceeded || hour_exceeded || day_exceeded {
                        return (false, Some(format!("Rate limit exceeded for {} after {} requests", service, record.minute_window.len())));
                    }

                    // Add current request
                    record.minute_window.push(now);
                    record.hour_window.push(now);
                    record.day_window.push(now);

                    (true, None)
                } else {
                    // Create new record
                    records.insert(service.to_string(), RateLimitRecord {
                        request_count: 0,
                        minute_window: vec![now],
                        hour_window: vec![now],
                        day_window: vec![now],
                    });

                    (true, None)
                }
            } else {
                (true, None) // No limit configured
            }
        }

        pub async fn get_stats(&self, service: &str) -> Option<RateLimitStats> {
            let records = self.records.read().await;
            records.get(service).map(|record| RateLimitStats {
                request_count: record.minute_window.len(),
                window_start: *record.minute_window.first()?,
                window_end: *record.minute_window.last()?,
            })
        }
    }

    #[derive(Debug, Clone)]
    pub struct RateLimitStats {
        pub request_count: usize,
        pub window_start: i64,
        pub window_end: i64,
    }
}