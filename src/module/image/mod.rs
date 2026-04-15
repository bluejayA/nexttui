pub mod view_model;

use crossterm::event::{KeyCode, KeyEvent};
use ratatui::layout::Rect;
use ratatui::Frame;

use crate::action::Action;
use crate::context::ActionSender;
use crate::component::Component;
use crate::event::AppEvent;
use crate::models::glance::Image;
use crate::module::{ConfirmHandler, PendingAction, ViewState};
use crate::port::types::ImageCreateParams;
use crate::ui::confirm::ConfirmDialog;
use crate::ui::form::{FormAction, FormWidget};
use crate::ui::resource_list::{ResourceList, Row};

use self::view_model::{image_columns, image_create_defs, image_detail_data, image_to_row};

pub struct ImageModule {
    view_state: ViewState,
    images: Vec<Image>,
    is_admin: bool,
    #[allow(dead_code)] // Phase 2: loading spinner
    loading: bool,
    error_message: Option<String>,
    confirm: ConfirmHandler,
    resource_list: ResourceList,
    form: Option<FormWidget>,
    all_tenants: bool,
    action_tx: ActionSender,
}

impl ImageModule {
    pub fn new(action_tx: ActionSender) -> Self {
        Self {
            view_state: ViewState::List,
            images: Vec::new(),
            is_admin: false,
            loading: false,
            error_message: None,
            confirm: ConfirmHandler::new(),
            resource_list: ResourceList::new(image_columns(false)),
            form: None,
            all_tenants: false,
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
        self.resource_list.selected_index()
    }

    pub fn error_message(&self) -> Option<&str> {
        self.error_message.as_deref()
    }

    fn selected_image(&self) -> Option<&Image> {
        self.images.get(self.resource_list.selected_index())
    }

    fn rows(&self) -> Vec<Row> {
        self.images.iter().map(|i| image_to_row(i, self.all_tenants)).collect()
    }

    fn resolve_action(pending: PendingAction) -> Option<Action> {
        match pending {
            PendingAction::DeleteImage { id, .. } => Some(Action::DeleteImage { id }),
            _ => None,
        }
    }

    fn open_create_form(&mut self) {
        let defs = image_create_defs();
        self.form = Some(FormWidget::new("Create Image", defs));
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
                if let Some(img) = self.selected_image() {
                    let id = img.id.clone();
                    self.view_state = ViewState::Detail(id);
                }
                None
            }
            KeyCode::Char('c') if self.is_admin => {
                self.open_create_form();
                Some(Action::EnterFormMode)
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
                let disk_format = values
                    .get("Disk Format")
                    .and_then(|v| match v {
                        crate::ui::form::FormValue::Selected(s) => Some(s.clone()),
                        _ => None,
                    })
                    .unwrap_or_default();
                let container_format = values
                    .get("Container Format")
                    .and_then(|v| match v {
                        crate::ui::form::FormValue::Selected(s) => Some(s.clone()),
                        _ => None,
                    })
                    .unwrap_or_default();
                let visibility = values
                    .get("Visibility")
                    .and_then(|v| match v {
                        crate::ui::form::FormValue::Selected(s) => {
                            if s.is_empty() { None } else { Some(s.clone()) }
                        }
                        _ => None,
                    });
                let min_disk = values
                    .get("Min Disk (GB)")
                    .and_then(|v| match v {
                        crate::ui::form::FormValue::Text(s) => s.parse::<u32>().ok(),
                        _ => None,
                    });
                let min_ram = values
                    .get("Min RAM (MB)")
                    .and_then(|v| match v {
                        crate::ui::form::FormValue::Text(s) => s.parse::<u32>().ok(),
                        _ => None,
                    });

                self.close_form();
                let _ = self.action_tx.send(Action::CreateImage(ImageCreateParams {
                    name,
                    disk_format,
                    container_format,
                    visibility,
                    min_disk,
                    min_ram,
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

impl Component for ImageModule {
    fn refresh_action(&self) -> Option<Action> { Some(Action::FetchImages) }
    fn is_modal(&self) -> bool { self.confirm.is_active() || self.form.is_some() }

    fn set_all_tenants(&mut self, v: bool) {
        self.all_tenants = v;
        self.resource_list = ResourceList::new(image_columns(v));
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
                let rows = self.rows();
                self.resource_list.set_rows(rows);
            }
            AppEvent::ImageCreated(_) => {
                self.close_form();
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
                let name = self.images.iter()
                    .find(|r| r.id == *id)
                    .map(|r| r.name.as_str())
                    .unwrap_or("...");
                Some(format!("Image: {name}"))
            }
            ViewState::Create => Some("Create Image".into()),
        }
    }

    fn help_hint(&self) -> &str {
        match &self.view_state {
            ViewState::List if self.is_admin => "Enter:Detail c:Create d:Delete r:Refresh",
            ViewState::List => "Enter:Detail r:Refresh",
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
            owner: None,
        }
    }

    fn setup(is_admin: bool) -> (ImageModule, ActionReceiver) {
        let (tx, rx) = test_action_channel();
        let mut module = ImageModule::new(tx);
        module.set_admin(is_admin);
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
        let (tx, _rx) = test_action_channel();
        let module = ImageModule::new(tx);
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
        let (tx, _rx) = test_action_channel();
        let mut module = ImageModule::new(tx);
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
        assert_eq!(form.field_count(), 6);
        assert_eq!(form.focused_field_name(), "Name");
    }

    #[test]
    fn test_help_hint_list_admin() {
        let (module, _rx) = setup(true);
        assert_eq!(module.help_hint(), "Enter:Detail c:Create d:Delete r:Refresh");
    }

    #[test]
    fn test_help_hint_list_non_admin() {
        let (module, _rx) = setup(false);
        assert_eq!(module.help_hint(), "Enter:Detail r:Refresh");
    }

    #[test]
    fn test_help_hint_detail() {
        let (mut module, _rx) = setup(false);
        module.handle_key(key(KeyCode::Enter));
        assert_eq!(module.help_hint(), "Esc:Back");
    }

    #[test]
    fn test_help_hint_create() {
        let (mut module, _rx) = setup(true);
        module.handle_key(key(KeyCode::Char('c')));
        assert_eq!(module.help_hint(), "Esc:Cancel Tab:Next Enter:Submit");
    }
}
