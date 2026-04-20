# Cargo Review Report — BL-P2-074 (SwitchCloud wire 완결)

## Summary
- **변경 파일**: 9개 Rust (수정 9 / 생성 0 / 삭제 0) + devflow-docs
- **Diff 규모**: 698 lines (+429 / -44)
- **테스트**: ✅ PASS (1328 통과, 0 실패 — baseline 1314 + 14 신규)
- **Clippy**: ✅ PASS (`cargo clippy --lib --tests -- -D warnings` clean)
- **Fmt**: ✅ PASS (`cargo fmt --all --check` clean)
- **Bin Build**: ✅ PASS (`cargo build --bin nexttui`)
- **테스트 커버리지**: 7/7 변경 컴포넌트 (동일 파일 내 `#[cfg(test)]`)
- **리뷰 모드**: Multi-Agent (3 reviewers, diff > 100줄)
- **포커스**: Full

---

## 1. Correctness (정확성)

| # | 파일:라인 | 심각도 | 이슈 | 제안 |
|---|----------|--------|------|------|
| C1 | `src/context/resolver.rs:120` | LOW | `CloudOnly` → `resolve_by_name_inner` 경로에서 `normalize_cloud_project`의 `split_once('/')` 파싱이 발동. `default_project` 값에 `/`가 포함되면 prefix_cloud로 파싱되어 project 이름이 슬래시 뒤 토큰만 남음. cloud는 `cloud_arg`가 보호하므로 오염 없음, Keystone 조회는 NotFound로 실패 → 데이터 손상 없음. | CloudOnly 경로는 `normalize_cloud_project`를 우회하거나 project 이름에 `/` 포함 시 경고 tracing. 회귀 테스트로 `test_cloud_only_with_slash_in_default_project` 추가 검토. |
| C2 | `src/context/error.rs:59-60` | LOW | `SwitchError::Clone`의 `Api/Io` arm이 `CommitFailed`로 collapse — pre-begin 단계 에러도 "commit failed" 라벨로 오인 가능. 내부 진단용 주석 존재. | 필요 시 `Api` 전용 placeholder variant 신설. 현재는 주석으로 충분. |
| C3 | `src/context/types.rs:19` | LOW | `ContextRequest`에 `#[non_exhaustive]` 부재 — 외부 크레이트 재수출 여부 확인 필요. 현재 크레이트는 binary only. | binary이므로 무해. 향후 lib 분리 시 `#[non_exhaustive]` 추가. |

**참고 (이슈 아님)**: `src/context/switcher.rs:82-93` idempotent path의 TOCTOU는 코드 주석에 명시 + FR-4 acceptance가 순차 호출 한정으로 정의됨.

### 정확성 확인 사항 (Good)
- `run_switch_to` step 7의 `previous_in_flight()` 호출 시점 올바름.
- `switch_back` peek-not-pop 동작이 실패 경로에서 history 보존.
- `worker.rs`의 `Action::SwitchContext | SwitchBack => None` fall-through 방어 유효.
- `CloudConfig::default_project` `#[serde(default)]`로 backward-compat.
- 신규 프로덕션 코드에 `.unwrap()`/`.expect()` 추가 없음.
- `SwitchError::NotConfigured` Clone arm 안전.
- `Action::SwitchCloud` dead code 제거 후 exhaustiveness 유지.

**HIGH/MED 없음**.

---

## 2. Style (스타일)

| # | 파일:라인 | 심각도 | 이슈 | 제안 |
|---|----------|--------|------|------|
| S1 | `src/context/error.rs:28` | MED | `SwitchError::NotConfigured { cloud }` struct variant가 기존 `NotFound(String)` / `Unsupported(String)` / `RescopeRejected(String)` tuple variant와 형식 불일치. (동일 enum에 `Ambiguous { candidates }` struct 전례 존재). | 단일 필드면 tuple로 통일하거나, struct 유지 시 "reason 확장 여지" doc comment 명시. application-design.md D2 결정 유지 권장. |
| S2 | `src/context/resolver.rs:110-114` | LOW | `tracing::warn!`이 `info_span!` 진입 전 발생 → span 컨텍스트 밖 로그. | warn을 `_enter` 이후로 이동하거나 `warn!(cloud = %cloud, ...)`로 필드 명시하여 span 의존성 제거. |
| S3 | `src/context/switcher.rs:79-88` | LOW | `crate::context::state_machine::SwitchStateView::Idle` full path 사용 — 파일 내 타입은 대체로 use로 끌어옴. | 파일 상단 `use crate::context::state_machine::SwitchStateView;` 추가. |
| S4 | `src/app.rs:1776` | LOW | `crate::context::ContextRequest::CloudOnly { cloud: name }` full path. | 파일 상단 use 추가 또는 `crate::context::ContextRequest` 부분만 import. |
| S5 | `src/context/resolver.rs:62-64` | LOW | trait `CloudDirectory`의 `default_project` doc comment가 상세한 반면 `active_cloud`/`known_clouds`는 doc 없음. Coverage 불일치. | 다른 두 메서드에도 한줄 doc 추가하거나 trait-level doc에서 묶어 설명. |
| S6 | `src/context/types.rs:29-31` | LOW | `CloudOnly` doc comment가 BL 참조 위주, 의미 설명 간략. | "no explicit project/domain — `CloudDirectory::default_project` provides resolution" 형식 추가. |
| S7 | tracing 이벤트 전반 | LOW | `info_span` vs `debug!` vs `warn!` 혼용 — 의도적 구분 (span vs event)이지만 프로젝트 다른 tracing 패턴과의 일관성 확인 필요. | 현재 level 분리는 의도적. 이벤트 네이밍 컨벤션(snake_case 유지) 점검으로 충분. |

