use serde::Deserialize;

#[derive(Debug, Clone, Deserialize)]
pub struct Network {
    pub id: String,
    pub name: String,
    pub status: String,
    pub admin_state_up: bool,
    #[serde(rename = "router:external", default)]
    pub external: bool,
    #[serde(default)]
    pub shared: bool,
    pub mtu: Option<u32>,
    #[serde(default)]
    pub subnets: Vec<String>,
    #[serde(rename = "provider:network_type")]
    pub provider_network_type: Option<String>,
    #[serde(rename = "provider:physical_network")]
    pub provider_physical_network: Option<String>,
    #[serde(rename = "provider:segmentation_id")]
    pub provider_segmentation_id: Option<u32>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct SecurityGroup {
    pub id: String,
    pub name: String,
    pub description: Option<String>,
    #[serde(default)]
    pub security_group_rules: Vec<SecurityGroupRule>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct SecurityGroupRule {
    pub id: String,
    pub direction: String,
    pub protocol: Option<String>,
    pub port_range_min: Option<u16>,
    pub port_range_max: Option<u16>,
    pub remote_ip_prefix: Option<String>,
    pub remote_group_id: Option<String>,
    pub ethertype: String,
}

#[derive(Debug, Clone, Deserialize)]
pub struct FloatingIp {
    pub id: String,
    pub floating_ip_address: String,
    pub status: String,
    pub port_id: Option<String>,
    pub floating_network_id: String,
    pub fixed_ip_address: Option<String>,
    pub router_id: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct NetworkAgent {
    pub id: String,
    pub agent_type: String,
    pub host: String,
    pub admin_state_up: bool,
    pub alive: bool,
    pub binary: String,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_network_deserialize() {
        let json = r#"{
            "id": "net-001",
            "name": "private-net",
            "status": "ACTIVE",
            "admin_state_up": true,
            "router:external": false,
            "shared": false,
            "mtu": 1500,
            "subnets": ["subnet-1", "subnet-2"],
            "provider:network_type": "vxlan",
            "provider:physical_network": null,
            "provider:segmentation_id": 100
        }"#;
        let net: Network = serde_json::from_str(json).unwrap();
        assert_eq!(net.name, "private-net");
        assert!(!net.external);
        assert_eq!(net.mtu, Some(1500));
        assert_eq!(net.provider_network_type.as_deref(), Some("vxlan"));
        assert_eq!(net.provider_segmentation_id, Some(100));
        assert_eq!(net.subnets.len(), 2);
    }

    #[test]
    fn test_security_group_deserialize() {
        let json = r#"{
            "id": "sg-001",
            "name": "default",
            "description": "Default SG",
            "security_group_rules": [
                {
                    "id": "rule-1",
                    "direction": "ingress",
                    "protocol": "tcp",
                    "port_range_min": 22,
                    "port_range_max": 22,
                    "remote_ip_prefix": "0.0.0.0/0",
                    "remote_group_id": null,
                    "ethertype": "IPv4"
                }
            ]
        }"#;
        let sg: SecurityGroup = serde_json::from_str(json).unwrap();
        assert_eq!(sg.name, "default");
        assert_eq!(sg.security_group_rules.len(), 1);
        let rule = &sg.security_group_rules[0];
        assert_eq!(rule.direction, "ingress");
        assert_eq!(rule.protocol.as_deref(), Some("tcp"));
        assert_eq!(rule.port_range_min, Some(22));
    }

    #[test]
    fn test_floating_ip_deserialize() {
        let json = r#"{
            "id": "fip-001",
            "floating_ip_address": "203.0.113.10",
            "status": "ACTIVE",
            "port_id": "port-123",
            "floating_network_id": "ext-net-1",
            "fixed_ip_address": "10.0.0.5",
            "router_id": "router-1"
        }"#;
        let fip: FloatingIp = serde_json::from_str(json).unwrap();
        assert_eq!(fip.floating_ip_address, "203.0.113.10");
        assert_eq!(fip.status, "ACTIVE");
        assert_eq!(fip.port_id.as_deref(), Some("port-123"));
    }

    #[test]
    fn test_network_agent_deserialize() {
        let json = r#"{
            "id": "agent-001",
            "agent_type": "Open vSwitch agent",
            "host": "network-01",
            "admin_state_up": true,
            "alive": true,
            "binary": "neutron-openvswitch-agent"
        }"#;
        let agent: NetworkAgent = serde_json::from_str(json).unwrap();
        assert_eq!(agent.agent_type, "Open vSwitch agent");
        assert!(agent.alive);
        assert!(agent.admin_state_up);
    }
}
