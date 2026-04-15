# Claude Review — application-design.md DETAIL

**Reviewer**: Claude (self-critique)
**Mode**: DETAIL section critique (interface signatures, dependency direction, atomicity)

## Top 3 Critical Issues

### 1. SwitchStateMachine 시그니처에 무효한 Rust 에러 타입 + 동시 변경 가능성 미정의
- **What**: `try_begin -> Result<Epoch, SwitchError::InProgress>` — Rust는 enum variant를 에러 타입으로 못 박을 수 없음. `Result<Epoch, SwitchError>` 여야 함. 또한 `&mut self`인데 `ContextSwitcher`는 `Arc`로 보유될 수밖에 없음(다중 호출자). 내부 lock이 명시되지 않음.
- **Why**: 컴파일 안 됨. 그리고 동시 switch 호출 시 `Mutex<SwitchState>` 등으로 상호배제 필수.
- **Fix**:
  ```rust
  pub struct SwitchStateMachine {
      state: Mutex<SwitchState>,        // or parking_lot::Mutex
      history: Arc<Mutex<ContextHistoryStore>>,
  }
  impl SwitchStateMachine {
      pub fn try_begin(&self, target: ContextTarget) -> Result<Epoch, SwitchError> { ... }
  }
  ```

### 2. ContextSwitcher.switch의 내부 절차 순서가 epoch race를 만든다
- **What**: 명시된 순서: `state.begin → registry.cancel_below(new_epoch) → session.begin → rescope → invalidate → token_cache.store → state.commit`. 그러나 `state.begin`이 epoch를 bump한다고 가정해도, cancel_below는 **새 epoch 미만의 모든 작업** cancel — 즉 새 epoch 자체의 token도 등록되기 전에 cancel 필터를 통과한다. OK. 하지만 worker spawn이 register한 토큰을 begin과 동시에 다른 thread가 spawn 중이라면, cancel이 새 spawn을 놓칠 수 있음.
- **Why**: 동시성 race — switch 시작 직후 들어온 신규 spawn이 옛 epoch로 등록되면 cancel 우회.
- **Fix**: spawn API를 `(epoch, cancel) = registry.register_with_current()`로 강제 — registry 내부에서 `current_epoch == registry.frozen_epoch?` 검사. 또는 switch 진입 시 spawn lock 잠시 차단. 또는 `cancel_below(new_epoch)`를 `state.commit` 직전에 1회 더 호출(idempotent).

### 3. ContextTarget vs TokenScope 타입 중복 + Resolver의 ProjectRef 미해결 상태
- **What**: `ContextTarget { cloud, project: ProjectRef, domain }`, `TokenScope::Project { name, domain }`. Resolver는 ProjectRef를 받아 ContextTarget을 반환해야 하는데, ContextTarget 내부에 ProjectRef가 또 들어있음. "resolved target"과 "user input"이 같은 타입.
- **Why**: switch 절차 곳곳에서 "이건 resolved인가?" 판단 필요 → 버그 양산.
- **Fix**:
  ```rust
  pub enum ContextRequest {            // user input (parser output)
      ByName { cloud: Option<String>, project: String, domain: Option<String> },
      ById   { cloud: Option<String>, project_id: String },
  }
  pub struct ContextTarget {           // resolved (post-resolver)
      pub cloud: String,
      pub project_id: String,
      pub project_name: String,
      pub domain: String,
  }
  ```
  Resolver: `ContextRequest → ContextTarget`. TokenScope는 `From<&ContextTarget>`로 변환.

## 추가 시그니처 교정

### Worker spawn API
DETAIL 표현 `pub fn spawn(epoch, cancel, fut: impl Future<Output = AppEvent>) -> JoinHandle<()>` — 여러 polling 사이트가 다른 반환 타입을 가짐. 일반화 필요:
```rust
pub fn spawn_versioned<F, T>(
    registry: &CancellationRegistry,
    epoch: Epoch,
    fut: F,
) -> JoinHandle<()>
where
    F: Future<Output = T> + Send + 'static,
    T: Into<AppEvent> + Send + 'static,
```
혹은 `app_tx`도 인자로 받아 내부에서 `VersionedEvent::new(ev, epoch)` 송신.

