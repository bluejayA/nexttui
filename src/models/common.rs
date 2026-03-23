#[derive(Debug, Clone, Copy, Hash, Eq, PartialEq)]
pub enum ResourceType {
    Servers,
    Networks,
    SecurityGroups,
    Volumes,
    Snapshots,
    Flavors,
    Images,
    Projects,
    Users,
    Aggregates,
    ComputeServices,
    Hypervisors,
    FloatingIps,
    Agents,
}

#[derive(Debug, Clone, Copy, Hash, Eq, PartialEq)]
pub enum Route {
    // Nova
    Servers,
    ServerDetail,
    ServerCreate,
    Flavors,
    Migrations,
    Aggregates,
    ComputeServices,
    Hypervisors,

    // Neutron
    Networks,
    NetworkDetail,
    SecurityGroups,
    SecurityGroupDetail,
    FloatingIps,
    Agents,

    // Cinder
    Volumes,
    VolumeDetail,
    VolumeCreate,
    Snapshots,

    // Glance
    Images,
    ImageDetail,

    // Keystone (Admin)
    Projects,
    Users,

    // Monitoring
    Usage,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_resource_type_variants() {
        // Ensure all variants exist and are distinct
        let types = [
            ResourceType::Servers,
            ResourceType::Networks,
            ResourceType::SecurityGroups,
            ResourceType::Volumes,
            ResourceType::Snapshots,
            ResourceType::Flavors,
            ResourceType::Images,
            ResourceType::Projects,
            ResourceType::Users,
            ResourceType::Aggregates,
            ResourceType::ComputeServices,
            ResourceType::Hypervisors,
            ResourceType::FloatingIps,
            ResourceType::Agents,
        ];
        assert_eq!(types.len(), 14);
        // Check uniqueness via HashSet
        let set: std::collections::HashSet<_> = types.iter().collect();
        assert_eq!(set.len(), 14);
    }

    #[test]
    fn test_route_variants() {
        let routes = [
            Route::Servers,
            Route::ServerDetail,
            Route::ServerCreate,
            Route::Flavors,
            Route::Migrations,
            Route::Aggregates,
            Route::ComputeServices,
            Route::Hypervisors,
            Route::Networks,
            Route::NetworkDetail,
            Route::SecurityGroups,
            Route::SecurityGroupDetail,
            Route::FloatingIps,
            Route::Agents,
            Route::Volumes,
            Route::VolumeDetail,
            Route::VolumeCreate,
            Route::Snapshots,
            Route::Images,
            Route::ImageDetail,
            Route::Projects,
            Route::Users,
            Route::Usage,
        ];
        assert_eq!(routes.len(), 23);
        let set: std::collections::HashSet<_> = routes.iter().collect();
        assert_eq!(set.len(), 23);
    }
}
