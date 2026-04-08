use std::collections::HashMap;

use async_trait::async_trait;
use chrono::{DateTime, Utc};

use super::error::{ApiError, ApiResult};
use super::types::*;
use crate::models::cinder::{Volume, VolumeSnapshot};
use crate::models::glance::Image;
use crate::models::keystone::{Project, Role, RoleAssignment, User};
use crate::models::neutron::{FloatingIp, Network, NetworkAgent, Port, SecurityGroup, SecurityGroupRule};
use crate::models::nova::{Aggregate, ComputeService, Flavor, Hypervisor, Server};

// ============================================================
// MockNovaAdapter
// ============================================================

pub struct MockNovaAdapter;

#[async_trait]
impl super::nova::NovaPort for MockNovaAdapter {
    async fn list_servers(
        &self,
        _filter: &ServerListFilter,
        _pagination: &PaginationParams,
    ) -> ApiResult<PaginatedResponse<Server>> {
        Ok(PaginatedResponse::empty())
    }
    async fn get_server(&self, server_id: &str) -> ApiResult<Server> {
        Err(ApiError::NotFound {
            resource_type: "server".into(),
            id: server_id.into(),
        })
    }
    async fn create_server(&self, _params: &ServerCreateParams) -> ApiResult<Server> {
        Err(ApiError::BadRequest("mock: not implemented".into()))
    }
    async fn delete_server(&self, _server_id: &str) -> ApiResult<()> {
        Ok(())
    }
    async fn reboot_server(&self, _server_id: &str, _reboot_type: RebootType) -> ApiResult<()> {
        Ok(())
    }
    async fn start_server(&self, _server_id: &str) -> ApiResult<()> {
        Ok(())
    }
    async fn stop_server(&self, _server_id: &str) -> ApiResult<()> {
        Ok(())
    }
    async fn force_set_server_state(&self, _server_id: &str, _state: ServerState) -> ApiResult<()> {
        Ok(())
    }
    async fn create_server_snapshot(
        &self,
        _server_id: &str,
        _image_name: &str,
    ) -> ApiResult<String> {
        Ok("mock-image-id".into())
    }
    async fn list_server_events(&self, _server_id: &str) -> ApiResult<Vec<ServerEvent>> {
        Ok(vec![])
    }
    async fn resize_server(&self, _server_id: &str, _flavor_id: &str) -> ApiResult<()> {
        Ok(())
    }
    async fn live_migrate_server(
        &self,
        _server_id: &str,
        _params: &LiveMigrateParams,
    ) -> ApiResult<()> {
        Ok(())
    }
    async fn cold_migrate_server(&self, _server_id: &str) -> ApiResult<()> {
        Ok(())
    }
    async fn confirm_migration(&self, _server_id: &str) -> ApiResult<()> {
        Ok(())
    }
    async fn revert_migration(&self, _server_id: &str) -> ApiResult<()> {
        Ok(())
    }
    async fn evacuate_server(&self, _server_id: &str, _params: &EvacuateParams) -> ApiResult<()> {
        Ok(())
    }
    async fn list_server_migrations(
        &self,
        _server_id: &str,
    ) -> ApiResult<Vec<crate::models::nova::ServerMigration>> {
        Ok(vec![])
    }
    async fn get_server_migration(
        &self,
        _server_id: &str,
        migration_id: i64,
    ) -> ApiResult<crate::models::nova::ServerMigration> {
        Err(ApiError::NotFound {
            resource_type: "migration".into(),
            id: migration_id.to_string(),
        })
    }
    async fn list_flavors(
        &self,
        _pagination: &PaginationParams,
    ) -> ApiResult<PaginatedResponse<Flavor>> {
        Ok(PaginatedResponse::empty())
    }
    async fn get_flavor(&self, flavor_id: &str) -> ApiResult<Flavor> {
        Err(ApiError::NotFound {
            resource_type: "flavor".into(),
            id: flavor_id.into(),
        })
    }
    async fn create_flavor(&self, _params: &FlavorCreateParams) -> ApiResult<Flavor> {
        Err(ApiError::BadRequest("mock: not implemented".into()))
    }
    async fn delete_flavor(&self, _flavor_id: &str) -> ApiResult<()> {
        Ok(())
    }
    async fn list_aggregates(&self) -> ApiResult<Vec<Aggregate>> {
        Ok(vec![])
    }
    async fn get_aggregate(&self, _aggregate_id: i64) -> ApiResult<Aggregate> {
        Err(ApiError::BadRequest("mock: not implemented".into()))
    }
    async fn create_aggregate(&self, _params: &AggregateCreateParams) -> ApiResult<Aggregate> {
        Err(ApiError::BadRequest("mock: not implemented".into()))
    }
    async fn update_aggregate(
        &self,
        _aggregate_id: i64,
        _params: &AggregateUpdateParams,
    ) -> ApiResult<Aggregate> {
        Err(ApiError::BadRequest("mock: not implemented".into()))
    }
    async fn delete_aggregate(&self, _aggregate_id: i64) -> ApiResult<()> {
        Ok(())
    }
    async fn aggregate_add_host(&self, _aggregate_id: i64, _host: &str) -> ApiResult<Aggregate> {
        Err(ApiError::BadRequest("mock: not implemented".into()))
    }
    async fn aggregate_remove_host(&self, _aggregate_id: i64, _host: &str) -> ApiResult<Aggregate> {
        Err(ApiError::BadRequest("mock: not implemented".into()))
    }
    async fn aggregate_set_metadata(
        &self,
        _aggregate_id: i64,
        _metadata: &HashMap<String, String>,
    ) -> ApiResult<Aggregate> {
        Err(ApiError::BadRequest("mock: not implemented".into()))
    }
    async fn list_compute_services(&self) -> ApiResult<Vec<ComputeService>> {
        Ok(vec![])
    }
    async fn enable_compute_service(&self, _service_id: &str) -> ApiResult<ComputeService> {
        Err(ApiError::BadRequest("mock: not implemented".into()))
    }
    async fn disable_compute_service(
        &self,
        _service_id: &str,
        _reason: Option<&str>,
    ) -> ApiResult<ComputeService> {
        Err(ApiError::BadRequest("mock: not implemented".into()))
    }
    async fn attach_volume(
        &self,
        _server_id: &str,
        _volume_id: &str,
        _device: Option<&str>,
    ) -> ApiResult<()> {
        Ok(())
    }
    async fn detach_volume(&self, _server_id: &str, _volume_id: &str) -> ApiResult<()> {
        Ok(())
    }
    async fn list_hypervisors(&self) -> ApiResult<Vec<Hypervisor>> {
        Ok(vec![])
    }
    async fn get_hypervisor(&self, _hypervisor_id: &str) -> ApiResult<Hypervisor> {
        Err(ApiError::BadRequest("mock: not implemented".into()))
    }
    async fn get_project_usage(
        &self,
        _project_id: &str,
        _start: DateTime<Utc>,
        _end: DateTime<Utc>,
    ) -> ApiResult<ProjectUsage> {
        Ok(ProjectUsage {
            total_vcpus_usage: 0.0,
            total_memory_mb_usage: 0.0,
            total_local_gb_usage: 0.0,
        })
    }
    async fn get_compute_quota(&self, _project_id: &str) -> ApiResult<ComputeQuota> {
        Ok(ComputeQuota {
            cores: 20,
            ram: 51200,
            instances: 10,
        })
    }
    async fn update_compute_quota(
        &self,
        _project_id: &str,
        _params: &ComputeQuotaUpdateParams,
    ) -> ApiResult<ComputeQuota> {
        Ok(ComputeQuota {
            cores: 20,
            ram: 51200,
            instances: 10,
        })
    }
}

