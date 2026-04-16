# Application Design

**Mode**: LIST (목록 단계, Council 리뷰 반영 개정)
**Timestamp**: 2026-04-13T00:00:00+09:00
**BL**: BL-P2-031 Keystone Rescoping
**Revision**: r3 (T3 Runtime Wire UPDATE — 2026-04-16)

## 의존 방향 원칙

- **App** (Controller, orchestrator) **owns** **ContextSwitcher**.
- **ContextSwitcher**는 commit 결과로 `(new_epoch, ContextSnapshot)` 반환. App이 epoch를 반영하고 `AppEvent::ContextChanged`를 디스패치.
- **ContextSwitcher**는 `ContextSessionPort`를 통해 atomic begin/commit/rollback 수행.
- **Worker spawn API**는 `(epoch, CancellationToken)` 페어를 반드시 받는다 (시그니처 강제).

## 컴포넌트 목록

### 신규 컴포넌트 (12개)

| 컴포넌트 | 책임 | 타입 | PR |
|---------|------|------|-----|
| `ContextEpoch` | 단조 증가 epoch 카운터, stale 이벤트 식별 키 | Util | PR1 |
| `CancellationRegistry` | 활성 폴링/장기 fetch에 대한 `CancellationToken` 등록·일괄 취소 | Service | PR1 |
| `VersionedEvent<T>` envelope | `{ event: T, epoch: u64 }` — Action/AppEvent를 감싸 epoch stamp (variant 폭증 회피) | Util | PR1 |
| `AppEvent::ContextChanged { target }` | UI에 컨텍스트 변경 통지 → 모듈 컴포넌트가 내부 데이터 비움 | Event variant | PR1 |
| `SwitchStateMachine` | `Idle → Switching → Committed | Failed` 전환 상태, rollback 규약 | Service | PR1 |
| `ContextSwitcher` | 전환 절차 오케스트레이터 (epoch++ → cancel → session begin → rescope → catalog 무효화 → commit) | Service | PR1 |
| `ContextSessionPort` (trait) | atomic begin/commit/rollback 인터페이스. `invalidate_all_endpoints()` hook | Port | PR1 |
| `ScopedAuthSession` | `ContextSessionPort` 구현체. 활성 scoped token + endpoint cache 일괄 관리 | Service | PR1 |
| `KeystoneRescopeAdapter` | Keystone v3 token-method scoped exchange 호출, expires_at 정본 사용 | Adapter | PR1 |
| `EndpointCatalogInvalidator` | 모든 HTTP client의 endpoint cache 일괄 무효화 (`src/adapter/http/base.rs` 자동화) | Service | PR1 |
| `ContextTargetResolver` | name/uuid/cloud-prefix → ContextTarget 변환, 충돌 disambiguation. 명령·피커·모듈 액션 공유 | Service | PR1 |
| `ContextHistoryStore` | switch-back 1단계 히스토리 (ContextSnapshot 저장), rollback 시 동일 사용 | Util | PR1 |
| `ContextIndicator` | 영구 컨텍스트 표시 위젯 (`cloud / project [/ domain]`), 전환 직후 강조 | UI Widget | PR3 |
| `ContextPicker` | Ctrl+P 모달, 프로젝트/클라우드 fuzzy 검색·선택 (`cloud • project • domain • project_id` 행) | UI Widget (modal) | PR4 |

> 신규 컴포넌트 14개 (PR1 인프라 12 + UI 2). `AppEvent::ContextChanged`는 enum variant 추가지만 표에 분리 표기.

### 변경 컴포넌트 (8개)

| 컴포넌트 | 변경 책임 | 타입 | PR |
|---------|----------|------|-----|
| `App` (src/app.rs) | 활성 cloud/project 컨텍스트 보유, ContextSwitcher 소유, epoch/스냅샷 반영, ContextChanged 디스패치 | Controller (재분류) | PR1 |
| `Worker` (src/worker.rs) | spawn API에 `(epoch, CancellationToken)` 페어 강제, `tokio::select!` cancel branch + epoch 검증 | Service | PR1 |
| `Action` (src/action.rs) | `VersionedEvent<Action>` envelope 적용 (또는 `Action::with_epoch`) | Type | PR1 |
| `AppEvent` (src/event.rs) | `VersionedEvent<AppEvent>` envelope 적용 + `ContextChanged` variant 추가 | Type | PR1 |
| `CommandParser` (src/input/command.rs) | `:switch-project <name|uuid|cloud/project>`, `:switch-cloud`, `:switch-back` 등록 + tab 자동완성. `ContextTargetResolver` 사용 | Controller | PR3 |
| `ConfirmDialog` (src/ui/confirm.rs) | destructive confirm에 `cloud • project` fingerprint 표시. 컨텍스트 변경 직후엔 추가 확인 강제 | UI Widget | PR3 |
| `StatusBar` (src/ui/status_bar.rs) | ContextIndicator 임베드 위치/우선순위 조정 | UI Widget | PR3 |
| `TokenCacheStore` (src/adapter/auth/token_cache.rs) | rescoped 토큰을 scope별로 저장/조회 (BL-P2-029 기반), 만료 시 재취득 | Repository | PR1 |
| `Project Module` (src/module/project/mod.rs) | 모듈-로컬 `s` 액션 핸들러. KeyMap 글로벌 등록 회피 (Enter는 Detail 유지) | Controller | PR5 |

> 변경 컴포넌트 9개. `Action`/`AppEvent`는 type 정의 변경이라 별도 표기.

### Mock / 테스트 시즘 (1개)

| 컴포넌트 | 책임 | 타입 | PR |
|---------|------|------|-----|
| `MockContextSession` (확장) | `src/port/mock.rs`에 추가. rescope 성공/실패, catalog invalidate 부분 실패 시뮬레이션 (fault-injection seam) | Mock | PR1 |

**총 24개 컴포넌트** (신규 14 + 변경 9 + 테스트 1)

## 개정된 PR 매핑