---

## 3. Suggestions (제안)

| # | 파일:라인 | 유형 | 제안 |
|---|----------|------|------|
| P1 | `src/context/resolver.rs:169-175` (validate_cloud) | PERF | `known_clouds()`가 매 호출마다 `Vec<String>` 재할당. `CloudOnly` 경로에서 `validate_cloud` + `normalize_cloud_project` 내부 체크 중복 → `default_project` 취득 직후 `resolve_by_name_inner`에서 한 번 더 발생 (총 3회). `trait`에 `fn contains_cloud(&self, name: &str) -> bool` 추가하거나 중복 체크 제거로 할당 감소. |
| R1 | `src/context/switcher.rs:82-93` | READABILITY | let-chain 패턴이 길어 가독성 저하. `SwitchStateMachine::committed_target() -> Option<ContextSnapshot>` 헬퍼 추가하면 `if self.state.committed_target().is_some_and(\|s\| s.target == target)` 형태로 평탄화. 테스트 fixture 재사용에도 도움. |
| R2 | `src/context/resolver.rs:236-260` (tests) | READABILITY | `clouds` 헬퍼와 `clouds_with_defaults` 헬퍼가 필드 구성만 다름. `FakeClouds::new(active, known).with_defaults(&[..])` builder 패턴으로 통합 가능. 테스트 전용, 리스크 0. |
| I1 | `src/context/types.rs:19` (CloudOnly) | IDIOM | variant 이름이 "cloud-scoped Keystone token"과 오독 위험. Keystone 맥락에서 "cloud-scoped"는 다른 의미. `ByCloud` rename 고려. Breaking 아님 (내부 enum). |
| I2 | `src/context/error.rs` Clone impl 전반 | IDIOM | `Api(_)`/`Io(_)`를 `Arc<ApiError>`/`Arc<io::Error>`로 감싸면 `#[derive(Clone)]`으로 수동 impl 제거 가능. BL 범위 외, 별도 cleanup BL. |

### 변경 권장하지 않는 검토 포인트
- `CloudDirectory::default_project` 반환 `Option<String>` — `Option<&str>`로 변경 시 FakeClouds `HashMap` lifetime 얽힘으로 결국 `.to_string()` 필요. 현 시그니처 유지.
- `resolve_by_name_inner` 헬퍼 추출 — async 재귀 회피 목적으로 적절.
- `tracing::warn!`의 `ok_or_else` 내부 side-effect — 관용적이며 주석 가능.

---

## 4. Verdict

### ✅ APPROVE

**기준**:
- HIGH 0개 ✓
- 테스트 1328/1328 통과 ✓
- Clippy clean ✓
- Fmt clean ✓
- 설계 결정 D1~D4 구현 정확 ✓
- R1 리뷰 반영 완료 (Action::SwitchCloud dead code 제거) ✓

**선택적 후속 작업**:
- **스타일 S1** (struct vs tuple variant): application-design.md D2 결정 재확인 — struct variant 유지 결정. 현 상태 OK.
- **스타일 S2** (tracing warn span 외부): 수정 간단, 1줄 이동. 선택.
- **제안 P1** (Vec 재할당 중복): 별도 perf BL 권장.
- **제안 R1** (let-chain 평탄화): `SwitchStateMachine::committed_target()` helper 추가. 선택.
- **제안 I1** (CloudOnly → ByCloud rename): breaking하지 않으나 네이밍 논의 필요. 선택.

**권고**: 현 상태로 PR 생성 가능. S2와 R1은 커밋 정리 전 반영하면 품질 상향 (모두 1~5줄 변경).

---

## Review Mode Details

- **Agent A** (Correctness): LOW 3건, HIGH/MED 0건
- **Agent B** (Style): MED 1건 (S1), LOW 8건
- **Agent C** (Suggestions): 8개 제안, 3개 권장 (P1/R1/R2)
- **병합 규칙**: 중복 제거 + 심각도 상향 채택. Agent B #1 (NotConfigured struct variant)은 기존 R1 리뷰에서도 Suggestion으로 지적됨 — application-design D2에서 struct 선택 근거 명시됨.
