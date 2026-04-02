pub mod view_model;

// MigrationModule — Phase 2: Live/Cold Migration, Evacuate, Force State
// Requires complex multi-step workflows (server selection → migration type → target host).
// Placeholder for now with server list view.

use crossterm::event::{KeyCode, KeyEvent};
use ratatui::layout::Rect;
use ratatui::style::{Color, Style};
use ratatui::text::Line;
use ratatui::widgets::Paragraph;
use ratatui::Frame;

use crate::action::Action;
use crate::component::Component;
use crate::event::AppEvent;

pub struct MigrationModule;

impl MigrationModule {
    pub fn new() -> Self { Self }
}

impl Component for MigrationModule {
    fn handle_key(&mut self, key: KeyEvent) -> Option<Action> {
        match key.code {
            KeyCode::Esc => Some(Action::Back),
            _ => None,
        }
    }

    fn handle_event(&mut self, _event: &AppEvent) {}

    fn render(&self, frame: &mut Frame, area: Rect) {
        let text = Paragraph::new(vec![
            Line::raw(""),
            Line::raw("  Migration Management"),
            Line::raw("  [Available in Phase 2: Live/Cold Migration, Evacuate, Force State]"),
            Line::raw("  Press Esc to go back"),
        ])
        .style(Style::default().fg(Color::DarkGray));
        frame.render_widget(text, area);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_esc_returns_back() {
        let mut m = MigrationModule::new();
        let action = m.handle_key(KeyEvent::from(KeyCode::Esc));
        assert!(matches!(action, Some(Action::Back)));
    }

    #[test]
    fn test_help_hint() {
        let m = MigrationModule::new();
        assert_eq!(m.help_hint(), "");
    }
}
