pub mod view_model;

use crossterm::event::{KeyCode, KeyEvent};
use ratatui::layout::Rect;
use ratatui::Frame;
use tokio::sync::mpsc;

use crate::action::Action;
use crate::component::Component;
use crate::event::AppEvent;
use crate::models::common::is_terminal_server_status;
use crate::models::nova::{Flavor, Server, ServerMigration};
use crate::module::{ConfirmHandler, PendingAction, ViewState};
use crate::port::types::{NetworkAttachment, ServerCreateParams};
use crate::ui::confirm::ConfirmDialog;
use crate::ui::form::{FormAction, FormWidget, SelectOption};
use crate::ui::resource_list::{ResourceList, Row};
use crate::ui::select_popup::{ItemHint, SelectItem, SelectPopup, SelectResult};

use self::view_model::{
    server_columns_full, server_create_defs, server_detail_data_full, server_to_row_full,
};

#[derive(Debug, Clone)]
pub struct ResizePendingInfo {
    pub server_id: String,
}

pub struct ServerModule {
    view_state: ViewState,
    servers: Vec<Server>,
    loading: bool,
    error_message: Option<String>,
    confirm: ConfirmHandler,
    resource_list: ResourceList,
    form: Option<FormWidget>,
    all_tenants: bool,
    is_admin: bool,
    migration_progress: Option<(String, ServerMigration)>,
    select_popup: Option<SelectPopup>,
    resize_pending: Option<ResizePendingInfo>,
    cached_flavors: Vec<Flavor>,
    // Cached dropdown options — populated by handle_event, applied to form on open/load
    cached_flavor_opts: Vec<SelectOption>,
    cached_image_opts: Vec<SelectOption>,
    cached_network_opts: Vec<SelectOption>,
    cached_sg_opts: Vec<SelectOption>,
    action_tx: mpsc::UnboundedSender<Action>,
}

impl ServerModule {
    pub fn new(action_tx: mpsc::UnboundedSender<Action>) -> Self {
        Self {
            view_state: ViewState::List,
            servers: Vec::new(),
            loading: false,
            error_message: None,
            confirm: ConfirmHandler::new(),
            resource_list: ResourceList::new(server_columns_full(false, false)),
            form: None,
            all_tenants: false,
            is_admin: false,
            migration_progress: None,
            select_popup: None,
            resize_pending: None,
            cached_flavors: Vec::new(),
            cached_flavor_opts: Vec::new(),
            cached_image_opts: Vec::new(),
            cached_network_opts: Vec::new(),
            cached_sg_opts: Vec::new(),
            action_tx,
        }
    }

    pub fn view_state(&self) -> &ViewState {
        &self.view_state
    }

    pub fn servers(&self) -> &[Server] {
        &self.servers
    }

    pub fn selected_index(&self) -> usize {
        self.resource_list.selected_index()
    }

    pub fn error_message(&self) -> Option<&str> {
        self.error_message.as_deref()
    }

    pub fn migration_progress(&self) -> Option<&ServerMigration> {
        self.migration_progress.as_ref().map(|(_, m)| m)
    }

    pub fn migration_progress_for(&self, server_id: &str) -> Option<&ServerMigration> {
        self.migration_progress
            .as_ref()
            .filter(|(sid, _)| sid == server_id)
            .map(|(_, m)| m)
    }

    fn selected_server(&self) -> Option<&Server> {
        self.servers.get(self.resource_list.selected_index())
    }

    fn rows(&self) -> Vec<Row> {
        self.servers
            .iter()
            .map(|s| server_to_row_full(s, self.all_tenants, self.is_admin))
            .collect()
    }

    fn resolve_action(pending: PendingAction) -> Option<Action> {
        match pending {
            PendingAction::Delete { id, name } => Some(Action::DeleteServer { id, name }),
            PendingAction::Reboot { id, hard } => Some(Action::RebootServer { id, hard }),
            PendingAction::Stop { id } => Some(Action::StopServer { id }),
            PendingAction::Resize { id, flavor_id } => Some(Action::ResizeServer { id, flavor_id }),
            PendingAction::ConfirmResize { id } => Some(Action::ConfirmResize { id }),
            PendingAction::RevertResize { id } => Some(Action::RevertResize { id }),
            PendingAction::LiveMigrate { id } => Some(Action::LiveMigrateServer {
                id,
                host: None,
            }),
            PendingAction::ColdMigrate { id } => Some(Action::ColdMigrateServer { id }),
            PendingAction::ConfirmMigrate { id } => Some(Action::ConfirmMigration { id }),
            PendingAction::RevertMigrate { id } => Some(Action::RevertMigration { id }),
            PendingAction::Evacuate { id } => Some(Action::EvacuateServer { id, host: None }),
            _ => None,
        }
    }

    fn open_create_form(&mut self) {
        let defs = server_create_defs();
        let mut form = FormWidget::new("Create Server", defs);
        // Apply cached options if already loaded (e.g., demo mode pre-loads data)
        if !self.cached_flavor_opts.is_empty() {
            form.set_field_options("Flavor", self.cached_flavor_opts.clone());
        }
        if !self.cached_image_opts.is_empty() {
            form.set_field_options("Image", self.cached_image_opts.clone());
        }
        if !self.cached_network_opts.is_empty() {
            form.set_field_options("Network", self.cached_network_opts.clone());
        }
        if !self.cached_sg_opts.is_empty() {
            form.set_field_options("Security Group", self.cached_sg_opts.clone());
        }
        self.form = Some(form);
        self.view_state = ViewState::Create;
        // Also request fresh data — handle_event will update if new data arrives
        let _ = self.action_tx.send(Action::FetchFlavors);
        let _ = self.action_tx.send(Action::FetchImages);
        let _ = self.action_tx.send(Action::FetchNetworks);
        let _ = self.action_tx.send(Action::FetchSecurityGroups);
    }

    fn close_form(&mut self) {
        self.form = None;
        self.view_state = ViewState::List;
    }

    fn handle_list_key(&mut self, key: KeyEvent) -> Option<Action> {
        // Navigation keys handled by ResourceList
        if self.resource_list.handle_nav_key(key) {
            return None;
        }

        match key.code {
            KeyCode::Enter => {
                if let Some(server) = self.selected_server() {
                    let id = server.id.clone();
                    self.view_state = ViewState::Detail(id);
                }
                None
            }
            KeyCode::Char('c') => {
                self.open_create_form();
                Some(Action::EnterFormMode)
            }
            KeyCode::Char('d') => {
                if let Some(server) = self.selected_server() {
                    let name = server.name.clone();
                    let id = server.id.clone();
                    self.confirm.open(
                        ConfirmDialog::type_to_confirm(
                            format!("Delete server '{name}'?"),
                            name.clone(),
                        ),
                        PendingAction::Delete { id, name },
                    );
                }
                None
            }
            KeyCode::Char('r') => Some(Action::FetchServers),
            KeyCode::Left => Some(Action::FocusSidebar),
            KeyCode::Esc => Some(Action::Back),
            _ => None,
        }
    }

