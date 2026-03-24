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
use crate::models::neutron::Network;
use crate::module::{ListNav, ViewState};
use crate::port::types::Subnet;
use crate::ui::resource_list::{ResourceList, Row};

use self::view_model::{network_columns, network_detail_data, network_to_row};

pub struct NetworkModule {
    view_state: ViewState,
    networks: Vec<Network>,
    subnets: Vec<Subnet>,
    nav: ListNav,
    #[allow(dead_code)] // Phase 2: set to true on Action dispatch, render loading spinner
    loading: bool,
    error_message: Option<String>,
    resource_list: ResourceList,
    action_tx: mpsc::UnboundedSender<Action>,
}

impl NetworkModule {
    pub fn new(action_tx: mpsc::UnboundedSender<Action>) -> Self {
        Self {
            view_state: ViewState::List,
            networks: Vec::new(),
            subnets: Vec::new(),
            nav: ListNav::new(),
            loading: false,
            error_message: None,
            resource_list: ResourceList::new(network_columns()),
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
        self.nav.selected_index
    }

    pub fn error_message(&self) -> Option<&str> {
        self.error_message.as_deref()
    }

    fn selected_network(&self) -> Option<&Network> {
        self.networks.get(self.nav.selected_index)
    }

    fn rows(&self) -> Vec<Row> {
        self.networks.iter().map(network_to_row).collect()
    }

    fn handle_list_key(&mut self, key: KeyEvent) -> Option<Action> {
        if self.nav.handle_key(key) {
            return None;
        }

        match key.code {
            KeyCode::Enter => {
                if let Some(network) = self.selected_network() {
                    let id = network.id.clone();
                    self.subnets.clear();
                    self.view_state = ViewState::Detail(id.clone());
                    let _ = self.action_tx.send(Action::FetchSubnets {
                        network_id: id,
                    });
                }
                None
            }
            KeyCode::Char('c') => {
                self.view_state = ViewState::Create;
                None
            }
            KeyCode::Char('r') => Some(Action::FetchNetworks),
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

impl Component for NetworkModule {
    fn handle_key(&mut self, key: KeyEvent) -> Option<Action> {
        match &self.view_state {
            ViewState::List => self.handle_list_key(key),
            ViewState::Detail(_) => self.handle_detail_key(key),
            ViewState::Create => self.handle_create_key(key),
        }
    }

    fn handle_event(&mut self, event: &AppEvent) {
        match event {
            AppEvent::NetworksLoaded(networks) => {
                self.networks = networks.clone();
                self.loading = false;
                self.error_message = None;
                self.nav.set_count(self.networks.len());
                let rows = self.rows();
                self.resource_list.set_rows(rows);
            }
            AppEvent::SubnetsLoaded { network_id, subnets } => {
                if let ViewState::Detail(ref current_id) = self.view_state {
                    if current_id == network_id {
                        self.subnets = subnets.clone();
                    }
                }
            }
            AppEvent::NetworkCreated(_) => {
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
                let text = Paragraph::new(vec![
                    Line::raw(""),
                    Line::raw("  Network Create Form (Tab/Enter to submit, Esc to cancel)"),
                    Line::raw("  [Form integration pending]"),
                ])
                .style(Style::default().fg(Color::DarkGray));
                frame.render_widget(text, area);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

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
        }
    }

    fn setup() -> (NetworkModule, mpsc::UnboundedReceiver<Action>) {
        let (tx, rx) = mpsc::unbounded_channel();
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
        let (tx, _rx) = mpsc::unbounded_channel();
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
    }

    #[test]
    fn test_handle_key_r_fetches_networks() {
        let (mut module, _rx) = setup();
        let action = module.handle_key(key(KeyCode::Char('r')));
        assert!(matches!(action, Some(Action::FetchNetworks)));
    }

    #[test]
    fn test_handle_event_networks_loaded() {
        let (tx, _rx) = mpsc::unbounded_channel();
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
}
