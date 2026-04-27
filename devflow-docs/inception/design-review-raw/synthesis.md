# Council Synthesis — BL-P2-085 application-design

**Timestamp**: 2026-04-24T13:30:00+09:00
**Council Mode**: FULL (Codex 0.121.0 + Gemini 2.5-pro + Claude 의장)
**Raw**: `codex.md`, `gemini.md` 동일 디렉토리

## 충돌 해결 4단계 적용

### 1. 공통 의견 (두 AI 모두 지적 → 즉시 채택)

| 항목 | Codex 표현 | Gemini 표현 | 조치 |
|------|-----------|------------|------|
| 중앙 스탬핑 | "Stamp once at `ActionSender` boundary" | "Centralize stamping — `app_ctx.dispatch()` helper" | 55 site 수정 폐기, **기존 `ActionSender` + `VersionedEvent<Action>`에 origin 필드 추가**하는 방식으로 전환 |
| `is_mutation()` exhaustive 보장 | "Must stay in lockstep with `action_to_kind()`" | "Use match with no default (`_`)" | exhaustive match + 기존 `worker.rs:151 action_to_kind()`과의 parity 테스트 추가 |
| Fingerprint canonicalization | "Canonicalize (length-prefix or JSON)" | "Canonicalize `resource_id` (empty string if `None`)" | 단순 concat 대신 delimiter + Option 명시 규칙 확정 |
| Glance `DeleteImage` 보호 | "Not safe to defer… mutation surface inconsistent" | "Ensure `is_mutation() == true`" | **FR2(origin-guard) + FR4(form validator)로 확실히 커버**. Codex는 더 엄격 요구 — 아래 Section 2 참조 |
| PII 허용 가능 | "Acceptable if retention documented" | "Use Keystone UUID not username" | 평문 허용 + Keystone **UUID 사용 명시** (username 회피) |

### 2. 상충 의견 (근거 비교 후 판정)

| 논점 | Codex 입장 | Gemini 입장 | 의장 판정 |
|------|-----------|------------|----------|
| **Glance 제외의 안전성** | 🔴 "Not safe to defer" — 동일 origin 상황에서 cross-target 차단 안 됨 | 🟡 "justified trade-off, `is_mutation()=true`면 worker guard가 커버" | **Codex 지지**. Gemini는 FR2가 origin-match이므로 "동일 origin에서 cross-target 대상 mutation"을 FR2가 잡지 못함을 간과. 구체 시나리오: admin이 project A에 머문 채 Glance 전역 리스트에서 project B 이미지 ID 확보 → DeleteImage(B_id) 발행 시 origin=A=active=A → FR2 통과, Glance 서버가 admin 토큰으로 실행 → B 이미지 삭제. **Must-fix**: Glance DeleteImage/UpdateImage에 대해서만이라도 **form 검증(FR4) 또는 adapter pre-mutation scope check** 추가. 전면 FR1 Glance 편입은 복잡(visibility 모델)해도, **DeleteImage 한 경로는 scope 검증 의무화** 가능. |
| **CrossProjectGuard trait vs free fn** | "Free fn 충분, trait 불필요 (정책 하나뿐)" | "Trait이 discoverability에 유리" | **Codex 지지**. 현재 정책 backend 1개뿐이고 trait은 premature abstraction. free fn으로 유지. |
| **T1 mock-only 적절성** | "Risky for P0, 최소 1개 integration path 추가" | "Structural fix 규모에 적절" | **Codex 부분 지지**. 현재 CI에 `devstack-integration` placeholder가 BL-P2-081 우선권 보유라 활성화는 피해야 함. 그러나 **mock 기반 "end-to-end form→worker→adapter" 하나는 merge-blocking으로 추가**할 수 있음 — 이는 진짜 integration이 아닌 Rust 내 adapter/worker/form 3축 연결 테스트. Should-consider. |

### 3. 단독 의견 (한 AI만 제기)

#### Codex 단독 발견 — 모두 근거 강함

