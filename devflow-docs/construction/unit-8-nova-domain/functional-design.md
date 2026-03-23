# Unit 8: nova-domain — Functional Design

**Timestamp**: 2026-03-24
**Components**: NovaHttpAdapter, ServerModule, FlavorModule
**Stories**: US-010~014

---

## 1. NovaHttpAdapter

`src/adapter/http/nova.rs` — `impl NovaPort for NovaHttpAdapter`

이 unit에서는 Server + Flavor 관련 메서드만 구현. Migration/Aggregate/ComputeService/Hypervisor/Usage/Quota는 Unit 13~14에서 stub → 실구현.

### 1.1 구조

```rust
pub struct NovaHttpAdapter {
    base: BaseHttpClient,  // service_type="compute", interface=Internal
}
```

### 1.2 Server API 매핑

| Method | HTTP | Path | Request Body | Response |
|--------|------|------|-------------|----------|
| `list_servers` | GET | `/servers/detail?{query}` | — | `{"servers": [...], "servers_links": [...]}` |
| `get_server` | GET | `/servers/{id}` | — | `{"server": {...}}` |
| `create_server` | POST | `/servers` | `{"server": {...}}` | `{"server": {...}}` |
| `delete_server` | DELETE | `/servers/{id}` | — | 204 |
| `reboot_server` | POST | `/servers/{id}/action` | `{"reboot": {"type": "SOFT"|"HARD"}}` | 202 |
| `start_server` | POST | `/servers/{id}/action` | `{"os-start": null}` | 202 |
| `stop_server` | POST | `/servers/{id}/action` | `{"os-stop": null}` | 202 |
| `create_server_snapshot` | POST | `/servers/{id}/action` | `{"createImage": {"name": "..."}}` | Location header → image_id |
| `list_server_events` | GET | `/servers/{id}/os-instance-actions` | — | `{"instanceActions": [...]}` |

**Query 빌드 (`list_servers`)**: `ServerListFilter` 필드를 query param으로 변환 + `PaginationParams` 추가.

### 1.3 Flavor API 매핑

| Method | HTTP | Path | Request Body | Response |
|--------|------|------|-------------|----------|
| `list_flavors` | GET | `/flavors/detail?{pagination}` | — | `{"flavors": [...], "flavors_links": [...]}` |
| `get_flavor` | GET | `/flavors/{id}` | — | `{"flavor": {...}}` |
| `create_flavor` | POST | `/flavors` | `{"flavor": {...}}` | `{"flavor": {...}}` |
| `delete_flavor` | DELETE | `/flavors/{id}` | — | 204 |

### 1.4 JSON Wrapper Structs (serde, 비공개)

```rust
#[derive(Deserialize)]
struct NovaServersResponse {
    servers: Vec<Server>,
    servers_links: Option<Vec<Link>>,
}

#[derive(Deserialize)]
struct NovaServerWrapper {
    server: Server,
}

#[derive(Deserialize)]
struct NovaFlavorsResponse {
    flavors: Vec<Flavor>,
    flavors_links: Option<Vec<Link>>,
}

#[derive(Deserialize)]
struct NovaFlavorWrapper {
    flavor: Flavor,
}

#[derive(Deserialize)]
struct NovaInstanceActionsResponse {
    #[serde(rename = "instanceActions")]
    instance_actions: Vec<ServerEvent>,
}

#[derive(Deserialize)]
struct Link {
    rel: String,
    href: String,
}

#[derive(Serialize)]
struct NovaServerCreateBody {
    server: NovaServerCreateInner,
}

#[derive(Serialize)]
struct NovaServerCreateInner {
    name: String,
    #[serde(rename = "imageRef")]
    image_ref: String,
    #[serde(rename = "flavorRef")]
    flavor_ref: String,
    networks: Vec<NovaNetworkAttachment>,
    #[serde(skip_serializing_if = "Option::is_none")]
    security_groups: Option<Vec<NovaSecurityGroupRef>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    key_name: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    availability_zone: Option<String>,
}

#[derive(Serialize)]
struct NovaFlavorCreateBody {
    flavor: NovaFlavorCreateInner,
}

#[derive(Serialize)]
struct NovaFlavorCreateInner {
    name: String,
    vcpus: u32,
    ram: u32,
    disk: u32,
    #[serde(rename = "os-flavor-access:is_public")]
    is_public: bool,
}
```

