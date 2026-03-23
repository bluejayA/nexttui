# Council Code Review Synthesis — Unit 4: infrastructure

**Date**: 2026-03-23
**Chair**: Claude Opus 4.6
**Reviewers**: Claude (Spec), Codex/gpt-5.3 (Quality+Maintainability), Gemini 2.5 Pro (Security)

## Gate Decision: CONDITIONAL

## Stage Summary

| Stage | Reviewer | Status |
|-------|----------|--------|
| 1. Spec Compliance | Claude | CONDITIONAL PASS |
| 2. Code Quality | Codex | FAIL |
| 3. Security/Edge-case | Gemini | CONDITIONAL |
| 4. Maintainability | Codex | CONDITIONAL |

## Cross-cutting Issues (consensus)

### Must-fix (2+ reviewers agree, blocking)

1. **AuditLogger silent failures** (Codex HIGH + Gemini WARNING)
   - `log_entry()`: write/flush/lock errors silently ignored, returns Ok(())
   - `rotate_if_needed()`: rename/reopen errors suppressed
   - **Fix**: Propagate errors from Mutex::lock, writeln!, flush

2. **RBAC capability staleness** (Codex HIGH)
   - `update_roles()` doesn't clear capabilities → stale capabilities override role fallback
   - **Fix**: Clear capabilities in `update_roles()` so Phase 1 role fallback works correctly

3. **Unbounded cache growth** (Gemini CRITICAL)
   - No max entries limit → memory DoS via many unique qualifiers
   - **Fix**: Add `max_entries` parameter, evict expired entries on put when limit reached

### Recommended (non-blocking, follow-up)

4. **RbacGuard atomic state** (Codex MAINTAINABILITY)
   - 4 separate RwLocks → inconsistent state possible between locks
   - Consolidate into single `RwLock<RbacState>` struct

5. **Remove unused RequiredRole** (Codex DEAD CODE)
   - Defined but never used in any production path

6. **Cache doc comment stale** (Codex)
   - get() comment says "caller must downcast" but downcast is internal

7. **Catalog endpoint dedup** (Codex MAINTAINABILITY)
   - `endpoint()` and `endpoint_in_region()` share logic, extract common helper

8. **AuditLogger log_result** (Claude SPEC)
   - Spec defines 2-phase logging (Initiated→Success/Failed), only single log_entry exists

## Action Items for Merge

| # | Item | Priority | Effort |
|---|------|----------|--------|
| 1 | AuditLogger: propagate write/lock errors | MUST | Small |
| 2 | RbacGuard: clear capabilities on update_roles | MUST | Small |
| 3 | Cache: add max_entries bound | MUST | Small |
| 4-8 | Recommended items | SHOULD | Medium |
