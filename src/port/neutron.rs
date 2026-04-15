use async_trait::async_trait;

use super::error::ApiResult;
use super::types::*;
use crate::models::neutron::{
    FloatingIp, Network, NetworkAgent, Port, SecurityGroup, SecurityGroupRule,
};

#[async_trait]
pub trait NeutronPort: Send + Sync {
    // Networks
    async fn list_networks(
        &self,
        filter: &NetworkListFilter,
        pagination: &PaginationParams,
    ) -> ApiResult<PaginatedResponse<Network>>;
    async fn get_network(&self, network_id: &str) -> ApiResult<Network>;
    async fn create_network(&self, params: &NetworkCreateParams) -> ApiResult<Network>;
    async fn update_network(
        &self,
        network_id: &str,
        params: &NetworkUpdateParams,
    ) -> ApiResult<Network>;
    async fn delete_network(&self, network_id: &str) -> ApiResult<()>;

    // Subnets
    async fn list_subnets(&self, network_id: Option<&str>) -> ApiResult<Vec<Subnet>>;

    // Security Groups
    async fn list_security_groups(
        &self,
        filter: &SecurityGroupListFilter,
        pagination: &PaginationParams,
    ) -> ApiResult<PaginatedResponse<SecurityGroup>>;
    async fn get_security_group(&self, sg_id: &str) -> ApiResult<SecurityGroup>;
    async fn create_security_group(
        &self,
        params: &SecurityGroupCreateParams,
    ) -> ApiResult<SecurityGroup>;
    async fn update_security_group(
        &self,
        sg_id: &str,
        params: &SecurityGroupUpdateParams,
    ) -> ApiResult<SecurityGroup>;
    async fn delete_security_group(&self, sg_id: &str) -> ApiResult<()>;

    // Security Group Rules
    async fn create_security_group_rule(
        &self,
        params: &SecurityGroupRuleCreateParams,
    ) -> ApiResult<SecurityGroupRule>;
    async fn delete_security_group_rule(&self, rule_id: &str) -> ApiResult<()>;

    // Floating IPs
    async fn list_floating_ips(
        &self,
        filter: &FloatingIpListFilter,
        pagination: &PaginationParams,
    ) -> ApiResult<PaginatedResponse<FloatingIp>>;
    async fn create_floating_ip(&self, params: &FloatingIpCreateParams) -> ApiResult<FloatingIp>;
    async fn delete_floating_ip(&self, fip_id: &str) -> ApiResult<()>;
    async fn associate_floating_ip(&self, fip_id: &str, port_id: &str) -> ApiResult<FloatingIp>;
    async fn disassociate_floating_ip(&self, fip_id: &str) -> ApiResult<FloatingIp>;

    // Ports
    async fn list_ports(&self, device_id: &str) -> ApiResult<Vec<Port>>;

    // Network Agents
    async fn list_network_agents(&self) -> ApiResult<Vec<NetworkAgent>>;
    async fn enable_network_agent(&self, agent_id: &str) -> ApiResult<NetworkAgent>;
    async fn disable_network_agent(&self, agent_id: &str) -> ApiResult<NetworkAgent>;
    async fn delete_network_agent(&self, agent_id: &str) -> ApiResult<()>;
}