| PR | 컴포넌트 | Depends on | 사용자 노출 |
|----|---------|-----------|-----------|
| **PR1 (safety infra + switch core)** | ContextEpoch, CancellationRegistry, VersionedEvent, AppEvent::ContextChanged, SwitchStateMachine, ContextSwitcher, ContextSessionPort, ScopedAuthSession, KeystoneRescopeAdapter, EndpointCatalogInvalidator, ContextTargetResolver, ContextHistoryStore, App 통합, Worker 시그니처 개정, Action/AppEvent envelope, TokenCacheStore 확장, MockContextSession | — | 없음 (인프라만) |
| **PR3 (안전 가시성 + 명령)** | ContextIndicator, StatusBar 임베드, ConfirmDialog fingerprint, CommandParser 확장 | PR1 | `:switch-*` 명령, 컨텍스트 인디케이터, fingerprint confirm |
| **PR4 (피커 UI)** | ContextPicker, KeyMap 글로벌 단축키 (Ctrl+P) | PR3 | Ctrl+P 모달 |
| **PR5 (Identity 통합)** | Project Module 모듈-로컬 `s` 핸들러 | PR3 | Identity 리스트 `s` 단축키 |

→ **PR 수: 6 → 4로 축소**. PR1 통합으로 stale 누설 창 제거. PR3에 안전 가시성 + 명령을 묶어 사용자가 전환을 쓸 수 있는 첫 시점부터 인디케이터/fingerprint가 함께 동작.

## NFR 매핑

| NFR | 보장 컴포넌트 |
|-----|--------------|
| NFR-1 안전성 (atomic switch, stale 차단) | ContextEpoch + CancellationRegistry + SwitchStateMachine + ContextSessionPort/ScopedAuthSession + EndpointCatalogInvalidator |
| NFR-2 성능 (1초 이내) | KeystoneRescopeAdapter (네트워크 왕복 측정), EndpointCatalogInvalidator (lazy refresh) |
| NFR-3 테스트 커버리지 | MockContextSession (port mock), ContextSwitcher 통합 테스트, 부분실패 (rescope OK + invalidate fail) 시뮬레이션 |
| NFR-4 UX 일관성 | ContextPicker (기존 SelectPopup 재사용), CommandParser 확장 (기존 패턴), ContextTargetResolver (단일 disambiguation 로직) |
| NFR-5 관측성 | ContextSwitcher의 `tracing` 이벤트 (epoch, target cloud/project, 결과, 소요 시간) |

## 개정 사유 요약 (Council 리뷰 반영)

| 변경 | 출처 | 이유 |
|------|------|------|
| ContextSessionPort + ScopedAuthSession 추가 | Codex | atomic begin/commit/rollback 부재 시 rescope 성공 + stale endpoint 호출 사고 가능 |
| EndpointCatalogInvalidator 추가 | Codex | `src/adapter/http/base.rs:66`의 매뉴얼 invalidate를 자동화해야 일관성 보장 |
| ContextHistoryStore 추가 | Codex+Claude | switch-back/rollback 공통 저장소 누락 |
| ContextTargetResolver 추가 | Codex | 명령·피커·모듈 액션이 같은 disambiguation 로직 필요 (3중 구현 회피) |
| AppEvent::ContextChanged 추가 | Gemini | epoch만으로는 잔존 데이터 표시 방지 불가 |
| VersionedEvent envelope | Gemini | epoch를 매 variant에 추가하지 않고 plumbing |
| ContextIndicator/Picker → UI Widget | Codex | Component trait 위젯이 정확. Controller는 라우팅 의미 |
| App → Controller | Codex | 코드베이스 실제 역할이 orchestrator/router |
| CommandRegistry → CommandParser 확장 | Codex | src/input/command.rs 실제 명명 일치 |
| PR1+PR2 통합 | Claude+Codex | PR1만 머지 시 사용자 노출 0이지만 stale 누설 창 발생. 통합으로 창 제거 |
| PR5 ContextIndicator/fingerprint를 PR3로 앞당김 | Codex | PR3/4에서 명령/피커가 안전 가시성 없이 노출되는 위험 차단 |
| PR6 KeyMap 분리 → PR5 모듈-로컬 | Claude+Codex | KeyMap 글로벌 동시 수정 충돌 회피, `s` 의미가 모듈 의존적 |
| MockContextSession 명시 | Claude | port mock 확장 누락 보완 |

---

# DETAIL Mode r2 (Standard depth — 메타 리뷰 반영)

**Timestamp**: 2026-04-13T00:00:00+09:00
**Revision**: r2 (Council 메타 리뷰의 6개 교정 + 4개 체크리스트 추가 반영 — design-review-raw/codex-meta.md, gemini-meta.md 참조)

## 핵심 타입

```rust
// src/context.rs (신규 모듈)
pub type Epoch = u64;

/// 사용자 입력 (CommandParser / Picker 출력). 미해결 상태.
#[derive(Debug, Clone)]
pub enum ContextRequest {
    ByName { cloud: Option<String>, project: String, domain: Option<String> },
    ById   { cloud: Option<String>, project_id: String },
}

/// Resolver를 통과한 권위 있는 타겟. 모든 식별자가 채워진 상태.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct ContextTarget {
    pub cloud: String,
    pub project_id: String,
    pub project_name: String,
    pub domain: String,
}

impl From<&ContextTarget> for TokenScope {
    fn from(t: &ContextTarget) -> Self {
        TokenScope::Project { name: t.project_name.clone(), domain: t.domain.clone() }
    }
}

/// 컨텍스트 전환 결과. self-verifying (epoch 포함).
#[derive(Debug, Clone)]
pub struct ContextSnapshot {
    pub target: ContextTarget,
    pub epoch: Epoch,
    pub token: Token,
    pub token_scope: TokenScope,
    pub captured_at: chrono::DateTime<chrono::Utc>,
}

#[derive(Debug, Clone, thiserror::Error)]
pub enum SwitchError {
    #[error("switch already in progress")]    InProgress,
    #[error("rescope rejected: {0}")]          RescopeRejected(String),
    #[error("catalog refresh failed: {0}")]    CatalogFailed(String),
    #[error("commit failed: {0}")]             CommitFailed(String),
    #[error("ambiguous target")]               Ambiguous { candidates: Vec<ContextTarget> },
    #[error("target not found: {0}")]          NotFound(String),
    #[error(transparent)]                      Api(#[from] ApiError),
    #[error(transparent)]                      Io(#[from] std::io::Error),
}

/// Port 계약: switch는 atomic. begin → rescope → refresh → commit 중 어느 단계에서
/// 실패해도 caller가 rollback(handle)을 호출하면 이전 상태로 완전 복원.
/// commit 자체는 self-reverting (commit 내부에서 실패 시 자동 rollback 후 에러 반환).
pub struct SessionHandle {
    pub(crate) epoch: Epoch,
    pub(crate) target: ContextTarget,
    pub(crate) previous_token: Token,
    pub(crate) previous_scope: TokenScope,
    // 내부 staging 필드는 비공개
    pub(crate) staged_new_token: Option<Token>,
    pub(crate) staged_catalog: Option<ServiceCatalog>,
}
```

