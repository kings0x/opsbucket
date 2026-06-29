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

impl RateLimiter {
    pub fn new(capacity: u64, refill_per_min: u64) -> Self {
        Self {
            buckets: HashMap::new(),
            capacity,
            refill_per_sec: refill_per_min as f64 / 60.0,
            check_count: 0,
        }
    }

    pub fn check(&mut self, key: &str) -> bool {
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
            true
        } else {
            false
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
            assert!(limiter.check("key-a"));
        }
    }

    #[test]
    fn blocks_requests_at_limit() {
        let mut limiter = RateLimiter::new(5, 5);
        for _ in 0..5 {
            assert!(limiter.check("key-a"));
        }
        assert!(!limiter.check("key-a"));
    }

    #[test]
    fn resets_after_refill_window() {
        let mut limiter = RateLimiter::new(5, 300);
        for _ in 0..5 {
            assert!(limiter.check("key-a"));
        }
        assert!(!limiter.check("key-a"));

        std::thread::sleep(Duration::from_millis(1100));

        assert!(limiter.check("key-a"));
    }

    #[test]
    fn multiple_keys_are_independent() {
        let mut limiter = RateLimiter::new(3, 3);

        assert!(limiter.check("key-a"));
        assert!(limiter.check("key-a"));
        assert!(limiter.check("key-a"));
        assert!(!limiter.check("key-a"));

        assert!(limiter.check("key-b"));
        assert!(limiter.check("key-b"));
        assert!(limiter.check("key-b"));
        assert!(!limiter.check("key-b"));
    }

    #[test]
    fn evicts_stale_keys() {
        let mut limiter = RateLimiter::new(5, 5);

        assert!(limiter.check("stale-key"));

        let bucket = limiter.buckets.get_mut("stale-key").unwrap();
        bucket.last_refill = Instant::now() - Duration::from_secs(700);

        limiter.check_count = 99;
        assert!(limiter.check("another-key"));

        assert!(!limiter.buckets.contains_key("stale-key"));
    }
}
