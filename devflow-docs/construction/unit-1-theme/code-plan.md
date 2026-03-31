# Code Generation Plan: Unit 1+2 — Theme 시스템 + LayoutManager + Toast

> **For agentic workers:** REQUIRED: Use `aidlc:aidlc-code-generation` with the
> "GENERATE" signal to execute this plan. Do NOT implement ad-hoc.
> `"code-generation: GENERATE — proceed with the approved plan for unit-1+2-theme-layout-toast"`

## Baseline

- 테스트: 694 passed
- 브랜치: `feature/ui-redesign-theme-layout` (main에서 분기)

## Files to Create

- [ ] `src/ui/theme.rs` — Theme, Icons, panel_title(), key_hint()

## Files to Modify

- [ ] `src/ui/mod.rs` — `pub mod theme;` 추가
- [ ] `src/ui/layout.rs` — LayoutAreas에 toast_bar 추가, calculate() 수정
- [ ] `src/ui/toast.rs` — resource_id 필드, truncation, Theme 토큰 적용
- [ ] `src/ui/status_bar.rs` — active_toasts 파라미터 제거
- [ ] `src/app.rs` — toast_bar 렌더, StatusBar 호출 변경

---

## Implementation Steps

### Step 1: Theme 구조체 — 시맨틱 스타일 토큰

> **신규 파일**: `src/ui/theme.rs`

- [ ] RED: `test_theme_active_is_yellow_bold` — active()가 Yellow + Bold 반환
- [ ] RED: `test_theme_done_is_green` — done()이 Green 반환
- [ ] RED: `test_theme_error_is_red` — error()가 Red 반환
- [ ] RED: `test_theme_highlight_is_bold_only` — highlight()가 fg 없이 Bold만 반환
- [ ] RED: `test_theme_active_vs_warning` — active()는 Bold, warning()은 Bold 없음
- [ ] Verify RED: 5개 테스트 컴파일 실패 확인
- [ ] GREEN: Theme impl
  ```rust
  pub struct Theme;
  impl Theme {
      pub fn active() -> Style   // Yellow Bold (전이 상태)
      pub fn done() -> Style     // Green (정상)
      pub fn error() -> Style    // Red
      pub fn waiting() -> Style  // DarkGray
      pub fn warning() -> Style  // Yellow (Bold 없음)
      pub fn focus_border() -> Style   // Cyan
      pub fn unfocus_border() -> Style // DarkGray
      pub fn highlight() -> Style      // Bold only (fg 없음)
      pub fn disabled() -> Style       // DarkGray Dim
      pub fn link() -> Style           // Cyan Underline
      pub fn timestamp() -> Style      // Cyan
  }
  ```
- [ ] `src/ui/mod.rs`에 `pub mod theme;` 추가
- [ ] Verify GREEN: 5개 통과 + 694 기존 회귀 통과

### Step 2: Icons 구조체 — 상태별 Unicode 아이콘

- [ ] RED: `test_icons_active` — active()가 "●" 반환
- [ ] RED: `test_icons_status_icon_mapping` — status_icon("ACTIVE")→"●", status_icon("ERROR")→"✗", status_icon("UNKNOWN")→"?"
- [ ] Verify RED: 2개 실패 확인
- [ ] GREEN: Icons impl
  ```rust
  pub struct Icons;
  impl Icons {
      pub fn active() -> &'static str    // "●"
      pub fn shutoff() -> &'static str   // "○"
      pub fn error() -> &'static str     // "✗"
      pub fn building() -> &'static str  // "⟳"
      pub fn verify() -> &'static str    // "◐"
      pub fn migrating() -> &'static str // "↔"
      pub fn status_icon(status: &str) -> &'static str  // 매핑, fallback "?"
  }
  ```
- [ ] Verify GREEN: 2개 + 전체 회귀 통과

### Step 3: panel_title() 유틸 함수

