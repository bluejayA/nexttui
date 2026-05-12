# Application Design

**Mode**: LIST + DETAIL + COUNCIL-REVISION
**Timestamp**: 2026-04-24T13:45:00+09:00 (Council 리뷰 반영)
**Review Ref**: `devflow-docs/inception/design-review-raw/synthesis.md` (must-fix 7건 반영)
**Work Item**: BL-P2-085 — Cross-project scoping 전면 fix
**Approach**: A안 Atomic Security Fix

## Assumption Verification (LIST 선결 실측 — Council 리뷰 후 교정)

| Assumption | 결과 | Notes |
|------------|------|------|
| A1 active_scope 전파 | ✅ (type refs 교정) | **실제 타입 (Codex 리뷰에서 지적)**: `TokenScope` enum (`src/port/types.rs:42`)는 `Project { name, domain }` variant로 project_id **없음**. `ScopedAuthSession` (`src/adapter/auth/scoped_session.rs:33`)도 project_id/name 필드 **없음** (ports만 보유). 실제 project_id 소스: **`Token.project: ProjectScope { id, name, domain_id, domain_name }`** (`src/port/types.rs`). 앱 레벨 캐시: **`RbacGuard::project_id() -> Option<String>`** (`src/infra/rbac.rs:113`). 1차 소스는 Token.project.id, RbacGuard는 파생 캐시. |
| A2 mock HTTP 인프라 | ⚠️ 부정 | dev-deps에 `mockito`/`wiremock` 부재. **설계 영향**: FR1은 (a) URL/query pure fn 테스트 + (b) **client-side re-filter** (adapter에서 응답 파싱 후 scope 불일치 리소스 제거, defense-in-depth) 단위 테스트 조합으로 수용 기준 커버. HTTP 서버 mock dev-dep은 회피. |
| A3 `build_disambiguated_opts` 순수성 | ✅ | `src/module/server/mod.rs:32` 순수 제네릭 함수, 2 호출부 (lines 1103, 1115). 이번 BL에선 확장 없이 유지. |

## 기존 구조 주요 발견 (Council 리뷰 후 보강)

- `src/infra/rbac.rs`에 `RbacGuard`(`project_id()`, `can_perform(ActionKind)`, `can_access_route`, `has_capability`) + `ActionKind` enum + `EffectiveRole` 이미 존재.
  - FR3는 **신규 struct가 아니라 `RbacGuard` 확장**.
- **`action_to_kind()` 매핑 이미 존재** (`src/worker.rs:151`, Codex 리뷰 발견): 모든 Action을 `Option<ActionKind>`로 분류. `Some(_)` == mutation, `None` == read-only. → **신규 `is_mutation()` 도입 불필요**, 이 기존 함수를 `is_mutation(a) = action_to_kind(a).is_some()` 형태로 재사용.
- **`ActionSender` + `VersionedEvent<Action>` 중앙 envelope 이미 존재** (`src/context/action_channel.rs:23`, `src/context/versioned.rs`, Codex 리뷰 발견):
  - `ActionSender { tx: mpsc::UnboundedSender<VersionedEvent<Action>> }`
  - `VersionedEvent<T> { payload: T, epoch: Epoch }` — generic, T 교체 가능
  - **설계 전환**: 55 call site 수정 대신 **이 envelope에 origin_project_id를 통합**. T = Action을 T = DispatchedAction으로 교체.
- `run_worker` (`src/worker.rs:53`)가 단일 worker 엔트리. action_to_kind()가 이미 line 67에서 호출됨. 가드 hook은 이 근처.
- `NeutronHttpAdapter::new(auth: Arc<dyn AuthProvider>, region)` — auth trait 주입 이미 존재. `AuthProvider::get_token_info().project.id`로 active project_id 취득.

## 컴포넌트 목록 (Council 리뷰 후 개정)

