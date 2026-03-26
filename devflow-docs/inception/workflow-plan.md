# Workflow Plan

**Timestamp**: 2026-03-26T17:45:00+09:00
**Selected Approach**: A안 rbac.rs 집중 변경

## Approaches Considered
- A안) rbac.rs 집중 변경 — application-design + code-generation + build-and-test
- B안) 풀 설계 포함 — application-design + units-generation + code-generation + build-and-test

## Approved Stages
### PRE-PLANNING
- user-stories: included — 6 stories (Must 5, Should 1)
- nfr-requirements: included — NFR-2 보안 항목 확장

### CONSTRUCTION
- application-design: included — RbacGuard 역할 enum + can_perform 로직 설계
- units-generation: skipped — 변경이 rbac.rs에 집중, 유닛 분리 불필요
- code-generation: included — always
- build-and-test: included — always

## Stage Depths
- application-design: Standard
- code-generation: Standard (TDD protocol 적용)
- build-and-test: Standard
