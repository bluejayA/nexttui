# Detail Design: Port Layer & Adapter Layer (13 Components)

**Timestamp**: 2026-03-23T11:00:00+09:00
**Scope**: Port Layer (6) + Adapter Layer (7)
**Prerequisites**: application-design.md, async-event-architecture-design.md

---

## Common Types (shared across all ports/adapters)

```rust
// src/domain/error.rs
#[derive(Debug, thiserror::Error)]
pub enum ApiError {
    #[error("Authentication failed: {0}")]
    AuthFailed(String),
    #[error("Token expired")]
    TokenExpired,
    #[error("Forbidden: {0}")]
    Forbidden(String),
    #[error("Not found: {resource_type} {id}")]
    NotFound { resource_type: String, id: String },
    #[error("Conflict: {0}")]
    Conflict(String),
    #[error("Bad request: {0}")]
    BadRequest(String),
    #[error("Rate limited: retry after {retry_after_secs}s")]
    RateLimited { retry_after_secs: u64 },
    #[error("Service unavailable: {service}")]
    ServiceUnavailable { service: String },
    #[error("Network error: {0}")]
    Network(#[from] reqwest::Error),
    #[error("Unexpected: {status} {body}")]
    Unexpected { status: u16, body: String },
}

pub type ApiResult<T> = Result<T, ApiError>;

// src/domain/pagination.rs
pub struct PaginationParams {
    pub marker: Option<String>,
    pub limit: Option<u32>,
    pub sort_key: Option<String>,
    pub sort_dir: Option<SortDirection>,
}

pub enum SortDirection { Asc, Desc }

pub struct PaginatedResponse<T> {
    pub items: Vec<T>,
    pub next_marker: Option<String>,
    pub has_more: bool,
}

// src/domain/auth.rs
#[derive(Clone, Debug)]
pub struct Token {
    pub id: String,              // X-Subject-Token value
    pub expires_at: DateTime<Utc>,
    pub project: ProjectScope,
    pub roles: Vec<Role>,
    pub catalog: Vec<CatalogEntry>,
}

#[derive(Clone, Debug)]
pub struct ProjectScope {
    pub id: String,
    pub name: String,
    pub domain_id: String,
    pub domain_name: String,
}

#[derive(Clone, Debug)]
pub struct CatalogEntry {
    pub service_type: String,    // "compute", "network", "volumev3", etc.
    pub service_name: String,
    pub endpoints: Vec<Endpoint>,
}

#[derive(Clone, Debug)]
pub struct Endpoint {
    pub region: String,
    pub interface: EndpointInterface,  // public, internal, admin
    pub url: String,
}

#[derive(Clone, Debug, PartialEq)]
pub enum EndpointInterface { Public, Internal, Admin }

#[derive(Clone, Debug)]
pub struct AuthCredential {
    pub auth_url: String,
    pub method: AuthMethod,
    pub project_scope: Option<ProjectScopeParam>,
}

pub enum AuthMethod {
    Password { username: String, password: String, domain_name: String },
    ApplicationCredential { id: String, secret: String },
    // Phase 2:
    // HmacKey { access_key: String, secret_key: String },
    // ApiKey { key: String },
}

pub struct ProjectScopeParam {
    pub name: String,
    pub domain_name: String,
}

// src/domain/filter.rs
pub struct ServerListFilter {
    pub name: Option<String>,
    pub status: Option<String>,
    pub host: Option<String>,
    pub flavor: Option<String>,
    pub all_tenants: bool,
}

pub struct VolumeListFilter {
    pub name: Option<String>,
    pub status: Option<String>,
    pub all_tenants: bool,
}

pub struct ImageListFilter {
    pub name: Option<String>,
    pub status: Option<String>,
    pub visibility: Option<String>,
}
```

---

## Port Layer (6 Components)

---

### 1. AuthProvider trait

**Responsibility**: Authentication abstraction -- token issue/refresh, service catalog resolution. Backend-agnostic interface for injecting auth into HTTP requests.

**Dependencies**: None (root of dependency tree)

**Data Owned**: None (trait only; implementors own token state)

```rust
// src/port/auth.rs

use async_trait::async_trait;
use tokio::sync::broadcast;

/// Phase 1: Keystone password/app-credential auth
/// Phase 2: HMAC (Cloudian), API Key, custom auth
///
/// [Agent Council Review 2026-03-23]
/// authenticate_request()를 통해 토큰 주입(Keystone)과 요청 서명(HMAC)을
/// 동일한 추상화 수준에서 처리. BaseHttpClient는 이 메서드에 인증을 위임.
#[async_trait]
pub trait AuthProvider: Send + Sync {
    /// Authenticate with the backend and obtain an initial token + catalog.
    async fn authenticate(&self, credential: &AuthCredential) -> ApiResult<Token>;

    /// Refresh the current token. Called automatically ~5min before expiry.
    /// Returns the new token.
    async fn refresh_token(&self) -> ApiResult<Token>;

    /// Get the current valid token string for X-Auth-Token header injection.
    /// If token is near-expiry, triggers refresh internally.
    async fn get_token(&self) -> ApiResult<String>;

    /// Get the current token's full metadata (roles, project, expiry).
    async fn get_token_info(&self) -> ApiResult<Token>;

    /// Sign/authenticate an outgoing HTTP request.
    /// - Keystone: injects X-Auth-Token header
    /// - HMAC (Phase 2): computes signature from method+url+body+timestamp
    /// - API Key (Phase 2): injects Authorization header
    /// BaseHttpClient delegates ALL auth injection to this method.
    async fn authenticate_request(
        &self,
        method: &str,
        url: &str,
        headers: &reqwest::header::HeaderMap,
        body: Option<&[u8]>,
    ) -> ApiResult<AuthHeaders>;

    /// Resolve the endpoint URL for a given service type + interface + region.
    /// Uses the service catalog from the last successful auth/refresh.
    async fn get_endpoint(
        &self,
        service_type: &str,
        interface: EndpointInterface,
        region: Option<&str>,
    ) -> ApiResult<String>;

    /// Subscribe to token refresh events. All adapters subscribe to this
    /// so they automatically pick up new tokens.
    fn subscribe_token_refresh(&self) -> broadcast::Receiver<Token>;

    /// Check whether the current token has the given role.
    async fn has_role(&self, role_name: &str) -> ApiResult<bool>;

    /// Returns the full service catalog from the current token.
    async fn get_catalog(&self) -> ApiResult<Vec<CatalogEntry>>;

    /// Returns the capabilities/permissions for the current session.
    /// Phase 1: derives from Keystone roles (admin/member/reader).
    /// Phase 2: may include backend-specific permissions.
    async fn get_capabilities(&self) -> ApiResult<Vec<Capability>>;
}

/// Auth headers to inject into outgoing requests.
/// Returned by authenticate_request().
pub struct AuthHeaders {
    pub headers: Vec<(String, String)>,
}

/// A capability/permission that the current session can perform.
/// Used by RbacGuard for Capability-based access control.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct Capability {
    pub resource: String,  // e.g. "server", "project", "aggregate"
    pub action: String,    // e.g. "create", "delete", "migrate"
}
```

**Interactions**:

```
                authenticate()
  App Startup ─────────────────► AuthProvider
                                     │
                                     ├─► Token + ServiceCatalog stored
                                     │
                 subscribe_token_refresh()
  NovaAdapter ◄──────────────────────┤
  NeutronAdapter ◄───────────────────┤   (broadcast::Receiver<Token>)
  CinderAdapter ◄────────────────────┤
  ...                                │
                                     │
               ┌─ tick (5min before) ─┘
               ▼
         refresh_token()
               │
               ├─► broadcast new Token to all subscribers
               └─► update internal token state
```

---

### 2. NovaPort trait

**Responsibility**: Nova Compute API abstraction -- servers CRUD, flavors, aggregates, compute services, migration, evacuate, snapshots, hypervisors, usage. Covers FR-08, FR-15, FR-16.1, FR-16.2, FR-16.3.

**Dependencies**: None (trait only)

**Data Owned**: None (trait only)

```rust
// src/port/nova.rs

#[async_trait]
pub trait NovaPort: Send + Sync {
    // ── Servers (FR-08.1 ~ FR-08.4) ──

    async fn list_servers(
        &self,
        filter: &ServerListFilter,
        pagination: &PaginationParams,
    ) -> ApiResult<PaginatedResponse<Server>>;

    async fn get_server(&self, server_id: &str) -> ApiResult<Server>;

    async fn create_server(&self, params: &ServerCreateParams) -> ApiResult<Server>;

    async fn delete_server(&self, server_id: &str) -> ApiResult<()>;

    async fn reboot_server(
        &self,
        server_id: &str,
        reboot_type: RebootType,  // Soft | Hard
    ) -> ApiResult<()>;

    async fn start_server(&self, server_id: &str) -> ApiResult<()>;

    async fn stop_server(&self, server_id: &str) -> ApiResult<()>;

    // ── Server State Force Change (FR-08.8, Admin) ──

    async fn force_set_server_state(
        &self,
        server_id: &str,
        state: ServerState,  // Active, Error, etc.
    ) -> ApiResult<()>;

    // ── Server Snapshot (FR-08.9) ──

    async fn create_server_snapshot(
        &self,
        server_id: &str,
        image_name: &str,
    ) -> ApiResult<String>;  // Returns image_id

    // ── Server Events (FR-16.3) ──

    async fn list_server_events(
        &self,
        server_id: &str,
    ) -> ApiResult<Vec<ServerEvent>>;

    // ── Migration (FR-08.6, Admin) ──

    async fn live_migrate_server(
        &self,
        server_id: &str,
        params: &LiveMigrateParams,
    ) -> ApiResult<()>;

    async fn cold_migrate_server(&self, server_id: &str) -> ApiResult<()>;

    async fn confirm_migration(&self, server_id: &str) -> ApiResult<()>;

    async fn revert_migration(&self, server_id: &str) -> ApiResult<()>;

    // ── Evacuate (FR-08.7, Admin) ──

    async fn evacuate_server(
        &self,
        server_id: &str,
        params: &EvacuateParams,
    ) -> ApiResult<()>;

    // ── Flavors (FR-08.5) ──

    async fn list_flavors(
        &self,
        pagination: &PaginationParams,
    ) -> ApiResult<PaginatedResponse<Flavor>>;

    async fn get_flavor(&self, flavor_id: &str) -> ApiResult<Flavor>;

    async fn create_flavor(&self, params: &FlavorCreateParams) -> ApiResult<Flavor>;

    async fn delete_flavor(&self, flavor_id: &str) -> ApiResult<()>;

    // ── Aggregates (FR-15.1, Admin) ──

    async fn list_aggregates(&self) -> ApiResult<Vec<Aggregate>>;

    async fn get_aggregate(&self, aggregate_id: i64) -> ApiResult<Aggregate>;

    async fn create_aggregate(&self, params: &AggregateCreateParams) -> ApiResult<Aggregate>;

    async fn update_aggregate(
        &self,
        aggregate_id: i64,
        params: &AggregateUpdateParams,
    ) -> ApiResult<Aggregate>;

    async fn delete_aggregate(&self, aggregate_id: i64) -> ApiResult<()>;

    async fn aggregate_add_host(
        &self,
        aggregate_id: i64,
        host: &str,
    ) -> ApiResult<Aggregate>;

    async fn aggregate_remove_host(
        &self,
        aggregate_id: i64,
        host: &str,
    ) -> ApiResult<Aggregate>;

    async fn aggregate_set_metadata(
        &self,
        aggregate_id: i64,
        metadata: &HashMap<String, String>,
    ) -> ApiResult<Aggregate>;

    // ── Compute Services (FR-15.2, Admin) ──

    async fn list_compute_services(&self) -> ApiResult<Vec<ComputeService>>;

    async fn enable_compute_service(&self, service_id: &str) -> ApiResult<ComputeService>;

    async fn disable_compute_service(
        &self,
        service_id: &str,
        reason: Option<&str>,
    ) -> ApiResult<ComputeService>;

    // ── Hypervisors (FR-16.1, Admin) ──

    async fn list_hypervisors(&self) -> ApiResult<Vec<Hypervisor>>;

    async fn get_hypervisor(&self, hypervisor_id: &str) -> ApiResult<Hypervisor>;

    // ── Usage (FR-16.2, Admin) ──

    async fn get_project_usage(
        &self,
        project_id: &str,
        start: DateTime<Utc>,
        end: DateTime<Utc>,
    ) -> ApiResult<ProjectUsage>;

    // ── Quota (Nova compute quota, FR-13.1) ──

    async fn get_compute_quota(
        &self,
        project_id: &str,
    ) -> ApiResult<ComputeQuota>;

    async fn update_compute_quota(
        &self,
        project_id: &str,
        params: &ComputeQuotaUpdateParams,
    ) -> ApiResult<ComputeQuota>;
}

// ── Supporting types ──

pub enum RebootType { Soft, Hard }

pub enum ServerState { Active, Error, Paused, Suspended, Stopped }

pub struct ServerCreateParams {
    pub name: String,
    pub image_id: String,
    pub flavor_id: String,
    pub networks: Vec<NetworkAttachment>,  // [{ uuid }] or "auto" or "none"
    pub security_groups: Option<Vec<String>>,
    pub key_name: Option<String>,
    pub availability_zone: Option<String>,
}

pub struct NetworkAttachment {
    pub uuid: String,
    pub fixed_ip: Option<String>,
}

pub struct LiveMigrateParams {
    pub host: Option<String>,       // None = scheduler decides
    pub block_migration: bool,      // true for block migration
}

pub struct EvacuateParams {
    pub host: Option<String>,       // None = scheduler decides
}

pub struct FlavorCreateParams {
    pub name: String,
    pub vcpus: u32,
    pub ram_mb: u32,
    pub disk_gb: u32,
    pub is_public: bool,
}

pub struct AggregateCreateParams {
    pub name: String,
    pub availability_zone: Option<String>,
}

pub struct AggregateUpdateParams {
    pub name: Option<String>,
    pub availability_zone: Option<String>,
}

pub struct ComputeQuotaUpdateParams {
    pub cores: Option<i64>,
    pub ram: Option<i64>,
    pub instances: Option<i64>,
    pub key_pairs: Option<i64>,
    pub server_groups: Option<i64>,
}
```

