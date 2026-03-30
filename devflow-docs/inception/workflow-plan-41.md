# Workflow Plan — #41 all_tenants

## Complexity
Standard

## Approach Options

### A안: 직행 구현 (application-design skip)
- 기존 Nova/Cinder all_tenants 패턴이 명확하여 별도 설계 없이 바로 구현
- code-generation → build-and-test
- 예상 단위: 3 units

### B안: 설계 포함 (application-design + code-generation + build-and-test)
- application-design에서 컬럼 동적 표시 방식, 글로벌 상태 전파 등 설계 선행
- 나머지 동일

## Recommended: A안

기존 패턴(ServerListFilter.all_tenants, build_server_query)이 검증 완료. Neutron/Glance/Cinder에 동일하게 확장하는 반복 작업이라 설계 단계 불필요.

## Units 분해

### Unit 1: Model + Filter + Port + Adapter (백엔드 레이어)
**범위**:
- 6개 모델에 `tenant_id`/`project_id` 필드 추가 (Network, SecurityGroup, FloatingIp, Volume, Snapshot, Image)
- Neutron용 `NetworkListFilter`, `SecurityGroupListFilter`, `FloatingIpListFilter` 신규 생성
- Cinder용 `SnapshotListFilter` 신규 생성
- `ImageListFilter`에 `all_tenants: bool` 추가
- Port trait 시그니처 업데이트 (filter 파라미터 추가)
- Adapter 쿼리 빌더에 `all_tenants=1` 추가
- 기존 테스트 + 신규 필터 테스트

### Unit 2: Action + Worker + App 상태 (중간 레이어)
**범위**:
- `Action::ToggleAllTenants` 추가
- `ActionKind::ViewAllTenants` 추가 (admin-only)
- App에 `all_tenants: bool` 글로벌 상태
- Worker: fetch 시 all_tenants 상태를 필터에 반영
- 토글 시 RefreshAll 트리거
- RBAC 게이트: Admin만 토글 가능

### Unit 3: UI 컬럼 + 토글 키 + 상태바 (프론트 레이어)
**범위**:
- 각 모듈의 `*_columns()` → `*_columns(show_tenant: bool)` 변경
- 각 모듈의 `*_to_row()` → tenant_id 셀 추가
- `Ctrl+A` 키 바인딩 → ToggleAllTenants 액션
- 상태바에 `[ALL]` 인디케이터
- 대상 모듈: server, volume, network, security_group, floating_ip, snapshot, image (7개)

## Stages
1. INCEPTION: workspace → complexity → requirements → workflow-planning ✓
2. CONSTRUCTION: Unit 1 → Unit 2 → Unit 3 (순차, 의존성 있음)
3. FINISHING: build-and-test → code-review → PR
