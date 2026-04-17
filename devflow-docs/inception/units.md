# Units

**BL**: BL-P2-031 Keystone Rescoping
**Timestamp**: 2026-04-13T00:00:00+09:00
**Updated**:
- 2026-04-16 — T3 Runtime Wire unit 추가
- 2026-04-17 — Unit 4.5 Command Bar Integration 추가 (stub blind spot 대응)

**총 11개 단위** (PR1: Unit 1~4 ✅, **PR3: Unit 4.5 + Unit 5**, PR4: Unit 6, PR5: Unit 7, T3: Unit 8~10 ✅)

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
                                      │      ▼
                                      │   Unit 4.5 (Command Bar Integration) ──┐
                                      │                                         │
                                      │                                         ▼
                                      │                        Unit 5 (Commands & Safety UI) ─┬──> Unit 6 (Picker UI)
                                      │                                                         │
                                      │                                                         └──> Unit 7 (Identity Module)
```

Unit 2와 Unit 3는 Unit 1 완료 후 **병렬 가능**.
Unit 4.5는 Unit 4 완료 후 수행 (기존 dead-code stub 해소 — Command Bar wire).
Unit 5는 Unit 4.5 완료 후 수행 (wire 위에서 switch 명령이 실제 동작).
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
- KeystoneRescopeAdapter: rescope 성공 → 새 토큰, expires_at 정본 (loopback HTTP)
- KeystoneRescopeAdapter: rescope 거부 (403/401) → `RescopeRejected`
- KeystoneRescopeAdapter: 404/400/429/5xx → 의미별 분리 매핑 (NotFound/BadRequest/RateLimited/ServiceUnavailable)
- KeystoneRescopeAdapter: response scope ≠ target.project_id → `Api(Parse)` (review T1 B4)
- KeystoneRescopeAdapter: X-Subject-Token 부재 / JSON 파싱 실패 → `Api(Parse)` (review T1 B3)
- KeystoneRescopeAdapter: 에러 body sanitize (X-Auth-Token/Set-Cookie 마스킹 + 256자 트렁케이트)
- TokenCacheStore: store_scoped/lookup_scoped, 만료 토큰 → None
- EndpointCatalogInvalidator: 모든 등록 client invalidate 호출 검증
- ScopedAuthSession: begin/rescope/refresh/commit happy path
- ScopedAuthSession: begin OK + rescope fail → handle은 외부 mutate 없음
- ScopedAuthSession: commit 내부 자동 rollback (partial fail) — atomic 계약 검증
- ScopedAuthSession: begin이 `current_token() == None`이면 `SwitchError::Unsupported`로 거부 (review T2 C2)
- KeystoneAuthAdapter ScopedAuthPort: current_scope/current_token snapshot, set_active atomic swap, 이전 scope 토큰 보존 (rollback invariant)
- KeystoneAuthAdapter ScopedAuthPort: current_token() pre-auth → `None` (placeholder 금지, review T2 C2)
- KeystoneAuthAdapter refresh: active scope ≠ initial scope (set_active drift) → `refresh_token`이 `AuthFailed("scope drift")` 반환, demo 엔트리 무손실 (review T2 C1)
- KeystoneAuthAdapter refresh loop: drift 시 do_authenticate skip (warn log + continue) (review T2 C1)
- KeystoneAuthAdapter authenticate: 외부 credential 무시 + initial_scope로 키 — drift된 active scope에 admin 토큰 누출 차단 (review T2 S1)
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

## Unit 4.5: Command Bar Integration (신규 — 2026-04-17)

**Responsibility**: 기존에 정의만 되어 있던 `CommandParser`와 `InputBar`를 `App`에 실제로 연결한다. `:` 키 → 커맨드 모드 → 입력 수집 → Enter 시 파싱 → Action dispatch의 전체 경로를 동작 상태로 전환한다.
**Dependencies**: Unit 4 (Action::SwitchContext / Action::SwitchBack variants)
**Rationale**: 2026-04-17 CONSTRUCTION 중 발견 — `CommandParser` 및 `InputBar`가 모두 외부 콜러 없는 dead code. app.rs:256은 `:` 키를 누르면 `input_mode = InputMode::Command`로 전환만 하고, 입력 수집·파싱·dispatch가 비어 있음. Unit 5에서 추가하는 switch 명령이 실동작하려면 이 wire가 선행되어야 함.

**Interfaces (exposes)**:
- `src/app.rs` 변경:
  - `App` 필드 추가: `input_bar: crate::ui::input_bar::InputBar`, `command_parser: crate::input::command::CommandParser`
  - `InputMode::Command` 분기: InputBar로 키 위임 → `InputAction::Commit(buf)` / `AutoComplete` / `HistoryUp` / `HistoryDown` / `Cancel` 처리
  - `InputAction::Commit` 수신 시: `command_parser.parse(&buf)` → `Command` → `Action` 변환 → dispatch
  - `InputAction::AutoComplete`: `command_parser.auto_complete(buffer)` → `input_bar.set_buffer(expanded)`
  - History Up/Down: `command_parser.history_prev/next` → `input_bar.set_buffer`
  - 히스토리 save/load: `new()` + quit 플로우
- `Command → Action` 변환 헬퍼 (`command.rs` 또는 `app.rs` 내부):
  - `Command::Navigate(route)` → `Action::Navigate(route)`
  - `Command::Quit` → `self.should_quit = true`
  - `Command::Refresh` → `Action::Refresh` (존재 시) 또는 기존 refresh 경로
  - `Command::Help` → 기존 help 경로 또는 toast
  - `Command::SwitchProject(name)` → `Action::SwitchContext(ContextRequest::ByName { cloud: None, project: name })`
  - `Command::SwitchCloud(name)` → `Action::SwitchContext(ContextRequest::ByName { cloud: Some(name), project: "" | 기본 })` *(정확한 시그니처는 Unit 4 types에 맞춤)*
  - `Command::SwitchBack` → `Action::SwitchBack`
  - `Command::ContextSwitch(_)` / `Command::ContextList` — 기존 legacy 명령. PR3에서는 Unknown 또는 toast 안내로 격하 처리 가능 (결정은 Step B에서)
  - `Command::Unknown(s)` → Toast/LogPanel 에러 메시지

**Tests** (`src/app.rs::tests` + 필요 시 `command.rs`):
- `:` 입력 → `input_mode == Command` (기존 테스트 유지)
- 문자 누적: `:`, `s`, `r`, `v` → InputBar buffer == "srv" (InputBar 내부 위임 검증)
- Enter → `Command::Navigate(Route::Servers)` → Action dispatch → route 전환
- Enter empty → Normal 모드 복귀, action 없음
- Tab(`s` + Tab) → buffer가 completions 중 하나로 확장
- Up/Down → 히스토리 네비게이션 (push_history 후 Enter → Up → buffer 복원)
- Esc → 취소, buffer 비움
- `:quit` Enter → `should_quit == true`
- `:switch-project admin` Enter → `Action::SwitchContext(ContextRequest::ByName { project: "admin", .. })` dispatch
- `:switch-back` Enter → `Action::SwitchBack` dispatch
- `:foobar` Enter → Unknown → toast/log 에러 emit (구체 형태는 Step B에서)

**비대상** (별도 Unit):
- ContextIndicator (Unit 5 Step 2)
- ConfirmDialog fingerprint (Unit 5 Step 4)

**Implementation order**: 4.5
**PR**: PR3 (Unit 5와 함께)

---

## Unit 5: Commands & Safety UI

**Responsibility**: 사용자 노출 시점의 명령 + 안전 가시성 (인디케이터 + destructive fingerprint).
**Dependencies**: Unit 4 (App.switch_context, Action variants), **Unit 4.5 (Command Bar wire — switch 명령의 실제 dispatch 경로)**
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
| PR1 (#68) | Unit 1, 2, 3, 4 | safety infra + atomic switch core (사용자 노출 0) |
| PR3 | **Unit 4.5, Unit 5** | 사용자 노출 시작 — Command Bar wire + 명령 + 인디케이터 + fingerprint 동시 활성 |
| PR4 | Unit 6 | 피커 모달 |
| PR5 | Unit 7 | Identity 통합 + 모든 모듈의 ContextChanged 핸들러 |

---

# T3 Runtime Wire Units (UPDATE 2026-04-16)

**Scope**: FR-11 — switch-core를 main.rs에 실제 연결 (B3 축소 범위)
**Prerequisites**: Unit 1~4 구현 완료 (PR #68 머지됨). Unit 5~7은 후속.
**Baseline**: 1240 tests

## T3 구현 순서

```
Unit 8 (AdapterRegistry HttpEndpointCache 노출)
  └──> Unit 9 (ConfigCloudDirectory + StaticProjectDirectory)
         └──> Unit 10 (main.rs Wire + Demo Guard)