### 1.5 Pagination Helper

```rust
fn build_server_query(filter: &ServerListFilter, pagination: &PaginationParams) -> String
fn build_pagination_query(pagination: &PaginationParams) -> String
fn extract_next_marker(links: &[Link]) -> Option<String>
```

### 1.6 Stub Methods (Unit 13~14)

Migration/Aggregate/ComputeService/Hypervisor/Usage/Quota 메서드들은 이 unit에서 `todo!()` 대신 `Err(ApiError::BadRequest("not yet implemented"))` 반환. Unit 13~14에서 실구현으로 교체.

---

## 2. ServerModule

`src/module/server/mod.rs` + `src/module/server/view_model.rs`

### 2.1 ViewState 사용

- `List` — 서버 목록 (ResourceList)
- `Detail(String)` — 서버 상세 (DetailView)
- `Create` — 서버 생성 폼 (FormWidget)

### 2.2 State

```rust
pub struct ServerModule {
    view_state: ViewState,
    servers: Vec<Server>,
    selected_index: usize,
    loading: bool,
    error_message: Option<String>,
    // 생성 폼 참조 데이터
    available_flavors: Vec<Flavor>,
    available_images: Vec<Image>,
    available_networks: Vec<Network>,
    available_security_groups: Vec<SecurityGroup>,
    // 위젯
    resource_list: ResourceList,
    detail_view: DetailView,
    form_widget: Option<FormWidget>,
    confirm_state: Option<ConfirmState>,
    // 채널
    action_tx: mpsc::UnboundedSender<Action>,
}
```

### 2.3 ViewState Enum

```rust
#[derive(Debug, Clone, PartialEq)]
pub enum ViewState {
    List,
    Detail(String),
    Create,
}
```

### 2.4 Component 구현

**handle_key**:
1. `confirm_state` 있으면 ConfirmDialog에 위임
2. ViewState 분기:
   - `List`: j/k/g/G 네비게이션, Enter→Detail, c→Create, d→Delete(ConfirmDialog), r→Refresh
   - `Detail`: Esc→List, R→HardReboot(Confirm), S→Start, X→Stop(Confirm)
   - `Create`: Tab/BackTab 필드 이동, Enter→Submit(Confirm), Esc→Cancel

**handle_event**:
- `ServersLoaded(Vec<Server>)` → 목록 업데이트, loading=false
- `ServerDeleted/Rebooted/Started/Stopped` → 목록 리프레시 트리거
- `ApiError` → error_message 설정

**render**:
- ViewState 분기 → ResourceList / DetailView / FormWidget 렌더
- Loading overlay, ConfirmDialog overlay

### 2.5 view_model.rs

```rust
pub fn server_columns() -> Vec<ColumnDef>
// columns: [Status Icon(3), Name(25%), Status(10), IP(20%), Flavor(15%), Image(15%)]

pub fn server_to_row(server: &Server) -> Row
// cells: [status_icon, name, status, format_ips, flavor_name, image_name]
// style_hint: status에 따라 Active/Error/Warning/Disabled 매핑

pub fn server_detail_data(server: &Server) -> DetailData
// sections: 기본정보, 하드웨어(Flavor), 네트워크(NestedTable), 볼륨(NestedTable)

pub fn server_create_form() -> Vec<FormField>
// fields: Name(Text,required), Image(Dropdown), Flavor(Dropdown),
//         Network(Dropdown), SecurityGroup(Dropdown), KeyPair(Dropdown,optional), AZ(Text,optional)

pub fn status_to_style_hint(status: &str) -> RowStyleHint
// ACTIVE→Active, ERROR/DELETED→Error, BUILD/RESIZE/REBOOT→Warning, SHUTOFF/SUSPENDED→Disabled, _→Normal

pub fn format_ips(addresses: &HashMap<String, Vec<Address>>) -> String
// "10.0.0.5, 192.168.1.100" (fixed first, then floating)
```

