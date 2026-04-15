use std::sync::RwLock;

use crate::error::{AppError, Result};
use crate::port::types::{CatalogEntry, Endpoint, EndpointInterface};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ServiceType {
    Compute,
    Network,
    BlockStorage,
    Identity,
    Image,
}

impl ServiceType {
    pub fn as_str(&self) -> &'static str {
        match self {
            ServiceType::Compute => "compute",
            ServiceType::Network => "network",
            ServiceType::BlockStorage => "volumev3",
            ServiceType::Identity => "identity",
            ServiceType::Image => "image",
        }
    }
}

pub struct ServiceCatalog {
    catalog: RwLock<Vec<CatalogEntry>>,
    region: RwLock<Option<String>>,
    interface_preference: RwLock<EndpointInterface>,
}

impl ServiceCatalog {
    pub fn new(interface_preference: EndpointInterface) -> Self {
        Self {
            catalog: RwLock::new(Vec::new()),
            region: RwLock::new(None),
            interface_preference: RwLock::new(interface_preference),
        }
    }

    /// Update catalog from Keystone token response.
    pub fn update(&self, catalog: Vec<CatalogEntry>, region: Option<String>) {
        if let Ok(mut c) = self.catalog.write() {
            *c = catalog;
        }
        if let Ok(mut r) = self.region.write() {
            *r = region;
        }
    }

    /// Resolve endpoint URL for a service type using the configured region.
    pub fn endpoint(&self, service_type: ServiceType) -> Result<String> {
        let region = self.region.read().ok().and_then(|r| r.clone());
        self.resolve(service_type, region.as_deref())
    }

    /// Get endpoint for a service type with explicit region override.
    pub fn endpoint_in_region(&self, service_type: ServiceType, region: &str) -> Result<String> {
        self.resolve(service_type, Some(region))
    }

    /// List all available regions across all services.
    pub fn available_regions(&self) -> Vec<String> {
        let catalog = match self.catalog.read() {
            Ok(c) => c,
            Err(_) => return Vec::new(),
        };
        let mut regions: Vec<String> = catalog
            .iter()
            .flat_map(|e| e.endpoints.iter().map(|ep| ep.region.clone()))
            .collect();
        regions.sort();
        regions.dedup();
        regions
    }

    /// List all discovered service types.
    pub fn available_services(&self) -> Vec<String> {
        let catalog = match self.catalog.read() {
            Ok(c) => c,
            Err(_) => return Vec::new(),
        };
        catalog.iter().map(|e| e.service_type.clone()).collect()
    }

    /// Set active region.
    pub fn set_region(&self, region: &str) {
        if let Ok(mut r) = self.region.write() {
            *r = Some(region.to_string());
        }
    }

    /// Get current region.
    pub fn current_region(&self) -> Option<String> {
        self.region.read().ok().and_then(|r| r.clone())
    }

    /// Check if a service type is available in the catalog.
    pub fn has_service(&self, service_type: ServiceType) -> bool {
        let catalog = match self.catalog.read() {
            Ok(c) => c,
            Err(_) => return false,
        };
        let type_str = service_type.as_str();
        catalog.iter().any(|e| e.service_type == type_str)
    }

    /// Common endpoint resolution logic shared by `endpoint` and `endpoint_in_region`.
    fn resolve(&self, service_type: ServiceType, region: Option<&str>) -> Result<String> {
        let catalog = self
            .catalog
            .read()
            .map_err(|_| AppError::Other("Failed to read service catalog".to_string()))?;
        let pref = self
            .interface_preference
            .read()
            .map(|p| p.clone())
            .unwrap_or(EndpointInterface::Public);

        let type_str = service_type.as_str();
        let entry = catalog
            .iter()
            .find(|e| e.service_type == type_str)
            .ok_or_else(|| {
                AppError::Other(format!("Service type '{type_str}' not found in catalog"))
            })?;

        resolve_endpoint(&entry.endpoints, region, &pref)
    }
}

