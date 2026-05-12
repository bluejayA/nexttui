# Requirements Analysis

**Depth**: Standard
**Timestamp**: 2026-04-24T10:50:00+09:00
**Work Item**: BL-P2-085 (P0, Critical) — Cross-project scoping 전면 fix

## User Intent

`:switch-project`로 project 전환 후 server create 시 `SecurityGroupNotFound` → ERROR 인스턴스가 발생하는 근본 원인인 **4겹 cross-project scoping 버그**를 구조적으로 제거한다. admin 토큰 사용자가 active scope와 불일치하는 project의 리소스를 mutation/delete 할 수 있는 운영 사고 위험을 차단한다.

PR#82 hotfix(c4590ab, 2026-04-24 머지)가 표면 증상(server form stale cache + 동명 옵션 disambiguation)만 차단했으므로, 이 위에 구조적 fix를 누적한다.

### 확정된 정책 모델 (해석 분기 결과)

**A (엄격 차단) + C-lite (구조화 이벤트 로그)** 채택.

- 모든 사용자(admin 포함)는 `active_scope`와 불일치하는 project 리소스를 mutation/delete 할 수 **없다**. UI가 해당 경로를 막는다.
- 차단되는 모든 cross-project 시도를 구조화된 `cross_project_block` 이벤트로 로그에 기록 → 감사 + 사용자 패턴 분석 양쪽에 재사용 가능한 스키마.
- **기각된 대안**:
  - B (opt-in) — B2(broad)는 현재 단일-프로젝트 UI 컨셉을 크게 재설계해야 하고 BL 한 건 범위를 초과(Unit 6 ContextPicker 연관). B1/B3은 운영 편의성 대비 구현 비용 정당화 어려움. 필요 시 별도 BL로 분리.
  - 감사 뷰어 / 패턴 대시보드 = 이벤트 스키마는 이번에 픽스하되 뷰어/UI 소비는 후속 BL.

## Functional Requirements

### FR1 [Critical] Adapter 읽기 경로 tenant-scoping
- `src/adapter/http/neutron.rs`의 SG / Network / FloatingIp List 빌더가 `active_scope`의 project_id를 `tenant_id` 쿼리 파라미터로 주입한다.
- `src/adapter/http/nova.rs`, `src/adapter/http/cinder.rs`의 list 엔드포인트는 `all_tenants` 플래그 모델로 통일 — 기본 미사용(scope-only), 의도적으로 필요한 상위 호출부만 명시 활성화.
- **수용 기준** (둘 다 충족):
  - (요청 측) List 호출 시 HTTP 요청 쿼리에 `tenant_id={active_scope}` (또는 등가의 `all_tenants=false` + project-scoped token)이 실제로 포함됨을 mock HTTP가 assertion.
  - (응답 측) mock이 cross-project 리소스를 섞어 반환해도 상위 레이어에 전달되는 결과는 `active_scope`에 속한 리소스만 포함.

### FR2 [Critical] Worker mutation origin-scope guard (구 "target-scope guard")
- `src/worker.rs`의 모든 mutation Action dispatch 직전 **origin-scope 가드**를 수행한다.
- **개정 시맨틱** (2026-04-24, application-design Assumption A1 검증 후): `Action` enum이 `project_id`를 per-variant로 가지지 않음이 실측 확인되어, "target.project_id 비교" 대신 **"Action 생성 시점의 scope가 현재 active scope와 동일한지 비교"** 로 재정의.
- 구현: 중앙 dispatch 지점에서 `StampedAction { action, origin_project_id }` 래퍼로 감싸 Action을 발행. worker가 `stamped.origin_project_id == current_active_scope.project.id` 검사.
- **커버되는 위협 시나리오**:
  - TOCTOU: 폼 open 당시 scope=A, 사용자가 `:switch-project B`로 전환한 뒤 폼을 submit → origin_project_id=A != active=B → 거부
  - Cache stale: mutation이 비동기 지연되는 동안 scope가 전환된 경우 동일하게 거부
