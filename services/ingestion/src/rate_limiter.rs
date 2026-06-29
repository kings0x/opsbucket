pub struct RateLimiter;

impl RateLimiter {
    pub fn new(_capacity: u64, _refill_per_min: u64) -> Self {
        Self
    }

    pub fn check(&mut self, _key: &str) -> bool {
        true
    }
}
