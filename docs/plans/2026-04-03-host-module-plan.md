# HostModule Implementation Plan

> **For agentic workers:** REQUIRED: Use `aidlc-subagent-driven-development` or `aidlc-executing-plans` to implement.

**Goal:** nexttui에 Host 관리 모듈(Evacuate + Host Disable/Enable)을 추가한다.
**Complexity:** Comprehensive
**Architecture:** HostModule은 Composite 패턴으로 4개 서브모듈(host_list, instance_list, evac_task, log_panel)을 조합한다. Content 영역 내부에서 2패널(35%/65%) + Log(3줄)로 자체 분할하며, LayoutHint::FullWidth로 sidebar를 숨긴다. 기존 Worker의 단건 Action 파이프라인을 재사용하여 벌크 Evacuate를 큐 기반으로 발행한다.
**Tech Stack:** Rust, ratatui, tokio, crossterm
**Design Doc:** `docs/plans/2026-04-03-host-module-design.md`

---

## Unit 1: Foundation — EvacuateParams + Component Trait + Action/Event/Worker

기존 코드 수정. 이 Unit 완료 후 기존 831개 테스트가 모두 통과해야 한다.

### Task 1: EvacuateParams 확장

**Files:**
- Modify: `src/port/types.rs:292-294`
- Modify: `src/adapter/http/nova.rs:387-391`
- Modify: `src/worker.rs:309-315`
- Modify: `src/module/server/mod.rs:128`
- Test: 기존 테스트 회귀 확인

- [ ] **Step 1: EvacuateParams에 Default derive + 신규 필드 추가**

```rust
// src/port/types.rs:292-294 → 변경
#[derive(Debug, Clone, Default)]
pub struct EvacuateParams {
    pub host: Option<String>,
    pub on_shared_storage: Option<bool>,
    pub force: Option<bool>,
}
```

- [ ] **Step 2: Nova Adapter evacuate body 확장**

```rust
// src/adapter/http/nova.rs:387-391 → 변경
let mut evac = serde_json::json!({});
if let Some(host) = &params.host {
    evac["host"] = serde_json::json!(host);
}
if let Some(oss) = params.on_shared_storage {
    evac["onSharedStorage"] = serde_json::json!(oss);
}
if let Some(force) = params.force {
    evac["force"] = serde_json::json!(force);
}
let body = serde_json::json!({ "evacuate": evac });
```

- [ ] **Step 3: Action::EvacuateServer 시그니처 변경**

```rust
// src/action.rs:77 → 변경
EvacuateServer { id: String, params: EvacuateParams },
```

`action_name()` (190줄), `action_to_kind()` (139줄)도 매칭 패턴 업데이트:
```rust
Action::EvacuateServer { .. } => // 기존과 동일, ..으로 매치하므로 변경 불필요
```

- [ ] **Step 4: Worker 디스트럭처링 수정**

```rust
// src/worker.rs:309-315 → 변경
Action::EvacuateServer { id, params } => {
    match registry.nova.evacuate_server(&id, &params).await {
        Ok(()) => Some(AppEvent::ServerEvacuated { id }),
        Err(e) => Some(api_error("EvacuateServer", e)),
    }
}
```

- [ ] **Step 5: ServerModule resolve_action 수정**

```rust
// src/module/server/mod.rs:128 → 변경
PendingAction::Evacuate { id } => Some(Action::EvacuateServer {
    id,
    params: EvacuateParams::default(),
}),
```

- [ ] **Step 6: 기존 테스트 회귀 수정 (action.rs, worker.rs, server/mod.rs 테스트)**
Run: `cargo test 2>&1 | head -50`
Expected: 컴파일 에러 → 테스트 내 EvacuateServer 생성자를 params 기반으로 수정

- [ ] **Step 7: 전체 테스트 통과 확인**
Run: `cargo test`
Expected: 831+ tests PASS

- [ ] **Step 8: Commit**
`feat: extend EvacuateParams with force/on_shared_storage fields`

---

### Task 2: Component Trait Extension (LayoutHint, is_busy)

**Files:**
- Modify: `src/component.rs:8-17`
- Test: 기존 테스트 회귀 없음 확인

- [ ] **Step 1: LayoutHint enum + trait 메서드 추가**