- 가드 실패 시: mutation을 거부하고 `cross_project_block` 이벤트(reason=`project_scope`)를 emit + 사용자에게 토스트로 즉시 알림.
- **Mutation 판정 기준**: `Action`에 `is_mutation()` 헬퍼 추가 (enum exhaustive match). read-only Action(`Fetch*`, `Navigate`, `Back` 등)은 가드 대상이 아님. 모든 Action은 `true`/`false` 중 하나로 명시적 분류되어야 하며, 미분류는 컴파일 에러.
- **ID 위조 시나리오 (target이 실제로 다른 scope)**: 이 가드가 직접 커버하지는 않음. FR1(adapter scope 필터)과 FR4(form 검증)가 커버. TUI 환경에서는 사용자가 직접 ID를 입력하는 경로가 제한적이므로 현실적 위협 최소.
- **수용 기준**:
  - (a) mock worker 테스트에서 `origin_project_id=A`, `current_active=B`인 mutation Action dispatch가 `AppError::CrossProjectBlocked`(또는 application-design에서 확정된 variant)로 거부되고 이벤트가 기록됨.
  - (b) `is_mutation()` 구현이 enum exhaustive → 미분류 Action은 컴파일 실패.
  - (c) read-only Action은 `StampedAction` 래핑 없이 통과(또는 래핑되어도 guard 비대상 분기).

### FR3 [Critical] RBAC project-mismatch 정책
- `src/infra/rbac.rs`에 project-mismatch 차단 규칙을 추가한다. 현재 role-tier 가드 위에 누적(AND) 적용.
- RBAC 결정 순서: (1) role-tier 검사 → (2) project-scope 일치 검사. **둘 다 PASS**해야 Action 허용. role-tier PASS + scope FAIL 조합은 deny.
- 차단 결정 시 FR5 이벤트의 `reason` 필드가 `role_tier` / `project_scope` / `both`로 구분 가능해야 한다 (거부 원인 식별 가능).
- **수용 기준**: 단위 테스트로 role×scope 2차원 매트릭스(admin/member/reader × match/mismatch) 커버. admin+mismatch 케이스가 deny이고 reason이 `project_scope`로 기록됨.

### FR4 [High] Form-selected ID cross-project 검증
- `src/adapter/http/cinder.rs`의 CreateSnapshot 및 다른 form 기반 mutation에서 선택된 리소스 ID의 project_id가 현재 `active_scope`와 일치하는지 제출 시점에 검증한다.
- 검증 실패 시 form이 제출되지 않고 에러 토스트 표시.
- **수용 기준**: 단위 테스트에서 cross-project ID를 담은 폼 제출 요청이 거부됨.

### FR5 [High] 구조화된 `cross_project_block` 이벤트 로깅
- FR2/FR3/FR4의 차단 이벤트를 공통 스키마로 기록한다:
  ```
  {
    timestamp, actor_user_id, actor_cloud,
    active_project_id, target_project_id,
    action_type, resource_kind, resource_id,
    outcome: "blocked",
    reason, fingerprint
  }
  ```
- 필드 정의:
  - `reason`: `role_tier` / `project_scope` / `both` / `form_selection` / `adapter_filter` 중 하나. 차단을 유발한 계층 식별.
  - `fingerprint`: `sha256(actor_user_id ∥ active_project_id ∥ target_project_id ∥ action_type ∥ resource_id)` 앞 12자. 동일 시도의 반복 그룹핑용.
- 출력 채널: 기존 nexttui 로그 파일 (`~/Library/Caches/nexttui/nexttui.log.<UTC-date>`)에 append. Grep 가능한 prefix/tag 사용 (예: `[cross-project-guard]`).
- 전용 로그 파일 분리 여부는 구현 시 판단 (tag prefix로 충분하면 분리 불요).
- PII: `actor_user_id`는 Keystone user ID 평문 기록. 해싱은 후속 옵션 (이번 BL 범위 밖).
- **쓰기 보장 수준**: best-effort — IO 실패 시 기록은 유실될 수 있으나(단, 차단 자체는 이미 완료된 상태), 동일 프로세스 내 에러 로그(`tracing::error!`)에 남는다. "이벤트 손실 0%"는 보장하지 않는다 (NFR1과 일관).
- **수용 기준**: 차단 이벤트 발생 시 위 스키마의 structured log line이 파일에 기록된다. 필드 중 하나라도 누락되면 테스트 실패.

