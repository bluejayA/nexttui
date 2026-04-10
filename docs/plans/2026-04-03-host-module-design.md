# HostModule Design — Host Operations for nexttui

**Complexity:** Comprehensive
**Date:** 2026-04-03
**Status:** Draft — Review Round 2 Complete (7건 보완)

## Summary

nexttui에 **Host 관리 모듈**을 추가한다. Evacuate를 첫 번째 기능으로, 향후 Live Migration All, Host Disable/Enable, 리밸런싱으로 확장하는 범용 "Compute 관리 센터"를 목표로 한다.

제3자 스펙(`docs/plans/Evacuate_SPEC.md`)을 참조하되, nexttui 기존 아키텍처(Module/Component trait, Port/Adapter, Worker, Theme)에 맞게 재설계한다.

### Design Inputs

- 제3자 스펙 (`Evacuate_SPEC.md`) — standalone TUI 설계, 참조용
- Spec Reviewer — 스펙 품질, 모호점, 엣지 케이스
- Architecture Reviewer — Module trait 호환, 레이아웃 전환, Action 패턴
- Security Reviewer — 파괴적 작업 안전장치, 동시성, 감사 추적
- UX/Navigation Reviewer — 네비게이션 일관성, Tab 충돌, 퇴출 경로
- UX Designer — 정보 밀도, 포커스 가시성, 접근성, 진행 중 UX
- Agent Council (Codex + Gemini) — Host 중심 워크플로우 타당성 합의

---

## Architecture

### Module Structure

```
src/module/host/
├── mod.rs              # HostModule — Component trait, 포커스 라우팅, 패널 조합
├── host_list.rs        # 좌측 패널: Hypervisor 목록 + 리소스 바
├── instance_list.rs    # 우측 패널: 호스트별 VM 목록 (체크박스 선택)
├── evac_task.rs        # Evacuate 워크플로우 상태 머신
└── log_panel.rs        # 하단: HostModule 전용 이벤트 로그 (3줄)
```

**Composite 패턴**: 각 서브모듈은 자체 `handle_key()` + `render()` 메서드를 가진 내부 Component struct. `mod.rs`의 HostModule은 포커스 라우팅과 패널 조합만 담당하여 ~150줄 이내 유지.

### Integration Points

```
Route::Hosts (신규) → ModuleRegistry → HostModule
                                         ├─ uses NovaPort (list_hypervisors, evacuate_server, ...)
                                         ├─ uses Theme, Icons, ConfirmDialog, SelectPopup
                                         ├─ dispatches Action::EvacuateServer (기존, 단건 재사용)
                                         ├─ dispatches Action::DisableComputeService / EnableComputeService (신규)
                                         └─ receives AppEvent::ServerEvacuateResult (신규, 성공/실패 포함)

기존 HypervisorModule → HostModule로 흡수 (Route::Hypervisors 리다이렉트)
```

### AppEvent 확장 — 벌크 실패 식별

현재 `AppEvent::ServerEvacuated { id }`는 성공만 표현하고, 실패 시 `ApiError`에 server_id가 없어 벌크 Evacuate에서 어떤 VM이 실패했는지 식별할 수 없다.

```rust
// event.rs — 신규 이벤트
pub enum AppEvent {
    // 기존 ServerEvacuated { id } 유지 (하위 호환)
    // 신규: HostModule의 벌크 워크플로우용
    ServerEvacuateResult { id: String, result: Result<(), String> },
    // 신규: Host Disable/Enable 결과
    ComputeServiceToggled { hostname: String, enabled: bool },
}
```

**Worker 변경** (`worker.rs`):
```rust
// EvacuateServer 처리 분기 수정
Action::EvacuateServer { id, params } => {
    match nova.evacuate_server(&id, &params).await {
        Ok(()) => {
            // 기존 이벤트 (하위 호환) + 신규 이벤트 (벌크용)
            Some(AppEvent::ServerEvacuated { id: id.clone() })
            // + event_tx.send(AppEvent::ServerEvacuateResult { id, result: Ok(()) })
        }
        Err(e) => {
            // 신규: 실패 시에도 server_id 포함
            Some(AppEvent::ServerEvacuateResult { id, result: Err(e.to_string()) })
        }
    }
}
```

