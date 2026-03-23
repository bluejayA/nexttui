use std::collections::HashSet;
use std::sync::RwLock;

use crate::models::common::Route;
use crate::port::types::{Capability, TokenRole};

/// Action types that can be permission-gated.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ActionKind {
    Read,
    Create,
    Delete,
    ForceDelete,
    Migrate,
    Evacuate,
    EnableDisable,
    ManageQuota,
}

/// Internal state consolidated under a single lock to ensure atomic updates.
struct RbacState {
    roles: Vec<TokenRole>,
    project_id: Option<String>,
    is_admin: bool,
    capabilities: HashSet<Capability>,
}

/// Role-based access control guard.
/// Phase 1: Keystone role-based filtering.
/// Phase 2: Capability-based extension via `has_capability()` / `update_capabilities()`.
pub struct RbacGuard {
    state: RwLock<RbacState>,
}

impl RbacGuard {
    pub fn new() -> Self {
        Self {
            state: RwLock::new(RbacState {
                roles: Vec::new(),
                project_id: None,
                is_admin: false,
                capabilities: HashSet::new(),
            }),
        }
    }

    /// Update roles from auth token. Automatically determines is_admin.
    /// Clears capabilities to prevent stale state — caller must re-populate
    /// via `update_capabilities()` if needed.
    pub fn update_roles(&self, roles: Vec<TokenRole>, project_id: Option<String>) {
        let admin = roles.iter().any(|r| r.name.eq_ignore_ascii_case("admin"));
        if let Ok(mut s) = self.state.write() {
            s.roles = roles;
            s.project_id = project_id;
            s.is_admin = admin;
            s.capabilities.clear();
        }
    }

    /// Update capabilities from AuthProvider.
    /// Phase 1: derives all capabilities for admin.
    /// Phase 2: populated from backend-specific capabilities.
    pub fn update_capabilities(&self, capabilities: Vec<Capability>) {
        if let Ok(mut s) = self.state.write() {
            s.capabilities = capabilities.into_iter().collect();
        }
    }

    pub fn is_admin(&self) -> bool {
        self.state.read().map(|s| s.is_admin).unwrap_or(false)
    }

    pub fn project_id(&self) -> Option<String> {
        self.state.read().ok().and_then(|s| s.project_id.clone())
    }

    /// Check if current user can access a route (for sidebar filtering).
    pub fn can_access_route(&self, route: &Route) -> bool {
        if self.is_admin() {
            return true;
        }
        !Self::is_admin_only_route(route)
    }

    /// Check if current user can perform a specific action.
    pub fn can_perform(&self, action: ActionKind) -> bool {
        if self.is_admin() {
            return true;
        }
        !Self::is_admin_only_action(action)
    }

    /// Capability-based permission check.
    /// If capabilities are populated, checks against them.
    /// Otherwise falls back to role-based check (admin = all).
    pub fn has_capability(&self, resource: &str, action: &str) -> bool {
        if let Ok(s) = self.state.read() {
            if !s.capabilities.is_empty() {
                return s.capabilities.contains(&Capability {
                    resource: resource.to_string(),
                    action: action.to_string(),
                });
            }
            return s.is_admin;
        }
        false
    }

    /// Filter a list of routes to only those accessible.
    pub fn filter_routes(&self, routes: &[Route]) -> Vec<Route> {
        routes
            .iter()
            .filter(|r| self.can_access_route(r))
            .copied()
            .collect()
    }

    /// Filter a list of actions to only those permitted.
    pub fn filter_actions(&self, actions: &[ActionKind]) -> Vec<ActionKind> {
        actions
            .iter()
            .filter(|a| self.can_perform(**a))
            .copied()
            .collect()
    }

    fn is_admin_only_route(route: &Route) -> bool {
        matches!(
            route,
            Route::Migrations
                | Route::Aggregates
                | Route::ComputeServices
                | Route::Hypervisors
                | Route::Projects
                | Route::Users
                | Route::Agents
                | Route::Usage
        )
    }

    fn is_admin_only_action(action: ActionKind) -> bool {
        matches!(
            action,
            ActionKind::ForceDelete
                | ActionKind::Migrate
                | ActionKind::Evacuate
                | ActionKind::EnableDisable
                | ActionKind::ManageQuota
        )
    }
}

impl Default for RbacGuard {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn role(name: &str) -> TokenRole {
        TokenRole {
            id: format!("{name}-id"),
            name: name.to_string(),
        }
    }

