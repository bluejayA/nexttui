//! BL-P2-085 Step 13b / Step-14-precedent-refactor-2 — adapter-side audit
//! context for cross-project filter violations.
//!
//! `AuditCtx` bundles the three pieces every adapter `list_*` impl needs
//! to emit a `CrossProjectBlockEvent` with reason `AdapterFilterViolation`
//! per row that survives the server-side `tenant_id={scope}` filter
//! (Step 12) but still lands in `dropped` from `refilter_by_scope`
//! (Step 13a):
//!
//! - `logger` — the production `AuditLogger` (shared `Arc` so adapter and
//!   worker write to the same file with a single `BufWriter`).
//! - `scope_provider` — live read of the active project id at emit time
//!   (avoids stale capture across cloud/project switches).
//! - `actor_ctx` — `Arc<RwLock<ActorContext>>` shared with the worker so
//!   `ContextChanged` updates to `cloud` / `user_id` are reflected here
//!   without re-spawning the adapter.
//!
//! `NeutronAuditCtx` / `NovaAuditCtx` / `CinderAuditCtx` are type aliases
//! preserved so callers can keep service-named imports while the struct
//! itself is service-agnostic. Step-14-precedent-refactor-3 will add the
//! `service: &'static str` discriminator and `AdapterAuditConfig` bundle.

use std::sync::{Arc, RwLock};

use crate::adapter::http::scope_refilter::{AuditEmitter, ScopedItem};
use crate::context::action_channel::ScopeProvider;
use crate::infra::audit::AuditLogger;
use crate::infra::cross_project_audit::{self, CrossProjectBlockEvent};
use crate::infra::cross_project_guard::{CrossProjectReason, GuardLayer};
use crate::worker::ActorContext;

/// Three pieces every adapter `list_*` impl needs to emit a per-row
/// `AdapterFilterViolation` event when [`refilter_by_scope`] returns a
/// non-empty `dropped` set. Service-agnostic — Step 14 adapters
/// (Nova/Cinder) reuse the same struct via type aliases.
///
/// [`refilter_by_scope`]: crate::adapter::http::scope_refilter::refilter_by_scope
pub struct AuditCtx {
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
    /// Service discriminator (`"neutron"` / `"nova"` / `"cinder"`),
    /// stamped by [`build_audit_config`]. Currently exposed via
    /// [`AuditCtx::service`] for analysis/grep workflows; future audit
    /// details enrichment may include it on the wire (would bump the
    /// fingerprint canonical from v1 — defer to that schema cycle).
    pub service: &'static str,
}

impl AuditCtx {
    /// Read the service discriminator. Equivalent to `self.service` but
    /// kept as a getter so the field can later become private without
    /// breaking call sites.
    pub fn service(&self) -> &'static str {
        self.service
    }
}

/// Service-named alias retained for callers that prefer explicit
/// service tagging (registry/main wiring, NeutronHttpAdapter::with_audit).
pub type NeutronAuditCtx = AuditCtx;
/// Step 14 placeholder — Nova adapter wiring will use this alias.
pub type NovaAuditCtx = AuditCtx;
/// Step 14 placeholder — Cinder adapter wiring will use this alias.
pub type CinderAuditCtx = AuditCtx;

/// Bundle of per-service [`AuditCtx`] instances passed to
/// [`crate::adapter::registry::AdapterRegistry::new_http`]. Replaces the
/// legacy 3-arg shape so Step 14 (Nova/Cinder) doesn't push the registry
/// signature past 5 arguments. Each field is `Option` because mock
/// registries and integration tests construct without an audit logger.
#[derive(Default)]
pub struct AdapterAuditConfig {
    pub neutron: Option<Arc<NeutronAuditCtx>>,
    pub nova: Option<Arc<NovaAuditCtx>>,
    pub cinder: Option<Arc<CinderAuditCtx>>,
}

