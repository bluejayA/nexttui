# Functional Design: Unit 1 — foundation

**Timestamp**: 2026-03-23T14:30:00+09:00
**Unit**: foundation
**Stories**: TR-05, TR-06
**Components**: Config + Domain Models + Error Types + Project Structure

---

## Step 1: Domain Entities

### 1.1 Config (clouds.yaml + app config)

```rust
pub struct Config {
    clouds: HashMap<String, CloudConfig>,
    active_cloud: String,
    app: AppConfig,
}
```

**Invariants**:
- `active_cloud`는 반드시 `clouds` 맵에 존재하는 키여야 한다
- `clouds`는 최소 1개 엔트리 필요 (비어 있으면 로드 실패)
- `active_cloud` 변경 시 기존 키 존재 여부 검증 필수

### 1.2 CloudConfig

```rust
pub struct CloudConfig {
    pub name: String,
    pub auth: AuthConfig,
    pub region_name: Option<String>,
    pub regions: Option<Vec<String>>,
    pub interface: String,              // "public" | "internal" | "admin"
    pub identity_api_version: u8,       // default: 3
    pub verify: bool,                   // SSL verification (default: true)
    pub cacert: Option<PathBuf>,
}
```

**Invariants**:
- `auth.auth_url`은 비어 있으면 안 된다
- `interface`는 "public" | "internal" | "admin" 중 하나
- `identity_api_version`은 3 (다른 값은 미지원 경고)
- `verify: false`일 때만 TLS 검증 비활성화 허용

### 1.3 AuthConfig

```rust
pub struct AuthConfig {
    pub auth_url: String,
    pub auth_type: AuthType,
    // password auth
    pub username: Option<String>,
    pub password: Option<String>,       // 메모리 전용, 로그 출력 금지 (TR-06)
    pub project_name: Option<String>,
    pub project_domain_name: Option<String>,
    pub user_domain_name: Option<String>,
    // application_credential auth
    pub application_credential_id: Option<String>,
    pub application_credential_secret: Option<String>,  // 메모리 전용 (TR-06)
}

pub enum AuthType {
    Password,
    ApplicationCredential,
}
```

**Invariants**:
- `AuthType::Password` → `username`, `password` 필수
- `AuthType::ApplicationCredential` → `application_credential_id`, `application_credential_secret` 필수
- `password`, `application_credential_secret`은 `Debug`, `Display` trait에서 마스킹 (`****`)
- `Serialize` 구현 시 시크릿 필드 제외

### 1.4 AppConfig

```rust
pub struct AppConfig {
    pub tick_rate_ms: u64,                  // default: 200
    pub cache_ttl: CacheTtlConfig,
    pub audit_log_path: PathBuf,            // default: ~/.config/nexttui/audit.log
    pub command_history_path: PathBuf,      // default: ~/.config/nexttui/history
    pub command_history_max: usize,         // default: 50
    pub default_cloud: Option<String>,
}

pub struct CacheTtlConfig {
    pub servers_secs: u64,                  // default: 120
    pub networks_secs: u64,                 // default: 300
    pub flavors_secs: u64,                  // default: 600
    pub images_secs: u64,                   // default: 600
    pub security_groups_secs: u64,          // default: 300
    pub volumes_secs: u64,                  // default: 120
    pub projects_secs: u64,                 // default: 300
}
```

**Invariants**:
- `tick_rate_ms` > 0 (0이면 CPU 100%)
- TTL 값은 양의 정수
- `AppConfig`가 없으면 전체 기본값으로 생성 (config.toml 없어도 정상 동작)

### 1.5 ResourceType (캐시/TTL 키)

```rust
#[derive(Debug, Clone, Copy, Hash, Eq, PartialEq)]
pub enum ResourceType {
    Servers,
    Networks,
    SecurityGroups,
    Volumes,
    Snapshots,
    Flavors,
    Images,
    Projects,
    Users,
    Aggregates,
    ComputeServices,
    Hypervisors,
    FloatingIps,
    Agents,
}
```

### 1.6 Route (네비게이션 상태)

