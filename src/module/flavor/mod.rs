pub mod view_model;

use crossterm::event::{KeyCode, KeyEvent};
use ratatui::Frame;
use ratatui::layout::Rect;

use crate::action::Action;
use crate::component::Component;
use crate::context::ActionSender;
use crate::event::AppEvent;
use crate::models::nova::Flavor;
use crate::module::{ConfirmHandler, PendingAction, ViewState};
use crate::port::types::FlavorCreateParams;
use crate::ui::confirm::ConfirmDialog;
use crate::ui::form::{FormAction, FormWidget};
use crate::ui::resource_list::{ResourceList, Row};

use self::view_model::{flavor_columns, flavor_create_defs, flavor_to_row};

pub struct FlavorModule {
    view_state: ViewState,
    flavors: Vec<Flavor>,
    loading: bool,
    error_message: Option<String>,
    is_admin: bool,
    confirm: ConfirmHandler,
    resource_list: ResourceList,
    form: Option<FormWidget>,
    action_tx: ActionSender,
}

impl FlavorModule {
    pub fn new(action_tx: ActionSender) -> Self {
        Self {
            view_state: ViewState::List,
            flavors: Vec::new(),
            loading: false,
            error_message: None,
            is_admin: false,
            confirm: ConfirmHandler::new(),
            resource_list: ResourceList::new(flavor_columns()),
            form: None,
            action_tx,
        }
    }

    pub fn view_state(&self) -> &ViewState {
        &self.view_state
    }

    pub fn flavors(&self) -> &[Flavor] {
        &self.flavors
    }

    pub fn selected_index(&self) -> usize {
        self.resource_list.selected_index()
    }

    fn selected_flavor(&self) -> Option<&Flavor> {
        self.flavors.get(self.resource_list.selected_index())
    }

    fn rows(&self) -> Vec<Row> {
        self.flavors.iter().map(flavor_to_row).collect()
    }

    fn resolve_action(pending: PendingAction) -> Option<Action> {
        match pending {
            PendingAction::Delete { id, .. } => Some(Action::DeleteFlavor { id }),
            _ => None,
        }
    }

