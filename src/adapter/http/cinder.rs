use std::collections::HashMap;
use std::sync::Arc;

use async_trait::async_trait;
use serde::{Deserialize, Serialize};

use super::{Link, build_pagination_query, encode_param, append_pagination_parts, extract_next_marker};
use crate::adapter::http::base::BaseHttpClient;
use crate::models::cinder::{Volume, VolumeSnapshot};
use crate::port::auth::AuthProvider;
use crate::port::error::{ApiError, ApiResult};
use crate::port::cinder::CinderPort;
use crate::port::types::*;

pub struct CinderHttpAdapter {
    base: BaseHttpClient,
}

impl CinderHttpAdapter {
    pub fn new(auth: Arc<dyn AuthProvider>, region: Option<String>) -> Self {
        Self {
            base: BaseHttpClient::new(auth, "volumev3", EndpointInterface::Internal, region),
        }
    }
}

// --- JSON wrapper structs ---

#[derive(Deserialize)]
struct CinderVolumesResponse {
    volumes: Vec<Volume>,
    volumes_links: Option<Vec<Link>>,
}

#[derive(Deserialize)]
struct CinderVolumeWrapper {
    volume: Volume,
}

#[derive(Deserialize)]
struct CinderSnapshotsResponse {
    snapshots: Vec<VolumeSnapshot>,
    snapshots_links: Option<Vec<Link>>,
}

#[derive(Deserialize)]
struct CinderSnapshotWrapper {
    snapshot: VolumeSnapshot,
}

#[allow(dead_code)] // Used in Unit 14
#[derive(Deserialize)]
struct CinderQosSpecsResponse {
    qos_specs: Vec<QosSpec>,
}

#[allow(dead_code)] // Used in Unit 14
#[derive(Deserialize)]
struct CinderQosSpecWrapper {
    qos_spec: QosSpec,
}

#[allow(dead_code)] // Used in Unit 14
#[derive(Deserialize)]
struct CinderStoragePoolsResponse {
    pools: Vec<StoragePool>,
}

#[allow(dead_code)] // Used in Unit 12
#[derive(Deserialize)]
struct CinderQuotaWrapper {
    quota_set: VolumeQuota,
}

// --- Serialize structs ---

#[derive(Serialize)]
struct VolumeCreateBody {
    volume: VolumeCreateInner,
}

#[derive(Serialize)]
struct VolumeCreateInner {
    name: String,
    size: u32,
    #[serde(skip_serializing_if = "Option::is_none")]
    volume_type: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    description: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    availability_zone: Option<String>,
}

#[derive(Serialize)]
struct SnapshotCreateBody {
    snapshot: SnapshotCreateInner,
}

#[derive(Serialize)]
struct SnapshotCreateInner {
    volume_id: String,
    name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    description: Option<String>,
    force: bool,
}

// --- Query builders ---

fn build_volume_query(filter: &VolumeListFilter, pagination: &PaginationParams) -> String {
    let mut parts = Vec::new();
    if let Some(ref name) = filter.name {
        parts.push(format!("name={}", encode_param(name)));
    }
    if let Some(ref status) = filter.status {
        parts.push(format!("status={}", encode_param(status)));
    }
    if filter.all_tenants {
        parts.push("all_tenants=1".to_string());
    }
    append_pagination_parts(&mut parts, pagination);
    parts.join("&")
}

// --- CinderPort implementation ---

#[async_trait]
impl CinderPort for CinderHttpAdapter {
    // -- Volumes --

    async fn list_volumes(
        &self,
        filter: &VolumeListFilter,
        pagination: &PaginationParams,
    ) -> ApiResult<PaginatedResponse<Volume>> {
        let query = build_volume_query(filter, pagination);
        let path = if query.is_empty() {
            "/volumes/detail".to_string()
        } else {
            format!("/volumes/detail?{query}")
        };
        let req = self.base.get(&path).await?;
        let resp: CinderVolumesResponse = self.base.send_json(req).await?;
        let next_marker = resp
            .volumes_links
            .as_deref()
            .and_then(extract_next_marker);
        let has_more = next_marker.is_some();
        Ok(PaginatedResponse {
            items: resp.volumes,
            next_marker,
            has_more,
        })
    }