## 신규 컴포넌트 상세

### ContextEpoch (Util)
**Responsibility**: App의 권위 epoch 카운터.
**Interface**:
- `pub fn new() -> Self` — `AtomicU64::new(0)` 내부 보유
- `pub fn current(&self) -> Epoch`
- `pub fn bump(&self) -> Epoch` — atomic increment, 새 값 반환

### CancellationRegistry (Service)
**Responsibility**: 활성 작업 토큰 등록·일괄 취소.
**Interface**:
- `pub fn register(&self, epoch: Epoch) -> CancellationToken`
- `pub fn cancel_below(&self, threshold: Epoch) -> usize` — idempotent, 두 번 호출 안전
- `pub fn active_count(&self) -> usize`
**Dependencies**: `tokio_util::sync::CancellationToken`

### VersionedEvent\<T\> (Util)
**Interface**:
- `pub fn new(payload: T, epoch: Epoch) -> Self`
- `pub fn epoch(&self) -> Epoch`
- `pub fn into_inner(self) -> T`
**용도**: `VersionedEvent<Action>`, `VersionedEvent<AppEvent>` 양쪽에 사용.

### AppEvent::ContextChanged variant
**Variant 정의**:
```rust
pub enum AppEvent {
    // ... 기존 ...
    ContextChanged { target: ContextTarget },   // epoch는 envelope이 보유, 중복 제거
}
```
**핸들링 컨트랙트 (ContextChanged whitelist)**: 다음 모듈 컴포넌트는 `handle_event(AppEvent::ContextChanged)`에서 내부 데이터를 반드시 비워야 한다. 외 컴포넌트는 default no-op.
- 모든 `<Resource>Module` (server, volume, network, security_group, floating_ip, image, snapshot, flavor, host, agent, aggregate, project, user, usage, migration, compute_service)
- `ResourceListView`, `DetailView`, 모든 `Form` 컴포넌트의 in-flight 입력 상태
- StatusBar의 컨텍스트 의존 위젯 (refresh)

### SwitchStateMachine (Service)
**Responsibility**: `Idle → Switching → Committed | Failed` 전이. 동시 호출 상호배제.
**동기화 계약**: `Mutex<SwitchState>` 보유 → `&self` 메서드. Switcher가 `Arc<SwitchStateMachine>`으로 보유.
**Interface**:
```rust
pub struct SwitchStateMachine {
    state: parking_lot::Mutex<SwitchState>,
    history: Arc<parking_lot::Mutex<ContextHistoryStore>>,
    epoch: Arc<ContextEpoch>,
}

pub enum SwitchState {
    Idle { current: Option<ContextSnapshot> },
    Switching { target: ContextTarget, started_at_epoch: Epoch },
    Committed { snapshot: ContextSnapshot },
    Failed { previous: ContextSnapshot, err: SwitchError },
}

impl SwitchStateMachine {
    pub fn try_begin(&self, target: ContextTarget) -> Result<Epoch, SwitchError>; // InProgress on busy
    pub fn commit(&self, snapshot: ContextSnapshot);
    pub fn fail(&self, err: SwitchError);
    pub fn state(&self) -> SwitchStateView;  // read-only snapshot
}
```
**Cancel 정책 (확정)**: Switching 중 신규 switch/cancel 요청 → 즉시 `SwitchError::InProgress` 반환. 협조적 cancel은 본 BL 비포함 (후속).

### ContextSwitcher (Service)
**Responsibility**: state + epoch + cancel + session 4개만 오케스트레이트.
**Dependencies**: `SwitchStateMachine`, `CancellationRegistry`, `dyn ContextSessionPort`, `ContextHistoryStore`, `ContextEpoch`
**Interface**:
```rust
pub async fn switch(&self, request: ContextRequest) -> Result<(Epoch, ContextSnapshot), SwitchError>;
pub async fn switch_back(&self) -> Result<(Epoch, ContextSnapshot), SwitchError>;
```

**switch 절차** (개정 — 컴파일 가능):
```rust
pub async fn switch(&self, request: ContextRequest) -> Result<(Epoch, ContextSnapshot), SwitchError> {
    // 1. Resolve user input → authoritative target
    let target = self.resolver.resolve(request).await?;

    // 2. epoch bump + state Switching (Mutex로 동시 호출 차단)
    let new_epoch = self.state.try_begin(target.clone())?;

    // 3. 이전 epoch 작업 일괄 cancel (idempotent)
    self.cancellation.cancel_below(new_epoch);

    // 4. session.begin (handle에 previous_token/scope 캡처)
    let mut handle = match self.session.begin(&target, new_epoch).await {
        Ok(h) => h,
        Err(e) => { self.state.fail(e.clone()); return Err(e); }
    };

    // 5. 명시적 async sequencing — rescope → refresh_catalog
    if let Err(e) = self.run_transition(&mut handle).await {
        let _ = self.session.rollback(handle).await;
        self.state.fail(e.clone());
        return Err(e);
    }

    // 6. commit (port 계약: 실패 시 self-reverting, 추가 rollback 호출 불필요)
    let snapshot = match self.session.commit(handle).await {
        Ok(s) => s,
        Err(e) => { self.state.fail(e.clone()); return Err(e); }
    };

    // 7. state.commit + history push + 최종 cancel 안전망
    self.state.commit(snapshot.clone());
    self.history.lock().push(snapshot.clone());
    self.cancellation.cancel_below(new_epoch);

    Ok((new_epoch, snapshot))
}

async fn run_transition(&self, handle: &mut SessionHandle) -> Result<(), SwitchError> {
    self.session.rescope(handle).await?;
    self.session.refresh_catalog(handle).await?;
    Ok(())
}
```

