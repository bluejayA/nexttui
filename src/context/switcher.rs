//! Seven-step switch orchestrator.
//!
//! BL-P2-031 Unit 4. The switcher owns exactly four collaborators —
//! state machine, cancellation registry, resolver, session port — and
//! sequences them. Atomicity lives inside [`ContextSessionPort::commit`];
//! the switcher's job is to make sure the step order is correct and that
//! rollback runs on any failure before `commit`.
//!
//! Step order (from the design doc):
//! 1. Resolve user input → authoritative [`ContextTarget`].
//! 2. `SwitchStateMachine::try_begin` — bumps the epoch atomically and
//!    rejects if another switch is in progress.
//! 3. `CancellationRegistry::cancel_below(new_epoch)` — kill every
//!    in-flight spawn from the previous generation.
//! 4. `session.begin` — captures rollback data into a `SessionHandle`.
//! 5. `session.rescope` → `session.refresh_catalog` — staged work; any
//!    failure calls `session.rollback(handle)` before returning.
//! 6. `session.commit` — self-reverting by port contract; no manual
//!    rollback after commit.
//! 7. `state.commit(snapshot)` + history push + a final cancel sweep.
//!
//! Concurrency invariant: `try_begin` is the single serialisation point.
//! Two concurrent `switch` calls race into `try_begin`; the loser returns
//! `SwitchError::InProgress` without touching any ports.

use std::sync::Arc;

use std::sync::Mutex as SyncMutex;

use super::cancellation::CancellationRegistry;
use super::epoch::Epoch;
use super::error::SwitchError;
use super::history::ContextHistoryStore;
use super::resolver::ContextTargetResolver;
use super::state_machine::SwitchStateMachine;
use super::types::{ContextRequest, ContextSnapshot, ContextTarget, SessionHandle};
use crate::port::context_session::ContextSessionPort;

pub struct ContextSwitcher {
    state: Arc<SwitchStateMachine>,
    cancellation: Arc<CancellationRegistry>,
    resolver: Arc<ContextTargetResolver>,
    session: Arc<dyn ContextSessionPort>,
    history: Arc<SyncMutex<ContextHistoryStore>>,
}

impl ContextSwitcher {
    pub fn new(
        state: Arc<SwitchStateMachine>,
        cancellation: Arc<CancellationRegistry>,
        resolver: Arc<ContextTargetResolver>,
        session: Arc<dyn ContextSessionPort>,
        history: Arc<SyncMutex<ContextHistoryStore>>,
    ) -> Self {
        Self {
            state,
            cancellation,
            resolver,
            session,
            history,
        }
    }

    pub async fn switch(
        &self,
        request: ContextRequest,
    ) -> Result<(Epoch, ContextSnapshot), (Epoch, SwitchError)> {
        // Snapshot the epoch *before* the async resolve. If another switch
        // completes (and its state.commit() bumps the epoch) while we are
        // resolving, we detect the drift here and return `InProgress`
        // rather than fighting for `try_begin` against a now-idle machine.
        // (FR-4 entry-epoch gate — BL-P2-080 Unit 2.)
        let entry_epoch = self.state.epoch().current();

        // 1. Resolve user input → authoritative target. Resolver errors
        //    happen before we touch the state machine, so stamp with the
        //    currently-committed epoch — the error toast then survives
        //    the dispatcher gate.
        let target = match self.resolver.resolve(request).await {
            Ok(t) => t,
            Err(e) => return Err((self.state.epoch().current(), e)),
        };

        // Entry-epoch gate: if another switch committed during our resolve,
        // the epoch will have advanced. Bail out rather than risk running
        // the session side-effects with stale context.
        if self.state.epoch().current() != entry_epoch {
            tracing::warn!(
                entry_epoch,
                current = self.state.epoch().current(),
                reason = "resolve_epoch_drift",
                "switch_epoch_drift_during_resolve"
            );
            return Err((self.state.epoch().current(), SwitchError::InProgress));
        }
        // BL-P2-074 FR-4 / D1: if the resolved target matches the currently
        // committed context, short-circuit without touching the state
        // machine. Race note (TOCTOU): a concurrent switch that beats this
        // caller into `try_begin` will leave the machine non-Idle and this
        // fast path will be skipped — the caller then observes `InProgress`.
        // FR-4 acceptance is defined for sequential callers only.
        if let Some(snap) = self.state.committed_snapshot()
            && snap.target == target
        {
            tracing::debug!(
                cloud = %target.cloud,
                project = %target.project_name,
                "switch_noop_same_target"
            );
            return Ok((snap.epoch, snap));
        }
        self.run_switch_to(target).await
    }

