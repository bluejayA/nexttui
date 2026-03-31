//! Background worker: consumes Actions from the UI, calls OpenStack APIs,
//! and sends AppEvents back to the event loop for UI updates.

use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

use tokio::sync::mpsc;
use tracing::Instrument;

use crate::action::Action;
use crate::adapter::registry::AdapterRegistry;
use crate::event::AppEvent;
use crate::infra::rbac::{ActionKind, RbacGuard};
use crate::port::types::*;

/// Run the background worker loop.
/// Receives Actions from `action_rx`, calls the appropriate API via `registry`,
/// and sends resulting AppEvents to `event_tx`.
#[tracing::instrument(skip_all)]
pub async fn run_worker(
    registry: Arc<AdapterRegistry>,
    rbac: Arc<RbacGuard>,
    all_tenants: Arc<AtomicBool>,
    mut action_rx: mpsc::UnboundedReceiver<Action>,
    event_tx: mpsc::UnboundedSender<AppEvent>,
) {
    while let Some(action) = action_rx.recv().await {
        // RBAC guard: check CUD permissions before API call
        if let Some(kind) = action_to_kind(&action)
            && !rbac.can_perform(kind)
        {
            let _ = event_tx.send(AppEvent::PermissionDenied {
                operation: action_name(&action).to_string(),
            });
            continue;
        }

        let registry = registry.clone();
        let event_tx = event_tx.clone();
        let all_tenants = all_tenants.clone();

        let poll_server_id = match &action {
            Action::LiveMigrateServer { id, .. } => Some(id.clone()),
            _ => None,
        };

        let span = tracing::info_span!("worker_task", action = action_name(&action));
        tokio::spawn(
            async move {
                let event = handle_action(&registry, &all_tenants, action).await;
                let success = event.as_ref().is_some_and(|ev| !matches!(ev, AppEvent::ApiError { .. }));
                if let Some(ev) = event {
                    let _ = event_tx.send(ev);
                }
                // Start polling after successful live migration
                if success {
                    if let Some(server_id) = poll_server_id {
                        poll_migration_progress(&registry, &event_tx, &server_id).await;
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

        // Server lifecycle — treated as CUD for RBAC purposes
        Action::RebootServer { .. }
        | Action::StartServer { .. }
        | Action::StopServer { .. } => Some(ActionKind::Create),

        // Volume extend
        Action::ExtendVolume { .. } => Some(ActionKind::Create),

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
        Action::FetchMigrationProgress { .. } => "FetchMigrationProgress",
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
        Action::EvacuateServer { id, host } => {
            let params = EvacuateParams { host };
            match registry.nova.evacuate_server(&id, &params).await {
                Ok(()) => Some(AppEvent::ServerEvacuated { id }),
                Err(e) => Some(api_error("EvacuateServer", e)),
            }
        }
        Action::FetchMigrationProgress { server_id } => {
            match registry.nova.list_server_migrations(&server_id).await {
                Ok(migrations) => {
                    if let Some(migration) = migrations.into_iter().last() {
                        Some(AppEvent::MigrationProgressLoaded { server_id, migration })
                    } else {
                        None
                    }
                }
                Err(e) => Some(api_error("FetchMigrationProgress", e)),
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

        // -- UI-only actions (handled by App::dispatch_action, not worker) --
        Action::Navigate(_)
        | Action::Back
        | Action::FocusSidebar
        | Action::SelectResource { .. }
        | Action::NavigateToResource { .. }
        | Action::EnterFormMode
        | Action::ExitFormMode
        | Action::ToggleAllTenants
        | Action::Quit => None,

        // -- System ---------------------------------------------------------
        Action::RefreshAll => {
            // RefreshAll is not handled by the worker — App::dispatch_action should
            // expand it into individual Fetch actions. If it reaches here, ignore.
            None
        }

        Action::SwitchCloud(_cloud_name) => {
            // Phase 2: switch auth provider and re-create adapters
            None
        }
    }
}

/// Poll migration progress every 2 seconds until completed or error.
async fn poll_migration_progress(
    registry: &AdapterRegistry,
    event_tx: &mpsc::UnboundedSender<AppEvent>,
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
                    let _ = event_tx.send(AppEvent::MigrationProgressLoaded {
                        server_id: server_id.to_string(),
                        migration,
                    });
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
    let _ = event_tx.send(AppEvent::MigrationPollingStopped {
        server_id: server_id.to_string(),
    });
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
            action_to_kind(&Action::EvacuateServer { id: "s1".into(), host: None }),
            Some(ActionKind::Evacuate),
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
}