// ============================================================
// MockNeutronAdapter
// ============================================================

pub struct MockNeutronAdapter;

#[async_trait]
impl super::neutron::NeutronPort for MockNeutronAdapter {
    async fn list_networks(
        &self,
        _filter: &NetworkListFilter,
        _pagination: &PaginationParams,
    ) -> ApiResult<PaginatedResponse<Network>> {
        Ok(PaginatedResponse::empty())
    }
    async fn get_network(&self, network_id: &str) -> ApiResult<Network> {
        Err(ApiError::NotFound {
            resource_type: "network".into(),
            id: network_id.into(),
        })
    }
    async fn create_network(&self, _params: &NetworkCreateParams) -> ApiResult<Network> {
        Err(ApiError::BadRequest("mock: not implemented".into()))
    }
    async fn update_network(
        &self,
        _network_id: &str,
        _params: &NetworkUpdateParams,
    ) -> ApiResult<Network> {
        Err(ApiError::BadRequest("mock: not implemented".into()))
    }
    async fn delete_network(&self, _network_id: &str) -> ApiResult<()> {
        Ok(())
    }
    async fn list_subnets(&self, _network_id: Option<&str>) -> ApiResult<Vec<Subnet>> {
        Ok(vec![])
    }
    async fn list_security_groups(
        &self,
        _filter: &SecurityGroupListFilter,
        _pagination: &PaginationParams,
    ) -> ApiResult<PaginatedResponse<SecurityGroup>> {
        Ok(PaginatedResponse::empty())
    }
    async fn get_security_group(&self, sg_id: &str) -> ApiResult<SecurityGroup> {
        Err(ApiError::NotFound {
            resource_type: "security_group".into(),
            id: sg_id.into(),
        })
    }
    async fn create_security_group(
        &self,
        _params: &SecurityGroupCreateParams,
    ) -> ApiResult<SecurityGroup> {
        Err(ApiError::BadRequest("mock: not implemented".into()))
    }
    async fn update_security_group(
        &self,
        _sg_id: &str,
        _params: &SecurityGroupUpdateParams,
    ) -> ApiResult<SecurityGroup> {
        Err(ApiError::BadRequest("mock: not implemented".into()))
    }
    async fn delete_security_group(&self, _sg_id: &str) -> ApiResult<()> {
        Ok(())
    }
    async fn create_security_group_rule(
        &self,
        _params: &SecurityGroupRuleCreateParams,
    ) -> ApiResult<SecurityGroupRule> {
        Err(ApiError::BadRequest("mock: not implemented".into()))
    }
    async fn delete_security_group_rule(&self, _rule_id: &str) -> ApiResult<()> {
        Ok(())
    }
    async fn list_floating_ips(
        &self,
        _filter: &FloatingIpListFilter,
        _pagination: &PaginationParams,
    ) -> ApiResult<PaginatedResponse<FloatingIp>> {
        Ok(PaginatedResponse::empty())
    }
    async fn create_floating_ip(&self, _params: &FloatingIpCreateParams) -> ApiResult<FloatingIp> {
        Err(ApiError::BadRequest("mock: not implemented".into()))
    }
    async fn delete_floating_ip(&self, _fip_id: &str) -> ApiResult<()> {
        Ok(())
    }
    async fn associate_floating_ip(&self, _fip_id: &str, _port_id: &str) -> ApiResult<FloatingIp> {
        Err(ApiError::BadRequest("mock: not implemented".into()))
    }
    async fn disassociate_floating_ip(&self, _fip_id: &str) -> ApiResult<FloatingIp> {
        Err(ApiError::BadRequest("mock: not implemented".into()))
    }
    async fn list_ports(&self, _device_id: &str) -> ApiResult<Vec<Port>> {
        Ok(vec![])
    }
    async fn list_network_agents(&self) -> ApiResult<Vec<NetworkAgent>> {
        Ok(vec![])
    }
    async fn enable_network_agent(&self, _agent_id: &str) -> ApiResult<NetworkAgent> {
        Err(ApiError::BadRequest("mock: not implemented".into()))
    }
    async fn disable_network_agent(&self, _agent_id: &str) -> ApiResult<NetworkAgent> {
        Err(ApiError::BadRequest("mock: not implemented".into()))
    }
    async fn delete_network_agent(&self, _agent_id: &str) -> ApiResult<()> {
        Ok(())
    }
}

