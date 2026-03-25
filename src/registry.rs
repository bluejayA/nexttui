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
}

impl ModuleRegistry {
    pub fn new() -> Self {
        Self { entries: Vec::new() }
    }

    pub fn register(&mut self, entry: ModuleEntry) {
        self.entries.push(entry);
    }

    pub fn into_parts(self) -> RegistryParts {
        let mut components = HashMap::new();
        let mut sidebar_items = Vec::new();
        let mut initial_actions = Vec::new();
        let mut route_labels = HashMap::new();

        for entry in self.entries {
            let route = entry.sidebar.route;
            route_labels.insert(route, entry.display_name);
            for &related in entry.related_routes {
                route_labels.insert(related, entry.display_name);
            }
            sidebar_items.push(entry.sidebar);
            if let Some(action) = entry.initial_action {
                initial_actions.push(action);
            }
            components.insert(route, entry.component);
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
