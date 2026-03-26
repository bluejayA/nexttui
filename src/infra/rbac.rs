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

/// Effective role derived from Keystone token roles.
/// Ordered by privilege level: Reader < Member < Admin.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum EffectiveRole {
    Reader,
    Member,
    Admin,
}

impl EffectiveRole {
    /// Derive the highest-privilege role from a list of Keystone roles.
    /// Unknown roles are ignored. If no known role matches, defaults to Reader.
    pub fn from_roles(roles: &[TokenRole]) -> Self {
        let mut effective = EffectiveRole::Reader;
        for role in roles {
            let level = match role.name.to_lowercase().as_str() {
                "admin" => EffectiveRole::Admin,
                "member" | "operator" => EffectiveRole::Member,
                "reader" => EffectiveRole::Reader,
                _ => continue,
            };
            if level > effective {
                effective = level;
            }
        }
        effective
    }
}

/// Internal state consolidated under a single lock to ensure atomic updates.
struct RbacState {
    roles: Vec<TokenRole>,
    project_id: Option<String>,
    effective_role: EffectiveRole,
    capabilities: HashSet<Capability>,
}

/// Role-based access control guard.
/// Phase 1 (current): 3-tier role-based filtering (Admin/Member/Reader).
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
                effective_role: EffectiveRole::Reader,
                capabilities: HashSet::new(),
            }),
        }
    }

    /// Update roles from auth token. Automatically determines effective role.
    /// Clears capabilities to prevent stale state — caller must re-populate
    /// via `update_capabilities()` if needed.
    pub fn update_roles(&self, roles: Vec<TokenRole>, project_id: Option<String>) {
        let effective = EffectiveRole::from_roles(&roles);
        if let Ok(mut s) = self.state.write() {
            s.roles = roles;
            s.project_id = project_id;
            s.effective_role = effective;
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

    pub fn effective_role(&self) -> EffectiveRole {
        self.state
            .read()
            .map(|s| s.effective_role)
            .unwrap_or(EffectiveRole::Reader)
    }

    pub fn is_admin(&self) -> bool {
        self.effective_role() == EffectiveRole::Admin
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
    /// Admin: all allowed. Member: CRUD allowed, admin-only denied. Reader: read only.
    pub fn can_perform(&self, action: ActionKind) -> bool {
        match self.effective_role() {
            EffectiveRole::Admin => true,
            EffectiveRole::Member => !Self::is_admin_only_action(action),
            EffectiveRole::Reader => action == ActionKind::Read,
        }
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
            return s.effective_role == EffectiveRole::Admin;
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

    // --- EffectiveRole tests ---

    #[test]
    fn test_effective_role_from_admin() {
        assert_eq!(
            EffectiveRole::from_roles(&[role("admin")]),
            EffectiveRole::Admin
        );
    }

    #[test]
    fn test_effective_role_from_member() {
        assert_eq!(
            EffectiveRole::from_roles(&[role("member")]),
            EffectiveRole::Member
        );
    }

    #[test]
    fn test_effective_role_from_reader() {
        assert_eq!(
            EffectiveRole::from_roles(&[role("reader")]),
            EffectiveRole::Reader
        );
    }

    #[test]
    fn test_effective_role_operator_maps_to_member() {
        assert_eq!(
            EffectiveRole::from_roles(&[role("operator")]),
            EffectiveRole::Member
        );
    }

    #[test]
    fn test_effective_role_highest_wins() {
        assert_eq!(
            EffectiveRole::from_roles(&[role("reader"), role("member")]),
            EffectiveRole::Member
        );
        assert_eq!(
            EffectiveRole::from_roles(&[role("member"), role("admin")]),
            EffectiveRole::Admin
        );
    }

    #[test]
    fn test_effective_role_unknown_ignored() {
        assert_eq!(
            EffectiveRole::from_roles(&[role("custom_role")]),
            EffectiveRole::Reader
        );
        // Unknown + known: known wins
        assert_eq!(
            EffectiveRole::from_roles(&[role("custom_role"), role("member")]),
            EffectiveRole::Member
        );
    }

    #[test]
    fn test_effective_role_case_insensitive() {
        assert_eq!(
            EffectiveRole::from_roles(&[role("Admin")]),
            EffectiveRole::Admin
        );
        assert_eq!(
            EffectiveRole::from_roles(&[role("MEMBER")]),
            EffectiveRole::Member
        );
    }

    #[test]
    fn test_effective_role_empty_roles() {
        assert_eq!(
            EffectiveRole::from_roles(&[]),
            EffectiveRole::Reader
        );
    }

    // --- RbacGuard tests (backward compatible) ---

    #[test]
    fn test_new_is_reader() {
        let guard = RbacGuard::new();
        assert!(!guard.is_admin());
        assert_eq!(guard.effective_role(), EffectiveRole::Reader);
        assert!(guard.project_id().is_none());
    }

    #[test]
    fn test_update_roles_admin() {
        let guard = RbacGuard::new();
        guard.update_roles(vec![role("admin"), role("member")], Some("proj-1".into()));
        assert!(guard.is_admin());
        assert_eq!(guard.effective_role(), EffectiveRole::Admin);
        assert_eq!(guard.project_id(), Some("proj-1".to_string()));
    }

    #[test]
    fn test_update_roles_member() {
        let guard = RbacGuard::new();
        guard.update_roles(vec![role("member"), role("reader")], None);
        assert!(!guard.is_admin());
        assert_eq!(guard.effective_role(), EffectiveRole::Member);
    }

    #[test]
    fn test_admin_case_insensitive() {
        let guard = RbacGuard::new();
        guard.update_roles(vec![role("Admin")], None);
        assert!(guard.is_admin());
    }

    // --- can_access_route ---

    #[test]
    fn test_can_access_route_admin() {
        let guard = RbacGuard::new();
        guard.update_roles(vec![role("admin")], None);
        assert!(guard.can_access_route(&Route::Migrations));
        assert!(guard.can_access_route(&Route::Servers));
    }

    #[test]
    fn test_can_access_route_member() {
        let guard = RbacGuard::new();
        guard.update_roles(vec![role("member")], None);
        assert!(!guard.can_access_route(&Route::Migrations));
        assert!(!guard.can_access_route(&Route::Projects));
        assert!(guard.can_access_route(&Route::Servers));
        assert!(guard.can_access_route(&Route::Networks));
        assert!(guard.can_access_route(&Route::Volumes));
    }

    #[test]
    fn test_can_access_route_reader() {
        let guard = RbacGuard::new();
        guard.update_roles(vec![role("reader")], None);
        assert!(!guard.can_access_route(&Route::Migrations));
        assert!(guard.can_access_route(&Route::Servers));
        assert!(guard.can_access_route(&Route::Networks));
    }

    // --- can_perform: 3-tier ---

    #[test]
    fn test_can_perform_admin() {
        let guard = RbacGuard::new();
        guard.update_roles(vec![role("admin")], None);
        assert!(guard.can_perform(ActionKind::Read));
        assert!(guard.can_perform(ActionKind::Create));
        assert!(guard.can_perform(ActionKind::Delete));
        assert!(guard.can_perform(ActionKind::ForceDelete));
        assert!(guard.can_perform(ActionKind::Migrate));
        assert!(guard.can_perform(ActionKind::Evacuate));
    }

    #[test]
    fn test_can_perform_member() {
        let guard = RbacGuard::new();
        guard.update_roles(vec![role("member")], None);
        assert!(guard.can_perform(ActionKind::Read));
        assert!(guard.can_perform(ActionKind::Create));
        assert!(guard.can_perform(ActionKind::Delete));
        assert!(!guard.can_perform(ActionKind::ForceDelete));
        assert!(!guard.can_perform(ActionKind::Migrate));
        assert!(!guard.can_perform(ActionKind::Evacuate));
        assert!(!guard.can_perform(ActionKind::EnableDisable));
        assert!(!guard.can_perform(ActionKind::ManageQuota));
    }

    #[test]
    fn test_can_perform_reader() {
        let guard = RbacGuard::new();
        guard.update_roles(vec![role("reader")], None);
        assert!(guard.can_perform(ActionKind::Read));
        assert!(!guard.can_perform(ActionKind::Create));
        assert!(!guard.can_perform(ActionKind::Delete));
        assert!(!guard.can_perform(ActionKind::ForceDelete));
        assert!(!guard.can_perform(ActionKind::Migrate));
    }

    // --- filter ---

    #[test]
    fn test_filter_routes() {
        let guard = RbacGuard::new();
        guard.update_roles(vec![role("member")], None);
        let routes = vec![Route::Servers, Route::Migrations, Route::Networks, Route::Projects];
        let filtered = guard.filter_routes(&routes);
        assert_eq!(filtered, vec![Route::Servers, Route::Networks]);
    }

    #[test]
    fn test_filter_actions_member() {
        let guard = RbacGuard::new();
        guard.update_roles(vec![role("member")], None);
        let actions = vec![ActionKind::Read, ActionKind::Delete, ActionKind::ForceDelete];
        let filtered = guard.filter_actions(&actions);
        assert_eq!(filtered, vec![ActionKind::Read, ActionKind::Delete]);
    }

    #[test]
    fn test_filter_actions_reader() {
        let guard = RbacGuard::new();
        guard.update_roles(vec![role("reader")], None);
        let actions = vec![ActionKind::Read, ActionKind::Create, ActionKind::Delete];
        let filtered = guard.filter_actions(&actions);
        assert_eq!(filtered, vec![ActionKind::Read]);
    }

    // --- capability (backward compatible) ---

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

        guard.update_roles(vec![role("member")], None);
        assert!(!guard.has_capability("server", "delete"));
    }
}
