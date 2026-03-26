use std::sync::Arc;

use crate::adapter::http::cinder::CinderHttpAdapter;
use crate::adapter::http::glance::GlanceHttpAdapter;
use crate::adapter::http::keystone::KeystoneHttpAdapter;
use crate::adapter::http::neutron::NeutronHttpAdapter;
use crate::adapter::http::nova::NovaHttpAdapter;
use crate::port::auth::AuthProvider;
use crate::port::cinder::CinderPort;
use crate::port::error::ApiError;
use crate::port::glance::GlancePort;
use crate::port::keystone::KeystonePort;
use crate::port::neutron::NeutronPort;
use crate::port::nova::NovaPort;

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
}

impl AdapterRegistry {
    /// Create all HTTP adapters from the given auth provider and region.
    pub fn new_http(auth: Arc<dyn AuthProvider>, region: Option<String>) -> Result<Self, ApiError> {
        Ok(Self {
            nova: Arc::new(NovaHttpAdapter::new(auth.clone(), region.clone())?),
            neutron: Arc::new(NeutronHttpAdapter::new(auth.clone(), region.clone())?),
            cinder: Arc::new(CinderHttpAdapter::new(auth.clone(), region.clone())?),
            glance: Arc::new(GlanceHttpAdapter::new(auth.clone(), region.clone())?),
            keystone: Arc::new(KeystoneHttpAdapter::new(auth, region)?),
        })
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
}
