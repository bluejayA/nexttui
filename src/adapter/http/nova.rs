use std::collections::HashMap;
use std::sync::Arc;

use async_trait::async_trait;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

use super::{Link, append_pagination_parts, build_pagination_query, encode_param, extract_next_marker, paginated_list};
use crate::adapter::http::base::BaseHttpClient;
use crate::models::nova::{Aggregate, ComputeService, Flavor, Hypervisor, Server, ServerMigration};
use crate::port::auth::AuthProvider;
use crate::port::error::{ApiError, ApiResult};
use crate::port::nova::NovaPort;
use crate::port::types::*;

pub struct NovaHttpAdapter {
    base: BaseHttpClient,
}

impl NovaHttpAdapter {
    pub fn new(auth: Arc<dyn AuthProvider>, region: Option<String>) -> Result<Self, ApiError> {
        Ok(Self {
            base: BaseHttpClient::new(auth, "compute", EndpointInterface::Public, region)?,
        })
    }
}

// --- JSON wrapper structs (private) ---

#[derive(Deserialize)]
struct NovaServersResponse {
    servers: Vec<Server>,
    servers_links: Option<Vec<Link>>,
}

#[derive(Deserialize)]
struct NovaServerWrapper {
    server: Server,
}

#[derive(Deserialize)]
struct NovaServerCreateResponse {
    server: NovaServerCreateResult,
}

#[derive(Deserialize)]
struct NovaServerCreateResult {
    id: String,
}

#[derive(Deserialize)]
struct NovaFlavorsResponse {
    flavors: Vec<Flavor>,
    flavors_links: Option<Vec<Link>>,
}

#[derive(Deserialize)]
struct NovaFlavorWrapper {
    flavor: Flavor,
}

#[derive(Deserialize)]
struct NovaInstanceActionsResponse {
    #[serde(rename = "instanceActions")]
    instance_actions: Vec<ServerEvent>,
}

#[derive(Deserialize)]
struct NovaMigrationsResponse {
    migrations: Vec<ServerMigration>,
}

#[derive(Deserialize)]
struct NovaMigrationWrapper {
    migration: ServerMigration,
}

#[derive(Serialize)]
struct NovaServerCreateBody {
    server: NovaServerCreateInner,
}

#[derive(Serialize)]
struct NovaServerCreateInner {
    name: String,
    #[serde(rename = "imageRef")]
    image_ref: String,
    #[serde(rename = "flavorRef")]
    flavor_ref: String,
    networks: Vec<NovaNetworkAttachment>,
    #[serde(skip_serializing_if = "Option::is_none")]
    security_groups: Option<Vec<NovaSecurityGroupRef>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    key_name: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    availability_zone: Option<String>,
}

#[derive(Serialize)]
struct NovaNetworkAttachment {
    uuid: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    fixed_ip: Option<String>,
}

#[derive(Serialize)]
struct NovaSecurityGroupRef {
    name: String,
}

#[derive(Serialize)]
struct NovaFlavorCreateBody {
    flavor: NovaFlavorCreateInner,
}

#[derive(Serialize)]
struct NovaFlavorCreateInner {
    name: String,
    vcpus: u32,
    ram: u32,
    disk: u32,
    #[serde(rename = "os-flavor-access:is_public")]
    is_public: bool,
}

// --- Query builders ---

fn build_server_query(filter: &ServerListFilter, pagination: &PaginationParams) -> String {
    let mut parts = Vec::new();
    if let Some(ref name) = filter.name {
        parts.push(format!("name={}", encode_param(name)));
    }
    if let Some(ref status) = filter.status {
        parts.push(format!("status={}", encode_param(status)));
    }
    if let Some(ref host) = filter.host {
        parts.push(format!("host={}", encode_param(host)));
    }
    if let Some(ref flavor) = filter.flavor {
        parts.push(format!("flavor={}", encode_param(flavor)));
    }
    if filter.all_tenants {
        parts.push("all_tenants=1".to_string());
    }
    append_pagination_parts(&mut parts, pagination);
    parts.join("&")
}