EvacTask는 `ServerEvacuateResult`를 수신하여 `on_completed(id, result.is_ok())`를 호출. 실패 사유도 `completed` 목록에 보관하여 EvacResult 팝업에 표시.

### Route & Sidebar Registration

1. `models/common.rs`: `Route` enum에 `Hosts` variant 추가
2. `models/common.rs`: `Route::Hosts` → display name "Hosts", admin-only 플래그
3. `app.rs`: `build_components()`에서 `Route::Hosts → HostModule::new()` 등록
4. `ui/sidebar.rs`: admin일 때 "Hosts" 메뉴 항목 표시 (기존 admin 섹션에 배치)
5. **HypervisorModule 흡수**:
   - `Route::Hypervisors` variant를 enum에서 **유지** (하위 호환), Router에서 `Route::Hosts`로 리다이렉트
   - `src/module/hypervisor/` 디렉토리 삭제, `module/mod.rs`에서 `pub mod hypervisor` 제거
   - Sidebar에서 "Hypervisors" 메뉴 제거, "Hosts" 메뉴로 대체 (admin 섹션)
   - 기존 HypervisorModule 테스트: HostModule의 host_list 테스트로 마이그레이션
   - `models/common.rs`의 Route 개수 assert 테스트 업데이트

### Action 시그니처 변경

기존 `Action::EvacuateServer { id, host }` → `Action::EvacuateServer { id, params: EvacuateParams }`로 변경.

```rust
// action.rs
pub enum Action {
    // ...
    EvacuateServer { id: String, params: EvacuateParams },  // host를 params로 통합
}
```

Worker에서 `params`를 그대로 `nova_port.evacuate_server(id, &params)`에 전달. 기존 ServerModule의 단건 Evacuate도 동일 시그니처 사용 (`EvacuateParams { host: None, ..Default::default() }`).

**Host Disable/Enable Action 추가**:

```rust
// action.rs — 신규 Action variant
pub enum Action {
    // ...
    DisableComputeService { service_id: String, hostname: String },
    EnableComputeService { service_id: String, hostname: String },
}
```

Worker에서 기존 `NovaPort::disable_compute_service()` / `enable_compute_service()` 호출 후 `AppEvent::ComputeServiceToggled` 발행. RBAC는 기존 `ActionKind::EnableDisable` (admin-only) 재사용.

### PendingAction 패턴

HostModule은 기존 `ConfirmHandler` 1:1 패턴 대신 **자체 confirm 로직**을 사용한다.

```rust
// host/mod.rs
enum HostPending {
    EvacuateSelected { server_ids: Vec<String>, params: EvacuateParams },
    EvacuateHost { hostname: String, params: EvacuateParams },
    DisableHost { service_id: String },
}
```

- TypeToConfirm 후 `HostPending` → `evac_task` 상태 머신으로 전달
- 기존 `PendingAction` enum에 variant를 추가하지 않음 (모듈 내부에서 완결)
- ConfirmDialog 위젯 자체는 재사용하되, 확인 후 처리는 HostModule이 직접 수행

---

## Screen Layout

### Terminal Requirements

- **최소 크기**: 120x36 (HostModule 진입 시 체크, 미달 시 경고)
- **일반 모듈**: 기존 80x24 최소 크기 유지

### Layout (120x36+ 기준)

```
┌─ Header ─── ⚙ HOST OPS ──── KT Cloud · region ──── 15:42:03 ─┐
├─[HOSTS 35%]──────────────┬─[INSTANCES 65%]───────────────────┤
│ ○ UP  compute-01  AZ-A 3│   NAME          STATUS    IP      │
│ ✗ DN  compute-02  AZ-A 5│ ☑ prod-web-01   ACTIVE   .2.11   │
│ ○ UP  compute-03  AZ-B 3│ ☑ prod-web-02   ACTIVE   .2.12   │
│ ⊘ DIS compute-05  AZ-C 0│ ☐ prod-worker   SHUTOFF  .2.13   │
│                          │                                    │
│  CPU ████████░░ 75%      │                                    │
│  RAM ███████░░░ 68%      │                                    │
│                          │ 2/5 selected                       │
├─[LOG 3줄]────────────────────────────────────────────────────┤
│ 15:42:01 [INFO] compute-02 DOWN · 5 instances at risk       │
│ 15:42:10 [ OK ] Evacuating 2 selected → compute-03          │
│ 15:42:45 [ OK ] ✓2 completed                                │
├─ StatusBar ── [Instances] 2/5 ── ←→:Panel  e:Evac  Tab:Exit ┤
└──────────────────────────────────────────────────────────────┘
```

