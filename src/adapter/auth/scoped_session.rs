//! Production [`ContextSessionPort`] implementation used by the runtime
//! context switcher.
//!
//! Composition:
//! - [`ScopedAuthPort`] — snapshot + mutate the active scope/token
//! - [`KeystoneRescopePort`] — perform the Keystone v3 rescope handshake
//! - [`EndpointCatalogInvalidator`] — clear every HTTP client's endpoint cache
//! - [`TokenCacheStore`] — persist the rescoped token for reuse across restarts
//!
//! The port contract (see `port::context_session`) is enforced here:
//! - `begin` stages state only, never mutates externally observable state.
//! - `rescope`/`refresh_catalog` perform network side effects but leave the
//!   active scope untouched — the session handle carries the staged token.
//! - `commit` is all-or-nothing: if any sub-step fails it self-reverts the
//!   scope and returns [`SwitchError::CommitFailed`].
//! - `rollback` is safe to call after any of {begin, rescope, refresh} failed
//!   or succeeded; it is *not* valid after `commit` returns.
//!
//! BL-P2-031 Unit 3.

use std::sync::Arc;

use async_trait::async_trait;

use super::token_cache::TokenCacheStore;
use crate::adapter::http::endpoint_invalidator::EndpointCatalogInvalidator;
use crate::context::{ContextSnapshot, ContextTarget, Epoch, SessionHandle, SwitchError};
use crate::port::context_session::ContextSessionPort;
use crate::port::keystone_rescope::KeystoneRescopePort;
use crate::port::scoped_auth::ScopedAuthPort;
use crate::port::types::TokenScope;

pub struct ScopedAuthSession {
    scoped_auth: Arc<dyn ScopedAuthPort>,
    rescoper: Arc<dyn KeystoneRescopePort>,
    invalidator: Arc<EndpointCatalogInvalidator>,
    token_cache: TokenCacheStore,
}

impl ScopedAuthSession {
    pub fn new(
        scoped_auth: Arc<dyn ScopedAuthPort>,
        rescoper: Arc<dyn KeystoneRescopePort>,
        invalidator: Arc<EndpointCatalogInvalidator>,
        token_cache: TokenCacheStore,
    ) -> Self {
        Self {
            scoped_auth,
            rescoper,
            invalidator,
            token_cache,
        }
    }
}

#[async_trait]
impl ContextSessionPort for ScopedAuthSession {
    async fn begin(
        &self,
        target: &ContextTarget,
        epoch: Epoch,
    ) -> Result<SessionHandle, SwitchError> {
        // Capture the pre-transition snapshot so rollback can restore it.
        // A missing previous token means rollback would have nothing valid to
        // restore — refuse the switch up front rather than committing to a
        // transition we can't safely unwind (review C2).
        let previous_scope = self.scoped_auth.current_scope();
        let previous_token = self.scoped_auth.current_token().ok_or_else(|| {
            SwitchError::Unsupported(
                "no active token to capture for rollback — authenticate before switching"
                    .into(),
            )
        })?;
        Ok(SessionHandle::new(epoch, target.clone(), previous_token, previous_scope))
    }

    async fn rescope(&self, handle: &mut SessionHandle) -> Result<(), SwitchError> {
        let new_token = self
            .rescoper
            .rescope(handle.previous_token(), handle.target())
            .await?;
        handle.stage_token(new_token);
        Ok(())
    }

    async fn refresh_catalog(&self, _handle: &mut SessionHandle) -> Result<(), SwitchError> {
        // Clearing cached endpoints forces every HTTP client to re-resolve
        // against the fresh service catalog that ships with the new token.
        self.invalidator.invalidate_all().await;
        Ok(())
    }

    async fn commit(&self, handle: SessionHandle) -> Result<ContextSnapshot, SwitchError> {
        let Some(new_token) = handle.staged_token().cloned() else {
            return Err(SwitchError::CommitFailed(
                "commit called without a staged token (rescope did not run?)".into(),
            ));
        };
        let new_scope = TokenScope::from(handle.target());
        let previous_scope = handle.previous_scope().clone();
        let previous_token = handle.previous_token().clone();

        // Step 1: activate the new scope/token on the auth provider.
        if let Err(err) = self
            .scoped_auth
            .set_active(new_scope.clone(), new_token.clone())
            .await
        {
            return Err(SwitchError::CommitFailed(format!(
                "set_active rejected: {err}"
            )));
        }

        // Step 2: persist the token so subsequent sessions can reuse it.
        if let Err(err) = self.token_cache.store_scoped(&new_scope, &new_token) {
            // Self-revert: restore the previous active scope.
            let _ = self
                .scoped_auth
                .set_active(previous_scope, previous_token)
                .await;
            return Err(SwitchError::CommitFailed(format!(
                "token cache store failed: {err}"
            )));
        }

        Ok(ContextSnapshot {
            target: handle.target().clone(),
            epoch: handle.epoch(),
            token: new_token,
            token_scope: new_scope,
            captured_at: chrono::Utc::now(),
        })
    }

