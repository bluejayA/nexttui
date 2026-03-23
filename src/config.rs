use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::time::Duration;

use serde::{Deserialize, Serialize};

use crate::error::{AppError, Result};
use crate::models::common::ResourceType;

#[derive(Debug, Clone)]
pub struct Config {
    clouds: HashMap<String, CloudConfig>,
    active_cloud: String,
    app: AppConfig,
    warnings: Vec<String>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct CloudConfig {
    #[serde(skip)]
    pub name: String,
    pub auth: AuthConfig,
    pub region_name: Option<String>,
    pub regions: Option<Vec<String>>,
    #[serde(default = "default_interface")]
    pub interface: String,
    #[serde(default = "default_api_version")]
    pub identity_api_version: u8,
    #[serde(default = "default_verify_true")]
    pub verify: bool,
    pub cacert: Option<PathBuf>,
}

fn default_interface() -> String {
    "public".to_string()
}
fn default_api_version() -> u8 {
    3
}
fn default_verify_true() -> bool {
    true
}

#[derive(Clone, Deserialize, Serialize)]
pub struct AuthConfig {
    pub auth_url: String,
    #[serde(default)]
    pub auth_type: Option<String>,
    pub username: Option<String>,
    #[serde(skip_serializing)]
    pub password: Option<String>,
    pub project_name: Option<String>,
    pub project_domain_name: Option<String>,
    pub user_domain_name: Option<String>,
    pub application_credential_id: Option<String>,
    #[serde(skip_serializing)]
    pub application_credential_secret: Option<String>,
}

impl std::fmt::Debug for AuthConfig {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("AuthConfig")
            .field("auth_url", &self.auth_url)
            .field("auth_type", &self.auth_type)
            .field("username", &self.username)
            .field("password", &"****")
            .field("project_name", &self.project_name)
            .field("project_domain_name", &self.project_domain_name)
            .field("user_domain_name", &self.user_domain_name)
            .field("application_credential_id", &self.application_credential_id)
            .field("application_credential_secret", &"****")
            .finish()
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AuthType {
    Password,
    ApplicationCredential,
}

#[derive(Debug, Clone, Deserialize)]
pub struct AppConfig {
    #[serde(default = "default_tick_rate")]
    pub tick_rate_ms: u64,
    #[serde(default)]
    pub cache_ttl: CacheTtlConfig,
    #[serde(default = "default_audit_path")]
    pub audit_log_path: PathBuf,
    #[serde(default = "default_history_path")]
    pub command_history_path: PathBuf,
    #[serde(default = "default_history_max")]
    pub command_history_max: usize,
    pub default_cloud: Option<String>,
}

fn default_tick_rate() -> u64 {
    200
}
fn default_audit_path() -> PathBuf {
    dirs::config_dir()
        .unwrap_or_else(|| PathBuf::from("~/.config"))
        .join("nexttui/audit.log")
}
fn default_history_path() -> PathBuf {
    dirs::config_dir()
        .unwrap_or_else(|| PathBuf::from("~/.config"))
        .join("nexttui/history")
}
fn default_history_max() -> usize {
    50
}

impl Default for AppConfig {
    fn default() -> Self {
        Self {
            tick_rate_ms: 200,
            cache_ttl: CacheTtlConfig::default(),
            audit_log_path: default_audit_path(),
            command_history_path: default_history_path(),
            command_history_max: 50,
            default_cloud: None,
        }
    }
}

#[derive(Debug, Clone, Deserialize)]
pub struct CacheTtlConfig {
    #[serde(default = "default_120")]
    pub servers_secs: u64,
    #[serde(default = "default_300")]
    pub networks_secs: u64,
    #[serde(default = "default_600")]
    pub flavors_secs: u64,
    #[serde(default = "default_600")]
    pub images_secs: u64,
    #[serde(default = "default_300")]
    pub security_groups_secs: u64,
    #[serde(default = "default_120")]
    pub volumes_secs: u64,
    #[serde(default = "default_300")]
    pub projects_secs: u64,
}

fn default_120() -> u64 {
    120
}
fn default_300() -> u64 {
    300
}
fn default_600() -> u64 {
    600
}

impl Default for CacheTtlConfig {
    fn default() -> Self {
        Self {
            servers_secs: 120,
            networks_secs: 300,
            flavors_secs: 600,
            images_secs: 600,
            security_groups_secs: 300,
            volumes_secs: 120,
            projects_secs: 300,
        }
    }
}

// Internal: clouds.yaml root structure
#[derive(Deserialize)]
struct CloudsYamlRoot {
    clouds: Option<HashMap<String, CloudConfig>>,
}

impl Config {
    /// Load config from clouds.yaml + optional app config.
    /// `override_clouds_path` is for testing — bypasses standard search paths.
    pub fn load_from(clouds_yaml_path: &Path) -> Result<Self> {
        let content =
            std::fs::read_to_string(clouds_yaml_path).map_err(|e| AppError::ConfigParse {
                path: clouds_yaml_path.to_path_buf(),
                source: Box::new(e),
            })?;

        let root: CloudsYamlRoot =
            serde_yaml::from_str(&content).map_err(|e| AppError::ConfigParse {
                path: clouds_yaml_path.to_path_buf(),
                source: Box::new(e),
            })?;

        let raw_clouds = root.clouds.ok_or_else(|| AppError::ConfigValidation {
            message: "Invalid clouds.yaml: missing 'clouds' key".to_string(),
        })?;

        // Validate each cloud, skip invalid ones
        let mut clouds = HashMap::new();
        let mut warnings = Vec::new();

        for (name, mut cloud) in raw_clouds {
            cloud.name = name.clone();
            match validate_cloud(&name, &cloud) {
                Ok(result) => {
                    warnings.extend(result.warnings);
                    clouds.insert(name, cloud);
                }
                Err(msg) => {
                    warnings.push(msg);
                }
            }
        }

        if clouds.is_empty() {
            return Err(AppError::ConfigValidation {
                message: "No valid cloud configurations found in clouds.yaml".to_string(),
            });
        }

        // Determine active cloud
        let active_cloud = Self::determine_active_cloud(&clouds, None)?;

        // Load app config (optional)
        let app = Self::load_app_config();

        Ok(Config {
            clouds,
            active_cloud,
            app,
            warnings,
        })
    }