### ContextSessionPort (Port trait)
**계약**:
- `begin/rescope/refresh_catalog`는 staging만 (외부 상태 미변경)
- `commit`은 atomic — 내부에서 ScopedAuthPort.set_active_scope, EndpointCatalogInvalidator.invalidate, TokenCacheStore.store_scoped를 모두 수행. 어느 단계가 실패하면 commit 내부에서 자동 rollback 후 `CommitFailed` 반환
- `rollback`은 begin/rescope/refresh 단계에서 caller가 명시 호출 (commit 후엔 호출 금지)
```rust
#[async_trait]
pub trait ContextSessionPort: Send + Sync {
    async fn begin(&self, target: &ContextTarget, epoch: Epoch) -> Result<SessionHandle, SwitchError>;
    async fn rescope(&self, handle: &mut SessionHandle) -> Result<(), SwitchError>;
    async fn refresh_catalog(&self, handle: &mut SessionHandle) -> Result<(), SwitchError>;
    async fn commit(&self, handle: SessionHandle) -> Result<ContextSnapshot, SwitchError>;
    async fn rollback(&self, handle: SessionHandle);
}
```

### ScopedAuthSession (Service — ContextSessionPort impl)
**Responsibility**: trait 구현. 토큰 영속성 단일 소유: **TokenCacheStore가 토큰 저장, ScopedAuthPort는 활성 scope 표시만**. 이중 소유 회피.
**Dependencies**: `Arc<dyn ScopedAuthPort>`, `Arc<KeystoneRescopeAdapter>`, `Arc<EndpointCatalogInvalidator>`, `Arc<TokenCacheStore>`

### ScopedAuthPort (Port trait — 신설)
**Responsibility**: 활성 scope/token mutation 전용. AuthProvider는 read 지향 유지.
```rust
#[async_trait]
pub trait ScopedAuthPort: Send + Sync {
    fn current_scope(&self) -> TokenScope;
    fn current_token(&self) -> Option<Token>;
    async fn set_active(&self, scope: TokenScope, token: Token) -> Result<(), SwitchError>;
}
```
**`current_token` 반환이 `Option`인 이유 (BL-P2-031 Unit 3b T2 review C2)**: pre-auth 상태에서 placeholder 토큰을 반환하면 `ScopedAuthSession::begin`이 이를 `previous_token`으로 캡처 → rollback 시 `set_active(prev_scope, empty_token)`로 토큰 캐시에 빈 엔트리 주입 → 후속 refresh에서 INITIAL credential 재인증 트리거 (C1 경로). `Option<Token>`은 호출자가 `None`을 명시적으로 다루도록 강제 — `ScopedAuthSession::begin`은 `None` 시 `SwitchError::Unsupported("no active token")`로 즉시 거부.

**구현**: `KeystoneAuthAdapter`가 기존 `AuthProvider` 외에 이 trait도 구현. 내부 `Arc<std::sync::RwLock<HashMap<TokenScope, Token>>>` + `Arc<std::sync::Mutex<TokenScope>>` 활용. **refresh 책임 분리 (C1 review)**: `KeystoneAuthAdapter`의 `refresh_token`/background refresh loop는 `self.credential` 기반이라 INITIAL scope 토큰만 발급 가능. set_active로 active scope가 drift된 상태에서 refresh를 시도하면 `is_refresh_safe(active, initial)` 가드가 차단 (`AuthFailed("scope drift")`). rescoped 토큰의 자동 refresh는 `KeystoneRescopePort` 기반의 별도 컴포넌트 책임 — 현재는 외부 트리거에 의존 (BL-P2-052 후속).

마찬가지로 `AuthProvider::authenticate(_credential)`도 외부 credential 파라미터를 무시하고 `self.credential` 사용 + `initial_scope`로 키 (S1 review): 외부 forged credential이나 drift된 active scope로 인한 admin 토큰의 demo 키 저장을 차단.

### KeystoneRescopeAdapter (Adapter)
**Interface**:
- `pub async fn rescope(&self, current_token: &Token, target: &ContextTarget) -> Result<Token, SwitchError>`
- `pub async fn discover_capabilities(&self) -> Result<KeystoneCapabilities, SwitchError>` — `/v3` discovery
**Capabilities 정의**:
```rust
#[derive(Debug, Clone)]
pub struct KeystoneCapabilities {
    pub allow_rescope_scoped_token: bool,
    pub auth_method: AuthMethod,
    pub api_version: KeystoneVersion,
}
pub enum AuthMethod { Password, Token, AppCredential }
pub enum KeystoneVersion { V3 }
```

### EndpointCatalogInvalidator (Service)
**Interface**:
- `pub fn invalidate_all(&self) -> Result<(), SwitchError>`
- `pub async fn refresh_from(&self, token: &Token) -> Result<ServiceCatalog, SwitchError>`
**Dependencies**: `AdapterRegistry` (모든 `Arc<dyn HttpEndpointCache>` 보유)

### HttpEndpointCache (trait — 신설)
```rust
pub trait HttpEndpointCache: Send + Sync {
    fn invalidate(&self);
}
```
모든 `BaseHttpClient`가 구현.

### ContextTargetResolver (Service)
**Interface**:
- `pub async fn resolve(&self, request: ContextRequest) -> Result<ContextTarget, SwitchError>` — Ambiguous/NotFound 처리
- `pub async fn list_user_projects(&self) -> Result<Vec<ContextTarget>, SwitchError>` — `/v3/auth/projects`
**Dependencies**: `ConfigLoader`, `KeystoneRescopeAdapter`

### ContextHistoryStore (Util)
**Interface**:
- `pub fn push(&mut self, snapshot: ContextSnapshot)` — 최신 1개 유지
- `pub fn previous(&self) -> Option<&ContextSnapshot>`
- `pub fn pop_previous(&mut self) -> Option<ContextSnapshot>`

