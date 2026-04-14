Reading additional input from stdin...
OpenAI Codex v0.118.0 (research preview)
--------
workdir: /Users/jay.ahn/projects/infra/nexttui/.worktrees/runtime-context-switch
model: gpt-5.3-codex
provider: openai
approval: never
sandbox: read-only
reasoning effort: high
reasoning summaries: none
session id: 019d85be-9b26-7360-ba05-ca099726fe9b
--------
user
You are reviewing the DETAIL section of application-design.md (LIST already approved with revisions).

PROJECT: nexttui — Rust + ratatui TUI for OpenStack admins.
FEATURE: BL-P2-031 — Runtime cloud/project context switching via Keystone rescoping.

REVIEW THIS FILE: devflow-docs/inception/application-design.md (focus on the "# DETAIL Mode" section onward, line ~103+)
ALSO CONSIDER: devflow-docs/inception/requirements.md (10 FR + 5 NFR for BL-P2-031), devflow-docs/inception/design-review-raw/synthesis.md (prior LIST council synthesis)

Codebase reference points:
- src/port/auth.rs (AuthProvider trait)
- src/adapter/auth/keystone.rs (KeystoneAuthAdapter, active_scope)
- src/adapter/auth/token_cache.rs (TokenScope, TokenCacheStore)
- src/adapter/http/base.rs (endpoint cache)
- src/component.rs (Component trait)
- src/action.rs (Action enum), src/event.rs (AppEvent enum)
- src/worker.rs (run_worker, polling spawn sites)

REVIEW SCOPE — assess the DETAIL design for:
1. **Interface correctness** — Are method signatures sensible? Async/sync split correct? Error types aligned with codebase patterns (ApiError, thiserror)?
2. **Dependency direction** — Cycles? Layering violations (Adapter depending on Service)? Port at right boundary?
3. **Atomicity gaps** — Does the begin/commit/rollback flow truly guarantee no torn state? Particularly around: rescope OK + invalidate fail; epoch bump vs cancel ordering; commit ordering (token cache write before or after state.commit?)
4. **Concurrency hazards** — Worker spawn signature change: does it cover all spawn sites? VersionedEvent unwrap in the dispatcher: where exactly is the epoch check enforced (Worker side, App side, or both)?
5. **State machine completeness** — Missing transitions (e.g. cancel during Switching, second switch request mid-flight)?
6. **UI correctness** — ContextChanged variant: does every Component need to handle it? What about Components that hold no per-context data — are they no-op safe? ContextIndicator highlight_for: who drives the timer?
7. **Test seams** — MockContextSession: does it cover all failure modes the production code can hit? Is there a way to deterministically reproduce "rescope succeeds + catalog refresh fails + rollback runs"?
8. **Codebase fit** — Are types like ContextTarget overlapping with existing TokenScope? Should ContextTarget reuse TokenScope or be a new type?
9. **Missing details** — Anything underspecified that will block code-generation TDD (RED phase needs to know expected behavior)?

DELIVER (concise, no hedging):
- Top 3 critical issues (each: what + why + concrete fix)
- Specific signature corrections (with exact Rust code if needed)
- Missing components or interfaces
- Verdict: APPROVE | APPROVE-WITH-CHANGES | REJECT
- Brief reasoning

Be harsh but constructive. Rust idiom and OpenStack production realism.
codex
I’m reviewing `application-design.md` from `# DETAIL Mode` onward against the requirements/synthesis and the referenced Rust modules. I’ll first extract the relevant sections and current code signatures, then I’ll map design claims to concrete interface/concurrency behavior and return hard findings with fixes.
exec
/bin/zsh -lc "nl -ba devflow-docs/inception/application-design.md | sed -n '1,260p'" in /Users/jay.ahn/projects/infra/nexttui/.worktrees/runtime-context-switch
 succeeded in 0ms:
     1	# Application Design
     2	
     3	**Mode**: LIST (목록 단계, Council 리뷰 반영 개정)
     4	**Timestamp**: 2026-04-13T00:00:00+09:00
     5	**BL**: BL-P2-031 Keystone Rescoping
     6	**Revision**: r2 (3-AI Council 종합 반영 — design-review-raw/synthesis.md 참조)
     7	
     8	## 의존 방향 원칙
     9	
    10	- **App** (Controller, orchestrator) **owns** **ContextSwitcher**.
    11	- **ContextSwitcher**는 commit 결과로 `(new_epoch, ContextSnapshot)` 반환. App이 epoch를 반영하고 `AppEvent::ContextChanged`를 디스패치.
    12	- **ContextSwitcher**는 `ContextSessionPort`를 통해 atomic begin/commit/rollback 수행.
    13	- **Worker spawn API**는 `(epoch, CancellationToken)` 페어를 반드시 받는다 (시그니처 강제).
    14	
    15	## 컴포넌트 목록
    16	
    17	### 신규 컴포넌트 (12개)
    18	
    19	| 컴포넌트 | 책임 | 타입 | PR |
    20	|---------|------|------|-----|
    21	| `ContextEpoch` | 단조 증가 epoch 카운터, stale 이벤트 식별 키 | Util | PR1 |
    22	| `CancellationRegistry` | 활성 폴링/장기 fetch에 대한 `CancellationToken` 등록·일괄 취소 | Service | PR1 |
    23	| `VersionedEvent<T>` envelope | `{ event: T, epoch: u64 }` — Action/AppEvent를 감싸 epoch stamp (variant 폭증 회피) | Util | PR1 |
    24	| `AppEvent::ContextChanged { target }` | UI에 컨텍스트 변경 통지 → 모듈 컴포넌트가 내부 데이터 비움 | Event variant | PR1 |
    25	| `SwitchStateMachine` | `Idle → Switching → Committed | Failed` 전환 상태, rollback 규약 | Service | PR1 |
    26	| `ContextSwitcher` | 전환 절차 오케스트레이터 (epoch++ → cancel → session begin → rescope → catalog 무효화 → commit) | Service | PR1 |
    27	| `ContextSessionPort` (trait) | atomic begin/commit/rollback 인터페이스. `invalidate_all_endpoints()` hook | Port | PR1 |
    28	| `ScopedAuthSession` | `ContextSessionPort` 구현체. 활성 scoped token + endpoint cache 일괄 관리 | Service | PR1 |
    29	| `KeystoneRescopeAdapter` | Keystone v3 token-method scoped exchange 호출, expires_at 정본 사용 | Adapter | PR1 |
    30	| `EndpointCatalogInvalidator` | 모든 HTTP client의 endpoint cache 일괄 무효화 (`src/adapter/http/base.rs` 자동화) | Service | PR1 |
    31	| `ContextTargetResolver` | name/uuid/cloud-prefix → ContextTarget 변환, 충돌 disambiguation. 명령·피커·모듈 액션 공유 | Service | PR1 |
    32	| `ContextHistoryStore` | switch-back 1단계 히스토리 (ContextSnapshot 저장), rollback 시 동일 사용 | Util | PR1 |
    33	| `ContextIndicator` | 영구 컨텍스트 표시 위젯 (`cloud / project [/ domain]`), 전환 직후 강조 | UI Widget | PR3 |
    34	| `ContextPicker` | Ctrl+P 모달, 프로젝트/클라우드 fuzzy 검색·선택 (`cloud • project • domain • project_id` 행) | UI Widget (modal) | PR4 |
    35	
    36	> 신규 컴포넌트 14개 (PR1 인프라 12 + UI 2). `AppEvent::ContextChanged`는 enum variant 추가지만 표에 분리 표기.
    37	
    38	### 변경 컴포넌트 (8개)
    39	
    40	| 컴포넌트 | 변경 책임 | 타입 | PR |
    41	|---------|----------|------|-----|
    42	| `App` (src/app.rs) | 활성 cloud/project 컨텍스트 보유, ContextSwitcher 소유, epoch/스냅샷 반영, ContextChanged 디스패치 | Controller (재분류) | PR1 |
    43	| `Worker` (src/worker.rs) | spawn API에 `(epoch, CancellationToken)` 페어 강제, `tokio::select!` cancel branch + epoch 검증 | Service | PR1 |
    44	| `Action` (src/action.rs) | `VersionedEvent<Action>` envelope 적용 (또는 `Action::with_epoch`) | Type | PR1 |
    45	| `AppEvent` (src/event.rs) | `VersionedEvent<AppEvent>` envelope 적용 + `ContextChanged` variant 추가 | Type | PR1 |
    46	| `CommandParser` (src/input/command.rs) | `:switch-project <name|uuid|cloud/project>`, `:switch-cloud`, `:switch-back` 등록 + tab 자동완성. `ContextTargetResolver` 사용 | Controller | PR3 |
    47	| `ConfirmDialog` (src/ui/confirm.rs) | destructive confirm에 `cloud • project` fingerprint 표시. 컨텍스트 변경 직후엔 추가 확인 강제 | UI Widget | PR3 |
    48	| `StatusBar` (src/ui/status_bar.rs) | ContextIndicator 임베드 위치/우선순위 조정 | UI Widget | PR3 |
    49	| `TokenCacheStore` (src/adapter/auth/token_cache.rs) | rescoped 토큰을 scope별로 저장/조회 (BL-P2-029 기반), 만료 시 재취득 | Repository | PR1 |
    50	| `Project Module` (src/module/project/mod.rs) | 모듈-로컬 `s` 액션 핸들러. KeyMap 글로벌 등록 회피 (Enter는 Detail 유지) | Controller | PR5 |
    51	
    52	> 변경 컴포넌트 9개. `Action`/`AppEvent`는 type 정의 변경이라 별도 표기.
    53	
    54	### Mock / 테스트 시즘 (1개)
    55	
    56	| 컴포넌트 | 책임 | 타입 | PR |
    57	|---------|------|------|-----|
    58	| `MockContextSession` (확장) | `src/port/mock.rs`에 추가. rescope 성공/실패, catalog invalidate 부분 실패 시뮬레이션 (fault-injection seam) | Mock | PR1 |
    59	
    60	**총 24개 컴포넌트** (신규 14 + 변경 9 + 테스트 1)
    61	
    62	## 개정된 PR 매핑
    63	
    64	| PR | 컴포넌트 | Depends on | 사용자 노출 |
    65	|----|---------|-----------|-----------|
    66	| **PR1 (safety infra + switch core)** | ContextEpoch, CancellationRegistry, VersionedEvent, AppEvent::ContextChanged, SwitchStateMachine, ContextSwitcher, ContextSessionPort, ScopedAuthSession, KeystoneRescopeAdapter, EndpointCatalogInvalidator, ContextTargetResolver, ContextHistoryStore, App 통합, Worker 시그니처 개정, Action/AppEvent envelope, TokenCacheStore 확장, MockContextSession | — | 없음 (인프라만) |
    67	| **PR3 (안전 가시성 + 명령)** | ContextIndicator, StatusBar 임베드, ConfirmDialog fingerprint, CommandParser 확장 | PR1 | `:switch-*` 명령, 컨텍스트 인디케이터, fingerprint confirm |
    68	| **PR4 (피커 UI)** | ContextPicker, KeyMap 글로벌 단축키 (Ctrl+P) | PR3 | Ctrl+P 모달 |
    69	| **PR5 (Identity 통합)** | Project Module 모듈-로컬 `s` 핸들러 | PR3 | Identity 리스트 `s` 단축키 |
    70	
    71	→ **PR 수: 6 → 4로 축소**. PR1 통합으로 stale 누설 창 제거. PR3에 안전 가시성 + 명령을 묶어 사용자가 전환을 쓸 수 있는 첫 시점부터 인디케이터/fingerprint가 함께 동작.
    72	
    73	## NFR 매핑
    74	
    75	| NFR | 보장 컴포넌트 |
    76	|-----|--------------|
    77	| NFR-1 안전성 (atomic switch, stale 차단) | ContextEpoch + CancellationRegistry + SwitchStateMachine + ContextSessionPort/ScopedAuthSession + EndpointCatalogInvalidator |
    78	| NFR-2 성능 (1초 이내) | KeystoneRescopeAdapter (네트워크 왕복 측정), EndpointCatalogInvalidator (lazy refresh) |
    79	| NFR-3 테스트 커버리지 | MockContextSession (port mock), ContextSwitcher 통합 테스트, 부분실패 (rescope OK + invalidate fail) 시뮬레이션 |
    80	| NFR-4 UX 일관성 | ContextPicker (기존 SelectPopup 재사용), CommandParser 확장 (기존 패턴), ContextTargetResolver (단일 disambiguation 로직) |
    81	| NFR-5 관측성 | ContextSwitcher의 `tracing` 이벤트 (epoch, target cloud/project, 결과, 소요 시간) |
    82	
    83	## 개정 사유 요약 (Council 리뷰 반영)
    84	
    85	| 변경 | 출처 | 이유 |
    86	|------|------|------|
    87	| ContextSessionPort + ScopedAuthSession 추가 | Codex | atomic begin/commit/rollback 부재 시 rescope 성공 + stale endpoint 호출 사고 가능 |
    88	| EndpointCatalogInvalidator 추가 | Codex | `src/adapter/http/base.rs:66`의 매뉴얼 invalidate를 자동화해야 일관성 보장 |
    89	| ContextHistoryStore 추가 | Codex+Claude | switch-back/rollback 공통 저장소 누락 |
    90	| ContextTargetResolver 추가 | Codex | 명령·피커·모듈 액션이 같은 disambiguation 로직 필요 (3중 구현 회피) |
    91	| AppEvent::ContextChanged 추가 | Gemini | epoch만으로는 잔존 데이터 표시 방지 불가 |
    92	| VersionedEvent envelope | Gemini | epoch를 매 variant에 추가하지 않고 plumbing |
    93	| ContextIndicator/Picker → UI Widget | Codex | Component trait 위젯이 정확. Controller는 라우팅 의미 |
    94	| App → Controller | Codex | 코드베이스 실제 역할이 orchestrator/router |
    95	| CommandRegistry → CommandParser 확장 | Codex | src/input/command.rs 실제 명명 일치 |
    96	| PR1+PR2 통합 | Claude+Codex | PR1만 머지 시 사용자 노출 0이지만 stale 누설 창 발생. 통합으로 창 제거 |
    97	| PR5 ContextIndicator/fingerprint를 PR3로 앞당김 | Codex | PR3/4에서 명령/피커가 안전 가시성 없이 노출되는 위험 차단 |
    98	| PR6 KeyMap 분리 → PR5 모듈-로컬 | Claude+Codex | KeyMap 글로벌 동시 수정 충돌 회피, `s` 의미가 모듈 의존적 |
    99	| MockContextSession 명시 | Claude | port mock 확장 누락 보완 |
   100	
   101	---
   102	
   103	# DETAIL Mode (Standard depth)
   104	
   105	**Timestamp**: 2026-04-13T00:00:00+09:00
   106	모든 컴포넌트의 주요 인터페이스(2~3개)와 의존 컴포넌트를 명시.
   107	
   108	## 핵심 타입
   109	
   110	```rust
   111	// src/context.rs (신규 모듈)
   112	pub type Epoch = u64;
   113	
   114	#[derive(Debug, Clone, PartialEq, Eq, Hash)]
   115	pub struct ContextTarget {
   116	    pub cloud: String,
   117	    pub project: ProjectRef,        // ByName(name) | ById(uuid)
   118	    pub domain: Option<String>,
   119	}
   120	
   121	#[derive(Debug, Clone)]
   122	pub struct ContextSnapshot {
   123	    pub target: ContextTarget,
   124	    pub epoch: Epoch,
   125	    pub token_scope: TokenScope,    // src/port/types.rs 재사용
   126	    pub captured_at: chrono::DateTime<chrono::Utc>,
   127	}
   128	
   129	#[derive(Debug, Clone, thiserror::Error)]
   130	pub enum SwitchError {
   131	    #[error("rescope rejected by Keystone: {0}")]
   132	    RescopeRejected(String),
   133	    #[error("catalog invalidation failed: {0}")]
   134	    CatalogFailed(String),
   135	    #[error("ambiguous target: {candidates:?}")]
   136	    Ambiguous { candidates: Vec<ContextTarget> },
   137	    #[error("target not found: {0}")]
   138	    NotFound(String),
   139	    #[error("switch in progress")]
   140	    InProgress,
   141	}
   142	```
   143	
   144	## 신규 컴포넌트 상세
   145	
   146	### ContextEpoch (Util)
   147	**Responsibility**: 단조 증가 epoch 카운터.
   148	**Interface**:
   149	- `pub fn new() -> Self` — 0부터 시작
   150	- `pub fn current(&self) -> Epoch`
   151	- `pub fn bump(&self) -> Epoch` — Atomic increment, 새 값 반환
   152	**Dependencies**: 없음 (`AtomicU64` 내부 사용)
   153	
   154	### CancellationRegistry (Service)
   155	**Responsibility**: 활성 작업의 `CancellationToken` 등록·일괄 취소.
   156	**Interface**:
   157	- `pub fn register(&self, epoch: Epoch) -> CancellationToken` — 새 토큰 발급·등록
   158	- `pub fn cancel_below(&self, threshold_epoch: Epoch) -> usize` — 이전 epoch 토큰 일괄 cancel, 취소 건수 반환
   159	- `pub fn active_count(&self) -> usize` — 관측용
   160	**Dependencies**: `tokio_util::sync::CancellationToken`
   161	
   162	### VersionedEvent\<T\> (Util)
   163	**Responsibility**: epoch + payload envelope.
   164	**Interface**:
   165	- `pub fn new(payload: T, epoch: Epoch) -> Self`
   166	- `pub fn epoch(&self) -> Epoch`
   167	- `pub fn into_inner(self) -> T`
   168	**Dependencies**: 없음. Action / AppEvent 양쪽에 사용.
   169	
   170	### AppEvent::ContextChanged variant
   171	**Responsibility**: 컨텍스트 전환 후 모듈 컴포넌트에 데이터 비우기 신호.
   172	**Interface (variant)**:
   173	- `AppEvent::ContextChanged { target: ContextTarget, epoch: Epoch }`
   174	**Dependencies**: `ContextTarget` (src/context.rs)
   175	**호환성**: 기존 `Component::handle_event(&mut self, event: &AppEvent)`에서 `match`로 처리
   176	
   177	### SwitchStateMachine (Service)
   178	**Responsibility**: `Idle → Switching → Committed | Failed` 상태 전이, rollback 규약.
   179	**Interface**:
   180	- `pub fn try_begin(&mut self, target: ContextTarget) -> Result<Epoch, SwitchError::InProgress>`
   181	- `pub fn commit(&mut self, snapshot: ContextSnapshot)` — Committed 상태로 이동
   182	- `pub fn fail(&mut self, err: SwitchError) -> ContextSnapshot` — Failed 후 직전 snapshot 반환 (rollback용)
   183	- `pub fn state(&self) -> SwitchState` — Idle/Switching/Committed/Failed
   184	**Dependencies**: `ContextHistoryStore` (이전 snapshot 조회)
   185	
   186	### ContextSwitcher (Service)
   187	**Responsibility**: 전환 절차 오케스트레이터.
   188	**Interface**:
   189	- `pub async fn switch(&self, target: ContextTarget) -> Result<ContextSnapshot, SwitchError>`
   190	- `pub async fn switch_back(&self) -> Result<ContextSnapshot, SwitchError>`
   191	- 절차: state.begin → registry.cancel_below(new_epoch) → session.begin → rescope_adapter.rescope → catalog_invalidator.invalidate_all → token_cache.store → state.commit → return snapshot
   192	**Dependencies**: `SwitchStateMachine`, `CancellationRegistry`, `ContextSessionPort`, `KeystoneRescopeAdapter`, `EndpointCatalogInvalidator`, `TokenCacheStore`, `ContextHistoryStore`, `ContextEpoch`
   193	
   194	### ContextSessionPort (Port trait)
   195	**Responsibility**: 활성 scoped token + endpoint cache의 atomic begin/commit/rollback 인터페이스.
   196	**Interface**:
   197	```rust
   198	#[async_trait]
   199	pub trait ContextSessionPort: Send + Sync {
   200	    async fn begin(&self, target: &ContextTarget) -> Result<SessionHandle, SwitchError>;
   201	    async fn commit(&self, handle: SessionHandle) -> Result<ContextSnapshot, SwitchError>;
   202	    async fn rollback(&self, handle: SessionHandle);
   203	    fn invalidate_all_endpoints(&self);
   204	}
   205	```
   206	**Dependencies**: `ContextTarget`, `ContextSnapshot`, `SwitchError`
   207	
   208	### ScopedAuthSession (Service — ContextSessionPort impl)
   209	**Responsibility**: `ContextSessionPort` 구현. 토큰 + endpoint cache 일괄 관리.
   210	**Interface**: trait 메서드 그대로
   211	**Dependencies**: `KeystoneRescopeAdapter`, `EndpointCatalogInvalidator`, `TokenCacheStore`, `AuthProvider` (src/port/auth.rs)
   212	
   213	### KeystoneRescopeAdapter (Adapter)
   214	**Responsibility**: Keystone v3 token-method scoped exchange.
   215	**Interface**:
   216	- `pub async fn rescope(&self, current_token: &Token, target: &ContextTarget) -> Result<Token, SwitchError>`
   217	- `pub fn capabilities(&self) -> KeystoneCapabilities` — `allow_rescope_scoped_token`, app-credential 여부 등
   218	**Dependencies**: `reqwest::Client` (기존 http base 재사용), `Token` (src/port/types.rs)
   219	
   220	### EndpointCatalogInvalidator (Service)
   221	**Responsibility**: 모든 HTTP client의 service catalog/endpoint cache 무효화.
   222	**Interface**:
   223	- `pub fn invalidate_all(&self)` — `src/adapter/http/base.rs`의 endpoint cache 일괄 비우기
   224	- `pub async fn refresh_catalog(&self, token: &Token) -> Result<ServiceCatalog, SwitchError>`
   225	**Dependencies**: `AdapterRegistry` (src/adapter/registry.rs), `KeystoneRescopeAdapter` (또는 별도 catalog API)
   226	
   227	### ContextTargetResolver (Service)
   228	**Responsibility**: name/uuid/cloud-prefix → ContextTarget 변환 + 충돌 disambiguation.
   229	**Interface**:
   230	- `pub fn resolve(&self, input: &str, current_cloud: &str) -> Result<ContextTarget, SwitchError>` — 단일 매치 시 OK, 충돌 시 `Ambiguous`, 미매치 시 `NotFound`
   231	- `pub fn list_user_projects(&self) -> Vec<ContextTarget>` — 피커용 (`/v3/auth/projects` 결과 캐시)
   232	**Dependencies**: `ConfigLoader` (clouds.yaml), `KeystoneRescopeAdapter` (project list 조회)
   233	
   234	### ContextHistoryStore (Util)
   235	**Responsibility**: switch-back 1단계 + rollback용 직전 snapshot 보관.
   236	**Interface**:
   237	- `pub fn push(&mut self, snapshot: ContextSnapshot)` — 가장 최근 1개만 유지
   238	- `pub fn previous(&self) -> Option<&ContextSnapshot>`
   239	- `pub fn pop_previous(&mut self) -> Option<ContextSnapshot>` — switch-back 시 사용
   240	**Dependencies**: 없음 (in-memory)
   241	
   242	### ContextIndicator (UI Widget)
   243	**Responsibility**: `cloud / project [/ domain]` 영구 표시. 전환 직후 N초 강조.
   244	**Interface (impl Component)**:
   245	- `pub fn new() -> Self`
   246	- `pub fn set_context(&mut self, snapshot: &ContextSnapshot)`
   247	- `pub fn highlight_for(&mut self, duration: Duration)` — 전환 직후 강조 시작
   248	- `fn render(&self, frame, area)` — Component trait
   249	**Dependencies**: `Theme` (src/ui/theme.rs)
   250	
   251	### ContextPicker (UI Widget — modal)
   252	**Responsibility**: Ctrl+P 모달, fuzzy 검색·선택.
   253	**Interface (impl Component, is_modal=true)**:
   254	- `pub fn open(&mut self, candidates: Vec<ContextTarget>, current: Option<&ContextTarget>)`
   255	- `pub fn close(&mut self)`
   256	- `fn handle_key(&mut self, key) -> Option<Action>` — Enter 시 `Action::SwitchContext(target)` 발행
   257	**Dependencies**: `SelectPopup` (src/ui/select_popup.rs 재사용), `ContextTarget`, `Theme`
   258	
   259	### MockContextSession (Mock — port impl, src/port/mock.rs 확장)
   260	**Responsibility**: `ContextSessionPort`의 테스트 더블. fault-injection.

exec
/bin/zsh -lc "nl -ba devflow-docs/inception/requirements.md | sed -n '1,260p'" in /Users/jay.ahn/projects/infra/nexttui/.worktrees/runtime-context-switch
 succeeded in 0ms:
     1	# Requirements Analysis
     2	
     3	**Depth**: Standard
     4	**Timestamp**: 2026-04-13T00:00:00+09:00
     5	**BL**: BL-P2-031 (#39)
     6	
     7	## User Intent
     8	nexttui에서 런타임 중 활성 cloud / project 컨텍스트를 전환할 수 있게 한다. Keystone rescoping을 사용해 토큰 재발급 없이 프로젝트 스코프를 변경하고, 변경된 컨텍스트로 모든 모듈이 일관되게 동작하도록 한다.
     9	
    10	**확정 해석**: 트리거 UX는 **B+ (인터랙티브 피커 + 명령 + Identity 리스트 `s` 단축키)**. Codex 적대적 리뷰 결과를 반영해 입력 UX 외에 **컨텍스트 전환의 원자성·이전 컨텍스트 격리·안전 가시성**을 동반 설계로 포함한다.
    11	
    12	**구현 전략**: 단일 BL을 단계적 PR로 분할 (옵션 C). feature 브랜치에 PR1~PR6 누적 머지 → 통합 검증 후 main에 단일 머지.
    13	
    14	## Functional Requirements
    15	
    16	### FR-1. 명령 기반 전환 (Must)
    17	- `:switch-project <name|uuid>` — 현재 cloud 내에서 프로젝트 변경
    18	- `:switch-cloud <name>` — cloud 전환 (프로젝트는 cloud 기본값 또는 미선택 상태)
    19	- `:switch-project <cloud>/<project>` — cloud-qualified 형식
    20	- `:switch-back` — 이전 컨텍스트로 복귀 (1단계 히스토리)
    21	- 이름 충돌 시 후보 목록을 표시하고 재선택을 요구한다 (silent pick 금지)
    22	- Tab 자동완성 지원 (현재 cloud의 프로젝트 목록 기준)
    23	
    24	### FR-2. 인터랙티브 피커 (Must)
    25	- 글로벌 단축키 (예: `Ctrl+P`)로 어디서든 호출
    26	- Fuzzy search 지원
    27	- 각 항목은 `cloud • project • domain • project_id` 표시
    28	- 기본 선택은 현재 컨텍스트 행
    29	
    30	### FR-3. Identity 리스트 통합 (Must — C-lite)
    31	- Identity 모듈의 Project / Cloud 리스트에서 `s` 키로 해당 행을 활성 컨텍스트로 전환
    32	- `Enter`는 기존 Detail 진입 의미 유지
    33	
    34	### FR-4. 전환 상태머신 (Must)
    35	- 상태: `Idle → Switching → Committed | Failed`
    36	- `Switching` 진입 시: epoch++, 이전 컨텍스트의 폴링/in-flight 작업 cancel, destructive 액션 입력 차단
    37	- Keystone rescope 호출 → service catalog 강제 재조회 → 새 컨텍스트로 commit
    38	- 실패 시 이전 컨텍스트로 rollback하고 사용자에게 가시적 에러 표시
    39	
    40	### FR-5. ContextEpoch / 동시성 격리 (Must)
    41	- 모든 액션·이벤트에 epoch 태그
    42	- 폴링 루프와 장기 fetch는 `tokio::select!` cancel branch 또는 epoch 검증 필수
    43	- 전환 후 이전 epoch의 이벤트는 폐기 (UI mutation 금지)
    44	
    45	### FR-6. Keystone Rescoping Adapter (Must)
    46	- token-method scoped exchange 사용 (Keystone v3)
    47	- 새 토큰의 `expires_at`을 정본으로 신뢰 (TTL 추론 금지)
    48	- rescope 후 service catalog와 endpoint 캐시 무효화·재조회
    49	- rescope 거부 (예: `allow_rescope_scoped_token=false`, app-credential, 권한 부족) 시 가시적 실패 + 사용자에게 full re-auth 안내
    50	
    51	### FR-7. 컨텍스트 인디케이터 (Must)
    52	- 영구 표시 (예: 상태바 상단/하단) — `cloud / project` 최소 표시, 가능하면 domain·region 포함
    53	- 전환 직후 일정 시간 강조 표시 (애니메이션 또는 색 강조)
    54	
    55	### FR-8. Destructive 액션 안전 게이트 (Must)
    56	- delete / force-delete / evacuate 등 destructive confirm 다이얼로그에 현재 `cloud • project` fingerprint를 명시적으로 표시
    57	- 세션 내에서 직전에 컨텍스트가 변경된 경우 destructive confirm을 한 번 더 강제 (typing 또는 추가 확인)
    58	
    59	### FR-9. UPDATE 모드 호환성 (Should)
    60	- BL-P2-029의 다중 토큰 맵을 활용해 cloud별 토큰을 캐시·재사용 (재인증 최소화)
    61	- BL-P2-028 토큰 캐시 영속화와 충돌 없이 동작
    62	
    63	### FR-10. Region 전환 (Out of Scope)
    64	- 본 BL은 Keystone scope 변경 (cloud / project)만 다룬다
    65	- region 변경은 별도 명령/모달로 후속 백로그에 분리
    66	
    67	## Non-Functional Requirements
    68	
    69	### NFR-1. 안전성 (Critical)
    70	- 전환 이후 이전 컨텍스트의 stale 이벤트가 새 UI 상태를 변경해서는 안 된다 (epoch 검증으로 보장)
    71	- rescope 실패 시 컨텍스트 인디케이터와 실제 활성 컨텍스트가 불일치해서는 안 된다 (atomic commit)
    72	
    73	### NFR-2. 성능
    74	- 전환 액션 (피커 선택 → commit)은 정상 경로에서 1초 이내 완료를 목표
    75	- rescope + catalog 재조회의 네트워크 왕복을 합산해 측정
    76	
    77	### NFR-3. 테스트 커버리지
    78	- 단위 테스트: state machine, epoch 검증, 명령 파서, 충돌 disambiguation
    79	- 통합 테스트: rescope 성공/실패, catalog 재조회 실패, 전환 중 in-flight 폴링, app-credential 경로 거부
    80	- 기존 1116 tests baseline 무회귀
    81	
    82	### NFR-4. UX 일관성
    83	- 단축키와 명령은 기존 CommandRegistry / KeyMap 패턴 준수
    84	- 모달은 기존 Toast / Popup 컴포넌트 스타일 일관
    85	
    86	### NFR-5. 관측성
    87	- 전환 단계별 `tracing` 이벤트 (epoch, 대상 cloud/project, 결과)
    88	- rescope 실패 사유 로깅
    89	
    90	## Technology Stack
    91	| 계층 | 선택 | 소스 | 비고 |
    92	|------|------|------|------|
    93	| Language | Rust (edition 2024) | Brownfield 감지 | — |
    94	| TUI Framework | ratatui 0.30 + crossterm 0.29 | Brownfield 감지 | — |
    95	| HTTP Client | reqwest | Brownfield 감지 | OpenStack 호출 |
    96	| Async Runtime | tokio | Brownfield 감지 | CancellationToken 도입 필요 |
    97	| Test Framework | built-in `#[cfg(test)]` | Brownfield 감지 | — |
    98	| Lint | clippy (deny unwrap/expect) | CLAUDE.md | — |
    99	
   100	## Assumptions
   101	1. 대상 OpenStack 배포는 Keystone v3 + token-method rescoping을 허용한다. 비활성 환경은 가시적 실패 + full re-auth 폴백으로 대응한다.
   102	2. cloud 정의는 기존 `clouds.yaml` 또는 nexttui Config의 cloud 목록을 그대로 사용한다 (별도 cloud 추가 UX는 본 BL 비포함).
   103	3. App-credential 인증 사용자는 본 BL의 전환 UX에서 명시적 거부 메시지로 안내한다 (별도 BL로 분리).
   104	4. 피커의 프로젝트 목록은 user-accessible projects API (`/v3/auth/projects` 또는 동등 엔드포인트)로 조회한다 (admin 전체 조회 아님).
   105	5. `:switch-back` 히스토리 깊이는 1 (직전 컨텍스트만). 다단계 히스토리는 후속 백로그.
   106	6. Region은 본 BL 비포함. 별도 후속 BL로 신설한다.
   107	
   108	## Open Questions
   109	없음 (Codex 적대적 리뷰의 10개 미결 질문은 위 요구사항에 모두 반영되었거나 명시적 Out of Scope / Assumption으로 처리됨).
   110	
   111	## Change Log
   112	- 2026-04-13: 초안 작성. Codex 적대적 리뷰 (10개 질문 + 3개 치명 결함 + 권장 수정안) 반영. UX 안 B+ 확정, 구현 전략 옵션 C (단일 BL 단계적 머지) 확정.

exec
/bin/zsh -lc "nl -ba devflow-docs/inception/design-review-raw/synthesis.md | sed -n '1,260p'" in /Users/jay.ahn/projects/infra/nexttui/.worktrees/runtime-context-switch
 succeeded in 0ms:
     1	# Council Synthesis — application-design.md (LIST)
     2	
     3	**Chair**: Claude
     4	**Reviewers**: Codex (REJECT), Gemini (APPROVE-WITH-CHANGES), Claude (APPROVE-WITH-CHANGES)
     5	**Verdict (synthesized)**: **APPROVE-WITH-CHANGES (LIST 개정 후 DETAIL 진입)**
     6	
     7	---
     8	
     9	## 0. 메타 이슈 (선결)
    10	
    11	Codex와 Gemini가 공통 지적: **워크트리의 `requirements.md`가 옛 ViewContext 리팩토링 버전**이었음.
    12	→ **원인**: requirements.md가 git-tracked, 워크트리가 HEAD에서 옛 버전을 체크아웃. 메인에 새로 쓴 BL-P2-031 버전이 워크트리로 동기화되지 않음.
    13	→ **조치**: 메인의 BL-P2-031 requirements/workflow-plan/state/audit/session-summary를 워크트리로 복사 완료. Codex의 Top Issue 1은 환경 문제로 종결.
    14	→ **재발 방지**: 추후 inception 단계의 모든 추가 산출물은 워크트리에서 작성. 게이트 통과 시점에 `git add devflow-docs/` 커밋해서 동기화 갈등을 차단.
    15	
    16	---
    17	
    18	## 1. 합의 사항 (3-AI 일치)
    19	
    20	### A. App ↔ ContextSwitcher 소유관계가 LIST에 미정의 (전원 합의)
    21	- **Codex**: App을 Service에서 Controller로 재분류, ContextSwitcher가 atomic boundary 보유
    22	- **Gemini**: ContextSwitcher가 Commit 결과로 new epoch 반환, 또는 App이 `increment_epoch()` 제공
    23	- **Claude**: "App owns Switcher; Switcher mutates context via App-provided handle" 명문화
    24	
    25	→ **결정**: LIST에 의존 방향 한 줄 추가. App이 Switcher를 소유, Switcher는 commit 결과로 `(new_epoch, snapshot)` 반환.
    26	
    27	### B. PR 시퀀싱이 unsafe intermediate state를 만든다 (전원 합의)
    28	- **Codex**: PR3/PR4가 PR5 안전 가시성 전에 사용자 노출 → 컨텍스트 인디케이터 없이 전환 가능
    29	- **Gemini**: PR1 plumbing이 Action/AppEvent 변경까지 포함해야 — 범위 과소평가
    30	- **Claude**: PR2가 PR1 미포함 시 stale 이벤트 누설
    31	
    32	→ **결정**:
    33	1. PR1 + PR2 통합 — "safety infra + switch core" 단일 PR (PR1만으로 사용자 노출 0이라 분리 가치 약함, 통합 시 stale 누설 창 자체 제거)
    34	2. PR3/PR4 진입 전에 ContextIndicator + ConfirmDialog fingerprint 필수 — PR5의 일부를 **새로운 PR3**로 앞당김
    35	3. PR 매핑에 `Depends on:` 컬럼 추가
    36	
    37	### C. UI 상태 invalidation 신호 누락 (Gemini 단독 — 합의 채택)
    38	- **Gemini**: ContextEpoch는 stale 이벤트 차단만 함. UI에 잔존 데이터(이전 cloud의 server list 등)는 사용자에게 거짓 정보 표시
    39	
    40	→ **결정**: `AppEvent::ContextChanged { target }` 추가. resource_list / detail_view / 모든 모듈 컴포넌트가 이 이벤트 처리해 내부 데이터 비우기.
    41	
    42	### D. 에피스 plumbing 범위 (Gemini + Claude)
    43	- **Gemini**: Action/AppEvent 모두 epoch stamp 필요. 매 variant 수정 회피 위해 `VersionedEvent { event, epoch: u64 }` envelope
    44	- **Claude**: 모든 spawn 시그니처가 `(epoch, cancel_token)` 페어 강제
    45	
    46	→ **결정**: VersionedEvent envelope + WorkerSpawn API 개정으로 LIST에 명시.
    47	
    48	---
    49	
    50	## 2. Codex 단독 — 채택할 추가 컴포넌트 (Critical)
    51	
    52	| 컴포넌트 | 책임 | 채택 사유 |
    53	|---------|------|----------|
    54	| `ContextSessionPort` (Port) + `ScopedAuthSession` (Service) | 활성 scoped token + endpoint cache 무효화의 atomic begin/commit/rollback | `AuthProvider`에 scope-switch API 부재. `KeystoneAuthAdapter.active_scope`는 fixed state. atomic boundary 없으면 rescope 성공 + stale endpoint 호출 사고 가능 |
    55	| `EndpointCatalogInvalidator` (Service) | 모든 HTTP client의 endpoint cache 일괄 무효화 | `src/adapter/http/base.rs:66`의 매뉴얼 invalidate 자동화 |
    56	| `ContextHistoryStore` (Util) | switch-back 1단계 히스토리 영속화 | 현재 LIST에 누락. Claude도 ContextSnapshot으로 동일 지적 |
    57	| `ContextTargetResolver` (Service) | name/uuid/cloud-prefix → ContextTarget 변환, 충돌 disambiguation | 명령·피커·Identity `s` 액션 셋 모두 같은 로직 필요. 공유하지 않으면 3중 구현 |
    58	
    59	→ **모두 채택**. ContextHistoryStore는 Claude의 ContextSnapshot과 통합.
    60	
    61	## 3. 재분류 / 명명 변경 (Codex 합의)
    62	
    63	| 컴포넌트 | 변경 | 사유 |
    64	|---------|------|------|
    65	| `ContextIndicator` | Controller → UI Widget | 단순 표시 위젯, Component trait 구현 (src/ui/context_indicator.rs) |
    66	| `ContextPicker` | Controller → UI Widget (modal) | 동일 — src/ui/context_picker.rs |
    67	| `App` | Service → Controller (orchestrator) | 코드베이스의 실제 App 역할이 라우터/오케스트레이터 |
    68	| `CommandRegistry` | → `CommandParser 확장` | src/input/command.rs의 실제 명명에 일치 |
    69	| `ContextSwitcher` | → `RuntimeContextSwitcher` (Claude 제안) | 보류 — 이름 길이/관행 검토 후 결정. 일단 ContextSwitcher 유지 |
    70	
    71	## 4. 테스트 시즘 보강 (Claude 단독 — 채택)
    72	
    73	- `port::auth::MockAuthProvider`에 rescope mock 추가 (기존 `src/port/mock.rs` 확장)
    74	- `ContextSessionPort` 도입으로 fault-injection seam 자연 확보 (rescope OK + invalidate fail 시뮬레이션 가능)
    75	
    76	## 5. 개정된 PR 매핑 (안)
    77	
    78	| PR | 컴포넌트 | Depends on |
    79	|----|---------|-----------|
    80	| **PR1+2 통합** ("safety infra + switch core") | ContextEpoch + CancellationRegistry, VersionedEvent envelope, AppEvent::ContextChanged, Action epoch 필드, Worker epoch+cancel 검증, SwitchStateMachine, ContextSwitcher, ContextSessionPort + ScopedAuthSession, KeystoneRescopeAdapter, EndpointCatalogInvalidator, TokenCacheStore 확장, ContextHistoryStore, ContextTargetResolver, App 통합, port mock 확장 | — |
    81	| **PR3** (안전 가시성 + 명령) | ContextIndicator (UI widget), StatusBar 통합, ConfirmDialog fingerprint, CommandParser 확장 (`:switch-*`, `:switch-back`) | PR1+2 |
    82	| **PR4** (피커 UI) | ContextPicker (UI modal), KeyMap 글로벌 단축키 (Ctrl+P) | PR3 |
    83	| **PR5** (Identity 통합) | Project Module 모듈-로컬 `s` 핸들러 (KeyMap 글로벌 등록 회피) | PR3 |
    84	
    85	→ **PR 수: 6개 → 4개로 축소**. PR1+2 통합으로 stale 누설 창 제거, PR3에 안전 가시성 묶어 사용자 노출 시점에 안전성 보장.
    86	
    87	## 6. 반려된 제안
    88	
    89	- **Codex**: ContextSwitcher 명명 변경 — 보류 (코드베이스 전례 없으나 명확성에서 양호)
    90	- **Gemini**: AppEvent에 epoch 직접 추가 — 거부, 대신 VersionedEvent envelope 채택 (variant 폭증 회피)
    91	
    92	---
    93	
    94	## 최종 Verdict
    95	
    96	**APPROVE-WITH-CHANGES** — LIST를 위 결정대로 개정 후 DETAIL 진입.
    97	
    98	### 개정 체크리스트 (LIST → DETAIL 진입 전)
    99	
   100	- [ ] App ↔ ContextSwitcher 의존 방향 1줄 추가
   101	- [ ] ContextSessionPort + ScopedAuthSession 추가 (Port + Service)
   102	- [ ] EndpointCatalogInvalidator 추가
   103	- [ ] ContextHistoryStore 추가
   104	- [ ] ContextTargetResolver 추가
   105	- [ ] AppEvent::ContextChanged 추가
   106	- [ ] VersionedEvent envelope (Action/AppEvent) 추가
   107	- [ ] port mock 확장 명시
   108	- [ ] ContextIndicator/ContextPicker → UI Widget 재분류
   109	- [ ] App → Controller 재분류
   110	- [ ] CommandRegistry → CommandParser 확장으로 표기 변경
   111	- [ ] PR1+PR2 통합, PR3에 안전 가시성 묶음
   112	- [ ] PR 매핑 표에 Depends on 컬럼 추가
   113	- [ ] PR6 KeyMap 분리 → PR5에서 모듈-로컬 핸들러로 변경

codex
I’ve pulled the top-level DETAIL definitions; next I’m extracting the rest of the DETAIL section and then I’ll cross-check each proposed interface against current code signatures in `src/*` for concrete mismatches.
exec
/bin/zsh -lc "nl -ba devflow-docs/inception/application-design.md | sed -n '260,560p'" in /Users/jay.ahn/projects/infra/nexttui/.worktrees/runtime-context-switch
 succeeded in 0ms:
   260	**Responsibility**: `ContextSessionPort`의 테스트 더블. fault-injection.
   261	**Interface**:
   262	- `pub fn with_rescope_failure(self, err: SwitchError) -> Self`
   263	- `pub fn with_invalidate_failure(self, err: SwitchError) -> Self`
   264	- `pub fn captured_targets(&self) -> Vec<ContextTarget>`
   265	**Dependencies**: `ContextSessionPort` trait 구현
   266	
   267	## 변경 컴포넌트 상세
   268	
   269	### App (src/app.rs) — Controller (재분류)
   270	**변경 인터페이스 (추가)**:
   271	- `pub fn current_context(&self) -> &ContextSnapshot`
   272	- `pub async fn switch_context(&mut self, target: ContextTarget) -> Result<(), SwitchError>` — ContextSwitcher 위임 + AppEvent::ContextChanged 디스패치
   273	- `pub fn switcher(&self) -> &Arc<ContextSwitcher>`
   274	**Dependencies (추가)**: `ContextSwitcher`, `ContextEpoch`, `ContextHistoryStore`
   275	
   276	### Worker (src/worker.rs) — Service
   277	**변경 인터페이스 (강제 시그니처 개정)**:
   278	- `pub fn spawn(epoch: Epoch, cancel: CancellationToken, fut: impl Future<Output = AppEvent>) -> JoinHandle<()>` — 기존 `tokio::spawn` 호출부 일괄 교체
   279	- 내부: `tokio::select! { _ = cancel.cancelled() => {}, ev = fut => app_tx.send(VersionedEvent::new(ev, epoch)) }`
   280	- run_worker는 epoch 비교 후 stale event drop
   281	**Dependencies (추가)**: `CancellationRegistry`, `ContextEpoch`, `VersionedEvent`
   282	
   283	### Action (src/action.rs) — Type
   284	**변경**: 외부에서 발행되는 모든 Action을 `VersionedEvent<Action>`으로 envelope. App 디스패처에서 unwrap → epoch 검증.
   285	**Interface 변화**: 신규 variant 없음 (envelope만)
   286	**Dependencies (추가)**: `VersionedEvent`, `ContextTarget` (`Action::SwitchContext`, `Action::SwitchBack` variant 추가)
   287	
   288	### AppEvent (src/event.rs) — Type
   289	**변경**: `VersionedEvent<AppEvent>` envelope + `ContextChanged { target, epoch }` variant 추가
   290	**Dependencies (추가)**: `VersionedEvent`, `ContextTarget`
   291	
   292	### CommandParser (src/input/command.rs) — Controller
   293	**변경 인터페이스 (추가 명령)**:
   294	- `:switch-project <name|uuid|cloud/project>` → `Action::SwitchContext`
   295	- `:switch-cloud <name>` → `Action::SwitchContext` (project=현재 cloud 기본값)
   296	- `:switch-back` → `Action::SwitchBack`
   297	- 충돌 시 `ContextTargetResolver`의 `Ambiguous` → 후보 출력 + 재선택 안내
   298	- tab 자동완성: resolver의 `list_user_projects()` 사용
   299	**Dependencies (추가)**: `ContextTargetResolver`
   300	
   301	### ConfirmDialog (src/ui/confirm.rs) — UI Widget
   302	**변경 인터페이스**:
   303	- `pub fn with_context_fingerprint(self, snapshot: &ContextSnapshot) -> Self` — `cloud • project` 표시 라인 추가
   304	- `pub fn require_recontext_confirm(self, recently_switched: bool) -> Self` — 직전 switch 시 추가 확인 강제
   305	**Dependencies (추가)**: `ContextSnapshot`
   306	
   307	### StatusBar (src/ui/status_bar.rs) — UI Widget
   308	**변경**: 상단/하단 임베드 위치 결정 + ContextIndicator 슬롯 추가
   309	**Interface (추가)**: `pub fn set_context_indicator(&mut self, indicator: Arc<RwLock<ContextIndicator>>)`
   310	**Dependencies (추가)**: `ContextIndicator`
   311	
   312	### TokenCacheStore (src/adapter/auth/token_cache.rs) — Repository
   313	**변경 인터페이스**: 기존 `save_token`/`load_all_tokens`은 유지. 추가:
   314	- `pub fn store_rescoped(&self, target: &ContextTarget, token: Token) -> Result<(), io::Error>`
   315	- `pub fn lookup(&self, target: &ContextTarget) -> Option<Token>`
   316	- 만료 (`token.expires_at < now`) 시 `None` 반환 (재취득 트리거)
   317	**Dependencies**: 기존 + `ContextTarget`
   318	
   319	### Project Module (src/module/project/mod.rs) — Controller
   320	**변경 인터페이스**:
   321	- 모듈-로컬 `s` 키 핸들러: 현재 행의 project → `Action::SwitchContext { target }` 발행
   322	- KeyMap 글로벌 등록 회피 (모듈 내 `handle_key`에서만 처리)
   323	- Enter는 기존 Detail 진입 유지
   324	**Dependencies (추가)**: `ContextTarget`
   325	
   326	## 의존 그래프 요약
   327	
   328	```
   329	                  +-------------------+
   330	                  |        App        |  (Controller)
   331	                  +---------+---------+
   332	                            |
   333	                            v
   334	                  +-------------------+
   335	                  |  ContextSwitcher  |  (Service, orchestrator)
   336	                  +---------+---------+
   337	                            |
   338	        +-----------+-------+-------+----------------+--------------+
   339	        v           v               v                v              v
   340	   StateMachine  Registry  ContextSessionPort  EndpointCatalog  HistoryStore
   341	                                |
   342	                                v
   343	                       ScopedAuthSession  (Service)
   344	                                |
   345	              +-----------------+------------------+
   346	              v                 v                  v
   347	       KeystoneRescope    TokenCacheStore     (uses) AuthProvider
   348	        Adapter
   349	
   350	  Resolver (ContextTargetResolver) ── used by CommandParser, ContextPicker, Project Module
   351	  VersionedEvent envelope ── wraps Action / AppEvent across Worker
   352	```

exec
/bin/zsh -lc "nl -ba src/port/auth.rs | sed -n '1,260p'" in /Users/jay.ahn/projects/infra/nexttui/.worktrees/runtime-context-switch
 succeeded in 0ms:
     1	use async_trait::async_trait;
     2	use tokio::sync::broadcast;
     3	
     4	use super::error::ApiResult;
     5	use super::types::*;
     6	
     7	#[async_trait]
     8	pub trait AuthProvider: Send + Sync {
     9	    async fn authenticate(&self, credential: &AuthCredential) -> ApiResult<Token>;
    10	    async fn refresh_token(&self) -> ApiResult<Token>;
    11	    async fn get_token(&self) -> ApiResult<String>;
    12	    async fn get_token_info(&self) -> ApiResult<Token>;
    13	    async fn authenticate_request(
    14	        &self,
    15	        method: &str,
    16	        url: &str,
    17	        headers: &reqwest::header::HeaderMap,
    18	        body: Option<&[u8]>,
    19	    ) -> ApiResult<AuthHeaders>;
    20	    async fn get_endpoint(
    21	        &self,
    22	        service_type: &str,
    23	        interface: EndpointInterface,
    24	        region: Option<&str>,
    25	    ) -> ApiResult<String>;
    26	    fn subscribe_token_refresh(&self) -> broadcast::Receiver<Token>;
    27	    async fn has_role(&self, role_name: &str) -> ApiResult<bool>;
    28	    async fn get_catalog(&self) -> ApiResult<Vec<CatalogEntry>>;
    29	    async fn get_capabilities(&self) -> ApiResult<Vec<Capability>>;
    30	}

exec
/bin/zsh -lc "nl -ba src/adapter/auth/keystone.rs | sed -n '1,320p'" in /Users/jay.ahn/projects/infra/nexttui/.worktrees/runtime-context-switch
 succeeded in 0ms:
     1	use std::collections::HashMap;
     2	use std::path::PathBuf;
     3	use std::sync::atomic::{AtomicBool, Ordering};
     4	use std::sync::Arc;
     5	use std::time::Duration;
     6	
     7	use async_trait::async_trait;
     8	use chrono::{DateTime, Utc};
     9	use serde::Deserialize;
    10	use tokio::sync::{broadcast, Mutex, RwLock};
    11	use tokio::task::JoinHandle;
    12	use tracing::Instrument;
    13	
    14	use crate::port::auth::AuthProvider;
    15	use crate::port::error::{ApiError, ApiResult};
    16	use crate::port::types::*;
    17	
    18	// --- Keystone v3 response types (internal) ---
    19	
    20	#[derive(Debug, Deserialize)]
    21	struct KeystoneTokenResponse {
    22	    token: KeystoneTokenBody,
    23	}
    24	
    25	#[derive(Debug, Deserialize)]
    26	struct KeystoneTokenBody {
    27	    expires_at: DateTime<Utc>,
    28	    project: Option<KeystoneProject>,
    29	    roles: Vec<KeystoneRole>,
    30	    catalog: Option<Vec<KeystoneCatalogEntry>>,
    31	}
    32	
    33	#[derive(Debug, Deserialize)]
    34	struct KeystoneProject {
    35	    id: String,
    36	    name: String,
    37	    domain: KeystoneDomain,
    38	}
    39	
    40	#[derive(Debug, Deserialize)]
    41	struct KeystoneDomain {
    42	    id: String,
    43	    name: String,
    44	}
    45	
    46	#[derive(Debug, Deserialize)]
    47	struct KeystoneRole {
    48	    id: String,
    49	    name: String,
    50	}
    51	
    52	#[derive(Debug, Deserialize)]
    53	struct KeystoneCatalogEntry {
    54	    #[serde(rename = "type")]
    55	    service_type: String,
    56	    name: String,
    57	    endpoints: Vec<KeystoneEndpoint>,
    58	}
    59	
    60	#[derive(Debug, Deserialize)]
    61	struct KeystoneEndpoint {
    62	    url: String,
    63	    interface: String,
    64	    region_id: String,
    65	}
    66	
    67	// --- Token conversion ---
    68	
    69	fn parse_token(token_id: String, resp: KeystoneTokenResponse) -> Token {
    70	    let body = resp.token;
    71	    let project = body
    72	        .project
    73	        .map(|p| ProjectScope {
    74	            id: p.id,
    75	            name: p.name,
    76	            domain_id: p.domain.id,
    77	            domain_name: p.domain.name,
    78	        })
    79	        .unwrap_or(ProjectScope {
    80	            id: String::new(),
    81	            name: String::new(),
    82	            domain_id: String::new(),
    83	            domain_name: String::new(),
    84	        });
    85	    let roles = body
    86	        .roles
    87	        .into_iter()
    88	        .map(|r| TokenRole {
    89	            id: r.id,
    90	            name: r.name,
    91	        })
    92	        .collect();
    93	    let catalog = body
    94	        .catalog
    95	        .unwrap_or_default()
    96	        .into_iter()
    97	        .map(|c| CatalogEntry {
    98	            service_type: c.service_type,
    99	            service_name: c.name,
   100	            endpoints: c
   101	                .endpoints
   102	                .into_iter()
   103	                .map(|e| Endpoint {
   104	                    url: e.url,
   105	                    interface: parse_interface(&e.interface),
   106	                    region: e.region_id,
   107	                })
   108	                .collect(),
   109	        })
   110	        .collect();
   111	
   112	    Token {
   113	        id: token_id,
   114	        expires_at: body.expires_at,
   115	        project,
   116	        roles,
   117	        catalog,
   118	    }
   119	}
   120	
   121	fn parse_interface(s: &str) -> EndpointInterface {
   122	    match s {
   123	        "internal" => EndpointInterface::Internal,
   124	        "admin" => EndpointInterface::Admin,
   125	        _ => EndpointInterface::Public,
   126	    }
   127	}
   128	
   129	// --- KeystoneAuthAdapter ---
   130	
   131	pub struct KeystoneAuthAdapter {
   132	    client: reqwest::Client,
   133	    credential: AuthCredential,
   134	    token_map: Arc<RwLock<HashMap<TokenScope, Token>>>,
   135	    active_scope: TokenScope,
   136	    token_tx: broadcast::Sender<Token>,
   137	    refresh_handle: Mutex<Option<JoinHandle<()>>>,
   138	    /// Guard to ensure refresh loop is started only once.
   139	    refresh_started: AtomicBool,
   140	    /// Mutex to serialize concurrent refresh attempts (prevents thundering herd).
   141	    refresh_lock: Mutex<()>,
   142	    /// Directory for scope-keyed token cache files.
   143	    cache_dir: PathBuf,
   144	}
   145	
   146	impl KeystoneAuthAdapter {
   147	    pub fn new(credential: AuthCredential) -> Result<Self, ApiError> {
   148	        use super::token_cache;
   149	
   150	        let username = match &credential.method {
   151	            AuthMethod::Password { username, .. } => username.clone(),
   152	            AuthMethod::ApplicationCredential { id, .. } => id.clone(),
   153	        };
   154	        let active_scope = TokenScope::from_credential(&credential);
   155	        let cloud_key = token_cache::compute_cloud_key(&credential.auth_url, &username);
   156	        let cache_dir = token_cache::cache_dir_path(&cloud_key);
   157	
   158	        // Load all cached tokens for this cloud from disk
   159	        let cached_tokens = token_cache::load_all_tokens(&cache_dir);
   160	
   161	        let (token_tx, _) = broadcast::channel::<Token>(16);
   162	        Ok(Self {
   163	            client: reqwest::Client::builder()
   164	                .timeout(Duration::from_secs(30))
   165	                .connect_timeout(Duration::from_secs(10))
   166	                .build()?,
   167	            credential,
   168	            token_map: Arc::new(RwLock::new(cached_tokens)),
   169	            active_scope,
   170	            token_tx,
   171	            refresh_handle: Mutex::new(None),
   172	            refresh_started: AtomicBool::new(false),
   173	            refresh_lock: Mutex::new(()),
   174	            cache_dir,
   175	        })
   176	    }
   177	
   178	    /// Start the background token refresh loop. Idempotent — only spawns once.
   179	    #[tracing::instrument(skip(self))]
   180	    async fn start_refresh_loop(&self) {
   181	        if self.refresh_started.swap(true, Ordering::SeqCst) {
   182	            return; // Already started
   183	        }
   184	
   185	        let token_map_ref = self.token_map.clone();
   186	        let client = self.client.clone();
   187	        let credential = self.credential.clone();
   188	        let tx = self.token_tx.clone();
   189	        let cache_dir = self.cache_dir.clone();
   190	        let scope = self.active_scope.clone();
   191	
   192	        let refresh_span = tracing::info_span!("token_refresh_loop");
   193	        let handle = tokio::spawn(
   194	            async move {
   195	                loop {
   196	                    let sleep_duration = {
   197	                        let map = token_map_ref.read().await;
   198	                        match map.get(&scope) {
   199	                            Some(t) => {
   200	                                let remaining = t.expires_at - Utc::now();
   201	                                let refresh_at = remaining - chrono::Duration::minutes(5);
   202	                                if refresh_at.num_seconds() > 0 {
   203	                                    Duration::from_secs(refresh_at.num_seconds() as u64)
   204	                                } else {
   205	                                    Duration::from_secs(10)
   206	                                }
   207	                            }
   208	                            None => Duration::from_secs(60),
   209	                        }
   210	                    };
   211	
   212	                    tokio::time::sleep(sleep_duration).await;
   213	
   214	                    match Self::do_authenticate(&client, &credential).await {
   215	                        Ok(new_token) => {
   216	                            let mut map = token_map_ref.write().await;
   217	                            map.insert(scope.clone(), new_token.clone());
   218	                            if let Err(e) = super::token_cache::save_token(&new_token, &cache_dir, &scope) {
   219	                                tracing::warn!(error = %e, "failed to cache token to disk");
   220	                            }
   221	                            let _ = tx.send(new_token);
   222	                        }
   223	                        Err(e) => {
   224	                            tracing::warn!(error = %e, "token refresh failed, retrying in 30s");
   225	                            tokio::time::sleep(Duration::from_secs(30)).await;
   226	                        }
   227	                    }
   228	                }
   229	            }
   230	            .instrument(refresh_span),
   231	        );
   232	
   233	        let mut h = self.refresh_handle.lock().await;
   234	        *h = Some(handle);
   235	    }
   236	
   237	    /// Perform the actual Keystone v3 auth POST.
   238	    #[tracing::instrument(skip(client, credential), fields(auth_url = %credential.auth_url))]
   239	    async fn do_authenticate(
   240	        client: &reqwest::Client,
   241	        credential: &AuthCredential,
   242	    ) -> ApiResult<Token> {
   243	        let auth_url = format!(
   244	            "{}/auth/tokens",
   245	            credential.auth_url.trim_end_matches('/')
   246	        );
   247	        let body = Self::build_auth_body(credential);
   248	        let resp = client
   249	            .post(&auth_url)
   250	            .json(&body)
   251	            .send()
   252	            .await
   253	            .map_err(ApiError::Network)?;
   254	
   255	        if !resp.status().is_success() {
   256	            let body = resp.text().await.unwrap_or_default();
   257	            return Err(ApiError::AuthFailed(body));
   258	        }
   259	
   260	        let token_id = resp
   261	            .headers()
   262	            .get("X-Subject-Token")
   263	            .and_then(|v| v.to_str().ok())
   264	            .ok_or_else(|| ApiError::AuthFailed("Missing X-Subject-Token header".into()))?
   265	            .to_string();
   266	
   267	        let body: KeystoneTokenResponse = resp
   268	            .json()
   269	            .await
   270	            .map_err(|e| ApiError::Parse(format!("Failed to parse Keystone response: {e}")))?;
   271	
   272	        Ok(parse_token(token_id, body))
   273	    }
   274	
   275	    /// Build the Keystone v3 auth request body based on AuthMethod.
   276	    /// Note: authenticate() always uses self.credential (passed at construction).
   277	    /// The credential parameter in AuthProvider::authenticate() should match self.credential.
   278	    pub(crate) fn build_auth_body(credential: &AuthCredential) -> serde_json::Value {
   279	        let identity = match &credential.method {
   280	            AuthMethod::Password {
   281	                username,
   282	                password,
   283	                domain_name,
   284	            } => serde_json::json!({
   285	                "methods": ["password"],
   286	                "password": {
   287	                    "user": {
   288	                        "name": username,
   289	                        "password": password,
   290	                        "domain": { "name": domain_name }
   291	                    }
   292	                }
   293	            }),
   294	            AuthMethod::ApplicationCredential { id, secret } => serde_json::json!({
   295	                "methods": ["application_credential"],
   296	                "application_credential": {
   297	                    "id": id,
   298	                    "secret": secret
   299	                }
   300	            }),
   301	        };
   302	
   303	        let mut auth = serde_json::json!({ "identity": identity });
   304	
   305	        if let Some(ref scope) = credential.project_scope {
   306	            auth["scope"] = serde_json::json!({
   307	                "project": {
   308	                    "name": scope.name,
   309	                    "domain": { "name": scope.domain_name }
   310	                }
   311	            });
   312	        }
   313	
   314	        serde_json::json!({ "auth": auth })
   315	    }
   316	}
   317	
   318	#[async_trait]
   319	impl AuthProvider for KeystoneAuthAdapter {
   320	    async fn authenticate(&self, credential: &AuthCredential) -> ApiResult<Token> {

exec
/bin/zsh -lc "nl -ba src/adapter/auth/token_cache.rs | sed -n '1,320p'" in /Users/jay.ahn/projects/infra/nexttui/.worktrees/runtime-context-switch
 succeeded in 0ms:
     1	//! Token cache persistence: save/load Keystone tokens to disk.
     2	//!
     3	//! Cache layout: `~/.cache/nexttui/auth/{cloud_key}/{scope_key}`
     4	//! File permissions: 0o600 (Unix only)
     5	
     6	use std::collections::HashMap;
     7	use std::path::{Path, PathBuf};
     8	
     9	use crate::port::types::{Token, TokenScope};
    10	
    11	/// Compute a deterministic cache key from cloud config fields.
    12	/// Uses a simple FNV-1a 64-bit hash (stable across Rust versions, no external deps).
    13	pub fn compute_cloud_key(auth_url: &str, username: &str) -> String {
    14	    let input = format!("{auth_url}|{username}");
    15	    let hash = fnv1a_64(input.as_bytes());
    16	    format!("{hash:016x}")
    17	}
    18	
    19	/// FNV-1a 64-bit hash — deterministic, no external dependency.
    20	fn fnv1a_64(data: &[u8]) -> u64 {
    21	    const FNV_OFFSET: u64 = 0xcbf29ce484222325;
    22	    const FNV_PRIME: u64 = 0x00000100000001B3;
    23	    let mut hash = FNV_OFFSET;
    24	    for &byte in data {
    25	        hash ^= byte as u64;
    26	        hash = hash.wrapping_mul(FNV_PRIME);
    27	    }
    28	    hash
    29	}
    30	
    31	/// Resolve the cache directory path for a given cloud key.
    32	pub fn cache_dir_path(cloud_key: &str) -> PathBuf {
    33	    dirs::cache_dir()
    34	        .unwrap_or_else(|| PathBuf::from("/tmp"))
    35	        .join("nexttui")
    36	        .join("auth")
    37	        .join(cloud_key)
    38	}
    39	
    40	/// Save a token to the cache directory, keyed by scope.
    41	/// Creates parent directories if needed.
    42	/// On Unix, creates the file with 0o600 permissions atomically (no TOCTOU window).
    43	pub fn save_token(token: &Token, cache_dir: &Path, scope: &TokenScope) -> Result<(), std::io::Error> {
    44	    use std::io::Write;
    45	
    46	    // Create cache directory with restricted permissions on Unix (0o700)
    47	    #[cfg(unix)]
    48	    {
    49	        use std::fs::DirBuilder;
    50	        use std::os::unix::fs::DirBuilderExt;
    51	        DirBuilder::new().recursive(true).mode(0o700).create(cache_dir)?;
    52	    }
    53	    #[cfg(not(unix))]
    54	    {
    55	        std::fs::create_dir_all(cache_dir)?;
    56	    }
    57	    let path = cache_dir.join(scope.cache_key());
    58	    let data = serde_json::to_vec(token)
    59	        .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e))?;
    60	
    61	    // NOTE: Token ID is stored in plaintext JSON. File permissions (0o600) provide
    62	    // basic protection. Encryption (AES-GCM / OS keychain) is tracked as BL-P2-016.
    63	    #[cfg(unix)]
    64	    {
    65	        use std::os::unix::fs::OpenOptionsExt;
    66	        let mut file = std::fs::OpenOptions::new()
    67	            .write(true)
    68	            .create(true)
    69	            .truncate(true)
    70	            .mode(0o600)
    71	            .open(&path)?;
    72	        file.write_all(&data)?;
    73	    }
    74	
    75	    #[cfg(not(unix))]
    76	    {
    77	        std::fs::write(&path, &data)?;
    78	    }
    79	
    80	    tracing::debug!(path = %path.display(), "token cached to disk");
    81	    Ok(())
    82	}
    83	
    84	/// Load a single token from a cache file.
    85	/// Returns None if the file doesn't exist, is unreadable, or the token is expired.
    86	/// Automatically deletes expired token files.
    87	fn load_token_file(path: &Path) -> Option<Token> {
    88	    let data = std::fs::read(path).ok()?;
    89	    let token: Token = serde_json::from_slice(&data).ok()?;
    90	
    91	    if token.expires_at > chrono::Utc::now() + chrono::Duration::minutes(1) {
    92	        Some(token)
    93	    } else {
    94	        tracing::info!(path = %path.display(), "cached token expired, removing");
    95	        let _ = std::fs::remove_file(path);
    96	        None
    97	    }
    98	}
    99	
   100	/// Load all valid cached tokens from the cache directory.
   101	/// Returns a map of scope → token. Expired tokens are auto-deleted.
   102	/// Skips non-files and unrecognized filenames.
   103	pub fn load_all_tokens(cache_dir: &Path) -> HashMap<TokenScope, Token> {
   104	    let mut map = HashMap::new();
   105	    let entries = match std::fs::read_dir(cache_dir) {
   106	        Ok(entries) => entries,
   107	        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return map,
   108	        Err(e) => {
   109	            tracing::warn!(path = %cache_dir.display(), error = %e, "failed to read token cache directory");
   110	            return map;
   111	        }
   112	    };
   113	
   114	    for entry in entries.flatten() {
   115	        // Skip non-files (directories, symlinks, etc.)
   116	        if !entry.file_type().map(|t| t.is_file()).unwrap_or(false) {
   117	            continue;
   118	        }
   119	
   120	        let file_name = entry.file_name();
   121	        let scope_key = file_name.to_string_lossy();
   122	
   123	        let scope = match parse_scope_from_filename(&scope_key) {
   124	            Some(s) => s,
   125	            None => {
   126	                tracing::warn!(filename = %scope_key, "unrecognized token cache file, skipping");
   127	                continue;
   128	            }
   129	        };
   130	
   131	        if let Some(token) = load_token_file(&entry.path()) {
   132	            tracing::debug!("loaded cached token from disk");
   133	            map.insert(scope, token);
   134	        }
   135	    }
   136	    map
   137	}
   138	
   139	/// Parse a TokenScope from a cache filename.
   140	/// Returns None for unrecognized filenames.
   141	fn parse_scope_from_filename(filename: &str) -> Option<TokenScope> {
   142	    if filename == "unscoped" {
   143	        return Some(TokenScope::Unscoped);
   144	    }
   145	    // Format: "project@{name}@{domain}" (@ separator avoids _ ambiguity)
   146	    if let Some(rest) = filename.strip_prefix("project@") {
   147	        if let Some((name, domain)) = rest.split_once('@') {
   148	            return Some(TokenScope::Project {
   149	                name: name.to_string(),
   150	                domain: domain.to_string(),
   151	            });
   152	        }
   153	    }
   154	    None
   155	}
   156	
   157	#[cfg(test)]
   158	mod tests {
   159	    use super::*;
   160	    use chrono::{Duration, Utc};
   161	    use crate::port::types::*;
   162	    use tempfile::TempDir;
   163	
   164	    fn sample_token(expires_in_minutes: i64) -> Token {
   165	        Token {
   166	            id: "tok-test-123".to_string(),
   167	            expires_at: Utc::now() + Duration::minutes(expires_in_minutes),
   168	            project: ProjectScope {
   169	                id: "proj-1".to_string(),
   170	                name: "admin".to_string(),
   171	                domain_id: "default".to_string(),
   172	                domain_name: "Default".to_string(),
   173	            },
   174	            roles: vec![TokenRole {
   175	                id: "role-1".to_string(),
   176	                name: "admin".to_string(),
   177	            }],
   178	            catalog: vec![CatalogEntry {
   179	                service_type: "compute".to_string(),
   180	                service_name: "nova".to_string(),
   181	                endpoints: vec![Endpoint {
   182	                    region: "RegionOne".to_string(),
   183	                    interface: EndpointInterface::Public,
   184	                    url: "https://nova:8774/v2.1".to_string(),
   185	                }],
   186	            }],
   187	        }
   188	    }
   189	
   190	    fn sample_scope() -> TokenScope {
   191	        TokenScope::Project {
   192	            name: "admin".to_string(),
   193	            domain: "default".to_string(),
   194	        }
   195	    }
   196	
   197	    #[test]
   198	    fn test_compute_cloud_key_deterministic() {
   199	        let k1 = compute_cloud_key("https://keystone:5000/v3", "admin");
   200	        let k2 = compute_cloud_key("https://keystone:5000/v3", "admin");
   201	        assert_eq!(k1, k2);
   202	        assert_eq!(k1.len(), 16);
   203	    }
   204	
   205	    #[test]
   206	    fn test_compute_cloud_key_different_inputs() {
   207	        let k1 = compute_cloud_key("https://keystone:5000/v3", "admin");
   208	        let k2 = compute_cloud_key("https://keystone:5000/v3", "user");
   209	        assert_ne!(k1, k2);
   210	    }
   211	
   212	    #[test]
   213	    fn test_token_scope_cache_key() {
   214	        let scope = TokenScope::Project {
   215	            name: "admin".to_string(),
   216	            domain: "default".to_string(),
   217	        };
   218	        assert_eq!(scope.cache_key(), "project@admin@default");
   219	        assert_eq!(TokenScope::Unscoped.cache_key(), "unscoped");
   220	    }
   221	
   222	    #[test]
   223	    fn test_cache_key_sanitizes_path_traversal() {
   224	        let scope = TokenScope::Project {
   225	            name: "../etc".to_string(),
   226	            domain: "default".to_string(),
   227	        };
   228	        // dots and slashes should be replaced with _
   229	        assert!(!scope.cache_key().contains('/'));
   230	        assert!(!scope.cache_key().contains(".."));
   231	    }
   232	
   233	    #[test]
   234	    fn test_cache_key_handles_underscore_in_name() {
   235	        let scope = TokenScope::Project {
   236	            name: "my_project".to_string(),
   237	            domain: "my_domain".to_string(),
   238	        };
   239	        // @ separator means underscores in name/domain are preserved correctly
   240	        assert_eq!(scope.cache_key(), "project@my_project@my_domain");
   241	    }
   242	
   243	    #[test]
   244	    fn test_save_and_load_scoped_token() {
   245	        let dir = TempDir::new().unwrap();
   246	        let cache_dir = dir.path().join("cloud-abc");
   247	        let scope = sample_scope();
   248	
   249	        let token = sample_token(60);
   250	        save_token(&token, &cache_dir, &scope).unwrap();
   251	
   252	        let loaded = load_all_tokens(&cache_dir);
   253	        assert_eq!(loaded.len(), 1);
   254	        let loaded_token = loaded.get(&scope).unwrap();
   255	        assert_eq!(loaded_token.id, "tok-test-123");
   256	    }
   257	
   258	    #[test]
   259	    fn test_save_multiple_scopes() {
   260	        let dir = TempDir::new().unwrap();
   261	        let cache_dir = dir.path().join("cloud-multi");
   262	
   263	        let scope_a = TokenScope::Project {
   264	            name: "projectA".to_string(),
   265	            domain: "Default".to_string(),
   266	        };
   267	        let scope_b = TokenScope::Project {
   268	            name: "projectB".to_string(),
   269	            domain: "Default".to_string(),
   270	        };
   271	
   272	        let mut token_a = sample_token(60);
   273	        token_a.id = "tok-a".to_string();
   274	        let mut token_b = sample_token(60);
   275	        token_b.id = "tok-b".to_string();
   276	
   277	        save_token(&token_a, &cache_dir, &scope_a).unwrap();
   278	        save_token(&token_b, &cache_dir, &scope_b).unwrap();
   279	
   280	        let loaded = load_all_tokens(&cache_dir);
   281	        assert_eq!(loaded.len(), 2);
   282	        assert_eq!(loaded.get(&scope_a).unwrap().id, "tok-a");
   283	        assert_eq!(loaded.get(&scope_b).unwrap().id, "tok-b");
   284	    }
   285	
   286	    #[test]
   287	    fn test_load_expired_token_deleted() {
   288	        let dir = TempDir::new().unwrap();
   289	        let cache_dir = dir.path().join("cloud-expired");
   290	        let scope = sample_scope();
   291	
   292	        let token = sample_token(-10);
   293	        save_token(&token, &cache_dir, &scope).unwrap();
   294	
   295	        let loaded = load_all_tokens(&cache_dir);
   296	        assert!(loaded.is_empty());
   297	        assert!(!cache_dir.join(scope.cache_key()).exists());
   298	    }
   299	
   300	    #[test]
   301	    fn test_load_nonexistent_dir_returns_empty() {
   302	        let path = PathBuf::from("/tmp/nexttui-test-nonexistent-dir");
   303	        let loaded = load_all_tokens(&path);
   304	        assert!(loaded.is_empty());
   305	    }
   306	
   307	    #[test]
   308	    fn test_load_corrupt_file_skipped() {
   309	        let dir = TempDir::new().unwrap();
   310	        let cache_dir = dir.path().join("cloud-corrupt");
   311	        std::fs::create_dir_all(&cache_dir).unwrap();
   312	        std::fs::write(cache_dir.join("project_bad_Default"), b"not json").unwrap();
   313	
   314	        let loaded = load_all_tokens(&cache_dir);
   315	        assert!(loaded.is_empty());
   316	    }
   317	
   318	    #[cfg(unix)]
   319	    #[test]
   320	    fn test_save_sets_permissions_0o600() {

exec
/bin/zsh -lc "nl -ba src/adapter/auth/keystone.rs | sed -n '320,760p'" in /Users/jay.ahn/projects/infra/nexttui/.worktrees/runtime-context-switch
 succeeded in 0ms:
   320	    async fn authenticate(&self, credential: &AuthCredential) -> ApiResult<Token> {
   321	        let token = Self::do_authenticate(&self.client, credential).await?;
   322	        {
   323	            let mut map = self.token_map.write().await;
   324	            map.insert(self.active_scope.clone(), token.clone());
   325	        }
   326	        if let Err(e) = super::token_cache::save_token(&token, &self.cache_dir, &self.active_scope) {
   327	            tracing::warn!(error = %e, "failed to cache token to disk");
   328	        }
   329	        self.start_refresh_loop().await;
   330	        Ok(token)
   331	    }
   332	
   333	    #[tracing::instrument(skip(self))]
   334	    async fn refresh_token(&self) -> ApiResult<Token> {
   335	        let token = Self::do_authenticate(&self.client, &self.credential).await?;
   336	        {
   337	            let mut map = self.token_map.write().await;
   338	            map.insert(self.active_scope.clone(), token.clone());
   339	        }
   340	        if let Err(e) = super::token_cache::save_token(&token, &self.cache_dir, &self.active_scope) {
   341	            tracing::warn!(error = %e, "failed to cache token to disk");
   342	        }
   343	        let _ = self.token_tx.send(token.clone());
   344	        Ok(token)
   345	    }
   346	
   347	    /// Get a valid token string. If near-expiry (<1min), refresh first.
   348	    /// Uses a Mutex to prevent thundering herd — only one refresh at a time.
   349	    #[tracing::instrument(skip(self))]
   350	    async fn get_token(&self) -> ApiResult<String> {
   351	        // Ensure refresh loop is running (idempotent — handles cached token from disk)
   352	        self.start_refresh_loop().await;
   353	
   354	        // Fast path: token is still valid for active scope
   355	        {
   356	            let map = self.token_map.read().await;
   357	            if let Some(t) = map.get(&self.active_scope) {
   358	                if t.expires_at > Utc::now() + chrono::Duration::minutes(1) {
   359	                    return Ok(t.id.clone());
   360	                }
   361	            }
   362	        }
   363	
   364	        // Slow path: serialize refresh attempts
   365	        let _guard = self.refresh_lock.lock().await;
   366	
   367	        // Double-check after acquiring lock
   368	        {
   369	            let map = self.token_map.read().await;
   370	            if let Some(t) = map.get(&self.active_scope) {
   371	                if t.expires_at > Utc::now() + chrono::Duration::minutes(1) {
   372	                    return Ok(t.id.clone());
   373	                }
   374	            }
   375	        }
   376	
   377	        let token = self.refresh_token().await?;
   378	        Ok(token.id)
   379	    }
   380	
   381	    async fn get_token_info(&self) -> ApiResult<Token> {
   382	        let map = self.token_map.read().await;
   383	        map.get(&self.active_scope)
   384	            .cloned()
   385	            .ok_or(ApiError::AuthFailed("Not authenticated".into()))
   386	    }
   387	
   388	    /// Inject X-Auth-Token header. Phase 1: token-based auth only.
   389	    /// Phase 2 note: for signed auth (HMAC), this method will need the actual
   390	    /// method/url/headers/body to compute the signature. Currently unused parameters
   391	    /// are preserved in the signature for forward compatibility.
   392	    #[tracing::instrument(skip(self, _headers, _body))]
   393	    async fn authenticate_request(
   394	        &self,
   395	        _method: &str,
   396	        _url: &str,
   397	        _headers: &reqwest::header::HeaderMap,
   398	        _body: Option<&[u8]>,
   399	    ) -> ApiResult<AuthHeaders> {
   400	        let token_id = self.get_token().await?;
   401	        Ok(AuthHeaders {
   402	            headers: vec![("X-Auth-Token".to_string(), token_id)],
   403	        })
   404	    }
   405	
   406	    #[tracing::instrument(skip(self))]
   407	    async fn get_endpoint(
   408	        &self,
   409	        service_type: &str,
   410	        interface: EndpointInterface,
   411	        region: Option<&str>,
   412	    ) -> ApiResult<String> {
   413	        // Ensure we have a valid token (triggers initial auth if needed)
   414	        let _ = self.get_token().await?;
   415	
   416	        let token = self.get_token_info().await?;
   417	
   418	        token
   419	            .catalog
   420	            .iter()
   421	            .find(|c| c.service_type == service_type)
   422	            .and_then(|c| {
   423	                c.endpoints.iter().find(|e| {
   424	                    e.interface == interface && region.map_or(true, |r| e.region == r)
   425	                })
   426	            })
   427	            .map(|e| e.url.clone())
   428	            .ok_or(ApiError::ServiceUnavailable {
   429	                service: service_type.to_string(),
   430	            })
   431	    }
   432	
   433	    fn subscribe_token_refresh(&self) -> broadcast::Receiver<Token> {
   434	        self.token_tx.subscribe()
   435	    }
   436	
   437	    async fn has_role(&self, role_name: &str) -> ApiResult<bool> {
   438	        let map = self.token_map.read().await;
   439	        let token = map
   440	            .get(&self.active_scope)
   441	            .ok_or(ApiError::AuthFailed("Not authenticated".into()))?;
   442	        Ok(token.roles.iter().any(|r| r.name == role_name))
   443	    }
   444	
   445	    async fn get_catalog(&self) -> ApiResult<Vec<CatalogEntry>> {
   446	        let map = self.token_map.read().await;
   447	        let token = map
   448	            .get(&self.active_scope)
   449	            .ok_or(ApiError::AuthFailed("Not authenticated".into()))?;
   450	        Ok(token.catalog.clone())
   451	    }
   452	
   453	    async fn get_capabilities(&self) -> ApiResult<Vec<Capability>> {
   454	        // Phase 1: Keystone has no capability concept. Return empty.
   455	        Ok(Vec::new())
   456	    }
   457	}
   458	
   459	#[cfg(test)]
   460	mod tests {
   461	    use super::*;
   462	
   463	    fn sample_credential_password() -> AuthCredential {
   464	        AuthCredential {
   465	            auth_url: "https://keystone.example.com:5000/v3".to_string(),
   466	            method: AuthMethod::Password {
   467	                username: "admin".to_string(),
   468	                password: "secret123".to_string(),
   469	                domain_name: "Default".to_string(),
   470	            },
   471	            project_scope: Some(ProjectScopeParam {
   472	                name: "admin-project".to_string(),
   473	                domain_name: "Default".to_string(),
   474	            }),
   475	        }
   476	    }
   477	
   478	    fn sample_credential_app() -> AuthCredential {
   479	        AuthCredential {
   480	            auth_url: "https://keystone.example.com:5000/v3".to_string(),
   481	            method: AuthMethod::ApplicationCredential {
   482	                id: "app-cred-id".to_string(),
   483	                secret: "app-cred-secret".to_string(),
   484	            },
   485	            project_scope: None,
   486	        }
   487	    }
   488	
   489	    fn sample_keystone_response_json() -> &'static str {
   490	        r#"{
   491	            "token": {
   492	                "expires_at": "2099-12-31T23:59:59.000000Z",
   493	                "project": {
   494	                    "id": "proj-123",
   495	                    "name": "admin-project",
   496	                    "domain": { "id": "default", "name": "Default" }
   497	                },
   498	                "roles": [
   499	                    { "id": "role-1", "name": "admin" },
   500	                    { "id": "role-2", "name": "member" }
   501	                ],
   502	                "catalog": [
   503	                    {
   504	                        "type": "compute",
   505	                        "name": "nova",
   506	                        "endpoints": [
   507	                            { "url": "https://nova:8774/v2.1", "interface": "internal", "region_id": "RegionOne" },
   508	                            { "url": "https://nova-pub:8774/v2.1", "interface": "public", "region_id": "RegionOne" }
   509	                        ]
   510	                    },
   511	                    {
   512	                        "type": "identity",
   513	                        "name": "keystone",
   514	                        "endpoints": [
   515	                            { "url": "https://keystone:5000/v3", "interface": "public", "region_id": "RegionOne" }
   516	                        ]
   517	                    }
   518	                ]
   519	            }
   520	        }"#
   521	    }
   522	
   523	    #[test]
   524	    fn test_build_auth_body_password() {
   525	        let cred = sample_credential_password();
   526	        let body = KeystoneAuthAdapter::build_auth_body(&cred);
   527	
   528	        assert_eq!(body["auth"]["identity"]["methods"][0], "password");
   529	        assert_eq!(body["auth"]["identity"]["password"]["user"]["name"], "admin");
   530	        assert_eq!(
   531	            body["auth"]["identity"]["password"]["user"]["domain"]["name"],
   532	            "Default"
   533	        );
   534	        assert_eq!(body["auth"]["scope"]["project"]["name"], "admin-project");
   535	    }
   536	
   537	    #[test]
   538	    fn test_build_auth_body_app_credential() {
   539	        let cred = sample_credential_app();
   540	        let body = KeystoneAuthAdapter::build_auth_body(&cred);
   541	
   542	        assert_eq!(
   543	            body["auth"]["identity"]["methods"][0],
   544	            "application_credential"
   545	        );
   546	        assert_eq!(
   547	            body["auth"]["identity"]["application_credential"]["id"],
   548	            "app-cred-id"
   549	        );
   550	        assert!(body["auth"]["scope"].is_null());
   551	    }
   552	
   553	    #[test]
   554	    fn test_parse_token_from_keystone_response() {
   555	        let json_str = sample_keystone_response_json();
   556	        let resp: KeystoneTokenResponse = serde_json::from_str(json_str).unwrap();
   557	        let token = parse_token("tok-abc-123".to_string(), resp);
   558	
   559	        assert_eq!(token.id, "tok-abc-123");
   560	        assert_eq!(token.project.name, "admin-project");
   561	        assert_eq!(token.project.domain_name, "Default");
   562	        assert_eq!(token.roles.len(), 2);
   563	        assert_eq!(token.roles[0].name, "admin");
   564	        assert_eq!(token.catalog.len(), 2);
   565	        assert_eq!(token.catalog[0].service_type, "compute");
   566	        assert_eq!(token.catalog[0].endpoints.len(), 2);
   567	        assert_eq!(
   568	            token.catalog[0].endpoints[0].interface,
   569	            EndpointInterface::Internal
   570	        );
   571	    }
   572	
   573	    #[test]
   574	    fn test_parse_token_no_catalog() {
   575	        let json_str = r#"{
   576	            "token": {
   577	                "expires_at": "2099-12-31T23:59:59.000000Z",
   578	                "roles": [{ "id": "r1", "name": "member" }]
   579	            }
   580	        }"#;
   581	        let resp: KeystoneTokenResponse = serde_json::from_str(json_str).unwrap();
   582	        let token = parse_token("tok-1".to_string(), resp);
   583	
   584	        assert!(token.catalog.is_empty());
   585	        assert_eq!(token.roles.len(), 1);
   586	        assert!(token.project.id.is_empty());
   587	    }
   588	
   589	    #[test]
   590	    fn test_parse_interface() {
   591	        assert_eq!(parse_interface("internal"), EndpointInterface::Internal);
   592	        assert_eq!(parse_interface("admin"), EndpointInterface::Admin);
   593	        assert_eq!(parse_interface("public"), EndpointInterface::Public);
   594	        assert_eq!(parse_interface("unknown"), EndpointInterface::Public);
   595	    }
   596	
   597	    #[tokio::test]
   598	    async fn test_get_endpoint_from_token() {
   599	        let adapter = KeystoneAuthAdapter::new(sample_credential_password()).unwrap();
   600	        let resp: KeystoneTokenResponse =
   601	            serde_json::from_str(sample_keystone_response_json()).unwrap();
   602	        let token = parse_token("tok-1".to_string(), resp);
   603	        {
   604	            let mut map = adapter.token_map.write().await;
   605	            map.insert(adapter.active_scope.clone(), token);
   606	        }
   607	
   608	        let url = adapter
   609	            .get_endpoint("compute", EndpointInterface::Internal, Some("RegionOne"))
   610	            .await
   611	            .unwrap();
   612	        assert_eq!(url, "https://nova:8774/v2.1");
   613	
   614	        let url = adapter
   615	            .get_endpoint("compute", EndpointInterface::Public, Some("RegionOne"))
   616	            .await
   617	            .unwrap();
   618	        assert_eq!(url, "https://nova-pub:8774/v2.1");
   619	
   620	        let err = adapter
   621	            .get_endpoint("image", EndpointInterface::Public, None)
   622	            .await;
   623	        assert!(err.is_err());
   624	    }
   625	
   626	    #[tokio::test]
   627	    async fn test_has_role() {
   628	        let adapter = KeystoneAuthAdapter::new(sample_credential_password()).unwrap();
   629	        let resp: KeystoneTokenResponse =
   630	            serde_json::from_str(sample_keystone_response_json()).unwrap();
   631	        let token = parse_token("tok-1".to_string(), resp);
   632	        {
   633	            let mut map = adapter.token_map.write().await;
   634	            map.insert(adapter.active_scope.clone(), token);
   635	        }
   636	
   637	        assert!(adapter.has_role("admin").await.unwrap());
   638	        assert!(adapter.has_role("member").await.unwrap());
   639	        assert!(!adapter.has_role("reader").await.unwrap());
   640	    }
   641	
   642	    #[tokio::test]
   643	    async fn test_authenticate_request_injects_token() {
   644	        let adapter = KeystoneAuthAdapter::new(sample_credential_password()).unwrap();
   645	        let resp: KeystoneTokenResponse =
   646	            serde_json::from_str(sample_keystone_response_json()).unwrap();
   647	        let token = parse_token("tok-xyz".to_string(), resp);
   648	        {
   649	            let mut map = adapter.token_map.write().await;
   650	            map.insert(adapter.active_scope.clone(), token);
   651	        }
   652	
   653	        let headers = reqwest::header::HeaderMap::new();
   654	        let auth = adapter
   655	            .authenticate_request("GET", "https://nova:8774/v2.1/servers", &headers, None)
   656	            .await
   657	            .unwrap();
   658	
   659	        assert_eq!(auth.headers.len(), 1);
   660	        assert_eq!(auth.headers[0].0, "X-Auth-Token");
   661	        assert_eq!(auth.headers[0].1, "tok-xyz");
   662	    }
   663	
   664	    #[tokio::test]
   665	    async fn test_get_token_info_not_authenticated() {
   666	        let adapter = KeystoneAuthAdapter::new(sample_credential_password()).unwrap();
   667	        let err = adapter.get_token_info().await;
   668	        assert!(err.is_err());
   669	    }
   670	
   671	    #[tokio::test]
   672	    async fn test_get_catalog() {
   673	        let adapter = KeystoneAuthAdapter::new(sample_credential_password()).unwrap();
   674	        let resp: KeystoneTokenResponse =
   675	            serde_json::from_str(sample_keystone_response_json()).unwrap();
   676	        let token = parse_token("tok-1".to_string(), resp);
   677	        {
   678	            let mut map = adapter.token_map.write().await;
   679	            map.insert(adapter.active_scope.clone(), token);
   680	        }
   681	
   682	        let catalog = adapter.get_catalog().await.unwrap();
   683	        assert_eq!(catalog.len(), 2);
   684	        assert_eq!(catalog[0].service_type, "compute");
   685	    }
   686	
   687	    #[tokio::test]
   688	    async fn test_refresh_loop_idempotent() {
   689	        let adapter = KeystoneAuthAdapter::new(sample_credential_password()).unwrap();
   690	        assert!(!adapter.refresh_started.load(Ordering::SeqCst));
   691	
   692	        // Simulate first start
   693	        adapter.refresh_started.store(true, Ordering::SeqCst);
   694	        assert!(adapter.refresh_started.load(Ordering::SeqCst));
   695	
   696	        // Second call should be no-op (tested via AtomicBool flag)
   697	        let was_started = adapter.refresh_started.swap(true, Ordering::SeqCst);
   698	        assert!(was_started); // was already true
   699	    }
   700	}

codex
I’m now checking the live code contracts (`Action/AppEvent`, worker flow, endpoint cache invalidation, error types) to validate whether the proposed DETAIL signatures are implementable without layering breaks.
exec
/bin/zsh -lc "nl -ba src/adapter/http/base.rs | sed -n '1,260p'" in /Users/jay.ahn/projects/infra/nexttui/.worktrees/runtime-context-switch
 succeeded in 0ms:
     1	use std::sync::Arc;
     2	use std::time::Duration;
     3	
     4	use reqwest::{Method, RequestBuilder, Response};
     5	use serde::de::DeserializeOwned;
     6	use tokio::sync::RwLock;
     7	
     8	use crate::port::auth::AuthProvider;
     9	use crate::port::error::{ApiError, ApiResult};
    10	use crate::port::types::EndpointInterface;
    11	
    12	/// Shared HTTP plumbing for all service adapters.
    13	/// Auth injection is delegated to AuthProvider::authenticate_request().
    14	///
    15	/// Endpoint caching note: cached endpoint is resolved once and reused.
    16	/// Callers should call `invalidate_endpoint()` when token refresh occurs
    17	/// to pick up potential catalog changes. In Phase 2, BaseHttpClient will
    18	/// subscribe to token refresh broadcast to automate this.
    19	pub struct BaseHttpClient {
    20	    client: reqwest::Client,
    21	    auth: Arc<dyn AuthProvider>,
    22	    service_type: String,
    23	    interface: EndpointInterface,
    24	    region: Option<String>,
    25	    endpoint: RwLock<Option<String>>,
    26	}
    27	
    28	impl BaseHttpClient {
    29	    pub fn new(
    30	        auth: Arc<dyn AuthProvider>,
    31	        service_type: &str,
    32	        interface: EndpointInterface,
    33	        region: Option<String>,
    34	    ) -> Result<Self, ApiError> {
    35	        Ok(Self {
    36	            client: reqwest::Client::builder()
    37	                .timeout(Duration::from_secs(30))
    38	                .connect_timeout(Duration::from_secs(10))
    39	                .build()?,
    40	            auth,
    41	            service_type: service_type.to_string(),
    42	            interface,
    43	            region,
    44	            endpoint: RwLock::new(None),
    45	        })
    46	    }
    47	
    48	    /// Resolve and cache the endpoint from service catalog.
    49	    #[tracing::instrument(skip(self), fields(service = %self.service_type))]
    50	    async fn resolve_endpoint(&self) -> ApiResult<String> {
    51	        {
    52	            let cached = self.endpoint.read().await;
    53	            if let Some(url) = cached.as_ref() {
    54	                return Ok(url.clone());
    55	            }
    56	        }
    57	        let url = self
    58	            .auth
    59	            .get_endpoint(&self.service_type, self.interface.clone(), self.region.as_deref())
    60	            .await?;
    61	        let mut cached = self.endpoint.write().await;
    62	        *cached = Some(url.clone());
    63	        Ok(url)
    64	    }
    65	
    66	    /// Invalidate cached endpoint. Should be called on token refresh
    67	    /// to pick up potential service catalog changes.
    68	    pub async fn invalidate_endpoint(&self) {
    69	        let mut cached = self.endpoint.write().await;
    70	        *cached = None;
    71	    }
    72	
    73	    /// Build an authenticated request.
    74	    /// Note (Phase 2): For signed auth methods (HMAC), authenticate_request()
    75	    /// will need actual headers/body. Currently passes empty values since
    76	    /// Phase 1 only uses X-Auth-Token which doesn't depend on request content.
    77	    async fn request(&self, method: Method, path: &str) -> ApiResult<RequestBuilder> {
    78	        let endpoint = self.resolve_endpoint().await?;
    79	        let url = format!("{}{}", endpoint.trim_end_matches('/'), path);
    80	        let method_str = method.as_str();
    81	        let empty_headers = reqwest::header::HeaderMap::new();
    82	        let auth_headers = self
    83	            .auth
    84	            .authenticate_request(method_str, &url, &empty_headers, None)
    85	            .await?;
    86	        let mut builder = self
    87	            .client
    88	            .request(method, &url)
    89	            .header("Content-Type", "application/json");
    90	        for (key, value) in &auth_headers.headers {
    91	            builder = builder.header(key.as_str(), value.as_str());
    92	        }
    93	        Ok(builder)
    94	    }
    95	
    96	    pub async fn get(&self, path: &str) -> ApiResult<RequestBuilder> {
    97	        self.request(Method::GET, path).await
    98	    }
    99	
   100	    pub async fn post(&self, path: &str) -> ApiResult<RequestBuilder> {
   101	        self.request(Method::POST, path).await
   102	    }
   103	
   104	    pub async fn put(&self, path: &str) -> ApiResult<RequestBuilder> {
   105	        self.request(Method::PUT, path).await
   106	    }
   107	
   108	    pub async fn patch(&self, path: &str) -> ApiResult<RequestBuilder> {
   109	        self.request(Method::PATCH, path).await
   110	    }
   111	
   112	    pub async fn delete(&self, path: &str) -> ApiResult<RequestBuilder> {
   113	        self.request(Method::DELETE, path).await
   114	    }
   115	
   116	    /// Send a request and map HTTP errors to ApiError.
   117	    pub async fn send(&self, request: RequestBuilder) -> ApiResult<Response> {
   118	        let resp = request.send().await.map_err(ApiError::Network)?;
   119	        Self::check_status(resp).await
   120	    }
   121	
   122	    /// Send + deserialize JSON body.
   123	    pub async fn send_json<T: DeserializeOwned>(
   124	        &self,
   125	        request: RequestBuilder,
   126	    ) -> ApiResult<T> {
   127	        let resp = self.send(request).await?;
   128	        resp.json::<T>()
   129	            .await
   130	            .map_err(|e| ApiError::Parse(format!("JSON deserialization failed: {e}")))
   131	    }
   132	
   133	    /// Send and expect 204 No Content (or 202 Accepted).
   134	    pub async fn send_no_content(&self, request: RequestBuilder) -> ApiResult<()> {
   135	        self.send(request).await?;
   136	        Ok(())
   137	    }
   138	
   139	    /// Extract a human-readable error message from OpenStack JSON error bodies.
   140	    /// Handles formats: {"NeutronError": {"message": "..."}}, {"badRequest": {"message": "..."}},
   141	    /// {"error": {"message": "..."}}, {"itemNotFound": {"message": "..."}}, etc.
   142	    fn extract_error_message(body: &str) -> String {
   143	        if let Ok(json) = serde_json::from_str::<serde_json::Value>(body) {
   144	            // Try common OpenStack error wrapper keys
   145	            for key in &["NeutronError", "badRequest", "itemNotFound", "conflictingRequest",
   146	                         "forbidden", "error", "computeFault"] {
   147	                if let Some(inner) = json.get(key) {
   148	                    if let Some(msg) = inner.get("message").and_then(|m| m.as_str()) {
   149	                        return msg.to_string();
   150	                    }
   151	                }
   152	            }
   153	            // Fallback: try any top-level object with a "message" field
   154	            if let Some(obj) = json.as_object() {
   155	                for (_k, v) in obj {
   156	                    if let Some(msg) = v.get("message").and_then(|m| m.as_str()) {
   157	                        return msg.to_string();
   158	                    }
   159	                }
   160	            }
   161	        }
   162	        // Not JSON or no message found — return raw body
   163	        body.to_string()
   164	    }
   165	
   166	    /// Map HTTP status codes to ApiError.
   167	    pub(crate) async fn check_status(resp: Response) -> ApiResult<Response> {
   168	        let status = resp.status();
   169	        if status.is_success() {
   170	            return Ok(resp);
   171	        }
   172	        let raw_body = resp.text().await.unwrap_or_default();
   173	        let body = Self::extract_error_message(&raw_body);
   174	        match status.as_u16() {
   175	            401 => Err(ApiError::TokenExpired),
   176	            403 => Err(ApiError::Forbidden(body)),
   177	            404 => Err(ApiError::NotFound {
   178	                resource_type: String::new(),
   179	                id: body,
   180	            }),
   181	            409 => Err(ApiError::Conflict(body)),
   182	            400 => Err(ApiError::BadRequest(body)),
   183	            429 => Err(ApiError::RateLimited {
   184	                retry_after_secs: 60,
   185	            }),
   186	            503 => Err(ApiError::ServiceUnavailable {
   187	                service: String::new(),
   188	            }),
   189	            _ => Err(ApiError::Unexpected {
   190	                status: status.as_u16(),
   191	                body,
   192	            }),
   193	        }
   194	    }
   195	}
   196	
   197	#[cfg(test)]
   198	mod tests {
   199	    use super::*;
   200	    use ::http::StatusCode;
   201	
   202	    fn mock_response(status: u16, body: &str) -> Response {
   203	        ::http::Response::builder()
   204	            .status(StatusCode::from_u16(status).unwrap())
   205	            .body(body.to_string())
   206	            .unwrap()
   207	            .into()
   208	    }
   209	
   210	    #[tokio::test]
   211	    async fn test_check_status_success() {
   212	        let resp = mock_response(200, r#"{"ok": true}"#);
   213	        let result = BaseHttpClient::check_status(resp).await;
   214	        assert!(result.is_ok());
   215	    }
   216	
   217	    #[tokio::test]
   218	    async fn test_check_status_401() {
   219	        let resp = mock_response(401, "unauthorized");
   220	        let err = BaseHttpClient::check_status(resp).await.unwrap_err();
   221	        assert!(matches!(err, ApiError::TokenExpired));
   222	    }
   223	
   224	    #[tokio::test]
   225	    async fn test_check_status_403() {
   226	        let resp = mock_response(403, "forbidden");
   227	        let err = BaseHttpClient::check_status(resp).await.unwrap_err();
   228	        assert!(matches!(err, ApiError::Forbidden(_)));
   229	    }
   230	
   231	    #[tokio::test]
   232	    async fn test_check_status_404() {
   233	        let resp = mock_response(404, "not found");
   234	        let err = BaseHttpClient::check_status(resp).await.unwrap_err();
   235	        assert!(matches!(err, ApiError::NotFound { .. }));
   236	    }
   237	
   238	    #[tokio::test]
   239	    async fn test_check_status_409() {
   240	        let resp = mock_response(409, "conflict");
   241	        let err = BaseHttpClient::check_status(resp).await.unwrap_err();
   242	        assert!(matches!(err, ApiError::Conflict(_)));
   243	    }
   244	
   245	    #[tokio::test]
   246	    async fn test_check_status_400() {
   247	        let resp = mock_response(400, "bad request");
   248	        let err = BaseHttpClient::check_status(resp).await.unwrap_err();
   249	        assert!(matches!(err, ApiError::BadRequest(_)));
   250	    }
   251	
   252	    #[tokio::test]
   253	    async fn test_check_status_429() {
   254	        let resp = mock_response(429, "rate limited");
   255	        let err = BaseHttpClient::check_status(resp).await.unwrap_err();
   256	        assert!(matches!(err, ApiError::RateLimited { .. }));
   257	    }
   258	
   259	    #[tokio::test]
   260	    async fn test_check_status_503() {

exec
/bin/zsh -lc "nl -ba src/component.rs | sed -n '1,260p'" in /Users/jay.ahn/projects/infra/nexttui/.worktrees/runtime-context-switch
 succeeded in 0ms:
     1	use crossterm::event::KeyEvent;
     2	use ratatui::Frame;
     3	use ratatui::layout::Rect;
     4	
     5	use crate::action::Action;
     6	use crate::event::AppEvent;
     7	
     8	#[derive(Debug, Clone, Copy, PartialEq, Eq)]
     9	pub enum LayoutHint {
    10	    Default,
    11	    FullWidth,
    12	}
    13	
    14	pub trait Component {
    15	    fn handle_key(&mut self, key: KeyEvent) -> Option<Action>;
    16	    fn handle_event(&mut self, event: &AppEvent);
    17	    fn render(&self, frame: &mut Frame, area: Rect);
    18	    fn set_admin(&mut self, _is_admin: bool) {}
    19	    fn set_all_tenants(&mut self, _all_tenants: bool) {}
    20	    fn help_hint(&self) -> &str { "" }
    21	    fn refresh_action(&self) -> Option<Action> { None }
    22	    fn has_transitional_resources(&self) -> bool { false }
    23	    fn is_modal(&self) -> bool { false }
    24	    fn layout_hint(&self) -> LayoutHint { LayoutHint::Default }
    25	    fn is_busy(&self) -> bool { false }
    26	    /// Dynamic content title based on view state (e.g. "Server: web-01").
    27	    /// Returns None to use the default route label.
    28	    fn content_title(&self) -> Option<String> { None }
    29	}
    30	
    31	#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
    32	pub enum InputMode {
    33	    #[default]
    34	    Normal,
    35	    Command,
    36	    Search,
    37	    Form,
    38	    Confirm,
    39	}
    40	
    41	#[cfg(test)]
    42	mod tests {
    43	    use super::*;
    44	
    45	    #[test]
    46	    fn test_input_mode_default() {
    47	        let mode = InputMode::default();
    48	        assert_eq!(mode, InputMode::Normal);
    49	    }
    50	
    51	    #[test]
    52	    fn test_component_default_refresh_action_is_none() {
    53	        struct Dummy;
    54	        impl Component for Dummy {
    55	            fn handle_key(&mut self, _key: KeyEvent) -> Option<Action> { None }
    56	            fn handle_event(&mut self, _event: &AppEvent) {}
    57	            fn render(&self, _frame: &mut Frame, _area: Rect) {}
    58	        }
    59	        let d = Dummy;
    60	        assert!(d.refresh_action().is_none());
    61	    }
    62	
    63	    #[test]
    64	    fn test_component_default_has_transitional_is_false() {
    65	        struct Dummy;
    66	        impl Component for Dummy {
    67	            fn handle_key(&mut self, _key: KeyEvent) -> Option<Action> { None }
    68	            fn handle_event(&mut self, _event: &AppEvent) {}
    69	            fn render(&self, _frame: &mut Frame, _area: Rect) {}
    70	        }
    71	        let d = Dummy;
    72	        assert!(!d.has_transitional_resources());
    73	    }
    74	
    75	    #[test]
    76	    fn test_component_default_is_modal_is_false() {
    77	        struct Dummy;
    78	        impl Component for Dummy {
    79	            fn handle_key(&mut self, _key: KeyEvent) -> Option<Action> { None }
    80	            fn handle_event(&mut self, _event: &AppEvent) {}
    81	            fn render(&self, _frame: &mut Frame, _area: Rect) {}
    82	        }
    83	        let d = Dummy;
    84	        assert!(!d.is_modal());
    85	    }
    86	
    87	    #[test]
    88	    fn test_component_default_layout_hint_is_default() {
    89	        struct Dummy;
    90	        impl Component for Dummy {
    91	            fn handle_key(&mut self, _key: KeyEvent) -> Option<Action> { None }
    92	            fn handle_event(&mut self, _event: &AppEvent) {}
    93	            fn render(&self, _frame: &mut Frame, _area: Rect) {}
    94	        }
    95	        let d = Dummy;
    96	        assert_eq!(d.layout_hint(), LayoutHint::Default);
    97	        assert!(!d.is_busy());
    98	    }
    99	
   100	    #[test]
   101	    fn test_component_set_admin_default() {
   102	        use crossterm::event::KeyEvent;
   103	        use ratatui::layout::Rect;
   104	        use ratatui::Frame;
   105	        use crate::action::Action;
   106	        use crate::event::AppEvent;
   107	
   108	        struct Dummy;
   109	        impl Component for Dummy {
   110	            fn handle_key(&mut self, _key: KeyEvent) -> Option<Action> { None }
   111	            fn handle_event(&mut self, _event: &AppEvent) {}
   112	            fn render(&self, _frame: &mut Frame, _area: Rect) {}
   113	        }
   114	
   115	        let mut d = Dummy;
   116	        // Should not panic — default no-op
   117	        d.set_admin(true);
   118	        d.set_admin(false);
   119	    }
   120	}

exec
/bin/zsh -lc "nl -ba src/action.rs | sed -n '1,320p'" in /Users/jay.ahn/projects/infra/nexttui/.worktrees/runtime-context-switch
 succeeded in 0ms:
     1	use crate::models::common::Route;
     2	use crate::port::types::EvacuateParams;
     3	
     4	#[derive(Debug, Clone)]
     5	pub enum Action {
     6	    // Navigation
     7	    Navigate(Route),
     8	    Back,
     9	
    10	    // Nova
    11	    FetchServers,
    12	    CreateServer(crate::port::types::ServerCreateParams),
    13	    DeleteServer { id: String, name: String },
    14	    RebootServer { id: String, hard: bool },
    15	    StartServer { id: String },
    16	    StopServer { id: String },
    17	    CreateServerSnapshot { server_id: String, name: String },
    18	    FetchFlavors,
    19	    CreateFlavor(crate::port::types::FlavorCreateParams),
    20	    DeleteFlavor { id: String },
    21	    FetchAggregates,
    22	    FetchComputeServices,
    23	    FetchHypervisors,
    24	
    25	    // Neutron
    26	    FetchNetworks,
    27	    CreateNetwork(crate::port::types::NetworkCreateParams),
    28	    FetchSecurityGroups,
    29	    CreateSecurityGroup(crate::port::types::SecurityGroupCreateParams),
    30	    DeleteSecurityGroup { id: String },
    31	    CreateSecurityGroupRule(crate::port::types::SecurityGroupRuleCreateParams),
    32	    DeleteSecurityGroupRule { rule_id: String },
    33	    FetchFloatingIps,
    34	    CreateFloatingIp { network_id: String },
    35	    DeleteFloatingIp { id: String },
    36	    FetchSubnets { network_id: String },
    37	    FetchAgents,
    38	
    39	    // Cinder
    40	    FetchVolumes,
    41	    CreateVolume(crate::port::types::VolumeCreateParams),
    42	    DeleteVolume { id: String, force: bool },
    43	    ExtendVolume { id: String, new_size: u32 },
    44	    FetchSnapshots,
    45	    CreateSnapshot(crate::port::types::SnapshotCreateParams),
    46	    DeleteSnapshot { id: String },
    47	
    48	    // Glance
    49	    FetchImages,
    50	    CreateImage(crate::port::types::ImageCreateParams),
    51	    DeleteImage { id: String },
    52	
    53	    // Keystone Admin
    54	    FetchProjects,
    55	    CreateProject(crate::port::types::ProjectCreateParams),
    56	    DeleteProject { id: String },
    57	    FetchUsers,
    58	    CreateUser(crate::port::types::UserCreateParams),
    59	    DeleteUser { id: String },
    60	
    61	    // Usage
    62	    FetchUsage { start: String, end: String },
    63	
    64	    // UI
    65	    FocusSidebar,
    66	    EnterFormMode,
    67	    ExitFormMode,
    68	    SelectResource { id: String },
    69	    NavigateToResource { route: Route, id: String },
    70	
    71	    // Resize
    72	    ResizeServer { id: String, flavor_id: String },
    73	    ConfirmResize { id: String },
    74	    RevertResize { id: String },
    75	
    76	    // Migration / Evacuate
    77	    LiveMigrateServer { id: String, host: Option<String> },
    78	    ColdMigrateServer { id: String },
    79	    ConfirmMigration { id: String },
    80	    RevertMigration { id: String },
    81	    EvacuateServer { id: String, params: EvacuateParams },
    82	    DisableComputeService { service_id: String, hostname: String },
    83	    EnableComputeService { service_id: String, hostname: String },
    84	    FetchMigrationProgress { server_id: String },
    85	
    86	    // Volume Attach/Detach
    87	    AttachVolume { volume_id: String, server_id: String, device: Option<String> },
    88	    DetachVolume { volume_id: String, server_id: String, attachment_id: String },
    89	    ForceDetachVolume { volume_id: String, server_id: String, attachment_id: String },
    90	    ForceResetVolumeState { volume_id: String, target_state: String },
    91	
    92	    // Floating IP Associate/Disassociate
    93	    AssociateFloatingIp { fip_id: String, port_id: String },
    94	    DisassociateFloatingIp { fip_id: String },
    95	
    96	    // Ports
    97	    FetchPorts { server_id: String },
    98	
    99	    // All Tenants
   100	    ToggleAllTenants,
   101	
   102	    // Toast (module-initiated hints)
   103	    ShowToast { message: String },
   104	
   105	    // System
   106	    RefreshAll,
   107	    SwitchCloud(String),
   108	    Quit,
   109	}
   110	
   111	#[cfg(test)]
   112	mod tests {
   113	    use super::*;
   114	
   115	    #[test]
   116	    fn test_action_variants_exist() {
   117	        use crate::port::types::{FlavorCreateParams, ServerCreateParams};
   118	
   119	        let actions: Vec<Action> = vec![
   120	            Action::Navigate(Route::Servers),
   121	            Action::Back,
   122	            Action::FetchServers,
   123	            Action::CreateServer(ServerCreateParams {
   124	                name: "test".into(),
   125	                image_id: "img-1".into(),
   126	                flavor_id: "flv-1".into(),
   127	                networks: vec![],
   128	                security_groups: None,
   129	                key_name: None,
   130	                availability_zone: None,
   131	            }),
   132	            Action::DeleteServer {
   133	                id: "s1".into(),
   134	                name: "web".into(),
   135	            },
   136	            Action::RebootServer {
   137	                id: "s1".into(),
   138	                hard: false,
   139	            },
   140	            Action::StartServer { id: "s1".into() },
   141	            Action::StopServer { id: "s1".into() },
   142	            Action::CreateServerSnapshot {
   143	                server_id: "s1".into(),
   144	                name: "snap".into(),
   145	            },
   146	            Action::FetchFlavors,
   147	            Action::CreateFlavor(FlavorCreateParams {
   148	                name: "m1.test".into(),
   149	                vcpus: 1,
   150	                ram_mb: 512,
   151	                disk_gb: 10,
   152	                is_public: true,
   153	            }),
   154	            Action::DeleteFlavor { id: "f1".into() },
   155	            Action::FetchNetworks,
   156	            Action::FetchVolumes,
   157	            Action::FetchImages,
   158	            Action::FetchProjects,
   159	            Action::RefreshAll,
   160	            Action::SwitchCloud("prod".into()),
   161	            Action::Quit,
   162	        ];
   163	        assert!(actions.len() >= 18);
   164	    }
   165	
   166	    #[test]
   167	    fn test_volume_fip_action_variants_exist() {
   168	        let actions: Vec<Action> = vec![
   169	            Action::AttachVolume { volume_id: "v1".into(), server_id: "s1".into(), device: Some("/dev/vdb".into()) },
   170	            Action::DetachVolume { volume_id: "v1".into(), server_id: "s1".into(), attachment_id: "att-1".into() },
   171	            Action::ForceDetachVolume { volume_id: "v1".into(), server_id: "s1".into(), attachment_id: "att-1".into() },
   172	            Action::ForceResetVolumeState { volume_id: "v1".into(), target_state: "available".into() },
   173	            Action::AssociateFloatingIp { fip_id: "fip-1".into(), port_id: "port-1".into() },
   174	            Action::DisassociateFloatingIp { fip_id: "fip-1".into() },
   175	            Action::FetchPorts { server_id: "s1".into() },
   176	        ];
   177	        assert_eq!(actions.len(), 7);
   178	    }
   179	
   180	    #[test]
   181	    fn test_resize_action_variants_exist() {
   182	        let actions: Vec<Action> = vec![
   183	            Action::ResizeServer { id: "s1".into(), flavor_id: "f2".into() },
   184	            Action::ConfirmResize { id: "s1".into() },
   185	            Action::RevertResize { id: "s1".into() },
   186	        ];
   187	        assert_eq!(actions.len(), 3);
   188	    }
   189	
   190	    #[test]
   191	    fn test_usage_action_variant_exists() {
   192	        let action = Action::FetchUsage {
   193	            start: "2026-01-01T00:00:00Z".into(),
   194	            end: "2026-01-31T23:59:59Z".into(),
   195	        };
   196	        match action {
   197	            Action::FetchUsage { start, end } => {
   198	                assert!(start.contains("2026"));
   199	                assert!(end.contains("2026"));
   200	            }
   201	            _ => panic!("expected FetchUsage"),
   202	        }
   203	    }
   204	
   205	    #[test]
   206	    fn test_migration_action_variants_exist() {
   207	        let actions: Vec<Action> = vec![
   208	            Action::LiveMigrateServer { id: "s1".into(), host: None },
   209	            Action::ColdMigrateServer { id: "s1".into() },
   210	            Action::ConfirmMigration { id: "s1".into() },
   211	            Action::RevertMigration { id: "s1".into() },
   212	            Action::EvacuateServer { id: "s1".into(), params: EvacuateParams { host: Some("compute-02".into()), ..Default::default() } },
   213	            Action::FetchMigrationProgress { server_id: "s1".into() },
   214	            Action::DisableComputeService { service_id: "svc-1".into(), hostname: "compute-01".into() },
   215	            Action::EnableComputeService { service_id: "svc-1".into(), hostname: "compute-01".into() },
   216	        ];
   217	        assert_eq!(actions.len(), 8);
   218	    }
   219	}

exec
/bin/zsh -lc "nl -ba src/event.rs | sed -n '1,320p'" in /Users/jay.ahn/projects/infra/nexttui/.worktrees/runtime-context-switch
 succeeded in 0ms:
     1	use crate::models::{
     2	    cinder::{Volume, VolumeSnapshot},
     3	    glance::Image,
     4	    keystone::{Project, User},
     5	    neutron::{FloatingIp, Network, NetworkAgent, Port, SecurityGroup},
     6	    nova::{Aggregate, ComputeService, Flavor, Hypervisor, Server, ServerMigration},
     7	};
     8	use crate::port::types::TenantUsage;
     9	
    10	#[derive(Debug)]
    11	pub enum AppEvent {
    12	    // Data loaded
    13	    ServersLoaded(Vec<Server>),
    14	    FlavorsLoaded(Vec<Flavor>),
    15	    NetworksLoaded(Vec<Network>),
    16	    SecurityGroupsLoaded(Vec<SecurityGroup>),
    17	    FloatingIpsLoaded(Vec<FloatingIp>),
    18	    VolumesLoaded(Vec<Volume>),
    19	    SnapshotsLoaded(Vec<VolumeSnapshot>),
    20	    ImagesLoaded(Vec<Image>),
    21	    ProjectsLoaded(Vec<Project>),
    22	    UsersLoaded(Vec<User>),
    23	    AggregatesLoaded(Vec<Aggregate>),
    24	    ComputeServicesLoaded(Vec<ComputeService>),
    25	    HypervisorsLoaded(Vec<Hypervisor>),
    26	    AgentsLoaded(Vec<NetworkAgent>),
    27	
    28	    // Usage
    29	    UsageLoaded(Vec<TenantUsage>),
    30	
    31	    // CUD results
    32	    ServerCreated(Server),
    33	    ServerDeleted { id: String, name: String },
    34	    ServerRebooted { id: String },
    35	    ServerStarted { id: String },
    36	    ServerStopped { id: String },
    37	    ServerSnapshotCreated { server_id: String, image_id: String },
    38	    FlavorCreated(Flavor),
    39	    FlavorDeleted { id: String },
    40	    NetworkCreated(Network),
    41	    SubnetsLoaded { network_id: String, subnets: Vec<crate::port::types::Subnet> },
    42	    SecurityGroupCreated(SecurityGroup),
    43	    SecurityGroupDeleted { id: String },
    44	    SecurityGroupRuleCreated(crate::models::neutron::SecurityGroupRule),
    45	    SecurityGroupRuleDeleted { rule_id: String },
    46	    VolumeCreated(Volume),
    47	    VolumeDeleted { id: String },
    48	    VolumeExtended { id: String },
    49	    SnapshotCreated(VolumeSnapshot),
    50	    SnapshotDeleted { id: String },
    51	    ImageCreated(Image),
    52	    ImageDeleted { id: String },
    53	    FloatingIpCreated(FloatingIp),
    54	    FloatingIpDeleted { id: String },
    55	
    56	    // Keystone CUD
    57	    ProjectCreated(Project),
    58	    ProjectDeleted { id: String },
    59	    UserCreated(User),
    60	    UserDeleted { id: String },
    61	
    62	    // Resize results
    63	    ServerResized { id: String },
    64	    ResizeConfirmed { id: String },
    65	    ResizeReverted { id: String },
    66	
    67	    // Migration results
    68	    ServerLiveMigrated { id: String },
    69	    ServerColdMigrated { id: String },
    70	    MigrationConfirmed { id: String },
    71	    MigrationReverted { id: String },
    72	    ServerEvacuated { id: String },
    73	    ServerEvacuateResult { id: String, result: Result<(), String> },
    74	    ComputeServiceToggled { hostname: String, enabled: bool },
    75	    MigrationProgressLoaded { server_id: String, migration: ServerMigration },
    76	    MigrationPollingStopped { server_id: String },
    77	
    78	    // Volume Attach/Detach results
    79	    VolumeAttached { volume_id: String, server_id: String },
    80	    VolumeDetached { volume_id: String },
    81	    VolumeForceDetached { volume_id: String },
    82	    VolumeStateReset { volume_id: String },
    83	
    84	    // Floating IP Associate/Disassociate results
    85	    FloatingIpAssociated(FloatingIp),
    86	    FloatingIpDisassociated(FloatingIp),
    87	
    88	    // Ports
    89	    PortsLoaded { server_id: String, ports: Vec<Port> },
    90	
    91	    // Server status polling (resize / cold-migrate state transitions)
    92	    ServerStatusPolled { server: Server },
    93	
    94	    // Error
    95	    ApiError { operation: String, message: String },
    96	
    97	    // Auth
    98	    TokenRefreshed(Vec<crate::port::types::TokenRole>),
    99	    AuthFailed(String),
   100	
   101	    // RBAC
   102	    PermissionDenied { operation: String },
   103	
   104	    // System
   105	    CloudSwitched(String),
   106	}
   107	
   108	#[cfg(test)]
   109	mod tests {
   110	    use super::*;
   111	
   112	    #[test]
   113	    fn test_app_event_variants_exist() {
   114	        let events: Vec<AppEvent> = vec![
   115	            AppEvent::ServersLoaded(vec![]),
   116	            AppEvent::FlavorsLoaded(vec![]),
   117	            AppEvent::NetworksLoaded(vec![]),
   118	            AppEvent::VolumesLoaded(vec![]),
   119	            AppEvent::ImagesLoaded(vec![]),
   120	            AppEvent::ServerDeleted {
   121	                id: "s1".into(),
   122	                name: "web".into(),
   123	            },
   124	            AppEvent::ServerSnapshotCreated {
   125	                server_id: "s1".into(),
   126	                image_id: "img-1".into(),
   127	            },
   128	            AppEvent::FlavorDeleted { id: "f1".into() },
   129	            AppEvent::ApiError {
   130	                operation: "delete".into(),
   131	                message: "not found".into(),
   132	            },
   133	            AppEvent::TokenRefreshed(vec![]),
   134	            AppEvent::AuthFailed("expired".into()),
   135	            AppEvent::CloudSwitched("prod".into()),
   136	        ];
   137	        assert!(events.len() >= 12);
   138	    }
   139	
   140	    #[test]
   141	    fn test_token_refreshed_carries_roles() {
   142	        use crate::port::types::TokenRole;
   143	        let role = TokenRole { id: "r1".into(), name: "admin".into() };
   144	        let event = AppEvent::TokenRefreshed(vec![role]);
   145	        match event {
   146	            AppEvent::TokenRefreshed(roles) => {
   147	                assert_eq!(roles.len(), 1);
   148	                assert_eq!(roles[0].name, "admin");
   149	            }
   150	            _ => panic!("expected TokenRefreshed"),
   151	        }
   152	    }
   153	
   154	    #[test]
   155	    fn test_migration_event_variants_exist() {
   156	        use crate::models::nova::ServerMigration;
   157	        let events: Vec<AppEvent> = vec![
   158	            AppEvent::ServerLiveMigrated { id: "s1".into() },
   159	            AppEvent::ServerColdMigrated { id: "s1".into() },
   160	            AppEvent::MigrationConfirmed { id: "s1".into() },
   161	            AppEvent::MigrationReverted { id: "s1".into() },
   162	            AppEvent::ServerEvacuated { id: "s1".into() },
   163	            AppEvent::MigrationProgressLoaded {
   164	                server_id: "s1".into(),
   165	                migration: ServerMigration {
   166	                    id: 1,
   167	                    status: "running".into(),
   168	                    source_compute: "compute-01".into(),
   169	                    dest_compute: "compute-02".into(),
   170	                    memory_total_bytes: Some(1024),
   171	                    memory_processed_bytes: Some(512),
   172	                    memory_remaining_bytes: Some(512),
   173	                    disk_total_bytes: Some(4096),
   174	                    disk_processed_bytes: Some(2048),
   175	                    disk_remaining_bytes: Some(2048),
   176	                    created_at: None,
   177	                    updated_at: None,
   178	                },
   179	            },
   180	        ];
   181	        assert_eq!(events.len(), 6);
   182	    }
   183	
   184	    #[test]
   185	    fn test_resize_event_variants_exist() {
   186	        let events: Vec<AppEvent> = vec![
   187	            AppEvent::ServerResized { id: "s1".into() },
   188	            AppEvent::ResizeConfirmed { id: "s1".into() },
   189	            AppEvent::ResizeReverted { id: "s1".into() },
   190	        ];
   191	        assert_eq!(events.len(), 3);
   192	    }
   193	
   194	    #[test]
   195	    fn test_server_status_polled_event() {
   196	        use crate::models::nova::Server;
   197	        let server = Server {
   198	            id: "s1".into(),
   199	            name: "test".into(),
   200	            status: "VERIFY_RESIZE".into(),
   201	            addresses: Default::default(),
   202	            flavor: crate::models::nova::FlavorRef {
   203	                id: "f1".into(),
   204	                original_name: None,
   205	                vcpus: None,
   206	                ram: None,
   207	                disk: None,
   208	            },
   209	            image: None,
   210	            key_name: None,
   211	            availability_zone: None,
   212	            created: "2026-01-01".into(),
   213	            updated: None,
   214	            tenant_id: None,
   215	            host_id: None,
   216	            host: None,
   217	            volumes_attached: vec![],
   218	            security_groups: vec![],
   219	        };
   220	        let event = AppEvent::ServerStatusPolled { server };
   221	        match event {
   222	            AppEvent::ServerStatusPolled { server } => {
   223	                assert_eq!(server.status, "VERIFY_RESIZE");
   224	            }
   225	            _ => panic!("expected ServerStatusPolled"),
   226	        }
   227	    }
   228	
   229	    #[test]
   230	    fn test_volume_fip_event_variants_exist() {
   231	        use crate::models::neutron::{FloatingIp, Port, FixedIp};
   232	        let events: Vec<AppEvent> = vec![
   233	            AppEvent::VolumeAttached { volume_id: "v1".into(), server_id: "s1".into() },
   234	            AppEvent::VolumeDetached { volume_id: "v1".into() },
   235	            AppEvent::VolumeForceDetached { volume_id: "v1".into() },
   236	            AppEvent::VolumeStateReset { volume_id: "v1".into() },
   237	            AppEvent::FloatingIpAssociated(FloatingIp {
   238	                id: "fip-1".into(),
   239	                floating_ip_address: "203.0.113.10".into(),
   240	                status: "ACTIVE".into(),
   241	                port_id: Some("port-1".into()),
   242	                floating_network_id: "ext-1".into(),
   243	                fixed_ip_address: None,
   244	                router_id: None,
   245	                tenant_id: None,
   246	            }),
   247	            AppEvent::FloatingIpDisassociated(FloatingIp {
   248	                id: "fip-1".into(),
   249	                floating_ip_address: "203.0.113.10".into(),
   250	                status: "DOWN".into(),
   251	                port_id: None,
   252	                floating_network_id: "ext-1".into(),
   253	                fixed_ip_address: None,
   254	                router_id: None,
   255	                tenant_id: None,
   256	            }),
   257	            AppEvent::PortsLoaded {
   258	                server_id: "s1".into(),
   259	                ports: vec![Port {
   260	                    id: "port-1".into(),
   261	                    name: None,
   262	                    network_id: "net-1".into(),
   263	                    fixed_ips: vec![FixedIp { subnet_id: "sub-1".into(), ip_address: "10.0.0.5".into() }],
   264	                    device_id: Some("s1".into()),
   265	                    device_owner: Some("compute:az1".into()),
   266	                    status: "ACTIVE".into(),
   267	                    tenant_id: None,
   268	                }],
   269	            },
   270	        ];
   271	        assert_eq!(events.len(), 7);
   272	    }
   273	
   274	    #[test]
   275	    fn test_usage_loaded_event() {
   276	        use crate::port::types::TenantUsage;
   277	        let usage = TenantUsage {
   278	            tenant_id: "proj-1".into(),
   279	            total_vcpus_usage: 4.0,
   280	            total_memory_mb_usage: 8192.0,
   281	            total_local_gb_usage: 100.0,
   282	            total_hours: 720.0,
   283	            server_usages: vec![],
   284	        };
   285	        let event = AppEvent::UsageLoaded(vec![usage]);
   286	        match event {
   287	            AppEvent::UsageLoaded(usages) => {
   288	                assert_eq!(usages.len(), 1);
   289	                assert_eq!(usages[0].tenant_id, "proj-1");
   290	            }
   291	            _ => panic!("expected UsageLoaded"),
   292	        }
   293	    }
   294	
   295	    #[test]
   296	    fn test_permission_denied_event() {
   297	        let event = AppEvent::PermissionDenied { operation: "CreateServer".into() };
   298	        match event {
   299	            AppEvent::PermissionDenied { operation } => {
   300	                assert_eq!(operation, "CreateServer");
   301	            }
   302	            _ => panic!("expected PermissionDenied"),
   303	        }
   304	    }
   305	}

exec
/bin/zsh -lc "nl -ba src/worker.rs | sed -n '1,360p'" in /Users/jay.ahn/projects/infra/nexttui/.worktrees/runtime-context-switch
 succeeded in 0ms:
     1	//! Background worker: consumes Actions from the UI, calls OpenStack APIs,
     2	//! and sends AppEvents back to the event loop for UI updates.
     3	
     4	use std::collections::HashSet;
     5	use std::sync::{Arc, Mutex};
     6	use std::sync::atomic::{AtomicBool, Ordering};
     7	
     8	use chrono::{DateTime, Utc};
     9	use tokio::sync::mpsc;
    10	use tracing::Instrument;
    11	
    12	use crate::action::Action;
    13	use crate::adapter::registry::AdapterRegistry;
    14	use crate::event::AppEvent;
    15	use crate::infra::rbac::{ActionKind, RbacGuard};
    16	use crate::port::types::*;
    17	
    18	/// Run the background worker loop.
    19	/// Receives Actions from `action_rx`, calls the appropriate API via `registry`,
    20	/// and sends resulting AppEvents to `event_tx`.
    21	#[tracing::instrument(skip_all)]
    22	pub async fn run_worker(
    23	    registry: Arc<AdapterRegistry>,
    24	    rbac: Arc<RbacGuard>,
    25	    all_tenants: Arc<AtomicBool>,
    26	    mut action_rx: mpsc::UnboundedReceiver<Action>,
    27	    event_tx: mpsc::UnboundedSender<AppEvent>,
    28	) {
    29	    let polling_servers: Arc<Mutex<HashSet<String>>> = Arc::new(Mutex::new(HashSet::new()));
    30	    let in_flight_fetches: Arc<Mutex<HashSet<String>>> = Arc::new(Mutex::new(HashSet::new()));
    31	
    32	    while let Some(action) = action_rx.recv().await {
    33	        // RBAC guard: check CUD permissions before API call
    34	        if let Some(kind) = action_to_kind(&action)
    35	            && !rbac.can_perform(kind)
    36	        {
    37	            let _ = event_tx.send(AppEvent::PermissionDenied {
    38	                operation: action_name(&action).to_string(),
    39	            });
    40	            continue;
    41	        }
    42	
    43	        // FetchDedup: skip if same fetch is already in-flight
    44	        let dedup_key = fetch_dedup_key(&action);
    45	        if let Some(key) = dedup_key
    46	            && !in_flight_fetches.lock().unwrap_or_else(|e| e.into_inner()).insert(key.to_string())
    47	        {
    48	            continue;
    49	        }
    50	
    51	        let registry = registry.clone();
    52	        let event_tx = event_tx.clone();
    53	        let all_tenants = all_tenants.clone();
    54	        let polling_servers = polling_servers.clone();
    55	        let in_flight_fetches = in_flight_fetches.clone();
    56	
    57	        let poll_migration_id = poll_migration_server_id(&action);
    58	        let poll_status_id = poll_server_id_for_status(&action);
    59	
    60	        let span = tracing::info_span!("worker_task", action = action_name(&action));
    61	        tokio::spawn(
    62	            async move {
    63	                let event = handle_action(&registry, &all_tenants, action).await;
    64	                let success = event.as_ref().is_some_and(|ev| !matches!(ev, AppEvent::ApiError { .. }));
    65	                if let Some(ev) = event {
    66	                    let _ = event_tx.send(ev);
    67	                }
    68	                // Release fetch dedup guard
    69	                if let Some(key) = dedup_key {
    70	                    in_flight_fetches.lock().unwrap_or_else(|e| e.into_inner()).remove(key);
    71	                }
    72	                if success {
    73	                    if let Some(ref server_id) = poll_migration_id
    74	                        && polling_servers.lock().unwrap_or_else(|e| e.into_inner()).insert(server_id.clone())
    75	                    {
    76	                        poll_migration_progress(&registry, &event_tx, server_id).await;
    77	                        polling_servers.lock().unwrap_or_else(|e| e.into_inner()).remove(server_id);
    78	                    }
    79	                    if let Some(ref server_id) = poll_status_id
    80	                        && polling_servers.lock().unwrap_or_else(|e| e.into_inner()).insert(server_id.clone())
    81	                    {
    82	                        poll_server_status(&registry, &event_tx, server_id).await;
    83	                        polling_servers.lock().unwrap_or_else(|e| e.into_inner()).remove(server_id);
    84	                    }
    85	                }
    86	            }
    87	            .instrument(span),
    88	        );
    89	    }
    90	}
    91	
    92	/// Map an Action to its RBAC ActionKind for permission checking.
    93	/// Returns None for read-only/UI actions that need no guard.
    94	fn action_to_kind(action: &Action) -> Option<ActionKind> {
    95	    match action {
    96	        // Create (member-level)
    97	        Action::CreateServer(_)
    98	        | Action::CreateFlavor(_)
    99	        | Action::CreateNetwork(_)
   100	        | Action::CreateSecurityGroup(_)
   101	        | Action::CreateSecurityGroupRule(_)
   102	        | Action::CreateFloatingIp { .. }
   103	        | Action::CreateVolume(_)
   104	        | Action::CreateSnapshot(_)
   105	        | Action::CreateImage(_)
   106	        | Action::CreateServerSnapshot { .. } => Some(ActionKind::Create),
   107	
   108	        // Create (admin-only: identity resources)
   109	        Action::CreateProject(_)
   110	        | Action::CreateUser(_) => Some(ActionKind::ManageQuota),
   111	
   112	        // Delete (member-level, non-force)
   113	        Action::DeleteServer { .. }
   114	        | Action::DeleteFlavor { .. }
   115	        | Action::DeleteSecurityGroup { .. }
   116	        | Action::DeleteSecurityGroupRule { .. }
   117	        | Action::DeleteFloatingIp { .. }
   118	        | Action::DeleteSnapshot { .. }
   119	        | Action::DeleteImage { .. } => Some(ActionKind::Delete),
   120	
   121	        // Delete (admin-only: identity resources)
   122	        Action::DeleteProject { .. }
   123	        | Action::DeleteUser { .. } => Some(ActionKind::ManageQuota),
   124	
   125	        // Force delete
   126	        Action::DeleteVolume { force: true, .. } => Some(ActionKind::ForceDelete),
   127	        Action::DeleteVolume { force: false, .. } => Some(ActionKind::Delete),
   128	
   129	        // Resize (member-level)
   130	        Action::ResizeServer { .. }
   131	        | Action::ConfirmResize { .. }
   132	        | Action::RevertResize { .. } => Some(ActionKind::Resize),
   133	
   134	        // Migration / Evacuate (admin-only)
   135	        Action::LiveMigrateServer { .. }
   136	        | Action::ColdMigrateServer { .. }
   137	        | Action::ConfirmMigration { .. }
   138	        | Action::RevertMigration { .. } => Some(ActionKind::Migrate),
   139	
   140	        Action::EvacuateServer { .. } => Some(ActionKind::Evacuate),
   141	
   142	        Action::DisableComputeService { .. }
   143	        | Action::EnableComputeService { .. } => Some(ActionKind::EnableDisable),
   144	
   145	        // Server lifecycle — treated as CUD for RBAC purposes
   146	        Action::RebootServer { .. }
   147	        | Action::StartServer { .. }
   148	        | Action::StopServer { .. } => Some(ActionKind::Create),
   149	
   150	        // Volume extend
   151	        Action::ExtendVolume { .. } => Some(ActionKind::Create),
   152	
   153	        // Attach / Associate (member-level)
   154	        Action::AttachVolume { .. }
   155	        | Action::AssociateFloatingIp { .. } => Some(ActionKind::Attach),
   156	
   157	        // Detach / Disassociate (member-level)
   158	        Action::DetachVolume { .. }
   159	        | Action::DisassociateFloatingIp { .. } => Some(ActionKind::Detach),
   160	
   161	        // Force operations (admin-only)
   162	        Action::ForceDetachVolume { .. }
   163	        | Action::ForceResetVolumeState { .. } => Some(ActionKind::ForceDelete),
   164	
   165	        // Read / UI / System — no guard
   166	        _ => None,
   167	    }
   168	}
   169	
   170	/// Human-readable name for an Action, used in PermissionDenied messages.
   171	fn action_name(action: &Action) -> &str {
   172	    match action {
   173	        Action::CreateServer(_) => "CreateServer",
   174	        Action::DeleteServer { .. } => "DeleteServer",
   175	        Action::RebootServer { .. } => "RebootServer",
   176	        Action::StartServer { .. } => "StartServer",
   177	        Action::StopServer { .. } => "StopServer",
   178	        Action::CreateServerSnapshot { .. } => "CreateServerSnapshot",
   179	        Action::CreateFlavor(_) => "CreateFlavor",
   180	        Action::DeleteFlavor { .. } => "DeleteFlavor",
   181	        Action::CreateNetwork(_) => "CreateNetwork",
   182	        Action::CreateSecurityGroup(_) => "CreateSecurityGroup",
   183	        Action::DeleteSecurityGroup { .. } => "DeleteSecurityGroup",
   184	        Action::CreateSecurityGroupRule(_) => "CreateSecurityGroupRule",
   185	        Action::DeleteSecurityGroupRule { .. } => "DeleteSecurityGroupRule",
   186	        Action::CreateFloatingIp { .. } => "CreateFloatingIp",
   187	        Action::DeleteFloatingIp { .. } => "DeleteFloatingIp",
   188	        Action::CreateVolume(_) => "CreateVolume",
   189	        Action::DeleteVolume { .. } => "DeleteVolume",
   190	        Action::ExtendVolume { .. } => "ExtendVolume",
   191	        Action::CreateSnapshot(_) => "CreateSnapshot",
   192	        Action::DeleteSnapshot { .. } => "DeleteSnapshot",
   193	        Action::CreateImage(_) => "CreateImage",
   194	        Action::DeleteImage { .. } => "DeleteImage",
   195	        Action::CreateProject(_) => "CreateProject",
   196	        Action::DeleteProject { .. } => "DeleteProject",
   197	        Action::CreateUser(_) => "CreateUser",
   198	        Action::DeleteUser { .. } => "DeleteUser",
   199	        Action::ResizeServer { .. } => "ResizeServer",
   200	        Action::ConfirmResize { .. } => "ConfirmResize",
   201	        Action::RevertResize { .. } => "RevertResize",
   202	        Action::LiveMigrateServer { .. } => "LiveMigrateServer",
   203	        Action::ColdMigrateServer { .. } => "ColdMigrateServer",
   204	        Action::ConfirmMigration { .. } => "ConfirmMigration",
   205	        Action::RevertMigration { .. } => "RevertMigration",
   206	        Action::EvacuateServer { .. } => "EvacuateServer",
   207	        Action::DisableComputeService { .. } => "DisableComputeService",
   208	        Action::EnableComputeService { .. } => "EnableComputeService",
   209	        Action::FetchMigrationProgress { .. } => "FetchMigrationProgress",
   210	        Action::AttachVolume { .. } => "AttachVolume",
   211	        Action::DetachVolume { .. } => "DetachVolume",
   212	        Action::ForceDetachVolume { .. } => "ForceDetachVolume",
   213	        Action::ForceResetVolumeState { .. } => "ForceResetVolumeState",
   214	        Action::AssociateFloatingIp { .. } => "AssociateFloatingIp",
   215	        Action::DisassociateFloatingIp { .. } => "DisassociateFloatingIp",
   216	        Action::FetchPorts { .. } => "FetchPorts",
   217	        Action::FetchUsage { .. } => "FetchUsage",
   218	        _ => "Unknown",
   219	    }
   220	}
   221	
   222	async fn handle_action(registry: &AdapterRegistry, all_tenants: &AtomicBool, action: Action) -> Option<AppEvent> {
   223	    let action_label = action_name(&action);
   224	    tracing::info!(action = action_label, "handling action");
   225	    let default_pagination = PaginationParams::default();
   226	    let at = all_tenants.load(Ordering::Relaxed);
   227	
   228	    match action {
   229	        // -- Nova: Servers --------------------------------------------------
   230	        Action::FetchServers => {
   231	            match registry
   232	                .nova
   233	                .list_servers(&ServerListFilter { all_tenants: at, ..Default::default() }, &default_pagination)
   234	                .await
   235	            {
   236	                Ok(resp) => Some(AppEvent::ServersLoaded(resp.items)),
   237	                Err(e) => Some(api_error("FetchServers", e)),
   238	            }
   239	        }
   240	        Action::CreateServer(params) => {
   241	            match registry.nova.create_server(&params).await {
   242	                Ok(server) => Some(AppEvent::ServerCreated(server)),
   243	                Err(e) => Some(api_error("CreateServer", e)),
   244	            }
   245	        }
   246	        Action::DeleteServer { id, name } => {
   247	            match registry.nova.delete_server(&id).await {
   248	                Ok(()) => Some(AppEvent::ServerDeleted { id, name }),
   249	                Err(e) => Some(api_error("DeleteServer", e)),
   250	            }
   251	        }
   252	        Action::RebootServer { id, hard } => {
   253	            let reboot_type = if hard {
   254	                RebootType::Hard
   255	            } else {
   256	                RebootType::Soft
   257	            };
   258	            match registry.nova.reboot_server(&id, reboot_type).await {
   259	                Ok(()) => Some(AppEvent::ServerRebooted { id }),
   260	                Err(e) => Some(api_error("RebootServer", e)),
   261	            }
   262	        }
   263	        Action::StartServer { id } => {
   264	            match registry.nova.start_server(&id).await {
   265	                Ok(()) => Some(AppEvent::ServerStarted { id }),
   266	                Err(e) => Some(api_error("StartServer", e)),
   267	            }
   268	        }
   269	        Action::StopServer { id } => {
   270	            match registry.nova.stop_server(&id).await {
   271	                Ok(()) => Some(AppEvent::ServerStopped { id }),
   272	                Err(e) => Some(api_error("StopServer", e)),
   273	            }
   274	        }
   275	        Action::CreateServerSnapshot { server_id, name } => {
   276	            match registry
   277	                .nova
   278	                .create_server_snapshot(&server_id, &name)
   279	                .await
   280	            {
   281	                Ok(image_id) => Some(AppEvent::ServerSnapshotCreated {
   282	                    server_id,
   283	                    image_id,
   284	                }),
   285	                Err(e) => Some(api_error("CreateServerSnapshot", e)),
   286	            }
   287	        }
   288	
   289	        // -- Nova: Resize --------------------------------------------------
   290	        Action::ResizeServer { id, flavor_id } => {
   291	            match registry.nova.resize_server(&id, &flavor_id).await {
   292	                Ok(()) => Some(AppEvent::ServerResized { id }),
   293	                Err(e) => Some(api_error("ResizeServer", e)),
   294	            }
   295	        }
   296	        Action::ConfirmResize { id } => {
   297	            match registry.nova.confirm_migration(&id).await {
   298	                Ok(()) => Some(AppEvent::ResizeConfirmed { id }),
   299	                Err(e) => Some(api_error("ConfirmResize", e)),
   300	            }
   301	        }
   302	        Action::RevertResize { id } => {
   303	            match registry.nova.revert_migration(&id).await {
   304	                Ok(()) => Some(AppEvent::ResizeReverted { id }),
   305	                Err(e) => Some(api_error("RevertResize", e)),
   306	            }
   307	        }
   308	
   309	        // -- Nova: Migration / Evacuate ------------------------------------
   310	        Action::LiveMigrateServer { id, host } => {
   311	            let params = LiveMigrateParams { host };
   312	            match registry.nova.live_migrate_server(&id, &params).await {
   313	                Ok(()) => Some(AppEvent::ServerLiveMigrated { id }),
   314	                Err(e) => Some(api_error("LiveMigrateServer", e)),
   315	            }
   316	        }
   317	        Action::ColdMigrateServer { id } => {
   318	            match registry.nova.cold_migrate_server(&id).await {
   319	                Ok(()) => Some(AppEvent::ServerColdMigrated { id }),
   320	                Err(e) => Some(api_error("ColdMigrateServer", e)),
   321	            }
   322	        }
   323	        Action::ConfirmMigration { id } => {
   324	            match registry.nova.confirm_migration(&id).await {
   325	                Ok(()) => Some(AppEvent::MigrationConfirmed { id }),
   326	                Err(e) => Some(api_error("ConfirmMigration", e)),
   327	            }
   328	        }
   329	        Action::RevertMigration { id } => {
   330	            match registry.nova.revert_migration(&id).await {
   331	                Ok(()) => Some(AppEvent::MigrationReverted { id }),
   332	                Err(e) => Some(api_error("RevertMigration", e)),
   333	            }
   334	        }
   335	        Action::EvacuateServer { id, params } => {
   336	            match registry.nova.evacuate_server(&id, &params).await {
   337	                Ok(()) => Some(AppEvent::ServerEvacuateResult { id, result: Ok(()) }),
   338	                Err(e) => Some(AppEvent::ServerEvacuateResult {
   339	                    id,
   340	                    result: Err(e.to_string()),
   341	                }),
   342	            }
   343	        }
   344	        Action::DisableComputeService { service_id, hostname } => {
   345	            match registry.nova.disable_compute_service(&service_id, None).await {
   346	                Ok(_) => Some(AppEvent::ComputeServiceToggled { hostname, enabled: false }),
   347	                Err(e) => Some(api_error("DisableComputeService", e)),
   348	            }
   349	        }
   350	        Action::EnableComputeService { service_id, hostname } => {
   351	            match registry.nova.enable_compute_service(&service_id).await {
   352	                Ok(_) => Some(AppEvent::ComputeServiceToggled { hostname, enabled: true }),
   353	                Err(e) => Some(api_error("EnableComputeService", e)),
   354	            }
   355	        }
   356	        Action::FetchMigrationProgress { server_id } => {
   357	            match registry.nova.list_server_migrations(&server_id).await {
   358	                Ok(migrations) => {
   359	                    if let Some(migration) = migrations.into_iter().last() {
   360	                        Some(AppEvent::MigrationProgressLoaded { server_id, migration })

exec
/bin/zsh -lc "nl -ba src/app.rs | sed -n '1,420p'" in /Users/jay.ahn/projects/infra/nexttui/.worktrees/runtime-context-switch
 succeeded in 0ms:
     1	use std::collections::HashMap;
     2	use std::sync::Arc;
     3	use std::sync::atomic::{AtomicBool, Ordering};
     4	
     5	use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
     6	use ratatui::Frame;
     7	use ratatui::widgets::{Block, BorderType, Borders};
     8	use tokio::sync::mpsc;
     9	
    10	use crate::action::Action;
    11	use crate::background::BackgroundTracker;
    12	use crate::component::{Component, InputMode, LayoutHint};
    13	use crate::config::Config;
    14	use crate::event::AppEvent;
    15	use crate::infra::audit::{AuditEntry, AuditLogger, AuditResult};
    16	use crate::infra::rbac::{ActionKind, RbacGuard};
    17	use crate::models::common::Route;
    18	use crate::router::Router;
    19	use crate::ui::header::{Header, HeaderContext};
    20	use crate::ui::layout::LayoutManager;
    21	use crate::ui::sidebar::Sidebar;
    22	use crate::ui::status_bar::{StatusBar, StatusInfo};
    23	use crate::ui::theme::{self, Theme};
    24	use crate::ui::refresh::RefreshScheduler;
    25	use crate::ui::activity_log::{ActivityLog, ActivityLogPopup};
    26	use crate::ui::toast::{ToastMessage, ToastSeverity};
    27	
    28	#[derive(Debug, Clone, Copy, PartialEq, Eq)]
    29	pub enum FocusPane {
    30	    Sidebar,
    31	    Content,
    32	}
    33	
    34	pub struct App {
    35	    pub should_quit: bool,
    36	    pub input_mode: InputMode,
    37	    pub sidebar_visible: bool,
    38	    pub focus: FocusPane,
    39	
    40	    router: Router,
    41	    components: HashMap<Route, Box<dyn Component>>,
    42	    background_tracker: BackgroundTracker,
    43	    action_tx: mpsc::UnboundedSender<Action>,
    44	
    45	    pub rbac: Arc<RbacGuard>,
    46	    pub all_tenants: Arc<AtomicBool>,
    47	    config: Arc<Config>,
    48	    layout: LayoutManager,
    49	    sidebar: Sidebar,
    50	    header: Header,
    51	    status_bar: StatusBar,
    52	    route_labels: HashMap<Route, &'static str>,
    53	    refresh_scheduler: RefreshScheduler,
    54	    activity_log: ActivityLog,
    55	    activity_popup: ActivityLogPopup,
    56	    show_activity_log: bool,
    57	    audit_logger: Option<AuditLogger>,
    58	}
    59	
    60	impl App {
    61	    pub fn new(config: Config, action_tx: mpsc::UnboundedSender<Action>) -> Self {
    62	        let tick_rate = std::time::Duration::from_millis(config.app_config().tick_rate_ms);
    63	        crate::ui::theme::Theme::init(config.app_config().theme);
    64	        let audit_logger = Self::init_audit_logger();
    65	        Self {
    66	            should_quit: false,
    67	            input_mode: InputMode::Normal,
    68	            sidebar_visible: true,
    69	            focus: FocusPane::Content,
    70	            router: Router::new(Route::Servers),
    71	            components: HashMap::new(),
    72	            background_tracker: BackgroundTracker::new(),
    73	            action_tx,
    74	            rbac: Arc::new(RbacGuard::new()),
    75	            all_tenants: Arc::new(AtomicBool::new(false)),
    76	            config: Arc::new(config),
    77	            layout: LayoutManager::new(),
    78	            sidebar: Sidebar::new(Vec::new()),
    79	            header: Header::new(),
    80	            status_bar: StatusBar::new(),
    81	            route_labels: HashMap::new(),
    82	            refresh_scheduler: RefreshScheduler::new(tick_rate),
    83	            activity_log: ActivityLog::new(),
    84	            activity_popup: ActivityLogPopup::new(),
    85	            show_activity_log: false,
    86	            audit_logger,
    87	        }
    88	    }
    89	
    90	    pub fn from_registry(
    91	        config: Config,
    92	        action_tx: mpsc::UnboundedSender<Action>,
    93	        registry: crate::registry::ModuleRegistry,
    94	        rbac: Arc<RbacGuard>,
    95	    ) -> (Self, Vec<Action>) {
    96	        let parts = registry.into_parts();
    97	        let tick_rate = std::time::Duration::from_millis(config.app_config().tick_rate_ms);
    98	        crate::ui::theme::Theme::init(config.app_config().theme);
    99	        let audit_logger = Self::init_audit_logger();
   100	        let mut app = Self {
   101	            should_quit: false,
   102	            input_mode: InputMode::Normal,
   103	            sidebar_visible: true,
   104	            focus: FocusPane::Content,
   105	            router: Router::new(Route::Servers),
   106	            components: parts.components,
   107	            background_tracker: BackgroundTracker::new(),
   108	            action_tx,
   109	            rbac,
   110	            all_tenants: Arc::new(AtomicBool::new(false)),
   111	            config: Arc::new(config),
   112	            layout: LayoutManager::new(),
   113	            sidebar: Sidebar::new(parts.sidebar_items),
   114	            header: Header::new(),
   115	            status_bar: StatusBar::new(),
   116	            route_labels: parts.route_labels,
   117	            refresh_scheduler: RefreshScheduler::new(tick_rate),
   118	            activity_log: ActivityLog::new(),
   119	            activity_popup: ActivityLogPopup::new(),
   120	            show_activity_log: false,
   121	            audit_logger,
   122	        };
   123	        // Store sidebar items for number-key navigation
   124	        app.sidebar.sync_active(&Route::Servers, false);
   125	        app.broadcast_admin();
   126	        (app, parts.initial_actions)
   127	    }
   128	
   129	    /// Broadcast current admin status to all registered modules.
   130	    pub fn broadcast_admin(&mut self) {
   131	        let is_admin = self.rbac.is_admin();
   132	        for component in self.components.values_mut() {
   133	            component.set_admin(is_admin);
   134	        }
   135	    }
   136	
   137	    pub fn route_label(&self, route: &Route) -> &str {
   138	        self.route_labels.get(route).copied().unwrap_or("Unknown")
   139	    }
   140	
   141	    /// Register a domain module component for a given route (test use only).
   142	    #[cfg(test)]
   143	    pub fn register_component(&mut self, route: Route, component: Box<dyn Component>) {
   144	        self.components.insert(route, component);
   145	    }
   146	
   147	    /// Inject an audit logger for testing.
   148	    #[cfg(test)]
   149	    pub fn set_audit_logger(&mut self, logger: AuditLogger) {
   150	        self.audit_logger = Some(logger);
   151	    }
   152	
   153	    /// Handle key input. Returns true if a re-render is needed.
   154	    pub fn handle_key(&mut self, key: KeyEvent) -> bool {
   155	        let no_modifiers = key.modifiers.is_empty();
   156	
   157	        // Activity log popup pseudo-modal: intercept j/k/Esc/! only
   158	        if self.show_activity_log {
   159	            match key.code {
   160	                KeyCode::Char('j') => {
   161	                    self.activity_popup
   162	                        .scroll_down(self.activity_log.entries().len());
   163	                }
   164	                KeyCode::Char('k') => {
   165	                    self.activity_popup.scroll_up();
   166	                }
   167	                KeyCode::Esc => {
   168	                    self.show_activity_log = false;
   169	                    self.activity_popup.reset_scroll();
   170	                }
   171	                KeyCode::Char('!') => {
   172	                    self.show_activity_log = false;
   173	                    self.activity_popup.reset_scroll();
   174	                }
   175	                KeyCode::Char('w') => {
   176	                    let path = std::path::PathBuf::from("/tmp/nexttui-activity.log");
   177	                    if let Err(e) = self.activity_log.export_to_file(&path) {
   178	                        self.background_tracker.add_toast(
   179	                            format!("Export failed: {e}"),
   180	                            crate::background::ToastLevel::Error,
   181	                        );
   182	                    } else {
   183	                        self.background_tracker.add_toast(
   184	                            format!("Activity log exported to {}", path.display()),
   185	                            crate::background::ToastLevel::Info,
   186	                        );
   187	                    }
   188	                }
   189	                _ => {}
   190	            }
   191	            return true;
   192	        }
   193	
   194	        // Ctrl+c always quits
   195	        if key.code == KeyCode::Char('c') && key.modifiers.contains(KeyModifiers::CONTROL) {
   196	            self.should_quit = true;
   197	            return true;
   198	        }
   199	
   200	        // Ctrl+a toggles all_tenants (admin only)
   201	        if key.code == KeyCode::Char('a') && key.modifiers.contains(KeyModifiers::CONTROL) {
   202	            if self.rbac.can_perform(ActionKind::ViewAllTenants) {
   203	                self.dispatch_action(Action::ToggleAllTenants);
   204	            }
   205	            return true;
   206	        }
   207	
   208	        // '!' toggle activity log (Shift+1 sends '!' with SHIFT modifier)
   209	        if self.input_mode == InputMode::Normal && key.code == KeyCode::Char('!') {
   210	            self.show_activity_log = !self.show_activity_log;
   211	            if self.show_activity_log {
   212	                self.activity_log.mark_all_read();
   213	            }
   214	            return true;
   215	        }
   216	
   217	        // Modal component (ConfirmDialog, SelectPopup) — delegate all keys directly
   218	        if self.input_mode == InputMode::Normal {
   219	            let is_modal = self.components.get(&self.router.current())
   220	                .map_or(false, |c| c.is_modal());
   221	            if is_modal {
   222	                if let Some(component) = self.components.get_mut(&self.router.current()) {
   223	                    if let Some(action) = component.handle_key(key) {
   224	                        self.dispatch_action(action);
   225	                    }
   226	                }
   227	                return true;
   228	            }
   229	        }
   230	
   231	        // Global keys in Normal mode (only without modifiers to avoid Ctrl+q etc.)
   232	        if self.input_mode == InputMode::Normal && no_modifiers {
   233	            match key.code {
   234	                KeyCode::Char(':') => {
   235	                    self.input_mode = InputMode::Command;
   236	                    return true;
   237	                }
   238	                // '/' search is handled by SelectPopup when open (not App-level)
   239	                // KeyCode::Char('/') — disabled: App-level search mode is unimplemented
   240	                KeyCode::Tab => {
   241	                    // FullWidth module: Tab restores sidebar and returns to previous route
   242	                    let full_width = self.components.get(&self.router.current())
   243	                        .map_or(false, |c| c.layout_hint() == LayoutHint::FullWidth);
   244	                    if full_width {
   245	                        // Block exit while module is busy (e.g. evacuating)
   246	                        let busy = self.components.get(&self.router.current())
   247	                            .map_or(false, |c| c.is_busy());
   248	                        if busy { return true; }
   249	                        self.sidebar_visible = true;
   250	                        self.layout.set_sidebar_visible(true);
   251	                        self.router.back();
   252	                        self.sidebar.sync_active(&self.router.current(), self.rbac.is_admin());
   253	                        self.focus = FocusPane::Sidebar;
   254	                    } else if self.sidebar_visible {
   255	                        self.focus = match self.focus {
   256	                            FocusPane::Content => FocusPane::Sidebar,
   257	                            FocusPane::Sidebar => FocusPane::Content,
   258	                        };
   259	                    }
   260	                    return true;
   261	                }
   262	                KeyCode::Char('q') => {
   263	                    self.should_quit = true;
   264	                    return true;
   265	                }
   266	                KeyCode::Char(c @ '1'..='9') | KeyCode::Char(c @ '0') | KeyCode::Char(c @ 'h') => {
   267	                    // Block route switching while current module is busy (e.g. evacuating)
   268	                    let busy = self.components.get(&self.router.current())
   269	                        .map_or(false, |comp| comp.is_busy());
   270	                    if busy { return true; }
   271	
   272	                    if c == 'h' {
   273	                        // 'h' shortcut for Host Ops
   274	                        if self.rbac.is_admin() {
   275	                            self.dispatch_action(Action::Navigate(Route::Hosts));
   276	                        }
   277	                    } else {
   278	                        let idx = if c == '0' { 9 } else { (c as usize) - ('1' as usize) };
   279	                        if let Some(route) = self.sidebar.route_at(idx, self.rbac.is_admin()) {
   280	                            self.dispatch_action(Action::Navigate(route));
   281	                        }
   282	                    }
   283	                    return true;
   284	                }
   285	                KeyCode::Esc => {
   286	                    if self.focus == FocusPane::Sidebar {
   287	                        self.focus = FocusPane::Content;
   288	                        return true;
   289	                    }
   290	                    // Fall through to let component handle Esc
   291	                    // (Detail→List transition, or return Action::Back for router)
   292	                }
   293	                _ => {}
   294	            }
   295	        }
   296	
   297	        // Esc from Command/Search/Confirm → Normal
   298	        if matches!(
   299	            self.input_mode,
   300	            InputMode::Command | InputMode::Search | InputMode::Confirm
   301	        ) && key.code == KeyCode::Esc
   302	        {
   303	            self.input_mode = InputMode::Normal;
   304	            return true;
   305	        }
   306	
   307	        // Form mode: delegate all keys to the active component (FormWidget handles everything)
   308	        if self.input_mode == InputMode::Form {
   309	            if let Some(component) = self.components.get_mut(&self.router.current()) {
   310	                if let Some(action) = component.handle_key(key) {
   311	                    self.dispatch_action(action);
   312	                }
   313	            }
   314	            return true;
   315	        }
   316	
   317	        // Delegate based on focus pane
   318	        if self.input_mode == InputMode::Normal {
   319	            if self.focus == FocusPane::Sidebar && self.sidebar_visible {
   320	                if let Some(action) = self.sidebar.handle_key(key, self.rbac.is_admin()) {
   321	                    self.dispatch_action(action);
   322	                }
   323	                return true;
   324	            }
   325	
   326	            if let Some(component) = self.components.get_mut(&self.router.current()) {
   327	                if let Some(action) = component.handle_key(key) {
   328	                    self.dispatch_action(action);
   329	                }
   330	                return true;
   331	            }
   332	
   333	            // Fallback: Esc with no component registered → router back
   334	            if key.code == KeyCode::Esc {
   335	                self.router.back();
   336	                return true;
   337	            }
   338	        }
   339	
   340	        true
   341	    }
   342	
   343	    /// Handle action — intercept navigation actions, forward the rest to action_tx.
   344	    fn dispatch_action(&mut self, action: Action) {
   345	        match action {
   346	            Action::Navigate(route) => {
   347	                self.router.navigate(route);
   348	                self.sidebar.sync_active(&self.router.current(), self.rbac.is_admin());
   349	                self.focus = FocusPane::Content;
   350	                // LayoutHint::FullWidth modules hide the sidebar
   351	                let full_width = self.components.get(&self.router.current())
   352	                    .map_or(false, |c| c.layout_hint() == LayoutHint::FullWidth);
   353	                if full_width && self.sidebar_visible {
   354	                    self.sidebar_visible = false;
   355	                } else if !full_width && !self.sidebar_visible {
   356	                    self.sidebar_visible = true;
   357	                }
   358	                self.layout.set_sidebar_visible(self.sidebar_visible);
   359	                self.refresh_scheduler.reset();
   360	            }
   361	            Action::Back => {
   362	                self.router.back();
   363	                // Restore sidebar if leaving a FullWidth module
   364	                let full_width = self.components.get(&self.router.current())
   365	                    .map_or(false, |c| c.layout_hint() == LayoutHint::FullWidth);
   366	                if !full_width && !self.sidebar_visible {
   367	                    self.sidebar_visible = true;
   368	                    self.layout.set_sidebar_visible(true);
   369	                }
   370	                self.sidebar.sync_active(&self.router.current(), self.rbac.is_admin());
   371	                self.refresh_scheduler.reset();
   372	            }
   373	            Action::FocusSidebar => {
   374	                if self.sidebar_visible {
   375	                    self.focus = FocusPane::Sidebar;
   376	                }
   377	            }
   378	            Action::EnterFormMode => {
   379	                self.input_mode = InputMode::Form;
   380	            }
   381	            Action::ExitFormMode => {
   382	                self.input_mode = InputMode::Normal;
   383	            }
   384	            Action::ToggleAllTenants => {
   385	                let prev = self.all_tenants.load(Ordering::Relaxed);
   386	                self.all_tenants.store(!prev, Ordering::Relaxed);
   387	                // Broadcast to modules
   388	                for component in self.components.values_mut() {
   389	                    component.set_all_tenants(!prev);
   390	                }
   391	                // Re-fetch all resources with new filter
   392	                let fetches = [
   393	                    Action::FetchServers,
   394	                    Action::FetchNetworks,
   395	                    Action::FetchSecurityGroups,
   396	                    Action::FetchFloatingIps,
   397	                    Action::FetchVolumes,
   398	                    Action::FetchSnapshots,
   399	                    Action::FetchImages,
   400	                ];
   401	                for a in fetches {
   402	                    let _ = self.action_tx.send(a);
   403	                }
   404	            }
   405	            Action::ShowToast { message } => {
   406	                self.background_tracker.add_toast(message, crate::background::ToastLevel::Info);
   407	            }
   408	            Action::Quit => {
   409	                self.should_quit = true;
   410	            }
   411	            other => {
   412	                if let Some(msg) = Self::progress_toast_text(&other) {
   413	                    self.background_tracker.add_toast(msg, crate::background::ToastLevel::Info);
   414	                }
   415	                let _ = self.action_tx.send(other);
   416	            }
   417	        }
   418	    }
   419	
   420	    /// Handle background event — broadcast to all registered components and generate toasts.

exec
/bin/zsh -lc "nl -ba src/port/error.rs | sed -n '1,260p'" in /Users/jay.ahn/projects/infra/nexttui/.worktrees/runtime-context-switch
 succeeded in 0ms:
     1	use thiserror::Error;
     2	
     3	#[derive(Debug, Error)]
     4	#[non_exhaustive]
     5	pub enum ApiError {
     6	    #[error("Authentication failed: {0}")]
     7	    AuthFailed(String),
     8	
     9	    #[error("Token expired")]
    10	    TokenExpired,
    11	
    12	    #[error("Forbidden: {0}")]
    13	    Forbidden(String),
    14	
    15	    #[error("Not found: {resource_type} {id}")]
    16	    NotFound { resource_type: String, id: String },
    17	
    18	    #[error("Conflict: {0}")]
    19	    Conflict(String),
    20	
    21	    #[error("Bad request: {0}")]
    22	    BadRequest(String),
    23	
    24	    #[error("Rate limited: retry after {retry_after_secs}s")]
    25	    RateLimited { retry_after_secs: u64 },
    26	
    27	    #[error("Service unavailable: {service}")]
    28	    ServiceUnavailable { service: String },
    29	
    30	    #[error("Network error: {0}")]
    31	    Network(#[from] reqwest::Error),
    32	
    33	    #[error("Response parse error: {0}")]
    34	    Parse(String),
    35	
    36	    #[error("Unexpected: {status} {}", truncate_body(body))]
    37	    Unexpected { status: u16, body: String },
    38	}
    39	
    40	pub type ApiResult<T> = Result<T, ApiError>;
    41	
    42	fn truncate_body(body: &str) -> String {
    43	    const MAX_LEN: usize = 200;
    44	    if body.len() <= MAX_LEN {
    45	        body.to_string()
    46	    } else {
    47	        format!("{}...[truncated]", &body[..MAX_LEN])
    48	    }
    49	}
    50	
    51	#[cfg(test)]
    52	mod tests {
    53	    use super::*;
    54	
    55	    #[test]
    56	    fn test_api_error_display() {
    57	        let err = ApiError::AuthFailed("bad creds".into());
    58	        assert!(err.to_string().contains("bad creds"));
    59	
    60	        let err = ApiError::NotFound {
    61	            resource_type: "server".into(),
    62	            id: "abc".into(),
    63	        };
    64	        assert!(err.to_string().contains("server"));
    65	        assert!(err.to_string().contains("abc"));
    66	
    67	        let err = ApiError::RateLimited {
    68	            retry_after_secs: 30,
    69	        };
    70	        assert!(err.to_string().contains("30"));
    71	
    72	        let err = ApiError::Unexpected {
    73	            status: 500,
    74	            body: "internal".into(),
    75	        };
    76	        assert!(err.to_string().contains("500"));
    77	    }
    78	}

exec
/bin/zsh -lc "nl -ba src/port/types.rs | sed -n '1,360p'" in /Users/jay.ahn/projects/infra/nexttui/.worktrees/runtime-context-switch
 succeeded in 0ms:
     1	use std::collections::HashMap;
     2	
     3	use chrono::{DateTime, Utc};
     4	use serde::{Deserialize, Serialize};
     5	
     6	// --- Pagination ---
     7	
     8	#[derive(Debug, Clone, Default)]
     9	pub struct PaginationParams {
    10	    pub marker: Option<String>,
    11	    pub limit: Option<u32>,
    12	    pub sort_key: Option<String>,
    13	    pub sort_dir: Option<SortDirection>,
    14	}
    15	
    16	#[derive(Debug, Clone, Copy)]
    17	pub enum SortDirection {
    18	    Asc,
    19	    Desc,
    20	}
    21	
    22	#[derive(Debug, Clone)]
    23	pub struct PaginatedResponse<T> {
    24	    pub items: Vec<T>,
    25	    pub next_marker: Option<String>,
    26	    pub has_more: bool,
    27	}
    28	
    29	impl<T> PaginatedResponse<T> {
    30	    pub fn empty() -> Self {
    31	        Self {
    32	            items: Vec::new(),
    33	            next_marker: None,
    34	            has_more: false,
    35	        }
    36	    }
    37	}
    38	
    39	// --- Auth ---
    40	
    41	#[derive(Debug, Clone, Hash, Eq, PartialEq, Serialize, Deserialize)]
    42	pub enum TokenScope {
    43	    Project { name: String, domain: String },
    44	    Unscoped,
    45	}
    46	
    47	impl TokenScope {
    48	    pub fn from_credential(credential: &AuthCredential) -> Self {
    49	        match &credential.project_scope {
    50	            Some(p) => Self::Project {
    51	                name: p.name.to_lowercase(),
    52	                domain: p.domain_name.to_lowercase(),
    53	            },
    54	            None => Self::Unscoped,
    55	        }
    56	    }
    57	
    58	    /// Generate a filesystem-safe cache key.
    59	    /// Uses `@` as separator (not `_` which appears in project/domain names).
    60	    /// Sanitizes path traversal characters.
    61	    pub fn cache_key(&self) -> String {
    62	        match self {
    63	            Self::Project { name, domain } => {
    64	                let safe_name = sanitize_for_filename(name);
    65	                let safe_domain = sanitize_for_filename(domain);
    66	                format!("project@{safe_name}@{safe_domain}")
    67	            }
    68	            Self::Unscoped => "unscoped".to_string(),
    69	        }
    70	    }
    71	}
    72	
    73	/// Remove path-traversal and filesystem-unsafe characters from a string.
    74	fn sanitize_for_filename(s: &str) -> String {
    75	    s.chars()
    76	        .map(|c| match c {
    77	            '/' | '\\' | '\0' | '.' => '_',
    78	            _ => c,
    79	        })
    80	        .collect()
    81	}
    82	
    83	#[derive(Clone, Serialize, Deserialize)]
    84	pub struct Token {
    85	    pub id: String,
    86	    pub expires_at: DateTime<Utc>,
    87	    pub project: ProjectScope,
    88	    pub roles: Vec<TokenRole>,
    89	    pub catalog: Vec<CatalogEntry>,
    90	}
    91	
    92	impl std::fmt::Debug for Token {
    93	    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    94	        f.debug_struct("Token")
    95	            .field("id", &"****")
    96	            .field("expires_at", &self.expires_at)
    97	            .field("project", &self.project)
    98	            .field("roles", &self.roles)
    99	            .field("catalog", &format!("[{} entries]", self.catalog.len()))
   100	            .finish()
   101	    }
   102	}
   103	
   104	#[derive(Clone, Debug, Serialize, Deserialize)]
   105	pub struct ProjectScope {
   106	    pub id: String,
   107	    pub name: String,
   108	    pub domain_id: String,
   109	    pub domain_name: String,
   110	}
   111	
   112	#[derive(Clone, Debug, Serialize, Deserialize)]
   113	pub struct TokenRole {
   114	    pub id: String,
   115	    pub name: String,
   116	}
   117	
   118	#[derive(Clone, Debug, Serialize, Deserialize)]
   119	pub struct CatalogEntry {
   120	    pub service_type: String,
   121	    pub service_name: String,
   122	    pub endpoints: Vec<Endpoint>,
   123	}
   124	
   125	#[derive(Clone, Debug, Serialize, Deserialize)]
   126	pub struct Endpoint {
   127	    pub region: String,
   128	    pub interface: EndpointInterface,
   129	    pub url: String,
   130	}
   131	
   132	#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
   133	pub enum EndpointInterface {
   134	    Public,
   135	    Internal,
   136	    Admin,
   137	}
   138	
   139	#[derive(Clone, Debug)]
   140	pub struct AuthCredential {
   141	    pub auth_url: String,
   142	    pub method: AuthMethod,
   143	    pub project_scope: Option<ProjectScopeParam>,
   144	}
   145	
   146	#[derive(Clone)]
   147	pub enum AuthMethod {
   148	    Password {
   149	        username: String,
   150	        password: String,
   151	        domain_name: String,
   152	    },
   153	    ApplicationCredential {
   154	        id: String,
   155	        secret: String,
   156	    },
   157	}
   158	
   159	impl std::fmt::Debug for AuthMethod {
   160	    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
   161	        match self {
   162	            AuthMethod::Password {
   163	                username,
   164	                domain_name,
   165	                ..
   166	            } => f
   167	                .debug_struct("Password")
   168	                .field("username", username)
   169	                .field("password", &"****")
   170	                .field("domain_name", domain_name)
   171	                .finish(),
   172	            AuthMethod::ApplicationCredential { id, .. } => f
   173	                .debug_struct("ApplicationCredential")
   174	                .field("id", id)
   175	                .field("secret", &"****")
   176	                .finish(),
   177	        }
   178	    }
   179	}
   180	
   181	#[derive(Clone, Debug)]
   182	pub struct ProjectScopeParam {
   183	    pub name: String,
   184	    pub domain_name: String,
   185	}
   186	
   187	#[derive(Clone)]
   188	pub struct AuthHeaders {
   189	    pub headers: Vec<(String, String)>,
   190	}
   191	
   192	impl std::fmt::Debug for AuthHeaders {
   193	    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
   194	        f.debug_struct("AuthHeaders")
   195	            .field("headers", &format!("[{} entries]", self.headers.len()))
   196	            .finish()
   197	    }
   198	}
   199	
   200	#[derive(Debug, Clone, PartialEq, Eq, Hash)]
   201	pub struct Capability {
   202	    pub resource: String,
   203	    pub action: String,
   204	}
   205	
   206	// --- Filters ---
   207	
   208	#[derive(Debug, Clone, Default)]
   209	pub struct ServerListFilter {
   210	    pub name: Option<String>,
   211	    pub status: Option<String>,
   212	    pub host: Option<String>,
   213	    pub flavor: Option<String>,
   214	    pub all_tenants: bool,
   215	}
   216	
   217	#[derive(Debug, Clone, Default)]
   218	pub struct VolumeListFilter {
   219	    pub name: Option<String>,
   220	    pub status: Option<String>,
   221	    pub all_tenants: bool,
   222	}
   223	
   224	#[derive(Debug, Clone, Default)]
   225	pub struct ImageListFilter {
   226	    pub name: Option<String>,
   227	    pub status: Option<String>,
   228	    pub visibility: Option<String>,
   229	    pub all_tenants: bool,
   230	}
   231	
   232	#[derive(Debug, Clone, Default)]
   233	pub struct NetworkListFilter {
   234	    pub all_tenants: bool,
   235	}
   236	
   237	#[derive(Debug, Clone, Default)]
   238	pub struct SecurityGroupListFilter {
   239	    pub all_tenants: bool,
   240	}
   241	
   242	#[derive(Debug, Clone, Default)]
   243	pub struct FloatingIpListFilter {
   244	    pub all_tenants: bool,
   245	}
   246	
   247	#[derive(Debug, Clone, Default)]
   248	pub struct SnapshotListFilter {
   249	    pub all_tenants: bool,
   250	}
   251	
   252	// --- Nova params ---
   253	
   254	#[derive(Debug, Clone)]
   255	pub enum RebootType {
   256	    Soft,
   257	    Hard,
   258	}
   259	
   260	#[derive(Debug, Clone)]
   261	pub enum ServerState {
   262	    Active,
   263	    Error,
   264	    Paused,
   265	    Suspended,
   266	    Stopped,
   267	}
   268	
   269	#[derive(Debug, Clone)]
   270	pub struct ServerCreateParams {
   271	    pub name: String,
   272	    pub image_id: String,
   273	    pub flavor_id: String,
   274	    pub networks: Vec<NetworkAttachment>,
   275	    pub security_groups: Option<Vec<String>>,
   276	    pub key_name: Option<String>,
   277	    pub availability_zone: Option<String>,
   278	}
   279	
   280	#[derive(Debug, Clone)]
   281	pub struct NetworkAttachment {
   282	    pub uuid: String,
   283	    pub fixed_ip: Option<String>,
   284	}
   285	
   286	#[derive(Debug, Clone)]
   287	pub struct LiveMigrateParams {
   288	    pub host: Option<String>,
   289	}
   290	
   291	#[derive(Debug, Clone, Default)]
   292	pub struct EvacuateParams {
   293	    pub host: Option<String>,
   294	    pub on_shared_storage: Option<bool>,
   295	    pub force: Option<bool>,
   296	}
   297	
   298	#[derive(Debug, Clone)]
   299	pub struct FlavorCreateParams {
   300	    pub name: String,
   301	    pub vcpus: u32,
   302	    pub ram_mb: u32,
   303	    pub disk_gb: u32,
   304	    pub is_public: bool,
   305	}
   306	
   307	#[derive(Debug, Clone)]
   308	pub struct AggregateCreateParams {
   309	    pub name: String,
   310	    pub availability_zone: Option<String>,
   311	}
   312	
   313	#[derive(Debug, Clone)]
   314	pub struct AggregateUpdateParams {
   315	    pub name: Option<String>,
   316	    pub availability_zone: Option<String>,
   317	}
   318	
   319	#[derive(Debug, Clone)]
   320	pub struct ComputeQuotaUpdateParams {
   321	    pub cores: Option<i64>,
   322	    pub ram: Option<i64>,
   323	    pub instances: Option<i64>,
   324	}
   325	
   326	// --- Nova response types ---
   327	
   328	#[derive(Debug, Clone, Deserialize)]
   329	pub struct ServerEvent {
   330	    pub action: String,
   331	    pub start_time: Option<String>,
   332	    pub finish_time: Option<String>,
   333	    pub result: Option<String>,
   334	    pub message: Option<String>,
   335	}
   336	
   337	#[derive(Debug, Clone, Deserialize)]
   338	pub struct ProjectUsage {
   339	    pub total_vcpus_usage: f64,
   340	    pub total_memory_mb_usage: f64,
   341	    pub total_local_gb_usage: f64,
   342	}
   343	
   344	#[derive(Debug, Clone, Deserialize)]
   345	pub struct TenantUsage {
   346	    pub tenant_id: String,
   347	    pub total_vcpus_usage: f64,
   348	    pub total_memory_mb_usage: f64,
   349	    pub total_local_gb_usage: f64,
   350	    #[serde(default)]
   351	    pub total_hours: f64,
   352	    #[serde(default)]
   353	    pub server_usages: Vec<ServerUsageEntry>,
   354	}
   355	
   356	#[derive(Debug, Clone, Deserialize)]
   357	pub struct ServerUsageEntry {
   358	    #[serde(default)]
   359	    pub instance_id: String,
   360	    #[serde(default)]

exec
/bin/zsh -lc "nl -ba src/app.rs | sed -n '420,900p'" in /Users/jay.ahn/projects/infra/nexttui/.worktrees/runtime-context-switch
 succeeded in 0ms:
   420	    /// Handle background event — broadcast to all registered components and generate toasts.
   421	    /// Events like ServersLoaded must reach ServerModule even if the user is on a different view.
   422	    pub fn handle_event(&mut self, event: AppEvent) {
   423	        // RBAC: update roles on token refresh
   424	        if let AppEvent::TokenRefreshed(ref roles) = event {
   425	            self.rbac.update_roles(roles.clone(), None);
   426	            self.broadcast_admin();
   427	        }
   428	        // Migration complete → refresh server list to reflect status change
   429	        let refresh_servers = matches!(
   430	            event,
   431	            AppEvent::MigrationPollingStopped { .. }
   432	            | AppEvent::ServerStatusPolled { .. }
   433	        ) || matches!(
   434	            event,
   435	            AppEvent::ServerLiveMigrated { .. }
   436	            | AppEvent::ServerColdMigrated { .. }
   437	            | AppEvent::MigrationConfirmed { .. }
   438	            | AppEvent::MigrationReverted { .. }
   439	            | AppEvent::ServerEvacuated { .. }
   440	            | AppEvent::ServerEvacuateResult { .. }
   441	            | AppEvent::ServerResized { .. }
   442	            | AppEvent::ResizeConfirmed { .. }
   443	            | AppEvent::ResizeReverted { .. }
   444	        );
   445	        // API backoff: slow down refresh on rate-limit/unavailable errors.
   446	        // NOTE: matches ApiError::RateLimited / ServiceUnavailable Display strings.
   447	        // If those Display impls change, update these patterns (or add a typed field to AppEvent).
   448	        match &event {
   449	            AppEvent::ApiError { message, .. }
   450	                if message.contains("Rate limited") || message.contains("unavailable") =>
   451	            {
   452	                self.refresh_scheduler.backoff();
   453	            }
   454	            AppEvent::ApiError { .. } => {}
   455	            _ => {
   456	                self.refresh_scheduler.reset_backoff();
   457	            }
   458	        }
   459	
   460	        self.generate_toast(&event);
   461	        self.record_audit(&event);
   462	        for component in self.components.values_mut() {
   463	            component.handle_event(&event);
   464	        }
   465	        if refresh_servers {
   466	            let _ = self.action_tx.send(Action::FetchServers);
   467	        }
   468	    }
   469	
   470	    fn progress_toast_text(action: &Action) -> Option<String> {
   471	        match action {
   472	            Action::CreateServer(_) => Some("Creating server...".into()),
   473	            Action::DeleteServer { name, .. } => Some(format!("Deleting server '{name}'...")),
   474	            Action::RebootServer { .. } => Some("Rebooting server...".into()),
   475	            Action::StartServer { .. } => Some("Starting server...".into()),
   476	            Action::StopServer { .. } => Some("Stopping server...".into()),
   477	            Action::CreateServerSnapshot { .. } => Some("Creating snapshot...".into()),
   478	            Action::CreateFlavor(_) => Some("Creating flavor...".into()),
   479	            Action::DeleteFlavor { .. } => Some("Deleting flavor...".into()),
   480	            Action::CreateNetwork(_) => Some("Creating network...".into()),
   481	            Action::CreateSecurityGroup(_) => Some("Creating security group...".into()),
   482	            Action::DeleteSecurityGroup { .. } => Some("Deleting security group...".into()),
   483	            Action::CreateSecurityGroupRule(_) => Some("Creating rule...".into()),
   484	            Action::DeleteSecurityGroupRule { .. } => Some("Deleting rule...".into()),
   485	            Action::CreateFloatingIp { .. } => Some("Creating floating IP...".into()),
   486	            Action::DeleteFloatingIp { .. } => Some("Deleting floating IP...".into()),
   487	            Action::CreateVolume(_) => Some("Creating volume...".into()),
   488	            Action::DeleteVolume { .. } => Some("Deleting volume...".into()),
   489	            Action::ExtendVolume { .. } => Some("Extending volume...".into()),
   490	            Action::CreateSnapshot(_) => Some("Creating snapshot...".into()),
   491	            Action::DeleteSnapshot { .. } => Some("Deleting snapshot...".into()),
   492	            Action::CreateImage(_) => Some("Creating image...".into()),
   493	            Action::DeleteImage { .. } => Some("Deleting image...".into()),
   494	            Action::CreateProject(_) => Some("Creating project...".into()),
   495	            Action::DeleteProject { .. } => Some("Deleting project...".into()),
   496	            Action::CreateUser(_) => Some("Creating user...".into()),
   497	            Action::DeleteUser { .. } => Some("Deleting user...".into()),
   498	            Action::LiveMigrateServer { .. } => Some("Live migrating server...".into()),
   499	            Action::ColdMigrateServer { .. } => Some("Cold migrating server...".into()),
   500	            Action::ConfirmMigration { .. } => Some("Confirming migration...".into()),
   501	            Action::RevertMigration { .. } => Some("Reverting migration...".into()),
   502	            Action::EvacuateServer { .. } => Some("Evacuating server...".into()),
   503	            Action::ResizeServer { .. } => Some("Resizing server...".into()),
   504	            Action::ConfirmResize { .. } => Some("Confirming resize...".into()),
   505	            Action::RevertResize { .. } => Some("Reverting resize...".into()),
   506	            _ => None,
   507	        }
   508	    }
   509	
   510	    /// Initialize audit logger. Returns None on failure (non-fatal).
   511	    fn init_audit_logger() -> Option<AuditLogger> {
   512	        #[cfg(test)]
   513	        {
   514	            // In tests, do not create audit logger by default
   515	            None
   516	        }
   517	        #[cfg(not(test))]
   518	        {
   519	            let path = crate::config::nexttui_config_dir().join("audit.log");
   520	            match AuditLogger::new(path) {
   521	                Ok(logger) => Some(logger),
   522	                Err(e) => {
   523	                    tracing::warn!("Failed to initialize audit logger: {e}");
   524	                    None
   525	                }
   526	            }
   527	        }
   528	    }
   529	
   530	    /// Record a CUD event to the audit log. Errors are logged as warnings, never propagated.
   531	    fn record_audit(&self, event: &AppEvent) {
   532	        let Some(ref logger) = self.audit_logger else {
   533	            return;
   534	        };
   535	        let Some(entry) = self.build_audit_entry(event) else {
   536	            return;
   537	        };
   538	        if let Err(e) = logger.log_entry(entry) {
   539	            tracing::warn!("Failed to write audit log: {e}");
   540	        }
   541	        if let Err(e) = logger.rotate_if_needed() {
   542	            tracing::warn!("Failed to rotate audit log: {e}");
   543	        }
   544	    }
   545	
   546	    /// Map an AppEvent to an AuditEntry. Returns None for non-auditable events.
   547	    fn build_audit_entry(&self, event: &AppEvent) -> Option<AuditEntry> {
   548	        let cloud = self.config.active_cloud_name().to_string();
   549	        let user = self
   550	            .config
   551	            .active_cloud_config()
   552	            .auth
   553	            .username
   554	            .clone()
   555	            .unwrap_or_else(|| "unknown".to_string());
   556	        let project = self.rbac.project_id();
   557	        let timestamp = chrono::Local::now().to_rfc3339();
   558	
   559	        let (action, resource_type, resource_id, resource_name, result) = match event {
   560	            // Server CUD
   561	            AppEvent::ServerCreated(s) => ("CreateServer", "server", s.id.clone(), Some(s.name.clone()), AuditResult::Success),
   562	            AppEvent::ServerDeleted { id, name } => ("DeleteServer", "server", id.clone(), Some(name.clone()), AuditResult::Success),
   563	            AppEvent::ServerRebooted { id } => ("RebootServer", "server", id.clone(), None, AuditResult::Success),
   564	            AppEvent::ServerStarted { id } => ("StartServer", "server", id.clone(), None, AuditResult::Success),
   565	            AppEvent::ServerStopped { id } => ("StopServer", "server", id.clone(), None, AuditResult::Success),
   566	            AppEvent::ServerSnapshotCreated { server_id, .. } => ("CreateSnapshot", "server", server_id.clone(), None, AuditResult::Success),
   567	            AppEvent::ServerResized { id } => ("ResizeServer", "server", id.clone(), None, AuditResult::Success),
   568	            AppEvent::ServerLiveMigrated { id } => ("LiveMigrate", "server", id.clone(), None, AuditResult::Success),
   569	            AppEvent::ServerColdMigrated { id } => ("ColdMigrate", "server", id.clone(), None, AuditResult::Success),
   570	            AppEvent::ServerEvacuated { id } => ("Evacuate", "server", id.clone(), None, AuditResult::Success),
   571	
   572	            // Volume CUD
   573	            AppEvent::VolumeCreated(v) => ("CreateVolume", "volume", v.id.clone(), v.name.clone(), AuditResult::Success),
   574	            AppEvent::VolumeDeleted { id } => ("DeleteVolume", "volume", id.clone(), None, AuditResult::Success),
   575	            AppEvent::VolumeExtended { id } => ("ExtendVolume", "volume", id.clone(), None, AuditResult::Success),
   576	            AppEvent::VolumeAttached { volume_id, server_id: _ } => ("AttachVolume", "volume", volume_id.clone(), None, AuditResult::Success),
   577	            AppEvent::VolumeDetached { volume_id } => ("DetachVolume", "volume", volume_id.clone(), None, AuditResult::Success),
   578	            AppEvent::VolumeForceDetached { volume_id } => ("ForceDetach", "volume", volume_id.clone(), None, AuditResult::Success),
   579	            AppEvent::VolumeStateReset { volume_id } => ("ResetState", "volume", volume_id.clone(), None, AuditResult::Success),
   580	
   581	            // Floating IP CUD
   582	            AppEvent::FloatingIpCreated(f) => ("CreateFloatingIp", "floatingip", f.id.clone(), Some(f.floating_ip_address.clone()), AuditResult::Success),
   583	            AppEvent::FloatingIpDeleted { id } => ("DeleteFloatingIp", "floatingip", id.clone(), None, AuditResult::Success),
   584	            AppEvent::FloatingIpAssociated(f) => ("AssociateFloatingIp", "floatingip", f.id.clone(), Some(f.floating_ip_address.clone()), AuditResult::Success),
   585	            AppEvent::FloatingIpDisassociated(f) => ("DisassociateFloatingIp", "floatingip", f.id.clone(), Some(f.floating_ip_address.clone()), AuditResult::Success),
   586	
   587	            // Image CUD
   588	            AppEvent::ImageCreated(i) => ("CreateImage", "image", i.id.clone(), Some(i.name.clone()), AuditResult::Success),
   589	            AppEvent::ImageDeleted { id } => ("DeleteImage", "image", id.clone(), None, AuditResult::Success),
   590	
   591	            // Network CUD
   592	            AppEvent::NetworkCreated(n) => ("CreateNetwork", "network", n.id.clone(), Some(n.name.clone()), AuditResult::Success),
   593	
   594	            // Security Group CUD
   595	            AppEvent::SecurityGroupCreated(sg) => ("CreateSecurityGroup", "securitygroup", sg.id.clone(), Some(sg.name.clone()), AuditResult::Success),
   596	            AppEvent::SecurityGroupDeleted { id } => ("DeleteSecurityGroup", "securitygroup", id.clone(), None, AuditResult::Success),
   597	            AppEvent::SecurityGroupRuleCreated(r) => ("CreateSGRule", "sgRule", r.id.clone(), None, AuditResult::Success),
   598	            AppEvent::SecurityGroupRuleDeleted { rule_id } => ("DeleteSGRule", "sgRule", rule_id.clone(), None, AuditResult::Success),
   599	
   600	            // Snapshot CUD
   601	            AppEvent::SnapshotCreated(s) => ("CreateSnapshot", "snapshot", s.id.clone(), s.name.clone(), AuditResult::Success),
   602	            AppEvent::SnapshotDeleted { id } => ("DeleteSnapshot", "snapshot", id.clone(), None, AuditResult::Success),
   603	
   604	            // Keystone CUD
   605	            AppEvent::ProjectCreated(p) => ("CreateProject", "project", p.id.clone(), Some(p.name.clone()), AuditResult::Success),
   606	            AppEvent::ProjectDeleted { id } => ("DeleteProject", "project", id.clone(), None, AuditResult::Success),
   607	            AppEvent::UserCreated(u) => ("CreateUser", "user", u.id.clone(), Some(u.name.clone()), AuditResult::Success),
   608	            AppEvent::UserDeleted { id } => ("DeleteUser", "user", id.clone(), None, AuditResult::Success),
   609	
   610	            // Errors
   611	            AppEvent::ApiError { operation, message } => ("ApiError", "error", String::new(), Some(operation.clone()), AuditResult::Failed(message.clone())),
   612	            AppEvent::PermissionDenied { operation } => ("PermissionDenied", "permission", String::new(), Some(operation.clone()), AuditResult::Failed(format!("Permission denied: {operation}"))),
   613	            AppEvent::AuthFailed(msg) => ("AuthFailed", "auth", String::new(), None, AuditResult::Failed(msg.clone())),
   614	
   615	            // Compute service toggle
   616	            AppEvent::ComputeServiceToggled { hostname, enabled } => {
   617	                let detail = if *enabled { "enabled" } else { "disabled" };
   618	                return Some(AuditEntry {
   619	                    timestamp,
   620	                    cloud,
   621	                    user,
   622	                    project,
   623	                    action: "ToggleService".to_string(),
   624	                    resource_type: "service".to_string(),
   625	                    resource_id: hostname.clone(),
   626	                    resource_name: Some(hostname.clone()),
   627	                    details: Some(serde_json::json!({ "enabled": *enabled, "state": detail })),
   628	                    result: AuditResult::Success,
   629	                });
   630	            }
   631	
   632	            // Non-auditable events (data loads, system, polling, etc.)
   633	            _ => return None,
   634	        };
   635	
   636	        Some(AuditEntry {
   637	            timestamp,
   638	            cloud,
   639	            user,
   640	            project,
   641	            action: action.to_string(),
   642	            resource_type: resource_type.to_string(),
   643	            resource_id,
   644	            resource_name,
   645	            details: None,
   646	            result,
   647	        })
   648	    }
   649	
   650	    fn truncate_name(name: &str, max_len: usize) -> &str {
   651	        if name.len() <= max_len {
   652	            name
   653	        } else {
   654	            let mut end = max_len;
   655	            while end > 0 && !name.is_char_boundary(end) {
   656	                end -= 1;
   657	            }
   658	            &name[..end]
   659	        }
   660	    }
   661	
   662	    fn generate_toast(&mut self, event: &AppEvent) {
   663	        use crate::background::ToastLevel;
   664	        const MAX_NAME: usize = 64;
   665	        // Single match: (toast_message, level, operation, resource_name)
   666	        let (msg, level, operation, resource_name) = match event {
   667	            // CUD success
   668	            AppEvent::ServerCreated(s) => (format!("Server '{}' created", Self::truncate_name(&s.name, MAX_NAME)), ToastLevel::Success, "Create".into(), s.name.clone()),
   669	            AppEvent::ServerDeleted { name, .. } => (format!("Server '{}' deleted", Self::truncate_name(name, MAX_NAME)), ToastLevel::Success, "Delete".into(), name.clone()),
   670	            AppEvent::ServerRebooted { id } => (format!("Server {id} rebooted"), ToastLevel::Success, "Reboot".into(), id.clone()),
   671	            AppEvent::ServerStarted { id } => (format!("Server {id} started"), ToastLevel::Success, "Start".into(), id.clone()),
   672	            AppEvent::ServerStopped { id } => (format!("Server {id} stopped"), ToastLevel::Success, "Stop".into(), id.clone()),
   673	            AppEvent::ServerSnapshotCreated { server_id, .. } => (format!("Snapshot created for {server_id}"), ToastLevel::Success, "Snapshot".into(), server_id.clone()),
   674	            AppEvent::FlavorCreated(f) => (format!("Flavor '{}' created", Self::truncate_name(&f.name, MAX_NAME)), ToastLevel::Success, "Create".into(), f.name.clone()),
   675	            AppEvent::FlavorDeleted { id } => (format!("Flavor {id} deleted"), ToastLevel::Success, "Delete".into(), id.clone()),
   676	            AppEvent::NetworkCreated(n) => (format!("Network '{}' created", Self::truncate_name(&n.name, MAX_NAME)), ToastLevel::Success, "Create".into(), n.name.clone()),
   677	            AppEvent::SecurityGroupCreated(sg) => (format!("Security group '{}' created", Self::truncate_name(&sg.name, MAX_NAME)), ToastLevel::Success, "Create".into(), sg.name.clone()),
   678	            AppEvent::SecurityGroupDeleted { id } => (format!("Security group {id} deleted"), ToastLevel::Success, "Delete".into(), id.clone()),
   679	            AppEvent::SecurityGroupRuleCreated(_) => ("Security group rule created".into(), ToastLevel::Success, "Create".into(), "SG Rule".into()),
   680	            AppEvent::SecurityGroupRuleDeleted { .. } => ("Security group rule deleted".into(), ToastLevel::Success, "Delete".into(), "SG Rule".into()),
   681	            AppEvent::VolumeCreated(v) => (format!("Volume '{}' created", Self::truncate_name(v.name.as_deref().unwrap_or(&v.id), MAX_NAME)), ToastLevel::Success, "Create".into(), v.name.as_deref().unwrap_or(&v.id).to_string()),
   682	            AppEvent::VolumeDeleted { id } => (format!("Volume {id} deleted"), ToastLevel::Success, "Delete".into(), id.clone()),
   683	            AppEvent::VolumeExtended { id } => (format!("Volume {id} extended"), ToastLevel::Success, "Extend".into(), id.clone()),
   684	            AppEvent::SnapshotCreated(s) => (format!("Snapshot '{}' created", Self::truncate_name(s.name.as_deref().unwrap_or(&s.id), MAX_NAME)), ToastLevel::Success, "Create".into(), s.name.as_deref().unwrap_or(&s.id).to_string()),
   685	            AppEvent::SnapshotDeleted { id } => (format!("Snapshot {id} deleted"), ToastLevel::Success, "Delete".into(), id.clone()),
   686	            AppEvent::ImageCreated(i) => (format!("Image '{}' created", Self::truncate_name(&i.name, MAX_NAME)), ToastLevel::Success, "Create".into(), i.name.clone()),
   687	            AppEvent::ImageDeleted { id } => (format!("Image {id} deleted"), ToastLevel::Success, "Delete".into(), id.clone()),
   688	            AppEvent::FloatingIpCreated(f) => (format!("Floating IP '{}' created", Self::truncate_name(&f.floating_ip_address, MAX_NAME)), ToastLevel::Success, "Create".into(), f.floating_ip_address.clone()),
   689	            AppEvent::FloatingIpDeleted { id } => (format!("Floating IP {id} deleted"), ToastLevel::Success, "Delete".into(), id.clone()),
   690	            AppEvent::ProjectCreated(p) => (format!("Project '{}' created", Self::truncate_name(&p.name, MAX_NAME)), ToastLevel::Success, "Create".into(), p.name.clone()),
   691	            AppEvent::ProjectDeleted { id } => (format!("Project {id} deleted"), ToastLevel::Success, "Delete".into(), id.clone()),
   692	            AppEvent::UserCreated(u) => (format!("User '{}' created", Self::truncate_name(&u.name, MAX_NAME)), ToastLevel::Success, "Create".into(), u.name.clone()),
   693	            AppEvent::UserDeleted { id } => (format!("User {id} deleted"), ToastLevel::Success, "Delete".into(), id.clone()),
   694	            // Migration
   695	            AppEvent::ServerLiveMigrated { id } => (format!("Server {id} live migrated"), ToastLevel::Success, "LiveMigrate".into(), id.clone()),
   696	            AppEvent::ServerColdMigrated { id } => (format!("Server {id} cold migrated — confirm(Y) or revert(N)"), ToastLevel::Success, "ColdMigrate".into(), id.clone()),
   697	            AppEvent::MigrationConfirmed { id } => (format!("Migration confirmed for {id}"), ToastLevel::Success, "ConfirmMigration".into(), id.clone()),
   698	            AppEvent::MigrationReverted { id } => (format!("Migration reverted for {id}"), ToastLevel::Success, "RevertMigration".into(), id.clone()),
   699	            AppEvent::ServerEvacuated { id } => (format!("Server {id} evacuated"), ToastLevel::Success, "Evacuate".into(), id.clone()),
   700	            // Resize
   701	            AppEvent::ServerResized { id } => (format!("Server {id} resized — confirm(Y) or revert(N)"), ToastLevel::Success, "Resize".into(), id.clone()),
   702	            AppEvent::ResizeConfirmed { id } => (format!("Resize confirmed for {id}"), ToastLevel::Success, "ConfirmResize".into(), id.clone()),
   703	            AppEvent::ResizeReverted { id } => (format!("Resize reverted for {id}"), ToastLevel::Success, "RevertResize".into(), id.clone()),
   704	            // Volume Attach/Detach
   705	            AppEvent::VolumeAttached { volume_id, .. } => (format!("Volume {volume_id} attached successfully"), ToastLevel::Success, "AttachVolume".into(), volume_id.clone()),
   706	            AppEvent::VolumeDetached { volume_id } => (format!("Volume {volume_id} detached successfully"), ToastLevel::Success, "DetachVolume".into(), volume_id.clone()),
   707	            AppEvent::VolumeForceDetached { volume_id } => (format!("Volume {volume_id} force-detached (verify data integrity)"), ToastLevel::Success, "ForceDetachVolume".into(), volume_id.clone()),
   708	            AppEvent::VolumeStateReset { volume_id } => (format!("Volume {volume_id} state reset to available"), ToastLevel::Success, "ResetVolumeState".into(), volume_id.clone()),
   709	            // Floating IP Associate/Disassociate
   710	            AppEvent::FloatingIpAssociated(f) => (format!("Floating IP {} associated successfully", f.floating_ip_address), ToastLevel::Success, "AssociateFloatingIp".into(), f.floating_ip_address.clone()),
   711	            AppEvent::FloatingIpDisassociated(f) => (format!("FIP {} disassociated. Press 'a' to re-associate.", f.floating_ip_address), ToastLevel::Success, "DisassociateFloatingIp".into(), f.floating_ip_address.clone()),
   712	            // Errors
   713	            AppEvent::ApiError { operation, message } => (format!("{operation} failed: {message}"), ToastLevel::Error, operation.clone(), String::new()),
   714	            AppEvent::AuthFailed(msg) => (format!("Auth failed: {msg}"), ToastLevel::Error, "Auth".into(), String::new()),
   715	            AppEvent::PermissionDenied { operation } => (format!("Permission denied: {operation}"), ToastLevel::Error, operation.clone(), String::new()),
   716	            // Data loaded / system events — no toast or activity log
   717	            _ => return,
   718	        };
   719	        let success = !matches!(level, ToastLevel::Error);
   720	        self.activity_log.push(crate::ui::activity_log::ActivityEntry {
   721	            timestamp: std::time::Instant::now(),
   722	            operation,
   723	            resource_name,
   724	            success,
   725	            message: if success { String::new() } else { msg.clone() },
   726	            read: false,
   727	        });
   728	        self.background_tracker.add_toast(msg, level);
   729	    }
   730	
   731	    /// Tick handler: toast expiry, background tracker GC, auto-refresh.
   732	    pub fn on_tick(&mut self) {
   733	        self.background_tracker.expire_toasts();
   734	        self.background_tracker.gc_old_entries();
   735	
   736	        // Auto-refresh: skip when user is interacting
   737	        if self.input_mode != InputMode::Normal {
   738	            return;
   739	        }
   740	        let route = self.router.current();
   741	        if let Some(component) = self.components.get(&route) {
   742	            if component.is_modal() {
   743	                return;
   744	            }
   745	            let has_transitional = component.has_transitional_resources();
   746	            self.refresh_scheduler.set_fast(has_transitional);
   747	            if self.refresh_scheduler.tick() && let Some(action) = component.refresh_action() {
   748	                let _ = self.action_tx.send(action);
   749	            }
   750	        }
   751	    }
   752	
   753	    /// Render the UI.
   754	    pub fn render(&self, frame: &mut Frame) {
   755	        let areas = self.layout.calculate(frame.area());
   756	
   757	        // Header
   758	        let route_label = self.route_label(&self.router.current());
   759	        let cloud_config = self.config.active_cloud_config();
   760	        let user_name = cloud_config.auth.username.clone().unwrap_or_default();
   761	        let cloud_name = self.config.active_cloud_name().to_string();
   762	        let region = cloud_config
   763	            .region_name.as_deref().unwrap_or("default").to_string();
   764	        self.header.render(frame, areas.header, &HeaderContext {
   765	            user_name,
   766	            cloud_name,
   767	            region,
   768	            all_tenants: self.all_tenants.load(Ordering::Relaxed),
   769	        });
   770	
   771	        // Sidebar
   772	        if let Some(sidebar_area) = areas.sidebar {
   773	            let sidebar_focused = self.focus == FocusPane::Sidebar;
   774	            self.sidebar.render(frame, sidebar_area, self.rbac.is_admin(), &self.router.current(), sidebar_focused);
   775	        }
   776	
   777	        // Content
   778	        if let Some(component) = self.components.get(&self.router.current()) {
   779	            if component.layout_hint() == LayoutHint::FullWidth {
   780	                // FullWidth modules manage their own borders/layout
   781	                component.render(frame, areas.content);
   782	            } else {
   783	                let content_focused = self.focus == FocusPane::Content;
   784	                let content_border_style = if content_focused {
   785	                    Theme::focus_border()
   786	                } else {
   787	                    Theme::unfocus_border()
   788	                };
   789	                let all_tenants = self.all_tenants.load(Ordering::Relaxed);
   790	                let display_label = component.content_title()
   791	                    .unwrap_or_else(|| route_label.to_string());
   792	                let title = theme::panel_title_line(&display_label, content_focused, all_tenants);
   793	                let content_block = Block::default()
   794	                    .title(title)
   795	                    .borders(Borders::ALL)
   796	                    .border_type(BorderType::Rounded)
   797	                    .border_style(content_border_style);
   798	                let content_inner = content_block.inner(areas.content);
   799	                frame.render_widget(content_block, areas.content);
   800	                component.render(frame, content_inner);
   801	            }
   802	        }
   803	
   804	        // Activity log popup overlay
   805	        if self.show_activity_log {
   806	            self.activity_popup
   807	                .render(frame, areas.content, self.activity_log.entries());
   808	        }
   809	
   810	        // Status bar — context_hints from component help_hint or defaults
   811	        let component_hint = self.components
   812	            .get(&self.router.current())
   813	            .map(|c| c.help_hint())
   814	            .unwrap_or("");
   815	        let context_hints: Vec<(String, String)> = if component_hint.is_empty() {
   816	            vec![
   817	                ("j/k".into(), "이동".into()),
   818	                ("Enter".into(), "선택".into()),
   819	                ("q".into(), "종료".into()),
   820	            ]
   821	        } else {
   822	            component_hint
   823	                .split(' ')
   824	                .filter_map(|part| {
   825	                    part.split_once(':').map(|(k, v)| (k.to_string(), v.to_string()))
   826	                })
   827	                .collect()
   828	        };
   829	        let info = StatusInfo {
   830	            panel_name: route_label.to_string(),
   831	            item_count: None,
   832	            selected_index: None,
   833	            context_hints,
   834	            error_badge_count: self.activity_log.unread_error_count(),
   835	        };
   836	        // Toast — render in dedicated toast_bar area
   837	        let active_toasts = self.background_tracker.active_toasts();
   838	        if let Some(t) = active_toasts.first() {
   839	            let toast_msg = ToastMessage {
   840	                text: t.message.clone(),
   841	                severity: ToastSeverity::from(t.level),
   842	                resource_id: None,
   843	            };
   844	            toast_msg.render(frame, areas.toast_bar);
   845	        }
   846	
   847	        self.status_bar.render(frame, areas.status_bar, &info);
   848	    }
   849	
   850	    pub fn router(&self) -> &Router {
   851	        &self.router
   852	    }
   853	
   854	    pub fn router_mut(&mut self) -> &mut Router {
   855	        &mut self.router
   856	    }
   857	
   858	    pub fn background_tracker(&self) -> &BackgroundTracker {
   859	        &self.background_tracker
   860	    }
   861	
   862	    pub fn background_tracker_mut(&mut self) -> &mut BackgroundTracker {
   863	        &mut self.background_tracker
   864	    }
   865	
   866	    pub fn config(&self) -> &Config {
   867	        &self.config
   868	    }
   869	
   870	    pub fn action_tx(&self) -> &mpsc::UnboundedSender<Action> {
   871	        &self.action_tx
   872	    }
   873	}
   874	
   875	#[cfg(test)]
   876	mod tests {
   877	    use super::*;
   878	    use crossterm::event::{KeyCode, KeyEvent, KeyEventKind, KeyEventState, KeyModifiers};
   879	    use ratatui::layout::Rect;
   880	    use std::time::Instant;
   881	
   882	    fn make_key(code: KeyCode) -> KeyEvent {
   883	        KeyEvent {
   884	            code,
   885	            modifiers: KeyModifiers::NONE,
   886	            kind: KeyEventKind::Press,
   887	            state: KeyEventState::NONE,
   888	        }
   889	    }
   890	
   891	    struct MockComponent {
   892	        last_key: Option<KeyCode>,
   893	        last_event_received: bool,
   894	    }
   895	
   896	    impl MockComponent {
   897	        fn new() -> Self {
   898	            Self {
   899	                last_key: None,
   900	                last_event_received: false,

exec
/bin/zsh -lc "nl -ba src/worker.rs | sed -n '360,980p'" in /Users/jay.ahn/projects/infra/nexttui/.worktrees/runtime-context-switch
 succeeded in 0ms:
   360	                        Some(AppEvent::MigrationProgressLoaded { server_id, migration })
   361	                    } else {
   362	                        None
   363	                    }
   364	                }
   365	                Err(e) => Some(api_error("FetchMigrationProgress", e)),
   366	            }
   367	        }
   368	
   369	        // -- Nova: Usage ---------------------------------------------------
   370	        Action::FetchUsage { start, end } => {
   371	            use crate::port::error::ApiError;
   372	            let start_dt = start.parse::<DateTime<Utc>>().map_err(|e| ApiError::Parse(e.to_string()));
   373	            let end_dt = end.parse::<DateTime<Utc>>().map_err(|e| ApiError::Parse(e.to_string()));
   374	            match (start_dt, end_dt) {
   375	                (Ok(s), Ok(e)) => {
   376	                    match registry.nova.list_all_tenant_usage(s, e).await {
   377	                        Ok(usages) => Some(AppEvent::UsageLoaded(usages)),
   378	                        Err(e) => Some(api_error("FetchUsage", e)),
   379	                    }
   380	                }
   381	                (Err(e), _) | (_, Err(e)) => Some(api_error("FetchUsage", e)),
   382	            }
   383	        }
   384	
   385	        // -- Nova: Flavors --------------------------------------------------
   386	        Action::FetchFlavors => {
   387	            match registry.nova.list_flavors(&default_pagination).await {
   388	                Ok(resp) => Some(AppEvent::FlavorsLoaded(resp.items)),
   389	                Err(e) => Some(api_error("FetchFlavors", e)),
   390	            }
   391	        }
   392	        Action::CreateFlavor(params) => {
   393	            match registry.nova.create_flavor(&params).await {
   394	                Ok(flavor) => Some(AppEvent::FlavorCreated(flavor)),
   395	                Err(e) => Some(api_error("CreateFlavor", e)),
   396	            }
   397	        }
   398	        Action::DeleteFlavor { id } => {
   399	            match registry.nova.delete_flavor(&id).await {
   400	                Ok(()) => Some(AppEvent::FlavorDeleted { id }),
   401	                Err(e) => Some(api_error("DeleteFlavor", e)),
   402	            }
   403	        }
   404	
   405	        // -- Nova: Admin ----------------------------------------------------
   406	        Action::FetchAggregates => {
   407	            match registry.nova.list_aggregates().await {
   408	                Ok(aggs) => Some(AppEvent::AggregatesLoaded(aggs)),
   409	                Err(e) => Some(api_error("FetchAggregates", e)),
   410	            }
   411	        }
   412	        Action::FetchComputeServices => {
   413	            match registry.nova.list_compute_services().await {
   414	                Ok(svcs) => Some(AppEvent::ComputeServicesLoaded(svcs)),
   415	                Err(e) => Some(api_error("FetchComputeServices", e)),
   416	            }
   417	        }
   418	        Action::FetchHypervisors => {
   419	            match registry.nova.list_hypervisors().await {
   420	                Ok(hvs) => Some(AppEvent::HypervisorsLoaded(hvs)),
   421	                Err(e) => Some(api_error("FetchHypervisors", e)),
   422	            }
   423	        }
   424	
   425	        // -- Neutron: Networks ----------------------------------------------
   426	        Action::FetchNetworks => {
   427	            match registry
   428	                .neutron
   429	                .list_networks(&NetworkListFilter { all_tenants: at }, &default_pagination)
   430	                .await
   431	            {
   432	                Ok(resp) => Some(AppEvent::NetworksLoaded(resp.items)),
   433	                Err(e) => Some(api_error("FetchNetworks", e)),
   434	            }
   435	        }
   436	        Action::CreateNetwork(params) => {
   437	            match registry.neutron.create_network(&params).await {
   438	                Ok(net) => Some(AppEvent::NetworkCreated(net)),
   439	                Err(e) => Some(api_error("CreateNetwork", e)),
   440	            }
   441	        }
   442	        Action::FetchSubnets { network_id } => {
   443	            match registry
   444	                .neutron
   445	                .list_subnets(Some(&network_id))
   446	                .await
   447	            {
   448	                Ok(subnets) => Some(AppEvent::SubnetsLoaded {
   449	                    network_id,
   450	                    subnets,
   451	                }),
   452	                Err(e) => Some(api_error("FetchSubnets", e)),
   453	            }
   454	        }
   455	
   456	        // -- Neutron: Security Groups ---------------------------------------
   457	        Action::FetchSecurityGroups => {
   458	            match registry
   459	                .neutron
   460	                .list_security_groups(&SecurityGroupListFilter { all_tenants: at }, &default_pagination)
   461	                .await
   462	            {
   463	                Ok(resp) => Some(AppEvent::SecurityGroupsLoaded(resp.items)),
   464	                Err(e) => Some(api_error("FetchSecurityGroups", e)),
   465	            }
   466	        }
   467	        Action::CreateSecurityGroup(params) => {
   468	            match registry.neutron.create_security_group(&params).await {
   469	                Ok(sg) => Some(AppEvent::SecurityGroupCreated(sg)),
   470	                Err(e) => Some(api_error("CreateSecurityGroup", e)),
   471	            }
   472	        }
   473	        Action::DeleteSecurityGroup { id } => {
   474	            match registry.neutron.delete_security_group(&id).await {
   475	                Ok(()) => Some(AppEvent::SecurityGroupDeleted { id }),
   476	                Err(e) => Some(api_error("DeleteSecurityGroup", e)),
   477	            }
   478	        }
   479	        Action::CreateSecurityGroupRule(params) => {
   480	            match registry.neutron.create_security_group_rule(&params).await {
   481	                Ok(rule) => Some(AppEvent::SecurityGroupRuleCreated(rule)),
   482	                Err(e) => Some(api_error("CreateSecurityGroupRule", e)),
   483	            }
   484	        }
   485	        Action::DeleteSecurityGroupRule { rule_id } => {
   486	            match registry
   487	                .neutron
   488	                .delete_security_group_rule(&rule_id)
   489	                .await
   490	            {
   491	                Ok(()) => Some(AppEvent::SecurityGroupRuleDeleted { rule_id }),
   492	                Err(e) => Some(api_error("DeleteSecurityGroupRule", e)),
   493	            }
   494	        }
   495	
   496	        // -- Neutron: Floating IPs ------------------------------------------
   497	        Action::FetchFloatingIps => {
   498	            match registry
   499	                .neutron
   500	                .list_floating_ips(&FloatingIpListFilter { all_tenants: at }, &default_pagination)
   501	                .await
   502	            {
   503	                Ok(resp) => Some(AppEvent::FloatingIpsLoaded(resp.items)),
   504	                Err(e) => Some(api_error("FetchFloatingIps", e)),
   505	            }
   506	        }
   507	        Action::CreateFloatingIp { network_id } => {
   508	            match registry
   509	                .neutron
   510	                .create_floating_ip(&FloatingIpCreateParams {
   511	                    floating_network_id: network_id,
   512	                    port_id: None,
   513	                    fixed_ip_address: None,
   514	                })
   515	                .await
   516	            {
   517	                Ok(fip) => Some(AppEvent::FloatingIpCreated(fip)),
   518	                Err(e) => Some(api_error("CreateFloatingIp", e)),
   519	            }
   520	        }
   521	        Action::DeleteFloatingIp { id } => {
   522	            match registry.neutron.delete_floating_ip(&id).await {
   523	                Ok(()) => Some(AppEvent::FloatingIpDeleted { id }),
   524	                Err(e) => Some(api_error("DeleteFloatingIp", e)),
   525	            }
   526	        }
   527	        Action::FetchAgents => {
   528	            match registry.neutron.list_network_agents().await {
   529	                Ok(agents) => Some(AppEvent::AgentsLoaded(agents)),
   530	                Err(e) => Some(api_error("FetchAgents", e)),
   531	            }
   532	        }
   533	
   534	        // -- Cinder: Volumes ------------------------------------------------
   535	        Action::FetchVolumes => {
   536	            match registry
   537	                .cinder
   538	                .list_volumes(&VolumeListFilter { all_tenants: at, ..Default::default() }, &default_pagination)
   539	                .await
   540	            {
   541	                Ok(resp) => Some(AppEvent::VolumesLoaded(resp.items)),
   542	                Err(e) => Some(api_error("FetchVolumes", e)),
   543	            }
   544	        }
   545	        Action::CreateVolume(params) => {
   546	            match registry.cinder.create_volume(&params).await {
   547	                Ok(vol) => Some(AppEvent::VolumeCreated(vol)),
   548	                Err(e) => Some(api_error("CreateVolume", e)),
   549	            }
   550	        }
   551	        Action::DeleteVolume { id, force } => {
   552	            let result = if force {
   553	                registry.cinder.force_delete_volume(&id).await
   554	            } else {
   555	                registry.cinder.delete_volume(&id).await
   556	            };
   557	            match result {
   558	                Ok(()) => Some(AppEvent::VolumeDeleted { id }),
   559	                Err(e) => Some(api_error("DeleteVolume", e)),
   560	            }
   561	        }
   562	        Action::ExtendVolume { id, new_size } => {
   563	            match registry.cinder.extend_volume(&id, new_size).await {
   564	                Ok(()) => Some(AppEvent::VolumeExtended { id }),
   565	                Err(e) => Some(api_error("ExtendVolume", e)),
   566	            }
   567	        }
   568	
   569	        // -- Cinder: Snapshots ----------------------------------------------
   570	        Action::FetchSnapshots => {
   571	            match registry
   572	                .cinder
   573	                .list_snapshots(&SnapshotListFilter { all_tenants: at }, &default_pagination)
   574	                .await
   575	            {
   576	                Ok(resp) => Some(AppEvent::SnapshotsLoaded(resp.items)),
   577	                Err(e) => Some(api_error("FetchSnapshots", e)),
   578	            }
   579	        }
   580	        Action::CreateSnapshot(params) => {
   581	            match registry.cinder.create_snapshot(&params).await {
   582	                Ok(snap) => Some(AppEvent::SnapshotCreated(snap)),
   583	                Err(e) => Some(api_error("CreateSnapshot", e)),
   584	            }
   585	        }
   586	        Action::DeleteSnapshot { id } => {
   587	            match registry.cinder.delete_snapshot(&id).await {
   588	                Ok(()) => Some(AppEvent::SnapshotDeleted { id }),
   589	                Err(e) => Some(api_error("DeleteSnapshot", e)),
   590	            }
   591	        }
   592	
   593	        // -- Glance: Images -------------------------------------------------
   594	        Action::FetchImages => {
   595	            match registry
   596	                .glance
   597	                .list_images(&ImageListFilter { all_tenants: at, ..Default::default() }, &default_pagination)
   598	                .await
   599	            {
   600	                Ok(resp) => Some(AppEvent::ImagesLoaded(resp.items)),
   601	                Err(e) => Some(api_error("FetchImages", e)),
   602	            }
   603	        }
   604	        Action::CreateImage(params) => {
   605	            match registry.glance.create_image(&params).await {
   606	                Ok(img) => Some(AppEvent::ImageCreated(img)),
   607	                Err(e) => Some(api_error("CreateImage", e)),
   608	            }
   609	        }
   610	        Action::DeleteImage { id } => {
   611	            match registry.glance.delete_image(&id).await {
   612	                Ok(()) => Some(AppEvent::ImageDeleted { id }),
   613	                Err(e) => Some(api_error("DeleteImage", e)),
   614	            }
   615	        }
   616	
   617	        // -- Keystone: Projects ---------------------------------------------
   618	        Action::FetchProjects => {
   619	            match registry
   620	                .keystone
   621	                .list_projects(&default_pagination)
   622	                .await
   623	            {
   624	                Ok(resp) => Some(AppEvent::ProjectsLoaded(resp.items)),
   625	                Err(e) => Some(api_error("FetchProjects", e)),
   626	            }
   627	        }
   628	        Action::CreateProject(params) => {
   629	            match registry.keystone.create_project(&params).await {
   630	                Ok(proj) => Some(AppEvent::ProjectCreated(proj)),
   631	                Err(e) => Some(api_error("CreateProject", e)),
   632	            }
   633	        }
   634	        Action::DeleteProject { id } => {
   635	            match registry.keystone.delete_project(&id).await {
   636	                Ok(()) => Some(AppEvent::ProjectDeleted { id }),
   637	                Err(e) => Some(api_error("DeleteProject", e)),
   638	            }
   639	        }
   640	
   641	        // -- Keystone: Users ------------------------------------------------
   642	        Action::FetchUsers => {
   643	            match registry.keystone.list_users(&default_pagination).await {
   644	                Ok(resp) => Some(AppEvent::UsersLoaded(resp.items)),
   645	                Err(e) => Some(api_error("FetchUsers", e)),
   646	            }
   647	        }
   648	        Action::CreateUser(params) => {
   649	            match registry.keystone.create_user(&params).await {
   650	                Ok(user) => Some(AppEvent::UserCreated(user)),
   651	                Err(e) => Some(api_error("CreateUser", e)),
   652	            }
   653	        }
   654	        Action::DeleteUser { id } => {
   655	            match registry.keystone.delete_user(&id).await {
   656	                Ok(()) => Some(AppEvent::UserDeleted { id }),
   657	                Err(e) => Some(api_error("DeleteUser", e)),
   658	            }
   659	        }
   660	
   661	        // -- Nova: Volume Attach/Detach (via Nova os-volume_attachments API) --
   662	        Action::AttachVolume { volume_id, server_id, device } => {
   663	            match registry.nova.attach_volume(&server_id, &volume_id, device.as_deref()).await {
   664	                Ok(()) => Some(AppEvent::VolumeAttached { volume_id, server_id }),
   665	                Err(e) => Some(api_error("AttachVolume", e)),
   666	            }
   667	        }
   668	        Action::DetachVolume { volume_id, server_id, .. } => {
   669	            match registry.nova.detach_volume(&server_id, &volume_id).await {
   670	                Ok(()) => Some(AppEvent::VolumeDetached { volume_id }),
   671	                Err(e) => Some(api_error("DetachVolume", e)),
   672	            }
   673	        }
   674	        Action::ForceDetachVolume { volume_id, attachment_id, .. } => {
   675	            match registry.cinder.force_detach_volume(&volume_id, &attachment_id).await {
   676	                Ok(()) => Some(AppEvent::VolumeForceDetached { volume_id }),
   677	                Err(e) => Some(api_error("ForceDetachVolume", e)),
   678	            }
   679	        }
   680	        Action::ForceResetVolumeState { volume_id, target_state } => {
   681	            match registry.cinder.force_set_volume_state(&volume_id, &target_state).await {
   682	                Ok(()) => Some(AppEvent::VolumeStateReset { volume_id }),
   683	                Err(e) => Some(api_error("ForceResetVolumeState", e)),
   684	            }
   685	        }
   686	
   687	        // -- Neutron: Floating IP Associate/Disassociate --------------------
   688	        Action::AssociateFloatingIp { fip_id, port_id } => {
   689	            match registry.neutron.associate_floating_ip(&fip_id, &port_id).await {
   690	                Ok(fip) => Some(AppEvent::FloatingIpAssociated(fip)),
   691	                Err(e) => Some(api_error("AssociateFloatingIp", e)),
   692	            }
   693	        }
   694	        Action::DisassociateFloatingIp { fip_id } => {
   695	            match registry.neutron.disassociate_floating_ip(&fip_id).await {
   696	                Ok(fip) => Some(AppEvent::FloatingIpDisassociated(fip)),
   697	                Err(e) => Some(api_error("DisassociateFloatingIp", e)),
   698	            }
   699	        }
   700	
   701	        // -- Neutron: Ports -------------------------------------------------
   702	        Action::FetchPorts { server_id } => {
   703	            match registry.neutron.list_ports(&server_id).await {
   704	                Ok(ports) => Some(AppEvent::PortsLoaded { server_id, ports }),
   705	                Err(e) => Some(api_error("FetchPorts", e)),
   706	            }
   707	        }
   708	
   709	        // -- UI-only actions (handled by App::dispatch_action, not worker) --
   710	        Action::Navigate(_)
   711	        | Action::Back
   712	        | Action::FocusSidebar
   713	        | Action::SelectResource { .. }
   714	        | Action::NavigateToResource { .. }
   715	        | Action::EnterFormMode
   716	        | Action::ExitFormMode
   717	        | Action::ToggleAllTenants
   718	        | Action::ShowToast { .. }
   719	        | Action::Quit => None,
   720	
   721	        // -- System ---------------------------------------------------------
   722	        Action::RefreshAll => {
   723	            // RefreshAll is not handled by the worker — App::dispatch_action should
   724	            // expand it into individual Fetch actions. If it reaches here, ignore.
   725	            None
   726	        }
   727	
   728	        Action::SwitchCloud(_cloud_name) => {
   729	            // Phase 2: switch auth provider and re-create adapters
   730	            None
   731	        }
   732	    }
   733	}
   734	
   735	/// Determine if an action should trigger migration-progress polling after success.
   736	fn poll_migration_server_id(action: &Action) -> Option<String> {
   737	    match action {
   738	        Action::LiveMigrateServer { id, .. }
   739	        | Action::ColdMigrateServer { id, .. } => Some(id.clone()),
   740	        _ => None,
   741	    }
   742	}
   743	
   744	/// Determine if an action should trigger server-status polling after success.
   745	fn poll_server_id_for_status(action: &Action) -> Option<String> {
   746	    match action {
   747	        Action::ResizeServer { id, .. }
   748	        | Action::ConfirmResize { id }
   749	        | Action::RevertResize { id }
   750	        | Action::RebootServer { id, .. }
   751	        | Action::StartServer { id }
   752	        | Action::StopServer { id } => Some(id.clone()),
   753	        _ => None,
   754	    }
   755	}
   756	
   757	/// Return a dedup key for Fetch-type actions (parameterless list fetches).
   758	/// Returns None for mutations, parameterized fetches, and non-fetch actions.
   759	fn fetch_dedup_key(action: &Action) -> Option<&'static str> {
   760	    match action {
   761	        Action::FetchServers => Some("FetchServers"),
   762	        Action::FetchVolumes => Some("FetchVolumes"),
   763	        Action::FetchNetworks => Some("FetchNetworks"),
   764	        Action::FetchImages => Some("FetchImages"),
   765	        Action::FetchFlavors => Some("FetchFlavors"),
   766	        Action::FetchSnapshots => Some("FetchSnapshots"),
   767	        Action::FetchFloatingIps => Some("FetchFloatingIps"),
   768	        Action::FetchSecurityGroups => Some("FetchSecurityGroups"),
   769	        Action::FetchProjects => Some("FetchProjects"),
   770	        Action::FetchUsers => Some("FetchUsers"),
   771	        Action::FetchAggregates => Some("FetchAggregates"),
   772	        Action::FetchComputeServices => Some("FetchComputeServices"),
   773	        Action::FetchHypervisors => Some("FetchHypervisors"),
   774	        Action::FetchAgents => Some("FetchAgents"),
   775	        _ => None,
   776	    }
   777	}
   778	
   779	use crate::models::common::is_terminal_server_status;
   780	
   781	/// Poll server status every 2 seconds until it reaches a terminal state.
   782	async fn poll_server_status(
   783	    registry: &AdapterRegistry,
   784	    event_tx: &mpsc::UnboundedSender<AppEvent>,
   785	    server_id: &str,
   786	) {
   787	    const POLL_INTERVAL: std::time::Duration = std::time::Duration::from_secs(2);
   788	    const MAX_POLLS: usize = 60; // 2 minutes max
   789	
   790	    for _ in 0..MAX_POLLS {
   791	        tokio::time::sleep(POLL_INTERVAL).await;
   792	        match registry.nova.get_server(server_id).await {
   793	            Ok(server) => {
   794	                let done = is_terminal_server_status(&server.status);
   795	                let _ = event_tx.send(AppEvent::ServerStatusPolled {
   796	                    server: server.clone(),
   797	                });
   798	                if done {
   799	                    return;
   800	                }
   801	            }
   802	            Err(_) => return,
   803	        }
   804	    }
   805	}
   806	
   807	/// Poll migration progress every 2 seconds until completed or error.
   808	async fn poll_migration_progress(
   809	    registry: &AdapterRegistry,
   810	    event_tx: &mpsc::UnboundedSender<AppEvent>,
   811	    server_id: &str,
   812	) {
   813	    const POLL_INTERVAL: std::time::Duration = std::time::Duration::from_secs(2);
   814	    const MAX_POLLS: usize = 150; // 5 minutes max
   815	
   816	    for _ in 0..MAX_POLLS {
   817	        tokio::time::sleep(POLL_INTERVAL).await;
   818	        match registry.nova.list_server_migrations(server_id).await {
   819	            Ok(migrations) => {
   820	                if let Some(migration) = migrations.into_iter().last() {
   821	                    let done = matches!(
   822	                        migration.status.as_str(),
   823	                        "completed" | "confirmed" | "error" | "cancelled"
   824	                    );
   825	                    let _ = event_tx.send(AppEvent::MigrationProgressLoaded {
   826	                        server_id: server_id.to_string(),
   827	                        migration,
   828	                    });
   829	                    if done {
   830	                        break;
   831	                    }
   832	                } else {
   833	                    // No migrations found — migration may have completed before first poll
   834	                    break;
   835	                }
   836	            }
   837	            Err(_) => {
   838	                // API error (e.g. 404 after migration completed) — stop polling
   839	                break;
   840	            }
   841	        }
   842	    }
   843	    // Always notify app to refresh server list when polling ends
   844	    let _ = event_tx.send(AppEvent::MigrationPollingStopped {
   845	        server_id: server_id.to_string(),
   846	    });
   847	}
   848	
   849	fn api_error(operation: &str, error: crate::port::error::ApiError) -> AppEvent {
   850	    tracing::error!(operation, error = %error, "API call failed");
   851	    AppEvent::ApiError {
   852	        operation: operation.to_string(),
   853	        message: error.to_string(),
   854	    }
   855	}
   856	
   857	#[cfg(test)]
   858	mod tests {
   859	    use super::*;
   860	
   861	    #[test]
   862	    fn test_action_to_kind_cud_actions() {
   863	        use crate::infra::rbac::ActionKind;
   864	        // Create actions should map to ActionKind::Create
   865	        assert_eq!(
   866	            action_to_kind(&Action::CreateServer(crate::port::types::ServerCreateParams {
   867	                name: "t".into(), image_id: "i".into(), flavor_id: "f".into(),
   868	                networks: vec![], security_groups: None, key_name: None, availability_zone: None,
   869	            })),
   870	            Some(ActionKind::Create),
   871	        );
   872	        // Delete actions should map to ActionKind::Delete
   873	        assert_eq!(
   874	            action_to_kind(&Action::DeleteServer { id: "s1".into(), name: "web".into() }),
   875	            Some(ActionKind::Delete),
   876	        );
   877	        // ForceDelete
   878	        assert_eq!(
   879	            action_to_kind(&Action::DeleteVolume { id: "v1".into(), force: true }),
   880	            Some(ActionKind::ForceDelete),
   881	        );
   882	        // Fetch actions should return None (no guard needed)
   883	        assert_eq!(action_to_kind(&Action::FetchServers), None);
   884	
   885	        // Migration actions should map to Migrate
   886	        assert_eq!(
   887	            action_to_kind(&Action::LiveMigrateServer { id: "s1".into(), host: None }),
   888	            Some(ActionKind::Migrate),
   889	        );
   890	        assert_eq!(
   891	            action_to_kind(&Action::ColdMigrateServer { id: "s1".into() }),
   892	            Some(ActionKind::Migrate),
   893	        );
   894	        assert_eq!(
   895	            action_to_kind(&Action::ConfirmMigration { id: "s1".into() }),
   896	            Some(ActionKind::Migrate),
   897	        );
   898	        assert_eq!(
   899	            action_to_kind(&Action::RevertMigration { id: "s1".into() }),
   900	            Some(ActionKind::Migrate),
   901	        );
   902	        // Evacuate should map to Evacuate
   903	        assert_eq!(
   904	            action_to_kind(&Action::EvacuateServer { id: "s1".into(), params: EvacuateParams::default() }),
   905	            Some(ActionKind::Evacuate),
   906	        );
   907	        // Disable/Enable should map to EnableDisable (admin-only)
   908	        assert_eq!(
   909	            action_to_kind(&Action::DisableComputeService { service_id: "svc-1".into(), hostname: "h1".into() }),
   910	            Some(ActionKind::EnableDisable),
   911	        );
   912	        assert_eq!(
   913	            action_to_kind(&Action::EnableComputeService { service_id: "svc-1".into(), hostname: "h1".into() }),
   914	            Some(ActionKind::EnableDisable),
   915	        );
   916	        // FetchMigrationProgress is read-only
   917	        assert_eq!(
   918	            action_to_kind(&Action::FetchMigrationProgress { server_id: "s1".into() }),
   919	            None,
   920	        );
   921	    }
   922	
   923	    #[test]
   924	    fn test_action_to_kind_resize_actions() {
   925	        // Resize actions should map to ActionKind::Resize (member-level)
   926	        assert_eq!(
   927	            action_to_kind(&Action::ResizeServer { id: "s1".into(), flavor_id: "f2".into() }),
   928	            Some(ActionKind::Resize),
   929	        );
   930	        assert_eq!(
   931	            action_to_kind(&Action::ConfirmResize { id: "s1".into() }),
   932	            Some(ActionKind::Resize),
   933	        );
   934	        assert_eq!(
   935	            action_to_kind(&Action::RevertResize { id: "s1".into() }),
   936	            Some(ActionKind::Resize),
   937	        );
   938	    }
   939	
   940	    #[test]
   941	    fn test_permission_denied_event_on_guard_failure() {
   942	        // Verify PermissionDenied event can be constructed with operation name
   943	        let event = AppEvent::PermissionDenied { operation: "CreateServer".into() };
   944	        match event {
   945	            AppEvent::PermissionDenied { operation } => assert_eq!(operation, "CreateServer"),
   946	            _ => panic!("expected PermissionDenied"),
   947	        }
   948	    }
   949	
   950	    #[test]
   951	    fn test_resize_actions_trigger_status_polling() {
   952	        // ResizeServer, ConfirmResize, RevertResize should all be identified
   953	        // as actions requiring server status polling
   954	        let resize = Action::ResizeServer { id: "s1".into(), flavor_id: "f2".into() };
   955	        let confirm = Action::ConfirmResize { id: "s1".into() };
   956	        let revert = Action::RevertResize { id: "s1".into() };
   957	
   958	        assert_eq!(poll_server_id_for_status(&resize), Some("s1".to_string()));
   959	        assert_eq!(poll_server_id_for_status(&confirm), Some("s1".to_string()));
   960	        assert_eq!(poll_server_id_for_status(&revert), Some("s1".to_string()));
   961	
   962	        // Non-resize actions should not trigger status polling
   963	        assert_eq!(poll_server_id_for_status(&Action::FetchServers), None);
   964	    }
   965	
   966	    #[test]
   967	    fn test_polling_dedup_guard() {
   968	        use std::sync::{Arc, Mutex};
   969	        use std::collections::HashSet;
   970	
   971	        let guard: Arc<Mutex<HashSet<String>>> = Arc::new(Mutex::new(HashSet::new()));
   972	
   973	        // First insert succeeds
   974	        assert!(guard.lock().unwrap().insert("s1".to_string()));
   975	        // Duplicate insert fails (already polling)
   976	        assert!(!guard.lock().unwrap().insert("s1".to_string()));
   977	        // Different server succeeds
   978	        assert!(guard.lock().unwrap().insert("s2".to_string()));
   979	        // Remove and re-insert succeeds
   980	        guard.lock().unwrap().remove("s1");

exec
/bin/zsh -lc "nl -ba src/input/command.rs | sed -n '1,320p'" in /Users/jay.ahn/projects/infra/nexttui/.worktrees/runtime-context-switch
 succeeded in 0ms:
     1	use std::collections::HashMap;
     2	use std::fs;
     3	use std::path::PathBuf;
     4	
     5	use crate::error::Result;
     6	use crate::models::common::Route;
     7	
     8	const MAX_HISTORY_ENTRY_LEN: usize = 1024;
     9	
    10	#[derive(Debug, Clone, PartialEq)]
    11	pub enum Command {
    12	    Navigate(Route),
    13	    Quit,
    14	    Refresh,
    15	    Help,
    16	    ContextSwitch(String),
    17	    ContextList,
    18	    Unknown(String),
    19	}
    20	
    21	/// Single source of truth for command name → (abbreviation, Route) mappings.
    22	/// Both abbreviation map and route map are derived from this table.
    23	struct CommandDef {
    24	    name: &'static str,
    25	    abbreviation: &'static str,
    26	    route: Route,
    27	}
    28	
    29	const COMMAND_TABLE: &[CommandDef] = &[
    30	    CommandDef { name: "servers", abbreviation: "srv", route: Route::Servers },
    31	    CommandDef { name: "networks", abbreviation: "net", route: Route::Networks },
    32	    CommandDef { name: "volumes", abbreviation: "vol", route: Route::Volumes },
    33	    CommandDef { name: "floatingip", abbreviation: "fip", route: Route::FloatingIps },
    34	    CommandDef { name: "security-groups", abbreviation: "sec", route: Route::SecurityGroups },
    35	    CommandDef { name: "images", abbreviation: "img", route: Route::Images },
    36	    CommandDef { name: "flavors", abbreviation: "flv", route: Route::Flavors },
    37	    CommandDef { name: "projects", abbreviation: "prj", route: Route::Projects },
    38	    CommandDef { name: "users", abbreviation: "usr", route: Route::Users },
    39	    CommandDef { name: "aggregates", abbreviation: "agg", route: Route::Aggregates },
    40	    CommandDef { name: "hypervisors", abbreviation: "hyp", route: Route::Hosts },
    41	    CommandDef { name: "hosts", abbreviation: "host", route: Route::Hosts },
    42	    CommandDef { name: "migrations", abbreviation: "mig", route: Route::Migrations },
    43	    CommandDef { name: "snapshots", abbreviation: "snap", route: Route::Snapshots },
    44	    CommandDef { name: "compute-services", abbreviation: "svc", route: Route::ComputeServices },
    45	    CommandDef { name: "agents", abbreviation: "agt", route: Route::Agents },
    46	    CommandDef { name: "usage", abbreviation: "usg", route: Route::Usage },
    47	];
    48	
    49	fn build_abbreviations() -> HashMap<String, String> {
    50	    COMMAND_TABLE
    51	        .iter()
    52	        .map(|def| (def.abbreviation.to_string(), def.name.to_string()))
    53	        .collect()
    54	}
    55	
    56	fn build_route_map() -> HashMap<String, Route> {
    57	    COMMAND_TABLE
    58	        .iter()
    59	        .map(|def| (def.name.to_string(), def.route))
    60	        .collect()
    61	}
    62	
    63	pub struct CommandParser {
    64	    abbreviations: HashMap<String, String>,
    65	    route_map: HashMap<String, Route>,
    66	    history: CommandHistory,
    67	    completions: Vec<String>,
    68	    completion_index: usize,
    69	    last_prefix: Option<String>,
    70	}
    71	
    72	impl CommandParser {
    73	    pub fn new(history_path: PathBuf) -> Self {
    74	        Self {
    75	            abbreviations: build_abbreviations(),
    76	            route_map: build_route_map(),
    77	            history: CommandHistory::new(history_path, 50),
    78	            completions: Vec::new(),
    79	            completion_index: 0,
    80	            last_prefix: None,
    81	        }
    82	    }
    83	
    84	    /// Parse a command string. Resolves abbreviations first.
    85	    pub fn parse(&mut self, input: &str) -> Command {
    86	        let trimmed = input.trim();
    87	        if trimmed.is_empty() {
    88	            return Command::Unknown(String::new());
    89	        }
    90	
    91	        let parts: Vec<&str> = trimmed.splitn(2, ' ').collect();
    92	        let cmd = parts[0].to_lowercase();
    93	        let arg = parts.get(1).map(|s| s.trim().to_string());
    94	
    95	        // Resolve abbreviation
    96	        let resolved = self
    97	            .abbreviations
    98	            .get(&cmd)
    99	            .cloned()
   100	            .unwrap_or_else(|| cmd.clone());
   101	
   102	        // System commands
   103	        match resolved.as_str() {
   104	            "q" | "quit" => return Command::Quit,
   105	            "refresh" => return Command::Refresh,
   106	            "help" => return Command::Help,
   107	            "ctx" => {
   108	                return match arg {
   109	                    Some(cloud) if !cloud.is_empty() => Command::ContextSwitch(cloud),
   110	                    _ => Command::ContextList,
   111	                };
   112	            }
   113	            _ => {}
   114	        }
   115	
   116	        // Route navigation
   117	        if let Some(route) = self.route_map.get(&resolved) {
   118	            return Command::Navigate(*route);
   119	        }
   120	
   121	        Command::Unknown(trimmed.to_string())
   122	    }
   123	
   124	    /// Tab auto-complete. Returns the expanded command name (not the abbreviation).
   125	    /// First Tab: collect matching commands by prefix, return first.
   126	    /// Subsequent Tabs with same prefix: cycle through matches.
   127	    /// If the prefix exactly matches an abbreviation, its expanded form is included.
   128	    pub fn auto_complete(&mut self, prefix: &str) -> Option<String> {
   129	        let prefix_lower = prefix.to_lowercase();
   130	
   131	        if self.last_prefix.as_deref() != Some(&prefix_lower) {
   132	            self.completions = self
   133	                .available_commands()
   134	                .into_iter()
   135	                .filter(|cmd| cmd.starts_with(&prefix_lower))
   136	                .collect();
   137	            // Include abbreviation expansions
   138	            for (abbr, full) in &self.abbreviations {
   139	                if abbr.starts_with(&prefix_lower) && !self.completions.contains(full) {
   140	                    self.completions.push(full.clone());
   141	                }
   142	            }
   143	            self.completions.sort();
   144	            self.completions.dedup();
   145	            self.completion_index = 0;
   146	            self.last_prefix = Some(prefix_lower);
   147	        } else if !self.completions.is_empty() {
   148	            self.completion_index = (self.completion_index + 1) % self.completions.len();
   149	        }
   150	
   151	        self.completions.get(self.completion_index).cloned()
   152	    }
   153	
   154	    pub fn reset_completion(&mut self) {
   155	        self.completions.clear();
   156	        self.completion_index = 0;
   157	        self.last_prefix = None;
   158	    }
   159	
   160	    pub fn push_history(&mut self, command: &str) {
   161	        self.history.push(command);
   162	    }
   163	
   164	    pub fn history_prev(&mut self) -> Option<&str> {
   165	        self.history.prev()
   166	    }
   167	
   168	    pub fn history_next(&mut self) -> Option<&str> {
   169	        self.history.next()
   170	    }
   171	
   172	    pub fn history_reset_cursor(&mut self) {
   173	        self.history.reset_cursor();
   174	    }
   175	
   176	    pub fn save_history(&self) -> Result<()> {
   177	        self.history.save()
   178	    }
   179	
   180	    pub fn load_history(&mut self) -> Result<()> {
   181	        self.history.load()
   182	    }
   183	
   184	    /// All valid command names (for auto-complete).
   185	    pub fn available_commands(&self) -> Vec<String> {
   186	        let mut cmds: Vec<String> = self.route_map.keys().cloned().collect();
   187	        cmds.extend(["quit", "refresh", "help", "ctx"].iter().map(|s| s.to_string()));
   188	        cmds.sort();
   189	        cmds
   190	    }
   191	}
   192	
   193	// --- CommandHistory ---
   194	
   195	struct CommandHistory {
   196	    entries: Vec<String>,
   197	    max_size: usize,
   198	    cursor: Option<usize>,
   199	    file_path: PathBuf,
   200	}
   201	
   202	impl CommandHistory {
   203	    fn new(file_path: PathBuf, max_size: usize) -> Self {
   204	        Self {
   205	            entries: Vec::new(),
   206	            max_size,
   207	            cursor: None,
   208	            file_path,
   209	        }
   210	    }
   211	
   212	    fn push(&mut self, command: &str) {
   213	        let cmd = command.trim().to_string();
   214	        if cmd.is_empty() || cmd.len() > MAX_HISTORY_ENTRY_LEN {
   215	            return;
   216	        }
   217	        self.entries.retain(|e| e != &cmd);
   218	        self.entries.push(cmd);
   219	        if self.entries.len() > self.max_size {
   220	            self.entries.remove(0);
   221	        }
   222	        self.cursor = None;
   223	    }
   224	
   225	    fn prev(&mut self) -> Option<&str> {
   226	        if self.entries.is_empty() {
   227	            return None;
   228	        }
   229	        let idx = match self.cursor {
   230	            None => self.entries.len().saturating_sub(1),
   231	            Some(0) => 0,
   232	            Some(c) => c - 1,
   233	        };
   234	        self.cursor = Some(idx);
   235	        self.entries.get(idx).map(|s| s.as_str())
   236	    }
   237	
   238	    fn next(&mut self) -> Option<&str> {
   239	        match self.cursor {
   240	            None => None,
   241	            Some(c) => {
   242	                if c + 1 >= self.entries.len() {
   243	                    self.cursor = None;
   244	                    None
   245	                } else {
   246	                    self.cursor = Some(c + 1);
   247	                    self.entries.get(c + 1).map(|s| s.as_str())
   248	                }
   249	            }
   250	        }
   251	    }
   252	
   253	    fn reset_cursor(&mut self) {
   254	        self.cursor = None;
   255	    }
   256	
   257	    fn save(&self) -> Result<()> {
   258	        if let Some(parent) = self.file_path.parent() {
   259	            fs::create_dir_all(parent).map_err(|e| {
   260	                crate::error::AppError::Other(format!(
   261	                    "Failed to create history directory: {e}"
   262	                ))
   263	            })?;
   264	        }
   265	        let content = self.entries.join("\n");
   266	        fs::write(&self.file_path, content).map_err(|e| {
   267	            crate::error::AppError::Other(format!("Failed to save history: {e}"))
   268	        })
   269	    }
   270	
   271	    fn load(&mut self) -> Result<()> {
   272	        match fs::read_to_string(&self.file_path) {
   273	            Ok(content) => {
   274	                self.entries = content
   275	                    .lines()
   276	                    .filter(|l| !l.is_empty() && l.len() <= MAX_HISTORY_ENTRY_LEN)
   277	                    .map(|l| l.to_string())
   278	                    .collect();
   279	                if self.entries.len() > self.max_size {
   280	                    let excess = self.entries.len() - self.max_size;
   281	                    self.entries.drain(..excess);
   282	                }
   283	                Ok(())
   284	            }
   285	            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(()),
   286	            Err(e) => Err(crate::error::AppError::Other(format!(
   287	                "Failed to load history: {e}"
   288	            ))),
   289	        }
   290	    }
   291	}
   292	
   293	#[cfg(test)]
   294	mod tests {
   295	    use super::*;
   296	    use tempfile::TempDir;
   297	
   298	    fn parser() -> CommandParser {
   299	        let dir = TempDir::new().unwrap();
   300	        CommandParser::new(dir.path().join("history"))
   301	    }
   302	
   303	    #[test]
   304	    fn test_parse_route_direct() {
   305	        let mut p = parser();
   306	        assert_eq!(p.parse("servers"), Command::Navigate(Route::Servers));
   307	        assert_eq!(p.parse("networks"), Command::Navigate(Route::Networks));
   308	    }
   309	
   310	    #[test]
   311	    fn test_parse_abbreviation() {
   312	        let mut p = parser();
   313	        assert_eq!(p.parse("srv"), Command::Navigate(Route::Servers));
   314	        assert_eq!(p.parse("net"), Command::Navigate(Route::Networks));
   315	        assert_eq!(p.parse("vol"), Command::Navigate(Route::Volumes));
   316	    }
   317	
   318	    #[test]
   319	    fn test_parse_system_commands() {
   320	        let mut p = parser();

exec
/bin/zsh -lc "nl -ba src/ui/status_bar.rs | sed -n '1,320p'" in /Users/jay.ahn/projects/infra/nexttui/.worktrees/runtime-context-switch
 succeeded in 0ms:
     1	use ratatui::layout::Rect;
     2	use ratatui::style::{Color, Style};
     3	use ratatui::text::{Line, Span};
     4	use ratatui::widgets::Paragraph;
     5	use ratatui::Frame;
     6	
     7	use super::theme;
     8	
     9	pub struct StatusInfo {
    10	    pub panel_name: String,
    11	    pub item_count: Option<usize>,
    12	    pub selected_index: Option<usize>,
    13	    pub context_hints: Vec<(String, String)>,
    14	    pub error_badge_count: usize,
    15	}
    16	
    17	impl StatusInfo {
    18	    /// Build left-side text: `[PanelName] idx/count` or `[PanelName]`.
    19	    pub fn left_text(&self) -> String {
    20	        if let (Some(count), Some(idx)) = (self.item_count, self.selected_index) {
    21	            format!("[{}] {}/{}", self.panel_name, idx + 1, count)
    22	        } else {
    23	            format!("[{}]", self.panel_name)
    24	        }
    25	    }
    26	}
    27	
    28	pub struct StatusBar;
    29	
    30	impl StatusBar {
    31	    pub fn new() -> Self {
    32	        Self
    33	    }
    34	
    35	    pub fn render(
    36	        &self,
    37	        frame: &mut Frame,
    38	        area: Rect,
    39	        info: &StatusInfo,
    40	    ) {
    41	        // Paragraph bg applies to spans without explicit bg (ratatui style merge)
    42	        let bg = Style::default().bg(Color::DarkGray).fg(Color::White);
    43	        let left = info.left_text();
    44	
    45	        // Error badge: " ⚠N" in red after left text
    46	        let badge = if info.error_badge_count > 0 {
    47	            format!(" \u{26A0}{}", info.error_badge_count)
    48	        } else {
    49	            String::new()
    50	        };
    51	        // Use char count for display width (⚠ is 1 column in most terminals)
    52	        let badge_len = badge.chars().count();
    53	
    54	        // Right: key hints using theme::key_hint()
    55	        let mut hint_spans: Vec<Span> = Vec::new();
    56	        for (i, (key, desc)) in info.context_hints.iter().enumerate() {
    57	            if i > 0 {
    58	                hint_spans.push(Span::raw("  "));
    59	            }
    60	            hint_spans.extend(theme::key_hint(key, desc));
    61	        }
    62	        let hint_plain_len: usize = hint_spans.iter().map(|s| s.content.len()).sum();
    63	
    64	        let padding_len = (area.width as usize)
    65	            .saturating_sub(left.len())
    66	            .saturating_sub(badge_len)
    67	            .saturating_sub(hint_plain_len);
    68	        let padding = " ".repeat(padding_len);
    69	
    70	        let mut spans = vec![
    71	            Span::styled(&left, bg),
    72	        ];
    73	        if info.error_badge_count > 0 {
    74	            spans.push(Span::styled(
    75	                badge,
    76	                Style::default().bg(Color::DarkGray).fg(Color::Red),
    77	            ));
    78	        }
    79	        spans.push(Span::styled(padding, bg));
    80	        spans.extend(hint_spans);
    81	
    82	        let line = Line::from(spans);
    83	        let widget = Paragraph::new(line).style(bg);
    84	        frame.render_widget(widget, area);
    85	    }
    86	}
    87	
    88	impl Default for StatusBar {
    89	    fn default() -> Self {
    90	        Self::new()
    91	    }
    92	}
    93	
    94	#[cfg(test)]
    95	mod tests {
    96	    use super::*;
    97	
    98	    fn sample_info() -> StatusInfo {
    99	        StatusInfo {
   100	            panel_name: "Servers".to_string(),
   101	            item_count: Some(5),
   102	            selected_index: Some(2),
   103	            context_hints: vec![
   104	                ("j/k".into(), "이동".into()),
   105	                ("Enter".into(), "상세".into()),
   106	            ],
   107	            error_badge_count: 0,
   108	        }
   109	    }
   110	
   111	    #[test]
   112	    fn test_status_info_new_fields() {
   113	        let info = sample_info();
   114	        assert_eq!(info.panel_name, "Servers");
   115	        assert_eq!(info.item_count, Some(5));
   116	        assert_eq!(info.selected_index, Some(2));
   117	        assert_eq!(info.context_hints.len(), 2);
   118	        assert_eq!(info.context_hints[0], ("j/k".into(), "이동".into()));
   119	    }
   120	
   121	    #[test]
   122	    fn test_status_info_left_text_with_count() {
   123	        let info = sample_info();
   124	        assert_eq!(info.left_text(), "[Servers] 3/5");
   125	    }
   126	
   127	    #[test]
   128	    fn test_status_info_left_text_without_count() {
   129	        let info = StatusInfo {
   130	            panel_name: "Flavors".to_string(),
   131	            item_count: None,
   132	            selected_index: None,
   133	            context_hints: vec![],
   134	            error_badge_count: 0,
   135	        };
   136	        assert_eq!(info.left_text(), "[Flavors]");
   137	    }
   138	
   139	    #[test]
   140	    fn test_error_badge_count_zero_no_badge_text() {
   141	        let info = sample_info();
   142	        let left = info.left_text();
   143	        // With error_badge_count=0, no badge should appear in left text
   144	        assert!(!left.contains('⚠'));
   145	    }
   146	
   147	    #[test]
   148	    fn test_error_badge_count_positive_shows_badge() {
   149	        let mut info = sample_info();
   150	        info.error_badge_count = 3;
   151	        // The badge is rendered in StatusBar::render, not in left_text.
   152	        // We test the render output by checking the spans.
   153	        // Use a buffer to capture render output.
   154	        use ratatui::backend::TestBackend;
   155	        use ratatui::Terminal;
   156	
   157	        let backend = TestBackend::new(80, 1);
   158	        let mut terminal = Terminal::new(backend).ok();
   159	        if let Some(ref mut term) = terminal {
   160	            let _ = term.draw(|frame| {
   161	                let area = frame.area();
   162	                let bar = StatusBar::new();
   163	                StatusBar::render(&bar, frame, area, &info);
   164	            });
   165	            let buf = term.backend().buffer().clone();
   166	            let content: String = (0..buf.area.width)
   167	                .filter_map(|x| {
   168	                    let cell = &buf[(x, 0)];
   169	                    Some(cell.symbol().to_string())
   170	                })
   171	                .collect();
   172	            assert!(content.contains('⚠'), "badge should appear in rendered output: {content}");
   173	            assert!(content.contains('3'), "badge count should appear: {content}");
   174	        }
   175	    }
   176	
   177	    #[test]
   178	    fn test_status_bar_key_hint_integration() {
   179	        let spans = theme::key_hint("Tab", "패널");
   180	        assert_eq!(spans.len(), 3); // key + separator + desc
   181	        assert_eq!(spans[0].style.fg, Some(ratatui::style::Color::Cyan));
   182	    }
   183	}

exec
/bin/zsh -lc "nl -ba src/module/project/mod.rs | sed -n '1,320p'" in /Users/jay.ahn/projects/infra/nexttui/.worktrees/runtime-context-switch
 succeeded in 0ms:
     1	pub mod view_model;
     2	
     3	use crossterm::event::{KeyCode, KeyEvent};
     4	use ratatui::layout::Rect;
     5	use ratatui::Frame;
     6	use tokio::sync::mpsc;
     7	
     8	use crate::action::Action;
     9	use crate::component::Component;
    10	use crate::event::AppEvent;
    11	use crate::models::keystone::Project;
    12	use crate::module::{ConfirmHandler, PendingAction, ViewState};
    13	use crate::port::types::ProjectCreateParams;
    14	use crate::ui::confirm::ConfirmDialog;
    15	use crate::ui::form::{FormAction, FormWidget, SelectOption};
    16	use crate::ui::resource_list::{ResourceList, Row};
    17	
    18	use self::view_model::{project_columns, project_create_defs, project_detail_data, project_to_row};
    19	
    20	pub struct ProjectModule {
    21	    view_state: ViewState,
    22	    projects: Vec<Project>,
    23	    #[allow(dead_code)]
    24	    loading: bool,
    25	    error_message: Option<String>,
    26	    confirm: ConfirmHandler,
    27	    resource_list: ResourceList,
    28	    form: Option<FormWidget>,
    29	    cached_domain_opts: Vec<SelectOption>,
    30	    action_tx: mpsc::UnboundedSender<Action>,
    31	}
    32	
    33	impl ProjectModule {
    34	    pub fn new(action_tx: mpsc::UnboundedSender<Action>) -> Self {
    35	        Self {
    36	            view_state: ViewState::List,
    37	            projects: Vec::new(),
    38	            loading: false,
    39	            error_message: None,
    40	            confirm: ConfirmHandler::new(),
    41	            resource_list: ResourceList::new(project_columns()),
    42	            form: None,
    43	            cached_domain_opts: Vec::new(),
    44	            action_tx,
    45	        }
    46	    }
    47	
    48	    pub fn view_state(&self) -> &ViewState { &self.view_state }
    49	    pub fn projects(&self) -> &[Project] { &self.projects }
    50	    pub fn selected_index(&self) -> usize { self.resource_list.selected_index() }
    51	    pub fn error_message(&self) -> Option<&str> { self.error_message.as_deref() }
    52	
    53	    fn selected_project(&self) -> Option<&Project> {
    54	        self.projects.get(self.resource_list.selected_index())
    55	    }
    56	
    57	    fn rows(&self) -> Vec<Row> {
    58	        self.projects.iter().map(project_to_row).collect()
    59	    }
    60	
    61	    fn resolve_action(pending: PendingAction) -> Option<Action> {
    62	        match pending {
    63	            PendingAction::DeleteProject { id, .. } => Some(Action::DeleteProject { id }),
    64	            _ => None,
    65	        }
    66	    }
    67	
    68	    fn open_create_form(&mut self) {
    69	        let defs = project_create_defs();
    70	        let mut form = FormWidget::new("Create Project", defs);
    71	        if !self.cached_domain_opts.is_empty() {
    72	            form.set_field_options("Domain", self.cached_domain_opts.clone());
    73	        }
    74	        self.form = Some(form);
    75	        self.view_state = ViewState::Create;
    76	    }
    77	
    78	    fn close_form(&mut self) {
    79	        self.form = None;
    80	        self.view_state = ViewState::List;
    81	    }
    82	
    83	    fn handle_list_key(&mut self, key: KeyEvent) -> Option<Action> {
    84	        if self.resource_list.handle_nav_key(key) { return None; }
    85	        match key.code {
    86	            KeyCode::Enter => {
    87	                if let Some(proj) = self.selected_project() {
    88	                    self.view_state = ViewState::Detail(proj.id.clone());
    89	                }
    90	                None
    91	            }
    92	            KeyCode::Char('c') => { self.open_create_form(); Some(Action::EnterFormMode) }
    93	            KeyCode::Char('d') => {
    94	                if let Some(proj) = self.selected_project() {
    95	                    let id = proj.id.clone();
    96	                    let name = proj.name.clone();
    97	                    self.confirm.open(
    98	                        ConfirmDialog::type_to_confirm(
    99	                            format!("Delete project '{name}'?"),
   100	                            name.clone(),
   101	                        ),
   102	                        PendingAction::DeleteProject { id, name },
   103	                    );
   104	                }
   105	                None
   106	            }
   107	            KeyCode::Char('r') => Some(Action::FetchProjects),
   108	            KeyCode::Left => Some(Action::FocusSidebar),
   109	            KeyCode::Esc => Some(Action::Back),
   110	            _ => None,
   111	        }
   112	    }
   113	
   114	    fn handle_detail_key(&mut self, key: KeyEvent) -> Option<Action> {
   115	        match key.code {
   116	            KeyCode::Esc | KeyCode::Char('q') | KeyCode::Left => { self.view_state = ViewState::List; None }
   117	            _ => None,
   118	        }
   119	    }
   120	
   121	    fn handle_create_key(&mut self, key: KeyEvent) -> Option<Action> {
   122	        let Some(form) = self.form.as_mut() else {
   123	            self.close_form();
   124	            return None;
   125	        };
   126	
   127	        match form.handle_key(key) {
   128	            FormAction::Submit(values) => {
   129	                let name = values
   130	                    .get("Name")
   131	                    .and_then(|v| match v {
   132	                        crate::ui::form::FormValue::Text(s) => Some(s.clone()),
   133	                        _ => None,
   134	                    })
   135	                    .unwrap_or_default();
   136	                let description = values
   137	                    .get("Description")
   138	                    .and_then(|v| match v {
   139	                        crate::ui::form::FormValue::Text(s) => {
   140	                            if s.is_empty() { None } else { Some(s.clone()) }
   141	                        }
   142	                        _ => None,
   143	                    });
   144	                let domain_id = values
   145	                    .get("Domain")
   146	                    .and_then(|v| match v {
   147	                        crate::ui::form::FormValue::Text(s) => Some(s.clone()),
   148	                        _ => None,
   149	                    })
   150	                    .unwrap_or_else(|| "default".to_string());
   151	                let enabled = values
   152	                    .get("Enabled")
   153	                    .and_then(|v| match v {
   154	                        crate::ui::form::FormValue::Bool(b) => Some(*b),
   155	                        _ => None,
   156	                    });
   157	
   158	                self.close_form();
   159	                let _ = self.action_tx.send(Action::CreateProject(ProjectCreateParams {
   160	                    name,
   161	                    description,
   162	                    domain_id,
   163	                    enabled,
   164	                }));
   165	                Some(Action::ExitFormMode)
   166	            }
   167	            FormAction::Cancel => {
   168	                self.close_form();
   169	                Some(Action::ExitFormMode)
   170	            }
   171	            FormAction::None => None,
   172	        }
   173	    }
   174	}
   175	
   176	impl Component for ProjectModule {
   177	    fn refresh_action(&self) -> Option<Action> { Some(Action::FetchProjects) }
   178	    fn is_modal(&self) -> bool { self.confirm.is_active() || self.form.is_some() }
   179	
   180	    fn handle_key(&mut self, key: KeyEvent) -> Option<Action> {
   181	        if let Some(result) = self.confirm.handle_key(key, Self::resolve_action) { return result; }
   182	        match &self.view_state {
   183	            ViewState::List => self.handle_list_key(key),
   184	            ViewState::Detail(_) => self.handle_detail_key(key),
   185	            ViewState::Create => self.handle_create_key(key),
   186	        }
   187	    }
   188	
   189	    fn handle_event(&mut self, event: &AppEvent) {
   190	        match event {
   191	            AppEvent::ProjectsLoaded(projects) => {
   192	                self.projects = projects.clone();
   193	                self.loading = false;
   194	                self.error_message = None;
   195	                let rows = self.rows();
   196	                self.resource_list.set_rows(rows);
   197	                // Build domain dropdown options from loaded projects
   198	                let mut domain_ids: Vec<String> = projects
   199	                    .iter()
   200	                    .filter_map(|p| p.domain_id.clone())
   201	                    .collect();
   202	                domain_ids.sort();
   203	                domain_ids.dedup();
   204	                self.cached_domain_opts = domain_ids
   205	                    .into_iter()
   206	                    .map(|d| SelectOption { value: d.clone(), display: d })
   207	                    .collect();
   208	            }
   209	            AppEvent::ProjectCreated(_) => {
   210	                self.view_state = ViewState::List;
   211	                let _ = self.action_tx.send(Action::FetchProjects);
   212	            }
   213	            AppEvent::ProjectDeleted { .. } => {
   214	                let _ = self.action_tx.send(Action::FetchProjects);
   215	            }
   216	            AppEvent::ApiError { operation, message, .. } => {
   217	                self.error_message = Some(format!("{operation}: {message}"));
   218	                self.loading = false;
   219	            }
   220	            _ => {}
   221	        }
   222	    }
   223	
   224	    fn render(&self, frame: &mut Frame, area: Rect) {
   225	        match &self.view_state {
   226	            ViewState::List => self.resource_list.render(frame, area),
   227	            ViewState::Detail(id) => {
   228	                if let Some(proj) = self.projects.iter().find(|p| p.id == *id) {
   229	                    let data = project_detail_data(proj);
   230	                    let mut dv = crate::ui::detail_view::DetailView::new();
   231	                    dv.set_data(data);
   232	                    dv.render(frame, area);
   233	                }
   234	            }
   235	            ViewState::Create => {
   236	                if let Some(form) = &self.form {
   237	                    form.render(frame, area);
   238	                } else {
   239	                    self.resource_list.render(frame, area);
   240	                }
   241	            }
   242	        }
   243	        self.confirm.render(frame, area);
   244	    }
   245	
   246	    fn content_title(&self) -> Option<String> {
   247	        match &self.view_state {
   248	            ViewState::List => None,
   249	            ViewState::Detail(id) => {
   250	                let name = self.projects.iter()
   251	                    .find(|r| r.id == *id)
   252	                    .map(|r| r.name.as_str())
   253	                    .unwrap_or("...");
   254	                Some(format!("Project: {name}"))
   255	            }
   256	            ViewState::Create => Some("Create Project".into()),
   257	        }
   258	    }
   259	
   260	    fn help_hint(&self) -> &str {
   261	        match &self.view_state {
   262	            ViewState::List => "Enter:Detail c:Create d:Delete r:Refresh",
   263	            ViewState::Detail(_) => "Esc:Back",
   264	            ViewState::Create => "Esc:Cancel Tab:Next Enter:Submit",
   265	        }
   266	    }
   267	}
   268	
   269	#[cfg(test)]
   270	mod tests {
   271	    use super::*;
   272	    fn key(code: KeyCode) -> KeyEvent { KeyEvent::from(code) }
   273	    fn make_project(id: &str, name: &str) -> Project {
   274	        Project { id: id.into(), name: name.into(), description: None, enabled: true, domain_id: Some("default".into()) }
   275	    }
   276	    fn setup() -> (ProjectModule, mpsc::UnboundedReceiver<Action>) {
   277	        let (tx, rx) = mpsc::unbounded_channel();
   278	        let mut m = ProjectModule::new(tx);
   279	        m.handle_event(&AppEvent::ProjectsLoaded(vec![
   280	            make_project("p1", "admin"), make_project("p2", "demo"),
   281	        ]));
   282	        (m, rx)
   283	    }
   284	
   285	    #[test] fn test_initial_state() { let (tx, _) = mpsc::unbounded_channel(); let m = ProjectModule::new(tx); assert_eq!(*m.view_state(), ViewState::List); }
   286	    #[test] fn test_nav() { let (mut m, _) = setup(); m.handle_key(key(KeyCode::Char('j'))); assert_eq!(m.selected_index(), 1); }
   287	    #[test] fn test_enter_detail() { let (mut m, _) = setup(); m.handle_key(key(KeyCode::Enter)); assert_eq!(*m.view_state(), ViewState::Detail("p1".into())); }
   288	    #[test] fn test_esc_to_list() { let (mut m, _) = setup(); m.handle_key(key(KeyCode::Enter)); m.handle_key(key(KeyCode::Esc)); assert_eq!(*m.view_state(), ViewState::List); }
   289	    #[test] fn test_create() { let (mut m, _) = setup(); m.handle_key(key(KeyCode::Char('c'))); assert_eq!(*m.view_state(), ViewState::Create); assert!(m.form.is_some()); }
   290	    #[test] fn test_delete_confirm() { let (mut m, _) = setup(); m.handle_key(key(KeyCode::Char('d'))); assert!(m.confirm.is_active()); }
   291	    #[test] fn test_confirm_delete() {
   292	        let (mut m, _) = setup();
   293	        m.handle_key(key(KeyCode::Char('d')));
   294	        for c in "admin".chars() { m.handle_key(key(KeyCode::Char(c))); }
   295	        let a = m.handle_key(key(KeyCode::Enter));
   296	        assert!(matches!(a, Some(Action::DeleteProject { .. })));
   297	    }
   298	    #[test] fn test_refresh() { let (mut m, _) = setup(); assert!(matches!(m.handle_key(key(KeyCode::Char('r'))), Some(Action::FetchProjects))); }
   299	    #[test] fn test_event_loaded() {
   300	        let (tx, _) = mpsc::unbounded_channel(); let mut m = ProjectModule::new(tx);
   301	        m.handle_event(&AppEvent::ProjectsLoaded(vec![make_project("p1", "t")]));
   302	        assert_eq!(m.projects().len(), 1);
   303	    }
   304	    #[test] fn test_event_created() {
   305	        let (mut m, mut rx) = setup(); m.view_state = ViewState::Create;
   306	        m.handle_event(&AppEvent::ProjectCreated(make_project("p3", "new")));
   307	        assert_eq!(*m.view_state(), ViewState::List);
   308	        assert!(matches!(rx.try_recv().unwrap(), Action::FetchProjects));
   309	    }
   310	    #[test] fn test_event_deleted() {
   311	        let (mut m, mut rx) = setup();
   312	        m.handle_event(&AppEvent::ProjectDeleted { id: "p1".into() });
   313	        assert!(matches!(rx.try_recv().unwrap(), Action::FetchProjects));
   314	    }
   315	
   316	    // -- Form integration tests -----------------------------------------------
   317	
   318	    #[test]
   319	    fn test_create_form_cancel_returns_to_list() {
   320	        let (mut m, _) = setup();

exec
/bin/zsh -lc "nl -ba src/ui/confirm.rs | sed -n '1,320p'" in /Users/jay.ahn/projects/infra/nexttui/.worktrees/runtime-context-switch
 succeeded in 0ms:
     1	use crossterm::event::{KeyCode, KeyEvent};
     2	use ratatui::layout::{Alignment, Rect};
     3	use ratatui::style::{Color, Modifier, Style};
     4	use ratatui::text::{Line, Span};
     5	use ratatui::widgets::{Block, BorderType, Borders, Clear, Paragraph};
     6	
     7	use super::theme::Theme;
     8	use ratatui::Frame;
     9	
    10	const MAX_BUFFER_LEN: usize = 256;
    11	
    12	/// Confirm dialog mode.
    13	#[derive(Debug, Clone, PartialEq, Eq)]
    14	pub enum ConfirmMode {
    15	    /// Simple Y/N confirmation.
    16	    YesNo { message: String },
    17	    /// Type resource name to confirm (destructive actions).
    18	    TypeToConfirm {
    19	        message: String,
    20	        expected: String,
    21	        buffer: String,
    22	    },
    23	}
    24	
    25	pub enum ConfirmResult {
    26	    Confirmed,
    27	    Cancelled,
    28	    Pending,
    29	}
    30	
    31	pub struct ConfirmDialog {
    32	    mode: ConfirmMode,
    33	    active: bool,
    34	    detail_lines: Vec<String>,
    35	}
    36	
    37	impl ConfirmDialog {
    38	    pub fn yes_no(message: impl Into<String>) -> Self {
    39	        Self {
    40	            mode: ConfirmMode::YesNo {
    41	                message: message.into(),
    42	            },
    43	            active: true,
    44	            detail_lines: Vec::new(),
    45	        }
    46	    }
    47	
    48	    pub fn yes_no_with_details(message: impl Into<String>, details: Vec<String>) -> Self {
    49	        Self {
    50	            mode: ConfirmMode::YesNo {
    51	                message: message.into(),
    52	            },
    53	            active: true,
    54	            detail_lines: details,
    55	        }
    56	    }
    57	
    58	    pub fn type_to_confirm(message: impl Into<String>, expected: impl Into<String>) -> Self {
    59	        Self {
    60	            mode: ConfirmMode::TypeToConfirm {
    61	                message: message.into(),
    62	                expected: expected.into(),
    63	                buffer: String::new(),
    64	            },
    65	            active: true,
    66	            detail_lines: Vec::new(),
    67	        }
    68	    }
    69	
    70	    pub fn type_to_confirm_with_details(
    71	        message: impl Into<String>,
    72	        expected: impl Into<String>,
    73	        details: Vec<String>,
    74	    ) -> Self {
    75	        Self {
    76	            mode: ConfirmMode::TypeToConfirm {
    77	                message: message.into(),
    78	                expected: expected.into(),
    79	                buffer: String::new(),
    80	            },
    81	            active: true,
    82	            detail_lines: details,
    83	        }
    84	    }
    85	
    86	    pub fn is_active(&self) -> bool {
    87	        self.active
    88	    }
    89	
    90	    pub fn message(&self) -> &str {
    91	        match &self.mode {
    92	            ConfirmMode::YesNo { message } => message,
    93	            ConfirmMode::TypeToConfirm { message, .. } => message,
    94	        }
    95	    }
    96	
    97	    pub fn detail_lines(&self) -> &[String] {
    98	        &self.detail_lines
    99	    }
   100	
   101	    pub fn handle_key(&mut self, key: KeyEvent) -> ConfirmResult {
   102	        if !self.active {
   103	            return ConfirmResult::Pending;
   104	        }
   105	
   106	        match &mut self.mode {
   107	            ConfirmMode::YesNo { .. } => match key.code {
   108	                KeyCode::Char('y') | KeyCode::Char('Y') => {
   109	                    self.active = false;
   110	                    ConfirmResult::Confirmed
   111	                }
   112	                KeyCode::Char('n') | KeyCode::Char('N') | KeyCode::Esc => {
   113	                    self.active = false;
   114	                    ConfirmResult::Cancelled
   115	                }
   116	                _ => ConfirmResult::Pending,
   117	            },
   118	            ConfirmMode::TypeToConfirm {
   119	                expected, buffer, ..
   120	            } => match key.code {
   121	                KeyCode::Esc => {
   122	                    self.active = false;
   123	                    ConfirmResult::Cancelled
   124	                }
   125	                KeyCode::Enter => {
   126	                    if buffer == expected {
   127	                        self.active = false;
   128	                        ConfirmResult::Confirmed
   129	                    } else {
   130	                        ConfirmResult::Pending
   131	                    }
   132	                }
   133	                KeyCode::Backspace => {
   134	                    buffer.pop();
   135	                    ConfirmResult::Pending
   136	                }
   137	                KeyCode::Char(c) => {
   138	                    if buffer.len() < MAX_BUFFER_LEN {
   139	                        buffer.push(c);
   140	                    }
   141	                    ConfirmResult::Pending
   142	                }
   143	                _ => ConfirmResult::Pending,
   144	            },
   145	        }
   146	    }
   147	
   148	    /// Render the confirm dialog as a centered modal overlay.
   149	    pub fn render(&self, frame: &mut Frame, area: Rect) {
   150	        if !self.active {
   151	            return;
   152	        }
   153	
   154	        // Calculate centered modal area (50% width, dynamic height)
   155	        let width = (area.width / 2).max(30).min(area.width);
   156	        let detail_count = self.detail_lines.len() as u16;
   157	        let height = (7u16 + detail_count).min(area.height);
   158	        let x = area.x + (area.width.saturating_sub(width)) / 2;
   159	        let y = area.y + (area.height.saturating_sub(height)) / 2;
   160	        let modal_area = Rect::new(x, y, width, height);
   161	
   162	        frame.render_widget(Clear, modal_area);
   163	
   164	        let detail_style = Style::default().fg(Color::DarkGray).add_modifier(Modifier::DIM);
   165	
   166	        let lines = match &self.mode {
   167	            ConfirmMode::YesNo { message } => {
   168	                let mut l = vec![
   169	                    Line::from(""),
   170	                    Line::from(Span::styled(
   171	                        message.as_str(),
   172	                        Theme::warning().add_modifier(Modifier::BOLD),
   173	                    )),
   174	                ];
   175	                for detail in &self.detail_lines {
   176	                    l.push(Line::from(Span::styled(detail.as_str(), detail_style)));
   177	                }
   178	                l.push(Line::from(""));
   179	                l.push(Line::from(vec![
   180	                    Span::styled("[Y]", Theme::focus_border().add_modifier(Modifier::BOLD)),
   181	                    Span::styled("es  ", Style::default().fg(Color::White)),
   182	                    Span::styled("[N]", Theme::focus_border().add_modifier(Modifier::BOLD)),
   183	                    Span::styled("o", Style::default().fg(Color::White)),
   184	                ]));
   185	                l
   186	            }
   187	            ConfirmMode::TypeToConfirm {
   188	                message,
   189	                expected,
   190	                buffer,
   191	                ..
   192	            } => {
   193	                let mut l = vec![
   194	                    Line::from(Span::styled(
   195	                        message.as_str(),
   196	                        Theme::warning().add_modifier(Modifier::BOLD),
   197	                    )),
   198	                ];
   199	                for detail in &self.detail_lines {
   200	                    l.push(Line::from(Span::styled(detail.as_str(), detail_style)));
   201	                }
   202	                l.push(Line::from(format!("Type '{expected}' to confirm:")));
   203	                l.push(Line::from(""));
   204	                l.push(Line::from(vec![
   205	                    Span::raw("> "),
   206	                    Span::styled(
   207	                        buffer.as_str(),
   208	                        Style::default().fg(Color::White),
   209	                    ),
   210	                    Span::styled("_", Theme::waiting()),
   211	                ]));
   212	                l
   213	            }
   214	        };
   215	
   216	        let block = Block::default()
   217	            .title(" Confirm ")
   218	            .borders(Borders::ALL)
   219	            .border_type(BorderType::Rounded)
   220	            .border_style(Theme::warning().add_modifier(Modifier::BOLD))
   221	            .style(Style::default().bg(Color::Rgb(30, 30, 40)));
   222	        let widget = Paragraph::new(lines)
   223	            .block(block)
   224	            .alignment(Alignment::Center)
   225	            .style(Style::default().bg(Color::Rgb(30, 30, 40)));
   226	        frame.render_widget(widget, modal_area);
   227	    }
   228	}
   229	
   230	#[cfg(test)]
   231	mod tests {
   232	    use super::*;
   233	
   234	    fn key(code: KeyCode) -> KeyEvent {
   235	        KeyEvent::from(code)
   236	    }
   237	
   238	    #[test]
   239	    fn test_yes_no_confirm() {
   240	        let mut dialog = ConfirmDialog::yes_no("Delete server?");
   241	        assert!(dialog.is_active());
   242	        let result = dialog.handle_key(key(KeyCode::Char('y')));
   243	        assert!(matches!(result, ConfirmResult::Confirmed));
   244	        assert!(!dialog.is_active());
   245	    }
   246	
   247	    #[test]
   248	    fn test_yes_no_cancel() {
   249	        let mut dialog = ConfirmDialog::yes_no("Delete server?");
   250	        let result = dialog.handle_key(key(KeyCode::Char('n')));
   251	        assert!(matches!(result, ConfirmResult::Cancelled));
   252	    }
   253	
   254	    #[test]
   255	    fn test_type_to_confirm_success() {
   256	        let mut dialog = ConfirmDialog::type_to_confirm("Type 'web-01' to delete", "web-01");
   257	        for c in "web-01".chars() {
   258	            let result = dialog.handle_key(key(KeyCode::Char(c)));
   259	            assert!(matches!(result, ConfirmResult::Pending));
   260	        }
   261	        let result = dialog.handle_key(key(KeyCode::Enter));
   262	        assert!(matches!(result, ConfirmResult::Confirmed));
   263	    }
   264	
   265	    #[test]
   266	    fn test_type_to_confirm_wrong_name() {
   267	        let mut dialog = ConfirmDialog::type_to_confirm("Type 'web-01' to delete", "web-01");
   268	        for c in "wrong".chars() {
   269	            dialog.handle_key(key(KeyCode::Char(c)));
   270	        }
   271	        let result = dialog.handle_key(key(KeyCode::Enter));
   272	        assert!(matches!(result, ConfirmResult::Pending));
   273	        assert!(dialog.is_active());
   274	    }
   275	
   276	    #[test]
   277	    fn test_backspace_in_type_to_confirm() {
   278	        let mut dialog = ConfirmDialog::type_to_confirm("confirm", "abc");
   279	        dialog.handle_key(key(KeyCode::Char('a')));
   280	        dialog.handle_key(key(KeyCode::Char('b')));
   281	        dialog.handle_key(key(KeyCode::Char('x')));
   282	        dialog.handle_key(key(KeyCode::Backspace));
   283	        dialog.handle_key(key(KeyCode::Char('c')));
   284	        let result = dialog.handle_key(key(KeyCode::Enter));
   285	        assert!(matches!(result, ConfirmResult::Confirmed));
   286	    }
   287	
   288	    #[test]
   289	    fn test_inactive_ignores_keys() {
   290	        let mut dialog = ConfirmDialog::yes_no("test");
   291	        dialog.handle_key(key(KeyCode::Char('y'))); // confirms, now inactive
   292	        let result = dialog.handle_key(key(KeyCode::Char('y'))); // should be ignored
   293	        assert!(matches!(result, ConfirmResult::Pending));
   294	    }
   295	
   296	    // --- detail_lines tests ---
   297	
   298	    #[test]
   299	    fn test_yes_no_with_details_creates_dialog() {
   300	        let details = vec!["Volume: vol-01".into(), "Size: 100GB".into()];
   301	        let dialog = ConfirmDialog::yes_no_with_details("Detach volume?", details.clone());
   302	        assert!(dialog.is_active());
   303	        assert_eq!(dialog.message(), "Detach volume?");
   304	        assert_eq!(dialog.detail_lines(), &details);
   305	    }
   306	
   307	    #[test]
   308	    fn test_type_to_confirm_with_details_creates_dialog() {
   309	        let details = vec!["Server: web-01".into(), "IP: 10.0.0.1".into()];
   310	        let dialog = ConfirmDialog::type_to_confirm_with_details(
   311	            "Type 'web-01' to delete",
   312	            "web-01",
   313	            details.clone(),
   314	        );
   315	        assert!(dialog.is_active());
   316	        assert_eq!(dialog.message(), "Type 'web-01' to delete");
   317	        assert_eq!(dialog.detail_lines(), &details);
   318	    }
   319	
   320	    #[test]

exec
/bin/zsh -lc "nl -ba src/adapter/registry.rs | sed -n '1,300p'" in /Users/jay.ahn/projects/infra/nexttui/.worktrees/runtime-context-switch
 succeeded in 0ms:
     1	use std::sync::Arc;
     2	
     3	use crate::adapter::http::cinder::CinderHttpAdapter;
     4	use crate::adapter::http::glance::GlanceHttpAdapter;
     5	use crate::adapter::http::keystone::KeystoneHttpAdapter;
     6	use crate::adapter::http::neutron::NeutronHttpAdapter;
     7	use crate::adapter::http::nova::NovaHttpAdapter;
     8	use crate::port::auth::AuthProvider;
     9	use crate::port::cinder::CinderPort;
    10	use crate::port::error::ApiError;
    11	use crate::port::glance::GlancePort;
    12	use crate::port::keystone::KeystonePort;
    13	use crate::port::neutron::NeutronPort;
    14	use crate::port::nova::NovaPort;
    15	
    16	/// AdapterRegistry creates and holds all service adapters.
    17	/// In Phase 1, all adapters use HTTP/REST via BaseHttpClient.
    18	/// In Phase 2, this will support config-based backend selection
    19	/// (e.g., Service Layer gateway instead of direct OpenStack API).
    20	pub struct AdapterRegistry {
    21	    pub nova: Arc<dyn NovaPort>,
    22	    pub neutron: Arc<dyn NeutronPort>,
    23	    pub cinder: Arc<dyn CinderPort>,
    24	    pub glance: Arc<dyn GlancePort>,
    25	    pub keystone: Arc<dyn KeystonePort>,
    26	}
    27	
    28	impl AdapterRegistry {
    29	    /// Create all HTTP adapters from the given auth provider and region.
    30	    pub fn new_http(auth: Arc<dyn AuthProvider>, region: Option<String>) -> Result<Self, ApiError> {
    31	        Ok(Self {
    32	            nova: Arc::new(NovaHttpAdapter::new(auth.clone(), region.clone())?),
    33	            neutron: Arc::new(NeutronHttpAdapter::new(auth.clone(), region.clone())?),
    34	            cinder: Arc::new(CinderHttpAdapter::new(auth.clone(), region.clone())?),
    35	            glance: Arc::new(GlanceHttpAdapter::new(auth.clone(), region.clone())?),
    36	            keystone: Arc::new(KeystoneHttpAdapter::new(auth, region)?),
    37	        })
    38	    }
    39	
    40	    /// Create registry from mock adapters (for testing).
    41	    #[cfg(test)]
    42	    pub fn new_mock() -> Self {
    43	        use crate::port::mock::*;
    44	        Self {
    45	            nova: Arc::new(MockNovaAdapter),
    46	            neutron: Arc::new(MockNeutronAdapter),
    47	            cinder: Arc::new(MockCinderAdapter),
    48	            glance: Arc::new(MockGlanceAdapter),
    49	            keystone: Arc::new(MockKeystoneAdapter),
    50	        }
    51	    }
    52	}
    53	
    54	#[cfg(test)]
    55	mod tests {
    56	    use super::*;
    57	
    58	    #[test]
    59	    fn test_adapter_registry_mock_creation() {
    60	        let registry = AdapterRegistry::new_mock();
    61	        let _nova: &dyn NovaPort = registry.nova.as_ref();
    62	        let _neutron: &dyn NeutronPort = registry.neutron.as_ref();
    63	        let _cinder: &dyn CinderPort = registry.cinder.as_ref();
    64	        let _glance: &dyn GlancePort = registry.glance.as_ref();
    65	        let _keystone: &dyn KeystonePort = registry.keystone.as_ref();
    66	    }
    67	
    68	    #[test]
    69	    fn test_registry_adapters_are_send_sync() {
    70	        fn assert_send_sync<T: Send + Sync>() {}
    71	        assert_send_sync::<AdapterRegistry>();
    72	    }
    73	}

exec
/bin/zsh -lc "rg -n \"invalidate_endpoint|BaseHttpClient|token_refresh|subscribe_token_refresh|CloudSwitched|SwitchCloud|ContextChanged|VersionedEvent|epoch|CancellationToken|cancelled\\(\" src | sed -n '1,260p'" in /Users/jay.ahn/projects/infra/nexttui/.worktrees/runtime-context-switch
 succeeded in 0ms:
src/app.rs:1078:    fn test_handle_token_refreshed_updates_rbac() {
src/action.rs:107:    SwitchCloud(String),
src/action.rs:160:            Action::SwitchCloud("prod".into()),
src/event.rs:105:    CloudSwitched(String),
src/event.rs:135:            AppEvent::CloudSwitched("prod".into()),
src/event.rs:141:    fn test_token_refreshed_carries_roles() {
src/port/auth.rs:26:    fn subscribe_token_refresh(&self) -> broadcast::Receiver<Token>;
src/adapter/http/cinder.rs:8:use crate::adapter::http::base::BaseHttpClient;
src/adapter/http/cinder.rs:16:    base: BaseHttpClient,
src/adapter/http/cinder.rs:22:            base: BaseHttpClient::new(auth, "block-storage", EndpointInterface::Public, region)?,
src/worker.rs:728:        Action::SwitchCloud(_cloud_name) => {
src/adapter/registry.rs:17:/// In Phase 1, all adapters use HTTP/REST via BaseHttpClient.
src/adapter/http/keystone.rs:7:use crate::adapter::http::base::BaseHttpClient;
src/adapter/http/keystone.rs:15:    base: BaseHttpClient,
src/adapter/http/keystone.rs:21:            base: BaseHttpClient::new(auth, "identity", EndpointInterface::Public, region)?,
src/adapter/http/mod.rs:11:use crate::adapter::http::base::BaseHttpClient;
src/adapter/http/mod.rs:89:    base: &BaseHttpClient,
src/adapter/http/base.rs:16:/// Callers should call `invalidate_endpoint()` when token refresh occurs
src/adapter/http/base.rs:17:/// to pick up potential catalog changes. In Phase 2, BaseHttpClient will
src/adapter/http/base.rs:19:pub struct BaseHttpClient {
src/adapter/http/base.rs:28:impl BaseHttpClient {
src/adapter/http/base.rs:68:    pub async fn invalidate_endpoint(&self) {
src/adapter/http/base.rs:213:        let result = BaseHttpClient::check_status(resp).await;
src/adapter/http/base.rs:220:        let err = BaseHttpClient::check_status(resp).await.unwrap_err();
src/adapter/http/base.rs:227:        let err = BaseHttpClient::check_status(resp).await.unwrap_err();
src/adapter/http/base.rs:234:        let err = BaseHttpClient::check_status(resp).await.unwrap_err();
src/adapter/http/base.rs:241:        let err = BaseHttpClient::check_status(resp).await.unwrap_err();
src/adapter/http/base.rs:248:        let err = BaseHttpClient::check_status(resp).await.unwrap_err();
src/adapter/http/base.rs:255:        let err = BaseHttpClient::check_status(resp).await.unwrap_err();
src/adapter/http/base.rs:262:        let err = BaseHttpClient::check_status(resp).await.unwrap_err();
src/adapter/http/base.rs:269:        let err = BaseHttpClient::check_status(resp).await.unwrap_err();
src/adapter/http/glance.rs:7:use crate::adapter::http::base::BaseHttpClient;
src/adapter/http/glance.rs:15:    base: BaseHttpClient,
src/adapter/http/glance.rs:21:            base: BaseHttpClient::new(auth, "image", EndpointInterface::Public, region)?,
src/adapter/http/neutron.rs:7:use crate::adapter::http::base::BaseHttpClient;
src/adapter/http/neutron.rs:15:    base: BaseHttpClient,
src/adapter/http/neutron.rs:21:            base: BaseHttpClient::new(auth, "network", EndpointInterface::Public, region)?,
src/adapter/http/nova.rs:9:use crate::adapter::http::base::BaseHttpClient;
src/adapter/http/nova.rs:17:    base: BaseHttpClient,
src/adapter/http/nova.rs:23:            base: BaseHttpClient::new(auth, "compute", EndpointInterface::Public, region)?,
src/adapter/auth/keystone.rs:192:        let refresh_span = tracing::info_span!("token_refresh_loop");
src/adapter/auth/keystone.rs:433:    fn subscribe_token_refresh(&self) -> broadcast::Receiver<Token> {

exec
/bin/zsh -lc "rg -n \"run_worker\\(|action_rx|event_rx|event_tx|handle_event\\(|Action::\" src/main.rs src/lib.rs src/bin -g '*.rs' | sed -n '1,260p'" in /Users/jay.ahn/projects/infra/nexttui/.worktrees/runtime-context-switch
 succeeded in 0ms:
rg: src/bin: No such file or directory (os error 2)
src/main.rs:48:    // Keep _event_tx alive so event_rx doesn't immediately return None in demo mode
src/main.rs:49:    let (mut app, event_rx, _keep_alive_tx) = if demo_mode {
src/main.rs:50:        let (app, _action_rx) = create_demo_app()?;
src/main.rs:51:        let (event_tx, event_rx) = mpsc::unbounded_channel::<AppEvent>();
src/main.rs:52:        (app, event_rx, Some(event_tx))
src/main.rs:76:        let (action_tx, action_rx) = mpsc::unbounded_channel();
src/main.rs:77:        let (event_tx, event_rx) = mpsc::unbounded_channel::<AppEvent>();
src/main.rs:119:        tokio::spawn(run_worker(registry, rbac, app.all_tenants.clone(), action_rx, event_tx));
src/main.rs:126:        (app, event_rx, None)
src/main.rs:144:    let result = run_event_loop(&mut terminal, &mut app, event_rx).await;

codex
I confirmed current runtime wiring in `main.rs` and will now inspect that loop plus `port/mock.rs` to evaluate whether the proposed mock/test seams and epoch-event envelope are practically complete.
exec
/bin/zsh -lc "nl -ba src/main.rs | sed -n '1,260p'" in /Users/jay.ahn/projects/infra/nexttui/.worktrees/runtime-context-switch
 succeeded in 0ms:
     1	use std::io;
     2	use std::path::PathBuf;
     3	use std::sync::Arc;
     4	
     5	use crossterm::{
     6	    execute,
     7	    terminal::{EnterAlternateScreen, LeaveAlternateScreen, disable_raw_mode, enable_raw_mode},
     8	};
     9	use ratatui::Terminal;
    10	use ratatui::backend::CrosstermBackend;
    11	use tokio::sync::mpsc;
    12	use tracing_subscriber::EnvFilter;
    13	
    14	use nexttui::adapter::auth::keystone::KeystoneAuthAdapter;
    15	use nexttui::adapter::registry::AdapterRegistry;
    16	use nexttui::app::App;
    17	use nexttui::config::Config;
    18	use nexttui::demo::create_demo_app;
    19	use nexttui::event::AppEvent;
    20	use nexttui::event_loop::run_event_loop;
    21	use nexttui::port::auth::AuthProvider;
    22	use nexttui::port::types::{AuthCredential, AuthMethod, ProjectScopeParam};
    23	use nexttui::worker::run_worker;
    24	
    25	#[tokio::main]
    26	async fn main() -> Result<(), Box<dyn std::error::Error>> {
    27	    // Initialize tracing (file-based, since TUI owns stdout/stderr)
    28	    let log_dir = dirs::cache_dir()
    29	        .unwrap_or_else(|| PathBuf::from("/tmp"))
    30	        .join("nexttui");
    31	    let file_appender = tracing_appender::rolling::daily(&log_dir, "nexttui.log");
    32	    let (non_blocking, _log_guard) = tracing_appender::non_blocking(file_appender);
    33	    tracing_subscriber::fmt()
    34	        .with_env_filter(
    35	            EnvFilter::try_from_default_env()
    36	                .unwrap_or_else(|_| EnvFilter::new("nexttui=info")),
    37	        )
    38	        .with_writer(non_blocking)
    39	        .with_ansi(false)
    40	        .init();
    41	
    42	    let args: Vec<String> = std::env::args().collect();
    43	    let demo_mode = args.iter().any(|a| a == "--demo");
    44	    let cloud_arg = args.windows(2)
    45	        .find(|w| w[0] == "--cloud")
    46	        .map(|w| w[1].clone());
    47	
    48	    // Keep _event_tx alive so event_rx doesn't immediately return None in demo mode
    49	    let (mut app, event_rx, _keep_alive_tx) = if demo_mode {
    50	        let (app, _action_rx) = create_demo_app()?;
    51	        let (event_tx, event_rx) = mpsc::unbounded_channel::<AppEvent>();
    52	        (app, event_rx, Some(event_tx))
    53	    } else {
    54	        let mut config = match Config::load() {
    55	            Ok(c) => c,
    56	            Err(e) => {
    57	                eprintln!("Error: {e}");
    58	                tracing::error!(%e, "failed to load config");
    59	                std::process::exit(1);
    60	            }
    61	        };
    62	
    63	        // --cloud CLI arg overrides OS_CLOUD and config.toml default_cloud
    64	        if let Some(ref name) = cloud_arg {
    65	            if let Err(e) = config.switch_cloud(name) {
    66	                eprintln!("Error: {e}");
    67	                std::process::exit(1);
    68	            }
    69	        }
    70	
    71	        for w in config.warnings() {
    72	            eprintln!("Warning: {w}");
    73	            tracing::warn!(warning = %w, "config warning");
    74	        }
    75	
    76	        let (action_tx, action_rx) = mpsc::unbounded_channel();
    77	        let (event_tx, event_rx) = mpsc::unbounded_channel::<AppEvent>();
    78	
    79	        // Build auth credential from config
    80	        let cloud = config.active_cloud_config();
    81	        let credential = AuthCredential {
    82	            auth_url: cloud.auth.auth_url.clone(),
    83	            method: AuthMethod::Password {
    84	                username: cloud.auth.username.clone().unwrap_or_default(),
    85	                password: cloud.auth.password.clone().unwrap_or_default(),
    86	                domain_name: cloud
    87	                    .auth
    88	                    .user_domain_name
    89	                    .clone()
    90	                    .unwrap_or_else(|| "Default".to_string()),
    91	            },
    92	            project_scope: cloud.auth.project_name.as_ref().map(|pn| ProjectScopeParam {
    93	                name: pn.clone(),
    94	                domain_name: cloud
    95	                    .auth
    96	                    .project_domain_name
    97	                    .clone()
    98	                    .unwrap_or_else(|| "Default".to_string()),
    99	            }),
   100	        };
   101	
   102	        let auth_provider = Arc::new(KeystoneAuthAdapter::new(credential)?);
   103	        let registry = Arc::new(AdapterRegistry::new_http(
   104	            auth_provider.clone(),
   105	            cloud.region_name.clone(),
   106	        )?);
   107	
   108	        // Trigger initial authentication, then initialize RBAC from token roles
   109	        let rbac = std::sync::Arc::new(nexttui::infra::rbac::RbacGuard::new());
   110	        let _ = auth_provider.get_token().await; // force auth before reading roles
   111	        if let Ok(token) = auth_provider.get_token_info().await {
   112	            rbac.update_roles(token.roles, Some(token.project.id));
   113	        }
   114	        let mut module_registry = nexttui::registry::ModuleRegistry::new();
   115	        nexttui::registry::register_all_modules(&mut module_registry, &action_tx);
   116	        let (app, initial_actions) = App::from_registry(config, action_tx.clone(), module_registry, rbac.clone());
   117	
   118	        // Spawn background worker
   119	        tokio::spawn(run_worker(registry, rbac, app.all_tenants.clone(), action_rx, event_tx));
   120	
   121	        // Trigger initial data load
   122	        for action in initial_actions {
   123	            let _ = action_tx.send(action);
   124	        }
   125	
   126	        (app, event_rx, None)
   127	    };
   128	
   129	    // Restore terminal on panic before raw mode corrupts output
   130	    let original_hook = std::panic::take_hook();
   131	    std::panic::set_hook(Box::new(move |info| {
   132	        let _ = disable_raw_mode();
   133	        let _ = execute!(io::stdout(), LeaveAlternateScreen);
   134	        original_hook(info);
   135	    }));
   136	
   137	    // Setup terminal
   138	    enable_raw_mode()?;
   139	    let mut stdout = io::stdout();
   140	    execute!(stdout, EnterAlternateScreen)?;
   141	    let backend = CrosstermBackend::new(stdout);
   142	    let mut terminal = Terminal::new(backend)?;
   143	
   144	    let result = run_event_loop(&mut terminal, &mut app, event_rx).await;
   145	
   146	    // Cleanup terminal
   147	    disable_raw_mode()?;
   148	    execute!(terminal.backend_mut(), LeaveAlternateScreen)?;
   149	    terminal.show_cursor()?;
   150	
   151	    if let Err(e) = result {
   152	        eprintln!("Error: {e}");
   153	        tracing::error!(%e, "event loop error");
   154	        std::process::exit(1);
   155	    }
   156	
   157	    Ok(())
   158	}

exec
/bin/zsh -lc "nl -ba src/port/mock.rs | sed -n '1,420p'" in /Users/jay.ahn/projects/infra/nexttui/.worktrees/runtime-context-switch
 succeeded in 0ms:
     1	use std::collections::HashMap;
     2	
     3	use async_trait::async_trait;
     4	use chrono::{DateTime, Utc};
     5	
     6	use super::error::{ApiError, ApiResult};
     7	use super::types::*;
     8	use crate::models::cinder::{Volume, VolumeSnapshot};
     9	use crate::models::glance::Image;
    10	use crate::models::keystone::{Project, Role, RoleAssignment, User};
    11	use crate::models::neutron::{FloatingIp, Network, NetworkAgent, Port, SecurityGroup, SecurityGroupRule};
    12	use crate::models::nova::{Aggregate, ComputeService, Flavor, Hypervisor, Server};
    13	
    14	// ============================================================
    15	// MockNovaAdapter
    16	// ============================================================
    17	
    18	pub struct MockNovaAdapter;
    19	
    20	#[async_trait]
    21	impl super::nova::NovaPort for MockNovaAdapter {
    22	    async fn list_servers(
    23	        &self,
    24	        _filter: &ServerListFilter,
    25	        _pagination: &PaginationParams,
    26	    ) -> ApiResult<PaginatedResponse<Server>> {
    27	        Ok(PaginatedResponse::empty())
    28	    }
    29	    async fn get_server(&self, server_id: &str) -> ApiResult<Server> {
    30	        Err(ApiError::NotFound {
    31	            resource_type: "server".into(),
    32	            id: server_id.into(),
    33	        })
    34	    }
    35	    async fn create_server(&self, _params: &ServerCreateParams) -> ApiResult<Server> {
    36	        Err(ApiError::BadRequest("mock: not implemented".into()))
    37	    }
    38	    async fn delete_server(&self, _server_id: &str) -> ApiResult<()> {
    39	        Ok(())
    40	    }
    41	    async fn reboot_server(&self, _server_id: &str, _reboot_type: RebootType) -> ApiResult<()> {
    42	        Ok(())
    43	    }
    44	    async fn start_server(&self, _server_id: &str) -> ApiResult<()> {
    45	        Ok(())
    46	    }
    47	    async fn stop_server(&self, _server_id: &str) -> ApiResult<()> {
    48	        Ok(())
    49	    }
    50	    async fn force_set_server_state(&self, _server_id: &str, _state: ServerState) -> ApiResult<()> {
    51	        Ok(())
    52	    }
    53	    async fn create_server_snapshot(
    54	        &self,
    55	        _server_id: &str,
    56	        _image_name: &str,
    57	    ) -> ApiResult<String> {
    58	        Ok("mock-image-id".into())
    59	    }
    60	    async fn list_server_events(&self, _server_id: &str) -> ApiResult<Vec<ServerEvent>> {
    61	        Ok(vec![])
    62	    }
    63	    async fn resize_server(&self, _server_id: &str, _flavor_id: &str) -> ApiResult<()> {
    64	        Ok(())
    65	    }
    66	    async fn live_migrate_server(
    67	        &self,
    68	        _server_id: &str,
    69	        _params: &LiveMigrateParams,
    70	    ) -> ApiResult<()> {
    71	        Ok(())
    72	    }
    73	    async fn cold_migrate_server(&self, _server_id: &str) -> ApiResult<()> {
    74	        Ok(())
    75	    }
    76	    async fn confirm_migration(&self, _server_id: &str) -> ApiResult<()> {
    77	        Ok(())
    78	    }
    79	    async fn revert_migration(&self, _server_id: &str) -> ApiResult<()> {
    80	        Ok(())
    81	    }
    82	    async fn evacuate_server(&self, _server_id: &str, _params: &EvacuateParams) -> ApiResult<()> {
    83	        Ok(())
    84	    }
    85	    async fn list_server_migrations(
    86	        &self,
    87	        _server_id: &str,
    88	    ) -> ApiResult<Vec<crate::models::nova::ServerMigration>> {
    89	        Ok(vec![])
    90	    }
    91	    async fn get_server_migration(
    92	        &self,
    93	        _server_id: &str,
    94	        migration_id: i64,
    95	    ) -> ApiResult<crate::models::nova::ServerMigration> {
    96	        Err(ApiError::NotFound {
    97	            resource_type: "migration".into(),
    98	            id: migration_id.to_string(),
    99	        })
   100	    }
   101	    async fn list_flavors(
   102	        &self,
   103	        _pagination: &PaginationParams,
   104	    ) -> ApiResult<PaginatedResponse<Flavor>> {
   105	        Ok(PaginatedResponse::empty())
   106	    }
   107	    async fn get_flavor(&self, flavor_id: &str) -> ApiResult<Flavor> {
   108	        Err(ApiError::NotFound {
   109	            resource_type: "flavor".into(),
   110	            id: flavor_id.into(),
   111	        })
   112	    }
   113	    async fn create_flavor(&self, _params: &FlavorCreateParams) -> ApiResult<Flavor> {
   114	        Err(ApiError::BadRequest("mock: not implemented".into()))
   115	    }
   116	    async fn delete_flavor(&self, _flavor_id: &str) -> ApiResult<()> {
   117	        Ok(())
   118	    }
   119	    async fn list_aggregates(&self) -> ApiResult<Vec<Aggregate>> {
   120	        Ok(vec![])
   121	    }
   122	    async fn get_aggregate(&self, _aggregate_id: i64) -> ApiResult<Aggregate> {
   123	        Err(ApiError::BadRequest("mock: not implemented".into()))
   124	    }
   125	    async fn create_aggregate(&self, _params: &AggregateCreateParams) -> ApiResult<Aggregate> {
   126	        Err(ApiError::BadRequest("mock: not implemented".into()))
   127	    }
   128	    async fn update_aggregate(
   129	        &self,
   130	        _aggregate_id: i64,
   131	        _params: &AggregateUpdateParams,
   132	    ) -> ApiResult<Aggregate> {
   133	        Err(ApiError::BadRequest("mock: not implemented".into()))
   134	    }
   135	    async fn delete_aggregate(&self, _aggregate_id: i64) -> ApiResult<()> {
   136	        Ok(())
   137	    }
   138	    async fn aggregate_add_host(&self, _aggregate_id: i64, _host: &str) -> ApiResult<Aggregate> {
   139	        Err(ApiError::BadRequest("mock: not implemented".into()))
   140	    }
   141	    async fn aggregate_remove_host(&self, _aggregate_id: i64, _host: &str) -> ApiResult<Aggregate> {
   142	        Err(ApiError::BadRequest("mock: not implemented".into()))
   143	    }
   144	    async fn aggregate_set_metadata(
   145	        &self,
   146	        _aggregate_id: i64,
   147	        _metadata: &HashMap<String, String>,
   148	    ) -> ApiResult<Aggregate> {
   149	        Err(ApiError::BadRequest("mock: not implemented".into()))
   150	    }
   151	    async fn list_compute_services(&self) -> ApiResult<Vec<ComputeService>> {
   152	        Ok(vec![])
   153	    }
   154	    async fn enable_compute_service(&self, _service_id: &str) -> ApiResult<ComputeService> {
   155	        Err(ApiError::BadRequest("mock: not implemented".into()))
   156	    }
   157	    async fn disable_compute_service(
   158	        &self,
   159	        _service_id: &str,
   160	        _reason: Option<&str>,
   161	    ) -> ApiResult<ComputeService> {
   162	        Err(ApiError::BadRequest("mock: not implemented".into()))
   163	    }
   164	    async fn attach_volume(
   165	        &self,
   166	        _server_id: &str,
   167	        _volume_id: &str,
   168	        _device: Option<&str>,
   169	    ) -> ApiResult<()> {
   170	        Ok(())
   171	    }
   172	    async fn detach_volume(&self, _server_id: &str, _volume_id: &str) -> ApiResult<()> {
   173	        Ok(())
   174	    }
   175	    async fn list_hypervisors(&self) -> ApiResult<Vec<Hypervisor>> {
   176	        Ok(vec![])
   177	    }
   178	    async fn get_hypervisor(&self, _hypervisor_id: &str) -> ApiResult<Hypervisor> {
   179	        Err(ApiError::BadRequest("mock: not implemented".into()))
   180	    }
   181	    async fn list_all_tenant_usage(
   182	        &self,
   183	        _start: DateTime<Utc>,
   184	        _end: DateTime<Utc>,
   185	    ) -> ApiResult<Vec<TenantUsage>> {
   186	        Ok(vec![])
   187	    }
   188	    async fn get_project_usage(
   189	        &self,
   190	        _project_id: &str,
   191	        _start: DateTime<Utc>,
   192	        _end: DateTime<Utc>,
   193	    ) -> ApiResult<ProjectUsage> {
   194	        Ok(ProjectUsage {
   195	            total_vcpus_usage: 0.0,
   196	            total_memory_mb_usage: 0.0,
   197	            total_local_gb_usage: 0.0,
   198	        })
   199	    }
   200	    async fn get_compute_quota(&self, _project_id: &str) -> ApiResult<ComputeQuota> {
   201	        Ok(ComputeQuota {
   202	            cores: 20,
   203	            ram: 51200,
   204	            instances: 10,
   205	        })
   206	    }
   207	    async fn update_compute_quota(
   208	        &self,
   209	        _project_id: &str,
   210	        _params: &ComputeQuotaUpdateParams,
   211	    ) -> ApiResult<ComputeQuota> {
   212	        Ok(ComputeQuota {
   213	            cores: 20,
   214	            ram: 51200,
   215	            instances: 10,
   216	        })
   217	    }
   218	}
   219	
   220	// ============================================================
   221	// MockNeutronAdapter
   222	// ============================================================
   223	
   224	pub struct MockNeutronAdapter;
   225	
   226	#[async_trait]
   227	impl super::neutron::NeutronPort for MockNeutronAdapter {
   228	    async fn list_networks(
   229	        &self,
   230	        _filter: &NetworkListFilter,
   231	        _pagination: &PaginationParams,
   232	    ) -> ApiResult<PaginatedResponse<Network>> {
   233	        Ok(PaginatedResponse::empty())
   234	    }
   235	    async fn get_network(&self, network_id: &str) -> ApiResult<Network> {
   236	        Err(ApiError::NotFound {
   237	            resource_type: "network".into(),
   238	            id: network_id.into(),
   239	        })
   240	    }
   241	    async fn create_network(&self, _params: &NetworkCreateParams) -> ApiResult<Network> {
   242	        Err(ApiError::BadRequest("mock: not implemented".into()))
   243	    }
   244	    async fn update_network(
   245	        &self,
   246	        _network_id: &str,
   247	        _params: &NetworkUpdateParams,
   248	    ) -> ApiResult<Network> {
   249	        Err(ApiError::BadRequest("mock: not implemented".into()))
   250	    }
   251	    async fn delete_network(&self, _network_id: &str) -> ApiResult<()> {
   252	        Ok(())
   253	    }
   254	    async fn list_subnets(&self, _network_id: Option<&str>) -> ApiResult<Vec<Subnet>> {
   255	        Ok(vec![])
   256	    }
   257	    async fn list_security_groups(
   258	        &self,
   259	        _filter: &SecurityGroupListFilter,
   260	        _pagination: &PaginationParams,
   261	    ) -> ApiResult<PaginatedResponse<SecurityGroup>> {
   262	        Ok(PaginatedResponse::empty())
   263	    }
   264	    async fn get_security_group(&self, sg_id: &str) -> ApiResult<SecurityGroup> {
   265	        Err(ApiError::NotFound {
   266	            resource_type: "security_group".into(),
   267	            id: sg_id.into(),
   268	        })
   269	    }
   270	    async fn create_security_group(
   271	        &self,
   272	        _params: &SecurityGroupCreateParams,
   273	    ) -> ApiResult<SecurityGroup> {
   274	        Err(ApiError::BadRequest("mock: not implemented".into()))
   275	    }
   276	    async fn update_security_group(
   277	        &self,
   278	        _sg_id: &str,
   279	        _params: &SecurityGroupUpdateParams,
   280	    ) -> ApiResult<SecurityGroup> {
   281	        Err(ApiError::BadRequest("mock: not implemented".into()))
   282	    }
   283	    async fn delete_security_group(&self, _sg_id: &str) -> ApiResult<()> {
   284	        Ok(())
   285	    }
   286	    async fn create_security_group_rule(
   287	        &self,
   288	        _params: &SecurityGroupRuleCreateParams,
   289	    ) -> ApiResult<SecurityGroupRule> {
   290	        Err(ApiError::BadRequest("mock: not implemented".into()))
   291	    }
   292	    async fn delete_security_group_rule(&self, _rule_id: &str) -> ApiResult<()> {
   293	        Ok(())
   294	    }
   295	    async fn list_floating_ips(
   296	        &self,
   297	        _filter: &FloatingIpListFilter,
   298	        _pagination: &PaginationParams,
   299	    ) -> ApiResult<PaginatedResponse<FloatingIp>> {
   300	        Ok(PaginatedResponse::empty())
   301	    }
   302	    async fn create_floating_ip(&self, _params: &FloatingIpCreateParams) -> ApiResult<FloatingIp> {
   303	        Err(ApiError::BadRequest("mock: not implemented".into()))
   304	    }
   305	    async fn delete_floating_ip(&self, _fip_id: &str) -> ApiResult<()> {
   306	        Ok(())
   307	    }
   308	    async fn associate_floating_ip(&self, _fip_id: &str, _port_id: &str) -> ApiResult<FloatingIp> {
   309	        Err(ApiError::BadRequest("mock: not implemented".into()))
   310	    }
   311	    async fn disassociate_floating_ip(&self, _fip_id: &str) -> ApiResult<FloatingIp> {
   312	        Err(ApiError::BadRequest("mock: not implemented".into()))
   313	    }
   314	    async fn list_ports(&self, _device_id: &str) -> ApiResult<Vec<Port>> {
   315	        Ok(vec![])
   316	    }
   317	    async fn list_network_agents(&self) -> ApiResult<Vec<NetworkAgent>> {
   318	        Ok(vec![])
   319	    }
   320	    async fn enable_network_agent(&self, _agent_id: &str) -> ApiResult<NetworkAgent> {
   321	        Err(ApiError::BadRequest("mock: not implemented".into()))
   322	    }
   323	    async fn disable_network_agent(&self, _agent_id: &str) -> ApiResult<NetworkAgent> {
   324	        Err(ApiError::BadRequest("mock: not implemented".into()))
   325	    }
   326	    async fn delete_network_agent(&self, _agent_id: &str) -> ApiResult<()> {
   327	        Ok(())
   328	    }
   329	}
   330	
   331	// ============================================================
   332	// MockCinderAdapter
   333	// ============================================================
   334	
   335	pub struct MockCinderAdapter;
   336	
   337	#[async_trait]
   338	impl super::cinder::CinderPort for MockCinderAdapter {
   339	    async fn list_volumes(
   340	        &self,
   341	        _filter: &VolumeListFilter,
   342	        _pagination: &PaginationParams,
   343	    ) -> ApiResult<PaginatedResponse<Volume>> {
   344	        Ok(PaginatedResponse::empty())
   345	    }
   346	    async fn get_volume(&self, volume_id: &str) -> ApiResult<Volume> {
   347	        Err(ApiError::NotFound {
   348	            resource_type: "volume".into(),
   349	            id: volume_id.into(),
   350	        })
   351	    }
   352	    async fn create_volume(&self, _params: &VolumeCreateParams) -> ApiResult<Volume> {
   353	        Err(ApiError::BadRequest("mock: not implemented".into()))
   354	    }
   355	    async fn delete_volume(&self, _volume_id: &str) -> ApiResult<()> {
   356	        Ok(())
   357	    }
   358	    async fn force_delete_volume(&self, _volume_id: &str) -> ApiResult<()> {
   359	        Ok(())
   360	    }
   361	    async fn extend_volume(&self, _volume_id: &str, _new_size_gb: u32) -> ApiResult<()> {
   362	        Ok(())
   363	    }
   364	    async fn attach_volume(
   365	        &self,
   366	        _volume_id: &str,
   367	        _server_id: &str,
   368	        _device: Option<&str>,
   369	    ) -> ApiResult<()> {
   370	        Ok(())
   371	    }
   372	    async fn detach_volume(&self, _volume_id: &str, _attachment_id: &str) -> ApiResult<()> {
   373	        Ok(())
   374	    }
   375	    async fn force_detach_volume(&self, _volume_id: &str, _attachment_id: &str) -> ApiResult<()> {
   376	        Ok(())
   377	    }
   378	    async fn force_set_volume_state(&self, _volume_id: &str, _state: &str) -> ApiResult<()> {
   379	        Ok(())
   380	    }
   381	    async fn migrate_volume(
   382	        &self,
   383	        _volume_id: &str,
   384	        _dest_host: &str,
   385	        _force_host_copy: bool,
   386	    ) -> ApiResult<()> {
   387	        Ok(())
   388	    }
   389	    async fn list_snapshots(
   390	        &self,
   391	        _filter: &SnapshotListFilter,
   392	        _pagination: &PaginationParams,
   393	    ) -> ApiResult<PaginatedResponse<VolumeSnapshot>> {
   394	        Ok(PaginatedResponse::empty())
   395	    }
   396	    async fn get_snapshot(&self, snapshot_id: &str) -> ApiResult<VolumeSnapshot> {
   397	        Err(ApiError::NotFound {
   398	            resource_type: "snapshot".into(),
   399	            id: snapshot_id.into(),
   400	        })
   401	    }
   402	    async fn create_snapshot(&self, _params: &SnapshotCreateParams) -> ApiResult<VolumeSnapshot> {
   403	        Err(ApiError::BadRequest("mock: not implemented".into()))
   404	    }
   405	    async fn delete_snapshot(&self, _snapshot_id: &str) -> ApiResult<()> {
   406	        Ok(())
   407	    }
   408	    async fn list_qos_specs(&self) -> ApiResult<Vec<QosSpec>> {
   409	        Ok(vec![])
   410	    }
   411	    async fn get_qos_spec(&self, _qos_id: &str) -> ApiResult<QosSpec> {
   412	        Err(ApiError::BadRequest("mock: not implemented".into()))
   413	    }
   414	    async fn create_qos_spec(&self, _params: &QosCreateParams) -> ApiResult<QosSpec> {
   415	        Err(ApiError::BadRequest("mock: not implemented".into()))
   416	    }
   417	    async fn update_qos_spec(
   418	        &self,
   419	        _qos_id: &str,
   420	        _specs: &HashMap<String, String>,

exec
/bin/zsh -lc "rg -n \"MockAuthProvider|impl super::auth::AuthProvider|subscribe_token_refresh|authenticate\\(\" src/port/mock.rs | sed -n '1,200p'" in /Users/jay.ahn/projects/infra/nexttui/.worktrees/runtime-context-switch
 succeeded in 0ms:
exec
/bin/zsh -lc "rg -n \"MockAuth|mock auth|AuthProvider for\" src/port src -g '*.rs' | sed -n '1,260p'" in /Users/jay.ahn/projects/infra/nexttui/.worktrees/runtime-context-switch
 succeeded in 0ms:
src/adapter/auth/keystone.rs:319:impl AuthProvider for KeystoneAuthAdapter {

codex
I found no existing `MockAuthProvider` implementation in the current tree, so I’m checking the event loop/dispatcher to locate where an epoch check would realistically live and whether the DETAIL text pins that location precisely enough.
exec
/bin/zsh -lc "nl -ba src/event_loop.rs | sed -n '1,360p'" in /Users/jay.ahn/projects/infra/nexttui/.worktrees/runtime-context-switch
 succeeded in 0ms:
     1	use std::io::Stdout;
     2	use std::time::Duration;
     3	
     4	use crossterm::event::{Event, EventStream};
     5	use futures::StreamExt;
     6	use ratatui::Terminal;
     7	use ratatui::backend::CrosstermBackend;
     8	use tokio::sync::mpsc;
     9	
    10	use crate::app::App;
    11	use crate::error::Result;
    12	use crate::event::AppEvent;
    13	
    14	/// Main event loop — runs until App.should_quit becomes true.
    15	pub async fn run_event_loop(
    16	    terminal: &mut Terminal<CrosstermBackend<Stdout>>,
    17	    app: &mut App,
    18	    mut event_rx: mpsc::UnboundedReceiver<AppEvent>,
    19	) -> Result<()> {
    20	    let mut key_events = EventStream::new();
    21	    let mut tick = tokio::time::interval(Duration::from_millis(200));
    22	
    23	    loop {
    24	        tokio::select! {
    25	            // Branch 1: key input
    26	            key_result = key_events.next() => {
    27	                match key_result {
    28	                    Some(Ok(Event::Key(key))) if key.kind == crossterm::event::KeyEventKind::Press => {
    29	                        app.handle_key(key);
    30	                    }
    31	                    Some(Ok(Event::Resize(_, _))) => {
    32	                        // Resize triggers immediate re-render (handled below)
    33	                    }
    34	                    Some(Ok(_)) => {}
    35	                    Some(Err(_)) | None => {
    36	                        // Key stream ended or errored — exit gracefully
    37	                        app.should_quit = true;
    38	                    }
    39	                }
    40	            }
    41	
    42	            // Branch 2: tick timer
    43	            _ = tick.tick() => {
    44	                app.on_tick();
    45	            }
    46	
    47	            // Branch 3: background events
    48	            event = event_rx.recv() => {
    49	                match event {
    50	                    Some(ev) => {
    51	                        app.handle_event(ev);
    52	                    }
    53	                    None => {
    54	                        // All event senders dropped — exit gracefully
    55	                        app.should_quit = true;
    56	                    }
    57	                }
    58	            }
    59	        }
    60	
    61	        // Render
    62	        terminal.draw(|f| app.render(f))?;
    63	
    64	        // Check quit
    65	        if app.should_quit {
    66	            break;
    67	        }
    68	    }
    69	
    70	    Ok(())
    71	}

exec
/bin/zsh -lc "rg -n \"fn handle_event\\(&mut self, event: &AppEvent\\)|match event|_ => \\{\\}\" src/module src/ui src | sed -n '1,400p'" in /Users/jay.ahn/projects/infra/nexttui/.worktrees/runtime-context-switch
 succeeded in 0ms:
src/event_loop.rs:49:                match event {
src/app.rs:189:                _ => {}
src/app.rs:293:                _ => {}
src/app.rs:559:        let (action, resource_type, resource_id, resource_name, result) = match event {
src/app.rs:666:        let (msg, level, operation, resource_name) = match event {
src/module/volume/mod.rs:576:    fn handle_event(&mut self, event: &AppEvent) {
src/module/volume/mod.rs:577:        match event {
src/module/volume/mod.rs:613:            _ => {}
src/ui/form.rs:420:                    _ => {}
src/ui/form.rs:459:                _ => {}
src/ui/form.rs:682:                        _ => {}
src/ui/form.rs:742:                _ => {}
src/ui/form.rs:798:                    _ => {}
src/ui/form.rs:830:                    _ => {}
src/ui/form.rs:833:            _ => {}
src/ui/form.rs:1177:            _ => {}
src/module/flavor/mod.rs:200:    fn handle_event(&mut self, event: &AppEvent) {
src/module/flavor/mod.rs:201:        match event {
src/module/flavor/mod.rs:222:            _ => {}
src/module/compute_service/mod.rs:46:    fn handle_event(&mut self, event: &AppEvent) {
src/component.rs:16:    fn handle_event(&mut self, event: &AppEvent);
src/event.rs:145:        match event {
src/event.rs:221:        match event {
src/event.rs:286:        match event {
src/event.rs:298:        match event {
src/module/security_group/mod.rs:167:            _ => {}
src/module/security_group/mod.rs:346:    fn handle_event(&mut self, event: &AppEvent) {
src/module/security_group/mod.rs:347:        match event {
src/module/security_group/mod.rs:367:            _ => {}
src/module/network/mod.rs:198:    fn handle_event(&mut self, event: &AppEvent) {
src/module/network/mod.rs:199:        match event {
src/module/network/mod.rs:224:            _ => {}
src/worker.rs:944:        match event {
src/worker.rs:1070:        match event {
src/module/project/mod.rs:189:    fn handle_event(&mut self, event: &AppEvent) {
src/module/project/mod.rs:190:        match event {
src/module/project/mod.rs:220:            _ => {}
src/module/agent/mod.rs:47:    fn handle_event(&mut self, event: &AppEvent) {
src/module/volume/mod.rs:576:    fn handle_event(&mut self, event: &AppEvent) {
src/module/volume/mod.rs:577:        match event {
src/module/volume/mod.rs:613:            _ => {}
src/module/aggregate/mod.rs:46:    fn handle_event(&mut self, event: &AppEvent) {
src/module/image/mod.rs:232:    fn handle_event(&mut self, event: &AppEvent) {
src/module/image/mod.rs:233:        match event {
src/module/image/mod.rs:254:            _ => {}
src/input/command.rs:113:            _ => {}
src/module/server/mod.rs:876:    fn handle_event(&mut self, event: &AppEvent) {
src/module/server/mod.rs:877:        match event {
src/module/server/mod.rs:986:            _ => {}
src/background.rs:70:        match event {
src/module/floating_ip/mod.rs:398:    fn handle_event(&mut self, event: &AppEvent) {
src/module/floating_ip/mod.rs:399:        match event {
src/module/floating_ip/mod.rs:456:            _ => {}
src/module/host/mod.rs:156:            _ => {}
src/module/host/mod.rs:169:                _ => {}
src/module/host/mod.rs:222:                _ => {}
src/module/host/mod.rs:228:    fn handle_event(&mut self, event: &AppEvent) {
src/module/host/mod.rs:229:        match event {
src/module/host/mod.rs:249:            _ => {}
src/module/user/mod.rs:180:    fn handle_event(&mut self, event: &AppEvent) {
src/module/user/mod.rs:181:        match event {
src/module/user/mod.rs:211:            _ => {}
src/module/user/mod.rs:180:    fn handle_event(&mut self, event: &AppEvent) {
src/module/user/mod.rs:181:        match event {
src/module/user/mod.rs:211:            _ => {}
src/module/aggregate/mod.rs:46:    fn handle_event(&mut self, event: &AppEvent) {
src/module/project/mod.rs:189:    fn handle_event(&mut self, event: &AppEvent) {
src/module/project/mod.rs:190:        match event {
src/module/project/mod.rs:220:            _ => {}
src/ui/form.rs:420:                    _ => {}
src/ui/form.rs:459:                _ => {}
src/ui/form.rs:682:                        _ => {}
src/ui/form.rs:742:                _ => {}
src/ui/form.rs:798:                    _ => {}
src/ui/form.rs:830:                    _ => {}
src/ui/form.rs:833:            _ => {}
src/ui/form.rs:1177:            _ => {}
src/module/flavor/mod.rs:200:    fn handle_event(&mut self, event: &AppEvent) {
src/module/flavor/mod.rs:201:        match event {
src/module/flavor/mod.rs:222:            _ => {}
src/module/snapshot/mod.rs:141:    fn handle_event(&mut self, event: &AppEvent) {
src/module/snapshot/mod.rs:142:        match event {
src/module/snapshot/mod.rs:159:            _ => {}
src/module/usage/mod.rs:551:    fn handle_event(&mut self, event: &AppEvent) {
src/module/usage/mod.rs:552:        match event {
src/module/usage/mod.rs:570:            _ => {}
src/infra/audit.rs:174:            _ => {}
src/module/snapshot/mod.rs:141:    fn handle_event(&mut self, event: &AppEvent) {
src/module/snapshot/mod.rs:142:        match event {
src/module/snapshot/mod.rs:159:            _ => {}
src/module/usage/mod.rs:551:    fn handle_event(&mut self, event: &AppEvent) {
src/module/usage/mod.rs:552:        match event {
src/module/usage/mod.rs:570:            _ => {}
src/module/security_group/mod.rs:167:            _ => {}
src/module/security_group/mod.rs:346:    fn handle_event(&mut self, event: &AppEvent) {
src/module/security_group/mod.rs:347:        match event {
src/module/security_group/mod.rs:367:            _ => {}
src/module/image/mod.rs:232:    fn handle_event(&mut self, event: &AppEvent) {
src/module/image/mod.rs:233:        match event {
src/module/image/mod.rs:254:            _ => {}
src/module/host/mod.rs:156:            _ => {}
src/module/host/mod.rs:169:                _ => {}
src/module/host/mod.rs:222:                _ => {}
src/module/host/mod.rs:228:    fn handle_event(&mut self, event: &AppEvent) {
src/module/host/mod.rs:229:        match event {
src/module/host/mod.rs:249:            _ => {}
src/module/compute_service/mod.rs:46:    fn handle_event(&mut self, event: &AppEvent) {
src/module/server/mod.rs:876:    fn handle_event(&mut self, event: &AppEvent) {
src/module/server/mod.rs:877:        match event {
src/module/server/mod.rs:986:            _ => {}
src/module/agent/mod.rs:47:    fn handle_event(&mut self, event: &AppEvent) {
src/module/floating_ip/mod.rs:398:    fn handle_event(&mut self, event: &AppEvent) {
src/module/floating_ip/mod.rs:399:        match event {
src/module/floating_ip/mod.rs:456:            _ => {}
src/module/network/mod.rs:198:    fn handle_event(&mut self, event: &AppEvent) {
src/module/network/mod.rs:199:        match event {
src/module/network/mod.rs:224:            _ => {}

exec
/bin/zsh -lc "nl -ba src/module/compute_service/mod.rs | sed -n '1,180p'" in /Users/jay.ahn/projects/infra/nexttui/.worktrees/runtime-context-switch
 succeeded in 0ms:
     1	pub mod view_model;
     2	
     3	use crossterm::event::{KeyCode, KeyEvent};
     4	use ratatui::layout::Rect;
     5	use ratatui::Frame;
     6	
     7	use crate::action::Action;
     8	use crate::component::Component;
     9	use crate::event::AppEvent;
    10	use crate::models::nova::ComputeService;
    11	use crate::ui::resource_list::{ResourceList, Row};
    12	
    13	use self::view_model::{compute_service_columns, compute_service_to_row};
    14	
    15	pub struct ComputeServiceModule {
    16	    services: Vec<ComputeService>,
    17	    #[allow(dead_code)]
    18	    loading: bool,
    19	    resource_list: ResourceList,
    20	}
    21	
    22	impl ComputeServiceModule {
    23	    pub fn new() -> Self {
    24	        Self {
    25	            services: Vec::new(),
    26	            loading: false,
    27	            resource_list: ResourceList::new(compute_service_columns()),
    28	        }
    29	    }
    30	    pub fn services(&self) -> &[ComputeService] { &self.services }
    31	    fn rows(&self) -> Vec<Row> { self.services.iter().map(compute_service_to_row).collect() }
    32	}
    33	
    34	impl Component for ComputeServiceModule {
    35	    fn refresh_action(&self) -> Option<Action> { Some(Action::FetchComputeServices) }
    36	
    37	    fn handle_key(&mut self, key: KeyEvent) -> Option<Action> {
    38	        if self.resource_list.handle_nav_key(key) { return None; }
    39	        match key.code {
    40	            KeyCode::Char('r') => Some(Action::FetchComputeServices),
    41	            KeyCode::Left => Some(Action::FocusSidebar),
    42	            KeyCode::Esc => Some(Action::Back),
    43	            _ => None,
    44	        }
    45	    }
    46	    fn handle_event(&mut self, event: &AppEvent) {
    47	        if let AppEvent::ComputeServicesLoaded(svcs) = event {
    48	            self.services = svcs.clone();
    49	            self.loading = false;
    50	            let rows = self.rows();
    51	            self.resource_list.set_rows(rows);
    52	        }
    53	    }
    54	    fn render(&self, frame: &mut Frame, area: Rect) {
    55	        self.resource_list.render(frame, area);
    56	    }
    57	
    58	    fn help_hint(&self) -> &str { "r:Refresh" }
    59	}
    60	
    61	#[cfg(test)]
    62	mod tests {
    63	    use super::*;
    64	    fn key(code: KeyCode) -> KeyEvent { KeyEvent::from(code) }
    65	
    66	    #[test] fn test_initial() { let m = ComputeServiceModule::new(); assert!(m.services().is_empty()); }
    67	    #[test] fn test_refresh() {
    68	        let mut m = ComputeServiceModule::new();
    69	        assert!(matches!(m.handle_key(key(KeyCode::Char('r'))), Some(Action::FetchComputeServices)));
    70	    }
    71	    #[test] fn test_event_loaded() {
    72	        let mut m = ComputeServiceModule::new();
    73	        m.handle_event(&AppEvent::ComputeServicesLoaded(vec![
    74	            ComputeService { id: "s1".into(), binary: "nova-compute".into(), host: "node1".into(), state: "up".into(), status: "enabled".into(), updated_at: None, disabled_reason: None },
    75	        ]));
    76	        assert_eq!(m.services().len(), 1);
    77	    }
    78	
    79	    #[test]
    80	    fn test_help_hint() {
    81	        let m = ComputeServiceModule::new();
    82	        assert_eq!(m.help_hint(), "r:Refresh");
    83	    }
    84	}

exec
/bin/zsh -lc "nl -ba src/module/agent/mod.rs | sed -n '1,180p'" in /Users/jay.ahn/projects/infra/nexttui/.worktrees/runtime-context-switch
 succeeded in 0ms:
     1	pub mod view_model;
     2	
     3	use crossterm::event::{KeyCode, KeyEvent};
     4	use ratatui::layout::Rect;
     5	use ratatui::Frame;
     6	
     7	use crate::action::Action;
     8	use crate::component::Component;
     9	use crate::event::AppEvent;
    10	use crate::models::neutron::NetworkAgent;
    11	use crate::ui::resource_list::{ResourceList, Row};
    12	
    13	use self::view_model::{agent_columns, agent_to_row};
    14	
    15	pub struct AgentModule {
    16	    agents: Vec<NetworkAgent>,
    17	    #[allow(dead_code)]
    18	    loading: bool,
    19	    resource_list: ResourceList,
    20	}
    21	
    22	impl AgentModule {
    23	    pub fn new() -> Self {
    24	        Self {
    25	            agents: Vec::new(),
    26	            loading: false,
    27	            resource_list: ResourceList::new(agent_columns()),
    28	        }
    29	    }
    30	    pub fn agents(&self) -> &[NetworkAgent] { &self.agents }
    31	    pub fn selected_index(&self) -> usize { self.resource_list.selected_index() }
    32	    fn rows(&self) -> Vec<Row> { self.agents.iter().map(agent_to_row).collect() }
    33	}
    34	
    35	impl Component for AgentModule {
    36	    fn refresh_action(&self) -> Option<Action> { Some(Action::FetchAgents) }
    37	
    38	    fn handle_key(&mut self, key: KeyEvent) -> Option<Action> {
    39	        if self.resource_list.handle_nav_key(key) { return None; }
    40	        match key.code {
    41	            KeyCode::Char('r') => Some(Action::FetchAgents),
    42	            KeyCode::Left => Some(Action::FocusSidebar),
    43	            KeyCode::Esc => Some(Action::Back),
    44	            _ => None,
    45	        }
    46	    }
    47	    fn handle_event(&mut self, event: &AppEvent) {
    48	        if let AppEvent::AgentsLoaded(agents) = event {
    49	            self.agents = agents.clone();
    50	            self.loading = false;
    51	            let rows = self.rows();
    52	            self.resource_list.set_rows(rows);
    53	        }
    54	    }
    55	    fn render(&self, frame: &mut Frame, area: Rect) {
    56	        self.resource_list.render(frame, area);
    57	    }
    58	
    59	    fn help_hint(&self) -> &str { "r:Refresh" }
    60	}
    61	
    62	#[cfg(test)]
    63	mod tests {
    64	    use super::*;
    65	    fn key(code: KeyCode) -> KeyEvent { KeyEvent::from(code) }
    66	
    67	    fn make_agent(id: &str, agent_type: &str) -> NetworkAgent {
    68	        NetworkAgent {
    69	            id: id.into(), agent_type: agent_type.into(), host: "network-01".into(),
    70	            admin_state_up: true, alive: true, binary: "neutron-agent".into(),
    71	        }
    72	    }
    73	
    74	    #[test] fn test_initial() { let m = AgentModule::new(); assert!(m.agents().is_empty()); }
    75	    #[test] fn test_nav() {
    76	        let mut m = AgentModule::new();
    77	        m.handle_event(&AppEvent::AgentsLoaded(vec![make_agent("a1", "OVS"), make_agent("a2", "L3")]));
    78	        m.handle_key(key(KeyCode::Char('j')));
    79	        assert_eq!(m.selected_index(), 1);
    80	    }
    81	    #[test] fn test_refresh() {
    82	        let mut m = AgentModule::new();
    83	        assert!(matches!(m.handle_key(key(KeyCode::Char('r'))), Some(Action::FetchAgents)));
    84	    }
    85	    #[test] fn test_event_loaded() {
    86	        let mut m = AgentModule::new();
    87	        m.handle_event(&AppEvent::AgentsLoaded(vec![make_agent("a1", "OVS")]));
    88	        assert_eq!(m.agents().len(), 1);
    89	    }
    90	
    91	    #[test]
    92	    fn test_help_hint() {
    93	        let m = AgentModule::new();
    94	        assert_eq!(m.help_hint(), "r:Refresh");
    95	    }
    96	}

exec
/bin/zsh -lc "nl -ba src/module/aggregate/mod.rs | sed -n '1,220p'" in /Users/jay.ahn/projects/infra/nexttui/.worktrees/runtime-context-switch
 succeeded in 0ms:
     1	pub mod view_model;
     2	
     3	use crossterm::event::{KeyCode, KeyEvent};
     4	use ratatui::layout::Rect;
     5	use ratatui::Frame;
     6	
     7	use crate::action::Action;
     8	use crate::component::Component;
     9	use crate::event::AppEvent;
    10	use crate::models::nova::Aggregate;
    11	use crate::ui::resource_list::{ResourceList, Row};
    12	
    13	use self::view_model::{aggregate_columns, aggregate_to_row};
    14	
    15	pub struct AggregateModule {
    16	    aggregates: Vec<Aggregate>,
    17	    #[allow(dead_code)]
    18	    loading: bool,
    19	    resource_list: ResourceList,
    20	}
    21	
    22	impl AggregateModule {
    23	    pub fn new() -> Self {
    24	        Self {
    25	            aggregates: Vec::new(),
    26	            loading: false,
    27	            resource_list: ResourceList::new(aggregate_columns()),
    28	        }
    29	    }
    30	    pub fn aggregates(&self) -> &[Aggregate] { &self.aggregates }
    31	    fn rows(&self) -> Vec<Row> { self.aggregates.iter().map(aggregate_to_row).collect() }
    32	}
    33	
    34	impl Component for AggregateModule {
    35	    fn refresh_action(&self) -> Option<Action> { Some(Action::FetchAggregates) }
    36	
    37	    fn handle_key(&mut self, key: KeyEvent) -> Option<Action> {
    38	        if self.resource_list.handle_nav_key(key) { return None; }
    39	        match key.code {
    40	            KeyCode::Char('r') => Some(Action::FetchAggregates),
    41	            KeyCode::Left => Some(Action::FocusSidebar),
    42	            KeyCode::Esc => Some(Action::Back),
    43	            _ => None,
    44	        }
    45	    }
    46	    fn handle_event(&mut self, event: &AppEvent) {
    47	        if let AppEvent::AggregatesLoaded(aggs) = event {
    48	            self.aggregates = aggs.clone();
    49	            self.loading = false;
    50	            let rows = self.rows();
    51	            self.resource_list.set_rows(rows);
    52	        }
    53	    }
    54	    fn render(&self, frame: &mut Frame, area: Rect) {
    55	        self.resource_list.render(frame, area);
    56	    }
    57	
    58	    fn help_hint(&self) -> &str { "r:Refresh" }
    59	}
    60	
    61	#[cfg(test)]
    62	mod tests {
    63	    use super::*;
    64	    fn key(code: KeyCode) -> KeyEvent { KeyEvent::from(code) }
    65	
    66	    #[test] fn test_initial() { let m = AggregateModule::new(); assert!(m.aggregates().is_empty()); }
    67	    #[test] fn test_refresh() {
    68	        let mut m = AggregateModule::new();
    69	        assert!(matches!(m.handle_key(key(KeyCode::Char('r'))), Some(Action::FetchAggregates)));
    70	    }
    71	    #[test] fn test_event_loaded() {
    72	        let mut m = AggregateModule::new();
    73	        m.handle_event(&AppEvent::AggregatesLoaded(vec![
    74	            Aggregate { id: 1, name: "agg1".into(), availability_zone: Some("az1".into()), hosts: vec!["h1".into()], metadata: Default::default() },
    75	        ]));
    76	        assert_eq!(m.aggregates().len(), 1);
    77	    }
    78	
    79	    #[test]
    80	    fn test_help_hint() {
    81	        let m = AggregateModule::new();
    82	        assert_eq!(m.help_hint(), "r:Refresh");
    83	    }
    84	}

exec
/bin/zsh -lc "nl -ba src/module/server/mod.rs | sed -n '860,1040p'" in /Users/jay.ahn/projects/infra/nexttui/.worktrees/runtime-context-switch
 succeeded in 0ms:
   860	        self.resource_list = ResourceList::new(server_columns_full(self.all_tenants, is_admin));
   861	    }
   862	
   863	    fn handle_key(&mut self, key: KeyEvent) -> Option<Action> {
   864	        // ConfirmHandler takes priority
   865	        if let Some(result) = self.confirm.handle_key(key, Self::resolve_action) {
   866	            return result;
   867	        }
   868	
   869	        match &self.view_state {
   870	            ViewState::List => self.handle_list_key(key),
   871	            ViewState::Detail(_) => self.handle_detail_key(key),
   872	            ViewState::Create => self.handle_create_key(key),
   873	        }
   874	    }
   875	
   876	    fn handle_event(&mut self, event: &AppEvent) {
   877	        match event {
   878	            AppEvent::ServersLoaded(servers) => {
   879	                self.servers = servers.clone();
   880	                self.loading = false;
   881	                self.error_message = None;
   882	                let rows = self.rows();
   883	                self.resource_list.set_rows(rows);
   884	                // Clear stale resize_pending if server is no longer VERIFY_RESIZE
   885	                if let Some(ref rp) = self.resize_pending {
   886	                    let still_verify = servers.iter()
   887	                        .find(|s| s.id == rp.server_id)
   888	                        .is_some_and(|s| s.status == "VERIFY_RESIZE");
   889	                    if !still_verify {
   890	                        self.resize_pending = None;
   891	                    }
   892	                }
   893	            }
   894	            AppEvent::ServerDeleted { .. }
   895	            | AppEvent::ServerRebooted { .. }
   896	            | AppEvent::ServerStarted { .. }
   897	            | AppEvent::ServerStopped { .. }
   898	            | AppEvent::ServerCreated(_) => {
   899	                let _ = self.action_tx.send(Action::FetchServers);
   900	            }
   901	            AppEvent::ServerResized { id } => {
   902	                self.resize_pending = Some(ResizePendingInfo {
   903	                    server_id: id.clone(),
   904	                });
   905	            }
   906	            AppEvent::ResizeConfirmed { .. } | AppEvent::ResizeReverted { .. } => {
   907	                self.resize_pending = None;
   908	            }
   909	            AppEvent::MigrationProgressLoaded { server_id, migration } => {
   910	                self.migration_progress = Some((server_id.clone(), migration.clone()));
   911	            }
   912	            AppEvent::ServerLiveMigrated { .. }
   913	            | AppEvent::MigrationConfirmed { .. }
   914	            | AppEvent::MigrationReverted { .. }
   915	            | AppEvent::ServerEvacuated { .. }
   916	            | AppEvent::MigrationPollingStopped { .. } => {
   917	                self.migration_progress = None;
   918	            }
   919	            AppEvent::FlavorsLoaded(flavors) => {
   920	                self.cached_flavors = flavors.clone();
   921	                let opts: Vec<SelectOption> = flavors
   922	                    .iter()
   923	                    .map(|f| SelectOption::new(&f.id, format!("{} ({}vCPU/{}MB/{}GB)", f.name, f.vcpus, f.ram, f.disk)))
   924	                    .collect();
   925	                self.cached_flavor_opts = opts.clone();
   926	                if let Some(form) = &mut self.form {
   927	                    form.set_field_options("Flavor", opts);
   928	                }
   929	            }
   930	            AppEvent::ImagesLoaded(images) => {
   931	                let opts: Vec<SelectOption> = images
   932	                    .iter()
   933	                    .map(|img| SelectOption::new(&img.id, &img.name))
   934	                    .collect();
   935	                self.cached_image_opts = opts.clone();
   936	                if let Some(form) = &mut self.form {
   937	                    form.set_field_options("Image", opts);
   938	                }
   939	            }
   940	            AppEvent::NetworksLoaded(networks) => {
   941	                let opts: Vec<SelectOption> = networks
   942	                    .iter()
   943	                    .map(|n| SelectOption::new(&n.id, &n.name))
   944	                    .collect();
   945	                self.cached_network_opts = opts.clone();
   946	                if let Some(form) = &mut self.form {
   947	                    form.set_field_options("Network", opts);
   948	                }
   949	            }
   950	            AppEvent::SecurityGroupsLoaded(sgs) => {
   951	                let opts: Vec<SelectOption> = sgs
   952	                    .iter()
   953	                    .map(|sg| SelectOption::new(&sg.id, &sg.name))
   954	                    .collect();
   955	                self.cached_sg_opts = opts.clone();
   956	                if let Some(form) = &mut self.form {
   957	                    form.set_field_options("Security Group", opts);
   958	                }
   959	            }
   960	            AppEvent::VolumesLoaded(volumes) => {
   961	                self.cached_volumes = volumes.clone();
   962	            }
   963	            AppEvent::FloatingIpsLoaded(fips) => {
   964	                self.cached_floating_ips = fips.clone();
   965	            }
   966	            AppEvent::PortsLoaded { server_id, ports } => {
   967	                // Only consume if this response matches our pending request
   968	                if self.pending_fip_id.is_some()
   969	                    && self.pending_ports_server_id.as_deref() == Some(server_id.as_str())
   970	                {
   971	                    self.handle_ports_loaded(ports.clone());
   972	                }
   973	            }
   974	            AppEvent::VolumeAttached { .. }
   975	            | AppEvent::VolumeDetached { .. }
   976	            | AppEvent::FloatingIpAssociated(_) => {
   977	                let _ = self.action_tx.send(Action::FetchServers);
   978	            }
   979	            AppEvent::ApiError {
   980	                operation, message, ..
   981	            } => {
   982	                self.error_message = Some(format!("{operation}: {message}"));
   983	                self.loading = false;
   984	                self.loading_ports = false;
   985	            }
   986	            _ => {}
   987	        }
   988	    }
   989	
   990	    fn render(&self, frame: &mut Frame, area: Rect) {
   991	        match &self.view_state {
   992	            ViewState::List => {
   993	                self.resource_list.render(frame, area);
   994	            }
   995	            ViewState::Detail(id) => {
   996	                if let Some(server) = self.servers.iter().find(|s| s.id == *id) {
   997	                    let matched_flavor = self.cached_flavors.iter().find(|f| f.id == server.flavor.id);
   998	                    let is_resize = self.resize_pending.as_ref().is_some_and(|rp| rp.server_id == *id);
   999	                    let data = server_detail_data(&ServerViewContext {
  1000	                        server,
  1001	                        migration_progress: self.migration_progress_for(id),
  1002	                        flavor: matched_flavor,
  1003	                        is_resize_pending: is_resize,
  1004	                        cached_volumes: &self.cached_volumes,
  1005	                        cached_floating_ips: &self.cached_floating_ips,
  1006	                    });
  1007	                    let mut dv = crate::ui::detail_view::DetailView::new();
  1008	                    dv.set_data(data);
  1009	                    dv.render(frame, area);
  1010	                }
  1011	            }
  1012	            ViewState::Create => {
  1013	                if let Some(form) = &self.form {
  1014	                    form.render(frame, area);
  1015	                } else {
  1016	                    // Defensive: form should always be Some in Create state.
  1017	                    // If not, render list as fallback (next key press will fix state via close_form).
  1018	                    self.resource_list.render(frame, area);
  1019	                }
  1020	            }
  1021	        }
  1022	
  1023	        // Overlay: SelectPopup
  1024	        if let Some(ref popup) = self.select_popup {
  1025	            popup.render(frame, area);
  1026	        }
  1027	        // Overlay: ConfirmDialog (highest priority)
  1028	        self.confirm.render(frame, area);
  1029	    }
  1030	
  1031	    fn content_title(&self) -> Option<String> {
  1032	        match &self.view_state {
  1033	            ViewState::List => None,
  1034	            ViewState::Detail(id) => {
  1035	                let name = self.servers.iter()
  1036	                    .find(|s| s.id == *id)
  1037	                    .map(|s| s.name.as_str())
  1038	                    .unwrap_or("...");
  1039	                Some(format!("Server: {name}"))
  1040	            }

exec
/bin/zsh -lc "rg -n \"CommandParser|Command::|ContextSwitch|ctx|switch-cloud|SwitchCloud|input_mode == InputMode::Command\" src/app.rs src/input/command.rs | sed -n '1,260p'" in /Users/jay.ahn/projects/infra/nexttui/.worktrees/runtime-context-switch
 succeeded in 0ms:
src/input/command.rs:16:    ContextSwitch(String),
src/input/command.rs:63:pub struct CommandParser {
src/input/command.rs:72:impl CommandParser {
src/input/command.rs:88:            return Command::Unknown(String::new());
src/input/command.rs:104:            "q" | "quit" => return Command::Quit,
src/input/command.rs:105:            "refresh" => return Command::Refresh,
src/input/command.rs:106:            "help" => return Command::Help,
src/input/command.rs:107:            "ctx" => {
src/input/command.rs:109:                    Some(cloud) if !cloud.is_empty() => Command::ContextSwitch(cloud),
src/input/command.rs:110:                    _ => Command::ContextList,
src/input/command.rs:118:            return Command::Navigate(*route);
src/input/command.rs:121:        Command::Unknown(trimmed.to_string())
src/input/command.rs:187:        cmds.extend(["quit", "refresh", "help", "ctx"].iter().map(|s| s.to_string()));
src/input/command.rs:298:    fn parser() -> CommandParser {
src/input/command.rs:300:        CommandParser::new(dir.path().join("history"))
src/input/command.rs:306:        assert_eq!(p.parse("servers"), Command::Navigate(Route::Servers));
src/input/command.rs:307:        assert_eq!(p.parse("networks"), Command::Navigate(Route::Networks));
src/input/command.rs:313:        assert_eq!(p.parse("srv"), Command::Navigate(Route::Servers));
src/input/command.rs:314:        assert_eq!(p.parse("net"), Command::Navigate(Route::Networks));
src/input/command.rs:315:        assert_eq!(p.parse("vol"), Command::Navigate(Route::Volumes));
src/input/command.rs:321:        assert_eq!(p.parse("quit"), Command::Quit);
src/input/command.rs:322:        assert_eq!(p.parse("q"), Command::Quit);
src/input/command.rs:323:        assert_eq!(p.parse("refresh"), Command::Refresh);
src/input/command.rs:324:        assert_eq!(p.parse("help"), Command::Help);
src/input/command.rs:331:            p.parse("ctx prod"),
src/input/command.rs:332:            Command::ContextSwitch("prod".to_string())
src/input/command.rs:334:        assert_eq!(p.parse("ctx"), Command::ContextList);
src/input/command.rs:340:        assert_eq!(p.parse("foobar"), Command::Unknown("foobar".to_string()));
src/input/command.rs:346:        assert_eq!(p.parse("SERVERS"), Command::Navigate(Route::Servers));
src/input/command.rs:347:        assert_eq!(p.parse("SRV"), Command::Navigate(Route::Servers));
src/input/command.rs:406:        let mut p1 = CommandParser::new(path.clone());
src/input/command.rs:411:        let mut p2 = CommandParser::new(path);
src/input/command.rs:420:        let mut p = CommandParser::new(dir.path().join("history"));

exec
/bin/zsh -lc "rg -n \"command_parser|parse\\(|ContextList|ContextSwitch|input buffer|Command\" src/app.rs src -g '*.rs' | sed -n '1,320p'" in /Users/jay.ahn/projects/infra/nexttui/.worktrees/runtime-context-switch
 succeeded in 0ms:
src/app.rs:235:                    self.input_mode = InputMode::Command;
src/app.rs:297:        // Esc from Command/Search/Confirm → Normal
src/app.rs:300:            InputMode::Command | InputMode::Search | InputMode::Confirm
src/app.rs:941:        assert_eq!(app.input_mode, InputMode::Command);
src/app.rs:974:        app.input_mode = InputMode::Command;
src/app.rs:235:                    self.input_mode = InputMode::Command;
src/app.rs:297:        // Esc from Command/Search/Confirm → Normal
src/app.rs:300:            InputMode::Command | InputMode::Search | InputMode::Confirm
src/app.rs:941:        assert_eq!(app.input_mode, InputMode::Command);
src/app.rs:974:        app.input_mode = InputMode::Command;
src/component.rs:35:    Command,
src/input/command.rs:11:pub enum Command {
src/input/command.rs:16:    ContextSwitch(String),
src/input/command.rs:17:    ContextList,
src/input/command.rs:23:struct CommandDef {
src/input/command.rs:29:const COMMAND_TABLE: &[CommandDef] = &[
src/input/command.rs:30:    CommandDef { name: "servers", abbreviation: "srv", route: Route::Servers },
src/input/command.rs:31:    CommandDef { name: "networks", abbreviation: "net", route: Route::Networks },
src/input/command.rs:32:    CommandDef { name: "volumes", abbreviation: "vol", route: Route::Volumes },
src/input/command.rs:33:    CommandDef { name: "floatingip", abbreviation: "fip", route: Route::FloatingIps },
src/input/command.rs:34:    CommandDef { name: "security-groups", abbreviation: "sec", route: Route::SecurityGroups },
src/input/command.rs:35:    CommandDef { name: "images", abbreviation: "img", route: Route::Images },
src/input/command.rs:36:    CommandDef { name: "flavors", abbreviation: "flv", route: Route::Flavors },
src/input/command.rs:37:    CommandDef { name: "projects", abbreviation: "prj", route: Route::Projects },
src/input/command.rs:38:    CommandDef { name: "users", abbreviation: "usr", route: Route::Users },
src/input/command.rs:39:    CommandDef { name: "aggregates", abbreviation: "agg", route: Route::Aggregates },
src/input/command.rs:40:    CommandDef { name: "hypervisors", abbreviation: "hyp", route: Route::Hosts },
src/input/command.rs:41:    CommandDef { name: "hosts", abbreviation: "host", route: Route::Hosts },
src/input/command.rs:42:    CommandDef { name: "migrations", abbreviation: "mig", route: Route::Migrations },
src/input/command.rs:43:    CommandDef { name: "snapshots", abbreviation: "snap", route: Route::Snapshots },
src/input/command.rs:44:    CommandDef { name: "compute-services", abbreviation: "svc", route: Route::ComputeServices },
src/input/command.rs:45:    CommandDef { name: "agents", abbreviation: "agt", route: Route::Agents },
src/input/command.rs:46:    CommandDef { name: "usage", abbreviation: "usg", route: Route::Usage },
src/input/command.rs:63:pub struct CommandParser {
src/input/command.rs:66:    history: CommandHistory,
src/input/command.rs:72:impl CommandParser {
src/input/command.rs:77:            history: CommandHistory::new(history_path, 50),
src/input/command.rs:85:    pub fn parse(&mut self, input: &str) -> Command {
src/input/command.rs:88:            return Command::Unknown(String::new());
src/input/command.rs:104:            "q" | "quit" => return Command::Quit,
src/input/command.rs:105:            "refresh" => return Command::Refresh,
src/input/command.rs:106:            "help" => return Command::Help,
src/input/command.rs:109:                    Some(cloud) if !cloud.is_empty() => Command::ContextSwitch(cloud),
src/input/command.rs:110:                    _ => Command::ContextList,
src/input/command.rs:118:            return Command::Navigate(*route);
src/input/command.rs:121:        Command::Unknown(trimmed.to_string())
src/input/command.rs:193:// --- CommandHistory ---
src/input/command.rs:195:struct CommandHistory {
src/input/command.rs:202:impl CommandHistory {
src/input/command.rs:298:    fn parser() -> CommandParser {
src/input/command.rs:300:        CommandParser::new(dir.path().join("history"))
src/input/command.rs:306:        assert_eq!(p.parse("servers"), Command::Navigate(Route::Servers));
src/input/command.rs:307:        assert_eq!(p.parse("networks"), Command::Navigate(Route::Networks));
src/input/command.rs:313:        assert_eq!(p.parse("srv"), Command::Navigate(Route::Servers));
src/input/command.rs:314:        assert_eq!(p.parse("net"), Command::Navigate(Route::Networks));
src/input/command.rs:315:        assert_eq!(p.parse("vol"), Command::Navigate(Route::Volumes));
src/input/command.rs:321:        assert_eq!(p.parse("quit"), Command::Quit);
src/input/command.rs:322:        assert_eq!(p.parse("q"), Command::Quit);
src/input/command.rs:323:        assert_eq!(p.parse("refresh"), Command::Refresh);
src/input/command.rs:324:        assert_eq!(p.parse("help"), Command::Help);
src/input/command.rs:331:            p.parse("ctx prod"),
src/input/command.rs:332:            Command::ContextSwitch("prod".to_string())
src/input/command.rs:334:        assert_eq!(p.parse("ctx"), Command::ContextList);
src/input/command.rs:340:        assert_eq!(p.parse("foobar"), Command::Unknown("foobar".to_string()));
src/input/command.rs:346:        assert_eq!(p.parse("SERVERS"), Command::Navigate(Route::Servers));
src/input/command.rs:347:        assert_eq!(p.parse("SRV"), Command::Navigate(Route::Servers));
src/input/command.rs:406:        let mut p1 = CommandParser::new(path.clone());
src/input/command.rs:411:        let mut p2 = CommandParser::new(path);
src/input/command.rs:420:        let mut p = CommandParser::new(dir.path().join("history"));
src/input/keymap.rs:6:    Command,
src/input/keymap.rs:23:    EnterCommandMode,
src/input/keymap.rs:64:            AppMode::Command => self.resolve_command(key),
src/input/keymap.rs:75:            AppMode::Command => "Enter:run  Tab:complete  Up/Down:history  Esc:cancel".into(),
src/input/keymap.rs:98:            KeyCode::Char(':') => KeyAction::EnterCommandMode,
src/input/keymap.rs:193:        assert_eq!(km.resolve(AppMode::Normal, key(KeyCode::Char(':'))), KeyAction::EnterCommandMode);
src/input/keymap.rs:209:        assert_eq!(km.resolve(AppMode::Command, key(KeyCode::Enter)), KeyAction::Select);
src/input/keymap.rs:210:        assert_eq!(km.resolve(AppMode::Command, key(KeyCode::Esc)), KeyAction::Back);
src/input/keymap.rs:211:        assert_eq!(km.resolve(AppMode::Command, key(KeyCode::Tab)), KeyAction::NextField);
src/input/keymap.rs:212:        assert_eq!(km.resolve(AppMode::Command, key(KeyCode::Char('a'))), KeyAction::CharInput('a'));
src/input/keymap.rs:228:        assert_eq!(km.resolve(AppMode::Command, ctrl_c), KeyAction::ForceQuit);
src/input/keymap.rs:239:        let help = km.context_help(AppMode::Command);
src/ui/input_bar.rs:15:    Command,
src/ui/input_bar.rs:123:            InputMode::Command => (":", Theme::warning()),
src/ui/input_bar.rs:160:        bar.activate(InputMode::Command);
src/ui/input_bar.rs:161:        assert_eq!(*bar.mode(), InputMode::Command);
src/ui/input_bar.rs:169:        bar.activate(InputMode::Command);
src/ui/input_bar.rs:191:        bar.activate(InputMode::Command);
src/ui/input_bar.rs:205:        bar.activate(InputMode::Command);
src/ui/input_bar.rs:216:        bar.activate(InputMode::Command);

exec
/bin/zsh -lc "nl -ba src/app.rs | sed -n '900,1120p'" in /Users/jay.ahn/projects/infra/nexttui/.worktrees/runtime-context-switch
 succeeded in 0ms:
   900	                last_event_received: false,
   901	            }
   902	        }
   903	    }
   904	
   905	    impl Component for MockComponent {
   906	        fn handle_key(&mut self, key: KeyEvent) -> Option<Action> {
   907	            self.last_key = Some(key.code);
   908	            None
   909	        }
   910	
   911	        fn handle_event(&mut self, _event: &AppEvent) {
   912	            self.last_event_received = true;
   913	        }
   914	
   915	        fn render(&self, _frame: &mut Frame, _area: Rect) {}
   916	    }
   917	
   918	    fn make_app() -> App {
   919	        let (tx, _rx) = mpsc::unbounded_channel();
   920	        let config = test_config();
   921	        App::new(config, tx)
   922	    }
   923	
   924	    fn test_config() -> Config {
   925	        // Use load_from with a temp file
   926	        let dir = tempfile::TempDir::new().unwrap();
   927	        let path = dir.path().join("clouds.yaml");
   928	        std::fs::write(
   929	            &path,
   930	            "clouds:\n  test:\n    auth:\n      auth_url: https://keystone/v3\n      username: admin\n      password: secret\n",
   931	        )
   932	        .unwrap();
   933	        Config::load_from(&path).unwrap()
   934	    }
   935	
   936	    #[test]
   937	    fn test_app_global_key_colon() {
   938	        let mut app = make_app();
   939	        assert_eq!(app.input_mode, InputMode::Normal);
   940	        app.handle_key(make_key(KeyCode::Char(':')));
   941	        assert_eq!(app.input_mode, InputMode::Command);
   942	    }
   943	
   944	    #[test]
   945	    fn test_app_global_key_slash_does_not_enter_search() {
   946	        let mut app = make_app();
   947	        app.handle_key(make_key(KeyCode::Char('/')));
   948	        // '/' no longer activates App-level search (unimplemented)
   949	        // Search is handled by SelectPopup when open
   950	        assert_eq!(app.input_mode, InputMode::Normal);
   951	    }
   952	
   953	    #[test]
   954	    fn test_app_global_key_tab_focus_toggle() {
   955	        let mut app = make_app();
   956	        assert_eq!(app.focus, FocusPane::Content);
   957	        app.handle_key(make_key(KeyCode::Tab));
   958	        assert_eq!(app.focus, FocusPane::Sidebar);
   959	        app.handle_key(make_key(KeyCode::Tab));
   960	        assert_eq!(app.focus, FocusPane::Content);
   961	    }
   962	
   963	    #[test]
   964	    fn test_app_global_key_q() {
   965	        let mut app = make_app();
   966	        assert!(!app.should_quit);
   967	        app.handle_key(make_key(KeyCode::Char('q')));
   968	        assert!(app.should_quit);
   969	    }
   970	
   971	    #[test]
   972	    fn test_app_esc_to_normal() {
   973	        let mut app = make_app();
   974	        app.input_mode = InputMode::Command;
   975	        app.handle_key(make_key(KeyCode::Esc));
   976	        assert_eq!(app.input_mode, InputMode::Normal);
   977	    }
   978	
   979	    #[test]
   980	    fn test_app_esc_normal_back() {
   981	        let mut app = make_app();
   982	        app.router_mut().navigate(Route::Networks);
   983	        assert_eq!(app.router().current(), Route::Networks);
   984	        app.handle_key(make_key(KeyCode::Esc));
   985	        assert_eq!(app.router().current(), Route::Servers);
   986	    }
   987	
   988	    #[test]
   989	    fn test_app_delegate_to_component() {
   990	        let mut app = make_app();
   991	        app.register_component(Route::Servers, Box::new(MockComponent::new()));
   992	
   993	        // Delegating 'j' to the component should not panic
   994	        // and should return true (needs re-render).
   995	        let needs_render = app.handle_key(make_key(KeyCode::Char('j')));
   996	        assert!(needs_render);
   997	        // Verify component is still registered (not consumed)
   998	        assert!(app.components.contains_key(&Route::Servers));
   999	    }
  1000	
  1001	    #[test]
  1002	    fn test_dispatch_cud_action_adds_progress_toast() {
  1003	        let mut app = make_app();
  1004	        app.dispatch_action(Action::CreateServer(crate::port::types::ServerCreateParams {
  1005	            name: "web-01".into(),
  1006	            image_id: "img-1".into(),
  1007	            flavor_id: "flv-1".into(),
  1008	            networks: vec![],
  1009	            security_groups: None,
  1010	            key_name: None,
  1011	            availability_zone: None,
  1012	        }));
  1013	        let toasts = app.background_tracker().active_toasts();
  1014	        assert!(toasts.iter().any(|t| t.message.contains("Creating server")));
  1015	        assert!(toasts.iter().any(|t| t.level == crate::background::ToastLevel::Info));
  1016	    }
  1017	
  1018	    #[test]
  1019	    fn test_handle_event_server_created_adds_toast() {
  1020	        let mut app = make_app();
  1021	        assert!(app.background_tracker().active_toasts().is_empty());
  1022	        let server: crate::models::nova::Server = serde_json::from_str(r#"{
  1023	            "id": "s1", "name": "web-01", "status": "ACTIVE",
  1024	            "addresses": {}, "flavor": {"id": "f1"}, "created": "2026-01-01"
  1025	        }"#).unwrap();
  1026	        app.handle_event(AppEvent::ServerCreated(server));
  1027	        let toasts = app.background_tracker().active_toasts();
  1028	        assert_eq!(toasts.len(), 1);
  1029	        assert_eq!(toasts[0].level, crate::background::ToastLevel::Success);
  1030	        assert!(toasts[0].message.contains("web-01"));
  1031	    }
  1032	
  1033	    #[test]
  1034	    fn test_handle_event_api_error_adds_toast() {
  1035	        let mut app = make_app();
  1036	        app.handle_event(AppEvent::ApiError {
  1037	            operation: "CreateServer".into(),
  1038	            message: "quota exceeded".into(),
  1039	        });
  1040	        let toasts = app.background_tracker().active_toasts();
  1041	        assert_eq!(toasts.len(), 1);
  1042	        assert_eq!(toasts[0].level, crate::background::ToastLevel::Error);
  1043	        assert!(toasts[0].message.contains("quota exceeded"));
  1044	    }
  1045	
  1046	    #[test]
  1047	    fn test_app_rbac_is_admin() {
  1048	        let app = make_app();
  1049	        assert!(!app.rbac.is_admin());
  1050	    }
  1051	
  1052	    #[test]
  1053	    fn test_app_broadcast_admin() {
  1054	        let mut app = make_app();
  1055	        app.register_component(Route::Servers, Box::new(MockComponent::new()));
  1056	        app.broadcast_admin();
  1057	    }
  1058	
  1059	    #[test]
  1060	    fn test_app_sidebar_uses_rbac() {
  1061	        use crate::ui::sidebar::SidebarItem;
  1062	        let (tx, _rx) = mpsc::unbounded_channel();
  1063	        let config = test_config();
  1064	        let mut app = App::new(config, tx);
  1065	        // App with default RbacGuard (not admin)
  1066	        app.sidebar = Sidebar::new(vec![
  1067	            SidebarItem { label: "Servers".into(), route: Route::Servers, shortcut: "1".into(), admin_only: false },
  1068	            SidebarItem { label: "Projects".into(), route: Route::Projects, shortcut: "2".into(), admin_only: true },
  1069	        ]);
  1070	        // Key '2' maps to index 1. With is_admin=true, visible_items has 2 items, index 1 = Projects.
  1071	        // With is_admin=false (rbac default), visible_items has 1 item, index 1 = None.
  1072	        app.handle_key(make_key(KeyCode::Char('2')));
  1073	        // Should NOT navigate to Projects when not admin
  1074	        assert_eq!(app.router().current(), Route::Servers);
  1075	    }
  1076	
  1077	    #[test]
  1078	    fn test_handle_token_refreshed_updates_rbac() {
  1079	        let mut app = make_app();
  1080	        assert!(!app.rbac.is_admin());
  1081	        let roles = vec![crate::port::types::TokenRole { id: "r1".into(), name: "admin".into() }];
  1082	        app.handle_event(AppEvent::TokenRefreshed(roles));
  1083	        assert!(app.rbac.is_admin());
  1084	    }
  1085	
  1086	    #[test]
  1087	    fn test_dispatch_migration_action_adds_progress_toast() {
  1088	        let mut app = make_app();
  1089	        app.dispatch_action(Action::LiveMigrateServer {
  1090	            id: "s1".into(), host: None,
  1091	        });
  1092	        let toasts = app.background_tracker().active_toasts();
  1093	        assert!(toasts.iter().any(|t| t.message.contains("Live migrating")));
  1094	    }
  1095	
  1096	    #[test]
  1097	    fn test_handle_cold_migrated_event_toast_and_refresh() {
  1098	        let (tx, mut rx) = mpsc::unbounded_channel();
  1099	        let config = test_config();
  1100	        let mut app = App::new(config, tx);
  1101	        app.handle_event(AppEvent::ServerColdMigrated { id: "s1".into() });
  1102	        let toasts = app.background_tracker().active_toasts();
  1103	        assert!(toasts.iter().any(|t| t.message.contains("confirm(Y) or revert(N)")));
  1104	        // Should have sent FetchServers for refresh
  1105	        let mut found = false;
  1106	        while let Ok(action) = rx.try_recv() {
  1107	            if matches!(action, Action::FetchServers) { found = true; }
  1108	        }
  1109	        assert!(found, "expected FetchServers after migration event");
  1110	    }
  1111	
  1112	    #[test]
  1113	    fn test_handle_evacuated_event_adds_toast() {
  1114	        let mut app = make_app();
  1115	        app.handle_event(AppEvent::ServerEvacuated { id: "s1".into() });
  1116	        let toasts = app.background_tracker().active_toasts();
  1117	        assert!(toasts.iter().any(|t| t.message.contains("evacuated")));
  1118	    }
  1119	
  1120	    #[test]

exec
/bin/zsh -lc "tail -n +560 devflow-docs/inception/application-design.md | nl -ba | sed -n '1,200p'" in /Users/jay.ahn/projects/infra/nexttui/.worktrees/runtime-context-switch
 succeeded in 0ms:
exec
/bin/zsh -lc 'wc -l devflow-docs/inception/application-design.md' in /Users/jay.ahn/projects/infra/nexttui/.worktrees/runtime-context-switch
 succeeded in 0ms:
     352 devflow-docs/inception/application-design.md

exec
/bin/zsh -lc "nl -ba src/adapter/http/nova.rs | sed -n '1,120p'" in /Users/jay.ahn/projects/infra/nexttui/.worktrees/runtime-context-switch
 succeeded in 0ms:
     1	use std::collections::HashMap;
     2	use std::sync::Arc;
     3	
     4	use async_trait::async_trait;
     5	use chrono::{DateTime, Utc};
     6	use serde::{Deserialize, Serialize};
     7	
     8	use super::{Link, append_pagination_parts, build_pagination_query, encode_param, extract_next_marker, paginated_list};
     9	use crate::adapter::http::base::BaseHttpClient;
    10	use crate::models::nova::{Aggregate, ComputeService, Flavor, Hypervisor, Server, ServerMigration};
    11	use crate::port::auth::AuthProvider;
    12	use crate::port::error::{ApiError, ApiResult};
    13	use crate::port::nova::NovaPort;
    14	use crate::port::types::*;
    15	
    16	pub struct NovaHttpAdapter {
    17	    base: BaseHttpClient,
    18	}
    19	
    20	impl NovaHttpAdapter {
    21	    pub fn new(auth: Arc<dyn AuthProvider>, region: Option<String>) -> Result<Self, ApiError> {
    22	        Ok(Self {
    23	            base: BaseHttpClient::new(auth, "compute", EndpointInterface::Public, region)?,
    24	        })
    25	    }
    26	}
    27	
    28	// --- JSON wrapper structs (private) ---
    29	
    30	#[derive(Deserialize)]
    31	struct NovaServersResponse {
    32	    servers: Vec<Server>,
    33	    servers_links: Option<Vec<Link>>,
    34	}
    35	
    36	#[derive(Deserialize)]
    37	struct NovaServerWrapper {
    38	    server: Server,
    39	}
    40	
    41	#[derive(Deserialize)]
    42	struct NovaServerCreateResponse {
    43	    server: NovaServerCreateResult,
    44	}
    45	
    46	#[derive(Deserialize)]
    47	struct NovaServerCreateResult {
    48	    id: String,
    49	}
    50	
    51	#[derive(Deserialize)]
    52	struct NovaFlavorsResponse {
    53	    flavors: Vec<Flavor>,
    54	    flavors_links: Option<Vec<Link>>,
    55	}
    56	
    57	#[derive(Deserialize)]
    58	struct NovaFlavorWrapper {
    59	    flavor: Flavor,
    60	}
    61	
    62	#[derive(Deserialize)]
    63	struct NovaInstanceActionsResponse {
    64	    #[serde(rename = "instanceActions")]
    65	    instance_actions: Vec<ServerEvent>,
    66	}
    67	
    68	#[derive(Deserialize)]
    69	struct NovaMigrationsResponse {
    70	    migrations: Vec<ServerMigration>,
    71	}
    72	
    73	#[derive(Deserialize)]
    74	struct NovaHypervisorsResponse {
    75	    hypervisors: Vec<Hypervisor>,
    76	}
    77	
    78	#[derive(Deserialize)]
    79	struct NovaHypervisorWrapper {
    80	    hypervisor: Hypervisor,
    81	}
    82	
    83	#[derive(Deserialize)]
    84	struct NovaComputeServicesResponse {
    85	    services: Vec<ComputeService>,
    86	}
    87	
    88	#[derive(Deserialize)]
    89	struct NovaComputeServiceWrapper {
    90	    service: ComputeService,
    91	}
    92	
    93	#[derive(Deserialize)]
    94	struct TenantUsagesResponse {
    95	    tenant_usages: Vec<TenantUsage>,
    96	}
    97	
    98	#[derive(Deserialize)]
    99	struct TenantUsageDetailResponse {
   100	    tenant_usage: ProjectUsage,
   101	}
   102	
   103	#[derive(Deserialize)]
   104	struct QuotaSetResponse {
   105	    quota_set: ComputeQuota,
   106	}
   107	
   108	#[derive(Deserialize)]
   109	struct NovaMigrationWrapper {
   110	    migration: ServerMigration,
   111	}
   112	
   113	#[derive(Serialize)]
   114	struct NovaServerCreateBody {
   115	    server: NovaServerCreateInner,
   116	}
   117	
   118	#[derive(Serialize)]
   119	struct NovaServerCreateInner {
   120	    name: String,

exec
/bin/zsh -lc "rg -n \"derive\\(Debug,\\s*Error\\)|derive\\(Debug,\\s*Clone,\\s*thiserror::Error\\)|enum .*Error\" src | sed -n '1,260p'" in /Users/jay.ahn/projects/infra/nexttui/.worktrees/runtime-context-switch
 succeeded in 0ms:
src/port/error.rs:3:#[derive(Debug, Error)]
src/port/error.rs:5:pub enum ApiError {
src/error.rs:6:pub enum AppError {

exec
/bin/zsh -lc "nl -ba src/error.rs | sed -n '1,220p'" in /Users/jay.ahn/projects/infra/nexttui/.worktrees/runtime-context-switch
 succeeded in 0ms:
     1	use std::path::PathBuf;
     2	use thiserror::Error;
     3	
     4	#[derive(Error, Debug)]
     5	#[non_exhaustive]
     6	pub enum AppError {
     7	    #[error("clouds.yaml not found. Searched: {searched_paths:?}")]
     8	    CloudsYamlNotFound { searched_paths: Vec<PathBuf> },
     9	
    10	    #[error("Failed to parse {path}: {source}")]
    11	    ConfigParse {
    12	        path: PathBuf,
    13	        #[source]
    14	        source: Box<dyn std::error::Error + Send + Sync>,
    15	    },
    16	
    17	    #[error("Config validation failed: {message}")]
    18	    ConfigValidation { message: String },
    19	
    20	    #[error("Cloud '{name}' not found. Available: {available:?}")]
    21	    CloudNotFound {
    22	        name: String,
    23	        available: Vec<String>,
    24	    },
    25	
    26	    #[error("API request failed: {message}")]
    27	    Api {
    28	        message: String,
    29	        status: Option<u16>,
    30	    },
    31	
    32	    #[error("Authentication failed: {message}")]
    33	    Auth { message: String },
    34	
    35	    #[error("IO error: {source}")]
    36	    Io {
    37	        #[from]
    38	        source: std::io::Error,
    39	    },
    40	
    41	    #[error("{0}")]
    42	    Other(String),
    43	}
    44	
    45	pub type Result<T> = std::result::Result<T, AppError>;
    46	
    47	#[cfg(test)]
    48	mod tests {
    49	    use super::*;
    50	
    51	    #[test]
    52	    fn test_app_error_display() {
    53	        let err = AppError::CloudsYamlNotFound {
    54	            searched_paths: vec![PathBuf::from("/a"), PathBuf::from("/b")],
    55	        };
    56	        assert!(err.to_string().contains("clouds.yaml not found"));
    57	        assert!(err.to_string().contains("/a"));
    58	
    59	        let err = AppError::ConfigValidation {
    60	            message: "missing auth".to_string(),
    61	        };
    62	        assert_eq!(err.to_string(), "Config validation failed: missing auth");
    63	
    64	        let err = AppError::CloudNotFound {
    65	            name: "prod".to_string(),
    66	            available: vec!["dev".to_string()],
    67	        };
    68	        assert!(err.to_string().contains("prod"));
    69	        assert!(err.to_string().contains("dev"));
    70	
    71	        let err = AppError::Api {
    72	            message: "timeout".to_string(),
    73	            status: Some(503),
    74	        };
    75	        assert!(err.to_string().contains("timeout"));
    76	
    77	        let err = AppError::Auth {
    78	            message: "invalid token".to_string(),
    79	        };
    80	        assert!(err.to_string().contains("invalid token"));
    81	
    82	        let err = AppError::Other("something".to_string());
    83	        assert_eq!(err.to_string(), "something");
    84	    }
    85	}

exec
/bin/zsh -lc "nl -ba src/adapter/http/mod.rs | sed -n '1,220p'" in /Users/jay.ahn/projects/infra/nexttui/.worktrees/runtime-context-switch
 succeeded in 0ms:
     1	pub mod base;
     2	pub mod cinder;
     3	pub mod glance;
     4	pub mod keystone;
     5	pub mod neutron;
     6	pub mod nova;
     7	
     8	use serde::Deserialize;
     9	use serde::de::DeserializeOwned;
    10	
    11	use crate::adapter::http::base::BaseHttpClient;
    12	use crate::port::error::ApiResult;
    13	use crate::port::types::{PaginatedResponse, PaginationParams, SortDirection};
    14	
    15	// --- Shared HTTP helpers (used by nova, neutron, cinder, etc.) ---
    16	
    17	#[derive(Deserialize)]
    18	pub(crate) struct Link {
    19	    pub rel: String,
    20	    pub href: String,
    21	}
    22	
    23	/// Percent-encode a query parameter value (RFC 3986).
    24	pub(crate) fn encode_param(value: &str) -> String {
    25	    let mut encoded = String::with_capacity(value.len());
    26	    for byte in value.bytes() {
    27	        match byte {
    28	            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => {
    29	                encoded.push(byte as char);
    30	            }
    31	            _ => {
    32	                encoded.push_str(&format!("%{:02X}", byte));
    33	            }
    34	        }
    35	    }
    36	    encoded
    37	}
    38	
    39	pub(crate) fn append_pagination_parts(parts: &mut Vec<String>, pagination: &PaginationParams) {
    40	    if let Some(ref marker) = pagination.marker {
    41	        parts.push(format!("marker={}", encode_param(marker)));
    42	    }
    43	    if let Some(limit) = pagination.limit {
    44	        parts.push(format!("limit={limit}"));
    45	    }
    46	    if let Some(ref key) = pagination.sort_key {
    47	        parts.push(format!("sort_key={}", encode_param(key)));
    48	    }
    49	    if let Some(ref dir) = pagination.sort_dir {
    50	        let dir_str = match dir {
    51	            SortDirection::Asc => "asc",
    52	            SortDirection::Desc => "desc",
    53	        };
    54	        parts.push(format!("sort_dir={dir_str}"));
    55	    }
    56	}
    57	
    58	pub(crate) fn build_pagination_query(pagination: &PaginationParams) -> String {
    59	    let mut parts = Vec::new();
    60	    append_pagination_parts(&mut parts, pagination);
    61	    parts.join("&")
    62	}
    63	
    64	pub(crate) fn extract_next_marker(links: &[Link]) -> Option<String> {
    65	    links
    66	        .iter()
    67	        .find(|l| l.rel == "next")
    68	        .and_then(|l| extract_marker_from_url(&l.href))
    69	}
    70	
    71	/// Extract `marker=` value from a URL query string.
    72	/// Shared by all marker extraction variants (Link array, Glance next URL, Keystone links).
    73	pub(crate) fn extract_marker_from_url(url: &str) -> Option<String> {
    74	    url.split('?')
    75	        .nth(1)
    76	        .and_then(|query| {
    77	            query
    78	                .split('&')
    79	                .find(|p| p.starts_with("marker="))
    80	                .map(|p| p.trim_start_matches("marker=").to_string())
    81	        })
    82	}
    83	
    84	/// Generic paginated list combinator.
    85	///
    86	/// Handles the common pattern: build path + query → GET → deserialize → extract items + marker.
    87	/// The `extract` closure receives the deserialized response and returns (items, next_marker).
    88	pub(crate) async fn paginated_list<T, R, F>(
    89	    base: &BaseHttpClient,
    90	    path: &str,
    91	    query: &str,
    92	    extract: F,
    93	) -> ApiResult<PaginatedResponse<T>>
    94	where
    95	    R: DeserializeOwned,
    96	    F: FnOnce(R) -> (Vec<T>, Option<String>),
    97	{
    98	    let full_path = if query.is_empty() {
    99	        path.to_string()
   100	    } else {
   101	        format!("{path}?{query}")
   102	    };
   103	    let req = base.get(&full_path).await?;
   104	    let resp: R = base.send_json(req).await?;
   105	    let (items, next_marker) = extract(resp);
   106	    let has_more = next_marker.is_some();
   107	    Ok(PaginatedResponse {
   108	        items,
   109	        next_marker,
   110	        has_more,
   111	    })
   112	}

exec
/bin/zsh -lc "rg -n \"enum ProjectRef|struct ProjectRef|type ProjectRef|SessionHandle|ServiceCatalog|ContextTarget\" src devflow-docs/inception/application-design.md | sed -n '1,260p'" in /Users/jay.ahn/projects/infra/nexttui/.worktrees/runtime-context-switch
 succeeded in 0ms:
devflow-docs/inception/application-design.md:31:| `ContextTargetResolver` | name/uuid/cloud-prefix → ContextTarget 변환, 충돌 disambiguation. 명령·피커·모듈 액션 공유 | Service | PR1 |
devflow-docs/inception/application-design.md:46:| `CommandParser` (src/input/command.rs) | `:switch-project <name|uuid|cloud/project>`, `:switch-cloud`, `:switch-back` 등록 + tab 자동완성. `ContextTargetResolver` 사용 | Controller | PR3 |
devflow-docs/inception/application-design.md:66:| **PR1 (safety infra + switch core)** | ContextEpoch, CancellationRegistry, VersionedEvent, AppEvent::ContextChanged, SwitchStateMachine, ContextSwitcher, ContextSessionPort, ScopedAuthSession, KeystoneRescopeAdapter, EndpointCatalogInvalidator, ContextTargetResolver, ContextHistoryStore, App 통합, Worker 시그니처 개정, Action/AppEvent envelope, TokenCacheStore 확장, MockContextSession | — | 없음 (인프라만) |
devflow-docs/inception/application-design.md:80:| NFR-4 UX 일관성 | ContextPicker (기존 SelectPopup 재사용), CommandParser 확장 (기존 패턴), ContextTargetResolver (단일 disambiguation 로직) |
devflow-docs/inception/application-design.md:90:| ContextTargetResolver 추가 | Codex | 명령·피커·모듈 액션이 같은 disambiguation 로직 필요 (3중 구현 회피) |
devflow-docs/inception/application-design.md:115:pub struct ContextTarget {
devflow-docs/inception/application-design.md:123:    pub target: ContextTarget,
devflow-docs/inception/application-design.md:136:    Ambiguous { candidates: Vec<ContextTarget> },
devflow-docs/inception/application-design.md:173:- `AppEvent::ContextChanged { target: ContextTarget, epoch: Epoch }`
devflow-docs/inception/application-design.md:174:**Dependencies**: `ContextTarget` (src/context.rs)
devflow-docs/inception/application-design.md:180:- `pub fn try_begin(&mut self, target: ContextTarget) -> Result<Epoch, SwitchError::InProgress>`
devflow-docs/inception/application-design.md:189:- `pub async fn switch(&self, target: ContextTarget) -> Result<ContextSnapshot, SwitchError>`
devflow-docs/inception/application-design.md:200:    async fn begin(&self, target: &ContextTarget) -> Result<SessionHandle, SwitchError>;
devflow-docs/inception/application-design.md:201:    async fn commit(&self, handle: SessionHandle) -> Result<ContextSnapshot, SwitchError>;
devflow-docs/inception/application-design.md:202:    async fn rollback(&self, handle: SessionHandle);
devflow-docs/inception/application-design.md:206:**Dependencies**: `ContextTarget`, `ContextSnapshot`, `SwitchError`
devflow-docs/inception/application-design.md:216:- `pub async fn rescope(&self, current_token: &Token, target: &ContextTarget) -> Result<Token, SwitchError>`
devflow-docs/inception/application-design.md:224:- `pub async fn refresh_catalog(&self, token: &Token) -> Result<ServiceCatalog, SwitchError>`
devflow-docs/inception/application-design.md:227:### ContextTargetResolver (Service)
devflow-docs/inception/application-design.md:228:**Responsibility**: name/uuid/cloud-prefix → ContextTarget 변환 + 충돌 disambiguation.
devflow-docs/inception/application-design.md:230:- `pub fn resolve(&self, input: &str, current_cloud: &str) -> Result<ContextTarget, SwitchError>` — 단일 매치 시 OK, 충돌 시 `Ambiguous`, 미매치 시 `NotFound`
devflow-docs/inception/application-design.md:231:- `pub fn list_user_projects(&self) -> Vec<ContextTarget>` — 피커용 (`/v3/auth/projects` 결과 캐시)
devflow-docs/inception/application-design.md:254:- `pub fn open(&mut self, candidates: Vec<ContextTarget>, current: Option<&ContextTarget>)`
devflow-docs/inception/application-design.md:257:**Dependencies**: `SelectPopup` (src/ui/select_popup.rs 재사용), `ContextTarget`, `Theme`
devflow-docs/inception/application-design.md:264:- `pub fn captured_targets(&self) -> Vec<ContextTarget>`
devflow-docs/inception/application-design.md:272:- `pub async fn switch_context(&mut self, target: ContextTarget) -> Result<(), SwitchError>` — ContextSwitcher 위임 + AppEvent::ContextChanged 디스패치
devflow-docs/inception/application-design.md:286:**Dependencies (추가)**: `VersionedEvent`, `ContextTarget` (`Action::SwitchContext`, `Action::SwitchBack` variant 추가)
devflow-docs/inception/application-design.md:290:**Dependencies (추가)**: `VersionedEvent`, `ContextTarget`
devflow-docs/inception/application-design.md:297:- 충돌 시 `ContextTargetResolver`의 `Ambiguous` → 후보 출력 + 재선택 안내
devflow-docs/inception/application-design.md:299:**Dependencies (추가)**: `ContextTargetResolver`
devflow-docs/inception/application-design.md:314:- `pub fn store_rescoped(&self, target: &ContextTarget, token: Token) -> Result<(), io::Error>`
devflow-docs/inception/application-design.md:315:- `pub fn lookup(&self, target: &ContextTarget) -> Option<Token>`
devflow-docs/inception/application-design.md:317:**Dependencies**: 기존 + `ContextTarget`
devflow-docs/inception/application-design.md:324:**Dependencies (추가)**: `ContextTarget`
devflow-docs/inception/application-design.md:350:  Resolver (ContextTargetResolver) ── used by CommandParser, ContextPicker, Project Module
src/models/keystone.rs:52:pub struct ProjectRef {
src/infra/catalog.rs:27:pub struct ServiceCatalog {
src/infra/catalog.rs:33:impl ServiceCatalog {
src/infra/catalog.rs:219:        let catalog = ServiceCatalog::new(EndpointInterface::Internal);
src/infra/catalog.rs:227:        let catalog = ServiceCatalog::new(EndpointInterface::Internal);
src/infra/catalog.rs:235:        let catalog = ServiceCatalog::new(EndpointInterface::Public);
src/infra/catalog.rs:243:        let catalog = ServiceCatalog::new(EndpointInterface::Internal);
src/infra/catalog.rs:253:        let catalog = ServiceCatalog::new(EndpointInterface::Internal);
src/infra/catalog.rs:261:        let catalog = ServiceCatalog::new(EndpointInterface::Public);
src/infra/catalog.rs:269:        let catalog = ServiceCatalog::new(EndpointInterface::Public);
src/infra/catalog.rs:277:        let catalog = ServiceCatalog::new(EndpointInterface::Public);
src/infra/catalog.rs:285:        let catalog = ServiceCatalog::new(EndpointInterface::Public);
src/infra/catalog.rs:304:        let catalog = ServiceCatalog::new(EndpointInterface::Public);
src/infra/catalog.rs:312:        let catalog = ServiceCatalog::new(EndpointInterface::Internal);

exec
/bin/zsh -lc "nl -ba src/models/keystone.rs | sed -n '1,140p'" in /Users/jay.ahn/projects/infra/nexttui/.worktrees/runtime-context-switch
 succeeded in 0ms:
     1	use serde::Deserialize;
     2	
     3	#[derive(Debug, Clone, Deserialize)]
     4	pub struct Project {
     5	    pub id: String,
     6	    pub name: String,
     7	    pub description: Option<String>,
     8	    #[serde(default = "default_true")]
     9	    pub enabled: bool,
    10	    pub domain_id: Option<String>,
    11	}
    12	
    13	fn default_true() -> bool {
    14	    true
    15	}
    16	
    17	#[derive(Debug, Clone, Deserialize)]
    18	pub struct User {
    19	    pub id: String,
    20	    pub name: String,
    21	    pub email: Option<String>,
    22	    #[serde(default = "default_true")]
    23	    pub enabled: bool,
    24	    pub default_project_id: Option<String>,
    25	    pub domain_id: Option<String>,
    26	}
    27	
    28	#[derive(Debug, Clone, Deserialize)]
    29	pub struct Role {
    30	    pub id: String,
    31	    pub name: String,
    32	}
    33	
    34	#[derive(Debug, Clone, Deserialize)]
    35	pub struct RoleAssignment {
    36	    pub role: Role,
    37	    pub user: Option<UserRef>,
    38	    pub scope: Option<Scope>,
    39	}
    40	
    41	#[derive(Debug, Clone, Deserialize)]
    42	pub struct UserRef {
    43	    pub id: String,
    44	}
    45	
    46	#[derive(Debug, Clone, Deserialize)]
    47	pub struct Scope {
    48	    pub project: Option<ProjectRef>,
    49	}
    50	
    51	#[derive(Debug, Clone, Deserialize)]
    52	pub struct ProjectRef {
    53	    pub id: String,
    54	}
    55	
    56	#[cfg(test)]
    57	mod tests {
    58	    use super::*;
    59	
    60	    #[test]
    61	    fn test_project_deserialize() {
    62	        let json = r#"{
    63	            "id": "proj-001",
    64	            "name": "my-project",
    65	            "description": "Test project",
    66	            "enabled": true,
    67	            "domain_id": "default"
    68	        }"#;
    69	        let proj: Project = serde_json::from_str(json).unwrap();
    70	        assert_eq!(proj.name, "my-project");
    71	        assert!(proj.enabled);
    72	    }
    73	
    74	    #[test]
    75	    fn test_user_deserialize() {
    76	        let json = r#"{
    77	            "id": "user-001",
    78	            "name": "admin",
    79	            "email": "admin@example.com",
    80	            "enabled": true,
    81	            "default_project_id": "proj-001",
    82	            "domain_id": "default"
    83	        }"#;
    84	        let user: User = serde_json::from_str(json).unwrap();
    85	        assert_eq!(user.name, "admin");
    86	        assert_eq!(user.email.as_deref(), Some("admin@example.com"));
    87	    }
    88	}

exec
/bin/zsh -lc "nl -ba src/infra/catalog.rs | sed -n '1,120p'" in /Users/jay.ahn/projects/infra/nexttui/.worktrees/runtime-context-switch
 succeeded in 0ms:
     1	use std::sync::RwLock;
     2	
     3	use crate::error::{AppError, Result};
     4	use crate::port::types::{CatalogEntry, Endpoint, EndpointInterface};
     5	
     6	#[derive(Debug, Clone, Copy, PartialEq, Eq)]
     7	pub enum ServiceType {
     8	    Compute,
     9	    Network,
    10	    BlockStorage,
    11	    Identity,
    12	    Image,
    13	}
    14	
    15	impl ServiceType {
    16	    pub fn as_str(&self) -> &'static str {
    17	        match self {
    18	            ServiceType::Compute => "compute",
    19	            ServiceType::Network => "network",
    20	            ServiceType::BlockStorage => "volumev3",
    21	            ServiceType::Identity => "identity",
    22	            ServiceType::Image => "image",
    23	        }
    24	    }
    25	}
    26	
    27	pub struct ServiceCatalog {
    28	    catalog: RwLock<Vec<CatalogEntry>>,
    29	    region: RwLock<Option<String>>,
    30	    interface_preference: RwLock<EndpointInterface>,
    31	}
    32	
    33	impl ServiceCatalog {
    34	    pub fn new(interface_preference: EndpointInterface) -> Self {
    35	        Self {
    36	            catalog: RwLock::new(Vec::new()),
    37	            region: RwLock::new(None),
    38	            interface_preference: RwLock::new(interface_preference),
    39	        }
    40	    }
    41	
    42	    /// Update catalog from Keystone token response.
    43	    pub fn update(&self, catalog: Vec<CatalogEntry>, region: Option<String>) {
    44	        if let Ok(mut c) = self.catalog.write() {
    45	            *c = catalog;
    46	        }
    47	        if let Ok(mut r) = self.region.write() {
    48	            *r = region;
    49	        }
    50	    }
    51	
    52	    /// Resolve endpoint URL for a service type using the configured region.
    53	    pub fn endpoint(&self, service_type: ServiceType) -> Result<String> {
    54	        let region = self.region.read().ok().and_then(|r| r.clone());
    55	        self.resolve(service_type, region.as_deref())
    56	    }
    57	
    58	    /// Get endpoint for a service type with explicit region override.
    59	    pub fn endpoint_in_region(
    60	        &self,
    61	        service_type: ServiceType,
    62	        region: &str,
    63	    ) -> Result<String> {
    64	        self.resolve(service_type, Some(region))
    65	    }
    66	
    67	    /// List all available regions across all services.
    68	    pub fn available_regions(&self) -> Vec<String> {
    69	        let catalog = match self.catalog.read() {
    70	            Ok(c) => c,
    71	            Err(_) => return Vec::new(),
    72	        };
    73	        let mut regions: Vec<String> = catalog
    74	            .iter()
    75	            .flat_map(|e| e.endpoints.iter().map(|ep| ep.region.clone()))
    76	            .collect();
    77	        regions.sort();
    78	        regions.dedup();
    79	        regions
    80	    }
    81	
    82	    /// List all discovered service types.
    83	    pub fn available_services(&self) -> Vec<String> {
    84	        let catalog = match self.catalog.read() {
    85	            Ok(c) => c,
    86	            Err(_) => return Vec::new(),
    87	        };
    88	        catalog.iter().map(|e| e.service_type.clone()).collect()
    89	    }
    90	
    91	    /// Set active region.
    92	    pub fn set_region(&self, region: &str) {
    93	        if let Ok(mut r) = self.region.write() {
    94	            *r = Some(region.to_string());
    95	        }
    96	    }
    97	
    98	    /// Get current region.
    99	    pub fn current_region(&self) -> Option<String> {
   100	        self.region.read().ok().and_then(|r| r.clone())
   101	    }
   102	
   103	    /// Check if a service type is available in the catalog.
   104	    pub fn has_service(&self, service_type: ServiceType) -> bool {
   105	        let catalog = match self.catalog.read() {
   106	            Ok(c) => c,
   107	            Err(_) => return false,
   108	        };
   109	        let type_str = service_type.as_str();
   110	        catalog.iter().any(|e| e.service_type == type_str)
   111	    }
   112	
   113	    /// Common endpoint resolution logic shared by `endpoint` and `endpoint_in_region`.
   114	    fn resolve(&self, service_type: ServiceType, region: Option<&str>) -> Result<String> {
   115	        let catalog = self.catalog.read().map_err(|_| {
   116	            AppError::Other("Failed to read service catalog".to_string())
   117	        })?;
   118	        let pref = self
   119	            .interface_preference
   120	            .read()

exec
/bin/zsh -lc "rg -n \"Context|SwitchBack|SwitchContext|switch_back|context indicator|ctx\" src | sed -n '1,260p'" in /Users/jay.ahn/projects/infra/nexttui/.worktrees/runtime-context-switch
 succeeded in 0ms:
src/app.rs:19:use crate::ui::header::{Header, HeaderContext};
src/app.rs:764:        self.header.render(frame, areas.header, &HeaderContext {
src/input/command.rs:16:    ContextSwitch(String),
src/input/command.rs:17:    ContextList,
src/input/command.rs:107:            "ctx" => {
src/input/command.rs:109:                    Some(cloud) if !cloud.is_empty() => Command::ContextSwitch(cloud),
src/input/command.rs:110:                    _ => Command::ContextList,
src/input/command.rs:187:        cmds.extend(["quit", "refresh", "help", "ctx"].iter().map(|s| s.to_string()));
src/input/command.rs:331:            p.parse("ctx prod"),
src/input/command.rs:332:            Command::ContextSwitch("prod".to_string())
src/input/command.rs:334:        assert_eq!(p.parse("ctx"), Command::ContextList);
src/ui/header.rs:7:pub struct HeaderContext {
src/ui/header.rs:21:    pub fn render(&self, frame: &mut Frame, area: Rect, ctx: &HeaderContext) {
src/ui/header.rs:22:        let (app, fill, badge, right) = Self::build_header_parts(area.width, ctx);
src/ui/header.rs:35:    pub fn build_header_parts(width: u16, ctx: &HeaderContext) -> (String, String, Option<String>, String) {
src/ui/header.rs:37:        let badge = if ctx.all_tenants { Some("[ALL] ".to_string()) } else { None };
src/ui/header.rs:38:        let right_text = format!("{}@{} | {}", ctx.user_name, ctx.cloud_name, ctx.region);
src/ui/header.rs:59:    fn sample_ctx() -> HeaderContext {
src/ui/header.rs:60:        HeaderContext {
src/ui/header.rs:70:        let ctx = sample_ctx();
src/ui/header.rs:71:        assert_eq!(ctx.user_name, "admin");
src/ui/header.rs:72:        assert_eq!(ctx.cloud_name, "prod");
src/ui/header.rs:77:        let ctx = sample_ctx();
src/ui/header.rs:78:        let (app_name, _, _, _) = Header::build_header_parts(80, &ctx);
src/ui/header.rs:84:        let ctx = sample_ctx();
src/ui/header.rs:85:        let (_, fill, _, _) = Header::build_header_parts(80, &ctx);
src/ui/header.rs:91:        let ctx = sample_ctx();
src/ui/header.rs:92:        let (_, _, badge, right) = Header::build_header_parts(80, &ctx);
src/ui/header.rs:99:        let ctx = HeaderContext {
src/ui/header.rs:101:            ..sample_ctx()
src/ui/header.rs:103:        let (_, _, badge, right) = Header::build_header_parts(80, &ctx);
src/ui/header.rs:110:        let ctx = sample_ctx();
src/ui/header.rs:111:        let (app, fill, badge, right) = Header::build_header_parts(80, &ctx);
src/ui/header.rs:119:        let ctx = HeaderContext {
src/ui/header.rs:121:            ..sample_ctx()
src/ui/header.rs:123:        let (app, fill, badge, right) = Header::build_header_parts(80, &ctx);
src/module/floating_ip/mod.rs:21:use self::view_model::{fip_columns, fip_create_defs, fip_to_row, FipRowContext};
src/module/floating_ip/mod.rs:92:        let ctx = FipRowContext {
src/module/floating_ip/mod.rs:97:        self.floating_ips.iter().map(|f| fip_to_row(f, &ctx)).collect()
src/module/floating_ip/view_model.rs:7:pub struct FipRowContext<'a> {
src/module/floating_ip/view_model.rs:13:impl<'a> FipRowContext<'a> {
src/module/floating_ip/view_model.rs:61:    fip_to_row(fip, &FipRowContext::default_for(show_tenant))
src/module/floating_ip/view_model.rs:64:pub fn fip_to_row(fip: &FloatingIp, ctx: &FipRowContext) -> Row {
src/module/floating_ip/view_model.rs:65:    let show_tenant = ctx.show_tenant;
src/module/floating_ip/view_model.rs:66:    let cached_servers = ctx.cached_servers;
src/module/floating_ip/view_model.rs:67:    let cached_ports = ctx.cached_ports;
src/module/server/view_model.rs:10:pub struct ServerViewContext<'a> {
src/module/server/view_model.rs:19:impl<'a> ServerViewContext<'a> {
src/module/server/view_model.rs:146:    server_detail_data(&ServerViewContext::default_for(server))
src/module/server/view_model.rs:149:pub fn server_detail_data(ctx: &ServerViewContext) -> DetailData {
src/module/server/view_model.rs:150:    let server = ctx.server;
src/module/server/view_model.rs:151:    let migration_progress = ctx.migration_progress;
src/module/server/view_model.rs:152:    let flavor = ctx.flavor;
src/module/server/view_model.rs:153:    let is_resize_pending = ctx.is_resize_pending;
src/module/server/view_model.rs:154:    let cached_volumes = ctx.cached_volumes;
src/module/server/view_model.rs:155:    let cached_floating_ips = ctx.cached_floating_ips;
src/module/server/view_model.rs:686:        let data = server_detail_data(&ServerViewContext::default_for(&server));
src/module/server/view_model.rs:693:        let data = server_detail_data(&ServerViewContext::default_for(&server));
src/module/server/view_model.rs:714:        let data = server_detail_data(&ServerViewContext {
src/module/server/view_model.rs:732:        let data = server_detail_data(&ServerViewContext::default_for(&server));
src/module/server/view_model.rs:771:        let data = server_detail_data(&ServerViewContext {
src/module/server/view_model.rs:787:        let data = server_detail_data(&ServerViewContext::default_for(&server));
src/module/server/view_model.rs:801:        let data = server_detail_data(&ServerViewContext {
src/module/server/mod.rs:24:    ServerViewContext,
src/module/server/mod.rs:515:            let ctx = self.popup_kind.unwrap_or(DetailPopupKind::Resize);
src/module/server/mod.rs:520:                    match ctx {
src/module/server/mod.rs:999:                    let data = server_detail_data(&ServerViewContext {

codex
**Top 3 critical issues**

1. **Atomic boundary is internally contradictory and cannot guarantee rollback safety.**  
What: DETAIL says `ContextSessionPort` is the atomic boundary, but the switch flow bypasses it (`state.begin -> ... -> token_cache.store -> state.commit`) and never calls `session.commit/rollback` ([application-design.md#L191](/Users/jay.ahn/projects/infra/nexttui/.worktrees/runtime-context-switch/devflow-docs/inception/application-design.md#L191), [application-design.md#L200](/Users/jay.ahn/projects/infra/nexttui/.worktrees/runtime-context-switch/devflow-docs/inception/application-design.md#L200)). Also `invalidate_all()` is infallible while `CatalogFailed` and mock invalidate-failure are defined ([application-design.md#L133](/Users/jay.ahn/projects/infra/nexttui/.worktrees/runtime-context-switch/devflow-docs/inception/application-design.md#L133), [application-design.md#L223](/Users/jay.ahn/projects/infra/nexttui/.worktrees/runtime-context-switch/devflow-docs/inception/application-design.md#L223), [application-design.md#L263](/Users/jay.ahn/projects/infra/nexttui/.worktrees/runtime-context-switch/devflow-docs/inception/application-design.md#L263)).  
Why: You cannot deterministically handle “rescope OK + invalidate fail + rollback,” so NFR-1 atomicity is not met ([requirements.md#L69](/Users/jay.ahn/projects/infra/nexttui/.worktrees/runtime-context-switch/devflow-docs/inception/requirements.md#L69)).  
Fix: Make `ContextSessionPort` own `rescope + catalog refresh + commit/rollback` end-to-end; `ContextSwitcher` should orchestrate state/epoch only.

2. **Port boundary is wrong: current auth port cannot perform scope switch commit.**  
What: `AuthProvider` has no scope mutation APIs ([auth.rs#L8](/Users/jay.ahn/projects/infra/nexttui/.worktrees/runtime-context-switch/src/port/auth.rs#L8)), while `KeystoneAuthAdapter` holds fixed `active_scope` and all token reads key off it ([keystone.rs#L135](/Users/jay.ahn/projects/infra/nexttui/.worktrees/runtime-context-switch/src/adapter/auth/keystone.rs#L135), [keystone.rs#L357](/Users/jay.ahn/projects/infra/nexttui/.worktrees/runtime-context-switch/src/adapter/auth/keystone.rs#L357)).  
Why: `ScopedAuthSession` cannot be implemented cleanly through `AuthProvider`; it will require concrete adapter leakage/downcast, violating your intended layering.  
Fix: Add a dedicated scoped-auth port (`set_active_scope`, `upsert_scoped_token`, `current_scope_snapshot`) and keep `AuthProvider` read/request-auth oriented.

3. **Epoch/cancellation enforcement point is underspecified and currently bypassable.**  
What: DETAIL says worker spawn wraps one future and run_worker does epoch drop ([application-design.md#L278](/Users/jay.ahn/projects/infra/nexttui/.worktrees/runtime-context-switch/devflow-docs/inception/application-design.md#L278)), but current worker emits events from multiple polling paths ([worker.rs#L795](/Users/jay.ahn/projects/infra/nexttui/.worktrees/runtime-context-switch/src/worker.rs#L795), [worker.rs#L825](/Users/jay.ahn/projects/infra/nexttui/.worktrees/runtime-context-switch/src/worker.rs#L825), [worker.rs#L844](/Users/jay.ahn/projects/infra/nexttui/.worktrees/runtime-context-switch/src/worker.rs#L844)); event loop currently forwards blindly ([event_loop.rs#L48](/Users/jay.ahn/projects/infra/nexttui/.worktrees/runtime-context-switch/src/event_loop.rs#L48)).  
Why: FR-5 requires hard stale-drop guarantees across all async emitters, not just one spawn style ([requirements.md#L40](/Users/jay.ahn/projects/infra/nexttui/.worktrees/runtime-context-switch/devflow-docs/inception/requirements.md#L40)).  
Fix: Enforce epoch gate at a single choke point (App/event-loop dispatcher) and require all event sends to use `VersionedEvent<AppEvent>`.

---

**Specific signature corrections**

```rust
pub type Epoch = u64;

#[derive(Debug, thiserror::Error)]
pub enum SwitchError {
    #[error("switch already in progress")]
    InProgress,
    #[error("target not found: {0}")]
    NotFound(String),
    #[error("ambiguous target")]
    Ambiguous { candidates: Vec<ContextTarget> },
    #[error(transparent)]
    Api(#[from] ApiError),
    #[error(transparent)]
    Io(#[from] std::io::Error),
}

pub trait SwitchStateMachine {
    fn try_begin(&mut self, target: ContextTarget) -> Result<Epoch, SwitchError>; // not Result<_, SwitchError::InProgress>
    fn commit(&mut self, snapshot: ContextSnapshot);
    fn fail(&mut self, err: SwitchError);
}
```

```rust
#[async_trait]
pub trait ContextSessionPort: Send + Sync {
    type Handle: Send;
    async fn begin(&self, target: &ContextTarget, epoch: Epoch) -> Result<Self::Handle, SwitchError>;
    async fn rescope(&self, handle: &mut Self::Handle) -> Result<(), SwitchError>;
    async fn refresh_catalog(&self, handle: &mut Self::Handle) -> Result<(), SwitchError>;
    async fn commit(&self, handle: Self::Handle) -> Result<ContextSnapshot, SwitchError>;
    async fn rollback(&self, handle: Self::Handle) -> Result<(), SwitchError>;
}
```

```rust
pub async fn switch(&self, target: ContextTarget) -> Result<(Epoch, ContextSnapshot), SwitchError>;
pub async fn switch_back(&self) -> Result<(Epoch, ContextSnapshot), SwitchError>;
```

```rust
// Avoid duplicate epoch in payload when VersionedEvent already carries it.
pub enum AppEvent {
    ContextChanged { target: ContextTarget },
    // ...
}
```

```rust
// Resolver hits Keystone project API; make it async.
pub async fn list_user_projects(&self) -> Result<Vec<ContextTarget>, SwitchError>;
```

```rust
// Keep cache key semantics aligned with existing storage model.
pub fn store_scoped(&self, scope: &TokenScope, token: &Token) -> io::Result<()>;
pub fn lookup_scoped(&self, scope: &TokenScope) -> Option<Token>;
```

---

**Missing components or interfaces**

- `ScopedAuthPort` (new port): required to mutate active scope without adapter downcast.
- `VersionedEvent` gate in dispatcher: mandatory single stale-drop authority.
- Switch transition policy for `Switching + (SwitchContext | SwitchBack | Cancel)`; currently unspecified.
- `ContextChanged` handling contract per component (clear list/detail/form pending state); currently aspirational only.
- Mock seam needs `begin_fail`, `commit_fail`, `rollback_called`, and ordered step assertions; current mock knobs are insufficient for NFR-3 failure-path tests.

---

**Verdict: REJECT**

**Brief reasoning:** DETAIL does not yet define an implementable atomic contract, violates intended port boundaries for auth scope mutation, and leaves stale-event isolation non-authoritative. These are correctness blockers, not polish issues.
tokens used
239,130
**Top 3 critical issues**

1. **Atomic boundary is internally contradictory and cannot guarantee rollback safety.**  
What: DETAIL says `ContextSessionPort` is the atomic boundary, but the switch flow bypasses it (`state.begin -> ... -> token_cache.store -> state.commit`) and never calls `session.commit/rollback` ([application-design.md#L191](/Users/jay.ahn/projects/infra/nexttui/.worktrees/runtime-context-switch/devflow-docs/inception/application-design.md#L191), [application-design.md#L200](/Users/jay.ahn/projects/infra/nexttui/.worktrees/runtime-context-switch/devflow-docs/inception/application-design.md#L200)). Also `invalidate_all()` is infallible while `CatalogFailed` and mock invalidate-failure are defined ([application-design.md#L133](/Users/jay.ahn/projects/infra/nexttui/.worktrees/runtime-context-switch/devflow-docs/inception/application-design.md#L133), [application-design.md#L223](/Users/jay.ahn/projects/infra/nexttui/.worktrees/runtime-context-switch/devflow-docs/inception/application-design.md#L223), [application-design.md#L263](/Users/jay.ahn/projects/infra/nexttui/.worktrees/runtime-context-switch/devflow-docs/inception/application-design.md#L263)).  
Why: You cannot deterministically handle “rescope OK + invalidate fail + rollback,” so NFR-1 atomicity is not met ([requirements.md#L69](/Users/jay.ahn/projects/infra/nexttui/.worktrees/runtime-context-switch/devflow-docs/inception/requirements.md#L69)).  
Fix: Make `ContextSessionPort` own `rescope + catalog refresh + commit/rollback` end-to-end; `ContextSwitcher` should orchestrate state/epoch only.

2. **Port boundary is wrong: current auth port cannot perform scope switch commit.**  
What: `AuthProvider` has no scope mutation APIs ([auth.rs#L8](/Users/jay.ahn/projects/infra/nexttui/.worktrees/runtime-context-switch/src/port/auth.rs#L8)), while `KeystoneAuthAdapter` holds fixed `active_scope` and all token reads key off it ([keystone.rs#L135](/Users/jay.ahn/projects/infra/nexttui/.worktrees/runtime-context-switch/src/adapter/auth/keystone.rs#L135), [keystone.rs#L357](/Users/jay.ahn/projects/infra/nexttui/.worktrees/runtime-context-switch/src/adapter/auth/keystone.rs#L357)).  
Why: `ScopedAuthSession` cannot be implemented cleanly through `AuthProvider`; it will require concrete adapter leakage/downcast, violating your intended layering.  
Fix: Add a dedicated scoped-auth port (`set_active_scope`, `upsert_scoped_token`, `current_scope_snapshot`) and keep `AuthProvider` read/request-auth oriented.

3. **Epoch/cancellation enforcement point is underspecified and currently bypassable.**  
What: DETAIL says worker spawn wraps one future and run_worker does epoch drop ([application-design.md#L278](/Users/jay.ahn/projects/infra/nexttui/.worktrees/runtime-context-switch/devflow-docs/inception/application-design.md#L278)), but current worker emits events from multiple polling paths ([worker.rs#L795](/Users/jay.ahn/projects/infra/nexttui/.worktrees/runtime-context-switch/src/worker.rs#L795), [worker.rs#L825](/Users/jay.ahn/projects/infra/nexttui/.worktrees/runtime-context-switch/src/worker.rs#L825), [worker.rs#L844](/Users/jay.ahn/projects/infra/nexttui/.worktrees/runtime-context-switch/src/worker.rs#L844)); event loop currently forwards blindly ([event_loop.rs#L48](/Users/jay.ahn/projects/infra/nexttui/.worktrees/runtime-context-switch/src/event_loop.rs#L48)).  
Why: FR-5 requires hard stale-drop guarantees across all async emitters, not just one spawn style ([requirements.md#L40](/Users/jay.ahn/projects/infra/nexttui/.worktrees/runtime-context-switch/devflow-docs/inception/requirements.md#L40)).  
Fix: Enforce epoch gate at a single choke point (App/event-loop dispatcher) and require all event sends to use `VersionedEvent<AppEvent>`.

---

**Specific signature corrections**

```rust
pub type Epoch = u64;

#[derive(Debug, thiserror::Error)]
pub enum SwitchError {
    #[error("switch already in progress")]
    InProgress,
    #[error("target not found: {0}")]
    NotFound(String),
    #[error("ambiguous target")]
    Ambiguous { candidates: Vec<ContextTarget> },
    #[error(transparent)]
    Api(#[from] ApiError),
    #[error(transparent)]
    Io(#[from] std::io::Error),
}

pub trait SwitchStateMachine {
    fn try_begin(&mut self, target: ContextTarget) -> Result<Epoch, SwitchError>; // not Result<_, SwitchError::InProgress>
    fn commit(&mut self, snapshot: ContextSnapshot);
    fn fail(&mut self, err: SwitchError);
}
```

```rust
#[async_trait]
pub trait ContextSessionPort: Send + Sync {
    type Handle: Send;
    async fn begin(&self, target: &ContextTarget, epoch: Epoch) -> Result<Self::Handle, SwitchError>;
    async fn rescope(&self, handle: &mut Self::Handle) -> Result<(), SwitchError>;
    async fn refresh_catalog(&self, handle: &mut Self::Handle) -> Result<(), SwitchError>;
    async fn commit(&self, handle: Self::Handle) -> Result<ContextSnapshot, SwitchError>;
    async fn rollback(&self, handle: Self::Handle) -> Result<(), SwitchError>;
}
```

```rust
pub async fn switch(&self, target: ContextTarget) -> Result<(Epoch, ContextSnapshot), SwitchError>;
pub async fn switch_back(&self) -> Result<(Epoch, ContextSnapshot), SwitchError>;
```

```rust
// Avoid duplicate epoch in payload when VersionedEvent already carries it.
pub enum AppEvent {
    ContextChanged { target: ContextTarget },
    // ...
}
```

```rust
// Resolver hits Keystone project API; make it async.
pub async fn list_user_projects(&self) -> Result<Vec<ContextTarget>, SwitchError>;
```

```rust
// Keep cache key semantics aligned with existing storage model.
pub fn store_scoped(&self, scope: &TokenScope, token: &Token) -> io::Result<()>;
pub fn lookup_scoped(&self, scope: &TokenScope) -> Option<Token>;
```

---

**Missing components or interfaces**

- `ScopedAuthPort` (new port): required to mutate active scope without adapter downcast.
- `VersionedEvent` gate in dispatcher: mandatory single stale-drop authority.
- Switch transition policy for `Switching + (SwitchContext | SwitchBack | Cancel)`; currently unspecified.
- `ContextChanged` handling contract per component (clear list/detail/form pending state); currently aspirational only.
- Mock seam needs `begin_fail`, `commit_fail`, `rollback_called`, and ordered step assertions; current mock knobs are insufficient for NFR-3 failure-path tests.

---

**Verdict: REJECT**

**Brief reasoning:** DETAIL does not yet define an implementable atomic contract, violates intended port boundaries for auth scope mutation, and leaves stale-event isolation non-authoritative. These are correctness blockers, not polish issues.
