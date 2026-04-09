// Rate Limiter — token bucket per client/agent
// Port of internal/relay/middleware.go
//
// Supports plan-aware rate limiting: each key can have its own RPM limit
// based on the billing plan of the authenticated client.

use dashmap::DashMap;
use std::sync::Arc;
use std::time::Instant;

struct Bucket {
    tokens: f64,
    max_tokens: f64,
    refill_rate: f64,
    last_refill: Instant,
}

impl Bucket {
    fn new(max_tokens: f64, refill_rate: f64) -> Self {
        Self { tokens: max_tokens, max_tokens, refill_rate, last_refill: Instant::now() }
    }

    fn try_consume(&mut self, cost: f64) -> bool {
        let now = Instant::now();
        let elapsed = (now - self.last_refill).as_secs_f64();
        self.tokens = (self.tokens + elapsed * self.refill_rate).min(self.max_tokens);
        self.last_refill = now;
        if self.tokens >= cost {
            self.tokens -= cost;
            true
        } else {
            false
        }
    }

    /// Update bucket limits if plan changed. Tokens reset to new max on upgrade.
    fn update_limits(&mut self, max_tokens: f64, refill_rate: f64) {
        // On upgrade: grant full new bucket. On downgrade: clamp to new max.
        if max_tokens > self.max_tokens {
            self.tokens = max_tokens;
        } else {
            self.tokens = self.tokens.min(max_tokens);
        }
        self.max_tokens = max_tokens;
        self.refill_rate = refill_rate;
    }
}

pub struct RateLimiter {
    buckets: Arc<DashMap<String, Bucket>>,
    /// Default limits for unauthenticated / unknown clients
    default_max_tokens: f64,
    default_refill_rate: f64,
}

impl RateLimiter {
    pub fn new(max_requests: u32, per_seconds: u32) -> Self {
        Self {
            buckets: Arc::new(DashMap::new()),
            default_max_tokens: max_requests as f64,
            default_refill_rate: max_requests as f64 / per_seconds as f64,
        }
    }

    /// Check if a key is allowed to proceed (uses default limits).
    pub fn allow(&self, key: &str) -> bool {
        let mut bucket = self.buckets
            .entry(key.to_string())
            .or_insert_with(|| Bucket::new(self.default_max_tokens, self.default_refill_rate));
        bucket.try_consume(1.0)
    }

    /// Check if a key is allowed with plan-specific RPM limit.
    /// `rpm` = requests per minute. 0 means unlimited (always allow).
    /// Bucket limits are updated if the plan changed since last request.
    pub fn allow_plan(&self, key: &str, rpm: u32) -> bool {
        if rpm == 0 {
            return true; // unlimited
        }
        let max_tokens = rpm as f64;
        let refill_rate = rpm as f64 / 60.0; // per second
        let mut bucket = self.buckets
            .entry(key.to_string())
            .or_insert_with(|| Bucket::new(max_tokens, refill_rate));
        // Update if plan changed (different limits)
        if (bucket.max_tokens - max_tokens).abs() > 0.01 {
            bucket.update_limits(max_tokens, refill_rate);
        }
        bucket.try_consume(1.0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_allow_under_limit() {
        let rl = RateLimiter::new(5, 1);
        for _ in 0..5 {
            assert!(rl.allow("k"));
        }
    }

    #[test]
    fn test_block_over_limit() {
        let rl = RateLimiter::new(2, 1);
        assert!(rl.allow("k"));
        assert!(rl.allow("k"));
        assert!(!rl.allow("k"));
    }

    #[test]
    fn test_separate_keys() {
        let rl = RateLimiter::new(1, 1);
        assert!(rl.allow("a"));
        assert!(!rl.allow("a"));
        assert!(rl.allow("b")); // different key
    }

    #[test]
    fn test_refill_after_time() {
        let rl = RateLimiter::new(1, 1); // 1 req per 1 second
        assert!(rl.allow("k"));
        assert!(!rl.allow("k"));
        assert!(rl.allow("other"));
    }

    #[test]
    fn test_allow_plan_basic() {
        let rl = RateLimiter::new(100, 10); // default
        // Plan limit: 3 rpm
        for _ in 0..3 {
            assert!(rl.allow_plan("p", 3));
        }
        assert!(!rl.allow_plan("p", 3));
    }

    #[test]
    fn test_allow_plan_unlimited() {
        let rl = RateLimiter::new(1, 10); // very strict default
        // rpm=0 = unlimited
        for _ in 0..100 {
            assert!(rl.allow_plan("unlimited", 0));
        }
    }

    #[test]
    fn test_allow_plan_upgrade() {
        let rl = RateLimiter::new(2, 1);
        // Start with 2 rpm
        assert!(rl.allow_plan("up", 2));
        assert!(rl.allow_plan("up", 2));
        assert!(!rl.allow_plan("up", 2));
        // Upgrade to 5 rpm — bucket resets
        assert!(rl.allow_plan("up", 5));
        assert!(rl.allow_plan("up", 5));
        assert!(rl.allow_plan("up", 5));
    }

    #[test]
    fn test_allow_plan_separate_keys() {
        let rl = RateLimiter::new(100, 10);
        assert!(rl.allow_plan("trial", 1));
        assert!(!rl.allow_plan("trial", 1));
        assert!(rl.allow_plan("pro", 10)); // different key, own bucket
    }
}
