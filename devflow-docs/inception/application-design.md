# Application Design

**Mode**: DETAIL (상세 설계 단계)
**Timestamp**: 2026-04-20T08:10:00+09:00 (LIST), 2026-04-20T08:25:00+09:00 (DETAIL)
**BL**: BL-P2-074 (SwitchCloud wire 완결)
**Depth**: Standard

## Scope Note

BL-P2-074는 **신규 컴포넌트가 아닌 기존 컴포넌트 7개 확장**. `src/context/` 주요 타입과 `src/config.rs` + `src/app.rs`의 narrow surgical change. Single-component system으로 보기엔 trait/enum/config가 서로 얽혀있어 LIST로 정리할 가치 있음.

## 컴포넌트 목록 (확장/변경)

| 컴포넌트 | 책임 | 타입 |
|---------|------|------|
| `CloudConfig` | clouds.yaml cloud 설정 — 신규 `default_project: Option<String>` 필드 추가 (auth.project_name과 분리) | Util (config struct) |
| `ContextRequest` | switch 요청 타입 — 신규 `CloudOnly { cloud: String }` variant 추가 | Util (enum) |
| `SwitchError` | switch 실패 타입 — 신규 `NotConfigured { cloud: String }` variant 추가 | Util (enum) |
| `CloudDirectory` | cloud 메타데이터 접근 trait — 신규 `default_project(&str) -> Option<String>` 메서드 추가 | Interface (trait) |
| `ConfigCloudDirectory` | Config 래퍼 — `default_project` 구현 (CloudConfig 참조) | Repository |
| `ContextTargetResolver` | Request → ContextTarget 변환 — `CloudOnly` arm 신설 (default_project 위임 + `ByName` 경로 재사용) | Service |
| `App::execute_command` | `:switch-cloud` handler — toast-only stub 제거 → `Action::SwitchContext(CloudOnly { cloud })` dispatch | Controller |

총 7개 컴포넌트 확장.

## 영향을 받지만 변경 없는 컴포넌트 (컴파일러 강제 재확인)

아래 컴포넌트는 `ContextRequest` / `SwitchError`에 variant가 추가되며 매처가 자동 실패 → 업데이트 필수. 로직은 "CloudOnly를 ByName 경로로 위임"으로 수렴하므로 추가 책임 없음.

- `src/context/switcher.rs::SwitchContextSwitcher` — request match arm
- `src/worker.rs` — context-switch dispatch 경로 (~line 790)
- `CloudDirectory` impl 사이트 **5개** (production 1 + test doubles 4):
  - production: `src/context/config_cloud_directory.rs::ConfigCloudDirectory` (확장 — `default_project` 구현)
  - test: `src/context/resolver.rs::tests::FakeClouds`
  - test: `src/context/switcher.rs::tests::FakeClouds`
  - test: `src/app.rs::tests::FakeClouds` (~line 3444)
  - test: 기타 추가 test double (신규 테스트 추가 시)

## 확정된 설계 결정

### D1. FR-4 idempotent 체크 위치 → **`ContextSwitcher::switch` 내부**, resolver 직후 / `try_begin` 직전

**근거**: `src/context/switcher.rs:64-77`의 step 1→2 경계. resolver가 성공(Ok)한 후 `state.try_begin`이 epoch을 bump하기 전에 비교해야 epoch 낭비 없음. app-level pre-check는 resolver를 거치지 않아 default_project가 이미 현재 project인지 판단 불가.

```
현재 step:  resolve → try_begin(epoch++) → cancel → session.begin → ... → commit
신규 step:  resolve → [IDEMPOTENT CHECK] → try_begin → ...
```

**알고리즘** (두 옵션, 구현 단계에서 택일):
- **옵션 α (권장)**: 기존 `self.state.state()` 재사용 — `SwitchStateView::Idle { current }`에서 `current: ContextSnapshot` 추출. 신규 메서드 불필요.
- **옵션 β**: `SwitchStateMachine::current_snapshot() -> Option<ContextSnapshot>` 얇은 read-only accessor 신규 추가 (RwLock peek). Idle 분기가 아닌 다른 상태에서도 "마지막 committed" 제공 가능.

선택 기준: `state_machine.rs:136-178` 확인 결과 `state()` + `previous_in_flight()`는 존재하나 `current_snapshot()`은 부재. 단순성 + delta 최소화 원칙으로 **α 우선**.

**TOCTOU 주의** (I1 리뷰 반영):
- peek → `try_begin` 사이에 동시 switch가 끼어들어 `try_begin`을 먼저 잡는 race는 현재 알고리즘에서 **"의도상 no-op인데 `InProgress` 반환"** 시나리오를 만듦.
- state_machine의 check-and-bump가 이미 Mutex로 원자화되어 있으므로 loser가 `InProgress`를 받는 동작 자체는 일관. FR-4의 "동일 target 재입력 no-op" acceptance는 **단일 호출자 컨텍스트에서만** 보장함을 명시 — 동시 호출 시 InProgress는 허용.
- 측정 테스트는 순차 2회 호출로 한정 (동시성 race는 별도 테스트가 아님).

