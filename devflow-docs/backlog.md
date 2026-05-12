# Backlog

## Pending

### BL-P2-088: Live-migrate pre-flight stale port binding check — P3
**Priority**: Low (편의 — BL-P2-086 진단 기능 사용 후 진짜 필요한지 결정)
**Category**: UX / Defensive
**Parent**: BL-P2-086 deferred-C

**Scope**:
- LiveMigrate 호출 직전 인스턴스의 모든 port에 대해 `list_port_bindings` 사전 조회. INACTIVE+migrating_to 발견 시 호출 자체 차단 + 사용자에게 "stale binding 정리 후 재시도" 안내 (또는 BL-P2-087 cleanup action 안내).
- 매번 N번의 추가 API 호출 비용 → 기본 off, settings opt-in (`live_migrate_preflight_check`).
- BL-P2-086에서 자동 fetch한 `cached_port_bindings`가 fresh하면 재호출 skip하는 캐시 활용도 검토.

**Acceptance**:
- 새 settings 키 + opt-in 시에만 동작
- BL-P2-086 cache가 있으면 재호출 skip
- 신규 테스트 4~6개

**Open question**: BL-P2-086 사용자 피드백 수집 후 진짜 가치 있는지 평가. 진단만으로 충분하면 이 항목 폐기.

### BL-P2-087: Port binding cleanup action — P2
**Priority**: Medium (BL-P2-086 진단을 발견 → 즉시 복구 경로 완결)
**Category**: UX / Recovery action (admin-only, destructive)
**Parent**: BL-P2-086 deferred-B

**Problem**: BL-P2-086으로 stale binding을 발견할 수는 있지만 정리는 controller VM의 Neutron REST API 직접 호출로만 가능. UX dead-end가 줄긴 했지만 완결이 아님.

**Scope**:
- Server Detail의 admin 액션에 `Cleanup stale port binding` 추가 (전용 keybinding, 예: `Shift+P`)
- INACTIVE+migrating_to 행만 후보로 list, 사용자가 어느 host의 binding을 지울지 명시 선택
- Confirm modal에 정확한 페이로드 표시: "DELETE /v2.0/ports/{id}/bindings/{host}"
- Neutron `binding-extended` extension 기본 정책이 admin조차 거부할 수 있음 (실증 — devstack 기본은 `delete_port_binding` 거부) → 403 시 hint: "Neutron policy `/etc/neutron/policy.json` 확인 필요"
- 호출 직후 BL-P2-086의 `FetchPortBindingsForServer` 자동 재실행으로 즉시 반영

**Out of scope**:
- 정책 자체 수정은 nexttui 책임 아님 (운영자 작업)
- bulk cleanup (여러 인스턴스 동시) — 단일 인스턴스 단위로만

**Acceptance**:
- `NeutronPort::delete_port_binding(port_id, host)` trait 메서드 + mock + HTTP adapter
- Server module에 새 destructive action + RBAC ActionKind::Delete 또는 신규 ActionKind 추가 검토
- 403 에러 enrichment (policy 안내)
- 신규 테스트 6~8개, 신규 dep 없음, 1 PR

**Ref**: 2026-05-08 BL-P2-086 진단 세션에서 직접 cleanup 시도 시 403 만났고 Neutron policy 완화로 우회. nexttui로 처리하려면 같은 우회 패턴 + 친절한 에러 가이드 필요.

### BL-P2-082: KeystoneProjectDirectory security hardening (Unit 1 fast-follow medium findings) — P2
**Priority**: Medium (defense-in-depth, 실측 위험 낮음)
**Category**: Auth / Security
**Parent**: BL-P2-080 Unit 1 (commits cd1f54d + d294cee, R1 Stage 3 security review)

**Scope (3 items)**:

1. **Fingerprint scope 확장**
   - 현재: `TokenScopeFingerprint::compute(token)`이 `token.id` 문자열만 해시
   - 위험: `token.id`가 rescope 후에도 동일한 희귀 경우 (Keystone 구현 따라 이론적 가능) 또는 SipHasher13 64-bit collision(2^-32) 시 "다른 scope의 cache 반환"
   - 수정: key에 `token.project.id` + `token.project.domain_id` 포함 또는 fingerprint 자체를 128-bit 튜플 hash로 확장
   - 테스트: 동일 `token.id`, 다른 `token.project.id` → 다른 fingerprint verify

2. **Response size DoS cap**
   - `src/adapter/auth/keystone_project_directory.rs`: `reqwest` 기본 body unlimited. 악성 Keystone이 수 GB JSON 반환 시 OOM
   - 수정: `resp.bytes_stream()`으로 N MB(예: 5 MB) cap 또는 Content-Length 사전 검사. 페이지당 `parsed.projects.len() > N`(예: 10000) cap 추가
   - 테스트: mock이 limit 초과 응답 반환 → adapter가 reject (`ApiError::Parse("response too large")` 등)

3. **DirectoryCache lock poisoning recovery 일관성**
   - 현재: `DirectoryCache` `.write().ok()` 패턴이 poison 시 조용히 dead. `DomainNameResolver`는 `map_err(...)` 또는 `unwrap_or_else(|e| e.into_inner())` 패턴 사용
   - 수정: `DirectoryCache`도 `DomainNameResolver`와 동일한 recovery 패턴 적용. poison 발생 시 조용히 dead 대신 복구 후 계속 동작. 현재 poison 경로가 없어 실무 무해하나 후속 코드 유지보수 일관성 측면
   - 테스트: 해당 없음 (poison 경로 없으므로 코드 일관성만)

**Acceptance**:
- 3개 item 모두 구현 + 테스트 추가
- clippy `-D warnings` green, 신규 dep 없음
- BL-P2-080과 독립 PR로 shippable

**Ref**: BL-P2-080 Unit 1 R1 Stage 3 security review (2026-04-22) — high 2 + Important 1은 Unit 1 내 d294cee로 해결, 본 BL은 medium 3 fast-follow.

### BL-P2-080: Same-cloud HTTP ProjectDirectory (real UUID via `/v3/auth/projects`) — P0
**Status**: 🟡 PR #80 open (pending merge, 2026-04-23). 머지 후 이 항목 제거 + 이력 이동.
**Priority**: Critical (`:switch-project` 실동작 전제, `:switch-cloud`는 BL-P2-081과 결합해서 완결)
**Category**: Auth / Resolver

**Problem**: 2026-04-21 systematic-debugging 결과. 현재 `src/context/static_project_directory.rs`는 stub:
- `auth.project_name` 단건만 반환 → `:switch-project <다른이름>` 모두 `NotFound` (증상 2)
- `project_id = project_name` placeholder → rescope body의 `scope.project.id`에 UUID 아닌 이름("admin") 전송 → Keystone **401** (증상 1). 실증 Test A(placeholder)=401 vs Test B(real UUID)=201로 유일 변수 확정

`static_project_directory.rs:5-6` 주석이 "BL-P2-052 will replace this with an HTTP-based implementation"이라고 하지만 BL-P2-052 description에는 이 작업이 없음 — 코드 주석과 backlog 불일치. 이 BL이 그 gap을 닫음.

**Scope (Phase 1 — 명확 경계)**:
- **In scope**: **same-cloud** (`active_cloud()` == target cloud) `/v3/auth/projects` 조회. resolver의 `ById`/`ByName`/`CloudOnly` 세 경로 모두 real UUID 혜택을 받아 **`:switch-project` end-to-end 성공** + `:switch-cloud <same>` idempotent fast-path 정상화
- **Out of scope (→ BL-P2-081)**: cross-cloud directory 조회 (target cloud token 필요) + `KeystoneRescopeAdapter` 단일 `wire_auth_url` 바인딩 제거. BL-P2-080은 `ProjectDirectoryPort`만 교체하고 per-cloud auth factory는 BL-P2-081이 담당 — 두 BL은 독립 PR로 shippable하되 `:switch-cloud` cross-cloud full E2E는 양쪽 머지 후 green

**필요 작업 (acceptance)**:

1. **`KeystoneProjectDirectory` 신규 HTTP adapter** (`src/adapter/auth/keystone_project_directory.rs`):
   - `GET {auth_url}/v3/auth/projects` 호출, `X-Auth-Token: <current_token.id>`
   - 응답 파싱: `projects[].{id, name, domain_id}` + `links.next`
   - `ProjectCandidate { cloud, project_id, project_name, domain }` 매핑 — `domain`은 `domain_id` 기반 resolve (필요시 `GET /v3/domains/{id}` 단건 조회 또는 domain_id 자체를 name 대신 저장 — resolver 호환성 확인)

2. **Pagination**:
   - `links.next` URL을 따라 다음 페이지 가져오기, `next`가 null/absent일 때 종료
   - **DoS 방어**: max_pages 제한 (예: 100) — 초과 시 `ApiError::Parse("pagination runaway, >100 pages")`
   - 각 페이지 HTTP error 시 누적된 결과 버리고 에러 전파 (partial result 금지)
   - 테스트: devstack container에 `?limit=1` 쿼리로 pagination 강제 트리거 + 3페이지 이상 verify

3. **Per-cloud cache**:
   - Key: `(cloud_name, token_scope_fingerprint)` — token rescope 후 `token_scope`가 바뀌면 자동 miss → 자동 invalidate
   - TTL: 5분 (외부에서 프로젝트 생성 후 바로 전환하는 운영 시나리오 고려)
   - Explicit invalidation trigger: `AppEvent::ContextChanged` 수신 시 해당 cloud cache flush
   - 테스트: TTL 만료 후 재조회, context switch 후 cache miss 확인

4. **Concurrency**:
   - 기존 `CancellationRegistry` (`src/context/cancellation.rs`) 재활용 또는 epoch gate 적용 — in-flight directory 호출 중 새 switch 요청이 들어오면 stale 결과 drop
   - `ContextSwitcher`의 epoch와 연동 — `resolver.resolve()` 내부에서 epoch 비교
   - 테스트: 두 개의 빠른 switch request 시퀀스 → 첫 번째의 directory 응답이 두 번째 도중 도착해도 첫 번째 target이 commit되지 않음

5. **Resolver 세 경로 모두 수혜 검증**:
   - `ContextRequest::ByName { project: "admin" }` — directory에서 name=="admin" 매칭 → real UUID 반환
   - `ContextRequest::ById { project_id: "<uuid>" }` — directory candidate 중 UUID 매칭 (resolver.rs:110 `c.project_id == project_id` 정상 동작)
   - `ContextRequest::CloudOnly { cloud }` — `default_project`를 name으로 disambiguate → real UUID

6. **`StaticProjectDirectory` 처리**:
   - **삭제 금지** — 기존 테스트들이 의존. `#[cfg(test)]` 또는 `pub(crate)`로 강등, 파일을 `src/context/test_fixtures/static_project_directory.rs` (또는 유사) 로 이동
   - `main.rs` wiring은 `KeystoneProjectDirectory`로 교체
   - `static_project_directory.rs:5-6` 주석을 "Test fixture only — production path uses `KeystoneProjectDirectory` (BL-P2-080)"로 갱신

