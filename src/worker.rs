//! Background worker: consumes Actions from the UI, calls OpenStack APIs,
//! and sends AppEvents back to the event loop for UI updates.

use std::collections::HashSet;
use std::future::Future;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex, RwLock};

use chrono::{DateTime, Utc};
use tokio::sync::mpsc;
use tokio::task::JoinHandle;
use tokio_util::sync::CancellationToken;
use tracing::Instrument;

use crate::action::{Action, DispatchedAction};
use crate::adapter::registry::AdapterRegistry;
use crate::context::{Epoch, VersionedEvent};
use crate::event::AppEvent;
use crate::infra::audit::AuditLogger;
use crate::infra::cross_project_audit::{self, CrossProjectBlockEvent};
use crate::infra::cross_project_guard::{self, CrossProjectReason, GuardDecision, GuardLayer};
use crate::infra::rbac::{ActionKind, RbacGuard};
use crate::models::glance::Image;
use crate::port::types::*;

/// Spawn an async future and forward its `AppEvent` result to `event_tx`
/// wrapped in a [`VersionedEvent`] stamped with `epoch`. If `cancel` fires
/// before the future completes the event is dropped — this is how the
/// switcher silences stale work from previous context generations.
///
/// BL-P2-031 Unit 2.
pub fn spawn_versioned<F>(
    cancel: CancellationToken,
    epoch: Epoch,
    event_tx: mpsc::UnboundedSender<VersionedEvent<AppEvent>>,
    fut: F,
) -> JoinHandle<()>
where
    F: Future<Output = AppEvent> + Send + 'static,
{
    tokio::spawn(async move {
        tokio::select! {
            _ = cancel.cancelled() => {
                // Stale generation — silently drop.
            }
            ev = fut => {
                let _ = event_tx.send(VersionedEvent::new(ev, epoch));
            }
        }
    })
}

/// Run the background worker loop.
/// Receives Actions from `action_rx`, calls the appropriate API via `registry`,
/// and sends resulting AppEvents to `event_tx`.
///
/// `audit_logger` and `actor_ctx` are FR2 (BL-P2-085) wiring: when an origin
/// guard rejects a mutation, the worker emits a structured
/// `CrossProjectBlockEvent` through the shared `AuditLogger` instance.
/// `actor_ctx` is read live at each emit so a runtime cloud-switch is
/// reflected in subsequent audit entries.
#[allow(clippy::too_many_arguments)]
#[tracing::instrument(skip_all)]
pub async fn run_worker(
    registry: Arc<AdapterRegistry>,
    rbac: Arc<RbacGuard>,
    all_tenants: Arc<AtomicBool>,
    mut action_rx: mpsc::UnboundedReceiver<VersionedEvent<DispatchedAction>>,
    event_tx: mpsc::UnboundedSender<VersionedEvent<AppEvent>>,
    audit_logger: Option<Arc<AuditLogger>>,
    actor_ctx: Arc<RwLock<ActorContext>>,
) {
    let polling_servers: Arc<Mutex<HashSet<String>>> = Arc::new(Mutex::new(HashSet::new()));
    let in_flight_fetches: Arc<Mutex<HashSet<String>>> = Arc::new(Mutex::new(HashSet::new()));

    while let Some(envelope) = action_rx.recv().await {
        let (dispatched, action_epoch) = envelope.into_parts();

        // BL-P2-085 Step 11a/b/c: gate mutations against the live active
        // scope, emit a structured audit entry, and surface a UI toast via
        // `AppEvent::CrossProjectBlocked`. Audit is emitted first so it lands
        // even when the receiver has been dropped; the UI event follows on
        // the same epoch so any concurrent `ApiError` for this dispatch is
        // preceded by the block notification.
        if let GuardDecision::Block { reason } = check_dispatched_origin(&dispatched, &rbac) {
            let block_event = make_cross_project_blocked_event(&reason, &dispatched.action);
            emit_origin_block_audit(
                reason,
                &dispatched,
                &rbac,
                audit_logger.as_deref(),
                &actor_ctx,
                action_epoch,
            );
            let _ = event_tx.send(VersionedEvent::new(block_event, action_epoch));
            continue;
        }

        let action = dispatched.action;

        // RBAC guard: check CUD permissions before API call
        if let Some(kind) = action_to_kind(&action)
            && !rbac.can_perform(kind)
        {
            let _ = event_tx.send(VersionedEvent::new(
                AppEvent::PermissionDenied {
                    operation: action_name(&action).to_string(),
                },
                action_epoch,
            ));
            continue;
        }

        // FetchDedup: skip if same fetch is already in-flight
        let dedup_key = fetch_dedup_key(&action);
        if let Some(key) = dedup_key
            && !in_flight_fetches
                .lock()
                .unwrap_or_else(|e| e.into_inner())
                .insert(key.to_string())
        {
            continue;
        }

        let registry = registry.clone();
        let event_tx = event_tx.clone();
        let all_tenants = all_tenants.clone();
        let polling_servers = polling_servers.clone();
        let in_flight_fetches = in_flight_fetches.clone();
        // Step 16 (FR4 Phase 9): the spawned task needs audit_logger /
        // actor_ctx so a pre-mutation form-block can emit its
        // `AdapterFilterViolation` / `FormSelectionMismatch` audit
        // entry without re-resolving them from globals.
        let audit_logger_task = audit_logger.clone();
        let actor_ctx_task = actor_ctx.clone();

        // BL-P2-085 Step 12: snapshot active project once per dispatch and
        // pass it to handle_action so Neutron list builders inject
        // `tenant_id={scope}` when `all_tenants=false`.
        let active_tenant = rbac.project_id();

        let poll_migration_id = poll_migration_server_id(&action);
        let poll_status_id = poll_server_id_for_status(&action);

        let span = tracing::info_span!("worker_task", action = action_name(&action));
        tokio::spawn(
            async move {
                let event = handle_action(
                    &registry,
                    &all_tenants,
                    active_tenant,
                    action,
                    audit_logger_task.as_deref(),
                    &actor_ctx_task,
                )
                .await;
                let success = event
                    .as_ref()
                    .is_some_and(|ev| !matches!(ev, AppEvent::ApiError { .. }));
                if let Some(ev) = event {
                    let _ = event_tx.send(VersionedEvent::new(ev, action_epoch));
                }
                // Release fetch dedup guard
                if let Some(key) = dedup_key {
                    in_flight_fetches
                        .lock()
                        .unwrap_or_else(|e| e.into_inner())
                        .remove(key);
                }
                if success {
                    if let Some(ref server_id) = poll_migration_id
                        && polling_servers
                            .lock()
                            .unwrap_or_else(|e| e.into_inner())
                            .insert(server_id.clone())
                    {
                        poll_migration_progress(&registry, &event_tx, action_epoch, server_id)
                            .await;
                        polling_servers
                            .lock()
                            .unwrap_or_else(|e| e.into_inner())
                            .remove(server_id);
                    }
                    if let Some(ref server_id) = poll_status_id
                        && polling_servers
                            .lock()
                            .unwrap_or_else(|e| e.into_inner())
                            .insert(server_id.clone())
                    {
                        poll_server_status(&registry, &event_tx, action_epoch, server_id).await;
                        polling_servers
                            .lock()
                            .unwrap_or_else(|e| e.into_inner())
                            .remove(server_id);
                    }
                }
            }
            .instrument(span),
        );
    }
}

/// Map an Action to its RBAC ActionKind for permission checking.
/// Returns None for read-only/UI actions that need no guard.
/// Map an [`Action`] to its RBAC [`ActionKind`]. Returns `None` for read-only,
/// UI, system, and orchestration actions that do not pass through RBAC gating.
///
/// Exhaustive match — adding a new `Action` variant breaks compilation here
/// (BL-P2-085 Step 7), forcing a deliberate classification decision.
pub(crate) fn action_to_kind(action: &Action) -> Option<ActionKind> {
    match action {
        // Create (member-level)
        Action::CreateServer(_)
        | Action::CreateFlavor(_)
        | Action::CreateNetwork(_)
        | Action::CreateSecurityGroup(_)
        | Action::CreateSecurityGroupRule(_)
        | Action::CreateFloatingIp { .. }
        | Action::CreateVolume(_)
        | Action::CreateSnapshot(_)
        | Action::CreateImage(_)
        | Action::CreateServerSnapshot { .. } => Some(ActionKind::Create),

        // Create (admin-only: identity resources)
        Action::CreateProject(_) | Action::CreateUser(_) => Some(ActionKind::ManageQuota),

        // Delete (member-level, non-force)
        Action::DeleteServer { .. }
        | Action::DeleteFlavor { .. }
        | Action::DeleteSecurityGroup { .. }
        | Action::DeleteSecurityGroupRule { .. }
        | Action::DeleteFloatingIp { .. }
        | Action::DeleteSnapshot { .. }
        | Action::DeleteImage { .. } => Some(ActionKind::Delete),

        // Delete (admin-only: identity resources)
        Action::DeleteProject { .. } | Action::DeleteUser { .. } => Some(ActionKind::ManageQuota),

        // Force delete
        Action::DeleteVolume { force: true, .. } => Some(ActionKind::ForceDelete),
        Action::DeleteVolume { force: false, .. } => Some(ActionKind::Delete),

        // Resize (member-level)
        Action::ResizeServer { .. }
        | Action::ConfirmResize { .. }
        | Action::RevertResize { .. } => Some(ActionKind::Resize),

        // Migration / Evacuate (admin-only)
        Action::LiveMigrateServer { .. }
        | Action::ColdMigrateServer { .. }
        | Action::ConfirmMigration { .. }
        | Action::RevertMigration { .. } => Some(ActionKind::Migrate),

        Action::EvacuateServer { .. } => Some(ActionKind::Evacuate),

        Action::DisableComputeService { .. } | Action::EnableComputeService { .. } => {
            Some(ActionKind::EnableDisable)
        }

        // Server lifecycle — treated as CUD for RBAC purposes
        Action::RebootServer { .. } | Action::StartServer { .. } | Action::StopServer { .. } => {
            Some(ActionKind::Create)
        }

        // Volume extend
        Action::ExtendVolume { .. } => Some(ActionKind::Create),

        // Attach / Associate (member-level)
        Action::AttachVolume { .. } | Action::AssociateFloatingIp { .. } => {
            Some(ActionKind::Attach)
        }

        // Detach / Disassociate (member-level)
        Action::DetachVolume { .. } | Action::DisassociateFloatingIp { .. } => {
            Some(ActionKind::Detach)
        }

        // Force operations (admin-only)
        Action::ForceDetachVolume { .. } | Action::ForceResetVolumeState { .. } => {
            Some(ActionKind::ForceDelete)
        }

        // --- Explicit None: not RBAC-gated mutations ---
        // Read-only fetches
        Action::FetchServers
        | Action::FetchFlavors
        | Action::FetchAggregates
        | Action::FetchComputeServices
        | Action::FetchHypervisors
        | Action::FetchNetworks
        | Action::FetchSecurityGroups
        | Action::FetchFloatingIps
        | Action::FetchSubnets { .. }
        | Action::FetchAgents
        | Action::FetchVolumes
        | Action::FetchSnapshots
        | Action::FetchImages
        | Action::FetchProjects
        | Action::FetchUsers
        | Action::FetchUsage { .. }
        | Action::FetchMigrationProgress { .. }
        | Action::FetchPorts { .. }
        | Action::FetchPortBindingsForServer { .. } => None,

        // Navigation / UI helpers
        Action::Navigate(_)
        | Action::Back
        | Action::FocusSidebar
        | Action::EnterFormMode
        | Action::ExitFormMode
        | Action::SelectResource { .. }
        | Action::NavigateToResource { .. }
        | Action::ShowToast { .. } => None,

        // System / global state
        Action::RefreshAll | Action::Quit | Action::ToggleAllTenants => None,

        // Context switch — orchestration, not RBAC mutation
        Action::SwitchContext(_) | Action::SwitchBack => None,
    }
}