```rust
// src/component.rs — enum 추가 (trait 위에)
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LayoutHint {
    Default,
    FullWidth,
}

// Component trait에 메서드 추가 (is_modal 다음)
fn layout_hint(&self) -> LayoutHint { LayoutHint::Default }
fn is_busy(&self) -> bool { false }
```

- [ ] **Step 2: 전체 테스트 통과 확인**
Run: `cargo test`
Expected: 기존 테스트 모두 PASS (기본값 제공이므로 기존 모듈 변경 불필요)

- [ ] **Step 3: Commit**
`feat: add LayoutHint and is_busy to Component trait`

---

### Task 3: AppEvent + Action 확장 (ServerEvacuateResult, Disable/Enable)

**Files:**
- Modify: `src/event.rs:68-87`
- Modify: `src/action.rs`
- Modify: `src/worker.rs`
- Test: 기존 테스트 회귀 확인

- [ ] **Step 1: AppEvent에 신규 variant 추가**

```rust
// src/event.rs — 기존 ServerEvacuated 아래에 추가
ServerEvacuateResult { id: String, result: Result<(), String> },
ComputeServiceToggled { hostname: String, enabled: bool },
```

- [ ] **Step 2: Action에 Disable/Enable variant 추가**

```rust
// src/action.rs — EvacuateServer 아래에 추가
DisableComputeService { service_id: String, hostname: String },
EnableComputeService { service_id: String, hostname: String },
```

`action_name()`, `action_to_kind()`에 매핑 추가:
```rust
Action::DisableComputeService { .. } => "DisableComputeService",
Action::EnableComputeService { .. } => "EnableComputeService",
// action_to_kind:
Action::DisableComputeService { .. } => Some(ActionKind::EnableDisable),
Action::EnableComputeService { .. } => Some(ActionKind::EnableDisable),
```

- [ ] **Step 3: Worker에 EvacuateServer 실패 시 ServerEvacuateResult 발행**

```rust
// src/worker.rs — EvacuateServer 처리 블록 수정
Action::EvacuateServer { id, params } => {
    match registry.nova.evacuate_server(&id, &params).await {
        Ok(()) => {
            let _ = event_tx.send(AppEvent::ServerEvacuateResult {
                id: id.clone(),
                result: Ok(()),
            });
            Some(AppEvent::ServerEvacuated { id })
        }
        Err(e) => {
            let msg = e.to_string();
            let _ = event_tx.send(AppEvent::ServerEvacuateResult {
                id: id.clone(),
                result: Err(msg.clone()),
            });
            Some(api_error("EvacuateServer", e))
        }
    }
}
```

주의: `handle_action`이 `Option<AppEvent>`를 반환하고, Worker가 별도로 `event_tx`를 가지는 구조인지 확인 필요. 기존 패턴에서 `event_tx`가 없다면, Worker의 반환값을 `ServerEvacuateResult`로 변경하고, App에서 `ServerEvacuated`를 추가 발행하는 방식도 가능.

- [ ] **Step 4: Worker에 Disable/Enable 처리 추가**

```rust
// src/worker.rs — handle_action에 추가
Action::DisableComputeService { service_id, hostname } => {
    match registry.nova.disable_compute_service(&service_id, None).await {
        Ok(_) => Some(AppEvent::ComputeServiceToggled { hostname, enabled: false }),
        Err(e) => Some(api_error("DisableComputeService", e)),
    }
}
Action::EnableComputeService { service_id, hostname } => {
    match registry.nova.enable_compute_service(&service_id).await {
        Ok(_) => Some(AppEvent::ComputeServiceToggled { hostname, enabled: true }),
        Err(e) => Some(api_error("EnableComputeService", e)),
    }
}
```

- [ ] **Step 5: 전체 테스트 통과 확인**
Run: `cargo test`

- [ ] **Step 6: Commit**
`feat: add ServerEvacuateResult event and Disable/Enable actions`

---

## Unit 2: Nova API — Stub 실구현

### Task 4: list_hypervisors 실구현

**Files:**
- Modify: `src/adapter/http/nova.rs:529-531`
- Modify: `src/port/mock.rs` (mock 구현)
- Test: `src/adapter/http/nova.rs` (기존 테스트 파일)

- [ ] **Step 1: 실패 테스트 작성**
```rust
#[tokio::test]
async fn test_list_hypervisors_returns_vec() {
    let adapter = mock_nova_adapter(); // 기존 mock 패턴 참조
    let result = adapter.list_hypervisors().await;
    assert!(result.is_ok());
}
```