// ============================================================
// MockCinderAdapter
// ============================================================

pub struct MockCinderAdapter;

#[async_trait]
impl super::cinder::CinderPort for MockCinderAdapter {
    async fn list_volumes(
        &self,
        _filter: &VolumeListFilter,
        _pagination: &PaginationParams,
    ) -> ApiResult<PaginatedResponse<Volume>> {
        Ok(PaginatedResponse::empty())
    }
    async fn get_volume(&self, volume_id: &str) -> ApiResult<Volume> {
        Err(ApiError::NotFound {
            resource_type: "volume".into(),
            id: volume_id.into(),
        })
    }
    async fn create_volume(&self, _params: &VolumeCreateParams) -> ApiResult<Volume> {
        Err(ApiError::BadRequest("mock: not implemented".into()))
    }
    async fn delete_volume(&self, _volume_id: &str) -> ApiResult<()> {
        Ok(())
    }
    async fn force_delete_volume(&self, _volume_id: &str) -> ApiResult<()> {
        Ok(())
    }
    async fn extend_volume(&self, _volume_id: &str, _new_size_gb: u32) -> ApiResult<()> {
        Ok(())
    }
    async fn attach_volume(
        &self,
        _volume_id: &str,
        _server_id: &str,
        _device: Option<&str>,
    ) -> ApiResult<()> {
        Ok(())
    }
    async fn detach_volume(&self, _volume_id: &str, _attachment_id: &str) -> ApiResult<()> {
        Ok(())
    }
    async fn force_detach_volume(&self, _volume_id: &str, _attachment_id: &str) -> ApiResult<()> {
        Ok(())
    }
    async fn force_set_volume_state(&self, _volume_id: &str, _state: &str) -> ApiResult<()> {
        Ok(())
    }
    async fn migrate_volume(
        &self,
        _volume_id: &str,
        _dest_host: &str,
        _force_host_copy: bool,
    ) -> ApiResult<()> {
        Ok(())
    }
    async fn list_snapshots(
        &self,
        _filter: &SnapshotListFilter,
        _pagination: &PaginationParams,
    ) -> ApiResult<PaginatedResponse<VolumeSnapshot>> {
        Ok(PaginatedResponse::empty())
    }
    async fn get_snapshot(&self, snapshot_id: &str) -> ApiResult<VolumeSnapshot> {
        Err(ApiError::NotFound {
            resource_type: "snapshot".into(),
            id: snapshot_id.into(),
        })
    }
    async fn create_snapshot(&self, _params: &SnapshotCreateParams) -> ApiResult<VolumeSnapshot> {
        Err(ApiError::BadRequest("mock: not implemented".into()))
    }
    async fn delete_snapshot(&self, _snapshot_id: &str) -> ApiResult<()> {
        Ok(())
    }
    async fn list_qos_specs(&self) -> ApiResult<Vec<QosSpec>> {
        Ok(vec![])
    }
    async fn get_qos_spec(&self, _qos_id: &str) -> ApiResult<QosSpec> {
        Err(ApiError::BadRequest("mock: not implemented".into()))
    }
    async fn create_qos_spec(&self, _params: &QosCreateParams) -> ApiResult<QosSpec> {
        Err(ApiError::BadRequest("mock: not implemented".into()))
    }
    async fn update_qos_spec(
        &self,
        _qos_id: &str,
        _specs: &HashMap<String, String>,
    ) -> ApiResult<QosSpec> {
        Err(ApiError::BadRequest("mock: not implemented".into()))
    }
    async fn delete_qos_spec(&self, _qos_id: &str) -> ApiResult<()> {
        Ok(())
    }
    async fn list_storage_pools(&self, _detail: bool) -> ApiResult<Vec<StoragePool>> {
        Ok(vec![])
    }
    async fn get_volume_quota(&self, _project_id: &str) -> ApiResult<VolumeQuota> {
        Ok(VolumeQuota {
            volumes: 10,
            gigabytes: 1000,
            snapshots: 10,
        })
    }
    async fn update_volume_quota(
        &self,
        _project_id: &str,
        _params: &VolumeQuotaUpdateParams,
    ) -> ApiResult<VolumeQuota> {
        Ok(VolumeQuota {
            volumes: 10,
            gigabytes: 1000,
            snapshots: 10,
        })
    }
}