7. **CI integration test (release gate)**:
   - devstack docker image 기동 (기존 `reauth` gate와 같은 chain에 추가 가능 — BL-P2-081 CI와 공유)
   - 실제 `/v3/auth/projects` 호출 + pagination 강제 (`?limit=1`) + real UUID로 rescope 성공 (201 Created)
   - 이게 없으면 regression이 로컬 실증 단계까지 안 잡힘 (Codex v3 M1 교훈 적용)

8. **코드 레벨 주석 정리**:
   - `static_project_directory.rs:5-6`, `:45-48` placeholder 주석 갱신
   - `resolver.rs:110` ById 경로의 "BL-P2-052" 참조 제거 (이 BL로 해결됨)

**Acceptance 요약**:
- `:switch-project admin` 포함 same-cloud 전환이 devstack에서 end-to-end 녹색
- Pagination/TTL/invalidation/concurrency 각 유닛 테스트 + devstack integration test green
- CI에서 devstack 컨테이너 기반 real `/v3/auth/projects` 호출 gate 추가
- `StaticProjectDirectory`는 test-only로 강등, production path는 `KeystoneProjectDirectory`
- BL-P2-081과 공유하는 CI infra 재사용

**Out of scope 확인 (→ BL-P2-081이 처리)**:
- Cross-cloud directory 조회 (target cloud token 필요)
- `KeystoneRescopeAdapter` 단일 `wire_auth_url` 바인딩 제거
- per-cloud auth strategy factory

**Ref**: 2026-04-21 systematic-debugging (Test A/B), Codex adversarial-review 3라운드 — v1 finding M2 (P0 scope 확장, pagination/scoping/invalidation/concurrency 요구), v3 finding M1 (CI release gate 요구).

### BL-P2-081: Trait-based cross-cloud auth — explicit declaration + fail-fast (v3.1) — P1
**Priority**: High (`:switch-cloud` 실동작 조건)
**Category**: Auth / Context Switch

**CI devstack gate activation checklist (BL-P2-080 Unit 3 R1 review 2026-04-23)**:
BL-P2-080 Unit 3(`.github/workflows/ci.yml::devstack-integration`)은 placeholder digest + `if: false` 상태로 merge됨. BL-P2-081 PR 또는 이후 activation PR에서 `if: false` 제거 시 **반드시 함께 적용**:
- [ ] `opendevstack/devstack:placeholder` → 실제 image digest (`sha256:...`)로 pin
- [ ] `DEVSTACK_TOKEN`을 `echo "::add-mask::$DEVSTACK_TOKEN"`으로 GHA masker 등록 (`--nocapture` + Debug 출력에서 token leak 방지)
- [ ] Third-party actions SHA digest pin: `dtolnay/rust-toolchain`, `Swatinem/rust-cache`, `rustsec/audit-check`, `actions/upload-artifact` (supply chain 방어)
- [ ] `docker logs devstack` artifact에 sanitize 파이프라인 추가: `grep -Ev '(token|password|secret|fernet)'`
- [ ] Job-level `timeout-minutes: 15` 명시
- [ ] `if: env.DEVSTACK_TOKEN != ''` guard 추가 (silent skip 방지)
- [ ] README/PR body에 "Unit 3 CI gate 활성화 완료" 명시


**Description**: 2026-04-21 systematic-debugging + Codex 3라운드 adversarial-review 결과. BL-P2-074 PR #78로 `:switch-cloud` UX/state는 wire 되었으나 실환경에서 **rescope가 cross-cloud 불가** (Test D = 404 Fernet recognition, Test E = target cloud password auth = 201). 단 환경 일반화는 확증 불가 — shared Fernet key repository / K2K federation에서는 rescope가 작동. 운영 배포는 shared-key / isolated / 회사별 custom auth(KT Cloud SSO, OIDC 게이트웨이 등)가 모두 쓰일 가능성. **단일 전략 lock-in 금지 + 확장 가능 구조 + 운영자 명시 선언**이 설계 원칙.

**Design (v3.1 — Codex 3라운드 확정, 이 섹션이 유일한 canonical source)**:

**원칙 (단순화)**:
- **명시 선언**: `:switch-cloud` 사용 시 `cross_cloud_mode` **required**. 미선언 → `SwitchError::NotConfigured { cloud, reason: "cross_cloud_mode not declared" }`
- **Fail-fast**: 선언된 전략 실패 시 폴백 없이 현재 mode + 가이드 포함 명시적 에러 반환
- **학습 cache / probe 없음**: 런타임 전략 변경 없음, 의도적 401/404 유발 없음
- 운영자 선언 = 배포 토폴로지 책임 표명. 잘못된 선언은 명시적 에러로 드러남

**원칙 (확장성 — Port/Adapter 패턴 정합)**:
- `CrossCloudAuthStrategy` **trait** + `CrossCloudAuthRegistry` (`HashMap<String, Arc<dyn CrossCloudAuthStrategy>>`)
- Builtin Phase 1: `TokenRescopeStrategy(name="rescope")`, `PasswordReauthStrategy(name="reauth")`
- 향후 확장: `KtCloudSsoStrategy`, `OidcStrategy`, `CustomCorpTokenStrategy` 등 — 소스 수정 없이 registry에 등록
- `clouds.yaml` 신규 필드:
  - `cross_cloud_mode: string` (required for `:switch-cloud`, registry key)
  - `cross_cloud_config: serde_yaml::Value` (optional free-form slot — Phase 1 미사용, 향후 strategy별 추가 파라미터 담기용)
- Registry lookup 실패 → `SwitchError::NotConfigured { cloud, reason: "unknown cross_cloud_mode: X (registered: rescope, reauth)" }`

**원칙 (감사성 — Codex v3 finding H2 대응)**:
- Startup 시 `tracing::info!(registered = [...], "cross_cloud_auth registry bootstrapped")`
- Cloud별 mode resolve 시 `tracing::info!(cloud = X, mode = Y, strategy = Z, "cross_cloud_mode resolved")`
- Config load 시 미등록 mode는 warn으로 조기 감지 (switch 시 hard fail은 기본)
- Introspection: 간단한 debug command 또는 TUI view — 운영자가 "현재 어떤 strategy가 어느 cloud에 활성인지" 확인 가능 (registry.list() + cloud별 resolve 결과)

**원칙 (검증 — Codex v3 finding M1 대응, 옵션 β)**:
- `reauth` validation: **CI integration test release gate** — devstack docker image 기동 후 target cloud password auth → rescoped token 획득 → rescope body로 rescope adapter 검증
- `rescope` validation: Phase 1은 **로컬 수동 실증만 요구**하되 backlog에 `BL-P2-084`(shared-Fernet CI fixture + rescope production promotion)을 follow-up BL로 등록. `rescope` 선언 허용하되 backlog에 "CI gate 미확보" 명시.

**Phase 1 필요 작업**:
1. `CrossCloudAuthStrategy` trait + `CrossCloudAuthRegistry` 구현
2. `KeystoneRescopeAdapter` factory화 — startup `wire_auth_url` 고정 바인딩 제거, target cloud URL로 호출
3. 신규 `PasswordReauthAdapter` — target cloud credentials로 password auth (`clouds.yaml`에서 해당 cloud의 `auth.username/password/user_domain_name` 사용)
4. `ContextSessionPort::rescope` 구현이 registry lookup → `strategy.authenticate(target, ctx)` 호출
5. `clouds.yaml` 파싱 확장 (`cross_cloud_mode`, `cross_cloud_config`)
6. Startup/resolve 로그 + introspection view
7. CI integration test: `reauth` 경로 (devstack docker), MockStrategy dispatcher 유닛 테스트
8. `rescope` 로컬 수동 실증 (BL-P2-084 완료 전까지는 experimental 성격)
9. credentials 없는 cloud에 `reauth` 요청 시 명시적 `SwitchError::NotConfigured` (기존 variant 재활용)

**Acceptance**:
- `:switch-cloud <target>` 미선언 시 명시적 NotConfigured, 선언 + registry 미등록 시 명시적 NotConfigured, 선언된 strategy 실행 성공 시 실제 전환 완료
- Startup 로그에 registered strategies + cloud별 resolve 결과 모두 기록
- `reauth` CI integration test green (release gate)
- `rescope` 로컬 실증 결과 문서화 (PR 본문 또는 `docs/`)
- `backlog.md`의 BL-P2-081 이 canonical source로서 코드와 일치 (spec drift 방지)

**Ref**: 2026-04-21 systematic-debugging (Test A/B/D/E), Codex adversarial-review 3라운드 findings (v1 H1/M1/M2, v2 H1/H2/M1, v3 H1/H2/M1). 이전 설계안(`auto` 폴백 + 학습 cache)은 v3.1에서 **deprecated/제거** — 해당 모델을 참조하는 PR 금지.

### BL-P2-083: Interim token-expiry guard (P0/P1 ship 전 필수)
**Priority**: High (P0+P1 머지 전 블로커)
**Category**: Auth / Safety
**Description**: 2026-04-21 Codex adversarial-review finding M1. BL-P2-052 Part A(본격 auto refresh)는 설계/구현 규모가 있어 P0/P1보다 뒤에 처리하지만, P0/P1 머지 순간부터 사용자 세션은 55분 뒤 deterministic auth 실패를 맞음. interim guard 없이 ship하면 장애가 switch 로직으로 오귀인될 위험. interim guard로 "세션 만료" 상태를 명확히 구분하고 telemetry를 깔아 BL-P2-052 Part A의 근거 데이터 확보.

필요 작업 (최소):
1. `get_token` 경로에서 token `expires_at`이 now + margin(예: 5분) 안쪽이면 명시적 `SwitchError::SessionExpired { cloud, project }` 또는 `ApiError::SessionExpired` (BL-P2-053 NotAuthenticated variant 신설 논의와 정합화) 반환.
2. UI toast 안내문: "세션이 만료되었습니다. `:switch-context`로 재전환하거나 앱을 다시 시작하세요." (safe_display 적용 — BL-P2-050 완료 후 결합 가능).
3. Telemetry: tracing `warn!(expired_at, cloud, project, "session_expired")` 로 기록 — BL-P2-052 Part A의 "얼마나 자주 발생하는가" 근거 데이터.
4. 테스트: expired token fixture → get_token 호출 → 명시적 SessionExpired 반환, `ApiError::AuthFailed`로 오인되지 않음.

**Ref**: 2026-04-21 Codex adversarial-review M1.

### BL-P2-084: Shared-Fernet CI fixture + `rescope` production promotion — Follow-up
**Priority**: Medium (BL-P2-081 후속)
**Category**: Auth / CI infrastructure
**Description**: BL-P2-081 v3.1의 `rescope` strategy는 Phase 1에서 로컬 수동 실증만 요구하고 CI gate 미확보 상태로 ship됨 (Codex v3 finding M1 타협안 β). 운영자 선언 기반이라 잘못된 선언은 fail-fast로 드러나지만, 공유 fixture가 없으면 regression 감지 어려움. 이 BL에서 shared-Fernet 토폴로지의 CI fixture를 구성하고 `rescope` 경로를 release gate로 승격.

