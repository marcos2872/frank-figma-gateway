//! Backoff and restart policy for sidecars.

use std::time::Duration;

#[derive(Debug, Clone)]
pub struct Backoff {
    pub initial: Duration,
    pub max: Duration,
    pub factor: u32,
}

impl Default for Backoff {
    fn default() -> Self {
        Self {
            initial: Duration::from_millis(500),
            max: Duration::from_secs(30),
            factor: 2,
        }
    }
}

impl Backoff {
    /// `attempt` is 1-based.
    pub fn delay(&self, attempt: u32) -> Duration {
        if attempt == 0 {
            return Duration::ZERO;
        }
        let exponent = attempt.saturating_sub(1).min(20);
        let factor = self.factor.max(1).saturating_pow(exponent);
        self.initial.saturating_mul(factor).min(self.max)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn grows_exponentially_and_caps() {
        let backoff = Backoff {
            initial: Duration::from_millis(100),
            max: Duration::from_millis(500),
            factor: 2,
        };
        assert_eq!(backoff.delay(0), Duration::ZERO);
        assert_eq!(backoff.delay(1), Duration::from_millis(100));
        assert_eq!(backoff.delay(2), Duration::from_millis(200));
        assert_eq!(backoff.delay(3), Duration::from_millis(400));
        assert_eq!(backoff.delay(4), Duration::from_millis(500));
        assert_eq!(backoff.delay(30), Duration::from_millis(500));
    }
}