// ============================================================
// MockGlanceAdapter
// ============================================================

pub struct MockGlanceAdapter;

#[async_trait]
impl super::glance::GlancePort for MockGlanceAdapter {
    async fn list_images(
        &self,
        _filter: &ImageListFilter,
        _pagination: &PaginationParams,
    ) -> ApiResult<PaginatedResponse<Image>> {
        Ok(PaginatedResponse::empty())
    }
    async fn get_image(&self, image_id: &str) -> ApiResult<Image> {
        Err(ApiError::NotFound {
            resource_type: "image".into(),
            id: image_id.into(),
        })
    }
    async fn create_image(&self, _params: &ImageCreateParams) -> ApiResult<Image> {
        Err(ApiError::BadRequest("mock: not implemented".into()))
    }
    async fn update_image(&self, _image_id: &str, _params: &ImageUpdateParams) -> ApiResult<Image> {
        Err(ApiError::BadRequest("mock: not implemented".into()))
    }
    async fn delete_image(&self, _image_id: &str) -> ApiResult<()> {
        Ok(())
    }
}

// ============================================================
// MockKeystoneAdapter
// ============================================================

pub struct MockKeystoneAdapter;

#[async_trait]
impl super::keystone::KeystonePort for MockKeystoneAdapter {
    async fn list_projects(
        &self,
        _pagination: &PaginationParams,
    ) -> ApiResult<PaginatedResponse<Project>> {
        Ok(PaginatedResponse::empty())
    }
    async fn get_project(&self, project_id: &str) -> ApiResult<Project> {
        Err(ApiError::NotFound {
            resource_type: "project".into(),
            id: project_id.into(),
        })
    }
    async fn create_project(&self, _params: &ProjectCreateParams) -> ApiResult<Project> {
        Err(ApiError::BadRequest("mock: not implemented".into()))
    }
    async fn update_project(
        &self,
        _project_id: &str,
        _params: &ProjectUpdateParams,
    ) -> ApiResult<Project> {
        Err(ApiError::BadRequest("mock: not implemented".into()))
    }
    async fn delete_project(&self, _project_id: &str) -> ApiResult<()> {
        Ok(())
    }
    async fn list_users(
        &self,
        _pagination: &PaginationParams,
    ) -> ApiResult<PaginatedResponse<User>> {
        Ok(PaginatedResponse::empty())
    }
    async fn get_user(&self, user_id: &str) -> ApiResult<User> {
        Err(ApiError::NotFound {
            resource_type: "user".into(),
            id: user_id.into(),
        })
    }
    async fn create_user(&self, _params: &UserCreateParams) -> ApiResult<User> {
        Err(ApiError::BadRequest("mock: not implemented".into()))
    }
    async fn update_user(&self, _user_id: &str, _params: &UserUpdateParams) -> ApiResult<User> {
        Err(ApiError::BadRequest("mock: not implemented".into()))
    }
    async fn delete_user(&self, _user_id: &str) -> ApiResult<()> {
        Ok(())
    }
    async fn list_roles(&self) -> ApiResult<Vec<Role>> {
        Ok(vec![])
    }
    async fn assign_role(&self, _params: &RoleAssignmentParams) -> ApiResult<()> {
        Ok(())
    }
    async fn revoke_role(&self, _params: &RoleAssignmentParams) -> ApiResult<()> {
        Ok(())
    }
    async fn list_role_assignments(
        &self,
        _filter: &RoleAssignmentFilter,
    ) -> ApiResult<Vec<RoleAssignment>> {
        Ok(vec![])
    }
    async fn list_domains(&self) -> ApiResult<Vec<Domain>> {
        Ok(vec![])
    }
}

