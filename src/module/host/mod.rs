pub mod evac_task;
pub mod host_list;
pub mod instance_list;
pub mod log_panel;

use crossterm::event::{KeyCode, KeyEvent};
use ratatui::layout::{Constraint, Direction, Layout, Rect};
use ratatui::Frame;

use crate::action::Action;
use crate::component::{Component, LayoutHint};
use crate::context::{ActionSender, test_action_channel};
use crate::event::AppEvent;
use crate::models::nova::Server;
use crate::port::types::EvacuateParams;

use self::evac_task::EvacTask;
use self::host_list::HostList;
use self::instance_list::{EvacInlineStatus, InstanceList};
use self::log_panel::{LogLevel, LogPanel};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum HostFocus {
    HostList,
    InstanceList,
}

pub struct HostModule {
    focus: HostFocus,
    host_list: HostList,
    instance_list: InstanceList,
    evac_task: Option<EvacTask>,
    log_panel: LogPanel,
    action_tx: ActionSender,
    is_admin: bool,
    all_servers: Vec<Server>,
    evac_confirm_pending: bool,
}

impl Default for HostModule {
    fn default() -> Self {
        let (tx, _rx) = test_action_channel();
        Self::new(tx)
    }
}

impl HostModule {
    pub fn new(action_tx: ActionSender) -> Self {
        Self {
            focus: HostFocus::HostList,
            host_list: HostList::new(),
            instance_list: InstanceList::new(),
            evac_task: None,
            log_panel: LogPanel::new(),
            action_tx,
            is_admin: false,
            all_servers: Vec::new(),
            evac_confirm_pending: false,
        }
    }

    fn sync_instance_list(&mut self) {
        if let Some(h) = self.host_list.selected_hypervisor() {
            let hostname = &h.hypervisor_hostname;
            let filtered: Vec<Server> = self
                .all_servers
                .iter()
                .filter(|s| s.host.as_deref() == Some(hostname))
                .cloned()
                .collect();
            self.instance_list.set_servers(filtered);
        } else {
            self.instance_list.set_servers(Vec::new());
        }
    }

    fn start_evacuate(&mut self) {
        let ids = self.instance_list.checked_ids();
        if ids.is_empty() {
            self.log_panel.push(
                chrono::Local::now().format("%H:%M:%S").to_string(),
                LogLevel::Warning,
                "No instances selected for evacuation".into(),
            );
            return;
        }
        let count = ids.len();
        for id in &ids {
            self.instance_list.set_evac_status(id, EvacInlineStatus::InFlight);
        }
        let mut task = EvacTask::new(ids, EvacuateParams::default(), 3);
        task.start();
        let actions = task.poll_next();
        for action in actions {
            let _ = self.action_tx.send(action);
        }
        self.evac_task = Some(task);
        self.log_panel.push(
            chrono::Local::now().format("%H:%M:%S").to_string(),
            LogLevel::Info,
            format!("Evacuate started: {count} instances"),
        );
    }

    fn handle_evac_result(&mut self, server_id: &str, result: Result<(), String>) {
        if let Some(ref mut task) = self.evac_task {
            let status = if result.is_ok() {
                EvacInlineStatus::Success
            } else {
                EvacInlineStatus::Failed
            };
            self.instance_list.set_evac_status(server_id, status);
            task.on_completed(server_id, result.clone());

            let ts = chrono::Local::now().format("%H:%M:%S").to_string();
            match &result {
                Ok(()) => {
                    self.log_panel.push(ts, LogLevel::Success, format!("Evacuated {server_id}"));
                }
                Err(msg) => {
                    self.log_panel.push(ts, LogLevel::Error, format!("Failed {server_id}: {msg}"));
                }
            }

            // Dispatch next batch
            let actions = task.poll_next();
            for action in actions {
                let _ = self.action_tx.send(action);
            }

            if task.is_completed() {
                let (ok, fail) = (task.succeeded_count(), task.failed_results().len());
                self.log_panel.push(
                    chrono::Local::now().format("%H:%M:%S").to_string(),
                    LogLevel::Info,
                    format!("Evacuate complete: {ok} ok, {fail} failed"),
                );
                self.evac_task = None;
            }
        }
    }
}

