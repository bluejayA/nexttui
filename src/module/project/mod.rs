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
use crate::models::keystone::Project;
use crate::module::{ConfirmHandler, ListNav, PendingAction, ViewState};
use crate::ui::confirm::ConfirmDialog;
use crate::ui::resource_list::{ResourceList, Row};

use self::view_model::{project_columns, project_detail_data, project_to_row};

pub struct ProjectModule {
    view_state: ViewState,
    projects: Vec<Project>,
    nav: ListNav,
    #[allow(dead_code)]
    loading: bool,
    error_message: Option<String>,
    confirm: ConfirmHandler,
    resource_list: ResourceList,
    action_tx: mpsc::UnboundedSender<Action>,
}

impl ProjectModule {
    pub fn new(action_tx: mpsc::UnboundedSender<Action>) -> Self {
        Self {
            view_state: ViewState::List,
            projects: Vec::new(),
            nav: ListNav::new(),
            loading: false,
            error_message: None,
            confirm: ConfirmHandler::new(),
            resource_list: ResourceList::new(project_columns()),
            action_tx,
        }
    }

    pub fn view_state(&self) -> &ViewState { &self.view_state }
    pub fn projects(&self) -> &[Project] { &self.projects }
    pub fn selected_index(&self) -> usize { self.nav.selected_index }
    pub fn error_message(&self) -> Option<&str> { self.error_message.as_deref() }

    fn selected_project(&self) -> Option<&Project> {
        self.projects.get(self.nav.selected_index)
    }

    fn rows(&self) -> Vec<Row> {
        self.projects.iter().map(project_to_row).collect()
    }

    fn resolve_action(pending: PendingAction) -> Option<Action> {
        match pending {
            PendingAction::DeleteProject { id, .. } => Some(Action::DeleteProject { id }),
            _ => None,
        }
    }

    fn handle_list_key(&mut self, key: KeyEvent) -> Option<Action> {
        if self.nav.handle_key(key) { return None; }
        match key.code {
            KeyCode::Enter => {
                if let Some(proj) = self.selected_project() {
                    self.view_state = ViewState::Detail(proj.id.clone());
                }
                None
            }
            KeyCode::Char('c') => { self.view_state = ViewState::Create; None }
            KeyCode::Char('d') => {
                if let Some(proj) = self.selected_project() {
                    let id = proj.id.clone();
                    let name = proj.name.clone();
                    self.confirm.open(
                        ConfirmDialog::type_to_confirm(
                            format!("Delete project '{name}'?"),
                            name.clone(),
                        ),
                        PendingAction::DeleteProject { id, name },
                    );
                }
                None
            }
            KeyCode::Char('r') => Some(Action::FetchProjects),
            KeyCode::Esc => Some(Action::Back),
            _ => None,
        }
    }

