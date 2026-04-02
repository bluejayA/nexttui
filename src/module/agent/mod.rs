pub mod view_model;

use crossterm::event::{KeyCode, KeyEvent};
use ratatui::layout::Rect;
use ratatui::Frame;

use crate::action::Action;
use crate::component::Component;
use crate::event::AppEvent;
use crate::models::neutron::NetworkAgent;
use crate::ui::resource_list::{ResourceList, Row};

use self::view_model::{agent_columns, agent_to_row};

pub struct AgentModule {
    agents: Vec<NetworkAgent>,
    #[allow(dead_code)]
    loading: bool,
    resource_list: ResourceList,
}

impl AgentModule {
    pub fn new() -> Self {
        Self {
            agents: Vec::new(),
            loading: false,
            resource_list: ResourceList::new(agent_columns()),
        }
    }
    pub fn agents(&self) -> &[NetworkAgent] { &self.agents }
    pub fn selected_index(&self) -> usize { self.resource_list.selected_index() }
    fn rows(&self) -> Vec<Row> { self.agents.iter().map(agent_to_row).collect() }
}

impl Component for AgentModule {
    fn refresh_action(&self) -> Option<Action> { Some(Action::FetchAgents) }

    fn handle_key(&mut self, key: KeyEvent) -> Option<Action> {
        if self.resource_list.handle_nav_key(key) { return None; }
        match key.code {
            KeyCode::Char('r') => Some(Action::FetchAgents),
            KeyCode::Left => Some(Action::FocusSidebar),
            KeyCode::Esc => Some(Action::Back),
            _ => None,
        }
    }
    fn handle_event(&mut self, event: &AppEvent) {
        if let AppEvent::AgentsLoaded(agents) = event {
            self.agents = agents.clone();
            self.loading = false;
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

    fn make_agent(id: &str, agent_type: &str) -> NetworkAgent {
        NetworkAgent {
            id: id.into(), agent_type: agent_type.into(), host: "network-01".into(),
            admin_state_up: true, alive: true, binary: "neutron-agent".into(),
        }
    }

    #[test] fn test_initial() { let m = AgentModule::new(); assert!(m.agents().is_empty()); }
    #[test] fn test_nav() {
        let mut m = AgentModule::new();
        m.handle_event(&AppEvent::AgentsLoaded(vec![make_agent("a1", "OVS"), make_agent("a2", "L3")]));
        m.handle_key(key(KeyCode::Char('j')));
        assert_eq!(m.selected_index(), 1);
    }
    #[test] fn test_refresh() {
        let mut m = AgentModule::new();
        assert!(matches!(m.handle_key(key(KeyCode::Char('r'))), Some(Action::FetchAgents)));
    }
    #[test] fn test_event_loaded() {
        let mut m = AgentModule::new();
        m.handle_event(&AppEvent::AgentsLoaded(vec![make_agent("a1", "OVS")]));
        assert_eq!(m.agents().len(), 1);
    }
}
