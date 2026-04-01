# Code Generation Plan: Unit 4 — ResourceList + DetailView + 팝업 Theme 적용

> **For agentic workers:** REQUIRED: Use `aidlc:aidlc-code-generation` with the
> "GENERATE" signal to execute this plan. Do NOT implement ad-hoc.

## Baseline

- 테스트: 726 passed
- 브랜치: `feature/ui-redesign-unit4` (main에서 분기)
- 대상: 6개 파일, 81개 Color:: → Theme 참조

## Files to Modify

- [ ] `src/ui/resource_list.rs` — 선택행 시맨틱 컬러 유지 + Bold, 상태 아이콘 (10 Color)
- [ ] `src/ui/detail_view.rs` — 섹션 헤더 Bold Underline, Theme 적용 (8 Color)
- [ ] `src/ui/input_bar.rs` — Theme 적용 (5 Color)
- [ ] `src/ui/form.rs` — Theme 적용 (29 Color)
- [ ] `src/ui/confirm.rs` — Theme 적용 + BorderType::Rounded (11 Color)
- [ ] `src/ui/select_popup.rs` — Theme 적용 + BorderType::Rounded (18 Color)

---

## Implementation Steps

### Step 1: ResourceList — 선택행 시맨틱 + 상태 아이콘

> 핵심 로직 변경. TDD 엄격 적용.

- [ ] RED: `test_selection_preserves_semantic_color` — 선택 행에서 Green(ACTIVE) fg가 유지되고 Bold 추가
- [ ] RED: `test_selection_preserves_error_color` — 선택 행에서 Red(ERROR) fg 유지
- [ ] RED: `test_row_style_uses_theme_tokens` — RowStyleHint::Active가 Theme::done() 색상 사용
- [ ] Verify RED: 3개 실패
- [ ] GREEN: resource_list.rs 수정
  - RowStyleHint 색상 매핑 → Theme 토큰:
    - Normal: White (유지), Active: `Theme::done()`, Error: `Theme::error()`,
      Warning: `Theme::warning()`, Disabled: `Theme::disabled()`
  - 선택 행: `fg(Color::Black).bg(Color::White)` → `base_style.patch(Theme::highlight())`
  - Loading/Empty: `Theme::active()` / `Theme::waiting()`
  - Header: White Bold Underline (유지)
- [ ] Verify GREEN: 3개 + 기존 10개 회귀 통과

### Step 2: DetailView — 섹션 헤더 + Theme

> 핵심 로직 변경 (섹션 구분 스타일).

- [ ] RED: `test_section_header_style` — 섹션 헤더가 Bold + Underline (Cyan 아닌 White Bold)
- [ ] RED: `test_resource_link_uses_theme_link` — ResourceLink가 Theme::link() 스타일
- [ ] Verify RED: 2개 실패
- [ ] GREEN: detail_view.rs 수정
  - `"-- {} --"` 포맷 → `"{}"` (대시 제거)
  - 섹션 헤더: `Color::Cyan + Bold` → `Style::default().fg(Color::White).add_modifier(Bold | Underline)`
  - ResourceLink: `Color::Cyan + Bold + Underline` → `Theme::link()`
  - Loading: `Theme::active()`, Title: `Theme::highlight()`
  - Key label: `Color::LightBlue` → `Theme::focus_border()` (Cyan)
  - Nested label: `Color::Gray` → `Theme::waiting()`
- [ ] Verify GREEN: 2개 + 기존 5개 회귀 통과

### Step 3: InputBar — Theme 적용

> 단순 Color 교체 (5개).

- [ ] GREEN 직접: input_bar.rs Color 교체
  - Normal hint: `Theme::disabled()`
  - Command `:`: `Theme::warning()`
  - Search `/`: `Theme::focus_border()`
  - Input text: White (유지)
  - Cursor: `Theme::waiting()`
- [ ] Verify: 기존 7개 테스트 회귀 통과

### Step 4: FormWidget — Theme 적용

> 대량 Color 교체 (29개). 로직 무변경.

- [ ] GREEN 직접: form.rs Color 교체
  - 보더: `Color::Cyan` → `Theme::focus_border()`
  - 포커스 라벨: `Color::Cyan` → `Theme::focus_border()`
  - 비포커스 라벨: `Color::Gray` → `Theme::waiting()`
  - 에러: `Color::Red` → `Theme::error()`
  - 힌트: `Color::DarkGray` → `Theme::disabled()`
  - 커서: Black-on-White (유지, 표준 텍스트 커서)
  - 드롭다운/멀티셀렉트 선택: `Color::Cyan` → `Theme::focus_border()`
  - Confirm view 보더: `Color::Yellow` → `Theme::warning()`
  - Confirm view 라벨/값: Theme 토큰
- [ ] Verify: 기존 72개 테스트 회귀 통과

### Step 5: ConfirmDialog + SelectPopup — Theme + Rounded

- [ ] RED: `test_confirm_dialog_uses_theme_warning_border` — 보더 스타일이 Theme::warning()
- [ ] RED: `test_select_popup_hint_uses_key_hint` — 힌트가 theme::key_hint() 패턴
- [ ] Verify RED: 2개 실패
- [ ] GREEN: confirm.rs + select_popup.rs 수정
  - 보더: `Color::Yellow + Bold` → `Theme::warning().add_modifier(Bold)`
  - `BorderType::Rounded` 추가
  - Modal bg: `Color::Rgb(30,30,40)` → 유지 (Theme에 modal_bg 없음)
  - confirm.rs: 버튼 키 `Theme::focus_border()`, 텍스트 White
  - select_popup.rs: 선택 `Theme::focus_border()`, 경고 `Theme::warning()`,
    비활성 `Theme::disabled()`, 힌트 `theme::key_hint()` 사용
- [ ] Verify GREEN: 2개 + 기존 15개 회귀 통과

### Step 6: 전체 검증

- [ ] `cargo clippy -- -D warnings` 변경 파일 에러 없음
- [ ] `cargo test` 전체 통과 (726 + ~7 = ~733)

---

## Test Strategy

| Step | 신규 테스트 | 기존 테스트 | 검증 대상 |
|------|-----------|-----------|----------|
| 1 | 3 | 10 | 선택행 시맨틱, Theme 토큰 |
| 2 | 2 | 5 | 섹션 헤더, ResourceLink |
| 3 | 0 | 7 | 회귀만 |
| 4 | 0 | 72 | 회귀만 |
| 5 | 2 | 15 | 보더, 힌트 |
| **합계** | **~7** | **109** | 726 + 7 = ~733 |

## Risk & Mitigation

| 리스크 | 완화 |
|--------|------|
| form.rs 29개 Color 교체 시 실수 | 기존 72개 테스트가 회귀 방어 |
| RowStyleHint 색상 변경 시 모듈별 영향 | 모듈은 RowStyleHint enum만 전달, 색상은 resource_list에서 결정 |
| Modal bg RGB 값이 Theme에 없음 | RGB(30,30,40) 유지 (Theme 확장은 Stage 2.5-B) |
