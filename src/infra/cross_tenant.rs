/// Guard that controls cross-tenant (all_tenants) access.
///
/// By default, operations that span all tenants are blocked.
/// The operator must explicitly toggle "break glass" mode to proceed.
pub struct CrossTenantGuard {
    break_glass_active: bool,
    current_project_id: Option<String>,
}

impl CrossTenantGuard {
    pub fn new() -> Self {
        Self {
            break_glass_active: false,
            current_project_id: None,
        }
    }

    /// Set the current project ID from the auth token.
    pub fn set_project_id(&mut self, project_id: Option<String>) {
        self.current_project_id = project_id;
    }

    /// Check if a cross-tenant request should be blocked.
    ///
    /// - `all_tenants=true` and break glass inactive → blocked (true)
    /// - `all_tenants=true` and break glass active → allowed (false)
    /// - `all_tenants=false` → allowed (false)
    pub fn is_blocked(&self, all_tenants: bool, _resource_tenant_id: Option<&str>) -> bool {
        all_tenants && !self.break_glass_active
    }

    /// Toggle break-glass mode on/off. Returns the new state.
    pub fn toggle_break_glass(&mut self) -> bool {
        self.break_glass_active = !self.break_glass_active;
        self.break_glass_active
    }

    /// Check if a resource belongs to a different tenant than the current one.
    pub fn is_cross_tenant(&self, resource_tenant_id: Option<&str>) -> bool {
        match (&self.current_project_id, resource_tenant_id) {
            (Some(current), Some(resource)) => current != resource,
            (None, Some(_)) => true,
            (Some(_), None) => true,
            (None, None) => false,
        }
    }

    /// Check if break-glass mode is currently active.
    pub fn is_break_glass_active(&self) -> bool {
        self.break_glass_active
    }
}

impl Default for CrossTenantGuard {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // --- Construction ---

    #[test]
    fn test_new_defaults() {
        let guard = CrossTenantGuard::new();
        assert!(!guard.is_break_glass_active());
    }

    #[test]
    fn test_default_trait() {
        let guard = CrossTenantGuard::default();
        assert!(!guard.is_break_glass_active());
    }

    // --- is_blocked ---

    #[test]
    fn test_all_tenants_blocked_without_break_glass() {
        let guard = CrossTenantGuard::new();
        assert!(guard.is_blocked(true, None));
    }

    #[test]
    fn test_all_tenants_allowed_with_break_glass() {
        let mut guard = CrossTenantGuard::new();
        guard.toggle_break_glass();
        assert!(!guard.is_blocked(true, None));
    }

    #[test]
    fn test_single_tenant_never_blocked() {
        let guard = CrossTenantGuard::new();
        assert!(!guard.is_blocked(false, None));
        assert!(!guard.is_blocked(false, Some("proj-1")));
    }

    // --- toggle_break_glass ---

    #[test]
    fn test_toggle_returns_new_state() {
        let mut guard = CrossTenantGuard::new();
        assert!(guard.toggle_break_glass()); // false -> true
        assert!(!guard.toggle_break_glass()); // true -> false
        assert!(guard.toggle_break_glass()); // false -> true
    }

    // --- is_cross_tenant ---

    #[test]
    fn test_same_tenant_is_not_cross() {
        let mut guard = CrossTenantGuard::new();
        guard.set_project_id(Some("proj-1".to_string()));
        assert!(!guard.is_cross_tenant(Some("proj-1")));
    }

    #[test]
    fn test_different_tenant_is_cross() {
        let mut guard = CrossTenantGuard::new();
        guard.set_project_id(Some("proj-1".to_string()));
        assert!(guard.is_cross_tenant(Some("proj-2")));
    }

    #[test]
    fn test_no_current_project_with_resource_tenant() {
        let guard = CrossTenantGuard::new();
        assert!(guard.is_cross_tenant(Some("proj-1")));
    }

    #[test]
    fn test_current_project_with_no_resource_tenant() {
        let mut guard = CrossTenantGuard::new();
        guard.set_project_id(Some("proj-1".to_string()));
        assert!(guard.is_cross_tenant(None));
    }

    #[test]
    fn test_both_none_is_not_cross() {
        let guard = CrossTenantGuard::new();
        assert!(!guard.is_cross_tenant(None));
    }

    // --- set_project_id ---

    #[test]
    fn test_set_project_id_updates() {
        let mut guard = CrossTenantGuard::new();
        guard.set_project_id(Some("proj-1".to_string()));
        assert!(!guard.is_cross_tenant(Some("proj-1")));
        guard.set_project_id(Some("proj-2".to_string()));
        assert!(guard.is_cross_tenant(Some("proj-1")));
    }

    #[test]
    fn test_set_project_id_to_none() {
        let mut guard = CrossTenantGuard::new();
        guard.set_project_id(Some("proj-1".to_string()));
        guard.set_project_id(None);
        assert!(guard.is_cross_tenant(Some("proj-1")));
    }

    // --- Combined scenarios ---

    #[test]
    fn test_break_glass_with_cross_tenant_resource() {
        let mut guard = CrossTenantGuard::new();
        guard.set_project_id(Some("proj-1".to_string()));
        guard.toggle_break_glass();
        // Cross-tenant resource, but break glass is active → not blocked
        assert!(!guard.is_blocked(true, Some("proj-2")));
        assert!(guard.is_cross_tenant(Some("proj-2")));
    }
}
