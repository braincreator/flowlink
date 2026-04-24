// Unified Tiered Rate Limiter — plan-aware, cleanup-enabled, internal-bypass.
// Replaces the old AuthRateLimiter with a comprehensive sliding-window system.

use dashmap::DashMap;
use serde::Deserialize;
use std::sync::{Arc, Mutex};
use std::time::Instant;

/// Rate limit tier based on org plan.
#[derive(Debug, Clone)]
pub struct RateLimitTier {
    pub api_requests_per_min: u32,
    pub auth_attempts_per_5min: u32,
    pub auth_attempts_per_hour: u32,
    pub email_change_per_hour: u32,
    pub invite_per_hour: u32,
}

pub const FREE_TIER: RateLimitTier = RateLimitTier {
    api_requests_per_min: 180,
    auth_attempts_per_5min: 10,
    auth_attempts_per_hour: 30,
    email_change_per_hour: 3,
    invite_per_hour: 5,
};

pub const STARTER_TIER: RateLimitTier = RateLimitTier {
    api_requests_per_min: 500,
    auth_attempts_per_5min: 15,
    auth_attempts_per_hour: 60,
    email_change_per_hour: 5,
    invite_per_hour: 20,
};

pub const PRO_TIER: RateLimitTier = RateLimitTier {
    api_requests_per_min: 2000,
    auth_attempts_per_5min: 30,
    auth_attempts_per_hour: 200,
    email_change_per_hour: 10,
    invite_per_hour: 100,
};

pub const ENTERPRISE_TIER: RateLimitTier = RateLimitTier {
    api_requests_per_min: 5000,
    auth_attempts_per_5min: 60,
    auth_attempts_per_hour: 500,
    email_change_per_hour: 20,
    invite_per_hour: 500,
};

/// Config-file overrides for rate limits per plan tier.
/// All fields optional — missing values use built-in tier defaults.
/// Hot-reloadable: changes take effect on config reload.
#[derive(Debug, Clone, Default, Deserialize)]
pub struct RateLimitsConfig {
    /// Free plan: API requests per minute (default: 180)
    pub free_api_rpm: Option<u32>,
    /// Free plan: auth attempts per 5 min (default: 10)
    pub free_auth_5m: Option<u32>,
    /// Free plan: auth attempts per hour (default: 30)
    pub free_auth_1h: Option<u32>,
    /// Starter plan: API requests per minute (default: 500)
    pub starter_api_rpm: Option<u32>,
    /// Starter plan: auth attempts per 5 min (default: 15)
    pub starter_auth_5m: Option<u32>,
    /// Starter plan: auth attempts per hour (default: 60)
    pub starter_auth_1h: Option<u32>,
    /// Pro plan: API requests per minute (default: 2000)
    pub pro_api_rpm: Option<u32>,
    /// Pro plan: auth attempts per 5 min (default: 30)
    pub pro_auth_5m: Option<u32>,
    /// Pro plan: auth attempts per hour (default: 200)
    pub pro_auth_1h: Option<u32>,
    /// Enterprise plan: API requests per minute (default: 5000)
    pub enterprise_api_rpm: Option<u32>,
    /// Enterprise plan: auth attempts per 5 min (default: 60)
    pub enterprise_auth_5m: Option<u32>,
    /// Enterprise plan: auth attempts per hour (default: 500)
    pub enterprise_auth_1h: Option<u32>,
    /// Email change limit per hour — applied to all tiers (default: 3)
    pub email_change_per_hour: Option<u32>,
    /// Org invite limit per hour — applied to all tiers (default: 5)
    pub invite_per_hour: Option<u32>,
}

