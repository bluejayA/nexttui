//! Epoch-stamping sender for the module → worker action channel.
//!
//! BL-P2-031 Unit 4. `ActionSender` is the module-facing facade: it wraps
//! the raw `mpsc::UnboundedSender<VersionedEvent<Action>>` and captures the
//! epoch at the send site, so a stale queued action can be dropped by the
//! worker's epoch gate after a switch.
//!
//! Stamping happens at send time (not receive time) — this is the only way
//! the worker can tell "action queued before switch, discard" from "action
//! queued after switch, run." Both arrive in the same queue, so without a
//! per-message epoch we could not distinguish them.

use std::sync::Arc;

use tokio::sync::mpsc::{self, error::SendError, error::TryRecvError};

use crate::action::Action;
use crate::infra::rbac::RbacGuard;

use super::epoch::ContextEpoch;
use super::versioned::VersionedEvent;

/// Read-only view of the currently active project scope, supplied to
/// `ActionSender` for FR2 origin stamping (BL-P2-085 Step 8).
///
/// `ActionSender` invokes `current_project_id()` at *send time* (not
/// construction time) so the stamp reflects the scope live at the moment a
/// mutating action is dispatched. Implementations must therefore read live
/// state — never a captured snapshot.
///
/// `Send + Sync` lets the sender be cloned across tasks (Tokio worker fanout)
/// while still calling the provider concurrently. The trait object form
/// (`Arc<dyn ScopeProvider>`) is what `ActionSender` will store in Step 9.
pub trait ScopeProvider: Send + Sync {
    /// Active project_id at the moment of the call. `None` means "unscoped"
    /// (e.g. before first auth) — the caller decides how to treat it
    /// (FR2 stamping uses `None` origin → no origin guard, no audit emit).
    fn current_project_id(&self) -> Option<String>;
}

impl ScopeProvider for Arc<RbacGuard> {
    fn current_project_id(&self) -> Option<String> {
        self.project_id()
    }
}

#[derive(Clone)]
pub struct ActionSender {
    tx: mpsc::UnboundedSender<VersionedEvent<Action>>,
    epoch: Arc<ContextEpoch>,
}

/// Receiver-side convenience wrapper that unwraps the `VersionedEvent`
/// envelope for callers that do not need the epoch (typically tests).
///
/// The worker uses the raw `mpsc::UnboundedReceiver<VersionedEvent<Action>>`
/// because it needs the epoch to drop stale actions; tests that only care
/// about the [`Action`] payload should use [`ActionReceiver`] so existing
/// `recv().await.unwrap()` assertions keep working.
pub struct ActionReceiver {
    rx: mpsc::UnboundedReceiver<VersionedEvent<Action>>,
}

impl ActionReceiver {
    pub fn new(rx: mpsc::UnboundedReceiver<VersionedEvent<Action>>) -> Self {
        Self { rx }
    }

    pub async fn recv(&mut self) -> Option<Action> {
        self.rx.recv().await.map(VersionedEvent::into_inner)
    }

    pub fn try_recv(&mut self) -> Result<Action, TryRecvError> {
        self.rx.try_recv().map(VersionedEvent::into_inner)
    }

    pub fn close(&mut self) {
        self.rx.close();
    }

    pub fn into_inner(self) -> mpsc::UnboundedReceiver<VersionedEvent<Action>> {
        self.rx
    }
}

/// Create a paired sender/receiver for tests. The sender stamps with a
/// fresh `ContextEpoch` that starts at 0; tests that want to simulate a
/// bumped epoch can call `.bump()` on the epoch handle returned alongside.
pub fn test_action_channel() -> (ActionSender, ActionReceiver) {
    let (tx, rx) = mpsc::unbounded_channel();
    let epoch = Arc::new(ContextEpoch::new());
    (ActionSender::new(tx, epoch), ActionReceiver::new(rx))
}

impl ActionSender {
    pub fn new(
        tx: mpsc::UnboundedSender<VersionedEvent<Action>>,
        epoch: Arc<ContextEpoch>,
    ) -> Self {
        Self { tx, epoch }
    }

