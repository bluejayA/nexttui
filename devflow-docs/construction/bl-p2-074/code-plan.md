# Code Generation Plan: BL-P2-074 (SwitchCloud wire 완결)

> **For agentic workers:** REQUIRED: Use `aidlc:aidlc-code-generation` with the
> "GENERATE" signal to execute this plan. Do NOT implement ad-hoc.
> `"code-generation: GENERATE — proceed with the approved plan for bl-p2-074"`

**BL**: BL-P2-074
**Timestamp**: 2026-04-20T08:50:00+09:00 (v1), 2026-04-20T09:10:00+09:00 (v2 — R1 리뷰 반영)
**Design source**: `devflow-docs/inception/application-design.md` (D1~D4 확정)
**Requirements source**: `devflow-docs/inception/requirements.md` (FR-1~8, NFR-1~6)
**Review response**: R1 code-plan-reviewer 결과 반영. Critical 2건 + Important 4건 + Suggestion 3건 처리. safe_display는 BL-P2-050으로 defer.

## Files to Modify

- [x] `src/config.rs` — `CloudConfig::default_project: Option<String>` 필드 + backward-compat 테스트 (FR-2, NFR-2)
- [x] `src/context/types.rs` — `ContextRequest::CloudOnly { cloud }` variant (FR-2)
- [x] `src/context/error.rs` — `SwitchError::NotConfigured { cloud }` + Clone impl 확장 + 테스트 (FR-8)
- [x] `src/context/resolver.rs` — `CloudDirectory::default_project` trait 메서드 + `resolve(CloudOnly)` arm + tests 확장 (FR-3, NFR-6)
- [x] `src/context/config_cloud_directory.rs` — `default_project` production impl + 테스트 (FR-3)
- [x] `src/context/switcher.rs` — D1 idempotent 체크 (α: state() 재사용) + tests FakeClouds 확장 + `test_switcher_noop_on_same_target` + tests 헬퍼 `by_name` 주변의 `ContextRequest` 매치 노출 (C1) (FR-4, NFR-6)
- [N/A] `src/worker.rs` — `ContextRequest` 매처 arm 추가 (~line 790 영역): **변경 불필요 확인** — worker.rs에서 `ContextRequest`를 직접 매치하지 않음. 컴파일러 exhaustiveness 에러 없음. (FR-2 exhaustiveness — 자동 충족)
- [x] `src/app.rs` — `:switch-cloud` handler stub 교체 + tests FakeClouds 확장 + dispatch 테스트(+toast 미발행 assertion, I3) (FR-1, NFR-6)

**Out of scope (BL-P2-050으로 defer)**: NFR-4의 toast `safe_display(60자 truncate + 제어문자)` 유틸 신설. `rg safe_display src/` = 0 match 확인 → 신규 유틸 결정사항 4개 (위치/truncate 문자/제어문자 정책/호출 사이트) 생성 범위가 BL-P2-074 scope를 팽창시킴. BL-P2-050 "LogPanel 텍스트 정제"와 통합 처리.

## Tracing 이벤트 (NFR-6)
- `resolve_cloud_only` (resolver arm 진입) — 필드: `cloud`, `resolved_project`
- `switch_noop_same_target` (switcher idempotent 분기) — 필드: `cloud`, `project`
- `cloud_no_default_project` (NotConfigured 발생) — 필드: `cloud`
- toast 경로는 기존 span 상속으로 커버 (추가 이벤트 없음 — I4 문구 정정)

## Implementation Steps (TDD)

### Step 1: `SwitchError::NotConfigured` variant + Clone arm (I1 통합)
- [ ] RED: `src/context/error.rs::tests`에 두 테스트 추가 (동시 RED):
  1. `test_not_configured_displays_human_readable` — `SwitchError::NotConfigured { cloud: "prod".into() }.to_string() == "cloud 'prod' has no default project — use :switch-project <name>"`
  2. `test_clone_preserves_not_configured` — `NotConfigured { cloud: "prod".into() }.clone()` match로 동일 cloud 보존 assert
- [ ] Verify RED: 컴파일 실패 (variant 부재) → 두 테스트 모두 실패.
- [ ] GREEN:
  1. enum variant 추가: `#[error("cloud '{cloud}' has no default project — use :switch-project <name>")] NotConfigured { cloud: String }`
  2. Clone impl (error.rs:38-57) arm 추가: `Self::NotConfigured { cloud } => Self::NotConfigured { cloud: cloud.clone() }`