impl RateLimitsConfig {
    /// Build from a generic HashMap (as stored in RelayConfig.rate_limits).
    pub fn from_map(map: &std::collections::HashMap<String, u32>) -> Self {
        Self {
            free_api_rpm: map.get("free_api_rpm").copied(),
            free_auth_5m: map.get("free_auth_5m").copied(),
            free_auth_1h: map.get("free_auth_1h").copied(),
            starter_api_rpm: map.get("starter_api_rpm").copied(),
            starter_auth_5m: map.get("starter_auth_5m").copied(),
            starter_auth_1h: map.get("starter_auth_1h").copied(),
            pro_api_rpm: map.get("pro_api_rpm").copied(),
            pro_auth_5m: map.get("pro_auth_5m").copied(),
            pro_auth_1h: map.get("pro_auth_1h").copied(),
            enterprise_api_rpm: map.get("enterprise_api_rpm").copied(),
            enterprise_auth_5m: map.get("enterprise_auth_5m").copied(),
            enterprise_auth_1h: map.get("enterprise_auth_1h").copied(),
            email_change_per_hour: map.get("email_change_per_hour").copied(),
            invite_per_hour: map.get("invite_per_hour").copied(),
        }
    }
    /// Build tiers from config overrides, falling back to built-in defaults.
    pub fn free_tier(&self) -> RateLimitTier {
        RateLimitTier {
            api_requests_per_min: self.free_api_rpm.unwrap_or(FREE_TIER.api_requests_per_min),
            auth_attempts_per_5min: self.free_auth_5m.unwrap_or(FREE_TIER.auth_attempts_per_5min),
            auth_attempts_per_hour: self.free_auth_1h.unwrap_or(FREE_TIER.auth_attempts_per_hour),
            email_change_per_hour: self.email_change_per_hour.unwrap_or(FREE_TIER.email_change_per_hour),
            invite_per_hour: self.invite_per_hour.unwrap_or(FREE_TIER.invite_per_hour),
        }
    }
    pub fn starter_tier(&self) -> RateLimitTier {
        RateLimitTier {
            api_requests_per_min: self.starter_api_rpm.unwrap_or(STARTER_TIER.api_requests_per_min),
            auth_attempts_per_5min: self.starter_auth_5m.unwrap_or(STARTER_TIER.auth_attempts_per_5min),
            auth_attempts_per_hour: self.starter_auth_1h.unwrap_or(STARTER_TIER.auth_attempts_per_hour),
            email_change_per_hour: self.email_change_per_hour.unwrap_or(STARTER_TIER.email_change_per_hour),
            invite_per_hour: self.invite_per_hour.unwrap_or(STARTER_TIER.invite_per_hour),
        }
    }
    pub fn pro_tier(&self) -> RateLimitTier {
        RateLimitTier {
            api_requests_per_min: self.pro_api_rpm.unwrap_or(PRO_TIER.api_requests_per_min),
            auth_attempts_per_5min: self.pro_auth_5m.unwrap_or(PRO_TIER.auth_attempts_per_5min),
            auth_attempts_per_hour: self.pro_auth_1h.unwrap_or(PRO_TIER.auth_attempts_per_hour),
            email_change_per_hour: self.email_change_per_hour.unwrap_or(PRO_TIER.email_change_per_hour),
            invite_per_hour: self.invite_per_hour.unwrap_or(PRO_TIER.invite_per_hour),
        }
    }
    pub fn enterprise_tier(&self) -> RateLimitTier {
        RateLimitTier {
            api_requests_per_min: self.enterprise_api_rpm.unwrap_or(ENTERPRISE_TIER.api_requests_per_min),
            auth_attempts_per_5min: self.enterprise_auth_5m.unwrap_or(ENTERPRISE_TIER.auth_attempts_per_5min),
            auth_attempts_per_hour: self.enterprise_auth_1h.unwrap_or(ENTERPRISE_TIER.auth_attempts_per_hour),
            email_change_per_hour: self.email_change_per_hour.unwrap_or(ENTERPRISE_TIER.email_change_per_hour),
            invite_per_hour: self.invite_per_hour.unwrap_or(ENTERPRISE_TIER.invite_per_hour),
        }
    }
}

/// Category of rate-limited operation.
#[derive(Debug, Clone, Copy)]
pub enum RateLimitCategory {
    AuthLogin,
    AuthSignup,
    EmailChange,
    OrgInvite,
    GeneralApi,
}

impl RateLimitCategory {
    /// Returns (max_requests, window_seconds) for this category at the given tier.
    pub fn limits(&self, tier: &RateLimitTier) -> (u32, u64) {
        match self {
            Self::AuthLogin | Self::AuthSignup => (tier.auth_attempts_per_5min, 300),
            Self::EmailChange => (tier.email_change_per_hour, 3600),
            Self::OrgInvite => (tier.invite_per_hour, 3600),
            Self::GeneralApi => (tier.api_requests_per_min, 60),
        }
    }

    /// Prefix used when constructing keys.
    pub fn prefix(&self) -> &'static str {
        match self {
            Self::AuthLogin => "auth_login",
            Self::AuthSignup => "auth_signup",
            Self::EmailChange => "email_change",
            Self::OrgInvite => "org_invite",
            Self::GeneralApi => "api",
        }
    }
}

/// Stats for monitoring the rate limiter.
#[derive(Debug, Clone, serde::Serialize)]
pub struct RateLimitStats {
    pub active_windows: usize,
    pub last_cleanup: String,
}

/// Tiered sliding-window rate limiter with periodic cleanup.
pub struct TieredRateLimiter {
    windows: Arc<DashMap<String, (u32, Instant, u64)>>, // key -> (count, window_start, window_secs)
    last_cleanup: Mutex<Instant>,
}

impl TieredRateLimiter {
    pub fn new() -> Self {
        Self {
            windows: Arc::new(DashMap::new()),
            last_cleanup: Mutex::new(Instant::now()),
        }
    }

    /// Check rate limit. Returns `Ok(())` or `Err(retry_after_seconds)`.
    pub fn check(&self, key: &str, max: u32, window_secs: u64) -> Result<(), u64> {
        let now = Instant::now();
        let mut entry = self
            .windows
            .entry(key.to_string())
            .or_insert_with(|| (0, now, window_secs));

        let elapsed = now.duration_since(entry.1).as_secs();
        if elapsed >= window_secs {
            *entry = (1, now, window_secs);
            return Ok(());
        }

        entry.0 += 1;
        if entry.0 > max {
            let remaining = window_secs - elapsed;
            return Err(remaining);
        }
        Ok(())
    }

