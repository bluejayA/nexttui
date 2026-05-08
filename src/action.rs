use crate::context::ContextRequest;
use crate::models::common::Route;
use crate::port::types::EvacuateParams;

#[derive(Debug, Clone)]
pub enum Action {
    // Navigation
    Navigate(Route),
    Back,

    // Nova
    FetchServers,
    CreateServer(crate::port::types::ServerCreateParams),
    DeleteServer {
        id: String,
        name: String,
    },
    RebootServer {
        id: String,
        hard: bool,
    },
    StartServer {
        id: String,
    },
    StopServer {
        id: String,
    },
    CreateServerSnapshot {
        server_id: String,
        name: String,
    },
    FetchFlavors,
    CreateFlavor(crate::port::types::FlavorCreateParams),
    DeleteFlavor {
        id: String,
    },
    FetchAggregates,
    FetchComputeServices,
    FetchHypervisors,

    // Neutron
    FetchNetworks,
    CreateNetwork(crate::port::types::NetworkCreateParams),
    FetchSecurityGroups,
    CreateSecurityGroup(crate::port::types::SecurityGroupCreateParams),
    DeleteSecurityGroup {
        id: String,
    },
    CreateSecurityGroupRule(crate::port::types::SecurityGroupRuleCreateParams),
    DeleteSecurityGroupRule {
        rule_id: String,
    },
    FetchFloatingIps,
    CreateFloatingIp {
        network_id: String,
    },
    DeleteFloatingIp {
        id: String,
    },
    FetchSubnets {
        network_id: String,
    },
    FetchAgents,

    // Cinder
    FetchVolumes,
    CreateVolume(crate::port::types::VolumeCreateParams),
    DeleteVolume {
        id: String,
        force: bool,
    },
    ExtendVolume {
        id: String,
        new_size: u32,
    },
    FetchSnapshots,
    CreateSnapshot(crate::port::types::SnapshotCreateParams),
    DeleteSnapshot {
        id: String,
    },

    // Glance
    FetchImages,
    CreateImage(crate::port::types::ImageCreateParams),
    DeleteImage {
        id: String,
    },

    // Keystone Admin
    FetchProjects,
    CreateProject(crate::port::types::ProjectCreateParams),
    DeleteProject {
        id: String,
    },
    FetchUsers,
    CreateUser(crate::port::types::UserCreateParams),
    DeleteUser {
        id: String,
    },

    // Usage
    FetchUsage {
        start: String,
        end: String,
    },

    // UI
    FocusSidebar,
    EnterFormMode,
    ExitFormMode,
    SelectResource {
        id: String,
    },
    NavigateToResource {
        route: Route,
        id: String,
    },

    // Resize
    ResizeServer {
        id: String,
        flavor_id: String,
    },
    ConfirmResize {
        id: String,
    },
    RevertResize {
        id: String,
    },

    // Migration / Evacuate
    LiveMigrateServer {
        id: String,
        host: Option<String>,
    },
    ColdMigrateServer {
        id: String,
    },
    ConfirmMigration {
        id: String,
    },
    RevertMigration {
        id: String,
    },
    EvacuateServer {
        id: String,
        params: EvacuateParams,
    },
    DisableComputeService {
        service_id: String,
        hostname: String,
    },
    EnableComputeService {
        service_id: String,
        hostname: String,
    },
    FetchMigrationProgress {
        server_id: String,
    },

    // Volume Attach/Detach
    AttachVolume {
        volume_id: String,
        server_id: String,
        device: Option<String>,
    },
    DetachVolume {
        volume_id: String,
        server_id: String,
        attachment_id: String,
    },
    ForceDetachVolume {
        volume_id: String,
        server_id: String,
        attachment_id: String,
    },
    ForceResetVolumeState {
        volume_id: String,
        target_state: String,
    },

    // Floating IP Associate/Disassociate
    AssociateFloatingIp {
        fip_id: String,
        port_id: String,
    },
    DisassociateFloatingIp {
        fip_id: String,
    },

    // Ports
    FetchPorts {
        server_id: String,
    },
    /// Fetch *port bindings* (Neutron `binding-extended`) for every port
    /// attached to `server_id`. Used by Server Detail (admin only) to surface
    /// stale bindings left by failed live-migrations — the root cause of the
    /// "No valid host" symptom users see in nexttui (BL-P2-086).
    FetchPortBindingsForServer {
        server_id: String,
    },

    // All Tenants
    ToggleAllTenants,

    // Toast (module-initiated hints)
    ShowToast {
        message: String,
    },

