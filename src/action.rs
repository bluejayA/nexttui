use crate::models::common::Route;

#[derive(Debug)]
pub enum Action {
    // Navigation
    Navigate(Route),
    Back,

    // Nova
    FetchServers,
    DeleteServer { id: String, name: String },
    RebootServer { id: String, hard: bool },
    StartServer { id: String },
    StopServer { id: String },
    FetchFlavors,
    FetchAggregates,
    FetchComputeServices,
    FetchHypervisors,

    // Neutron
    FetchNetworks,
    FetchSecurityGroups,
    FetchFloatingIps,
    CreateFloatingIp { network_id: String },
    DeleteFloatingIp { id: String },
    FetchAgents,

    // Cinder
    FetchVolumes,
    FetchSnapshots,
    DeleteVolume { id: String, force: bool },
    ExtendVolume { id: String, new_size: u32 },

    // Glance
    FetchImages,
    DeleteImage { id: String },

    // Keystone Admin
    FetchProjects,
    FetchUsers,

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
        // Verify key variants compile and can be created
        let actions: Vec<Action> = vec![
            Action::Navigate(Route::Servers),
            Action::Back,
            Action::FetchServers,
            Action::DeleteServer {
                id: "s1".into(),
                name: "web".into(),
            },
            Action::RebootServer {
                id: "s1".into(),
                hard: false,
            },
            Action::FetchNetworks,
            Action::FetchVolumes,
            Action::FetchImages,
            Action::FetchProjects,
            Action::RefreshAll,
            Action::SwitchCloud("prod".into()),
            Action::Quit,
        ];
        assert!(actions.len() >= 12);
    }
}
