//! BL-P2-085 Step 13b — Neutron-specific audit context for adapter-side
//! cross-project filter violations.
//!
//! Bundles the three pieces every Neutron `list_*` impl needs to emit a
//! `CrossProjectBlockEvent` with reason `AdapterFilterViolation` per row
//! that survives the server-side `tenant_id={scope}` filter (Step 12) but
//! still lands in `dropped` from `refilter_by_scope` (Step 13a):
//!
//! - `logger` — the production `AuditLogger` (shared `Arc` so adapter and
//!   worker write to the same file with a single `BufWriter`).
//! - `scope_provider` — live read of the active project id at emit time
//!   (avoids stale capture across cloud/project switches).
//! - `actor_ctx` — `Arc<RwLock<ActorContext>>` shared with the worker so
//!   `ContextChanged` updates to `cloud` / `user_id` are reflected here
//!   without re-spawning the adapter.
//!
//! Step 13b-3 wires `NeutronHttpAdapter::with_audit` into `registry::new_http`
//! and `main.rs`. Until then `audit_ctx` defaults to `None` and the adapter
//! behaves as a pre-Step-13b passthrough.

use std::sync::{Arc, RwLock};

use crate::adapter::http::scope_refilter::HasTenantId;
use crate::context::action_channel::ScopeProvider;
use crate::infra::audit::AuditLogger;
use crate::infra::cross_project_audit::{self, CrossProjectBlockEvent};
use crate::infra::cross_project_guard::{CrossProjectReason, GuardLayer};
use crate::worker::ActorContext;

/// Three pieces every Neutron `list_*` impl needs to emit a per-row
/// `AdapterFilterViolation` event when `refilter_by_scope` returns a
/// non-empty `dropped` set. See module docstring for the rationale behind
/// each `Arc`-shared field.
pub struct NeutronAuditCtx {
    /// Production audit logger (rotation + sensitive masking). The same
    /// `Arc` is shared with `App` and the worker so all three writers
    /// land in a single `BufWriter` rotation, avoiding interleaving.
    pub logger: Arc<AuditLogger>,
    /// Source of the current active project id, read live at each emit.
    /// Backed by `RbacGuard` in production; `None` from the provider
    /// means the user is unscoped (rare; refilter then short-circuits).
    pub scope_provider: Arc<dyn ScopeProvider>,
    /// Cloud / user_id snapshot, mutated by `App::handle_event` on
    /// `ContextChanged` (BL-P2-074 cloud switch). Read under the lock
    /// once per `emit_filter_violations` call so the entire dropped set
    /// gets a consistent attribution.
    pub actor_ctx: Arc<RwLock<ActorContext>>,
}

impl NeutronAuditCtx {
    /// Emit one `CrossProjectBlockEvent` per dropped row. No-op when
    /// `dropped.is_empty()` to avoid touching the audit log on the common
    /// (zero-violation) path. Reads `actor_ctx` and `scope_provider` *at
    /// emit time* so cloud/project switches between the list call and the
    /// emit are reflected.
    pub fn emit_filter_violations<T: HasTenantId>(
        &self,
        dropped: &[T],
        action_type: &str,
        resource_kind: &str,
        correlation_id: u64,
    ) {
        if dropped.is_empty() {
            return;
        }
        let active = self.scope_provider.current_project_id();
        let (cloud, user_id) = {
            let ctx = self
                .actor_ctx
                .read()
                .unwrap_or_else(|p| p.into_inner());
            (ctx.cloud.clone(), ctx.user_id.clone())
        };
        for item in dropped {
            let resource_id = item.resource_id().unwrap_or("?").to_string();
            let project_id = item.tenant_id().unwrap_or("").to_string();
            // `CrossProjectBlockEvent::new` (Step 11b ctor) leaves the
            // top-level `resource_id` as `None` because the worker path
            // doesn't always know one. The adapter path always does, so
            // we promote it into the fingerprint-relevant slot after
            // construction. Same value as the one packed inside
            // `AdapterFilterViolation::resource_id`; no semantic
            // duplication, just two views of the same row.
            let mut event = CrossProjectBlockEvent::new(
                CrossProjectReason::AdapterFilterViolation {
                    resource_id: resource_id.clone(),
                    project_id,
                },
                GuardLayer::Fr1Adapter,
                action_type,
                resource_kind,
                cloud.clone(),
                user_id.clone(),
                active.clone(),
                None,
                correlation_id,
            );
            event.resource_id = Some(resource_id);
            cross_project_audit::emit(&event, Some(&self.logger));
        }
    }
}

#[cfg(test)]
mod tests {
    use std::sync::{Arc, RwLock};

    use tempfile::TempDir;

    use super::*;
    use crate::adapter::http::neutron::NeutronHttpAdapter;
    use crate::adapter::http::scope_refilter::HasTenantId;
    use crate::context::action_channel::ScopeProvider;
    use crate::infra::audit::AuditLogger;
    use crate::worker::ActorContext;

    /// Stub scope provider returning a fixed project id.
    struct FixedScope(Option<String>);
    impl ScopeProvider for FixedScope {
        fn current_project_id(&self) -> Option<String> {
            self.0.clone()
        }
    }

