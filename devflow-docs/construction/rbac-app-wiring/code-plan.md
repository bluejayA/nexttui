# Code Generation Plan: rbac-app-wiring

> **For agentic workers:** REQUIRED: Use `aidlc:aidlc-code-generation` with the
> "GENERATE" signal to execute this plan. Do NOT implement ad-hoc.
> `"code-generation: GENERATE — proceed with the approved plan for rbac-app-wiring"`

## Files to Modify
- [ ] `src/component.rs` — Component trait에 set_admin default method 추가
- [ ] `src/event.rs` — TokenRefreshed(Vec<TokenRole>) tuple variant + PermissionDenied variant
- [ ] `src/app.rs` — rbac: Arc<RbacGuard> 필드, from_registry 시그니처 변경, broadcast_admin, is_admin 하드코딩 제거, TokenRefreshed/PermissionDenied 이벤트 처리
- [ ] `src/module/flavor/mod.rs` — is_admin 파라미터 제거, 기본값 false
- [ ] `src/module/image/mod.rs` — is_admin 파라미터 제거, set_admin() 구현
- [ ] `src/registry.rs` — register_all_modules()에서 FlavorModule/ImageModule 생성자 변경

## Implementation Steps

- [ ] Step 1: Component trait set_admin default method
  - [ ] RED: test_component_set_admin_default — DummyComponent에 set_admin 호출 시 패닉하지 않음
  - [ ] Verify RED: 실패 확인 (메서드 없음)
  - [ ] GREEN: Component trait에 `fn set_admin(&mut self, _is_admin: bool) {}` 추가
  - [ ] Verify GREEN: 통과 확인 + 전체 회귀

- [ ] Step 2: AppEvent 변경 (TokenRefreshed + PermissionDenied)
  - [ ] RED: test_token_refreshed_carries_roles — TokenRefreshed(vec![role]) 매칭 확인
  - [ ] RED: test_permission_denied_event — PermissionDenied { operation } 매칭 확인
  - [ ] Verify RED: 실패 확인
  - [ ] GREEN: TokenRefreshed를 tuple variant로 변경, PermissionDenied variant 추가, 기존 매칭 수정
  - [ ] Verify GREEN: 통과 확인 + 전체 회귀

- [ ] Step 3: FlavorModule/ImageModule is_admin 파라미터 제거
  - [ ] RED: 기존 FlavorModule 테스트에서 new(tx) 호출 (파라미터 1개) — 컴파일 실패
  - [ ] Verify RED: 실패 확인
  - [ ] GREEN: FlavorModule::new(tx) — is_admin 기본값 false, ImageModule::new(tx) — 동일 + set_admin() 구현
  - [ ] Verify GREEN: 통과 확인 + 전체 회귀
  - [ ] REFACTOR: register_all_modules()에서 `FlavorModule::new(tx, true)` → `FlavorModule::new(tx)`, `ImageModule::new(tx, true)` → `ImageModule::new(tx)` 변경

- [ ] Step 4: App에 Arc<RbacGuard> 통합
  - [ ] RED: test_app_rbac_is_admin — App에 rbac 필드 접근 + is_admin() 호출
  - [ ] RED: test_app_broadcast_admin — broadcast_admin 호출 후 모듈 상태 변경 확인
  - [ ] Verify RED: 실패 확인
  - [ ] GREEN: App 구조체에 rbac 필드, from_registry에 rbac 파라미터, broadcast_admin() 구현, App::new()에 기본 RbacGuard
  - [ ] Verify GREEN: 통과 확인 + 전체 회귀

- [ ] Step 5: Sidebar/키 핸들링 is_admin 하드코딩 제거
  - [ ] RED: test_app_sidebar_uses_rbac — non-admin rbac로 App 생성, admin_only route 접근 불가 확인
  - [ ] Verify RED: 실패 확인 (현재 하드코딩 true)
  - [ ] GREEN: sidebar.route_at(idx, true) → sidebar.route_at(idx, self.rbac.is_admin()), 동일하게 handle_key/sync_active/render 변경
  - [ ] Verify GREEN: 통과 확인 + 전체 회귀

- [ ] Step 6: TokenRefreshed/PermissionDenied 이벤트 처리
  - [ ] RED: test_handle_token_refreshed_updates_rbac — TokenRefreshed(roles) 이벤트 → rbac.is_admin() 변경 확인
  - [ ] RED: test_handle_permission_denied_adds_toast — PermissionDenied 이벤트 → Toast 생성 확인
  - [ ] Verify RED: 실패 확인
  - [ ] GREEN: handle_event에서 TokenRefreshed(roles) → rbac.update_roles + broadcast_admin, PermissionDenied → toast
  - [ ] Verify GREEN: 통과 확인 + 전체 회귀

## Test Strategy
- [ ] test_component_set_admin_default: trait default method no-op 확인
- [ ] test_token_refreshed_carries_roles: TokenRefreshed에 roles 데이터 포함
- [ ] test_permission_denied_event: PermissionDenied variant 존재 확인
- [ ] test_app_rbac_is_admin: App에서 rbac.is_admin() 접근
- [ ] test_app_broadcast_admin: 전 모듈 set_admin broadcast
- [ ] test_app_sidebar_uses_rbac: non-admin일 때 admin_only route 필터링
- [ ] test_handle_token_refreshed_updates_rbac: 이벤트로 역할 업데이트
- [ ] test_handle_permission_denied_adds_toast: 권한 거부 Toast
