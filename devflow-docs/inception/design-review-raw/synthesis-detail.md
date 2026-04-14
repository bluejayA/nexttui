# Council Synthesis — application-design.md DETAIL

**Chair**: Claude
**Reviewers**: Codex (REJECT), Gemini (APPROVE-WITH-CHANGES), Claude (APPROVE-WITH-CHANGES)
**Verdict (synthesized)**: **APPROVE-WITH-CHANGES (DETAIL r2 개정 후 INCEPTION 완료)**

Codex의 REJECT는 "atomic 계약과 port 경계의 보강 없이는 RED 진입 위험"이라는 정확한 지적. 종합 개정안에 핵심 사항을 모두 반영하면 APPROVE 수준으로 충분히 끌어올릴 수 있다.

---

## 1. 합의 사항 (3-AI 일치)

### A. atomic 계약을 ContextSessionPort에 완전히 위임 (Codex+Gemini 합의)

**현재 문제**: ContextSwitcher가 절차의 중간에서 `rescope_adapter.rescope`, `catalog_invalidator.invalidate_all`, `token_cache.store`를 직접 호출. begin/commit 사이에 외부 호출이 끼어들어 atomicity 보장 불가.

**합의 결정**:
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
ContextSwitcher는 state/epoch/cancel만 오케스트레이트. rescope·invalidate·token store는 모두 port 구현 내부.

### B. SessionHandle은 이전 token + scope를 캡처 (Gemini+Claude)

```rust
pub struct SessionHandle {
    pub epoch: Epoch,
    pub previous_token: Token,
    pub previous_scope: TokenScope,
    pub target: ContextTarget,
    // (impl 내부) staged_new_token, staged_new_catalog
}
```
rollback 시 ScopedAuthSession이 KeystoneAuthAdapter의 active_scope/token을 이전 값으로 강제 복원.

### C. 단일 epoch 게이트를 App/event-loop 디스패처에 명시 (Codex+Gemini)

- App이 `current_epoch: AtomicU64` 보유 (단일 권위)
- 모든 `VersionedEvent<AppEvent>` 수신 시 `event.epoch < current_epoch` → drop
- Worker 다중 폴링 사이트는 모두 `VersionedEvent<AppEvent>` 송신으로 통일
- event_loop의 blind forward 제거

### D. Worker 시그니처 통일 (Gemini+Claude)

```rust
pub async fn run_worker(
    // ... existing ...
    mut action_rx: mpsc::UnboundedReceiver<VersionedEvent<Action>>,
    event_tx: mpsc::UnboundedSender<VersionedEvent<AppEvent>>,
)
```
모든 spawn은 `(epoch, CancellationToken)` 페어를 받고, 송신 시 `VersionedEvent::new(ev, epoch)` 강제.

### E. SwitchError::InProgress를 에러 타입으로 못 박지 않음 (Codex+Claude)

```rust
#[derive(Debug, thiserror::Error)]
pub enum SwitchError {
    #[error("switch already in progress")]    InProgress,
    #[error("rescope rejected: {0}")]          RescopeRejected(String),
    #[error("catalog invalidation failed")]    CatalogFailed(String),
    #[error("ambiguous target")]               Ambiguous { candidates: Vec<ContextTarget> },
    #[error("target not found: {0}")]          NotFound(String),
    #[error(transparent)]                      Api(#[from] ApiError),
    #[error(transparent)]                      Io(#[from] std::io::Error),
}

pub fn try_begin(&self, target: ContextTarget) -> Result<Epoch, SwitchError>; // 변수 SwitchError
```

### F. ContextIndicator 패시브 타이머 (Gemini+Claude)

```rust
pub struct ContextIndicator {
    snapshot: Option<ContextSnapshot>,
    last_switch_at: Option<Instant>,
    highlight_duration: Duration,
}
impl Component for ContextIndicator {
    fn render(&self, frame, area) {
        let highlighting = self.last_switch_at
            .map_or(false, |t| t.elapsed() < self.highlight_duration);
        // ...
    }
}
```
`set_context(snapshot, mark_highlight: bool)` — mark_highlight=true 시 last_switch_at = Instant::now().

### G. ContextSnapshot에 epoch + token 포함 (Gemini)

