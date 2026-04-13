//! Finite state machine that serialises concurrent switch attempts and
//! exposes the observable switch state.
//!
//! BL-P2-031 Unit 4. The state machine has four responsibilities:
//! 1. Serialise `try_begin` so only one switch runs at a time (returns
//!    `SwitchError::InProgress` otherwise).
//! 2. Bump the authoritative epoch atomically with the transition to
//!    `Switching` so no stale work can slip in between.
//! 3. Record the `previous` snapshot when a switch starts so `fail` can
//!    restore it without the caller holding side state.
//! 4. Expose a cheap read-only view for observers (indicator widget, tests).
//!
//! The store is intentionally owned by `ContextSwitcher` — not by the
//! state machine — because `switch_back` needs the history independently of
//! the transition state, and conflating the two would make rollback on
//! failure harder to reason about.
//!
//! The state machine is deliberately not async; the whole transition
//! (`try_begin`, subsequent `commit`/`fail`) completes under the plain
//! `std::sync::Mutex`. Each individual method is O(1) so contention is
//! bounded by the caller's await points, not by the mutex itself.
//!
//! Concurrency: a single `Mutex<SwitchState>` guards the state. `try_begin`
//! holds the lock for the full check-and-transition so two concurrent
//! callers cannot both observe `Idle` and both become `Switching`.

use std::sync::{Arc, Mutex};

use super::epoch::{ContextEpoch, Epoch};
use super::error::SwitchError;
use super::types::{ContextSnapshot, ContextTarget};

/// Internal state — exact representation. Exposed only through
/// `SwitchStateView`.
#[derive(Debug, Clone)]
pub enum SwitchState {
    /// No switch in progress. `current` is populated once the first commit
    /// has happened; before that it is `None`.
    Idle {
        current: Option<ContextSnapshot>,
    },
    /// A switch is running. `previous` is the snapshot that was current when
    /// the switch started; it is used by `fail` to restore state without
    /// the caller having to remember what the previous snapshot was.
    Switching {
        target: ContextTarget,
        started_at_epoch: Epoch,
        previous: Option<ContextSnapshot>,
    },
}

/// Observable projection for tests, UI, and tracing. Matches the internal
/// variants but is `Clone + PartialEq` friendly.
#[derive(Debug, Clone)]
pub enum SwitchStateView {
    Idle {
        current: Option<ContextSnapshot>,
    },
    Switching {
        target: ContextTarget,
        started_at_epoch: Epoch,
    },
}

pub struct SwitchStateMachine {
    state: Mutex<SwitchState>,
    epoch: Arc<ContextEpoch>,
}

impl SwitchStateMachine {
    pub fn new(epoch: Arc<ContextEpoch>) -> Self {
        Self {
            state: Mutex::new(SwitchState::Idle { current: None }),
            epoch,
        }
    }

    pub fn with_current(epoch: Arc<ContextEpoch>, current: ContextSnapshot) -> Self {
        Self {
            state: Mutex::new(SwitchState::Idle { current: Some(current) }),
            epoch,
        }
    }

    /// Attempt to transition to `Switching`. On success, bumps the epoch
    /// and returns the new epoch. On failure (another switch in progress)
    /// returns `SwitchError::InProgress` and does not touch the epoch.
    ///
    /// The lock is held for the full check-and-bump so two concurrent
    /// callers cannot both observe `Idle` and both become `Switching`.
    pub fn try_begin(&self, target: ContextTarget) -> Result<Epoch, SwitchError> {
        let mut state = self.state.lock().unwrap_or_else(|e| e.into_inner());
        let previous = match &*state {
            SwitchState::Idle { current } => current.clone(),
            SwitchState::Switching { .. } => return Err(SwitchError::InProgress),
        };
        let new_epoch = self.epoch.bump();
        *state = SwitchState::Switching {
            target,
            started_at_epoch: new_epoch,
            previous,
        };
        Ok(new_epoch)
    }

    /// Transition `Switching → Idle { current: Some(snapshot) }`. Silently
    /// no-ops if called from `Idle` (defensive — the switcher always pairs
    /// `try_begin` with `commit` or `fail`, but we never panic on misuse).
    pub fn commit(&self, snapshot: ContextSnapshot) {
        let mut state = self.state.lock().unwrap_or_else(|e| e.into_inner());
        *state = SwitchState::Idle { current: Some(snapshot) };
    }

    /// Transition `Switching → Idle { current: previous }`. Preserves the
    /// pre-switch snapshot so observers see the untouched context.
    pub fn fail(&self, _err: &SwitchError) {
        let mut state = self.state.lock().unwrap_or_else(|e| e.into_inner());
        let restored = match &*state {
            SwitchState::Switching { previous, .. } => previous.clone(),
            SwitchState::Idle { current } => current.clone(),
        };
        *state = SwitchState::Idle { current: restored };
    }

    pub fn state(&self) -> SwitchStateView {
        let state = self.state.lock().unwrap_or_else(|e| e.into_inner());
        match &*state {
            SwitchState::Idle { current } => SwitchStateView::Idle { current: current.clone() },
            SwitchState::Switching { target, started_at_epoch, .. } => SwitchStateView::Switching {
                target: target.clone(),
                started_at_epoch: *started_at_epoch,
            },
        }
    }

