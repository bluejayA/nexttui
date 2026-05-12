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

use crate::action::{Action, DispatchedAction};
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
/// (`Arc<dyn ScopeProvider>`) is what `ActionSender` stores in Step 9.
pub trait ScopeProvider: Send + Sync {
    /// Active project_id at the moment of the call. `None` means "unscoped"
    /// (e.g. before first auth) — the caller decides how to treat it
    /// (FR2 stamping uses `None` origin → no origin guard, no audit emit).
    fn current_project_id(&self) -> Option<String>;
}

// Implementation lives on `RbacGuard` itself (not `Arc<RbacGuard>`) so an
// `Arc<RbacGuard>` value coerces to `Arc<dyn ScopeProvider>` via the
// standard unsized coercion (T: Trait ⇒ Arc<T> → Arc<dyn Trait>).
impl ScopeProvider for RbacGuard {
    fn current_project_id(&self) -> Option<String> {
        self.project_id()
    }
}

/// Module-facing facade over the worker action channel.
///
/// Two responsibilities, both done at *send time*:
/// 1. Stamp the current `ContextEpoch` so the worker can drop stale work after
///    a switch (BL-P2-031 Unit 4).
/// 2. Stamp the `origin_project_id` for mutation Actions, sourced live from
///    `scope_provider`, so the worker (Step 11) can reject TOCTOU mismatches
///    where a mutation queued under one scope is consumed under another
///    (BL-P2-085 FR2).
#[derive(Clone)]
pub struct ActionSender {
    tx: mpsc::UnboundedSender<VersionedEvent<DispatchedAction>>,
    epoch: Arc<ContextEpoch>,
    scope_provider: Arc<dyn ScopeProvider>,
}

/// Receiver-side convenience wrapper that unwraps the `VersionedEvent`
/// envelope for callers that do not need the epoch (typically tests).
///
/// The worker uses the raw
/// `mpsc::UnboundedReceiver<VersionedEvent<DispatchedAction>>` because it
/// needs both the epoch (drop stale actions) and the `origin_project_id`
/// (FR2 mutation guard, Step 11). Tests that only care about the [`Action`]
/// payload should use [`ActionReceiver`] so existing `recv().await.unwrap()`
/// assertions keep working — the receiver internally strips the
/// `DispatchedAction` envelope and yields the underlying `Action`.
pub struct ActionReceiver {
    rx: mpsc::UnboundedReceiver<VersionedEvent<DispatchedAction>>,
}

impl ActionReceiver {
    pub fn new(rx: mpsc::UnboundedReceiver<VersionedEvent<DispatchedAction>>) -> Self {
        Self { rx }
    }

    pub async fn recv(&mut self) -> Option<Action> {
        self.rx.recv().await.map(|env| env.into_inner().action)
    }

    pub fn try_recv(&mut self) -> Result<Action, TryRecvError> {
        self.rx.try_recv().map(|env| env.into_inner().action)
    }

    pub fn close(&mut self) {
        self.rx.close();
    }

    pub fn into_inner(self) -> mpsc::UnboundedReceiver<VersionedEvent<DispatchedAction>> {
        self.rx
    }
}

/// Create a paired sender/receiver for tests. The sender stamps with a
/// fresh `ContextEpoch` that starts at 0 and an unscoped `RbacGuard` as the
/// scope provider (always returns `None` origin) — tests that want to
/// observe FR2 stamping should construct an `ActionSender` manually with a
/// configured `Arc<RbacGuard>` (see `test_sender_stamps_mutation_with_current_scope`).
pub fn test_action_channel() -> (ActionSender, ActionReceiver) {
    let (tx, rx) = mpsc::unbounded_channel();
    let epoch = Arc::new(ContextEpoch::new());
    let scope: Arc<dyn ScopeProvider> = Arc::new(RbacGuard::new());
    (ActionSender::new(tx, epoch, scope), ActionReceiver::new(rx))
}

impl ActionSender {
    pub fn new(
        tx: mpsc::UnboundedSender<VersionedEvent<DispatchedAction>>,
        epoch: Arc<ContextEpoch>,
        scope_provider: Arc<dyn ScopeProvider>,
    ) -> Self {
        Self {
            tx,
            epoch,
            scope_provider,
        }
    }

