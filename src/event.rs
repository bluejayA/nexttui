use crate::models::{
    cinder::{Volume, VolumeSnapshot},
    glance::Image,
    keystone::{Project, User},
    neutron::{FloatingIp, Network, NetworkAgent, SecurityGroup},
    nova::{Aggregate, ComputeService, Flavor, Hypervisor, Server, ServerMigration},
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
    ServerCreated(Server),
    ServerDeleted { id: String, name: String },
    ServerRebooted { id: String },
    ServerStarted { id: String },
    ServerStopped { id: String },
    ServerSnapshotCreated { server_id: String, image_id: String },
    FlavorCreated(Flavor),
    FlavorDeleted { id: String },
    NetworkCreated(Network),
    SubnetsLoaded { network_id: String, subnets: Vec<crate::port::types::Subnet> },
    SecurityGroupCreated(SecurityGroup),
    SecurityGroupDeleted { id: String },
    SecurityGroupRuleCreated(crate::models::neutron::SecurityGroupRule),
    SecurityGroupRuleDeleted { rule_id: String },
    VolumeCreated(Volume),
    VolumeDeleted { id: String },
    VolumeExtended { id: String },
    SnapshotCreated(VolumeSnapshot),
    SnapshotDeleted { id: String },
    ImageCreated(Image),
    ImageDeleted { id: String },
    FloatingIpCreated(FloatingIp),
    FloatingIpDeleted { id: String },

    // Keystone CUD
    ProjectCreated(Project),
    ProjectDeleted { id: String },
    UserCreated(User),
    UserDeleted { id: String },

    // Resize results
    ServerResized { id: String },
    ResizeConfirmed { id: String },
    ResizeReverted { id: String },

    // Migration results
    ServerLiveMigrated { id: String },
    ServerColdMigrated { id: String },
    MigrationConfirmed { id: String },
    MigrationReverted { id: String },
    ServerEvacuated { id: String },
    MigrationProgressLoaded { server_id: String, migration: ServerMigration },
    MigrationPollingStopped { server_id: String },

    // Error
    ApiError { operation: String, message: String },

    // Auth
    TokenRefreshed(Vec<crate::port::types::TokenRole>),
    AuthFailed(String),

    // RBAC
    PermissionDenied { operation: String },

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
            AppEvent::FlavorsLoaded(vec![]),
            AppEvent::NetworksLoaded(vec![]),
            AppEvent::VolumesLoaded(vec![]),
            AppEvent::ImagesLoaded(vec![]),
            AppEvent::ServerDeleted {
                id: "s1".into(),
                name: "web".into(),
            },
            AppEvent::ServerSnapshotCreated {
                server_id: "s1".into(),
                image_id: "img-1".into(),
            },
            AppEvent::FlavorDeleted { id: "f1".into() },
            AppEvent::ApiError {
                operation: "delete".into(),
                message: "not found".into(),
            },
            AppEvent::TokenRefreshed(vec![]),
            AppEvent::AuthFailed("expired".into()),
            AppEvent::CloudSwitched("prod".into()),
        ];
        assert!(events.len() >= 12);
    }

    #[test]
    fn test_token_refreshed_carries_roles() {
        use crate::port::types::TokenRole;
        let role = TokenRole { id: "r1".into(), name: "admin".into() };
        let event = AppEvent::TokenRefreshed(vec![role]);
        match event {
            AppEvent::TokenRefreshed(roles) => {
                assert_eq!(roles.len(), 1);
                assert_eq!(roles[0].name, "admin");
            }
            _ => panic!("expected TokenRefreshed"),
        }
    }

    #[test]
    fn test_migration_event_variants_exist() {
        use crate::models::nova::ServerMigration;
        let events: Vec<AppEvent> = vec![
            AppEvent::ServerLiveMigrated { id: "s1".into() },
            AppEvent::ServerColdMigrated { id: "s1".into() },
            AppEvent::MigrationConfirmed { id: "s1".into() },
            AppEvent::MigrationReverted { id: "s1".into() },
            AppEvent::ServerEvacuated { id: "s1".into() },
            AppEvent::MigrationProgressLoaded {
                server_id: "s1".into(),
                migration: ServerMigration {
                    id: 1,
                    status: "running".into(),
                    source_compute: "compute-01".into(),
                    dest_compute: "compute-02".into(),
                    memory_total_bytes: Some(1024),
                    memory_processed_bytes: Some(512),
                    memory_remaining_bytes: Some(512),
                    disk_total_bytes: Some(4096),
                    disk_processed_bytes: Some(2048),
                    disk_remaining_bytes: Some(2048),
                    created_at: None,
                    updated_at: None,
                },
            },
        ];
        assert_eq!(events.len(), 6);
    }

    #[test]
    fn test_resize_event_variants_exist() {
        let events: Vec<AppEvent> = vec![
            AppEvent::ServerResized { id: "s1".into() },
            AppEvent::ResizeConfirmed { id: "s1".into() },
            AppEvent::ResizeReverted { id: "s1".into() },
        ];
        assert_eq!(events.len(), 3);
    }

    #[test]
    fn test_permission_denied_event() {
        let event = AppEvent::PermissionDenied { operation: "CreateServer".into() };
        match event {
            AppEvent::PermissionDenied { operation } => {
                assert_eq!(operation, "CreateServer");
            }
            _ => panic!("expected PermissionDenied"),
        }
    }
}