/// Wrapper over [`action_to_kind`]: true if the action is an RBAC-gated
/// mutation. Wired up by [`crate::context::ActionSender`] in BL-P2-085 Phase 6
/// Wired by `ActionSender::send` (Step 9) to decide whether to stamp
/// `origin_project_id` on the outgoing `DispatchedAction`.
pub(crate) fn action_is_mutation(action: &Action) -> bool {
    action_to_kind(action).is_some()
}

/// FR2 (BL-P2-085 Step 11a): compare a dispatched action's `origin_project_id`
/// against the live active scope on `RbacGuard`.
///
/// Read-only (unstamped) actions return [`GuardDecision::Allow`]. Stamped
/// actions defer to [`cross_project_guard::check_origin_scope`], which
/// fail-safe blocks empty/unscoped values. Sync — callable in unit tests
/// without spawning the worker loop. Step 11b wires `AuditLogger::emit`
/// on `Block`; Step 11c will add toast emission.
pub(crate) fn check_dispatched_origin(
    dispatched: &DispatchedAction,
    rbac: &RbacGuard,
) -> GuardDecision {
    match &dispatched.origin_project_id {
        Some(origin) => {
            let active = rbac.project_id().unwrap_or_default();
            cross_project_guard::check_origin_scope(origin, &active)
        }
        None => GuardDecision::Allow,
    }
}

/// FR2 (BL-P2-085 Phase 7 폴리싱): live actor identity used by the worker when
/// emitting `CrossProjectBlockEvent`s. Wrapped in `Arc<RwLock<...>>` so a
/// runtime cloud-switch (BL-P2-074) can update the active cloud without
/// re-spawning the worker. Without this, audit entries stay anchored to the
/// startup cloud forever.
///
/// `user_id` falls back to `"unknown"` (matches `App::build_audit_entry` line
/// 814) when the configured credential lacks an explicit username — empty
/// strings would silently break audit attribution.
#[derive(Debug, Clone)]
pub struct ActorContext {
    pub cloud: String,
    pub user_id: String,
}

/// FR2 (BL-P2-085 Step 11c): build the user-facing `AppEvent::CrossProjectBlocked`
/// payload from a guard reason and the offending action. The String fields
/// keep the variant decoupled from `cross_project_guard` so the UI layer
/// doesn't need to import the guard module.
pub(crate) fn make_cross_project_blocked_event(
    reason: &CrossProjectReason,
    action: &Action,
) -> AppEvent {
    AppEvent::CrossProjectBlocked {
        reason: reason.as_str().to_string(),
        action: action_name(action).to_string(),
    }
}

/// FR2 (BL-P2-085 Step 11b, 폴리싱): build a [`CrossProjectBlockEvent`] from a
/// worker origin-mismatch decision and emit it via
/// [`cross_project_audit::emit`].
///
/// Reads `actor_ctx` at call time so a cloud-switch landing between worker
/// spawn and the next block is reflected in the audit entry.
///
/// Best-effort: when `audit_logger` is `None`, `emit` falls back to
/// `tracing::warn!` so the block still surfaces in process logs. Sync —
/// callable in unit tests without spawning the worker loop.
///
/// `resource_kind` / `resource_id` / `target_project_id` are left blank/None
/// for now; a follow-up enrichment pass will populate them per-action so
/// audit consumers can slice on `details.resource_kind`. Until then they
/// grep on `action_type` + `details.guard_layer`.
pub(crate) fn emit_origin_block_audit(
    reason: CrossProjectReason,
    dispatched: &DispatchedAction,
    rbac: &RbacGuard,
    audit_logger: Option<&AuditLogger>,
    actor_ctx: &Arc<RwLock<ActorContext>>,
    correlation_id: u64,
) {
    let actor = actor_ctx
        .read()
        .map(|guard| (guard.cloud.clone(), guard.user_id.clone()))
        .unwrap_or_else(|e| {
            let guard = e.into_inner();
            (guard.cloud.clone(), guard.user_id.clone())
        });
    let event = CrossProjectBlockEvent::new(
        reason,
        GuardLayer::Fr2Worker,
        action_name(&dispatched.action),
        "", // resource_kind: enriched in follow-up pass
        actor.0,
        actor.1,
        rbac.project_id(),
        dispatched.origin_project_id.clone(),
        correlation_id,
    );
    cross_project_audit::emit(&event, audit_logger);
}

/// FR4 (BL-P2-085 Step 16, Phase 9): pure decision over a refetched
/// Glance image. `image.owner` carries the project id (Glance schema);
/// `None` is treated as fail-safe deny so an upstream that omits owner
/// cannot route a cross-project delete past the form layer.
pub(crate) fn check_image_owner_scope(image: &Image, active: &str) -> GuardDecision {
    // Parity with `RefilterScope::strict` / `validate_form_scope` — an
    // empty active combined with `owner == Some("")` would silent-allow
    // a cross-project mutation. Caught in dev builds; production
    // callers must filter `Some("")` to `None` before reaching here
    // (DeleteImage branch wraps with `active_tenant.as_deref()` and
    // `RefilterScope::from_parts` normalization handles the same case
    // for the adapter list path).
    debug_assert!(
        !active.is_empty(),
        "check_image_owner_scope requires a non-empty active project id — empty would silent-allow Some(\"\")"
    );
    match image.owner.as_deref() {
        Some(owner) if owner == active => GuardDecision::Allow,
        Some(owner) => GuardDecision::Block {
            reason: CrossProjectReason::FormSelectionMismatch {
                selected: owner.to_string(),
                active: active.to_string(),
            },
        },
        None => GuardDecision::Block {
            reason: CrossProjectReason::FormSelectionMismatch {
                selected: String::new(),
                active: active.to_string(),
            },
        },
    }
}

/// FR4 (BL-P2-085 Step 16, Phase 9): build a [`CrossProjectBlockEvent`]
/// for a form-layer block (`GuardLayer::Fr4Form`) and emit via
/// [`cross_project_audit::emit`]. Mirrors [`emit_origin_block_audit`]
/// for the FR2 path; the worker uses this when pre-mutation GET shows
/// the selected resource lives in another project.
///
/// `resource_id` is stamped on the top-level event so the audit
/// fingerprint stays unique per row; `asserted_origin_project_id` is
/// `None` because FR4 has no origin to compare against.
#[allow(clippy::too_many_arguments)]
pub(crate) fn emit_form_block_audit(
    reason: CrossProjectReason,
    action_type: &str,
    resource_kind: &str,
    resource_id: &str,
    active_project_id: Option<String>,
    audit_logger: Option<&AuditLogger>,
    actor_ctx: &Arc<RwLock<ActorContext>>,
    correlation_id: u64,
) {
    let (cloud, user_id) = actor_ctx
        .read()
        .map(|g| (g.cloud.clone(), g.user_id.clone()))
        .unwrap_or_else(|e| {
            let g = e.into_inner();
            (g.cloud.clone(), g.user_id.clone())
        });
    let mut event = CrossProjectBlockEvent::new(
        reason,
        GuardLayer::Fr4Form,
        action_type,
        resource_kind,
        cloud,
        user_id,
        active_project_id,
        None, // FR4: no origin to assert
        correlation_id,
    );
    event.resource_id = Some(resource_id.to_string());
    cross_project_audit::emit(&event, audit_logger);
}

/// Human-readable name for an Action, used in PermissionDenied messages.
fn action_name(action: &Action) -> &str {
    match action {
        Action::CreateServer(_) => "CreateServer",
        Action::DeleteServer { .. } => "DeleteServer",
        Action::RebootServer { .. } => "RebootServer",
        Action::StartServer { .. } => "StartServer",
        Action::StopServer { .. } => "StopServer",
        Action::CreateServerSnapshot { .. } => "CreateServerSnapshot",
        Action::CreateFlavor(_) => "CreateFlavor",
        Action::DeleteFlavor { .. } => "DeleteFlavor",
        Action::CreateNetwork(_) => "CreateNetwork",
        Action::CreateSecurityGroup(_) => "CreateSecurityGroup",
        Action::DeleteSecurityGroup { .. } => "DeleteSecurityGroup",
        Action::CreateSecurityGroupRule(_) => "CreateSecurityGroupRule",
        Action::DeleteSecurityGroupRule { .. } => "DeleteSecurityGroupRule",
        Action::CreateFloatingIp { .. } => "CreateFloatingIp",
        Action::DeleteFloatingIp { .. } => "DeleteFloatingIp",
        Action::CreateVolume(_) => "CreateVolume",
        Action::DeleteVolume { .. } => "DeleteVolume",
        Action::ExtendVolume { .. } => "ExtendVolume",
        Action::CreateSnapshot(_) => "CreateSnapshot",
        Action::DeleteSnapshot { .. } => "DeleteSnapshot",
        Action::CreateImage(_) => "CreateImage",
        Action::DeleteImage { .. } => "DeleteImage",
        Action::CreateProject(_) => "CreateProject",
        Action::DeleteProject { .. } => "DeleteProject",
        Action::CreateUser(_) => "CreateUser",
        Action::DeleteUser { .. } => "DeleteUser",
        Action::ResizeServer { .. } => "ResizeServer",
        Action::ConfirmResize { .. } => "ConfirmResize",
        Action::RevertResize { .. } => "RevertResize",
        Action::LiveMigrateServer { .. } => "LiveMigrateServer",
        Action::ColdMigrateServer { .. } => "ColdMigrateServer",
        Action::ConfirmMigration { .. } => "ConfirmMigration",
        Action::RevertMigration { .. } => "RevertMigration",
        Action::EvacuateServer { .. } => "EvacuateServer",
        Action::DisableComputeService { .. } => "DisableComputeService",
        Action::EnableComputeService { .. } => "EnableComputeService",
        Action::FetchMigrationProgress { .. } => "FetchMigrationProgress",
        Action::AttachVolume { .. } => "AttachVolume",
        Action::DetachVolume { .. } => "DetachVolume",
        Action::ForceDetachVolume { .. } => "ForceDetachVolume",
        Action::ForceResetVolumeState { .. } => "ForceResetVolumeState",
        Action::AssociateFloatingIp { .. } => "AssociateFloatingIp",
        Action::DisassociateFloatingIp { .. } => "DisassociateFloatingIp",
        Action::FetchPorts { .. } => "FetchPorts",
        Action::FetchPortBindingsForServer { .. } => "FetchPortBindingsForServer",
        Action::FetchUsage { .. } => "FetchUsage",
        _ => "Unknown",
    }
}

