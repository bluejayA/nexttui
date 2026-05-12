//! One-step history used by `:switch-back` and by the rollback path.
//!
//! The history depth is intentionally 1 for this BL (see requirements:
//! multi-step history is deferred to a follow-up). Keeping only the previous
//! snapshot also avoids surfacing a surprising "undo stack" UX.

use super::types::ContextSnapshot;

#[derive(Debug, Default)]
pub struct ContextHistoryStore {
    previous: Option<ContextSnapshot>,
}

impl ContextHistoryStore {
    pub fn new() -> Self {
        Self::default()
    }

    /// Stores the given snapshot as the new "previous". The slot holds at
    /// most one snapshot — older values are dropped.
    pub fn push(&mut self, snapshot: ContextSnapshot) {
        self.previous = Some(snapshot);
    }

    pub fn previous(&self) -> Option<&ContextSnapshot> {
        self.previous.as_ref()
    }

    /// Consumes the stored snapshot, leaving the store empty. Used by
    /// `switch_back` so that a second consecutive back call is a no-op.
    pub fn pop_previous(&mut self) -> Option<ContextSnapshot> {
        self.previous.take()
    }

    pub fn is_empty(&self) -> bool {
        self.previous.is_none()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::context::types::ContextTarget;
    use crate::port::types::{CatalogEntry, ProjectScope, Token, TokenScope};
    use chrono::{TimeZone, Utc};

    fn sample_snapshot(name: &str, epoch: u64) -> ContextSnapshot {
        let target = ContextTarget {
            cloud: "devstack".to_string(),
            project_id: format!("id-{name}"),
            project_name: name.to_string(),
            domain: "default".to_string(),
        };
        let token = Token {
            id: "tok".to_string(),
            expires_at: Utc.with_ymd_and_hms(2030, 1, 1, 0, 0, 0).unwrap(),
            project: ProjectScope {
                id: target.project_id.clone(),
                name: target.project_name.clone(),
                domain_id: "default".to_string(),
                domain_name: target.domain.clone(),
            },
            roles: Vec::new(),
            catalog: Vec::<CatalogEntry>::new(),
            user_id: String::new(),
        };
        ContextSnapshot {
            target: target.clone(),
            epoch,
            token,
            token_scope: TokenScope::from(&target),
            captured_at: Utc.with_ymd_and_hms(2026, 4, 13, 0, 0, 0).unwrap(),
        }
    }

    #[test]
    fn new_is_empty() {
        let h = ContextHistoryStore::new();
        assert!(h.is_empty());
        assert!(h.previous().is_none());
    }

    #[test]
    fn push_replaces_prior_entry() {
        let mut h = ContextHistoryStore::new();
        h.push(sample_snapshot("admin", 1));
        h.push(sample_snapshot("demo", 2));
        assert_eq!(h.previous().unwrap().target.project_name, "demo");
    }

    #[test]
    fn pop_previous_empties_the_store() {
        let mut h = ContextHistoryStore::new();
        h.push(sample_snapshot("admin", 1));
        let popped = h.pop_previous().expect("has value");
        assert_eq!(popped.target.project_name, "admin");
        assert!(h.is_empty());
        assert!(h.pop_previous().is_none());
    }
}
