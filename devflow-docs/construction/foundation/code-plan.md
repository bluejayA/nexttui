# Code Generation Plan: foundation

> **For agentic workers:** REQUIRED: Use `aidlc:aidlc-code-generation` with the
> "GENERATE" signal to execute this plan. Do NOT implement ad-hoc.
> `"code-generation: GENERATE — proceed with the approved plan for foundation"`

## Files to Create
- [ ] `src/error.rs` — AppError enum (thiserror), Result type alias
- [ ] `src/config.rs` — Config, CloudConfig, AuthConfig, AppConfig, CacheTtlConfig
- [ ] `src/models/mod.rs` — models 모듈 선언
- [ ] `src/models/nova.rs` — Server, Flavor, FlavorRef, Address, Aggregate, ComputeService, Hypervisor
- [ ] `src/models/neutron.rs` — Network, SecurityGroup, SecurityGroupRule, FloatingIp, NetworkAgent
- [ ] `src/models/cinder.rs` — Volume, VolumeAttachment, VolumeSnapshot
- [ ] `src/models/glance.rs` — Image
- [ ] `src/models/keystone.rs` — Project, User, Role, RoleAssignment, UserRef, Scope, ProjectRef
- [ ] `src/models/common.rs` — ResourceType, Route enums
- [ ] `src/lib.rs` — 크레이트 루트 (모듈 선언)
- [ ] `tests/config_test.rs` — Config 통합 테스트 (실제 파일 I/O)

## Files to Modify
- [ ] `Cargo.toml` — 의존성 추가 (serde, serde_yaml, serde_json, toml, thiserror, dirs, clap)
- [ ] `src/main.rs` — lib 크레이트 사용하도록 변경

## Implementation Steps

### Step 1: 프로젝트 구조 + 에러 타입
- [ ] RED: `test_app_error_display` — AppError 각 variant의 Display 메시지 검증
- [ ] Verify RED: 컴파일 실패 확인 (error 모듈 미존재)
- [ ] GREEN: Cargo.toml 의존성 추가 + `src/error.rs` + `src/lib.rs` 작성
- [ ] Verify GREEN: 테스트 통과 확인

### Step 2: Config 기본 구조체 + clouds.yaml 파싱
- [ ] RED: `test_load_clouds_yaml_from_standard_path` — 임시 파일로 정상 파싱 검증
- [ ] RED: `test_clouds_yaml_not_found` — 4개 경로 모두 없으면 에러
- [ ] RED: `test_missing_clouds_key` — clouds 키 없는 YAML → 에러
- [ ] Verify RED: 실패 확인
- [ ] GREEN: `src/config.rs` — Config::load(), CloudConfig, AuthConfig, clouds.yaml 파싱 구현
- [ ] Verify GREEN: 3개 테스트 통과 + 전체 회귀

### Step 3: AuthType 자동 감지 + 검증
- [ ] RED: `test_auth_type_auto_detect_password` — password 필드 → Password 타입
- [ ] RED: `test_auth_type_auto_detect_app_credential` — app_credential_id → AppCredential 타입
- [ ] RED: `test_password_auth_missing_username` — username 누락 시 cloud 스킵
- [ ] RED: `test_app_credential_missing_secret` — secret 누락 시 cloud 스킵
- [ ] RED: `test_all_clouds_invalid_fatal` — 유효한 cloud 0개 → 에러
- [ ] RED: `test_partial_invalid_clouds_skip` — 일부만 무효 → 스킵하고 나머지 정상
- [ ] Verify RED: 실패 확인
- [ ] GREEN: Config::load() 내 검증 로직 추가 (auth_type 감지, 필수 필드 체크, 부분 실패 허용)
- [ ] Verify GREEN: 6개 테스트 통과 + 전체 회귀

### Step 4: active_cloud 결정 + switch_cloud
- [ ] RED: `test_active_cloud_fallback_to_first` — 지정 없으면 첫 엔트리
- [ ] RED: `test_active_cloud_not_found` — 없는 이름 에러
- [ ] RED: `test_switch_cloud_success` — 정상 전환
- [ ] RED: `test_switch_cloud_not_found` — 없는 이름 에러
- [ ] Verify RED: 실패 확인
- [ ] GREEN: active_cloud 결정 로직 + switch_cloud() 구현
- [ ] Verify GREEN: 4개 테스트 통과 + 전체 회귀

### Step 5: 시크릿 마스킹 (TR-06)
- [ ] RED: `test_password_debug_masked` — Debug 출력에 `****`
- [ ] RED: `test_secret_not_serialized` — serde_json 직렬화에서 시크릿 필드 제외
- [ ] Verify RED: 실패 확인
- [ ] GREEN: AuthConfig에 커스텀 Debug impl + #[serde(skip_serializing)] 적용
- [ ] Verify GREEN: 2개 테스트 통과 + 전체 회귀

### Step 6: AppConfig + CacheTtlConfig 기본값
- [ ] RED: `test_app_config_missing_uses_defaults` — config.toml 없어도 기본값
- [ ] RED: `test_app_config_partial_override` — 일부 값만 오버라이드
- [ ] RED: `test_cache_ttl_mapping` — ResourceType → Duration 매핑 검증
- [ ] Verify RED: 실패 확인
- [ ] GREEN: AppConfig::default(), CacheTtlConfig::default(), Config::cache_ttl() 구현
- [ ] Verify GREEN: 3개 테스트 통과 + 전체 회귀