async fn handle_action(
    registry: &AdapterRegistry,
    all_tenants: &AtomicBool,
    active_tenant: Option<String>,
    action: Action,
    audit_logger: Option<&AuditLogger>,
    actor_ctx: &Arc<RwLock<ActorContext>>,
) -> Option<AppEvent> {
    let action_label = action_name(&action);
    tracing::info!(action = action_label, "handling action");
    let default_pagination = PaginationParams::default();
    let at = all_tenants.load(Ordering::Relaxed);
    // BL-P2-085 Step 12: when admin explicitly toggled `all_tenants` we omit
    // tenant_id; otherwise inject the live active scope so server-side scoping
    // matches the worker mutation guard's view of the world.
    let scoped_tenant_id = if at { None } else { active_tenant.clone() };

    match action {
        // -- Nova: Servers --------------------------------------------------
        Action::FetchServers => {
            match registry
                .nova
                .list_servers(
                    &ServerListFilter {
                        all_tenants: at,
                        ..Default::default()
                    },
                    &default_pagination,
                )
                .await
            {
                Ok(resp) => Some(AppEvent::ServersLoaded(resp.items)),
                Err(e) => Some(api_error("FetchServers", e)),
            }
        }
        Action::CreateServer(params) => match registry.nova.create_server(&params).await {
            Ok(server) => Some(AppEvent::ServerCreated(server)),
            Err(e) => Some(api_error("CreateServer", e)),
        },
        Action::DeleteServer { id, name } => match registry.nova.delete_server(&id).await {
            Ok(()) => Some(AppEvent::ServerDeleted { id, name }),
            Err(e) => Some(api_error("DeleteServer", e)),
        },
        Action::RebootServer { id, hard } => {
            let reboot_type = if hard {
                RebootType::Hard
            } else {
                RebootType::Soft
            };
            match registry.nova.reboot_server(&id, reboot_type).await {
                Ok(()) => Some(AppEvent::ServerRebooted { id }),
                Err(e) => Some(api_error("RebootServer", e)),
            }
        }
        Action::StartServer { id } => match registry.nova.start_server(&id).await {
            Ok(()) => Some(AppEvent::ServerStarted { id }),
            Err(e) => Some(api_error("StartServer", e)),
        },
        Action::StopServer { id } => match registry.nova.stop_server(&id).await {
            Ok(()) => Some(AppEvent::ServerStopped { id }),
            Err(e) => Some(api_error("StopServer", e)),
        },
        Action::CreateServerSnapshot { server_id, name } => {
            match registry
                .nova
                .create_server_snapshot(&server_id, &name)
                .await
            {
                Ok(image_id) => Some(AppEvent::ServerSnapshotCreated {
                    server_id,
                    image_id,
                }),
                Err(e) => Some(api_error("CreateServerSnapshot", e)),
            }
        }

        // -- Nova: Resize --------------------------------------------------
        Action::ResizeServer { id, flavor_id } => {
            match registry.nova.resize_server(&id, &flavor_id).await {
                Ok(()) => Some(AppEvent::ServerResized { id }),
                Err(e) => Some(api_error("ResizeServer", e)),
            }
        }
        Action::ConfirmResize { id } => match registry.nova.confirm_migration(&id).await {
            Ok(()) => Some(AppEvent::ResizeConfirmed { id }),
            Err(e) => Some(api_error("ConfirmResize", e)),
        },
        Action::RevertResize { id } => match registry.nova.revert_migration(&id).await {
            Ok(()) => Some(AppEvent::ResizeReverted { id }),
            Err(e) => Some(api_error("RevertResize", e)),
        },

        // -- Nova: Migration / Evacuate ------------------------------------
        Action::LiveMigrateServer { id, host } => {
            let params = LiveMigrateParams { host };
            match registry.nova.live_migrate_server(&id, &params).await {
                Ok(()) => Some(AppEvent::ServerLiveMigrated { id }),
                Err(e) => Some(api_error("LiveMigrateServer", enrich_live_migrate_error(e))),
            }
        }
        Action::ColdMigrateServer { id } => match registry.nova.cold_migrate_server(&id).await {
            Ok(()) => Some(AppEvent::ServerColdMigrated { id }),
            Err(e) => Some(api_error("ColdMigrateServer", e)),
        },
        Action::ConfirmMigration { id } => match registry.nova.confirm_migration(&id).await {
            Ok(()) => Some(AppEvent::MigrationConfirmed { id }),
            Err(e) => Some(api_error("ConfirmMigration", e)),
        },
        Action::RevertMigration { id } => match registry.nova.revert_migration(&id).await {
            Ok(()) => Some(AppEvent::MigrationReverted { id }),
            Err(e) => Some(api_error("RevertMigration", e)),
        },
        Action::EvacuateServer { id, params } => {
            match registry.nova.evacuate_server(&id, &params).await {
                Ok(()) => Some(AppEvent::ServerEvacuateResult { id, result: Ok(()) }),
                Err(e) => Some(AppEvent::ServerEvacuateResult {
                    id,
                    result: Err(e.to_string()),
                }),
            }
        }
        Action::DisableComputeService {
            service_id,
            hostname,
        } => {
            match registry
                .nova
                .disable_compute_service(&service_id, None)
                .await
            {
                Ok(_) => Some(AppEvent::ComputeServiceToggled {
                    hostname,
                    enabled: false,
                }),
                Err(e) => Some(api_error("DisableComputeService", e)),
            }
        }
        Action::EnableComputeService {
            service_id,
            hostname,
        } => match registry.nova.enable_compute_service(&service_id).await {
            Ok(_) => Some(AppEvent::ComputeServiceToggled {
                hostname,
                enabled: true,
            }),
            Err(e) => Some(api_error("EnableComputeService", e)),
        },
        Action::FetchMigrationProgress { server_id } => {
            match registry.nova.list_server_migrations(&server_id).await {
                Ok(migrations) => migrations.into_iter().last().map(|migration| {
                    AppEvent::MigrationProgressLoaded {
                        server_id,
                        migration,
                    }
                }),
                Err(e) => Some(api_error("FetchMigrationProgress", e)),
            }
        }

        // -- Nova: Usage ---------------------------------------------------
        Action::FetchUsage { start, end } => {
            use crate::port::error::ApiError;
            let start_dt = start
                .parse::<DateTime<Utc>>()
                .map_err(|e| ApiError::Parse(e.to_string()));
            let end_dt = end
                .parse::<DateTime<Utc>>()
                .map_err(|e| ApiError::Parse(e.to_string()));
            match (start_dt, end_dt) {
                (Ok(s), Ok(e)) => match registry.nova.list_all_tenant_usage(s, e).await {
                    Ok(usages) => Some(AppEvent::UsageLoaded(usages)),
                    Err(e) => Some(api_error("FetchUsage", e)),
                },
                (Err(e), _) | (_, Err(e)) => Some(api_error("FetchUsage", e)),
            }
        }

        // -- Nova: Flavors --------------------------------------------------
        Action::FetchFlavors => match registry.nova.list_flavors(&default_pagination).await {
            Ok(resp) => Some(AppEvent::FlavorsLoaded(resp.items)),
            Err(e) => Some(api_error("FetchFlavors", e)),
        },
        Action::CreateFlavor(params) => match registry.nova.create_flavor(&params).await {
            Ok(flavor) => Some(AppEvent::FlavorCreated(flavor)),
            Err(e) => Some(api_error("CreateFlavor", e)),
        },
        Action::DeleteFlavor { id } => match registry.nova.delete_flavor(&id).await {
            Ok(()) => Some(AppEvent::FlavorDeleted { id }),
            Err(e) => Some(api_error("DeleteFlavor", e)),
        },

        // -- Nova: Admin ----------------------------------------------------
        Action::FetchAggregates => match registry.nova.list_aggregates().await {
            Ok(aggs) => Some(AppEvent::AggregatesLoaded(aggs)),
            Err(e) => Some(api_error("FetchAggregates", e)),
        },
        Action::FetchComputeServices => match registry.nova.list_compute_services().await {
            Ok(svcs) => Some(AppEvent::ComputeServicesLoaded(svcs)),
            Err(e) => Some(api_error("FetchComputeServices", e)),
        },
        Action::FetchHypervisors => match registry.nova.list_hypervisors().await {
            Ok(hvs) => Some(AppEvent::HypervisorsLoaded(hvs)),
            Err(e) => Some(api_error("FetchHypervisors", e)),
        },

        // -- Neutron: Networks ----------------------------------------------
        Action::FetchNetworks => {
            match registry
                .neutron
                .list_networks(
                    &NetworkListFilter {
                        all_tenants: at,
                        tenant_id: scoped_tenant_id.clone(),
                    },
                    &default_pagination,
                )
                .await
            {
                Ok(resp) => Some(AppEvent::NetworksLoaded(resp.items)),
                Err(e) => Some(api_error("FetchNetworks", e)),
            }
        }
        Action::CreateNetwork(params) => match registry.neutron.create_network(&params).await {
            Ok(net) => Some(AppEvent::NetworkCreated(net)),
            Err(e) => Some(api_error("CreateNetwork", e)),
        },
        Action::FetchSubnets { network_id } => {
            match registry.neutron.list_subnets(Some(&network_id)).await {
                Ok(subnets) => Some(AppEvent::SubnetsLoaded {
                    network_id,
                    subnets,
                }),
                Err(e) => Some(api_error("FetchSubnets", e)),
            }
        }

        // -- Neutron: Security Groups ---------------------------------------
        Action::FetchSecurityGroups => {
            match registry
                .neutron
                .list_security_groups(
                    &SecurityGroupListFilter {
                        all_tenants: at,
                        tenant_id: scoped_tenant_id.clone(),
                    },
                    &default_pagination,
                )
                .await
            {
                Ok(resp) => Some(AppEvent::SecurityGroupsLoaded(resp.items)),
                Err(e) => Some(api_error("FetchSecurityGroups", e)),
            }
        }
        Action::CreateSecurityGroup(params) => {
            match registry.neutron.create_security_group(&params).await {
                Ok(sg) => Some(AppEvent::SecurityGroupCreated(sg)),
                Err(e) => Some(api_error("CreateSecurityGroup", e)),
            }
        }
        Action::DeleteSecurityGroup { id } => {
            match registry.neutron.delete_security_group(&id).await {
                Ok(()) => Some(AppEvent::SecurityGroupDeleted { id }),
                Err(e) => Some(api_error("DeleteSecurityGroup", e)),
            }
        }
        Action::CreateSecurityGroupRule(params) => {
            match registry.neutron.create_security_group_rule(&params).await {
                Ok(rule) => Some(AppEvent::SecurityGroupRuleCreated(rule)),
                Err(e) => Some(api_error("CreateSecurityGroupRule", e)),
            }
        }
        Action::DeleteSecurityGroupRule { rule_id } => {
            match registry.neutron.delete_security_group_rule(&rule_id).await {
                Ok(()) => Some(AppEvent::SecurityGroupRuleDeleted { rule_id }),
                Err(e) => Some(api_error("DeleteSecurityGroupRule", e)),
            }
        }

        // -- Neutron: Floating IPs ------------------------------------------
        Action::FetchFloatingIps => {
            match registry
                .neutron
                .list_floating_ips(
                    &FloatingIpListFilter {
                        all_tenants: at,
                        tenant_id: scoped_tenant_id.clone(),
                    },
                    &default_pagination,
                )
                .await
            {
                Ok(resp) => Some(AppEvent::FloatingIpsLoaded(resp.items)),
                Err(e) => Some(api_error("FetchFloatingIps", e)),
            }
        }
        Action::CreateFloatingIp { network_id } => {
            match registry
                .neutron
                .create_floating_ip(&FloatingIpCreateParams {
                    floating_network_id: network_id,
                    port_id: None,
                    fixed_ip_address: None,
                })
                .await
            {
                Ok(fip) => Some(AppEvent::FloatingIpCreated(fip)),
                Err(e) => Some(api_error("CreateFloatingIp", e)),
            }
        }
        Action::DeleteFloatingIp { id } => match registry.neutron.delete_floating_ip(&id).await {
            Ok(()) => Some(AppEvent::FloatingIpDeleted { id }),
            Err(e) => Some(api_error("DeleteFloatingIp", e)),
        },
        Action::FetchAgents => match registry.neutron.list_network_agents().await {
            Ok(agents) => Some(AppEvent::AgentsLoaded(agents)),
            Err(e) => Some(api_error("FetchAgents", e)),
        },

        // -- Cinder: Volumes ------------------------------------------------
        Action::FetchVolumes => {
            match registry
                .cinder
                .list_volumes(
                    &VolumeListFilter {
                        all_tenants: at,
                        ..Default::default()
                    },
                    &default_pagination,
                )
                .await
            {
                Ok(resp) => Some(AppEvent::VolumesLoaded(resp.items)),
                Err(e) => Some(api_error("FetchVolumes", e)),
            }
        }
        Action::CreateVolume(params) => match registry.cinder.create_volume(&params).await {
            Ok(vol) => Some(AppEvent::VolumeCreated(vol)),
            Err(e) => Some(api_error("CreateVolume", e)),
        },
        Action::DeleteVolume { id, force } => {
            let result = if force {
                registry.cinder.force_delete_volume(&id).await
            } else {
                registry.cinder.delete_volume(&id).await
            };
            match result {
                Ok(()) => Some(AppEvent::VolumeDeleted { id }),
                Err(e) => Some(api_error("DeleteVolume", e)),
            }
        }
        Action::ExtendVolume { id, new_size } => {
            match registry.cinder.extend_volume(&id, new_size).await {
                Ok(()) => Some(AppEvent::VolumeExtended { id }),
                Err(e) => Some(api_error("ExtendVolume", e)),
            }
        }

        // -- Cinder: Snapshots ----------------------------------------------
        Action::FetchSnapshots => {
            match registry
                .cinder
                .list_snapshots(&SnapshotListFilter { all_tenants: at }, &default_pagination)
                .await
            {
                Ok(resp) => Some(AppEvent::SnapshotsLoaded(resp.items)),
                Err(e) => Some(api_error("FetchSnapshots", e)),
            }
        }
        Action::CreateSnapshot(params) => match registry.cinder.create_snapshot(&params).await {
            Ok(snap) => Some(AppEvent::SnapshotCreated(snap)),
            Err(e) => Some(api_error("CreateSnapshot", e)),
        },
        Action::DeleteSnapshot { id } => match registry.cinder.delete_snapshot(&id).await {
            Ok(()) => Some(AppEvent::SnapshotDeleted { id }),
            Err(e) => Some(api_error("DeleteSnapshot", e)),
        },

        // -- Glance: Images -------------------------------------------------
        Action::FetchImages => {
            match registry
                .glance
                .list_images(
                    &ImageListFilter {
                        all_tenants: at,
                        ..Default::default()
                    },
                    &default_pagination,
                )
                .await
            {
                Ok(resp) => Some(AppEvent::ImagesLoaded(resp.items)),
                Err(e) => Some(api_error("FetchImages", e)),
            }
        }
        Action::CreateImage(params) => match registry.glance.create_image(&params).await {
            Ok(img) => Some(AppEvent::ImageCreated(img)),
            Err(e) => Some(api_error("CreateImage", e)),
        },
        Action::DeleteImage { id } => {
            // FR4 (BL-P2-085 Step 16, Phase 9): pre-mutation owner check.
            // Refetch the image to read `owner` live — a list snapshot in
            // the UI can be stale by the time the user confirms delete.
            //
            // Unscoped session: FR2 does NOT block unstamped envelopes
            // (check_dispatched_origin returns Allow when origin=None),
            // and this branch also skips FR4 because `active_tenant` is
            // None. RBAC `can_perform` is the only check; project-scope
            // verification is intentionally absent because the user has
            // not yet authenticated to a project. The pre-auth gate is
            // the App-layer router's responsibility, not FR4's.
            //
            // Pre-GET failure: silent passthrough — the subsequent
            // `delete_image` call surfaces the underlying error via
            // `api_error`. Trade-off: a 401/403 on GET (e.g. token
            // expired) could in principle route past FR4 to delete; a
            // future cycle (BL follow-up) can fail-closed for 4xx-auth
            // / 5xx while keeping 404 as the existing pass-through.
            if let Some(active) = active_tenant.as_deref()
                && let Ok(image) = registry.glance.get_image(&id).await
                && let GuardDecision::Block { reason } = check_image_owner_scope(&image, active)
            {
                emit_form_block_audit(
                    reason.clone(),
                    "DeleteImage",
                    "image",
                    &id,
                    Some(active.to_string()),
                    audit_logger,
                    actor_ctx,
                    0, // correlation_id: form layer not bound to a dispatch epoch
                );
                return Some(AppEvent::CrossProjectBlocked {
                    reason: reason.as_str().to_string(),
                    action: "DeleteImage".to_string(),
                });
            }
            match registry.glance.delete_image(&id).await {
                Ok(()) => Some(AppEvent::ImageDeleted { id }),
                Err(e) => Some(api_error("DeleteImage", e)),
            }
        }

        // -- Keystone: Projects ---------------------------------------------
        Action::FetchProjects => match registry.keystone.list_projects(&default_pagination).await {
            Ok(resp) => Some(AppEvent::ProjectsLoaded(resp.items)),
            Err(e) => Some(api_error("FetchProjects", e)),
        },
        Action::CreateProject(params) => match registry.keystone.create_project(&params).await {
            Ok(proj) => Some(AppEvent::ProjectCreated(proj)),
            Err(e) => Some(api_error("CreateProject", e)),
        },
        Action::DeleteProject { id } => match registry.keystone.delete_project(&id).await {
            Ok(()) => Some(AppEvent::ProjectDeleted { id }),
            Err(e) => Some(api_error("DeleteProject", e)),
        },

        // -- Keystone: Users ------------------------------------------------
        Action::FetchUsers => match registry.keystone.list_users(&default_pagination).await {
            Ok(resp) => Some(AppEvent::UsersLoaded(resp.items)),
            Err(e) => Some(api_error("FetchUsers", e)),
        },
        Action::CreateUser(params) => match registry.keystone.create_user(&params).await {
            Ok(user) => Some(AppEvent::UserCreated(user)),
            Err(e) => Some(api_error("CreateUser", e)),
        },
        Action::DeleteUser { id } => match registry.keystone.delete_user(&id).await {
            Ok(()) => Some(AppEvent::UserDeleted { id }),
            Err(e) => Some(api_error("DeleteUser", e)),
        },

        // -- Nova: Volume Attach/Detach (via Nova os-volume_attachments API) --
        Action::AttachVolume {
            volume_id,
            server_id,
            device,
        } => {
            match registry
                .nova
                .attach_volume(&server_id, &volume_id, device.as_deref())
                .await
            {
                Ok(()) => Some(AppEvent::VolumeAttached {
                    volume_id,
                    server_id,
                }),
                Err(e) => Some(api_error("AttachVolume", e)),
            }
        }
        Action::DetachVolume {
            volume_id,
            server_id,
            ..
        } => match registry.nova.detach_volume(&server_id, &volume_id).await {
            Ok(()) => Some(AppEvent::VolumeDetached { volume_id }),
            Err(e) => Some(api_error("DetachVolume", e)),
        },
        Action::ForceDetachVolume {
            volume_id,
            attachment_id,
            ..
        } => {
            match registry
                .cinder
                .force_detach_volume(&volume_id, &attachment_id)
                .await
            {
                Ok(()) => Some(AppEvent::VolumeForceDetached { volume_id }),
                Err(e) => Some(api_error("ForceDetachVolume", e)),
            }
        }
        Action::ForceResetVolumeState {
            volume_id,
            target_state,
        } => {
            match registry
                .cinder
                .force_set_volume_state(&volume_id, &target_state)
                .await
            {
                Ok(()) => Some(AppEvent::VolumeStateReset { volume_id }),
                Err(e) => Some(api_error("ForceResetVolumeState", e)),
            }
        }

        // -- Neutron: Floating IP Associate/Disassociate --------------------
        Action::AssociateFloatingIp { fip_id, port_id } => {
            match registry
                .neutron
                .associate_floating_ip(&fip_id, &port_id)
                .await
            {
                Ok(fip) => Some(AppEvent::FloatingIpAssociated(fip)),
                Err(e) => Some(api_error("AssociateFloatingIp", e)),
            }
        }
        Action::DisassociateFloatingIp { fip_id } => {
            match registry.neutron.disassociate_floating_ip(&fip_id).await {
                Ok(fip) => Some(AppEvent::FloatingIpDisassociated(fip)),
                Err(e) => Some(api_error("DisassociateFloatingIp", e)),
            }
        }

        // -- Neutron: Ports -------------------------------------------------
        Action::FetchPorts { server_id } => match registry.neutron.list_ports(&server_id).await {
            Ok(ports) => Some(AppEvent::PortsLoaded { server_id, ports }),
            Err(e) => Some(api_error("FetchPorts", e)),
        },
        Action::FetchPortBindingsForServer { server_id } => {
            // Two-step: list ports for the server, then for each port fetch
            // its bindings concurrently. We swallow per-port binding errors
            // (e.g. 403 on non-admin clouds) so a single failure doesn't
            // black out the whole section — the user still sees the ports
            // that did succeed.
            match registry.neutron.list_ports(&server_id).await {
                Ok(ports) => {
                    let mut futs = Vec::with_capacity(ports.len());
                    for p in &ports {
                        let port_id = p.id.clone();
                        let neutron = registry.neutron.clone();
                        futs.push(async move {
                            let bindings = neutron
                                .list_port_bindings(&port_id)
                                .await
                                .unwrap_or_default();
                            (port_id, bindings)
                        });
                    }
                    let port_bindings = futures::future::join_all(futs).await;
                    Some(AppEvent::PortBindingsLoaded {
                        server_id,
                        port_bindings,
                    })
                }
                Err(e) => Some(api_error("FetchPortBindingsForServer", e)),
            }
        }

        // -- UI-only actions (handled by App::dispatch_action, not worker) --
        Action::Navigate(_)
        | Action::Back
        | Action::FocusSidebar
        | Action::SelectResource { .. }
        | Action::NavigateToResource { .. }
        | Action::EnterFormMode
        | Action::ExitFormMode
        | Action::ToggleAllTenants
        | Action::ShowToast { .. }
        | Action::Quit => None,

        // -- System ---------------------------------------------------------
        Action::RefreshAll => {
            // RefreshAll is not handled by the worker — App::dispatch_action should
            // expand it into individual Fetch actions. If it reaches here, ignore.
            None
        }

        Action::SwitchContext(_) | Action::SwitchBack => {
            // Intercepted by `App::dispatch_action` (Unit 4) — a real
            // switch never reaches the worker. This arm stays as a
            // defensive no-op so a misrouted action is a drop, not a
            // panic.
            None
        }
    }
}

