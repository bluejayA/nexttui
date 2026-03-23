pub mod view_model;

use crossterm::event::{KeyCode, KeyEvent};
use ratatui::layout::Rect;
use ratatui::style::{Color, Style};
use ratatui::text::Line;
use ratatui::widgets::Paragraph;
use ratatui::Frame;
use tokio::sync::mpsc;

use crate::action::Action;
use crate::component::Component;
use crate::event::AppEvent;
use crate::models::nova::Server;
use crate::module::{ConfirmHandler, ListNav, PendingAction, ViewState};
use crate::ui::confirm::ConfirmDialog;
use crate::ui::resource_list::{ResourceList, Row};

use self::view_model::{server_columns, server_detail_data, server_to_row};

pub struct ServerModule {
    view_state: ViewState,
    servers: Vec<Server>,
    nav: ListNav,
    loading: bool,
    error_message: Option<String>,
    confirm: ConfirmHandler,
    resource_list: ResourceList,
    action_tx: mpsc::UnboundedSender<Action>,
}

impl ServerModule {
    pub fn new(action_tx: mpsc::UnboundedSender<Action>) -> Self {
        Self {
            view_state: ViewState::List,
            servers: Vec::new(),
            nav: ListNav::new(),
            loading: false,
            error_message: None,
            confirm: ConfirmHandler::new(),
            resource_list: ResourceList::new(server_columns()),
            action_tx,
        }
    }

    pub fn view_state(&self) -> &ViewState {
        &self.view_state
    }

    pub fn servers(&self) -> &[Server] {
        &self.servers
    }

    pub fn selected_index(&self) -> usize {
        self.nav.selected_index
    }

    pub fn error_message(&self) -> Option<&str> {
        self.error_message.as_deref()
    }

    fn selected_server(&self) -> Option<&Server> {
        self.servers.get(self.nav.selected_index)
    }

    fn rows(&self) -> Vec<Row> {
        self.servers.iter().map(server_to_row).collect()
    }

    fn resolve_action(pending: PendingAction) -> Option<Action> {
        match pending {
            PendingAction::Delete { id, name } => Some(Action::DeleteServer { id, name }),
            PendingAction::Reboot { id, hard } => Some(Action::RebootServer { id, hard }),
            PendingAction::Stop { id } => Some(Action::StopServer { id }),
            PendingAction::Submit => None,
        }
    }

    fn handle_list_key(&mut self, key: KeyEvent) -> Option<Action> {
        // Navigation keys handled by ListNav
        if self.nav.handle_key(key) {
            return None;
        }

        match key.code {
            KeyCode::Enter => {
                if let Some(server) = self.selected_server() {
                    let id = server.id.clone();
                    self.view_state = ViewState::Detail(id);
                }
                None
            }
            KeyCode::Char('c') => {
                self.view_state = ViewState::Create;
                None
            }
            KeyCode::Char('d') => {
                if let Some(server) = self.selected_server() {
                    let name = server.name.clone();
                    let id = server.id.clone();
                    self.confirm.open(
                        ConfirmDialog::type_to_confirm(
                            format!("Delete server '{name}'?"),
                            name.clone(),
                        ),
                        PendingAction::Delete { id, name },
                    );
                }
                None
            }
            KeyCode::Char('r') => Some(Action::FetchServers),
            KeyCode::Esc => Some(Action::Back),
            _ => None,
        }
    }

    fn handle_detail_key(&mut self, key: KeyEvent) -> Option<Action> {
        match key.code {
            KeyCode::Esc | KeyCode::Char('q') => {
                self.view_state = ViewState::List;
                None
            }
            KeyCode::Char('R') => {
                if let ViewState::Detail(ref id) = self.view_state {
                    let id = id.clone();
                    self.confirm.open(
                        ConfirmDialog::yes_no("Hard reboot this server?"),
                        PendingAction::Reboot { id, hard: true },
                    );
                }
                None
            }
            KeyCode::Char('S') => {
                if let ViewState::Detail(ref id) = self.view_state {
                    return Some(Action::StartServer { id: id.clone() });
                }
                None
            }
            KeyCode::Char('X') => {
                if let ViewState::Detail(ref id) = self.view_state {
                    let id = id.clone();
                    self.confirm.open(
                        ConfirmDialog::yes_no("Stop this server?"),
                        PendingAction::Stop { id },
                    );
                }
                None
            }
            _ => None,
        }
    }

