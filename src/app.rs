use std::collections::HashMap;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use ratatui::Frame;
use ratatui::widgets::{Block, BorderType, Borders};

use crate::action::Action;
use crate::background::BackgroundTracker;
use crate::component::{Component, InputMode, LayoutHint};
use crate::config::Config;
use crate::event::AppEvent;
use crate::infra::audit::{AuditEntry, AuditLogger, AuditResult};
use crate::infra::rbac::{ActionKind, RbacGuard};
use crate::input::command::{Command, CommandParser};
use crate::models::common::Route;
use crate::router::Router;
use crate::ui::activity_log::{ActivityLog, ActivityLogPopup};
use crate::ui::context_indicator::ContextIndicator;
use crate::ui::header::{Header, HeaderContext};
use crate::ui::input_bar::{InputAction, InputBar};
use crate::ui::layout::LayoutManager;
use crate::ui::refresh::RefreshScheduler;
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
    /// All writes must route through [`App::set_input_mode`] so the
    /// `input_bar` widget stays in sync with `input_mode` (BL-P2-073).
    /// `pub(crate)` blocks external crates from breaking that invariant;
    /// intra-crate test readers (`assert_eq!(app.input_mode, ...)`) still work.
    pub(crate) input_mode: InputMode,
    pub sidebar_visible: bool,
    pub focus: FocusPane,

    router: Router,
    components: HashMap<Route, Box<dyn Component>>,
    background_tracker: BackgroundTracker,
    action_tx: crate::context::ActionSender,

    pub rbac: Arc<RbacGuard>,
    pub all_tenants: Arc<AtomicBool>,
    /// Single authority for the active context generation. Bumped by the
    /// switcher (Unit 4); read by [`App::handle_versioned_event`] to drop
    /// stale events from previous generations. (BL-P2-031 Unit 2.)
    pub current_epoch: Arc<crate::context::ContextEpoch>,
    /// Orchestrator for runtime context switches. `None` until Unit 3b
    /// wires a real [`crate::port::context_session::ContextSessionPort`]
    /// adapter; `Action::SwitchContext` short-circuits to a toast in that
    /// state instead of silently dropping.
    switcher: Option<Arc<crate::context::ContextSwitcher>>,
    /// Event sender the switcher uses to publish `ContextChanged` after a
    /// successful commit, and a stamped `ApiError` on failure. Kept
    /// optional for tests that don't exercise the async switch path.
    event_tx: Option<tokio::sync::mpsc::UnboundedSender<crate::context::VersionedEvent<AppEvent>>>,
    /// Per-cloud project directory cache. When wired, `ContextChanged`
    /// events invalidate the relevant cloud's entries so the next
    /// `list_projects` call reflects the new token. (BL-P2-080 D2.)
    directory_cache: Option<Arc<crate::adapter::auth::DirectoryCache>>,
    config: Arc<Config>,
    layout: LayoutManager,
    sidebar: Sidebar,
    header: Header,
    status_bar: StatusBar,
    route_labels: HashMap<Route, &'static str>,
    refresh_scheduler: RefreshScheduler,
    activity_log: ActivityLog,
    activity_popup: ActivityLogPopup,
    show_activity_log: bool,
    audit_logger: Option<Arc<AuditLogger>>,
    /// Phase 7 폴리싱: shared with the background worker so a runtime
    /// cloud-switch updates the next `CrossProjectBlockEvent`'s `cloud`
    /// field. Wired post-construction via `set_actor_ctx`.
    actor_ctx: Option<Arc<std::sync::RwLock<crate::worker::ActorContext>>>,
    /// Command bar input widget (`:`-triggered). Paired with `command_parser`.
    pub(crate) input_bar: InputBar,
    pub(crate) command_parser: CommandParser,
    /// Active cloud/project identity rendered by the status bar. Updated from
    /// `AppEvent::ContextChanged` after a successful switch. (Unit 5 Step 3)
    pub(crate) context_indicator: ContextIndicator,
    /// Prefix captured at the start of a Tab-completion cycle. Reused on
    /// subsequent Tabs so the parser's cycling logic keeps seeing the same
    /// prefix instead of the expanded value. Reset on any non-Tab input.
    /// (Codex review 2차 P3)
    tab_cycle_prefix: Option<String>,
}