| 항목 | 평가 | 조치 |
|------|------|------|
| **FR2 동일 origin cross-target gap** (Section A) | ✅ 정당. origin-match는 "폼 open 후 scope 전환"은 잡지만 "동일 scope에서 타 project 리소스 ID로 직접 mutation"은 잡지 못함. | 위 Glance 논의로 이미 커버. FR4 form validation + (선택적) adapter pre-mutation 확인으로 보강. **Must-fix (scope 제한적)**. |
| **A1 type reference가 틀렸음** | ✅ 정당. 실측 재확인: `TokenScope` (port/types.rs:42)은 enum `Project { name, domain }` — id 없음. `ScopedAuthSession` (scoped_session.rs:33)는 struct에 project field 자체 없음. 실제 project_id 소스는 `Token.project.id` (ProjectScope at port/types.rs) 또는 cached `RbacGuard.project_id`. | **Must-fix**: application-design.md 타입 레퍼런스 교정. |
| **기존 `ActionSender` + `VersionedEvent<Action>` envelope 존재** | ✅ 정당. `src/context/action_channel.rs`에 이미 ActionSender가 있음. StampedAction은 두 번째 envelope. | **Must-fix (설계 전환)**: StampedAction 래퍼 **폐기**. 대신 `VersionedEvent`에 `origin_project_id` 필드 추가하거나, ActionSender::send에서 current scope를 함께 stamp. 55 call site 수정 → 5 파일 수정으로 축소. |
| **Worker 스탈 action drop 부재** | 🟡 부분 정당. epoch 기반 drop은 BL 스코프 밖 pre-existing 이슈. 다만 origin-stamp가 있으면 "스탈 action이 새 scope에서 실행 시도"는 차단됨. 완전 커버는 아니나 BL 범위에서는 충분. | Should-consider. 별도 개선 BL로 분리 고려 (BL-P2-086?). |
| **Token refresh path: project_id=None 덮어쓰기** | ✅ 정당. `RbacGuard::update_roles(roles, project_id: Option<String>)` 호출 시 refresh path가 `None`을 전달하면 FR3 scope check가 깨짐. | **Must-fix**: refresh 이벤트에서 project_id를 유지(또는 명시적 읽기) 확인 + RbacGuard가 None으로 overwrite되지 않도록 방어. |
| **FR1 response-side assertion 부재** | ✅ 정당. 현재 pure fn URL 빌더 테스트만으로는 "응답 측 필터링 효과"를 검증하지 못함. requirements.md FR1 수용 기준 (b)와의 gap. | **Must-fix**: mock HTTP 없이 가능한 형태로 — adapter layer에서 response가 오면 **client-side re-filter**(방어심층)를 추가하고, 그 re-filter를 단위 테스트. |
| **Missing schema fields: `guard_layer`, `epoch/correlation_id`** | ✅ 정당. 이벤트 재구성/원인 추적에 도움. | **Should-consider**: FR5 스키마에 `guard_layer` (fr1/fr2/fr3/fr4) + `correlation_id` (u64 epoch 또는 uuid) 추가. |
| **`target_project_id` 의미 모호 (origin-match 하에서)** | ✅ 정당. FR2가 origin-match이면 event field 이름이 혼란을 줌. | **Should-consider**: 필드 rename — `target_project_id` → `asserted_origin_project_id` 또는 `blocked_action_origin_project_id`. 실제 target이 확인된 경우만 `target_project_id` 남기기. |

#### Gemini 단독 발견

| 항목 | 평가 | 조치 |
|------|------|------|
| **Background polling task mutations** (`poll_migration_progress` 등) | ✅ 정당. | **Should-consider**: worker.rs 내 background task가 Action emit하는 경로에 대해 `is_mutation()=true`인 것은 반드시 현재 scope 기준 stamp 필요. code-generation 단계에서 명시. |
| **Toast 전달 순서** (ApiError보다 먼저) | ✅ 정당. UX 혼란 방지. | **Should-consider**: code-generation 단계에서 Toast emit을 Action reject와 동기 경로로. |
| **Toast level = Warning** (현재 Error 예비 결정) | ✅ 정당. 시스템 에러와 구분 필요. | **Should-consider**: 확정 — Warning level. |

### 4. 최종 Action Item 리스트

#### Must-Fix (blocking this BL)

