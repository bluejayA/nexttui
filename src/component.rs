use crossterm::event::KeyEvent;
use ratatui::Frame;
use ratatui::layout::Rect;

use crate::action::Action;
use crate::event::AppEvent;

pub trait Component {
    fn handle_key(&mut self, key: KeyEvent) -> Option<Action>;
    fn handle_event(&mut self, event: &AppEvent);
    fn render(&self, frame: &mut Frame, area: Rect);
    fn set_admin(&mut self, _is_admin: bool) {}
    fn set_all_tenants(&mut self, _all_tenants: bool) {}
    fn help_hint(&self) -> &str { "" }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum InputMode {
    #[default]
    Normal,
    Command,
    Search,
    Form,
    Confirm,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_input_mode_default() {
        let mode = InputMode::default();
        assert_eq!(mode, InputMode::Normal);
    }

    #[test]
    fn test_component_set_admin_default() {
        use crossterm::event::KeyEvent;
        use ratatui::layout::Rect;
        use ratatui::Frame;
        use crate::action::Action;
        use crate::event::AppEvent;

        struct Dummy;
        impl Component for Dummy {
            fn handle_key(&mut self, _key: KeyEvent) -> Option<Action> { None }
            fn handle_event(&mut self, _event: &AppEvent) {}
            fn render(&self, _frame: &mut Frame, _area: Rect) {}
        }

        let mut d = Dummy;
        // Should not panic — default no-op
        d.set_admin(true);
        d.set_admin(false);
    }
}
