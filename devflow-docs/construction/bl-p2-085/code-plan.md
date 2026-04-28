# Code Generation Plan: BL-P2-085 Cross-project scoping atomic fix

> **For agentic workers:** REQUIRED: Use `aidlc:aidlc-code-generation` with the "GENERATE" signal to execute this plan. Do NOT implement ad-hoc.
> `"code-generation: GENERATE — proceed with the approved plan for BL-P2-085"`

**Complexity**: Standard
**Approach**: A안 Atomic Security Fix (단일 unit, 단일 PR)
**Baseline**: 1370 tests green (c4590ab)
**Branch**: feature/bl-p2-085-cross-project-scoping

## 결정 확정 (RED 진입 전)

| 미결 항목 | 확정 내용 | 근거 |
|----------|----------|------|
| `FormSelectedIdValidator` 위치 | `src/module/common/scope_validator.rs` (신규 `common/` 모듈 생성) | 모듈별 중복 회피, `src/module/` 하위 규약 |
| `ScopeProvider` 1차 소스 | **`RbacGuard::project_id()` (cached copy)** | `ActionSender::send()` sync 유지. AuthProvider는 async → sync API 깨질 위험. RbacGuard는 Token 업데이트 이벤트로 동기화 |
| `update_roles_preserve_project` 인터페이스 | 신규 메서드 추가 (기존 `update_roles` 유지). `update_roles_preserve_project(&self, roles: Vec<TokenRole>)` — project_id 미변경 | preserve API가 callsite에서 의도 명시적. enum 대비 간결 |

## Files to Create

- [ ] `src/infra/cross_project_guard.rs` — `CrossProjectGuard` pure fn + `CrossProjectReason` / `GuardLayer` / `GuardDecision` enum
- [ ] `src/infra/cross_project_audit.rs` — `CrossProjectBlockEvent` 빌더 + v1 canonical fingerprint (RED-time 발견: 기존 `src/infra/audit.rs::AuditLogger` 재사용. 서브디렉토리 폐기, 단일 파일로 축소)
- [ ] `src/ui/cross_project_toast.rs` — 공통 Warning-level 토스트 빌더
- [ ] `src/module/common/mod.rs` — common 서브모듈 entry
- [ ] `src/module/common/scope_validator.rs` — `FormSelectedIdValidator`

### RED-time 발견 (2026-04-27)

#### A. 기존 AuditLogger (production-ready, 적극 사용 중)

`src/infra/audit.rs`에 `AuditLogger` 존재 (file rotation 10MB×5, sensitive masking, 2-phase logging). `src/app.rs:14, 80, 98, 126, 143, 171, 203, 761-770, 782-788`에서 적극 사용. 본 BL의 `CrossProjectBlockEvent`는 신규 audit 인프라가 아니라 **AuditLogger 위에서 동작하는 specialized AuditEntry 빌더**로 재정의:
- `CrossProjectBlockEvent::to_audit_entry() -> AuditEntry` — 변환 함수
- `cross_project_audit::emit(event, logger: &AuditLogger)` — wraps `logger.log_entry()`
- v1 canonical fingerprint, guard_layer, correlation_id, asserted_origin/target 등은 `AuditEntry::details: Option<serde_json::Value>` 안에 packed
- `result = AuditResult::Failed("cross_project_block:{reason}")` 형식
- mask_sensitive 자동 적용 (audit.rs 기존 기능)
- 결과: 기존 audit 파일(`~/Library/Caches/nexttui/audit.log`)에 동일 line 형식으로 합류 → grep/감사 도구 일관성 ↑

#### B. all_tenants 인프라 (광범위 사용, BL-P2-032 도입)

`Arc<AtomicBool>` 기반 admin 전용 Ctrl+A 토글이 **이미 우리 BL의 일부 가정을 충족**:
- `src/app.rs:52, 254, 528` — toggle handler
- `src/worker.rs:56-645` — 모든 list 호출에 `all_tenants` 전달
- `src/port/types.rs:209-236` — `ServerListFilter`/`VolumeListFilter`/`NetworkListFilter`/`SecurityGroupListFilter`/`FloatingIpListFilter`/`SnapshotListFilter` 모두 `pub all_tenants: bool` 보유
- `src/ui/header.rs, ui/theme.rs` — 활성화 시 UI 배지

**갱신된 정책 명시 (이전 plan 누락분)**: **all_tenants 토글은 read-side만 영향. mutation은 항상 active scope.** admin이 Ctrl+A로 의도적으로 모든 프로젝트를 보다가 mutation 시도해도 FR2 worker origin-guard로 차단. 사용자 경험: 보기는 가능하지만 수정은 불가.

#### C. Adapter filter 적용 실태 (Neutron이 진짜 버그, Nova/Cinder는 정상)

| Adapter | filter 처리 | 상태 |
|---------|------------|------|
| `build_server_query` (nova.rs:175) | `name`/`status`/`host`/`flavor`/`all_tenants` 모두 처리 | ✅ 정상 |
| `build_volume_query` (cinder.rs:133) | `name`/`status`/`all_tenants` 처리 | ✅ 정상 |
| `build_snapshot_query` (cinder.rs:124) | `all_tenants` 처리 | ✅ 정상 |
| **`build_security_group_query`** (neutron.rs:231) | **`_filter` IGNORE** | ❌ **버그** |
| **`build_network_query`** (neutron.rs:225) | **`_filter` IGNORE** | ❌ **버그** |
| **`build_floating_ip_query`** (neutron.rs:240) | **`_filter` IGNORE** | ❌ **버그** |

