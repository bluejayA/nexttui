use std::collections::HashMap;

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

// --- Pagination ---

#[derive(Debug, Clone, Default)]
pub struct PaginationParams {
    pub marker: Option<String>,
    pub limit: Option<u32>,
    pub sort_key: Option<String>,
    pub sort_dir: Option<SortDirection>,
}

#[derive(Debug, Clone, Copy)]
pub enum SortDirection {
    Asc,
    Desc,
}

#[derive(Debug, Clone)]
pub struct PaginatedResponse<T> {
    pub items: Vec<T>,
    pub next_marker: Option<String>,
    pub has_more: bool,
}

impl<T> PaginatedResponse<T> {
    pub fn empty() -> Self {
        Self {
            items: Vec::new(),
            next_marker: None,
            has_more: false,
        }
    }
}

// --- Auth ---

#[derive(Debug, Clone, Hash, Eq, PartialEq, Serialize, Deserialize)]
pub enum TokenScope {
    Project { name: String, domain: String },
    Unscoped,
}

impl TokenScope {
    pub fn from_credential(credential: &AuthCredential) -> Self {
        match &credential.project_scope {
            Some(p) => Self::Project {
                name: p.name.to_lowercase(),
                domain: p.domain_name.to_lowercase(),
            },
            None => Self::Unscoped,
        }
    }

    /// Generate a filesystem-safe cache key.
    /// Uses `@` as separator (not `_` which appears in project/domain names).
    /// Sanitizes path traversal characters.
    pub fn cache_key(&self) -> String {
        match self {
            Self::Project { name, domain } => {
                let safe_name = sanitize_for_filename(name);
                let safe_domain = sanitize_for_filename(domain);
                format!("project@{safe_name}@{safe_domain}")
            }
            Self::Unscoped => "unscoped".to_string(),
        }
    }
}

/// Remove path-traversal and filesystem-unsafe characters from a string.
fn sanitize_for_filename(s: &str) -> String {
    s.chars()
        .map(|c| match c {
            '/' | '\\' | '\0' | '.' => '_',
            _ => c,
        })
        .collect()
}

#[derive(Clone, Serialize, Deserialize)]
pub struct Token {
    pub id: String,
    pub expires_at: DateTime<Utc>,
    pub project: ProjectScope,
    pub roles: Vec<TokenRole>,
    pub catalog: Vec<CatalogEntry>,
}