impl App {
    pub fn new(config: Config, action_tx: crate::context::ActionSender) -> Self {
        let tick_rate = std::time::Duration::from_millis(config.app_config().tick_rate_ms);
        crate::ui::theme::Theme::init(config.app_config().theme);
        let audit_logger = Self::init_audit_logger();
        let current_epoch = action_tx.epoch();
        let command_parser = Self::init_command_parser(&config);
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
            current_epoch,
            switcher: None,
            event_tx: None,
            directory_cache: None,
            config: Arc::new(config),
            layout: LayoutManager::new(),
            sidebar: Sidebar::new(Vec::new()),
            header: Header::new(),
            status_bar: StatusBar::new(),
            route_labels: HashMap::new(),
            refresh_scheduler: RefreshScheduler::new(tick_rate),
            activity_log: ActivityLog::new(),
            activity_popup: ActivityLogPopup::new(),
            show_activity_log: false,
            audit_logger,
            actor_ctx: None,
            input_bar: InputBar::new(),
            command_parser,
            context_indicator: ContextIndicator::new(std::time::Duration::from_secs(2)),
            tab_cycle_prefix: None,
        }
    }

    pub fn from_registry(
        config: Config,
        action_tx: crate::context::ActionSender,
        registry: crate::registry::ModuleRegistry,
        rbac: Arc<RbacGuard>,
    ) -> (Self, Vec<Action>) {
        let parts = registry.into_parts();
        let tick_rate = std::time::Duration::from_millis(config.app_config().tick_rate_ms);
        crate::ui::theme::Theme::init(config.app_config().theme);
        let audit_logger = Self::init_audit_logger();
        let current_epoch = action_tx.epoch();
        let command_parser = Self::init_command_parser(&config);
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
            current_epoch,
            switcher: None,
            event_tx: None,
            directory_cache: None,
            config: Arc::new(config),
            layout: LayoutManager::new(),
            sidebar: Sidebar::new(parts.sidebar_items),
            header: Header::new(),
            status_bar: StatusBar::new(),
            route_labels: parts.route_labels,
            refresh_scheduler: RefreshScheduler::new(tick_rate),
            activity_log: ActivityLog::new(),
            activity_popup: ActivityLogPopup::new(),
            show_activity_log: false,
            audit_logger,
            actor_ctx: None,
            input_bar: InputBar::new(),
            command_parser,
            context_indicator: ContextIndicator::new(std::time::Duration::from_secs(2)),
            tab_cycle_prefix: None,
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

    /// Inject an audit logger for testing.
    #[cfg(test)]
    pub fn set_audit_logger(&mut self, logger: AuditLogger) {
        self.audit_logger = Some(Arc::new(logger));
    }

    /// Handle key input. Returns true if a re-render is needed.
    pub fn handle_key(&mut self, key: KeyEvent) -> bool {
        let no_modifiers = key.modifiers.is_empty();

        // Activity log popup pseudo-modal: intercept j/k/Esc/! only
        if self.show_activity_log {
            match key.code {
                KeyCode::Char('j') => {
                    self.activity_popup
                        .scroll_down(self.activity_log.entries().len());
                }
                KeyCode::Char('k') => {
                    self.activity_popup.scroll_up();
                }
                KeyCode::Esc => {
                    self.show_activity_log = false;
                    self.activity_popup.reset_scroll();
                }
                KeyCode::Char('!') => {
                    self.show_activity_log = false;
                    self.activity_popup.reset_scroll();
                }
                KeyCode::Char('w') => {
                    let path = std::path::PathBuf::from("/tmp/nexttui-activity.log");
                    if let Err(e) = self.activity_log.export_to_file(&path) {
                        self.background_tracker.add_toast(
                            format!("Export failed: {e}"),
                            crate::background::ToastLevel::Error,
                        );
                    } else {
                        self.background_tracker.add_toast(
                            format!("Activity log exported to {}", path.display()),
                            crate::background::ToastLevel::Info,
                        );
                    }
                }
                _ => {}
            }
            return true;
        }

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

        // '!' toggle activity log (Shift+1 sends '!' with SHIFT modifier)
        if self.input_mode == InputMode::Normal && key.code == KeyCode::Char('!') {
            self.show_activity_log = !self.show_activity_log;
            if self.show_activity_log {
                self.activity_log.mark_all_read();
            }
            return true;
        }

        // Modal component (ConfirmDialog, SelectPopup) — delegate all keys directly
        if self.input_mode == InputMode::Normal {
            let is_modal = self
                .components
                .get(&self.router.current())
                .is_some_and(|c| c.is_modal());
            if is_modal {
                if let Some(component) = self.components.get_mut(&self.router.current())
                    && let Some(action) = component.handle_key(key)
                {
                    self.dispatch_action(action);
                }
                return true;
            }
        }

        // Command mode — delegate keys to InputBar, then convert InputAction
        // back into app-level effects. Runs before the Esc-to-Normal block so
        // Esc cancels via InputBar (which clears the buffer).
        if self.input_mode == InputMode::Command {
            // Codex review 2차 P3: any buffer-modifying key must reset the
            // Tab-cycle prefix. In Command mode InputBar does not emit
            // `SearchChanged` for Char/Backspace, so detect at the key level.
            if matches!(key.code, KeyCode::Char(_) | KeyCode::Backspace) {
                self.tab_cycle_prefix = None;
            }
            let act = self.input_bar.handle_key(key);
            match act {
                InputAction::Commit(buf) => {
                    self.tab_cycle_prefix = None;
                    self.set_input_mode(InputMode::Normal);
                    if !buf.trim().is_empty() {
                        let cmd = self.command_parser.parse(&buf);
                        // Only persist successfully-parsed commands so typos
                        // don't pollute history (PR3 cargo-review C2).
                        if !matches!(cmd, Command::Unknown(_)) {
                            self.command_parser.push_history(&buf);
                        }
                        self.execute_command(cmd);
                    }
                }
                InputAction::Cancel => {
                    self.tab_cycle_prefix = None;
                    self.set_input_mode(InputMode::Normal);
                }
                InputAction::AutoComplete => {
                    // Codex review 2차 P3: reuse the pre-expansion prefix so
                    // CommandParser's `last_prefix` gate stays matched and
                    // cycling works. Without this, the expanded buffer
                    // becomes the new prefix and subsequent Tabs only match
                    // the already-expanded command.
                    let prefix = self
                        .tab_cycle_prefix
                        .clone()
                        .unwrap_or_else(|| self.input_bar.buffer().to_string());
                    if let Some(expanded) = self.command_parser.auto_complete(&prefix) {
                        self.input_bar.set_buffer(expanded);
                        if self.tab_cycle_prefix.is_none() {
                            self.tab_cycle_prefix = Some(prefix);
                        }
                    }
                }
                InputAction::HistoryUp => {
                    self.tab_cycle_prefix = None;
                    if let Some(h) = self.command_parser.history_prev() {
                        self.input_bar.set_buffer(h.to_string());
                    }
                }
                InputAction::HistoryDown => {
                    self.tab_cycle_prefix = None;
                    if let Some(h) = self.command_parser.history_next() {
                        self.input_bar.set_buffer(h.to_string());
                    }
                }
                InputAction::SearchChanged(_) => {
                    self.tab_cycle_prefix = None;
                }
                InputAction::None => {}
            }
            return true;
        }

        // Global keys in Normal mode (only without modifiers to avoid Ctrl+q etc.)
        if self.input_mode == InputMode::Normal && no_modifiers {
            match key.code {
                KeyCode::Char(':') => {
                    self.set_input_mode(InputMode::Command);
                    return true;
                }
                // '/' search is handled by SelectPopup when open (not App-level)
                // KeyCode::Char('/') — disabled: App-level search mode is unimplemented
                KeyCode::Tab => {
                    // FullWidth module: Tab restores sidebar and returns to previous route
                    let full_width = self
                        .components
                        .get(&self.router.current())
                        .is_some_and(|c| c.layout_hint() == LayoutHint::FullWidth);
                    if full_width {
                        // Block exit while module is busy (e.g. evacuating)
                        let busy = self
                            .components
                            .get(&self.router.current())
                            .is_some_and(|c| c.is_busy());
                        if busy {
                            return true;
                        }
                        self.sidebar_visible = true;
                        self.layout.set_sidebar_visible(true);
                        self.router.back();
                        self.sidebar
                            .sync_active(&self.router.current(), self.rbac.is_admin());
                        self.focus = FocusPane::Sidebar;
                    } else if self.sidebar_visible {
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
                KeyCode::Char(c @ '1'..='9') | KeyCode::Char(c @ '0') | KeyCode::Char(c @ 'h') => {
                    // Block route switching while current module is busy (e.g. evacuating)
                    let busy = self
                        .components
                        .get(&self.router.current())
                        .is_some_and(|comp| comp.is_busy());
                    if busy {
                        return true;
                    }

                    if c == 'h' {
                        // 'h' shortcut for Host Ops
                        if self.rbac.is_admin() {
                            self.dispatch_action(Action::Navigate(Route::Hosts));
                        }
                    } else {
                        let idx = if c == '0' {
                            9
                        } else {
                            (c as usize) - ('1' as usize)
                        };
                        if let Some(route) = self.sidebar.route_at(idx, self.rbac.is_admin()) {
                            self.dispatch_action(Action::Navigate(route));
                        }
                    }
                    return true;
                }
                KeyCode::Esc if self.focus == FocusPane::Sidebar => {
                    self.focus = FocusPane::Content;
                    return true;
                }
                KeyCode::Esc => {
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
            self.set_input_mode(InputMode::Normal);
            return true;
        }

        // Form mode: delegate all keys to the active component (FormWidget handles everything)
        if self.input_mode == InputMode::Form {
            if let Some(component) = self.components.get_mut(&self.router.current())
                && let Some(action) = component.handle_key(key)
            {
                self.dispatch_action(action);
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
                self.sidebar
                    .sync_active(&self.router.current(), self.rbac.is_admin());
                self.focus = FocusPane::Content;
                // LayoutHint::FullWidth modules hide the sidebar
                let full_width = self
                    .components
                    .get(&self.router.current())
                    .is_some_and(|c| c.layout_hint() == LayoutHint::FullWidth);
                if full_width && self.sidebar_visible {
                    self.sidebar_visible = false;
                } else if !full_width && !self.sidebar_visible {
                    self.sidebar_visible = true;
                }
                self.layout.set_sidebar_visible(self.sidebar_visible);
                self.refresh_scheduler.reset();
            }
            Action::Back => {
                self.router.back();
                // Restore sidebar if leaving a FullWidth module
                let full_width = self
                    .components
                    .get(&self.router.current())
                    .is_some_and(|c| c.layout_hint() == LayoutHint::FullWidth);
                if !full_width && !self.sidebar_visible {
                    self.sidebar_visible = true;
                    self.layout.set_sidebar_visible(true);
                }
                self.sidebar
                    .sync_active(&self.router.current(), self.rbac.is_admin());
                self.refresh_scheduler.reset();
            }
            Action::FocusSidebar => {
                if self.sidebar_visible {
                    self.focus = FocusPane::Sidebar;
                }
            }
            Action::EnterFormMode => {
                self.set_input_mode(InputMode::Form);
            }
            Action::ExitFormMode => {
                self.set_input_mode(InputMode::Normal);
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
            Action::ShowToast { message } => {
                self.background_tracker
                    .add_toast(message, crate::background::ToastLevel::Info);
            }
            Action::Quit => {
                self.should_quit = true;
            }
            Action::SwitchContext(request) => {
                self.spawn_switch(request);
            }
            Action::SwitchBack => {
                self.spawn_switch_back();
            }
            other => {
                // Reject worker-bound actions while a switch is in
                // flight. The epoch is bumped at `try_begin` but auth
                // context isn't live until `commit`, so forwarding this
                // action would run it with the *old* session token but
                // under the *new* epoch stamp — a cross-context
                // mis-execution. Pure-UI actions (Navigate / Back /
                // FocusSidebar / EnterFormMode / …) are handled by the
                // earlier match arms, so this guard only fires for
                // port-bound work.
                if let Some(switcher) = self.switcher.as_ref()
                    && !switcher.is_idle()
                {
                    self.background_tracker.add_toast(
                        "Switch in progress — please wait".into(),
                        crate::background::ToastLevel::Info,
                    );
                    return;
                }
                if let Some(msg) = Self::progress_toast_text(&other) {
                    self.background_tracker
                        .add_toast(msg, crate::background::ToastLevel::Info);
                }
                let _ = self.action_tx.send(other);
            }
        }
    }

    /// Dispatcher entry point for events stamped with an epoch. Drops events
    /// whose epoch is older than [`App::current_epoch`] — this is the single
    /// authority for stale-event isolation across the runtime context switch
    /// flow (BL-P2-031 Unit 2). Returns `true` if the event was forwarded,
    /// `false` if it was dropped.
    pub fn handle_versioned_event(
        &mut self,
        event: crate::context::VersionedEvent<AppEvent>,
    ) -> bool {
        let event_epoch = event.epoch();
        let current = self.current_epoch.current();
        if event_epoch < current {
            tracing::debug!(
                event_epoch,
                current,
                "dropping stale event from previous context generation"
            );
            return false;
        }
        self.handle_event(event.into_inner());
        true
    }

    /// Handle background event — broadcast to all registered components and generate toasts.
    /// Events like ServersLoaded must reach ServerModule even if the user is on a different view.
    pub fn handle_event(&mut self, event: AppEvent) {
        // RBAC: update roles on token refresh
        if let AppEvent::TokenRefreshed(ref roles) = event {
            self.rbac.update_roles(roles.clone(), None);
            self.broadcast_admin();
        }
        // Context switch completion (Codex adversarial HIGH #1 / BL-P2-052
        // Part B safety portion). On `ContextChanged`:
        //   1. Refresh the status-bar indicator (Step 3).
        //   2. Broadcast `on_context_changed` to every module so they drop
        //      stale caches — blocks "wrong-context destructive action".
        //   3. Dispatch each module's `refresh_action` so the new project's
        //      data starts loading immediately.
        // The remaining UX bits (router/selection reset + toast) are tracked
        // by BL-P2-052 Part B leftovers.
        if let AppEvent::ContextChanged { ref target } = event {
            // Codex review 3차 P1: async `ContextChanged` can arrive while
            // the user is in Form/Command mode. The module reset below
            // leaves the form behind, but without normalizing `input_mode`
            // here subsequent keys stay routed through the Form-only path
            // (global shortcuts / command mode become unreachable) and the
            // UI is effectively stuck until quit.
            self.set_input_mode(InputMode::Normal);
            // Invalidate cached project lists for the new cloud so the next
            // `list_projects` call fetches with the fresh token. (BL-P2-080 D2.)
            if let Some(cache) = &self.directory_cache {
                cache.invalidate_cloud(&target.cloud);
            }
            // BL-P2-085 Phase 7 폴리싱: the worker reads `actor_ctx` live at
            // each block emit. Without this update, audit entries from the
            // worker stay anchored to the spawn-time cloud after the user
            // switches.
            if let Some(ref ctx) = self.actor_ctx
                && let Ok(mut guard) = ctx.write()
            {
                guard.cloud = target.cloud.clone();
            }
            self.context_indicator.set_target(target, true);
            for component in self.components.values_mut() {
                component.on_context_changed();
            }
            // Broadcast the target + highlight state so destructive dialogs
            // can attach a fingerprint and escalate to TypeToConfirm while the
            // switch is still visually fresh.
            let t = self.context_indicator.target().cloned();
            let recently = self.context_indicator.is_highlighting();
            for component in self.components.values_mut() {
                component.set_context_state(t.clone(), recently);
            }
            let refreshes: Vec<Action> = self
                .components
                .values()
                .filter_map(|c| c.refresh_action())
                .collect();
            for action in refreshes {
                let _ = self.action_tx.send(action);
            }
        }
        // Migration complete → refresh server list to reflect status change
        let refresh_servers = matches!(
            event,
            AppEvent::MigrationPollingStopped { .. } | AppEvent::ServerStatusPolled { .. }
        ) || matches!(
            event,
            AppEvent::ServerLiveMigrated { .. }
                | AppEvent::ServerColdMigrated { .. }
                | AppEvent::MigrationConfirmed { .. }
                | AppEvent::MigrationReverted { .. }
                | AppEvent::ServerEvacuated { .. }
                | AppEvent::ServerEvacuateResult { .. }
                | AppEvent::ServerResized { .. }
                | AppEvent::ResizeConfirmed { .. }
                | AppEvent::ResizeReverted { .. }
        );
        // API backoff: slow down refresh on rate-limit/unavailable errors.
        // NOTE: matches ApiError::RateLimited / ServiceUnavailable Display strings.
        // If those Display impls change, update these patterns (or add a typed field to AppEvent).
        match &event {
            AppEvent::ApiError { message, .. }
                if message.contains("Rate limited") || message.contains("unavailable") =>
            {
                self.refresh_scheduler.backoff();
            }
            AppEvent::ApiError { .. } => {}
            _ => {
                self.refresh_scheduler.reset_backoff();
            }
        }

        self.generate_toast(&event);
        self.record_audit(&event);
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

    /// Build the `CommandParser` with the configured history path and, in
    /// non-test builds, best-effort load prior history.
    fn init_command_parser(config: &Config) -> CommandParser {
        let history_path = config.app_config().command_history_path.clone();
        #[cfg(test)]
        {
            CommandParser::new(history_path)
        }
        #[cfg(not(test))]
        {
            let mut parser = CommandParser::new(history_path);
            let _ = parser.load_history();
            parser
        }
    }

    /// Initialize audit logger. Returns None on failure (non-fatal).
    /// Wrapped in `Arc` so the worker (BL-P2-085 Step 11b) can share the same
    /// instance — two `AuditLogger` instances on the same path would interleave
    /// writes through their independent `BufWriter`s.
    fn init_audit_logger() -> Option<Arc<AuditLogger>> {
        #[cfg(test)]
        {
            // In tests, do not create audit logger by default
            None
        }
        #[cfg(not(test))]
        {
            let path = crate::config::nexttui_config_dir().join("audit.log");
            match AuditLogger::new(path) {
                Ok(logger) => Some(Arc::new(logger)),
                Err(e) => {
                    tracing::warn!("Failed to initialize audit logger: {e}");
                    None
                }
            }
        }
    }

    /// FR2 Step 11b: handle to the audit logger so the worker can share the
    /// same `Arc` and emit `CrossProjectBlockEvent` entries through it.
    pub fn audit_logger_arc(&self) -> Option<Arc<AuditLogger>> {
        self.audit_logger.clone()
    }

    /// Phase 7 폴리싱: install the shared actor context so `ContextChanged`
    /// updates land in the worker's next audit entry. The Arc is held by
    /// both the worker and this `App`; mutations through the `RwLock` are
    /// visible to both sides without re-spawning the worker.
    pub fn set_actor_ctx(
        &mut self,
        ctx: Arc<std::sync::RwLock<crate::worker::ActorContext>>,
    ) {
        self.actor_ctx = Some(ctx);
    }

    /// Record a CUD event to the audit log. Errors are logged as warnings, never propagated.
    fn record_audit(&self, event: &AppEvent) {
        let Some(ref logger) = self.audit_logger else {
            return;
        };
        let Some(entry) = self.build_audit_entry(event) else {
            return;
        };
        if let Err(e) = logger.log_entry(entry) {
            tracing::warn!("Failed to write audit log: {e}");
        }
        if let Err(e) = logger.rotate_if_needed() {
            tracing::warn!("Failed to rotate audit log: {e}");
        }
    }

    /// Map an AppEvent to an AuditEntry. Returns None for non-auditable events.
    fn build_audit_entry(&self, event: &AppEvent) -> Option<AuditEntry> {
        let cloud = self.config.active_cloud_name().to_string();
        let user = self
            .config
            .active_cloud_config()
            .auth
            .username
            .clone()
            .unwrap_or_else(|| "unknown".to_string());
        let project = self.rbac.project_id();
        let timestamp = chrono::Local::now().to_rfc3339();

        let (action, resource_type, resource_id, resource_name, result) = match event {
            // Server CUD
            AppEvent::ServerCreated(s) => (
                "CreateServer",
                "server",
                s.id.clone(),
                Some(s.name.clone()),
                AuditResult::Success,
            ),
            AppEvent::ServerDeleted { id, name } => (
                "DeleteServer",
                "server",
                id.clone(),
                Some(name.clone()),
                AuditResult::Success,
            ),
            AppEvent::ServerRebooted { id } => (
                "RebootServer",
                "server",
                id.clone(),
                None,
                AuditResult::Success,
            ),
            AppEvent::ServerStarted { id } => (
                "StartServer",
                "server",
                id.clone(),
                None,
                AuditResult::Success,
            ),
            AppEvent::ServerStopped { id } => (
                "StopServer",
                "server",
                id.clone(),
                None,
                AuditResult::Success,
            ),
            AppEvent::ServerSnapshotCreated { server_id, .. } => (
                "CreateSnapshot",
                "server",
                server_id.clone(),
                None,
                AuditResult::Success,
            ),
            AppEvent::ServerResized { id } => (
                "ResizeServer",
                "server",
                id.clone(),
                None,
                AuditResult::Success,
            ),
            AppEvent::ServerLiveMigrated { id } => (
                "LiveMigrate",
                "server",
                id.clone(),
                None,
                AuditResult::Success,
            ),
            AppEvent::ServerColdMigrated { id } => (
                "ColdMigrate",
                "server",
                id.clone(),
                None,
                AuditResult::Success,
            ),
            AppEvent::ServerEvacuated { id } => {
                ("Evacuate", "server", id.clone(), None, AuditResult::Success)
            }

            // Volume CUD
            AppEvent::VolumeCreated(v) => (
                "CreateVolume",
                "volume",
                v.id.clone(),
                v.name.clone(),
                AuditResult::Success,
            ),
            AppEvent::VolumeDeleted { id } => (
                "DeleteVolume",
                "volume",
                id.clone(),
                None,
                AuditResult::Success,
            ),
            AppEvent::VolumeExtended { id } => (
                "ExtendVolume",
                "volume",
                id.clone(),
                None,
                AuditResult::Success,
            ),
            AppEvent::VolumeAttached {
                volume_id,
                server_id: _,
            } => (
                "AttachVolume",
                "volume",
                volume_id.clone(),
                None,
                AuditResult::Success,
            ),
            AppEvent::VolumeDetached { volume_id } => (
                "DetachVolume",
                "volume",
                volume_id.clone(),
                None,
                AuditResult::Success,
            ),
            AppEvent::VolumeForceDetached { volume_id } => (
                "ForceDetach",
                "volume",
                volume_id.clone(),
                None,
                AuditResult::Success,
            ),
            AppEvent::VolumeStateReset { volume_id } => (
                "ResetState",
                "volume",
                volume_id.clone(),
                None,
                AuditResult::Success,
            ),

            // Floating IP CUD
            AppEvent::FloatingIpCreated(f) => (
                "CreateFloatingIp",
                "floatingip",
                f.id.clone(),
                Some(f.floating_ip_address.clone()),
                AuditResult::Success,
            ),
            AppEvent::FloatingIpDeleted { id } => (
                "DeleteFloatingIp",
                "floatingip",
                id.clone(),
                None,
                AuditResult::Success,
            ),
            AppEvent::FloatingIpAssociated(f) => (
                "AssociateFloatingIp",
                "floatingip",
                f.id.clone(),
                Some(f.floating_ip_address.clone()),
                AuditResult::Success,
            ),
            AppEvent::FloatingIpDisassociated(f) => (
                "DisassociateFloatingIp",
                "floatingip",
                f.id.clone(),
                Some(f.floating_ip_address.clone()),
                AuditResult::Success,
            ),

            // Image CUD
            AppEvent::ImageCreated(i) => (
                "CreateImage",
                "image",
                i.id.clone(),
                Some(i.name.clone()),
                AuditResult::Success,
            ),
            AppEvent::ImageDeleted { id } => (
                "DeleteImage",
                "image",
                id.clone(),
                None,
                AuditResult::Success,
            ),

            // Network CUD
            AppEvent::NetworkCreated(n) => (
                "CreateNetwork",
                "network",
                n.id.clone(),
                Some(n.name.clone()),
                AuditResult::Success,
            ),

            // Security Group CUD
            AppEvent::SecurityGroupCreated(sg) => (
                "CreateSecurityGroup",
                "securitygroup",
                sg.id.clone(),
                Some(sg.name.clone()),
                AuditResult::Success,
            ),
            AppEvent::SecurityGroupDeleted { id } => (
                "DeleteSecurityGroup",
                "securitygroup",
                id.clone(),
                None,
                AuditResult::Success,
            ),
            AppEvent::SecurityGroupRuleCreated(r) => (
                "CreateSGRule",
                "sgRule",
                r.id.clone(),
                None,
                AuditResult::Success,
            ),
            AppEvent::SecurityGroupRuleDeleted { rule_id } => (
                "DeleteSGRule",
                "sgRule",
                rule_id.clone(),
                None,
                AuditResult::Success,
            ),

            // Snapshot CUD
            AppEvent::SnapshotCreated(s) => (
                "CreateSnapshot",
                "snapshot",
                s.id.clone(),
                s.name.clone(),
                AuditResult::Success,
            ),
            AppEvent::SnapshotDeleted { id } => (
                "DeleteSnapshot",
                "snapshot",
                id.clone(),
                None,
                AuditResult::Success,
            ),

            // Keystone CUD
            AppEvent::ProjectCreated(p) => (
                "CreateProject",
                "project",
                p.id.clone(),
                Some(p.name.clone()),
                AuditResult::Success,
            ),
            AppEvent::ProjectDeleted { id } => (
                "DeleteProject",
                "project",
                id.clone(),
                None,
                AuditResult::Success,
            ),
            AppEvent::UserCreated(u) => (
                "CreateUser",
                "user",
                u.id.clone(),
                Some(u.name.clone()),
                AuditResult::Success,
            ),
            AppEvent::UserDeleted { id } => {
                ("DeleteUser", "user", id.clone(), None, AuditResult::Success)
            }

            // Errors
            AppEvent::ApiError { operation, message } => (
                "ApiError",
                "error",
                String::new(),
                Some(operation.clone()),
                AuditResult::Failed(message.clone()),
            ),
            AppEvent::PermissionDenied { operation } => (
                "PermissionDenied",
                "permission",
                String::new(),
                Some(operation.clone()),
                AuditResult::Failed(format!("Permission denied: {operation}")),
            ),
            AppEvent::AuthFailed(msg) => (
                "AuthFailed",
                "auth",
                String::new(),
                None,
                AuditResult::Failed(msg.clone()),
            ),

            // Compute service toggle
            AppEvent::ComputeServiceToggled { hostname, enabled } => {
                let detail = if *enabled { "enabled" } else { "disabled" };
                return Some(AuditEntry {
                    timestamp,
                    cloud,
                    user,
                    project,
                    action: "ToggleService".to_string(),
                    resource_type: "service".to_string(),
                    resource_id: hostname.clone(),
                    resource_name: Some(hostname.clone()),
                    details: Some(serde_json::json!({ "enabled": *enabled, "state": detail })),
                    result: AuditResult::Success,
                });
            }

            // Non-auditable events (data loads, system, polling, etc.)
            _ => return None,
        };

        Some(AuditEntry {
            timestamp,
            cloud,
            user,
            project,
            action: action.to_string(),
            resource_type: resource_type.to_string(),
            resource_id,
            resource_name,
            details: None,
            result,
        })
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
        // Single match: (toast_message, level, operation, resource_name)
        let (msg, level, operation, resource_name) = match event {
            // CUD success
            AppEvent::ServerCreated(s) => (
                format!(
                    "Server '{}' created",
                    Self::truncate_name(&s.name, MAX_NAME)
                ),
                ToastLevel::Success,
                "Create".into(),
                s.name.clone(),
            ),
            AppEvent::ServerDeleted { name, .. } => (
                format!("Server '{}' deleted", Self::truncate_name(name, MAX_NAME)),
                ToastLevel::Success,
                "Delete".into(),
                name.clone(),
            ),
            AppEvent::ServerRebooted { id } => (
                format!("Server {id} rebooted"),
                ToastLevel::Success,
                "Reboot".into(),
                id.clone(),
            ),
            AppEvent::ServerStarted { id } => (
                format!("Server {id} started"),
                ToastLevel::Success,
                "Start".into(),
                id.clone(),
            ),
            AppEvent::ServerStopped { id } => (
                format!("Server {id} stopped"),
                ToastLevel::Success,
                "Stop".into(),
                id.clone(),
            ),
            AppEvent::ServerSnapshotCreated { server_id, .. } => (
                format!("Snapshot created for {server_id}"),
                ToastLevel::Success,
                "Snapshot".into(),
                server_id.clone(),
            ),
            AppEvent::FlavorCreated(f) => (
                format!(
                    "Flavor '{}' created",
                    Self::truncate_name(&f.name, MAX_NAME)
                ),
                ToastLevel::Success,
                "Create".into(),
                f.name.clone(),
            ),
            AppEvent::FlavorDeleted { id } => (
                format!("Flavor {id} deleted"),
                ToastLevel::Success,
                "Delete".into(),
                id.clone(),
            ),
            AppEvent::NetworkCreated(n) => (
                format!(
                    "Network '{}' created",
                    Self::truncate_name(&n.name, MAX_NAME)
                ),
                ToastLevel::Success,
                "Create".into(),
                n.name.clone(),
            ),
            AppEvent::SecurityGroupCreated(sg) => (
                format!(
                    "Security group '{}' created",
                    Self::truncate_name(&sg.name, MAX_NAME)
                ),
                ToastLevel::Success,
                "Create".into(),
                sg.name.clone(),
            ),
            AppEvent::SecurityGroupDeleted { id } => (
                format!("Security group {id} deleted"),
                ToastLevel::Success,
                "Delete".into(),
                id.clone(),
            ),
            AppEvent::SecurityGroupRuleCreated(_) => (
                "Security group rule created".into(),
                ToastLevel::Success,
                "Create".into(),
                "SG Rule".into(),
            ),
            AppEvent::SecurityGroupRuleDeleted { .. } => (
                "Security group rule deleted".into(),
                ToastLevel::Success,
                "Delete".into(),
                "SG Rule".into(),
            ),
            AppEvent::VolumeCreated(v) => (
                format!(
                    "Volume '{}' created",
                    Self::truncate_name(v.name.as_deref().unwrap_or(&v.id), MAX_NAME)
                ),
                ToastLevel::Success,
                "Create".into(),
                v.name.as_deref().unwrap_or(&v.id).to_string(),
            ),
            AppEvent::VolumeDeleted { id } => (
                format!("Volume {id} deleted"),
                ToastLevel::Success,
                "Delete".into(),
                id.clone(),
            ),
            AppEvent::VolumeExtended { id } => (
                format!("Volume {id} extended"),
                ToastLevel::Success,
                "Extend".into(),
                id.clone(),
            ),
            AppEvent::SnapshotCreated(s) => (
                format!(
                    "Snapshot '{}' created",
                    Self::truncate_name(s.name.as_deref().unwrap_or(&s.id), MAX_NAME)
                ),
                ToastLevel::Success,
                "Create".into(),
                s.name.as_deref().unwrap_or(&s.id).to_string(),
            ),
            AppEvent::SnapshotDeleted { id } => (
                format!("Snapshot {id} deleted"),
                ToastLevel::Success,
                "Delete".into(),
                id.clone(),
            ),
            AppEvent::ImageCreated(i) => (
                format!("Image '{}' created", Self::truncate_name(&i.name, MAX_NAME)),
                ToastLevel::Success,
                "Create".into(),
                i.name.clone(),
            ),
            AppEvent::ImageDeleted { id } => (
                format!("Image {id} deleted"),
                ToastLevel::Success,
                "Delete".into(),
                id.clone(),
            ),
            AppEvent::FloatingIpCreated(f) => (
                format!(
                    "Floating IP '{}' created",
                    Self::truncate_name(&f.floating_ip_address, MAX_NAME)
                ),
                ToastLevel::Success,
                "Create".into(),
                f.floating_ip_address.clone(),
            ),
            AppEvent::FloatingIpDeleted { id } => (
                format!("Floating IP {id} deleted"),
                ToastLevel::Success,
                "Delete".into(),
                id.clone(),
            ),
            AppEvent::ProjectCreated(p) => (
                format!(
                    "Project '{}' created",
                    Self::truncate_name(&p.name, MAX_NAME)
                ),
                ToastLevel::Success,
                "Create".into(),
                p.name.clone(),
            ),
            AppEvent::ProjectDeleted { id } => (
                format!("Project {id} deleted"),
                ToastLevel::Success,
                "Delete".into(),
                id.clone(),
            ),
            AppEvent::UserCreated(u) => (
                format!("User '{}' created", Self::truncate_name(&u.name, MAX_NAME)),
                ToastLevel::Success,
                "Create".into(),
                u.name.clone(),
            ),
            AppEvent::UserDeleted { id } => (
                format!("User {id} deleted"),
                ToastLevel::Success,
                "Delete".into(),
                id.clone(),
            ),
            // Migration
            AppEvent::ServerLiveMigrated { id } => (
                format!("Server {id} live migrated"),
                ToastLevel::Success,
                "LiveMigrate".into(),
                id.clone(),
            ),
            AppEvent::ServerColdMigrated { id } => (
                format!("Server {id} cold migrated — confirm(Y) or revert(N)"),
                ToastLevel::Success,
                "ColdMigrate".into(),
                id.clone(),
            ),
            AppEvent::MigrationConfirmed { id } => (
                format!("Migration confirmed for {id}"),
                ToastLevel::Success,
                "ConfirmMigration".into(),
                id.clone(),
            ),
            AppEvent::MigrationReverted { id } => (
                format!("Migration reverted for {id}"),
                ToastLevel::Success,
                "RevertMigration".into(),
                id.clone(),
            ),
            AppEvent::ServerEvacuated { id } => (
                format!("Server {id} evacuated"),
                ToastLevel::Success,
                "Evacuate".into(),
                id.clone(),
            ),
            // Resize
            AppEvent::ServerResized { id } => (
                format!("Server {id} resized — confirm(Y) or revert(N)"),
                ToastLevel::Success,
                "Resize".into(),
                id.clone(),
            ),
            AppEvent::ResizeConfirmed { id } => (
                format!("Resize confirmed for {id}"),
                ToastLevel::Success,
                "ConfirmResize".into(),
                id.clone(),
            ),
            AppEvent::ResizeReverted { id } => (
                format!("Resize reverted for {id}"),
                ToastLevel::Success,
                "RevertResize".into(),
                id.clone(),
            ),
            // Volume Attach/Detach
            AppEvent::VolumeAttached { volume_id, .. } => (
                format!("Volume {volume_id} attached successfully"),
                ToastLevel::Success,
                "AttachVolume".into(),
                volume_id.clone(),
            ),
            AppEvent::VolumeDetached { volume_id } => (
                format!("Volume {volume_id} detached successfully"),
                ToastLevel::Success,
                "DetachVolume".into(),
                volume_id.clone(),
            ),
            AppEvent::VolumeForceDetached { volume_id } => (
                format!("Volume {volume_id} force-detached (verify data integrity)"),
                ToastLevel::Success,
                "ForceDetachVolume".into(),
                volume_id.clone(),
            ),
            AppEvent::VolumeStateReset { volume_id } => (
                format!("Volume {volume_id} state reset to available"),
                ToastLevel::Success,
                "ResetVolumeState".into(),
                volume_id.clone(),
            ),
            // Floating IP Associate/Disassociate
            AppEvent::FloatingIpAssociated(f) => (
                format!(
                    "Floating IP {} associated successfully",
                    f.floating_ip_address
                ),
                ToastLevel::Success,
                "AssociateFloatingIp".into(),
                f.floating_ip_address.clone(),
            ),
            AppEvent::FloatingIpDisassociated(f) => (
                format!(
                    "FIP {} disassociated. Press 'a' to re-associate.",
                    f.floating_ip_address
                ),
                ToastLevel::Success,
                "DisassociateFloatingIp".into(),
                f.floating_ip_address.clone(),
            ),
            // Errors
            AppEvent::ApiError { operation, message } => (
                format!("{operation} failed: {message}"),
                ToastLevel::Error,
                operation.clone(),
                String::new(),
            ),
            AppEvent::AuthFailed(msg) => (
                format!("Auth failed: {msg}"),
                ToastLevel::Error,
                "Auth".into(),
                String::new(),
            ),
            AppEvent::PermissionDenied { operation } => (
                format!("Permission denied: {operation}"),
                ToastLevel::Error,
                operation.clone(),
                String::new(),
            ),
            // BL-P2-085 Step 11c: cross-project block surfaced from the worker.
            // Worker has already written the structured audit entry; the toast
            // is the user-visible counterpart and uses Error level (parity with
            // PermissionDenied) so it inherits the longer Error TTL.
            AppEvent::CrossProjectBlocked { reason, action } => (
                format!("Cross-project block: {action} ({reason})"),
                ToastLevel::Error,
                action.clone(),
                String::new(),
            ),
            // Data loaded / system events — no toast or activity log
            _ => return,
        };
        let success = !matches!(level, ToastLevel::Error);
        self.activity_log
            .push(crate::ui::activity_log::ActivityEntry {
                timestamp: std::time::Instant::now(),
                operation,
                resource_name,
                success,
                message: if success { String::new() } else { msg.clone() },
                read: false,
            });
        self.background_tracker.add_toast(msg, level);
    }

    /// Tick handler: toast expiry, background tracker GC, auto-refresh.
    pub fn on_tick(&mut self) {
        self.background_tracker.expire_toasts();
        self.background_tracker.gc_old_entries();

        // Re-broadcast the context state each tick so modules flip back to
        // `recently_switched = false` once the indicator's highlight window
        // expires. Runs before the input-mode early-return so destructive
        // dialogs inside confirm/command mode also see the transition.
        // (Codex review P2 — without this, recently_switched stays true
        // forever after the first `ContextChanged` and every destructive
        // dialog escalates to type-to-confirm.)
        let ctx_target = self.context_indicator.target().cloned();
        let ctx_recently = self.context_indicator.is_highlighting();
        for component in self.components.values_mut() {
            component.set_context_state(ctx_target.clone(), ctx_recently);
        }

        // Auto-refresh: skip when user is interacting
        if self.input_mode != InputMode::Normal {
            return;
        }
        let route = self.router.current();
        if let Some(component) = self.components.get(&route) {
            if component.is_modal() {
                return;
            }
            let has_transitional = component.has_transitional_resources();
            self.refresh_scheduler.set_fast(has_transitional);
            if self.refresh_scheduler.tick()
                && let Some(action) = component.refresh_action()
            {
                let _ = self.action_tx.send(action);
            }
        }
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
            .region_name
            .as_deref()
            .unwrap_or("default")
            .to_string();
        self.header.render(
            frame,
            areas.header,
            &HeaderContext {
                user_name,
                cloud_name,
                region,
                all_tenants: self.all_tenants.load(Ordering::Relaxed),
            },
        );

        // Sidebar
        if let Some(sidebar_area) = areas.sidebar {
            let sidebar_focused = self.focus == FocusPane::Sidebar;
            self.sidebar.render(
                frame,
                sidebar_area,
                self.rbac.is_admin(),
                &self.router.current(),
                sidebar_focused,
            );
        }

        // Content
        if let Some(component) = self.components.get(&self.router.current()) {
            if component.layout_hint() == LayoutHint::FullWidth {
                // FullWidth modules manage their own borders/layout
                component.render(frame, areas.content);
            } else {
                let content_focused = self.focus == FocusPane::Content;
                let content_border_style = if content_focused {
                    Theme::focus_border()
                } else {
                    Theme::unfocus_border()
                };
                let all_tenants = self.all_tenants.load(Ordering::Relaxed);
                let display_label = component
                    .content_title()
                    .unwrap_or_else(|| route_label.to_string());
                let title = theme::panel_title_line(&display_label, content_focused, all_tenants);
                let content_block = Block::default()
                    .title(title)
                    .borders(Borders::ALL)
                    .border_type(BorderType::Rounded)
                    .border_style(content_border_style);
                let content_inner = content_block.inner(areas.content);
                frame.render_widget(content_block, areas.content);
                component.render(frame, content_inner);
            }
        }

        // Activity log popup overlay
        if self.show_activity_log {
            self.activity_popup
                .render(frame, areas.content, self.activity_log.entries());
        }

        // Status bar — context_hints from component help_hint or defaults
        let component_hint = self
            .components
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
                    part.split_once(':')
                        .map(|(k, v)| (k.to_string(), v.to_string()))
                })
                .collect()
        };
        let info = StatusInfo {
            panel_name: route_label.to_string(),
            item_count: None,
            selected_index: None,
            context_hints,
            error_badge_count: self.activity_log.unread_error_count(),
        };
        // Command / Search / hint input bar
        self.input_bar.render(frame, areas.input_bar);

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

        self.status_bar
            .render(frame, areas.status_bar, &info, &self.context_indicator);
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

    /// Wire the context switcher + event sender post-construction. Main
    /// calls this once during startup; tests that exercise the switch path
    /// call it with a mock switcher. Kept separate from `new` so the App
    /// can still be built without a concrete `ContextSessionPort` adapter
    /// (Unit 3b).
    pub fn wire_context_switch(
        &mut self,
        switcher: Arc<crate::context::ContextSwitcher>,
        event_tx: tokio::sync::mpsc::UnboundedSender<crate::context::VersionedEvent<AppEvent>>,
    ) {
        self.switcher = Some(switcher);
        self.event_tx = Some(event_tx);
    }

    /// Wire the project directory cache post-construction. When wired,
    /// `ContextChanged` events invalidate the relevant cloud's entries so
    /// subsequent `list_projects` calls see the new token's project list.
    /// (BL-P2-080 D2.)
    pub fn wire_directory_cache(&mut self, cache: Arc<crate::adapter::auth::DirectoryCache>) {
        self.directory_cache = Some(cache);
    }

    /// Spawn an async switch. Emits `ContextChanged` on success or an
    /// `ApiError` stamped with the *current* epoch on failure (so the
    /// gate won't drop the error toast). Does nothing if the switcher
    /// isn't wired yet — surfaces a toast instead of silently dropping.
    fn spawn_switch(&mut self, request: crate::context::ContextRequest) {
        let Some(switcher) = self.switcher.clone() else {
            self.background_tracker.add_toast(
                "Context switch not available yet (runtime adapter pending)".into(),
                crate::background::ToastLevel::Error,
            );
            return;
        };
        let Some(event_tx) = self.event_tx.clone() else {
            tracing::error!("switcher wired without event_tx — impossible state");
            return;
        };
        tokio::spawn(async move {
            match switcher.switch(request).await {
                Ok((epoch, snapshot)) => {
                    let _ = event_tx.send(crate::context::VersionedEvent::new(
                        AppEvent::ContextChanged {
                            target: snapshot.target,
                        },
                        epoch,
                    ));
                }
                Err((epoch, err)) => {
                    // Switcher returns the attempt's epoch on failure —
                    // pre-begin errors use the last-committed epoch,
                    // post-begin errors use the bumped epoch. Either way
                    // the stamp survives the dispatcher gate without
                    // being "adopted" by a subsequent successful switch.
                    let _ = event_tx.send(crate::context::VersionedEvent::new(
                        AppEvent::ApiError {
                            operation: "SwitchContext".into(),
                            message: err.to_string(),
                        },
                        epoch,
                    ));
                }
            }
        });
    }

    fn spawn_switch_back(&mut self) {
        let Some(switcher) = self.switcher.clone() else {
            self.background_tracker.add_toast(
                "Context switch not available yet (runtime adapter pending)".into(),
                crate::background::ToastLevel::Error,
            );
            return;
        };
        let Some(event_tx) = self.event_tx.clone() else {
            return;
        };
        tokio::spawn(async move {
            match switcher.switch_back().await {
                Ok((epoch, snapshot)) => {
                    let _ = event_tx.send(crate::context::VersionedEvent::new(
                        AppEvent::ContextChanged {
                            target: snapshot.target,
                        },
                        epoch,
                    ));
                }
                Err((epoch, err)) => {
                    let _ = event_tx.send(crate::context::VersionedEvent::new(
                        AppEvent::ApiError {
                            operation: "SwitchBack".into(),
                            message: err.to_string(),
                        },
                        epoch,
                    ));
                }
            }
        });
    }

    pub fn action_tx(&self) -> &crate::context::ActionSender {
        &self.action_tx
    }

    /// Single write path for `input_mode` that keeps the `InputBar` widget in
    /// sync (BL-P2-073). Activates the bar for Command / Search, deactivates
    /// it for Normal / Form / Confirm. All direct `self.input_mode = ...`
    /// assignments must route through this method so the two representations
    /// can never drift apart.
    fn set_input_mode(&mut self, mode: InputMode) {
        self.input_mode = mode;
        match mode {
            InputMode::Command | InputMode::Search => {
                self.input_bar.activate(mode);
                self.command_parser.history_reset_cursor();
                self.command_parser.reset_completion();
            }
            InputMode::Normal | InputMode::Form | InputMode::Confirm => {
                self.input_bar.deactivate();
            }
        }
    }

    /// Persist user state before the process exits. Currently best-effort
    /// saves command history. Safe to call from any successful exit path
    /// (Ctrl+C, `:quit`, normal `q`, etc.). Errors are logged, never
    /// propagated, so terminal cleanup still runs. (BL-P2-071)
    pub fn shutdown(&self) {
        if let Err(e) = self.command_parser.save_history() {
            tracing::warn!(%e, "failed to save command history on shutdown");
        }
    }

    #[cfg(test)]
    pub(crate) fn set_command_history_path(&mut self, path: std::path::PathBuf) {
        self.command_parser = CommandParser::new(path);
    }

    /// Translate a parsed `Command` into app-level effects.
    /// Unit 4.5 Step B covers Navigate / Quit / Refresh / Help / Switch* and
    /// soft-deprecates the legacy `:ctx` variants with a toast. Unknown input
    /// surfaces as an error toast so nothing ever silently drops.
    fn execute_command(&mut self, cmd: Command) {
        match cmd {
            Command::Quit => {
                self.should_quit = true;
            }
            Command::Navigate(route) => {
                self.dispatch_action(Action::Navigate(route));
            }
            Command::Refresh => {
                if let Some(component) = self.components.get(&self.router.current())
                    && let Some(action) = component.refresh_action()
                {
                    self.dispatch_action(action);
                }
            }
            Command::Help => {
                self.background_tracker.add_toast(
                    "Help — :<route> navigate · :q quit · :refresh · :switch-project <name> · :switch-cloud <name> · :switch-back".into(),
                    crate::background::ToastLevel::Info,
                );
            }
            Command::SwitchProject(name) => {
                self.dispatch_action(Action::SwitchContext(
                    crate::context::ContextRequest::ByName {
                        cloud: None,
                        project: name,
                        domain: None,
                    },
                ));
            }
            Command::SwitchCloud(name) => {
                // BL-P2-074: resolve via `CloudConfig::default_project` and
                // delegate to the standard switch path. Errors (unknown
                // cloud, no default project, stale default) surface via
                // the existing `spawn_switch` error → ApiError toast.
                self.dispatch_action(Action::SwitchContext(
                    crate::context::ContextRequest::CloudOnly { cloud: name },
                ));
            }
            Command::SwitchBack => {
                self.dispatch_action(Action::SwitchBack);
            }
            Command::ContextSwitch(cloud) => {
                // Legacy `:ctx <cloud>` — soft-deprecated in favor of the
                // Unit 4.5/5 switch commands.
                self.background_tracker.add_toast(
                    format!("Legacy :ctx {cloud} — use :switch-cloud or :switch-project instead"),
                    crate::background::ToastLevel::Info,
                );
            }
            Command::ContextList => {
                let active = self.config.active_cloud_name();
                self.background_tracker.add_toast(
                    format!("Active cloud: {active} (picker: Ctrl+P — Unit 6)"),
                    crate::background::ToastLevel::Info,
                );
            }
            Command::Unknown(raw) => {
                let message = if raw.is_empty() {
                    "Unknown command".to_string()
                } else {
                    format!("Unknown command: {raw}")
                };
                self.background_tracker
                    .add_toast(message, crate::background::ToastLevel::Error);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crossterm::event::{KeyCode, KeyEvent, KeyEventKind, KeyEventState, KeyModifiers};
    use ratatui::layout::Rect;
    use std::time::Instant;

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
        let (tx, _rx) = crate::context::test_action_channel();
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
    fn test_app_global_key_slash_does_not_enter_search() {
        let mut app = make_app();
        app.handle_key(make_key(KeyCode::Char('/')));
        // '/' no longer activates App-level search (unimplemented)
        // Search is handled by SelectPopup when open
        assert_eq!(app.input_mode, InputMode::Normal);
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
        // Enter Command mode through the real user path so `input_bar` and
        // `input_mode` are both activated together.
        app.handle_key(make_key(KeyCode::Char(':')));
        assert_eq!(app.input_mode, InputMode::Command);
        app.handle_key(make_key(KeyCode::Esc));
        assert_eq!(app.input_mode, InputMode::Normal);
    }

    // --- Unit 4.5 Step A: Command Bar Integration ---

    #[test]
    fn test_command_bar_colon_activates_input_bar() {
        let mut app = make_app();
        app.handle_key(make_key(KeyCode::Char(':')));
        assert_eq!(app.input_mode, InputMode::Command);
        // Typing while in command mode accumulates in the input bar buffer.
        app.handle_key(make_key(KeyCode::Char('s')));
        app.handle_key(make_key(KeyCode::Char('r')));
        app.handle_key(make_key(KeyCode::Char('v')));
        assert_eq!(app.input_bar.buffer(), "srv");
    }

    #[test]
    fn test_command_bar_enter_commits_quit() {
        let mut app = make_app();
        app.handle_key(make_key(KeyCode::Char(':')));
        for c in "quit".chars() {
            app.handle_key(make_key(KeyCode::Char(c)));
        }
        app.handle_key(make_key(KeyCode::Enter));
        assert!(app.should_quit, "quit should flip should_quit");
        assert_eq!(app.input_mode, InputMode::Normal);
        assert_eq!(app.input_bar.buffer(), "");
    }

    #[test]
    fn test_command_bar_enter_commits_quit_abbrev_q() {
        let mut app = make_app();
        app.handle_key(make_key(KeyCode::Char(':')));
        app.handle_key(make_key(KeyCode::Char('q')));
        app.handle_key(make_key(KeyCode::Enter));
        assert!(app.should_quit);
        assert_eq!(app.input_mode, InputMode::Normal);
    }

    #[test]
    fn test_command_bar_empty_enter_returns_to_normal() {
        let mut app = make_app();
        app.handle_key(make_key(KeyCode::Char(':')));
        app.handle_key(make_key(KeyCode::Enter));
        assert_eq!(app.input_mode, InputMode::Normal);
        assert!(!app.should_quit);
    }

    #[test]
    fn test_command_bar_esc_cancels_and_clears_buffer() {
        let mut app = make_app();
        app.handle_key(make_key(KeyCode::Char(':')));
        app.handle_key(make_key(KeyCode::Char('x')));
        app.handle_key(make_key(KeyCode::Esc));
        assert_eq!(app.input_mode, InputMode::Normal);
        assert_eq!(app.input_bar.buffer(), "");
    }

    #[test]
    fn test_command_bar_tab_expands_abbreviation() {
        let mut app = make_app();
        app.handle_key(make_key(KeyCode::Char(':')));
        app.handle_key(make_key(KeyCode::Char('s')));
        app.handle_key(make_key(KeyCode::Char('r')));
        app.handle_key(make_key(KeyCode::Char('v')));
        app.handle_key(make_key(KeyCode::Tab));
        // "srv" is an abbreviation for "servers" — Tab expands the buffer.
        assert_eq!(app.input_bar.buffer(), "servers");
    }

    #[test]
    fn test_command_bar_tab_cycles_through_prefix_matches() {
        // Codex review 2차 P3: subsequent Tabs with the same prefix must
        // cycle through all matching commands, not get stuck on the first
        // expansion. Prefix "s" matches servers/security-groups/snapshots
        // (plus sb/sc/sp switch abbreviations).
        let mut app = make_app();
        app.handle_key(make_key(KeyCode::Char(':')));
        app.handle_key(make_key(KeyCode::Char('s')));
        app.handle_key(make_key(KeyCode::Tab));
        let first = app.input_bar.buffer().to_string();
        app.handle_key(make_key(KeyCode::Tab));
        let second = app.input_bar.buffer().to_string();
        assert_ne!(
            first, second,
            "Tab+Tab must cycle through matches, not stick at {first:?}"
        );
    }

    #[test]
    fn test_command_bar_typing_after_tab_resets_cycle() {
        // After typing, the next Tab must start cycling from the new buffer,
        // not from the stored pre-expansion prefix.
        let mut app = make_app();
        app.handle_key(make_key(KeyCode::Char(':')));
        app.handle_key(make_key(KeyCode::Char('s')));
        app.handle_key(make_key(KeyCode::Tab));
        // Backspace to edit buffer → resets the stored prefix.
        let mut backspaces = app.input_bar.buffer().chars().count();
        while backspaces > 0 {
            app.handle_key(make_key(KeyCode::Backspace));
            backspaces -= 1;
        }
        app.handle_key(make_key(KeyCode::Char('v')));
        app.handle_key(make_key(KeyCode::Tab));
        assert_eq!(
            app.input_bar.buffer(),
            "volumes",
            "after retyping prefix 'v', Tab should expand to 'volumes'"
        );
    }

    #[test]
    fn test_command_bar_history_up_restores_previous() {
        let mut app = make_app();
        // Use a valid command so it gets pushed to history (Unknown commands
        // are filtered out per PR3 cargo-review C2).
        app.handle_key(make_key(KeyCode::Char(':')));
        for c in "servers".chars() {
            app.handle_key(make_key(KeyCode::Char(c)));
        }
        app.handle_key(make_key(KeyCode::Enter));
        // Re-enter command mode and navigate up.
        app.handle_key(make_key(KeyCode::Char(':')));
        app.handle_key(make_key(KeyCode::Up));
        assert_eq!(app.input_bar.buffer(), "servers");
    }

    // --- Unit 5 Step 3: ContextIndicator wired to ContextChanged ---

    #[test]
    fn test_context_changed_updates_indicator() {
        use crate::context::types::ContextTarget;
        let mut app = make_app();
        assert!(app.context_indicator.target().is_none());
        app.handle_event(AppEvent::ContextChanged {
            target: ContextTarget {
                cloud: "devstack".into(),
                project_id: "p1".into(),
                project_name: "admin".into(),
                domain: "default".into(),
            },
        });
        let t = app
            .context_indicator
            .target()
            .expect("indicator should be set after ContextChanged");
        assert_eq!(t.cloud, "devstack");
        assert_eq!(t.project_name, "admin");
        // The switch marks a highlight.
        assert!(app.context_indicator.is_highlighting());
    }

    #[test]
    fn test_context_changed_resets_input_mode_to_normal() {
        // Codex review 3차 P1: async `ContextChanged` can arrive while the
        // user is in Form mode. Without normalizing `input_mode`, subsequent
        // keys stay routed through the Form-only path and the UI is stuck
        // (no `:` command mode, no global shortcuts) until quit.
        use crate::context::types::ContextTarget;
        let mut app = make_app();
        app.set_input_mode(InputMode::Form);
        assert_eq!(app.input_mode, InputMode::Form);
        app.handle_event(AppEvent::ContextChanged {
            target: ContextTarget {
                cloud: "devstack".into(),
                project_id: "p1".into(),
                project_name: "admin".into(),
                domain: "default".into(),
            },
        });
        assert_eq!(
            app.input_mode,
            InputMode::Normal,
            "ContextChanged must restore input_mode to Normal"
        );
    }

    // Spy component that records the last (target, recently_switched) pair
    // delivered via `Component::set_context_state`. Used to verify the
    // `on_tick` re-broadcast flips `recently_switched` back to false once the
    // indicator's highlight window expires (Codex review P2).
    #[derive(Default)]
    struct ContextSpyState {
        last_target: Option<crate::context::types::ContextTarget>,
        last_recently: bool,
        calls: usize,
    }

    struct ContextStateSpy {
        state: std::rc::Rc<std::cell::RefCell<ContextSpyState>>,
    }

    impl ContextStateSpy {
        fn new() -> (Self, std::rc::Rc<std::cell::RefCell<ContextSpyState>>) {
            let state = std::rc::Rc::new(std::cell::RefCell::new(ContextSpyState::default()));
            (
                Self {
                    state: state.clone(),
                },
                state,
            )
        }
    }

    impl Component for ContextStateSpy {
        fn handle_key(&mut self, _key: KeyEvent) -> Option<Action> {
            None
        }
        fn handle_event(&mut self, _event: &AppEvent) {}
        fn render(&self, _frame: &mut Frame, _area: Rect) {}
        fn set_context_state(
            &mut self,
            target: Option<crate::context::types::ContextTarget>,
            recently_switched: bool,
        ) {
            let mut s = self.state.borrow_mut();
            s.last_target = target;
            s.last_recently = recently_switched;
            s.calls += 1;
        }
    }

    #[test]
    fn test_on_tick_rebroadcasts_recently_switched_when_highlight_expires() {
        use crate::context::types::ContextTarget;
        // Codex review P2: after the highlight window elapses, the next
        // `on_tick` must push `recently_switched = false` to every module,
        // otherwise destructive dialogs keep demanding the project name.
        let mut app = make_app();
        let (spy, state) = ContextStateSpy::new();
        app.register_component(Route::Servers, Box::new(spy));

        // Simulate a context switch — indicator goes hot, broadcast marks
        // recently=true on registered modules.
        app.handle_event(AppEvent::ContextChanged {
            target: ContextTarget {
                cloud: "devstack".into(),
                project_id: "p1".into(),
                project_name: "admin".into(),
                domain: "default".into(),
            },
        });
        assert!(
            state.borrow().last_recently,
            "recently=true right after switch"
        );
        let calls_after_switch = state.borrow().calls;
        assert!(calls_after_switch >= 1);

        // Rewind the indicator's last switch instant to simulate the
        // highlight window elapsing without sleeping.
        app.context_indicator.set_last_switch_at_for_test(
            std::time::Instant::now() - std::time::Duration::from_secs(10),
        );
        assert!(!app.context_indicator.is_highlighting());

        // on_tick must propagate the transition.
        app.on_tick();
        let s = state.borrow();
        assert!(
            !s.last_recently,
            "on_tick should flip recently back to false once highlight expires"
        );
        assert_eq!(
            s.last_target.as_ref().map(|t| t.project_name.as_str()),
            Some("admin")
        );
        assert!(
            s.calls > calls_after_switch,
            "on_tick should add a broadcast call"
        );
    }

    #[test]
    fn test_context_changed_dispatches_refresh_for_each_module() {
        // Codex adversarial HIGH #1: after switch, every registered module's
        // `refresh_action` must be dispatched so stale caches reload from the
        // new project. Combined with `on_context_changed` broadcast (module
        // tests), this closes the "wrong-context destructive action" gap.
        use crate::context::types::ContextTarget;
        let (tx, mut rx) = crate::context::test_action_channel();
        let config = test_config();
        let mut app = App::new(config, tx);
        app.register_component(
            Route::Servers,
            Box::new(RefreshMock::new(Action::FetchServers)),
        );
        app.register_component(
            Route::Networks,
            Box::new(RefreshMock::new(Action::FetchNetworks)),
        );

        app.handle_event(AppEvent::ContextChanged {
            target: ContextTarget {
                cloud: "devstack".into(),
                project_id: "p1".into(),
                project_name: "admin".into(),
                domain: "default".into(),
            },
        });

        let mut received: Vec<Action> = Vec::new();
        while let Ok(action) = rx.try_recv() {
            received.push(action);
        }
        assert!(
            received.iter().any(|a| matches!(a, Action::FetchServers)),
            "expected FetchServers among: {received:?}"
        );
        assert!(
            received.iter().any(|a| matches!(a, Action::FetchNetworks)),
            "expected FetchNetworks among: {received:?}"
        );
    }

    // --- BL-P2-073: InputMode sync invariants ---

    #[test]
    fn test_set_input_mode_command_activates_input_bar() {
        let mut app = make_app();
        app.set_input_mode(InputMode::Command);
        assert_eq!(app.input_mode, InputMode::Command);
        // InputBar must now capture typed characters.
        app.handle_key(make_key(KeyCode::Char('x')));
        assert_eq!(app.input_bar.buffer(), "x");
    }

    #[test]
    fn test_set_input_mode_normal_clears_input_bar_buffer() {
        let mut app = make_app();
        app.set_input_mode(InputMode::Command);
        app.handle_key(make_key(KeyCode::Char('x')));
        assert_eq!(app.input_bar.buffer(), "x");
        app.set_input_mode(InputMode::Normal);
        assert_eq!(app.input_bar.buffer(), "");
    }

    #[test]
    fn test_set_input_mode_form_deactivates_input_bar() {
        let mut app = make_app();
        app.set_input_mode(InputMode::Command);
        app.handle_key(make_key(KeyCode::Char('x')));
        // Switching to Form must drop any in-flight command buffer so the
        // bar cannot show stale input while a form owns the screen.
        app.set_input_mode(InputMode::Form);
        assert_eq!(app.input_mode, InputMode::Form);
        assert_eq!(app.input_bar.buffer(), "");
    }

    #[test]
    fn test_set_input_mode_confirm_deactivates_input_bar() {
        let mut app = make_app();
        app.set_input_mode(InputMode::Command);
        app.handle_key(make_key(KeyCode::Char('x')));
        app.set_input_mode(InputMode::Confirm);
        assert_eq!(app.input_mode, InputMode::Confirm);
        assert_eq!(app.input_bar.buffer(), "");
    }

    #[test]
    fn test_shutdown_persists_command_history() {
        // BL-P2-071: shutdown() must save history so the next session can
        // restore previous commands via Up.
        let dir = tempfile::TempDir::new().unwrap();
        let path = dir.path().join("history");
        let mut app = make_app();
        app.set_command_history_path(path.clone());

        // Enter and commit a valid command so it's pushed to history.
        app.handle_key(make_key(KeyCode::Char(':')));
        for c in "servers".chars() {
            app.handle_key(make_key(KeyCode::Char(c)));
        }
        app.handle_key(make_key(KeyCode::Enter));

        app.shutdown();

        assert!(path.exists(), "history file must exist after shutdown");
        let content = std::fs::read_to_string(&path).unwrap();
        assert!(
            content.contains("servers"),
            "history file should contain 'servers', got: {content:?}"
        );
    }

    #[test]
    fn test_shutdown_with_no_commands_does_not_error() {
        let dir = tempfile::TempDir::new().unwrap();
        let path = dir.path().join("history");
        let mut app = make_app();
        app.set_command_history_path(path);
        // No commands entered. shutdown must be a no-op crash-wise.
        app.shutdown();
    }

    #[test]
    fn test_command_bar_unknown_not_pushed_to_history() {
        // PR3 cargo-review C2: unknown input must not pollute history.
        let mut app = make_app();
        // Push a valid command first.
        app.handle_key(make_key(KeyCode::Char(':')));
        for c in "servers".chars() {
            app.handle_key(make_key(KeyCode::Char(c)));
        }
        app.handle_key(make_key(KeyCode::Enter));
        // Then an unknown command — should NOT enter history.
        app.handle_key(make_key(KeyCode::Char(':')));
        for c in "foobar".chars() {
            app.handle_key(make_key(KeyCode::Char(c)));
        }
        app.handle_key(make_key(KeyCode::Enter));
        // Up restores the valid command, skipping the unknown one.
        app.handle_key(make_key(KeyCode::Char(':')));
        app.handle_key(make_key(KeyCode::Up));
        assert_eq!(app.input_bar.buffer(), "servers");
    }

    #[test]
    fn test_command_bar_backspace_shrinks_buffer() {
        let mut app = make_app();
        app.handle_key(make_key(KeyCode::Char(':')));
        app.handle_key(make_key(KeyCode::Char('a')));
        app.handle_key(make_key(KeyCode::Char('b')));
        app.handle_key(make_key(KeyCode::Backspace));
        assert_eq!(app.input_bar.buffer(), "a");
    }

    // --- Unit 4.5 Step B: Command → Action dispatch ---

    #[test]
    fn test_command_bar_navigate_servers() {
        let mut app = make_app();
        // Start elsewhere so the navigation is observable (default is Servers).
        app.router_mut().navigate(Route::Networks);
        app.handle_key(make_key(KeyCode::Char(':')));
        for c in "servers".chars() {
            app.handle_key(make_key(KeyCode::Char(c)));
        }
        app.handle_key(make_key(KeyCode::Enter));
        assert_eq!(app.router().current(), Route::Servers);
        assert_eq!(app.input_mode, InputMode::Normal);
    }

    #[test]
    fn test_command_bar_navigate_abbreviation() {
        let mut app = make_app();
        app.router_mut().navigate(Route::Servers);
        app.handle_key(make_key(KeyCode::Char(':')));
        for c in "net".chars() {
            app.handle_key(make_key(KeyCode::Char(c)));
        }
        app.handle_key(make_key(KeyCode::Enter));
        assert_eq!(app.router().current(), Route::Networks);
    }

    #[test]
    fn test_command_bar_switch_project_without_switcher_emits_toast() {
        let mut app = make_app();
        app.handle_key(make_key(KeyCode::Char(':')));
        for c in "switch-project admin".chars() {
            app.handle_key(make_key(KeyCode::Char(c)));
        }
        app.handle_key(make_key(KeyCode::Enter));
        let toasts = app.background_tracker().active_toasts();
        assert!(
            toasts
                .iter()
                .any(|t| t.message.to_lowercase().contains("not available")),
            "expected switcher-pending toast for switch-project, got: {:?}",
            toasts.iter().map(|t| t.message.clone()).collect::<Vec<_>>()
        );
    }

    #[test]
    fn test_command_bar_switch_back_without_switcher_emits_toast() {
        let mut app = make_app();
        app.handle_key(make_key(KeyCode::Char(':')));
        for c in "switch-back".chars() {
            app.handle_key(make_key(KeyCode::Char(c)));
        }
        app.handle_key(make_key(KeyCode::Enter));
        let toasts = app.background_tracker().active_toasts();
        assert!(
            toasts
                .iter()
                .any(|t| t.message.to_lowercase().contains("not available")),
            "expected switcher-pending toast for switch-back"
        );
    }

    /// BL-P2-074 FR-1: `:switch-cloud prod` routes to the switch pipeline
    /// instead of the legacy "not-available" stub. With no wired switcher
    /// (test context), the call falls through to `spawn_switch`, which
    /// emits the generic "Context switch not available yet (runtime
    /// adapter pending)" toast. The legacy stub phrasing
    /// (`switch-cloud <name>: not available yet — use :switch-project ...`)
    /// must no longer appear — its presence would mean the handler never
    /// dispatched `Action::SwitchContext(CloudOnly)`.
    #[test]
    fn test_command_bar_switch_cloud_dispatches_context_request_without_toast() {
        let mut app = make_app();
        app.handle_key(make_key(KeyCode::Char(':')));
        for c in "switch-cloud prod".chars() {
            app.handle_key(make_key(KeyCode::Char(c)));
        }
        app.handle_key(make_key(KeyCode::Enter));

        let toasts = app.background_tracker().active_toasts();
        let messages: Vec<String> = toasts.iter().map(|t| t.message.clone()).collect();

        // Legacy stub phrasing must be absent.
        assert!(
            !messages
                .iter()
                .any(|m| m.to_lowercase().contains("use :switch-project")
                    && m.to_lowercase().contains("switch-cloud")),
            "legacy stub toast must be removed, got: {messages:?}"
        );
        // spawn_switch path (switcher=None) toast must be present, confirming
        // the SwitchContext action reached the pipeline.
        assert!(
            messages
                .iter()
                .any(|m| m.to_lowercase().contains("context switch not available")),
            "expected spawn_switch fallback toast, got: {messages:?}"
        );
    }

    #[test]
    fn test_command_bar_help_emits_toast() {
        let mut app = make_app();
        app.handle_key(make_key(KeyCode::Char(':')));
        for c in "help".chars() {
            app.handle_key(make_key(KeyCode::Char(c)));
        }
        app.handle_key(make_key(KeyCode::Enter));
        let toasts = app.background_tracker().active_toasts();
        assert!(
            toasts.iter().any(|t| t.message.contains(":switch-project")
                || t.message.contains(":switch-back")
                || t.message.contains(":q")),
            "expected help toast listing commands, got: {:?}",
            toasts.iter().map(|t| t.message.clone()).collect::<Vec<_>>()
        );
    }

    #[test]
    fn test_command_bar_unknown_command_stays_in_normal() {
        let mut app = make_app();
        app.handle_key(make_key(KeyCode::Char(':')));
        for c in "foobar".chars() {
            app.handle_key(make_key(KeyCode::Char(c)));
        }
        app.handle_key(make_key(KeyCode::Enter));
        assert_eq!(app.input_mode, InputMode::Normal);
        assert!(!app.should_quit);
        // The unknown command must surface through the user-visible toast/log path,
        // not silently vanish. A warning-level toast is expected.
        let toasts = app.background_tracker().active_toasts();
        assert!(
            toasts
                .iter()
                .any(|t| t.message.to_lowercase().contains("unknown")
                    || t.message.contains("foobar")),
            "expected toast for unknown command, got: {:?}",
            toasts.iter().map(|t| t.message.clone()).collect::<Vec<_>>()
        );
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
        app.dispatch_action(Action::CreateServer(
            crate::port::types::ServerCreateParams {
                name: "web-01".into(),
                image_id: "img-1".into(),
                flavor_id: "flv-1".into(),
                networks: vec![],
                security_groups: None,
                key_name: None,
                availability_zone: None,
            },
        ));
        let toasts = app.background_tracker().active_toasts();
        assert!(toasts.iter().any(|t| t.message.contains("Creating server")));
        assert!(
            toasts
                .iter()
                .any(|t| t.level == crate::background::ToastLevel::Info)
        );
    }

    #[test]
    fn test_handle_event_server_created_adds_toast() {
        let mut app = make_app();
        assert!(app.background_tracker().active_toasts().is_empty());
        let server: crate::models::nova::Server = serde_json::from_str(
            r#"{
            "id": "s1", "name": "web-01", "status": "ACTIVE",
            "addresses": {}, "flavor": {"id": "f1"}, "created": "2026-01-01"
        }"#,
        )
        .unwrap();
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
        let (tx, _rx) = crate::context::test_action_channel();
        let config = test_config();
        let mut app = App::new(config, tx);
        // App with default RbacGuard (not admin)
        app.sidebar = Sidebar::new(vec![
            SidebarItem {
                label: "Servers".into(),
                route: Route::Servers,
                shortcut: "1".into(),
                admin_only: false,
            },
            SidebarItem {
                label: "Projects".into(),
                route: Route::Projects,
                shortcut: "2".into(),
                admin_only: true,
            },
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
        let roles = vec![crate::port::types::TokenRole {
            id: "r1".into(),
            name: "admin".into(),
        }];
        app.handle_event(AppEvent::TokenRefreshed(roles));
        assert!(app.rbac.is_admin());
    }

    #[test]
    fn test_dispatch_migration_action_adds_progress_toast() {
        let mut app = make_app();
        app.dispatch_action(Action::LiveMigrateServer {
            id: "s1".into(),
            host: None,
        });
        let toasts = app.background_tracker().active_toasts();
        assert!(toasts.iter().any(|t| t.message.contains("Live migrating")));
    }

    #[test]
    fn test_handle_cold_migrated_event_toast_and_refresh() {
        let (tx, mut rx) = crate::context::test_action_channel();
        let config = test_config();
        let mut app = App::new(config, tx);
        app.handle_event(AppEvent::ServerColdMigrated { id: "s1".into() });
        let toasts = app.background_tracker().active_toasts();
        assert!(
            toasts
                .iter()
                .any(|t| t.message.contains("confirm(Y) or revert(N)"))
        );
        // Should have sent FetchServers for refresh
        let mut found = false;
        while let Ok(action) = rx.try_recv() {
            if matches!(action, Action::FetchServers) {
                found = true;
            }
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
        app.handle_event(AppEvent::PermissionDenied {
            operation: "CreateServer".into(),
        });
        let toasts = app.background_tracker().active_toasts();
        assert_eq!(toasts.len(), 1);
        assert_eq!(toasts[0].level, crate::background::ToastLevel::Error);
        assert!(toasts[0].message.contains("Permission denied"));
    }

    // --- Step 5: on_tick refresh dispatch ---

    struct RefreshMock {
        action: Option<Action>,
        modal: bool,
        transitional: bool,
    }

    impl RefreshMock {
        fn new(action: Action) -> Self {
            Self {
                action: Some(action),
                modal: false,
                transitional: false,
            }
        }
    }

    impl Component for RefreshMock {
        fn handle_key(&mut self, _key: KeyEvent) -> Option<Action> {
            None
        }
        fn handle_event(&mut self, _event: &AppEvent) {}
        fn render(&self, _frame: &mut Frame, _area: Rect) {}
        fn refresh_action(&self) -> Option<Action> {
            self.action.clone()
        }
        fn is_modal(&self) -> bool {
            self.modal
        }
        fn has_transitional_resources(&self) -> bool {
            self.transitional
        }
    }

    #[test]
    fn test_on_tick_dispatches_refresh_action() {
        let (tx, mut rx) = crate::context::test_action_channel();
        let config = test_config();
        let mut app = App::new(config, tx);
        app.register_component(
            Route::Servers,
            Box::new(RefreshMock::new(Action::FetchServers)),
        );
        app.router = Router::new(Route::Servers);

        // Advance scheduler to trigger
        for _ in 0..150 {
            app.on_tick();
        }
        // Should have dispatched FetchServers
        let mut found = false;
        while let Ok(action) = rx.try_recv() {
            if matches!(action, Action::FetchServers) {
                found = true;
            }
        }
        assert!(found, "expected FetchServers to be dispatched");
    }

    #[test]
    fn test_on_tick_suppressed_when_form_mode() {
        let (tx, mut rx) = crate::context::test_action_channel();
        let config = test_config();
        let mut app = App::new(config, tx);
        app.register_component(
            Route::Servers,
            Box::new(RefreshMock::new(Action::FetchServers)),
        );
        app.router = Router::new(Route::Servers);
        app.set_input_mode(InputMode::Form);

        for _ in 0..150 {
            app.on_tick();
        }
        let mut found = false;
        while let Ok(action) = rx.try_recv() {
            if matches!(action, Action::FetchServers) {
                found = true;
            }
        }
        assert!(!found, "should not dispatch when in form mode");
    }

    #[test]
    fn test_on_tick_suppressed_when_modal() {
        let (tx, mut rx) = crate::context::test_action_channel();
        let config = test_config();
        let mut app = App::new(config, tx);
        let mut mock = RefreshMock::new(Action::FetchServers);
        mock.modal = true;
        app.register_component(Route::Servers, Box::new(mock));
        app.router = Router::new(Route::Servers);

        for _ in 0..150 {
            app.on_tick();
        }
        let mut found = false;
        while let Ok(action) = rx.try_recv() {
            if matches!(action, Action::FetchServers) {
                found = true;
            }
        }
        assert!(!found, "should not dispatch when modal is active");
    }

    // --- API Backoff ---

    #[test]
    fn test_api_error_rate_limited_triggers_backoff() {
        let (tx, mut rx) = crate::context::test_action_channel();
        let config = test_config();
        let mut app = App::new(config, tx);
        app.register_component(
            Route::Servers,
            Box::new(RefreshMock::new(Action::FetchServers)),
        );
        app.router = Router::new(Route::Servers);

        app.handle_event(AppEvent::ApiError {
            operation: "FetchServers".into(),
            message: "Rate limited: retry after 30s".into(),
        });

        // After backoff, 150 ticks should NOT trigger (needs 300 at 2x)
        for _ in 0..150 {
            app.on_tick();
        }
        let mut found = false;
        while let Ok(action) = rx.try_recv() {
            if matches!(action, Action::FetchServers) {
                found = true;
            }
        }
        assert!(
            !found,
            "should not trigger at 150 ticks after backoff (2x = 300 needed)"
        );
    }

    #[test]
    fn test_api_error_service_unavailable_triggers_backoff() {
        let (tx, mut rx) = crate::context::test_action_channel();
        let config = test_config();
        let mut app = App::new(config, tx);
        app.register_component(
            Route::Servers,
            Box::new(RefreshMock::new(Action::FetchServers)),
        );
        app.router = Router::new(Route::Servers);

        app.handle_event(AppEvent::ApiError {
            operation: "FetchServers".into(),
            message: "Service unavailable: nova".into(),
        });

        for _ in 0..150 {
            app.on_tick();
        }
        let mut found = false;
        while let Ok(action) = rx.try_recv() {
            if matches!(action, Action::FetchServers) {
                found = true;
            }
        }
        assert!(
            !found,
            "should not trigger at 150 ticks after backoff (2x = 300 needed)"
        );
    }

    #[test]
    fn test_success_event_resets_backoff() {
        let (tx, mut rx) = crate::context::test_action_channel();
        let config = test_config();
        let mut app = App::new(config, tx);
        app.register_component(
            Route::Servers,
            Box::new(RefreshMock::new(Action::FetchServers)),
        );
        app.router = Router::new(Route::Servers);

        // Trigger backoff
        app.handle_event(AppEvent::ApiError {
            operation: "FetchServers".into(),
            message: "Rate limited: retry after 30s".into(),
        });
        // Then success event resets backoff
        app.handle_event(AppEvent::ServersLoaded(vec![]));

        // After reset, 150 ticks should trigger (back to 1x)
        for _ in 0..150 {
            app.on_tick();
        }
        let mut found = false;
        while let Ok(action) = rx.try_recv() {
            if matches!(action, Action::FetchServers) {
                found = true;
            }
        }
        assert!(found, "should trigger at 150 ticks after backoff reset");
    }

    // --- Step 6: Navigate/Back reset ---

    // --- Unit 2: Activity Log Popup integration ---

    fn make_key_with_modifiers(code: KeyCode, modifiers: KeyModifiers) -> KeyEvent {
        KeyEvent {
            code,
            modifiers,
            kind: KeyEventKind::Press,
            state: KeyEventState::NONE,
        }
    }

    #[test]
    fn test_exclamation_toggles_show_activity_log() {
        let mut app = make_app();
        assert!(!app.show_activity_log);
        // '!' is Shift+1 in crossterm
        app.handle_key(make_key_with_modifiers(
            KeyCode::Char('!'),
            KeyModifiers::SHIFT,
        ));
        assert!(app.show_activity_log);
        app.handle_key(make_key_with_modifiers(
            KeyCode::Char('!'),
            KeyModifiers::SHIFT,
        ));
        assert!(!app.show_activity_log);
    }

    #[test]
    fn test_close_activity_popup_resets_scroll() {
        let mut app = make_app();
        // Open popup
        app.handle_key(make_key_with_modifiers(
            KeyCode::Char('!'),
            KeyModifiers::SHIFT,
        ));
        assert!(app.show_activity_log);
        // Scroll down
        app.handle_key(make_key(KeyCode::Char('j')));
        // Close with Esc
        app.handle_key(make_key(KeyCode::Esc));
        assert!(!app.show_activity_log);
        assert_eq!(app.activity_popup.scroll_offset(), 0);
    }

    #[test]
    fn test_exclamation_calls_mark_all_read_on_open() {
        let mut app = make_app();
        // Push an unread error entry
        app.activity_log
            .push(crate::ui::activity_log::ActivityEntry {
                timestamp: Instant::now(),
                operation: "Delete".into(),
                resource_name: "srv-1".into(),
                success: false,
                message: "fail".into(),
                read: false,
            });
        assert_eq!(app.activity_log.unread_error_count(), 1);
        // Open popup
        app.handle_key(make_key_with_modifiers(
            KeyCode::Char('!'),
            KeyModifiers::SHIFT,
        ));
        assert!(app.show_activity_log);
        assert_eq!(app.activity_log.unread_error_count(), 0);
    }

    #[test]
    fn test_exclamation_blocked_in_form_mode() {
        let mut app = make_app();
        app.set_input_mode(InputMode::Form);
        app.register_component(Route::Servers, Box::new(MockComponent::new()));
        app.handle_key(make_key_with_modifiers(
            KeyCode::Char('!'),
            KeyModifiers::SHIFT,
        ));
        assert!(!app.show_activity_log);
    }

    #[test]
    fn test_exclamation_blocked_in_confirm_mode() {
        let mut app = make_app();
        app.set_input_mode(InputMode::Confirm);
        app.register_component(Route::Servers, Box::new(MockComponent::new()));
        app.handle_key(make_key_with_modifiers(
            KeyCode::Char('!'),
            KeyModifiers::SHIFT,
        ));
        assert!(!app.show_activity_log);
    }

    #[test]
    fn test_fetch_success_not_logged_to_activity() {
        let mut app = make_app();
        app.generate_toast(&AppEvent::ServersLoaded(vec![]));
        assert!(app.activity_log.entries().is_empty());
    }

    #[test]
    fn test_activity_popup_pseudo_modal_blocks_keys() {
        let mut app = make_app();
        app.register_component(Route::Servers, Box::new(MockComponent::new()));
        app.show_activity_log = true;
        // 'q' should NOT quit when popup is open
        app.handle_key(make_key(KeyCode::Char('q')));
        assert!(!app.should_quit);
        // ':' should NOT switch to command mode
        app.handle_key(make_key(KeyCode::Char(':')));
        assert_eq!(app.input_mode, InputMode::Normal);
    }

    #[test]
    fn test_activity_popup_esc_closes() {
        let mut app = make_app();
        app.show_activity_log = true;
        app.handle_key(make_key(KeyCode::Esc));
        assert!(!app.show_activity_log);
    }

    #[test]
    fn test_activity_popup_j_k_scroll() {
        let mut app = make_app();
        app.show_activity_log = true;
        // Push entries so scroll_down works
        for i in 0..5 {
            app.activity_log
                .push(crate::ui::activity_log::ActivityEntry {
                    timestamp: Instant::now(),
                    operation: format!("Op{i}"),
                    resource_name: "r".into(),
                    success: true,
                    message: String::new(),
                    read: false,
                });
        }
        app.handle_key(make_key(KeyCode::Char('j')));
        assert_eq!(app.activity_popup.scroll_offset(), 1);
        app.handle_key(make_key(KeyCode::Char('k')));
        assert_eq!(app.activity_popup.scroll_offset(), 0);
    }

    #[test]
    fn test_generate_toast_pushes_to_activity_log() {
        let mut app = make_app();
        assert!(app.activity_log.entries().is_empty());
        app.handle_event(AppEvent::ServerDeleted {
            id: "s1".into(),
            name: "web-01".into(),
        });
        assert_eq!(app.activity_log.entries().len(), 1);
        let entry = &app.activity_log.entries()[0];
        assert!(entry.success);
        assert_eq!(entry.resource_name, "web-01");
    }

    #[test]
    fn test_generate_toast_error_pushes_to_activity_log() {
        let mut app = make_app();
        app.handle_event(AppEvent::ApiError {
            operation: "CreateServer".into(),
            message: "quota exceeded".into(),
        });
        assert_eq!(app.activity_log.entries().len(), 1);
        let entry = &app.activity_log.entries()[0];
        assert!(!entry.success);
        assert_eq!(entry.operation, "CreateServer");
        assert!(entry.message.contains("quota exceeded"));
    }

    // --- BL-P2-085 Step 11c: cross-project block toast ---

    #[test]
    fn test_generate_toast_for_cross_project_blocked_pushes_error() {
        let mut app = make_app();
        app.handle_event(AppEvent::CrossProjectBlocked {
            reason: "origin_scope_mismatch".into(),
            action: "DeleteServer".into(),
        });
        assert_eq!(app.activity_log.entries().len(), 1);
        let entry = &app.activity_log.entries()[0];
        assert!(
            !entry.success,
            "cross-project block must surface as a failure in the activity log",
        );
        assert_eq!(entry.operation, "DeleteServer");
        assert!(
            entry.message.contains("origin_scope_mismatch"),
            "toast message must include the reason: {msg}",
            msg = entry.message,
        );
    }

    #[test]
    fn test_error_badge_count_reflects_activity_log() {
        let mut app = make_app();
        // Two unread errors
        app.handle_event(AppEvent::ApiError {
            operation: "CreateServer".into(),
            message: "fail1".into(),
        });
        app.handle_event(AppEvent::ApiError {
            operation: "DeleteServer".into(),
            message: "fail2".into(),
        });
        assert_eq!(app.activity_log.unread_error_count(), 2);
        // Opening popup marks all read
        app.handle_key(make_key_with_modifiers(
            KeyCode::Char('!'),
            KeyModifiers::SHIFT,
        ));
        assert_eq!(app.activity_log.unread_error_count(), 0);
    }

    // --- Audit Logger integration ---

    fn make_app_with_audit() -> (App, tempfile::TempDir) {
        let (tx, _rx) = crate::context::test_action_channel();
        let config = test_config();
        let mut app = App::new(config, tx);
        let dir = tempfile::TempDir::new().unwrap();
        let path = dir.path().join("audit.log");
        let logger = crate::infra::audit::AuditLogger::new(path).unwrap();
        app.set_audit_logger(logger);
        (app, dir)
    }

    fn read_audit_lines(dir: &tempfile::TempDir) -> Vec<serde_json::Value> {
        let path = dir.path().join("audit.log");
        let content = std::fs::read_to_string(path).unwrap_or_default();
        content
            .lines()
            .filter(|l| !l.is_empty())
            .map(|l| serde_json::from_str(l).unwrap())
            .collect()
    }

    #[test]
    fn test_audit_server_created() {
        let (mut app, dir) = make_app_with_audit();
        let server: crate::models::nova::Server = serde_json::from_str(
            r#"{
            "id": "s1", "name": "web-01", "status": "ACTIVE",
            "addresses": {}, "flavor": {"id": "f1"}, "created": "2026-01-01"
        }"#,
        )
        .unwrap();
        app.handle_event(AppEvent::ServerCreated(server));
        let lines = read_audit_lines(&dir);
        assert_eq!(lines.len(), 1);
        assert_eq!(lines[0]["action"], "CreateServer");
        assert_eq!(lines[0]["resource_type"], "server");
        assert_eq!(lines[0]["resource_id"], "s1");
        assert_eq!(lines[0]["resource_name"], "web-01");
        assert_eq!(lines[0]["result"], "success");
    }

    #[test]
    fn test_audit_server_deleted() {
        let (mut app, dir) = make_app_with_audit();
        app.handle_event(AppEvent::ServerDeleted {
            id: "s1".into(),
            name: "web-01".into(),
        });
        let lines = read_audit_lines(&dir);
        assert_eq!(lines.len(), 1);
        assert_eq!(lines[0]["action"], "DeleteServer");
        assert_eq!(lines[0]["resource_id"], "s1");
        assert_eq!(lines[0]["resource_name"], "web-01");
    }

    #[test]
    fn test_audit_volume_created() {
        let (mut app, dir) = make_app_with_audit();
        let volume: crate::models::cinder::Volume = serde_json::from_str(
            r#"{
            "id": "v1", "name": "data-vol", "status": "available", "size": 100
        }"#,
        )
        .unwrap();
        app.handle_event(AppEvent::VolumeCreated(volume));
        let lines = read_audit_lines(&dir);
        assert_eq!(lines.len(), 1);
        assert_eq!(lines[0]["action"], "CreateVolume");
        assert_eq!(lines[0]["resource_type"], "volume");
        assert_eq!(lines[0]["resource_id"], "v1");
        assert_eq!(lines[0]["resource_name"], "data-vol");
    }

    #[test]
    fn test_audit_api_error() {
        let (mut app, dir) = make_app_with_audit();
        app.handle_event(AppEvent::ApiError {
            operation: "CreateServer".into(),
            message: "quota exceeded".into(),
        });
        let lines = read_audit_lines(&dir);
        assert_eq!(lines.len(), 1);
        assert_eq!(lines[0]["action"], "ApiError");
        assert_eq!(lines[0]["resource_type"], "error");
        let result = &lines[0]["result"];
        assert!(
            result.is_object() || result.as_str().is_some(),
            "result should indicate failure"
        );
    }

    #[test]
    fn test_audit_permission_denied() {
        let (mut app, dir) = make_app_with_audit();
        app.handle_event(AppEvent::PermissionDenied {
            operation: "DeleteServer".into(),
        });
        let lines = read_audit_lines(&dir);
        assert_eq!(lines.len(), 1);
        assert_eq!(lines[0]["action"], "PermissionDenied");
        assert_eq!(lines[0]["resource_type"], "permission");
    }

    #[test]
    fn test_audit_auth_failed() {
        let (mut app, dir) = make_app_with_audit();
        app.handle_event(AppEvent::AuthFailed("expired token".into()));
        let lines = read_audit_lines(&dir);
        assert_eq!(lines.len(), 1);
        assert_eq!(lines[0]["action"], "AuthFailed");
        assert_eq!(lines[0]["resource_type"], "auth");
    }

    #[test]
    fn test_audit_skips_data_loaded_events() {
        let (mut app, dir) = make_app_with_audit();
        app.handle_event(AppEvent::ServersLoaded(vec![]));
        app.handle_event(AppEvent::VolumesLoaded(vec![]));
        app.handle_event(AppEvent::ImagesLoaded(vec![]));
        let lines = read_audit_lines(&dir);
        assert!(lines.is_empty(), "data-load events should not be audited");
    }

    #[test]
    fn test_audit_floating_ip_created() {
        use crate::models::neutron::FloatingIp;
        let (mut app, dir) = make_app_with_audit();
        let fip = FloatingIp {
            id: "fip-1".into(),
            floating_ip_address: "203.0.113.10".into(),
            status: "ACTIVE".into(),
            port_id: None,
            floating_network_id: "ext-1".into(),
            fixed_ip_address: None,
            router_id: None,
            tenant_id: None,
        };
        app.handle_event(AppEvent::FloatingIpCreated(fip));
        let lines = read_audit_lines(&dir);
        assert_eq!(lines.len(), 1);
        assert_eq!(lines[0]["action"], "CreateFloatingIp");
        assert_eq!(lines[0]["resource_type"], "floatingip");
        assert_eq!(lines[0]["resource_id"], "fip-1");
    }

    #[test]
    fn test_audit_security_group_deleted() {
        let (mut app, dir) = make_app_with_audit();
        app.handle_event(AppEvent::SecurityGroupDeleted { id: "sg-1".into() });
        let lines = read_audit_lines(&dir);
        assert_eq!(lines.len(), 1);
        assert_eq!(lines[0]["action"], "DeleteSecurityGroup");
        assert_eq!(lines[0]["resource_type"], "securitygroup");
    }

    #[test]
    fn test_audit_multiple_events_produce_multiple_lines() {
        let (mut app, dir) = make_app_with_audit();
        app.handle_event(AppEvent::ServerRebooted { id: "s1".into() });
        app.handle_event(AppEvent::ServerStarted { id: "s2".into() });
        app.handle_event(AppEvent::ServerStopped { id: "s3".into() });
        let lines = read_audit_lines(&dir);
        assert_eq!(lines.len(), 3);
        assert_eq!(lines[0]["action"], "RebootServer");
        assert_eq!(lines[1]["action"], "StartServer");
        assert_eq!(lines[2]["action"], "StopServer");
    }

    #[test]
    fn test_audit_contains_cloud_and_user() {
        let (mut app, dir) = make_app_with_audit();
        app.handle_event(AppEvent::ServerRebooted { id: "s1".into() });
        let lines = read_audit_lines(&dir);
        assert_eq!(lines.len(), 1);
        // Cloud name comes from test_config (cloud name "test")
        assert_eq!(lines[0]["cloud"], "test");
        // User comes from test_config (username "admin")
        assert_eq!(lines[0]["user"], "admin");
    }

    #[test]
    fn test_audit_no_logger_does_not_panic() {
        // App without audit logger (default in tests)
        let mut app = make_app();
        // Should not panic even without audit logger
        app.handle_event(AppEvent::ServerRebooted { id: "s1".into() });
        app.handle_event(AppEvent::ApiError {
            operation: "op".into(),
            message: "err".into(),
        });
    }

    #[test]
    fn test_audit_volume_attach_detach() {
        let (mut app, dir) = make_app_with_audit();
        app.handle_event(AppEvent::VolumeAttached {
            volume_id: "v1".into(),
            server_id: "s1".into(),
        });
        app.handle_event(AppEvent::VolumeDetached {
            volume_id: "v2".into(),
        });
        app.handle_event(AppEvent::VolumeForceDetached {
            volume_id: "v3".into(),
        });
        app.handle_event(AppEvent::VolumeStateReset {
            volume_id: "v4".into(),
        });
        let lines = read_audit_lines(&dir);
        assert_eq!(lines.len(), 4);
        assert_eq!(lines[0]["action"], "AttachVolume");
        assert_eq!(lines[1]["action"], "DetachVolume");
        assert_eq!(lines[2]["action"], "ForceDetach");
        assert_eq!(lines[3]["action"], "ResetState");
    }

    #[test]
    fn test_audit_migration_events() {
        let (mut app, dir) = make_app_with_audit();
        app.handle_event(AppEvent::ServerLiveMigrated { id: "s1".into() });
        app.handle_event(AppEvent::ServerColdMigrated { id: "s2".into() });
        app.handle_event(AppEvent::ServerEvacuated { id: "s3".into() });
        let lines = read_audit_lines(&dir);
        assert_eq!(lines.len(), 3);
        assert_eq!(lines[0]["action"], "LiveMigrate");
        assert_eq!(lines[1]["action"], "ColdMigrate");
        assert_eq!(lines[2]["action"], "Evacuate");
    }

    #[test]
    fn test_audit_keystone_events() {
        let (mut app, dir) = make_app_with_audit();
        let project: crate::models::keystone::Project = serde_json::from_str(
            r#"{
            "id": "p1", "name": "infra", "domain_id": "default", "enabled": true
        }"#,
        )
        .unwrap();
        app.handle_event(AppEvent::ProjectCreated(project));
        app.handle_event(AppEvent::ProjectDeleted { id: "p2".into() });
        let user: crate::models::keystone::User = serde_json::from_str(
            r#"{
            "id": "u1", "name": "jay", "domain_id": "default", "enabled": true
        }"#,
        )
        .unwrap();
        app.handle_event(AppEvent::UserCreated(user));
        app.handle_event(AppEvent::UserDeleted { id: "u2".into() });
        let lines = read_audit_lines(&dir);
        assert_eq!(lines.len(), 4);
        assert_eq!(lines[0]["action"], "CreateProject");
        assert_eq!(lines[1]["action"], "DeleteProject");
        assert_eq!(lines[2]["action"], "CreateUser");
        assert_eq!(lines[3]["action"], "DeleteUser");
    }

    #[test]
    fn test_audit_image_snapshot_events() {
        let (mut app, dir) = make_app_with_audit();
        let image: crate::models::glance::Image = serde_json::from_str(
            r#"{
            "id": "i1", "name": "ubuntu-22", "status": "active", "size": 1000,
            "min_disk": 0, "min_ram": 0, "visibility": "public"
        }"#,
        )
        .unwrap();
        app.handle_event(AppEvent::ImageCreated(image));
        app.handle_event(AppEvent::ImageDeleted { id: "i2".into() });
        let lines = read_audit_lines(&dir);
        assert_eq!(lines.len(), 2);
        assert_eq!(lines[0]["action"], "CreateImage");
        assert_eq!(lines[1]["action"], "DeleteImage");
    }

    #[test]
    fn test_audit_does_not_break_toast_generation() {
        let (mut app, _dir) = make_app_with_audit();
        let server: crate::models::nova::Server = serde_json::from_str(
            r#"{
            "id": "s1", "name": "web-01", "status": "ACTIVE",
            "addresses": {}, "flavor": {"id": "f1"}, "created": "2026-01-01"
        }"#,
        )
        .unwrap();
        app.handle_event(AppEvent::ServerCreated(server));
        // Toast should still be generated
        let toasts = app.background_tracker().active_toasts();
        assert_eq!(toasts.len(), 1);
        assert!(toasts[0].message.contains("web-01"));
    }

    #[test]
    fn test_audit_compute_service_toggled() {
        let (mut app, dir) = make_app_with_audit();
        app.handle_event(AppEvent::ComputeServiceToggled {
            hostname: "compute-01".into(),
            enabled: false,
        });
        let lines = read_audit_lines(&dir);
        assert_eq!(lines.len(), 1);
        assert_eq!(lines[0]["action"], "ToggleService");
        assert_eq!(lines[0]["resource_type"], "service");
        assert_eq!(lines[0]["resource_id"], "compute-01");
        assert_eq!(lines[0]["details"]["enabled"], false);
    }

    #[test]
    fn test_navigate_resets_refresh_scheduler() {
        let (tx, mut rx) = crate::context::test_action_channel();
        let config = test_config();
        let mut app = App::new(config, tx);
        app.register_component(
            Route::Servers,
            Box::new(RefreshMock::new(Action::FetchServers)),
        );
        app.register_component(
            Route::Volumes,
            Box::new(RefreshMock::new(Action::FetchVolumes)),
        );
        app.router = Router::new(Route::Servers);

        // Advance 100 ticks (not enough to trigger)
        for _ in 0..100 {
            app.on_tick();
        }
        // Navigate — should reset counter
        app.dispatch_action(Action::Navigate(Route::Volumes));
        // 150 more ticks from reset → should trigger at tick 150
        for _ in 0..150 {
            app.on_tick();
        }
        let mut found = false;
        while let Ok(action) = rx.try_recv() {
            if matches!(action, Action::FetchVolumes) {
                found = true;
            }
        }
        assert!(found, "expected FetchVolumes after navigate + 150 ticks");
    }

    // -- Dispatcher epoch gate (BL-P2-031 Unit 2) --

    #[test]
    fn handle_versioned_event_forwards_when_epoch_matches_current() {
        use crate::context::VersionedEvent;
        let mut app = make_app();
        // Fresh app starts at epoch 0.
        let envelope = VersionedEvent::new(AppEvent::CloudSwitched("dev".into()), 0);
        let forwarded = app.handle_versioned_event(envelope);
        assert!(forwarded);
    }

    #[test]
    fn handle_versioned_event_drops_when_epoch_below_current() {
        use crate::context::VersionedEvent;
        let mut app = make_app();
        // Simulate the switcher bumping the epoch.
        app.current_epoch.bump();
        app.current_epoch.bump();
        assert_eq!(app.current_epoch.current(), 2);

        let stale = VersionedEvent::new(AppEvent::CloudSwitched("old".into()), 1);
        let forwarded = app.handle_versioned_event(stale);
        assert!(!forwarded);
    }

    #[test]
    fn handle_versioned_event_forwards_when_epoch_strictly_greater_than_current() {
        // Defensive: out-of-order arrival of a future epoch should not be
        // dropped — that path is reserved for stale events only.
        use crate::context::VersionedEvent;
        let mut app = make_app();
        let envelope = VersionedEvent::new(AppEvent::CloudSwitched("future".into()), 99);
        let forwarded = app.handle_versioned_event(envelope);
        assert!(forwarded);
    }

    // ---------- BL-P2-031 Unit 4: App.switch_context integration ----------

    fn wire_test_switcher(
        app: &mut App,
    ) -> (
        Arc<crate::port::mock_context::MockContextSession>,
        tokio::sync::mpsc::UnboundedReceiver<crate::context::VersionedEvent<AppEvent>>,
    ) {
        use crate::context::{
            CancellationRegistry, ContextHistoryStore, ContextSwitcher, ContextTargetResolver,
            SwitchStateMachine,
            resolver::{CloudDirectory, ProjectCandidate, ProjectDirectoryPort},
        };
        use crate::port::context_session::ContextSessionPort;
        use crate::port::mock_context::MockContextSession;
        use crate::port::types::{CatalogEntry, ProjectScope, Token, TokenScope};
        use async_trait::async_trait;
        use chrono::{TimeZone, Utc};

        struct FakeClouds;
        impl CloudDirectory for FakeClouds {
            fn active_cloud(&self) -> String {
                "devstack".into()
            }
            fn known_clouds(&self) -> Vec<String> {
                vec!["devstack".into()]
            }
            fn default_project(&self, _cloud: &str) -> Option<String> {
                None
            }
        }
        struct FakeDirectory;
        #[async_trait]
        impl ProjectDirectoryPort for FakeDirectory {
            async fn list_projects(
                &self,
                _cloud: &str,
            ) -> Result<Vec<ProjectCandidate>, crate::context::SwitchError> {
                Ok(vec![ProjectCandidate {
                    cloud: "devstack".into(),
                    project_id: "id-demo".into(),
                    project_name: "demo".into(),
                    domain: "default".into(),
                }])
            }
        }

        let prev_scope = TokenScope::Project {
            name: "admin".into(),
            domain: "default".into(),
        };
        let old_token = Token {
            id: "old".into(),
            expires_at: Utc.with_ymd_and_hms(2030, 1, 1, 0, 0, 0).unwrap(),
            project: ProjectScope {
                id: "id-admin".into(),
                name: "admin".into(),
                domain_id: "default".into(),
                domain_name: "default".into(),
            },
            roles: Vec::new(),
            catalog: Vec::<CatalogEntry>::new(),
        };
        let mut new_token = old_token.clone();
        new_token.id = "new".into();
        new_token.project.name = "demo".into();
        new_token.project.id = "id-demo".into();

        let session = Arc::new(MockContextSession::new(prev_scope, old_token, new_token));
        let state = Arc::new(SwitchStateMachine::new(app.current_epoch.clone()));
        let cancellation = Arc::new(CancellationRegistry::new());
        let history = Arc::new(std::sync::Mutex::new(ContextHistoryStore::new()));
        let resolver = Arc::new(ContextTargetResolver::new(
            Arc::new(FakeClouds),
            Arc::new(FakeDirectory),
            None,
            None,
        ));
        let switcher = Arc::new(ContextSwitcher::new(
            state,
            cancellation,
            resolver,
            session.clone() as Arc<dyn ContextSessionPort>,
            history,
        ));

        let (event_tx, event_rx) = tokio::sync::mpsc::unbounded_channel();
        app.wire_context_switch(switcher, event_tx);
        (session, event_rx)
    }

    #[tokio::test]
    async fn dispatch_switch_context_emits_context_changed_on_success() {
        use crate::context::ContextRequest;
        let mut app = make_app();
        let (_session, mut event_rx) = wire_test_switcher(&mut app);

        app.dispatch_action(Action::SwitchContext(ContextRequest::ByName {
            cloud: None,
            project: "demo".into(),
            domain: None,
        }));

        // Spawned task needs one yield to run to completion.
        let envelope = tokio::time::timeout(std::time::Duration::from_secs(2), event_rx.recv())
            .await
            .expect("timed out waiting for ContextChanged")
            .expect("channel closed");
        assert_eq!(envelope.epoch(), 1);
        match envelope.into_inner() {
            AppEvent::ContextChanged { target } => {
                assert_eq!(target.project_name, "demo");
                assert_eq!(target.cloud, "devstack");
            }
            other => panic!("expected ContextChanged, got {other:?}"),
        }
        // App epoch must be bumped by the switcher (shared Arc).
        assert_eq!(app.current_epoch.current(), 1);
    }

    #[tokio::test]
    async fn dispatch_switch_context_without_switcher_adds_error_toast() {
        use crate::context::ContextRequest;
        let mut app = make_app();
        // switcher intentionally not wired.
        app.dispatch_action(Action::SwitchContext(ContextRequest::ByName {
            cloud: None,
            project: "demo".into(),
            domain: None,
        }));
        let toasts = app.background_tracker().active_toasts();
        assert_eq!(toasts.len(), 1);
        assert!(toasts[0].message.contains("not available"));
    }

    #[tokio::test]
    async fn dispatch_rejects_worker_actions_while_switch_in_flight() {
        // C1 regression: if the switcher has bumped the epoch via
        // `try_begin` but not yet committed, forwarding a worker-bound
        // action lets it execute with stale auth but a fresh epoch
        // stamp — cross-context mis-execution. The dispatcher must
        // drop the action with a visible toast instead.
        let mut app = make_app();
        let (_session, _event_rx) = wire_test_switcher(&mut app);

        // Force the state machine into `Switching` without actually
        // running the async switch.
        let target = crate::context::ContextTarget {
            cloud: "devstack".into(),
            project_id: "id-demo".into(),
            project_name: "demo".into(),
            domain: "default".into(),
        };
        app.switcher
            .as_ref()
            .unwrap()
            .state()
            .try_begin(target)
            .unwrap();
        assert!(!app.switcher.as_ref().unwrap().is_idle());

        // Baseline: action_tx is empty. A worker-bound action should
        // NOT be forwarded.
        app.dispatch_action(Action::FetchServers);
        // The action channel receiver lives inside make_app()'s `_rx`,
        // which was dropped — so we can't observe "not forwarded"
        // directly. Instead, we verify the toast was surfaced.
        let toasts = app.background_tracker().active_toasts();
        assert!(
            toasts
                .iter()
                .any(|t| t.message.contains("Switch in progress")),
            "expected mid-switch toast, got: {:?}",
            toasts.iter().map(|t| &t.message).collect::<Vec<_>>()
        );
    }

    #[tokio::test]
    async fn dispatch_switch_back_with_empty_history_emits_api_error() {
        // First switch from an empty baseline leaves history empty
        // (there was no pre-switch context to remember). `switch_back`
        // then correctly reports "no previous context" as an ApiError
        // rather than replaying the just-entered context.
        use crate::context::ContextRequest;
        let mut app = make_app();
        let (_session, mut event_rx) = wire_test_switcher(&mut app);

        app.dispatch_action(Action::SwitchContext(ContextRequest::ByName {
            cloud: None,
            project: "demo".into(),
            domain: None,
        }));
        // Drain the success event so we can observe the switch_back result.
        let _ = tokio::time::timeout(std::time::Duration::from_secs(2), event_rx.recv())
            .await
            .unwrap();

        app.dispatch_action(Action::SwitchBack);
        let envelope = tokio::time::timeout(std::time::Duration::from_secs(2), event_rx.recv())
            .await
            .expect("timed out")
            .expect("channel closed");
        match envelope.into_inner() {
            AppEvent::ApiError { operation, message } => {
                assert_eq!(operation, "SwitchBack");
                assert!(message.contains("no previous"));
            }
            other => panic!("expected ApiError, got {other:?}"),
        }
    }

    // -----------------------------------------------------------------------
    // BL-P2-080 D2: directory_cache invalidation hook tests
    // -----------------------------------------------------------------------

    #[tokio::test]
    async fn handle_context_changed_invalidates_directory_cache() {
        use crate::adapter::auth::DirectoryCache;
        use crate::context::ContextTarget;
        use std::time::Duration;

        let mut app = make_app();

        // Wire a real DirectoryCache with some pre-seeded entries.
        let cache = Arc::new(DirectoryCache::new(Duration::from_secs(300)));
        // Seed entries for "devstack" and "prod"
        cache.put("devstack", "fp-1", vec![]);
        cache.put("prod", "fp-2", vec![]);
        app.wire_directory_cache(cache.clone());

        // Fire a ContextChanged for "devstack"
        let target = ContextTarget {
            cloud: "devstack".into(),
            project_id: "id-1".into(),
            project_name: "admin".into(),
            domain: "default".into(),
        };
        app.handle_event(AppEvent::ContextChanged { target });

        // "devstack" entries should be gone; "prod" should remain.
        assert!(
            cache.get("devstack", "fp-1").is_none(),
            "devstack cache entry must be invalidated on ContextChanged"
        );
        assert!(
            cache.get("prod", "fp-2").is_some(),
            "prod cache entry must not be affected"
        );
    }

    #[test]
    fn handle_context_changed_without_wired_cache_no_panic() {
        use crate::context::ContextTarget;

        let mut app = make_app();
        // directory_cache is None — must not panic
        let target = ContextTarget {
            cloud: "devstack".into(),
            project_id: "id-1".into(),
            project_name: "admin".into(),
            domain: "default".into(),
        };
        app.handle_event(AppEvent::ContextChanged { target });
        // If we reach here without panicking, the test passes.
    }
}
