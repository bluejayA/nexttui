# Workflow Plan — UI/UX Redesign Stage 2.5-A

**Timestamp**: 2026-03-31T15:30:00+09:00
**Selected Approach**: A안 설계 포함 (application-design Standard + units/code/build)

## Approaches Considered

### A안) 설계 포함 (권장)
- **포함 스테이지**: application-design (LIST + DETAIL), units-generation, code-generation, build-and-test
- **깊이**: application-design=Standard, units-generation=Standard, code-generation=Standard
- **적합**: 12개 파일 109개 Color:: 교체 + Toast 위치 변경 + 포커스 전파 등 기존 로직 정합성 확인 필요. 화면 구성 아이디어 도출 포함
- **주의**: INCEPTION 시간 추가 (application-design DETAIL 단계)

### B안) 설계 스킵, 바로 구현
- **포함 스테이지**: units-generation, code-generation, build-and-test (application-design 스킵)
- **깊이**: units-generation=Standard, code-generation=Standard
- **적합**: 요구사항이 명확하고 기존 코드 구조를 잘 알고 있을 때
- **주의**: Theme 시스템 도입 시 컴포넌트 간 의존성을 놓칠 수 있음. 화면 구성 개선 아이디어 없이 기존 레이아웃에 스타일만 적용

## Workflow Visualization (A안 기준)

```
INCEPTION
  ✅ workspace-detection (완료 — Brownfield, Rust/ratatui 0.30)
  ✅ requirements-analysis (완료 — FR-UI-1~5, NFR-UI-1~4, UPDATE)
  ✅ user-stories (완료 — US-049~057 추가)
  ✅ nfr-requirements (완료 — NFR-6-1~8 추가)
  ✅ workflow-planning (현재)
  ➡ application-design [Standard] — LIST: 변경 컴포넌트 도출, DETAIL: 화면 구성 + Theme 설계

CONSTRUCTION
  ➡ units-generation [Standard] — 독립 구현 단위 분해
  ➡ code-generation [Standard] — TDD 기반 구현
  ➡ build-and-test [Standard] — 전체 빌드 + 694 테스트 회귀 검증
```

## Approved Stages

### PRE-PLANNING
- user-stories: included — US-049~057 (Theme, 포커스, 상태바, 하이라이트, 최소크기, 진행상태, 에러알림, 호스트뷰, 멀티셀렉트)
- nfr-requirements: included — NFR-6-1~8 (시각일관성, 렌더링, 최소크기, 테스트회귀, 적응형폴링, 벌크동시성, 알림정책, 알림위치)

### CONSTRUCTION
- application-design: included — 12파일 109개 Color 교체 + Toast 위치 변경 + 포커스 전파 설계 필요. 화면 구성 아이디어 도출
- units-generation: included — 5개 백로그(BL-P2-034~038) 기반 독립 단위 분해
- code-generation: included — always (TDD protocol)
- build-and-test: included — always

## Stage Depths
- application-design: Standard (LIST + DETAIL, 화면 구성 포함)
- units-generation: Standard
- code-generation: Standard (TDD protocol 적용)
- build-and-test: Standard

## Scope Note

이번 워크플로우는 **Stage 2.5-A (Theme & Polish)** 범위만 대상:
- BL-P2-034: Theme 시스템 도입
- BL-P2-035: Rounded 보더 + 포커스 피드백
- BL-P2-036: 패널 타이틀 포맷
- BL-P2-037: 상태바 리디자인
- BL-P2-038: 리스트 하이라이트 개선

US-054~057 (진행상태 폴링, 에러알림, 호스트뷰, 멀티셀렉트)는 요구사항으로 기록되었으나,
구현은 Stage 2.5-B/C 또는 별도 백로그에서 진행. 이번 Theme/보더/상태바 변경이 이후 구현의 기반이 됨.
