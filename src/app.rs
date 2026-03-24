use std::collections::HashMap;
use std::sync::Arc;

use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use ratatui::Frame;
use tokio::sync::mpsc;

use crate::action::Action;
use crate::background::BackgroundTracker;
use crate::component::{Component, InputMode};
use crate::config::Config;
use crate::event::AppEvent;
use crate::models::common::Route;
use crate::router::Router;
use crate::ui::header::{Header, HeaderContext};
use crate::ui::layout::LayoutManager;
use crate::ui::sidebar::{Sidebar, SidebarItem};
use crate::ui::status_bar::{StatusBar, StatusInfo};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FocusPane {
    Sidebar,
    Content,
}

pub struct App {
    pub should_quit: bool,
    pub input_mode: InputMode,
    pub sidebar_visible: bool,
    pub focus: FocusPane,

    router: Router,
    components: HashMap<Route, Box<dyn Component>>,
    background_tracker: BackgroundTracker,
    action_tx: mpsc::UnboundedSender<Action>,

    config: Arc<Config>,
    layout: LayoutManager,
    sidebar: Sidebar,
    header: Header,
    status_bar: StatusBar,
}

impl App {
    pub fn new(config: Config, action_tx: mpsc::UnboundedSender<Action>) -> Self {
        let sidebar = Sidebar::new(default_sidebar_items());
        Self {
            should_quit: false,
            input_mode: InputMode::Normal,
            sidebar_visible: true,
            focus: FocusPane::Content,
            router: Router::new(Route::Servers),
            components: HashMap::new(),
            background_tracker: BackgroundTracker::new(),
            action_tx,
            config: Arc::new(config),
            layout: LayoutManager::new(),
            sidebar,
            header: Header::new(),
            status_bar: StatusBar::new(),
        }
    }

    /// Register a domain module component for a given route.
    pub fn register_component(&mut self, route: Route, component: Box<dyn Component>) {
        self.components.insert(route, component);
    }

    /// Handle key input. Returns true if a re-render is needed.
    pub fn handle_key(&mut self, key: KeyEvent) -> bool {
        let no_modifiers = key.modifiers.is_empty();

        // Ctrl+c always quits
        if key.code == KeyCode::Char('c') && key.modifiers.contains(KeyModifiers::CONTROL) {
            self.should_quit = true;
            return true;
        }

        // Global keys in Normal mode (only without modifiers to avoid Ctrl+q etc.)
        if self.input_mode == InputMode::Normal && no_modifiers {
            match key.code {
                KeyCode::Char(':') => {
                    self.input_mode = InputMode::Command;
                    return true;
                }
                KeyCode::Char('/') => {
                    self.input_mode = InputMode::Search;
                    return true;
                }
                KeyCode::Tab => {
                    if self.sidebar_visible {
                        self.focus = match self.focus {
                            FocusPane::Content => FocusPane::Sidebar,
                            FocusPane::Sidebar => FocusPane::Content,
                        };
                    }
                    return true;
                }
                KeyCode::Char('q') => {
                    self.should_quit = true;
                    return true;
                }
                KeyCode::Char(c @ '1'..='9') | KeyCode::Char(c @ '0') => {
                    let items = default_sidebar_items();
                    let idx = if c == '0' { 9 } else { (c as usize) - ('1' as usize) };
                    if let Some(item) = items.get(idx) {
                        self.dispatch_action(Action::Navigate(item.route));
                    }
                    return true;
                }
                KeyCode::Esc => {
                    if self.focus == FocusPane::Sidebar {
                        self.focus = FocusPane::Content;
                        return true;
                    }
                    // Fall through to let component handle Esc
                    // (Detail→List transition, or return Action::Back for router)
                }
                _ => {}
            }
        }

        // Esc from Command/Search/Confirm → Normal
        if matches!(
            self.input_mode,
            InputMode::Command | InputMode::Search | InputMode::Confirm
        ) && key.code == KeyCode::Esc
        {
            self.input_mode = InputMode::Normal;
            return true;
        }

        // Form mode: delegate all keys to the active component (FormWidget handles everything)
        if self.input_mode == InputMode::Form {
            if let Some(component) = self.components.get_mut(&self.router.current()) {
                if let Some(action) = component.handle_key(key) {
                    self.dispatch_action(action);
                }
            }
            return true;
        }

        // Delegate based on focus pane
        if self.input_mode == InputMode::Normal {
            if self.focus == FocusPane::Sidebar && self.sidebar_visible {
                if let Some(action) = self.sidebar.handle_key(key, true) {
                    self.dispatch_action(action);
                }
                return true;
            }

            if let Some(component) = self.components.get_mut(&self.router.current()) {
                if let Some(action) = component.handle_key(key) {
                    self.dispatch_action(action);
                }
                return true;
            }

            // Fallback: Esc with no component registered → router back
            if key.code == KeyCode::Esc {
                self.router.back();
                return true;
            }
        }

        true
    }

