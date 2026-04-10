# Workspace Analysis

**Detected**: Brownfield
**Timestamp**: 2026-04-07T12:45:00+09:00
**Source**: 이전 분석(2026-04-02T09:30:00+09:00) 기반 + 델타 업데이트
**Project Root**: /Users/jay.ahn/projects/infra/nexttui
**Requires Path Confirmation**: false

## Project Structure
Rust TUI 애플리케이션. Component-Based + TEA 하이브리드 아키텍처, Port/Adapter 패턴, ModuleRegistry 기반. 16개 도메인 모듈(+HostModule), 1017 tests.

## Key Files Found
- Cargo.toml, src/main.rs, src/lib.rs
- 94+ .rs files across src/

## Pre-specified Tech Stack
- **Source**: CLAUDE.md
- **Rust**: cargo clippy 필수
- **Test**: cargo test
- **Lint**: clippy (unwrap_used, expect_used, enum_glob_use = deny)

## Technology Stack
- **Language**: Rust (edition 2024)
- **Framework**: ratatui 0.30 + crossterm 0.29
- **Package Manager**: Cargo
- **Test Framework**: built-in (#[cfg(test)])
- **Key Dependencies**: tokio, reqwest, serde, tracing, chrono, async-trait, thiserror

## Git Activity
- **Last Commit**: 2026-04-06 — 프로젝트 활성
- **Recent Focus**: src/app.rs, src/worker.rs, src/module/server/mod.rs, src/event.rs, src/adapter/http/nova.rs
- **Recent Commits**: Visual Enhancement — scrollbar, content title (#60), HostModule — Composite Host Operations Panel (#59), help_hint() 14개 모듈 (#58), Activity Log + StatusBar (#57), Auto-Refresh Polling (#56)

## Existing Documentation
- README.md: 프로젝트 개요
- CLAUDE.md (프로젝트 루트 외 ~/.claude/): 개발 규칙, 언어별 컨벤션, Git 보안 정책

## Code Structure
- **Directory Layout**: src/ (app, component, models, module, adapter, port, ui, infra, input)
- **Entry Points**: src/main.rs, src/lib.rs
- **Observed Patterns**: src 레이아웃, Port/Adapter (src/port/ + src/adapter/), Module 기반 도메인 분리 (src/module/)

## Coding Patterns (Sampled)
- **Source**: src/component.rs
- **Naming**: snake_case (Rust 표준)
- **Imports**: crate:: 절대 경로
- **Error Handling**: Result + thiserror, clippy deny unwrap/expect
- **Comments**: 영어 doc comments, 한국어 인라인 주석