    fn open_create_form(&mut self) {
        let defs = flavor_create_defs();
        self.form = Some(FormWidget::new("Create Flavor", defs));
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
            KeyCode::Char('c') if self.is_admin => {
                self.open_create_form();
                Some(Action::EnterFormMode)
            }
            KeyCode::Char('d') if self.is_admin => {
                if let Some(flavor) = self.selected_flavor() {
                    let id = flavor.id.clone();
                    let name = flavor.name.clone();
                    self.confirm.open(
                        ConfirmDialog::yes_no(format!("Delete flavor '{name}'?")),
                        PendingAction::Delete {
                            id,
                            name: name.clone(),
                        },
                    );
                }
                None
            }
            KeyCode::Char('r') => Some(Action::FetchFlavors),
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
                let name = values
                    .get("Name")
                    .and_then(|v| match v {
                        crate::ui::form::FormValue::Text(s) => Some(s.clone()),
                        _ => None,
                    })
                    .unwrap_or_default();
                let vcpus = values
                    .get("vCPU")
                    .and_then(|v| match v {
                        crate::ui::form::FormValue::Text(s) => s.parse::<u32>().ok(),
                        _ => None,
                    })
                    .unwrap_or(1);
                let ram_mb = values
                    .get("RAM (MB)")
                    .and_then(|v| match v {
                        crate::ui::form::FormValue::Text(s) => s.parse::<u32>().ok(),
                        _ => None,
                    })
                    .unwrap_or(512);
                let disk_gb = values
                    .get("Disk (GB)")
                    .and_then(|v| match v {
                        crate::ui::form::FormValue::Text(s) => s.parse::<u32>().ok(),
                        _ => None,
                    })
                    .unwrap_or(10);
                let is_public = values
                    .get("Public")
                    .and_then(|v| match v {
                        crate::ui::form::FormValue::Bool(b) => Some(*b),
                        _ => None,
                    })
                    .unwrap_or(true);

                self.close_form();
                let _ = self
                    .action_tx
                    .send(Action::CreateFlavor(FlavorCreateParams {
                        name,
                        vcpus,
                        ram_mb,
                        disk_gb,
                        is_public,
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

impl Component for FlavorModule {
    fn refresh_action(&self) -> Option<Action> {
        Some(Action::FetchFlavors)
    }
    fn is_modal(&self) -> bool {
        self.confirm.is_active() || self.form.is_some()
    }

    fn set_admin(&mut self, is_admin: bool) {
        self.is_admin = is_admin;
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
            AppEvent::FlavorsLoaded(flavors) => {
                self.flavors = flavors.clone();
                self.loading = false;
                self.error_message = None;
                let rows = self.rows();
                self.resource_list.set_rows(rows);
            }
            AppEvent::FlavorCreated(_) => {
                self.close_form();
                let _ = self.action_tx.send(Action::FetchFlavors);
            }
            AppEvent::FlavorDeleted { .. } => {
                let _ = self.action_tx.send(Action::FetchFlavors);
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

    fn content_title(&self) -> Option<String> {
        match &self.view_state {
            ViewState::List => None,
            ViewState::Detail(id) => {
                let name = self
                    .flavors
                    .iter()
                    .find(|r| r.id == *id)
                    .map(|r| r.name.as_str())
                    .unwrap_or("...");
                Some(format!("Flavor: {name}"))
            }
            ViewState::Create => Some("Create Flavor".into()),
        }
    }

    fn help_hint(&self) -> &str {
        match &self.view_state {
            ViewState::List if self.is_admin => "c:Create d:Delete r:Refresh",
            ViewState::List => "r:Refresh",
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

    fn make_flavor(id: &str, name: &str) -> Flavor {
        Flavor {
            id: id.into(),
            name: name.into(),
            vcpus: 2,
            ram: 4096,
            disk: 40,
            is_public: true,
        }
    }

    fn setup(is_admin: bool) -> (FlavorModule, ActionReceiver) {
        let (tx, rx) = test_action_channel();
        let mut module = FlavorModule::new(tx);
        module.set_admin(is_admin);
        let flavors = vec![
            make_flavor("f1", "m1.small"),
            make_flavor("f2", "m1.medium"),
            make_flavor("f3", "m1.large"),
        ];
        module.handle_event(&AppEvent::FlavorsLoaded(flavors));
        (module, rx)
    }

    #[test]
    fn test_initial_state_is_list() {
        let (tx, _rx) = test_action_channel();
        let module = FlavorModule::new(tx);
        assert_eq!(*module.view_state(), ViewState::List);
        assert!(module.flavors().is_empty());
    }

    #[test]
    fn test_handle_key_navigation() {
        let (mut module, _rx) = setup(false);
        assert_eq!(module.selected_index(), 0);

        module.handle_key(key(KeyCode::Char('j')));
        assert_eq!(module.selected_index(), 1);

        module.handle_key(key(KeyCode::Char('k')));
        assert_eq!(module.selected_index(), 0);
    }

    #[test]
    fn test_handle_key_c_admin_only() {
        let (mut module, _rx) = setup(false);
        module.handle_key(key(KeyCode::Char('c')));
        assert_eq!(*module.view_state(), ViewState::List);
        assert!(module.form.is_none());

        let (mut module, _rx) = setup(true);
        module.handle_key(key(KeyCode::Char('c')));
        assert_eq!(*module.view_state(), ViewState::Create);
        assert!(module.form.is_some());
    }

    #[test]
    fn test_handle_key_d_admin_only() {
        let (mut module, _rx) = setup(false);
        module.handle_key(key(KeyCode::Char('d')));
        assert!(!module.confirm.is_active());

        let (mut module, _rx) = setup(true);
        module.handle_key(key(KeyCode::Char('d')));
        assert!(module.confirm.is_active());
    }

    #[test]
    fn test_confirm_delete_flavor() {
        let (mut module, _rx) = setup(true);
        module.handle_key(key(KeyCode::Char('d')));
        let action = module.handle_key(key(KeyCode::Char('y')));
        assert!(matches!(action, Some(Action::DeleteFlavor { .. })));
    }

    #[test]
    fn test_handle_event_flavors_loaded() {
        let (tx, _rx) = test_action_channel();
        let mut module = FlavorModule::new(tx);
        assert!(module.flavors().is_empty());

        let flavors = vec![make_flavor("f1", "test")];
        module.handle_event(&AppEvent::FlavorsLoaded(flavors));
        assert_eq!(module.flavors().len(), 1);
    }

    #[test]
    fn test_handle_event_flavor_deleted_triggers_refresh() {
        let (mut module, mut rx) = setup(true);
        module.handle_event(&AppEvent::FlavorDeleted { id: "f1".into() });
        let action = rx.try_recv().unwrap();
        assert!(matches!(action, Action::FetchFlavors));
    }

    #[test]
    fn test_handle_key_r_fetches_flavors() {
        let (mut module, _rx) = setup(false);
        let action = module.handle_key(key(KeyCode::Char('r')));
        assert!(matches!(action, Some(Action::FetchFlavors)));
    }

    // -- Form integration tests ---------------------------------------------

    #[test]
    fn test_create_form_cancel_returns_to_list() {
        let (mut module, _rx) = setup(true);
        module.handle_key(key(KeyCode::Char('c')));
        assert_eq!(*module.view_state(), ViewState::Create);

        module.handle_key(key(KeyCode::Esc));
        assert_eq!(*module.view_state(), ViewState::List);
        assert!(module.form.is_none());
    }

    #[test]
    fn test_create_form_has_expected_fields() {
        let (mut module, _rx) = setup(true);
        module.handle_key(key(KeyCode::Char('c')));
        let form = module.form.as_ref().unwrap();
        assert_eq!(form.field_count(), 5);
        assert_eq!(form.focused_field_name(), "Name");
    }

    #[test]
    fn test_set_admin_via_trait_dispatch() {
        let (tx, _rx) = test_action_channel();
        let mut module = FlavorModule::new(tx);
        // Call via trait object — same path as App::broadcast_admin
        let component: &mut dyn Component = &mut module;
        component.set_admin(true);
        // Verify the trait dispatch actually reached FlavorModule's override
        assert!(module.is_admin);
    }

    #[test]
    fn test_help_hint_list_admin() {
        let (module, _rx) = setup(true);
        assert_eq!(module.help_hint(), "c:Create d:Delete r:Refresh");
    }

    #[test]
    fn test_help_hint_list_non_admin() {
        let (module, _rx) = setup(false);
        assert_eq!(module.help_hint(), "r:Refresh");
    }

    #[test]
    fn test_help_hint_create() {
        let (mut module, _rx) = setup(true);
        module.handle_key(key(KeyCode::Char('c')));
        assert_eq!(module.help_hint(), "Esc:Cancel Tab:Next Enter:Submit");
    }
}
