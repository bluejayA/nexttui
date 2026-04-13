//! Shared vocabulary for the runtime context switch flow.
//!
//! - [`ContextRequest`] is the unresolved user input (from parser or picker).
//! - [`ContextTarget`] is the resolver's authoritative output.
//! - [`ContextSnapshot`] is what the switch returns after a successful commit.
//! - [`SessionHandle`] is the port-local handle returned by `begin` that the
//!   switcher passes through the transition and eventually to `commit` or
//!   `rollback`.

use crate::infra::catalog::ServiceCatalog;
use crate::port::types::{Token, TokenScope};
use chrono::{DateTime, Utc};

use super::epoch::Epoch;

/// Unresolved user input. The resolver maps this into a [`ContextTarget`]
/// before any side effects are performed.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ContextRequest {
    ByName {
        cloud: Option<String>,
        project: String,
        domain: Option<String>,
    },
    ById {
        cloud: Option<String>,
        project_id: String,
    },
}

/// Fully resolved target: every identifier is populated and authoritative.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct ContextTarget {
    pub cloud: String,
    pub project_id: String,
    pub project_name: String,
    pub domain: String,
}

impl From<&ContextTarget> for TokenScope {
    fn from(t: &ContextTarget) -> Self {
        TokenScope::Project {
            name: t.project_name.clone(),
            domain: t.domain.clone(),
        }
    }
}

/// The post-commit view of a switch. Carries the epoch so observers can
/// verify they are looking at the current generation.
#[derive(Debug, Clone)]
pub struct ContextSnapshot {
    pub target: ContextTarget,
    pub epoch: Epoch,
    pub token: Token,
    pub token_scope: TokenScope,
    pub captured_at: DateTime<Utc>,
}

/// Opaque-to-callers handle threaded through the session port. Contains the
/// rollback data (previous token + scope) plus staging slots that the port
/// uses internally during the transition.
#[allow(dead_code)] // Fields consumed by ContextSessionPort impl in Unit 3.
pub struct SessionHandle {
    pub(crate) epoch: Epoch,
    pub(crate) target: ContextTarget,
    pub(crate) previous_token: Token,
    pub(crate) previous_scope: TokenScope,
    pub(crate) staged_new_token: Option<Token>,
    pub(crate) staged_catalog: Option<ServiceCatalog>,
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_target() -> ContextTarget {
        ContextTarget {
            cloud: "devstack".to_string(),
            project_id: "abc123".to_string(),
            project_name: "admin".to_string(),
            domain: "default".to_string(),
        }
    }

    #[test]
    fn context_target_converts_to_project_token_scope() {
        let target = sample_target();
        let scope: TokenScope = (&target).into();
        assert_eq!(
            scope,
            TokenScope::Project {
                name: "admin".to_string(),
                domain: "default".to_string(),
            }
        );
    }

    #[test]
    fn context_request_by_name_is_constructible() {
        let r = ContextRequest::ByName {
            cloud: Some("devstack".to_string()),
            project: "admin".to_string(),
            domain: None,
        };
        match r {
            ContextRequest::ByName { project, .. } => assert_eq!(project, "admin"),
            _ => panic!("expected ByName"),
        }
    }

    #[test]
    fn context_request_by_id_is_constructible() {
        let r = ContextRequest::ById {
            cloud: None,
            project_id: "uuid-1".to_string(),
        };
        match r {
            ContextRequest::ById { project_id, .. } => assert_eq!(project_id, "uuid-1"),
            _ => panic!("expected ById"),
        }
    }

    #[test]
    fn context_target_equality_and_hash_match_structural_fields() {
        use std::collections::HashSet;
        let a = sample_target();
        let b = sample_target();
        assert_eq!(a, b);
        let mut set: HashSet<ContextTarget> = HashSet::new();
        set.insert(a);
        assert!(set.contains(&b));
    }
}
