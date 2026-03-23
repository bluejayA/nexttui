# Functional Design: Unit 3 — port-layer

**Timestamp**: 2026-03-23T16:30:00+09:00
**Unit**: port-layer
**Stories**: TR-01 (Port/Adapter 디커플링), TR-04 (Mock 테스트)
**Components**: AuthProvider, NovaPort, NeutronPort, CinderPort, KeystonePort, GlancePort + Mock adapters

---

## Summary

이 unit은 **trait 정의** + **지원 타입** + **Mock 구현체**로 구성됩니다. 비즈니스 로직은 없으며 (인터페이스만), detail-design-port-adapter.md에 구현 수준의 코드가 이미 있습니다.

## Entities

1. **ApiError** — 서비스 공통 에러 enum (AuthFailed, NotFound, Conflict, RateLimited 등)
2. **AuthProvider trait** — 인증 추상화 (authenticate, refresh, get_token, authenticate_request)
3. **NovaPort trait** — Nova API (servers, flavors, aggregates, compute services, hypervisors, usage)
4. **NeutronPort trait** — Neutron API (networks, security groups, floating IPs, agents)
5. **CinderPort trait** — Cinder API (volumes, snapshots, QoS, storage pools)
6. **KeystonePort trait** — Keystone Admin API (projects, users, roles, quotas)
7. **GlancePort trait** — Glance API (images CRUD)
8. **Supporting types** — 필터, 페이지네이션, 생성/수정 파라미터, Token, Capability 등
9. **Mock adapters** — 각 trait의 테스트용 구현체 (고정 데이터 반환)

## Business Rules

- BR-01: 모든 Port trait은 `async_trait` + `Send + Sync`
- BR-02: ApiError는 reqwest::Error를 From으로 변환 (이 unit에서는 reqwest를 직접 의존하지 않고 에러 타입만 정의)
- BR-03: Mock adapter는 빈 Vec 또는 고정 데이터 반환, 에러 시뮬레이션 가능하도록 설계

## Test Strategy

- Port trait은 trait 정의이므로 직접 테스트 불가
- **Mock adapter가 trait을 올바르게 구현하는지** 컴파일 타임 검증
- Mock adapter의 기본 동작 테스트 (빈 리스트 반환, 에러 반환 등)

## code-generation Connection

- `test_mock_nova_list_servers` — MockNovaAdapter.list_servers() → 빈 Vec
- `test_mock_neutron_list_networks` — MockNeutronAdapter.list_networks() → 빈 Vec
- `test_mock_cinder_list_volumes` — MockCinderAdapter.list_volumes() → 빈 Vec
- `test_mock_glance_list_images` — MockGlanceAdapter.list_images() → 빈 Vec
- `test_mock_keystone_list_projects` — MockKeystoneAdapter.list_projects() → 빈 Vec
- `test_api_error_display` — ApiError 각 variant Display 검증
