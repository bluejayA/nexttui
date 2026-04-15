use std::collections::HashMap;

use crate::action::Action;
use crate::component::Component;
use crate::models::common::Route;
use crate::ui::sidebar::SidebarItem;

pub struct ModuleEntry {
    pub sidebar: SidebarItem,
    pub component: Box<dyn Component>,
    pub initial_action: Option<Action>,
    pub related_routes: &'static [Route],
    pub display_name: &'static str,
}

pub struct RegistryParts {
    pub components: HashMap<Route, Box<dyn Component>>,
    pub sidebar_items: Vec<SidebarItem>,
    pub initial_actions: Vec<Action>,
    pub route_labels: HashMap<Route, &'static str>,
}

pub struct ModuleRegistry {
    entries: Vec<ModuleEntry>,
    extra_labels: Vec<(Route, &'static str)>,
}

impl ModuleRegistry {
    pub fn new() -> Self {
        Self { entries: Vec::new(), extra_labels: Vec::new() }
    }

    pub fn register(&mut self, entry: ModuleEntry) {
        self.entries.push(entry);
    }

    /// Add a display label for a route that has no module yet.
    pub fn add_route_label(&mut self, route: Route, label: &'static str) {
        self.extra_labels.push((route, label));
    }

    pub fn into_parts(self) -> RegistryParts {
        let mut components = HashMap::new();
        let mut sidebar_items = Vec::new();
        let mut initial_actions = Vec::new();
        let mut route_labels = HashMap::new();

        for entry in self.entries {
            let route = entry.sidebar.route;
            debug_assert!(!components.contains_key(&route), "Duplicate route registered: {:?}", route);
            route_labels.insert(route, entry.display_name);
            for &related in entry.related_routes {
                debug_assert!(!route_labels.contains_key(&related), "Duplicate related route: {:?}", related);
                route_labels.insert(related, entry.display_name);
            }
            sidebar_items.push(entry.sidebar);
            if let Some(action) = entry.initial_action {
                initial_actions.push(action);
            }
            components.insert(route, entry.component);
        }

        // Add extra labels for routes without modules
        for (route, label) in self.extra_labels {
            route_labels.entry(route).or_insert(label);
        }

        RegistryParts {
            components,
            sidebar_items,
            initial_actions,
            route_labels,
        }
    }
}

impl Default for ModuleRegistry {
    fn default() -> Self {
        Self::new()
    }
}

/// Register all standard modules. Shared by main.rs and demo.rs.
pub fn register_all_modules(
    registry: &mut ModuleRegistry,
    action_tx: &crate::context::ActionSender,
) {
    use crate::module::{
        server::ServerModule, flavor::FlavorModule, network::NetworkModule,
        security_group::SecurityGroupModule, floating_ip::FloatingIpModule,
        volume::VolumeModule, snapshot::SnapshotModule, image::ImageModule,
        project::ProjectModule, user::UserModule,
        host::HostModule, usage::UsageModule,
    };

    let entries = vec![
        ModuleEntry {
            sidebar: SidebarItem { label: "Servers".into(), route: Route::Servers, shortcut: "1".into(), admin_only: false },
            component: Box::new(ServerModule::new(action_tx.clone())),
            initial_action: Some(Action::FetchServers),
            related_routes: &[Route::ServerDetail, Route::ServerCreate],
            display_name: "Servers",
        },
        ModuleEntry {
            sidebar: SidebarItem { label: "Flavors".into(), route: Route::Flavors, shortcut: "2".into(), admin_only: false },
            component: Box::new(FlavorModule::new(action_tx.clone())),
            initial_action: Some(Action::FetchFlavors),
            related_routes: &[],
            display_name: "Flavors",
        },
        ModuleEntry {
            sidebar: SidebarItem { label: "Networks".into(), route: Route::Networks, shortcut: "3".into(), admin_only: false },
            component: Box::new(NetworkModule::new(action_tx.clone())),
            initial_action: Some(Action::FetchNetworks),
            related_routes: &[Route::NetworkDetail],
            display_name: "Networks",
        },
        ModuleEntry {
            sidebar: SidebarItem { label: "Security Groups".into(), route: Route::SecurityGroups, shortcut: "4".into(), admin_only: false },
            component: Box::new(SecurityGroupModule::new(action_tx.clone())),
            initial_action: Some(Action::FetchSecurityGroups),
            related_routes: &[Route::SecurityGroupDetail],
            display_name: "Security Groups",
        },
        ModuleEntry {
            sidebar: SidebarItem { label: "Floating IPs".into(), route: Route::FloatingIps, shortcut: "5".into(), admin_only: false },
            component: Box::new(FloatingIpModule::new(action_tx.clone())),
            initial_action: Some(Action::FetchFloatingIps),
            related_routes: &[],
            display_name: "Floating IPs",
        },
        ModuleEntry {
            sidebar: SidebarItem { label: "Volumes".into(), route: Route::Volumes, shortcut: "6".into(), admin_only: false },
            component: Box::new(VolumeModule::new(action_tx.clone())),
            initial_action: Some(Action::FetchVolumes),
            related_routes: &[Route::VolumeDetail, Route::VolumeCreate],
            display_name: "Volumes",
        },
        ModuleEntry {
            sidebar: SidebarItem { label: "Snapshots".into(), route: Route::Snapshots, shortcut: "7".into(), admin_only: false },
            component: Box::new(SnapshotModule::new(action_tx.clone())),
            initial_action: Some(Action::FetchSnapshots),
            related_routes: &[],
            display_name: "Snapshots",
        },
        ModuleEntry {
            sidebar: SidebarItem { label: "Images".into(), route: Route::Images, shortcut: "8".into(), admin_only: false },
            component: Box::new(ImageModule::new(action_tx.clone())),
            initial_action: Some(Action::FetchImages),
            related_routes: &[Route::ImageDetail],
            display_name: "Images",
        },
        ModuleEntry {
            sidebar: SidebarItem { label: "Projects".into(), route: Route::Projects, shortcut: "9".into(), admin_only: true },
            component: Box::new(ProjectModule::new(action_tx.clone())),
            initial_action: Some(Action::FetchProjects),
            related_routes: &[],
            display_name: "Projects",
        },
        ModuleEntry {
            sidebar: SidebarItem { label: "Users".into(), route: Route::Users, shortcut: "0".into(), admin_only: true },
            component: Box::new(UserModule::new(action_tx.clone())),
            initial_action: Some(Action::FetchUsers),
            related_routes: &[],
            display_name: "Users",
        },
        ModuleEntry {
            sidebar: SidebarItem { label: "Host Ops".into(), route: Route::Hosts, shortcut: "h".into(), admin_only: true },
            component: Box::new(HostModule::new(action_tx.clone())),
            initial_action: Some(Action::FetchHypervisors),
            related_routes: &[],
            display_name: "Host Ops",
        },
        ModuleEntry {
            sidebar: SidebarItem { label: "Usage".into(), route: Route::Usage, shortcut: "u".into(), admin_only: true },
            component: Box::new(UsageModule::new(action_tx.clone())),
            initial_action: None,
            related_routes: &[],
            display_name: "Usage",
        },
    ];

    for entry in entries {
        registry.register(entry);
    }

    // Routes with no module yet — display name only (no sidebar, no component)
    registry.add_route_label(Route::Migrations, "Migrations");
    registry.add_route_label(Route::Aggregates, "Aggregates");
    registry.add_route_label(Route::ComputeServices, "Compute Services");
    registry.add_route_label(Route::Hypervisors, "Hypervisors");
    registry.add_route_label(Route::Agents, "Agents");
}

#[cfg(test)]
mod tests {
    use super::*;
    use crossterm::event::KeyEvent;
    use ratatui::layout::Rect;
    use ratatui::Frame;
    use crate::event::AppEvent;