    /// Wrap `action` in a [`DispatchedAction`] (stamping `origin_project_id`
    /// for mutations from the live scope provider) and forward to the worker
    /// via a [`VersionedEvent`] carrying the current epoch.
    ///
    /// The `Action` signature is preserved for the ~100 module call sites;
    /// scope stamping is a hidden side-channel concern of the sender.
    ///
    /// The `Err` variant is ~176+ bytes because it carries the whole
    /// `VersionedEvent<DispatchedAction>`. Boxing it (or changing `Action`'s
    /// representation) would touch every send site in the codebase and
    /// needs benchmark-based justification — tracked under BL-P2-060.
    #[allow(
        clippy::result_large_err,
        reason = "tracked by BL-P2-060 — pending bench-based boxing decision"
    )]
    pub fn send(&self, action: Action) -> Result<(), SendError<VersionedEvent<DispatchedAction>>> {
        let dispatched = if crate::worker::action_is_mutation(&action) {
            match self.scope_provider.current_project_id() {
                Some(project_id) => DispatchedAction::stamped(action, project_id),
                None => DispatchedAction::unstamped(action),
            }
        } else {
            DispatchedAction::unstamped(action)
        };
        self.tx
            .send(VersionedEvent::new(dispatched, self.epoch.current()))
    }

    /// Exposes the underlying raw sender. Used by the few sites that need
    /// to forward a pre-stamped envelope (e.g. replay from a queued event).
    pub fn raw(&self) -> &mpsc::UnboundedSender<VersionedEvent<DispatchedAction>> {
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
        // Auto-deref: Arc<RbacGuard> → &RbacGuard → ScopeProvider method.
        assert_eq!(
            guard.current_project_id(),
            Some("proj-A".to_string()),
            "Arc<RbacGuard> must read project_id() via the impl on RbacGuard"
        );
        // Coercion to trait object also works (used by ActionSender).
        let dyn_scope: Arc<dyn ScopeProvider> = guard;
        assert_eq!(dyn_scope.current_project_id(), Some("proj-A".to_string()));
    }

    #[test]
    fn test_scope_provider_returns_none_when_unscoped() {
        let guard: Arc<RbacGuard> = Arc::new(RbacGuard::new());
        assert_eq!(
            guard.current_project_id(),
            None,
            "Unscoped guard must yield None — caller (ActionSender) treats as no origin stamp"
        );
    }

    // --- BL-P2-085 Step 9: ActionSender FR2 origin stamping ---

    #[tokio::test]
    async fn test_sender_stamps_mutation_with_current_scope() {
        // FR2: ActionSender stamps mutation Actions with the live project_id
        // at send time. Worker (Step 11) compares stamp to active scope.
        let (raw_tx, mut raw_rx) = mpsc::unbounded_channel();
        let epoch = Arc::new(ContextEpoch::new());
        let guard = Arc::new(RbacGuard::new());
        guard.update_roles(vec![role("admin")], Some("proj-A".into()));
        let scope: Arc<dyn ScopeProvider> = guard;
        let sender = ActionSender::new(raw_tx, epoch, scope);

        sender
            .send(Action::DeleteServer {
                id: "srv-1".into(),
                name: "web".into(),
            })
            .unwrap();

        let envelope = raw_rx.recv().await.unwrap();
        let dispatched = envelope.into_inner();
        assert_eq!(
            dispatched.origin_project_id,
            Some("proj-A".to_string()),
            "mutation must be stamped with current scope project_id"
        );
        assert!(
            matches!(dispatched.action, Action::DeleteServer { .. }),
            "action payload preserved"
        );
    }

    #[tokio::test]
    async fn test_sender_leaves_readonly_unstamped() {
        // FR2: read-only Actions carry origin=None and bypass the worker guard.
        let (raw_tx, mut raw_rx) = mpsc::unbounded_channel();
        let epoch = Arc::new(ContextEpoch::new());
        let guard = Arc::new(RbacGuard::new());
        guard.update_roles(vec![role("admin")], Some("proj-A".into()));
        let scope: Arc<dyn ScopeProvider> = guard;
        let sender = ActionSender::new(raw_tx, epoch, scope);

        sender.send(Action::FetchServers).unwrap();

        let envelope = raw_rx.recv().await.unwrap();
        let dispatched = envelope.into_inner();
        assert_eq!(
            dispatched.origin_project_id, None,
            "read-only must not be stamped"
        );
    }

    #[tokio::test]
    async fn test_sender_handles_unscoped_provider_returns_none_origin() {
        // Defensive: even mutation Actions get origin=None when scope is unset
        // (pre-auth path). The worker treats None as "no FR2 guard" — caller
        // is expected to also gate the Action separately during pre-auth.
        let (raw_tx, mut raw_rx) = mpsc::unbounded_channel();
        let epoch = Arc::new(ContextEpoch::new());
        let guard: Arc<dyn ScopeProvider> = Arc::new(RbacGuard::new()); // no roles, no scope
        let sender = ActionSender::new(raw_tx, epoch, guard);

        sender
            .send(Action::DeleteServer {
                id: "srv-1".into(),
                name: "web".into(),
            })
            .unwrap();

        let envelope = raw_rx.recv().await.unwrap();
        let dispatched = envelope.into_inner();
        assert_eq!(
            dispatched.origin_project_id, None,
            "unscoped provider must yield None origin even for mutations"
        );
    }

    #[test]
    fn test_scope_provider_reflects_post_update_change() {
        // FR2 invariant: ActionSender stamps using *current* scope at send time.
        // Therefore ScopeProvider must read live state, not a captured snapshot.
        let guard = Arc::new(RbacGuard::new());
        guard.update_roles(vec![role("admin")], Some("proj-A".into()));
        assert_eq!(guard.current_project_id(), Some("proj-A".to_string()));
        guard.update_roles(vec![role("admin")], Some("proj-B".into()));
        assert_eq!(
            guard.current_project_id(),
            Some("proj-B".to_string()),
            "Provider must reflect live state for FR2 stamping correctness"
        );
    }

    #[tokio::test]
    async fn send_stamps_with_current_epoch() {
        let (tx, mut rx) = mpsc::unbounded_channel();
        let epoch = Arc::new(ContextEpoch::new());
        let scope: Arc<dyn ScopeProvider> = Arc::new(RbacGuard::new());
        let sender = ActionSender::new(tx, epoch.clone(), scope);

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
        let scope: Arc<dyn ScopeProvider> = Arc::new(RbacGuard::new());
        let a = ActionSender::new(tx, epoch, scope);
        let b = a.clone();
        // Both should point at the same epoch/tx/scope_provider.
        a.send(Action::Quit).unwrap();
        b.send(Action::Quit).unwrap();
    }
}