---

### 3. NeutronPort trait

**Responsibility**: Neutron Network API abstraction -- networks CRUD, security groups + rules CRUD, floating IPs, network agents. Covers FR-09.

**Dependencies**: None (trait only)

**Data Owned**: None (trait only)

```rust
// src/port/neutron.rs

#[async_trait]
pub trait NeutronPort: Send + Sync {
    // ── Networks (FR-09.1 ~ FR-09.3) ──

    async fn list_networks(
        &self,
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

    // ── Subnets (needed by network detail) ──

    async fn list_subnets(
        &self,
        network_id: Option<&str>,
    ) -> ApiResult<Vec<Subnet>>;

    // ── Security Groups (FR-09.4 ~ FR-09.6) ──

    async fn list_security_groups(
        &self,
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

    // ── Security Group Rules (FR-09.6) ──

    async fn create_security_group_rule(
        &self,
        params: &SecurityGroupRuleCreateParams,
    ) -> ApiResult<SecurityGroupRule>;

    async fn delete_security_group_rule(&self, rule_id: &str) -> ApiResult<()>;

    // ── Floating IPs (FR-09.7) ──

    async fn list_floating_ips(
        &self,
        pagination: &PaginationParams,
    ) -> ApiResult<PaginatedResponse<FloatingIp>>;

    async fn create_floating_ip(
        &self,
        params: &FloatingIpCreateParams,
    ) -> ApiResult<FloatingIp>;

    async fn delete_floating_ip(&self, fip_id: &str) -> ApiResult<()>;

    async fn associate_floating_ip(
        &self,
        fip_id: &str,
        port_id: &str,
    ) -> ApiResult<FloatingIp>;

    async fn disassociate_floating_ip(&self, fip_id: &str) -> ApiResult<FloatingIp>;

    // ── Network Agents (FR-09.8, Admin) ──

    async fn list_network_agents(&self) -> ApiResult<Vec<NetworkAgent>>;

    async fn enable_network_agent(&self, agent_id: &str) -> ApiResult<NetworkAgent>;

    async fn disable_network_agent(&self, agent_id: &str) -> ApiResult<NetworkAgent>;

    async fn delete_network_agent(&self, agent_id: &str) -> ApiResult<()>;
}

// ── Supporting types ──

pub struct NetworkCreateParams {
    pub name: String,
    pub admin_state_up: bool,
    pub shared: Option<bool>,
    pub external: Option<bool>,        // router:external
    pub mtu: Option<u32>,
    pub port_security_enabled: Option<bool>,
}

pub struct NetworkUpdateParams {
    pub name: Option<String>,
    pub admin_state_up: Option<bool>,
    pub shared: Option<bool>,
    pub mtu: Option<u32>,
}

pub struct SecurityGroupCreateParams {
    pub name: String,
    pub description: Option<String>,
}

pub struct SecurityGroupUpdateParams {
    pub name: Option<String>,
    pub description: Option<String>,
}

pub struct SecurityGroupRuleCreateParams {
    pub security_group_id: String,
    pub direction: RuleDirection,         // Ingress | Egress
    pub protocol: Option<String>,         // "tcp", "udp", "icmp", None = any
    pub port_range_min: Option<u16>,
    pub port_range_max: Option<u16>,
    pub remote_ip_prefix: Option<String>, // CIDR e.g. "0.0.0.0/0"
    pub remote_group_id: Option<String>,  // or reference another SG
    pub ethertype: Option<String>,        // "IPv4" | "IPv6"
}

pub enum RuleDirection { Ingress, Egress }

pub struct FloatingIpCreateParams {
    pub floating_network_id: String,
    pub port_id: Option<String>,
    pub fixed_ip_address: Option<String>,
}
```

---

### 4. CinderPort trait

**Responsibility**: Cinder Block Storage API abstraction -- volumes CRUD + actions, snapshots, QoS, storage pools, migration. Covers FR-10.

**Dependencies**: None (trait only)

**Data Owned**: None (trait only)

```rust
// src/port/cinder.rs

#[async_trait]
pub trait CinderPort: Send + Sync {
    // ── Volumes (FR-10.1 ~ FR-10.4) ──

    async fn list_volumes(
        &self,
        filter: &VolumeListFilter,
        pagination: &PaginationParams,
    ) -> ApiResult<PaginatedResponse<Volume>>;

    async fn get_volume(&self, volume_id: &str) -> ApiResult<Volume>;

    async fn create_volume(&self, params: &VolumeCreateParams) -> ApiResult<Volume>;

    async fn delete_volume(&self, volume_id: &str) -> ApiResult<()>;

    async fn force_delete_volume(&self, volume_id: &str) -> ApiResult<()>;

    async fn extend_volume(
        &self,
        volume_id: &str,
        new_size_gb: u32,
    ) -> ApiResult<()>;

    async fn attach_volume(
        &self,
        volume_id: &str,
        server_id: &str,
        device: Option<&str>,      // e.g. "/dev/vdb", None = auto
    ) -> ApiResult<()>;

    async fn detach_volume(
        &self,
        volume_id: &str,
        attachment_id: &str,
    ) -> ApiResult<()>;

    async fn force_set_volume_state(
        &self,
        volume_id: &str,
        state: &str,               // "available", "error", "in-use", etc.
    ) -> ApiResult<()>;

    // ── Volume Migration (FR-10.8, Admin) ──

    async fn migrate_volume(
        &self,
        volume_id: &str,
        dest_host: &str,
        force_host_copy: bool,
    ) -> ApiResult<()>;

    // ── Snapshots (FR-10.5) ──

    async fn list_snapshots(
        &self,
        pagination: &PaginationParams,
    ) -> ApiResult<PaginatedResponse<VolumeSnapshot>>;

    async fn get_snapshot(&self, snapshot_id: &str) -> ApiResult<VolumeSnapshot>;

    async fn create_snapshot(&self, params: &SnapshotCreateParams) -> ApiResult<VolumeSnapshot>;

    async fn delete_snapshot(&self, snapshot_id: &str) -> ApiResult<()>;

    // ── QoS Specs (FR-10.6, Admin) ──

    async fn list_qos_specs(&self) -> ApiResult<Vec<QosSpec>>;

    async fn get_qos_spec(&self, qos_id: &str) -> ApiResult<QosSpec>;

    async fn create_qos_spec(&self, params: &QosCreateParams) -> ApiResult<QosSpec>;

    async fn update_qos_spec(
        &self,
        qos_id: &str,
        specs: &HashMap<String, String>,
    ) -> ApiResult<QosSpec>;

    async fn delete_qos_spec(&self, qos_id: &str) -> ApiResult<()>;

    // ── Storage Pools (FR-10.7, Admin) ──

    async fn list_storage_pools(&self, detail: bool) -> ApiResult<Vec<StoragePool>>;

    // ── Volume Quota (FR-13.1) ──

    async fn get_volume_quota(&self, project_id: &str) -> ApiResult<VolumeQuota>;

    async fn update_volume_quota(
        &self,
        project_id: &str,
        params: &VolumeQuotaUpdateParams,
    ) -> ApiResult<VolumeQuota>;
}

// ── Supporting types ──

pub struct VolumeCreateParams {
    pub name: String,
    pub size_gb: u32,
    pub volume_type: Option<String>,
    pub description: Option<String>,
    pub availability_zone: Option<String>,
    pub source: Option<VolumeSource>,
}

pub enum VolumeSource {
    Snapshot(String),    // snapshot_id
    Image(String),       // image_id
    Volume(String),      // source volume_id (clone)
}

pub struct SnapshotCreateParams {
    pub volume_id: String,
    pub name: String,
    pub description: Option<String>,
    pub force: bool,             // snapshot even if volume is in-use
}

pub struct QosCreateParams {
    pub name: String,
    pub consumer: QosConsumer,   // Front-end | Back-end | Both
    pub specs: HashMap<String, String>,
}

pub enum QosConsumer { Frontend, Backend, Both }

pub struct VolumeQuotaUpdateParams {
    pub volumes: Option<i64>,
    pub gigabytes: Option<i64>,
    pub snapshots: Option<i64>,
    pub backups: Option<i64>,
    pub backup_gigabytes: Option<i64>,
}
```

---

### 5. KeystonePort trait

**Responsibility**: Keystone Admin API abstraction -- projects CRUD, users CRUD, role assignment/revocation, quota delegation. Covers FR-12, FR-13.

**Dependencies**: None (trait only)

**Data Owned**: None (trait only)

```rust
// src/port/keystone.rs

#[async_trait]
pub trait KeystonePort: Send + Sync {
    // ── Projects (FR-12.1) ──

    async fn list_projects(
        &self,
        pagination: &PaginationParams,
    ) -> ApiResult<PaginatedResponse<Project>>;

    async fn get_project(&self, project_id: &str) -> ApiResult<Project>;

    async fn create_project(&self, params: &ProjectCreateParams) -> ApiResult<Project>;

    async fn update_project(
        &self,
        project_id: &str,
        params: &ProjectUpdateParams,
    ) -> ApiResult<Project>;

    async fn delete_project(&self, project_id: &str) -> ApiResult<()>;

    // ── Users (FR-12.2) ──

    async fn list_users(
        &self,
        pagination: &PaginationParams,
    ) -> ApiResult<PaginatedResponse<User>>;

    async fn get_user(&self, user_id: &str) -> ApiResult<User>;

    async fn create_user(&self, params: &UserCreateParams) -> ApiResult<User>;

    async fn update_user(
        &self,
        user_id: &str,
        params: &UserUpdateParams,
    ) -> ApiResult<User>;

    async fn delete_user(&self, user_id: &str) -> ApiResult<()>;

    // ── Roles (FR-12.3) ──

    async fn list_roles(&self) -> ApiResult<Vec<Role>>;

    async fn assign_role(
        &self,
        params: &RoleAssignmentParams,
    ) -> ApiResult<()>;

    async fn revoke_role(
        &self,
        params: &RoleAssignmentParams,
    ) -> ApiResult<()>;

    async fn list_role_assignments(
        &self,
        filter: &RoleAssignmentFilter,
    ) -> ApiResult<Vec<RoleAssignment>>;

    // ── Domains (needed for project/user creation) ──

    async fn list_domains(&self) -> ApiResult<Vec<Domain>>;
}

// ── Supporting types ──

pub struct ProjectCreateParams {
    pub name: String,
    pub description: Option<String>,
    pub domain_id: String,
    pub enabled: Option<bool>,
}

pub struct ProjectUpdateParams {
    pub name: Option<String>,
    pub description: Option<String>,
    pub enabled: Option<bool>,
}

pub struct UserCreateParams {
    pub name: String,
    pub password: String,
    pub email: Option<String>,
    pub default_project_id: Option<String>,
    pub domain_id: String,
    pub enabled: Option<bool>,
}

pub struct UserUpdateParams {
    pub name: Option<String>,
    pub password: Option<String>,
    pub email: Option<String>,
    pub enabled: Option<bool>,
}

pub struct RoleAssignmentParams {
    pub user_id: String,
    pub project_id: String,
    pub role_id: String,
}

pub struct RoleAssignmentFilter {
    pub user_id: Option<String>,
    pub project_id: Option<String>,
    pub role_id: Option<String>,
}
```