필요 작업:
1. **docker-compose fixture**: 두 Keystone 인스턴스 + 공유 `/etc/keystone/fernet-keys` 볼륨. 각각 다른 endpoint URL, 같은 fernet key repository.
2. **Integration test**: 첫 Keystone에서 token 발급 → 두 번째 Keystone에 rescope (token-method) → 201 Created 검증. devstack image 재사용 가능한지 평가.
3. **CI 워크플로우 추가**: `reauth` 경로(BL-P2-081에서 도입)와 동일 체인에 `rescope` integration test 추가.
4. **Backlog 정리**: BL-P2-081의 "rescope CI gate 미확보" 주석 제거, production promotion 반영.
5. **Docs**: `docs/dev-topologies.md` (또는 유사)에 shared-Fernet 구성 방법 문서화 — 로컬 기여자가 재현 가능하도록.

**Acceptance**:
- CI에서 shared-Fernet 토폴로지 기동 + rescope 경로 integration test green
- BL-P2-081의 v3.1 canonical section에서 rescope가 production-ready로 갱신
- `docs/dev-topologies.md` (또는 유사)에 재현 절차 명시

**Ref**: 2026-04-21 Codex adversarial-review v3 finding M1, BL-P2-081 옵션 β 타협안 follow-up.

### BL-P2-089: Glance `UpdateImage` Action + FR4 cross-project guard (BL-P2-085 follow-up)
**Priority**: Medium
**Parent**: BL-P2-085 Step 16 (descope)
**Category**: Security / Functional

**Description**: BL-P2-085 Step 16에서 `Action::DeleteImage` 경로에는 FR4 pre-mutation 가드 (Glance `get_image` → `check_image_owner_scope` → `Fr4Form` audit + reject)를 적용했으나, image **update** path는 `Action::UpdateImage` variant 자체가 미존재해 가드를 적용할 곳이 없었음. Glance `update_image` adapter는 이미 존재하지만 worker가 호출하지 않음.

본 BL에서:
1. `Action::UpdateImage { id, params: ImageUpdateParams }` variant 신규 (`src/action.rs`)
2. `module/image/mod.rs`에 update 키 핸들러 (`PendingAction::UpdateImage` 또는 직접 form submit)
3. `worker.rs::action_to_kind`에 `Action::UpdateImage => ActionKind::Update` 분류 + `action_name` 매핑 추가
4. `worker.rs::handle_action(Action::UpdateImage)` 분기 — `DeleteImage`와 동일 패턴:
   - pre-mutation `glance.get_image(&id).await`
   - `check_image_owner_scope(&image, active)` 호출 (Step 16 pure helper 재사용)
   - Block이면 `emit_form_block_audit(reason, "UpdateImage", "image", &id, ...)` + `AppEvent::CrossProjectBlocked { reason, action: "UpdateImage" }`
   - 진행 시 `glance.update_image(&id, &params).await`
5. 3 신규 tests (worker.rs::tests):
   - `test_update_image_rejects_cross_project_owner`
   - `test_update_image_allows_same_project_owner`
   - `test_update_image_emits_fr4_form_event` — `emit_form_block_audit`로 "UpdateImage" action_type stamp 확인

**Out of scope**: image visibility/min_disk/min_ram 등 update field 전체 surface — 본 BL은 FR4 가드 적용에 집중. Form UI 추가/변경은 별도 cycle.

**Ref**: BL-P2-085 Step 16 commit `e91cca1` ("Plan adjustment: descoped `UpdateImage` to a follow-up BL because no `Action::UpdateImage` variant exists today").

### BL-P2-090: `MockGlanceWithImage` configurable mock for `handle_action` FR4 integration test (BL-P2-085 follow-up)
**Priority**: Low
**Parent**: BL-P2-085 Step 16 (test gap)
**Category**: Testing Infrastructure

**Description**: BL-P2-085 Step 16의 pure helper (`check_image_owner_scope` / `emit_form_block_audit`) 5 tests로 결정 매트릭스는 cover됐으나, `handle_action(Action::DeleteImage)` **통합 경로** 자체는 단위 테스트로 검증되지 않음. 원인: 기본 `MockGlanceAdapter::get_image`가 `Err(ApiError::NotFound)`만 반환하므로 cross-project owner 시나리오를 `handle_action`에 끝까지 흘려보낼 수 없음.

본 BL에서:
1. `MockGlanceAdapter`에 configurable behavior 추가 — 다음 중 1택:
   - Option (a): `MockGlanceAdapter::with_image(Image)` builder + 내부 `Mutex<Option<Image>>`로 get_image 반환 제어
   - Option (b): 별 mock struct `MockGlanceWithImage { owner: Option<String>, delete_called: AtomicBool }`로 분리
   - Option (c): 더 일반화된 `MockAdapter<T>` 패턴 도입 (다른 adapter mock에도 확장 가능)
2. `worker.rs::tests`에 통합 test 추가:
   - `test_handle_action_delete_image_emits_cross_project_blocked_when_owner_mismatch` — AdapterRegistry(with mock) + active=A → `handle_action` returns `AppEvent::CrossProjectBlocked`, mock이 `delete_image`를 호출받지 않았는지 확인 (Atomic flag)
   - `test_handle_action_delete_image_proceeds_when_owner_matches` — owner=A + active=A → audit log empty, `delete_image` 호출됨
   - `test_handle_action_delete_image_falls_through_on_pre_get_error` — pre-GET이 NotFound 등이면 기존 delete 흐름 (현재 동작 보존)

**Out of scope**: 다른 adapter mock 일반화 (별도 BL). Option (c)를 선택할 경우 다른 adapter에도 적용 여부는 follow-up.

**Ref**: BL-P2-085 Step 16 commit `e91cca1` ("Pure helpers cover the decision matrix; `handle_action` integration is not unit-tested directly because `MockGlanceAdapter::get_image` returns `NotFound`").

### BL-P2-091: Glance FR1 — `ScopedItem for Image` + `GlanceHttpAdapter::with_audit` (BL-P2-085 follow-up)
**Priority**: Medium
**Parent**: BL-P2-085 cargo-review branch-full Correctness #1 / Suggestions #4
**Category**: Security / Functional

**Description**: BL-P2-085가 Neutron/Nova/Cinder 3개 list adapter에 FR1 (`refilter_response` + `AdapterFilterViolation` audit emit)을 wire했으나, GlanceHttpAdapter는 **FR1 미적용**. 결과적으로 `FetchImages`가 admin 토큰 등에서 cross-project image를 list UI에 노출할 수 있고, 차단은 FR4 (DeleteImage pre-mutation) 경로에서만 발생. atomic security PR의 "FR1+FR2+FR3+FR4 layered defense" 약속과 비대칭.

본 BL에서:
1. `src/adapter/http/scope_refilter.rs`에 `impl ScopedItem for Image` 추가 — `Image.owner: Option<String>` → `tenant_id()`, `Image.id: String` → `resource_id()`
2. `src/adapter/http/glance.rs`:
   - `audit_ctx: Option<Arc<GlanceAuditCtx>>` 필드 + `with_audit(ctx)` builder (Nova/Cinder/Neutron 패턴 mirror)
   - `refilter_response<T: ScopedItem>` helper
   - `list_images` 본체에서 `refilter_response(resp, filter.all_tenants, "FetchImages", "image")` 호출
3. `src/adapter/http/neutron_audit.rs`에 `pub type GlanceAuditCtx = AuditCtx;` + `AdapterAuditConfig.glance: Option<Arc<GlanceAuditCtx>>` 필드 추가 + `build_audit_config`에서 `glance` 채우기
4. `src/adapter/registry.rs::new_http` body에 `audit.glance` 소비 (`glance.with_audit(ctx)`)
5. 신규 tests: `test_image_has_scoped_item_returns_some_when_present` / `_returns_none_when_absent` / `_build_audit_config_returns_glance_with_service_glance` / `test_glance_with_audit_attaches_ctx_default_none`

**Out of scope**: Glance image visibility 의미론 (`public`/`private`/`shared`/`community`)의 정밀한 처리 — `owner` 비교만으로 부족할 수 있는 케이스는 별도 BL.