### ContextIndicator (UI Widget — 패시브 타이머)
**Interface**:
```rust
pub struct ContextIndicator {
    snapshot: Option<ContextSnapshot>,
    last_switch_at: Option<Instant>,
    highlight_duration: Duration,
}
impl ContextIndicator {
    pub fn new(highlight_duration: Duration) -> Self;
    pub fn set_context(&mut self, snapshot: &ContextSnapshot, mark_highlight: bool);
}
impl Component for ContextIndicator {
    fn render(&self, frame, area) {
        // 매 render마다 비교: Instant::now() - last_switch_at < highlight_duration
        let highlighting = self.last_switch_at
            .map_or(false, |t| t.elapsed() < self.highlight_duration);
        // ...
    }
}
```
**Tick 의존성 명시**: 강조가 자동 종료되려면 highlight_duration 이내에 redraw가 발생해야 함. App의 idle redraw 정책이 없다면 `AppEvent::Tick` 또는 short-interval refresh 필요.

### ContextPicker (UI Widget — modal)
**Interface (impl Component, is_modal=true)**:
- `pub fn open(&mut self, candidates: Vec<ContextTarget>, current: Option<&ContextTarget>)`
- `pub fn close(&mut self)`
- `fn handle_key(&mut self, key) -> Option<Action>` — Enter 시 `Action::SwitchContext(ContextRequest::ById { cloud, project_id })` 발행
**Dependencies**: `SelectPopup`, `Theme`

### MockContextSession (Mock — port impl, src/port/mock.rs 확장)
**Interface (fault-injection)**:
- `with_begin_failure(self, err: SwitchError) -> Self`
- `with_rescope_failure(self, err: SwitchError) -> Self`
- `with_refresh_failure(self, err: SwitchError) -> Self`
- `with_commit_failure(self, err: SwitchError) -> Self`
- `with_partial_commit_failure(self) -> Self` — commit 내부 자동 rollback 검증
- `with_slow_rescope(self, delay: Duration) -> Self`
**관측**:
- `transition_steps(&self) -> Vec<&'static str>` — ["begin","rescope","refresh","commit"|"rollback"] 순서 검증
- `rollback_called(&self) -> bool`
- `captured_targets(&self) -> Vec<ContextTarget>`

## 변경 컴포넌트 상세

### App (src/app.rs) — Controller (재분류)
**추가**:
- `current_epoch: Arc<ContextEpoch>` (단일 권위)
- `pub fn current_context(&self) -> &ContextSnapshot`
- `pub async fn switch_context(&self, request: ContextRequest) -> Result<(), SwitchError>`
- **단일 epoch 게이트** (디스패처):
  ```rust
  fn handle_versioned_event(&mut self, ev: VersionedEvent<AppEvent>) {
      if ev.epoch() < self.current_epoch.current() { return; } // stale drop
      // dispatch
  }
  ```

### Worker (src/worker.rs)
**시그니처 통일**:
```rust
pub async fn run_worker(
    mut action_rx: mpsc::UnboundedReceiver<VersionedEvent<Action>>,
    event_tx: mpsc::UnboundedSender<VersionedEvent<AppEvent>>,
    cancellation: Arc<CancellationRegistry>,
)
```
모든 spawn은 `(epoch, CancellationToken)` 페어 사용:
```rust
pub fn spawn_versioned<F, T>(
    cancel: CancellationToken,
    epoch: Epoch,
    event_tx: &mpsc::UnboundedSender<VersionedEvent<AppEvent>>,
    fut: F,
)
where F: Future<Output = T> + Send + 'static, T: Into<AppEvent> + Send + 'static
{
    tokio::spawn(async move {
        tokio::select! {
            _ = cancel.cancelled() => {}
            ev = fut => { let _ = event_tx.send(VersionedEvent::new(ev.into(), epoch)); }
        }
    });
}
```

### Action / AppEvent (src/action.rs, src/event.rs) — Type
- 모든 외부 발행: `VersionedEvent<Action>` / `VersionedEvent<AppEvent>` envelope
- 신규 variant: `Action::SwitchContext(ContextRequest)`, `Action::SwitchBack`, `AppEvent::ContextChanged { target: ContextTarget }`

### CommandParser (src/input/command.rs)
**추가 명령**:
- `:switch-project <name|uuid|cloud/project>` → `Action::SwitchContext(ContextRequest::ByName | ById)`
- `:switch-cloud <name>` → `Action::SwitchContext(ContextRequest::ByName { cloud: Some, project: 기본 })`
- `:switch-back` → `Action::SwitchBack`
- 충돌 시 Resolver의 `Ambiguous` → 후보 출력
- Tab 자동완성: `resolver.list_user_projects().await`

### ConfirmDialog (src/ui/confirm.rs)
**추가**:
- `pub fn with_context_fingerprint(self, snapshot: &ContextSnapshot) -> Self` — `cloud • project` 라인
- `pub fn require_recontext_confirm(self, recently_switched: bool) -> Self`

### StatusBar (src/ui/status_bar.rs)
- `pub fn set_context_indicator(&mut self, indicator: Arc<RwLock<ContextIndicator>>)`

### TokenCacheStore (src/adapter/auth/token_cache.rs)
**추가 시그니처** (TokenScope 키 사용 — 기존 모델 일치):
- `pub fn store_scoped(&self, scope: &TokenScope, token: &Token) -> io::Result<()>`
- `pub fn lookup_scoped(&self, scope: &TokenScope) -> Option<Token>` — 만료 시 `None`

### Project Module (src/module/project/mod.rs)
- 모듈-로컬 `s` 키 → `Action::SwitchContext(ContextRequest::ById { cloud: None, project_id: 행.id })`
- KeyMap 글로벌 등록 회피

## 의존 그래프 (개정)

```
                      App (Controller)
                       |  current_epoch (AtomicU64 권위)
                       |  switch_context()
                       v
                  ContextSwitcher (Service)
                  /    |    |     \
                 v     v    v      v
            State   Cancel Resolver ContextSessionPort (trait)
                                       |
                                       | impl
                                       v
                                ScopedAuthSession (Service)
                                /     |       |        \
                               v      v       v         v
                       KeystoneRescope ScopedAuth EndpointCatalog TokenCacheStore
                         Adapter         Port      Invalidator       (Repository)
                                          |               |
                                          | impl          | reads
                                          v               v
                                  KeystoneAuthAdapter   AdapterRegistry
                                                            |
                                                            v
                                                      HttpEndpointCache trait
```