### Layout Split Ratios

| Area | Direction | Size |
|------|-----------|------|
| Header | Vertical | 1 line (Length) — 기존 Header 재사용 |
| Main area | Vertical | Fill |
| Log panel | Vertical | 3 lines (Length) |
| StatusBar + ToastBar + InputBar | Vertical | 3 lines — 기존 재사용 |
| Left (Hosts) | Horizontal | 35% |
| Right (Instances) | Horizontal | 65% |

**렌더 경로 상세**: LayoutManager는 변경하지 않는다. App이 `layout_hint()` 체크 후 `LayoutManager.toggle_sidebar()`를 호출하여 `LayoutManager` 내부의 `sidebar_visible`을 false로 설정한다 (App.sidebar_visible이 아닌 LayoutManager의 기존 메서드 활용). HostModule의 3분할(Hosts 35% + Instances 65% + Log 3줄)은 Content 영역 내부에서 `Layout::split()`으로 자체 처리하며, LayoutManager의 구조 변경은 없다.

### Mode Entry/Exit

**진입 시퀀스** (App.dispatch_action):
1. 사용자가 Sidebar에서 "Hosts" 선택 → `Action::Navigate(Route::Hosts)` 발행
2. App이 `router.push(Route::Hosts)` 실행
3. App이 현재 모듈의 `layout_hint()` 체크 → `LayoutHint::FullWidth` 반환
4. App이 `self.sidebar_visible = false` 설정 + Header에 `⚙ HOST OPS` 배지 플래그
5. Content 영역이 전체 너비를 사용하여 HostModule 렌더링

**퇴출 시퀀스** (App.handle_key):
1. Tab 키 입력 시, 현재 모듈이 `LayoutHint::FullWidth`이면:
   - `self.sidebar_visible = true` 복원
   - `self.focus = FocusPane::Sidebar` 설정
   - Header 배지 제거
2. 숫자 키(1-9) 입력 시:
   - 대상 모듈의 `layout_hint()` 체크
   - `FullWidth`가 아니면 `sidebar_visible = true` 자동 복원
   - 대상 모듈로 라우팅

**Evacuate 중 키 차단** (App.handle_key):
- HostModule이 `is_busy() -> bool` 반환 (Executing 상태에서 true)
- `is_busy() == true`이면 App이 `q`(Quit), Tab(퇴출), 숫자 키(모듈 전환)를 무시
- `a`(Abort)만 HostModule에 위임
- StatusBar에 `Evacuating: 2/5 (40%)  a:Abort` 표시

```rust
// app.rs handle_key 변경 (의사코드)
if let Some(component) = self.current_component() {
    if component.is_busy() {
        // Quit, Tab, 숫자키 무시 — Abort만 허용
        return component.handle_key(key);
    }
    if component.layout_hint() == LayoutHint::FullWidth {
        if key == Tab {
            self.sidebar_visible = true;
            self.focus = FocusPane::Sidebar;
            return None;
        }
    }
}
```

- **StatusBar**: 항상 `Tab:나가기` 힌트 표시하여 퇴출 경로 명시

---

## Components

### Host List Panel (host_list.rs)

**데이터 소스**: `NovaPort::list_hypervisors()` (현재 stub → 실구현 필요)

**행 형식**:
```
 ○ UP  compute-01  AZ-A  3 VMs
   CPU ████████░░ 75%
   RAM ███████░░░ 68%
```

**상태 아이콘 + 텍스트 레이블** (접근성):

| State | Icon | Color | Text |
|-------|------|-------|------|
| UP + enabled | `○` | Green | `UP` |
| DOWN | `✗` | Red Bold | `DN` |
| UP + disabled | `⊘` | Yellow Dim | `DIS` |

