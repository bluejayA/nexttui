# Backlog

## Pending

### BL-P2-052: Rescoped 토큰 자동 refresh + ContextChanged handler
**Priority**: High
**Category**: Auth / Functional Regression
**Description**: BL-P2-031 Unit 3b T2 review S2 + PR1 cargo-review integration finding.

**Part A — Rescoped 토큰 자동 refresh**: C1 가드 도입으로 KeystoneAuthAdapter는 initial scope 토큰만 자동 refresh. set_active(demo) 후 demo 토큰이 expire되면 `get_token`이 영구 실패 (`refresh_token` 가드가 AuthFailed 반환). 사용자 영향: ~55분 후 demo 세션이 모든 API 호출 실패.
필요 작업: ScopedAuthSession 또는 신규 RescopeRefresher가 active scope 토큰의 near-expiry를 감지 → KeystoneRescopePort로 새 토큰 발급 → set_active로 갱신. 또는 최소한 get_token 에러 메시지를 "session expired, please switch context again"으로 명확화.

**Part B — `AppEvent::ContextChanged` handler 구현**: 현재 ContextChanged는 fire-and-forget. handle_event에 arm이 없어 switch 성공 후 16개 모듈 캐시(Vec<Server> 등)가 이전 project 데이터 유지. T3 wire 이후에도 동일 문제. 필요 작업:
- `App.handle_event::ContextChanged` arm 구현
- 16개 Resource Module 캐시 invalidate (Vec 비움 + is_loading=true)
- `Fetch*` 일괄 dispatch
- router reset (필요 시) + toast ("Switched to project X")

**Acceptance**: switch 성공 → UI 즉시 새 project 데이터로 전환 + token expiry 자동 처리.
**Ref**: Security Reviewer S2 (P0 fix 리뷰), Cargo Review PR #68 통합 finding

### BL-P2-053: SwitchError NotAuthenticated variant + ApiError ScopeDrift variant
**Priority**: Medium
**Category**: Error model / Caller classification
**Description**: BL-P2-031 Unit 3b T2 review I1+I2. 현재 ScopedAuthSession::begin이 pre-auth 상태를 `SwitchError::Unsupported`로 매핑 (의미: 기능 미지원). 또한 KeystoneAuthAdapter의 C1 scope drift 가드가 `ApiError::AuthFailed` 사용 (의미: credential 거부 → 사용자가 reauth 시도하지만 같은 에러 반복).
필요 작업: `SwitchError::NotAuthenticated`(또는 `Precondition`), `ApiError::ScopeDrift`(또는 `InvalidState`) variant 추가 → caller가 분기 가능. SwitchError는 #[non_exhaustive] 없음 (외부 매처 영향 검토 필요), ApiError는 #[non_exhaustive] (안전).
**Ref**: Quality Reviewer I1+I2 (P0 fix 리뷰)

### BL-P2-054: KeystoneAuthAdapter Drop::abort + refresh task lifecycle
**Priority**: Medium
**Category**: Resource leak
**Description**: BL-P2-031 Unit 3b T2 Codex review. start_refresh_loop이 spawn한 tokio task가 self.credential/scope_ref/token_map의 strong Arc를 보유. adapter drop 시 JoinHandle abort 호출 없으므로 백그라운드 task가 계속 인증 시도. 프로세스 수명 내내 누적 가능.
필요 작업: KeystoneAuthAdapter::Drop 구현 → refresh_handle abort. 또는 CancellationToken 도입.
**Ref**: Codex P0 review

### BL-P2-055: Refresh loop 백오프 + 로그 rate-limit
**Priority**: Low
**Category**: Observability
**Description**: BL-P2-031 Unit 3b T2 review S3. C1 가드 도입 후 active scope drift 시 refresh loop이 sleep_duration 10s로 떨어지고 매 tick warn 로그 발행. demo 세션 expiry 후 분당 6회 누적.
필요 작업: scope drift 감지 시 최소 60s sleep 강제 또는 break 후 set_active 재발생까지 대기. 로그도 최초 1회 또는 N tick마다 1회.
**Ref**: Security Reviewer S3 (P0 fix 리뷰)

### BL-P2-056: TokenScope 정규화 일관화
**Priority**: Medium
**Category**: Auth / Correctness
**Description**: BL-P2-031 Unit 3b T2 Codex review. `TokenScope::from_credential`은 name/domain을 `to_lowercase()` 적용, `From<&ContextTarget> for TokenScope`는 원문 보존. 동일 프로젝트가 케이스 차이로 다른 키로 분리되어 토큰 캐시 miss 발생 가능.
필요 작업: 정규화 정책을 단일 위치에 통합 (TokenScope::Project 생성 시 항상 lowercase). resolver/parser/cache 경로 모두 검증.
**Ref**: Codex P0 review

