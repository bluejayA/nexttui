//! BL-P2-085 Step 13a — cross-project response refilter (pure helper).
//!
//! Used by HTTP list adapters (Neutron Step 13b, Nova/Cinder Step 14) to
//! enforce project scoping on the response side. This is defense-in-depth
//! atop the server-side `tenant_id={scope}` injection wired in Step 12.
//!
//! Policy
//! - `all_tenants == true` (admin opt-out)         → no-op, `(items, [])`.
//! - `all_tenants == false && active.is_none()`    → no-op, `(items, [])`.
//!   The caller has no ground truth to compare against; worker-side guard
//!   already deny-blocks mutations in unscoped state via UnscopedFailSafe,
//!   so emitting per-item events here would be noise.
//! - `all_tenants == false && active.is_some()`    → strict refilter.
//!   Items whose `tenant_id() == active` go to `kept`; everything else —
//!   including items with `tenant_id() == None` — goes to `dropped` (the
//!   server returned a row we cannot prove belongs to the active scope).
//!
//! Emitting [`CrossProjectBlockEvent`] with reason
//! [`CrossProjectReason::AdapterFilterViolation`] per dropped item is the
//! caller's responsibility (Step 13b). The trait surfaces `resource_id` to
//! make that wiring ergonomic. When a dropped item's `tenant_id() == None`
//! (server returned a row without a project-id label), the caller MUST
//! still emit the event with the `tenant_id` field preserved as missing —
//! the audit chain depends on every drop being attributable.

use crate::models::neutron::{FloatingIp, Network, SecurityGroup};

/// Minimal contract a list item must satisfy to participate in
/// project-scope refiltering. `tenant_id` returns `None` when the underlying
/// model lacks a project-id field on the wire (treated as fail-safe drop
/// under strict scoping). `resource_id` is consumed by the AdapterFilter-
/// Violation event builder to report which row was rejected.
pub trait HasTenantId {
    /// Project-id label as returned by the upstream API. `None` means the
    /// model has no project-id field on the wire (or the server omitted
    /// it); under strict scoping such rows are dropped fail-safe.
    fn tenant_id(&self) -> Option<&str>;
    /// Stable identifier for audit reporting. `None` is tolerated for
    /// models without a primary id — the AdapterFilterViolation event
    /// will fall back to a placeholder rather than skipping the emit.
    fn resource_id(&self) -> Option<&str>;
}

/// Encodes the (active, all_tenants) invariant for [`refilter_by_scope`] in
/// three ctor-validated states. Constructed via [`RefilterScope::strict`],
/// [`RefilterScope::all_tenants`], [`RefilterScope::unscoped`], or
/// [`RefilterScope::from_parts`]; raw fields are kept private so an invalid
/// combination (e.g. `active=Some + all_tenants=true`) cannot be expressed.
#[derive(Debug, Clone, Copy)]
pub struct RefilterScope<'a> {
    active: Option<&'a str>,
    all_tenants: bool,
}

impl<'a> RefilterScope<'a> {
    /// Strict refilter: drop everything not matching `active`.
    pub fn strict(active: &'a str) -> Self {
        Self {
            active: Some(active),
            all_tenants: false,
        }
    }

    /// Admin opt-out: keep every row regardless of project.
    pub fn all_tenants() -> Self {
        Self {
            active: None,
            all_tenants: true,
        }
    }

    /// No scope to compare against (worker-side guard handles mutations).
    pub fn unscoped() -> Self {
        Self {
            active: None,
            all_tenants: false,
        }
    }

    /// Adapter from the legacy 2-arg shape. Normalizes `all_tenants=true` by
    /// clearing `active`, so the resulting scope always satisfies the
    /// ctor-validated invariant.
    pub fn from_parts(active: Option<&'a str>, all_tenants: bool) -> Self {
        if all_tenants {
            Self::all_tenants()
        } else if let Some(a) = active {
            Self::strict(a)
        } else {
            Self::unscoped()
        }
    }