```

선형 의존 — 병렬 없음. 각 unit이 작으므로 단일 세션에서 순차 완료 가능.

---

## Unit 8: AdapterRegistry HttpEndpointCache 노출

**Responsibility**: 5개 HttpAdapter의 BaseHttpClient를 Arc로 변경하고, AdapterRegistry가 endpoint_caches()를 노출하도록 한다.
**Dependencies**: Unit 3 (HttpEndpointCache trait, EndpointCatalogInvalidator — 이미 구현됨)
**Interfaces (exposes)**:
- `src/adapter/http/nova.rs` 변경 — `base: Arc<BaseHttpClient>` + `pub fn from_base(base: Arc<BaseHttpClient>) -> Self`
- `src/adapter/http/neutron.rs` 변경 — 동일 패턴
- `src/adapter/http/cinder.rs` 변경 — 동일 패턴
- `src/adapter/http/glance.rs` 변경 — 동일 패턴
- `src/adapter/http/keystone.rs` 변경 — 동일 패턴
- `src/adapter/registry.rs` 변경:
  - `http_caches: Vec<Arc<dyn HttpEndpointCache>>` 필드 추가
  - `new_http`: BaseHttpClient를 Arc로 먼저 생성 → adapter에 from_base 전달 + http_caches Vec 보유
  - `pub fn endpoint_caches(&self) -> Vec<Arc<dyn HttpEndpointCache>>`
  - `new_mock()`: `http_caches: Vec::new()` 추가

**Tests**:
- AdapterRegistry::new_http: endpoint_caches().len() == 5
- AdapterRegistry::new_mock: endpoint_caches().is_empty()
- 각 HttpAdapter from_base: 기존 API 호출이 Deref로 동일 동작 (대표로 Nova 1개)
- 기존 new() 생성자 하위 호환 유지 (호출 사이트 깨짐 없음)
- EndpointCatalogInvalidator::new(registry.endpoint_caches()) → invalidate_all 호출 성공
- 회귀: 1240 tests pass

**Implementation order**: 8
**PR**: T3 PR (단일 PR)

---

## Unit 9: ConfigCloudDirectory + StaticProjectDirectory

**Responsibility**: Config 래퍼 2개를 구현하여 ContextTargetResolver에 주입 가능하게 한다.
**Dependencies**: Unit 8 (불필요하나 T3 PR 내 순서 유지), Unit 4 (CloudDirectory/ProjectDirectoryPort trait — 이미 구현됨)
**Interfaces (exposes)**:
- `src/context/config_cloud_directory.rs` (신규) — `ConfigCloudDirectory { new(Arc<Config>) }`, `CloudDirectory` impl
- `src/context/static_project_directory.rs` (신규) — `StaticProjectDirectory { new(Arc<Config>) }`, `ProjectDirectoryPort` impl
- `src/context/mod.rs` 변경 — 두 모듈 pub re-export

**Tests**:
- ConfigCloudDirectory: active_cloud() == config.active_cloud_name()
- ConfigCloudDirectory: known_clouds() == config.cloud_names() (순서 무관)
- StaticProjectDirectory: list_projects("existing_cloud") → project_name 1건 반환
- StaticProjectDirectory: project_id == project_name (placeholder 검증 + PLACEHOLDER 주석)
- StaticProjectDirectory: project_name 없는 cloud → 빈 목록
- StaticProjectDirectory: 존재하지 않는 cloud → 빈 목록
- ContextTargetResolver::new(ConfigCloudDirectory, StaticProjectDirectory) → resolve 성공 (통합)
- 회귀: 1240 + Unit 8 tests pass

**Implementation order**: 9
**PR**: T3 PR (단일 PR)

---

## Unit 10: main.rs Wire + Demo Guard

**Responsibility**: main.rs에 3-phase wire 시퀀스를 삽입하여 ContextSwitcher를 App에 연결한다. Demo 모드 무회귀 보장.
**Dependencies**: Unit 8, 9
**Interfaces (exposes)**:
- `src/main.rs` 변경:
  - Phase A (config move 전): `config.clone()` → `Arc::new()`, credential 복제
  - Phase B (registry 후 worker 전): `registry.endpoint_caches()` 수집
  - Phase C (worker 후): wire 시퀀스 ④~⑫ + `app.wire_context_switch(switcher, event_tx)`
- KeystoneRescopeAdapter: `reqwest::Client::builder().timeout(30s).connect_timeout(10s)` 사용
- TokenCacheStore: `token_cache::compute_cloud_key(auth_url, username)` 직접 호출

**Tests**:
- 컴파일 성공 (NFR-6 — 타입 매칭 컴파일 타임 검증)
- `--demo` 모드: app.switcher == None (NFR-7)
- `--demo` 모드: 기존 동작 변경 없음 (회귀)
- 실제 모드: app.switcher == Some(_) (wire 성공 검증)
- 실제 모드: SwitchContext 액션 dispatch → switcher.switch 호출 경로 존재 (spawn_switch 테스트는 기존 Unit 4에 있음)
- 회귀: 1240 + Unit 8, 9 tests pass

**주의**: main.rs 통합 테스트는 실제 Keystone 없이 수행. mock auth_provider + mock registry를 사용하는 별도 integration test 또는 App 생성 후 wire 검증으로 대체.

**Implementation order**: 10
**PR**: T3 PR (단일 PR)

---

## T3 PR 매핑

| PR | Units | 기대 산출 |
|----|-------|----------|
| T3 PR | Unit 8, 9, 10 | switch-core가 main.rs에 실제 연결. demo 무회귀. 사용자 노출: 아직 없음 (명령/피커는 Unit 5~7) |

## 전체 PR 매핑 (갱신)

| PR | Units | 상태 |
|----|-------|------|
| PR1 (#68) | Unit 1, 2, 3, 4 | ✅ 머지 완료 |
| **T3 PR** | **Unit 8, 9, 10** | **← 현재 세션 대상** |
| PR3 | Unit 5 | 후속 |
| PR4 | Unit 6 | 후속 |
| PR5 | Unit 7 | 후속 |

## 실행 모드 가이드

각 unit은 TDD RED-GREEN-REFACTOR. Standard depth.
- Unit 1, 2, 3, 4 (PR1): ✅ 완료
- **Unit 8, 9, 10 (T3 PR)**: 단일 PR로 누적. 순차 실행. 완료 후 통합 회귀 + Codex 리뷰
- Unit 5 (PR3): 후속 — 사용자 노출 시작
- Unit 6, 7: 각각 PR