ContextSwitcher 협력자 4개 (state + cancel + resolver + session). atomic은 단일 port에 집중.

## DETAIL r2 적용 체크리스트 (21개) — 모두 반영 완료

1. ContextSessionPort에 begin/rescope/refresh_catalog/commit/rollback 정의
2. ContextSwitcher.switch 절차 (컴파일 가능 코드)
3. SwitchError 재정의 (CommitFailed 추가, transparent ApiError/IoError)
4. SessionHandle 정의 (previous_token, previous_scope 포함)
5. ContextSnapshot에 epoch + token 추가
6. AppEvent::ContextChanged에서 epoch 제거
7. ScopedAuthPort 신설
8. HttpEndpointCache trait 신설
9. KeystoneCapabilities 정의
10. App에 current_epoch + dispatcher epoch gate
11. Worker 시그니처: `VersionedEvent<Action>` rx + `VersionedEvent<AppEvent>` tx, spawn_versioned 헬퍼
12. ContextIndicator 패시브 타이머 (last_switch_at + render check) + tick 의존성 명시
13. ContextChanged 핸들링 컨트랙트: 모듈 화이트리스트 (16개 Resource Module)
14. TokenCacheStore: TokenScope 키 시그니처
15. ContextTargetResolver.resolve / list_user_projects async
16. Switching 정책: 신규 switch/cancel → InProgress
17. MockContextSession 시즘 확장 (with_*_failure, transition_steps, rollback_called)
18. ContextRequest vs ContextTarget 타입 분리
19. commit 실패 시 atomic 계약: port self-reverting (commit 내부 자동 rollback)
20. Cancel during Switching: 거부 (InProgress) — 협조적 cancel은 후속 BL
21. SwitchStateMachine: parking_lot::Mutex + &self, Switcher가 Arc로 보유

---

# T3 Runtime Wire UPDATE (r3)

**Timestamp**: 2026-04-16
**Scope**: FR-11 — PR1 switch-core를 main.rs에 실제 연결. B3 축소 범위.

## T3 신규 컴포넌트 (2개)

### ConfigCloudDirectory (Service — CloudDirectory impl)

**위치**: `src/context/config_cloud_directory.rs` (신규)
**Responsibility**: `Config`를 래핑하여 `CloudDirectory` trait 구현. Startup 시점 cloud 목록을 반영.
**Dependencies**: `Arc<Config>` (현재 `Config`는 `Arc` 미래핑 — 아래 변경사항 참조)

```rust
use std::sync::Arc;
use crate::config::Config;
use crate::context::resolver::CloudDirectory;

pub struct ConfigCloudDirectory {
    config: Arc<Config>,
}

impl ConfigCloudDirectory {
    pub fn new(config: Arc<Config>) -> Self {
        Self { config }
    }
}

impl CloudDirectory for ConfigCloudDirectory {
    fn active_cloud(&self) -> String {
        self.config.active_cloud_name().to_string()
    }

    fn known_clouds(&self) -> Vec<String> {
        self.config.cloud_names().iter().map(|s| s.to_string()).collect()
    }
}
```

**설계 판단**:
- `Config`를 `Arc`로 감싸는 이유: main.rs에서 Config를 App + ConfigCloudDirectory 양쪽에 공유해야 함
- `Config`의 `switch_cloud()` mutation은 main.rs 초기화에서만 호출되므로 `Arc` 공유에 문제 없음 (wire 시점엔 이미 확정)
- 단, `Config::switch_cloud`는 `&mut self`이므로 **wire 전에** 호출 완료되어야 함

### StaticProjectDirectory (Service — ProjectDirectoryPort impl)

**위치**: `src/context/static_project_directory.rs` (신규)
**Responsibility**: `Config` 기반 정적 프로젝트 목록 반환. 동적 HTTP 조회 없이 `clouds.yaml`에 선언된 `project_name`만 반환.
**Dependencies**: `Arc<Config>`

```rust
use std::sync::Arc;
use async_trait::async_trait;
use crate::config::Config;
use crate::context::{SwitchError, resolver::{ProjectCandidate, ProjectDirectoryPort}};

pub struct StaticProjectDirectory {
    config: Arc<Config>,
}

impl StaticProjectDirectory {
    pub fn new(config: Arc<Config>) -> Self {
        Self { config }
    }
}

#[async_trait]
impl ProjectDirectoryPort for StaticProjectDirectory {
    async fn list_projects(&self, cloud: &str) -> Result<Vec<ProjectCandidate>, SwitchError> {
        let Some(cloud_config) = self.config.cloud_config(cloud) else {
            return Ok(Vec::new());
        };

        let Some(ref project_name) = cloud_config.auth.project_name else {
            return Ok(Vec::new());
        };

        let domain = cloud_config.auth.project_domain_name
            .clone()
            .unwrap_or_else(|| "Default".to_string());

        Ok(vec![ProjectCandidate {
            cloud: cloud.to_string(),
            project_id: project_name.clone(), // placeholder — 실제 id lookup은 BL-P2-052
            project_name: project_name.clone(),
            domain,
        }])
    }
}
```

**설계 판단**:
- `project_id`에 `project_name`을 placeholder로 사용 — FR-11 Assumption #4에 명시
- `project_name` 없는 cloud → 빈 목록 반환 (에러 아님)
- BL-P2-052에서 `StaticProjectDirectory`를 `HttpProjectDirectory`로 교체 예정 (trait 이면의 구현 교체)

## T3 변경 컴포넌트 (2개)

### AdapterRegistry — HttpEndpointCache 노출

**위치**: `src/adapter/registry.rs` 변경
**변경**:
- 각 HttpAdapter의 `BaseHttpClient`를 `Arc<dyn HttpEndpointCache>`로 수집하는 메서드 추가
- HttpAdapter에 `base` 필드 접근자 필요

