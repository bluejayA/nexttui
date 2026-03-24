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
use crate::models::cinder::Volume;
use crate::module::{ConfirmHandler, ListNav, PendingAction, ViewState};
use crate::ui::confirm::ConfirmDialog;
use crate::ui::resource_list::{ResourceList, Row};

use self::view_model::{volume_columns, volume_detail_data, volume_to_row};

pub struct VolumeModule {
    view_state: ViewState,
    volumes: Vec<Volume>,
    nav: ListNav,
    #[allow(dead_code)] // Phase 2: loading spinner
    loading: bool,
    error_message: Option<String>,
    confirm: ConfirmHandler,
    resource_list: ResourceList,
    action_tx: mpsc::UnboundedSender<Action>,
}

impl VolumeModule {
    pub fn new(action_tx: mpsc::UnboundedSender<Action>) -> Self {
        Self {
            view_state: ViewState::List,
            volumes: Vec::new(),
            nav: ListNav::new(),
            loading: false,
            error_message: None,
            confirm: ConfirmHandler::new(),
            resource_list: ResourceList::new(volume_columns()),
            action_tx,
        }
    }

    pub fn view_state(&self) -> &ViewState {
        &self.view_state
    }

    pub fn volumes(&self) -> &[Volume] {
        &self.volumes
    }

    pub fn selected_index(&self) -> usize {
        self.nav.selected_index
    }

    pub fn error_message(&self) -> Option<&str> {
        self.error_message.as_deref()
    }

    fn selected_volume(&self) -> Option<&Volume> {
        self.volumes.get(self.nav.selected_index)
    }

    fn rows(&self) -> Vec<Row> {
        self.volumes.iter().map(volume_to_row).collect()
    }

    fn resolve_action(pending: PendingAction) -> Option<Action> {
        match pending {
            PendingAction::DeleteVolume { id, .. } => {
                Some(Action::DeleteVolume { id, force: false })
            }
            _ => None,
        }
    }

