# Code Generation Plan: port-layer

> **For agentic workers:** REQUIRED: Use `aidlc:aidlc-code-generation` with the
> "GENERATE" signal to execute this plan. Do NOT implement ad-hoc.

## Files to Create
- [ ] `src/port/mod.rs` — port 모듈 선언
- [ ] `src/port/error.rs` — ApiError enum, ApiResult type
- [ ] `src/port/types.rs` — 공통 지원 타입 (PaginationParams, Token, Capability, 필터, 파라미터)
- [ ] `src/port/auth.rs` — AuthProvider trait
- [ ] `src/port/nova.rs` — NovaPort trait
- [ ] `src/port/neutron.rs` — NeutronPort trait
- [ ] `src/port/cinder.rs` — CinderPort trait
- [ ] `src/port/keystone.rs` — KeystonePort trait
- [ ] `src/port/glance.rs` — GlancePort trait
- [ ] `src/port/mock.rs` — 전체 Mock adapter 구현

## Files to Modify
- [ ] `Cargo.toml` — async-trait, reqwest, chrono 의존성 추가
- [ ] `src/lib.rs` — port 모듈 선언

## Implementation Steps

### Step 1: ApiError + 공통 타입
- [ ] RED: `test_api_error_display` — ApiError 각 variant Display
- [ ] GREEN: `src/port/error.rs` + `src/port/types.rs`

### Step 2: AuthProvider trait
- [ ] GREEN: `src/port/auth.rs` — trait 정의 (컴파일 검증)

### Step 3: NovaPort trait
- [ ] GREEN: `src/port/nova.rs` — trait 정의 + 지원 타입

### Step 4: NeutronPort + CinderPort + KeystonePort + GlancePort
- [ ] GREEN: 4개 trait 정의 + 지원 타입

### Step 5: Mock adapters
- [ ] RED: `test_mock_nova_list_servers` — 빈 Vec 반환
- [ ] RED: `test_mock_neutron_list_networks`
- [ ] RED: `test_mock_cinder_list_volumes`
- [ ] RED: `test_mock_glance_list_images`
- [ ] RED: `test_mock_keystone_list_projects`
- [ ] GREEN: `src/port/mock.rs` — 5개 Mock adapter 구현

### Step 6: cargo clippy + fmt
- [ ] Verify: 전체 테스트 통과