/// Determine if an action should trigger migration-progress polling after success.
fn poll_migration_server_id(action: &Action) -> Option<String> {
    match action {
        Action::LiveMigrateServer { id, .. } | Action::ColdMigrateServer { id, .. } => {
            Some(id.clone())
        }
        _ => None,
    }
}

/// Determine if an action should trigger server-status polling after success.
fn poll_server_id_for_status(action: &Action) -> Option<String> {
    match action {
        Action::ResizeServer { id, .. }
        | Action::ConfirmResize { id }
        | Action::RevertResize { id }
        | Action::RebootServer { id, .. }
        | Action::StartServer { id }
        | Action::StopServer { id } => Some(id.clone()),
        _ => None,
    }
}

/// Return a dedup key for Fetch-type actions (parameterless list fetches).
/// Returns None for mutations, parameterized fetches, and non-fetch actions.
fn fetch_dedup_key(action: &Action) -> Option<&'static str> {
    match action {
        Action::FetchServers => Some("FetchServers"),
        Action::FetchVolumes => Some("FetchVolumes"),
        Action::FetchNetworks => Some("FetchNetworks"),
        Action::FetchImages => Some("FetchImages"),
        Action::FetchFlavors => Some("FetchFlavors"),
        Action::FetchSnapshots => Some("FetchSnapshots"),
        Action::FetchFloatingIps => Some("FetchFloatingIps"),
        Action::FetchSecurityGroups => Some("FetchSecurityGroups"),
        Action::FetchProjects => Some("FetchProjects"),
        Action::FetchUsers => Some("FetchUsers"),
        Action::FetchAggregates => Some("FetchAggregates"),
        Action::FetchComputeServices => Some("FetchComputeServices"),
        Action::FetchHypervisors => Some("FetchHypervisors"),
        Action::FetchAgents => Some("FetchAgents"),
        _ => None,
    }
}