    fn handle_create_key(&mut self, key: KeyEvent) -> Option<Action> {
        match key.code {
            KeyCode::Esc => {
                self.view_state = ViewState::List;
                None
            }
            _ => None,
        }
    }
}

impl Component for ServerModule {
    fn handle_key(&mut self, key: KeyEvent) -> Option<Action> {
        // ConfirmHandler takes priority
        if let Some(result) = self.confirm.handle_key(key, Self::resolve_action) {
            return result;
        }

        match &self.view_state {
            ViewState::List => self.handle_list_key(key),
            ViewState::Detail(_) => self.handle_detail_key(key),
            ViewState::Create => self.handle_create_key(key),
        }
    }

    fn handle_event(&mut self, event: &AppEvent) {
        match event {
            AppEvent::ServersLoaded(servers) => {
                self.servers = servers.clone();
                self.loading = false;
                self.error_message = None;
                self.nav.set_count(self.servers.len());
                let rows = self.rows();
                self.resource_list.set_rows(rows);
            }
            AppEvent::ServerDeleted { .. }
            | AppEvent::ServerRebooted { .. }
            | AppEvent::ServerStarted { .. }
            | AppEvent::ServerStopped { .. }
            | AppEvent::ServerCreated(_) => {
                let _ = self.action_tx.send(Action::FetchServers);
            }
            AppEvent::ApiError {
                operation, message, ..
            } => {
                self.error_message = Some(format!("{operation}: {message}"));
                self.loading = false;
            }
            _ => {}
        }
    }

