# Detail Design — Core & Infrastructure Components

**Mode**: DETAIL
**Timestamp**: 2026-03-23
**Depth**: Comprehensive (Implementation-Ready)

---

## Table of Contents

- [Core / Application Layer](#core--application-layer)
  1. [App](#1-app)
  2. [EventLoop](#2-eventloop)
  3. [Router](#3-router)
  4. [ActionDispatcher](#4-actiondispatcher)
  5. [BackgroundTracker](#5-backgroundtracker)
- [Infrastructure](#infrastructure)
  6. [Config](#6-config)
  7. [Cache](#7-cache)
  8. [RbacGuard](#8-rbacguard)
  9. [AuditLogger](#9-auditlogger)
  10. [ServiceCatalog](#10-servicecatalog)
- [End-to-End Interaction: Delete Server](#end-to-end-interaction-delete-server-lifecycle)

---

## Core / Application Layer

### 1. App

**Responsibility**: Main orchestrator — owns all components, delegates key/event/tick to the active component via Router, manages global state (auth, quit flag, sidebar toggle).

**Data Owned**:

```rust
pub struct App {
    // --- Global State ---
    pub should_quit: bool,
    pub input_mode: InputMode,             // Normal | Command | Search | Form
    pub sidebar_visible: bool,

    // --- Owned Sub-systems ---
    router: Router,
    components: HashMap<Route, Box<dyn Component>>,
    background_tracker: BackgroundTracker,
    action_tx: mpsc::UnboundedSender<Action>,

    // --- Shared (Arc) ---
    auth: Arc<AuthManager>,
    cache: Arc<Cache>,
    rbac: Arc<RbacGuard>,
    service_catalog: Arc<ServiceCatalog>,
    config: Arc<Config>,
    audit_logger: Arc<AuditLogger>,
}

pub enum InputMode {
    Normal,
    Command,   // ':' prefix active
    Search,    // '/' prefix active
    Form,      // FormWidget has focus
    Confirm,   // ConfirmDialog modal active
}
```

**Interface**:

```rust
impl App {
    /// Bootstrap: load config, authenticate, build components, return App + event_rx
    pub async fn new(config: Config) -> Result<(Self, mpsc::UnboundedReceiver<AppEvent>)>;

    /// Delegate KeyEvent to active component or handle global keys (`:`, `/`, Tab, `q`)
    /// Returns: whether a re-render is needed
    pub fn handle_key(&mut self, key: KeyEvent) -> bool;

    /// Receive background event, route to target component + update BackgroundTracker
    pub fn handle_event(&mut self, event: AppEvent);

    /// Tick handler: spinner animation, Toast TTL expiry, token refresh check
    pub fn on_tick(&mut self);

    /// Master render: LayoutManager -> Header, Sidebar, active Component, InputBar, StatusBar
    pub fn render(&self, frame: &mut Frame);

    /// Register a domain module component for a given route
    pub fn register_component(&mut self, route: Route, component: Box<dyn Component>);

    /// Get mutable reference to active component (determined by Router)
    fn active_component_mut(&mut self) -> Option<&mut Box<dyn Component>>;

    /// Get immutable reference to active component
    fn active_component(&self) -> Option<&Box<dyn Component>>;
}
```

**Dependencies**:

```
App ───depends-on──► Router           (owns, delegates route decisions)
App ───depends-on──► BackgroundTracker (owns, updates on events)
App ───depends-on──► Component (trait) (owns HashMap of dyn Component)
App ───depends-on──► ActionDispatcher  (holds action_tx sender half)
App ───depends-on──► Cache             (Arc, passes to components)
App ───depends-on──► RbacGuard         (Arc, sidebar filtering)
App ───depends-on──► Config            (Arc, cloud context)
App ───depends-on──► ServiceCatalog    (Arc, endpoint resolution)
App ───depends-on──► AuditLogger       (Arc, CUD logging)
App ───depends-on──► AuthManager       (Arc, token state)
```

**Interactions — App.handle_key() flow**:

```
 User KeyEvent
      |
      v
+-----+------+
|   App       |
| handle_key()|
+------+------+
       |
       +--- Is global key? (`:`, `/`, Tab, `q`, Esc from modal)
       |    YES --> handle internally (toggle sidebar, switch InputMode, quit)
       |    NO  --> delegate to active_component_mut()
       |                |
       |                v
       |    +---------------------+
       |    | Component           |
       |    | handle_key(key)     |
       |    +----------+----------+
       |               |
       |               v
       |       Option<Action>
       |               |
       |     Some(action) --> action_tx.send(action)
       |     None         --> (local state update only, re-render)
       v
    return true (needs re-render)
```

---

### 2. EventLoop

**Responsibility**: Unified `tokio::select!` loop — multiplexes crossterm key events, tick timer (200ms), and background `AppEvent` channel into sequential calls on `App`.

**Data Owned**:

```rust
// EventLoop is a function, not a long-lived struct.
// It borrows/owns the following for the duration of the loop:
//   - terminal: &mut Terminal<CrosstermBackend<Stdout>>
//   - app: &mut App
//   - event_rx: mpsc::UnboundedReceiver<AppEvent>
//   - key_stream: crossterm::event::EventStream
//   - tick_interval: tokio::time::Interval (200ms)
```

**Interface**:

```rust
/// Entry point — runs until App.should_quit becomes true.
/// Called from main() after App::new().
pub async fn run_event_loop(
    terminal: &mut Terminal<CrosstermBackend<Stdout>>,
    app: &mut App,
    event_rx: mpsc::UnboundedReceiver<AppEvent>,
) -> Result<()>;
```

**Dependencies**:

```
EventLoop ───calls──► App.handle_key()
EventLoop ───calls──► App.handle_event()
EventLoop ───calls──► App.on_tick()
EventLoop ───calls──► App.render()
EventLoop ───reads──► crossterm::event::EventStream
EventLoop ───reads──► mpsc::UnboundedReceiver<AppEvent>
```

**Interactions — select! loop iteration**:

```
+------------------------------------------------------------------+
|                     tokio::select! loop                           |
|                                                                  |
|  branch 1: key_stream.next()                                     |
|  +-----------+    KeyEvent    +---------+                        |
|  | Crossterm |--------------->|  App    |                        |
|  | EventStream|               |handle_key|                       |
|  +-----------+                +---------+                        |
|                                                                  |
|  branch 2: tick.tick()                                           |
|  +-----------+    ()          +---------+                        |
|  |  Interval |--------------->|  App    |                        |
|  |  200ms    |                |on_tick  |                        |
|  +-----------+                +---------+                        |
|                                                                  |
|  branch 3: event_rx.recv()                                       |
|  +-----------+    AppEvent    +---------+                        |
|  |  mpsc rx  |--------------->|  App    |                        |
|  |           |                |handle_event|                     |
|  +-----------+                +---------+                        |
|                                                                  |
|  after select!:                                                  |
|  +-----------+                +---------+                        |
|  | terminal  |<---------------|  App    |                        |
|  |  .draw()  |   render(f)   | render  |                        |
|  +-----------+                +---------+                        |
|                                                                  |
|  if app.should_quit { break; }                                   |
+------------------------------------------------------------------+
```

---

### 3. Router

**Responsibility**: Maintains current `Route`, determines which component is active, and handles route transitions (push/pop with history stack for Esc back-navigation).

**Data Owned**:

```rust
pub struct Router {
    current: Route,
    history: Vec<Route>,       // stack for Esc back-navigation (max 20)
}

#[derive(Debug, Clone, Copy, Hash, Eq, PartialEq)]
pub enum Route {
    // Nova
    Servers,
    ServerDetail(/* selected at component level */),
    ServerCreate,
    Flavors,
    Migrations,
    Aggregates,
    ComputeServices,
    Hypervisors,

    // Neutron
    Networks,
    NetworkDetail,
    SecurityGroups,
    SecurityGroupDetail,
    FloatingIps,
    Agents,

    // Cinder
    Volumes,
    VolumeDetail,
    VolumeCreate,
    Snapshots,

    // Glance
    Images,
    ImageDetail,

    // Keystone (Admin)
    Projects,
    Users,

    // Monitoring
    Usage,
}
```

**Interface**:

```rust
impl Router {
    pub fn new(initial: Route) -> Self;

    /// Navigate to a new route, pushing current onto history stack
    pub fn navigate(&mut self, to: Route);

    /// Pop history stack, return to previous route. Returns None if history empty.
    pub fn back(&mut self) -> Option<Route>;

    /// Current active route
    pub fn current(&self) -> Route;

    /// Peek at previous route (for breadcrumb display)
    pub fn previous(&self) -> Option<Route>;

    /// Replace current route without pushing to history (e.g., tab switching within same level)
    pub fn replace(&mut self, to: Route);

    /// Clear history and navigate (e.g., on cloud context switch)
    pub fn reset(&mut self, to: Route);
}
```

**Dependencies**:

```
Router ───owned-by──► App
Router ───no external dependencies (pure state machine)
```

**Interactions — Navigation flow**:

```
User types `:servers`       User presses Esc         User types `:ctx prod`
       |                         |                          |
       v                         v                          v
  App.handle_key()          App.handle_key()           App.handle_key()
       |                         |                          |
       v                         v                          v
  CommandParser              Router.back()             Router.reset(Servers)
  -> Route::Servers              |                     (clear history,
       |                    returns Some(prev)          full reload)
       v                         |
  Router.navigate(Servers)       v
  (push old to history)    App switches to prev
       |                   component
       v
  App switches to
  Servers component
```

---

### 4. ActionDispatcher

**Responsibility**: Receives `Action` from UI thread via `action_tx`, spawns tokio background tasks that call API adapters, sends results back via `event_tx`. Manages channel lifecycle.

**Data Owned**:

```rust
// ActionDispatcher is not a persistent struct — it is the background task runner.
// The channel pair is created at App::new() and split:
//   - action_tx: held by App (cloned to components)
//   - action_rx: consumed by the dispatcher loop
//   - event_tx: held by dispatcher, cloned into each spawned task
//   - event_rx: held by EventLoop

pub enum Action {
    // --- Navigation ---
    Navigate(Route),
    Back,

    // --- Nova ---
    FetchServers,
    DeleteServer { id: String, name: String },
    RebootServer { id: String, hard: bool },
    StartServer { id: String },
    StopServer { id: String },
    CreateServer(CreateServerRequest),
    LiveMigrate { id: String, host: Option<String> },
    Evacuate { id: String, host: Option<String> },

    // --- Neutron ---
    FetchNetworks,
    FetchSecurityGroups,
    CreateFloatingIp(CreateFloatingIpRequest),
    DeleteFloatingIp { id: String },

    // --- Cinder ---
    FetchVolumes,
    DeleteVolume { id: String, force: bool },
    ExtendVolume { id: String, new_size: u64 },

    // --- Glance ---
    FetchImages,
    DeleteImage { id: String },

    // --- Keystone Admin ---
    FetchProjects,
    FetchUsers,

    // --- System ---
    RefreshAll,
    SwitchCloud(String),
    InvalidateCache(ResourceType),
    Quit,
}

pub enum AppEvent {
    // --- Data loaded ---
    ServersLoaded(Vec<Server>),
    NetworksLoaded(Vec<Network>),
    VolumesLoaded(Vec<Volume>),
    ImagesLoaded(Vec<Image>),
    ProjectsLoaded(Vec<Project>),
    UsersLoaded(Vec<User>),
    SecurityGroupsLoaded(Vec<SecurityGroup>),
    FloatingIpsLoaded(Vec<FloatingIp>),
    FlavorsLoaded(Vec<Flavor>),

    // --- CUD results ---
    ServerDeleted { id: String, name: String },
    ServerRebooted { id: String },
    ServerStarted { id: String },
    ServerStopped { id: String },
    ServerCreated(Server),
    VolumeDeleted { id: String },
    ImageDeleted { id: String },
    FloatingIpCreated(FloatingIp),

    // --- Error ---
    ApiError { operation_id: String, message: String },

    // --- Auth ---
    TokenRefreshed,
    AuthFailed(String),

    // --- System ---
    CloudSwitched(String),
}
```

**Interface**:

```rust
/// Spawn the dispatcher loop. Returns the action_tx sender for UI to use.
/// The event_tx is captured internally; event_rx is returned to EventLoop.
pub fn spawn_dispatcher(
    action_rx: mpsc::UnboundedReceiver<Action>,
    event_tx: mpsc::UnboundedSender<AppEvent>,
    nova: Arc<dyn NovaService>,
    neutron: Arc<dyn NeutronService>,
    cinder: Arc<dyn CinderService>,
    glance: Arc<dyn GlanceService>,
    keystone: Arc<dyn KeystoneService>,
    auth: Arc<AuthManager>,
    cache: Arc<Cache>,
    audit_logger: Arc<AuditLogger>,
    background_tracker_tx: mpsc::UnboundedSender<TrackingEvent>,
) -> tokio::task::JoinHandle<()>;

/// Individual spawn helper (called inside the dispatcher loop)
fn spawn_api_task<F, Fut>(
    event_tx: mpsc::UnboundedSender<AppEvent>,
    tracker_tx: mpsc::UnboundedSender<TrackingEvent>,
    operation_id: String,
    description: String,
    fut: F,
)
where
    F: FnOnce() -> Fut + Send + 'static,
    Fut: Future<Output = Result<AppEvent>> + Send;
```

**Dependencies**:

```
ActionDispatcher ───calls──► NovaService (Arc<dyn>)
ActionDispatcher ───calls──► NeutronService (Arc<dyn>)
ActionDispatcher ───calls──► CinderService (Arc<dyn>)
ActionDispatcher ───calls──► GlanceService (Arc<dyn>)
ActionDispatcher ───calls──► KeystoneService (Arc<dyn>)
ActionDispatcher ───calls──► AuthManager (Arc, token refresh)
ActionDispatcher ───calls──► Cache (Arc, read/invalidate)
ActionDispatcher ───calls──► AuditLogger (Arc, CUD logging)
ActionDispatcher ───sends──► BackgroundTracker (via tracker_tx)
ActionDispatcher ───sends──► EventLoop (via event_tx)
ActionDispatcher ───reads──► App (via action_rx)
```

**Interactions — Dispatcher loop for DeleteServer**:

```
action_rx.recv()
      |
      v
match Action::DeleteServer { id, name }
      |
      +---> tracker_tx.send(TrackingEvent::Started {
      |         operation_id: uuid,
      |         description: "Deleting server {name}",
      |     })
      |
      +---> audit_logger.log_action(AuditEntry {
      |         action: "DELETE_SERVER",
      |         resource_id: id.clone(),
      |         resource_name: name.clone(),
      |         ..
      |     })
      |
      +---> tokio::spawn(async move {
                match nova.delete_server(&id).await {
                    Ok(()) => {
                        cache.invalidate(ResourceType::Servers);
                        event_tx.send(AppEvent::ServerDeleted { id, name });
                        tracker_tx.send(TrackingEvent::Completed { operation_id });
                    }
                    Err(e) => {
                        event_tx.send(AppEvent::ApiError {
                            operation_id,
                            message: e.to_string(),
                        });
                        tracker_tx.send(TrackingEvent::Failed {
                            operation_id,
                            error: e.to_string(),
                        });
                    }
                }
            });
```

---

### 5. BackgroundTracker

**Responsibility**: Tracks in-progress background operations, updates their status (InProgress/Completed/Failed), triggers Toast notifications on completion or failure.

**Data Owned**:

```rust
pub struct BackgroundTracker {
    operations: HashMap<String, OperationInfo>,  // operation_id -> info
    tracker_rx: mpsc::UnboundedReceiver<TrackingEvent>,
    toasts: Vec<Toast>,
}

pub struct OperationInfo {
    pub operation_id: String,
    pub description: String,         // "Deleting server web-01"
    pub started_at: Instant,
    pub status: OperationStatus,
}

#[derive(Debug, Clone)]
pub enum OperationStatus {
    InProgress,
    Completed,
    Failed(String),
}

pub enum TrackingEvent {
    Started { operation_id: String, description: String },
    Completed { operation_id: String },
    Failed { operation_id: String, error: String },
}

pub struct Toast {
    pub message: String,
    pub level: ToastLevel,           // Success | Error | Info
    pub created_at: Instant,
    pub ttl: Duration,               // default: 5s for success, 10s for error
}

pub enum ToastLevel {
    Success,
    Error,
    Info,
}
```

**Interface**:

```rust
impl BackgroundTracker {
    pub fn new(tracker_rx: mpsc::UnboundedReceiver<TrackingEvent>) -> Self;

    /// Drain all pending TrackingEvents from channel, update internal state.
    /// Called by App.on_tick() every 200ms.
    pub fn poll_updates(&mut self);

    /// Remove expired toasts. Called by App.on_tick().
    pub fn expire_toasts(&mut self);

    /// Get all currently in-progress operations (for status bar spinner display)
    pub fn in_progress(&self) -> Vec<&OperationInfo>;

    /// Get count of in-progress operations
    pub fn in_progress_count(&self) -> usize;

    /// Get active (non-expired) toasts for rendering
    pub fn active_toasts(&self) -> &[Toast];

    /// Check if a specific resource has a pending operation
    pub fn has_pending(&self, resource_id: &str) -> Option<&OperationStatus>;

    /// Clean up completed/failed entries older than 60s
    pub fn gc_old_entries(&mut self);
}
```

**Dependencies**:

```
BackgroundTracker ───owned-by──► App
BackgroundTracker ───reads-from──► ActionDispatcher (via tracker_rx channel)
BackgroundTracker ───used-by──► StatusBar (render toasts)
BackgroundTracker ───used-by──► Components (check pending ops on resources)
```

**Interactions — Lifecycle of a tracked operation**:

```
Time ------>

ActionDispatcher                BackgroundTracker              StatusBar/Toast
      |                               |                             |
      | TrackingEvent::Started         |                             |
      |------------------------------->|                             |
      |                          poll_updates()                      |
      |                          insert InProgress                   |
      |                               |                             |
      |                               |--- on_tick() renders ------>|
      |                               |    spinner: "Deleting..."   |
      |                               |                             |
      | (API call completes)           |                             |
      |                               |                             |
      | TrackingEvent::Completed       |                             |
      |------------------------------->|                             |
      |                          poll_updates()                      |
      |                          status -> Completed                 |
      |                          push Toast(Success, 5s TTL)         |
      |                               |                             |
      |                               |--- on_tick() renders ------>|
      |                               |    toast: "Server deleted"  |
      |                               |                             |
      |                          expire_toasts()                     |
      |                          (after 5s, toast removed)           |
      |                               |                             |
      |                          gc_old_entries()                    |
      |                          (after 60s, operation removed)      |
```

---

## Infrastructure

### 6. Config

**Responsibility**: Parse `clouds.yaml` (OpenStack standard) and app-specific config (`~/.config/nexttui/config.toml`), provide multi-cloud definitions and runtime settings.

**Data Owned**:

```rust
pub struct Config {
    pub clouds: HashMap<String, CloudConfig>,
    pub active_cloud: String,
    pub app: AppConfig,
}

pub struct CloudConfig {
    pub name: String,
    pub auth: AuthConfig,
    pub region_name: Option<String>,
    pub regions: Option<Vec<String>>,       // multi-region support
    pub interface: String,                  // "public" | "internal" | "admin"
    pub identity_api_version: u8,           // default: 3
    pub verify: bool,                       // SSL verification
    pub cacert: Option<PathBuf>,
}

pub struct AuthConfig {
    pub auth_url: String,
    pub auth_type: AuthType,
    // password auth
    pub username: Option<String>,
    pub password: Option<String>,
    pub project_name: Option<String>,
    pub project_domain_name: Option<String>,
    pub user_domain_name: Option<String>,
    // application_credential auth
    pub application_credential_id: Option<String>,
    pub application_credential_secret: Option<String>,
}

pub enum AuthType {
    Password,
    ApplicationCredential,
}

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

**Interface**:

```rust
impl Config {
    /// Load clouds.yaml from standard paths + app config from ~/.config/nexttui/config.toml
    /// Search order for clouds.yaml:
    ///   1. $OS_CLIENT_CONFIG_FILE
    ///   2. ./clouds.yaml
    ///   3. ~/.config/openstack/clouds.yaml
    ///   4. /etc/openstack/clouds.yaml
    pub fn load() -> Result<Self>;

    /// Get active cloud configuration
    pub fn active_cloud_config(&self) -> &CloudConfig;

    /// Get specific cloud configuration by name
    pub fn cloud_config(&self, name: &str) -> Option<&CloudConfig>;

    /// List all available cloud names (for `:ctx` command completion)
    pub fn cloud_names(&self) -> Vec<&str>;

    /// Switch active cloud. Returns Err if cloud name not found.
    pub fn switch_cloud(&mut self, name: &str) -> Result<()>;

    /// Get TTL for a given resource type
    pub fn cache_ttl(&self, resource_type: ResourceType) -> Duration;
}
```

**Dependencies**:

```
Config ───no runtime dependencies (loaded at startup)
Config ───used-by──► App (Arc<Config>)
Config ───used-by──► Cache (TTL values)
Config ───used-by──► AuthManager (auth credentials)
Config ───used-by──► AdapterRegistry (endpoint/SSL settings)
```

**Interactions — Config loading at startup**:

```
main()
  |
  v
Config::load()
  |
  +---> locate clouds.yaml (search 4 paths)
  |     |
  |     v
  |     serde_yaml::from_reader() -> HashMap<String, CloudConfig>
  |
  +---> locate ~/.config/nexttui/config.toml
  |     |
  |     v
  |     toml::from_str() -> AppConfig (or defaults if missing)
  |
  +---> determine active_cloud:
  |     1. CLI arg --cloud
  |     2. config.toml default_cloud
  |     3. $OS_CLOUD env var
  |     4. first entry in clouds.yaml
  |
  v
Config { clouds, active_cloud, app }
  |
  v
App::new(config)
  |
  +---> Arc::new(config) shared to all subsystems
```

---

### 7. Cache

**Responsibility**: In-memory `HashMap` + TTL single-level cache. Per-resource-type TTL. Supports forced invalidation via `:refresh` command. Thread-safe via `RwLock`.

**Data Owned**:

```rust
pub struct Cache {
    entries: RwLock<HashMap<CacheKey, CacheEntry>>,
    ttl_config: HashMap<ResourceType, Duration>,
}

#[derive(Debug, Clone, Hash, Eq, PartialEq)]
pub struct CacheKey {
    pub resource_type: ResourceType,
    pub cloud: String,
    pub qualifier: Option<String>,  // e.g., project_id for scoped queries
}

struct CacheEntry {
    data: CachedData,
    inserted_at: Instant,
    ttl: Duration,
}

/// Type-erased cached data.
/// Each variant holds the deserialized domain model Vec.
pub enum CachedData {
    Servers(Vec<Server>),
    Networks(Vec<Network>),
    Volumes(Vec<Volume>),
    Images(Vec<Image>),
    Flavors(Vec<Flavor>),
    SecurityGroups(Vec<SecurityGroup>),
    FloatingIps(Vec<FloatingIp>),
    Projects(Vec<Project>),
    Users(Vec<User>),
}

#[derive(Debug, Clone, Copy, Hash, Eq, PartialEq)]
pub enum ResourceType {
    Servers,
    Networks,
    Volumes,
    Images,
    Flavors,
    SecurityGroups,
    FloatingIps,
    Projects,
    Users,
    Snapshots,
    Aggregates,
    Hypervisors,
    ComputeServices,
    Agents,
}
```

**Interface**:

```rust
impl Cache {
    pub fn new(ttl_config: HashMap<ResourceType, Duration>) -> Self;

    /// Get cached data if present and not expired. Returns None if miss or expired.
    pub fn get(&self, key: &CacheKey) -> Option<CachedData>;

    /// Insert data with resource-type-specific TTL.
    pub fn put(&self, key: CacheKey, data: CachedData);

    /// Invalidate a specific resource type for the current cloud.
    pub fn invalidate(&self, resource_type: ResourceType, cloud: &str);

    /// Invalidate ALL entries for a cloud (used on cloud context switch).
    pub fn invalidate_cloud(&self, cloud: &str);

    /// Invalidate everything (`:refresh` with no args).
    pub fn invalidate_all(&self);

    /// Check if a key has a valid (non-expired) entry.
    pub fn is_valid(&self, key: &CacheKey) -> bool;

    /// Remove all expired entries (called periodically from on_tick, every 30s).
    pub fn gc_expired(&self);

    /// Get cache stats for status bar display.
    pub fn stats(&self) -> CacheStats;
}

pub struct CacheStats {
    pub total_entries: usize,
    pub valid_entries: usize,
    pub expired_entries: usize,
}
```

**Dependencies**:

```
Cache ───used-by──► ActionDispatcher (read before API call, write after, invalidate on CUD)
Cache ───used-by──► App (invalidate_all on :refresh, invalidate_cloud on :ctx switch)
Cache ───configured-by──► Config (TTL values)
```

**Interactions — Cache hit/miss in FetchServers**:

```
ActionDispatcher receives Action::FetchServers
      |
      v
cache.get(&CacheKey { Servers, "prod", None })
      |
      +--- Some(CachedData::Servers(list))  [HIT]
      |         |
      |         v
      |    event_tx.send(AppEvent::ServersLoaded(list))
      |    (no API call, instant response)
      |
      +--- None  [MISS or EXPIRED]
                |
                v
          nova.list_servers().await
                |
                v
          cache.put(key, CachedData::Servers(result.clone()))
                |
                v
          event_tx.send(AppEvent::ServersLoaded(result))


User types `:refresh`
      |
      v
App -> cache.invalidate_all()
App -> action_tx.send(Action::FetchServers)  // re-fetch current view
```

---

### 8. RbacGuard

**Responsibility**: Determines menu/action visibility based on roles and capabilities. Filters admin-only functions for non-admin users.

[Agent Council Review 2026-03-23] Phase 1은 Keystone 역할 기반, Phase 2에서 Capability 기반으로 확장.
AuthProvider::get_capabilities()로부터 세션별 권한을 수신하여 다중 백엔드 권한 체계를 통합 지원.

**Data Owned**:

```rust
pub struct RbacGuard {
    roles: RwLock<Vec<Role>>,
    project_id: RwLock<Option<String>>,
    is_admin: RwLock<bool>,
    /// [Agent Council] Capability-based access control.
    /// Phase 1: derived from Keystone roles (admin -> all capabilities).
    /// Phase 2: populated from AuthProvider::get_capabilities() per backend.
    capabilities: RwLock<HashSet<Capability>>,
}

#[derive(Debug, Clone)]
pub struct Role {
    pub id: String,
    pub name: String,
}

/// Capability = (resource, action) pair.
/// Imported from port/auth.rs — shared with AuthProvider.
pub use crate::port::auth::Capability;

/// Static permission table — maps Route/Action to required role OR capability.
pub struct Permission {
    pub route: Option<Route>,
    pub action: Option<ActionKind>,
    pub required_role: RequiredRole,
    /// Phase 2: capability-based check (takes precedence over role if set).
    pub required_capability: Option<Capability>,
}

pub enum RequiredRole {
    Any,                    // any authenticated user
    Admin,                  // admin role required
    Role(String),           // specific role name
    AnyOf(Vec<String>),     // any of these roles
}

pub enum ActionKind {
    Read,
    Create,
    Delete,
    ForceDelete,
    Migrate,
    Evacuate,
    EnableDisable,
    ManageQuota,
}
```

**Interface**:

```rust
impl RbacGuard {
    pub fn new() -> Self;

    /// Update roles from auth token (called after authentication and token refresh).
    pub fn update_roles(&self, roles: Vec<Role>, project_id: Option<String>);

    /// [Agent Council] Update capabilities from AuthProvider.
    /// Phase 1: called after auth, derives capabilities from roles.
    /// Phase 2: called with backend-specific capabilities.
    pub fn update_capabilities(&self, capabilities: Vec<Capability>);

    /// Check if current user can access a route (for sidebar filtering).
    pub fn can_access_route(&self, route: &Route) -> bool;

    /// Check if current user can perform a specific action.
    pub fn can_perform(&self, action: ActionKind) -> bool;

    /// [Agent Council] Capability-based permission check.
    /// Phase 1: falls back to role-based check.
    /// Phase 2: checks capabilities HashSet directly.
    pub fn has_capability(&self, resource: &str, action: &str) -> bool;

    /// Filter a list of routes to only those accessible (for sidebar rendering).
    pub fn filter_routes(&self, routes: &[Route]) -> Vec<Route>;

    /// Filter a list of actions to only those permitted (for action menu in detail view).
    pub fn filter_actions(&self, actions: &[ActionKind]) -> Vec<ActionKind>;

    /// Is current user admin?
    pub fn is_admin(&self) -> bool;

    /// Get current project ID.
    pub fn project_id(&self) -> Option<String>;
}
```

**Admin-only routes and actions (hardcoded permission table)**:

```
Admin-only Routes:
  Migrations, Aggregates, ComputeServices, Hypervisors,
  Projects, Users, Agents, Usage

Admin-only Actions:
  ForceDelete, Migrate, Evacuate, EnableDisable, ManageQuota
```

**Dependencies**:

```
RbacGuard ───updated-by──► AuthManager (on token issue/refresh, roles extracted)
RbacGuard ───used-by──► App/Sidebar (filter visible menu items)
RbacGuard ───used-by──► Components (filter available actions in detail view)
RbacGuard ───used-by──► ActionDispatcher (pre-check before spawning CUD tasks)
```

**Interactions — Sidebar filtering**:

```
AuthManager issues token
      |
      v
token.roles = ["admin", "member"]
      |
      v
rbac.update_roles(roles, project_id)
      |   sets is_admin = true (found "admin" role)
      v

Sidebar.render()
      |
      v
all_routes = [Servers, Networks, Volumes, Images,
              Migrations, Aggregates, Projects, Users, ...]
      |
      v
rbac.filter_routes(&all_routes)
      |
      +--- is_admin == true  --> return all routes
      +--- is_admin == false --> filter out admin-only routes
      |
      v
render only visible routes in sidebar
```

---

### 9. AuditLogger

**Responsibility**: Write local audit log for all CUD (Create/Update/Delete) operations to `~/.config/nexttui/audit.log`. Mask sensitive fields (passwords, tokens). Append-only, structured JSON lines.

**Data Owned**:

```rust
pub struct AuditLogger {
    log_path: PathBuf,
    writer: Mutex<BufWriter<File>>,   // append mode
}

#[derive(Debug, Serialize)]
pub struct AuditEntry {
    pub timestamp: String,            // ISO 8601
    pub cloud: String,
    pub user: String,
    pub project: Option<String>,
    pub action: String,               // "DELETE_SERVER", "CREATE_VOLUME", etc.
    pub resource_type: String,        // "server", "volume", etc.
    pub resource_id: String,
    pub resource_name: Option<String>,
    pub details: Option<serde_json::Value>,  // additional context (masked)
    pub result: AuditResult,
}

#[derive(Debug, Serialize)]
pub enum AuditResult {
    Initiated,
    Success,
    Failed(String),
}

/// Fields that must be masked in audit log details
const SENSITIVE_FIELDS: &[&str] = &[
    "password", "token", "secret", "credential",
    "api_key", "private_key", "auth_token",
];
```

**Interface**:

```rust
impl AuditLogger {
    /// Open (or create) audit log file in append mode.
    pub fn new(log_path: PathBuf) -> Result<Self>;

    /// Log a CUD action initiation. Called by ActionDispatcher before spawning task.
    pub fn log_initiated(&self, entry: AuditEntry) -> Result<()>;

    /// Log a CUD action result (success or failure). Called by ActionDispatcher after task completes.
    pub fn log_result(&self, operation_id: &str, result: AuditResult) -> Result<()>;

    /// Mask sensitive fields in a serde_json::Value recursively.
    fn mask_sensitive(value: &mut serde_json::Value);

    /// Rotate log if size exceeds 10MB (rename to .1, .2, etc., keep 5).
    pub fn rotate_if_needed(&self) -> Result<()>;
}
```

**Dependencies**:

```
AuditLogger ───called-by──► ActionDispatcher (on every CUD action)
AuditLogger ───configured-by──► Config (log path)
AuditLogger ───no runtime dependencies (writes to local filesystem only)
```

**Interactions — Audit log write on delete**:

```
ActionDispatcher: Action::DeleteServer { id: "abc-123", name: "web-01" }
      |
      v
audit_logger.log_initiated(AuditEntry {
    timestamp: "2026-03-23T14:30:00+09:00",
    cloud: "prod",
    user: "admin",
    project: Some("infra"),
    action: "DELETE_SERVER",
    resource_type: "server",
    resource_id: "abc-123",
    resource_name: Some("web-01"),
    details: None,
    result: AuditResult::Initiated,
})
      |
      v
--> appends to ~/.config/nexttui/audit.log:
{"timestamp":"2026-03-23T14:30:00+09:00","cloud":"prod","user":"admin",
 "project":"infra","action":"DELETE_SERVER","resource_type":"server",
 "resource_id":"abc-123","resource_name":"web-01","result":"Initiated"}

      ... API call completes ...

audit_logger.log_result("op-uuid", AuditResult::Success)
      |
      v
--> appends:
{"timestamp":"2026-03-23T14:30:02+09:00","operation_id":"op-uuid",
 "result":"Success"}
```

---

### 10. ServiceCatalog

**Responsibility**: Store and query Keystone service catalog from auth token. Resolve per-service endpoints by type, interface preference, and region.

**Data Owned**:

```rust
pub struct ServiceCatalog {
    catalog: RwLock<Vec<CatalogEntry>>,
    region: RwLock<Option<String>>,
    interface_preference: RwLock<String>,  // "public" | "internal" | "admin"
}

#[derive(Debug, Clone)]
pub struct CatalogEntry {
    pub service_type: String,     // "compute", "network", "volumev3", "identity", "image"
    pub service_name: String,     // "nova", "neutron", "cinderv3", "keystone", "glance"
    pub endpoints: Vec<Endpoint>,
}

#[derive(Debug, Clone)]
pub struct Endpoint {
    pub url: String,
    pub interface: String,        // "public", "internal", "admin"
    pub region: String,
    pub region_id: String,
}

/// Well-known OpenStack service types
pub enum ServiceType {
    Compute,      // "compute"    -> Nova
    Network,      // "network"    -> Neutron
    BlockStorage, // "volumev3"   -> Cinder
    Identity,     // "identity"   -> Keystone
    Image,        // "image"      -> Glance
}
```

**Interface**:

```rust
impl ServiceCatalog {
    pub fn new(interface_preference: String) -> Self;

    /// Update catalog from Keystone token response. Called after auth and token refresh.
    pub fn update(&self, catalog: Vec<CatalogEntry>, region: Option<String>);

    /// Resolve endpoint URL for a service type.
    /// Resolution order:
    ///   1. Match service_type
    ///   2. Match region (if set)
    ///   3. Match interface preference (fallback: public -> internal -> admin)
    pub fn endpoint(&self, service_type: ServiceType) -> Result<String>;

    /// Get endpoint for a specific service type with explicit region override.
    pub fn endpoint_in_region(&self, service_type: ServiceType, region: &str) -> Result<String>;

    /// List all available regions across all services.
    pub fn available_regions(&self) -> Vec<String>;

    /// List all discovered service types.
    pub fn available_services(&self) -> Vec<String>;

    /// Set active region (for multi-region switching).
    pub fn set_region(&self, region: &str);

    /// Get current region.
    pub fn current_region(&self) -> Option<String>;

    /// Check if a service type is available in the catalog.
    pub fn has_service(&self, service_type: ServiceType) -> bool;
}
```

**Dependencies**:

```
ServiceCatalog ───updated-by──► AuthManager (on token issue/refresh)
ServiceCatalog ───used-by──► AdapterRegistry (to construct HTTP adapters with correct endpoints)
ServiceCatalog ───used-by──► Header widget (display current region)
ServiceCatalog ───configured-by──► Config (interface preference, region)
```

**Interactions — Endpoint resolution flow**:

```
AuthManager authenticates with Keystone
      |
      v
Token response includes service catalog JSON:
  [{ "type": "compute", "endpoints": [
       { "url": "https://nova.prod:8774/v2.1", "interface": "internal", "region": "RegionOne" },
       { "url": "https://nova-pub.prod:8774/v2.1", "interface": "public", "region": "RegionOne" },
  ]}, ...]
      |
      v
service_catalog.update(parsed_catalog, Some("RegionOne"))
      |
      v

AdapterRegistry building NovaHttpAdapter:
      |
      v
service_catalog.endpoint(ServiceType::Compute)
      |
      +---> filter by type == "compute"
      +---> filter by region == "RegionOne"
      +---> filter by interface == "internal" (from Config)
      |         found? YES -> return "https://nova.prod:8774/v2.1"
      |         found? NO  -> fallback to "public"
      |                        -> fallback to "admin"
      |                        -> Err(ServiceNotFound)
      v
NovaHttpAdapter { endpoint: "https://nova.prod:8774/v2.1", ... }
```

---

## End-to-End Interaction: Delete Server Lifecycle

User presses `d` on a selected server in the Servers list view. The full lifecycle from keypress to UI update:

```
User
  |
  | presses 'd'
  v
+-------------------------------------------------------------------+
| EventLoop (tokio::select! branch 1: key_stream)                   |
|   crossterm::Event::Key(KeyEvent { code: Char('d'), .. })         |
+----------------------------+--------------------------------------+
                             |
                             v
+-------------------------------------------------------------------+
| App.handle_key(key)                                               |
|   input_mode == Normal                                            |
|   not a global key -> delegate to active component                |
|   active_component = components[Route::Servers]                   |
+----------------------------+--------------------------------------+
                             |
                             v
+-------------------------------------------------------------------+
| ServerModule.handle_key(key)                                      |
|   key == 'd' -> get selected server                               |
|   server = self.servers[self.selected_index]                      |
|   self.confirm_dialog = Some(ConfirmDialog::new(                  |
|       "Delete server 'web-01'? Type server name to confirm:"     |
|   ))                                                              |
|   return None  (no Action yet — waiting for confirmation)         |
+-------------------------------------------------------------------+
                             |
                             v
                    (render shows ConfirmDialog modal)

User types "web-01" + Enter
  |
  v
+-------------------------------------------------------------------+
| ServerModule.handle_key(key) [in Confirm mode]                    |
|   confirmation matches server name                                |
|   self.confirm_dialog = None                                      |
|   return Some(Action::DeleteServer {                              |
|       id: "abc-123", name: "web-01"                               |
|   })                                                              |
+----------------------------+--------------------------------------+
                             |
                             v
+-------------------------------------------------------------------+
| App.handle_key() receives Some(action)                            |
|   action_tx.send(Action::DeleteServer { id, name })               |
+----------------------------+--------------------------------------+
                             |
                             | (mpsc channel)
                             v
+-------------------------------------------------------------------+
| ActionDispatcher (background tokio task)                          |
|   action_rx.recv() -> Action::DeleteServer { id, name }          |
|                                                                   |
|   1. Generate operation_id = Uuid::new_v4()                      |
|                                                                   |
|   2. tracker_tx.send(TrackingEvent::Started {                    |
|          operation_id, description: "Deleting server web-01"     |
|      })                                                           |
|                                                                   |
|   3. audit_logger.log_initiated(AuditEntry {                     |
|          action: "DELETE_SERVER",                                 |
|          resource_id: "abc-123",                                 |
|          resource_name: "web-01",                                |
|          result: Initiated,                                      |
|      })                                                           |
|                                                                   |
|   4. tokio::spawn(async move {                                   |
|          // --- runs on tokio thread pool ---                    |
|          let result = nova.delete_server("abc-123").await;       |
|                                                                   |
|          match result {                                          |
|              Ok(()) => {                                         |
|                  cache.invalidate(Servers, "prod");              |
|                  event_tx.send(AppEvent::ServerDeleted {         |
|                      id: "abc-123", name: "web-01"               |
|                  });                                             |
|                  tracker_tx.send(TrackingEvent::Completed {      |
|                      operation_id                                |
|                  });                                             |
|                  audit_logger.log_result(op_id, Success);        |
|              }                                                   |
|              Err(e) => {                                         |
|                  event_tx.send(AppEvent::ApiError {              |
|                      operation_id, message: e.to_string()        |
|                  });                                             |
|                  tracker_tx.send(TrackingEvent::Failed {         |
|                      operation_id, error: e.to_string()          |
|                  });                                             |
|                  audit_logger.log_result(op_id, Failed(..));     |
|              }                                                   |
|          }                                                       |
|      });                                                         |
+-------------------------------------------------------------------+
                             |
          (meanwhile, on next tick — 200ms later)
                             |
                             v
+-------------------------------------------------------------------+
| EventLoop (tokio::select! branch 2: tick)                         |
|   App.on_tick()                                                   |
|     -> background_tracker.poll_updates()                         |
|        (drains TrackingEvent::Started from tracker_rx)           |
|     -> StatusBar now shows spinner: "Deleting server web-01..."  |
+-------------------------------------------------------------------+
                             |
          (API call completes, ~500ms-2s later)
                             |
                             v
+-------------------------------------------------------------------+
| EventLoop (tokio::select! branch 3: event_rx)                    |
|   event_rx.recv() -> AppEvent::ServerDeleted { id, name }        |
|                                                                   |
|   App.handle_event(event)                                        |
|     -> route to active component:                                |
|        ServerModule.handle_event(AppEvent::ServerDeleted { .. }) |
|          -> self.servers.retain(|s| s.id != "abc-123")           |
|          -> self.selected_index = min(idx, len-1)                |
|                                                                   |
|     -> on next tick: background_tracker.poll_updates()           |
|        (drains TrackingEvent::Completed)                         |
|        -> push Toast { "Server 'web-01' deleted", Success, 5s } |
+-------------------------------------------------------------------+
                             |
                             v
+-------------------------------------------------------------------+
| EventLoop: terminal.draw(|f| app.render(f))                      |
|                                                                   |
|   Renders:                                                        |
|   +--------------------------------------------------+           |
|   | HEADER: Servers | prod | RegionOne               |           |
|   +--------+-----------------------------------------+           |
|   |SIDEBAR | ID       | Name    | Status   | ...     |           |
|   |        | def-456  | web-02  | ACTIVE   |         |           |
|   | >Servers| ghi-789 | db-01   | ACTIVE   |         |           |
|   |  Networks|         |         |          |         |           |
|   |  Volumes |         |(web-01 is gone)    |         |           |
|   +--------+-----------------------------------------+           |
|   | :                                                 |           |
|   +--------------------------------------------------+           |
|   | Server 'web-01' deleted [SUCCESS]   2 servers     |           |
|   +--------------------------------------------------+           |
|     ^--- Toast (auto-removes after 5s)                            |
+-------------------------------------------------------------------+
```

**Sequence diagram (compact ASCII)**:

```
User        EventLoop     App         ServerModule   ActionDisp.   Nova API    BgTracker   StatusBar
 |              |           |              |              |            |            |           |
 |--'d'-------->|           |              |              |            |            |           |
 |              |--key----->|              |              |            |            |           |
 |              |           |--key-------->|              |            |            |           |
 |              |           |    (show ConfirmDialog)     |            |            |           |
 |              |           |<---None------|              |            |            |           |
 |              |           |              |              |            |            |           |
 |--"web-01"--->|           |              |              |            |            |           |
 |--Enter------>|           |              |              |            |            |           |
 |              |--key----->|              |              |            |            |           |
 |              |           |--key-------->|              |            |            |           |
 |              |           |<-DeleteServer|              |            |            |           |
 |              |           |--action_tx------------------>|            |            |           |
 |              |           |              |              |            |            |           |
 |              |           |              |         track Started-------------------->|           |
 |              |           |              |         audit log        |            |           |
 |              |           |              |         spawn----------->|            |           |
 |              |           |              |              |  DELETE   |            |           |
 |              |           |              |              |  /servers |            |           |
 |              |           |              |              |  /abc-123 |            |           |
 |              |           |              |              |            |            |           |
 |              |--tick---->|              |              |            |            |           |
 |              |           |--poll--------|--------------|------------|--updates-->|           |
 |              |           |              |              |            |      "Deleting..."    |
 |              |           |              |              |            |            |---------->|
 |              |           |              |              |            |            |  spinner  |
 |              |           |              |              |<--200 OK---|            |           |
 |              |           |              |         cache.invalidate  |            |           |
 |              |           |              |         event_tx.send     |            |           |
 |              |           |              |         track Completed----------------->|           |
 |              |           |              |              |            |            |           |
 |              |<---event--|              |              |            |            |           |
 |              |--event--->|              |              |            |            |           |
 |              |           |--event------>|              |            |            |           |
 |              |           |    servers.retain(!=abc-123)|            |            |           |
 |              |           |              |              |            |            |           |
 |              |--tick---->|              |              |            |            |           |
 |              |           |--poll--------|--------------|------------|--updates-->|           |
 |              |           |              |              |            |  push Toast(Success)   |
 |              |           |              |              |            |            |---------->|
 |              |           |              |              |            |            |  "deleted"|
 |              |--draw---->|              |              |            |            |           |
 |<-------------|  (updated list + toast)  |              |            |            |           |
 |              |           |              |              |            |            |           |
```

---

## Summary: Dependency Graph

```
                          +------------------+
                          |    EventLoop     |
                          | (tokio::select!) |
                          +--------+---------+
                                   |
                          calls    |   handle_key / handle_event / on_tick / render
                                   v
                          +--------+---------+
          +---------------|       App        |---------------+
          |               | (orchestrator)   |               |
          |               +---+---------+----+               |
          |                   |         |                    |
          v                   v         v                    v
   +------+------+    +------+---+ +---+--------+   +------+----------+
   |   Router    |    |Components| |Background  |   | ActionDispatcher|
   | (navigation)|    |(dyn trait)| |Tracker     |   | (tokio tasks)   |
   +-------------+    +----------+ +---+--------+   +---+---+---------+
                                       |                |   |
                                       | tracker_tx     |   | event_tx
                                       +<---------------+   |
                                                            |
                    +-------+-------+-------+-------+       |
                    v       v       v       v       v       |
                +------+ +------+ +------+ +-----+ +------+|
                | Nova | |Neutron| |Cinder| |Glance| |Keystone|
                | Port | | Port | | Port | | Port| | Port  |
                +------+ +------+ +------+ +-----+ +-------+
                    |                                    |
                    |           Shared (Arc)             |
          +---------+-----+-----+--------+--------------+
          v               v     v        v
   +------+---+    +------+-+ +-+------+ +----------+  +-----------+
   |  Config  |    | Cache  | | RBAC   | |  Audit   |  | Service   |
   |          |    |        | | Guard  | |  Logger  |  | Catalog   |
   +----------+    +--------+ +--------+ +----------+  +-----------+
```

---

## Component Trait (shared interface for all domain modules)

For reference, the trait that all 16 domain modules implement:

```rust
pub trait Component {
    /// Handle key input. Return Some(Action) to trigger background work.
    fn handle_key(&mut self, key: KeyEvent) -> Option<Action>;

    /// Handle background event result (API response, error, etc.)
    fn handle_event(&mut self, event: &AppEvent);

    /// Render this component into the given frame area.
    fn render(&self, frame: &mut Frame, area: Rect);

    /// Called every tick (200ms). For animations, polling, etc.
    fn on_tick(&mut self) {}

    /// Return the Action needed to load initial data for this component.
    /// Called by App when navigating to this component's route.
    fn init_action(&self) -> Option<Action> { None }

    /// Human-readable title for the header bar.
    fn title(&self) -> &str;
}
```