    async fn get_volume(&self, volume_id: &str) -> ApiResult<Volume> {
        let req = self
            .base
            .get(&format!("/volumes/{}", encode_param(volume_id)))
            .await?;
        let resp: CinderVolumeWrapper = self.base.send_json(req).await?;
        Ok(resp.volume)
    }

    async fn create_volume(&self, params: &VolumeCreateParams) -> ApiResult<Volume> {
        let body = VolumeCreateBody {
            volume: VolumeCreateInner {
                name: params.name.clone(),
                size: params.size_gb,
                volume_type: params.volume_type.clone(),
                description: params.description.clone(),
                availability_zone: params.availability_zone.clone(),
            },
        };
        let req = self.base.post("/volumes").await?.json(&body);
        let resp: CinderVolumeWrapper = self.base.send_json(req).await?;
        Ok(resp.volume)
    }

    async fn delete_volume(&self, volume_id: &str) -> ApiResult<()> {
        let req = self
            .base
            .delete(&format!("/volumes/{}", encode_param(volume_id)))
            .await?;
        self.base.send_no_content(req).await
    }

    async fn force_delete_volume(&self, volume_id: &str) -> ApiResult<()> {
        let body = serde_json::json!({
            "os-force_delete": {}
        });
        let req = self
            .base
            .post(&format!("/volumes/{}/action", encode_param(volume_id)))
            .await?
            .json(&body);
        self.base.send_no_content(req).await
    }

    async fn extend_volume(&self, volume_id: &str, new_size_gb: u32) -> ApiResult<()> {
        let body = serde_json::json!({
            "os-extend": { "new_size": new_size_gb }
        });
        let req = self
            .base
            .post(&format!("/volumes/{}/action", encode_param(volume_id)))
            .await?
            .json(&body);
        self.base.send_no_content(req).await
    }

    // Phase 2: attach/detach require Nova server integration
    async fn attach_volume(
        &self,
        _volume_id: &str,
        _server_id: &str,
        _device: Option<&str>,
    ) -> ApiResult<()> {
        Err(ApiError::BadRequest("not yet implemented".into()))
    }

    async fn detach_volume(&self, _volume_id: &str, _attachment_id: &str) -> ApiResult<()> {
        Err(ApiError::BadRequest("not yet implemented".into()))
    }

    async fn force_set_volume_state(&self, volume_id: &str, state: &str) -> ApiResult<()> {
        let body = serde_json::json!({
            "os-reset_status": { "status": state }
        });
        let req = self
            .base
            .post(&format!("/volumes/{}/action", encode_param(volume_id)))
            .await?
            .json(&body);
        self.base.send_no_content(req).await
    }

    async fn migrate_volume(
        &self,
        volume_id: &str,
        dest_host: &str,
        force_host_copy: bool,
    ) -> ApiResult<()> {
        let body = serde_json::json!({
            "os-migrate_volume": {
                "host": dest_host,
                "force_host_copy": force_host_copy
            }
        });
        let req = self
            .base
            .post(&format!("/volumes/{}/action", encode_param(volume_id)))
            .await?
            .json(&body);
        self.base.send_no_content(req).await
    }

    // -- Snapshots --

    async fn list_snapshots(
        &self,
        pagination: &PaginationParams,
    ) -> ApiResult<PaginatedResponse<VolumeSnapshot>> {
        let query = build_pagination_query(pagination);
        let path = if query.is_empty() {
            "/snapshots/detail".to_string()
        } else {
            format!("/snapshots/detail?{query}")
        };
        let req = self.base.get(&path).await?;
        let resp: CinderSnapshotsResponse = self.base.send_json(req).await?;
        let next_marker = resp
            .snapshots_links
            .as_deref()
            .and_then(extract_next_marker);
        let has_more = next_marker.is_some();
        Ok(PaginatedResponse {
            items: resp.snapshots,
            next_marker,
            has_more,
        })
    }

