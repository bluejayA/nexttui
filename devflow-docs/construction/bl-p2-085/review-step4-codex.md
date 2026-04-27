# Codex Review — Phase 3 Step 4 (BL-P2-085)

- **시각**: 2026-04-27
- **대상**: `feature/bl-p2-085-cross-project-scoping` vs `ca2ec2a` (Step 4 단독 commit `e68d50b`)
- **scope**: `--scope branch --base ca2ec2a`
- **diff 규모**: 7 files, +336 / -13 (소스 1 신규 + Cargo.toml/lock + mod.rs + docs)
- **결과**: P2 1건 — 즉시 반영 완료
- **gate 처리**: 통과. Phase 4 Step 5 진입 가능.

## Findings

### [P2] Rotate audit log after successful cross-project emits

`/src/infra/cross_project_audit.rs:92`

> When `logger.log_entry(...)` succeeds, `emit` returns immediately and never invokes `rotate_if_needed`, so this new audit path bypasses the existing size/retention policy (`MAX_LOG_SIZE`, `MAX_ROTATED_FILES`). In environments where cross-project blocks occur repeatedly, `audit.log` can grow without rotation even though normal app audit writes do rotate; adding a best-effort rotation call after successful writes keeps behavior consistent and prevents unbounded log growth.

**검증**: `app.rs:788-793`이 정확히 `logger.log_entry(entry)` 직후 `logger.rotate_if_needed()`를 호출하는 패턴이 확립되어 있음을 grep으로 확인. 우리 emit은 success 시 early return으로 rotation skip — 명백한 inconsistency.

**처리**: 즉시 반영. emit() success branch에 rotate_if_needed() 호출 추가. tracing::warn fallback. 회귀 0 (1390 stable).

### Codex 검사 흔적

- `git diff ca2ec2a151c9652c06dbb408ba27386ba02d50e8`
- `sed src/infra/audit.rs` (rotation API 시그니처 확인)
- `sed src/infra/cross_project_guard.rs`
- `rg "cross_project_audit|CrossProjectBlockEvent|emit("`
- `rg "AuditResult::Failed|log_entry(|emit(|fingerprint|correlation_id"`
- `sed src/app.rs:720,860` (기존 record_audit 패턴 비교) — 핵심 finding의 근거
- `rg "log_entry("` (호출자 매트릭스)
- `nl src/infra/cross_project_audit.rs:80,130` (emit 본문)
- `sed src/lib.rs` (모듈 등록)

**메타**: codex가 audit.rs의 별도 `rotate_if_needed` API 존재를 확인하고, 기존 호출자(app.rs:780)의 패턴과 비교한 다음 구조적 inconsistency를 잡아냄. fingerprint v1 canonical schema 결정, sha2 dep 추가, AuditResult 매핑 등은 모두 통과 (P0/P1 0건).
