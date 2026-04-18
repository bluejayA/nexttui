pub mod view_model;

use crossterm::event::{KeyCode, KeyEvent};
use ratatui::Frame;
use ratatui::layout::Rect;

use crate::action::Action;
use crate::component::Component;
use crate::context::ActionSender;
use crate::event::AppEvent;
use crate::models::neutron::Network;
use crate::module::ViewState;
use crate::port::types::{NetworkCreateParams, Subnet};
use crate::ui::form::{FormAction, FormWidget};
use crate::ui::resource_list::{ResourceList, Row};

use self::view_model::{network_columns, network_create_defs, network_detail_data, network_to_row};

pub struct NetworkModule {
    view_state: ViewState,
    networks: Vec<Network>,
    subnets: Vec<Subnet>,
    #[allow(dead_code)] // Phase 2: set to true on Action dispatch, render loading spinner
    loading: bool,
    error_message: Option<String>,
    resource_list: ResourceList,
    form: Option<FormWidget>,
    all_tenants: bool,
    action_tx: ActionSender,
}

impl NetworkModule {
    pub fn new(action_tx: ActionSender) -> Self {
        Self {
            view_state: ViewState::List,
            networks: Vec::new(),
            subnets: Vec::new(),
            loading: false,
            error_message: None,
            resource_list: ResourceList::new(network_columns(false)),
            form: None,
            all_tenants: false,
            action_tx,
        }
    }

    pub fn view_state(&self) -> &ViewState {
        &self.view_state
    }

    pub fn networks(&self) -> &[Network] {
        &self.networks
    }

    pub fn selected_index(&self) -> usize {
        self.resource_list.selected_index()
    }

    pub fn error_message(&self) -> Option<&str> {
        self.error_message.as_deref()
    }

    fn selected_network(&self) -> Option<&Network> {
        self.networks.get(self.resource_list.selected_index())
    }

    fn rows(&self) -> Vec<Row> {
        self.networks
            .iter()
            .map(|n| network_to_row(n, self.all_tenants))
            .collect()
    }

    fn open_create_form(&mut self) {
        let defs = network_create_defs();
        self.form = Some(FormWidget::new("Create Network", defs));
        self.view_state = ViewState::Create;
    }

    fn close_form(&mut self) {
        self.form = None;
        self.view_state = ViewState::List;
    }

    fn handle_list_key(&mut self, key: KeyEvent) -> Option<Action> {
        if self.resource_list.handle_nav_key(key) {
            return None;
        }

        match key.code {
            KeyCode::Enter => {
                if let Some(network) = self.selected_network() {
                    let id = network.id.clone();
                    self.subnets.clear();
                    self.view_state = ViewState::Detail(id.clone());
                    let _ = self.action_tx.send(Action::FetchSubnets { network_id: id });
                }
                None
            }
            KeyCode::Char('c') => {
                self.open_create_form();
                Some(Action::EnterFormMode)
            }
            KeyCode::Char('r') => Some(Action::FetchNetworks),
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
                let admin_state_up = values
                    .get("Admin State Up")
                    .and_then(|v| match v {
                        crate::ui::form::FormValue::Bool(b) => Some(*b),
                        _ => None,
                    })
                    .unwrap_or(true);
                let shared = values.get("Shared").and_then(|v| match v {
                    crate::ui::form::FormValue::Bool(b) => Some(*b),
                    _ => None,
                });
                let external = values.get("External").and_then(|v| match v {
                    crate::ui::form::FormValue::Bool(b) => Some(*b),
                    _ => None,
                });
                let mtu = values.get("MTU").and_then(|v| match v {
                    crate::ui::form::FormValue::Text(s) => s.parse::<u32>().ok(),
                    _ => None,
                });

                self.close_form();
                let _ = self
                    .action_tx
                    .send(Action::CreateNetwork(NetworkCreateParams {
                        name,
                        admin_state_up,
                        shared,
                        external,
                        mtu,
                        port_security_enabled: None,
                    }));
                Some(Action::ExitFormMode)
            }
            FormAction::Cancel => {
                self.close_form();
                Some(Action::ExitFormMode)
            }
            FormAction::None => None,
        }
    }
}

