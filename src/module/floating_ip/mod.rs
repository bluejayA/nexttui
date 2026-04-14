pub mod view_model;

use std::collections::HashSet;

use crossterm::event::{KeyCode, KeyEvent};
use ratatui::layout::Rect;
use ratatui::Frame;

use crate::action::Action;
use crate::context::ActionSender;
use crate::component::Component;
use crate::event::AppEvent;
use crate::models::neutron::{FloatingIp, Network, Port};
use crate::models::nova::Server;
use crate::module::{ConfirmHandler, PendingAction, ViewState};
use crate::ui::confirm::ConfirmDialog;
use crate::ui::form::{FormAction, FormWidget, SelectOption};
use crate::ui::resource_list::{ResourceList, Row};
use crate::ui::select_popup::{ItemHint, SelectItem, SelectPopup, SelectResult};

use self::view_model::{fip_columns, fip_create_defs, fip_to_row, FipRowContext};

pub struct FloatingIpModule {
    view_state: ViewState,
    floating_ips: Vec<FloatingIp>,
    #[allow(dead_code)] // Phase 2: set to true on Action dispatch, render loading spinner
    loading: bool,
    error_message: Option<String>,
    confirm: ConfirmHandler,
    resource_list: ResourceList,
    form: Option<FormWidget>,
    all_tenants: bool,
    is_admin: bool,
    cached_ext_network_opts: Vec<SelectOption>,
    cached_servers: Vec<Server>,
    cached_ports: Vec<Port>,
    cached_networks: Vec<Network>,
    select_popup: Option<SelectPopup>,
    pending_fip_id: Option<String>,
    pending_ports_server_id: Option<String>,
    loading_ports: bool,
    keymap_hints_shown: HashSet<char>,
    action_tx: ActionSender,
}

impl FloatingIpModule {
    pub fn new(action_tx: ActionSender) -> Self {
        Self {
            view_state: ViewState::List,
            floating_ips: Vec::new(),
            loading: false,
            error_message: None,
            confirm: ConfirmHandler::new(),
            resource_list: ResourceList::new(fip_columns(false)),
            form: None,
            all_tenants: false,
            is_admin: false,
            cached_ext_network_opts: Vec::new(),
            cached_servers: Vec::new(),
            cached_ports: Vec::new(),
            cached_networks: Vec::new(),
            select_popup: None,
            pending_fip_id: None,
            pending_ports_server_id: None,
            loading_ports: false,
            keymap_hints_shown: HashSet::new(),
            action_tx,
        }
    }

    pub fn view_state(&self) -> &ViewState {
        &self.view_state
    }

    pub fn floating_ips(&self) -> &[FloatingIp] {
        &self.floating_ips
    }

    pub fn selected_index(&self) -> usize {
        self.resource_list.selected_index()
    }

    pub fn error_message(&self) -> Option<&str> {
        self.error_message.as_deref()
    }

    fn selected_fip(&self) -> Option<&FloatingIp> {
        self.floating_ips.get(self.resource_list.selected_index())
    }

    fn rows(&self) -> Vec<Row> {
        let ctx = FipRowContext {
            show_tenant: self.all_tenants,
            cached_servers: &self.cached_servers,
            cached_ports: &self.cached_ports,
        };
        self.floating_ips.iter().map(|f| fip_to_row(f, &ctx)).collect()
    }

    fn resolve_action(pending: PendingAction) -> Option<Action> {
        match pending {
            PendingAction::DeleteFloatingIp { id, .. } => Some(Action::DeleteFloatingIp { id }),
            PendingAction::AssociateFloatingIp { fip_id, port_id } => {
                Some(Action::AssociateFloatingIp { fip_id, port_id })
            }
            PendingAction::DisassociateFloatingIp { fip_id } => {
                Some(Action::DisassociateFloatingIp { fip_id })
            }
            _ => None,
        }
    }

    fn build_server_items(&self) -> Vec<SelectItem> {
        self.cached_servers
            .iter()
            .filter(|s| s.status == "ACTIVE" || s.status == "PAUSED" || s.status == "SHUTOFF")
            .map(|s| {
                let hint = if s.status == "SHUTOFF" {
                    ItemHint::Warning("SHUTOFF".into())
                } else {
                    ItemHint::Normal
                };
                SelectItem {
                    id: s.id.clone(),
                    label: format!("{} ({})", s.name, s.status),
                    hint,
                }
            })
            .collect()
    }

    fn build_port_items(&self) -> Vec<SelectItem> {
        self.cached_ports
            .iter()
            .map(|p| SelectItem {
                id: p.id.clone(),
                label: p.display_label(&self.cached_networks),
                hint: ItemHint::Normal,
            })
            .collect()
    }