impl RebootType {
    fn as_str(&self) -> &str {
        match self {
            RebootType::Soft => "SOFT",
            RebootType::Hard => "HARD",
        }
    }
}

impl ServerState {
    fn as_str(&self) -> &str {
        match self {
            ServerState::Active => "active",
            ServerState::Error => "error",
            ServerState::Paused => "paused",
            ServerState::Suspended => "suspended",
            ServerState::Stopped => "stopped",
        }
    }
}

// --- NovaPort implementation ---

#[async_trait]
impl NovaPort for NovaHttpAdapter {
    // -- Servers --

    async fn list_servers(
        &self,
        filter: &ServerListFilter,
        pagination: &PaginationParams,
    ) -> ApiResult<PaginatedResponse<Server>> {
        let query = build_server_query(filter, pagination);
        paginated_list(&self.base, "/servers/detail", &query, |resp: NovaServersResponse| {
            let next = resp.servers_links.as_deref().and_then(extract_next_marker);
            (resp.servers, next)
        }).await
    }

    async fn get_server(&self, server_id: &str) -> ApiResult<Server> {
        let req = self.base.get(&format!("/servers/{server_id}")).await?;
        let resp: NovaServerWrapper = self.base.send_json(req).await?;
        Ok(resp.server)
    }

    async fn create_server(&self, params: &ServerCreateParams) -> ApiResult<Server> {
        let body = NovaServerCreateBody {
            server: NovaServerCreateInner {
                name: params.name.clone(),
                image_ref: params.image_id.clone(),
                flavor_ref: params.flavor_id.clone(),
                networks: params
                    .networks
                    .iter()
                    .map(|n| NovaNetworkAttachment {
                        uuid: n.uuid.clone(),
                        fixed_ip: n.fixed_ip.clone(),
                    })
                    .collect(),
                security_groups: params.security_groups.as_ref().map(|sgs| {
                    sgs.iter()
                        .map(|name| NovaSecurityGroupRef { name: name.clone() })
                        .collect()
                }),
                key_name: params.key_name.clone(),
                availability_zone: params.availability_zone.clone(),
            },
        };
        let req = self.base.post("/servers").await?.json(&body);
        let resp: NovaServerCreateResponse = self.base.send_json(req).await?;
        // Create response has minimal fields; fetch full server detail
        let detail_req = self.base.get(&format!("/servers/{}", resp.server.id)).await?;
        let detail: NovaServerWrapper = self.base.send_json(detail_req).await?;
        Ok(detail.server)
    }

    async fn delete_server(&self, server_id: &str) -> ApiResult<()> {
        let req = self.base.delete(&format!("/servers/{server_id}")).await?;
        self.base.send_no_content(req).await
    }

    async fn reboot_server(&self, server_id: &str, reboot_type: RebootType) -> ApiResult<()> {
        let body = serde_json::json!({
            "reboot": { "type": reboot_type.as_str() }
        });
        let req = self
            .base
            .post(&format!("/servers/{server_id}/action"))
            .await?
            .json(&body);
        self.base.send_no_content(req).await
    }

    async fn start_server(&self, server_id: &str) -> ApiResult<()> {
        let body = serde_json::json!({ "os-start": null });
        let req = self
            .base
            .post(&format!("/servers/{server_id}/action"))
            .await?
            .json(&body);
        self.base.send_no_content(req).await
    }

    async fn stop_server(&self, server_id: &str) -> ApiResult<()> {
        let body = serde_json::json!({ "os-stop": null });
        let req = self
            .base
            .post(&format!("/servers/{server_id}/action"))
            .await?
            .json(&body);
        self.base.send_no_content(req).await
    }

