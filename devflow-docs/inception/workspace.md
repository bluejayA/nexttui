# Workspace Analysis

**Detected**: Brownfield
**Timestamp**: 2026-04-18T22:40:00+09:00
**Source**: 이전 분석(2026-04-16T12:30:00+09:00) 기반 + 델타 업데이트
**Project Root**: /Users/jay.ahn/projects/infra/nexttui
**Requires Path Confirmation**: false

## Project Structure
Rust TUI 애플리케이션. Component-Based + TEA 하이브리드 아키텍처, Port/Adapter 패턴, ModuleRegistry 기반. 17개 도메인 모듈(+HostModule), 128 .rs files, 1314 tests (PR#76 기준). PR#68에서 `src/context/` switch orchestration 추가, PR#75에서 main.rs runtime wire, PR#76에서 Commands & Safety UI (Unit 4.5 + Unit 5) 완료.

## Key Files Found
- Cargo.toml, src/main.rs, src/lib.rs
- 128 .rs files across src/
- .github/workflows/ci.yml (CI: fmt, test, clippy, audit)
- rust-toolchain.toml, .git-blame-ignore-revs

## Pre-specified Tech Stack
- **Source**: ~/CLAUDE.md (프로젝트 루트에는 CLAUDE.md 없음)
- **Rust**: cargo clippy 필수
- **Test**: cargo test
- **Lint**: clippy (unwrap_used, expect_used, enum_glob_use = deny)

## Technology Stack
- **Language**: Rust (edition 2024)
- **Framework**: ratatui 0.30 + crossterm 0.29
- **Package Manager**: Cargo
- **Test Framework**: built-in (#[cfg(test)])
- **Key Dependencies**: tokio, tokio-util (CancellationToken), reqwest, serde, tracing, chrono, async-trait, thiserror, http, **unicode-width 0.2** (신규 — PR#76 BL-P2-077)

## Git Activity
- **Last Commit**: 2026-04-18 — 프로젝트 활성
- **Recent Focus**: src/app.rs, src/context/*, src/ui/command_bar*, src/ui/context_indicator*, src/ui/confirm.rs, devflow-docs/backlog.md
- **Recent Commits**: chore(hooks) devflow-guard loosen (3735929), PR#76 머지 후 state 정리 (e7216ce), **PR#76 BL-P2-031 PR3 Commands & Safety UI** (d76e578), **PR#75 BL-P2-031 T3 runtime wire** (a00c044), BL-P2-064 backlog mark (82e9dc4)

## Existing Documentation
- README.md: 프로젝트 개요
- CLAUDE.md (~/.claude/ — 글로벌): 개발 규칙, 언어별 컨벤션, Git 보안 정책
- docs/git-blame-hygiene-in-ai-devflow.md: AI 협업 blame 위생 가이드

## Code Structure
- **Directory Layout**: src/ (app, component, context, models, module, adapter, port, ui, infra, input)
- **Entry Points**: src/main.rs, src/lib.rs
- **Observed Patterns**: src 레이아웃, Port/Adapter (src/port/ + src/adapter/), Module 기반 도메인 분리 (src/module/), Context Switch Orchestration (src/context/)
- **New since 2026-04-16 analysis**: PR#75 — main.rs `wire_production_mode` 영역 확장 (switch-core 실제 연결). PR#76 — src/ui/command_bar.rs + command_bar_table.rs (Unit 4.5), src/ui/context_indicator.rs (Unit 5 Step 2), src/ui/confirm.rs 확장 (TypeToConfirm + fingerprint), src/ui/input_bar.rs 리팩터 (InputMode 단일화 BL-P2-073), src/app.rs Command parser/executor + SwitchCloud stub

## Coding Patterns (Sampled)
- **Source**: src/component.rs
- **Naming**: snake_case (Rust 표준)
- **Imports**: crate:: 절대 경로
- **Error Handling**: Result + thiserror, clippy deny unwrap/expect
- **Comments**: 영어 doc comments, 한국어 인라인 주석

## BL-P2-074 관련 현재 상태
- `:switch-cloud <name>` 파서: `Command::SwitchCloud(String)` 생성 (src/input/command.rs)
- 실행부: src/app.rs:1771-1780 — toast-only stub (ContextRequest에 CloudOnly variant 부재)
- ContextRequest (src/context/types.rs:19): ByName / ById 두 variant, 둘 다 `project: String` 필수
- ContextTargetResolver::list_projects(cloud) (src/context/resolver.rs:91) — 이미 존재, 옵션 (a)/(b) 공통 재사용 가능
- Unit 6 ContextPicker: **미구현** (src/ui/에 없음, app.rs에서 "(picker: Ctrl+P — Unit 6)" toast로 announce만)