    /// Load from standard search paths
    pub fn load() -> Result<Self> {
        let path = Self::find_clouds_yaml()?;
        let mut config = Self::load_from(&path)?;

        // Override active cloud: config.toml default_cloud > $OS_CLOUD
        let default_cloud = config.app.default_cloud.clone();
        let env_cloud = std::env::var("OS_CLOUD").ok();
        let preferred = default_cloud.or(env_cloud);
        if let Some(ref name) = preferred {
            if !config.clouds.contains_key(name) {
                return Err(AppError::CloudNotFound {
                    name: name.clone(),
                    available: config.clouds.keys().cloned().collect(),
                });
            }
            config.active_cloud = name.clone();
        }

        Ok(config)
    }

    /// Get warnings generated during config loading (e.g., skipped clouds)
    pub fn warnings(&self) -> &[String] {
        &self.warnings
    }

    fn find_clouds_yaml() -> Result<PathBuf> {
        let mut searched = Vec::new();

        // 1. $OS_CLIENT_CONFIG_FILE
        if let Ok(p) = std::env::var("OS_CLIENT_CONFIG_FILE") {
            let path = PathBuf::from(&p);
            if path.exists() {
                return Ok(path);
            }
            searched.push(path);
        }

        // 2. ./clouds.yaml
        let local = PathBuf::from("./clouds.yaml");
        if local.exists() {
            return Ok(local);
        }
        searched.push(local);

        // 3. ~/.config/openstack/clouds.yaml
        if let Some(config_dir) = dirs::config_dir() {
            let path = config_dir.join("openstack/clouds.yaml");
            if path.exists() {
                return Ok(path);
            }
            searched.push(path);
        }

        // 4. /etc/openstack/clouds.yaml
        let etc = PathBuf::from("/etc/openstack/clouds.yaml");
        if etc.exists() {
            return Ok(etc);
        }
        searched.push(etc);

        Err(AppError::CloudsYamlNotFound {
            searched_paths: searched,
        })
    }