    async fn rollback(&self, _handle: SessionHandle) {
        // No externally observable state was mutated by begin/rescope/refresh
        // — they only staged data inside the handle. Dropping the handle is
        // sufficient.
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::adapter::http::endpoint_invalidator::EndpointCatalogInvalidator;
    use crate::context::{ContextSnapshot, ContextTarget};
    use crate::port::keystone_rescope::KeystoneRescopePort;
    use crate::port::mock_context::{MockHttpEndpointCache, MockScopedAuthPort};
    use crate::port::types::{CatalogEntry, ProjectScope, Token, TokenScope};
    use chrono::{TimeZone, Utc};
    use std::sync::Mutex;
    use tempfile::TempDir;

    fn make_token(id: &str, project: &str) -> Token {
        Token {
            id: id.to_string(),
            expires_at: Utc.with_ymd_and_hms(2030, 1, 1, 0, 0, 0).unwrap(),
            project: ProjectScope {
                id: format!("id-{project}"),
                name: project.to_string(),
                domain_id: "default".to_string(),
                domain_name: "default".to_string(),
            },
            roles: Vec::new(),
            catalog: Vec::<CatalogEntry>::new(),
        }
    }

    fn make_target(project: &str) -> ContextTarget {
        ContextTarget {
            cloud: "devstack".to_string(),
            project_id: format!("id-{project}"),
            project_name: project.to_string(),
            domain: "default".to_string(),
        }
    }

    struct StubRescoper {
        response: Mutex<Result<Token, SwitchError>>,
    }

    impl StubRescoper {
        fn ok(token: Token) -> Self {
            Self { response: Mutex::new(Ok(token)) }
        }
        fn err(err: SwitchError) -> Self {
            Self { response: Mutex::new(Err(err)) }
        }
    }

    #[async_trait]
    impl KeystoneRescopePort for StubRescoper {
        async fn rescope(
            &self,
            _current_token: &Token,
            _target: &ContextTarget,
        ) -> Result<Token, SwitchError> {
            let slot = std::mem::replace(
                &mut *self.response.lock().unwrap(),
                Err(SwitchError::Unsupported("consumed".into())),
            );
            slot
        }
    }

    fn setup_session(
        rescoper: Arc<dyn KeystoneRescopePort>,
        failing_set_active: Option<SwitchError>,
    ) -> (ScopedAuthSession, Arc<MockScopedAuthPort>, Arc<MockHttpEndpointCache>, TempDir) {
        let tmp = TempDir::new().unwrap();
        let initial_scope = TokenScope::Project {
            name: "admin".into(),
            domain: "default".into(),
        };
        let auth = Arc::new(MockScopedAuthPort::new(initial_scope, make_token("old", "admin")));
        if let Some(err) = failing_set_active {
            auth.fail_next_set_active(err);
        }
        let cache = Arc::new(MockHttpEndpointCache::new());
        let invalidator = Arc::new(EndpointCatalogInvalidator::new(vec![cache.clone()]));
        let store = TokenCacheStore::new(tmp.path());
        let session = ScopedAuthSession::new(
            auth.clone(),
            rescoper,
            invalidator,
            store,
        );
        (session, auth, cache, tmp)
    }

    #[tokio::test]
    async fn happy_path_activates_new_scope_and_returns_snapshot() {
        let rescoper = Arc::new(StubRescoper::ok(make_token("new", "demo")));
        let (session, auth, cache, _tmp) = setup_session(rescoper, None);
        let target = make_target("demo");

        let mut handle = session.begin(&target, 42).await.unwrap();
        session.rescope(&mut handle).await.unwrap();
        session.refresh_catalog(&mut handle).await.unwrap();
        let snapshot: ContextSnapshot = session.commit(handle).await.unwrap();

        assert_eq!(snapshot.epoch, 42);
        assert_eq!(snapshot.target.project_name, "demo");
        assert_eq!(snapshot.token.id, "new");
        assert_eq!(
            snapshot.token_scope,
            TokenScope::Project { name: "demo".into(), domain: "default".into() }
        );
        assert_eq!(
            auth.current_scope(),
            TokenScope::Project { name: "demo".into(), domain: "default".into() }
        );
        assert_eq!(cache.invalidate_count(), 1);
    }

    #[tokio::test]
    async fn rescope_failure_leaves_active_scope_untouched() {
        let rescoper = Arc::new(StubRescoper::err(SwitchError::RescopeRejected("403".into())));
        let (session, auth, cache, _tmp) = setup_session(rescoper, None);
        let target = make_target("demo");

        let mut handle = session.begin(&target, 1).await.unwrap();
        let err = session.rescope(&mut handle).await.unwrap_err();
        assert!(matches!(err, SwitchError::RescopeRejected(_)));
        session.rollback(handle).await;

        // Still on admin — set_active was never called, catalog not invalidated.
        assert_eq!(
            auth.current_scope(),
            TokenScope::Project { name: "admin".into(), domain: "default".into() }
        );
        assert_eq!(cache.invalidate_count(), 0);
        assert!(auth.set_active_calls().is_empty());
    }

    #[tokio::test]
    async fn begin_refuses_when_no_active_token_exists() {
        // C2 fix: begin must not fabricate an empty Token for rollback when
        // current_token() returns None — that would let an invalid token
        // flow into a future rollback's set_active. Refuse the switch up
        // front instead.
        struct EmptyAuth;
        #[async_trait]
        impl ScopedAuthPort for EmptyAuth {
            fn current_scope(&self) -> TokenScope {
                TokenScope::Unscoped
            }
            fn current_token(&self) -> Option<Token> {
                None
            }
            async fn set_active(
                &self,
                _scope: TokenScope,
                _token: Token,
            ) -> Result<(), SwitchError> {
                Ok(())
            }
        }

        let tmp = TempDir::new().unwrap();
        let invalidator = Arc::new(EndpointCatalogInvalidator::empty());
        let store = TokenCacheStore::new(tmp.path());
        let session = ScopedAuthSession::new(
            Arc::new(EmptyAuth),
            Arc::new(StubRescoper::ok(make_token("new", "demo"))),
            invalidator,
            store,
        );

        match session.begin(&make_target("demo"), 1).await {
            Err(SwitchError::Unsupported(_)) => {}
            Err(other) => panic!("expected Unsupported, got {other:?}"),
            Ok(_) => panic!("expected begin to refuse — current_token was None"),
        }
    }

    #[tokio::test]
    async fn commit_without_staged_token_fails_cleanly() {
        let rescoper = Arc::new(StubRescoper::ok(make_token("new", "demo")));
        let (session, auth, _cache, _tmp) = setup_session(rescoper, None);
        let target = make_target("demo");

        let handle = session.begin(&target, 2).await.unwrap();
        // Skip rescope — commit should reject.
        let err = session.commit(handle).await.unwrap_err();
        assert!(matches!(err, SwitchError::CommitFailed(_)));
        assert_eq!(
            auth.current_scope(),
            TokenScope::Project { name: "admin".into(), domain: "default".into() }
        );
    }

    #[tokio::test]
    async fn commit_reverts_scope_when_token_cache_fails() {
        use std::sync::Arc as StdArc;

        struct FailingRescoper;
        #[async_trait]
        impl KeystoneRescopePort for FailingRescoper {
            async fn rescope(
                &self,
                _t: &Token,
                _target: &ContextTarget,
            ) -> Result<Token, SwitchError> {
                Ok(make_token("new", "demo"))
            }
        }

        // Point the TokenCacheStore at a non-writable path so store_scoped
        // fails and commit must self-revert.
        let invalid_path = std::path::PathBuf::from("/nonexistent/nexttui-test-path/\0");
        let initial_scope = TokenScope::Project {
            name: "admin".into(),
            domain: "default".into(),
        };
        let auth = StdArc::new(MockScopedAuthPort::new(
            initial_scope.clone(),
            make_token("old", "admin"),
        ));
        let cache = StdArc::new(MockHttpEndpointCache::new());
        let invalidator = StdArc::new(EndpointCatalogInvalidator::new(vec![cache.clone()]));
        let store = TokenCacheStore::new(invalid_path);
        let session = ScopedAuthSession::new(
            auth.clone(),
            StdArc::new(FailingRescoper),
            invalidator,
            store,
        );

        let target = make_target("demo");
        let mut handle = session.begin(&target, 5).await.unwrap();
        session.rescope(&mut handle).await.unwrap();
        session.refresh_catalog(&mut handle).await.unwrap();
        let err = session.commit(handle).await.unwrap_err();

        assert!(matches!(err, SwitchError::CommitFailed(_)));
        // set_active was called twice: once to activate demo, once to revert.
        assert_eq!(auth.set_active_calls().len(), 2);
        // Final state is back to admin.
        assert_eq!(auth.current_scope(), initial_scope);
    }
}