    fn render(&self, frame: &mut Frame, area: Rect) {
        match &self.view_state {
            ViewState::List => {
                self.resource_list.render(frame, area);
            }
            ViewState::Detail(id) => {
                if let Some(server) = self.servers.iter().find(|s| s.id == *id) {
                    let data = server_detail_data(server);
                    let mut dv = crate::ui::detail_view::DetailView::new();
                    dv.set_data(data);
                    dv.render(frame, area);
                }
            }
            ViewState::Create => {
                let text = Paragraph::new(vec![
                    Line::raw(""),
                    Line::raw("  Server Create Form (Tab/Enter to submit, Esc to cancel)"),
                    Line::raw("  [Form integration pending]"),
                ])
                .style(Style::default().fg(Color::DarkGray));
                frame.render_widget(text, area);
            }
        }

        // Overlay: ConfirmDialog
        self.confirm.render(frame, area);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::nova::{FlavorRef, ImageRef};
    use std::collections::HashMap;

    fn key(code: KeyCode) -> KeyEvent {
        KeyEvent::from(code)
    }

    fn make_test_server(id: &str, name: &str, status: &str) -> Server {
        Server {
            id: id.into(),
            name: name.into(),
            status: status.into(),
            addresses: HashMap::new(),
            flavor: FlavorRef {
                id: "flv-1".into(),
                original_name: Some("m1.small".into()),
                vcpus: Some(2),
                ram: Some(4096),
                disk: Some(40),
            },
            image: Some(ImageRef { id: "img-1".into() }),
            key_name: None,
            availability_zone: None,
            created: "2026-01-01T00:00:00Z".into(),
            updated: None,
            tenant_id: None,
            host_id: None,
            host: None,
        }
    }

    fn setup() -> (ServerModule, mpsc::UnboundedReceiver<Action>) {
        let (tx, rx) = mpsc::unbounded_channel();
        let mut module = ServerModule::new(tx);
        let servers = vec![
            make_test_server("s1", "web-01", "ACTIVE"),
            make_test_server("s2", "web-02", "SHUTOFF"),
            make_test_server("s3", "db-01", "ERROR"),
        ];
        module.handle_event(&AppEvent::ServersLoaded(servers));
        (module, rx)
    }

    #[test]
    fn test_initial_state_is_list() {
        let (tx, _rx) = mpsc::unbounded_channel();
        let module = ServerModule::new(tx);
        assert_eq!(*module.view_state(), ViewState::List);
        assert!(module.servers().is_empty());
    }

    #[test]
    fn test_handle_key_j_k_navigation() {
        let (mut module, _rx) = setup();
        assert_eq!(module.selected_index(), 0);

        module.handle_key(key(KeyCode::Char('j')));
        assert_eq!(module.selected_index(), 1);

        module.handle_key(key(KeyCode::Char('j')));
        assert_eq!(module.selected_index(), 2);

        // Can't go past end
        module.handle_key(key(KeyCode::Char('j')));
        assert_eq!(module.selected_index(), 2);

        module.handle_key(key(KeyCode::Char('k')));
        assert_eq!(module.selected_index(), 1);

        module.handle_key(key(KeyCode::Char('k')));
        assert_eq!(module.selected_index(), 0);

        module.handle_key(key(KeyCode::Char('k')));
        assert_eq!(module.selected_index(), 0);
    }

    #[test]
    fn test_handle_key_g_G() {
        let (mut module, _rx) = setup();

        module.handle_key(key(KeyCode::Char('G')));
        assert_eq!(module.selected_index(), 2);

        module.handle_key(key(KeyCode::Char('g')));
        assert_eq!(module.selected_index(), 0);
    }

    #[test]
    fn test_handle_key_enter_to_detail() {
        let (mut module, _rx) = setup();
        module.handle_key(key(KeyCode::Enter));
        assert_eq!(*module.view_state(), ViewState::Detail("s1".into()));
    }

    #[test]
    fn test_handle_key_esc_detail_to_list() {
        let (mut module, _rx) = setup();
        module.handle_key(key(KeyCode::Enter));
        assert!(matches!(*module.view_state(), ViewState::Detail(_)));

        module.handle_key(key(KeyCode::Esc));
        assert_eq!(*module.view_state(), ViewState::List);
    }

    #[test]
    fn test_handle_key_c_opens_create() {
        let (mut module, _rx) = setup();
        module.handle_key(key(KeyCode::Char('c')));
        assert_eq!(*module.view_state(), ViewState::Create);
    }

    #[test]
    fn test_handle_key_d_opens_confirm() {
        let (mut module, _rx) = setup();
        assert!(!module.confirm.is_active());
        module.handle_key(key(KeyCode::Char('d')));
        assert!(module.confirm.is_active());
    }

    #[test]
    fn test_confirm_delete_executes_action() {
        let (mut module, _rx) = setup();
        module.handle_key(key(KeyCode::Char('d')));

        for c in "web-01".chars() {
            module.handle_key(key(KeyCode::Char(c)));
        }
        let action = module.handle_key(key(KeyCode::Enter));
        assert!(matches!(action, Some(Action::DeleteServer { .. })));
        assert!(!module.confirm.is_active());
    }

    #[test]
    fn test_confirm_cancel() {
        let (mut module, _rx) = setup();
        module.handle_key(key(KeyCode::Char('d')));
        module.handle_key(key(KeyCode::Esc));
        assert!(!module.confirm.is_active());
    }

    #[test]
    fn test_handle_event_servers_loaded() {
        let (tx, _rx) = mpsc::unbounded_channel();
        let mut module = ServerModule::new(tx);
        assert!(module.servers().is_empty());

        let servers = vec![make_test_server("s1", "test", "ACTIVE")];
        module.handle_event(&AppEvent::ServersLoaded(servers));
        assert_eq!(module.servers().len(), 1);
        assert!(!module.loading);
    }

    #[test]
    fn test_handle_event_server_deleted_triggers_refresh() {
        let (mut module, mut rx) = setup();
        module.handle_event(&AppEvent::ServerDeleted {
            id: "s1".into(),
            name: "web-01".into(),
        });
        let action = rx.try_recv().unwrap();
        assert!(matches!(action, Action::FetchServers));
    }

    #[test]
    fn test_handle_event_api_error() {
        let (mut module, _rx) = setup();
        module.handle_event(&AppEvent::ApiError {
            operation: "delete".into(),
            message: "not found".into(),
        });
        assert_eq!(module.error_message(), Some("delete: not found"));
    }

    #[test]
    fn test_handle_key_r_fetches_servers() {
        let (mut module, _rx) = setup();
        let action = module.handle_key(key(KeyCode::Char('r')));
        assert!(matches!(action, Some(Action::FetchServers)));
    }

    #[test]
    fn test_detail_hard_reboot_confirm() {
        let (mut module, _rx) = setup();
        module.handle_key(key(KeyCode::Enter));
        module.handle_key(key(KeyCode::Char('R')));
        assert!(module.confirm.is_active());
    }

    #[test]
    fn test_detail_start_server() {
        let (mut module, _rx) = setup();
        module.handle_key(key(KeyCode::Enter));
        let action = module.handle_key(key(KeyCode::Char('S')));
        assert!(matches!(action, Some(Action::StartServer { .. })));
    }

    #[test]
    fn test_detail_stop_server_confirm() {
        let (mut module, _rx) = setup();
        module.handle_key(key(KeyCode::Enter));
        module.handle_key(key(KeyCode::Char('X')));
        assert!(module.confirm.is_active());
    }
}
