use crate::models::common::Route;

#[derive(Debug, Clone)]
pub enum Action {
    // Navigation
    Navigate(Route),
    Back,

    // Nova
    FetchServers,
    CreateServer(crate::port::types::ServerCreateParams),
    DeleteServer { id: String, name: String },
    RebootServer { id: String, hard: bool },
    StartServer { id: String },
    StopServer { id: String },
    CreateServerSnapshot { server_id: String, name: String },
    FetchFlavors,
    CreateFlavor(crate::port::types::FlavorCreateParams),
    DeleteFlavor { id: String },
    FetchAggregates,
    FetchComputeServices,
    FetchHypervisors,

    // Neutron
    FetchNetworks,
    CreateNetwork(crate::port::types::NetworkCreateParams),
    FetchSecurityGroups,
    CreateSecurityGroup(crate::port::types::SecurityGroupCreateParams),
    DeleteSecurityGroup { id: String },
    CreateSecurityGroupRule(crate::port::types::SecurityGroupRuleCreateParams),
    DeleteSecurityGroupRule { rule_id: String },
    FetchFloatingIps,
    CreateFloatingIp { network_id: String },
    DeleteFloatingIp { id: String },
    FetchSubnets { network_id: String },
    FetchAgents,

    // Cinder
    FetchVolumes,
    CreateVolume(crate::port::types::VolumeCreateParams),
    DeleteVolume { id: String, force: bool },
    ExtendVolume { id: String, new_size: u32 },
    FetchSnapshots,
    CreateSnapshot(crate::port::types::SnapshotCreateParams),
    DeleteSnapshot { id: String },

    // Glance
    FetchImages,
    CreateImage(crate::port::types::ImageCreateParams),
    DeleteImage { id: String },

    // Keystone Admin
    FetchProjects,
    CreateProject(crate::port::types::ProjectCreateParams),
    DeleteProject { id: String },
    FetchUsers,
    CreateUser(crate::port::types::UserCreateParams),
    DeleteUser { id: String },

    // UI
    FocusSidebar,
    EnterFormMode,
    ExitFormMode,
    SelectResource { id: String },
    NavigateToResource { route: Route, id: String },

    // All Tenants
    ToggleAllTenants,

    // System
    RefreshAll,
    SwitchCloud(String),
    Quit,
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
            Action::SwitchCloud("prod".into()),
            Action::Quit,
        ];
        assert!(actions.len() >= 18);
    }
}
