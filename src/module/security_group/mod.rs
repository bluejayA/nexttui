pub mod view_model;

use crossterm::event::{KeyCode, KeyEvent};
use ratatui::Frame;
use ratatui::layout::Rect;

use crate::action::Action;
use crate::component::Component;
use crate::context::ActionSender;
use crate::event::AppEvent;
use crate::models::neutron::SecurityGroup;
use crate::module::{ConfirmHandler, PendingAction, ViewState};
use crate::port::types::{RuleDirection, SecurityGroupCreateParams, SecurityGroupRuleCreateParams};
use crate::ui::confirm::ConfirmDialog;
use crate::ui::form::{FormAction, FormWidget};
use crate::ui::resource_list::{ResourceList, Row};

use self::view_model::{sg_columns, sg_create_defs, sg_detail_data, sg_rule_defs, sg_to_row};

pub struct SecurityGroupModule {
    view_state: ViewState,
    security_groups: Vec<SecurityGroup>,
    rule_selected: usize,
    /// When adding a rule from detail view, remembers which SG we're in.
    detail_sg_id: Option<String>,
    #[allow(dead_code)] // Phase 2: set to true on Action dispatch, render loading spinner
    loading: bool,
    error_message: Option<String>,
    confirm: ConfirmHandler,
    resource_list: ResourceList,
    form: Option<FormWidget>,
    all_tenants: bool,
    action_tx: ActionSender,
    context_target: Option<crate::context::types::ContextTarget>,
    context_recently_switched: bool,
}

impl SecurityGroupModule {
    pub fn new(action_tx: ActionSender) -> Self {
        Self {
            view_state: ViewState::List,
            security_groups: Vec::new(),
            rule_selected: 0,
            detail_sg_id: None,
            loading: false,
            error_message: None,
            confirm: ConfirmHandler::new(),
            resource_list: ResourceList::new(sg_columns(false)),
            form: None,
            all_tenants: false,
            action_tx,
            context_target: None,
            context_recently_switched: false,
        }
    }

    fn destructive_confirm(&self, message: impl Into<String>) -> ConfirmDialog {
        ConfirmDialog::for_destructive_opt(
            message,
            self.context_target.as_ref(),
            self.context_recently_switched,
        )
    }

    pub fn view_state(&self) -> &ViewState {
        &self.view_state
    }

    pub fn security_groups(&self) -> &[SecurityGroup] {
        &self.security_groups
    }

    pub fn selected_index(&self) -> usize {
        self.resource_list.selected_index()
    }

    pub fn error_message(&self) -> Option<&str> {
        self.error_message.as_deref()
    }

    fn selected_sg(&self) -> Option<&SecurityGroup> {
        self.security_groups
            .get(self.resource_list.selected_index())
    }

    fn current_sg(&self) -> Option<&SecurityGroup> {
        if let ViewState::Detail(ref id) = self.view_state {
            self.security_groups.iter().find(|sg| sg.id == *id)
        } else {
            None
        }
    }

    fn rows(&self) -> Vec<Row> {
        self.security_groups
            .iter()
            .map(|sg| sg_to_row(sg, self.all_tenants))
            .collect()
    }

    fn resolve_action(pending: PendingAction) -> Option<Action> {
        match pending {
            PendingAction::DeleteSecurityGroup { id, .. } => {
                Some(Action::DeleteSecurityGroup { id })
            }
            PendingAction::DeleteSecurityGroupRule { rule_id } => {
                Some(Action::DeleteSecurityGroupRule { rule_id })
            }
            _ => None,
        }
    }

    fn open_create_form(&mut self) {
        let defs = sg_create_defs();
        self.form = Some(FormWidget::new("Create Security Group", defs));
        self.view_state = ViewState::Create;
    }