1. **설계 전환**: `StampedAction` 래퍼 폐기 → 기존 `ActionSender` + `VersionedEvent<Action>` envelope에 `origin_project_id` 스탬프 통합. (C2 → 재설계, diff 55 site → ~5 파일)
2. **Type reference 교정**: application-design.md의 `TokenScope` / `ScopedAuthSession` 언급을 실제 타입으로 수정 — active_scope는 `Token.project.id` (port/types.rs ProjectScope) 또는 cached `RbacGuard.project_id`에서 온다.
3. **`is_mutation()` parity**: 기존 `worker.rs:151 action_to_kind()` 매핑과의 parity 테스트 강제. 두 exhaustive 매치가 일치하지 않으면 컴파일 실패.
4. **Glance DeleteImage/UpdateImage scope 검증**: FR4 form validator가 Glance 이미지 ID의 owner project_id를 선택 시점에 검증. FR1 전면 Glance 편입 대신 mutation path에만 scope 의무화 (scope 제한적 fix).
5. **Fingerprint canonicalization**: sha256 input에 명시적 delimiter 포함 (예: `"user|active|target|action|resource"`) + None은 빈 문자열로 명시. version prefix(`"v1:"`) 권장.
6. **Token refresh path의 project_id 보존**: refresh 이벤트가 `RbacGuard::update_roles(roles, None)`로 project_id를 덮지 않도록 방어. 명시적 `update_roles(roles, retain_project_id: bool)` 또는 별도 메서드 분리.
7. **FR1 response-side defense-in-depth**: adapter layer에서 응답 parsing 후 **client-side re-filter**로 scope 불일치 리소스 제거 + 단위 테스트. mock HTTP 없이 pure fn 기반 검증 가능.

#### Should-Consider (강력 권고)

1. **Audit schema 필드 추가**: `guard_layer`(fr1/fr2/fr3/fr4) + `correlation_id` (u64 epoch 또는 uuid).
2. **`target_project_id` 필드 rename**: origin-match 하에서 의미 모호 해소. `asserted_origin_project_id` 등.
3. **End-to-end mock integration test 1개**: form→ActionSender→worker→adapter 연쇄를 단일 시나리오로 merge-blocking. 기존 1370 tests 위에 1-2개 추가.
4. **Background task Action emit 경로 audit**: `poll_migration_progress` 등에서 mutation emit 시 현재 scope stamp 강제.
5. **Toast level = Warning** (예비 결정 확정).
6. **Toast emit 동기성**: Action reject와 Toast send가 동일 경로에서 (ApiError보다 먼저) 발행되도록 code-generation에서 확인.
7. **Actor user_id = Keystone UUID** (username 금지) 명시.

#### Future-BL (정당한 deferral)

1. **Glance visibility 전면 정합**: public/shared 이미지 scope 모델 전면 검토. 별도 BL 권고. (Codex 지지)
2. **Epoch 기반 worker stale-action drop**: cancellation hygiene 개선. BL-P2-086 후보. (Codex 원래 제기, BL 범위 밖)
3. **PII user_id 해싱 옵션**: 조직별 privacy 수준에 따라. 후속 옵션.
4. **HTTP mock server 도입 (mockito/wiremock)**: BL-P2-081 (dual-strategy cross-cloud auth) 착수 시 자연스럽게.
5. **Audit 뷰어 / 패턴 대시보드**: 스키마는 이번에 픽스됨.

### 합의된 설계 변경 요약

| 원래 (DETAIL) | 변경 후 |
|--------------|---------|
| `StampedAction { action, origin_project_id }` wrapper 신규 | **폐기**. `VersionedEvent<Action>` envelope에 `origin_project_id` 필드 추가. |
| 55 dispatch site 래핑 | `ActionSender::send()` 1곳에서 중앙 stamp |
| A1 type ref: `TokenScope`, `ScopedAuthSession` | `Token.project: ProjectScope{id,name,...}` + `RbacGuard::project_id()` (cached) |
| `target_project_id` (event field) | `asserted_origin_project_id` 또는 rename (origin-match 의미 명확) |
| Glance 전면 제외 (FR1+FR2+FR3) | FR1 제외 유지 + **Glance mutation path에 FR4 scope validation 의무화** |
| FR1 수용 기준 "응답측" = pure fn URL 빌더만 | 추가로 **adapter client-side re-filter** (defense-in-depth) 단위 테스트 |
| `is_mutation()` 단독 exhaustive match | `is_mutation()` + `action_to_kind()` parity 테스트 pair |
| Fingerprint = 단순 concat | version-prefixed + delimiter + Option 명시 규칙 |
| Refresh event project_id=None 덮어쓰기 | RbacGuard project_id 보존 가드 추가 |

### 리뷰 품질 평가

Codex가 실제 코드베이스에 깊숙이 들어가 existing infrastructure(`ActionSender`, `action_to_kind`)을 발견해 중복 설계를 방지. Gemini가 UX/운영자 관점에서 보조 제언. 두 리뷰 상호 보완. **Council Full 선택이 정당화됨** — single review(Claude만)였다면 ActionSender 중복 / A1 type error를 놓칠 가능성 실재.
