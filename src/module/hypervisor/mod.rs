pub mod view_model;

use crossterm::event::{KeyCode, KeyEvent};
use ratatui::layout::Rect;
use ratatui::Frame;

use crate::action::Action;
use crate::component::Component;
use crate::event::AppEvent;
use crate::models::nova::Hypervisor;
use crate::module::ViewState;
use crate::ui::resource_list::{ResourceList, Row};

use self::view_model::{hypervisor_columns, hypervisor_to_row};

pub struct HypervisorModule {
    view_state: ViewState,
    hypervisors: Vec<Hypervisor>,
    #[allow(dead_code)]
    loading: bool,
    error_message: Option<String>,
    resource_list: ResourceList,
}

impl HypervisorModule {
    pub fn new() -> Self {
        Self {
            view_state: ViewState::List,
            hypervisors: Vec::new(),
            loading: false,
            error_message: None,
            resource_list: ResourceList::new(hypervisor_columns()),
        }
    }

    pub fn view_state(&self) -> &ViewState { &self.view_state }
    pub fn hypervisors(&self) -> &[Hypervisor] { &self.hypervisors }
    pub fn selected_index(&self) -> usize { self.resource_list.selected_index() }

    fn rows(&self) -> Vec<Row> { self.hypervisors.iter().map(hypervisor_to_row).collect() }
}

impl Component for HypervisorModule {
    fn handle_key(&mut self, key: KeyEvent) -> Option<Action> {
        if self.resource_list.handle_nav_key(key) { return None; }
        match key.code {
            KeyCode::Char('r') => Some(Action::FetchHypervisors),
            KeyCode::Left => Some(Action::FocusSidebar),
            KeyCode::Esc => Some(Action::Back),
            _ => None,
        }
    }

    fn handle_event(&mut self, event: &AppEvent) {
        match event {
            AppEvent::HypervisorsLoaded(hv) => {
                self.hypervisors = hv.clone();
                self.loading = false;
                self.error_message = None;
                let rows = self.rows();
                self.resource_list.set_rows(rows);
            }
            AppEvent::ApiError { operation, message, .. } => {
                self.error_message = Some(format!("{operation}: {message}"));
                self.loading = false;
            }
            _ => {}
        }
    }

    fn render(&self, frame: &mut Frame, area: Rect) {
        self.resource_list.render(frame, area);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn key(code: KeyCode) -> KeyEvent { KeyEvent::from(code) }

    fn make_hypervisor(id: &str, name: &str) -> Hypervisor {
        Hypervisor {
            id: id.into(), hypervisor_hostname: name.into(), state: "up".into(),
            status: "enabled".into(), vcpus: 64, vcpus_used: 32,
            memory_mb: 131072, memory_mb_used: 65536,
            running_vms: 10, hypervisor_type: "QEMU".into(), local_gb: 1000, local_gb_used: 500,
        }
    }

    #[test] fn test_initial_state() { let m = HypervisorModule::new(); assert!(m.hypervisors().is_empty()); }
    #[test] fn test_nav() {
        let mut m = HypervisorModule::new();
        m.handle_event(&AppEvent::HypervisorsLoaded(vec![make_hypervisor("h1", "node1"), make_hypervisor("h2", "node2")]));
        m.handle_key(key(KeyCode::Char('j')));
        assert_eq!(m.selected_index(), 1);
    }
    #[test] fn test_refresh() {
        let mut m = HypervisorModule::new();
        assert!(matches!(m.handle_key(key(KeyCode::Char('r'))), Some(Action::FetchHypervisors)));
    }
}