    async fn force_set_server_state(
        &self,
        server_id: &str,
        state: ServerState,
    ) -> ApiResult<()> {
        let body = serde_json::json!({
            "os-resetState": { "state": state.as_str() }
        });
        let req = self
            .base
            .post(&format!("/servers/{server_id}/action"))
            .await?
            .json(&body);
        self.base.send_no_content(req).await
    }

    async fn create_server_snapshot(
        &self,
        server_id: &str,
        image_name: &str,
    ) -> ApiResult<String> {
        let body = serde_json::json!({
            "createImage": { "name": image_name }
        });
        let req = self
            .base
            .post(&format!("/servers/{server_id}/action"))
            .await?
            .json(&body);
        let resp = self.base.send(req).await?;
        let image_id = resp
            .headers()
            .get("Location")
            .and_then(|v| v.to_str().ok())
            .and_then(|url| url.rsplit('/').next())
            .map(String::from)
            .ok_or(ApiError::Unexpected {
                status: 200,
                body: "Missing Location header".into(),
            })?;
        Ok(image_id)
    }

    async fn list_server_events(&self, server_id: &str) -> ApiResult<Vec<ServerEvent>> {
        let req = self
            .base
            .get(&format!("/servers/{server_id}/os-instance-actions"))
            .await?;
        let resp: NovaInstanceActionsResponse = self.base.send_json(req).await?;
        Ok(resp.instance_actions)
    }

    // -- Resize --

    async fn resize_server(&self, server_id: &str, flavor_id: &str) -> ApiResult<()> {
        let body = serde_json::json!({
            "resize": {
                "flavorRef": flavor_id
            }
        });
        let req = self
            .base
            .post(&format!("/servers/{}/action", encode_param(server_id)))
            .await?
            .json(&body);
        self.base.send_no_content(req).await
    }

    // -- Migration --

    async fn live_migrate_server(
        &self,
        server_id: &str,
        params: &LiveMigrateParams,
    ) -> ApiResult<()> {
        let body = serde_json::json!({
            "os-migrateLive": {
                "host": params.host,
                "block_migration": true,
                "disk_over_commit": false
            }
        });
        let req = self
            .base
            .post(&format!("/servers/{}/action", encode_param(server_id)))
            .await?
            .json(&body);
        self.base.send_no_content(req).await
    }

    async fn cold_migrate_server(&self, server_id: &str) -> ApiResult<()> {
        let body = serde_json::json!({ "migrate": null });
        let req = self
            .base
            .post(&format!("/servers/{}/action", encode_param(server_id)))
            .await?
            .json(&body);
        self.base.send_no_content(req).await
    }

    async fn confirm_migration(&self, server_id: &str) -> ApiResult<()> {
        let body = serde_json::json!({ "confirmResize": null });
        let req = self
            .base
            .post(&format!("/servers/{}/action", encode_param(server_id)))
            .await?
            .json(&body);
        self.base.send_no_content(req).await
    }

    async fn revert_migration(&self, server_id: &str) -> ApiResult<()> {
        let body = serde_json::json!({ "revertResize": null });
        let req = self
            .base
            .post(&format!("/servers/{}/action", encode_param(server_id)))
            .await?
            .json(&body);
        self.base.send_no_content(req).await
    }

    async fn evacuate_server(
        &self,
        server_id: &str,
        params: &EvacuateParams,
    ) -> ApiResult<()> {
        let mut evac = serde_json::json!({});
        if let Some(host) = &params.host {
            evac["host"] = serde_json::json!(host);
        }
        if let Some(oss) = params.on_shared_storage {
            evac["onSharedStorage"] = serde_json::json!(oss);
        }
        if let Some(force) = params.force {
            evac["force"] = serde_json::json!(force);
        }
        let body = serde_json::json!({ "evacuate": evac });
        let req = self
            .base
            .post(&format!("/servers/{}/action", encode_param(server_id)))
            .await?
            .json(&body);
        self.base.send_no_content(req).await
    }