```rust
#[derive(Debug, Clone, Copy, Hash, Eq, PartialEq)]
pub enum Route {
    Servers, ServerDetail, ServerCreate,
    Flavors,
    Migrations,
    Aggregates,
    ComputeServices,
    Hypervisors,
    Networks, NetworkDetail,
    SecurityGroups, SecurityGroupDetail,
    FloatingIps,
    Agents,
    Volumes, VolumeDetail, VolumeCreate,
    Snapshots,
    Images, ImageDetail,
    Projects,
    Users,
    Usage,
}
```

### 1.7 OpenStack Domain Models

API 응답을 역직렬화할 도메인 구조체. 각 모델은 `serde::Deserialize`를 구현하며, API JSON의 snake_case 필드에 매핑.

```rust
// --- Nova ---
pub struct Server {
    pub id: String,
    pub name: String,
    pub status: String,          // ACTIVE, SHUTOFF, ERROR, BUILD, etc.
    pub addresses: HashMap<String, Vec<Address>>,
    pub flavor: FlavorRef,
    pub image: Option<ImageRef>,
    pub key_name: Option<String>,
    pub availability_zone: Option<String>,
    pub created: String,
    pub updated: Option<String>,
    pub tenant_id: String,
    pub host_id: String,
    pub host: Option<String>,    // admin only — compute host
}

pub struct Address {
    pub addr: String,
    pub version: u8,             // 4 or 6
    pub mac_addr: Option<String>,
    #[serde(rename = "OS-EXT-IPS:type")]
    pub ip_type: Option<String>, // "fixed" | "floating"
}

pub struct FlavorRef {
    pub id: String,
    pub original_name: Option<String>,
    pub vcpus: Option<u32>,
    pub ram: Option<u32>,        // MB
    pub disk: Option<u32>,       // GB
}

pub struct Flavor {
    pub id: String,
    pub name: String,
    pub vcpus: u32,
    pub ram: u32,
    pub disk: u32,
    #[serde(rename = "os-flavor-access:is_public")]
    pub is_public: bool,
}

// --- Neutron ---
pub struct Network {
    pub id: String,
    pub name: String,
    pub status: String,
    pub admin_state_up: bool,
    #[serde(rename = "router:external")]
    pub external: bool,
    pub shared: bool,
    pub mtu: Option<u32>,
    pub subnets: Vec<String>,
    #[serde(rename = "provider:network_type")]
    pub provider_network_type: Option<String>,
    #[serde(rename = "provider:physical_network")]
    pub provider_physical_network: Option<String>,
    #[serde(rename = "provider:segmentation_id")]
    pub provider_segmentation_id: Option<u32>,
}

pub struct SecurityGroup {
    pub id: String,
    pub name: String,
    pub description: Option<String>,
    pub security_group_rules: Vec<SecurityGroupRule>,
}

pub struct SecurityGroupRule {
    pub id: String,
    pub direction: String,       // "ingress" | "egress"
    pub protocol: Option<String>,
    pub port_range_min: Option<u16>,
    pub port_range_max: Option<u16>,
    pub remote_ip_prefix: Option<String>,
    pub remote_group_id: Option<String>,
    pub ethertype: String,       // "IPv4" | "IPv6"
}

pub struct FloatingIp {
    pub id: String,
    pub floating_ip_address: String,
    pub status: String,
    pub port_id: Option<String>,
    pub floating_network_id: String,
    pub fixed_ip_address: Option<String>,
    pub router_id: Option<String>,
}

// --- Cinder ---
pub struct Volume {
    pub id: String,
    pub name: Option<String>,
    pub status: String,
    pub size: u32,               // GB
    pub volume_type: Option<String>,
    pub encrypted: bool,
    pub bootable: String,        // "true" | "false" (API returns string)
    pub attachments: Vec<VolumeAttachment>,
    pub availability_zone: Option<String>,
    pub created_at: String,
}

pub struct VolumeAttachment {
    pub server_id: String,
    pub device: String,
    pub id: String,
}

pub struct VolumeSnapshot {
    pub id: String,
    pub name: Option<String>,
    pub status: String,
    pub size: u32,
    pub volume_id: String,
    pub created_at: String,
}

// --- Glance ---
pub struct Image {
    pub id: String,
    pub name: String,
    pub status: String,
    pub disk_format: Option<String>,
    pub container_format: Option<String>,
    pub size: Option<u64>,
    pub visibility: String,      // "public" | "private" | "shared" | "community"
    pub min_disk: u32,
    pub min_ram: u32,
    pub checksum: Option<String>,
    pub created_at: String,
}

// --- Keystone ---
pub struct Project {
    pub id: String,
    pub name: String,
    pub description: Option<String>,
    pub enabled: bool,
    pub domain_id: String,
}

pub struct User {
    pub id: String,
    pub name: String,
    pub email: Option<String>,
    pub enabled: bool,
    pub default_project_id: Option<String>,
    pub domain_id: String,
}

pub struct Role {
    pub id: String,
    pub name: String,
}

pub struct RoleAssignment {
    pub role: Role,
    pub user: Option<UserRef>,
    pub scope: Option<Scope>,
}

pub struct UserRef { pub id: String }
pub struct Scope { pub project: Option<ProjectRef> }
pub struct ProjectRef { pub id: String }

// --- Compute Admin ---
pub struct Aggregate {
    pub id: u32,
    pub name: String,
    pub availability_zone: Option<String>,
    pub hosts: Vec<String>,
    pub metadata: HashMap<String, String>,
}

pub struct ComputeService {
    pub id: String,
    pub binary: String,
    pub host: String,
    pub status: String,          // "enabled" | "disabled"
    pub state: String,           // "up" | "down"
    pub updated_at: Option<String>,
    pub disabled_reason: Option<String>,
}

pub struct Hypervisor {
    pub id: u32,
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

// --- Neutron Admin ---
pub struct NetworkAgent {
    pub id: String,
    pub agent_type: String,
    pub host: String,
    pub admin_state_up: bool,
    pub alive: bool,
    pub binary: String,
}
```

