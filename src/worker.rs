//! Background worker: consumes Actions from the UI, calls OpenStack APIs,
//! and sends AppEvents back to the event loop for UI updates.

use std::collections::HashSet;
use std::future::Future;
use std::sync::{Arc, Mutex};
use std::sync::atomic::{AtomicBool, Ordering};

use chrono::{DateTime, Utc};
use tokio::sync::mpsc;
use tokio::task::JoinHandle;
use tokio_util::sync::CancellationToken;
use tracing::Instrument;

use crate::action::Action;
use crate::adapter::registry::AdapterRegistry;
use crate::context::{Epoch, VersionedEvent};
use crate::event::AppEvent;
use crate::infra::rbac::{ActionKind, RbacGuard};
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
#[tracing::instrument(skip_all)]
pub async fn run_worker(
    registry: Arc<AdapterRegistry>,
    rbac: Arc<RbacGuard>,
    all_tenants: Arc<AtomicBool>,
    mut action_rx: mpsc::UnboundedReceiver<VersionedEvent<Action>>,
    event_tx: mpsc::UnboundedSender<VersionedEvent<AppEvent>>,
) {
    let polling_servers: Arc<Mutex<HashSet<String>>> = Arc::new(Mutex::new(HashSet::new()));
    let in_flight_fetches: Arc<Mutex<HashSet<String>>> = Arc::new(Mutex::new(HashSet::new()));

    while let Some(envelope) = action_rx.recv().await {
        let (action, action_epoch) = envelope.into_parts();

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
            && !in_flight_fetches.lock().unwrap_or_else(|e| e.into_inner()).insert(key.to_string())
        {
            continue;
        }

        let registry = registry.clone();
        let event_tx = event_tx.clone();
        let all_tenants = all_tenants.clone();
        let polling_servers = polling_servers.clone();
        let in_flight_fetches = in_flight_fetches.clone();

        let poll_migration_id = poll_migration_server_id(&action);
        let poll_status_id = poll_server_id_for_status(&action);

        let span = tracing::info_span!("worker_task", action = action_name(&action));
        tokio::spawn(
            async move {
                let event = handle_action(&registry, &all_tenants, action).await;
                let success = event.as_ref().is_some_and(|ev| !matches!(ev, AppEvent::ApiError { .. }));
                if let Some(ev) = event {
                    let _ = event_tx.send(VersionedEvent::new(ev, action_epoch));
                }
                // Release fetch dedup guard
                if let Some(key) = dedup_key {
                    in_flight_fetches.lock().unwrap_or_else(|e| e.into_inner()).remove(key);
                }
                if success {
                    if let Some(ref server_id) = poll_migration_id
                        && polling_servers.lock().unwrap_or_else(|e| e.into_inner()).insert(server_id.clone())
                    {
                        poll_migration_progress(&registry, &event_tx, action_epoch, server_id).await;
                        polling_servers.lock().unwrap_or_else(|e| e.into_inner()).remove(server_id);
                    }
                    if let Some(ref server_id) = poll_status_id
                        && polling_servers.lock().unwrap_or_else(|e| e.into_inner()).insert(server_id.clone())
                    {
                        poll_server_status(&registry, &event_tx, action_epoch, server_id).await;
                        polling_servers.lock().unwrap_or_else(|e| e.into_inner()).remove(server_id);
                    }
                }
            }
            .instrument(span),
        );
    }
}