    async fn list_server_migrations(&self, server_id: &str) -> ApiResult<Vec<ServerMigration>> {
        let req = self
            .base
            .get(&format!("/servers/{}/migrations", encode_param(server_id)))
            .await?
            .header("OpenStack-API-Version", "compute 2.80");
        let resp: NovaMigrationsResponse = self.base.send_json(req).await?;
        Ok(resp.migrations)
    }

    async fn get_server_migration(&self, server_id: &str, migration_id: i64) -> ApiResult<ServerMigration> {
        let req = self
            .base
            .get(&format!("/servers/{}/migrations/{}", encode_param(server_id), migration_id))
            .await?
            .header("OpenStack-API-Version", "compute 2.80");
        let resp: NovaMigrationWrapper = self.base.send_json(req).await?;
        Ok(resp.migration)
    }

    // -- Flavors --

    async fn list_flavors(
        &self,
        pagination: &PaginationParams,
    ) -> ApiResult<PaginatedResponse<Flavor>> {
        let query = build_pagination_query(pagination);
        paginated_list(&self.base, "/flavors/detail", &query, |resp: NovaFlavorsResponse| {
            let next = resp.flavors_links.as_deref().and_then(extract_next_marker);
            (resp.flavors, next)
        }).await
    }

    async fn get_flavor(&self, flavor_id: &str) -> ApiResult<Flavor> {
        let req = self.base.get(&format!("/flavors/{flavor_id}")).await?;
        let resp: NovaFlavorWrapper = self.base.send_json(req).await?;
        Ok(resp.flavor)
    }

    async fn create_flavor(&self, params: &FlavorCreateParams) -> ApiResult<Flavor> {
        let body = NovaFlavorCreateBody {
            flavor: NovaFlavorCreateInner {
                name: params.name.clone(),
                vcpus: params.vcpus,
                ram: params.ram_mb,
                disk: params.disk_gb,
                is_public: params.is_public,
            },
        };
        let req = self.base.post("/flavors").await?.json(&body);
        let resp: NovaFlavorWrapper = self.base.send_json(req).await?;
        Ok(resp.flavor)
    }

    async fn delete_flavor(&self, flavor_id: &str) -> ApiResult<()> {
        let req = self.base.delete(&format!("/flavors/{flavor_id}")).await?;
        self.base.send_no_content(req).await
    }

    // -- Aggregates (stub — Unit 13) --

    async fn list_aggregates(&self) -> ApiResult<Vec<Aggregate>> {
        Err(ApiError::BadRequest("not yet implemented".into()))
    }

    async fn get_aggregate(&self, _aggregate_id: i64) -> ApiResult<Aggregate> {
        Err(ApiError::BadRequest("not yet implemented".into()))
    }

    async fn create_aggregate(&self, _params: &AggregateCreateParams) -> ApiResult<Aggregate> {
        Err(ApiError::BadRequest("not yet implemented".into()))
    }

    async fn update_aggregate(
        &self,
        _aggregate_id: i64,
        _params: &AggregateUpdateParams,
    ) -> ApiResult<Aggregate> {
        Err(ApiError::BadRequest("not yet implemented".into()))
    }

    async fn delete_aggregate(&self, _aggregate_id: i64) -> ApiResult<()> {
        Err(ApiError::BadRequest("not yet implemented".into()))
    }

    async fn aggregate_add_host(
        &self,
        _aggregate_id: i64,
        _host: &str,
    ) -> ApiResult<Aggregate> {
        Err(ApiError::BadRequest("not yet implemented".into()))
    }

    async fn aggregate_remove_host(
        &self,
        _aggregate_id: i64,
        _host: &str,
    ) -> ApiResult<Aggregate> {
        Err(ApiError::BadRequest("not yet implemented".into()))
    }

    async fn aggregate_set_metadata(
        &self,
        _aggregate_id: i64,
        _metadata: &HashMap<String, String>,
    ) -> ApiResult<Aggregate> {
        Err(ApiError::BadRequest("not yet implemented".into()))
    }

    // -- Compute Services (stub — Unit 13) --