- 텍스트 레이블 병기로 색맹 사용자 대응
- 선택 행: `▶` prefix + Cyan 하이라이트
- Resource bar: `█`=used, `░`=free, 폭 12자

**키바인딩** (Hosts 패널 포커스 시):

| Key | Action |
|-----|--------|
| ↑/↓ (j/k) | 호스트 선택 이동 |
| e | Host Evacuate — 선택된 호스트의 모든 VM 대피 |
| d | Disable/Enable Host 토글 |
| r | 전체 데이터 새로고침 |
| Right | Instances 패널로 포커스 이동 |

### Instance List Panel (instance_list.rs)

**데이터 소스**: `NovaPort::list_servers(filter: host=선택된_호스트)` — 기존 list_servers에 host 필터 추가 필요

**행 형식**:
```
 ☑ prod-web-01    m2.large    ACTIVE   10.10.2.11
```

**체크박스**: `☑` selected, `☐` unselected
**상태 색상**: 기존 Theme의 status_display() 재사용 (ACTIVE=Green, ERROR=Red, SHUTOFF=DarkGray)

**Evacuate 진행 중 인라인 표시** (팝업 차단하지 않음):

| State | Display |
|-------|---------|
| Pending | `○ prod-web-01  Pending` |
| Rebuilding | `⟳ prod-web-01  Rebuilding...` (tick 기반 블링크) |
| Done | `✓ prod-web-01  Done 12.3s` (Green Bold) |
| Failed | `✗ prod-web-01  No valid host` (Red Bold) |
| Aborted | `⊘ prod-web-01  Aborted` (Yellow Dim) |

**키바인딩** (Instances 패널 포커스 시):

| Key | Action |
|-----|--------|
| ↑/↓ (j/k) | 인스턴스 선택 이동 |
| Space | 체크박스 토글 |
| a | 전체 선택/해제 토글 |
| Enter | 선택된 VM들 Evacuate 시작 → ConfirmDialog |
| t | Target 호스트 변경 → SelectPopup |
| f | 상태 필터 cycle (All → ACTIVE → ERROR → SHUTOFF → All) |
| Left | Hosts 패널로 포커스 이동 |

### Log Panel (log_panel.rs)

- **크기**: 3줄 고정, 최대 200줄 보관
- **포커스 불가** — PgUp/PgDn으로만 스크롤
- **자동 스크롤**: 사용자가 수동 스크롤하지 않은 경우 최신 항목으로
- **레벨 색상**: INFO=DarkGray, WARN=Yellow, ERRR=Red, OK=Green Bold
- **3중 기록** (각각 역할이 다름):
  - HostModule Log (이 패널): **시간축 추적** — Evacuate 워크플로우의 시간 순서 파악 (UI 표시)
  - ActivityLog (전역, `!`키 팝업): **전체 이력** — 다른 모듈 이벤트와 함께 통합 조회
  - AuditLogger (파일 영속): **감사 추적** — 앱 종료 후에도 보존, 운영 증적

### Evacuate Workflow (evac_task.rs)

**상태 머신**:
```
Idle → Confirming → Executing → Completed
                  ↘ Aborted ↗
```

**Confirming 단계** — ConfirmDialog 활용:

```
┌─── Evacuate Confirmation ──────────────────────────┐
│                                                     │
│  Source  : compute-02  (DOWN)                       │
│  Target  : compute-03  (Auto if not specified)      │
│  VMs     : 3 selected instances                     │
│  Storage : shared (no data loss expected)           │
│                                                     │
│  Type hostname to confirm: compute-02               │
│  > compute-0█                                       │
│                                                     │
└─────────────────────────────────────────────────────┘
```

- TypeToConfirm 패턴 (호스트명 타이핑) — 기존 `ConfirmDialog::type_to_confirm()` 재사용
- non-shared storage 경고: "Data loss may occur on non-volume-backed instances"
- force 옵션 활성 시 추가 경고: "DANGEROUS: bypasses host capacity check"

**Executing 단계** — Action 발행 큐 + Semaphore:

HostModule은 직접 `tokio::spawn`하지 않는다. 대신 **Action 발행 큐** 패턴으로 Worker의 기존 인프라(toast, activity log, BackgroundTracker, token refresh)를 재사용한다.

```rust
// evac_task.rs 상태
pub struct EvacTask {
    queue: VecDeque<String>,        // 대기 중 server_ids
    in_flight: HashSet<String>,     // 실행 중 (최대 3개)
    completed: Vec<(String, bool)>, // (server_id, success)
    cancel_requested: bool,
    max_concurrent: usize,          // 기본 3
}

impl EvacTask {
    /// HostModule.handle_event(AppEvent::Tick) 에서 호출
    /// in_flight에 여유가 있으면 큐에서 꺼내 Action 반환
    pub fn poll_next(&mut self) -> Vec<Action> {
        let mut actions = vec![];
        while self.in_flight.len() < self.max_concurrent {
            if self.cancel_requested { break; }
            match self.queue.pop_front() {
                Some(id) => {
                    self.in_flight.insert(id.clone());
                    actions.push(Action::EvacuateServer {
                        id,
                        params: self.params.clone(),
                    });
                }
                None => break,
            }
        }
        actions
    }

    /// AppEvent::ServerEvacuated 수신 시 호출
    pub fn on_completed(&mut self, server_id: &str, success: bool) {
        self.in_flight.remove(server_id);
        self.completed.push((server_id.to_string(), success));
    }
}
```

- **Worker 변경 필요**: `Action::EvacuateServer` 시그니처 변경(`{ id, host }` → `{ id, params }`)에 따라 Worker의 패턴 매칭, `action_name()`, `action_to_kind()` 수정 필수. 또한 실패 시 `ServerEvacuateResult` 이벤트 발행 로직 추가.

**Action::EvacuateServer 시그니처 변경 영향 파일** (총 6곳):

| 파일 | 위치 | 변경 내용 |
|------|------|-----------|
| `src/action.rs:77` | enum 정의 | `{ id, host }` → `{ id, params: EvacuateParams }` |
| `src/action.rs:161` | 테스트 | 생성자 수정 |
| `src/worker.rs:309-314` | handle_action | 디스트럭처링 + `ServerEvacuateResult` 발행 |
| `src/worker.rs:799` | 테스트 | 시그니처 맞춤 |
| `src/module/server/mod.rs:128` | resolve_action | `EvacuateParams { host: None, ..Default::default() }` |
| `src/module/server/mod.rs:1108,1165` | 테스트 2곳 | 시그니처 맞춤 |
- **Semaphore 대신 큐 기반 동시성 제어**: `max_concurrent=3`, in_flight set으로 관리
- **Abort**: `cancel_requested=true` → 큐 소진 중단, in_flight는 Nova 측에서 계속 진행 (취소 불가)
- **진행률**: `completed.len() / (queue.len() + in_flight.len() + completed.len())`

**Tick 트리거 메커니즘**:

EvacTask의 `poll_next()`는 App의 기존 Tick 이벤트 브로드캐스트를 활용한다. 새로운 trait 메서드 추가 불필요.

```rust
// host/mod.rs — HostModule의 handle_event
fn handle_event(&mut self, event: &AppEvent) {
    match event {
        AppEvent::Tick => {
            if self.evac_task.is_executing() {
                // 큐에서 다음 배치 꺼내기
                let actions = self.evac_task.poll_next();
                for action in actions {
                    let _ = self.action_tx.send(action);
                }
            }
        }
        AppEvent::ServerEvacuateResult { id, result } => {
            self.evac_task.on_completed(id, result.is_ok());
            // Instance 리스트 인라인 상태 업데이트
            self.instance_list.update_evac_status(id, result);
            // Log 패널에 기록
            self.log_panel.push(/*...*/);
        }
        // ...
    }
}
```

- App은 250ms Tick을 **모든 컴포넌트에 브로드캐스트** (`app.rs:389` 기존 동작)
- HostModule이 `handle_event(Tick)`에서 `evac_task.poll_next()`를 호출
- `action_tx` (기존 ServerModule과 동일 패턴)로 Worker에 Action 발행
- Component trait 변경 불필요, 기존 이벤트 파이프라인 그대로 활용

