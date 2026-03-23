# Workflow Plan

**Timestamp**: 2026-03-23T09:15:00+09:00
**Selected Approach**: A안 — 체계적 점진 구축

## Approaches Considered
- A안) 체계적 점진 구축 (선택) — application-design Comprehensive + units/code/build Standard. 아키텍처 선행 설계로 Phase 2 전환 리스크 최소화
- B안) 설계 경량화 + 빠른 구현 — application-design Standard + units Minimal. 기존 합의 활용, 구현 속도 우선
- C안) 최소 설계 + 즉시 구현 — application-design 스킵. 코드로 아키텍처 탐색. 프로토타입용

## Approved Stages
### PRE-PLANNING
- user-stories: included — 48개 (Must 42 + Should 6), Admin API + 기획 배경 반영 완료
- nfr-requirements: included — 5개 카테고리 (성능/보안/가용성/데이터무결성/배포운영), 도메인: 사내도구+보안상향

### CONSTRUCTION
- application-design: included — Port/Adapter 멀티 백엔드 추상화, Component 시스템, RBAC, Service Layer 전환 대비 등 횡단 관심사 설계 필수
- units-generation: included — 48개 스토리를 독립 구현 가능한 유닛으로 분해
- code-generation: included — always (TDD protocol 적용)
- build-and-test: included — always

## Stage Depths
- application-design: Comprehensive (NFR Design 활성화, 멀티 백엔드 인증 설계, RBAC 구조, Service Layer 전환 대비)
- units-generation: Standard (의존성 순서 정의, 유닛별 스토리 매핑)
- code-generation: Standard (TDD protocol 적용 — RED-GREEN-REFACTOR)
- build-and-test: Standard (단위 테스트 + 통합 빌드 검증)
