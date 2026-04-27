# Workspace Analysis

**Detected**: Brownfield
**Timestamp**: 2026-04-24T10:35:00+09:00
**Source**: 이전 분석(2026-04-18T22:40:00+09:00) 기반 + 델타 업데이트 (PR#78/#80/#81/#82 반영)
**Project Root**: /Users/jay.ahn/projects/infra/nexttui
**Requires Path Confirmation**: false

## Project Structure
Rust TUI 애플리케이션. Component-Based + TEA 하이브리드 아키텍처, Port/Adapter 패턴, ModuleRegistry 기반. 17개 도메인 모듈(+HostModule), **132 .rs files**, **1370 tests** (PR#82 기준). 최근: PR#78 SwitchCloud wire, PR#80 same-cloud HTTP ProjectDirectory, PR#82 BL-P2-085 hotfix.

## Key Files Found
- Cargo.toml, src/main.rs, src/lib.rs
- 132 .rs files across src/
- .github/workflows/ci.yml (CI: fmt, test, clippy, audit, devstack-integration placeholder)
- rust-toolchain.toml, .git-blame-ignore-revs
- tests/devstack_directory.rs (integration test)

## Pre-specified Tech Stack
- **Source**: ~/CLAUDE.md (프로젝트 루트에는 CLAUDE.md 없음)
- **Rust**: cargo clippy 필수
- **Test**: cargo test
- **Lint**: clippy (unwrap_used, expect_used, enum_glob_use = deny)

## Technology Stack
- **Language**: Rust (edition 2024)
- **Framework**: ratatui 0.30 + crossterm 0.29
- **Package Manager**: Cargo
- **Test Framework**: built-in (#[cfg(test)]) + integration (tests/)
- **Key Dependencies**: tokio, tokio-util (CancellationToken), reqwest, serde, tracing, chrono, async-trait, thiserror, http, unicode-width 0.2

## Git Activity
- **Last Commit**: 2026-04-24 — 프로젝트 활성
- **Recent Focus**: src/app.rs, src/module/server/mod.rs, src/module/user/mod.rs, src/module/project/mod.rs, src/module/host/mod.rs, devflow-docs/*
- **Recent Commits** (top 5):
  - c4590ab PR#82 BL-P2-085 hotfix: server dropdown cache + cross-project disambiguation
  - c0d20e2 PR#81 chore: devflow session-scoped artifacts gitignored
  - 733d88f PR#80 BL-P2-080: same-cloud HTTP ProjectDirectory via /v3/auth/projects
  - aca622c PR#79 chore(backlog) BL-P2-074 완료 마킹
  - af03fd9 PR#78 BL-P2-074 SwitchCloud wire

## Existing Documentation
- README.md: 프로젝트 개요
- CLAUDE.md (~/.claude/ — 글로벌): 개발 규칙, 언어별 컨벤션, Git 보안 정책
- docs/git-blame-hygiene-in-ai-devflow.md: AI 협업 blame 위생 가이드

## Code Structure
- **Directory Layout**: src/ (app, component, context, models, module, adapter, port, ui, infra, input, router, event_loop, worker, background, action, demo, registry)
- **Entry Points**: src/main.rs, src/lib.rs
- **Observed Patterns**: src 레이아웃, Port/Adapter (src/port/ + src/adapter/), Module 기반 도메인 분리 (src/module/), Context Switch Orchestration (src/context/), 워커 기반 mutation (src/worker.rs)
- **Adapter 레이아웃 (중요 — 이전 분석 기록 정정)**:
  - `src/adapter/auth/` — Keystone auth/project directory/domain resolver/token cache/scoped session/rescope (10개 파일)
  - `src/adapter/http/` — OpenStack API 어댑터: `keystone.rs`, `neutron.rs`, `nova.rs`, `cinder.rs`, `glance.rs`, `base.rs`, `endpoint_invalidator.rs`, `mod.rs`
  - **⚠️ BL-P2-085 실제 대상은 `src/adapter/http/*.rs`** (인자에 있던 `src/adapter/openstack/*`는 부정확한 경로. inception에서 spec화 시 교정 필요)

## Coding Patterns (Sampled)
- **Source**: src/component.rs
- **Naming**: snake_case (Rust 표준)
- **Imports**: crate:: 절대 경로
- **Error Handling**: Result + thiserror, clippy deny unwrap/expect
- **Comments**: 영어 doc comments, 한국어 인라인 주석

## BL-P2-085 관련 현재 상태 (2026-04-24 추가)

### 건드릴 주요 파일 (크기 / 역할)
| 파일 | LoC | 현재 역할 | 예상 변경 축 |
|------|-----|----------|-----------|
| `src/adapter/http/neutron.rs` | 849 | SG / Network / FloatingIP List 빌더 | `tenant_id` 필터 auth scope 주입 (Critical) |
| `src/adapter/http/nova.rs` | 1147 | 서버/플레이버/키페어 API | `all_tenants` 플래그 모델 통일 (Critical) |
| `src/adapter/http/cinder.rs` | 567 | 볼륨/스냅샷/백업 API | `all_tenants` 통일 + form-selected ID 검증 (High) |
| `src/worker.rs` | 1269 | Action dispatch / mutation 실행 | 모든 mutation 직전 `target.project_id == active_scope` 가드 (Critical) |
| `src/infra/rbac.rs` | — | 역할 기반 정책 | project-mismatch 차단 정책 추가 (Critical) |
| `src/module/server/mod.rs` (PR#82) | — | 서버 폼 드롭다운 | `build_disambiguated_opts` 헬퍼 — **재사용 검토** (다른 module 폼에도 확장 가능) |

### 테스트 스캐폴드
- `tests/devstack_directory.rs` — PR#80이 도입한 통합 테스트. BL-P2-085 회귀 테스트(두 project에 동명 SG 배치 시나리오)는 이 파일 또는 신규 `tests/devstack_cross_project.rs`에 추가 예상.

### 관련 타입
- `active_scope` 참조 지점: main.rs, app.rs, context/state_machine, context/switcher, context/history, context/resolver, adapter/auth/*
- `TokenScopeFingerprint` (adapter/auth/token_scope_fingerprint.rs) — Unit 1 결과물, project 범위 식별자
