//! Token cache persistence: save/load Keystone tokens to disk.
//!
//! Cache layout: `~/.cache/nexttui/auth/{cloud_key}/{scope_key}`
//! File permissions: 0o600 (Unix only)

use std::collections::HashMap;
use std::path::{Path, PathBuf};

use crate::port::types::{Token, TokenScope};

/// Struct wrapper over the cache directory, exposing scope-keyed
/// store/lookup. Used by the runtime context switch flow so callers don't
/// have to thread `&Path` everywhere.
#[derive(Debug, Clone)]
pub struct TokenCacheStore {
    cache_dir: PathBuf,
}

impl TokenCacheStore {
    pub fn new(cache_dir: impl Into<PathBuf>) -> Self {
        Self { cache_dir: cache_dir.into() }
    }

    pub fn cache_dir(&self) -> &Path {
        &self.cache_dir
    }

    /// Persist `token` under `scope`. Overwrites any existing entry.
    pub fn store_scoped(&self, scope: &TokenScope, token: &Token) -> std::io::Result<()> {
        save_token(token, &self.cache_dir, scope)
    }

    /// Returns the cached token for `scope`, or `None` if the file is missing
    /// or the stored token has expired (expired files are auto-removed).
    pub fn lookup_scoped(&self, scope: &TokenScope) -> Option<Token> {
        let path = self.cache_dir.join(scope.cache_key());
        load_token_file(&path)
    }
}

/// Compute a deterministic cache key from cloud config fields.
/// Uses a simple FNV-1a 64-bit hash (stable across Rust versions, no external deps).
pub fn compute_cloud_key(auth_url: &str, username: &str) -> String {
    let input = format!("{auth_url}|{username}");
    let hash = fnv1a_64(input.as_bytes());
    format!("{hash:016x}")
}

/// FNV-1a 64-bit hash — deterministic, no external dependency.
fn fnv1a_64(data: &[u8]) -> u64 {
    const FNV_OFFSET: u64 = 0xcbf29ce484222325;
    const FNV_PRIME: u64 = 0x00000100000001B3;
    let mut hash = FNV_OFFSET;
    for &byte in data {
        hash ^= byte as u64;
        hash = hash.wrapping_mul(FNV_PRIME);
    }
    hash
}

/// Resolve the cache directory path for a given cloud key.
pub fn cache_dir_path(cloud_key: &str) -> PathBuf {
    dirs::cache_dir()
        .unwrap_or_else(|| PathBuf::from("/tmp"))
        .join("nexttui")
        .join("auth")
        .join(cloud_key)
}

/// Save a token to the cache directory, keyed by scope.
/// Creates parent directories if needed.
/// On Unix, creates the file with 0o600 permissions atomically (no TOCTOU window).
pub fn save_token(token: &Token, cache_dir: &Path, scope: &TokenScope) -> Result<(), std::io::Error> {
    use std::io::Write;

    // Create cache directory with restricted permissions on Unix (0o700)
    #[cfg(unix)]
    {
        use std::fs::DirBuilder;
        use std::os::unix::fs::DirBuilderExt;
        DirBuilder::new().recursive(true).mode(0o700).create(cache_dir)?;
    }
    #[cfg(not(unix))]
    {
        std::fs::create_dir_all(cache_dir)?;
    }
    let path = cache_dir.join(scope.cache_key());
    let data = serde_json::to_vec(token)
        .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e))?;

    // NOTE: Token ID is stored in plaintext JSON. File permissions (0o600) provide
    // basic protection. Encryption (AES-GCM / OS keychain) is tracked as BL-P2-016.
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        let mut file = std::fs::OpenOptions::new()
            .write(true)
            .create(true)
            .truncate(true)
            .mode(0o600)
            .open(&path)?;
        file.write_all(&data)?;
    }

    #[cfg(not(unix))]
    {
        std::fs::write(&path, &data)?;
    }

    tracing::debug!(path = %path.display(), "token cached to disk");
    Ok(())
}