    fn handle_list_key(&mut self, key: KeyEvent) -> Option<Action> {
        if self.nav.handle_key(key) {
            return None;
        }

        match key.code {
            KeyCode::Enter => {
                if let Some(vol) = self.selected_volume() {
                    let id = vol.id.clone();
                    self.view_state = ViewState::Detail(id);
                }
                None
            }
            KeyCode::Char('c') => {
                self.view_state = ViewState::Create;
                None
            }
            KeyCode::Char('d') => {
                if let Some(vol) = self.selected_volume() {
                    let id = vol.id.clone();
                    let name = vol
                        .name
                        .clone()
                        .unwrap_or_else(|| id.chars().take(8).collect());
                    self.confirm.open(
                        ConfirmDialog::type_to_confirm(
                            format!("Delete volume '{name}'?"),
                            name.clone(),
                        ),
                        PendingAction::DeleteVolume { id, name },
                    );
                }
                None
            }
            KeyCode::Char('r') => Some(Action::FetchVolumes),
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

impl Component for VolumeModule {
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
            AppEvent::VolumesLoaded(volumes) => {
                self.volumes = volumes.clone();
                self.loading = false;
                self.error_message = None;
                self.nav.set_count(self.volumes.len());
                let rows = self.rows();
                self.resource_list.set_rows(rows);
            }
            AppEvent::VolumeCreated(_) => {
                self.view_state = ViewState::List;
                let _ = self.action_tx.send(Action::FetchVolumes);
            }
            AppEvent::VolumeDeleted { .. } | AppEvent::VolumeExtended { .. } => {
                let _ = self.action_tx.send(Action::FetchVolumes);
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
                if let Some(vol) = self.volumes.iter().find(|v| v.id == *id) {
                    let data = volume_detail_data(vol);
                    let mut dv = crate::ui::detail_view::DetailView::new();
                    dv.set_data(data);
                    dv.render(frame, area);
                }
            }
            ViewState::Create => {
                let text = Paragraph::new(vec![
                    Line::raw(""),
                    Line::raw("  Volume Create Form (Tab/Enter to submit, Esc to cancel)"),
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

    fn key(code: KeyCode) -> KeyEvent {
        KeyEvent::from(code)
    }

    fn make_volume(id: &str, name: &str, status: &str) -> Volume {
        Volume {
            id: id.into(),
            name: Some(name.into()),
            description: None,
            status: status.into(),
            size: 100,
            volume_type: Some("ssd".into()),
            encrypted: false,
            bootable: "false".into(),
            attachments: vec![],
            availability_zone: None,
            created_at: None,
        }
    }

    fn setup() -> (VolumeModule, mpsc::UnboundedReceiver<Action>) {
        let (tx, rx) = mpsc::unbounded_channel();
        let mut module = VolumeModule::new(tx);
        let volumes = vec![
            make_volume("vol-1", "data", "available"),
            make_volume("vol-2", "boot", "in-use"),
            make_volume("vol-3", "temp", "error"),
        ];
        module.handle_event(&AppEvent::VolumesLoaded(volumes));
        (module, rx)
    }

    #[test]
    fn test_initial_state_is_list() {
        let (tx, _rx) = mpsc::unbounded_channel();
        let module = VolumeModule::new(tx);
        assert_eq!(*module.view_state(), ViewState::List);
        assert!(module.volumes().is_empty());
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
        assert_eq!(*module.view_state(), ViewState::Detail("vol-1".into()));
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
    fn test_handle_key_d_opens_confirm() {
        let (mut module, _rx) = setup();
        assert!(!module.confirm.is_active());
        module.handle_key(key(KeyCode::Char('d')));
        assert!(module.confirm.is_active());
    }

    #[test]
    fn test_confirm_delete_volume() {
        let (mut module, _rx) = setup();
        module.handle_key(key(KeyCode::Char('d')));
        // Type the volume name to confirm
        for c in "data".chars() {
            module.handle_key(key(KeyCode::Char(c)));
        }
        let action = module.handle_key(key(KeyCode::Enter));
        assert!(matches!(action, Some(Action::DeleteVolume { .. })));
        assert!(!module.confirm.is_active());
    }

    #[test]
    fn test_handle_key_r_fetches_volumes() {
        let (mut module, _rx) = setup();
        let action = module.handle_key(key(KeyCode::Char('r')));
        assert!(matches!(action, Some(Action::FetchVolumes)));
    }

    #[test]
    fn test_handle_event_volumes_loaded() {
        let (tx, _rx) = mpsc::unbounded_channel();
        let mut module = VolumeModule::new(tx);
        let volumes = vec![make_volume("vol-1", "test", "available")];
        module.handle_event(&AppEvent::VolumesLoaded(volumes));
        assert_eq!(module.volumes().len(), 1);
    }

    #[test]
    fn test_handle_event_volume_created_transitions_to_list() {
        let (mut module, mut rx) = setup();
        module.view_state = ViewState::Create;
        let vol = make_volume("vol-new", "new-vol", "creating");
        module.handle_event(&AppEvent::VolumeCreated(vol));
        assert_eq!(*module.view_state(), ViewState::List);
        let action = rx.try_recv().unwrap();
        assert!(matches!(action, Action::FetchVolumes));
    }

    #[test]
    fn test_handle_event_volume_deleted_triggers_refresh() {
        let (mut module, mut rx) = setup();
        module.handle_event(&AppEvent::VolumeDeleted { id: "vol-1".into() });
        let action = rx.try_recv().unwrap();
        assert!(matches!(action, Action::FetchVolumes));
    }

    #[test]
    fn test_handle_event_volume_extended_triggers_refresh() {
        let (mut module, mut rx) = setup();
        module.handle_event(&AppEvent::VolumeExtended { id: "vol-1".into() });
        let action = rx.try_recv().unwrap();
        assert!(matches!(action, Action::FetchVolumes));
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