**공통 불변 조건**:
- 모든 모델의 `id`는 비어 있지 않다
- `serde(rename)` 어노테이션으로 OpenStack API의 비표준 필드명 처리
- `Option<T>`은 API 응답에서 해당 필드가 없거나 null인 경우

### 1.8 Entity Relationships

```
Server 1──N Address
Server N──1 Flavor (via FlavorRef)
Server N──0..1 Image (via ImageRef)
Server 1──N VolumeAttachment

Network 1──N Subnet (ID refs)
SecurityGroup 1──N SecurityGroupRule

Volume 1──N VolumeAttachment
Volume 1──N VolumeSnapshot

Project 1──N User (via RoleAssignment)
Project 1──N RoleAssignment
Aggregate 1──N Host (String refs)
```

---

## Step 2: Business Rules

### BR-01: clouds.yaml 탐색 순서
- **조건**: Config::load() 호출 시
- **동작**: 아래 순서로 첫 번째 존재하는 파일 사용
  1. `$OS_CLIENT_CONFIG_FILE` 환경변수
  2. `./clouds.yaml` (현재 디렉터리)
  3. `~/.config/openstack/clouds.yaml`
  4. `/etc/openstack/clouds.yaml`
- **우선순위**: 환경변수 > 로컬 > 사용자 > 시스템

### BR-02: active_cloud 결정 순서
- **조건**: clouds.yaml 로드 완료 후
- **동작**: 아래 순서로 첫 번째 유효한 값 사용
  1. CLI arg `--cloud <name>`
  2. `config.toml`의 `default_cloud`
  3. `$OS_CLOUD` 환경변수
  4. `clouds.yaml`의 첫 번째 엔트리
- **예외**: 지정된 이름이 clouds 맵에 없으면 에러

### BR-03: AuthType 자동 감지
- **조건**: clouds.yaml에 `auth_type` 필드가 명시되지 않은 경우
- **동작**:
  - `application_credential_id` 존재 → `ApplicationCredential`
  - 그 외 → `Password`
- **예외**: 둘 다 없으면 필수 필드 누락 에러

### BR-04: 시크릿 마스킹 (TR-06)
- **조건**: `password`, `application_credential_secret` 필드
- **동작**:
  - `Debug` impl: `"****"` 출력
  - `Display` impl: 미구현 (필요 없음)
  - `Serialize` impl: 시크릿 필드 `#[serde(skip_serializing)]`
  - 로그 출력 시 절대 포함 금지
- **우선순위**: 무조건 적용 (보안 NFR)

### BR-05: AppConfig 기본값 Fallback
- **조건**: `~/.config/nexttui/config.toml`이 없거나 파싱 실패 시
- **동작**: 전체 기본값으로 AppConfig 생성 (에러 아님)
- **기본값**: tick_rate 200ms, servers TTL 120s, networks TTL 300s 등