| 컴포넌트 | 책임 | 타입 | 신규/확장 | 위치 | 해당 FR |
|---------|------|------|----------|------|---------|
| ~~`ActionKind::is_mutation()`~~ | ~~신규 분류 메서드~~ | ~~Util~~ | **폐기** | — | FR2 (action_to_kind()로 충족) |
| `action_to_kind()` parity guard | **기존 함수** 유지. is_mutation 의미는 `action_to_kind(a).is_some()`. RBAC ActionKind 매핑과의 parity 테스트 추가 (미분류 Action 회귀 방지) | Test | 보강 | `src/worker.rs:151` + 테스트 | FR2 |
| ~~`StampedAction` wrapper~~ | ~~Action + origin_project_id 래핑~~ | ~~Util~~ | **폐기 → DispatchedAction으로 교체 (아래)** | — | FR2 |
| `DispatchedAction` | 중앙 envelope 페이로드. `{ action: Action, origin_project_id: Option<String> }`. `VersionedEvent`의 T 파라미터로 사용. read-only Action은 `None`, mutation Action은 `Some(scope)` | Util | 신규 | `src/action.rs` (Action enum 옆) | FR2 |
| `RbacGuard::check_project_scope()` | role-tier 가드 위에 project_id 일치 검사 누적(AND) 적용. reason(`role_tier`/`project_scope`/`both`) 분리 반환 | Service | 확장 | `src/infra/rbac.rs` | FR3 |
| `RbacGuard::update_roles_preserve_project()` | token refresh 시 project_id=None 덮어쓰기 방지. 명시적 preserve API 또는 기존 update_roles를 `Option<Option<String>>` 시맨틱으로 수정 | Service | 확장 | `src/infra/rbac.rs` | FR3 (token refresh 방어) |
| `CrossProjectGuard` | origin_project_id vs current active 비교 + form selection 검증 순수 함수. RbacGuard와 협업 — worker/form 공통 호출부 | Util | 신규 | `src/infra/cross_project_guard.rs` | FR2, FR3, FR4 공통 |
| `ActionSender::send()` 중앙 스탬핑 | send 호출부에서 current scope 취득 후 `DispatchedAction { action, origin }`로 감싸 `VersionedEvent<DispatchedAction>` 발행. 호출 레벨 변경 0 | Service | 확장 | `src/context/action_channel.rs` | FR2 |
| `WorkerMutationGuardHook` | `run_worker`의 epoch-match 직후, DispatchedAction에서 `action_to_kind(action).is_some()`인 경우 `CrossProjectGuard::check_origin` 호출. 실패 시 Action 거부 + 이벤트 emit + 토스트 전파 | Service | 확장 | `src/worker.rs` | FR2 |
| `NeutronListQueryBuilder` | SG/Network/FloatingIp/Subnet List 빌더에 `tenant_id` 주입. URL 조립 pure fn 추출 | Adapter | 확장 | `src/adapter/http/neutron.rs` | FR1 |
| `NeutronResponseRefilter` | 응답 파싱 결과에 대해 **client-side re-filter** (project_id != active인 리소스 제거). defense-in-depth | Adapter | 확장 | `src/adapter/http/neutron.rs` (paginated_list 결과 후처리) | FR1 (응답측 수용기준) |
| `NovaCinderAllTenantsPolicy` | nova/cinder list 엔드포인트 `all_tenants` 플래그 기본 false 명시 + opt-in 파라미터 + 응답 re-filter | Adapter | 확장 | `src/adapter/http/nova.rs`, `src/adapter/http/cinder.rs` | FR1 |
| `FormSelectedIdValidator` | 제출 시점 선택된 리소스 ID의 project_id vs active scope 비교. 실패 시 form reject + 에러 토스트. **Glance 이미지 등 FR1 제외 리소스의 mutation path에 필수 적용**. 공통 헬퍼 | Util | 신규 | `src/module/common/scope_validator.rs` | FR4 |
| `CrossProjectBlockEvent` | 이벤트 스키마 struct + fingerprint(version-prefixed + delimiter + 명시적 None 규칙) 계산 + tracing emit + tag prefix `[cross-project-guard]`. best-effort append | Util | 신규 | `src/infra/audit/cross_project_block.rs` | FR5 |
| `CrossProjectToast` | 차단 시 공통 카피 생성. `active_project_name` + `target_project_name`/`origin_project_name` 포함. Toast level = Warning. 단일 정의 공통 모듈 | Util | 신규 | `src/ui/cross_project_toast.rs` | FR6 |

총 **12개 컴포넌트** (신규 5, 확장 7, 폐기 2). 이전 LIST의 9개에서 개편:
- `ActionKind::is_mutation()` 폐기 (기존 action_to_kind 재사용)
- `StampedAction` 폐기 → `DispatchedAction` + `ActionSender::send()` 중앙 스탬핑으로 대체
- `NeutronResponseRefilter` 신규 (FR1 응답측 수용기준 + defense-in-depth)
- `RbacGuard::update_roles_preserve_project` 신규 (token refresh 방어)
- `action_to_kind parity guard` 보강 (테스트)

## DETAIL 단계에서 확정 기대

1. **Q1 정식 확정** `tenant_id` 필터 불가 endpoint 매트릭스 — keystone 제외 nova/neutron/cinder/glance 전수 조사. 불가 endpoint는 worker/RBAC 레벨에서 커버.
2. **Q3 정식 확정** Worker 거부 에러 variant — 신규 `WorkerError::CrossProjectBlocked { actor, active, target }` vs 기존 `RbacDenied` 재사용. `src/worker.rs`의 현재 에러 타입 + `src/error.rs`와 호환성 확인.
3. **A1 교정 반영**: "active_scope" → `TokenScope` 용어 통일 (requirements.md Change Log에 후속 반영 권고).
4. **`AuthProvider` trait surface 확인**: `project_id()` 메서드 제공 여부. 없으면 추가 필요.
5. **`run_worker` Action target 식별**: `Action`이 `target_project_id()`를 어떻게 노출하는지 (parameter? enum variant? self.target?). Worker guard hook 신호 보강 필요 여부.
6. **FormSelectedIdValidator 위치 확정**: `src/module/common/` 존재 여부. 없으면 `src/ui/` 쪽 배치.
7. **에러 전파 경로**: Worker 거부가 Toast로 올라가는 채널 (기존 `Event` / `Toast` 경로 재사용).

## Scope Exclusions (재확인)

- `build_disambiguated_opts` 다른 모듈 확장 = 제외
- 감사 로그 뷰어 / 패턴 대시보드 = 제외 (스키마만 확정)
- PII 해싱 = 제외
- HTTP 서버 mock dev-dep 추가 = 회피 (pure fn 추출로 대체)

## Decision Log

- **[2026-04-24] A2 mitigation = pure fn 추출**: mockito/wiremock-rs 도입 대신 URL/query 빌더를 pure function으로 추출하여 단위 테스트. 근거: (a) 이번 BL은 보안 경계 fix지 테스트 인프라 근대화가 아님 (스코프 규율), (b) FR1 수용기준의 본질은 URL 조립까지이며 그 이후는 reqwest stable, (c) 기존 adapter 테스트 스타일(serde body 단위)과 일관, (d) mock server가 필요해지면 BL-P2-081(HTTP 경로 자체 손대는 BL)에서 도입이 자연스러움.
- **[2026-04-24] FR2 구현 = 2b Stamped wrapper**: `Action` enum이 `project_id`를 per-variant로 가지지 않음을 실측 확인 (55 dispatch site). 중앙 dispatch 지점에서 `StampedAction { action, origin_project_id }` 래퍼로 감싸 worker가 `origin == current_active`를 검사. 근거: TOCTOU + cache-stale 시나리오를 정확히 커버하면서 diff 최소. ID 위조 시나리오는 FR1+FR4에 위임 (TUI 환경에서 현실적 위협 최소).
- **[2026-04-24] Q3 = AppError 확장**: 신규 `WorkerError` enum 도입 대신 기존 `AppError`(`#[non_exhaustive]`)에 variant 추가. 근거: 현재 프로젝트에 `WorkerError`/`RbacError` 분리된 타입 없고 `AppError`가 공통 채널. 미래 확장 여지는 `#[non_exhaustive]`로 보존.

