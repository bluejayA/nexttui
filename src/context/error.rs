//! Errors that can arise during runtime context switching.

use crate::port::error::ApiError;
use thiserror::Error;

use super::types::ContextTarget;

#[derive(Debug, Error)]
pub enum SwitchError {
    #[error("switch already in progress")]
    InProgress,

    #[error("rescope rejected: {0}")]
    RescopeRejected(String),

    #[error("catalog refresh failed: {0}")]
    CatalogFailed(String),

    #[error("commit failed: {0}")]
    CommitFailed(String),

    #[error("ambiguous target ({} candidates)", candidates.len())]
    Ambiguous { candidates: Vec<ContextTarget> },

    #[error("target not found: {0}")]
    NotFound(String),

    #[error("cloud '{cloud}' has no default project — use :switch-project <name>")]
    NotConfigured { cloud: String },

    #[error("unsupported: {0}")]
    Unsupported(String),

    #[error(transparent)]
    Api(#[from] ApiError),

    #[error(transparent)]
    Io(#[from] std::io::Error),
}

impl Clone for SwitchError {
    fn clone(&self) -> Self {
        match self {
            Self::InProgress => Self::InProgress,
            Self::RescopeRejected(s) => Self::RescopeRejected(s.clone()),
            Self::CatalogFailed(s) => Self::CatalogFailed(s.clone()),
            Self::CommitFailed(s) => Self::CommitFailed(s.clone()),
            Self::Ambiguous { candidates } => Self::Ambiguous {
                candidates: candidates.clone(),
            },
            Self::NotFound(s) => Self::NotFound(s.clone()),
            Self::NotConfigured { cloud } => Self::NotConfigured {
                cloud: cloud.clone(),
            },
            Self::Unsupported(s) => Self::Unsupported(s.clone()),
            // ApiError and io::Error are not Clone; we surface them as
            // preserved message strings on clone (only used for diagnostic
            // propagation inside the switcher, not public API).
            Self::Api(err) => Self::CommitFailed(format!("api: {err}")),
            Self::Io(err) => Self::CommitFailed(format!("io: {err}")),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_target() -> ContextTarget {
        ContextTarget {
            cloud: "devstack".to_string(),
            project_id: "abc".to_string(),
            project_name: "admin".to_string(),
            domain: "default".to_string(),
        }
    }

    #[test]
    fn in_progress_displays_human_readable() {
        let e = SwitchError::InProgress;
        assert_eq!(e.to_string(), "switch already in progress");
    }

    #[test]
    fn ambiguous_mentions_candidate_count() {
        let e = SwitchError::Ambiguous {
            candidates: vec![sample_target(), sample_target()],
        };
        assert_eq!(e.to_string(), "ambiguous target (2 candidates)");
    }

    #[test]
    fn rescope_rejected_preserves_message() {
        let e = SwitchError::RescopeRejected("forbidden".into());
        assert_eq!(e.to_string(), "rescope rejected: forbidden");
    }

    #[test]
    fn api_error_converts_via_from() {
        let api = ApiError::AuthFailed("bad creds".into());
        let switch: SwitchError = api.into();
        assert!(matches!(switch, SwitchError::Api(_)));
    }

    #[test]
    fn io_error_converts_via_from() {
        let io = std::io::Error::other("disk full");
        let switch: SwitchError = io.into();
        assert!(matches!(switch, SwitchError::Io(_)));
    }

    #[test]
    fn clone_preserves_plain_variants() {
        let e = SwitchError::NotFound("admin".into());
        match e.clone() {
            SwitchError::NotFound(s) => assert_eq!(s, "admin"),
            _ => panic!("expected NotFound"),
        }
    }

    #[test]
    fn clone_collapses_api_into_commit_failed_with_message() {
        let e = SwitchError::Api(ApiError::AuthFailed("bad".into()));
        match e.clone() {
            SwitchError::CommitFailed(msg) => assert!(msg.starts_with("api:")),
            _ => panic!("expected CommitFailed"),
        }
    }

    #[test]
    fn test_not_configured_displays_human_readable() {
        let e = SwitchError::NotConfigured {
            cloud: "prod".into(),
        };
        assert_eq!(
            e.to_string(),
            "cloud 'prod' has no default project — use :switch-project <name>"
        );
    }

    #[test]
    fn test_clone_preserves_not_configured() {
        let e = SwitchError::NotConfigured {
            cloud: "prod".into(),
        };
        match e.clone() {
            SwitchError::NotConfigured { cloud } => assert_eq!(cloud, "prod"),
            _ => panic!("expected NotConfigured"),
        }
    }
}
