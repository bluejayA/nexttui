use std::collections::HashMap;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use ratatui::Frame;
use ratatui::widgets::{Block, BorderType, Borders};
use tokio::sync::mpsc;

use crate::action::Action;
use crate::background::BackgroundTracker;
use crate::component::{Component, InputMode};
use crate::config::Config;
use crate::event::AppEvent;
use crate::infra::rbac::{ActionKind, RbacGuard};
use crate::models::common::Route;
use crate::router::Router;
use crate::ui::header::{Header, HeaderContext};
use crate::ui::layout::LayoutManager;
use crate::ui::sidebar::Sidebar;
use crate::ui::status_bar::{StatusBar, StatusInfo};
use crate::ui::theme::{self, Theme};
use crate::ui::toast::{ToastMessage, ToastSeverity};

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

    pub rbac: Arc<RbacGuard>,
    pub all_tenants: Arc<AtomicBool>,
    config: Arc<Config>,
    layout: LayoutManager,
    sidebar: Sidebar,
    header: Header,
    status_bar: StatusBar,
    route_labels: HashMap<Route, &'static str>,
}

impl App {
    pub fn new(config: Config, action_tx: mpsc::UnboundedSender<Action>) -> Self {
        Self {
            should_quit: false,
            input_mode: InputMode::Normal,
            sidebar_visible: true,
            focus: FocusPane::Content,
            router: Router::new(Route::Servers),
            components: HashMap::new(),
            background_tracker: BackgroundTracker::new(),
            action_tx,
            rbac: Arc::new(RbacGuard::new()),
            all_tenants: Arc::new(AtomicBool::new(false)),
            config: Arc::new(config),
            layout: LayoutManager::new(),
            sidebar: Sidebar::new(Vec::new()),
            header: Header::new(),
            status_bar: StatusBar::new(),
            route_labels: HashMap::new(),
        }
    }

    pub fn from_registry(
        config: Config,
        action_tx: mpsc::UnboundedSender<Action>,
        registry: crate::registry::ModuleRegistry,
        rbac: Arc<RbacGuard>,
    ) -> (Self, Vec<Action>) {
        let parts = registry.into_parts();
        let mut app = Self {
            should_quit: false,
            input_mode: InputMode::Normal,
            sidebar_visible: true,
            focus: FocusPane::Content,
            router: Router::new(Route::Servers),
            components: parts.components,
            background_tracker: BackgroundTracker::new(),
            action_tx,
            rbac,
            all_tenants: Arc::new(AtomicBool::new(false)),
            config: Arc::new(config),
            layout: LayoutManager::new(),
            sidebar: Sidebar::new(parts.sidebar_items),
            header: Header::new(),
            status_bar: StatusBar::new(),
            route_labels: parts.route_labels,
        };
        // Store sidebar items for number-key navigation
        app.sidebar.sync_active(&Route::Servers, false);
        app.broadcast_admin();
        (app, parts.initial_actions)
    }

    /// Broadcast current admin status to all registered modules.
    pub fn broadcast_admin(&mut self) {
        let is_admin = self.rbac.is_admin();
        for component in self.components.values_mut() {
            component.set_admin(is_admin);
        }
    }

    pub fn route_label(&self, route: &Route) -> &str {
        self.route_labels.get(route).copied().unwrap_or("Unknown")
    }

    /// Register a domain module component for a given route (test use only).
    #[cfg(test)]
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

