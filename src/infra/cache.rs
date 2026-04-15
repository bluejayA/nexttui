use std::any::Any;
use std::collections::HashMap;
use std::sync::RwLock;
use std::time::{Duration, Instant};

use crate::config::CacheTtlConfig;
use crate::models::common::ResourceType;

const DEFAULT_MAX_ENTRIES: usize = 1024;

#[derive(Debug, Clone, Hash, Eq, PartialEq)]
pub struct CacheKey {
    pub resource_type: ResourceType,
    pub cloud: String,
    pub qualifier: Option<String>,
}

struct CacheEntry {
    data: Box<dyn Any + Send + Sync>,
    inserted_at: Instant,
    ttl: Duration,
}

impl CacheEntry {
    fn is_expired(&self) -> bool {
        self.inserted_at.elapsed() > self.ttl
    }
}

#[derive(Debug, Clone, Default)]
pub struct CacheStats {
    pub total_entries: usize,
    pub valid_entries: usize,
    pub expired_entries: usize,
}

pub struct Cache {
    entries: RwLock<HashMap<CacheKey, CacheEntry>>,
    ttl_config: HashMap<ResourceType, Duration>,
    max_entries: usize,
}

impl Cache {
    pub fn new(config: &CacheTtlConfig) -> Self {
        Self::with_max_entries(config, DEFAULT_MAX_ENTRIES)
    }

    pub fn with_max_entries(config: &CacheTtlConfig, max_entries: usize) -> Self {
        let mut ttl_config = HashMap::new();
        ttl_config.insert(
            ResourceType::Servers,
            Duration::from_secs(config.servers_secs),
        );
        ttl_config.insert(
            ResourceType::Networks,
            Duration::from_secs(config.networks_secs),
        );
        ttl_config.insert(
            ResourceType::Flavors,
            Duration::from_secs(config.flavors_secs),
        );
        ttl_config.insert(
            ResourceType::Images,
            Duration::from_secs(config.images_secs),
        );
        ttl_config.insert(
            ResourceType::SecurityGroups,
            Duration::from_secs(config.security_groups_secs),
        );
        ttl_config.insert(
            ResourceType::Volumes,
            Duration::from_secs(config.volumes_secs),
        );
        ttl_config.insert(
            ResourceType::Snapshots,
            Duration::from_secs(config.volumes_secs),
        );
        ttl_config.insert(
            ResourceType::Projects,
            Duration::from_secs(config.projects_secs),
        );
        ttl_config.insert(
            ResourceType::Users,
            Duration::from_secs(config.projects_secs),
        );
        ttl_config.insert(
            ResourceType::FloatingIps,
            Duration::from_secs(config.networks_secs),
        );
        ttl_config.insert(
            ResourceType::Aggregates,
            Duration::from_secs(config.servers_secs),
        );
        ttl_config.insert(
            ResourceType::ComputeServices,
            Duration::from_secs(config.servers_secs),
        );
        ttl_config.insert(
            ResourceType::Hypervisors,
            Duration::from_secs(config.servers_secs),
        );
        ttl_config.insert(
            ResourceType::Agents,
            Duration::from_secs(config.networks_secs),
        );
        Self {
            entries: RwLock::new(HashMap::new()),
            ttl_config,
            max_entries,
        }
    }

