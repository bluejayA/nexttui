use serde::Deserialize;

#[derive(Debug, Clone, Deserialize)]
pub struct Image {
    pub id: String,
    pub name: String,
    pub status: String,
    pub disk_format: Option<String>,
    pub container_format: Option<String>,
    pub size: Option<u64>,
    #[serde(default = "default_private")]
    pub visibility: String,
    #[serde(default)]
    pub min_disk: u32,
    #[serde(default)]
    pub min_ram: u32,
    pub checksum: Option<String>,
    pub created_at: Option<String>,
    #[serde(default)]
    pub owner: Option<String>,
}

fn default_private() -> String {
    "private".to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_image_deserialize() {
        let json = r#"{
            "id": "img-001",
            "name": "Ubuntu 22.04",
            "status": "active",
            "disk_format": "qcow2",
            "container_format": "bare",
            "size": 2147483648,
            "visibility": "public",
            "min_disk": 10,
            "min_ram": 512,
            "checksum": "abc123def456",
            "created_at": "2026-01-01T00:00:00Z"
        }"#;
        let img: Image = serde_json::from_str(json).unwrap();
        assert_eq!(img.name, "Ubuntu 22.04");
        assert_eq!(img.visibility, "public");
        assert_eq!(img.size, Some(2147483648));
        assert_eq!(img.min_disk, 10);
    }
}
