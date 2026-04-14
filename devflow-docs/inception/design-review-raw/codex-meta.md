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
session id: 019d85cc-3548-7e11-a47b-033bc13139d0
--------
user
META-REVIEW — synthesis of prior council review.

PROJECT: nexttui — Rust + ratatui TUI for OpenStack admins.
FEATURE: BL-P2-031 — Runtime cloud/project context switching via Keystone rescoping.

REVIEW THIS FILE: devflow-docs/inception/design-review-raw/synthesis-detail.md
ALSO CONSIDER:
- devflow-docs/inception/application-design.md (current DETAIL design under review)
- devflow-docs/inception/requirements.md (10 FR + 5 NFR)
- devflow-docs/inception/design-review-raw/codex-detail.md (your prior review if Codex)
- devflow-docs/inception/design-review-raw/gemini-detail.md (prior Gemini review)
- devflow-docs/inception/design-review-raw/claude-detail.md (prior Claude review)

CONTEXT:
The synthesis was produced by Claude (chair) consolidating three independent reviews of the DETAIL design. Now we need a sanity check on the synthesis itself before applying it as DETAIL r2.

YOUR JOB — judge whether the synthesis:

1. **Correctly captures all critical issues** raised by the three reviewers, or did it drop/dilute important findings?
2. **Resolutions are technically sound** — particularly the new ContextSessionPort surface (begin / rescope / refresh_catalog / commit / rollback). Is this actually atomic? Does the proposed Switcher procedure (steps 1-7) preserve atomicity under failure modes?
3. **Newly introduced abstractions don't add fresh problems** — ScopedAuthPort, HttpEndpointCache, KeystoneCapabilities. Any layering violation, ownership ambiguity, or test seam regression?
4. **Type design is consistent** — ContextSnapshot now carries token + epoch + scope. ContextRequest vs ContextTarget split (mentioned by Claude review only, may be missed in synthesis). SessionHandle's exposure level.
5. **Switcher procedure step 4 (rescope + refresh_catalog)** uses `.and_then` on `Future` — does this even compile? Does it correctly chain async ops? What about step 5 (commit) when refresh succeeds — is the order correct (commit applies new scope only after both succeed)?
6. **Cancel during Switching** — synthesis mentions adding `state.cancel()` and `session.rollback()`, but user-driven cancellation mid-rescope is non-trivial. Is it actually implementable as described?
7. **17-item checklist completeness** — anything you'd add or remove?
8. **PR1 implementability** — given the synthesis as DETAIL r2, can RED-phase tests be written today? Or are there still hidden specs?