---

## 컴포넌트 상세 설계 (DETAIL, Standard depth)

아래는 각 컴포넌트의 **public interface**(주요 2~3 메서드) + **의존성**. 실제 시그니처는 TDD RED 단계에서 테스트가 강제하는 대로 세부 조정됨.

### C1. `action_to_kind()` parity guard [보강 — 신규 `is_mutation()` 폐기]

**Responsibility**: 기존 `action_to_kind(action: &Action) -> Option<ActionKind>` 함수(`src/worker.rs:151`)를 "mutation 분류자"로 재사용. `Some(_)` == mutation, `None` == read-only.

**Interface** (변경 없음, 이미 존재):
```rust
// src/worker.rs:151 (이미 구현됨)
fn action_to_kind(action: &Action) -> Option<ActionKind> {
    match action {
        Action::CreateServer(_) | ... => Some(ActionKind::Create),
        Action::DeleteServer { .. } | ... | Action::DeleteImage { .. } => Some(ActionKind::Delete),
        Action::DeleteVolume { force: true, .. } => Some(ActionKind::ForceDelete),
        // ... 읽기 액션은 해당 arm에서 None
        _ => None,
    }
}

// 신규 편의 함수 (옵션)
#[inline]
pub fn action_is_mutation(action: &Action) -> bool {
    action_to_kind(action).is_some()
}
```

**보강할 것**:
1. **Parity 테스트**: 모든 `Action::*` variant에 대해 `action_to_kind` 매핑을 exhaustive match로 돌려 fallthrough(`_`)를 제거. 신규 variant 추가 시 컴파일 실패로 강제.
2. **read-only variant 리스트 명시화**: 현재 `_ => None` fallthrough를 각 `Fetch*`/`Navigate`/`Back` 등으로 풀어서 exhaustive화.

**Test strategy**: `test_action_to_kind_cud_actions` (이미 존재, line 939)에 다음 추가:
- 모든 `Fetch*` / `Navigate` / `Back`에 대해 `action_to_kind(a) == None` 검증
- 모든 mutation variant에 대해 `Some(_)` 검증 (범주별 정확 매핑 assertion)
- Glance 특히: `DeleteImage` / `CreateImage` 검증 (Codex의 Glance gap 보강)

**Dependencies**: 없음 (기존 함수 유지 + 테스트 보강).

---

### C2. `DispatchedAction` envelope + `ActionSender` 중앙 스탬핑 [신규 → 기존 `StampedAction` 폐기]

**핵심 설계 변경**: Council 리뷰에서 **기존 `ActionSender` + `VersionedEvent<Action>` 중앙 envelope** (`src/context/action_channel.rs`) 발견. 새 래퍼를 55 call site에 적용하는 대신 **기존 envelope의 T 파라미터를 교체**.

**Responsibility**: Action에 origin project_id 스탬프를 부착. `ActionSender::send()` 내부에서 자동 스탬핑.

**Location**: `src/action.rs` (DispatchedAction struct) + `src/context/action_channel.rs` (ActionSender 확장)

**Interface**:
```rust
// src/action.rs
#[derive(Debug, Clone)]
pub struct DispatchedAction {
    pub action: Action,
    pub origin_project_id: Option<String>,  // Some for mutation, None for read-only
}

// src/context/action_channel.rs
pub struct ActionSender {
    tx: mpsc::UnboundedSender<VersionedEvent<DispatchedAction>>,  // T 교체 (Action → DispatchedAction)
    epoch_source: Arc<...>,       // 기존
    scope_provider: Arc<dyn ScopeProvider>,  // 신규: current project_id 취득
}

pub trait ScopeProvider: Send + Sync {
    fn current_project_id(&self) -> Option<String>;
}

impl ActionSender {
    pub fn send(&self, action: Action) -> Result<(), SendError> {
        let origin = if action_is_mutation(&action) {
            self.scope_provider.current_project_id()
        } else {
            None
        };
        let dispatched = DispatchedAction { action, origin_project_id: origin };
        let envelope = VersionedEvent::new(dispatched, self.epoch_source.current());
        self.tx.send(envelope)
    }
}
```

**왜 이 설계인가 (Council 근거)**:
- **Codex**: "Stamp once at `ActionSender` boundary (centralized)" — 55 call site 수정 회피.
- **Gemini**: "Centralize stamping — `app_ctx.dispatch()` helper" — 동일 맥락.
- 기존 `ActionSender::send(action)` 호출 코드는 **변경 없음**. 내부 구현만 교체.

**Dependencies**:
- `Action` (기존), `VersionedEvent<T>` (기존 generic), `Epoch` (기존)
- 신규: `ScopeProvider` trait (Token/RbacGuard 어느 쪽을 1차 소스로 삼을지는 RED 단계 확정)

**Change footprint** (Codex 약속 "~5 파일"):
1. `src/action.rs` — `DispatchedAction` struct 추가
2. `src/context/action_channel.rs` — ActionSender 시그니처 + send 내부 stamp
3. `src/context/versioned.rs` — 변경 없음 (generic)
4. `src/worker.rs` — receive 쪽 `VersionedEvent<DispatchedAction>`로 payload 타입 교체 + 내부에서 `.action` 추출
5. `src/app.rs` — ActionSender 생성자에 ScopeProvider 주입 (1~2곳)
6. (테스트용) `src/app.rs` 등에서 `FakeActionSender` 패턴 업데이트

