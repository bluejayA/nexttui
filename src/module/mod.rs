pub mod flavor;
pub mod server;

use crossterm::event::{KeyCode, KeyEvent};

use crate::action::Action;
use crate::ui::confirm::{ConfirmDialog, ConfirmResult};

/// ViewState shared by domain modules.
#[derive(Debug, Clone, PartialEq)]
pub enum ViewState {
    List,
    Detail(String),
    Create,
}

/// Pending confirmed action — wraps what should happen after user confirms.
#[derive(Debug, Clone)]
pub enum PendingAction {
    Delete { id: String, name: String },
    Reboot { id: String, hard: bool },
    Stop { id: String },
    Submit,
}

/// Shared list navigation state. Extracts the common j/k/g/G/selection logic
/// so domain modules don't duplicate it.
pub struct ListNav {
    pub selected_index: usize,
    item_count: usize,
}

impl ListNav {
    pub fn new() -> Self {
        Self {
            selected_index: 0,
            item_count: 0,
        }
    }

    pub fn set_count(&mut self, count: usize) {
        self.item_count = count;
        self.clamp();
    }

    pub fn clamp(&mut self) {
        if self.item_count > 0 {
            self.selected_index = self.selected_index.min(self.item_count - 1);
        } else {
            self.selected_index = 0;
        }
    }

    /// Handle common list navigation keys. Returns true if the key was consumed.
    pub fn handle_key(&mut self, key: KeyEvent) -> bool {
        match key.code {
            KeyCode::Char('j') | KeyCode::Down => {
                if self.item_count > 0 {
                    self.selected_index = (self.selected_index + 1).min(self.item_count - 1);
                }
                true
            }
            KeyCode::Char('k') | KeyCode::Up => {
                self.selected_index = self.selected_index.saturating_sub(1);
                true
            }
            KeyCode::Char('g') => {
                self.selected_index = 0;
                true
            }
            KeyCode::Char('G') => {
                if self.item_count > 0 {
                    self.selected_index = self.item_count - 1;
                }
                true
            }
            _ => false,
        }
    }
}

/// Shared confirm dialog handler. Wraps the ConfirmDialog + PendingAction
/// pattern common to all domain modules.
pub struct ConfirmHandler {
    pub dialog: Option<ConfirmDialog>,
    pub pending: Option<PendingAction>,
}

impl ConfirmHandler {
    pub fn new() -> Self {
        Self {
            dialog: None,
            pending: None,
        }
    }

    pub fn is_active(&self) -> bool {
        self.dialog.is_some()
    }

    pub fn open(&mut self, dialog: ConfirmDialog, action: PendingAction) {
        self.dialog = Some(dialog);
        self.pending = Some(action);
    }

    /// Try to handle a key event on the active dialog.
    /// Returns `Some(Some(Action))` if confirmed with an action,
    /// `Some(None)` if cancelled or confirmed with no action,
    /// `None` if no dialog is active.
    pub fn handle_key<F>(&mut self, key: KeyEvent, resolve: F) -> Option<Option<Action>>
    where
        F: FnOnce(PendingAction) -> Option<Action>,
    {
        let dialog = self.dialog.as_mut()?;
        match dialog.handle_key(key) {
            ConfirmResult::Confirmed => {
                self.dialog = None;
                let action = self.pending.take().and_then(resolve);
                Some(action)
            }
            ConfirmResult::Cancelled => {
                self.dialog = None;
                self.pending = None;
                Some(None)
            }
            ConfirmResult::Pending => Some(None),
        }
    }

    pub fn render(&self, frame: &mut ratatui::Frame, area: ratatui::layout::Rect) {
        if let Some(ref dialog) = self.dialog {
            dialog.render(frame, area);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_view_state_default_is_list() {
        let state = ViewState::List;
        assert_eq!(state, ViewState::List);
    }

    #[test]
    fn test_pending_action_variants() {
        let actions: Vec<PendingAction> = vec![
            PendingAction::Delete {
                id: "s1".into(),
                name: "web".into(),
            },
            PendingAction::Reboot {
                id: "s1".into(),
                hard: true,
            },
            PendingAction::Stop { id: "s1".into() },
            PendingAction::Submit,
        ];
        assert_eq!(actions.len(), 4);
    }

    #[test]
    fn test_list_nav_j_k() {
        let mut nav = ListNav::new();
        nav.set_count(5);
        assert_eq!(nav.selected_index, 0);

        assert!(nav.handle_key(KeyEvent::from(KeyCode::Char('j'))));
        assert_eq!(nav.selected_index, 1);

        assert!(nav.handle_key(KeyEvent::from(KeyCode::Char('k'))));
        assert_eq!(nav.selected_index, 0);

        // Can't go below 0
        assert!(nav.handle_key(KeyEvent::from(KeyCode::Char('k'))));
        assert_eq!(nav.selected_index, 0);
    }

    #[test]
    fn test_list_nav_g_G() {
        let mut nav = ListNav::new();
        nav.set_count(10);

        assert!(nav.handle_key(KeyEvent::from(KeyCode::Char('G'))));
        assert_eq!(nav.selected_index, 9);

        assert!(nav.handle_key(KeyEvent::from(KeyCode::Char('g'))));
        assert_eq!(nav.selected_index, 0);
    }

    #[test]
    fn test_list_nav_empty_list() {
        let mut nav = ListNav::new();
        nav.set_count(0);
        assert!(nav.handle_key(KeyEvent::from(KeyCode::Char('j'))));
        assert_eq!(nav.selected_index, 0);
    }

    #[test]
    fn test_list_nav_clamp_on_shrink() {
        let mut nav = ListNav::new();
        nav.set_count(10);
        nav.selected_index = 9;
        nav.set_count(3);
        assert_eq!(nav.selected_index, 2);
    }

    #[test]
    fn test_list_nav_ignores_unrelated_keys() {
        let mut nav = ListNav::new();
        nav.set_count(5);
        assert!(!nav.handle_key(KeyEvent::from(KeyCode::Enter)));
        assert!(!nav.handle_key(KeyEvent::from(KeyCode::Char('x'))));
    }

    #[test]
    fn test_confirm_handler_flow() {
        let mut handler = ConfirmHandler::new();
        assert!(!handler.is_active());

        handler.open(
            ConfirmDialog::yes_no("Delete?"),
            PendingAction::Delete {
                id: "s1".into(),
                name: "web".into(),
            },
        );
        assert!(handler.is_active());

        // Confirm
        let result = handler.handle_key(KeyEvent::from(KeyCode::Char('y')), |pa| match pa {
            PendingAction::Delete { id, name } => Some(Action::DeleteServer { id, name }),
            _ => None,
        });
        assert!(result.is_some());
        let action = result.unwrap();
        assert!(matches!(action, Some(Action::DeleteServer { .. })));
        assert!(!handler.is_active());
    }

    #[test]
    fn test_confirm_handler_cancel() {
        let mut handler = ConfirmHandler::new();
        handler.open(
            ConfirmDialog::yes_no("Delete?"),
            PendingAction::Delete {
                id: "s1".into(),
                name: "web".into(),
            },
        );

        let result = handler.handle_key(KeyEvent::from(KeyCode::Esc), |_| None);
        assert!(matches!(result, Some(None)));
        assert!(!handler.is_active());
        assert!(handler.pending.is_none());
    }

    #[test]
    fn test_confirm_handler_inactive_returns_none() {
        let mut handler = ConfirmHandler::new();
        let result = handler.handle_key(KeyEvent::from(KeyCode::Char('y')), |_| None);
        assert!(result.is_none());
    }
}
