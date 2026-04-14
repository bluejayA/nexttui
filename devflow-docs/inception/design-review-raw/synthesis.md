# Council Synthesis — application-design.md (LIST)

**Chair**: Claude
**Reviewers**: Codex (REJECT), Gemini (APPROVE-WITH-CHANGES), Claude (APPROVE-WITH-CHANGES)
**Verdict (synthesized)**: **APPROVE-WITH-CHANGES (LIST 개정 후 DETAIL 진입)**

---

## 0. 메타 이슈 (선결)

Codex와 Gemini가 공통 지적: **워크트리의 `requirements.md`가 옛 ViewContext 리팩토링 버전**이었음.
→ **원인**: requirements.md가 git-tracked, 워크트리가 HEAD에서 옛 버전을 체크아웃. 메인에 새로 쓴 BL-P2-031 버전이 워크트리로 동기화되지 않음.
→ **조치**: 메인의 BL-P2-031 requirements/workflow-plan/state/audit/session-summary를 워크트리로 복사 완료. Codex의 Top Issue 1은 환경 문제로 종결.
→ **재발 방지**: 추후 inception 단계의 모든 추가 산출물은 워크트리에서 작성. 게이트 통과 시점에 `git add devflow-docs/` 커밋해서 동기화 갈등을 차단.

---

## 1. 합의 사항 (3-AI 일치)

### A. App ↔ ContextSwitcher 소유관계가 LIST에 미정의 (전원 합의)
- **Codex**: App을 Service에서 Controller로 재분류, ContextSwitcher가 atomic boundary 보유
- **Gemini**: ContextSwitcher가 Commit 결과로 new epoch 반환, 또는 App이 `increment_epoch()` 제공
- **Claude**: "App owns Switcher; Switcher mutates context via App-provided handle" 명문화

→ **결정**: LIST에 의존 방향 한 줄 추가. App이 Switcher를 소유, Switcher는 commit 결과로 `(new_epoch, snapshot)` 반환.

### B. PR 시퀀싱이 unsafe intermediate state를 만든다 (전원 합의)
- **Codex**: PR3/PR4가 PR5 안전 가시성 전에 사용자 노출 → 컨텍스트 인디케이터 없이 전환 가능
- **Gemini**: PR1 plumbing이 Action/AppEvent 변경까지 포함해야 — 범위 과소평가
- **Claude**: PR2가 PR1 미포함 시 stale 이벤트 누설

→ **결정**:
1. PR1 + PR2 통합 — "safety infra + switch core" 단일 PR (PR1만으로 사용자 노출 0이라 분리 가치 약함, 통합 시 stale 누설 창 자체 제거)
2. PR3/PR4 진입 전에 ContextIndicator + ConfirmDialog fingerprint 필수 — PR5의 일부를 **새로운 PR3**로 앞당김
3. PR 매핑에 `Depends on:` 컬럼 추가

### C. UI 상태 invalidation 신호 누락 (Gemini 단독 — 합의 채택)
- **Gemini**: ContextEpoch는 stale 이벤트 차단만 함. UI에 잔존 데이터(이전 cloud의 server list 등)는 사용자에게 거짓 정보 표시

→ **결정**: `AppEvent::ContextChanged { target }` 추가. resource_list / detail_view / 모든 모듈 컴포넌트가 이 이벤트 처리해 내부 데이터 비우기.

### D. 에피스 plumbing 범위 (Gemini + Claude)
- **Gemini**: Action/AppEvent 모두 epoch stamp 필요. 매 variant 수정 회피 위해 `VersionedEvent { event, epoch: u64 }` envelope
- **Claude**: 모든 spawn 시그니처가 `(epoch, cancel_token)` 페어 강제

→ **결정**: VersionedEvent envelope + WorkerSpawn API 개정으로 LIST에 명시.

---

## 2. Codex 단독 — 채택할 추가 컴포넌트 (Critical)

| 컴포넌트 | 책임 | 채택 사유 |
|---------|------|----------|
| `ContextSessionPort` (Port) + `ScopedAuthSession` (Service) | 활성 scoped token + endpoint cache 무효화의 atomic begin/commit/rollback | `AuthProvider`에 scope-switch API 부재. `KeystoneAuthAdapter.active_scope`는 fixed state. atomic boundary 없으면 rescope 성공 + stale endpoint 호출 사고 가능 |
| `EndpointCatalogInvalidator` (Service) | 모든 HTTP client의 endpoint cache 일괄 무효화 | `src/adapter/http/base.rs:66`의 매뉴얼 invalidate 자동화 |
| `ContextHistoryStore` (Util) | switch-back 1단계 히스토리 영속화 | 현재 LIST에 누락. Claude도 ContextSnapshot으로 동일 지적 |
| `ContextTargetResolver` (Service) | name/uuid/cloud-prefix → ContextTarget 변환, 충돌 disambiguation | 명령·피커·Identity `s` 액션 셋 모두 같은 로직 필요. 공유하지 않으면 3중 구현 |