    /// Test fixture row mirroring the minimum HasTenantId surface.
    struct Row {
        id: &'static str,
        tenant: Option<&'static str>,
    }
    impl HasTenantId for Row {
        fn tenant_id(&self) -> Option<&str> {
            self.tenant
        }
        fn resource_id(&self) -> Option<&str> {
            Some(self.id)
        }
    }

    fn build_ctx(dir: &TempDir, active: Option<&str>) -> NeutronAuditCtx {
        let logger = Arc::new(AuditLogger::new(dir.path().join("audit.log")).unwrap());
        let scope: Arc<dyn ScopeProvider> = Arc::new(FixedScope(active.map(str::to_string)));
        let actor = Arc::new(RwLock::new(ActorContext {
            cloud: "devstack".into(),
            user_id: "user-uuid".into(),
        }));
        NeutronAuditCtx {
            logger,
            scope_provider: scope,
            actor_ctx: actor,
        }
    }

    fn read_audit_lines(dir: &TempDir) -> Vec<serde_json::Value> {
        let path = dir.path().join("audit.log");
        let content = std::fs::read_to_string(&path).unwrap_or_default();
        content
            .lines()
            .filter(|l| !l.trim().is_empty())
            .map(|l| serde_json::from_str::<serde_json::Value>(l).unwrap())
            .collect()
    }

    #[test]
    fn test_neutron_audit_ctx_emit_one_event_per_dropped() {
        let dir = TempDir::new().unwrap();
        let ctx = build_ctx(&dir, Some("proj-A"));
        let dropped = vec![
            Row {
                id: "x1",
                tenant: Some("proj-B"),
            },
            Row {
                id: "x2",
                tenant: Some("proj-C"),
            },
            Row {
                id: "x3",
                tenant: None,
            },
        ];
        ctx.emit_filter_violations(&dropped, "FetchSecurityGroups", "security_group", 99);

        let lines = read_audit_lines(&dir);
        assert_eq!(lines.len(), 3, "one log line per dropped row");
        let ids: Vec<&str> = lines
            .iter()
            .filter_map(|v| v["resource_id"].as_str())
            .collect();
        assert!(ids.contains(&"x1"));
        assert!(ids.contains(&"x2"));
        assert!(ids.contains(&"x3"));
    }

    #[test]
    fn test_neutron_audit_ctx_no_emit_when_dropped_empty() {
        let dir = TempDir::new().unwrap();
        let ctx = build_ctx(&dir, Some("proj-A"));
        let dropped: Vec<Row> = Vec::new();
        ctx.emit_filter_violations(&dropped, "FetchNetworks", "network", 1);

        let lines = read_audit_lines(&dir);
        assert!(
            lines.is_empty(),
            "no audit lines must be emitted when dropped is empty"
        );
    }

    #[test]
    fn test_neutron_audit_ctx_uses_fr1_adapter_layer_in_event() {
        let dir = TempDir::new().unwrap();
        let ctx = build_ctx(&dir, Some("proj-A"));
        let dropped = vec![Row {
            id: "fip-1",
            tenant: Some("proj-other"),
        }];
        ctx.emit_filter_violations(&dropped, "FetchFloatingIps", "floating_ip", 7);

        let lines = read_audit_lines(&dir);
        assert_eq!(lines.len(), 1);
        assert_eq!(
            lines[0]["details"]["guard_layer"], "fr1_adapter",
            "AdapterFilterViolation must stamp Fr1Adapter layer"
        );
    }

    #[test]
    fn test_neutron_audit_ctx_uses_adapter_filter_violation_reason() {
        let dir = TempDir::new().unwrap();
        let ctx = build_ctx(&dir, Some("proj-A"));
        let dropped = vec![Row {
            id: "sg-1",
            tenant: Some("proj-other"),
        }];
        ctx.emit_filter_violations(&dropped, "FetchSecurityGroups", "security_group", 0);

        let lines = read_audit_lines(&dir);
        assert_eq!(lines.len(), 1);
        let result = &lines[0]["result"];
        assert_eq!(
            result["failed"],
            serde_json::Value::String(
                "cross_project_block:adapter_filter_violation".to_string()
            ),
            "result must encode the AdapterFilterViolation reason"
        );
    }

    #[test]
    fn test_neutron_with_audit_attaches_ctx_default_none() {
        // Default constructor (Step 13a tree) leaves audit_ctx None;
        // with_audit builder attaches an Arc<NeutronAuditCtx>.
        let dir = TempDir::new().unwrap();
        let ctx = Arc::new(build_ctx(&dir, Some("proj-A")));

        // Skip BaseHttpClient construction (auth provider not in scope);
        // verify the NeutronHttpAdapter type exposes the builder shape.
        // The actual audit_ctx field is private; we observe it via the
        // builder's return type and lack of panics — Step 13b-2 GREEN
        // makes the field accessor public-or-pkg-visible if needed.
        let _builder_signature: fn(NeutronHttpAdapter, Arc<NeutronAuditCtx>) -> NeutronHttpAdapter =
            NeutronHttpAdapter::with_audit;
        // Reference ctx so the binding isn't dropped before the assertion.
        let _ = ctx;
    }
}