    /// Replay the snapshot most recently pushed to history. `history` is
    /// peeked, not popped — the entry is only consumed by
    /// `run_switch_to`'s step 7, which replaces it with the pre-switch
    /// snapshot on success. A failed switch leaves history intact so the
    /// user can retry.
    pub async fn switch_back(&self) -> Result<(Epoch, ContextSnapshot), (Epoch, SwitchError)> {
        let previous = self
            .history
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .previous()
            .cloned();
        let previous = match previous {
            Some(p) => p,
            None => {
                return Err((
                    self.state.epoch().current(),
                    SwitchError::NotFound("no previous context".into()),
                ));
            }
        };
        self.run_switch_to(previous.target).await
    }

    async fn run_switch_to(
        &self,
        target: ContextTarget,
    ) -> Result<(Epoch, ContextSnapshot), (Epoch, SwitchError)> {
        // 2. State machine bumps the epoch atomically with Idle→Switching.
        //    If try_begin rejects (another switch in progress), the epoch
        //    hasn't moved — stamp with the currently-committed epoch.
        let new_epoch = match self.state.try_begin(target.clone()) {
            Ok(e) => e,
            Err(e) => return Err((self.state.epoch().current(), e)),
        };

        // 3. Kill every in-flight spawn from a previous generation.
        self.cancellation.cancel_below(new_epoch);

        // 4. Stage the transition; captures rollback data.
        let mut handle = match self.session.begin(&target, new_epoch).await {
            Ok(h) => h,
            Err(e) => {
                self.state.fail(&e);
                return Err((new_epoch, e));
            }
        };

        // 5. Network work — rescope + catalog refresh. Rollback on failure.
        if let Err(e) = self.run_transition(&mut handle).await {
            self.session.rollback(handle).await;
            self.state.fail(&e);
            return Err((new_epoch, e));
        }

        // 6. Commit is self-reverting by port contract.
        let snapshot = match self.session.commit(handle).await {
            Ok(s) => s,
            Err(e) => {
                self.state.fail(&e);
                return Err((new_epoch, e));
            }
        };

        // 7. Publish the new state; push the PRE-SWITCH snapshot into
        //    history so `switch_back` returns to what we came from (not
        //    what we just entered); belt-and-suspenders cancel sweep.
        //
        //    The cancel_below here is idempotent — step 3 already
        //    cancelled old epoch work, and every new spawn between
        //    steps 3 and 7 is stamped with `new_epoch` so it isn't
        //    caught by this call. Kept as a guard against future
        //    send-sites that might bypass `ActionSender`'s epoch
        //    stamping and spawn directly.
        let pre_switch = self.state.previous_in_flight();
        self.state.commit(snapshot.clone());
        if let Some(prev) = pre_switch {
            self.history
                .lock()
                .unwrap_or_else(|e| e.into_inner())
                .push(prev);
        } else {
            // First-ever switch (no prior context). Drop any existing
            // history entry so `switch_back` correctly reports "no
            // previous" rather than replaying a stale snapshot from a
            // crashed/earlier session.
            let _ = self
                .history
                .lock()
                .unwrap_or_else(|e| e.into_inner())
                .pop_previous();
        }
        self.cancellation.cancel_below(new_epoch);

        Ok((new_epoch, snapshot))
    }