### 2.6 Action/Event 매핑

| 사용자 액션 | Action enum variant | 비고 |
|------------|-------------------|------|
| 서버 목록 로드 | `FetchServers` | 기존 variant 사용 |
| 서버 삭제 | `DeleteServer { id, name }` | ConfirmDialog 후 |
| 서버 리부트 | `RebootServer { id, hard }` | ConfirmDialog 후 |
| 서버 시작 | `StartServer { id }` | 즉시 |
| 서버 중지 | `StopServer { id }` | 즉시 |
| 서버 생성 | `CreateServer(ServerCreateParams)` — 신규 추가 | ConfirmDialog 후 |
| 서버 스냅샷 | `CreateServerSnapshot { server_id, name }` — 신규 추가 | Detail 뷰에서 |

**AppEvent 신규 variant 필요**:
- `ServerCreated(Server)` — 생성 완료 후 목록 리프레시

---

## 3. FlavorModule

`src/module/flavor/mod.rs` + `src/module/flavor/view_model.rs`

### 3.1 ViewState 사용

- `List` — 플레이버 목록 (ResourceList)
- `Create` — Admin 전용 생성 폼

Detail 미사용 (플레이버는 immutable, 리스트 칼럼에 충분한 정보).

### 3.2 State

```rust
pub struct FlavorModule {
    view_state: ViewState,
    flavors: Vec<Flavor>,
    selected_index: usize,
    loading: bool,
    error_message: Option<String>,
    is_admin: bool,
    resource_list: ResourceList,
    form_widget: Option<FormWidget>,
    confirm_state: Option<ConfirmState>,
    action_tx: mpsc::UnboundedSender<Action>,
}
```

### 3.3 Component 구현

**handle_key**:
- `List`: j/k/g/G, c→Create(Admin only), d→Delete(Admin only, Confirm), r→Refresh
  - Admin이 아닌 경우 c/d 무시
- `Create`: Tab/BackTab, Enter→Submit, Esc→Cancel

**handle_event**:
- `FlavorsLoaded(Vec<Flavor>)` → 업데이트
- `ApiError` → error_message

**render**: List / Create form

### 3.4 view_model.rs

```rust
pub fn flavor_columns() -> Vec<ColumnDef>
// columns: [Name(25%), vCPU(8), RAM(10), Disk(10), Public(8)]

pub fn flavor_to_row(flavor: &Flavor) -> Row
// cells: [name, vcpus, format_ram(ram), disk, public_icon]

pub fn flavor_create_form() -> Vec<FormField>
// fields: Name(Text,required), vCPU(Text,required), RAM MB(Text,required),
//         Disk GB(Text,required), Public(Checkbox)
```

### 3.5 Action/Event 매핑

| 사용자 액션 | Action enum variant |
|------------|-------------------|
| 플레이버 목록 | `FetchFlavors` (기존) |
| 플레이버 생성 | `CreateFlavor(FlavorCreateParams)` — 신규 추가 |
| 플레이버 삭제 | `DeleteFlavor { id }` — 신규 추가 |

**AppEvent 신규 variant 필요**:
- `FlavorCreated(Flavor)`
- `FlavorDeleted { id: String }`

---

## 4. 파일 구조

```
src/
├── adapter/http/
│   ├── nova.rs          # NEW: NovaHttpAdapter impl NovaPort
│   └── mod.rs           # nova 모듈 추가
├── module/
│   ├── mod.rs           # NEW: module 모듈 선언
│   ├── server/
│   │   ├── mod.rs       # ServerModule impl Component
│   │   └── view_model.rs
│   └── flavor/
│       ├── mod.rs       # FlavorModule impl Component
│       └── view_model.rs
├── action.rs            # CreateServer, CreateServerSnapshot, CreateFlavor, DeleteFlavor 추가
├── event.rs             # ServerCreated, FlavorCreated, FlavorDeleted 추가
└── lib.rs               # `pub mod module;` 추가
```