→ **FR1 진짜 fix 대상은 Neutron 3개 빌더만**. Nova/Cinder는 이미 정상이라 변경 거의 없음 (defense-in-depth refilter만 추가).

#### D. CrossTenantGuard

`src/infra/cross_tenant.rs::CrossTenantGuard` (175 LoC, break-glass 모델 = 거절된 옵션 B)는 진짜 **dead code** (어떤 호출부도 없음). 이번 BL에선 무시. 후속 정리 BL에서 삭제 또는 all_tenants Arc<AtomicBool>과 통합.

## Files to Modify

- [x] `src/action.rs` — `DispatchedAction` struct 추가 (Action enum 옆) — Step 2 완료
- [x] `src/infra/mod.rs` — `cross_project_guard` 서브모듈 export — Step 1 완료
- [ ] `src/context/action_channel.rs` — `ActionSender` T 교체 (Action → DispatchedAction), `ScopeProvider` trait + send 내부 스탬핑, `ActionReceiver`도 T 교체
- [ ] `src/context/mod.rs` — (필요 시) `ScopeProvider` re-export
- [ ] `src/worker.rs` — (1) `action_to_kind()` exhaustive 보강 + RBAC parity 테스트. (2) `run_worker` DispatchedAction 수신 + origin guard hook + AuditLogger 주입. (3) list 호출부 `tenant_id` enrichment
- [ ] `src/infra/rbac.rs` — `check_project_scope()`, `update_roles_preserve_project()` 메서드 추가
- [ ] `src/port/types.rs` — `NetworkListFilter`/`SecurityGroupListFilter`/`FloatingIpListFilter`에 `pub tenant_id: Option<String>` 추가
- [ ] `src/adapter/http/neutron.rs` — **`_filter` IGNORE 패턴 fix** (build_network_query/build_security_group_query/build_floating_ip_query 본체 수정) + impl 내부 refilter wiring + audit_logger 필드 추가
- [ ] `src/adapter/http/nova.rs` — refilter wiring + audit_logger 필드 추가 (query는 이미 정상)
- [ ] `src/adapter/http/cinder.rs` — refilter wiring + audit_logger 필드 추가 (query는 이미 정상)
- [ ] `src/module/image/mod.rs` — DeleteImage/UpdateImage 경로에 adapter pre-mutation GET 추가
- [ ] `src/error.rs` — `AppError::CrossProjectBlocked { origin, active, reason }` variant 추가
- [ ] `src/app.rs` — `ActionSender` 생성자에 `ScopeProvider`(RbacGuard arc) 주입 + `Arc<AuditLogger>` 어댑터/워커에 분배. 테스트 헬퍼 `FakeActionSender` 갱신
- [ ] `src/ui/mod.rs` — `cross_project_toast` 서브모듈 export
- [ ] `src/module/mod.rs` — `common` 서브모듈 export

## Policy Clarification (RED-time 발견 반영)

**all_tenants 토글과 정책 A의 분업** — 새 명시:
- **Read-side**: admin이 Ctrl+A로 `all_tenants=true` 활성화 → `all_tenants=1` 쿼리 추가 → 모든 프로젝트 리소스 조회 가능. refilter SKIP. 정상 동작.
- **Read-side strict**: `all_tenants=false` (기본) → `tenant_id={active_scope}` 주입. server-side가 enforce. 추가로 client-side refilter로 defense-in-depth (서버 enforcement 누락 방어).
- **Write-side**: `all_tenants` 상태와 **무관하게** 항상 active scope만 mutation 허용. FR2 worker origin-guard가 강제. admin이 list로 cross-project를 봤어도 mutation은 차단.
- **사용자 경험**: all_tenants=true일 때 admin이 cross-project 리소스를 보고 mutation 시도 → "보기는 가능, 수정은 불가 + 토스트로 :switch-project 안내"

이 정책은 requirements.md / application-design.md의 시맨틱과 정합하며, BL-P2-032(Ctrl+A 토글)와 자연스럽게 공존.

## Implementation Steps

### Phase 1 — Foundation (pure, no dependencies)

- [x] **Step 1**: `CrossProjectGuard` 순수 함수 모듈 — 9 tests passed, total 1379 (+9)
  - [x] RED+GREEN: `check_origin_scope` / `check_form_selection` / `GuardDecision` / `CrossProjectReason` (4 variants 포함 UnscopedFailSafe) / `GuardLayer` (4 variants) — `src/infra/cross_project_guard.rs`
  - [x] Verify GREEN: 회귀 0

- [x] **Step 2**: `DispatchedAction` struct — 2 tests passed, total 1381 (+2)
  - [x] RED+GREEN: struct + `stamped(action, origin)` / `unstamped(action)` / `is_stamped()` helpers — `src/action.rs`
  - [x] Verify GREEN: 회귀 0

### Phase 2 — Error variant

- [x] **Step 3**: `AppError::CrossProjectBlocked` — 1 test passed, total **1382** (+1)
  - [x] RED: `tests::test_cross_project_blocked_error_display` in `src/error.rs` — display 메시지 포함 요소 assertion (variant 미존재 컴파일 실패 확인)
  - [x] Verify RED
  - [x] GREEN: `CrossProjectBlocked { reason: CrossProjectReason, guard_layer: GuardLayer }` variant + `#[error("Cross-project operation blocked: {r} (layer: {l})", r = reason.as_str(), l = guard_layer.as_str())]` + `crate::infra::cross_project_guard` import
  - [x] Verify GREEN: 기존 `test_app_error_display` + 신규 테스트 + 회귀 0 + clippy `-D warnings` clean