**Executing 중 제한**:
- 다른 호스트 선택 차단, 추가 Evacuate 시작 차단
- 읽기 탐색(패널 이동, 스크롤)은 허용
- StatusBar에 `Evacuating: 2/5 (40%)  a:Abort` 상시 표시

**Completed 단계** — EvacResult 팝업:

```
┌─── Evacuation Complete ────────────────────────────┐
│                                                     │
│  ✓  2 succeeded                                     │
│  ✗  1 failed                                        │
│  ⊘  0 aborted                                       │
│                                                     │
│  ✗  prod-worker-01   No valid host found            │
│                                                     │
│  [r] Retry Failed   [d] Disable Host   [Esc] Close  │
└─────────────────────────────────────────────────────┘
```

- Abort 후 3단계 상태: succeeded / still-rebuilding (cannot cancel) / aborted
- Retry: 실패한 VM만 다시 시도
- Disable Host: compute service 비활성화 (후속 안전 조치)

---

## Data Flow

### Port/Adapter Changes

**EvacuateParams 확장**:
```rust
#[derive(Default)]
pub struct EvacuateParams {
    pub host: Option<String>,
    pub on_shared_storage: Option<bool>,  // 신규, Nova v2.14 미만에서만 유효
    pub force: Option<bool>,              // 신규
}
```

**Nova HTTP Adapter body 변경**:
```rust
let mut evac = json!({});
if let Some(host) = &params.host {
    evac["host"] = json!(host);
}
if let Some(oss) = params.on_shared_storage {
    evac["onSharedStorage"] = json!(oss);
}
if let Some(force) = params.force {
    evac["force"] = json!(force);
}
let body = json!({ "evacuate": evac });
```

**Stub → 실구현 필요**:

| Method | Current | Action |
|--------|---------|--------|
| `list_hypervisors()` | `Err(BadRequest)` | 실구현: `GET /os-hypervisors/detail` |
| `enable_compute_service()` | `Err(BadRequest)` | 실구현: `PUT /os-services/{id}` |
| `disable_compute_service()` | `Err(BadRequest)` | 실구현: `PUT /os-services/{id}` |
| `list_servers(filter: host=X)` | host 필터 이미 존재 (`ServerListFilter.host: Option<String>`) | 변경 불필요 |

### Component Trait Extension

```rust
// component.rs
pub enum LayoutHint {
    Default,      // Sidebar + Content (기존)
    FullWidth,    // Sidebar 숨김, Content 전체 사용
}

pub trait Component {
    // 기존 메서드 유지...
    fn layout_hint(&self) -> LayoutHint { LayoutHint::Default }  // 신규, 기본값 Default
    fn is_busy(&self) -> bool { false }  // 신규, Evacuate 진행 중 등 차단 상태
}
```

- 기존 14개 모듈: 두 메서드 모두 기본값 반환 (변경 없음)
- HostModule: `LayoutHint::FullWidth` 반환, Executing 상태에서 `is_busy() = true`
- App이 `layout_hint()`로 sidebar 표시 여부, `is_busy()`로 키 차단 여부 결정 → 모듈-App 간 결합도 최소화

---

## Theme & Icons

### 신규 Theme 토큰

```rust
// theme.rs
pub fn evacuating() -> Style { Style::default().fg(Color::Yellow) }
pub fn evac_success() -> Style { Style::default().fg(Color::LightGreen).add_modifier(Modifier::BOLD) }
```

### 신규 Icons

```rust
// theme.rs Icons
pub fn host_up() -> &'static str { "○" }
pub fn host_down() -> &'static str { "✗" }
pub fn host_disabled() -> &'static str { "⊘" }
pub fn checkbox_on() -> &'static str { "☑" }
pub fn checkbox_off() -> &'static str { "☐" }
```

### 기존 Theme 재사용

| 용도 | 기존 토큰 |
|------|-----------|
| 포커스 테두리 | `focus_border()` = Cyan |
| 비포커스 테두리 | `unfocus_border()` = DarkGray |
| 패널 타이틀 | `panel_title(name, focused)` |
| Active 상태 | `done()` = Green |
| Error 상태 | `error()` = Red |
| Warning/Disabled | `warning()` = Yellow / `disabled()` = DarkGray Dim |