- [ ] Verify GREEN: 두 신규 테스트 + 전체 회귀 통과. `cargo build --lib --tests` — SwitchError 매처가 있다면 컴파일러가 즉시 잡음 (NotConfigured arm 없음). 발견 시 각 사이트에 `_` 또는 명시적 arm 추가 (로직 변경 없이 propagate). (FR-8, NFR-5)

### Step 2: `CloudConfig::default_project` 필드 + backward-compat + 긍정 경로 (I1 통합)
- [ ] RED: `src/config.rs::tests`에 두 테스트 추가:
  1. `test_load_clouds_yaml_without_default_project_yields_none` — 기존 valid_clouds_yaml() 로드 후 `None` assert
  2. `test_load_clouds_yaml_with_default_project_yields_some` — `default_project: my_project` 포함 YAML → `Some("my_project")` assert
- [ ] Verify RED: 컴파일 실패 (필드 부재).
- [ ] GREEN:
  1. `CloudConfig`에 `#[serde(default)] pub default_project: Option<String>` 필드 추가 (cloud 레벨 직하, auth 블록 **밖**).
  2. `Config::cloud(&str) -> Option<&CloudConfig>` accessor가 없으면 추가 (Step 3 의존).
- [ ] Verify GREEN: 두 신규 테스트 + 기존 config 테스트 전부 통과. (FR-2, NFR-2)

### Step 3: `CloudDirectory::default_project` trait 메서드 + 5 impl 사이트 (I1 Step 5/6 통합, S1 반영)
**S1 주의**: FakeClouds 구조가 3곳 이질적 (resolver.rs: struct with fields / switcher.rs 및 app.rs: unit struct). trait에 default 구현을 두지 **않는 설계 결정** (application-design.md L158)을 따름 → 각 FakeClouds의 구조에 맞춰 개별 확장.

- [ ] RED: `src/context/resolver.rs::tests`에 `test_cloud_directory_default_project_reflects_config` (FakeClouds 확장 기반) 추가. `src/context/config_cloud_directory.rs::tests`에 `test_default_project_returns_configured_value` + `test_default_project_none_when_unset` 추가. 컴파일 실패 (trait 메서드 부재).
- [ ] Verify RED: 컴파일 실패.
- [ ] GREEN:
  1. `CloudDirectory` trait에 `fn default_project(&self, cloud: &str) -> Option<String>;` 추가 (default 구현 없음).
  2. production impl: `ConfigCloudDirectory::default_project` — `self.config.cloud(cloud).and_then(|c| c.default_project.clone())`.
  3. `src/context/resolver.rs::tests::FakeClouds` — 기존 struct에 `defaults: HashMap<String, String>` 필드 추가 + impl 확장.
  4. `src/context/switcher.rs::tests::FakeClouds` — unit struct면 struct with defaults로 변환, 기존 생성자 호출 사이트 업데이트.
  5. `src/app.rs::tests::FakeClouds` (~line 3444) — 동일 패턴.
- [ ] Verify GREEN: 3 FakeClouds + ConfigCloudDirectory + trait = 5 impl 사이트 컴파일 통과. 전체 테스트 회귀 없음. (FR-3, NFR-2, S1)

### Step 4: `ContextRequest::CloudOnly` variant + exhaustiveness placeholder (C1 반영)
- [ ] RED: `src/context/types.rs::tests`에 `test_context_request_cloud_only_is_constructible` 추가. 컴파일 실패 (variant 부재).
- [ ] Verify RED: 컴파일 실패.
- [ ] GREEN:
  1. `ContextRequest` enum에 `CloudOnly { cloud: String }` variant 추가.
  2. 매처 exhaustiveness 에러 발생 → **모든 사이트에 placeholder arm 추가** (컴파일만 확보):
     - `src/context/resolver.rs::resolve` — `ContextRequest::CloudOnly { .. } => Err(SwitchError::Unsupported("CloudOnly — pending Step 5".into()))` (Step 5에서 교체)
     - `src/context/switcher.rs:321` **(C1 반영)** — switcher tests 영역에서 `ContextRequest::ByName {..}` 매치 존재. CloudOnly arm placeholder 추가 또는 `_` wildcard 사용. **선호: 명시적 arm**으로 Step 5 교체 지점 명확화.
     - `src/worker.rs` (~line 790) — `ContextRequest` 매처 존재 시 placeholder arm 추가.
     - `src/app.rs` — `ContextRequest` 매처 없음 (SwitchCloud는 `Command` 매처). 변경 없음.
- [ ] Verify GREEN: 전체 컴파일 통과 + 테스트 회귀 없음. (FR-2)
- [ ] REFACTOR: placeholder arm은 Step 5 이후 최종 로직으로 교체.