        // Ctrl+a toggles all_tenants (admin only)
        if key.code == KeyCode::Char('a') && key.modifiers.contains(KeyModifiers::CONTROL) {
            if self.rbac.can_perform(ActionKind::ViewAllTenants) {
                self.dispatch_action(Action::ToggleAllTenants);
            }
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
                    let idx = if c == '0' { 9 } else { (c as usize) - ('1' as usize) };
                    if let Some(route) = self.sidebar.route_at(idx, self.rbac.is_admin()) {
                        self.dispatch_action(Action::Navigate(route));
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
                if let Some(action) = self.sidebar.handle_key(key, self.rbac.is_admin()) {
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
                self.sidebar.sync_active(&self.router.current(), self.rbac.is_admin());
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
            Action::ToggleAllTenants => {
                let prev = self.all_tenants.load(Ordering::Relaxed);
                self.all_tenants.store(!prev, Ordering::Relaxed);
                // Broadcast to modules
                for component in self.components.values_mut() {
                    component.set_all_tenants(!prev);
                }
                // Re-fetch all resources with new filter
                let fetches = [
                    Action::FetchServers,
                    Action::FetchNetworks,
                    Action::FetchSecurityGroups,
                    Action::FetchFloatingIps,
                    Action::FetchVolumes,
                    Action::FetchSnapshots,
                    Action::FetchImages,
                ];
                for a in fetches {
                    let _ = self.action_tx.send(a);
                }
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
        // RBAC: update roles on token refresh
        if let AppEvent::TokenRefreshed(ref roles) = event {
            self.rbac.update_roles(roles.clone(), None);
            self.broadcast_admin();
        }
        // Migration complete → refresh server list to reflect status change
        let refresh_servers = matches!(
            event,
            AppEvent::MigrationPollingStopped { .. }
            | AppEvent::ServerStatusPolled { .. }
        ) || matches!(
            event,
            AppEvent::ServerLiveMigrated { .. }
            | AppEvent::ServerColdMigrated { .. }
            | AppEvent::MigrationConfirmed { .. }
            | AppEvent::MigrationReverted { .. }
            | AppEvent::ServerEvacuated { .. }
            | AppEvent::ServerResized { .. }
            | AppEvent::ResizeConfirmed { .. }
            | AppEvent::ResizeReverted { .. }
        );
        self.generate_toast(&event);
        for component in self.components.values_mut() {
            component.handle_event(&event);
        }
        if refresh_servers {
            let _ = self.action_tx.send(Action::FetchServers);
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
            Action::LiveMigrateServer { .. } => Some("Live migrating server...".into()),
            Action::ColdMigrateServer { .. } => Some("Cold migrating server...".into()),
            Action::ConfirmMigration { .. } => Some("Confirming migration...".into()),
            Action::RevertMigration { .. } => Some("Reverting migration...".into()),
            Action::EvacuateServer { .. } => Some("Evacuating server...".into()),
            Action::ResizeServer { .. } => Some("Resizing server...".into()),
            Action::ConfirmResize { .. } => Some("Confirming resize...".into()),
            Action::RevertResize { .. } => Some("Reverting resize...".into()),
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
            // Migration
            AppEvent::ServerLiveMigrated { id } => (format!("Server {id} live migrated"), ToastLevel::Success),
            AppEvent::ServerColdMigrated { id } => (format!("Server {id} cold migrated — confirm(Y) or revert(N)"), ToastLevel::Success),
            AppEvent::MigrationConfirmed { id } => (format!("Migration confirmed for {id}"), ToastLevel::Success),
            AppEvent::MigrationReverted { id } => (format!("Migration reverted for {id}"), ToastLevel::Success),
            AppEvent::ServerEvacuated { id } => (format!("Server {id} evacuated"), ToastLevel::Success),
            // Resize
            AppEvent::ServerResized { id } => (format!("Server {id} resized — confirm(Y) or revert(N)"), ToastLevel::Success),
            AppEvent::ResizeConfirmed { id } => (format!("Resize confirmed for {id}"), ToastLevel::Success),
            AppEvent::ResizeReverted { id } => (format!("Resize reverted for {id}"), ToastLevel::Success),
            // Errors
            AppEvent::ApiError { operation, message } => (format!("{operation} failed: {message}"), ToastLevel::Error),
            AppEvent::AuthFailed(msg) => (format!("Auth failed: {msg}"), ToastLevel::Error),
            AppEvent::PermissionDenied { operation } => (format!("Permission denied: {operation}"), ToastLevel::Error),
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
        let route_label = self.route_label(&self.router.current());
        let cloud_config = self.config.active_cloud_config();
        let user_name = cloud_config.auth.username.clone().unwrap_or_default();
        let cloud_name = self.config.active_cloud_name().to_string();
        let region = cloud_config
            .region_name.as_deref().unwrap_or("default").to_string();
        self.header.render(frame, areas.header, &HeaderContext {
            user_name,
            cloud_name,
            region,
            all_tenants: self.all_tenants.load(Ordering::Relaxed),
        });

        // Sidebar
        if let Some(sidebar_area) = areas.sidebar {
            let sidebar_focused = self.focus == FocusPane::Sidebar;
            self.sidebar.render(frame, sidebar_area, self.rbac.is_admin(), &self.router.current(), sidebar_focused);
        }

        // Content
        if let Some(component) = self.components.get(&self.router.current()) {
            let content_focused = self.focus == FocusPane::Content;
            let content_border_style = if content_focused {
                Theme::focus_border()
            } else {
                Theme::unfocus_border()
            };
            let all_tenants = self.all_tenants.load(Ordering::Relaxed);
            let title = theme::panel_title_line(&route_label, content_focused, all_tenants);
            let content_block = Block::default()
                .title(title)
                .borders(Borders::ALL)
                .border_type(BorderType::Rounded)
                .border_style(content_border_style);
            let content_inner = content_block.inner(areas.content);
            frame.render_widget(content_block, areas.content);
            component.render(frame, content_inner);
        }

        // Status bar — context_hints from component help_hint or defaults
        let component_hint = self.components
            .get(&self.router.current())
            .map(|c| c.help_hint())
            .unwrap_or("");
        let context_hints: Vec<(String, String)> = if component_hint.is_empty() {
            vec![
                ("j/k".into(), "이동".into()),
                ("Enter".into(), "선택".into()),
                ("q".into(), "종료".into()),
            ]
        } else {
            component_hint
                .split(' ')
                .filter_map(|part| {
                    part.split_once(':').map(|(k, v)| (k.to_string(), v.to_string()))
                })
                .collect()
        };
        let info = StatusInfo {
            panel_name: route_label.to_string(),
            item_count: None,
            selected_index: None,
            context_hints,
        };
        // Toast — render in dedicated toast_bar area
        let active_toasts = self.background_tracker.active_toasts();
        if let Some(t) = active_toasts.first() {
            let toast_msg = ToastMessage {
                text: t.message.clone(),
                severity: ToastSeverity::from(t.level),
                resource_id: None,
            };
            toast_msg.render(frame, areas.toast_bar);
        }

        self.status_bar.render(frame, areas.status_bar, &info);
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

    #[test]
    fn test_app_rbac_is_admin() {
        let app = make_app();
        assert!(!app.rbac.is_admin());
    }

    #[test]
    fn test_app_broadcast_admin() {
        let mut app = make_app();
        app.register_component(Route::Servers, Box::new(MockComponent::new()));
        app.broadcast_admin();
    }

    #[test]
    fn test_app_sidebar_uses_rbac() {
        use crate::ui::sidebar::SidebarItem;
        let (tx, _rx) = mpsc::unbounded_channel();
        let config = test_config();
        let mut app = App::new(config, tx);
        // App with default RbacGuard (not admin)
        app.sidebar = Sidebar::new(vec![
            SidebarItem { label: "Servers".into(), route: Route::Servers, shortcut: "1".into(), admin_only: false },
            SidebarItem { label: "Projects".into(), route: Route::Projects, shortcut: "2".into(), admin_only: true },
        ]);
        // Key '2' maps to index 1. With is_admin=true, visible_items has 2 items, index 1 = Projects.
        // With is_admin=false (rbac default), visible_items has 1 item, index 1 = None.
        app.handle_key(make_key(KeyCode::Char('2')));
        // Should NOT navigate to Projects when not admin
        assert_eq!(app.router().current(), Route::Servers);
    }

    #[test]
    fn test_handle_token_refreshed_updates_rbac() {
        let mut app = make_app();
        assert!(!app.rbac.is_admin());
        let roles = vec![crate::port::types::TokenRole { id: "r1".into(), name: "admin".into() }];
        app.handle_event(AppEvent::TokenRefreshed(roles));
        assert!(app.rbac.is_admin());
    }

    #[test]
    fn test_dispatch_migration_action_adds_progress_toast() {
        let mut app = make_app();
        app.dispatch_action(Action::LiveMigrateServer {
            id: "s1".into(), host: None,
        });
        let toasts = app.background_tracker().active_toasts();
        assert!(toasts.iter().any(|t| t.message.contains("Live migrating")));
    }

    #[test]
    fn test_handle_cold_migrated_event_toast_and_refresh() {
        let (tx, mut rx) = mpsc::unbounded_channel();
        let config = test_config();
        let mut app = App::new(config, tx);
        app.handle_event(AppEvent::ServerColdMigrated { id: "s1".into() });
        let toasts = app.background_tracker().active_toasts();
        assert!(toasts.iter().any(|t| t.message.contains("confirm(Y) or revert(N)")));
        // Should have sent FetchServers for refresh
        let mut found = false;
        while let Ok(action) = rx.try_recv() {
            if matches!(action, Action::FetchServers) { found = true; }
        }
        assert!(found, "expected FetchServers after migration event");
    }

    #[test]
    fn test_handle_evacuated_event_adds_toast() {
        let mut app = make_app();
        app.handle_event(AppEvent::ServerEvacuated { id: "s1".into() });
        let toasts = app.background_tracker().active_toasts();
        assert!(toasts.iter().any(|t| t.message.contains("evacuated")));
    }

    #[test]
    fn test_handle_permission_denied_adds_toast() {
        let mut app = make_app();
        app.handle_event(AppEvent::PermissionDenied { operation: "CreateServer".into() });
        let toasts = app.background_tracker().active_toasts();
        assert_eq!(toasts.len(), 1);
        assert_eq!(toasts[0].level, crate::background::ToastLevel::Error);
        assert!(toasts[0].message.contains("Permission denied"));
    }
}