- [ ] RED: `test_panel_title_focused` — panel_title("Servers", true) → `"[ Servers ]"`
- [ ] RED: `test_panel_title_unfocused` — panel_title("Servers", false) → `"  Servers  "`
- [ ] Verify RED: 2개 실패 확인
- [ ] GREEN: panel_title 구현
- [ ] Verify GREEN: 2개 + 전체 회귀 통과

### Step 4: key_hint() 유틸 함수

- [ ] RED: `test_key_hint_produces_two_spans` — key_hint("Tab", "패널") → 2개 Span
- [ ] RED: `test_key_hint_key_is_cyan_bold` — 첫 Span이 Cyan + Bold
- [ ] Verify RED: 2개 실패 확인
- [ ] GREEN: key_hint 구현
  ```rust
  pub fn key_hint<'a>(key: &'a str, desc: &'a str) -> Vec<Span<'a>>;
  // [Span(key, Cyan Bold), Span(" ", reset), Span(desc, Dim)]
  ```
- [ ] Verify GREEN: 2개 + 전체 회귀 통과

> **Step 1-4 완료 시점**: Theme 시스템 완성, 기존 694 + ~11 신규 테스트 통과

---

### Step 5: LayoutAreas에 toast_bar 추가

> **변경 파일**: `src/ui/layout.rs`

- [ ] RED: `test_layout_has_toast_bar` — LayoutAreas에 toast_bar 필드 존재, height == 1
- [ ] RED: `test_layout_no_overlap_80x24` — 80x24에서 header/body/input/toast/status 영역 겹침 없음
- [ ] RED: `test_layout_body_height_is_frame_minus_4` — body = frame.height - 4 (header 1 + input 1 + toast 1 + status 1)
- [ ] Verify RED: 3개 실패 확인 (toast_bar 필드 없으므로 컴파일 에러)
- [ ] GREEN: LayoutAreas + calculate() 수정
  ```rust
  pub struct LayoutAreas {
      pub header: Rect,
      pub sidebar: Option<Rect>,
      pub content: Rect,
      pub input_bar: Rect,
      pub toast_bar: Rect,     // 신규: 항상 1줄 예약
      pub status_bar: Rect,
  }
  // calculate() 레이아웃:
  // Header(1) + Body(Min(0)) + InputBar(1) + ToastBar(1) + StatusBar(1)
  ```
- [ ] 기존 layout 테스트 6개 수정: body height 어설션 조정 (30-3→30-4 등)
- [ ] Verify GREEN: 기존 6개 수정 + 3개 신규 + 전체 회귀 통과

> **주의**: LayoutAreas에 toast_bar 추가 시 `app.rs` 컴파일 에러 발생.
> Step 5 GREEN에서 app.rs 최소 수정 (toast_bar 무시) 진행하여 컴파일 통과.

### Step 6: ToastMessage에 resource_id + truncation + Theme 적용

> **변경 파일**: `src/ui/toast.rs`

- [ ] RED: `test_toast_resource_id_field` — ToastMessage에 resource_id: Option<String> 필드 존재
- [ ] RED: `test_toast_with_resource_id_format` — resource_id 있을 때 `"[OK] server-01: Resize confirmed"` 포맷
- [ ] RED: `test_toast_truncation_75_chars` — 75자 초과 시 `…` 말줄임
- [ ] RED: `test_toast_truncation_preserves_short` — 75자 이하는 그대로
- [ ] RED: `test_toast_color_uses_theme` — color()가 Theme 토큰의 fg Color 반환
- [ ] Verify RED: 5개 실패 확인
- [ ] GREEN: ToastMessage 변경
  ```rust
  pub struct ToastMessage {
      pub text: String,
      pub severity: ToastSeverity,
      pub resource_id: Option<String>,  // 신규
  }
  // 생성자에 resource_id: None 기본값
  // display_text() -> String: resource_id 포함 포맷 + 75자 truncation
  // color() -> Theme 토큰 사용 (done/error/warning)
  // render(): display_text() 사용
  ```
