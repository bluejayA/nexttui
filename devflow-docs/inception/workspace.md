# Workspace Analysis

**Detected**: Brownfield
**Timestamp**: 2026-04-16T12:30:00+09:00
**Source**: 이전 분석(2026-04-07T12:45:00+09:00) 기반 + 델타 업데이트
**Project Root**: /Users/jay.ahn/projects/infra/nexttui
**Requires Path Confirmation**: false

## Project Structure
Rust TUI 애플리케이션. Component-Based + TEA 하이브리드 아키텍처, Port/Adapter 패턴, ModuleRegistry 기반. 17개 도메인 모듈(+HostModule), 125 .rs files, 1240 tests. PR#68에서 `src/context/` 모듈(switch orchestration) 추가.

## Key Files Found
- Cargo.toml, src/main.rs, src/lib.rs
- 125 .rs files across src/
- .github/workflows/ci.yml (CI: fmt, test, clippy, audit)
- rust-toolchain.toml, .git-blame-ignore-revs

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
- **Key Dependencies**: tokio, tokio-util (CancellationToken), reqwest, serde, tracing, chrono, async-trait, thiserror, http

## Git Activity
- **Last Commit**: 2026-04-16 — 프로젝트 활성
- **Recent Focus**: src/app.rs, src/worker.rs, src/module/server/mod.rs, src/module/floating_ip/mod.rs, devflow-docs/backlog.md
- **Recent Commits**: BL-P2-064 cargo audit CI (#73-#74), git-blame-ignore-revs (#72), git blame hygiene docs (#71), clippy -D warnings CI (#70), DevStack scripts (#69), BL-P2-031 switch-core PR1 (#68), Dark/Light theme (#66)

## Existing Documentation
- README.md: 프로젝트 개요
- CLAUDE.md (프로젝트 루트 외 ~/.claude/): 개발 규칙, 언어별 컨벤션, Git 보안 정책
- docs/git-blame-hygiene-in-ai-devflow.md: AI 협업 blame 위생 가이드

## Code Structure
- **Directory Layout**: src/ (app, component, context, models, module, adapter, port, ui, infra, input)
- **Entry Points**: src/main.rs, src/lib.rs
- **Observed Patterns**: src 레이아웃, Port/Adapter (src/port/ + src/adapter/), Module 기반 도메인 분리 (src/module/), Context Switch Orchestration (src/context/)
- **New since last analysis**: src/context/ (epoch, state_machine, switcher, resolver, cancellation, history, types, versioned, action_channel, capabilities, error), src/adapter/auth/{rescope.rs, scoped_session.rs}, src/adapter/http/endpoint_invalidator.rs, src/port/{context_session.rs, http_endpoint_cache.rs, keystone_rescope.rs, scoped_auth.rs, mock_context.rs}

## Coding Patterns (Sampled)
- **Source**: src/component.rs
- **Naming**: snake_case (Rust 표준)
- **Imports**: crate:: 절대 경로
- **Error Handling**: Result + thiserror, clippy deny unwrap/expect
- **Comments**: 영어 doc comments, 한국어 인라인 주석