    #[test]
    fn test_new_is_not_admin() {
        let guard = RbacGuard::new();
        assert!(!guard.is_admin());
        assert!(guard.project_id().is_none());
    }

    #[test]
    fn test_update_roles_admin() {
        let guard = RbacGuard::new();
        guard.update_roles(vec![role("admin"), role("member")], Some("proj-1".into()));
        assert!(guard.is_admin());
        assert_eq!(guard.project_id(), Some("proj-1".to_string()));
    }

    #[test]
    fn test_update_roles_non_admin() {
        let guard = RbacGuard::new();
        guard.update_roles(vec![role("member"), role("reader")], None);
        assert!(!guard.is_admin());
    }

    #[test]
    fn test_admin_case_insensitive() {
        let guard = RbacGuard::new();
        guard.update_roles(vec![role("Admin")], None);
        assert!(guard.is_admin());
    }

    #[test]
    fn test_can_access_route_admin() {
        let guard = RbacGuard::new();
        guard.update_roles(vec![role("admin")], None);
        assert!(guard.can_access_route(&Route::Migrations));
        assert!(guard.can_access_route(&Route::Servers));
    }

    #[test]
    fn test_can_access_route_non_admin() {
        let guard = RbacGuard::new();
        guard.update_roles(vec![role("member")], None);
        assert!(!guard.can_access_route(&Route::Migrations));
        assert!(!guard.can_access_route(&Route::Projects));
        assert!(guard.can_access_route(&Route::Servers));
        assert!(guard.can_access_route(&Route::Networks));
        assert!(guard.can_access_route(&Route::Volumes));
    }

    #[test]
    fn test_can_perform_admin() {
        let guard = RbacGuard::new();
        guard.update_roles(vec![role("admin")], None);
        assert!(guard.can_perform(ActionKind::ForceDelete));
        assert!(guard.can_perform(ActionKind::Read));
    }

    #[test]
    fn test_can_perform_non_admin() {
        let guard = RbacGuard::new();
        guard.update_roles(vec![role("member")], None);
        assert!(!guard.can_perform(ActionKind::ForceDelete));
        assert!(!guard.can_perform(ActionKind::Migrate));
        assert!(!guard.can_perform(ActionKind::Evacuate));
        assert!(guard.can_perform(ActionKind::Read));
        assert!(guard.can_perform(ActionKind::Create));
        assert!(guard.can_perform(ActionKind::Delete));
    }

    #[test]
    fn test_filter_routes() {
        let guard = RbacGuard::new();
        guard.update_roles(vec![role("member")], None);
        let routes = vec![Route::Servers, Route::Migrations, Route::Networks, Route::Projects];
        let filtered = guard.filter_routes(&routes);
        assert_eq!(filtered, vec![Route::Servers, Route::Networks]);
    }

    #[test]
    fn test_filter_actions() {
        let guard = RbacGuard::new();
        guard.update_roles(vec![role("member")], None);
        let actions = vec![ActionKind::Read, ActionKind::Delete, ActionKind::ForceDelete];
        let filtered = guard.filter_actions(&actions);
        assert_eq!(filtered, vec![ActionKind::Read, ActionKind::Delete]);
    }

    #[test]
    fn test_has_capability_with_capabilities() {
        let guard = RbacGuard::new();
        guard.update_capabilities(vec![Capability {
            resource: "server".to_string(),
            action: "delete".to_string(),
        }]);
        assert!(guard.has_capability("server", "delete"));
        assert!(!guard.has_capability("server", "force_delete"));
    }

    #[test]
    fn test_has_capability_fallback_to_role() {
        let guard = RbacGuard::new();
        // No capabilities set, fallback to role-based
        guard.update_roles(vec![role("admin")], None);
        assert!(guard.has_capability("server", "anything"));

        let guard2 = RbacGuard::new();
        guard2.update_roles(vec![role("member")], None);
        assert!(!guard2.has_capability("server", "anything"));
    }

    #[test]
    fn test_update_roles_clears_capabilities() {
        let guard = RbacGuard::new();
        guard.update_capabilities(vec![Capability {
            resource: "server".to_string(),
            action: "delete".to_string(),
        }]);
        assert!(guard.has_capability("server", "delete"));

        // update_roles should clear capabilities
        guard.update_roles(vec![role("member")], None);
        // Now capabilities are empty, falls back to role-based (member = not admin = false)
        assert!(!guard.has_capability("server", "delete"));
    }
}
