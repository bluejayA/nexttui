# Functional Design: Unit 2 — core-runtime

**Timestamp**: 2026-03-23T15:30:00+09:00
**Unit**: core-runtime
**Stories**: US-008 (논블로킹 API), US-009 (백그라운드 알림)
**Components**: App, EventLoop, Router, ActionDispatcher, BackgroundTracker, Component trait

---

## Step 1: Domain Entities

### 1.1 Component trait

```rust
pub trait Component {
    fn handle_key(&mut self, key: KeyEvent) -> Option<Action>;
    fn handle_event(&mut self, event: &AppEvent);
    fn render(&self, frame: &mut Frame, area: Rect);
}
```

**Invariants**:
- `handle_key`는 UI 블로킹 없이 즉시 반환 (async 아님)
- `Option<Action>` 반환: Some이면 action_tx로 전송, None이면 로컬 상태 변경만
- `handle_event`는 자신에게 해당하는 이벤트만 처리 (App이 라우팅)

### 1.2 InputMode

```rust
pub enum InputMode {
    Normal,     // 기본 — 키 바인딩 활성
    Command,    // ':' prefix — CommandParser 활성
    Search,     // '/' prefix — SearchFilter 활성
    Form,       // FormWidget 포커스
    Confirm,    // ConfirmDialog 모달 활성
}
```

**Invariants**:
- Normal에서만 컴포넌트에 키 이벤트 전달
- Command/Search/Form/Confirm에서는 입력 위젯이 키 이벤트 소비
- Esc는 모든 모드에서 Normal로 복귀 (Form 제외 — Form은 취소 확인)

### 1.3 App

```rust
pub struct App {
    pub should_quit: bool,
    pub input_mode: InputMode,
    pub sidebar_visible: bool,

    router: Router,
    components: HashMap<Route, Box<dyn Component>>,
    background_tracker: BackgroundTracker,
    action_tx: mpsc::UnboundedSender<Action>,

    // Shared (Arc) — Unit 4~5에서 주입
    config: Arc<Config>,
}
```

**Invariants**:
- `should_quit`이 true이면 EventLoop가 다음 반복에서 종료
- `router.current()`에 해당하는 컴포넌트가 `components`에 반드시 존재
- `action_tx`는 항상 유효 (receiver가 살아있는 동안)

### 1.4 Router

```rust
pub struct Router {
    current: Route,
    history: Vec<Route>,  // max 20, Esc back-navigation
}
```

**Invariants**:
- `history` 크기 최대 20 (초과 시 oldest 제거)
- `navigate`는 현재를 history에 push하고 새 route로 이동
- `back`은 history pop → 비어있으면 None
- `reset`은 history 비우고 새 route로 이동 (클라우드 전환 시)

### 1.5 Action enum (UI → Background)

```rust
pub enum Action {
    // Navigation
    Navigate(Route),
    Back,

    // Nova
    FetchServers,
    DeleteServer { id: String, name: String },
    RebootServer { id: String, hard: bool },
    StartServer { id: String },
    StopServer { id: String },
    CreateServer(Box<CreateServerRequest>),
    LiveMigrate { id: String, host: Option<String> },
    Evacuate { id: String, host: Option<String> },
    FetchFlavors,
    FetchAggregates,
    FetchComputeServices,
    FetchHypervisors,

    // Neutron
    FetchNetworks,
    FetchSecurityGroups,
    FetchFloatingIps,
    CreateFloatingIp { network_id: String },
    DeleteFloatingIp { id: String },
    FetchAgents,

    // Cinder
    FetchVolumes,
    FetchSnapshots,
    DeleteVolume { id: String, force: bool },
    ExtendVolume { id: String, new_size: u32 },

    // Glance
    FetchImages,
    DeleteImage { id: String },

    // Keystone Admin
    FetchProjects,
    FetchUsers,

    // System
    RefreshAll,
    SwitchCloud(String),
    Quit,
}
```

### 1.6 AppEvent enum (Background → UI)

```rust
pub enum AppEvent {
    // Data loaded
    ServersLoaded(Vec<Server>),
    FlavorsLoaded(Vec<Flavor>),
    NetworksLoaded(Vec<Network>),
    SecurityGroupsLoaded(Vec<SecurityGroup>),
    FloatingIpsLoaded(Vec<FloatingIp>),
    VolumesLoaded(Vec<Volume>),
    SnapshotsLoaded(Vec<VolumeSnapshot>),
    ImagesLoaded(Vec<Image>),
    ProjectsLoaded(Vec<Project>),
    UsersLoaded(Vec<User>),
    AggregatesLoaded(Vec<Aggregate>),
    ComputeServicesLoaded(Vec<ComputeService>),
    HypervisorsLoaded(Vec<Hypervisor>),
    AgentsLoaded(Vec<NetworkAgent>),

    // CUD results
    ServerDeleted { id: String, name: String },
    ServerRebooted { id: String },
    ServerStarted { id: String },
    ServerStopped { id: String },
    VolumeDeleted { id: String },
    ImageDeleted { id: String },
    FloatingIpCreated(FloatingIp),
    FloatingIpDeleted { id: String },

    // Error
    ApiError { operation: String, message: String },

    // Auth
    TokenRefreshed,
    AuthFailed(String),

    // System
    CloudSwitched(String),
}
```