    /// Stamp `action` with the current epoch and forward it to the worker.
    /// Signature mirrors `UnboundedSender::send` so existing call sites
    /// compile unchanged.
    ///
    /// The `Err` variant is ~176 bytes because it carries the whole
    /// `VersionedEvent<Action>`. Boxing it (or changing `Action`'s
    /// representation) would touch every send site in the codebase and
    /// needs benchmark-based justification — tracked under BL-P2-060.
    #[allow(
        clippy::result_large_err,
        reason = "tracked by BL-P2-060 — pending bench-based boxing decision"
    )]
    pub fn send(&self, action: Action) -> Result<(), SendError<VersionedEvent<Action>>> {
        self.tx
            .send(VersionedEvent::new(action, self.epoch.current()))
    }

    /// Exposes the underlying raw sender. Used by the few sites that need
    /// to forward a pre-stamped envelope (e.g. replay from a queued event).
    pub fn raw(&self) -> &mpsc::UnboundedSender<VersionedEvent<Action>> {
        &self.tx
    }

    /// Returns a shared handle to the epoch used for stamping. The
    /// `App` uses this to keep its `current_epoch` in sync with the one
    /// that sent messages are stamped against — they must be the same
    /// object or the epoch gate is meaningless.
    pub fn epoch(&self) -> Arc<ContextEpoch> {
        self.epoch.clone()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::infra::rbac::RbacGuard;
    use crate::port::types::TokenRole;

    fn role(name: &str) -> TokenRole {
        TokenRole {
            id: format!("{name}-id"),
            name: name.to_string(),
        }
    }

    // --- BL-P2-085 Step 8: ScopeProvider trait ---

    #[test]
    fn test_scope_provider_returns_current_project_id() {
        let guard = Arc::new(RbacGuard::new());
        guard.update_roles(vec![role("admin")], Some("proj-A".into()));
        assert_eq!(
            ScopeProvider::current_project_id(&guard),
            Some("proj-A".to_string()),
            "Arc<RbacGuard> impl must read project_id() from current snapshot"
        );
    }

    #[test]
    fn test_scope_provider_returns_none_when_unscoped() {
        let guard: Arc<RbacGuard> = Arc::new(RbacGuard::new());
        assert_eq!(
            ScopeProvider::current_project_id(&guard),
            None,
            "Unscoped guard must yield None — caller (ActionSender) treats as no origin stamp"
        );
    }

    #[test]
    fn test_scope_provider_reflects_post_update_change() {
        // FR2 invariant: ActionSender stamps using *current* scope at send time.
        // Therefore ScopeProvider must read live state, not a captured snapshot.
        let guard = Arc::new(RbacGuard::new());
        guard.update_roles(vec![role("admin")], Some("proj-A".into()));
        assert_eq!(
            ScopeProvider::current_project_id(&guard),
            Some("proj-A".to_string())
        );
        guard.update_roles(vec![role("admin")], Some("proj-B".into()));
        assert_eq!(
            ScopeProvider::current_project_id(&guard),
            Some("proj-B".to_string()),
            "Provider must reflect live state for FR2 stamping correctness"
        );
    }

    #[tokio::test]
    async fn send_stamps_with_current_epoch() {
        let (tx, mut rx) = mpsc::unbounded_channel();
        let epoch = Arc::new(ContextEpoch::new());
        let sender = ActionSender::new(tx, epoch.clone());

        sender.send(Action::Quit).unwrap();
        let ev = rx.recv().await.unwrap();
        assert_eq!(ev.epoch(), 0);

        epoch.bump();
        epoch.bump();
        sender.send(Action::Quit).unwrap();
        let ev2 = rx.recv().await.unwrap();
        assert_eq!(ev2.epoch(), 2);
    }

    #[test]
    fn sender_is_cheaply_cloneable() {
        let (tx, _rx) = mpsc::unbounded_channel();
        let epoch = Arc::new(ContextEpoch::new());
        let a = ActionSender::new(tx, epoch);
        let b = a.clone();
        // Both should point at the same epoch/tx.
        a.send(Action::Quit).unwrap();
        b.send(Action::Quit).unwrap();
    }
}