impl Component for HostModule {
    fn handle_key(&mut self, key: KeyEvent) -> Option<Action> {
        match key.code {
            // Panel switching
            KeyCode::Left => {
                self.focus = HostFocus::HostList;
                return None;
            }
            KeyCode::Right => {
                self.focus = HostFocus::InstanceList;
                return None;
            }
            _ => {}
        }

        match self.focus {
            HostFocus::HostList => match key.code {
                KeyCode::Up | KeyCode::Char('k') => {
                    self.host_list.move_up();
                    self.sync_instance_list();
                }
                KeyCode::Down | KeyCode::Char('j') => {
                    self.host_list.move_down();
                    self.sync_instance_list();
                }
                _ => {}
            },
            HostFocus::InstanceList => {
                // Cancel pending confirm on any key other than 'e'
                if self.evac_confirm_pending && key.code != KeyCode::Char('e') {
                    self.evac_confirm_pending = false;
                    self.log_panel.push(
                        chrono::Local::now().format("%H:%M:%S").to_string(),
                        LogLevel::Info,
                        "Evacuate cancelled".into(),
                    );
                }
                match key.code {
                KeyCode::Up | KeyCode::Char('k') => self.instance_list.move_up(),
                KeyCode::Down | KeyCode::Char('j') => self.instance_list.move_down(),
                KeyCode::Char(' ') => self.instance_list.toggle_check(),
                KeyCode::Char('a') => self.instance_list.select_all(),
                KeyCode::Char('d') => self.instance_list.deselect_all(),
                KeyCode::Char('f') => self.instance_list.cycle_filter(),
                KeyCode::Char('e') => {
                    if self.evac_task.is_none() {
                        if self.evac_confirm_pending {
                            self.evac_confirm_pending = false;
                            self.start_evacuate();
                        } else {
                            let count = self.instance_list.checked_count();
                            if count > 0 {
                                self.evac_confirm_pending = true;
                                self.log_panel.push(
                                    chrono::Local::now().format("%H:%M:%S").to_string(),
                                    LogLevel::Warning,
                                    format!("Evacuate {count} instances? Press 'e' again to confirm, any other key to cancel"),
                                );
                            } else {
                                self.log_panel.push(
                                    chrono::Local::now().format("%H:%M:%S").to_string(),
                                    LogLevel::Warning,
                                    "No instances selected for evacuation".into(),
                                );
                            }
                        }
                    }
                }
                KeyCode::Char('c') => {
                    if let Some(ref mut task) = self.evac_task {
                        task.request_cancel();
                        self.log_panel.push(
                            chrono::Local::now().format("%H:%M:%S").to_string(),
                            LogLevel::Warning,
                            "Evacuate cancelled by user".into(),
                        );
                    }
                }
                _ => {}
            }},
        }
        None
    }

    fn handle_event(&mut self, event: &AppEvent) {
        match event {
            AppEvent::HypervisorsLoaded(hvs) => {
                self.host_list.set_hypervisors(hvs.clone());
                self.sync_instance_list();
            }
            AppEvent::ServersLoaded(servers) => {
                self.all_servers = servers.clone();
                self.sync_instance_list();
            }
            AppEvent::ServerEvacuateResult { id, result } => {
                self.handle_evac_result(id, result.clone());
            }
            AppEvent::ComputeServiceToggled { hostname, enabled } => {
                let action = if *enabled { "enabled" } else { "disabled" };
                self.log_panel.push(
                    chrono::Local::now().format("%H:%M:%S").to_string(),
                    LogLevel::Info,
                    format!("Host {hostname} {action}"),
                );
            }
            _ => {}
        }
    }