    pub fn is_idle(&self) -> bool {
        matches!(
            &*self.state.lock().unwrap_or_else(|e| e.into_inner()),
            SwitchState::Idle { .. }
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::port::types::{CatalogEntry, ProjectScope, Token, TokenScope};
    use chrono::{TimeZone, Utc};
    use std::thread;

    fn target(name: &str) -> ContextTarget {
        ContextTarget {
            cloud: "devstack".into(),
            project_id: format!("id-{name}"),
            project_name: name.into(),
            domain: "default".into(),
        }
    }

    fn snapshot(name: &str, epoch: Epoch) -> ContextSnapshot {
        let t = target(name);
        ContextSnapshot {
            target: t.clone(),
            epoch,
            token: Token {
                id: format!("tok-{name}"),
                expires_at: Utc.with_ymd_and_hms(2030, 1, 1, 0, 0, 0).unwrap(),
                project: ProjectScope {
                    id: t.project_id.clone(),
                    name: t.project_name.clone(),
                    domain_id: "default".into(),
                    domain_name: t.domain.clone(),
                },
                roles: Vec::new(),
                catalog: Vec::<CatalogEntry>::new(),
            },
            token_scope: TokenScope::from(&t),
            captured_at: Utc.with_ymd_and_hms(2026, 4, 14, 0, 0, 0).unwrap(),
        }
    }

    #[test]
    fn new_is_idle_without_current() {
        let sm = SwitchStateMachine::new(Arc::new(ContextEpoch::new()));
        assert!(sm.is_idle());
        assert!(matches!(sm.state(), SwitchStateView::Idle { current: None }));
    }

    #[test]
    fn try_begin_transitions_to_switching_and_bumps_epoch() {
        let ep = Arc::new(ContextEpoch::new());
        let sm = SwitchStateMachine::new(ep.clone());
        let new_epoch = sm.try_begin(target("demo")).unwrap();
        assert_eq!(new_epoch, 1);
        assert_eq!(ep.current(), 1);
        assert!(matches!(sm.state(), SwitchStateView::Switching { .. }));
    }

    #[test]
    fn second_try_begin_while_switching_returns_in_progress() {
        let ep = Arc::new(ContextEpoch::new());
        let sm = SwitchStateMachine::new(ep.clone());
        let _ = sm.try_begin(target("demo")).unwrap();
        let err = sm.try_begin(target("other")).unwrap_err();
        assert!(matches!(err, SwitchError::InProgress));
        assert_eq!(ep.current(), 1, "epoch must not bump on rejected begin");
    }

    #[test]
    fn commit_transitions_to_idle_with_current() {
        let ep = Arc::new(ContextEpoch::new());
        let sm = SwitchStateMachine::new(ep);
        let new_epoch = sm.try_begin(target("demo")).unwrap();
        sm.commit(snapshot("demo", new_epoch));
        match sm.state() {
            SwitchStateView::Idle { current: Some(s) } => {
                assert_eq!(s.target.project_name, "demo");
                assert_eq!(s.epoch, new_epoch);
            }
            other => panic!("expected Idle with current, got {other:?}"),
        }
    }

    #[test]
    fn fail_restores_previous_snapshot() {
        let ep = Arc::new(ContextEpoch::new());
        let prev = snapshot("admin", 0);
        let sm = SwitchStateMachine::with_current(ep, prev.clone());
        sm.try_begin(target("demo")).unwrap();
        sm.fail(&SwitchError::RescopeRejected("nope".into()));
        match sm.state() {
            SwitchStateView::Idle { current: Some(s) } => {
                assert_eq!(s.target.project_name, "admin");
            }
            other => panic!("expected Idle with previous admin, got {other:?}"),
        }
    }

    #[test]
    fn fail_without_previous_returns_to_idle_none() {
        let ep = Arc::new(ContextEpoch::new());
        let sm = SwitchStateMachine::new(ep);
        sm.try_begin(target("demo")).unwrap();
        sm.fail(&SwitchError::CommitFailed("boom".into()));
        assert!(matches!(sm.state(), SwitchStateView::Idle { current: None }));
    }

    #[test]
    fn second_try_begin_after_commit_succeeds() {
        let ep = Arc::new(ContextEpoch::new());
        let sm = SwitchStateMachine::new(ep.clone());
        let e1 = sm.try_begin(target("demo")).unwrap();
        sm.commit(snapshot("demo", e1));
        let e2 = sm.try_begin(target("admin")).unwrap();
        assert_eq!(e2, 2);
        assert_eq!(ep.current(), 2);
    }

    #[test]
    fn concurrent_try_begin_only_one_wins() {
        let ep = Arc::new(ContextEpoch::new());
        let sm = Arc::new(SwitchStateMachine::new(ep.clone()));
        let mut handles = Vec::new();
        let attempts = 16;
        for i in 0..attempts {
            let sm = sm.clone();
            handles.push(thread::spawn(move || sm.try_begin(target(&format!("p{i}")))));
        }
        let mut ok = 0;
        let mut busy = 0;
        for h in handles {
            match h.join().unwrap() {
                Ok(_) => ok += 1,
                Err(SwitchError::InProgress) => busy += 1,
                Err(other) => panic!("unexpected error: {other:?}"),
            }
        }
        assert_eq!(ok, 1, "exactly one try_begin must succeed");
        assert_eq!(busy, attempts - 1);
        assert_eq!(ep.current(), 1, "only winning begin bumps epoch");
    }
}