**Test strategy**:
- `DispatchedAction { action: mutation, origin: Some("A") }` 생성 시 대칭 스탬프 테스트
- `DispatchedAction { action: read_only, origin: None }` 규약 테스트
- ActionSender integration: scope=A에서 send → origin=A 스탬프 확인
- read-only(Fetch*)는 origin=None 확인

---

### C3. `CrossProjectGuard` [신규]
**Responsibility**: `origin_project_id`(또는 target ID의 project_id) vs 현재 active scope 비교의 **순수 함수**. worker / form validator / RBAC 에서 공통 호출.

**Location**: `src/infra/cross_project_guard.rs`

**Interface**:
```rust
pub enum GuardDecision {
    Allow,
    Block { reason: CrossProjectReason },
}

pub enum CrossProjectReason {
    /// Action origin scope != current active scope
    OriginScopeMismatch { origin: String, active: String },
    /// Form-selected resource's project_id != active scope
    FormSelectionMismatch { selected: String, active: String },
    /// Adapter response contained cross-project resource (invariant violation)
    AdapterFilterViolation { resource_id: String, project_id: String },
}

pub fn check_origin_scope(origin: &str, active: &str) -> GuardDecision { ... }
pub fn check_form_selection(selected_project_id: &str, active: &str) -> GuardDecision { ... }
```

**Dependencies**: 없음 (순수 함수 모듈). `CrossProjectReason`은 FR5 `reason` 필드의 rust-side 소스 enum. `reason` 문자열 매핑은 `CrossProjectBlockEvent::from(reason)`에서.

**Test strategy**: 각 케이스별 단위 테스트 (match/mismatch).

---

### C4. `RbacGuard::check_project_scope()` [확장]
**Responsibility**: 기존 `can_perform(ActionKind)` role-tier 검사에 project-scope 일치 검사를 AND 누적.

**Location**: `src/infra/rbac.rs`

**Interface**:
```rust
impl RbacGuard {
    // 신규 메서드 추가
    pub fn check_project_scope(&self, target_project_id: &str) -> GuardDecision {
        match &self.project_id {
            Some(active) if active == target_project_id => GuardDecision::Allow,
            Some(active) => GuardDecision::Block {
                reason: CrossProjectReason::OriginScopeMismatch {
                    origin: target_project_id.into(),
                    active: active.clone(),
                },
            },
            None => GuardDecision::Block { ... },  // scope 미설정 시 fail-safe: deny
        }
    }

    // 기존 can_perform 확장
    pub fn can_perform_in_scope(&self, action: ActionKind, target_project_id: &str) -> GuardDecision {
        let role_pass = self.can_perform(action);
        let scope_pass = matches!(self.check_project_scope(target_project_id), GuardDecision::Allow);
        match (role_pass, scope_pass) {
            (true, true) => GuardDecision::Allow,
            (false, true) => ... // role_tier,
            (true, false) => ... // project_scope,
            (false, false) => ... // both,
        }
    }
}
```

**Dependencies**: `CrossProjectGuard::CrossProjectReason` (C3), 기존 `self.project_id`, `ActionKind`.

**Test strategy**: role×scope 매트릭스 (admin/member/reader × match/mismatch). reason 정확히 구분 검증.

---

### C4-bis. `RbacGuard::update_roles_preserve_project()` [확장 — Council 필수]

**Responsibility**: Token refresh 이벤트가 `RbacGuard::update_roles(roles, None)`로 project_id를 덮어쓰지 않도록 방어. Codex가 발견한 위협: refresh가 roles만 갱신하는데 함께 project_id=None을 넣어버리면 FR3 scope check가 always-deny로 깨짐.

**Location**: `src/infra/rbac.rs`

**Interface (2가지 중 택1, RED에서 확정)**:
```rust
// (a) 신규 preserve API
impl RbacGuard {
    pub fn update_roles_preserve_project(&self, roles: Vec<TokenRole>) { ... }
    // 기존 update_roles는 "명시적으로 project_id 변경이 있는 경우"에만 사용
}

// (b) 시맨틱 개선: Option<Option<String>> — 명시적 "프로젝트는 변경 없음"
impl RbacGuard {
    pub fn update_roles(&self, roles: Vec<TokenRole>, project_id: ProjectIdUpdate) { ... }
}
enum ProjectIdUpdate { Keep, Set(Option<String>) }
```

**Dependencies**: 기존 `self.project_id: RwLock<Option<String>>` 내부 상태.

**Test strategy**: refresh 이벤트가 도는 mock 시나리오에서 `RbacGuard::project_id()`가 새 토큰의 project scope와 동일하게 유지되는지 assertion.

---

### C5. `WorkerMutationGuardHook` [확장]
**Responsibility**: `run_worker`의 Action dispatch 루프 진입 직후(epoch 매치 이후), `DispatchedAction`에서 mutation인 경우 `CrossProjectGuard::check_origin_scope` 호출. 실패 시 Action 실행 차단, `CrossProjectBlockEvent` emit, Toast 채널로 사용자 알림, `AppError::CrossProjectBlocked` 반환.

**Location**: `src/worker.rs` (run_worker 함수 내부, line 67 부근 epoch 체크 직후)

