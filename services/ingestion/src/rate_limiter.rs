use std::collections::HashMap;
use std::time::{Duration, Instant};

struct TokenBucket {
    tokens: f64,
    last_refill: Instant,
}

pub struct RateLimiter {
    buckets: HashMap<String, TokenBucket>,
    capacity: u64,
    refill_per_sec: f64,
    check_count: u64,
}

pub struct RateLimitResult {
    pub allowed: bool,
    pub retry_after_seconds: u64,
}

impl RateLimiter {
    pub fn new(capacity: u64, refill_per_min: u64) -> Self {
        Self {
            buckets: HashMap::new(),
            capacity,
            refill_per_sec: refill_per_min as f64 / 60.0,
            check_count: 0,
        }
    }

    pub fn check(&mut self, key: &str) -> RateLimitResult {
        self.check_count += 1;

        if self.check_count.is_multiple_of(100) {
            self.evict_stale();
        }

        let now = Instant::now();
        let bucket = self
            .buckets
            .entry(key.to_string())
            .or_insert_with(|| TokenBucket {
                tokens: self.capacity as f64,
                last_refill: Instant::now(),
            });

        let elapsed = now.duration_since(bucket.last_refill).as_secs_f64();
        bucket.tokens = (bucket.tokens + elapsed * self.refill_per_sec).min(self.capacity as f64);
        bucket.last_refill = now;

        if bucket.tokens >= 1.0 {
            bucket.tokens -= 1.0;
            RateLimitResult {
                allowed: true,
                retry_after_seconds: 0,
            }
        } else {
            let retry = ((1.0 - bucket.tokens) / self.refill_per_sec).ceil() as u64;
            RateLimitResult {
                allowed: false,
                retry_after_seconds: retry.max(1),
            }
        }
    }

    fn evict_stale(&mut self) {
        let cutoff = Instant::now() - Duration::from_secs(600);
        self.buckets.retain(|_, b| b.last_refill > cutoff);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn allows_requests_under_limit() {
        let mut limiter = RateLimiter::new(10, 10);
        for _ in 0..10 {
            let result = limiter.check("key-a");
            assert!(result.allowed);
            assert_eq!(result.retry_after_seconds, 0);
        }
    }

    #[test]
    fn blocks_requests_at_limit() {
        let mut limiter = RateLimiter::new(5, 5);
        for _ in 0..5 {
            let result = limiter.check("key-a");
            assert!(result.allowed);
        }
        let result = limiter.check("key-a");
        assert!(!result.allowed);
        assert!(result.retry_after_seconds >= 1);
    }

    #[test]
    fn resets_after_refill_window() {
        let mut limiter = RateLimiter::new(5, 300);
        for _ in 0..5 {
            assert!(limiter.check("key-a").allowed);
        }
        assert!(!limiter.check("key-a").allowed);

        std::thread::sleep(Duration::from_millis(1100));

        assert!(limiter.check("key-a").allowed);
    }

    #[test]
    fn multiple_keys_are_independent() {
        let mut limiter = RateLimiter::new(3, 3);

        assert!(limiter.check("key-a").allowed);
        assert!(limiter.check("key-a").allowed);
        assert!(limiter.check("key-a").allowed);
        assert!(!limiter.check("key-a").allowed);

        assert!(limiter.check("key-b").allowed);
        assert!(limiter.check("key-b").allowed);
        assert!(limiter.check("key-b").allowed);
        assert!(!limiter.check("key-b").allowed);
    }

    #[test]
    fn evicts_stale_keys() {
        let mut limiter = RateLimiter::new(5, 5);

        assert!(limiter.check("stale-key").allowed);

        let bucket = limiter.buckets.get_mut("stale-key").unwrap();
        bucket.last_refill = Instant::now() - Duration::from_secs(700);

        limiter.check_count = 99;
        assert!(limiter.check("another-key").allowed);

        assert!(!limiter.buckets.contains_key("stale-key"));
    }
}
