pub mod view_model;

use crossterm::event::{KeyCode, KeyEvent};
use ratatui::Frame;
use ratatui::layout::Rect;

use crate::action::Action;
use crate::component::Component;
use crate::event::AppEvent;
use crate::models::nova::Aggregate;
use crate::ui::resource_list::{ResourceList, Row};

use self::view_model::{aggregate_columns, aggregate_to_row};

pub struct AggregateModule {
    aggregates: Vec<Aggregate>,
    #[allow(dead_code)]
    loading: bool,
    resource_list: ResourceList,
}

impl Default for AggregateModule {
    fn default() -> Self {
        Self::new()
    }
}

impl AggregateModule {
    pub fn new() -> Self {
        Self {
            aggregates: Vec::new(),
            loading: false,
            resource_list: ResourceList::new(aggregate_columns()),
        }
    }
    pub fn aggregates(&self) -> &[Aggregate] {
        &self.aggregates
    }
    fn rows(&self) -> Vec<Row> {
        self.aggregates.iter().map(aggregate_to_row).collect()
    }
}

impl Component for AggregateModule {
    fn refresh_action(&self) -> Option<Action> {
        Some(Action::FetchAggregates)
    }

    fn handle_key(&mut self, key: KeyEvent) -> Option<Action> {
        if self.resource_list.handle_nav_key(key) {
            return None;
        }
        match key.code {
            KeyCode::Char('r') => Some(Action::FetchAggregates),
            KeyCode::Left => Some(Action::FocusSidebar),
            KeyCode::Esc => Some(Action::Back),
            _ => None,
        }
    }
    fn handle_event(&mut self, event: &AppEvent) {
        if let AppEvent::AggregatesLoaded(aggs) = event {
            self.aggregates = aggs.clone();
            self.loading = false;
            let rows = self.rows();
            self.resource_list.set_rows(rows);
        }
    }
    fn render(&self, frame: &mut Frame, area: Rect) {
        self.resource_list.render(frame, area);
    }

    fn help_hint(&self) -> &str {
        "r:Refresh"
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn key(code: KeyCode) -> KeyEvent {
        KeyEvent::from(code)
    }

    #[test]
    fn test_initial() {
        let m = AggregateModule::new();
        assert!(m.aggregates().is_empty());
    }
    #[test]
    fn test_refresh() {
        let mut m = AggregateModule::new();
        assert!(matches!(
            m.handle_key(key(KeyCode::Char('r'))),
            Some(Action::FetchAggregates)
        ));
    }
    #[test]
    fn test_event_loaded() {
        let mut m = AggregateModule::new();
        m.handle_event(&AppEvent::AggregatesLoaded(vec![Aggregate {
            id: 1,
            name: "agg1".into(),
            availability_zone: Some("az1".into()),
            hosts: vec!["h1".into()],
            metadata: Default::default(),
        }]));
        assert_eq!(m.aggregates().len(), 1);
    }

    #[test]
    fn test_help_hint() {
        let m = AggregateModule::new();
        assert_eq!(m.help_hint(), "r:Refresh");
    }
}
