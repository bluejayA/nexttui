use std::path::PathBuf;
use thiserror::Error;

#[derive(Error, Debug)]
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

    #[error("{0}")]
    Other(String),
}

pub type Result<T> = std::result::Result<T, AppError>;

#[cfg(test)]
mod tests {
    use super::*;

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
