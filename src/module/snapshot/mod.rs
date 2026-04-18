pub mod view_model;

use crossterm::event::{KeyCode, KeyEvent};
use ratatui::Frame;
use ratatui::layout::Rect;

use crate::action::Action;
use crate::component::Component;
use crate::context::ActionSender;
use crate::event::AppEvent;
use crate::models::cinder::VolumeSnapshot;
use crate::module::{ConfirmHandler, PendingAction, ViewState};
use crate::ui::confirm::ConfirmDialog;
use crate::ui::resource_list::{ResourceList, Row};

use self::view_model::{snapshot_columns, snapshot_detail_data, snapshot_to_row};

pub struct SnapshotModule {
    view_state: ViewState,
    snapshots: Vec<VolumeSnapshot>,
    #[allow(dead_code)] // Phase 2: loading spinner
    loading: bool,
    error_message: Option<String>,
    confirm: ConfirmHandler,
    resource_list: ResourceList,
    all_tenants: bool,
    action_tx: ActionSender,
    context_target: Option<crate::context::types::ContextTarget>,
    context_recently_switched: bool,
}

impl SnapshotModule {
    pub fn new(action_tx: ActionSender) -> Self {
        Self {
            view_state: ViewState::List,
            snapshots: Vec::new(),
            loading: false,
            error_message: None,
            confirm: ConfirmHandler::new(),
            resource_list: ResourceList::new(snapshot_columns(false)),
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

    pub fn snapshots(&self) -> &[VolumeSnapshot] {
        &self.snapshots
    }

    pub fn selected_index(&self) -> usize {
        self.resource_list.selected_index()
    }

    pub fn error_message(&self) -> Option<&str> {
        self.error_message.as_deref()
    }

    fn selected_snapshot(&self) -> Option<&VolumeSnapshot> {
        self.snapshots.get(self.resource_list.selected_index())
    }

    fn rows(&self) -> Vec<Row> {
        self.snapshots
            .iter()
            .map(|s| snapshot_to_row(s, self.all_tenants))
            .collect()
    }

    fn resolve_action(pending: PendingAction) -> Option<Action> {
        match pending {
            PendingAction::DeleteSnapshot { id, .. } => Some(Action::DeleteSnapshot { id }),
            _ => None,
        }
    }

    fn handle_list_key(&mut self, key: KeyEvent) -> Option<Action> {
        if self.resource_list.handle_nav_key(key) {
            return None;
        }

        match key.code {
            KeyCode::Enter => {
                if let Some(snap) = self.selected_snapshot() {
                    let id = snap.id.clone();
                    self.view_state = ViewState::Detail(id);
                }
                None
            }
            KeyCode::Char('d') => {
                if let Some(snap) = self.selected_snapshot() {
                    let id = snap.id.clone();
                    let name = snap
                        .name
                        .clone()
                        .unwrap_or_else(|| id.chars().take(8).collect());
                    self.confirm.open(
                        self.destructive_confirm(format!("Delete snapshot '{name}'?")),
                        PendingAction::DeleteSnapshot { id, name },
                    );
                }
                None
            }
            KeyCode::Char('r') => Some(Action::FetchSnapshots),
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
}

impl Component for SnapshotModule {
    fn refresh_action(&self) -> Option<Action> {
        Some(Action::FetchSnapshots)
    }
    fn is_modal(&self) -> bool {
        self.confirm.is_active()
    }

    fn set_all_tenants(&mut self, v: bool) {
        self.all_tenants = v;
        self.resource_list = ResourceList::new(snapshot_columns(v));
    }

    fn handle_key(&mut self, key: KeyEvent) -> Option<Action> {
        if let Some(result) = self.confirm.handle_key(key, Self::resolve_action) {
            return result;
        }

        match &self.view_state {
            ViewState::List => self.handle_list_key(key),
            ViewState::Detail(_) => self.handle_detail_key(key),
            ViewState::Create => None, // No create view for snapshots in Phase 1
        }
    }

    fn on_context_changed(&mut self) {
        self.snapshots.clear();
        self.loading = true;
        self.error_message = None;
        self.resource_list.set_rows(Vec::new());
        self.view_state = ViewState::List;
        // Codex review 2차 P1: pending destructive confirm must not survive
        // across a context switch.
        self.confirm = ConfirmHandler::new();
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
            AppEvent::SnapshotsLoaded(snapshots) => {
                self.snapshots = snapshots.clone();
                self.loading = false;
                self.error_message = None;
                let rows = self.rows();
                self.resource_list.set_rows(rows);
            }
            AppEvent::SnapshotCreated(_) | AppEvent::SnapshotDeleted { .. } => {
                let _ = self.action_tx.send(Action::FetchSnapshots);
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
                if let Some(snap) = self.snapshots.iter().find(|s| s.id == *id) {
                    let data = snapshot_detail_data(snap);
                    let mut dv = crate::ui::detail_view::DetailView::new();
                    dv.set_data(data);
                    dv.render(frame, area);
                }
            }
            ViewState::Create => {}
        }

        self.confirm.render(frame, area);
    }

    fn content_title(&self) -> Option<String> {
        match &self.view_state {
            ViewState::List => None,
            ViewState::Detail(id) => {
                let name = self
                    .snapshots
                    .iter()
                    .find(|r| r.id == *id)
                    .and_then(|r| r.name.as_deref())
                    .unwrap_or("...");
                Some(format!("Snapshot: {name}"))
            }
            ViewState::Create => None,
        }
    }

    fn help_hint(&self) -> &str {
        match &self.view_state {
            ViewState::List => "Enter:Detail d:Delete r:Refresh",
            ViewState::Detail(_) => "Esc:Back",
            ViewState::Create => "",
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

    fn make_snapshot(id: &str, name: &str, status: &str) -> VolumeSnapshot {
        VolumeSnapshot {
            id: id.into(),
            name: Some(name.into()),
            status: status.into(),
            size: 100,
            volume_id: "vol-1".into(),
            created_at: Some("2026-01-15T00:00:00Z".into()),
            tenant_id: None,
        }
    }

    fn setup() -> (SnapshotModule, ActionReceiver) {
        let (tx, rx) = test_action_channel();
        let mut module = SnapshotModule::new(tx);
        let snaps = vec![
            make_snapshot("snap-1", "daily", "available"),
            make_snapshot("snap-2", "weekly", "available"),
            make_snapshot("snap-3", "failed", "error"),
        ];
        module.handle_event(&AppEvent::SnapshotsLoaded(snaps));
        (module, rx)
    }

    #[test]
    fn test_initial_state_is_list() {
        let (tx, _rx) = test_action_channel();
        let module = SnapshotModule::new(tx);
        assert_eq!(*module.view_state(), ViewState::List);
        assert!(module.snapshots().is_empty());
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
        assert_eq!(*module.view_state(), ViewState::Detail("snap-1".into()));
    }

    #[test]
    fn test_handle_key_esc_detail_to_list() {
        let (mut module, _rx) = setup();
        module.handle_key(key(KeyCode::Enter));
        module.handle_key(key(KeyCode::Esc));
        assert_eq!(*module.view_state(), ViewState::List);
    }

    #[test]
    fn test_handle_key_d_delete_confirm() {
        let (mut module, _rx) = setup();
        assert!(!module.confirm.is_active());
        module.handle_key(key(KeyCode::Char('d')));
        assert!(module.confirm.is_active());
    }

    #[test]
    fn test_confirm_delete_snapshot() {
        let (mut module, _rx) = setup();
        module.handle_key(key(KeyCode::Char('d')));
        let action = module.handle_key(key(KeyCode::Char('y')));
        assert!(matches!(action, Some(Action::DeleteSnapshot { .. })));
        assert!(!module.confirm.is_active());
    }

    #[test]
    fn test_handle_key_r_fetches_snapshots() {
        let (mut module, _rx) = setup();
        let action = module.handle_key(key(KeyCode::Char('r')));
        assert!(matches!(action, Some(Action::FetchSnapshots)));
    }

    #[test]
    fn test_handle_event_snapshots_loaded() {
        let (tx, _rx) = test_action_channel();
        let mut module = SnapshotModule::new(tx);
        let snaps = vec![make_snapshot("snap-1", "test", "available")];
        module.handle_event(&AppEvent::SnapshotsLoaded(snaps));
        assert_eq!(module.snapshots().len(), 1);
    }

    #[test]
    fn test_handle_event_snapshot_deleted_triggers_refresh() {
        let (mut module, mut rx) = setup();
        module.handle_event(&AppEvent::SnapshotDeleted {
            id: "snap-1".into(),
        });
        let action = rx.try_recv().unwrap();
        assert!(matches!(action, Action::FetchSnapshots));
    }

    #[test]
    fn test_handle_event_api_error() {
        let (mut module, _rx) = setup();
        module.handle_event(&AppEvent::ApiError {
            operation: "delete".into(),
            message: "in-use".into(),
        });
        assert_eq!(module.error_message(), Some("delete: in-use"));
    }

    #[test]
    fn test_help_hint_list() {
        let (module, _rx) = setup();
        assert_eq!(module.help_hint(), "Enter:Detail d:Delete r:Refresh");
    }

    #[test]
    fn test_help_hint_detail() {
        let (mut module, _rx) = setup();
        module.handle_key(key(KeyCode::Enter));
        assert_eq!(module.help_hint(), "Esc:Back");
    }
}