    fn open_rule_form(&mut self, sg_id: String) {
        self.detail_sg_id = Some(sg_id);
        let defs = sg_rule_defs();
        self.form = Some(FormWidget::new("Add Security Group Rule", defs));
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
                if let Some(sg) = self.selected_sg() {
                    let id = sg.id.clone();
                    self.rule_selected = 0;
                    self.view_state = ViewState::Detail(id);
                }
                None
            }
            KeyCode::Char('c') => {
                self.open_create_form();
                Some(Action::EnterFormMode)
            }
            KeyCode::Char('d') => {
                if let Some(sg) = self.selected_sg() {
                    let id = sg.id.clone();
                    let name = sg.name.clone();
                    self.confirm.open(
                        self.destructive_confirm(format!("Delete security group '{name}'?")),
                        PendingAction::DeleteSecurityGroup { id, name },
                    );
                }
                None
            }
            KeyCode::Char('r') => Some(Action::FetchSecurityGroups),
            KeyCode::Left => Some(Action::FocusSidebar),
            KeyCode::Esc => Some(Action::Back),
            _ => None,
        }
    }

    fn handle_detail_key(&mut self, key: KeyEvent) -> Option<Action> {
        // Rule navigation in detail view
        match key.code {
            KeyCode::Char('j') | KeyCode::Down => {
                if let Some(sg) = self.current_sg() {
                    let max = sg.security_group_rules.len().saturating_sub(1);
                    if self.rule_selected < max {
                        self.rule_selected += 1;
                    }
                }
                return None;
            }
            KeyCode::Char('k') | KeyCode::Up => {
                self.rule_selected = self.rule_selected.saturating_sub(1);
                return None;
            }
            _ => {}
        }

        match key.code {
            KeyCode::Esc | KeyCode::Char('q') | KeyCode::Left => {
                self.view_state = ViewState::List;
                None
            }
            KeyCode::Char('a') => {
                if let Some(sg) = self.current_sg() {
                    let sg_id = sg.id.clone();
                    self.open_rule_form(sg_id);
                    return Some(Action::EnterFormMode);
                }
                None
            }
            KeyCode::Char('d') => {
                if let Some(sg) = self.current_sg()
                    && let Some(rule) = sg.security_group_rules.get(self.rule_selected)
                {
                    let rule_id = rule.id.clone();
                    let short_id = if rule_id.len() > 8 {
                        &rule_id[..8]
                    } else {
                        &rule_id
                    };
                    self.confirm.open(
                        self.destructive_confirm(format!("Delete rule {short_id}...?")),
                        PendingAction::DeleteSecurityGroupRule { rule_id },
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

        let is_rule_form = form.title() == "Add Security Group Rule";

        match form.handle_key(key) {
            FormAction::Submit(values) => {
                if is_rule_form {
                    // Extract rule fields
                    let sg_id = self.detail_sg_id.clone().unwrap_or_default();
                    let direction_str = values
                        .get("Direction")
                        .and_then(|v| match v {
                            crate::ui::form::FormValue::Selected(s) => Some(s.clone()),
                            _ => None,
                        })
                        .unwrap_or_default();
                    let direction = if direction_str == "egress" {
                        RuleDirection::Egress
                    } else {
                        RuleDirection::Ingress
                    };
                    let protocol = values.get("Protocol").and_then(|v| match v {
                        crate::ui::form::FormValue::Selected(s) => {
                            if s.is_empty() {
                                None
                            } else {
                                Some(s.clone())
                            }
                        }
                        _ => None,
                    });
                    let port_range_min = values.get("Port Min").and_then(|v| match v {
                        crate::ui::form::FormValue::Text(s) => s.parse::<u16>().ok(),
                        _ => None,
                    });
                    let port_range_max = values.get("Port Max").and_then(|v| match v {
                        crate::ui::form::FormValue::Text(s) => s.parse::<u16>().ok(),
                        _ => None,
                    });
                    let remote_ip_prefix = values.get("Source CIDR").and_then(|v| match v {
                        crate::ui::form::FormValue::Text(s) => {
                            if s.is_empty() {
                                None
                            } else {
                                Some(s.clone())
                            }
                        }
                        _ => None,
                    });

                    // Return to detail view, not list
                    self.form = None;
                    if let Some(sg_id) = self.detail_sg_id.take() {
                        self.view_state = ViewState::Detail(sg_id.clone());
                    } else {
                        self.view_state = ViewState::List;
                    }

                    let _ = self.action_tx.send(Action::CreateSecurityGroupRule(
                        SecurityGroupRuleCreateParams {
                            security_group_id: sg_id,
                            direction,
                            protocol,
                            port_range_min,
                            port_range_max,
                            remote_ip_prefix,
                            remote_group_id: None,
                            ethertype: None,
                        },
                    ));
                    Some(Action::ExitFormMode)
                } else {
                    // SG create form
                    let name = values
                        .get("Name")
                        .and_then(|v| match v {
                            crate::ui::form::FormValue::Text(s) => Some(s.clone()),
                            _ => None,
                        })
                        .unwrap_or_default();
                    let description = values.get("Description").and_then(|v| match v {
                        crate::ui::form::FormValue::Text(s) => {
                            if s.is_empty() {
                                None
                            } else {
                                Some(s.clone())
                            }
                        }
                        _ => None,
                    });

                    self.close_form();
                    let _ = self.action_tx.send(Action::CreateSecurityGroup(
                        SecurityGroupCreateParams { name, description },
                    ));
                    Some(Action::ExitFormMode)
                }
            }
            FormAction::Cancel => {
                if is_rule_form {
                    // Return to detail view on cancel from rule form
                    self.form = None;
                    if let Some(sg_id) = self.detail_sg_id.take() {
                        self.view_state = ViewState::Detail(sg_id);
                    } else {
                        self.view_state = ViewState::List;
                    }
                } else {
                    self.close_form();
                }
                Some(Action::ExitFormMode)
            }
            FormAction::None => None,
        }
    }
}

impl Component for SecurityGroupModule {
    fn refresh_action(&self) -> Option<Action> {
        Some(Action::FetchSecurityGroups)
    }
    fn is_modal(&self) -> bool {
        self.confirm.is_active() || self.form.is_some()
    }

    fn set_all_tenants(&mut self, v: bool) {
        self.all_tenants = v;
        self.resource_list = ResourceList::new(sg_columns(v));
    }

    fn handle_key(&mut self, key: KeyEvent) -> Option<Action> {
        if let Some(result) = self.confirm.handle_key(key, Self::resolve_action) {
            return result;
        }

        match &self.view_state {
            ViewState::List => self.handle_list_key(key),
            ViewState::Detail(_) => self.handle_detail_key(key),
            ViewState::Create => self.handle_create_key(key),
        }
    }

    fn on_context_changed(&mut self) {
        self.security_groups.clear();
        self.detail_sg_id = None;
        self.loading = true;
        self.error_message = None;
        self.resource_list.set_rows(Vec::new());
        self.view_state = ViewState::List;
        // Codex review 2차 P1: destructive confirm/form must not carry over
        // across a context switch.
        self.confirm = ConfirmHandler::new();
        self.form = None;
    }

    fn set_context_state(
        &mut self,
        target: Option<crate::context::types::ContextTarget>,
        recently_switched: bool,
    ) {
        self.context_target = target;
        self.context_recently_switched = recently_switched;
    }

    fn handle_event(&mut self, event: &AppEvent) {
        match event {
            AppEvent::SecurityGroupsLoaded(sgs) => {
                self.security_groups = sgs.clone();
                self.loading = false;
                self.error_message = None;
                let rows = self.rows();
                self.resource_list.set_rows(rows);
            }
            AppEvent::SecurityGroupCreated(_)
            | AppEvent::SecurityGroupDeleted { .. }
            | AppEvent::SecurityGroupRuleCreated(_)
            | AppEvent::SecurityGroupRuleDeleted { .. } => {
                let _ = self.action_tx.send(Action::FetchSecurityGroups);
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
                if let Some(sg) = self.security_groups.iter().find(|s| s.id == *id) {
                    let data = sg_detail_data(sg);
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

        self.confirm.render(frame, area);
    }

    fn content_title(&self) -> Option<String> {
        match &self.view_state {
            ViewState::List => None,
            ViewState::Detail(id) => {
                let name = self
                    .security_groups
                    .iter()
                    .find(|r| r.id == *id)
                    .map(|r| r.name.as_str())
                    .unwrap_or("...");
                Some(format!("Security Group: {name}"))
            }
            ViewState::Create => Some("Create Security Group".into()),
        }
    }

    fn help_hint(&self) -> &str {
        match &self.view_state {
            ViewState::List => "Enter:Detail c:Create d:Delete r:Refresh",
            ViewState::Detail(_) => "j/k:Rule a:AddRule d:DeleteRule Esc:Back",
            ViewState::Create => "Esc:Cancel Tab:Next Enter:Submit",
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::context::{ActionReceiver, test_action_channel};
    use crate::models::neutron::SecurityGroupRule;

    fn key(code: KeyCode) -> KeyEvent {
        KeyEvent::from(code)
    }

    fn make_sg(id: &str, name: &str) -> SecurityGroup {
        SecurityGroup {
            id: id.into(),
            name: name.into(),
            description: Some("Test SG".into()),
            security_group_rules: vec![
                SecurityGroupRule {
                    id: format!("{id}-rule-1"),
                    direction: "ingress".into(),
                    protocol: Some("tcp".into()),
                    port_range_min: Some(22),
                    port_range_max: Some(22),
                    remote_ip_prefix: Some("0.0.0.0/0".into()),
                    remote_group_id: None,
                    ethertype: "IPv4".into(),
                },
                SecurityGroupRule {
                    id: format!("{id}-rule-2"),
                    direction: "egress".into(),
                    protocol: None,
                    port_range_min: None,
                    port_range_max: None,
                    remote_ip_prefix: None,
                    remote_group_id: None,
                    ethertype: "IPv4".into(),
                },
            ],
            tenant_id: None,
        }
    }

    fn setup() -> (SecurityGroupModule, ActionReceiver) {
        let (tx, rx) = test_action_channel();
        let mut module = SecurityGroupModule::new(tx);
        let sgs = vec![
            make_sg("sg-1", "default"),
            make_sg("sg-2", "web"),
            make_sg("sg-3", "db"),
        ];
        module.handle_event(&AppEvent::SecurityGroupsLoaded(sgs));
        (module, rx)
    }

    #[test]
    fn test_initial_state_is_list() {
        let (tx, _rx) = test_action_channel();
        let module = SecurityGroupModule::new(tx);
        assert_eq!(*module.view_state(), ViewState::List);
        assert!(module.security_groups().is_empty());
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
    fn test_handle_key_enter_to_detail() {
        let (mut module, _rx) = setup();
        module.handle_key(key(KeyCode::Enter));
        assert_eq!(*module.view_state(), ViewState::Detail("sg-1".into()));
    }

    #[test]
    fn test_handle_key_esc_detail_to_list() {
        let (mut module, _rx) = setup();
        module.handle_key(key(KeyCode::Enter));
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
    fn test_handle_key_d_delete_sg_confirm() {
        let (mut module, _rx) = setup();
        assert!(!module.confirm.is_active());
        module.handle_key(key(KeyCode::Char('d')));
        assert!(module.confirm.is_active());
    }

    #[test]
    fn test_confirm_delete_sg() {
        let (mut module, _rx) = setup();
        module.handle_key(key(KeyCode::Char('d')));
        let action = module.handle_key(key(KeyCode::Char('y')));
        assert!(matches!(action, Some(Action::DeleteSecurityGroup { .. })));
        assert!(!module.confirm.is_active());
    }

    #[test]
    fn test_detail_delete_rule_confirm() {
        let (mut module, _rx) = setup();
        module.handle_key(key(KeyCode::Enter)); // go to detail
        module.handle_key(key(KeyCode::Char('d'))); // delete rule
        assert!(module.confirm.is_active());
    }

    #[test]
    fn test_detail_confirm_delete_rule_action() {
        let (mut module, _rx) = setup();
        module.handle_key(key(KeyCode::Enter));
        module.handle_key(key(KeyCode::Char('d')));
        let action = module.handle_key(key(KeyCode::Char('y')));
        assert!(matches!(
            action,
            Some(Action::DeleteSecurityGroupRule { .. })
        ));
    }

    #[test]
    fn test_handle_key_r_fetches_sgs() {
        let (mut module, _rx) = setup();
        let action = module.handle_key(key(KeyCode::Char('r')));
        assert!(matches!(action, Some(Action::FetchSecurityGroups)));
    }

    #[test]
    fn test_handle_event_sgs_loaded() {
        let (tx, _rx) = test_action_channel();
        let mut module = SecurityGroupModule::new(tx);
        let sgs = vec![make_sg("sg-1", "test")];
        module.handle_event(&AppEvent::SecurityGroupsLoaded(sgs));
        assert_eq!(module.security_groups().len(), 1);
    }

    #[test]
    fn test_handle_event_sg_deleted_triggers_refresh() {
        let (mut module, mut rx) = setup();
        module.handle_event(&AppEvent::SecurityGroupDeleted { id: "sg-1".into() });
        let action = rx.try_recv().unwrap();
        assert!(matches!(action, Action::FetchSecurityGroups));
    }

    #[test]
    fn test_handle_event_rule_created_triggers_refresh() {
        let (mut module, mut rx) = setup();
        let rule = SecurityGroupRule {
            id: "rule-new".into(),
            direction: "ingress".into(),
            protocol: Some("tcp".into()),
            port_range_min: Some(443),
            port_range_max: Some(443),
            remote_ip_prefix: None,
            remote_group_id: None,
            ethertype: "IPv4".into(),
        };
        module.handle_event(&AppEvent::SecurityGroupRuleCreated(rule));
        let action = rx.try_recv().unwrap();
        assert!(matches!(action, Action::FetchSecurityGroups));
    }

    #[test]
    fn test_handle_event_api_error() {
        let (mut module, _rx) = setup();
        module.handle_event(&AppEvent::ApiError {
            operation: "delete".into(),
            message: "conflict".into(),
        });
        assert_eq!(module.error_message(), Some("delete: conflict"));
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
        assert_eq!(form.field_count(), 2);
        assert_eq!(form.focused_field_name(), "Name");
    }

    #[test]
    fn test_create_form_submit_produces_action() {
        let (mut module, mut rx) = setup();
        module.handle_key(key(KeyCode::Char('c')));

        // Type SG name
        for c in "my-sg".chars() {
            module.handle_key(key(KeyCode::Char(c)));
        }

        // Navigate to last field (Description) and trigger confirm
        module.handle_key(key(KeyCode::Down));
        module.handle_key(key(KeyCode::Enter)); // enters Confirming phase
        let action = module.handle_key(key(KeyCode::Enter)); // confirms submit

        // Submit now returns ExitFormMode; CreateSecurityGroup is sent via action_tx
        assert!(matches!(action, Some(Action::ExitFormMode)));
        assert_eq!(*module.view_state(), ViewState::List);
        assert!(module.form.is_none());

        // Verify CreateSecurityGroup was dispatched via channel
        let sent = rx.try_recv().unwrap();
        assert!(matches!(sent, Action::CreateSecurityGroup(_)));
    }

    #[test]
    fn test_help_hint_list() {
        let (module, _rx) = setup();
        assert_eq!(
            module.help_hint(),
            "Enter:Detail c:Create d:Delete r:Refresh"
        );
    }

    #[test]
    fn test_help_hint_detail() {
        let (mut module, _rx) = setup();
        module.handle_key(key(KeyCode::Enter));
        assert_eq!(
            module.help_hint(),
            "j/k:Rule a:AddRule d:DeleteRule Esc:Back"
        );
    }

    #[test]
    fn test_help_hint_create() {
        let (mut module, _rx) = setup();
        module.handle_key(key(KeyCode::Char('c')));
        assert_eq!(module.help_hint(), "Esc:Cancel Tab:Next Enter:Submit");
    }
}