### FR6 [Medium] 사용자 토스트 카피
- 차단 시 사용자에게 표시할 토스트 메시지 공통 카피를 확정한다 (예: `"차단: 활성 프로젝트 '{active}'에서 '{target}' 리소스는 수정할 수 없습니다. :switch-project {target} 후 재시도하세요."`).
- 정확한 카피는 구현 시 확정 (i18n은 이번 범위 밖).
- **수용 기준**: (a) 차단 발생 시 UI 테스트/통합 테스트에서 토스트가 한 번 표시됨을 검증. (b) 토스트 메시지에 `active_project_name`과 `target_project_name` 두 값이 포함됨. (c) 공통 카피가 단일 모듈에서 생성되어 재사용(복붙 카피 금지).

## Non-Functional Requirements

### NFR1 보안
- **차단 실패 금지 (우선순위 1)**: 4겹 버그 전부에 대해 false negative(차단해야 하는 시도가 통과)는 허용되지 않는다. 테스트로 보장.
- **이벤트 손실은 best-effort**: `cross_project_block` 이벤트는 IO 실패 시 로그 파일 append가 실패할 수 있으나, 그 경우에도 **차단 자체는 이미 완료**되어 있어야 한다. 이벤트 기록과 차단 행위는 독립 경로이며, 차단이 이벤트 기록에 의존하지 않는다. (FR5 "쓰기 보장 수준"과 일관)

### NFR2 회귀 테스트 / CI 게이팅
- **확정된 테스트 전략 = T1 (mock unit만 merge-blocking)**.
- mock HTTP 어댑터 + mock worker로 **FR1~FR5 전체 5개 FR** 단위 테스트 커버. ("4축"이 아니라 5개 FR — adapter(FR1) + worker(FR2) + rbac(FR3) + form(FR4) + log(FR5).)
- DevStack 통합 테스트(두 project 동명 SG 시나리오)는 **로컬/수동 실행만** — CI merge 게이트 아님.
- 이유:
  - BL-P2-081이 `devstack-integration` CI placeholder의 "정식 activation" 우선권 보유 (메모리 기준).
  - mock 단위 테스트가 deterministic + 빠름 + 구조 커버 충분.
  - DevStack은 "실측 안전망"이지 매 PR 게이트가 아님 (flakiness 관리).

### NFR3 호환성
- PR#82 hotfix 위에 누적 — `build_disambiguated_opts` 헬퍼와 `on_context_changed` cache-clear 경로는 유지·재사용.
- `build_disambiguated_opts`를 다른 모듈(server 외 volume/snapshot/network 등)의 드롭다운에도 확장 여부는 **구현 재량** (이번 범위에 포함하면 Open Question 해결 후 결정).

### NFR4 브랜치/배포 정책
- `main` 직접 커밋 금지. `feature/bl-p2-085-cross-project-scoping` (또는 도출된 유사 이름) 브랜치 + PR.
- CONSTRUCTION 완료 후 `/codex:review --scope branch --base main` 게이트 실행. 선택적으로 adversarial-review.

## Technology Stack

| 계층 | 선택 | 소스 | 비고 |
|------|------|------|------|
| Language | Rust edition 2024 | Brownfield 감지 | 변경 없음 |
| Framework | ratatui 0.30 + crossterm 0.29 | Brownfield 감지 | 변경 없음 |
| HTTP | reqwest | Brownfield 감지 | 변경 없음 |
| Test | `#[cfg(test)]` + `tests/` integration | Brownfield 감지 | mock HTTP 어댑터 기반 unit 추가 |
| Logging | tracing | Brownfield 감지 | 구조화 이벤트 필드 추가 |

→ 전체 스킵 (사전 지정 + Brownfield 완전 커버).

## Assumptions

Assumption 위험도 등급: ⚠️ = 거짓이면 설계/범위 재조정 필요.