---

## Navigation

### Focus Model

```rust
// host/mod.rs
enum HostFocus {
    HostList,
    InstanceList,
}
```

- **2패널만 포커스 대상** (Log 패널은 포커스 불가)
- Left/Right: HostList ↔ InstanceList 전환
- Tab: HostModule 퇴출 → Sidebar 복원 + Sidebar 포커스
- 숫자 키(1-9): 다른 모듈로 직접 이동

### Bidirectional Jump (2단계)

1단계에서는 미구현. 2단계에서 NavigationStack 도입 후:
- Server Detail의 Host 필드 → HostModule 점프 (해당 호스트 자동 선택)
- HostModule Instance에서 Enter → ServerModule Detail 점프
- Esc/Backspace → 스택 pop으로 원래 위치 복귀
- 스택 깊이 제한: 최대 5

---

## RBAC & Safety

### Access Control

- HostModule 전체: admin-only (Sidebar에서 비-admin에게 숨김, `is_admin_only_route(Route::Hosts)` 추가)
- **기존 ActionKind 재사용** (신규 variant 추가하지 않음):
  - `Action::EvacuateServer` → `ActionKind::Evacuate` (기존, admin-only)
  - `Action::DisableComputeService` / `EnableComputeService` → `ActionKind::EnableDisable` (기존, admin-only)
- `force=true` 사용 시 AuditLogger에 명시적 기록 (`force: true` 플래그 포함)
- 1단계에서 Evacuate/Disable 모두 admin-only, RBAC 세분화는 실제 요구 확인 후

### Confirmation Levels

| Action | Confirmation |
|--------|-------------|
| Evacuate (선택된 VM) | TypeToConfirm (호스트명 타이핑) |
| Evacuate (전체 호스트) | TypeToConfirm (호스트명 타이핑) |
| Disable Host | ConfirmDialog Yes/No |
| Enable Host | 즉시 실행 (안전한 작업) |
| force 옵션 활성 | 추가 경고 팝업 후 TypeToConfirm |

### Audit Trail

- `AuditLogger` (파일 영속, 10MB rotation) — 기존 인프라 재사용
- 기록 항목: initiation (who, what, when), VM별 결과 (success/fail/abort), completion summary
- ActivityLog (인메모리) — UI 표시용 동시 기록

---

## Error Handling

### Edge Cases

| Case | Handling |
|------|----------|
| VM 0대 선택 후 Enter | Evacuate 버튼 비활성, StatusBar에 "Select instances first" |
| Target = Source 호스트 | SelectPopup에서 source 호스트 제외 (표시하되 선택 불가) |
| Evacuate 중 앱 종료 (q) | q 키 무시, a(Abort)만 허용 |
| SIGINT/SIGTERM | 진행 중 작업은 Nova 측에서 계속됨, AuditLogger에 "interrupted" 기록 |
| Token 만료 중 벌크 | 기존 `refresh_lock` Mutex 활용 — thundering herd 이미 방지됨 |
| 동일 서버 중복 evacuate | evac_task에서 in-flight set 관리, 중복 발행 차단 |
| Disable 후 즉시 Evacuate | Disable 완료 이벤트 수신 후에만 Evacuate 허용 (순차 강제) |

---

## Implementation Phases

### Phase 1 (이번 구현)

1. `Route::Hosts` + `HostModule` 등록
2. `Component::layout_hint()` trait 확장
3. `EvacuateParams` 필드 추가 + Adapter body 수정
4. `NovaPort` stub 실구현 (list_hypervisors, enable/disable service, list_servers host 필터)
5. HostList 패널 (Hypervisor 목록 + 리소스 바)
6. InstanceList 패널 (호스트별 VM + 체크박스 선택)
7. Evacuate 워크플로우 (ConfirmDialog + Semaphore + 인라인 진행)
8. Host Disable/Enable
9. Log 패널 (3줄)
10. Header 모드 배지, StatusBar 힌트
11. 기존 HypervisorModule → HostModule 흡수

