//! Port for the Keystone v3 rescoping handshake. Kept separate from
//! [`AuthProvider`](super::auth::AuthProvider) so that the rescope path can
//! be mocked without pulling in the full auth surface.

use async_trait::async_trait;

use super::types::Token;
use crate::context::{ContextTarget, SwitchError};

#[async_trait]
pub trait KeystoneRescopePort: Send + Sync {
    /// Exchange the currently active token for one scoped to `target`.
    /// Implementations must honour `expires_at` returned by Keystone
    /// verbatim (no TTL inference).
    async fn rescope(
        &self,
        current_token: &Token,
        target: &ContextTarget,
    ) -> Result<Token, SwitchError>;
}