- [ ] **Step 2: Run test — verify FAIL**
Run: `cargo test test_list_hypervisors`
Expected: FAIL (현재 stub은 Err(BadRequest) 반환)

- [ ] **Step 3: HTTP Adapter 실구현**
```rust
// src/adapter/http/nova.rs:529-531 → 교체
async fn list_hypervisors(&self) -> ApiResult<Vec<Hypervisor>> {
    let url = format!("{}/os-hypervisors/detail", self.base.endpoint());
    let resp: serde_json::Value = self.base.get(&url).await?;
    let hypervisors = resp["hypervisors"]
        .as_array()
        .unwrap_or(&vec![])
        .iter()
        .filter_map(|h| serde_json::from_value(h.clone()).ok())
        .collect();
    Ok(hypervisors)
}
```

- [ ] **Step 4: Mock adapter 구현**
`src/port/mock.rs`에서 `list_hypervisors()`가 테스트용 Hypervisor 벡터 반환하도록 구현.

- [ ] **Step 5: Run test — verify PASS**
- [ ] **Step 6: Commit**
`feat: implement list_hypervisors Nova API`

---

### Task 5: enable/disable_compute_service 실구현

**Files:**
- Modify: `src/adapter/http/nova.rs` (enable/disable stub 위치)
- Test: 신규 테스트

- [ ] **Step 1: 실패 테스트 작성**
- [ ] **Step 2: HTTP Adapter 실구현**
```rust
async fn disable_compute_service(&self, service_id: &str, reason: Option<&str>) -> ApiResult<ComputeService> {
    let url = format!("{}/os-services/{}", self.base.endpoint(), service_id);
    let mut body = serde_json::json!({ "status": "disabled" });
    if let Some(r) = reason {
        body["disabled_reason"] = serde_json::json!(r);
    }
    let resp: serde_json::Value = self.base.put_json(&url, &body).await?;
    // parse ComputeService from resp["service"]
    Ok(serde_json::from_value(resp["service"].clone())?)
}
```
- [ ] **Step 3: Run test — verify PASS**
- [ ] **Step 4: Commit**
`feat: implement enable/disable_compute_service Nova API`

---

## Unit 3: EvacTask State Machine

### Task 6: EvacTask 핵심 로직

**Files:**
- Create: `src/module/host/evac_task.rs`
- Test: 같은 파일 하단 `#[cfg(test)]` 모듈

- [ ] **Step 1: 실패 테스트 — 상태 전이**
```rust
#[test]
fn test_evac_task_idle_to_executing() {
    let params = EvacuateParams::default();
    let mut task = EvacTask::new(vec!["s1".into(), "s2".into()], params, 2);
    assert!(task.is_idle()); // 초기 Idle이지만 start() 호출 전

    task.start();
    assert!(task.is_executing());

    let actions = task.poll_next();
    assert_eq!(actions.len(), 2); // max_concurrent=2, 큐에 2개
    assert_eq!(task.in_flight_count(), 2);
}
```

- [ ] **Step 2: Run test — verify FAIL**