### Step 5: `ContextTargetResolver::resolve(CloudOnly)` + 실패 경로 3종 (S3 — 헬퍼 분리 기본값)
**S3 반영**: async_trait 재귀 호출의 BoxFuture 타입 위험 회피 — `resolve_by_name_inner(&self, cloud, project, domain)` 헬퍼를 **기본값**으로 추출. `resolve` 매칭은 이 헬퍼에 위임.

- [ ] RED: `src/context/resolver.rs::tests`에 네 테스트 추가:
  1. `test_resolve_cloud_only_returns_default_project_target` — 성공 경로
  2. `test_resolve_cloud_only_unknown_cloud_returns_not_found` — 실패 #1
  3. `test_resolve_cloud_only_no_default_returns_not_configured` — 실패 #2
  4. `test_resolve_cloud_only_stale_default_returns_not_found` — 실패 #3 (default 설정, keystone empty list)
- [ ] Verify RED: Step 4 placeholder 때문에 Unsupported 반환 → 실패.
- [ ] GREEN:
  1. `resolve_by_name_inner(cloud: String, project: String, domain: Option<String>)` 헬퍼 추출 (기존 ByName arm 본체 이동).
  2. `ByName` arm: `self.resolve_by_name_inner(cloud.unwrap_or(...), project, domain).await`.
  3. `CloudOnly` arm:
     ```rust
     ContextRequest::CloudOnly { cloud } => {
         self.validate_cloud(&cloud)?;
         let project = self.clouds.default_project(&cloud)
             .ok_or(SwitchError::NotConfigured { cloud: cloud.clone() })?;
         let span = tracing::info_span!("resolve_cloud_only", cloud = %cloud, resolved_project = %project);
         let _enter = span.enter();
         self.resolve_by_name_inner(cloud, project, None).await
     }
     ```
  4. `NotConfigured` 발생 경로에 `tracing::warn!("cloud_no_default_project", cloud = %cloud)`.
- [ ] Verify GREEN: 네 테스트 + 기존 resolver 테스트 회귀 없음. (FR-3, FR-5, FR-8, NFR-6, S3)
- [ ] REFACTOR: Step 4 placeholder arm 제거 (resolver는 final, switcher/worker는 여전히 placeholder OK — logic상 도달 불가).

### Step 6: `ContextSwitcher::switch` idempotent 체크 (D1 α, C2 정정)
**C2 반영**: `SwitchStateView::Idle { current: Option<ContextSnapshot> }` — `current`가 `Option`. 올바른 패턴은 `if let SwitchStateView::Idle { current: Some(snap) } = self.state.state() && snap.target == target`.

- [ ] RED: `src/context/switcher.rs::tests`에 두 테스트 추가:
  1. `test_switcher_noop_on_same_target` — initial target A commit 후 동일 target으로 switch → `Ok((epoch_before, snapshot_A))` 반환 + `state.epoch().current()` 첫 호출 이후 불변 + FakeSessionPort의 `begin_call_count == 1` (카운터 필드 추가 필요).
  2. `test_switch_back_after_cloud_only_returns_previous_target` (Step 12 병합, D4 검증) — FakeSessionPort 재활용 시나리오
- [ ] Verify RED: 현재 구현은 resolver 이후 `try_begin` → `rollback`으로 epoch bump → 실패.
- [ ] GREEN: `switch()` 내부 resolver 성공 직후 분기 추가
  ```rust
  if let SwitchStateView::Idle { current: Some(snap) } = self.state.state()
      && snap.target == target
  {
      tracing::debug!("switch_noop_same_target", cloud = %target.cloud, project = %target.project_name);
      return Ok((snap.epoch, snap));
  }
  ```
  `state.state()`의 반환 타입이 이미 `SwitchStateView`(Clone)인지 확인. `snap.epoch`의 타입 (`Epoch`) 확인.
- [ ] Verify GREEN: 두 테스트 + 기존 switcher 테스트 회귀 없음. (FR-4, D1, D4, NFR-6)
- [ ] REFACTOR: TOCTOU 주의 주석 추가 (application-design.md D1 참조). `run_switch_to` helper가 이미 있다면 분리 요구 없음.

