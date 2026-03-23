use std::collections::HashMap;

use async_trait::async_trait;

use super::error::ApiResult;
use super::types::*;
use crate::models::cinder::{Volume, VolumeSnapshot};

#[async_trait]
pub trait CinderPort: Send + Sync {
    // Volumes
    async fn list_volumes(
        &self,
        filter: &VolumeListFilter,
        pagination: &PaginationParams,
    ) -> ApiResult<PaginatedResponse<Volume>>;
    async fn get_volume(&self, volume_id: &str) -> ApiResult<Volume>;
    async fn create_volume(&self, params: &VolumeCreateParams) -> ApiResult<Volume>;
    async fn delete_volume(&self, volume_id: &str) -> ApiResult<()>;
    async fn force_delete_volume(&self, volume_id: &str) -> ApiResult<()>;
    async fn extend_volume(&self, volume_id: &str, new_size_gb: u32) -> ApiResult<()>;
    async fn attach_volume(
        &self,
        volume_id: &str,
        server_id: &str,
        device: Option<&str>,
    ) -> ApiResult<()>;
    async fn detach_volume(&self, volume_id: &str, attachment_id: &str) -> ApiResult<()>;
    async fn force_set_volume_state(&self, volume_id: &str, state: &str) -> ApiResult<()>;
    async fn migrate_volume(
        &self,
        volume_id: &str,
        dest_host: &str,
        force_host_copy: bool,
    ) -> ApiResult<()>;

    // Snapshots
    async fn list_snapshots(
        &self,
        pagination: &PaginationParams,
    ) -> ApiResult<PaginatedResponse<VolumeSnapshot>>;
    async fn get_snapshot(&self, snapshot_id: &str) -> ApiResult<VolumeSnapshot>;
    async fn create_snapshot(&self, params: &SnapshotCreateParams) -> ApiResult<VolumeSnapshot>;
    async fn delete_snapshot(&self, snapshot_id: &str) -> ApiResult<()>;

    // QoS
    async fn list_qos_specs(&self) -> ApiResult<Vec<QosSpec>>;
    async fn get_qos_spec(&self, qos_id: &str) -> ApiResult<QosSpec>;
    async fn create_qos_spec(&self, params: &QosCreateParams) -> ApiResult<QosSpec>;
    async fn update_qos_spec(
        &self,
        qos_id: &str,
        specs: &HashMap<String, String>,
    ) -> ApiResult<QosSpec>;
    async fn delete_qos_spec(&self, qos_id: &str) -> ApiResult<()>;

    // Storage Pools
    async fn list_storage_pools(&self, detail: bool) -> ApiResult<Vec<StoragePool>>;

    // Quota
    async fn get_volume_quota(&self, project_id: &str) -> ApiResult<VolumeQuota>;
    async fn update_volume_quota(
        &self,
        project_id: &str,
        params: &VolumeQuotaUpdateParams,
    ) -> ApiResult<VolumeQuota>;
}
