//! Iterates every registered [`HttpEndpointCache`] and clears its endpoint
//! cache. Invoked by [`ScopedAuthSession`](crate::adapter::auth::scoped_session)
//! whenever the active scope changes so subsequent requests re-resolve the
//! URL against the fresh service catalog.
//!
//! BL-P2-031 Unit 3.

use std::sync::Arc;

use crate::port::http_endpoint_cache::HttpEndpointCache;

pub struct EndpointCatalogInvalidator {
    caches: Vec<Arc<dyn HttpEndpointCache>>,
}

impl EndpointCatalogInvalidator {
    pub fn new(caches: Vec<Arc<dyn HttpEndpointCache>>) -> Self {
        Self { caches }
    }

    pub fn empty() -> Self {
        Self { caches: Vec::new() }
    }

    pub fn with_cache(mut self, cache: Arc<dyn HttpEndpointCache>) -> Self {
        self.caches.push(cache);
        self
    }

    pub fn len(&self) -> usize {
        self.caches.len()
    }

    pub fn is_empty(&self) -> bool {
        self.caches.is_empty()
    }

    /// Invalidate every registered cache. Each call is awaited sequentially
    /// — the caches are cheap in-memory operations and we want predictable
    /// ordering for failure diagnostics.
    pub async fn invalidate_all(&self) {
        for cache in &self.caches {
            cache.invalidate().await;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::port::mock_context::MockHttpEndpointCache;

    #[tokio::test]
    async fn empty_invalidator_is_a_noop() {
        let inv = EndpointCatalogInvalidator::empty();
        assert_eq!(inv.len(), 0);
        assert!(inv.is_empty());
        inv.invalidate_all().await; // must not panic
    }

    #[tokio::test]
    async fn invalidate_all_forwards_to_every_cache() {
        let a = Arc::new(MockHttpEndpointCache::new());
        let b = Arc::new(MockHttpEndpointCache::new());
        let inv = EndpointCatalogInvalidator::new(vec![a.clone(), b.clone()]);
        assert_eq!(inv.len(), 2);

        inv.invalidate_all().await;
        inv.invalidate_all().await;

        assert_eq!(a.invalidate_count(), 2);
        assert_eq!(b.invalidate_count(), 2);
    }

    #[tokio::test]
    async fn with_cache_appends_to_registry() {
        let a: Arc<dyn HttpEndpointCache> = Arc::new(MockHttpEndpointCache::new());
        let inv = EndpointCatalogInvalidator::empty().with_cache(a);
        assert_eq!(inv.len(), 1);
    }
}
