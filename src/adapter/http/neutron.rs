use std::sync::Arc;

use async_trait::async_trait;
use serde::{Deserialize, Serialize};

use super::{Link, append_pagination_parts, encode_param, extract_next_marker, paginated_list};
use crate::adapter::http::base::BaseHttpClient;
use crate::adapter::http::neutron_audit::NeutronAuditCtx;
use crate::adapter::http::scope_refilter::{RefilterScope, refilter_and_audit};
use crate::models::neutron::{
    FloatingIp, Network, NetworkAgent, Port, SecurityGroup, SecurityGroupRule,
};
use crate::port::auth::AuthProvider;
use crate::port::error::{ApiError, ApiResult};
use crate::port::neutron::NeutronPort;
use crate::port::types::*;

pub struct NeutronHttpAdapter {
    base: Arc<BaseHttpClient>,
    audit_ctx: Option<Arc<NeutronAuditCtx>>,
}

impl NeutronHttpAdapter {
    pub fn new(auth: Arc<dyn AuthProvider>, region: Option<String>) -> Result<Self, ApiError> {
        Ok(Self {
            base: Arc::new(BaseHttpClient::new(
                auth,
                "network",
                EndpointInterface::Public,
                region,
            )?),
            audit_ctx: None,
        })
    }

    pub fn from_base(base: Arc<BaseHttpClient>) -> Self {
        Self {
            base,
            audit_ctx: None,
        }
    }

    /// BL-P2-085 Step 13b: attach a `NeutronAuditCtx` so every `list_*` call
    /// runs `refilter_by_scope` against the response and emits an
    /// `AdapterFilterViolation` audit event per dropped row. Wired by
    /// `registry::new_http` (Step 13b-3).
    pub fn with_audit(mut self, ctx: Arc<NeutronAuditCtx>) -> Self {
        self.audit_ctx = Some(ctx);
        self
    }

    /// Apply response-side scope refiltering to a `PaginatedResponse`.
    /// No-op when `audit_ctx` is None (pre-Step-13b-3 adapters), preserving
    /// the original response shape. When attached, partitions via
    /// [`refilter_and_audit`] which fans out one `AdapterFilterViolation`
    /// event per dropped row before returning the kept items.
    ///
    /// [`refilter_and_audit`]: crate::adapter::http::scope_refilter::refilter_and_audit
    fn refilter_response<T>(
        &self,
        resp: PaginatedResponse<T>,
        all_tenants: bool,
        action_type: &str,
        resource_kind: &str,
    ) -> PaginatedResponse<T>
    where
        T: crate::adapter::http::scope_refilter::ScopedItem,
    {
        let active = self
            .audit_ctx
            .as_ref()
            .and_then(|ctx| ctx.scope_provider.current_project_id());
        let scope = RefilterScope::from_parts(active.as_deref(), all_tenants);
        // correlation_id=0: list_* are not bound to a worker dispatch,
        // and the canonical fingerprint already disambiguates per-row
        // events via `resource_id`. Replace with the dispatch epoch
        // when worker→adapter epoch propagation lands (post-Step-14
        // refactor cycle is the natural slot — see Phase 8 cumulative
        // cargo-review verdict).
        let kept = refilter_and_audit(
            resp.items,
            &scope,
            self.audit_ctx.as_deref(),
            action_type,
            resource_kind,
            0,
        );
        PaginatedResponse {
            items: kept,
            next_marker: resp.next_marker,
            has_more: resp.has_more,
        }
    }
}

// --- JSON wrapper structs (private) ---

#[derive(Deserialize)]
struct NeutronNetworksResponse {
    networks: Vec<Network>,
    networks_links: Option<Vec<Link>>,
}

#[derive(Deserialize)]
struct NeutronNetworkWrapper {
    network: Network,
}

#[derive(Deserialize)]
struct NeutronSubnetsResponse {
    subnets: Vec<Subnet>,
}

#[derive(Deserialize)]
struct NeutronSecurityGroupsResponse {
    security_groups: Vec<SecurityGroup>,
    security_groups_links: Option<Vec<Link>>,
}