/// Build an [`AdapterAuditConfig`] from the three pieces every service
/// audit ctx shares. Returns `Default` (all `None`) when `audit_logger`
/// is `None` so mock paths and audit-disabled environments don't pay the
/// `Arc` allocation. Each Some-arm shares the same `logger` /
/// `scope_provider` / `actor_ctx`; only the `service` discriminator
/// differs, matching the Step-14 plan to colocate per-service tagging
/// in one place rather than at every adapter call site.
pub fn build_audit_config(
    audit_logger: Option<Arc<AuditLogger>>,
    scope_provider: Arc<dyn ScopeProvider>,
    actor_ctx: Arc<RwLock<ActorContext>>,
) -> AdapterAuditConfig {
    let Some(logger) = audit_logger else {
        return AdapterAuditConfig::default();
    };
    let make = |service: &'static str| -> Arc<AuditCtx> {
        Arc::new(AuditCtx {
            logger: logger.clone(),
            scope_provider: scope_provider.clone(),
            actor_ctx: actor_ctx.clone(),
            service,
        })
    };
    AdapterAuditConfig {
        neutron: Some(make("neutron")),
        nova: Some(make("nova")),
        cinder: Some(make("cinder")),
    }
}

impl<T: ScopedItem> AuditEmitter<T> for AuditCtx {
    /// Emit one `CrossProjectBlockEvent` per dropped row. No-op when
    /// `dropped.is_empty()` to avoid touching the audit log on the common
    /// (zero-violation) path. Reads `actor_ctx` and `scope_provider` *at
    /// emit time* so cloud/project switches between the list call and the
    /// emit are reflected.
    fn emit_filter_violations(
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
            let ctx = self.actor_ctx.read().unwrap_or_else(|p| p.into_inner());
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
    use crate::adapter::http::scope_refilter::ScopedItem;
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

    /// Test fixture row mirroring the minimum ScopedItem surface.
    struct Row {
        id: &'static str,
        tenant: Option<&'static str>,
    }
    impl ScopedItem for Row {
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
            service: "neutron",
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
            serde_json::Value::String("cross_project_block:adapter_filter_violation".to_string()),
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

    // --- BL-P2-085 Step-14-precedent-refactor-cycle (refactor-3) ---
    // `AdapterAuditConfig` bundles the three per-service audit ctxs into a
    // single registry::new_http argument. `build_audit_config` is the
    // canonical constructor — it tags each ctx with its service name so
    // future audit details enrichment can discriminate per-service.

    fn dummy_actor_ctx() -> Arc<RwLock<ActorContext>> {
        Arc::new(RwLock::new(ActorContext {
            cloud: "devstack".into(),
            user_id: "user-uuid".into(),
        }))
    }

    #[test]
    fn test_adapter_audit_config_default_all_none() {
        let cfg = AdapterAuditConfig::default();
        assert!(cfg.neutron.is_none());
        assert!(cfg.nova.is_none());
        assert!(cfg.cinder.is_none());
    }

    #[test]
    fn test_build_audit_config_returns_none_when_logger_none() {
        let scope: Arc<dyn ScopeProvider> = Arc::new(FixedScope(None));
        let cfg = build_audit_config(None, scope, dummy_actor_ctx());
        assert!(cfg.neutron.is_none());
        assert!(cfg.nova.is_none());
        assert!(cfg.cinder.is_none());
    }

    #[test]
    fn test_build_audit_config_returns_three_service_tagged_ctxs_when_logger_some() {
        let dir = TempDir::new().unwrap();
        let logger = Arc::new(AuditLogger::new(dir.path().join("audit.log")).unwrap());
        let scope: Arc<dyn ScopeProvider> = Arc::new(FixedScope(Some("proj-A".into())));
        let cfg = build_audit_config(Some(logger), scope, dummy_actor_ctx());

        let neutron = cfg
            .neutron
            .expect("neutron ctx must be Some when logger is Some");
        assert_eq!(neutron.service(), "neutron");

        let nova = cfg.nova.expect("nova ctx must be Some when logger is Some");
        assert_eq!(nova.service(), "nova");

        let cinder = cfg
            .cinder
            .expect("cinder ctx must be Some when logger is Some");
        assert_eq!(cinder.service(), "cinder");
    }
}
