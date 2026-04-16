//! [`CloudDirectory`] implementation backed by [`Config`].
//!
//! Wraps an `Arc<Config>` to expose the startup-time cloud list and active
//! cloud selection. The `Config` must be fully initialized (including any
//! `--cloud` CLI override) before this wrapper is constructed.
//!
//! BL-P2-031 T3 — replaced by a dynamic implementation when runtime cloud
//! discovery is introduced.

use std::sync::Arc;

use crate::config::Config;
use crate::context::resolver::CloudDirectory;

pub struct ConfigCloudDirectory {
    config: Arc<Config>,
}

impl ConfigCloudDirectory {
    pub fn new(config: Arc<Config>) -> Self {
        Self { config }
    }
}

impl CloudDirectory for ConfigCloudDirectory {
    fn active_cloud(&self) -> String {
        self.config.active_cloud_name().to_string()
    }

    fn known_clouds(&self) -> Vec<String> {
        self.config
            .cloud_names()
            .iter()
            .map(|s| s.to_string())
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;
    use tempfile::NamedTempFile;

    fn test_config(clouds_yaml: &str) -> Config {
        let mut f = NamedTempFile::with_suffix(".yaml").unwrap();
        f.write_all(clouds_yaml.as_bytes()).unwrap();
        Config::load_from(f.path()).unwrap()
    }

    #[test]
    fn active_cloud_matches_config() {
        let config = Arc::new(test_config(
            r#"
clouds:
  prod:
    auth:
      auth_url: https://keystone/v3
      username: admin
      password: secret
    region_name: RegionOne
"#,
        ));
        let dir = ConfigCloudDirectory::new(config);
        assert_eq!(dir.active_cloud(), "prod");
    }

    #[test]
    fn known_clouds_returns_all() {
        let config = Arc::new(test_config(
            r#"
clouds:
  alpha:
    auth:
      auth_url: https://a/v3
      username: u
      password: p
  beta:
    auth:
      auth_url: https://b/v3
      username: u
      password: p
"#,
        ));
        let dir = ConfigCloudDirectory::new(config);
        let mut clouds = dir.known_clouds();
        clouds.sort();
        assert_eq!(clouds, vec!["alpha", "beta"]);
    }
}
