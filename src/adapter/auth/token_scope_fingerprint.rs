//! Token-id → short hex fingerprint for cache keying (BL-P2-080).

use std::collections::hash_map::DefaultHasher;
use std::hash::{BuildHasher, BuildHasherDefault};

use crate::port::types::Token;

/// Computes a 16-char hex fingerprint from a [`Token`]'s `id` field.
///
/// Uses `std::collections::hash_map::DefaultHasher` via
/// `BuildHasherDefault` — stdlib only, no extra deps.
///
/// `BuildHasherDefault<DefaultHasher>` is a ZST so instantiating per-call
/// is zero-cost; in production you may store one instance and share it, but
/// per-call is semantically equivalent.
pub struct TokenScopeFingerprint {
    hasher_factory: BuildHasherDefault<DefaultHasher>,
}

impl TokenScopeFingerprint {
    /// Construct a new fingerprint factory. The underlying
    /// `BuildHasherDefault<DefaultHasher>` is a zero-sized type, so this
    /// constructor is effectively free to call; `Default` is provided for
    /// callers that prefer `TokenScopeFingerprint::default()`.
    pub fn new() -> Self {
        Self {
            hasher_factory: BuildHasherDefault::default(),
        }
    }

    /// Compute a 16-char lowercase hex fingerprint for the given token.
    pub fn compute(&self, token: &Token) -> String {
        format!("{:016x}", self.hasher_factory.hash_one(&token.id))
    }
}

impl Default for TokenScopeFingerprint {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::port::types::{CatalogEntry, ProjectScope, Token, TokenRole};
    use chrono::Utc;

    fn make_token(id: &str) -> Token {
        Token {
            id: id.to_string(),
            expires_at: Utc::now(),
            project: ProjectScope {
                id: "proj-1".into(),
                name: "admin".into(),
                domain_id: "default".into(),
                domain_name: "Default".into(),
            },
            roles: Vec::<TokenRole>::new(),
            catalog: Vec::<CatalogEntry>::new(),
            user_id: String::new(),
        }
    }

    #[test]
    fn same_token_same_fingerprint() {
        let fp = TokenScopeFingerprint::new();
        let token = make_token("gAAAAABsomeLongTokenString");
        let a = fp.compute(&token);
        let b = fp.compute(&token);
        assert_eq!(a, b);
    }

    #[test]
    fn different_tokens_different_fingerprints() {
        let fp = TokenScopeFingerprint::new();
        let t1 = make_token("tokenAAA");
        let t2 = make_token("tokenBBB");
        assert_ne!(fp.compute(&t1), fp.compute(&t2));
    }

    #[test]
    fn fingerprint_hex_length_16() {
        let fp = TokenScopeFingerprint::new();
        let token = make_token("any-token-id");
        let result = fp.compute(&token);
        assert_eq!(result.len(), 16, "fingerprint must be exactly 16 hex chars");
        assert!(
            result.chars().all(|c| c.is_ascii_hexdigit()),
            "fingerprint must be hex: {result}"
        );
    }
}
