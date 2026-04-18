pub mod view_model;

use crossterm::event::{KeyCode, KeyEvent};
use ratatui::Frame;
use ratatui::layout::Rect;

use crate::action::Action;
use crate::component::Component;
use crate::context::ActionSender;
use crate::event::AppEvent;
use crate::models::keystone::Project;
use crate::module::{ConfirmHandler, PendingAction, ViewState};
use crate::port::types::ProjectCreateParams;
use crate::ui::confirm::ConfirmDialog;
use crate::ui::form::{FormAction, FormWidget, SelectOption};
use crate::ui::resource_list::{ResourceList, Row};

use self::view_model::{project_columns, project_create_defs, project_detail_data, project_to_row};

pub struct ProjectModule {
    view_state: ViewState,
    projects: Vec<Project>,
    #[allow(dead_code)]
    loading: bool,
    error_message: Option<String>,
    confirm: ConfirmHandler,
    resource_list: ResourceList,
    form: Option<FormWidget>,
    cached_domain_opts: Vec<SelectOption>,
    action_tx: ActionSender,
    context_target: Option<crate::context::types::ContextTarget>,
    context_recently_switched: bool,
}

impl ProjectModule {
    pub fn new(action_tx: ActionSender) -> Self {
        Self {
            view_state: ViewState::List,
            projects: Vec::new(),
            loading: false,
            error_message: None,
            confirm: ConfirmHandler::new(),
            resource_list: ResourceList::new(project_columns()),
            form: None,
            cached_domain_opts: Vec::new(),
            action_tx,
            context_target: None,
            context_recently_switched: false,
        }
    }

    fn destructive_confirm_typed(
        &self,
        message: impl Into<String>,
        expected: impl Into<String>,
    ) -> ConfirmDialog {
        ConfirmDialog::for_destructive_typed_opt(message, expected, self.context_target.as_ref())
    }

    pub fn view_state(&self) -> &ViewState {
        &self.view_state
    }
    pub fn projects(&self) -> &[Project] {
        &self.projects
    }
    pub fn selected_index(&self) -> usize {
        self.resource_list.selected_index()
    }
    pub fn error_message(&self) -> Option<&str> {
        self.error_message.as_deref()
    }

