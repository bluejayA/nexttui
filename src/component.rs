use crossterm::event::KeyEvent;
use ratatui::Frame;
use ratatui::layout::Rect;

use crate::action::Action;
use crate::event::AppEvent;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LayoutHint {
    Default,
    FullWidth,
}

pub trait Component {
    fn handle_key(&mut self, key: KeyEvent) -> Option<Action>;
    fn handle_event(&mut self, event: &AppEvent);
    fn render(&self, frame: &mut Frame, area: Rect);
    fn set_admin(&mut self, _is_admin: bool) {}
    fn set_all_tenants(&mut self, _all_tenants: bool) {}
    fn help_hint(&self) -> &str {
        ""
    }
    fn refresh_action(&self) -> Option<Action> {
        None
    }
    fn has_transitional_resources(&self) -> bool {
        false
    }
    fn is_modal(&self) -> bool {
        false
    }
    fn layout_hint(&self) -> LayoutHint {
        LayoutHint::Default
    }
    fn is_busy(&self) -> bool {
        false
    }
    /// Dynamic content title based on view state (e.g. "Server: web-01").
    /// Returns None to use the default route label.
    fn content_title(&self) -> Option<String> {
        None
    }

    /// Called by `App` when `AppEvent::ContextChanged` fires — the active
    /// cloud/project just switched. Implementations should drop cached
    /// resource lists, reset transient selection/detail state, and set
    /// `is_loading = true` so stale entries can't be acted upon before the
    /// next fetch lands. Default is no-op for read-only/leaf modules.
    /// (Codex adversarial HIGH #1 / BL-P2-052 Part B safety portion.)
    fn on_context_changed(&mut self) {}
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
    fn test_component_default_refresh_action_is_none() {
        struct Dummy;
        impl Component for Dummy {
            fn handle_key(&mut self, _key: KeyEvent) -> Option<Action> {
                None
            }
            fn handle_event(&mut self, _event: &AppEvent) {}
            fn render(&self, _frame: &mut Frame, _area: Rect) {}
        }
        let d = Dummy;
        assert!(d.refresh_action().is_none());
    }

    #[test]
    fn test_component_default_has_transitional_is_false() {
        struct Dummy;
        impl Component for Dummy {
            fn handle_key(&mut self, _key: KeyEvent) -> Option<Action> {
                None
            }
            fn handle_event(&mut self, _event: &AppEvent) {}
            fn render(&self, _frame: &mut Frame, _area: Rect) {}
        }
        let d = Dummy;
        assert!(!d.has_transitional_resources());
    }

    #[test]
    fn test_component_default_is_modal_is_false() {
        struct Dummy;
        impl Component for Dummy {
            fn handle_key(&mut self, _key: KeyEvent) -> Option<Action> {
                None
            }
            fn handle_event(&mut self, _event: &AppEvent) {}
            fn render(&self, _frame: &mut Frame, _area: Rect) {}
        }
        let d = Dummy;
        assert!(!d.is_modal());
    }

    #[test]
    fn test_component_default_layout_hint_is_default() {
        struct Dummy;
        impl Component for Dummy {
            fn handle_key(&mut self, _key: KeyEvent) -> Option<Action> {
                None
            }
            fn handle_event(&mut self, _event: &AppEvent) {}
            fn render(&self, _frame: &mut Frame, _area: Rect) {}
        }
        let d = Dummy;
        assert_eq!(d.layout_hint(), LayoutHint::Default);
        assert!(!d.is_busy());
    }

    #[test]
    fn test_component_set_admin_default() {
        use crate::action::Action;
        use crate::event::AppEvent;
        use crossterm::event::KeyEvent;
        use ratatui::Frame;
        use ratatui::layout::Rect;

        struct Dummy;
        impl Component for Dummy {
            fn handle_key(&mut self, _key: KeyEvent) -> Option<Action> {
                None
            }
            fn handle_event(&mut self, _event: &AppEvent) {}
            fn render(&self, _frame: &mut Frame, _area: Rect) {}
        }

        let mut d = Dummy;
        // Should not panic — default no-op
        d.set_admin(true);
        d.set_admin(false);
    }
}