impl Component for NetworkModule {
    fn refresh_action(&self) -> Option<Action> {
        Some(Action::FetchNetworks)
    }
    fn is_modal(&self) -> bool {
        self.form.is_some()
    }

    fn set_all_tenants(&mut self, v: bool) {
        self.all_tenants = v;
        self.resource_list = ResourceList::new(network_columns(v));
    }

    fn handle_key(&mut self, key: KeyEvent) -> Option<Action> {
        match &self.view_state {
            ViewState::List => self.handle_list_key(key),
            ViewState::Detail(_) => self.handle_detail_key(key),
            ViewState::Create => self.handle_create_key(key),
        }
    }

    fn on_context_changed(&mut self) {
        self.networks.clear();
        self.subnets.clear();
        self.loading = true;
        self.error_message = None;
        self.resource_list.set_rows(Vec::new());
        self.view_state = ViewState::List;
    }

    fn handle_event(&mut self, event: &AppEvent) {
        match event {
            AppEvent::NetworksLoaded(networks) => {
                self.networks = networks.clone();
                self.loading = false;
                self.error_message = None;
                let rows = self.rows();
                self.resource_list.set_rows(rows);
            }
            AppEvent::SubnetsLoaded {
                network_id,
                subnets,
            } => {
                if let ViewState::Detail(ref current_id) = self.view_state
                    && current_id == network_id
                {
                    self.subnets = subnets.clone();
                }
            }
            AppEvent::NetworkCreated(_) => {
                self.close_form();
                let _ = self.action_tx.send(Action::FetchNetworks);
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
                if let Some(network) = self.networks.iter().find(|n| n.id == *id) {
                    let data = network_detail_data(network, &self.subnets);
                    let mut dv = crate::ui::detail_view::DetailView::new();
                    dv.set_data(data);
                    dv.render(frame, area);
                }
            }
            ViewState::Create => {
                if let Some(form) = &self.form {
                    form.render(frame, area);
                } else {
                    self.resource_list.render(frame, area);
                }
            }
        }
    }

    fn content_title(&self) -> Option<String> {
        match &self.view_state {
            ViewState::List => None,
            ViewState::Detail(id) => {
                let name = self
                    .networks
                    .iter()
                    .find(|r| r.id == *id)
                    .map(|r| r.name.as_str())
                    .unwrap_or("...");
                Some(format!("Network: {name}"))
            }
            ViewState::Create => Some("Create Network".into()),
        }
    }

