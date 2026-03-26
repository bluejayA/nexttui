# Code Generation Plan: registry-core

> **For agentic workers:** REQUIRED: Use `aidlc:aidlc-code-generation` with the
> "GENERATE" signal to execute this plan. Do NOT implement ad-hoc.

## Files to Create
- [ ] `src/registry.rs` — ModuleEntry, RegistryParts, ModuleRegistry

## Files to Modify
- [ ] `src/lib.rs` — `pub mod registry;` 추가

## Implementation Steps

- [ ] Step 1: ModuleEntry 구조체 + RegistryParts 구조체
  - [ ] RED: `test_module_entry_sidebar_item` — ModuleEntry에서 sidebar 필드 접근 확인
  - [ ] Verify RED
  - [ ] GREEN: ModuleEntry struct 정의 (sidebar: SidebarItem, component, initial_action, related_routes, display_name)
  - [ ] Verify GREEN + 회귀

- [ ] Step 2: ModuleRegistry::new + register
  - [ ] RED: `test_registry_register_and_count` — register 후 entries 개수 확인
  - [ ] Verify RED
  - [ ] GREEN: ModuleRegistry struct + new() + register()
  - [ ] Verify GREEN + 회귀

- [ ] Step 3: ModuleRegistry::into_parts
  - [ ] RED: `test_registry_into_parts_components` — into_parts 후 components HashMap에 등록한 route 존재
  - [ ] Verify RED
  - [ ] GREEN: into_parts() → RegistryParts 빌드 (components, sidebar_items, initial_actions, route_labels)
  - [ ] Verify GREEN + 회귀
  - [ ] RED: `test_registry_into_parts_route_labels` — primary + related routes 모두 같은 display_name 매핑
  - [ ] Verify RED
  - [ ] GREEN: route_labels에 related_routes 포함
  - [ ] Verify GREEN + 회귀
  - [ ] RED: `test_registry_into_parts_initial_actions` — initial_action이 Some인 모듈만 actions에 포함
  - [ ] Verify RED
  - [ ] GREEN: initial_actions 필터링
  - [ ] Verify GREEN + 회귀

## Test Strategy
- [ ] `test_module_entry_sidebar_item`: ModuleEntry 생성 + sidebar 필드 접근
- [ ] `test_registry_register_and_count`: register 후 개수
- [ ] `test_registry_into_parts_components`: components HashMap 키 확인
- [ ] `test_registry_into_parts_route_labels`: primary + related → 같은 label
- [ ] `test_registry_into_parts_initial_actions`: Some만 포함, None 제외