    fn fip_associate_detail_lines(fip: &FloatingIp, server_name: &str, port_label: &str) -> Vec<String> {
        vec![
            format!("  Floating IP: {}", fip.floating_ip_address),
            format!("  Server: {server_name}"),
            format!("  Port: {port_label}"),
        ]
    }

    fn fip_disassociate_detail_lines(fip: &FloatingIp) -> Vec<String> {
        let fixed = fip.fixed_ip_address.as_deref().unwrap_or("-");
        let port = fip.port_id.as_deref().unwrap_or("-");
        vec![
            format!("  Floating IP: {}", fip.floating_ip_address),
            format!("  Fixed IP: {fixed}"),
            format!("  Port: {port}"),
        ]
    }

    fn handle_ports_loaded(&mut self, ports: Vec<Port>) {
        self.cached_ports = ports;
        self.loading_ports = false;

        let Some(fip_id) = self.pending_fip_id.clone() else { return };

        if self.cached_ports.is_empty() {
            let _ = self.action_tx.send(Action::ShowToast {
                message: "No ports found for this server".into(),
            });
            self.pending_fip_id = None;
            return;
        }

        if self.cached_ports.len() == 1 {
            let port = &self.cached_ports[0];
            let port_label = port.display_label(&self.cached_networks);
            let fip = self.floating_ips.iter().find(|f| f.id == fip_id);
            let Some(fip) = fip else {
                self.pending_fip_id = None;
                return;
            };
            // Find server name from cached_servers by port's device_id
            let server_name = port.device_id.as_deref()
                .and_then(|did| self.cached_servers.iter().find(|s| s.id == did))
                .map(|s| s.name.as_str())
                .unwrap_or("unknown");
            let details = Self::fip_associate_detail_lines(fip, server_name, &port_label);
            self.confirm.open(
                ConfirmDialog::yes_no_with_details(
                    format!("Associate {} to port {}?", fip.floating_ip_address, port_label),
                    details,
                ),
                PendingAction::AssociateFloatingIp {
                    fip_id,
                    port_id: port.id.clone(),
                },
            );
            self.pending_fip_id = None;
            return;
        }

        // Multiple ports: show SelectPopup
        let items = self.build_port_items();
        self.select_popup = Some(SelectPopup::new("Select Port", items));
    }

    fn handle_port_selected(&mut self, port_id: String) {
        let Some(fip_id) = self.pending_fip_id.take() else { return };
        let port = self.cached_ports.iter().find(|p| p.id == port_id);
        let fip = self.floating_ips.iter().find(|f| f.id == fip_id);
        let (Some(port), Some(fip)) = (port, fip) else { return };

        let port_label = port.display_label(&self.cached_networks);
        let server_name = port.device_id.as_deref()
            .and_then(|did| self.cached_servers.iter().find(|s| s.id == did))
            .map(|s| s.name.as_str())
            .unwrap_or("unknown");
        let details = Self::fip_associate_detail_lines(fip, server_name, &port_label);
        self.confirm.open(
            ConfirmDialog::yes_no_with_details(
                format!("Associate {} to port {}?", fip.floating_ip_address, port_label),
                details,
            ),
            PendingAction::AssociateFloatingIp {
                fip_id: fip.id.clone(),
                port_id,
            },
        );
    }

    fn open_create_form(&mut self) {
        let defs = fip_create_defs();
        let mut form = FormWidget::new("Allocate Floating IP", defs);
        if !self.cached_ext_network_opts.is_empty() {
            form.set_field_options("External Network", self.cached_ext_network_opts.clone());
        }
        self.form = Some(form);
        self.view_state = ViewState::Create;
        let _ = self.action_tx.send(Action::FetchNetworks);
    }

    fn close_form(&mut self) {
        self.form = None;
        self.view_state = ViewState::List;
    }

