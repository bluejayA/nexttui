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

use crate::models::cinder::{Volume, VolumeSnapshot};
use crate::models::glance::Image;
use crate::models::neutron::{FloatingIp, Network, SecurityGroup};
use crate::models::nova::Server;

/// Minimal contract a list item must satisfy to participate in
/// project-scope refiltering. `tenant_id` returns `None` when the underlying
/// model lacks a project-id field on the wire (treated as fail-safe drop
/// under strict scoping). `resource_id` is consumed by the AdapterFilter-
/// Violation event builder to report which row was rejected.
pub trait ScopedItem {
    /// Project-id label as returned by the upstream API. `None` means the
    /// model has no project-id field on the wire (or the server omitted
    /// it); under strict scoping such rows are dropped fail-safe.
    fn tenant_id(&self) -> Option<&str>;
    /// Stable identifier for audit reporting. `None` is tolerated for
    /// models without a primary id — the AdapterFilterViolation event
    /// will fall back to a placeholder rather than skipping the emit.
    fn resource_id(&self) -> Option<&str>;
    /// BL-P2-091: short-circuit keep for rows whose access model is
    /// governed by something other than `tenant_id` equality. The canonical
    /// case is Glance: `visibility = public/community/shared` images are
    /// intentionally cross-project, and their `owner` is often a different
    /// project (or absent) — owner-equality refilter would drop them and
    /// break the standard non-admin `list_images` flow.
    ///
    /// Default `false` preserves Neutron/Nova/Cinder behaviour (tenant_id
    /// equality is the only authoritative scope test). Image overrides
    /// to keep `visibility != "private"` rows regardless of `owner`.
    ///
    /// Globally-accessible rows bypass refilter entirely; they neither
    /// land in `kept` nor `dropped` from the filtering perspective — they
    /// are returned unchanged to the caller and never emit an
    /// AdapterFilterViolation event (no leak signal).
    fn is_globally_accessible(&self) -> bool {
        false
    }
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
    ///
    /// `active` must be non-empty — an empty string would cause every row
    /// to be dropped silently (since list models never carry
    /// `tenant_id == ""`). The `debug_assert!` catches the caller bug in
    /// dev builds; release builds rely on the [`from_parts`] empty-string
    /// normalization (which is the only production caller).
    ///
    /// [`from_parts`]: RefilterScope::from_parts
    pub fn strict(active: &'a str) -> Self {
        debug_assert!(
            !active.is_empty(),
            "RefilterScope::strict requires a non-empty active project id — empty would drop all rows"
        );
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

    /// Adapter from the legacy 2-arg shape. Normalizes two corner cases so
    /// the resulting scope always satisfies the ctor-validated invariant:
    ///   - `all_tenants=true` wins over `active` (cleared to None).
    ///   - `active=Some("")` is treated as `None` (unscoped) so an empty
    ///     project id from `scope_provider` cannot route into the
    ///     [`strict`] panic path.
    ///
    /// [`strict`]: RefilterScope::strict
    pub fn from_parts(active: Option<&'a str>, all_tenants: bool) -> Self {
        if all_tenants {
            Self::all_tenants()
        } else {
            match active {
                Some(a) if !a.is_empty() => Self::strict(a),
                _ => Self::unscoped(),
            }
        }
    }

    /// Active project id under strict scoping. `None` for `all_tenants`
    /// or unscoped — callers should branch on this together with
    /// [`is_all_tenants`] when reconstructing the policy.
    ///
    /// [`is_all_tenants`]: RefilterScope::is_all_tenants
    pub fn active(&self) -> Option<&'a str> {
        self.active
    }

    /// `true` when the scope is the admin opt-out (every row kept). Always
    /// implies [`active`] is `None`; the ctor invariant rules out the
    /// `active=Some + all_tenants=true` combination.
    ///
    /// [`active`]: RefilterScope::active
    pub fn is_all_tenants(&self) -> bool {
        self.all_tenants
    }
}