**Ref**: cargo-review branch-full report 2026-05-12 (Correctness #1, Suggestions #4).

### BL-P2-092: Cross-resource pre-mutation FR4 (FIP/port, Volume/server project mismatch) (BL-P2-085 follow-up)
**Priority**: Medium
**Parent**: BL-P2-085 cargo-review branch-full Suggestions #5
**Category**: Security / Functional

**Description**: BL-P2-085 FR4 (`check_image_owner_scope` / `validate_form_scope`)는 단일 resource의 owner-vs-active 비교만 처리. 그러나 cross-resource mutation은 두 resource가 서로 다른 project에 속할 가능성에 대한 pre-mutation check가 부재:
- `AssociateFloatingIp { fip_id, port_id }` — fip가 proj-A, port가 proj-B 소속일 수 있음
- `DisassociateFloatingIp { fip_id }` — fip가 active scope와 다른 project일 수 있음
- `AttachVolume { volume_id, server_id }` — volume과 server가 다른 project일 수 있음
- `DetachVolume` / `ForceDetachVolume` — 동일
- `LiveMigrateServer` / `Evacuate` 등 — destination host가 다른 project AZ일 수 있음 (별 BL 영역일 수도)

현재는 OpenStack 서버측 RBAC에 의존 — admin 토큰이면 통과 가능. atomic security의 client-side defense-in-depth가 비대칭.

본 BL에서:
1. `Action::AssociateFloatingIp` 분기에 pre-mutation GET 두 번 (fip + port) → 각자 project_id 추출 → active_tenant와 비교 → 첫 mismatch면 `Fr4Form` audit emit + `AppEvent::CrossProjectBlocked`
2. `Action::AttachVolume` / `DetachVolume` / `ForceDetachVolume` 동일 패턴 (volume + server)
3. `Action::DisassociateFloatingIp` — fip만 비교
4. helper `check_pair_scope(left_project, right_project, active)` pure fn 도입 (이번 BL의 단일 resource helper 일반화)
5. 5+ tests — 각 action별 cross-project deny + same-project allow + missing project_id fail-safe

**Out of scope**: Live-migrate destination host AZ scope (BL-P2-086 직후 영역). Phase 9 plan에는 FIP/Volume만 있었음.

**Ref**: cargo-review branch-full report 2026-05-12 (Suggestions #5).

### BL-P2-093: `actor_ctx.user_id` cloud-switch live sync (BL-P2-085 follow-up)
**Priority**: High
**Parent**: BL-P2-085 cargo-review branch-full Suggestions #1
**Category**: Security / Audit Attribution

**Description**: BL-P2-085 Phase 7 폴리싱에서 `actor_ctx.cloud`는 `App::handle_event(ContextChanged)`에서 live update 되도록 wire됐으나, `actor_ctx.user_id`는 **wire-startup 시점의 `wire_username`으로 고정**. 즉 사용자가 cloud-A → cloud-B로 switch한 뒤 다른 자격증명으로 재인증해도, 그 후 발생하는 모든 cross-project block audit entry는 **cloud-A의 user_id를 사용**한다.

영향:
1. Audit attribution 손상 — multi-cloud 환경에서 누가 block을 trigger했는지 잘못 기록
2. **fingerprint v1 dedup 깨짐** — canonical `"v1|user|active|origin|target|action|resource_id"`에 user가 포함됨 → 동일 user가 두 cloud에서 동일 cross-project pattern 시도 시 fingerprint 다르게 생성 → dedup 미발동

본 BL에서:
1. `ContextTarget` 또는 `ContextChanged` event payload에 새 토큰의 `user.id` 포함 (Keystone token의 user UUID 사용)
2. `App::handle_event(ContextChanged)`에서 `actor_ctx.write().user_id = new_user_id`로 갱신
3. `KeystoneAuthAdapter::get_token_info()`이 user_id를 노출하는지 확인 + wire
4. 신규 test: `test_actor_ctx_user_id_updates_on_cloud_switch` (RwLock mutate → 다음 emit이 새 user_id 반영) — Phase 7 폴리싱의 `test_emit_origin_block_audit_picks_up_actor_context_mutation` 패턴 mirror

**Out of scope**: token refresh 중 user_id 변경 hook (BL-P2-052의 token refresh 작업과 결합). Username/UUID 매핑 (현재는 UUID 우선).

**Ref**: cargo-review branch-full report 2026-05-12 (Suggestions #1).

### BL-P2-094: Fingerprint v2 schema — `|` escape + length-prefix (BL-P2-085 follow-up)
**Priority**: Low (v2 cycle 시 필수)
**Parent**: BL-P2-085 cargo-review branch-full Suggestions #2
**Category**: Schema Hardening

**Description**: BL-P2-085 fingerprint v1 canonical은 `"v1|user|active|origin|target|action|resource_id"` 형태로 field 값을 unescape된 채 `|`로 join. 어느 하나가 `|`를 포함하면 collision 가능:
- `(user="a|b", active="c")` ↔ `(user="a", active="b|c")` 동일 fingerprint
- Keystone user_id는 UUID라 안전하지만 `action_type` (예: "Action|with|pipe"), `resource_id` (사용자 정의 가능한 경우) 등 customizable fields는 위험

v1 schema는 LOCKED라 즉시 변경 X. **다음 schema bump 시점에 반드시 escape rule을 같이 도입**.

본 BL에서:
1. fingerprint v2 canonical 설계:
   - 옵션 (a): `\|` / `\\` escape rule + version prefix `"v2|escaped(user)|escaped(active)|..."`
   - 옵션 (b): length-prefix encoding `"v2|3:foo|5:bar|..."` (collision 불가, simple)
   - 옵션 (c): JSON canonicalization (serde_json with `sort_keys`) — 가독성 ↓이지만 표준
2. v1 → v2 migration: 기존 v1 entry 보존 (rotation 이전 데이터). 새 entry는 v2.
3. `CrossProjectBlockEvent::fingerprint` 갱신 + `tests/cross_project_audit.rs` schema-stable tests 갱신
4. v1 LOCKED 약속 해제 — state.md "Schema-Stable 결정" 갱신
5. **호환성**: audit consumer (grep/jq script 등)는 v1/v2 둘 다 지원하도록 release notes

**Out of scope**: hex 변환 최적화 (cargo-review Suggestions #6, byte-level lookup) — 본 BL과 함께 진행하면 자연.

**Trigger**: 새 schema field 추가 (예: `service` from refactor-3 / `correlation_id` epoch propagation) 또는 v1 LOCKED 해제 결정 시.

**Ref**: cargo-review branch-full report 2026-05-12 (Suggestions #2).

### BL-P2-052: Rescoped 토큰 자동 refresh + ContextChanged handler
**Priority**: High (Part A 기준) — **BL-P2-080 / BL-P2-081 / BL-P2-083 이후**
**Category**: Auth / Functional Regression
**Description**: BL-P2-031 Unit 3b T2 review S2 + PR1 cargo-review integration finding.

**주의 (2026-04-21)**: 이 BL의 description은 **토큰 refresh + ContextChanged UX에만 해당**. `static_project_directory.rs:5-6` 주석이 암시하는 "HTTP-based ProjectDirectory 교체"는 이 BL의 scope에 없음 — `BL-P2-080`이 담당. 해당 주석은 BL-P2-080 구현 시 갱신.

**Part A — Rescoped 토큰 자동 refresh**: C1 가드 도입으로 KeystoneAuthAdapter는 initial scope 토큰만 자동 refresh. set_active(demo) 후 demo 토큰이 expire되면 `get_token`이 영구 실패 (`refresh_token` 가드가 AuthFailed 반환). 사용자 영향: ~55분 후 demo 세션이 모든 API 호출 실패. `BL-P2-083` interim guard가 먼저 ship되어 장애 명시성을 확보한 후, 이 Part A가 완결판(백그라운드 auto refresh)으로 대체.
필요 작업: ScopedAuthSession 또는 신규 RescopeRefresher가 active scope 토큰의 near-expiry를 감지 → KeystoneRescopePort로 새 토큰 발급 → set_active로 갱신. 또는 최소한 get_token 에러 메시지를 "session expired, please switch context again"으로 명확화.

**Part B — `AppEvent::ContextChanged` handler 구현** (PR3 진행 중 대폭 처리, 잔여만 여기 남음):
- ✅ `App.handle_event::ContextChanged` arm (ContextIndicator 갱신) — PR3 Step 3 (f138af6)
- ✅ 16개 Resource Module 캐시 invalidate (Vec 비움 + is_loading=true) — PR3 Codex HIGH #1 대응 (A+Fetch commit, 예정)
- ✅ `Fetch*` 일괄 dispatch — PR3 Codex HIGH #1 대응 (A+Fetch commit, 예정)
- ⬜ **router reset / selection reset** (필요 시) — 잔여
- ⬜ **"Switched to project X" toast** — 잔여
- ⬜ `on_context_changed(&target)` 메서드 추출 (거대 match 분할) — 잔여. Step 2/3 cargo-review Suggestions Future Work

**Part C — ContextChanged channel round-trip 통합 테스트** (BL-P2-077 G6에서 이관, 2026-04-18):
- 현재 `test_context_changed_updates_indicator`는 `app.handle_event(...)` 직접 호출 — epoch gate 우회
- 프로덕션 경로: `event_tx.send → drain → handle_versioned_event (epoch gate) → handle_event`
- 통과 케이스(current epoch) + drop 케이스(stale epoch +1) 양쪽 검증 추가
- Part B 수정과 같은 diff에서 처리해야 stale drop 경로까지 회귀 감지

**Acceptance**:
- Part A: token expire 자동 처리.
- Part B: switch 성공 → router/selection reset + 사용자 toast. (Vec clear / Fetch* / indicator는 PR3에서 완료)
- Part C: channel round-trip epoch gate 테스트 통과.

**Priority**: High (Part A 기준). Part B/C는 UX 완결성 + 테스트 부채로 Medium 레벨.

**Ref**: Security Reviewer S2 (P0 fix 리뷰), Cargo Review PR #68 통합 finding. PR3 Codex adversarial HIGH #1 대응으로 Part B의 안전 부분은 선처리됨. Part C는 BL-P2-077 G6(2026-04-17 등록)에서 이관.

### BL-P2-053: SwitchError NotAuthenticated variant + ApiError ScopeDrift variant
**Priority**: Medium
**Category**: Error model / Caller classification
**Description**: BL-P2-031 Unit 3b T2 review I1+I2. 현재 ScopedAuthSession::begin이 pre-auth 상태를 `SwitchError::Unsupported`로 매핑 (의미: 기능 미지원). 또한 KeystoneAuthAdapter의 C1 scope drift 가드가 `ApiError::AuthFailed` 사용 (의미: credential 거부 → 사용자가 reauth 시도하지만 같은 에러 반복).
필요 작업: `SwitchError::NotAuthenticated`(또는 `Precondition`), `ApiError::ScopeDrift`(또는 `InvalidState`) variant 추가 → caller가 분기 가능. SwitchError는 #[non_exhaustive] 없음 (외부 매처 영향 검토 필요), ApiError는 #[non_exhaustive] (안전).
**Ref**: Quality Reviewer I1+I2 (P0 fix 리뷰)

### BL-P2-054: KeystoneAuthAdapter Drop::abort + refresh task lifecycle
**Priority**: Medium
**Category**: Resource leak
**Description**: BL-P2-031 Unit 3b T2 Codex review. start_refresh_loop이 spawn한 tokio task가 self.credential/scope_ref/token_map의 strong Arc를 보유. adapter drop 시 JoinHandle abort 호출 없으므로 백그라운드 task가 계속 인증 시도. 프로세스 수명 내내 누적 가능.
필요 작업: KeystoneAuthAdapter::Drop 구현 → refresh_handle abort. 또는 CancellationToken 도입.
**Ref**: Codex P0 review

### BL-P2-055: Refresh loop 백오프 + 로그 rate-limit
**Priority**: Low
**Category**: Observability
**Description**: BL-P2-031 Unit 3b T2 review S3. C1 가드 도입 후 active scope drift 시 refresh loop이 sleep_duration 10s로 떨어지고 매 tick warn 로그 발행. demo 세션 expiry 후 분당 6회 누적.
필요 작업: scope drift 감지 시 최소 60s sleep 강제 또는 break 후 set_active 재발생까지 대기. 로그도 최초 1회 또는 N tick마다 1회.
**Ref**: Security Reviewer S3 (P0 fix 리뷰)

### BL-P2-056: TokenScope 정규화 일관화
**Priority**: Medium
**Category**: Auth / Correctness
**Description**: BL-P2-031 Unit 3b T2 Codex review. `TokenScope::from_credential`은 name/domain을 `to_lowercase()` 적용, `From<&ContextTarget> for TokenScope`는 원문 보존. 동일 프로젝트가 케이스 차이로 다른 키로 분리되어 토큰 캐시 miss 발생 가능.
필요 작업: 정규화 정책을 단일 위치에 통합 (TokenScope::Project 생성 시 항상 lowercase). resolver/parser/cache 경로 모두 검증.
**Ref**: Codex P0 review

### BL-P2-057: ScopedAuthPort/AuthProvider 동시성 race 테스트 (loom)
**Priority**: Low
**Category**: Test coverage
**Description**: BL-P2-031 Unit 3b T2 review M6. set_active ↔ refresh_token / authenticate 동시 호출 시 invariant (token_map ↔ active_scope 정합) 검증 부재. 현재 락 순서로 race-free하지만 회귀 방지를 위한 명시적 테스트 필요.
필요 작업: loom 또는 직접 thread spawn 기반 동시성 테스트.
**Ref**: Quality Reviewer M6 (P0 fix 리뷰)

### BL-P2-058: AuthCredential Zeroize 도입
**Priority**: Medium
**Category**: Security / Credential hygiene
**Description**: BL-P2-031 Unit 3b T2 Security I3. AuthCredential의 password가 refresh loop으로 클론되어 프로세스 수명 내내 heap 체류. Drop 시 zeroize 없음 → 메모리 덤프/core dump에 평문 password 노출 window.
필요 작업: zeroize crate 추가 → AuthCredential에 ZeroizeOnDrop. application_credential 우선 사용 권장 정책.
**Ref**: Security Reviewer I3

### BL-P2-059: Poison fail-closed 정책 전환 (auth 경로 한정)
**Priority**: Low
**Category**: Security / Defense-in-depth
**Description**: BL-P2-031 Unit 3b T2 Security I2. KeystoneAuthAdapter의 모든 락에서 `unwrap_or_else(|e| e.into_inner())` 사용 → poison 무시. 토큰 같은 security-critical 데이터에 대해 fail-secure 원칙과 충돌. 실제 panic 가능성은 낮으나 OWASP 권고와 대비.
필요 작업: auth 경로 한정으로 poison 시 인증 무효화 + 강제 재인증 트리거. 또는 각 사이트에 "왜 안전한가" 주석 추가.
**Ref**: Security Reviewer I2

### BL-P2-060: Action channel `Result<(), SendError<VersionedEvent<Action>>>` boxing
**Priority**: Low
**Category**: Performance / Code size
**Description**: PR1 cargo-review clippy `result_large_err`. `src/context/action_channel.rs:81`의 `pub fn send(&self, action: Action) -> Result<(), SendError<VersionedEvent<Action>>>`에서 Err variant가 176 bytes. send() 콜사이트가 수백 곳 (16개 모듈 전반)이라 모든 Result가 stack에 176-byte 슬롯 점유.
필요 작업: `Box<SendError<VersionedEvent<Action>>>`로 감싸거나 Action enum 자체를 Box화. bench로 실제 영향 (instruction cache miss, frame size) 측정 후 결정.
**Ref**: Cargo Review PR #68 clippy

### BL-P2-061: `SwitchStateView::Switching` large_enum_variant
**Priority**: Low
**Category**: Performance / Code size
**Description**: PR1 cargo-review clippy `large_enum_variant`. `src/context/state_machine.rs:55`의 `SwitchStateView::Switching` variant가 `ContextTarget`을 직접 보유 (적어도 352 bytes 큰 variant). state machine은 sync 코드라 핫 경로일 가능성.
필요 작업: `Switching { target: Box<ContextTarget>, ... }`로 변경 검토. clone 경로 전반에 영향. bench로 실제 영향 측정 후 결정.
**Ref**: Cargo Review PR #68 clippy

### BL-P2-063: Pre-existing clippy `-D warnings` 35건 일괄 정리 + CI 게이트
**Priority**: Medium
**Category**: Code quality / Tech debt
**Description**: PR1 cargo-review에서 `cargo clippy --lib --tests -- -D warnings` 실행 시 PR1 무관 pre-existing 35건 위반 발견 (PR #68 머지 후 재측정). 유형:
- `clippy::map_or` simplification (다수)
- `clippy::collapsible_if` / `collapsible_match`
- `clippy::doc_lazy_continuation` (doc 렌더링 깨짐)
- `clippy::manual_map`
- `clippy::result_large_err` — BL-P2-060 중복 영역, item-level `#[allow]` + reason으로 deferred
- `clippy::large_enum_variant` — BL-P2-061 중복 영역, item-level `#[allow]` + reason으로 deferred

기타 sed/scaffold 시점부터 누적된 idiom 위반.

**Canonical gate command** (local + CI 일치): `cargo clippy --lib --tests -- -D warnings`

**Suppression 정책**: `#[allow(clippy::<lint>)]`은 **item-level만 허용** (module-level 금지). 각 allow 사이트는 `reason = "..."` + BL ID 필수 명시. 시간 제한은 추적 BL의 acceptance 달성 시점.

**작업 스코프**:
1. `rust-toolchain.toml` 생성 — `stable` 채널 + clippy/rustfmt 컴포넌트 핀 (로컬/CI 일관성)
2. `cargo clippy --fix --lib --tests --allow-dirty` 자동 수정 → diff 검토 (clean 상태에서만)
3. 수동 수정 잔여 항목 처리. `result_large_err` / `large_enum_variant`는 BL-P2-060/061 item-level `#[allow]`로 위임
4. 잔여 위반 없이 canonical gate command 통과 확인
5. `.github/workflows/ci.yml` 신규 — 4-stage gate: fmt check / lib tests / clippy(--lib --tests) / bin compile
6. `Cargo.toml`의 `[lints.clippy]`에 추가 deny할 lint 검토 (현재 unwrap_used/expect_used/enum_glob_use만 deny)

**Acceptance**: `cargo clippy --lib --tests -- -D warnings` 0 errors. CI에서 동일 명령 실행 + 실패 시 머지 차단. `cargo build --bin nexttui` 통과. `cargo fmt --all -- --check` 통과.

**예상 작업량**: 1세션 (1~2시간). 자동 수정으로 70~80% 처리 예상.

**Ref**: Cargo Review PR #68 — "기존 clippy 위반 38개 — pre-existing, 본 PR 무관" 후속 처리. Codex planning review — CONDITIONAL APPROVE (R6: command scope 통일, R7: bin compile gate, R9: item-level allow 제약, R10: CI component pin)

### BL-P2-062: Stale action drop E2E 통합 테스트
**Priority**: Low
**Category**: Test coverage
**Description**: PR1 cargo-review missing test. switch 도중 큐에 쌓인 old-epoch action (e.g., FetchServers) → worker가 처리 → response event가 dispatcher epoch 게이트에서 drop되는 경로를 E2E로 검증하는 테스트가 없음. unit-level은 spawn_versioned/dispatcher 각자 검증되지만 통합 시나리오는 미커버.
필요 작업: `app.rs::tests`에 통합 테스트 추가 — (1) action 큐에 스테이지, (2) try_begin → epoch bump, (3) worker가 큐에서 꺼내 응답, (4) dispatcher가 stale event drop 확인.
**Ref**: Cargo Review PR #68 missing test

### BL-P2-065: Rust toolchain 정확한 버전 핀
**Priority**: Medium
**Category**: Reproducibility / CI
**Description**: 현재 `rust-toolchain.toml`이 `channel = "stable"` — floating pin. 매 stable 릴리스마다 clippy lint 셋/behavior 변경 가능 → "어제 green, 오늘 red" 가능성.
필요 작업:
1. `channel = "1.94.0"` 같은 정확한 버전 핀
2. Dependabot 또는 수동 주기(월 1회) bump 정책 문서화
3. MSRV (Minimum Supported Rust Version)를 `Cargo.toml`의 `rust-version` 필드에도 명시
4. Rust edition 2024는 1.85.0+ 필요 — 핀 버전이 호환되는지 검증

**Acceptance**: CI는 정확한 버전으로 실행. bump는 의도적 PR로만 발생.
**의존**: BL-P2-063 완료 (rust-toolchain.toml 자체가 PR #70에서 도입됨)
**Ref**: BL-P2-063 PR #70 cargo-review

### BL-P2-066: `.git-blame-ignore-revs` 도입 + AI 협업 blame hygiene 운영
**Priority**: Medium
**Category**: DX / AI collaboration infrastructure
**Description**: BL-P2-063 PR #70에 포함된 T1 clippy-fix + T2.5 fmt-all 두 commit이 squash merge로 main에 `9128305`로 통합됨. 이 merge commit SHA를 `.git-blame-ignore-revs`에 등록하면 blame UI (GitHub + 로컬)가 mechanical 변경을 투명하게 스킵 → 진짜 저자 복원.

AI 개발 맥락에서 이는 "preference" 수준이 아니라 **claude-code 세션의 context-building 효율에 직결되는 인프라**. 자세한 배경은 `docs/git-blame-hygiene-in-ai-devflow.md` 참조.

필요 작업 (단계별):
1. **즉시**: `.git-blame-ignore-revs` 파일 생성, PR #70 merge commit (`9128305`) 등록 + 주석
2. **중기**: devflow 플러그인 hook 확장
   - `PostToolUse` 또는 `Stop` hook: mechanical commit (chore(fmt)/chore(clippy)/chore(deps)/chore(codemod) prefix) 자동 감지
   - Squash merge 방식 고려 → feature SHA 대신 merge commit SHA를 등록하는 follow-up PR 자동 생성
3. **정책**: CONTRIBUTING.md에 Mechanical commit 판정 기준 명시 (`docs/git-blame-hygiene-in-ai-devflow.md` §7.1 표 참조)
4. **Commit message**: `Blame-Ignore: true` footer 표준화 (자동화용 마킹)

**Acceptance**:
- `.git-blame-ignore-revs` 파일 존재 + merge commit 최소 1개 등록
- GitHub blame UI에서 해당 commit 스킵 동작 확인
- 정책 문서화 완료

**예상 작업량**:
- Phase 1 (파일 추가): 15분
- Phase 2 (hook 자동화): 1세션 (2~3시간)
- Phase 3 (정책 문서화): 30분

**Ref**: PR #70 cargo-review (agent C), `docs/git-blame-hygiene-in-ai-devflow.md`

### BL-P2-067: Clippy 정책 확장 파일럿
**Priority**: Low
**Category**: Code quality policy
**Description**: 현재 `Cargo.toml [lints.clippy]` 3개만 deny (`unwrap_used`, `expect_used`, `enum_glob_use`). BL-P2-063 cargo-review가 추가 후보 제안.
검토 대상 lint:
- `clippy::unwrap_in_result` — Result에서 unwrap은 흔한 오류 패턴
- `clippy::panic` — 명시적 panic 금지 (필요 시 `#[allow]`)
- `clippy::todo` / `clippy::unimplemented` — 미완성 코드 감지
- `clippy::dbg_macro` — 디버깅 코드 잔류 방지

필요 작업:
1. 각 lint 후보에 대해 현재 코드베이스 영향도 벤치 (`cargo clippy -- -W clippy::<lint>` 횟수)
2. 영향도 낮은 것부터 점진 deny
3. BL-P2-063과 동일 패턴 (autofix + manual + allow) 적용

**Acceptance**: 각 lint 0건 도달 + deny 추가 + CI 통과 유지.
**의존**: BL-P2-063 완료 (베이스 깨끗한 상태)
**Ref**: BL-P2-063 PR #70 cargo-review

### BL-P2-068: GitHub Actions SHA pinning (공급망 강화)
**Priority**: Low
**Category**: Security / Supply chain
**Description**: 현재 `.github/workflows/ci.yml`의 actions는 태그 기반 (`@v4`, `@v2`, `@stable`). 태그는 repo 소유자가 이동 가능 → 공급망 공격 벡터.
대안: commit SHA로 핀 (e.g., `actions/checkout@abc123def...`).

필요 작업:
1. 현재 actions 4개 (`actions/checkout@v4`, `dtolnay/rust-toolchain@stable`, `Swatinem/rust-cache@v2`) 각각 최신 안정 SHA 조회
2. SHA로 교체, 주석에 원 태그 명시 (`# @v4`)
3. Dependabot 설정 (`.github/dependabot.yml`)으로 매주 자동 bump PR 생성
4. Dependabot bump PR의 리뷰 프로세스 문서화

**Acceptance**: 모든 action이 SHA로 핀. Dependabot이 bump PR 자동 생성.
**Ref**: BL-P2-063 PR #70 cargo-review (agent B — quality reviewer 제안)

### BL-P2-069: 벤치마크 프레임워크 도입 (BL-P2-060/061 선결)
**Priority**: Medium
**Category**: Performance measurement / Test infrastructure
**Description**: BL-P2-060 (action_channel Result boxing) / BL-P2-061 (SwitchStateView enum boxing)은 "벤치 기반 판단 필요"로 defer됨. 선결 조건인 벤치 프레임워크가 아직 없음.
필요 작업:
1. `criterion` crate를 dev-dependency로 추가
2. `benches/` 디렉토리 구조 설계 (예: `benches/action_channel.rs`, `benches/state_machine.rs`)
3. 초기 벤치 케이스 작성:
   - ActionSender::send p50/p95/p99
   - SwitchStateMachine.state() clone cost
   - Context switch 전체 flow (mock adapter)
4. CI 또는 로컬 전용 실행 정책 결정 (벤치는 시간 많이 걸리므로 CI에 포함 여부 트레이드오프)
5. Baseline 수치 확보 → 이후 BL-P2-060/061에서 boxing 후 비교

**Acceptance**: `cargo bench` 실행 가능 + 3개 이상 벤치 케이스 + baseline 결과 문서화.
**의존**: BL-P2-063 완료
**차단해제**: BL-P2-060, BL-P2-061
**Ref**: BL-P2-063 PR #70 cargo-review (agent C)

### BL-P2-070: main.rs 구조 개선 (production wire 함수 추출 + AuthCredential 변환 + clap 검토)
**Priority**: Low
**Category**: Readability / Maintainability
**Description**: PR #75 cargo-review에서 식별된 3개 개선 사항 통합.
1. main.rs else 블록 ~140줄 → `fn wire_production_mode(...)` 또는 유사 함수로 추출하여 main 흐름 가독성 향상
2. `AuthCredential` 빌드 24줄 → `impl From<&CloudConfig> for AuthCredential` 변환 함수 추출 (단위 테스트 가능)
3. CLI 인자 파싱 `std::env::args()` + `windows(2)` → `clap` derive API 도입 검토 (인자가 4개 이상 되면 투자 가치 있음)
각 항목은 독립적으로 실행 가능. 기능 변경 없이 순수 리팩토링.
**Ref**: BL-P2-031 T3 PR #75 cargo-review (Suggestions S4/S5/S6)

### BL-P2-072: Unknown command 토스트 페이로드 일관성 + truncation/sanitize
**Priority**: Medium
**Category**: UX / Defensive UI
**Description**: PR3 cargo-review C4 + G3.
- C4: `:foobar` → `Command::Unknown("foobar")` (콜론 strip 후), `switch-project` 무인자 → `Command::Unknown(resolved)` (소문자 명령). 토스트 메시지가 입력에 따라 다른 형태로 노출. 정책 결정 필요: 원본 입력 보존 vs 일관된 resolved.
- G3: `format!("Unknown command: {raw}")`가 입력 원문을 그대로 토스트로 노출. 길이 제한(예: 64자 truncate, `…` 표기) + 개행/제어문자 제거 없음 → 악성 붙여넣기/긴 명령 시 토스트/상태바 레이아웃 깨짐 가능.
필요 작업:
- 페이로드 정책 결정 (원본 vs resolved) 문서화
- `safe_display(&str, max_len)` 유틸로 추출 (다른 사용자 입력 표시 지점에서 재사용 가능)
- BL-P2-050 (LogPanel 제어문자 필터링) 과 정책 정렬 검토
**Ref**: PR3 Unit 4.5 cargo-review C4 + G3

### BL-P2-075: legacy `:ctx` 명령 deprecation 타임라인
**Priority**: Low
**Category**: Tech debt
**Description**: PR3 cargo-review G6. `Command::ContextSwitch(String)` / `Command::ContextList`는 파서가 여전히 생성하나 실행부는 toast 안내뿐. dead path 축적 방지를 위해 deprecation 일정 명시 필요.
필요 작업:
- Unit 6 (ContextPicker) 머지 후 파서에서 `ctx` 매치를 `Command::Unknown` 위임으로 전환
- `Command::ContextSwitch` / `Command::ContextList` enum variant 제거
- 기존 `test_parse_context` 테스트 업데이트 또는 제거
- 사용자 안내 메시지 (CHANGELOG, help 토스트) 갱신
**의존**: Unit 6 (ContextPicker) 완료
**Ref**: PR3 Unit 4.5 cargo-review G6

### BL-P2-076: Command Bar 코드 품질 cleanup (가시성/toast 호출/Switch 테이블 단일화)
**Priority**: Low
**Category**: Code quality
**Description**: PR3 cargo-review 잔여 style/idiom finding 모음.
- S1 (Unit 4.5 리뷰): `App.input_bar` / `command_parser` 가시성 `pub(crate)` → 주변 wire 필드(`audit_logger` 등)와 동일하게 private + `#[cfg(test)]` accessor
- S3 (Unit 4.5): `add_toast` 호출 시 `format!()` / `.into()` 혼용 → 메시지 변수 분리 후 일관 호출
- S4 + G8 (Unit 4.5): `SWITCH_ABBREVIATIONS` (튜플) ↔ `COMMAND_TABLE` (struct) 형식 이원화 → 단일 `&[(&str, &str)]` source + `SWITCH_COMMANDS`는 iter로 유도
- G9 + S8 (Unit 4.5): `switch-project` / `switch-cloud` arm 복붙 → `fn require_arg(arg, resolved, ctor) -> Command` 헬퍼
- **Step 2/3 리뷰 추가 항목**:
  - S3 (Step 2/3): `ContextIndicator::new(std::time::Duration::from_secs(2))` 매직 2초 두 생성자 반복 → 모듈 상수 추출 또는 `with_default_highlight()` 팩토리
  - S4 (Step 2/3): `ContextIndicator::display_text()` accessor로 포맷 책임 이전 (StatusBar에서 target 내부 필드 직접 접근 제거)
  - S5 (Step 2/3): `status_bar.rs` `ctx_text` `Option::map` 패턴으로 이중 분기 제거
  - S6 + G9 (Step 2/3): `input_bar.rs` `_` wildcard → `InputMode::Normal \| Form \| Confirm =>` 명시적 매치 (variant 추가 시 drift 방지)
  - S7 (Step 2/3): `ContextIndicator::set_last_switch_at_for_test` — `pub(super)` 또는 private로 가시성 축소
  - S8 + G7 (Step 2/3): `ContextIndicator::set_target(_, bool)` boolean param → 메서드 분리 또는 enum
  - S9 (Step 2/3): `status_bar.rs` `use super::theme; use super::theme::Theme;` → `use super::theme::{self, Theme};` 합치기
  - S10 (Step 2/3): `set_input_mode` idempotent 가드 (`if self.input_mode == mode { return; }`) — C3
  - Step 2/3 Search arm (C5): `set_input_mode(Search)`에서 `history_reset_cursor`/`reset_completion` 호출 불필요 — Command 전용 분리
**Acceptance**: PR3 동일 동작 + 스타일 일관성 + 미래 variant 추가 시 컴파일러 감지.
**Ref**: PR3 Unit 4.5 cargo-review S1/S3/S4/S8/G8/G9 + PR3 Step 2/3 cargo-review S3~S10/C3/C5/G7/G9

### BL-P2-078: destructive ConfirmDialog API 강제력 보완 (Codex adversarial HIGH #2 후속)
**Priority**: Medium
**Category**: Safety / Release enforcement
**Status**: **Step 5 이후 처리 예정** — 신규 세션 시작 시 반드시 확인
**Description**: Codex adversarial review HIGH #2 권고 "타입/상태로 강제해 caller가 잊을 수 없게 하라"에서 PR3는 차선책(A+B: `ConfirmDialog::for_destructive` convenience + `ContextTarget::fingerprint` helper)만 채택. 여전히 `ConfirmDialog::yes_no`로 직접 호출하면 fingerprint/recontext escalation이 누락되는 opt-in 구조.

**미달된 조건**: "caller cannot forget" — 신규 destructive action 추가 시 `for_destructive`를 안 쓰면 그만.

**강제력 옵션 검토**:
- (A) Lint/CI test: destructive keyword(`Delete`/`Force`/`Evacuate` 등)를 포함한 Action variant에 대응하는 모듈의 `ConfirmDialog` 호출이 `for_destructive`인지 grep 검증. CI `cargo test --test destructive_enforcement`.
- (B) PendingAction 레벨 강제: `PendingAction::Delete*`가 `context_fingerprint: ContextTarget` 필드를 필수로 가지고 `to_dialog(&indicator)` 메서드 자체가 자동 attach. 16 모듈 파급 있으나 컴파일 강제.
- (C) ConfirmMode에 `kind: Destructive/NonDestructive` payload — Destructive variant는 생성 시 target 필수. 기존 4개 factory 시그니처 변경.
- (D) `#[must_use]` 또는 clippy custom lint — 약한 강제.

**필요 작업**:
1. 위 옵션 중 trade-off 분석 (Step 5 콜사이트 32개 적용 완료 후 재평가)
2. 선택된 옵션 구현 + 테스트
3. 신규 destructive action 추가 플로우 문서화 (CONTRIBUTING 또는 src/module/README)

**Acceptance**: 신규 destructive action을 `for_destructive` 없이 추가하면 CI/컴파일이 차단.

**타이밍**: PR3 Step 5 완료 후. Step 5가 실제 콜사이트 패턴을 드러내므로 그 이후가 강제력 설계 비용 최저.

**Ref**: Codex adversarial review HIGH #2 (verbatim 원문은 PR3 feat/bl-p2-031-pr3-commands-ui 브랜치 세션 기록).

### BL-P2-050: LogPanel 텍스트 정제 (제어문자 필터링) + toast safe_display 유틸
**Priority**: Medium (BL-P2-074 defer 통합 후 상향)
**Category**: Security / UX
**Description**:
- LogPanel의 push()가 임의 문자열을 받아 그대로 렌더링. API 에러 메시지에 ANSI 제어문자가 포함되면 TUI 표시 교란 가능.
- **신규 스코프 (BL-P2-074 code-plan R1 리뷰에서 defer, 2026-04-20)**: toast 표시 경로에 `safe_display(&str, max_len=60)` 유틸 신설. `rg safe_display src/` = 0 match 확인. BL-P2-074 NFR-4가 "신규 외부 입력 없음 + toast 터미널 이스케이프 방지"를 요구하지만 유틸 결정사항 4개(유틸 위치/truncate 문자/제어문자 정책/호출 사이트)가 BL-P2-074 scope를 팽창시켜 통합 defer.

필요 작업:
1. `safe_display(&str, max_len)` 유틸 — 위치 결정 (`src/ui/text.rs` 또는 `src/util/safe_display.rs`), truncate 표기(`…`), 제어문자 정책 (ASCII `\x00-\x1F`, `\x7F`, CR/LF 제거)
2. LogPanel::push() 적용
3. toast 발행 경로(`background::add_toast`) 적용 — BL-P2-074 에러 메시지 (`ApiError`, `SwitchError::NotConfigured` 등) 보호
4. unit 테스트: truncate / CR-LF / ANSI escape / null 입력

**Ref**: Codex Batch 3 리뷰 #5, BL-P2-074 code-plan R1 (Important I2)

### BL-P2-051: 기존 Nova adapter encode_param() 통일
**Priority**: Low
**Category**: Security / Consistency
**Description**: get_server, delete_server, get_flavor, delete_flavor 등 기존 메서드에서 URL 파라미터를 raw interpolation으로 사용. UUID 특성상 공격 벡터 낮지만, 신규 메서드와의 일관성을 위해 encode_param() 통일 필요.
**Ref**: Security Reviewer Batch 2 Important-1

### BL-002: Snapshots 서비스 타입 매핑 수정
**Priority**: Low
**Category**: Bug
**Description**: FetchSnapshots가 "Service unavailable:" 에러 발생. Cinder snapshot API가 volume 서비스와 동일 엔드포인트를 사용하는데 별도 서비스로 조회하는 문제.

### BL-003: DevStack Glance↔Nova 통신 오류 조사
**Priority**: Low
**Category**: Infra
**Description**: Server 생성 시 500 에러 (GlanceConnection). DevStack 환경에서 Glance 이미지 접근 경로 확인 필요. nexttui 코드 문제 아님.

---

## Phase 2 Backlog

> Substation(Swift TUI) 리버스 엔지니어링 분석 + Phase 1 defer 항목 기반.
> Stage 1 = 아키텍처 고도화, Stage 2 = 기능 확장, Stage 3 = 신규 백엔드.

### Stage 1: 아키텍처 고도화

#### BL-P2-002: Multi-Level Cache (L1/L2/L3)
**Priority**: Medium
**Category**: Architecture
**Description**:
- 현재 L1(인메모리 HashMap) 단일 레벨 → 3단계 티어링 도입
  - L1: 인메모리 (현재와 동일, 가장 빠름)
  - L2: gzip 압축 메모리 (대용량 리소스 목록의 메모리 사용량 절감)
  - L3: 디스크 영속 (앱 재시작 시 콜드 스타트 없이 즉시 표시)
- 리소스 종류별 차등 TTL (현재 구현됨) + 티어별 승격/강등 정책
- `cloud` 필드 기반 멀티 클라우드 캐시 격리 (현재 CacheKey에 cloud 있으나 L1만 활용)
- L3 디스크 캐시: index/data 일관성 보장, 비정상 종료 대비 WAL 또는 atomic write
**Motivation**: VDI 환경에서 앱 재시작이 잦고 서버 목록이 수천 건일 때 매번 API 전체 fetch는 비효율적. L2 압축으로 메모리 절약, L3 영속으로 콜드 스타트 개선
**Ref**: Substation `MultiLevelCacheManager.swift`, `OpenStackCacheManager.swift`
**주의**: Substation의 3중 캐시(OSClient L1/L2/L3 + TUI cacheManager + DataManager MemoryKit) 간 일관성 문제(Risk #1)를 반복하지 않도록, nexttui에서는 단일 Cache 구조체 내에서 티어링 구현

#### BL-P2-003: Intelligent Cache Invalidation (의존성 그래프 기반)
**Priority**: High
**Depends on**: BL-P2-002
**Category**: Architecture
**Description**:
- 리소스 간 의존성 그래프 구축 (예: Server → FloatingIP, Port, Volume)
- CUD 액션 완료 시 관련 리소스 캐시를 연쇄 무효화
- 시간 지연 무효화 지원 (3~10초, API 전파 시간 고려)
- 현재 Cache(RwLock, TTL, GC)를 확장하되 Substation의 3중 캐시 복잡성은 피함
**Motivation**: 서버 삭제 후 FloatingIP 목록에 여전히 연결된 IP가 보이는 등 캐시 불일치 문제 예방. 현재는 단순 TTL 만료에만 의존
**Ref**: Substation `IntelligentCacheInvalidation.swift`

#### BL-P2-003-B: DataProvider Registry 패턴
**Priority**: Medium
**Category**: Architecture
**Description**:
- 문자열 키 기반 DataProvider 조회 (`DataProviderRegistry.fetch("servers")`)
- 각 DataProvider가 Port를 통해 API 호출 → 결과 캐싱 → 갱신 이벤트 발행
- `refreshAllDataOptimized()`: Phase 1(독립) 리소스 우선 로드 → Phase 2/3 순차 로드
- `executeWithTokenRefresh` 래핑: 토큰 만료 시 자동 재인증 후 재시도
**Motivation**: 현재 각 모듈이 개별적으로 데이터를 fetch하는데, 공통 DataProvider 레이어로 캐시/재시도/우선순위를 중앙 관리
**Ref**: Substation `DataManager.refreshAllDataOptimized()`, `ServersDataProvider`

#### BL-P2-004: Adaptive Polling (이벤트 루프 최적화)
**Priority**: Low
**Category**: Performance
**Description**:
- 활성 입력 감지 시: 짧은 폴링 간격 (5ms)
- 유휴 상태: 지수 백오프로 최대 30ms까지 증가
- 현재 crossterm 이벤트 루프의 고정 폴링 간격을 적응형으로 전환
**Motivation**: VDI 환경에서 CPU 사용량 최소화. 입력 시 빠른 반응성 유지하면서 유휴 시 CPU 양보
**Ref**: Substation `TUI.swift` nodelay 모드 + 지수 백오프

#### BL-P2-005: ViewModel 분리 (Domain Model ↔ UI 표현 결합도 감소)
**Priority**: Medium
**Category**: Architecture
**Description**:
- Domain Module의 UI 표현 로직(컬럼 정의, 색상, 포맷팅)을 `view_model` 모듈로 분리
- Domain Model은 순수 데이터, ViewModel이 UI 위젯 파라미터 변환 담당
- UI 위젯 변경 시 view_model만 수정, Domain Model과 Component 로직은 무변경
**Motivation**: Agent Council Review 액션 아이템 #4. 현재 모듈 내에서 모델과 UI 표현이 혼재
**Ref**: Council review `agent-council-review.md` 항목 4

#### BL-P2-006: Microversion 협상
**Priority**: Low
**Category**: Infrastructure
**Description**:
- 서비스별 지원 API 마이크로버전을 자동 협상
- `X-OpenStack-Nova-Microversion` 등 헤더 자동 주입
- 서비스 카탈로그에서 버전 정보 추출 → 요청별 적절한 버전 헤더 설정
**Motivation**: OpenStack 서비스는 동일 엔드포인트에서 마이크로버전별로 응답 스키마가 달라짐. 현재는 고정 버전 또는 버전 미지정으로 호출
**Ref**: Substation `OpenStackClientCore.swift` MicroversionManager

### Stage 2: 기능 확장

#### BL-P2-011: 감사 로그 (Audit Log)
**Priority**: Medium
**Category**: Feature
**Description**:
- `~/.config/nexttui/audit.log`에 JSON Lines 형식 기록
- `AuditLogger`가 Action 채널 구독, 매 기록 즉시 flush
- 기록 항목: 타임스탬프, 사용자, 액션 종류, 대상 리소스, 결과(성공/실패)
- Log rotation 지원
**Motivation**: FR-18 (감사 로그). 운영 환경에서 누가 어떤 작업을 했는지 추적 필요
**Ref**: `detail-design-domain-nfr.md` 항목 G, user-story US-047

#### BL-P2-012: 통합 조회 (서버-리소스 연관 뷰)
**Priority**: Medium
**Category**: Feature
**Description**:
- 서버 상세에서 연결된 Volume, FloatingIP, SecurityGroup, Network를 한 화면에 표시
- 리소스 간 연관 관계 그래프 기반 탐색
**Motivation**: FR-19 (통합 조회). 현재 각 리소스를 개별 모듈에서만 볼 수 있어 서버 전체 상태 파악이 어려움
**Ref**: user-story US-048

#### BL-P2-013: UsageModule (리소스 사용량 모니터링)
**Priority**: Medium
**Category**: Feature
**Description**:
- Nova simple-tenant-usage API 연동
- 프로젝트별 vCPU, RAM, 디스크 사용량 표시
- 쿼터 대비 사용률 시각화
**Motivation**: Phase 1에서 deferred. 운영자가 프로젝트별 리소스 현황을 빠르게 확인할 수 있어야 함
**Ref**: session-summary Unit 14 (UsageModule deferred)

#### BL-P2-015: Attach / Detach / Associate 워크플로우
**Priority**: Medium
**Category**: Feature
**Description**:
- Volume Attach/Detach: 서버에 볼륨 연결/분리
- Volume Migration: 스토리지 백엔드 간 볼륨 이동 (US-032)
- FloatingIP Associate/Disassociate
- Role Grant/Revoke
- Quota Management
**Motivation**: Phase 1에서 defer된 CUD 확장. 빈번한 정기 운영 액션
**Ref**: session-summary Next Steps, user-stories US-027, US-030~032

#### BL-P2-031: 프로젝트 전환 + Keystone Rescoping (#39)
**Priority**: High
**Category**: Feature
**Depends on**: BL-P2-029 (다중 토큰 맵, 완료)
**Description**:
- 런타임 프로젝트/클라우드 전환 (SwitchCloud / SwitchProject)
- CommandParser→App 레벨에서 cloud 컨텍스트 전환
- Keystone rescoping으로 토큰 재발급 없이 프로젝트 전환
- Auth 재생성 플로우
**Motivation**: --cloud CLI(PR#55)로 시작 시 선택 가능하지만, 런타임 중 전환 미지원
**Ref**: GitHub Issue #39

#### BL-P2-016: 토큰 보안 강화
**Priority**: Low
**Category**: Security
**Description**:
- 메모리 내 토큰 암호화 저장 (AES-GCM 또는 OS keychain)
- 선제 갱신: 만료 5분 전 자동 갱신 시작
- HTTP 429/5xx 지수 백오프 재시도 (최대 3회)
**Motivation**: Substation이 AES-GCM으로 토큰 보호. 현재 nexttui는 plaintext 토큰 보유
**Ref**: Substation `OpenStackClientCore.swift` CoreTokenManager

#### BL-P2-017: 멀티 인증 방식 지원
**Priority**: Low
**Category**: Feature
**Description**:
- Keystone v3 password (현재) + appCredential + token 인증 지원
- HMAC (Cloudian), API Key 등 비-Keystone 인증 확장
- `AuthProvider` trait에 `sign_request()` 메서드 확장 (현재 Phase 2 주석만 존재)
**Motivation**: FR-05.4, FR-05.5. 멀티 백엔드 환경에서 인증 체계 중립적 설계 필요
**Ref**: `detail-design-port-adapter.md` Phase 2 HMAC/API Key 주석

#### BL-P2-033: TestBackend 스냅샷 테스트 확장
**Priority**: Low
**Category**: Testing / UX Verification
**Description**:
- 현재 FormWidget에만 있는 `TestBackend` 렌더 테스트를 핵심 UI 전체로 확장
- `insta` 크레이트 도입으로 스냅샷 기반 렌더링 회귀 테스트 구축
- 대상: 서버 리스트 테이블, 디테일 패널, 네비게이션 바, 상태 바 등
- 다양한 터미널 크기(40x10, 80x24, 120x40 등)에서 레이아웃 깨짐 검증
- 키 입력 후 상태 변화 → 재렌더 → 스냅샷 비교 시나리오
**Motivation**: 현재 830+ 테스트가 로직/상태 전이를 검증하지만, 실제 렌더링 출력은 FormWidget 5개 테스트만 커버. UI 변경 시 레이아웃 깨짐이나 스타일 변경을 자동 감지할 수 없음
**Ref**: `src/ui/form.rs:2160` 기존 `render_to_buffer` 헬퍼

#### BL-P2-018: 커스텀 키 바인딩
**Priority**: Low
**Category**: UX
**Description**:
- Config 파일에서 키 바인딩 커스터마이징 로드
- 기본 키맵 + 사용자 오버라이드
**Motivation**: detail-design-ui-input.md에서 Phase 2로 분류
**Ref**: `detail-design-ui-input.md` Config 항목

#### BL-P2-019: 이미지 로컬 파일 업로드
**Priority**: Low
**Category**: Feature
**Description**:
- Glance 이미지 생성 시 URL 지정 외에 로컬 파일 업로드 지원
- 파일 선택 UI + 진행률 표시
**Motivation**: Phase 1은 URL 지정만 지원
**Ref**: `detail-design-domain-nfr.md` line 1171

#### BL-P2-020: Service Layer 전환 대비
**Priority**: Low
**Category**: Architecture
**Description**:
- AdapterRegistry에서 직접 호출 Adapter를 Service Layer 프록시 Adapter로 교체
- Admin API GW 경유 모드 지원
- `replace_*()` 메서드 활용한 런타임 Adapter 스왑
**Motivation**: TR-09. Phase 1의 Thick Client에서 Phase 2의 Service Layer 중심으로 점진 전환
**Ref**: `detail-design-port-adapter.md` Phase 2 Adapter Swap 섹션

### Stage 2.5-B: Visual Enhancement (Medium Priority)

> Stage 2.5-A (Theme & Polish)는 전체 완료 — PR #51~#53
> Stage 2.5-B 전체 완료 — PR #60

### Stage 2.5-C: Advanced Layout (Low Priority)

##### BL-P2-044: 반응형 레이아웃 모드
**Priority**: Low
**Category**: UX
**Description**:
- `LayoutMode` enum: TooSmall / Compact / Standard / Wide
- Compact (< 120x30): Sidebar 숨김, 단일 패널
- Standard (120x30+): 현재 Sidebar + Content
- Wide (200x50+): Sidebar + List + Detail 동시 표시 (3-column)
- devflow-tui `LayoutManager` 패턴 참조
**Motivation**: 터미널 크기에 따라 최적 레이아웃 제공

##### BL-P2-045: 다크 테마 옵션
**Priority**: Low
**Category**: UX
**Depends on**: BL-P2-034 (완료)
**Description**:
- Config에서 `theme: dark | light` 선택
- 다크 테마: btop 스타일 어두운 배경 + 밝은 텍스트
- Theme 구조체에 variant 추가
**Motivation**: btop처럼 다크 테마가 장시간 모니터링에 적합

##### BL-P2-046: NO_COLOR 접근성 지원
**Priority**: Low
**Category**: UX
**Depends on**: BL-P2-034 (완료)
**Description**:
- `NO_COLOR` 환경변수 감지 시 색상 대신 Bold/Dim/Underline만 사용
- devflow-tui `no_color()` 패턴 참조
- 모든 Theme 메서드에 NO_COLOR 분기 추가
**Motivation**: 터미널 접근성 표준 (https://no-color.org)

### Stage 3: 신규 백엔드

#### BL-P2-021: Manila (Shared FS / NAS)
**Priority**: Low
**Category**: New Backend
**Description**: Share Network, Migration, QoS, CIFS Account 관리
**Ref**: user-stories Phase 2 예정 서비스

#### BL-P2-022: Cloudian (Object Storage)
**Priority**: Low
**Category**: New Backend
**Description**: Policy, Bucket, Group, Monitor, Permission, QoS 관리. S3 HMAC 인증 필요 (BL-P2-017 선행)
**Ref**: user-stories Phase 2 예정 서비스

#### BL-P2-023: Network System Admin
**Priority**: Low
**Category**: New Backend
**Description**: Routing Table, VPC, Subnet, Routing Rule, NACL, External Network 관리
**Ref**: user-stories Phase 2 예정 서비스

#### BL-P2-024: Placement
**Priority**: Low
**Category**: New Backend
**Description**: Resource Provider, Inventory 조회/관리
**Ref**: user-stories Phase 2 예정 서비스

## Completed

- **BL-001**: Submit 확인 화면 + Toast 피드백 (PR #27, 2026-03-25)
- **BL-P2-001**: Module Registry 시스템 (PR #28, 2026-03-25)
- **#32 BL-P2-027**: Error enum `#[non_exhaustive]` 적용 (PR #36, 2026-03-26)
- **#30 BL-P2-025**: Clippy 엄격 lint 정책 도입 (PR #36, 2026-03-26)
- **#35 BL-P2-030**: Pagination Combinator 추상화 (PR #36, 2026-03-26)
- **#31 BL-P2-026**: tracing 구조적 로깅/계측 도입 (PR #37, 2026-03-26)
- **#33 BL-P2-028**: 토큰 캐시 파일 영속화 (PR #38, 2026-03-26)
- **#34 BL-P2-029**: Scope 기반 다중 토큰 맵 (PR #40, 2026-03-26)
- **#12 BL-P2-010**: RBAC 3단계 권한 제어 (PR #42, 2026-03-26)
- **#41 BL-P2-032**: 전체 프로젝트 리소스 조회 all_tenants (PR #43, 2026-03-27)
- **#16 BL-P2-014**: Server Migration / Evacuate (PR #44, 2026-03-30)
- **Server Resize**: flavor SelectPopup (PR #47~#48, 2026-03-31)
- **BL-P2-034**: Theme 시스템 도입 (PR #51, 2026-03-31)
- **BL-P2-035**: Rounded 보더 + 포커스 피드백 (PR #51, 2026-03-31)
- **BL-P2-036**: 패널 타이틀 포맷 (PR #51, 2026-03-31)
- **BL-P2-037**: 상태바 리디자인 (PR #51, 2026-03-31)
- **BL-P2-038**: 리스트 하이라이트 개선 (PR #51, 2026-03-31)
- **BL-P2-039**: 헤더 리디자인 (PR #52, 2026-03-31)
- **BL-P2-040**: 상태 아이콘 도입 (PR #53, 2026-04-01)
- **UX 가시성 수정**: ALL 뱃지, admin 마커, 패널 타이틀 (PR #54, 2026-04-01)
- **--cloud CLI**: 시작 시 클라우드 선택 (PR #55, 2026-04-01)
- **Auto-Refresh Polling**: FetchDedup + API Backoff (PR #56, 2026-04-02)
- **Activity Log**: StatusBar 에러 뱃지 (PR #57, 2026-04-02)
- **help_hint()**: 14개 모듈 컨텍스트 인식 힌트 (PR #58, 2026-04-02)
- **HostModule**: Composite Host Operations Panel (PR #59, 2026-04-04)
- **BL-P2-041**: 스크롤바 추가 (PR #60, 2026-04-06)
- **BL-P2-042**: Content 보더 컨테이너 (PR #60, 2026-04-06)
- **BL-P2-043**: Detail 섹션 구분 개선 (PR #60, 2026-04-06)
- **BL-P2-015**: Volume Attach/Detach + FloatingIP Associate/Disassociate (PR #61, 2026-04-08)
- **BL-P2-011**: 감사 로그 Audit Log 연동 (PR #62, 2026-04-08)
- **BL-P2-012**: 통합 조회 — SG 섹션 + 리소스 네비게이션 (PR #63, 2026-04-08)
- **BL-P2-013**: UsageModule — btop 스타일 사용량 대시보드 (PR #64, 2026-04-10)
- **BL-P2-005**: ViewModel 분리 — ViewContext 패턴 도입 (PR #65, 2026-04-10)
- **BL-P2-064**: cargo audit CI gate + rustls-webpki CVE fix (PR #73, 2026-04-15)
- **BL-P2-031**: PR3 Commands & Safety UI — Unit 4.5 Command Bar 통합 + Unit 5 ConfirmDialog fingerprint/TypeToConfirm (PR #76, 2026-04-18)
- **BL-P2-071**: Command history persist — App::shutdown() + save_history hook (PR #76, 2026-04-18)
- **BL-P2-073**: InputMode 단일화 — component::InputMode 단일 소스 + set_input_mode 헬퍼 (PR #76, 2026-04-18)
- **BL-P2-077**: PR3 cargo-review 잔여 MED — unicode-width 전환 + NO_COLOR bg 제거 (PR #76, 2026-04-18)
- **BL-P2-079**: PR3 Codex 2차/3차 review 잔여 finding — confirm reset + Usage refetch + Tab cycling + input_mode/Network form reset (PR #76, 2026-04-18)
- **BL-P2-074**: SwitchCloud wire 완결 — `ContextRequest::CloudOnly` variant + `CloudConfig::default_project` 필드 + `SwitchError::NotConfigured` + `ContextSwitcher` idempotent fast-path (PR #78, 2026-04-20). safe_display는 BL-P2-050으로 defer. Codex P2 2건(tracing Instrument, slash in default_project) 반영.
- **BL-P2-086**: Live-migrate stale port binding diagnosis — A1 worker error enrichment + A2 Server Detail "Port bindings" 섹션(admin only, INACTIVE+migrating_to ⚠ 마커). NeutronPort::list_port_bindings + binding-extended adapter. +16 tests (PR #83, 2026-05-08). Follow-ups: BL-P2-087 cleanup action, BL-P2-088 pre-flight check.
