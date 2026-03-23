# Code Generation Plan: core-runtime

> **For agentic workers:** REQUIRED: Use `aidlc:aidlc-code-generation` with the
> "GENERATE" signal to execute this plan. Do NOT implement ad-hoc.
> `"code-generation: GENERATE — proceed with the approved plan for core-runtime"`

## Files to Create
- [ ] `src/action.rs` — Action enum (UI → Background)
- [ ] `src/event.rs` — AppEvent enum (Background → UI)
- [ ] `src/component.rs` — Component trait + InputMode enum
- [ ] `src/router.rs` — Router (route state machine + history)
- [ ] `src/background.rs` — BackgroundTracker, OperationInfo, Toast, ToastLevel
- [ ] `src/app.rs` — App struct (orchestrator, handle_key, on_tick, render skeleton)
- [ ] `src/event_loop.rs` — run_event_loop (tokio::select! 3-branch)

## Files to Modify
- [ ] `Cargo.toml` — tokio, crossterm, ratatui, futures, uuid 의존성 추가
- [ ] `src/lib.rs` — 신규 모듈 선언
- [ ] `src/main.rs` — tokio::main + App::new + run_event_loop 연결

## Implementation Steps

### Step 1: Action + AppEvent enums
- [ ] RED: `test_action_variants_exist` + `test_app_event_variants_exist` — enum 생성/매칭 검증
- [ ] Verify RED: 컴파일 실패 (모듈 미존재)
- [ ] GREEN: `src/action.rs` + `src/event.rs` 작성, Cargo.toml 의존성 추가
- [ ] Verify GREEN: 테스트 통과 + 전체 회귀

### Step 2: Component trait + InputMode
- [ ] RED: `test_input_mode_default` — InputMode::Normal이 기본
- [ ] Verify RED: 실패 확인
- [ ] GREEN: `src/component.rs` — Component trait + InputMode enum
- [ ] Verify GREEN: 통과 + 회귀

### Step 3: Router
- [ ] RED: `test_router_navigate` — navigate 시 history push + current 변경
- [ ] RED: `test_router_back` — back 시 이전 route
- [ ] RED: `test_router_back_empty` — 빈 history → None
- [ ] RED: `test_router_replace` — replace는 history 변경 없음
- [ ] RED: `test_router_reset` — history 초기화
- [ ] RED: `test_router_history_limit` — 20 초과 시 oldest 제거
- [ ] RED: `test_router_navigate_same_route` — 같은 route 중복 방지
- [ ] Verify RED: 실패 확인
- [ ] GREEN: `src/router.rs` 전체 구현
- [ ] Verify GREEN: 7개 테스트 통과 + 전체 회귀

### Step 4: BackgroundTracker + Toast
- [ ] RED: `test_tracker_started` — Started → InProgress 추가
- [ ] RED: `test_tracker_completed` — Completed → status 변경 + Toast(Success)
- [ ] RED: `test_tracker_failed` — Failed → Toast(Error)
- [ ] RED: `test_toast_expiry` — TTL 후 toast 제거
- [ ] RED: `test_tracker_gc` — 60초 후 항목 정리
- [ ] RED: `test_in_progress_count` — 진행 중 카운트
- [ ] Verify RED: 실패 확인
- [ ] GREEN: `src/background.rs` 전체 구현
- [ ] Verify GREEN: 6개 테스트 통과 + 전체 회귀

### Step 5: App struct (handle_key, on_tick)
- [ ] RED: `test_app_global_key_colon` — `:` → Command
- [ ] RED: `test_app_global_key_slash` — `/` → Search
- [ ] RED: `test_app_global_key_tab` — Tab → sidebar 토글
- [ ] RED: `test_app_global_key_q` — `q` → should_quit
- [ ] RED: `test_app_esc_to_normal` — Esc in Command → Normal
- [ ] RED: `test_app_esc_normal_back` — Esc in Normal → Router.back()
- [ ] RED: `test_app_delegate_to_component` — 비글로벌 키 → component 위임
- [ ] Verify RED: 실패 확인
- [ ] GREEN: `src/app.rs` — App::new, handle_key, on_tick, render skeleton
- [ ] Verify GREEN: 7개 테스트 통과 + 전체 회귀

### Step 6: EventLoop + main.rs 연결
- [ ] GREEN: `src/event_loop.rs` — run_event_loop (tokio::select!)
- [ ] Modify: `src/main.rs` — #[tokio::main], App::new, run_event_loop
- [ ] Verify: `cargo build` 성공 (EventLoop은 통합 테스트 대상, 단위 테스트 없음)
- [ ] REFACTOR: `cargo clippy` + `cargo fmt`
- [ ] Verify: `cargo test` 전체 통과

## Test Strategy

### Action/AppEvent (src/action.rs, src/event.rs — 단위)
- [ ] `test_action_variants_exist`: 주요 Action variant 생성 가능 확인
- [ ] `test_app_event_variants_exist`: 주요 AppEvent variant 생성 가능 확인

### Component/InputMode (src/component.rs — 단위)
- [ ] `test_input_mode_default`: Normal이 기본값

### Router (src/router.rs — 단위)
- [ ] `test_router_navigate`: navigate → history push + current 변경
- [ ] `test_router_back`: back → 이전 route
- [ ] `test_router_back_empty`: 빈 history → None
- [ ] `test_router_replace`: replace → history 불변
- [ ] `test_router_reset`: reset → history 초기화
- [ ] `test_router_history_limit`: 20 초과 → oldest 제거
- [ ] `test_router_navigate_same_route`: 같은 route → 중복 방지

### BackgroundTracker (src/background.rs — 단위)
- [ ] `test_tracker_started`: Started → InProgress
- [ ] `test_tracker_completed`: Completed → Toast(Success)
- [ ] `test_tracker_failed`: Failed → Toast(Error)
- [ ] `test_toast_expiry`: TTL 만료 → 제거
- [ ] `test_tracker_gc`: 60초 후 GC
- [ ] `test_in_progress_count`: 진행 중 카운트

### App (src/app.rs — 단위, mock component)
- [ ] `test_app_global_key_colon`: `:` → Command
- [ ] `test_app_global_key_slash`: `/` → Search
- [ ] `test_app_global_key_tab`: Tab → sidebar 토글
- [ ] `test_app_global_key_q`: `q` → should_quit
- [ ] `test_app_esc_to_normal`: Esc → Normal
- [ ] `test_app_esc_normal_back`: Normal + Esc → back
- [ ] `test_app_delegate_to_component`: 비글로벌 키 → component 위임
