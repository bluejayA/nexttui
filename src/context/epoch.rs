//! Monotonic epoch counter that is the single authority for the active
//! context generation. Every spawned async task and every event is expected
//! to carry an epoch; the dispatcher drops anything older than the current
//! value.

use std::sync::atomic::{AtomicU64, Ordering};

pub type Epoch = u64;

#[derive(Debug, Default)]
pub struct ContextEpoch {
    value: AtomicU64,
}

impl ContextEpoch {
    pub fn new() -> Self {
        Self { value: AtomicU64::new(0) }
    }

    pub fn current(&self) -> Epoch {
        self.value.load(Ordering::Acquire)
    }

    /// Increment and return the new value. Monotonic across threads.
    pub fn bump(&self) -> Epoch {
        self.value.fetch_add(1, Ordering::AcqRel) + 1
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Arc;

    #[test]
    fn new_starts_at_zero() {
        let e = ContextEpoch::new();
        assert_eq!(e.current(), 0);
    }

    #[test]
    fn bump_returns_new_monotonic_value() {
        let e = ContextEpoch::new();
        assert_eq!(e.bump(), 1);
        assert_eq!(e.bump(), 2);
        assert_eq!(e.bump(), 3);
        assert_eq!(e.current(), 3);
    }

    #[test]
    fn concurrent_bumps_produce_unique_values() {
        let e = Arc::new(ContextEpoch::new());
        let mut handles = Vec::new();
        for _ in 0..8 {
            let e = e.clone();
            handles.push(std::thread::spawn(move || {
                (0..100).map(|_| e.bump()).collect::<Vec<_>>()
            }));
        }
        let mut all: Vec<Epoch> = handles.into_iter().flat_map(|h| h.join().unwrap()).collect();
        all.sort_unstable();
        all.dedup();
        assert_eq!(all.len(), 800, "all bump values should be unique");
        assert_eq!(*all.first().unwrap(), 1);
        assert_eq!(*all.last().unwrap(), 800);
    }
}