**측정 (FR-4 acceptance)**: `test_switcher_noop_on_same_target`에서 switch 2번 순차 호출 후 `state.epoch().current()`가 불변임을 assert.

### D2. `SwitchError::NotConfigured` 메시지 최종 문구

```rust
#[error("cloud '{cloud}' has no default project — use :switch-project <name>")]
NotConfigured { cloud: String },
```

- `#[error(...)]` 안에 cloud 값을 직접 포함 (safe_display는 toast 레이어에서 60자 truncate 적용 — Display는 정직한 원문).
- `:switch-project <name>` 리터럴 유지 — Help toast(src/app.rs:1758)와 동일 표기.
- **struct variant 선택 근거** (S1 리뷰 반영): 기존 `SwitchError`는 대부분 tuple variant(`NotFound(String)`, `RescopeRejected(String)`)지만 `Ambiguous { candidates: Vec<ContextTarget> }` 전례가 있음. 향후 "왜 not configured인지" 맥락 확장(예: `reason: MissingField | ExplicitNull`)을 위해 struct variant로 선택. 현재는 `cloud` 한 필드.

### D3. `CloudConfig::default_project` 직렬화 위치 → **cloud 레벨 직하**

```yaml
clouds:
  prod:
    auth:
      auth_url: https://keystone/v3
      username: admin
      password: secret
      project_name: admin        # bootstrap scope (unchanged)
    default_project: my_workload  # 신규 — runtime switch-cloud default
    region_name: RegionOne
```

**근거**:
- `CloudConfig`에 이미 `region_name`, `identity_api_version` 등 cloud 레벨 필드가 존재. `default_project`도 동일 레벨에 배치하면 일관성 유지.
- `auth.default_project`로 두면 bootstrap scope와 섞여 개념 혼동 재발 (H1 adversarial 문제 반복).
- `app.default_project`는 app-level 설정(`AppConfig`)과 충돌 — AppConfig는 single-value, CloudConfig는 per-cloud.

**serde 호환성**:
- `#[serde(default)]` 적용 → 기존 clouds.yaml 100% backward compatible.
- `CloudConfig` 자체는 `Deserialize` 외에 `Serialize`도 이미 구현(config.rs:53). 신규 필드도 파생 매크로로 자동.

**Backward-compat 테스트 acceptance** (S3 리뷰 반영):
- `test_load_clouds_yaml_without_default_project_yields_none` — `default_project` 필드가 없는 기존 YAML 로드 시 `CloudConfig::default_project == None` 검증.
- `test_load_clouds_yaml_from_standard_path`(config.rs:509)와 동일 테스트 헬퍼 패턴 재활용.

### D4. `ContextSnapshot`의 CloudOnly 원본 보존 → **보존하지 않음** (Assumption 7 확정)

**근거**:
- `:switch-back`은 "이전 `ContextTarget`으로 되돌아간다" — resolved target이 복귀 대상. Request origin(ByName vs CloudOnly)은 rollback 시맨틱에 영향 없음.
- `ContextHistoryStore`는 이미 `ContextSnapshot`만 저장. 변경 없이 재활용.
- 만약 "어떻게 들어왔는지" 추적이 필요하면 `tracing` 필드로 충분 (NFR-6).

**테스트**: `test_switch_back_after_cloud_only_returns_previous_target` — CloudOnly로 진입 후 switch-back → 원래 target 반환 확인.

## 컴포넌트 상세 설계

### CloudConfig (확장)
**Responsibility**: clouds.yaml에 선언된 cloud별 설정을 담는 serde struct. 신규 `default_project` 필드로 runtime `:switch-cloud`의 기본 전환 대상을 명시.
**Interface**:
- `pub default_project: Option<String>` — `#[serde(default)]`. None이면 `:switch-cloud <this>` 호출 시 `NotConfigured`.
- 기존 getter/setter는 변경 없음.
**Dependencies**: 없음 (pure data struct, `Clone + Deserialize + Serialize`).

### ContextRequest (확장)
**Responsibility**: 사용자 입력을 resolver 입력 형태로 정규화한 unresolved request.
**Interface** (enum variant 추가):
```rust
pub enum ContextRequest {
    ByName { cloud: Option<String>, project: String, domain: Option<String> },
    ById   { cloud: Option<String>, project_id: String },
    CloudOnly { cloud: String }, // 신규
}
```
**Dependencies**: 없음. 매처 4개 사이트 업데이트 강제 (resolver/switcher/worker/app) — `#[non_exhaustive]` 부재로 컴파일 실패가 차단.

