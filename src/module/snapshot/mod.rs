pub mod view_model;

use crossterm::event::{KeyCode, KeyEvent};
use ratatui::layout::Rect;
use ratatui::Frame;
use tokio::sync::mpsc;

use crate::action::Action;
use crate::component::Component;
use crate::event::AppEvent;
use crate::models::cinder::VolumeSnapshot;
use crate::module::{ConfirmHandler, ListNav, PendingAction, ViewState};
use crate::ui::confirm::ConfirmDialog;
use crate::ui::resource_list::{ResourceList, Row};

use self::view_model::{snapshot_columns, snapshot_detail_data, snapshot_to_row};

pub struct SnapshotModule {
    view_state: ViewState,
    snapshots: Vec<VolumeSnapshot>,
    nav: ListNav,
    #[allow(dead_code)] // Phase 2: loading spinner
    loading: bool,
    error_message: Option<String>,
    confirm: ConfirmHandler,
    resource_list: ResourceList,
    action_tx: mpsc::UnboundedSender<Action>,
}

impl SnapshotModule {
    pub fn new(action_tx: mpsc::UnboundedSender<Action>) -> Self {
        Self {
            view_state: ViewState::List,
            snapshots: Vec::new(),
            nav: ListNav::new(),
            loading: false,
            error_message: None,
            confirm: ConfirmHandler::new(),
            resource_list: ResourceList::new(snapshot_columns()),
            action_tx,
        }
    }

    pub fn view_state(&self) -> &ViewState {
        &self.view_state
    }

    pub fn snapshots(&self) -> &[VolumeSnapshot] {
        &self.snapshots
    }

    pub fn selected_index(&self) -> usize {
        self.nav.selected_index
    }

    pub fn error_message(&self) -> Option<&str> {
        self.error_message.as_deref()
    }

    fn selected_snapshot(&self) -> Option<&VolumeSnapshot> {
        self.snapshots.get(self.nav.selected_index)
    }

    fn rows(&self) -> Vec<Row> {
        self.snapshots.iter().map(snapshot_to_row).collect()
    }

    fn resolve_action(pending: PendingAction) -> Option<Action> {
        match pending {
            PendingAction::DeleteSnapshot { id, .. } => Some(Action::DeleteSnapshot { id }),
            _ => None,
        }
    }

    fn handle_list_key(&mut self, key: KeyEvent) -> Option<Action> {
        if self.nav.handle_key(key) {
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
                        ConfirmDialog::yes_no(format!("Delete snapshot '{name}'?")),
                        PendingAction::DeleteSnapshot { id, name },
                    );
                }
                None
            }
            KeyCode::Char('r') => Some(Action::FetchSnapshots),
            KeyCode::Esc => Some(Action::Back),
            _ => None,
        }
    }

    fn handle_detail_key(&mut self, key: KeyEvent) -> Option<Action> {
        match key.code {
            KeyCode::Esc | KeyCode::Char('q') => {
                self.view_state = ViewState::List;
                None
            }
            _ => None,
        }
    }
}

impl Component for SnapshotModule {
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

    fn handle_event(&mut self, event: &AppEvent) {
        match event {
            AppEvent::SnapshotsLoaded(snapshots) => {
                self.snapshots = snapshots.clone();
                self.loading = false;
                self.error_message = None;
                self.nav.set_count(self.snapshots.len());
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
}

#[cfg(test)]
mod tests {
    use super::*;

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
        }
    }

    fn setup() -> (SnapshotModule, mpsc::UnboundedReceiver<Action>) {
        let (tx, rx) = mpsc::unbounded_channel();
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
        let (tx, _rx) = mpsc::unbounded_channel();
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
        let (tx, _rx) = mpsc::unbounded_channel();
        let mut module = SnapshotModule::new(tx);
        let snaps = vec![make_snapshot("snap-1", "test", "available")];
        module.handle_event(&AppEvent::SnapshotsLoaded(snaps));
        assert_eq!(module.snapshots().len(), 1);
    }

    #[test]
    fn test_handle_event_snapshot_deleted_triggers_refresh() {
        let (mut module, mut rx) = setup();
        module.handle_event(&AppEvent::SnapshotDeleted { id: "snap-1".into() });
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
}
