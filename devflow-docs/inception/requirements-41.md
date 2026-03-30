# Requirements Analysis — #41 all_tenants

## Issue
BL-P2-032: 전체 프로젝트 리소스 조회 (all_tenants)

## Scope
Admin 사용자가 모든 프로젝트의 리소스를 한눈에 조회할 수 있도록 한다.

## Functional Requirements

### FR-1: all_tenants 토글 액션
- `Action::ToggleAllTenants` 추가
- 키 바인딩: `Ctrl+A` (전체 프로젝트 토글)
- RBAC `EffectiveRole::Admin`일 때만 활성화

### FR-2: 모델에 tenant_id 추가
- Network, SecurityGroup, FloatingIp, Volume, VolumeSnapshot, Image에 `tenant_id: Option<String>` (또는 API 필드명에 맞게 `project_id`, `owner`)
- Server는 이미 있음 ✓

### FR-3: Neutron/Glance/Cinder 필터 확장
- `NetworkListFilter`, `SecurityGroupListFilter`, `FloatingIpListFilter`, `SnapshotListFilter` 신규 생성 (all_tenants: bool)
- `ImageListFilter`에 all_tenants 추가
- Port trait 시그니처 업데이트: list_networks, list_security_groups, list_floating_ips, list_snapshots에 filter 파라미터 추가
- Adapter 쿼리 빌더에 `all_tenants=1` 추가

### FR-4: Worker 필터 전달
- App에 글로벌 `all_tenants: bool` 상태 저장
- Worker가 각 fetch 액션 시 현재 all_tenants 상태를 필터에 반영
- 토글 변경 시 전체 리소스 자동 리프레시

### FR-5: UI 프로젝트 컬럼 동적 표시
- all_tenants=true일 때 각 모듈의 컬럼에 "Project" 컬럼 추가
- view_model의 columns/to_row 함수에 show_tenant 파라미터
- 상태바에 `[ALL]` 인디케이터 표시

## Non-Functional Requirements
- 기존 NFR 그대로 적용 (성능, 보안)
- all_tenants 쿼리는 대량 데이터 반환 가능 → 기존 pagination 활용

## Assumptions
1. OpenStack Neutron도 `?all_tenants=1` 쿼리 지원 (표준 API)
2. Glance는 admin 토큰으로 조회 시 기본적으로 전체 이미지 반환, `owner` 필터로 제한 가능
3. 기존 pagination combinator가 all_tenants 응답에도 그대로 동작
4. 프로젝트 이름 해석(tenant_id → name)은 후속 과제로 분리 (이번엔 ID만 표시)

## Open Questions
없음 — 기존 패턴(Nova/Cinder)이 명확하므로 동일 패턴 확장
