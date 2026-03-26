//! Token cache persistence: save/load Keystone tokens to disk.
//!
//! Cache location: `~/.cache/nexttui/auth/{cache_key}`
//! File permissions: 0o600 (Unix only)

use std::path::{Path, PathBuf};

use crate::port::types::Token;

/// Compute a deterministic cache key from cloud config fields.
/// Uses a simple FNV-1a 64-bit hash (stable across Rust versions, no external deps).
pub fn compute_cache_key(auth_url: &str, username: &str, project_name: Option<&str>) -> String {
    let input = format!(
        "{}|{}|{}",
        auth_url,
        username,
        project_name.unwrap_or("")
    );
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

/// Resolve the cache file path for a given cache key.
pub fn cache_file_path(cache_key: &str) -> PathBuf {
    dirs::cache_dir()
        .unwrap_or_else(|| PathBuf::from("/tmp"))
        .join("nexttui")
        .join("auth")
        .join(cache_key)
}

/// Save a token to the cache file.
/// Creates parent directories if needed.
/// On Unix, creates the file with 0o600 permissions atomically (no TOCTOU window).
pub fn save_token(token: &Token, path: &Path) -> Result<(), std::io::Error> {
    use std::io::Write;

    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
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
            .open(path)?;
        file.write_all(&data)?;
    }

    #[cfg(not(unix))]
    {
        std::fs::write(path, &data)?;
    }

    tracing::info!(path = %path.display(), "token cached to disk");
    Ok(())
}

/// Load a token from the cache file.
/// Returns None if the file doesn't exist, is unreadable, or the token is expired.
/// Automatically deletes expired token files.
pub fn load_token(path: &Path) -> Option<Token> {
    let data = std::fs::read(path).ok()?;
    let token: Token = serde_json::from_slice(&data).ok()?;

    if token.expires_at > chrono::Utc::now() + chrono::Duration::minutes(1) {
        tracing::info!(path = %path.display(), "loaded cached token from disk");
        Some(token)
    } else {
        tracing::info!(path = %path.display(), "cached token expired, removing");
        let _ = std::fs::remove_file(path);
        None
    }
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

    #[test]
    fn test_compute_cache_key_deterministic() {
        let k1 = compute_cache_key("https://keystone:5000/v3", "admin", Some("admin"));
        let k2 = compute_cache_key("https://keystone:5000/v3", "admin", Some("admin"));
        assert_eq!(k1, k2);
        assert_eq!(k1.len(), 16); // 16 hex chars
    }

    #[test]
    fn test_compute_cache_key_different_inputs() {
        let k1 = compute_cache_key("https://keystone:5000/v3", "admin", Some("admin"));
        let k2 = compute_cache_key("https://keystone:5000/v3", "user", Some("project1"));
        assert_ne!(k1, k2);
    }

    #[test]
    fn test_save_and_load_token() {
        let dir = TempDir::new().unwrap();
        let path = dir.path().join("auth").join("test-key");

        let token = sample_token(60); // expires in 60 minutes
        save_token(&token, &path).unwrap();

        let loaded = load_token(&path);
        assert!(loaded.is_some());
        let loaded = loaded.unwrap();
        assert_eq!(loaded.id, "tok-test-123");
        assert_eq!(loaded.project.name, "admin");
        assert_eq!(loaded.roles.len(), 1);
        assert_eq!(loaded.catalog.len(), 1);
    }

    #[test]
    fn test_load_expired_token_returns_none_and_deletes() {
        let dir = TempDir::new().unwrap();
        let path = dir.path().join("expired-token");

        let token = sample_token(-10); // expired 10 minutes ago
        save_token(&token, &path).unwrap();
        assert!(path.exists());

        let loaded = load_token(&path);
        assert!(loaded.is_none());
        assert!(!path.exists()); // file should be deleted
    }

    #[test]
    fn test_load_nonexistent_file_returns_none() {
        let path = PathBuf::from("/tmp/nexttui-test-nonexistent-token");
        assert!(load_token(&path).is_none());
    }

    #[test]
    fn test_load_corrupt_file_returns_none() {
        let dir = TempDir::new().unwrap();
        let path = dir.path().join("corrupt-token");
        std::fs::write(&path, b"not valid json").unwrap();

        assert!(load_token(&path).is_none());
    }

    #[cfg(unix)]
    #[test]
    fn test_save_sets_permissions_0o600() {
        use std::os::unix::fs::PermissionsExt;

        let dir = TempDir::new().unwrap();
        let path = dir.path().join("perm-token");

        let token = sample_token(60);
        save_token(&token, &path).unwrap();

        let perms = std::fs::metadata(&path).unwrap().permissions();
        assert_eq!(perms.mode() & 0o777, 0o600);
    }

    #[test]
    fn test_save_creates_parent_directories() {
        let dir = TempDir::new().unwrap();
        let path = dir.path().join("nested").join("dirs").join("token");

        let token = sample_token(60);
        save_token(&token, &path).unwrap();
        assert!(path.exists());
    }

    #[test]
    fn test_token_serialization_roundtrip() {
        let token = sample_token(60);
        let json = serde_json::to_vec(&token).unwrap();
        let deserialized: Token = serde_json::from_slice(&json).unwrap();
        assert_eq!(deserialized.id, token.id);
        assert_eq!(deserialized.project.name, token.project.name);
    }
}
