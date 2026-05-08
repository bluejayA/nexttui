use serde::Deserialize;

#[derive(Debug, Clone, Deserialize)]
pub struct Network {
    pub id: String,
    pub name: String,
    pub status: String,
    pub description: Option<String>,
    pub admin_state_up: bool,
    #[serde(rename = "router:external", default)]
    pub external: bool,
    #[serde(default)]
    pub shared: bool,
    pub mtu: Option<u32>,
    #[serde(default)]
    pub port_security_enabled: Option<bool>,
    #[serde(default)]
    pub subnets: Vec<String>,
    #[serde(rename = "provider:network_type")]
    pub provider_network_type: Option<String>,
    #[serde(rename = "provider:physical_network")]
    pub provider_physical_network: Option<String>,
    #[serde(rename = "provider:segmentation_id")]
    pub provider_segmentation_id: Option<u32>,
    #[serde(default)]
    pub tenant_id: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct SecurityGroup {
    pub id: String,
    pub name: String,
    pub description: Option<String>,
    #[serde(default)]
    pub security_group_rules: Vec<SecurityGroupRule>,
    #[serde(default)]
    pub tenant_id: Option<String>,
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
    #[serde(default)]
    pub tenant_id: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct Port {
    pub id: String,
    pub name: Option<String>,
    pub network_id: String,
    pub fixed_ips: Vec<FixedIp>,
    pub device_id: Option<String>,
    pub device_owner: Option<String>,
    pub status: String,
    #[serde(default)]
    pub tenant_id: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct FixedIp {
    pub subnet_id: String,
    pub ip_address: String,
}

impl Port {
    pub fn display_label(&self, networks: &[Network]) -> String {
        let ip = self
            .fixed_ips
            .first()
            .map(|f| f.ip_address.as_str())
            .unwrap_or("no-ip");
        let net = networks.iter().find(|n| n.id == self.network_id);
        let net_name = net.map(|n| n.name.as_str()).unwrap_or("unknown-net");
        let ext_badge = if net.is_some_and(|n| n.external) {
            " [EXT]"
        } else {
            ""
        };
        format!("{ip} on {net_name}{ext_badge}")
    }
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

// -- Port bindings (binding-extended Neutron API) --

#[derive(Debug, Clone, Deserialize)]
pub struct PortBinding {
    pub host: String,
    pub vif_type: String,
    #[serde(default)]
    pub vnic_type: Option<String>,
    pub status: BindingStatus,
    #[serde(default)]
    pub profile: PortBindingProfile,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "UPPERCASE")]
pub enum BindingStatus {
    Active,
    Inactive,
    #[serde(other)]
    Unknown,
}

#[derive(Debug, Clone, Default, Deserialize)]
pub struct PortBindingProfile {
    #[serde(default)]
    pub migrating_to: Option<String>,
}

impl PortBinding {
    /// True iff this binding is the leftover of a failed/aborted live-migration:
    /// `INACTIVE` status combined with a `migrating_to` profile entry. We
    /// deliberately require both — `INACTIVE` alone can occur on legitimately
    /// disabled standby bindings, and `migrating_to` alone is normal during
    /// an in-progress migration.
    pub fn is_stale_migration_remnant(&self) -> bool {
        self.status == BindingStatus::Inactive && self.profile.migrating_to.is_some()
    }
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
            "description": "Private network",
            "admin_state_up": true,
            "router:external": false,
            "shared": false,
            "mtu": 1500,
            "port_security_enabled": true,
            "subnets": ["subnet-1", "subnet-2"],
            "provider:network_type": "vxlan",
            "provider:physical_network": null,
            "provider:segmentation_id": 100
        }"#;
        let net: Network = serde_json::from_str(json).unwrap();
        assert_eq!(net.name, "private-net");
        assert!(!net.external);
        assert_eq!(net.mtu, Some(1500));
        assert_eq!(net.description.as_deref(), Some("Private network"));
        assert_eq!(net.port_security_enabled, Some(true));
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
    fn test_port_deserialize() {
        let json = r#"{
            "id": "port-001",
            "name": "my-port",
            "network_id": "net-001",
            "fixed_ips": [
                {"subnet_id": "sub-1", "ip_address": "10.0.0.5"}
            ],
            "device_id": "srv-1",
            "device_owner": "compute:az1",
            "status": "ACTIVE",
            "tenant_id": "proj-1"
        }"#;
        let port: Port = serde_json::from_str(json).unwrap();
        assert_eq!(port.id, "port-001");
        assert_eq!(port.name.as_deref(), Some("my-port"));
        assert_eq!(port.network_id, "net-001");
        assert_eq!(port.fixed_ips.len(), 1);
        assert_eq!(port.fixed_ips[0].ip_address, "10.0.0.5");
        assert_eq!(port.device_id.as_deref(), Some("srv-1"));
        assert_eq!(port.status, "ACTIVE");
    }

    #[test]
    fn test_port_display_label_with_network() {
        let port = Port {
            id: "port-1".into(),
            name: None,
            network_id: "net-1".into(),
            fixed_ips: vec![FixedIp {
                subnet_id: "sub-1".into(),
                ip_address: "10.0.0.5".into(),
            }],
            device_id: None,
            device_owner: None,
            status: "ACTIVE".into(),
            tenant_id: None,
        };
        let networks = vec![Network {
            id: "net-1".into(),
            name: "private-net".into(),
            status: "ACTIVE".into(),
            description: None,
            admin_state_up: true,
            external: false,
            shared: false,
            mtu: None,
            port_security_enabled: None,
            subnets: vec![],
            provider_network_type: None,
            provider_physical_network: None,
            provider_segmentation_id: None,
            tenant_id: None,
        }];
        assert_eq!(port.display_label(&networks), "10.0.0.5 on private-net");
    }

    #[test]
    fn test_port_display_label_external_network() {
        let port = Port {
            id: "port-1".into(),
            name: None,
            network_id: "ext-1".into(),
            fixed_ips: vec![FixedIp {
                subnet_id: "sub-1".into(),
                ip_address: "203.0.113.10".into(),
            }],
            device_id: None,
            device_owner: None,
            status: "ACTIVE".into(),
            tenant_id: None,
        };
        let networks = vec![Network {
            id: "ext-1".into(),
            name: "public".into(),
            status: "ACTIVE".into(),
            description: None,
            admin_state_up: true,
            external: true,
            shared: false,
            mtu: None,
            port_security_enabled: None,
            subnets: vec![],
            provider_network_type: None,
            provider_physical_network: None,
            provider_segmentation_id: None,
            tenant_id: None,
        }];
        assert_eq!(
            port.display_label(&networks),
            "203.0.113.10 on public [EXT]"
        );
    }

    #[test]
    fn test_port_display_label_no_ip_unknown_net() {
        let port = Port {
            id: "port-1".into(),
            name: None,
            network_id: "unknown-net".into(),
            fixed_ips: vec![],
            device_id: None,
            device_owner: None,
            status: "ACTIVE".into(),
            tenant_id: None,
        };
        assert_eq!(port.display_label(&[]), "no-ip on unknown-net");
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

    // -- BL-P2-086: PortBinding (binding-extended Neutron API) --

    #[test]
    fn test_port_binding_deserialize_active() {
        let json = r#"{
            "host": "lima-devstack-cp1",
            "vif_type": "ovs",
            "vnic_type": "normal",
            "status": "ACTIVE",
            "profile": {"os_vif_delegation": true}
        }"#;
        let binding: PortBinding = serde_json::from_str(json).unwrap();
        assert_eq!(binding.host, "lima-devstack-cp1");
        assert_eq!(binding.vif_type, "ovs");
        assert_eq!(binding.status, BindingStatus::Active);
        assert!(binding.profile.migrating_to.is_none());
        assert!(!binding.is_stale_migration_remnant());
    }

    #[test]
    fn test_port_binding_deserialize_inactive_with_migrating_to() {
        // Real payload from a stale binding left by an aborted live-migration.
        let json = r#"{
            "host": "lima-devstack-cp2",
            "vif_type": "unbound",
            "vnic_type": "normal",
            "status": "INACTIVE",
            "profile": {"os_vif_delegation": true, "migrating_to": "lima-devstack-cp1"}
        }"#;
        let binding: PortBinding = serde_json::from_str(json).unwrap();
        assert_eq!(binding.host, "lima-devstack-cp2");
        assert_eq!(binding.vif_type, "unbound");
        assert_eq!(binding.status, BindingStatus::Inactive);
        assert_eq!(
            binding.profile.migrating_to.as_deref(),
            Some("lima-devstack-cp1")
        );
        // The combination INACTIVE + migrating_to is the stale-remnant signature
        // we want to surface to the user.
        assert!(binding.is_stale_migration_remnant());
    }

