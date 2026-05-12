//! Mock implementations of the context session / scoped auth / endpoint
//! cache ports, used by Unit 4 (switcher) tests and by anything that needs
//! deterministic fault injection without hitting a real Keystone.
//!
//! BL-P2-031 Unit 3.

use std::sync::Arc;

use async_trait::async_trait;
use std::sync::Mutex;

use super::context_session::ContextSessionPort;
use super::http_endpoint_cache::HttpEndpointCache;
use super::scoped_auth::ScopedAuthPort;
use super::types::{Token, TokenScope};
use crate::context::{ContextSnapshot, ContextTarget, Epoch, SessionHandle, SwitchError};

// ---------- MockHttpEndpointCache ----------

#[derive(Debug, Default)]
pub struct MockHttpEndpointCache {
    invalidate_count: Mutex<usize>,
}

impl MockHttpEndpointCache {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn invalidate_count(&self) -> usize {
        *self
            .invalidate_count
            .lock()
            .unwrap_or_else(|e| e.into_inner())
    }
}

#[async_trait]
impl HttpEndpointCache for MockHttpEndpointCache {
    async fn invalidate(&self) {
        *self
            .invalidate_count
            .lock()
            .unwrap_or_else(|e| e.into_inner()) += 1;
    }
}

// ---------- MockScopedAuthPort ----------

pub struct MockScopedAuthPort {
    scope: Mutex<TokenScope>,
    token: Mutex<Token>,
    set_active_error: Mutex<Option<SwitchError>>,
    set_active_calls: Mutex<Vec<(TokenScope, Token)>>,
}

impl MockScopedAuthPort {
    pub fn new(initial_scope: TokenScope, initial_token: Token) -> Self {
        Self {
            scope: Mutex::new(initial_scope),
            token: Mutex::new(initial_token),
            set_active_error: Mutex::new(None),
            set_active_calls: Mutex::new(Vec::new()),
        }
    }

    pub fn fail_next_set_active(&self, err: SwitchError) {
        *self
            .set_active_error
            .lock()
            .unwrap_or_else(|e| e.into_inner()) = Some(err);
    }

    pub fn set_active_calls(&self) -> Vec<(TokenScope, Token)> {
        self.set_active_calls
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .clone()
    }
}

#[async_trait]
impl ScopedAuthPort for MockScopedAuthPort {
    fn current_scope(&self) -> TokenScope {
        self.scope.lock().unwrap_or_else(|e| e.into_inner()).clone()
    }

    fn current_token(&self) -> Option<Token> {
        Some(self.token.lock().unwrap_or_else(|e| e.into_inner()).clone())
    }

    async fn set_active(&self, scope: TokenScope, token: Token) -> Result<(), SwitchError> {
        if let Some(err) = self
            .set_active_error
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .take()
        {
            return Err(err);
        }
        self.set_active_calls
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .push((scope.clone(), token.clone()));
        *self.scope.lock().unwrap_or_else(|e| e.into_inner()) = scope;
        *self.token.lock().unwrap_or_else(|e| e.into_inner()) = token;
        Ok(())
    }
}

// ---------- MockContextSession ----------

/// Step label recorded for order assertions.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TransitionStep {
    Begin,
    Rescope,
    Refresh,
    Commit,
    Rollback,
}

#[derive(Debug, Default)]
struct MockFailures {
    begin: Option<SwitchError>,
    rescope: Option<SwitchError>,
    refresh: Option<SwitchError>,
    commit: Option<SwitchError>,
}

/// Programmable [`ContextSessionPort`] double for Unit 4 tests.
pub struct MockContextSession {
    previous_token: Token,
    previous_scope: TokenScope,
    new_token: Token,
    steps: Arc<Mutex<Vec<TransitionStep>>>,
    failures: Arc<Mutex<MockFailures>>,
    rollback_called: Arc<Mutex<bool>>,
    commit_auto_revert: Arc<Mutex<bool>>,
}

impl MockContextSession {
    pub fn new(previous_scope: TokenScope, previous_token: Token, new_token: Token) -> Self {
        Self {
            previous_token,
            previous_scope,
            new_token,
            steps: Arc::new(Mutex::new(Vec::new())),
            failures: Arc::new(Mutex::new(MockFailures::default())),
            rollback_called: Arc::new(Mutex::new(false)),
            commit_auto_revert: Arc::new(Mutex::new(false)),
        }
    }

