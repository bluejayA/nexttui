use serde::{Deserialize, Deserializer};
use std::collections::HashMap;

/// Deserialize a value that may be a string or integer into a String
fn string_or_int<'de, D>(deserializer: D) -> std::result::Result<String, D::Error>
where
    D: Deserializer<'de>,
{
    let value: serde_json::Value = Deserialize::deserialize(deserializer)?;
    match value {
        serde_json::Value::String(s) => Ok(s),
        serde_json::Value::Number(n) => Ok(n.to_string()),
        _ => Err(serde::de::Error::custom("expected string or number")),
    }
}

#[derive(Debug, Clone, Deserialize)]
pub struct Server {
    pub id: String,
    pub name: String,
    pub status: String,
    pub addresses: HashMap<String, Vec<Address>>,
    pub flavor: FlavorRef,
    pub image: Option<ImageRef>,
    pub key_name: Option<String>,
    #[serde(rename = "OS-EXT-AZ:availability_zone")]
    pub availability_zone: Option<String>,
    pub created: String,
    pub updated: Option<String>,
    pub tenant_id: Option<String>,
    #[serde(rename = "hostId")]
    pub host_id: Option<String>,
    #[serde(rename = "OS-EXT-SRV-ATTR:host")]
    pub host: Option<String>,
    #[serde(default, rename = "os-extended-volumes:volumes_attached")]
    pub volumes_attached: Vec<AttachedVolume>,
    #[serde(default)]
    pub security_groups: Vec<ServerSecurityGroup>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct ServerSecurityGroup {
    pub name: String,
}

#[derive(Debug, Clone, Deserialize)]
pub struct AttachedVolume {
    pub id: String,
    #[serde(default)]
    pub delete_on_termination: bool,
}

#[derive(Debug, Clone, Deserialize)]
pub struct Address {
    pub addr: String,
    pub version: u8,
    #[serde(rename = "OS-EXT-IPS-MAC:mac_addr")]
    pub mac_addr: Option<String>,
    #[serde(rename = "OS-EXT-IPS:type")]
    pub ip_type: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct FlavorRef {
    pub id: String,
    pub original_name: Option<String>,
    pub vcpus: Option<u32>,
    pub ram: Option<u32>,
    pub disk: Option<u32>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct ImageRef {
    pub id: String,
}

#[derive(Debug, Clone, Deserialize)]
pub struct Flavor {
    pub id: String,
    pub name: String,
    pub vcpus: u32,
    pub ram: u32,
    pub disk: u32,
    #[serde(rename = "os-flavor-access:is_public", default = "default_true")]
    pub is_public: bool,
}

fn default_true() -> bool {
    true
}

#[derive(Debug, Clone, Deserialize)]
pub struct Aggregate {
    pub id: i64,
    pub name: String,
    pub availability_zone: Option<String>,
    #[serde(default)]
    pub hosts: Vec<String>,
    #[serde(default)]
    pub metadata: HashMap<String, String>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct ComputeService {
    #[serde(deserialize_with = "string_or_int")]
    pub id: String,
    pub binary: String,
    pub host: String,
    pub status: String,
    pub state: String,
    pub updated_at: Option<String>,
    pub disabled_reason: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct Hypervisor {
    #[serde(deserialize_with = "string_or_int")]
    pub id: String,
    pub hypervisor_hostname: String,
    pub hypervisor_type: String,
    pub vcpus: u32,
    pub vcpus_used: u32,
    pub memory_mb: u32,
    pub memory_mb_used: u32,
    pub local_gb: u32,
    pub local_gb_used: u32,
    pub running_vms: u32,
    pub status: String,
    pub state: String,
}

#[derive(Debug, Clone, Deserialize)]
pub struct ServerMigration {
    pub id: i64,
    pub status: String,
    pub source_compute: String,
    pub dest_compute: String,
    #[serde(default)]
    pub memory_total_bytes: Option<i64>,
    #[serde(default)]
    pub memory_processed_bytes: Option<i64>,
    #[serde(default)]
    pub memory_remaining_bytes: Option<i64>,
    #[serde(default)]
    pub disk_total_bytes: Option<i64>,
    #[serde(default)]
    pub disk_processed_bytes: Option<i64>,
    #[serde(default)]
    pub disk_remaining_bytes: Option<i64>,
    #[serde(default)]
    pub created_at: Option<String>,
    #[serde(default)]
    pub updated_at: Option<String>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_server_deserialize() {
        let json = r#"{
            "id": "srv-001",
            "name": "web-01",
            "status": "ACTIVE",
            "addresses": {
                "private": [
                    {"addr": "10.0.0.5", "version": 4, "OS-EXT-IPS:type": "fixed", "OS-EXT-IPS-MAC:mac_addr": "fa:16:3e:aa:bb:cc"}
                ]
            },
            "flavor": {"id": "flv-1", "vcpus": 2, "ram": 4096, "disk": 40},
            "image": {"id": "img-1"},
            "key_name": "mykey",
            "OS-EXT-AZ:availability_zone": "nova",
            "created": "2026-01-01T00:00:00Z",
            "updated": null,
            "tenant_id": "proj-1",
            "hostId": "host-hash",
            "OS-EXT-SRV-ATTR:host": "compute-01"
        }"#;
        let server: Server = serde_json::from_str(json).unwrap();
        assert_eq!(server.id, "srv-001");
        assert_eq!(server.name, "web-01");
        assert_eq!(server.status, "ACTIVE");
        assert_eq!(server.addresses["private"].len(), 1);
        assert_eq!(
            server.addresses["private"][0].ip_type.as_deref(),
            Some("fixed")
        );
        assert_eq!(server.flavor.vcpus, Some(2));
        assert_eq!(server.availability_zone.as_deref(), Some("nova"));
        assert_eq!(server.host.as_deref(), Some("compute-01"));
        // security_groups defaults to empty when absent
        assert!(server.security_groups.is_empty());
    }

    #[test]
    fn test_server_deserialize_with_security_groups() {
        let json = r#"{
            "id": "srv-002",
            "name": "web-02",
            "status": "ACTIVE",
            "addresses": {},
            "flavor": {"id": "flv-1"},
            "created": "2026-01-01T00:00:00Z",
            "security_groups": [
                {"name": "default"},
                {"name": "web-sg"}
            ]
        }"#;
        let server: Server = serde_json::from_str(json).unwrap();
        assert_eq!(server.security_groups.len(), 2);
        assert_eq!(server.security_groups[0].name, "default");
        assert_eq!(server.security_groups[1].name, "web-sg");
    }

    #[test]
    fn test_flavor_deserialize() {
        let json = r#"{
            "id": "flv-1",
            "name": "m1.small",
            "vcpus": 1,
            "ram": 2048,
            "disk": 20,
            "os-flavor-access:is_public": true
        }"#;
        let flavor: Flavor = serde_json::from_str(json).unwrap();
        assert_eq!(flavor.name, "m1.small");
        assert!(flavor.is_public);
    }

    #[test]
    fn test_aggregate_deserialize() {
        let json = r#"{
            "id": 1,
            "name": "az1-agg",
            "availability_zone": "az1",
            "hosts": ["compute-01", "compute-02"],
            "metadata": {"ssd": "true"}
        }"#;
        let agg: Aggregate = serde_json::from_str(json).unwrap();
        assert_eq!(agg.name, "az1-agg");
        assert_eq!(agg.hosts.len(), 2);
        assert_eq!(agg.metadata.get("ssd").unwrap(), "true");
    }