```rust
// registry.rs에 추가
use crate::port::http_endpoint_cache::HttpEndpointCache;

impl AdapterRegistry {
    /// Collect endpoint caches from all HTTP adapters for the
    /// EndpointCatalogInvalidator. Only meaningful for HTTP-backed
    /// registries — mock registries return an empty vec.
    pub fn endpoint_caches(&self) -> Vec<Arc<dyn HttpEndpointCache>> {
        // 각 HttpAdapter에 as_endpoint_cache() 메서드 추가 필요
        // 또는 별도 필드로 생성 시점에 캡처
        self.http_caches.clone()
    }
}
```

**구현 전략 — 2안 비교**:

| 방안 | 설명 | 장단점 |
|------|------|--------|
| **A) 생성 시 캡처** | `new_http`에서 각 BaseHttpClient를 Arc로 감싸 별도 Vec에 보관 | 타입 안전, 추가 trait 불필요. 단 구조체 크기 증가 |
| **B) downcast** | 각 Port trait에 `as_endpoint_cache()` 메서드 추가 | 기존 trait 오염, mock에서도 구현 필요 |

**선택: A안** — `new_http` 시점에 `BaseHttpClient`를 `Arc`로 먼저 생성 → adapter에 `Arc::clone` 전달 + `http_caches: Vec<Arc<dyn HttpEndpointCache>>`에 보관.

**구체 구현**:

```rust
pub struct AdapterRegistry {
    pub nova: Arc<dyn NovaPort>,
    pub neutron: Arc<dyn NeutronPort>,
    pub cinder: Arc<dyn CinderPort>,
    pub glance: Arc<dyn GlancePort>,
    pub keystone: Arc<dyn KeystonePort>,
    http_caches: Vec<Arc<dyn HttpEndpointCache>>,
}

impl AdapterRegistry {
    pub fn new_http(auth: Arc<dyn AuthProvider>, region: Option<String>) -> Result<Self, ApiError> {
        let nova_base = Arc::new(BaseHttpClient::new(auth.clone(), "compute", EndpointInterface::Public, region.clone())?);
        let neutron_base = Arc::new(BaseHttpClient::new(auth.clone(), "network", EndpointInterface::Public, region.clone())?);
        let cinder_base = Arc::new(BaseHttpClient::new(auth.clone(), "block-storage", EndpointInterface::Public, region.clone())?);
        let glance_base = Arc::new(BaseHttpClient::new(auth.clone(), "image", EndpointInterface::Public, region.clone())?);
        let keystone_base = Arc::new(BaseHttpClient::new(auth, "identity", EndpointInterface::Public, region)?);

        let http_caches: Vec<Arc<dyn HttpEndpointCache>> = vec![
            nova_base.clone(),
            neutron_base.clone(),
            cinder_base.clone(),
            glance_base.clone(),
            keystone_base.clone(),
        ];

        Ok(Self {
            nova: Arc::new(NovaHttpAdapter::from_base(nova_base)),
            neutron: Arc::new(NeutronHttpAdapter::from_base(neutron_base)),
            cinder: Arc::new(CinderHttpAdapter::from_base(cinder_base)),
            glance: Arc::new(GlanceHttpAdapter::from_base(glance_base)),
            keystone: Arc::new(KeystoneHttpAdapter::from_base(keystone_base)),
            http_caches,
        })
    }

    pub fn endpoint_caches(&self) -> Vec<Arc<dyn HttpEndpointCache>> {
        self.http_caches.clone()
    }

    #[cfg(test)]
    pub fn new_mock() -> Self {
        // ... 기존 + http_caches: Vec::new()
    }
}
```

**연쇄 변경 — 각 HttpAdapter**:
- `NovaHttpAdapter`, `NeutronHttpAdapter`, `CinderHttpAdapter`, `GlanceHttpAdapter`, `KeystoneHttpAdapter` 모두:
  - `base: BaseHttpClient` → `base: Arc<BaseHttpClient>`
  - 신규: `pub fn from_base(base: Arc<BaseHttpClient>) -> Self`
  - 기존 `new()` 유지 (하위 호환) — 내부에서 `Arc::new(BaseHttpClient::new(...))` 호출
  - `self.base.method()` 호출은 `Arc<BaseHttpClient>`도 `Deref`로 동일 동작

### main.rs — T3 Wire 시퀀스

**위치**: `src/main.rs` 변경 (else 분기 — 실제 모드)
**변경**: `app.wire_context_switch(switcher, event_tx)` 호출 추가

#### 삽입 위치 제약 (R1 리뷰 I1 반영)

현재 main.rs 흐름에서 `config`는 `App::from_registry`에 move, `registry`는 `Arc`로 감싸져 worker에 clone 전달. wire 시퀀스는 3개 시점에 분산 삽입해야 한다:

```
Phase A: App::from_registry 호출 전 (config move 전)
  ① config_for_wire = Arc::new(config.clone())
  ② credential 복제 (auth_url, username — TokenCacheStore용)

Phase B: AdapterRegistry 생성 후, worker spawn 전
  ③ endpoint_caches = registry.endpoint_caches()

Phase C: worker spawn 후 (app 생성 완료 후)
  ④~⑫ 나머지 wire + app.wire_context_switch
```

#### 구체 코드 (3-phase)

