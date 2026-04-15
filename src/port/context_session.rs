//! Atomic runtime context transition boundary.
//!
//! The port encapsulates Keystone rescoping, catalog refresh, token-cache
//! persistence, and active-scope mutation so the [`ContextSwitcher`] only
//! has to orchestrate epoch/cancel/state transitions.
//!
//! Contract:
//! - `begin` stages a transition and captures the rollback data inside the
//!   returned [`SessionHandle`]. External state is not mutated yet.
//! - `rescope` and `refresh_catalog` perform network work; on failure the
//!   caller must invoke `rollback(handle)` to release the session.
//! - `commit` is all-or-nothing. If any of its internal steps fails it
//!   self-reverts and returns [`SwitchError::CommitFailed`]; the caller must
//!   *not* call `rollback` after `commit` returns.
//! - `rollback` drops staged state and restores the previous scope/token.

use async_trait::async_trait;

use crate::context::{ContextSnapshot, ContextTarget, Epoch, SessionHandle, SwitchError};

#[async_trait]
pub trait ContextSessionPort: Send + Sync {
    async fn begin(
        &self,
        target: &ContextTarget,
        epoch: Epoch,
    ) -> Result<SessionHandle, SwitchError>;

    async fn rescope(&self, handle: &mut SessionHandle) -> Result<(), SwitchError>;

    async fn refresh_catalog(&self, handle: &mut SessionHandle) -> Result<(), SwitchError>;

    async fn commit(&self, handle: SessionHandle) -> Result<ContextSnapshot, SwitchError>;

    async fn rollback(&self, handle: SessionHandle);
}
