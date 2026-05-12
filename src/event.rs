use crate::context::ContextTarget;
use crate::models::{
    cinder::{Volume, VolumeSnapshot},
    glance::Image,
    keystone::{Project, User},
    neutron::{FloatingIp, Network, NetworkAgent, Port, PortBinding, SecurityGroup},
    nova::{Aggregate, ComputeService, Flavor, Hypervisor, Server, ServerMigration},
};
use crate::port::types::TenantUsage;

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

    // Usage
    UsageLoaded(Vec<TenantUsage>),

    // CUD results
    ServerCreated(Server),
    ServerDeleted {
        id: String,
        name: String,
    },
    ServerRebooted {
        id: String,
    },
    ServerStarted {
        id: String,
    },
    ServerStopped {
        id: String,
    },
    ServerSnapshotCreated {
        server_id: String,
        image_id: String,
    },
    FlavorCreated(Flavor),
    FlavorDeleted {
        id: String,
    },
    NetworkCreated(Network),
    SubnetsLoaded {
        network_id: String,
        subnets: Vec<crate::port::types::Subnet>,
    },
    SecurityGroupCreated(SecurityGroup),
    SecurityGroupDeleted {
        id: String,
    },
    SecurityGroupRuleCreated(crate::models::neutron::SecurityGroupRule),
    SecurityGroupRuleDeleted {
        rule_id: String,
    },
    VolumeCreated(Volume),
    VolumeDeleted {
        id: String,
    },
    VolumeExtended {
        id: String,
    },
    SnapshotCreated(VolumeSnapshot),
    SnapshotDeleted {
        id: String,
    },
    ImageCreated(Image),
    ImageDeleted {
        id: String,
    },
    FloatingIpCreated(FloatingIp),
    FloatingIpDeleted {
        id: String,
    },

    // Keystone CUD
    ProjectCreated(Project),
    ProjectDeleted {
        id: String,
    },
    UserCreated(User),
    UserDeleted {
        id: String,
    },

    // Resize results
    ServerResized {
        id: String,
    },
    ResizeConfirmed {
        id: String,
    },
    ResizeReverted {
        id: String,
    },

    // Migration results
    ServerLiveMigrated {
        id: String,
    },
    ServerColdMigrated {
        id: String,
    },
    MigrationConfirmed {
        id: String,
    },
    MigrationReverted {
        id: String,
    },
    ServerEvacuated {
        id: String,
    },
    ServerEvacuateResult {
        id: String,
        result: Result<(), String>,
    },
    ComputeServiceToggled {
        hostname: String,
        enabled: bool,
    },
    MigrationProgressLoaded {
        server_id: String,
        migration: ServerMigration,
    },
    MigrationPollingStopped {
        server_id: String,
    },

    // Volume Attach/Detach results
    VolumeAttached {
        volume_id: String,
        server_id: String,
    },
    VolumeDetached {
        volume_id: String,
    },
    VolumeForceDetached {
        volume_id: String,
    },
    VolumeStateReset {
        volume_id: String,
    },

    // Floating IP Associate/Disassociate results
    FloatingIpAssociated(FloatingIp),
    FloatingIpDisassociated(FloatingIp),

    // Ports
    PortsLoaded {
        server_id: String,
        ports: Vec<Port>,
    },
    /// Result of `Action::FetchPortBindingsForServer` — pairs of `(port_id,
    /// bindings)` for every port attached to `server_id`. Bindings come from
    /// Neutron's `binding-extended` API (BL-P2-086).
    PortBindingsLoaded {
        server_id: String,
        port_bindings: Vec<(String, Vec<PortBinding>)>,
    },

    // Server status polling (resize / cold-migrate state transitions)
    ServerStatusPolled {
        server: Server,
    },

    // Error
    ApiError {
        operation: String,
        message: String,
    },

    // Auth
    TokenRefreshed(Vec<crate::port::types::TokenRole>),
    AuthFailed(String),

    // RBAC
    PermissionDenied {
        operation: String,
    },

    // FR2 Cross-project block (BL-P2-085 Step 11c)
    /// Worker rejected a stamped mutation because its `origin_project_id`
    /// disagreed with the live active scope on `RbacGuard`. `reason` is the
    /// stable `CrossProjectReason::as_str()` slug (`"origin_scope_mismatch"`,
    /// `"unscoped_fail_safe"`, …); `action` is the `Action` variant name.
    /// The structured audit entry is written by the worker via
    /// `cross_project_audit::emit` before this event reaches the UI.
    CrossProjectBlocked {
        reason: String,
        action: String,
    },

    // System
    CloudSwitched(String),

    // Runtime context switch (BL-P2-031)
    /// Active cloud/project context changed. Resource modules listen for
    /// this and clear cached data so the user never sees stale rows.
    /// Epoch lives on the surrounding `VersionedEvent` envelope, not the
    /// payload — see `crate::context::VersionedEvent`.
    ///
    /// BL-P2-093: `user_id` carries the Keystone-issued UUID for the freshly
    /// rescoped token. `App::handle_event` writes it into the shared
    /// `ActorContext` so subsequent cross-project block audits attribute the
    /// new actor — without this, multi-cloud reauth leaves audit entries
    /// anchored to the wire-startup user and breaks fingerprint v1 dedup
    /// (canonical `"v1|user|active|origin|target|action|resource_id"`).
    /// Empty string falls back to the previous value (kept for adapters/tests
    /// that lack user info; production tokens always populate it).
    ContextChanged {
        target: ContextTarget,
        user_id: String,
    },
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
        let role = TokenRole {
            id: "r1".into(),
            name: "admin".into(),
        };
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
    fn test_server_status_polled_event() {
        use crate::models::nova::Server;
        let server = Server {
            id: "s1".into(),
            name: "test".into(),
            status: "VERIFY_RESIZE".into(),
            addresses: Default::default(),
            flavor: crate::models::nova::FlavorRef {
                id: "f1".into(),
                original_name: None,
                vcpus: None,
                ram: None,
                disk: None,
            },
            image: None,
            key_name: None,
            availability_zone: None,
            created: "2026-01-01".into(),
            updated: None,
            tenant_id: None,
            host_id: None,
            host: None,
            volumes_attached: vec![],
            security_groups: vec![],
        };
        let event = AppEvent::ServerStatusPolled { server };
        match event {
            AppEvent::ServerStatusPolled { server } => {
                assert_eq!(server.status, "VERIFY_RESIZE");
            }
            _ => panic!("expected ServerStatusPolled"),
        }
    }

    #[test]
    fn test_volume_fip_event_variants_exist() {
        use crate::models::neutron::{FixedIp, FloatingIp, Port};
        let events: Vec<AppEvent> = vec![
            AppEvent::VolumeAttached {
                volume_id: "v1".into(),
                server_id: "s1".into(),
            },
            AppEvent::VolumeDetached {
                volume_id: "v1".into(),
            },
            AppEvent::VolumeForceDetached {
                volume_id: "v1".into(),
            },
            AppEvent::VolumeStateReset {
                volume_id: "v1".into(),
            },
            AppEvent::FloatingIpAssociated(FloatingIp {
                id: "fip-1".into(),
                floating_ip_address: "203.0.113.10".into(),
                status: "ACTIVE".into(),
                port_id: Some("port-1".into()),
                floating_network_id: "ext-1".into(),
                fixed_ip_address: None,
                router_id: None,
                tenant_id: None,
            }),
            AppEvent::FloatingIpDisassociated(FloatingIp {
                id: "fip-1".into(),
                floating_ip_address: "203.0.113.10".into(),
                status: "DOWN".into(),
                port_id: None,
                floating_network_id: "ext-1".into(),
                fixed_ip_address: None,
                router_id: None,
                tenant_id: None,
            }),
            AppEvent::PortsLoaded {
                server_id: "s1".into(),
                ports: vec![Port {
                    id: "port-1".into(),
                    name: None,
                    network_id: "net-1".into(),
                    fixed_ips: vec![FixedIp {
                        subnet_id: "sub-1".into(),
                        ip_address: "10.0.0.5".into(),
                    }],
                    device_id: Some("s1".into()),
                    device_owner: Some("compute:az1".into()),
                    status: "ACTIVE".into(),
                    tenant_id: None,
                }],
            },
        ];
        assert_eq!(events.len(), 7);
    }

    #[test]
    fn test_usage_loaded_event() {
        use crate::port::types::TenantUsage;
        let usage = TenantUsage {
            tenant_id: "proj-1".into(),
            total_vcpus_usage: 4.0,
            total_memory_mb_usage: 8192.0,
            total_local_gb_usage: 100.0,
            total_hours: 720.0,
            server_usages: vec![],
        };
        let event = AppEvent::UsageLoaded(vec![usage]);
        match event {
            AppEvent::UsageLoaded(usages) => {
                assert_eq!(usages.len(), 1);
                assert_eq!(usages[0].tenant_id, "proj-1");
            }
            _ => panic!("expected UsageLoaded"),
        }
    }

    #[test]
    fn test_permission_denied_event() {
        let event = AppEvent::PermissionDenied {
            operation: "CreateServer".into(),
        };
        match event {
            AppEvent::PermissionDenied { operation } => {
                assert_eq!(operation, "CreateServer");
            }
            _ => panic!("expected PermissionDenied"),
        }
    }
}
