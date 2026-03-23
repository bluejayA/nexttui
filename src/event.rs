use crate::models::{
    cinder::{Volume, VolumeSnapshot},
    glance::Image,
    keystone::{Project, User},
    neutron::{FloatingIp, Network, NetworkAgent, SecurityGroup},
    nova::{Aggregate, ComputeService, Flavor, Hypervisor, Server},
};

#[derive(Debug)]
pub enum AppEvent {
    // Data loaded
    ServersLoaded(Vec<Server>),
    FlavorsLoaded(Vec<Flavor>),
    NetworksLoaded(Vec<Network>),
    SecurityGroupsLoaded(Vec<SecurityGroup>),
    FloatingIpsLoaded(Vec<FloatingIp>),
    VolumesLoaded(Vec<Volume>),
    SnapshotsLoaded(Vec<VolumeSnapshot>),
    ImagesLoaded(Vec<Image>),
    ProjectsLoaded(Vec<Project>),
    UsersLoaded(Vec<User>),
    AggregatesLoaded(Vec<Aggregate>),
    ComputeServicesLoaded(Vec<ComputeService>),
    HypervisorsLoaded(Vec<Hypervisor>),
    AgentsLoaded(Vec<NetworkAgent>),

    // CUD results
    ServerDeleted { id: String, name: String },
    ServerRebooted { id: String },
    ServerStarted { id: String },
    ServerStopped { id: String },
    VolumeDeleted { id: String },
    ImageDeleted { id: String },
    FloatingIpCreated(FloatingIp),
    FloatingIpDeleted { id: String },

    // Error
    ApiError { operation: String, message: String },

    // Auth
    TokenRefreshed,
    AuthFailed(String),

    // System
    CloudSwitched(String),
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_app_event_variants_exist() {
        let events: Vec<AppEvent> = vec![
            AppEvent::ServersLoaded(vec![]),
            AppEvent::NetworksLoaded(vec![]),
            AppEvent::VolumesLoaded(vec![]),
            AppEvent::ImagesLoaded(vec![]),
            AppEvent::ServerDeleted {
                id: "s1".into(),
                name: "web".into(),
            },
            AppEvent::ApiError {
                operation: "delete".into(),
                message: "not found".into(),
            },
            AppEvent::TokenRefreshed,
            AppEvent::AuthFailed("expired".into()),
            AppEvent::CloudSwitched("prod".into()),
        ];
        assert!(events.len() >= 9);
    }
}