### KeystoneCapabilities 미정의
DETAIL에 언급만 됨. 정의 필요:
```rust
#[derive(Debug, Clone)]
pub struct KeystoneCapabilities {
    pub allow_rescope_scoped_token: bool,
    pub auth_method: AuthMethod,        // Password | Token | AppCredential
    pub api_version: KeystoneVersion,
}
```
초기 호출 시 `/v3` discovery 또는 첫 토큰 응답에서 추론.

### EndpointCatalogInvalidator의 AdapterRegistry 의존
`AdapterRegistry`가 가변 상태인지 확인 필요. Invalidate가 모든 adapter의 endpoint cache를 비우려면 registry가 `Vec<Arc<dyn HttpClient>>` 같은 형태로 보유하고, 각 client에 `invalidate_endpoints()` trait method가 필요. trait이 LIST에 없음.
**Fix**: `pub trait HttpEndpointCache { fn invalidate(&self); }` 추가 후 모든 `BaseHttpClient`가 구현. Invalidator는 registry 순회.

## 미명시 사항 (RED phase 진입 전 결정 필수)

1. **ScopedAuthSession이 AuthProvider를 owns/wraps/replaces?** — 현재는 "uses"라고 표기. 실 코드에서 KeystoneAuthAdapter가 active_scope 필드를 가짐. ScopedAuthSession이 KeystoneAuthAdapter를 mutex로 감싸 active_scope를 교체하는 형태인지 명시 필요.

2. **ContextChanged variant를 받는 Component 화이트리스트** — 어떤 모듈이 데이터를 비워야 하는가? 후보: `ResourceListView`, `DetailView`, 모든 `<Resource>Module` (Server, Volume, Network 등). LIST 어딘가에 명시 필요. 그렇지 않으면 어떤 모듈은 옛 데이터를 그대로 들고 있음.

3. **ContextIndicator 강조 타이머 구동** — render loop가 60fps 폴링이면 `Instant::now()` 비교만으로 OK. 하지만 idle 시 redraw 안 되는 구조라면 `AppEvent::Tick` 또는 timer task 필요. App의 render 정책 확인.

4. **Switching 중 들어온 두 번째 switch 요청 처리** — `try_begin`이 `InProgress` 반환 → CommandParser는 사용자에게 "전환 중" 에러 표시? 또는 큐잉? 정책 미정.

5. **Cancel during Switching** — 사용자가 `Esc` 또는 다른 명령으로 진행 중 switch 취소? state machine에 `Cancelled` transition 부재.

6. **App의 mut/lock 정책** — `App.switch_context(&mut self, target)` 표기인데, 현재 코드의 App은 single-threaded loop 안에서 mutate. async switch가 진행 중일 때 다른 keypress 처리는? mainloop가 await 점에서 yield하면 OK. 명시 필요.

## Test seams

- **MockContextSession** — `with_rescope_failure`, `with_invalidate_failure`까지는 좋음. 추가 필요: `with_partial_commit_failure` (rescope OK + invalidate OK + token store fail), `with_slow_rescope` (timeout 시뮬레이션).
- **Worker epoch 검증** — 단위 테스트가 어디 들어가는지 미명시. `worker.rs::tests` 안에 stale event drop 케이스 추가 필요.
- **Resolver 충돌** — `Ambiguous` 케이스 테스트 데이터 (cloud A/B에 같은 이름 admin 프로젝트) 픽스처 필요.

## Verdict

**APPROVE-WITH-CHANGES**

이유: 컴포넌트 분해와 의존 그래프는 정합. 그러나 (1) `Result<Epoch, SwitchError::InProgress>` 같은 컴파일 불가 시그니처, (2) ContextTarget vs TokenScope 타입 중복, (3) atomicity 경계의 동시성 race 처리 미명시 — 이 셋이 RED phase 진입 전 보강 필수. HttpEndpointCache trait 부재도 빠진 인터페이스로 명시.
