//! [`ProjectDirectoryPort`] implementation backed by static [`Config`] data.
//!
//! Returns the single `project_name` declared in each cloud's auth section.
//! Clouds without a `project_name` yield an empty list. `project_id` is set
//! to `project_name` as a placeholder — BL-P2-052 will replace this with an
//! HTTP-based implementation that queries `/v3/auth/projects` for real UUIDs.

use std::sync::Arc;

use async_trait::async_trait;

use crate::config::Config;
use crate::context::error::SwitchError;
use crate::context::resolver::{ProjectCandidate, ProjectDirectoryPort};

pub struct StaticProjectDirectory {
    config: Arc<Config>,
}

impl StaticProjectDirectory {
    pub fn new(config: Arc<Config>) -> Self {
        Self { config }
    }
}

#[async_trait]
impl ProjectDirectoryPort for StaticProjectDirectory {
    async fn list_projects(&self, cloud: &str) -> Result<Vec<ProjectCandidate>, SwitchError> {
        let Some(cloud_config) = self.config.cloud_config(cloud) else {
            return Ok(Vec::new());
        };

        let Some(ref project_name) = cloud_config.auth.project_name else {
            return Ok(Vec::new());
        };

        let domain = cloud_config
            .auth
            .project_domain_name
            .clone()
            .unwrap_or_else(|| "Default".to_string());

        Ok(vec![ProjectCandidate {
            cloud: cloud.to_string(),
            // PLACEHOLDER: project_id uses project_name until BL-P2-052
            // adds HTTP-based `/v3/auth/projects` lookup for real UUIDs.
            project_id: project_name.clone(),
            project_name: project_name.clone(),
            domain,
        }])
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

    #[tokio::test]
    async fn list_projects_returns_project_name() {
        let config = Arc::new(test_config(
            r#"
clouds:
  prod:
    auth:
      auth_url: https://keystone/v3
      username: admin
      password: secret
      project_name: admin-project
      project_domain_name: Default
    region_name: RegionOne
"#,
        ));
        let dir = StaticProjectDirectory::new(config);
        let projects = dir.list_projects("prod").await.unwrap();
        assert_eq!(projects.len(), 1);
        assert_eq!(projects[0].project_name, "admin-project");
        assert_eq!(projects[0].cloud, "prod");
        assert_eq!(projects[0].domain, "Default");
        // PLACEHOLDER: project_id == project_name until BL-P2-052
        assert_eq!(projects[0].project_id, "admin-project");
    }

    #[tokio::test]
    async fn list_projects_no_project_name_returns_empty() {
        let config = Arc::new(test_config(
            r#"
clouds:
  noproject:
    auth:
      auth_url: https://keystone/v3
      username: admin
      password: secret
    region_name: RegionOne
"#,
        ));
        let dir = StaticProjectDirectory::new(config);
        let projects = dir.list_projects("noproject").await.unwrap();
        assert!(projects.is_empty());
    }

    #[tokio::test]
    async fn list_projects_unknown_cloud_returns_empty() {
        let config = Arc::new(test_config(
            r#"
clouds:
  prod:
    auth:
      auth_url: https://keystone/v3
      username: admin
      password: secret
"#,
        ));
        let dir = StaticProjectDirectory::new(config);
        let projects = dir.list_projects("nonexistent").await.unwrap();
        assert!(projects.is_empty());
    }

    #[tokio::test]
    async fn list_projects_default_domain_when_missing() {
        let config = Arc::new(test_config(
            r#"
clouds:
  prod:
    auth:
      auth_url: https://keystone/v3
      username: admin
      password: secret
      project_name: my-project
"#,
        ));
        let dir = StaticProjectDirectory::new(config);
        let projects = dir.list_projects("prod").await.unwrap();
        assert_eq!(projects[0].domain, "Default");
    }
}