### Phase 3 — Audit infrastructure

- [x] **Step 4**: `CrossProjectBlockEvent` builder → AuditLogger 통합 (RED-time 발견 반영) — 8 tests passed, total **1390** (+8). `sha2 = "0.10"` dep 추가. 5분 audit.rs 시그니처 사전 검증으로 매핑 정확도 확보.
  - **위치**: `src/infra/cross_project_audit.rs` (단일 파일. 기존 `src/infra/audit.rs::AuditLogger` 재사용)
  - **설계**:
    ```rust
    pub struct CrossProjectBlockEvent {
        pub timestamp: DateTime<Utc>,
        pub actor_user_id: String,         // Keystone UUID
        pub actor_cloud: String,
        pub active_project_id: Option<String>,
        pub asserted_origin_project_id: Option<String>,
        pub target_project_id: Option<String>,
        pub action_type: String,
        pub resource_kind: String,
        pub resource_id: Option<String>,
        pub resource_name: Option<String>,
        pub reason: CrossProjectReason,
        pub guard_layer: GuardLayer,
        pub correlation_id: u64,           // epoch
    }

    impl CrossProjectBlockEvent {
        pub fn to_audit_entry(&self) -> AuditEntry { ... }  // details JSON에 fingerprint, guard_layer, correlation_id, asserted_origin, target packing
        pub fn fingerprint(&self) -> String { ... }         // v1 canonical: "v1|" + user + "|" + active + "|" + origin + "|" + target + "|" + action_type + "|" + resource_id, sha256.hex()[..12]
    }

    pub fn emit(event: &CrossProjectBlockEvent, logger: Option<&AuditLogger>) {
        // logger 있으면 logger.log_entry(event.to_audit_entry()) 시도
        // 없거나 실패 시 tracing::warn! fallback (best-effort)
    }
    ```
  - [x] RED:
    - [x] `test_event_to_audit_entry_field_mapping`
    - [x] `test_audit_entry_details_contains_fingerprint_guard_layer_correlation_id`
    - [x] `test_audit_entry_result_is_failed_with_reason_string`
    - [x] `test_fingerprint_v1_canonical_format` — canonical 문자열 재유도 + sha256[..6] hex 비교 (impl과 독립)
    - [x] `test_fingerprint_boundary_collision_free`
    - [x] `test_fingerprint_none_resource_id_uses_empty`
    - [x] `test_emit_with_logger_writes_audit_entry` — `tempfile::TempDir` 패턴
    - [x] `test_emit_without_logger_fallback_to_tracing`
  - [x] Verify RED (4 compile errors: struct + emit fn 미존재)
  - [x] GREEN: 위 설계 구현. `chrono::DateTime<Utc>`, `sha2::Sha256` (신규 dep `sha2 = "0.10"`), `serde_json::Value` 활용. `write!` macro로 hex encode (clippy `unwrap_used = deny` 회피)
  - [x] Verify GREEN: 회귀 0, clippy `-D warnings` clean
  - [x] **Codex P2 review fix** (2026-04-27): `emit()` success branch에 `logger.rotate_if_needed()` 추가. `App::record_audit` (src/app.rs:780)의 패턴과 parity 확보 — `MAX_LOG_SIZE`(10MB)/`MAX_ROTATED_FILES`(5) 정책이 cross_project_block 폭발 시에도 작동. 회귀 0 (1390 stable, 5/5 runs).

  **사전 검증 결과** (audit.rs 5분 점검, plan과 차이):
  - `AuditEntry.timestamp`: plan `DateTime<Utc>` → 실제 `String` (ISO). `to_rfc3339()`로 변환.
  - `AuditEntry.resource_id`: plan `Option<String>` → 실제 `String` (NOT Option). `unwrap_or_default()` 사용.
  - 필드 rename: `action_type → action`, `resource_kind → resource_type`.
  - `sha2`는 plan이 "이미 deps"라고 잘못 가정 — Cargo.toml에 부재. 옵션 (a) `sha2 = "0.10"` 추가로 결정.
  - `AuditLogger.log_entry(&self, entry) -> Result<()>` sync. emit best-effort 패턴 그대로 적용.
  - 자동 sensitive masking 작동 — 우리 details 키는 충돌 0.

### Phase 4 — RBAC 확장

- [x] **Step 5**: `RbacGuard::check_project_scope()` — 7 tests passed
  - [x] RED (admin match/mismatch / unscoped fail-safe / reader create match/mismatch / member admin-only action / reader read match): 23 compile errors (enum + 2 methods 미존재)
  - [x] Verify RED
  - [x] GREEN: `RbacScopeDecision { Allow, Deny { reason: RbacDenialReason } }` + `RbacDenialReason { RoleTier, ProjectScope, Both } + as_str()` + `check_project_scope(target, action) -> RbacScopeDecision` (`is_some_and(|p| p == target)`로 None=fail-safe)
  - [x] Verify GREEN: reason 정확히 `role_tier`/`project_scope`/`both` 반환