### BL-P2-057: ScopedAuthPort/AuthProvider 동시성 race 테스트 (loom)
**Priority**: Low
**Category**: Test coverage
**Description**: BL-P2-031 Unit 3b T2 review M6. set_active ↔ refresh_token / authenticate 동시 호출 시 invariant (token_map ↔ active_scope 정합) 검증 부재. 현재 락 순서로 race-free하지만 회귀 방지를 위한 명시적 테스트 필요.
필요 작업: loom 또는 직접 thread spawn 기반 동시성 테스트.
**Ref**: Quality Reviewer M6 (P0 fix 리뷰)

### BL-P2-058: AuthCredential Zeroize 도입
**Priority**: Medium
**Category**: Security / Credential hygiene
**Description**: BL-P2-031 Unit 3b T2 Security I3. AuthCredential의 password가 refresh loop으로 클론되어 프로세스 수명 내내 heap 체류. Drop 시 zeroize 없음 → 메모리 덤프/core dump에 평문 password 노출 window.
필요 작업: zeroize crate 추가 → AuthCredential에 ZeroizeOnDrop. application_credential 우선 사용 권장 정책.
**Ref**: Security Reviewer I3

### BL-P2-059: Poison fail-closed 정책 전환 (auth 경로 한정)
**Priority**: Low
**Category**: Security / Defense-in-depth
**Description**: BL-P2-031 Unit 3b T2 Security I2. KeystoneAuthAdapter의 모든 락에서 `unwrap_or_else(|e| e.into_inner())` 사용 → poison 무시. 토큰 같은 security-critical 데이터에 대해 fail-secure 원칙과 충돌. 실제 panic 가능성은 낮으나 OWASP 권고와 대비.
필요 작업: auth 경로 한정으로 poison 시 인증 무효화 + 강제 재인증 트리거. 또는 각 사이트에 "왜 안전한가" 주석 추가.
**Ref**: Security Reviewer I2

### BL-P2-060: Action channel `Result<(), SendError<VersionedEvent<Action>>>` boxing
**Priority**: Low
**Category**: Performance / Code size
**Description**: PR1 cargo-review clippy `result_large_err`. `src/context/action_channel.rs:81`의 `pub fn send(&self, action: Action) -> Result<(), SendError<VersionedEvent<Action>>>`에서 Err variant가 176 bytes. send() 콜사이트가 수백 곳 (16개 모듈 전반)이라 모든 Result가 stack에 176-byte 슬롯 점유.
필요 작업: `Box<SendError<VersionedEvent<Action>>>`로 감싸거나 Action enum 자체를 Box화. bench로 실제 영향 (instruction cache miss, frame size) 측정 후 결정.
**Ref**: Cargo Review PR #68 clippy

### BL-P2-061: `SwitchStateView::Switching` large_enum_variant
**Priority**: Low
**Category**: Performance / Code size
**Description**: PR1 cargo-review clippy `large_enum_variant`. `src/context/state_machine.rs:55`의 `SwitchStateView::Switching` variant가 `ContextTarget`을 직접 보유 (적어도 352 bytes 큰 variant). state machine은 sync 코드라 핫 경로일 가능성.
필요 작업: `Switching { target: Box<ContextTarget>, ... }`로 변경 검토. clone 경로 전반에 영향. bench로 실제 영향 측정 후 결정.
**Ref**: Cargo Review PR #68 clippy

