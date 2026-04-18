# Workflow Plan

**Timestamp**: 2026-04-13T00:00:00+09:00
**Selected Approach**: A안 (안전 완전)
**Updated**:
- 2026-04-16 — T3 Runtime Wire (FR-11, B3 축소 범위) UPDATE
- 2026-04-17 — Unit 4.5 Command Bar Integration (stub blind spot 대응) UPDATE

## Approaches Considered
- A안) 안전 완전 — 설계 + 유닛 분해 전체 (권장) — 단일 BL을 PR1~PR6에 정합
- B안) 간결 — application-design LIST만, units-generation Minimal — 검증 단축, 안전성 리스크

## Approaches

### A안) 안전 완전 (권장)
- **포함 스테이지**: application-design (Standard, LIST + DETAIL), units-generation (Standard), code-generation (Standard, TDD), build-and-test (Standard)
- **깊이**: Standard 전반
- **적합**:
  - Codex 리뷰가 지적한 cross-cutting 변경 (ContextEpoch, CancellationToken, 폴링 루프 전반 수정)을 컴포넌트 수준에서 사전 설계
  - PR1~PR6 단계적 머지 전략과 1:1 정합되는 unit 분해 가능
  - safety-critical NFR (NFR-1) 보장에 필수
- **주의**: DETAIL 단계 + units-generation으로 inception 시간 증가 (예상 추가 30~60분)

### B안) 간결
- **포함 스테이지**: application-design (Minimal, LIST만), units-generation (Minimal), code-generation (Standard, TDD), build-and-test (Standard)
- **깊이**: 설계 Minimal, 구현 Standard
- **적합**: 단일 영역 변경, 위험도 낮은 기능
- **주의**: 본 BL은 cross-cutting (모든 폴링 루프) + 동시성 (epoch/cancel) + 외부 API (Keystone rescope) + UI (피커/인디케이터) 4축 동시 변경. Minimal 설계로는 통합 지점 누락 가능성이 높아 본 BL에는 비권장.

## Workflow Visualization (T3 UPDATE)

```
INCEPTION (UPDATE 모드)
  ✅ workspace-detection (delta update)
  ✅ complexity-declaration (Standard)
  ✅ requirements-analysis (FR-11 + NFR-6/7 추가)
  ✅ pre-planning (B — NFR 검토 갱신)
  ✅ workflow-planning (UPDATE — 현재)
  ➡ application-design UPDATE — T3 컴포넌트 추가 + wire 시퀀스 설계

CONSTRUCTION
  ➡ units-generation UPDATE — T3 unit 분해
  ➡ code-generation [Standard, TDD]
  ➡ build-and-test [Standard]
```

## Approved Stages
### PRE-PLANNING
- user-stories: skipped — Pre-Planning C, 운영자 시나리오 명확
- nfr-requirements: skipped → B (검토 갱신) — NFR-6 (컴파일 안전성), NFR-7 (Demo 모드 무회귀) 추가

### CONSTRUCTION
- application-design: included (UPDATE) — T3 wire에 필요한 3개 신규 컴포넌트 + wire 시퀀스 설계
- units-generation: included (UPDATE) — T3 전용 unit 분해
- code-generation: included — always
- build-and-test: included — always

## Stage Depths
- application-design: Standard (기존 DETAIL 유지 + T3 컴포넌트 추가)
- units-generation: Standard
- code-generation: Standard (TDD protocol 적용 — _shared/tdd-protocol.md)
- build-and-test: Standard

## T3 UPDATE 범위 요약

T3에서 추가/변경되는 컴포넌트:

| 컴포넌트 | 유형 | 설명 |
|---------|------|------|
| ConfigCloudDirectory | 신규 Service | Config 래퍼, CloudDirectory trait 구현 |
| StaticProjectDirectory | 신규 Service | Config 기반 ProjectDirectoryPort 구현 |
| HttpEndpointCache 노출 | 변경 (AdapterRegistry) | 5개 HttpAdapter의 BaseHttpClient를 Arc<dyn HttpEndpointCache>로 노출 |
| main.rs wire | 변경 (main.rs) | ContextSwitcher 조립 + app.wire_context_switch 호출 |
| demo 모드 가드 | 변경 (main.rs) | --demo 시 switcher=None, wire 스킵 |

기존 application-design의 ContextTargetResolver 의존성이 ConfigCloudDirectory + StaticProjectDirectory로 구체화됨.

## PR3 UPDATE 범위 요약 (2026-04-17)

**Trigger**: Unit 5 Step 1 구현 중 발견 — `CommandParser` (`src/input/command.rs`) 및 `InputBar` (`src/ui/input_bar.rs`)가 외부 콜러 0건의 dead code. app.rs:256은 `:` 키를 누르면 `InputMode::Command`로 전환만 하고, 입력 수집·파싱·dispatch 경로 부재.

**영향**: Unit 5의 switch 명령이 파서 단위로만 GREEN이 될 뿐 사용자에게 실제 동작하지 않는 상태로 PR될 위험. `feedback_stub_blind_spot` 패턴 회귀.

**대응**: Unit 5 앞에 Unit 4.5 "Command Bar Integration" 신규 분해 삽입. PR3 = Unit 4.5 + Unit 5.

| 컴포넌트 | 유형 | 설명 |
|---------|------|------|
| App (src/app.rs) | 변경 | `input_bar`, `command_parser` 필드 + `InputMode::Command` 위임 + `command_to_action` 매퍼 |
| InputMode 타입 정합 | 변경 | `component::InputMode` ↔ `ui::input_bar::InputMode` 동기화 진입점 |
| CommandParser (기존 확장 유지) | 변경 | switch-project/cloud/back 파싱 (Unit 5 Step 1에서 선구현) |

CONSTRUCTION 흐름: INCEPTION UPDATE(units + application-design + workflow-plan) → CONSTRUCTION 재개 → Unit 4.5 → Unit 5.