    struct DummyComponent;
    impl Component for DummyComponent {
        fn handle_key(&mut self, _key: KeyEvent) -> Option<Action> { None }
        fn handle_event(&mut self, _event: &AppEvent) {}
        fn render(&self, _frame: &mut Frame, _area: Rect) {}
    }

    fn make_entry(route: Route, label: &str, shortcut: &str, display_name: &'static str, related: &'static [Route], action: Option<Action>) -> ModuleEntry {
        ModuleEntry {
            sidebar: SidebarItem {
                label: label.into(),
                route,
                shortcut: shortcut.into(),
                admin_only: false,
            },
            component: Box::new(DummyComponent),
            initial_action: action,
            related_routes: related,
            display_name,
        }
    }

    #[test]
    fn test_module_entry_sidebar_item() {
        let entry = make_entry(Route::Servers, "Servers", "1", "Servers", &[], None);
        assert_eq!(entry.sidebar.label, "Servers");
        assert_eq!(entry.sidebar.route, Route::Servers);
        assert_eq!(entry.display_name, "Servers");
    }

    #[test]
    fn test_registry_register_and_count() {
        let mut registry = ModuleRegistry::new();
        registry.register(make_entry(Route::Servers, "Servers", "1", "Servers", &[], None));
        registry.register(make_entry(Route::Flavors, "Flavors", "2", "Flavors", &[], None));
        let parts = registry.into_parts();
        assert_eq!(parts.components.len(), 2);
        assert_eq!(parts.sidebar_items.len(), 2);
    }

    #[test]
    fn test_registry_into_parts_components() {
        let mut registry = ModuleRegistry::new();
        registry.register(make_entry(Route::Servers, "Servers", "1", "Servers", &[], None));
        let parts = registry.into_parts();
        assert!(parts.components.contains_key(&Route::Servers));
    }

    #[test]
    fn test_registry_into_parts_route_labels() {
        let mut registry = ModuleRegistry::new();
        registry.register(make_entry(
            Route::Servers, "Servers", "1", "Servers",
            &[Route::ServerDetail, Route::ServerCreate],
            None,
        ));
        let parts = registry.into_parts();
        assert_eq!(parts.route_labels.get(&Route::Servers), Some(&"Servers"));
        assert_eq!(parts.route_labels.get(&Route::ServerDetail), Some(&"Servers"));
        assert_eq!(parts.route_labels.get(&Route::ServerCreate), Some(&"Servers"));
    }

    #[test]
    fn test_registry_into_parts_initial_actions() {
        let mut registry = ModuleRegistry::new();
        registry.register(make_entry(Route::Servers, "Servers", "1", "Servers", &[], Some(Action::FetchServers)));
        registry.register(make_entry(Route::Flavors, "Flavors", "2", "Flavors", &[], None));
        registry.register(make_entry(Route::Networks, "Networks", "3", "Networks", &[], Some(Action::FetchNetworks)));
        let parts = registry.into_parts();
        assert_eq!(parts.initial_actions.len(), 2); // Servers + Networks, not Flavors
    }
}
