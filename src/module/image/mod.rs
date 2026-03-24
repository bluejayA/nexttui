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
use crate::models::glance::Image;
use crate::module::{ConfirmHandler, ListNav, PendingAction, ViewState};
use crate::ui::confirm::ConfirmDialog;
use crate::ui::resource_list::{ResourceList, Row};

use self::view_model::{image_columns, image_detail_data, image_to_row};

pub struct ImageModule {
    view_state: ViewState,
    images: Vec<Image>,
    nav: ListNav,
    is_admin: bool,
    #[allow(dead_code)] // Phase 2: loading spinner
    loading: bool,
    error_message: Option<String>,
    confirm: ConfirmHandler,
    resource_list: ResourceList,
    action_tx: mpsc::UnboundedSender<Action>,
}

impl ImageModule {
    pub fn new(action_tx: mpsc::UnboundedSender<Action>, is_admin: bool) -> Self {
        Self {
            view_state: ViewState::List,
            images: Vec::new(),
            nav: ListNav::new(),
            is_admin,
            loading: false,
            error_message: None,
            confirm: ConfirmHandler::new(),
            resource_list: ResourceList::new(image_columns()),
            action_tx,
        }
    }

    pub fn view_state(&self) -> &ViewState {
        &self.view_state
    }

    pub fn images(&self) -> &[Image] {
        &self.images
    }

    pub fn selected_index(&self) -> usize {
        self.nav.selected_index
    }

    pub fn error_message(&self) -> Option<&str> {
        self.error_message.as_deref()
    }

    fn selected_image(&self) -> Option<&Image> {
        self.images.get(self.nav.selected_index)
    }

    fn rows(&self) -> Vec<Row> {
        self.images.iter().map(image_to_row).collect()
    }

    fn resolve_action(pending: PendingAction) -> Option<Action> {
        match pending {
            PendingAction::DeleteImage { id, .. } => Some(Action::DeleteImage { id }),
            _ => None,
        }
    }