### Step 7: Domain Models (Nova + Neutron)
- [ ] RED: `test_server_deserialize` — Nova servers API JSON → Server struct
- [ ] RED: `test_flavor_deserialize` — Flavor JSON (is_public rename)
- [ ] RED: `test_network_deserialize` — Neutron JSON (provider:* rename 필드)
- [ ] RED: `test_security_group_deserialize` — rules 포함 JSON
- [ ] RED: `test_floating_ip_deserialize` — FloatingIp JSON
- [ ] Verify RED: 실패 확인
- [ ] GREEN: `src/models/nova.rs`, `src/models/neutron.rs` 구현
- [ ] Verify GREEN: 5개 테스트 통과 + 전체 회귀

### Step 8: Domain Models (Cinder + Glance + Keystone)
- [ ] RED: `test_volume_deserialize` — Volume JSON (attachments, bootable string)
- [ ] RED: `test_snapshot_deserialize` — VolumeSnapshot JSON
- [ ] RED: `test_image_deserialize` — Image JSON
- [ ] RED: `test_project_deserialize` — Project JSON
- [ ] RED: `test_user_deserialize` — User JSON
- [ ] Verify RED: 실패 확인
- [ ] GREEN: `src/models/cinder.rs`, `src/models/glance.rs`, `src/models/keystone.rs` 구현
- [ ] Verify GREEN: 5개 테스트 통과 + 전체 회귀

### Step 9: Domain Models (Admin + Common Enums)
- [ ] RED: `test_aggregate_deserialize` — Aggregate JSON
- [ ] RED: `test_compute_service_deserialize` — ComputeService JSON
- [ ] RED: `test_hypervisor_deserialize` — Hypervisor JSON
- [ ] RED: `test_network_agent_deserialize` — NetworkAgent JSON
- [ ] RED: `test_resource_type_variants` — ResourceType enum exhaustiveness
- [ ] RED: `test_route_variants` — Route enum exhaustiveness
- [ ] Verify RED: 실패 확인
- [ ] GREEN: `src/models/common.rs` + 나머지 admin 모델 보완
- [ ] Verify GREEN: 6개 테스트 통과 + 전체 회귀

### Step 10: main.rs 연결 + 최종 정리
- [ ] `src/main.rs` — lib 크레이트 import, Config::load() 호출 스켈레톤
- [ ] REFACTOR: 전체 코드 `cargo clippy` + `cargo fmt` 정리
- [ ] Verify: `cargo build` + `cargo test` 전체 통과

## Test Strategy

### Config Tests (tests/config_test.rs — 통합)
- [ ] `test_load_clouds_yaml_from_standard_path`: tempdir에 clouds.yaml 생성 → 정상 로드
- [ ] `test_clouds_yaml_not_found`: 경로 없음 → CloudsYamlNotFound 에러
- [ ] `test_missing_clouds_key`: 잘못된 YAML → ConfigParse 에러
- [ ] `test_auth_type_auto_detect_password`: password 필드 기반 감지
- [ ] `test_auth_type_auto_detect_app_credential`: app_credential 필드 기반 감지
- [ ] `test_password_auth_missing_username`: 검증 실패 → 해당 cloud 스킵
- [ ] `test_app_credential_missing_secret`: 검증 실패 → 해당 cloud 스킵
- [ ] `test_all_clouds_invalid_fatal`: 유효 cloud 0개 → 에러
- [ ] `test_partial_invalid_clouds_skip`: 부분 무효 → 나머지 정상
- [ ] `test_active_cloud_fallback_to_first`: 기본 선택
- [ ] `test_active_cloud_not_found`: 없는 이름 에러
- [ ] `test_switch_cloud_success`: 런타임 전환
- [ ] `test_switch_cloud_not_found`: 전환 실패

### Secret Masking Tests (src/config.rs — 단위)
- [ ] `test_password_debug_masked`: Debug trait 출력 검증
- [ ] `test_secret_not_serialized`: serde 직렬화 제외 검증

### AppConfig Tests (src/config.rs — 단위)
- [ ] `test_app_config_missing_uses_defaults`: 기본값 폴백
- [ ] `test_app_config_partial_override`: 부분 오버라이드
- [ ] `test_cache_ttl_mapping`: ResourceType → Duration

### Domain Model Tests (src/models/*.rs — 단위)
- [ ] `test_server_deserialize`: Nova Server JSON
- [ ] `test_flavor_deserialize`: Flavor JSON (serde rename)
- [ ] `test_network_deserialize`: Neutron Network JSON (provider rename)
- [ ] `test_security_group_deserialize`: SG + rules
- [ ] `test_floating_ip_deserialize`: FloatingIp JSON
- [ ] `test_volume_deserialize`: Volume JSON (bootable string)
- [ ] `test_snapshot_deserialize`: VolumeSnapshot JSON
- [ ] `test_image_deserialize`: Image JSON
- [ ] `test_project_deserialize`: Project JSON
- [ ] `test_user_deserialize`: User JSON
- [ ] `test_aggregate_deserialize`: Aggregate JSON
- [ ] `test_compute_service_deserialize`: ComputeService JSON
- [ ] `test_hypervisor_deserialize`: Hypervisor JSON
- [ ] `test_network_agent_deserialize`: NetworkAgent JSON
- [ ] `test_resource_type_variants`: enum 검증
- [ ] `test_route_variants`: enum 검증

### Error Type Tests (src/error.rs — 단위)
- [ ] `test_app_error_display`: 각 variant Display 메시지
