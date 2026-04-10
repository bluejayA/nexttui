# Requirements Analysis

**Depth**: Standard
**Timestamp**: 2026-04-10T11:15:00+09:00

## User Intent
파라미터가 비대화된 view_model 함수에 ViewContext 패턴을 도입하여 구조적으로 정리한다. 기능 변경 없는 순수 리팩토링.

## Scope 분석

파라미터 수 기준으로 리팩토링 대상을 선별:

| 모듈 | 함수 | 파라미터 | 대상 |
|------|------|---------|------|
| server | server_detail_data_full | 7개 (server, migration, flavor, is_resize, cached_volumes, cached_floating_ips) | ✅ |
| server | server_to_row_full | 3개 (server, show_tenant, show_host) | ⚠️ 포함 |
| floating_ip | fip_to_row_with_servers | 4개 (fip, show_tenant, cached_servers, cached_ports) | ⚠️ 포함 |
| volume | volume_detail_data_with_servers | 2개 | ❌ 불필요 |
| volume | volume_to_row_with_servers | 3개 | ❌ 불필요 |
| 나머지 12개 | 1~2개 | ❌ 불필요 |

## Functional Requirements

### ServerViewContext
- FR-01: ServerViewContext 구조체 도입 — server, migration_progress, flavor, is_resize_pending, cached_volumes, cached_floating_ips 필드
- FR-02: server_detail_data_full(7 params) → server_detail_data(ctx: &ServerViewContext) 변경
- FR-03: server_to_row_full(3 params)은 유지 — 파라미터가 적고 ViewContext에 넣기 부자연스러움
- FR-04: server_detail_data(server) 편의 함수 유지 (빈 ViewContext로 위임)
- FR-05: ServerModule의 render에서 ViewContext 생성 → 전달

### FipViewContext
- FR-06: FipRowContext 구조체 도입 — show_tenant, cached_servers, cached_ports 필드
- FR-07: fip_to_row_with_servers(4 params) → fip_to_row(fip, ctx: &FipRowContext) 변경
- FR-08: FloatingIpModule의 rows()에서 FipRowContext 생성 → 전달

### 정리 — _full/_with_servers 패턴 통합
- FR-09: server_detail_data / server_detail_data_full 2개 함수 → server_detail_data(ctx) 1개로 통합
- FR-10: volume_detail_data / volume_detail_data_with_servers → 그대로 유지 (파라미터 2개로 충분)
- FR-11: fip_to_row / fip_to_row_with_servers 2개 → fip_to_row(fip, ctx) 1개로 통합

## Non-Functional Requirements
- NFR-01: 기존 1108 tests 전체 통과 (리팩토링이므로 기능 회귀 0건 필수)
- NFR-02: 외부 API 동작 변경 없음 — 렌더링 결과 동일
- NFR-03: 테스트에서 ViewContext builder 또는 Default 사용 가능하도록 설계

## Assumptions
- 파라미터 3개 이하인 함수는 리팩토링 대상에서 제외
- server_to_row_full은 show_tenant/show_host가 UI 설정이라 ViewContext보다는 별도 파라미터가 자연스러움
- ViewContext는 모듈별 로컬 타입 (공유 trait 불필요)

## Open Questions
없음