### BR-06: SSL/TLS 설정
- **조건**: `CloudConfig.verify` 값
- **동작**:
  - `verify: true` (기본) → TLS 인증서 검증 활성
  - `verify: false` → 검증 비활성 (insecure)
  - `cacert` 지정 → 커스텀 CA 인증서 사용
- **예외**: `cacert` 경로가 존재하지 않으면 에러

### BR-07: clouds.yaml 형식 호환
- **조건**: OpenStack 표준 `clouds.yaml` 포맷
- **동작**: 최상위 `clouds:` 키 아래의 맵 파싱
  ```yaml
  clouds:
    mycloud:
      auth:
        auth_url: https://keystone.example.com/v3
        ...
  ```
- **예외**: `clouds:` 키가 없으면 에러 ("Invalid clouds.yaml: missing 'clouds' key")

### BR-08: ResourceType → TTL 매핑
- **조건**: Cache에서 TTL 조회 시
- **동작**: `Config.cache_ttl(resource_type)` → `Duration`
  - 매핑 테이블은 CacheTtlConfig 기반
  - 매핑에 없는 ResourceType → 기본 120초

---

## Step 3: Data Flow

### 3.1 Config 로딩 플로우

```
main()
  |
  v
Config::load()
  |
  +---> env::var("OS_CLIENT_CONFIG_FILE")
  |     or scan 4 paths → find first existing clouds.yaml
  |     |
  |     v
  |     fs::read_to_string(path)
  |     |
  |     v
  |     serde_yaml::from_str::<CloudsYamlRoot>()
  |     |
  |     v
  |     CloudsYamlRoot.clouds → HashMap<String, CloudConfig>
  |     (auth_type 자동 감지 BR-03 적용)
  |
  +---> locate ~/.config/nexttui/config.toml
  |     |
  |     v
  |     존재하면: toml::from_str::<AppConfig>()
  |     없으면: AppConfig::default() (BR-05)
  |
  +---> active_cloud 결정 (BR-02)
  |     |
  |     v
  |     clouds 맵에 존재 여부 검증
  |
  v
Ok(Config { clouds, active_cloud, app })
```

### 3.2 에러 전파 경로

```
Config::load() → Result<Config>
  |
  +--- IoError: 파일 읽기 실패 → AppError::ConfigIo { path, source }
  +--- YamlError: YAML 파싱 실패 → AppError::ConfigParse { path, source }
  +--- TomlError: TOML 파싱 실패 → (무시, 기본값 사용 + 경고 로그)
  +--- ValidationError: 필수 필드 누락 → AppError::ConfigValidation { message }
  +--- CloudNotFound: active_cloud 없음 → AppError::CloudNotFound { name }
```

### 3.3 외부 시스템 연동 포인트

- **파일 시스템**: clouds.yaml, config.toml 읽기 (startup only)
- **환경 변수**: `OS_CLIENT_CONFIG_FILE`, `OS_CLOUD` 읽기
- **CLI 인자**: `--cloud <name>` 파싱 (clap 또는 수동)

---

## Step 4: Error/Exception Scenarios

| 시나리오 | 원인 | 처리 방식 | 사용자 메시지 |
|----------|------|-----------|---------------|
| clouds.yaml 미발견 | 4개 경로 모두에 파일 없음 | 앱 종료 (Fatal) | `Error: clouds.yaml not found. Searched: [paths]. See: https://docs.openstack.org/os-client-config/` |
| clouds.yaml 파싱 실패 | YAML 문법 오류 | 앱 종료 (Fatal) | `Error: Failed to parse {path}: {yaml_error}` |
| clouds.yaml에 clouds 키 없음 | 잘못된 포맷 | 앱 종료 (Fatal) | `Error: Invalid clouds.yaml: missing 'clouds' key` |
| cloud 엔트리에 auth 블록 없음 | 필수 필드 누락 | 해당 cloud 스킵 + 경고 | `Warning: Cloud '{name}' skipped: missing auth configuration` |
| auth_url 비어 있음 | 필수 필드 누락 | 해당 cloud 스킵 + 경고 | `Warning: Cloud '{name}' skipped: auth_url is required` |
| password auth에 username/password 없음 | 필수 필드 누락 | 해당 cloud 스킵 + 경고 | `Warning: Cloud '{name}' skipped: username and password required for password auth` |
| app_credential auth에 id/secret 없음 | 필수 필드 누락 | 해당 cloud 스킵 + 경고 | `Warning: Cloud '{name}' skipped: credential id and secret required` |
| 지정된 active_cloud가 없음 | 맵에 해당 키 없음 | 앱 종료 (Fatal) | `Error: Cloud '{name}' not found. Available: [cloud_names]` |
| 모든 cloud가 스킵됨 | 유효한 cloud 0개 | 앱 종료 (Fatal) | `Error: No valid cloud configurations found in clouds.yaml` |
| config.toml 파싱 실패 | TOML 문법 오류 | 기본값 사용 + 경고 | `Warning: Failed to parse config.toml, using defaults: {error}` |
| cacert 경로 미존재 | 파일 없음 | 해당 cloud 스킵 + 경고 | `Warning: Cloud '{name}': cacert path not found: {path}` |
| switch_cloud 시 이름 미발견 | 런타임 전환 실패 | 에러 반환 (앱 종료 아님) | Toast: `Cloud '{name}' not found` |