    async fn list_compute_services(&self) -> ApiResult<Vec<ComputeService>> {
        Err(ApiError::BadRequest("not yet implemented".into()))
    }

    async fn enable_compute_service(&self, _service_id: &str) -> ApiResult<ComputeService> {
        Err(ApiError::BadRequest("not yet implemented".into()))
    }

    async fn disable_compute_service(
        &self,
        _service_id: &str,
        _reason: Option<&str>,
    ) -> ApiResult<ComputeService> {
        Err(ApiError::BadRequest("not yet implemented".into()))
    }

    // -- Hypervisors (stub — Unit 13) --

    async fn list_hypervisors(&self) -> ApiResult<Vec<Hypervisor>> {
        Err(ApiError::BadRequest("not yet implemented".into()))
    }

    async fn get_hypervisor(&self, _hypervisor_id: &str) -> ApiResult<Hypervisor> {
        Err(ApiError::BadRequest("not yet implemented".into()))
    }

    // -- Usage (stub — Unit 14) --

    async fn get_project_usage(
        &self,
        _project_id: &str,
        _start: DateTime<Utc>,
        _end: DateTime<Utc>,
    ) -> ApiResult<ProjectUsage> {
        Err(ApiError::BadRequest("not yet implemented".into()))
    }

    // -- Quota (stub — Unit 12) --

    async fn get_compute_quota(&self, _project_id: &str) -> ApiResult<ComputeQuota> {
        Err(ApiError::BadRequest("not yet implemented".into()))
    }

