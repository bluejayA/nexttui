# Test Instructions — BL-P2-074

## Unit + Integration Tests
Run:
```
cargo test --lib --tests
```
Expected: **1328 passed, 0 failed** (baseline 1314 + 14 신규).

## Clippy Gate
Run:
```
cargo clippy --lib --tests -- -D warnings
```
Expected: `Finished 'dev' profile` with no warnings.

## Format Check
Run:
```
cargo fmt --all --check
```
Expected: no diff.

## BL-P2-074 신규 테스트 (14건)

| 테스트 | 위치 | 검증 |
|---|---|---|
| test_not_configured_displays_human_readable | context/error.rs | NotConfigured Display 문구 (FR-8) |
| test_clone_preserves_not_configured | context/error.rs | Clone arm (FR-8) |
| test_load_clouds_yaml_without_default_project_yields_none | config.rs | serde default backward-compat (FR-2, NFR-2) |
| test_load_clouds_yaml_with_default_project_yields_some | config.rs | 필드 역직렬화 (FR-2) |
| test_cloud_directory_default_project_reflects_config | context/resolver.rs | trait 메서드 (FR-3) |
| test_default_project_returns_configured_value | context/config_cloud_directory.rs | production impl (FR-3) |
| test_default_project_none_when_unset | context/config_cloud_directory.rs | 미설정 케이스 (FR-3) |
| test_context_request_cloud_only_is_constructible | context/types.rs | variant 생성 (FR-2) |
| test_resolve_cloud_only_returns_default_project_target | context/resolver.rs | 성공 경로 (FR-3) |
| test_resolve_cloud_only_unknown_cloud_returns_not_found | context/resolver.rs | 실패 unknown cloud (FR-5) |
| test_resolve_cloud_only_no_default_returns_not_configured | context/resolver.rs | 실패 no default (FR-5, FR-8) |
| test_resolve_cloud_only_stale_default_returns_not_found | context/resolver.rs | 실패 stale (FR-5) |
| test_switcher_noop_on_same_target | context/switcher.rs | idempotent epoch 불변 (FR-4, D1) |
| test_switch_back_after_cloud_only_returns_previous_target | context/switcher.rs | D4 CloudOnly→switch-back |

대체된 테스트: `test_command_bar_switch_cloud_emits_info_toast` → `test_command_bar_switch_cloud_dispatches_context_request_without_toast` (app.rs — FR-1, dispatch 검증 + legacy stub 문구 미발행 assertion).

## Manual Verification
1. 기본 flow — CLI 또는 devstack 환경 필요:
   ```
   cargo run --bin nexttui -- --cloud devstack
   ```
   → `:switch-cloud prod` 입력 시 `CloudConfig::default_project` 설정에 따라 전환 또는 `NotConfigured` 토스트.

2. clouds.yaml에 `default_project`가 없는 cloud로 `:switch-cloud` 호출 → 토스트:
   > cloud '<name>' has no default project — use :switch-project <name>

## Last Verified
- **Commit base**: 551265b (main)
- **Branch**: feat/bl-p2-074-switch-cloud-wire
- **Timestamp**: 2026-04-20T10:15+09:00
- **Status**: ✅ 1328/1328 pass, clippy clean, fmt clean