---

## Step 5: Error Type Hierarchy

```rust
use thiserror::Error;

#[derive(Error, Debug)]
pub enum AppError {
    // --- Config ---
    #[error("clouds.yaml not found. Searched: {searched_paths:?}")]
    CloudsYamlNotFound { searched_paths: Vec<PathBuf> },

    #[error("Failed to parse {path}: {source}")]
    ConfigParse {
        path: PathBuf,
        #[source]
        source: Box<dyn std::error::Error + Send + Sync>,
    },

    #[error("Config validation failed: {message}")]
    ConfigValidation { message: String },

    #[error("Cloud '{name}' not found. Available: {available:?}")]
    CloudNotFound { name: String, available: Vec<String> },

    // --- API ---
    #[error("API request failed: {message}")]
    Api { message: String, status: Option<u16> },

    #[error("Authentication failed: {message}")]
    Auth { message: String },

    // --- IO ---
    #[error("IO error: {source}")]
    Io {
        #[from]
        source: std::io::Error,
    },

    // --- General ---
    #[error("{0}")]
    Other(String),
}

pub type Result<T> = std::result::Result<T, AppError>;
```

---

## code-generation Connection

### TDD RED 테스트 케이스 도출

**Config 로딩 (BR-01~BR-07)**:
1. `test_load_clouds_yaml_from_standard_path` — 정상 파싱
2. `test_load_clouds_yaml_env_override` — 환경변수 우선
3. `test_clouds_yaml_not_found` — 4개 경로 모두 없으면 에러
4. `test_invalid_yaml_syntax` — YAML 파싱 에러
5. `test_missing_clouds_key` — clouds 키 없음 에러
6. `test_auth_type_auto_detect_password` — password 자동 감지
7. `test_auth_type_auto_detect_app_credential` — app_credential 자동 감지
8. `test_active_cloud_cli_arg` — CLI 인자 우선
9. `test_active_cloud_fallback_to_first` — 기본값 첫 엔트리
10. `test_active_cloud_not_found` — 없는 이름 에러

**Validation (불변 조건)**:
11. `test_password_auth_missing_username` — username 누락 시 스킵
12. `test_app_credential_missing_secret` — secret 누락 시 스킵
13. `test_all_clouds_invalid_fatal` — 유효한 cloud 0개 → 에러
14. `test_partial_invalid_clouds_skip` — 일부 무효 cloud 스킵, 나머지 정상

**시크릿 마스킹 (BR-04)**:
15. `test_password_debug_masked` — Debug 출력에 `****`
16. `test_secret_not_serialized` — Serialize에서 시크릿 제외

**AppConfig Fallback (BR-05)**:
17. `test_app_config_missing_uses_defaults` — config.toml 없어도 정상
18. `test_app_config_partial_override` — 일부 값만 오버라이드

**Runtime 전환**:
19. `test_switch_cloud_success` — 정상 전환
20. `test_switch_cloud_not_found` — 없는 이름 에러

**Domain Model Deserialization**:
21. `test_server_deserialize` — Nova API JSON → Server
22. `test_network_deserialize` — Neutron API JSON → Network (rename 필드)
23. `test_volume_deserialize` — Cinder API JSON → Volume