    fn handle_list_key(&mut self, key: KeyEvent) -> Option<Action> {
        // SelectPopup takes priority (server/port selection)
        if let Some(ref mut popup) = self.select_popup {
            match popup.handle_key(key) {
                SelectResult::Selected(selected_id) => {
                    self.select_popup = None;
                    // If we have a pending_fip_id, this is a port selection
                    if self.pending_fip_id.is_some() {
                        self.handle_port_selected(selected_id);
                    } else {
                        // Server selection — start FetchPorts flow
                        self.handle_server_selected(selected_id);
                    }
                    return None;
                }
                SelectResult::Cancelled => {
                    self.select_popup = None;
                    self.pending_fip_id = None;
                    return None;
                }
                SelectResult::Pending => return None,
            }
        }

        if self.resource_list.handle_nav_key(key) {
            return None;
        }

        match key.code {
            KeyCode::Char('c') => {
                self.open_create_form();
                Some(Action::EnterFormMode)
            }
            KeyCode::Char('D') => {
                if let Some(fip) = self.selected_fip() {
                    let id = fip.id.clone();
                    let ip = fip.floating_ip_address.clone();
                    self.confirm.open(
                        ConfirmDialog::yes_no(format!("Delete floating IP '{ip}'?")),
                        PendingAction::DeleteFloatingIp { id, ip },
                    );
                }
                None
            }
            KeyCode::Char('d') => {
                if !self.keymap_hints_shown.contains(&'d') {
                    self.keymap_hints_shown.insert('d');
                    Some(Action::ShowToast {
                        message: "Delete is now Shift+D. Press 'x' to disassociate.".into(),
                    })
                } else {
                    None
                }
            }
            KeyCode::Char('a') => {
                if let Some(fip) = self.selected_fip().filter(|f| f.port_id.is_none()) {
                    let _ = fip; // used only as guard
                    let items = self.build_server_items();
                    if !items.is_empty() {
                        self.select_popup = Some(SelectPopup::new("Select Server", items));
                    }
                }
                None
            }
            KeyCode::Char('x') => {
                if let Some(fip) = self.selected_fip().filter(|f| f.port_id.is_some()) {
                    let id = fip.id.clone();
                    let ip = fip.floating_ip_address.clone();
                    let details = Self::fip_disassociate_detail_lines(fip);
                    self.confirm.open(
                        ConfirmDialog::type_to_confirm_with_details(
                            format!("Disassociate {ip}? External access will be lost immediately."),
                            ip,
                            details,
                        ),
                        PendingAction::DisassociateFloatingIp { fip_id: id },
                    );
                }
                None
            }
            KeyCode::Char('r') => Some(Action::FetchFloatingIps),
            KeyCode::Left => Some(Action::FocusSidebar),
            KeyCode::Esc => Some(Action::Back),
            _ => None,
        }
    }

    fn handle_server_selected(&mut self, server_id: String) {
        if let Some(fip) = self.selected_fip() {
            self.pending_fip_id = Some(fip.id.clone());
            self.pending_ports_server_id = Some(server_id.clone());
            self.loading_ports = true;
            let _ = self.action_tx.send(Action::FetchPorts { server_id });
        }
    }

