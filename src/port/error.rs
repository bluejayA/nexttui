use thiserror::Error;

#[derive(Debug, Error)]
#[non_exhaustive]
pub enum ApiError {
    #[error("Authentication failed: {0}")]
    AuthFailed(String),

    #[error("Token expired")]
    TokenExpired,

    /// BL-P2-083: rescoped session reached near-expiry and cannot be
    /// auto-refreshed (the C1 guard in `KeystoneAuthAdapter::refresh_token`
    /// blocks refreshing tokens whose scope differs from the credential's
    /// initial scope — that's the rescope adapter's job, not the auth
    /// adapter's). Distinct from `AuthFailed` so callers can surface a
    /// "switch context to reauth" prompt rather than "credentials
    /// rejected." `project` carries the active project's name for the
    /// user-facing message.
    #[error("Session expired for project {project}; switch context to reauthenticate")]
    SessionExpired { project: String },

    #[error("Forbidden: {0}")]
    Forbidden(String),

    #[error("Not found: {resource_type} {id}")]
    NotFound { resource_type: String, id: String },

    #[error("Conflict: {0}")]
    Conflict(String),

    #[error("Bad request: {0}")]
    BadRequest(String),

    #[error("Rate limited: retry after {retry_after_secs}s")]
    RateLimited { retry_after_secs: u64 },

    #[error("Service unavailable: {service}")]
    ServiceUnavailable { service: String },

    #[error("Network error: {0}")]
    Network(#[from] reqwest::Error),

    #[error("Response parse error: {0}")]
    Parse(String),

    #[error("Unexpected: {status} {}", truncate_body(body))]
    Unexpected { status: u16, body: String },
}

pub type ApiResult<T> = Result<T, ApiError>;

fn truncate_body(body: &str) -> String {
    const MAX_LEN: usize = 200;
    if body.len() <= MAX_LEN {
        body.to_string()
    } else {
        format!("{}...[truncated]", &body[..MAX_LEN])
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_api_error_display() {
        let err = ApiError::AuthFailed("bad creds".into());
        assert!(err.to_string().contains("bad creds"));

        let err = ApiError::NotFound {
            resource_type: "server".into(),
            id: "abc".into(),
        };
        assert!(err.to_string().contains("server"));
        assert!(err.to_string().contains("abc"));

        let err = ApiError::RateLimited {
            retry_after_secs: 30,
        };
        assert!(err.to_string().contains("30"));

        let err = ApiError::Unexpected {
            status: 500,
            body: "internal".into(),
        };
        assert!(err.to_string().contains("500"));
    }
}
