// Auth Rate Limiter — sliding-window brute-force protection for auth endpoints.
// Separate from the token-bucket RateLimiter used for infrastructure protection.

use axum::{
    extract::Request,
    http::{header, StatusCode},
    response::{IntoResponse, Json, Response},
};
use dashmap::DashMap;
use std::sync::Arc;
use std::time::Instant;

#[derive(Clone)]
pub struct AuthRateLimiter {
    windows: Arc<DashMap<String, (u32, Instant)>>,
}

impl AuthRateLimiter {
    pub fn new() -> Self {
        Self {
            windows: Arc::new(DashMap::new()),
        }
    }

    /// Check if action is allowed. Returns `Ok(())` or `Err(retry_after_seconds)`.
    pub fn check(&self, key: &str, max: u32, window_secs: u64) -> Result<(), u64> {
        let now = Instant::now();
        let mut entry = self
            .windows
            .entry(key.to_string())
            .or_insert_with(|| (0, now));

        let elapsed = now.duration_since(entry.1).as_secs();
        if elapsed >= window_secs {
            // Window expired — reset
            *entry = (1, now);
            return Ok(());
        }

        entry.0 += 1;
        if entry.0 > max {
            let remaining = window_secs - elapsed;
            return Err(remaining);
        }
        Ok(())
    }
}

/// Extract client IP from X-Forwarded-For, X-Real-IP, or fallback.
pub fn extract_client_ip(req: &Request) -> String {
    if let Some(forwarded) = req.headers().get(header::FORWARDED) {
        if let Ok(val) = forwarded.to_str() {
            // Forwarded: for=192.0.2.1;proto=https
            for part in val.split(';') {
                let part = part.trim();
                if let Some(ip) = part.strip_prefix("for=") {
                    let ip = ip.trim();
                    // Strip quoted values and optional port
                    let ip = ip.trim_matches('"').split(':').next().unwrap_or(ip);
                    return ip.to_string();
                }
            }
        }
    }
    if let Some(xff) = req.headers().get("x-forwarded-for") {
        if let Ok(val) = xff.to_str() {
            // Take the first IP in the chain (original client)
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

/// Helper to build a 429 response.
pub fn too_many_requests(retry_after: u64) -> Response {
    (
        StatusCode::TOO_MANY_REQUESTS,
        [(header::RETRY_AFTER, retry_after.to_string())],
        Json(serde_json::json!({
            "error": "too_many_attempts",
            "retry_after": retry_after
        })),
    )
        .into_response()
}

/// Result of checking multiple rate limit rules.
pub struct RateLimitResult {
    pub retry_after: u64,
}

impl AuthRateLimiter {
    /// Check multiple rate-limit rules; return the worst (longest) retry_after if any fails.
    pub fn check_multi<const N: usize>(
        &self,
        checks: [(&str, u32, u64); N],
    ) -> Result<(), RateLimitResult> {
        let mut worst = 0u64;
        for (key, max, window) in checks {
            if let Err(secs) = self.check(key, max, window) {
                worst = worst.max(secs);
            }
        }
        if worst > 0 {
            Err(RateLimitResult { retry_after: worst })
        } else {
            Ok(())
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_allows_under_limit() {
        let rl = AuthRateLimiter::new();
        for _ in 0..5 {
            assert!(rl.check("email@test.com", 5, 300).is_ok());
        }
    }

    #[test]
    fn test_blocks_over_limit() {
        let rl = AuthRateLimiter::new();
        for _ in 0..5 {
            assert!(rl.check("k", 5, 300).is_ok());
        }
        let err = rl.check("k", 5, 300).unwrap_err();
        assert!(err <= 300);
    }

    #[test]
    fn test_separate_keys() {
        let rl = AuthRateLimiter::new();
        for _ in 0..5 {
            rl.check("a", 1, 300).ok();
        }
        assert!(rl.check("b", 1, 300).is_ok());
    }
}