    /// Handle action — intercept navigation actions, forward the rest to action_tx.
    fn dispatch_action(&mut self, action: Action) {
        match action {
            Action::Navigate(route) => {
                self.router.navigate(route);
                self.sidebar.sync_active(&self.router.current(), true);
                self.focus = FocusPane::Content;
            }
            Action::Back => {
                self.router.back();
            }
            Action::FocusSidebar => {
                if self.sidebar_visible {
                    self.focus = FocusPane::Sidebar;
                }
            }
            Action::EnterFormMode => {
                self.input_mode = InputMode::Form;
            }
            Action::ExitFormMode => {
                self.input_mode = InputMode::Normal;
            }
            Action::Quit => {
                self.should_quit = true;
            }
            other => {
                let _ = self.action_tx.send(other);
            }
        }
    }

    /// Handle background event — broadcast to all registered components.
    /// Events like ServersLoaded must reach ServerModule even if the user is on a different view.
    pub fn handle_event(&mut self, event: AppEvent) {
        for component in self.components.values_mut() {
            component.handle_event(&event);
        }
    }

    /// Tick handler: toast expiry, background tracker GC.
    pub fn on_tick(&mut self) {
        self.background_tracker.expire_toasts();
        self.background_tracker.gc_old_entries();
    }

    /// Render the UI.
    pub fn render(&self, frame: &mut Frame) {
        let areas = self.layout.calculate(frame.area());

        // Header
        let route_label = route_display_name(&self.router.current());
        let cloud_name = self.config.active_cloud_name().to_string();
        let region = self.config.active_cloud_config()
            .region_name.as_deref().unwrap_or("default").to_string();
        self.header.render(frame, areas.header, &HeaderContext {
            resource_type: route_label.to_string(),
            cloud_name,
            region,
        });

        // Sidebar
        if let Some(sidebar_area) = areas.sidebar {
            let sidebar_focused = self.focus == FocusPane::Sidebar;
            self.sidebar.render(frame, sidebar_area, true, &self.router.current(), sidebar_focused);
        }

        // Content
        if let Some(component) = self.components.get(&self.router.current()) {
            component.render(frame, areas.content);
        }

        // Status bar
        let info = StatusInfo {
            message: format!("{} | {:?}", route_label, self.input_mode),
            help_hint: "←→:Navigate q:Quit /:Search".into(),
            item_count: None,
            selected_index: None,
        };
        self.status_bar.render(frame, areas.status_bar, &info, &[]);
    }

    pub fn router(&self) -> &Router {
        &self.router
    }

    pub fn router_mut(&mut self) -> &mut Router {
        &mut self.router
    }

    pub fn background_tracker(&self) -> &BackgroundTracker {
        &self.background_tracker
    }

    pub fn background_tracker_mut(&mut self) -> &mut BackgroundTracker {
        &mut self.background_tracker
    }

    pub fn config(&self) -> &Config {
        &self.config
    }

    pub fn action_tx(&self) -> &mpsc::UnboundedSender<Action> {
        &self.action_tx
    }
}

fn default_sidebar_items() -> Vec<SidebarItem> {
    vec![
        SidebarItem { label: "Servers".into(), route: Route::Servers, shortcut: "1".into(), admin_only: false },
        SidebarItem { label: "Flavors".into(), route: Route::Flavors, shortcut: "2".into(), admin_only: false },
        SidebarItem { label: "Networks".into(), route: Route::Networks, shortcut: "3".into(), admin_only: false },
        SidebarItem { label: "Security Groups".into(), route: Route::SecurityGroups, shortcut: "4".into(), admin_only: false },
        SidebarItem { label: "Floating IPs".into(), route: Route::FloatingIps, shortcut: "5".into(), admin_only: false },
        SidebarItem { label: "Volumes".into(), route: Route::Volumes, shortcut: "6".into(), admin_only: false },
        SidebarItem { label: "Snapshots".into(), route: Route::Snapshots, shortcut: "7".into(), admin_only: false },
        SidebarItem { label: "Images".into(), route: Route::Images, shortcut: "8".into(), admin_only: false },
        SidebarItem { label: "Projects".into(), route: Route::Projects, shortcut: "9".into(), admin_only: true },
        SidebarItem { label: "Users".into(), route: Route::Users, shortcut: "0".into(), admin_only: true },
    ]
}