```rust
pub struct ContextSnapshot {
    pub target: ContextTarget,
    pub epoch: Epoch,
    pub token: Token,                           // self-verifying
    pub token_scope: TokenScope,
    pub captured_at: chrono::DateTime<chrono::Utc>,
}
```

### H. AppEvent::ContextChanged에서 epoch 제거 — VersionedEvent envelope이 이미 보유 (Codex)

```rust
pub enum AppEvent {
    ContextChanged { target: ContextTarget },
    // ...
}
```

---

## 2. Codex 단독 — 채택

### ScopedAuthPort 신설 (port 경계 보강)

`AuthProvider`는 read/request-auth 지향으로 유지. scope 변경 전용 port 분리:
```rust
#[async_trait]
pub trait ScopedAuthPort: Send + Sync {
    fn current_scope(&self) -> TokenScope;
    fn current_token(&self) -> Token;
    async fn set_active_scope(&self, scope: TokenScope, token: Token) -> Result<(), SwitchError>;
    async fn upsert_scoped_token(&self, scope: TokenScope, token: Token) -> Result<(), SwitchError>;
}
```
KeystoneAuthAdapter가 양쪽 trait 모두 구현. ScopedAuthSession은 ScopedAuthPort를 통해 active_scope를 mutate.

### 미명시 정책 명문화

- **Switching + (SwitchContext|SwitchBack|Cancel) 정책**: Switching 중 신규 switch 요청 → 즉시 `SwitchError::InProgress` 반환. Cancel 명령 → SwitchStateMachine에 `cancel()` 추가, port 핸들이 있으면 rollback 호출.
- **ContextChanged 핸들링 컨트랙트**: 각 모듈 컴포넌트(Server/Volume/Network/.../Project Module)는 `handle_event(AppEvent::ContextChanged)`에서 내부 `Vec<T>` 비우기 + `is_loading=true`. Sidebar/StatusBar 등 컨텍스트 무관 컴포넌트는 default no-op.
- **TokenCacheStore 키 정렬**: `store_scoped(&self, scope: &TokenScope, token: &Token)` — 기존 모델 일치, ContextTarget 직접 키 사용 금지.
- **list_user_projects를 async로**: `pub async fn list_user_projects(&self) -> Result<Vec<ContextTarget>, SwitchError>` (Keystone API 호출).

### Mock 시즘 확장

`MockContextSession`에 추가:
- `with_begin_failure`, `with_rescope_failure`, `with_refresh_failure`, `with_commit_failure`
- `rollback_called() -> bool`, `transition_steps() -> Vec<&'static str>` (순서 검증)

---

## 3. Claude 단독 — 채택

### HttpEndpointCache trait 추가

EndpointCatalogInvalidator가 모든 HTTP client의 endpoint cache를 일괄 무효화하려면 공통 trait 필요:
```rust
pub trait HttpEndpointCache {
    fn invalidate(&self);
}
```
모든 `BaseHttpClient`가 구현. AdapterRegistry가 `Vec<Arc<dyn HttpEndpointCache>>` 보유.

### KeystoneCapabilities 정의

```rust
#[derive(Debug, Clone)]
pub struct KeystoneCapabilities {
    pub allow_rescope_scoped_token: bool,
    pub auth_method: AuthMethod,        // Password | Token | AppCredential
    pub api_version: KeystoneVersion,
}
pub enum AuthMethod { Password, Token, AppCredential }
```
첫 토큰 응답 또는 `/v3` discovery에서 추론.

### Test seam 추가

- `MockContextSession::with_partial_commit_failure` (rescope OK + invalidate OK + commit fail)
- `MockContextSession::with_slow_rescope(Duration)` (timeout 시뮬레이션)
- worker.rs::tests에 stale event drop unit test (epoch 이전 이벤트가 drop되는지)
- Resolver Ambiguous 픽스처 (cloud A/B에 같은 이름 admin)

---

## 4. Switcher 절차 개정 (모두 통합)