- [x] **Step 6**: `RbacGuard::update_roles_preserve_project()` — 4 tests passed (Step 5와 동일 commit)
  - [x] RED:
    - [x] `test_preserve_project_keeps_existing_project_id`
    - [x] `test_preserve_project_updates_roles_and_effective`
    - [x] `test_update_roles_vs_preserve_diff` (회귀 방지)
    - [x] `test_preserve_project_clears_capabilities_parity_with_update_roles` (parity)
  - [x] Verify RED
  - [x] GREEN: `update_roles_preserve_project(roles)` 신규 — `self.state.write()` 잡고 `roles + effective_role` 갱신 + `capabilities.clear()` (update_roles와 parity), `project_id` 보존
  - [x] Verify GREEN: 회귀 0 (1390 → 1401, +11), clippy clean

  **사전 검증 결과** (rbac.rs 5분 점검): plan 가정 모두 정확. RbacGuard structure / RwLock / can_perform 매트릭스 / TokenRole role(name) helper 패턴 그대로 채용.

### Phase 5 — Action 분류 exhaustive

- [x] **Step 7**: `action_to_kind()` exhaustive + RBAC parity 테스트 (Reviewer Should-consider #1 반영) — 5 tests passed, total **1406** (+5)
  - [x] RED:
    - [x] `test_action_to_kind_fetch_and_nav_variants_return_none` — 28개 None variant 일괄 (Fetch*/Navigate/Back/UI/system/SwitchBack)
    - [x] `test_action_to_kind_all_mutations_have_kind` — 38개 mutation variant 일괄 (param shape 사전 점검 필요했음)
    - [x] `test_action_is_mutation_helper_parity` — `action_to_kind.is_some() == action_is_mutation`
    - [x] `test_action_to_kind_rbac_mapping_lockstep` — 11 mutation × ActionKind 명시 매핑 (Create/Delete/ForceDelete/Resize/Migrate/Evacuate/EnableDisable/ManageQuota/Attach/Detach)
    - [x] `test_action_to_kind_switch_context_returns_none` — orchestration vs mutation 구분
  - [x] Verify RED (3 errors: `action_is_mutation` 미존재. param shape mismatch 13건은 사전점검 누락분으로 즉시 수정)
  - [x] GREEN: `_ => None` fallthrough 제거 + 모든 Fetch/Nav/UI/System/Context variant 명시. `pub(crate) fn action_to_kind` + `pub(crate) fn action_is_mutation(action: &Action) -> bool` 도입. `#[allow(dead_code)]` on action_is_mutation (Phase 6 Step 9에서 ActionSender가 wire)
  - [x] Verify GREEN: 1406 stable, clippy `-D warnings` clean

  **컴파일타임 안전망 확보**: 새 Action variant 추가 시 `action_to_kind` 컴파일 실패 — silent miss 불가능

### Phase 6 — Envelope 교체 (가장 침습적)

- [x] **Step 8**: `ScopeProvider` trait — 3 tests passed, total **1410** (+3). commit `73b347d`
  - [x] RED: 3 tests in `src/context/action_channel.rs::tests` — fails to compile (ScopeProvider undeclared)
  - [x] Verify RED (E0433: cannot find type `ScopeProvider`)
  - [x] GREEN: `pub trait ScopeProvider: Send + Sync { fn current_project_id(&self) -> Option<String>; }` + `impl ScopeProvider for Arc<RbacGuard>` (delegates to `RbacGuard::project_id()` which is single-snapshot atomic post-Codex P1 hotfix `b4b4c44`)
  - [x] Verify GREEN — cargo test 1410 pass, clippy -D warnings clean
  - [x] Bonus test `test_scope_provider_reflects_post_update_change` — locks in *live read* contract (FR2 stamping requires live state, not captured snapshot)

- [x] **Step 9**: `ActionSender` 타입 교체 + 스탬핑 — 3 tests passed, total 1413 (+3). commit `1f80968` (Step 9+10 단일 atomic)
  - [ ] RED:
    - `test_sender_stamps_mutation_with_current_scope` — scope=A에서 send(CreateServer) → envelope.payload.origin_project_id == Some("A")
    - `test_sender_leaves_readonly_unstamped` — send(FetchServers) → origin == None
    - `test_sender_handles_unscoped_provider_returns_none_origin`
    - 기존 ActionReceiver 테스트 회귀 없음 검증
  - [ ] Verify RED (컴파일 실패 포함 — tx 타입 바뀌면 app.rs 호출부 정합성 문제 → 다음 Step에서 해결)
  - [ ] GREEN:
    - `ActionSender { tx: mpsc::UnboundedSender<VersionedEvent<DispatchedAction>>, epoch: Arc<ContextEpoch>, scope_provider: Arc<dyn ScopeProvider> }`
    - `send(action: Action)` 내부: mutation이면 origin=Some(scope_provider.current_project_id().unwrap_or_default()), read-only면 None → DispatchedAction wrap → VersionedEvent wrap → tx send
    - `ActionReceiver`도 `VersionedEvent<DispatchedAction>`으로 교체. `recv()`는 기존 호환을 위해 `Option<Action>` 유지하고 `.map(|d| d.action)` 내부 unwrap.
    - **Worker 수신 경로 명시 (Reviewer Should-consider #2)**: `run_worker`는 **ActionReceiver를 쓰지 않고 raw `mpsc::UnboundedReceiver<VersionedEvent<DispatchedAction>>`를 직접 소비**해야 origin_project_id에 접근 가능. ActionReceiver는 테스트 호환 목적으로만 유지. Step 11에서 worker 측 raw 소비 코드로 전환.
  - [ ] Verify GREEN: 새 테스트 통과 + 기존 ActionReceiver 테스트 통과

- [x] **Step 10**: `app.rs` + 테스트 헬퍼 갱신 — Step 9와 단일 commit `1f80968`. main.rs RbacGuard Arc 공유, ActionReceiver 외부 API 보존하여 ~100 module 테스트 무수정 무회귀.
  - [ ] RED: 컴파일 에러로 드러나는 위치들 정리. 기존 테스트가 ActionSender를 생성하는 곳마다 ScopeProvider 주입 필요
  - [ ] Verify RED (compile fail)
  - [ ] GREEN: `ActionSender::new(...)` 생성자에 scope_provider 인자 추가. `App::new()` 등에서 RbacGuard arc 전달. 테스트 헬퍼 `FakeActionSender` / `FakeScopeProvider` 추가
  - [ ] Verify GREEN: `cargo test` 전체 green (1370 + 신규)

### Phase 7 — Worker hook

- [ ] **Step 11**: `run_worker` C5 pseudo-flow + AuditLogger 통합 (RED-time 발견 반영)
  - **Worker signature 확장**: `run_worker(...)`가 `Option<Arc<AuditLogger>>` 추가 인자 (test 호환). 기존 worker는 audit_logger 부재로도 동작 가능 (best-effort).
  - [ ] RED:
    - `test_worker_allows_mutation_when_origin_matches`
    - `test_worker_blocks_mutation_when_origin_mismatch` — emit된 AuditEntry가 `result == AuditResult::Failed("cross_project_block:origin_scope_mismatch")` + details에 `guard_layer="fr2_worker"` + `correlation_id == envelope.epoch()` 포함
    - `test_worker_allows_readonly_without_guard`
    - `test_worker_emits_toast_before_any_api_error` — Warning 토스트 동기 emit 순서
    - `test_worker_block_works_without_audit_logger` — logger=None일 때도 차단 동작
  - [ ] Verify RED
  - [ ] GREEN:
    ```rust
    // src/worker.rs:53 근처
    let (dispatched, epoch) = envelope.into_parts();
    if !ctx.epoch_matches(epoch) { continue; }
    let DispatchedAction { action, origin_project_id } = dispatched;
    if let Some(origin) = origin_project_id {
        let current = rbac.project_id().unwrap_or_default();
        match cross_project_guard::check_origin_scope(&origin, &current) {
            GuardDecision::Allow => proceed_dispatch(action).await,
            GuardDecision::Block { reason } => {
                let event = CrossProjectBlockEvent::new(
                    reason.clone(),
                    GuardLayer::Fr2Worker,
                    &action,
                    &token_info,
                    epoch,
                );
                cross_project_audit::emit(&event, audit_logger.as_deref());
                toast_tx.send(CrossProjectToast::build_origin_mismatch(&active_name, &origin_name)).await;
                continue;
            }
        }
    } else {
        proceed_dispatch(action).await;  // read-only
    }
    ```
  - [ ] Verify GREEN

### Phase 8 — Adapter FR1

- [ ] **Step 12**: Neutron query builder fix (`_filter` IGNORE 패턴 제거) — RED-time 발견 반영
  - **버그 위치 실측 확정**: `src/adapter/http/neutron.rs:225 build_network_query`, `:231 build_security_group_query`, `:240 build_floating_ip_query` — 모두 `_filter` 접두사로 무시 중. Nova/Cinder는 이미 정상이라 제외.
  - **Filter struct 확장**: `NetworkListFilter`/`SecurityGroupListFilter`/`FloatingIpListFilter`(`src/port/types.rs:233-`)에 `pub tenant_id: Option<String>` 추가. 기존 `all_tenants: bool`은 유지.
  - **Worker enrichment**: `src/worker.rs`의 list 호출부에서 token에서 active project_id 추출하여 filter에 채워 넣음 (`tenant_id: Some(rbac.project_id().unwrap_or_default())`)
  - **Subnet 처리**: `list_subnets(network_id)`은 device 범위 내포라 변경 없음 (Q1 매트릭스 결정)
  - [ ] RED:
    - `test_build_security_group_query_injects_tenant_id_when_all_tenants_false`
    - `test_build_security_group_query_uses_all_tenants_1_when_true_skips_tenant_id`
    - `test_build_security_group_query_no_op_when_no_tenant_id_no_all_tenants` (fail-safe — 둘 다 없으면 query 비워둠)
    - `test_build_network_query_injects_tenant_id_when_all_tenants_false`
    - `test_build_network_query_all_tenants_true_branch`
    - `test_build_floating_ip_query_injects_tenant_id_when_all_tenants_false`
    - `test_build_floating_ip_query_all_tenants_true_branch`
    - 각 테스트: `query` 문자열에 `tenant_id={scope}` 포함 또는 `all_tenants=1` 포함 + pagination 유지
  - [ ] Verify RED
  - [ ] GREEN:
    ```rust
    // src/adapter/http/neutron.rs:231 (예시 — security_group)
    fn build_security_group_query(
        filter: &SecurityGroupListFilter,
        pagination: &PaginationParams,
    ) -> String {
        let mut parts = Vec::new();
        if filter.all_tenants {
            parts.push("all_tenants=1".to_string());
        } else if let Some(ref tid) = filter.tenant_id {
            parts.push(format!("tenant_id={}", encode_param(tid)));
        }
        append_pagination_parts(&mut parts, pagination);
        parts.join("&")
    }
    ```
    동일 패턴으로 build_network_query, build_floating_ip_query.
  - [ ] Verify GREEN: 신규 7-8 tests + 회귀 0

- [ ] **Step 13**: Adapter response refilter + adapter_filter audit event (Reviewer Must-fix #2 + RED-time 발견)
  - **policy 명시**: `all_tenants=true`(admin이 의도적으로 토글)인 경우 refilter SKIP. `all_tenants=false`인데 응답에 cross-project 섞이면 → drop + AdapterFilterViolation event (서버측 enforcement 실패 보호 = defense-in-depth)
  - **공통 pure fn 위치**: `src/adapter/http/scope_refilter.rs` 신규 (`src/adapter/http/mod.rs` export)
  - [ ] RED:
    - `test_refilter_drops_cross_project_items_when_scope_strict` — `active=A`, `all_tenants=false`, items mixed → cross-project 항목 드롭, dropped Vec 반환
    - `test_refilter_keeps_all_when_all_tenants_true` — `all_tenants=true` → no-op, 모든 항목 유지
    - `test_refilter_keeps_active_scope_items`
    - **`test_refilter_emits_cross_project_block_event_with_adapter_filter_reason`** — dropped > 0 시 각 드롭 항목마다 `CrossProjectBlockEvent`(reason=`AdapterFilterViolation { resource_id, project_id }`, guard_layer=`Fr1Adapter`) 1건 emit
    - `test_refilter_no_emit_when_strict_and_no_drops`
  - [ ] Verify RED
  - [ ] GREEN:
    ```rust
    // src/adapter/http/scope_refilter.rs
    pub trait HasTenantId {
        fn tenant_id(&self) -> Option<&str>;
        fn resource_id(&self) -> Option<&str>;
    }

    pub fn refilter_by_scope<T: HasTenantId>(
        items: Vec<T>,
        active: Option<&str>,
        all_tenants: bool,
    ) -> (Vec<T>, Vec<T>) {
        if all_tenants { return (items, Vec::new()); }
        let Some(active) = active else { return (items, Vec::new()); };
        let mut kept = Vec::new();
        let mut dropped = Vec::new();
        for item in items {
            match item.tenant_id() {
                Some(tid) if tid == active => kept.push(item),
                _ => dropped.push(item),
            }
        }
        (kept, dropped)
    }
    ```
    - `HasTenantId` impl for **SecurityGroup / Network / FloatingIp** (Neutron Step 13) + **Server (Nova) / Volume / Snapshot (Cinder)** (Step 14)
    - list_* 내부 wiring: paginated_list 결과 → refilter_by_scope 적용 → dropped iterate → `CrossProjectBlockEvent::new(AdapterFilterViolation { resource_id, project_id }, GuardLayer::Fr1Adapter, ...)` + `cross_project_audit::emit(&event, logger)` (각 항목별)
    - **AuditLogger 접근**: NeutronHttpAdapter에 `audit_logger: Option<Arc<AuditLogger>>` 필드 추가 (생성자 변경) — App에서 주입
  - [ ] Verify GREEN

- [ ] **Step 14**: Nova/Cinder defense-in-depth refilter (이미 query 정상이라 query 수정 불필요 — RED-time 발견 반영)
  - **상태 확정**: `build_server_query` (nova.rs:175), `build_volume_query` (cinder.rs:133), `build_snapshot_query` (cinder.rs:124) 모두 이미 `all_tenants` flag 정상 처리. **추가 query 변경 불필요**.
  - **이번 step 범위**: defense-in-depth로 **응답측 refilter만 추가** + `HasTenantId` trait impl 추가
  - [ ] RED:
    - `test_nova_server_has_tenant_id_impl_extracts_field`
    - `test_cinder_volume_has_tenant_id_impl_extracts_project_id_field`  (Cinder는 `project_id`/`tenant_id` 필드명이 둘 다 사용됨, models 확인)
    - `test_cinder_snapshot_has_tenant_id_impl`
    - `test_nova_list_servers_refilter_drops_cross_project_when_strict` — `all_tenants=false`에서 mock 응답에 cross-project Server 섞어 → drop + AdapterFilterViolation event 1건
    - `test_cinder_list_volumes_refilter_drops_cross_project_when_strict`
  - [ ] Verify RED
  - [ ] GREEN:
    - `HasTenantId` impl for `Server`, `Volume`, `Snapshot` (models 확인 후 정확 필드명 매핑)
    - `list_servers`, `list_volumes`, `list_snapshots` impl 내부에 `refilter_by_scope` 호출 + AdapterFilterViolation event emit (Step 13의 wiring 패턴 재사용)
    - NovaHttpAdapter, CinderHttpAdapter에 `audit_logger: Option<Arc<AuditLogger>>` 필드 추가
  - [ ] Verify GREEN

### Phase 9 — Form validation FR4

- [ ] **Step 15**: `FormSelectedIdValidator` 공통 헬퍼
  - [ ] RED:
    - `test_validate_single_selection_match_passes`
    - `test_validate_single_selection_mismatch_returns_error`
    - `test_validate_multi_selection_first_mismatch_wins`
    - `test_validation_error_carries_field_name_and_reason`
  - [ ] Verify RED
  - [ ] GREEN: `src/module/common/scope_validator.rs` 생성. `pub struct FormValidationError { field, reason: CrossProjectReason }` + `pub fn validate_form_scope<'a>(active, selections) -> Result<(), FormValidationError>`
  - [ ] Verify GREEN

- [ ] **Step 16**: Glance DeleteImage/UpdateImage pre-mutation GET (Reviewer Must-fix #3 확인: 어댑터 surface 실측)
  - **Adapter surface 실측 (plan-time)**: ✅ 확인됨
    - `src/adapter/http/glance.rs:115 async fn get_image(&self, image_id) -> ApiResult<Image>` 존재
    - `src/models/glance.rs:20 pub owner: Option<String>` 존재 → project_id 매핑
  - [ ] RED:
    - `test_image_delete_rejects_cross_project_owner` — mock GET returns image with owner=B, active=A → form submit 거부 + toast
    - `test_image_delete_allows_same_project_owner`
    - `test_image_update_rejects_cross_project`
    - `test_image_delete_with_missing_owner_fail_safe` — owner=None → deny (fail-safe)
    - `test_image_delete_emits_cross_project_block_event_with_fr4_form_layer`
  - [ ] Verify RED
  - [ ] GREEN: `src/module/image/mod.rs`의 Delete/Update 경로에서 submit 직전 `self.adapter.get_image(id).await` → `image.owner` 추출 → `CrossProjectGuard::check_form_selection(owner, active)` 호출 → Block이면 event emit(`GuardLayer::Fr4Form`) + reject + toast
  - [ ] Verify GREEN

### Phase 10 — UI FR6

- [ ] **Step 17**: `CrossProjectToast` 공통 카피
  - [ ] RED:
    - `test_origin_mismatch_toast_contains_both_project_names`
    - `test_glance_owner_mismatch_toast_contains_owner_name`
    - `test_form_mismatch_toast_contains_field_label`
    - `test_toast_level_is_warning`
    - `test_toast_respects_safe_display_60char_truncate`
  - [ ] Verify RED
  - [ ] GREEN: `src/ui/cross_project_toast.rs` 생성. 3 builder 함수. `ToastLevel::Warning`, `safe_display(name, 60)` 적용
  - [ ] Verify GREEN

### Phase 11 — Background Task Audit (Reviewer Must-fix #4)

- [ ] **Step 18**: Background task action emit 경로 scope 검증
  - 대상: `src/worker.rs` 내 `poll_migration_progress`, `poll_volume_attachment`, `poll_server_status` 등 background task가 `ActionSender::send()`를 호출하는 경로.
  - 우려: 중앙 스탬핑(Step 9)이 ActionSender 레벨에서 동작하므로 background task가 **동일 ActionSender를 clone해서 사용한다면** 자동으로 current scope 스탬프. 그러나 **background task가 spawn 시점의 Arc 복제만 보유하고, scope 전환 후에도 "과거 scope 문맥에서 발생한 작업을 과거 scope로 stamp하려는" 의도라면 갈등 가능**.
  - [ ] RED:
    - `test_background_poll_emits_action_with_current_scope_stamp` — background task가 emit한 Action이 **현재 시점** active scope로 스탬프되는지 assertion (의도: 사용자가 이미 B로 전환했으면 background도 B 기준으로 차단 판단)
    - `test_background_poll_stale_action_blocked_after_scope_switch` — poll_server_status가 오래 걸려서 완료된 시점에 scope=B면 origin=A로 스탬프되어 차단
  - [ ] Verify RED
  - [ ] GREEN: 대부분 기존 ActionSender 재사용으로 충족. 별도 코드 변경은 일반적으로 불필요 — **단, spawn 시 ActionSender clone이 같은 scope_provider arc를 공유함을 주석으로 명시**. 필요 시 worker.rs의 polling 함수에 DocComment 추가
  - [ ] Verify GREEN

## Test Strategy

| Test file | 주 검증 | 총 테스트 수 (예상) |
|-----------|---------|-------------------|
| `src/infra/cross_project_guard.rs::tests` | 순수 가드 의사결정 매트릭스 | ~8 |
| `src/action.rs::tests` (DispatchedAction 부분) | 엔벨로프 구조 | 3 |
| `src/error.rs::tests` | CrossProjectBlocked 메시지 | 1 |
| `src/infra/audit/cross_project_block.rs::tests` | 스키마 + fingerprint canonicalization | 5-6 |
| `src/infra/rbac.rs::tests` | scope matrix + preserve | 6-8 |
| `src/worker.rs::tests` (action_to_kind 보강 + RBAC parity) | 전 variant parity + RBAC lockstep | 6-8 |
| `src/context/action_channel.rs::tests` | ScopeProvider + 스탬핑 | 5 |
| `src/worker.rs::tests` (guard hook + background poll) | allow/block/order + poll stamp | 6 |
| `src/adapter/http/neutron.rs::tests` | query + refilter + adapter_filter event | 9-10 |
| `src/adapter/http/nova.rs::tests`, `cinder.rs::tests` | all_tenants 정책 + refilter T impl | 6-8 |
| `src/module/common/scope_validator.rs::tests` | form validation | 4 |
| `src/module/image/mod.rs::tests` (Glance pre-check) | pre-mutation GET + reject + fail-safe + event | 5 |
| `src/ui/cross_project_toast.rs::tests` | 카피 + safe_display | 5 |
| **합계** | — | **~65 신규 테스트** |

**Plan-level 명시적 defer**: synthesis Should-consider #3 "e2e mock integration test merge-blocking" 은 mock HTTP 서버 도입 없이는 기존 패턴 안에서 e2e 재현이 어려움. 이번 BL에선 **각 계층 단위 테스트 조합 + adapter_filter 이벤트 end-to-end trace**로 cover. 전면 e2e는 BL-P2-081에서 자연스럽게 도입.

## Verification Contract

### 완료 조건
- [ ] 6 FR 모두 해당 수용기준 충족 (requirements.md 참조)
- [ ] Must-fix 8건, Should-consider 5건 반영 확인 (synthesis.md 매핑)
- [ ] 신규 테스트 ~60건 전부 green
- [ ] 기존 1370 tests 회귀 없음 → 총 **~1430 tests green**
- [ ] Clippy `-D warnings` 통과 (unwrap/expect/enum_glob_use deny 유지)
- [ ] `cargo fmt --check` 통과
- [ ] DispatchedAction + ActionSender 중앙 스탬핑으로 구현 (55 call site 수정 없음)

### 검증 명령
- `cargo test --lib` — 전체 라이브러리 테스트
- `cargo test --test devstack_directory` — integration test (BL-P2-080 placeholder)
- `cargo clippy --all-targets -- -D warnings` — lint
- `cargo fmt --check` — 포맷
- `grep -rn "not yet implemented\|todo!()\|unimplemented!()" src/` — 신규 stub 도입 없음 확인

### 리스크 태그
- [x] **auth/security** — P0 권한 경계 변경 (이 BL의 본질)
- [ ] DB schema change — 해당 없음 (로컬 파일 로그만 변경)

### 예상 diff 규모
- 신규 파일: 6 (cross_project_guard, audit/mod, audit/cross_project_block, cross_project_toast, module/common/mod, module/common/scope_validator)
- 수정 파일: ~14 (action, action_channel, worker, rbac, infra/mod, neutron, nova, cinder, image/mod, error, app, ui/mod, module/mod, context/mod)
- LoC 추가: 추정 ~1200-1600 lines (테스트 포함)

## 기존 테스트 회귀 위험 지점

1. **ActionSender 타입 교체 (Step 9-10)**: `ActionSender::new` 시그니처 변경 → 모든 테스트 헬퍼의 생성자 호출부 영향. 대책: `FakeScopeProvider` 테스트 유틸 제공, 테스트별 유지보수
2. **action_to_kind exhaustive (Step 7)**: `_ => None` 제거 → 신규 Action variant가 컴파일 차단 (의도된 효과, 그러나 PR#82 직후 머지된 신규 variant와 충돌 가능성 낮음)
3. **Neutron response refilter (Step 13)**: 기존 list 테스트가 "response에 포함된 모든 항목"을 검증하면 깨짐. 대책: mock 응답을 active_scope와 일치하도록 조정하거나 테스트가 refilter 통과 후 기대값 assertion
4. **Image module GET 호출 추가 (Step 16)**: mock adapter가 `get_image`를 제공해야 함. 기존 mock에 없으면 신규 추가 필요

## TDD 위반 방지 체크
- [ ] 각 Step별 "RED → Verify RED → GREEN → Verify GREEN" 네 포인트 엄수
- [ ] 테스트 작성 전 프로덕션 코드 타이핑 금지
- [ ] 각 Step GREEN 직후 전체 `cargo test --lib` 실행하여 회귀 감지

## Self-Review 항목 (모든 Step 완료 후)
- [ ] Council synthesis must-fix 8건 모두 반영
- [ ] Council synthesis should-consider 5건 반영 + plan-reviewer must-fix 4건 반영 (Step 7 RBAC parity, Step 11 guard_layer/correlation_id, Step 13 adapter_filter event, Step 14 Nova/Cinder refilter, Step 16 adapter surface 확인, Step 18 background poll audit)
- [ ] `_shared/tdd-protocol.md` Iron Law 준수
- [ ] requirements.md의 FR1~FR6 + NFR1~NFR4 모두 대응
- [ ] A1/A2/A3 Assumption 실측 반영 완료

## Plan Review Response (Plan-Reviewer must-fix → 본 plan 반영 매핑)

| Plan-Reviewer Must-fix | 반영 위치 | 상태 |
|------------------------|-----------|------|
| #1 Step 11 guard_layer + correlation_id 명시 | Step 11 RED+GREEN 개정 | ✅ |
| #2 Step 13 adapter_filter CrossProjectBlockEvent emit | Step 13 RED/GREEN 개정 | ✅ |
| #3 Step 16 adapter surface 실측 확인 | Step 16 상단 `Adapter surface 실측` 섹션 추가 | ✅ |
| #4 Background-task mutation audit | Step 18 신규 (Phase 11) | ✅ |

| Plan-Reviewer Should-consider | 반영 위치 | 상태 |
|-------------------------------|-----------|------|
| #1 Step 7 RBAC lockstep parity | Step 7 신규 `test_action_to_kind_rbac_mapping_lockstep` | ✅ |
| #2 Step 9 worker raw mpsc 명시 | Step 9 GREEN 주석 | ✅ |
| #3 Step 14 HasTenantId for Server/Volume/Snapshot | Step 14 GREEN 확장 | ✅ |
| #4 e2e mock integration test | Test Strategy 끝에 defer 명시 | 🟡 (defer 정당화) |
| #5 Toast 순서 assertion | Step 11 `test_worker_emits_toast_before_any_api_error` | ✅ (이미 존재) |