### Phase 2 (후속)

- Live Migration All (호스트의 모든 VM live migration)
- NavigationStack + 양방향 점프
- 80x24 축소 모드 (Log 숨김, 2패널만)

### Phase 3 (장기)

- Aggregate/AZ 관리
- 리밸런싱 액션
- Host 통계 대시보드

---

## Assumptions

- KT Cloud의 Nova API 버전에서 `onSharedStorage` 파라미터가 유효하다고 가정. v2.14+ 에서 deprecated인 경우 무시 처리.
- `PUT /os-services/{id}` (microversion >= 2.53) 사용 가능. 미만인 경우 legacy endpoint 분기 필요.
- DevStack 단일 노드에서는 Evacuate 실행 불가 (`NoValidHost`). Mock 모드에서 UI/워크플로우만 검증.
- 기존 831개 테스트에 대한 회귀 영향은 `EvacuateParams` Default derive로 최소화. 예상 수정 필요 테스트: 15-25개 (대부분 컴파일 에러 수준).

### Regression Impact Summary

| 변경 | 영향 테스트 수 (추정) | 심각도 |
|------|----------------------|--------|
| `Action::EvacuateServer` 시그니처 변경 | ~6개 | 컴파일 에러 (쉬운 수정) |
| `EvacuateParams` 필드 추가 + Default | ~2개 | 컴파일 에러 |
| `Component` trait 확장 (layout_hint, is_busy) | 0개 (기본값 제공) | 안전 |
| `AppEvent` 신규 variant 추가 | 0개 (추가만, 기존 변경 없음) | 안전 |
| `Route::Hosts` 추가 | ~1개 (Route variant 수 assert) | 단순 수정 |
| `HypervisorModule` 흡수 | ~3-5개 | 삭제/마이그레이션 |
| `App.handle_key` 수정 (is_busy/layout_hint) | ~5-10개 | 주의 필요 |

---

## Test Strategy

### Unit Tests

| 대상 | 테스트 시나리오 |
|------|----------------|
| **host_list** | Hypervisor 목록 렌더링, 상태 아이콘/색상 매핑, 선택 이동, 빈 목록 |
| **instance_list** | 호스트별 VM 필터링, 체크박스 토글/전체 선택, 상태 필터 cycle |
| **evac_task 상태 머신** | Idle→Confirming→Executing→Completed 전이, Abort 시 큐 소진 중단 |
| **evac_task 동시성** | max_concurrent=3 준수, in_flight 초과 방지 |
| **evac_task 완료 처리** | on_completed()로 in_flight 제거 + 다음 큐 항목 발행 |
| **EvacuateParams** | Default derive 동작, force/on_shared_storage 직렬화 |

### Integration Tests

| 대상 | 테스트 시나리오 |
|------|----------------|
| **HostModule → MockNovaPort** | list_hypervisors → host 선택 → list_servers(host=X) → evacuate 흐름 |
| **LayoutHint → App sidebar** | HostModule 진입 시 sidebar 숨김, 퇴출 시 복원 |
| **is_busy() → 키 차단** | Executing 상태에서 q/Tab/숫자키 무시, a만 허용 |
| **TypeToConfirm** | 정확한 호스트명 입력 시 진행, 불일치 시 차단 |

### Edge Case Tests

| Case | Expected |
|------|----------|
| VM 0대 선택 후 Enter | 무반응, StatusBar에 "Select instances first" |
| 모든 VM evacuate 성공 | Toast "All 5 evacuated successfully", 팝업 스킵 |
| 부분 실패 (3성공 2실패) | EvacResult 팝업에 실패 목록 + Retry 옵션 |
| Abort 후 결과 | 3단계 표시: succeeded / still-rebuilding / aborted |
| 중복 evacuate 방지 | in_flight에 있는 서버 재발행 차단 |
| 120x36 미만 진입 시도 | 경고 메시지 표시, 진입 차단 |

- **TDD**: 실패 테스트 먼저, RED-GREEN-REFACTOR 사이클 엄수
- **Mock Evacuate**: sleep(600..1800ms) + 10% 랜덤 실패 (스펙 참조)