DELIVER (concise):
- Top 3 issues with the synthesis (or "none" if it's solid)
- Specific corrections to the synthesis text
- Anything dropped from the original three reviews that should be restored
- Verdict on the synthesis: APPROVE | APPROVE-WITH-CHANGES | REJECT
- Brief reasoning

This is a meta-review — be thorough about whether Claude (chair) did synthesis correctly. Don't repeat the original critiques; assess only the consolidation quality.
codex
Reviewing the synthesis as a consolidation artifact, not re-running a fresh design review. I’ll read the synthesis and all three source reviews plus requirements/design, then map each original critical finding to whether it was preserved accurately and whether any new synthesis edits introduce technical issues.
exec
/bin/zsh -lc 'rg --files devflow-docs/inception | sort' in /Users/jay.ahn/projects/infra/nexttui/.worktrees/runtime-context-switch
 succeeded in 0ms:
devflow-docs/inception/application-design.md
devflow-docs/inception/btop-screen1.png
devflow-docs/inception/btop-screen2.png
devflow-docs/inception/btop-screen3.png
devflow-docs/inception/design-review-raw/claude-detail.md
devflow-docs/inception/design-review-raw/claude.md
devflow-docs/inception/design-review-raw/codex-detail.md
devflow-docs/inception/design-review-raw/codex-meta.md
devflow-docs/inception/design-review-raw/codex.md
devflow-docs/inception/design-review-raw/gemini-detail.md
devflow-docs/inception/design-review-raw/gemini-meta.md
devflow-docs/inception/design-review-raw/gemini.md
devflow-docs/inception/design-review-raw/synthesis-detail.md
devflow-docs/inception/design-review-raw/synthesis.md
devflow-docs/inception/requirements.md
devflow-docs/inception/workflow-plan.md
devflow-docs/inception/workspace.md

exec
/bin/zsh -lc 'pwd && ls -la' in /Users/jay.ahn/projects/infra/nexttui/.worktrees/runtime-context-switch
 succeeded in 0ms:
/Users/jay.ahn/projects/infra/nexttui/.worktrees/runtime-context-switch
total 240
drwxr-xr-x@ 16 jay.ahn  staff    512 Apr 13 15:54 .
drwxr-xr-x@  3 jay.ahn  staff     96 Apr 13 15:54 ..
drwxr-xr-x@  3 jay.ahn  staff     96 Apr 13 15:54 .claude
-rw-r--r--@  1 jay.ahn  staff     84 Apr 13 15:54 .git
-rw-r--r--@  1 jay.ahn  staff    256 Apr 13 15:54 .gitignore
-rw-r--r--@  1 jay.ahn  staff  81636 Apr 13 15:54 Cargo.lock
-rw-r--r--@  1 jay.ahn  staff    752 Apr 13 15:54 Cargo.toml
-rw-r--r--@  1 jay.ahn  staff   1064 Apr 13 15:54 LICENSE
-rw-r--r--@  1 jay.ahn  staff   6634 Apr 13 15:54 README.en.md
-rw-r--r--@  1 jay.ahn  staff   6941 Apr 13 15:54 README.md
-rw-r--r--@  1 jay.ahn  staff     58 Apr 13 15:54 clippy.toml
drwxr-xr-x@ 23 jay.ahn  staff    736 Apr 13 16:14 devflow-docs
-rw-r--r--@  1 jay.ahn  staff   2864 Apr 13 15:54 devstack-vm.yaml
drwxr-xr-x@  4 jay.ahn  staff    128 Apr 13 15:54 docs
drwxr-xr-x@ 23 jay.ahn  staff    736 Apr 13 15:54 src
drwxr-xr-x@  5 jay.ahn  staff    160 Apr 13 15:55 target

codex
I’ve confirmed the target files and will now extract the synthesis decisions and trace them against each source review line-by-line to identify dropped or distorted findings.
exec
/bin/zsh -lc 'wc -l devflow-docs/inception/design-review-raw/synthesis-detail.md devflow-docs/inception/design-review-raw/codex-detail.md devflow-docs/inception/design-review-raw/gemini-detail.md devflow-docs/inception/design-review-raw/claude-detail.md devflow-docs/inception/application-design.md devflow-docs/inception/requirements.md' in /Users/jay.ahn/projects/infra/nexttui/.worktrees/runtime-context-switch
 succeeded in 0ms:
     286 devflow-docs/inception/design-review-raw/synthesis-detail.md
    8766 devflow-docs/inception/design-review-raw/codex-detail.md
      64 devflow-docs/inception/design-review-raw/gemini-detail.md
     101 devflow-docs/inception/design-review-raw/claude-detail.md
     352 devflow-docs/inception/application-design.md
     112 devflow-docs/inception/requirements.md
    9681 total

exec
/bin/zsh -lc 'rg -n "''^#|''^##|''^###|ContextSessionPort|ScopedAuthPort|HttpEndpointCache|KeystoneCapabilities|ContextRequest|ContextTarget|SessionHandle|and_then|cancel|rollback|checklist|PR1|atomic|commit|refresh_catalog|rescope" devflow-docs/inception/design-review-raw/synthesis-detail.md' in /Users/jay.ahn/projects/infra/nexttui/.worktrees/runtime-context-switch
 succeeded in 0ms:
1:# Council Synthesis — application-design.md DETAIL
7:Codex의 REJECT는 "atomic 계약과 port 경계의 보강 없이는 RED 진입 위험"이라는 정확한 지적. 종합 개정안에 핵심 사항을 모두 반영하면 APPROVE 수준으로 충분히 끌어올릴 수 있다.
11:## 1. 합의 사항 (3-AI 일치)
13:### A. atomic 계약을 ContextSessionPort에 완전히 위임 (Codex+Gemini 합의)
15:**현재 문제**: ContextSwitcher가 절차의 중간에서 `rescope_adapter.rescope`, `catalog_invalidator.invalidate_all`, `token_cache.store`를 직접 호출. begin/commit 사이에 외부 호출이 끼어들어 atomicity 보장 불가.
19:#[async_trait]
20:pub trait ContextSessionPort: Send + Sync {
22:    async fn begin(&self, target: &ContextTarget, epoch: Epoch) -> Result<Self::Handle, SwitchError>;
23:    async fn rescope(&self, handle: &mut Self::Handle) -> Result<(), SwitchError>;
24:    async fn refresh_catalog(&self, handle: &mut Self::Handle) -> Result<(), SwitchError>;
25:    async fn commit(&self, handle: Self::Handle) -> Result<ContextSnapshot, SwitchError>;
26:    async fn rollback(&self, handle: Self::Handle) -> Result<(), SwitchError>;
29:ContextSwitcher는 state/epoch/cancel만 오케스트레이트. rescope·invalidate·token store는 모두 port 구현 내부.
31:### B. SessionHandle은 이전 token + scope를 캡처 (Gemini+Claude)
34:pub struct SessionHandle {
38:    pub target: ContextTarget,
42:rollback 시 ScopedAuthSession이 KeystoneAuthAdapter의 active_scope/token을 이전 값으로 강제 복원.
44:### C. 단일 epoch 게이트를 App/event-loop 디스패처에 명시 (Codex+Gemini)
51:### D. Worker 시그니처 통일 (Gemini+Claude)
62:### E. SwitchError::InProgress를 에러 타입으로 못 박지 않음 (Codex+Claude)
65:#[derive(Debug, thiserror::Error)]
68:    #[error("rescope rejected: {0}")]          RescopeRejected(String),
70:    #[error("ambiguous target")]               Ambiguous { candidates: Vec<ContextTarget> },
76:pub fn try_begin(&self, target: ContextTarget) -> Result<Epoch, SwitchError>; // 변수 SwitchError
79:### F. ContextIndicator 패시브 타이머 (Gemini+Claude)
97:### G. ContextSnapshot에 epoch + token 포함 (Gemini)
101:    pub target: ContextTarget,
109:### H. AppEvent::ContextChanged에서 epoch 제거 — VersionedEvent envelope이 이미 보유 (Codex)
113:    ContextChanged { target: ContextTarget },
120:## 2. Codex 단독 — 채택
122:### ScopedAuthPort 신설 (port 경계 보강)
126:#[async_trait]
127:pub trait ScopedAuthPort: Send + Sync {
134:KeystoneAuthAdapter가 양쪽 trait 모두 구현. ScopedAuthSession은 ScopedAuthPort를 통해 active_scope를 mutate.
136:### 미명시 정책 명문화
138:- **Switching + (SwitchContext|SwitchBack|Cancel) 정책**: Switching 중 신규 switch 요청 → 즉시 `SwitchError::InProgress` 반환. Cancel 명령 → SwitchStateMachine에 `cancel()` 추가, port 핸들이 있으면 rollback 호출.
140:- **TokenCacheStore 키 정렬**: `store_scoped(&self, scope: &TokenScope, token: &Token)` — 기존 모델 일치, ContextTarget 직접 키 사용 금지.
141:- **list_user_projects를 async로**: `pub async fn list_user_projects(&self) -> Result<Vec<ContextTarget>, SwitchError>` (Keystone API 호출).
143:### Mock 시즘 확장
146:- `with_begin_failure`, `with_rescope_failure`, `with_refresh_failure`, `with_commit_failure`
147:- `rollback_called() -> bool`, `transition_steps() -> Vec<&'static str>` (순서 검증)
151:## 3. Claude 단독 — 채택
153:### HttpEndpointCache trait 추가
157:pub trait HttpEndpointCache {
161:모든 `BaseHttpClient`가 구현. AdapterRegistry가 `Vec<Arc<dyn HttpEndpointCache>>` 보유.
163:### KeystoneCapabilities 정의
166:#[derive(Debug, Clone)]
167:pub struct KeystoneCapabilities {
168:    pub allow_rescope_scoped_token: bool,
176:### Test seam 추가
178:- `MockContextSession::with_partial_commit_failure` (rescope OK + invalidate OK + commit fail)
179:- `MockContextSession::with_slow_rescope(Duration)` (timeout 시뮬레이션)
185:## 4. Switcher 절차 개정 (모두 통합)
188:pub async fn switch(&self, target: ContextTarget) -> Result<(Epoch, ContextSnapshot), SwitchError> {
192:    // 2. 이전 epoch의 모든 작업 cancel (idempotent, 두 번 호출 안전)
193:    self.cancellation.cancel_below(new_epoch);
201:    // 4. atomic transition (모두 port 내부)
202:    if let Err(e) = self.session.rescope(&mut handle).await
203:        .and_then(|_| self.session.refresh_catalog(&mut handle).await) {
204:        let _ = self.session.rollback(handle).await;
209:    // 5. commit (port 내부에서 ScopedAuthPort.set_active_scope + token store)
210:    let snapshot = match self.session.commit(handle).await {
215:    // 6. state.commit + history push
216:    self.state.commit(snapshot.clone());
219:    // 7. (마지막 안전망) 다시 한 번 cancel
220:    self.cancellation.cancel_below(new_epoch);
226:Switcher는 `state + cancellation + session` 3개에만 의존. rescope/invalidate/token store는 모두 session 내부로 이동.
228:## 5. 컴포넌트 의존 그래프 (개정)
238:            State  Cancel  ContextSessionPort (trait)
253:                                               HttpEndpointCache trait
256:ContextSwitcher의 협력자가 8개 → 3개로 축소. atomic 책임이 단일 port에 집중.
260:## 6. DETAIL r2 개정 체크리스트
262:- [ ] ContextSessionPort에 `Handle` 연관타입 + rescope/refresh_catalog/commit/rollback 메서드 도입
265:- [ ] SessionHandle 정의 (previous_token, previous_scope 포함)
268:- [ ] ScopedAuthPort 신설 + KeystoneAuthAdapter가 구현
269:- [ ] HttpEndpointCache trait 신설
270:- [ ] KeystoneCapabilities 정의 명시
276:- [ ] ContextTargetResolver.list_user_projects를 async로
277:- [ ] Switching 정책: 신규 switch → InProgress, Cancel → state.cancel + session.rollback
278:- [ ] MockContextSession 시즘 확장 (with_*_failure, transition_steps, rollback_called)
282:## 최종 Verdict
286:핵심 변경 1줄 요약: **atomic 책임을 ContextSessionPort 내부로 완전 통합 + 단일 epoch 게이트 디스패처에 명시 + ScopedAuthPort 분리**.

exec
/bin/zsh -lc 'rg -n "Critical|High|Medium|atomic|ContextSessionPort|ContextRequest|ContextTarget|SessionHandle|ScopedAuthPort|HttpEndpointCache|KeystoneCapabilities|and_then|cancel|rollback|checklist|PR1|hidden|test|rescope|refresh_catalog|commit" devflow-docs/inception/design-review-raw/codex-detail.md devflow-docs/inception/design-review-raw/gemini-detail.md devflow-docs/inception/design-review-raw/claude-detail.md' in /Users/jay.ahn/projects/infra/nexttui/.worktrees/runtime-context-switch
 succeeded in 0ms:
devflow-docs/inception/design-review-raw/gemini-detail.md:6:The technical review of the **DETAIL** section for feature **BL-P2-031** is as follows. The design is robust in its use of epochs for concurrency control, but there are critical ambiguities in ownership and atomicity that must be addressed before code generation.
devflow-docs/inception/design-review-raw/gemini-detail.md:8:### Top 3 Critical Issues
devflow-docs/inception/design-review-raw/gemini-detail.md:11:    *   **What**: `ContextSwitcher` (Service) depends on both `ContextSessionPort` (Port) and `KeystoneRescopeAdapter` (Adapter). The procedure description has `ContextSwitcher` manually calling `rescope` and `invalidate_all` between `session.begin` and `session.commit`.
devflow-docs/inception/design-review-raw/gemini-detail.md:12:    *   **Why**: This splits the atomic transition logic across two layers. If `ContextSessionPort` is the boundary for a "Session," it should encapsulate the *entire* session-level change (rescope + catalog refresh + endpoint invalidation).
devflow-docs/inception/design-review-raw/gemini-detail.md:13:    *   **Fix**: Move the rescope and invalidation calls *inside* the `ContextSessionPort` implementation. `ContextSwitcher` should only call `port.transition(target)`, which handles the Keystone handshake and cache clearing as a single unit.
devflow-docs/inception/design-review-raw/gemini-detail.md:16:    *   **What**: `ContextSessionPort::rollback(handle)` is defined, but `SessionHandle` is opaque. If `rescope` succeeds but `invalidate_all` or `catalog_refresh` fails, the `AuthProvider` state is already mutated (new token scoped to new project).
devflow-docs/inception/design-review-raw/gemini-detail.md:17:    *   **Why**: Without restoring the old token/catalog in the `AuthProvider`, the "rollback" only reverts the UI state, leaving the HTTP client with a new-project token but a "Failed" switcher state—a torn state.
devflow-docs/inception/design-review-raw/gemini-detail.md:18:    *   **Fix**: `SessionHandle` must contain the **captured previous `Token` and `TokenScope`**. `rollback` must explicitly re-inject these into the `AuthProvider` (KeystoneAuthAdapter) to guarantee a return to the exact previous state.
devflow-docs/inception/design-review-raw/gemini-detail.md:20:3.  **Passive UI Highlight Hazard (UI Correctness)**
devflow-docs/inception/design-review-raw/gemini-detail.md:31:    pub target: ContextTarget,
devflow-docs/inception/design-review-raw/gemini-detail.md:37:// src/port/auth.rs (ContextSessionPort trait)
devflow-docs/inception/design-review-raw/gemini-detail.md:38:// Change: Atomic transition instead of begin/commit split to prevent logic leakage.
devflow-docs/inception/design-review-raw/gemini-detail.md:40:pub trait ContextSessionPort: Send + Sync {
devflow-docs/inception/design-review-raw/gemini-detail.md:41:    /// Performs rescope + catalog refresh. Returns handle for rollback.
devflow-docs/inception/design-review-raw/gemini-detail.md:42:    async fn transition(&self, target: &ContextTarget) -> Result<(SessionHandle, ContextSnapshot), SwitchError>;
devflow-docs/inception/design-review-raw/gemini-detail.md:43:    async fn rollback(&self, handle: SessionHandle) -> Result<(), SwitchError>;
devflow-docs/inception/design-review-raw/gemini-detail.md:58:2.  **Opaque `SessionHandle` Definition**: Needs to be a struct in `src/port/auth.rs` containing `old_token: Token` and `old_scope: TokenScope`.
devflow-docs/inception/design-review-raw/gemini-detail.md:62:**Reasoning**: The concurrency model (Epoch + CancellationToken) is excellent and follows Rust best practices for async TUI. However, the layering of the `ContextSwitcher` vs `ContextSessionPort` is "chatty" and risks leaving the application in a torn state if the rollback doesn't restore the `AuthProvider` token. Consolidating the rescope logic into the `Port` implementation ensures true atomicity.
devflow-docs/inception/design-review-raw/gemini-detail.md:64:**Next Step**: Once these ownership and rollback details are clarified in the document, the project is ready for the **RED** phase of PR1 (Infrastructure).
devflow-docs/inception/design-review-raw/claude-detail.md:4:**Mode**: DETAIL section critique (interface signatures, dependency direction, atomicity)
devflow-docs/inception/design-review-raw/claude-detail.md:6:## Top 3 Critical Issues
devflow-docs/inception/design-review-raw/claude-detail.md:18:      pub fn try_begin(&self, target: ContextTarget) -> Result<Epoch, SwitchError> { ... }
devflow-docs/inception/design-review-raw/claude-detail.md:23:- **What**: 명시된 순서: `state.begin → registry.cancel_below(new_epoch) → session.begin → rescope → invalidate → token_cache.store → state.commit`. 그러나 `state.begin`이 epoch를 bump한다고 가정해도, cancel_below는 **새 epoch 미만의 모든 작업** cancel — 즉 새 epoch 자체의 token도 등록되기 전에 cancel 필터를 통과한다. OK. 하지만 worker spawn이 register한 토큰을 begin과 동시에 다른 thread가 spawn 중이라면, cancel이 새 spawn을 놓칠 수 있음.
devflow-docs/inception/design-review-raw/claude-detail.md:24:- **Why**: 동시성 race — switch 시작 직후 들어온 신규 spawn이 옛 epoch로 등록되면 cancel 우회.
devflow-docs/inception/design-review-raw/claude-detail.md:25:- **Fix**: spawn API를 `(epoch, cancel) = registry.register_with_current()`로 강제 — registry 내부에서 `current_epoch == registry.frozen_epoch?` 검사. 또는 switch 진입 시 spawn lock 잠시 차단. 또는 `cancel_below(new_epoch)`를 `state.commit` 직전에 1회 더 호출(idempotent).
devflow-docs/inception/design-review-raw/claude-detail.md:27:### 3. ContextTarget vs TokenScope 타입 중복 + Resolver의 ProjectRef 미해결 상태
devflow-docs/inception/design-review-raw/claude-detail.md:28:- **What**: `ContextTarget { cloud, project: ProjectRef, domain }`, `TokenScope::Project { name, domain }`. Resolver는 ProjectRef를 받아 ContextTarget을 반환해야 하는데, ContextTarget 내부에 ProjectRef가 또 들어있음. "resolved target"과 "user input"이 같은 타입.
devflow-docs/inception/design-review-raw/claude-detail.md:32:  pub enum ContextRequest {            // user input (parser output)
devflow-docs/inception/design-review-raw/claude-detail.md:36:  pub struct ContextTarget {           // resolved (post-resolver)
devflow-docs/inception/design-review-raw/claude-detail.md:43:  Resolver: `ContextRequest → ContextTarget`. TokenScope는 `From<&ContextTarget>`로 변환.
devflow-docs/inception/design-review-raw/claude-detail.md:48:DETAIL 표현 `pub fn spawn(epoch, cancel, fut: impl Future<Output = AppEvent>) -> JoinHandle<()>` — 여러 polling 사이트가 다른 반환 타입을 가짐. 일반화 필요:
devflow-docs/inception/design-review-raw/claude-detail.md:61:### KeystoneCapabilities 미정의
devflow-docs/inception/design-review-raw/claude-detail.md:65:pub struct KeystoneCapabilities {
devflow-docs/inception/design-review-raw/claude-detail.md:66:    pub allow_rescope_scoped_token: bool,
devflow-docs/inception/design-review-raw/claude-detail.md:75:**Fix**: `pub trait HttpEndpointCache { fn invalidate(&self); }` 추가 후 모든 `BaseHttpClient`가 구현. Invalidator는 registry 순회.
devflow-docs/inception/design-review-raw/claude-detail.md:93:- **MockContextSession** — `with_rescope_failure`, `with_invalidate_failure`까지는 좋음. 추가 필요: `with_partial_commit_failure` (rescope OK + invalidate OK + token store fail), `with_slow_rescope` (timeout 시뮬레이션).
devflow-docs/inception/design-review-raw/claude-detail.md:94:- **Worker epoch 검증** — 단위 테스트가 어디 들어가는지 미명시. `worker.rs::tests` 안에 stale event drop 케이스 추가 필요.
devflow-docs/inception/design-review-raw/claude-detail.md:101:이유: 컴포넌트 분해와 의존 그래프는 정합. 그러나 (1) `Result<Epoch, SwitchError::InProgress>` 같은 컴파일 불가 시그니처, (2) ContextTarget vs TokenScope 타입 중복, (3) atomicity 경계의 동시성 race 처리 미명시 — 이 셋이 RED phase 진입 전 보강 필수. HttpEndpointCache trait 부재도 빠진 인터페이스로 명시.
devflow-docs/inception/design-review-raw/codex-detail.md:34:3. **Atomicity gaps** — Does the begin/commit/rollback flow truly guarantee no torn state? Particularly around: rescope OK + invalidate fail; epoch bump vs cancel ordering; commit ordering (token cache write before or after state.commit?)
devflow-docs/inception/design-review-raw/codex-detail.md:36:5. **State machine completeness** — Missing transitions (e.g. cancel during Switching, second switch request mid-flight)?
devflow-docs/inception/design-review-raw/codex-detail.md:38:7. **Test seams** — MockContextSession: does it cover all failure modes the production code can hit? Is there a way to deterministically reproduce "rescope succeeds + catalog refresh fails + rollback runs"?
devflow-docs/inception/design-review-raw/codex-detail.md:39:8. **Codebase fit** — Are types like ContextTarget overlapping with existing TokenScope? Should ContextTarget reuse TokenScope or be a new type?
devflow-docs/inception/design-review-raw/codex-detail.md:65:    11	- **ContextSwitcher**는 commit 결과로 `(new_epoch, ContextSnapshot)` 반환. App이 epoch를 반영하고 `AppEvent::ContextChanged`를 디스패치.
devflow-docs/inception/design-review-raw/codex-detail.md:66:    12	- **ContextSwitcher**는 `ContextSessionPort`를 통해 atomic begin/commit/rollback 수행.
devflow-docs/inception/design-review-raw/codex-detail.md:75:    21	| `ContextEpoch` | 단조 증가 epoch 카운터, stale 이벤트 식별 키 | Util | PR1 |
devflow-docs/inception/design-review-raw/codex-detail.md:76:    22	| `CancellationRegistry` | 활성 폴링/장기 fetch에 대한 `CancellationToken` 등록·일괄 취소 | Service | PR1 |
devflow-docs/inception/design-review-raw/codex-detail.md:77:    23	| `VersionedEvent<T>` envelope | `{ event: T, epoch: u64 }` — Action/AppEvent를 감싸 epoch stamp (variant 폭증 회피) | Util | PR1 |
devflow-docs/inception/design-review-raw/codex-detail.md:78:    24	| `AppEvent::ContextChanged { target }` | UI에 컨텍스트 변경 통지 → 모듈 컴포넌트가 내부 데이터 비움 | Event variant | PR1 |
devflow-docs/inception/design-review-raw/codex-detail.md:79:    25	| `SwitchStateMachine` | `Idle → Switching → Committed | Failed` 전환 상태, rollback 규약 | Service | PR1 |
devflow-docs/inception/design-review-raw/codex-detail.md:80:    26	| `ContextSwitcher` | 전환 절차 오케스트레이터 (epoch++ → cancel → session begin → rescope → catalog 무효화 → commit) | Service | PR1 |
devflow-docs/inception/design-review-raw/codex-detail.md:81:    27	| `ContextSessionPort` (trait) | atomic begin/commit/rollback 인터페이스. `invalidate_all_endpoints()` hook | Port | PR1 |
devflow-docs/inception/design-review-raw/codex-detail.md:82:    28	| `ScopedAuthSession` | `ContextSessionPort` 구현체. 활성 scoped token + endpoint cache 일괄 관리 | Service | PR1 |
devflow-docs/inception/design-review-raw/codex-detail.md:83:    29	| `KeystoneRescopeAdapter` | Keystone v3 token-method scoped exchange 호출, expires_at 정본 사용 | Adapter | PR1 |
devflow-docs/inception/design-review-raw/codex-detail.md:84:    30	| `EndpointCatalogInvalidator` | 모든 HTTP client의 endpoint cache 일괄 무효화 (`src/adapter/http/base.rs` 자동화) | Service | PR1 |
devflow-docs/inception/design-review-raw/codex-detail.md:85:    31	| `ContextTargetResolver` | name/uuid/cloud-prefix → ContextTarget 변환, 충돌 disambiguation. 명령·피커·모듈 액션 공유 | Service | PR1 |
devflow-docs/inception/design-review-raw/codex-detail.md:86:    32	| `ContextHistoryStore` | switch-back 1단계 히스토리 (ContextSnapshot 저장), rollback 시 동일 사용 | Util | PR1 |
devflow-docs/inception/design-review-raw/codex-detail.md:90:    36	> 신규 컴포넌트 14개 (PR1 인프라 12 + UI 2). `AppEvent::ContextChanged`는 enum variant 추가지만 표에 분리 표기.
devflow-docs/inception/design-review-raw/codex-detail.md:96:    42	| `App` (src/app.rs) | 활성 cloud/project 컨텍스트 보유, ContextSwitcher 소유, epoch/스냅샷 반영, ContextChanged 디스패치 | Controller (재분류) | PR1 |
devflow-docs/inception/design-review-raw/codex-detail.md:97:    43	| `Worker` (src/worker.rs) | spawn API에 `(epoch, CancellationToken)` 페어 강제, `tokio::select!` cancel branch + epoch 검증 | Service | PR1 |
devflow-docs/inception/design-review-raw/codex-detail.md:98:    44	| `Action` (src/action.rs) | `VersionedEvent<Action>` envelope 적용 (또는 `Action::with_epoch`) | Type | PR1 |
devflow-docs/inception/design-review-raw/codex-detail.md:99:    45	| `AppEvent` (src/event.rs) | `VersionedEvent<AppEvent>` envelope 적용 + `ContextChanged` variant 추가 | Type | PR1 |
devflow-docs/inception/design-review-raw/codex-detail.md:100:    46	| `CommandParser` (src/input/command.rs) | `:switch-project <name|uuid|cloud/project>`, `:switch-cloud`, `:switch-back` 등록 + tab 자동완성. `ContextTargetResolver` 사용 | Controller | PR3 |
devflow-docs/inception/design-review-raw/codex-detail.md:103:    49	| `TokenCacheStore` (src/adapter/auth/token_cache.rs) | rescoped 토큰을 scope별로 저장/조회 (BL-P2-029 기반), 만료 시 재취득 | Repository | PR1 |
devflow-docs/inception/design-review-raw/codex-detail.md:112:    58	| `MockContextSession` (확장) | `src/port/mock.rs`에 추가. rescope 성공/실패, catalog invalidate 부분 실패 시뮬레이션 (fault-injection seam) | Mock | PR1 |
devflow-docs/inception/design-review-raw/codex-detail.md:120:    66	| **PR1 (safety infra + switch core)** | ContextEpoch, CancellationRegistry, VersionedEvent, AppEvent::ContextChanged, SwitchStateMachine, ContextSwitcher, ContextSessionPort, ScopedAuthSession, KeystoneRescopeAdapter, EndpointCatalogInvalidator, ContextTargetResolver, ContextHistoryStore, App 통합, Worker 시그니처 개정, Action/AppEvent envelope, TokenCacheStore 확장, MockContextSession | — | 없음 (인프라만) |
devflow-docs/inception/design-review-raw/codex-detail.md:121:    67	| **PR3 (안전 가시성 + 명령)** | ContextIndicator, StatusBar 임베드, ConfirmDialog fingerprint, CommandParser 확장 | PR1 | `:switch-*` 명령, 컨텍스트 인디케이터, fingerprint confirm |
devflow-docs/inception/design-review-raw/codex-detail.md:125:    71	→ **PR 수: 6 → 4로 축소**. PR1 통합으로 stale 누설 창 제거. PR3에 안전 가시성 + 명령을 묶어 사용자가 전환을 쓸 수 있는 첫 시점부터 인디케이터/fingerprint가 함께 동작.
devflow-docs/inception/design-review-raw/codex-detail.md:131:    77	| NFR-1 안전성 (atomic switch, stale 차단) | ContextEpoch + CancellationRegistry + SwitchStateMachine + ContextSessionPort/ScopedAuthSession + EndpointCatalogInvalidator |
devflow-docs/inception/design-review-raw/codex-detail.md:133:    79	| NFR-3 테스트 커버리지 | MockContextSession (port mock), ContextSwitcher 통합 테스트, 부분실패 (rescope OK + invalidate fail) 시뮬레이션 |
devflow-docs/inception/design-review-raw/codex-detail.md:134:    80	| NFR-4 UX 일관성 | ContextPicker (기존 SelectPopup 재사용), CommandParser 확장 (기존 패턴), ContextTargetResolver (단일 disambiguation 로직) |
devflow-docs/inception/design-review-raw/codex-detail.md:141:    87	| ContextSessionPort + ScopedAuthSession 추가 | Codex | atomic begin/commit/rollback 부재 시 rescope 성공 + stale endpoint 호출 사고 가능 |
devflow-docs/inception/design-review-raw/codex-detail.md:143:    89	| ContextHistoryStore 추가 | Codex+Claude | switch-back/rollback 공통 저장소 누락 |
devflow-docs/inception/design-review-raw/codex-detail.md:144:    90	| ContextTargetResolver 추가 | Codex | 명령·피커·모듈 액션이 같은 disambiguation 로직 필요 (3중 구현 회피) |
devflow-docs/inception/design-review-raw/codex-detail.md:150:    96	| PR1+PR2 통합 | Claude+Codex | PR1만 머지 시 사용자 노출 0이지만 stale 누설 창 발생. 통합으로 창 제거 |
devflow-docs/inception/design-review-raw/codex-detail.md:169:   115	pub struct ContextTarget {
devflow-docs/inception/design-review-raw/codex-detail.md:177:   123	    pub target: ContextTarget,
devflow-docs/inception/design-review-raw/codex-detail.md:185:   131	    #[error("rescope rejected by Keystone: {0}")]
devflow-docs/inception/design-review-raw/codex-detail.md:190:   136	    Ambiguous { candidates: Vec<ContextTarget> },
devflow-docs/inception/design-review-raw/codex-detail.md:212:   158	- `pub fn cancel_below(&self, threshold_epoch: Epoch) -> usize` — 이전 epoch 토큰 일괄 cancel, 취소 건수 반환
devflow-docs/inception/design-review-raw/codex-detail.md:227:   173	- `AppEvent::ContextChanged { target: ContextTarget, epoch: Epoch }`
devflow-docs/inception/design-review-raw/codex-detail.md:228:   174	**Dependencies**: `ContextTarget` (src/context.rs)
devflow-docs/inception/design-review-raw/codex-detail.md:232:   178	**Responsibility**: `Idle → Switching → Committed | Failed` 상태 전이, rollback 규약.
devflow-docs/inception/design-review-raw/codex-detail.md:234:   180	- `pub fn try_begin(&mut self, target: ContextTarget) -> Result<Epoch, SwitchError::InProgress>`
devflow-docs/inception/design-review-raw/codex-detail.md:235:   181	- `pub fn commit(&mut self, snapshot: ContextSnapshot)` — Committed 상태로 이동
devflow-docs/inception/design-review-raw/codex-detail.md:236:   182	- `pub fn fail(&mut self, err: SwitchError) -> ContextSnapshot` — Failed 후 직전 snapshot 반환 (rollback용)
devflow-docs/inception/design-review-raw/codex-detail.md:243:   189	- `pub async fn switch(&self, target: ContextTarget) -> Result<ContextSnapshot, SwitchError>`
devflow-docs/inception/design-review-raw/codex-detail.md:245:   191	- 절차: state.begin → registry.cancel_below(new_epoch) → session.begin → rescope_adapter.rescope → catalog_invalidator.invalidate_all → token_cache.store → state.commit → return snapshot
devflow-docs/inception/design-review-raw/codex-detail.md:246:   192	**Dependencies**: `SwitchStateMachine`, `CancellationRegistry`, `ContextSessionPort`, `KeystoneRescopeAdapter`, `EndpointCatalogInvalidator`, `TokenCacheStore`, `ContextHistoryStore`, `ContextEpoch`
devflow-docs/inception/design-review-raw/codex-detail.md:248:   194	### ContextSessionPort (Port trait)
devflow-docs/inception/design-review-raw/codex-detail.md:249:   195	**Responsibility**: 활성 scoped token + endpoint cache의 atomic begin/commit/rollback 인터페이스.
devflow-docs/inception/design-review-raw/codex-detail.md:253:   199	pub trait ContextSessionPort: Send + Sync {
devflow-docs/inception/design-review-raw/codex-detail.md:254:   200	    async fn begin(&self, target: &ContextTarget) -> Result<SessionHandle, SwitchError>;
devflow-docs/inception/design-review-raw/codex-detail.md:255:   201	    async fn commit(&self, handle: SessionHandle) -> Result<ContextSnapshot, SwitchError>;
devflow-docs/inception/design-review-raw/codex-detail.md:256:   202	    async fn rollback(&self, handle: SessionHandle);
devflow-docs/inception/design-review-raw/codex-detail.md:260:   206	**Dependencies**: `ContextTarget`, `ContextSnapshot`, `SwitchError`
devflow-docs/inception/design-review-raw/codex-detail.md:262:   208	### ScopedAuthSession (Service — ContextSessionPort impl)
devflow-docs/inception/design-review-raw/codex-detail.md:263:   209	**Responsibility**: `ContextSessionPort` 구현. 토큰 + endpoint cache 일괄 관리.
devflow-docs/inception/design-review-raw/codex-detail.md:270:   216	- `pub async fn rescope(&self, current_token: &Token, target: &ContextTarget) -> Result<Token, SwitchError>`
devflow-docs/inception/design-review-raw/codex-detail.md:271:   217	- `pub fn capabilities(&self) -> KeystoneCapabilities` — `allow_rescope_scoped_token`, app-credential 여부 등
devflow-docs/inception/design-review-raw/codex-detail.md:278:   224	- `pub async fn refresh_catalog(&self, token: &Token) -> Result<ServiceCatalog, SwitchError>`
devflow-docs/inception/design-review-raw/codex-detail.md:281:   227	### ContextTargetResolver (Service)
devflow-docs/inception/design-review-raw/codex-detail.md:282:   228	**Responsibility**: name/uuid/cloud-prefix → ContextTarget 변환 + 충돌 disambiguation.
devflow-docs/inception/design-review-raw/codex-detail.md:284:   230	- `pub fn resolve(&self, input: &str, current_cloud: &str) -> Result<ContextTarget, SwitchError>` — 단일 매치 시 OK, 충돌 시 `Ambiguous`, 미매치 시 `NotFound`
devflow-docs/inception/design-review-raw/codex-detail.md:285:   231	- `pub fn list_user_projects(&self) -> Vec<ContextTarget>` — 피커용 (`/v3/auth/projects` 결과 캐시)
devflow-docs/inception/design-review-raw/codex-detail.md:289:   235	**Responsibility**: switch-back 1단계 + rollback용 직전 snapshot 보관.
devflow-docs/inception/design-review-raw/codex-detail.md:308:   254	- `pub fn open(&mut self, candidates: Vec<ContextTarget>, current: Option<&ContextTarget>)`
devflow-docs/inception/design-review-raw/codex-detail.md:311:   257	**Dependencies**: `SelectPopup` (src/ui/select_popup.rs 재사용), `ContextTarget`, `Theme`
devflow-docs/inception/design-review-raw/codex-detail.md:314:   260	**Responsibility**: `ContextSessionPort`의 테스트 더블. fault-injection.
devflow-docs/inception/design-review-raw/codex-detail.md:330:    12	**구현 전략**: 단일 BL을 단계적 PR로 분할 (옵션 C). feature 브랜치에 PR1~PR6 누적 머지 → 통합 검증 후 main에 단일 머지.
devflow-docs/inception/design-review-raw/codex-detail.md:354:    36	- `Switching` 진입 시: epoch++, 이전 컨텍스트의 폴링/in-flight 작업 cancel, destructive 액션 입력 차단
devflow-docs/inception/design-review-raw/codex-detail.md:355:    37	- Keystone rescope 호출 → service catalog 강제 재조회 → 새 컨텍스트로 commit
devflow-docs/inception/design-review-raw/codex-detail.md:356:    38	- 실패 시 이전 컨텍스트로 rollback하고 사용자에게 가시적 에러 표시
devflow-docs/inception/design-review-raw/codex-detail.md:360:    42	- 폴링 루프와 장기 fetch는 `tokio::select!` cancel branch 또는 epoch 검증 필수
devflow-docs/inception/design-review-raw/codex-detail.md:366:    48	- rescope 후 service catalog와 endpoint 캐시 무효화·재조회
devflow-docs/inception/design-review-raw/codex-detail.md:367:    49	- rescope 거부 (예: `allow_rescope_scoped_token=false`, app-credential, 권한 부족) 시 가시적 실패 + 사용자에게 full re-auth 안내
devflow-docs/inception/design-review-raw/codex-detail.md:387:    69	### NFR-1. 안전성 (Critical)
devflow-docs/inception/design-review-raw/codex-detail.md:389:    71	- rescope 실패 시 컨텍스트 인디케이터와 실제 활성 컨텍스트가 불일치해서는 안 된다 (atomic commit)
devflow-docs/inception/design-review-raw/codex-detail.md:392:    74	- 전환 액션 (피커 선택 → commit)은 정상 경로에서 1초 이내 완료를 목표
devflow-docs/inception/design-review-raw/codex-detail.md:393:    75	- rescope + catalog 재조회의 네트워크 왕복을 합산해 측정
devflow-docs/inception/design-review-raw/codex-detail.md:397:    79	- 통합 테스트: rescope 성공/실패, catalog 재조회 실패, 전환 중 in-flight 폴링, app-credential 경로 거부
devflow-docs/inception/design-review-raw/codex-detail.md:398:    80	- 기존 1116 tests baseline 무회귀
devflow-docs/inception/design-review-raw/codex-detail.md:406:    88	- rescope 실패 사유 로깅
devflow-docs/inception/design-review-raw/codex-detail.md:415:    97	| Test Framework | built-in `#[cfg(test)]` | Brownfield 감지 | — |
devflow-docs/inception/design-review-raw/codex-detail.md:455:    21	- **Codex**: App을 Service에서 Controller로 재분류, ContextSwitcher가 atomic boundary 보유
devflow-docs/inception/design-review-raw/codex-detail.md:459:    25	→ **결정**: LIST에 의존 방향 한 줄 추가. App이 Switcher를 소유, Switcher는 commit 결과로 `(new_epoch, snapshot)` 반환.
devflow-docs/inception/design-review-raw/codex-detail.md:463:    29	- **Gemini**: PR1 plumbing이 Action/AppEvent 변경까지 포함해야 — 범위 과소평가
devflow-docs/inception/design-review-raw/codex-detail.md:464:    30	- **Claude**: PR2가 PR1 미포함 시 stale 이벤트 누설
devflow-docs/inception/design-review-raw/codex-detail.md:467:    33	1. PR1 + PR2 통합 — "safety infra + switch core" 단일 PR (PR1만으로 사용자 노출 0이라 분리 가치 약함, 통합 시 stale 누설 창 자체 제거)
devflow-docs/inception/design-review-raw/codex-detail.md:478:    44	- **Claude**: 모든 spawn 시그니처가 `(epoch, cancel_token)` 페어 강제
devflow-docs/inception/design-review-raw/codex-detail.md:484:    50	## 2. Codex 단독 — 채택할 추가 컴포넌트 (Critical)
devflow-docs/inception/design-review-raw/codex-detail.md:488:    54	| `ContextSessionPort` (Port) + `ScopedAuthSession` (Service) | 활성 scoped token + endpoint cache 무효화의 atomic begin/commit/rollback | `AuthProvider`에 scope-switch API 부재. `KeystoneAuthAdapter.active_scope`는 fixed state. atomic boundary 없으면 rescope 성공 + stale endpoint 호출 사고 가능 |
devflow-docs/inception/design-review-raw/codex-detail.md:491:    57	| `ContextTargetResolver` (Service) | name/uuid/cloud-prefix → ContextTarget 변환, 충돌 disambiguation | 명령·피커·Identity `s` 액션 셋 모두 같은 로직 필요. 공유하지 않으면 3중 구현 |
devflow-docs/inception/design-review-raw/codex-detail.md:507:    73	- `port::auth::MockAuthProvider`에 rescope mock 추가 (기존 `src/port/mock.rs` 확장)
devflow-docs/inception/design-review-raw/codex-detail.md:508:    74	- `ContextSessionPort` 도입으로 fault-injection seam 자연 확보 (rescope OK + invalidate fail 시뮬레이션 가능)
devflow-docs/inception/design-review-raw/codex-detail.md:514:    80	| **PR1+2 통합** ("safety infra + switch core") | ContextEpoch + CancellationRegistry, VersionedEvent envelope, AppEvent::ContextChanged, Action epoch 필드, Worker epoch+cancel 검증, SwitchStateMachine, ContextSwitcher, ContextSessionPort + ScopedAuthSession, KeystoneRescopeAdapter, EndpointCatalogInvalidator, TokenCacheStore 확장, ContextHistoryStore, ContextTargetResolver, App 통합, port mock 확장 | — |
devflow-docs/inception/design-review-raw/codex-detail.md:515:    81	| **PR3** (안전 가시성 + 명령) | ContextIndicator (UI widget), StatusBar 통합, ConfirmDialog fingerprint, CommandParser 확장 (`:switch-*`, `:switch-back`) | PR1+2 |
devflow-docs/inception/design-review-raw/codex-detail.md:519:    85	→ **PR 수: 6개 → 4개로 축소**. PR1+2 통합으로 stale 누설 창 제거, PR3에 안전 가시성 묶어 사용자 노출 시점에 안전성 보장.
devflow-docs/inception/design-review-raw/codex-detail.md:535:   101	- [ ] ContextSessionPort + ScopedAuthSession 추가 (Port + Service)
devflow-docs/inception/design-review-raw/codex-detail.md:538:   104	- [ ] ContextTargetResolver 추가
devflow-docs/inception/design-review-raw/codex-detail.md:545:   111	- [ ] PR1+PR2 통합, PR3에 안전 가시성 묶음
devflow-docs/inception/design-review-raw/codex-detail.md:554:   260	**Responsibility**: `ContextSessionPort`의 테스트 더블. fault-injection.
devflow-docs/inception/design-review-raw/codex-detail.md:556:   262	- `pub fn with_rescope_failure(self, err: SwitchError) -> Self`
devflow-docs/inception/design-review-raw/codex-detail.md:558:   264	- `pub fn captured_targets(&self) -> Vec<ContextTarget>`
devflow-docs/inception/design-review-raw/codex-detail.md:559:   265	**Dependencies**: `ContextSessionPort` trait 구현
devflow-docs/inception/design-review-raw/codex-detail.md:566:   272	- `pub async fn switch_context(&mut self, target: ContextTarget) -> Result<(), SwitchError>` — ContextSwitcher 위임 + AppEvent::ContextChanged 디스패치
devflow-docs/inception/design-review-raw/codex-detail.md:572:   278	- `pub fn spawn(epoch: Epoch, cancel: CancellationToken, fut: impl Future<Output = AppEvent>) -> JoinHandle<()>` — 기존 `tokio::spawn` 호출부 일괄 교체
devflow-docs/inception/design-review-raw/codex-detail.md:573:   279	- 내부: `tokio::select! { _ = cancel.cancelled() => {}, ev = fut => app_tx.send(VersionedEvent::new(ev, epoch)) }`
devflow-docs/inception/design-review-raw/codex-detail.md:580:   286	**Dependencies (추가)**: `VersionedEvent`, `ContextTarget` (`Action::SwitchContext`, `Action::SwitchBack` variant 추가)
devflow-docs/inception/design-review-raw/codex-detail.md:584:   290	**Dependencies (추가)**: `VersionedEvent`, `ContextTarget`
devflow-docs/inception/design-review-raw/codex-detail.md:591:   297	- 충돌 시 `ContextTargetResolver`의 `Ambiguous` → 후보 출력 + 재선택 안내
devflow-docs/inception/design-review-raw/codex-detail.md:593:   299	**Dependencies (추가)**: `ContextTargetResolver`
devflow-docs/inception/design-review-raw/codex-detail.md:608:   314	- `pub fn store_rescoped(&self, target: &ContextTarget, token: Token) -> Result<(), io::Error>`
devflow-docs/inception/design-review-raw/codex-detail.md:609:   315	- `pub fn lookup(&self, target: &ContextTarget) -> Option<Token>`
devflow-docs/inception/design-review-raw/codex-detail.md:611:   317	**Dependencies**: 기존 + `ContextTarget`
devflow-docs/inception/design-review-raw/codex-detail.md:618:   324	**Dependencies (추가)**: `ContextTarget`
devflow-docs/inception/design-review-raw/codex-detail.md:634:   340	   StateMachine  Registry  ContextSessionPort  EndpointCatalog  HistoryStore
devflow-docs/inception/design-review-raw/codex-detail.md:644:   350	  Resolver (ContextTargetResolver) ── used by CommandParser, ContextPicker, Project Module
devflow-docs/inception/design-review-raw/codex-detail.md:687:     3	use std::sync::atomic::{AtomicBool, Ordering};
devflow-docs/inception/design-review-raw/codex-detail.md:947:   263	            .and_then(|v| v.to_str().ok())
devflow-docs/inception/design-review-raw/codex-detail.md:1050:    42	/// On Unix, creates the file with 0o600 permissions atomically (no TOCTOU window).
devflow-docs/inception/design-review-raw/codex-detail.md:1165:   157	#[cfg(test)]
devflow-docs/inception/design-review-raw/codex-detail.md:1166:   158	mod tests {
devflow-docs/inception/design-review-raw/codex-detail.md:1174:   166	            id: "tok-test-123".to_string(),
devflow-docs/inception/design-review-raw/codex-detail.md:1205:   197	    #[test]
devflow-docs/inception/design-review-raw/codex-detail.md:1206:   198	    fn test_compute_cloud_key_deterministic() {
devflow-docs/inception/design-review-raw/codex-detail.md:1213:   205	    #[test]
devflow-docs/inception/design-review-raw/codex-detail.md:1214:   206	    fn test_compute_cloud_key_different_inputs() {
devflow-docs/inception/design-review-raw/codex-detail.md:1220:   212	    #[test]
devflow-docs/inception/design-review-raw/codex-detail.md:1221:   213	    fn test_token_scope_cache_key() {
devflow-docs/inception/design-review-raw/codex-detail.md:1230:   222	    #[test]
devflow-docs/inception/design-review-raw/codex-detail.md:1231:   223	    fn test_cache_key_sanitizes_path_traversal() {
devflow-docs/inception/design-review-raw/codex-detail.md:1241:   233	    #[test]
devflow-docs/inception/design-review-raw/codex-detail.md:1242:   234	    fn test_cache_key_handles_underscore_in_name() {
devflow-docs/inception/design-review-raw/codex-detail.md:1251:   243	    #[test]
devflow-docs/inception/design-review-raw/codex-detail.md:1252:   244	    fn test_save_and_load_scoped_token() {
devflow-docs/inception/design-review-raw/codex-detail.md:1263:   255	        assert_eq!(loaded_token.id, "tok-test-123");
devflow-docs/inception/design-review-raw/codex-detail.md:1266:   258	    #[test]
devflow-docs/inception/design-review-raw/codex-detail.md:1267:   259	    fn test_save_multiple_scopes() {
devflow-docs/inception/design-review-raw/codex-detail.md:1294:   286	    #[test]
devflow-docs/inception/design-review-raw/codex-detail.md:1295:   287	    fn test_load_expired_token_deleted() {
devflow-docs/inception/design-review-raw/codex-detail.md:1308:   300	    #[test]
devflow-docs/inception/design-review-raw/codex-detail.md:1309:   301	    fn test_load_nonexistent_dir_returns_empty() {
devflow-docs/inception/design-review-raw/codex-detail.md:1310:   302	        let path = PathBuf::from("/tmp/nexttui-test-nonexistent-dir");
devflow-docs/inception/design-review-raw/codex-detail.md:1315:   307	    #[test]
devflow-docs/inception/design-review-raw/codex-detail.md:1316:   308	    fn test_load_corrupt_file_skipped() {
devflow-docs/inception/design-review-raw/codex-detail.md:1327:   319	    #[test]
devflow-docs/inception/design-review-raw/codex-detail.md:1328:   320	    fn test_save_sets_permissions_0o600() {
devflow-docs/inception/design-review-raw/codex-detail.md:1435:   422	            .and_then(|c| {
devflow-docs/inception/design-review-raw/codex-detail.md:1472:   459	#[cfg(test)]
devflow-docs/inception/design-review-raw/codex-detail.md:1473:   460	mod tests {
devflow-docs/inception/design-review-raw/codex-detail.md:1536:   523	    #[test]
devflow-docs/inception/design-review-raw/codex-detail.md:1537:   524	    fn test_build_auth_body_password() {
devflow-docs/inception/design-review-raw/codex-detail.md:1550:   537	    #[test]
devflow-docs/inception/design-review-raw/codex-detail.md:1551:   538	    fn test_build_auth_body_app_credential() {
devflow-docs/inception/design-review-raw/codex-detail.md:1566:   553	    #[test]
devflow-docs/inception/design-review-raw/codex-detail.md:1567:   554	    fn test_parse_token_from_keystone_response() {
devflow-docs/inception/design-review-raw/codex-detail.md:1586:   573	    #[test]
devflow-docs/inception/design-review-raw/codex-detail.md:1587:   574	    fn test_parse_token_no_catalog() {
devflow-docs/inception/design-review-raw/codex-detail.md:1602:   589	    #[test]
devflow-docs/inception/design-review-raw/codex-detail.md:1603:   590	    fn test_parse_interface() {
devflow-docs/inception/design-review-raw/codex-detail.md:1610:   597	    #[tokio::test]
devflow-docs/inception/design-review-raw/codex-detail.md:1611:   598	    async fn test_get_endpoint_from_token() {
devflow-docs/inception/design-review-raw/codex-detail.md:1639:   626	    #[tokio::test]
devflow-docs/inception/design-review-raw/codex-detail.md:1640:   627	    async fn test_has_role() {
devflow-docs/inception/design-review-raw/codex-detail.md:1655:   642	    #[tokio::test]
devflow-docs/inception/design-review-raw/codex-detail.md:1656:   643	    async fn test_authenticate_request_injects_token() {
devflow-docs/inception/design-review-raw/codex-detail.md:1677:   664	    #[tokio::test]
devflow-docs/inception/design-review-raw/codex-detail.md:1678:   665	    async fn test_get_token_info_not_authenticated() {
devflow-docs/inception/design-review-raw/codex-detail.md:1684:   671	    #[tokio::test]
devflow-docs/inception/design-review-raw/codex-detail.md:1685:   672	    async fn test_get_catalog() {
devflow-docs/inception/design-review-raw/codex-detail.md:1700:   687	    #[tokio::test]
devflow-docs/inception/design-review-raw/codex-detail.md:1701:   688	    async fn test_refresh_loop_idempotent() {
devflow-docs/inception/design-review-raw/codex-detail.md:1709:   696	        // Second call should be no-op (tested via AtomicBool flag)
devflow-docs/inception/design-review-raw/codex-detail.md:1867:   148	                    if let Some(msg) = inner.get("message").and_then(|m| m.as_str()) {
devflow-docs/inception/design-review-raw/codex-detail.md:1875:   156	                    if let Some(msg) = v.get("message").and_then(|m| m.as_str()) {
devflow-docs/inception/design-review-raw/codex-detail.md:1916:   197	#[cfg(test)]
devflow-docs/inception/design-review-raw/codex-detail.md:1917:   198	mod tests {
devflow-docs/inception/design-review-raw/codex-detail.md:1929:   210	    #[tokio::test]
devflow-docs/inception/design-review-raw/codex-detail.md:1930:   211	    async fn test_check_status_success() {
devflow-docs/inception/design-review-raw/codex-detail.md:1936:   217	    #[tokio::test]
devflow-docs/inception/design-review-raw/codex-detail.md:1937:   218	    async fn test_check_status_401() {
devflow-docs/inception/design-review-raw/codex-detail.md:1943:   224	    #[tokio::test]
devflow-docs/inception/design-review-raw/codex-detail.md:1944:   225	    async fn test_check_status_403() {
devflow-docs/inception/design-review-raw/codex-detail.md:1950:   231	    #[tokio::test]
devflow-docs/inception/design-review-raw/codex-detail.md:1951:   232	    async fn test_check_status_404() {
devflow-docs/inception/design-review-raw/codex-detail.md:1957:   238	    #[tokio::test]
devflow-docs/inception/design-review-raw/codex-detail.md:1958:   239	    async fn test_check_status_409() {
devflow-docs/inception/design-review-raw/codex-detail.md:1964:   245	    #[tokio::test]
devflow-docs/inception/design-review-raw/codex-detail.md:1965:   246	    async fn test_check_status_400() {
devflow-docs/inception/design-review-raw/codex-detail.md:1971:   252	    #[tokio::test]
devflow-docs/inception/design-review-raw/codex-detail.md:1972:   253	    async fn test_check_status_429() {
devflow-docs/inception/design-review-raw/codex-detail.md:1978:   259	    #[tokio::test]
devflow-docs/inception/design-review-raw/codex-detail.md:1979:   260	    async fn test_check_status_503() {
devflow-docs/inception/design-review-raw/codex-detail.md:2024:    41	#[cfg(test)]
devflow-docs/inception/design-review-raw/codex-detail.md:2025:    42	mod tests {
devflow-docs/inception/design-review-raw/codex-detail.md:2028:    45	    #[test]
devflow-docs/inception/design-review-raw/codex-detail.md:2029:    46	    fn test_input_mode_default() {
devflow-docs/inception/design-review-raw/codex-detail.md:2034:    51	    #[test]
devflow-docs/inception/design-review-raw/codex-detail.md:2035:    52	    fn test_component_default_refresh_action_is_none() {
devflow-docs/inception/design-review-raw/codex-detail.md:2046:    63	    #[test]
devflow-docs/inception/design-review-raw/codex-detail.md:2047:    64	    fn test_component_default_has_transitional_is_false() {
devflow-docs/inception/design-review-raw/codex-detail.md:2058:    75	    #[test]
devflow-docs/inception/design-review-raw/codex-detail.md:2059:    76	    fn test_component_default_is_modal_is_false() {
devflow-docs/inception/design-review-raw/codex-detail.md:2070:    87	    #[test]
devflow-docs/inception/design-review-raw/codex-detail.md:2071:    88	    fn test_component_default_layout_hint_is_default() {
devflow-docs/inception/design-review-raw/codex-detail.md:2083:   100	    #[test]
devflow-docs/inception/design-review-raw/codex-detail.md:2084:   101	    fn test_component_set_admin_default() {
devflow-docs/inception/design-review-raw/codex-detail.md:2218:   111	#[cfg(test)]
devflow-docs/inception/design-review-raw/codex-detail.md:2219:   112	mod tests {
devflow-docs/inception/design-review-raw/codex-detail.md:2222:   115	    #[test]
devflow-docs/inception/design-review-raw/codex-detail.md:2223:   116	    fn test_action_variants_exist() {
devflow-docs/inception/design-review-raw/codex-detail.md:2231:   124	                name: "test".into(),
devflow-docs/inception/design-review-raw/codex-detail.md:2255:   148	                name: "m1.test".into(),
devflow-docs/inception/design-review-raw/codex-detail.md:2273:   166	    #[test]
devflow-docs/inception/design-review-raw/codex-detail.md:2274:   167	    fn test_volume_fip_action_variants_exist() {
devflow-docs/inception/design-review-raw/codex-detail.md:2287:   180	    #[test]
devflow-docs/inception/design-review-raw/codex-detail.md:2288:   181	    fn test_resize_action_variants_exist() {
devflow-docs/inception/design-review-raw/codex-detail.md:2297:   190	    #[test]
devflow-docs/inception/design-review-raw/codex-detail.md:2298:   191	    fn test_usage_action_variant_exists() {
devflow-docs/inception/design-review-raw/codex-detail.md:2312:   205	    #[test]
devflow-docs/inception/design-review-raw/codex-detail.md:2313:   206	    fn test_migration_action_variants_exist() {
devflow-docs/inception/design-review-raw/codex-detail.md:2438:   108	#[cfg(test)]
devflow-docs/inception/design-review-raw/codex-detail.md:2439:   109	mod tests {
devflow-docs/inception/design-review-raw/codex-detail.md:2442:   112	    #[test]
devflow-docs/inception/design-review-raw/codex-detail.md:2443:   113	    fn test_app_event_variants_exist() {
devflow-docs/inception/design-review-raw/codex-detail.md:2470:   140	    #[test]
devflow-docs/inception/design-review-raw/codex-detail.md:2471:   141	    fn test_token_refreshed_carries_roles() {
devflow-docs/inception/design-review-raw/codex-detail.md:2484:   154	    #[test]
devflow-docs/inception/design-review-raw/codex-detail.md:2485:   155	    fn test_migration_event_variants_exist() {
devflow-docs/inception/design-review-raw/codex-detail.md:2514:   184	    #[test]
devflow-docs/inception/design-review-raw/codex-detail.md:2515:   185	    fn test_resize_event_variants_exist() {
devflow-docs/inception/design-review-raw/codex-detail.md:2524:   194	    #[test]
devflow-docs/inception/design-review-raw/codex-detail.md:2525:   195	    fn test_server_status_polled_event() {
devflow-docs/inception/design-review-raw/codex-detail.md:2529:   199	            name: "test".into(),
devflow-docs/inception/design-review-raw/codex-detail.md:2559:   229	    #[test]
devflow-docs/inception/design-review-raw/codex-detail.md:2560:   230	    fn test_volume_fip_event_variants_exist() {
devflow-docs/inception/design-review-raw/codex-detail.md:2604:   274	    #[test]
devflow-docs/inception/design-review-raw/codex-detail.md:2605:   275	    fn test_usage_loaded_event() {
devflow-docs/inception/design-review-raw/codex-detail.md:2625:   295	    #[test]
devflow-docs/inception/design-review-raw/codex-detail.md:2626:   296	    fn test_permission_denied_event() {
devflow-docs/inception/design-review-raw/codex-detail.md:2645:     6	use std::sync::atomic::{AtomicBool, Ordering};
devflow-docs/inception/design-review-raw/codex-detail.md:3006:     3	use std::sync::atomic::{AtomicBool, Ordering};
devflow-docs/inception/design-review-raw/codex-detail.md:3144:   141	    /// Register a domain module component for a given route (test use only).
devflow-docs/inception/design-review-raw/codex-detail.md:3145:   142	    #[cfg(test)]
devflow-docs/inception/design-review-raw/codex-detail.md:3150:   147	    /// Inject an audit logger for testing.
devflow-docs/inception/design-review-raw/codex-detail.md:3151:   148	    #[cfg(test)]
devflow-docs/inception/design-review-raw/codex-detail.md:3478:    51	#[cfg(test)]
devflow-docs/inception/design-review-raw/codex-detail.md:3479:    52	mod tests {
devflow-docs/inception/design-review-raw/codex-detail.md:3482:    55	    #[test]
devflow-docs/inception/design-review-raw/codex-detail.md:3483:    56	    fn test_api_error_display() {
devflow-docs/inception/design-review-raw/codex-detail.md:3966:   512	        #[cfg(test)]
devflow-docs/inception/design-review-raw/codex-detail.md:3968:   514	            // In tests, do not create audit logger by default
devflow-docs/inception/design-review-raw/codex-detail.md:3971:   517	        #[cfg(not(test))]
devflow-docs/inception/design-review-raw/codex-detail.md:4329:   875	#[cfg(test)]
devflow-docs/inception/design-review-raw/codex-detail.md:4330:   876	mod tests {
devflow-docs/inception/design-review-raw/codex-detail.md:4822:   823	                        "completed" | "confirmed" | "error" | "cancelled"
devflow-docs/inception/design-review-raw/codex-detail.md:4856:   857	#[cfg(test)]
devflow-docs/inception/design-review-raw/codex-detail.md:4857:   858	mod tests {
devflow-docs/inception/design-review-raw/codex-detail.md:4860:   861	    #[test]
devflow-docs/inception/design-review-raw/codex-detail.md:4861:   862	    fn test_action_to_kind_cud_actions() {
devflow-docs/inception/design-review-raw/codex-detail.md:4922:   923	    #[test]
devflow-docs/inception/design-review-raw/codex-detail.md:4923:   924	    fn test_action_to_kind_resize_actions() {
devflow-docs/inception/design-review-raw/codex-detail.md:4939:   940	    #[test]
devflow-docs/inception/design-review-raw/codex-detail.md:4940:   941	    fn test_permission_denied_event_on_guard_failure() {
devflow-docs/inception/design-review-raw/codex-detail.md:4949:   950	    #[test]
devflow-docs/inception/design-review-raw/codex-detail.md:4950:   951	    fn test_resize_actions_trigger_status_polling() {
devflow-docs/inception/design-review-raw/codex-detail.md:4965:   966	    #[test]
devflow-docs/inception/design-review-raw/codex-detail.md:4966:   967	    fn test_polling_dedup_guard() {
devflow-docs/inception/design-review-raw/codex-detail.md:5276:   293	#[cfg(test)]
devflow-docs/inception/design-review-raw/codex-detail.md:5277:   294	mod tests {
devflow-docs/inception/design-review-raw/codex-detail.md:5286:   303	    #[test]
devflow-docs/inception/design-review-raw/codex-detail.md:5287:   304	    fn test_parse_route_direct() {
devflow-docs/inception/design-review-raw/codex-detail.md:5293:   310	    #[test]
devflow-docs/inception/design-review-raw/codex-detail.md:5294:   311	    fn test_parse_abbreviation() {
devflow-docs/inception/design-review-raw/codex-detail.md:5301:   318	    #[test]
devflow-docs/inception/design-review-raw/codex-detail.md:5302:   319	    fn test_parse_system_commands() {
devflow-docs/inception/design-review-raw/codex-detail.md:5401:    94	#[cfg(test)]
devflow-docs/inception/design-review-raw/codex-detail.md:5402:    95	mod tests {
devflow-docs/inception/design-review-raw/codex-detail.md:5418:   111	    #[test]
devflow-docs/inception/design-review-raw/codex-detail.md:5419:   112	    fn test_status_info_new_fields() {
devflow-docs/inception/design-review-raw/codex-detail.md:5428:   121	    #[test]
devflow-docs/inception/design-review-raw/codex-detail.md:5429:   122	    fn test_status_info_left_text_with_count() {
devflow-docs/inception/design-review-raw/codex-detail.md:5434:   127	    #[test]
devflow-docs/inception/design-review-raw/codex-detail.md:5435:   128	    fn test_status_info_left_text_without_count() {
devflow-docs/inception/design-review-raw/codex-detail.md:5446:   139	    #[test]
devflow-docs/inception/design-review-raw/codex-detail.md:5447:   140	    fn test_error_badge_count_zero_no_badge_text() {
devflow-docs/inception/design-review-raw/codex-detail.md:5454:   147	    #[test]
devflow-docs/inception/design-review-raw/codex-detail.md:5455:   148	    fn test_error_badge_count_positive_shows_badge() {
devflow-docs/inception/design-review-raw/codex-detail.md:5459:   152	        // We test the render output by checking the spans.
devflow-docs/inception/design-review-raw/codex-detail.md:5484:   177	    #[test]
devflow-docs/inception/design-review-raw/codex-detail.md:5485:   178	    fn test_status_bar_key_hint_integration() {
devflow-docs/inception/design-review-raw/codex-detail.md:5625:   131	                    .and_then(|v| match v {
devflow-docs/inception/design-review-raw/codex-detail.md:5632:   138	                    .and_then(|v| match v {
devflow-docs/inception/design-review-raw/codex-detail.md:5640:   146	                    .and_then(|v| match v {
devflow-docs/inception/design-review-raw/codex-detail.md:5647:   153	                    .and_then(|v| match v {
devflow-docs/inception/design-review-raw/codex-detail.md:5763:   269	#[cfg(test)]
devflow-docs/inception/design-review-raw/codex-detail.md:5764:   270	mod tests {
devflow-docs/inception/design-review-raw/codex-detail.md:5779:   285	    #[test] fn test_initial_state() { let (tx, _) = mpsc::unbounded_channel(); let m = ProjectModule::new(tx); assert_eq!(*m.view_state(), ViewState::List); }
devflow-docs/inception/design-review-raw/codex-detail.md:5780:   286	    #[test] fn test_nav() { let (mut m, _) = setup(); m.handle_key(key(KeyCode::Char('j'))); assert_eq!(m.selected_index(), 1); }
devflow-docs/inception/design-review-raw/codex-detail.md:5781:   287	    #[test] fn test_enter_detail() { let (mut m, _) = setup(); m.handle_key(key(KeyCode::Enter)); assert_eq!(*m.view_state(), ViewState::Detail("p1".into())); }
devflow-docs/inception/design-review-raw/codex-detail.md:5782:   288	    #[test] fn test_esc_to_list() { let (mut m, _) = setup(); m.handle_key(key(KeyCode::Enter)); m.handle_key(key(KeyCode::Esc)); assert_eq!(*m.view_state(), ViewState::List); }
devflow-docs/inception/design-review-raw/codex-detail.md:5783:   289	    #[test] fn test_create() { let (mut m, _) = setup(); m.handle_key(key(KeyCode::Char('c'))); assert_eq!(*m.view_state(), ViewState::Create); assert!(m.form.is_some()); }
devflow-docs/inception/design-review-raw/codex-detail.md:5784:   290	    #[test] fn test_delete_confirm() { let (mut m, _) = setup(); m.handle_key(key(KeyCode::Char('d'))); assert!(m.confirm.is_active()); }
devflow-docs/inception/design-review-raw/codex-detail.md:5785:   291	    #[test] fn test_confirm_delete() {
devflow-docs/inception/design-review-raw/codex-detail.md:5792:   298	    #[test] fn test_refresh() { let (mut m, _) = setup(); assert!(matches!(m.handle_key(key(KeyCode::Char('r'))), Some(Action::FetchProjects))); }
devflow-docs/inception/design-review-raw/codex-detail.md:5793:   299	    #[test] fn test_event_loaded() {
devflow-docs/inception/design-review-raw/codex-detail.md:5798:   304	    #[test] fn test_event_created() {
devflow-docs/inception/design-review-raw/codex-detail.md:5804:   310	    #[test] fn test_event_deleted() {
devflow-docs/inception/design-review-raw/codex-detail.md:5810:   316	    // -- Form integration tests -----------------------------------------------
devflow-docs/inception/design-review-raw/codex-detail.md:5812:   318	    #[test]
devflow-docs/inception/design-review-raw/codex-detail.md:5813:   319	    fn test_create_form_cancel_returns_to_list() {
devflow-docs/inception/design-review-raw/codex-detail.md:6048:   230	#[cfg(test)]
devflow-docs/inception/design-review-raw/codex-detail.md:6049:   231	mod tests {
devflow-docs/inception/design-review-raw/codex-detail.md:6056:   238	    #[test]
devflow-docs/inception/design-review-raw/codex-detail.md:6057:   239	    fn test_yes_no_confirm() {
devflow-docs/inception/design-review-raw/codex-detail.md:6065:   247	    #[test]
devflow-docs/inception/design-review-raw/codex-detail.md:6066:   248	    fn test_yes_no_cancel() {
devflow-docs/inception/design-review-raw/codex-detail.md:6072:   254	    #[test]
devflow-docs/inception/design-review-raw/codex-detail.md:6073:   255	    fn test_type_to_confirm_success() {
devflow-docs/inception/design-review-raw/codex-detail.md:6083:   265	    #[test]
devflow-docs/inception/design-review-raw/codex-detail.md:6084:   266	    fn test_type_to_confirm_wrong_name() {
devflow-docs/inception/design-review-raw/codex-detail.md:6094:   276	    #[test]
devflow-docs/inception/design-review-raw/codex-detail.md:6095:   277	    fn test_backspace_in_type_to_confirm() {
devflow-docs/inception/design-review-raw/codex-detail.md:6106:   288	    #[test]
devflow-docs/inception/design-review-raw/codex-detail.md:6107:   289	    fn test_inactive_ignores_keys() {
devflow-docs/inception/design-review-raw/codex-detail.md:6108:   290	        let mut dialog = ConfirmDialog::yes_no("test");
devflow-docs/inception/design-review-raw/codex-detail.md:6114:   296	    // --- detail_lines tests ---
devflow-docs/inception/design-review-raw/codex-detail.md:6116:   298	    #[test]
devflow-docs/inception/design-review-raw/codex-detail.md:6117:   299	    fn test_yes_no_with_details_creates_dialog() {
devflow-docs/inception/design-review-raw/codex-detail.md:6125:   307	    #[test]
devflow-docs/inception/design-review-raw/codex-detail.md:6126:   308	    fn test_type_to_confirm_with_details_creates_dialog() {
devflow-docs/inception/design-review-raw/codex-detail.md:6138:   320	    #[test]
devflow-docs/inception/design-review-raw/codex-detail.md:6182:    40	    /// Create registry from mock adapters (for testing).
devflow-docs/inception/design-review-raw/codex-detail.md:6183:    41	    #[cfg(test)]
devflow-docs/inception/design-review-raw/codex-detail.md:6196:    54	#[cfg(test)]
devflow-docs/inception/design-review-raw/codex-detail.md:6197:    55	mod tests {
devflow-docs/inception/design-review-raw/codex-detail.md:6200:    58	    #[test]
devflow-docs/inception/design-review-raw/codex-detail.md:6201:    59	    fn test_adapter_registry_mock_creation() {
devflow-docs/inception/design-review-raw/codex-detail.md:6210:    68	    #[test]
devflow-docs/inception/design-review-raw/codex-detail.md:6211:    69	    fn test_registry_adapters_are_send_sync() {
devflow-docs/inception/design-review-raw/codex-detail.md:6218:/bin/zsh -lc "rg -n \"invalidate_endpoint|BaseHttpClient|token_refresh|subscribe_token_refresh|CloudSwitched|SwitchCloud|ContextChanged|VersionedEvent|epoch|CancellationToken|cancelled\\(\" src | sed -n '1,260p'" in /Users/jay.ahn/projects/infra/nexttui/.worktrees/runtime-context-switch
devflow-docs/inception/design-review-raw/codex-detail.md:6220:src/app.rs:1078:    fn test_handle_token_refreshed_updates_rbac() {
devflow-docs/inception/design-review-raw/codex-detail.md:6225:src/event.rs:141:    fn test_token_refreshed_carries_roles() {
devflow-docs/inception/design-review-raw/codex-detail.md:6279:I confirmed current runtime wiring in `main.rs` and will now inspect that loop plus `port/mock.rs` to evaluate whether the proposed mock/test seams and epoch-event envelope are practically complete.
devflow-docs/inception/design-review-raw/codex-detail.md:7135:    61	#[cfg(test)]
devflow-docs/inception/design-review-raw/codex-detail.md:7136:    62	mod tests {
devflow-docs/inception/design-review-raw/codex-detail.md:7140:    66	    #[test] fn test_initial() { let m = ComputeServiceModule::new(); assert!(m.services().is_empty()); }
devflow-docs/inception/design-review-raw/codex-detail.md:7141:    67	    #[test] fn test_refresh() {
devflow-docs/inception/design-review-raw/codex-detail.md:7145:    71	    #[test] fn test_event_loaded() {
devflow-docs/inception/design-review-raw/codex-detail.md:7153:    79	    #[test]
devflow-docs/inception/design-review-raw/codex-detail.md:7154:    80	    fn test_help_hint() {
devflow-docs/inception/design-review-raw/codex-detail.md:7224:    62	#[cfg(test)]
devflow-docs/inception/design-review-raw/codex-detail.md:7225:    63	mod tests {
devflow-docs/inception/design-review-raw/codex-detail.md:7236:    74	    #[test] fn test_initial() { let m = AgentModule::new(); assert!(m.agents().is_empty()); }
devflow-docs/inception/design-review-raw/codex-detail.md:7237:    75	    #[test] fn test_nav() {
devflow-docs/inception/design-review-raw/codex-detail.md:7243:    81	    #[test] fn test_refresh() {
devflow-docs/inception/design-review-raw/codex-detail.md:7247:    85	    #[test] fn test_event_loaded() {
devflow-docs/inception/design-review-raw/codex-detail.md:7253:    91	    #[test]
devflow-docs/inception/design-review-raw/codex-detail.md:7254:    92	    fn test_help_hint() {
devflow-docs/inception/design-review-raw/codex-detail.md:7323:    61	#[cfg(test)]
devflow-docs/inception/design-review-raw/codex-detail.md:7324:    62	mod tests {
devflow-docs/inception/design-review-raw/codex-detail.md:7328:    66	    #[test] fn test_initial() { let m = AggregateModule::new(); assert!(m.aggregates().is_empty()); }
devflow-docs/inception/design-review-raw/codex-detail.md:7329:    67	    #[test] fn test_refresh() {
devflow-docs/inception/design-review-raw/codex-detail.md:7333:    71	    #[test] fn test_event_loaded() {
devflow-docs/inception/design-review-raw/codex-detail.md:7341:    79	    #[test]
devflow-docs/inception/design-review-raw/codex-detail.md:7342:    80	    fn test_help_hint() {
devflow-docs/inception/design-review-raw/codex-detail.md:7645:src/input/keymap.rs:75:            AppMode::Command => "Enter:run  Tab:complete  Up/Down:history  Esc:cancel".into(),
devflow-docs/inception/design-review-raw/codex-detail.md:7686:   920	        let config = test_config();
devflow-docs/inception/design-review-raw/codex-detail.md:7690:   924	    fn test_config() -> Config {
devflow-docs/inception/design-review-raw/codex-detail.md:7696:   930	            "clouds:\n  test:\n    auth:\n      auth_url: https://keystone/v3\n      username: admin\n      password: secret\n",
devflow-docs/inception/design-review-raw/codex-detail.md:7702:   936	    #[test]
devflow-docs/inception/design-review-raw/codex-detail.md:7703:   937	    fn test_app_global_key_colon() {
devflow-docs/inception/design-review-raw/codex-detail.md:7710:   944	    #[test]
devflow-docs/inception/design-review-raw/codex-detail.md:7711:   945	    fn test_app_global_key_slash_does_not_enter_search() {
devflow-docs/inception/design-review-raw/codex-detail.md:7719:   953	    #[test]
devflow-docs/inception/design-review-raw/codex-detail.md:7720:   954	    fn test_app_global_key_tab_focus_toggle() {
devflow-docs/inception/design-review-raw/codex-detail.md:7729:   963	    #[test]
devflow-docs/inception/design-review-raw/codex-detail.md:7730:   964	    fn test_app_global_key_q() {
devflow-docs/inception/design-review-raw/codex-detail.md:7737:   971	    #[test]
devflow-docs/inception/design-review-raw/codex-detail.md:7738:   972	    fn test_app_esc_to_normal() {
devflow-docs/inception/design-review-raw/codex-detail.md:7745:   979	    #[test]
devflow-docs/inception/design-review-raw/codex-detail.md:7746:   980	    fn test_app_esc_normal_back() {
devflow-docs/inception/design-review-raw/codex-detail.md:7754:   988	    #[test]
devflow-docs/inception/design-review-raw/codex-detail.md:7755:   989	    fn test_app_delegate_to_component() {
devflow-docs/inception/design-review-raw/codex-detail.md:7767:  1001	    #[test]
devflow-docs/inception/design-review-raw/codex-detail.md:7768:  1002	    fn test_dispatch_cud_action_adds_progress_toast() {
devflow-docs/inception/design-review-raw/codex-detail.md:7784:  1018	    #[test]
devflow-docs/inception/design-review-raw/codex-detail.md:7785:  1019	    fn test_handle_event_server_created_adds_toast() {
devflow-docs/inception/design-review-raw/codex-detail.md:7799:  1033	    #[test]
devflow-docs/inception/design-review-raw/codex-detail.md:7800:  1034	    fn test_handle_event_api_error_adds_toast() {
devflow-docs/inception/design-review-raw/codex-detail.md:7812:  1046	    #[test]
devflow-docs/inception/design-review-raw/codex-detail.md:7813:  1047	    fn test_app_rbac_is_admin() {
devflow-docs/inception/design-review-raw/codex-detail.md:7818:  1052	    #[test]
devflow-docs/inception/design-review-raw/codex-detail.md:7819:  1053	    fn test_app_broadcast_admin() {
devflow-docs/inception/design-review-raw/codex-detail.md:7825:  1059	    #[test]
devflow-docs/inception/design-review-raw/codex-detail.md:7826:  1060	    fn test_app_sidebar_uses_rbac() {
devflow-docs/inception/design-review-raw/codex-detail.md:7829:  1063	        let config = test_config();
devflow-docs/inception/design-review-raw/codex-detail.md:7843:  1077	    #[test]
devflow-docs/inception/design-review-raw/codex-detail.md:7844:  1078	    fn test_handle_token_refreshed_updates_rbac() {
devflow-docs/inception/design-review-raw/codex-detail.md:7852:  1086	    #[test]
devflow-docs/inception/design-review-raw/codex-detail.md:7853:  1087	    fn test_dispatch_migration_action_adds_progress_toast() {
devflow-docs/inception/design-review-raw/codex-detail.md:7862:  1096	    #[test]
devflow-docs/inception/design-review-raw/codex-detail.md:7863:  1097	    fn test_handle_cold_migrated_event_toast_and_refresh() {
devflow-docs/inception/design-review-raw/codex-detail.md:7865:  1099	        let config = test_config();
devflow-docs/inception/design-review-raw/codex-detail.md:7878:  1112	    #[test]
devflow-docs/inception/design-review-raw/codex-detail.md:7879:  1113	    fn test_handle_evacuated_event_adds_toast() {
devflow-docs/inception/design-review-raw/codex-detail.md:7886:  1120	    #[test]
devflow-docs/inception/design-review-raw/codex-detail.md:8076:    47	#[cfg(test)]
devflow-docs/inception/design-review-raw/codex-detail.md:8077:    48	mod tests {
devflow-docs/inception/design-review-raw/codex-detail.md:8080:    51	    #[test]
devflow-docs/inception/design-review-raw/codex-detail.md:8081:    52	    fn test_app_error_display() {
devflow-docs/inception/design-review-raw/codex-detail.md:8186:    68	        .and_then(|l| extract_marker_from_url(&l.href))
devflow-docs/inception/design-review-raw/codex-detail.md:8194:    76	        .and_then(|query| {
devflow-docs/inception/design-review-raw/codex-detail.md:8233:/bin/zsh -lc "rg -n \"enum ProjectRef|struct ProjectRef|type ProjectRef|SessionHandle|ServiceCatalog|ContextTarget\" src devflow-docs/inception/application-design.md | sed -n '1,260p'" in /Users/jay.ahn/projects/infra/nexttui/.worktrees/runtime-context-switch
devflow-docs/inception/design-review-raw/codex-detail.md:8235:devflow-docs/inception/application-design.md:31:| `ContextTargetResolver` | name/uuid/cloud-prefix → ContextTarget 변환, 충돌 disambiguation. 명령·피커·모듈 액션 공유 | Service | PR1 |
devflow-docs/inception/design-review-raw/codex-detail.md:8236:devflow-docs/inception/application-design.md:46:| `CommandParser` (src/input/command.rs) | `:switch-project <name|uuid|cloud/project>`, `:switch-cloud`, `:switch-back` 등록 + tab 자동완성. `ContextTargetResolver` 사용 | Controller | PR3 |
devflow-docs/inception/design-review-raw/codex-detail.md:8237:devflow-docs/inception/application-design.md:66:| **PR1 (safety infra + switch core)** | ContextEpoch, CancellationRegistry, VersionedEvent, AppEvent::ContextChanged, SwitchStateMachine, ContextSwitcher, ContextSessionPort, ScopedAuthSession, KeystoneRescopeAdapter, EndpointCatalogInvalidator, ContextTargetResolver, ContextHistoryStore, App 통합, Worker 시그니처 개정, Action/AppEvent envelope, TokenCacheStore 확장, MockContextSession | — | 없음 (인프라만) |
devflow-docs/inception/design-review-raw/codex-detail.md:8238:devflow-docs/inception/application-design.md:80:| NFR-4 UX 일관성 | ContextPicker (기존 SelectPopup 재사용), CommandParser 확장 (기존 패턴), ContextTargetResolver (단일 disambiguation 로직) |
devflow-docs/inception/design-review-raw/codex-detail.md:8239:devflow-docs/inception/application-design.md:90:| ContextTargetResolver 추가 | Codex | 명령·피커·모듈 액션이 같은 disambiguation 로직 필요 (3중 구현 회피) |
devflow-docs/inception/design-review-raw/codex-detail.md:8240:devflow-docs/inception/application-design.md:115:pub struct ContextTarget {
devflow-docs/inception/design-review-raw/codex-detail.md:8241:devflow-docs/inception/application-design.md:123:    pub target: ContextTarget,
devflow-docs/inception/design-review-raw/codex-detail.md:8242:devflow-docs/inception/application-design.md:136:    Ambiguous { candidates: Vec<ContextTarget> },
devflow-docs/inception/design-review-raw/codex-detail.md:8243:devflow-docs/inception/application-design.md:173:- `AppEvent::ContextChanged { target: ContextTarget, epoch: Epoch }`
devflow-docs/inception/design-review-raw/codex-detail.md:8244:devflow-docs/inception/application-design.md:174:**Dependencies**: `ContextTarget` (src/context.rs)
devflow-docs/inception/design-review-raw/codex-detail.md:8245:devflow-docs/inception/application-design.md:180:- `pub fn try_begin(&mut self, target: ContextTarget) -> Result<Epoch, SwitchError::InProgress>`
devflow-docs/inception/design-review-raw/codex-detail.md:8246:devflow-docs/inception/application-design.md:189:- `pub async fn switch(&self, target: ContextTarget) -> Result<ContextSnapshot, SwitchError>`
devflow-docs/inception/design-review-raw/codex-detail.md:8247:devflow-docs/inception/application-design.md:200:    async fn begin(&self, target: &ContextTarget) -> Result<SessionHandle, SwitchError>;
devflow-docs/inception/design-review-raw/codex-detail.md:8248:devflow-docs/inception/application-design.md:201:    async fn commit(&self, handle: SessionHandle) -> Result<ContextSnapshot, SwitchError>;
devflow-docs/inception/design-review-raw/codex-detail.md:8249:devflow-docs/inception/application-design.md:202:    async fn rollback(&self, handle: SessionHandle);
devflow-docs/inception/design-review-raw/codex-detail.md:8250:devflow-docs/inception/application-design.md:206:**Dependencies**: `ContextTarget`, `ContextSnapshot`, `SwitchError`
devflow-docs/inception/design-review-raw/codex-detail.md:8251:devflow-docs/inception/application-design.md:216:- `pub async fn rescope(&self, current_token: &Token, target: &ContextTarget) -> Result<Token, SwitchError>`
devflow-docs/inception/design-review-raw/codex-detail.md:8252:devflow-docs/inception/application-design.md:224:- `pub async fn refresh_catalog(&self, token: &Token) -> Result<ServiceCatalog, SwitchError>`
devflow-docs/inception/design-review-raw/codex-detail.md:8253:devflow-docs/inception/application-design.md:227:### ContextTargetResolver (Service)
devflow-docs/inception/design-review-raw/codex-detail.md:8254:devflow-docs/inception/application-design.md:228:**Responsibility**: name/uuid/cloud-prefix → ContextTarget 변환 + 충돌 disambiguation.
devflow-docs/inception/design-review-raw/codex-detail.md:8255:devflow-docs/inception/application-design.md:230:- `pub fn resolve(&self, input: &str, current_cloud: &str) -> Result<ContextTarget, SwitchError>` — 단일 매치 시 OK, 충돌 시 `Ambiguous`, 미매치 시 `NotFound`
devflow-docs/inception/design-review-raw/codex-detail.md:8256:devflow-docs/inception/application-design.md:231:- `pub fn list_user_projects(&self) -> Vec<ContextTarget>` — 피커용 (`/v3/auth/projects` 결과 캐시)
devflow-docs/inception/design-review-raw/codex-detail.md:8257:devflow-docs/inception/application-design.md:254:- `pub fn open(&mut self, candidates: Vec<ContextTarget>, current: Option<&ContextTarget>)`
devflow-docs/inception/design-review-raw/codex-detail.md:8258:devflow-docs/inception/application-design.md:257:**Dependencies**: `SelectPopup` (src/ui/select_popup.rs 재사용), `ContextTarget`, `Theme`
devflow-docs/inception/design-review-raw/codex-detail.md:8259:devflow-docs/inception/application-design.md:264:- `pub fn captured_targets(&self) -> Vec<ContextTarget>`
devflow-docs/inception/design-review-raw/codex-detail.md:8260:devflow-docs/inception/application-design.md:272:- `pub async fn switch_context(&mut self, target: ContextTarget) -> Result<(), SwitchError>` — ContextSwitcher 위임 + AppEvent::ContextChanged 디스패치
devflow-docs/inception/design-review-raw/codex-detail.md:8261:devflow-docs/inception/application-design.md:286:**Dependencies (추가)**: `VersionedEvent`, `ContextTarget` (`Action::SwitchContext`, `Action::SwitchBack` variant 추가)
devflow-docs/inception/design-review-raw/codex-detail.md:8262:devflow-docs/inception/application-design.md:290:**Dependencies (추가)**: `VersionedEvent`, `ContextTarget`
devflow-docs/inception/design-review-raw/codex-detail.md:8263:devflow-docs/inception/application-design.md:297:- 충돌 시 `ContextTargetResolver`의 `Ambiguous` → 후보 출력 + 재선택 안내
devflow-docs/inception/design-review-raw/codex-detail.md:8264:devflow-docs/inception/application-design.md:299:**Dependencies (추가)**: `ContextTargetResolver`
devflow-docs/inception/design-review-raw/codex-detail.md:8265:devflow-docs/inception/application-design.md:314:- `pub fn store_rescoped(&self, target: &ContextTarget, token: Token) -> Result<(), io::Error>`
devflow-docs/inception/design-review-raw/codex-detail.md:8266:devflow-docs/inception/application-design.md:315:- `pub fn lookup(&self, target: &ContextTarget) -> Option<Token>`
devflow-docs/inception/design-review-raw/codex-detail.md:8267:devflow-docs/inception/application-design.md:317:**Dependencies**: 기존 + `ContextTarget`
devflow-docs/inception/design-review-raw/codex-detail.md:8268:devflow-docs/inception/application-design.md:324:**Dependencies (추가)**: `ContextTarget`
devflow-docs/inception/design-review-raw/codex-detail.md:8269:devflow-docs/inception/application-design.md:350:  Resolver (ContextTargetResolver) ── used by CommandParser, ContextPicker, Project Module
devflow-docs/inception/design-review-raw/codex-detail.md:8343:    56	#[cfg(test)]
devflow-docs/inception/design-review-raw/codex-detail.md:8344:    57	mod tests {
devflow-docs/inception/design-review-raw/codex-detail.md:8347:    60	    #[test]
devflow-docs/inception/design-review-raw/codex-detail.md:8348:    61	    fn test_project_deserialize() {
devflow-docs/inception/design-review-raw/codex-detail.md:8361:    74	    #[test]
devflow-docs/inception/design-review-raw/codex-detail.md:8362:    75	    fn test_user_deserialize() {
devflow-docs/inception/design-review-raw/codex-detail.md:8433:    54	        let region = self.region.read().ok().and_then(|r| r.clone());
devflow-docs/inception/design-review-raw/codex-detail.md:8479:   100	        self.region.read().ok().and_then(|r| r.clone())
devflow-docs/inception/design-review-raw/codex-detail.md:8575:1. **Atomic boundary is internally contradictory and cannot guarantee rollback safety.**  
devflow-docs/inception/design-review-raw/codex-detail.md:8576:What: DETAIL says `ContextSessionPort` is the atomic boundary, but the switch flow bypasses it (`state.begin -> ... -> token_cache.store -> state.commit`) and never calls `session.commit/rollback` ([application-design.md#L191](/Users/jay.ahn/projects/infra/nexttui/.worktrees/runtime-context-switch/devflow-docs/inception/application-design.md#L191), [application-design.md#L200](/Users/jay.ahn/projects/infra/nexttui/.worktrees/runtime-context-switch/devflow-docs/inception/application-design.md#L200)). Also `invalidate_all()` is infallible while `CatalogFailed` and mock invalidate-failure are defined ([application-design.md#L133](/Users/jay.ahn/projects/infra/nexttui/.worktrees/runtime-context-switch/devflow-docs/inception/application-design.md#L133), [application-design.md#L223](/Users/jay.ahn/projects/infra/nexttui/.worktrees/runtime-context-switch/devflow-docs/inception/application-design.md#L223), [application-design.md#L263](/Users/jay.ahn/projects/infra/nexttui/.worktrees/runtime-context-switch/devflow-docs/inception/application-design.md#L263)).  
devflow-docs/inception/design-review-raw/codex-detail.md:8577:Why: You cannot deterministically handle “rescope OK + invalidate fail + rollback,” so NFR-1 atomicity is not met ([requirements.md#L69](/Users/jay.ahn/projects/infra/nexttui/.worktrees/runtime-context-switch/devflow-docs/inception/requirements.md#L69)).  
devflow-docs/inception/design-review-raw/codex-detail.md:8578:Fix: Make `ContextSessionPort` own `rescope + catalog refresh + commit/rollback` end-to-end; `ContextSwitcher` should orchestrate state/epoch only.
devflow-docs/inception/design-review-raw/codex-detail.md:8580:2. **Port boundary is wrong: current auth port cannot perform scope switch commit.**  
devflow-docs/inception/design-review-raw/codex-detail.md:8585:3. **Epoch/cancellation enforcement point is underspecified and currently bypassable.**  
devflow-docs/inception/design-review-raw/codex-detail.md:8604:    Ambiguous { candidates: Vec<ContextTarget> },
devflow-docs/inception/design-review-raw/codex-detail.md:8612:    fn try_begin(&mut self, target: ContextTarget) -> Result<Epoch, SwitchError>; // not Result<_, SwitchError::InProgress>
devflow-docs/inception/design-review-raw/codex-detail.md:8613:    fn commit(&mut self, snapshot: ContextSnapshot);
devflow-docs/inception/design-review-raw/codex-detail.md:8620:pub trait ContextSessionPort: Send + Sync {
devflow-docs/inception/design-review-raw/codex-detail.md:8622:    async fn begin(&self, target: &ContextTarget, epoch: Epoch) -> Result<Self::Handle, SwitchError>;
devflow-docs/inception/design-review-raw/codex-detail.md:8623:    async fn rescope(&self, handle: &mut Self::Handle) -> Result<(), SwitchError>;
devflow-docs/inception/design-review-raw/codex-detail.md:8624:    async fn refresh_catalog(&self, handle: &mut Self::Handle) -> Result<(), SwitchError>;
devflow-docs/inception/design-review-raw/codex-detail.md:8625:    async fn commit(&self, handle: Self::Handle) -> Result<ContextSnapshot, SwitchError>;
devflow-docs/inception/design-review-raw/codex-detail.md:8626:    async fn rollback(&self, handle: Self::Handle) -> Result<(), SwitchError>;
devflow-docs/inception/design-review-raw/codex-detail.md:8631:pub async fn switch(&self, target: ContextTarget) -> Result<(Epoch, ContextSnapshot), SwitchError>;
devflow-docs/inception/design-review-raw/codex-detail.md:8638:    ContextChanged { target: ContextTarget },
devflow-docs/inception/design-review-raw/codex-detail.md:8645:pub async fn list_user_projects(&self) -> Result<Vec<ContextTarget>, SwitchError>;
devflow-docs/inception/design-review-raw/codex-detail.md:8658:- `ScopedAuthPort` (new port): required to mutate active scope without adapter downcast.
devflow-docs/inception/design-review-raw/codex-detail.md:8662:- Mock seam needs `begin_fail`, `commit_fail`, `rollback_called`, and ordered step assertions; current mock knobs are insufficient for NFR-3 failure-path tests.
devflow-docs/inception/design-review-raw/codex-detail.md:8668:**Brief reasoning:** DETAIL does not yet define an implementable atomic contract, violates intended port boundaries for auth scope mutation, and leaves stale-event isolation non-authoritative. These are correctness blockers, not polish issues.
devflow-docs/inception/design-review-raw/codex-detail.md:8673:1. **Atomic boundary is internally contradictory and cannot guarantee rollback safety.**  
devflow-docs/inception/design-review-raw/codex-detail.md:8674:What: DETAIL says `ContextSessionPort` is the atomic boundary, but the switch flow bypasses it (`state.begin -> ... -> token_cache.store -> state.commit`) and never calls `session.commit/rollback` ([application-design.md#L191](/Users/jay.ahn/projects/infra/nexttui/.worktrees/runtime-context-switch/devflow-docs/inception/application-design.md#L191), [application-design.md#L200](/Users/jay.ahn/projects/infra/nexttui/.worktrees/runtime-context-switch/devflow-docs/inception/application-design.md#L200)). Also `invalidate_all()` is infallible while `CatalogFailed` and mock invalidate-failure are defined ([application-design.md#L133](/Users/jay.ahn/projects/infra/nexttui/.worktrees/runtime-context-switch/devflow-docs/inception/application-design.md#L133), [application-design.md#L223](/Users/jay.ahn/projects/infra/nexttui/.worktrees/runtime-context-switch/devflow-docs/inception/application-design.md#L223), [application-design.md#L263](/Users/jay.ahn/projects/infra/nexttui/.worktrees/runtime-context-switch/devflow-docs/inception/application-design.md#L263)).  
devflow-docs/inception/design-review-raw/codex-detail.md:8675:Why: You cannot deterministically handle “rescope OK + invalidate fail + rollback,” so NFR-1 atomicity is not met ([requirements.md#L69](/Users/jay.ahn/projects/infra/nexttui/.worktrees/runtime-context-switch/devflow-docs/inception/requirements.md#L69)).  
devflow-docs/inception/design-review-raw/codex-detail.md:8676:Fix: Make `ContextSessionPort` own `rescope + catalog refresh + commit/rollback` end-to-end; `ContextSwitcher` should orchestrate state/epoch only.
devflow-docs/inception/design-review-raw/codex-detail.md:8678:2. **Port boundary is wrong: current auth port cannot perform scope switch commit.**  
devflow-docs/inception/design-review-raw/codex-detail.md:8683:3. **Epoch/cancellation enforcement point is underspecified and currently bypassable.**  
devflow-docs/inception/design-review-raw/codex-detail.md:8702:    Ambiguous { candidates: Vec<ContextTarget> },
devflow-docs/inception/design-review-raw/codex-detail.md:8710:    fn try_begin(&mut self, target: ContextTarget) -> Result<Epoch, SwitchError>; // not Result<_, SwitchError::InProgress>
devflow-docs/inception/design-review-raw/codex-detail.md:8711:    fn commit(&mut self, snapshot: ContextSnapshot);
devflow-docs/inception/design-review-raw/codex-detail.md:8718:pub trait ContextSessionPort: Send + Sync {
devflow-docs/inception/design-review-raw/codex-detail.md:8720:    async fn begin(&self, target: &ContextTarget, epoch: Epoch) -> Result<Self::Handle, SwitchError>;
devflow-docs/inception/design-review-raw/codex-detail.md:8721:    async fn rescope(&self, handle: &mut Self::Handle) -> Result<(), SwitchError>;
devflow-docs/inception/design-review-raw/codex-detail.md:8722:    async fn refresh_catalog(&self, handle: &mut Self::Handle) -> Result<(), SwitchError>;
devflow-docs/inception/design-review-raw/codex-detail.md:8723:    async fn commit(&self, handle: Self::Handle) -> Result<ContextSnapshot, SwitchError>;
devflow-docs/inception/design-review-raw/codex-detail.md:8724:    async fn rollback(&self, handle: Self::Handle) -> Result<(), SwitchError>;
devflow-docs/inception/design-review-raw/codex-detail.md:8729:pub async fn switch(&self, target: ContextTarget) -> Result<(Epoch, ContextSnapshot), SwitchError>;
devflow-docs/inception/design-review-raw/codex-detail.md:8736:    ContextChanged { target: ContextTarget },
devflow-docs/inception/design-review-raw/codex-detail.md:8743:pub async fn list_user_projects(&self) -> Result<Vec<ContextTarget>, SwitchError>;
devflow-docs/inception/design-review-raw/codex-detail.md:8756:- `ScopedAuthPort` (new port): required to mutate active scope without adapter downcast.
devflow-docs/inception/design-review-raw/codex-detail.md:8760:- Mock seam needs `begin_fail`, `commit_fail`, `rollback_called`, and ordered step assertions; current mock knobs are insufficient for NFR-3 failure-path tests.
devflow-docs/inception/design-review-raw/codex-detail.md:8766:**Brief reasoning:** DETAIL does not yet define an implementable atomic contract, violates intended port boundaries for auth scope mutation, and leaves stale-event isolation non-authoritative. These are correctness blockers, not polish issues.

exec
/bin/zsh -lc "nl -ba devflow-docs/inception/design-review-raw/synthesis-detail.md | sed -n '1,320p'" in /Users/jay.ahn/projects/infra/nexttui/.worktrees/runtime-context-switch
 succeeded in 0ms:
     1	# Council Synthesis — application-design.md DETAIL
     2	
     3	**Chair**: Claude
     4	**Reviewers**: Codex (REJECT), Gemini (APPROVE-WITH-CHANGES), Claude (APPROVE-WITH-CHANGES)
     5	**Verdict (synthesized)**: **APPROVE-WITH-CHANGES (DETAIL r2 개정 후 INCEPTION 완료)**
     6	
     7	Codex의 REJECT는 "atomic 계약과 port 경계의 보강 없이는 RED 진입 위험"이라는 정확한 지적. 종합 개정안에 핵심 사항을 모두 반영하면 APPROVE 수준으로 충분히 끌어올릴 수 있다.
     8	
     9	---
    10	
    11	## 1. 합의 사항 (3-AI 일치)
    12	
    13	### A. atomic 계약을 ContextSessionPort에 완전히 위임 (Codex+Gemini 합의)
    14	
    15	**현재 문제**: ContextSwitcher가 절차의 중간에서 `rescope_adapter.rescope`, `catalog_invalidator.invalidate_all`, `token_cache.store`를 직접 호출. begin/commit 사이에 외부 호출이 끼어들어 atomicity 보장 불가.
    16	
    17	**합의 결정**:
    18	```rust
    19	#[async_trait]
    20	pub trait ContextSessionPort: Send + Sync {
    21	    type Handle: Send;
    22	    async fn begin(&self, target: &ContextTarget, epoch: Epoch) -> Result<Self::Handle, SwitchError>;
    23	    async fn rescope(&self, handle: &mut Self::Handle) -> Result<(), SwitchError>;
    24	    async fn refresh_catalog(&self, handle: &mut Self::Handle) -> Result<(), SwitchError>;
    25	    async fn commit(&self, handle: Self::Handle) -> Result<ContextSnapshot, SwitchError>;
    26	    async fn rollback(&self, handle: Self::Handle) -> Result<(), SwitchError>;
    27	}
    28	```
    29	ContextSwitcher는 state/epoch/cancel만 오케스트레이트. rescope·invalidate·token store는 모두 port 구현 내부.
    30	
    31	### B. SessionHandle은 이전 token + scope를 캡처 (Gemini+Claude)
    32	
    33	```rust
    34	pub struct SessionHandle {
    35	    pub epoch: Epoch,
    36	    pub previous_token: Token,
    37	    pub previous_scope: TokenScope,
    38	    pub target: ContextTarget,
    39	    // (impl 내부) staged_new_token, staged_new_catalog
    40	}
    41	```
    42	rollback 시 ScopedAuthSession이 KeystoneAuthAdapter의 active_scope/token을 이전 값으로 강제 복원.
    43	
    44	### C. 단일 epoch 게이트를 App/event-loop 디스패처에 명시 (Codex+Gemini)
    45	
    46	- App이 `current_epoch: AtomicU64` 보유 (단일 권위)
    47	- 모든 `VersionedEvent<AppEvent>` 수신 시 `event.epoch < current_epoch` → drop
    48	- Worker 다중 폴링 사이트는 모두 `VersionedEvent<AppEvent>` 송신으로 통일
    49	- event_loop의 blind forward 제거
    50	
    51	### D. Worker 시그니처 통일 (Gemini+Claude)
    52	
    53	```rust
    54	pub async fn run_worker(
    55	    // ... existing ...
    56	    mut action_rx: mpsc::UnboundedReceiver<VersionedEvent<Action>>,
    57	    event_tx: mpsc::UnboundedSender<VersionedEvent<AppEvent>>,
    58	)
    59	```
    60	모든 spawn은 `(epoch, CancellationToken)` 페어를 받고, 송신 시 `VersionedEvent::new(ev, epoch)` 강제.
    61	
    62	### E. SwitchError::InProgress를 에러 타입으로 못 박지 않음 (Codex+Claude)
    63	
    64	```rust
    65	#[derive(Debug, thiserror::Error)]
    66	pub enum SwitchError {
    67	    #[error("switch already in progress")]    InProgress,
    68	    #[error("rescope rejected: {0}")]          RescopeRejected(String),
    69	    #[error("catalog invalidation failed")]    CatalogFailed(String),
    70	    #[error("ambiguous target")]               Ambiguous { candidates: Vec<ContextTarget> },
    71	    #[error("target not found: {0}")]          NotFound(String),
    72	    #[error(transparent)]                      Api(#[from] ApiError),
    73	    #[error(transparent)]                      Io(#[from] std::io::Error),
    74	}
    75	
    76	pub fn try_begin(&self, target: ContextTarget) -> Result<Epoch, SwitchError>; // 변수 SwitchError
    77	```
    78	
    79	### F. ContextIndicator 패시브 타이머 (Gemini+Claude)
    80	
    81	```rust
    82	pub struct ContextIndicator {
    83	    snapshot: Option<ContextSnapshot>,
    84	    last_switch_at: Option<Instant>,
    85	    highlight_duration: Duration,
    86	}
    87	impl Component for ContextIndicator {
    88	    fn render(&self, frame, area) {
    89	        let highlighting = self.last_switch_at
    90	            .map_or(false, |t| t.elapsed() < self.highlight_duration);
    91	        // ...
    92	    }
    93	}
    94	```
    95	`set_context(snapshot, mark_highlight: bool)` — mark_highlight=true 시 last_switch_at = Instant::now().
    96	
    97	### G. ContextSnapshot에 epoch + token 포함 (Gemini)
    98	
    99	```rust
   100	pub struct ContextSnapshot {
   101	    pub target: ContextTarget,
   102	    pub epoch: Epoch,
   103	    pub token: Token,                           // self-verifying
   104	    pub token_scope: TokenScope,
   105	    pub captured_at: chrono::DateTime<chrono::Utc>,
   106	}
   107	```
   108	
   109	### H. AppEvent::ContextChanged에서 epoch 제거 — VersionedEvent envelope이 이미 보유 (Codex)
   110	
   111	```rust
   112	pub enum AppEvent {
   113	    ContextChanged { target: ContextTarget },
   114	    // ...
   115	}
   116	```
   117	
   118	---
   119	
   120	## 2. Codex 단독 — 채택
   121	
   122	### ScopedAuthPort 신설 (port 경계 보강)
   123	
   124	`AuthProvider`는 read/request-auth 지향으로 유지. scope 변경 전용 port 분리:
   125	```rust
   126	#[async_trait]
   127	pub trait ScopedAuthPort: Send + Sync {
   128	    fn current_scope(&self) -> TokenScope;
   129	    fn current_token(&self) -> Token;
   130	    async fn set_active_scope(&self, scope: TokenScope, token: Token) -> Result<(), SwitchError>;
   131	    async fn upsert_scoped_token(&self, scope: TokenScope, token: Token) -> Result<(), SwitchError>;
   132	}
   133	```
   134	KeystoneAuthAdapter가 양쪽 trait 모두 구현. ScopedAuthSession은 ScopedAuthPort를 통해 active_scope를 mutate.
   135	
   136	### 미명시 정책 명문화
   137	
   138	- **Switching + (SwitchContext|SwitchBack|Cancel) 정책**: Switching 중 신규 switch 요청 → 즉시 `SwitchError::InProgress` 반환. Cancel 명령 → SwitchStateMachine에 `cancel()` 추가, port 핸들이 있으면 rollback 호출.
   139	- **ContextChanged 핸들링 컨트랙트**: 각 모듈 컴포넌트(Server/Volume/Network/.../Project Module)는 `handle_event(AppEvent::ContextChanged)`에서 내부 `Vec<T>` 비우기 + `is_loading=true`. Sidebar/StatusBar 등 컨텍스트 무관 컴포넌트는 default no-op.
   140	- **TokenCacheStore 키 정렬**: `store_scoped(&self, scope: &TokenScope, token: &Token)` — 기존 모델 일치, ContextTarget 직접 키 사용 금지.
   141	- **list_user_projects를 async로**: `pub async fn list_user_projects(&self) -> Result<Vec<ContextTarget>, SwitchError>` (Keystone API 호출).
   142	
   143	### Mock 시즘 확장
   144	
   145	`MockContextSession`에 추가:
   146	- `with_begin_failure`, `with_rescope_failure`, `with_refresh_failure`, `with_commit_failure`
   147	- `rollback_called() -> bool`, `transition_steps() -> Vec<&'static str>` (순서 검증)
   148	
   149	---
   150	
   151	## 3. Claude 단독 — 채택
   152	
   153	### HttpEndpointCache trait 추가
   154	
   155	EndpointCatalogInvalidator가 모든 HTTP client의 endpoint cache를 일괄 무효화하려면 공통 trait 필요:
   156	```rust
   157	pub trait HttpEndpointCache {
   158	    fn invalidate(&self);
   159	}
   160	```
   161	모든 `BaseHttpClient`가 구현. AdapterRegistry가 `Vec<Arc<dyn HttpEndpointCache>>` 보유.
   162	
   163	### KeystoneCapabilities 정의
   164	
   165	```rust
   166	#[derive(Debug, Clone)]
   167	pub struct KeystoneCapabilities {
   168	    pub allow_rescope_scoped_token: bool,
   169	    pub auth_method: AuthMethod,        // Password | Token | AppCredential
   170	    pub api_version: KeystoneVersion,
   171	}
   172	pub enum AuthMethod { Password, Token, AppCredential }
   173	```
   174	첫 토큰 응답 또는 `/v3` discovery에서 추론.
   175	
   176	### Test seam 추가
   177	
   178	- `MockContextSession::with_partial_commit_failure` (rescope OK + invalidate OK + commit fail)
   179	- `MockContextSession::with_slow_rescope(Duration)` (timeout 시뮬레이션)
   180	- worker.rs::tests에 stale event drop unit test (epoch 이전 이벤트가 drop되는지)
   181	- Resolver Ambiguous 픽스처 (cloud A/B에 같은 이름 admin)
   182	
   183	---
   184	
   185	## 4. Switcher 절차 개정 (모두 통합)
   186	
   187	```rust
   188	pub async fn switch(&self, target: ContextTarget) -> Result<(Epoch, ContextSnapshot), SwitchError> {
   189	    // 1. epoch bump + state Switching
   190	    let new_epoch = self.state.try_begin(target.clone())?;
   191	
   192	    // 2. 이전 epoch의 모든 작업 cancel (idempotent, 두 번 호출 안전)
   193	    self.cancellation.cancel_below(new_epoch);
   194	
   195	    // 3. session.begin (handle에 previous_token/scope 캡처)
   196	    let mut handle = match self.session.begin(&target, new_epoch).await {
   197	        Ok(h) => h,
   198	        Err(e) => { self.state.fail(e.clone()); return Err(e); }
   199	    };
   200	
   201	    // 4. atomic transition (모두 port 내부)
   202	    if let Err(e) = self.session.rescope(&mut handle).await
   203	        .and_then(|_| self.session.refresh_catalog(&mut handle).await) {
   204	        let _ = self.session.rollback(handle).await;
   205	        self.state.fail(e.clone());
   206	        return Err(e);
   207	    }
   208	
   209	    // 5. commit (port 내부에서 ScopedAuthPort.set_active_scope + token store)
   210	    let snapshot = match self.session.commit(handle).await {
   211	        Ok(s) => s,
   212	        Err(e) => { self.state.fail(e.clone()); return Err(e); }
   213	    };
   214	
   215	    // 6. state.commit + history push
   216	    self.state.commit(snapshot.clone());
   217	    self.history.push(snapshot.clone());
   218	
   219	    // 7. (마지막 안전망) 다시 한 번 cancel
   220	    self.cancellation.cancel_below(new_epoch);
   221	
   222	    Ok((new_epoch, snapshot))
   223	}
   224	```
   225	
   226	Switcher는 `state + cancellation + session` 3개에만 의존. rescope/invalidate/token store는 모두 session 내부로 이동.
   227	
   228	## 5. 컴포넌트 의존 그래프 (개정)
   229	
   230	```
   231	                      App (Controller)
   232	                       |
   233	                       | owns
   234	                       v
   235	                  ContextSwitcher (Service)
   236	                  /    |    \
   237	                 v     v     v
   238	            State  Cancel  ContextSessionPort (trait)
   239	                              |
   240	                              | impl
   241	                              v
   242	                       ScopedAuthSession (Service)
   243	                       /     |       |        \
   244	                      v      v       v         v
   245	              KeystoneRescope  ScopedAuth  EndpointCatalog  TokenCacheStore
   246	                Adapter          Port       Invalidator       (Repository)
   247	                                  |               |
   248	                                  | impl          | reads
   249	                                  v               v
   250	                          KeystoneAuthAdapter   AdapterRegistry
   251	                                                     |
   252	                                                     v
   253	                                               HttpEndpointCache trait
   254	```
   255	
   256	ContextSwitcher의 협력자가 8개 → 3개로 축소. atomic 책임이 단일 port에 집중.
   257	
   258	---
   259	
   260	## 6. DETAIL r2 개정 체크리스트
   261	
   262	- [ ] ContextSessionPort에 `Handle` 연관타입 + rescope/refresh_catalog/commit/rollback 메서드 도입
   263	- [ ] ContextSwitcher.switch 절차 개정 (위 4번 코드)
   264	- [ ] SwitchError 재정의 (transparent ApiError/IoError 포함)
   265	- [ ] SessionHandle 정의 (previous_token, previous_scope 포함)
   266	- [ ] ContextSnapshot에 epoch + token 추가
   267	- [ ] AppEvent::ContextChanged에서 epoch 제거 (envelope이 보유)
   268	- [ ] ScopedAuthPort 신설 + KeystoneAuthAdapter가 구현
   269	- [ ] HttpEndpointCache trait 신설
   270	- [ ] KeystoneCapabilities 정의 명시
   271	- [ ] App에 `current_epoch: AtomicU64` + dispatcher epoch gate 명시
   272	- [ ] Worker 시그니처: `VersionedEvent<Action>` rx + `VersionedEvent<AppEvent>` tx
   273	- [ ] ContextIndicator 패시브 타이머 (last_switch_at, render에서 check)
   274	- [ ] ContextChanged 핸들링 컨트랙트: 모듈 컴포넌트 white-list 추가
   275	- [ ] TokenCacheStore: `store_scoped(scope, token)` 시그니처 (TokenScope 키)
   276	- [ ] ContextTargetResolver.list_user_projects를 async로
   277	- [ ] Switching 정책: 신규 switch → InProgress, Cancel → state.cancel + session.rollback
   278	- [ ] MockContextSession 시즘 확장 (with_*_failure, transition_steps, rollback_called)
   279	
   280	---
   281	
   282	## 최종 Verdict
   283	
   284	**APPROVE-WITH-CHANGES** — 위 17개 체크리스트 적용 후 INCEPTION 완료로 진입.
   285	
   286	핵심 변경 1줄 요약: **atomic 책임을 ContextSessionPort 내부로 완전 통합 + 단일 epoch 게이트 디스패처에 명시 + ScopedAuthPort 분리**.

exec
/bin/zsh -lc 'nl -ba devflow-docs/inception/design-review-raw/gemini-detail.md' in /Users/jay.ahn/projects/infra/nexttui/.worktrees/runtime-context-switch
 succeeded in 0ms:
     1	Loaded cached credentials.
     2	I will start by reading the design document `devflow-docs/inception/application-design.md` focusing on the `# DETAIL Mode` section, along with the requirements and synthesis documents to understand the full context.
     3	I'll start by checking the current definitions of `TokenScope`, `Action`, `AppEvent`, and the worker spawning logic in `src/port/types.rs`, `src/action.rs`, `src/event.rs`, and `src/worker.rs` to ensure the design's "Codebase fit" and "Concurrency hazards" are properly addressed.
     4	I'll read `src/app.rs` to understand how the event loop is structured and where the `epoch` check should be integrated to ensure that stale events from previous contexts are correctly ignored.
     5	I'll examine `src/adapter/http/base.rs` to see how the endpoint cache is implemented and how it can be invalidated, which will help clarify the "Redundant Invalidation" issue I've identified.
     6	The technical review of the **DETAIL** section for feature **BL-P2-031** is as follows. The design is robust in its use of epochs for concurrency control, but there are critical ambiguities in ownership and atomicity that must be addressed before code generation.
     7	
     8	### Top 3 Critical Issues
     9	
    10	1.  **Redundant Orchestration & Ownership Ambiguity (Dependency Direction)**
    11	    *   **What**: `ContextSwitcher` (Service) depends on both `ContextSessionPort` (Port) and `KeystoneRescopeAdapter` (Adapter). The procedure description has `ContextSwitcher` manually calling `rescope` and `invalidate_all` between `session.begin` and `session.commit`.
    12	    *   **Why**: This splits the atomic transition logic across two layers. If `ContextSessionPort` is the boundary for a "Session," it should encapsulate the *entire* session-level change (rescope + catalog refresh + endpoint invalidation).
    13	    *   **Fix**: Move the rescope and invalidation calls *inside* the `ContextSessionPort` implementation. `ContextSwitcher` should only call `port.transition(target)`, which handles the Keystone handshake and cache clearing as a single unit.
    14	
    15	2.  **Incomplete Rollback Contract (Atomicity Gaps)**
    16	    *   **What**: `ContextSessionPort::rollback(handle)` is defined, but `SessionHandle` is opaque. If `rescope` succeeds but `invalidate_all` or `catalog_refresh` fails, the `AuthProvider` state is already mutated (new token scoped to new project).
    17	    *   **Why**: Without restoring the old token/catalog in the `AuthProvider`, the "rollback" only reverts the UI state, leaving the HTTP client with a new-project token but a "Failed" switcher state—a torn state.
    18	    *   **Fix**: `SessionHandle` must contain the **captured previous `Token` and `TokenScope`**. `rollback` must explicitly re-inject these into the `AuthProvider` (KeystoneAuthAdapter) to guarantee a return to the exact previous state.
    19	
    20	3.  **Passive UI Highlight Hazard (UI Correctness)**
    21	    *   **What**: `ContextIndicator::highlight_for(Duration)` implies an active timer.
    22	    *   **Why**: `ratatui` components are passive and only move on `render`. A `highlight_for` method called once will not "stop" the highlight after N seconds unless the component tracks time relative to the render loop.
    23	    *   **Fix**: Change the signature to `set_context(&mut self, snapshot: &ContextSnapshot, highlight: bool)`. Internally, the widget must store `last_switch_at: Instant` and check `Instant::now() - last_switch_at < highlight_duration` during every `render` call.
    24	
    25	### Specific Signature Corrections
    26	
    27	```rust
    28	// src/context.rs
    29	// Change: Add Epoch to ContextSnapshot to ensure snapshot is self-verifying.
    30	pub struct ContextSnapshot {
    31	    pub target: ContextTarget,
    32	    pub epoch: Epoch, 
    33	    pub token: Token, // Explicitly include the scoped token
    34	    pub captured_at: chrono::DateTime<chrono::Utc>,
    35	}
    36	
    37	// src/port/auth.rs (ContextSessionPort trait)
    38	// Change: Atomic transition instead of begin/commit split to prevent logic leakage.
    39	#[async_trait]
    40	pub trait ContextSessionPort: Send + Sync {
    41	    /// Performs rescope + catalog refresh. Returns handle for rollback.
    42	    async fn transition(&self, target: &ContextTarget) -> Result<(SessionHandle, ContextSnapshot), SwitchError>;
    43	    async fn rollback(&self, handle: SessionHandle) -> Result<(), SwitchError>;
    44	}
    45	
    46	// src/worker.rs
    47	// Change: Worker loop MUST receive the versioned envelope.
    48	pub async fn run_worker(
    49	    // ... existing ...
    50	    mut action_rx: mpsc::UnboundedReceiver<VersionedEvent<Action>>,
    51	    event_tx: mpsc::UnboundedSender<VersionedEvent<AppEvent>>,
    52	)
    53	```
    54	
    55	### Missing Components or Interfaces
    56	
    57	1.  **Epoch Enforcement Point (Controller)**: The design lacks the explicit check logic in `App::handle_event`. It must be stated that `App` maintains a `current_epoch: AtomicU64` and *drops* any `VersionedEvent<AppEvent>` where `event.epoch < current_epoch`.
    58	2.  **Opaque `SessionHandle` Definition**: Needs to be a struct in `src/port/auth.rs` containing `old_token: Token` and `old_scope: TokenScope`.
    59	
    60	### Verdict: **APPROVE-WITH-CHANGES**
    61	
    62	**Reasoning**: The concurrency model (Epoch + CancellationToken) is excellent and follows Rust best practices for async TUI. However, the layering of the `ContextSwitcher` vs `ContextSessionPort` is "chatty" and risks leaving the application in a torn state if the rollback doesn't restore the `AuthProvider` token. Consolidating the rescope logic into the `Port` implementation ensures true atomicity.
    63	
    64	**Next Step**: Once these ownership and rollback details are clarified in the document, the project is ready for the **RED** phase of PR1 (Infrastructure).

exec
/bin/zsh -lc 'nl -ba devflow-docs/inception/design-review-raw/claude-detail.md' in /Users/jay.ahn/projects/infra/nexttui/.worktrees/runtime-context-switch
 succeeded in 0ms:
     1	# Claude Review — application-design.md DETAIL
     2	
     3	**Reviewer**: Claude (self-critique)
     4	**Mode**: DETAIL section critique (interface signatures, dependency direction, atomicity)
     5	
     6	## Top 3 Critical Issues
     7	
     8	### 1. SwitchStateMachine 시그니처에 무효한 Rust 에러 타입 + 동시 변경 가능성 미정의
     9	- **What**: `try_begin -> Result<Epoch, SwitchError::InProgress>` — Rust는 enum variant를 에러 타입으로 못 박을 수 없음. `Result<Epoch, SwitchError>` 여야 함. 또한 `&mut self`인데 `ContextSwitcher`는 `Arc`로 보유될 수밖에 없음(다중 호출자). 내부 lock이 명시되지 않음.
    10	- **Why**: 컴파일 안 됨. 그리고 동시 switch 호출 시 `Mutex<SwitchState>` 등으로 상호배제 필수.
    11	- **Fix**:
    12	  ```rust
    13	  pub struct SwitchStateMachine {
    14	      state: Mutex<SwitchState>,        // or parking_lot::Mutex
    15	      history: Arc<Mutex<ContextHistoryStore>>,
    16	  }
    17	  impl SwitchStateMachine {
    18	      pub fn try_begin(&self, target: ContextTarget) -> Result<Epoch, SwitchError> { ... }
    19	  }
    20	  ```
    21	
    22	### 2. ContextSwitcher.switch의 내부 절차 순서가 epoch race를 만든다
    23	- **What**: 명시된 순서: `state.begin → registry.cancel_below(new_epoch) → session.begin → rescope → invalidate → token_cache.store → state.commit`. 그러나 `state.begin`이 epoch를 bump한다고 가정해도, cancel_below는 **새 epoch 미만의 모든 작업** cancel — 즉 새 epoch 자체의 token도 등록되기 전에 cancel 필터를 통과한다. OK. 하지만 worker spawn이 register한 토큰을 begin과 동시에 다른 thread가 spawn 중이라면, cancel이 새 spawn을 놓칠 수 있음.
    24	- **Why**: 동시성 race — switch 시작 직후 들어온 신규 spawn이 옛 epoch로 등록되면 cancel 우회.
    25	- **Fix**: spawn API를 `(epoch, cancel) = registry.register_with_current()`로 강제 — registry 내부에서 `current_epoch == registry.frozen_epoch?` 검사. 또는 switch 진입 시 spawn lock 잠시 차단. 또는 `cancel_below(new_epoch)`를 `state.commit` 직전에 1회 더 호출(idempotent).
    26	
    27	### 3. ContextTarget vs TokenScope 타입 중복 + Resolver의 ProjectRef 미해결 상태
    28	- **What**: `ContextTarget { cloud, project: ProjectRef, domain }`, `TokenScope::Project { name, domain }`. Resolver는 ProjectRef를 받아 ContextTarget을 반환해야 하는데, ContextTarget 내부에 ProjectRef가 또 들어있음. "resolved target"과 "user input"이 같은 타입.
    29	- **Why**: switch 절차 곳곳에서 "이건 resolved인가?" 판단 필요 → 버그 양산.
    30	- **Fix**:
    31	  ```rust
    32	  pub enum ContextRequest {            // user input (parser output)
    33	      ByName { cloud: Option<String>, project: String, domain: Option<String> },
    34	      ById   { cloud: Option<String>, project_id: String },
    35	  }
    36	  pub struct ContextTarget {           // resolved (post-resolver)
    37	      pub cloud: String,
    38	      pub project_id: String,
    39	      pub project_name: String,
    40	      pub domain: String,
    41	  }
    42	  ```
    43	  Resolver: `ContextRequest → ContextTarget`. TokenScope는 `From<&ContextTarget>`로 변환.
    44	
    45	## 추가 시그니처 교정
    46	
    47	### Worker spawn API
    48	DETAIL 표현 `pub fn spawn(epoch, cancel, fut: impl Future<Output = AppEvent>) -> JoinHandle<()>` — 여러 polling 사이트가 다른 반환 타입을 가짐. 일반화 필요:
    49	```rust
    50	pub fn spawn_versioned<F, T>(
    51	    registry: &CancellationRegistry,
    52	    epoch: Epoch,
    53	    fut: F,
    54	) -> JoinHandle<()>
    55	where
    56	    F: Future<Output = T> + Send + 'static,
    57	    T: Into<AppEvent> + Send + 'static,
    58	```
    59	혹은 `app_tx`도 인자로 받아 내부에서 `VersionedEvent::new(ev, epoch)` 송신.
    60	
    61	### KeystoneCapabilities 미정의
    62	DETAIL에 언급만 됨. 정의 필요:
    63	```rust
    64	#[derive(Debug, Clone)]
    65	pub struct KeystoneCapabilities {
    66	    pub allow_rescope_scoped_token: bool,
    67	    pub auth_method: AuthMethod,        // Password | Token | AppCredential
    68	    pub api_version: KeystoneVersion,
    69	}
    70	```
    71	초기 호출 시 `/v3` discovery 또는 첫 토큰 응답에서 추론.
    72	
    73	### EndpointCatalogInvalidator의 AdapterRegistry 의존
    74	`AdapterRegistry`가 가변 상태인지 확인 필요. Invalidate가 모든 adapter의 endpoint cache를 비우려면 registry가 `Vec<Arc<dyn HttpClient>>` 같은 형태로 보유하고, 각 client에 `invalidate_endpoints()` trait method가 필요. trait이 LIST에 없음.
    75	**Fix**: `pub trait HttpEndpointCache { fn invalidate(&self); }` 추가 후 모든 `BaseHttpClient`가 구현. Invalidator는 registry 순회.
    76	
    77	## 미명시 사항 (RED phase 진입 전 결정 필수)
    78	
    79	1. **ScopedAuthSession이 AuthProvider를 owns/wraps/replaces?** — 현재는 "uses"라고 표기. 실 코드에서 KeystoneAuthAdapter가 active_scope 필드를 가짐. ScopedAuthSession이 KeystoneAuthAdapter를 mutex로 감싸 active_scope를 교체하는 형태인지 명시 필요.
    80	
    81	2. **ContextChanged variant를 받는 Component 화이트리스트** — 어떤 모듈이 데이터를 비워야 하는가? 후보: `ResourceListView`, `DetailView`, 모든 `<Resource>Module` (Server, Volume, Network 등). LIST 어딘가에 명시 필요. 그렇지 않으면 어떤 모듈은 옛 데이터를 그대로 들고 있음.
    82	
    83	3. **ContextIndicator 강조 타이머 구동** — render loop가 60fps 폴링이면 `Instant::now()` 비교만으로 OK. 하지만 idle 시 redraw 안 되는 구조라면 `AppEvent::Tick` 또는 timer task 필요. App의 render 정책 확인.
    84	
    85	4. **Switching 중 들어온 두 번째 switch 요청 처리** — `try_begin`이 `InProgress` 반환 → CommandParser는 사용자에게 "전환 중" 에러 표시? 또는 큐잉? 정책 미정.
    86	
    87	5. **Cancel during Switching** — 사용자가 `Esc` 또는 다른 명령으로 진행 중 switch 취소? state machine에 `Cancelled` transition 부재.
    88	
    89	6. **App의 mut/lock 정책** — `App.switch_context(&mut self, target)` 표기인데, 현재 코드의 App은 single-threaded loop 안에서 mutate. async switch가 진행 중일 때 다른 keypress 처리는? mainloop가 await 점에서 yield하면 OK. 명시 필요.
    90	
    91	## Test seams
    92	
    93	- **MockContextSession** — `with_rescope_failure`, `with_invalidate_failure`까지는 좋음. 추가 필요: `with_partial_commit_failure` (rescope OK + invalidate OK + token store fail), `with_slow_rescope` (timeout 시뮬레이션).
    94	- **Worker epoch 검증** — 단위 테스트가 어디 들어가는지 미명시. `worker.rs::tests` 안에 stale event drop 케이스 추가 필요.
    95	- **Resolver 충돌** — `Ambiguous` 케이스 테스트 데이터 (cloud A/B에 같은 이름 admin 프로젝트) 픽스처 필요.
    96	
    97	## Verdict
    98	
    99	**APPROVE-WITH-CHANGES**
   100	
   101	이유: 컴포넌트 분해와 의존 그래프는 정합. 그러나 (1) `Result<Epoch, SwitchError::InProgress>` 같은 컴파일 불가 시그니처, (2) ContextTarget vs TokenScope 타입 중복, (3) atomicity 경계의 동시성 race 처리 미명시 — 이 셋이 RED phase 진입 전 보강 필수. HttpEndpointCache trait 부재도 빠진 인터페이스로 명시.

codex
I’ve read the short reviews and next I’m extracting the decisive findings from the much longer Codex review to verify nothing critical was lost in the synthesis.
exec
/bin/zsh -lc "nl -ba devflow-docs/inception/design-review-raw/codex-detail.md | sed -n '1,260p'" in /Users/jay.ahn/projects/infra/nexttui/.worktrees/runtime-context-switch
 succeeded in 0ms:
     1	Reading additional input from stdin...
     2	OpenAI Codex v0.118.0 (research preview)
     3	--------
     4	workdir: /Users/jay.ahn/projects/infra/nexttui/.worktrees/runtime-context-switch
     5	model: gpt-5.3-codex
     6	provider: openai
     7	approval: never
     8	sandbox: read-only
     9	reasoning effort: high
    10	reasoning summaries: none
    11	session id: 019d85be-9b26-7360-ba05-ca099726fe9b
    12	--------
    13	user
    14	You are reviewing the DETAIL section of application-design.md (LIST already approved with revisions).
    15	
    16	PROJECT: nexttui — Rust + ratatui TUI for OpenStack admins.
    17	FEATURE: BL-P2-031 — Runtime cloud/project context switching via Keystone rescoping.
    18	
    19	REVIEW THIS FILE: devflow-docs/inception/application-design.md (focus on the "# DETAIL Mode" section onward, line ~103+)
    20	ALSO CONSIDER: devflow-docs/inception/requirements.md (10 FR + 5 NFR for BL-P2-031), devflow-docs/inception/design-review-raw/synthesis.md (prior LIST council synthesis)
    21	
    22	Codebase reference points:
    23	- src/port/auth.rs (AuthProvider trait)
    24	- src/adapter/auth/keystone.rs (KeystoneAuthAdapter, active_scope)
    25	- src/adapter/auth/token_cache.rs (TokenScope, TokenCacheStore)
    26	- src/adapter/http/base.rs (endpoint cache)
    27	- src/component.rs (Component trait)
    28	- src/action.rs (Action enum), src/event.rs (AppEvent enum)
    29	- src/worker.rs (run_worker, polling spawn sites)
    30	
    31	REVIEW SCOPE — assess the DETAIL design for:
    32	1. **Interface correctness** — Are method signatures sensible? Async/sync split correct? Error types aligned with codebase patterns (ApiError, thiserror)?
    33	2. **Dependency direction** — Cycles? Layering violations (Adapter depending on Service)? Port at right boundary?
    34	3. **Atomicity gaps** — Does the begin/commit/rollback flow truly guarantee no torn state? Particularly around: rescope OK + invalidate fail; epoch bump vs cancel ordering; commit ordering (token cache write before or after state.commit?)
    35	4. **Concurrency hazards** — Worker spawn signature change: does it cover all spawn sites? VersionedEvent unwrap in the dispatcher: where exactly is the epoch check enforced (Worker side, App side, or both)?
    36	5. **State machine completeness** — Missing transitions (e.g. cancel during Switching, second switch request mid-flight)?
    37	6. **UI correctness** — ContextChanged variant: does every Component need to handle it? What about Components that hold no per-context data — are they no-op safe? ContextIndicator highlight_for: who drives the timer?
    38	7. **Test seams** — MockContextSession: does it cover all failure modes the production code can hit? Is there a way to deterministically reproduce "rescope succeeds + catalog refresh fails + rollback runs"?
    39	8. **Codebase fit** — Are types like ContextTarget overlapping with existing TokenScope? Should ContextTarget reuse TokenScope or be a new type?
    40	9. **Missing details** — Anything underspecified that will block code-generation TDD (RED phase needs to know expected behavior)?
    41	
    42	DELIVER (concise, no hedging):
    43	- Top 3 critical issues (each: what + why + concrete fix)
    44	- Specific signature corrections (with exact Rust code if needed)
    45	- Missing components or interfaces
    46	- Verdict: APPROVE | APPROVE-WITH-CHANGES | REJECT
    47	- Brief reasoning
    48	
    49	Be harsh but constructive. Rust idiom and OpenStack production realism.
    50	codex
    51	I’m reviewing `application-design.md` from `# DETAIL Mode` onward against the requirements/synthesis and the referenced Rust modules. I’ll first extract the relevant sections and current code signatures, then I’ll map design claims to concrete interface/concurrency behavior and return hard findings with fixes.
    52	exec
    53	/bin/zsh -lc "nl -ba devflow-docs/inception/application-design.md | sed -n '1,260p'" in /Users/jay.ahn/projects/infra/nexttui/.worktrees/runtime-context-switch
    54	 succeeded in 0ms:
    55	     1	# Application Design
    56	     2	
    57	     3	**Mode**: LIST (목록 단계, Council 리뷰 반영 개정)
    58	     4	**Timestamp**: 2026-04-13T00:00:00+09:00
    59	     5	**BL**: BL-P2-031 Keystone Rescoping
    60	     6	**Revision**: r2 (3-AI Council 종합 반영 — design-review-raw/synthesis.md 참조)
    61	     7	
    62	     8	## 의존 방향 원칙
    63	     9	
    64	    10	- **App** (Controller, orchestrator) **owns** **ContextSwitcher**.
    65	    11	- **ContextSwitcher**는 commit 결과로 `(new_epoch, ContextSnapshot)` 반환. App이 epoch를 반영하고 `AppEvent::ContextChanged`를 디스패치.
    66	    12	- **ContextSwitcher**는 `ContextSessionPort`를 통해 atomic begin/commit/rollback 수행.
    67	    13	- **Worker spawn API**는 `(epoch, CancellationToken)` 페어를 반드시 받는다 (시그니처 강제).
    68	    14	
    69	    15	## 컴포넌트 목록
    70	    16	
    71	    17	### 신규 컴포넌트 (12개)
    72	    18	
    73	    19	| 컴포넌트 | 책임 | 타입 | PR |
    74	    20	|---------|------|------|-----|
    75	    21	| `ContextEpoch` | 단조 증가 epoch 카운터, stale 이벤트 식별 키 | Util | PR1 |
    76	    22	| `CancellationRegistry` | 활성 폴링/장기 fetch에 대한 `CancellationToken` 등록·일괄 취소 | Service | PR1 |
    77	    23	| `VersionedEvent<T>` envelope | `{ event: T, epoch: u64 }` — Action/AppEvent를 감싸 epoch stamp (variant 폭증 회피) | Util | PR1 |
    78	    24	| `AppEvent::ContextChanged { target }` | UI에 컨텍스트 변경 통지 → 모듈 컴포넌트가 내부 데이터 비움 | Event variant | PR1 |
    79	    25	| `SwitchStateMachine` | `Idle → Switching → Committed | Failed` 전환 상태, rollback 규약 | Service | PR1 |
    80	    26	| `ContextSwitcher` | 전환 절차 오케스트레이터 (epoch++ → cancel → session begin → rescope → catalog 무효화 → commit) | Service | PR1 |
    81	    27	| `ContextSessionPort` (trait) | atomic begin/commit/rollback 인터페이스. `invalidate_all_endpoints()` hook | Port | PR1 |
    82	    28	| `ScopedAuthSession` | `ContextSessionPort` 구현체. 활성 scoped token + endpoint cache 일괄 관리 | Service | PR1 |
    83	    29	| `KeystoneRescopeAdapter` | Keystone v3 token-method scoped exchange 호출, expires_at 정본 사용 | Adapter | PR1 |
    84	    30	| `EndpointCatalogInvalidator` | 모든 HTTP client의 endpoint cache 일괄 무효화 (`src/adapter/http/base.rs` 자동화) | Service | PR1 |
    85	    31	| `ContextTargetResolver` | name/uuid/cloud-prefix → ContextTarget 변환, 충돌 disambiguation. 명령·피커·모듈 액션 공유 | Service | PR1 |
    86	    32	| `ContextHistoryStore` | switch-back 1단계 히스토리 (ContextSnapshot 저장), rollback 시 동일 사용 | Util | PR1 |
    87	    33	| `ContextIndicator` | 영구 컨텍스트 표시 위젯 (`cloud / project [/ domain]`), 전환 직후 강조 | UI Widget | PR3 |
    88	    34	| `ContextPicker` | Ctrl+P 모달, 프로젝트/클라우드 fuzzy 검색·선택 (`cloud • project • domain • project_id` 행) | UI Widget (modal) | PR4 |
    89	    35	
    90	    36	> 신규 컴포넌트 14개 (PR1 인프라 12 + UI 2). `AppEvent::ContextChanged`는 enum variant 추가지만 표에 분리 표기.
    91	    37	
    92	    38	### 변경 컴포넌트 (8개)
    93	    39	
    94	    40	| 컴포넌트 | 변경 책임 | 타입 | PR |
    95	    41	|---------|----------|------|-----|
    96	    42	| `App` (src/app.rs) | 활성 cloud/project 컨텍스트 보유, ContextSwitcher 소유, epoch/스냅샷 반영, ContextChanged 디스패치 | Controller (재분류) | PR1 |
    97	    43	| `Worker` (src/worker.rs) | spawn API에 `(epoch, CancellationToken)` 페어 강제, `tokio::select!` cancel branch + epoch 검증 | Service | PR1 |
    98	    44	| `Action` (src/action.rs) | `VersionedEvent<Action>` envelope 적용 (또는 `Action::with_epoch`) | Type | PR1 |
    99	    45	| `AppEvent` (src/event.rs) | `VersionedEvent<AppEvent>` envelope 적용 + `ContextChanged` variant 추가 | Type | PR1 |
   100	    46	| `CommandParser` (src/input/command.rs) | `:switch-project <name|uuid|cloud/project>`, `:switch-cloud`, `:switch-back` 등록 + tab 자동완성. `ContextTargetResolver` 사용 | Controller | PR3 |
   101	    47	| `ConfirmDialog` (src/ui/confirm.rs) | destructive confirm에 `cloud • project` fingerprint 표시. 컨텍스트 변경 직후엔 추가 확인 강제 | UI Widget | PR3 |
   102	    48	| `StatusBar` (src/ui/status_bar.rs) | ContextIndicator 임베드 위치/우선순위 조정 | UI Widget | PR3 |
   103	    49	| `TokenCacheStore` (src/adapter/auth/token_cache.rs) | rescoped 토큰을 scope별로 저장/조회 (BL-P2-029 기반), 만료 시 재취득 | Repository | PR1 |
   104	    50	| `Project Module` (src/module/project/mod.rs) | 모듈-로컬 `s` 액션 핸들러. KeyMap 글로벌 등록 회피 (Enter는 Detail 유지) | Controller | PR5 |
   105	    51	
   106	    52	> 변경 컴포넌트 9개. `Action`/`AppEvent`는 type 정의 변경이라 별도 표기.
   107	    53	
   108	    54	### Mock / 테스트 시즘 (1개)
   109	    55	
   110	    56	| 컴포넌트 | 책임 | 타입 | PR |
   111	    57	|---------|------|------|-----|
   112	    58	| `MockContextSession` (확장) | `src/port/mock.rs`에 추가. rescope 성공/실패, catalog invalidate 부분 실패 시뮬레이션 (fault-injection seam) | Mock | PR1 |
   113	    59	
   114	    60	**총 24개 컴포넌트** (신규 14 + 변경 9 + 테스트 1)
   115	    61	
   116	    62	## 개정된 PR 매핑
   117	    63	
   118	    64	| PR | 컴포넌트 | Depends on | 사용자 노출 |
   119	    65	|----|---------|-----------|-----------|
   120	    66	| **PR1 (safety infra + switch core)** | ContextEpoch, CancellationRegistry, VersionedEvent, AppEvent::ContextChanged, SwitchStateMachine, ContextSwitcher, ContextSessionPort, ScopedAuthSession, KeystoneRescopeAdapter, EndpointCatalogInvalidator, ContextTargetResolver, ContextHistoryStore, App 통합, Worker 시그니처 개정, Action/AppEvent envelope, TokenCacheStore 확장, MockContextSession | — | 없음 (인프라만) |
   121	    67	| **PR3 (안전 가시성 + 명령)** | ContextIndicator, StatusBar 임베드, ConfirmDialog fingerprint, CommandParser 확장 | PR1 | `:switch-*` 명령, 컨텍스트 인디케이터, fingerprint confirm |
   122	    68	| **PR4 (피커 UI)** | ContextPicker, KeyMap 글로벌 단축키 (Ctrl+P) | PR3 | Ctrl+P 모달 |
   123	    69	| **PR5 (Identity 통합)** | Project Module 모듈-로컬 `s` 핸들러 | PR3 | Identity 리스트 `s` 단축키 |
   124	    70	
   125	    71	→ **PR 수: 6 → 4로 축소**. PR1 통합으로 stale 누설 창 제거. PR3에 안전 가시성 + 명령을 묶어 사용자가 전환을 쓸 수 있는 첫 시점부터 인디케이터/fingerprint가 함께 동작.
   126	    72	
   127	    73	## NFR 매핑
   128	    74	
   129	    75	| NFR | 보장 컴포넌트 |
   130	    76	|-----|--------------|
   131	    77	| NFR-1 안전성 (atomic switch, stale 차단) | ContextEpoch + CancellationRegistry + SwitchStateMachine + ContextSessionPort/ScopedAuthSession + EndpointCatalogInvalidator |
   132	    78	| NFR-2 성능 (1초 이내) | KeystoneRescopeAdapter (네트워크 왕복 측정), EndpointCatalogInvalidator (lazy refresh) |
   133	    79	| NFR-3 테스트 커버리지 | MockContextSession (port mock), ContextSwitcher 통합 테스트, 부분실패 (rescope OK + invalidate fail) 시뮬레이션 |
   134	    80	| NFR-4 UX 일관성 | ContextPicker (기존 SelectPopup 재사용), CommandParser 확장 (기존 패턴), ContextTargetResolver (단일 disambiguation 로직) |
   135	    81	| NFR-5 관측성 | ContextSwitcher의 `tracing` 이벤트 (epoch, target cloud/project, 결과, 소요 시간) |
   136	    82	
   137	    83	## 개정 사유 요약 (Council 리뷰 반영)
   138	    84	
   139	    85	| 변경 | 출처 | 이유 |
   140	    86	|------|------|------|
   141	    87	| ContextSessionPort + ScopedAuthSession 추가 | Codex | atomic begin/commit/rollback 부재 시 rescope 성공 + stale endpoint 호출 사고 가능 |
   142	    88	| EndpointCatalogInvalidator 추가 | Codex | `src/adapter/http/base.rs:66`의 매뉴얼 invalidate를 자동화해야 일관성 보장 |
   143	    89	| ContextHistoryStore 추가 | Codex+Claude | switch-back/rollback 공통 저장소 누락 |
   144	    90	| ContextTargetResolver 추가 | Codex | 명령·피커·모듈 액션이 같은 disambiguation 로직 필요 (3중 구현 회피) |
   145	    91	| AppEvent::ContextChanged 추가 | Gemini | epoch만으로는 잔존 데이터 표시 방지 불가 |
   146	    92	| VersionedEvent envelope | Gemini | epoch를 매 variant에 추가하지 않고 plumbing |
   147	    93	| ContextIndicator/Picker → UI Widget | Codex | Component trait 위젯이 정확. Controller는 라우팅 의미 |
   148	    94	| App → Controller | Codex | 코드베이스 실제 역할이 orchestrator/router |
   149	    95	| CommandRegistry → CommandParser 확장 | Codex | src/input/command.rs 실제 명명 일치 |
   150	    96	| PR1+PR2 통합 | Claude+Codex | PR1만 머지 시 사용자 노출 0이지만 stale 누설 창 발생. 통합으로 창 제거 |
   151	    97	| PR5 ContextIndicator/fingerprint를 PR3로 앞당김 | Codex | PR3/4에서 명령/피커가 안전 가시성 없이 노출되는 위험 차단 |
   152	    98	| PR6 KeyMap 분리 → PR5 모듈-로컬 | Claude+Codex | KeyMap 글로벌 동시 수정 충돌 회피, `s` 의미가 모듈 의존적 |
   153	    99	| MockContextSession 명시 | Claude | port mock 확장 누락 보완 |
   154	   100	
   155	   101	---
   156	   102	
   157	   103	# DETAIL Mode (Standard depth)
   158	   104	
   159	   105	**Timestamp**: 2026-04-13T00:00:00+09:00
   160	   106	모든 컴포넌트의 주요 인터페이스(2~3개)와 의존 컴포넌트를 명시.
   161	   107	
   162	   108	## 핵심 타입
   163	   109	
   164	   110	```rust
   165	   111	// src/context.rs (신규 모듈)
   166	   112	pub type Epoch = u64;
   167	   113	
   168	   114	#[derive(Debug, Clone, PartialEq, Eq, Hash)]
   169	   115	pub struct ContextTarget {
   170	   116	    pub cloud: String,
   171	   117	    pub project: ProjectRef,        // ByName(name) | ById(uuid)
   172	   118	    pub domain: Option<String>,
   173	   119	}
   174	   120	
   175	   121	#[derive(Debug, Clone)]
   176	   122	pub struct ContextSnapshot {
   177	   123	    pub target: ContextTarget,
   178	   124	    pub epoch: Epoch,
   179	   125	    pub token_scope: TokenScope,    // src/port/types.rs 재사용
   180	   126	    pub captured_at: chrono::DateTime<chrono::Utc>,
   181	   127	}
   182	   128	
   183	   129	#[derive(Debug, Clone, thiserror::Error)]
   184	   130	pub enum SwitchError {
   185	   131	    #[error("rescope rejected by Keystone: {0}")]
   186	   132	    RescopeRejected(String),
   187	   133	    #[error("catalog invalidation failed: {0}")]
   188	   134	    CatalogFailed(String),
   189	   135	    #[error("ambiguous target: {candidates:?}")]
   190	   136	    Ambiguous { candidates: Vec<ContextTarget> },
   191	   137	    #[error("target not found: {0}")]
   192	   138	    NotFound(String),
   193	   139	    #[error("switch in progress")]
   194	   140	    InProgress,
   195	   141	}
   196	   142	```
   197	   143	
   198	   144	## 신규 컴포넌트 상세
   199	   145	
   200	   146	### ContextEpoch (Util)
   201	   147	**Responsibility**: 단조 증가 epoch 카운터.
   202	   148	**Interface**:
   203	   149	- `pub fn new() -> Self` — 0부터 시작
   204	   150	- `pub fn current(&self) -> Epoch`
   205	   151	- `pub fn bump(&self) -> Epoch` — Atomic increment, 새 값 반환
   206	   152	**Dependencies**: 없음 (`AtomicU64` 내부 사용)
   207	   153	
   208	   154	### CancellationRegistry (Service)
   209	   155	**Responsibility**: 활성 작업의 `CancellationToken` 등록·일괄 취소.
   210	   156	**Interface**:
   211	   157	- `pub fn register(&self, epoch: Epoch) -> CancellationToken` — 새 토큰 발급·등록
   212	   158	- `pub fn cancel_below(&self, threshold_epoch: Epoch) -> usize` — 이전 epoch 토큰 일괄 cancel, 취소 건수 반환
   213	   159	- `pub fn active_count(&self) -> usize` — 관측용
   214	   160	**Dependencies**: `tokio_util::sync::CancellationToken`
   215	   161	
   216	   162	### VersionedEvent\<T\> (Util)
   217	   163	**Responsibility**: epoch + payload envelope.
   218	   164	**Interface**:
   219	   165	- `pub fn new(payload: T, epoch: Epoch) -> Self`
   220	   166	- `pub fn epoch(&self) -> Epoch`
   221	   167	- `pub fn into_inner(self) -> T`
   222	   168	**Dependencies**: 없음. Action / AppEvent 양쪽에 사용.
   223	   169	
   224	   170	### AppEvent::ContextChanged variant
   225	   171	**Responsibility**: 컨텍스트 전환 후 모듈 컴포넌트에 데이터 비우기 신호.
   226	   172	**Interface (variant)**:
   227	   173	- `AppEvent::ContextChanged { target: ContextTarget, epoch: Epoch }`
   228	   174	**Dependencies**: `ContextTarget` (src/context.rs)
   229	   175	**호환성**: 기존 `Component::handle_event(&mut self, event: &AppEvent)`에서 `match`로 처리
   230	   176	
   231	   177	### SwitchStateMachine (Service)
   232	   178	**Responsibility**: `Idle → Switching → Committed | Failed` 상태 전이, rollback 규약.
   233	   179	**Interface**:
   234	   180	- `pub fn try_begin(&mut self, target: ContextTarget) -> Result<Epoch, SwitchError::InProgress>`
   235	   181	- `pub fn commit(&mut self, snapshot: ContextSnapshot)` — Committed 상태로 이동
   236	   182	- `pub fn fail(&mut self, err: SwitchError) -> ContextSnapshot` — Failed 후 직전 snapshot 반환 (rollback용)
   237	   183	- `pub fn state(&self) -> SwitchState` — Idle/Switching/Committed/Failed
   238	   184	**Dependencies**: `ContextHistoryStore` (이전 snapshot 조회)
   239	   185	
   240	   186	### ContextSwitcher (Service)
   241	   187	**Responsibility**: 전환 절차 오케스트레이터.
   242	   188	**Interface**:
   243	   189	- `pub async fn switch(&self, target: ContextTarget) -> Result<ContextSnapshot, SwitchError>`
   244	   190	- `pub async fn switch_back(&self) -> Result<ContextSnapshot, SwitchError>`
   245	   191	- 절차: state.begin → registry.cancel_below(new_epoch) → session.begin → rescope_adapter.rescope → catalog_invalidator.invalidate_all → token_cache.store → state.commit → return snapshot
   246	   192	**Dependencies**: `SwitchStateMachine`, `CancellationRegistry`, `ContextSessionPort`, `KeystoneRescopeAdapter`, `EndpointCatalogInvalidator`, `TokenCacheStore`, `ContextHistoryStore`, `ContextEpoch`
   247	   193	
   248	   194	### ContextSessionPort (Port trait)
   249	   195	**Responsibility**: 활성 scoped token + endpoint cache의 atomic begin/commit/rollback 인터페이스.
   250	   196	**Interface**:
   251	   197	```rust
   252	   198	#[async_trait]
   253	   199	pub trait ContextSessionPort: Send + Sync {
   254	   200	    async fn begin(&self, target: &ContextTarget) -> Result<SessionHandle, SwitchError>;
   255	   201	    async fn commit(&self, handle: SessionHandle) -> Result<ContextSnapshot, SwitchError>;
   256	   202	    async fn rollback(&self, handle: SessionHandle);
   257	   203	    fn invalidate_all_endpoints(&self);
   258	   204	}
   259	   205	```
   260	   206	**Dependencies**: `ContextTarget`, `ContextSnapshot`, `SwitchError`

exec
/bin/zsh -lc "nl -ba devflow-docs/inception/design-review-raw/codex-detail.md | sed -n '8540,8780p'" in /Users/jay.ahn/projects/infra/nexttui/.worktrees/runtime-context-switch
 succeeded in 0ms:
  8540	src/module/floating_ip/mod.rs:21:use self::view_model::{fip_columns, fip_create_defs, fip_to_row, FipRowContext};
  8541	src/module/floating_ip/mod.rs:92:        let ctx = FipRowContext {
  8542	src/module/floating_ip/mod.rs:97:        self.floating_ips.iter().map(|f| fip_to_row(f, &ctx)).collect()
  8543	src/module/floating_ip/view_model.rs:7:pub struct FipRowContext<'a> {
  8544	src/module/floating_ip/view_model.rs:13:impl<'a> FipRowContext<'a> {
  8545	src/module/floating_ip/view_model.rs:61:    fip_to_row(fip, &FipRowContext::default_for(show_tenant))
  8546	src/module/floating_ip/view_model.rs:64:pub fn fip_to_row(fip: &FloatingIp, ctx: &FipRowContext) -> Row {
  8547	src/module/floating_ip/view_model.rs:65:    let show_tenant = ctx.show_tenant;
  8548	src/module/floating_ip/view_model.rs:66:    let cached_servers = ctx.cached_servers;
  8549	src/module/floating_ip/view_model.rs:67:    let cached_ports = ctx.cached_ports;
  8550	src/module/server/view_model.rs:10:pub struct ServerViewContext<'a> {
  8551	src/module/server/view_model.rs:19:impl<'a> ServerViewContext<'a> {
  8552	src/module/server/view_model.rs:146:    server_detail_data(&ServerViewContext::default_for(server))
  8553	src/module/server/view_model.rs:149:pub fn server_detail_data(ctx: &ServerViewContext) -> DetailData {
  8554	src/module/server/view_model.rs:150:    let server = ctx.server;
  8555	src/module/server/view_model.rs:151:    let migration_progress = ctx.migration_progress;
  8556	src/module/server/view_model.rs:152:    let flavor = ctx.flavor;
  8557	src/module/server/view_model.rs:153:    let is_resize_pending = ctx.is_resize_pending;
  8558	src/module/server/view_model.rs:154:    let cached_volumes = ctx.cached_volumes;
  8559	src/module/server/view_model.rs:155:    let cached_floating_ips = ctx.cached_floating_ips;
  8560	src/module/server/view_model.rs:686:        let data = server_detail_data(&ServerViewContext::default_for(&server));
  8561	src/module/server/view_model.rs:693:        let data = server_detail_data(&ServerViewContext::default_for(&server));
  8562	src/module/server/view_model.rs:714:        let data = server_detail_data(&ServerViewContext {
  8563	src/module/server/view_model.rs:732:        let data = server_detail_data(&ServerViewContext::default_for(&server));
  8564	src/module/server/view_model.rs:771:        let data = server_detail_data(&ServerViewContext {
  8565	src/module/server/view_model.rs:787:        let data = server_detail_data(&ServerViewContext::default_for(&server));
  8566	src/module/server/view_model.rs:801:        let data = server_detail_data(&ServerViewContext {
  8567	src/module/server/mod.rs:24:    ServerViewContext,
  8568	src/module/server/mod.rs:515:            let ctx = self.popup_kind.unwrap_or(DetailPopupKind::Resize);
  8569	src/module/server/mod.rs:520:                    match ctx {
  8570	src/module/server/mod.rs:999:                    let data = server_detail_data(&ServerViewContext {
  8571	
  8572	codex
  8573	**Top 3 critical issues**
  8574	
  8575	1. **Atomic boundary is internally contradictory and cannot guarantee rollback safety.**  
  8576	What: DETAIL says `ContextSessionPort` is the atomic boundary, but the switch flow bypasses it (`state.begin -> ... -> token_cache.store -> state.commit`) and never calls `session.commit/rollback` ([application-design.md#L191](/Users/jay.ahn/projects/infra/nexttui/.worktrees/runtime-context-switch/devflow-docs/inception/application-design.md#L191), [application-design.md#L200](/Users/jay.ahn/projects/infra/nexttui/.worktrees/runtime-context-switch/devflow-docs/inception/application-design.md#L200)). Also `invalidate_all()` is infallible while `CatalogFailed` and mock invalidate-failure are defined ([application-design.md#L133](/Users/jay.ahn/projects/infra/nexttui/.worktrees/runtime-context-switch/devflow-docs/inception/application-design.md#L133), [application-design.md#L223](/Users/jay.ahn/projects/infra/nexttui/.worktrees/runtime-context-switch/devflow-docs/inception/application-design.md#L223), [application-design.md#L263](/Users/jay.ahn/projects/infra/nexttui/.worktrees/runtime-context-switch/devflow-docs/inception/application-design.md#L263)).  
  8577	Why: You cannot deterministically handle “rescope OK + invalidate fail + rollback,” so NFR-1 atomicity is not met ([requirements.md#L69](/Users/jay.ahn/projects/infra/nexttui/.worktrees/runtime-context-switch/devflow-docs/inception/requirements.md#L69)).  
  8578	Fix: Make `ContextSessionPort` own `rescope + catalog refresh + commit/rollback` end-to-end; `ContextSwitcher` should orchestrate state/epoch only.
  8579	
  8580	2. **Port boundary is wrong: current auth port cannot perform scope switch commit.**  
  8581	What: `AuthProvider` has no scope mutation APIs ([auth.rs#L8](/Users/jay.ahn/projects/infra/nexttui/.worktrees/runtime-context-switch/src/port/auth.rs#L8)), while `KeystoneAuthAdapter` holds fixed `active_scope` and all token reads key off it ([keystone.rs#L135](/Users/jay.ahn/projects/infra/nexttui/.worktrees/runtime-context-switch/src/adapter/auth/keystone.rs#L135), [keystone.rs#L357](/Users/jay.ahn/projects/infra/nexttui/.worktrees/runtime-context-switch/src/adapter/auth/keystone.rs#L357)).  
  8582	Why: `ScopedAuthSession` cannot be implemented cleanly through `AuthProvider`; it will require concrete adapter leakage/downcast, violating your intended layering.  
  8583	Fix: Add a dedicated scoped-auth port (`set_active_scope`, `upsert_scoped_token`, `current_scope_snapshot`) and keep `AuthProvider` read/request-auth oriented.
  8584	
  8585	3. **Epoch/cancellation enforcement point is underspecified and currently bypassable.**  
  8586	What: DETAIL says worker spawn wraps one future and run_worker does epoch drop ([application-design.md#L278](/Users/jay.ahn/projects/infra/nexttui/.worktrees/runtime-context-switch/devflow-docs/inception/application-design.md#L278)), but current worker emits events from multiple polling paths ([worker.rs#L795](/Users/jay.ahn/projects/infra/nexttui/.worktrees/runtime-context-switch/src/worker.rs#L795), [worker.rs#L825](/Users/jay.ahn/projects/infra/nexttui/.worktrees/runtime-context-switch/src/worker.rs#L825), [worker.rs#L844](/Users/jay.ahn/projects/infra/nexttui/.worktrees/runtime-context-switch/src/worker.rs#L844)); event loop currently forwards blindly ([event_loop.rs#L48](/Users/jay.ahn/projects/infra/nexttui/.worktrees/runtime-context-switch/src/event_loop.rs#L48)).  
  8587	Why: FR-5 requires hard stale-drop guarantees across all async emitters, not just one spawn style ([requirements.md#L40](/Users/jay.ahn/projects/infra/nexttui/.worktrees/runtime-context-switch/devflow-docs/inception/requirements.md#L40)).  
  8588	Fix: Enforce epoch gate at a single choke point (App/event-loop dispatcher) and require all event sends to use `VersionedEvent<AppEvent>`.
  8589	
  8590	---
  8591	
  8592	**Specific signature corrections**
  8593	
  8594	```rust
  8595	pub type Epoch = u64;
  8596	
  8597	#[derive(Debug, thiserror::Error)]
  8598	pub enum SwitchError {
  8599	    #[error("switch already in progress")]
  8600	    InProgress,
  8601	    #[error("target not found: {0}")]
  8602	    NotFound(String),
  8603	    #[error("ambiguous target")]
  8604	    Ambiguous { candidates: Vec<ContextTarget> },
  8605	    #[error(transparent)]
  8606	    Api(#[from] ApiError),
  8607	    #[error(transparent)]
  8608	    Io(#[from] std::io::Error),
  8609	}
  8610	
  8611	pub trait SwitchStateMachine {
  8612	    fn try_begin(&mut self, target: ContextTarget) -> Result<Epoch, SwitchError>; // not Result<_, SwitchError::InProgress>
  8613	    fn commit(&mut self, snapshot: ContextSnapshot);
  8614	    fn fail(&mut self, err: SwitchError);
  8615	}
  8616	```
  8617	
  8618	```rust
  8619	#[async_trait]
  8620	pub trait ContextSessionPort: Send + Sync {
  8621	    type Handle: Send;
  8622	    async fn begin(&self, target: &ContextTarget, epoch: Epoch) -> Result<Self::Handle, SwitchError>;
  8623	    async fn rescope(&self, handle: &mut Self::Handle) -> Result<(), SwitchError>;
  8624	    async fn refresh_catalog(&self, handle: &mut Self::Handle) -> Result<(), SwitchError>;
  8625	    async fn commit(&self, handle: Self::Handle) -> Result<ContextSnapshot, SwitchError>;
  8626	    async fn rollback(&self, handle: Self::Handle) -> Result<(), SwitchError>;
  8627	}
  8628	```
  8629	
  8630	```rust
  8631	pub async fn switch(&self, target: ContextTarget) -> Result<(Epoch, ContextSnapshot), SwitchError>;
  8632	pub async fn switch_back(&self) -> Result<(Epoch, ContextSnapshot), SwitchError>;
  8633	```
  8634	
  8635	```rust
  8636	// Avoid duplicate epoch in payload when VersionedEvent already carries it.
  8637	pub enum AppEvent {
  8638	    ContextChanged { target: ContextTarget },
  8639	    // ...
  8640	}
  8641	```
  8642	
  8643	```rust
  8644	// Resolver hits Keystone project API; make it async.
  8645	pub async fn list_user_projects(&self) -> Result<Vec<ContextTarget>, SwitchError>;
  8646	```
  8647	
  8648	```rust
  8649	// Keep cache key semantics aligned with existing storage model.
  8650	pub fn store_scoped(&self, scope: &TokenScope, token: &Token) -> io::Result<()>;
  8651	pub fn lookup_scoped(&self, scope: &TokenScope) -> Option<Token>;
  8652	```
  8653	
  8654	---
  8655	
  8656	**Missing components or interfaces**
  8657	
  8658	- `ScopedAuthPort` (new port): required to mutate active scope without adapter downcast.
  8659	- `VersionedEvent` gate in dispatcher: mandatory single stale-drop authority.
  8660	- Switch transition policy for `Switching + (SwitchContext | SwitchBack | Cancel)`; currently unspecified.
  8661	- `ContextChanged` handling contract per component (clear list/detail/form pending state); currently aspirational only.
  8662	- Mock seam needs `begin_fail`, `commit_fail`, `rollback_called`, and ordered step assertions; current mock knobs are insufficient for NFR-3 failure-path tests.
  8663	
  8664	---
  8665	
  8666	**Verdict: REJECT**
  8667	
  8668	**Brief reasoning:** DETAIL does not yet define an implementable atomic contract, violates intended port boundaries for auth scope mutation, and leaves stale-event isolation non-authoritative. These are correctness blockers, not polish issues.
  8669	tokens used
  8670	239,130
  8671	**Top 3 critical issues**
  8672	
  8673	1. **Atomic boundary is internally contradictory and cannot guarantee rollback safety.**  
  8674	What: DETAIL says `ContextSessionPort` is the atomic boundary, but the switch flow bypasses it (`state.begin -> ... -> token_cache.store -> state.commit`) and never calls `session.commit/rollback` ([application-design.md#L191](/Users/jay.ahn/projects/infra/nexttui/.worktrees/runtime-context-switch/devflow-docs/inception/application-design.md#L191), [application-design.md#L200](/Users/jay.ahn/projects/infra/nexttui/.worktrees/runtime-context-switch/devflow-docs/inception/application-design.md#L200)). Also `invalidate_all()` is infallible while `CatalogFailed` and mock invalidate-failure are defined ([application-design.md#L133](/Users/jay.ahn/projects/infra/nexttui/.worktrees/runtime-context-switch/devflow-docs/inception/application-design.md#L133), [application-design.md#L223](/Users/jay.ahn/projects/infra/nexttui/.worktrees/runtime-context-switch/devflow-docs/inception/application-design.md#L223), [application-design.md#L263](/Users/jay.ahn/projects/infra/nexttui/.worktrees/runtime-context-switch/devflow-docs/inception/application-design.md#L263)).  
  8675	Why: You cannot deterministically handle “rescope OK + invalidate fail + rollback,” so NFR-1 atomicity is not met ([requirements.md#L69](/Users/jay.ahn/projects/infra/nexttui/.worktrees/runtime-context-switch/devflow-docs/inception/requirements.md#L69)).  
  8676	Fix: Make `ContextSessionPort` own `rescope + catalog refresh + commit/rollback` end-to-end; `ContextSwitcher` should orchestrate state/epoch only.
  8677	
  8678	2. **Port boundary is wrong: current auth port cannot perform scope switch commit.**  
  8679	What: `AuthProvider` has no scope mutation APIs ([auth.rs#L8](/Users/jay.ahn/projects/infra/nexttui/.worktrees/runtime-context-switch/src/port/auth.rs#L8)), while `KeystoneAuthAdapter` holds fixed `active_scope` and all token reads key off it ([keystone.rs#L135](/Users/jay.ahn/projects/infra/nexttui/.worktrees/runtime-context-switch/src/adapter/auth/keystone.rs#L135), [keystone.rs#L357](/Users/jay.ahn/projects/infra/nexttui/.worktrees/runtime-context-switch/src/adapter/auth/keystone.rs#L357)).  
  8680	Why: `ScopedAuthSession` cannot be implemented cleanly through `AuthProvider`; it will require concrete adapter leakage/downcast, violating your intended layering.  
  8681	Fix: Add a dedicated scoped-auth port (`set_active_scope`, `upsert_scoped_token`, `current_scope_snapshot`) and keep `AuthProvider` read/request-auth oriented.
  8682	
  8683	3. **Epoch/cancellation enforcement point is underspecified and currently bypassable.**  
  8684	What: DETAIL says worker spawn wraps one future and run_worker does epoch drop ([application-design.md#L278](/Users/jay.ahn/projects/infra/nexttui/.worktrees/runtime-context-switch/devflow-docs/inception/application-design.md#L278)), but current worker emits events from multiple polling paths ([worker.rs#L795](/Users/jay.ahn/projects/infra/nexttui/.worktrees/runtime-context-switch/src/worker.rs#L795), [worker.rs#L825](/Users/jay.ahn/projects/infra/nexttui/.worktrees/runtime-context-switch/src/worker.rs#L825), [worker.rs#L844](/Users/jay.ahn/projects/infra/nexttui/.worktrees/runtime-context-switch/src/worker.rs#L844)); event loop currently forwards blindly ([event_loop.rs#L48](/Users/jay.ahn/projects/infra/nexttui/.worktrees/runtime-context-switch/src/event_loop.rs#L48)).  
  8685	Why: FR-5 requires hard stale-drop guarantees across all async emitters, not just one spawn style ([requirements.md#L40](/Users/jay.ahn/projects/infra/nexttui/.worktrees/runtime-context-switch/devflow-docs/inception/requirements.md#L40)).  
  8686	Fix: Enforce epoch gate at a single choke point (App/event-loop dispatcher) and require all event sends to use `VersionedEvent<AppEvent>`.
  8687	
  8688	---
  8689	
  8690	**Specific signature corrections**
  8691	
  8692	```rust
  8693	pub type Epoch = u64;
  8694	
  8695	#[derive(Debug, thiserror::Error)]
  8696	pub enum SwitchError {
  8697	    #[error("switch already in progress")]
  8698	    InProgress,
  8699	    #[error("target not found: {0}")]
  8700	    NotFound(String),
  8701	    #[error("ambiguous target")]
  8702	    Ambiguous { candidates: Vec<ContextTarget> },
  8703	    #[error(transparent)]
  8704	    Api(#[from] ApiError),
  8705	    #[error(transparent)]
  8706	    Io(#[from] std::io::Error),
  8707	}
  8708	
  8709	pub trait SwitchStateMachine {
  8710	    fn try_begin(&mut self, target: ContextTarget) -> Result<Epoch, SwitchError>; // not Result<_, SwitchError::InProgress>
  8711	    fn commit(&mut self, snapshot: ContextSnapshot);
  8712	    fn fail(&mut self, err: SwitchError);
  8713	}
  8714	```
  8715	
  8716	```rust
  8717	#[async_trait]
  8718	pub trait ContextSessionPort: Send + Sync {
  8719	    type Handle: Send;
  8720	    async fn begin(&self, target: &ContextTarget, epoch: Epoch) -> Result<Self::Handle, SwitchError>;
  8721	    async fn rescope(&self, handle: &mut Self::Handle) -> Result<(), SwitchError>;
  8722	    async fn refresh_catalog(&self, handle: &mut Self::Handle) -> Result<(), SwitchError>;
  8723	    async fn commit(&self, handle: Self::Handle) -> Result<ContextSnapshot, SwitchError>;
  8724	    async fn rollback(&self, handle: Self::Handle) -> Result<(), SwitchError>;
  8725	}
  8726	```
  8727	
  8728	```rust
  8729	pub async fn switch(&self, target: ContextTarget) -> Result<(Epoch, ContextSnapshot), SwitchError>;
  8730	pub async fn switch_back(&self) -> Result<(Epoch, ContextSnapshot), SwitchError>;
  8731	```
  8732	
  8733	```rust
  8734	// Avoid duplicate epoch in payload when VersionedEvent already carries it.
  8735	pub enum AppEvent {
  8736	    ContextChanged { target: ContextTarget },
  8737	    // ...
  8738	}
  8739	```
  8740	
  8741	```rust
  8742	// Resolver hits Keystone project API; make it async.
  8743	pub async fn list_user_projects(&self) -> Result<Vec<ContextTarget>, SwitchError>;
  8744	```
  8745	
  8746	```rust
  8747	// Keep cache key semantics aligned with existing storage model.
  8748	pub fn store_scoped(&self, scope: &TokenScope, token: &Token) -> io::Result<()>;
  8749	pub fn lookup_scoped(&self, scope: &TokenScope) -> Option<Token>;
  8750	```
  8751	
  8752	---
  8753	
  8754	**Missing components or interfaces**
  8755	
  8756	- `ScopedAuthPort` (new port): required to mutate active scope without adapter downcast.
  8757	- `VersionedEvent` gate in dispatcher: mandatory single stale-drop authority.
  8758	- Switch transition policy for `Switching + (SwitchContext | SwitchBack | Cancel)`; currently unspecified.
  8759	- `ContextChanged` handling contract per component (clear list/detail/form pending state); currently aspirational only.
  8760	- Mock seam needs `begin_fail`, `commit_fail`, `rollback_called`, and ordered step assertions; current mock knobs are insufficient for NFR-3 failure-path tests.
  8761	
  8762	---
  8763	
  8764	**Verdict: REJECT**
  8765	
  8766	**Brief reasoning:** DETAIL does not yet define an implementable atomic contract, violates intended port boundaries for auth scope mutation, and leaves stale-event isolation non-authoritative. These are correctness blockers, not polish issues.

exec
/bin/zsh -lc 'rg -n "''^## |''^### |Top 3|Verdict|APPROVE|REJECT|Brief reasoning|Missing|Open question|must|blocker" devflow-docs/inception/design-review-raw/codex-detail.md | sed -n '"'1,240p'" in /Users/jay.ahn/projects/infra/nexttui/.worktrees/runtime-context-switch
 succeeded in 0ms:
36:5. **State machine completeness** — Missing transitions (e.g. cancel during Switching, second switch request mid-flight)?
40:9. **Missing details** — Anything underspecified that will block code-generation TDD (RED phase needs to know expected behavior)?
43:- Top 3 critical issues (each: what + why + concrete fix)
45:- Missing components or interfaces
46:- Verdict: APPROVE | APPROVE-WITH-CHANGES | REJECT
47:- Brief reasoning
438:     4	**Reviewers**: Codex (REJECT), Gemini (APPROVE-WITH-CHANGES), Claude (APPROVE-WITH-CHANGES)
439:     5	**Verdict (synthesized)**: **APPROVE-WITH-CHANGES (LIST 개정 후 DETAIL 진입)**
528:    94	## 최종 Verdict
530:    96	**APPROVE-WITH-CHANGES** — LIST를 위 결정대로 개정 후 DETAIL 진입.
948:   264	            .ok_or_else(|| ApiError::AuthFailed("Missing X-Subject-Token header".into()))?
3875:   421	    /// Events like ServersLoaded must reach ServerModule even if the user is on a different view.
8573:**Top 3 critical issues**
8656:**Missing components or interfaces**
8666:**Verdict: REJECT**
8668:**Brief reasoning:** DETAIL does not yet define an implementable atomic contract, violates intended port boundaries for auth scope mutation, and leaves stale-event isolation non-authoritative. These are correctness blockers, not polish issues.
8671:**Top 3 critical issues**
8754:**Missing components or interfaces**
8764:**Verdict: REJECT**
8766:**Brief reasoning:** DETAIL does not yet define an implementable atomic contract, violates intended port boundaries for auth scope mutation, and leaves stale-event isolation non-authoritative. These are correctness blockers, not polish issues.

exec
/bin/zsh -lc "nl -ba devflow-docs/inception/requirements.md | sed -n '1,220p'" in /Users/jay.ahn/projects/infra/nexttui/.worktrees/runtime-context-switch
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
/bin/zsh -lc "nl -ba devflow-docs/inception/application-design.md | sed -n '100,360p'" in /Users/jay.ahn/projects/infra/nexttui/.worktrees/runtime-context-switch
 succeeded in 0ms:
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

codex
**Top 3 Issues**
1. **Switcher step 4 is not compilable, and step 5 can break atomicity on commit error.**  
Ref: [synthesis-detail.md#L202](/Users/jay.ahn/projects/infra/nexttui/.worktrees/runtime-context-switch/devflow-docs/inception/design-review-raw/synthesis-detail.md#L202), [synthesis-detail.md#L210](/Users/jay.ahn/projects/infra/nexttui/.worktrees/runtime-context-switch/devflow-docs/inception/design-review-raw/synthesis-detail.md#L210).  
`.await.and_then(|_| ... .await)` is invalid on `Result` (cannot `await` inside that closure). Also `commit` failure path does `state.fail` only, no rollback, so “partial commit failure” remains torn-state risk.

2. **“Cancel during Switching” is specified but not implementable as written.**  
Ref: [synthesis-detail.md#L138](/Users/jay.ahn/projects/infra/nexttui/.worktrees/runtime-context-switch/devflow-docs/inception/design-review-raw/synthesis-detail.md#L138), [synthesis-detail.md#L277](/Users/jay.ahn/projects/infra/nexttui/.worktrees/runtime-context-switch/devflow-docs/inception/design-review-raw/synthesis-detail.md#L277).  
`SessionHandle` is local inside `switch()` ([synthesis-detail.md#L196](/Users/jay.ahn/projects/infra/nexttui/.worktrees/runtime-context-switch/devflow-docs/inception/design-review-raw/synthesis-detail.md#L196)); external cancel cannot reliably call rollback mid-rescope without a defined ownership/cancellation model.

3. **One critical type-model issue from Claude was dropped.**  
Dropped item: `ContextRequest` (user input) vs `ContextTarget` (resolved target) split.  
Source: [claude-detail.md#L27](/Users/jay.ahn/projects/infra/nexttui/.worktrees/runtime-context-switch/devflow-docs/inception/design-review-raw/claude-detail.md#L27).  
Synthesis keeps only `ContextTarget`, so ambiguity around resolved vs unresolved scope is still latent.

**Specific Corrections to Synthesis Text**
1. Replace step 4 chaining with explicit async sequencing:
```rust
self.session.rescope(&mut handle).await?;
self.session.refresh_catalog(&mut handle).await?;
```
2. Define commit failure contract explicitly (choose one):
- `commit` is all-or-nothing and self-reverting on error, **or**
- `commit` returns recoverable handle (or uses `&mut handle`) so caller can rollback on commit error.
3. Replace checklist item “Cancel -> state.cancel + session.rollback” with a concrete model:
- either reject cancel during `Switching` (`InProgress`), or
- cooperative cancellation token + handle ownership in state machine.
4. Add a checklist item for `ContextRequest -> ContextTarget` resolver boundary.
5. Tighten handle/type ownership:
- make `SessionHandle` fields non-public (or `pub(crate)`),  
- clarify whether `ScopedAuthPort` or `TokenCacheStore` owns scoped token persistence to avoid dual ownership.

**Dropped From Original Reviews That Should Be Restored**
1. `ContextRequest` vs `ContextTarget` split ([claude-detail.md#L27](/Users/jay.ahn/projects/infra/nexttui/.worktrees/runtime-context-switch/devflow-docs/inception/design-review-raw/claude-detail.md#L27)).  
2. Explicit concurrency/locking expectation for state machine access ([claude-detail.md#L8](/Users/jay.ahn/projects/infra/nexttui/.worktrees/runtime-context-switch/devflow-docs/inception/design-review-raw/claude-detail.md#L8)).  
3. Indicator timeout dependency on redraw/tick policy ([claude-detail.md#L83](/Users/jay.ahn/projects/infra/nexttui/.worktrees/runtime-context-switch/devflow-docs/inception/design-review-raw/claude-detail.md#L83)).

**17-Item Checklist Completeness**
Needs additions. Add at least:
1. Commit-failure rollback/atomic contract.
2. Cancel-during-switching execution model.
3. `ContextRequest -> ContextTarget` type split.
4. State machine synchronization contract (single-thread invariant or mutexed state).

**PR1 Implementability**
Not fully ready for deterministic RED-phase tests yet.  
Most seams are present, but commit-failure semantics, cancel semantics, and request/target typing are still underspecified, so some failure-path tests remain ambiguous.

**Verdict**
**APPROVE-WITH-CHANGES**  
Synthesis captures most major findings well, but it introduces a non-compilable procedure snippet and leaves two spec-level blockers (cancel model, request/target boundary) that should be fixed before adopting as DETAIL r2.
tokens used
117,231
**Top 3 Issues**
1. **Switcher step 4 is not compilable, and step 5 can break atomicity on commit error.**  
Ref: [synthesis-detail.md#L202](/Users/jay.ahn/projects/infra/nexttui/.worktrees/runtime-context-switch/devflow-docs/inception/design-review-raw/synthesis-detail.md#L202), [synthesis-detail.md#L210](/Users/jay.ahn/projects/infra/nexttui/.worktrees/runtime-context-switch/devflow-docs/inception/design-review-raw/synthesis-detail.md#L210).  
`.await.and_then(|_| ... .await)` is invalid on `Result` (cannot `await` inside that closure). Also `commit` failure path does `state.fail` only, no rollback, so “partial commit failure” remains torn-state risk.

2. **“Cancel during Switching” is specified but not implementable as written.**  
Ref: [synthesis-detail.md#L138](/Users/jay.ahn/projects/infra/nexttui/.worktrees/runtime-context-switch/devflow-docs/inception/design-review-raw/synthesis-detail.md#L138), [synthesis-detail.md#L277](/Users/jay.ahn/projects/infra/nexttui/.worktrees/runtime-context-switch/devflow-docs/inception/design-review-raw/synthesis-detail.md#L277).  
`SessionHandle` is local inside `switch()` ([synthesis-detail.md#L196](/Users/jay.ahn/projects/infra/nexttui/.worktrees/runtime-context-switch/devflow-docs/inception/design-review-raw/synthesis-detail.md#L196)); external cancel cannot reliably call rollback mid-rescope without a defined ownership/cancellation model.

3. **One critical type-model issue from Claude was dropped.**  
Dropped item: `ContextRequest` (user input) vs `ContextTarget` (resolved target) split.  
Source: [claude-detail.md#L27](/Users/jay.ahn/projects/infra/nexttui/.worktrees/runtime-context-switch/devflow-docs/inception/design-review-raw/claude-detail.md#L27).  
Synthesis keeps only `ContextTarget`, so ambiguity around resolved vs unresolved scope is still latent.

**Specific Corrections to Synthesis Text**
1. Replace step 4 chaining with explicit async sequencing:
```rust
self.session.rescope(&mut handle).await?;
self.session.refresh_catalog(&mut handle).await?;
```
2. Define commit failure contract explicitly (choose one):
- `commit` is all-or-nothing and self-reverting on error, **or**
- `commit` returns recoverable handle (or uses `&mut handle`) so caller can rollback on commit error.
3. Replace checklist item “Cancel -> state.cancel + session.rollback” with a concrete model:
- either reject cancel during `Switching` (`InProgress`), or
- cooperative cancellation token + handle ownership in state machine.
4. Add a checklist item for `ContextRequest -> ContextTarget` resolver boundary.
5. Tighten handle/type ownership:
- make `SessionHandle` fields non-public (or `pub(crate)`),  
- clarify whether `ScopedAuthPort` or `TokenCacheStore` owns scoped token persistence to avoid dual ownership.

**Dropped From Original Reviews That Should Be Restored**
1. `ContextRequest` vs `ContextTarget` split ([claude-detail.md#L27](/Users/jay.ahn/projects/infra/nexttui/.worktrees/runtime-context-switch/devflow-docs/inception/design-review-raw/claude-detail.md#L27)).  
2. Explicit concurrency/locking expectation for state machine access ([claude-detail.md#L8](/Users/jay.ahn/projects/infra/nexttui/.worktrees/runtime-context-switch/devflow-docs/inception/design-review-raw/claude-detail.md#L8)).  
3. Indicator timeout dependency on redraw/tick policy ([claude-detail.md#L83](/Users/jay.ahn/projects/infra/nexttui/.worktrees/runtime-context-switch/devflow-docs/inception/design-review-raw/claude-detail.md#L83)).

**17-Item Checklist Completeness**
Needs additions. Add at least:
1. Commit-failure rollback/atomic contract.
2. Cancel-during-switching execution model.
3. `ContextRequest -> ContextTarget` type split.
4. State machine synchronization contract (single-thread invariant or mutexed state).

**PR1 Implementability**
Not fully ready for deterministic RED-phase tests yet.  
Most seams are present, but commit-failure semantics, cancel semantics, and request/target typing are still underspecified, so some failure-path tests remain ambiguous.

**Verdict**
**APPROVE-WITH-CHANGES**  
Synthesis captures most major findings well, but it introduces a non-compilable procedure snippet and leaves two spec-level blockers (cancel model, request/target boundary) that should be fixed before adopting as DETAIL r2.
