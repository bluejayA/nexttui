# Code Generation Plan: Unit 3 — Sidebar + Header + StatusBar 리디자인

> **For agentic workers:** REQUIRED: Use `aidlc:aidlc-code-generation` with the
> "GENERATE" signal to execute this plan. Do NOT implement ad-hoc.
> `"code-generation: GENERATE — proceed with the approved plan for unit-3-frame-redesign"`

## Baseline

- 테스트: 717 passed
- 브랜치: `feature/ui-redesign-unit3` (main에서 분기)

## Files to Modify

- [ ] `src/ui/sidebar.rs` — Rounded 전체 보더, Theme 토큰, 포커스 색상
- [ ] `src/ui/header.rs` — Dim 기반 + 채움선 + 우측 context
- [ ] `src/ui/status_bar.rs` — DarkGray 배경, StatusInfo 구조체 변경, key_hint() 사용
- [ ] `src/app.rs` — StatusInfo 새 구조체 빌드, context_hints 결정

---

## Implementation Steps

### Step 1: Sidebar — Rounded 보더 + Theme 적용

> **변경 파일**: `src/ui/sidebar.rs`
> **하드코딩 Color 8개 → Theme 호출**

- [ ] RED: `test_sidebar_border_is_rounded` — Block의 border_type이 BorderType::Rounded
- [ ] RED: `test_sidebar_border_is_all_sides` — Borders::ALL 사용 확인
- [ ] RED: `test_sidebar_focused_border_uses_theme` — 포커스 시 Theme::focus_border() 색상
- [ ] RED: `test_sidebar_title_no_bracket` — 타이틀이 `" Modules "` (bracket 없음)
- [ ] Verify RED: 4개 실패 확인
- [ ] GREEN: sidebar.rs render() 수정
  - `Borders::RIGHT` → `Borders::ALL`
  - `BorderType` 추가: `BorderType::Rounded`
  - `.title(" Modules ")` 유지 (bracket 없음, P0 결정)
  - 하드코딩 Color:: 8개 → Theme 호출:
    - 선택+포커스: `Theme::highlight()` (Bold) + `Theme::focus_border()` bg
    - 선택+비포커스: `Theme::highlight()` (Bold) + unfocus bg
    - Admin-only: `Theme::disabled()`
    - 기본: `Style::default().fg(Color::White)` → 유지 (Theme에 "normal" 없음)
    - 보더 포커스: `Theme::focus_border()`
    - 보더 비포커스: `Theme::unfocus_border()`
- [ ] 기존 sidebar 테스트 통과 확인 (handle_key 등)
- [ ] Verify GREEN: 4개 신규 + 기존 회귀 통과

### Step 2: Header — Dim 기반 + 채움선

> **변경 파일**: `src/ui/header.rs`
> **하드코딩 Color 7개 → Theme + 새 레이아웃**

- [ ] RED: `test_header_app_name_is_white_bold` — 좌측에 "nexttui" White Bold 표시
- [ ] RED: `test_header_fill_char_is_dash` — 앱명과 context 사이를 `─` 문자로 채움
- [ ] RED: `test_header_context_format` — 우측에 `"user@cloud | Region"` 포맷
- [ ] Verify RED: 3개 실패 확인
- [ ] GREEN: header.rs render() 전면 재작성
  ```
  Before:  [SERVER] [ALL]              [prod | RegionOne]
           Blue bg   Yellow bg          RGB(60,60,60) bg

  After:   nexttui ──────────── admin@prod | RegionOne
           White Bold  DarkGray fill   Dim
  ```
  - Badge 렌더링 제거 → 단순 Span 기반 라인
  - 좌측: `Span::styled("nexttui", Theme::highlight())` (White Bold)
  - 중앙: `─` 반복으로 채움 (DarkGray/Dim)
  - 우측: `Span::styled(context, Style::default().add_modifier(Modifier::DIM))`
  - all_tenants 표시: context에 `"[ALL] "` prefix 추가
  - 배경: 투명 (DarkGray bg 제거)
- [ ] 기존 header 테스트 (있으면) 수정
- [ ] Verify GREEN: 3개 신규 + 전체 회귀 통과

### Step 3: StatusBar — DarkGray 배경 + StatusInfo 변경 + key_hint()