---

## 5. 테스트 전략

### 5.1 NovaHttpAdapter (Unit Tests)

실제 HTTP 호출 불가 → JSON 파싱 + query 빌드 로직만 단위 테스트.

| 테스트 | 검증 |
|--------|------|
| `test_build_server_query_full` | 모든 filter 필드 + pagination → query string |
| `test_build_server_query_empty` | 빈 필터 → 빈 query |
| `test_build_pagination_query` | marker + limit + sort |
| `test_extract_next_marker` | links에서 next marker 추출 |
| `test_extract_next_marker_none` | next 없음 → None |
| `test_nova_server_create_body_serialize` | ServerCreateParams → JSON body 구조 |
| `test_nova_flavor_create_body_serialize` | FlavorCreateParams → JSON body 구조 |
| `test_server_events_deserialize` | instanceActions JSON → Vec<ServerEvent> |

### 5.2 ServerModule (Unit Tests)

MockNovaAdapter 사용 (Port trait mock).

| 테스트 | 검증 |
|--------|------|
| `test_initial_state_is_list` | 초기 view_state == List |
| `test_handle_key_j_k_navigation` | j/k로 selected_index 이동 |
| `test_handle_key_enter_to_detail` | Enter → ViewState::Detail(id) + FetchServers |
| `test_handle_key_esc_detail_to_list` | Detail에서 Esc → List |
| `test_handle_key_c_opens_create` | c → ViewState::Create |
| `test_handle_key_d_opens_confirm` | d → confirm_state 활성화 |
| `test_handle_event_servers_loaded` | ServersLoaded → servers 업데이트 |
| `test_handle_event_server_deleted` | ServerDeleted → 리프레시 |
| `test_handle_event_api_error` | ApiError → error_message 설정 |
| `test_create_form_fields` | server_create_form() 필드 수 + 타입 확인 |

### 5.3 FlavorModule (Unit Tests)

| 테스트 | 검증 |
|--------|------|
| `test_initial_state_is_list` | 초기 상태 |
| `test_handle_key_navigation` | j/k |
| `test_handle_key_c_admin_only` | is_admin=true → Create, false → 무시 |
| `test_handle_key_d_admin_only` | is_admin=true → Confirm, false → 무시 |
| `test_handle_event_flavors_loaded` | FlavorsLoaded → 업데이트 |

### 5.4 ViewModel Tests

| 테스트 | 검증 |
|--------|------|
| `test_server_columns_count` | 6 columns |
| `test_server_to_row_active` | ACTIVE → RowStyleHint::Active |
| `test_server_to_row_error` | ERROR → RowStyleHint::Error |
| `test_format_ips_fixed_and_floating` | fixed + floating IP 포맷 |
| `test_format_ips_empty` | 빈 addresses → "" |
| `test_flavor_columns_count` | 5 columns |
| `test_flavor_to_row` | 값 변환 확인 |
| `test_status_to_style_hint_mapping` | 모든 상태 매핑 |

**예상 테스트 수: ~27개**

---

## 6. 의존성 요약

| 의존 대상 | 사용 |
|-----------|------|
| `port::nova::NovaPort` | ServerModule, FlavorModule이 trait object로 사용 |
| `port::types::*` | ServerListFilter, PaginationParams, ServerCreateParams 등 |
| `models::nova::*` | Server, Flavor, Address 등 domain models |
| `adapter::http::base::BaseHttpClient` | NovaHttpAdapter 내부 |
| `ui::resource_list::*` | ColumnDef, Row, RowStyleHint, ResourceList |
| `ui::detail_view::*` | DetailData, DetailSection, DetailField, DetailView |
| `ui::form::*` | FormField, FormWidget, FormAction |
| `ui::confirm::*` | ConfirmDialog (ConfirmState) |
| `action::Action` | variant 추가 |
| `event::AppEvent` | variant 추가 |
| `component::Component` | trait impl |
