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
                if let Some(msg) = Self::progress_toast_text(&other) {
                    self.background_tracker.add_toast(msg, crate::background::ToastLevel::Info);
                }
                let _ = self.action_tx.send(other);
            }
        }
    }

    /// Handle background event — broadcast to all registered components and generate toasts.
    /// Events like ServersLoaded must reach ServerModule even if the user is on a different view.
    pub fn handle_event(&mut self, event: AppEvent) {
        self.generate_toast(&event);
        for component in self.components.values_mut() {
            component.handle_event(&event);
        }
    }

    fn progress_toast_text(action: &Action) -> Option<String> {
        match action {
            Action::CreateServer(_) => Some("Creating server...".into()),
            Action::DeleteServer { name, .. } => Some(format!("Deleting server '{name}'...")),
            Action::RebootServer { .. } => Some("Rebooting server...".into()),
            Action::StartServer { .. } => Some("Starting server...".into()),
            Action::StopServer { .. } => Some("Stopping server...".into()),
            Action::CreateServerSnapshot { .. } => Some("Creating snapshot...".into()),
            Action::CreateFlavor(_) => Some("Creating flavor...".into()),
            Action::DeleteFlavor { .. } => Some("Deleting flavor...".into()),
            Action::CreateNetwork(_) => Some("Creating network...".into()),
            Action::CreateSecurityGroup(_) => Some("Creating security group...".into()),
            Action::DeleteSecurityGroup { .. } => Some("Deleting security group...".into()),
            Action::CreateSecurityGroupRule(_) => Some("Creating rule...".into()),
            Action::DeleteSecurityGroupRule { .. } => Some("Deleting rule...".into()),
            Action::CreateFloatingIp { .. } => Some("Creating floating IP...".into()),
            Action::DeleteFloatingIp { .. } => Some("Deleting floating IP...".into()),
            Action::CreateVolume(_) => Some("Creating volume...".into()),
            Action::DeleteVolume { .. } => Some("Deleting volume...".into()),
            Action::ExtendVolume { .. } => Some("Extending volume...".into()),
            Action::CreateSnapshot(_) => Some("Creating snapshot...".into()),
            Action::DeleteSnapshot { .. } => Some("Deleting snapshot...".into()),
            Action::CreateImage(_) => Some("Creating image...".into()),
            Action::DeleteImage { .. } => Some("Deleting image...".into()),
            Action::CreateProject(_) => Some("Creating project...".into()),
            Action::DeleteProject { .. } => Some("Deleting project...".into()),
            Action::CreateUser(_) => Some("Creating user...".into()),
            Action::DeleteUser { .. } => Some("Deleting user...".into()),
            _ => None,
        }
    }

    fn truncate_name(name: &str, max_len: usize) -> &str {
        if name.len() <= max_len {
            name
        } else {
            let mut end = max_len;
            while end > 0 && !name.is_char_boundary(end) {
                end -= 1;
            }
            &name[..end]
        }
    }

    fn generate_toast(&mut self, event: &AppEvent) {
        use crate::background::ToastLevel;
        const MAX_NAME: usize = 64;
        let (msg, level) = match event {
            // CUD success
            AppEvent::ServerCreated(s) => (format!("Server '{}' created", Self::truncate_name(&s.name, MAX_NAME)), ToastLevel::Success),
            AppEvent::ServerDeleted { name, .. } => (format!("Server '{}' deleted", Self::truncate_name(name, MAX_NAME)), ToastLevel::Success),
            AppEvent::ServerRebooted { id } => (format!("Server {id} rebooted"), ToastLevel::Success),
            AppEvent::ServerStarted { id } => (format!("Server {id} started"), ToastLevel::Success),
            AppEvent::ServerStopped { id } => (format!("Server {id} stopped"), ToastLevel::Success),
            AppEvent::ServerSnapshotCreated { server_id, .. } => (format!("Snapshot created for {server_id}"), ToastLevel::Success),
            AppEvent::FlavorCreated(f) => (format!("Flavor '{}' created", Self::truncate_name(&f.name, MAX_NAME)), ToastLevel::Success),
            AppEvent::FlavorDeleted { id } => (format!("Flavor {id} deleted"), ToastLevel::Success),
            AppEvent::NetworkCreated(n) => (format!("Network '{}' created", Self::truncate_name(&n.name, MAX_NAME)), ToastLevel::Success),
            AppEvent::SecurityGroupCreated(sg) => (format!("Security group '{}' created", Self::truncate_name(&sg.name, MAX_NAME)), ToastLevel::Success),
            AppEvent::SecurityGroupDeleted { id } => (format!("Security group {id} deleted"), ToastLevel::Success),
            AppEvent::SecurityGroupRuleCreated(_) => ("Security group rule created".into(), ToastLevel::Success),
            AppEvent::SecurityGroupRuleDeleted { .. } => ("Security group rule deleted".into(), ToastLevel::Success),
            AppEvent::VolumeCreated(v) => (format!("Volume '{}' created", Self::truncate_name(v.name.as_deref().unwrap_or(&v.id), MAX_NAME)), ToastLevel::Success),
            AppEvent::VolumeDeleted { id } => (format!("Volume {id} deleted"), ToastLevel::Success),
            AppEvent::VolumeExtended { id } => (format!("Volume {id} extended"), ToastLevel::Success),
            AppEvent::SnapshotCreated(s) => (format!("Snapshot '{}' created", Self::truncate_name(s.name.as_deref().unwrap_or(&s.id), MAX_NAME)), ToastLevel::Success),
            AppEvent::SnapshotDeleted { id } => (format!("Snapshot {id} deleted"), ToastLevel::Success),
            AppEvent::ImageCreated(i) => (format!("Image '{}' created", Self::truncate_name(&i.name, MAX_NAME)), ToastLevel::Success),
            AppEvent::ImageDeleted { id } => (format!("Image {id} deleted"), ToastLevel::Success),
            AppEvent::FloatingIpCreated(f) => (format!("Floating IP '{}' created", Self::truncate_name(&f.floating_ip_address, MAX_NAME)), ToastLevel::Success),
            AppEvent::FloatingIpDeleted { id } => (format!("Floating IP {id} deleted"), ToastLevel::Success),
            AppEvent::ProjectCreated(p) => (format!("Project '{}' created", Self::truncate_name(&p.name, MAX_NAME)), ToastLevel::Success),
            AppEvent::ProjectDeleted { id } => (format!("Project {id} deleted"), ToastLevel::Success),
            AppEvent::UserCreated(u) => (format!("User '{}' created", Self::truncate_name(&u.name, MAX_NAME)), ToastLevel::Success),
            AppEvent::UserDeleted { id } => (format!("User {id} deleted"), ToastLevel::Success),
            // Errors
            AppEvent::ApiError { operation, message } => (format!("{operation} failed: {message}"), ToastLevel::Error),
            AppEvent::AuthFailed(msg) => (format!("Auth failed: {msg}"), ToastLevel::Error),
            // Data loaded / system events — no toast
            _ => return,
        };
        self.background_tracker.add_toast(msg, level);
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
        let toast_messages: Vec<crate::ui::toast::ToastMessage> = self
            .background_tracker
            .active_toasts()
            .iter()
            .map(|t| {
                let severity = match t.level {
                    crate::background::ToastLevel::Success => crate::ui::toast::ToastSeverity::Success,
                    crate::background::ToastLevel::Error => crate::ui::toast::ToastSeverity::Error,
                    crate::background::ToastLevel::Info => crate::ui::toast::ToastSeverity::Info,
                };
                crate::ui::toast::ToastMessage {
                    text: t.message.clone(),
                    severity,
                }
            })
            .collect();
        self.status_bar.render(frame, areas.status_bar, &info, &toast_messages);
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

    #[test]
    fn test_dispatch_cud_action_adds_progress_toast() {
        let mut app = make_app();
        app.dispatch_action(Action::CreateServer(crate::port::types::ServerCreateParams {
            name: "web-01".into(),
            image_id: "img-1".into(),
            flavor_id: "flv-1".into(),
            networks: vec![],
            security_groups: None,
            key_name: None,
            availability_zone: None,
        }));
        let toasts = app.background_tracker().active_toasts();
        assert!(toasts.iter().any(|t| t.message.contains("Creating server")));
        assert!(toasts.iter().any(|t| t.level == crate::background::ToastLevel::Info));
    }

    #[test]
    fn test_handle_event_server_created_adds_toast() {
        let mut app = make_app();
        assert!(app.background_tracker().active_toasts().is_empty());
        let server: crate::models::nova::Server = serde_json::from_str(r#"{
            "id": "s1", "name": "web-01", "status": "ACTIVE",
            "addresses": {}, "flavor": {"id": "f1"}, "created": "2026-01-01"
        }"#).unwrap();
        app.handle_event(AppEvent::ServerCreated(server));
        let toasts = app.background_tracker().active_toasts();
        assert_eq!(toasts.len(), 1);
        assert_eq!(toasts[0].level, crate::background::ToastLevel::Success);
        assert!(toasts[0].message.contains("web-01"));
    }

    #[test]
    fn test_handle_event_api_error_adds_toast() {
        let mut app = make_app();
        app.handle_event(AppEvent::ApiError {
            operation: "CreateServer".into(),
            message: "quota exceeded".into(),
        });
        let toasts = app.background_tracker().active_toasts();
        assert_eq!(toasts.len(), 1);
        assert_eq!(toasts[0].level, crate::background::ToastLevel::Error);
        assert!(toasts[0].message.contains("quota exceeded"));
    }
}