    async fn get_snapshot(&self, snapshot_id: &str) -> ApiResult<VolumeSnapshot> {
        let req = self
            .base
            .get(&format!("/snapshots/{}", encode_param(snapshot_id)))
            .await?;
        let resp: CinderSnapshotWrapper = self.base.send_json(req).await?;
        Ok(resp.snapshot)
    }

    async fn create_snapshot(&self, params: &SnapshotCreateParams) -> ApiResult<VolumeSnapshot> {
        let body = SnapshotCreateBody {
            snapshot: SnapshotCreateInner {
                volume_id: params.volume_id.clone(),
                name: params.name.clone(),
                description: params.description.clone(),
                force: params.force,
            },
        };
        let req = self.base.post("/snapshots").await?.json(&body);
        let resp: CinderSnapshotWrapper = self.base.send_json(req).await?;
        Ok(resp.snapshot)
    }

    async fn delete_snapshot(&self, snapshot_id: &str) -> ApiResult<()> {
        let req = self
            .base
            .delete(&format!("/snapshots/{}", encode_param(snapshot_id)))
            .await?;
        self.base.send_no_content(req).await
    }

    // -- QoS (stubs — Unit 14) --

    async fn list_qos_specs(&self) -> ApiResult<Vec<QosSpec>> {
        Err(ApiError::BadRequest("not yet implemented".into()))
    }

    async fn get_qos_spec(&self, _qos_id: &str) -> ApiResult<QosSpec> {
        Err(ApiError::BadRequest("not yet implemented".into()))
    }

    async fn create_qos_spec(&self, _params: &QosCreateParams) -> ApiResult<QosSpec> {
        Err(ApiError::BadRequest("not yet implemented".into()))
    }

    async fn update_qos_spec(
        &self,
        _qos_id: &str,
        _specs: &HashMap<String, String>,
    ) -> ApiResult<QosSpec> {
        Err(ApiError::BadRequest("not yet implemented".into()))
    }

    async fn delete_qos_spec(&self, _qos_id: &str) -> ApiResult<()> {
        Err(ApiError::BadRequest("not yet implemented".into()))
    }

    // -- Storage Pools (stub — Unit 14) --

    async fn list_storage_pools(&self, _detail: bool) -> ApiResult<Vec<StoragePool>> {
        Err(ApiError::BadRequest("not yet implemented".into()))
    }

    // -- Quota (stub — Unit 12) --

    async fn get_volume_quota(&self, _project_id: &str) -> ApiResult<VolumeQuota> {
        Err(ApiError::BadRequest("not yet implemented".into()))
    }

