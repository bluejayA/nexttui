pub mod agent;
pub mod aggregate;
pub mod compute_service;
pub mod flavor;
pub mod floating_ip;
pub mod hypervisor;
pub mod image;
pub mod migration;
pub mod network;
// Note: UsageModule is deferred to Phase 2 (requires date range picker UI)
pub mod project;
pub mod security_group;
pub mod server;
pub mod snapshot;
pub mod user;
pub mod volume;

use crossterm::event::KeyEvent;

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
    // Neutron
    DeleteSecurityGroup { id: String, name: String },
    DeleteSecurityGroupRule { rule_id: String },
    DeleteFloatingIp { id: String, ip: String },
    // Cinder
    DeleteVolume { id: String, name: String },
    DeleteSnapshot { id: String, name: String },
    // Glance
    DeleteImage { id: String, name: String },
    // Keystone
    DeleteProject { id: String, name: String },
    DeleteUser { id: String, name: String },
    // Nova: Migration / Evacuate
    LiveMigrate { id: String },
    ColdMigrate { id: String },
    ConfirmMigrate { id: String },
    RevertMigrate { id: String },
    Evacuate { id: String },
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
    use crossterm::event::KeyCode;

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
            PendingAction::DeleteSecurityGroup {
                id: "sg1".into(),
                name: "default".into(),
            },
            PendingAction::DeleteSecurityGroupRule {
                rule_id: "rule1".into(),
            },
            PendingAction::DeleteFloatingIp {
                id: "fip1".into(),
                ip: "203.0.113.10".into(),
            },
            PendingAction::DeleteVolume {
                id: "vol1".into(),
                name: "data-vol".into(),
            },
            PendingAction::DeleteSnapshot {
                id: "snap1".into(),
                name: "daily".into(),
            },
            PendingAction::DeleteImage {
                id: "img1".into(),
                name: "ubuntu".into(),
            },
            PendingAction::DeleteProject {
                id: "proj1".into(),
                name: "test".into(),
            },
            PendingAction::DeleteUser {
                id: "user1".into(),
                name: "admin".into(),
            },
        ];
        assert_eq!(actions.len(), 12);
    }

    #[test]
    fn test_migration_pending_action_variants() {
        let actions: Vec<PendingAction> = vec![
            PendingAction::LiveMigrate { id: "s1".into() },
            PendingAction::ColdMigrate { id: "s1".into() },
            PendingAction::ConfirmMigrate { id: "s1".into() },
            PendingAction::RevertMigrate { id: "s1".into() },
            PendingAction::Evacuate { id: "s1".into() },
        ];
        assert_eq!(actions.len(), 5);
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