> **변경 파일**: `src/ui/status_bar.rs`
> **하드코딩 Color 5개 → Theme, StatusInfo 구조체 변경**

- [ ] RED: `test_status_bar_has_dark_gray_bg` — 배경이 on_dark_gray().white()
- [ ] RED: `test_status_info_new_fields` — StatusInfo에 panel_name, context_hints 필드
- [ ] RED: `test_status_bar_uses_key_hint` — key 스타일이 theme::key_hint() 패턴과 일치
- [ ] RED: `test_status_bar_left_format` — 좌측이 `"[Servers] 1/5"` 포맷
- [ ] Verify RED: 4개 실패 확인
- [ ] GREEN: status_bar.rs 수정
  - `StatusInfo` 구조체 변경:
    ```rust
    pub struct StatusInfo {
        pub panel_name: String,
        pub item_count: Option<usize>,
        pub selected_index: Option<usize>,
        pub context_hints: Vec<(String, String)>, // (key, desc) 쌍
    }
    ```
  - `style_hint()` 제거 → `theme::key_hint()` 사용
  - 배경: `Style::default().bg(Color::DarkGray).fg(Color::White)`
  - 좌측: `format!("[{}] {}/{}", panel_name, idx+1, count)`
  - 우측: context_hints를 key_hint()로 변환하여 렌더
  - 하드코딩 Color 5개 제거
- [ ] 기존 `test_status_info_creation` 수정 (필드 변경 반영)
- [ ] Verify GREEN: 4개 신규 + 전체 회귀 통과

### Step 4: App — StatusInfo 빌드 + context_hints

> **변경 파일**: `src/app.rs`
> **StatusInfo 새 구조체 빌드, context_hints 매칭**

- [ ] RED: `test_app_status_info_has_panel_name` — StatusInfo에 panel_name이 route_label
- [ ] Verify RED: 1개 실패 확인
- [ ] GREEN: app.rs render() 내 StatusInfo 빌드 수정
  ```rust
  let context_hints = match (self.focus, &self.router.current(), &view_state) {
      (_, Route::Servers, _) =>
          vec![("j/k".into(), "이동".into()), ("Enter".into(), "상세".into()),
               ("c".into(), "생성".into()), ("q".into(), "종료".into())],
      _ => vec![("j/k".into(), "이동".into()), ("Enter".into(), "선택".into()),
                ("q".into(), "종료".into())],
  };
  let info = StatusInfo {
      panel_name: route_label.to_string(),
      item_count: None,
      selected_index: None,
      context_hints,
  };
  ```
  - 기존 `message`, `help_hint` 필드 제거
- [ ] 기존 app 테스트 수정 (StatusInfo 필드 변경 반영)
- [ ] Verify GREEN: 1개 신규 + 전체 회귀 통과

### Step 5: 전체 검증

- [ ] `cargo clippy -- -D warnings` 변경 파일 에러 없음
- [ ] `cargo test` 전체 통과 (717 기존 + ~12 신규 = ~729)

---

## Test Strategy

| Step | 테스트 수 | 검증 대상 |
|------|-----------|----------|
| 1 | 4 | Sidebar 보더/타이틀/포커스 Theme |
| 2 | 3 | Header 앱명/채움선/context |
| 3 | 4 (+1 수정) | StatusBar 배경/StatusInfo/key_hint/포맷 |
| 4 | 1 (+N 수정) | App StatusInfo 빌드 |
| **합계** | **~12 신규** | 717 + 12 = ~729 |

## Dependency Order

```
Step 1: Sidebar (Theme 의존)
Step 2: Header (Theme 의존)  ← Step 1과 독립, 병렬 가능
Step 3: StatusBar (Theme + key_hint 의존)
Step 4: App (StatusInfo 변경 의존)
Step 5: 전체 검증
```

## Risk & Mitigation

| 리스크 | 완화 |
|--------|------|
| StatusInfo 필드 변경 시 app.rs 컴파일 깨짐 | Step 3-4를 연속 진행하여 한 번에 해결 |
| Header 전면 재작성으로 기존 테스트 깨짐 | Header 테스트가 없으면 신규 테스트로 커버 |
| style_hint() 제거 시 help_hint 파싱 로직 손실 | theme::key_hint()로 대체, context_hints (key, desc) 쌍으로 명확화 |