#[derive(Deserialize)]
struct NeutronSecurityGroupWrapper {
    security_group: SecurityGroup,
}

#[derive(Deserialize)]
struct NeutronSecurityGroupRuleWrapper {
    security_group_rule: SecurityGroupRule,
}

#[derive(Deserialize)]
struct NeutronFloatingIpsResponse {
    floatingips: Vec<FloatingIp>,
    floatingips_links: Option<Vec<Link>>,
}

#[derive(Deserialize)]
struct NeutronFloatingIpWrapper {
    floatingip: FloatingIp,
}

#[derive(Deserialize)]
struct NeutronPortsResponse {
    ports: Vec<Port>,
}

#[allow(dead_code)] // Used in Unit 14
#[derive(Deserialize)]
struct NeutronAgentsResponse {
    agents: Vec<NetworkAgent>,
}

#[allow(dead_code)] // Used in Unit 14
#[derive(Deserialize)]
struct NeutronAgentWrapper {
    agent: NetworkAgent,
}

// --- Serialize structs ---

#[derive(Serialize)]
struct NetworkCreateBody {
    network: NetworkCreateInner,
}

#[derive(Serialize)]
struct NetworkCreateInner {
    name: String,
    admin_state_up: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    shared: Option<bool>,
    #[serde(rename = "router:external", skip_serializing_if = "Option::is_none")]
    external: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    mtu: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    port_security_enabled: Option<bool>,
}

#[derive(Serialize)]
struct NetworkUpdateBody {
    network: NetworkUpdateInner,
}

#[derive(Serialize)]
struct NetworkUpdateInner {
    #[serde(skip_serializing_if = "Option::is_none")]
    name: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    admin_state_up: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    shared: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    mtu: Option<u32>,
}

#[derive(Serialize)]
struct SecurityGroupCreateBody {
    security_group: SecurityGroupCreateInner,
}

#[derive(Serialize)]
struct SecurityGroupCreateInner {
    name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    description: Option<String>,
}

#[derive(Serialize)]
struct SecurityGroupUpdateBody {
    security_group: SecurityGroupUpdateInner,
}

#[derive(Serialize)]
struct SecurityGroupUpdateInner {
    #[serde(skip_serializing_if = "Option::is_none")]
    name: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    description: Option<String>,
}

#[derive(Serialize)]
struct SecurityGroupRuleCreateBody {
    security_group_rule: SecurityGroupRuleCreateInner,
}

#[derive(Serialize)]
struct SecurityGroupRuleCreateInner {
    security_group_id: String,
    direction: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    protocol: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    port_range_min: Option<u16>,
    #[serde(skip_serializing_if = "Option::is_none")]
    port_range_max: Option<u16>,
    #[serde(skip_serializing_if = "Option::is_none")]
    remote_ip_prefix: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    remote_group_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    ethertype: Option<String>,
}

#[derive(Serialize)]
struct FloatingIpCreateBody {
    floatingip: FloatingIpCreateInner,
}

#[derive(Serialize)]
struct FloatingIpCreateInner {
    floating_network_id: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    port_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    fixed_ip_address: Option<String>,
}

#[derive(Serialize)]
struct FloatingIpUpdateBody {
    floatingip: FloatingIpUpdateInner,
}

#[derive(Serialize)]
struct FloatingIpUpdateInner {
    #[serde(skip_serializing_if = "Option::is_none")]
    port_id: Option<String>,
}

impl RuleDirection {
    fn as_str(&self) -> &str {
        match self {
            RuleDirection::Ingress => "ingress",
            RuleDirection::Egress => "egress",
        }
    }
}

// --- Query builders ---

// BL-P2-085 Step 12: Neutron list endpoints accept `tenant_id={scope}` for
// strict project scoping and `all_tenants=1` (admin-only) to opt out. The two
// are mutually exclusive: `all_tenants=true` wins and `tenant_id` is omitted.
// When neither is set, the query falls back to pagination only (fail-safe —
// the server keeps its default scoping under the current admin token).

