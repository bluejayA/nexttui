# Claude Review — application-design.md (LIST)

**Reviewer**: Claude (self-critique)
**Mode**: Internal review focused on hidden coupling and PR safety

## Top 3 Issues

### 1. ContextEpoch와 CancellationRegistry의 책임 경계가 모호
- **What**: 두 컴포넌트가 모두 "이전 컨텍스트 작업 무효화"라는 단일 목적을 향함. epoch는 이벤트 검증에, registry는 fetch 취소에 쓰이지만, 이벤트 발행 시점에 epoch를 캡처한 task가 token cancel과 분리되면 race가 발생할 수 있음.
- **Why it matters**: PR1에서 이 둘을 분리해 도입하면, 한쪽만 수정된 폴링 루프가 만들어질 가능성. 또한 누가 epoch를 발급(App만? Switcher도?)하는지가 LIST에 명시되지 않음.
- **Suggestion**: 두 컴포넌트를 `ContextLifecycle` 단일 Service로 합치거나, "epoch 발급은 SwitchStateMachine 전유"로 명문화. 모든 spawn에서 `(epoch, cancel_token)` 페어를 함께 받도록 워커 시그니처 강제.

### 2. App과 ContextSwitcher의 호출 방향 / 소유관계가 미정의
- **What**: 변경 컴포넌트 표에 App이 "ContextSwitcher 통합, epoch 보유"라고 적혀있지만, 누가 누구를 호출하는지(App→Switcher, Switcher→App, 또는 양방향) 미명시.
- **Why it matters**: DETAIL 단계에서 메서드 시그니처를 그릴 때 순환 의존이 발견되거나, App이 비대해질 위험. 특히 Switcher가 catalog/token cache까지 무효화해야 하므로 App을 통해 일방향으로 라우팅할지 결정 필요.
- **Suggestion**: LIST에 의존 방향을 1줄 추가 — "App owns Switcher; Switcher mutates context via App-provided handle". 이로써 PR2 설계가 쉬워짐.

### 3. PR2 단독으로 머지 가능한가? — Worker 수정(PR1) 미반영 시 unsafe
- **What**: PR1(Worker epoch 검증)이 머지되기 전 PR2(SwitchStateMachine + ContextSwitcher) 단독으로는 컨텍스트 전환이 동작하나, 이전 컨텍스트의 폴링이 그대로 살아남음. 즉 PR2를 PR1 의존으로 명시하지 않으면 reviewer가 순서를 바꿀 수 있음.
- **Why it matters**: feature 브랜치 안에서 누적되더라도, 중간 검증 단계에서 PR2만 활성화되면 stale 이벤트 사고를 재현 가능. PR 의존성 그래프가 LIST에 없음.
- **Suggestion**: PR 매핑 표에 `Depends on:` 컬럼 추가. 또는 PR1+PR2를 합쳐 단일 PR로 바꾸는 게 안전 (PR1만으로는 사용자에게 보이지 않으므로 PR1을 별도로 머지할 가치는 낮음).

## 추가/제거/병합/분리 제안

- **병합 검토**: ContextEpoch + CancellationRegistry → `ContextLifecycle` (단일 Service)
- **병합 검토**: PR1 + PR2 → 단일 PR ("safety infra + switch core") — PR1만으로 사용자 노출 0이라 분리 가치 약함
- **추가 검토**: `ContextSnapshot` (Util) — switch-back 1단계 히스토리 + rollback에서 공통으로 사용 (현재 SwitchHistory가 빠져있음 — 본문엔 책임만 언급)
- **분리 검토**: KeyMap 변경이 PR4(Ctrl+P)와 PR6(`s`)에 양쪽 등장 — 같은 파일을 두 PR이 건드리면 conflict 위험. PR6에서 `s` 추가는 Project Module 내부 핸들러로 끝낼 수 있는지 재검토
- **명명 검토**: `ContextSwitcher` → `ContextSwitchService` 또는 `RuntimeContextSwitcher`로 좀 더 명확화 (코드베이스에 'Switcher' 명명 전례 부재)

## PR 경계 우려

| PR | 우려 |
|----|------|
| PR1 단독 머지 | 사용자 노출 0이지만 폴링 시그니처 변경이 모든 모듈에 영향. 회귀 리스크 분산을 위해서라도 PR2와 묶을 가치 있음 |
| PR3 (명령) | PR2 미머지 상태에서 명령 등록 시 동작 불가능 — 의존 명시 필수 |
| PR4 (피커) | KeyMap이 PR6과 겹침. 글로벌 단축키만 PR4에서 처리하고 모듈 내 키는 PR6에서 처리하도록 분리 명시 |
| PR5 (안전 가시성) | ConfirmDialog 변경이 destructive 액션이 있는 모든 모듈에 영향. 단일 PR이 너무 광범위해질 수 있음 |

## 테스트 관점 누락

- LIST에 "테스트 전용 mock" 컴포넌트가 빠져있음 — `KeystoneRescopeAdapter`의 mock 구현이 port에 추가되어야 (기존 `src/port/mock.rs` 확장)
- ContextSwitcher의 부분 실패 (rescope 성공 + catalog 실패) 시뮬레이션을 위한 fault-injection seam이 어디에 들어갈지 LIST에 없음

## Verdict

**APPROVE-WITH-CHANGES**

이유: 컴포넌트 분해는 합리적이고 PR 매핑 의도도 분명. 다만 (1) 의존 방향 명시, (2) PR1+PR2 병합 검토, (3) ContextEpoch+CancellationRegistry 책임 경계 명문화 — 세 가지를 LIST에 반영한 뒤 DETAIL로 가는 것이 안전.