    fn determine_active_cloud(
        clouds: &HashMap<String, CloudConfig>,
        preferred: Option<&str>,
    ) -> Result<String> {
        if let Some(name) = preferred {
            if clouds.contains_key(name) {
                return Ok(name.to_string());
            }
            return Err(AppError::CloudNotFound {
                name: name.to_string(),
                available: clouds.keys().cloned().collect(),
            });
        }
        // fallback: first key (sorted for determinism)
        let mut keys: Vec<&String> = clouds.keys().collect();
        keys.sort();
        Ok(keys[0].clone())
    }

    fn load_app_config() -> AppConfig {
        let config_path = dirs::config_dir()
            .unwrap_or_else(|| PathBuf::from("~/.config"))
            .join("nexttui/config.toml");

        if !config_path.exists() {
            return AppConfig::default();
        }

        match std::fs::read_to_string(&config_path) {
            Ok(content) => toml::from_str(&content).unwrap_or_else(|e| {
                eprintln!("Warning: Failed to parse config.toml, using defaults: {e}");
                AppConfig::default()
            }),
            Err(_) => AppConfig::default(),
        }
    }

    pub fn active_cloud_config(&self) -> &CloudConfig {
        &self.clouds[&self.active_cloud]
    }

    pub fn cloud_config(&self, name: &str) -> Option<&CloudConfig> {
        self.clouds.get(name)
    }

    pub fn cloud_names(&self) -> Vec<&str> {
        let mut names: Vec<&str> = self.clouds.keys().map(|s| s.as_str()).collect();
        names.sort();
        names
    }

    pub fn active_cloud_name(&self) -> &str {
        &self.active_cloud
    }

    pub fn switch_cloud(&mut self, name: &str) -> Result<()> {
        if !self.clouds.contains_key(name) {
            return Err(AppError::CloudNotFound {
                name: name.to_string(),
                available: self.clouds.keys().cloned().collect(),
            });
        }
        self.active_cloud = name.to_string();
        Ok(())
    }

    pub fn cache_ttl(&self, resource_type: ResourceType) -> Duration {
        let secs = match resource_type {
            ResourceType::Servers => self.app.cache_ttl.servers_secs,
            ResourceType::Networks => self.app.cache_ttl.networks_secs,
            ResourceType::Flavors => self.app.cache_ttl.flavors_secs,
            ResourceType::Images => self.app.cache_ttl.images_secs,
            ResourceType::SecurityGroups => self.app.cache_ttl.security_groups_secs,
            ResourceType::Volumes | ResourceType::Snapshots => self.app.cache_ttl.volumes_secs,
            ResourceType::Projects | ResourceType::Users => self.app.cache_ttl.projects_secs,
            _ => 120, // default fallback
        };
        Duration::from_secs(secs)
    }

