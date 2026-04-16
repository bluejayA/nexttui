use std::sync::Arc;

use crate::adapter::http::base::BaseHttpClient;
use crate::adapter::http::cinder::CinderHttpAdapter;
use crate::adapter::http::glance::GlanceHttpAdapter;
use crate::adapter::http::keystone::KeystoneHttpAdapter;
use crate::adapter::http::neutron::NeutronHttpAdapter;
use crate::adapter::http::nova::NovaHttpAdapter;
use crate::port::auth::AuthProvider;
use crate::port::cinder::CinderPort;
use crate::port::error::ApiError;
use crate::port::glance::GlancePort;
use crate::port::http_endpoint_cache::HttpEndpointCache;
use crate::port::keystone::KeystonePort;
use crate::port::neutron::NeutronPort;
use crate::port::nova::NovaPort;
use crate::port::types::EndpointInterface;

/// AdapterRegistry creates and holds all service adapters.
/// In Phase 1, all adapters use HTTP/REST via BaseHttpClient.
/// In Phase 2, this will support config-based backend selection
/// (e.g., Service Layer gateway instead of direct OpenStack API).
pub struct AdapterRegistry {
    pub nova: Arc<dyn NovaPort>,
    pub neutron: Arc<dyn NeutronPort>,
    pub cinder: Arc<dyn CinderPort>,
    pub glance: Arc<dyn GlancePort>,
    pub keystone: Arc<dyn KeystonePort>,
    http_caches: Vec<Arc<dyn HttpEndpointCache>>,
}

impl AdapterRegistry {
    /// Create all HTTP adapters from the given auth provider and region.
    pub fn new_http(auth: Arc<dyn AuthProvider>, region: Option<String>) -> Result<Self, ApiError> {
        let nova_base = Arc::new(BaseHttpClient::new(
            auth.clone(),
            "compute",
            EndpointInterface::Public,
            region.clone(),
        )?);
        let neutron_base = Arc::new(BaseHttpClient::new(
            auth.clone(),
            "network",
            EndpointInterface::Public,
            region.clone(),
        )?);
        let cinder_base = Arc::new(BaseHttpClient::new(
            auth.clone(),
            "block-storage",
            EndpointInterface::Public,
            region.clone(),
        )?);
        let glance_base = Arc::new(BaseHttpClient::new(
            auth.clone(),
            "image",
            EndpointInterface::Public,
            region.clone(),
        )?);
        let keystone_base = Arc::new(BaseHttpClient::new(
            auth,
            "identity",
            EndpointInterface::Public,
            region,
        )?);

        let http_caches: Vec<Arc<dyn HttpEndpointCache>> = vec![
            nova_base.clone(),
            neutron_base.clone(),
            cinder_base.clone(),
            glance_base.clone(),
            keystone_base.clone(),
        ];

        Ok(Self {
            nova: Arc::new(NovaHttpAdapter::from_base(nova_base)),
            neutron: Arc::new(NeutronHttpAdapter::from_base(neutron_base)),
            cinder: Arc::new(CinderHttpAdapter::from_base(cinder_base)),
            glance: Arc::new(GlanceHttpAdapter::from_base(glance_base)),
            keystone: Arc::new(KeystoneHttpAdapter::from_base(keystone_base)),
            http_caches,
        })
    }

    /// Collect endpoint caches from all HTTP adapters for the
    /// EndpointCatalogInvalidator. Mock registries return an empty vec.
    pub fn endpoint_caches(&self) -> Vec<Arc<dyn HttpEndpointCache>> {
        self.http_caches.clone()
    }

    /// Create registry from mock adapters (for testing).
    #[cfg(test)]
    pub fn new_mock() -> Self {
        use crate::port::mock::*;
        Self {
            nova: Arc::new(MockNovaAdapter),
            neutron: Arc::new(MockNeutronAdapter),
            cinder: Arc::new(MockCinderAdapter),
            glance: Arc::new(MockGlanceAdapter),
            keystone: Arc::new(MockKeystoneAdapter),
            http_caches: Vec::new(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_adapter_registry_mock_creation() {
        let registry = AdapterRegistry::new_mock();
        let _nova: &dyn NovaPort = registry.nova.as_ref();
        let _neutron: &dyn NeutronPort = registry.neutron.as_ref();
        let _cinder: &dyn CinderPort = registry.cinder.as_ref();
        let _glance: &dyn GlancePort = registry.glance.as_ref();
        let _keystone: &dyn KeystonePort = registry.keystone.as_ref();
    }

    #[test]
    fn test_registry_adapters_are_send_sync() {
        fn assert_send_sync<T: Send + Sync>() {}
        assert_send_sync::<AdapterRegistry>();
    }

    #[test]
    fn test_mock_registry_endpoint_caches_empty() {
        let registry = AdapterRegistry::new_mock();
        assert!(registry.endpoint_caches().is_empty());
    }
}
