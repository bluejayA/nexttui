# Session Summary

**BL**: BL-P2-080
**Branch**: feat/bl-p2-080-keystone-project-directory
**Worktree**: .worktrees/bl-p2-080-keystone-project-directory
**Base commit**: aca622c (main)

## Current State
- **Phase**: CONSTRUCTION
- **Stage**: (pending — awaiting construction-orchestrator)
- **Commit**: aca622c
- **Complexity**: Standard
- **Approach**: A안 Design-First

## Completed Work

### INCEPTION
- [x] workspace-detection — Brownfield (Rust ratatui), 델타 모드
- [x] requirements-analysis — Standard, 9 FR + 5 NFR, Q1=C (domain lazy fallback)
- [x] pre-planning — C (skipped user-stories + nfr-requirements)
- [x] workflow-planning — A안 Design-First 선택
- [x] worktree-setup — feat/bl-p2-080-keystone-project-directory, 1329 tests passed
- [x] application-design LIST — 5 components (KeystoneProjectDirectory, DirectoryCache, TokenScopeFingerprint, DomainNameResolver, StaticProjectDirectory 강등)
- [x] application-design DETAIL — 4 iteration 완결 (R1 / R2 / Codex R3 / R4)

## Key Decisions

- **Q1 Domain 매핑**: C (lazy fallback — domain_id 1차 저장, ByName + domain 명시 매칭 실패 시에만 `/v3/domains/{id}` 호출)
- **D4 fallback 위치** (R3 재설계): `KeystoneProjectDirectory` 내부 → `ContextTargetResolver::disambiguate_by_name` 내부로 이동. Port 시그니처 불변
- **D2 invalidation** (R3 재설계): broadcast 채널 도입 포기. `App::handle_event(AppEvent::ContextChanged)` 블록에서 직접 `directory_cache.invalidate_cloud()` 호출. spawn task 제거
- **FR-4 concurrency** (R3 재설계): `ContextSwitcher::switch()` entry에 `entry_epoch = state.epoch().current()` snapshot → resolve 후 epoch drift 시 `InProgress` 반환. `SwitchError::InProgress`를 try_begin 경합과 resolve drift 두 경로가 공유 (의도된 선택)
- **D3 fingerprint**: `BuildHasherDefault<DefaultHasher>` zero-seed SipHasher13 (신규 dep 0, 동일 프로세스 내 stable)
- **D5 StaticProjectDirectory**: `#[cfg(test)] pub use` (design-closed, code 반영은 CONSTRUCTION)
- **CI gate**: `.github/workflows/ci.yml::devstack-integration` — image digest 고정, healthcheck, 실패 분류, ownership transition (BL-P2-081과 공유)
- **out of scope**: cross-cloud directory (→ BL-P2-081)

## Review Trail

- artifact-reviewer R1 → 5 blocker + 4 권고 → 모두 반영
- artifact-reviewer R2 → 3 비차단 권고 → 모두 반영
- Codex adversarial (round 1) → 스코프 빗나감 (focus file 미도달, working-tree diff만 봄) → audit.md format finding 1건만 유효
- Codex adversarial R3 → 5 findings (3 critical/high) → D4/D2/FR-4 재설계 완료
- artifact-reviewer R4 → 1 blocker (의존성 도식 미갱신) + 2 권고 → 도식 갱신 + InProgress 의미 통합 + migration 범위 표

## Next Steps

→ aidlc-construction-orchestrator 호출
→ units-generation (Minimal depth) → code-generation (Standard, TDD) → build-and-test (Standard, unit + integration + CI)
→ 완료 후 aidlc-finishing-a-development-branch로 머지/PR 진행

## Affected Code Paths (code-generation 범위 요약)

**신규 파일**:
- `src/adapter/auth/keystone_project_directory.rs`
- `src/adapter/auth/directory_cache.rs`
- `src/adapter/auth/token_scope_fingerprint.rs`
- `src/adapter/auth/keystone_domain_resolver.rs`

**변경 파일**:
- `src/context/resolver.rs` — `ContextTargetResolver::new` 시그니처 확장, `disambiguate_by_name` fallback 로직
- `src/context/switcher.rs` — `switch()` entry-epoch gate 5줄
- `src/context/static_project_directory.rs` — 주석 갱신
- `src/context/mod.rs:33` — `#[cfg(test)] pub use`
- `src/app.rs` — `directory_cache` field + `wire_directory_cache` + handle_event hook
- `src/main.rs` — wiring 변경
- `.github/workflows/ci.yml` — `devstack-integration` job

**`ContextTargetResolver::new` migration**: 20개 호출 site (prod 2 + test 18)
