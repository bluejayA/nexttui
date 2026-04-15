//! Port for scope/token mutation, separated from [`AuthProvider`] so that
//! read-only auth access stays untouched while runtime rescope has its own
//! explicit API (BL-P2-031 Unit 3).

use async_trait::async_trait;

use super::types::{Token, TokenScope};
use crate::context::SwitchError;

#[async_trait]
pub trait ScopedAuthPort: Send + Sync {
    /// Snapshot of the currently active scope.
    fn current_scope(&self) -> TokenScope;

    /// Snapshot of the currently active token, or `None` if no token has
    /// been authenticated yet for the active scope. Cloning is intentional —
    /// callers need an owned copy to capture pre-switch state for rollback.
    /// Callers that require a token (e.g. `ScopedAuthSession::begin`) must
    /// translate `None` into an error rather than fabricating a placeholder.
    fn current_token(&self) -> Option<Token>;

    /// Atomically switch the active scope to the provided one and stash the
    /// matching token. Implementations must not leave the adapter in a
    /// half-mutated state on failure — if anything fails here, the previous
    /// scope/token remain authoritative.
    async fn set_active(&self, scope: TokenScope, token: Token) -> Result<(), SwitchError>;
}
