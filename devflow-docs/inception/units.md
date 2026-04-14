# Units

**BL**: BL-P2-031 Keystone Rescoping
**Timestamp**: 2026-04-13T00:00:00+09:00
**총 7개 단위** (PR1: Unit 1~4, PR3: Unit 5, PR4: Unit 6, PR5: Unit 7)

## 분해 원칙

- 각 unit은 단일 세션 내 완료 가능 (TDD RED-GREEN-REFACTOR 1~3 사이클)
- 각 unit은 격리 테스트 가능 (mock 또는 trait 인터페이스로 의존성 차단)
- PR 매핑에 정합: 같은 PR에 묶이는 unit은 feature 브랜치 내 누적 머지 가능
- 의존 그래프는 DAG (Unit 1이 모든 후속의 토대)

## 구현 순서

```
Unit 1 (Foundation Types)
  ├──> Unit 2 (Concurrency Infra)  ─┐
  └──> Unit 3 (Auth & Session Port) ─┼──> Unit 4 (Switch Orchestration)
                                      │      │
                                      │      ├──> Unit 5 (Commands & Safety UI)  ─┬──> Unit 6 (Picker UI)
                                      │      │                                      │
                                      │      └──────────────────────────────────────┴──> Unit 7 (Identity Module)
```

Unit 2와 Unit 3는 Unit 1 완료 후 **병렬 가능**.
Unit 6와 Unit 7은 Unit 5 완료 후 **병렬 가능**.

---

## Unit 1: Foundation Types

**Responsibility**: 모든 후속 unit이 의존하는 핵심 타입과 무상태 유틸리티를 정의한다.
**Dependencies**: none
**Interfaces (exposes)**:
- `src/context/mod.rs` (신규 모듈) — `ContextRequest`, `ContextTarget`, `ContextSnapshot`, `SessionHandle`, `Epoch`
- `src/context/error.rs` — `SwitchError` enum (transparent ApiError/IoError)
- `src/context/epoch.rs` — `ContextEpoch` (atomic counter)
- `src/context/versioned.rs` — `VersionedEvent<T>` envelope
- `src/context/history.rs` — `ContextHistoryStore` (1단계 이전 snapshot)
- `src/context/capabilities.rs` — `KeystoneCapabilities`, `AuthMethod`, `KeystoneVersion`
- `From<&ContextTarget> for TokenScope` impl

**Tests**:
- ContextEpoch.bump 단조 증가
- VersionedEvent.epoch round-trip
- ContextHistoryStore push/pop_previous
- ContextTarget → TokenScope 변환

**Implementation order**: 1
**PR**: PR1

---

## Unit 2: Concurrency Infrastructure

**Responsibility**: stale 이벤트 격리를 위한 cancellation 인프라와 Worker 시그니처 통일.
**Dependencies**: Unit 1 (Epoch, VersionedEvent)
**Interfaces (exposes)**:
- `src/context/cancellation.rs` — `CancellationRegistry { register(epoch), cancel_below(threshold) }`
- `src/port/http_endpoint_cache.rs` — `HttpEndpointCache` trait
- `src/event.rs` 변경 — `AppEvent::ContextChanged { target: ContextTarget }` variant 추가
- `src/action.rs` 변경 — `Action::SwitchContext(ContextRequest)`, `Action::SwitchBack` variants 추가
- `src/worker.rs` 변경:
  - `run_worker` 시그니처: `mpsc::UnboundedReceiver<VersionedEvent<Action>>` rx, `mpsc::UnboundedSender<VersionedEvent<AppEvent>>` tx
  - `spawn_versioned<F, T>(cancel, epoch, event_tx, fut)` 헬퍼
  - 모든 기존 `tokio::spawn` 사이트가 헬퍼 사용으로 마이그레이트
- `src/app.rs` 부분 변경 — `current_epoch: Arc<ContextEpoch>` 필드 + 디스패처 epoch gate (`event.epoch() < current_epoch.current()` → drop)

**Tests**:
- CancellationRegistry: register N tokens at epoch e, cancel_below(e+1) → 모두 cancel
- spawn_versioned: cancel 시 이벤트 미발행, 정상 시 epoch 보존
- App dispatcher: stale event drop 검증
- 회귀: 기존 1116 tests pass

**Implementation order**: 2
**PR**: PR1
**병렬 가능**: Unit 3와 동시 진행 가능 (의존 분리)

---

## Unit 3: Auth & Session Port Layer