---

### 6. GlancePort trait

**Responsibility**: Glance Image API abstraction -- images list/show/create/update/delete. Covers FR-14.

**Dependencies**: None (trait only)

**Data Owned**: None (trait only)

```rust
// src/port/glance.rs

#[async_trait]
pub trait GlancePort: Send + Sync {
    // ── Images (FR-14.1 ~ FR-14.5) ──

    async fn list_images(
        &self,
        filter: &ImageListFilter,
        pagination: &PaginationParams,
    ) -> ApiResult<PaginatedResponse<Image>>;

    async fn get_image(&self, image_id: &str) -> ApiResult<Image>;

    /// Create image metadata entry. Returns the image record.
    /// Actual data upload is a separate call (upload_image_data).
    async fn create_image(&self, params: &ImageCreateParams) -> ApiResult<Image>;

    /// Upload binary image data to an existing image record.
    /// `data` is a stream to support large files without full memory load.
    async fn upload_image_data(
        &self,
        image_id: &str,
        data: Box<dyn tokio::io::AsyncRead + Send + Unpin>,
        content_type: Option<&str>,
    ) -> ApiResult<()>;

    /// Import image from a URI (alternative to upload).
    async fn import_image(
        &self,
        image_id: &str,
        method: ImageImportMethod,
    ) -> ApiResult<()>;

    async fn update_image(
        &self,
        image_id: &str,
        params: &ImageUpdateParams,
    ) -> ApiResult<Image>;

    async fn delete_image(&self, image_id: &str) -> ApiResult<()>;

    /// Deactivate image (Admin) -- prevents download.
    async fn deactivate_image(&self, image_id: &str) -> ApiResult<()>;

    /// Reactivate image (Admin).
    async fn reactivate_image(&self, image_id: &str) -> ApiResult<()>;
}

// ── Supporting types ──

pub struct ImageCreateParams {
    pub name: String,
    pub disk_format: DiskFormat,
    pub container_format: ContainerFormat,
    pub visibility: ImageVisibility,
    pub min_disk_gb: Option<u32>,
    pub min_ram_mb: Option<u32>,
    pub properties: Option<HashMap<String, String>>,
}

pub enum DiskFormat { Raw, Qcow2, Vmdk, Vdi, Iso, Vhd, Aki, Ari, Ami }

pub enum ContainerFormat { Bare, Ovf, Ova, Aki, Ari, Ami, Docker }

pub enum ImageVisibility { Public, Private, Shared, Community }

pub enum ImageImportMethod {
    WebDownload { uri: String },
    // Phase 2: GlanceDirect, Copy
}

pub struct ImageUpdateParams {
    pub name: Option<String>,
    pub visibility: Option<ImageVisibility>,
    pub min_disk_gb: Option<u32>,
    pub min_ram_mb: Option<u32>,
    pub properties: Option<HashMap<String, String>>,
}
```

---

## Adapter Layer (7 Components)

---

### Common Base: `BaseHttpClient`

All HTTP adapters share this foundation for auth header injection, endpoint resolution, error mapping, and JSON deserialization. This is NOT a trait -- it is a concrete utility struct composed into each adapter.

**Responsibility**: Shared HTTP plumbing -- auth injection, endpoint resolution, response error mapping, JSON deser.

**Dependencies**: `AuthProvider`, `reqwest::Client`

**Data Owned**: `reqwest::Client`, `Arc<dyn AuthProvider>`, resolved endpoint URL, service type string

```rust
// src/adapter/http/base.rs

pub struct BaseHttpClient {
    client: reqwest::Client,
    auth: Arc<dyn AuthProvider>,
    service_type: String,           // "compute", "network", "volumev3", "identity", "image"
    interface: EndpointInterface,   // typically Internal for datacenter deployments
    region: Option<String>,
    /// Cached endpoint URL. Refreshed on token refresh.
    endpoint: RwLock<Option<String>>,
}

impl BaseHttpClient {
    pub fn new(
        auth: Arc<dyn AuthProvider>,
        service_type: &str,
        interface: EndpointInterface,
        region: Option<String>,
    ) -> Self {
        Self {
            client: reqwest::Client::builder()
                .timeout(Duration::from_secs(30))
                .connect_timeout(Duration::from_secs(10))
                .build()
                .expect("failed to build HTTP client"),
            auth,
            service_type: service_type.to_string(),
            interface,
            region,
            endpoint: RwLock::new(None),
        }
    }

    /// Resolve and cache the endpoint from service catalog.
    async fn resolve_endpoint(&self) -> ApiResult<String> {
        {
            let cached = self.endpoint.read().await;
            if let Some(url) = cached.as_ref() {
                return Ok(url.clone());
            }
        }
        let url = self.auth.get_endpoint(
            &self.service_type,
            self.interface.clone(),
            self.region.as_deref(),
        ).await?;
        let mut cached = self.endpoint.write().await;
        *cached = Some(url.clone());
        Ok(url)
    }

    /// Invalidate cached endpoint (called on token refresh).
    pub async fn invalidate_endpoint(&self) {
        let mut cached = self.endpoint.write().await;
        *cached = None;
    }

    /// Build an authenticated request.
    /// [Agent Council Review] Auth injection is delegated to AuthProvider::authenticate_request().
    /// This makes BaseHttpClient auth-scheme-agnostic:
    /// - Keystone: injects X-Auth-Token
    /// - HMAC (Phase 2): computes request signature
    /// - API Key (Phase 2): injects Authorization header
    async fn request(&self, method: Method, path: &str) -> ApiResult<RequestBuilder> {
        let endpoint = self.resolve_endpoint().await?;
        let url = format!("{}{}", endpoint.trim_end_matches('/'), path);
        let method_str = method.as_str();
        let empty_headers = reqwest::header::HeaderMap::new();
        let auth_headers = self.auth
            .authenticate_request(method_str, &url, &empty_headers, None)
            .await?;
        let mut builder = self.client
            .request(method, &url)
            .header("Content-Type", "application/json");
        for (key, value) in &auth_headers.headers {
            builder = builder.header(key.as_str(), value.as_str());
        }
        Ok(builder)
    }

    // ── Convenience methods ──

    pub async fn get(&self, path: &str) -> ApiResult<RequestBuilder> {
        self.request(Method::GET, path).await
    }

    pub async fn post(&self, path: &str) -> ApiResult<RequestBuilder> {
        self.request(Method::POST, path).await
    }

    pub async fn put(&self, path: &str) -> ApiResult<RequestBuilder> {
        self.request(Method::PUT, path).await
    }

    pub async fn patch(&self, path: &str) -> ApiResult<RequestBuilder> {
        self.request(Method::PATCH, path).await
    }

    pub async fn delete(&self, path: &str) -> ApiResult<RequestBuilder> {
        self.request(Method::DELETE, path).await
    }

    /// Send a request and map HTTP errors to ApiError.
    pub async fn send(&self, request: RequestBuilder) -> ApiResult<Response> {
        let resp = request.send().await.map_err(ApiError::Network)?;
        Self::check_status(resp).await
    }

    /// Send + deserialize JSON body.
    pub async fn send_json<T: DeserializeOwned>(
        &self,
        request: RequestBuilder,
    ) -> ApiResult<T> {
        let resp = self.send(request).await?;
        resp.json::<T>().await.map_err(ApiError::Network)
    }

    /// Send and expect 204 No Content (or 202 Accepted).
    pub async fn send_no_content(&self, request: RequestBuilder) -> ApiResult<()> {
        self.send(request).await?;
        Ok(())
    }

    /// Map HTTP status codes to ApiError.
    async fn check_status(resp: Response) -> ApiResult<Response> {
        let status = resp.status();
        if status.is_success() {
            return Ok(resp);
        }
        let body = resp.text().await.unwrap_or_default();
        match status.as_u16() {
            401 => Err(ApiError::TokenExpired),
            403 => Err(ApiError::Forbidden(body)),
            404 => Err(ApiError::NotFound {
                resource_type: String::new(),
                id: String::new(),
            }),
            409 => Err(ApiError::Conflict(body)),
            400 => Err(ApiError::BadRequest(body)),
            429 => {
                // Parse Retry-After header if present
                Err(ApiError::RateLimited { retry_after_secs: 60 })
            }
            503 => Err(ApiError::ServiceUnavailable {
                service: String::new(),
            }),
            _ => Err(ApiError::Unexpected {
                status: status.as_u16(),
                body,
            }),
        }
    }
}
```

**Interactions -- Common HTTP Adapter Pattern**:

```
  Any HttpAdapter method call (e.g., list_servers)
       │
       ▼
  BaseHttpClient::get("/servers/detail?...")
       │
       ├─ resolve_endpoint()
       │      │
       │      ├─ cache hit? ──► use cached URL
       │      └─ cache miss? ─► AuthProvider::get_endpoint() ──► cache it
       │
       ├─ AuthProvider::get_token()
       │      │
       │      └─ near expiry? ──► refresh_token() internally
       │
       ├─ Build reqwest::RequestBuilder with token + URL
       │
       ▼
  BaseHttpClient::send_json::<NovaServersResponse>()
       │
       ├─ reqwest send()
       ├─ check_status() ──► map HTTP errors to ApiError
       └─ serde deserialize ──► domain model
```

---

### 7. KeystoneAuthAdapter

**Responsibility**: Keystone v3 token lifecycle -- password/app-credential auth, token refresh (5min before expiry), service catalog parsing, broadcast token refresh to all adapters.

**Dependencies**: `reqwest::Client` (direct, no BaseHttpClient -- this IS the auth provider)

**Data Owned**: `AuthCredential`, `Token` (current), `broadcast::Sender<Token>`, `reqwest::Client`, refresh task handle