fn build_network_query(filter: &NetworkListFilter, pagination: &PaginationParams) -> String {
    let mut parts = Vec::new();
    if filter.all_tenants {
        parts.push("all_tenants=1".to_string());
    } else if let Some(ref tid) = filter.tenant_id {
        parts.push(format!("tenant_id={}", encode_param(tid)));
    }
    append_pagination_parts(&mut parts, pagination);
    parts.join("&")
}

fn build_security_group_query(
    filter: &SecurityGroupListFilter,
    pagination: &PaginationParams,
) -> String {
    let mut parts = Vec::new();
    if filter.all_tenants {
        parts.push("all_tenants=1".to_string());
    } else if let Some(ref tid) = filter.tenant_id {
        parts.push(format!("tenant_id={}", encode_param(tid)));
    }
    append_pagination_parts(&mut parts, pagination);
    parts.join("&")
}

fn build_floating_ip_query(filter: &FloatingIpListFilter, pagination: &PaginationParams) -> String {
    let mut parts = Vec::new();
    if filter.all_tenants {
        parts.push("all_tenants=1".to_string());
    } else if let Some(ref tid) = filter.tenant_id {
        parts.push(format!("tenant_id={}", encode_param(tid)));
    }
    append_pagination_parts(&mut parts, pagination);
    parts.join("&")
}

// --- NeutronPort implementation ---

#[async_trait]
impl NeutronPort for NeutronHttpAdapter {
    // -- Networks --

    async fn list_networks(
        &self,
        filter: &NetworkListFilter,
        pagination: &PaginationParams,
    ) -> ApiResult<PaginatedResponse<Network>> {
        let query = build_network_query(filter, pagination);
        let resp = paginated_list(
            &self.base,
            "/v2.0/networks",
            &query,
            |resp: NeutronNetworksResponse| {
                let next = resp.networks_links.as_deref().and_then(extract_next_marker);
                (resp.networks, next)
            },
        )
        .await?;
        Ok(self.refilter_response(resp, filter.all_tenants, "FetchNetworks", "network"))
    }

    async fn get_network(&self, network_id: &str) -> ApiResult<Network> {
        let req = self
            .base
            .get(&format!("/v2.0/networks/{}", encode_param(network_id)))
            .await?;
        let resp: NeutronNetworkWrapper = self.base.send_json(req).await?;
        Ok(resp.network)
    }

    async fn create_network(&self, params: &NetworkCreateParams) -> ApiResult<Network> {
        let body = NetworkCreateBody {
            network: NetworkCreateInner {
                name: params.name.clone(),
                admin_state_up: params.admin_state_up,
                shared: params.shared,
                external: params.external,
                mtu: params.mtu,
                port_security_enabled: params.port_security_enabled,
            },
        };
        let req = self.base.post("/v2.0/networks").await?.json(&body);
        let resp: NeutronNetworkWrapper = self.base.send_json(req).await?;
        Ok(resp.network)
    }

    async fn update_network(
        &self,
        network_id: &str,
        params: &NetworkUpdateParams,
    ) -> ApiResult<Network> {
        let body = NetworkUpdateBody {
            network: NetworkUpdateInner {
                name: params.name.clone(),
                admin_state_up: params.admin_state_up,
                shared: params.shared,
                mtu: params.mtu,
            },
        };
        let req = self
            .base
            .put(&format!("/v2.0/networks/{}", encode_param(network_id)))
            .await?
            .json(&body);
        let resp: NeutronNetworkWrapper = self.base.send_json(req).await?;
        Ok(resp.network)
    }

    async fn delete_network(&self, network_id: &str) -> ApiResult<()> {
        let req = self
            .base
            .delete(&format!("/v2.0/networks/{}", encode_param(network_id)))
            .await?;
        self.base.send_no_content(req).await
    }

    // -- Subnets --

    async fn list_subnets(&self, network_id: Option<&str>) -> ApiResult<Vec<Subnet>> {
        let path = match network_id {
            Some(id) => format!("/v2.0/subnets?network_id={}", encode_param(id)),
            None => "/v2.0/subnets".to_string(),
        };
        let req = self.base.get(&path).await?;
        let resp: NeutronSubnetsResponse = self.base.send_json(req).await?;
        Ok(resp.subnets)
    }