### Step 7: `App::execute_command` SwitchCloud handler 교체 (I3 반영)
- [ ] RED: `src/app.rs::tests`에 `test_command_bar_switch_cloud_dispatches_context_request_without_toast` 추가 (I3 반영: dispatch emit 1회 + **toast 미발행** 확인). 기존 `test_command_bar_switch_cloud_emits_info_toast`는 **의도 변경** 방식으로 교체 (삭제보다 안전).
- [ ] Verify RED: 현재 구현은 toast 발행 → dispatch assertion 실패 + toast 미발행 assertion 실패.
- [ ] GREEN: `src/app.rs:1771-1780` stub 교체
  ```rust
  Command::SwitchCloud(name) => {
      self.dispatch_action(Action::SwitchContext(
          crate::context::ContextRequest::CloudOnly { cloud: name },
      ));
  }
  ```
  Help toast(src/app.rs:1758)의 `:switch-cloud <name>` 문구는 유지.
- [ ] Verify GREEN: 신규 테스트 통과, 전체 회귀 없음. dispatch 1회 + toast 미발행. (FR-1, I3)

### Step 8: 전체 회귀 + CI 게이트 (S2 — Step 13 흡수)
- [ ] GREEN: 아래 명령 전부 통과:
  - `cargo fmt --all --check`
  - `cargo test --lib --tests` (기존 1314 + 신규 12~14건)
  - `cargo clippy --lib --tests -- -D warnings`
  - `cargo build --bin nexttui`
- [ ] Verify GREEN: 4개 명령 clean. Step 4의 placeholder arm이 완전히 제거됐는지 (switcher/worker가 `SwitchError::Unsupported("...pending...")`를 반환하는 코드가 남아있지 않은지) grep 검증. (NFR-2, NFR-5)

## Test Strategy

| 테스트명 | 위치 | 검증 내용 | 연결 요구사항 |
|---|---|---|---|
| test_not_configured_displays_human_readable | error.rs | NotConfigured Display | FR-8 |
| test_clone_preserves_not_configured | error.rs | Clone arm 보존 | FR-8 |
| test_load_clouds_yaml_without_default_project_yields_none | config.rs | serde default backward-compat | FR-2, NFR-2 |
| test_load_clouds_yaml_with_default_project_yields_some | config.rs | 필드 역직렬화 | FR-2 |
| test_cloud_directory_default_project_reflects_config | resolver.rs (tests) | trait 메서드 | FR-3 |
| test_default_project_returns_configured_value | config_cloud_directory.rs | production impl | FR-3 |
| test_default_project_none_when_unset | config_cloud_directory.rs | 미설정 케이스 | FR-3 |
| test_context_request_cloud_only_is_constructible | types.rs | variant 생성 | FR-2 |
| test_resolve_cloud_only_returns_default_project_target | resolver.rs | 성공 경로 | FR-3 |
| test_resolve_cloud_only_unknown_cloud_returns_not_found | resolver.rs | 실패 unknown cloud | FR-5 |
| test_resolve_cloud_only_no_default_returns_not_configured | resolver.rs | 실패 no default | FR-5, FR-8 |
| test_resolve_cloud_only_stale_default_returns_not_found | resolver.rs | 실패 stale | FR-5 |
| test_switcher_noop_on_same_target | switcher.rs | idempotent epoch 불변 | FR-4, D1 |
| test_switch_back_after_cloud_only_returns_previous_target | switcher.rs | post-CloudOnly rollback | D4 |
| test_command_bar_switch_cloud_dispatches_context_request_without_toast | app.rs | dispatch 1회 + toast 없음 | FR-1, I3 |

**기존 테스트 변경**:
- `test_command_bar_switch_cloud_emits_info_toast` (app.rs:2409) → 신규 테스트로 의도 변경 교체 (삭제 아님).

**총 신규/변경 테스트**: 15건 (신규 15 + 기존 1 의도 변경).

## Verification Contract

### 완료 조건 (FR/NFR 번호 병기, I4 반영)
- [ ] `:switch-cloud <name>` 입력 시 `Action::SwitchContext(ContextRequest::CloudOnly { cloud })` dispatch 됨 **(FR-1)**
- [ ] `ContextRequest::CloudOnly` variant 존재 + 4 매처 사이트(resolver/switcher/worker/app) 컴파일 통과 **(FR-2)**
- [ ] `CloudConfig::default_project`가 clouds.yaml cloud 레벨에서 선택적으로 설정 가능, `#[serde(default)]`로 기존 YAML 100% backward compat **(FR-2, NFR-2)**
- [ ] `ContextTargetResolver`가 CloudOnly request를 default_project로 위임 (ByName 경로 재사용) **(FR-3)**
- [ ] `:switch-cloud <current-cloud>` 재입력 시 state_machine transition 카운터 불변 (epoch 동일, 순차 호출 한정) **(FR-4, D1)**
- [ ] unknown cloud / no default / stale default 3종 실패 경로 명확한 에러 variant 반환 + 적절한 toast 발행 **(FR-5)**
- [ ] Legacy `:ctx` 명령은 이번 BL에서 변경 없이 toast-only 유지 **(FR-6)**
- [ ] 신규/변경 15개 테스트 전부 통과, 기존 1314 테스트 회귀 0건 **(FR-7, NFR-2)**
- [ ] `SwitchError::NotConfigured { cloud }` 전용 variant + Clone arm **(FR-8)**
- [ ] `#[non_exhaustive]` 없는 `ContextRequest` / `SwitchError` 매처 컴파일 강제로 사이트 누락 방지 **(NFR-5)**
- [ ] tracing 이벤트 3종 (`resolve_cloud_only`, `switch_noop_same_target`, `cloud_no_default_project`) + toast 경로 기존 span 상속 **(NFR-6)**
- [ ] clippy `-D warnings` clean **(NFR-5)**
- [ ] 동시성 격리 (NFR-1) 기존 invariant 유지 — CloudOnly는 기존 epoch/state_machine 경로 재사용, 추가 spawn 없음