- [ ] **Step 3: EvacTask 구현**
```rust
use std::collections::{HashSet, VecDeque};
use crate::action::Action;
use crate::port::types::EvacuateParams;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EvacState {
    Idle,
    Executing,
    Completed,
}

pub struct EvacTask {
    state: EvacState,
    queue: VecDeque<String>,
    in_flight: HashSet<String>,
    completed: Vec<(String, Result<(), String>)>,
    cancel_requested: bool,
    params: EvacuateParams,
    max_concurrent: usize,
}

impl EvacTask {
    pub fn new(server_ids: Vec<String>, params: EvacuateParams, max_concurrent: usize) -> Self {
        Self {
            state: EvacState::Idle,
            queue: server_ids.into(),
            in_flight: HashSet::new(),
            completed: Vec::new(),
            cancel_requested: false,
            params,
            max_concurrent,
        }
    }

    pub fn start(&mut self) { self.state = EvacState::Executing; }
    pub fn is_idle(&self) -> bool { self.state == EvacState::Idle }
    pub fn is_executing(&self) -> bool { self.state == EvacState::Executing }
    pub fn is_completed(&self) -> bool { self.state == EvacState::Completed }
    pub fn in_flight_count(&self) -> usize { self.in_flight.len() }

    pub fn poll_next(&mut self) -> Vec<Action> {
        if self.state != EvacState::Executing { return vec![]; }
        let mut actions = vec![];
        while self.in_flight.len() < self.max_concurrent && !self.cancel_requested {
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
        // 큐 비고 + in_flight 비면 → Completed
        if self.queue.is_empty() && self.in_flight.is_empty() {
            self.state = EvacState::Completed;
        }
        actions
    }

    pub fn on_completed(&mut self, server_id: &str, result: Result<(), String>) {
        self.in_flight.remove(server_id);
        self.completed.push((server_id.to_string(), result));
        if self.queue.is_empty() && self.in_flight.is_empty() {
            self.state = EvacState::Completed;
        }
    }

    pub fn request_cancel(&mut self) {
        self.cancel_requested = true;
        // 큐에 남은 것들을 aborted로 처리
        while let Some(id) = self.queue.pop_front() {
            self.completed.push((id, Err("Aborted".into())));
        }
        if self.in_flight.is_empty() {
            self.state = EvacState::Completed;
        }
    }

    pub fn progress(&self) -> (usize, usize) {
        let total = self.completed.len() + self.in_flight.len() + self.queue.len();
        (self.completed.len(), total)
    }

    pub fn results(&self) -> &[(String, Result<(), String>)] { &self.completed }
    pub fn succeeded_count(&self) -> usize { self.completed.iter().filter(|(_, r)| r.is_ok()).count() }
    pub fn failed_results(&self) -> Vec<&(String, Result<(), String>)> {
        self.completed.iter().filter(|(_, r)| r.is_err()).collect()
    }
}
```

- [ ] **Step 4: 추가 테스트 — 동시성, Abort, 완료**
```rust
#[test]
fn test_evac_task_max_concurrent_limit() {
    let mut task = EvacTask::new(vec!["s1".into(), "s2".into(), "s3".into()], EvacuateParams::default(), 2);
    task.start();
    let batch1 = task.poll_next();
    assert_eq!(batch1.len(), 2);
    assert_eq!(task.poll_next().len(), 0); // in_flight full

    task.on_completed("s1", Ok(()));
    let batch2 = task.poll_next();
    assert_eq!(batch2.len(), 1); // s3 발행
}

#[test]
fn test_evac_task_cancel() {
    let mut task = EvacTask::new(vec!["s1".into(), "s2".into(), "s3".into()], EvacuateParams::default(), 1);
    task.start();
    task.poll_next(); // s1 in_flight
    task.request_cancel();
    assert_eq!(task.poll_next().len(), 0); // 큐 소진 중단
    // s2, s3는 Aborted
    task.on_completed("s1", Ok(()));
    assert!(task.is_completed());
    assert_eq!(task.succeeded_count(), 1);
    assert_eq!(task.failed_results().len(), 2); // s2, s3 aborted
}
```

- [ ] **Step 5: Run test — verify PASS**
Run: `cargo test evac_task`

- [ ] **Step 6: Commit**
`feat: EvacTask state machine with queue-based concurrency`

---

## Unit 4: HostModule Sub-panels

### Task 7: HostList Panel

**Files:**
- Create: `src/module/host/host_list.rs`
- Test: 같은 파일 `#[cfg(test)]`

핵심: Hypervisor 목록 렌더링, 상태 아이콘, 리소스 바, 선택 이동.
기존 `ResourceList` 위젯을 직접 사용하지 않고 (3줄/호스트 렌더링 필요), 자체 렌더 로직 구현.

- [ ] **Step 1: 실패 테스트 — 선택 이동**
- [ ] **Step 2: HostList struct + handle_key/render 구현**
- [ ] **Step 3: 테스트 통과**
- [ ] **Step 4: Commit**
`feat: HostList panel with hypervisor display`

### Task 8: InstanceList Panel

**Files:**
- Create: `src/module/host/instance_list.rs`
- Test: 같은 파일 `#[cfg(test)]`

핵심: 호스트별 VM 목록, 체크박스 선택, 상태 필터, Evacuate 진행 인라인 표시.