```rust
// src/adapter/auth/keystone.rs

pub struct KeystoneAuthAdapter {
    client: reqwest::Client,
    credential: AuthCredential,
    /// Current token, protected by RwLock for concurrent reads.
    current_token: Arc<RwLock<Option<Token>>>,
    /// Broadcast sender for token refresh notifications.
    token_tx: broadcast::Sender<Token>,
    /// Handle to the background refresh task.
    refresh_handle: Mutex<Option<JoinHandle<()>>>,
}

impl KeystoneAuthAdapter {
    pub fn new(credential: AuthCredential) -> Self {
        let (token_tx, _) = broadcast::channel::<Token>(16);
        Self {
            client: reqwest::Client::builder()
                .timeout(Duration::from_secs(30))
                .build()
                .expect("HTTP client"),
            credential,
            current_token: Arc::new(RwLock::new(None)),
            token_tx,
            refresh_handle: Mutex::new(None),
        }
    }

    /// Start the background refresh loop. Called once after initial authenticate().
    fn start_refresh_loop(&self) {
        let token_ref = self.current_token.clone();
        let client = self.client.clone();
        let credential = self.credential.clone();
        let tx = self.token_tx.clone();

        let handle = tokio::spawn(async move {
            loop {
                let sleep_duration = {
                    let token = token_ref.read().await;
                    match token.as_ref() {
                        Some(t) => {
                            let remaining = t.expires_at - Utc::now();
                            let refresh_at = remaining - chrono::Duration::minutes(5);
                            if refresh_at.num_seconds() > 0 {
                                Duration::from_secs(refresh_at.num_seconds() as u64)
                            } else {
                                Duration::from_secs(10) // already near-expiry
                            }
                        }
                        None => Duration::from_secs(60),
                    }
                };

                tokio::time::sleep(sleep_duration).await;

                // Perform token refresh
                match Self::do_authenticate(&client, &credential).await {
                    Ok(new_token) => {
                        let mut current = token_ref.write().await;
                        *current = Some(new_token.clone());
                        let _ = tx.send(new_token); // broadcast to all adapters
                    }
                    Err(e) => {
                        tracing::error!("Token refresh failed: {e}");
                        // Retry after short delay
                        tokio::time::sleep(Duration::from_secs(30)).await;
                    }
                }
            }
        });

        // Store handle (dropping it would NOT cancel the task, but we keep it for shutdown)
        // self.refresh_handle locked externally
    }

    /// Internal: perform the actual Keystone v3 auth POST.
    async fn do_authenticate(
        client: &reqwest::Client,
        credential: &AuthCredential,
    ) -> ApiResult<Token> {
        let auth_url = format!(
            "{}/auth/tokens",
            credential.auth_url.trim_end_matches('/')
        );
        let body = Self::build_auth_body(credential);
        let resp = client
            .post(&auth_url)
            .json(&body)
            .send()
            .await
            .map_err(ApiError::Network)?;

        if !resp.status().is_success() {
            let body = resp.text().await.unwrap_or_default();
            return Err(ApiError::AuthFailed(body));
        }

        let token_id = resp
            .headers()
            .get("X-Subject-Token")
            .and_then(|v| v.to_str().ok())
            .ok_or_else(|| ApiError::AuthFailed("Missing X-Subject-Token".into()))?
            .to_string();

        let body: KeystoneTokenResponse = resp.json().await.map_err(ApiError::Network)?;
        Ok(Token::from_keystone(token_id, body))
    }

    fn build_auth_body(credential: &AuthCredential) -> serde_json::Value {
        // Builds the Keystone v3 auth JSON based on AuthMethod variant
        // (password vs application_credential)
        todo!("implementation detail")
    }
}

#[async_trait]
impl AuthProvider for KeystoneAuthAdapter {
    async fn authenticate(&self, credential: &AuthCredential) -> ApiResult<Token> {
        let token = Self::do_authenticate(&self.client, credential).await?;
        {
            let mut current = self.current_token.write().await;
            *current = Some(token.clone());
        }
        self.start_refresh_loop();
        Ok(token)
    }

    async fn refresh_token(&self) -> ApiResult<Token> {
        let token = Self::do_authenticate(&self.client, &self.credential).await?;
        {
            let mut current = self.current_token.write().await;
            *current = Some(token.clone());
        }
        let _ = self.token_tx.send(token.clone());
        Ok(token)
    }

    async fn get_token(&self) -> ApiResult<String> {
        let current = self.current_token.read().await;
        match current.as_ref() {
            Some(t) if t.expires_at > Utc::now() + chrono::Duration::minutes(1) => {
                Ok(t.id.clone())
            }
            _ => {
                drop(current);
                let token = self.refresh_token().await?;
                Ok(token.id)
            }
        }
    }

    async fn get_token_info(&self) -> ApiResult<Token> {
        let current = self.current_token.read().await;
        current.clone().ok_or(ApiError::AuthFailed("Not authenticated".into()))
    }

    async fn get_endpoint(
        &self,
        service_type: &str,
        interface: EndpointInterface,
        region: Option<&str>,
    ) -> ApiResult<String> {
        let current = self.current_token.read().await;
        let token = current.as_ref()
            .ok_or(ApiError::AuthFailed("Not authenticated".into()))?;

        token.catalog.iter()
            .find(|c| c.service_type == service_type)
            .and_then(|c| {
                c.endpoints.iter().find(|e| {
                    e.interface == interface
                        && region.map_or(true, |r| e.region == r)
                })
            })
            .map(|e| e.url.clone())
            .ok_or(ApiError::ServiceUnavailable {
                service: service_type.to_string(),
            })
    }

    fn subscribe_token_refresh(&self) -> broadcast::Receiver<Token> {
        self.token_tx.subscribe()
    }

    async fn has_role(&self, role_name: &str) -> ApiResult<bool> {
        let current = self.current_token.read().await;
        let token = current.as_ref()
            .ok_or(ApiError::AuthFailed("Not authenticated".into()))?;
        Ok(token.roles.iter().any(|r| r.name == role_name))
    }

    async fn get_catalog(&self) -> ApiResult<Vec<CatalogEntry>> {
        let current = self.current_token.read().await;
        let token = current.as_ref()
            .ok_or(ApiError::AuthFailed("Not authenticated".into()))?;
        Ok(token.catalog.clone())
    }
}
```

**Interactions -- Auth + Refresh Flow**:

```
  App startup
       │
       ▼
  KeystoneAuthAdapter::authenticate(credential)
       │
       ├─ POST /v3/auth/tokens
       │      Body: { "auth": { "identity": {...}, "scope": {...} } }
       │      Response Header: X-Subject-Token: <token_id>
       │      Response Body: { "token": { "catalog": [...], "roles": [...], ... } }
       │
       ├─ Store Token in RwLock
       ├─ start_refresh_loop()
       │      │
       │      └─ tokio::spawn ──► loop {
       │              sleep(expires_at - 5min - now);
       │              POST /v3/auth/tokens (re-auth)
       │              broadcast::send(new_token)  ──► all adapters
       │          }
       │
       └─ Return Token to App

  During operation:
  ┌──────────────────────────────────────────────────────────┐
  │  NovaHttpAdapter ─► BaseHttpClient::get_token()          │
  │       │                    │                              │
  │       │          RwLock<Token>.read() ──► token.id       │
  │       │                                                  │
  │  Meanwhile, refresh loop:                                │
  │       broadcast::send(new_token)                         │
  │            │                                             │
  │            ├──► NovaHttpAdapter (invalidate_endpoint)    │
  │            ├──► NeutronHttpAdapter                       │
  │            └──► CinderHttpAdapter                        │
  └──────────────────────────────────────────────────────────┘
```

---

### 8. NovaHttpAdapter

**Responsibility**: Nova REST API calls via reqwest, JSON deserialization to domain models via serde. Implements `NovaPort`.

**Dependencies**: `BaseHttpClient`, `AuthProvider` (via base)

**Data Owned**: `BaseHttpClient` (service_type = "compute")