use crate::models::common::is_terminal_server_status;

/// Poll server status every 2 seconds until it reaches a terminal state.
///
/// BL-P2-085 Step 18 (Phase 11) — background read-only contract: this
/// task sends `AppEvent::ServerStatusPolled` directly on the event
/// channel and **never** dispatches a mutation through `ActionSender`.
/// FR2 origin-stamping therefore does not apply here; cross-project
/// scope checks remain a property of the user-initiated mutation path
/// (worker recv loop). If a future refactor needs the polling task to
/// emit mutation `Action`s, route them through `ActionSender::send` so
/// the Step-9 central stamping covers them automatically — do not
/// hand-craft `DispatchedAction`s in the polling body.
async fn poll_server_status(
    registry: &AdapterRegistry,
    event_tx: &mpsc::UnboundedSender<VersionedEvent<AppEvent>>,
    epoch: Epoch,
    server_id: &str,
) {
    const POLL_INTERVAL: std::time::Duration = std::time::Duration::from_secs(2);
    const MAX_POLLS: usize = 60; // 2 minutes max

    for _ in 0..MAX_POLLS {
        tokio::time::sleep(POLL_INTERVAL).await;
        match registry.nova.get_server(server_id).await {
            Ok(server) => {
                let done = is_terminal_server_status(&server.status);
                let _ = event_tx.send(VersionedEvent::new(
                    AppEvent::ServerStatusPolled {
                        server: server.clone(),
                    },
                    epoch,
                ));
                if done {
                    return;
                }
            }
            Err(_) => return,
        }
    }
}

/// Poll migration progress every 2 seconds until completed or error.
///
/// BL-P2-085 Step 18 (Phase 11) — background read-only contract: same
/// invariant as [`poll_server_status`]. The two `AppEvent` variants
/// emitted here (`MigrationProgressLoaded` / `MigrationPollingStopped`)
/// flow on the event channel directly and bypass the FR2 stamping
/// path. Adding mutation dispatch to this body must go through
/// `ActionSender::send` (Step 9 central stamping).
async fn poll_migration_progress(
    registry: &AdapterRegistry,
    event_tx: &mpsc::UnboundedSender<VersionedEvent<AppEvent>>,
    epoch: Epoch,
    server_id: &str,
) {
    const POLL_INTERVAL: std::time::Duration = std::time::Duration::from_secs(2);
    const MAX_POLLS: usize = 150; // 5 minutes max

    for _ in 0..MAX_POLLS {
        tokio::time::sleep(POLL_INTERVAL).await;
        match registry.nova.list_server_migrations(server_id).await {
            Ok(migrations) => {
                if let Some(migration) = migrations.into_iter().last() {
                    let done = matches!(
                        migration.status.as_str(),
                        "completed" | "confirmed" | "error" | "cancelled"
                    );
                    let _ = event_tx.send(VersionedEvent::new(
                        AppEvent::MigrationProgressLoaded {
                            server_id: server_id.to_string(),
                            migration,
                        },
                        epoch,
                    ));
                    if done {
                        break;
                    }
                } else {
                    // No migrations found — migration may have completed before first poll
                    break;
                }
            }
            Err(_) => {
                // API error (e.g. 404 after migration completed) — stop polling
                break;
            }
        }
    }
    // Always notify app to refresh server list when polling ends
    let _ = event_tx.send(VersionedEvent::new(
        AppEvent::MigrationPollingStopped {
            server_id: server_id.to_string(),
        },
        epoch,
    ));
}

fn api_error(operation: &str, error: crate::port::error::ApiError) -> AppEvent {
    tracing::error!(operation, error = %error, "API call failed");
    AppEvent::ApiError {
        operation: operation.to_string(),
        message: error.to_string(),
    }
}

