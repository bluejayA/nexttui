//! Per-epoch cancellation registry used to abort all in-flight work belonging
//! to a previous context generation. The switcher calls `cancel_below` after
//! bumping the epoch so any spawn started under the old epoch terminates
//! before the new context emits its first event.
//!
//! BL-P2-031 Unit 2.

use std::sync::Mutex;

use tokio_util::sync::CancellationToken;

use super::epoch::Epoch;

#[derive(Default)]
pub struct CancellationRegistry {
    entries: Mutex<Vec<RegistryEntry>>,
}

struct RegistryEntry {
    epoch: Epoch,
    token: CancellationToken,
}

impl CancellationRegistry {
    pub fn new() -> Self {
        Self { entries: Mutex::new(Vec::new()) }
    }

    /// Register a fresh cancellation token under `epoch` and return it.
    /// The caller passes this token into the spawned future via
    /// `tokio::select!` so that `cancel_below` can terminate it.
    pub fn register(&self, epoch: Epoch) -> CancellationToken {
        let token = CancellationToken::new();
        // Drop already-cancelled entries opportunistically to bound memory.
        let mut entries = self.entries.lock().unwrap_or_else(|e| e.into_inner());
        entries.retain(|e| !e.token.is_cancelled());
        entries.push(RegistryEntry { epoch, token: token.clone() });
        token
    }

    /// Cancel every registered token whose epoch is strictly less than
    /// `threshold`. Returns the number of tokens cancelled. Idempotent —
    /// subsequent calls with the same threshold are no-ops.
    pub fn cancel_below(&self, threshold: Epoch) -> usize {
        let mut entries = self.entries.lock().unwrap_or_else(|e| e.into_inner());
        let mut cancelled = 0;
        entries.retain(|e| {
            if e.epoch < threshold {
                if !e.token.is_cancelled() {
                    e.token.cancel();
                    cancelled += 1;
                }
                false // drop from registry once cancelled
            } else {
                true
            }
        });
        cancelled
    }

    /// Number of live (non-cancelled) tokens currently tracked. Intended for
    /// observability/tests; production code should rely on cancellation
    /// semantics rather than counting.
    pub fn active_count(&self) -> usize {
        let mut entries = self.entries.lock().unwrap_or_else(|e| e.into_inner());
        entries.retain(|e| !e.token.is_cancelled());
        entries.len()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn register_returns_distinct_uncancelled_tokens() {
        let reg = CancellationRegistry::new();
        let a = reg.register(1);
        let b = reg.register(1);
        assert!(!a.is_cancelled());
        assert!(!b.is_cancelled());
        assert_eq!(reg.active_count(), 2);
    }

    #[test]
    fn cancel_below_cancels_only_older_epochs() {
        let reg = CancellationRegistry::new();
        let old_a = reg.register(1);
        let old_b = reg.register(2);
        let new = reg.register(5);

        let cancelled = reg.cancel_below(5);
        assert_eq!(cancelled, 2);
        assert!(old_a.is_cancelled());
        assert!(old_b.is_cancelled());
        assert!(!new.is_cancelled());
        assert_eq!(reg.active_count(), 1);
    }

    #[test]
    fn cancel_below_is_idempotent() {
        let reg = CancellationRegistry::new();
        let _ = reg.register(1);
        let _ = reg.register(2);

        let first = reg.cancel_below(3);
        let second = reg.cancel_below(3);
        assert_eq!(first, 2);
        assert_eq!(second, 0);
    }

    #[test]
    fn cancel_below_threshold_zero_cancels_nothing() {
        let reg = CancellationRegistry::new();
        let token = reg.register(0);
        let cancelled = reg.cancel_below(0);
        assert_eq!(cancelled, 0);
        assert!(!token.is_cancelled());
    }

    #[tokio::test]
    async fn cancelled_token_terminates_select_branch() {
        use tokio::time::{Duration, sleep};

        let reg = CancellationRegistry::new();
        let token = reg.register(1);
        let task = tokio::spawn(async move {
            tokio::select! {
                _ = token.cancelled() => "cancelled",
                _ = sleep(Duration::from_secs(5)) => "timeout",
            }
        });

        // Give the task a tick to enter select.
        tokio::task::yield_now().await;
        reg.cancel_below(2);
        let outcome = task.await.unwrap();
        assert_eq!(outcome, "cancelled");
    }

    #[test]
    fn cancelled_entries_are_garbage_collected_on_register() {
        let reg = CancellationRegistry::new();
        let _ = reg.register(1);
        let _ = reg.register(2);
        reg.cancel_below(3);
        assert_eq!(reg.active_count(), 0);
        // New registration should not retain the old entries.
        let fresh = reg.register(10);
        assert_eq!(reg.active_count(), 1);
        assert!(!fresh.is_cancelled());
    }
}