#[cfg(test)]
mod tests {
    use super::super::cinder::CinderPort;
    use super::super::glance::GlancePort;
    use super::super::keystone::KeystonePort;
    use super::super::neutron::NeutronPort;
    use super::super::nova::NovaPort;
    use super::*;

    #[tokio::test]
    async fn test_mock_nova_list_servers() {
        let mock = MockNovaAdapter;
        let result = mock
            .list_servers(&ServerListFilter::default(), &PaginationParams::default())
            .await;
        assert!(result.is_ok());
        assert!(result.unwrap().items.is_empty());
    }

    #[tokio::test]
    async fn test_mock_neutron_list_networks() {
        let mock = MockNeutronAdapter;
        let result = mock.list_networks(&NetworkListFilter::default(), &PaginationParams::default()).await;
        assert!(result.is_ok());
        assert!(result.unwrap().items.is_empty());
    }

    #[tokio::test]
    async fn test_mock_cinder_list_volumes() {
        let mock = MockCinderAdapter;
        let result = mock
            .list_volumes(&VolumeListFilter::default(), &PaginationParams::default())
            .await;
        assert!(result.is_ok());
        assert!(result.unwrap().items.is_empty());
    }

    #[tokio::test]
    async fn test_mock_glance_list_images() {
        let mock = MockGlanceAdapter;
        let result = mock
            .list_images(&ImageListFilter::default(), &PaginationParams::default())
            .await;
        assert!(result.is_ok());
        assert!(result.unwrap().items.is_empty());
    }

    #[tokio::test]
    async fn test_mock_keystone_list_projects() {
        let mock = MockKeystoneAdapter;
        let result = mock.list_projects(&PaginationParams::default()).await;
        assert!(result.is_ok());
        assert!(result.unwrap().items.is_empty());
    }
}