    // -- Security Groups --

    async fn list_security_groups(
        &self,
        filter: &SecurityGroupListFilter,
        pagination: &PaginationParams,
    ) -> ApiResult<PaginatedResponse<SecurityGroup>> {
        let query = build_security_group_query(filter, pagination);
        let resp = paginated_list(
            &self.base,
            "/v2.0/security-groups",
            &query,
            |resp: NeutronSecurityGroupsResponse| {
                let next = resp
                    .security_groups_links
                    .as_deref()
                    .and_then(extract_next_marker);
                (resp.security_groups, next)
            },
        )
        .await?;
        Ok(self.refilter_response(
            resp,
            filter.all_tenants,
            "FetchSecurityGroups",
            "security_group",
        ))
    }

    async fn get_security_group(&self, sg_id: &str) -> ApiResult<SecurityGroup> {
        let req = self
            .base
            .get(&format!("/v2.0/security-groups/{}", encode_param(sg_id)))
            .await?;
        let resp: NeutronSecurityGroupWrapper = self.base.send_json(req).await?;
        Ok(resp.security_group)
    }

    async fn create_security_group(
        &self,
        params: &SecurityGroupCreateParams,
    ) -> ApiResult<SecurityGroup> {
        let body = SecurityGroupCreateBody {
            security_group: SecurityGroupCreateInner {
                name: params.name.clone(),
                description: params.description.clone(),
            },
        };
        let req = self.base.post("/v2.0/security-groups").await?.json(&body);
        let resp: NeutronSecurityGroupWrapper = self.base.send_json(req).await?;
        Ok(resp.security_group)
    }

    async fn update_security_group(
        &self,
        sg_id: &str,
        params: &SecurityGroupUpdateParams,
    ) -> ApiResult<SecurityGroup> {
        let body = SecurityGroupUpdateBody {
            security_group: SecurityGroupUpdateInner {
                name: params.name.clone(),
                description: params.description.clone(),
            },
        };
        let req = self
            .base
            .put(&format!("/v2.0/security-groups/{}", encode_param(sg_id)))
            .await?
            .json(&body);
        let resp: NeutronSecurityGroupWrapper = self.base.send_json(req).await?;
        Ok(resp.security_group)
    }

    async fn delete_security_group(&self, sg_id: &str) -> ApiResult<()> {
        let req = self
            .base
            .delete(&format!("/v2.0/security-groups/{}", encode_param(sg_id)))
            .await?;
        self.base.send_no_content(req).await
    }

    // -- Security Group Rules --

    async fn create_security_group_rule(
        &self,
        params: &SecurityGroupRuleCreateParams,
    ) -> ApiResult<SecurityGroupRule> {
        let body = SecurityGroupRuleCreateBody {
            security_group_rule: SecurityGroupRuleCreateInner {
                security_group_id: params.security_group_id.clone(),
                direction: params.direction.as_str().to_string(),
                protocol: params.protocol.clone(),
                port_range_min: params.port_range_min,
                port_range_max: params.port_range_max,
                remote_ip_prefix: params.remote_ip_prefix.clone(),
                remote_group_id: params.remote_group_id.clone(),
                ethertype: params.ethertype.clone(),
            },
        };
        let req = self
            .base
            .post("/v2.0/security-group-rules")
            .await?
            .json(&body);
        let resp: NeutronSecurityGroupRuleWrapper = self.base.send_json(req).await?;
        Ok(resp.security_group_rule)
    }

    async fn delete_security_group_rule(&self, rule_id: &str) -> ApiResult<()> {
        let req = self
            .base
            .delete(&format!(
                "/v2.0/security-group-rules/{}",
                encode_param(rule_id)
            ))
            .await?;
        self.base.send_no_content(req).await
    }

    // -- Floating IPs --