```rust
pub async fn switch(&self, target: ContextTarget) -> Result<(Epoch, ContextSnapshot), SwitchError> {
    // 1. epoch bump + state Switching
    let new_epoch = self.state.try_begin(target.clone())?;

    // 2. 이전 epoch의 모든 작업 cancel (idempotent, 두 번 호출 안전)
    self.cancellation.cancel_below(new_epoch);

    // 3. session.begin (handle에 previous_token/scope 캡처)
    let mut handle = match self.session.begin(&target, new_epoch).await {
        Ok(h) => h,
        Err(e) => { self.state.fail(e.clone()); return Err(e); }
    };

    // 4. atomic transition (모두 port 내부)
    if let Err(e) = self.session.rescope(&mut handle).await
        .and_then(|_| self.session.refresh_catalog(&mut handle).await) {
        let _ = self.session.rollback(handle).await;
        self.state.fail(e.clone());
        return Err(e);
    }

    // 5. commit (port 내부에서 ScopedAuthPort.set_active_scope + token store)
    let snapshot = match self.session.commit(handle).await {
        Ok(s) => s,
        Err(e) => { self.state.fail(e.clone()); return Err(e); }
    };

    // 6. state.commit + history push
    self.state.commit(snapshot.clone());
    self.history.push(snapshot.clone());

    // 7. (마지막 안전망) 다시 한 번 cancel
    self.cancellation.cancel_below(new_epoch);

    Ok((new_epoch, snapshot))
}
```

Switcher는 `state + cancellation + session` 3개에만 의존. rescope/invalidate/token store는 모두 session 내부로 이동.

## 5. 컴포넌트 의존 그래프 (개정)

```
                      App (Controller)
                       |
                       | owns
                       v
                  ContextSwitcher (Service)
                  /    |    \
                 v     v     v
            State  Cancel  ContextSessionPort (trait)
                              |
                              | impl
                              v
                       ScopedAuthSession (Service)
                       /     |       |        \
                      v      v       v         v
              KeystoneRescope  ScopedAuth  EndpointCatalog  TokenCacheStore
                Adapter          Port       Invalidator       (Repository)
                                  |               |
                                  | impl          | reads
                                  v               v
                          KeystoneAuthAdapter   AdapterRegistry
                                                     |
                                                     v
                                               HttpEndpointCache trait
```

ContextSwitcher의 협력자가 8개 → 3개로 축소. atomic 책임이 단일 port에 집중.

---

## 6. DETAIL r2 개정 체크리스트

- [ ] ContextSessionPort에 `Handle` 연관타입 + rescope/refresh_catalog/commit/rollback 메서드 도입
- [ ] ContextSwitcher.switch 절차 개정 (위 4번 코드)
- [ ] SwitchError 재정의 (transparent ApiError/IoError 포함)
- [ ] SessionHandle 정의 (previous_token, previous_scope 포함)
- [ ] ContextSnapshot에 epoch + token 추가
- [ ] AppEvent::ContextChanged에서 epoch 제거 (envelope이 보유)
- [ ] ScopedAuthPort 신설 + KeystoneAuthAdapter가 구현
- [ ] HttpEndpointCache trait 신설
- [ ] KeystoneCapabilities 정의 명시
- [ ] App에 `current_epoch: AtomicU64` + dispatcher epoch gate 명시
- [ ] Worker 시그니처: `VersionedEvent<Action>` rx + `VersionedEvent<AppEvent>` tx
- [ ] ContextIndicator 패시브 타이머 (last_switch_at, render에서 check)
- [ ] ContextChanged 핸들링 컨트랙트: 모듈 컴포넌트 white-list 추가
- [ ] TokenCacheStore: `store_scoped(scope, token)` 시그니처 (TokenScope 키)
- [ ] ContextTargetResolver.list_user_projects를 async로
- [ ] Switching 정책: 신규 switch → InProgress, Cancel → state.cancel + session.rollback
- [ ] MockContextSession 시즘 확장 (with_*_failure, transition_steps, rollback_called)

---

## 최종 Verdict

**APPROVE-WITH-CHANGES** — 위 17개 체크리스트 적용 후 INCEPTION 완료로 진입.

핵심 변경 1줄 요약: **atomic 책임을 ContextSessionPort 내부로 완전 통합 + 단일 epoch 게이트 디스패처에 명시 + ScopedAuthPort 분리**.
