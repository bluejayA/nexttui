//! Trait implemented by every HTTP client that caches discovered service
//! endpoints. [`EndpointCatalogInvalidator`](crate::adapter::http::endpoint_invalidator)
//! iterates the registry and calls
//! [`invalidate`](HttpEndpointCache::invalidate) on each entry whenever the
//! active scope changes.
//!
//! The method is async because existing HTTP clients hold their endpoint
//! cache behind a `tokio::sync::RwLock` — mirroring that shape here keeps
//! implementations idiomatic and avoids forcing a lock-type migration.

use async_trait::async_trait;

#[async_trait]
pub trait HttpEndpointCache: Send + Sync {
    /// Clear any cached endpoint URL; subsequent requests must re-resolve
    /// via the auth provider's catalog.
    async fn invalidate(&self);
}
