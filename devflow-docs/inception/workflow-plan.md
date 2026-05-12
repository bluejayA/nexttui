# Workflow Plan

**Timestamp**: 2026-04-24T11:15:00+09:00
**Work Item**: BL-P2-085 — Cross-project scoping 전면 fix (P0, Critical)
**Selected Approach**: A안 Atomic Security Fix

## Approaches Considered

- **A안) Atomic Security Fix** — application-design Standard + 단일 PR + 5 FR atomic. 권장. P0 보안 경계의 rollback/일관성 단순화.
- **B안) Split Layered Rollout** — application-design Minimal + units 3개 + 3개 PR 순차. 리뷰 부담 분산하지만 **중간 상태에서 보안 경계 부분 적용 창**이 생겨 P0 긴급도에 부적합.

## Approved Stages (A안 기준 초기값 — 게이트 확정 후 업데이트)

### PRE-PLANNING
- user-stories: skipped — Bug-fix BL, INVEST 스토리 가치 낮음 (pre-planning gate=C)
- nfr-requirements: skipped — NFR은 requirements.md 내부로 충분 (pre-planning gate=C)

### CONSTRUCTION
- application-design: included — 5 FR 계층 매트릭스 + ⚠️ Assumption 3건 실측 검증 필요
- units-generation: skipped — A안은 단일 atomic unit (보안 경계 fix 일체)
- code-generation: included — always (TDD 엄수)
- build-and-test: included — always

## Stage Depths

- application-design: **Standard** — LIST + DETAIL + review. NFR Design 비활성 (Comprehensive 아님).
- units-generation: N/A (skipped)
- code-generation: **Standard** — TDD protocol 적용 (RED-GREEN-REFACTOR, `_shared/tdd-protocol.md`)
- build-and-test: **Standard** — cargo test + clippy + audit + `/codex:review --scope branch --base main` 게이트

## Rationale (A안 선택 근거 요약)

1. **P0 보안 경계 atomic rollback/일관성**: 5 FR 상호 의존 (reason 스키마, RBAC→worker, form→RBAC) → 분할 시 중간 창 + 스키마 drift 위험.
2. **상용 운영 조직 / 안정성 우선 철학**과 정합: 부분 적용 상태가 운영 사고 위험 창을 여는 것을 회피.
3. **리뷰 부담 관리 가능**: spec-reviewer (requirements에서 이미 1회) + R1 artifact-review (application-design) + `/codex:review` (CONSTRUCTION 완료 후) — 기존 체인으로 충분. Council은 저장된 `feedback_review_depth.md`에 따라 고위험 변경 시만 (현재 4겹 매핑 명확해서 불요).

## Scope Exclusions (명시)

- `build_disambiguated_opts` 다른 모듈 확장 (Open Q2): **이번 범위 밖**. 필요 시 후속 BL로 분리.
- 감사 로그 뷰어 / 패턴 분석 대시보드: 이벤트 스키마만 이번에 픽스, UI 소비는 후속 BL.
- PII 해싱 옵션: 후속 BL.
- DevStack `devstack-integration` CI placeholder 활성화: BL-P2-081 우선권 보유.

## Open Questions To Resolve Downstream

- Q1 — `tenant_id` 필터 주입 불가 endpoint 여부 → application-design adapter 매트릭스
- Q3 — Worker 거부 에러 variant → application-design 에러 분류
- Q4 — 단일 PR vs 분할 PR → **A안 선택으로 확정: 단일 PR**

## Workflow Visualization

```
INCEPTION
  ✅ workspace-detection (완료, gate=C)
  ✅ complexity-declaration (완료, Standard)
  ✅ requirements-analysis (완료, gate=B, 4 open Qs deferred)
  ✅ pre-planning (완료, gate=C — user-stories/nfr 스킵)
  ⏭ workflow-planning (현재)
  ➡ application-design [Standard] — LIST + DETAIL + review

CONSTRUCTION
  ⏭ units-generation — 스킵 (A안 기준, atomic 단일 변경 범위)
  ➡ code-generation [Standard] — TDD RED-GREEN-REFACTOR
  ➡ build-and-test [Standard] — cargo test + clippy + codex-review 게이트
```
