# Code Generation Plan: event-toast-wiring

> **For agentic workers:** REQUIRED: Use `aidlc:aidlc-code-generation` with the
> "GENERATE" signal to execute this plan. Do NOT implement ad-hoc.

## Files to Create
(none)

## Files to Modify
- [ ] `src/app.rs` — handle_event에서 CUD AppEvent → BackgroundTracker toast 생성, render에서 active_toasts 전달

## Implementation Steps

- [ ] Step 1: CUD 결과 이벤트 → Toast 생성
  - [ ] RED: `test_handle_event_server_created_adds_toast` — ServerCreated 이벤트 처리 후 active_toasts에 Success 토스트 존재
  - [ ] Verify RED: 실패 확인
  - [ ] GREEN: App::handle_event에서 CUD AppEvent 매칭 → BackgroundTracker::add_toast 호출
  - [ ] Verify GREEN: 통과 + 전체 회귀

- [ ] Step 2: ApiError → Error Toast 생성
  - [ ] RED: `test_handle_event_api_error_adds_toast` — ApiError 이벤트 처리 후 active_toasts에 Error 토스트 존재
  - [ ] Verify RED: 실패 확인
  - [ ] GREEN: App::handle_event에서 ApiError → add_toast(Error)
  - [ ] Verify GREEN: 통과 + 전체 회귀

- [ ] Step 3: StatusBar에 active_toasts 연결
  - [ ] RED: (render 테스트 — StatusBar가 toast를 표시하는지 확인, 기존 빈 배열 대신 실제 toasts 전달)
  - [ ] GREEN: render()에서 `&[]` → BackgroundTracker active_toasts를 ToastMessage로 변환하여 전달
  - [ ] Verify GREEN: 통과 + 전체 회귀

## Test Strategy
- [ ] `test_handle_event_server_created_adds_toast`: CUD success → toast
- [ ] `test_handle_event_api_error_adds_toast`: ApiError → error toast
