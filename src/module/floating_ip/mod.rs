pub mod view_model;

use crossterm::event::{KeyCode, KeyEvent};
use ratatui::layout::Rect;
use ratatui::Frame;
use tokio::sync::mpsc;

use crate::action::Action;
use crate::component::Component;
use crate::event::AppEvent;
use crate::models::neutron::FloatingIp;
use crate::module::{ConfirmHandler, PendingAction, ViewState};
use crate::ui::confirm::ConfirmDialog;
use crate::ui::form::{FormAction, FormWidget, SelectOption};
use crate::ui::resource_list::{ResourceList, Row};

use self::view_model::{fip_columns, fip_create_defs, fip_to_row};

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
    cached_ext_network_opts: Vec<SelectOption>,
    action_tx: mpsc::UnboundedSender<Action>,
}

impl FloatingIpModule {
    pub fn new(action_tx: mpsc::UnboundedSender<Action>) -> Self {
        Self {
            view_state: ViewState::List,
            floating_ips: Vec::new(),
            loading: false,
            error_message: None,
            confirm: ConfirmHandler::new(),
            resource_list: ResourceList::new(fip_columns(false)),
            form: None,
            all_tenants: false,
            cached_ext_network_opts: Vec::new(),
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
        self.floating_ips.iter().map(|f| fip_to_row(f, self.all_tenants)).collect()
    }

    fn resolve_action(pending: PendingAction) -> Option<Action> {
        match pending {
            PendingAction::DeleteFloatingIp { id, .. } => Some(Action::DeleteFloatingIp { id }),
            _ => None,
        }
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
        if self.resource_list.handle_nav_key(key) {
            return None;
        }

        match key.code {
            KeyCode::Char('c') => {
                self.open_create_form();
                Some(Action::EnterFormMode)
            }
            KeyCode::Char('d') => {
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
            KeyCode::Char('r') => Some(Action::FetchFloatingIps),
            KeyCode::Left => Some(Action::FocusSidebar),
            KeyCode::Esc => Some(Action::Back),
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
    fn is_modal(&self) -> bool { self.confirm.is_active() || self.form.is_some() }

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
            AppEvent::NetworksLoaded(networks) => {
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
    }

    fn help_hint(&self) -> &str {
        match &self.view_state {
            ViewState::List => "c:Create d:Delete r:Refresh",
            ViewState::Create => "Esc:Cancel Tab:Next Enter:Submit",
            ViewState::Detail(_) => "",
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn key(code: KeyCode) -> KeyEvent {
        KeyEvent::from(code)
    }

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

    fn setup() -> (FloatingIpModule, mpsc::UnboundedReceiver<Action>) {
        let (tx, rx) = mpsc::unbounded_channel();
        let mut module = FloatingIpModule::new(tx);
        let fips = vec![
            make_fip("fip-1", "203.0.113.10", "ACTIVE"),
            make_fip("fip-2", "203.0.113.11", "DOWN"),
            make_fip("fip-3", "203.0.113.12", "ACTIVE"),
        ];
        module.handle_event(&AppEvent::FloatingIpsLoaded(fips));
        (module, rx)
    }

    #[test]
    fn test_initial_state_is_list() {
        let (tx, _rx) = mpsc::unbounded_channel();
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
    fn test_handle_key_d_delete_confirm() {
        let (mut module, _rx) = setup();
        assert!(!module.confirm.is_active());
        module.handle_key(key(KeyCode::Char('d')));
        assert!(module.confirm.is_active());
    }

    #[test]
    fn test_confirm_delete_fip() {
        let (mut module, _rx) = setup();
        module.handle_key(key(KeyCode::Char('d')));
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
        let (tx, _rx) = mpsc::unbounded_channel();
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
        assert_eq!(module.help_hint(), "c:Create d:Delete r:Refresh");
    }

    #[test]
    fn test_help_hint_create() {
        let (mut module, _rx) = setup();
        module.handle_key(key(KeyCode::Char('c')));
        assert_eq!(module.help_hint(), "Esc:Cancel Tab:Next Enter:Submit");
    }
}