    async fn update_volume_quota(
        &self,
        _project_id: &str,
        _params: &VolumeQuotaUpdateParams,
    ) -> ApiResult<VolumeQuota> {
        Err(ApiError::BadRequest("not yet implemented".into()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_cinder_volumes_response_deserialize() {
        let json = r#"{
            "volumes": [
                {
                    "id": "vol-1",
                    "name": "data",
                    "status": "available",
                    "size": 100,
                    "volume_type": "ssd",
                    "encrypted": false,
                    "bootable": "false",
                    "attachments": [],
                    "availability_zone": "az1",
                    "created_at": "2026-01-01T00:00:00Z"
                }
            ],
            "volumes_links": [
                {"rel": "next", "href": "http://cinder/volumes?marker=vol-1&limit=50"}
            ]
        }"#;
        let resp: CinderVolumesResponse = serde_json::from_str(json).unwrap();
        assert_eq!(resp.volumes.len(), 1);
        assert_eq!(resp.volumes[0].name.as_deref(), Some("data"));
        let marker = extract_next_marker(resp.volumes_links.as_deref().unwrap());
        assert_eq!(marker, Some("vol-1".to_string()));
    }

    #[test]
    fn test_cinder_volume_wrapper_deserialize() {
        let json = r#"{
            "volume": {
                "id": "vol-1",
                "name": "boot",
                "description": "Boot volume",
                "status": "in-use",
                "size": 50,
                "volume_type": "ssd",
                "encrypted": true,
                "bootable": "true",
                "attachments": [
                    {"server_id": "srv-1", "device": "/dev/vda", "id": "att-1"}
                ],
                "availability_zone": "az1",
                "created_at": "2026-01-01T00:00:00Z"
            }
        }"#;
        let resp: CinderVolumeWrapper = serde_json::from_str(json).unwrap();
        assert_eq!(resp.volume.name.as_deref(), Some("boot"));
        assert_eq!(resp.volume.description.as_deref(), Some("Boot volume"));
        assert!(resp.volume.encrypted);
        assert_eq!(resp.volume.bootable, "true");
        assert_eq!(resp.volume.attachments.len(), 1);
    }

    #[test]
    fn test_volume_create_body_serialize() {
        let body = VolumeCreateBody {
            volume: VolumeCreateInner {
                name: "test-vol".into(),
                size: 100,
                volume_type: Some("ssd".into()),
                description: None,
                availability_zone: None,
            },
        };
        let json = serde_json::to_value(&body).unwrap();
        let vol = &json["volume"];
        assert_eq!(vol["name"], "test-vol");
        assert_eq!(vol["size"], 100);
        assert_eq!(vol["volume_type"], "ssd");
        assert!(vol.get("description").is_none());
        assert!(vol.get("availability_zone").is_none());
    }

    #[test]
    fn test_cinder_snapshots_response_deserialize() {
        let json = r#"{
            "snapshots": [
                {
                    "id": "snap-1",
                    "name": "daily",
                    "status": "available",
                    "size": 100,
                    "volume_id": "vol-1",
                    "created_at": "2026-01-15T00:00:00Z"
                }
            ]
        }"#;
        let resp: CinderSnapshotsResponse = serde_json::from_str(json).unwrap();
        assert_eq!(resp.snapshots.len(), 1);
        assert_eq!(resp.snapshots[0].name.as_deref(), Some("daily"));
    }

    #[test]
    fn test_snapshot_create_body_serialize() {
        let body = SnapshotCreateBody {
            snapshot: SnapshotCreateInner {
                volume_id: "vol-1".into(),
                name: "snap-test".into(),
                description: Some("Test snapshot".into()),
                force: false,
            },
        };
        let json = serde_json::to_value(&body).unwrap();
        let snap = &json["snapshot"];
        assert_eq!(snap["volume_id"], "vol-1");
        assert_eq!(snap["name"], "snap-test");
        assert_eq!(snap["description"], "Test snapshot");
        assert_eq!(snap["force"], false);
    }

    #[test]
    fn test_build_volume_query_full() {
        let filter = VolumeListFilter {
            name: Some("data".into()),
            status: Some("available".into()),
            all_tenants: true,
        };
        let pagination = PaginationParams {
            marker: Some("vol-last".into()),
            limit: Some(50),
            sort_key: None,
            sort_dir: None,
        };
        let query = build_volume_query(&filter, &pagination);
        assert!(query.contains("name=data"));
        assert!(query.contains("status=available"));
        assert!(query.contains("all_tenants=1"));
        assert!(query.contains("marker=vol-last"));
        assert!(query.contains("limit=50"));
    }

    #[test]
    fn test_build_volume_query_empty() {
        let filter = VolumeListFilter::default();
        let pagination = PaginationParams::default();
        let query = build_volume_query(&filter, &pagination);
        assert!(query.is_empty());
    }

    #[test]
    fn test_build_volume_query_injection_safe() {
        let filter = VolumeListFilter {
            name: Some("foo&all_tenants=1".into()),
            ..Default::default()
        };
        let pagination = PaginationParams::default();
        let query = build_volume_query(&filter, &pagination);
        assert!(query.contains("name=foo%26all_tenants%3D1"));
        assert!(!query.contains("all_tenants=1"));
    }
}
