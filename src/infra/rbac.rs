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
    Resize,
    Migrate,
    Evacuate,
    EnableDisable,
    ManageQuota,
    ViewAllTenants,
    Attach,
    Detach,
}

/// Effective role derived from Keystone token roles.
/// Ordered by privilege level: Reader < Member < Admin.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum EffectiveRole {
    Reader,
    Member,
    Admin,
}

/// Combined RBAC decision over (role-tier × project-scope).
/// Returned by `RbacGuard::check_project_scope` (FR3 of BL-P2-085).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RbacScopeDecision {
    Allow,
    Deny { reason: RbacDenialReason },
}

/// Why an RBAC scope check denied an action. Stable strings via `as_str()`
/// so audit consumers can grep.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RbacDenialReason {
    /// `can_perform(action)` returned false (insufficient privilege tier).
    RoleTier,
    /// Active scope ≠ target_project_id, or scope is unscoped (fail-safe).
    ProjectScope,
    /// Both role-tier and project-scope failed.
    Both,
}

impl RbacDenialReason {
    pub fn as_str(self) -> &'static str {
        match self {
            RbacDenialReason::RoleTier => "role_tier",
            RbacDenialReason::ProjectScope => "project_scope",
            RbacDenialReason::Both => "both",
        }
    }
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

impl RbacState {
    /// Snapshot-based scope decision. Operates purely on `&self` field reads
    /// so that callers holding a single `RwLock` read guard get an atomic
    /// (role × scope) decision — preventing the BL-P2-085 Codex P1 race
    /// where role and project_id could be sampled from different snapshots.
    fn scope_decision(&self, target_project_id: &str, action: ActionKind) -> RbacScopeDecision {
        let role_ok = match self.effective_role {
            EffectiveRole::Admin => true,
            EffectiveRole::Member => !RbacGuard::is_admin_only_action(action),
            EffectiveRole::Reader => action == ActionKind::Read,
        };
        let scope_ok = self
            .project_id
            .as_deref()
            .is_some_and(|p| p == target_project_id);
        match (role_ok, scope_ok) {
            (true, true) => RbacScopeDecision::Allow,
            (false, true) => RbacScopeDecision::Deny {
                reason: RbacDenialReason::RoleTier,
            },
            (true, false) => RbacScopeDecision::Deny {
                reason: RbacDenialReason::ProjectScope,
            },
            (false, false) => RbacScopeDecision::Deny {
                reason: RbacDenialReason::Both,
            },
        }
    }
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