- [ ] 기존 toast 테스트 2개 수정: resource_id 필드 추가
- [ ] Verify GREEN: 기존 2개 수정 + 5개 신규 + 전체 회귀 통과

### Step 7: StatusBar에서 toast 파라미터 제거

> **변경 파일**: `src/ui/status_bar.rs`, `src/app.rs`

- [ ] RED: `test_status_bar_render_without_toast_param` — render()가 info만 받음 (toast 파라미터 없음)
- [ ] Verify RED: 1개 실패 확인
- [ ] GREEN: StatusBar::render() 시그니처 변경
  ```rust
  // Before
  pub fn render(&self, frame, area, info: &StatusInfo, active_toasts: &[ToastMessage]);
  // After
  pub fn render(&self, frame, area, info: &StatusInfo);
  ```
  - StatusBar 내부 toast 분기 로직 제거 (lines 30-34)
  - `use super::toast::ToastMessage;` import 제거
- [ ] app.rs 수정: StatusBar 호출에서 toast_messages 제거
- [ ] Verify GREEN: 1개 + 전체 회귀 통과

### Step 8: App에서 toast_bar 렌더

> **변경 파일**: `src/app.rs`

- [ ] RED: `test_app_toast_bar_renders_separately` — toast가 status_bar가 아닌 toast_bar 영역에 렌더
- [ ] Verify RED: 1개 실패 확인
- [ ] GREEN: app.rs render() 수정
  ```rust
  let areas = self.layout.calculate(frame.area());
  // ... header, sidebar, content 동일 ...

  // Toast — toast_bar 영역에 별도 렌더
  let toast_messages: Vec<ToastMessage> = self.background_tracker
      .active_toasts().iter()
      .map(|t| { /* 기존 변환 로직 + resource_id: None */ })
      .collect();
  if let Some(toast) = toast_messages.first() {
      toast.render(frame, areas.toast_bar);
  }

  // StatusBar — toast 파라미터 없음
  self.status_bar.render(frame, areas.status_bar, &info);
  ```
- [ ] Verify GREEN: 1개 + 전체 회귀 통과

### Step 9: 전체 검증

- [ ] `cargo clippy -- -D warnings` 통과
- [ ] `cargo test` 전체 통과 (694 기존 + ~21 신규 = ~715)
- [ ] 80x24 레이아웃 영역 합산 검증 (테스트에서)

---

## Test Strategy

| Step | 테스트 수 | 검증 대상 |
|------|-----------|----------|
| 1 | 5 | Theme 시맨틱 스타일 |
| 2 | 2 | Icons 매핑 |
| 3 | 2 | panel_title 포맷 |
| 4 | 2 | key_hint Span |
| 5 | 3 (+6 수정) | LayoutAreas toast_bar, 겹침, body height |
| 6 | 5 (+2 수정) | resource_id, truncation, Theme color |
| 7 | 1 | StatusBar 시그니처 |
| 8 | 1 | App toast_bar 렌더 |
| **합계** | **~21 신규** | 694 + 21 = ~715 |

## Dependency Order

```
Step 1-4: Theme (leaf, 의존성 없음)
  ↓
Step 5: Layout (toast_bar 추가) — app.rs 최소 수정 포함
  ↓
Step 6: Toast (Theme 의존)
  ↓
Step 7: StatusBar (toast 제거)
  ↓
Step 8: App (Layout + Toast + StatusBar 통합)
  ↓
Step 9: 전체 검증
```

## Risk & Mitigation

| 리스크 | 완화 |
|--------|------|
| Step 5에서 LayoutAreas 변경 시 app.rs 컴파일 깨짐 | Step 5 GREEN에서 app.rs 최소 수정 (toast_bar 무시) 동시 진행 |
| 기존 layout 테스트 6개 body height 어설션 불일치 | 4줄 차감으로 일괄 수정 (3→4) |
| toast 생성자 호출부 resource_id 누락 | resource_id: None 기본값으로 기존 호출 호환 |
