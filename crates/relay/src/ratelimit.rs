// Rate Limiter — token bucket per client/agent
// Port of internal/relay/middleware.go
//
// Uniform rate limiting for infrastructure protection (not plan-gated).
// All clients share the same limits; per-key buckets prevent cross-contamination.

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
}

pub struct RateLimiter {
    buckets: Arc<DashMap<String, Bucket>>,
    max_tokens: f64,
    refill_rate: f64,
}

impl RateLimiter {
    pub fn new(max_requests: u32, per_seconds: u32) -> Self {
        Self {
            buckets: Arc::new(DashMap::new()),
            max_tokens: max_requests as f64,
            refill_rate: max_requests as f64 / per_seconds as f64,
        }
    }

    /// Check if a key is allowed to proceed.
    pub fn allow(&self, key: &str) -> bool {
        let mut bucket = self.buckets
            .entry(key.to_string())
            .or_insert_with(|| Bucket::new(self.max_tokens, self.refill_rate));
        bucket.try_consume(1.0)
    }

    /// Remove stale buckets not accessed in the last hour.
    pub fn prune(&self) {
        let cutoff = std::time::Instant::now() - std::time::Duration::from_secs(3600);
        self.buckets.retain(|_, b| b.last_refill > cutoff);
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
        // Different key works independently
        assert!(rl.allow("other"));
    }
}