    /// Refresh roles and re-derive `effective_role` without touching
    /// `project_id`. Used on token re-issue paths where the active project
    /// has not changed (e.g. Keystone token refresh inside the same scope).
    ///
    /// Mirrors `update_roles` by clearing capabilities so callers must
    /// repopulate them via `update_capabilities` if needed.
    pub fn update_roles_preserve_project(&self, roles: Vec<TokenRole>) {
        let effective = EffectiveRole::from_roles(&roles);
        if let Ok(mut s) = self.state.write() {
            s.roles = roles;
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
    /// Admin: all routes. Member/Reader: non-admin routes only.
    pub fn can_access_route(&self, route: &Route) -> bool {
        match self.effective_role() {
            EffectiveRole::Admin => true,
            _ => !Self::is_admin_only_route(route),
        }
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

    /// Combined RBAC check: role-tier (`can_perform`) + project-scope
    /// (`target_project_id == active project_id`). Returns a structured
    /// reason so audit consumers and toasts can disambiguate role-tier vs
    /// scope-mismatch denials. Unscoped guard (`project_id == None`) is
    /// treated as a scope-mismatch fail-safe.
    ///
    /// Atomicity (Codex P1, 2026-04-28): role and project_id are read from a
    /// single `state.read()` snapshot via `RbacState::scope_decision`, so a
    /// concurrent `update_roles*` cannot interleave between the two reads.
    /// On a poisoned lock the decision falls back to `Deny { Both }`
    /// (fail-safe — no privilege should be granted on corrupt state).
    pub fn check_project_scope(
        &self,
        target_project_id: &str,
        action: ActionKind,
    ) -> RbacScopeDecision {
        self.state
            .read()
            .map(|s| s.scope_decision(target_project_id, action))
            .unwrap_or(RbacScopeDecision::Deny {
                reason: RbacDenialReason::Both,
            })
    }

    /// Capability-based permission check.
    /// If capabilities are populated, checks against them.
    /// Otherwise falls back to role-based check: Admin = all, Member/Reader = denied.
    /// Note: Member permissions without capabilities should use `can_perform()` instead.
    /// Phase 2 will populate capabilities for Member via `update_capabilities()`.
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
                | Route::Hosts
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
                | ActionKind::ViewAllTenants
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
        assert_eq!(
            EffectiveRole::from_roles(&[role("reader"), role("admin")]),
            EffectiveRole::Admin
        );
        assert_eq!(
            EffectiveRole::from_roles(&[role("operator"), role("admin")]),
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
        assert_eq!(EffectiveRole::from_roles(&[]), EffectiveRole::Reader);
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
        assert!(!guard.can_perform(ActionKind::Evacuate));
        assert!(!guard.can_perform(ActionKind::EnableDisable));
        assert!(!guard.can_perform(ActionKind::ManageQuota));
    }

    // --- filter ---

    #[test]
    fn test_filter_routes() {
        let guard = RbacGuard::new();
        guard.update_roles(vec![role("member")], None);
        let routes = vec![
            Route::Servers,
            Route::Migrations,
            Route::Networks,
            Route::Projects,
        ];
        let filtered = guard.filter_routes(&routes);
        assert_eq!(filtered, vec![Route::Servers, Route::Networks]);
    }

    #[test]
    fn test_filter_actions_member() {
        let guard = RbacGuard::new();
        guard.update_roles(vec![role("member")], None);
        let actions = vec![
            ActionKind::Read,
            ActionKind::Delete,
            ActionKind::ForceDelete,
        ];
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

    // --- ViewAllTenants (admin-only) ---

    #[test]
    fn test_view_all_tenants_admin_allowed() {
        let guard = RbacGuard::new();
        guard.update_roles(vec![role("admin")], None);
        assert!(guard.can_perform(ActionKind::ViewAllTenants));
    }

    #[test]
    fn test_view_all_tenants_member_denied() {
        let guard = RbacGuard::new();
        guard.update_roles(vec![role("member")], None);
        assert!(!guard.can_perform(ActionKind::ViewAllTenants));
    }

    #[test]
    fn test_view_all_tenants_reader_denied() {
        let guard = RbacGuard::new();
        assert!(!guard.can_perform(ActionKind::ViewAllTenants));
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

    // --- Resize RBAC (member-level) ---

    #[test]
    fn test_resize_admin_allowed() {
        let guard = RbacGuard::new();
        guard.update_roles(vec![role("admin")], None);
        assert!(guard.can_perform(ActionKind::Resize));
    }

    #[test]
    fn test_resize_member_allowed() {
        let guard = RbacGuard::new();
        guard.update_roles(vec![role("member")], None);
        assert!(guard.can_perform(ActionKind::Resize));
    }

    #[test]
    fn test_resize_reader_denied() {
        let guard = RbacGuard::new();
        guard.update_roles(vec![role("reader")], None);
        assert!(!guard.can_perform(ActionKind::Resize));
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

    // --- BL-P2-085 Step 5: check_project_scope (FR3 RBAC project-scope) ---

    #[test]
    fn test_check_project_scope_admin_match_allows() {
        let guard = RbacGuard::new();
        guard.update_roles(vec![role("admin")], Some("proj-A".into()));
        assert_eq!(
            guard.check_project_scope("proj-A", ActionKind::Create),
            RbacScopeDecision::Allow
        );
    }

    #[test]
    fn test_check_project_scope_admin_mismatch_denies_project_scope() {
        let guard = RbacGuard::new();
        guard.update_roles(vec![role("admin")], Some("proj-A".into()));
        assert_eq!(
            guard.check_project_scope("proj-B", ActionKind::Delete),
            RbacScopeDecision::Deny {
                reason: RbacDenialReason::ProjectScope
            }
        );
    }

    #[test]
    fn test_check_project_scope_unscoped_denies_project_scope_fail_safe() {
        let guard = RbacGuard::new();
        guard.update_roles(vec![role("admin")], None); // unscoped admin
        assert_eq!(
            guard.check_project_scope("proj-A", ActionKind::Create),
            RbacScopeDecision::Deny {
                reason: RbacDenialReason::ProjectScope
            },
            "None scope must fail-safe deny on the scope dimension"
        );
    }

    #[test]
    fn test_check_project_scope_reader_create_match_denies_role_tier() {
        let guard = RbacGuard::new();
        guard.update_roles(vec![role("reader")], Some("proj-A".into()));
        assert_eq!(
            guard.check_project_scope("proj-A", ActionKind::Create),
            RbacScopeDecision::Deny {
                reason: RbacDenialReason::RoleTier
            }
        );
    }

    #[test]
    fn test_check_project_scope_reader_create_mismatch_denies_both() {
        let guard = RbacGuard::new();
        guard.update_roles(vec![role("reader")], Some("proj-A".into()));
        assert_eq!(
            guard.check_project_scope("proj-B", ActionKind::Create),
            RbacScopeDecision::Deny {
                reason: RbacDenialReason::Both
            }
        );
    }

    #[test]
    fn test_check_project_scope_member_admin_only_action_match_denies_role_tier() {
        let guard = RbacGuard::new();
        guard.update_roles(vec![role("member")], Some("proj-A".into()));
        // ForceDelete is admin-only; scope matches
        assert_eq!(
            guard.check_project_scope("proj-A", ActionKind::ForceDelete),
            RbacScopeDecision::Deny {
                reason: RbacDenialReason::RoleTier
            }
        );
    }

    #[test]
    fn test_check_project_scope_reader_read_match_allows() {
        // Reader can Read in own scope
        let guard = RbacGuard::new();
        guard.update_roles(vec![role("reader")], Some("proj-A".into()));
        assert_eq!(
            guard.check_project_scope("proj-A", ActionKind::Read),
            RbacScopeDecision::Allow
        );
    }

    // --- BL-P2-085 Step 6: update_roles_preserve_project (token re-issue path) ---

    #[test]
    fn test_preserve_project_keeps_existing_project_id() {
        let guard = RbacGuard::new();
        guard.update_roles(vec![role("admin")], Some("proj-A".into()));
        guard.update_roles_preserve_project(vec![role("member")]);
        assert_eq!(
            guard.project_id(),
            Some("proj-A".to_string()),
            "preserve must not touch project_id"
        );
    }

    #[test]
    fn test_preserve_project_updates_roles_and_effective() {
        let guard = RbacGuard::new();
        guard.update_roles(vec![role("admin")], Some("proj-A".into()));
        assert_eq!(guard.effective_role(), EffectiveRole::Admin);

        guard.update_roles_preserve_project(vec![role("reader")]);
        assert_eq!(guard.effective_role(), EffectiveRole::Reader);
        assert!(!guard.is_admin());
    }

    #[test]
    fn test_update_roles_vs_preserve_diff() {
        // Regression: original update_roles still overwrites project_id.
        let guard1 = RbacGuard::new();
        guard1.update_roles(vec![role("admin")], Some("proj-A".into()));
        guard1.update_roles(vec![role("member")], Some("proj-B".into()));
        assert_eq!(guard1.project_id(), Some("proj-B".to_string()));

        let guard2 = RbacGuard::new();
        guard2.update_roles(vec![role("admin")], Some("proj-A".into()));
        guard2.update_roles_preserve_project(vec![role("member")]);
        assert_eq!(guard2.project_id(), Some("proj-A".to_string()));
    }

    #[test]
    fn test_preserve_project_clears_capabilities_parity_with_update_roles() {
        let guard = RbacGuard::new();
        guard.update_roles(vec![role("admin")], Some("proj-A".into()));
        guard.update_capabilities(vec![Capability {
            resource: "server".to_string(),
            action: "delete".to_string(),
        }]);
        assert!(guard.has_capability("server", "delete"));

        guard.update_roles_preserve_project(vec![role("member")]);
        assert!(
            !guard.has_capability("server", "delete"),
            "preserve must clear capabilities for parity with update_roles"
        );
    }

    // --- BL-P2-085 Codex P1: atomic scope decision over single state snapshot ---

    #[test]
    fn test_rbac_state_scope_decision_atomic_snapshot() {
        // Codex P1 fix: check_project_scope must read role + project_id from
        // a single RwLock snapshot. This test exercises RbacState::scope_decision
        // directly (no lock involved) to lock in the snapshot-based contract:
        // any future race-fix refactor must keep the decision derivable from
        // a single &RbacState reference.
        let state = RbacState {
            roles: vec![role("admin")],
            project_id: Some("proj-a".into()),
            effective_role: EffectiveRole::Admin,
            capabilities: HashSet::new(),
        };
        assert_eq!(
            state.scope_decision("proj-a", ActionKind::Create),
            RbacScopeDecision::Allow,
            "admin in matching scope must Allow"
        );
        assert_eq!(
            state.scope_decision("proj-b", ActionKind::Create),
            RbacScopeDecision::Deny {
                reason: RbacDenialReason::ProjectScope
            },
            "admin in mismatched scope must Deny ProjectScope"
        );

        let unscoped = RbacState {
            roles: vec![role("admin")],
            project_id: None,
            effective_role: EffectiveRole::Admin,
            capabilities: HashSet::new(),
        };
        assert_eq!(
            unscoped.scope_decision("proj-a", ActionKind::Create),
            RbacScopeDecision::Deny {
                reason: RbacDenialReason::ProjectScope
            },
            "unscoped guard must fail-safe to ProjectScope denial"
        );

        let reader = RbacState {
            roles: vec![role("reader")],
            project_id: Some("proj-a".into()),
            effective_role: EffectiveRole::Reader,
            capabilities: HashSet::new(),
        };
        assert_eq!(
            reader.scope_decision("proj-b", ActionKind::Create),
            RbacScopeDecision::Deny {
                reason: RbacDenialReason::Both
            },
            "role-tier + scope mismatch must Deny Both"
        );
    }
}
