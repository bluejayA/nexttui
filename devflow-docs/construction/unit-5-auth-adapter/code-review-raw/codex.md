# Stage 2+4: Code Quality + Maintainability Review (Codex / gpt-5.3-codex)

## Stage 2: Code Quality — CONDITIONAL
### Issues (blocking)
1. HIGH: Unbounded/duplicated refresh task lifecycle — each authenticate() starts new tokio::spawn with no idempotency guard or shutdown path
2. HIGH: Credential source inconsistency — authenticate() uses parameter, refresh uses self.credential
3. MEDIUM: try_lock() can orphan refresh handles

### Recommendations (non-blocking)
1. Single-flight refresh protection in get_token()
2. Expand tests for refresh-loop behavior
3. check_status: richer context for 404/503/429 placeholders

## Stage 4: Maintainability — CONDITIONAL
### Issues (blocking)
1. HIGH: authenticate_request() called with empty headers/None body — limits future signed auth methods
2. MEDIUM: Endpoint cache invalidation not wired to token-refresh events

### Recommendations (non-blocking)
1. Narrow public API (build_auth_body, check_status → pub(crate))
2. Stronger typing for service_type (enum vs String)
3. Explicit refresh task ownership/cancellation strategy