1. **⚠️** `active_scope`는 `src/context/` 모듈에서 전파되는 단일 값으로 이미 확립되어 있다 — 새로 도입할 필요 없음 (PR#80 `TokenScopeFingerprint` 등). **거짓 시 영향**: FR2/FR3/FR4의 "target vs active" 비교 기반이 흔들림 → application-design에서 전파 경로 실측 확인 선행 필요.
2. **⚠️** mock HTTP 테스트 인프라는 기존 `src/adapter/http/` 하위에 이미 존재하거나 일반 관례로 구축 가능하다. **거짓 시 영향**: NFR2 T1 전략 자체가 재설계 대상 (Construction 초반 체크 필요).
3. **⚠️** `build_disambiguated_opts` 헬퍼(PR#82 도입)는 서버 모듈 외 다른 모듈에도 "이름만 바꿔서" 적용 가능한 순수 함수 형태다 — 호출부만 추가하면 확장 가능. **거짓 시 영향**: NFR3 재사용 가정과 Open Q2(다른 모듈 확장 여부) 결정이 뒤집힘 → 확장 자체를 포기하거나 별도 리팩터링 선행.
4. `cross_project_block` 이벤트의 소비자(뷰어/대시보드)는 이번 BL 범위 밖이며, 후속 BL에서 동일 스키마를 기반으로 구축한다.
5. DevStack 싱글노드 환경은 개발자 로컬에서 이 BL의 수동 검증에 사용된다 (기동 중, 이전 세션 정보).

⚠️ 표시된 가정은 CONSTRUCTION 진입 직후 실측 검증이 필수다 (application-design 또는 TDD RED 단계 초기).

## Open Questions

1. **[구현 단계에서 결정]** `tenant_id` 필터를 주입하지 못하는 특수 endpoint (예: Keystone admin-only global list, quota/pricing API 등)가 존재하는가? 존재하면 해당 endpoint는 FR1 대상에서 제외하고 worker/RBAC 레벨에서 커버한다. (application-design에서 adapter 매트릭스 작성 시 확정)
2. **[구현 단계에서 결정]** `build_disambiguated_opts`를 서버 외 모듈(volume/snapshot/network 등)로 확장할지 — 이번 PR 범위 포함 여부. (workflow-planning 또는 application-design의 component 스코프에서 확정)
3. **[구현 단계에서 결정]** Worker 거부 에러 타입 — `WorkerError::CrossProjectBlocked` 신규 variant vs 기존 `SwitchError`/`RbacDenied` 재사용. (application-design에서 에러 분류 확정)
4. **[workflow-planning에서 결정]** 구현을 1개 PR로 통합 vs adapter/worker/rbac 분할 PR. 안정성 우선과 리뷰 부담의 트레이드오프.

## Change Log
- [2026-04-24T10:50:00+09:00] INITIAL — 4겹 버그 매핑 + A+C-lite 정책 모델 + T1 테스트 전략 기반 초안 작성
- [2026-04-24T11:00:00+09:00] REVIEW-RESPONSE — spec-reviewer must-fix 3건 + should-consider 일부 반영: FR1 요청측 수용기준 추가, FR2 mutation 판정기준 명시, FR3 reason 분리 조건, FR5 fingerprint 정의 + best-effort 쓰기 보장, FR6 수용기준 추가, NFR1 손실-없음 문구 정정(best-effort), NFR2 "4축"→"5 FR" 정정, Assumptions 위험도 플래그 추가
- [2026-04-24T11:50:00+09:00] DESIGN-DERIVED — application-design LIST에서 실측 확인으로 아래 보정:
  - FR2 시맨틱 개정: `target.project_id == active_scope` → `origin_project_id == current_active_scope`. 근거: `Action` enum이 `project_id`를 per-variant로 가지지 않음 (55개 dispatch site). 중앙 dispatch 지점 `StampedAction` 래퍼로 스탬핑하여 TOCTOU/cache-stale 시나리오 커버. ID 위조 시나리오는 FR1+FR4에 위임.
  - NFR2 용어 정정: mock HTTP 서버 도입 대신 **pure fn URL/query 빌더 추출** 방향 확정 (dev-dep 증가 없음, 기존 serde 단위 테스트 스타일과 일관).
  - 용어 통일: `active_scope` → `TokenScope`/`ScopedAuthSession`/`RbacGuard::project_id()` 실제 타입으로 DETAIL 단계에서 매핑.
