use std::collections::HashMap;
use std::sync::Arc;

use crossterm::event::{KeyCode, KeyEvent};
use ratatui::Frame;
use tokio::sync::mpsc;

use crate::action::Action;
use crate::background::BackgroundTracker;
use crate::component::{Component, InputMode};
use crate::config::Config;
use crate::event::AppEvent;
use crate::models::common::Route;
use crate::router::Router;

pub struct App {
    pub should_quit: bool,
    pub input_mode: InputMode,
    pub sidebar_visible: bool,

    router: Router,
    components: HashMap<Route, Box<dyn Component>>,
    background_tracker: BackgroundTracker,
    action_tx: mpsc::UnboundedSender<Action>,

    config: Arc<Config>,
}

impl App {
    pub fn new(config: Config, action_tx: mpsc::UnboundedSender<Action>) -> Self {
        Self {
            should_quit: false,
            input_mode: InputMode::Normal,
            sidebar_visible: true,
            router: Router::new(Route::Servers),
            components: HashMap::new(),
            background_tracker: BackgroundTracker::new(),
            action_tx,
            config: Arc::new(config),
        }
    }

    /// Register a domain module component for a given route.
    pub fn register_component(&mut self, route: Route, component: Box<dyn Component>) {
        self.components.insert(route, component);
    }

    /// Handle key input. Returns true if a re-render is needed.
    pub fn handle_key(&mut self, key: KeyEvent) -> bool {
        let no_modifiers = key.modifiers.is_empty();

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
                    self.sidebar_visible = !self.sidebar_visible;
                    return true;
                }
                KeyCode::Char('q') => {
                    self.should_quit = true;
                    return true;
                }
                KeyCode::Esc => {
                    self.router.back();
                    return true;
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

        // TODO(Unit 6): Form mode Esc → show cancel confirmation dialog (BR-02)
        // Currently Form mode delegates all keys to FormWidget which is not yet implemented.

        // Delegate to active component in Normal mode
        if self.input_mode == InputMode::Normal
            && let Some(component) = self.components.get_mut(&self.router.current())
        {
            if let Some(action) = component.handle_key(key) {
                self.dispatch_action(action);
            }
            return true;
        }

        true
    }

    /// Handle action — intercept navigation actions, forward the rest to action_tx.
    fn dispatch_action(&mut self, action: Action) {
        match action {
            Action::Navigate(route) => {
                self.router.navigate(route);
            }
            Action::Back => {
                self.router.back();
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
        // Skeleton: render active component in full area
        let area = frame.area();
        if let Some(component) = self.components.get(&self.router.current()) {
            component.render(frame, area);
        }
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
    fn test_app_global_key_tab() {
        let mut app = make_app();
        assert!(app.sidebar_visible);
        app.handle_key(make_key(KeyCode::Tab));
        assert!(!app.sidebar_visible);
        app.handle_key(make_key(KeyCode::Tab));
        assert!(app.sidebar_visible);
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