### 1.7 BackgroundTracker

```rust
pub struct BackgroundTracker {
    operations: HashMap<String, OperationInfo>,
    toasts: Vec<Toast>,
}

pub struct OperationInfo {
    pub description: String,
    pub started_at: Instant,
    pub status: OperationStatus,
}

pub enum OperationStatus {
    InProgress,
    Completed,
    Failed(String),
}

pub struct Toast {
    pub message: String,
    pub level: ToastLevel,
    pub created_at: Instant,
    pub ttl: Duration,
}

pub enum ToastLevel {
    Success,
    Error,
    Info,
}
```

**Invariants**:
- Toast TTL: Success 5초, Error 10초, Info 5초
- 완료/실패 후 60초 경과한 operation은 GC
- `poll_updates`는 on_tick에서 호출 (200ms마다)

### 1.8 Entity Relationships

```
App 1──1 Router
App 1──1 BackgroundTracker
App 1──N Component (via HashMap<Route, Box<dyn Component>>)
App 1──1 action_tx (mpsc sender)

EventLoop ──calls──> App (handle_key, handle_event, on_tick, render)
EventLoop ──reads──> event_rx (mpsc receiver)
EventLoop ──reads──> crossterm EventStream

ActionDispatcher ──reads──> action_rx
ActionDispatcher ──sends──> event_tx
```

---

## Step 2: Business Rules

### BR-01: 글로벌 키 핸들링 (App.handle_key)
- **조건**: Normal 모드에서 키 입력
- **동작**:
  - `:` → InputMode::Command 전환
  - `/` → InputMode::Search 전환
  - `Tab` → sidebar_visible 토글
  - `q` → Action::Quit (확인 다이얼로그 없이 즉시)
  - 그 외 → active component에 위임
- **우선순위**: 글로벌 키 > 컴포넌트 키

### BR-02: InputMode 전환 규칙
- **조건**: 키 입력 시
- **동작**:
  - `Esc` in Command/Search → Normal
  - `Esc` in Confirm → Normal (취소)
  - `Esc` in Form → 확인 다이얼로그 표시 ("취소하시겠습니까?")
  - `Enter` in Command → 커맨드 실행 후 Normal
  - `Enter` in Search → 필터 확정, Normal
- **예외**: 이미 Normal이면 Esc는 Router.back()

### BR-03: Router 네비게이션
- **조건**: Navigate(route) 액션 수신
- **동작**: `router.navigate(route)` — 현재 route를 history push
- **예외**: 이미 같은 route면 무시 (중복 push 방지)

### BR-04: Router back
- **조건**: Esc in Normal mode
- **동작**: `router.back()` → 이전 route로 이동
- **예외**: history 비어있으면 무시 (최상위에서 Esc는 아무 동작 없음)

### BR-05: Router history 크기 제한
- **조건**: navigate 시 history 크기 > 20
- **동작**: oldest 엔트리 제거 (VecDeque pop_front)

### BR-06: tokio::select! 루프 우선순위
- **조건**: 여러 소스에 동시 이벤트
- **동작**: tokio::select!는 무작위 우선순위 (기본). 모든 branch 공정 처리
- **예외**: should_quit 체크는 select! 이후 (매 반복 끝에)

### BR-07: BackgroundTracker poll_updates
- **조건**: on_tick 호출 시 (200ms마다)
- **동작**: tracker_rx에서 모든 대기 이벤트를 drain
  - Started → operations에 InProgress 추가
  - Completed → status 변경 + Toast(Success) 추가
  - Failed → status 변경 + Toast(Error) 추가

### BR-08: Toast TTL 만료
- **조건**: on_tick 호출 시
- **동작**: `created_at + ttl < now`인 Toast 제거

### BR-09: 렌더링 조건
- **조건**: select! branch 처리 후
- **동작**: 매 반복 terminal.draw() 호출 (조건부 최적화는 하지 않음 — ratatui가 diff 기반)

### BR-10: Action → AppEvent 라우팅 (ActionDispatcher)
- **조건**: action_rx에서 Action 수신
- **동작**:
  - Navigation 액션 (Navigate, Back) → App.handle_key에서 직접 처리 (dispatcher 경유 불필요)
  - Fetch* → API 호출 → *Loaded 이벤트
  - Delete*/Create*/etc → API 호출 → 성공/실패 이벤트
  - Quit → should_quit = true (dispatcher 경유 불필요)
- **참고**: 이 unit에서는 dispatcher 스켈레톤만 구현. 실제 API 호출은 도메인 unit에서.

---

## Step 3: Data Flow

