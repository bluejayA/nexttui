use std::path::PathBuf;
use thiserror::Error;

use crate::infra::cross_project_guard::{CrossProjectReason, GuardLayer};

#[derive(Error, Debug)]
#[non_exhaustive]
pub enum AppError {
    #[error("clouds.yaml not found. Searched: {searched_paths:?}")]
    CloudsYamlNotFound { searched_paths: Vec<PathBuf> },

    #[error("Failed to parse {path}: {source}")]
    ConfigParse {
        path: PathBuf,
        #[source]
        source: Box<dyn std::error::Error + Send + Sync>,
    },

    #[error("Config validation failed: {message}")]
    ConfigValidation { message: String },

    #[error("Cloud '{name}' not found. Available: {available:?}")]
    CloudNotFound {
        name: String,
        available: Vec<String>,
    },

    #[error("API request failed: {message}")]
    Api {
        message: String,
        status: Option<u16>,
    },

    #[error("Authentication failed: {message}")]
    Auth { message: String },

    #[error("IO error: {source}")]
    Io {
        #[from]
        source: std::io::Error,
    },

    #[error(
        "Cross-project operation blocked: {r} (layer: {l})",
        r = reason.as_str(),
        l = guard_layer.as_str()
    )]
    CrossProjectBlocked {
        reason: CrossProjectReason,
        guard_layer: GuardLayer,
    },

    #[error("{0}")]
    Other(String),
}

pub type Result<T> = std::result::Result<T, AppError>;

#[cfg(test)]
mod tests {
    use super::*;
    use crate::infra::cross_project_guard::{CrossProjectReason, GuardLayer};

    #[test]
    fn test_cross_project_blocked_error_display() {
        let err = AppError::CrossProjectBlocked {
            reason: CrossProjectReason::OriginScopeMismatch {
                origin: "admin-uuid".to_string(),
                active: "demo-uuid".to_string(),
            },
            guard_layer: GuardLayer::Fr2Worker,
        };
        let msg = err.to_string();
        assert!(
            msg.to_lowercase().contains("cross-project"),
            "missing cross-project label: {msg}"
        );
        assert!(
            msg.to_lowercase().contains("blocked"),
            "missing 'blocked': {msg}"
        );
        assert!(
            msg.contains("origin_scope_mismatch"),
            "missing reason as_str(): {msg}"
        );
        assert!(
            msg.contains("fr2_worker"),
            "missing guard layer as_str(): {msg}"
        );

        let err = AppError::CrossProjectBlocked {
            reason: CrossProjectReason::FormSelectionMismatch {
                selected: "demo-uuid".to_string(),
                active: "admin-uuid".to_string(),
            },
            guard_layer: GuardLayer::Fr4Form,
        };
        let msg = err.to_string();
        assert!(
            msg.contains("form_selection_mismatch"),
            "form mismatch reason missing: {msg}"
        );
        assert!(msg.contains("fr4_form"), "fr4 layer missing: {msg}");
    }

    #[test]
    fn test_app_error_display() {
        let err = AppError::CloudsYamlNotFound {
            searched_paths: vec![PathBuf::from("/a"), PathBuf::from("/b")],
        };
        assert!(err.to_string().contains("clouds.yaml not found"));
        assert!(err.to_string().contains("/a"));

        let err = AppError::ConfigValidation {
            message: "missing auth".to_string(),
        };
        assert_eq!(err.to_string(), "Config validation failed: missing auth");

        let err = AppError::CloudNotFound {
            name: "prod".to_string(),
            available: vec!["dev".to_string()],
        };
        assert!(err.to_string().contains("prod"));
        assert!(err.to_string().contains("dev"));

        let err = AppError::Api {
            message: "timeout".to_string(),
            status: Some(503),
        };
        assert!(err.to_string().contains("timeout"));

        let err = AppError::Auth {
            message: "invalid token".to_string(),
        };
        assert!(err.to_string().contains("invalid token"));

        let err = AppError::Other("something".to_string());
        assert_eq!(err.to_string(), "something");
    }
}
