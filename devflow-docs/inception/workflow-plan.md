# Workflow Plan

**Timestamp**: 2026-04-13T00:00:00+09:00
**Selected Approach**: A안 (안전 완전)

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

## Workflow Visualization (A안 기준)

```
INCEPTION
  ✅ workspace-detection
  ✅ complexity-declaration
  ✅ requirements-analysis
  ⏭ user-stories — 스킵 (Pre-Planning C)
  ⏭ nfr-requirements — 스킵 (Pre-Planning C, requirements.md에 통합)
  ➡ workflow-planning (현재)
  ➡ application-design [Standard] LIST → DETAIL

CONSTRUCTION
  ➡ units-generation [Standard]
  ➡ code-generation [Standard, TDD]
  ➡ build-and-test [Standard]
```

## Approved Stages
### PRE-PLANNING
- user-stories: skipped — Pre-Planning C, 운영자 시나리오 명확
- nfr-requirements: skipped — Pre-Planning C, NFR이 requirements.md에 5개 명시됨

### CONSTRUCTION
- application-design: included — cross-cutting 동시성 변경 + Adapter 신규 + UI 컴포넌트 다수, 컴포넌트 설계 필수
- units-generation: included — PR1~PR6 단계적 머지에 정합하는 unit 분해 필요
- code-generation: included — always
- build-and-test: included — always

## Stage Depths
- application-design: Standard (LIST + DETAIL, NFR Design은 Comprehensive 미선택으로 비활성)
- units-generation: Standard
- code-generation: Standard (TDD protocol 적용 — _shared/tdd-protocol.md)
- build-and-test: Standard