    pub fn with_begin_failure(self, err: SwitchError) -> Self {
        self.failures
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .begin = Some(err);
        self
    }

    pub fn with_rescope_failure(self, err: SwitchError) -> Self {
        self.failures
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .rescope = Some(err);
        self
    }

    pub fn with_refresh_failure(self, err: SwitchError) -> Self {
        self.failures
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .refresh = Some(err);
        self
    }

    pub fn with_commit_failure(self, err: SwitchError) -> Self {
        self.failures
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .commit = Some(err);
        self
    }

    /// Simulates commit self-revert: commit will run its internal rollback
    /// and return CommitFailed without the caller touching `rollback`.
    pub fn with_partial_commit_failure(self) -> Self {
        self.failures
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .commit = Some(SwitchError::CommitFailed("staged step failed".into()));
        *self
            .commit_auto_revert
            .lock()
            .unwrap_or_else(|e| e.into_inner()) = true;
        self
    }

    pub fn transition_steps(&self) -> Vec<TransitionStep> {
        self.steps.lock().unwrap_or_else(|e| e.into_inner()).clone()
    }

    pub fn rollback_called(&self) -> bool {
        *self
            .rollback_called
            .lock()
            .unwrap_or_else(|e| e.into_inner())
    }
}

#[async_trait]
impl ContextSessionPort for MockContextSession {
    async fn begin(
        &self,
        target: &ContextTarget,
        epoch: Epoch,
    ) -> Result<SessionHandle, SwitchError> {
        self.steps
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .push(TransitionStep::Begin);
        if let Some(err) = self
            .failures
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .begin
            .take()
        {
            return Err(err);
        }
        Ok(SessionHandle::new(
            epoch,
            target.clone(),
            self.previous_token.clone(),
            self.previous_scope.clone(),
        ))
    }

    async fn rescope(&self, handle: &mut SessionHandle) -> Result<(), SwitchError> {
        self.steps
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .push(TransitionStep::Rescope);
        if let Some(err) = self
            .failures
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .rescope
            .take()
        {
            return Err(err);
        }
        handle.stage_token(self.new_token.clone());
        Ok(())
    }

    async fn refresh_catalog(&self, _handle: &mut SessionHandle) -> Result<(), SwitchError> {
        self.steps
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .push(TransitionStep::Refresh);
        if let Some(err) = self
            .failures
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .refresh
            .take()
        {
            return Err(err);
        }
        Ok(())
    }

    async fn commit(&self, handle: SessionHandle) -> Result<ContextSnapshot, SwitchError> {
        self.steps
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .push(TransitionStep::Commit);
        if let Some(err) = self
            .failures
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .commit
            .take()
        {
            // Self-revert simulates commit's atomic contract.
            if *self
                .commit_auto_revert
                .lock()
                .unwrap_or_else(|e| e.into_inner())
            {
                self.steps
                    .lock()
                    .unwrap_or_else(|e| e.into_inner())
                    .push(TransitionStep::Rollback);
            }
            return Err(err);
        }
        Ok(ContextSnapshot {
            target: handle.target().clone(),
            epoch: handle.epoch(),
            token: self.new_token.clone(),
            token_scope: TokenScope::from(handle.target()),
            captured_at: chrono::Utc::now(),
        })
    }