/// Resolve endpoint from a list with region and interface preference.
/// Fallback order: preferred interface -> Public -> Internal -> Admin.
fn resolve_endpoint(
    endpoints: &[Endpoint],
    region: Option<&str>,
    preferred: &EndpointInterface,
) -> Result<String> {
    let candidates: Vec<&Endpoint> = if let Some(region) = region {
        endpoints.iter().filter(|ep| ep.region == region).collect()
    } else {
        endpoints.iter().collect()
    };

    if candidates.is_empty() {
        return Err(AppError::Other(format!(
            "No endpoints found for region '{}'",
            region.unwrap_or("any")
        )));
    }

    // Try preferred interface first
    if let Some(ep) = candidates.iter().find(|ep| &ep.interface == preferred) {
        return Ok(ep.url.clone());
    }

    // Fallback order
    let fallback_order = [
        EndpointInterface::Public,
        EndpointInterface::Internal,
        EndpointInterface::Admin,
    ];
    for iface in &fallback_order {
        if let Some(ep) = candidates.iter().find(|ep| &ep.interface == iface) {
            return Ok(ep.url.clone());
        }
    }

    Ok(candidates[0].url.clone())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_endpoint(region: &str, interface: EndpointInterface, url: &str) -> Endpoint {
        Endpoint {
            region: region.to_string(),
            interface,
            url: url.to_string(),
        }
    }

    fn sample_catalog() -> Vec<CatalogEntry> {
        vec![
            CatalogEntry {
                service_type: "compute".to_string(),
                service_name: "nova".to_string(),
                endpoints: vec![
                    make_endpoint(
                        "RegionOne",
                        EndpointInterface::Internal,
                        "https://nova-int:8774",
                    ),
                    make_endpoint(
                        "RegionOne",
                        EndpointInterface::Public,
                        "https://nova-pub:8774",
                    ),
                    make_endpoint(
                        "RegionTwo",
                        EndpointInterface::Internal,
                        "https://nova-r2:8774",
                    ),
                ],
            },
            CatalogEntry {
                service_type: "network".to_string(),
                service_name: "neutron".to_string(),
                endpoints: vec![make_endpoint(
                    "RegionOne",
                    EndpointInterface::Internal,
                    "https://neutron:9696",
                )],
            },
            CatalogEntry {
                service_type: "identity".to_string(),
                service_name: "keystone".to_string(),
                endpoints: vec![
                    make_endpoint(
                        "RegionOne",
                        EndpointInterface::Public,
                        "https://keystone:5000",
                    ),
                    make_endpoint(
                        "RegionOne",
                        EndpointInterface::Admin,
                        "https://keystone:35357",
                    ),
                ],
            },
        ]
    }

    #[test]
    fn test_endpoint_preferred_interface() {
        let catalog = ServiceCatalog::new(EndpointInterface::Internal);
        catalog.update(sample_catalog(), Some("RegionOne".to_string()));
        let url = catalog.endpoint(ServiceType::Compute).unwrap();
        assert_eq!(url, "https://nova-int:8774");
    }

    #[test]
    fn test_endpoint_fallback_to_public() {
        let catalog = ServiceCatalog::new(EndpointInterface::Internal);
        catalog.update(sample_catalog(), Some("RegionOne".to_string()));
        let url = catalog.endpoint(ServiceType::Identity).unwrap();
        assert_eq!(url, "https://keystone:5000");
    }

    #[test]
    fn test_endpoint_not_found() {
        let catalog = ServiceCatalog::new(EndpointInterface::Public);
        catalog.update(sample_catalog(), None);
        let result = catalog.endpoint(ServiceType::Image);
        assert!(result.is_err());
    }

    #[test]
    fn test_endpoint_in_region() {
        let catalog = ServiceCatalog::new(EndpointInterface::Internal);
        catalog.update(sample_catalog(), None);
        let url = catalog
            .endpoint_in_region(ServiceType::Compute, "RegionTwo")
            .unwrap();
        assert_eq!(url, "https://nova-r2:8774");
    }

    #[test]
    fn test_endpoint_in_region_not_found() {
        let catalog = ServiceCatalog::new(EndpointInterface::Internal);
        catalog.update(sample_catalog(), None);
        let result = catalog.endpoint_in_region(ServiceType::Compute, "RegionThree");
        assert!(result.is_err());
    }

    #[test]
    fn test_available_regions() {
        let catalog = ServiceCatalog::new(EndpointInterface::Public);
        catalog.update(sample_catalog(), None);
        let regions = catalog.available_regions();
        assert_eq!(regions, vec!["RegionOne", "RegionTwo"]);
    }

    #[test]
    fn test_available_services() {
        let catalog = ServiceCatalog::new(EndpointInterface::Public);
        catalog.update(sample_catalog(), None);
        let services = catalog.available_services();
        assert_eq!(services, vec!["compute", "network", "identity"]);
    }

    #[test]
    fn test_set_and_get_region() {
        let catalog = ServiceCatalog::new(EndpointInterface::Public);
        assert!(catalog.current_region().is_none());
        catalog.set_region("RegionOne");
        assert_eq!(catalog.current_region(), Some("RegionOne".to_string()));
    }

    #[test]
    fn test_has_service() {
        let catalog = ServiceCatalog::new(EndpointInterface::Public);
        catalog.update(sample_catalog(), None);
        assert!(catalog.has_service(ServiceType::Compute));
        assert!(catalog.has_service(ServiceType::Network));
        assert!(!catalog.has_service(ServiceType::Image));
        assert!(!catalog.has_service(ServiceType::BlockStorage));
    }

    #[test]
    fn test_service_type_as_str() {
        assert_eq!(ServiceType::Compute.as_str(), "compute");
        assert_eq!(ServiceType::Network.as_str(), "network");
        assert_eq!(ServiceType::BlockStorage.as_str(), "volumev3");
        assert_eq!(ServiceType::Identity.as_str(), "identity");
        assert_eq!(ServiceType::Image.as_str(), "image");
    }

    #[test]
    fn test_empty_catalog() {
        let catalog = ServiceCatalog::new(EndpointInterface::Public);
        assert!(!catalog.has_service(ServiceType::Compute));
        assert!(catalog.available_regions().is_empty());
        assert!(catalog.available_services().is_empty());
    }

    #[test]
    fn test_region_filtering() {
        let catalog = ServiceCatalog::new(EndpointInterface::Internal);
        catalog.update(sample_catalog(), Some("RegionOne".to_string()));
        let url = catalog.endpoint(ServiceType::Compute).unwrap();
        assert_eq!(url, "https://nova-int:8774");
    }
}
