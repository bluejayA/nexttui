# Workspace Analysis

**Detected**: Brownfield
**Timestamp**: 2026-03-26T15:00:00+09:00
**Project Root**: /Users/jay.ahn/projects/infra/nexttui
**Requires Path Confirmation**: false

## Project Structure
Rust TUI 애플리케이션 (nexttui). OpenStack 클라우드 인프라 관리용 터미널 UI.
Phase 1 완료 (16개 모듈, 563 tests), Phase 2 아키텍처 고도화 진행 중.

## Key Files Found
- `src/adapter/auth/keystone.rs` — Keystone v3 인증 어댑터 (토큰 관리 핵심)
- `src/config.rs` — clouds.yaml + config.toml 파싱
- `src/worker.rs` — Background worker (Action→API→Event)
- `src/port/types.rs` — Token, AuthCredential 등 도메인 타입
- `src/port/auth.rs` — AuthProvider trait

## Pre-specified Tech Stack
- **Source**: /Users/jay.ahn/CLAUDE.md
- **Language**: Rust (`cargo clippy` 필수)
- **Lints**: unwrap_used/expect_used/enum_glob_use deny
- **Test**: 기본 `cargo test`

## Technology Stack
- **Language**: Rust 2024 edition
- **Framework**: ratatui 0.30 (TUI), tokio (async runtime)
- **Package Manager**: Cargo
- **Test Framework**: built-in `#[test]` + `#[tokio::test]`
- **Key Dependencies**: reqwest (HTTP), serde/serde_json/serde_yaml (serialization), chrono (datetime), thiserror (error types), tracing (logging), crossterm (terminal I/O)

## Git Activity
- **Last Commit**: 2026-03-26 — 활발히 개발 중
- **Recent Focus**: src/main.rs, src/app.rs, src/demo.rs, src/worker.rs, src/registry.rs
- **Recent Commits**: tracing 도입, code quality foundation (clippy/non_exhaustive/pagination), RBAC wiring, Module Registry, form submit confirm

## Existing Documentation
- `README.md` — 프로젝트 개요, 설치, 아키텍처 설명
- `devflow-docs/` — Phase 1 INCEPTION/CONSTRUCTION 전체 산출물 (요구사항, 설계, 유닛, 빌드 기록)
- `devflow-docs/backlog.md` — Phase 2 백로그 (BL-P2-002 ~ P2-024, 신규 #30~#35)

## Code Structure
- **Directory Layout**: `src/{adapter, port, module, models, ui, infra, input}` — Hexagonal Architecture
- **Entry Points**: `src/main.rs` (binary), `src/lib.rs` (library)
- **Observed Patterns**: Port/Adapter (Hexagonal), Module Registry, Component trait

## Coding Patterns (Sampled)
- **Source**: src/adapter/auth/keystone.rs
- **Naming**: snake_case (Rust standard)
- **Imports**: crate-relative paths (`crate::port::auth::AuthProvider`)
- **Error Handling**: `Result<T, ApiError>` with `?` propagation, `#[non_exhaustive]` enums
- **Comments**: English doc comments, Phase 2 TODO notes inline