    /// Get cached data if present and not expired.
    /// Returns `None` on miss, expiry, or type mismatch.
    pub fn get<T: 'static + Send + Sync + Clone>(&self, key: &CacheKey) -> Option<T> {
        let entries = self.entries.read().ok()?;
        let entry = entries.get(key)?;
        if entry.is_expired() {
            return None;
        }
        entry.data.downcast_ref::<T>().cloned()
    }

    /// Insert data with resource-type-specific TTL.
    /// Evicts expired entries when cache exceeds max_entries.
    pub fn put<T: 'static + Send + Sync>(&self, key: CacheKey, data: T) {
        let ttl = self
            .ttl_config
            .get(&key.resource_type)
            .copied()
            .unwrap_or(Duration::from_secs(120));
        let entry = CacheEntry {
            data: Box::new(data),
            inserted_at: Instant::now(),
            ttl,
        };
        if let Ok(mut entries) = self.entries.write() {
            // Evict expired entries if at capacity
            if entries.len() >= self.max_entries {
                entries.retain(|_, v| !v.is_expired());
            }
            entries.insert(key, entry);
        }
    }

    /// Invalidate a specific resource type for a cloud.
    pub fn invalidate(&self, resource_type: ResourceType, cloud: &str) {
        if let Ok(mut entries) = self.entries.write() {
            entries.retain(|k, _| !(k.resource_type == resource_type && k.cloud == cloud));
        }
    }

    /// Invalidate ALL entries for a cloud (used on cloud context switch).
    pub fn invalidate_cloud(&self, cloud: &str) {
        if let Ok(mut entries) = self.entries.write() {
            entries.retain(|k, _| k.cloud != cloud);
        }
    }

    /// Invalidate everything (`:refresh` with no args).
    pub fn invalidate_all(&self) {
        if let Ok(mut entries) = self.entries.write() {
            entries.clear();
        }
    }

    /// Check if a key has a valid (non-expired) entry.
    pub fn is_valid(&self, key: &CacheKey) -> bool {
        let entries = match self.entries.read() {
            Ok(e) => e,
            Err(_) => return false,
        };
        entries.get(key).is_some_and(|e| !e.is_expired())
    }

    /// Remove all expired entries (called periodically from on_tick).
    pub fn gc_expired(&self) {
        if let Ok(mut entries) = self.entries.write() {
            entries.retain(|_, v| !v.is_expired());
        }
    }

    /// Get cache stats for status bar display.
    pub fn stats(&self) -> CacheStats {
        let entries = match self.entries.read() {
            Ok(e) => e,
            Err(_) => return CacheStats::default(),
        };
        let total = entries.len();
        let expired = entries.values().filter(|e| e.is_expired()).count();
        CacheStats {
            total_entries: total,
            valid_entries: total - expired,
            expired_entries: expired,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn default_cache() -> Cache {
        Cache::new(&CacheTtlConfig::default())
    }

    fn key(rt: ResourceType, cloud: &str) -> CacheKey {
        CacheKey {
            resource_type: rt,
            cloud: cloud.to_string(),
            qualifier: None,
        }
    }

    #[test]
    fn test_put_and_get() {
        let cache = default_cache();
        let k = key(ResourceType::Servers, "prod");
        cache.put(
            k.clone(),
            vec!["server1".to_string(), "server2".to_string()],
        );
        let result: Option<Vec<String>> = cache.get(&k);
        assert_eq!(result.unwrap().len(), 2);
    }

    #[test]
    fn test_get_miss() {
        let cache = default_cache();
        let k = key(ResourceType::Servers, "prod");
        let result: Option<Vec<String>> = cache.get(&k);
        assert!(result.is_none());
    }

    #[test]
    fn test_get_wrong_type_returns_none() {
        let cache = default_cache();
        let k = key(ResourceType::Servers, "prod");
        cache.put(k.clone(), vec!["server1".to_string()]);
        let result: Option<Vec<u32>> = cache.get(&k);
        assert!(result.is_none());
    }

    #[test]
    fn test_ttl_expiry() {
        let config = CacheTtlConfig {
            servers_secs: 0,
            ..CacheTtlConfig::default()
        };
        let cache = Cache::new(&config);
        let k = key(ResourceType::Servers, "prod");
        cache.put(k.clone(), vec!["server1".to_string()]);
        let result: Option<Vec<String>> = cache.get(&k);
        assert!(result.is_none());
        assert!(!cache.is_valid(&k));
    }

    #[test]
    fn test_invalidate_specific() {
        let cache = default_cache();
        let k1 = key(ResourceType::Servers, "prod");
        let k2 = key(ResourceType::Networks, "prod");
        cache.put(k1.clone(), vec!["s1".to_string()]);
        cache.put(k2.clone(), vec!["n1".to_string()]);
        cache.invalidate(ResourceType::Servers, "prod");
        assert!(!cache.is_valid(&k1));
        assert!(cache.is_valid(&k2));
    }

    #[test]
    fn test_invalidate_cloud() {
        let cache = default_cache();
        let k1 = key(ResourceType::Servers, "prod");
        let k2 = key(ResourceType::Servers, "staging");
        cache.put(k1.clone(), vec!["s1".to_string()]);
        cache.put(k2.clone(), vec!["s2".to_string()]);
        cache.invalidate_cloud("prod");
        assert!(!cache.is_valid(&k1));
        assert!(cache.is_valid(&k2));
    }

    #[test]
    fn test_invalidate_all() {
        let cache = default_cache();
        cache.put(key(ResourceType::Servers, "prod"), vec!["s1".to_string()]);
        cache.put(
            key(ResourceType::Networks, "staging"),
            vec!["n1".to_string()],
        );
        cache.invalidate_all();
        assert_eq!(cache.stats().total_entries, 0);
    }

    #[test]
    fn test_gc_expired() {
        let config = CacheTtlConfig {
            servers_secs: 0,
            networks_secs: 3600,
            ..CacheTtlConfig::default()
        };
        let cache = Cache::new(&config);
        cache.put(key(ResourceType::Servers, "prod"), vec!["s1".to_string()]);
        cache.put(key(ResourceType::Networks, "prod"), vec!["n1".to_string()]);
        cache.gc_expired();
        assert_eq!(cache.stats().total_entries, 1);
    }

    #[test]
    fn test_stats() {
        let cache = default_cache();
        cache.put(key(ResourceType::Servers, "prod"), vec!["s1".to_string()]);
        cache.put(key(ResourceType::Networks, "prod"), vec!["n1".to_string()]);
        let stats = cache.stats();
        assert_eq!(stats.total_entries, 2);
        assert_eq!(stats.valid_entries, 2);
        assert_eq!(stats.expired_entries, 0);
    }

    #[test]
    fn test_qualifier_differentiates_keys() {
        let cache = default_cache();
        let k1 = CacheKey {
            resource_type: ResourceType::Servers,
            cloud: "prod".to_string(),
            qualifier: Some("project-a".to_string()),
        };
        let k2 = CacheKey {
            resource_type: ResourceType::Servers,
            cloud: "prod".to_string(),
            qualifier: Some("project-b".to_string()),
        };
        cache.put(k1.clone(), vec!["s1".to_string()]);
        cache.put(k2.clone(), vec!["s2".to_string()]);
        let r1: Vec<String> = cache.get(&k1).unwrap();
        let r2: Vec<String> = cache.get(&k2).unwrap();
        assert_eq!(r1, vec!["s1"]);
        assert_eq!(r2, vec!["s2"]);
    }

    #[test]
    fn test_max_entries_evicts_expired() {
        let config = CacheTtlConfig {
            servers_secs: 0, // instant expiry
            networks_secs: 3600,
            ..CacheTtlConfig::default()
        };
        let cache = Cache::with_max_entries(&config, 2);

        // Fill cache with 2 expired server entries
        cache.put(key(ResourceType::Servers, "a"), vec!["s1".to_string()]);
        cache.put(key(ResourceType::Servers, "b"), vec!["s2".to_string()]);
        assert_eq!(cache.stats().total_entries, 2);

        // Adding a 3rd entry should evict expired ones
        cache.put(key(ResourceType::Networks, "a"), vec!["n1".to_string()]);
        // Expired entries should be gone, only the new one remains
        assert_eq!(cache.stats().total_entries, 1);
        assert!(cache.is_valid(&key(ResourceType::Networks, "a")));
    }
}