    pub fn active(&self) -> Option<&'a str> {
        self.active
    }

    pub fn is_all_tenants(&self) -> bool {
        self.all_tenants
    }
}

/// Caller-provided sink for `AdapterFilterViolation` events. Step-14 adapter
/// audit contexts (Neutron/Nova/Cinder) implement this for any
/// `T: HasTenantId`, allowing [`refilter_and_audit`] to fan one event out
/// per dropped row colocated with the partition step. Generic over `T` so
/// each adapter context handles its native list-item type without erasing
/// `tenant_id` / `resource_id` to `&dyn HasTenantId`.
pub trait AuditEmitter<T: HasTenantId> {
    fn emit_filter_violations(
        &self,
        dropped: &[T],
        action_type: &str,
        resource_kind: &str,
        correlation_id: u64,
    );
}

/// Partition `items` into `(kept, dropped)` according to the scope policy
/// described in the module-level docstring. The function is allocation-
/// minimal — `kept` is pre-sized to match `items`, and `dropped` only
/// allocates when at least one item is rejected. Do NOT replace this with
/// `Iterator::partition`: that pre-allocates both sides, which is wasteful
/// for the common path (large list, zero drops).
pub fn refilter_by_scope<T: HasTenantId>(
    items: Vec<T>,
    scope: &RefilterScope<'_>,
) -> (Vec<T>, Vec<T>) {
    if scope.is_all_tenants() {
        return (items, Vec::new());
    }
    let Some(active) = scope.active() else {
        return (items, Vec::new());
    };
    let mut kept = Vec::with_capacity(items.len());
    let mut dropped = Vec::new();
    for item in items {
        match item.tenant_id() {
            Some(tid) if tid == active => kept.push(item),
            _ => dropped.push(item),
        }
    }
    (kept, dropped)
}

/// Partition `items` via [`refilter_by_scope`] and, when `audit` is `Some`
/// and `dropped` is non-empty, fan one event out per dropped row through
/// `audit.emit_filter_violations`. Returns only `kept` because the
/// `dropped` set is consumed by the audit path; callers that need both
/// vectors should call [`refilter_by_scope`] directly.
///
/// This colocates the partition with the audit emit so Step-14 adapters
/// (Nova/Cinder) can replace 8-line wrappers with a single call. The
/// generic `A` allows `Option<&NeutronAuditCtx>` callers to avoid
/// `dyn AuditEmitter<T>` erasure (each adapter has exactly one audit ctx
/// type at compile time).
pub fn refilter_and_audit<T, A>(
    items: Vec<T>,
    scope: &RefilterScope<'_>,
    audit: Option<&A>,
    action_type: &str,
    resource_kind: &str,
    correlation_id: u64,
) -> Vec<T>
where
    T: HasTenantId,
    A: AuditEmitter<T> + ?Sized,
{
    let (kept, dropped) = refilter_by_scope(items, scope);
    if !dropped.is_empty()
        && let Some(a) = audit
    {
        a.emit_filter_violations(&dropped, action_type, resource_kind, correlation_id);
    }
    kept
}

// --- BL-P2-085 Step 13b: HasTenantId impls for Neutron list models ---
// All three models share the same shape: `id: String` (always present) and
// `tenant_id: Option<String>` (server may omit under unusual configurations,
// in which case strict refiltering drops the row fail-safe).

impl HasTenantId for Network {
    fn tenant_id(&self) -> Option<&str> {
        self.tenant_id.as_deref()
    }
    fn resource_id(&self) -> Option<&str> {
        Some(&self.id)
    }
}

impl HasTenantId for SecurityGroup {
    fn tenant_id(&self) -> Option<&str> {
        self.tenant_id.as_deref()
    }
    fn resource_id(&self) -> Option<&str> {
        Some(&self.id)
    }
}