    fn handle_detail_key(&mut self, key: KeyEvent) -> Option<Action> {
        match key.code {
            KeyCode::Esc | KeyCode::Char('q') => { self.view_state = ViewState::List; None }
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

impl Component for ProjectModule {
    fn handle_key(&mut self, key: KeyEvent) -> Option<Action> {
        if let Some(result) = self.confirm.handle_key(key, Self::resolve_action) { return result; }
        match &self.view_state {
            ViewState::List => self.handle_list_key(key),
            ViewState::Detail(_) => self.handle_detail_key(key),
            ViewState::Create => self.handle_create_key(key),
        }
    }

    fn handle_event(&mut self, event: &AppEvent) {
        match event {
            AppEvent::ProjectsLoaded(projects) => {
                self.projects = projects.clone();
                self.loading = false;
                self.error_message = None;
                self.nav.set_count(self.projects.len());
                let rows = self.rows();
                self.resource_list.set_rows(rows);
            }
            AppEvent::ProjectCreated(_) => {
                self.view_state = ViewState::List;
                let _ = self.action_tx.send(Action::FetchProjects);
            }
            AppEvent::ProjectDeleted { .. } => {
                let _ = self.action_tx.send(Action::FetchProjects);
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
            ViewState::Detail(id) => {
                if let Some(proj) = self.projects.iter().find(|p| p.id == *id) {
                    let data = project_detail_data(proj);
                    let mut dv = crate::ui::detail_view::DetailView::new();
                    dv.set_data(data);
                    dv.render(frame, area);
                }
            }
            ViewState::Create => {
                let text = Paragraph::new(vec![
                    Line::raw(""),
                    Line::raw("  Project Create Form (Esc to cancel)"),
                    Line::raw("  [Form integration pending]"),
                ]).style(Style::default().fg(Color::DarkGray));
                frame.render_widget(text, area);
            }
        }
        self.confirm.render(frame, area);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn key(code: KeyCode) -> KeyEvent { KeyEvent::from(code) }
    fn make_project(id: &str, name: &str) -> Project {
        Project { id: id.into(), name: name.into(), description: None, enabled: true, domain_id: Some("default".into()) }
    }
    fn setup() -> (ProjectModule, mpsc::UnboundedReceiver<Action>) {
        let (tx, rx) = mpsc::unbounded_channel();
        let mut m = ProjectModule::new(tx);
        m.handle_event(&AppEvent::ProjectsLoaded(vec![
            make_project("p1", "admin"), make_project("p2", "demo"),
        ]));
        (m, rx)
    }

    #[test] fn test_initial_state() { let (tx, _) = mpsc::unbounded_channel(); let m = ProjectModule::new(tx); assert_eq!(*m.view_state(), ViewState::List); }
    #[test] fn test_nav() { let (mut m, _) = setup(); m.handle_key(key(KeyCode::Char('j'))); assert_eq!(m.selected_index(), 1); }
    #[test] fn test_enter_detail() { let (mut m, _) = setup(); m.handle_key(key(KeyCode::Enter)); assert_eq!(*m.view_state(), ViewState::Detail("p1".into())); }
    #[test] fn test_esc_to_list() { let (mut m, _) = setup(); m.handle_key(key(KeyCode::Enter)); m.handle_key(key(KeyCode::Esc)); assert_eq!(*m.view_state(), ViewState::List); }
    #[test] fn test_create() { let (mut m, _) = setup(); m.handle_key(key(KeyCode::Char('c'))); assert_eq!(*m.view_state(), ViewState::Create); }
    #[test] fn test_delete_confirm() { let (mut m, _) = setup(); m.handle_key(key(KeyCode::Char('d'))); assert!(m.confirm.is_active()); }
    #[test] fn test_confirm_delete() {
        let (mut m, _) = setup();
        m.handle_key(key(KeyCode::Char('d')));
        for c in "admin".chars() { m.handle_key(key(KeyCode::Char(c))); }
        let a = m.handle_key(key(KeyCode::Enter));
        assert!(matches!(a, Some(Action::DeleteProject { .. })));
    }
    #[test] fn test_refresh() { let (mut m, _) = setup(); assert!(matches!(m.handle_key(key(KeyCode::Char('r'))), Some(Action::FetchProjects))); }
    #[test] fn test_event_loaded() {
        let (tx, _) = mpsc::unbounded_channel(); let mut m = ProjectModule::new(tx);
        m.handle_event(&AppEvent::ProjectsLoaded(vec![make_project("p1", "t")]));
        assert_eq!(m.projects().len(), 1);
    }
    #[test] fn test_event_created() {
        let (mut m, mut rx) = setup(); m.view_state = ViewState::Create;
        m.handle_event(&AppEvent::ProjectCreated(make_project("p3", "new")));
        assert_eq!(*m.view_state(), ViewState::List);
        assert!(matches!(rx.try_recv().unwrap(), Action::FetchProjects));
    }
    #[test] fn test_event_deleted() {
        let (mut m, mut rx) = setup();
        m.handle_event(&AppEvent::ProjectDeleted { id: "p1".into() });
        assert!(matches!(rx.try_recv().unwrap(), Action::FetchProjects));
    }
}