    #[test]
    fn test_port_binding_deserialize_missing_profile_uses_default() {
        // Some Neutron payloads omit profile entirely.
        let json = r#"{
            "host": "h1",
            "vif_type": "ovs",
            "status": "ACTIVE"
        }"#;
        let binding: PortBinding = serde_json::from_str(json).unwrap();
        assert!(binding.profile.migrating_to.is_none());
        assert!(!binding.is_stale_migration_remnant());
    }

    #[test]
    fn test_port_binding_inactive_without_migrating_to_is_not_stale() {
        // INACTIVE alone is not enough — we only flag the specific
        // INACTIVE + migrating_to combo to avoid false positives on
        // legitimately-disabled standby bindings.
        let json = r#"{
            "host": "h2",
            "vif_type": "unbound",
            "status": "INACTIVE",
            "profile": {}
        }"#;
        let binding: PortBinding = serde_json::from_str(json).unwrap();
        assert_eq!(binding.status, BindingStatus::Inactive);
        assert!(!binding.is_stale_migration_remnant());
    }

    #[test]
    fn test_binding_status_unknown_variant() {
        // Future-proof: unrecognized status strings should not panic
        // — they map to BindingStatus::Unknown.
        let json = r#"{
            "host": "h3",
            "vif_type": "ovs",
            "status": "PROVISIONING"
        }"#;
        let binding: PortBinding = serde_json::from_str(json).unwrap();
        assert_eq!(binding.status, BindingStatus::Unknown);
    }
}