    async fn update_compute_quota(
        &self,
        _project_id: &str,
        _params: &ComputeQuotaUpdateParams,
    ) -> ApiResult<ComputeQuota> {
        Err(ApiError::BadRequest("not yet implemented".into()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_build_server_query_full() {
        let filter = ServerListFilter {
            name: Some("web".into()),
            status: Some("ACTIVE".into()),
            host: Some("compute-01".into()),
            flavor: Some("m1.small".into()),
            all_tenants: true,
        };
        let pagination = PaginationParams {
            marker: Some("abc".into()),
            limit: Some(50),
            sort_key: Some("name".into()),
            sort_dir: Some(SortDirection::Asc),
        };
        let query = build_server_query(&filter, &pagination);
        assert!(query.contains("name=web"));
        assert!(query.contains("status=ACTIVE"));
        assert!(query.contains("host=compute-01"));
        assert!(query.contains("flavor=m1.small"));
        assert!(query.contains("all_tenants=1"));
        assert!(query.contains("marker=abc"));
        assert!(query.contains("limit=50"));
        assert!(query.contains("sort_key=name"));
        assert!(query.contains("sort_dir=asc"));
    }

    #[test]
    fn test_encode_param_special_chars() {
        assert_eq!(encode_param("foo&bar=baz"), "foo%26bar%3Dbaz");
        assert_eq!(encode_param("all_tenants=1"), "all_tenants%3D1");
        assert_eq!(encode_param("hello world"), "hello%20world");
        assert_eq!(encode_param("simple"), "simple");
    }

    #[test]
    fn test_build_server_query_injection_safe() {
        let filter = ServerListFilter {
            name: Some("foo&all_tenants=1".into()),
            ..Default::default()
        };
        let pagination = PaginationParams::default();
        let query = build_server_query(&filter, &pagination);
        // The & and = in the name should be encoded, preventing injection
        assert!(query.contains("name=foo%26all_tenants%3D1"));
        assert!(!query.contains("all_tenants=1"));
    }

    #[test]
    fn test_build_server_query_empty() {
        let filter = ServerListFilter::default();
        let pagination = PaginationParams::default();
        let query = build_server_query(&filter, &pagination);
        assert!(query.is_empty());
    }

    #[test]
    fn test_build_pagination_query() {
        let pagination = PaginationParams {
            marker: Some("marker-123".into()),
            limit: Some(100),
            sort_key: None,
            sort_dir: None,
        };
        let query = build_pagination_query(&pagination);
        assert!(query.contains("marker=marker-123"));
        assert!(query.contains("limit=100"));
        assert!(!query.contains("sort_key"));
    }

    #[test]
    fn test_extract_next_marker() {
        let links = vec![
            Link {
                rel: "self".into(),
                href: "http://nova/servers?limit=50".into(),
            },
            Link {
                rel: "next".into(),
                href: "http://nova/servers?limit=50&marker=srv-last".into(),
            },
        ];
        let marker = extract_next_marker(&links);
        assert_eq!(marker, Some("srv-last".to_string()));
    }

    #[test]
    fn test_extract_next_marker_none() {
        let links = vec![Link {
            rel: "self".into(),
            href: "http://nova/servers?limit=50".into(),
        }];
        let marker = extract_next_marker(&links);
        assert!(marker.is_none());
    }

    #[test]
    fn test_nova_server_create_body_serialize() {
        let body = NovaServerCreateBody {
            server: NovaServerCreateInner {
                name: "test-vm".into(),
                image_ref: "img-1".into(),
                flavor_ref: "flv-1".into(),
                networks: vec![NovaNetworkAttachment {
                    uuid: "net-1".into(),
                    fixed_ip: None,
                }],
                security_groups: Some(vec![NovaSecurityGroupRef {
                    name: "default".into(),
                }]),
                key_name: Some("mykey".into()),
                availability_zone: None,
            },
        };
        let json = serde_json::to_value(&body).unwrap();
        let server = &json["server"];
        assert_eq!(server["name"], "test-vm");
        assert_eq!(server["imageRef"], "img-1");
        assert_eq!(server["flavorRef"], "flv-1");
        assert_eq!(server["networks"][0]["uuid"], "net-1");
        assert_eq!(server["security_groups"][0]["name"], "default");
        assert_eq!(server["key_name"], "mykey");
        assert!(server.get("availability_zone").is_none());
    }

    #[test]
    fn test_nova_flavor_create_body_serialize() {
        let body = NovaFlavorCreateBody {
            flavor: NovaFlavorCreateInner {
                name: "m1.test".into(),
                vcpus: 2,
                ram: 4096,
                disk: 40,
                is_public: true,
            },
        };
        let json = serde_json::to_value(&body).unwrap();
        let flavor = &json["flavor"];
        assert_eq!(flavor["name"], "m1.test");
        assert_eq!(flavor["vcpus"], 2);
        assert_eq!(flavor["ram"], 4096);
        assert_eq!(flavor["disk"], 40);
        assert_eq!(flavor["os-flavor-access:is_public"], true);
    }

    #[test]
    fn test_server_events_deserialize() {
        let json = r#"{
            "instanceActions": [
                {"action": "create", "start_time": "2026-01-01T00:00:00Z", "finish_time": "2026-01-01T00:01:00Z", "result": "Success", "message": null},
                {"action": "reboot", "start_time": "2026-01-02T00:00:00Z", "finish_time": null, "result": null, "message": null}
            ]
        }"#;
        let resp: NovaInstanceActionsResponse = serde_json::from_str(json).unwrap();
        assert_eq!(resp.instance_actions.len(), 2);
        assert_eq!(resp.instance_actions[0].action, "create");
        assert_eq!(
            resp.instance_actions[0].result.as_deref(),
            Some("Success")
        );
        assert_eq!(resp.instance_actions[1].action, "reboot");
        assert!(resp.instance_actions[1].finish_time.is_none());
    }

    #[test]
    fn test_reboot_type_as_str() {
        assert_eq!(RebootType::Soft.as_str(), "SOFT");
        assert_eq!(RebootType::Hard.as_str(), "HARD");
    }

    #[test]
    fn test_server_state_as_str() {
        assert_eq!(ServerState::Active.as_str(), "active");
        assert_eq!(ServerState::Error.as_str(), "error");
        assert_eq!(ServerState::Stopped.as_str(), "stopped");
    }

    #[test]
    fn test_live_migrate_body_with_host() {
        let params = LiveMigrateParams {
            host: Some("compute-02".into()),
        };
        let body = serde_json::json!({
            "os-migrateLive": {
                "host": params.host,
                "block_migration": true
            }
        });
        let obj = body.as_object().unwrap();
        let inner = obj["os-migrateLive"].as_object().unwrap();
        assert_eq!(inner["host"], "compute-02");
        assert_eq!(inner["block_migration"], true);
    }

    #[test]
    fn test_live_migrate_body_auto_host() {
        let params = LiveMigrateParams {
            host: None,
        };
        let body = serde_json::json!({
            "os-migrateLive": {
                "host": params.host,
                "block_migration": true
            }
        });
        let inner = &body["os-migrateLive"];
        assert!(inner["host"].is_null());
        assert_eq!(inner["block_migration"], true);
    }

    #[test]
    fn test_cold_migrate_body() {
        let body = serde_json::json!({ "migrate": null });
        assert!(body["migrate"].is_null());
        assert!(body.as_object().unwrap().contains_key("migrate"));
    }

    #[test]
    fn test_confirm_revert_body() {
        let confirm = serde_json::json!({ "confirmResize": null });
        let revert = serde_json::json!({ "revertResize": null });
        assert!(confirm.as_object().unwrap().contains_key("confirmResize"));
        assert!(revert.as_object().unwrap().contains_key("revertResize"));
    }

    #[test]
    fn test_evacuate_body() {
        let with_host = serde_json::json!({
            "evacuate": { "host": "compute-03" }
        });
        assert_eq!(with_host["evacuate"]["host"], "compute-03");

        let auto_host = serde_json::json!({
            "evacuate": { "host": serde_json::Value::Null }
        });
        assert!(auto_host["evacuate"]["host"].is_null());
    }

    #[test]
    fn test_migrations_response_deserialize() {
        let json = r#"{
            "migrations": [
                {
                    "id": 1,
                    "status": "running",
                    "source_compute": "compute-01",
                    "dest_compute": "compute-02",
                    "memory_total_bytes": 1073741824,
                    "memory_processed_bytes": 536870912,
                    "memory_remaining_bytes": 536870912,
                    "disk_total_bytes": 10737418240,
                    "disk_processed_bytes": 5368709120,
                    "disk_remaining_bytes": 5368709120,
                    "created_at": "2026-03-28T10:00:00Z",
                    "updated_at": "2026-03-28T10:01:00Z"
                },
                {
                    "id": 2,
                    "status": "completed",
                    "source_compute": "compute-01",
                    "dest_compute": "compute-03"
                }
            ]
        }"#;
        let resp: NovaMigrationsResponse = serde_json::from_str(json).unwrap();
        assert_eq!(resp.migrations.len(), 2);
        assert_eq!(resp.migrations[0].status, "running");
        assert_eq!(resp.migrations[0].memory_total_bytes, Some(1_073_741_824));
        assert_eq!(resp.migrations[1].status, "completed");
        assert!(resp.migrations[1].memory_total_bytes.is_none());
    }

    #[test]
    fn test_migration_wrapper_deserialize() {
        let json = r#"{
            "migration": {
                "id": 42,
                "status": "post-migrating",
                "source_compute": "node-a",
                "dest_compute": "node-b",
                "memory_total_bytes": 2147483648,
                "memory_processed_bytes": 2147483648,
                "memory_remaining_bytes": 0,
                "disk_total_bytes": 0,
                "disk_processed_bytes": 0,
                "disk_remaining_bytes": 0
            }
        }"#;
        let resp: NovaMigrationWrapper = serde_json::from_str(json).unwrap();
        assert_eq!(resp.migration.id, 42);
        assert_eq!(resp.migration.status, "post-migrating");
        assert_eq!(resp.migration.memory_remaining_bytes, Some(0));
    }
}