    async fn rollback(&self, _handle: SessionHandle) {
        self.steps
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .push(TransitionStep::Rollback);
        *self
            .rollback_called
            .lock()
            .unwrap_or_else(|e| e.into_inner()) = true;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::port::types::{CatalogEntry, ProjectScope, Token};
    use chrono::{TimeZone, Utc};

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
            user_id: String::new(),
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

    #[tokio::test]
    async fn happy_path_records_all_steps_in_order() {
        let prev_scope = TokenScope::Project {
            name: "admin".into(),
            domain: "default".into(),
        };
        let session = MockContextSession::new(
            prev_scope,
            make_token("old", "admin"),
            make_token("new", "demo"),
        );
        let target = make_target("demo");

        let mut handle = session.begin(&target, 7).await.unwrap();
        session.rescope(&mut handle).await.unwrap();
        session.refresh_catalog(&mut handle).await.unwrap();
        let snapshot = session.commit(handle).await.unwrap();

        assert_eq!(
            session.transition_steps(),
            vec![
                TransitionStep::Begin,
                TransitionStep::Rescope,
                TransitionStep::Refresh,
                TransitionStep::Commit,
            ]
        );
        assert_eq!(snapshot.target.project_name, "demo");
        assert_eq!(snapshot.epoch, 7);
        assert_eq!(snapshot.token.id, "new");
        assert!(!session.rollback_called());
    }

    #[tokio::test]
    async fn rescope_failure_keeps_previous_state_untouched() {
        let prev_scope = TokenScope::Project {
            name: "admin".into(),
            domain: "default".into(),
        };
        let session = MockContextSession::new(
            prev_scope,
            make_token("old", "admin"),
            make_token("new", "demo"),
        )
        .with_rescope_failure(SwitchError::RescopeRejected("forbidden".into()));
        let target = make_target("demo");

        let mut handle = session.begin(&target, 1).await.unwrap();
        let err = session.rescope(&mut handle).await.unwrap_err();
        assert!(matches!(err, SwitchError::RescopeRejected(_)));
        session.rollback(handle).await;

        assert_eq!(
            session.transition_steps(),
            vec![
                TransitionStep::Begin,
                TransitionStep::Rescope,
                TransitionStep::Rollback,
            ]
        );
        assert!(session.rollback_called());
    }

    #[tokio::test]
    async fn partial_commit_failure_is_self_reverting() {
        let prev_scope = TokenScope::Project {
            name: "admin".into(),
            domain: "default".into(),
        };
        let session = MockContextSession::new(
            prev_scope,
            make_token("old", "admin"),
            make_token("new", "demo"),
        )
        .with_partial_commit_failure();
        let target = make_target("demo");

        let mut handle = session.begin(&target, 2).await.unwrap();
        session.rescope(&mut handle).await.unwrap();
        session.refresh_catalog(&mut handle).await.unwrap();
        let err = session.commit(handle).await.unwrap_err();

        assert!(matches!(err, SwitchError::CommitFailed(_)));
        assert_eq!(
            session.transition_steps(),
            vec![
                TransitionStep::Begin,
                TransitionStep::Rescope,
                TransitionStep::Refresh,
                TransitionStep::Commit,
                TransitionStep::Rollback, // self-revert inside commit
            ]
        );
    }

    #[tokio::test]
    async fn mock_scoped_auth_port_tracks_set_active() {
        let initial_scope = TokenScope::Project {
            name: "admin".into(),
            domain: "default".into(),
        };
        let new_scope = TokenScope::Project {
            name: "demo".into(),
            domain: "default".into(),
        };
        let port = MockScopedAuthPort::new(initial_scope.clone(), make_token("old", "admin"));

        assert_eq!(port.current_scope(), initial_scope);
        port.set_active(new_scope.clone(), make_token("new", "demo"))
            .await
            .unwrap();
        assert_eq!(port.current_scope(), new_scope);
        assert_eq!(port.set_active_calls().len(), 1);
    }

    #[tokio::test]
    async fn mock_scoped_auth_port_respects_forced_failure() {
        let scope = TokenScope::Project {
            name: "admin".into(),
            domain: "default".into(),
        };
        let port = MockScopedAuthPort::new(scope.clone(), make_token("old", "admin"));
        port.fail_next_set_active(SwitchError::Unsupported("rescope disabled".into()));

        let err = port
            .set_active(
                TokenScope::Project {
                    name: "demo".into(),
                    domain: "default".into(),
                },
                make_token("new", "demo"),
            )
            .await
            .unwrap_err();
        assert!(matches!(err, SwitchError::Unsupported(_)));
        // Previous state preserved
        assert_eq!(port.current_scope(), scope);
    }

    #[tokio::test]
    async fn mock_http_endpoint_cache_counts_invalidations() {
        let cache = MockHttpEndpointCache::new();
        assert_eq!(cache.invalidate_count(), 0);
        cache.invalidate().await;
        cache.invalidate().await;
        assert_eq!(cache.invalidate_count(), 2);
    }
}