/// Load a single token from a cache file.
/// Returns None if the file doesn't exist, is unreadable, or the token is expired.
/// Automatically deletes expired token files.
fn load_token_file(path: &Path) -> Option<Token> {
    let data = std::fs::read(path).ok()?;
    let token: Token = serde_json::from_slice(&data).ok()?;

    if token.expires_at > chrono::Utc::now() + chrono::Duration::minutes(1) {
        Some(token)
    } else {
        tracing::info!(path = %path.display(), "cached token expired, removing");
        let _ = std::fs::remove_file(path);
        None
    }
}

/// Load all valid cached tokens from the cache directory.
/// Returns a map of scope → token. Expired tokens are auto-deleted.
/// Skips non-files and unrecognized filenames.
pub fn load_all_tokens(cache_dir: &Path) -> HashMap<TokenScope, Token> {
    let mut map = HashMap::new();
    let entries = match std::fs::read_dir(cache_dir) {
        Ok(entries) => entries,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return map,
        Err(e) => {
            tracing::warn!(path = %cache_dir.display(), error = %e, "failed to read token cache directory");
            return map;
        }
    };

    for entry in entries.flatten() {
        // Skip non-files (directories, symlinks, etc.)
        if !entry.file_type().map(|t| t.is_file()).unwrap_or(false) {
            continue;
        }

        let file_name = entry.file_name();
        let scope_key = file_name.to_string_lossy();

        let scope = match parse_scope_from_filename(&scope_key) {
            Some(s) => s,
            None => {
                tracing::warn!(filename = %scope_key, "unrecognized token cache file, skipping");
                continue;
            }
        };

        if let Some(token) = load_token_file(&entry.path()) {
            tracing::debug!("loaded cached token from disk");
            map.insert(scope, token);
        }
    }
    map
}

