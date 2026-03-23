# Stage 2+4: Code Quality + Maintainability Review (Codex / gpt-5.3-codex)

## Stage 2: Code Quality
- Status: FAIL
- Issues:
  1. High: AuditLogger::log_entry returns Ok(()) even when write/flush/lock fails — silent audit loss (audit.rs L82-84)
  2. High: rotate_if_needed suppresses rename/reopen failures — partial rotation without error (audit.rs L104, L111-112)
  3. High: RBAC capability state stale across role updates — update_roles doesn't clear capabilities, has_capability prioritizes non-empty capabilities over role fallback (rbac.rs L43, L91)
- Recommendations:
  1. Propagate IO/lock errors in audit paths
  2. Update role/capability/project/admin atomically (single locked state struct)
  3. Fix stale doc comment on cache get() (says caller downcasts, but internal)
  4. Add TTL boundary test (> vs >=) for cache expiry
  5. Remove unused RequiredRole enum

## Stage 4: Maintainability
- Status: CONDITIONAL
- Issues:
  1. RBAC state split across multiple locks — no invariant boundary, race-prone (rbac.rs L25, L43)
- Recommendations:
  1. Centralize cache TTL mapping (Cache::new vs Config::cache_ttl duplication)
  2. Add module-level docs for RBAC precedence and catalog fallback policy
  3. Deduplicate endpoint/endpoint_in_region behind one internal helper

## Overall Assessment
Module structure and tests are solid. Stage 2 FAIL due to silent audit failures and RBAC state staleness.