    fn help_hint(&self) -> &str {
        match &self.view_state {
            ViewState::List => "Enter:Detail c:Create r:Refresh",
            ViewState::Detail(_) => "Esc:Back",
            ViewState::Create => "Esc:Cancel Tab:Next Enter:Submit",
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::context::{ActionReceiver, test_action_channel};

    fn key(code: KeyCode) -> KeyEvent {
        KeyEvent::from(code)
    }

    fn make_network(id: &str, name: &str, status: &str) -> Network {
        Network {
            id: id.into(),
            name: name.into(),
            status: status.into(),
            description: None,
            admin_state_up: true,
            external: false,
            shared: false,
            mtu: Some(1500),
            port_security_enabled: None,
            subnets: vec![],
            provider_network_type: None,
            provider_physical_network: None,
            provider_segmentation_id: None,
            tenant_id: None,
        }
    }

    fn setup() -> (NetworkModule, ActionReceiver) {
        let (tx, rx) = test_action_channel();
        let mut module = NetworkModule::new(tx);
        let networks = vec![
            make_network("net-1", "private", "ACTIVE"),
            make_network("net-2", "public", "ACTIVE"),
            make_network("net-3", "mgmt", "DOWN"),
        ];
        module.handle_event(&AppEvent::NetworksLoaded(networks));
        (module, rx)
    }

    #[test]
    fn test_initial_state_is_list() {
        let (tx, _rx) = test_action_channel();
        let module = NetworkModule::new(tx);
        assert_eq!(*module.view_state(), ViewState::List);
        assert!(module.networks().is_empty());
    }

    #[test]
    fn test_handle_key_j_k_navigation() {
        let (mut module, _rx) = setup();
        assert_eq!(module.selected_index(), 0);

        module.handle_key(key(KeyCode::Char('j')));
        assert_eq!(module.selected_index(), 1);

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
        let (mut module, mut rx) = setup();
        module.handle_key(key(KeyCode::Enter));
        assert_eq!(*module.view_state(), ViewState::Detail("net-1".into()));
        // Should send FetchSubnets
        let action = rx.try_recv().unwrap();
        assert!(matches!(action, Action::FetchSubnets { .. }));
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
    fn test_handle_key_r_fetches_networks() {
        let (mut module, _rx) = setup();
        let action = module.handle_key(key(KeyCode::Char('r')));
        assert!(matches!(action, Some(Action::FetchNetworks)));
    }

    #[test]
    fn test_handle_event_networks_loaded() {
        let (tx, _rx) = test_action_channel();
        let mut module = NetworkModule::new(tx);
        assert!(module.networks().is_empty());

        let networks = vec![make_network("net-1", "test", "ACTIVE")];
        module.handle_event(&AppEvent::NetworksLoaded(networks));
        assert_eq!(module.networks().len(), 1);
    }

    #[test]
    fn test_handle_event_subnets_loaded() {
        let (mut module, _rx) = setup();
        module.view_state = ViewState::Detail("net-1".into());
        module.handle_event(&AppEvent::SubnetsLoaded {
            network_id: "net-1".into(),
            subnets: vec![Subnet {
                id: "sub-1".into(),
                name: "private".into(),
                network_id: "net-1".into(),
                cidr: "10.0.0.0/24".into(),
                ip_version: 4,
                gateway_ip: Some("10.0.0.1".into()),
            }],
        });
        assert_eq!(module.subnets.len(), 1);
    }

    #[test]
    fn test_handle_event_network_created_triggers_refresh() {
        let (mut module, mut rx) = setup();
        let net = make_network("net-4", "new-net", "ACTIVE");
        module.handle_event(&AppEvent::NetworkCreated(net));
        let action = rx.try_recv().unwrap();
        assert!(matches!(action, Action::FetchNetworks));
    }

    #[test]
    fn test_handle_event_api_error() {
        let (mut module, _rx) = setup();
        module.handle_event(&AppEvent::ApiError {
            operation: "list".into(),
            message: "timeout".into(),
        });
        assert_eq!(module.error_message(), Some("list: timeout"));
    }

    // -- Form integration tests ---------------------------------------------

    #[test]
    fn test_create_form_cancel_returns_to_list() {
        let (mut module, _rx) = setup();
        module.handle_key(key(KeyCode::Char('c')));
        assert_eq!(*module.view_state(), ViewState::Create);

        module.handle_key(key(KeyCode::Esc));
        assert_eq!(*module.view_state(), ViewState::List);
        assert!(module.form.is_none());
    }

    #[test]
    fn test_create_form_has_expected_fields() {
        let (mut module, _rx) = setup();
        module.handle_key(key(KeyCode::Char('c')));
        let form = module.form.as_ref().unwrap();
        assert_eq!(form.field_count(), 5);
        assert_eq!(form.focused_field_name(), "Name");
    }

    #[test]
    fn test_help_hint_list() {
        let (module, _rx) = setup();
        assert_eq!(module.help_hint(), "Enter:Detail c:Create r:Refresh");
    }

    #[test]
    fn test_help_hint_detail() {
        let (mut module, _rx) = setup();
        module.handle_key(key(KeyCode::Enter));
        assert_eq!(module.help_hint(), "Esc:Back");
    }

    #[test]
    fn test_help_hint_create() {
        let (mut module, _rx) = setup();
        module.handle_key(key(KeyCode::Char('c')));
        assert_eq!(module.help_hint(), "Esc:Cancel Tab:Next Enter:Submit");
    }
}
