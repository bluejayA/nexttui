use std::time::Duration;

pub const NORMAL_INTERVAL: Duration = Duration::from_secs(30);
pub const FAST_INTERVAL: Duration = Duration::from_secs(10);
pub const MAX_BACKOFF_MULTIPLIER: u32 = 8;

pub struct RefreshScheduler {
    tick_count: u64,
    normal_interval: Duration,
    fast_interval: Duration,
    is_fast: bool,
    backoff_multiplier: u32,
    tick_rate: Duration,
}

impl RefreshScheduler {
    pub fn new(tick_rate: Duration) -> Self {
        Self {
            tick_count: 0,
            normal_interval: NORMAL_INTERVAL,
            fast_interval: FAST_INTERVAL,
            is_fast: false,
            backoff_multiplier: 1,
            tick_rate,
        }
    }

    pub fn tick(&mut self) -> bool {
        self.tick_count += 1;
        let interval = if self.is_fast {
            self.fast_interval
        } else {
            self.normal_interval
        };
        let effective = interval * self.backoff_multiplier;
        let target_ticks = effective.as_millis() / self.tick_rate.as_millis();
        if self.tick_count >= target_ticks as u64 {
            self.tick_count = 0;
            true
        } else {
            false
        }
    }

    pub fn set_fast(&mut self, fast: bool) {
        self.is_fast = fast;
    }

    pub fn reset(&mut self) {
        self.tick_count = 0;
    }

    pub fn backoff(&mut self) {
        if self.backoff_multiplier < MAX_BACKOFF_MULTIPLIER {
            self.backoff_multiplier *= 2;
        }
    }

    pub fn reset_backoff(&mut self) {
        self.backoff_multiplier = 1;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn scheduler() -> RefreshScheduler {
        RefreshScheduler {
            tick_count: 0,
            normal_interval: NORMAL_INTERVAL,
            fast_interval: FAST_INTERVAL,
            is_fast: false,
            backoff_multiplier: 1,
            tick_rate: Duration::from_millis(200),
        }
    }

    #[test]
    fn test_refresh_scheduler_tick_normal_interval() {
        let mut s = scheduler();
        // 30s / 200ms = 150 ticks
        for _ in 0..149 {
            assert!(!s.tick());
        }
        assert!(s.tick()); // 150th tick
    }

    #[test]
    fn test_refresh_scheduler_fast_interval() {
        let mut s = scheduler();
        s.set_fast(true);
        // 10s / 200ms = 50 ticks
        for _ in 0..49 {
            assert!(!s.tick());
        }
        assert!(s.tick()); // 50th tick
    }

    #[test]
    fn test_refresh_scheduler_reset() {
        let mut s = scheduler();
        for _ in 0..100 {
            s.tick();
        }
        s.reset();
        // After reset, should need full interval again
        for _ in 0..149 {
            assert!(!s.tick());
        }
        assert!(s.tick());
    }

    #[test]
    fn test_refresh_scheduler_backoff() {
        let mut s = scheduler();
        s.backoff(); // 2x
        // 30s * 2 / 200ms = 300 ticks
        for _ in 0..299 {
            assert!(!s.tick());
        }
        assert!(s.tick());

        s.backoff(); // 4x
        s.backoff(); // 8x (max)
        s.backoff(); // still 8x (capped)
        // 30s * 8 / 200ms = 1200 ticks
        for _ in 0..1199 {
            assert!(!s.tick());
        }
        assert!(s.tick());
    }

    #[test]
    fn test_refresh_scheduler_reset_backoff() {
        let mut s = scheduler();
        s.backoff(); // 2x
        s.reset_backoff(); // back to 1x
        // 30s / 200ms = 150 ticks
        for _ in 0..149 {
            assert!(!s.tick());
        }
        assert!(s.tick());
    }
}
