use std::sync::Arc;

use crate::adapter::http::base::BaseHttpClient;
use crate::adapter::http::cinder::CinderHttpAdapter;
use crate::adapter::http::glance::GlanceHttpAdapter;
use crate::adapter::http::keystone::KeystoneHttpAdapter;
use crate::adapter::http::neutron::NeutronHttpAdapter;
use crate::adapter::http::neutron_audit::AdapterAuditConfig;
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
    fn make_base(
        auth: Arc<dyn AuthProvider>,
        service_type: &str,
        region: Option<String>,
    ) -> Result<Arc<BaseHttpClient>, ApiError> {
        Ok(Arc::new(BaseHttpClient::new(
            auth,
            service_type,
            EndpointInterface::Public,
            region,
        )?))
    }

    /// Create all HTTP adapters from the given auth provider and region.
    ///
    /// `audit` (BL-P2-085 Step-14-precedent-refactor-3) bundles per-service
    /// audit contexts. The Neutron context is wired today (Step 13b);
    /// `audit.nova` and `audit.cinder` are accepted but not yet attached
    /// (Step 14 will add `with_audit` builders to those adapters). Pass
    /// `AdapterAuditConfig::default()` for mock registries and integration
    /// tests that don't care about audit emission; the Neutron adapter
    /// then behaves as a pre-Step-13b passthrough.
    pub fn new_http(
        auth: Arc<dyn AuthProvider>,
        region: Option<String>,
        audit: AdapterAuditConfig,
    ) -> Result<Self, ApiError> {
        let nova_base = Self::make_base(auth.clone(), "compute", region.clone())?;
        let neutron_base = Self::make_base(auth.clone(), "network", region.clone())?;
        let cinder_base = Self::make_base(auth.clone(), "block-storage", region.clone())?;
        let glance_base = Self::make_base(auth.clone(), "image", region.clone())?;
        let keystone_base = Self::make_base(auth, "identity", region)?;

        let http_caches: Vec<Arc<dyn HttpEndpointCache>> = vec![
            nova_base.clone(),
            neutron_base.clone(),
            cinder_base.clone(),
            glance_base.clone(),
            keystone_base.clone(),
        ];

        let mut neutron = NeutronHttpAdapter::from_base(neutron_base);
        if let Some(ctx) = audit.neutron {
            neutron = neutron.with_audit(ctx);
        }
        // audit.nova / audit.cinder land here in Step 14 once
        // NovaHttpAdapter / CinderHttpAdapter expose `with_audit`.

        Ok(Self {
            nova: Arc::new(NovaHttpAdapter::from_base(nova_base)),
            neutron: Arc::new(neutron),
            cinder: Arc::new(CinderHttpAdapter::from_base(cinder_base)),
            glance: Arc::new(GlanceHttpAdapter::from_base(glance_base)),
            keystone: Arc::new(KeystoneHttpAdapter::from_base(keystone_base)),
            http_caches,
        })
    }

    /// Endpoint caches for the EndpointCatalogInvalidator.
    /// Mock registries return an empty slice.
    pub fn endpoint_caches(&self) -> &[Arc<dyn HttpEndpointCache>] {
        &self.http_caches
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