/// Map an Action to its RBAC ActionKind for permission checking.
/// Returns None for read-only/UI actions that need no guard.
fn action_to_kind(action: &Action) -> Option<ActionKind> {
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
        Action::CreateProject(_)
        | Action::CreateUser(_) => Some(ActionKind::ManageQuota),

        // Delete (member-level, non-force)
        Action::DeleteServer { .. }
        | Action::DeleteFlavor { .. }
        | Action::DeleteSecurityGroup { .. }
        | Action::DeleteSecurityGroupRule { .. }
        | Action::DeleteFloatingIp { .. }
        | Action::DeleteSnapshot { .. }
        | Action::DeleteImage { .. } => Some(ActionKind::Delete),

        // Delete (admin-only: identity resources)
        Action::DeleteProject { .. }
        | Action::DeleteUser { .. } => Some(ActionKind::ManageQuota),

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

        Action::DisableComputeService { .. }
        | Action::EnableComputeService { .. } => Some(ActionKind::EnableDisable),

        // Server lifecycle — treated as CUD for RBAC purposes
        Action::RebootServer { .. }
        | Action::StartServer { .. }
        | Action::StopServer { .. } => Some(ActionKind::Create),

        // Volume extend
        Action::ExtendVolume { .. } => Some(ActionKind::Create),

        // Attach / Associate (member-level)
        Action::AttachVolume { .. }
        | Action::AssociateFloatingIp { .. } => Some(ActionKind::Attach),

        // Detach / Disassociate (member-level)
        Action::DetachVolume { .. }
        | Action::DisassociateFloatingIp { .. } => Some(ActionKind::Detach),

        // Force operations (admin-only)
        Action::ForceDetachVolume { .. }
        | Action::ForceResetVolumeState { .. } => Some(ActionKind::ForceDelete),

        // Read / UI / System — no guard
        _ => None,
    }
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
        Action::FetchUsage { .. } => "FetchUsage",
        _ => "Unknown",
    }
}