    async fn list_floating_ips(
        &self,
        filter: &FloatingIpListFilter,
        pagination: &PaginationParams,
    ) -> ApiResult<PaginatedResponse<FloatingIp>> {
        let query = build_floating_ip_query(filter, pagination);
        let resp = paginated_list(
            &self.base,
            "/v2.0/floatingips",
            &query,
            |resp: NeutronFloatingIpsResponse| {
                let next = resp
                    .floatingips_links
                    .as_deref()
                    .and_then(extract_next_marker);
                (resp.floatingips, next)
            },
        )
        .await?;
        Ok(self.refilter_response(resp, filter.all_tenants, "FetchFloatingIps", "floating_ip"))
    }

    async fn create_floating_ip(&self, params: &FloatingIpCreateParams) -> ApiResult<FloatingIp> {
        let body = FloatingIpCreateBody {
            floatingip: FloatingIpCreateInner {
                floating_network_id: params.floating_network_id.clone(),
                port_id: params.port_id.clone(),
                fixed_ip_address: params.fixed_ip_address.clone(),
            },
        };
        let req = self.base.post("/v2.0/floatingips").await?.json(&body);
        let resp: NeutronFloatingIpWrapper = self.base.send_json(req).await?;
        Ok(resp.floatingip)
    }

    async fn delete_floating_ip(&self, fip_id: &str) -> ApiResult<()> {
        let req = self
            .base
            .delete(&format!("/v2.0/floatingips/{}", encode_param(fip_id)))
            .await?;
        self.base.send_no_content(req).await
    }

    async fn associate_floating_ip(&self, fip_id: &str, port_id: &str) -> ApiResult<FloatingIp> {
        let body = FloatingIpUpdateBody {
            floatingip: FloatingIpUpdateInner {
                port_id: Some(port_id.to_string()),
            },
        };
        let req = self
            .base
            .put(&format!("/v2.0/floatingips/{}", encode_param(fip_id)))
            .await?
            .json(&body);
        let resp: NeutronFloatingIpWrapper = self.base.send_json(req).await?;
        Ok(resp.floatingip)
    }

    // Neutron API requires explicit JSON null for port_id to disassociate (RFC 7386 merge-patch).
    // FloatingIpUpdateInner with skip_serializing_if would omit the field entirely, which Neutron
    // interprets as "no change". We use serde_json::json! to produce {"floatingip":{"port_id":null}}.
    async fn disassociate_floating_ip(&self, fip_id: &str) -> ApiResult<FloatingIp> {
        let body = serde_json::json!({
            "floatingip": { "port_id": null }
        });
        let req = self
            .base
            .put(&format!("/v2.0/floatingips/{}", encode_param(fip_id)))
            .await?
            .json(&body);
        let resp: NeutronFloatingIpWrapper = self.base.send_json(req).await?;
        Ok(resp.floatingip)
    }

    // -- Ports --

    async fn list_ports(&self, device_id: &str) -> ApiResult<Vec<Port>> {
        let path = format!("/v2.0/ports?device_id={}", encode_param(device_id));
        let req = self.base.get(&path).await?;
        let resp: NeutronPortsResponse = self.base.send_json(req).await?;
        Ok(resp.ports)
    }

    // -- Network Agents (stubs — Unit 14) --

    async fn list_network_agents(&self) -> ApiResult<Vec<NetworkAgent>> {
        Err(ApiError::BadRequest("not yet implemented".into()))
    }

    async fn enable_network_agent(&self, _agent_id: &str) -> ApiResult<NetworkAgent> {
        Err(ApiError::BadRequest("not yet implemented".into()))
    }

    async fn disable_network_agent(&self, _agent_id: &str) -> ApiResult<NetworkAgent> {
        Err(ApiError::BadRequest("not yet implemented".into()))
    }

