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
use crate::models::keystone::User;
use crate::module::{ConfirmHandler, PendingAction, ViewState};
use crate::ui::confirm::ConfirmDialog;
use crate::ui::resource_list::{ResourceList, Row};

use self::view_model::{user_columns, user_to_row};

pub struct UserModule {
    view_state: ViewState,
    users: Vec<User>,
    #[allow(dead_code)]
    loading: bool,
    error_message: Option<String>,
    confirm: ConfirmHandler,
    resource_list: ResourceList,
    action_tx: mpsc::UnboundedSender<Action>,
}

impl UserModule {
    pub fn new(action_tx: mpsc::UnboundedSender<Action>) -> Self {
        Self {
            view_state: ViewState::List,
            users: Vec::new(),
            loading: false,
            error_message: None,
            confirm: ConfirmHandler::new(),
            resource_list: ResourceList::new(user_columns()),
            action_tx,
        }
    }

    pub fn view_state(&self) -> &ViewState { &self.view_state }
    pub fn users(&self) -> &[User] { &self.users }
    pub fn selected_index(&self) -> usize { self.resource_list.selected_index() }
    pub fn error_message(&self) -> Option<&str> { self.error_message.as_deref() }

    fn selected_user(&self) -> Option<&User> { self.users.get(self.resource_list.selected_index()) }
    fn rows(&self) -> Vec<Row> { self.users.iter().map(user_to_row).collect() }

    fn resolve_action(pending: PendingAction) -> Option<Action> {
        match pending {
            PendingAction::DeleteUser { id, .. } => Some(Action::DeleteUser { id }),
            _ => None,
        }
    }

    fn handle_list_key(&mut self, key: KeyEvent) -> Option<Action> {
        if self.resource_list.handle_nav_key(key) { return None; }
        match key.code {
            KeyCode::Char('c') => { self.view_state = ViewState::Create; None }
            KeyCode::Char('d') => {
                if let Some(user) = self.selected_user() {
                    let id = user.id.clone();
                    let name = user.name.clone();
                    self.confirm.open(
                        ConfirmDialog::type_to_confirm(
                            format!("Delete user '{name}'?"),
                            name.clone(),
                        ),
                        PendingAction::DeleteUser { id, name },
                    );
                }
                None
            }
            KeyCode::Char('r') => Some(Action::FetchUsers),
            KeyCode::Left => Some(Action::FocusSidebar),
            KeyCode::Esc => Some(Action::Back),
            _ => None,
        }
    }

    fn handle_create_key(&mut self, key: KeyEvent) -> Option<Action> {
        match key.code {
            KeyCode::Esc => { self.view_state = ViewState::List; None }
            _ => None,
        }
    }
}

impl Component for UserModule {
    fn handle_key(&mut self, key: KeyEvent) -> Option<Action> {
        if let Some(result) = self.confirm.handle_key(key, Self::resolve_action) { return result; }
        match &self.view_state {
            ViewState::List => self.handle_list_key(key),
            ViewState::Create => self.handle_create_key(key),
            ViewState::Detail(_) => None,
        }
    }

    fn handle_event(&mut self, event: &AppEvent) {
        match event {
            AppEvent::UsersLoaded(users) => {
                self.users = users.clone();
                self.loading = false;
                self.error_message = None;
                let rows = self.rows();
                self.resource_list.set_rows(rows);
            }
            AppEvent::UserCreated(_) => {
                self.view_state = ViewState::List;
                let _ = self.action_tx.send(Action::FetchUsers);
            }
            AppEvent::UserDeleted { .. } => {
                let _ = self.action_tx.send(Action::FetchUsers);
            }
            AppEvent::ApiError { operation, message, .. } => {
                self.error_message = Some(format!("{operation}: {message}"));
                self.loading = false;
            }
            _ => {}
        }
    }

    fn render(&self, frame: &mut Frame, area: Rect) {
        match &self.view_state {
            ViewState::List => self.resource_list.render(frame, area),
            ViewState::Create => {
                let text = Paragraph::new(vec![
                    Line::raw(""),
                    Line::raw("  User Create Form (Esc to cancel)"),
                    Line::raw("  [Form integration pending]"),
                ]).style(Style::default().fg(Color::DarkGray));
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
    fn key(code: KeyCode) -> KeyEvent { KeyEvent::from(code) }
    fn make_user(id: &str, name: &str) -> User {
        User { id: id.into(), name: name.into(), email: None, enabled: true, default_project_id: None, domain_id: Some("default".into()) }
    }
    fn setup() -> (UserModule, mpsc::UnboundedReceiver<Action>) {
        let (tx, rx) = mpsc::unbounded_channel();
        let mut m = UserModule::new(tx);
        m.handle_event(&AppEvent::UsersLoaded(vec![make_user("u1", "admin"), make_user("u2", "demo")]));
        (m, rx)
    }

    #[test] fn test_initial_state() { let (tx, _) = mpsc::unbounded_channel(); let m = UserModule::new(tx); assert_eq!(*m.view_state(), ViewState::List); }
    #[test] fn test_nav() { let (mut m, _) = setup(); m.handle_key(key(KeyCode::Char('j'))); assert_eq!(m.selected_index(), 1); }
    #[test] fn test_create() { let (mut m, _) = setup(); m.handle_key(key(KeyCode::Char('c'))); assert_eq!(*m.view_state(), ViewState::Create); }
    #[test] fn test_delete_confirm() { let (mut m, _) = setup(); m.handle_key(key(KeyCode::Char('d'))); assert!(m.confirm.is_active()); }
    #[test] fn test_confirm_delete() {
        let (mut m, _) = setup(); m.handle_key(key(KeyCode::Char('d')));
        for c in "admin".chars() { m.handle_key(key(KeyCode::Char(c))); }
        assert!(matches!(m.handle_key(key(KeyCode::Enter)), Some(Action::DeleteUser { .. })));
    }
    #[test] fn test_refresh() { let (mut m, _) = setup(); assert!(matches!(m.handle_key(key(KeyCode::Char('r'))), Some(Action::FetchUsers))); }
    #[test] fn test_event_loaded() {
        let (tx, _) = mpsc::unbounded_channel(); let mut m = UserModule::new(tx);
        m.handle_event(&AppEvent::UsersLoaded(vec![make_user("u1", "t")]));
        assert_eq!(m.users().len(), 1);
    }
    #[test] fn test_event_created() {
        let (mut m, mut rx) = setup(); m.view_state = ViewState::Create;
        m.handle_event(&AppEvent::UserCreated(make_user("u3", "new")));
        assert_eq!(*m.view_state(), ViewState::List);
        assert!(matches!(rx.try_recv().unwrap(), Action::FetchUsers));
    }
    #[test] fn test_event_deleted() {
        let (mut m, mut rx) = setup();
        m.handle_event(&AppEvent::UserDeleted { id: "u1".into() });
        assert!(matches!(rx.try_recv().unwrap(), Action::FetchUsers));
    }
}