    pub fn app_config(&self) -> &AppConfig {
        &self.app
    }
}

/// Detect auth type from config fields
pub fn detect_auth_type(auth: &AuthConfig) -> AuthType {
    if let Some(ref t) = auth.auth_type {
        match t.as_str() {
            "v3applicationcredential" => return AuthType::ApplicationCredential,
            "v3password" | "password" | "" => return AuthType::Password,
            _ => return AuthType::Password,
        }
    }
    if auth.application_credential_id.is_some() {
        AuthType::ApplicationCredential
    } else {
        AuthType::Password
    }
}

struct ValidationResult {
    warnings: Vec<String>,
}

fn validate_cloud(
    name: &str,
    cloud: &CloudConfig,
) -> std::result::Result<ValidationResult, String> {
    let mut warnings = Vec::new();

    if cloud.auth.auth_url.is_empty() {
        return Err(format!("Cloud '{name}' skipped: auth_url is required"));
    }

    let auth_type = detect_auth_type(&cloud.auth);
    match auth_type {
        AuthType::Password => {
            if cloud.auth.username.is_none() || cloud.auth.password.is_none() {
                return Err(format!(
                    "Cloud '{name}' skipped: username and password required for password auth"
                ));
            }
        }
        AuthType::ApplicationCredential => {
            if cloud.auth.application_credential_id.is_none()
                || cloud.auth.application_credential_secret.is_none()
            {
                return Err(format!(
                    "Cloud '{name}' skipped: credential id and secret required"
                ));
            }
        }
    }

    if let Some(ref cacert) = cloud.cacert
        && !cacert.exists()
    {
        return Err(format!(
            "Cloud '{name}': cacert path not found: {}",
            cacert.display()
        ));
    }

    if !cloud.verify {
        warnings.push(format!(
            "Cloud '{name}': TLS verification disabled (insecure)"
        ));
    }

    Ok(ValidationResult { warnings })
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    fn write_clouds_yaml(dir: &TempDir, content: &str) -> PathBuf {
        let path = dir.path().join("clouds.yaml");
        std::fs::write(&path, content).unwrap();
        path
    }

    fn valid_clouds_yaml() -> &'static str {
        r#"
clouds:
  devstack:
    auth:
      auth_url: https://keystone.dev.example.com/v3
      username: admin
      password: secret123
      project_name: admin
      user_domain_name: Default
      project_domain_name: Default
    region_name: RegionOne
  production:
    auth:
      auth_url: https://keystone.prod.example.com/v3
      application_credential_id: abc123
      application_credential_secret: supersecret
    region_name: RegionTwo
"#
    }

    #[test]
    fn test_load_clouds_yaml_from_standard_path() {
        let dir = TempDir::new().unwrap();
        let path = write_clouds_yaml(&dir, valid_clouds_yaml());
        let config = Config::load_from(&path).unwrap();
        assert_eq!(config.clouds.len(), 2);
        assert!(config.clouds.contains_key("devstack"));
        assert!(config.clouds.contains_key("production"));
    }

    #[test]
    fn test_clouds_yaml_not_found() {
        let result = Config::load_from(Path::new("/nonexistent/clouds.yaml"));
        assert!(result.is_err());
        let err = result.unwrap_err();
        assert!(matches!(err, AppError::ConfigParse { .. }));
    }

    #[test]
    fn test_missing_clouds_key() {
        let dir = TempDir::new().unwrap();
        let path = write_clouds_yaml(&dir, "something_else:\n  key: value\n");
        let result = Config::load_from(&path);
        assert!(result.is_err());
        let err = result.unwrap_err();
        match err {
            AppError::ConfigValidation { ref message } => {
                assert!(message.contains("missing 'clouds' key"));
            }
            _ => panic!("Expected ConfigValidation, got: {err:?}"),
        }
    }

    #[test]
    fn test_auth_type_auto_detect_password() {
        let auth = AuthConfig {
            auth_url: "https://keystone/v3".to_string(),
            auth_type: None,
            username: Some("admin".to_string()),
            password: Some("pass".to_string()),
            project_name: None,
            project_domain_name: None,
            user_domain_name: None,
            application_credential_id: None,
            application_credential_secret: None,
        };
        assert_eq!(detect_auth_type(&auth), AuthType::Password);
    }