    async fn delete_network_agent(&self, _agent_id: &str) -> ApiResult<()> {
        Err(ApiError::BadRequest("not yet implemented".into()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // --- Networks ---

    #[test]
    fn test_neutron_networks_response_deserialize() {
        let json = r#"{
            "networks": [
                {
                    "id": "net-1",
                    "name": "private",
                    "status": "ACTIVE",
                    "admin_state_up": true,
                    "router:external": false,
                    "shared": false,
                    "mtu": 1500,
                    "subnets": ["sub-1"]
                }
            ],
            "networks_links": [
                {"rel": "next", "href": "http://neutron/v2.0/networks?marker=net-1&limit=50"}
            ]
        }"#;
        let resp: NeutronNetworksResponse = serde_json::from_str(json).unwrap();
        assert_eq!(resp.networks.len(), 1);
        assert_eq!(resp.networks[0].name, "private");
        let marker = extract_next_marker(resp.networks_links.as_deref().unwrap());
        assert_eq!(marker, Some("net-1".to_string()));
    }

    #[test]
    fn test_neutron_network_wrapper_deserialize() {
        let json = r#"{
            "network": {
                "id": "net-1",
                "name": "public",
                "status": "ACTIVE",
                "description": "External network",
                "admin_state_up": true,
                "router:external": true,
                "shared": true,
                "mtu": 9000,
                "port_security_enabled": false,
                "subnets": ["sub-1", "sub-2"]
            }
        }"#;
        let resp: NeutronNetworkWrapper = serde_json::from_str(json).unwrap();
        assert_eq!(resp.network.name, "public");
        assert!(resp.network.external);
        assert!(resp.network.shared);
        assert_eq!(resp.network.mtu, Some(9000));
        assert_eq!(
            resp.network.description.as_deref(),
            Some("External network")
        );
        assert_eq!(resp.network.port_security_enabled, Some(false));
    }

    #[test]
    fn test_network_create_body_serialize() {
        let body = NetworkCreateBody {
            network: NetworkCreateInner {
                name: "test-net".into(),
                admin_state_up: true,
                shared: Some(false),
                external: None,
                mtu: Some(1500),
                port_security_enabled: None,
            },
        };
        let json = serde_json::to_value(&body).unwrap();
        let net = &json["network"];
        assert_eq!(net["name"], "test-net");
        assert_eq!(net["admin_state_up"], true);
        assert_eq!(net["shared"], false);
        assert!(net.get("router:external").is_none());
        assert_eq!(net["mtu"], 1500);
        assert!(net.get("port_security_enabled").is_none());
    }

    // --- Subnets ---

    #[test]
    fn test_neutron_subnets_response_deserialize() {
        let json = r#"{
            "subnets": [
                {
                    "id": "sub-1",
                    "name": "private-subnet",
                    "network_id": "net-1",
                    "cidr": "10.0.0.0/24",
                    "ip_version": 4,
                    "gateway_ip": "10.0.0.1"
                }
            ]
        }"#;
        let resp: NeutronSubnetsResponse = serde_json::from_str(json).unwrap();
        assert_eq!(resp.subnets.len(), 1);
        assert_eq!(resp.subnets[0].cidr, "10.0.0.0/24");
        assert_eq!(resp.subnets[0].gateway_ip.as_deref(), Some("10.0.0.1"));
    }

    // --- Security Groups ---