- [ ] **Step 1: 실패 테스트 — 체크박스 토글**
- [ ] **Step 2: InstanceList struct 구현**
- [ ] **Step 3: 상태 필터 cycle 구현**
- [ ] **Step 4: Evacuate 인라인 상태 표시 구현**
- [ ] **Step 5: 테스트 통과**
- [ ] **Step 6: Commit**
`feat: InstanceList panel with checkbox selection`

### Task 9: Log Panel

**Files:**
- Create: `src/module/host/log_panel.rs`

핵심: 3줄 고정, 200줄 링버퍼, 레벨별 색상.

- [ ] **Step 1: LogPanel struct 구현**
```rust
pub struct LogPanel {
    entries: VecDeque<LogEntry>,
    scroll: usize,
    max_entries: usize, // 200
}

pub struct LogEntry {
    pub timestamp: String,
    pub level: LogLevel,
    pub message: String,
}
```
- [ ] **Step 2: 테스트 + Commit**
`feat: LogPanel for HostModule event log`

---

## Unit 5: HostModule Assembly + Route Registration

### Task 10: HostModule 조합 (mod.rs)

**Files:**
- Create: `src/module/host/mod.rs`
- Modify: `src/module/mod.rs` (pub mod host 추가)
- Modify: `src/models/common.rs` (Route::Hosts 추가)
- Modify: `src/infra/rbac.rs:170-182` (is_admin_only_route에 Hosts 추가)

핵심: Composite 패턴으로 서브모듈 조합, HostFocus enum, handle_key 라우팅, render 레이아웃 분할.

- [ ] **Step 1: Route::Hosts 추가 + RBAC 등록**

```rust
// src/models/common.rs — Route enum에 추가
Hosts,
```

```rust
// src/infra/rbac.rs:170-182 — is_admin_only_route에 추가
Route::Hosts
```

- [ ] **Step 2: module/mod.rs에 pub mod host 추가**

- [ ] **Step 3: HostModule struct 구현**
```rust
pub struct HostModule {
    focus: HostFocus,
    host_list: HostList,
    instance_list: InstanceList,
    evac_task: Option<EvacTask>,
    log_panel: LogPanel,
    action_tx: mpsc::UnboundedSender<Action>,
    is_admin: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum HostFocus { HostList, InstanceList }
```

- [ ] **Step 4: Component trait 구현**
- `layout_hint()` → `LayoutHint::FullWidth`
- `is_busy()` → `evac_task.is_some_and(|t| t.is_executing())`
- `handle_key()` → HostFocus 기반 라우팅 + Left/Right 패널 전환
- `handle_event()` → Tick에서 evac_task.poll_next(), ServerEvacuateResult 수신
- `render()` → Content 영역을 Layout::split()으로 3분할

- [ ] **Step 5: App에 HostModule 등록**
기존 ModuleRegistry 패턴에 따라 Route::Hosts → HostModule 등록.

- [ ] **Step 6: 테스트**
Run: `cargo test`

- [ ] **Step 7: Commit**
`feat: HostModule assembly with route registration`

---

## Unit 6: App Integration — LayoutHint + is_busy

### Task 11: App handle_key/render 수정

**Files:**
- Modify: `src/app.rs:214-221` (Tab 처리)
- Modify: `src/app.rs:140` (handle_key 상단에 is_busy 체크)
- Modify: `src/app.rs:533` (render에서 layout_hint 체크)
- Modify: `src/ui/header.rs` (HOST OPS 배지)

- [ ] **Step 1: 실패 테스트 — LayoutHint::FullWidth 시 sidebar 숨김**

- [ ] **Step 2: App.handle_key에 is_busy + LayoutHint 분기 추가**

```rust
// app.rs handle_key — Tab 처리 전에 삽입
if let Some(component) = self.current_component() {
    if component.is_busy() {
        return component.handle_key(key).is_some();
    }
}

// Tab 처리 수정
KeyCode::Tab => {
    if let Some(component) = self.current_component() {
        if component.layout_hint() == LayoutHint::FullWidth {
            self.layout.toggle_sidebar(); // sidebar 복원
            self.focus = FocusPane::Sidebar;
            return true;
        }
    }
    // 기존 Tab 로직 유지
    if self.sidebar_visible { /* ... */ }
}
```