/// Parse a TokenScope from a cache filename.
/// Returns None for unrecognized filenames.
fn parse_scope_from_filename(filename: &str) -> Option<TokenScope> {
    if filename == "unscoped" {
        return Some(TokenScope::Unscoped);
    }
    // Format: "project@{name}@{domain}" (@ separator avoids _ ambiguity)
    if let Some(rest) = filename.strip_prefix("project@") {
        if let Some((name, domain)) = rest.split_once('@') {
            return Some(TokenScope::Project {
                name: name.to_string(),
                domain: domain.to_string(),
            });
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::{Duration, Utc};
    use crate::port::types::*;
    use tempfile::TempDir;

    fn sample_token(expires_in_minutes: i64) -> Token {
        Token {
            id: "tok-test-123".to_string(),
            expires_at: Utc::now() + Duration::minutes(expires_in_minutes),
            project: ProjectScope {
                id: "proj-1".to_string(),
                name: "admin".to_string(),
                domain_id: "default".to_string(),
                domain_name: "Default".to_string(),
            },
            roles: vec![TokenRole {
                id: "role-1".to_string(),
                name: "admin".to_string(),
            }],
            catalog: vec![CatalogEntry {
                service_type: "compute".to_string(),
                service_name: "nova".to_string(),
                endpoints: vec![Endpoint {
                    region: "RegionOne".to_string(),
                    interface: EndpointInterface::Public,
                    url: "https://nova:8774/v2.1".to_string(),
                }],
            }],
        }
    }

    fn sample_scope() -> TokenScope {
        TokenScope::Project {
            name: "admin".to_string(),
            domain: "default".to_string(),
        }
    }

    #[test]
    fn test_compute_cloud_key_deterministic() {
        let k1 = compute_cloud_key("https://keystone:5000/v3", "admin");
        let k2 = compute_cloud_key("https://keystone:5000/v3", "admin");
        assert_eq!(k1, k2);
        assert_eq!(k1.len(), 16);
    }

    #[test]
    fn test_compute_cloud_key_different_inputs() {
        let k1 = compute_cloud_key("https://keystone:5000/v3", "admin");
        let k2 = compute_cloud_key("https://keystone:5000/v3", "user");
        assert_ne!(k1, k2);
    }

    #[test]
    fn test_token_scope_cache_key() {
        let scope = TokenScope::Project {
            name: "admin".to_string(),
            domain: "default".to_string(),
        };
        assert_eq!(scope.cache_key(), "project@admin@default");
        assert_eq!(TokenScope::Unscoped.cache_key(), "unscoped");
    }

    #[test]
    fn test_cache_key_sanitizes_path_traversal() {
        let scope = TokenScope::Project {
            name: "../etc".to_string(),
            domain: "default".to_string(),
        };
        // dots and slashes should be replaced with _
        assert!(!scope.cache_key().contains('/'));
        assert!(!scope.cache_key().contains(".."));
    }

    #[test]
    fn test_cache_key_handles_underscore_in_name() {
        let scope = TokenScope::Project {
            name: "my_project".to_string(),
            domain: "my_domain".to_string(),
        };
        // @ separator means underscores in name/domain are preserved correctly
        assert_eq!(scope.cache_key(), "project@my_project@my_domain");
    }

    #[test]
    fn test_save_and_load_scoped_token() {
        let dir = TempDir::new().unwrap();
        let cache_dir = dir.path().join("cloud-abc");
        let scope = sample_scope();

        let token = sample_token(60);
        save_token(&token, &cache_dir, &scope).unwrap();

        let loaded = load_all_tokens(&cache_dir);
        assert_eq!(loaded.len(), 1);
        let loaded_token = loaded.get(&scope).unwrap();
        assert_eq!(loaded_token.id, "tok-test-123");
    }

    #[test]
    fn test_save_multiple_scopes() {
        let dir = TempDir::new().unwrap();
        let cache_dir = dir.path().join("cloud-multi");

        let scope_a = TokenScope::Project {
            name: "projectA".to_string(),
            domain: "Default".to_string(),
        };
        let scope_b = TokenScope::Project {
            name: "projectB".to_string(),
            domain: "Default".to_string(),
        };

        let mut token_a = sample_token(60);
        token_a.id = "tok-a".to_string();
        let mut token_b = sample_token(60);
        token_b.id = "tok-b".to_string();

        save_token(&token_a, &cache_dir, &scope_a).unwrap();
        save_token(&token_b, &cache_dir, &scope_b).unwrap();

        let loaded = load_all_tokens(&cache_dir);
        assert_eq!(loaded.len(), 2);
        assert_eq!(loaded.get(&scope_a).unwrap().id, "tok-a");
        assert_eq!(loaded.get(&scope_b).unwrap().id, "tok-b");
    }

    #[test]
    fn test_load_expired_token_deleted() {
        let dir = TempDir::new().unwrap();
        let cache_dir = dir.path().join("cloud-expired");
        let scope = sample_scope();

        let token = sample_token(-10);
        save_token(&token, &cache_dir, &scope).unwrap();

        let loaded = load_all_tokens(&cache_dir);
        assert!(loaded.is_empty());
        assert!(!cache_dir.join(scope.cache_key()).exists());
    }

    #[test]
    fn test_load_nonexistent_dir_returns_empty() {
        let path = PathBuf::from("/tmp/nexttui-test-nonexistent-dir");
        let loaded = load_all_tokens(&path);
        assert!(loaded.is_empty());
    }

    #[test]
    fn test_load_corrupt_file_skipped() {
        let dir = TempDir::new().unwrap();
        let cache_dir = dir.path().join("cloud-corrupt");
        std::fs::create_dir_all(&cache_dir).unwrap();
        std::fs::write(cache_dir.join("project_bad_Default"), b"not json").unwrap();

        let loaded = load_all_tokens(&cache_dir);
        assert!(loaded.is_empty());
    }

    #[cfg(unix)]
    #[test]
    fn test_save_sets_permissions_0o600() {
        use std::os::unix::fs::PermissionsExt;

        let dir = TempDir::new().unwrap();
        let cache_dir = dir.path().join("cloud-perm");
        let scope = sample_scope();

        let token = sample_token(60);
        save_token(&token, &cache_dir, &scope).unwrap();

        let path = cache_dir.join(scope.cache_key());
        let perms = std::fs::metadata(&path).unwrap().permissions();
        assert_eq!(perms.mode() & 0o777, 0o600);
    }

    #[test]
    fn test_parse_scope_from_filename() {
        assert_eq!(
            parse_scope_from_filename("project@admin@default"),
            Some(TokenScope::Project { name: "admin".to_string(), domain: "default".to_string() })
        );
        assert_eq!(parse_scope_from_filename("unscoped"), Some(TokenScope::Unscoped));
        assert_eq!(parse_scope_from_filename("unknown_file"), None);
    }

    #[test]
    fn test_parse_scope_with_underscore_in_name() {
        assert_eq!(
            parse_scope_from_filename("project@my_project@my_domain"),
            Some(TokenScope::Project { name: "my_project".to_string(), domain: "my_domain".to_string() })
        );
    }

    #[test]
    fn test_token_scope_from_credential() {
        let cred = AuthCredential {
            auth_url: "https://keystone:5000/v3".to_string(),
            method: AuthMethod::Password {
                username: "admin".to_string(),
                password: "pass".to_string(),
                domain_name: "Default".to_string(),
            },
            project_scope: Some(ProjectScopeParam {
                name: "admin".to_string(),
                domain_name: "Default".to_string(),
            }),
        };
        assert_eq!(
            TokenScope::from_credential(&cred),
            TokenScope::Project { name: "admin".to_string(), domain: "default".to_string() }
        );

        let unsoped_cred = AuthCredential {
            auth_url: "https://keystone:5000/v3".to_string(),
            method: AuthMethod::Password {
                username: "admin".to_string(),
                password: "pass".to_string(),
                domain_name: "Default".to_string(),
            },
            project_scope: None,
        };
        assert_eq!(TokenScope::from_credential(&unsoped_cred), TokenScope::Unscoped);
    }

    #[test]
    fn store_scoped_then_lookup_scoped_returns_same_token() {
        let tmp = TempDir::new().unwrap();
        let store = TokenCacheStore::new(tmp.path());
        let scope = sample_scope();
        let token = sample_token(60);

        store.store_scoped(&scope, &token).unwrap();
        let loaded = store.lookup_scoped(&scope).expect("token present");
        assert_eq!(loaded.id, token.id);
    }

    #[test]
    fn lookup_scoped_returns_none_for_missing_scope() {
        let tmp = TempDir::new().unwrap();
        let store = TokenCacheStore::new(tmp.path());
        let scope = TokenScope::Project {
            name: "demo".into(),
            domain: "default".into(),
        };
        assert!(store.lookup_scoped(&scope).is_none());
    }

    #[test]
    fn lookup_scoped_drops_expired_entry() {
        let tmp = TempDir::new().unwrap();
        let store = TokenCacheStore::new(tmp.path());
        let scope = sample_scope();
        // Expired token (expires 5 minutes ago).
        let token = sample_token(-5);
        store.store_scoped(&scope, &token).unwrap();
        assert!(store.lookup_scoped(&scope).is_none());
    }

    #[test]
    fn store_scoped_overwrites_previous_entry() {
        let tmp = TempDir::new().unwrap();
        let store = TokenCacheStore::new(tmp.path());
        let scope = sample_scope();
        let old = sample_token(30);
        store.store_scoped(&scope, &old).unwrap();
        let mut new = sample_token(60);
        new.id = "replacement".into();
        store.store_scoped(&scope, &new).unwrap();
        assert_eq!(store.lookup_scoped(&scope).unwrap().id, "replacement");
    }
}