    #[test]
    fn test_neutron_sg_response_deserialize() {
        let json = r#"{
            "security_groups": [
                {
                    "id": "sg-1",
                    "name": "default",
                    "description": "Default SG",
                    "security_group_rules": []
                }
            ]
        }"#;
        let resp: NeutronSecurityGroupsResponse = serde_json::from_str(json).unwrap();
        assert_eq!(resp.security_groups.len(), 1);
        assert_eq!(resp.security_groups[0].name, "default");
    }

    #[test]
    fn test_sg_create_body_serialize() {
        let body = SecurityGroupCreateBody {
            security_group: SecurityGroupCreateInner {
                name: "web-sg".into(),
                description: Some("Web security group".into()),
            },
        };
        let json = serde_json::to_value(&body).unwrap();
        let sg = &json["security_group"];
        assert_eq!(sg["name"], "web-sg");
        assert_eq!(sg["description"], "Web security group");
    }

    #[test]
    fn test_rule_create_body_serialize() {
        let body = SecurityGroupRuleCreateBody {
            security_group_rule: SecurityGroupRuleCreateInner {
                security_group_id: "sg-1".into(),
                direction: "ingress".into(),
                protocol: Some("tcp".into()),
                port_range_min: Some(22),
                port_range_max: Some(22),
                remote_ip_prefix: Some("0.0.0.0/0".into()),
                remote_group_id: None,
                ethertype: Some("IPv4".into()),
            },
        };
        let json = serde_json::to_value(&body).unwrap();
        let rule = &json["security_group_rule"];
        assert_eq!(rule["security_group_id"], "sg-1");
        assert_eq!(rule["direction"], "ingress");
        assert_eq!(rule["protocol"], "tcp");
        assert_eq!(rule["port_range_min"], 22);
        assert_eq!(rule["port_range_max"], 22);
        assert_eq!(rule["remote_ip_prefix"], "0.0.0.0/0");
        assert!(rule.get("remote_group_id").is_none());
        assert_eq!(rule["ethertype"], "IPv4");
    }

    #[test]
    fn test_rule_direction_as_str() {
        assert_eq!(RuleDirection::Ingress.as_str(), "ingress");
        assert_eq!(RuleDirection::Egress.as_str(), "egress");
    }

    // --- Floating IPs ---

    #[test]
    fn test_neutron_fip_response_deserialize() {
        let json = r#"{
            "floatingips": [
                {
                    "id": "fip-1",
                    "floating_ip_address": "203.0.113.10",
                    "status": "ACTIVE",
                    "port_id": "port-1",
                    "floating_network_id": "ext-net-1",
                    "fixed_ip_address": "10.0.0.5",
                    "router_id": "router-1"
                }
            ]
        }"#;
        let resp: NeutronFloatingIpsResponse = serde_json::from_str(json).unwrap();
        assert_eq!(resp.floatingips.len(), 1);
        assert_eq!(resp.floatingips[0].floating_ip_address, "203.0.113.10");
    }

    #[test]
    fn test_fip_create_body_serialize() {
        let body = FloatingIpCreateBody {
            floatingip: FloatingIpCreateInner {
                floating_network_id: "ext-net-1".into(),
                port_id: None,
                fixed_ip_address: None,
            },
        };
        let json = serde_json::to_value(&body).unwrap();
        let fip = &json["floatingip"];
        assert_eq!(fip["floating_network_id"], "ext-net-1");
        assert!(fip.get("port_id").is_none());
        assert!(fip.get("fixed_ip_address").is_none());
    }

    #[test]
    fn test_fip_associate_body_serialize() {
        let body = FloatingIpUpdateBody {
            floatingip: FloatingIpUpdateInner {
                port_id: Some("port-1".into()),
            },
        };
        let json = serde_json::to_value(&body).unwrap();
        assert_eq!(json["floatingip"]["port_id"], "port-1");
    }

    #[test]
    fn test_network_deserialize_with_tenant_id() {
        let json = r#"{
            "network": {
                "id": "net-1",
                "name": "admin-net",
                "status": "ACTIVE",
                "admin_state_up": true,
                "subnets": [],
                "tenant_id": "proj-abc-123"
            }
        }"#;
        let resp: NeutronNetworkWrapper = serde_json::from_str(json).unwrap();
        assert_eq!(resp.network.tenant_id.as_deref(), Some("proj-abc-123"));
    }

    #[test]
    fn test_floating_ip_deserialize_with_tenant_id() {
        let json = r#"{
            "floatingips": [{
                "id": "fip-1",
                "floating_ip_address": "203.0.113.10",
                "status": "ACTIVE",
                "floating_network_id": "ext-1",
                "tenant_id": "proj-xyz"
            }]
        }"#;
        let resp: NeutronFloatingIpsResponse = serde_json::from_str(json).unwrap();
        assert_eq!(resp.floatingips[0].tenant_id.as_deref(), Some("proj-xyz"));
    }

    // --- Query builders ---
    // BL-P2-085 Step 12: tenant_id injection / all_tenants=1 branch ---
    // Policy:
    //   - filter.all_tenants == true   → query contains "all_tenants=1", no tenant_id
    //   - filter.all_tenants == false && filter.tenant_id = Some(scope)
    //                                  → query contains "tenant_id={scope}", no all_tenants
    //   - filter.all_tenants == false && filter.tenant_id = None
    //                                  → query has neither (no-op fail-safe; pagination only)

    #[test]
    fn test_build_network_query_injects_tenant_id_when_all_tenants_false() {
        let filter = NetworkListFilter {
            all_tenants: false,
            tenant_id: Some("proj-A".into()),
        };
        let pagination = PaginationParams::default();
        let query = build_network_query(&filter, &pagination);
        assert!(
            query.contains("tenant_id=proj-A"),
            "expected tenant_id=proj-A in query, got: {query}"
        );
        assert!(!query.contains("all_tenants"));
    }

    #[test]
    fn test_build_network_query_all_tenants_true_skips_tenant_id() {
        let filter = NetworkListFilter {
            all_tenants: true,
            tenant_id: Some("proj-A".into()),
        };
        let pagination = PaginationParams::default();
        let query = build_network_query(&filter, &pagination);
        assert!(
            query.contains("all_tenants=1"),
            "expected all_tenants=1 in query, got: {query}"
        );
        assert!(!query.contains("tenant_id"));
    }

    #[test]
    fn test_build_security_group_query_injects_tenant_id_when_all_tenants_false() {
        let filter = SecurityGroupListFilter {
            all_tenants: false,
            tenant_id: Some("proj-B".into()),
        };
        let pagination = PaginationParams::default();
        let query = build_security_group_query(&filter, &pagination);
        assert!(
            query.contains("tenant_id=proj-B"),
            "expected tenant_id=proj-B in query, got: {query}"
        );
        assert!(!query.contains("all_tenants"));
    }

    #[test]
    fn test_build_security_group_query_all_tenants_true_skips_tenant_id() {
        let filter = SecurityGroupListFilter {
            all_tenants: true,
            tenant_id: Some("proj-B".into()),
        };
        let pagination = PaginationParams::default();
        let query = build_security_group_query(&filter, &pagination);
        assert!(
            query.contains("all_tenants=1"),
            "expected all_tenants=1 in query, got: {query}"
        );
        assert!(!query.contains("tenant_id"));
    }

    #[test]
    fn test_build_security_group_query_no_op_when_no_tenant_id_no_all_tenants() {
        let filter = SecurityGroupListFilter {
            all_tenants: false,
            tenant_id: None,
        };
        let pagination = PaginationParams::default();
        let query = build_security_group_query(&filter, &pagination);
        assert!(!query.contains("tenant_id"));
        assert!(!query.contains("all_tenants"));
    }

    #[test]
    fn test_build_floating_ip_query_injects_tenant_id_when_all_tenants_false() {
        let filter = FloatingIpListFilter {
            all_tenants: false,
            tenant_id: Some("proj-C".into()),
        };
        let pagination = PaginationParams::default();
        let query = build_floating_ip_query(&filter, &pagination);
        assert!(
            query.contains("tenant_id=proj-C"),
            "expected tenant_id=proj-C in query, got: {query}"
        );
        assert!(!query.contains("all_tenants"));
    }

    #[test]
    fn test_build_floating_ip_query_all_tenants_true_skips_tenant_id() {
        let filter = FloatingIpListFilter {
            all_tenants: true,
            tenant_id: Some("proj-C".into()),
        };
        let pagination = PaginationParams::default();
        let query = build_floating_ip_query(&filter, &pagination);
        assert!(
            query.contains("all_tenants=1"),
            "expected all_tenants=1 in query, got: {query}"
        );
        assert!(!query.contains("tenant_id"));
    }

    #[test]
    fn test_networks_response_no_links() {
        let json = r#"{
            "networks": [
                {
                    "id": "net-1",
                    "name": "test",
                    "status": "ACTIVE",
                    "admin_state_up": true,
                    "subnets": []
                }
            ]
        }"#;
        let resp: NeutronNetworksResponse = serde_json::from_str(json).unwrap();
        assert_eq!(resp.networks.len(), 1);
        assert!(resp.networks_links.is_none());
    }
}
