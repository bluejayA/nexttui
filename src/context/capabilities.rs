//! Keystone feature flags that determine whether runtime rescoping is viable
//! for the active auth session.

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AuthMethod {
    Password,
    Token,
    AppCredential,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum KeystoneVersion {
    V3,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct KeystoneCapabilities {
    pub allow_rescope_scoped_token: bool,
    pub auth_method: AuthMethod,
    pub api_version: KeystoneVersion,
}

impl KeystoneCapabilities {
    /// Deployments that forbid scoped-token rescoping require a full
    /// re-authentication path (see FR-6).
    pub fn supports_runtime_rescope(&self) -> bool {
        self.allow_rescope_scoped_token && self.auth_method != AuthMethod::AppCredential
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn password_auth_with_rescope_allowed_supports_runtime_rescope() {
        let caps = KeystoneCapabilities {
            allow_rescope_scoped_token: true,
            auth_method: AuthMethod::Password,
            api_version: KeystoneVersion::V3,
        };
        assert!(caps.supports_runtime_rescope());
    }

    #[test]
    fn app_credential_does_not_support_runtime_rescope() {
        let caps = KeystoneCapabilities {
            allow_rescope_scoped_token: true,
            auth_method: AuthMethod::AppCredential,
            api_version: KeystoneVersion::V3,
        };
        assert!(!caps.supports_runtime_rescope());
    }

    #[test]
    fn rescope_disabled_is_not_supported_even_for_password() {
        let caps = KeystoneCapabilities {
            allow_rescope_scoped_token: false,
            auth_method: AuthMethod::Password,
            api_version: KeystoneVersion::V3,
        };
        assert!(!caps.supports_runtime_rescope());
    }
}