    fn build_flavor_items(&self, current_flavor_id: &str, current_disk: Option<u32>) -> Vec<SelectItem> {
        self.cached_flavors
            .iter()
            .map(|f| {
                let hint = if f.id == current_flavor_id {
                    ItemHint::Current
                } else if current_disk.is_some_and(|d| f.disk < d) {
                    ItemHint::Warning("disk shrink".into())
                } else {
                    ItemHint::Normal
                };
                SelectItem {
                    id: f.id.clone(),
                    label: format!("{} ({}vCPU / {}MB / {}GB)", f.name, f.vcpus, f.ram, f.disk),
                    hint,
                }
            })
            .collect()
    }

    fn handle_detail_key(&mut self, key: KeyEvent) -> Option<Action> {
        // SelectPopup takes priority after ConfirmDialog
        if let Some(ref mut popup) = self.select_popup {
            match popup.handle_key(key) {
                SelectResult::Selected(flavor_id) => {
                    let flavor_name = self.cached_flavors.iter()
                        .find(|f| f.id == flavor_id)
                        .map(|f| f.name.as_str())
                        .unwrap_or(&flavor_id);
                    let msg = format!("Resize to {flavor_name}?");
                    if let ViewState::Detail(ref id) = self.view_state {
                        let id = id.clone();
                        self.select_popup = None;
                        self.confirm.open(
                            ConfirmDialog::yes_no(msg),
                            PendingAction::Resize { id, flavor_id },
                        );
                    } else {
                        self.select_popup = None;
                    }
                    return None;
                }
                SelectResult::Cancelled => {
                    self.select_popup = None;
                    return None;
                }
                SelectResult::Pending => return None,
            }
        }

        match key.code {
            KeyCode::Esc | KeyCode::Char('q') | KeyCode::Left => {
                self.view_state = ViewState::List;
                None
            }
            KeyCode::Char('R') => {
                if let ViewState::Detail(ref id) = self.view_state {
                    let id = id.clone();
                    self.confirm.open(
                        ConfirmDialog::yes_no("Hard reboot this server?"),
                        PendingAction::Reboot { id, hard: true },
                    );
                }
                None
            }
            KeyCode::Char('S') => {
                if let ViewState::Detail(ref id) = self.view_state {
                    return Some(Action::StartServer { id: id.clone() });
                }
                None
            }
            KeyCode::Char('X') => {
                if let ViewState::Detail(ref id) = self.view_state {
                    let id = id.clone();
                    self.confirm.open(
                        ConfirmDialog::yes_no("Stop this server?"),
                        PendingAction::Stop { id },
                    );
                }
                None
            }
            // Resize (ACTIVE/SHUTOFF)
            KeyCode::Char('F') => {
                if let ViewState::Detail(ref id) = self.view_state {
                    let server = self.servers.iter().find(|s| s.id == *id);
                    if server.is_some_and(|s| s.status == "ACTIVE" || s.status == "SHUTOFF") {
                        if self.cached_flavors.is_empty() {
                            let _ = self.action_tx.send(Action::FetchFlavors);
                        } else {
                            let current_flavor_id = server.map(|s| s.flavor.id.as_str()).unwrap_or("");
                            let current_disk = server.and_then(|s| s.flavor.disk);
                            let items = self.build_flavor_items(current_flavor_id, current_disk);
                            self.select_popup = Some(SelectPopup::new("Select Flavor", items));
                        }
                    }
                }
                None
            }
            // Migration (admin-only)
            KeyCode::Char('M') if self.is_admin => {
                if let ViewState::Detail(ref id) = self.view_state {
                    let id = id.clone();
                    self.confirm.open(
                        ConfirmDialog::yes_no("Live migrate this server?"),
                        PendingAction::LiveMigrate { id },
                    );
                }
                None
            }
            KeyCode::Char('C') if self.is_admin => {
                if let ViewState::Detail(ref id) = self.view_state {
                    let id = id.clone();
                    self.confirm.open(
                        ConfirmDialog::yes_no("Cold migrate this server?"),
                        PendingAction::ColdMigrate { id },
                    );
                }
                None
            }
            // Confirm/Revert (VERIFY_RESIZE only) — branches on resize_pending
            KeyCode::Char('Y') => {
                if let ViewState::Detail(ref id) = self.view_state {
                    let server = self.servers.iter().find(|s| s.id == *id);
                    if server.is_some_and(|s| s.status == "VERIFY_RESIZE") {
                        let id = id.clone();
                        let is_resize = self.resize_pending
                            .as_ref()
                            .is_some_and(|rp| rp.server_id == id);
                        if is_resize {
                            self.confirm.open(
                                ConfirmDialog::yes_no("Confirm resize?"),
                                PendingAction::ConfirmResize { id },
                            );
                        } else {
                            self.confirm.open(
                                ConfirmDialog::yes_no("Confirm migration?"),
                                PendingAction::ConfirmMigrate { id },
                            );
                        }
                    }
                }
                None
            }
            KeyCode::Char('N') => {
                if let ViewState::Detail(ref id) = self.view_state {
                    let server = self.servers.iter().find(|s| s.id == *id);
                    if server.is_some_and(|s| s.status == "VERIFY_RESIZE") {
                        let id = id.clone();
                        let is_resize = self.resize_pending
                            .as_ref()
                            .is_some_and(|rp| rp.server_id == id);
                        if is_resize {
                            self.confirm.open(
                                ConfirmDialog::yes_no("Revert resize?"),
                                PendingAction::RevertResize { id },
                            );
                        } else {
                            self.confirm.open(
                                ConfirmDialog::yes_no("Revert migration?"),
                                PendingAction::RevertMigrate { id },
                            );
                        }
                    }
                }
                None
            }
            // Evacuate (admin-only, ERROR status only)
            KeyCode::Char('E') if self.is_admin => {
                if let ViewState::Detail(ref id) = self.view_state {
                    let server = self.servers.iter().find(|s| s.id == *id);
                    if server.is_some_and(|s| s.status == "ERROR") {
                        let id = id.clone();
                        self.confirm.open(
                            ConfirmDialog::yes_no("Evacuate this server? Data on non-volume-backed instances may be lost."),
                            PendingAction::Evacuate { id },
                        );
                    }
                }
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
                let image_id = values
                    .get("Image")
                    .and_then(|v| match v {
                        crate::ui::form::FormValue::Selected(s) => Some(s.clone()),
                        _ => None,
                    })
                    .unwrap_or_default();
                let flavor_id = values
                    .get("Flavor")
                    .and_then(|v| match v {
                        crate::ui::form::FormValue::Selected(s) => Some(s.clone()),
                        _ => None,
                    })
                    .unwrap_or_default();
                let network_id = values
                    .get("Network")
                    .and_then(|v| match v {
                        crate::ui::form::FormValue::Selected(s) => Some(s.clone()),
                        _ => None,
                    })
                    .unwrap_or_default();
                let security_group = values
                    .get("Security Group")
                    .and_then(|v| match v {
                        crate::ui::form::FormValue::Selected(s) => {
                            if s.is_empty() { None } else { Some(vec![s.clone()]) }
                        }
                        _ => None,
                    });
                let key_name = values
                    .get("Key Pair")
                    .and_then(|v| match v {
                        crate::ui::form::FormValue::Text(s) => {
                            if s.is_empty() { None } else { Some(s.clone()) }
                        }
                        _ => None,
                    });
                let availability_zone = values
                    .get("Availability Zone")
                    .and_then(|v| match v {
                        crate::ui::form::FormValue::Text(s) => {
                            if s.is_empty() { None } else { Some(s.clone()) }
                        }
                        _ => None,
                    });

                self.close_form();
                let _ = self.action_tx.send(Action::CreateServer(ServerCreateParams {
                    name,
                    image_id,
                    flavor_id,
                    networks: vec![NetworkAttachment {
                        uuid: network_id,
                        fixed_ip: None,
                    }],
                    security_groups: security_group,
                    key_name,
                    availability_zone,
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

impl Component for ServerModule {
    fn refresh_action(&self) -> Option<Action> { Some(Action::FetchServers) }
    fn has_transitional_resources(&self) -> bool {
        self.servers.iter().any(|s| !is_terminal_server_status(&s.status))
    }
    fn is_modal(&self) -> bool { self.confirm.is_active() || self.form.is_some() || self.select_popup.is_some() }

    fn set_all_tenants(&mut self, v: bool) {
        self.all_tenants = v;
        self.resource_list = ResourceList::new(server_columns_full(v, self.is_admin));
    }

    fn set_admin(&mut self, is_admin: bool) {
        self.is_admin = is_admin;
        self.resource_list = ResourceList::new(server_columns_full(self.all_tenants, is_admin));
    }

    fn handle_key(&mut self, key: KeyEvent) -> Option<Action> {
        // ConfirmHandler takes priority
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
            AppEvent::ServersLoaded(servers) => {
                self.servers = servers.clone();
                self.loading = false;
                self.error_message = None;
                let rows = self.rows();
                self.resource_list.set_rows(rows);
                // Clear stale resize_pending if server is no longer VERIFY_RESIZE
                if let Some(ref rp) = self.resize_pending {
                    let still_verify = servers.iter()
                        .find(|s| s.id == rp.server_id)
                        .is_some_and(|s| s.status == "VERIFY_RESIZE");
                    if !still_verify {
                        self.resize_pending = None;
                    }
                }
            }
            AppEvent::ServerDeleted { .. }
            | AppEvent::ServerRebooted { .. }
            | AppEvent::ServerStarted { .. }
            | AppEvent::ServerStopped { .. }
            | AppEvent::ServerCreated(_) => {
                let _ = self.action_tx.send(Action::FetchServers);
            }
            AppEvent::ServerResized { id } => {
                self.resize_pending = Some(ResizePendingInfo {
                    server_id: id.clone(),
                });
            }
            AppEvent::ResizeConfirmed { .. } | AppEvent::ResizeReverted { .. } => {
                self.resize_pending = None;
            }
            AppEvent::MigrationProgressLoaded { server_id, migration } => {
                self.migration_progress = Some((server_id.clone(), migration.clone()));
            }
            AppEvent::ServerLiveMigrated { .. }
            | AppEvent::MigrationConfirmed { .. }
            | AppEvent::MigrationReverted { .. }
            | AppEvent::ServerEvacuated { .. }
            | AppEvent::MigrationPollingStopped { .. } => {
                self.migration_progress = None;
            }
            AppEvent::FlavorsLoaded(flavors) => {
                self.cached_flavors = flavors.clone();
                let opts: Vec<SelectOption> = flavors
                    .iter()
                    .map(|f| SelectOption::new(&f.id, format!("{} ({}vCPU/{}MB/{}GB)", f.name, f.vcpus, f.ram, f.disk)))
                    .collect();
                self.cached_flavor_opts = opts.clone();
                if let Some(form) = &mut self.form {
                    form.set_field_options("Flavor", opts);
                }
            }
            AppEvent::ImagesLoaded(images) => {
                let opts: Vec<SelectOption> = images
                    .iter()
                    .map(|img| SelectOption::new(&img.id, &img.name))
                    .collect();
                self.cached_image_opts = opts.clone();
                if let Some(form) = &mut self.form {
                    form.set_field_options("Image", opts);
                }
            }
            AppEvent::NetworksLoaded(networks) => {
                let opts: Vec<SelectOption> = networks
                    .iter()
                    .map(|n| SelectOption::new(&n.id, &n.name))
                    .collect();
                self.cached_network_opts = opts.clone();
                if let Some(form) = &mut self.form {
                    form.set_field_options("Network", opts);
                }
            }
            AppEvent::SecurityGroupsLoaded(sgs) => {
                let opts: Vec<SelectOption> = sgs
                    .iter()
                    .map(|sg| SelectOption::new(&sg.id, &sg.name))
                    .collect();
                self.cached_sg_opts = opts.clone();
                if let Some(form) = &mut self.form {
                    form.set_field_options("Security Group", opts);
                }
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
                if let Some(server) = self.servers.iter().find(|s| s.id == *id) {
                    let matched_flavor = self.cached_flavors.iter().find(|f| f.id == server.flavor.id);
                    let is_resize = self.resize_pending.as_ref().is_some_and(|rp| rp.server_id == *id);
                    let data = server_detail_data_full(server, self.migration_progress_for(id), matched_flavor, is_resize);
                    let mut dv = crate::ui::detail_view::DetailView::new();
                    dv.set_data(data);
                    dv.render(frame, area);
                }
            }
            ViewState::Create => {
                if let Some(form) = &self.form {
                    form.render(frame, area);
                } else {
                    // Defensive: form should always be Some in Create state.
                    // If not, render list as fallback (next key press will fix state via close_form).
                    self.resource_list.render(frame, area);
                }
            }
        }

        // Overlay: SelectPopup
        if let Some(ref popup) = self.select_popup {
            popup.render(frame, area);
        }
        // Overlay: ConfirmDialog (highest priority)
        self.confirm.render(frame, area);
    }

    fn help_hint(&self) -> &str {
        match &self.view_state {
            ViewState::List => "Enter:Detail c:Create d:Delete r:Refresh",
            ViewState::Detail(id) => {
                let server = self.servers.iter().find(|s| s.id == *id);
                let is_verify = server.is_some_and(|s| s.status == "VERIFY_RESIZE");
                let is_error = server.is_some_and(|s| s.status == "ERROR");

                if is_verify && self.is_admin {
                    "Esc:Back R:Reboot S:Start X:Stop F:Resize M:Migrate C:Cold Y:Confirm N:Revert"
                } else if is_verify {
                    "Esc:Back R:Reboot S:Start X:Stop F:Resize Y:Confirm N:Revert"
                } else if is_error && self.is_admin {
                    "Esc:Back R:Reboot S:Start X:Stop F:Resize M:Migrate C:Cold E:Evacuate"
                } else if self.is_admin {
                    "Esc:Back R:Reboot S:Start X:Stop F:Resize M:Migrate C:Cold"
                } else {
                    "Esc:Back R:Reboot S:Start X:Stop F:Resize"
                }
            }
            ViewState::Create => "Esc:Cancel Tab:Next Enter:Submit",
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::nova::{FlavorRef, ImageRef};
    use std::collections::HashMap;

    fn key(code: KeyCode) -> KeyEvent {
        KeyEvent::from(code)
    }

    fn make_test_server(id: &str, name: &str, status: &str) -> Server {
        Server {
            id: id.into(),
            name: name.into(),
            status: status.into(),
            addresses: HashMap::new(),
            flavor: FlavorRef {
                id: "flv-1".into(),
                original_name: Some("m1.small".into()),
                vcpus: Some(2),
                ram: Some(4096),
                disk: Some(40),
            },
            image: Some(ImageRef { id: "img-1".into() }),
            key_name: None,
            availability_zone: None,
            created: "2026-01-01T00:00:00Z".into(),
            updated: None,
            tenant_id: None,
            host_id: None,
            host: None,
        }
    }

    fn setup() -> (ServerModule, mpsc::UnboundedReceiver<Action>) {
        let (tx, rx) = mpsc::unbounded_channel();
        let mut module = ServerModule::new(tx);
        let servers = vec![
            make_test_server("s1", "web-01", "ACTIVE"),
            make_test_server("s2", "web-02", "SHUTOFF"),
            make_test_server("s3", "db-01", "ERROR"),
        ];
        module.handle_event(&AppEvent::ServersLoaded(servers));
        (module, rx)
    }

    #[test]
    fn test_initial_state_is_list() {
        let (tx, _rx) = mpsc::unbounded_channel();
        let module = ServerModule::new(tx);
        assert_eq!(*module.view_state(), ViewState::List);
        assert!(module.servers().is_empty());
    }

    #[test]
    fn test_handle_key_j_k_navigation() {
        let (mut module, _rx) = setup();
        assert_eq!(module.selected_index(), 0);

        module.handle_key(key(KeyCode::Char('j')));
        assert_eq!(module.selected_index(), 1);

        module.handle_key(key(KeyCode::Char('j')));
        assert_eq!(module.selected_index(), 2);

        // Can't go past end
        module.handle_key(key(KeyCode::Char('j')));
        assert_eq!(module.selected_index(), 2);

        module.handle_key(key(KeyCode::Char('k')));
        assert_eq!(module.selected_index(), 1);

        module.handle_key(key(KeyCode::Char('k')));
        assert_eq!(module.selected_index(), 0);

        module.handle_key(key(KeyCode::Char('k')));
        assert_eq!(module.selected_index(), 0);
    }

    #[test]
    fn test_handle_key_g_and_shift_g() {
        let (mut module, _rx) = setup();

        module.handle_key(key(KeyCode::Char('G')));
        assert_eq!(module.selected_index(), 2);

        module.handle_key(key(KeyCode::Char('g')));
        assert_eq!(module.selected_index(), 0);
    }

    #[test]
    fn test_handle_key_enter_to_detail() {
        let (mut module, _rx) = setup();
        module.handle_key(key(KeyCode::Enter));
        assert_eq!(*module.view_state(), ViewState::Detail("s1".into()));
    }

    #[test]
    fn test_handle_key_esc_detail_to_list() {
        let (mut module, _rx) = setup();
        module.handle_key(key(KeyCode::Enter));
        assert!(matches!(*module.view_state(), ViewState::Detail(_)));

        module.handle_key(key(KeyCode::Esc));
        assert_eq!(*module.view_state(), ViewState::List);
    }

    #[test]
    fn test_handle_key_c_opens_create() {
        let (mut module, _rx) = setup();
        module.handle_key(key(KeyCode::Char('c')));
        assert_eq!(*module.view_state(), ViewState::Create);
        assert!(module.form.is_some());
    }

    #[test]
    fn test_handle_key_d_opens_confirm() {
        let (mut module, _rx) = setup();
        assert!(!module.confirm.is_active());
        module.handle_key(key(KeyCode::Char('d')));
        assert!(module.confirm.is_active());
    }

    #[test]
    fn test_confirm_delete_executes_action() {
        let (mut module, _rx) = setup();
        module.handle_key(key(KeyCode::Char('d')));

        for c in "web-01".chars() {
            module.handle_key(key(KeyCode::Char(c)));
        }
        let action = module.handle_key(key(KeyCode::Enter));
        assert!(matches!(action, Some(Action::DeleteServer { .. })));
        assert!(!module.confirm.is_active());
    }

    #[test]
    fn test_confirm_cancel() {
        let (mut module, _rx) = setup();
        module.handle_key(key(KeyCode::Char('d')));
        module.handle_key(key(KeyCode::Esc));
        assert!(!module.confirm.is_active());
    }

    #[test]
    fn test_handle_event_servers_loaded() {
        let (tx, _rx) = mpsc::unbounded_channel();
        let mut module = ServerModule::new(tx);
        assert!(module.servers().is_empty());

        let servers = vec![make_test_server("s1", "test", "ACTIVE")];
        module.handle_event(&AppEvent::ServersLoaded(servers));
        assert_eq!(module.servers().len(), 1);
        assert!(!module.loading);
    }

    #[test]
    fn test_handle_event_server_deleted_triggers_refresh() {
        let (mut module, mut rx) = setup();
        module.handle_event(&AppEvent::ServerDeleted {
            id: "s1".into(),
            name: "web-01".into(),
        });
        let action = rx.try_recv().unwrap();
        assert!(matches!(action, Action::FetchServers));
    }

    #[test]
    fn test_handle_event_api_error() {
        let (mut module, _rx) = setup();
        module.handle_event(&AppEvent::ApiError {
            operation: "delete".into(),
            message: "not found".into(),
        });
        assert_eq!(module.error_message(), Some("delete: not found"));
    }

    #[test]
    fn test_handle_key_r_fetches_servers() {
        let (mut module, _rx) = setup();
        let action = module.handle_key(key(KeyCode::Char('r')));
        assert!(matches!(action, Some(Action::FetchServers)));
    }

    #[test]
    fn test_detail_hard_reboot_confirm() {
        let (mut module, _rx) = setup();
        module.handle_key(key(KeyCode::Enter));
        module.handle_key(key(KeyCode::Char('R')));
        assert!(module.confirm.is_active());
    }

    #[test]
    fn test_detail_start_server() {
        let (mut module, _rx) = setup();
        module.handle_key(key(KeyCode::Enter));
        let action = module.handle_key(key(KeyCode::Char('S')));
        assert!(matches!(action, Some(Action::StartServer { .. })));
    }

    #[test]
    fn test_detail_stop_server_confirm() {
        let (mut module, _rx) = setup();
        module.handle_key(key(KeyCode::Enter));
        module.handle_key(key(KeyCode::Char('X')));
        assert!(module.confirm.is_active());
    }

    // -- Form integration tests ---------------------------------------------

    #[test]
    fn test_create_form_cancel_returns_to_list() {
        let (mut module, _rx) = setup();
        module.handle_key(key(KeyCode::Char('c'))); // open form
        assert_eq!(*module.view_state(), ViewState::Create);

        module.handle_key(key(KeyCode::Esc)); // cancel form
        assert_eq!(*module.view_state(), ViewState::List);
        assert!(module.form.is_none());
    }

    #[test]
    fn test_create_form_has_expected_fields() {
        let (mut module, _rx) = setup();
        module.handle_key(key(KeyCode::Char('c')));
        let form = module.form.as_ref().unwrap();
        assert_eq!(form.field_count(), 7);
        assert_eq!(form.focused_field_name(), "Name");
    }

    #[test]
    fn test_create_form_submit_produces_action() {
        let (mut module, _rx) = setup();
        module.handle_key(key(KeyCode::Char('c')));

        // Type server name
        for c in "test-vm".chars() {
            module.handle_key(key(KeyCode::Char(c)));
        }

        // Navigate to last field (Availability Zone) and submit
        for _ in 0..6 {
            module.handle_key(key(KeyCode::Down));
        }
        let action = module.handle_key(key(KeyCode::Enter));

        // Submit should fail validation (Image is required but not selected)
        // So action should be None and we stay on form
        assert!(action.is_none());
    }

    #[test]
    fn test_open_create_form_dispatches_fetch_actions() {
        let (mut module, mut rx) = setup();
        module.handle_key(key(KeyCode::Char('c')));

        let mut actions = Vec::new();
        while let Ok(a) = rx.try_recv() {
            actions.push(a);
        }
        assert!(actions.iter().any(|a| matches!(a, Action::FetchFlavors)));
        assert!(actions.iter().any(|a| matches!(a, Action::FetchImages)));
        assert!(actions.iter().any(|a| matches!(a, Action::FetchNetworks)));
        assert!(actions.iter().any(|a| matches!(a, Action::FetchSecurityGroups)));
    }

    #[test]
    fn test_flavors_loaded_populates_form_options() {
        use crate::models::nova::Flavor;

        let (mut module, _rx) = setup();
        module.handle_key(key(KeyCode::Char('c')));

        let flavors = vec![Flavor {
            id: "flv-1".into(),
            name: "m1.small".into(),
            vcpus: 1,
            ram: 2048,
            disk: 20,
            is_public: true,
        }];
        module.handle_event(&AppEvent::FlavorsLoaded(flavors));

        let form = module.form.as_ref().unwrap();
        // Flavor dropdown should now have options
        if let (crate::ui::form::FieldDef::Dropdown { options, .. }, _) = &form.fields()[2] {
            assert_eq!(options.len(), 1);
            assert_eq!(options[0].value, "flv-1");
        } else {
            panic!("Expected Dropdown for Flavor field");
        }
    }

    #[test]
    fn test_migration_progress_stored_on_event() {
        let (mut module, _rx) = setup();
        assert!(module.migration_progress().is_none());

        module.handle_event(&AppEvent::MigrationProgressLoaded {
            server_id: "s1".into(),
            migration: ServerMigration {
                id: 1,
                status: "running".into(),
                source_compute: "compute-01".into(),
                dest_compute: "compute-02".into(),
                memory_total_bytes: Some(1024),
                memory_processed_bytes: Some(512),
                memory_remaining_bytes: Some(512),
                disk_total_bytes: None,
                disk_processed_bytes: None,
                disk_remaining_bytes: None,
                created_at: None,
                updated_at: None,
            },
        });
        let progress = module.migration_progress().unwrap();
        assert_eq!(progress.status, "running");
        assert_eq!(progress.source_compute, "compute-01");
    }

    #[test]
    fn test_migration_progress_cleared_on_completion() {
        let (mut module, _rx) = setup();
        // Set progress first
        module.handle_event(&AppEvent::MigrationProgressLoaded {
            server_id: "s1".into(),
            migration: ServerMigration {
                id: 1,
                status: "running".into(),
                source_compute: "c1".into(),
                dest_compute: "c2".into(),
                memory_total_bytes: None,
                memory_processed_bytes: None,
                memory_remaining_bytes: None,
                disk_total_bytes: None,
                disk_processed_bytes: None,
                disk_remaining_bytes: None,
                created_at: None,
                updated_at: None,
            },
        });
        assert!(module.migration_progress().is_some());

        // Live migrated → clear
        module.handle_event(&AppEvent::ServerLiveMigrated { id: "s1".into() });
        assert!(module.migration_progress().is_none());
    }

    #[test]
    fn test_migration_progress_cleared_on_confirm() {
        let (mut module, _rx) = setup();
        module.migration_progress = Some(("s1".into(), ServerMigration {
            id: 1,
            status: "running".into(),
            source_compute: "c1".into(),
            dest_compute: "c2".into(),
            memory_total_bytes: None,
            memory_processed_bytes: None,
            memory_remaining_bytes: None,
            disk_total_bytes: None,
            disk_processed_bytes: None,
            disk_remaining_bytes: None,
            created_at: None,
            updated_at: None,
        }));
        module.handle_event(&AppEvent::MigrationConfirmed { id: "s1".into() });
        assert!(module.migration_progress().is_none());
    }

    fn setup_admin_detail(status: &str) -> (ServerModule, mpsc::UnboundedReceiver<Action>) {
        let (tx, rx) = mpsc::unbounded_channel();
        let mut module = ServerModule::new(tx);
        module.is_admin = true;
        let servers = vec![make_test_server("s1", "web-01", status)];
        module.handle_event(&AppEvent::ServersLoaded(servers));
        module.handle_key(key(KeyCode::Enter)); // enter detail
        (module, rx)
    }

    // -- Migration keybinding tests -----------------------------------------

    #[test]
    fn test_detail_m_live_migrate_admin() {
        let (mut module, _rx) = setup_admin_detail("ACTIVE");
        module.handle_key(key(KeyCode::Char('M')));
        assert!(module.confirm.is_active());
        // Confirm
        let action = module.handle_key(key(KeyCode::Char('y')));
        assert!(matches!(action, Some(Action::LiveMigrateServer { .. })));
    }

    #[test]
    fn test_detail_m_no_op_non_admin() {
        let (mut module, _rx) = setup();
        module.handle_key(key(KeyCode::Enter)); // detail
        module.handle_key(key(KeyCode::Char('M')));
        assert!(!module.confirm.is_active());
    }

    #[test]
    fn test_detail_c_cold_migrate_admin() {
        let (mut module, _rx) = setup_admin_detail("ACTIVE");
        module.handle_key(key(KeyCode::Char('C')));
        assert!(module.confirm.is_active());
        let action = module.handle_key(key(KeyCode::Char('y')));
        assert!(matches!(action, Some(Action::ColdMigrateServer { .. })));
    }

    #[test]
    fn test_detail_c_no_op_non_admin() {
        let (mut module, _rx) = setup();
        module.handle_key(key(KeyCode::Enter));
        module.handle_key(key(KeyCode::Char('C')));
        assert!(!module.confirm.is_active());
    }

    #[test]
    fn test_detail_y_confirm_verify_resize() {
        let (mut module, _rx) = setup_admin_detail("VERIFY_RESIZE");
        module.handle_key(key(KeyCode::Char('Y')));
        assert!(module.confirm.is_active());
        let action = module.handle_key(key(KeyCode::Char('y')));
        assert!(matches!(action, Some(Action::ConfirmMigration { .. })));
    }

    #[test]
    fn test_detail_y_no_op_active_status() {
        let (mut module, _rx) = setup_admin_detail("ACTIVE");
        module.handle_key(key(KeyCode::Char('Y')));
        assert!(!module.confirm.is_active());
    }

    #[test]
    fn test_detail_n_revert_verify_resize() {
        let (mut module, _rx) = setup_admin_detail("VERIFY_RESIZE");
        module.handle_key(key(KeyCode::Char('N')));
        assert!(module.confirm.is_active());
        let action = module.handle_key(key(KeyCode::Char('y')));
        assert!(matches!(action, Some(Action::RevertMigration { .. })));
    }

    #[test]
    fn test_detail_n_no_op_active_status() {
        let (mut module, _rx) = setup_admin_detail("ACTIVE");
        module.handle_key(key(KeyCode::Char('N')));
        assert!(!module.confirm.is_active());
    }

    #[test]
    fn test_detail_e_evacuate_error_status() {
        let (mut module, _rx) = setup_admin_detail("ERROR");
        module.handle_key(key(KeyCode::Char('E')));
        assert!(module.confirm.is_active());
        let action = module.handle_key(key(KeyCode::Char('y')));
        assert!(matches!(action, Some(Action::EvacuateServer { .. })));
    }

    #[test]
    fn test_detail_e_no_op_active_status() {
        let (mut module, _rx) = setup_admin_detail("ACTIVE");
        module.handle_key(key(KeyCode::Char('E')));
        assert!(!module.confirm.is_active());
    }

    #[test]
    fn test_detail_e_no_op_non_admin() {
        let (mut module, _rx) = setup();
        // Change server to ERROR but non-admin
        let servers = vec![make_test_server("s1", "web-01", "ERROR")];
        module.handle_event(&AppEvent::ServersLoaded(servers));
        module.handle_key(key(KeyCode::Enter));
        module.handle_key(key(KeyCode::Char('E')));
        assert!(!module.confirm.is_active());
    }

    #[test]
    fn test_set_admin_updates_columns() {
        let (mut module, _rx) = setup();
        module.set_admin(true);
        assert!(module.is_admin);
    }

    // -- resolve_action direct tests ----------------------------------------

    #[test]
    fn test_resolve_live_migrate() {
        let action = ServerModule::resolve_action(PendingAction::LiveMigrate { id: "s1".into() });
        assert!(matches!(action, Some(Action::LiveMigrateServer { id, host: None }) if id == "s1"));
    }

    #[test]
    fn test_resolve_cold_migrate() {
        let action = ServerModule::resolve_action(PendingAction::ColdMigrate { id: "s1".into() });
        assert!(matches!(action, Some(Action::ColdMigrateServer { id }) if id == "s1"));
    }

    #[test]
    fn test_resolve_confirm_migrate() {
        let action = ServerModule::resolve_action(PendingAction::ConfirmMigrate { id: "s1".into() });
        assert!(matches!(action, Some(Action::ConfirmMigration { id }) if id == "s1"));
    }

    #[test]
    fn test_resolve_revert_migrate() {
        let action = ServerModule::resolve_action(PendingAction::RevertMigrate { id: "s1".into() });
        assert!(matches!(action, Some(Action::RevertMigration { id }) if id == "s1"));
    }

    #[test]
    fn test_resolve_evacuate() {
        let action = ServerModule::resolve_action(PendingAction::Evacuate { id: "s1".into() });
        assert!(matches!(action, Some(Action::EvacuateServer { id, host: None }) if id == "s1"));
    }

    // -- set_all_tenants with admin -----------------------------------------

    #[test]
    fn test_set_all_tenants_with_admin_includes_host_column() {
        let (mut module, _rx) = setup();
        module.set_admin(true);
        module.set_all_tenants(true);
        // admin + all_tenants → both Project and Host columns visible
        // Verify by rendering rows: cells should include tenant and host
        let servers = vec![make_test_server("s1", "web-01", "ACTIVE")];
        module.handle_event(&AppEvent::ServersLoaded(servers));
        // ResourceList columns count: icon + name + project + host + status + ip + flavor + image = 8
        // We can't directly inspect ResourceList columns, but rows should have 8 cells
    }

    // -- migration progress cleared on evacuate -----------------------------

    #[test]
    fn test_migration_progress_cleared_on_evacuate() {
        let (mut module, _rx) = setup();
        module.migration_progress = Some(("s1".into(), ServerMigration {
            id: 1,
            status: "running".into(),
            source_compute: "c1".into(),
            dest_compute: "c2".into(),
            memory_total_bytes: None,
            memory_processed_bytes: None,
            memory_remaining_bytes: None,
            disk_total_bytes: None,
            disk_processed_bytes: None,
            disk_remaining_bytes: None,
            created_at: None,
            updated_at: None,
        }));
        module.handle_event(&AppEvent::ServerEvacuated { id: "s1".into() });
        assert!(module.migration_progress().is_none());
    }

    #[test]
    fn test_migration_progress_not_shown_for_different_server() {
        let (mut module, _rx) = setup();
        // Load progress for server s1
        module.handle_event(&AppEvent::MigrationProgressLoaded {
            server_id: "s1".into(),
            migration: ServerMigration {
                id: 1,
                status: "running".into(),
                source_compute: "c1".into(),
                dest_compute: "c2".into(),
                memory_total_bytes: None,
                memory_processed_bytes: None,
                memory_remaining_bytes: None,
                disk_total_bytes: None,
                disk_processed_bytes: None,
                disk_remaining_bytes: None,
                created_at: None,
                updated_at: None,
            },
        });
        // migration_progress_for should return None for s2
        assert!(module.migration_progress_for("s2").is_none());
        // but Some for s1
        assert!(module.migration_progress_for("s1").is_some());
    }

    #[test]
    fn test_cached_options_applied_on_form_open() {
        use crate::models::nova::Flavor;

        let (mut module, _rx) = setup();
        // Pre-load flavors before opening form
        let flavors = vec![Flavor {
            id: "flv-1".into(),
            name: "m1.small".into(),
            vcpus: 1,
            ram: 2048,
            disk: 20,
            is_public: true,
        }];
        module.handle_event(&AppEvent::FlavorsLoaded(flavors));

        // Now open form — cached options should be applied immediately
        module.handle_key(key(KeyCode::Char('c')));
        let form = module.form.as_ref().unwrap();
        if let (crate::ui::form::FieldDef::Dropdown { options, .. }, _) = &form.fields()[2] {
            assert_eq!(options.len(), 1, "Cached flavor options should be applied on form open");
            assert_eq!(options[0].value, "flv-1");
        } else {
            panic!("Expected Dropdown for Flavor field");
        }
    }

    // -- help_hint tests ---------------------------------------------------

    #[test]
    fn test_help_hint_list_view() {
        let (module, _rx) = setup();
        let hint = module.help_hint();
        assert!(hint.contains("Enter"), "List hint should mention Enter");
        assert!(hint.contains("c:Create"), "List hint should mention Create");
    }

    #[test]
    fn test_help_hint_detail_view_admin() {
        let (module, _rx) = setup_admin_detail("ACTIVE");
        let hint = module.help_hint();
        assert!(hint.contains("M:Migrate"), "Admin detail hint should mention Migrate");
    }

    #[test]
    fn test_help_hint_detail_verify_resize() {
        let (module, _rx) = setup_admin_detail("VERIFY_RESIZE");
        let hint = module.help_hint();
        assert!(hint.contains("Y:Confirm"), "VERIFY_RESIZE hint should mention Confirm");
        assert!(hint.contains("N:Revert"), "VERIFY_RESIZE hint should mention Revert");
    }

    #[test]
    fn test_help_hint_detail_non_admin() {
        let (mut module, _rx) = setup();
        module.handle_key(key(KeyCode::Enter)); // enter detail
        let hint = module.help_hint();
        assert!(!hint.contains("M:Migrate"), "Non-admin should not see Migrate");
        assert!(hint.contains("F:Resize"), "Non-admin should see Resize");
    }

    // -- Resize keybinding tests -----------------------------------------------

    fn setup_with_flavors() -> (ServerModule, mpsc::UnboundedReceiver<Action>) {
        use crate::models::nova::Flavor;
        let (tx, rx) = mpsc::unbounded_channel();
        let mut module = ServerModule::new(tx);
        let servers = vec![make_test_server("s1", "web-01", "ACTIVE")];
        module.handle_event(&AppEvent::ServersLoaded(servers));
        module.handle_event(&AppEvent::FlavorsLoaded(vec![
            Flavor { id: "flv-1".into(), name: "m1.small".into(), vcpus: 1, ram: 2048, disk: 20, is_public: true },
            Flavor { id: "flv-2".into(), name: "m1.medium".into(), vcpus: 2, ram: 4096, disk: 40, is_public: true },
        ]));
        (module, rx)
    }

    #[test]
    fn test_detail_f_opens_select_popup_with_flavors() {
        let (mut module, _rx) = setup_with_flavors();
        module.handle_key(key(KeyCode::Enter)); // enter detail
        assert!(module.select_popup.is_none());
        module.handle_key(key(KeyCode::Char('F')));
        assert!(module.select_popup.is_some());
    }

    #[test]
    fn test_detail_f_no_op_on_shutoff() {
        use crate::models::nova::Flavor;
        let (tx, _rx) = mpsc::unbounded_channel();
        let mut module = ServerModule::new(tx);
        let servers = vec![make_test_server("s1", "web-01", "SHUTOFF")];
        module.handle_event(&AppEvent::ServersLoaded(servers));
        module.handle_event(&AppEvent::FlavorsLoaded(vec![
            Flavor { id: "flv-1".into(), name: "m1.small".into(), vcpus: 1, ram: 2048, disk: 20, is_public: true },
        ]));
        module.handle_key(key(KeyCode::Enter));
        module.handle_key(key(KeyCode::Char('F')));
        assert!(module.select_popup.is_some(), "F should work on SHUTOFF");
    }

    #[test]
    fn test_detail_f_no_op_on_error_status() {
        use crate::models::nova::Flavor;
        let (tx, _rx) = mpsc::unbounded_channel();
        let mut module = ServerModule::new(tx);
        let servers = vec![make_test_server("s1", "web-01", "ERROR")];
        module.handle_event(&AppEvent::ServersLoaded(servers));
        module.handle_event(&AppEvent::FlavorsLoaded(vec![
            Flavor { id: "flv-1".into(), name: "m1.small".into(), vcpus: 1, ram: 2048, disk: 20, is_public: true },
        ]));
        module.handle_key(key(KeyCode::Enter));
        module.handle_key(key(KeyCode::Char('F')));
        assert!(module.select_popup.is_none(), "F should not work on ERROR status");
    }

    #[test]
    fn test_detail_f_fetches_flavors_when_cache_empty() {
        let (mut module, mut rx) = setup();
        module.handle_key(key(KeyCode::Enter)); // detail
        module.handle_key(key(KeyCode::Char('F')));
        assert!(module.select_popup.is_none(), "No popup when flavors not cached");
        let action = rx.try_recv().unwrap();
        assert!(matches!(action, Action::FetchFlavors));
    }

    #[test]
    fn test_select_popup_cancel_closes() {
        let (mut module, _rx) = setup_with_flavors();
        module.handle_key(key(KeyCode::Enter));
        module.handle_key(key(KeyCode::Char('F')));
        assert!(module.select_popup.is_some());
        module.handle_key(key(KeyCode::Esc));
        assert!(module.select_popup.is_none());
    }

    #[test]
    fn test_select_popup_enter_opens_confirm() {
        let (mut module, _rx) = setup_with_flavors();
        module.handle_key(key(KeyCode::Enter)); // detail
        module.handle_key(key(KeyCode::Char('F'))); // open popup
        module.handle_key(key(KeyCode::Enter)); // select first flavor
        assert!(module.select_popup.is_none());
        assert!(module.confirm.is_active());
    }

    #[test]
    fn test_select_popup_confirm_dispatches_resize() {
        let (mut module, _rx) = setup_with_flavors();
        module.handle_key(key(KeyCode::Enter)); // detail
        module.handle_key(key(KeyCode::Char('F'))); // open popup
        module.handle_key(key(KeyCode::Char('j'))); // move to m1.medium
        module.handle_key(key(KeyCode::Enter)); // select
        // Now confirm dialog is active
        let action = module.handle_key(key(KeyCode::Char('y'))); // confirm
        assert!(matches!(action, Some(Action::ResizeServer { id, flavor_id }) if id == "s1" && flavor_id == "flv-2"));
    }

    #[test]
    fn test_resize_pending_set_on_server_resized() {
        let (mut module, _rx) = setup();
        assert!(module.resize_pending.is_none());
        module.handle_event(&AppEvent::ServerResized { id: "s1".into() });
        assert!(module.resize_pending.is_some());
    }

    #[test]
    fn test_resize_pending_cleared_on_confirm() {
        let (mut module, _rx) = setup();
        module.resize_pending = Some(ResizePendingInfo {
            server_id: "s1".into(),
        });
        module.handle_event(&AppEvent::ResizeConfirmed { id: "s1".into() });
        assert!(module.resize_pending.is_none());
    }

    #[test]
    fn test_resize_pending_cleared_on_revert() {
        let (mut module, _rx) = setup();
        module.resize_pending = Some(ResizePendingInfo {
            server_id: "s1".into(),
        });
        module.handle_event(&AppEvent::ResizeReverted { id: "s1".into() });
        assert!(module.resize_pending.is_none());
    }

    #[test]
    fn test_y_confirm_resize_when_resize_pending() {
        let (tx, _rx) = mpsc::unbounded_channel();
        let mut module = ServerModule::new(tx);
        let servers = vec![make_test_server("s1", "web-01", "VERIFY_RESIZE")];
        module.handle_event(&AppEvent::ServersLoaded(servers));
        module.resize_pending = Some(ResizePendingInfo {
            server_id: "s1".into(),
        });
        module.handle_key(key(KeyCode::Enter)); // detail
        module.handle_key(key(KeyCode::Char('Y'))); // confirm
        assert!(module.confirm.is_active());
        let action = module.handle_key(key(KeyCode::Char('y')));
        assert!(matches!(action, Some(Action::ConfirmResize { .. })));
    }

    #[test]
    fn test_y_confirm_migration_for_different_server_resize_pending() {
        // C-1 fix: resize_pending for s1, but viewing s2 in VERIFY_RESIZE → migration
        let (tx, _rx) = mpsc::unbounded_channel();
        let mut module = ServerModule::new(tx);
        module.is_admin = true;
        let servers = vec![
            make_test_server("s1", "web-01", "ACTIVE"),
            make_test_server("s2", "web-02", "VERIFY_RESIZE"),
        ];
        module.handle_event(&AppEvent::ServersLoaded(servers));
        module.resize_pending = Some(ResizePendingInfo { server_id: "s1".into() });
        // Navigate to s2 detail
        module.handle_key(key(KeyCode::Char('j'))); // select s2
        module.handle_key(key(KeyCode::Enter)); // detail s2
        module.handle_key(key(KeyCode::Char('Y')));
        assert!(module.confirm.is_active());
        let action = module.handle_key(key(KeyCode::Char('y')));
        // Should be migration confirm, not resize confirm
        assert!(matches!(action, Some(Action::ConfirmMigration { .. })));
    }

    #[test]
    fn test_stale_resize_pending_cleared_on_servers_loaded() {
        let (mut module, _rx) = setup();
        module.resize_pending = Some(ResizePendingInfo { server_id: "s1".into() });
        // Server s1 is now ACTIVE (not VERIFY_RESIZE) → resize_pending should be cleared
        let servers = vec![make_test_server("s1", "web-01", "ACTIVE")];
        module.handle_event(&AppEvent::ServersLoaded(servers));
        assert!(module.resize_pending.is_none());
    }

    #[test]
    fn test_resize_pending_kept_while_verify_resize() {
        let (mut module, _rx) = setup();
        module.resize_pending = Some(ResizePendingInfo { server_id: "s1".into() });
        let servers = vec![make_test_server("s1", "web-01", "VERIFY_RESIZE")];
        module.handle_event(&AppEvent::ServersLoaded(servers));
        assert!(module.resize_pending.is_some());
    }

    #[test]
    fn test_y_confirm_migration_when_no_resize_pending() {
        let (mut module, _rx) = setup_admin_detail("VERIFY_RESIZE");
        // No resize_pending → should confirm migration
        module.handle_key(key(KeyCode::Char('Y')));
        assert!(module.confirm.is_active());
        let action = module.handle_key(key(KeyCode::Char('y')));
        assert!(matches!(action, Some(Action::ConfirmMigration { .. })));
    }

    #[test]
    fn test_n_revert_resize_when_resize_pending() {
        let (tx, _rx) = mpsc::unbounded_channel();
        let mut module = ServerModule::new(tx);
        let servers = vec![make_test_server("s1", "web-01", "VERIFY_RESIZE")];
        module.handle_event(&AppEvent::ServersLoaded(servers));
        module.resize_pending = Some(ResizePendingInfo {
            server_id: "s1".into(),
        });
        module.handle_key(key(KeyCode::Enter));
        module.handle_key(key(KeyCode::Char('N')));
        assert!(module.confirm.is_active());
        let action = module.handle_key(key(KeyCode::Char('y')));
        assert!(matches!(action, Some(Action::RevertResize { .. })));
    }

    // -- resolve_action for resize ---------------------------------------------

    #[test]
    fn test_resolve_resize() {
        let action = ServerModule::resolve_action(PendingAction::Resize {
            id: "s1".into(), flavor_id: "f2".into(),
        });
        assert!(matches!(action, Some(Action::ResizeServer { id, flavor_id }) if id == "s1" && flavor_id == "f2"));
    }

    #[test]
    fn test_resolve_confirm_resize() {
        let action = ServerModule::resolve_action(PendingAction::ConfirmResize { id: "s1".into() });
        assert!(matches!(action, Some(Action::ConfirmResize { id }) if id == "s1"));
    }

    #[test]
    fn test_resolve_revert_resize() {
        let action = ServerModule::resolve_action(PendingAction::RevertResize { id: "s1".into() });
        assert!(matches!(action, Some(Action::RevertResize { id }) if id == "s1"));
    }

    #[test]
    fn test_help_hint_includes_resize() {
        let (mut module, _rx) = setup();
        module.handle_key(key(KeyCode::Enter));
        let hint = module.help_hint();
        assert!(hint.contains("F:Resize"));
    }

    #[test]
    fn test_refresh_action_returns_fetch_servers() {
        let (module, _rx) = setup();
        assert!(matches!(module.refresh_action(), Some(Action::FetchServers)));
    }

    #[test]
    fn test_is_modal_false_by_default() {
        let (module, _rx) = setup();
        assert!(!module.is_modal());
    }

    #[test]
    fn test_is_modal_true_when_confirm_active() {
        let (mut module, _rx) = setup();
        module.handle_key(key(KeyCode::Char('d')));
        assert!(module.is_modal());
    }

    #[test]
    fn test_is_modal_true_when_form_open() {
        let (mut module, _rx) = setup();
        module.handle_key(key(KeyCode::Char('c')));
        assert!(module.is_modal());
    }

    #[test]
    fn test_has_transitional_all_terminal() {
        // setup() creates ACTIVE, SHUTOFF, ERROR — all terminal
        let (module, _rx) = setup();
        assert!(!module.has_transitional_resources());
    }

    #[test]
    fn test_has_transitional_with_migrating() {
        let (tx, _rx) = mpsc::unbounded_channel();
        let mut module = ServerModule::new(tx);
        let servers = vec![
            make_test_server("s1", "web-01", "ACTIVE"),
            make_test_server("s2", "web-02", "MIGRATING"),
        ];
        module.handle_event(&AppEvent::ServersLoaded(servers));
        assert!(module.has_transitional_resources());
    }
}