```rust
// src/adapter/http/nova.rs

pub struct NovaHttpAdapter {
    base: BaseHttpClient,
}

impl NovaHttpAdapter {
    pub fn new(auth: Arc<dyn AuthProvider>, region: Option<String>) -> Self {
        Self {
            base: BaseHttpClient::new(auth, "compute", EndpointInterface::Internal, region),
        }
    }
}

#[async_trait]
impl NovaPort for NovaHttpAdapter {
    async fn list_servers(
        &self,
        filter: &ServerListFilter,
        pagination: &PaginationParams,
    ) -> ApiResult<PaginatedResponse<Server>> {
        let mut path = "/servers/detail".to_string();
        let query = build_server_query(filter, pagination);
        if !query.is_empty() {
            path.push_str(&format!("?{query}"));
        }
        let req = self.base.get(&path).await?;
        let resp: NovaServersResponse = self.base.send_json(req).await?;
        Ok(PaginatedResponse {
            items: resp.servers.into_iter().map(Server::from).collect(),
            next_marker: resp.servers_links.and_then(extract_next_marker),
            has_more: resp.servers_links.is_some(),
        })
    }

    async fn get_server(&self, server_id: &str) -> ApiResult<Server> {
        let req = self.base.get(&format!("/servers/{server_id}")).await?;
        let resp: NovaServerWrapper = self.base.send_json(req).await?;
        Ok(Server::from(resp.server))
    }

    async fn create_server(&self, params: &ServerCreateParams) -> ApiResult<Server> {
        let body = NovaServerCreateRequest::from(params);
        let req = self.base.post("/servers").await?.json(&body);
        let resp: NovaServerWrapper = self.base.send_json(req).await?;
        Ok(Server::from(resp.server))
    }

    async fn delete_server(&self, server_id: &str) -> ApiResult<()> {
        let req = self.base.delete(&format!("/servers/{server_id}")).await?;
        self.base.send_no_content(req).await
    }

    async fn reboot_server(&self, server_id: &str, reboot_type: RebootType) -> ApiResult<()> {
        let body = serde_json::json!({
            "reboot": { "type": reboot_type.as_str() }
        });
        let req = self.base.post(&format!("/servers/{server_id}/action")).await?.json(&body);
        self.base.send_no_content(req).await
    }

    async fn start_server(&self, server_id: &str) -> ApiResult<()> {
        let body = serde_json::json!({ "os-start": null });
        let req = self.base.post(&format!("/servers/{server_id}/action")).await?.json(&body);
        self.base.send_no_content(req).await
    }

    async fn stop_server(&self, server_id: &str) -> ApiResult<()> {
        let body = serde_json::json!({ "os-stop": null });
        let req = self.base.post(&format!("/servers/{server_id}/action")).await?.json(&body);
        self.base.send_no_content(req).await
    }

    async fn force_set_server_state(&self, server_id: &str, state: ServerState) -> ApiResult<()> {
        let body = serde_json::json!({
            "os-resetState": { "state": state.as_str() }
        });
        let req = self.base.post(&format!("/servers/{server_id}/action")).await?.json(&body);
        self.base.send_no_content(req).await
    }

    async fn create_server_snapshot(&self, server_id: &str, image_name: &str) -> ApiResult<String> {
        let body = serde_json::json!({
            "createImage": { "name": image_name }
        });
        let req = self.base.post(&format!("/servers/{server_id}/action")).await?.json(&body);
        let resp = self.base.send(req).await?;
        // Image ID is in the Location header or response body
        let image_id = resp.headers()
            .get("Location")
            .and_then(|v| v.to_str().ok())
            .and_then(|url| url.rsplit('/').next())
            .map(String::from)
            .ok_or(ApiError::Unexpected { status: 200, body: "Missing Location header".into() })?;
        Ok(image_id)
    }

    async fn live_migrate_server(&self, server_id: &str, params: &LiveMigrateParams) -> ApiResult<()> {
        let body = serde_json::json!({
            "os-migrateLive": {
                "host": params.host,
                "block_migration": params.block_migration,
                "disk_over_commit": false,
            }
        });
        let req = self.base.post(&format!("/servers/{server_id}/action")).await?.json(&body);
        self.base.send_no_content(req).await
    }

    async fn cold_migrate_server(&self, server_id: &str) -> ApiResult<()> {
        let body = serde_json::json!({ "migrate": null });
        let req = self.base.post(&format!("/servers/{server_id}/action")).await?.json(&body);
        self.base.send_no_content(req).await
    }

    async fn confirm_migration(&self, server_id: &str) -> ApiResult<()> {
        let body = serde_json::json!({ "confirmResize": null });
        let req = self.base.post(&format!("/servers/{server_id}/action")).await?.json(&body);
        self.base.send_no_content(req).await
    }

    async fn revert_migration(&self, server_id: &str) -> ApiResult<()> {
        let body = serde_json::json!({ "revertResize": null });
        let req = self.base.post(&format!("/servers/{server_id}/action")).await?.json(&body);
        self.base.send_no_content(req).await
    }

    async fn evacuate_server(&self, server_id: &str, params: &EvacuateParams) -> ApiResult<()> {
        let body = serde_json::json!({
            "evacuate": { "host": params.host }
        });
        let req = self.base.post(&format!("/servers/{server_id}/action")).await?.json(&body);
        self.base.send_no_content(req).await
    }

    async fn list_server_events(&self, server_id: &str) -> ApiResult<Vec<ServerEvent>> {
        let req = self.base.get(&format!("/servers/{server_id}/os-instance-actions")).await?;
        let resp: NovaActionsResponse = self.base.send_json(req).await?;
        Ok(resp.instance_actions.into_iter().map(ServerEvent::from).collect())
    }

    // ── Flavors ──

    async fn list_flavors(&self, pagination: &PaginationParams) -> ApiResult<PaginatedResponse<Flavor>> {
        let mut path = "/flavors/detail".to_string();
        append_pagination(&mut path, pagination);
        let req = self.base.get(&path).await?;
        let resp: NovaFlavorsResponse = self.base.send_json(req).await?;
        Ok(PaginatedResponse {
            items: resp.flavors.into_iter().map(Flavor::from).collect(),
            next_marker: resp.flavors_links.and_then(extract_next_marker),
            has_more: resp.flavors_links.is_some(),
        })
    }

    async fn get_flavor(&self, flavor_id: &str) -> ApiResult<Flavor> {
        let req = self.base.get(&format!("/flavors/{flavor_id}")).await?;
        let resp: NovaFlavorWrapper = self.base.send_json(req).await?;
        Ok(Flavor::from(resp.flavor))
    }

    async fn create_flavor(&self, params: &FlavorCreateParams) -> ApiResult<Flavor> {
        let body = NovaFlavorCreateRequest::from(params);
        let req = self.base.post("/flavors").await?.json(&body);
        let resp: NovaFlavorWrapper = self.base.send_json(req).await?;
        Ok(Flavor::from(resp.flavor))
    }

    async fn delete_flavor(&self, flavor_id: &str) -> ApiResult<()> {
        let req = self.base.delete(&format!("/flavors/{flavor_id}")).await?;
        self.base.send_no_content(req).await
    }

    // ── Aggregates ──

    async fn list_aggregates(&self) -> ApiResult<Vec<Aggregate>> {
        let req = self.base.get("/os-aggregates").await?;
        let resp: NovaAggregatesResponse = self.base.send_json(req).await?;
        Ok(resp.aggregates.into_iter().map(Aggregate::from).collect())
    }

    async fn get_aggregate(&self, aggregate_id: i64) -> ApiResult<Aggregate> {
        let req = self.base.get(&format!("/os-aggregates/{aggregate_id}")).await?;
        let resp: NovaAggregateWrapper = self.base.send_json(req).await?;
        Ok(Aggregate::from(resp.aggregate))
    }

    async fn create_aggregate(&self, params: &AggregateCreateParams) -> ApiResult<Aggregate> {
        let body = serde_json::json!({
            "aggregate": {
                "name": params.name,
                "availability_zone": params.availability_zone,
            }
        });
        let req = self.base.post("/os-aggregates").await?.json(&body);
        let resp: NovaAggregateWrapper = self.base.send_json(req).await?;
        Ok(Aggregate::from(resp.aggregate))
    }

    async fn update_aggregate(&self, aggregate_id: i64, params: &AggregateUpdateParams) -> ApiResult<Aggregate> {
        let body = serde_json::json!({ "aggregate": params });
        let req = self.base.put(&format!("/os-aggregates/{aggregate_id}")).await?.json(&body);
        let resp: NovaAggregateWrapper = self.base.send_json(req).await?;
        Ok(Aggregate::from(resp.aggregate))
    }

    async fn delete_aggregate(&self, aggregate_id: i64) -> ApiResult<()> {
        let req = self.base.delete(&format!("/os-aggregates/{aggregate_id}")).await?;
        self.base.send_no_content(req).await
    }

    async fn aggregate_add_host(&self, aggregate_id: i64, host: &str) -> ApiResult<Aggregate> {
        let body = serde_json::json!({ "add_host": { "host": host } });
        let req = self.base.post(&format!("/os-aggregates/{aggregate_id}/action")).await?.json(&body);
        let resp: NovaAggregateWrapper = self.base.send_json(req).await?;
        Ok(Aggregate::from(resp.aggregate))
    }

    async fn aggregate_remove_host(&self, aggregate_id: i64, host: &str) -> ApiResult<Aggregate> {
        let body = serde_json::json!({ "remove_host": { "host": host } });
        let req = self.base.post(&format!("/os-aggregates/{aggregate_id}/action")).await?.json(&body);
        let resp: NovaAggregateWrapper = self.base.send_json(req).await?;
        Ok(Aggregate::from(resp.aggregate))
    }

    async fn aggregate_set_metadata(&self, aggregate_id: i64, metadata: &HashMap<String, String>) -> ApiResult<Aggregate> {
        let body = serde_json::json!({ "set_metadata": { "metadata": metadata } });
        let req = self.base.post(&format!("/os-aggregates/{aggregate_id}/action")).await?.json(&body);
        let resp: NovaAggregateWrapper = self.base.send_json(req).await?;
        Ok(Aggregate::from(resp.aggregate))
    }

    // ── Compute Services ──

    async fn list_compute_services(&self) -> ApiResult<Vec<ComputeService>> {
        let req = self.base.get("/os-services").await?;
        let resp: NovaServicesResponse = self.base.send_json(req).await?;
        Ok(resp.services.into_iter().map(ComputeService::from).collect())
    }

    async fn enable_compute_service(&self, service_id: &str) -> ApiResult<ComputeService> {
        let body = serde_json::json!({ "status": "enabled" });
        let req = self.base.put(&format!("/os-services/{service_id}")).await?.json(&body);
        let resp: NovaServiceWrapper = self.base.send_json(req).await?;
        Ok(ComputeService::from(resp.service))
    }

    async fn disable_compute_service(&self, service_id: &str, reason: Option<&str>) -> ApiResult<ComputeService> {
        let body = match reason {
            Some(r) => serde_json::json!({ "status": "disabled", "disabled_reason": r }),
            None => serde_json::json!({ "status": "disabled" }),
        };
        let req = self.base.put(&format!("/os-services/{service_id}")).await?.json(&body);
        let resp: NovaServiceWrapper = self.base.send_json(req).await?;
        Ok(ComputeService::from(resp.service))
    }

    // ── Hypervisors ──

    async fn list_hypervisors(&self) -> ApiResult<Vec<Hypervisor>> {
        let req = self.base.get("/os-hypervisors/detail").await?;
        let resp: NovaHypervisorsResponse = self.base.send_json(req).await?;
        Ok(resp.hypervisors.into_iter().map(Hypervisor::from).collect())
    }

    async fn get_hypervisor(&self, hypervisor_id: &str) -> ApiResult<Hypervisor> {
        let req = self.base.get(&format!("/os-hypervisors/{hypervisor_id}")).await?;
        let resp: NovaHypervisorWrapper = self.base.send_json(req).await?;
        Ok(Hypervisor::from(resp.hypervisor))
    }

    // ── Usage ──

    async fn get_project_usage(
        &self,
        project_id: &str,
        start: DateTime<Utc>,
        end: DateTime<Utc>,
    ) -> ApiResult<ProjectUsage> {
        let path = format!(
            "/os-simple-tenant-usage/{}?start={}&end={}",
            project_id,
            start.format("%Y-%m-%dT%H:%M:%S"),
            end.format("%Y-%m-%dT%H:%M:%S"),
        );
        let req = self.base.get(&path).await?;
        let resp: NovaUsageWrapper = self.base.send_json(req).await?;
        Ok(ProjectUsage::from(resp.tenant_usage))
    }

    // ── Quota ──

    async fn get_compute_quota(&self, project_id: &str) -> ApiResult<ComputeQuota> {
        let req = self.base.get(&format!("/os-quota-sets/{project_id}")).await?;
        let resp: NovaQuotaWrapper = self.base.send_json(req).await?;
        Ok(ComputeQuota::from(resp.quota_set))
    }

    async fn update_compute_quota(&self, project_id: &str, params: &ComputeQuotaUpdateParams) -> ApiResult<ComputeQuota> {
        let body = serde_json::json!({ "quota_set": params });
        let req = self.base.put(&format!("/os-quota-sets/{project_id}")).await?.json(&body);
        let resp: NovaQuotaWrapper = self.base.send_json(req).await?;
        Ok(ComputeQuota::from(resp.quota_set))
    }
}
```

---

### 9. NeutronHttpAdapter

**Responsibility**: Neutron REST API calls -- networks, security groups/rules, floating IPs, agents.

**Dependencies**: `BaseHttpClient` (service_type = "network")

**Data Owned**: `BaseHttpClient`

```rust
// src/adapter/http/neutron.rs

pub struct NeutronHttpAdapter {
    base: BaseHttpClient,
}

impl NeutronHttpAdapter {
    pub fn new(auth: Arc<dyn AuthProvider>, region: Option<String>) -> Self {
        Self {
            base: BaseHttpClient::new(auth, "network", EndpointInterface::Internal, region),
        }
    }
}

#[async_trait]
impl NeutronPort for NeutronHttpAdapter {
    // ── Networks ──

    async fn list_networks(&self, pagination: &PaginationParams) -> ApiResult<PaginatedResponse<Network>> {
        let mut path = "/v2.0/networks".to_string();
        append_pagination(&mut path, pagination);
        let req = self.base.get(&path).await?;
        let resp: NeutronNetworksResponse = self.base.send_json(req).await?;
        Ok(PaginatedResponse {
            items: resp.networks.into_iter().map(Network::from).collect(),
            next_marker: resp.networks_links.and_then(extract_next_marker),
            has_more: resp.networks_links.is_some(),
        })
    }

    async fn get_network(&self, network_id: &str) -> ApiResult<Network> {
        let req = self.base.get(&format!("/v2.0/networks/{network_id}")).await?;
        let resp: NeutronNetworkWrapper = self.base.send_json(req).await?;
        Ok(Network::from(resp.network))
    }

    async fn create_network(&self, params: &NetworkCreateParams) -> ApiResult<Network> {
        let body = NeutronNetworkCreateRequest::from(params);
        let req = self.base.post("/v2.0/networks").await?.json(&body);
        let resp: NeutronNetworkWrapper = self.base.send_json(req).await?;
        Ok(Network::from(resp.network))
    }

    async fn update_network(&self, network_id: &str, params: &NetworkUpdateParams) -> ApiResult<Network> {
        let body = serde_json::json!({ "network": params });
        let req = self.base.put(&format!("/v2.0/networks/{network_id}")).await?.json(&body);
        let resp: NeutronNetworkWrapper = self.base.send_json(req).await?;
        Ok(Network::from(resp.network))
    }

    async fn delete_network(&self, network_id: &str) -> ApiResult<()> {
        let req = self.base.delete(&format!("/v2.0/networks/{network_id}")).await?;
        self.base.send_no_content(req).await
    }

    async fn list_subnets(&self, network_id: Option<&str>) -> ApiResult<Vec<Subnet>> {
        let path = match network_id {
            Some(id) => format!("/v2.0/subnets?network_id={id}"),
            None => "/v2.0/subnets".to_string(),
        };
        let req = self.base.get(&path).await?;
        let resp: NeutronSubnetsResponse = self.base.send_json(req).await?;
        Ok(resp.subnets.into_iter().map(Subnet::from).collect())
    }

    // ── Security Groups ──

    async fn list_security_groups(&self, pagination: &PaginationParams) -> ApiResult<PaginatedResponse<SecurityGroup>> {
        let mut path = "/v2.0/security-groups".to_string();
        append_pagination(&mut path, pagination);
        let req = self.base.get(&path).await?;
        let resp: NeutronSGsResponse = self.base.send_json(req).await?;
        Ok(PaginatedResponse {
            items: resp.security_groups.into_iter().map(SecurityGroup::from).collect(),
            next_marker: resp.security_groups_links.and_then(extract_next_marker),
            has_more: resp.security_groups_links.is_some(),
        })
    }

    async fn get_security_group(&self, sg_id: &str) -> ApiResult<SecurityGroup> {
        let req = self.base.get(&format!("/v2.0/security-groups/{sg_id}")).await?;
        let resp: NeutronSGWrapper = self.base.send_json(req).await?;
        Ok(SecurityGroup::from(resp.security_group))
    }

    async fn create_security_group(&self, params: &SecurityGroupCreateParams) -> ApiResult<SecurityGroup> {
        let body = serde_json::json!({ "security_group": params });
        let req = self.base.post("/v2.0/security-groups").await?.json(&body);
        let resp: NeutronSGWrapper = self.base.send_json(req).await?;
        Ok(SecurityGroup::from(resp.security_group))
    }

    async fn update_security_group(&self, sg_id: &str, params: &SecurityGroupUpdateParams) -> ApiResult<SecurityGroup> {
        let body = serde_json::json!({ "security_group": params });
        let req = self.base.put(&format!("/v2.0/security-groups/{sg_id}")).await?.json(&body);
        let resp: NeutronSGWrapper = self.base.send_json(req).await?;
        Ok(SecurityGroup::from(resp.security_group))
    }

    async fn delete_security_group(&self, sg_id: &str) -> ApiResult<()> {
        let req = self.base.delete(&format!("/v2.0/security-groups/{sg_id}")).await?;
        self.base.send_no_content(req).await
    }

    // ── Security Group Rules ──

    async fn create_security_group_rule(&self, params: &SecurityGroupRuleCreateParams) -> ApiResult<SecurityGroupRule> {
        let body = serde_json::json!({ "security_group_rule": params });
        let req = self.base.post("/v2.0/security-group-rules").await?.json(&body);
        let resp: NeutronSGRuleWrapper = self.base.send_json(req).await?;
        Ok(SecurityGroupRule::from(resp.security_group_rule))
    }

    async fn delete_security_group_rule(&self, rule_id: &str) -> ApiResult<()> {
        let req = self.base.delete(&format!("/v2.0/security-group-rules/{rule_id}")).await?;
        self.base.send_no_content(req).await
    }

    // ── Floating IPs ──

    async fn list_floating_ips(&self, pagination: &PaginationParams) -> ApiResult<PaginatedResponse<FloatingIp>> {
        let mut path = "/v2.0/floatingips".to_string();
        append_pagination(&mut path, pagination);
        let req = self.base.get(&path).await?;
        let resp: NeutronFipsResponse = self.base.send_json(req).await?;
        Ok(PaginatedResponse {
            items: resp.floatingips.into_iter().map(FloatingIp::from).collect(),
            next_marker: resp.floatingips_links.and_then(extract_next_marker),
            has_more: resp.floatingips_links.is_some(),
        })
    }

    async fn create_floating_ip(&self, params: &FloatingIpCreateParams) -> ApiResult<FloatingIp> {
        let body = serde_json::json!({ "floatingip": params });
        let req = self.base.post("/v2.0/floatingips").await?.json(&body);
        let resp: NeutronFipWrapper = self.base.send_json(req).await?;
        Ok(FloatingIp::from(resp.floatingip))
    }

    async fn delete_floating_ip(&self, fip_id: &str) -> ApiResult<()> {
        let req = self.base.delete(&format!("/v2.0/floatingips/{fip_id}")).await?;
        self.base.send_no_content(req).await
    }

    async fn associate_floating_ip(&self, fip_id: &str, port_id: &str) -> ApiResult<FloatingIp> {
        let body = serde_json::json!({ "floatingip": { "port_id": port_id } });
        let req = self.base.put(&format!("/v2.0/floatingips/{fip_id}")).await?.json(&body);
        let resp: NeutronFipWrapper = self.base.send_json(req).await?;
        Ok(FloatingIp::from(resp.floatingip))
    }

    async fn disassociate_floating_ip(&self, fip_id: &str) -> ApiResult<FloatingIp> {
        let body = serde_json::json!({ "floatingip": { "port_id": null } });
        let req = self.base.put(&format!("/v2.0/floatingips/{fip_id}")).await?.json(&body);
        let resp: NeutronFipWrapper = self.base.send_json(req).await?;
        Ok(FloatingIp::from(resp.floatingip))
    }

    // ── Network Agents ──

    async fn list_network_agents(&self) -> ApiResult<Vec<NetworkAgent>> {
        let req = self.base.get("/v2.0/agents").await?;
        let resp: NeutronAgentsResponse = self.base.send_json(req).await?;
        Ok(resp.agents.into_iter().map(NetworkAgent::from).collect())
    }

    async fn enable_network_agent(&self, agent_id: &str) -> ApiResult<NetworkAgent> {
        let body = serde_json::json!({ "agent": { "admin_state_up": true } });
        let req = self.base.put(&format!("/v2.0/agents/{agent_id}")).await?.json(&body);
        let resp: NeutronAgentWrapper = self.base.send_json(req).await?;
        Ok(NetworkAgent::from(resp.agent))
    }

    async fn disable_network_agent(&self, agent_id: &str) -> ApiResult<NetworkAgent> {
        let body = serde_json::json!({ "agent": { "admin_state_up": false } });
        let req = self.base.put(&format!("/v2.0/agents/{agent_id}")).await?.json(&body);
        let resp: NeutronAgentWrapper = self.base.send_json(req).await?;
        Ok(NetworkAgent::from(resp.agent))
    }

    async fn delete_network_agent(&self, agent_id: &str) -> ApiResult<()> {
        let req = self.base.delete(&format!("/v2.0/agents/{agent_id}")).await?;
        self.base.send_no_content(req).await
    }
}
```

