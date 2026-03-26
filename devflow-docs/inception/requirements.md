# Requirements Analysis

**Depth**: Standard
**Timestamp**: 2026-03-26T17:30:00+09:00

## User Intent

현재 admin/non-admin 이분법 RBAC를 admin/member/reader 3단계로 세분화한다.
Phase 1의 하드코딩 역할 체크를 유지하면서, reader 역할의 읽기 전용 제한과 member 역할의 CUD 허용을 명확히 구분한다.
후속 이슈에서 Capability 기반으로 전환할 때 `can_perform()` 인터페이스는 동일하게 유지되므로 전환 비용이 낮다.

해석 확정: A안 (역할 세분화) → 후속 B안 (Capability 전환)으로 점진 전환.

## Functional Requirements

### FR-1: 역할 3단계 정의
- **Admin**: 모든 액션 허용 (현재와 동일)
- **Member**: CRUD 허용, admin 전용 액션(ForceDelete, Migrate, Evacuate, EnableDisable, ManageQuota) 차단
- **Reader**: 읽기(Read) 전용, 모든 CUD 차단

### FR-2: can_perform 역할 연동
- `can_perform(ActionKind)` 내부에서 현재 역할 기준으로 판단
- Admin → 전부 허용
- Member → Create, Delete, Read 허용; ForceDelete, Migrate, Evacuate, EnableDisable, ManageQuota 차단
- Reader → Read만 허용, 나머지 전부 차단

### FR-3: Sidebar 메뉴 필터링
- Admin 전용 라우트(Migrations, Aggregates, ComputeServices, Hypervisors, Projects, Users, Agents, Usage)는 admin만 표시 (현재와 동일)
- Reader도 일반 라우트(Servers, Networks, Volumes 등)는 볼 수 있어야 함

### FR-4: Worker RBAC 가드 강화
- Worker의 `action_to_kind()` 매핑은 현재와 동일하게 유지
- `can_perform()` 내부 로직만 변경되므로 Worker 코드 변경 불필요

### FR-5: UI 액션 버튼 필터링
- 모듈의 키 핸들링에서 CUD 액션 키('c', 'd' 등)를 역할에 따라 비활성화
- Reader는 'c'(생성), 'd'(삭제) 키가 무시됨

## Non-Functional Requirements

- `can_perform()` 인터페이스 변경 없음 — Worker, Module 호출 코드 최소 변경
- 후속 Capability 전환 시 `is_admin_only_action()` + 역할 match 로직 ~50줄만 폐기
- 기존 테스트 하위 호환 — admin/member 테스트 그대로 통과

## Technology Stack

| 계층 | 선택 | 소스 | 비고 |
|------|------|------|------|
| Language | Rust 2024 | Brownfield | |
| 변경 대상 | src/infra/rbac.rs | 기존 | 핵심 변경 파일 |

## Assumptions

1. Keystone 역할 이름 기준: "admin", "member", "reader" (OpenStack 표준 3역할)
2. 역할이 여러 개면 가장 높은 권한 기준 (admin > member > reader)
3. 알 수 없는 역할은 reader로 처리 (최소 권한 원칙)
4. B단계(Capability 전환)는 별도 이슈로 분리

## Open Questions

(없음 — 해석 A안으로 확정)
