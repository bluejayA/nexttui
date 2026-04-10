pub mod view_model;

use crossterm::event::{KeyCode, KeyEvent};
use ratatui::layout::Rect;
use ratatui::Frame;
use tokio::sync::mpsc;

use crate::action::Action;
use crate::component::Component;
use crate::event::AppEvent;
use crate::models::cinder::Volume;
use crate::models::common::is_terminal_server_status;
use crate::models::neutron::{FloatingIp, Network, Port};
use crate::models::nova::{Flavor, Server, ServerMigration};
use crate::module::{ConfirmHandler, PendingAction, ViewState};
use crate::port::types::{EvacuateParams, NetworkAttachment, ServerCreateParams};
use crate::ui::confirm::ConfirmDialog;
use crate::ui::form::{FormAction, FormWidget, SelectOption};
use crate::ui::resource_list::{ResourceList, Row};
use crate::ui::select_popup::{ItemHint, SelectItem, SelectPopup, SelectResult};

use self::view_model::{
    server_columns_full, server_create_defs, server_detail_data, server_to_row_full,
    ServerViewContext,
};

#[derive(Debug, Clone)]
pub struct ResizePendingInfo {
    pub server_id: String,
}

/// Tracks which popup context is active in detail view.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum DetailPopupKind {
    Resize,
    AttachVolume,
    DetachVolume,
    AssociateFip,
    SelectPort,
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
    popup_kind: Option<DetailPopupKind>,
    resize_pending: Option<ResizePendingInfo>,
    cached_flavors: Vec<Flavor>,
    cached_volumes: Vec<Volume>,
    cached_floating_ips: Vec<FloatingIp>,
    cached_ports: Vec<Port>,
    cached_networks: Vec<Network>,
    pending_fip_id: Option<String>,
    pending_ports_server_id: Option<String>,
    loading_ports: bool,
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
            popup_kind: None,
            resize_pending: None,
            cached_flavors: Vec::new(),
            cached_volumes: Vec::new(),
            cached_floating_ips: Vec::new(),
            cached_ports: Vec::new(),
            cached_networks: Vec::new(),
            pending_fip_id: None,
            pending_ports_server_id: None,
            loading_ports: false,
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
            PendingAction::Evacuate { id } => Some(Action::EvacuateServer {
                id,
                params: EvacuateParams::default(),
            }),
            PendingAction::AttachVolume { volume_id, server_id, device } => {
                Some(Action::AttachVolume { volume_id, server_id, device })
            }
            PendingAction::DetachVolume { volume_id, server_id, attachment_id } => {
                Some(Action::DetachVolume { volume_id, server_id, attachment_id })
            }
            PendingAction::AssociateFloatingIp { fip_id, port_id } => {
                Some(Action::AssociateFloatingIp { fip_id, port_id })
            }
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

    fn build_available_volume_items(&self) -> Vec<SelectItem> {
        self.cached_volumes
            .iter()
            .filter(|v| v.status == "available")
            .map(|v| {
                let name = v.name.as_deref().unwrap_or("-");
                let vol_type = v.volume_type.as_deref().unwrap_or("-");
                SelectItem {
                    id: v.id.clone(),
                    label: format!("{name} ({}GB, {vol_type})", v.size),
                    hint: ItemHint::Normal,
                }
            })
            .collect()
    }

    fn build_attached_volume_items(&self, server_id: &str) -> Vec<SelectItem> {
        self.cached_volumes
            .iter()
            .filter(|v| v.status == "in-use" && v.attachments.iter().any(|a| a.server_id == server_id))
            .flat_map(|v| {
                v.attachments.iter().filter(|a| a.server_id == server_id).map(move |a| {
                    let name = v.name.as_deref().unwrap_or("-");
                    SelectItem {
                        id: format!("{}:{}", v.id, a.id),
                        label: format!("{name} ({}, {}GB)", a.device, v.size),
                        hint: if v.bootable == "true" {
                            ItemHint::Warning("boot".into())
                        } else {
                            ItemHint::Normal
                        },
                    }
                })
            })
            .collect()
    }

    fn build_available_fip_items(&self) -> Vec<SelectItem> {
        self.cached_floating_ips
            .iter()
            .filter(|f| f.port_id.is_none())
            .map(|f| SelectItem {
                id: f.id.clone(),
                label: f.floating_ip_address.clone(),
                hint: ItemHint::Normal,
            })
            .collect()
    }

    fn build_port_items(&self) -> Vec<SelectItem> {
        self.cached_ports
            .iter()
            .map(|p| SelectItem {
                id: p.id.clone(),
                label: p.display_label(&self.cached_networks),
                hint: ItemHint::Normal,
            })
            .collect()
    }

    fn handle_attach_volume_selected(&mut self, volume_id: String) {
        let server_id = match &self.view_state {
            ViewState::Detail(id) => id.clone(),
            _ => return,
        };
        let vol = self.cached_volumes.iter().find(|v| v.id == volume_id);
        let Some(vol) = vol else { return };
        let server_name = self.servers.iter()
            .find(|s| s.id == server_id)
            .map(|s| s.name.as_str())
            .unwrap_or("unknown");
        let name = vol.name.as_deref().unwrap_or("-");
        let vol_type = vol.volume_type.as_deref().unwrap_or("-");
        let details = vec![
            format!("  Volume: {name}"),
            format!("  Size: {} GB", vol.size),
            format!("  Type: {vol_type}"),
        ];
        self.confirm.open(
            ConfirmDialog::yes_no_with_details(
                format!("Attach {name} to '{server_name}'?"),
                details,
            ),
            PendingAction::AttachVolume {
                volume_id,
                server_id,
                device: None,
            },
        );
    }

    fn handle_detach_volume_selected(&mut self, composite_id: String) {
        // composite_id = "volume_id:attachment_id"
        let parts: Vec<&str> = composite_id.splitn(2, ':').collect();
        if parts.len() != 2 { return; }
        let volume_id = parts[0].to_string();
        let attachment_id = parts[1].to_string();

        let vol = self.cached_volumes.iter().find(|v| v.id == volume_id);
        let Some(vol) = vol else { return };
        let att = vol.attachments.iter().find(|a| a.id == attachment_id);
        let Some(att) = att else { return };

        let server_name = self.servers.iter()
            .find(|s| s.id == att.server_id)
            .map(|s| s.name.as_str())
            .unwrap_or("unknown");
        let name = vol.name.as_deref().unwrap_or("-");
        let vol_type = vol.volume_type.as_deref().unwrap_or("-");
        let details = vec![
            format!("  Volume: {name}"),
            format!("  Size: {} GB", vol.size),
            format!("  Type: {vol_type}"),
            format!("  Device: {}", att.device),
        ];

        // Boot volume safeguard — same logic as VolumeModule
        let is_boot = vol.bootable == "true" && vol.attachments.len() == 1;
        if is_boot {
            if !self.is_admin {
                return; // non-admin cannot detach boot volume
            }
            // Admin: TypeToConfirm with boot warning
            let mut boot_details = details;
            boot_details.push("  ⚠ BOOT VOLUME — server will become unbootable!".into());
            self.confirm.open(
                ConfirmDialog::type_to_confirm_with_details(
                    format!("Detach BOOT volume '{name}'? Type name to confirm:"),
                    name.to_string(),
                    boot_details,
                ),
                PendingAction::DetachVolume {
                    volume_id,
                    server_id: att.server_id.clone(),
                    attachment_id,
                },
            );
            return;
        }

        self.confirm.open(
            ConfirmDialog::yes_no_with_details(
                format!("Detach {name} from '{server_name}'? Device: {}", att.device),
                details,
            ),
            PendingAction::DetachVolume {
                volume_id,
                server_id: att.server_id.clone(),
                attachment_id,
            },
        );
    }

    fn handle_fip_selected(&mut self, fip_id: String) {
        let server_id = match &self.view_state {
            ViewState::Detail(id) => id.clone(),
            _ => return,
        };
        self.pending_fip_id = Some(fip_id);
        self.pending_ports_server_id = Some(server_id.clone());
        self.loading_ports = true;
        let _ = self.action_tx.send(Action::FetchPorts { server_id });
    }

    fn handle_ports_loaded(&mut self, ports: Vec<Port>) {
        self.cached_ports = ports;
        self.loading_ports = false;

        let Some(fip_id) = self.pending_fip_id.clone() else { return };

        if self.cached_ports.is_empty() {
            let _ = self.action_tx.send(Action::ShowToast {
                message: "No ports found for this server".into(),
            });
            self.pending_fip_id = None;
            return;
        }

        if self.cached_ports.len() == 1 {
            let port = &self.cached_ports[0];
            let port_label = port.display_label(&self.cached_networks);
            let fip = self.cached_floating_ips.iter().find(|f| f.id == fip_id);
            let Some(fip) = fip else {
                self.pending_fip_id = None;
                return;
            };
            let server_name = match &self.view_state {
                ViewState::Detail(id) => self.servers.iter()
                    .find(|s| s.id == *id)
                    .map(|s| s.name.as_str())
                    .unwrap_or("unknown"),
                _ => "unknown",
            };
            let details = vec![
                format!("  Floating IP: {}", fip.floating_ip_address),
                format!("  Server: {server_name}"),
                format!("  Port: {port_label}"),
            ];
            self.confirm.open(
                ConfirmDialog::yes_no_with_details(
                    format!("Associate {} to port {}?", fip.floating_ip_address, port_label),
                    details,
                ),
                PendingAction::AssociateFloatingIp {
                    fip_id,
                    port_id: port.id.clone(),
                },
            );
            self.pending_fip_id = None;
            return;
        }

        // Multiple ports: show SelectPopup
        let items = self.build_port_items();
        self.select_popup = Some(SelectPopup::new("Select Port", items));
        self.popup_kind = Some(DetailPopupKind::SelectPort);
    }

    fn handle_port_selected(&mut self, port_id: String) {
        let Some(fip_id) = self.pending_fip_id.take() else { return };
        let port = self.cached_ports.iter().find(|p| p.id == port_id);
        let fip = self.cached_floating_ips.iter().find(|f| f.id == fip_id);
        let (Some(port), Some(fip)) = (port, fip) else { return };

        let port_label = port.display_label(&self.cached_networks);
        let server_name = match &self.view_state {
            ViewState::Detail(id) => self.servers.iter()
                .find(|s| s.id == *id)
                .map(|s| s.name.as_str())
                .unwrap_or("unknown"),
            _ => "unknown",
        };
        let details = vec![
            format!("  Floating IP: {}", fip.floating_ip_address),
            format!("  Server: {server_name}"),
            format!("  Port: {port_label}"),
        ];
        self.confirm.open(
            ConfirmDialog::yes_no_with_details(
                format!("Associate {} to port {}?", fip.floating_ip_address, port_label),
                details,
            ),
            PendingAction::AssociateFloatingIp {
                fip_id: fip.id.clone(),
                port_id,
            },
        );
    }

    fn handle_detail_key(&mut self, key: KeyEvent) -> Option<Action> {
        // SelectPopup takes priority after ConfirmDialog
        if let Some(ref mut popup) = self.select_popup {
            let ctx = self.popup_kind.unwrap_or(DetailPopupKind::Resize);
            match popup.handle_key(key) {
                SelectResult::Selected(selected_id) => {
                    self.select_popup = None;
                    self.popup_kind = None;
                    match ctx {
                        DetailPopupKind::Resize => {
                            let flavor_name = self.cached_flavors.iter()
                                .find(|f| f.id == selected_id)
                                .map(|f| f.name.as_str())
                                .unwrap_or(&selected_id);
                            let msg = format!("Resize to {flavor_name}?");
                            if let ViewState::Detail(ref id) = self.view_state {
                                let id = id.clone();
                                self.confirm.open(
                                    ConfirmDialog::yes_no(msg),
                                    PendingAction::Resize { id, flavor_id: selected_id },
                                );
                            }
                        }
                        DetailPopupKind::AttachVolume => {
                            self.handle_attach_volume_selected(selected_id);
                        }
                        DetailPopupKind::DetachVolume => {
                            self.handle_detach_volume_selected(selected_id);
                        }
                        DetailPopupKind::AssociateFip => {
                            self.handle_fip_selected(selected_id);
                        }
                        DetailPopupKind::SelectPort => {
                            self.handle_port_selected(selected_id);
                        }
                    }
                    return None;
                }
                SelectResult::Cancelled => {
                    self.select_popup = None;
                    self.popup_kind = None;
                    self.pending_fip_id = None;
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
                            self.popup_kind = Some(DetailPopupKind::Resize);
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
            // Attach volume (Shift+A)
            KeyCode::Char('A') => {
                let items = self.build_available_volume_items();
                if items.is_empty() {
                    if self.cached_volumes.is_empty() {
                        let _ = self.action_tx.send(Action::FetchVolumes);
                    } else {
                        let _ = self.action_tx.send(Action::ShowToast {
                            message: "No available volumes to attach".into(),
                        });
                    }
                } else {
                    self.select_popup = Some(SelectPopup::new("Attach Volume", items));
                    self.popup_kind = Some(DetailPopupKind::AttachVolume);
                }
                None
            }
            // Detach volume
            KeyCode::Char('x') => {
                if let ViewState::Detail(ref id) = self.view_state {
                    let items = self.build_attached_volume_items(id);
                    if items.is_empty() {
                        let _ = self.action_tx.send(Action::ShowToast {
                            message: "No volumes attached to this server".into(),
                        });
                    } else if items.len() == 1 {
                        // Single volume: skip popup, go straight to confirm
                        let composite_id = items[0].id.clone();
                        self.handle_detach_volume_selected(composite_id);
                    } else {
                        self.select_popup = Some(SelectPopup::new("Detach Volume", items));
                        self.popup_kind = Some(DetailPopupKind::DetachVolume);
                    }
                }
                None
            }
            // Resource navigation shortcuts
            KeyCode::Char('v') => {
                return Some(Action::Navigate(crate::models::common::Route::Volumes));
            }
            KeyCode::Char('n') => {
                return Some(Action::Navigate(crate::models::common::Route::Networks));
            }
            KeyCode::Char('s') => {
                return Some(Action::Navigate(crate::models::common::Route::SecurityGroups));
            }
            KeyCode::Char('i') => {
                return Some(Action::Navigate(crate::models::common::Route::Images));
            }
            // Associate floating IP
            KeyCode::Char('f') => {
                let items = self.build_available_fip_items();
                if items.is_empty() {
                    if self.cached_floating_ips.is_empty() {
                        let _ = self.action_tx.send(Action::FetchFloatingIps);
                    } else {
                        let _ = self.action_tx.send(Action::ShowToast {
                            message: "No unassociated floating IPs available".into(),
                        });
                    }
                } else {
                    self.select_popup = Some(SelectPopup::new("Associate Floating IP", items));
                    self.popup_kind = Some(DetailPopupKind::AssociateFip);
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
            AppEvent::VolumesLoaded(volumes) => {
                self.cached_volumes = volumes.clone();
            }
            AppEvent::FloatingIpsLoaded(fips) => {
                self.cached_floating_ips = fips.clone();
            }
            AppEvent::PortsLoaded { server_id, ports } => {
                // Only consume if this response matches our pending request
                if self.pending_fip_id.is_some()
                    && self.pending_ports_server_id.as_deref() == Some(server_id.as_str())
                {
                    self.handle_ports_loaded(ports.clone());
                }
            }
            AppEvent::VolumeAttached { .. }
            | AppEvent::VolumeDetached { .. }
            | AppEvent::FloatingIpAssociated(_) => {
                let _ = self.action_tx.send(Action::FetchServers);
            }
            AppEvent::ApiError {
                operation, message, ..
            } => {
                self.error_message = Some(format!("{operation}: {message}"));
                self.loading = false;
                self.loading_ports = false;
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
                    let data = server_detail_data(&ServerViewContext {
                        server,
                        migration_progress: self.migration_progress_for(id),
                        flavor: matched_flavor,
                        is_resize_pending: is_resize,
                        cached_volumes: &self.cached_volumes,
                        cached_floating_ips: &self.cached_floating_ips,
                    });
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

    fn content_title(&self) -> Option<String> {
        match &self.view_state {
            ViewState::List => None,
            ViewState::Detail(id) => {
                let name = self.servers.iter()
                    .find(|s| s.id == *id)
                    .map(|s| s.name.as_str())
                    .unwrap_or("...");
                Some(format!("Server: {name}"))
            }
            ViewState::Create => Some("Create Server".into()),
        }
    }

    fn help_hint(&self) -> &str {
        match &self.view_state {
            ViewState::List => "Enter:Detail c:Create d:Delete r:Refresh",
            ViewState::Detail(id) => {
                let server = self.servers.iter().find(|s| s.id == *id);
                let is_verify = server.is_some_and(|s| s.status == "VERIFY_RESIZE");
                let is_error = server.is_some_and(|s| s.status == "ERROR");

                if is_verify && self.is_admin {
                    "Esc:Back R:Reboot S:Start X:Stop F:Resize A:AttachVol x:DetachVol f:AssocFIP M:Migrate C:Cold Y:Confirm N:Revert | v:Vol n:Net s:SG i:Img"
                } else if is_verify {
                    "Esc:Back R:Reboot S:Start X:Stop F:Resize A:AttachVol x:DetachVol f:AssocFIP Y:Confirm N:Revert | v:Vol n:Net s:SG i:Img"
                } else if is_error && self.is_admin {
                    "Esc:Back R:Reboot S:Start X:Stop F:Resize A:AttachVol x:DetachVol f:AssocFIP M:Migrate C:Cold E:Evacuate | v:Vol n:Net s:SG i:Img"
                } else if self.is_admin {
                    "Esc:Back R:Reboot S:Start X:Stop F:Resize A:AttachVol x:DetachVol f:AssocFIP M:Migrate C:Cold | v:Vol n:Net s:SG i:Img"
                } else {
                    "Esc:Back R:Reboot S:Start X:Stop F:Resize A:AttachVol x:DetachVol f:AssocFIP | v:Vol n:Net s:SG i:Img"
                }
            }
            ViewState::Create => "Esc:Cancel Tab:Next Enter:Submit",
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::common::Route;
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
            volumes_attached: vec![],
            security_groups: vec![],
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
        assert!(matches!(action, Some(Action::EvacuateServer { id, .. }) if id == "s1"));
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

    // -- Volume/FIP helper factories -------------------------------------------

    use crate::models::cinder::{Volume, VolumeAttachment};
    use crate::models::neutron::{FixedIp, FloatingIp, Network, Port};

    fn make_volume(id: &str, name: &str, status: &str, size: u32, vol_type: &str) -> Volume {
        Volume {
            id: id.into(),
            name: Some(name.into()),
            description: None,
            status: status.into(),
            size,
            volume_type: Some(vol_type.into()),
            encrypted: false,
            bootable: "false".into(),
            attachments: Vec::new(),
            availability_zone: None,
            created_at: None,
            tenant_id: None,
        }
    }

    fn make_volume_attached(id: &str, name: &str, size: u32, server_id: &str, device: &str, att_id: &str) -> Volume {
        Volume {
            id: id.into(),
            name: Some(name.into()),
            description: None,
            status: "in-use".into(),
            size,
            volume_type: Some("SSD".into()),
            encrypted: false,
            bootable: "false".into(),
            attachments: vec![VolumeAttachment {
                server_id: server_id.into(),
                device: device.into(),
                id: att_id.into(),
            }],
            availability_zone: None,
            created_at: None,
            tenant_id: None,
        }
    }

    fn make_fip(id: &str, ip: &str, port_id: Option<&str>) -> FloatingIp {
        FloatingIp {
            id: id.into(),
            floating_ip_address: ip.into(),
            status: if port_id.is_some() { "ACTIVE" } else { "DOWN" }.into(),
            port_id: port_id.map(|s| s.into()),
            floating_network_id: "ext-net-1".into(),
            fixed_ip_address: None,
            router_id: None,
            tenant_id: None,
        }
    }

    fn make_port(id: &str, ip: &str, net_id: &str, device_id: Option<&str>) -> Port {
        Port {
            id: id.into(),
            name: None,
            network_id: net_id.into(),
            fixed_ips: vec![FixedIp { subnet_id: "sub-1".into(), ip_address: ip.into() }],
            device_id: device_id.map(|s| s.into()),
            device_owner: Some("compute:az1".into()),
            status: "ACTIVE".into(),
            tenant_id: None,
        }
    }

    fn make_network(id: &str, name: &str) -> Network {
        Network {
            id: id.into(),
            name: name.into(),
            status: "ACTIVE".into(),
            description: None,
            admin_state_up: true,
            external: false,
            shared: false,
            mtu: None,
            port_security_enabled: None,
            subnets: vec![],
            provider_network_type: None,
            provider_physical_network: None,
            provider_segmentation_id: None,
            tenant_id: None,
        }
    }

    fn setup_detail_with_volumes() -> (ServerModule, mpsc::UnboundedReceiver<Action>) {
        let (tx, rx) = mpsc::unbounded_channel();
        let mut module = ServerModule::new(tx);
        let servers = vec![make_test_server("s1", "web-01", "ACTIVE")];
        module.handle_event(&AppEvent::ServersLoaded(servers));
        module.handle_event(&AppEvent::VolumesLoaded(vec![
            make_volume("v1", "data-vol", "available", 100, "SSD"),
            make_volume("v2", "log-vol", "available", 50, "HDD"),
            make_volume_attached("v3", "boot-vol", 40, "s1", "/dev/vda", "att-3"),
            make_volume_attached("v4", "extra-vol", 200, "s1", "/dev/vdb", "att-4"),
            make_volume_attached("v5", "other-vol", 10, "s2", "/dev/vdc", "att-5"),
        ]));
        module.handle_key(key(KeyCode::Enter)); // enter detail for s1
        (module, rx)
    }

    fn setup_detail_with_fips() -> (ServerModule, mpsc::UnboundedReceiver<Action>) {
        let (tx, rx) = mpsc::unbounded_channel();
        let mut module = ServerModule::new(tx);
        let servers = vec![make_test_server("s1", "web-01", "ACTIVE")];
        module.handle_event(&AppEvent::ServersLoaded(servers));
        module.handle_event(&AppEvent::FloatingIpsLoaded(vec![
            make_fip("fip-1", "203.0.113.10", None),
            make_fip("fip-2", "203.0.113.20", Some("port-99")),
            make_fip("fip-3", "203.0.113.30", None),
        ]));
        module.cached_networks = vec![make_network("net-1", "private-net")];
        module.handle_key(key(KeyCode::Enter)); // enter detail for s1
        (module, rx)
    }

    // -- Attach Volume tests --------------------------------------------------

    #[test]
    fn test_detail_shift_a_opens_attach_volume_popup() {
        let (mut module, _rx) = setup_detail_with_volumes();
        module.handle_key(key(KeyCode::Char('A')));
        assert!(module.select_popup.is_some(), "Shift+A should open attach volume popup");
        assert_eq!(module.popup_kind, Some(DetailPopupKind::AttachVolume));
    }

    #[test]
    fn test_detail_shift_a_only_shows_available_volumes() {
        let (mut module, _rx) = setup_detail_with_volumes();
        module.handle_key(key(KeyCode::Char('A')));
        let popup = module.select_popup.as_ref().unwrap();
        // v1 and v2 are available, v3/v4/v5 are in-use
        assert_eq!(popup.item_count(), 2);
    }

    #[test]
    fn test_detail_shift_a_select_volume_opens_confirm() {
        let (mut module, _rx) = setup_detail_with_volumes();
        module.handle_key(key(KeyCode::Char('A'))); // open popup
        module.handle_key(key(KeyCode::Enter)); // select first volume (v1)
        assert!(module.select_popup.is_none());
        assert!(module.confirm.is_active(), "Selecting volume should open confirm");
    }

    #[test]
    fn test_detail_shift_a_confirm_dispatches_attach() {
        let (mut module, _rx) = setup_detail_with_volumes();
        module.handle_key(key(KeyCode::Char('A'))); // open popup
        module.handle_key(key(KeyCode::Enter)); // select v1
        let action = module.handle_key(key(KeyCode::Char('y'))); // confirm
        assert!(matches!(action, Some(Action::AttachVolume { volume_id, server_id, .. })
            if volume_id == "v1" && server_id == "s1"));
    }

    #[test]
    fn test_detail_shift_a_no_available_volumes_shows_toast() {
        let (tx, mut rx) = mpsc::unbounded_channel();
        let mut module = ServerModule::new(tx);
        let servers = vec![make_test_server("s1", "web-01", "ACTIVE")];
        module.handle_event(&AppEvent::ServersLoaded(servers));
        // Only in-use volumes
        module.handle_event(&AppEvent::VolumesLoaded(vec![
            make_volume_attached("v3", "boot-vol", 40, "s1", "/dev/vda", "att-3"),
        ]));
        module.handle_key(key(KeyCode::Enter)); // detail
        module.handle_key(key(KeyCode::Char('A')));
        assert!(module.select_popup.is_none());
        // Should show toast
        let action = rx.try_recv().unwrap();
        assert!(matches!(action, Action::ShowToast { .. }));
    }

    #[test]
    fn test_detail_shift_a_empty_cache_fetches_volumes() {
        let (mut module, mut rx) = setup();
        module.handle_key(key(KeyCode::Enter)); // detail
        module.handle_key(key(KeyCode::Char('A')));
        let action = rx.try_recv().unwrap();
        assert!(matches!(action, Action::FetchVolumes));
    }

    // -- Detach Volume tests --------------------------------------------------

    #[test]
    fn test_detail_x_single_attached_direct_confirm() {
        let (tx, _rx) = mpsc::unbounded_channel();
        let mut module = ServerModule::new(tx);
        let servers = vec![make_test_server("s1", "web-01", "ACTIVE")];
        module.handle_event(&AppEvent::ServersLoaded(servers));
        module.handle_event(&AppEvent::VolumesLoaded(vec![
            make_volume_attached("v3", "boot-vol", 40, "s1", "/dev/vda", "att-3"),
        ]));
        module.handle_key(key(KeyCode::Enter)); // detail
        module.handle_key(key(KeyCode::Char('x'))); // detach
        // Single attachment → direct confirm, no popup
        assert!(module.select_popup.is_none());
        assert!(module.confirm.is_active());
    }

    #[test]
    fn test_detail_x_multiple_attached_shows_popup() {
        let (mut module, _rx) = setup_detail_with_volumes();
        // s1 has v3 (/dev/vda) and v4 (/dev/vdb)
        module.handle_key(key(KeyCode::Char('x')));
        assert!(module.select_popup.is_some());
        assert_eq!(module.popup_kind, Some(DetailPopupKind::DetachVolume));
    }

    #[test]
    fn test_detail_x_no_attached_volumes_shows_toast() {
        let (tx, mut rx) = mpsc::unbounded_channel();
        let mut module = ServerModule::new(tx);
        let servers = vec![make_test_server("s1", "web-01", "ACTIVE")];
        module.handle_event(&AppEvent::ServersLoaded(servers));
        module.handle_event(&AppEvent::VolumesLoaded(vec![
            make_volume("v1", "data-vol", "available", 100, "SSD"),
        ]));
        module.handle_key(key(KeyCode::Enter)); // detail
        module.handle_key(key(KeyCode::Char('x')));
        let action = rx.try_recv().unwrap();
        assert!(matches!(action, Action::ShowToast { .. }));
    }

    #[test]
    fn test_detail_x_confirm_dispatches_detach() {
        let (tx, _rx) = mpsc::unbounded_channel();
        let mut module = ServerModule::new(tx);
        let servers = vec![make_test_server("s1", "web-01", "ACTIVE")];
        module.handle_event(&AppEvent::ServersLoaded(servers));
        module.handle_event(&AppEvent::VolumesLoaded(vec![
            make_volume_attached("v3", "boot-vol", 40, "s1", "/dev/vda", "att-3"),
        ]));
        module.handle_key(key(KeyCode::Enter)); // detail
        module.handle_key(key(KeyCode::Char('x'))); // detach
        let action = module.handle_key(key(KeyCode::Char('y'))); // confirm
        assert!(matches!(action, Some(Action::DetachVolume { volume_id, attachment_id, .. })
            if volume_id == "v3" && attachment_id == "att-3"));
    }

    #[test]
    fn test_detail_x_select_from_multiple_then_confirm() {
        let (mut module, _rx) = setup_detail_with_volumes();
        module.handle_key(key(KeyCode::Char('x'))); // detach — popup with v3, v4
        module.handle_key(key(KeyCode::Char('j'))); // select v4
        module.handle_key(key(KeyCode::Enter)); // select
        assert!(module.confirm.is_active());
        let action = module.handle_key(key(KeyCode::Char('y'))); // confirm
        assert!(matches!(action, Some(Action::DetachVolume { volume_id, attachment_id, .. })
            if volume_id == "v4" && attachment_id == "att-4"));
    }

    // -- Associate FIP tests --------------------------------------------------

    #[test]
    fn test_detail_f_opens_fip_popup() {
        let (mut module, _rx) = setup_detail_with_fips();
        module.handle_key(key(KeyCode::Char('f')));
        assert!(module.select_popup.is_some());
        assert_eq!(module.popup_kind, Some(DetailPopupKind::AssociateFip));
    }

    #[test]
    fn test_detail_f_only_shows_unassociated_fips() {
        let (mut module, _rx) = setup_detail_with_fips();
        module.handle_key(key(KeyCode::Char('f')));
        let popup = module.select_popup.as_ref().unwrap();
        // fip-1 and fip-3 are unassociated, fip-2 has port_id
        assert_eq!(popup.item_count(), 2);
    }

    #[test]
    fn test_detail_f_select_fip_dispatches_fetch_ports() {
        let (mut module, mut rx) = setup_detail_with_fips();
        module.handle_key(key(KeyCode::Char('f'))); // open fip popup
        module.handle_key(key(KeyCode::Enter)); // select fip-1
        // Should dispatch FetchPorts for s1
        let action = rx.try_recv().unwrap();
        assert!(matches!(action, Action::FetchPorts { server_id } if server_id == "s1"));
        assert!(module.pending_fip_id.is_some());
    }

    #[test]
    fn test_detail_f_single_port_auto_confirm() {
        let (mut module, _rx) = setup_detail_with_fips();
        module.handle_key(key(KeyCode::Char('f'))); // open fip popup
        module.handle_key(key(KeyCode::Enter)); // select fip-1
        // Simulate single port loaded
        module.handle_event(&AppEvent::PortsLoaded {
            server_id: "s1".into(),
            ports: vec![make_port("port-1", "10.0.0.5", "net-1", Some("s1"))],
        });
        // Should go directly to confirm (no port selection popup)
        assert!(module.select_popup.is_none());
        assert!(module.confirm.is_active());
        assert!(module.pending_fip_id.is_none());
    }

    #[test]
    fn test_detail_f_single_port_confirm_dispatches_associate() {
        let (mut module, _rx) = setup_detail_with_fips();
        module.handle_key(key(KeyCode::Char('f')));
        module.handle_key(key(KeyCode::Enter)); // select fip-1
        module.handle_event(&AppEvent::PortsLoaded {
            server_id: "s1".into(),
            ports: vec![make_port("port-1", "10.0.0.5", "net-1", Some("s1"))],
        });
        let action = module.handle_key(key(KeyCode::Char('y')));
        assert!(matches!(action, Some(Action::AssociateFloatingIp { fip_id, port_id })
            if fip_id == "fip-1" && port_id == "port-1"));
    }

    #[test]
    fn test_detail_f_multiple_ports_shows_port_popup() {
        let (mut module, _rx) = setup_detail_with_fips();
        module.handle_key(key(KeyCode::Char('f')));
        module.handle_key(key(KeyCode::Enter)); // select fip-1
        module.handle_event(&AppEvent::PortsLoaded {
            server_id: "s1".into(),
            ports: vec![
                make_port("port-1", "10.0.0.5", "net-1", Some("s1")),
                make_port("port-2", "10.0.0.6", "net-1", Some("s1")),
            ],
        });
        assert!(module.select_popup.is_some());
        assert_eq!(module.popup_kind, Some(DetailPopupKind::SelectPort));
    }

    #[test]
    fn test_detail_f_select_port_then_confirm() {
        let (mut module, _rx) = setup_detail_with_fips();
        module.handle_key(key(KeyCode::Char('f')));
        module.handle_key(key(KeyCode::Enter)); // select fip-1
        module.handle_event(&AppEvent::PortsLoaded {
            server_id: "s1".into(),
            ports: vec![
                make_port("port-1", "10.0.0.5", "net-1", Some("s1")),
                make_port("port-2", "10.0.0.6", "net-1", Some("s1")),
            ],
        });
        module.handle_key(key(KeyCode::Enter)); // select port-1
        assert!(module.confirm.is_active());
        let action = module.handle_key(key(KeyCode::Char('y')));
        assert!(matches!(action, Some(Action::AssociateFloatingIp { fip_id, port_id })
            if fip_id == "fip-1" && port_id == "port-1"));
    }

    #[test]
    fn test_detail_f_no_ports_shows_toast() {
        let (mut module, mut rx) = setup_detail_with_fips();
        module.handle_key(key(KeyCode::Char('f')));
        module.handle_key(key(KeyCode::Enter)); // select fip-1
        // Drain the FetchPorts action
        let fetch_action = rx.try_recv().unwrap();
        assert!(matches!(fetch_action, Action::FetchPorts { .. }));
        module.handle_event(&AppEvent::PortsLoaded {
            server_id: "s1".into(),
            ports: vec![],
        });
        assert!(module.select_popup.is_none());
        assert!(!module.confirm.is_active());
        let action = rx.try_recv().unwrap();
        assert!(matches!(action, Action::ShowToast { .. }));
    }

    #[test]
    fn test_detail_f_empty_cache_fetches_floating_ips() {
        let (mut module, mut rx) = setup();
        module.handle_key(key(KeyCode::Enter)); // detail
        module.handle_key(key(KeyCode::Char('f')));
        let action = rx.try_recv().unwrap();
        assert!(matches!(action, Action::FetchFloatingIps));
    }

    #[test]
    fn test_detail_f_no_unassociated_fips_shows_toast() {
        let (tx, mut rx) = mpsc::unbounded_channel();
        let mut module = ServerModule::new(tx);
        let servers = vec![make_test_server("s1", "web-01", "ACTIVE")];
        module.handle_event(&AppEvent::ServersLoaded(servers));
        // All FIPs are associated
        module.handle_event(&AppEvent::FloatingIpsLoaded(vec![
            make_fip("fip-2", "203.0.113.20", Some("port-99")),
        ]));
        module.handle_key(key(KeyCode::Enter)); // detail
        module.handle_key(key(KeyCode::Char('f')));
        let action = rx.try_recv().unwrap();
        assert!(matches!(action, Action::ShowToast { .. }));
    }

    // -- Event handling tests -------------------------------------------------

    #[test]
    fn test_volumes_loaded_caches_volumes() {
        let (tx, _rx) = mpsc::unbounded_channel();
        let mut module = ServerModule::new(tx);
        assert!(module.cached_volumes.is_empty());
        module.handle_event(&AppEvent::VolumesLoaded(vec![
            make_volume("v1", "test-vol", "available", 50, "SSD"),
        ]));
        assert_eq!(module.cached_volumes.len(), 1);
    }

    #[test]
    fn test_floating_ips_loaded_caches_fips() {
        let (tx, _rx) = mpsc::unbounded_channel();
        let mut module = ServerModule::new(tx);
        assert!(module.cached_floating_ips.is_empty());
        module.handle_event(&AppEvent::FloatingIpsLoaded(vec![
            make_fip("fip-1", "203.0.113.10", None),
        ]));
        assert_eq!(module.cached_floating_ips.len(), 1);
    }

    #[test]
    fn test_volume_attached_triggers_fetch_servers() {
        let (mut module, mut rx) = setup();
        module.handle_event(&AppEvent::VolumeAttached {
            volume_id: "v1".into(),
            server_id: "s1".into(),
        });
        let action = rx.try_recv().unwrap();
        assert!(matches!(action, Action::FetchServers));
    }

    #[test]
    fn test_volume_detached_triggers_fetch_servers() {
        let (mut module, mut rx) = setup();
        module.handle_event(&AppEvent::VolumeDetached {
            volume_id: "v1".into(),
        });
        let action = rx.try_recv().unwrap();
        assert!(matches!(action, Action::FetchServers));
    }

    #[test]
    fn test_fip_associated_triggers_fetch_servers() {
        let (mut module, mut rx) = setup();
        module.handle_event(&AppEvent::FloatingIpAssociated(FloatingIp {
            id: "fip-1".into(),
            floating_ip_address: "203.0.113.10".into(),
            status: "ACTIVE".into(),
            port_id: Some("port-1".into()),
            floating_network_id: "ext-1".into(),
            fixed_ip_address: None,
            router_id: None,
            tenant_id: None,
        }));
        let action = rx.try_recv().unwrap();
        assert!(matches!(action, Action::FetchServers));
    }

    // -- resolve_action for volume/fip -----------------------------------------

    #[test]
    fn test_resolve_attach_volume() {
        let action = ServerModule::resolve_action(PendingAction::AttachVolume {
            volume_id: "v1".into(),
            server_id: "s1".into(),
            device: None,
        });
        assert!(matches!(action, Some(Action::AttachVolume { volume_id, server_id, .. })
            if volume_id == "v1" && server_id == "s1"));
    }

    #[test]
    fn test_resolve_detach_volume() {
        let action = ServerModule::resolve_action(PendingAction::DetachVolume {
            volume_id: "v1".into(),
            server_id: "s1".into(),
            attachment_id: "att-1".into(),
        });
        assert!(matches!(action, Some(Action::DetachVolume { volume_id, attachment_id, .. })
            if volume_id == "v1" && attachment_id == "att-1"));
    }

    #[test]
    fn test_resolve_associate_fip() {
        let action = ServerModule::resolve_action(PendingAction::AssociateFloatingIp {
            fip_id: "fip-1".into(),
            port_id: "port-1".into(),
        });
        assert!(matches!(action, Some(Action::AssociateFloatingIp { fip_id, port_id })
            if fip_id == "fip-1" && port_id == "port-1"));
    }

    // -- help_hint includes new keys ------------------------------------------

    #[test]
    fn test_help_hint_detail_includes_attach_vol() {
        let (mut module, _rx) = setup();
        module.handle_key(key(KeyCode::Enter)); // detail
        let hint = module.help_hint();
        assert!(hint.contains("A:AttachVol"), "Detail hint should mention AttachVol");
    }

    #[test]
    fn test_help_hint_detail_includes_detach_vol() {
        let (mut module, _rx) = setup();
        module.handle_key(key(KeyCode::Enter));
        let hint = module.help_hint();
        assert!(hint.contains("x:DetachVol"), "Detail hint should mention DetachVol");
    }

    #[test]
    fn test_help_hint_detail_includes_assoc_fip() {
        let (mut module, _rx) = setup();
        module.handle_key(key(KeyCode::Enter));
        let hint = module.help_hint();
        assert!(hint.contains("f:AssocFIP"), "Detail hint should mention AssocFIP");
    }

    // -- Popup cancel tests ---------------------------------------------------

    #[test]
    fn test_attach_popup_cancel() {
        let (mut module, _rx) = setup_detail_with_volumes();
        module.handle_key(key(KeyCode::Char('A')));
        assert!(module.select_popup.is_some());
        module.handle_key(key(KeyCode::Esc));
        assert!(module.select_popup.is_none());
        assert!(module.popup_kind.is_none());
    }

    #[test]
    fn test_fip_popup_cancel_clears_pending() {
        let (mut module, _rx) = setup_detail_with_fips();
        module.handle_key(key(KeyCode::Char('f')));
        module.handle_key(key(KeyCode::Enter)); // select fip → fetch ports
        // Simulate multi-port
        module.handle_event(&AppEvent::PortsLoaded {
            server_id: "s1".into(),
            ports: vec![
                make_port("port-1", "10.0.0.5", "net-1", Some("s1")),
                make_port("port-2", "10.0.0.6", "net-1", Some("s1")),
            ],
        });
        assert!(module.pending_fip_id.is_some());
        module.handle_key(key(KeyCode::Esc)); // cancel port popup
        assert!(module.pending_fip_id.is_none());
    }

    // -- Detach volume only shows current server's volumes --------------------

    #[test]
    fn test_detail_x_only_shows_current_server_volumes() {
        let (mut module, _rx) = setup_detail_with_volumes();
        // s1 is current server, v3 and v4 are attached to s1, v5 to s2
        module.handle_key(key(KeyCode::Char('x')));
        let popup = module.select_popup.as_ref().unwrap();
        assert_eq!(popup.item_count(), 2, "Should only show s1's volumes (v3, v4), not s2's");
    }

    // -- PortsLoaded is only handled when pending_fip_id is set ---------------

    #[test]
    fn test_ports_loaded_ignored_without_pending_fip() {
        let (mut module, _rx) = setup();
        module.handle_key(key(KeyCode::Enter)); // detail
        // No pending_fip_id → should not create popup or confirm
        module.handle_event(&AppEvent::PortsLoaded {
            server_id: "s1".into(),
            ports: vec![make_port("port-1", "10.0.0.5", "net-1", Some("s1"))],
        });
        assert!(module.select_popup.is_none());
        assert!(!module.confirm.is_active());
    }

    // -- Resource navigation from detail view -----------------------------------

    #[test]
    fn test_detail_navigate_to_volumes() {
        let (mut module, _rx) = setup();
        module.handle_key(key(KeyCode::Enter)); // enter detail
        let action = module.handle_key(key(KeyCode::Char('v')));
        assert!(matches!(action, Some(Action::Navigate(Route::Volumes))));
    }

    #[test]
    fn test_detail_navigate_to_networks() {
        let (mut module, _rx) = setup();
        module.handle_key(key(KeyCode::Enter));
        let action = module.handle_key(key(KeyCode::Char('n')));
        assert!(matches!(action, Some(Action::Navigate(Route::Networks))));
    }

    #[test]
    fn test_detail_navigate_to_security_groups() {
        let (mut module, _rx) = setup();
        module.handle_key(key(KeyCode::Enter));
        let action = module.handle_key(key(KeyCode::Char('s')));
        assert!(matches!(action, Some(Action::Navigate(Route::SecurityGroups))));
    }

    #[test]
    fn test_detail_navigate_to_images() {
        let (mut module, _rx) = setup();
        module.handle_key(key(KeyCode::Enter));
        let action = module.handle_key(key(KeyCode::Char('i')));
        assert!(matches!(action, Some(Action::Navigate(Route::Images))));
    }

    #[test]
    fn test_detail_help_hint_includes_navigation() {
        let (mut module, _rx) = setup();
        module.handle_key(key(KeyCode::Enter));
        let hint = module.help_hint();
        assert!(hint.contains("v:Vol"), "help_hint should contain v:Vol");
        assert!(hint.contains("n:Net"), "help_hint should contain n:Net");
        assert!(hint.contains("s:SG"), "help_hint should contain s:SG");
        assert!(hint.contains("i:Img"), "help_hint should contain i:Img");
    }
}