### SwitchError (확장)
**Responsibility**: switch 경로 실패 타입.
**Interface**:
```rust
pub enum SwitchError {
    // ... 기존 9개 variant ...
    #[error("cloud '{cloud}' has no default project — use :switch-project <name>")]
    NotConfigured { cloud: String }, // 신규
}
```
**Dependencies**:
- `Clone` impl 수동 확장 (error.rs:38-57): `Self::NotConfigured { cloud } => Self::NotConfigured { cloud: cloud.clone() }`.
- 사용자 facing 경로: resolver → switcher → app toast. Toast 레이어에서 `safe_display(60)` 적용.
- **Clone 테스트 acceptance** (S2 리뷰 반영): `error.rs::tests`에 `test_clone_preserves_not_configured` 추가 — clone이 cloud 문자열을 보존함을 assert.

### CloudDirectory (trait 확장)
**Responsibility**: cloud 메타데이터의 read-only 추상화 (resolver가 Config 구현에 직접 의존하지 않도록 분리).
**Interface**:
```rust
pub trait CloudDirectory: Send + Sync {
    fn active_cloud(&self) -> String;
    fn known_clouds(&self) -> Vec<String>;
    fn default_project(&self, cloud: &str) -> Option<String>; // 신규
}
```
**Dependencies**: 구현체 4개(ConfigCloudDirectory + 3 FakeClouds) 전수 업데이트. default 구현을 trait에 두지 않음 — 테스트 stub이 명시적으로 데이터를 넣도록 강제.

### ConfigCloudDirectory (impl 확장)
**Responsibility**: `Arc<Config>` 기반 `CloudDirectory` 실제 구현.
**Interface**:
```rust
impl CloudDirectory for ConfigCloudDirectory {
    fn default_project(&self, cloud: &str) -> Option<String> {
        self.config.cloud(cloud).and_then(|c| c.default_project.clone())
    }
    // 기존 active_cloud, known_clouds 그대로
}
```
**Dependencies**:
- `Config::cloud(&str) -> Option<&CloudConfig>` accessor 필요 (기존 미존재 시 추가).
- `CloudConfig::default_project` 필드 (D3 참조).

### ContextTargetResolver (확장)
**Responsibility**: Request → ContextTarget 변환 + disambiguation.
**Interface** (`resolve` 매칭 arm 추가):
```rust
ContextRequest::CloudOnly { cloud } => {
    self.validate_cloud(&cloud)?;                       // NotFound(cloud)
    let project = self.clouds.default_project(&cloud)
        .ok_or(SwitchError::NotConfigured { cloud: cloud.clone() })?;
    self.resolve(ContextRequest::ByName {               // delegate
        cloud: Some(cloud),
        project,
        domain: None,
    }).await
}
```
**Dependencies**:
- `CloudDirectory::default_project` (신규 메서드).
- `SwitchError::NotConfigured` (신규 variant).
- 기존 `ByName` 경로 재사용 — disambiguation / not-found / ambiguous 모두 자동 상속.

### App::execute_command (확장)
**Responsibility**: 파싱된 `Command`를 app-level Action으로 변환.
**Interface** (src/app.rs:1771-1780 교체):
```rust
Command::SwitchCloud(name) => {
    self.dispatch_action(Action::SwitchContext(
        ContextRequest::CloudOnly { cloud: name },
    ));
}
```
**Dependencies**:
- `Action::SwitchContext` (기존).
- `ContextRequest::CloudOnly` (신규 variant).
- stub toast 코드 제거. 기존 테스트 `test_command_bar_switch_cloud_emits_info_toast`는 dispatch 검증으로 대체.

### ContextSwitcher (확장) — FR-4 idempotent
**Responsibility**: 7-step switch orchestrator. FR-4 idempotent 체크를 resolver 직후에 삽입.
**Interface** (`switch` 메서드 내부 변경, signature 불변):
```rust
pub async fn switch(&self, request: ContextRequest)
    -> Result<(Epoch, ContextSnapshot), (Epoch, SwitchError)>
{
    let target = match self.resolver.resolve(request).await { ... };
    // [NEW] idempotent check
    if let Some(current) = self.state.current_snapshot()
        && current.target == target
    {
        return Ok((current.epoch, current));
    }
    self.run_switch_to(target).await
}
```
**Dependencies**:
- `SwitchStateMachine::current_snapshot() -> Option<ContextSnapshot>` — 기존 존재 여부 확인 필요. 없으면 얕은 read-only accessor 신규 추가 (epoch/state_machine 내부 RwLock peek).

## 관측성 (NFR-6 반영)

| 위치 | 이벤트 | 필드 |
|---|---|---|
| `ContextTargetResolver::resolve(CloudOnly)` | `tracing::info_span!("resolve_cloud_only")` | `cloud`, `resolved_project` |
| `ContextSwitcher::switch` idempotent 분기 | `tracing::debug!("switch_noop_same_target")` | `cloud`, `project` |
| `SwitchError::NotConfigured` 발생 | `tracing::warn!("cloud_no_default_project")` | `cloud` |
| `App` toast 발행 | 기존 span 상속 | 추가 필드 없음 |