→ **모두 채택**. ContextHistoryStore는 Claude의 ContextSnapshot과 통합.

## 3. 재분류 / 명명 변경 (Codex 합의)

| 컴포넌트 | 변경 | 사유 |
|---------|------|------|
| `ContextIndicator` | Controller → UI Widget | 단순 표시 위젯, Component trait 구현 (src/ui/context_indicator.rs) |
| `ContextPicker` | Controller → UI Widget (modal) | 동일 — src/ui/context_picker.rs |
| `App` | Service → Controller (orchestrator) | 코드베이스의 실제 App 역할이 라우터/오케스트레이터 |
| `CommandRegistry` | → `CommandParser 확장` | src/input/command.rs의 실제 명명에 일치 |
| `ContextSwitcher` | → `RuntimeContextSwitcher` (Claude 제안) | 보류 — 이름 길이/관행 검토 후 결정. 일단 ContextSwitcher 유지 |

## 4. 테스트 시즘 보강 (Claude 단독 — 채택)

- `port::auth::MockAuthProvider`에 rescope mock 추가 (기존 `src/port/mock.rs` 확장)
- `ContextSessionPort` 도입으로 fault-injection seam 자연 확보 (rescope OK + invalidate fail 시뮬레이션 가능)

## 5. 개정된 PR 매핑 (안)

| PR | 컴포넌트 | Depends on |
|----|---------|-----------|
| **PR1+2 통합** ("safety infra + switch core") | ContextEpoch + CancellationRegistry, VersionedEvent envelope, AppEvent::ContextChanged, Action epoch 필드, Worker epoch+cancel 검증, SwitchStateMachine, ContextSwitcher, ContextSessionPort + ScopedAuthSession, KeystoneRescopeAdapter, EndpointCatalogInvalidator, TokenCacheStore 확장, ContextHistoryStore, ContextTargetResolver, App 통합, port mock 확장 | — |
| **PR3** (안전 가시성 + 명령) | ContextIndicator (UI widget), StatusBar 통합, ConfirmDialog fingerprint, CommandParser 확장 (`:switch-*`, `:switch-back`) | PR1+2 |
| **PR4** (피커 UI) | ContextPicker (UI modal), KeyMap 글로벌 단축키 (Ctrl+P) | PR3 |
| **PR5** (Identity 통합) | Project Module 모듈-로컬 `s` 핸들러 (KeyMap 글로벌 등록 회피) | PR3 |

→ **PR 수: 6개 → 4개로 축소**. PR1+2 통합으로 stale 누설 창 제거, PR3에 안전 가시성 묶어 사용자 노출 시점에 안전성 보장.

## 6. 반려된 제안

- **Codex**: ContextSwitcher 명명 변경 — 보류 (코드베이스 전례 없으나 명확성에서 양호)
- **Gemini**: AppEvent에 epoch 직접 추가 — 거부, 대신 VersionedEvent envelope 채택 (variant 폭증 회피)

---

## 최종 Verdict

**APPROVE-WITH-CHANGES** — LIST를 위 결정대로 개정 후 DETAIL 진입.

### 개정 체크리스트 (LIST → DETAIL 진입 전)

- [ ] App ↔ ContextSwitcher 의존 방향 1줄 추가
- [ ] ContextSessionPort + ScopedAuthSession 추가 (Port + Service)
- [ ] EndpointCatalogInvalidator 추가
- [ ] ContextHistoryStore 추가
- [ ] ContextTargetResolver 추가
- [ ] AppEvent::ContextChanged 추가
- [ ] VersionedEvent envelope (Action/AppEvent) 추가
- [ ] port mock 확장 명시
- [ ] ContextIndicator/ContextPicker → UI Widget 재분류
- [ ] App → Controller 재분류
- [ ] CommandRegistry → CommandParser 확장으로 표기 변경
- [ ] PR1+PR2 통합, PR3에 안전 가시성 묶음
- [ ] PR 매핑 표에 Depends on 컬럼 추가
- [ ] PR6 KeyMap 분리 → PR5에서 모듈-로컬 핸들러로 변경
