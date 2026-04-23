//! Per-cloud TTL cache for project directory lookups (BL-P2-080).

use std::collections::HashMap;
use std::sync::RwLock;
use std::time::{Duration, Instant};

use crate::context::resolver::ProjectCandidate;

type CacheKey = (String, String);
type CacheValue = (Vec<ProjectCandidate>, Instant);
type CacheStore = RwLock<HashMap<CacheKey, CacheValue>>;

/// In-memory, per-cloud, per-token-fingerprint cache for `ProjectCandidate`
/// lists. Entries expire after the configured TTL. Thread-safe via `RwLock`.
pub struct DirectoryCache {
    ttl: Duration,
    inner: CacheStore,
}

impl DirectoryCache {
    /// Create a new cache with the given TTL.
    pub fn new(ttl: Duration) -> Self {
        Self {
            ttl,
            inner: RwLock::new(HashMap::new()),
        }
    }

    /// Return a clone of the cached candidates for `(cloud, fingerprint)`, or
    /// `None` if the entry is absent or has expired.
    pub fn get(&self, cloud: &str, fp: &str) -> Option<Vec<ProjectCandidate>> {
        let guard = self.inner.read().ok()?;
        let key = (cloud.to_string(), fp.to_string());
        match guard.get(&key) {
            Some((candidates, inserted_at)) if inserted_at.elapsed() < self.ttl => {
                Some(candidates.clone())
            }
            _ => None,
        }
    }

    /// Insert or overwrite the entry for `(cloud, fingerprint)`.
    pub fn put(&self, cloud: &str, fp: &str, candidates: Vec<ProjectCandidate>) {
        if let Ok(mut guard) = self.inner.write() {
            guard.insert(
                (cloud.to_string(), fp.to_string()),
                (candidates, Instant::now()),
            );
        }
    }

    /// Remove all cached entries whose cloud prefix matches `cloud`.
    pub fn invalidate_cloud(&self, cloud: &str) {
        if let Ok(mut guard) = self.inner.write() {
            guard.retain(|(c, _), _| c != cloud);
        }
        tracing::info!(cloud, "directory_cache_invalidated");
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn candidate(id: &str) -> ProjectCandidate {
        ProjectCandidate {
            cloud: "devstack".into(),
            project_id: id.into(),
            project_name: format!("proj-{id}"),
            domain: "default".into(),
        }
    }

    #[test]
    fn get_miss_returns_none() {
        let cache = DirectoryCache::new(Duration::from_secs(60));
        assert!(cache.get("devstack", "fp-abc").is_none());
    }

    #[test]
    fn put_then_get_returns_clone() {
        let cache = DirectoryCache::new(Duration::from_secs(60));
        let candidates = vec![candidate("p1"), candidate("p2")];
        cache.put("devstack", "fp-abc", candidates.clone());
        let result = cache.get("devstack", "fp-abc").expect("cache hit expected");
        assert_eq!(result, candidates);
    }

    #[test]
    fn get_expired_returns_none() {
        let cache = DirectoryCache::new(Duration::from_millis(5));
        cache.put("devstack", "fp-abc", vec![candidate("p1")]);
        std::thread::sleep(Duration::from_millis(10));
        assert!(cache.get("devstack", "fp-abc").is_none());
    }

    #[test]
    fn invalidate_cloud_removes_all_matching() {
        let cache = DirectoryCache::new(Duration::from_secs(60));
        cache.put("devstack", "fp-a", vec![candidate("p1")]);
        cache.put("devstack", "fp-b", vec![candidate("p2")]);
        cache.put("prod", "fp-c", vec![candidate("p3")]);

        cache.invalidate_cloud("devstack");

        assert!(cache.get("devstack", "fp-a").is_none());
        assert!(cache.get("devstack", "fp-b").is_none());
        assert!(cache.get("prod", "fp-c").is_some());
    }

    #[test]
    fn different_token_fingerprints_segregate() {
        let cache = DirectoryCache::new(Duration::from_secs(60));
        cache.put("devstack", "fp-A", vec![candidate("p1")]);
        assert!(cache.get("devstack", "fp-B").is_none());
    }
}