**Responsibility**: Keystone rescoping을 atomic begin/commit/rollback으로 제공하는 port 레이어 + 모든 관련 adapter.
**Dependencies**: Unit 1 (타입)
**Interfaces (exposes)**:
- `src/port/scoped_auth.rs` — `ScopedAuthPort` trait (`current_scope`, `current_token`, `set_active`)
- `src/port/context_session.rs` — `ContextSessionPort` trait (`begin`, `rescope`, `refresh_catalog`, `commit`, `rollback`)
- `src/adapter/auth/keystone.rs` 변경 — `KeystoneAuthAdapter`가 `ScopedAuthPort` 추가 구현
- `src/adapter/auth/rescope.rs` (신규) — `KeystoneRescopeAdapter { rescope, discover_capabilities }`
- `src/adapter/auth/token_cache.rs` 변경 — `store_scoped(scope, token)`, `lookup_scoped(scope)` 추가
- `src/adapter/http/endpoint_invalidator.rs` (신규) — `EndpointCatalogInvalidator { invalidate_all, refresh_from }`
- `src/adapter/http/base.rs` 변경 — `HttpEndpointCache` trait 구현
- `src/adapter/registry.rs` 변경 — `Vec<Arc<dyn HttpEndpointCache>>` 보유 + `invalidate_all` 위임
- `src/adapter/auth/scoped_session.rs` (신규) — `ScopedAuthSession` (`ContextSessionPort` 구현)
- `src/port/mock.rs` 변경 — `MockContextSession` 확장 (with_*_failure, with_partial_commit_failure, with_slow_rescope, transition_steps, rollback_called)

**Tests**:
- KeystoneRescopeAdapter: rescope 성공 → 새 토큰, expires_at 정본
- KeystoneRescopeAdapter: rescope 거부 (403) → `RescopeRejected`
- TokenCacheStore: store_scoped/lookup_scoped, 만료 토큰 → None
- EndpointCatalogInvalidator: 모든 등록 client invalidate 호출 검증
- ScopedAuthSession: begin/rescope/refresh/commit happy path
- ScopedAuthSession: begin OK + rescope fail → handle은 외부 mutate 없음
- ScopedAuthSession: commit 내부 자동 rollback (partial fail) — atomic 계약 검증
- MockContextSession: transition_steps 순서 ["begin","rescope","refresh","commit"|"rollback"]

**Implementation order**: 3 (Unit 2와 병렬 가능)
**PR**: PR1

---

## Unit 4: Switch Orchestration

**Responsibility**: Switcher 레이어 — state machine, resolver, switch 절차 오케스트레이터, App 통합.
**Dependencies**: Unit 1, 2, 3
**Interfaces (exposes)**:
- `src/context/state_machine.rs` — `SwitchStateMachine` (parking_lot::Mutex, `try_begin`, `commit`, `fail`, `state`)
- `src/context/resolver.rs` — `ContextTargetResolver` (`resolve(request) async`, `list_user_projects() async`)
- `src/context/switcher.rs` — `ContextSwitcher` (`switch(request) async`, `switch_back() async`) — 7-step 절차
- `src/app.rs` 변경 — `switch_context(request) async` 메서드 + ContextChanged 디스패치 + history push 통합
- `Action::SwitchContext` / `Action::SwitchBack` 처리 디스패처 라우팅 (App.handle_action)

**Tests**:
- SwitchStateMachine: try_begin 동시 호출 → 1개만 성공, 나머지 InProgress
- SwitchStateMachine: fail 후 previous snapshot 복원 가능
- ContextTargetResolver: name 단일 매치 / 충돌 (Ambiguous + 후보) / 미매치 (NotFound)
- ContextTargetResolver: cloud-prefix 형식 (`cloud/project`) 파싱
- ContextSwitcher.switch happy path (mock session) — snapshot 반환, history push
- ContextSwitcher.switch rescope 실패 → state.fail + rollback 호출
- ContextSwitcher.switch_back: history 없으면 NotFound
- ContextSwitcher: switch 동시 호출 → 두 번째는 InProgress

**Implementation order**: 4
**PR**: PR1

---

## Unit 5: Commands & Safety UI

**Responsibility**: 사용자 노출 시점의 명령 + 안전 가시성 (인디케이터 + destructive fingerprint).
**Dependencies**: Unit 4 (App.switch_context, Action variants)
**Interfaces (exposes)**:
- `src/input/command.rs` 변경 — `:switch-project`, `:switch-cloud`, `:switch-back` 명령 등록
  - 충돌 시 후보 출력 + 재선택 안내
  - Tab 자동완성 (resolver.list_user_projects)
- `src/ui/context_indicator.rs` (신규) — `ContextIndicator` 위젯 (Component impl, 패시브 타이머)
- `src/ui/status_bar.rs` 변경 — `set_context_indicator(Arc<RwLock<ContextIndicator>>)`
- `src/ui/confirm.rs` 변경 — `with_context_fingerprint(snapshot)`, `require_recontext_confirm(recently_switched)`
- destructive 액션 호출 사이트 (server delete/force-delete/evacuate, volume delete, network delete 등) 모두 fingerprint 적용