impl HasTenantId for FloatingIp {
    fn tenant_id(&self) -> Option<&str> {
        self.tenant_id.as_deref()
    }
    fn resource_id(&self) -> Option<&str> {
        Some(&self.id)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::neutron::{FloatingIp, Network, SecurityGroup};

    /// Test fixture mirroring the minimal shape required for refilter
    /// (id + optional tenant). Real impls (Network / SecurityGroup /
    /// FloatingIp / Server / Volume / Snapshot) land in Step 13b/14.
    #[derive(Debug, PartialEq, Eq)]
    struct FakeItem {
        id: &'static str,
        tenant: Option<&'static str>,
    }

    impl HasTenantId for FakeItem {
        fn tenant_id(&self) -> Option<&str> {
            self.tenant
        }
        fn resource_id(&self) -> Option<&str> {
            Some(self.id)
        }
    }

    #[test]
    fn test_refilter_drops_cross_project_items_when_scope_strict() {
        let items = vec![
            FakeItem {
                id: "a",
                tenant: Some("A"),
            },
            FakeItem {
                id: "b",
                tenant: Some("B"),
            },
            FakeItem {
                id: "c",
                tenant: Some("A"),
            },
        ];
        let (kept, dropped) = refilter_by_scope(items, &RefilterScope::strict("A"));
        assert_eq!(kept.len(), 2, "two A-tenant items should be kept");
        assert_eq!(dropped.len(), 1);
        assert_eq!(dropped[0].id, "b");
    }

    #[test]
    fn test_refilter_keeps_all_when_all_tenants_true() {
        let items = vec![
            FakeItem {
                id: "a",
                tenant: Some("A"),
            },
            FakeItem {
                id: "b",
                tenant: Some("B"),
            },
        ];
        let (kept, dropped) = refilter_by_scope(items, &RefilterScope::all_tenants());
        assert_eq!(kept.len(), 2, "all_tenants=true must short-circuit");
        assert!(dropped.is_empty());
    }

    #[test]
    fn test_refilter_keeps_active_scope_items() {
        let items = vec![
            FakeItem {
                id: "a1",
                tenant: Some("A"),
            },
            FakeItem {
                id: "a2",
                tenant: Some("A"),
            },
        ];
        let (kept, dropped) = refilter_by_scope(items, &RefilterScope::strict("A"));
        assert_eq!(kept.len(), 2);
        assert!(dropped.is_empty());
    }

    #[test]
    fn test_refilter_drops_items_with_missing_tenant_id_when_strict() {
        // Fail-safe: if the server returned an item with no tenant_id, we
        // cannot prove it belongs to the active scope, so we drop it.
        let items = vec![
            FakeItem {
                id: "x",
                tenant: None,
            },
            FakeItem {
                id: "a",
                tenant: Some("A"),
            },
        ];
        let (kept, dropped) = refilter_by_scope(items, &RefilterScope::strict("A"));
        assert_eq!(kept.len(), 1);
        assert_eq!(kept[0].id, "a");
        assert_eq!(dropped.len(), 1);
        assert_eq!(dropped[0].id, "x");
    }

    #[test]
    fn test_refilter_no_op_when_active_none() {
        let items = vec![
            FakeItem {
                id: "a",
                tenant: Some("A"),
            },
            FakeItem {
                id: "b",
                tenant: Some("B"),
            },
        ];
        let (kept, dropped) = refilter_by_scope(items, &RefilterScope::unscoped());
        assert_eq!(kept.len(), 2, "active=None must short-circuit (no-op)");
        assert!(dropped.is_empty());
    }

    // --- BL-P2-085 Step-14-precedent-refactor-cycle (refactor-1) ---
    // RefilterScope encodes the (active, all_tenants) invariant in three
    // ctor-validated states: strict / all_tenants / unscoped. Replaces the
    // previous 2-arg call shape so 5+ Step-14 callers cannot accidentally
    // pass active=Some + all_tenants=true (a meaningless combination).

    #[test]
    fn test_refilter_scope_strict_drops_cross_project_items() {
        let items = vec![
            FakeItem {
                id: "a",
                tenant: Some("A"),
            },
            FakeItem {
                id: "b",
                tenant: Some("B"),
            },
        ];
        let scope = RefilterScope::strict("A");
        let (kept, dropped) = refilter_by_scope(items, &scope);
        assert_eq!(kept.len(), 1);
        assert_eq!(kept[0].id, "a");
        assert_eq!(dropped.len(), 1);
        assert_eq!(dropped[0].id, "b");
    }

    #[test]
    fn test_refilter_scope_all_tenants_short_circuits() {
        let items = vec![
            FakeItem {
                id: "a",
                tenant: Some("A"),
            },
            FakeItem {
                id: "b",
                tenant: Some("B"),
            },
        ];
        let scope = RefilterScope::all_tenants();
        let (kept, dropped) = refilter_by_scope(items, &scope);
        assert_eq!(kept.len(), 2);
        assert!(dropped.is_empty());
    }

    #[test]
    fn test_refilter_scope_unscoped_short_circuits() {
        let items = vec![FakeItem {
            id: "a",
            tenant: Some("A"),
        }];
        let scope = RefilterScope::unscoped();
        let (kept, dropped) = refilter_by_scope(items, &scope);
        assert_eq!(kept.len(), 1);
        assert!(dropped.is_empty());
    }

    #[test]
    fn test_refilter_scope_from_parts_normalizes_invalid_combinations() {
        let strict = RefilterScope::from_parts(Some("A"), false);
        assert_eq!(strict.active(), Some("A"));
        assert!(!strict.is_all_tenants());

        let admin = RefilterScope::from_parts(Some("A"), true);
        assert_eq!(admin.active(), None, "all_tenants=true must clear active");
        assert!(admin.is_all_tenants());

        let unscoped = RefilterScope::from_parts(None, false);
        assert_eq!(unscoped.active(), None);
        assert!(!unscoped.is_all_tenants());
    }

    // --- BL-P2-085 Step-14-precedent-refactor-cycle (refactor-2) ---
    // `refilter_and_audit` colocates the partition with the per-row audit
    // emit. `AuditEmitter` is the trait Step-14 adapter contexts
    // (NeutronAuditCtx / NovaAuditCtx / CinderAuditCtx) implement so the
    // free fn stays generic over the audit sink.

    use std::cell::RefCell;

    /// Test double for `AuditEmitter` — records (dropped_len, action_type,
    /// resource_kind, correlation_id) per call so assertions can verify
    /// the emit was forwarded with the right arguments.
    struct CountingEmitter {
        calls: RefCell<Vec<(usize, String, String, u64)>>,
    }

    impl CountingEmitter {
        fn new() -> Self {
            Self {
                calls: RefCell::new(Vec::new()),
            }
        }
    }

    impl AuditEmitter<FakeItem> for CountingEmitter {
        fn emit_filter_violations(
            &self,
            dropped: &[FakeItem],
            action_type: &str,
            resource_kind: &str,
            correlation_id: u64,
        ) {
            self.calls.borrow_mut().push((
                dropped.len(),
                action_type.to_string(),
                resource_kind.to_string(),
                correlation_id,
            ));
        }
    }

    #[test]
    fn test_refilter_and_audit_emits_when_dropped_nonempty() {
        let emitter = CountingEmitter::new();
        let items = vec![
            FakeItem {
                id: "a",
                tenant: Some("A"),
            },
            FakeItem {
                id: "b",
                tenant: Some("B"),
            },
        ];
        let kept = refilter_and_audit(
            items,
            &RefilterScope::strict("A"),
            Some(&emitter),
            "FetchTest",
            "test_resource",
            42,
        );
        assert_eq!(kept.len(), 1);
        assert_eq!(kept[0].id, "a");
        let calls = emitter.calls.borrow();
        assert_eq!(calls.len(), 1, "emitter should be called exactly once");
        assert_eq!(calls[0].0, 1, "dropped_len=1");
        assert_eq!(calls[0].1, "FetchTest");
        assert_eq!(calls[0].2, "test_resource");
        assert_eq!(calls[0].3, 42);
    }

    #[test]
    fn test_refilter_and_audit_skips_emit_when_audit_none() {
        let items = vec![
            FakeItem {
                id: "a",
                tenant: Some("A"),
            },
            FakeItem {
                id: "b",
                tenant: Some("B"),
            },
        ];
        let kept = refilter_and_audit::<_, CountingEmitter>(
            items,
            &RefilterScope::strict("A"),
            None,
            "FetchTest",
            "test_resource",
            42,
        );
        assert_eq!(kept.len(), 1, "kept must still be filtered when audit=None");
    }

    #[test]
    fn test_refilter_and_audit_skips_emit_when_dropped_empty() {
        let emitter = CountingEmitter::new();
        let items = vec![FakeItem {
            id: "a",
            tenant: Some("A"),
        }];
        let kept = refilter_and_audit(
            items,
            &RefilterScope::strict("A"),
            Some(&emitter),
            "FetchTest",
            "test_resource",
            42,
        );
        assert_eq!(kept.len(), 1);
        assert!(
            emitter.calls.borrow().is_empty(),
            "emitter must not be called when dropped is empty"
        );
    }

    // --- BL-P2-085 Step 13b: HasTenantId impl for Neutron models ---
    // These tests assert that Network / SecurityGroup / FloatingIp implement
    // HasTenantId in a way that maps `tenant_id: Option<String>` →
    // `tenant_id()` and `id: String` → `resource_id()`.

    fn sample_network(id: &str, tenant: Option<&str>) -> Network {
        Network {
            id: id.to_string(),
            name: "n".to_string(),
            status: "ACTIVE".to_string(),
            description: None,
            admin_state_up: true,
            external: false,
            shared: false,
            mtu: None,
            port_security_enabled: None,
            subnets: Vec::new(),
            provider_network_type: None,
            provider_physical_network: None,
            provider_segmentation_id: None,
            tenant_id: tenant.map(str::to_string),
        }
    }

    fn sample_security_group(id: &str, tenant: Option<&str>) -> SecurityGroup {
        SecurityGroup {
            id: id.to_string(),
            name: "sg".to_string(),
            description: None,
            security_group_rules: Vec::new(),
            tenant_id: tenant.map(str::to_string),
        }
    }

    fn sample_floating_ip(id: &str, tenant: Option<&str>) -> FloatingIp {
        FloatingIp {
            id: id.to_string(),
            floating_ip_address: "203.0.113.1".to_string(),
            status: "ACTIVE".to_string(),
            port_id: None,
            floating_network_id: "ext".to_string(),
            fixed_ip_address: None,
            router_id: None,
            tenant_id: tenant.map(str::to_string),
        }
    }

    #[test]
    fn test_network_has_tenant_id_returns_some_when_present() {
        let net = sample_network("net-1", Some("proj-A"));
        assert_eq!(net.tenant_id(), Some("proj-A"));
        assert_eq!(net.resource_id(), Some("net-1"));
    }

    #[test]
    fn test_network_has_tenant_id_returns_none_when_absent() {
        let net = sample_network("net-2", None);
        assert_eq!(net.tenant_id(), None);
        assert_eq!(net.resource_id(), Some("net-2"));
    }

    #[test]
    fn test_security_group_has_tenant_id_returns_some_when_present() {
        let sg = sample_security_group("sg-1", Some("proj-B"));
        assert_eq!(sg.tenant_id(), Some("proj-B"));
        assert_eq!(sg.resource_id(), Some("sg-1"));
    }

    #[test]
    fn test_floating_ip_has_tenant_id_returns_some_when_present() {
        let fip = sample_floating_ip("fip-1", Some("proj-C"));
        assert_eq!(fip.tenant_id(), Some("proj-C"));
        assert_eq!(fip.resource_id(), Some("fip-1"));
    }

    #[test]
    fn test_floating_ip_has_tenant_id_returns_none_when_absent() {
        let fip = sample_floating_ip("fip-2", None);
        assert_eq!(fip.tenant_id(), None);
        assert_eq!(fip.resource_id(), Some("fip-2"));
    }
}
