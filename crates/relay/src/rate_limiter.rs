// Unified Tiered Rate Limiter — plan-aware, cleanup-enabled, internal-bypass.
// Replaces the old AuthRateLimiter with a comprehensive sliding-window system.

use dashmap::DashMap;
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
    api_requests_per_min: 60,
    auth_attempts_per_5min: 10,
    auth_attempts_per_hour: 30,
    email_change_per_hour: 3,
    invite_per_hour: 5,
};

pub const STARTER_TIER: RateLimitTier = RateLimitTier {
    api_requests_per_min: 200,
    auth_attempts_per_5min: 15,
    auth_attempts_per_hour: 60,
    email_change_per_hour: 5,
    invite_per_hour: 20,
};

pub const PRO_TIER: RateLimitTier = RateLimitTier {
    api_requests_per_min: 1000,
    auth_attempts_per_5min: 30,
    auth_attempts_per_hour: 200,
    email_change_per_hour: 10,
    invite_per_hour: 100,
};

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