impl std::fmt::Debug for Token {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Token")
            .field("id", &"****")
            .field("expires_at", &self.expires_at)
            .field("project", &self.project)
            .field("roles", &self.roles)
            .field("catalog", &format!("[{} entries]", self.catalog.len()))
            .finish()
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ProjectScope {
    pub id: String,
    pub name: String,
    pub domain_id: String,
    pub domain_name: String,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct TokenRole {
    pub id: String,
    pub name: String,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct CatalogEntry {
    pub service_type: String,
    pub service_name: String,
    pub endpoints: Vec<Endpoint>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Endpoint {
    pub region: String,
    pub interface: EndpointInterface,
    pub url: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum EndpointInterface {
    Public,
    Internal,
    Admin,
}

#[derive(Clone, Debug)]
pub struct AuthCredential {
    pub auth_url: String,
    pub method: AuthMethod,
    pub project_scope: Option<ProjectScopeParam>,
}

#[derive(Clone)]
pub enum AuthMethod {
    Password {
        username: String,
        password: String,
        domain_name: String,
    },
    ApplicationCredential {
        id: String,
        secret: String,
    },
}

impl std::fmt::Debug for AuthMethod {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            AuthMethod::Password {
                username,
                domain_name,
                ..
            } => f
                .debug_struct("Password")
                .field("username", username)
                .field("password", &"****")
                .field("domain_name", domain_name)
                .finish(),
            AuthMethod::ApplicationCredential { id, .. } => f
                .debug_struct("ApplicationCredential")
                .field("id", id)
                .field("secret", &"****")
                .finish(),
        }
    }
}

#[derive(Clone, Debug)]
pub struct ProjectScopeParam {
    pub name: String,
    pub domain_name: String,
}

#[derive(Clone)]
pub struct AuthHeaders {
    pub headers: Vec<(String, String)>,
}

impl std::fmt::Debug for AuthHeaders {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("AuthHeaders")
            .field("headers", &format!("[{} entries]", self.headers.len()))
            .finish()
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct Capability {
    pub resource: String,
    pub action: String,
}

// --- Filters ---

#[derive(Debug, Clone, Default)]
pub struct ServerListFilter {
    pub name: Option<String>,
    pub status: Option<String>,
    pub host: Option<String>,
    pub flavor: Option<String>,
    pub all_tenants: bool,
}

#[derive(Debug, Clone, Default)]
pub struct VolumeListFilter {
    pub name: Option<String>,
    pub status: Option<String>,
    pub all_tenants: bool,
}

#[derive(Debug, Clone, Default)]
pub struct ImageListFilter {
    pub name: Option<String>,
    pub status: Option<String>,
    pub visibility: Option<String>,
    pub all_tenants: bool,
}

#[derive(Debug, Clone, Default)]
pub struct NetworkListFilter {
    pub all_tenants: bool,
}

#[derive(Debug, Clone, Default)]
pub struct SecurityGroupListFilter {
    pub all_tenants: bool,
}

#[derive(Debug, Clone, Default)]
pub struct FloatingIpListFilter {
    pub all_tenants: bool,
}

#[derive(Debug, Clone, Default)]
pub struct SnapshotListFilter {
    pub all_tenants: bool,
}

// --- Nova params ---

#[derive(Debug, Clone)]
pub enum RebootType {
    Soft,
    Hard,
}

#[derive(Debug, Clone)]
pub enum ServerState {
    Active,
    Error,
    Paused,
    Suspended,
    Stopped,
}

#[derive(Debug, Clone)]
pub struct ServerCreateParams {
    pub name: String,
    pub image_id: String,
    pub flavor_id: String,
    pub networks: Vec<NetworkAttachment>,
    pub security_groups: Option<Vec<String>>,
    pub key_name: Option<String>,
    pub availability_zone: Option<String>,
}

#[derive(Debug, Clone)]
pub struct NetworkAttachment {
    pub uuid: String,
    pub fixed_ip: Option<String>,
}

#[derive(Debug, Clone)]
pub struct LiveMigrateParams {
    pub host: Option<String>,
}

#[derive(Debug, Clone, Default)]
pub struct EvacuateParams {
    pub host: Option<String>,
    pub on_shared_storage: Option<bool>,
    pub force: Option<bool>,
}

#[derive(Debug, Clone)]
pub struct FlavorCreateParams {
    pub name: String,
    pub vcpus: u32,
    pub ram_mb: u32,
    pub disk_gb: u32,
    pub is_public: bool,
}

#[derive(Debug, Clone)]
pub struct AggregateCreateParams {
    pub name: String,
    pub availability_zone: Option<String>,
}

#[derive(Debug, Clone)]
pub struct AggregateUpdateParams {
    pub name: Option<String>,
    pub availability_zone: Option<String>,
}

#[derive(Debug, Clone)]
pub struct ComputeQuotaUpdateParams {
    pub cores: Option<i64>,
    pub ram: Option<i64>,
    pub instances: Option<i64>,
}

// --- Nova response types ---

#[derive(Debug, Clone, Deserialize)]
pub struct ServerEvent {
    pub action: String,
    pub start_time: Option<String>,
    pub finish_time: Option<String>,
    pub result: Option<String>,
    pub message: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct ProjectUsage {
    pub total_vcpus_usage: f64,
    pub total_memory_mb_usage: f64,
    pub total_local_gb_usage: f64,
}

#[derive(Debug, Clone, Deserialize)]
pub struct ComputeQuota {
    pub cores: i64,
    pub ram: i64,
    pub instances: i64,
}

// --- Neutron params ---

#[derive(Debug, Clone)]
pub struct NetworkCreateParams {
    pub name: String,
    pub admin_state_up: bool,
    pub shared: Option<bool>,
    pub external: Option<bool>,
    pub mtu: Option<u32>,
    pub port_security_enabled: Option<bool>,
}

#[derive(Debug, Clone)]
pub struct NetworkUpdateParams {
    pub name: Option<String>,
    pub admin_state_up: Option<bool>,
    pub shared: Option<bool>,
    pub mtu: Option<u32>,
}

#[derive(Debug, Clone)]
pub struct SecurityGroupCreateParams {
    pub name: String,
    pub description: Option<String>,
}

#[derive(Debug, Clone)]
pub struct SecurityGroupUpdateParams {
    pub name: Option<String>,
    pub description: Option<String>,
}

#[derive(Debug, Clone)]
pub struct SecurityGroupRuleCreateParams {
    pub security_group_id: String,
    pub direction: RuleDirection,
    pub protocol: Option<String>,
    pub port_range_min: Option<u16>,
    pub port_range_max: Option<u16>,
    pub remote_ip_prefix: Option<String>,
    pub remote_group_id: Option<String>,
    pub ethertype: Option<String>,
}

#[derive(Debug, Clone)]
pub enum RuleDirection {
    Ingress,
    Egress,
}

#[derive(Debug, Clone)]
pub struct FloatingIpCreateParams {
    pub floating_network_id: String,
    pub port_id: Option<String>,
    pub fixed_ip_address: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct Subnet {
    pub id: String,
    pub name: String,
    pub network_id: String,
    pub cidr: String,
    pub ip_version: u8,
    pub gateway_ip: Option<String>,
}

// --- Cinder params ---

#[derive(Debug, Clone)]
pub struct VolumeCreateParams {
    pub name: String,
    pub size_gb: u32,
    pub volume_type: Option<String>,
    pub description: Option<String>,
    pub availability_zone: Option<String>,
}

#[derive(Debug, Clone)]
pub struct SnapshotCreateParams {
    pub volume_id: String,
    pub name: String,
    pub description: Option<String>,
    pub force: bool,
}

#[derive(Debug, Clone, Deserialize)]
pub struct QosSpec {
    pub id: String,
    pub name: String,
    pub consumer: String,
    pub specs: HashMap<String, String>,
}

#[derive(Debug, Clone)]
pub struct QosCreateParams {
    pub name: String,
    pub consumer: String,
    pub specs: HashMap<String, String>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct StoragePool {
    pub name: String,
    pub capabilities: HashMap<String, serde_json::Value>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct VolumeQuota {
    pub volumes: i64,
    pub gigabytes: i64,
    pub snapshots: i64,
}

#[derive(Debug, Clone)]
pub struct VolumeQuotaUpdateParams {
    pub volumes: Option<i64>,
    pub gigabytes: Option<i64>,
    pub snapshots: Option<i64>,
}

// --- Keystone params ---

#[derive(Debug, Clone)]
pub struct ProjectCreateParams {
    pub name: String,
    pub description: Option<String>,
    pub domain_id: String,
    pub enabled: Option<bool>,
}

#[derive(Debug, Clone)]
pub struct ProjectUpdateParams {
    pub name: Option<String>,
    pub description: Option<String>,
    pub enabled: Option<bool>,
}

#[derive(Clone)]
pub struct UserCreateParams {
    pub name: String,
    pub password: String,
    pub email: Option<String>,
    pub default_project_id: Option<String>,
    pub domain_id: String,
    pub enabled: Option<bool>,
}

impl std::fmt::Debug for UserCreateParams {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("UserCreateParams")
            .field("name", &self.name)
            .field("password", &"****")
            .field("email", &self.email)
            .field("default_project_id", &self.default_project_id)
            .field("domain_id", &self.domain_id)
            .field("enabled", &self.enabled)
            .finish()
    }
}

#[derive(Clone)]
pub struct UserUpdateParams {
    pub name: Option<String>,
    pub password: Option<String>,
    pub email: Option<String>,
    pub enabled: Option<bool>,
}

impl std::fmt::Debug for UserUpdateParams {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("UserUpdateParams")
            .field("name", &self.name)
            .field("password", &self.password.as_ref().map(|_| "****"))
            .field("email", &self.email)
            .field("enabled", &self.enabled)
            .finish()
    }
}

#[derive(Debug, Clone)]
pub struct RoleAssignmentParams {
    pub user_id: String,
    pub project_id: String,
    pub role_id: String,
}

#[derive(Debug, Clone, Default)]
pub struct RoleAssignmentFilter {
    pub user_id: Option<String>,
    pub project_id: Option<String>,
    pub role_id: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct Domain {
    pub id: String,
    pub name: String,
    pub enabled: bool,
}

// --- Glance params ---

#[derive(Debug, Clone)]
pub struct ImageCreateParams {
    pub name: String,
    pub disk_format: String,
    pub container_format: String,
    pub visibility: Option<String>,
    pub min_disk: Option<u32>,
    pub min_ram: Option<u32>,
}

#[derive(Debug, Clone)]
pub struct ImageUpdateParams {
    pub name: Option<String>,
    pub visibility: Option<String>,
}