    #[test]
    fn test_auth_type_auto_detect_app_credential() {
        let auth = AuthConfig {
            auth_url: "https://keystone/v3".to_string(),
            auth_type: None,
            username: None,
            password: None,
            project_name: None,
            project_domain_name: None,
            user_domain_name: None,
            application_credential_id: Some("abc".to_string()),
            application_credential_secret: Some("xyz".to_string()),
        };
        assert_eq!(detect_auth_type(&auth), AuthType::ApplicationCredential);
    }

    #[test]
    fn test_password_auth_missing_username() {
        let dir = TempDir::new().unwrap();
        let yaml = r#"
clouds:
  bad:
    auth:
      auth_url: https://keystone/v3
      password: secret
  good:
    auth:
      auth_url: https://keystone/v3
      username: admin
      password: secret
"#;
        let path = write_clouds_yaml(&dir, yaml);
        let config = Config::load_from(&path).unwrap();
        assert_eq!(config.clouds.len(), 1);
        assert!(config.clouds.contains_key("good"));
        assert!(!config.clouds.contains_key("bad"));
    }

    #[test]
    fn test_app_credential_missing_secret() {
        let dir = TempDir::new().unwrap();
        let yaml = r#"
clouds:
  bad:
    auth:
      auth_url: https://keystone/v3
      application_credential_id: abc
  good:
    auth:
      auth_url: https://keystone/v3
      username: admin
      password: secret
"#;
        let path = write_clouds_yaml(&dir, yaml);
        let config = Config::load_from(&path).unwrap();
        assert_eq!(config.clouds.len(), 1);
        assert!(config.clouds.contains_key("good"));
    }

    #[test]
    fn test_all_clouds_invalid_fatal() {
        let dir = TempDir::new().unwrap();
        let yaml = r#"
clouds:
  bad1:
    auth:
      auth_url: ""
  bad2:
    auth:
      auth_url: https://keystone/v3
      password: nouser
"#;
        let path = write_clouds_yaml(&dir, yaml);
        let result = Config::load_from(&path);
        assert!(result.is_err());
        match result.unwrap_err() {
            AppError::ConfigValidation { message } => {
                assert!(message.contains("No valid cloud"));
            }
            e => panic!("Expected ConfigValidation, got: {e:?}"),
        }
    }

    #[test]
    fn test_partial_invalid_clouds_skip() {
        let dir = TempDir::new().unwrap();
        let yaml = r#"
clouds:
  bad:
    auth:
      auth_url: ""
  good:
    auth:
      auth_url: https://keystone/v3
      username: admin
      password: secret
"#;
        let path = write_clouds_yaml(&dir, yaml);
        let config = Config::load_from(&path).unwrap();
        assert_eq!(config.clouds.len(), 1);
        assert!(config.clouds.contains_key("good"));
    }

    #[test]
    fn test_active_cloud_fallback_to_first() {
        let dir = TempDir::new().unwrap();
        let path = write_clouds_yaml(&dir, valid_clouds_yaml());
        let config = Config::load_from(&path).unwrap();
        // sorted: "devstack" < "production"
        assert_eq!(config.active_cloud, "devstack");
    }

    #[test]
    fn test_active_cloud_not_found() {
        let clouds = HashMap::from([(
            "dev".to_string(),
            CloudConfig {
                name: "dev".to_string(),
                auth: AuthConfig {
                    auth_url: "https://keystone/v3".to_string(),
                    auth_type: None,
                    username: Some("a".to_string()),
                    password: Some("b".to_string()),
                    project_name: None,
                    project_domain_name: None,
                    user_domain_name: None,
                    application_credential_id: None,
                    application_credential_secret: None,
                },
                region_name: None,
                regions: None,
                interface: "public".to_string(),
                identity_api_version: 3,
                verify: true,
                cacert: None,
            },
        )]);
        let result = Config::determine_active_cloud(&clouds, Some("nonexistent"));
        assert!(result.is_err());
        match result.unwrap_err() {
            AppError::CloudNotFound { name, .. } => assert_eq!(name, "nonexistent"),
            e => panic!("Expected CloudNotFound, got: {e:?}"),
        }
    }