**Pseudo-flow** (Council 리뷰 반영 개정):
```rust
run_worker(mut rx: ActionReceiver, ctx):  // rx가 이제 VersionedEvent<DispatchedAction> 기반
    while let Some(envelope) = rx.recv().await:
        let (dispatched, epoch) = envelope.into_parts();
        if !ctx.epoch_matches(epoch) { continue; }  // 기존 stale drop

        let DispatchedAction { action, origin_project_id } = dispatched;
        let current_active = ctx.auth.get_token_info().await?.project.id;

        if let Some(origin) = origin_project_id {
            // mutation 경로
            match CrossProjectGuard::check_origin_scope(&origin, &current_active) {
                Allow => proceed_dispatch(action),
                Block { reason } => {
                    let ev = CrossProjectBlockEvent::new(reason, &action, &ctx);
                    ev.emit();  // tracing::warn! best-effort
                    toast_channel.send(CrossProjectToast::build_origin_mismatch(...)).await;
                    // AppError::CrossProjectBlocked 상위 반환 경로 (현재 worker는 err 채널이 없어 이벤트로 전파)
                    continue;
                }
            }
        } else {
            proceed_dispatch(action);  // read-only는 guard 우회
        }
```

**Dependencies**: `DispatchedAction` (C2), `CrossProjectGuard` (C3), `CrossProjectBlockEvent` (C9), `CrossProjectToast` (C10), `AuthProvider::get_token_info()`, `action_to_kind()` (C1, 이미 존재).

**주의** (Codex 지적): 현재 worker는 "epoch 기반 stale action drop"이 없음 (이벤트 드롭만 존재). 이 BL에서는 **origin-stamp가 스탈 케이스를 커버** (origin != active 시 차단). 전면 stale-action drop은 future BL (BL-P2-086 후보).

**Test strategy**: mock worker 루프에서 각 케이스:
- (origin=A, active=A) + mutation: 실행 허용
- (origin=A, active=B) + mutation: 차단 + 이벤트 + 토스트
- origin=None + read-only: 실행 허용
- Toast가 ApiError보다 먼저 emit되는 순서 검증 (Gemini 권고)

---

### C6. `NeutronListQueryBuilder` [확장 + pure fn 추출]
**Responsibility**: neutron list 엔드포인트 URL에 `tenant_id` 쿼리 파라미터 주입. URL 조립을 **pure function**으로 분리.

**Location**: `src/adapter/http/neutron.rs`

**Affected endpoints (Q1 매트릭스 partial)**:
| Endpoint | 현재 상태 | 변경 |
|----------|----------|------|
| `list_networks` | `_filter` 무시 | `tenant_id={scope}` 주입 |
| `list_security_groups` | `_filter` 무시 | `tenant_id={scope}` 주입 |
| `list_floating_ips` | `_filter` 무시 | `tenant_id={scope}` 주입 |
| `list_subnets` | filter minimal | `tenant_id={scope}` 주입 |
| `list_ports(device_id)` | device 범위 | 변경 없음 (device가 scope를 내포) |
| `list_network_agents` | 관리 API | **제외** (admin 전용 global) |

**Interface** (타입 교정 + response refilter 추가):
```rust
use crate::port::types::ProjectScope;  // 실제 타입 (Codex 교정)

// 신규 pure fn (요청측)
pub(crate) fn build_list_networks_query(scope: &ProjectScope, filter: &NetworkListFilter) -> Vec<(String, String)> { ... }
pub(crate) fn build_list_security_groups_query(scope: &ProjectScope, filter: &SecurityGroupListFilter) -> Vec<(String, String)> { ... }
pub(crate) fn build_list_floating_ips_query(scope: &ProjectScope, filter: &FloatingIpListFilter) -> Vec<(String, String)> { ... }
pub(crate) fn build_list_subnets_query(scope: &ProjectScope, network_id: Option<&str>) -> Vec<(String, String)> { ... }

// 신규 pure fn (응답측 defense-in-depth — Must-fix #6/7)
pub(crate) fn refilter_by_scope<T: HasTenantId>(
    items: Vec<T>,
    active_project_id: &str,
) -> (Vec<T>, usize /* dropped count */) { ... }

// 기존 impl 내부에서 호출
async fn list_security_groups(&self, filter, pag) -> Result<...> {
    let scope = self.auth.get_token_info().await?.project;
    let query = build_list_security_groups_query(&scope, filter);
    let raw = paginated_list(&self.base, "/v2.0/security-groups", &query, ...).await?;
    let (filtered, dropped) = refilter_by_scope(raw.items, &scope.id);
    if dropped > 0 {
        tracing::warn!(target: "cross-project-guard",
            "[adapter_filter] dropped {} unscoped items in list_security_groups", dropped);
        // 별도 CrossProjectBlockEvent(reason=adapter_filter)도 emit 가능
    }
    Ok(PaginatedResponse { items: filtered, next: raw.next })
}
```

**Dependencies**: `ProjectScope` (실제 타입, `src/port/types.rs`), `NetworkListFilter` 등 기존 filter struct. `AuthProvider`.

