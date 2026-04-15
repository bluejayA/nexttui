Reading additional input from stdin...
OpenAI Codex v0.118.0 (research preview)
--------
workdir: /Users/jay.ahn/projects/infra/nexttui/.worktrees/runtime-context-switch
model: gpt-5.3-codex
provider: openai
approval: never
sandbox: read-only
reasoning effort: high
reasoning summaries: none
session id: 019d85a9-344f-7423-b08b-1e8bd0e739e9
--------
user
You are reviewing an application-design.md (component LIST) for a Rust+ratatui TUI feature.

PROJECT: nexttui — OpenStack admin TUI. Existing CommandParser, Port/Adapter pattern, ModuleRegistry.
FEATURE: BL-P2-031 — Runtime cloud/project context switching via Keystone rescoping.

REVIEW THIS FILE: devflow-docs/inception/application-design.md
ALSO CONSIDER: devflow-docs/inception/requirements.md (10 FR + 5 NFR), devflow-docs/inception/workspace.md.

Prior Codex adversarial review already shaped these requirements:
- ContextEpoch + CancellationRegistry for stale-event isolation
- Switch state machine with atomic rollback
- Keystone rescope adapter + service catalog invalidation
- Destructive confirm fingerprint + persistent context indicator
- Phased PR1~PR6 merge strategy

REVIEW SCOPE — assess the LIST for:
1. **Completeness** — Are any required components missing? (cross-cutting concern coverage)
2. **Boundary clarity** — Component responsibilities crisp? Any overlap or unclear ownership?
3. **PR mapping risk** — Are PR1~PR6 boundaries safe? Any PR ships an unsafe state?
4. **Naming** — Are component names accurate to their responsibility and consistent with codebase conventions?
5. **Type classification** — Util/Service/Adapter/Controller/Repository assignments correct?
6. **Hidden coupling** — Any pair of components that should be merged or split?
7. **Test seams** — Will components be unit-testable in isolation?

DELIVER:
- Top 3 issues (each: what + why it matters + concrete suggestion)
- Components to add / remove / merge / split (be specific)
- PR boundary concerns
- Verdict: APPROVE | APPROVE-WITH-CHANGES | REJECT
- Brief reasoning

Be concise. No hedging.
codex
I’ll inspect the three inception docs first and extract the component list plus FR/NFR and workspace constraints, then I’ll assess gaps and PR-safety against the prior adversarial requirements you listed.
exec
/bin/zsh -lc 'ls -la' in /Users/jay.ahn/projects/infra/nexttui/.worktrees/runtime-context-switch
 succeeded in 0ms:
total 240
drwxr-xr-x@ 16 jay.ahn  staff    512 Apr 13 15:54 .
drwxr-xr-x@  3 jay.ahn  staff     96 Apr 13 15:54 ..
drwxr-xr-x@  3 jay.ahn  staff     96 Apr 13 15:54 .claude
-rw-r--r--@  1 jay.ahn  staff     84 Apr 13 15:54 .git
-rw-r--r--@  1 jay.ahn  staff    256 Apr 13 15:54 .gitignore
-rw-r--r--@  1 jay.ahn  staff  81636 Apr 13 15:54 Cargo.lock
-rw-r--r--@  1 jay.ahn  staff    752 Apr 13 15:54 Cargo.toml
-rw-r--r--@  1 jay.ahn  staff   1064 Apr 13 15:54 LICENSE
-rw-r--r--@  1 jay.ahn  staff   6634 Apr 13 15:54 README.en.md
-rw-r--r--@  1 jay.ahn  staff   6941 Apr 13 15:54 README.md
-rw-r--r--@  1 jay.ahn  staff     58 Apr 13 15:54 clippy.toml
drwxr-xr-x@ 21 jay.ahn  staff    672 Apr 13 15:54 devflow-docs
-rw-r--r--@  1 jay.ahn  staff   2864 Apr 13 15:54 devstack-vm.yaml
drwxr-xr-x@  4 jay.ahn  staff    128 Apr 13 15:54 docs
drwxr-xr-x@ 23 jay.ahn  staff    736 Apr 13 15:54 src
drwxr-xr-x@  5 jay.ahn  staff    160 Apr 13 15:55 target

exec
/bin/zsh -lc 'rg -n "''^#|''^##|''^###|FR|NFR|PR[1-6]|ContextEpoch|CancellationRegistry|rescope|switch|component|Adapter|Service|Controller|Repository" devflow-docs/inception/application-design.md devflow-docs/inception/requirements.md devflow-docs/inception/workspace.md' in /Users/jay.ahn/projects/infra/nexttui/.worktrees/runtime-context-switch
 succeeded in 0ms:
devflow-docs/inception/requirements.md:1:# Requirements Analysis
devflow-docs/inception/requirements.md:6:## User Intent
devflow-docs/inception/requirements.md:9:## Scope 분석
devflow-docs/inception/requirements.md:22:## Functional Requirements
devflow-docs/inception/requirements.md:24:### ServerViewContext
devflow-docs/inception/requirements.md:25:- FR-01: ServerViewContext 구조체 도입 — server, migration_progress, flavor, is_resize_pending, cached_volumes, cached_floating_ips 필드
devflow-docs/inception/requirements.md:26:- FR-02: server_detail_data_full(7 params) → server_detail_data(ctx: &ServerViewContext) 변경
devflow-docs/inception/requirements.md:27:- FR-03: server_to_row_full(3 params)은 유지 — 파라미터가 적고 ViewContext에 넣기 부자연스러움
devflow-docs/inception/requirements.md:28:- FR-04: server_detail_data(server) 편의 함수 유지 (빈 ViewContext로 위임)
devflow-docs/inception/requirements.md:29:- FR-05: ServerModule의 render에서 ViewContext 생성 → 전달
devflow-docs/inception/requirements.md:31:### FipViewContext
devflow-docs/inception/requirements.md:32:- FR-06: FipRowContext 구조체 도입 — show_tenant, cached_servers, cached_ports 필드
devflow-docs/inception/requirements.md:33:- FR-07: fip_to_row_with_servers(4 params) → fip_to_row(fip, ctx: &FipRowContext) 변경
devflow-docs/inception/requirements.md:34:- FR-08: FloatingIpModule의 rows()에서 FipRowContext 생성 → 전달
devflow-docs/inception/requirements.md:36:### 정리 — _full/_with_servers 패턴 통합
devflow-docs/inception/requirements.md:37:- FR-09: server_detail_data / server_detail_data_full 2개 함수 → server_detail_data(ctx) 1개로 통합
devflow-docs/inception/requirements.md:38:- FR-10: volume_detail_data / volume_detail_data_with_servers → 그대로 유지 (파라미터 2개로 충분)
devflow-docs/inception/requirements.md:39:- FR-11: fip_to_row / fip_to_row_with_servers 2개 → fip_to_row(fip, ctx) 1개로 통합
devflow-docs/inception/requirements.md:41:## Non-Functional Requirements
devflow-docs/inception/requirements.md:42:- NFR-01: 기존 1108 tests 전체 통과 (리팩토링이므로 기능 회귀 0건 필수)
devflow-docs/inception/requirements.md:43:- NFR-02: 외부 API 동작 변경 없음 — 렌더링 결과 동일
devflow-docs/inception/requirements.md:44:- NFR-03: 테스트에서 ViewContext builder 또는 Default 사용 가능하도록 설계
devflow-docs/inception/requirements.md:46:## Assumptions
devflow-docs/inception/requirements.md:51:## Open Questions
devflow-docs/inception/workspace.md:1:# Workspace Analysis
devflow-docs/inception/workspace.md:9:## Project Structure
devflow-docs/inception/workspace.md:10:Rust TUI 애플리케이션. Component-Based + TEA 하이브리드 아키텍처, Port/Adapter 패턴, ModuleRegistry 기반. 16개 도메인 모듈(+HostModule), 1017 tests.
devflow-docs/inception/workspace.md:12:## Key Files Found
devflow-docs/inception/workspace.md:16:## Pre-specified Tech Stack
devflow-docs/inception/workspace.md:22:## Technology Stack
devflow-docs/inception/workspace.md:29:## Git Activity
devflow-docs/inception/workspace.md:34:## Existing Documentation
devflow-docs/inception/workspace.md:38:## Code Structure
devflow-docs/inception/workspace.md:39:- **Directory Layout**: src/ (app, component, models, module, adapter, port, ui, infra, input)
devflow-docs/inception/workspace.md:41:- **Observed Patterns**: src 레이아웃, Port/Adapter (src/port/ + src/adapter/), Module 기반 도메인 분리 (src/module/)
devflow-docs/inception/workspace.md:43:## Coding Patterns (Sampled)
devflow-docs/inception/workspace.md:44:- **Source**: src/component.rs
devflow-docs/inception/application-design.md:1:# Application Design
devflow-docs/inception/application-design.md:7:## 컴포넌트 목록
devflow-docs/inception/application-design.md:9:### 신규 컴포넌트 (7개)
devflow-docs/inception/application-design.md:13:| `ContextEpoch` | App-level epoch 카운터로 이전 컨텍스트의 stale 이벤트 격리 | Util | PR1 |
devflow-docs/inception/application-design.md:14:| `CancellationRegistry` | 활성 폴링/장기 fetch에 대한 `CancellationToken` 등록·취소 | Service | PR1 |
devflow-docs/inception/application-design.md:15:| `SwitchStateMachine` | `Idle → Switching → Committed | Failed` 전환 상태 및 rollback 규약 | Service | PR2 |
devflow-docs/inception/application-design.md:16:| `ContextSwitcher` | 전환 절차(epoch++ → cancel → rescope → catalog 무효화 → commit) 오케스트레이터 | Service | PR2 |
devflow-docs/inception/application-design.md:17:| `KeystoneRescopeAdapter` | Keystone v3 token-method scoped exchange 호출, expires_at 정본 사용 | Adapter | PR2 |
devflow-docs/inception/application-design.md:18:| `ContextPicker` | Ctrl+P 모달, 프로젝트/클라우드 fuzzy 검색·선택 UI (`cloud • project • domain • project_id` 행 표시) | Controller | PR4 |
devflow-docs/inception/application-design.md:19:| `ContextIndicator` | 영구 컨텍스트 표시 위젯 (cloud/project/domain), 전환 직후 강조 | Controller | PR5 |
devflow-docs/inception/application-design.md:21:### 변경 컴포넌트 (8개)
devflow-docs/inception/application-design.md:25:| `App` (src/app.rs) | 활성 cloud/project 컨텍스트 상태 관리, ContextSwitcher 통합, epoch 보유 | Service | PR2/PR3 |
devflow-docs/inception/application-design.md:26:| `Worker` (src/worker.rs) | 모든 `tokio::spawn` 폴링/장기 fetch에 epoch 검증 + `tokio::select!` cancel branch 추가 | Service | PR1 |
devflow-docs/inception/application-design.md:27:| `CommandRegistry` (src/input/command.rs) | `:switch-project <name|uuid|cloud/project>`, `:switch-cloud`, `:switch-back` 등록 + 충돌 disambiguation + tab 자동완성 | Controller | PR3 |
devflow-docs/inception/application-design.md:28:| `KeyMap` (src/input/keymap.rs) | 글로벌 단축키 (예: Ctrl+P) → ContextPicker 오픈, Identity 리스트의 `s` → 컨텍스트 전환 | Controller | PR4/PR6 |
devflow-docs/inception/application-design.md:29:| `ConfirmDialog` (src/ui/confirm.rs) | destructive 액션 confirm에 `cloud • project` fingerprint 표시, 컨텍스트 변경 직후엔 추가 확인 강제 | Controller | PR5 |
devflow-docs/inception/application-design.md:30:| `StatusBar` (src/ui/status_bar.rs) | ContextIndicator 임베드 위치/우선순위 조정 | Controller | PR5 |
devflow-docs/inception/application-design.md:31:| `Project Module` (src/module/project/mod.rs) | 리스트 행 `s` 액션 핸들러 (Identity 진입점), Enter는 Detail 유지 | Controller | PR6 |
devflow-docs/inception/application-design.md:32:| `TokenCacheStore` (src/adapter/auth/token_cache.rs) | rescoped 토큰을 scope별로 저장/조회 (BL-P2-029 기반), 만료 시 재취득 경로 | Repository | PR2 |
devflow-docs/inception/application-design.md:36:## PR 매핑 요약
devflow-docs/inception/application-design.md:40:| PR1 (선행 인프라) | ContextEpoch, CancellationRegistry, Worker 수정 |
devflow-docs/inception/application-design.md:41:| PR2 (전환 코어) | SwitchStateMachine, ContextSwitcher, KeystoneRescopeAdapter, App 통합, TokenCacheStore 확장 |
devflow-docs/inception/application-design.md:42:| PR3 (명령 UX) | CommandRegistry 확장 |
devflow-docs/inception/application-design.md:43:| PR4 (피커 UI) | ContextPicker, KeyMap (Ctrl+P) |
devflow-docs/inception/application-design.md:44:| PR5 (안전 가시성) | ContextIndicator, StatusBar, ConfirmDialog |
devflow-docs/inception/application-design.md:45:| PR6 (Identity 통합) | Project Module `s` 액션, KeyMap (`s`) |
devflow-docs/inception/application-design.md:47:## NFR 매핑 요약
devflow-docs/inception/application-design.md:49:| NFR | 보장 컴포넌트 |
devflow-docs/inception/application-design.md:51:| NFR-1 안전성 (atomic switch, stale 차단) | ContextEpoch + CancellationRegistry + SwitchStateMachine |
devflow-docs/inception/application-design.md:52:| NFR-2 성능 (1초 이내) | KeystoneRescopeAdapter (네트워크 왕복 측정) |
devflow-docs/inception/application-design.md:53:| NFR-3 테스트 커버리지 | 모든 신규 컴포넌트 unit + ContextSwitcher 통합 테스트 |
devflow-docs/inception/application-design.md:54:| NFR-4 UX 일관성 | ContextPicker (기존 SelectPopup 재사용), CommandRegistry (기존 패턴) |
devflow-docs/inception/application-design.md:55:| NFR-5 관측성 | ContextSwitcher의 `tracing` 이벤트 (epoch, target, 결과) |

exec
/bin/zsh -lc 'wc -l devflow-docs/inception/application-design.md devflow-docs/inception/requirements.md devflow-docs/inception/workspace.md' in /Users/jay.ahn/projects/infra/nexttui/.worktrees/runtime-context-switch
 succeeded in 0ms:
      55 devflow-docs/inception/application-design.md
      52 devflow-docs/inception/requirements.md
      48 devflow-docs/inception/workspace.md
     155 total

codex
I found the component list and PR/NFR mapping; next I’m reading the full three docs to validate exact FR/NFR alignment and naming/type expectations before issuing findings.
exec
/bin/zsh -lc "sed -n '1,220p' devflow-docs/inception/application-design.md" in /Users/jay.ahn/projects/infra/nexttui/.worktrees/runtime-context-switch
 succeeded in 0ms:
# Application Design

**Mode**: LIST (목록 단계)
**Timestamp**: 2026-04-13T00:00:00+09:00
**BL**: BL-P2-031 Keystone Rescoping

## 컴포넌트 목록

### 신규 컴포넌트 (7개)

| 컴포넌트 | 책임 | 타입 | PR |
|---------|------|------|-----|
| `ContextEpoch` | App-level epoch 카운터로 이전 컨텍스트의 stale 이벤트 격리 | Util | PR1 |
| `CancellationRegistry` | 활성 폴링/장기 fetch에 대한 `CancellationToken` 등록·취소 | Service | PR1 |
| `SwitchStateMachine` | `Idle → Switching → Committed | Failed` 전환 상태 및 rollback 규약 | Service | PR2 |
| `ContextSwitcher` | 전환 절차(epoch++ → cancel → rescope → catalog 무효화 → commit) 오케스트레이터 | Service | PR2 |
| `KeystoneRescopeAdapter` | Keystone v3 token-method scoped exchange 호출, expires_at 정본 사용 | Adapter | PR2 |
| `ContextPicker` | Ctrl+P 모달, 프로젝트/클라우드 fuzzy 검색·선택 UI (`cloud • project • domain • project_id` 행 표시) | Controller | PR4 |
| `ContextIndicator` | 영구 컨텍스트 표시 위젯 (cloud/project/domain), 전환 직후 강조 | Controller | PR5 |

### 변경 컴포넌트 (8개)

| 컴포넌트 | 변경 책임 | 타입 | PR |
|---------|----------|------|-----|
| `App` (src/app.rs) | 활성 cloud/project 컨텍스트 상태 관리, ContextSwitcher 통합, epoch 보유 | Service | PR2/PR3 |
| `Worker` (src/worker.rs) | 모든 `tokio::spawn` 폴링/장기 fetch에 epoch 검증 + `tokio::select!` cancel branch 추가 | Service | PR1 |
| `CommandRegistry` (src/input/command.rs) | `:switch-project <name|uuid|cloud/project>`, `:switch-cloud`, `:switch-back` 등록 + 충돌 disambiguation + tab 자동완성 | Controller | PR3 |
| `KeyMap` (src/input/keymap.rs) | 글로벌 단축키 (예: Ctrl+P) → ContextPicker 오픈, Identity 리스트의 `s` → 컨텍스트 전환 | Controller | PR4/PR6 |
| `ConfirmDialog` (src/ui/confirm.rs) | destructive 액션 confirm에 `cloud • project` fingerprint 표시, 컨텍스트 변경 직후엔 추가 확인 강제 | Controller | PR5 |
| `StatusBar` (src/ui/status_bar.rs) | ContextIndicator 임베드 위치/우선순위 조정 | Controller | PR5 |
| `Project Module` (src/module/project/mod.rs) | 리스트 행 `s` 액션 핸들러 (Identity 진입점), Enter는 Detail 유지 | Controller | PR6 |
| `TokenCacheStore` (src/adapter/auth/token_cache.rs) | rescoped 토큰을 scope별로 저장/조회 (BL-P2-029 기반), 만료 시 재취득 경로 | Repository | PR2 |

**총 15개 컴포넌트** (신규 7 + 변경 8)

## PR 매핑 요약

| PR | 컴포넌트 |
|----|---------|
| PR1 (선행 인프라) | ContextEpoch, CancellationRegistry, Worker 수정 |
| PR2 (전환 코어) | SwitchStateMachine, ContextSwitcher, KeystoneRescopeAdapter, App 통합, TokenCacheStore 확장 |
| PR3 (명령 UX) | CommandRegistry 확장 |
| PR4 (피커 UI) | ContextPicker, KeyMap (Ctrl+P) |
| PR5 (안전 가시성) | ContextIndicator, StatusBar, ConfirmDialog |
| PR6 (Identity 통합) | Project Module `s` 액션, KeyMap (`s`) |

## NFR 매핑 요약

| NFR | 보장 컴포넌트 |
|-----|--------------|
| NFR-1 안전성 (atomic switch, stale 차단) | ContextEpoch + CancellationRegistry + SwitchStateMachine |
| NFR-2 성능 (1초 이내) | KeystoneRescopeAdapter (네트워크 왕복 측정) |
| NFR-3 테스트 커버리지 | 모든 신규 컴포넌트 unit + ContextSwitcher 통합 테스트 |
| NFR-4 UX 일관성 | ContextPicker (기존 SelectPopup 재사용), CommandRegistry (기존 패턴) |
| NFR-5 관측성 | ContextSwitcher의 `tracing` 이벤트 (epoch, target, 결과) |

exec
/bin/zsh -lc "sed -n '1,220p' devflow-docs/inception/requirements.md" in /Users/jay.ahn/projects/infra/nexttui/.worktrees/runtime-context-switch
 succeeded in 0ms:
# Requirements Analysis

**Depth**: Standard
**Timestamp**: 2026-04-10T11:15:00+09:00

## User Intent
파라미터가 비대화된 view_model 함수에 ViewContext 패턴을 도입하여 구조적으로 정리한다. 기능 변경 없는 순수 리팩토링.

## Scope 분석

파라미터 수 기준으로 리팩토링 대상을 선별:

| 모듈 | 함수 | 파라미터 | 대상 |
|------|------|---------|------|
| server | server_detail_data_full | 7개 (server, migration, flavor, is_resize, cached_volumes, cached_floating_ips) | ✅ |
| server | server_to_row_full | 3개 (server, show_tenant, show_host) | ⚠️ 포함 |
| floating_ip | fip_to_row_with_servers | 4개 (fip, show_tenant, cached_servers, cached_ports) | ⚠️ 포함 |
| volume | volume_detail_data_with_servers | 2개 | ❌ 불필요 |
| volume | volume_to_row_with_servers | 3개 | ❌ 불필요 |
| 나머지 12개 | 1~2개 | ❌ 불필요 |

## Functional Requirements

### ServerViewContext
- FR-01: ServerViewContext 구조체 도입 — server, migration_progress, flavor, is_resize_pending, cached_volumes, cached_floating_ips 필드
- FR-02: server_detail_data_full(7 params) → server_detail_data(ctx: &ServerViewContext) 변경
- FR-03: server_to_row_full(3 params)은 유지 — 파라미터가 적고 ViewContext에 넣기 부자연스러움
- FR-04: server_detail_data(server) 편의 함수 유지 (빈 ViewContext로 위임)
- FR-05: ServerModule의 render에서 ViewContext 생성 → 전달

### FipViewContext
- FR-06: FipRowContext 구조체 도입 — show_tenant, cached_servers, cached_ports 필드
- FR-07: fip_to_row_with_servers(4 params) → fip_to_row(fip, ctx: &FipRowContext) 변경
- FR-08: FloatingIpModule의 rows()에서 FipRowContext 생성 → 전달

### 정리 — _full/_with_servers 패턴 통합
- FR-09: server_detail_data / server_detail_data_full 2개 함수 → server_detail_data(ctx) 1개로 통합
- FR-10: volume_detail_data / volume_detail_data_with_servers → 그대로 유지 (파라미터 2개로 충분)
- FR-11: fip_to_row / fip_to_row_with_servers 2개 → fip_to_row(fip, ctx) 1개로 통합

## Non-Functional Requirements
- NFR-01: 기존 1108 tests 전체 통과 (리팩토링이므로 기능 회귀 0건 필수)
- NFR-02: 외부 API 동작 변경 없음 — 렌더링 결과 동일
- NFR-03: 테스트에서 ViewContext builder 또는 Default 사용 가능하도록 설계

## Assumptions
- 파라미터 3개 이하인 함수는 리팩토링 대상에서 제외
- server_to_row_full은 show_tenant/show_host가 UI 설정이라 ViewContext보다는 별도 파라미터가 자연스러움
- ViewContext는 모듈별 로컬 타입 (공유 trait 불필요)

## Open Questions
없음

exec
/bin/zsh -lc "sed -n '1,220p' devflow-docs/inception/workspace.md" in /Users/jay.ahn/projects/infra/nexttui/.worktrees/runtime-context-switch
 succeeded in 0ms:
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

codex
I confirmed a major inconsistency: `requirements.md` is for a different refactor (ViewContext), not BL-P2-031 context switching. I’ll now scan inception docs for BL-P2-031 requirements to anchor this review and avoid false alignment.
exec
/bin/zsh -lc "rg -n \"BL-P2-031|ContextEpoch|CancellationRegistry|Keystone|rescop|switch-project|switch-cloud|switch-back|NFR-5|10 FR|5 NFR|context switching|context indicator\" devflow-docs/inception devflow-docs -g '*.md'" in /Users/jay.ahn/projects/infra/nexttui/.worktrees/runtime-context-switch
 succeeded in 0ms:
devflow-docs/backlog.md:148:#### BL-P2-031: 프로젝트 전환 + Keystone Rescoping (#39)
devflow-docs/backlog.md:155:- Keystone rescoping으로 토큰 재발급 없이 프로젝트 전환
devflow-docs/backlog.md:174:- Keystone v3 password (현재) + appCredential + token 인증 지원
devflow-docs/backlog.md:175:- HMAC (Cloudian), API Key 등 비-Keystone 인증 확장
devflow-docs/inception/design-review-raw/codex.md:17:FEATURE: BL-P2-031 — Runtime cloud/project context switching via Keystone rescoping.
devflow-docs/inception/design-review-raw/codex.md:20:ALSO CONSIDER: devflow-docs/inception/requirements.md (10 FR + 5 NFR), devflow-docs/inception/workspace.md.
devflow-docs/inception/design-review-raw/codex.md:23:- ContextEpoch + CancellationRegistry for stale-event isolation
devflow-docs/inception/design-review-raw/codex.md:25:- Keystone rescope adapter + service catalog invalidation
devflow-docs/inception/design-review-raw/codex.md:26:- Destructive confirm fingerprint + persistent context indicator
devflow-docs/inception/design-review-raw/codex.md:70:/bin/zsh -lc 'rg -n "''^#|''^##|''^###|FR|NFR|PR[1-6]|ContextEpoch|CancellationRegistry|rescope|switch|component|Adapter|Service|Controller|Repository" devflow-docs/inception/application-design.md devflow-docs/inception/requirements.md devflow-docs/inception/workspace.md' in /Users/jay.ahn/projects/infra/nexttui/.worktrees/runtime-context-switch
devflow-docs/inception/design-review-raw/codex.md:112:devflow-docs/inception/application-design.md:13:| `ContextEpoch` | App-level epoch 카운터로 이전 컨텍스트의 stale 이벤트 격리 | Util | PR1 |
devflow-docs/inception/design-review-raw/codex.md:113:devflow-docs/inception/application-design.md:14:| `CancellationRegistry` | 활성 폴링/장기 fetch에 대한 `CancellationToken` 등록·취소 | Service | PR1 |
devflow-docs/inception/design-review-raw/codex.md:115:devflow-docs/inception/application-design.md:16:| `ContextSwitcher` | 전환 절차(epoch++ → cancel → rescope → catalog 무효화 → commit) 오케스트레이터 | Service | PR2 |
devflow-docs/inception/design-review-raw/codex.md:116:devflow-docs/inception/application-design.md:17:| `KeystoneRescopeAdapter` | Keystone v3 token-method scoped exchange 호출, expires_at 정본 사용 | Adapter | PR2 |
devflow-docs/inception/design-review-raw/codex.md:122:devflow-docs/inception/application-design.md:27:| `CommandRegistry` (src/input/command.rs) | `:switch-project <name|uuid|cloud/project>`, `:switch-cloud`, `:switch-back` 등록 + 충돌 disambiguation + tab 자동완성 | Controller | PR3 |
devflow-docs/inception/design-review-raw/codex.md:127:devflow-docs/inception/application-design.md:32:| `TokenCacheStore` (src/adapter/auth/token_cache.rs) | rescoped 토큰을 scope별로 저장/조회 (BL-P2-029 기반), 만료 시 재취득 경로 | Repository | PR2 |
devflow-docs/inception/design-review-raw/codex.md:129:devflow-docs/inception/application-design.md:40:| PR1 (선행 인프라) | ContextEpoch, CancellationRegistry, Worker 수정 |
devflow-docs/inception/design-review-raw/codex.md:130:devflow-docs/inception/application-design.md:41:| PR2 (전환 코어) | SwitchStateMachine, ContextSwitcher, KeystoneRescopeAdapter, App 통합, TokenCacheStore 확장 |
devflow-docs/inception/design-review-raw/codex.md:137:devflow-docs/inception/application-design.md:51:| NFR-1 안전성 (atomic switch, stale 차단) | ContextEpoch + CancellationRegistry + SwitchStateMachine |
devflow-docs/inception/design-review-raw/codex.md:138:devflow-docs/inception/application-design.md:52:| NFR-2 성능 (1초 이내) | KeystoneRescopeAdapter (네트워크 왕복 측정) |
devflow-docs/inception/design-review-raw/codex.md:141:devflow-docs/inception/application-design.md:55:| NFR-5 관측성 | ContextSwitcher의 `tracing` 이벤트 (epoch, target, 결과) |
devflow-docs/inception/design-review-raw/codex.md:160:**BL**: BL-P2-031 Keystone Rescoping
devflow-docs/inception/design-review-raw/codex.md:168:| `ContextEpoch` | App-level epoch 카운터로 이전 컨텍스트의 stale 이벤트 격리 | Util | PR1 |
devflow-docs/inception/design-review-raw/codex.md:169:| `CancellationRegistry` | 활성 폴링/장기 fetch에 대한 `CancellationToken` 등록·취소 | Service | PR1 |
devflow-docs/inception/design-review-raw/codex.md:171:| `ContextSwitcher` | 전환 절차(epoch++ → cancel → rescope → catalog 무효화 → commit) 오케스트레이터 | Service | PR2 |
devflow-docs/inception/design-review-raw/codex.md:172:| `KeystoneRescopeAdapter` | Keystone v3 token-method scoped exchange 호출, expires_at 정본 사용 | Adapter | PR2 |
devflow-docs/inception/design-review-raw/codex.md:182:| `CommandRegistry` (src/input/command.rs) | `:switch-project <name|uuid|cloud/project>`, `:switch-cloud`, `:switch-back` 등록 + 충돌 disambiguation + tab 자동완성 | Controller | PR3 |
devflow-docs/inception/design-review-raw/codex.md:187:| `TokenCacheStore` (src/adapter/auth/token_cache.rs) | rescoped 토큰을 scope별로 저장/조회 (BL-P2-029 기반), 만료 시 재취득 경로 | Repository | PR2 |
devflow-docs/inception/design-review-raw/codex.md:195:| PR1 (선행 인프라) | ContextEpoch, CancellationRegistry, Worker 수정 |
devflow-docs/inception/design-review-raw/codex.md:196:| PR2 (전환 코어) | SwitchStateMachine, ContextSwitcher, KeystoneRescopeAdapter, App 통합, TokenCacheStore 확장 |
devflow-docs/inception/design-review-raw/codex.md:206:| NFR-1 안전성 (atomic switch, stale 차단) | ContextEpoch + CancellationRegistry + SwitchStateMachine |
devflow-docs/inception/design-review-raw/codex.md:207:| NFR-2 성능 (1초 이내) | KeystoneRescopeAdapter (네트워크 왕복 측정) |
devflow-docs/inception/design-review-raw/codex.md:210:| NFR-5 관측성 | ContextSwitcher의 `tracing` 이벤트 (epoch, target, 결과) |
devflow-docs/inception/design-review-raw/codex.md:321:I confirmed a major inconsistency: `requirements.md` is for a different refactor (ViewContext), not BL-P2-031 context switching. I’ll now scan inception docs for BL-P2-031 requirements to anchor this review and avoid false alignment.
devflow-docs/session-summary-archived-20260324-132401.md:18:- [x] application-design — DETAIL 완료 (52개 컴포넌트 Comprehensive + 5 NFR Design Patterns)
devflow-docs/session-summary-archived-20260324-132401.md:30:- RBAC: Keystone 역할 기반 메뉴/액션 가시성 제어
devflow-docs/session-summary-archived-20260324-132401.md:44:- `devflow-docs/inception/detail-design-domain-nfr.md` — Domain Module + NFR 패턴 (16개 + 5 NFR)
devflow-docs/session-summary-archived-20260324-132401.md:53:  - Domain Models (Nova/Neutron/Cinder/Glance/Keystone + Admin)
devflow-docs/session-summary-archived-20260324-132401.md:66:  - 6개 Port trait (AuthProvider, NovaPort, NeutronPort, CinderPort, KeystonePort, GlancePort)
devflow-docs/session-summary-archived-20260324-132401.md:68:  - 5개 Mock adapter (MockNova/Neutron/Cinder/Glance/Keystone)
devflow-docs/session-summary-archived-20260324-132401.md:79:  - KeystoneAuthAdapter (AuthProvider impl, Keystone v3 auth, token parsing, refresh loop)
devflow-docs/session-summary-archived-20260324-132401.md:120:- [x] Unit 12: identity-domain — KeystoneHttpAdapter + ProjectModule + UserModule (35 tests)
devflow-docs/devflow-state-archived-20260324-132401.md:13:- application-design: done (52개 컴포넌트 Comprehensive + 5 NFR)
devflow-docs/audit.md:41:- **Complexity** — Minimal, #34 범위 축소 (SwitchCloud/rescoping → #39 분리)
devflow-docs/inception/application-design.md:5:**BL**: BL-P2-031 Keystone Rescoping
devflow-docs/inception/application-design.md:13:| `ContextEpoch` | App-level epoch 카운터로 이전 컨텍스트의 stale 이벤트 격리 | Util | PR1 |
devflow-docs/inception/application-design.md:14:| `CancellationRegistry` | 활성 폴링/장기 fetch에 대한 `CancellationToken` 등록·취소 | Service | PR1 |
devflow-docs/inception/application-design.md:16:| `ContextSwitcher` | 전환 절차(epoch++ → cancel → rescope → catalog 무효화 → commit) 오케스트레이터 | Service | PR2 |
devflow-docs/inception/application-design.md:17:| `KeystoneRescopeAdapter` | Keystone v3 token-method scoped exchange 호출, expires_at 정본 사용 | Adapter | PR2 |
devflow-docs/inception/application-design.md:27:| `CommandRegistry` (src/input/command.rs) | `:switch-project <name|uuid|cloud/project>`, `:switch-cloud`, `:switch-back` 등록 + 충돌 disambiguation + tab 자동완성 | Controller | PR3 |
devflow-docs/inception/application-design.md:32:| `TokenCacheStore` (src/adapter/auth/token_cache.rs) | rescoped 토큰을 scope별로 저장/조회 (BL-P2-029 기반), 만료 시 재취득 경로 | Repository | PR2 |
devflow-docs/inception/application-design.md:40:| PR1 (선행 인프라) | ContextEpoch, CancellationRegistry, Worker 수정 |
devflow-docs/inception/application-design.md:41:| PR2 (전환 코어) | SwitchStateMachine, ContextSwitcher, KeystoneRescopeAdapter, App 통합, TokenCacheStore 확장 |
devflow-docs/inception/application-design.md:51:| NFR-1 안전성 (atomic switch, stale 차단) | ContextEpoch + CancellationRegistry + SwitchStateMachine |
devflow-docs/inception/application-design.md:52:| NFR-2 성능 (1초 이내) | KeystoneRescopeAdapter (네트워크 왕복 측정) |
devflow-docs/inception/application-design.md:55:| NFR-5 관측성 | ContextSwitcher의 `tracing` 이벤트 (epoch, target, 결과) |
devflow-docs/inception/design-review-raw/codex.md:17:FEATURE: BL-P2-031 — Runtime cloud/project context switching via Keystone rescoping.
devflow-docs/inception/design-review-raw/codex.md:20:ALSO CONSIDER: devflow-docs/inception/requirements.md (10 FR + 5 NFR), devflow-docs/inception/workspace.md.
devflow-docs/inception/design-review-raw/codex.md:23:- ContextEpoch + CancellationRegistry for stale-event isolation
devflow-docs/inception/design-review-raw/codex.md:25:- Keystone rescope adapter + service catalog invalidation
devflow-docs/inception/design-review-raw/codex.md:26:- Destructive confirm fingerprint + persistent context indicator
devflow-docs/inception/design-review-raw/codex.md:70:/bin/zsh -lc 'rg -n "''^#|''^##|''^###|FR|NFR|PR[1-6]|ContextEpoch|CancellationRegistry|rescope|switch|component|Adapter|Service|Controller|Repository" devflow-docs/inception/application-design.md devflow-docs/inception/requirements.md devflow-docs/inception/workspace.md' in /Users/jay.ahn/projects/infra/nexttui/.worktrees/runtime-context-switch
devflow-docs/inception/design-review-raw/codex.md:112:devflow-docs/inception/application-design.md:13:| `ContextEpoch` | App-level epoch 카운터로 이전 컨텍스트의 stale 이벤트 격리 | Util | PR1 |
devflow-docs/inception/design-review-raw/codex.md:113:devflow-docs/inception/application-design.md:14:| `CancellationRegistry` | 활성 폴링/장기 fetch에 대한 `CancellationToken` 등록·취소 | Service | PR1 |
devflow-docs/inception/design-review-raw/codex.md:115:devflow-docs/inception/application-design.md:16:| `ContextSwitcher` | 전환 절차(epoch++ → cancel → rescope → catalog 무효화 → commit) 오케스트레이터 | Service | PR2 |
devflow-docs/inception/design-review-raw/codex.md:116:devflow-docs/inception/application-design.md:17:| `KeystoneRescopeAdapter` | Keystone v3 token-method scoped exchange 호출, expires_at 정본 사용 | Adapter | PR2 |
devflow-docs/inception/design-review-raw/codex.md:122:devflow-docs/inception/application-design.md:27:| `CommandRegistry` (src/input/command.rs) | `:switch-project <name|uuid|cloud/project>`, `:switch-cloud`, `:switch-back` 등록 + 충돌 disambiguation + tab 자동완성 | Controller | PR3 |
devflow-docs/inception/design-review-raw/codex.md:127:devflow-docs/inception/application-design.md:32:| `TokenCacheStore` (src/adapter/auth/token_cache.rs) | rescoped 토큰을 scope별로 저장/조회 (BL-P2-029 기반), 만료 시 재취득 경로 | Repository | PR2 |
devflow-docs/inception/design-review-raw/codex.md:129:devflow-docs/inception/application-design.md:40:| PR1 (선행 인프라) | ContextEpoch, CancellationRegistry, Worker 수정 |
devflow-docs/inception/design-review-raw/codex.md:130:devflow-docs/inception/application-design.md:41:| PR2 (전환 코어) | SwitchStateMachine, ContextSwitcher, KeystoneRescopeAdapter, App 통합, TokenCacheStore 확장 |
devflow-docs/inception/design-review-raw/codex.md:137:devflow-docs/inception/application-design.md:51:| NFR-1 안전성 (atomic switch, stale 차단) | ContextEpoch + CancellationRegistry + SwitchStateMachine |
devflow-docs/inception/design-review-raw/codex.md:138:devflow-docs/inception/application-design.md:52:| NFR-2 성능 (1초 이내) | KeystoneRescopeAdapter (네트워크 왕복 측정) |
devflow-docs/inception/design-review-raw/codex.md:141:devflow-docs/inception/application-design.md:55:| NFR-5 관측성 | ContextSwitcher의 `tracing` 이벤트 (epoch, target, 결과) |
devflow-docs/inception/design-review-raw/codex.md:160:**BL**: BL-P2-031 Keystone Rescoping
devflow-docs/inception/design-review-raw/codex.md:168:| `ContextEpoch` | App-level epoch 카운터로 이전 컨텍스트의 stale 이벤트 격리 | Util | PR1 |
devflow-docs/inception/design-review-raw/codex.md:169:| `CancellationRegistry` | 활성 폴링/장기 fetch에 대한 `CancellationToken` 등록·취소 | Service | PR1 |
devflow-docs/inception/design-review-raw/codex.md:171:| `ContextSwitcher` | 전환 절차(epoch++ → cancel → rescope → catalog 무효화 → commit) 오케스트레이터 | Service | PR2 |
devflow-docs/inception/design-review-raw/codex.md:172:| `KeystoneRescopeAdapter` | Keystone v3 token-method scoped exchange 호출, expires_at 정본 사용 | Adapter | PR2 |
devflow-docs/inception/design-review-raw/codex.md:182:| `CommandRegistry` (src/input/command.rs) | `:switch-project <name|uuid|cloud/project>`, `:switch-cloud`, `:switch-back` 등록 + 충돌 disambiguation + tab 자동완성 | Controller | PR3 |
devflow-docs/inception/design-review-raw/codex.md:187:| `TokenCacheStore` (src/adapter/auth/token_cache.rs) | rescoped 토큰을 scope별로 저장/조회 (BL-P2-029 기반), 만료 시 재취득 경로 | Repository | PR2 |
devflow-docs/inception/design-review-raw/codex.md:195:| PR1 (선행 인프라) | ContextEpoch, CancellationRegistry, Worker 수정 |
devflow-docs/inception/design-review-raw/codex.md:196:| PR2 (전환 코어) | SwitchStateMachine, ContextSwitcher, KeystoneRescopeAdapter, App 통합, TokenCacheStore 확장 |
devflow-docs/inception/design-review-raw/codex.md:206:| NFR-1 안전성 (atomic switch, stale 차단) | ContextEpoch + CancellationRegistry + SwitchStateMachine |
devflow-docs/inception/design-review-raw/codex.md:207:| NFR-2 성능 (1초 이내) | KeystoneRescopeAdapter (네트워크 왕복 측정) |
devflow-docs/inception/design-review-raw/codex.md:210:| NFR-5 관측성 | ContextSwitcher의 `tracing` 이벤트 (epoch, target, 결과) |
devflow-docs/inception/design-review-raw/codex.md:321:I confirmed a major inconsistency: `requirements.md` is for a different refactor (ViewContext), not BL-P2-031 context switching. I’ll now scan inception docs for BL-P2-031 requirements to anchor this review and avoid false alignment.
devflow-docs/inception/application-design.md:5:**BL**: BL-P2-031 Keystone Rescoping
devflow-docs/inception/application-design.md:13:| `ContextEpoch` | App-level epoch 카운터로 이전 컨텍스트의 stale 이벤트 격리 | Util | PR1 |
devflow-docs/inception/application-design.md:14:| `CancellationRegistry` | 활성 폴링/장기 fetch에 대한 `CancellationToken` 등록·취소 | Service | PR1 |
devflow-docs/inception/application-design.md:16:| `ContextSwitcher` | 전환 절차(epoch++ → cancel → rescope → catalog 무효화 → commit) 오케스트레이터 | Service | PR2 |
devflow-docs/inception/application-design.md:17:| `KeystoneRescopeAdapter` | Keystone v3 token-method scoped exchange 호출, expires_at 정본 사용 | Adapter | PR2 |
devflow-docs/inception/application-design.md:27:| `CommandRegistry` (src/input/command.rs) | `:switch-project <name|uuid|cloud/project>`, `:switch-cloud`, `:switch-back` 등록 + 충돌 disambiguation + tab 자동완성 | Controller | PR3 |
devflow-docs/inception/application-design.md:32:| `TokenCacheStore` (src/adapter/auth/token_cache.rs) | rescoped 토큰을 scope별로 저장/조회 (BL-P2-029 기반), 만료 시 재취득 경로 | Repository | PR2 |
devflow-docs/inception/application-design.md:40:| PR1 (선행 인프라) | ContextEpoch, CancellationRegistry, Worker 수정 |
devflow-docs/inception/application-design.md:41:| PR2 (전환 코어) | SwitchStateMachine, ContextSwitcher, KeystoneRescopeAdapter, App 통합, TokenCacheStore 확장 |
devflow-docs/inception/application-design.md:51:| NFR-1 안전성 (atomic switch, stale 차단) | ContextEpoch + CancellationRegistry + SwitchStateMachine |
devflow-docs/inception/application-design.md:52:| NFR-2 성능 (1초 이내) | KeystoneRescopeAdapter (네트워크 왕복 측정) |
devflow-docs/inception/application-design.md:55:| NFR-5 관측성 | ContextSwitcher의 `tracing` 이벤트 (epoch, target, 결과) |

exec
/bin/zsh -lc 'ls -la devflow-docs/inception' in /Users/jay.ahn/projects/infra/nexttui/.worktrees/runtime-context-switch
 succeeded in 0ms:
total 1064
drwxr-xr-x@  9 jay.ahn  staff     288 Apr 13 16:04 .
drwxr-xr-x@ 21 jay.ahn  staff     672 Apr 13 15:54 ..
-rw-r--r--@  1 jay.ahn  staff    3638 Apr 13 16:02 application-design.md
-rw-r--r--@  1 jay.ahn  staff  141652 Apr 13 15:54 btop-screen1.png
-rw-r--r--@  1 jay.ahn  staff  263869 Apr 13 15:54 btop-screen2.png
-rw-r--r--@  1 jay.ahn  staff  120891 Apr 13 15:54 btop-screen3.png
drwxr-xr-x@  3 jay.ahn  staff      96 Apr 13 16:05 design-review-raw
-rw-r--r--@  1 jay.ahn  staff    2716 Apr 13 15:54 requirements.md
-rw-r--r--@  1 jay.ahn  staff    2113 Apr 13 15:54 workspace.md

exec
/bin/zsh -lc 'rg --files devflow-docs/inception' in /Users/jay.ahn/projects/infra/nexttui/.worktrees/runtime-context-switch
 succeeded in 0ms:
devflow-docs/inception/design-review-raw/codex.md
devflow-docs/inception/workspace.md
devflow-docs/inception/btop-screen3.png
devflow-docs/inception/btop-screen2.png
devflow-docs/inception/application-design.md
devflow-docs/inception/btop-screen1.png
devflow-docs/inception/requirements.md

codex
Only those three inception files exist, and there is no BL-P2-031-specific requirements doc in that folder. I’ll cross-check against actual code structure conventions (`src/app.rs`, `src/worker.rs`, input/ui modules) to judge naming/type fit and test seams accurately.
exec
/bin/zsh -lc "sed -n '1,220p' src/app.rs" in /Users/jay.ahn/projects/infra/nexttui/.worktrees/runtime-context-switch
 succeeded in 0ms:
use std::collections::HashMap;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use ratatui::Frame;
use ratatui::widgets::{Block, BorderType, Borders};
use tokio::sync::mpsc;

use crate::action::Action;
use crate::background::BackgroundTracker;
use crate::component::{Component, InputMode, LayoutHint};
use crate::config::Config;
use crate::event::AppEvent;
use crate::infra::audit::{AuditEntry, AuditLogger, AuditResult};
use crate::infra::rbac::{ActionKind, RbacGuard};
use crate::models::common::Route;
use crate::router::Router;
use crate::ui::header::{Header, HeaderContext};
use crate::ui::layout::LayoutManager;
use crate::ui::sidebar::Sidebar;
use crate::ui::status_bar::{StatusBar, StatusInfo};
use crate::ui::theme::{self, Theme};
use crate::ui::refresh::RefreshScheduler;
use crate::ui::activity_log::{ActivityLog, ActivityLogPopup};
use crate::ui::toast::{ToastMessage, ToastSeverity};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FocusPane {
    Sidebar,
    Content,
}

pub struct App {
    pub should_quit: bool,
    pub input_mode: InputMode,
    pub sidebar_visible: bool,
    pub focus: FocusPane,

    router: Router,
    components: HashMap<Route, Box<dyn Component>>,
    background_tracker: BackgroundTracker,
    action_tx: mpsc::UnboundedSender<Action>,

    pub rbac: Arc<RbacGuard>,
    pub all_tenants: Arc<AtomicBool>,
    config: Arc<Config>,
    layout: LayoutManager,
    sidebar: Sidebar,
    header: Header,
    status_bar: StatusBar,
    route_labels: HashMap<Route, &'static str>,
    refresh_scheduler: RefreshScheduler,
    activity_log: ActivityLog,
    activity_popup: ActivityLogPopup,
    show_activity_log: bool,
    audit_logger: Option<AuditLogger>,
}

impl App {
    pub fn new(config: Config, action_tx: mpsc::UnboundedSender<Action>) -> Self {
        let tick_rate = std::time::Duration::from_millis(config.app_config().tick_rate_ms);
        crate::ui::theme::Theme::init(config.app_config().theme);
        let audit_logger = Self::init_audit_logger();
        Self {
            should_quit: false,
            input_mode: InputMode::Normal,
            sidebar_visible: true,
            focus: FocusPane::Content,
            router: Router::new(Route::Servers),
            components: HashMap::new(),
            background_tracker: BackgroundTracker::new(),
            action_tx,
            rbac: Arc::new(RbacGuard::new()),
            all_tenants: Arc::new(AtomicBool::new(false)),
            config: Arc::new(config),
            layout: LayoutManager::new(),
            sidebar: Sidebar::new(Vec::new()),
            header: Header::new(),
            status_bar: StatusBar::new(),
            route_labels: HashMap::new(),
            refresh_scheduler: RefreshScheduler::new(tick_rate),
            activity_log: ActivityLog::new(),
            activity_popup: ActivityLogPopup::new(),
            show_activity_log: false,
            audit_logger,
        }
    }

    pub fn from_registry(
        config: Config,
        action_tx: mpsc::UnboundedSender<Action>,
        registry: crate::registry::ModuleRegistry,
        rbac: Arc<RbacGuard>,
    ) -> (Self, Vec<Action>) {
        let parts = registry.into_parts();
        let tick_rate = std::time::Duration::from_millis(config.app_config().tick_rate_ms);
        crate::ui::theme::Theme::init(config.app_config().theme);
        let audit_logger = Self::init_audit_logger();
        let mut app = Self {
            should_quit: false,
            input_mode: InputMode::Normal,
            sidebar_visible: true,
            focus: FocusPane::Content,
            router: Router::new(Route::Servers),
            components: parts.components,
            background_tracker: BackgroundTracker::new(),
            action_tx,
            rbac,
            all_tenants: Arc::new(AtomicBool::new(false)),
            config: Arc::new(config),
            layout: LayoutManager::new(),
            sidebar: Sidebar::new(parts.sidebar_items),
            header: Header::new(),
            status_bar: StatusBar::new(),
            route_labels: parts.route_labels,
            refresh_scheduler: RefreshScheduler::new(tick_rate),
            activity_log: ActivityLog::new(),
            activity_popup: ActivityLogPopup::new(),
            show_activity_log: false,
            audit_logger,
        };
        // Store sidebar items for number-key navigation
        app.sidebar.sync_active(&Route::Servers, false);
        app.broadcast_admin();
        (app, parts.initial_actions)
    }

    /// Broadcast current admin status to all registered modules.
    pub fn broadcast_admin(&mut self) {
        let is_admin = self.rbac.is_admin();
        for component in self.components.values_mut() {
            component.set_admin(is_admin);
        }
    }

    pub fn route_label(&self, route: &Route) -> &str {
        self.route_labels.get(route).copied().unwrap_or("Unknown")
    }

    /// Register a domain module component for a given route (test use only).
    #[cfg(test)]
    pub fn register_component(&mut self, route: Route, component: Box<dyn Component>) {
        self.components.insert(route, component);
    }

    /// Inject an audit logger for testing.
    #[cfg(test)]
    pub fn set_audit_logger(&mut self, logger: AuditLogger) {
        self.audit_logger = Some(logger);
    }

    /// Handle key input. Returns true if a re-render is needed.
    pub fn handle_key(&mut self, key: KeyEvent) -> bool {
        let no_modifiers = key.modifiers.is_empty();

        // Activity log popup pseudo-modal: intercept j/k/Esc/! only
        if self.show_activity_log {
            match key.code {
                KeyCode::Char('j') => {
                    self.activity_popup
                        .scroll_down(self.activity_log.entries().len());
                }
                KeyCode::Char('k') => {
                    self.activity_popup.scroll_up();
                }
                KeyCode::Esc => {
                    self.show_activity_log = false;
                    self.activity_popup.reset_scroll();
                }
                KeyCode::Char('!') => {
                    self.show_activity_log = false;
                    self.activity_popup.reset_scroll();
                }
                KeyCode::Char('w') => {
                    let path = std::path::PathBuf::from("/tmp/nexttui-activity.log");
                    if let Err(e) = self.activity_log.export_to_file(&path) {
                        self.background_tracker.add_toast(
                            format!("Export failed: {e}"),
                            crate::background::ToastLevel::Error,
                        );
                    } else {
                        self.background_tracker.add_toast(
                            format!("Activity log exported to {}", path.display()),
                            crate::background::ToastLevel::Info,
                        );
                    }
                }
                _ => {}
            }
            return true;
        }

        // Ctrl+c always quits
        if key.code == KeyCode::Char('c') && key.modifiers.contains(KeyModifiers::CONTROL) {
            self.should_quit = true;
            return true;
        }

        // Ctrl+a toggles all_tenants (admin only)
        if key.code == KeyCode::Char('a') && key.modifiers.contains(KeyModifiers::CONTROL) {
            if self.rbac.can_perform(ActionKind::ViewAllTenants) {
                self.dispatch_action(Action::ToggleAllTenants);
            }
            return true;
        }

        // '!' toggle activity log (Shift+1 sends '!' with SHIFT modifier)
        if self.input_mode == InputMode::Normal && key.code == KeyCode::Char('!') {
            self.show_activity_log = !self.show_activity_log;
            if self.show_activity_log {
                self.activity_log.mark_all_read();
            }
            return true;
        }

        // Modal component (ConfirmDialog, SelectPopup) — delegate all keys directly
        if self.input_mode == InputMode::Normal {
            let is_modal = self.components.get(&self.router.current())
                .map_or(false, |c| c.is_modal());

exec
/bin/zsh -lc "sed -n '1,260p' src/worker.rs" in /Users/jay.ahn/projects/infra/nexttui/.worktrees/runtime-context-switch
 succeeded in 0ms:
//! Background worker: consumes Actions from the UI, calls OpenStack APIs,
//! and sends AppEvents back to the event loop for UI updates.

use std::collections::HashSet;
use std::sync::{Arc, Mutex};
use std::sync::atomic::{AtomicBool, Ordering};

use chrono::{DateTime, Utc};
use tokio::sync::mpsc;
use tracing::Instrument;

use crate::action::Action;
use crate::adapter::registry::AdapterRegistry;
use crate::event::AppEvent;
use crate::infra::rbac::{ActionKind, RbacGuard};
use crate::port::types::*;

/// Run the background worker loop.
/// Receives Actions from `action_rx`, calls the appropriate API via `registry`,
/// and sends resulting AppEvents to `event_tx`.
#[tracing::instrument(skip_all)]
pub async fn run_worker(
    registry: Arc<AdapterRegistry>,
    rbac: Arc<RbacGuard>,
    all_tenants: Arc<AtomicBool>,
    mut action_rx: mpsc::UnboundedReceiver<Action>,
    event_tx: mpsc::UnboundedSender<AppEvent>,
) {
    let polling_servers: Arc<Mutex<HashSet<String>>> = Arc::new(Mutex::new(HashSet::new()));
    let in_flight_fetches: Arc<Mutex<HashSet<String>>> = Arc::new(Mutex::new(HashSet::new()));

    while let Some(action) = action_rx.recv().await {
        // RBAC guard: check CUD permissions before API call
        if let Some(kind) = action_to_kind(&action)
            && !rbac.can_perform(kind)
        {
            let _ = event_tx.send(AppEvent::PermissionDenied {
                operation: action_name(&action).to_string(),
            });
            continue;
        }

        // FetchDedup: skip if same fetch is already in-flight
        let dedup_key = fetch_dedup_key(&action);
        if let Some(key) = dedup_key
            && !in_flight_fetches.lock().unwrap_or_else(|e| e.into_inner()).insert(key.to_string())
        {
            continue;
        }

        let registry = registry.clone();
        let event_tx = event_tx.clone();
        let all_tenants = all_tenants.clone();
        let polling_servers = polling_servers.clone();
        let in_flight_fetches = in_flight_fetches.clone();

        let poll_migration_id = poll_migration_server_id(&action);
        let poll_status_id = poll_server_id_for_status(&action);

        let span = tracing::info_span!("worker_task", action = action_name(&action));
        tokio::spawn(
            async move {
                let event = handle_action(&registry, &all_tenants, action).await;
                let success = event.as_ref().is_some_and(|ev| !matches!(ev, AppEvent::ApiError { .. }));
                if let Some(ev) = event {
                    let _ = event_tx.send(ev);
                }
                // Release fetch dedup guard
                if let Some(key) = dedup_key {
                    in_flight_fetches.lock().unwrap_or_else(|e| e.into_inner()).remove(key);
                }
                if success {
                    if let Some(ref server_id) = poll_migration_id
                        && polling_servers.lock().unwrap_or_else(|e| e.into_inner()).insert(server_id.clone())
                    {
                        poll_migration_progress(&registry, &event_tx, server_id).await;
                        polling_servers.lock().unwrap_or_else(|e| e.into_inner()).remove(server_id);
                    }
                    if let Some(ref server_id) = poll_status_id
                        && polling_servers.lock().unwrap_or_else(|e| e.into_inner()).insert(server_id.clone())
                    {
                        poll_server_status(&registry, &event_tx, server_id).await;
                        polling_servers.lock().unwrap_or_else(|e| e.into_inner()).remove(server_id);
                    }
                }
            }
            .instrument(span),
        );
    }
}

/// Map an Action to its RBAC ActionKind for permission checking.
/// Returns None for read-only/UI actions that need no guard.
fn action_to_kind(action: &Action) -> Option<ActionKind> {
    match action {
        // Create (member-level)
        Action::CreateServer(_)
        | Action::CreateFlavor(_)
        | Action::CreateNetwork(_)
        | Action::CreateSecurityGroup(_)
        | Action::CreateSecurityGroupRule(_)
        | Action::CreateFloatingIp { .. }
        | Action::CreateVolume(_)
        | Action::CreateSnapshot(_)
        | Action::CreateImage(_)
        | Action::CreateServerSnapshot { .. } => Some(ActionKind::Create),

        // Create (admin-only: identity resources)
        Action::CreateProject(_)
        | Action::CreateUser(_) => Some(ActionKind::ManageQuota),

        // Delete (member-level, non-force)
        Action::DeleteServer { .. }
        | Action::DeleteFlavor { .. }
        | Action::DeleteSecurityGroup { .. }
        | Action::DeleteSecurityGroupRule { .. }
        | Action::DeleteFloatingIp { .. }
        | Action::DeleteSnapshot { .. }
        | Action::DeleteImage { .. } => Some(ActionKind::Delete),

        // Delete (admin-only: identity resources)
        Action::DeleteProject { .. }
        | Action::DeleteUser { .. } => Some(ActionKind::ManageQuota),

        // Force delete
        Action::DeleteVolume { force: true, .. } => Some(ActionKind::ForceDelete),
        Action::DeleteVolume { force: false, .. } => Some(ActionKind::Delete),

        // Resize (member-level)
        Action::ResizeServer { .. }
        | Action::ConfirmResize { .. }
        | Action::RevertResize { .. } => Some(ActionKind::Resize),

        // Migration / Evacuate (admin-only)
        Action::LiveMigrateServer { .. }
        | Action::ColdMigrateServer { .. }
        | Action::ConfirmMigration { .. }
        | Action::RevertMigration { .. } => Some(ActionKind::Migrate),

        Action::EvacuateServer { .. } => Some(ActionKind::Evacuate),

        Action::DisableComputeService { .. }
        | Action::EnableComputeService { .. } => Some(ActionKind::EnableDisable),

        // Server lifecycle — treated as CUD for RBAC purposes
        Action::RebootServer { .. }
        | Action::StartServer { .. }
        | Action::StopServer { .. } => Some(ActionKind::Create),

        // Volume extend
        Action::ExtendVolume { .. } => Some(ActionKind::Create),

        // Attach / Associate (member-level)
        Action::AttachVolume { .. }
        | Action::AssociateFloatingIp { .. } => Some(ActionKind::Attach),

        // Detach / Disassociate (member-level)
        Action::DetachVolume { .. }
        | Action::DisassociateFloatingIp { .. } => Some(ActionKind::Detach),

        // Force operations (admin-only)
        Action::ForceDetachVolume { .. }
        | Action::ForceResetVolumeState { .. } => Some(ActionKind::ForceDelete),

        // Read / UI / System — no guard
        _ => None,
    }
}

/// Human-readable name for an Action, used in PermissionDenied messages.
fn action_name(action: &Action) -> &str {
    match action {
        Action::CreateServer(_) => "CreateServer",
        Action::DeleteServer { .. } => "DeleteServer",
        Action::RebootServer { .. } => "RebootServer",
        Action::StartServer { .. } => "StartServer",
        Action::StopServer { .. } => "StopServer",
        Action::CreateServerSnapshot { .. } => "CreateServerSnapshot",
        Action::CreateFlavor(_) => "CreateFlavor",
        Action::DeleteFlavor { .. } => "DeleteFlavor",
        Action::CreateNetwork(_) => "CreateNetwork",
        Action::CreateSecurityGroup(_) => "CreateSecurityGroup",
        Action::DeleteSecurityGroup { .. } => "DeleteSecurityGroup",
        Action::CreateSecurityGroupRule(_) => "CreateSecurityGroupRule",
        Action::DeleteSecurityGroupRule { .. } => "DeleteSecurityGroupRule",
        Action::CreateFloatingIp { .. } => "CreateFloatingIp",
        Action::DeleteFloatingIp { .. } => "DeleteFloatingIp",
        Action::CreateVolume(_) => "CreateVolume",
        Action::DeleteVolume { .. } => "DeleteVolume",
        Action::ExtendVolume { .. } => "ExtendVolume",
        Action::CreateSnapshot(_) => "CreateSnapshot",
        Action::DeleteSnapshot { .. } => "DeleteSnapshot",
        Action::CreateImage(_) => "CreateImage",
        Action::DeleteImage { .. } => "DeleteImage",
        Action::CreateProject(_) => "CreateProject",
        Action::DeleteProject { .. } => "DeleteProject",
        Action::CreateUser(_) => "CreateUser",
        Action::DeleteUser { .. } => "DeleteUser",
        Action::ResizeServer { .. } => "ResizeServer",
        Action::ConfirmResize { .. } => "ConfirmResize",
        Action::RevertResize { .. } => "RevertResize",
        Action::LiveMigrateServer { .. } => "LiveMigrateServer",
        Action::ColdMigrateServer { .. } => "ColdMigrateServer",
        Action::ConfirmMigration { .. } => "ConfirmMigration",
        Action::RevertMigration { .. } => "RevertMigration",
        Action::EvacuateServer { .. } => "EvacuateServer",
        Action::DisableComputeService { .. } => "DisableComputeService",
        Action::EnableComputeService { .. } => "EnableComputeService",
        Action::FetchMigrationProgress { .. } => "FetchMigrationProgress",
        Action::AttachVolume { .. } => "AttachVolume",
        Action::DetachVolume { .. } => "DetachVolume",
        Action::ForceDetachVolume { .. } => "ForceDetachVolume",
        Action::ForceResetVolumeState { .. } => "ForceResetVolumeState",
        Action::AssociateFloatingIp { .. } => "AssociateFloatingIp",
        Action::DisassociateFloatingIp { .. } => "DisassociateFloatingIp",
        Action::FetchPorts { .. } => "FetchPorts",
        Action::FetchUsage { .. } => "FetchUsage",
        _ => "Unknown",
    }
}

async fn handle_action(registry: &AdapterRegistry, all_tenants: &AtomicBool, action: Action) -> Option<AppEvent> {
    let action_label = action_name(&action);
    tracing::info!(action = action_label, "handling action");
    let default_pagination = PaginationParams::default();
    let at = all_tenants.load(Ordering::Relaxed);

    match action {
        // -- Nova: Servers --------------------------------------------------
        Action::FetchServers => {
            match registry
                .nova
                .list_servers(&ServerListFilter { all_tenants: at, ..Default::default() }, &default_pagination)
                .await
            {
                Ok(resp) => Some(AppEvent::ServersLoaded(resp.items)),
                Err(e) => Some(api_error("FetchServers", e)),
            }
        }
        Action::CreateServer(params) => {
            match registry.nova.create_server(&params).await {
                Ok(server) => Some(AppEvent::ServerCreated(server)),
                Err(e) => Some(api_error("CreateServer", e)),
            }
        }
        Action::DeleteServer { id, name } => {
            match registry.nova.delete_server(&id).await {
                Ok(()) => Some(AppEvent::ServerDeleted { id, name }),
                Err(e) => Some(api_error("DeleteServer", e)),
            }
        }
        Action::RebootServer { id, hard } => {
            let reboot_type = if hard {
                RebootType::Hard
            } else {
                RebootType::Soft
            };
            match registry.nova.reboot_server(&id, reboot_type).await {
                Ok(()) => Some(AppEvent::ServerRebooted { id }),
                Err(e) => Some(api_error("RebootServer", e)),

exec
/bin/zsh -lc "sed -n '1,260p' src/input/command.rs" in /Users/jay.ahn/projects/infra/nexttui/.worktrees/runtime-context-switch
 succeeded in 0ms:
use std::collections::HashMap;
use std::fs;
use std::path::PathBuf;

use crate::error::Result;
use crate::models::common::Route;

const MAX_HISTORY_ENTRY_LEN: usize = 1024;

#[derive(Debug, Clone, PartialEq)]
pub enum Command {
    Navigate(Route),
    Quit,
    Refresh,
    Help,
    ContextSwitch(String),
    ContextList,
    Unknown(String),
}

/// Single source of truth for command name → (abbreviation, Route) mappings.
/// Both abbreviation map and route map are derived from this table.
struct CommandDef {
    name: &'static str,
    abbreviation: &'static str,
    route: Route,
}

const COMMAND_TABLE: &[CommandDef] = &[
    CommandDef { name: "servers", abbreviation: "srv", route: Route::Servers },
    CommandDef { name: "networks", abbreviation: "net", route: Route::Networks },
    CommandDef { name: "volumes", abbreviation: "vol", route: Route::Volumes },
    CommandDef { name: "floatingip", abbreviation: "fip", route: Route::FloatingIps },
    CommandDef { name: "security-groups", abbreviation: "sec", route: Route::SecurityGroups },
    CommandDef { name: "images", abbreviation: "img", route: Route::Images },
    CommandDef { name: "flavors", abbreviation: "flv", route: Route::Flavors },
    CommandDef { name: "projects", abbreviation: "prj", route: Route::Projects },
    CommandDef { name: "users", abbreviation: "usr", route: Route::Users },
    CommandDef { name: "aggregates", abbreviation: "agg", route: Route::Aggregates },
    CommandDef { name: "hypervisors", abbreviation: "hyp", route: Route::Hosts },
    CommandDef { name: "hosts", abbreviation: "host", route: Route::Hosts },
    CommandDef { name: "migrations", abbreviation: "mig", route: Route::Migrations },
    CommandDef { name: "snapshots", abbreviation: "snap", route: Route::Snapshots },
    CommandDef { name: "compute-services", abbreviation: "svc", route: Route::ComputeServices },
    CommandDef { name: "agents", abbreviation: "agt", route: Route::Agents },
    CommandDef { name: "usage", abbreviation: "usg", route: Route::Usage },
];

fn build_abbreviations() -> HashMap<String, String> {
    COMMAND_TABLE
        .iter()
        .map(|def| (def.abbreviation.to_string(), def.name.to_string()))
        .collect()
}

fn build_route_map() -> HashMap<String, Route> {
    COMMAND_TABLE
        .iter()
        .map(|def| (def.name.to_string(), def.route))
        .collect()
}

pub struct CommandParser {
    abbreviations: HashMap<String, String>,
    route_map: HashMap<String, Route>,
    history: CommandHistory,
    completions: Vec<String>,
    completion_index: usize,
    last_prefix: Option<String>,
}

impl CommandParser {
    pub fn new(history_path: PathBuf) -> Self {
        Self {
            abbreviations: build_abbreviations(),
            route_map: build_route_map(),
            history: CommandHistory::new(history_path, 50),
            completions: Vec::new(),
            completion_index: 0,
            last_prefix: None,
        }
    }

    /// Parse a command string. Resolves abbreviations first.
    pub fn parse(&mut self, input: &str) -> Command {
        let trimmed = input.trim();
        if trimmed.is_empty() {
            return Command::Unknown(String::new());
        }

        let parts: Vec<&str> = trimmed.splitn(2, ' ').collect();
        let cmd = parts[0].to_lowercase();
        let arg = parts.get(1).map(|s| s.trim().to_string());

        // Resolve abbreviation
        let resolved = self
            .abbreviations
            .get(&cmd)
            .cloned()
            .unwrap_or_else(|| cmd.clone());

        // System commands
        match resolved.as_str() {
            "q" | "quit" => return Command::Quit,
            "refresh" => return Command::Refresh,
            "help" => return Command::Help,
            "ctx" => {
                return match arg {
                    Some(cloud) if !cloud.is_empty() => Command::ContextSwitch(cloud),
                    _ => Command::ContextList,
                };
            }
            _ => {}
        }

        // Route navigation
        if let Some(route) = self.route_map.get(&resolved) {
            return Command::Navigate(*route);
        }

        Command::Unknown(trimmed.to_string())
    }

    /// Tab auto-complete. Returns the expanded command name (not the abbreviation).
    /// First Tab: collect matching commands by prefix, return first.
    /// Subsequent Tabs with same prefix: cycle through matches.
    /// If the prefix exactly matches an abbreviation, its expanded form is included.
    pub fn auto_complete(&mut self, prefix: &str) -> Option<String> {
        let prefix_lower = prefix.to_lowercase();

        if self.last_prefix.as_deref() != Some(&prefix_lower) {
            self.completions = self
                .available_commands()
                .into_iter()
                .filter(|cmd| cmd.starts_with(&prefix_lower))
                .collect();
            // Include abbreviation expansions
            for (abbr, full) in &self.abbreviations {
                if abbr.starts_with(&prefix_lower) && !self.completions.contains(full) {
                    self.completions.push(full.clone());
                }
            }
            self.completions.sort();
            self.completions.dedup();
            self.completion_index = 0;
            self.last_prefix = Some(prefix_lower);
        } else if !self.completions.is_empty() {
            self.completion_index = (self.completion_index + 1) % self.completions.len();
        }

        self.completions.get(self.completion_index).cloned()
    }

    pub fn reset_completion(&mut self) {
        self.completions.clear();
        self.completion_index = 0;
        self.last_prefix = None;
    }

    pub fn push_history(&mut self, command: &str) {
        self.history.push(command);
    }

    pub fn history_prev(&mut self) -> Option<&str> {
        self.history.prev()
    }

    pub fn history_next(&mut self) -> Option<&str> {
        self.history.next()
    }

    pub fn history_reset_cursor(&mut self) {
        self.history.reset_cursor();
    }

    pub fn save_history(&self) -> Result<()> {
        self.history.save()
    }

    pub fn load_history(&mut self) -> Result<()> {
        self.history.load()
    }

    /// All valid command names (for auto-complete).
    pub fn available_commands(&self) -> Vec<String> {
        let mut cmds: Vec<String> = self.route_map.keys().cloned().collect();
        cmds.extend(["quit", "refresh", "help", "ctx"].iter().map(|s| s.to_string()));
        cmds.sort();
        cmds
    }
}

// --- CommandHistory ---

struct CommandHistory {
    entries: Vec<String>,
    max_size: usize,
    cursor: Option<usize>,
    file_path: PathBuf,
}

impl CommandHistory {
    fn new(file_path: PathBuf, max_size: usize) -> Self {
        Self {
            entries: Vec::new(),
            max_size,
            cursor: None,
            file_path,
        }
    }

    fn push(&mut self, command: &str) {
        let cmd = command.trim().to_string();
        if cmd.is_empty() || cmd.len() > MAX_HISTORY_ENTRY_LEN {
            return;
        }
        self.entries.retain(|e| e != &cmd);
        self.entries.push(cmd);
        if self.entries.len() > self.max_size {
            self.entries.remove(0);
        }
        self.cursor = None;
    }

    fn prev(&mut self) -> Option<&str> {
        if self.entries.is_empty() {
            return None;
        }
        let idx = match self.cursor {
            None => self.entries.len().saturating_sub(1),
            Some(0) => 0,
            Some(c) => c - 1,
        };
        self.cursor = Some(idx);
        self.entries.get(idx).map(|s| s.as_str())
    }

    fn next(&mut self) -> Option<&str> {
        match self.cursor {
            None => None,
            Some(c) => {
                if c + 1 >= self.entries.len() {
                    self.cursor = None;
                    None
                } else {
                    self.cursor = Some(c + 1);
                    self.entries.get(c + 1).map(|s| s.as_str())
                }
            }
        }
    }

    fn reset_cursor(&mut self) {
        self.cursor = None;
    }

    fn save(&self) -> Result<()> {
        if let Some(parent) = self.file_path.parent() {
            fs::create_dir_all(parent).map_err(|e| {
                crate::error::AppError::Other(format!(

exec
/bin/zsh -lc "sed -n '1,260p' src/input/keymap.rs" in /Users/jay.ahn/projects/infra/nexttui/.worktrees/runtime-context-switch
 succeeded in 0ms:
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum AppMode {
    Normal,
    Command,
    Search,
    Form,
    Dialog,
}

#[derive(Debug, Clone, PartialEq)]
pub enum KeyAction {
    MoveUp,
    MoveDown,
    MoveToTop,
    MoveToBottom,
    PageUp,
    PageDown,
    Select,
    Back,

    EnterCommandMode,
    EnterSearchMode,
    ToggleSidebar,

    Create,
    Delete,
    Edit,
    Refresh,

    NextField,
    PrevField,
    ToggleField,
    SubmitForm,
    CancelForm,

    Confirm,
    Deny,

    Quit,
    ForceQuit,

    CharInput(char),
    Unmapped,
}

pub struct KeyMap;

impl KeyMap {
    pub fn new() -> Self {
        Self
    }

    /// Resolve a key event to a semantic action based on current mode.
    pub fn resolve(&self, mode: AppMode, key: KeyEvent) -> KeyAction {
        // Force quit always works
        if key.code == KeyCode::Char('c') && key.modifiers.contains(KeyModifiers::CONTROL) {
            return KeyAction::ForceQuit;
        }

        match mode {
            AppMode::Normal => self.resolve_normal(key),
            AppMode::Command => self.resolve_command(key),
            AppMode::Search => self.resolve_search(key),
            AppMode::Form => self.resolve_form(key),
            AppMode::Dialog => self.resolve_dialog(key),
        }
    }

    /// Generate context help string for status bar.
    pub fn context_help(&self, mode: AppMode) -> String {
        match mode {
            AppMode::Normal => "j/k:move  Enter:select  /:search  ::cmd  Tab:sidebar  q:quit".into(),
            AppMode::Command => "Enter:run  Tab:complete  Up/Down:history  Esc:cancel".into(),
            AppMode::Search => "Enter:apply  Esc:cancel  (type to filter)".into(),
            AppMode::Form => "Tab:next  Shift+Tab:prev  Enter:submit  Esc:cancel".into(),
            AppMode::Dialog => "y:confirm  n/Esc:deny".into(),
        }
    }

    fn resolve_normal(&self, key: KeyEvent) -> KeyAction {
        match key.code {
            KeyCode::Char('j') | KeyCode::Down => KeyAction::MoveDown,
            KeyCode::Char('k') | KeyCode::Up => KeyAction::MoveUp,
            KeyCode::Char('g') => KeyAction::MoveToTop,
            KeyCode::Char('G') => KeyAction::MoveToBottom,
            KeyCode::PageUp => KeyAction::PageUp,
            KeyCode::PageDown => KeyAction::PageDown,
            KeyCode::Char('u') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                KeyAction::PageUp
            }
            KeyCode::Char('d') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                KeyAction::PageDown
            }
            KeyCode::Enter => KeyAction::Select,
            KeyCode::Esc => KeyAction::Back,
            KeyCode::Char(':') => KeyAction::EnterCommandMode,
            KeyCode::Char('/') => KeyAction::EnterSearchMode,
            KeyCode::Tab => KeyAction::ToggleSidebar,
            KeyCode::Char('c') => KeyAction::Create,
            KeyCode::Char('d') => KeyAction::Delete,
            KeyCode::Char('e') => KeyAction::Edit,
            KeyCode::Char('r') => KeyAction::Refresh,
            KeyCode::Char('q') => KeyAction::Quit,
            _ => KeyAction::Unmapped,
        }
    }

    fn resolve_command(&self, key: KeyEvent) -> KeyAction {
        match key.code {
            KeyCode::Enter => KeyAction::Select,
            KeyCode::Esc => KeyAction::Back,
            KeyCode::Tab => KeyAction::NextField,
            KeyCode::Up => KeyAction::MoveUp,
            KeyCode::Down => KeyAction::MoveDown,
            KeyCode::Char(c) => KeyAction::CharInput(c),
            KeyCode::Backspace => KeyAction::CharInput('\x08'),
            _ => KeyAction::Unmapped,
        }
    }

    fn resolve_search(&self, key: KeyEvent) -> KeyAction {
        match key.code {
            KeyCode::Enter => KeyAction::Select,
            KeyCode::Esc => KeyAction::Back,
            KeyCode::Char(c) => KeyAction::CharInput(c),
            KeyCode::Backspace => KeyAction::CharInput('\x08'),
            _ => KeyAction::Unmapped,
        }
    }

    fn resolve_form(&self, key: KeyEvent) -> KeyAction {
        match key.code {
            KeyCode::Tab => KeyAction::NextField,
            KeyCode::BackTab => KeyAction::PrevField,
            KeyCode::Enter => KeyAction::SubmitForm,
            KeyCode::Esc => KeyAction::CancelForm,
            KeyCode::Char(' ') => KeyAction::ToggleField,
            KeyCode::Char('j') | KeyCode::Down => KeyAction::MoveDown,
            KeyCode::Char('k') | KeyCode::Up => KeyAction::MoveUp,
            KeyCode::Char(c) => KeyAction::CharInput(c),
            KeyCode::Backspace => KeyAction::CharInput('\x08'),
            _ => KeyAction::Unmapped,
        }
    }

    fn resolve_dialog(&self, key: KeyEvent) -> KeyAction {
        match key.code {
            KeyCode::Char('y') | KeyCode::Char('Y') => KeyAction::Confirm,
            KeyCode::Char('n') | KeyCode::Char('N') => KeyAction::Deny,
            KeyCode::Esc => KeyAction::Deny,
            KeyCode::Enter => KeyAction::Confirm,
            KeyCode::Char(c) => KeyAction::CharInput(c),
            KeyCode::Backspace => KeyAction::CharInput('\x08'),
            _ => KeyAction::Unmapped,
        }
    }
}

impl Default for KeyMap {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn key(code: KeyCode) -> KeyEvent {
        KeyEvent::from(code)
    }

    fn key_with_mod(code: KeyCode, modifiers: KeyModifiers) -> KeyEvent {
        KeyEvent::new(code, modifiers)
    }

    #[test]
    fn test_normal_mode_navigation() {
        let km = KeyMap::new();
        assert_eq!(km.resolve(AppMode::Normal, key(KeyCode::Char('j'))), KeyAction::MoveDown);
        assert_eq!(km.resolve(AppMode::Normal, key(KeyCode::Char('k'))), KeyAction::MoveUp);
        assert_eq!(km.resolve(AppMode::Normal, key(KeyCode::Char('g'))), KeyAction::MoveToTop);
        assert_eq!(km.resolve(AppMode::Normal, key(KeyCode::Char('G'))), KeyAction::MoveToBottom);
        assert_eq!(km.resolve(AppMode::Normal, key(KeyCode::Enter)), KeyAction::Select);
        assert_eq!(km.resolve(AppMode::Normal, key(KeyCode::Esc)), KeyAction::Back);
    }

    #[test]
    fn test_normal_mode_switching() {
        let km = KeyMap::new();
        assert_eq!(km.resolve(AppMode::Normal, key(KeyCode::Char(':'))), KeyAction::EnterCommandMode);
        assert_eq!(km.resolve(AppMode::Normal, key(KeyCode::Char('/'))), KeyAction::EnterSearchMode);
        assert_eq!(km.resolve(AppMode::Normal, key(KeyCode::Tab)), KeyAction::ToggleSidebar);
    }

    #[test]
    fn test_normal_mode_actions() {
        let km = KeyMap::new();
        assert_eq!(km.resolve(AppMode::Normal, key(KeyCode::Char('c'))), KeyAction::Create);
        assert_eq!(km.resolve(AppMode::Normal, key(KeyCode::Char('r'))), KeyAction::Refresh);
        assert_eq!(km.resolve(AppMode::Normal, key(KeyCode::Char('q'))), KeyAction::Quit);
    }

    #[test]
    fn test_command_mode() {
        let km = KeyMap::new();
        assert_eq!(km.resolve(AppMode::Command, key(KeyCode::Enter)), KeyAction::Select);
        assert_eq!(km.resolve(AppMode::Command, key(KeyCode::Esc)), KeyAction::Back);
        assert_eq!(km.resolve(AppMode::Command, key(KeyCode::Tab)), KeyAction::NextField);
        assert_eq!(km.resolve(AppMode::Command, key(KeyCode::Char('a'))), KeyAction::CharInput('a'));
    }

    #[test]
    fn test_search_mode() {
        let km = KeyMap::new();
        assert_eq!(km.resolve(AppMode::Search, key(KeyCode::Enter)), KeyAction::Select);
        assert_eq!(km.resolve(AppMode::Search, key(KeyCode::Esc)), KeyAction::Back);
        assert_eq!(km.resolve(AppMode::Search, key(KeyCode::Char('w'))), KeyAction::CharInput('w'));
    }

    #[test]
    fn test_force_quit_any_mode() {
        let km = KeyMap::new();
        let ctrl_c = key_with_mod(KeyCode::Char('c'), KeyModifiers::CONTROL);
        assert_eq!(km.resolve(AppMode::Normal, ctrl_c), KeyAction::ForceQuit);
        assert_eq!(km.resolve(AppMode::Command, ctrl_c), KeyAction::ForceQuit);
        assert_eq!(km.resolve(AppMode::Form, ctrl_c), KeyAction::ForceQuit);
    }

    #[test]
    fn test_context_help() {
        let km = KeyMap::new();
        let help = km.context_help(AppMode::Normal);
        assert!(help.contains("j/k"));
        assert!(help.contains("quit"));

        let help = km.context_help(AppMode::Command);
        assert!(help.contains("Tab"));
        assert!(help.contains("history"));
    }
}

exec
/bin/zsh -lc "sed -n '1,260p' src/ui/status_bar.rs" in /Users/jay.ahn/projects/infra/nexttui/.worktrees/runtime-context-switch
 succeeded in 0ms:
use ratatui::layout::Rect;
use ratatui::style::{Color, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::Paragraph;
use ratatui::Frame;

use super::theme;

pub struct StatusInfo {
    pub panel_name: String,
    pub item_count: Option<usize>,
    pub selected_index: Option<usize>,
    pub context_hints: Vec<(String, String)>,
    pub error_badge_count: usize,
}

impl StatusInfo {
    /// Build left-side text: `[PanelName] idx/count` or `[PanelName]`.
    pub fn left_text(&self) -> String {
        if let (Some(count), Some(idx)) = (self.item_count, self.selected_index) {
            format!("[{}] {}/{}", self.panel_name, idx + 1, count)
        } else {
            format!("[{}]", self.panel_name)
        }
    }
}

pub struct StatusBar;

impl StatusBar {
    pub fn new() -> Self {
        Self
    }

    pub fn render(
        &self,
        frame: &mut Frame,
        area: Rect,
        info: &StatusInfo,
    ) {
        // Paragraph bg applies to spans without explicit bg (ratatui style merge)
        let bg = Style::default().bg(Color::DarkGray).fg(Color::White);
        let left = info.left_text();

        // Error badge: " ⚠N" in red after left text
        let badge = if info.error_badge_count > 0 {
            format!(" \u{26A0}{}", info.error_badge_count)
        } else {
            String::new()
        };
        // Use char count for display width (⚠ is 1 column in most terminals)
        let badge_len = badge.chars().count();

        // Right: key hints using theme::key_hint()
        let mut hint_spans: Vec<Span> = Vec::new();
        for (i, (key, desc)) in info.context_hints.iter().enumerate() {
            if i > 0 {
                hint_spans.push(Span::raw("  "));
            }
            hint_spans.extend(theme::key_hint(key, desc));
        }
        let hint_plain_len: usize = hint_spans.iter().map(|s| s.content.len()).sum();

        let padding_len = (area.width as usize)
            .saturating_sub(left.len())
            .saturating_sub(badge_len)
            .saturating_sub(hint_plain_len);
        let padding = " ".repeat(padding_len);

        let mut spans = vec![
            Span::styled(&left, bg),
        ];
        if info.error_badge_count > 0 {
            spans.push(Span::styled(
                badge,
                Style::default().bg(Color::DarkGray).fg(Color::Red),
            ));
        }
        spans.push(Span::styled(padding, bg));
        spans.extend(hint_spans);

        let line = Line::from(spans);
        let widget = Paragraph::new(line).style(bg);
        frame.render_widget(widget, area);
    }
}

impl Default for StatusBar {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_info() -> StatusInfo {
        StatusInfo {
            panel_name: "Servers".to_string(),
            item_count: Some(5),
            selected_index: Some(2),
            context_hints: vec![
                ("j/k".into(), "이동".into()),
                ("Enter".into(), "상세".into()),
            ],
            error_badge_count: 0,
        }
    }

    #[test]
    fn test_status_info_new_fields() {
        let info = sample_info();
        assert_eq!(info.panel_name, "Servers");
        assert_eq!(info.item_count, Some(5));
        assert_eq!(info.selected_index, Some(2));
        assert_eq!(info.context_hints.len(), 2);
        assert_eq!(info.context_hints[0], ("j/k".into(), "이동".into()));
    }

    #[test]
    fn test_status_info_left_text_with_count() {
        let info = sample_info();
        assert_eq!(info.left_text(), "[Servers] 3/5");
    }

    #[test]
    fn test_status_info_left_text_without_count() {
        let info = StatusInfo {
            panel_name: "Flavors".to_string(),
            item_count: None,
            selected_index: None,
            context_hints: vec![],
            error_badge_count: 0,
        };
        assert_eq!(info.left_text(), "[Flavors]");
    }

    #[test]
    fn test_error_badge_count_zero_no_badge_text() {
        let info = sample_info();
        let left = info.left_text();
        // With error_badge_count=0, no badge should appear in left text
        assert!(!left.contains('⚠'));
    }

    #[test]
    fn test_error_badge_count_positive_shows_badge() {
        let mut info = sample_info();
        info.error_badge_count = 3;
        // The badge is rendered in StatusBar::render, not in left_text.
        // We test the render output by checking the spans.
        // Use a buffer to capture render output.
        use ratatui::backend::TestBackend;
        use ratatui::Terminal;

        let backend = TestBackend::new(80, 1);
        let mut terminal = Terminal::new(backend).ok();
        if let Some(ref mut term) = terminal {
            let _ = term.draw(|frame| {
                let area = frame.area();
                let bar = StatusBar::new();
                StatusBar::render(&bar, frame, area, &info);
            });
            let buf = term.backend().buffer().clone();
            let content: String = (0..buf.area.width)
                .filter_map(|x| {
                    let cell = &buf[(x, 0)];
                    Some(cell.symbol().to_string())
                })
                .collect();
            assert!(content.contains('⚠'), "badge should appear in rendered output: {content}");
            assert!(content.contains('3'), "badge count should appear: {content}");
        }
    }

    #[test]
    fn test_status_bar_key_hint_integration() {
        let spans = theme::key_hint("Tab", "패널");
        assert_eq!(spans.len(), 3); // key + separator + desc
        assert_eq!(spans[0].style.fg, Some(ratatui::style::Color::Cyan));
    }
}

exec
/bin/zsh -lc "sed -n '1,260p' src/ui/confirm.rs" in /Users/jay.ahn/projects/infra/nexttui/.worktrees/runtime-context-switch
 succeeded in 0ms:
use crossterm::event::{KeyCode, KeyEvent};
use ratatui::layout::{Alignment, Rect};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, BorderType, Borders, Clear, Paragraph};

use super::theme::Theme;
use ratatui::Frame;

const MAX_BUFFER_LEN: usize = 256;

/// Confirm dialog mode.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ConfirmMode {
    /// Simple Y/N confirmation.
    YesNo { message: String },
    /// Type resource name to confirm (destructive actions).
    TypeToConfirm {
        message: String,
        expected: String,
        buffer: String,
    },
}

pub enum ConfirmResult {
    Confirmed,
    Cancelled,
    Pending,
}

pub struct ConfirmDialog {
    mode: ConfirmMode,
    active: bool,
    detail_lines: Vec<String>,
}

impl ConfirmDialog {
    pub fn yes_no(message: impl Into<String>) -> Self {
        Self {
            mode: ConfirmMode::YesNo {
                message: message.into(),
            },
            active: true,
            detail_lines: Vec::new(),
        }
    }

    pub fn yes_no_with_details(message: impl Into<String>, details: Vec<String>) -> Self {
        Self {
            mode: ConfirmMode::YesNo {
                message: message.into(),
            },
            active: true,
            detail_lines: details,
        }
    }

    pub fn type_to_confirm(message: impl Into<String>, expected: impl Into<String>) -> Self {
        Self {
            mode: ConfirmMode::TypeToConfirm {
                message: message.into(),
                expected: expected.into(),
                buffer: String::new(),
            },
            active: true,
            detail_lines: Vec::new(),
        }
    }

    pub fn type_to_confirm_with_details(
        message: impl Into<String>,
        expected: impl Into<String>,
        details: Vec<String>,
    ) -> Self {
        Self {
            mode: ConfirmMode::TypeToConfirm {
                message: message.into(),
                expected: expected.into(),
                buffer: String::new(),
            },
            active: true,
            detail_lines: details,
        }
    }

    pub fn is_active(&self) -> bool {
        self.active
    }

    pub fn message(&self) -> &str {
        match &self.mode {
            ConfirmMode::YesNo { message } => message,
            ConfirmMode::TypeToConfirm { message, .. } => message,
        }
    }

    pub fn detail_lines(&self) -> &[String] {
        &self.detail_lines
    }

    pub fn handle_key(&mut self, key: KeyEvent) -> ConfirmResult {
        if !self.active {
            return ConfirmResult::Pending;
        }

        match &mut self.mode {
            ConfirmMode::YesNo { .. } => match key.code {
                KeyCode::Char('y') | KeyCode::Char('Y') => {
                    self.active = false;
                    ConfirmResult::Confirmed
                }
                KeyCode::Char('n') | KeyCode::Char('N') | KeyCode::Esc => {
                    self.active = false;
                    ConfirmResult::Cancelled
                }
                _ => ConfirmResult::Pending,
            },
            ConfirmMode::TypeToConfirm {
                expected, buffer, ..
            } => match key.code {
                KeyCode::Esc => {
                    self.active = false;
                    ConfirmResult::Cancelled
                }
                KeyCode::Enter => {
                    if buffer == expected {
                        self.active = false;
                        ConfirmResult::Confirmed
                    } else {
                        ConfirmResult::Pending
                    }
                }
                KeyCode::Backspace => {
                    buffer.pop();
                    ConfirmResult::Pending
                }
                KeyCode::Char(c) => {
                    if buffer.len() < MAX_BUFFER_LEN {
                        buffer.push(c);
                    }
                    ConfirmResult::Pending
                }
                _ => ConfirmResult::Pending,
            },
        }
    }

    /// Render the confirm dialog as a centered modal overlay.
    pub fn render(&self, frame: &mut Frame, area: Rect) {
        if !self.active {
            return;
        }

        // Calculate centered modal area (50% width, dynamic height)
        let width = (area.width / 2).max(30).min(area.width);
        let detail_count = self.detail_lines.len() as u16;
        let height = (7u16 + detail_count).min(area.height);
        let x = area.x + (area.width.saturating_sub(width)) / 2;
        let y = area.y + (area.height.saturating_sub(height)) / 2;
        let modal_area = Rect::new(x, y, width, height);

        frame.render_widget(Clear, modal_area);

        let detail_style = Style::default().fg(Color::DarkGray).add_modifier(Modifier::DIM);

        let lines = match &self.mode {
            ConfirmMode::YesNo { message } => {
                let mut l = vec![
                    Line::from(""),
                    Line::from(Span::styled(
                        message.as_str(),
                        Theme::warning().add_modifier(Modifier::BOLD),
                    )),
                ];
                for detail in &self.detail_lines {
                    l.push(Line::from(Span::styled(detail.as_str(), detail_style)));
                }
                l.push(Line::from(""));
                l.push(Line::from(vec![
                    Span::styled("[Y]", Theme::focus_border().add_modifier(Modifier::BOLD)),
                    Span::styled("es  ", Style::default().fg(Color::White)),
                    Span::styled("[N]", Theme::focus_border().add_modifier(Modifier::BOLD)),
                    Span::styled("o", Style::default().fg(Color::White)),
                ]));
                l
            }
            ConfirmMode::TypeToConfirm {
                message,
                expected,
                buffer,
                ..
            } => {
                let mut l = vec![
                    Line::from(Span::styled(
                        message.as_str(),
                        Theme::warning().add_modifier(Modifier::BOLD),
                    )),
                ];
                for detail in &self.detail_lines {
                    l.push(Line::from(Span::styled(detail.as_str(), detail_style)));
                }
                l.push(Line::from(format!("Type '{expected}' to confirm:")));
                l.push(Line::from(""));
                l.push(Line::from(vec![
                    Span::raw("> "),
                    Span::styled(
                        buffer.as_str(),
                        Style::default().fg(Color::White),
                    ),
                    Span::styled("_", Theme::waiting()),
                ]));
                l
            }
        };

        let block = Block::default()
            .title(" Confirm ")
            .borders(Borders::ALL)
            .border_type(BorderType::Rounded)
            .border_style(Theme::warning().add_modifier(Modifier::BOLD))
            .style(Style::default().bg(Color::Rgb(30, 30, 40)));
        let widget = Paragraph::new(lines)
            .block(block)
            .alignment(Alignment::Center)
            .style(Style::default().bg(Color::Rgb(30, 30, 40)));
        frame.render_widget(widget, modal_area);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn key(code: KeyCode) -> KeyEvent {
        KeyEvent::from(code)
    }

    #[test]
    fn test_yes_no_confirm() {
        let mut dialog = ConfirmDialog::yes_no("Delete server?");
        assert!(dialog.is_active());
        let result = dialog.handle_key(key(KeyCode::Char('y')));
        assert!(matches!(result, ConfirmResult::Confirmed));
        assert!(!dialog.is_active());
    }

    #[test]
    fn test_yes_no_cancel() {
        let mut dialog = ConfirmDialog::yes_no("Delete server?");
        let result = dialog.handle_key(key(KeyCode::Char('n')));
        assert!(matches!(result, ConfirmResult::Cancelled));
    }

    #[test]
    fn test_type_to_confirm_success() {
        let mut dialog = ConfirmDialog::type_to_confirm("Type 'web-01' to delete", "web-01");
        for c in "web-01".chars() {
            let result = dialog.handle_key(key(KeyCode::Char(c)));
            assert!(matches!(result, ConfirmResult::Pending));
        }

exec
/bin/zsh -lc "sed -n '1,260p' src/adapter/auth/token_cache.rs" in /Users/jay.ahn/projects/infra/nexttui/.worktrees/runtime-context-switch
 succeeded in 0ms:
//! Token cache persistence: save/load Keystone tokens to disk.
//!
//! Cache layout: `~/.cache/nexttui/auth/{cloud_key}/{scope_key}`
//! File permissions: 0o600 (Unix only)

use std::collections::HashMap;
use std::path::{Path, PathBuf};

use crate::port::types::{Token, TokenScope};

/// Compute a deterministic cache key from cloud config fields.
/// Uses a simple FNV-1a 64-bit hash (stable across Rust versions, no external deps).
pub fn compute_cloud_key(auth_url: &str, username: &str) -> String {
    let input = format!("{auth_url}|{username}");
    let hash = fnv1a_64(input.as_bytes());
    format!("{hash:016x}")
}

/// FNV-1a 64-bit hash — deterministic, no external dependency.
fn fnv1a_64(data: &[u8]) -> u64 {
    const FNV_OFFSET: u64 = 0xcbf29ce484222325;
    const FNV_PRIME: u64 = 0x00000100000001B3;
    let mut hash = FNV_OFFSET;
    for &byte in data {
        hash ^= byte as u64;
        hash = hash.wrapping_mul(FNV_PRIME);
    }
    hash
}

/// Resolve the cache directory path for a given cloud key.
pub fn cache_dir_path(cloud_key: &str) -> PathBuf {
    dirs::cache_dir()
        .unwrap_or_else(|| PathBuf::from("/tmp"))
        .join("nexttui")
        .join("auth")
        .join(cloud_key)
}

/// Save a token to the cache directory, keyed by scope.
/// Creates parent directories if needed.
/// On Unix, creates the file with 0o600 permissions atomically (no TOCTOU window).
pub fn save_token(token: &Token, cache_dir: &Path, scope: &TokenScope) -> Result<(), std::io::Error> {
    use std::io::Write;

    // Create cache directory with restricted permissions on Unix (0o700)
    #[cfg(unix)]
    {
        use std::fs::DirBuilder;
        use std::os::unix::fs::DirBuilderExt;
        DirBuilder::new().recursive(true).mode(0o700).create(cache_dir)?;
    }
    #[cfg(not(unix))]
    {
        std::fs::create_dir_all(cache_dir)?;
    }
    let path = cache_dir.join(scope.cache_key());
    let data = serde_json::to_vec(token)
        .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e))?;

    // NOTE: Token ID is stored in plaintext JSON. File permissions (0o600) provide
    // basic protection. Encryption (AES-GCM / OS keychain) is tracked as BL-P2-016.
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        let mut file = std::fs::OpenOptions::new()
            .write(true)
            .create(true)
            .truncate(true)
            .mode(0o600)
            .open(&path)?;
        file.write_all(&data)?;
    }

    #[cfg(not(unix))]
    {
        std::fs::write(&path, &data)?;
    }

    tracing::debug!(path = %path.display(), "token cached to disk");
    Ok(())
}

/// Load a single token from a cache file.
/// Returns None if the file doesn't exist, is unreadable, or the token is expired.
/// Automatically deletes expired token files.
fn load_token_file(path: &Path) -> Option<Token> {
    let data = std::fs::read(path).ok()?;
    let token: Token = serde_json::from_slice(&data).ok()?;

    if token.expires_at > chrono::Utc::now() + chrono::Duration::minutes(1) {
        Some(token)
    } else {
        tracing::info!(path = %path.display(), "cached token expired, removing");
        let _ = std::fs::remove_file(path);
        None
    }
}

/// Load all valid cached tokens from the cache directory.
/// Returns a map of scope → token. Expired tokens are auto-deleted.
/// Skips non-files and unrecognized filenames.
pub fn load_all_tokens(cache_dir: &Path) -> HashMap<TokenScope, Token> {
    let mut map = HashMap::new();
    let entries = match std::fs::read_dir(cache_dir) {
        Ok(entries) => entries,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return map,
        Err(e) => {
            tracing::warn!(path = %cache_dir.display(), error = %e, "failed to read token cache directory");
            return map;
        }
    };

    for entry in entries.flatten() {
        // Skip non-files (directories, symlinks, etc.)
        if !entry.file_type().map(|t| t.is_file()).unwrap_or(false) {
            continue;
        }

        let file_name = entry.file_name();
        let scope_key = file_name.to_string_lossy();

        let scope = match parse_scope_from_filename(&scope_key) {
            Some(s) => s,
            None => {
                tracing::warn!(filename = %scope_key, "unrecognized token cache file, skipping");
                continue;
            }
        };

        if let Some(token) = load_token_file(&entry.path()) {
            tracing::debug!("loaded cached token from disk");
            map.insert(scope, token);
        }
    }
    map
}

/// Parse a TokenScope from a cache filename.
/// Returns None for unrecognized filenames.
fn parse_scope_from_filename(filename: &str) -> Option<TokenScope> {
    if filename == "unscoped" {
        return Some(TokenScope::Unscoped);
    }
    // Format: "project@{name}@{domain}" (@ separator avoids _ ambiguity)
    if let Some(rest) = filename.strip_prefix("project@") {
        if let Some((name, domain)) = rest.split_once('@') {
            return Some(TokenScope::Project {
                name: name.to_string(),
                domain: domain.to_string(),
            });
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::{Duration, Utc};
    use crate::port::types::*;
    use tempfile::TempDir;

    fn sample_token(expires_in_minutes: i64) -> Token {
        Token {
            id: "tok-test-123".to_string(),
            expires_at: Utc::now() + Duration::minutes(expires_in_minutes),
            project: ProjectScope {
                id: "proj-1".to_string(),
                name: "admin".to_string(),
                domain_id: "default".to_string(),
                domain_name: "Default".to_string(),
            },
            roles: vec![TokenRole {
                id: "role-1".to_string(),
                name: "admin".to_string(),
            }],
            catalog: vec![CatalogEntry {
                service_type: "compute".to_string(),
                service_name: "nova".to_string(),
                endpoints: vec![Endpoint {
                    region: "RegionOne".to_string(),
                    interface: EndpointInterface::Public,
                    url: "https://nova:8774/v2.1".to_string(),
                }],
            }],
        }
    }

    fn sample_scope() -> TokenScope {
        TokenScope::Project {
            name: "admin".to_string(),
            domain: "default".to_string(),
        }
    }

    #[test]
    fn test_compute_cloud_key_deterministic() {
        let k1 = compute_cloud_key("https://keystone:5000/v3", "admin");
        let k2 = compute_cloud_key("https://keystone:5000/v3", "admin");
        assert_eq!(k1, k2);
        assert_eq!(k1.len(), 16);
    }

    #[test]
    fn test_compute_cloud_key_different_inputs() {
        let k1 = compute_cloud_key("https://keystone:5000/v3", "admin");
        let k2 = compute_cloud_key("https://keystone:5000/v3", "user");
        assert_ne!(k1, k2);
    }

    #[test]
    fn test_token_scope_cache_key() {
        let scope = TokenScope::Project {
            name: "admin".to_string(),
            domain: "default".to_string(),
        };
        assert_eq!(scope.cache_key(), "project@admin@default");
        assert_eq!(TokenScope::Unscoped.cache_key(), "unscoped");
    }

    #[test]
    fn test_cache_key_sanitizes_path_traversal() {
        let scope = TokenScope::Project {
            name: "../etc".to_string(),
            domain: "default".to_string(),
        };
        // dots and slashes should be replaced with _
        assert!(!scope.cache_key().contains('/'));
        assert!(!scope.cache_key().contains(".."));
    }

    #[test]
    fn test_cache_key_handles_underscore_in_name() {
        let scope = TokenScope::Project {
            name: "my_project".to_string(),
            domain: "my_domain".to_string(),
        };
        // @ separator means underscores in name/domain are preserved correctly
        assert_eq!(scope.cache_key(), "project@my_project@my_domain");
    }

    #[test]
    fn test_save_and_load_scoped_token() {
        let dir = TempDir::new().unwrap();
        let cache_dir = dir.path().join("cloud-abc");
        let scope = sample_scope();

        let token = sample_token(60);
        save_token(&token, &cache_dir, &scope).unwrap();

        let loaded = load_all_tokens(&cache_dir);
        assert_eq!(loaded.len(), 1);
        let loaded_token = loaded.get(&scope).unwrap();
        assert_eq!(loaded_token.id, "tok-test-123");
    }

    #[test]
    fn test_save_multiple_scopes() {
        let dir = TempDir::new().unwrap();

exec
/bin/zsh -lc "sed -n '1,300p' src/module/project/mod.rs" in /Users/jay.ahn/projects/infra/nexttui/.worktrees/runtime-context-switch
 succeeded in 0ms:
pub mod view_model;

use crossterm::event::{KeyCode, KeyEvent};
use ratatui::layout::Rect;
use ratatui::Frame;
use tokio::sync::mpsc;

use crate::action::Action;
use crate::component::Component;
use crate::event::AppEvent;
use crate::models::keystone::Project;
use crate::module::{ConfirmHandler, PendingAction, ViewState};
use crate::port::types::ProjectCreateParams;
use crate::ui::confirm::ConfirmDialog;
use crate::ui::form::{FormAction, FormWidget, SelectOption};
use crate::ui::resource_list::{ResourceList, Row};

use self::view_model::{project_columns, project_create_defs, project_detail_data, project_to_row};

pub struct ProjectModule {
    view_state: ViewState,
    projects: Vec<Project>,
    #[allow(dead_code)]
    loading: bool,
    error_message: Option<String>,
    confirm: ConfirmHandler,
    resource_list: ResourceList,
    form: Option<FormWidget>,
    cached_domain_opts: Vec<SelectOption>,
    action_tx: mpsc::UnboundedSender<Action>,
}

impl ProjectModule {
    pub fn new(action_tx: mpsc::UnboundedSender<Action>) -> Self {
        Self {
            view_state: ViewState::List,
            projects: Vec::new(),
            loading: false,
            error_message: None,
            confirm: ConfirmHandler::new(),
            resource_list: ResourceList::new(project_columns()),
            form: None,
            cached_domain_opts: Vec::new(),
            action_tx,
        }
    }

    pub fn view_state(&self) -> &ViewState { &self.view_state }
    pub fn projects(&self) -> &[Project] { &self.projects }
    pub fn selected_index(&self) -> usize { self.resource_list.selected_index() }
    pub fn error_message(&self) -> Option<&str> { self.error_message.as_deref() }

    fn selected_project(&self) -> Option<&Project> {
        self.projects.get(self.resource_list.selected_index())
    }

    fn rows(&self) -> Vec<Row> {
        self.projects.iter().map(project_to_row).collect()
    }

    fn resolve_action(pending: PendingAction) -> Option<Action> {
        match pending {
            PendingAction::DeleteProject { id, .. } => Some(Action::DeleteProject { id }),
            _ => None,
        }
    }

    fn open_create_form(&mut self) {
        let defs = project_create_defs();
        let mut form = FormWidget::new("Create Project", defs);
        if !self.cached_domain_opts.is_empty() {
            form.set_field_options("Domain", self.cached_domain_opts.clone());
        }
        self.form = Some(form);
        self.view_state = ViewState::Create;
    }

    fn close_form(&mut self) {
        self.form = None;
        self.view_state = ViewState::List;
    }

    fn handle_list_key(&mut self, key: KeyEvent) -> Option<Action> {
        if self.resource_list.handle_nav_key(key) { return None; }
        match key.code {
            KeyCode::Enter => {
                if let Some(proj) = self.selected_project() {
                    self.view_state = ViewState::Detail(proj.id.clone());
                }
                None
            }
            KeyCode::Char('c') => { self.open_create_form(); Some(Action::EnterFormMode) }
            KeyCode::Char('d') => {
                if let Some(proj) = self.selected_project() {
                    let id = proj.id.clone();
                    let name = proj.name.clone();
                    self.confirm.open(
                        ConfirmDialog::type_to_confirm(
                            format!("Delete project '{name}'?"),
                            name.clone(),
                        ),
                        PendingAction::DeleteProject { id, name },
                    );
                }
                None
            }
            KeyCode::Char('r') => Some(Action::FetchProjects),
            KeyCode::Left => Some(Action::FocusSidebar),
            KeyCode::Esc => Some(Action::Back),
            _ => None,
        }
    }

    fn handle_detail_key(&mut self, key: KeyEvent) -> Option<Action> {
        match key.code {
            KeyCode::Esc | KeyCode::Char('q') | KeyCode::Left => { self.view_state = ViewState::List; None }
            _ => None,
        }
    }

    fn handle_create_key(&mut self, key: KeyEvent) -> Option<Action> {
        let Some(form) = self.form.as_mut() else {
            self.close_form();
            return None;
        };

        match form.handle_key(key) {
            FormAction::Submit(values) => {
                let name = values
                    .get("Name")
                    .and_then(|v| match v {
                        crate::ui::form::FormValue::Text(s) => Some(s.clone()),
                        _ => None,
                    })
                    .unwrap_or_default();
                let description = values
                    .get("Description")
                    .and_then(|v| match v {
                        crate::ui::form::FormValue::Text(s) => {
                            if s.is_empty() { None } else { Some(s.clone()) }
                        }
                        _ => None,
                    });
                let domain_id = values
                    .get("Domain")
                    .and_then(|v| match v {
                        crate::ui::form::FormValue::Text(s) => Some(s.clone()),
                        _ => None,
                    })
                    .unwrap_or_else(|| "default".to_string());
                let enabled = values
                    .get("Enabled")
                    .and_then(|v| match v {
                        crate::ui::form::FormValue::Bool(b) => Some(*b),
                        _ => None,
                    });

                self.close_form();
                let _ = self.action_tx.send(Action::CreateProject(ProjectCreateParams {
                    name,
                    description,
                    domain_id,
                    enabled,
                }));
                Some(Action::ExitFormMode)
            }
            FormAction::Cancel => {
                self.close_form();
                Some(Action::ExitFormMode)
            }
            FormAction::None => None,
        }
    }
}

impl Component for ProjectModule {
    fn refresh_action(&self) -> Option<Action> { Some(Action::FetchProjects) }
    fn is_modal(&self) -> bool { self.confirm.is_active() || self.form.is_some() }

    fn handle_key(&mut self, key: KeyEvent) -> Option<Action> {
        if let Some(result) = self.confirm.handle_key(key, Self::resolve_action) { return result; }
        match &self.view_state {
            ViewState::List => self.handle_list_key(key),
            ViewState::Detail(_) => self.handle_detail_key(key),
            ViewState::Create => self.handle_create_key(key),
        }
    }

    fn handle_event(&mut self, event: &AppEvent) {
        match event {
            AppEvent::ProjectsLoaded(projects) => {
                self.projects = projects.clone();
                self.loading = false;
                self.error_message = None;
                let rows = self.rows();
                self.resource_list.set_rows(rows);
                // Build domain dropdown options from loaded projects
                let mut domain_ids: Vec<String> = projects
                    .iter()
                    .filter_map(|p| p.domain_id.clone())
                    .collect();
                domain_ids.sort();
                domain_ids.dedup();
                self.cached_domain_opts = domain_ids
                    .into_iter()
                    .map(|d| SelectOption { value: d.clone(), display: d })
                    .collect();
            }
            AppEvent::ProjectCreated(_) => {
                self.view_state = ViewState::List;
                let _ = self.action_tx.send(Action::FetchProjects);
            }
            AppEvent::ProjectDeleted { .. } => {
                let _ = self.action_tx.send(Action::FetchProjects);
            }
            AppEvent::ApiError { operation, message, .. } => {
                self.error_message = Some(format!("{operation}: {message}"));
                self.loading = false;
            }
            _ => {}
        }
    }

    fn render(&self, frame: &mut Frame, area: Rect) {
        match &self.view_state {
            ViewState::List => self.resource_list.render(frame, area),
            ViewState::Detail(id) => {
                if let Some(proj) = self.projects.iter().find(|p| p.id == *id) {
                    let data = project_detail_data(proj);
                    let mut dv = crate::ui::detail_view::DetailView::new();
                    dv.set_data(data);
                    dv.render(frame, area);
                }
            }
            ViewState::Create => {
                if let Some(form) = &self.form {
                    form.render(frame, area);
                } else {
                    self.resource_list.render(frame, area);
                }
            }
        }
        self.confirm.render(frame, area);
    }

    fn content_title(&self) -> Option<String> {
        match &self.view_state {
            ViewState::List => None,
            ViewState::Detail(id) => {
                let name = self.projects.iter()
                    .find(|r| r.id == *id)
                    .map(|r| r.name.as_str())
                    .unwrap_or("...");
                Some(format!("Project: {name}"))
            }
            ViewState::Create => Some("Create Project".into()),
        }
    }

    fn help_hint(&self) -> &str {
        match &self.view_state {
            ViewState::List => "Enter:Detail c:Create d:Delete r:Refresh",
            ViewState::Detail(_) => "Esc:Back",
            ViewState::Create => "Esc:Cancel Tab:Next Enter:Submit",
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn key(code: KeyCode) -> KeyEvent { KeyEvent::from(code) }
    fn make_project(id: &str, name: &str) -> Project {
        Project { id: id.into(), name: name.into(), description: None, enabled: true, domain_id: Some("default".into()) }
    }
    fn setup() -> (ProjectModule, mpsc::UnboundedReceiver<Action>) {
        let (tx, rx) = mpsc::unbounded_channel();
        let mut m = ProjectModule::new(tx);
        m.handle_event(&AppEvent::ProjectsLoaded(vec![
            make_project("p1", "admin"), make_project("p2", "demo"),
        ]));
        (m, rx)
    }

    #[test] fn test_initial_state() { let (tx, _) = mpsc::unbounded_channel(); let m = ProjectModule::new(tx); assert_eq!(*m.view_state(), ViewState::List); }
    #[test] fn test_nav() { let (mut m, _) = setup(); m.handle_key(key(KeyCode::Char('j'))); assert_eq!(m.selected_index(), 1); }
    #[test] fn test_enter_detail() { let (mut m, _) = setup(); m.handle_key(key(KeyCode::Enter)); assert_eq!(*m.view_state(), ViewState::Detail("p1".into())); }
    #[test] fn test_esc_to_list() { let (mut m, _) = setup(); m.handle_key(key(KeyCode::Enter)); m.handle_key(key(KeyCode::Esc)); assert_eq!(*m.view_state(), ViewState::List); }
    #[test] fn test_create() { let (mut m, _) = setup(); m.handle_key(key(KeyCode::Char('c'))); assert_eq!(*m.view_state(), ViewState::Create); assert!(m.form.is_some()); }
    #[test] fn test_delete_confirm() { let (mut m, _) = setup(); m.handle_key(key(KeyCode::Char('d'))); assert!(m.confirm.is_active()); }
    #[test] fn test_confirm_delete() {
        let (mut m, _) = setup();
        m.handle_key(key(KeyCode::Char('d')));
        for c in "admin".chars() { m.handle_key(key(KeyCode::Char(c))); }
        let a = m.handle_key(key(KeyCode::Enter));
        assert!(matches!(a, Some(Action::DeleteProject { .. })));
    }
    #[test] fn test_refresh() { let (mut m, _) = setup(); assert!(matches!(m.handle_key(key(KeyCode::Char('r'))), Some(Action::FetchProjects))); }
    #[test] fn test_event_loaded() {
        let (tx, _) = mpsc::unbounded_channel(); let mut m = ProjectModule::new(tx);

exec
/bin/zsh -lc "sed -n '1,280p' src/adapter/registry.rs" in /Users/jay.ahn/projects/infra/nexttui/.worktrees/runtime-context-switch
 succeeded in 0ms:
use std::sync::Arc;

use crate::adapter::http::cinder::CinderHttpAdapter;
use crate::adapter::http::glance::GlanceHttpAdapter;
use crate::adapter::http::keystone::KeystoneHttpAdapter;
use crate::adapter::http::neutron::NeutronHttpAdapter;
use crate::adapter::http::nova::NovaHttpAdapter;
use crate::port::auth::AuthProvider;
use crate::port::cinder::CinderPort;
use crate::port::error::ApiError;
use crate::port::glance::GlancePort;
use crate::port::keystone::KeystonePort;
use crate::port::neutron::NeutronPort;
use crate::port::nova::NovaPort;

/// AdapterRegistry creates and holds all service adapters.
/// In Phase 1, all adapters use HTTP/REST via BaseHttpClient.
/// In Phase 2, this will support config-based backend selection
/// (e.g., Service Layer gateway instead of direct OpenStack API).
pub struct AdapterRegistry {
    pub nova: Arc<dyn NovaPort>,
    pub neutron: Arc<dyn NeutronPort>,
    pub cinder: Arc<dyn CinderPort>,
    pub glance: Arc<dyn GlancePort>,
    pub keystone: Arc<dyn KeystonePort>,
}

impl AdapterRegistry {
    /// Create all HTTP adapters from the given auth provider and region.
    pub fn new_http(auth: Arc<dyn AuthProvider>, region: Option<String>) -> Result<Self, ApiError> {
        Ok(Self {
            nova: Arc::new(NovaHttpAdapter::new(auth.clone(), region.clone())?),
            neutron: Arc::new(NeutronHttpAdapter::new(auth.clone(), region.clone())?),
            cinder: Arc::new(CinderHttpAdapter::new(auth.clone(), region.clone())?),
            glance: Arc::new(GlanceHttpAdapter::new(auth.clone(), region.clone())?),
            keystone: Arc::new(KeystoneHttpAdapter::new(auth, region)?),
        })
    }

    /// Create registry from mock adapters (for testing).
    #[cfg(test)]
    pub fn new_mock() -> Self {
        use crate::port::mock::*;
        Self {
            nova: Arc::new(MockNovaAdapter),
            neutron: Arc::new(MockNeutronAdapter),
            cinder: Arc::new(MockCinderAdapter),
            glance: Arc::new(MockGlanceAdapter),
            keystone: Arc::new(MockKeystoneAdapter),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_adapter_registry_mock_creation() {
        let registry = AdapterRegistry::new_mock();
        let _nova: &dyn NovaPort = registry.nova.as_ref();
        let _neutron: &dyn NeutronPort = registry.neutron.as_ref();
        let _cinder: &dyn CinderPort = registry.cinder.as_ref();
        let _glance: &dyn GlancePort = registry.glance.as_ref();
        let _keystone: &dyn KeystonePort = registry.keystone.as_ref();
    }

    #[test]
    fn test_registry_adapters_are_send_sync() {
        fn assert_send_sync<T: Send + Sync>() {}
        assert_send_sync::<AdapterRegistry>();
    }
}

exec
/bin/zsh -lc "rg -n \"struct AdapterRegistry|pub struct AdapterRegistry|auth|token|catalog|Keystone|scope|Context\" src/adapter src/port src/action.rs src/event.rs src/registry.rs src/module -g '*.rs'" in /Users/jay.ahn/projects/infra/nexttui/.worktrees/runtime-context-switch
 succeeded in 0ms:
src/event.rs:56:    // Keystone CUD
src/event.rs:141:    fn test_token_refreshed_carries_roles() {
src/action.rs:53:    // Keystone Admin
src/port/mock.rs:483:// MockKeystoneAdapter
src/port/mock.rs:486:pub struct MockKeystoneAdapter;
src/port/mock.rs:489:impl super::keystone::KeystonePort for MockKeystoneAdapter {
src/port/mock.rs:560:    use super::super::keystone::KeystonePort;
src/port/mock.rs:615:        let mock = MockKeystoneAdapter;
src/adapter/http/cinder.rs:10:use crate::port::auth::AuthProvider;
src/adapter/http/cinder.rs:20:    pub fn new(auth: Arc<dyn AuthProvider>, region: Option<String>) -> Result<Self, ApiError> {
src/adapter/http/cinder.rs:22:            base: BaseHttpClient::new(auth, "block-storage", EndpointInterface::Public, region)?,
src/port/mod.rs:1:pub mod auth;
src/adapter/http/mod.rs:72:/// Shared by all marker extraction variants (Link array, Glance next URL, Keystone links).
src/adapter/http/nova.rs:11:use crate::port::auth::AuthProvider;
src/adapter/http/nova.rs:21:    pub fn new(auth: Arc<dyn AuthProvider>, region: Option<String>) -> Result<Self, ApiError> {
src/adapter/http/nova.rs:23:            base: BaseHttpClient::new(auth, "compute", EndpointInterface::Public, region)?,
src/port/auth.rs:9:    async fn authenticate(&self, credential: &AuthCredential) -> ApiResult<Token>;
src/port/auth.rs:10:    async fn refresh_token(&self) -> ApiResult<Token>;
src/port/auth.rs:11:    async fn get_token(&self) -> ApiResult<String>;
src/port/auth.rs:12:    async fn get_token_info(&self) -> ApiResult<Token>;
src/port/auth.rs:13:    async fn authenticate_request(
src/port/auth.rs:26:    fn subscribe_token_refresh(&self) -> broadcast::Receiver<Token>;
src/port/auth.rs:28:    async fn get_catalog(&self) -> ApiResult<Vec<CatalogEntry>>;
src/adapter/http/base.rs:8:use crate::port::auth::AuthProvider;
src/adapter/http/base.rs:13:/// Auth injection is delegated to AuthProvider::authenticate_request().
src/adapter/http/base.rs:16:/// Callers should call `invalidate_endpoint()` when token refresh occurs
src/adapter/http/base.rs:17:/// to pick up potential catalog changes. In Phase 2, BaseHttpClient will
src/adapter/http/base.rs:18:/// subscribe to token refresh broadcast to automate this.
src/adapter/http/base.rs:21:    auth: Arc<dyn AuthProvider>,
src/adapter/http/base.rs:30:        auth: Arc<dyn AuthProvider>,
src/adapter/http/base.rs:40:            auth,
src/adapter/http/base.rs:48:    /// Resolve and cache the endpoint from service catalog.
src/adapter/http/base.rs:58:            .auth
src/adapter/http/base.rs:66:    /// Invalidate cached endpoint. Should be called on token refresh
src/adapter/http/base.rs:67:    /// to pick up potential service catalog changes.
src/adapter/http/base.rs:73:    /// Build an authenticated request.
src/adapter/http/base.rs:74:    /// Note (Phase 2): For signed auth methods (HMAC), authenticate_request()
src/adapter/http/base.rs:82:        let auth_headers = self
src/adapter/http/base.rs:83:            .auth
src/adapter/http/base.rs:84:            .authenticate_request(method_str, &url, &empty_headers, None)
src/adapter/http/base.rs:90:        for (key, value) in &auth_headers.headers {
src/adapter/http/base.rs:219:        let resp = mock_response(401, "unauthorized");
src/port/keystone.rs:8:pub trait KeystonePort: Send + Sync {
src/adapter/http/neutron.rs:9:use crate::port::auth::AuthProvider;
src/adapter/http/neutron.rs:19:    pub fn new(auth: Arc<dyn AuthProvider>, region: Option<String>) -> Result<Self, ApiError> {
src/adapter/http/neutron.rs:21:            base: BaseHttpClient::new(auth, "network", EndpointInterface::Public, region)?,
src/adapter/http/neutron.rs:211:// Admin tokens automatically see all projects' resources.
src/adapter/http/neutron.rs:767:    // Neutron does not send all_tenants query param — admin token sees all automatically
src/port/types.rs:44:    Unscoped,
src/port/types.rs:49:        match &credential.project_scope {
src/port/types.rs:54:            None => Self::Unscoped,
src/port/types.rs:68:            Self::Unscoped => "unscoped".to_string(),
src/port/types.rs:89:    pub catalog: Vec<CatalogEntry>,
src/port/types.rs:99:            .field("catalog", &format!("[{} entries]", self.catalog.len()))
src/port/types.rs:141:    pub auth_url: String,
src/port/types.rs:143:    pub project_scope: Option<ProjectScopeParam>,
src/port/types.rs:502:// --- Keystone params ---
src/adapter/http/keystone.rs:9:use crate::port::auth::AuthProvider;
src/adapter/http/keystone.rs:11:use crate::port::keystone::KeystonePort;
src/adapter/http/keystone.rs:14:pub struct KeystoneHttpAdapter {
src/adapter/http/keystone.rs:18:impl KeystoneHttpAdapter {
src/adapter/http/keystone.rs:19:    pub fn new(auth: Arc<dyn AuthProvider>, region: Option<String>) -> Result<Self, ApiError> {
src/adapter/http/keystone.rs:21:            base: BaseHttpClient::new(auth, "identity", EndpointInterface::Public, region)?,
src/adapter/http/keystone.rs:29:struct KeystoneProjectsResponse {
src/adapter/http/keystone.rs:31:    links: Option<KeystoneLinks>,
src/adapter/http/keystone.rs:35:struct KeystoneProjectWrapper {
src/adapter/http/keystone.rs:40:struct KeystoneUsersResponse {
src/adapter/http/keystone.rs:42:    links: Option<KeystoneLinks>,
src/adapter/http/keystone.rs:46:struct KeystoneUserWrapper {
src/adapter/http/keystone.rs:51:struct KeystoneRolesResponse {
src/adapter/http/keystone.rs:56:struct KeystoneRoleAssignmentsResponse {
src/adapter/http/keystone.rs:61:struct KeystoneDomainsResponse {
src/adapter/http/keystone.rs:65:// Keystone uses {"links": {"next": "url", "previous": "url"}} pattern
src/adapter/http/keystone.rs:67:struct KeystoneLinks {
src/adapter/http/keystone.rs:71:fn extract_keystone_marker(links: &Option<KeystoneLinks>) -> Option<String> {
src/adapter/http/keystone.rs:142:// --- KeystonePort implementation ---
src/adapter/http/keystone.rs:145:impl KeystonePort for KeystoneHttpAdapter {
src/adapter/http/keystone.rs:153:        paginated_list(&self.base, "/v3/projects", &query, |resp: KeystoneProjectsResponse| {
src/adapter/http/keystone.rs:164:        let resp: KeystoneProjectWrapper = self.base.send_json(req).await?;
src/adapter/http/keystone.rs:178:        let resp: KeystoneProjectWrapper = self.base.send_json(req).await?;
src/adapter/http/keystone.rs:199:        let resp: KeystoneProjectWrapper = self.base.send_json(req).await?;
src/adapter/http/keystone.rs:218:        paginated_list(&self.base, "/v3/users", &query, |resp: KeystoneUsersResponse| {
src/adapter/http/keystone.rs:229:        let resp: KeystoneUserWrapper = self.base.send_json(req).await?;
src/adapter/http/keystone.rs:245:        let resp: KeystoneUserWrapper = self.base.send_json(req).await?;
src/adapter/http/keystone.rs:267:        let resp: KeystoneUserWrapper = self.base.send_json(req).await?;
src/adapter/http/keystone.rs:283:        let resp: KeystoneRolesResponse = self.base.send_json(req).await?;
src/adapter/http/keystone.rs:318:            parts.push(format!("scope.project.id={}", encode_param(pid)));
src/adapter/http/keystone.rs:330:        let resp: KeystoneRoleAssignmentsResponse = self.base.send_json(req).await?;
src/adapter/http/keystone.rs:338:        let resp: KeystoneDomainsResponse = self.base.send_json(req).await?;
src/adapter/http/keystone.rs:355:        let resp: KeystoneProjectsResponse = serde_json::from_str(json).unwrap();
src/adapter/http/keystone.rs:369:        let resp: KeystoneUsersResponse = serde_json::from_str(json).unwrap();
src/adapter/http/keystone.rs:413:        let resp: KeystoneRolesResponse = serde_json::from_str(json).unwrap();
src/adapter/http/keystone.rs:419:        let links = Some(KeystoneLinks { next: None });
src/module/mod.rs:47:    // Keystone
src/adapter/http/glance.rs:9:use crate::port::auth::AuthProvider;
src/adapter/http/glance.rs:19:    pub fn new(auth: Arc<dyn AuthProvider>, region: Option<String>) -> Result<Self, ApiError> {
src/adapter/http/glance.rs:21:            base: BaseHttpClient::new(auth, "image", EndpointInterface::Public, region)?,
src/adapter/mod.rs:1:pub mod auth;
src/adapter/auth/mod.rs:2:pub mod token_cache;
src/adapter/auth/token_cache.rs:1://! Token cache persistence: save/load Keystone tokens to disk.
src/adapter/auth/token_cache.rs:3://! Cache layout: `~/.cache/nexttui/auth/{cloud_key}/{scope_key}`
src/adapter/auth/token_cache.rs:13:pub fn compute_cloud_key(auth_url: &str, username: &str) -> String {
src/adapter/auth/token_cache.rs:14:    let input = format!("{auth_url}|{username}");
src/adapter/auth/token_cache.rs:36:        .join("auth")
src/adapter/auth/token_cache.rs:40:/// Save a token to the cache directory, keyed by scope.
src/adapter/auth/token_cache.rs:43:pub fn save_token(token: &Token, cache_dir: &Path, scope: &TokenScope) -> Result<(), std::io::Error> {
src/adapter/auth/token_cache.rs:57:    let path = cache_dir.join(scope.cache_key());
src/adapter/auth/token_cache.rs:58:    let data = serde_json::to_vec(token)
src/adapter/auth/token_cache.rs:80:    tracing::debug!(path = %path.display(), "token cached to disk");
src/adapter/auth/token_cache.rs:84:/// Load a single token from a cache file.
src/adapter/auth/token_cache.rs:85:/// Returns None if the file doesn't exist, is unreadable, or the token is expired.
src/adapter/auth/token_cache.rs:86:/// Automatically deletes expired token files.
src/adapter/auth/token_cache.rs:87:fn load_token_file(path: &Path) -> Option<Token> {
src/adapter/auth/token_cache.rs:89:    let token: Token = serde_json::from_slice(&data).ok()?;
src/adapter/auth/token_cache.rs:91:    if token.expires_at > chrono::Utc::now() + chrono::Duration::minutes(1) {
src/adapter/auth/token_cache.rs:92:        Some(token)
src/adapter/auth/token_cache.rs:94:        tracing::info!(path = %path.display(), "cached token expired, removing");
src/adapter/auth/token_cache.rs:100:/// Load all valid cached tokens from the cache directory.
src/adapter/auth/token_cache.rs:101:/// Returns a map of scope → token. Expired tokens are auto-deleted.
src/adapter/auth/token_cache.rs:103:pub fn load_all_tokens(cache_dir: &Path) -> HashMap<TokenScope, Token> {
src/adapter/auth/token_cache.rs:109:            tracing::warn!(path = %cache_dir.display(), error = %e, "failed to read token cache directory");
src/adapter/auth/token_cache.rs:121:        let scope_key = file_name.to_string_lossy();
src/adapter/auth/token_cache.rs:123:        let scope = match parse_scope_from_filename(&scope_key) {
src/adapter/auth/token_cache.rs:126:                tracing::warn!(filename = %scope_key, "unrecognized token cache file, skipping");
src/adapter/auth/token_cache.rs:131:        if let Some(token) = load_token_file(&entry.path()) {
src/adapter/auth/token_cache.rs:132:            tracing::debug!("loaded cached token from disk");
src/adapter/auth/token_cache.rs:133:            map.insert(scope, token);
src/adapter/auth/token_cache.rs:141:fn parse_scope_from_filename(filename: &str) -> Option<TokenScope> {
src/adapter/auth/token_cache.rs:142:    if filename == "unscoped" {
src/adapter/auth/token_cache.rs:143:        return Some(TokenScope::Unscoped);
src/adapter/auth/token_cache.rs:164:    fn sample_token(expires_in_minutes: i64) -> Token {
src/adapter/auth/token_cache.rs:178:            catalog: vec![CatalogEntry {
src/adapter/auth/token_cache.rs:190:    fn sample_scope() -> TokenScope {
src/adapter/auth/token_cache.rs:213:    fn test_token_scope_cache_key() {
src/adapter/auth/token_cache.rs:214:        let scope = TokenScope::Project {
src/adapter/auth/token_cache.rs:218:        assert_eq!(scope.cache_key(), "project@admin@default");
src/adapter/auth/token_cache.rs:219:        assert_eq!(TokenScope::Unscoped.cache_key(), "unscoped");
src/adapter/auth/token_cache.rs:224:        let scope = TokenScope::Project {
src/adapter/auth/token_cache.rs:229:        assert!(!scope.cache_key().contains('/'));
src/adapter/auth/token_cache.rs:230:        assert!(!scope.cache_key().contains(".."));
src/adapter/auth/token_cache.rs:235:        let scope = TokenScope::Project {
src/adapter/auth/token_cache.rs:240:        assert_eq!(scope.cache_key(), "project@my_project@my_domain");
src/adapter/auth/token_cache.rs:244:    fn test_save_and_load_scoped_token() {
src/adapter/auth/token_cache.rs:247:        let scope = sample_scope();
src/adapter/auth/token_cache.rs:249:        let token = sample_token(60);
src/adapter/auth/token_cache.rs:250:        save_token(&token, &cache_dir, &scope).unwrap();
src/adapter/auth/token_cache.rs:252:        let loaded = load_all_tokens(&cache_dir);
src/adapter/auth/token_cache.rs:254:        let loaded_token = loaded.get(&scope).unwrap();
src/adapter/auth/token_cache.rs:255:        assert_eq!(loaded_token.id, "tok-test-123");
src/adapter/auth/token_cache.rs:259:    fn test_save_multiple_scopes() {
src/adapter/auth/token_cache.rs:263:        let scope_a = TokenScope::Project {
src/adapter/auth/token_cache.rs:267:        let scope_b = TokenScope::Project {
src/adapter/auth/token_cache.rs:272:        let mut token_a = sample_token(60);
src/adapter/auth/token_cache.rs:273:        token_a.id = "tok-a".to_string();
src/adapter/auth/token_cache.rs:274:        let mut token_b = sample_token(60);
src/adapter/auth/token_cache.rs:275:        token_b.id = "tok-b".to_string();
src/adapter/auth/token_cache.rs:277:        save_token(&token_a, &cache_dir, &scope_a).unwrap();
src/adapter/auth/token_cache.rs:278:        save_token(&token_b, &cache_dir, &scope_b).unwrap();
src/adapter/auth/token_cache.rs:280:        let loaded = load_all_tokens(&cache_dir);
src/adapter/auth/token_cache.rs:282:        assert_eq!(loaded.get(&scope_a).unwrap().id, "tok-a");
src/adapter/auth/token_cache.rs:283:        assert_eq!(loaded.get(&scope_b).unwrap().id, "tok-b");
src/adapter/auth/token_cache.rs:287:    fn test_load_expired_token_deleted() {
src/adapter/auth/token_cache.rs:290:        let scope = sample_scope();
src/adapter/auth/token_cache.rs:292:        let token = sample_token(-10);
src/adapter/auth/token_cache.rs:293:        save_token(&token, &cache_dir, &scope).unwrap();
src/adapter/auth/token_cache.rs:295:        let loaded = load_all_tokens(&cache_dir);
src/adapter/auth/token_cache.rs:297:        assert!(!cache_dir.join(scope.cache_key()).exists());
src/adapter/auth/token_cache.rs:303:        let loaded = load_all_tokens(&path);
src/adapter/auth/token_cache.rs:314:        let loaded = load_all_tokens(&cache_dir);
src/adapter/auth/token_cache.rs:325:        let scope = sample_scope();
src/adapter/auth/token_cache.rs:327:        let token = sample_token(60);
src/adapter/auth/token_cache.rs:328:        save_token(&token, &cache_dir, &scope).unwrap();
src/adapter/auth/token_cache.rs:330:        let path = cache_dir.join(scope.cache_key());
src/adapter/auth/token_cache.rs:336:    fn test_parse_scope_from_filename() {
src/adapter/auth/token_cache.rs:338:            parse_scope_from_filename("project@admin@default"),
src/adapter/auth/token_cache.rs:341:        assert_eq!(parse_scope_from_filename("unscoped"), Some(TokenScope::Unscoped));
src/adapter/auth/token_cache.rs:342:        assert_eq!(parse_scope_from_filename("unknown_file"), None);
src/adapter/auth/token_cache.rs:346:    fn test_parse_scope_with_underscore_in_name() {
src/adapter/auth/token_cache.rs:348:            parse_scope_from_filename("project@my_project@my_domain"),
src/adapter/auth/token_cache.rs:354:    fn test_token_scope_from_credential() {
src/adapter/auth/token_cache.rs:356:            auth_url: "https://keystone:5000/v3".to_string(),
src/adapter/auth/token_cache.rs:362:            project_scope: Some(ProjectScopeParam {
src/adapter/auth/token_cache.rs:373:            auth_url: "https://keystone:5000/v3".to_string(),
src/adapter/auth/token_cache.rs:379:            project_scope: None,
src/adapter/auth/token_cache.rs:381:        assert_eq!(TokenScope::from_credential(&unsoped_cred), TokenScope::Unscoped);
src/module/floating_ip/mod.rs:21:use self::view_model::{fip_columns, fip_create_defs, fip_to_row, FipRowContext};
src/module/floating_ip/mod.rs:92:        let ctx = FipRowContext {
src/adapter/auth/keystone.rs:14:use crate::port::auth::AuthProvider;
src/adapter/auth/keystone.rs:18:// --- Keystone v3 response types (internal) ---
src/adapter/auth/keystone.rs:21:struct KeystoneTokenResponse {
src/adapter/auth/keystone.rs:22:    token: KeystoneTokenBody,
src/adapter/auth/keystone.rs:26:struct KeystoneTokenBody {
src/adapter/auth/keystone.rs:28:    project: Option<KeystoneProject>,
src/adapter/auth/keystone.rs:29:    roles: Vec<KeystoneRole>,
src/adapter/auth/keystone.rs:30:    catalog: Option<Vec<KeystoneCatalogEntry>>,
src/adapter/auth/keystone.rs:34:struct KeystoneProject {
src/adapter/auth/keystone.rs:37:    domain: KeystoneDomain,
src/adapter/auth/keystone.rs:41:struct KeystoneDomain {
src/adapter/auth/keystone.rs:47:struct KeystoneRole {
src/adapter/auth/keystone.rs:53:struct KeystoneCatalogEntry {
src/adapter/auth/keystone.rs:57:    endpoints: Vec<KeystoneEndpoint>,
src/adapter/auth/keystone.rs:61:struct KeystoneEndpoint {
src/adapter/auth/keystone.rs:69:fn parse_token(token_id: String, resp: KeystoneTokenResponse) -> Token {
src/adapter/auth/keystone.rs:70:    let body = resp.token;
src/adapter/auth/keystone.rs:93:    let catalog = body
src/adapter/auth/keystone.rs:94:        .catalog
src/adapter/auth/keystone.rs:113:        id: token_id,
src/adapter/auth/keystone.rs:117:        catalog,
src/adapter/auth/keystone.rs:129:// --- KeystoneAuthAdapter ---
src/adapter/auth/keystone.rs:131:pub struct KeystoneAuthAdapter {
src/adapter/auth/keystone.rs:134:    token_map: Arc<RwLock<HashMap<TokenScope, Token>>>,
src/adapter/auth/keystone.rs:135:    active_scope: TokenScope,
src/adapter/auth/keystone.rs:136:    token_tx: broadcast::Sender<Token>,
src/adapter/auth/keystone.rs:142:    /// Directory for scope-keyed token cache files.
src/adapter/auth/keystone.rs:146:impl KeystoneAuthAdapter {
src/adapter/auth/keystone.rs:148:        use super::token_cache;
src/adapter/auth/keystone.rs:154:        let active_scope = TokenScope::from_credential(&credential);
src/adapter/auth/keystone.rs:155:        let cloud_key = token_cache::compute_cloud_key(&credential.auth_url, &username);
src/adapter/auth/keystone.rs:156:        let cache_dir = token_cache::cache_dir_path(&cloud_key);
src/adapter/auth/keystone.rs:158:        // Load all cached tokens for this cloud from disk
src/adapter/auth/keystone.rs:159:        let cached_tokens = token_cache::load_all_tokens(&cache_dir);
src/adapter/auth/keystone.rs:161:        let (token_tx, _) = broadcast::channel::<Token>(16);
src/adapter/auth/keystone.rs:168:            token_map: Arc::new(RwLock::new(cached_tokens)),
src/adapter/auth/keystone.rs:169:            active_scope,
src/adapter/auth/keystone.rs:170:            token_tx,
src/adapter/auth/keystone.rs:178:    /// Start the background token refresh loop. Idempotent — only spawns once.
src/adapter/auth/keystone.rs:185:        let token_map_ref = self.token_map.clone();
src/adapter/auth/keystone.rs:188:        let tx = self.token_tx.clone();
src/adapter/auth/keystone.rs:190:        let scope = self.active_scope.clone();
src/adapter/auth/keystone.rs:192:        let refresh_span = tracing::info_span!("token_refresh_loop");
src/adapter/auth/keystone.rs:197:                        let map = token_map_ref.read().await;
src/adapter/auth/keystone.rs:198:                        match map.get(&scope) {
src/adapter/auth/keystone.rs:214:                    match Self::do_authenticate(&client, &credential).await {
src/adapter/auth/keystone.rs:215:                        Ok(new_token) => {
src/adapter/auth/keystone.rs:216:                            let mut map = token_map_ref.write().await;
src/adapter/auth/keystone.rs:217:                            map.insert(scope.clone(), new_token.clone());
src/adapter/auth/keystone.rs:218:                            if let Err(e) = super::token_cache::save_token(&new_token, &cache_dir, &scope) {
src/adapter/auth/keystone.rs:219:                                tracing::warn!(error = %e, "failed to cache token to disk");
src/adapter/auth/keystone.rs:221:                            let _ = tx.send(new_token);
src/adapter/auth/keystone.rs:224:                            tracing::warn!(error = %e, "token refresh failed, retrying in 30s");
src/adapter/auth/keystone.rs:237:    /// Perform the actual Keystone v3 auth POST.
src/adapter/auth/keystone.rs:238:    #[tracing::instrument(skip(client, credential), fields(auth_url = %credential.auth_url))]
src/adapter/auth/keystone.rs:239:    async fn do_authenticate(
src/adapter/auth/keystone.rs:243:        let auth_url = format!(
src/adapter/auth/keystone.rs:244:            "{}/auth/tokens",
src/adapter/auth/keystone.rs:245:            credential.auth_url.trim_end_matches('/')
src/adapter/auth/keystone.rs:247:        let body = Self::build_auth_body(credential);
src/adapter/auth/keystone.rs:249:            .post(&auth_url)
src/adapter/auth/keystone.rs:260:        let token_id = resp
src/adapter/auth/keystone.rs:267:        let body: KeystoneTokenResponse = resp
src/adapter/auth/keystone.rs:270:            .map_err(|e| ApiError::Parse(format!("Failed to parse Keystone response: {e}")))?;
src/adapter/auth/keystone.rs:272:        Ok(parse_token(token_id, body))
src/adapter/auth/keystone.rs:275:    /// Build the Keystone v3 auth request body based on AuthMethod.
src/adapter/auth/keystone.rs:276:    /// Note: authenticate() always uses self.credential (passed at construction).
src/adapter/auth/keystone.rs:277:    /// The credential parameter in AuthProvider::authenticate() should match self.credential.
src/adapter/auth/keystone.rs:278:    pub(crate) fn build_auth_body(credential: &AuthCredential) -> serde_json::Value {
src/adapter/auth/keystone.rs:303:        let mut auth = serde_json::json!({ "identity": identity });
src/adapter/auth/keystone.rs:305:        if let Some(ref scope) = credential.project_scope {
src/adapter/auth/keystone.rs:306:            auth["scope"] = serde_json::json!({
src/adapter/auth/keystone.rs:308:                    "name": scope.name,
src/adapter/auth/keystone.rs:309:                    "domain": { "name": scope.domain_name }
src/adapter/auth/keystone.rs:314:        serde_json::json!({ "auth": auth })
src/adapter/auth/keystone.rs:319:impl AuthProvider for KeystoneAuthAdapter {
src/adapter/auth/keystone.rs:320:    async fn authenticate(&self, credential: &AuthCredential) -> ApiResult<Token> {
src/adapter/auth/keystone.rs:321:        let token = Self::do_authenticate(&self.client, credential).await?;
src/adapter/auth/keystone.rs:323:            let mut map = self.token_map.write().await;
src/adapter/auth/keystone.rs:324:            map.insert(self.active_scope.clone(), token.clone());
src/adapter/auth/keystone.rs:326:        if let Err(e) = super::token_cache::save_token(&token, &self.cache_dir, &self.active_scope) {
src/adapter/auth/keystone.rs:327:            tracing::warn!(error = %e, "failed to cache token to disk");
src/adapter/auth/keystone.rs:330:        Ok(token)
src/adapter/auth/keystone.rs:334:    async fn refresh_token(&self) -> ApiResult<Token> {
src/adapter/auth/keystone.rs:335:        let token = Self::do_authenticate(&self.client, &self.credential).await?;
src/adapter/auth/keystone.rs:337:            let mut map = self.token_map.write().await;
src/adapter/auth/keystone.rs:338:            map.insert(self.active_scope.clone(), token.clone());
src/adapter/auth/keystone.rs:340:        if let Err(e) = super::token_cache::save_token(&token, &self.cache_dir, &self.active_scope) {
src/adapter/auth/keystone.rs:341:            tracing::warn!(error = %e, "failed to cache token to disk");
src/adapter/auth/keystone.rs:343:        let _ = self.token_tx.send(token.clone());
src/adapter/auth/keystone.rs:344:        Ok(token)
src/adapter/auth/keystone.rs:347:    /// Get a valid token string. If near-expiry (<1min), refresh first.
src/adapter/auth/keystone.rs:350:    async fn get_token(&self) -> ApiResult<String> {
src/adapter/auth/keystone.rs:351:        // Ensure refresh loop is running (idempotent — handles cached token from disk)
src/adapter/auth/keystone.rs:354:        // Fast path: token is still valid for active scope
src/adapter/auth/keystone.rs:356:            let map = self.token_map.read().await;
src/adapter/auth/keystone.rs:357:            if let Some(t) = map.get(&self.active_scope) {
src/adapter/auth/keystone.rs:369:            let map = self.token_map.read().await;
src/adapter/auth/keystone.rs:370:            if let Some(t) = map.get(&self.active_scope) {
src/adapter/auth/keystone.rs:377:        let token = self.refresh_token().await?;
src/adapter/auth/keystone.rs:378:        Ok(token.id)
src/adapter/auth/keystone.rs:381:    async fn get_token_info(&self) -> ApiResult<Token> {
src/adapter/auth/keystone.rs:382:        let map = self.token_map.read().await;
src/adapter/auth/keystone.rs:383:        map.get(&self.active_scope)
src/adapter/auth/keystone.rs:385:            .ok_or(ApiError::AuthFailed("Not authenticated".into()))
src/adapter/auth/keystone.rs:388:    /// Inject X-Auth-Token header. Phase 1: token-based auth only.
src/adapter/auth/keystone.rs:389:    /// Phase 2 note: for signed auth (HMAC), this method will need the actual
src/adapter/auth/keystone.rs:393:    async fn authenticate_request(
src/adapter/auth/keystone.rs:400:        let token_id = self.get_token().await?;
src/adapter/auth/keystone.rs:402:            headers: vec![("X-Auth-Token".to_string(), token_id)],
src/adapter/auth/keystone.rs:413:        // Ensure we have a valid token (triggers initial auth if needed)
src/adapter/auth/keystone.rs:414:        let _ = self.get_token().await?;
src/adapter/auth/keystone.rs:416:        let token = self.get_token_info().await?;
src/adapter/auth/keystone.rs:418:        token
src/adapter/auth/keystone.rs:419:            .catalog
src/adapter/auth/keystone.rs:433:    fn subscribe_token_refresh(&self) -> broadcast::Receiver<Token> {
src/adapter/auth/keystone.rs:434:        self.token_tx.subscribe()
src/adapter/auth/keystone.rs:438:        let map = self.token_map.read().await;
src/adapter/auth/keystone.rs:439:        let token = map
src/adapter/auth/keystone.rs:440:            .get(&self.active_scope)
src/adapter/auth/keystone.rs:441:            .ok_or(ApiError::AuthFailed("Not authenticated".into()))?;
src/adapter/auth/keystone.rs:442:        Ok(token.roles.iter().any(|r| r.name == role_name))
src/adapter/auth/keystone.rs:445:    async fn get_catalog(&self) -> ApiResult<Vec<CatalogEntry>> {
src/adapter/auth/keystone.rs:446:        let map = self.token_map.read().await;
src/adapter/auth/keystone.rs:447:        let token = map
src/adapter/auth/keystone.rs:448:            .get(&self.active_scope)
src/adapter/auth/keystone.rs:449:            .ok_or(ApiError::AuthFailed("Not authenticated".into()))?;
src/adapter/auth/keystone.rs:450:        Ok(token.catalog.clone())
src/adapter/auth/keystone.rs:454:        // Phase 1: Keystone has no capability concept. Return empty.
src/adapter/auth/keystone.rs:465:            auth_url: "https://keystone.example.com:5000/v3".to_string(),
src/adapter/auth/keystone.rs:471:            project_scope: Some(ProjectScopeParam {
src/adapter/auth/keystone.rs:480:            auth_url: "https://keystone.example.com:5000/v3".to_string(),
src/adapter/auth/keystone.rs:485:            project_scope: None,
src/adapter/auth/keystone.rs:491:            "token": {
src/adapter/auth/keystone.rs:502:                "catalog": [
src/adapter/auth/keystone.rs:524:    fn test_build_auth_body_password() {
src/adapter/auth/keystone.rs:526:        let body = KeystoneAuthAdapter::build_auth_body(&cred);
src/adapter/auth/keystone.rs:528:        assert_eq!(body["auth"]["identity"]["methods"][0], "password");
src/adapter/auth/keystone.rs:529:        assert_eq!(body["auth"]["identity"]["password"]["user"]["name"], "admin");
src/adapter/auth/keystone.rs:531:            body["auth"]["identity"]["password"]["user"]["domain"]["name"],
src/adapter/auth/keystone.rs:534:        assert_eq!(body["auth"]["scope"]["project"]["name"], "admin-project");
src/adapter/auth/keystone.rs:538:    fn test_build_auth_body_app_credential() {
src/adapter/auth/keystone.rs:540:        let body = KeystoneAuthAdapter::build_auth_body(&cred);
src/adapter/auth/keystone.rs:543:            body["auth"]["identity"]["methods"][0],
src/adapter/auth/keystone.rs:547:            body["auth"]["identity"]["application_credential"]["id"],
src/adapter/auth/keystone.rs:550:        assert!(body["auth"]["scope"].is_null());
src/adapter/auth/keystone.rs:554:    fn test_parse_token_from_keystone_response() {
src/adapter/auth/keystone.rs:556:        let resp: KeystoneTokenResponse = serde_json::from_str(json_str).unwrap();
src/adapter/auth/keystone.rs:557:        let token = parse_token("tok-abc-123".to_string(), resp);
src/adapter/auth/keystone.rs:559:        assert_eq!(token.id, "tok-abc-123");
src/adapter/auth/keystone.rs:560:        assert_eq!(token.project.name, "admin-project");
src/adapter/auth/keystone.rs:561:        assert_eq!(token.project.domain_name, "Default");
src/adapter/auth/keystone.rs:562:        assert_eq!(token.roles.len(), 2);
src/adapter/auth/keystone.rs:563:        assert_eq!(token.roles[0].name, "admin");
src/adapter/auth/keystone.rs:564:        assert_eq!(token.catalog.len(), 2);
src/adapter/auth/keystone.rs:565:        assert_eq!(token.catalog[0].service_type, "compute");
src/adapter/auth/keystone.rs:566:        assert_eq!(token.catalog[0].endpoints.len(), 2);
src/adapter/auth/keystone.rs:568:            token.catalog[0].endpoints[0].interface,
src/adapter/auth/keystone.rs:574:    fn test_parse_token_no_catalog() {
src/adapter/auth/keystone.rs:576:            "token": {
src/adapter/auth/keystone.rs:581:        let resp: KeystoneTokenResponse = serde_json::from_str(json_str).unwrap();
src/adapter/auth/keystone.rs:582:        let token = parse_token("tok-1".to_string(), resp);
src/adapter/auth/keystone.rs:584:        assert!(token.catalog.is_empty());
src/adapter/auth/keystone.rs:585:        assert_eq!(token.roles.len(), 1);
src/adapter/auth/keystone.rs:586:        assert!(token.project.id.is_empty());
src/adapter/auth/keystone.rs:598:    async fn test_get_endpoint_from_token() {
src/adapter/auth/keystone.rs:599:        let adapter = KeystoneAuthAdapter::new(sample_credential_password()).unwrap();
src/adapter/auth/keystone.rs:600:        let resp: KeystoneTokenResponse =
src/adapter/auth/keystone.rs:602:        let token = parse_token("tok-1".to_string(), resp);
src/adapter/auth/keystone.rs:604:            let mut map = adapter.token_map.write().await;
src/adapter/auth/keystone.rs:605:            map.insert(adapter.active_scope.clone(), token);
src/adapter/auth/keystone.rs:628:        let adapter = KeystoneAuthAdapter::new(sample_credential_password()).unwrap();
src/adapter/auth/keystone.rs:629:        let resp: KeystoneTokenResponse =
src/adapter/auth/keystone.rs:631:        let token = parse_token("tok-1".to_string(), resp);
src/adapter/auth/keystone.rs:633:            let mut map = adapter.token_map.write().await;
src/adapter/auth/keystone.rs:634:            map.insert(adapter.active_scope.clone(), token);
src/adapter/auth/keystone.rs:643:    async fn test_authenticate_request_injects_token() {
src/adapter/auth/keystone.rs:644:        let adapter = KeystoneAuthAdapter::new(sample_credential_password()).unwrap();
src/adapter/auth/keystone.rs:645:        let resp: KeystoneTokenResponse =
src/adapter/auth/keystone.rs:647:        let token = parse_token("tok-xyz".to_string(), resp);
src/adapter/auth/keystone.rs:649:            let mut map = adapter.token_map.write().await;
src/adapter/auth/keystone.rs:650:            map.insert(adapter.active_scope.clone(), token);
src/adapter/auth/keystone.rs:654:        let auth = adapter
src/adapter/auth/keystone.rs:655:            .authenticate_request("GET", "https://nova:8774/v2.1/servers", &headers, None)
src/adapter/auth/keystone.rs:659:        assert_eq!(auth.headers.len(), 1);
src/adapter/auth/keystone.rs:660:        assert_eq!(auth.headers[0].0, "X-Auth-Token");
src/adapter/auth/keystone.rs:661:        assert_eq!(auth.headers[0].1, "tok-xyz");
src/adapter/auth/keystone.rs:665:    async fn test_get_token_info_not_authenticated() {
src/adapter/auth/keystone.rs:666:        let adapter = KeystoneAuthAdapter::new(sample_credential_password()).unwrap();
src/adapter/auth/keystone.rs:667:        let err = adapter.get_token_info().await;
src/adapter/auth/keystone.rs:672:    async fn test_get_catalog() {
src/adapter/auth/keystone.rs:673:        let adapter = KeystoneAuthAdapter::new(sample_credential_password()).unwrap();
src/adapter/auth/keystone.rs:674:        let resp: KeystoneTokenResponse =
src/adapter/auth/keystone.rs:676:        let token = parse_token("tok-1".to_string(), resp);
src/adapter/auth/keystone.rs:678:            let mut map = adapter.token_map.write().await;
src/adapter/auth/keystone.rs:679:            map.insert(adapter.active_scope.clone(), token);
src/adapter/auth/keystone.rs:682:        let catalog = adapter.get_catalog().await.unwrap();
src/adapter/auth/keystone.rs:683:        assert_eq!(catalog.len(), 2);
src/adapter/auth/keystone.rs:684:        assert_eq!(catalog[0].service_type, "compute");
src/adapter/auth/keystone.rs:689:        let adapter = KeystoneAuthAdapter::new(sample_credential_password()).unwrap();
src/adapter/registry.rs:5:use crate::adapter::http::keystone::KeystoneHttpAdapter;
src/adapter/registry.rs:8:use crate::port::auth::AuthProvider;
src/adapter/registry.rs:12:use crate::port::keystone::KeystonePort;
src/adapter/registry.rs:20:pub struct AdapterRegistry {
src/adapter/registry.rs:25:    pub keystone: Arc<dyn KeystonePort>,
src/adapter/registry.rs:29:    /// Create all HTTP adapters from the given auth provider and region.
src/adapter/registry.rs:30:    pub fn new_http(auth: Arc<dyn AuthProvider>, region: Option<String>) -> Result<Self, ApiError> {
src/adapter/registry.rs:32:            nova: Arc::new(NovaHttpAdapter::new(auth.clone(), region.clone())?),
src/adapter/registry.rs:33:            neutron: Arc::new(NeutronHttpAdapter::new(auth.clone(), region.clone())?),
src/adapter/registry.rs:34:            cinder: Arc::new(CinderHttpAdapter::new(auth.clone(), region.clone())?),
src/adapter/registry.rs:35:            glance: Arc::new(GlanceHttpAdapter::new(auth.clone(), region.clone())?),
src/adapter/registry.rs:36:            keystone: Arc::new(KeystoneHttpAdapter::new(auth, region)?),
src/adapter/registry.rs:49:            keystone: Arc::new(MockKeystoneAdapter),
src/adapter/registry.rs:65:        let _keystone: &dyn KeystonePort = registry.keystone.as_ref();
src/module/floating_ip/view_model.rs:7:pub struct FipRowContext<'a> {
src/module/floating_ip/view_model.rs:13:impl<'a> FipRowContext<'a> {
src/module/floating_ip/view_model.rs:61:    fip_to_row(fip, &FipRowContext::default_for(show_tenant))
src/module/floating_ip/view_model.rs:64:pub fn fip_to_row(fip: &FloatingIp, ctx: &FipRowContext) -> Row {
src/module/server/view_model.rs:10:pub struct ServerViewContext<'a> {
src/module/server/view_model.rs:19:impl<'a> ServerViewContext<'a> {
src/module/server/view_model.rs:146:    server_detail_data(&ServerViewContext::default_for(server))
src/module/server/view_model.rs:149:pub fn server_detail_data(ctx: &ServerViewContext) -> DetailData {
src/module/server/view_model.rs:686:        let data = server_detail_data(&ServerViewContext::default_for(&server));
src/module/server/view_model.rs:693:        let data = server_detail_data(&ServerViewContext::default_for(&server));
src/module/server/view_model.rs:714:        let data = server_detail_data(&ServerViewContext {
src/module/server/view_model.rs:732:        let data = server_detail_data(&ServerViewContext::default_for(&server));
src/module/server/view_model.rs:771:        let data = server_detail_data(&ServerViewContext {
src/module/server/view_model.rs:787:        let data = server_detail_data(&ServerViewContext::default_for(&server));
src/module/server/view_model.rs:801:        let data = server_detail_data(&ServerViewContext {
src/module/server/mod.rs:24:    ServerViewContext,
src/module/server/mod.rs:999:                    let data = server_detail_data(&ServerViewContext {

exec
/bin/zsh -lc "sed -n '1,320p' src/action.rs" in /Users/jay.ahn/projects/infra/nexttui/.worktrees/runtime-context-switch
 succeeded in 0ms:
use crate::models::common::Route;
use crate::port::types::EvacuateParams;

#[derive(Debug, Clone)]
pub enum Action {
    // Navigation
    Navigate(Route),
    Back,

    // Nova
    FetchServers,
    CreateServer(crate::port::types::ServerCreateParams),
    DeleteServer { id: String, name: String },
    RebootServer { id: String, hard: bool },
    StartServer { id: String },
    StopServer { id: String },
    CreateServerSnapshot { server_id: String, name: String },
    FetchFlavors,
    CreateFlavor(crate::port::types::FlavorCreateParams),
    DeleteFlavor { id: String },
    FetchAggregates,
    FetchComputeServices,
    FetchHypervisors,

    // Neutron
    FetchNetworks,
    CreateNetwork(crate::port::types::NetworkCreateParams),
    FetchSecurityGroups,
    CreateSecurityGroup(crate::port::types::SecurityGroupCreateParams),
    DeleteSecurityGroup { id: String },
    CreateSecurityGroupRule(crate::port::types::SecurityGroupRuleCreateParams),
    DeleteSecurityGroupRule { rule_id: String },
    FetchFloatingIps,
    CreateFloatingIp { network_id: String },
    DeleteFloatingIp { id: String },
    FetchSubnets { network_id: String },
    FetchAgents,

    // Cinder
    FetchVolumes,
    CreateVolume(crate::port::types::VolumeCreateParams),
    DeleteVolume { id: String, force: bool },
    ExtendVolume { id: String, new_size: u32 },
    FetchSnapshots,
    CreateSnapshot(crate::port::types::SnapshotCreateParams),
    DeleteSnapshot { id: String },

    // Glance
    FetchImages,
    CreateImage(crate::port::types::ImageCreateParams),
    DeleteImage { id: String },

    // Keystone Admin
    FetchProjects,
    CreateProject(crate::port::types::ProjectCreateParams),
    DeleteProject { id: String },
    FetchUsers,
    CreateUser(crate::port::types::UserCreateParams),
    DeleteUser { id: String },

    // Usage
    FetchUsage { start: String, end: String },

    // UI
    FocusSidebar,
    EnterFormMode,
    ExitFormMode,
    SelectResource { id: String },
    NavigateToResource { route: Route, id: String },

    // Resize
    ResizeServer { id: String, flavor_id: String },
    ConfirmResize { id: String },
    RevertResize { id: String },

    // Migration / Evacuate
    LiveMigrateServer { id: String, host: Option<String> },
    ColdMigrateServer { id: String },
    ConfirmMigration { id: String },
    RevertMigration { id: String },
    EvacuateServer { id: String, params: EvacuateParams },
    DisableComputeService { service_id: String, hostname: String },
    EnableComputeService { service_id: String, hostname: String },
    FetchMigrationProgress { server_id: String },

    // Volume Attach/Detach
    AttachVolume { volume_id: String, server_id: String, device: Option<String> },
    DetachVolume { volume_id: String, server_id: String, attachment_id: String },
    ForceDetachVolume { volume_id: String, server_id: String, attachment_id: String },
    ForceResetVolumeState { volume_id: String, target_state: String },

    // Floating IP Associate/Disassociate
    AssociateFloatingIp { fip_id: String, port_id: String },
    DisassociateFloatingIp { fip_id: String },

    // Ports
    FetchPorts { server_id: String },

    // All Tenants
    ToggleAllTenants,

    // Toast (module-initiated hints)
    ShowToast { message: String },

    // System
    RefreshAll,
    SwitchCloud(String),
    Quit,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_action_variants_exist() {
        use crate::port::types::{FlavorCreateParams, ServerCreateParams};

        let actions: Vec<Action> = vec![
            Action::Navigate(Route::Servers),
            Action::Back,
            Action::FetchServers,
            Action::CreateServer(ServerCreateParams {
                name: "test".into(),
                image_id: "img-1".into(),
                flavor_id: "flv-1".into(),
                networks: vec![],
                security_groups: None,
                key_name: None,
                availability_zone: None,
            }),
            Action::DeleteServer {
                id: "s1".into(),
                name: "web".into(),
            },
            Action::RebootServer {
                id: "s1".into(),
                hard: false,
            },
            Action::StartServer { id: "s1".into() },
            Action::StopServer { id: "s1".into() },
            Action::CreateServerSnapshot {
                server_id: "s1".into(),
                name: "snap".into(),
            },
            Action::FetchFlavors,
            Action::CreateFlavor(FlavorCreateParams {
                name: "m1.test".into(),
                vcpus: 1,
                ram_mb: 512,
                disk_gb: 10,
                is_public: true,
            }),
            Action::DeleteFlavor { id: "f1".into() },
            Action::FetchNetworks,
            Action::FetchVolumes,
            Action::FetchImages,
            Action::FetchProjects,
            Action::RefreshAll,
            Action::SwitchCloud("prod".into()),
            Action::Quit,
        ];
        assert!(actions.len() >= 18);
    }

    #[test]
    fn test_volume_fip_action_variants_exist() {
        let actions: Vec<Action> = vec![
            Action::AttachVolume { volume_id: "v1".into(), server_id: "s1".into(), device: Some("/dev/vdb".into()) },
            Action::DetachVolume { volume_id: "v1".into(), server_id: "s1".into(), attachment_id: "att-1".into() },
            Action::ForceDetachVolume { volume_id: "v1".into(), server_id: "s1".into(), attachment_id: "att-1".into() },
            Action::ForceResetVolumeState { volume_id: "v1".into(), target_state: "available".into() },
            Action::AssociateFloatingIp { fip_id: "fip-1".into(), port_id: "port-1".into() },
            Action::DisassociateFloatingIp { fip_id: "fip-1".into() },
            Action::FetchPorts { server_id: "s1".into() },
        ];
        assert_eq!(actions.len(), 7);
    }

    #[test]
    fn test_resize_action_variants_exist() {
        let actions: Vec<Action> = vec![
            Action::ResizeServer { id: "s1".into(), flavor_id: "f2".into() },
            Action::ConfirmResize { id: "s1".into() },
            Action::RevertResize { id: "s1".into() },
        ];
        assert_eq!(actions.len(), 3);
    }

    #[test]
    fn test_usage_action_variant_exists() {
        let action = Action::FetchUsage {
            start: "2026-01-01T00:00:00Z".into(),
            end: "2026-01-31T23:59:59Z".into(),
        };
        match action {
            Action::FetchUsage { start, end } => {
                assert!(start.contains("2026"));
                assert!(end.contains("2026"));
            }
            _ => panic!("expected FetchUsage"),
        }
    }

    #[test]
    fn test_migration_action_variants_exist() {
        let actions: Vec<Action> = vec![
            Action::LiveMigrateServer { id: "s1".into(), host: None },
            Action::ColdMigrateServer { id: "s1".into() },
            Action::ConfirmMigration { id: "s1".into() },
            Action::RevertMigration { id: "s1".into() },
            Action::EvacuateServer { id: "s1".into(), params: EvacuateParams { host: Some("compute-02".into()), ..Default::default() } },
            Action::FetchMigrationProgress { server_id: "s1".into() },
            Action::DisableComputeService { service_id: "svc-1".into(), hostname: "compute-01".into() },
            Action::EnableComputeService { service_id: "svc-1".into(), hostname: "compute-01".into() },
        ];
        assert_eq!(actions.len(), 8);
    }
}

exec
/bin/zsh -lc "sed -n '1,340p' src/event.rs" in /Users/jay.ahn/projects/infra/nexttui/.worktrees/runtime-context-switch
 succeeded in 0ms:
use crate::models::{
    cinder::{Volume, VolumeSnapshot},
    glance::Image,
    keystone::{Project, User},
    neutron::{FloatingIp, Network, NetworkAgent, Port, SecurityGroup},
    nova::{Aggregate, ComputeService, Flavor, Hypervisor, Server, ServerMigration},
};
use crate::port::types::TenantUsage;

#[derive(Debug)]
pub enum AppEvent {
    // Data loaded
    ServersLoaded(Vec<Server>),
    FlavorsLoaded(Vec<Flavor>),
    NetworksLoaded(Vec<Network>),
    SecurityGroupsLoaded(Vec<SecurityGroup>),
    FloatingIpsLoaded(Vec<FloatingIp>),
    VolumesLoaded(Vec<Volume>),
    SnapshotsLoaded(Vec<VolumeSnapshot>),
    ImagesLoaded(Vec<Image>),
    ProjectsLoaded(Vec<Project>),
    UsersLoaded(Vec<User>),
    AggregatesLoaded(Vec<Aggregate>),
    ComputeServicesLoaded(Vec<ComputeService>),
    HypervisorsLoaded(Vec<Hypervisor>),
    AgentsLoaded(Vec<NetworkAgent>),

    // Usage
    UsageLoaded(Vec<TenantUsage>),

    // CUD results
    ServerCreated(Server),
    ServerDeleted { id: String, name: String },
    ServerRebooted { id: String },
    ServerStarted { id: String },
    ServerStopped { id: String },
    ServerSnapshotCreated { server_id: String, image_id: String },
    FlavorCreated(Flavor),
    FlavorDeleted { id: String },
    NetworkCreated(Network),
    SubnetsLoaded { network_id: String, subnets: Vec<crate::port::types::Subnet> },
    SecurityGroupCreated(SecurityGroup),
    SecurityGroupDeleted { id: String },
    SecurityGroupRuleCreated(crate::models::neutron::SecurityGroupRule),
    SecurityGroupRuleDeleted { rule_id: String },
    VolumeCreated(Volume),
    VolumeDeleted { id: String },
    VolumeExtended { id: String },
    SnapshotCreated(VolumeSnapshot),
    SnapshotDeleted { id: String },
    ImageCreated(Image),
    ImageDeleted { id: String },
    FloatingIpCreated(FloatingIp),
    FloatingIpDeleted { id: String },

    // Keystone CUD
    ProjectCreated(Project),
    ProjectDeleted { id: String },
    UserCreated(User),
    UserDeleted { id: String },

    // Resize results
    ServerResized { id: String },
    ResizeConfirmed { id: String },
    ResizeReverted { id: String },

    // Migration results
    ServerLiveMigrated { id: String },
    ServerColdMigrated { id: String },
    MigrationConfirmed { id: String },
    MigrationReverted { id: String },
    ServerEvacuated { id: String },
    ServerEvacuateResult { id: String, result: Result<(), String> },
    ComputeServiceToggled { hostname: String, enabled: bool },
    MigrationProgressLoaded { server_id: String, migration: ServerMigration },
    MigrationPollingStopped { server_id: String },

    // Volume Attach/Detach results
    VolumeAttached { volume_id: String, server_id: String },
    VolumeDetached { volume_id: String },
    VolumeForceDetached { volume_id: String },
    VolumeStateReset { volume_id: String },

    // Floating IP Associate/Disassociate results
    FloatingIpAssociated(FloatingIp),
    FloatingIpDisassociated(FloatingIp),

    // Ports
    PortsLoaded { server_id: String, ports: Vec<Port> },

    // Server status polling (resize / cold-migrate state transitions)
    ServerStatusPolled { server: Server },

    // Error
    ApiError { operation: String, message: String },

    // Auth
    TokenRefreshed(Vec<crate::port::types::TokenRole>),
    AuthFailed(String),

    // RBAC
    PermissionDenied { operation: String },

    // System
    CloudSwitched(String),
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_app_event_variants_exist() {
        let events: Vec<AppEvent> = vec![
            AppEvent::ServersLoaded(vec![]),
            AppEvent::FlavorsLoaded(vec![]),
            AppEvent::NetworksLoaded(vec![]),
            AppEvent::VolumesLoaded(vec![]),
            AppEvent::ImagesLoaded(vec![]),
            AppEvent::ServerDeleted {
                id: "s1".into(),
                name: "web".into(),
            },
            AppEvent::ServerSnapshotCreated {
                server_id: "s1".into(),
                image_id: "img-1".into(),
            },
            AppEvent::FlavorDeleted { id: "f1".into() },
            AppEvent::ApiError {
                operation: "delete".into(),
                message: "not found".into(),
            },
            AppEvent::TokenRefreshed(vec![]),
            AppEvent::AuthFailed("expired".into()),
            AppEvent::CloudSwitched("prod".into()),
        ];
        assert!(events.len() >= 12);
    }

    #[test]
    fn test_token_refreshed_carries_roles() {
        use crate::port::types::TokenRole;
        let role = TokenRole { id: "r1".into(), name: "admin".into() };
        let event = AppEvent::TokenRefreshed(vec![role]);
        match event {
            AppEvent::TokenRefreshed(roles) => {
                assert_eq!(roles.len(), 1);
                assert_eq!(roles[0].name, "admin");
            }
            _ => panic!("expected TokenRefreshed"),
        }
    }

    #[test]
    fn test_migration_event_variants_exist() {
        use crate::models::nova::ServerMigration;
        let events: Vec<AppEvent> = vec![
            AppEvent::ServerLiveMigrated { id: "s1".into() },
            AppEvent::ServerColdMigrated { id: "s1".into() },
            AppEvent::MigrationConfirmed { id: "s1".into() },
            AppEvent::MigrationReverted { id: "s1".into() },
            AppEvent::ServerEvacuated { id: "s1".into() },
            AppEvent::MigrationProgressLoaded {
                server_id: "s1".into(),
                migration: ServerMigration {
                    id: 1,
                    status: "running".into(),
                    source_compute: "compute-01".into(),
                    dest_compute: "compute-02".into(),
                    memory_total_bytes: Some(1024),
                    memory_processed_bytes: Some(512),
                    memory_remaining_bytes: Some(512),
                    disk_total_bytes: Some(4096),
                    disk_processed_bytes: Some(2048),
                    disk_remaining_bytes: Some(2048),
                    created_at: None,
                    updated_at: None,
                },
            },
        ];
        assert_eq!(events.len(), 6);
    }

    #[test]
    fn test_resize_event_variants_exist() {
        let events: Vec<AppEvent> = vec![
            AppEvent::ServerResized { id: "s1".into() },
            AppEvent::ResizeConfirmed { id: "s1".into() },
            AppEvent::ResizeReverted { id: "s1".into() },
        ];
        assert_eq!(events.len(), 3);
    }

    #[test]
    fn test_server_status_polled_event() {
        use crate::models::nova::Server;
        let server = Server {
            id: "s1".into(),
            name: "test".into(),
            status: "VERIFY_RESIZE".into(),
            addresses: Default::default(),
            flavor: crate::models::nova::FlavorRef {
                id: "f1".into(),
                original_name: None,
                vcpus: None,
                ram: None,
                disk: None,
            },
            image: None,
            key_name: None,
            availability_zone: None,
            created: "2026-01-01".into(),
            updated: None,
            tenant_id: None,
            host_id: None,
            host: None,
            volumes_attached: vec![],
            security_groups: vec![],
        };
        let event = AppEvent::ServerStatusPolled { server };
        match event {
            AppEvent::ServerStatusPolled { server } => {
                assert_eq!(server.status, "VERIFY_RESIZE");
            }
            _ => panic!("expected ServerStatusPolled"),
        }
    }

    #[test]
    fn test_volume_fip_event_variants_exist() {
        use crate::models::neutron::{FloatingIp, Port, FixedIp};
        let events: Vec<AppEvent> = vec![
            AppEvent::VolumeAttached { volume_id: "v1".into(), server_id: "s1".into() },
            AppEvent::VolumeDetached { volume_id: "v1".into() },
            AppEvent::VolumeForceDetached { volume_id: "v1".into() },
            AppEvent::VolumeStateReset { volume_id: "v1".into() },
            AppEvent::FloatingIpAssociated(FloatingIp {
                id: "fip-1".into(),
                floating_ip_address: "203.0.113.10".into(),
                status: "ACTIVE".into(),
                port_id: Some("port-1".into()),
                floating_network_id: "ext-1".into(),
                fixed_ip_address: None,
                router_id: None,
                tenant_id: None,
            }),
            AppEvent::FloatingIpDisassociated(FloatingIp {
                id: "fip-1".into(),
                floating_ip_address: "203.0.113.10".into(),
                status: "DOWN".into(),
                port_id: None,
                floating_network_id: "ext-1".into(),
                fixed_ip_address: None,
                router_id: None,
                tenant_id: None,
            }),
            AppEvent::PortsLoaded {
                server_id: "s1".into(),
                ports: vec![Port {
                    id: "port-1".into(),
                    name: None,
                    network_id: "net-1".into(),
                    fixed_ips: vec![FixedIp { subnet_id: "sub-1".into(), ip_address: "10.0.0.5".into() }],
                    device_id: Some("s1".into()),
                    device_owner: Some("compute:az1".into()),
                    status: "ACTIVE".into(),
                    tenant_id: None,
                }],
            },
        ];
        assert_eq!(events.len(), 7);
    }

    #[test]
    fn test_usage_loaded_event() {
        use crate::port::types::TenantUsage;
        let usage = TenantUsage {
            tenant_id: "proj-1".into(),
            total_vcpus_usage: 4.0,
            total_memory_mb_usage: 8192.0,
            total_local_gb_usage: 100.0,
            total_hours: 720.0,
            server_usages: vec![],
        };
        let event = AppEvent::UsageLoaded(vec![usage]);
        match event {
            AppEvent::UsageLoaded(usages) => {
                assert_eq!(usages.len(), 1);
                assert_eq!(usages[0].tenant_id, "proj-1");
            }
            _ => panic!("expected UsageLoaded"),
        }
    }

    #[test]
    fn test_permission_denied_event() {
        let event = AppEvent::PermissionDenied { operation: "CreateServer".into() };
        match event {
            AppEvent::PermissionDenied { operation } => {
                assert_eq!(operation, "CreateServer");
            }
            _ => panic!("expected PermissionDenied"),
        }
    }
}

exec
/bin/zsh -lc "sed -n '120,520p' src/adapter/auth/keystone.rs" in /Users/jay.ahn/projects/infra/nexttui/.worktrees/runtime-context-switch
 succeeded in 0ms:

fn parse_interface(s: &str) -> EndpointInterface {
    match s {
        "internal" => EndpointInterface::Internal,
        "admin" => EndpointInterface::Admin,
        _ => EndpointInterface::Public,
    }
}

// --- KeystoneAuthAdapter ---

pub struct KeystoneAuthAdapter {
    client: reqwest::Client,
    credential: AuthCredential,
    token_map: Arc<RwLock<HashMap<TokenScope, Token>>>,
    active_scope: TokenScope,
    token_tx: broadcast::Sender<Token>,
    refresh_handle: Mutex<Option<JoinHandle<()>>>,
    /// Guard to ensure refresh loop is started only once.
    refresh_started: AtomicBool,
    /// Mutex to serialize concurrent refresh attempts (prevents thundering herd).
    refresh_lock: Mutex<()>,
    /// Directory for scope-keyed token cache files.
    cache_dir: PathBuf,
}

impl KeystoneAuthAdapter {
    pub fn new(credential: AuthCredential) -> Result<Self, ApiError> {
        use super::token_cache;

        let username = match &credential.method {
            AuthMethod::Password { username, .. } => username.clone(),
            AuthMethod::ApplicationCredential { id, .. } => id.clone(),
        };
        let active_scope = TokenScope::from_credential(&credential);
        let cloud_key = token_cache::compute_cloud_key(&credential.auth_url, &username);
        let cache_dir = token_cache::cache_dir_path(&cloud_key);

        // Load all cached tokens for this cloud from disk
        let cached_tokens = token_cache::load_all_tokens(&cache_dir);

        let (token_tx, _) = broadcast::channel::<Token>(16);
        Ok(Self {
            client: reqwest::Client::builder()
                .timeout(Duration::from_secs(30))
                .connect_timeout(Duration::from_secs(10))
                .build()?,
            credential,
            token_map: Arc::new(RwLock::new(cached_tokens)),
            active_scope,
            token_tx,
            refresh_handle: Mutex::new(None),
            refresh_started: AtomicBool::new(false),
            refresh_lock: Mutex::new(()),
            cache_dir,
        })
    }

    /// Start the background token refresh loop. Idempotent — only spawns once.
    #[tracing::instrument(skip(self))]
    async fn start_refresh_loop(&self) {
        if self.refresh_started.swap(true, Ordering::SeqCst) {
            return; // Already started
        }

        let token_map_ref = self.token_map.clone();
        let client = self.client.clone();
        let credential = self.credential.clone();
        let tx = self.token_tx.clone();
        let cache_dir = self.cache_dir.clone();
        let scope = self.active_scope.clone();

        let refresh_span = tracing::info_span!("token_refresh_loop");
        let handle = tokio::spawn(
            async move {
                loop {
                    let sleep_duration = {
                        let map = token_map_ref.read().await;
                        match map.get(&scope) {
                            Some(t) => {
                                let remaining = t.expires_at - Utc::now();
                                let refresh_at = remaining - chrono::Duration::minutes(5);
                                if refresh_at.num_seconds() > 0 {
                                    Duration::from_secs(refresh_at.num_seconds() as u64)
                                } else {
                                    Duration::from_secs(10)
                                }
                            }
                            None => Duration::from_secs(60),
                        }
                    };

                    tokio::time::sleep(sleep_duration).await;

                    match Self::do_authenticate(&client, &credential).await {
                        Ok(new_token) => {
                            let mut map = token_map_ref.write().await;
                            map.insert(scope.clone(), new_token.clone());
                            if let Err(e) = super::token_cache::save_token(&new_token, &cache_dir, &scope) {
                                tracing::warn!(error = %e, "failed to cache token to disk");
                            }
                            let _ = tx.send(new_token);
                        }
                        Err(e) => {
                            tracing::warn!(error = %e, "token refresh failed, retrying in 30s");
                            tokio::time::sleep(Duration::from_secs(30)).await;
                        }
                    }
                }
            }
            .instrument(refresh_span),
        );

        let mut h = self.refresh_handle.lock().await;
        *h = Some(handle);
    }

    /// Perform the actual Keystone v3 auth POST.
    #[tracing::instrument(skip(client, credential), fields(auth_url = %credential.auth_url))]
    async fn do_authenticate(
        client: &reqwest::Client,
        credential: &AuthCredential,
    ) -> ApiResult<Token> {
        let auth_url = format!(
            "{}/auth/tokens",
            credential.auth_url.trim_end_matches('/')
        );
        let body = Self::build_auth_body(credential);
        let resp = client
            .post(&auth_url)
            .json(&body)
            .send()
            .await
            .map_err(ApiError::Network)?;

        if !resp.status().is_success() {
            let body = resp.text().await.unwrap_or_default();
            return Err(ApiError::AuthFailed(body));
        }

        let token_id = resp
            .headers()
            .get("X-Subject-Token")
            .and_then(|v| v.to_str().ok())
            .ok_or_else(|| ApiError::AuthFailed("Missing X-Subject-Token header".into()))?
            .to_string();

        let body: KeystoneTokenResponse = resp
            .json()
            .await
            .map_err(|e| ApiError::Parse(format!("Failed to parse Keystone response: {e}")))?;

        Ok(parse_token(token_id, body))
    }

    /// Build the Keystone v3 auth request body based on AuthMethod.
    /// Note: authenticate() always uses self.credential (passed at construction).
    /// The credential parameter in AuthProvider::authenticate() should match self.credential.
    pub(crate) fn build_auth_body(credential: &AuthCredential) -> serde_json::Value {
        let identity = match &credential.method {
            AuthMethod::Password {
                username,
                password,
                domain_name,
            } => serde_json::json!({
                "methods": ["password"],
                "password": {
                    "user": {
                        "name": username,
                        "password": password,
                        "domain": { "name": domain_name }
                    }
                }
            }),
            AuthMethod::ApplicationCredential { id, secret } => serde_json::json!({
                "methods": ["application_credential"],
                "application_credential": {
                    "id": id,
                    "secret": secret
                }
            }),
        };

        let mut auth = serde_json::json!({ "identity": identity });

        if let Some(ref scope) = credential.project_scope {
            auth["scope"] = serde_json::json!({
                "project": {
                    "name": scope.name,
                    "domain": { "name": scope.domain_name }
                }
            });
        }

        serde_json::json!({ "auth": auth })
    }
}

#[async_trait]
impl AuthProvider for KeystoneAuthAdapter {
    async fn authenticate(&self, credential: &AuthCredential) -> ApiResult<Token> {
        let token = Self::do_authenticate(&self.client, credential).await?;
        {
            let mut map = self.token_map.write().await;
            map.insert(self.active_scope.clone(), token.clone());
        }
        if let Err(e) = super::token_cache::save_token(&token, &self.cache_dir, &self.active_scope) {
            tracing::warn!(error = %e, "failed to cache token to disk");
        }
        self.start_refresh_loop().await;
        Ok(token)
    }

    #[tracing::instrument(skip(self))]
    async fn refresh_token(&self) -> ApiResult<Token> {
        let token = Self::do_authenticate(&self.client, &self.credential).await?;
        {
            let mut map = self.token_map.write().await;
            map.insert(self.active_scope.clone(), token.clone());
        }
        if let Err(e) = super::token_cache::save_token(&token, &self.cache_dir, &self.active_scope) {
            tracing::warn!(error = %e, "failed to cache token to disk");
        }
        let _ = self.token_tx.send(token.clone());
        Ok(token)
    }

    /// Get a valid token string. If near-expiry (<1min), refresh first.
    /// Uses a Mutex to prevent thundering herd — only one refresh at a time.
    #[tracing::instrument(skip(self))]
    async fn get_token(&self) -> ApiResult<String> {
        // Ensure refresh loop is running (idempotent — handles cached token from disk)
        self.start_refresh_loop().await;

        // Fast path: token is still valid for active scope
        {
            let map = self.token_map.read().await;
            if let Some(t) = map.get(&self.active_scope) {
                if t.expires_at > Utc::now() + chrono::Duration::minutes(1) {
                    return Ok(t.id.clone());
                }
            }
        }

        // Slow path: serialize refresh attempts
        let _guard = self.refresh_lock.lock().await;

        // Double-check after acquiring lock
        {
            let map = self.token_map.read().await;
            if let Some(t) = map.get(&self.active_scope) {
                if t.expires_at > Utc::now() + chrono::Duration::minutes(1) {
                    return Ok(t.id.clone());
                }
            }
        }

        let token = self.refresh_token().await?;
        Ok(token.id)
    }

    async fn get_token_info(&self) -> ApiResult<Token> {
        let map = self.token_map.read().await;
        map.get(&self.active_scope)
            .cloned()
            .ok_or(ApiError::AuthFailed("Not authenticated".into()))
    }

    /// Inject X-Auth-Token header. Phase 1: token-based auth only.
    /// Phase 2 note: for signed auth (HMAC), this method will need the actual
    /// method/url/headers/body to compute the signature. Currently unused parameters
    /// are preserved in the signature for forward compatibility.
    #[tracing::instrument(skip(self, _headers, _body))]
    async fn authenticate_request(
        &self,
        _method: &str,
        _url: &str,
        _headers: &reqwest::header::HeaderMap,
        _body: Option<&[u8]>,
    ) -> ApiResult<AuthHeaders> {
        let token_id = self.get_token().await?;
        Ok(AuthHeaders {
            headers: vec![("X-Auth-Token".to_string(), token_id)],
        })
    }

    #[tracing::instrument(skip(self))]
    async fn get_endpoint(
        &self,
        service_type: &str,
        interface: EndpointInterface,
        region: Option<&str>,
    ) -> ApiResult<String> {
        // Ensure we have a valid token (triggers initial auth if needed)
        let _ = self.get_token().await?;

        let token = self.get_token_info().await?;

        token
            .catalog
            .iter()
            .find(|c| c.service_type == service_type)
            .and_then(|c| {
                c.endpoints.iter().find(|e| {
                    e.interface == interface && region.map_or(true, |r| e.region == r)
                })
            })
            .map(|e| e.url.clone())
            .ok_or(ApiError::ServiceUnavailable {
                service: service_type.to_string(),
            })
    }

    fn subscribe_token_refresh(&self) -> broadcast::Receiver<Token> {
        self.token_tx.subscribe()
    }

    async fn has_role(&self, role_name: &str) -> ApiResult<bool> {
        let map = self.token_map.read().await;
        let token = map
            .get(&self.active_scope)
            .ok_or(ApiError::AuthFailed("Not authenticated".into()))?;
        Ok(token.roles.iter().any(|r| r.name == role_name))
    }

    async fn get_catalog(&self) -> ApiResult<Vec<CatalogEntry>> {
        let map = self.token_map.read().await;
        let token = map
            .get(&self.active_scope)
            .ok_or(ApiError::AuthFailed("Not authenticated".into()))?;
        Ok(token.catalog.clone())
    }

    async fn get_capabilities(&self) -> ApiResult<Vec<Capability>> {
        // Phase 1: Keystone has no capability concept. Return empty.
        Ok(Vec::new())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_credential_password() -> AuthCredential {
        AuthCredential {
            auth_url: "https://keystone.example.com:5000/v3".to_string(),
            method: AuthMethod::Password {
                username: "admin".to_string(),
                password: "secret123".to_string(),
                domain_name: "Default".to_string(),
            },
            project_scope: Some(ProjectScopeParam {
                name: "admin-project".to_string(),
                domain_name: "Default".to_string(),
            }),
        }
    }

    fn sample_credential_app() -> AuthCredential {
        AuthCredential {
            auth_url: "https://keystone.example.com:5000/v3".to_string(),
            method: AuthMethod::ApplicationCredential {
                id: "app-cred-id".to_string(),
                secret: "app-cred-secret".to_string(),
            },
            project_scope: None,
        }
    }

    fn sample_keystone_response_json() -> &'static str {
        r#"{
            "token": {
                "expires_at": "2099-12-31T23:59:59.000000Z",
                "project": {
                    "id": "proj-123",
                    "name": "admin-project",
                    "domain": { "id": "default", "name": "Default" }
                },
                "roles": [
                    { "id": "role-1", "name": "admin" },
                    { "id": "role-2", "name": "member" }
                ],
                "catalog": [
                    {
                        "type": "compute",
                        "name": "nova",
                        "endpoints": [
                            { "url": "https://nova:8774/v2.1", "interface": "internal", "region_id": "RegionOne" },
                            { "url": "https://nova-pub:8774/v2.1", "interface": "public", "region_id": "RegionOne" }
                        ]
                    },
                    {
                        "type": "identity",
                        "name": "keystone",
                        "endpoints": [
                            { "url": "https://keystone:5000/v3", "interface": "public", "region_id": "RegionOne" }
                        ]
                    }
                ]
            }
        }"#

exec
/bin/zsh -lc "sed -n '1,220p' src/port/auth.rs" in /Users/jay.ahn/projects/infra/nexttui/.worktrees/runtime-context-switch
 succeeded in 0ms:
use async_trait::async_trait;
use tokio::sync::broadcast;

use super::error::ApiResult;
use super::types::*;

#[async_trait]
pub trait AuthProvider: Send + Sync {
    async fn authenticate(&self, credential: &AuthCredential) -> ApiResult<Token>;
    async fn refresh_token(&self) -> ApiResult<Token>;
    async fn get_token(&self) -> ApiResult<String>;
    async fn get_token_info(&self) -> ApiResult<Token>;
    async fn authenticate_request(
        &self,
        method: &str,
        url: &str,
        headers: &reqwest::header::HeaderMap,
        body: Option<&[u8]>,
    ) -> ApiResult<AuthHeaders>;
    async fn get_endpoint(
        &self,
        service_type: &str,
        interface: EndpointInterface,
        region: Option<&str>,
    ) -> ApiResult<String>;
    fn subscribe_token_refresh(&self) -> broadcast::Receiver<Token>;
    async fn has_role(&self, role_name: &str) -> ApiResult<bool>;
    async fn get_catalog(&self) -> ApiResult<Vec<CatalogEntry>>;
    async fn get_capabilities(&self) -> ApiResult<Vec<Capability>>;
}

exec
/bin/zsh -lc "sed -n '1,240p' src/port/keystone.rs" in /Users/jay.ahn/projects/infra/nexttui/.worktrees/runtime-context-switch
 succeeded in 0ms:
use async_trait::async_trait;

use super::error::ApiResult;
use super::types::*;
use crate::models::keystone::{Project, Role, RoleAssignment, User};

#[async_trait]
pub trait KeystonePort: Send + Sync {
    // Projects
    async fn list_projects(
        &self,
        pagination: &PaginationParams,
    ) -> ApiResult<PaginatedResponse<Project>>;
    async fn get_project(&self, project_id: &str) -> ApiResult<Project>;
    async fn create_project(&self, params: &ProjectCreateParams) -> ApiResult<Project>;
    async fn update_project(
        &self,
        project_id: &str,
        params: &ProjectUpdateParams,
    ) -> ApiResult<Project>;
    async fn delete_project(&self, project_id: &str) -> ApiResult<()>;

    // Users
    async fn list_users(&self, pagination: &PaginationParams)
    -> ApiResult<PaginatedResponse<User>>;
    async fn get_user(&self, user_id: &str) -> ApiResult<User>;
    async fn create_user(&self, params: &UserCreateParams) -> ApiResult<User>;
    async fn update_user(&self, user_id: &str, params: &UserUpdateParams) -> ApiResult<User>;
    async fn delete_user(&self, user_id: &str) -> ApiResult<()>;

    // Roles
    async fn list_roles(&self) -> ApiResult<Vec<Role>>;
    async fn assign_role(&self, params: &RoleAssignmentParams) -> ApiResult<()>;
    async fn revoke_role(&self, params: &RoleAssignmentParams) -> ApiResult<()>;
    async fn list_role_assignments(
        &self,
        filter: &RoleAssignmentFilter,
    ) -> ApiResult<Vec<RoleAssignment>>;

    // Domains
    async fn list_domains(&self) -> ApiResult<Vec<Domain>>;
}

codex
I’m validating interfaces now; there’s a likely seam gap because current `AuthProvider`/`KeystonePort` don’t expose rescope operations, which affects both component completeness and PR safety.
exec
/bin/zsh -lc "sed -n '1,220p' src/adapter/http/base.rs" in /Users/jay.ahn/projects/infra/nexttui/.worktrees/runtime-context-switch
 succeeded in 0ms:
use std::sync::Arc;
use std::time::Duration;

use reqwest::{Method, RequestBuilder, Response};
use serde::de::DeserializeOwned;
use tokio::sync::RwLock;

use crate::port::auth::AuthProvider;
use crate::port::error::{ApiError, ApiResult};
use crate::port::types::EndpointInterface;

/// Shared HTTP plumbing for all service adapters.
/// Auth injection is delegated to AuthProvider::authenticate_request().
///
/// Endpoint caching note: cached endpoint is resolved once and reused.
/// Callers should call `invalidate_endpoint()` when token refresh occurs
/// to pick up potential catalog changes. In Phase 2, BaseHttpClient will
/// subscribe to token refresh broadcast to automate this.
pub struct BaseHttpClient {
    client: reqwest::Client,
    auth: Arc<dyn AuthProvider>,
    service_type: String,
    interface: EndpointInterface,
    region: Option<String>,
    endpoint: RwLock<Option<String>>,
}

impl BaseHttpClient {
    pub fn new(
        auth: Arc<dyn AuthProvider>,
        service_type: &str,
        interface: EndpointInterface,
        region: Option<String>,
    ) -> Result<Self, ApiError> {
        Ok(Self {
            client: reqwest::Client::builder()
                .timeout(Duration::from_secs(30))
                .connect_timeout(Duration::from_secs(10))
                .build()?,
            auth,
            service_type: service_type.to_string(),
            interface,
            region,
            endpoint: RwLock::new(None),
        })
    }

    /// Resolve and cache the endpoint from service catalog.
    #[tracing::instrument(skip(self), fields(service = %self.service_type))]
    async fn resolve_endpoint(&self) -> ApiResult<String> {
        {
            let cached = self.endpoint.read().await;
            if let Some(url) = cached.as_ref() {
                return Ok(url.clone());
            }
        }
        let url = self
            .auth
            .get_endpoint(&self.service_type, self.interface.clone(), self.region.as_deref())
            .await?;
        let mut cached = self.endpoint.write().await;
        *cached = Some(url.clone());
        Ok(url)
    }

    /// Invalidate cached endpoint. Should be called on token refresh
    /// to pick up potential service catalog changes.
    pub async fn invalidate_endpoint(&self) {
        let mut cached = self.endpoint.write().await;
        *cached = None;
    }

    /// Build an authenticated request.
    /// Note (Phase 2): For signed auth methods (HMAC), authenticate_request()
    /// will need actual headers/body. Currently passes empty values since
    /// Phase 1 only uses X-Auth-Token which doesn't depend on request content.
    async fn request(&self, method: Method, path: &str) -> ApiResult<RequestBuilder> {
        let endpoint = self.resolve_endpoint().await?;
        let url = format!("{}{}", endpoint.trim_end_matches('/'), path);
        let method_str = method.as_str();
        let empty_headers = reqwest::header::HeaderMap::new();
        let auth_headers = self
            .auth
            .authenticate_request(method_str, &url, &empty_headers, None)
            .await?;
        let mut builder = self
            .client
            .request(method, &url)
            .header("Content-Type", "application/json");
        for (key, value) in &auth_headers.headers {
            builder = builder.header(key.as_str(), value.as_str());
        }
        Ok(builder)
    }

    pub async fn get(&self, path: &str) -> ApiResult<RequestBuilder> {
        self.request(Method::GET, path).await
    }

    pub async fn post(&self, path: &str) -> ApiResult<RequestBuilder> {
        self.request(Method::POST, path).await
    }

    pub async fn put(&self, path: &str) -> ApiResult<RequestBuilder> {
        self.request(Method::PUT, path).await
    }

    pub async fn patch(&self, path: &str) -> ApiResult<RequestBuilder> {
        self.request(Method::PATCH, path).await
    }

    pub async fn delete(&self, path: &str) -> ApiResult<RequestBuilder> {
        self.request(Method::DELETE, path).await
    }

    /// Send a request and map HTTP errors to ApiError.
    pub async fn send(&self, request: RequestBuilder) -> ApiResult<Response> {
        let resp = request.send().await.map_err(ApiError::Network)?;
        Self::check_status(resp).await
    }

    /// Send + deserialize JSON body.
    pub async fn send_json<T: DeserializeOwned>(
        &self,
        request: RequestBuilder,
    ) -> ApiResult<T> {
        let resp = self.send(request).await?;
        resp.json::<T>()
            .await
            .map_err(|e| ApiError::Parse(format!("JSON deserialization failed: {e}")))
    }

    /// Send and expect 204 No Content (or 202 Accepted).
    pub async fn send_no_content(&self, request: RequestBuilder) -> ApiResult<()> {
        self.send(request).await?;
        Ok(())
    }

    /// Extract a human-readable error message from OpenStack JSON error bodies.
    /// Handles formats: {"NeutronError": {"message": "..."}}, {"badRequest": {"message": "..."}},
    /// {"error": {"message": "..."}}, {"itemNotFound": {"message": "..."}}, etc.
    fn extract_error_message(body: &str) -> String {
        if let Ok(json) = serde_json::from_str::<serde_json::Value>(body) {
            // Try common OpenStack error wrapper keys
            for key in &["NeutronError", "badRequest", "itemNotFound", "conflictingRequest",
                         "forbidden", "error", "computeFault"] {
                if let Some(inner) = json.get(key) {
                    if let Some(msg) = inner.get("message").and_then(|m| m.as_str()) {
                        return msg.to_string();
                    }
                }
            }
            // Fallback: try any top-level object with a "message" field
            if let Some(obj) = json.as_object() {
                for (_k, v) in obj {
                    if let Some(msg) = v.get("message").and_then(|m| m.as_str()) {
                        return msg.to_string();
                    }
                }
            }
        }
        // Not JSON or no message found — return raw body
        body.to_string()
    }

    /// Map HTTP status codes to ApiError.
    pub(crate) async fn check_status(resp: Response) -> ApiResult<Response> {
        let status = resp.status();
        if status.is_success() {
            return Ok(resp);
        }
        let raw_body = resp.text().await.unwrap_or_default();
        let body = Self::extract_error_message(&raw_body);
        match status.as_u16() {
            401 => Err(ApiError::TokenExpired),
            403 => Err(ApiError::Forbidden(body)),
            404 => Err(ApiError::NotFound {
                resource_type: String::new(),
                id: body,
            }),
            409 => Err(ApiError::Conflict(body)),
            400 => Err(ApiError::BadRequest(body)),
            429 => Err(ApiError::RateLimited {
                retry_after_secs: 60,
            }),
            503 => Err(ApiError::ServiceUnavailable {
                service: String::new(),
            }),
            _ => Err(ApiError::Unexpected {
                status: status.as_u16(),
                body,
            }),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use ::http::StatusCode;

    fn mock_response(status: u16, body: &str) -> Response {
        ::http::Response::builder()
            .status(StatusCode::from_u16(status).unwrap())
            .body(body.to_string())
            .unwrap()
            .into()
    }

    #[tokio::test]
    async fn test_check_status_success() {
        let resp = mock_response(200, r#"{"ok": true}"#);
        let result = BaseHttpClient::check_status(resp).await;
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_check_status_401() {
        let resp = mock_response(401, "unauthorized");
        let err = BaseHttpClient::check_status(resp).await.unwrap_err();

exec
/bin/zsh -lc "sed -n '1,220p' src/adapter/http/keystone.rs" in /Users/jay.ahn/projects/infra/nexttui/.worktrees/runtime-context-switch
 succeeded in 0ms:
use std::sync::Arc;

use async_trait::async_trait;
use serde::{Deserialize, Serialize};

use super::{build_pagination_query, encode_param, extract_marker_from_url, paginated_list};
use crate::adapter::http::base::BaseHttpClient;
use crate::models::keystone::{Project, Role, RoleAssignment, User};
use crate::port::auth::AuthProvider;
use crate::port::error::{ApiError, ApiResult};
use crate::port::keystone::KeystonePort;
use crate::port::types::*;

pub struct KeystoneHttpAdapter {
    base: BaseHttpClient,
}

impl KeystoneHttpAdapter {
    pub fn new(auth: Arc<dyn AuthProvider>, region: Option<String>) -> Result<Self, ApiError> {
        Ok(Self {
            base: BaseHttpClient::new(auth, "identity", EndpointInterface::Public, region)?,
        })
    }
}

// --- JSON wrapper structs ---

#[derive(Deserialize)]
struct KeystoneProjectsResponse {
    projects: Vec<Project>,
    links: Option<KeystoneLinks>,
}

#[derive(Deserialize)]
struct KeystoneProjectWrapper {
    project: Project,
}

#[derive(Deserialize)]
struct KeystoneUsersResponse {
    users: Vec<User>,
    links: Option<KeystoneLinks>,
}

#[derive(Deserialize)]
struct KeystoneUserWrapper {
    user: User,
}

#[derive(Deserialize)]
struct KeystoneRolesResponse {
    roles: Vec<Role>,
}

#[derive(Deserialize)]
struct KeystoneRoleAssignmentsResponse {
    role_assignments: Vec<RoleAssignment>,
}

#[derive(Deserialize)]
struct KeystoneDomainsResponse {
    domains: Vec<Domain>,
}

// Keystone uses {"links": {"next": "url", "previous": "url"}} pattern
#[derive(Deserialize)]
struct KeystoneLinks {
    next: Option<String>,
}

fn extract_keystone_marker(links: &Option<KeystoneLinks>) -> Option<String> {
    links.as_ref().and_then(|l| l.next.as_deref().and_then(extract_marker_from_url))
}

// --- Serialize structs ---

#[derive(Serialize)]
struct ProjectCreateBody {
    project: ProjectCreateInner,
}

#[derive(Serialize)]
struct ProjectCreateInner {
    name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    description: Option<String>,
    domain_id: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    enabled: Option<bool>,
}

#[derive(Serialize)]
struct ProjectUpdateBody {
    project: ProjectUpdateInner,
}

#[derive(Serialize)]
struct ProjectUpdateInner {
    #[serde(skip_serializing_if = "Option::is_none")]
    name: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    description: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    enabled: Option<bool>,
}

#[derive(Serialize)]
struct UserCreateBody {
    user: UserCreateInner,
}

#[derive(Serialize)]
struct UserCreateInner {
    name: String,
    password: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    email: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    default_project_id: Option<String>,
    domain_id: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    enabled: Option<bool>,
}

#[derive(Serialize)]
struct UserUpdateBody {
    user: UserUpdateInner,
}

#[derive(Serialize)]
struct UserUpdateInner {
    #[serde(skip_serializing_if = "Option::is_none")]
    name: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    password: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    email: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    enabled: Option<bool>,
}

// --- KeystonePort implementation ---

#[async_trait]
impl KeystonePort for KeystoneHttpAdapter {
    // -- Projects --

    async fn list_projects(
        &self,
        pagination: &PaginationParams,
    ) -> ApiResult<PaginatedResponse<Project>> {
        let query = build_pagination_query(pagination);
        paginated_list(&self.base, "/v3/projects", &query, |resp: KeystoneProjectsResponse| {
            let next = extract_keystone_marker(&resp.links);
            (resp.projects, next)
        }).await
    }

    async fn get_project(&self, project_id: &str) -> ApiResult<Project> {
        let req = self
            .base
            .get(&format!("/v3/projects/{}", encode_param(project_id)))
            .await?;
        let resp: KeystoneProjectWrapper = self.base.send_json(req).await?;
        Ok(resp.project)
    }

    async fn create_project(&self, params: &ProjectCreateParams) -> ApiResult<Project> {
        let body = ProjectCreateBody {
            project: ProjectCreateInner {
                name: params.name.clone(),
                description: params.description.clone(),
                domain_id: params.domain_id.clone(),
                enabled: params.enabled,
            },
        };
        let req = self.base.post("/v3/projects").await?.json(&body);
        let resp: KeystoneProjectWrapper = self.base.send_json(req).await?;
        Ok(resp.project)
    }

    async fn update_project(
        &self,
        project_id: &str,
        params: &ProjectUpdateParams,
    ) -> ApiResult<Project> {
        let body = ProjectUpdateBody {
            project: ProjectUpdateInner {
                name: params.name.clone(),
                description: params.description.clone(),
                enabled: params.enabled,
            },
        };
        let req = self
            .base
            .put(&format!("/v3/projects/{}", encode_param(project_id)))
            .await?
            .json(&body);
        let resp: KeystoneProjectWrapper = self.base.send_json(req).await?;
        Ok(resp.project)
    }

    async fn delete_project(&self, project_id: &str) -> ApiResult<()> {
        let req = self
            .base
            .delete(&format!("/v3/projects/{}", encode_param(project_id)))
            .await?;
        self.base.send_no_content(req).await
    }

    // -- Users --

    async fn list_users(
        &self,
        pagination: &PaginationParams,
    ) -> ApiResult<PaginatedResponse<User>> {
        let query = build_pagination_query(pagination);
        paginated_list(&self.base, "/v3/users", &query, |resp: KeystoneUsersResponse| {
            let next = extract_keystone_marker(&resp.links);
            (resp.users, next)

exec
/bin/zsh -lc "sed -n '1,240p' src/registry.rs" in /Users/jay.ahn/projects/infra/nexttui/.worktrees/runtime-context-switch
 succeeded in 0ms:
use std::collections::HashMap;

use crate::action::Action;
use crate::component::Component;
use crate::models::common::Route;
use crate::ui::sidebar::SidebarItem;

pub struct ModuleEntry {
    pub sidebar: SidebarItem,
    pub component: Box<dyn Component>,
    pub initial_action: Option<Action>,
    pub related_routes: &'static [Route],
    pub display_name: &'static str,
}

pub struct RegistryParts {
    pub components: HashMap<Route, Box<dyn Component>>,
    pub sidebar_items: Vec<SidebarItem>,
    pub initial_actions: Vec<Action>,
    pub route_labels: HashMap<Route, &'static str>,
}

pub struct ModuleRegistry {
    entries: Vec<ModuleEntry>,
    extra_labels: Vec<(Route, &'static str)>,
}

impl ModuleRegistry {
    pub fn new() -> Self {
        Self { entries: Vec::new(), extra_labels: Vec::new() }
    }

    pub fn register(&mut self, entry: ModuleEntry) {
        self.entries.push(entry);
    }

    /// Add a display label for a route that has no module yet.
    pub fn add_route_label(&mut self, route: Route, label: &'static str) {
        self.extra_labels.push((route, label));
    }

    pub fn into_parts(self) -> RegistryParts {
        let mut components = HashMap::new();
        let mut sidebar_items = Vec::new();
        let mut initial_actions = Vec::new();
        let mut route_labels = HashMap::new();

        for entry in self.entries {
            let route = entry.sidebar.route;
            debug_assert!(!components.contains_key(&route), "Duplicate route registered: {:?}", route);
            route_labels.insert(route, entry.display_name);
            for &related in entry.related_routes {
                debug_assert!(!route_labels.contains_key(&related), "Duplicate related route: {:?}", related);
                route_labels.insert(related, entry.display_name);
            }
            sidebar_items.push(entry.sidebar);
            if let Some(action) = entry.initial_action {
                initial_actions.push(action);
            }
            components.insert(route, entry.component);
        }

        // Add extra labels for routes without modules
        for (route, label) in self.extra_labels {
            route_labels.entry(route).or_insert(label);
        }

        RegistryParts {
            components,
            sidebar_items,
            initial_actions,
            route_labels,
        }
    }
}

impl Default for ModuleRegistry {
    fn default() -> Self {
        Self::new()
    }
}

/// Register all standard modules. Shared by main.rs and demo.rs.
pub fn register_all_modules(
    registry: &mut ModuleRegistry,
    action_tx: &tokio::sync::mpsc::UnboundedSender<Action>,
) {
    use crate::module::{
        server::ServerModule, flavor::FlavorModule, network::NetworkModule,
        security_group::SecurityGroupModule, floating_ip::FloatingIpModule,
        volume::VolumeModule, snapshot::SnapshotModule, image::ImageModule,
        project::ProjectModule, user::UserModule,
        host::HostModule, usage::UsageModule,
    };

    let entries = vec![
        ModuleEntry {
            sidebar: SidebarItem { label: "Servers".into(), route: Route::Servers, shortcut: "1".into(), admin_only: false },
            component: Box::new(ServerModule::new(action_tx.clone())),
            initial_action: Some(Action::FetchServers),
            related_routes: &[Route::ServerDetail, Route::ServerCreate],
            display_name: "Servers",
        },
        ModuleEntry {
            sidebar: SidebarItem { label: "Flavors".into(), route: Route::Flavors, shortcut: "2".into(), admin_only: false },
            component: Box::new(FlavorModule::new(action_tx.clone())),
            initial_action: Some(Action::FetchFlavors),
            related_routes: &[],
            display_name: "Flavors",
        },
        ModuleEntry {
            sidebar: SidebarItem { label: "Networks".into(), route: Route::Networks, shortcut: "3".into(), admin_only: false },
            component: Box::new(NetworkModule::new(action_tx.clone())),
            initial_action: Some(Action::FetchNetworks),
            related_routes: &[Route::NetworkDetail],
            display_name: "Networks",
        },
        ModuleEntry {
            sidebar: SidebarItem { label: "Security Groups".into(), route: Route::SecurityGroups, shortcut: "4".into(), admin_only: false },
            component: Box::new(SecurityGroupModule::new(action_tx.clone())),
            initial_action: Some(Action::FetchSecurityGroups),
            related_routes: &[Route::SecurityGroupDetail],
            display_name: "Security Groups",
        },
        ModuleEntry {
            sidebar: SidebarItem { label: "Floating IPs".into(), route: Route::FloatingIps, shortcut: "5".into(), admin_only: false },
            component: Box::new(FloatingIpModule::new(action_tx.clone())),
            initial_action: Some(Action::FetchFloatingIps),
            related_routes: &[],
            display_name: "Floating IPs",
        },
        ModuleEntry {
            sidebar: SidebarItem { label: "Volumes".into(), route: Route::Volumes, shortcut: "6".into(), admin_only: false },
            component: Box::new(VolumeModule::new(action_tx.clone())),
            initial_action: Some(Action::FetchVolumes),
            related_routes: &[Route::VolumeDetail, Route::VolumeCreate],
            display_name: "Volumes",
        },
        ModuleEntry {
            sidebar: SidebarItem { label: "Snapshots".into(), route: Route::Snapshots, shortcut: "7".into(), admin_only: false },
            component: Box::new(SnapshotModule::new(action_tx.clone())),
            initial_action: Some(Action::FetchSnapshots),
            related_routes: &[],
            display_name: "Snapshots",
        },
        ModuleEntry {
            sidebar: SidebarItem { label: "Images".into(), route: Route::Images, shortcut: "8".into(), admin_only: false },
            component: Box::new(ImageModule::new(action_tx.clone())),
            initial_action: Some(Action::FetchImages),
            related_routes: &[Route::ImageDetail],
            display_name: "Images",
        },
        ModuleEntry {
            sidebar: SidebarItem { label: "Projects".into(), route: Route::Projects, shortcut: "9".into(), admin_only: true },
            component: Box::new(ProjectModule::new(action_tx.clone())),
            initial_action: Some(Action::FetchProjects),
            related_routes: &[],
            display_name: "Projects",
        },
        ModuleEntry {
            sidebar: SidebarItem { label: "Users".into(), route: Route::Users, shortcut: "0".into(), admin_only: true },
            component: Box::new(UserModule::new(action_tx.clone())),
            initial_action: Some(Action::FetchUsers),
            related_routes: &[],
            display_name: "Users",
        },
        ModuleEntry {
            sidebar: SidebarItem { label: "Host Ops".into(), route: Route::Hosts, shortcut: "h".into(), admin_only: true },
            component: Box::new(HostModule::new(action_tx.clone())),
            initial_action: Some(Action::FetchHypervisors),
            related_routes: &[],
            display_name: "Host Ops",
        },
        ModuleEntry {
            sidebar: SidebarItem { label: "Usage".into(), route: Route::Usage, shortcut: "u".into(), admin_only: true },
            component: Box::new(UsageModule::new(action_tx.clone())),
            initial_action: None,
            related_routes: &[],
            display_name: "Usage",
        },
    ];

    for entry in entries {
        registry.register(entry);
    }

    // Routes with no module yet — display name only (no sidebar, no component)
    registry.add_route_label(Route::Migrations, "Migrations");
    registry.add_route_label(Route::Aggregates, "Aggregates");
    registry.add_route_label(Route::ComputeServices, "Compute Services");
    registry.add_route_label(Route::Hypervisors, "Hypervisors");
    registry.add_route_label(Route::Agents, "Agents");
}

#[cfg(test)]
mod tests {
    use super::*;
    use crossterm::event::KeyEvent;
    use ratatui::layout::Rect;
    use ratatui::Frame;
    use crate::event::AppEvent;

    struct DummyComponent;
    impl Component for DummyComponent {
        fn handle_key(&mut self, _key: KeyEvent) -> Option<Action> { None }
        fn handle_event(&mut self, _event: &AppEvent) {}
        fn render(&self, _frame: &mut Frame, _area: Rect) {}
    }

    fn make_entry(route: Route, label: &str, shortcut: &str, display_name: &'static str, related: &'static [Route], action: Option<Action>) -> ModuleEntry {
        ModuleEntry {
            sidebar: SidebarItem {
                label: label.into(),
                route,
                shortcut: shortcut.into(),
                admin_only: false,
            },
            component: Box::new(DummyComponent),
            initial_action: action,
            related_routes: related,
            display_name,
        }
    }

    #[test]
    fn test_module_entry_sidebar_item() {
        let entry = make_entry(Route::Servers, "Servers", "1", "Servers", &[], None);
        assert_eq!(entry.sidebar.label, "Servers");
        assert_eq!(entry.sidebar.route, Route::Servers);
        assert_eq!(entry.display_name, "Servers");
    }

    #[test]
    fn test_registry_register_and_count() {
        let mut registry = ModuleRegistry::new();
        registry.register(make_entry(Route::Servers, "Servers", "1", "Servers", &[], None));
        registry.register(make_entry(Route::Flavors, "Flavors", "2", "Flavors", &[], None));
        let parts = registry.into_parts();
        assert_eq!(parts.components.len(), 2);
        assert_eq!(parts.sidebar_items.len(), 2);

exec
/bin/zsh -lc "sed -n '1,260p' src/main.rs" in /Users/jay.ahn/projects/infra/nexttui/.worktrees/runtime-context-switch
 succeeded in 0ms:
use std::io;
use std::path::PathBuf;
use std::sync::Arc;

use crossterm::{
    execute,
    terminal::{EnterAlternateScreen, LeaveAlternateScreen, disable_raw_mode, enable_raw_mode},
};
use ratatui::Terminal;
use ratatui::backend::CrosstermBackend;
use tokio::sync::mpsc;
use tracing_subscriber::EnvFilter;

use nexttui::adapter::auth::keystone::KeystoneAuthAdapter;
use nexttui::adapter::registry::AdapterRegistry;
use nexttui::app::App;
use nexttui::config::Config;
use nexttui::demo::create_demo_app;
use nexttui::event::AppEvent;
use nexttui::event_loop::run_event_loop;
use nexttui::port::auth::AuthProvider;
use nexttui::port::types::{AuthCredential, AuthMethod, ProjectScopeParam};
use nexttui::worker::run_worker;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Initialize tracing (file-based, since TUI owns stdout/stderr)
    let log_dir = dirs::cache_dir()
        .unwrap_or_else(|| PathBuf::from("/tmp"))
        .join("nexttui");
    let file_appender = tracing_appender::rolling::daily(&log_dir, "nexttui.log");
    let (non_blocking, _log_guard) = tracing_appender::non_blocking(file_appender);
    tracing_subscriber::fmt()
        .with_env_filter(
            EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| EnvFilter::new("nexttui=info")),
        )
        .with_writer(non_blocking)
        .with_ansi(false)
        .init();

    let args: Vec<String> = std::env::args().collect();
    let demo_mode = args.iter().any(|a| a == "--demo");
    let cloud_arg = args.windows(2)
        .find(|w| w[0] == "--cloud")
        .map(|w| w[1].clone());

    // Keep _event_tx alive so event_rx doesn't immediately return None in demo mode
    let (mut app, event_rx, _keep_alive_tx) = if demo_mode {
        let (app, _action_rx) = create_demo_app()?;
        let (event_tx, event_rx) = mpsc::unbounded_channel::<AppEvent>();
        (app, event_rx, Some(event_tx))
    } else {
        let mut config = match Config::load() {
            Ok(c) => c,
            Err(e) => {
                eprintln!("Error: {e}");
                tracing::error!(%e, "failed to load config");
                std::process::exit(1);
            }
        };

        // --cloud CLI arg overrides OS_CLOUD and config.toml default_cloud
        if let Some(ref name) = cloud_arg {
            if let Err(e) = config.switch_cloud(name) {
                eprintln!("Error: {e}");
                std::process::exit(1);
            }
        }

        for w in config.warnings() {
            eprintln!("Warning: {w}");
            tracing::warn!(warning = %w, "config warning");
        }

        let (action_tx, action_rx) = mpsc::unbounded_channel();
        let (event_tx, event_rx) = mpsc::unbounded_channel::<AppEvent>();

        // Build auth credential from config
        let cloud = config.active_cloud_config();
        let credential = AuthCredential {
            auth_url: cloud.auth.auth_url.clone(),
            method: AuthMethod::Password {
                username: cloud.auth.username.clone().unwrap_or_default(),
                password: cloud.auth.password.clone().unwrap_or_default(),
                domain_name: cloud
                    .auth
                    .user_domain_name
                    .clone()
                    .unwrap_or_else(|| "Default".to_string()),
            },
            project_scope: cloud.auth.project_name.as_ref().map(|pn| ProjectScopeParam {
                name: pn.clone(),
                domain_name: cloud
                    .auth
                    .project_domain_name
                    .clone()
                    .unwrap_or_else(|| "Default".to_string()),
            }),
        };

        let auth_provider = Arc::new(KeystoneAuthAdapter::new(credential)?);
        let registry = Arc::new(AdapterRegistry::new_http(
            auth_provider.clone(),
            cloud.region_name.clone(),
        )?);

        // Trigger initial authentication, then initialize RBAC from token roles
        let rbac = std::sync::Arc::new(nexttui::infra::rbac::RbacGuard::new());
        let _ = auth_provider.get_token().await; // force auth before reading roles
        if let Ok(token) = auth_provider.get_token_info().await {
            rbac.update_roles(token.roles, Some(token.project.id));
        }
        let mut module_registry = nexttui::registry::ModuleRegistry::new();
        nexttui::registry::register_all_modules(&mut module_registry, &action_tx);
        let (app, initial_actions) = App::from_registry(config, action_tx.clone(), module_registry, rbac.clone());

        // Spawn background worker
        tokio::spawn(run_worker(registry, rbac, app.all_tenants.clone(), action_rx, event_tx));

        // Trigger initial data load
        for action in initial_actions {
            let _ = action_tx.send(action);
        }

        (app, event_rx, None)
    };

    // Restore terminal on panic before raw mode corrupts output
    let original_hook = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |info| {
        let _ = disable_raw_mode();
        let _ = execute!(io::stdout(), LeaveAlternateScreen);
        original_hook(info);
    }));

    // Setup terminal
    enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen)?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;

    let result = run_event_loop(&mut terminal, &mut app, event_rx).await;

    // Cleanup terminal
    disable_raw_mode()?;
    execute!(terminal.backend_mut(), LeaveAlternateScreen)?;
    terminal.show_cursor()?;

    if let Err(e) = result {
        eprintln!("Error: {e}");
        tracing::error!(%e, "event loop error");
        std::process::exit(1);
    }

    Ok(())
}

exec
/bin/zsh -lc "rg -n \"switch_cloud|active_cloud|project_name|default_cloud|cloud\" src/config.rs src/config -g '*.rs'" in /Users/jay.ahn/projects/infra/nexttui/.worktrees/runtime-context-switch
 exited 2 in 0ms:
rg: src/config: No such file or directory (os error 2)
src/config.rs:20:    clouds: HashMap<String, CloudConfig>,
src/config.rs:21:    active_cloud: String,
src/config.rs:60:    pub project_name: Option<String>,
src/config.rs:75:            .field("project_name", &self.project_name)
src/config.rs:102:    pub default_cloud: Option<String>,
src/config.rs:137:            default_cloud: None,
src/config.rs:185:// Internal: clouds.yaml root structure
src/config.rs:188:    clouds: Option<HashMap<String, CloudConfig>>,
src/config.rs:192:    /// Load config from clouds.yaml + optional app config.
src/config.rs:193:    /// `override_clouds_path` is for testing — bypasses standard search paths.
src/config.rs:194:    pub fn load_from(clouds_yaml_path: &Path) -> Result<Self> {
src/config.rs:196:            std::fs::read_to_string(clouds_yaml_path).map_err(|e| AppError::ConfigParse {
src/config.rs:197:                path: clouds_yaml_path.to_path_buf(),
src/config.rs:203:                path: clouds_yaml_path.to_path_buf(),
src/config.rs:207:        let raw_clouds = root.clouds.ok_or_else(|| AppError::ConfigValidation {
src/config.rs:208:            message: "Invalid clouds.yaml: missing 'clouds' key".to_string(),
src/config.rs:211:        // Validate each cloud, skip invalid ones
src/config.rs:212:        let mut clouds = HashMap::new();
src/config.rs:215:        for (name, mut cloud) in raw_clouds {
src/config.rs:216:            cloud.name = name.clone();
src/config.rs:217:            match validate_cloud(&name, &cloud) {
src/config.rs:220:                    clouds.insert(name, cloud);
src/config.rs:228:        if clouds.is_empty() {
src/config.rs:230:                message: "No valid cloud configurations found in clouds.yaml".to_string(),
src/config.rs:234:        // Determine active cloud (OS_CLOUD env var takes precedence)
src/config.rs:235:        let os_cloud = std::env::var("OS_CLOUD").ok();
src/config.rs:236:        let active_cloud = Self::determine_active_cloud(&clouds, os_cloud.as_deref())?;
src/config.rs:242:            clouds,
src/config.rs:243:            active_cloud,
src/config.rs:251:        let path = Self::find_clouds_yaml()?;
src/config.rs:254:        // Override active cloud: config.toml default_cloud > $OS_CLOUD
src/config.rs:255:        let default_cloud = config.app.default_cloud.clone();
src/config.rs:256:        let env_cloud = std::env::var("OS_CLOUD").ok();
src/config.rs:257:        let preferred = default_cloud.or(env_cloud);
src/config.rs:259:            if !config.clouds.contains_key(name) {
src/config.rs:262:                    available: config.clouds.keys().cloned().collect(),
src/config.rs:265:            config.active_cloud = name.clone();
src/config.rs:271:    /// Get warnings generated during config loading (e.g., skipped clouds)
src/config.rs:276:    fn find_clouds_yaml() -> Result<PathBuf> {
src/config.rs:288:        // 2. ./clouds.yaml
src/config.rs:289:        let local = PathBuf::from("./clouds.yaml");
src/config.rs:297:            let path = config_dir.join("openstack/clouds.yaml");
src/config.rs:304:        // 4. XDG fallback: ~/.config/openstack/clouds.yaml (explicit, for macOS compatibility)
src/config.rs:306:            let path = home.join(".config/openstack/clouds.yaml");
src/config.rs:313:        // 5. /etc/openstack/clouds.yaml
src/config.rs:314:        let etc = PathBuf::from("/etc/openstack/clouds.yaml");
src/config.rs:325:    fn determine_active_cloud(
src/config.rs:326:        clouds: &HashMap<String, CloudConfig>,
src/config.rs:330:            if clouds.contains_key(name) {
src/config.rs:335:                available: clouds.keys().cloned().collect(),
src/config.rs:339:        let mut keys: Vec<&String> = clouds.keys().collect();
src/config.rs:360:    pub fn active_cloud_config(&self) -> &CloudConfig {
src/config.rs:361:        &self.clouds[&self.active_cloud]
src/config.rs:364:    pub fn cloud_config(&self, name: &str) -> Option<&CloudConfig> {
src/config.rs:365:        self.clouds.get(name)
src/config.rs:368:    pub fn cloud_names(&self) -> Vec<&str> {
src/config.rs:369:        let mut names: Vec<&str> = self.clouds.keys().map(|s| s.as_str()).collect();
src/config.rs:374:    pub fn active_cloud_name(&self) -> &str {
src/config.rs:375:        &self.active_cloud
src/config.rs:378:    pub fn switch_cloud(&mut self, name: &str) -> Result<()> {
src/config.rs:379:        if !self.clouds.contains_key(name) {
src/config.rs:382:                available: self.clouds.keys().cloned().collect(),
src/config.rs:385:        self.active_cloud = name.to_string();
src/config.rs:428:fn validate_cloud(
src/config.rs:430:    cloud: &CloudConfig,
src/config.rs:434:    if cloud.auth.auth_url.is_empty() {
src/config.rs:438:    let auth_type = detect_auth_type(&cloud.auth);
src/config.rs:441:            if cloud.auth.username.is_none() || cloud.auth.password.is_none() {
src/config.rs:448:            if cloud.auth.application_credential_id.is_none()
src/config.rs:449:                || cloud.auth.application_credential_secret.is_none()
src/config.rs:458:    if let Some(ref cacert) = cloud.cacert
src/config.rs:467:    if !cloud.verify {
src/config.rs:481:    fn write_clouds_yaml(dir: &TempDir, content: &str) -> PathBuf {
src/config.rs:482:        let path = dir.path().join("clouds.yaml");
src/config.rs:487:    fn valid_clouds_yaml() -> &'static str {
src/config.rs:489:clouds:
src/config.rs:495:      project_name: admin
src/config.rs:509:    fn test_load_clouds_yaml_from_standard_path() {
src/config.rs:511:        let path = write_clouds_yaml(&dir, valid_clouds_yaml());
src/config.rs:513:        assert_eq!(config.clouds.len(), 2);
src/config.rs:514:        assert!(config.clouds.contains_key("devstack"));
src/config.rs:515:        assert!(config.clouds.contains_key("production"));
src/config.rs:519:    fn test_clouds_yaml_not_found() {
src/config.rs:520:        let result = Config::load_from(Path::new("/nonexistent/clouds.yaml"));
src/config.rs:527:    fn test_missing_clouds_key() {
src/config.rs:529:        let path = write_clouds_yaml(&dir, "something_else:\n  key: value\n");
src/config.rs:535:                assert!(message.contains("missing 'clouds' key"));
src/config.rs:548:            project_name: None,
src/config.rs:564:            project_name: None,
src/config.rs:577:clouds:
src/config.rs:588:        let path = write_clouds_yaml(&dir, yaml);
src/config.rs:590:        assert_eq!(config.clouds.len(), 1);
src/config.rs:591:        assert!(config.clouds.contains_key("good"));
src/config.rs:592:        assert!(!config.clouds.contains_key("bad"));
src/config.rs:599:clouds:
src/config.rs:610:        let path = write_clouds_yaml(&dir, yaml);
src/config.rs:612:        assert_eq!(config.clouds.len(), 1);
src/config.rs:613:        assert!(config.clouds.contains_key("good"));
src/config.rs:617:    fn test_all_clouds_invalid_fatal() {
src/config.rs:620:clouds:
src/config.rs:629:        let path = write_clouds_yaml(&dir, yaml);
src/config.rs:634:                assert!(message.contains("No valid cloud"));
src/config.rs:641:    fn test_partial_invalid_clouds_skip() {
src/config.rs:644:clouds:
src/config.rs:654:        let path = write_clouds_yaml(&dir, yaml);
src/config.rs:656:        assert_eq!(config.clouds.len(), 1);
src/config.rs:657:        assert!(config.clouds.contains_key("good"));
src/config.rs:661:    fn test_active_cloud_fallback_to_first() {
src/config.rs:663:        let path = write_clouds_yaml(&dir, valid_clouds_yaml());
src/config.rs:666:        assert_eq!(config.active_cloud, "devstack");
src/config.rs:670:    fn test_active_cloud_not_found() {
src/config.rs:671:        let clouds = HashMap::from([(
src/config.rs:680:                    project_name: None,
src/config.rs:694:        let result = Config::determine_active_cloud(&clouds, Some("nonexistent"));
src/config.rs:703:    fn test_switch_cloud_success() {
src/config.rs:705:        let path = write_clouds_yaml(&dir, valid_clouds_yaml());
src/config.rs:707:        assert_eq!(config.active_cloud, "devstack");
src/config.rs:708:        config.switch_cloud("production").unwrap();
src/config.rs:709:        assert_eq!(config.active_cloud, "production");
src/config.rs:713:    fn test_switch_cloud_not_found() {
src/config.rs:715:        let path = write_clouds_yaml(&dir, valid_clouds_yaml());
src/config.rs:717:        let result = config.switch_cloud("nonexistent");
src/config.rs:721:    fn make_cloud_config(name: &str) -> CloudConfig {
src/config.rs:729:                project_name: None,
src/config.rs:745:    fn test_os_cloud_preferred_selects_cloud() {
src/config.rs:746:        let clouds = HashMap::from([
src/config.rs:747:            ("devstack".to_string(), make_cloud_config("devstack")),
src/config.rs:748:            ("production".to_string(), make_cloud_config("production")),
src/config.rs:751:        let result = Config::determine_active_cloud(&clouds, Some("production")).unwrap();
src/config.rs:762:            project_name: None,
src/config.rs:781:            project_name: None,
src/config.rs:843:        let path = write_clouds_yaml(&dir, valid_clouds_yaml());

exec
/bin/zsh -lc "sed -n '1,280p' src/event_loop.rs" in /Users/jay.ahn/projects/infra/nexttui/.worktrees/runtime-context-switch
exec
/bin/zsh -lc "rg -n \"SwitchCloud|CloudSwitched|Context|switch-back|switch-project|ctx|TokenRefreshed|RefreshAll\" src/app.rs src/event_loop.rs src/worker.rs src/input src/module -g '*.rs'" in /Users/jay.ahn/projects/infra/nexttui/.worktrees/runtime-context-switch
 succeeded in 0ms:
use std::io::Stdout;
use std::time::Duration;

use crossterm::event::{Event, EventStream};
use futures::StreamExt;
use ratatui::Terminal;
use ratatui::backend::CrosstermBackend;
use tokio::sync::mpsc;

use crate::app::App;
use crate::error::Result;
use crate::event::AppEvent;

/// Main event loop — runs until App.should_quit becomes true.
pub async fn run_event_loop(
    terminal: &mut Terminal<CrosstermBackend<Stdout>>,
    app: &mut App,
    mut event_rx: mpsc::UnboundedReceiver<AppEvent>,
) -> Result<()> {
    let mut key_events = EventStream::new();
    let mut tick = tokio::time::interval(Duration::from_millis(200));

    loop {
        tokio::select! {
            // Branch 1: key input
            key_result = key_events.next() => {
                match key_result {
                    Some(Ok(Event::Key(key))) if key.kind == crossterm::event::KeyEventKind::Press => {
                        app.handle_key(key);
                    }
                    Some(Ok(Event::Resize(_, _))) => {
                        // Resize triggers immediate re-render (handled below)
                    }
                    Some(Ok(_)) => {}
                    Some(Err(_)) | None => {
                        // Key stream ended or errored — exit gracefully
                        app.should_quit = true;
                    }
                }
            }

            // Branch 2: tick timer
            _ = tick.tick() => {
                app.on_tick();
            }

            // Branch 3: background events
            event = event_rx.recv() => {
                match event {
                    Some(ev) => {
                        app.handle_event(ev);
                    }
                    None => {
                        // All event senders dropped — exit gracefully
                        app.should_quit = true;
                    }
                }
            }
        }

        // Render
        terminal.draw(|f| app.render(f))?;

        // Check quit
        if app.should_quit {
            break;
        }
    }

    Ok(())
}

 succeeded in 0ms:
src/worker.rs:722:        Action::RefreshAll => {
src/worker.rs:723:            // RefreshAll is not handled by the worker — App::dispatch_action should
src/worker.rs:728:        Action::SwitchCloud(_cloud_name) => {
src/app.rs:19:use crate::ui::header::{Header, HeaderContext};
src/app.rs:424:        if let AppEvent::TokenRefreshed(ref roles) = event {
src/app.rs:764:        self.header.render(frame, areas.header, &HeaderContext {
src/app.rs:1082:        app.handle_event(AppEvent::TokenRefreshed(roles));
src/input/command.rs:16:    ContextSwitch(String),
src/input/command.rs:17:    ContextList,
src/input/command.rs:107:            "ctx" => {
src/input/command.rs:109:                    Some(cloud) if !cloud.is_empty() => Command::ContextSwitch(cloud),
src/input/command.rs:110:                    _ => Command::ContextList,
src/input/command.rs:187:        cmds.extend(["quit", "refresh", "help", "ctx"].iter().map(|s| s.to_string()));
src/input/command.rs:331:            p.parse("ctx prod"),
src/input/command.rs:332:            Command::ContextSwitch("prod".to_string())
src/input/command.rs:334:        assert_eq!(p.parse("ctx"), Command::ContextList);
src/module/server/mod.rs:24:    ServerViewContext,
src/module/server/mod.rs:515:            let ctx = self.popup_kind.unwrap_or(DetailPopupKind::Resize);
src/module/server/mod.rs:520:                    match ctx {
src/module/server/mod.rs:999:                    let data = server_detail_data(&ServerViewContext {
src/module/server/view_model.rs:10:pub struct ServerViewContext<'a> {
src/module/server/view_model.rs:19:impl<'a> ServerViewContext<'a> {
src/module/server/view_model.rs:146:    server_detail_data(&ServerViewContext::default_for(server))
src/module/server/view_model.rs:149:pub fn server_detail_data(ctx: &ServerViewContext) -> DetailData {
src/module/server/view_model.rs:150:    let server = ctx.server;
src/module/server/view_model.rs:151:    let migration_progress = ctx.migration_progress;
src/module/server/view_model.rs:152:    let flavor = ctx.flavor;
src/module/server/view_model.rs:153:    let is_resize_pending = ctx.is_resize_pending;
src/module/server/view_model.rs:154:    let cached_volumes = ctx.cached_volumes;
src/module/server/view_model.rs:155:    let cached_floating_ips = ctx.cached_floating_ips;
src/module/server/view_model.rs:686:        let data = server_detail_data(&ServerViewContext::default_for(&server));
src/module/server/view_model.rs:693:        let data = server_detail_data(&ServerViewContext::default_for(&server));
src/module/server/view_model.rs:714:        let data = server_detail_data(&ServerViewContext {
src/module/server/view_model.rs:732:        let data = server_detail_data(&ServerViewContext::default_for(&server));
src/module/server/view_model.rs:771:        let data = server_detail_data(&ServerViewContext {
src/module/server/view_model.rs:787:        let data = server_detail_data(&ServerViewContext::default_for(&server));
src/module/server/view_model.rs:801:        let data = server_detail_data(&ServerViewContext {
src/module/floating_ip/mod.rs:21:use self::view_model::{fip_columns, fip_create_defs, fip_to_row, FipRowContext};
src/module/floating_ip/mod.rs:92:        let ctx = FipRowContext {
src/module/floating_ip/mod.rs:97:        self.floating_ips.iter().map(|f| fip_to_row(f, &ctx)).collect()
src/module/floating_ip/view_model.rs:7:pub struct FipRowContext<'a> {
src/module/floating_ip/view_model.rs:13:impl<'a> FipRowContext<'a> {
src/module/floating_ip/view_model.rs:61:    fip_to_row(fip, &FipRowContext::default_for(show_tenant))
src/module/floating_ip/view_model.rs:64:pub fn fip_to_row(fip: &FloatingIp, ctx: &FipRowContext) -> Row {
src/module/floating_ip/view_model.rs:65:    let show_tenant = ctx.show_tenant;
src/module/floating_ip/view_model.rs:66:    let cached_servers = ctx.cached_servers;
src/module/floating_ip/view_model.rs:67:    let cached_ports = ctx.cached_ports;

exec
/bin/zsh -lc "sed -n '220,560p' src/app.rs" in /Users/jay.ahn/projects/infra/nexttui/.worktrees/runtime-context-switch
 succeeded in 0ms:
                .map_or(false, |c| c.is_modal());
            if is_modal {
                if let Some(component) = self.components.get_mut(&self.router.current()) {
                    if let Some(action) = component.handle_key(key) {
                        self.dispatch_action(action);
                    }
                }
                return true;
            }
        }

        // Global keys in Normal mode (only without modifiers to avoid Ctrl+q etc.)
        if self.input_mode == InputMode::Normal && no_modifiers {
            match key.code {
                KeyCode::Char(':') => {
                    self.input_mode = InputMode::Command;
                    return true;
                }
                // '/' search is handled by SelectPopup when open (not App-level)
                // KeyCode::Char('/') — disabled: App-level search mode is unimplemented
                KeyCode::Tab => {
                    // FullWidth module: Tab restores sidebar and returns to previous route
                    let full_width = self.components.get(&self.router.current())
                        .map_or(false, |c| c.layout_hint() == LayoutHint::FullWidth);
                    if full_width {
                        // Block exit while module is busy (e.g. evacuating)
                        let busy = self.components.get(&self.router.current())
                            .map_or(false, |c| c.is_busy());
                        if busy { return true; }
                        self.sidebar_visible = true;
                        self.layout.set_sidebar_visible(true);
                        self.router.back();
                        self.sidebar.sync_active(&self.router.current(), self.rbac.is_admin());
                        self.focus = FocusPane::Sidebar;
                    } else if self.sidebar_visible {
                        self.focus = match self.focus {
                            FocusPane::Content => FocusPane::Sidebar,
                            FocusPane::Sidebar => FocusPane::Content,
                        };
                    }
                    return true;
                }
                KeyCode::Char('q') => {
                    self.should_quit = true;
                    return true;
                }
                KeyCode::Char(c @ '1'..='9') | KeyCode::Char(c @ '0') | KeyCode::Char(c @ 'h') => {
                    // Block route switching while current module is busy (e.g. evacuating)
                    let busy = self.components.get(&self.router.current())
                        .map_or(false, |comp| comp.is_busy());
                    if busy { return true; }

                    if c == 'h' {
                        // 'h' shortcut for Host Ops
                        if self.rbac.is_admin() {
                            self.dispatch_action(Action::Navigate(Route::Hosts));
                        }
                    } else {
                        let idx = if c == '0' { 9 } else { (c as usize) - ('1' as usize) };
                        if let Some(route) = self.sidebar.route_at(idx, self.rbac.is_admin()) {
                            self.dispatch_action(Action::Navigate(route));
                        }
                    }
                    return true;
                }
                KeyCode::Esc => {
                    if self.focus == FocusPane::Sidebar {
                        self.focus = FocusPane::Content;
                        return true;
                    }
                    // Fall through to let component handle Esc
                    // (Detail→List transition, or return Action::Back for router)
                }
                _ => {}
            }
        }

        // Esc from Command/Search/Confirm → Normal
        if matches!(
            self.input_mode,
            InputMode::Command | InputMode::Search | InputMode::Confirm
        ) && key.code == KeyCode::Esc
        {
            self.input_mode = InputMode::Normal;
            return true;
        }

        // Form mode: delegate all keys to the active component (FormWidget handles everything)
        if self.input_mode == InputMode::Form {
            if let Some(component) = self.components.get_mut(&self.router.current()) {
                if let Some(action) = component.handle_key(key) {
                    self.dispatch_action(action);
                }
            }
            return true;
        }

        // Delegate based on focus pane
        if self.input_mode == InputMode::Normal {
            if self.focus == FocusPane::Sidebar && self.sidebar_visible {
                if let Some(action) = self.sidebar.handle_key(key, self.rbac.is_admin()) {
                    self.dispatch_action(action);
                }
                return true;
            }

            if let Some(component) = self.components.get_mut(&self.router.current()) {
                if let Some(action) = component.handle_key(key) {
                    self.dispatch_action(action);
                }
                return true;
            }

            // Fallback: Esc with no component registered → router back
            if key.code == KeyCode::Esc {
                self.router.back();
                return true;
            }
        }

        true
    }

    /// Handle action — intercept navigation actions, forward the rest to action_tx.
    fn dispatch_action(&mut self, action: Action) {
        match action {
            Action::Navigate(route) => {
                self.router.navigate(route);
                self.sidebar.sync_active(&self.router.current(), self.rbac.is_admin());
                self.focus = FocusPane::Content;
                // LayoutHint::FullWidth modules hide the sidebar
                let full_width = self.components.get(&self.router.current())
                    .map_or(false, |c| c.layout_hint() == LayoutHint::FullWidth);
                if full_width && self.sidebar_visible {
                    self.sidebar_visible = false;
                } else if !full_width && !self.sidebar_visible {
                    self.sidebar_visible = true;
                }
                self.layout.set_sidebar_visible(self.sidebar_visible);
                self.refresh_scheduler.reset();
            }
            Action::Back => {
                self.router.back();
                // Restore sidebar if leaving a FullWidth module
                let full_width = self.components.get(&self.router.current())
                    .map_or(false, |c| c.layout_hint() == LayoutHint::FullWidth);
                if !full_width && !self.sidebar_visible {
                    self.sidebar_visible = true;
                    self.layout.set_sidebar_visible(true);
                }
                self.sidebar.sync_active(&self.router.current(), self.rbac.is_admin());
                self.refresh_scheduler.reset();
            }
            Action::FocusSidebar => {
                if self.sidebar_visible {
                    self.focus = FocusPane::Sidebar;
                }
            }
            Action::EnterFormMode => {
                self.input_mode = InputMode::Form;
            }
            Action::ExitFormMode => {
                self.input_mode = InputMode::Normal;
            }
            Action::ToggleAllTenants => {
                let prev = self.all_tenants.load(Ordering::Relaxed);
                self.all_tenants.store(!prev, Ordering::Relaxed);
                // Broadcast to modules
                for component in self.components.values_mut() {
                    component.set_all_tenants(!prev);
                }
                // Re-fetch all resources with new filter
                let fetches = [
                    Action::FetchServers,
                    Action::FetchNetworks,
                    Action::FetchSecurityGroups,
                    Action::FetchFloatingIps,
                    Action::FetchVolumes,
                    Action::FetchSnapshots,
                    Action::FetchImages,
                ];
                for a in fetches {
                    let _ = self.action_tx.send(a);
                }
            }
            Action::ShowToast { message } => {
                self.background_tracker.add_toast(message, crate::background::ToastLevel::Info);
            }
            Action::Quit => {
                self.should_quit = true;
            }
            other => {
                if let Some(msg) = Self::progress_toast_text(&other) {
                    self.background_tracker.add_toast(msg, crate::background::ToastLevel::Info);
                }
                let _ = self.action_tx.send(other);
            }
        }
    }

    /// Handle background event — broadcast to all registered components and generate toasts.
    /// Events like ServersLoaded must reach ServerModule even if the user is on a different view.
    pub fn handle_event(&mut self, event: AppEvent) {
        // RBAC: update roles on token refresh
        if let AppEvent::TokenRefreshed(ref roles) = event {
            self.rbac.update_roles(roles.clone(), None);
            self.broadcast_admin();
        }
        // Migration complete → refresh server list to reflect status change
        let refresh_servers = matches!(
            event,
            AppEvent::MigrationPollingStopped { .. }
            | AppEvent::ServerStatusPolled { .. }
        ) || matches!(
            event,
            AppEvent::ServerLiveMigrated { .. }
            | AppEvent::ServerColdMigrated { .. }
            | AppEvent::MigrationConfirmed { .. }
            | AppEvent::MigrationReverted { .. }
            | AppEvent::ServerEvacuated { .. }
            | AppEvent::ServerEvacuateResult { .. }
            | AppEvent::ServerResized { .. }
            | AppEvent::ResizeConfirmed { .. }
            | AppEvent::ResizeReverted { .. }
        );
        // API backoff: slow down refresh on rate-limit/unavailable errors.
        // NOTE: matches ApiError::RateLimited / ServiceUnavailable Display strings.
        // If those Display impls change, update these patterns (or add a typed field to AppEvent).
        match &event {
            AppEvent::ApiError { message, .. }
                if message.contains("Rate limited") || message.contains("unavailable") =>
            {
                self.refresh_scheduler.backoff();
            }
            AppEvent::ApiError { .. } => {}
            _ => {
                self.refresh_scheduler.reset_backoff();
            }
        }

        self.generate_toast(&event);
        self.record_audit(&event);
        for component in self.components.values_mut() {
            component.handle_event(&event);
        }
        if refresh_servers {
            let _ = self.action_tx.send(Action::FetchServers);
        }
    }

    fn progress_toast_text(action: &Action) -> Option<String> {
        match action {
            Action::CreateServer(_) => Some("Creating server...".into()),
            Action::DeleteServer { name, .. } => Some(format!("Deleting server '{name}'...")),
            Action::RebootServer { .. } => Some("Rebooting server...".into()),
            Action::StartServer { .. } => Some("Starting server...".into()),
            Action::StopServer { .. } => Some("Stopping server...".into()),
            Action::CreateServerSnapshot { .. } => Some("Creating snapshot...".into()),
            Action::CreateFlavor(_) => Some("Creating flavor...".into()),
            Action::DeleteFlavor { .. } => Some("Deleting flavor...".into()),
            Action::CreateNetwork(_) => Some("Creating network...".into()),
            Action::CreateSecurityGroup(_) => Some("Creating security group...".into()),
            Action::DeleteSecurityGroup { .. } => Some("Deleting security group...".into()),
            Action::CreateSecurityGroupRule(_) => Some("Creating rule...".into()),
            Action::DeleteSecurityGroupRule { .. } => Some("Deleting rule...".into()),
            Action::CreateFloatingIp { .. } => Some("Creating floating IP...".into()),
            Action::DeleteFloatingIp { .. } => Some("Deleting floating IP...".into()),
            Action::CreateVolume(_) => Some("Creating volume...".into()),
            Action::DeleteVolume { .. } => Some("Deleting volume...".into()),
            Action::ExtendVolume { .. } => Some("Extending volume...".into()),
            Action::CreateSnapshot(_) => Some("Creating snapshot...".into()),
            Action::DeleteSnapshot { .. } => Some("Deleting snapshot...".into()),
            Action::CreateImage(_) => Some("Creating image...".into()),
            Action::DeleteImage { .. } => Some("Deleting image...".into()),
            Action::CreateProject(_) => Some("Creating project...".into()),
            Action::DeleteProject { .. } => Some("Deleting project...".into()),
            Action::CreateUser(_) => Some("Creating user...".into()),
            Action::DeleteUser { .. } => Some("Deleting user...".into()),
            Action::LiveMigrateServer { .. } => Some("Live migrating server...".into()),
            Action::ColdMigrateServer { .. } => Some("Cold migrating server...".into()),
            Action::ConfirmMigration { .. } => Some("Confirming migration...".into()),
            Action::RevertMigration { .. } => Some("Reverting migration...".into()),
            Action::EvacuateServer { .. } => Some("Evacuating server...".into()),
            Action::ResizeServer { .. } => Some("Resizing server...".into()),
            Action::ConfirmResize { .. } => Some("Confirming resize...".into()),
            Action::RevertResize { .. } => Some("Reverting resize...".into()),
            _ => None,
        }
    }

    /// Initialize audit logger. Returns None on failure (non-fatal).
    fn init_audit_logger() -> Option<AuditLogger> {
        #[cfg(test)]
        {
            // In tests, do not create audit logger by default
            None
        }
        #[cfg(not(test))]
        {
            let path = crate::config::nexttui_config_dir().join("audit.log");
            match AuditLogger::new(path) {
                Ok(logger) => Some(logger),
                Err(e) => {
                    tracing::warn!("Failed to initialize audit logger: {e}");
                    None
                }
            }
        }
    }

    /// Record a CUD event to the audit log. Errors are logged as warnings, never propagated.
    fn record_audit(&self, event: &AppEvent) {
        let Some(ref logger) = self.audit_logger else {
            return;
        };
        let Some(entry) = self.build_audit_entry(event) else {
            return;
        };
        if let Err(e) = logger.log_entry(entry) {
            tracing::warn!("Failed to write audit log: {e}");
        }
        if let Err(e) = logger.rotate_if_needed() {
            tracing::warn!("Failed to rotate audit log: {e}");
        }
    }

    /// Map an AppEvent to an AuditEntry. Returns None for non-auditable events.
    fn build_audit_entry(&self, event: &AppEvent) -> Option<AuditEntry> {
        let cloud = self.config.active_cloud_name().to_string();
        let user = self
            .config
            .active_cloud_config()
            .auth
            .username
            .clone()
            .unwrap_or_else(|| "unknown".to_string());
        let project = self.rbac.project_id();
        let timestamp = chrono::Local::now().to_rfc3339();

        let (action, resource_type, resource_id, resource_name, result) = match event {
            // Server CUD

exec
/bin/zsh -lc "sed -n '660,780p' src/worker.rs" in /Users/jay.ahn/projects/infra/nexttui/.worktrees/runtime-context-switch
 succeeded in 0ms:

        // -- Nova: Volume Attach/Detach (via Nova os-volume_attachments API) --
        Action::AttachVolume { volume_id, server_id, device } => {
            match registry.nova.attach_volume(&server_id, &volume_id, device.as_deref()).await {
                Ok(()) => Some(AppEvent::VolumeAttached { volume_id, server_id }),
                Err(e) => Some(api_error("AttachVolume", e)),
            }
        }
        Action::DetachVolume { volume_id, server_id, .. } => {
            match registry.nova.detach_volume(&server_id, &volume_id).await {
                Ok(()) => Some(AppEvent::VolumeDetached { volume_id }),
                Err(e) => Some(api_error("DetachVolume", e)),
            }
        }
        Action::ForceDetachVolume { volume_id, attachment_id, .. } => {
            match registry.cinder.force_detach_volume(&volume_id, &attachment_id).await {
                Ok(()) => Some(AppEvent::VolumeForceDetached { volume_id }),
                Err(e) => Some(api_error("ForceDetachVolume", e)),
            }
        }
        Action::ForceResetVolumeState { volume_id, target_state } => {
            match registry.cinder.force_set_volume_state(&volume_id, &target_state).await {
                Ok(()) => Some(AppEvent::VolumeStateReset { volume_id }),
                Err(e) => Some(api_error("ForceResetVolumeState", e)),
            }
        }

        // -- Neutron: Floating IP Associate/Disassociate --------------------
        Action::AssociateFloatingIp { fip_id, port_id } => {
            match registry.neutron.associate_floating_ip(&fip_id, &port_id).await {
                Ok(fip) => Some(AppEvent::FloatingIpAssociated(fip)),
                Err(e) => Some(api_error("AssociateFloatingIp", e)),
            }
        }
        Action::DisassociateFloatingIp { fip_id } => {
            match registry.neutron.disassociate_floating_ip(&fip_id).await {
                Ok(fip) => Some(AppEvent::FloatingIpDisassociated(fip)),
                Err(e) => Some(api_error("DisassociateFloatingIp", e)),
            }
        }

        // -- Neutron: Ports -------------------------------------------------
        Action::FetchPorts { server_id } => {
            match registry.neutron.list_ports(&server_id).await {
                Ok(ports) => Some(AppEvent::PortsLoaded { server_id, ports }),
                Err(e) => Some(api_error("FetchPorts", e)),
            }
        }

        // -- UI-only actions (handled by App::dispatch_action, not worker) --
        Action::Navigate(_)
        | Action::Back
        | Action::FocusSidebar
        | Action::SelectResource { .. }
        | Action::NavigateToResource { .. }
        | Action::EnterFormMode
        | Action::ExitFormMode
        | Action::ToggleAllTenants
        | Action::ShowToast { .. }
        | Action::Quit => None,

        // -- System ---------------------------------------------------------
        Action::RefreshAll => {
            // RefreshAll is not handled by the worker — App::dispatch_action should
            // expand it into individual Fetch actions. If it reaches here, ignore.
            None
        }

        Action::SwitchCloud(_cloud_name) => {
            // Phase 2: switch auth provider and re-create adapters
            None
        }
    }
}

/// Determine if an action should trigger migration-progress polling after success.
fn poll_migration_server_id(action: &Action) -> Option<String> {
    match action {
        Action::LiveMigrateServer { id, .. }
        | Action::ColdMigrateServer { id, .. } => Some(id.clone()),
        _ => None,
    }
}

/// Determine if an action should trigger server-status polling after success.
fn poll_server_id_for_status(action: &Action) -> Option<String> {
    match action {
        Action::ResizeServer { id, .. }
        | Action::ConfirmResize { id }
        | Action::RevertResize { id }
        | Action::RebootServer { id, .. }
        | Action::StartServer { id }
        | Action::StopServer { id } => Some(id.clone()),
        _ => None,
    }
}

/// Return a dedup key for Fetch-type actions (parameterless list fetches).
/// Returns None for mutations, parameterized fetches, and non-fetch actions.
fn fetch_dedup_key(action: &Action) -> Option<&'static str> {
    match action {
        Action::FetchServers => Some("FetchServers"),
        Action::FetchVolumes => Some("FetchVolumes"),
        Action::FetchNetworks => Some("FetchNetworks"),
        Action::FetchImages => Some("FetchImages"),
        Action::FetchFlavors => Some("FetchFlavors"),
        Action::FetchSnapshots => Some("FetchSnapshots"),
        Action::FetchFloatingIps => Some("FetchFloatingIps"),
        Action::FetchSecurityGroups => Some("FetchSecurityGroups"),
        Action::FetchProjects => Some("FetchProjects"),
        Action::FetchUsers => Some("FetchUsers"),
        Action::FetchAggregates => Some("FetchAggregates"),
        Action::FetchComputeServices => Some("FetchComputeServices"),
        Action::FetchHypervisors => Some("FetchHypervisors"),
        Action::FetchAgents => Some("FetchAgents"),
        _ => None,
    }
}

use crate::models::common::is_terminal_server_status;


exec
/bin/zsh -lc "nl -ba devflow-docs/inception/application-design.md | sed -n '1,220p'" in /Users/jay.ahn/projects/infra/nexttui/.worktrees/runtime-context-switch
 succeeded in 0ms:
     1	# Application Design
     2	
     3	**Mode**: LIST (목록 단계)
     4	**Timestamp**: 2026-04-13T00:00:00+09:00
     5	**BL**: BL-P2-031 Keystone Rescoping
     6	
     7	## 컴포넌트 목록
     8	
     9	### 신규 컴포넌트 (7개)
    10	
    11	| 컴포넌트 | 책임 | 타입 | PR |
    12	|---------|------|------|-----|
    13	| `ContextEpoch` | App-level epoch 카운터로 이전 컨텍스트의 stale 이벤트 격리 | Util | PR1 |
    14	| `CancellationRegistry` | 활성 폴링/장기 fetch에 대한 `CancellationToken` 등록·취소 | Service | PR1 |
    15	| `SwitchStateMachine` | `Idle → Switching → Committed | Failed` 전환 상태 및 rollback 규약 | Service | PR2 |
    16	| `ContextSwitcher` | 전환 절차(epoch++ → cancel → rescope → catalog 무효화 → commit) 오케스트레이터 | Service | PR2 |
    17	| `KeystoneRescopeAdapter` | Keystone v3 token-method scoped exchange 호출, expires_at 정본 사용 | Adapter | PR2 |
    18	| `ContextPicker` | Ctrl+P 모달, 프로젝트/클라우드 fuzzy 검색·선택 UI (`cloud • project • domain • project_id` 행 표시) | Controller | PR4 |
    19	| `ContextIndicator` | 영구 컨텍스트 표시 위젯 (cloud/project/domain), 전환 직후 강조 | Controller | PR5 |
    20	
    21	### 변경 컴포넌트 (8개)
    22	
    23	| 컴포넌트 | 변경 책임 | 타입 | PR |
    24	|---------|----------|------|-----|
    25	| `App` (src/app.rs) | 활성 cloud/project 컨텍스트 상태 관리, ContextSwitcher 통합, epoch 보유 | Service | PR2/PR3 |
    26	| `Worker` (src/worker.rs) | 모든 `tokio::spawn` 폴링/장기 fetch에 epoch 검증 + `tokio::select!` cancel branch 추가 | Service | PR1 |
    27	| `CommandRegistry` (src/input/command.rs) | `:switch-project <name|uuid|cloud/project>`, `:switch-cloud`, `:switch-back` 등록 + 충돌 disambiguation + tab 자동완성 | Controller | PR3 |
    28	| `KeyMap` (src/input/keymap.rs) | 글로벌 단축키 (예: Ctrl+P) → ContextPicker 오픈, Identity 리스트의 `s` → 컨텍스트 전환 | Controller | PR4/PR6 |
    29	| `ConfirmDialog` (src/ui/confirm.rs) | destructive 액션 confirm에 `cloud • project` fingerprint 표시, 컨텍스트 변경 직후엔 추가 확인 강제 | Controller | PR5 |
    30	| `StatusBar` (src/ui/status_bar.rs) | ContextIndicator 임베드 위치/우선순위 조정 | Controller | PR5 |
    31	| `Project Module` (src/module/project/mod.rs) | 리스트 행 `s` 액션 핸들러 (Identity 진입점), Enter는 Detail 유지 | Controller | PR6 |
    32	| `TokenCacheStore` (src/adapter/auth/token_cache.rs) | rescoped 토큰을 scope별로 저장/조회 (BL-P2-029 기반), 만료 시 재취득 경로 | Repository | PR2 |
    33	
    34	**총 15개 컴포넌트** (신규 7 + 변경 8)
    35	
    36	## PR 매핑 요약
    37	
    38	| PR | 컴포넌트 |
    39	|----|---------|
    40	| PR1 (선행 인프라) | ContextEpoch, CancellationRegistry, Worker 수정 |
    41	| PR2 (전환 코어) | SwitchStateMachine, ContextSwitcher, KeystoneRescopeAdapter, App 통합, TokenCacheStore 확장 |
    42	| PR3 (명령 UX) | CommandRegistry 확장 |
    43	| PR4 (피커 UI) | ContextPicker, KeyMap (Ctrl+P) |
    44	| PR5 (안전 가시성) | ContextIndicator, StatusBar, ConfirmDialog |
    45	| PR6 (Identity 통합) | Project Module `s` 액션, KeyMap (`s`) |
    46	
    47	## NFR 매핑 요약
    48	
    49	| NFR | 보장 컴포넌트 |
    50	|-----|--------------|
    51	| NFR-1 안전성 (atomic switch, stale 차단) | ContextEpoch + CancellationRegistry + SwitchStateMachine |
    52	| NFR-2 성능 (1초 이내) | KeystoneRescopeAdapter (네트워크 왕복 측정) |
    53	| NFR-3 테스트 커버리지 | 모든 신규 컴포넌트 unit + ContextSwitcher 통합 테스트 |
    54	| NFR-4 UX 일관성 | ContextPicker (기존 SelectPopup 재사용), CommandRegistry (기존 패턴) |
    55	| NFR-5 관측성 | ContextSwitcher의 `tracing` 이벤트 (epoch, target, 결과) |

exec
/bin/zsh -lc "nl -ba devflow-docs/inception/requirements.md | sed -n '1,220p'" in /Users/jay.ahn/projects/infra/nexttui/.worktrees/runtime-context-switch
 succeeded in 0ms:
     1	# Requirements Analysis
     2	
     3	**Depth**: Standard
     4	**Timestamp**: 2026-04-10T11:15:00+09:00
     5	
     6	## User Intent
     7	파라미터가 비대화된 view_model 함수에 ViewContext 패턴을 도입하여 구조적으로 정리한다. 기능 변경 없는 순수 리팩토링.
     8	
     9	## Scope 분석
    10	
    11	파라미터 수 기준으로 리팩토링 대상을 선별:
    12	
    13	| 모듈 | 함수 | 파라미터 | 대상 |
    14	|------|------|---------|------|
    15	| server | server_detail_data_full | 7개 (server, migration, flavor, is_resize, cached_volumes, cached_floating_ips) | ✅ |
    16	| server | server_to_row_full | 3개 (server, show_tenant, show_host) | ⚠️ 포함 |
    17	| floating_ip | fip_to_row_with_servers | 4개 (fip, show_tenant, cached_servers, cached_ports) | ⚠️ 포함 |
    18	| volume | volume_detail_data_with_servers | 2개 | ❌ 불필요 |
    19	| volume | volume_to_row_with_servers | 3개 | ❌ 불필요 |
    20	| 나머지 12개 | 1~2개 | ❌ 불필요 |
    21	
    22	## Functional Requirements
    23	
    24	### ServerViewContext
    25	- FR-01: ServerViewContext 구조체 도입 — server, migration_progress, flavor, is_resize_pending, cached_volumes, cached_floating_ips 필드
    26	- FR-02: server_detail_data_full(7 params) → server_detail_data(ctx: &ServerViewContext) 변경
    27	- FR-03: server_to_row_full(3 params)은 유지 — 파라미터가 적고 ViewContext에 넣기 부자연스러움
    28	- FR-04: server_detail_data(server) 편의 함수 유지 (빈 ViewContext로 위임)
    29	- FR-05: ServerModule의 render에서 ViewContext 생성 → 전달
    30	
    31	### FipViewContext
    32	- FR-06: FipRowContext 구조체 도입 — show_tenant, cached_servers, cached_ports 필드
    33	- FR-07: fip_to_row_with_servers(4 params) → fip_to_row(fip, ctx: &FipRowContext) 변경
    34	- FR-08: FloatingIpModule의 rows()에서 FipRowContext 생성 → 전달
    35	
    36	### 정리 — _full/_with_servers 패턴 통합
    37	- FR-09: server_detail_data / server_detail_data_full 2개 함수 → server_detail_data(ctx) 1개로 통합
    38	- FR-10: volume_detail_data / volume_detail_data_with_servers → 그대로 유지 (파라미터 2개로 충분)
    39	- FR-11: fip_to_row / fip_to_row_with_servers 2개 → fip_to_row(fip, ctx) 1개로 통합
    40	
    41	## Non-Functional Requirements
    42	- NFR-01: 기존 1108 tests 전체 통과 (리팩토링이므로 기능 회귀 0건 필수)
    43	- NFR-02: 외부 API 동작 변경 없음 — 렌더링 결과 동일
    44	- NFR-03: 테스트에서 ViewContext builder 또는 Default 사용 가능하도록 설계
    45	
    46	## Assumptions
    47	- 파라미터 3개 이하인 함수는 리팩토링 대상에서 제외
    48	- server_to_row_full은 show_tenant/show_host가 UI 설정이라 ViewContext보다는 별도 파라미터가 자연스러움
    49	- ViewContext는 모듈별 로컬 타입 (공유 trait 불필요)
    50	
    51	## Open Questions
    52	없음

exec
/bin/zsh -lc "nl -ba src/port/auth.rs | sed -n '1,220p'" in /Users/jay.ahn/projects/infra/nexttui/.worktrees/runtime-context-switch
 succeeded in 0ms:
     1	use async_trait::async_trait;
     2	use tokio::sync::broadcast;
     3	
     4	use super::error::ApiResult;
     5	use super::types::*;
     6	
     7	#[async_trait]
     8	pub trait AuthProvider: Send + Sync {
     9	    async fn authenticate(&self, credential: &AuthCredential) -> ApiResult<Token>;
    10	    async fn refresh_token(&self) -> ApiResult<Token>;
    11	    async fn get_token(&self) -> ApiResult<String>;
    12	    async fn get_token_info(&self) -> ApiResult<Token>;
    13	    async fn authenticate_request(
    14	        &self,
    15	        method: &str,
    16	        url: &str,
    17	        headers: &reqwest::header::HeaderMap,
    18	        body: Option<&[u8]>,
    19	    ) -> ApiResult<AuthHeaders>;
    20	    async fn get_endpoint(
    21	        &self,
    22	        service_type: &str,
    23	        interface: EndpointInterface,
    24	        region: Option<&str>,
    25	    ) -> ApiResult<String>;
    26	    fn subscribe_token_refresh(&self) -> broadcast::Receiver<Token>;
    27	    async fn has_role(&self, role_name: &str) -> ApiResult<bool>;
    28	    async fn get_catalog(&self) -> ApiResult<Vec<CatalogEntry>>;
    29	    async fn get_capabilities(&self) -> ApiResult<Vec<Capability>>;
    30	}

exec
/bin/zsh -lc "nl -ba src/input/command.rs | sed -n '1,220p'" in /Users/jay.ahn/projects/infra/nexttui/.worktrees/runtime-context-switch
exec
/bin/zsh -lc "nl -ba src/adapter/http/base.rs | sed -n '1,140p'" in /Users/jay.ahn/projects/infra/nexttui/.worktrees/runtime-context-switch
 succeeded in 0ms:
     1	use std::collections::HashMap;
     2	use std::fs;
     3	use std::path::PathBuf;
     4	
     5	use crate::error::Result;
     6	use crate::models::common::Route;
     7	
     8	const MAX_HISTORY_ENTRY_LEN: usize = 1024;
     9	
    10	#[derive(Debug, Clone, PartialEq)]
    11	pub enum Command {
    12	    Navigate(Route),
    13	    Quit,
    14	    Refresh,
    15	    Help,
    16	    ContextSwitch(String),
    17	    ContextList,
    18	    Unknown(String),
    19	}
    20	
    21	/// Single source of truth for command name → (abbreviation, Route) mappings.
    22	/// Both abbreviation map and route map are derived from this table.
    23	struct CommandDef {
    24	    name: &'static str,
    25	    abbreviation: &'static str,
    26	    route: Route,
    27	}
    28	
    29	const COMMAND_TABLE: &[CommandDef] = &[
    30	    CommandDef { name: "servers", abbreviation: "srv", route: Route::Servers },
    31	    CommandDef { name: "networks", abbreviation: "net", route: Route::Networks },
    32	    CommandDef { name: "volumes", abbreviation: "vol", route: Route::Volumes },
    33	    CommandDef { name: "floatingip", abbreviation: "fip", route: Route::FloatingIps },
    34	    CommandDef { name: "security-groups", abbreviation: "sec", route: Route::SecurityGroups },
    35	    CommandDef { name: "images", abbreviation: "img", route: Route::Images },
    36	    CommandDef { name: "flavors", abbreviation: "flv", route: Route::Flavors },
    37	    CommandDef { name: "projects", abbreviation: "prj", route: Route::Projects },
    38	    CommandDef { name: "users", abbreviation: "usr", route: Route::Users },
    39	    CommandDef { name: "aggregates", abbreviation: "agg", route: Route::Aggregates },
    40	    CommandDef { name: "hypervisors", abbreviation: "hyp", route: Route::Hosts },
    41	    CommandDef { name: "hosts", abbreviation: "host", route: Route::Hosts },
    42	    CommandDef { name: "migrations", abbreviation: "mig", route: Route::Migrations },
    43	    CommandDef { name: "snapshots", abbreviation: "snap", route: Route::Snapshots },
    44	    CommandDef { name: "compute-services", abbreviation: "svc", route: Route::ComputeServices },
    45	    CommandDef { name: "agents", abbreviation: "agt", route: Route::Agents },
    46	    CommandDef { name: "usage", abbreviation: "usg", route: Route::Usage },
    47	];
    48	
    49	fn build_abbreviations() -> HashMap<String, String> {
    50	    COMMAND_TABLE
    51	        .iter()
    52	        .map(|def| (def.abbreviation.to_string(), def.name.to_string()))
    53	        .collect()
    54	}
    55	
    56	fn build_route_map() -> HashMap<String, Route> {
    57	    COMMAND_TABLE
    58	        .iter()
    59	        .map(|def| (def.name.to_string(), def.route))
    60	        .collect()
    61	}
    62	
    63	pub struct CommandParser {
    64	    abbreviations: HashMap<String, String>,
    65	    route_map: HashMap<String, Route>,
    66	    history: CommandHistory,
    67	    completions: Vec<String>,
    68	    completion_index: usize,
    69	    last_prefix: Option<String>,
    70	}
    71	
    72	impl CommandParser {
    73	    pub fn new(history_path: PathBuf) -> Self {
    74	        Self {
    75	            abbreviations: build_abbreviations(),
    76	            route_map: build_route_map(),
    77	            history: CommandHistory::new(history_path, 50),
    78	            completions: Vec::new(),
    79	            completion_index: 0,
    80	            last_prefix: None,
    81	        }
    82	    }
    83	
    84	    /// Parse a command string. Resolves abbreviations first.
    85	    pub fn parse(&mut self, input: &str) -> Command {
    86	        let trimmed = input.trim();
    87	        if trimmed.is_empty() {
    88	            return Command::Unknown(String::new());
    89	        }
    90	
    91	        let parts: Vec<&str> = trimmed.splitn(2, ' ').collect();
    92	        let cmd = parts[0].to_lowercase();
    93	        let arg = parts.get(1).map(|s| s.trim().to_string());
    94	
    95	        // Resolve abbreviation
    96	        let resolved = self
    97	            .abbreviations
    98	            .get(&cmd)
    99	            .cloned()
   100	            .unwrap_or_else(|| cmd.clone());
   101	
   102	        // System commands
   103	        match resolved.as_str() {
   104	            "q" | "quit" => return Command::Quit,
   105	            "refresh" => return Command::Refresh,
   106	            "help" => return Command::Help,
   107	            "ctx" => {
   108	                return match arg {
   109	                    Some(cloud) if !cloud.is_empty() => Command::ContextSwitch(cloud),
   110	                    _ => Command::ContextList,
   111	                };
   112	            }
   113	            _ => {}
   114	        }
   115	
   116	        // Route navigation
   117	        if let Some(route) = self.route_map.get(&resolved) {
   118	            return Command::Navigate(*route);
   119	        }
   120	
   121	        Command::Unknown(trimmed.to_string())
   122	    }
   123	
   124	    /// Tab auto-complete. Returns the expanded command name (not the abbreviation).
   125	    /// First Tab: collect matching commands by prefix, return first.
   126	    /// Subsequent Tabs with same prefix: cycle through matches.
   127	    /// If the prefix exactly matches an abbreviation, its expanded form is included.
   128	    pub fn auto_complete(&mut self, prefix: &str) -> Option<String> {
   129	        let prefix_lower = prefix.to_lowercase();
   130	
   131	        if self.last_prefix.as_deref() != Some(&prefix_lower) {
   132	            self.completions = self
   133	                .available_commands()
   134	                .into_iter()
   135	                .filter(|cmd| cmd.starts_with(&prefix_lower))
   136	                .collect();
   137	            // Include abbreviation expansions
   138	            for (abbr, full) in &self.abbreviations {
   139	                if abbr.starts_with(&prefix_lower) && !self.completions.contains(full) {
   140	                    self.completions.push(full.clone());
   141	                }
   142	            }
   143	            self.completions.sort();
   144	            self.completions.dedup();
   145	            self.completion_index = 0;
   146	            self.last_prefix = Some(prefix_lower);
   147	        } else if !self.completions.is_empty() {
   148	            self.completion_index = (self.completion_index + 1) % self.completions.len();
   149	        }
   150	
   151	        self.completions.get(self.completion_index).cloned()
   152	    }
   153	
   154	    pub fn reset_completion(&mut self) {
   155	        self.completions.clear();
   156	        self.completion_index = 0;
   157	        self.last_prefix = None;
   158	    }
   159	
   160	    pub fn push_history(&mut self, command: &str) {
   161	        self.history.push(command);
   162	    }
   163	
   164	    pub fn history_prev(&mut self) -> Option<&str> {
   165	        self.history.prev()
   166	    }
   167	
   168	    pub fn history_next(&mut self) -> Option<&str> {
   169	        self.history.next()
   170	    }
   171	
   172	    pub fn history_reset_cursor(&mut self) {
   173	        self.history.reset_cursor();
   174	    }
   175	
   176	    pub fn save_history(&self) -> Result<()> {
   177	        self.history.save()
   178	    }
   179	
   180	    pub fn load_history(&mut self) -> Result<()> {
   181	        self.history.load()
   182	    }
   183	
   184	    /// All valid command names (for auto-complete).
   185	    pub fn available_commands(&self) -> Vec<String> {
   186	        let mut cmds: Vec<String> = self.route_map.keys().cloned().collect();
   187	        cmds.extend(["quit", "refresh", "help", "ctx"].iter().map(|s| s.to_string()));
   188	        cmds.sort();
   189	        cmds
   190	    }
   191	}
   192	
   193	// --- CommandHistory ---
   194	
   195	struct CommandHistory {
   196	    entries: Vec<String>,
   197	    max_size: usize,
   198	    cursor: Option<usize>,
   199	    file_path: PathBuf,
   200	}
   201	
   202	impl CommandHistory {
   203	    fn new(file_path: PathBuf, max_size: usize) -> Self {
   204	        Self {
   205	            entries: Vec::new(),
   206	            max_size,
   207	            cursor: None,
   208	            file_path,
   209	        }
   210	    }
   211	
   212	    fn push(&mut self, command: &str) {
   213	        let cmd = command.trim().to_string();
   214	        if cmd.is_empty() || cmd.len() > MAX_HISTORY_ENTRY_LEN {
   215	            return;
   216	        }
   217	        self.entries.retain(|e| e != &cmd);
   218	        self.entries.push(cmd);
   219	        if self.entries.len() > self.max_size {
   220	            self.entries.remove(0);

 succeeded in 0ms:
     1	use std::sync::Arc;
     2	use std::time::Duration;
     3	
     4	use reqwest::{Method, RequestBuilder, Response};
     5	use serde::de::DeserializeOwned;
     6	use tokio::sync::RwLock;
     7	
     8	use crate::port::auth::AuthProvider;
     9	use crate::port::error::{ApiError, ApiResult};
    10	use crate::port::types::EndpointInterface;
    11	
    12	/// Shared HTTP plumbing for all service adapters.
    13	/// Auth injection is delegated to AuthProvider::authenticate_request().
    14	///
    15	/// Endpoint caching note: cached endpoint is resolved once and reused.
    16	/// Callers should call `invalidate_endpoint()` when token refresh occurs
    17	/// to pick up potential catalog changes. In Phase 2, BaseHttpClient will
    18	/// subscribe to token refresh broadcast to automate this.
    19	pub struct BaseHttpClient {
    20	    client: reqwest::Client,
    21	    auth: Arc<dyn AuthProvider>,
    22	    service_type: String,
    23	    interface: EndpointInterface,
    24	    region: Option<String>,
    25	    endpoint: RwLock<Option<String>>,
    26	}
    27	
    28	impl BaseHttpClient {
    29	    pub fn new(
    30	        auth: Arc<dyn AuthProvider>,
    31	        service_type: &str,
    32	        interface: EndpointInterface,
    33	        region: Option<String>,
    34	    ) -> Result<Self, ApiError> {
    35	        Ok(Self {
    36	            client: reqwest::Client::builder()
    37	                .timeout(Duration::from_secs(30))
    38	                .connect_timeout(Duration::from_secs(10))
    39	                .build()?,
    40	            auth,
    41	            service_type: service_type.to_string(),
    42	            interface,
    43	            region,
    44	            endpoint: RwLock::new(None),
    45	        })
    46	    }
    47	
    48	    /// Resolve and cache the endpoint from service catalog.
    49	    #[tracing::instrument(skip(self), fields(service = %self.service_type))]
    50	    async fn resolve_endpoint(&self) -> ApiResult<String> {
    51	        {
    52	            let cached = self.endpoint.read().await;
    53	            if let Some(url) = cached.as_ref() {
    54	                return Ok(url.clone());
    55	            }
    56	        }
    57	        let url = self
    58	            .auth
    59	            .get_endpoint(&self.service_type, self.interface.clone(), self.region.as_deref())
    60	            .await?;
    61	        let mut cached = self.endpoint.write().await;
    62	        *cached = Some(url.clone());
    63	        Ok(url)
    64	    }
    65	
    66	    /// Invalidate cached endpoint. Should be called on token refresh
    67	    /// to pick up potential service catalog changes.
    68	    pub async fn invalidate_endpoint(&self) {
    69	        let mut cached = self.endpoint.write().await;
    70	        *cached = None;
    71	    }
    72	
    73	    /// Build an authenticated request.
    74	    /// Note (Phase 2): For signed auth methods (HMAC), authenticate_request()
    75	    /// will need actual headers/body. Currently passes empty values since
    76	    /// Phase 1 only uses X-Auth-Token which doesn't depend on request content.
    77	    async fn request(&self, method: Method, path: &str) -> ApiResult<RequestBuilder> {
    78	        let endpoint = self.resolve_endpoint().await?;
    79	        let url = format!("{}{}", endpoint.trim_end_matches('/'), path);
    80	        let method_str = method.as_str();
    81	        let empty_headers = reqwest::header::HeaderMap::new();
    82	        let auth_headers = self
    83	            .auth
    84	            .authenticate_request(method_str, &url, &empty_headers, None)
    85	            .await?;
    86	        let mut builder = self
    87	            .client
    88	            .request(method, &url)
    89	            .header("Content-Type", "application/json");
    90	        for (key, value) in &auth_headers.headers {
    91	            builder = builder.header(key.as_str(), value.as_str());
    92	        }
    93	        Ok(builder)
    94	    }
    95	
    96	    pub async fn get(&self, path: &str) -> ApiResult<RequestBuilder> {
    97	        self.request(Method::GET, path).await
    98	    }
    99	
   100	    pub async fn post(&self, path: &str) -> ApiResult<RequestBuilder> {
   101	        self.request(Method::POST, path).await
   102	    }
   103	
   104	    pub async fn put(&self, path: &str) -> ApiResult<RequestBuilder> {
   105	        self.request(Method::PUT, path).await
   106	    }
   107	
   108	    pub async fn patch(&self, path: &str) -> ApiResult<RequestBuilder> {
   109	        self.request(Method::PATCH, path).await
   110	    }
   111	
   112	    pub async fn delete(&self, path: &str) -> ApiResult<RequestBuilder> {
   113	        self.request(Method::DELETE, path).await
   114	    }
   115	
   116	    /// Send a request and map HTTP errors to ApiError.
   117	    pub async fn send(&self, request: RequestBuilder) -> ApiResult<Response> {
   118	        let resp = request.send().await.map_err(ApiError::Network)?;
   119	        Self::check_status(resp).await
   120	    }
   121	
   122	    /// Send + deserialize JSON body.
   123	    pub async fn send_json<T: DeserializeOwned>(
   124	        &self,
   125	        request: RequestBuilder,
   126	    ) -> ApiResult<T> {
   127	        let resp = self.send(request).await?;
   128	        resp.json::<T>()
   129	            .await
   130	            .map_err(|e| ApiError::Parse(format!("JSON deserialization failed: {e}")))
   131	    }
   132	
   133	    /// Send and expect 204 No Content (or 202 Accepted).
   134	    pub async fn send_no_content(&self, request: RequestBuilder) -> ApiResult<()> {
   135	        self.send(request).await?;
   136	        Ok(())
   137	    }
   138	
   139	    /// Extract a human-readable error message from OpenStack JSON error bodies.
   140	    /// Handles formats: {"NeutronError": {"message": "..."}}, {"badRequest": {"message": "..."}},

exec
/bin/zsh -lc "nl -ba src/adapter/auth/keystone.rs | sed -n '120,280p'" in /Users/jay.ahn/projects/infra/nexttui/.worktrees/runtime-context-switch
 succeeded in 0ms:
   120	
   121	fn parse_interface(s: &str) -> EndpointInterface {
   122	    match s {
   123	        "internal" => EndpointInterface::Internal,
   124	        "admin" => EndpointInterface::Admin,
   125	        _ => EndpointInterface::Public,
   126	    }
   127	}
   128	
   129	// --- KeystoneAuthAdapter ---
   130	
   131	pub struct KeystoneAuthAdapter {
   132	    client: reqwest::Client,
   133	    credential: AuthCredential,
   134	    token_map: Arc<RwLock<HashMap<TokenScope, Token>>>,
   135	    active_scope: TokenScope,
   136	    token_tx: broadcast::Sender<Token>,
   137	    refresh_handle: Mutex<Option<JoinHandle<()>>>,
   138	    /// Guard to ensure refresh loop is started only once.
   139	    refresh_started: AtomicBool,
   140	    /// Mutex to serialize concurrent refresh attempts (prevents thundering herd).
   141	    refresh_lock: Mutex<()>,
   142	    /// Directory for scope-keyed token cache files.
   143	    cache_dir: PathBuf,
   144	}
   145	
   146	impl KeystoneAuthAdapter {
   147	    pub fn new(credential: AuthCredential) -> Result<Self, ApiError> {
   148	        use super::token_cache;
   149	
   150	        let username = match &credential.method {
   151	            AuthMethod::Password { username, .. } => username.clone(),
   152	            AuthMethod::ApplicationCredential { id, .. } => id.clone(),
   153	        };
   154	        let active_scope = TokenScope::from_credential(&credential);
   155	        let cloud_key = token_cache::compute_cloud_key(&credential.auth_url, &username);
   156	        let cache_dir = token_cache::cache_dir_path(&cloud_key);
   157	
   158	        // Load all cached tokens for this cloud from disk
   159	        let cached_tokens = token_cache::load_all_tokens(&cache_dir);
   160	
   161	        let (token_tx, _) = broadcast::channel::<Token>(16);
   162	        Ok(Self {
   163	            client: reqwest::Client::builder()
   164	                .timeout(Duration::from_secs(30))
   165	                .connect_timeout(Duration::from_secs(10))
   166	                .build()?,
   167	            credential,
   168	            token_map: Arc::new(RwLock::new(cached_tokens)),
   169	            active_scope,
   170	            token_tx,
   171	            refresh_handle: Mutex::new(None),
   172	            refresh_started: AtomicBool::new(false),
   173	            refresh_lock: Mutex::new(()),
   174	            cache_dir,
   175	        })
   176	    }
   177	
   178	    /// Start the background token refresh loop. Idempotent — only spawns once.
   179	    #[tracing::instrument(skip(self))]
   180	    async fn start_refresh_loop(&self) {
   181	        if self.refresh_started.swap(true, Ordering::SeqCst) {
   182	            return; // Already started
   183	        }
   184	
   185	        let token_map_ref = self.token_map.clone();
   186	        let client = self.client.clone();
   187	        let credential = self.credential.clone();
   188	        let tx = self.token_tx.clone();
   189	        let cache_dir = self.cache_dir.clone();
   190	        let scope = self.active_scope.clone();
   191	
   192	        let refresh_span = tracing::info_span!("token_refresh_loop");
   193	        let handle = tokio::spawn(
   194	            async move {
   195	                loop {
   196	                    let sleep_duration = {
   197	                        let map = token_map_ref.read().await;
   198	                        match map.get(&scope) {
   199	                            Some(t) => {
   200	                                let remaining = t.expires_at - Utc::now();
   201	                                let refresh_at = remaining - chrono::Duration::minutes(5);
   202	                                if refresh_at.num_seconds() > 0 {
   203	                                    Duration::from_secs(refresh_at.num_seconds() as u64)
   204	                                } else {
   205	                                    Duration::from_secs(10)
   206	                                }
   207	                            }
   208	                            None => Duration::from_secs(60),
   209	                        }
   210	                    };
   211	
   212	                    tokio::time::sleep(sleep_duration).await;
   213	
   214	                    match Self::do_authenticate(&client, &credential).await {
   215	                        Ok(new_token) => {
   216	                            let mut map = token_map_ref.write().await;
   217	                            map.insert(scope.clone(), new_token.clone());
   218	                            if let Err(e) = super::token_cache::save_token(&new_token, &cache_dir, &scope) {
   219	                                tracing::warn!(error = %e, "failed to cache token to disk");
   220	                            }
   221	                            let _ = tx.send(new_token);
   222	                        }
   223	                        Err(e) => {
   224	                            tracing::warn!(error = %e, "token refresh failed, retrying in 30s");
   225	                            tokio::time::sleep(Duration::from_secs(30)).await;
   226	                        }
   227	                    }
   228	                }
   229	            }
   230	            .instrument(refresh_span),
   231	        );
   232	
   233	        let mut h = self.refresh_handle.lock().await;
   234	        *h = Some(handle);
   235	    }
   236	
   237	    /// Perform the actual Keystone v3 auth POST.
   238	    #[tracing::instrument(skip(client, credential), fields(auth_url = %credential.auth_url))]
   239	    async fn do_authenticate(
   240	        client: &reqwest::Client,
   241	        credential: &AuthCredential,
   242	    ) -> ApiResult<Token> {
   243	        let auth_url = format!(
   244	            "{}/auth/tokens",
   245	            credential.auth_url.trim_end_matches('/')
   246	        );
   247	        let body = Self::build_auth_body(credential);
   248	        let resp = client
   249	            .post(&auth_url)
   250	            .json(&body)
   251	            .send()
   252	            .await
   253	            .map_err(ApiError::Network)?;
   254	
   255	        if !resp.status().is_success() {
   256	            let body = resp.text().await.unwrap_or_default();
   257	            return Err(ApiError::AuthFailed(body));
   258	        }
   259	
   260	        let token_id = resp
   261	            .headers()
   262	            .get("X-Subject-Token")
   263	            .and_then(|v| v.to_str().ok())
   264	            .ok_or_else(|| ApiError::AuthFailed("Missing X-Subject-Token header".into()))?
   265	            .to_string();
   266	
   267	        let body: KeystoneTokenResponse = resp
   268	            .json()
   269	            .await
   270	            .map_err(|e| ApiError::Parse(format!("Failed to parse Keystone response: {e}")))?;
   271	
   272	        Ok(parse_token(token_id, body))
   273	    }
   274	
   275	    /// Build the Keystone v3 auth request body based on AuthMethod.
   276	    /// Note: authenticate() always uses self.credential (passed at construction).
   277	    /// The credential parameter in AuthProvider::authenticate() should match self.credential.
   278	    pub(crate) fn build_auth_body(credential: &AuthCredential) -> serde_json::Value {
   279	        let identity = match &credential.method {
   280	            AuthMethod::Password {

exec
/bin/zsh -lc "nl -ba src/input/keymap.rs | sed -n '1,180p'" in /Users/jay.ahn/projects/infra/nexttui/.worktrees/runtime-context-switch
 succeeded in 0ms:
     1	use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
     2	
     3	#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
     4	pub enum AppMode {
     5	    Normal,
     6	    Command,
     7	    Search,
     8	    Form,
     9	    Dialog,
    10	}
    11	
    12	#[derive(Debug, Clone, PartialEq)]
    13	pub enum KeyAction {
    14	    MoveUp,
    15	    MoveDown,
    16	    MoveToTop,
    17	    MoveToBottom,
    18	    PageUp,
    19	    PageDown,
    20	    Select,
    21	    Back,
    22	
    23	    EnterCommandMode,
    24	    EnterSearchMode,
    25	    ToggleSidebar,
    26	
    27	    Create,
    28	    Delete,
    29	    Edit,
    30	    Refresh,
    31	
    32	    NextField,
    33	    PrevField,
    34	    ToggleField,
    35	    SubmitForm,
    36	    CancelForm,
    37	
    38	    Confirm,
    39	    Deny,
    40	
    41	    Quit,
    42	    ForceQuit,
    43	
    44	    CharInput(char),
    45	    Unmapped,
    46	}
    47	
    48	pub struct KeyMap;
    49	
    50	impl KeyMap {
    51	    pub fn new() -> Self {
    52	        Self
    53	    }
    54	
    55	    /// Resolve a key event to a semantic action based on current mode.
    56	    pub fn resolve(&self, mode: AppMode, key: KeyEvent) -> KeyAction {
    57	        // Force quit always works
    58	        if key.code == KeyCode::Char('c') && key.modifiers.contains(KeyModifiers::CONTROL) {
    59	            return KeyAction::ForceQuit;
    60	        }
    61	
    62	        match mode {
    63	            AppMode::Normal => self.resolve_normal(key),
    64	            AppMode::Command => self.resolve_command(key),
    65	            AppMode::Search => self.resolve_search(key),
    66	            AppMode::Form => self.resolve_form(key),
    67	            AppMode::Dialog => self.resolve_dialog(key),
    68	        }
    69	    }
    70	
    71	    /// Generate context help string for status bar.
    72	    pub fn context_help(&self, mode: AppMode) -> String {
    73	        match mode {
    74	            AppMode::Normal => "j/k:move  Enter:select  /:search  ::cmd  Tab:sidebar  q:quit".into(),
    75	            AppMode::Command => "Enter:run  Tab:complete  Up/Down:history  Esc:cancel".into(),
    76	            AppMode::Search => "Enter:apply  Esc:cancel  (type to filter)".into(),
    77	            AppMode::Form => "Tab:next  Shift+Tab:prev  Enter:submit  Esc:cancel".into(),
    78	            AppMode::Dialog => "y:confirm  n/Esc:deny".into(),
    79	        }
    80	    }
    81	
    82	    fn resolve_normal(&self, key: KeyEvent) -> KeyAction {
    83	        match key.code {
    84	            KeyCode::Char('j') | KeyCode::Down => KeyAction::MoveDown,
    85	            KeyCode::Char('k') | KeyCode::Up => KeyAction::MoveUp,
    86	            KeyCode::Char('g') => KeyAction::MoveToTop,
    87	            KeyCode::Char('G') => KeyAction::MoveToBottom,
    88	            KeyCode::PageUp => KeyAction::PageUp,
    89	            KeyCode::PageDown => KeyAction::PageDown,
    90	            KeyCode::Char('u') if key.modifiers.contains(KeyModifiers::CONTROL) => {
    91	                KeyAction::PageUp
    92	            }
    93	            KeyCode::Char('d') if key.modifiers.contains(KeyModifiers::CONTROL) => {
    94	                KeyAction::PageDown
    95	            }
    96	            KeyCode::Enter => KeyAction::Select,
    97	            KeyCode::Esc => KeyAction::Back,
    98	            KeyCode::Char(':') => KeyAction::EnterCommandMode,
    99	            KeyCode::Char('/') => KeyAction::EnterSearchMode,
   100	            KeyCode::Tab => KeyAction::ToggleSidebar,
   101	            KeyCode::Char('c') => KeyAction::Create,
   102	            KeyCode::Char('d') => KeyAction::Delete,
   103	            KeyCode::Char('e') => KeyAction::Edit,
   104	            KeyCode::Char('r') => KeyAction::Refresh,
   105	            KeyCode::Char('q') => KeyAction::Quit,
   106	            _ => KeyAction::Unmapped,
   107	        }
   108	    }
   109	
   110	    fn resolve_command(&self, key: KeyEvent) -> KeyAction {
   111	        match key.code {
   112	            KeyCode::Enter => KeyAction::Select,
   113	            KeyCode::Esc => KeyAction::Back,
   114	            KeyCode::Tab => KeyAction::NextField,
   115	            KeyCode::Up => KeyAction::MoveUp,
   116	            KeyCode::Down => KeyAction::MoveDown,
   117	            KeyCode::Char(c) => KeyAction::CharInput(c),
   118	            KeyCode::Backspace => KeyAction::CharInput('\x08'),
   119	            _ => KeyAction::Unmapped,
   120	        }
   121	    }
   122	
   123	    fn resolve_search(&self, key: KeyEvent) -> KeyAction {
   124	        match key.code {
   125	            KeyCode::Enter => KeyAction::Select,
   126	            KeyCode::Esc => KeyAction::Back,
   127	            KeyCode::Char(c) => KeyAction::CharInput(c),
   128	            KeyCode::Backspace => KeyAction::CharInput('\x08'),
   129	            _ => KeyAction::Unmapped,
   130	        }
   131	    }
   132	
   133	    fn resolve_form(&self, key: KeyEvent) -> KeyAction {
   134	        match key.code {
   135	            KeyCode::Tab => KeyAction::NextField,
   136	            KeyCode::BackTab => KeyAction::PrevField,
   137	            KeyCode::Enter => KeyAction::SubmitForm,
   138	            KeyCode::Esc => KeyAction::CancelForm,
   139	            KeyCode::Char(' ') => KeyAction::ToggleField,
   140	            KeyCode::Char('j') | KeyCode::Down => KeyAction::MoveDown,
   141	            KeyCode::Char('k') | KeyCode::Up => KeyAction::MoveUp,
   142	            KeyCode::Char(c) => KeyAction::CharInput(c),
   143	            KeyCode::Backspace => KeyAction::CharInput('\x08'),
   144	            _ => KeyAction::Unmapped,
   145	        }
   146	    }
   147	
   148	    fn resolve_dialog(&self, key: KeyEvent) -> KeyAction {
   149	        match key.code {
   150	            KeyCode::Char('y') | KeyCode::Char('Y') => KeyAction::Confirm,
   151	            KeyCode::Char('n') | KeyCode::Char('N') => KeyAction::Deny,
   152	            KeyCode::Esc => KeyAction::Deny,
   153	            KeyCode::Enter => KeyAction::Confirm,
   154	            KeyCode::Char(c) => KeyAction::CharInput(c),
   155	            KeyCode::Backspace => KeyAction::CharInput('\x08'),
   156	            _ => KeyAction::Unmapped,
   157	        }
   158	    }
   159	}
   160	
   161	impl Default for KeyMap {
   162	    fn default() -> Self {
   163	        Self::new()
   164	    }
   165	}
   166	
   167	#[cfg(test)]
   168	mod tests {
   169	    use super::*;
   170	
   171	    fn key(code: KeyCode) -> KeyEvent {
   172	        KeyEvent::from(code)
   173	    }
   174	
   175	    fn key_with_mod(code: KeyCode, modifiers: KeyModifiers) -> KeyEvent {
   176	        KeyEvent::new(code, modifiers)
   177	    }
   178	
   179	    #[test]
   180	    fn test_normal_mode_navigation() {

exec
/bin/zsh -lc "rg -n \"CommandParser|parse\\(|ContextSwitch|ContextList|input_mode == InputMode::Command|command\" src/app.rs" in /Users/jay.ahn/projects/infra/nexttui/.worktrees/runtime-context-switch
 succeeded in 0ms:
1378:        // ':' should NOT switch to command mode

codex
I’m now checking command/key handling integration details in `App` because PR3/PR4 safety depends on exactly where switching becomes reachable in runtime, not just in the list table.
exec
/bin/zsh -lc "nl -ba src/app.rs | sed -n '1180,1480p'" in /Users/jay.ahn/projects/infra/nexttui/.worktrees/runtime-context-switch
 succeeded in 0ms:
  1180	        app.input_mode = InputMode::Form;
  1181	
  1182	        for _ in 0..150 {
  1183	            app.on_tick();
  1184	        }
  1185	        let mut found = false;
  1186	        while let Ok(action) = rx.try_recv() {
  1187	            if matches!(action, Action::FetchServers) { found = true; }
  1188	        }
  1189	        assert!(!found, "should not dispatch when in form mode");
  1190	    }
  1191	
  1192	    #[test]
  1193	    fn test_on_tick_suppressed_when_modal() {
  1194	        let (tx, mut rx) = mpsc::unbounded_channel();
  1195	        let config = test_config();
  1196	        let mut app = App::new(config, tx);
  1197	        let mut mock = RefreshMock::new(Action::FetchServers);
  1198	        mock.modal = true;
  1199	        app.register_component(Route::Servers, Box::new(mock));
  1200	        app.router = Router::new(Route::Servers);
  1201	
  1202	        for _ in 0..150 {
  1203	            app.on_tick();
  1204	        }
  1205	        let mut found = false;
  1206	        while let Ok(action) = rx.try_recv() {
  1207	            if matches!(action, Action::FetchServers) { found = true; }
  1208	        }
  1209	        assert!(!found, "should not dispatch when modal is active");
  1210	    }
  1211	
  1212	    // --- API Backoff ---
  1213	
  1214	    #[test]
  1215	    fn test_api_error_rate_limited_triggers_backoff() {
  1216	        let (tx, mut rx) = mpsc::unbounded_channel();
  1217	        let config = test_config();
  1218	        let mut app = App::new(config, tx);
  1219	        app.register_component(Route::Servers, Box::new(RefreshMock::new(Action::FetchServers)));
  1220	        app.router = Router::new(Route::Servers);
  1221	
  1222	        app.handle_event(AppEvent::ApiError {
  1223	            operation: "FetchServers".into(),
  1224	            message: "Rate limited: retry after 30s".into(),
  1225	        });
  1226	
  1227	        // After backoff, 150 ticks should NOT trigger (needs 300 at 2x)
  1228	        for _ in 0..150 {
  1229	            app.on_tick();
  1230	        }
  1231	        let mut found = false;
  1232	        while let Ok(action) = rx.try_recv() {
  1233	            if matches!(action, Action::FetchServers) { found = true; }
  1234	        }
  1235	        assert!(!found, "should not trigger at 150 ticks after backoff (2x = 300 needed)");
  1236	    }
  1237	
  1238	    #[test]
  1239	    fn test_api_error_service_unavailable_triggers_backoff() {
  1240	        let (tx, mut rx) = mpsc::unbounded_channel();
  1241	        let config = test_config();
  1242	        let mut app = App::new(config, tx);
  1243	        app.register_component(Route::Servers, Box::new(RefreshMock::new(Action::FetchServers)));
  1244	        app.router = Router::new(Route::Servers);
  1245	
  1246	        app.handle_event(AppEvent::ApiError {
  1247	            operation: "FetchServers".into(),
  1248	            message: "Service unavailable: nova".into(),
  1249	        });
  1250	
  1251	        for _ in 0..150 {
  1252	            app.on_tick();
  1253	        }
  1254	        let mut found = false;
  1255	        while let Ok(action) = rx.try_recv() {
  1256	            if matches!(action, Action::FetchServers) { found = true; }
  1257	        }
  1258	        assert!(!found, "should not trigger at 150 ticks after backoff (2x = 300 needed)");
  1259	    }
  1260	
  1261	    #[test]
  1262	    fn test_success_event_resets_backoff() {
  1263	        let (tx, mut rx) = mpsc::unbounded_channel();
  1264	        let config = test_config();
  1265	        let mut app = App::new(config, tx);
  1266	        app.register_component(Route::Servers, Box::new(RefreshMock::new(Action::FetchServers)));
  1267	        app.router = Router::new(Route::Servers);
  1268	
  1269	        // Trigger backoff
  1270	        app.handle_event(AppEvent::ApiError {
  1271	            operation: "FetchServers".into(),
  1272	            message: "Rate limited: retry after 30s".into(),
  1273	        });
  1274	        // Then success event resets backoff
  1275	        app.handle_event(AppEvent::ServersLoaded(vec![]));
  1276	
  1277	        // After reset, 150 ticks should trigger (back to 1x)
  1278	        for _ in 0..150 {
  1279	            app.on_tick();
  1280	        }
  1281	        let mut found = false;
  1282	        while let Ok(action) = rx.try_recv() {
  1283	            if matches!(action, Action::FetchServers) { found = true; }
  1284	        }
  1285	        assert!(found, "should trigger at 150 ticks after backoff reset");
  1286	    }
  1287	
  1288	    // --- Step 6: Navigate/Back reset ---
  1289	
  1290	    // --- Unit 2: Activity Log Popup integration ---
  1291	
  1292	    fn make_key_with_modifiers(code: KeyCode, modifiers: KeyModifiers) -> KeyEvent {
  1293	        KeyEvent {
  1294	            code,
  1295	            modifiers,
  1296	            kind: KeyEventKind::Press,
  1297	            state: KeyEventState::NONE,
  1298	        }
  1299	    }
  1300	
  1301	    #[test]
  1302	    fn test_exclamation_toggles_show_activity_log() {
  1303	        let mut app = make_app();
  1304	        assert!(!app.show_activity_log);
  1305	        // '!' is Shift+1 in crossterm
  1306	        app.handle_key(make_key_with_modifiers(KeyCode::Char('!'), KeyModifiers::SHIFT));
  1307	        assert!(app.show_activity_log);
  1308	        app.handle_key(make_key_with_modifiers(KeyCode::Char('!'), KeyModifiers::SHIFT));
  1309	        assert!(!app.show_activity_log);
  1310	    }
  1311	
  1312	    #[test]
  1313	    fn test_close_activity_popup_resets_scroll() {
  1314	        let mut app = make_app();
  1315	        // Open popup
  1316	        app.handle_key(make_key_with_modifiers(KeyCode::Char('!'), KeyModifiers::SHIFT));
  1317	        assert!(app.show_activity_log);
  1318	        // Scroll down
  1319	        app.handle_key(make_key(KeyCode::Char('j')));
  1320	        // Close with Esc
  1321	        app.handle_key(make_key(KeyCode::Esc));
  1322	        assert!(!app.show_activity_log);
  1323	        assert_eq!(app.activity_popup.scroll_offset(), 0);
  1324	    }
  1325	
  1326	    #[test]
  1327	    fn test_exclamation_calls_mark_all_read_on_open() {
  1328	        let mut app = make_app();
  1329	        // Push an unread error entry
  1330	        app.activity_log.push(crate::ui::activity_log::ActivityEntry {
  1331	            timestamp: Instant::now(),
  1332	            operation: "Delete".into(),
  1333	            resource_name: "srv-1".into(),
  1334	            success: false,
  1335	            message: "fail".into(),
  1336	            read: false,
  1337	        });
  1338	        assert_eq!(app.activity_log.unread_error_count(), 1);
  1339	        // Open popup
  1340	        app.handle_key(make_key_with_modifiers(KeyCode::Char('!'), KeyModifiers::SHIFT));
  1341	        assert!(app.show_activity_log);
  1342	        assert_eq!(app.activity_log.unread_error_count(), 0);
  1343	    }
  1344	
  1345	    #[test]
  1346	    fn test_exclamation_blocked_in_form_mode() {
  1347	        let mut app = make_app();
  1348	        app.input_mode = InputMode::Form;
  1349	        app.register_component(Route::Servers, Box::new(MockComponent::new()));
  1350	        app.handle_key(make_key_with_modifiers(KeyCode::Char('!'), KeyModifiers::SHIFT));
  1351	        assert!(!app.show_activity_log);
  1352	    }
  1353	
  1354	    #[test]
  1355	    fn test_exclamation_blocked_in_confirm_mode() {
  1356	        let mut app = make_app();
  1357	        app.input_mode = InputMode::Confirm;
  1358	        app.register_component(Route::Servers, Box::new(MockComponent::new()));
  1359	        app.handle_key(make_key_with_modifiers(KeyCode::Char('!'), KeyModifiers::SHIFT));
  1360	        assert!(!app.show_activity_log);
  1361	    }
  1362	
  1363	    #[test]
  1364	    fn test_fetch_success_not_logged_to_activity() {
  1365	        let mut app = make_app();
  1366	        app.generate_toast(&AppEvent::ServersLoaded(vec![]));
  1367	        assert!(app.activity_log.entries().is_empty());
  1368	    }
  1369	
  1370	    #[test]
  1371	    fn test_activity_popup_pseudo_modal_blocks_keys() {
  1372	        let mut app = make_app();
  1373	        app.register_component(Route::Servers, Box::new(MockComponent::new()));
  1374	        app.show_activity_log = true;
  1375	        // 'q' should NOT quit when popup is open
  1376	        app.handle_key(make_key(KeyCode::Char('q')));
  1377	        assert!(!app.should_quit);
  1378	        // ':' should NOT switch to command mode
  1379	        app.handle_key(make_key(KeyCode::Char(':')));
  1380	        assert_eq!(app.input_mode, InputMode::Normal);
  1381	    }
  1382	
  1383	    #[test]
  1384	    fn test_activity_popup_esc_closes() {
  1385	        let mut app = make_app();
  1386	        app.show_activity_log = true;
  1387	        app.handle_key(make_key(KeyCode::Esc));
  1388	        assert!(!app.show_activity_log);
  1389	    }
  1390	
  1391	    #[test]
  1392	    fn test_activity_popup_j_k_scroll() {
  1393	        let mut app = make_app();
  1394	        app.show_activity_log = true;
  1395	        // Push entries so scroll_down works
  1396	        for i in 0..5 {
  1397	            app.activity_log.push(crate::ui::activity_log::ActivityEntry {
  1398	                timestamp: Instant::now(),
  1399	                operation: format!("Op{i}"),
  1400	                resource_name: "r".into(),
  1401	                success: true,
  1402	                message: String::new(),
  1403	                read: false,
  1404	            });
  1405	        }
  1406	        app.handle_key(make_key(KeyCode::Char('j')));
  1407	        assert_eq!(app.activity_popup.scroll_offset(), 1);
  1408	        app.handle_key(make_key(KeyCode::Char('k')));
  1409	        assert_eq!(app.activity_popup.scroll_offset(), 0);
  1410	    }
  1411	
  1412	    #[test]
  1413	    fn test_generate_toast_pushes_to_activity_log() {
  1414	        let mut app = make_app();
  1415	        assert!(app.activity_log.entries().is_empty());
  1416	        app.handle_event(AppEvent::ServerDeleted {
  1417	            id: "s1".into(),
  1418	            name: "web-01".into(),
  1419	        });
  1420	        assert_eq!(app.activity_log.entries().len(), 1);
  1421	        let entry = &app.activity_log.entries()[0];
  1422	        assert!(entry.success);
  1423	        assert_eq!(entry.resource_name, "web-01");
  1424	    }
  1425	
  1426	    #[test]
  1427	    fn test_generate_toast_error_pushes_to_activity_log() {
  1428	        let mut app = make_app();
  1429	        app.handle_event(AppEvent::ApiError {
  1430	            operation: "CreateServer".into(),
  1431	            message: "quota exceeded".into(),
  1432	        });
  1433	        assert_eq!(app.activity_log.entries().len(), 1);
  1434	        let entry = &app.activity_log.entries()[0];
  1435	        assert!(!entry.success);
  1436	        assert_eq!(entry.operation, "CreateServer");
  1437	        assert!(entry.message.contains("quota exceeded"));
  1438	    }
  1439	
  1440	    #[test]
  1441	    fn test_error_badge_count_reflects_activity_log() {
  1442	        let mut app = make_app();
  1443	        // Two unread errors
  1444	        app.handle_event(AppEvent::ApiError {
  1445	            operation: "CreateServer".into(),
  1446	            message: "fail1".into(),
  1447	        });
  1448	        app.handle_event(AppEvent::ApiError {
  1449	            operation: "DeleteServer".into(),
  1450	            message: "fail2".into(),
  1451	        });
  1452	        assert_eq!(app.activity_log.unread_error_count(), 2);
  1453	        // Opening popup marks all read
  1454	        app.handle_key(make_key_with_modifiers(KeyCode::Char('!'), KeyModifiers::SHIFT));
  1455	        assert_eq!(app.activity_log.unread_error_count(), 0);
  1456	    }
  1457	
  1458	    // --- Audit Logger integration ---
  1459	
  1460	    fn make_app_with_audit() -> (App, tempfile::TempDir) {
  1461	        let (tx, _rx) = mpsc::unbounded_channel();
  1462	        let config = test_config();
  1463	        let mut app = App::new(config, tx);
  1464	        let dir = tempfile::TempDir::new().unwrap();
  1465	        let path = dir.path().join("audit.log");
  1466	        let logger = crate::infra::audit::AuditLogger::new(path).unwrap();
  1467	        app.set_audit_logger(logger);
  1468	        (app, dir)
  1469	    }
  1470	
  1471	    fn read_audit_lines(dir: &tempfile::TempDir) -> Vec<serde_json::Value> {
  1472	        let path = dir.path().join("audit.log");
  1473	        let content = std::fs::read_to_string(path).unwrap_or_default();
  1474	        content
  1475	            .lines()
  1476	            .filter(|l| !l.is_empty())
  1477	            .map(|l| serde_json::from_str(l).unwrap())
  1478	            .collect()
  1479	    }
  1480	

exec
/bin/zsh -lc "rg -n \"CommandParser|Command::ContextSwitch|ContextList|InputMode::Command|auto_complete\\(|history_prev\\(|history_next\\(\" src -g '*.rs'" in /Users/jay.ahn/projects/infra/nexttui/.worktrees/runtime-context-switch
 succeeded in 0ms:
src/app.rs:235:                    self.input_mode = InputMode::Command;
src/app.rs:300:            InputMode::Command | InputMode::Search | InputMode::Confirm
src/app.rs:941:        assert_eq!(app.input_mode, InputMode::Command);
src/app.rs:974:        app.input_mode = InputMode::Command;
src/input/command.rs:17:    ContextList,
src/input/command.rs:63:pub struct CommandParser {
src/input/command.rs:72:impl CommandParser {
src/input/command.rs:109:                    Some(cloud) if !cloud.is_empty() => Command::ContextSwitch(cloud),
src/input/command.rs:110:                    _ => Command::ContextList,
src/input/command.rs:128:    pub fn auto_complete(&mut self, prefix: &str) -> Option<String> {
src/input/command.rs:164:    pub fn history_prev(&mut self) -> Option<&str> {
src/input/command.rs:168:    pub fn history_next(&mut self) -> Option<&str> {
src/input/command.rs:298:    fn parser() -> CommandParser {
src/input/command.rs:300:        CommandParser::new(dir.path().join("history"))
src/input/command.rs:332:            Command::ContextSwitch("prod".to_string())
src/input/command.rs:334:        assert_eq!(p.parse("ctx"), Command::ContextList);
src/input/command.rs:353:        let result = p.auto_complete("ser");
src/input/command.rs:360:        let r1 = p.auto_complete("s");
src/input/command.rs:362:        let r2 = p.auto_complete("s");
src/input/command.rs:370:        assert!(p.auto_complete("zzz").is_none());
src/input/command.rs:380:        assert_eq!(p.history_prev(), Some("volumes"));
src/input/command.rs:381:        assert_eq!(p.history_prev(), Some("networks"));
src/input/command.rs:382:        assert_eq!(p.history_prev(), Some("servers"));
src/input/command.rs:383:        assert_eq!(p.history_prev(), Some("servers"));
src/input/command.rs:385:        assert_eq!(p.history_next(), Some("networks"));
src/input/command.rs:386:        assert_eq!(p.history_next(), Some("volumes"));
src/input/command.rs:387:        assert_eq!(p.history_next(), None);
src/input/command.rs:397:        assert_eq!(p.history_prev(), Some("servers"));
src/input/command.rs:398:        assert_eq!(p.history_prev(), Some("networks"));
src/input/command.rs:406:        let mut p1 = CommandParser::new(path.clone());
src/input/command.rs:411:        let mut p2 = CommandParser::new(path);
src/input/command.rs:413:        assert_eq!(p2.history_prev(), Some("networks"));
src/input/command.rs:414:        assert_eq!(p2.history_prev(), Some("servers"));
src/input/command.rs:420:        let mut p = CommandParser::new(dir.path().join("history"));
src/input/command.rs:425:        assert_eq!(p.history_prev(), Some("cmd-59"));
src/input/command.rs:428:            p.history_prev();
src/input/command.rs:430:        assert_eq!(p.history_prev(), Some("cmd-10")); // cmd-0..cmd-9 evicted
src/input/command.rs:438:        assert!(p.history_prev().is_none()); // rejected
src/ui/input_bar.rs:123:            InputMode::Command => (":", Theme::warning()),
src/ui/input_bar.rs:160:        bar.activate(InputMode::Command);
src/ui/input_bar.rs:161:        assert_eq!(*bar.mode(), InputMode::Command);
src/ui/input_bar.rs:169:        bar.activate(InputMode::Command);
src/ui/input_bar.rs:191:        bar.activate(InputMode::Command);
src/ui/input_bar.rs:205:        bar.activate(InputMode::Command);
src/ui/input_bar.rs:216:        bar.activate(InputMode::Command);

exec
/bin/zsh -lc "sed -n '1,220p' src/component.rs" in /Users/jay.ahn/projects/infra/nexttui/.worktrees/runtime-context-switch
 succeeded in 0ms:
use crossterm::event::KeyEvent;
use ratatui::Frame;
use ratatui::layout::Rect;

use crate::action::Action;
use crate::event::AppEvent;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LayoutHint {
    Default,
    FullWidth,
}

pub trait Component {
    fn handle_key(&mut self, key: KeyEvent) -> Option<Action>;
    fn handle_event(&mut self, event: &AppEvent);
    fn render(&self, frame: &mut Frame, area: Rect);
    fn set_admin(&mut self, _is_admin: bool) {}
    fn set_all_tenants(&mut self, _all_tenants: bool) {}
    fn help_hint(&self) -> &str { "" }
    fn refresh_action(&self) -> Option<Action> { None }
    fn has_transitional_resources(&self) -> bool { false }
    fn is_modal(&self) -> bool { false }
    fn layout_hint(&self) -> LayoutHint { LayoutHint::Default }
    fn is_busy(&self) -> bool { false }
    /// Dynamic content title based on view state (e.g. "Server: web-01").
    /// Returns None to use the default route label.
    fn content_title(&self) -> Option<String> { None }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum InputMode {
    #[default]
    Normal,
    Command,
    Search,
    Form,
    Confirm,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_input_mode_default() {
        let mode = InputMode::default();
        assert_eq!(mode, InputMode::Normal);
    }

    #[test]
    fn test_component_default_refresh_action_is_none() {
        struct Dummy;
        impl Component for Dummy {
            fn handle_key(&mut self, _key: KeyEvent) -> Option<Action> { None }
            fn handle_event(&mut self, _event: &AppEvent) {}
            fn render(&self, _frame: &mut Frame, _area: Rect) {}
        }
        let d = Dummy;
        assert!(d.refresh_action().is_none());
    }

    #[test]
    fn test_component_default_has_transitional_is_false() {
        struct Dummy;
        impl Component for Dummy {
            fn handle_key(&mut self, _key: KeyEvent) -> Option<Action> { None }
            fn handle_event(&mut self, _event: &AppEvent) {}
            fn render(&self, _frame: &mut Frame, _area: Rect) {}
        }
        let d = Dummy;
        assert!(!d.has_transitional_resources());
    }

    #[test]
    fn test_component_default_is_modal_is_false() {
        struct Dummy;
        impl Component for Dummy {
            fn handle_key(&mut self, _key: KeyEvent) -> Option<Action> { None }
            fn handle_event(&mut self, _event: &AppEvent) {}
            fn render(&self, _frame: &mut Frame, _area: Rect) {}
        }
        let d = Dummy;
        assert!(!d.is_modal());
    }

    #[test]
    fn test_component_default_layout_hint_is_default() {
        struct Dummy;
        impl Component for Dummy {
            fn handle_key(&mut self, _key: KeyEvent) -> Option<Action> { None }
            fn handle_event(&mut self, _event: &AppEvent) {}
            fn render(&self, _frame: &mut Frame, _area: Rect) {}
        }
        let d = Dummy;
        assert_eq!(d.layout_hint(), LayoutHint::Default);
        assert!(!d.is_busy());
    }

    #[test]
    fn test_component_set_admin_default() {
        use crossterm::event::KeyEvent;
        use ratatui::layout::Rect;
        use ratatui::Frame;
        use crate::action::Action;
        use crate::event::AppEvent;

        struct Dummy;
        impl Component for Dummy {
            fn handle_key(&mut self, _key: KeyEvent) -> Option<Action> { None }
            fn handle_event(&mut self, _event: &AppEvent) {}
            fn render(&self, _frame: &mut Frame, _area: Rect) {}
        }

        let mut d = Dummy;
        // Should not panic — default no-op
        d.set_admin(true);
        d.set_admin(false);
    }
}

codex
I have enough context to conclude the review: there are three blocking issues (requirements source mismatch, missing scope-session boundary, and unsafe PR sequencing). I’ll give a direct reject/changes list with concrete component adds/merges and PR boundary fixes.
codex
1. **Top issue 1 — Requirements baseline is wrong**
- What: [`application-design.md`](/Users/jay.ahn/projects/infra/nexttui/.worktrees/runtime-context-switch/devflow-docs/inception/application-design.md#L5) targets BL-P2-031, but [`requirements.md`](/Users/jay.ahn/projects/infra/nexttui/.worktrees/runtime-context-switch/devflow-docs/inception/requirements.md#L7) is a different ViewContext refactor (11 FR, 3 NFR), not the “10 FR + 5 NFR” runtime-switch spec.
- Why it matters: completeness and PR safety cannot be validated against the stated acceptance criteria.
- Concrete suggestion: replace `requirements.md` with BL-P2-031 requirements and add an explicit `FR/NFR -> Component -> PR -> Test` trace table.

2. **Top issue 2 — Missing critical runtime boundary for atomic rescope**
- What: list has `ContextSwitcher`/`KeystoneRescopeAdapter`, but no component/port owns “active scoped token + endpoint cache invalidation” atomically. Current seams do not support this: [`AuthProvider`](/Users/jay.ahn/projects/infra/nexttui/.worktrees/runtime-context-switch/src/port/auth.rs#L8) has no scope-switch API, `KeystoneAuthAdapter.active_scope` is fixed state ([`keystone.rs`](/Users/jay.ahn/projects/infra/nexttui/.worktrees/runtime-context-switch/src/adapter/auth/keystone.rs#L131)), and endpoint invalidation is manual per client ([`base.rs`](/Users/jay.ahn/projects/infra/nexttui/.worktrees/runtime-context-switch/src/adapter/http/base.rs#L66)).
- Why it matters: rescope can succeed but runtime can still issue requests with stale scope/catalog; rollback is not truly atomic.
- Concrete suggestion: add `ContextSessionPort` + implementation (`ScopedAuthSession`), with `begin/commit/rollback` and `invalidate_all_endpoints()` hook through adapter registry.

3. **Top issue 3 — PR sequencing ships unsafe intermediate states**
- What: PR3/PR4 expose switching (`:switch-*`, picker) before PR5 safety visibility/fingerprint ([PR map in design](/Users/jay.ahn/projects/infra/nexttui/.worktrees/runtime-context-switch/devflow-docs/inception/application-design.md#L40)).
- Why it matters: users can switch context without persistent indicator/strong destructive confirmation, violating the stated safety posture.
- Concrete suggestion: move minimal safety (`ContextIndicator` + confirm fingerprint) to PR3 gate, or hard-feature-flag switch commands/picker until PR5 merges.

**Components to add / remove / merge / split**
- Add: `ContextSessionPort` (Port), `ScopedAuthSession` (Service), `EndpointCatalogInvalidator` (Adapter-facing service), `ContextHistoryStore` (for `:switch-back`).
- Add: `ContextTargetResolver` (Service) shared by command/picker/project-action disambiguation.
- Rename: `CommandRegistry` -> `CommandParser` extension (matches current code naming in [`src/input/command.rs`](/Users/jay.ahn/projects/infra/nexttui/.worktrees/runtime-context-switch/src/input/command.rs#L63)).
- Reclassify: `ContextIndicator` as UI widget/view (not Controller), `App` as orchestrator/controller (not Service).
- Split: PR6 `KeyMap` global `s` mapping from project-list-local `s` action to avoid global key coupling.

**PR boundary concerns**
1. PR2 must include session commit/rollback seam + endpoint invalidation tests before any user trigger path.
2. PR3/PR4 are unsafe as currently scoped unless safety UI is pulled earlier or feature-flagged off.
3. PR6 should keep `s` handling module-local; avoid global keymap behavior drift.

**Verdict: REJECT**

Reason: the list is close, but it still misses a required atomic session boundary and currently sequences user-visible switching before required safety guarantees.
tokens used
203,256
1. **Top issue 1 — Requirements baseline is wrong**
- What: [`application-design.md`](/Users/jay.ahn/projects/infra/nexttui/.worktrees/runtime-context-switch/devflow-docs/inception/application-design.md#L5) targets BL-P2-031, but [`requirements.md`](/Users/jay.ahn/projects/infra/nexttui/.worktrees/runtime-context-switch/devflow-docs/inception/requirements.md#L7) is a different ViewContext refactor (11 FR, 3 NFR), not the “10 FR + 5 NFR” runtime-switch spec.
- Why it matters: completeness and PR safety cannot be validated against the stated acceptance criteria.
- Concrete suggestion: replace `requirements.md` with BL-P2-031 requirements and add an explicit `FR/NFR -> Component -> PR -> Test` trace table.

2. **Top issue 2 — Missing critical runtime boundary for atomic rescope**
- What: list has `ContextSwitcher`/`KeystoneRescopeAdapter`, but no component/port owns “active scoped token + endpoint cache invalidation” atomically. Current seams do not support this: [`AuthProvider`](/Users/jay.ahn/projects/infra/nexttui/.worktrees/runtime-context-switch/src/port/auth.rs#L8) has no scope-switch API, `KeystoneAuthAdapter.active_scope` is fixed state ([`keystone.rs`](/Users/jay.ahn/projects/infra/nexttui/.worktrees/runtime-context-switch/src/adapter/auth/keystone.rs#L131)), and endpoint invalidation is manual per client ([`base.rs`](/Users/jay.ahn/projects/infra/nexttui/.worktrees/runtime-context-switch/src/adapter/http/base.rs#L66)).
- Why it matters: rescope can succeed but runtime can still issue requests with stale scope/catalog; rollback is not truly atomic.
- Concrete suggestion: add `ContextSessionPort` + implementation (`ScopedAuthSession`), with `begin/commit/rollback` and `invalidate_all_endpoints()` hook through adapter registry.

3. **Top issue 3 — PR sequencing ships unsafe intermediate states**
- What: PR3/PR4 expose switching (`:switch-*`, picker) before PR5 safety visibility/fingerprint ([PR map in design](/Users/jay.ahn/projects/infra/nexttui/.worktrees/runtime-context-switch/devflow-docs/inception/application-design.md#L40)).
- Why it matters: users can switch context without persistent indicator/strong destructive confirmation, violating the stated safety posture.
- Concrete suggestion: move minimal safety (`ContextIndicator` + confirm fingerprint) to PR3 gate, or hard-feature-flag switch commands/picker until PR5 merges.

**Components to add / remove / merge / split**
- Add: `ContextSessionPort` (Port), `ScopedAuthSession` (Service), `EndpointCatalogInvalidator` (Adapter-facing service), `ContextHistoryStore` (for `:switch-back`).
- Add: `ContextTargetResolver` (Service) shared by command/picker/project-action disambiguation.
- Rename: `CommandRegistry` -> `CommandParser` extension (matches current code naming in [`src/input/command.rs`](/Users/jay.ahn/projects/infra/nexttui/.worktrees/runtime-context-switch/src/input/command.rs#L63)).
- Reclassify: `ContextIndicator` as UI widget/view (not Controller), `App` as orchestrator/controller (not Service).
- Split: PR6 `KeyMap` global `s` mapping from project-list-local `s` action to avoid global key coupling.

**PR boundary concerns**
1. PR2 must include session commit/rollback seam + endpoint invalidation tests before any user trigger path.
2. PR3/PR4 are unsafe as currently scoped unless safety UI is pulled earlier or feature-flagged off.
3. PR6 should keep `s` handling module-local; avoid global keymap behavior drift.

**Verdict: REJECT**

Reason: the list is close, but it still misses a required atomic session boundary and currently sequences user-visible switching before required safety guarantees.