    async fn run_transition(&self, handle: &mut SessionHandle) -> Result<(), SwitchError> {
        self.session.rescope(handle).await?;
        self.session.refresh_catalog(handle).await?;
        Ok(())
    }

    pub fn state(&self) -> Arc<SwitchStateMachine> {
        self.state.clone()
    }

    pub fn history(&self) -> Arc<SyncMutex<ContextHistoryStore>> {
        self.history.clone()
    }

    /// True when no switch is in flight. Used by the app's dispatcher
    /// to reject side-effecting actions mid-switch so the worker can't
    /// execute them with stale auth but a freshly-bumped epoch stamp.
    pub fn is_idle(&self) -> bool {
        self.state.is_idle()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::context::epoch::ContextEpoch;
    use crate::context::resolver::{
        CloudDirectory, ContextTargetResolver, ProjectCandidate, ProjectDirectoryPort,
    };
    use crate::port::mock_context::{MockContextSession, TransitionStep};
    use crate::port::types::{CatalogEntry, ProjectScope, Token, TokenScope};
    use async_trait::async_trait;
    use chrono::{TimeZone, Utc};
    use std::collections::HashMap;
    use std::sync::Mutex as StdMutex;

    struct FakeClouds;
    impl CloudDirectory for FakeClouds {
        fn active_cloud(&self) -> String {
            "devstack".into()
        }
        fn known_clouds(&self) -> Vec<String> {
            vec!["devstack".into()]
        }
        fn default_project(&self, _cloud: &str) -> Option<String> {
            None
        }
    }

    struct FakeDirectory {
        data: StdMutex<HashMap<String, Vec<ProjectCandidate>>>,
    }

    #[async_trait]
    impl ProjectDirectoryPort for FakeDirectory {
        async fn list_projects(&self, cloud: &str) -> Result<Vec<ProjectCandidate>, SwitchError> {
            Ok(self
                .data
                .lock()
                .unwrap()
                .get(cloud)
                .cloned()
                .unwrap_or_default())
        }
    }

    fn token(id: &str, project: &str) -> Token {
        Token {
            id: id.into(),
            expires_at: Utc.with_ymd_and_hms(2030, 1, 1, 0, 0, 0).unwrap(),
            project: ProjectScope {
                id: format!("id-{project}"),
                name: project.into(),
                domain_id: "default".into(),
                domain_name: "default".into(),
            },
            roles: Vec::new(),
            catalog: Vec::<CatalogEntry>::new(),
        }
    }

    fn scope(project: &str) -> TokenScope {
        TokenScope::Project {
            name: project.into(),
            domain: "default".into(),
        }
    }

    fn candidate(name: &str, id: &str) -> ProjectCandidate {
        ProjectCandidate {
            cloud: "devstack".into(),
            project_id: id.into(),
            project_name: name.into(),
            domain: "default".into(),
        }
    }

    struct Fixture {
        switcher: ContextSwitcher,
        session: Arc<MockContextSession>,
        epoch: Arc<ContextEpoch>,
        cancellation: Arc<CancellationRegistry>,
        state: Arc<SwitchStateMachine>,
        history: Arc<SyncMutex<ContextHistoryStore>>,
    }

    fn fixture_with(session: MockContextSession) -> Fixture {
        let epoch = Arc::new(ContextEpoch::new());
        let state = Arc::new(SwitchStateMachine::new(epoch.clone()));
        let cancellation = Arc::new(CancellationRegistry::new());
        let history = Arc::new(SyncMutex::new(ContextHistoryStore::new()));

        let clouds: Arc<dyn CloudDirectory> = Arc::new(FakeClouds);
        let directory: Arc<dyn ProjectDirectoryPort> = Arc::new(FakeDirectory {
            data: StdMutex::new(HashMap::from([(
                "devstack".into(),
                vec![candidate("demo", "id-demo"), candidate("admin", "id-admin")],
            )])),
        });
        let resolver = Arc::new(ContextTargetResolver::new(clouds, directory, None, None));

        let session = Arc::new(session);
        let switcher = ContextSwitcher::new(
            state.clone(),
            cancellation.clone(),
            resolver,
            session.clone() as Arc<dyn ContextSessionPort>,
            history.clone(),
        );
        Fixture {
            switcher,
            session,
            epoch,
            cancellation,
            state,
            history,
        }
    }

    fn default_fixture() -> Fixture {
        fixture_with(MockContextSession::new(
            scope("admin"),
            token("old", "admin"),
            token("new", "demo"),
        ))
    }

    fn by_name(project: &str) -> ContextRequest {
        ContextRequest::ByName {
            cloud: None,
            project: project.into(),
            domain: None,
        }
    }

    #[tokio::test]
    async fn first_switch_from_empty_commits_and_leaves_history_empty() {
        // With no prior committed context, the switcher has nothing to
        // push into "previous" — so history stays empty after the first
        // successful switch. `switch_back` will correctly report NotFound.
        let f = default_fixture();
        let (epoch, snapshot) = f.switcher.switch(by_name("demo")).await.unwrap();
        assert_eq!(epoch, 1);
        assert_eq!(snapshot.target.project_name, "demo");
        assert_eq!(
            f.session.transition_steps(),
            vec![
                TransitionStep::Begin,
                TransitionStep::Rescope,
                TransitionStep::Refresh,
                TransitionStep::Commit,
            ]
        );
        assert_eq!(f.epoch.current(), 1);
        assert!(f.history.lock().unwrap().previous().is_none());
        assert!(f.state.is_idle());
    }

    #[tokio::test]
    async fn second_switch_pushes_pre_switch_snapshot_to_history() {
        // After two switches (admin→demo→admin), history must hold the
        // *previous* context (demo) so `switch_back` returns there.
        let f = default_fixture();
        f.switcher.switch(by_name("demo")).await.unwrap();
        f.switcher.switch(by_name("admin")).await.unwrap();
        let previous = f
            .history
            .lock()
            .unwrap()
            .previous()
            .cloned()
            .expect("history populated after 2 switches");
        assert_eq!(previous.target.project_name, "demo");
    }

    #[tokio::test]
    async fn rescope_failure_rolls_back_and_resets_state() {
        let session =
            MockContextSession::new(scope("admin"), token("old", "admin"), token("new", "demo"))
                .with_rescope_failure(SwitchError::RescopeRejected("forbidden".into()));
        let f = fixture_with(session);

        let (err_epoch, err) = f.switcher.switch(by_name("demo")).await.unwrap_err();
        assert!(matches!(err, SwitchError::RescopeRejected(_)));
        assert_eq!(
            err_epoch, 1,
            "post-begin error stamps with the attempt's epoch"
        );
        assert_eq!(
            f.session.transition_steps(),
            vec![
                TransitionStep::Begin,
                TransitionStep::Rescope,
                TransitionStep::Rollback,
            ]
        );
        assert!(f.session.rollback_called());
        assert!(f.state.is_idle());
        assert!(f.history.lock().unwrap().previous().is_none());
    }

    #[tokio::test]
    async fn refresh_failure_rolls_back() {
        let session =
            MockContextSession::new(scope("admin"), token("old", "admin"), token("new", "demo"))
                .with_refresh_failure(SwitchError::CatalogFailed("timeout".into()));
        let f = fixture_with(session);

        let (_, err) = f.switcher.switch(by_name("demo")).await.unwrap_err();
        assert!(matches!(err, SwitchError::CatalogFailed(_)));
        assert!(f.session.rollback_called());
        assert_eq!(
            f.session.transition_steps(),
            vec![
                TransitionStep::Begin,
                TransitionStep::Rescope,
                TransitionStep::Refresh,
                TransitionStep::Rollback,
            ]
        );
    }

    #[tokio::test]
    async fn begin_failure_does_not_call_rollback() {
        let session =
            MockContextSession::new(scope("admin"), token("old", "admin"), token("new", "demo"))
                .with_begin_failure(SwitchError::Unsupported("bad cloud".into()));
        let f = fixture_with(session);

        let (_, err) = f.switcher.switch(by_name("demo")).await.unwrap_err();
        assert!(matches!(err, SwitchError::Unsupported(_)));
        assert!(!f.session.rollback_called());
        assert!(f.state.is_idle());
    }

    #[tokio::test]
    async fn commit_self_revert_surfaces_commit_failed() {
        let session =
            MockContextSession::new(scope("admin"), token("old", "admin"), token("new", "demo"))
                .with_partial_commit_failure();
        let f = fixture_with(session);

        let (_, err) = f.switcher.switch(by_name("demo")).await.unwrap_err();
        assert!(matches!(err, SwitchError::CommitFailed(_)));
        // commit is self-reverting; the switcher must NOT invoke rollback again.
        let steps = f.session.transition_steps();
        let rollback_count = steps
            .iter()
            .filter(|s| matches!(s, TransitionStep::Rollback))
            .count();
        assert_eq!(
            rollback_count, 1,
            "only commit's internal rollback, not a caller rollback"
        );
        assert!(f.state.is_idle());
    }

    #[tokio::test]
    async fn resolver_not_found_short_circuits_without_touching_state() {
        let f = default_fixture();
        let (err_epoch, err) = f.switcher.switch(by_name("ghost")).await.unwrap_err();
        assert!(matches!(err, SwitchError::NotFound(_)));
        // No epoch bump, no port calls. Pre-begin error stamps with the
        // committed epoch (still 0 — we haven't switched at all yet).
        assert_eq!(err_epoch, 0);
        assert_eq!(f.epoch.current(), 0);
        assert!(f.session.transition_steps().is_empty());
        assert!(f.state.is_idle());
    }

    #[tokio::test]
    async fn switch_back_replays_previous_context() {
        // Trace: (empty) → demo → admin. switch_back must take us from
        // admin BACK to demo (the context before admin), not "forward"
        // to admin itself.
        let f = default_fixture();
        f.switcher.switch(by_name("demo")).await.unwrap();
        f.switcher.switch(by_name("admin")).await.unwrap();
        // After admin switch, history stores the pre-switch context (demo).
        assert_eq!(
            f.history
                .lock()
                .unwrap()
                .previous()
                .unwrap()
                .target
                .project_name,
            "demo"
        );

        let (_, snap) = f.switcher.switch_back().await.unwrap();
        assert_eq!(snap.target.project_name, "demo");
    }

    #[tokio::test]
    async fn switch_back_empty_history_returns_not_found() {
        let f = default_fixture();
        let (_, err) = f.switcher.switch_back().await.unwrap_err();
        assert!(matches!(err, SwitchError::NotFound(_)));
    }

    #[tokio::test]
    async fn switch_back_failure_preserves_history_for_retry() {
        // H1 regression: switch_back used to pop history at the start,
        // so any failure (including `InProgress`) silently consumed the
        // only rollback entry. Peek-not-pop keeps the entry around.
        let f = default_fixture();
        f.switcher.switch(by_name("demo")).await.unwrap();
        f.switcher.switch(by_name("admin")).await.unwrap();
        // history: [demo]
        // Occupy the state machine so `switch_back` fails with InProgress.
        let busy_target = ContextTarget {
            cloud: "devstack".into(),
            project_id: "busy".into(),
            project_name: "busy".into(),
            domain: "default".into(),
        };
        let _ = f.state.try_begin(busy_target).unwrap();

        let (_, err) = f.switcher.switch_back().await.unwrap_err();
        assert!(matches!(err, SwitchError::InProgress));
        // History entry must still be there for a retry.
        assert_eq!(
            f.history
                .lock()
                .unwrap()
                .previous()
                .unwrap()
                .target
                .project_name,
            "demo"
        );
    }

    #[tokio::test]
    async fn second_switch_while_first_in_progress_returns_in_progress() {
        // Occupy the state machine by manually beginning a switch.
        let f = default_fixture();
        let target = ContextTarget {
            cloud: "devstack".into(),
            project_id: "busy".into(),
            project_name: "busy".into(),
            domain: "default".into(),
        };
        let _epoch = f.state.try_begin(target).unwrap();
        let (_, err) = f.switcher.switch(by_name("demo")).await.unwrap_err();
        assert!(matches!(err, SwitchError::InProgress));
        // No port calls attempted.
        assert!(f.session.transition_steps().is_empty());
    }

    #[tokio::test]
    async fn successful_switch_cancels_previous_epoch_work() {
        let f = default_fixture();
        // Register a token under epoch 0 (pre-switch).
        let old_token = f.cancellation.register(0);
        assert!(!old_token.is_cancelled());

        f.switcher.switch(by_name("demo")).await.unwrap();
        assert!(
            old_token.is_cancelled(),
            "pre-switch epoch work must be cancelled as part of the switch"
        );
    }

    /// BL-P2-074 FR-4 / D1: repeating `:switch-cloud` against the same
    /// cloud (which resolves to the same `ContextTarget`) must not bump
    /// the epoch, and must not re-run the rescope/commit side effects.
    /// TOCTOU: this only holds for sequential callers — a concurrent
    /// switch racing `try_begin` still returns `InProgress` (by design).
    #[tokio::test]
    async fn test_switcher_noop_on_same_target() {
        let f = default_fixture();
        // Prime: commit target "demo".
        f.switcher.switch(by_name("demo")).await.unwrap();
        let epoch_after_first = f.epoch.current();
        let steps_after_first = f.session.transition_steps();

        // Re-issue: same target, no session port re-entry, epoch unchanged.
        let (epoch, snapshot) = f.switcher.switch(by_name("demo")).await.unwrap();
        assert_eq!(
            epoch, epoch_after_first,
            "noop must return the committed epoch"
        );
        assert_eq!(snapshot.target.project_name, "demo");
        assert_eq!(
            f.epoch.current(),
            epoch_after_first,
            "epoch counter must not advance on noop"
        );
        assert_eq!(
            f.session.transition_steps(),
            steps_after_first,
            "no additional port calls on noop"
        );
    }

    /// BL-P2-074 D4: `:switch-back` after a `CloudOnly` transition returns
    /// the pre-switch `ContextTarget`. CloudOnly origin is not preserved.
    #[tokio::test]
    async fn test_switch_back_after_cloud_only_returns_previous_target() {
        // Build a fixture whose resolver honours CloudOnly via a default
        // project. Reuse default_fixture topology but swap CloudDirectory.
        let epoch = Arc::new(ContextEpoch::new());
        let state = Arc::new(SwitchStateMachine::new(epoch.clone()));
        let cancellation = Arc::new(CancellationRegistry::new());
        let history = Arc::new(SyncMutex::new(ContextHistoryStore::new()));

        struct CloudsWithDefault;
        impl CloudDirectory for CloudsWithDefault {
            fn active_cloud(&self) -> String {
                "devstack".into()
            }
            fn known_clouds(&self) -> Vec<String> {
                vec!["devstack".into()]
            }
            fn default_project(&self, cloud: &str) -> Option<String> {
                (cloud == "devstack").then(|| "demo".to_string())
            }
        }
        let clouds: Arc<dyn CloudDirectory> = Arc::new(CloudsWithDefault);
        let directory: Arc<dyn ProjectDirectoryPort> = Arc::new(FakeDirectory {
            data: StdMutex::new(HashMap::from([(
                "devstack".into(),
                vec![candidate("demo", "id-demo"), candidate("admin", "id-admin")],
            )])),
        });
        let resolver = Arc::new(ContextTargetResolver::new(clouds, directory, None, None));
        let session = Arc::new(MockContextSession::new(
            scope("admin"),
            token("old", "admin"),
            token("new", "demo"),
        ));
        let switcher = ContextSwitcher::new(
            state,
            cancellation,
            resolver,
            session.clone() as Arc<dyn ContextSessionPort>,
            history,
        );

        // admin → demo (via CloudOnly resolving to default "demo")
        switcher.switch(by_name("admin")).await.unwrap();
        switcher
            .switch(ContextRequest::CloudOnly {
                cloud: "devstack".into(),
            })
            .await
            .unwrap();
        let (_, snapshot) = switcher.switch_back().await.unwrap();
        assert_eq!(
            snapshot.target.project_name, "admin",
            "switch-back must restore the pre-CloudOnly target"
        );
    }

    // -----------------------------------------------------------------------
    // BL-P2-080 FR-4: entry-epoch gate tests
    // -----------------------------------------------------------------------

    /// When the epoch changes during `resolve` (simulated by bumping state
    /// externally), `switch()` must return `SwitchError::InProgress`.
    #[tokio::test]
    async fn switch_epoch_drift_during_resolve_returns_in_progress() {
        // We need a resolver that bumps the epoch WHILE it's resolving.
        // We achieve this by having the resolver use a barrier-based fake
        // directory that signals back so the test can bump the epoch.
        use crate::context::epoch::ContextEpoch;
        use crate::context::state_machine::SwitchStateMachine;
        use crate::context::types::ContextTarget;
        use std::sync::{Arc, Mutex};

        let epoch = Arc::new(ContextEpoch::new());
        let state = Arc::new(SwitchStateMachine::new(epoch.clone()));
        let cancellation = Arc::new(CancellationRegistry::new());
        let history = Arc::new(Mutex::new(ContextHistoryStore::new()));

        // A directory that bumps epoch externally while resolving
        struct EpochBumpingDirectory {
            state: Arc<SwitchStateMachine>,
            data: StdMutex<HashMap<String, Vec<ProjectCandidate>>>,
        }

        #[async_trait]
        impl ProjectDirectoryPort for EpochBumpingDirectory {
            async fn list_projects(
                &self,
                cloud: &str,
            ) -> Result<Vec<ProjectCandidate>, SwitchError> {
                // Simulate a concurrent switch bumping the epoch before we return
                let bump_target = ContextTarget {
                    cloud: cloud.to_string(),
                    project_id: "concurrent".into(),
                    project_name: "concurrent".into(),
                    domain: "default".into(),
                };
                // Try to begin a concurrent switch to bump epoch
                let _ = self.state.try_begin(bump_target);
                Ok(self
                    .data
                    .lock()
                    .unwrap()
                    .get(cloud)
                    .cloned()
                    .unwrap_or_default())
            }
        }

        let directory: Arc<dyn ProjectDirectoryPort> = Arc::new(EpochBumpingDirectory {
            state: state.clone(),
            data: StdMutex::new(HashMap::from([(
                "devstack".into(),
                vec![candidate("demo", "id-demo"), candidate("admin", "id-admin")],
            )])),
        });
        let clouds: Arc<dyn CloudDirectory> = Arc::new(FakeClouds);
        let resolver = Arc::new(ContextTargetResolver::new(clouds, directory, None, None));
        let session = Arc::new(MockContextSession::new(
            scope("admin"),
            token("old", "admin"),
            token("new", "demo"),
        ));
        let switcher = ContextSwitcher::new(
            state,
            cancellation,
            resolver,
            session.clone() as Arc<dyn ContextSessionPort>,
            history,
        );

        // switch() should detect epoch drift and return InProgress
        let (_, err) = switcher.switch(by_name("demo")).await.unwrap_err();
        assert!(
            matches!(err, SwitchError::InProgress),
            "expected InProgress due to epoch drift, got {err:?}"
        );
        // No session port calls should have been made
        assert!(
            session.transition_steps().is_empty(),
            "no session calls expected when epoch drifted"
        );
    }

    /// Without epoch drift, switch proceeds normally (regression guard).
    #[tokio::test]
    async fn switch_no_drift_proceeds_normally() {
        let f = default_fixture();
        let (epoch, snapshot) = f.switcher.switch(by_name("demo")).await.unwrap();
        assert_eq!(epoch, 1);
        assert_eq!(snapshot.target.project_name, "demo");
    }
}