```rust
// === Phase A: config move 전 ===

// ① Config clone → Arc (Config는 #[derive(Clone)] 확인됨)
let config_for_wire = Arc::new(config.clone());

// ② credential 복제 — TokenCacheStore 경로 계산용 (R1 리뷰 I2 반영)
//    auth_provider.cloud_key()는 존재하지 않음.
//    token_cache::compute_cloud_key(auth_url, username)을 직접 호출.
let wire_auth_url = credential.auth_url.clone();
let wire_username = match &credential.method {
    AuthMethod::Password { username, .. } => username.clone(),
    AuthMethod::ApplicationCredential { id, .. } => id.clone(),
};

// ... App::from_registry(config, ...) — config move ...

// === Phase B: registry 생성 후, worker spawn 전 ===

// ③ endpoint caches 수집 (registry는 Arc이므로 clone 후 worker에 전달)
let endpoint_caches = registry.endpoint_caches();

// ... worker spawn(registry.clone(), ...) ...

// === Phase C: worker spawn 후 ===

// ④ CloudDirectory + ProjectDirectory (Config 기반 정적)
let cloud_dir: Arc<dyn CloudDirectory> =
    Arc::new(ConfigCloudDirectory::new(config_for_wire.clone()));
let project_dir: Arc<dyn ProjectDirectoryPort> =
    Arc::new(StaticProjectDirectory::new(config_for_wire.clone()));

// ⑤ ContextTargetResolver
let resolver = Arc::new(ContextTargetResolver::new(cloud_dir, project_dir));

// ⑥ EndpointCatalogInvalidator
let invalidator = Arc::new(EndpointCatalogInvalidator::new(endpoint_caches));

// ⑦ KeystoneRescopeAdapter (R1 리뷰 W1 반영 — timeout 설정)
let rescope_client = reqwest::Client::builder()
    .timeout(std::time::Duration::from_secs(30))
    .connect_timeout(std::time::Duration::from_secs(10))
    .build()
    .expect("rescope client build");
let rescoper: Arc<dyn KeystoneRescopePort> = Arc::new(KeystoneRescopeAdapter::new(
    rescope_client,
    config_for_wire.active_cloud_config().auth.auth_url.clone(),
));

// ⑧ TokenCacheStore (R1 리뷰 I2 반영 — compute_cloud_key 직접 호출)
let cloud_key = token_cache::compute_cloud_key(&wire_auth_url, &wire_username);
let token_cache = TokenCacheStore::new(token_cache::cache_dir_path(&cloud_key));

// ⑨ ScopedAuthSession (ContextSessionPort impl)
let session: Arc<dyn ContextSessionPort> = Arc::new(ScopedAuthSession::new(
    auth_provider.clone() as Arc<dyn ScopedAuthPort>,
    rescoper,
    invalidator,
    token_cache,
));

// ⑩ SwitchStateMachine
let state = Arc::new(SwitchStateMachine::new(current_epoch.clone()));

// ⑪ CancellationRegistry + ContextHistoryStore
let cancellation = Arc::new(CancellationRegistry::new());
let history = Arc::new(std::sync::Mutex::new(ContextHistoryStore::new()));

// ⑫ ContextSwitcher 조립 + App wire
let switcher = Arc::new(ContextSwitcher::new(
    state, cancellation, resolver, session, history,
));
app.wire_context_switch(switcher, event_tx.clone());
```

**demo 모드 가드**: demo 분기에서는 wire_context_switch 호출 없음 → `app.switcher == None` → 기존 동작 유지. NFR-7 충족.

## Config 소유권 전략 (확정 — 옵션 2)

Config는 `#[derive(Clone)]` 확인됨 (`src/config.rs:18`).
- `config.clone()` → `Arc::new()` 후 App에 owned Config 전달, wire에 `Arc<Config>` 사용
- App 시그니처 변경 없음

## 의존 그래프 (T3 추가분)

```
main.rs (wire 시퀀스)
  |
  |-- Arc<Config> ──────────────┐
  |                              |
  |-- ConfigCloudDirectory ──── Config
  |-- StaticProjectDirectory ── Config
  |         |
  |         v
  |    ContextTargetResolver
  |         |
  |-- ScopedAuthSession ───── KeystoneRescopeAdapter
  |         |                  EndpointCatalogInvalidator ── registry.endpoint_caches()
  |         |                  TokenCacheStore
  |         |                  ScopedAuthPort (KeystoneAuthAdapter)
  |         v
  |    ContextSwitcher ──── SwitchStateMachine
  |         |               CancellationRegistry
  |         |               ContextHistoryStore
  |         v
  └── app.wire_context_switch(switcher, event_tx)
```

## NFR 매핑 (T3 추가분)

| NFR | T3 보장 방법 |
|-----|-------------|
| NFR-6 컴파일 안전성 | 모든 wire가 concrete type → trait object 변환으로 컴파일 타임 검증. dyn 다운캐스트 없음 |
| NFR-7 Demo 모드 무회귀 | demo 분기에서 wire_context_switch 호출 없음. switcher=None → spawn_switch가 에러 toast 표시 |

## T3 DETAIL 체크리스트 (13개)

1. ConfigCloudDirectory: Config 래퍼, active_cloud/known_clouds 위임
2. StaticProjectDirectory: Config.cloud_config(cloud).auth.project_name → ProjectCandidate 1건 반환
3. StaticProjectDirectory: project_id = project_name (placeholder, BL-P2-052에서 교체). 테스트에 PLACEHOLDER 주석 추가
4. StaticProjectDirectory: project_name 없는 cloud → 빈 목록 (에러 아님)
5. AdapterRegistry: new_http에서 BaseHttpClient를 Arc로 생성 → http_caches Vec 보유
6. 각 HttpAdapter: base 필드 Arc<BaseHttpClient>로 변경 + from_base 생성자 추가
7. AdapterRegistry: endpoint_caches() 메서드 추가. new_mock()에도 http_caches: Vec::new() 추가
8. main.rs: 3-phase 분산 삽입 (A: config clone 전, B: registry 후 worker 전, C: worker 후 wire)
9. main.rs: demo 분기는 wire_context_switch 미호출 (NFR-7)
10. Config 소유권: 옵션 2 확정. config.clone() → Arc, App에 owned Config
11. TokenCacheStore: `token_cache::compute_cloud_key(auth_url, username)` 직접 호출 (cloud_key() 메서드 없음)
12. KeystoneRescopeAdapter: reqwest::Client에 timeout(30s) + connect_timeout(10s) 설정
13. credential 복제 (auth_url, username): Phase A에서 config move 전에 수행

## R1 리뷰 반영 이력

| 이슈 | 출처 | 반영 |
|------|------|------|
| I1 wire 삽입 위치 | quality | 3-phase 분산 삽입으로 재설계 (체크리스트 #8) |
| I2 cloud_key() 미존재 | spec | compute_cloud_key 직접 호출로 교정 (체크리스트 #11) |
| W1 timeout 미설정 | quality | rescope client에 30s/10s 설정 추가 (체크리스트 #12) |
| W2 wire helper 추출 | maintainability | CONSTRUCTION에서 판단 (현시점 미반영) |
| W3 placeholder 주석 | maintainability | 테스트에 PLACEHOLDER 주석 추가 (체크리스트 #3) |
| W4 r2 SwitchStateMachine drift | quality | 본 세션 범위 밖 (기록만) |
