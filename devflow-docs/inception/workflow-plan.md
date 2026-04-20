# Workflow Plan

**Timestamp**: 2026-04-19T09:40:00+09:00
**BL**: BL-P2-074 (SwitchCloud wire 완결)
**Parent BL**: BL-P2-031 (PR#76 머지 완료). 부모 workflow-plan은 `.archive/inception-20260418-223706/workflow-plan.md` 참조.
**Parent Requirements**: `devflow-docs/inception/requirements.md` FR-1 `:switch-cloud <name>` — cloud 전환 (프로젝트는 cloud 기본값 또는 미선택 상태)
**Selected Approach**: A안 (설계 먼저 + 단일 unit TDD) — 2026-04-19 확정

## Context

BL-P2-074는 BL-P2-031 PR#76의 toast-only stub(`src/app.rs:1771-1780`)을 완결한다. requirements.md FR-1의 "cloud 기본값" 경로를 구체화.

**Review artifacts**: `inception/requirements-review-raw/{spec,quality,adversarial}.md` — 3자 리뷰 원문 + 종합 의견.

### BL-P2-074 확정 설계 결정

| 결정 | 선택 | 근거 |
|---|---|---|
| 구현 전략 | 옵션 (a) `ContextRequest::CloudOnly { cloud }` variant | 옵션 (b) picker 경로는 Unit 6 의존 → BL-P2-075로 분리 |
| Default project source | **신규 `CloudConfig::default_project: Option<String>` 필드** | `auth.project_name` (Keystone bootstrap credential) 재사용은 admin scope 실착지 위험 (adversarial H1) |
| 미설정 cloud 전환 요청 | **`SwitchError::NotConfigured { cloud }` 전용 variant 신설** | 문자열 fallback은 BL-P2-053 migration 부담 (adversarial H3 + spec + quality 합의) |
| 동일 cloud 재입력 no-op | state_machine transition 카운터 불변 단위 테스트로 acceptance 측정 | FR-4 testability (3자 합의) |
| CloudDirectory trait | `default_project(&str) -> Option<String>` 메서드 추가 | 4 impl 사이트 전수 업데이트 (ConfigCloudDirectory + 3 FakeClouds) |
| ContextRequest 매처 | resolver/switcher/worker/app 4개 사이트 전수 업데이트 | 컴파일러 강제 (`#[non_exhaustive]` 부재) |
| :switch-back post-CloudOnly | pre-switch `ContextTarget`으로 복귀 | `ContextHistoryStore` 기존 동작 유지 |
| Toast 안전 | `safe_display(60자 truncate + 제어문자 제거)` 의무화 | 터미널 이스케이프/CR-LF injection 방지 |

## Approaches Considered

### A안) 설계 먼저 + 단일 unit TDD (권장)
- **포함**: application-design (Standard) → code-generation (Standard, TDD) → build-and-test (Standard)
- **스킵**: units-generation (단일 unit)
- **깊이**: Standard
- **적합**: `CloudConfig` 스키마 확장 + `CloudDirectory` trait 확장 + FR-4 idempotent 위치 + `SwitchError::NotConfigured` 통합을 단일 문서로 응축 → code-generation 중 흔들림 최소화
- **주의**: +0.5 세션 오버헤드 (설계 문서 작성)

### B안) 바로 구현
- **포함**: code-generation (Minimal, TDD) → build-and-test (Standard)
- **스킵**: application-design, units-generation
- **깊이**: Minimal (code-generation) / Standard (build-and-test)
- **적합**: 설계 결정이 requirements-review-raw에서 이미 확정됐다고 간주, 바로 TDD 진입
- **주의**: idempotent 위치, config 스키마 배치, serde 직렬화 호환성 등 세부 결정을 TDD 루프 안에서 내려야 함 → 리팩토링 증가 가능

## Workflow Visualization (A안 기준)

```
INCEPTION
  ✅ workspace-detection (완료, 델타)
  ✅ requirements-analysis (완료, 3자 리뷰 반영)
  ⏭ workflow-planning (현재)
  ➡ application-design [Standard] (A안)
  ⏭ units-generation — 스킵 (A안: 단일 unit)

CONSTRUCTION
  ➡ code-generation [Standard] (TDD RED-GREEN-REFACTOR)
  ➡ build-and-test [Standard]
```

## Approved Stages (A안 기준 — gate 선택 대기)

### PRE-PLANNING
- user-stories: skipped — Standard complexity + 단일 기능 완결 BL (Pre-Planning Gate C 선택)
- nfr-requirements: skipped — requirements 개정 + 3자 리뷰에서 NFR 응축 완료 (Pre-Planning Gate C 선택)

### CONSTRUCTION
- application-design: included — `CloudConfig` 스키마 확장 + `CloudDirectory` trait 확장 + FR-4 idempotent 위치 + `SwitchError::NotConfigured` 통합
- units-generation: skipped — 단일 unit (variant + handler + resolver + tests)
- code-generation: included — always (TDD protocol 적용)
- build-and-test: included — always

## Stage Depths
- application-design: Standard
- units-generation: N/A (skipped)
- code-generation: Standard (TDD protocol — `_shared/tdd-protocol.md`)
- build-and-test: Standard
