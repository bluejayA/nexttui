# Requirements Analysis

**BL**: BL-P2-074
**Depth**: Standard
**Timestamp**: 2026-04-19T09:20:00+09:00 (revised)
**Parent BL**: BL-P2-031 (PR#76 머지 완료, stub 남김)
**Reference**: `.archive/inception-20260418-223706/requirements.md` (BL-P2-031 원본 FR-1)
**Review**: 3자 리뷰 반영 완료 (`inception/requirements-review-raw/`)

## User Intent

PR#76(BL-P2-031 PR3)에서 의도적으로 toast-only stub으로 남긴 `:switch-cloud <name>` 명령을 완결한다. **옵션 (a) — `ContextRequest::CloudOnly { cloud }` variant 도입 + resolver default-project 위임**을 채택한다. 사용자가 `:switch-cloud prod`를 입력하면 해당 cloud의 **명시적으로 선언된 선호 project** (clouds.yaml의 신규 필드 `CloudConfig::default_project`)로 전환된다.

**Default project source 결정** (H1 adversarial 리뷰 반영):
- `CloudConfig`에 **신규 필드 `default_project: Option<String>` 추가** — 기존 `auth.project_name`(Keystone bootstrap credential scope)과 분리.
- `auth.project_name` 재사용 금지 — 많은 환경이 `admin`/`service`로 고정되어 있어 runtime switch default로 쓰면 사용자가 실수로 admin scope에 착지할 수 있음 (OWASP A01 privilege 오남용 표면). PR#76 ConfirmDialog fingerprint가 파괴적 액션을 안전하게 만들려는 노력과 정면 충돌.
- `default_project`가 설정되지 않은 cloud에 대한 `:switch-cloud` 호출은 `SwitchError::NotConfigured` 반환 + toast로 `:switch-project <name>` 사용 유도.

**옵션 (b) picker 두 단계 플로우 거절 비용 근거**:
- Unit 6 ContextPicker는 별도 BL로 스코프 분리되어 있음 (BL-P2-075의 의존). 이 BL에서 picker를 먼저 짓는 것은 단일 PR에 "cloud-only variant + default 정책 + 신규 picker UI + 테스트" 4축을 압축하는 것 → 리뷰 부담 + 회귀 위험 증가.
- 현재 toast 힌트 "(picker: Ctrl+P — Unit 6)"는 **미래 기능 예고**이지 `:switch-cloud` 차단 계약이 아님. CloudOnly 경로는 picker와 orthogonal — picker 도입 후에도 "explicit choice 레이어 아래의 빠른 기본값"으로 자연스럽게 공존 (zsh의 `cd` vs fuzzy `cdi` 관계).
- **현재 BL은 stub 제거가 최우선 목표**. picker 통합은 Unit 6 착수 시 별도 BL로 병합.

## Functional Requirements

### FR-1. `:switch-cloud <name>` 즉시 전환 (Must)
- 사용자가 `:switch-cloud <cloud-name>` 입력 시, `Command::SwitchCloud(name)` → `Action::SwitchContext(ContextRequest::CloudOnly { cloud: name })` dispatch.
- 현재 toast-only stub (src/app.rs:1771-1780)을 실제 dispatch로 교체.
- 파서 변경 없음 — `Command::SwitchCloud(String)` 이미 존재 (src/input/command.rs:205 확인, `test_parse_switch_cloud` 통과).

### FR-2. `ContextRequest::CloudOnly { cloud }` variant (Must)
- `src/context/types.rs`의 `ContextRequest` enum에 `CloudOnly { cloud: String }` variant 추가. `ByName` / `ById`와 병렬.
- `ContextRequest`는 `#[non_exhaustive]`가 **없고** 구조적 매칭이 전제 → 컴파일러가 모든 매처 업데이트를 강제. 업데이트 필수 사이트 (grep 검증 완료):
  1. `src/context/resolver.rs` — `ContextTargetResolver::resolve` arm
  2. `src/context/switcher.rs` — `SwitchContextSwitcher` 내부 매처
  3. `src/worker.rs` — context-switch dispatch 경로
  4. `src/app.rs` — handler / tests
- clippy `-D warnings`에 의존하지 않고 사전 컴파일로 검증.

### FR-3. Resolver default-project 위임 (Must)
- `ContextTargetResolver::resolve(CloudOnly { cloud })` 경로 구현.
- 해결 알고리즘:
  1. cloud가 known_clouds에 있는지 확인 (없으면 `SwitchError::NotFound`).
  2. `CloudDirectory::default_project(cloud)` 조회 (신규 trait 메서드, `CloudConfig::default_project` 기반).
  3. `None` → `SwitchError::NotConfigured { cloud }` (FR-8로 신설).
  4. 있으면 그 project 이름으로 `ByName` 경로 재사용 → disambiguation + `ContextTarget` 반환.
- `CloudDirectory` trait에 `default_project(cloud: &str) -> Option<String>` 추가.

### FR-4. 이미 해당 cloud의 default project에 있을 때 no-op (Should)
- `:switch-cloud <current-cloud>` 입력 시 resolver가 반환한 `ContextTarget`이 현재 활성 `ContextTarget`과 동일하면, state_machine transition 생략 + `ToastLevel::Info` "already on <cloud> • <project>" 발행.
- **Acceptance 측정 기준** (quality P0 + spec Important 반영):
  - 단위 테스트: `SwitchContextSwitcher` 또는 상위 dispatcher에서 동일 target 재입력 시 `Switching` 상태 전이 카운터가 0 증가함을 assert.
  - 구현 위치 결정 (설계 단계): switcher 내부 idempotent 체크 (쉬움) vs app-level pre-check (명시적). **예비 결정: switcher 내부**, application-design에서 최종 확정.

### FR-5. Error 경로 가시적 처리 (Must)
- unknown cloud → toast `cloud '<name>' not found` (`SwitchError::NotFound` 매핑, safe_display 적용 — **60자 truncate + 제어문자 제거**).
- `CloudConfig::default_project` 없음 → toast `cloud '<name>' has no default project — use :switch-project <name>` (`SwitchError::NotConfigured` 매핑, safe_display 적용).
- auth 블록 자체가 None인 clouds.yaml 엔트리 → `CloudConfig::default_project`도 None으로 coalesce → 위와 동일 NotConfigured 경로 (spec Critical 반영).
- `default_project` 설정됐으나 Keystone에 없음(stale) → 기존 resolver `NotFound` 경로.
- 0 projects available (keystone empty list) → `NotFound` 경로에 흡수.
- 모든 에러는 `ToastLevel::Error`, 상태 전이 없음, help toast(src/app.rs:1758)와 guidance 문구 일관성 유지.

### FR-6. Legacy :ctx와 분리 유지 (Must)
- `Command::ContextSwitch` / `Command::ContextList`는 PR3와 동일하게 toast-only 유지. 이번 BL에서 변경하지 않음.
- BL-P2-075 (Unit 6 이후 deprecate)가 처리.

### FR-7. 테스트 (Must)
**단위 테스트**:
- `ContextRequest::CloudOnly` variant 생성/매칭 테스트 (types.rs).
- Resolver 성공 경로: `CloudOnly { cloud: "devstack" }` → default_project 기반 `ContextTarget` 반환.
- Resolver 실패 경로 3종:
  - unknown cloud → `SwitchError::NotFound`
  - default_project 없음 → `SwitchError::NotConfigured`
  - default_project 설정되어 있으나 Keystone에 없음 → `SwitchError::NotFound`
- FR-4 idempotency: 동일 target 재입력 시 transition 카운터 불변.

**`CloudDirectory` trait impl 4개 사이트 전수 업데이트 및 테스트** (quality P0 반영):
1. `src/context/config_cloud_directory.rs::ConfigCloudDirectory` — `default_project(&str)` 실제 구현 (CloudConfig 참조).
2. `src/context/resolver.rs::tests::FakeClouds` — 테스트용 stub.
3. `src/app.rs::tests::FakeClouds` (~line 3444) — stub.
4. `src/context/switcher.rs::tests::FakeClouds` (~line 213) — stub.

**통합 테스트** (src/app.rs::tests):
- `:switch-cloud prod` 입력 → `Action::SwitchContext(ContextRequest::CloudOnly { cloud: "prod" })`가 **정확히 1회** emit됨 (spec Important 반영).
- 기존 `test_command_bar_switch_cloud_emits_info_toast`는 **신규 동작으로 대체** — 성공 경로는 dispatch assert, 실패 경로는 `ToastLevel::Error` + 메시지 substring assert.
- help toast(src/app.rs:1758)의 "switch-cloud" 문자열과의 regex 충돌 점검 (quality P1 반영).

### FR-8. `SwitchError::NotConfigured { cloud: String }` variant (Must)
- `src/context/error.rs`에 variant 추가.
- 용도: cloud가 `default_project` 설정 없이 `:switch-cloud`로 접근되었을 때.
- **전용 variant 도입 이유** (H3 반영): 문자열 substring으로 fallback하면 BL-P2-053(SwitchError 확장 BL) 착수 시 테스트 + 사용자 facing message 양쪽이 깨짐. 지금 variant 하나 추가가 migration 비용 최소.
- `SwitchError`는 `#[non_exhaustive]`가 없어 매처 전수 업데이트 필요 (FR-2와 동일 사이트 집합 + error 경로 핸들러).

## Non-Functional Requirements

### NFR-1. 동시성 격리 (기존 invariant 유지)
- 변경 없음. `CloudOnly` dispatch도 기존 epoch/state_machine 경로를 탄다. 추가 spawn이나 race 없음.

### NFR-2. 테스트 회귀 0건
- 기존 1314 tests 전부 통과 유지. PR3에서 커버한 `test_command_bar_switch_cloud_emits_info_toast`는 dispatch 검증으로 대체됨.
- `CloudDirectory` trait 확장으로 4개 impl 사이트 재컴파일 → 미업데이트 시 즉시 컴파일 실패로 탐지.

### NFR-3. 코드 규모 (Guidance only, not acceptance)
- 예상 diff: +150~250 lines (variant + NotConfigured variant + resolver arm + `default_project` accessor + 4 impl 사이트 + handler + 테스트). 단일 PR. **acceptance 기준이 아닌 가이드라인**.

### NFR-4. 보안 / OWASP
- 신규 외부 입력: `CloudConfig::default_project` (clouds.yaml 사용자 제어 문자열). 신뢰할 수 있으나 toast 표시 시 **`safe_display(&str, max_len=60)` 유틸 적용 필수** — 제어문자 / CR-LF injection / 터미널 이스케이프 방지.
- cloud name도 마찬가지 (파서 레벨 sanitization이 있어도 방어심층).
- 위험 scope 착지 방지: `default_project`를 명시적 필드로 분리한 것이 핵심 mitigation (H1 반영).

### NFR-5. Clippy `-D warnings` 유지
- `ContextRequest` / `SwitchError` 모두 `#[non_exhaustive]` 부재로 컴파일러가 매처 업데이트를 강제.

### NFR-6. Observability / Tracing
- `CloudOnly` resolve/dispatch 경로에 기존 `tracing::info_span!` 상속 + 필드 추가:
  - `cloud=<input>` — 사용자 입력 그대로
  - `resolved_project=<name>` — resolver가 선택한 default project
- 실패 시 `tracing::warn!` 이벤트 1건 (error variant 매핑).

## Technology Stack

(brownfield — workspace.md 참조, 변경 없음)

| 계층 | 선택 | 소스 | 비고 |
|------|------|------|------|
| Language | Rust (edition 2024) | Brownfield | 변경 없음 |
| TUI | ratatui 0.30 + crossterm 0.29 | Brownfield | 변경 없음 |
| Async | tokio + tokio-util | Brownfield | 변경 없음 |
| Test | built-in `#[cfg(test)]` | Brownfield | 변경 없음 |

## Assumptions

1. **Default project source**: 신규 `CloudConfig::default_project: Option<String>` 필드가 유일한 default source. `auth.project_name` fallback은 **하지 않음** — 의미 충돌 방지 (H1).
2. **`CloudDirectory` trait 확장**: 3개 메서드로 확장 (`active_cloud` / `known_clouds` / `default_project`). 4개 impl 사이트 전수 업데이트 (config_cloud_directory + 3 FakeClouds).
3. **`ContextRequest` 매처 사이트 전수 조사 완료**: resolver / switcher / worker / app — 컴파일러가 누락을 막음.
4. **State machine transition**: CloudOnly도 일반 ByName과 동일한 transition 규칙(Idle→Switching→Committed/Failed). 추가 상태 없음.
5. **이미 활성 project와 동일할 때**: resolver가 Ok(ContextTarget)를 반환하면 switcher 또는 app-level pre-check가 no-op 판단. FR-4 acceptance로 측정 가능. 구현 위치는 application-design에서 확정.
6. **Unit 6 ContextPicker 미구현 상태 유지**: 이 BL은 picker를 건드리지 않음. toast 힌트 "(picker: Ctrl+P — Unit 6)"도 그대로 — 미래 기능 예고로 해석.
7. **`:switch-back` 시맨틱 (post-CloudOnly)**: `CloudOnly { cloud }` → resolver가 `ContextTarget`으로 변환한 뒤부터는 기존 ByName 경로와 동일. 즉 `:switch-back`은 **pre-switch `ContextTarget`**으로 복귀 (pre-switch request가 아님). `ContextHistoryStore`는 이미 `ContextSnapshot`만 저장하므로 추가 작업 없음 (M3 반영).
8. **clouds.yaml 스키마 확장**: `default_project` 필드는 `Option<String>` → 기존 clouds.yaml 파일과 100% backward compatible (serde(default)). 미설정 시 `None`.

## Open Questions

없음. 설계 단계(application-design)에서 확정할 항목:
- FR-4 idempotent 체크의 실제 위치 (switcher 내부 vs app-level pre-check)
- `SwitchError::NotConfigured` 표시 메시지 최종 문구
- `default_project` config 스키마 위치 (cloud 레벨 직하 vs 기존 subfield 내부)

## Change Log

- 2026-04-18T22:45 INITIAL — BL-P2-074 단독 requirements, 옵션 (a) 채택.
- 2026-04-19T09:20 REVISE — 3자 리뷰(spec/quality/adversarial) 반영. 주요 변경:
  - H1: `auth.project_name` → 신규 `CloudConfig::default_project` 필드로 전환 (의미 충돌 해소)
  - H3: `SwitchError::NotConfigured` 전용 variant 신설 (FR-8)
  - FR-4 measurable acceptance 추가 (transition 카운터 불변 테스트)
  - FR-7 확장: 4개 impl 사이트 + 4개 매처 사이트 + emit 1회 + failure 3종
  - NFR-4: toast safe_display 의무화
  - NFR-6: tracing 필드 커버 추가
  - Assumption 7: `:switch-back` post-CloudOnly 시맨틱 명시
  - Option (b) 거절 비용 근거 보강
  - NFR-3 "guidance only"로 완화
