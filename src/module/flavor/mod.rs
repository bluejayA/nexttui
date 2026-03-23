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
use crate::models::nova::Flavor;
use crate::module::{ConfirmHandler, ListNav, PendingAction, ViewState};
use crate::ui::confirm::ConfirmDialog;
use crate::ui::resource_list::{ResourceList, Row};

use self::view_model::{flavor_columns, flavor_to_row};

pub struct FlavorModule {
    view_state: ViewState,
    flavors: Vec<Flavor>,
    nav: ListNav,
    loading: bool,
    error_message: Option<String>,
    is_admin: bool,
    confirm: ConfirmHandler,
    resource_list: ResourceList,
    action_tx: mpsc::UnboundedSender<Action>,
}

impl FlavorModule {
    pub fn new(action_tx: mpsc::UnboundedSender<Action>, is_admin: bool) -> Self {
        Self {
            view_state: ViewState::List,
            flavors: Vec::new(),
            nav: ListNav::new(),
            loading: false,
            error_message: None,
            is_admin,
            confirm: ConfirmHandler::new(),
            resource_list: ResourceList::new(flavor_columns()),
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
        self.nav.selected_index
    }

    pub fn set_admin(&mut self, is_admin: bool) {
        self.is_admin = is_admin;
    }

    fn selected_flavor(&self) -> Option<&Flavor> {
        self.flavors.get(self.nav.selected_index)
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

    fn handle_list_key(&mut self, key: KeyEvent) -> Option<Action> {
        if self.nav.handle_key(key) {
            return None;
        }

        match key.code {
            KeyCode::Char('c') if self.is_admin => {
                self.view_state = ViewState::Create;
                None
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
            KeyCode::Esc => Some(Action::Back),
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

impl Component for FlavorModule {
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
                self.nav.set_count(self.flavors.len());
                let rows = self.rows();
                self.resource_list.set_rows(rows);
            }
            AppEvent::FlavorCreated(_) | AppEvent::FlavorDeleted { .. } => {
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
                let text = Paragraph::new(vec![
                    Line::raw(""),
                    Line::raw("  Flavor Create Form (Tab/Enter to submit, Esc to cancel)"),
                    Line::raw("  [Form integration pending]"),
                ])
                .style(Style::default().fg(Color::DarkGray));
                frame.render_widget(text, area);
            }
            ViewState::Detail(_) => {}
        }

        self.confirm.render(frame, area);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

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

    fn setup(is_admin: bool) -> (FlavorModule, mpsc::UnboundedReceiver<Action>) {
        let (tx, rx) = mpsc::unbounded_channel();
        let mut module = FlavorModule::new(tx, is_admin);
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
        let (tx, _rx) = mpsc::unbounded_channel();
        let module = FlavorModule::new(tx, false);
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

        let (mut module, _rx) = setup(true);
        module.handle_key(key(KeyCode::Char('c')));
        assert_eq!(*module.view_state(), ViewState::Create);
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
        let (tx, _rx) = mpsc::unbounded_channel();
        let mut module = FlavorModule::new(tx, false);
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
}