---

### 10. CinderHttpAdapter

**Responsibility**: Cinder v3 REST API calls -- volumes, snapshots, QoS, storage pools.

**Dependencies**: `BaseHttpClient` (service_type = "volumev3")

**Data Owned**: `BaseHttpClient`

```rust
// src/adapter/http/cinder.rs

pub struct CinderHttpAdapter {
    base: BaseHttpClient,
}

impl CinderHttpAdapter {
    pub fn new(auth: Arc<dyn AuthProvider>, region: Option<String>) -> Self {
        Self {
            base: BaseHttpClient::new(auth, "volumev3", EndpointInterface::Internal, region),
        }
    }
}

#[async_trait]
impl CinderPort for CinderHttpAdapter {
    // ── Volumes ──

    async fn list_volumes(&self, filter: &VolumeListFilter, pagination: &PaginationParams) -> ApiResult<PaginatedResponse<Volume>> {
        let mut path = "/volumes/detail".to_string();
        let query = build_volume_query(filter, pagination);
        if !query.is_empty() { path.push_str(&format!("?{query}")); }
        let req = self.base.get(&path).await?;
        let resp: CinderVolumesResponse = self.base.send_json(req).await?;
        Ok(PaginatedResponse {
            items: resp.volumes.into_iter().map(Volume::from).collect(),
            next_marker: resp.volumes_links.and_then(extract_next_marker),
            has_more: resp.volumes_links.is_some(),
        })
    }

    async fn get_volume(&self, volume_id: &str) -> ApiResult<Volume> {
        let req = self.base.get(&format!("/volumes/{volume_id}")).await?;
        let resp: CinderVolumeWrapper = self.base.send_json(req).await?;
        Ok(Volume::from(resp.volume))
    }

    async fn create_volume(&self, params: &VolumeCreateParams) -> ApiResult<Volume> {
        let body = CinderVolumeCreateRequest::from(params);
        let req = self.base.post("/volumes").await?.json(&body);
        let resp: CinderVolumeWrapper = self.base.send_json(req).await?;
        Ok(Volume::from(resp.volume))
    }

    async fn delete_volume(&self, volume_id: &str) -> ApiResult<()> {
        let req = self.base.delete(&format!("/volumes/{volume_id}")).await?;
        self.base.send_no_content(req).await
    }

    async fn force_delete_volume(&self, volume_id: &str) -> ApiResult<()> {
        let body = serde_json::json!({ "os-force_delete": {} });
        let req = self.base.post(&format!("/volumes/{volume_id}/action")).await?.json(&body);
        self.base.send_no_content(req).await
    }

    async fn extend_volume(&self, volume_id: &str, new_size_gb: u32) -> ApiResult<()> {
        let body = serde_json::json!({ "os-extend": { "new_size": new_size_gb } });
        let req = self.base.post(&format!("/volumes/{volume_id}/action")).await?.json(&body);
        self.base.send_no_content(req).await
    }

    async fn attach_volume(&self, volume_id: &str, server_id: &str, device: Option<&str>) -> ApiResult<()> {
        // NOTE: Volume attach is actually a Nova API call (POST /servers/{id}/os-volume_attachments).
        // Cinder's own attach is via action. Using the Cinder os-attach action here.
        let body = serde_json::json!({
            "os-attach": {
                "instance_uuid": server_id,
                "mountpoint": device,
            }
        });
        let req = self.base.post(&format!("/volumes/{volume_id}/action")).await?.json(&body);
        self.base.send_no_content(req).await
    }

    async fn detach_volume(&self, volume_id: &str, attachment_id: &str) -> ApiResult<()> {
        let body = serde_json::json!({ "os-detach": { "attachment_id": attachment_id } });
        let req = self.base.post(&format!("/volumes/{volume_id}/action")).await?.json(&body);
        self.base.send_no_content(req).await
    }

    async fn force_set_volume_state(&self, volume_id: &str, state: &str) -> ApiResult<()> {
        let body = serde_json::json!({ "os-reset_status": { "status": state } });
        let req = self.base.post(&format!("/volumes/{volume_id}/action")).await?.json(&body);
        self.base.send_no_content(req).await
    }

    async fn migrate_volume(&self, volume_id: &str, dest_host: &str, force_host_copy: bool) -> ApiResult<()> {
        let body = serde_json::json!({
            "os-migrate_volume": { "host": dest_host, "force_host_copy": force_host_copy }
        });
        let req = self.base.post(&format!("/volumes/{volume_id}/action")).await?.json(&body);
        self.base.send_no_content(req).await
    }

    // ── Snapshots ──

    async fn list_snapshots(&self, pagination: &PaginationParams) -> ApiResult<PaginatedResponse<VolumeSnapshot>> {
        let mut path = "/snapshots/detail".to_string();
        append_pagination(&mut path, pagination);
        let req = self.base.get(&path).await?;
        let resp: CinderSnapshotsResponse = self.base.send_json(req).await?;
        Ok(PaginatedResponse {
            items: resp.snapshots.into_iter().map(VolumeSnapshot::from).collect(),
            next_marker: resp.snapshots_links.and_then(extract_next_marker),
            has_more: resp.snapshots_links.is_some(),
        })
    }

    async fn get_snapshot(&self, snapshot_id: &str) -> ApiResult<VolumeSnapshot> {
        let req = self.base.get(&format!("/snapshots/{snapshot_id}")).await?;
        let resp: CinderSnapshotWrapper = self.base.send_json(req).await?;
        Ok(VolumeSnapshot::from(resp.snapshot))
    }

    async fn create_snapshot(&self, params: &SnapshotCreateParams) -> ApiResult<VolumeSnapshot> {
        let body = serde_json::json!({
            "snapshot": {
                "volume_id": params.volume_id,
                "name": params.name,
                "description": params.description,
                "force": params.force,
            }
        });
        let req = self.base.post("/snapshots").await?.json(&body);
        let resp: CinderSnapshotWrapper = self.base.send_json(req).await?;
        Ok(VolumeSnapshot::from(resp.snapshot))
    }

    async fn delete_snapshot(&self, snapshot_id: &str) -> ApiResult<()> {
        let req = self.base.delete(&format!("/snapshots/{snapshot_id}")).await?;
        self.base.send_no_content(req).await
    }

    // ── QoS ──

    async fn list_qos_specs(&self) -> ApiResult<Vec<QosSpec>> {
        let req = self.base.get("/qos-specs").await?;
        let resp: CinderQosListResponse = self.base.send_json(req).await?;
        Ok(resp.qos_specs.into_iter().map(QosSpec::from).collect())
    }

    async fn get_qos_spec(&self, qos_id: &str) -> ApiResult<QosSpec> {
        let req = self.base.get(&format!("/qos-specs/{qos_id}")).await?;
        let resp: CinderQosWrapper = self.base.send_json(req).await?;
        Ok(QosSpec::from(resp.qos_specs))
    }

    async fn create_qos_spec(&self, params: &QosCreateParams) -> ApiResult<QosSpec> {
        let body = serde_json::json!({
            "qos_specs": {
                "name": params.name,
                "consumer": params.consumer.as_str(),
            }
        });
        // Merge specs into body
        let req = self.base.post("/qos-specs").await?.json(&body);
        let resp: CinderQosWrapper = self.base.send_json(req).await?;
        Ok(QosSpec::from(resp.qos_specs))
    }

    async fn update_qos_spec(&self, qos_id: &str, specs: &HashMap<String, String>) -> ApiResult<QosSpec> {
        let body = serde_json::json!({ "qos_specs": specs });
        let req = self.base.put(&format!("/qos-specs/{qos_id}")).await?.json(&body);
        let resp: CinderQosWrapper = self.base.send_json(req).await?;
        Ok(QosSpec::from(resp.qos_specs))
    }

    async fn delete_qos_spec(&self, qos_id: &str) -> ApiResult<()> {
        let req = self.base.delete(&format!("/qos-specs/{qos_id}")).await?;
        self.base.send_no_content(req).await
    }

    // ── Storage Pools ──

    async fn list_storage_pools(&self, detail: bool) -> ApiResult<Vec<StoragePool>> {
        let path = if detail {
            "/scheduler-stats/get_pools?detail=true"
        } else {
            "/scheduler-stats/get_pools"
        };
        let req = self.base.get(path).await?;
        let resp: CinderPoolsResponse = self.base.send_json(req).await?;
        Ok(resp.pools.into_iter().map(StoragePool::from).collect())
    }

    // ── Quota ──

    async fn get_volume_quota(&self, project_id: &str) -> ApiResult<VolumeQuota> {
        let req = self.base.get(&format!("/os-quota-sets/{project_id}")).await?;
        let resp: CinderQuotaWrapper = self.base.send_json(req).await?;
        Ok(VolumeQuota::from(resp.quota_set))
    }

    async fn update_volume_quota(&self, project_id: &str, params: &VolumeQuotaUpdateParams) -> ApiResult<VolumeQuota> {
        let body = serde_json::json!({ "quota_set": params });
        let req = self.base.put(&format!("/os-quota-sets/{project_id}")).await?.json(&body);
        let resp: CinderQuotaWrapper = self.base.send_json(req).await?;
        Ok(VolumeQuota::from(resp.quota_set))
    }
}
```