### BL-P2-063: Pre-existing clippy `-D warnings` 35건 일괄 정리 + CI 게이트
**Priority**: Medium
**Category**: Code quality / Tech debt
**Description**: PR1 cargo-review에서 `cargo clippy --lib --tests -- -D warnings` 실행 시 PR1 무관 pre-existing 35건 위반 발견 (PR #68 머지 후 재측정). 유형:
- `clippy::map_or` simplification (다수)
- `clippy::collapsible_if` / `collapsible_match`
- `clippy::doc_lazy_continuation` (doc 렌더링 깨짐)
- `clippy::manual_map`
- `clippy::result_large_err` — BL-P2-060 중복 영역, item-level `#[allow]` + reason으로 deferred
- `clippy::large_enum_variant` — BL-P2-061 중복 영역, item-level `#[allow]` + reason으로 deferred

기타 sed/scaffold 시점부터 누적된 idiom 위반.

**Canonical gate command** (local + CI 일치): `cargo clippy --lib --tests -- -D warnings`

**Suppression 정책**: `#[allow(clippy::<lint>)]`은 **item-level만 허용** (module-level 금지). 각 allow 사이트는 `reason = "..."` + BL ID 필수 명시. 시간 제한은 추적 BL의 acceptance 달성 시점.

**작업 스코프**:
1. `rust-toolchain.toml` 생성 — `stable` 채널 + clippy/rustfmt 컴포넌트 핀 (로컬/CI 일관성)
2. `cargo clippy --fix --lib --tests --allow-dirty` 자동 수정 → diff 검토 (clean 상태에서만)
3. 수동 수정 잔여 항목 처리. `result_large_err` / `large_enum_variant`는 BL-P2-060/061 item-level `#[allow]`로 위임
4. 잔여 위반 없이 canonical gate command 통과 확인
5. `.github/workflows/ci.yml` 신규 — 4-stage gate: fmt check / lib tests / clippy(--lib --tests) / bin compile
6. `Cargo.toml`의 `[lints.clippy]`에 추가 deny할 lint 검토 (현재 unwrap_used/expect_used/enum_glob_use만 deny)

**Acceptance**: `cargo clippy --lib --tests -- -D warnings` 0 errors. CI에서 동일 명령 실행 + 실패 시 머지 차단. `cargo build --bin nexttui` 통과. `cargo fmt --all -- --check` 통과.

**예상 작업량**: 1세션 (1~2시간). 자동 수정으로 70~80% 처리 예상.

**Ref**: Cargo Review PR #68 — "기존 clippy 위반 38개 — pre-existing, 본 PR 무관" 후속 처리. Codex planning review — CONDITIONAL APPROVE (R6: command scope 통일, R7: bin compile gate, R9: item-level allow 제약, R10: CI component pin)

### BL-P2-062: Stale action drop E2E 통합 테스트
**Priority**: Low
**Category**: Test coverage
**Description**: PR1 cargo-review missing test. switch 도중 큐에 쌓인 old-epoch action (e.g., FetchServers) → worker가 처리 → response event가 dispatcher epoch 게이트에서 drop되는 경로를 E2E로 검증하는 테스트가 없음. unit-level은 spawn_versioned/dispatcher 각자 검증되지만 통합 시나리오는 미커버.
필요 작업: `app.rs::tests`에 통합 테스트 추가 — (1) action 큐에 스테이지, (2) try_begin → epoch bump, (3) worker가 큐에서 꺼내 응답, (4) dispatcher가 stale event drop 확인.
**Ref**: Cargo Review PR #68 missing test

### BL-P2-064: `cargo audit` CI 통합 (공급망 보안)
**Priority**: High
**Category**: Security / CI
**Description**: BL-P2-063 (PR #70) cargo-review의 maintainability reviewer 제안. `Cargo.lock`이 존재하므로 즉시 적용 가능한 공급망 보안 기본선.
필요 작업:
1. `rustsec/audit-check@v2` GitHub Action을 `.github/workflows/ci.yml`에 추가
2. 기존 4-gate(fmt / test / clippy / bin) 뒤에 5번째 audit step으로 삽입
3. audit DB 캐시 설정 (매 run마다 DB 최신 유지 + 런타임 최적화)
4. 고위 CVE 발견 시 CI fail, 낮은 심각도는 warn (정책 결정 필요)

**Acceptance**: PR마다 RustSec advisory database 대비 dep tree 감사. 고위험 CVE는 머지 차단.
**예상 작업량**: 30분 (workflow YAML 추가 + 정책 1~2줄).
**Ref**: BL-P2-063 PR #70 cargo-review (agent C — maintainability reviewer 제안)

### BL-P2-065: Rust toolchain 정확한 버전 핀
**Priority**: Medium
**Category**: Reproducibility / CI
**Description**: 현재 `rust-toolchain.toml`이 `channel = "stable"` — floating pin. 매 stable 릴리스마다 clippy lint 셋/behavior 변경 가능 → "어제 green, 오늘 red" 가능성.
필요 작업:
1. `channel = "1.94.0"` 같은 정확한 버전 핀
2. Dependabot 또는 수동 주기(월 1회) bump 정책 문서화
3. MSRV (Minimum Supported Rust Version)를 `Cargo.toml`의 `rust-version` 필드에도 명시
4. Rust edition 2024는 1.85.0+ 필요 — 핀 버전이 호환되는지 검증

**Acceptance**: CI는 정확한 버전으로 실행. bump는 의도적 PR로만 발생.
**의존**: BL-P2-063 완료 (rust-toolchain.toml 자체가 PR #70에서 도입됨)
**Ref**: BL-P2-063 PR #70 cargo-review

### BL-P2-066: `.git-blame-ignore-revs` 도입 + AI 협업 blame hygiene 운영
**Priority**: Medium
**Category**: DX / AI collaboration infrastructure
**Description**: BL-P2-063 PR #70에 포함된 T1 clippy-fix + T2.5 fmt-all 두 commit이 squash merge로 main에 `9128305`로 통합됨. 이 merge commit SHA를 `.git-blame-ignore-revs`에 등록하면 blame UI (GitHub + 로컬)가 mechanical 변경을 투명하게 스킵 → 진짜 저자 복원.

AI 개발 맥락에서 이는 "preference" 수준이 아니라 **claude-code 세션의 context-building 효율에 직결되는 인프라**. 자세한 배경은 `docs/git-blame-hygiene-in-ai-devflow.md` 참조.

필요 작업 (단계별):
1. **즉시**: `.git-blame-ignore-revs` 파일 생성, PR #70 merge commit (`9128305`) 등록 + 주석
2. **중기**: devflow 플러그인 hook 확장
   - `PostToolUse` 또는 `Stop` hook: mechanical commit (chore(fmt)/chore(clippy)/chore(deps)/chore(codemod) prefix) 자동 감지
   - Squash merge 방식 고려 → feature SHA 대신 merge commit SHA를 등록하는 follow-up PR 자동 생성
3. **정책**: CONTRIBUTING.md에 Mechanical commit 판정 기준 명시 (`docs/git-blame-hygiene-in-ai-devflow.md` §7.1 표 참조)
4. **Commit message**: `Blame-Ignore: true` footer 표준화 (자동화용 마킹)

**Acceptance**:
- `.git-blame-ignore-revs` 파일 존재 + merge commit 최소 1개 등록
- GitHub blame UI에서 해당 commit 스킵 동작 확인
- 정책 문서화 완료

**예상 작업량**:
- Phase 1 (파일 추가): 15분
- Phase 2 (hook 자동화): 1세션 (2~3시간)
- Phase 3 (정책 문서화): 30분

**Ref**: PR #70 cargo-review (agent C), `docs/git-blame-hygiene-in-ai-devflow.md`

### BL-P2-067: Clippy 정책 확장 파일럿
**Priority**: Low
**Category**: Code quality policy
**Description**: 현재 `Cargo.toml [lints.clippy]` 3개만 deny (`unwrap_used`, `expect_used`, `enum_glob_use`). BL-P2-063 cargo-review가 추가 후보 제안.
검토 대상 lint:
- `clippy::unwrap_in_result` — Result에서 unwrap은 흔한 오류 패턴
- `clippy::panic` — 명시적 panic 금지 (필요 시 `#[allow]`)
- `clippy::todo` / `clippy::unimplemented` — 미완성 코드 감지
- `clippy::dbg_macro` — 디버깅 코드 잔류 방지

필요 작업:
1. 각 lint 후보에 대해 현재 코드베이스 영향도 벤치 (`cargo clippy -- -W clippy::<lint>` 횟수)
2. 영향도 낮은 것부터 점진 deny
3. BL-P2-063과 동일 패턴 (autofix + manual + allow) 적용

**Acceptance**: 각 lint 0건 도달 + deny 추가 + CI 통과 유지.
**의존**: BL-P2-063 완료 (베이스 깨끗한 상태)
**Ref**: BL-P2-063 PR #70 cargo-review

### BL-P2-068: GitHub Actions SHA pinning (공급망 강화)
**Priority**: Low
**Category**: Security / Supply chain
**Description**: 현재 `.github/workflows/ci.yml`의 actions는 태그 기반 (`@v4`, `@v2`, `@stable`). 태그는 repo 소유자가 이동 가능 → 공급망 공격 벡터.
대안: commit SHA로 핀 (e.g., `actions/checkout@abc123def...`).

필요 작업:
1. 현재 actions 4개 (`actions/checkout@v4`, `dtolnay/rust-toolchain@stable`, `Swatinem/rust-cache@v2`) 각각 최신 안정 SHA 조회
2. SHA로 교체, 주석에 원 태그 명시 (`# @v4`)
3. Dependabot 설정 (`.github/dependabot.yml`)으로 매주 자동 bump PR 생성
4. Dependabot bump PR의 리뷰 프로세스 문서화

**Acceptance**: 모든 action이 SHA로 핀. Dependabot이 bump PR 자동 생성.
**Ref**: BL-P2-063 PR #70 cargo-review (agent B — quality reviewer 제안)

### BL-P2-069: 벤치마크 프레임워크 도입 (BL-P2-060/061 선결)
**Priority**: Medium
**Category**: Performance measurement / Test infrastructure
**Description**: BL-P2-060 (action_channel Result boxing) / BL-P2-061 (SwitchStateView enum boxing)은 "벤치 기반 판단 필요"로 defer됨. 선결 조건인 벤치 프레임워크가 아직 없음.
필요 작업:
1. `criterion` crate를 dev-dependency로 추가
2. `benches/` 디렉토리 구조 설계 (예: `benches/action_channel.rs`, `benches/state_machine.rs`)
3. 초기 벤치 케이스 작성:
   - ActionSender::send p50/p95/p99
   - SwitchStateMachine.state() clone cost
   - Context switch 전체 flow (mock adapter)
4. CI 또는 로컬 전용 실행 정책 결정 (벤치는 시간 많이 걸리므로 CI에 포함 여부 트레이드오프)
5. Baseline 수치 확보 → 이후 BL-P2-060/061에서 boxing 후 비교

**Acceptance**: `cargo bench` 실행 가능 + 3개 이상 벤치 케이스 + baseline 결과 문서화.
**의존**: BL-P2-063 완료
**차단해제**: BL-P2-060, BL-P2-061
**Ref**: BL-P2-063 PR #70 cargo-review (agent C)

### BL-P2-050: LogPanel 텍스트 정제 (제어문자 필터링)
**Priority**: Low
**Category**: Security / UX
**Description**: LogPanel의 push()가 임의 문자열을 받아 그대로 렌더링. API 에러 메시지에 ANSI 제어문자가 포함되면 TUI 표시 교란 가능. 현재는 내부 문자열만 사용하므로 낮은 위험이나, 로그 내보내기 기능 추가 시 sanitization 필요.
**Ref**: Codex Batch 3 리뷰 #5

### BL-P2-051: 기존 Nova adapter encode_param() 통일
**Priority**: Low
**Category**: Security / Consistency
**Description**: get_server, delete_server, get_flavor, delete_flavor 등 기존 메서드에서 URL 파라미터를 raw interpolation으로 사용. UUID 특성상 공격 벡터 낮지만, 신규 메서드와의 일관성을 위해 encode_param() 통일 필요.
**Ref**: Security Reviewer Batch 2 Important-1

### BL-002: Snapshots 서비스 타입 매핑 수정
**Priority**: Low
**Category**: Bug
**Description**: FetchSnapshots가 "Service unavailable:" 에러 발생. Cinder snapshot API가 volume 서비스와 동일 엔드포인트를 사용하는데 별도 서비스로 조회하는 문제.

### BL-003: DevStack Glance↔Nova 통신 오류 조사
**Priority**: Low
**Category**: Infra
**Description**: Server 생성 시 500 에러 (GlanceConnection). DevStack 환경에서 Glance 이미지 접근 경로 확인 필요. nexttui 코드 문제 아님.

---

## Phase 2 Backlog

> Substation(Swift TUI) 리버스 엔지니어링 분석 + Phase 1 defer 항목 기반.
> Stage 1 = 아키텍처 고도화, Stage 2 = 기능 확장, Stage 3 = 신규 백엔드.

### Stage 1: 아키텍처 고도화

#### BL-P2-002: Multi-Level Cache (L1/L2/L3)
**Priority**: Medium
**Category**: Architecture
**Description**:
- 현재 L1(인메모리 HashMap) 단일 레벨 → 3단계 티어링 도입
  - L1: 인메모리 (현재와 동일, 가장 빠름)
  - L2: gzip 압축 메모리 (대용량 리소스 목록의 메모리 사용량 절감)
  - L3: 디스크 영속 (앱 재시작 시 콜드 스타트 없이 즉시 표시)
- 리소스 종류별 차등 TTL (현재 구현됨) + 티어별 승격/강등 정책
- `cloud` 필드 기반 멀티 클라우드 캐시 격리 (현재 CacheKey에 cloud 있으나 L1만 활용)
- L3 디스크 캐시: index/data 일관성 보장, 비정상 종료 대비 WAL 또는 atomic write
**Motivation**: VDI 환경에서 앱 재시작이 잦고 서버 목록이 수천 건일 때 매번 API 전체 fetch는 비효율적. L2 압축으로 메모리 절약, L3 영속으로 콜드 스타트 개선
**Ref**: Substation `MultiLevelCacheManager.swift`, `OpenStackCacheManager.swift`
**주의**: Substation의 3중 캐시(OSClient L1/L2/L3 + TUI cacheManager + DataManager MemoryKit) 간 일관성 문제(Risk #1)를 반복하지 않도록, nexttui에서는 단일 Cache 구조체 내에서 티어링 구현

#### BL-P2-003: Intelligent Cache Invalidation (의존성 그래프 기반)
**Priority**: High
**Depends on**: BL-P2-002
**Category**: Architecture
**Description**:
- 리소스 간 의존성 그래프 구축 (예: Server → FloatingIP, Port, Volume)
- CUD 액션 완료 시 관련 리소스 캐시를 연쇄 무효화
- 시간 지연 무효화 지원 (3~10초, API 전파 시간 고려)
- 현재 Cache(RwLock, TTL, GC)를 확장하되 Substation의 3중 캐시 복잡성은 피함
**Motivation**: 서버 삭제 후 FloatingIP 목록에 여전히 연결된 IP가 보이는 등 캐시 불일치 문제 예방. 현재는 단순 TTL 만료에만 의존
**Ref**: Substation `IntelligentCacheInvalidation.swift`

#### BL-P2-003-B: DataProvider Registry 패턴
**Priority**: Medium
**Category**: Architecture
**Description**:
- 문자열 키 기반 DataProvider 조회 (`DataProviderRegistry.fetch("servers")`)
- 각 DataProvider가 Port를 통해 API 호출 → 결과 캐싱 → 갱신 이벤트 발행
- `refreshAllDataOptimized()`: Phase 1(독립) 리소스 우선 로드 → Phase 2/3 순차 로드
- `executeWithTokenRefresh` 래핑: 토큰 만료 시 자동 재인증 후 재시도
**Motivation**: 현재 각 모듈이 개별적으로 데이터를 fetch하는데, 공통 DataProvider 레이어로 캐시/재시도/우선순위를 중앙 관리
**Ref**: Substation `DataManager.refreshAllDataOptimized()`, `ServersDataProvider`

#### BL-P2-004: Adaptive Polling (이벤트 루프 최적화)
**Priority**: Low
**Category**: Performance
**Description**:
- 활성 입력 감지 시: 짧은 폴링 간격 (5ms)
- 유휴 상태: 지수 백오프로 최대 30ms까지 증가
- 현재 crossterm 이벤트 루프의 고정 폴링 간격을 적응형으로 전환
**Motivation**: VDI 환경에서 CPU 사용량 최소화. 입력 시 빠른 반응성 유지하면서 유휴 시 CPU 양보
**Ref**: Substation `TUI.swift` nodelay 모드 + 지수 백오프

#### BL-P2-005: ViewModel 분리 (Domain Model ↔ UI 표현 결합도 감소)
**Priority**: Medium
**Category**: Architecture
**Description**:
- Domain Module의 UI 표현 로직(컬럼 정의, 색상, 포맷팅)을 `view_model` 모듈로 분리
- Domain Model은 순수 데이터, ViewModel이 UI 위젯 파라미터 변환 담당
- UI 위젯 변경 시 view_model만 수정, Domain Model과 Component 로직은 무변경
**Motivation**: Agent Council Review 액션 아이템 #4. 현재 모듈 내에서 모델과 UI 표현이 혼재
**Ref**: Council review `agent-council-review.md` 항목 4

#### BL-P2-006: Microversion 협상
**Priority**: Low
**Category**: Infrastructure
**Description**:
- 서비스별 지원 API 마이크로버전을 자동 협상
- `X-OpenStack-Nova-Microversion` 등 헤더 자동 주입
- 서비스 카탈로그에서 버전 정보 추출 → 요청별 적절한 버전 헤더 설정
**Motivation**: OpenStack 서비스는 동일 엔드포인트에서 마이크로버전별로 응답 스키마가 달라짐. 현재는 고정 버전 또는 버전 미지정으로 호출
**Ref**: Substation `OpenStackClientCore.swift` MicroversionManager

### Stage 2: 기능 확장

#### BL-P2-011: 감사 로그 (Audit Log)
**Priority**: Medium
**Category**: Feature
**Description**:
- `~/.config/nexttui/audit.log`에 JSON Lines 형식 기록
- `AuditLogger`가 Action 채널 구독, 매 기록 즉시 flush
- 기록 항목: 타임스탬프, 사용자, 액션 종류, 대상 리소스, 결과(성공/실패)
- Log rotation 지원
**Motivation**: FR-18 (감사 로그). 운영 환경에서 누가 어떤 작업을 했는지 추적 필요
**Ref**: `detail-design-domain-nfr.md` 항목 G, user-story US-047

#### BL-P2-012: 통합 조회 (서버-리소스 연관 뷰)
**Priority**: Medium
**Category**: Feature
**Description**:
- 서버 상세에서 연결된 Volume, FloatingIP, SecurityGroup, Network를 한 화면에 표시
- 리소스 간 연관 관계 그래프 기반 탐색
**Motivation**: FR-19 (통합 조회). 현재 각 리소스를 개별 모듈에서만 볼 수 있어 서버 전체 상태 파악이 어려움
**Ref**: user-story US-048

#### BL-P2-013: UsageModule (리소스 사용량 모니터링)
**Priority**: Medium
**Category**: Feature
**Description**:
- Nova simple-tenant-usage API 연동
- 프로젝트별 vCPU, RAM, 디스크 사용량 표시
- 쿼터 대비 사용률 시각화
**Motivation**: Phase 1에서 deferred. 운영자가 프로젝트별 리소스 현황을 빠르게 확인할 수 있어야 함
**Ref**: session-summary Unit 14 (UsageModule deferred)

#### BL-P2-015: Attach / Detach / Associate 워크플로우
**Priority**: Medium
**Category**: Feature
**Description**:
- Volume Attach/Detach: 서버에 볼륨 연결/분리
- Volume Migration: 스토리지 백엔드 간 볼륨 이동 (US-032)
- FloatingIP Associate/Disassociate
- Role Grant/Revoke
- Quota Management
**Motivation**: Phase 1에서 defer된 CUD 확장. 빈번한 정기 운영 액션
**Ref**: session-summary Next Steps, user-stories US-027, US-030~032

#### BL-P2-031: 프로젝트 전환 + Keystone Rescoping (#39)
**Priority**: High
**Category**: Feature
**Depends on**: BL-P2-029 (다중 토큰 맵, 완료)
**Description**:
- 런타임 프로젝트/클라우드 전환 (SwitchCloud / SwitchProject)
- CommandParser→App 레벨에서 cloud 컨텍스트 전환
- Keystone rescoping으로 토큰 재발급 없이 프로젝트 전환
- Auth 재생성 플로우
**Motivation**: --cloud CLI(PR#55)로 시작 시 선택 가능하지만, 런타임 중 전환 미지원
**Ref**: GitHub Issue #39

#### BL-P2-016: 토큰 보안 강화
**Priority**: Low
**Category**: Security
**Description**:
- 메모리 내 토큰 암호화 저장 (AES-GCM 또는 OS keychain)
- 선제 갱신: 만료 5분 전 자동 갱신 시작
- HTTP 429/5xx 지수 백오프 재시도 (최대 3회)
**Motivation**: Substation이 AES-GCM으로 토큰 보호. 현재 nexttui는 plaintext 토큰 보유
**Ref**: Substation `OpenStackClientCore.swift` CoreTokenManager

#### BL-P2-017: 멀티 인증 방식 지원
**Priority**: Low
**Category**: Feature
**Description**:
- Keystone v3 password (현재) + appCredential + token 인증 지원
- HMAC (Cloudian), API Key 등 비-Keystone 인증 확장
- `AuthProvider` trait에 `sign_request()` 메서드 확장 (현재 Phase 2 주석만 존재)
**Motivation**: FR-05.4, FR-05.5. 멀티 백엔드 환경에서 인증 체계 중립적 설계 필요
**Ref**: `detail-design-port-adapter.md` Phase 2 HMAC/API Key 주석

#### BL-P2-033: TestBackend 스냅샷 테스트 확장
**Priority**: Low
**Category**: Testing / UX Verification
**Description**:
- 현재 FormWidget에만 있는 `TestBackend` 렌더 테스트를 핵심 UI 전체로 확장
- `insta` 크레이트 도입으로 스냅샷 기반 렌더링 회귀 테스트 구축
- 대상: 서버 리스트 테이블, 디테일 패널, 네비게이션 바, 상태 바 등
- 다양한 터미널 크기(40x10, 80x24, 120x40 등)에서 레이아웃 깨짐 검증
- 키 입력 후 상태 변화 → 재렌더 → 스냅샷 비교 시나리오
**Motivation**: 현재 830+ 테스트가 로직/상태 전이를 검증하지만, 실제 렌더링 출력은 FormWidget 5개 테스트만 커버. UI 변경 시 레이아웃 깨짐이나 스타일 변경을 자동 감지할 수 없음
**Ref**: `src/ui/form.rs:2160` 기존 `render_to_buffer` 헬퍼

#### BL-P2-018: 커스텀 키 바인딩
**Priority**: Low
**Category**: UX
**Description**:
- Config 파일에서 키 바인딩 커스터마이징 로드
- 기본 키맵 + 사용자 오버라이드
**Motivation**: detail-design-ui-input.md에서 Phase 2로 분류
**Ref**: `detail-design-ui-input.md` Config 항목

#### BL-P2-019: 이미지 로컬 파일 업로드
**Priority**: Low
**Category**: Feature
**Description**:
- Glance 이미지 생성 시 URL 지정 외에 로컬 파일 업로드 지원
- 파일 선택 UI + 진행률 표시
**Motivation**: Phase 1은 URL 지정만 지원
**Ref**: `detail-design-domain-nfr.md` line 1171

#### BL-P2-020: Service Layer 전환 대비
**Priority**: Low
**Category**: Architecture
**Description**:
- AdapterRegistry에서 직접 호출 Adapter를 Service Layer 프록시 Adapter로 교체
- Admin API GW 경유 모드 지원
- `replace_*()` 메서드 활용한 런타임 Adapter 스왑
**Motivation**: TR-09. Phase 1의 Thick Client에서 Phase 2의 Service Layer 중심으로 점진 전환
**Ref**: `detail-design-port-adapter.md` Phase 2 Adapter Swap 섹션

### Stage 2.5-B: Visual Enhancement (Medium Priority)

> Stage 2.5-A (Theme & Polish)는 전체 완료 — PR #51~#53
> Stage 2.5-B 전체 완료 — PR #60

### Stage 2.5-C: Advanced Layout (Low Priority)

##### BL-P2-044: 반응형 레이아웃 모드
**Priority**: Low
**Category**: UX
**Description**:
- `LayoutMode` enum: TooSmall / Compact / Standard / Wide
- Compact (< 120x30): Sidebar 숨김, 단일 패널
- Standard (120x30+): 현재 Sidebar + Content
- Wide (200x50+): Sidebar + List + Detail 동시 표시 (3-column)
- devflow-tui `LayoutManager` 패턴 참조
**Motivation**: 터미널 크기에 따라 최적 레이아웃 제공

##### BL-P2-045: 다크 테마 옵션
**Priority**: Low
**Category**: UX
**Depends on**: BL-P2-034 (완료)
**Description**:
- Config에서 `theme: dark | light` 선택
- 다크 테마: btop 스타일 어두운 배경 + 밝은 텍스트
- Theme 구조체에 variant 추가
**Motivation**: btop처럼 다크 테마가 장시간 모니터링에 적합

##### BL-P2-046: NO_COLOR 접근성 지원
**Priority**: Low
**Category**: UX
**Depends on**: BL-P2-034 (완료)
**Description**:
- `NO_COLOR` 환경변수 감지 시 색상 대신 Bold/Dim/Underline만 사용
- devflow-tui `no_color()` 패턴 참조
- 모든 Theme 메서드에 NO_COLOR 분기 추가
**Motivation**: 터미널 접근성 표준 (https://no-color.org)

### Stage 3: 신규 백엔드

#### BL-P2-021: Manila (Shared FS / NAS)
**Priority**: Low
**Category**: New Backend
**Description**: Share Network, Migration, QoS, CIFS Account 관리
**Ref**: user-stories Phase 2 예정 서비스

#### BL-P2-022: Cloudian (Object Storage)
**Priority**: Low
**Category**: New Backend
**Description**: Policy, Bucket, Group, Monitor, Permission, QoS 관리. S3 HMAC 인증 필요 (BL-P2-017 선행)
**Ref**: user-stories Phase 2 예정 서비스

#### BL-P2-023: Network System Admin
**Priority**: Low
**Category**: New Backend
**Description**: Routing Table, VPC, Subnet, Routing Rule, NACL, External Network 관리
**Ref**: user-stories Phase 2 예정 서비스

#### BL-P2-024: Placement
**Priority**: Low
**Category**: New Backend
**Description**: Resource Provider, Inventory 조회/관리
**Ref**: user-stories Phase 2 예정 서비스

## Completed

- **BL-001**: Submit 확인 화면 + Toast 피드백 (PR #27, 2026-03-25)
- **BL-P2-001**: Module Registry 시스템 (PR #28, 2026-03-25)
- **#32 BL-P2-027**: Error enum `#[non_exhaustive]` 적용 (PR #36, 2026-03-26)
- **#30 BL-P2-025**: Clippy 엄격 lint 정책 도입 (PR #36, 2026-03-26)
- **#35 BL-P2-030**: Pagination Combinator 추상화 (PR #36, 2026-03-26)
- **#31 BL-P2-026**: tracing 구조적 로깅/계측 도입 (PR #37, 2026-03-26)
- **#33 BL-P2-028**: 토큰 캐시 파일 영속화 (PR #38, 2026-03-26)
- **#34 BL-P2-029**: Scope 기반 다중 토큰 맵 (PR #40, 2026-03-26)
- **#12 BL-P2-010**: RBAC 3단계 권한 제어 (PR #42, 2026-03-26)
- **#41 BL-P2-032**: 전체 프로젝트 리소스 조회 all_tenants (PR #43, 2026-03-27)
- **#16 BL-P2-014**: Server Migration / Evacuate (PR #44, 2026-03-30)
- **Server Resize**: flavor SelectPopup (PR #47~#48, 2026-03-31)
- **BL-P2-034**: Theme 시스템 도입 (PR #51, 2026-03-31)
- **BL-P2-035**: Rounded 보더 + 포커스 피드백 (PR #51, 2026-03-31)
- **BL-P2-036**: 패널 타이틀 포맷 (PR #51, 2026-03-31)
- **BL-P2-037**: 상태바 리디자인 (PR #51, 2026-03-31)
- **BL-P2-038**: 리스트 하이라이트 개선 (PR #51, 2026-03-31)
- **BL-P2-039**: 헤더 리디자인 (PR #52, 2026-03-31)
- **BL-P2-040**: 상태 아이콘 도입 (PR #53, 2026-04-01)
- **UX 가시성 수정**: ALL 뱃지, admin 마커, 패널 타이틀 (PR #54, 2026-04-01)
- **--cloud CLI**: 시작 시 클라우드 선택 (PR #55, 2026-04-01)
- **Auto-Refresh Polling**: FetchDedup + API Backoff (PR #56, 2026-04-02)
- **Activity Log**: StatusBar 에러 뱃지 (PR #57, 2026-04-02)
- **help_hint()**: 14개 모듈 컨텍스트 인식 힌트 (PR #58, 2026-04-02)
- **HostModule**: Composite Host Operations Panel (PR #59, 2026-04-04)
- **BL-P2-041**: 스크롤바 추가 (PR #60, 2026-04-06)
- **BL-P2-042**: Content 보더 컨테이너 (PR #60, 2026-04-06)
- **BL-P2-043**: Detail 섹션 구분 개선 (PR #60, 2026-04-06)
- **BL-P2-015**: Volume Attach/Detach + FloatingIP Associate/Disassociate (PR #61, 2026-04-08)
- **BL-P2-011**: 감사 로그 Audit Log 연동 (PR #62, 2026-04-08)
- **BL-P2-012**: 통합 조회 — SG 섹션 + 리소스 네비게이션 (PR #63, 2026-04-08)
- **BL-P2-013**: UsageModule — btop 스타일 사용량 대시보드 (PR #64, 2026-04-10)
- **BL-P2-005**: ViewModel 분리 — ViewContext 패턴 도입 (PR #65, 2026-04-10)