**Tests**:
- CommandParser: `:switch-project admin` → `Action::SwitchContext(ByName{project:"admin",..})`
- CommandParser: `:switch-back` → `Action::SwitchBack`
- CommandParser: 충돌 후보 출력 검증
- ContextIndicator: set_context(snapshot, mark_highlight=true) → render 시 highlight active until elapsed > duration
- ContextIndicator: highlight 만료 후 일반 스타일
- ConfirmDialog: fingerprint 라인 포함 검증
- ConfirmDialog: recently_switched=true → 재확인 강제 (typing 또는 추가 step)
- 최소 1개 destructive 액션 (server delete) 통합 테스트 — fingerprint가 다이얼로그에 표시

**Implementation order**: 5
**PR**: PR3

---

## Unit 6: Picker UI

**Responsibility**: Ctrl+P 모달 피커 + 글로벌 단축키 등록.
**Dependencies**: Unit 5 (Action::SwitchContext, ContextIndicator는 picker 결과 후 갱신됨)
**Interfaces (exposes)**:
- `src/ui/context_picker.rs` (신규) — `ContextPicker` 위젯 (Component impl, is_modal=true)
  - `open(candidates, current)`, `close()`
  - fuzzy 검색 (기존 SelectPopup 재사용 또는 확장)
  - Enter 시 `Action::SwitchContext(ContextRequest::ById{cloud, project_id})` 발행
  - 행 표시: `cloud • project • domain • project_id`
- `src/input/keymap.rs` 변경 — 글로벌 `Ctrl+P` → `Action::OpenContextPicker` (또는 직접 picker open)
- `src/app.rs` 부분 변경 — picker 컴포넌트 보유 + 모달 라우팅

**Tests**:
- ContextPicker.open: candidates 표시, current 행에 기본 선택
- ContextPicker fuzzy: 입력 "adm" → "admin" 매치
- ContextPicker Enter: 선택된 row의 ContextRequest::ById 발행
- ContextPicker Esc: close() 호출, action 미발행
- KeyMap: Ctrl+P → picker open

**Implementation order**: 6 (Unit 7과 병렬 가능)
**PR**: PR4

---

## Unit 7: Identity Module Integration

**Responsibility**: Project 모듈에 모듈-로컬 `s` 핸들러 + 16개 Resource Module의 ContextChanged 핸들러 추가.
**Dependencies**: Unit 4 (Action::SwitchContext, AppEvent::ContextChanged)
**Interfaces (exposes)**:
- `src/module/project/mod.rs` 변경 — `handle_key(s)` → `Action::SwitchContext(ContextRequest::ById{cloud:None, project_id:row.id})`
- 16개 Resource Module 변경 (server, volume, network, security_group, floating_ip, image, snapshot, flavor, host, agent, aggregate, project, user, usage, migration, compute_service):
  - `handle_event(AppEvent::ContextChanged { .. })` → 내부 `Vec<T>` 비우기 + `is_loading=true`
- `src/module/_shared/` (있으면) 또는 trait default — ContextChanged 처리 helper 제공

**Tests**:
- Project module: 행 선택 + `s` → Action::SwitchContext 발행
- Project module: Enter는 기존 Detail 진입 유지 (회귀 없음)
- 각 Resource Module: ContextChanged 수신 → Vec 비움, is_loading=true (대표로 server, volume, network 3개 명시 테스트)
- 회귀: 기존 1116 tests + 신규 unit 1~6 tests pass

**Implementation order**: 7 (Unit 6과 병렬 가능)
**PR**: PR5

---

## PR 매핑 요약

| PR | Units | 기대 산출 |
|----|-------|----------|
| PR1 | Unit 1, 2, 3, 4 | safety infra + atomic switch core (사용자 노출 0) |
| PR3 | Unit 5 | 사용자 노출 시작 — 명령 + 인디케이터 + fingerprint 동시 활성 |
| PR4 | Unit 6 | 피커 모달 |
| PR5 | Unit 7 | Identity 통합 + 모든 모듈의 ContextChanged 핸들러 |

## 실행 모드 가이드

각 unit은 TDD RED-GREEN-REFACTOR. Standard depth.
- Unit 1, 2, 3, 4 (PR1): 단일 PR로 누적. PR1 머지 시점에 통합 회귀 테스트 + Codex/Council 리뷰
- Unit 5 (PR3): 별도 PR. 머지 후 사용자가 실제 전환 사용 가능 — 문서/changelog 동반
- Unit 6, 7: 각각 PR