    fn handle_create_key(&mut self, key: KeyEvent) -> Option<Action> {
        let Some(form) = self.form.as_mut() else {
            self.close_form();
            return None;
        };

        match form.handle_key(key) {
            FormAction::Submit(values) => {
                let network_id = values
                    .get("External Network")
                    .and_then(|v| match v {
                        crate::ui::form::FormValue::Selected(s) => Some(s.clone()),
                        _ => None,
                    })
                    .unwrap_or_default();

                self.close_form();
                let _ = self.action_tx.send(Action::CreateFloatingIp { network_id });
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

impl Component for FloatingIpModule {
    fn refresh_action(&self) -> Option<Action> { Some(Action::FetchFloatingIps) }
    fn is_modal(&self) -> bool { self.confirm.is_active() || self.form.is_some() || self.select_popup.is_some() }

    fn set_admin(&mut self, is_admin: bool) {
        self.is_admin = is_admin;
    }

    fn set_all_tenants(&mut self, v: bool) {
        self.all_tenants = v;
        self.resource_list = ResourceList::new(fip_columns(v));
    }

    fn handle_key(&mut self, key: KeyEvent) -> Option<Action> {
        if let Some(result) = self.confirm.handle_key(key, Self::resolve_action) {
            return result;
        }

        match &self.view_state {
            ViewState::List => self.handle_list_key(key),
            ViewState::Create => self.handle_create_key(key),
            ViewState::Detail(_) => None,
        }
    }

    fn handle_event(&mut self, event: &AppEvent) {
        match event {
            AppEvent::FloatingIpsLoaded(fips) => {
                self.floating_ips = fips.clone();
                self.loading = false;
                self.error_message = None;
                let rows = self.rows();
                self.resource_list.set_rows(rows);
            }
            AppEvent::FloatingIpCreated(_) | AppEvent::FloatingIpDeleted { .. } => {
                let _ = self.action_tx.send(Action::FetchFloatingIps);
            }
            AppEvent::FloatingIpAssociated(fip) => {
                // Update local state immediately for visual feedback
                if let Some(local_fip) = self.floating_ips.iter_mut().find(|f| f.id == fip.id) {
                    local_fip.port_id = fip.port_id.clone();
                    local_fip.fixed_ip_address = fip.fixed_ip_address.clone();
                    local_fip.status = "ACTIVE".into();
                }
                let _ = self.action_tx.send(Action::FetchFloatingIps);
            }
            AppEvent::FloatingIpDisassociated(fip) => {
                // Update local state immediately for visual feedback
                if let Some(local_fip) = self.floating_ips.iter_mut().find(|f| f.id == fip.id) {
                    local_fip.port_id = None;
                    local_fip.fixed_ip_address = None;
                    local_fip.status = "DOWN".into();
                }
                let _ = self.action_tx.send(Action::FetchFloatingIps);
            }
            AppEvent::ServersLoaded(servers) => {
                self.cached_servers = servers.clone();
            }
            AppEvent::NetworksLoaded(networks) => {
                self.cached_networks = networks.clone();
                let opts: Vec<SelectOption> = networks
                    .iter()
                    .filter(|n| n.external)
                    .map(|n| SelectOption::new(&n.id, &n.name))
                    .collect();
                self.cached_ext_network_opts = opts.clone();
                if let Some(form) = &mut self.form {
                    form.set_field_options("External Network", opts);
                }
            }
            AppEvent::PortsLoaded { server_id, ports } => {
                // Only consume if this response matches our pending request
                if self.pending_ports_server_id.as_deref() == Some(server_id.as_str()) {
                    self.handle_ports_loaded(ports.clone());
                }
            }
            AppEvent::ApiError {
                operation, message, ..
            } => {
                self.error_message = Some(format!("{operation}: {message}"));
                self.loading = false;
                self.loading_ports = false;
            }
            _ => {}
        }
    }

    fn render(&self, frame: &mut Frame, area: Rect) {
        match &self.view_state {
            ViewState::List => {
                self.resource_list.render(frame, area);
            }
            ViewState::Create => {
                if let Some(form) = &self.form {
                    form.render(frame, area);
                } else {
                    self.resource_list.render(frame, area);
                }
            }
            ViewState::Detail(_) => {}
        }

        self.confirm.render(frame, area);

        // Overlay: SelectPopup
        if let Some(ref popup) = self.select_popup {
            popup.render(frame, area);
        }
    }

    fn content_title(&self) -> Option<String> {
        match &self.view_state {
            ViewState::List => None,
            ViewState::Detail(id) => {
                let addr = self.floating_ips.iter()
                    .find(|r| r.id == *id)
                    .map(|r| r.floating_ip_address.as_str())
                    .unwrap_or("...");
                Some(format!("Floating IP: {addr}"))
            }
            ViewState::Create => Some("Create Floating IP".into()),
        }
    }

    fn help_hint(&self) -> &str {
        match &self.view_state {
            ViewState::List => "c:Create a:Associate x:Disassociate D:Delete r:Refresh",
            ViewState::Create => "Esc:Cancel Tab:Next Enter:Submit",
            ViewState::Detail(_) => "",
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

    use crate::models::neutron::{FixedIp, Network, Port};
    use crate::models::nova::Server;

    fn make_fip(id: &str, ip: &str, status: &str) -> FloatingIp {
        FloatingIp {
            id: id.into(),
            floating_ip_address: ip.into(),
            status: status.into(),
            port_id: None,
            floating_network_id: "ext-net-1".into(),
            fixed_ip_address: None,
            router_id: None,
            tenant_id: None,
        }
    }

    fn make_fip_associated(id: &str, ip: &str, port_id: &str) -> FloatingIp {
        FloatingIp {
            id: id.into(),
            floating_ip_address: ip.into(),
            status: "ACTIVE".into(),
            port_id: Some(port_id.into()),
            floating_network_id: "ext-net-1".into(),
            fixed_ip_address: Some("10.0.0.5".into()),
            router_id: None,
            tenant_id: None,
        }
    }

    fn make_server(id: &str, name: &str, status: &str) -> Server {
        Server {
            id: id.into(),
            name: name.into(),
            status: status.into(),
            addresses: Default::default(),
            flavor: crate::models::nova::FlavorRef {
                id: "f1".into(),
                original_name: None,
                vcpus: None,
                ram: None,
                disk: None,
            },
            image: None,
            key_name: None,
            availability_zone: None,
            created: "2026-01-01".into(),
            updated: None,
            tenant_id: None,
            host_id: None,
            host: None,
            volumes_attached: vec![],
            security_groups: vec![],
        }
    }

    fn make_port(id: &str, network_id: &str, ip: &str, device_id: &str) -> Port {
        Port {
            id: id.into(),
            name: None,
            network_id: network_id.into(),
            fixed_ips: vec![FixedIp {
                subnet_id: "sub-1".into(),
                ip_address: ip.into(),
            }],
            device_id: Some(device_id.into()),
            device_owner: Some("compute:az1".into()),
            status: "ACTIVE".into(),
            tenant_id: None,
        }
    }

    fn make_network(id: &str, name: &str, external: bool) -> Network {
        Network {
            id: id.into(),
            name: name.into(),
            status: "ACTIVE".into(),
            description: None,
            admin_state_up: true,
            external,
            shared: false,
            mtu: None,
            port_security_enabled: None,
            subnets: vec![],
            provider_network_type: None,
            provider_physical_network: None,
            provider_segmentation_id: None,
            tenant_id: None,
        }
    }

    fn setup() -> (FloatingIpModule, ActionReceiver) {
        let (tx, rx) = test_action_channel();
        let mut module = FloatingIpModule::new(tx);
        let fips = vec![
            make_fip("fip-1", "203.0.113.10", "ACTIVE"),
            make_fip("fip-2", "203.0.113.11", "DOWN"),
            make_fip("fip-3", "203.0.113.12", "ACTIVE"),
        ];
        module.handle_event(&AppEvent::FloatingIpsLoaded(fips));
        (module, rx)
    }

    fn setup_with_servers() -> (FloatingIpModule, ActionReceiver) {
        let (mut module, rx) = setup();
        let servers = vec![
            make_server("srv-1", "web-01", "ACTIVE"),
            make_server("srv-2", "web-02", "SHUTOFF"),
            make_server("srv-3", "db-01", "ACTIVE"),
        ];
        module.handle_event(&AppEvent::ServersLoaded(servers));
        (module, rx)
    }

    #[test]
    fn test_initial_state_is_list() {
        let (tx, _rx) = test_action_channel();
        let module = FloatingIpModule::new(tx);
        assert_eq!(*module.view_state(), ViewState::List);
        assert!(module.floating_ips().is_empty());
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
    fn test_handle_key_c_opens_create() {
        let (mut module, _rx) = setup();
        module.handle_key(key(KeyCode::Char('c')));
        assert_eq!(*module.view_state(), ViewState::Create);
        assert!(module.form.is_some());
    }

    #[test]
    fn test_handle_key_shift_d_delete_confirm() {
        let (mut module, _rx) = setup();
        assert!(!module.confirm.is_active());
        module.handle_key(key(KeyCode::Char('D')));
        assert!(module.confirm.is_active());
    }

    #[test]
    fn test_confirm_delete_fip() {
        let (mut module, _rx) = setup();
        module.handle_key(key(KeyCode::Char('D')));
        let action = module.handle_key(key(KeyCode::Char('y')));
        assert!(matches!(action, Some(Action::DeleteFloatingIp { .. })));
        assert!(!module.confirm.is_active());
    }

    #[test]
    fn test_handle_key_r_fetches_fips() {
        let (mut module, _rx) = setup();
        let action = module.handle_key(key(KeyCode::Char('r')));
        assert!(matches!(action, Some(Action::FetchFloatingIps)));
    }

    #[test]
    fn test_handle_event_fips_loaded() {
        let (tx, _rx) = test_action_channel();
        let mut module = FloatingIpModule::new(tx);
        let fips = vec![make_fip("fip-1", "1.2.3.4", "ACTIVE")];
        module.handle_event(&AppEvent::FloatingIpsLoaded(fips));
        assert_eq!(module.floating_ips().len(), 1);
    }

    #[test]
    fn test_handle_event_fip_created_triggers_refresh() {
        let (mut module, mut rx) = setup();
        let fip = make_fip("fip-new", "1.2.3.5", "ACTIVE");
        module.handle_event(&AppEvent::FloatingIpCreated(fip));
        let action = rx.try_recv().unwrap();
        assert!(matches!(action, Action::FetchFloatingIps));
    }

    #[test]
    fn test_handle_event_fip_deleted_triggers_refresh() {
        let (mut module, mut rx) = setup();
        module.handle_event(&AppEvent::FloatingIpDeleted { id: "fip-1".into() });
        let action = rx.try_recv().unwrap();
        assert!(matches!(action, Action::FetchFloatingIps));
    }

    #[test]
    fn test_handle_event_api_error() {
        let (mut module, _rx) = setup();
        module.handle_event(&AppEvent::ApiError {
            operation: "create".into(),
            message: "pool exhausted".into(),
        });
        assert_eq!(module.error_message(), Some("create: pool exhausted"));
    }

    // -- Form integration tests -----------------------------------------------

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
        assert_eq!(form.field_count(), 1);
        assert_eq!(form.focused_field_name(), "External Network");
    }

    #[test]
    fn test_help_hint_list() {
        let (module, _rx) = setup();
        assert_eq!(module.help_hint(), "c:Create a:Associate x:Disassociate D:Delete r:Refresh");
    }

    #[test]
    fn test_help_hint_create() {
        let (mut module, _rx) = setup();
        module.handle_key(key(KeyCode::Char('c')));
        assert_eq!(module.help_hint(), "Esc:Cancel Tab:Next Enter:Submit");
    }

    // -- Keymap migration tests -----------------------------------------------

    #[test]
    fn test_d_key_shows_hint_toast_once() {
        let (mut module, _rx) = setup();
        let action = module.handle_key(key(KeyCode::Char('d')));
        assert!(matches!(action, Some(Action::ShowToast { .. })));

        // Second press: no toast
        let action = module.handle_key(key(KeyCode::Char('d')));
        assert!(action.is_none());
    }

    #[test]
    fn test_d_key_does_not_open_confirm() {
        let (mut module, _rx) = setup();
        module.handle_key(key(KeyCode::Char('d')));
        assert!(!module.confirm.is_active());
    }

    // -- Associate tests -------------------------------------------------------

    #[test]
    fn test_a_on_unassociated_fip_opens_server_popup() {
        let (mut module, _rx) = setup_with_servers();
        // fip-1 has no port_id (unassociated)
        module.handle_key(key(KeyCode::Char('a')));
        assert!(module.select_popup.is_some());
    }

    #[test]
    fn test_a_on_associated_fip_no_popup() {
        let (tx, _rx) = test_action_channel();
        let mut module = FloatingIpModule::new(tx);
        let fips = vec![make_fip_associated("fip-1", "203.0.113.10", "port-1")];
        module.handle_event(&AppEvent::FloatingIpsLoaded(fips));
        module.handle_event(&AppEvent::ServersLoaded(vec![
            make_server("srv-1", "web-01", "ACTIVE"),
        ]));
        module.handle_key(key(KeyCode::Char('a')));
        assert!(module.select_popup.is_none());
    }

    #[test]
    fn test_a_on_empty_servers_no_popup() {
        let (mut module, _rx) = setup();
        // No servers loaded
        module.handle_key(key(KeyCode::Char('a')));
        assert!(module.select_popup.is_none());
    }

    #[test]
    fn test_server_popup_filters_active_paused_shutoff() {
        let (tx, _rx) = test_action_channel();
        let mut module = FloatingIpModule::new(tx);
        let fips = vec![make_fip("fip-1", "203.0.113.10", "ACTIVE")];
        module.handle_event(&AppEvent::FloatingIpsLoaded(fips));
        let servers = vec![
            make_server("srv-1", "web-01", "ACTIVE"),
            make_server("srv-2", "web-02", "SHUTOFF"),
            make_server("srv-3", "db-01", "BUILD"),  // should be filtered out
            make_server("srv-4", "db-02", "PAUSED"),
        ];
        module.handle_event(&AppEvent::ServersLoaded(servers));

        module.handle_key(key(KeyCode::Char('a')));
        let popup = module.select_popup.as_ref().unwrap();
        assert_eq!(popup.items().len(), 3); // ACTIVE, SHUTOFF, PAUSED
    }

    #[test]
    fn test_server_selected_triggers_fetch_ports() {
        let (mut module, mut rx) = setup_with_servers();
        module.handle_key(key(KeyCode::Char('a'))); // open server popup
        // Select first server (Enter)
        module.handle_key(key(KeyCode::Enter));
        assert!(module.select_popup.is_none());
        assert!(module.loading_ports);
        assert!(module.pending_fip_id.is_some());

        let action = rx.try_recv().unwrap();
        assert!(matches!(action, Action::FetchPorts { .. }));
    }

    #[test]
    fn test_ports_loaded_zero_shows_toast() {
        let (mut module, mut rx) = setup_with_servers();
        module.handle_key(key(KeyCode::Char('a')));
        module.handle_key(key(KeyCode::Enter));
        // Drain FetchPorts action
        let _ = rx.try_recv();

        // Load zero ports
        module.handle_event(&AppEvent::PortsLoaded {
            server_id: "srv-1".into(),
            ports: vec![],
        });

        assert!(!module.loading_ports);
        assert!(module.pending_fip_id.is_none());
        let action = rx.try_recv().unwrap();
        assert!(matches!(action, Action::ShowToast { message } if message.contains("No ports")));
    }

    #[test]
    fn test_ports_loaded_one_auto_confirm() {
        let (mut module, mut rx) = setup_with_servers();
        let networks = vec![make_network("net-1", "private", false)];
        module.handle_event(&AppEvent::NetworksLoaded(networks));

        module.handle_key(key(KeyCode::Char('a')));
        module.handle_key(key(KeyCode::Enter)); // select server srv-1
        let _ = rx.try_recv(); // drain FetchPorts

        // Load one port
        module.handle_event(&AppEvent::PortsLoaded {
            server_id: "srv-1".into(),
            ports: vec![make_port("port-1", "net-1", "10.0.0.5", "srv-1")],
        });

        assert!(!module.loading_ports);
        assert!(module.confirm.is_active()); // auto-confirm dialog
        assert!(module.pending_fip_id.is_none());
    }

    #[test]
    fn test_ports_loaded_multiple_shows_port_popup() {
        let (mut module, mut rx) = setup_with_servers();
        let networks = vec![make_network("net-1", "private", false)];
        module.handle_event(&AppEvent::NetworksLoaded(networks));

        module.handle_key(key(KeyCode::Char('a')));
        module.handle_key(key(KeyCode::Enter));
        let _ = rx.try_recv();

        // Load multiple ports
        module.handle_event(&AppEvent::PortsLoaded {
            server_id: "srv-1".into(),
            ports: vec![
                make_port("port-1", "net-1", "10.0.0.5", "srv-1"),
                make_port("port-2", "net-1", "10.0.0.6", "srv-1"),
            ],
        });

        assert!(!module.loading_ports);
        assert!(module.select_popup.is_some());
        let popup = module.select_popup.as_ref().unwrap();
        assert_eq!(popup.items().len(), 2);
    }

    #[test]
    fn test_port_selected_opens_confirm() {
        let (mut module, mut rx) = setup_with_servers();
        let networks = vec![make_network("net-1", "private", false)];
        module.handle_event(&AppEvent::NetworksLoaded(networks));

        module.handle_key(key(KeyCode::Char('a')));
        module.handle_key(key(KeyCode::Enter)); // select server
        let _ = rx.try_recv();

        module.handle_event(&AppEvent::PortsLoaded {
            server_id: "srv-1".into(),
            ports: vec![
                make_port("port-1", "net-1", "10.0.0.5", "srv-1"),
                make_port("port-2", "net-1", "10.0.0.6", "srv-1"),
            ],
        });

        // Select first port
        module.handle_key(key(KeyCode::Enter));
        assert!(module.select_popup.is_none());
        assert!(module.confirm.is_active());
    }

    #[test]
    fn test_confirm_associate_produces_action() {
        let (mut module, mut rx) = setup_with_servers();
        let networks = vec![make_network("net-1", "private", false)];
        module.handle_event(&AppEvent::NetworksLoaded(networks));

        module.handle_key(key(KeyCode::Char('a')));
        module.handle_key(key(KeyCode::Enter));
        let _ = rx.try_recv();

        // Single port auto-confirm
        module.handle_event(&AppEvent::PortsLoaded {
            server_id: "srv-1".into(),
            ports: vec![make_port("port-1", "net-1", "10.0.0.5", "srv-1")],
        });

        assert!(module.confirm.is_active());
        let action = module.handle_key(key(KeyCode::Char('y')));
        assert!(matches!(action, Some(Action::AssociateFloatingIp { fip_id, port_id }) if fip_id == "fip-1" && port_id == "port-1"));
    }

    // -- Disassociate tests ----------------------------------------------------

    #[test]
    fn test_x_on_associated_fip_opens_confirm() {
        let (tx, _rx) = test_action_channel();
        let mut module = FloatingIpModule::new(tx);
        let fips = vec![make_fip_associated("fip-1", "203.0.113.10", "port-1")];
        module.handle_event(&AppEvent::FloatingIpsLoaded(fips));

        module.handle_key(key(KeyCode::Char('x')));
        assert!(module.confirm.is_active());
    }

    #[test]
    fn test_x_on_unassociated_fip_no_confirm() {
        let (mut module, _rx) = setup();
        module.handle_key(key(KeyCode::Char('x')));
        assert!(!module.confirm.is_active());
    }

    #[test]
    fn test_confirm_disassociate_produces_action() {
        let (tx, _rx) = test_action_channel();
        let mut module = FloatingIpModule::new(tx);
        let fips = vec![make_fip_associated("fip-1", "203.0.113.10", "port-1")];
        module.handle_event(&AppEvent::FloatingIpsLoaded(fips));

        module.handle_key(key(KeyCode::Char('x')));
        // TypeToConfirm: type the IP address
        for c in "203.0.113.10".chars() {
            module.handle_key(key(KeyCode::Char(c)));
        }
        let action = module.handle_key(key(KeyCode::Enter));
        assert!(matches!(action, Some(Action::DisassociateFloatingIp { fip_id }) if fip_id == "fip-1"));
    }

    // -- Event handling tests --------------------------------------------------

    #[test]
    fn test_handle_event_fip_associated_triggers_refresh() {
        let (mut module, mut rx) = setup();
        let fip = make_fip_associated("fip-1", "203.0.113.10", "port-1");
        module.handle_event(&AppEvent::FloatingIpAssociated(fip));
        let action = rx.try_recv().unwrap();
        assert!(matches!(action, Action::FetchFloatingIps));
    }

    #[test]
    fn test_handle_event_fip_disassociated_updates_local_and_refreshes() {
        let (mut module, mut rx) = setup();
        // Add a FIP to local state first
        let fip = make_fip_associated("fip-1", "203.0.113.10", "port-1");
        module.floating_ips.push(fip.clone());
        // Disassociate event
        let disassociated_fip = make_fip("fip-1", "203.0.113.10", "DOWN");
        module.handle_event(&AppEvent::FloatingIpDisassociated(disassociated_fip));
        // Local state should be updated immediately
        assert!(module.floating_ips[0].port_id.is_none());
        assert_eq!(module.floating_ips[0].status, "DOWN");
        // Should trigger refresh
        let action = rx.try_recv().unwrap();
        assert!(matches!(action, Action::FetchFloatingIps));
    }

    #[test]
    fn test_handle_event_servers_loaded_caches() {
        let (mut module, _rx) = setup();
        let servers = vec![make_server("srv-1", "web-01", "ACTIVE")];
        module.handle_event(&AppEvent::ServersLoaded(servers));
        assert_eq!(module.cached_servers.len(), 1);
    }

    #[test]
    fn test_handle_event_networks_loaded_caches() {
        let (mut module, _rx) = setup();
        let networks = vec![make_network("net-1", "private", false)];
        module.handle_event(&AppEvent::NetworksLoaded(networks));
        assert_eq!(module.cached_networks.len(), 1);
    }

    // -- resolve_action tests --------------------------------------------------

    #[test]
    fn test_resolve_associate() {
        let action = FloatingIpModule::resolve_action(PendingAction::AssociateFloatingIp {
            fip_id: "fip-1".into(),
            port_id: "port-1".into(),
        });
        assert!(matches!(action, Some(Action::AssociateFloatingIp { fip_id, port_id }) if fip_id == "fip-1" && port_id == "port-1"));
    }

    #[test]
    fn test_resolve_disassociate() {
        let action = FloatingIpModule::resolve_action(PendingAction::DisassociateFloatingIp {
            fip_id: "fip-1".into(),
        });
        assert!(matches!(action, Some(Action::DisassociateFloatingIp { fip_id }) if fip_id == "fip-1"));
    }

    // -- is_modal tests --------------------------------------------------------

    #[test]
    fn test_is_modal_with_select_popup() {
        let (mut module, _rx) = setup_with_servers();
        assert!(!module.is_modal());
        module.handle_key(key(KeyCode::Char('a')));
        assert!(module.is_modal()); // select popup active
    }

    // -- Cancel flow tests -----------------------------------------------------

    #[test]
    fn test_server_popup_cancel_clears_state() {
        let (mut module, _rx) = setup_with_servers();
        module.handle_key(key(KeyCode::Char('a')));
        assert!(module.select_popup.is_some());
        module.handle_key(key(KeyCode::Esc));
        assert!(module.select_popup.is_none());
        assert!(module.pending_fip_id.is_none());
    }

    #[test]
    fn test_port_popup_cancel_clears_state() {
        let (mut module, mut rx) = setup_with_servers();
        let networks = vec![make_network("net-1", "private", false)];
        module.handle_event(&AppEvent::NetworksLoaded(networks));

        module.handle_key(key(KeyCode::Char('a')));
        module.handle_key(key(KeyCode::Enter));
        let _ = rx.try_recv();

        module.handle_event(&AppEvent::PortsLoaded {
            server_id: "srv-1".into(),
            ports: vec![
                make_port("port-1", "net-1", "10.0.0.5", "srv-1"),
                make_port("port-2", "net-1", "10.0.0.6", "srv-1"),
            ],
        });

        assert!(module.select_popup.is_some());
        module.handle_key(key(KeyCode::Esc)); // cancel port popup
        assert!(module.select_popup.is_none());
        assert!(module.pending_fip_id.is_none());
    }

    // -- Detail lines tests ----------------------------------------------------

    #[test]
    fn test_fip_disassociate_detail_lines() {
        let fip = make_fip_associated("fip-1", "203.0.113.10", "port-1");
        let lines = FloatingIpModule::fip_disassociate_detail_lines(&fip);
        assert!(lines.iter().any(|l| l.contains("203.0.113.10")));
        assert!(lines.iter().any(|l| l.contains("port-1")));
    }

    #[test]
    fn test_fip_associate_detail_lines() {
        let fip = make_fip("fip-1", "203.0.113.10", "ACTIVE");
        let lines = FloatingIpModule::fip_associate_detail_lines(&fip, "web-01", "10.0.0.5 on private");
        assert!(lines.iter().any(|l| l.contains("203.0.113.10")));
        assert!(lines.iter().any(|l| l.contains("web-01")));
        assert!(lines.iter().any(|l| l.contains("10.0.0.5 on private")));
    }
}