---

### 11. KeystoneHttpAdapter

**Responsibility**: Keystone Admin REST API calls -- projects, users, roles, domains. Separate from `KeystoneAuthAdapter` (auth lifecycle).

**Dependencies**: `BaseHttpClient` (service_type = "identity")

**Data Owned**: `BaseHttpClient`

```rust
// src/adapter/http/keystone.rs

pub struct KeystoneHttpAdapter {
    base: BaseHttpClient,
}

impl KeystoneHttpAdapter {
    pub fn new(auth: Arc<dyn AuthProvider>, region: Option<String>) -> Self {
        Self {
            base: BaseHttpClient::new(auth, "identity", EndpointInterface::Internal, region),
        }
    }
}

#[async_trait]
impl KeystonePort for KeystoneHttpAdapter {
    // ── Projects ──

    async fn list_projects(&self, pagination: &PaginationParams) -> ApiResult<PaginatedResponse<Project>> {
        let mut path = "/v3/projects".to_string();
        append_pagination(&mut path, pagination);
        let req = self.base.get(&path).await?;
        let resp: KeystoneProjectsResponse = self.base.send_json(req).await?;
        Ok(PaginatedResponse {
            items: resp.projects.into_iter().map(Project::from).collect(),
            next_marker: resp.links.and_then(|l| l.next).map(|_| String::new()),
            has_more: resp.links.map_or(false, |l| l.next.is_some()),
        })
    }

    async fn get_project(&self, project_id: &str) -> ApiResult<Project> {
        let req = self.base.get(&format!("/v3/projects/{project_id}")).await?;
        let resp: KeystoneProjectWrapper = self.base.send_json(req).await?;
        Ok(Project::from(resp.project))
    }

    async fn create_project(&self, params: &ProjectCreateParams) -> ApiResult<Project> {
        let body = serde_json::json!({ "project": params });
        let req = self.base.post("/v3/projects").await?.json(&body);
        let resp: KeystoneProjectWrapper = self.base.send_json(req).await?;
        Ok(Project::from(resp.project))
    }

    async fn update_project(&self, project_id: &str, params: &ProjectUpdateParams) -> ApiResult<Project> {
        let body = serde_json::json!({ "project": params });
        let req = self.base.patch(&format!("/v3/projects/{project_id}")).await?.json(&body);
        let resp: KeystoneProjectWrapper = self.base.send_json(req).await?;
        Ok(Project::from(resp.project))
    }

    async fn delete_project(&self, project_id: &str) -> ApiResult<()> {
        let req = self.base.delete(&format!("/v3/projects/{project_id}")).await?;
        self.base.send_no_content(req).await
    }

    // ── Users ──

    async fn list_users(&self, pagination: &PaginationParams) -> ApiResult<PaginatedResponse<User>> {
        let mut path = "/v3/users".to_string();
        append_pagination(&mut path, pagination);
        let req = self.base.get(&path).await?;
        let resp: KeystoneUsersResponse = self.base.send_json(req).await?;
        Ok(PaginatedResponse {
            items: resp.users.into_iter().map(User::from).collect(),
            next_marker: resp.links.and_then(|l| l.next).map(|_| String::new()),
            has_more: resp.links.map_or(false, |l| l.next.is_some()),
        })
    }

    async fn get_user(&self, user_id: &str) -> ApiResult<User> {
        let req = self.base.get(&format!("/v3/users/{user_id}")).await?;
        let resp: KeystoneUserWrapper = self.base.send_json(req).await?;
        Ok(User::from(resp.user))
    }

    async fn create_user(&self, params: &UserCreateParams) -> ApiResult<User> {
        let body = serde_json::json!({ "user": params });
        let req = self.base.post("/v3/users").await?.json(&body);
        let resp: KeystoneUserWrapper = self.base.send_json(req).await?;
        Ok(User::from(resp.user))
    }

    async fn update_user(&self, user_id: &str, params: &UserUpdateParams) -> ApiResult<User> {
        let body = serde_json::json!({ "user": params });
        let req = self.base.patch(&format!("/v3/users/{user_id}")).await?.json(&body);
        let resp: KeystoneUserWrapper = self.base.send_json(req).await?;
        Ok(User::from(resp.user))
    }

    async fn delete_user(&self, user_id: &str) -> ApiResult<()> {
        let req = self.base.delete(&format!("/v3/users/{user_id}")).await?;
        self.base.send_no_content(req).await
    }

    // ── Roles ──

    async fn list_roles(&self) -> ApiResult<Vec<Role>> {
        let req = self.base.get("/v3/roles").await?;
        let resp: KeystoneRolesResponse = self.base.send_json(req).await?;
        Ok(resp.roles.into_iter().map(Role::from).collect())
    }

    async fn assign_role(&self, params: &RoleAssignmentParams) -> ApiResult<()> {
        let path = format!(
            "/v3/projects/{}/users/{}/roles/{}",
            params.project_id, params.user_id, params.role_id
        );
        let req = self.base.put(&path).await?;
        self.base.send_no_content(req).await
    }

    async fn revoke_role(&self, params: &RoleAssignmentParams) -> ApiResult<()> {
        let path = format!(
            "/v3/projects/{}/users/{}/roles/{}",
            params.project_id, params.user_id, params.role_id
        );
        let req = self.base.delete(&path).await?;
        self.base.send_no_content(req).await
    }

    async fn list_role_assignments(&self, filter: &RoleAssignmentFilter) -> ApiResult<Vec<RoleAssignment>> {
        let mut path = "/v3/role_assignments".to_string();
        let mut params = vec![];
        if let Some(ref uid) = filter.user_id { params.push(format!("user.id={uid}")); }
        if let Some(ref pid) = filter.project_id { params.push(format!("scope.project.id={pid}")); }
        if let Some(ref rid) = filter.role_id { params.push(format!("role.id={rid}")); }
        if !params.is_empty() { path.push_str(&format!("?{}", params.join("&"))); }
        let req = self.base.get(&path).await?;
        let resp: KeystoneRoleAssignmentsResponse = self.base.send_json(req).await?;
        Ok(resp.role_assignments.into_iter().map(RoleAssignment::from).collect())
    }

    // ── Domains ──

    async fn list_domains(&self) -> ApiResult<Vec<Domain>> {
        let req = self.base.get("/v3/domains").await?;
        let resp: KeystoneDomainsResponse = self.base.send_json(req).await?;
        Ok(resp.domains.into_iter().map(Domain::from).collect())
    }
}
```

---

### 12. GlanceHttpAdapter

**Responsibility**: Glance v2 REST API calls -- images CRUD, data upload, import.

**Dependencies**: `BaseHttpClient` (service_type = "image")

**Data Owned**: `BaseHttpClient`

```rust
// src/adapter/http/glance.rs

pub struct GlanceHttpAdapter {
    base: BaseHttpClient,
}

impl GlanceHttpAdapter {
    pub fn new(auth: Arc<dyn AuthProvider>, region: Option<String>) -> Self {
        Self {
            base: BaseHttpClient::new(auth, "image", EndpointInterface::Internal, region),
        }
    }
}

#[async_trait]
impl GlancePort for GlanceHttpAdapter {
    async fn list_images(&self, filter: &ImageListFilter, pagination: &PaginationParams) -> ApiResult<PaginatedResponse<Image>> {
        let mut path = "/v2/images".to_string();
        let query = build_image_query(filter, pagination);
        if !query.is_empty() { path.push_str(&format!("?{query}")); }
        let req = self.base.get(&path).await?;
        let resp: GlanceImagesResponse = self.base.send_json(req).await?;
        let next_marker = resp.next.as_ref().and_then(|url| {
            url::Url::parse(url).ok()
                .and_then(|u| u.query_pairs().find(|(k, _)| k == "marker").map(|(_, v)| v.to_string()))
        });
        Ok(PaginatedResponse {
            items: resp.images.into_iter().map(Image::from).collect(),
            has_more: resp.next.is_some(),
            next_marker,
        })
    }

    async fn get_image(&self, image_id: &str) -> ApiResult<Image> {
        let req = self.base.get(&format!("/v2/images/{image_id}")).await?;
        let resp: GlanceImageApiModel = self.base.send_json(req).await?;
        Ok(Image::from(resp))
    }

    async fn create_image(&self, params: &ImageCreateParams) -> ApiResult<Image> {
        let body = GlanceImageCreateRequest::from(params);
        let req = self.base.post("/v2/images").await?.json(&body);
        let resp: GlanceImageApiModel = self.base.send_json(req).await?;
        Ok(Image::from(resp))
    }

    async fn upload_image_data(
        &self,
        image_id: &str,
        data: Box<dyn tokio::io::AsyncRead + Send + Unpin>,
        content_type: Option<&str>,
    ) -> ApiResult<()> {
        let endpoint = self.base.resolve_endpoint().await?;
        let token = self.base.auth.get_token().await?;
        let url = format!("{}/v2/images/{image_id}/file", endpoint.trim_end_matches('/'));

        let body = reqwest::Body::wrap_stream(tokio_util::io::ReaderStream::new(data));
        let resp = self.base.client
            .put(&url)
            .header("X-Auth-Token", &token)
            .header("Content-Type", content_type.unwrap_or("application/octet-stream"))
            .body(body)
            .send()
            .await
            .map_err(ApiError::Network)?;

        BaseHttpClient::check_status(resp).await?;
        Ok(())
    }

    async fn import_image(&self, image_id: &str, method: ImageImportMethod) -> ApiResult<()> {
        let body = match method {
            ImageImportMethod::WebDownload { ref uri } => serde_json::json!({
                "method": { "name": "web-download", "uri": uri }
            }),
        };
        let req = self.base.post(&format!("/v2/images/{image_id}/import")).await?.json(&body);
        self.base.send_no_content(req).await
    }

    async fn update_image(&self, image_id: &str, params: &ImageUpdateParams) -> ApiResult<Image> {
        // Glance uses JSON Patch (RFC 6902) for updates
        let mut ops: Vec<serde_json::Value> = vec![];
        if let Some(ref name) = params.name {
            ops.push(serde_json::json!({ "op": "replace", "path": "/name", "value": name }));
        }
        if let Some(ref vis) = params.visibility {
            ops.push(serde_json::json!({ "op": "replace", "path": "/visibility", "value": vis.as_str() }));
        }
        if let Some(min_disk) = params.min_disk_gb {
            ops.push(serde_json::json!({ "op": "replace", "path": "/min_disk", "value": min_disk }));
        }
        if let Some(min_ram) = params.min_ram_mb {
            ops.push(serde_json::json!({ "op": "replace", "path": "/min_ram", "value": min_ram }));
        }
        if let Some(ref props) = params.properties {
            for (k, v) in props {
                ops.push(serde_json::json!({ "op": "replace", "path": format!("/{k}"), "value": v }));
            }
        }

        let endpoint = self.base.resolve_endpoint().await?;
        let token = self.base.auth.get_token().await?;
        let url = format!("{}/v2/images/{image_id}", endpoint.trim_end_matches('/'));

        let resp = self.base.client
            .patch(&url)
            .header("X-Auth-Token", &token)
            .header("Content-Type", "application/openstack-images-v2.1-json-patch")
            .json(&ops)
            .send()
            .await
            .map_err(ApiError::Network)?;

        let resp = BaseHttpClient::check_status(resp).await?;
        let model: GlanceImageApiModel = resp.json().await.map_err(ApiError::Network)?;
        Ok(Image::from(model))
    }

    async fn delete_image(&self, image_id: &str) -> ApiResult<()> {
        let req = self.base.delete(&format!("/v2/images/{image_id}")).await?;
        self.base.send_no_content(req).await
    }

    async fn deactivate_image(&self, image_id: &str) -> ApiResult<()> {
        let req = self.base.post(&format!("/v2/images/{image_id}/actions/deactivate")).await?;
        self.base.send_no_content(req).await
    }

    async fn reactivate_image(&self, image_id: &str) -> ApiResult<()> {
        let req = self.base.post(&format!("/v2/images/{image_id}/actions/reactivate")).await?;
        self.base.send_no_content(req).await
    }
}
```