    fn handle_list_key(&mut self, key: KeyEvent) -> Option<Action> {
        if self.nav.handle_key(key) {
            return None;
        }

        match key.code {
            KeyCode::Enter => {
                if let Some(img) = self.selected_image() {
                    let id = img.id.clone();
                    self.view_state = ViewState::Detail(id);
                }
                None
            }
            KeyCode::Char('c') if self.is_admin => {
                self.view_state = ViewState::Create;
                None
            }
            KeyCode::Char('d') if self.is_admin => {
                if let Some(img) = self.selected_image() {
                    let id = img.id.clone();
                    let name = img.name.clone();
                    self.confirm.open(
                        ConfirmDialog::yes_no(format!("Delete image '{name}'?")),
                        PendingAction::DeleteImage { id, name },
                    );
                }
                None
            }
            KeyCode::Char('r') => Some(Action::FetchImages),
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

impl Component for ImageModule {
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
            AppEvent::ImagesLoaded(images) => {
                self.images = images.clone();
                self.loading = false;
                self.error_message = None;
                self.nav.set_count(self.images.len());
                let rows = self.rows();
                self.resource_list.set_rows(rows);
            }
            AppEvent::ImageCreated(_) => {
                self.view_state = ViewState::List;
                let _ = self.action_tx.send(Action::FetchImages);
            }
            AppEvent::ImageDeleted { .. } => {
                let _ = self.action_tx.send(Action::FetchImages);
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
                if let Some(img) = self.images.iter().find(|i| i.id == *id) {
                    let data = image_detail_data(img);
                    let mut dv = crate::ui::detail_view::DetailView::new();
                    dv.set_data(data);
                    dv.render(frame, area);
                }
            }
            ViewState::Create => {
                let text = Paragraph::new(vec![
                    Line::raw(""),
                    Line::raw("  Image Create Form (Tab/Enter to submit, Esc to cancel)"),
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

    fn make_image(id: &str, name: &str, status: &str) -> Image {
        Image {
            id: id.into(),
            name: name.into(),
            status: status.into(),
            disk_format: Some("qcow2".into()),
            container_format: Some("bare".into()),
            size: Some(1_073_741_824),
            visibility: "public".into(),
            min_disk: 10,
            min_ram: 512,
            checksum: None,
            created_at: None,
        }
    }

    fn setup(is_admin: bool) -> (ImageModule, mpsc::UnboundedReceiver<Action>) {
        let (tx, rx) = mpsc::unbounded_channel();
        let mut module = ImageModule::new(tx, is_admin);
        let images = vec![
            make_image("img-1", "Ubuntu 22.04", "active"),
            make_image("img-2", "CentOS 9", "active"),
            make_image("img-3", "Windows", "deactivated"),
        ];
        module.handle_event(&AppEvent::ImagesLoaded(images));
        (module, rx)
    }

    #[test]
    fn test_initial_state_is_list() {
        let (tx, _rx) = mpsc::unbounded_channel();
        let module = ImageModule::new(tx, false);
        assert_eq!(*module.view_state(), ViewState::List);
        assert!(module.images().is_empty());
    }

    #[test]
    fn test_handle_key_j_k_navigation() {
        let (mut module, _rx) = setup(false);
        assert_eq!(module.selected_index(), 0);
        module.handle_key(key(KeyCode::Char('j')));
        assert_eq!(module.selected_index(), 1);
        module.handle_key(key(KeyCode::Char('k')));
        assert_eq!(module.selected_index(), 0);
    }

    #[test]
    fn test_handle_key_enter_to_detail() {
        let (mut module, _rx) = setup(false);
        module.handle_key(key(KeyCode::Enter));
        assert_eq!(*module.view_state(), ViewState::Detail("img-1".into()));
    }

    #[test]
    fn test_handle_key_esc_detail_to_list() {
        let (mut module, _rx) = setup(false);
        module.handle_key(key(KeyCode::Enter));
        module.handle_key(key(KeyCode::Esc));
        assert_eq!(*module.view_state(), ViewState::List);
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
    fn test_confirm_delete_image() {
        let (mut module, _rx) = setup(true);
        module.handle_key(key(KeyCode::Char('d')));
        let action = module.handle_key(key(KeyCode::Char('y')));
        assert!(matches!(action, Some(Action::DeleteImage { .. })));
    }

    #[test]
    fn test_handle_key_r_fetches_images() {
        let (mut module, _rx) = setup(false);
        let action = module.handle_key(key(KeyCode::Char('r')));
        assert!(matches!(action, Some(Action::FetchImages)));
    }

    #[test]
    fn test_handle_event_images_loaded() {
        let (tx, _rx) = mpsc::unbounded_channel();
        let mut module = ImageModule::new(tx, false);
        let images = vec![make_image("img-1", "test", "active")];
        module.handle_event(&AppEvent::ImagesLoaded(images));
        assert_eq!(module.images().len(), 1);
    }

    #[test]
    fn test_handle_event_image_created_transitions_to_list() {
        let (mut module, mut rx) = setup(true);
        module.view_state = ViewState::Create;
        let img = make_image("img-new", "new-img", "queued");
        module.handle_event(&AppEvent::ImageCreated(img));
        assert_eq!(*module.view_state(), ViewState::List);
        let action = rx.try_recv().unwrap();
        assert!(matches!(action, Action::FetchImages));
    }

    #[test]
    fn test_handle_event_image_deleted_triggers_refresh() {
        let (mut module, mut rx) = setup(true);
        module.handle_event(&AppEvent::ImageDeleted { id: "img-1".into() });
        let action = rx.try_recv().unwrap();
        assert!(matches!(action, Action::FetchImages));
    }

    #[test]
    fn test_handle_event_api_error() {
        let (mut module, _rx) = setup(false);
        module.handle_event(&AppEvent::ApiError {
            operation: "delete".into(),
            message: "forbidden".into(),
        });
        assert_eq!(module.error_message(), Some("delete: forbidden"));
    }
}