async fn handle_action(registry: &AdapterRegistry, all_tenants: &AtomicBool, action: Action) -> Option<AppEvent> {
    let action_label = action_name(&action);
    tracing::info!(action = action_label, "handling action");
    let default_pagination = PaginationParams::default();
    let at = all_tenants.load(Ordering::Relaxed);

    match action {
        // -- Nova: Servers --------------------------------------------------
        Action::FetchServers => {
            match registry
                .nova
                .list_servers(&ServerListFilter { all_tenants: at, ..Default::default() }, &default_pagination)
                .await
            {
                Ok(resp) => Some(AppEvent::ServersLoaded(resp.items)),
                Err(e) => Some(api_error("FetchServers", e)),
            }
        }
        Action::CreateServer(params) => {
            match registry.nova.create_server(&params).await {
                Ok(server) => Some(AppEvent::ServerCreated(server)),
                Err(e) => Some(api_error("CreateServer", e)),
            }
        }
        Action::DeleteServer { id, name } => {
            match registry.nova.delete_server(&id).await {
                Ok(()) => Some(AppEvent::ServerDeleted { id, name }),
                Err(e) => Some(api_error("DeleteServer", e)),
            }
        }
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
        Action::StartServer { id } => {
            match registry.nova.start_server(&id).await {
                Ok(()) => Some(AppEvent::ServerStarted { id }),
                Err(e) => Some(api_error("StartServer", e)),
            }
        }
        Action::StopServer { id } => {
            match registry.nova.stop_server(&id).await {
                Ok(()) => Some(AppEvent::ServerStopped { id }),
                Err(e) => Some(api_error("StopServer", e)),
            }
        }
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
        Action::ConfirmResize { id } => {
            match registry.nova.confirm_migration(&id).await {
                Ok(()) => Some(AppEvent::ResizeConfirmed { id }),
                Err(e) => Some(api_error("ConfirmResize", e)),
            }
        }
        Action::RevertResize { id } => {
            match registry.nova.revert_migration(&id).await {
                Ok(()) => Some(AppEvent::ResizeReverted { id }),
                Err(e) => Some(api_error("RevertResize", e)),
            }
        }

        // -- Nova: Migration / Evacuate ------------------------------------
        Action::LiveMigrateServer { id, host } => {
            let params = LiveMigrateParams { host };
            match registry.nova.live_migrate_server(&id, &params).await {
                Ok(()) => Some(AppEvent::ServerLiveMigrated { id }),
                Err(e) => Some(api_error("LiveMigrateServer", e)),
            }
        }
        Action::ColdMigrateServer { id } => {
            match registry.nova.cold_migrate_server(&id).await {
                Ok(()) => Some(AppEvent::ServerColdMigrated { id }),
                Err(e) => Some(api_error("ColdMigrateServer", e)),
            }
        }
        Action::ConfirmMigration { id } => {
            match registry.nova.confirm_migration(&id).await {
                Ok(()) => Some(AppEvent::MigrationConfirmed { id }),
                Err(e) => Some(api_error("ConfirmMigration", e)),
            }
        }
        Action::RevertMigration { id } => {
            match registry.nova.revert_migration(&id).await {
                Ok(()) => Some(AppEvent::MigrationReverted { id }),
                Err(e) => Some(api_error("RevertMigration", e)),
            }
        }
        Action::EvacuateServer { id, params } => {
            match registry.nova.evacuate_server(&id, &params).await {
                Ok(()) => Some(AppEvent::ServerEvacuateResult { id, result: Ok(()) }),
                Err(e) => Some(AppEvent::ServerEvacuateResult {
                    id,
                    result: Err(e.to_string()),
                }),
            }
        }
        Action::DisableComputeService { service_id, hostname } => {
            match registry.nova.disable_compute_service(&service_id, None).await {
                Ok(_) => Some(AppEvent::ComputeServiceToggled { hostname, enabled: false }),
                Err(e) => Some(api_error("DisableComputeService", e)),
            }
        }
        Action::EnableComputeService { service_id, hostname } => {
            match registry.nova.enable_compute_service(&service_id).await {
                Ok(_) => Some(AppEvent::ComputeServiceToggled { hostname, enabled: true }),
                Err(e) => Some(api_error("EnableComputeService", e)),
            }
        }
        Action::FetchMigrationProgress { server_id } => {
            match registry.nova.list_server_migrations(&server_id).await {
                Ok(migrations) => {
                    migrations.into_iter().last().map(|migration| AppEvent::MigrationProgressLoaded { server_id, migration })
                }
                Err(e) => Some(api_error("FetchMigrationProgress", e)),
            }
        }

        // -- Nova: Usage ---------------------------------------------------
        Action::FetchUsage { start, end } => {
            use crate::port::error::ApiError;
            let start_dt = start.parse::<DateTime<Utc>>().map_err(|e| ApiError::Parse(e.to_string()));
            let end_dt = end.parse::<DateTime<Utc>>().map_err(|e| ApiError::Parse(e.to_string()));
            match (start_dt, end_dt) {
                (Ok(s), Ok(e)) => {
                    match registry.nova.list_all_tenant_usage(s, e).await {
                        Ok(usages) => Some(AppEvent::UsageLoaded(usages)),
                        Err(e) => Some(api_error("FetchUsage", e)),
                    }
                }
                (Err(e), _) | (_, Err(e)) => Some(api_error("FetchUsage", e)),
            }
        }

        // -- Nova: Flavors --------------------------------------------------
        Action::FetchFlavors => {
            match registry.nova.list_flavors(&default_pagination).await {
                Ok(resp) => Some(AppEvent::FlavorsLoaded(resp.items)),
                Err(e) => Some(api_error("FetchFlavors", e)),
            }
        }
        Action::CreateFlavor(params) => {
            match registry.nova.create_flavor(&params).await {
                Ok(flavor) => Some(AppEvent::FlavorCreated(flavor)),
                Err(e) => Some(api_error("CreateFlavor", e)),
            }
        }
        Action::DeleteFlavor { id } => {
            match registry.nova.delete_flavor(&id).await {
                Ok(()) => Some(AppEvent::FlavorDeleted { id }),
                Err(e) => Some(api_error("DeleteFlavor", e)),
            }
        }

        // -- Nova: Admin ----------------------------------------------------
        Action::FetchAggregates => {
            match registry.nova.list_aggregates().await {
                Ok(aggs) => Some(AppEvent::AggregatesLoaded(aggs)),
                Err(e) => Some(api_error("FetchAggregates", e)),
            }
        }
        Action::FetchComputeServices => {
            match registry.nova.list_compute_services().await {
                Ok(svcs) => Some(AppEvent::ComputeServicesLoaded(svcs)),
                Err(e) => Some(api_error("FetchComputeServices", e)),
            }
        }
        Action::FetchHypervisors => {
            match registry.nova.list_hypervisors().await {
                Ok(hvs) => Some(AppEvent::HypervisorsLoaded(hvs)),
                Err(e) => Some(api_error("FetchHypervisors", e)),
            }
        }

        // -- Neutron: Networks ----------------------------------------------
        Action::FetchNetworks => {
            match registry
                .neutron
                .list_networks(&NetworkListFilter { all_tenants: at }, &default_pagination)
                .await
            {
                Ok(resp) => Some(AppEvent::NetworksLoaded(resp.items)),
                Err(e) => Some(api_error("FetchNetworks", e)),
            }
        }
        Action::CreateNetwork(params) => {
            match registry.neutron.create_network(&params).await {
                Ok(net) => Some(AppEvent::NetworkCreated(net)),
                Err(e) => Some(api_error("CreateNetwork", e)),
            }
        }
        Action::FetchSubnets { network_id } => {
            match registry
                .neutron
                .list_subnets(Some(&network_id))
                .await
            {
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
                .list_security_groups(&SecurityGroupListFilter { all_tenants: at }, &default_pagination)
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
            match registry
                .neutron
                .delete_security_group_rule(&rule_id)
                .await
            {
                Ok(()) => Some(AppEvent::SecurityGroupRuleDeleted { rule_id }),
                Err(e) => Some(api_error("DeleteSecurityGroupRule", e)),
            }
        }

        // -- Neutron: Floating IPs ------------------------------------------
        Action::FetchFloatingIps => {
            match registry
                .neutron
                .list_floating_ips(&FloatingIpListFilter { all_tenants: at }, &default_pagination)
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
        Action::DeleteFloatingIp { id } => {
            match registry.neutron.delete_floating_ip(&id).await {
                Ok(()) => Some(AppEvent::FloatingIpDeleted { id }),
                Err(e) => Some(api_error("DeleteFloatingIp", e)),
            }
        }
        Action::FetchAgents => {
            match registry.neutron.list_network_agents().await {
                Ok(agents) => Some(AppEvent::AgentsLoaded(agents)),
                Err(e) => Some(api_error("FetchAgents", e)),
            }
        }

        // -- Cinder: Volumes ------------------------------------------------
        Action::FetchVolumes => {
            match registry
                .cinder
                .list_volumes(&VolumeListFilter { all_tenants: at, ..Default::default() }, &default_pagination)
                .await
            {
                Ok(resp) => Some(AppEvent::VolumesLoaded(resp.items)),
                Err(e) => Some(api_error("FetchVolumes", e)),
            }
        }
        Action::CreateVolume(params) => {
            match registry.cinder.create_volume(&params).await {
                Ok(vol) => Some(AppEvent::VolumeCreated(vol)),
                Err(e) => Some(api_error("CreateVolume", e)),
            }
        }
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
        Action::CreateSnapshot(params) => {
            match registry.cinder.create_snapshot(&params).await {
                Ok(snap) => Some(AppEvent::SnapshotCreated(snap)),
                Err(e) => Some(api_error("CreateSnapshot", e)),
            }
        }
        Action::DeleteSnapshot { id } => {
            match registry.cinder.delete_snapshot(&id).await {
                Ok(()) => Some(AppEvent::SnapshotDeleted { id }),
                Err(e) => Some(api_error("DeleteSnapshot", e)),
            }
        }

        // -- Glance: Images -------------------------------------------------
        Action::FetchImages => {
            match registry
                .glance
                .list_images(&ImageListFilter { all_tenants: at, ..Default::default() }, &default_pagination)
                .await
            {
                Ok(resp) => Some(AppEvent::ImagesLoaded(resp.items)),
                Err(e) => Some(api_error("FetchImages", e)),
            }
        }
        Action::CreateImage(params) => {
            match registry.glance.create_image(&params).await {
                Ok(img) => Some(AppEvent::ImageCreated(img)),
                Err(e) => Some(api_error("CreateImage", e)),
            }
        }
        Action::DeleteImage { id } => {
            match registry.glance.delete_image(&id).await {
                Ok(()) => Some(AppEvent::ImageDeleted { id }),
                Err(e) => Some(api_error("DeleteImage", e)),
            }
        }

        // -- Keystone: Projects ---------------------------------------------
        Action::FetchProjects => {
            match registry
                .keystone
                .list_projects(&default_pagination)
                .await
            {
                Ok(resp) => Some(AppEvent::ProjectsLoaded(resp.items)),
                Err(e) => Some(api_error("FetchProjects", e)),
            }
        }
        Action::CreateProject(params) => {
            match registry.keystone.create_project(&params).await {
                Ok(proj) => Some(AppEvent::ProjectCreated(proj)),
                Err(e) => Some(api_error("CreateProject", e)),
            }
        }
        Action::DeleteProject { id } => {
            match registry.keystone.delete_project(&id).await {
                Ok(()) => Some(AppEvent::ProjectDeleted { id }),
                Err(e) => Some(api_error("DeleteProject", e)),
            }
        }

        // -- Keystone: Users ------------------------------------------------
        Action::FetchUsers => {
            match registry.keystone.list_users(&default_pagination).await {
                Ok(resp) => Some(AppEvent::UsersLoaded(resp.items)),
                Err(e) => Some(api_error("FetchUsers", e)),
            }
        }
        Action::CreateUser(params) => {
            match registry.keystone.create_user(&params).await {
                Ok(user) => Some(AppEvent::UserCreated(user)),
                Err(e) => Some(api_error("CreateUser", e)),
            }
        }
        Action::DeleteUser { id } => {
            match registry.keystone.delete_user(&id).await {
                Ok(()) => Some(AppEvent::UserDeleted { id }),
                Err(e) => Some(api_error("DeleteUser", e)),
            }
        }

        // -- Nova: Volume Attach/Detach (via Nova os-volume_attachments API) --
        Action::AttachVolume { volume_id, server_id, device } => {
            match registry.nova.attach_volume(&server_id, &volume_id, device.as_deref()).await {
                Ok(()) => Some(AppEvent::VolumeAttached { volume_id, server_id }),
                Err(e) => Some(api_error("AttachVolume", e)),
            }
        }
        Action::DetachVolume { volume_id, server_id, .. } => {
            match registry.nova.detach_volume(&server_id, &volume_id).await {
                Ok(()) => Some(AppEvent::VolumeDetached { volume_id }),
                Err(e) => Some(api_error("DetachVolume", e)),
            }
        }
        Action::ForceDetachVolume { volume_id, attachment_id, .. } => {
            match registry.cinder.force_detach_volume(&volume_id, &attachment_id).await {
                Ok(()) => Some(AppEvent::VolumeForceDetached { volume_id }),
                Err(e) => Some(api_error("ForceDetachVolume", e)),
            }
        }
        Action::ForceResetVolumeState { volume_id, target_state } => {
            match registry.cinder.force_set_volume_state(&volume_id, &target_state).await {
                Ok(()) => Some(AppEvent::VolumeStateReset { volume_id }),
                Err(e) => Some(api_error("ForceResetVolumeState", e)),
            }
        }

        // -- Neutron: Floating IP Associate/Disassociate --------------------
        Action::AssociateFloatingIp { fip_id, port_id } => {
            match registry.neutron.associate_floating_ip(&fip_id, &port_id).await {
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
        Action::FetchPorts { server_id } => {
            match registry.neutron.list_ports(&server_id).await {
                Ok(ports) => Some(AppEvent::PortsLoaded { server_id, ports }),
                Err(e) => Some(api_error("FetchPorts", e)),
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

        Action::SwitchCloud(_cloud_name) => {
            // Legacy stub from Phase 1. Superseded by Action::SwitchContext
            // (BL-P2-031). Kept until callers migrate.
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
        Action::LiveMigrateServer { id, .. }
        | Action::ColdMigrateServer { id, .. } => Some(id.clone()),
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
                    AppEvent::ServerStatusPolled { server: server.clone() },
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
        AppEvent::MigrationPollingStopped { server_id: server_id.to_string() },
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_action_to_kind_cud_actions() {
        use crate::infra::rbac::ActionKind;
        // Create actions should map to ActionKind::Create
        assert_eq!(
            action_to_kind(&Action::CreateServer(crate::port::types::ServerCreateParams {
                name: "t".into(), image_id: "i".into(), flavor_id: "f".into(),
                networks: vec![], security_groups: None, key_name: None, availability_zone: None,
            })),
            Some(ActionKind::Create),
        );
        // Delete actions should map to ActionKind::Delete
        assert_eq!(
            action_to_kind(&Action::DeleteServer { id: "s1".into(), name: "web".into() }),
            Some(ActionKind::Delete),
        );
        // ForceDelete
        assert_eq!(
            action_to_kind(&Action::DeleteVolume { id: "v1".into(), force: true }),
            Some(ActionKind::ForceDelete),
        );
        // Fetch actions should return None (no guard needed)
        assert_eq!(action_to_kind(&Action::FetchServers), None);

        // Migration actions should map to Migrate
        assert_eq!(
            action_to_kind(&Action::LiveMigrateServer { id: "s1".into(), host: None }),
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
            action_to_kind(&Action::EvacuateServer { id: "s1".into(), params: EvacuateParams::default() }),
            Some(ActionKind::Evacuate),
        );
        // Disable/Enable should map to EnableDisable (admin-only)
        assert_eq!(
            action_to_kind(&Action::DisableComputeService { service_id: "svc-1".into(), hostname: "h1".into() }),
            Some(ActionKind::EnableDisable),
        );
        assert_eq!(
            action_to_kind(&Action::EnableComputeService { service_id: "svc-1".into(), hostname: "h1".into() }),
            Some(ActionKind::EnableDisable),
        );
        // FetchMigrationProgress is read-only
        assert_eq!(
            action_to_kind(&Action::FetchMigrationProgress { server_id: "s1".into() }),
            None,
        );
    }

    #[test]
    fn test_action_to_kind_resize_actions() {
        // Resize actions should map to ActionKind::Resize (member-level)
        assert_eq!(
            action_to_kind(&Action::ResizeServer { id: "s1".into(), flavor_id: "f2".into() }),
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
        let event = AppEvent::PermissionDenied { operation: "CreateServer".into() };
        match event {
            AppEvent::PermissionDenied { operation } => assert_eq!(operation, "CreateServer"),
            _ => panic!("expected PermissionDenied"),
        }
    }

    #[test]
    fn test_resize_actions_trigger_status_polling() {
        // ResizeServer, ConfirmResize, RevertResize should all be identified
        // as actions requiring server status polling
        let resize = Action::ResizeServer { id: "s1".into(), flavor_id: "f2".into() };
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
        use std::sync::{Arc, Mutex};
        use std::collections::HashSet;

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
            poll_migration_server_id(&Action::LiveMigrateServer { id: "s2".into(), host: None }),
            Some("s2".to_string()),
        );
        // FetchServers should not trigger
        assert_eq!(poll_migration_server_id(&Action::FetchServers), None);
    }

    #[test]
    fn test_reboot_start_stop_trigger_status_polling() {
        let reboot = Action::RebootServer { id: "s1".into(), hard: false };
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
        assert_eq!(fetch_dedup_key(&Action::FetchNetworks), Some("FetchNetworks"));
        assert_eq!(fetch_dedup_key(&Action::FetchImages), Some("FetchImages"));
        assert_eq!(fetch_dedup_key(&Action::FetchFlavors), Some("FetchFlavors"));
        assert_eq!(fetch_dedup_key(&Action::FetchSnapshots), Some("FetchSnapshots"));
        assert_eq!(fetch_dedup_key(&Action::FetchFloatingIps), Some("FetchFloatingIps"));
        assert_eq!(fetch_dedup_key(&Action::FetchSecurityGroups), Some("FetchSecurityGroups"));
        assert_eq!(fetch_dedup_key(&Action::FetchProjects), Some("FetchProjects"));
        assert_eq!(fetch_dedup_key(&Action::FetchUsers), Some("FetchUsers"));
        assert_eq!(fetch_dedup_key(&Action::FetchAggregates), Some("FetchAggregates"));
        assert_eq!(fetch_dedup_key(&Action::FetchComputeServices), Some("FetchComputeServices"));
        assert_eq!(fetch_dedup_key(&Action::FetchHypervisors), Some("FetchHypervisors"));
        assert_eq!(fetch_dedup_key(&Action::FetchAgents), Some("FetchAgents"));
    }

    #[test]
    fn test_fetch_dedup_key_returns_none_for_mutations() {
        assert_eq!(fetch_dedup_key(&Action::DeleteServer { id: "s1".into(), name: "w1".into() }), None);
        assert_eq!(fetch_dedup_key(&Action::RebootServer { id: "s1".into(), hard: false }), None);
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