### 3.1 EventLoop 메인 루프

```
main()
  |
  v
App::new(config) → (App, event_rx)
  |                     |
  v                     v
run_event_loop(terminal, app, event_rx)
  |
  v
loop {
    tokio::select! {
        key_event   => app.handle_key(key)
        tick        => app.on_tick()
        app_event   => app.handle_event(event)
    }
    terminal.draw(|f| app.render(f))
    if app.should_quit { break }
}
  |
  v
cleanup: terminal restore, exit
```

### 3.2 키 입력 → 액션 → 이벤트 → UI 갱신

```
KeyEvent
  |
  v
App.handle_key()
  |
  +--- Global key? (`:`, `/`, Tab, `q`)
  |    YES → InputMode 전환 or quit
  |    NO  → active_component.handle_key(key)
  |              |
  |              v
  |         Option<Action>
  |              |
  |         Some(action) → action_tx.send(action)
  |                              |
  |                              v
  |                    ActionDispatcher.recv()
  |                              |
  |                              v
  |                    tokio::spawn(API call)
  |                              |
  |                              v
  |                    event_tx.send(AppEvent)
  |                              |
  v                              v
  re-render              EventLoop.select! branch 3
                                 |
                                 v
                         App.handle_event(event)
                                 |
                                 v
                         active_component.handle_event(event)
                                 |
                                 v
                         re-render
```

### 3.3 에러 전파 경로

```
API call failure
  → AppEvent::ApiError { operation, message }
  → App.handle_event()
  → BackgroundTracker status → Failed
  → Toast(Error, message, 10s TTL)
  → StatusBar renders error toast
```

---

## Step 4: Error/Exception Scenarios

| 시나리오 | 원인 | 처리 방식 | 사용자 메시지 |
|----------|------|-----------|---------------|
| crossterm key stream 종료 | 터미널 닫힘 | should_quit = true | (none — 앱 종료) |
| event_rx 채널 닫힘 | dispatcher panic | should_quit = true | (none — 앱 종료) |
| action_tx send 실패 | receiver dropped | 무시 (TUI는 이미 종료 중) | (none) |
| 컴포넌트 미등록 route | 프로그래밍 에러 | 빈 화면 렌더, 에러 로그 | StatusBar: "Unknown view" |
| 다중 빠른 navigate | 사용자 빠른 입력 | 정상 — 각 navigate 순차 처리 | (none) |
| on_tick에서 BackgroundTracker panic | 내부 버그 | catch_unwind 미사용 — 앱 종료 | (panic message) |

---

## Step 5: Unit Scope 경계

이 unit에서 **구현하는 것**:
- App 구조체 + handle_key (글로벌 키만), on_tick, render 스켈레톤
- EventLoop (tokio::select! 루프)
- Router (navigate, back, reset, replace)
- BackgroundTracker (poll_updates, expire_toasts, active_toasts)
- Component trait 정의
- Action / AppEvent enum 정의
- InputMode enum

이 unit에서 **구현하지 않는 것** (후속 unit):
- ActionDispatcher의 실제 API 호출 로직 (Port/Adapter 의존 → Unit 3+)
- 구체적인 Component 구현체 (Domain unit에서)
- UI 위젯 렌더링 (Unit 6)

---

## code-generation Connection

### TDD RED 테스트 케이스 도출

**Router (BR-03~05)**:
1. `test_router_navigate` — navigate 시 history push
2. `test_router_back` — back 시 이전 route 복원
3. `test_router_back_empty` — history 비어있으면 None
4. `test_router_replace` — replace는 history 안 남김
5. `test_router_reset` — reset은 history 초기화
6. `test_router_history_limit` — 20 초과 시 oldest 제거
7. `test_router_navigate_same_route` — 같은 route 중복 push 방지

**BackgroundTracker (BR-07~08)**:
8. `test_tracker_started` — Started 이벤트 → InProgress
9. `test_tracker_completed` — Completed → status 변경 + Toast(Success)
10. `test_tracker_failed` — Failed → status 변경 + Toast(Error)
11. `test_toast_expiry` — TTL 후 toast 제거
12. `test_tracker_gc` — 60초 후 완료/실패 항목 정리
13. `test_in_progress_count` — 진행 중 작업 수

**App.handle_key (BR-01~02)**:
14. `test_app_global_key_colon` — `:` → Command 모드
15. `test_app_global_key_slash` — `/` → Search 모드
16. `test_app_global_key_tab` — Tab → sidebar 토글
17. `test_app_global_key_q` — `q` → should_quit
18. `test_app_esc_to_normal` — Esc → Normal 복귀
19. `test_app_esc_normal_back` — Normal에서 Esc → Router.back()
20. `test_app_delegate_to_component` — Normal에서 비글로벌 키 → component 위임

**Action/AppEvent enums**:
21. `test_action_variants_exist` — 주요 variant 컴파일 검증
22. `test_app_event_variants_exist` — 주요 variant 컴파일 검증