    #[test]
    fn test_switch_cloud_success() {
        let dir = TempDir::new().unwrap();
        let path = write_clouds_yaml(&dir, valid_clouds_yaml());
        let mut config = Config::load_from(&path).unwrap();
        assert_eq!(config.active_cloud, "devstack");
        config.switch_cloud("production").unwrap();
        assert_eq!(config.active_cloud, "production");
    }

    #[test]
    fn test_switch_cloud_not_found() {
        let dir = TempDir::new().unwrap();
        let path = write_clouds_yaml(&dir, valid_clouds_yaml());
        let mut config = Config::load_from(&path).unwrap();
        let result = config.switch_cloud("nonexistent");
        assert!(result.is_err());
    }

    #[test]
    fn test_password_debug_masked() {
        let auth = AuthConfig {
            auth_url: "https://keystone/v3".to_string(),
            auth_type: None,
            username: Some("admin".to_string()),
            password: Some("my_secret_password".to_string()),
            project_name: None,
            project_domain_name: None,
            user_domain_name: None,
            application_credential_id: None,
            application_credential_secret: Some("my_app_secret".to_string()),
        };
        let debug_str = format!("{auth:?}");
        assert!(!debug_str.contains("my_secret_password"));
        assert!(!debug_str.contains("my_app_secret"));
        assert!(debug_str.contains("****"));
    }

    #[test]
    fn test_secret_not_serialized() {
        let auth = AuthConfig {
            auth_url: "https://keystone/v3".to_string(),
            auth_type: None,
            username: Some("admin".to_string()),
            password: Some("my_secret_password".to_string()),
            project_name: None,
            project_domain_name: None,
            user_domain_name: None,
            application_credential_id: None,
            application_credential_secret: Some("my_app_secret".to_string()),
        };
        let json = serde_json::to_string(&auth).unwrap();
        assert!(!json.contains("my_secret_password"));
        assert!(!json.contains("my_app_secret"));
        assert!(json.contains("admin")); // username is still there
    }

    #[test]
    fn test_app_config_missing_uses_defaults() {
        let app = AppConfig::default();
        assert_eq!(app.tick_rate_ms, 200);
        assert_eq!(app.command_history_max, 50);
        assert_eq!(app.cache_ttl.servers_secs, 120);
        assert_eq!(app.cache_ttl.flavors_secs, 600);
    }

    #[test]
    fn test_app_config_partial_override() {
        let toml_str = r#"
tick_rate_ms = 100

[cache_ttl]
servers_secs = 60
"#;
        let app: AppConfig = toml::from_str(toml_str).unwrap();
        assert_eq!(app.tick_rate_ms, 100);
        assert_eq!(app.cache_ttl.servers_secs, 60);
        // non-overridden fields keep defaults
        assert_eq!(app.cache_ttl.flavors_secs, 600);
        assert_eq!(app.command_history_max, 50);
    }

    #[test]
    fn test_cache_ttl_mapping() {
        let dir = TempDir::new().unwrap();
        let path = write_clouds_yaml(&dir, valid_clouds_yaml());
        let config = Config::load_from(&path).unwrap();

        assert_eq!(
            config.cache_ttl(ResourceType::Servers),
            Duration::from_secs(120)
        );
        assert_eq!(
            config.cache_ttl(ResourceType::Networks),
            Duration::from_secs(300)
        );
        assert_eq!(
            config.cache_ttl(ResourceType::Flavors),
            Duration::from_secs(600)
        );
        assert_eq!(
            config.cache_ttl(ResourceType::Images),
            Duration::from_secs(600)
        );
        assert_eq!(
            config.cache_ttl(ResourceType::Volumes),
            Duration::from_secs(120)
        );
        // fallback for unmapped types
        assert_eq!(
            config.cache_ttl(ResourceType::Aggregates),
            Duration::from_secs(120)
        );
    }
}
