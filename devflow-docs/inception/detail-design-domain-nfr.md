# Detail Design — Domain Modules & NFR Patterns

**Mode**: DETAIL
**Timestamp**: 2026-03-23
**Depth**: Comprehensive (Implementation-Ready)
**Scope**: 16 Domain Modules (Component trait) + 5 NFR Pattern Categories

---

## Table of Contents

- [PART 1: Domain Modules](#part-1-domain-modules)
  - [1.0 Common Pattern](#10-common-pattern)
  - [1.1 ServerModule](#11-servermodule)
  - [1.2 MigrationModule](#12-migrationmodule)
  - [1.3 FlavorModule](#13-flavormodule)
  - [1.4 NetworkModule](#14-networkmodule)
  - [1.5 SecurityGroupModule](#15-securitygroupmodule)
  - [1.6 FloatingIpModule](#16-floatingipmodule)
  - [1.7 AgentModule](#17-agentmodule)
  - [1.8 VolumeModule](#18-volumemodule)
  - [1.9 SnapshotModule](#19-snapshotmodule)
  - [1.10 ImageModule](#110-imagemodule)
  - [1.11 ProjectModule](#111-projectmodule)
  - [1.12 UserModule](#112-usermodule)
  - [1.13 AggregateModule](#113-aggregatemodule)
  - [1.14 ComputeServiceModule](#114-computeservicemodule)
  - [1.15 HypervisorModule](#115-hypervisormodule)
  - [1.16 UsageModule](#116-usagemodule)
  - [Summary Table](#1x-domain-module-summary)
- [PART 2: NFR Design Patterns](#part-2-nfr-design-patterns)
  - [2.1 Performance](#21-performance)
  - [2.2 Security](#22-security)
  - [2.3 Availability](#23-availability)
  - [2.4 Data Integrity](#24-data-integrity)
  - [2.5 Deployment](#25-deployment)
- [Appendix: Action / AppEvent Enum](#appendix-action--appevent-enum)

---

## PART 1: Domain Modules

All 16 domain modules implement the Component trait:

```rust
pub trait Component {
    fn handle_key(&mut self, key: KeyEvent) -> Option<Action>;
    fn handle_event(&mut self, event: AppEvent);
    fn render(&self, frame: &mut Frame, area: Rect);
}
```

### 1.0 Common Pattern

모든 domain module이 공유하는 구조적 패턴을 정의한다. 개별 모듈 섹션에서는 이 패턴과 **다른 점**만 기술한다.

#### ViewModel 분리 (Agent Council 권고)

[Agent Council Review 2026-03-23] Domain Model(`Server`, `Volume` 등)이 UI 위젯 파라미터(`ColumnDef`, `FormField`)에 직접 결합되지 않도록, 변환 로직을 별도 `view_model` 모듈에 분리한다.

```rust
// src/module/server/view_model.rs
// Domain Model → UI 표현 변환을 담당. Domain Module 내부에서만 사용.

pub fn server_columns() -> Vec<ColumnDef> { /* ... */ }
pub fn server_to_row(server: &Server) -> Vec<Cell> { /* ... */ }
pub fn server_detail_fields(server: &Server) -> Vec<DetailField> { /* ... */ }
pub fn server_create_form_fields() -> Vec<FormFieldDef> { /* ... */ }
```

- Phase 1: 각 모듈 내 `view_model.rs` 서브모듈로 분리
- Phase 2: UI 위젯 변경 시 view_model만 수정, Domain Model과 Component 로직은 무변경
- 파일 구조: `src/module/<name>/mod.rs` + `src/module/<name>/view_model.rs`

#### ViewState 상태 머신

```rust
#[derive(Debug, Clone, PartialEq)]
pub enum ViewState {
    List,             // ResourceList 위젯으로 목록 표시
    Detail(String),   // DetailView 위젯으로 상세 표시 (String = resource_id)
    Create,           // FormWidget으로 생성 폼
    Edit(String),     // FormWidget으로 수정 폼 (String = resource_id)
}
```

모듈별로 사용하는 ViewState subset이 다르다. 예를 들어 HypervisorModule은 `List`와 `Detail`만, AgentModule은 `List`만 사용한다.

#### Common State 구조

```rust
/// 모든 domain module이 내부적으로 보유하는 공통 상태.
/// 제네릭 T는 해당 모듈의 도메인 모델 (Server, Volume 등).
struct ModuleState<T> {
    view_state: ViewState,
    items: Vec<T>,
    selected_index: usize,
    loading: bool,
    search_query: Option<String>,
    filtered_indices: Option<Vec<usize>>,   // 검색 필터 적용 시
    pending_ops: HashMap<String, OperationStatus>,
    form_state: Option<FormState>,
    confirm_dialog: Option<ConfirmDialogState>,
    error_message: Option<String>,
    action_tx: mpsc::UnboundedSender<Action>,
}
```

#### Component trait 구현 — 공통 흐름

**handle_key**: ViewState 분기 + ConfirmDialog 우선 처리

```rust
fn handle_key(&mut self, key: KeyEvent) -> Option<Action> {
    // Step 1: ConfirmDialog가 활성화되어 있으면 다이얼로그에 먼저 위임
    if let Some(ref mut dialog) = self.state.confirm_dialog {
        return match dialog.handle_key(key) {
            ConfirmResult::Confirmed => {
                let action = self.execute_pending_action();
                self.state.confirm_dialog = None;
                action
            }
            ConfirmResult::Cancelled => {
                self.state.confirm_dialog = None;
                None
            }
            ConfirmResult::Pending => None,
        };
    }

    // Step 2: ViewState별 키 처리
    match self.state.view_state {
        ViewState::List   => self.handle_list_key(key),
        ViewState::Detail(_) => self.handle_detail_key(key),
        ViewState::Create | ViewState::Edit(_) => self.handle_form_key(key),
    }
}
```

**handle_event**: 백그라운드 작업 결과 수신

```rust
fn handle_event(&mut self, event: AppEvent) {
    match event {
        AppEvent::ResourceLoaded { resource_type, data } if resource_type == Self::TYPE => {
            self.state.items = *data.downcast().unwrap();
            self.state.loading = false;
            self.state.error_message = None;
        }
        AppEvent::OperationCompleted { id, action, .. } => {
            self.state.pending_ops.remove(&id);
            // 목록 자동 리프레시 트리거
        }
        AppEvent::ApiError { id, error, .. } => {
            if let Some(id) = &id {
                self.state.pending_ops.insert(id.clone(), OperationStatus::Failed(error.clone()));
            }
            self.state.error_message = Some(error);
            self.state.loading = false;
        }
        _ => {}
    }
}
```

**render**: ViewState별 위젯 위임 + overlay

```rust
fn render(&self, frame: &mut Frame, area: Rect) {
    // 메인 콘텐츠
    match &self.state.view_state {
        ViewState::List => {
            self.resource_list.render(frame, area, &self.visible_items(), self.state.selected_index);
        }
        ViewState::Detail(id) => {
            if let Some(item) = self.find_item(id) {
                self.detail_view.render(frame, area, item);
            }
        }
        ViewState::Create | ViewState::Edit(_) => {
            if let Some(ref form) = self.state.form_state {
                self.form_widget.render(frame, area, form);
            }
        }
    }

    // Overlay: loading spinner
    if self.state.loading {
        render_loading_spinner(frame, area);
    }

    // Overlay: ConfirmDialog (최상위)
    if let Some(ref dialog) = self.state.confirm_dialog {
        dialog.render(frame, area);
    }
}
```

#### List 뷰 공통 키 바인딩

```rust
fn handle_list_key(&mut self, key: KeyEvent) -> Option<Action> {
    match key.code {
        KeyCode::Char('j') | KeyCode::Down => { self.move_selection_down(); None }
        KeyCode::Char('k') | KeyCode::Up   => { self.move_selection_up(); None }
        KeyCode::Char('G')                  => { self.move_selection_last(); None }
        KeyCode::Char('g')                  => { self.move_selection_first(); None }
        KeyCode::PageDown                   => { self.page_down(); None }
        KeyCode::PageUp                     => { self.page_up(); None }
        KeyCode::Enter => {
            let id = self.selected_item_id();
            self.state.view_state = ViewState::Detail(id.clone());
            Some(Action::LoadDetail { resource_type: Self::TYPE, id })
        }
        KeyCode::Char('c') => {
            self.init_create_form();
            self.state.view_state = ViewState::Create;
            None
        }
        KeyCode::Char('d') => {
            self.state.confirm_dialog = Some(ConfirmDialogState::new_destructive(
                "Delete",
                &self.selected_item_display_name(),
            ));
            None
        }
        KeyCode::Char('r') => Some(Self::list_action()),  // 수동 리프레시
        KeyCode::Esc => Some(Action::Navigate(Route::previous())),
        _ => None,
    }
}
```

#### Detail 뷰 공통 키 바인딩

```rust
fn handle_detail_key(&mut self, key: KeyEvent) -> Option<Action> {
    match key.code {
        KeyCode::Esc | KeyCode::Char('q') => {
            self.state.view_state = ViewState::List;
            None
        }
        KeyCode::Char('e') => {
            // Edit 지원 모듈만 구현
            self.init_edit_form();
            None
        }
        _ => self.handle_detail_specific_key(key),  // 모듈별 추가 키
    }
}
```

#### Form 뷰 공통 키 바인딩

```rust
fn handle_form_key(&mut self, key: KeyEvent) -> Option<Action> {
    if let Some(ref mut form) = self.state.form_state {
        match key.code {
            KeyCode::Tab       => { form.next_field(); None }
            KeyCode::BackTab   => { form.prev_field(); None }
            KeyCode::Enter     => {
                if form.validate_all() {
                    let request = self.build_request_from_form(form);
                    self.state.confirm_dialog = Some(
                        ConfirmDialogState::new_submit("Create", &self.form_summary(form))
                    );
                    None
                } else {
                    None  // 검증 실패 시 에러 하이라이트만
                }
            }
            KeyCode::Esc => {
                self.state.view_state = ViewState::List;
                self.state.form_state = None;
                None
            }
            _ => { form.handle_input(key); None }
        }
    } else {
        None
    }
}
```

#### Action 디스패치 / Event 수신 시퀀스

```
User Input (KeyEvent)
  -> Component::handle_key()
    -> returns Some(Action::Server(ServerAction::Delete(id)))
      -> action_tx.send(action)
        -> ActionDispatcher receives
          -> tokio::spawn(async { ports.nova.delete_server(&id).await })
            -> success: event_tx.send(AppEvent::Server(ServerEvent::Deleted(id)))
            -> failure: event_tx.send(AppEvent::ApiError { .. })
              -> EventLoop receives via select!
                -> App::handle_event() -> routes to active Component
                  -> Component::handle_event() -> updates local state
                    -> next render() reflects new state
```

#### UI 위젯 사용 규칙

| ViewState | 주 위젯 | 보조 위젯 |
|-----------|---------|----------|
| `List` | `ResourceList` | `SearchFilter` 하이라이트 |
| `Detail` | `DetailView` | 중첩 `ResourceList` (연관 리소스 테이블) |
| `Create` / `Edit` | `FormWidget` | — |
| 모든 상태 (overlay) | — | `ConfirmDialog`, Loading spinner, `Toast` |

---

### 1.1 ServerModule

- **Responsibility**: 서버(인스턴스) 리스트/상세/생성/기본액션(삭제/리부트/시작/중지)/이벤트/스냅샷 관리
- **Admin-only**: No (일반 사용자도 접근, 소유 리소스 범위)

#### Interface — Module-specific Actions & Events

```rust
pub enum ServerAction {
    List,
    GetDetail(String),
    Create(ServerCreateRequest),
    Delete(String),                            // ConfirmDialog 필수
    Reboot { id: String, hard: bool },         // ConfirmDialog 필수
    Start(String),
    Stop(String),
    CreateSnapshot { server_id: String, name: String },
    LoadEvents(String),
    // 생성 폼 참조 데이터
    LoadFormReferences,                        // flavors + images + networks + SGs + keypairs
}

pub enum ServerEvent {
    ListLoaded(Vec<Server>),
    DetailLoaded(Server),
    EventsLoaded { server_id: String, events: Vec<InstanceAction> },
    FormReferencesLoaded {
        flavors: Vec<Flavor>,
        images: Vec<Image>,
        networks: Vec<Network>,
        security_groups: Vec<SecurityGroup>,
        keypairs: Vec<String>,
    },
    ActionCompleted { id: String, action_name: String },
    ActionFailed { id: String, action_name: String, error: String },
}
```

#### Dependencies

| Port trait | Methods |
|-----------|---------|
| `NovaPort` | `list_servers`, `get_server`, `create_server`, `delete_server`, `reboot_server`, `start_server`, `stop_server`, `create_server_image`, `list_instance_actions`, `list_flavors`, `list_keypairs` |
| `GlancePort` | `list_images` (생성 폼 드롭다운) |
| `NeutronPort` | `list_networks`, `list_security_groups` (생성 폼 드롭다운) |

UI Widgets: `ResourceList`, `DetailView`, `FormWidget`, `ConfirmDialog`

#### Data Owned

```rust
pub struct ServerModule {
    state: ModuleState<Server>,
    // Detail 뷰 추가 데이터
    events_cache: HashMap<String, Vec<InstanceAction>>,
    // 생성 폼 참조 데이터 (드롭다운 소스)
    available_flavors: Vec<Flavor>,
    available_images: Vec<Image>,
    available_networks: Vec<Network>,
    available_security_groups: Vec<SecurityGroup>,
    available_keypairs: Vec<String>,
    // 위젯 인스턴스
    resource_list: ResourceList,
    detail_view: DetailView,
    form_widget: FormWidget,
}
```

#### ResourceList 칼럼 정의

```rust
fn columns() -> Vec<ColumnDef> {
    vec![
        ColumnDef::icon("", 3, |s: &Server| status_icon(&s.status)),
        ColumnDef::text("Name", 25, |s: &Server| &s.name),
        ColumnDef::text("Status", 10, |s: &Server| s.status.as_str()),
        ColumnDef::computed("IP", 20, |s: &Server| format_ips(&s.addresses)),
        ColumnDef::text("Flavor", 15, |s: &Server| &s.flavor_name),
        ColumnDef::text("Image", 15, |s: &Server| &s.image_name),
    ]
}
```

#### Detail 뷰 추가 키

```rust
fn handle_detail_specific_key(&mut self, key: KeyEvent) -> Option<Action> {
    match key.code {
        KeyCode::Char('R') => {  // Hard reboot
            self.state.confirm_dialog = Some(ConfirmDialogState::new_destructive(
                "Hard Reboot", &self.selected_server_name(),
            ));
            None
        }
        KeyCode::Char('s') => {  // Snapshot
            self.init_snapshot_form();
            None
        }
        KeyCode::Char('S') => Some(Action::Server(ServerAction::Start(self.current_id()))),
        KeyCode::Char('X') => Some(Action::Server(ServerAction::Stop(self.current_id()))),
        KeyCode::Char('v') => {  // Events 탭 토글
            self.toggle_events_view();
            Some(Action::Server(ServerAction::LoadEvents(self.current_id())))
        }
        _ => None,
    }
}
```

#### DetailView 섹션 구성

- **기본 정보**: ID, Name, Status, AZ, Keypair, Created, Uptime
- **하드웨어**: Flavor (vCPU, RAM, Disk)
- **네트워크**: 중첩 ResourceList (Network Name, Fixed IP, Floating IP, MAC)
- **볼륨**: 중첩 ResourceList (Volume Name, Size, Device, Status)
- **이벤트** (토글): 중첩 ResourceList (Action, Start, Finish, Result, Message)

---

### 1.2 MigrationModule

- **Responsibility**: 서버 마이그레이션(Live/Block/Cold), Evacuate, 상태 강제 변경
- **Admin-only**: **Yes**

#### Interface

```rust
pub enum MigrationAction {
    ListServers,
    LiveMigrate { server_id: String, host: Option<String>, block_migration: bool },
    ColdMigrate { server_id: String },
    Evacuate { server_id: String, host: Option<String> },
    ForceState { server_id: String, state: ServerState },  // 2단계 확인 필수
    LoadTargetHosts,
}

pub enum MigrationEvent {
    ServersLoaded(Vec<Server>),
    TargetHostsLoaded(Vec<String>),
    MigrationStarted { server_id: String, kind: String },
    MigrationCompleted { server_id: String },
    MigrationFailed { server_id: String, error: String },
    StateChanged { server_id: String, new_state: ServerState },
}
```

#### Dependencies

| Port trait | Methods |
|-----------|---------|
| `NovaPort` | `list_servers`, `live_migrate_server`, `migrate_server`, `evacuate_server`, `reset_server_state`, `list_hypervisors` |

UI Widgets: `ResourceList`, `FormWidget`, `ConfirmDialog`

#### Data Owned

```rust
pub struct MigrationModule {
    state: ModuleState<Server>,
    target_hosts: Vec<String>,
    resource_list: ResourceList,
    form_widget: FormWidget,
}
```

#### ViewState 사용

- `List`: 서버 목록에서 마이그레이션 대상 선택
- `Create`: 마이그레이션 타입/대상 호스트 폼 (FormWidget)
- `Detail`, `Edit`: 미사용

#### 폼 필드 정의

```rust
fn migration_form_fields() -> Vec<FormField> {
    vec![
        FormField::dropdown("type", "Migration Type",
            &["Live Migration", "Block Migration", "Cold Migration", "Evacuate", "Force State"]),
        FormField::dropdown_optional("target_host", "Target Host", &[]),  // 동적 로드
        FormField::dropdown("force_state", "Target State",               // type=ForceState일 때만 표시
            &["ACTIVE", "ERROR", "PAUSED", "SUSPENDED"]),
    ]
}
```

#### 특수 로직

- **모든 작업이 2단계 확인** (서버명 재입력) -- 운영 중 서버에 대한 고위험 작업
- Evacuate 시: 호스트 상태가 `down`인 서버만 필터링 표시
- ForceState: 현재 상태와 대상 상태를 함께 표시하여 혼동 방지

---

### 1.3 FlavorModule

- **Responsibility**: 플레이버 리스트/생성/삭제
- **Admin-only**: No (리스트 전체, 생성/삭제는 RbacGuard가 Admin일 때만 키 활성화)

#### Interface

```rust
pub enum FlavorAction {
    List,
    Create(FlavorCreateRequest),   // Admin only
    Delete(String),                // Admin only
}

pub enum FlavorEvent {
    ListLoaded(Vec<Flavor>),
    Created(Flavor),
    Deleted(String),
    ActionFailed { id: Option<String>, error: String },
}
```

#### Dependencies

| Port trait | Methods |
|-----------|---------|
| `NovaPort` | `list_flavors`, `create_flavor`, `delete_flavor` |

UI Widgets: `ResourceList`, `FormWidget`, `ConfirmDialog`

#### Data Owned

```rust
pub struct FlavorModule {
    state: ModuleState<Flavor>,
    resource_list: ResourceList,
    form_widget: FormWidget,
}
```

#### ViewState 사용

- `List`, `Create`만 사용
- `Detail`, `Edit`: 미사용 (플레이버는 immutable, 상세 정보가 리스트 칼럼에 충분히 표시됨)

#### ResourceList 칼럼 정의

```rust
fn columns() -> Vec<ColumnDef> {
    vec![
        ColumnDef::text("Name", 20, |f: &Flavor| &f.name),
        ColumnDef::number("vCPU", 6, |f: &Flavor| f.vcpus),
        ColumnDef::number("RAM (MB)", 10, |f: &Flavor| f.ram),
        ColumnDef::number("Disk (GB)", 10, |f: &Flavor| f.disk),
        ColumnDef::bool_icon("Public", 8, |f: &Flavor| f.is_public),
    ]
}
```

#### 특수 로직

- `c` (Create), `d` (Delete) 키는 `RbacGuard::is_admin()` 체크 후 활성화
- Admin이 아닌 경우 해당 키 입력 무시 (StatusBar에 "Admin 권한 필요" 표시)

---

### 1.4 NetworkModule

- **Responsibility**: 네트워크 리스트/상세/생성
- **Admin-only**: No

#### Interface

```rust
pub enum NetworkAction {
    List,
    GetDetail(String),
    Create(NetworkCreateRequest),
}

pub enum NetworkEvent {
    ListLoaded(Vec<Network>),
    DetailLoaded(Network),  // subnets 포함
    Created(Network),
    ActionFailed { id: Option<String>, error: String },
}
```

#### Dependencies

| Port trait | Methods |
|-----------|---------|
| `NeutronPort` | `list_networks`, `get_network`, `create_network` |

UI Widgets: `ResourceList`, `DetailView`, `FormWidget`

#### Data Owned

```rust
pub struct NetworkModule {
    state: ModuleState<Network>,
    resource_list: ResourceList,
    detail_view: DetailView,
    form_widget: FormWidget,
}
```

#### ResourceList 칼럼 정의

```rust
fn columns() -> Vec<ColumnDef> {
    vec![
        ColumnDef::text("Name", 25, |n: &Network| &n.name),
        ColumnDef::text("Status", 10, |n: &Network| n.status.as_str()),
        ColumnDef::computed("Admin", 8, |n: &Network| bool_label(n.admin_state_up, "UP", "DOWN")),
        ColumnDef::bool_icon("External", 9, |n: &Network| n.is_external),
        ColumnDef::bool_icon("Shared", 8, |n: &Network| n.shared),
        ColumnDef::number("MTU", 6, |n: &Network| n.mtu),
    ]
}
```

#### DetailView 섹션 구성

- **기본 정보**: ID, Name, Status, Description
- **설정**: Shared, External, MTU, Port Security Enabled
- **Provider**: Network Type, Physical Network, Segmentation ID
- **Subnets**: 중첩 ResourceList (Name, CIDR, Gateway, DHCP Enabled, Allocation Pools)

---

### 1.5 SecurityGroupModule

- **Responsibility**: 보안그룹 리스트/상세/CRUD + 룰 추가/삭제
- **Admin-only**: No

#### Interface

```rust
pub enum SecurityGroupAction {
    List,
    GetDetail(String),
    Create(SecurityGroupCreateRequest),
    Update { id: String, name: Option<String>, description: Option<String> },
    Delete(String),                                     // ConfirmDialog
    AddRule(SecurityGroupRuleCreateRequest),
    DeleteRule(String),                                 // ConfirmDialog
}

pub enum SecurityGroupEvent {
    ListLoaded(Vec<SecurityGroup>),
    DetailLoaded(SecurityGroup),                        // rules 포함
    Created(SecurityGroup),
    Updated(SecurityGroup),
    Deleted(String),
    RuleAdded(SecurityGroupRule),
    RuleDeleted(String),
    ActionFailed { id: Option<String>, error: String },
}
```

#### Dependencies

| Port trait | Methods |
|-----------|---------|
| `NeutronPort` | `list_security_groups`, `get_security_group`, `create_security_group`, `update_security_group`, `delete_security_group`, `create_security_group_rule`, `delete_security_group_rule` |

UI Widgets: `ResourceList` (x2: SG 목록 + 룰 목록), `DetailView`, `FormWidget` (x2: SG 폼 + 룰 폼), `ConfirmDialog`

#### Data Owned

```rust
pub struct SecurityGroupModule {
    state: ModuleState<SecurityGroup>,
    // Detail 뷰 내 룰 목록 상태
    selected_rule_index: usize,
    show_rule_form: bool,
    rule_form_state: Option<FormState>,
    // 위젯
    resource_list: ResourceList,
    rules_list: ResourceList,      // 상세뷰 내 중첩 목록
    detail_view: DetailView,
    form_widget: FormWidget,
    rule_form_widget: FormWidget,
}
```

#### Detail 뷰 내 키 바인딩

```rust
fn handle_detail_specific_key(&mut self, key: KeyEvent) -> Option<Action> {
    match key.code {
        KeyCode::Char('j') | KeyCode::Down => { self.selected_rule_index += 1; None }
        KeyCode::Char('k') | KeyCode::Up   => { self.selected_rule_index -= 1; None }
        KeyCode::Char('a') => {  // Add Rule
            self.show_rule_form = true;
            self.rule_form_state = Some(FormState::new(Self::rule_form_fields()));
            None
        }
        KeyCode::Char('d') => {  // Delete Rule
            let rule_id = self.selected_rule_id();
            self.state.confirm_dialog = Some(ConfirmDialogState::new_simple(
                "Delete Rule", &format!("Rule {}", &rule_id[..8]),
            ));
            None
        }
        _ => None,
    }
}
```

#### 룰 폼 필드

```rust
fn rule_form_fields() -> Vec<FormField> {
    vec![
        FormField::dropdown("direction", "Direction", &["Ingress", "Egress"]),
        FormField::dropdown("protocol", "Protocol", &["TCP", "UDP", "ICMP", "Any"]),
        FormField::text("port_range_min", "Port Min", Validator::port()),
        FormField::text("port_range_max", "Port Max", Validator::port()),
        FormField::text("remote_ip_prefix", "Source CIDR", Validator::cidr_or_empty()),
        FormField::text("remote_group_id", "Source SG ID", Validator::uuid_or_empty()),
    ]
}
```

#### DetailView 섹션

- **기본 정보**: ID, Name, Description
- **Ingress Rules**: 중첩 ResourceList (Protocol, Port Range, Source)
- **Egress Rules**: 중첩 ResourceList (Protocol, Port Range, Destination)

---

### 1.6 FloatingIpModule

- **Responsibility**: Floating IP 리스트/생성/삭제/Associate/Disassociate
- **Admin-only**: No

#### Interface

```rust
pub enum FloatingIpAction {
    List,
    Create { network_id: String },
    Delete(String),                                     // ConfirmDialog
    Associate { fip_id: String, port_id: String },
    Disassociate(String),
    LoadExternalNetworks,
    LoadPorts,
}

pub enum FloatingIpEvent {
    ListLoaded(Vec<FloatingIp>),
    Created(FloatingIp),
    Deleted(String),
    Associated { fip_id: String },
    Disassociated { fip_id: String },
    ExternalNetworksLoaded(Vec<Network>),
    PortsLoaded(Vec<PortSummary>),
    ActionFailed { id: Option<String>, error: String },
}
```

#### Dependencies

| Port trait | Methods |
|-----------|---------|
| `NeutronPort` | `list_floating_ips`, `create_floating_ip`, `delete_floating_ip`, `update_floating_ip`, `list_networks` (external filter), `list_ports` |

UI Widgets: `ResourceList`, `FormWidget`, `ConfirmDialog`

#### Data Owned

```rust
pub struct FloatingIpModule {
    state: ModuleState<FloatingIp>,
    external_networks: Vec<Network>,    // Create 드롭다운
    available_ports: Vec<PortSummary>,  // Associate 드롭다운
    resource_list: ResourceList,
    form_widget: FormWidget,
}
```

#### ViewState 사용

- `List`, `Create`만 사용 (`Detail`, `Edit` 미사용)

#### ResourceList 칼럼 정의

```rust
fn columns() -> Vec<ColumnDef> {
    vec![
        ColumnDef::text("IP Address", 18, |f: &FloatingIp| &f.floating_ip_address),
        ColumnDef::text("Status", 10, |f: &FloatingIp| f.status.as_str()),
        ColumnDef::computed("Server", 20, |f: &FloatingIp|
            f.attached_server_name.as_deref().unwrap_or("-").to_string()),
        ColumnDef::computed("Fixed IP", 18, |f: &FloatingIp|
            f.fixed_ip_address.as_deref().unwrap_or("-").to_string()),
        ColumnDef::text("Network", 20, |f: &FloatingIp| &f.floating_network_name),
    ]
}
```

#### List 뷰 추가 키

```rust
// 공통 handle_list_key에 추가
KeyCode::Char('a') => {  // Associate
    self.load_ports_and_show_form();
    None
}
KeyCode::Char('u') => {  // Disassociate (unbind)
    let fip = self.selected_fip();
    if fip.fixed_ip_address.is_some() {
        self.state.confirm_dialog = Some(ConfirmDialogState::new_simple(
            "Disassociate", &fip.floating_ip_address,
        ));
    }
    None
}
```

---

### 1.7 AgentModule

- **Responsibility**: Network Agent 리스트/Enable/Disable/삭제
- **Admin-only**: **Yes**

#### Interface

```rust
pub enum AgentAction {
    List,
    Enable(String),
    Disable(String),
    Delete(String),   // 2단계 확인
}

pub enum AgentEvent {
    ListLoaded(Vec<NetworkAgent>),
    Enabled(String),
    Disabled(String),
    Deleted(String),
    ActionFailed { id: String, error: String },
}
```

#### Dependencies

| Port trait | Methods |
|-----------|---------|
| `NeutronPort` | `list_agents`, `update_agent`, `delete_agent` |

UI Widgets: `ResourceList`, `ConfirmDialog`

#### Data Owned

```rust
pub struct AgentModule {
    state: ModuleState<NetworkAgent>,
    resource_list: ResourceList,
}
```

#### ViewState 사용

- `List`만 사용 (Detail/Create/Edit 없음)

#### ResourceList 칼럼 정의

```rust
fn columns() -> Vec<ColumnDef> {
    vec![
        ColumnDef::text("Type", 25, |a: &NetworkAgent| &a.agent_type),
        ColumnDef::text("Host", 20, |a: &NetworkAgent| &a.host),
        ColumnDef::computed("Admin", 10, |a: &NetworkAgent|
            bool_label(a.admin_state_up, "Enabled", "Disabled").to_string()),
        ColumnDef::bool_icon("Alive", 8, |a: &NetworkAgent| a.alive),
        ColumnDef::computed("Updated", 20, |a: &NetworkAgent| format_time(&a.heartbeat_timestamp)),
    ]
}
```

#### List 뷰 키 바인딩 (공통 오버라이드)

```rust
KeyCode::Char('e') => Some(Action::Agent(AgentAction::Enable(self.selected_id()))),
KeyCode::Char('x') => Some(Action::Agent(AgentAction::Disable(self.selected_id()))),
KeyCode::Char('d') => {
    self.state.confirm_dialog = Some(ConfirmDialogState::new_destructive(
        "Delete Agent", &self.selected_agent_display(),
    ));
    None
}
```

---

### 1.8 VolumeModule

- **Responsibility**: 볼륨 리스트/상세/생성/액션(삭제/확장/연결/분리/강제삭제/상태변경)
- **Admin-only**: No (ForceDelete/ResetState는 Admin)

#### Interface

```rust
pub enum VolumeAction {
    List,
    GetDetail(String),
    Create(VolumeCreateRequest),
    Delete(String),                                      // ConfirmDialog
    Extend { id: String, new_size_gb: u64 },
    Attach { volume_id: String, server_id: String },
    Detach { volume_id: String, server_id: String },
    ForceDelete(String),                                 // Admin, 2단계 확인
    ResetState { id: String, state: VolumeState },       // Admin, 2단계 확인
    LoadServers,
    LoadVolumeTypes,
}

pub enum VolumeEvent {
    ListLoaded(Vec<Volume>),
    DetailLoaded(Volume),
    Created(Volume),
    Deleted(String),
    Extended { id: String },
    Attached { volume_id: String, server_id: String },
    Detached { volume_id: String },
    ForceDeleted(String),
    StateReset { id: String, new_state: VolumeState },
    ServersLoaded(Vec<ServerSummary>),
    VolumeTypesLoaded(Vec<VolumeType>),
    ActionFailed { id: Option<String>, error: String },
}
```

#### Dependencies

| Port trait | Methods |
|-----------|---------|
| `CinderPort` | `list_volumes`, `get_volume`, `create_volume`, `delete_volume`, `extend_volume`, `attach_volume`, `detach_volume`, `force_delete_volume`, `reset_volume_state`, `list_volume_types` |
| `NovaPort` | `list_servers` (Attach 시 서버 드롭다운) |

UI Widgets: `ResourceList`, `DetailView`, `FormWidget`, `ConfirmDialog`

#### Data Owned

```rust
pub struct VolumeModule {
    state: ModuleState<Volume>,
    available_servers: Vec<ServerSummary>,
    available_volume_types: Vec<VolumeType>,
    resource_list: ResourceList,
    detail_view: DetailView,
    form_widget: FormWidget,
}
```

#### ResourceList 칼럼 정의

```rust
fn columns() -> Vec<ColumnDef> {
    vec![
        ColumnDef::text("Name", 20, |v: &Volume| &v.name),
        ColumnDef::text("Status", 12, |v: &Volume| v.status.as_str()),
        ColumnDef::number("Size(GB)", 9, |v: &Volume| v.size),
        ColumnDef::computed("Type", 12, |v: &Volume| v.volume_type.as_deref().unwrap_or("-").to_string()),
        ColumnDef::bool_icon("Boot", 6, |v: &Volume| v.bootable),
        ColumnDef::computed("Server", 18, |v: &Volume|
            v.attached_server_name.as_deref().unwrap_or("-").to_string()),
    ]
}
```

#### Detail 뷰 추가 키

```rust
fn handle_detail_specific_key(&mut self, key: KeyEvent) -> Option<Action> {
    match key.code {
        KeyCode::Char('E') => { self.show_extend_form(); None }       // Extend
        KeyCode::Char('a') => { self.show_attach_form(); None }       // Attach
        KeyCode::Char('t') => {                                        // Detach
            self.state.confirm_dialog = Some(ConfirmDialogState::new_simple(
                "Detach Volume", &self.current_volume_name(),
            ));
            None
        }
        KeyCode::Char('F') if self.is_admin => {                      // Force Delete (Admin)
            self.state.confirm_dialog = Some(ConfirmDialogState::new_destructive(
                "Force Delete", &self.current_volume_name(),
            ));
            None
        }
        KeyCode::Char('Z') if self.is_admin => {                      // Reset State (Admin)
            self.show_reset_state_form();
            None
        }
        _ => None,
    }
}
```

#### DetailView 섹션

- **기본 정보**: ID, Name, Description, Size, Status, Type, Encrypted, Bootable, AZ
- **연결 정보**: Server Name, Device Path, Attachment Status
- **스냅샷**: 중첩 ResourceList (Name, Size, Status, Created)

#### 특수 로직

- Extend: 현재 크기보다 큰 값만 허용 (폼 검증: `Validator::min(current_size + 1)`)
- Attach: 서버 목록 드롭다운 (LoadServers 선행 필요)
- ForceDelete/ResetState: `RbacGuard::is_admin()` 런타임 체크, 2단계 확인

---

### 1.9 SnapshotModule

- **Responsibility**: 볼륨 스냅샷 리스트/상세/삭제
- **Admin-only**: No

#### Interface

```rust
pub enum SnapshotAction {
    List,
    GetDetail(String),
    Delete(String),   // ConfirmDialog
}

pub enum SnapshotEvent {
    ListLoaded(Vec<VolumeSnapshot>),
    DetailLoaded(VolumeSnapshot),
    Deleted(String),
    ActionFailed { id: Option<String>, error: String },
}
```

#### Dependencies

| Port trait | Methods |
|-----------|---------|
| `CinderPort` | `list_snapshots`, `get_snapshot`, `delete_snapshot` |

UI Widgets: `ResourceList`, `DetailView`, `ConfirmDialog`

#### Data Owned

```rust
pub struct SnapshotModule {
    state: ModuleState<VolumeSnapshot>,
    resource_list: ResourceList,
    detail_view: DetailView,
}
```

#### ViewState 사용

- `List`, `Detail`만 사용 (Create/Edit 미사용)

#### ResourceList 칼럼 정의

```rust
fn columns() -> Vec<ColumnDef> {
    vec![
        ColumnDef::text("Name", 20, |s: &VolumeSnapshot| &s.name),
        ColumnDef::text("Volume", 20, |s: &VolumeSnapshot| &s.volume_name),
        ColumnDef::number("Size(GB)", 9, |s: &VolumeSnapshot| s.size),
        ColumnDef::text("Status", 12, |s: &VolumeSnapshot| s.status.as_str()),
        ColumnDef::computed("Created", 20, |s: &VolumeSnapshot| format_time(&s.created_at)),
    ]
}
```

---

### 1.10 ImageModule

- **Responsibility**: 이미지 리스트/상세/등록/수정/삭제
- **Admin-only**: No (등록/삭제는 Admin, 수정은 소유자 또는 Admin)

#### Interface

```rust
pub enum ImageAction {
    List,
    GetDetail(String),
    Create(ImageCreateRequest),     // Admin
    Update(ImageUpdateRequest),
    Delete(String),                 // Admin, ConfirmDialog
}

pub enum ImageEvent {
    ListLoaded(Vec<Image>),
    DetailLoaded(Image),
    Created(Image),
    Updated(Image),
    Deleted(String),
    ActionFailed { id: Option<String>, error: String },
}
```

#### Dependencies

| Port trait | Methods |
|-----------|---------|
| `GlancePort` | `list_images`, `get_image`, `create_image`, `update_image`, `delete_image` |

UI Widgets: `ResourceList`, `DetailView`, `FormWidget`, `ConfirmDialog`

#### Data Owned

```rust
pub struct ImageModule {
    state: ModuleState<Image>,
    resource_list: ResourceList,
    detail_view: DetailView,
    form_widget: FormWidget,
}
```

#### ResourceList 칼럼 정의

```rust
fn columns() -> Vec<ColumnDef> {
    vec![
        ColumnDef::text("Name", 25, |i: &Image| &i.name),
        ColumnDef::text("Status", 10, |i: &Image| i.status.as_str()),
        ColumnDef::text("Format", 10, |i: &Image| &i.disk_format),
        ColumnDef::computed("Size", 10, |i: &Image| format_bytes(i.size)),
        ColumnDef::text("Visibility", 12, |i: &Image| i.visibility.as_str()),
        ColumnDef::computed("Created", 12, |i: &Image| format_date(&i.created_at)),
    ]
}
```

#### 생성 폼 필드

```rust
fn create_form_fields() -> Vec<FormField> {
    vec![
        FormField::text("name", "Name", Validator::required()),
        FormField::dropdown("disk_format", "Disk Format",
            &["qcow2", "raw", "vmdk", "vdi", "iso"]),
        FormField::dropdown("container_format", "Container Format",
            &["bare", "docker", "ova", "aki", "ari", "ami"]),
        FormField::dropdown("visibility", "Visibility",
            &["public", "private", "shared", "community"]),
        FormField::text("source_url", "Source URL", Validator::url_or_empty()),
        FormField::number("min_disk", "Min Disk (GB)", Validator::non_negative()),
        FormField::number("min_ram", "Min RAM (MB)", Validator::non_negative()),
    ]
}
```

#### DetailView 섹션

- **기본 정보**: ID, Name, Status, Disk Format, Container Format, Size, Checksum
- **속성**: Min Disk, Min RAM, Architecture, OS Type, OS Version
- **가시성**: Visibility, Owner, Protected

#### 특수 로직

- Phase 1: 이미지 소스는 URL 지정만 지원 (로컬 파일 업로드는 Phase 2)
- 수정 폼: name, visibility, min_disk, min_ram만 변경 허용

---

### 1.11 ProjectModule

- **Responsibility**: 프로젝트 리스트/생성/삭제 + Quota 관리
- **Admin-only**: **Yes**

#### Interface

```rust
pub enum ProjectAction {
    List,
    GetDetail(String),
    Create(ProjectCreateRequest),
    Delete(String),                                      // 2단계 확인
    GetQuota(String),                                    // compute + volume + network 통합
    UpdateQuota { project_id: String, quota: QuotaUpdateRequest },
}

pub enum ProjectEvent {
    ListLoaded(Vec<Project>),
    DetailLoaded(Project),
    Created(Project),
    Deleted(String),
    QuotaLoaded { project_id: String, quota: ProjectQuota },
    QuotaUpdated { project_id: String },
    ActionFailed { id: Option<String>, error: String },
}

/// 세 서비스의 Quota를 통합한 뷰 모델
pub struct ProjectQuota {
    // Compute (Nova)
    pub cores: QuotaValue,
    pub ram_mb: QuotaValue,
    pub instances: QuotaValue,
    // Volume (Cinder)
    pub volumes: QuotaValue,
    pub gigabytes: QuotaValue,
    pub snapshots: QuotaValue,
    // Network (Neutron)
    pub floating_ips: QuotaValue,
    pub security_groups: QuotaValue,
    pub routers: QuotaValue,
}

pub struct QuotaValue {
    pub limit: i64,     // -1 = unlimited
    pub in_use: i64,
}
```

#### Dependencies

| Port trait | Methods |
|-----------|---------|
| `KeystonePort` | `list_projects`, `get_project`, `create_project`, `delete_project` |
| `NovaPort` | `get_quota`, `update_quota` |
| `CinderPort` | `get_quota`, `update_quota` |
| `NeutronPort` | `get_quota`, `update_quota` |

UI Widgets: `ResourceList`, `DetailView`, `FormWidget` (x2: 프로젝트 생성 + Quota 수정), `ConfirmDialog`

#### Data Owned

```rust
pub struct ProjectModule {
    state: ModuleState<Project>,
    current_quota: Option<ProjectQuota>,
    show_quota_form: bool,
    quota_form_state: Option<FormState>,
    resource_list: ResourceList,
    detail_view: DetailView,
    form_widget: FormWidget,
    quota_form_widget: FormWidget,
}
```

#### ResourceList 칼럼 정의

```rust
fn columns() -> Vec<ColumnDef> {
    vec![
        ColumnDef::text("Name", 25, |p: &Project| &p.name),
        ColumnDef::text("ID", 36, |p: &Project| &p.id),
        ColumnDef::bool_icon("Enabled", 9, |p: &Project| p.enabled),
        ColumnDef::computed("Description", 30, |p: &Project|
            p.description.as_deref().unwrap_or("-").to_string()),
    ]
}
```

#### Detail 뷰 추가 키

```rust
KeyCode::Char('q') => {  // Quota 조회/수정
    Some(Action::Project(ProjectAction::GetQuota(self.current_id())))
}
```

#### DetailView 섹션

- **기본 정보**: ID, Name, Description, Domain, Enabled
- **Quota (토글)**: Compute (cores in_use/limit, ram, instances) + Volume (volumes, GB, snapshots) + Network (FIPs, SGs, routers)

#### 특수 로직

- Quota 수정 폼: 현재값(in_use/limit) 표시, limit 필드만 편집 가능
- Quota 업데이트는 세 서비스(Nova/Cinder/Neutron)에 각각 API 호출 (병렬 dispatch)
- 프로젝트 삭제는 2단계 확인 (프로젝트명 재입력)

---

### 1.12 UserModule

- **Responsibility**: 사용자 리스트/생성/삭제 + 역할 부여/회수
- **Admin-only**: **Yes**

#### Interface

```rust
pub enum UserAction {
    List,
    Create(UserCreateRequest),
    Delete(String),                                    // 2단계 확인
    GrantRole { user_id: String, project_id: String, role_id: String },
    RevokeRole { user_id: String, project_id: String, role_id: String },  // ConfirmDialog
    LoadProjects,
    LoadRoles,
}

pub enum UserEvent {
    ListLoaded(Vec<User>),
    Created(User),
    Deleted(String),
    RoleGranted { user_id: String, project_name: String, role_name: String },
    RoleRevoked { user_id: String, project_name: String, role_name: String },
    ProjectsLoaded(Vec<ProjectSummary>),
    RolesLoaded(Vec<Role>),
    ActionFailed { id: Option<String>, error: String },
}
```

#### Dependencies

| Port trait | Methods |
|-----------|---------|
| `KeystonePort` | `list_users`, `create_user`, `delete_user`, `grant_role`, `revoke_role`, `list_roles`, `list_projects` |

UI Widgets: `ResourceList`, `FormWidget` (x2: 생성 + 역할 부여), `ConfirmDialog`

#### Data Owned

```rust
pub struct UserModule {
    state: ModuleState<User>,
    available_projects: Vec<ProjectSummary>,
    available_roles: Vec<Role>,
    role_form_state: Option<FormState>,
    resource_list: ResourceList,
    form_widget: FormWidget,
    role_form_widget: FormWidget,
}
```

#### ViewState 사용

- `List`, `Create`만 사용 (Detail/Edit 미사용)

#### ResourceList 칼럼 정의

```rust
fn columns() -> Vec<ColumnDef> {
    vec![
        ColumnDef::text("Name", 20, |u: &User| &u.name),
        ColumnDef::text("ID", 36, |u: &User| &u.id),
        ColumnDef::computed("Email", 25, |u: &User|
            u.email.as_deref().unwrap_or("-").to_string()),
        ColumnDef::bool_icon("Enabled", 9, |u: &User| u.enabled),
        ColumnDef::computed("Project", 20, |u: &User|
            u.default_project_name.as_deref().unwrap_or("-").to_string()),
    ]
}
```

#### List 뷰 추가 키

```rust
KeyCode::Char('r') => {  // Role management
    self.load_role_references();
    self.show_role_form = true;
    None
}
```

#### 생성 폼 필드

```rust
fn create_form_fields() -> Vec<FormField> {
    vec![
        FormField::text("name", "Username", Validator::required()),
        FormField::password("password", "Password", Validator::min_length(8)),
        FormField::text("email", "Email", Validator::email_or_empty()),
        FormField::dropdown("default_project_id", "Default Project", &[]),  // 동적 로드
        FormField::dropdown("domain_id", "Domain", &[]),                    // 동적 로드
    ]
}
```

#### 역할 부여 폼 필드

```rust
fn role_form_fields() -> Vec<FormField> {
    vec![
        FormField::dropdown("project_id", "Project", &[]),   // 동적 로드
        FormField::dropdown("role_id", "Role", &[]),          // 동적 로드
    ]
}
```

#### 특수 로직

- 패스워드 필드: `FormField::password` 타입으로 입력 시 `*` 마스킹
- 패스워드는 메모리 전용 (`secrecy::Secret<String>`), 감사 로그에 `[REDACTED]` 기록
- 사용자 삭제: 2단계 확인 (사용자명 재입력)
- 역할 회수: ConfirmDialog (단순 Y/N, 역할명+프로젝트명 표시)

---

### 1.13 AggregateModule

- **Responsibility**: Aggregate 리스트/상세/CRUD + 호스트 추가/제거
- **Admin-only**: **Yes**

#### Interface

```rust
pub enum AggregateAction {
    List,
    GetDetail(String),
    Create(AggregateCreateRequest),
    Update { id: String, name: Option<String>, availability_zone: Option<String> },
    Delete(String),                                      // ConfirmDialog
    AddHost { aggregate_id: String, host: String },
    RemoveHost { aggregate_id: String, host: String },   // ConfirmDialog
    SetMetadata { aggregate_id: String, metadata: HashMap<String, String> },
    LoadAvailableHosts,
}

pub enum AggregateEvent {
    ListLoaded(Vec<Aggregate>),
    DetailLoaded(Aggregate),
    Created(Aggregate),
    Updated(Aggregate),
    Deleted(String),
    HostAdded { aggregate_id: String, host: String },
    HostRemoved { aggregate_id: String, host: String },
    MetadataUpdated { aggregate_id: String },
    AvailableHostsLoaded(Vec<String>),
    ActionFailed { id: Option<String>, error: String },
}
```

#### Dependencies

| Port trait | Methods |
|-----------|---------|
| `NovaPort` | `list_aggregates`, `get_aggregate`, `create_aggregate`, `update_aggregate`, `delete_aggregate`, `add_host_to_aggregate`, `remove_host_from_aggregate`, `set_aggregate_metadata`, `list_hypervisors` |

UI Widgets: `ResourceList` (x2: Aggregate + Host), `DetailView`, `FormWidget`, `ConfirmDialog`

#### Data Owned

```rust
pub struct AggregateModule {
    state: ModuleState<Aggregate>,
    available_hosts: Vec<String>,
    selected_host_index: usize,
    resource_list: ResourceList,
    hosts_list: ResourceList,
    detail_view: DetailView,
    form_widget: FormWidget,
}
```

#### ResourceList 칼럼 정의

```rust
fn columns() -> Vec<ColumnDef> {
    vec![
        ColumnDef::text("Name", 20, |a: &Aggregate| &a.name),
        ColumnDef::computed("AZ", 15, |a: &Aggregate|
            a.availability_zone.as_deref().unwrap_or("-").to_string()),
        ColumnDef::computed("Hosts", 6, |a: &Aggregate| a.hosts.len().to_string()),
        ColumnDef::computed("Metadata", 30, |a: &Aggregate| format_metadata(&a.metadata)),
    ]
}
```

#### Detail 뷰 추가 키

```rust
fn handle_detail_specific_key(&mut self, key: KeyEvent) -> Option<Action> {
    match key.code {
        KeyCode::Char('a') => {  // Add Host
            self.load_available_hosts();
            self.show_host_select_form();
            None
        }
        KeyCode::Char('d') => {  // Remove Host
            let host = self.selected_host();
            self.state.confirm_dialog = Some(ConfirmDialogState::new_simple(
                "Remove Host", &host,
            ));
            None
        }
        KeyCode::Char('m') => {  // Set Metadata
            self.show_metadata_form();
            None
        }
        KeyCode::Char('j') | KeyCode::Down => { self.selected_host_index += 1; None }
        KeyCode::Char('k') | KeyCode::Up   => {
            self.selected_host_index = self.selected_host_index.saturating_sub(1);
            None
        }
        _ => None,
    }
}
```

#### DetailView 섹션

- **기본 정보**: ID, Name, Availability Zone, Created, Updated
- **Metadata**: Key-Value 테이블
- **Hosts**: 중첩 ResourceList (Hostname) -- `a`로 추가, `d`로 제거

---

### 1.14 ComputeServiceModule

- **Responsibility**: Compute Service 리스트/Enable/Disable
- **Admin-only**: **Yes**

#### Interface

```rust
pub enum ComputeServiceAction {
    List,
    Enable(String),                                    // service_id
    Disable { id: String, reason: Option<String> },    // disable 사유 입력
}

pub enum ComputeServiceEvent {
    ListLoaded(Vec<ComputeService>),
    Enabled(String),
    Disabled(String),
    ActionFailed { id: String, error: String },
}
```

#### Dependencies

| Port trait | Methods |
|-----------|---------|
| `NovaPort` | `list_compute_services`, `enable_compute_service`, `disable_compute_service` |

UI Widgets: `ResourceList`, `FormWidget` (disable 사유), `ConfirmDialog`

#### Data Owned

```rust
pub struct ComputeServiceModule {
    state: ModuleState<ComputeService>,
    resource_list: ResourceList,
    form_widget: FormWidget,   // disable reason 입력
}
```

#### ViewState 사용

- `List`만 사용

#### ResourceList 칼럼 정의

```rust
fn columns() -> Vec<ColumnDef> {
    vec![
        ColumnDef::text("Host", 25, |s: &ComputeService| &s.host),
        ColumnDef::text("Binary", 20, |s: &ComputeService| &s.binary),
        ColumnDef::text("Status", 10, |s: &ComputeService| s.status.as_str()),
        ColumnDef::computed("State", 8, |s: &ComputeService|
            if s.state == "up" { "Up" } else { "Down" }.to_string()),
        ColumnDef::computed("Disabled Reason", 25, |s: &ComputeService|
            s.disabled_reason.as_deref().unwrap_or("-").to_string()),
        ColumnDef::computed("Updated", 20, |s: &ComputeService| format_time(&s.updated_at)),
    ]
}
```

#### List 뷰 키 바인딩

```rust
KeyCode::Char('e') => {
    Some(Action::ComputeService(ComputeServiceAction::Enable(self.selected_id())))
}
KeyCode::Char('x') => {
    // Disable: 사유 입력 폼 표시
    self.state.form_state = Some(FormState::new(vec![
        FormField::text("reason", "Disable Reason", Validator::optional()),
    ]));
    None
}
```

#### 특수 로직

- Disable 시 사유 입력 폼 (FormWidget, 단일 텍스트 필드)
- Disable 확인 다이얼로그에 경고: "해당 호스트에 신규 VM 배치가 중단됩니다"
- Enable은 확인 없이 즉시 실행 (복구 작업이므로)

---

### 1.15 HypervisorModule

- **Responsibility**: Hypervisor 리스트/상세 (읽기 전용)
- **Admin-only**: **Yes**

#### Interface

```rust
pub enum HypervisorAction {
    List,
    GetDetail(String),
}

pub enum HypervisorEvent {
    ListLoaded(Vec<Hypervisor>),
    DetailLoaded(Hypervisor),
    ActionFailed { error: String },
}
```

#### Dependencies

| Port trait | Methods |
|-----------|---------|
| `NovaPort` | `list_hypervisors`, `get_hypervisor` |

UI Widgets: `ResourceList`, `DetailView`

#### Data Owned

```rust
pub struct HypervisorModule {
    state: ModuleState<Hypervisor>,
    resource_list: ResourceList,
    detail_view: DetailView,
}
```

#### ViewState 사용

- `List`, `Detail`만 사용 (Create/Edit 없음)
- FormWidget, ConfirmDialog 불필요

#### ResourceList 칼럼 정의

```rust
fn columns() -> Vec<ColumnDef> {
    vec![
        ColumnDef::text("Hostname", 25, |h: &Hypervisor| &h.hypervisor_hostname),
        ColumnDef::text("Type", 10, |h: &Hypervisor| &h.hypervisor_type),
        ColumnDef::computed("vCPU", 12, |h: &Hypervisor|
            format!("{}/{}", h.vcpus_used, h.vcpus)),
        ColumnDef::computed("RAM(GB)", 14, |h: &Hypervisor|
            format!("{}/{}", h.memory_mb_used / 1024, h.memory_mb / 1024)),
        ColumnDef::computed("Disk(GB)", 14, |h: &Hypervisor|
            format!("{}/{}", h.local_gb_used, h.local_gb)),
        ColumnDef::computed("VMs", 5, |h: &Hypervisor| h.running_vms.to_string()),
    ]
}
```

#### DetailView 섹션

- **기본 정보**: ID, Hostname, Hypervisor Type, Hypervisor Version
- **리소스**: vCPU (used/total), RAM (used/total), Disk (used/total), Running VMs
- **서비스**: Host, Service ID, Status, State
- **네트워크**: Host IP, CPU Info (model, vendor, features)

---

### 1.16 UsageModule

- **Responsibility**: 프로젝트별 사용량 조회 + 기간 필터
- **Admin-only**: **Yes**

#### Interface

```rust
pub enum UsageAction {
    LoadUsage { start: NaiveDate, end: NaiveDate },
    LoadProjectUsage { project_id: String, start: NaiveDate, end: NaiveDate },
}

pub enum UsageEvent {
    UsageLoaded(Vec<ProjectUsage>),
    ProjectUsageLoaded { project_id: String, detail: ProjectUsageDetail },
    ActionFailed { error: String },
}

/// 기간 필터 상태
pub struct DateRangeFilter {
    pub start: NaiveDate,
    pub end: NaiveDate,
    pub editing: Option<DateField>,
}

pub enum DateField { Start, End }
```

#### Dependencies

| Port trait | Methods |
|-----------|---------|
| `NovaPort` | `list_usage`, `get_project_usage` |

UI Widgets: `ResourceList`, `FormWidget` (날짜 범위 입력)

#### Data Owned

```rust
pub struct UsageModule {
    state: ModuleState<ProjectUsage>,
    date_filter: DateRangeFilter,
    resource_list: ResourceList,
    form_widget: FormWidget,   // 날짜 입력 폼
}
```

#### ViewState 사용

- `List`만 사용 (Detail/Create/Edit 없음)

#### ResourceList 칼럼 정의

```rust
fn columns() -> Vec<ColumnDef> {
    vec![
        ColumnDef::text("Project", 25, |u: &ProjectUsage| &u.project_name),
        ColumnDef::computed("vCPU Hours", 14, |u: &ProjectUsage|
            format!("{:.1}", u.total_vcpus_usage)),
        ColumnDef::computed("RAM MB*H", 14, |u: &ProjectUsage|
            format!("{:.0}", u.total_memory_mb_usage)),
        ColumnDef::computed("Disk GB*H", 14, |u: &ProjectUsage|
            format!("{:.0}", u.total_local_gb_usage)),
        ColumnDef::computed("Instances", 10, |u: &ProjectUsage|
            u.server_usages.len().to_string()),
    ]
}
```

#### List 뷰 추가 키

```rust
KeyCode::Char('f') => {  // Filter: 날짜 범위 수정
    self.state.form_state = Some(FormState::new(vec![
        FormField::date("start", "Start Date", self.date_filter.start),
        FormField::date("end", "End Date", self.date_filter.end),
    ]));
    None
}
```

#### 특수 로직

- 기본 기간: 현재 월 1일 ~ 오늘
- 날짜 형식: `YYYY-MM-DD` (폼 검증)
- 기간 변경 시 자동 리로드 (폼 제출 -> LoadUsage 액션)
- 완전 읽기 전용: CUD 작업 없음, ConfirmDialog 불필요

---

### 1.x Domain Module Summary

| # | Module | ViewStates Used | Port Dependencies | Admin-only | CUD Actions | 2-Step Confirm |
|---|--------|----------------|-------------------|-----------|-------------|----------------|
| 1 | ServerModule | List, Detail, Create | NovaPort, GlancePort, NeutronPort | No | Create, Delete, Reboot, Start, Stop, Snapshot | Delete, Reboot |
| 2 | MigrationModule | List, Create | NovaPort | **Yes** | LiveMigrate, ColdMigrate, Evacuate, ForceState | **All** |
| 3 | FlavorModule | List, Create | NovaPort | No (CUD=Admin) | Create, Delete | Delete |
| 4 | NetworkModule | List, Detail, Create | NeutronPort | No | Create | - |
| 5 | SecurityGroupModule | List, Detail, Create, Edit | NeutronPort | No | Create, Update, Delete, AddRule, DeleteRule | Delete SG, Delete Rule |
| 6 | FloatingIpModule | List, Create | NeutronPort | No | Create, Delete, Associate, Disassociate | Delete |
| 7 | AgentModule | List | NeutronPort | **Yes** | Enable, Disable, Delete | Delete |
| 8 | VolumeModule | List, Detail, Create | CinderPort, NovaPort | No (some=Admin) | Create, Delete, Extend, Attach, Detach, ForceDelete, ResetState | Delete, ForceDelete, ResetState |
| 9 | SnapshotModule | List, Detail | CinderPort | No | Delete | Delete |
| 10 | ImageModule | List, Detail, Create, Edit | GlancePort | No (CUD=Admin) | Create, Update, Delete | Delete |
| 11 | ProjectModule | List, Detail, Create | KeystonePort, NovaPort, CinderPort, NeutronPort | **Yes** | Create, Delete, UpdateQuota | Delete |
| 12 | UserModule | List, Create | KeystonePort | **Yes** | Create, Delete, GrantRole, RevokeRole | Delete |
| 13 | AggregateModule | List, Detail, Create, Edit | NovaPort | **Yes** | Create, Update, Delete, AddHost, RemoveHost, SetMetadata | Delete, RemoveHost |
| 14 | ComputeServiceModule | List | NovaPort | **Yes** | Enable, Disable | Disable |
| 15 | HypervisorModule | List, Detail | NovaPort | **Yes** | - (read-only) | - |
| 16 | UsageModule | List | NovaPort | **Yes** | - (read-only) | - |

---

## PART 2: NFR Design Patterns

> **WARNING**: NFR 패턴 선택은 운영 환경과 비용에 따라 달라집니다. 기술 담당자와 상의를 권장합니다.

---

### 2.1 Performance

**목표값**: 키 입력 -> 렌더링 < 16ms, API 호출 중 UI 블로킹 0ms, 1,000개 리소스 리스트 렌더링 < 50ms, 메모리 < 50MB

| Pattern | Pros | Cons | Cost Impact |
|---------|------|------|-------------|
| **A. tokio::spawn 완전 비동기** -- 모든 API 호출을 tokio::spawn으로 분리. UI 스레드는 메모리 내 상태만 렌더링. 현 아키텍처 기본 설계. | UI 블로킹 0ms 보장. 구현 단순 (아키텍처 변경 없음). 추가 인프라 불필요. | 채널 통신 오버헤드 (TUI 규모에서 무시 가능). 상태 반영 지연 (최대 200ms 틱 주기). | **낮음** -- 추가 비용 없음 |
| **B. 가상 스크롤 (visible window rendering)** -- 1,000개 아이템 중 화면에 보이는 행만 `Table::new()`에 전달. `scroll_offset + visible_rows` 범위 계산. | 렌더링 복잡도 O(visible_rows) 보장. 10,000개+ 리소스까지 확장 가능. | 전체 리스트 검색 시 별도 인덱스 필요. 스크롤바 위치 계산 추가 로직. | **낮음** -- 코드 복잡도 소폭 증가 |
| **C. 적응형 틱 (adaptive tick)** -- 키 입력 시 즉시 렌더링 (tick 무관), 유휴 시 틱 주기를 200ms -> 500ms로 연장. `last_input: Instant` 기반 판단. | 키 반응 < 16ms 보장. 유휴 시 CPU 사용 최소화 (VDI 친화). | 구현 복잡도 증가. 틱 기반 애니메이션(스피너) 속도가 유휴 시 느려짐. | **낮음** |
| **D. 인크리멘탈 렌더링 (dirty flag)** -- 각 Component에 `dirty: bool` 플래그. 변경 없으면 `terminal.draw()` 스킵. | CPU/전력 절약. 대형 터미널에서 효과. | ratatui의 double-buffer diff가 이미 최적화 수행. dirty flag 효과가 제한적일 수 있음. | **낮음** |
| **E. 백그라운드 프리페치** -- 모듈 전환 예측 시 다음 모듈 데이터 미리 로드. (예: 서버 상세 진입 시 볼륨/네트워크 프리페치) | 체감 로딩 시간 단축. 사용자 대기 없음. | 불필요한 API 호출 증가. 캐시 메모리 사용 증가. API rate limit 위험. | **중간** -- API 호출 비용 |

---

### 2.2 Security

**목표값**: 시크릿 메모리 전용 (로그 출력 금지), TLS 기본 활성화, RBAC 메뉴/액션 필터링, CUD 감사 로그, 고위험 작업 2단계 확인

| Pattern | Pros | Cons | Cost Impact |
|---------|------|------|-------------|
| **A. secrecy::Secret + Zeroize** -- `secrecy::Secret<String>` 크레이트로 토큰/패스워드 래핑. Drop 시 메모리 제로화. `Debug`/`Display` 자동 마스킹 (`[REDACTED]`). | 메모리 덤프 공격 방어. 실수로 로그 출력 원천 차단. 업계 표준 패턴. | `expose_secret()` 명시적 호출 필요 (개발 불편). 추가 크레이트 의존. | **낮음** |
| **B. 단순 String + 로그 정규식 마스킹** -- 패스워드/토큰을 일반 String으로 관리. 로그 출력부에서 `password=***`, `token=***` 정규식 치환. | 구현 단순. 기존 코드 변경 최소. | 메모리 제로화 미보장. 신규 로그 추가 시 마스킹 누락 위험. | **매우 낮음** |
| **C. TLS: rustls (기본)** -- `reqwest`의 `rustls-tls` feature 사용. 정적 링크 가능. `insecure` 옵션 시 `danger_accept_invalid_certs`. | OpenSSL 불필요 (musl 타겟 호환). 정적 바이너리 유지. 메모리 안전. | 사내 CA 인증서 수동 추가 필요 (`--cacert` 옵션). 일부 레거시 TLS 미지원. | **낮음** |
| **D. TLS: native-tls** -- OS 인증서 저장소 활용 (Windows: SChannel, macOS: Security.framework, Linux: OpenSSL). | 사내 CA 자동 인식. 운영팀 인증서 관리 부담 없음. | 정적 바이너리 불가 (동적 링크). cross-compile 복잡도 증가. | **중간** |
| **E. RBAC: 클라이언트 사이드 필터링** -- Keystone 토큰의 `roles` 필드 기반으로 `RbacGuard`가 사이드바 메뉴/액션 키바인딩 필터링. Admin 전용 모듈은 `App::components`에 등록 자체 스킵. | 구현 단순. 서버 부하 없음. API 호출 전 사전 차단. | 클라이언트 우회 가능 (진짜 보안 경계는 API 서버 RBAC). 역할 변경 시 앱 재시작 필요. | **낮음** |
| **F. RBAC: 서버 위임 (403 반응형)** -- 클라이언트에서 필터링 없이 모든 요청 전송. 403 응답 시 "권한 없음" Toast. | 서버 RBAC이 단일 진실 소스. 클라이언트 로직 단순. 역할 변경 즉시 반영. | UX 저하 (보이는데 실패). 불필요한 API 호출. | **낮음** |
| **G. 감사 로그: 구조화 JSON Lines 로컬 파일** -- `~/.config/nexttui/audit.log`에 JSON lines 형식 기록. `AuditLogger`가 Action 채널 구독. 매 기록 즉시 flush. | grep/jq로 분석 가능. 크래시 안전 (즉시 flush). 구현 단순. | 중앙 수집 불가 (Phase 2 대응). 디스크 공간 관리 필요 (log rotation). | **매우 낮음** |
| **H. 감사 로그: syslog 전송** -- 로컬 파일 + syslog (UDP/TCP) 중앙 로그 서버 전송. | 중앙 수집 즉시 가능. SIEM 연동. | 네트워크 의존. syslog 서버 인프라 필요. 구현 복잡도 증가. | **중간** |
| **I. 2단계 확인: 리소스명 재입력** -- `ConfirmDialog`에서 리소스 이름을 정확히 재입력해야 실행. 고위험 작업(삭제/마이그레이션/강제변경)에만 적용. | 실수 방지 효과 높음. GitHub/AWS 삭제 패턴과 동일하여 친숙. | UX 마찰 증가 (반복 작업 시 피로). 긴 리소스명 입력 불편. | **매우 낮음** |
| **J. 2단계 확인: Y/N + 카운트다운** -- Y/N 확인 후 3초 카운트다운 표시. 카운트다운 중 아무 키로 취소 가능. | 재입력보다 빠름. 시각적 피드백 명확. "실수 인지 시간" 확보. | 리소스명 재입력 대비 실수 방지 효과 낮음. 3초 강제 대기. | **매우 낮음** |

---

### 2.3 Availability

**목표값**: API 실패 시 캐시 데이터 표시 + 에러 알림, 인증 토큰 만료 시 자동 갱신 -> 실패 시 재인증 프롬프트, 네트워크 단절 시 크래시 없이 상태 표시

| Pattern | Pros | Cons | Cost Impact |
|---------|------|------|-------------|
| **A. Stale-While-Revalidate 캐시 폴백** -- API 실패 시 만료된 캐시 데이터를 `[stale]` 배지와 함께 표시. 백그라운드에서 주기적 재시도. | 네트워크 단절에도 기존 데이터 조회 가능. UX 끊김 없음. | stale 데이터로 잘못된 운영 판단 위험 (예: 삭제된 서버가 여전히 표시). 캐시 무효화 시점 판단 어려움. | **낮음** |
| **B. 에러 표시 + 수동 재시도** -- API 실패 시 에러 메시지 표시 + 빈 리스트. `r` 키로 수동 재시도. | 구현 가장 단순. stale 데이터 혼동 없음. 항상 진실된 상태 표시. | UX 저하 (빈 화면). 네트워크 복구 전 기능 사용 불가. | **매우 낮음** |
| **C. Exponential Backoff 자동 재시도** -- API 실패 시 1s -> 2s -> 4s 간격 최대 3회 재시도. 재시도 중 스피너 + "Retrying (2/3)..." 표시. | 일시적 네트워크 문제 자동 복구. 사용자 개입 불필요. | 영구 장애 시 최대 7초 불필요 대기. 재시도 중 다른 작업 가능 여부 결정 필요. | **낮음** |
| **D. 토큰 선제 갱신 (proactive)** -- 토큰 만료 5분 전에 백그라운드 갱신. `broadcast` 채널로 모든 adapter에 새 토큰 전파. | 401 에러 사전 방지. 사용자 인지 없이 연속 사용. | 시스템 시계 오차 시 오작동. 갱신 실패 시 폴백 로직 필요. | **낮음** |
| **E. 토큰 반응적 갱신 (reactive)** -- API 호출 결과 401 수신 시 토큰 갱신 후 원래 요청 1회 재시도. `Mutex`로 동시 갱신 1회 제한. | 구현 단순. 시계 의존 없음. 확실한 만료 감지. | 첫 401에서 1회 지연 (갱신 + 재시도). 동시 다발 요청 시 직렬화 병목. | **낮음** |
| **F. 연결 상태 모니터: 능동 하트비트** -- 30초마다 Keystone `/` 에 HEAD 요청. 실패 시 StatusBar에 `[DISCONNECTED]` 표시. 복구 시 자동 전환. | 네트워크 상태 사전 인지. 운영자가 CUD 자제 가능. 시각적 피드백. | 추가 API 호출 (30초당 1회). 하트비트 성공해도 다른 서비스 장애 가능. | **낮음** |
| **G. 연결 상태 모니터: 수동 감지** -- 하트비트 없이 실제 API 호출 결과로 판단. 마지막 성공/실패 시각을 StatusBar에 표시. | 추가 네트워크 부하 0. 실제 사용 패턴 반영. | 유휴 시 연결 상태 불명. 장애 감지 지연 (다음 조작까지). | **매우 낮음** |

---

### 2.4 Data Integrity

**목표값**: 커맨드 히스토리 로컬 저장 (최대 50개), CUD 감사 로그 로컬 저장, 캐시 휘발성, CUD 전 확인 다이얼로그

| Pattern | Pros | Cons | Cost Impact |
|---------|------|------|-------------|
| **A. 즉시 fsync (write-through)** -- 커맨드 히스토리/감사 로그 매 기록 시 `File::sync_all()` 호출. 크래시 시 데이터 손실 0건 보장. | 크래시 안전 최대. 감사 로그 무결성 보장. 규제 환경 대응. | 매 기록마다 디스크 I/O (TUI 빈도에서는 무시 가능, 초당 수 회 이하). | **매우 낮음** |
| **B. 버퍼링 + 주기적 flush** -- `BufWriter` 사용, 10초 또는 10건마다 flush. | 디스크 I/O 최소화. 대량 기록 시 효율적. | 크래시 시 최대 10초/10건 손실 가능. 감사 로그 누락 위험. | **매우 낮음** |
| **C. 히스토리: 앱 종료 시 1회 저장** -- `should_quit` 시점에 한 번만 파일 저장. | 구현 극도로 단순. 디스크 I/O 최소. | 비정상 종료(Ctrl+C, SIGKILL, 크래시) 시 전체 세션 히스토리 손실. 감사 로그에는 부적합. | **매우 낮음** |
| **D. 캐시: 리소스별 차등 TTL** -- 변경 빈도 기반 TTL (서버 2분, 플레이버 10분). CUD 작업 시 해당 리소스 캐시 즉시 무효화. | API 호출 절감. CUD 후 즉시 반영. 리소스 특성 반영. | TTL 기준이 경험적/주관적. 외부 변경(다른 사용자 CUD) 반영 지연. | **매우 낮음** |
| **E. 캐시: 없음 (항상 API 호출)** -- 모든 조회 시 API 직접 호출. | 항상 최신 데이터. 구현 가장 단순. 캐시 무효화 버그 없음. | API 부하 증가. 모듈 전환마다 로딩 대기. rate limit 위험. | **낮음** |
| **F. 확인 다이얼로그: 위험도 기반 3단계** -- Low(Y/N만, e.g. Disassociate), Medium(Y/N + 리소스명 표시, e.g. Delete), High(리소스명 재입력, e.g. Migration/ForceDelete). 각 Action에 `RiskLevel` 태깅. | 위험도에 비례한 UX 마찰. 저위험 작업 생산성 유지. | 위험도 분류 합의 필요. 분류 오류 시 위험 작업이 쉽게 통과. | **매우 낮음** |
| **G. 확인 다이얼로그: 모든 CUD에 재입력** -- CUD 작업 전부 리소스명 재입력 요구. | 안전성 최대. 실수 가능성 최소. 일관된 UX. | UX 마찰 극대화. 대량 작업(다수 리소스 삭제) 시 생산성 심각 저하. | **매우 낮음** |

---

### 2.5 Deployment

**목표값**: 단일 정적 바이너리 (musl), macOS + Linux + Windows 크로스 플랫폼, VDI 관리망 내부 실행, 런타임 의존성 없음

| Pattern | Pros | Cons | Cost Impact |
|---------|------|------|-------------|
| **A. musl 정적 링크 (Linux)** -- `x86_64-unknown-linux-musl` + `aarch64-unknown-linux-musl` 타겟. `cross` 크레이트 사용. 단일 바이너리, glibc 의존 없음. | 모든 Linux 배포판 즉시 실행. 설치 = 파일 복사, 제거 = 파일 삭제. VDI 최적. | musl DNS resolver 제한 (NSS 미지원, `/etc/resolv.conf` 기반만). TLS는 rustls 필수. 일부 성능 차이 (musl malloc). | **낮음** |
| **B. glibc 동적 링크 (Linux)** -- `x86_64-unknown-linux-gnu` 타겟. glibc 2.17+ 호환 빌드 (manylinux 기준). | 빌드 단순. DNS/NSS 완전 지원. malloc 성능 최적. | glibc 버전 호환 이슈 (오래된 VDI). 동적 라이브러리 의존. | **매우 낮음** |
| **C. macOS: Universal Binary** -- `lipo`로 aarch64 + x86_64 fat binary 생성. | M-series + Intel Mac 단일 바이너리. 배포 단순. | 바이너리 크기 2배. 빌드 시간 2배. | **낮음** |
| **D. macOS: 아키텍처별 개별 배포** -- `aarch64-apple-darwin`, `x86_64-apple-darwin` 각각 빌드 배포. | 바이너리 크기 최소. 빌드 단순. | 배포 시 아키텍처 구분 필요. 사용자 혼동 가능. | **매우 낮음** |
| **E. Windows: MSVC 타겟** -- `x86_64-pc-windows-msvc`. Visual Studio Build Tools 기반. | Windows 네이티브. Windows Terminal + ConPTY 최적 호환. MSVC 최적화. | 크로스 컴파일 불가 (Windows CI runner 필요). MSVC 재배포 패키지 가능성. | **중간** |
| **F. Windows: GNU 타겟** -- `x86_64-pc-windows-gnu`. MinGW 기반, Linux CI에서 크로스 컴파일 가능. | Linux CI에서 Windows 빌드 가능. MSVC 불필요. | 일부 Windows API 호환 이슈. crossterm Windows 터미널 지원 차이. 디버깅 어려움. | **낮음** |
| **G. CI/CD: GitHub Actions 매트릭스 빌드** -- `matrix: {os: [ubuntu, macos, windows], target: [...]}` 6개 타겟 병렬 빌드. 릴리즈 태그 시 자동 바이너리 게시. | 모든 플랫폼 네이티브 빌드/테스트. 재현 가능. 자동 릴리즈. | 빌드 시간 (6 타겟 x ~10분). private repo CI 비용. macOS/Windows runner 비용 높음. | **중간** |
| **H. CI/CD: cross 크레이트 단일 Linux CI** -- 모든 타겟을 `cross`로 Linux CI에서 크로스 컴파일. | CI runner 1종류. 비용 최소. 설정 단순. | macOS/Windows 네이티브 테스트 불가 (크로스 컴파일만). 일부 타겟 빌드 실패 위험. | **낮음** |
| **I. 배포: 사내 Artifact 저장소** -- Nexus/Artifactory에 바이너리 업로드. VDI에서 CLI/스크립트로 다운로드. 버전 관리. | 관리망 보안 정책 준수. 버전 관리/롤백 가능. 무결성 검증 (checksum). | 내부 저장소 인프라 필요. 배포 자동화 추가 개발. | **중간** |
| **J. 배포: 직접 파일 복사 (공유 폴더/scp)** -- VDI 공유 폴더에 바이너리 배치. 운영자가 복사하여 사용. | 인프라 불필요. 즉시 시작 가능. 설정 0. | 버전 관리 수동. 구버전 잔존 위험. 무결성 검증 없음. 배포 추적 불가. | **매우 낮음** |

---

## Appendix: Action / AppEvent Enum

전체 시스템의 Action/Event 라우팅 구조. `ActionDispatcher`가 수신하여 백그라운드 작업을 spawn하고, 결과를 `AppEvent`로 반환한다.

```rust
/// UI -> Background: 사용자 의도
pub enum Action {
    // --- Global ---
    Navigate(Route),
    Notify { message: String, level: NotifyLevel },
    Quit,
    RefreshCurrent,

    // --- Domain ---
    Server(ServerAction),
    Migration(MigrationAction),
    Flavor(FlavorAction),
    Network(NetworkAction),
    SecurityGroup(SecurityGroupAction),
    FloatingIp(FloatingIpAction),
    Agent(AgentAction),
    Volume(VolumeAction),
    Snapshot(SnapshotAction),
    Image(ImageAction),
    Project(ProjectAction),
    User(UserAction),
    Aggregate(AggregateAction),
    ComputeService(ComputeServiceAction),
    Hypervisor(HypervisorAction),
    Usage(UsageAction),
}

/// Background -> UI: 작업 결과
pub enum AppEvent {
    // --- Generic ---
    ResourceLoaded { resource_type: ResourceType, data: Box<dyn Any + Send> },
    OperationCompleted { resource_type: ResourceType, id: String, action: String },
    ApiError { resource_type: ResourceType, id: Option<String>, error: String },
    TokenRefreshed,
    TokenExpired,
    ConnectionStatusChanged(bool),

    // --- Domain-specific ---
    Server(ServerEvent),
    Migration(MigrationEvent),
    Flavor(FlavorEvent),
    Network(NetworkEvent),
    SecurityGroup(SecurityGroupEvent),
    FloatingIp(FloatingIpEvent),
    Agent(AgentEvent),
    Volume(VolumeEvent),
    Snapshot(SnapshotEvent),
    Image(ImageEvent),
    Project(ProjectEvent),
    User(UserEvent),
    Aggregate(AggregateEvent),
    ComputeService(ComputeServiceEvent),
    Hypervisor(HypervisorEvent),
    Usage(UsageEvent),
}

/// 라우팅
pub enum Route {
    Servers,
    ServerDetail(String),
    Migration,
    Flavors,
    Networks,
    NetworkDetail(String),
    SecurityGroups,
    SecurityGroupDetail(String),
    FloatingIps,
    Agents,
    Volumes,
    VolumeDetail(String),
    Snapshots,
    SnapshotDetail(String),
    Images,
    ImageDetail(String),
    Projects,
    ProjectDetail(String),
    Users,
    Aggregates,
    AggregateDetail(String),
    ComputeServices,
    Hypervisors,
    HypervisorDetail(String),
    Usage,
}

/// 리소스 타입 (캐시 키, 이벤트 라우팅, 감사 로그 분류)
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ResourceType {
    Server,
    Flavor,
    Network,
    SecurityGroup,
    FloatingIp,
    Agent,
    Volume,
    Snapshot,
    Image,
    Project,
    User,
    Aggregate,
    ComputeService,
    Hypervisor,
    Usage,
}
```