---

### 13. AdapterRegistry

**Responsibility**: Config-based adapter instantiation and dependency injection. Reads clouds.yaml/app config, creates the correct adapter instances, wires `AuthProvider` into all HTTP adapters. Phase 2: swap direct-call adapters for Service Layer proxy adapters.

**Dependencies**: `Config`, `AuthProvider`, all HTTP adapters

**Data Owned**: `Arc<dyn AuthProvider>`, `Arc<dyn NovaPort>`, `Arc<dyn NeutronPort>`, `Arc<dyn CinderPort>`, `Arc<dyn KeystonePort>`, `Arc<dyn GlancePort>`, token refresh subscriber handles

```rust
// src/adapter/registry.rs

pub struct AdapterRegistry {
    auth: Arc<dyn AuthProvider>,
    nova: Arc<dyn NovaPort>,
    neutron: Arc<dyn NeutronPort>,
    cinder: Arc<dyn CinderPort>,
    keystone: Arc<dyn KeystonePort>,
    glance: Arc<dyn GlancePort>,
    /// Token refresh subscription handles for endpoint cache invalidation
    _refresh_handles: Vec<JoinHandle<()>>,
}

impl AdapterRegistry {
    /// Build the full adapter registry from application config.
    /// Performs initial authentication before returning.
    pub async fn build(config: &AppConfig, cloud_name: &str) -> ApiResult<Self> {
        let cloud = config.get_cloud(cloud_name)?;
        let credential = AuthCredential::from_cloud_config(&cloud);
        let region = cloud.region_name.clone();

        // ── Step 1: Create and authenticate the AuthProvider ──
        let auth: Arc<dyn AuthProvider> = match cloud.auth_type.as_deref() {
            Some("v3applicationcredential") | None => {
                let adapter = KeystoneAuthAdapter::new(credential.clone());
                adapter.authenticate(&credential).await?;
                Arc::new(adapter)
            }
            // Phase 2:
            // Some("hmac") => Arc::new(HmacAuthAdapter::new(...)),
            // Some("apikey") => Arc::new(ApiKeyAuthAdapter::new(...)),
            _ => return Err(ApiError::AuthFailed(
                format!("Unsupported auth_type: {:?}", cloud.auth_type)
            )),
        };

        // ── Step 2: Create service adapters ──
        let nova: Arc<dyn NovaPort> = Arc::new(
            NovaHttpAdapter::new(auth.clone(), region.clone())
        );
        let neutron: Arc<dyn NeutronPort> = Arc::new(
            NeutronHttpAdapter::new(auth.clone(), region.clone())
        );
        let cinder: Arc<dyn CinderPort> = Arc::new(
            CinderHttpAdapter::new(auth.clone(), region.clone())
        );
        let keystone: Arc<dyn KeystonePort> = Arc::new(
            KeystoneHttpAdapter::new(auth.clone(), region.clone())
        );
        let glance: Arc<dyn GlancePort> = Arc::new(
            GlanceHttpAdapter::new(auth.clone(), region.clone())
        );

        // ── Step 3: Subscribe to token refresh for endpoint cache invalidation ──
        let refresh_handles = Self::spawn_refresh_listeners(
            &auth,
            [&nova, &neutron, &cinder, &keystone, &glance],
        );

        Ok(Self {
            auth,
            nova,
            neutron,
            cinder,
            keystone,
            glance,
            _refresh_handles: refresh_handles,
        })
    }

    // ── Accessors ──

    pub fn auth(&self) -> &Arc<dyn AuthProvider> { &self.auth }
    pub fn nova(&self) -> &Arc<dyn NovaPort> { &self.nova }
    pub fn neutron(&self) -> &Arc<dyn NeutronPort> { &self.neutron }
    pub fn cinder(&self) -> &Arc<dyn CinderPort> { &self.cinder }
    pub fn keystone(&self) -> &Arc<dyn KeystonePort> { &self.keystone }
    pub fn glance(&self) -> &Arc<dyn GlancePort> { &self.glance }

    /// Switch to a different cloud at runtime.
    /// Re-authenticates and rebuilds all adapters.
    pub async fn switch_cloud(
        &mut self,
        config: &AppConfig,
        cloud_name: &str,
    ) -> ApiResult<()> {
        let new_registry = Self::build(config, cloud_name).await?;
        *self = new_registry;
        Ok(())
    }

    /// Phase 2: Replace a specific port with a Service Layer proxy adapter.
    /// Example: swap NovaHttpAdapter for NovaServiceLayerAdapter
    /// that routes through an Admin API Gateway.
    pub fn replace_nova(&mut self, adapter: Arc<dyn NovaPort>) {
        self.nova = adapter;
    }

    pub fn replace_neutron(&mut self, adapter: Arc<dyn NeutronPort>) {
        self.neutron = adapter;
    }

    pub fn replace_cinder(&mut self, adapter: Arc<dyn CinderPort>) {
        self.cinder = adapter;
    }

    fn spawn_refresh_listeners(
        auth: &Arc<dyn AuthProvider>,
        // In practice: iterate over adapters that have `invalidate_endpoint()`
        _adapters: [&Arc<dyn _>; 5],  // pseudo — actual impl uses concrete types
    ) -> Vec<JoinHandle<()>> {
        // Each listener subscribes to auth.subscribe_token_refresh()
        // and calls adapter.base.invalidate_endpoint() on each refresh event.
        // This ensures cached endpoint URLs are re-resolved with the new catalog.
        vec![] // placeholder — actual implementation spawns tasks
    }
}
```

**Interactions -- Startup & DI Flow**:

```
  main()
    │
    ▼
  Config::load("~/.config/openstack/clouds.yaml")
    │
    ▼
  AdapterRegistry::build(&config, "my-cloud")
    │
    ├─ 1. Create KeystoneAuthAdapter
    ├─ 2. authenticate() ──► POST /v3/auth/tokens
    │      └─► Token { catalog, roles, expires_at }
    │
    ├─ 3. Create NovaHttpAdapter(auth, region)
    ├─ 4. Create NeutronHttpAdapter(auth, region)
    ├─ 5. Create CinderHttpAdapter(auth, region)
    ├─ 6. Create KeystoneHttpAdapter(auth, region)
    ├─ 7. Create GlanceHttpAdapter(auth, region)
    │
    ├─ 8. Spawn token refresh broadcast listeners
    │
    └─► Return AdapterRegistry
           │
           ▼
  App::new(registry)
    │
    ├─ ServerModule::new(registry.nova().clone())
    ├─ NetworkModule::new(registry.neutron().clone())
    ├─ VolumeModule::new(registry.cinder().clone())
    ├─ ProjectModule::new(registry.keystone().clone())
    ├─ ImageModule::new(registry.glance().clone())
    └─ ...
```

**Phase 2 Adapter Swap (Service Layer Proxy)**:

```
  Phase 1 (direct):
    TUI ──► NovaHttpAdapter ──► Nova REST API

  Phase 2 (via gateway):
    TUI ──► NovaServiceLayerAdapter ──► Admin API GW ──► Nova REST API
                 │
                 └─ Same NovaPort trait, different impl
                    AdapterRegistry::replace_nova(Arc::new(NovaServiceLayerAdapter))
```

---

## File Structure

```
src/
├── domain/
│   ├── error.rs           # ApiError, ApiResult
│   ├── pagination.rs      # PaginationParams, PaginatedResponse
│   ├── auth.rs            # Token, AuthCredential, AuthMethod, CatalogEntry, Endpoint
│   ├── filter.rs          # ServerListFilter, VolumeListFilter, ImageListFilter
│   ├── server.rs          # Server, ServerCreateParams, LiveMigrateParams, ...
│   ├── flavor.rs          # Flavor, FlavorCreateParams
│   ├── aggregate.rs       # Aggregate, AggregateCreateParams, ...
│   ├── compute_service.rs # ComputeService
│   ├── hypervisor.rs      # Hypervisor
│   ├── usage.rs           # ProjectUsage, ComputeQuota, ...
│   ├── network.rs         # Network, Subnet, NetworkCreateParams, ...
│   ├── security_group.rs  # SecurityGroup, SecurityGroupRule, ...
│   ├── floating_ip.rs     # FloatingIp, FloatingIpCreateParams
│   ├── network_agent.rs   # NetworkAgent
│   ├── volume.rs          # Volume, VolumeCreateParams, VolumeSource, ...
│   ├── snapshot.rs        # VolumeSnapshot, SnapshotCreateParams
│   ├── qos.rs             # QosSpec, QosCreateParams, QosConsumer
│   ├── storage_pool.rs    # StoragePool
│   ├── project.rs         # Project, ProjectCreateParams, ...
│   ├── user.rs            # User, UserCreateParams, ...
│   ├── role.rs            # Role, RoleAssignment, RoleAssignmentParams, ...
│   ├── image.rs           # Image, ImageCreateParams, DiskFormat, ...
│   └── mod.rs
├── port/
│   ├── auth.rs            # AuthProvider trait
│   ├── nova.rs            # NovaPort trait
│   ├── neutron.rs         # NeutronPort trait
│   ├── cinder.rs          # CinderPort trait
│   ├── keystone.rs        # KeystonePort trait
│   ├── glance.rs          # GlancePort trait
│   └── mod.rs
├── adapter/
│   ├── http/
│   │   ├── base.rs        # BaseHttpClient
│   │   ├── nova.rs        # NovaHttpAdapter
│   │   ├── neutron.rs     # NeutronHttpAdapter
│   │   ├── cinder.rs      # CinderHttpAdapter
│   │   ├── keystone.rs    # KeystoneHttpAdapter
│   │   ├── glance.rs      # GlanceHttpAdapter
│   │   └── mod.rs
│   ├── auth/
│   │   ├── keystone.rs    # KeystoneAuthAdapter
│   │   └── mod.rs         # Phase 2: hmac.rs, apikey.rs
│   ├── registry.rs        # AdapterRegistry
│   └── mod.rs
└── ...
```

---

## Cross-Cutting Concerns

### Token Refresh Broadcast

```
  KeystoneAuthAdapter (refresh loop)
       │
       │  broadcast::send(Token)
       │
       ├──► Adapter A (NovaHttpAdapter)
       │       └─ invalidate_endpoint() → next request re-resolves from new catalog
       ├──► Adapter B (NeutronHttpAdapter)
       │       └─ invalidate_endpoint()
       ├──► Adapter C (CinderHttpAdapter)
       │       └─ invalidate_endpoint()
       ├──► RbacGuard
       │       └─ update roles from new Token
       └──► App
               └─ update UI header (token expiry, project info)
```

### Error Handling Flow (401 Retry)

```
  NovaHttpAdapter::list_servers()
       │
       ├─ BaseHttpClient::send_json()
       │       │
       │       └─ HTTP 401 ──► ApiError::TokenExpired
       │
       └─ Caller (ActionDispatcher) catches TokenExpired:
              │
              ├─ AuthProvider::refresh_token()
              ├─ Retry original request (1 attempt)
              └─ If still 401 → propagate error → Toast "Session expired"
```

### Pagination Helper (shared by all adapters)

```rust
// src/adapter/http/base.rs

fn append_pagination(path: &mut String, params: &PaginationParams) {
    let mut query_parts = vec![];
    if let Some(ref marker) = params.marker {
        query_parts.push(format!("marker={marker}"));
    }
    if let Some(limit) = params.limit {
        query_parts.push(format!("limit={limit}"));
    }
    if let Some(ref key) = params.sort_key {
        query_parts.push(format!("sort_key={key}"));
    }
    if let Some(ref dir) = params.sort_dir {
        query_parts.push(format!("sort_dir={}", dir.as_str()));
    }
    if !query_parts.is_empty() {
        let sep = if path.contains('?') { "&" } else { "?" };
        path.push_str(&format!("{sep}{}", query_parts.join("&")));
    }
}

fn extract_next_marker(links: Vec<Link>) -> Option<String> {
    links.iter()
        .find(|l| l.rel == "next")
        .and_then(|l| {
            url::Url::parse(&l.href).ok()
                .and_then(|u| u.query_pairs()
                    .find(|(k, _)| k == "marker")
                    .map(|(_, v)| v.to_string()))
        })
}
```