    fn selected_project(&self) -> Option<&Project> {
        self.projects.get(self.resource_list.selected_index())
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

    fn open_create_form(&mut self) {
        let defs = project_create_defs();
        let mut form = FormWidget::new("Create Project", defs);
        if !self.cached_domain_opts.is_empty() {
            form.set_field_options("Domain", self.cached_domain_opts.clone());
        }
        self.form = Some(form);
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
                if let Some(proj) = self.selected_project() {
                    self.view_state = ViewState::Detail(proj.id.clone());
                }
                None
            }
            KeyCode::Char('c') => {
                self.open_create_form();
                Some(Action::EnterFormMode)
            }
            KeyCode::Char('d') => {
                if let Some(proj) = self.selected_project() {
                    let id = proj.id.clone();
                    let name = proj.name.clone();
                    self.confirm.open(
                        self.destructive_confirm_typed(
                            format!("Delete project '{name}'?"),
                            name.clone(),
                        ),
                        PendingAction::DeleteProject { id, name },
                    );
                }
                None
            }
            KeyCode::Char('r') => Some(Action::FetchProjects),
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
                let domain_id = values
                    .get("Domain")
                    .and_then(|v| match v {
                        crate::ui::form::FormValue::Text(s) => Some(s.clone()),
                        _ => None,
                    })
                    .unwrap_or_else(|| "default".to_string());
                let enabled = values.get("Enabled").and_then(|v| match v {
                    crate::ui::form::FormValue::Bool(b) => Some(*b),
                    _ => None,
                });

                self.close_form();
                let _ = self
                    .action_tx
                    .send(Action::CreateProject(ProjectCreateParams {
                        name,
                        description,
                        domain_id,
                        enabled,
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

impl Component for ProjectModule {
    fn refresh_action(&self) -> Option<Action> {
        Some(Action::FetchProjects)
    }
    fn is_modal(&self) -> bool {
        self.confirm.is_active() || self.form.is_some()
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
        self.projects.clear();
        self.loading = true;
        self.error_message = None;
        self.resource_list.set_rows(Vec::new());
        self.view_state = ViewState::List;
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
            AppEvent::ProjectsLoaded(projects) => {
                self.projects = projects.clone();
                self.loading = false;
                self.error_message = None;
                let rows = self.rows();
                self.resource_list.set_rows(rows);
                // Build domain dropdown options from loaded projects
                let mut domain_ids: Vec<String> = projects
                    .iter()
                    .filter_map(|p| p.domain_id.clone())
                    .collect();
                domain_ids.sort();
                domain_ids.dedup();
                self.cached_domain_opts = domain_ids
                    .into_iter()
                    .map(|d| SelectOption {
                        value: d.clone(),
                        display: d,
                    })
                    .collect();
            }
            AppEvent::ProjectCreated(_) => {
                self.view_state = ViewState::List;
                let _ = self.action_tx.send(Action::FetchProjects);
            }
            AppEvent::ProjectDeleted { .. } => {
                let _ = self.action_tx.send(Action::FetchProjects);
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
                    .projects
                    .iter()
                    .find(|r| r.id == *id)
                    .map(|r| r.name.as_str())
                    .unwrap_or("...");
                Some(format!("Project: {name}"))
            }
            ViewState::Create => Some("Create Project".into()),
        }
    }

    fn help_hint(&self) -> &str {
        match &self.view_state {
            ViewState::List => "Enter:Detail c:Create d:Delete r:Refresh",
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
    fn make_project(id: &str, name: &str) -> Project {
        Project {
            id: id.into(),
            name: name.into(),
            description: None,
            enabled: true,
            domain_id: Some("default".into()),
        }
    }
    fn setup() -> (ProjectModule, ActionReceiver) {
        let (tx, rx) = test_action_channel();
        let mut m = ProjectModule::new(tx);
        m.handle_event(&AppEvent::ProjectsLoaded(vec![
            make_project("p1", "admin"),
            make_project("p2", "demo"),
        ]));
        (m, rx)
    }

    #[test]
    fn test_initial_state() {
        let (tx, _) = test_action_channel();
        let m = ProjectModule::new(tx);
        assert_eq!(*m.view_state(), ViewState::List);
    }
    #[test]
    fn test_nav() {
        let (mut m, _) = setup();
        m.handle_key(key(KeyCode::Char('j')));
        assert_eq!(m.selected_index(), 1);
    }
    #[test]
    fn test_enter_detail() {
        let (mut m, _) = setup();
        m.handle_key(key(KeyCode::Enter));
        assert_eq!(*m.view_state(), ViewState::Detail("p1".into()));
    }
    #[test]
    fn test_esc_to_list() {
        let (mut m, _) = setup();
        m.handle_key(key(KeyCode::Enter));
        m.handle_key(key(KeyCode::Esc));
        assert_eq!(*m.view_state(), ViewState::List);
    }
    #[test]
    fn test_create() {
        let (mut m, _) = setup();
        m.handle_key(key(KeyCode::Char('c')));
        assert_eq!(*m.view_state(), ViewState::Create);
        assert!(m.form.is_some());
    }
    #[test]
    fn test_delete_confirm() {
        let (mut m, _) = setup();
        m.handle_key(key(KeyCode::Char('d')));
        assert!(m.confirm.is_active());
    }
    #[test]
    fn test_confirm_delete() {
        let (mut m, _) = setup();
        m.handle_key(key(KeyCode::Char('d')));
        for c in "admin".chars() {
            m.handle_key(key(KeyCode::Char(c)));
        }
        let a = m.handle_key(key(KeyCode::Enter));
        assert!(matches!(a, Some(Action::DeleteProject { .. })));
    }
    #[test]
    fn test_refresh() {
        let (mut m, _) = setup();
        assert!(matches!(
            m.handle_key(key(KeyCode::Char('r'))),
            Some(Action::FetchProjects)
        ));
    }
    #[test]
    fn test_event_loaded() {
        let (tx, _) = test_action_channel();
        let mut m = ProjectModule::new(tx);
        m.handle_event(&AppEvent::ProjectsLoaded(vec![make_project("p1", "t")]));
        assert_eq!(m.projects().len(), 1);
    }
    #[test]
    fn test_event_created() {
        let (mut m, mut rx) = setup();
        m.view_state = ViewState::Create;
        m.handle_event(&AppEvent::ProjectCreated(make_project("p3", "new")));
        assert_eq!(*m.view_state(), ViewState::List);
        assert!(matches!(rx.try_recv().unwrap(), Action::FetchProjects));
    }
    #[test]
    fn test_event_deleted() {
        let (mut m, mut rx) = setup();
        m.handle_event(&AppEvent::ProjectDeleted { id: "p1".into() });
        assert!(matches!(rx.try_recv().unwrap(), Action::FetchProjects));
    }

    // -- Form integration tests -----------------------------------------------

    #[test]
    fn test_create_form_cancel_returns_to_list() {
        let (mut m, _) = setup();
        m.handle_key(key(KeyCode::Char('c')));
        assert_eq!(*m.view_state(), ViewState::Create);
        m.handle_key(key(KeyCode::Esc));
        assert_eq!(*m.view_state(), ViewState::List);
        assert!(m.form.is_none());
    }

    #[test]
    fn test_create_form_has_expected_fields() {
        let (mut m, _) = setup();
        m.handle_key(key(KeyCode::Char('c')));
        let form = m.form.as_ref().unwrap();
        assert_eq!(form.field_count(), 4);
        assert_eq!(form.focused_field_name(), "Name");
    }

    #[test]
    fn test_create_form_submit_blocked_by_validation() {
        let (mut m, _) = setup();
        m.handle_key(key(KeyCode::Char('c')));

        // Type project name
        for c in "test-proj".chars() {
            m.handle_key(key(KeyCode::Char(c)));
        }

        // Skip Description (Down), go to Domain ID — leave empty
        m.handle_key(key(KeyCode::Down));
        m.handle_key(key(KeyCode::Down));

        // Try to submit with Enter on Domain ID (required field is empty)
        // Domain ID is a text field, but it's not the last field, so Enter shouldn't submit
        let action = m.handle_key(key(KeyCode::Enter));
        assert!(action.is_none());
        // Still in create mode
        assert_eq!(*m.view_state(), ViewState::Create);
    }

    #[test]
    fn test_help_hint_list() {
        let (m, _) = setup();
        assert_eq!(m.help_hint(), "Enter:Detail c:Create d:Delete r:Refresh");
    }

    #[test]
    fn test_help_hint_detail() {
        let (mut m, _) = setup();
        m.handle_key(key(KeyCode::Enter));
        assert_eq!(m.help_hint(), "Esc:Back");
    }

    #[test]
    fn test_help_hint_create() {
        let (mut m, _) = setup();
        m.handle_key(key(KeyCode::Char('c')));
        assert_eq!(m.help_hint(), "Esc:Cancel Tab:Next Enter:Submit");
    }
}