    // System
    RefreshAll,
    Quit,

    // Runtime context switch (BL-P2-031)
    /// Initiate a runtime cloud/project switch via Keystone rescoping.
    /// The payload is unresolved user input — the resolver maps it to a
    /// concrete target before any side effect is performed.
    SwitchContext(ContextRequest),

    /// Restore the previous context (1-step history). No-op if no previous
    /// snapshot exists.
    SwitchBack,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_action_variants_exist() {
        use crate::port::types::{FlavorCreateParams, ServerCreateParams};

        let actions: Vec<Action> = vec![
            Action::Navigate(Route::Servers),
            Action::Back,
            Action::FetchServers,
            Action::CreateServer(ServerCreateParams {
                name: "test".into(),
                image_id: "img-1".into(),
                flavor_id: "flv-1".into(),
                networks: vec![],
                security_groups: None,
                key_name: None,
                availability_zone: None,
            }),
            Action::DeleteServer {
                id: "s1".into(),
                name: "web".into(),
            },
            Action::RebootServer {
                id: "s1".into(),
                hard: false,
            },
            Action::StartServer { id: "s1".into() },
            Action::StopServer { id: "s1".into() },
            Action::CreateServerSnapshot {
                server_id: "s1".into(),
                name: "snap".into(),
            },
            Action::FetchFlavors,
            Action::CreateFlavor(FlavorCreateParams {
                name: "m1.test".into(),
                vcpus: 1,
                ram_mb: 512,
                disk_gb: 10,
                is_public: true,
            }),
            Action::DeleteFlavor { id: "f1".into() },
            Action::FetchNetworks,
            Action::FetchVolumes,
            Action::FetchImages,
            Action::FetchProjects,
            Action::RefreshAll,
            Action::Quit,
        ];
        assert!(actions.len() >= 18);
    }

    #[test]
    fn test_volume_fip_action_variants_exist() {
        let actions: Vec<Action> = vec![
            Action::AttachVolume {
                volume_id: "v1".into(),
                server_id: "s1".into(),
                device: Some("/dev/vdb".into()),
            },
            Action::DetachVolume {
                volume_id: "v1".into(),
                server_id: "s1".into(),
                attachment_id: "att-1".into(),
            },
            Action::ForceDetachVolume {
                volume_id: "v1".into(),
                server_id: "s1".into(),
                attachment_id: "att-1".into(),
            },
            Action::ForceResetVolumeState {
                volume_id: "v1".into(),
                target_state: "available".into(),
            },
            Action::AssociateFloatingIp {
                fip_id: "fip-1".into(),
                port_id: "port-1".into(),
            },
            Action::DisassociateFloatingIp {
                fip_id: "fip-1".into(),
            },
            Action::FetchPorts {
                server_id: "s1".into(),
            },
        ];
        assert_eq!(actions.len(), 7);
    }

    #[test]
    fn test_resize_action_variants_exist() {
        let actions: Vec<Action> = vec![
            Action::ResizeServer {
                id: "s1".into(),
                flavor_id: "f2".into(),
            },
            Action::ConfirmResize { id: "s1".into() },
            Action::RevertResize { id: "s1".into() },
        ];
        assert_eq!(actions.len(), 3);
    }

    #[test]
    fn test_usage_action_variant_exists() {
        let action = Action::FetchUsage {
            start: "2026-01-01T00:00:00Z".into(),
            end: "2026-01-31T23:59:59Z".into(),
        };
        match action {
            Action::FetchUsage { start, end } => {
                assert!(start.contains("2026"));
                assert!(end.contains("2026"));
            }
            _ => panic!("expected FetchUsage"),
        }
    }

    #[test]
    fn test_migration_action_variants_exist() {
        let actions: Vec<Action> = vec![
            Action::LiveMigrateServer {
                id: "s1".into(),
                host: None,
            },
            Action::ColdMigrateServer { id: "s1".into() },
            Action::ConfirmMigration { id: "s1".into() },
            Action::RevertMigration { id: "s1".into() },
            Action::EvacuateServer {
                id: "s1".into(),
                params: EvacuateParams {
                    host: Some("compute-02".into()),
                    ..Default::default()
                },
            },
            Action::FetchMigrationProgress {
                server_id: "s1".into(),
            },
            Action::DisableComputeService {
                service_id: "svc-1".into(),
                hostname: "compute-01".into(),
            },
            Action::EnableComputeService {
                service_id: "svc-1".into(),
                hostname: "compute-01".into(),
            },
        ];
        assert_eq!(actions.len(), 8);
    }
}