/// Caller-provided sink for `AdapterFilterViolation` events. Step-14 adapter
/// audit contexts (Neutron/Nova/Cinder) implement this for any
/// `T: ScopedItem`, allowing [`refilter_and_audit`] to fan one event out
/// per dropped row colocated with the partition step. Generic over `T` so
/// each adapter context handles its native list-item type without erasing
/// `tenant_id` / `resource_id` to `&dyn ScopedItem`.
pub trait AuditEmitter<T: ScopedItem> {
    /// Emit one `CrossProjectBlockEvent` with reason `AdapterFilterViolation`
    /// per dropped row. Implementations MUST be no-op when `dropped` is
    /// empty (callers rely on this to avoid touching the audit log on the
    /// zero-violation path), and MUST attribute every dropped row — even
    /// rows whose `tenant_id()` is `None` — so the audit chain stays
    /// loss-less per the module-level contract.
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
pub fn refilter_by_scope<T: ScopedItem>(
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
        // BL-P2-091: globally-accessible rows (Glance public/community/
        // shared) bypass the owner check — their visibility marker is
        // the authoritative access decision, and their `tenant_id` is
        // routinely a different project (or absent). Drop would break
        // standard non-admin list_images for everyone.
        if item.is_globally_accessible() {
            kept.push(item);
            continue;
        }
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
    T: ScopedItem,
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

// --- BL-P2-085 Step 13b: ScopedItem impls for Neutron list models ---
// All three models share the same shape: `id: String` (always present) and
// `tenant_id: Option<String>` (server may omit under unusual configurations,
// in which case strict refiltering drops the row fail-safe).

impl ScopedItem for Network {
    fn tenant_id(&self) -> Option<&str> {
        self.tenant_id.as_deref()
    }
    fn resource_id(&self) -> Option<&str> {
        Some(&self.id)
    }
}

impl ScopedItem for SecurityGroup {
    fn tenant_id(&self) -> Option<&str> {
        self.tenant_id.as_deref()
    }
    fn resource_id(&self) -> Option<&str> {
        Some(&self.id)
    }
}

impl ScopedItem for FloatingIp {
    fn tenant_id(&self) -> Option<&str> {
        self.tenant_id.as_deref()
    }
    fn resource_id(&self) -> Option<&str> {
        Some(&self.id)
    }
}

// --- BL-P2-085 Step 14: ScopedItem impls for Nova/Cinder list models ---
// `Server.tenant_id: Option<String>` is direct; `Volume.tenant_id` and
// `VolumeSnapshot.tenant_id` are Rust-side renames of the upstream
// `os-vol-tenant-attr:tenant_id` / `os-extended-snapshot-attributes:
// project_id` wire fields, so the impl shape is identical.

impl ScopedItem for Server {
    fn tenant_id(&self) -> Option<&str> {
        self.tenant_id.as_deref()
    }
    fn resource_id(&self) -> Option<&str> {
        Some(&self.id)
    }
}

impl ScopedItem for Volume {
    fn tenant_id(&self) -> Option<&str> {
        self.tenant_id.as_deref()
    }
    fn resource_id(&self) -> Option<&str> {
        Some(&self.id)
    }
}

impl ScopedItem for VolumeSnapshot {
    fn tenant_id(&self) -> Option<&str> {
        self.tenant_id.as_deref()
    }
    fn resource_id(&self) -> Option<&str> {
        Some(&self.id)
    }
}

// --- BL-P2-091: ScopedItem impl for Glance Image ---
// Glance is the asymmetric branch. `owner` is the project-id equivalent of
// `tenant_id` on Neutron/Nova/Cinder, but Glance's visibility model
// (`public` / `private` / `shared` / `community`) means the OWNER check
// only applies to `private` rows. Public/community/shared images are
// intentionally cross-project — their visibility marker IS the access
// decision, and refusing them would break the standard non-admin
// `list_images` flow (no public Ubuntu image, etc).
//
// `is_globally_accessible()` therefore short-circuits the refilter for any
// non-private visibility. The FR1 leak signal we still catch:
//   `visibility == "private"` AND `owner != active`
// — a private image of another project surfaced into the response.

impl ScopedItem for Image {
    fn tenant_id(&self) -> Option<&str> {
        self.owner.as_deref()
    }
    fn resource_id(&self) -> Option<&str> {
        Some(&self.id)
    }
    fn is_globally_accessible(&self) -> bool {
        // Glance v2 visibility is one of `public`, `private`, `community`,
        // `shared`. The three non-private values are governed by
        // visibility ACLs server-side and must be passed through; positive
        // allowlist (not `!= "private"`) so any unrecognized value
        // fail-safes to the owner refilter rather than silently bypassing
        // it when a new variant ships.
        matches!(self.visibility.as_str(), "public" | "community" | "shared")
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

    impl ScopedItem for FakeItem {
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

    // --- cargo-review SECURITY follow-ups (Sugg #1 + #2) ---
    // Guard against the two latent footguns the SECURITY reviewer raised:
    //   #1 `strict("")` would silently drop every row (tenant_id == "")
    //   #2 `from_parts(Some(""), false)` would also reach strict("") via
    //      the legacy 2-arg adapter path.

    #[test]
    #[should_panic(expected = "non-empty active")]
    fn test_refilter_scope_strict_panics_on_empty_active() {
        // `strict("")` is a caller bug — every row's `tenant_id == ""` test
        // would fail, dropping all data. Caught in debug builds.
        let _ = RefilterScope::strict("");
    }

    #[test]
    fn test_refilter_scope_from_parts_treats_empty_string_as_unscoped() {
        // Fail-safe: if `scope_provider.current_project_id()` ever yields
        // `Some("")` (unscoped session, partial token, etc.), the legacy
        // 2-arg path must NOT route into the panic above. Normalize to
        // unscoped (refilter no-op) so the worker-side guard handles it.
        let scope = RefilterScope::from_parts(Some(""), false);
        assert_eq!(
            scope.active(),
            None,
            "empty string must be normalized to None"
        );
        assert!(!scope.is_all_tenants());
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

    // --- BL-P2-085 Step-14-precedent-refactor-cycle (refactor-3) ---
    // Trait rename `HasTenantId` → `ScopedItem` because the contract is
    // "this row participates in scope comparison", not merely "has a
    // tenant_id field". The new name accommodates Step 14 models like
    // Cinder that may carry `project_id` instead of `tenant_id`.
    #[test]
    fn test_scoped_item_trait_used_for_refilter_signature() {
        fn _bound<T: ScopedItem>() {}
        _bound::<FakeItem>();
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

    // --- BL-P2-085 Step 13b: ScopedItem impl for Neutron models ---
    // These tests assert that Network / SecurityGroup / FloatingIp implement
    // ScopedItem in a way that maps `tenant_id: Option<String>` →
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

    // --- BL-P2-085 Step 14: ScopedItem impls for Nova/Cinder models ---
    // Server (nova): `tenant_id: Option<String>` direct.
    // Volume (cinder): wire field `os-vol-tenant-attr:tenant_id` renamed
    //   to `tenant_id` in the struct.
    // VolumeSnapshot (cinder): wire field
    //   `os-extended-snapshot-attributes:project_id` renamed to
    //   `tenant_id` in the struct — same Rust-side shape as Neutron and
    //   Nova, so the impl is identical.

    fn sample_server(id: &str, tenant: Option<&str>) -> crate::models::nova::Server {
        crate::models::nova::Server {
            id: id.to_string(),
            name: "srv".to_string(),
            status: "ACTIVE".to_string(),
            addresses: std::collections::HashMap::new(),
            flavor: crate::models::nova::FlavorRef {
                id: "f-1".to_string(),
                original_name: None,
                vcpus: None,
                ram: None,
                disk: None,
            },
            image: None,
            key_name: None,
            availability_zone: None,
            created: "2026-01-01T00:00:00Z".to_string(),
            updated: None,
            tenant_id: tenant.map(str::to_string),
            host_id: None,
            host: None,
            volumes_attached: Vec::new(),
            security_groups: Vec::new(),
        }
    }

    fn sample_volume(id: &str, tenant: Option<&str>) -> crate::models::cinder::Volume {
        crate::models::cinder::Volume {
            id: id.to_string(),
            name: None,
            description: None,
            status: "available".to_string(),
            size: 1,
            volume_type: None,
            encrypted: false,
            bootable: "false".to_string(),
            attachments: Vec::new(),
            availability_zone: None,
            created_at: None,
            tenant_id: tenant.map(str::to_string),
        }
    }

    fn sample_volume_snapshot(
        id: &str,
        tenant: Option<&str>,
    ) -> crate::models::cinder::VolumeSnapshot {
        crate::models::cinder::VolumeSnapshot {
            id: id.to_string(),
            name: None,
            status: "available".to_string(),
            size: 1,
            volume_id: "vol-1".to_string(),
            created_at: None,
            tenant_id: tenant.map(str::to_string),
        }
    }

    #[test]
    fn test_server_has_scoped_item_returns_some_when_present() {
        let srv = sample_server("srv-1", Some("proj-A"));
        assert_eq!(srv.tenant_id(), Some("proj-A"));
        assert_eq!(srv.resource_id(), Some("srv-1"));
    }

    #[test]
    fn test_server_has_scoped_item_returns_none_when_absent() {
        let srv = sample_server("srv-2", None);
        assert_eq!(srv.tenant_id(), None);
        assert_eq!(srv.resource_id(), Some("srv-2"));
    }

    #[test]
    fn test_volume_has_scoped_item_returns_some_when_present() {
        let vol = sample_volume("vol-1", Some("proj-B"));
        assert_eq!(vol.tenant_id(), Some("proj-B"));
        assert_eq!(vol.resource_id(), Some("vol-1"));
    }

    #[test]
    fn test_volume_has_scoped_item_returns_none_when_absent() {
        let vol = sample_volume("vol-2", None);
        assert_eq!(vol.tenant_id(), None);
        assert_eq!(vol.resource_id(), Some("vol-2"));
    }

    #[test]
    fn test_volume_snapshot_has_scoped_item_returns_some_when_present() {
        let snap = sample_volume_snapshot("snap-1", Some("proj-C"));
        assert_eq!(snap.tenant_id(), Some("proj-C"));
        assert_eq!(snap.resource_id(), Some("snap-1"));
    }

    #[test]
    fn test_volume_snapshot_has_scoped_item_returns_none_when_absent() {
        let snap = sample_volume_snapshot("snap-2", None);
        assert_eq!(snap.tenant_id(), None);
        assert_eq!(snap.resource_id(), Some("snap-2"));
    }

    // --- BL-P2-091: ScopedItem impl for Glance Image ---
    // `Image.owner: Option<String>` maps to `tenant_id()` (Glance v2 uses
    // `owner` rather than `tenant_id` on the wire). `Image.id: String`
    // always present → `resource_id()`.

    fn sample_image(id: &str, owner: Option<&str>) -> crate::models::glance::Image {
        crate::models::glance::Image {
            id: id.to_string(),
            name: "img".to_string(),
            status: "active".to_string(),
            disk_format: None,
            container_format: None,
            size: None,
            visibility: "private".to_string(),
            min_disk: 0,
            min_ram: 0,
            checksum: None,
            created_at: None,
            owner: owner.map(str::to_string),
        }
    }

    #[test]
    fn test_image_has_scoped_item_returns_some_when_present() {
        let img = sample_image("img-1", Some("proj-A"));
        assert_eq!(img.tenant_id(), Some("proj-A"));
        assert_eq!(img.resource_id(), Some("img-1"));
    }

    #[test]
    fn test_image_has_scoped_item_returns_none_when_absent() {
        // Private image without an `owner` field — under strict scoping the
        // refilter drops it fail-safe (no proof of ownership).
        let img = sample_image("img-2", None);
        assert_eq!(img.tenant_id(), None);
        assert_eq!(img.resource_id(), Some("img-2"));
        assert!(
            !img.is_globally_accessible(),
            "default sample_image is private; owner check applies"
        );
    }

    // --- BL-P2-091 Codex P1 fix: visibility-aware short-circuit ---
    // Glance visibility marker IS the access decision for public/community/
    // shared images. Refilter must let them through regardless of `owner`,
    // or non-admin users lose access to the standard image catalog (Ubuntu
    // public images, distro AMIs, etc.).

    fn sample_image_with_visibility(
        id: &str,
        owner: Option<&str>,
        visibility: &str,
    ) -> crate::models::glance::Image {
        let mut img = sample_image(id, owner);
        img.visibility = visibility.to_string();
        img
    }

    #[test]
    fn test_image_is_globally_accessible_when_visibility_public() {
        let img = sample_image_with_visibility("img-pub", Some("other-proj"), "public");
        assert!(
            img.is_globally_accessible(),
            "public images must bypass owner refilter"
        );
    }

    #[test]
    fn test_image_is_globally_accessible_when_visibility_community() {
        let img = sample_image_with_visibility("img-com", Some("other-proj"), "community");
        assert!(img.is_globally_accessible());
    }

    #[test]
    fn test_image_is_globally_accessible_when_visibility_shared() {
        let img = sample_image_with_visibility("img-sh", Some("other-proj"), "shared");
        assert!(img.is_globally_accessible());
    }

    #[test]
    fn test_image_is_not_globally_accessible_when_visibility_private() {
        let img = sample_image_with_visibility("img-priv", Some("other-proj"), "private");
        assert!(
            !img.is_globally_accessible(),
            "private images must apply owner refilter"
        );
    }

    #[test]
    fn test_image_unknown_visibility_treated_as_private_fail_safe() {
        // Unknown visibility values must NOT relax the refilter — future
        // Glance versions adding a new variant shouldn't silently bypass
        // the owner check.
        let img = sample_image_with_visibility("img-?", Some("other-proj"), "unobtainium");
        assert!(
            !img.is_globally_accessible(),
            "unknown visibility must fail-safe to owner refilter"
        );
    }

    #[test]
    fn test_refilter_keeps_public_image_even_when_cross_project() {
        // Codex P1 regression: a non-admin user listing images under
        // strict scope must still see public/community/shared images that
        // happen to be owned by a different project.
        let items = vec![
            sample_image_with_visibility("pub-1", Some("admin-proj"), "public"),
            sample_image_with_visibility("priv-mine", Some("proj-A"), "private"),
            sample_image_with_visibility("priv-other", Some("proj-B"), "private"),
        ];
        let (kept, dropped) = refilter_by_scope(items, &RefilterScope::strict("proj-A"));
        let kept_ids: Vec<&str> = kept.iter().map(|i| i.id.as_str()).collect();
        let dropped_ids: Vec<&str> = dropped.iter().map(|i| i.id.as_str()).collect();
        assert!(
            kept_ids.contains(&"pub-1"),
            "public image must survive refilter (kept_ids={kept_ids:?})"
        );
        assert!(
            kept_ids.contains(&"priv-mine"),
            "private image owned by active project must be kept"
        );
        assert_eq!(
            dropped_ids,
            vec!["priv-other"],
            "only private cross-project images must be dropped"
        );
    }
}