    #[test]
    fn test_compute_service_deserialize() {
        let json = r#"{
            "id": "svc-1",
            "binary": "nova-compute",
            "host": "compute-01",
            "status": "enabled",
            "state": "up",
            "updated_at": "2026-01-01T00:00:00Z",
            "disabled_reason": null
        }"#;
        let svc: ComputeService = serde_json::from_str(json).unwrap();
        assert_eq!(svc.binary, "nova-compute");
        assert_eq!(svc.status, "enabled");
        assert_eq!(svc.state, "up");
    }

    #[test]
    fn test_server_migration_deserialize() {
        let json = r#"{
            "id": 42,
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
        }"#;
        let mig: ServerMigration = serde_json::from_str(json).unwrap();
        assert_eq!(mig.id, 42);
        assert_eq!(mig.status, "running");
        assert_eq!(mig.source_compute, "compute-01");
        assert_eq!(mig.dest_compute, "compute-02");
        assert_eq!(mig.memory_total_bytes, Some(1_073_741_824));
        assert_eq!(mig.disk_processed_bytes, Some(5_368_709_120));
    }

    #[test]
    fn test_server_migration_deserialize_minimal() {
        let json = r#"{
            "id": 1,
            "status": "completed",
            "source_compute": "node-a",
            "dest_compute": "node-b"
        }"#;
        let mig: ServerMigration = serde_json::from_str(json).unwrap();
        assert_eq!(mig.id, 1);
        assert_eq!(mig.status, "completed");
        assert!(mig.memory_total_bytes.is_none());
        assert!(mig.created_at.is_none());
    }

    #[test]
    fn test_hypervisor_deserialize() {
        let json = r#"{
            "id": 1,
            "hypervisor_hostname": "compute-01.local",
            "hypervisor_type": "QEMU",
            "vcpus": 64,
            "vcpus_used": 32,
            "memory_mb": 131072,
            "memory_mb_used": 65536,
            "local_gb": 2000,
            "local_gb_used": 500,
            "running_vms": 20,
            "status": "enabled",
            "state": "up"
        }"#;
        let hv: Hypervisor = serde_json::from_str(json).unwrap();
        assert_eq!(hv.hypervisor_hostname, "compute-01.local");
        assert_eq!(hv.vcpus, 64);
        assert_eq!(hv.running_vms, 20);
    }
}