- [ ] **Step 3: Route 전환 시 LayoutHint 체크 → sidebar 자동 숨김**
`dispatch_action`에서 `Action::Navigate` 처리 시:
```rust
// 새 모듈의 layout_hint 체크
if new_component.layout_hint() == LayoutHint::FullWidth {
    self.layout.toggle_sidebar(); // sidebar 숨김
}
```

- [ ] **Step 4: Header에 HOST OPS 배지 추가**
`src/ui/header.rs`에서 현재 Route가 Hosts이면 `⚙ HOST OPS` 배지 렌더링.

- [ ] **Step 5: 테스트 통과**
- [ ] **Step 6: Commit**
`feat: App LayoutHint/is_busy integration for HostModule`

---

## Unit 7: Theme + Icons 확장

### Task 12: Theme/Icons 신규 토큰

**Files:**
- Modify: `src/ui/theme.rs:45-49` (Theme에 메서드 추가)
- Modify: `src/ui/theme.rs:83-93` (Icons에 메서드 추가)

- [ ] **Step 1: Theme 메서드 추가**
```rust
pub fn evacuating() -> Style { Style::default().fg(Color::Yellow) }
pub fn evac_success() -> Style { Style::default().fg(Color::LightGreen).add_modifier(Modifier::BOLD) }
```

- [ ] **Step 2: Icons 메서드 추가**
```rust
pub fn host_up() -> &'static str { "○" }
pub fn host_down() -> &'static str { "✗" }
pub fn host_disabled() -> &'static str { "⊘" }
pub fn checkbox_on() -> &'static str { "☑" }
pub fn checkbox_off() -> &'static str { "☐" }
```

- [ ] **Step 3: instance_list status_color에 VERIFY_RESIZE 색상 추가**
  (Codex 리뷰 #4: theme Icons.status_icon은 VERIFY_RESIZE 지원하지만 instance_list의 status_color에 누락)

- [ ] **Step 4: Commit**
`feat: add Host/Evacuate theme tokens and icons`

---

## Unit 8: HypervisorModule Absorption + Cleanup

### Task 13: HypervisorModule 흡수

**Files:**
- Delete: `src/module/hypervisor/` (전체 디렉토리)
- Modify: `src/module/mod.rs` (pub mod hypervisor 제거)
- Modify: `src/models/common.rs` (Route::Hypervisors → Route::Hosts 리다이렉트)
- Modify: `src/infra/rbac.rs` (Route::Hypervisors → Route::Hosts)

- [ ] **Step 1: Route::Hypervisors를 Route::Hosts로 리다이렉트**
Router에서 Hypervisors 접근 시 Hosts로 전환.

- [ ] **Step 2: pub mod hypervisor 제거 + 디렉토리 삭제**

- [ ] **Step 3: 기존 HypervisorModule 테스트를 HostList 테스트로 마이그레이션**

- [ ] **Step 4: Route 개수 assert 테스트 업데이트**

- [ ] **Step 5: 전체 테스트 통과**
Run: `cargo test`

- [ ] **Step 6: Commit**
`refactor: absorb HypervisorModule into HostModule`

---

## Execution Order Summary

```
Unit 1 (Foundation)  ←── 가장 먼저, 기존 코드 수정
  Task 1: EvacuateParams
  Task 2: Component trait
  Task 3: Action/Event/Worker
     ↓
Unit 2 (Nova API)  ←── Unit 1 후, 독립적
  Task 4: list_hypervisors
  Task 5: enable/disable service
     ↓
Unit 3 (EvacTask)  ←── Unit 1 후, 독립적 (Unit 2와 병렬 가능)
  Task 6: EvacTask state machine
     ↓
Unit 4 (Sub-panels)  ←── Unit 2, 3 후
  Task 7: HostList
  Task 8: InstanceList
  Task 9: LogPanel
     ↓
Unit 5 (Assembly)  ←── Unit 4 후
  Task 10: HostModule + Route
     ↓
Unit 6 (App Integration)  ←── Unit 5 후
  Task 11: App handle_key/render
     ↓
Unit 7 (Theme)  ←── 독립, 어느 시점이든 가능
  Task 12: Theme/Icons
     ↓
Unit 8 (Cleanup)  ←── 모든 Unit 후 마지막
  Task 13: HypervisorModule 흡수
```

**총 예상**: 13 Tasks, 8 Units
**병렬 가능**: Unit 2 + Unit 3, Unit 7은 독립