**Test strategy**:
- 각 pure fn에 대해 단위 테스트. assertion = "`query_pairs`에 `tenant_id={project_id}` 포함 + 기타 기존 filter 유지".
- **응답측**: `refilter_by_scope`에 cross-project 섞인 mock 데이터 주입 → 필터 후 active 리소스만 남는지 + dropped count == 기대값. (Codex Must-fix #6 충족)

---

### C7. `NovaCinderAllTenantsPolicy` [확장]
**Responsibility**: nova/cinder list 엔드포인트의 `all_tenants` 플래그를 **기본 false(scope-only)** 로 강제. 필요 시 상위 호출부만 명시 opt-in.

**Location**: `src/adapter/http/nova.rs`, `src/adapter/http/cinder.rs`

**Affected endpoints**:
| Endpoint | 현재 상태 | 변경 |
|----------|----------|------|
| `nova.list_servers` | scope via token | `all_tenants=false` 명시, opt-in 파라미터 추가 |
| `nova.list_flavors` | public | 변경 없음 (flavor는 cross-project shared) |
| `nova.list_server_events(id)` | server 범위 | 변경 없음 |
| `cinder.list_volumes` | scope via token | `all_tenants=false` 명시 |
| `cinder.list_snapshots` | scope via token | `all_tenants=false` 명시 |
| `cinder.list_qos_specs`, `list_storage_pools` | 관리 API | **제외** (admin global) |

**Interface**:
```rust
// 기존 signature 확장 — optional flag
async fn list_servers(&self, filter: &ServerListFilter, pag: &Pag, all_tenants: bool) -> Result<...>;
// 또는 Filter struct에 all_tenants 필드 추가 (deserialize로 default=false)
```

**Dependencies**: 기존 filter struct, `TokenScope`는 token 자체에서 자연스럽게 scope 제공 (명시 주입 불필요 for nova/cinder — 토큰이 project-scoped이므로 OpenStack이 알아서 scope-limit).

**Test strategy**: URL/query에 `all_tenants=true`가 들어가지 않는 default 케이스 + opt-in 시 들어가는 케이스 둘 다 단위 테스트.

---

### C8. `FormSelectedIdValidator` [신규]
**Responsibility**: 폼 제출 시점에 선택된 리소스 ID(예: VolumeId, SecurityGroupId)의 project_id를 검증. 불일치 시 제출 거부 + 토스트.

**Location**: `src/module/common/scope_validator.rs` (신규 common 모듈. 존재하지 않으면 생성)

**Interface**:
```rust
pub struct FormValidationError {
    pub field: String,
    pub reason: CrossProjectReason,
}

pub fn validate_form_scope<'a>(
    active: &str,
    selections: impl Iterator<Item = (&'a str, &'a str)>,  // (field_name, selected_project_id)
) -> Result<(), FormValidationError> { ... }
```

**Dependencies**: `CrossProjectGuard::check_form_selection` (C3).

**Caller 수정 대상**: cinder CreateSnapshot form, server create form (SG/Network/FlavorKey 선택), network create form (subnet 선택) 등. **실제 수정 대상 리스트는 TDD 단계에서 form 내부 dropdown 카탈로그를 훑어 열거**.

**Glance Mutation 경로 특수 처리 (Must-fix #3)**: FR1이 Glance를 제외하므로, Glance `DeleteImage` / `UpdateImage`는 origin-stamp만으로는 cross-target 공격(admin이 A scope에서 B의 image ID를 직접 지정해 삭제) 차단 불가. 구체 처리:
- `DeleteImage { id }` / `UpdateImage { id, .. }` 경로에서 **adapter pre-mutation GET으로 image `owner` 필드 취득** → active_scope와 비교 후 mismatch면 reject.
- Glance `owner`는 project_id에 해당. 기존 `glance.rs`의 `get_image`를 활용.
- 이 1개 adapter GET은 FR2의 일반 규칙(RTT 추가 회피)에 예외로 허용 — Glance의 scope 모델 특성상.
- Image module에서 form submit 전에 이 pre-check를 호출.

**Test strategy**: 단일/복수 field 불일치 케이스. "선택 ID가 disambiguated option(`(proj: xxx)` 접미사)에서 온 cross-project인지" 시뮬레이션.

---

### C9. `CrossProjectBlockEvent` [신규 — Council 개정]
**Responsibility**: FR5 스키마 structured event. fingerprint canonicalization + 스키마 필드 보강, tracing emit.

**Location**: `src/infra/audit/cross_project_block.rs` (신규 `audit` 서브모듈)

**Interface** (Council must-fix/should-consider 반영):
```rust
pub struct CrossProjectBlockEvent {
    // 기본 식별
    pub timestamp: DateTime<Utc>,
    pub actor_user_id: String,          // **Keystone UUID (username 금지, Gemini 권고)**
    pub actor_cloud: String,

    // 프로젝트 scope 정보 (Codex 권고: target field 의미 명확화)
    pub active_project_id: String,
    pub asserted_origin_project_id: Option<String>,  // FR2: action stamp 시점 scope. Option: form selection 시에는 None
    pub target_project_id: Option<String>,            // 실제 target이 확인된 경우만 (FR4 form, FR1 adapter_filter)

    // Action / resource
    pub action_type: String,   // 예: "DeleteServer"
    pub resource_kind: String, // 예: "server"
    pub resource_id: Option<String>,

    // 결정
    pub outcome: &'static str, // "blocked"
    pub reason: String,        // "origin_scope_mismatch" / "form_selection_mismatch" / "adapter_filter" / "role_tier" / "both"
    pub guard_layer: &'static str,  // **신규 (Codex should-consider)**: "fr2_worker" / "fr3_rbac" / "fr4_form" / "fr1_adapter"
    pub correlation_id: u64,         // **신규 (Codex should-consider)**: Epoch 값 — 다중 이벤트 연관 추적

    // 무결성
    pub fingerprint: String,   // v1 canonical sha256 앞 12자 (아래 규약)
}

impl CrossProjectBlockEvent {
    pub fn new(
        reason: CrossProjectReason,
        guard_layer: GuardLayer,
        action: &Action,
        context: &TokenInfo,
        epoch: Epoch,
    ) -> Self { ... }

    pub fn emit(&self);  // tracing::warn! with target "cross-project-guard"
}
```

**Fingerprint canonicalization (Must-fix #5, Codex+Gemini 공통)**:
```
input = "v1|"
      + actor_user_id + "|"
      + active_project_id + "|"
      + asserted_origin_project_id.unwrap_or("") + "|"
      + target_project_id.unwrap_or("") + "|"
      + action_type + "|"
      + resource_id.unwrap_or("")
fingerprint = sha256(input.as_bytes()).hex()[..12]
```
- `v1|` 버전 prefix로 스키마 진화 시 구분 가능.
- `|` 구분자로 boundary ambiguity 방지 (e.g., "a" + "bc" vs "ab" + "c" 충돌 회피).
- `None` → 빈 문자열 명시 규칙.

**Dependencies**: `CrossProjectReason` (C3), `GuardLayer` (C3 enum 확장), `Action` (for `action_type`/`resource_kind`/`resource_id` 추출), `Token` (for active project/actor), `Epoch` (from `src/context/epoch.rs`).

**Best-effort 쓰기 (NFR1 정합)**: tracing이 로그 IO 실패해도 차단 자체는 이미 완료 상태. 이벤트 기록과 차단 행위는 독립 경로.

**Test strategy**:
- 스키마 필드 모두 채워지는지
- fingerprint 재현성(동일 input → 동일 fingerprint) + 경계 충돌 회피 검증 (e.g., `user="ab"` + `proj=""` vs `user="a"` + `proj="b"` → 다른 fingerprint)
- `reason` 매핑(enum → string) 정확성
- `guard_layer` 네 값 모두 생성 가능한지 (각 FR 경로별)
- `correlation_id` = 현재 epoch 값으로 stamp되는지

---

### C10. `CrossProjectToast` [신규]
**Responsibility**: 차단 시 공통 한국어 카피 생성. `active_project_name` + `target_project_name`/`origin_project_name` 포함. 복붙 방지 위해 단일 모듈. Toast level = **Warning** 확정 (Gemini 권고 — 시스템 에러 vs 사용자 실수 구분).

**Location**: `src/ui/cross_project_toast.rs`

**Interface**:
```rust
pub fn build_origin_mismatch_toast(active_name: &str, origin_name: &str) -> Toast { ... }
pub fn build_form_mismatch_toast(active_name: &str, selected_name: &str, field: &str) -> Toast { ... }
pub fn build_glance_owner_mismatch_toast(active_name: &str, target_owner_name: &str) -> Toast { ... }  // Glance 특수
```

**Toast 전달 동기성 (Gemini 권고)**: `CrossProjectToast`는 Action reject **직전** emit되어야 하며, 이후 발생할 수 있는 `ApiError` Toast보다 먼저 사용자에게 표시됨. Worker hook에서 toast_channel.send() → dispatch skip 순서 강제.

**Dependencies**: 기존 `Toast` struct, `ToastLevel::Warning`.

**Copy 예시**:
```
"차단: '{active_name}' 프로젝트에서 '{origin_name}' 프로젝트의 리소스는 수정할 수 없습니다.
 :switch-project {origin_name} 후 재시도하세요."
```

**Test strategy**: 포함 단어/치환자 + safe_display 규약(60자 truncate, 제어문자 제거) 준수.

---

## 의존성 다이어그램 (ASCII — Council 개정)

```
   [UI form / command / button]
            |
            | channel.send(Action)  (55 call sites UNCHANGED)
            v
   +------------------------+
   | ActionSender C2        |<----- ScopeProvider (TokenInfo.project.id)
   | (src/context/          |
   |  action_channel.rs)    |
   +------------------------+
            |
            | stamp: DispatchedAction { action, origin_project_id }
            | wrap:  VersionedEvent<DispatchedAction>
            v
   +------------------------+
   | mpsc channel           |
   +------------------------+
            |
            v
   +------------------------+
   | run_worker (C5 hook)   |-----> action_to_kind(C1): Option<ActionKind>
   | (src/worker.rs:53)     |           |
   +------------------------+           | if Some(_) → mutation path
            |                            v
            |                  +------------------------+
            |                  | CrossProjectGuard C3   |
            |                  | check_origin_scope()   |
            |                  +------------------------+
            |                            |
            |                   Allow    |    Block(reason)
            |                     |      |      |
            |      +--------------+      |      +----------------+
            |      |                     |                       |
            v      v                     v                       v
     proceed_dispatch(action)     +----------------+   +-----------------+
                                  | CrossProject-  |   | CrossProject-   |
                                  | BlockEvent C9  |   | Toast C10       |
                                  | (tracing)      |   | (Warning)       |
                                  +----------------+   +-----------------+
                                                              |
                                                              v
                                                      [Toast channel]
                                                      (ApiError보다 먼저)

  [Read 경로 — FR1]                           [Form submit 경로 — FR4]
        |                                              |
        v                                              v
  +-------------------------+              +-------------------------+
  | NeutronListQueryBuilder |              | FormSelectedIdValidator |
  | + NovaCinder all_tenants|              | C8                      |
  | C6/C7 (pure fn + refilt)|              +-------------------------+
  +-------------------------+                          |
        |                                              v
        |                                     CrossProjectGuard C3
        | dropped > 0 시:                     (check_form_selection)
        v                                              |
  [tracing target="cross-project-guard"                v
   adapter_filter event]                       Glance mutation 경로:
                                               adapter GET image.owner
                                               → pre-mutation check

  [Token refresh event — FR3 방어]
        |
        v
  +---------------------------+
  | RbacGuard                 |
  | update_roles_preserve_*() |   <-- project_id None-overwrite 방지
  +---------------------------+
```

## Q1 확정: Adapter Endpoint 매트릭스

| Service | Endpoint | 현재 | 변경 방침 | 근거 |
|---------|----------|------|----------|------|
| Neutron | list_networks | `_filter` 무시 | `tenant_id` 주입 | SG와 동일 leak 표면 |
| Neutron | list_security_groups | `_filter` 무시 | `tenant_id` 주입 | 4겹 버그 원인 |
| Neutron | list_floating_ips | `_filter` 무시 | `tenant_id` 주입 | 동일 |
| Neutron | list_subnets | minimal filter | `tenant_id` 주입 | 네트워크 계열 일관성 |
| Neutron | list_ports | device_id 범위 | 변경 없음 | device_id가 scope 내포 |
| Neutron | list_network_agents | admin API | 제외 | admin-only global listing |
| Nova | list_servers | token scope | `all_tenants=false` 명시 | 현재도 사실상 scope-only, 문서화 |
| Nova | list_flavors | public | 변경 없음 | flavor는 tenant-agnostic shared |
| Nova | list_server_events(id) | server 범위 | 변경 없음 | id가 scope 내포 |
| Nova | list_server_migrations(id) | server 범위 | 변경 없음 | 동일 |
| Nova | list_aggregates / compute_services / hypervisors | admin API | 제외 | admin-only global |
| Cinder | list_volumes | token scope | `all_tenants=false` 명시 | scope-only 재확인 |
| Cinder | list_snapshots | token scope | `all_tenants=false` 명시 | 동일 |
| Cinder | list_qos_specs / storage_pools | admin API | 제외 | admin-only global |
| Glance | list_images | visibility mixed | **이번 범위 제외** | public/shared 이미지 정합 처리가 복잡, 후속 BL |
| Keystone | list_projects/users/roles/domains | admin API | 제외 | 명시적 cross-project 관리 API |

**"제외" 처리된 endpoint는 FR2(worker origin-scope guard) + FR3(RBAC role-tier)로 보호**. 읽기 레벨에서 cross-project 데이터가 보일 수는 있지만, mutation은 여전히 차단됨. Glance는 별도 BL로 분리(visibility 모델이 별개 이슈).

## Q3 확정: Worker 거부 에러 variant

**결정**: 기존 `AppError`(`#[non_exhaustive]`)에 `CrossProjectBlocked` variant 추가.

```rust
#[derive(Error, Debug)]
#[non_exhaustive]
pub enum AppError {
    // ... 기존 variants ...

    #[error("Cross-project action blocked: origin '{origin}' != active '{active}' (reason: {reason:?})")]
    CrossProjectBlocked {
        origin: String,
        active: String,
        reason: CrossProjectReason,
    },
}
```

`CrossProjectReason`은 `CrossProjectGuard` 모듈에서 정의하고 `src/error.rs`에서 re-export. 순환 의존 회피.

## Assumption 재확정 (DETAIL 기반)

| Assumption | 상태 | Notes |
|-----------|------|------|
| A1 active_scope 전파 | ✅ 확정 | 실제 경로: `AuthProvider::get_token_info().project.id` (ProjectScope{id, name, domain}). `RbacGuard::project_id()`는 cache된 copy. Worker는 전자를 1차 소스로. |
| A2 mock HTTP 인프라 | ✅ 회피 확정 | pure fn URL/query 빌더 추출. C6/C7에서 구체화. |
| A3 `build_disambiguated_opts` 순수성 | ✅ 확정 | 이번 BL에선 확장 없이 유지. |

## DETAIL 단계에서 남은 결정(TDD RED에서 확정)

- `FormSelectedIdValidator` 정확한 위치 (`src/module/common/` 존재 여부에 따라) — 없으면 `src/ui/form_scope_validator.rs`
- **Toast level = Warning (확정, Gemini 권고 반영)**
- `DispatchedAction.origin_project_id`: read-only는 `None`으로 확정 (mutation만 `Some(scope)`). 가드 skip 시맨틱이 None 존재로 명시화.
- `ScopeProvider` trait의 1차 소스: `Token.project.id` (AuthProvider) vs `RbacGuard.project_id()` cache. **권고**: primary = Token, fallback = RbacGuard cache. RED에서 검증.
- C4-bis `update_roles_preserve_project()` interface 선택 (preserve API vs Option<Option<String>>) — RED에서 확정

## Council Review Response (Must-fix → 본 문서 반영 매핑)

| Must-fix | 반영 위치 | 상태 |
|---------|-----------|------|
| #1 StampedAction 폐기 → ActionSender 중앙 스탬핑 | C2 재설계, 다이어그램 개정 | ✅ |
| #2 Type ref 교정 (TokenScope/ScopedAuthSession → Token.project.id/RbacGuard.project_id) | Assumption Verification 표, C6 interface | ✅ |
| #3 Glance DeleteImage/UpdateImage scope 검증 | C8 Glance 특수 처리 섹션 | ✅ |
| #4 Glance = FR4 adapter pre-mutation GET | C8 Glance pre-check | ✅ |
| #5 Fingerprint canonicalization (v1 prefix + `\|` delimiter + None 규칙) | C9 Interface + Fingerprint 규약 | ✅ |
| #6 Token refresh project_id 보존 | C4-bis 신규 | ✅ |
| #7 FR1 response-side defense-in-depth (client-side re-filter) | C6 Interface (`refilter_by_scope`) | ✅ |
| #8 is_mutation / action_to_kind parity | C1 보강 (기존 action_to_kind 재사용 + parity 테스트) | ✅ |

| Should-consider | 반영 위치 | 상태 |
|-----------------|-----------|------|
| Schema `guard_layer` + `correlation_id` 추가 | C9 Interface 확장 | ✅ |
| `target_project_id` rename → asserted_origin/target 분리 | C9 Interface 개정 | ✅ |
| Toast level = Warning 확정 | C10 | ✅ |
| user_id = Keystone UUID | C9 field 주석 | ✅ |
| Toast 동기성 (ApiError 앞) | C10 설명 추가, C5 pseudo-flow 순서 명시 | ✅ |
| Background task Action emit audit | C5 Test strategy 주석 (TDD 단계 audit) | 🟡 (구현 시 확인) |
| End-to-end mock integration 1개 merge-blocking | NFR2 — 다음 섹션 보강 필요 | 🟡 (code-generation에서 추가) |

Future-BL 5건은 backlog에 따로 기록 (시스템-summary Next Steps 참조).
