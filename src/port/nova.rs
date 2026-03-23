use std::collections::HashMap;

use async_trait::async_trait;
use chrono::{DateTime, Utc};

use super::error::ApiResult;
use super::types::*;
use crate::models::nova::{Aggregate, ComputeService, Flavor, Hypervisor, Server};

#[async_trait]
pub trait NovaPort: Send + Sync {
    // Servers
    async fn list_servers(
        &self,
        filter: &ServerListFilter,
        pagination: &PaginationParams,
    ) -> ApiResult<PaginatedResponse<Server>>;
    async fn get_server(&self, server_id: &str) -> ApiResult<Server>;
    async fn create_server(&self, params: &ServerCreateParams) -> ApiResult<Server>;
    async fn delete_server(&self, server_id: &str) -> ApiResult<()>;
    async fn reboot_server(&self, server_id: &str, reboot_type: RebootType) -> ApiResult<()>;
    async fn start_server(&self, server_id: &str) -> ApiResult<()>;
    async fn stop_server(&self, server_id: &str) -> ApiResult<()>;
    async fn force_set_server_state(&self, server_id: &str, state: ServerState) -> ApiResult<()>;
    async fn create_server_snapshot(&self, server_id: &str, image_name: &str) -> ApiResult<String>;
    async fn list_server_events(&self, server_id: &str) -> ApiResult<Vec<ServerEvent>>;

    // Migration
    async fn live_migrate_server(
        &self,
        server_id: &str,
        params: &LiveMigrateParams,
    ) -> ApiResult<()>;
    async fn cold_migrate_server(&self, server_id: &str) -> ApiResult<()>;
    async fn confirm_migration(&self, server_id: &str) -> ApiResult<()>;
    async fn revert_migration(&self, server_id: &str) -> ApiResult<()>;
    async fn evacuate_server(&self, server_id: &str, params: &EvacuateParams) -> ApiResult<()>;

    // Flavors
    async fn list_flavors(
        &self,
        pagination: &PaginationParams,
    ) -> ApiResult<PaginatedResponse<Flavor>>;
    async fn get_flavor(&self, flavor_id: &str) -> ApiResult<Flavor>;
    async fn create_flavor(&self, params: &FlavorCreateParams) -> ApiResult<Flavor>;
    async fn delete_flavor(&self, flavor_id: &str) -> ApiResult<()>;

    // Aggregates
    async fn list_aggregates(&self) -> ApiResult<Vec<Aggregate>>;
    async fn get_aggregate(&self, aggregate_id: i64) -> ApiResult<Aggregate>;
    async fn create_aggregate(&self, params: &AggregateCreateParams) -> ApiResult<Aggregate>;
    async fn update_aggregate(
        &self,
        aggregate_id: i64,
        params: &AggregateUpdateParams,
    ) -> ApiResult<Aggregate>;
    async fn delete_aggregate(&self, aggregate_id: i64) -> ApiResult<()>;
    async fn aggregate_add_host(&self, aggregate_id: i64, host: &str) -> ApiResult<Aggregate>;
    async fn aggregate_remove_host(&self, aggregate_id: i64, host: &str) -> ApiResult<Aggregate>;
    async fn aggregate_set_metadata(
        &self,
        aggregate_id: i64,
        metadata: &HashMap<String, String>,
    ) -> ApiResult<Aggregate>;

    // Compute Services
    async fn list_compute_services(&self) -> ApiResult<Vec<ComputeService>>;
    async fn enable_compute_service(&self, service_id: &str) -> ApiResult<ComputeService>;
    async fn disable_compute_service(
        &self,
        service_id: &str,
        reason: Option<&str>,
    ) -> ApiResult<ComputeService>;

    // Hypervisors
    async fn list_hypervisors(&self) -> ApiResult<Vec<Hypervisor>>;
    async fn get_hypervisor(&self, hypervisor_id: &str) -> ApiResult<Hypervisor>;

    // Usage
    async fn get_project_usage(
        &self,
        project_id: &str,
        start: DateTime<Utc>,
        end: DateTime<Utc>,
    ) -> ApiResult<ProjectUsage>;

    // Quota
    async fn get_compute_quota(&self, project_id: &str) -> ApiResult<ComputeQuota>;
    async fn update_compute_quota(
        &self,
        project_id: &str,
        params: &ComputeQuotaUpdateParams,
    ) -> ApiResult<ComputeQuota>;
}
