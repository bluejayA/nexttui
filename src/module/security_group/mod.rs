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
use crate::models::neutron::SecurityGroup;
use crate::module::{ConfirmHandler, PendingAction, ViewState};
use crate::ui::confirm::ConfirmDialog;
use crate::ui::resource_list::{ResourceList, Row};

use self::view_model::{sg_columns, sg_detail_data, sg_to_row};

pub struct SecurityGroupModule {
    view_state: ViewState,
    security_groups: Vec<SecurityGroup>,
    rule_selected: usize,
    #[allow(dead_code)] // Phase 2: set to true on Action dispatch, render loading spinner
    loading: bool,
    error_message: Option<String>,
    confirm: ConfirmHandler,
    resource_list: ResourceList,
    action_tx: mpsc::UnboundedSender<Action>,
}

impl SecurityGroupModule {
    pub fn new(action_tx: mpsc::UnboundedSender<Action>) -> Self {
        Self {
            view_state: ViewState::List,
            security_groups: Vec::new(),
            rule_selected: 0,
            loading: false,
            error_message: None,
            confirm: ConfirmHandler::new(),
            resource_list: ResourceList::new(sg_columns()),
            action_tx,
        }
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
        self.security_groups.get(self.resource_list.selected_index())
    }

    fn current_sg(&self) -> Option<&SecurityGroup> {
        if let ViewState::Detail(ref id) = self.view_state {
            self.security_groups.iter().find(|sg| sg.id == *id)
        } else {
            None
        }
    }

    fn rows(&self) -> Vec<Row> {
        self.security_groups.iter().map(sg_to_row).collect()
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
                self.view_state = ViewState::Create;
                None
            }
            KeyCode::Char('d') => {
                if let Some(sg) = self.selected_sg() {
                    let id = sg.id.clone();
                    let name = sg.name.clone();
                    self.confirm.open(
                        ConfirmDialog::yes_no(format!("Delete security group '{name}'?")),
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
            KeyCode::Char('d') => {
                if let Some(sg) = self.current_sg() {
                    if let Some(rule) = sg
                        .security_group_rules
                        .get(self.rule_selected)
                    {
                        let rule_id = rule.id.clone();
                        let short_id = if rule_id.len() > 8 {
                            &rule_id[..8]
                        } else {
                            &rule_id
                        };
                        self.confirm.open(
                            ConfirmDialog::yes_no(format!("Delete rule {short_id}...?")),
                            PendingAction::DeleteSecurityGroupRule { rule_id },
                        );
                    }
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

impl Component for SecurityGroupModule {
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
                let text = Paragraph::new(vec![
                    Line::raw(""),
                    Line::raw("  Security Group Create Form (Tab/Enter, Esc to cancel)"),
                    Line::raw("  [Form integration pending]"),
                ])
                .style(Style::default().fg(Color::DarkGray));
                frame.render_widget(text, area);
            }
        }

        self.confirm.render(frame, area);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
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
        }
    }

    fn setup() -> (SecurityGroupModule, mpsc::UnboundedReceiver<Action>) {
        let (tx, rx) = mpsc::unbounded_channel();
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
        let (tx, _rx) = mpsc::unbounded_channel();
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
        let (tx, _rx) = mpsc::unbounded_channel();
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
}
