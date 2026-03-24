# Workspace Analysis

**Detected**: Brownfield
**Timestamp**: 2026-03-24T22:30:00+09:00
**Project Root**: /Users/jay.ahn/projects/infra/nexttui
**Requires Path Confirmation**: false

## Project Structure
Rust TUI application for OpenStack management. Phase 1 complete: 91 Rust files, 534 tests, 16 domain modules. Port/Adapter hexagonal architecture. FormWidget fully implemented with FieldDef/FieldState, render, module integration, dynamic dropdown options.

## Key Files Found
- `src/app.rs` — App orchestrator (FocusPane, InputMode, component HashMap)
- `src/module/mod.rs` — ViewState, PendingAction, ConfirmHandler (shared module patterns)
- `src/module/*/mod.rs` — 16 domain modules implementing Component trait
- `src/port/*.rs` — 6 Port traits (Auth, Nova, Neutron, Cinder, Glance, Keystone)
- `src/adapter/registry.rs` — AdapterRegistry (Arc<dyn Port> management)
- `src/infra/cache.rs` — Cache (RwLock, TTL, GC)
- `src/ui/sidebar.rs` — Sidebar with hardcoded SidebarItems
- `src/models/common.rs` — Route enum (23 variants), ResourceType enum

## Technology Stack
- **Language**: Rust (edition 2024)
- **Framework**: ratatui 0.30 + crossterm 0.29
- **Package Manager**: Cargo
- **Test Framework**: built-in (#[test]), 534 tests passing
- **Key Dependencies**: tokio 1 (async), reqwest 0.12 (HTTP), serde 1 (serialization), chrono 0.4, uuid 1, async-trait 0.1, thiserror 2

## Git Activity
- **Last Commit**: 2026-03-24 — fix: use LightBlue for detail view key labels
- **Recent Focus**: src/ui/form.rs, src/module/server/mod.rs, src/ui/detail_view.rs, src/module/floating_ip/mod.rs
- **Recent Commits**: detail view UI fixes, dropdown option caching, FormField→FieldDef migration, form-widget PR merge

## Existing Documentation
- `CLAUDE.md` — Jay의 개발 규칙 (TDD, 승인 후 진행, 언어별 컨벤션)
- `devflow-docs/inception/` — Phase 1 INCEPTION 전체 산출물 (requirements, user-stories 48개, nfr, application-design 52개 컴포넌트, detail-design 4개 문서)
- `devflow-docs/inception/agent-council-review.md` — Codex+Gemini+Claude 3자 리뷰 결과
- `docs/plans/2026-03-18-async-event-architecture-design.md` — 비동기 이벤트 아키텍처 설계

## Code Structure
- **Directory Layout**: `src/{adapter,infra,input,models,module,port,ui}` + `src/main.rs`
- **Entry Points**: `src/main.rs` (tokio::main), `src/lib.rs`
- **Observed Patterns**: Port/Adapter (src/port + src/adapter), Component-Based (src/module/*/mod.rs implements Component trait), TEA hybrid (Action/Event channels)
- **Module Count**: 16 (server, flavor, network, security_group, floating_ip, volume, snapshot, image, project, user, aggregate, compute_service, hypervisor, agent, migration)

## Coding Patterns (Sampled)
- **Source**: src/component.rs (33 lines)
- **Naming**: snake_case (Rust standard)
- **Imports**: crate-relative (`use crate::action::Action`)
- **Error Handling**: Option/Result pattern, thiserror for custom errors
- **Comments**: English code comments, Korean in docs/CLAUDE.md

## Reference Analysis
- **Substation** (Swift 6.2 OpenStack TUI): `/Users/jay.ahn/workspaces/substation/reverse-engineering/README.md`
- Key patterns to adopt: Module Registry (OpenStackModule protocol + Phase-based loading), Intelligent Cache Invalidation (dependency graph), DataProvider Registry, Adaptive polling
