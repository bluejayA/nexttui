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

/// Partition `items` into `(kept, dropped)` according to the scope policy
/// described in the module-level docstring. The function is allocation-
/// minimal — `kept` is pre-sized to match `items`, and `dropped` only
/// allocates when at least one item is rejected. Do NOT replace this with
/// `Iterator::partition`: that pre-allocates both sides, which is wasteful
/// for the common path (large list, zero drops).
pub fn refilter_by_scope<T: HasTenantId>(
    items: Vec<T>,
    active: Option<&str>,
    all_tenants: bool,
) -> (Vec<T>, Vec<T>) {
    if all_tenants {
        return (items, Vec::new());
    }
    let Some(active) = active else {
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

#[cfg(test)]
mod tests {
    use super::*;

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
        let (kept, dropped) = refilter_by_scope(items, Some("A"), false);
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
        let (kept, dropped) = refilter_by_scope(items, Some("A"), true);
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
        let (kept, dropped) = refilter_by_scope(items, Some("A"), false);
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
        let (kept, dropped) = refilter_by_scope(items, Some("A"), false);
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
        let (kept, dropped) = refilter_by_scope(items, None, false);
        assert_eq!(kept.len(), 2, "active=None must short-circuit (no-op)");
        assert!(dropped.is_empty());
    }
}
