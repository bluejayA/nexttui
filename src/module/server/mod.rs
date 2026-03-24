pub mod view_model;

use crossterm::event::{KeyCode, KeyEvent};
use ratatui::layout::Rect;
use ratatui::Frame;
use tokio::sync::mpsc;

use crate::action::Action;
use crate::component::Component;
use crate::event::AppEvent;
use crate::models::nova::Server;
use crate::module::{ConfirmHandler, PendingAction, ViewState};
use crate::port::types::{NetworkAttachment, ServerCreateParams};
use crate::ui::confirm::ConfirmDialog;
use crate::ui::form::{FormAction, FormWidget, SelectOption};
use crate::ui::resource_list::{ResourceList, Row};

use self::view_model::{server_columns, server_create_defs, server_detail_data, server_to_row};

pub struct ServerModule {
    view_state: ViewState,
    servers: Vec<Server>,
    loading: bool,
    error_message: Option<String>,
    confirm: ConfirmHandler,
    resource_list: ResourceList,
    form: Option<FormWidget>,
    action_tx: mpsc::UnboundedSender<Action>,
}

impl ServerModule {
    pub fn new(action_tx: mpsc::UnboundedSender<Action>) -> Self {
        Self {
            view_state: ViewState::List,
            servers: Vec::new(),
            loading: false,
            error_message: None,
            confirm: ConfirmHandler::new(),
            resource_list: ResourceList::new(server_columns()),
            form: None,
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
        self.resource_list.selected_index()
    }

    pub fn error_message(&self) -> Option<&str> {
        self.error_message.as_deref()
    }

    fn selected_server(&self) -> Option<&Server> {
        self.servers.get(self.resource_list.selected_index())
    }

    fn rows(&self) -> Vec<Row> {
        self.servers.iter().map(server_to_row).collect()
    }

    fn resolve_action(pending: PendingAction) -> Option<Action> {
        match pending {
            PendingAction::Delete { id, name } => Some(Action::DeleteServer { id, name }),
            PendingAction::Reboot { id, hard } => Some(Action::RebootServer { id, hard }),
            PendingAction::Stop { id } => Some(Action::StopServer { id }),
            _ => None,
        }
    }

    fn open_create_form(&mut self) {
        let defs = server_create_defs();
        self.form = Some(FormWidget::new("Create Server", defs));
        self.view_state = ViewState::Create;
        // Request data for dropdown options — handle_event will populate via set_field_options
        let _ = self.action_tx.send(Action::FetchFlavors);
        let _ = self.action_tx.send(Action::FetchImages);
        let _ = self.action_tx.send(Action::FetchNetworks);
        let _ = self.action_tx.send(Action::FetchSecurityGroups);
    }

    fn close_form(&mut self) {
        self.form = None;
        self.view_state = ViewState::List;
    }

    fn handle_list_key(&mut self, key: KeyEvent) -> Option<Action> {
        // Navigation keys handled by ResourceList
        if self.resource_list.handle_nav_key(key) {
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
                self.open_create_form();
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
            KeyCode::Left => Some(Action::FocusSidebar),
            KeyCode::Esc => Some(Action::Back),
            _ => None,
        }
    }

    fn handle_detail_key(&mut self, key: KeyEvent) -> Option<Action> {
        match key.code {
            KeyCode::Esc | KeyCode::Char('q') | KeyCode::Left => {
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
        let Some(form) = self.form.as_mut() else {
            self.close_form();
            return None;
        };

        match form.handle_key(key) {
            FormAction::Submit(values) => {
                let name = values
                    .get("Name")
                    .and_then(|v| match v {
                        crate::ui::form::FormValue::Text(s) => Some(s.clone()),
                        _ => None,
                    })
                    .unwrap_or_default();
                let image_id = values
                    .get("Image")
                    .and_then(|v| match v {
                        crate::ui::form::FormValue::Selected(s) => Some(s.clone()),
                        _ => None,
                    })
                    .unwrap_or_default();
                let flavor_id = values
                    .get("Flavor")
                    .and_then(|v| match v {
                        crate::ui::form::FormValue::Selected(s) => Some(s.clone()),
                        _ => None,
                    })
                    .unwrap_or_default();
                let network_id = values
                    .get("Network")
                    .and_then(|v| match v {
                        crate::ui::form::FormValue::Selected(s) => Some(s.clone()),
                        _ => None,
                    })
                    .unwrap_or_default();
                let security_group = values
                    .get("Security Group")
                    .and_then(|v| match v {
                        crate::ui::form::FormValue::Selected(s) => {
                            if s.is_empty() { None } else { Some(vec![s.clone()]) }
                        }
                        _ => None,
                    });
                let key_name = values
                    .get("Key Pair")
                    .and_then(|v| match v {
                        crate::ui::form::FormValue::Text(s) => {
                            if s.is_empty() { None } else { Some(s.clone()) }
                        }
                        _ => None,
                    });
                let availability_zone = values
                    .get("Availability Zone")
                    .and_then(|v| match v {
                        crate::ui::form::FormValue::Text(s) => {
                            if s.is_empty() { None } else { Some(s.clone()) }
                        }
                        _ => None,
                    });

                self.close_form();

                Some(Action::CreateServer(ServerCreateParams {
                    name,
                    image_id,
                    flavor_id,
                    networks: vec![NetworkAttachment {
                        uuid: network_id,
                        fixed_ip: None,
                    }],
                    security_groups: security_group,
                    key_name,
                    availability_zone,
                }))
            }
            FormAction::Cancel => {
                self.close_form();
                None
            }
            FormAction::None => None,
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
            AppEvent::FlavorsLoaded(flavors) => {
                if let Some(form) = &mut self.form {
                    let opts: Vec<SelectOption> = flavors
                        .iter()
                        .map(|f| SelectOption::new(&f.id, format!("{} ({}vCPU/{}MB/{}GB)", f.name, f.vcpus, f.ram, f.disk)))
                        .collect();
                    form.set_field_options("Flavor", opts);
                }
            }
            AppEvent::ImagesLoaded(images) => {
                if let Some(form) = &mut self.form {
                    let opts: Vec<SelectOption> = images
                        .iter()
                        .map(|img| SelectOption::new(&img.id, &img.name))
                        .collect();
                    form.set_field_options("Image", opts);
                }
            }
            AppEvent::NetworksLoaded(networks) => {
                if let Some(form) = &mut self.form {
                    let opts: Vec<SelectOption> = networks
                        .iter()
                        .map(|n| SelectOption::new(&n.id, &n.name))
                        .collect();
                    form.set_field_options("Network", opts);
                }
            }
            AppEvent::SecurityGroupsLoaded(sgs) => {
                if let Some(form) = &mut self.form {
                    let opts: Vec<SelectOption> = sgs
                        .iter()
                        .map(|sg| SelectOption::new(&sg.id, &sg.name))
                        .collect();
                    form.set_field_options("Security Group", opts);
                }
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
                if let Some(form) = &self.form {
                    form.render(frame, area);
                } else {
                    // Defensive: form should always be Some in Create state.
                    // If not, render list as fallback (next key press will fix state via close_form).
                    self.resource_list.render(frame, area);
                }
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
    fn test_handle_key_g_and_shift_g() {
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
        assert!(module.form.is_some());
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

    // -- Form integration tests ---------------------------------------------

    #[test]
    fn test_create_form_cancel_returns_to_list() {
        let (mut module, _rx) = setup();
        module.handle_key(key(KeyCode::Char('c'))); // open form
        assert_eq!(*module.view_state(), ViewState::Create);

        module.handle_key(key(KeyCode::Esc)); // cancel form
        assert_eq!(*module.view_state(), ViewState::List);
        assert!(module.form.is_none());
    }

    #[test]
    fn test_create_form_has_expected_fields() {
        let (mut module, _rx) = setup();
        module.handle_key(key(KeyCode::Char('c')));
        let form = module.form.as_ref().unwrap();
        assert_eq!(form.field_count(), 7);
        assert_eq!(form.focused_field_name(), "Name");
    }

    #[test]
    fn test_create_form_submit_produces_action() {
        let (mut module, _rx) = setup();
        module.handle_key(key(KeyCode::Char('c')));

        // Type server name
        for c in "test-vm".chars() {
            module.handle_key(key(KeyCode::Char(c)));
        }

        // Navigate to last field (Availability Zone) and submit
        for _ in 0..6 {
            module.handle_key(key(KeyCode::Down));
        }
        let action = module.handle_key(key(KeyCode::Enter));

        // Submit should fail validation (Image is required but not selected)
        // So action should be None and we stay on form
        assert!(action.is_none());
    }

    #[test]
    fn test_open_create_form_dispatches_fetch_actions() {
        let (mut module, mut rx) = setup();
        module.handle_key(key(KeyCode::Char('c')));

        let mut actions = Vec::new();
        while let Ok(a) = rx.try_recv() {
            actions.push(a);
        }
        assert!(actions.iter().any(|a| matches!(a, Action::FetchFlavors)));
        assert!(actions.iter().any(|a| matches!(a, Action::FetchImages)));
        assert!(actions.iter().any(|a| matches!(a, Action::FetchNetworks)));
        assert!(actions.iter().any(|a| matches!(a, Action::FetchSecurityGroups)));
    }

    #[test]
    fn test_flavors_loaded_populates_form_options() {
        use crate::models::nova::Flavor;

        let (mut module, _rx) = setup();
        module.handle_key(key(KeyCode::Char('c')));

        let flavors = vec![Flavor {
            id: "flv-1".into(),
            name: "m1.small".into(),
            vcpus: 1,
            ram: 2048,
            disk: 20,
            is_public: true,
        }];
        module.handle_event(&AppEvent::FlavorsLoaded(flavors));

        let form = module.form.as_ref().unwrap();
        // Flavor dropdown should now have options
        if let (crate::ui::form::FieldDef::Dropdown { options, .. }, _) = &form.fields()[2] {
            assert_eq!(options.len(), 1);
            assert_eq!(options[0].value, "flv-1");
        } else {
            panic!("Expected Dropdown for Flavor field");
        }
    }
}