fn route_display_name(route: &Route) -> &'static str {
    match route {
        Route::Servers | Route::ServerDetail | Route::ServerCreate => "Servers",
        Route::Flavors => "Flavors",
        Route::Networks | Route::NetworkDetail => "Networks",
        Route::SecurityGroups | Route::SecurityGroupDetail => "Security Groups",
        Route::FloatingIps => "Floating IPs",
        Route::Volumes | Route::VolumeDetail | Route::VolumeCreate => "Volumes",
        Route::Snapshots => "Snapshots",
        Route::Images | Route::ImageDetail => "Images",
        Route::Projects => "Projects",
        Route::Users => "Users",
        Route::Migrations => "Migrations",
        Route::Aggregates => "Aggregates",
        Route::ComputeServices => "Compute Services",
        Route::Hypervisors => "Hypervisors",
        Route::Agents => "Agents",
        Route::Usage => "Usage",
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crossterm::event::{KeyCode, KeyEvent, KeyEventKind, KeyEventState, KeyModifiers};
    use ratatui::layout::Rect;

    fn make_key(code: KeyCode) -> KeyEvent {
        KeyEvent {
            code,
            modifiers: KeyModifiers::NONE,
            kind: KeyEventKind::Press,
            state: KeyEventState::NONE,
        }
    }

    struct MockComponent {
        last_key: Option<KeyCode>,
        last_event_received: bool,
    }

    impl MockComponent {
        fn new() -> Self {
            Self {
                last_key: None,
                last_event_received: false,
            }
        }
    }

    impl Component for MockComponent {
        fn handle_key(&mut self, key: KeyEvent) -> Option<Action> {
            self.last_key = Some(key.code);
            None
        }

        fn handle_event(&mut self, _event: &AppEvent) {
            self.last_event_received = true;
        }

        fn render(&self, _frame: &mut Frame, _area: Rect) {}
    }

    fn make_app() -> App {
        let (tx, _rx) = mpsc::unbounded_channel();
        let config = test_config();
        App::new(config, tx)
    }

    fn test_config() -> Config {
        // Use load_from with a temp file
        let dir = tempfile::TempDir::new().unwrap();
        let path = dir.path().join("clouds.yaml");
        std::fs::write(
            &path,
            "clouds:\n  test:\n    auth:\n      auth_url: https://keystone/v3\n      username: admin\n      password: secret\n",
        )
        .unwrap();
        Config::load_from(&path).unwrap()
    }

    #[test]
    fn test_app_global_key_colon() {
        let mut app = make_app();
        assert_eq!(app.input_mode, InputMode::Normal);
        app.handle_key(make_key(KeyCode::Char(':')));
        assert_eq!(app.input_mode, InputMode::Command);
    }

    #[test]
    fn test_app_global_key_slash() {
        let mut app = make_app();
        app.handle_key(make_key(KeyCode::Char('/')));
        assert_eq!(app.input_mode, InputMode::Search);
    }

    #[test]
    fn test_app_global_key_tab_focus_toggle() {
        let mut app = make_app();
        assert_eq!(app.focus, FocusPane::Content);
        app.handle_key(make_key(KeyCode::Tab));
        assert_eq!(app.focus, FocusPane::Sidebar);
        app.handle_key(make_key(KeyCode::Tab));
        assert_eq!(app.focus, FocusPane::Content);
    }

    #[test]
    fn test_app_global_key_q() {
        let mut app = make_app();
        assert!(!app.should_quit);
        app.handle_key(make_key(KeyCode::Char('q')));
        assert!(app.should_quit);
    }

    #[test]
    fn test_app_esc_to_normal() {
        let mut app = make_app();
        app.input_mode = InputMode::Command;
        app.handle_key(make_key(KeyCode::Esc));
        assert_eq!(app.input_mode, InputMode::Normal);
    }

    #[test]
    fn test_app_esc_normal_back() {
        let mut app = make_app();
        app.router_mut().navigate(Route::Networks);
        assert_eq!(app.router().current(), Route::Networks);
        app.handle_key(make_key(KeyCode::Esc));
        assert_eq!(app.router().current(), Route::Servers);
    }

    #[test]
    fn test_app_delegate_to_component() {
        let mut app = make_app();
        app.register_component(Route::Servers, Box::new(MockComponent::new()));

        // Delegating 'j' to the component should not panic
        // and should return true (needs re-render).
        let needs_render = app.handle_key(make_key(KeyCode::Char('j')));
        assert!(needs_render);
        // Verify component is still registered (not consumed)
        assert!(app.components.contains_key(&Route::Servers));
    }
}
