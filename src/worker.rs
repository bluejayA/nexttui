//! Background worker: consumes Actions from the UI, calls OpenStack APIs,
//! and sends AppEvents back to the event loop for UI updates.

use std::sync::Arc;

use tokio::sync::mpsc;

use crate::action::Action;
use crate::adapter::registry::AdapterRegistry;
use crate::event::AppEvent;
use crate::port::types::*;

/// Run the background worker loop.
/// Receives Actions from `action_rx`, calls the appropriate API via `registry`,
/// and sends resulting AppEvents to `event_tx`.
pub async fn run_worker(
    registry: Arc<AdapterRegistry>,
    mut action_rx: mpsc::UnboundedReceiver<Action>,
    event_tx: mpsc::UnboundedSender<AppEvent>,
) {
    while let Some(action) = action_rx.recv().await {
        let registry = registry.clone();
        let event_tx = event_tx.clone();

        tokio::spawn(async move {
            let action_name = format!("{:?}", action).chars().take(60).collect::<String>();
            eprintln!("[worker] Processing: {action_name}");
            let event = handle_action(&registry, action).await;
            match &event {
                Some(AppEvent::ApiError { operation, message }) => {
                    eprintln!("[worker] ERROR {operation}: {message}");
                }
                Some(ev) => {
                    eprintln!("[worker] OK: {:?}", std::mem::discriminant(ev));
                }
                None => {}
            }
            if let Some(ev) = event {
                let _ = event_tx.send(ev);
            }
        });
    }
}

async fn handle_action(registry: &AdapterRegistry, action: Action) -> Option<AppEvent> {
    let default_pagination = PaginationParams::default();

    match action {
        // -- Nova: Servers --------------------------------------------------
        Action::FetchServers => {
            match registry
                .nova
                .list_servers(&ServerListFilter::default(), &default_pagination)
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
                .list_networks(&default_pagination)
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
                .list_security_groups(&default_pagination)
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
                .list_floating_ips(&default_pagination)
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
                .list_volumes(&VolumeListFilter::default(), &default_pagination)
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
                .list_snapshots(&default_pagination)
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
                .list_images(&ImageListFilter::default(), &default_pagination)
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

fn api_error(operation: &str, error: crate::port::error::ApiError) -> AppEvent {
    AppEvent::ApiError {
        operation: operation.to_string(),
        message: error.to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

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
