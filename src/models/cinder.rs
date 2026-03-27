use serde::Deserialize;

#[derive(Debug, Clone, Deserialize)]
pub struct Volume {
    pub id: String,
    pub name: Option<String>,
    pub description: Option<String>,
    pub status: String,
    pub size: u32,
    pub volume_type: Option<String>,
    #[serde(default)]
    pub encrypted: bool,
    #[serde(default = "default_false_str")]
    pub bootable: String,
    #[serde(default)]
    pub attachments: Vec<VolumeAttachment>,
    pub availability_zone: Option<String>,
    pub created_at: Option<String>,
    #[serde(default, rename = "os-vol-tenant-attr:tenant_id")]
    pub tenant_id: Option<String>,
}

fn default_false_str() -> String {
    "false".to_string()
}

#[derive(Debug, Clone, Deserialize)]
pub struct VolumeAttachment {
    pub server_id: String,
    pub device: String,
    pub id: String,
}

#[derive(Debug, Clone, Deserialize)]
pub struct VolumeSnapshot {
    pub id: String,
    pub name: Option<String>,
    pub status: String,
    pub size: u32,
    pub volume_id: String,
    pub created_at: Option<String>,
    #[serde(default, rename = "os-extended-snapshot-attributes:project_id")]
    pub tenant_id: Option<String>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_volume_deserialize() {
        let json = r#"{
            "id": "vol-001",
            "name": "data-vol",
            "status": "in-use",
            "size": 100,
            "volume_type": "ssd",
            "encrypted": false,
            "bootable": "false",
            "attachments": [
                {
                    "server_id": "srv-001",
                    "device": "/dev/vdb",
                    "id": "att-001"
                }
            ],
            "availability_zone": "az1",
            "created_at": "2026-01-01T00:00:00Z"
        }"#;
        let vol: Volume = serde_json::from_str(json).unwrap();
        assert_eq!(vol.id, "vol-001");
        assert_eq!(vol.size, 100);
        assert_eq!(vol.bootable, "false");
        assert_eq!(vol.attachments.len(), 1);
        assert_eq!(vol.attachments[0].device, "/dev/vdb");
    }

    #[test]
    fn test_snapshot_deserialize() {
        let json = r#"{
            "id": "snap-001",
            "name": "daily-backup",
            "status": "available",
            "size": 100,
            "volume_id": "vol-001",
            "created_at": "2026-01-15T00:00:00Z"
        }"#;
        let snap: VolumeSnapshot = serde_json::from_str(json).unwrap();
        assert_eq!(snap.id, "snap-001");
        assert_eq!(snap.volume_id, "vol-001");
        assert_eq!(snap.size, 100);
    }
}
