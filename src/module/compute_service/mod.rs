pub mod view_model;

use crossterm::event::{KeyCode, KeyEvent};
use ratatui::layout::Rect;
use ratatui::Frame;

use crate::action::Action;
use crate::component::Component;
use crate::event::AppEvent;
use crate::models::nova::ComputeService;
use crate::module::ListNav;
use crate::ui::resource_list::{ResourceList, Row};

use self::view_model::{compute_service_columns, compute_service_to_row};

pub struct ComputeServiceModule {
    services: Vec<ComputeService>,
    nav: ListNav,
    #[allow(dead_code)]
    loading: bool,
    resource_list: ResourceList,
}

impl ComputeServiceModule {
    pub fn new() -> Self {
        Self {
            services: Vec::new(),
            nav: ListNav::new(),
            loading: false,
            resource_list: ResourceList::new(compute_service_columns()),
        }
    }
    pub fn services(&self) -> &[ComputeService] { &self.services }
    fn rows(&self) -> Vec<Row> { self.services.iter().map(compute_service_to_row).collect() }
}

impl Component for ComputeServiceModule {
    fn handle_key(&mut self, key: KeyEvent) -> Option<Action> {
        if self.nav.handle_key(key) { return None; }
        match key.code {
            KeyCode::Char('r') => Some(Action::FetchComputeServices),
            KeyCode::Esc => Some(Action::Back),
            _ => None,
        }
    }
    fn handle_event(&mut self, event: &AppEvent) {
        if let AppEvent::ComputeServicesLoaded(svcs) = event {
            self.services = svcs.clone();
            self.loading = false;
            self.nav.set_count(self.services.len());
            let rows = self.rows();
            self.resource_list.set_rows(rows);
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

    #[test] fn test_initial() { let m = ComputeServiceModule::new(); assert!(m.services().is_empty()); }
    #[test] fn test_refresh() {
        let mut m = ComputeServiceModule::new();
        assert!(matches!(m.handle_key(key(KeyCode::Char('r'))), Some(Action::FetchComputeServices)));
    }
    #[test] fn test_event_loaded() {
        let mut m = ComputeServiceModule::new();
        m.handle_event(&AppEvent::ComputeServicesLoaded(vec![
            ComputeService { id: "s1".into(), binary: "nova-compute".into(), host: "node1".into(), state: "up".into(), status: "enabled".into(), updated_at: None, disabled_reason: None },
        ]));
        assert_eq!(m.services().len(), 1);
    }
}