### Out of scope (BL-P2-050으로 defer)
- **NFR-4 toast `safe_display(60자)` 적용**: src/에 `safe_display` 유틸 부재. 신규 유틸 결정사항 4개 생성 필요 → BL-P2-050 "LogPanel 텍스트 정제"와 통합 처리. 이 BL에서는 cloud name 파서 레벨 sanitization에 의존 (기존 동작).

### 검증 명령
- `cargo test --lib --tests` — 전체 단위 + 통합 테스트
- `cargo clippy --lib --tests -- -D warnings` — clippy 게이트
- `cargo fmt --all --check` — 포맷 체크
- `cargo build --bin nexttui` — 바이너리 빌드

### 리스크 태그
- [x] `ContextRequest`/`SwitchError` enum 확장 — 매처 5+4 사이트 컴파일러 강제 (risk 낮음)
- [x] clouds.yaml 스키마 확장 — `#[serde(default)]`로 backward compat (risk 낮음)
- [ ] auth/security — 해당 없음 (신규 외부 입력 없음; safe_display는 BL-P2-050으로 defer하되 원본 파서 sanitization 유지)
- [ ] DB schema change — 해당 없음

## Status (완료 요약)

**2026-04-20T09:50 — GENERATE 완료 / 10:10 — R1 review 반영 (dead code 제거)**
- 모든 Step GREEN (1~8) — TDD RED→GREEN→REFACTOR 준수.
- **Tests**: 1314 (baseline) → **1328** (+14 신규, 기존 1 대체).
- **검증 명령 전부 통과**:
  - `cargo fmt --all --check` ✓
  - `cargo test --lib --tests` ✓ (1328/1328)
  - `cargo clippy --lib --tests -- -D warnings` ✓
  - `cargo build --bin nexttui` ✓
- **Stub 잔존 확인**: `grep "pending Step\|not available yet — use :switch-project" src/` → 0 매치 (doc 주석 제외).
- **FR/NFR coverage**: 7개 변경 컴포넌트 + 5 `CloudDirectory` impl 사이트 업데이트 + 4 매처 사이트 중 1개(worker.rs)는 컴파일러 불필요 확인 + 3 tracing 이벤트 추가 + D1 idempotent 설계 α 채택.
- **R1 리뷰 반영 (10:10)**: dead code 제거 — `Action::SwitchCloud(String)` enum variant(action.rs:206), test 참조(action.rs:269), worker stub arm(worker.rs:790-794) 3곳 삭제. caller migration 완료로 stub 주석 전제 해소. CI gate 재통과 확인(1328/fmt/clippy/bin).

## Change Log
- 2026-04-20T08:50 v1 — 초안 작성 (15 Steps)
- 2026-04-20T09:10 v2 — R1 리뷰 반영:
  - C1: Step 4 placeholder arm 목록에 `src/context/switcher.rs:321` 추가
  - C2: Step 6 idempotent 패턴을 `Idle { current: Some(snap) }`로 정정
  - I1: Step 1-2, 3-4, 5-6 병합 → 8 Steps로 축소
  - I2: Step 14 safe_display → BL-P2-050으로 defer
  - I3: Step 7에 toast 미발행 assertion + 테스트 의도 변경 교체 (삭제 아님)
  - I4: Verification Contract에 FR/NFR 번호 병기, tracing 3종 + 상속 문구 정정
  - S1: Step 3 FakeClouds 이질성 주의 추가
  - S2: Step 13을 Step 8(통합 게이트)에 흡수
  - S3: Step 5 async 재귀 회피를 위한 `resolve_by_name_inner` 헬퍼 기본값