    /// Check with plan-aware tier selection.
    pub fn check_tiered(
        &self,
        key: &str,
        category: RateLimitCategory,
        tier: &RateLimitTier,
    ) -> Result<(), u64> {
        let (max, window_secs) = category.limits(tier);
        let full_key = format!("{}:{}", category.prefix(), key);
        self.check(&full_key, max, window_secs)
    }

    /// Remove all expired entries. Call periodically (e.g. every 60s).
    pub fn cleanup(&self) {
        let now = Instant::now();
        self.windows.retain(|_key, (_count, start, window_secs)| {
            now.duration_since(*start).as_secs() < *window_secs
        });
        if let Ok(mut last) = self.last_cleanup.lock() {
            *last = now;
        }
    }

    /// Get monitoring stats.
    pub fn stats(&self) -> RateLimitStats {
        let last_cleanup = self
            .last_cleanup
            .lock()
            .map(|t| {
                let epoch = std::time::SystemTime::now()
                    - t.elapsed();
                let dt = chrono::DateTime::<chrono::Utc>::from(epoch);
                dt.to_rfc3339()
            })
            .unwrap_or_else(|_| "unknown".into());
        RateLimitStats {
            active_windows: self.windows.len(),
            last_cleanup,
        }
    }
}

impl Default for TieredRateLimiter {
    fn default() -> Self {
        Self::new()
    }
}

/// Detect internal requests that should bypass rate limiting.
pub fn is_internal_request(req: &axum::extract::Request) -> bool {
    // X-Internal-Skip-Rate-Limit header from systemd timers / cron
    if req
        .headers()
        .get("x-internal-skip-rate-limit")
        .and_then(|v| v.to_str().ok())
        == Some("true")
    {
        return true;
    }

    let ip = extract_client_ip(req);
    ip == "127.0.0.1"
        || ip == "::1"
        || ip.starts_with("10.")
        || ip.starts_with("172.")
        || ip == "unknown"
}

/// Extract client IP (re-exported from auth_rate_limiter for backward compat).
pub fn extract_client_ip(req: &axum::extract::Request) -> String {
    if let Some(forwarded) = req.headers().get("forwarded") {
        if let Ok(val) = forwarded.to_str() {
            for part in val.split(';') {
                let part = part.trim();
                if let Some(ip) = part.strip_prefix("for=") {
                    let ip = ip.trim();
                    let ip = ip.trim_matches('"').split(':').next().unwrap_or(ip);
                    return ip.to_string();
                }
            }
        }
    }
    if let Some(xff) = req.headers().get("x-forwarded-for") {
        if let Ok(val) = xff.to_str() {
            if let Some(ip) = val.split(',').next() {
                return ip.trim().to_string();
            }
        }
    }
    if let Some(real_ip) = req.headers().get("x-real-ip") {
        if let Ok(val) = real_ip.to_str() {
            return val.trim().to_string();
        }
    }
    "unknown".to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_check_allows_under_limit() {
        let rl = TieredRateLimiter::new();
        for _ in 0..5 {
            assert!(rl.check("k", 5, 300).is_ok());
        }
    }

    #[test]
    fn test_check_blocks_over_limit() {
        let rl = TieredRateLimiter::new();
        for _ in 0..5 {
            assert!(rl.check("k", 5, 300).is_ok());
        }
        let err = rl.check("k", 5, 300).unwrap_err();
        assert!(err <= 300);
    }

    #[test]
    fn test_check_tiered_uses_plan_limits() {
        let rl = TieredRateLimiter::new();
        // Free tier allows 10 auth attempts per 5min
        for _ in 0..10 {
            assert!(rl.check_tiered("user@t.com", RateLimitCategory::AuthLogin, &FREE_TIER).is_ok());
        }
        assert!(rl.check_tiered("user@t.com", RateLimitCategory::AuthLogin, &FREE_TIER).is_err());
    }

    #[test]
    fn test_check_tiered_pro_higher_limits() {
        let rl = TieredRateLimiter::new();
        // Pro tier allows 30 auth attempts per 5min
        for _ in 0..30 {
            assert!(rl.check_tiered("user@t.com", RateLimitCategory::AuthLogin, &PRO_TIER).is_ok());
        }
        assert!(rl.check_tiered("user@t.com", RateLimitCategory::AuthLogin, &PRO_TIER).is_err());
    }

    #[test]
    fn test_cleanup_removes_expired() {
        let rl = TieredRateLimiter::new();
        // Use a 1-second window
        for _ in 0..3 {
            rl.check("short", 3, 1).ok();
        }
        assert_eq!(rl.stats().active_windows, 1);
        std::thread::sleep(std::time::Duration::from_secs(2));
        rl.cleanup();
        assert_eq!(rl.stats().active_windows, 0);
    }

    #[test]
    fn test_separate_keys() {
        let rl = TieredRateLimiter::new();
        for _ in 0..5 {
            rl.check("a", 1, 300).ok();
        }
        assert!(rl.check("b", 1, 300).is_ok());
    }
}
