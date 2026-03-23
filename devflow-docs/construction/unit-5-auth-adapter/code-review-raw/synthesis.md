# Council Code Review Synthesis — Unit 5: auth-adapter

**Date**: 2026-03-23
**Chair**: Claude Opus 4.6
**Reviewers**: Claude (Spec), Codex/gpt-5.3 (Quality+Maintainability), Gemini 2.5 Pro (Security)

## Gate Decision: CONDITIONAL

| Stage | Reviewer | Status |
|-------|----------|--------|
| 1. Spec Compliance | Claude | CONDITIONAL PASS |
| 2. Code Quality | Codex | CONDITIONAL |
| 3. Security/Edge-case | Gemini | CONDITIONAL |
| 4. Maintainability | Codex | CONDITIONAL |

## Must-fix (consensus across 2+ reviewers)

### 1. Refresh loop idempotency guard (Codex HIGH + Gemini WARNING)
- authenticate() spawns new refresh task every call — no guard prevents duplicates
- **Fix**: Add `started: AtomicBool` or check existing handle before spawning

### 2. Thundering herd in get_token (Gemini CRITICAL + Codex RECOMMENDATION)
- Multiple concurrent callers hitting expired token all trigger refresh_token simultaneously
- **Fix**: Add Mutex or Notify around refresh path in get_token()

### 3. send_json error mapping (Claude SPEC + Codex implicit)
- Deserialization errors mapped to ApiError::Network instead of ApiError::Parse
- **Fix**: `.map_err(|e| ApiError::Parse(format!(...)))` like keystone.rs does

## Recommended (non-blocking, should-fix)

### 4. Endpoint cache not wired to token refresh (Gemini WARNING + Codex MEDIUM)
- invalidate_endpoint() exists but never called — stale endpoints after token refresh
- Phase 2 concern — document the intended wiring for now

### 5. Credential source consistency (Codex HIGH)
- authenticate() uses parameter, refresh uses self.credential — potential divergence
- Accept: App always passes same credential as new(). Add doc comment clarifying contract.

### 6. Narrow public API surface (Codex)
- build_auth_body → pub(crate), check_status → pub(crate)

### 7. authenticate_request empty headers (Codex MAINTAINABILITY HIGH)
- Phase 2 concern for signed auth. Phase 1 X-Auth-Token doesn't need headers/body.
- Accept for now, document limitation.

## Action Items for Merge

| # | Item | Priority | Effort |
|---|------|----------|--------|
| 1 | Refresh loop idempotency guard | MUST | Small |
| 2 | get_token single-flight (Mutex around refresh) | MUST | Small |
| 3 | send_json → ApiError::Parse | MUST | Trivial |
| 4-7 | Recommended items | SHOULD | Small-Medium |