    fn render(&self, frame: &mut Frame, area: Rect) {
        // Split: top (host_list 35% | instance_list 65%), bottom (log 3 lines)
        let log_height = 4; // 3 lines + border
        let chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Min(5),
                Constraint::Length(log_height),
            ])
            .split(area);

        let panels = Layout::default()
            .direction(Direction::Horizontal)
            .constraints([Constraint::Percentage(35), Constraint::Percentage(65)])
            .split(chunks[0]);

        self.host_list.render(frame, panels[0], self.focus == HostFocus::HostList);
        self.instance_list.render(frame, panels[1], self.focus == HostFocus::InstanceList);
        self.log_panel.render(frame, chunks[1]);
    }

    fn set_admin(&mut self, is_admin: bool) {
        self.is_admin = is_admin;
    }

    fn layout_hint(&self) -> LayoutHint {
        LayoutHint::FullWidth
    }

    fn is_busy(&self) -> bool {
        self.evac_task.is_some()
    }

    fn help_hint(&self) -> &str {
        match self.focus {
            HostFocus::HostList => "↑↓:select  →:instances  Tab:sidebar",
            HostFocus::InstanceList => "↑↓:select  ←:hosts  Space:check  a:all  d:clear  f:filter  e:evacuate  c:cancel",
        }
    }

    fn refresh_action(&self) -> Option<Action> {
        Some(Action::FetchHypervisors)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::context::{ActionReceiver, test_action_channel};
    use crate::models::nova::{FlavorRef, Hypervisor};

    fn make_hypervisor(id: &str, hostname: &str) -> Hypervisor {
        Hypervisor {
            id: id.into(),
            hypervisor_hostname: hostname.into(),
            hypervisor_type: "QEMU".into(),
            vcpus: 16, vcpus_used: 8,
            memory_mb: 32768, memory_mb_used: 16384,
            local_gb: 500, local_gb_used: 200,
            running_vms: 2,
            status: "enabled".into(),
            state: "up".into(),
        }
    }

    fn make_server(id: &str, name: &str, host: &str) -> Server {
        Server {
            id: id.into(), name: name.into(), status: "ACTIVE".into(),
            tenant_id: Some("t1".into()), host: Some(host.into()), host_id: None,
            availability_zone: None,
            flavor: FlavorRef { id: "f1".into(), original_name: None, vcpus: None, ram: None, disk: None },
            image: None, addresses: std::collections::HashMap::new(),
            created: "2026-01-01T00:00:00Z".into(), updated: None, key_name: None,
            volumes_attached: vec![], security_groups: vec![],
        }
    }

    #[test]
    fn test_host_module_layout_hint_full_width() {
        let m = HostModule::default();
        assert_eq!(m.layout_hint(), LayoutHint::FullWidth);
    }

    #[test]
    fn test_host_module_is_busy_when_evacuating() {
        let mut m = HostModule::default();
        assert!(!m.is_busy());

        // Simulate evacuate
        let mut task = EvacTask::new(vec!["s1".into()], EvacuateParams::default(), 2);
        task.start();
        m.evac_task = Some(task);
        assert!(m.is_busy());
    }

    #[test]
    fn test_host_module_sync_instance_list_filters_by_host() {
        let mut m = HostModule::default();
        m.handle_event(&AppEvent::HypervisorsLoaded(vec![
            make_hypervisor("1", "compute-01"),
            make_hypervisor("2", "compute-02"),
        ]));
        m.all_servers = vec![
            make_server("s1", "web-01", "compute-01"),
            make_server("s2", "web-02", "compute-02"),
            make_server("s3", "db-01", "compute-01"),
        ];
        m.sync_instance_list();

        // compute-01 selected → should see s1, s3
        assert_eq!(m.instance_list.filtered_count(), 2);
    }

    #[test]
    fn test_host_module_focus_switching() {
        let mut m = HostModule::default();
        assert_eq!(m.focus, HostFocus::HostList);

        m.handle_key(KeyEvent::from(KeyCode::Right));
        assert_eq!(m.focus, HostFocus::InstanceList);

        m.handle_key(KeyEvent::from(KeyCode::Left));
        assert_eq!(m.focus, HostFocus::HostList);
    }

    #[test]
    fn test_host_module_help_hint_changes_with_focus() {
        let mut m = HostModule::default();
        assert!(m.help_hint().contains("instances"));

        m.focus = HostFocus::InstanceList;
        assert!(m.help_hint().contains("evacuate"));
    }

    #[test]
    fn test_host_module_evac_result_logs() {
        let mut m = HostModule::default();
        m.handle_event(&AppEvent::HypervisorsLoaded(vec![make_hypervisor("1", "compute-01")]));
        m.all_servers = vec![make_server("s1", "web-01", "compute-01")];
        m.sync_instance_list();

        // Manually set up evac task
        let mut task = EvacTask::new(vec!["s1".into()], EvacuateParams::default(), 2);
        task.start();
        task.poll_next();
        m.evac_task = Some(task);

        m.handle_event(&AppEvent::ServerEvacuateResult {
            id: "s1".into(),
            result: Ok(()),
        });

        assert!(!m.is_busy()); // task completed
        assert!(m.log_panel.len() >= 1);
    }
}