// Nova returns "No valid host was found" generically when any conductor-side
// step makes scheduling fail. A common cause is a stale Neutron port binding
// left over from a prior failed live-migration attempt — invisible to the
// nexttui user without controller log access. We append a one-line hint
// pointing to the 'Port bindings' section we render in the Server Detail.
fn enrich_live_migrate_error(error: crate::port::error::ApiError) -> crate::port::error::ApiError {
    use crate::port::error::ApiError;
    match error {
        ApiError::BadRequest(msg) if msg.contains("No valid host") => {
            ApiError::BadRequest(format!(
                "{msg} (hint: stale port binding likely — check 'Port bindings' in instance detail)"
            ))
        }
        other => other,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_action_to_kind_cud_actions() {
        use crate::infra::rbac::ActionKind;
        // Create actions should map to ActionKind::Create
        assert_eq!(
            action_to_kind(&Action::CreateServer(
                crate::port::types::ServerCreateParams {
                    name: "t".into(),
                    image_id: "i".into(),
                    flavor_id: "f".into(),
                    networks: vec![],
                    security_groups: None,
                    key_name: None,
                    availability_zone: None,
                }
            )),
            Some(ActionKind::Create),
        );
        // Delete actions should map to ActionKind::Delete
        assert_eq!(
            action_to_kind(&Action::DeleteServer {
                id: "s1".into(),
                name: "web".into()
            }),
            Some(ActionKind::Delete),
        );
        // ForceDelete
        assert_eq!(
            action_to_kind(&Action::DeleteVolume {
                id: "v1".into(),
                force: true
            }),
            Some(ActionKind::ForceDelete),
        );
        // Fetch actions should return None (no guard needed)
        assert_eq!(action_to_kind(&Action::FetchServers), None);

        // Migration actions should map to Migrate
        assert_eq!(
            action_to_kind(&Action::LiveMigrateServer {
                id: "s1".into(),
                host: None
            }),
            Some(ActionKind::Migrate),
        );
        assert_eq!(
            action_to_kind(&Action::ColdMigrateServer { id: "s1".into() }),
            Some(ActionKind::Migrate),
        );
        assert_eq!(
            action_to_kind(&Action::ConfirmMigration { id: "s1".into() }),
            Some(ActionKind::Migrate),
        );
        assert_eq!(
            action_to_kind(&Action::RevertMigration { id: "s1".into() }),
            Some(ActionKind::Migrate),
        );
        // Evacuate should map to Evacuate
        assert_eq!(
            action_to_kind(&Action::EvacuateServer {
                id: "s1".into(),
                params: EvacuateParams::default()
            }),
            Some(ActionKind::Evacuate),
        );
        // Disable/Enable should map to EnableDisable (admin-only)
        assert_eq!(
            action_to_kind(&Action::DisableComputeService {
                service_id: "svc-1".into(),
                hostname: "h1".into()
            }),
            Some(ActionKind::EnableDisable),
        );
        assert_eq!(
            action_to_kind(&Action::EnableComputeService {
                service_id: "svc-1".into(),
                hostname: "h1".into()
            }),
            Some(ActionKind::EnableDisable),
        );
        // FetchMigrationProgress is read-only
        assert_eq!(
            action_to_kind(&Action::FetchMigrationProgress {
                server_id: "s1".into()
            }),
            None,
        );
    }

    // --- BL-P2-085 Step 7: action_to_kind exhaustive + action_is_mutation ---

    #[test]
    fn test_action_to_kind_fetch_and_nav_variants_return_none() {
        // Fetch* + Navigate/Back + UI helpers + system + context switch all
        // must return None — they are not RBAC-gated mutations.
        let none_actions = vec![
            Action::Navigate(crate::models::common::Route::Servers),
            Action::Back,
            Action::FetchServers,
            Action::FetchFlavors,
            Action::FetchAggregates,
            Action::FetchComputeServices,
            Action::FetchHypervisors,
            Action::FetchNetworks,
            Action::FetchSecurityGroups,
            Action::FetchFloatingIps,
            Action::FetchSubnets {
                network_id: "n1".into(),
            },
            Action::FetchAgents,
            Action::FetchVolumes,
            Action::FetchSnapshots,
            Action::FetchImages,
            Action::FetchProjects,
            Action::FetchUsers,
            Action::FetchUsage {
                start: "s".into(),
                end: "e".into(),
            },
            Action::FetchPorts {
                server_id: "s1".into(),
            },
            Action::FocusSidebar,
            Action::EnterFormMode,
            Action::ExitFormMode,
            Action::SelectResource { id: "x".into() },
            Action::NavigateToResource {
                route: crate::models::common::Route::Volumes,
                id: "v1".into(),
            },
            Action::ToggleAllTenants, // UI state toggle, not backend mutation
            Action::ShowToast {
                message: "hi".into(),
            },
            Action::RefreshAll,
            Action::Quit,
            Action::SwitchBack, // orchestration, not RBAC mutation
        ];
        for a in &none_actions {
            assert_eq!(
                action_to_kind(a),
                None,
                "{:?} must return None (not a mutation)",
                std::mem::discriminant(a)
            );
        }
    }

    #[test]
    fn test_action_to_kind_all_mutations_have_kind() {
        use crate::port::types::*;
        let mutations: Vec<Action> = vec![
            Action::CreateServer(ServerCreateParams {
                name: "t".into(),
                image_id: "i".into(),
                flavor_id: "f".into(),
                networks: vec![],
                security_groups: None,
                key_name: None,
                availability_zone: None,
            }),
            Action::DeleteServer {
                id: "s".into(),
                name: "n".into(),
            },
            Action::RebootServer {
                id: "s".into(),
                hard: false,
            },
            Action::StartServer { id: "s".into() },
            Action::StopServer { id: "s".into() },
            Action::CreateServerSnapshot {
                server_id: "s".into(),
                name: "snap".into(),
            },
            Action::CreateFlavor(FlavorCreateParams {
                name: "f".into(),
                vcpus: 1,
                ram_mb: 1,
                disk_gb: 1,
                is_public: true,
            }),
            Action::DeleteFlavor { id: "f".into() },
            Action::CreateNetwork(NetworkCreateParams {
                name: "n".into(),
                admin_state_up: true,
                shared: None,
                external: None,
                mtu: None,
                port_security_enabled: None,
            }),
            Action::CreateSecurityGroup(SecurityGroupCreateParams {
                name: "sg".into(),
                description: None,
            }),
            Action::DeleteSecurityGroup { id: "sg".into() },
            Action::CreateSecurityGroupRule(SecurityGroupRuleCreateParams {
                security_group_id: "sg".into(),
                direction: RuleDirection::Ingress,
                protocol: None,
                port_range_min: None,
                port_range_max: None,
                remote_ip_prefix: None,
                remote_group_id: None,
                ethertype: None,
            }),
            Action::DeleteSecurityGroupRule {
                rule_id: "r".into(),
            },
            Action::CreateFloatingIp {
                network_id: "n".into(),
            },
            Action::DeleteFloatingIp { id: "f".into() },
            Action::CreateVolume(VolumeCreateParams {
                name: "v".into(),
                size_gb: 1,
                volume_type: None,
                description: None,
                availability_zone: None,
            }),
            Action::DeleteVolume {
                id: "v".into(),
                force: false,
            },
            Action::ExtendVolume {
                id: "v".into(),
                new_size: 2,
            },
            Action::CreateSnapshot(SnapshotCreateParams {
                name: "sn".into(),
                volume_id: "v".into(),
                description: None,
                force: false,
            }),
            Action::DeleteSnapshot { id: "s".into() },
            Action::CreateImage(ImageCreateParams {
                name: "img".into(),
                disk_format: "qcow2".into(),
                container_format: "bare".into(),
                visibility: None,
                min_disk: None,
                min_ram: None,
            }),
            Action::DeleteImage { id: "i".into() },
            Action::CreateProject(ProjectCreateParams {
                name: "p".into(),
                description: None,
                domain_id: "default".into(),
                enabled: Some(true),
            }),
            Action::DeleteProject { id: "p".into() },
            Action::CreateUser(UserCreateParams {
                name: "u".into(),
                password: "pw".into(),
                email: None,
                domain_id: "default".into(),
                enabled: Some(true),
                default_project_id: None,
            }),
            Action::DeleteUser { id: "u".into() },
            Action::ResizeServer {
                id: "s".into(),
                flavor_id: "f".into(),
            },
            Action::ConfirmResize { id: "s".into() },
            Action::RevertResize { id: "s".into() },
            Action::LiveMigrateServer {
                id: "s".into(),
                host: None,
            },
            Action::ColdMigrateServer { id: "s".into() },
            Action::ConfirmMigration { id: "s".into() },
            Action::RevertMigration { id: "s".into() },
            Action::EvacuateServer {
                id: "s".into(),
                params: EvacuateParams::default(),
            },
            Action::DisableComputeService {
                service_id: "svc".into(),
                hostname: "h".into(),
            },
            Action::EnableComputeService {
                service_id: "svc".into(),
                hostname: "h".into(),
            },
            Action::AttachVolume {
                volume_id: "v".into(),
                server_id: "s".into(),
                device: None,
            },
            Action::DetachVolume {
                volume_id: "v".into(),
                server_id: "s".into(),
                attachment_id: "a".into(),
            },
            Action::ForceDetachVolume {
                volume_id: "v".into(),
                server_id: "s".into(),
                attachment_id: "a".into(),
            },
            Action::ForceResetVolumeState {
                volume_id: "v".into(),
                target_state: "available".into(),
            },
            Action::AssociateFloatingIp {
                fip_id: "f".into(),
                port_id: "p".into(),
            },
            Action::DisassociateFloatingIp { fip_id: "f".into() },
        ];
        for a in &mutations {
            assert!(
                action_to_kind(a).is_some(),
                "{:?} must map to Some(ActionKind)",
                std::mem::discriminant(a)
            );
        }
    }

    #[test]
    fn test_action_to_kind_rbac_mapping_lockstep() {
        // Explicit lockstep — each mutation variant maps to the documented
        // ActionKind. Catches accidental reclassification.
        use crate::infra::rbac::ActionKind;
        use crate::port::types::*;

        let cases: Vec<(Action, ActionKind)> = vec![
            (
                Action::CreateServer(ServerCreateParams {
                    name: "t".into(),
                    image_id: "i".into(),
                    flavor_id: "f".into(),
                    networks: vec![],
                    security_groups: None,
                    key_name: None,
                    availability_zone: None,
                }),
                ActionKind::Create,
            ),
            (
                Action::DeleteVolume {
                    id: "v".into(),
                    force: true,
                },
                ActionKind::ForceDelete,
            ),
            (
                Action::DeleteVolume {
                    id: "v".into(),
                    force: false,
                },
                ActionKind::Delete,
            ),
            (
                Action::ResizeServer {
                    id: "s".into(),
                    flavor_id: "f".into(),
                },
                ActionKind::Resize,
            ),
            (
                Action::LiveMigrateServer {
                    id: "s".into(),
                    host: None,
                },
                ActionKind::Migrate,
            ),
            (
                Action::EvacuateServer {
                    id: "s".into(),
                    params: EvacuateParams::default(),
                },
                ActionKind::Evacuate,
            ),
            (
                Action::DisableComputeService {
                    service_id: "svc".into(),
                    hostname: "h".into(),
                },
                ActionKind::EnableDisable,
            ),
            (
                Action::CreateProject(ProjectCreateParams {
                    name: "p".into(),
                    description: None,
                    domain_id: "default".into(),
                    enabled: Some(true),
                }),
                ActionKind::ManageQuota,
            ),
            (
                Action::AttachVolume {
                    volume_id: "v".into(),
                    server_id: "s".into(),
                    device: None,
                },
                ActionKind::Attach,
            ),
            (
                Action::DetachVolume {
                    volume_id: "v".into(),
                    server_id: "s".into(),
                    attachment_id: "a".into(),
                },
                ActionKind::Detach,
            ),
            (
                Action::ForceDetachVolume {
                    volume_id: "v".into(),
                    server_id: "s".into(),
                    attachment_id: "a".into(),
                },
                ActionKind::ForceDelete,
            ),
        ];
        for (action, expected) in &cases {
            assert_eq!(
                action_to_kind(action),
                Some(*expected),
                "RBAC lockstep mismatch for {:?}",
                std::mem::discriminant(action)
            );
        }
    }

    #[test]
    fn test_action_is_mutation_helper_parity() {
        let m = Action::DeleteServer {
            id: "s".into(),
            name: "n".into(),
        };
        let r = Action::FetchServers;
        assert!(action_is_mutation(&m));
        assert!(!action_is_mutation(&r));
        // Wrapper parity: action_is_mutation == action_to_kind.is_some()
        for a in [&m, &r] {
            assert_eq!(action_is_mutation(a), action_to_kind(a).is_some());
        }
    }

    // --- BL-P2-085 Step 11a: worker origin/active guard hook ---

    fn rbac_with_project(project_id: &str) -> RbacGuard {
        use crate::port::types::TokenRole;
        let guard = RbacGuard::new();
        guard.update_roles(
            vec![TokenRole {
                id: "member-id".into(),
                name: "member".into(),
            }],
            Some(project_id.into()),
        );
        guard
    }

    fn actor_ctx_with(cloud: &str, user_id: &str) -> Arc<RwLock<ActorContext>> {
        Arc::new(RwLock::new(ActorContext {
            cloud: cloud.into(),
            user_id: user_id.into(),
        }))
    }

    #[test]
    fn test_worker_allows_mutation_when_origin_matches() {
        use crate::infra::cross_project_guard::GuardDecision;

        let rbac = rbac_with_project("p-active");
        let dispatched = DispatchedAction::stamped(
            Action::DeleteServer {
                id: "s1".into(),
                name: "n".into(),
            },
            "p-active".into(),
        );
        assert_eq!(
            check_dispatched_origin(&dispatched, &rbac),
            GuardDecision::Allow,
        );
    }

    #[test]
    fn test_worker_blocks_mutation_when_origin_mismatch() {
        use crate::infra::cross_project_guard::{CrossProjectReason, GuardDecision};

        let rbac = rbac_with_project("p-active");
        let dispatched = DispatchedAction::stamped(
            Action::DeleteServer {
                id: "s1".into(),
                name: "n".into(),
            },
            "p-stale".into(),
        );
        match check_dispatched_origin(&dispatched, &rbac) {
            GuardDecision::Block {
                reason:
                    CrossProjectReason::OriginScopeMismatch {
                        ref origin,
                        ref active,
                    },
            } => {
                assert_eq!(origin, "p-stale");
                assert_eq!(active, "p-active");
            }
            other => panic!("expected OriginScopeMismatch Block, got {other:?}"),
        }
    }

    // --- BL-P2-085 Step 11b: AuditLogger integration on Block ---

    #[test]
    fn test_emit_origin_block_audit_writes_entry_when_logger_present() {
        use crate::infra::audit::AuditLogger;
        use crate::infra::cross_project_guard::CrossProjectReason;

        let dir = tempfile::TempDir::new().unwrap();
        let path = dir.path().join("audit.log");
        let logger = AuditLogger::new(path.clone()).unwrap();

        let rbac = rbac_with_project("p-active");
        let dispatched = DispatchedAction::stamped(
            Action::DeleteServer {
                id: "s1".into(),
                name: "n".into(),
            },
            "p-stale".into(),
        );
        let reason = CrossProjectReason::OriginScopeMismatch {
            origin: "p-stale".into(),
            active: "p-active".into(),
        };
        let actor = actor_ctx_with("devstack", "user-uuid");

        emit_origin_block_audit(reason, &dispatched, &rbac, Some(&logger), &actor, 42);

        let content = std::fs::read_to_string(&path).unwrap();
        assert!(
            !content.is_empty(),
            "audit log must contain entry after Block emit"
        );
        let parsed: serde_json::Value = serde_json::from_str(content.trim()).unwrap();
        assert_eq!(parsed["action"], "DeleteServer");
        assert_eq!(parsed["cloud"], "devstack");
        assert_eq!(parsed["user"], "user-uuid");
        assert_eq!(parsed["project"], "p-active");
        assert_eq!(
            parsed["result"],
            serde_json::json!({ "failed": "cross_project_block:origin_scope_mismatch" }),
        );
        assert_eq!(parsed["details"]["guard_layer"], "fr2_worker");
        assert_eq!(parsed["details"]["correlation_id"], 42);
        assert_eq!(parsed["details"]["asserted_origin_project_id"], "p-stale");
    }

    #[test]
    fn test_emit_origin_block_audit_does_not_panic_when_logger_none() {
        use crate::infra::cross_project_guard::CrossProjectReason;

        let rbac = rbac_with_project("p-active");
        let dispatched = DispatchedAction::stamped(
            Action::DeleteServer {
                id: "s1".into(),
                name: "n".into(),
            },
            "p-stale".into(),
        );
        let reason = CrossProjectReason::OriginScopeMismatch {
            origin: "p-stale".into(),
            active: "p-active".into(),
        };
        let actor = actor_ctx_with("devstack", "user-uuid");

        // logger=None must remain best-effort — the worker must still block,
        // and emit() falls back to tracing without panicking.
        emit_origin_block_audit(reason, &dispatched, &rbac, None, &actor, 7);
    }

    #[test]
    fn test_emit_origin_block_audit_picks_up_actor_context_mutation() {
        use crate::infra::audit::AuditLogger;
        use crate::infra::cross_project_guard::CrossProjectReason;

        // Phase 7 폴리싱: a runtime cloud-switch (BL-P2-074) mutates
        // `ActorContext.cloud` in place via the shared `RwLock`. The next
        // worker block emit must reflect the new cloud — without live read,
        // audit entries stay anchored to the worker's spawn cloud forever.
        let dir = tempfile::TempDir::new().unwrap();
        let path = dir.path().join("audit.log");
        let logger = AuditLogger::new(path.clone()).unwrap();

        let rbac = rbac_with_project("p-active");
        let dispatched = || {
            DispatchedAction::stamped(
                Action::DeleteServer {
                    id: "s1".into(),
                    name: "n".into(),
                },
                "p-stale".into(),
            )
        };
        let reason_a = CrossProjectReason::OriginScopeMismatch {
            origin: "p-stale".into(),
            active: "p-active".into(),
        };
        let reason_b = reason_a.clone();
        let actor = actor_ctx_with("cloud-A", "user-uuid");

        emit_origin_block_audit(reason_a, &dispatched(), &rbac, Some(&logger), &actor, 1);

        // Cloud-switch arrives between dispatches — App's ContextChanged
        // handler updates the shared RwLock.
        actor
            .write()
            .expect("actor_ctx write lock")
            .cloud
            .replace_range(.., "cloud-B");

        emit_origin_block_audit(reason_b, &dispatched(), &rbac, Some(&logger), &actor, 2);

        let content = std::fs::read_to_string(&path).unwrap();
        let lines: Vec<&str> = content.lines().collect();
        assert_eq!(lines.len(), 2, "expected 2 audit lines, got: {content}");
        let first: serde_json::Value = serde_json::from_str(lines[0]).unwrap();
        let second: serde_json::Value = serde_json::from_str(lines[1]).unwrap();
        assert_eq!(
            first["cloud"], "cloud-A",
            "first emit must use the spawn-time cloud"
        );
        assert_eq!(
            second["cloud"], "cloud-B",
            "second emit must reflect post-switch cloud (live RwLock read)"
        );
    }

    // --- BL-P2-085 Step 16 (Phase 9): FR4 form-layer image-owner guard ---

    fn sample_image(id: &str, owner: Option<&str>) -> Image {
        Image {
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
    fn test_check_image_owner_scope_match_allows() {
        let img = sample_image("img-1", Some("proj-A"));
        assert_eq!(
            check_image_owner_scope(&img, "proj-A"),
            GuardDecision::Allow
        );
    }

    #[test]
    fn test_check_image_owner_scope_mismatch_blocks() {
        let img = sample_image("img-1", Some("proj-B"));
        match check_image_owner_scope(&img, "proj-A") {
            GuardDecision::Block { reason } => match reason {
                CrossProjectReason::FormSelectionMismatch { selected, active } => {
                    assert_eq!(selected, "proj-B");
                    assert_eq!(active, "proj-A");
                }
                other => panic!("expected FormSelectionMismatch, got {other:?}"),
            },
            GuardDecision::Allow => panic!("mismatch must block"),
        }
    }

    #[test]
    fn test_check_image_owner_scope_missing_owner_fail_safe() {
        // Glance omitted `owner` → can't prove same-project → fail-safe deny.
        let img = sample_image("img-2", None);
        match check_image_owner_scope(&img, "proj-A") {
            GuardDecision::Block { reason } => match reason {
                CrossProjectReason::FormSelectionMismatch { selected, active } => {
                    assert_eq!(selected, "", "None owner encodes as empty selected");
                    assert_eq!(active, "proj-A");
                }
                other => panic!("expected FormSelectionMismatch, got {other:?}"),
            },
            GuardDecision::Allow => panic!("missing owner must fail-safe deny"),
        }
    }

    // cargo-review branch-full Correctness #5 (S-class follow-up):
    // RefilterScope::strict / scope_validator::validate_form_scope parity —
    // empty active must trip in dev so a leaked `Some("")` from the token
    // doesn't quietly Allow because `owner == Some("")` happens to match.
    #[test]
    #[should_panic(expected = "non-empty active")]
    fn test_check_image_owner_scope_panics_on_empty_active() {
        let img = sample_image("img-1", Some(""));
        let _ = check_image_owner_scope(&img, "");
    }

    #[test]
    fn test_emit_form_block_audit_writes_entry_with_fr4_form_layer() {
        let dir = tempfile::TempDir::new().unwrap();
        let path = dir.path().join("audit.log");
        let logger = AuditLogger::new(path.clone()).unwrap();
        let actor = actor_ctx_with("devstack", "user-uuid");

        let reason = CrossProjectReason::FormSelectionMismatch {
            selected: "proj-B".into(),
            active: "proj-A".into(),
        };
        emit_form_block_audit(
            reason,
            "DeleteImage",
            "image",
            "img-99",
            Some("proj-A".to_string()),
            Some(&logger),
            &actor,
            17,
        );

        let content = std::fs::read_to_string(&path).unwrap();
        assert!(!content.is_empty(), "audit log must contain entry");
        let parsed: serde_json::Value = serde_json::from_str(content.trim()).unwrap();
        assert_eq!(parsed["action"], "DeleteImage");
        assert_eq!(parsed["resource_id"], "img-99");
        assert_eq!(parsed["details"]["guard_layer"], "fr4_form");
        assert_eq!(parsed["details"]["correlation_id"], 17);
        assert_eq!(
            parsed["result"],
            serde_json::json!({ "failed": "cross_project_block:form_selection_mismatch" })
        );
    }

    #[test]
    fn test_emit_form_block_audit_does_not_panic_when_logger_none() {
        // logger=None must remain best-effort (tracing::warn! fallback).
        let actor = actor_ctx_with("devstack", "user-uuid");
        let reason = CrossProjectReason::FormSelectionMismatch {
            selected: "proj-B".into(),
            active: "proj-A".into(),
        };
        emit_form_block_audit(
            reason,
            "DeleteImage",
            "image",
            "img-99",
            Some("proj-A".to_string()),
            None,
            &actor,
            1,
        );
    }

    // --- BL-P2-085 Step 18 (Phase 11): background poll read-only audit ---
    //
    // The plan-time worry was that background pollers might dispatch
    // mutation actions with a stale scope. Inspection shows the two
    // existing pollers (`poll_server_status` / `poll_migration_progress`)
    // only send `AppEvent`s on `event_tx` — never `ActionSender::send`.
    // The architectural pin lives on the two function-level doc comments
    // (`poll_server_status` / `poll_migration_progress`): any future
    // change that needs the polling task to dispatch a mutation must
    // route through `ActionSender::send` so the Step-9 central stamping
    // handles origin automatically. Hand-built `DispatchedAction`s
    // inside poll bodies are forbidden.
    //
    // No runtime test is added here because (a) the existing pollers
    // are async, multi-minute loops not suited to unit-level invocation
    // without elaborate mocking, and (b) an `async fn` signature
    // doesn't coerce to a `fn` pointer, so a compile-time signature pin
    // would be artificial. The two doc comments + this module-level
    // note are the canonical guard.

    // --- BL-P2-085 Step 11c: read-only bypass + AppEvent::CrossProjectBlocked ---

    #[test]
    fn test_worker_allows_readonly_without_guard() {
        use crate::infra::cross_project_guard::GuardDecision;

        // Read-only (unstamped) actions carry `origin_project_id = None`. The
        // worker must let them through without invoking the origin guard, even
        // if `RbacGuard.project_id()` is `None` (pre-auth state).
        let rbac = RbacGuard::new(); // no project_id set
        let dispatched = DispatchedAction::unstamped(Action::FetchServers);
        assert_eq!(
            check_dispatched_origin(&dispatched, &rbac),
            GuardDecision::Allow,
            "unstamped actions must skip the origin guard",
        );

        // Even when `RbacGuard` has an active project, an unstamped action
        // still bypasses (the guard only fires on stamped envelopes).
        let scoped = rbac_with_project("p-active");
        let dispatched_scoped = DispatchedAction::unstamped(Action::FetchServers);
        assert_eq!(
            check_dispatched_origin(&dispatched_scoped, &scoped),
            GuardDecision::Allow,
        );
    }

    #[test]
    fn test_make_cross_project_blocked_event_carries_reason_and_action() {
        use crate::infra::cross_project_guard::CrossProjectReason;

        let action = Action::DeleteServer {
            id: "s1".into(),
            name: "web".into(),
        };
        let reason = CrossProjectReason::OriginScopeMismatch {
            origin: "p-stale".into(),
            active: "p-active".into(),
        };
        let event = make_cross_project_blocked_event(&reason, &action);
        match event {
            AppEvent::CrossProjectBlocked {
                reason: ev_reason,
                action: ev_action,
            } => {
                assert_eq!(ev_reason, "origin_scope_mismatch");
                assert_eq!(ev_action, "DeleteServer");
            }
            other => panic!("expected CrossProjectBlocked, got {other:?}"),
        }
    }

    #[test]
    fn test_action_to_kind_switch_context_returns_none() {
        // Context switch is orchestration (Keystone rescope), not an RBAC-gated
        // mutation. Worker handles it via a different path.
        let action = Action::SwitchContext(crate::context::ContextRequest::CloudOnly {
            cloud: "devstack".into(),
        });
        assert_eq!(action_to_kind(&action), None);
    }

    #[test]
    fn test_action_to_kind_resize_actions() {
        // Resize actions should map to ActionKind::Resize (member-level)
        assert_eq!(
            action_to_kind(&Action::ResizeServer {
                id: "s1".into(),
                flavor_id: "f2".into()
            }),
            Some(ActionKind::Resize),
        );
        assert_eq!(
            action_to_kind(&Action::ConfirmResize { id: "s1".into() }),
            Some(ActionKind::Resize),
        );
        assert_eq!(
            action_to_kind(&Action::RevertResize { id: "s1".into() }),
            Some(ActionKind::Resize),
        );
    }

    #[test]
    fn test_permission_denied_event_on_guard_failure() {
        // Verify PermissionDenied event can be constructed with operation name
        let event = AppEvent::PermissionDenied {
            operation: "CreateServer".into(),
        };
        match event {
            AppEvent::PermissionDenied { operation } => assert_eq!(operation, "CreateServer"),
            _ => panic!("expected PermissionDenied"),
        }
    }

    #[test]
    fn test_resize_actions_trigger_status_polling() {
        // ResizeServer, ConfirmResize, RevertResize should all be identified
        // as actions requiring server status polling
        let resize = Action::ResizeServer {
            id: "s1".into(),
            flavor_id: "f2".into(),
        };
        let confirm = Action::ConfirmResize { id: "s1".into() };
        let revert = Action::RevertResize { id: "s1".into() };

        assert_eq!(poll_server_id_for_status(&resize), Some("s1".to_string()));
        assert_eq!(poll_server_id_for_status(&confirm), Some("s1".to_string()));
        assert_eq!(poll_server_id_for_status(&revert), Some("s1".to_string()));

        // Non-resize actions should not trigger status polling
        assert_eq!(poll_server_id_for_status(&Action::FetchServers), None);
    }

    #[test]
    fn test_polling_dedup_guard() {
        use std::collections::HashSet;
        use std::sync::{Arc, Mutex};

        let guard: Arc<Mutex<HashSet<String>>> = Arc::new(Mutex::new(HashSet::new()));

        // First insert succeeds
        assert!(guard.lock().unwrap().insert("s1".to_string()));
        // Duplicate insert fails (already polling)
        assert!(!guard.lock().unwrap().insert("s1".to_string()));
        // Different server succeeds
        assert!(guard.lock().unwrap().insert("s2".to_string()));
        // Remove and re-insert succeeds
        guard.lock().unwrap().remove("s1");
        assert!(guard.lock().unwrap().insert("s1".to_string()));
    }

    #[test]
    fn test_cold_migrate_triggers_migration_polling() {
        // ColdMigrateServer should trigger migration polling
        assert_eq!(
            poll_migration_server_id(&Action::ColdMigrateServer { id: "s1".into() }),
            Some("s1".to_string()),
        );
        // LiveMigrate should still work
        assert_eq!(
            poll_migration_server_id(&Action::LiveMigrateServer {
                id: "s2".into(),
                host: None
            }),
            Some("s2".to_string()),
        );
        // FetchServers should not trigger
        assert_eq!(poll_migration_server_id(&Action::FetchServers), None);
    }

    #[test]
    fn test_reboot_start_stop_trigger_status_polling() {
        let reboot = Action::RebootServer {
            id: "s1".into(),
            hard: false,
        };
        let start = Action::StartServer { id: "s1".into() };
        let stop = Action::StopServer { id: "s1".into() };

        assert_eq!(poll_server_id_for_status(&reboot), Some("s1".to_string()));
        assert_eq!(poll_server_id_for_status(&start), Some("s1".to_string()));
        assert_eq!(poll_server_id_for_status(&stop), Some("s1".to_string()));
    }

    #[test]
    fn test_is_terminal_server_status() {
        assert!(is_terminal_server_status("ACTIVE"));
        assert!(is_terminal_server_status("ERROR"));
        assert!(is_terminal_server_status("VERIFY_RESIZE"));
        assert!(is_terminal_server_status("SHUTOFF"));

        assert!(!is_terminal_server_status("RESIZE"));
        assert!(!is_terminal_server_status("REVERT_RESIZE"));
        assert!(!is_terminal_server_status("MIGRATING"));
    }

    #[test]
    fn test_fetch_dedup_key_returns_key_for_fetch_actions() {
        assert_eq!(fetch_dedup_key(&Action::FetchServers), Some("FetchServers"));
        assert_eq!(fetch_dedup_key(&Action::FetchVolumes), Some("FetchVolumes"));
        assert_eq!(
            fetch_dedup_key(&Action::FetchNetworks),
            Some("FetchNetworks")
        );
        assert_eq!(fetch_dedup_key(&Action::FetchImages), Some("FetchImages"));
        assert_eq!(fetch_dedup_key(&Action::FetchFlavors), Some("FetchFlavors"));
        assert_eq!(
            fetch_dedup_key(&Action::FetchSnapshots),
            Some("FetchSnapshots")
        );
        assert_eq!(
            fetch_dedup_key(&Action::FetchFloatingIps),
            Some("FetchFloatingIps")
        );
        assert_eq!(
            fetch_dedup_key(&Action::FetchSecurityGroups),
            Some("FetchSecurityGroups")
        );
        assert_eq!(
            fetch_dedup_key(&Action::FetchProjects),
            Some("FetchProjects")
        );
        assert_eq!(fetch_dedup_key(&Action::FetchUsers), Some("FetchUsers"));
        assert_eq!(
            fetch_dedup_key(&Action::FetchAggregates),
            Some("FetchAggregates")
        );
        assert_eq!(
            fetch_dedup_key(&Action::FetchComputeServices),
            Some("FetchComputeServices")
        );
        assert_eq!(
            fetch_dedup_key(&Action::FetchHypervisors),
            Some("FetchHypervisors")
        );
        assert_eq!(fetch_dedup_key(&Action::FetchAgents), Some("FetchAgents"));
    }

    #[test]
    fn test_fetch_dedup_key_returns_none_for_mutations() {
        assert_eq!(
            fetch_dedup_key(&Action::DeleteServer {
                id: "s1".into(),
                name: "w1".into()
            }),
            None
        );
        assert_eq!(
            fetch_dedup_key(&Action::RebootServer {
                id: "s1".into(),
                hard: false
            }),
            None
        );
    }

    #[test]
    fn test_fetch_dedup_guard_skips_duplicate() {
        let guard: Arc<Mutex<HashSet<String>>> = Arc::new(Mutex::new(HashSet::new()));
        let key = "FetchServers";

        // First insert succeeds — action should proceed
        assert!(guard.lock().unwrap().insert(key.to_string()));
        // Duplicate — should be skipped
        assert!(!guard.lock().unwrap().insert(key.to_string()));
        // After removal — should succeed again
        guard.lock().unwrap().remove(key);
        assert!(guard.lock().unwrap().insert(key.to_string()));
    }

    #[test]
    fn test_api_error_creates_event() {
        let event = api_error(
            "FetchServers",
            crate::port::error::ApiError::NotFound {
                resource_type: "server".into(),
                id: "s1".into(),
            },
        );
        match event {
            AppEvent::ApiError { operation, message } => {
                assert_eq!(operation, "FetchServers");
                assert!(message.contains("not found") || message.contains("Not"));
            }
            _ => panic!("Expected ApiError"),
        }
    }

    // -- BL-P2-086: live-migrate "No valid host" enrichment --
    //
    // Nova returns "No valid host was found" as a generic message when
    // any conductor-side step (including stale Neutron port bindings)
    // makes scheduling fail. Without enrichment, users have no path
    // forward — the real cause is buried in controller logs.

    #[test]
    fn test_enrich_live_migrate_error_no_valid_host_adds_hint() {
        use crate::port::error::ApiError;
        let err = ApiError::BadRequest(
            "No valid host was found. There are not enough hosts available.".into(),
        );
        let enriched = enrich_live_migrate_error(err);
        let msg = enriched.to_string();
        // Original message preserved
        assert!(msg.contains("No valid host"));
        // Hint added — points users to the diagnostic section we render in detail
        assert!(
            msg.contains("Port bindings"),
            "expected hint to reference 'Port bindings' section, got: {msg}"
        );
        assert!(
            msg.to_lowercase().contains("stale"),
            "expected hint to mention 'stale', got: {msg}"
        );
    }

    #[test]
    fn test_enrich_live_migrate_error_other_bad_request_unchanged() {
        use crate::port::error::ApiError;
        let err = ApiError::BadRequest("Instance is locked".into());
        let enriched = enrich_live_migrate_error(err);
        // Non-matching BadRequest: no hint appended
        assert_eq!(enriched.to_string(), "Bad request: Instance is locked");
    }

    #[test]
    fn test_enrich_live_migrate_error_non_bad_request_unchanged() {
        use crate::port::error::ApiError;
        let err = ApiError::Forbidden("not admin".into());
        let enriched = enrich_live_migrate_error(err);
        // Other variants pass through unchanged
        assert_eq!(enriched.to_string(), "Forbidden: not admin");
    }

    // -- spawn_versioned (BL-P2-031 Unit 2) --

    #[tokio::test]
    async fn spawn_versioned_emits_event_with_epoch_when_not_cancelled() {
        let cancel = CancellationToken::new();
        let (tx, mut rx) = mpsc::unbounded_channel::<VersionedEvent<AppEvent>>();
        let handle = spawn_versioned(cancel, 7, tx, async {
            AppEvent::CloudSwitched("devstack".into())
        });
        handle.await.unwrap();
        let received = rx.try_recv().expect("event delivered");
        assert_eq!(received.epoch(), 7);
        match received.into_inner() {
            AppEvent::CloudSwitched(name) => assert_eq!(name, "devstack"),
            _ => panic!("unexpected event"),
        }
    }

    #[tokio::test]
    async fn spawn_versioned_drops_event_when_cancelled_before_completion() {
        use tokio::time::{Duration, sleep};

        let cancel = CancellationToken::new();
        let (tx, mut rx) = mpsc::unbounded_channel::<VersionedEvent<AppEvent>>();
        let cancel_clone = cancel.clone();
        let handle = spawn_versioned(cancel_clone, 1, tx, async {
            sleep(Duration::from_secs(5)).await;
            AppEvent::CloudSwitched("late".into())
        });

        // Give the spawn a moment to enter select.
        tokio::task::yield_now().await;
        cancel.cancel();
        handle.await.unwrap();
        assert!(rx.try_recv().is_err(), "no event should be delivered");
    }
}
