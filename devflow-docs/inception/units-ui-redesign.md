# Units — UI/UX Redesign Stage 2.5-A

**Timestamp**: 2026-03-31T18:00:00+09:00
**Source**: application-design-ui-redesign.md
**Total Units**: 5

## 의존성 그래프

```
Unit 1: Theme 시스템
  ↓
Unit 2: LayoutManager + Toast 위치 변경
  ↓
Unit 3: Sidebar + Header + StatusBar 리디자인
  ↓
Unit 4: ResourceList + DetailView + 팝업 Theme 적용
  ↓
Unit 5: App 통합 + NotificationHistory + 전체 검증
```

## Units

---

### Unit 1: Theme 시스템 도입

**Responsibility**: `src/ui/theme.rs` 신규 생성 — Theme, Icons, panel_title(), key_hint() 구현
**Dependencies**: none (leaf 모듈)
**Implementation order**: 1

**Scope**:
- `Theme` 구조체: active(), done(), error(), waiting(), warning(), focus_border(), unfocus_border(), highlight() (Bold only), disabled(), link(), timestamp()
- `Icons` 구조체: active(●), shutoff(○), error(✗), building(⟳), verify(◐), migrating(↔), status_icon(status) 매핑
- `panel_title(name, focused)`: focused → `[ Name ]`, unfocused → `  Name  `
- `key_hint(key, desc) -> Vec<Span>`: key=Cyan Bold, desc=Dim
- `src/ui/mod.rs`에 `pub mod theme;` 추가

**Testable in isolation**: Theme 메서드가 올바른 Style/Span 반환하는지 단위 테스트. Icons::status_icon() 매핑 테스트. panel_title 포맷 테스트.

**Interfaces exposed**:
- `Theme::*()` — 모든 UI 컴포넌트에서 사용
- `Icons::status_icon()` — ResourceList에서 사용
- `panel_title()` — Sidebar, App(Content Block)에서 사용
- `key_hint()` — StatusBar에서 사용

**Expected tests**: ~15개

---

### Unit 2: LayoutManager + Toast 위치 변경

**Responsibility**: LayoutManager에 toast_bar 항상 1줄 예약. ToastMessage에 resource_id 추가.
**Dependencies**: Unit 1 (Theme — toast.rs에서 Theme 사용)
**Implementation order**: 2

**Scope**:
- `LayoutAreas`에 `toast_bar: Rect` 추가 (Option 아닌 항상 할당)
- `LayoutManager::calculate()` 수정: Header(1) + Body(나머지-3) + InputBar(1) + ToastBar(1) + StatusBar(1)
- `ToastMessage`에 `resource_id: Option<String>` 필드 추가
- `ToastMessage::color()` → Theme 토큰 사용
- Toast 텍스트 75자 truncation 로직
- 기존 `calculate()` 호출부 (`app.rs`) 시그니처 변경 대응

**Testable in isolation**: LayoutAreas에 toast_bar 존재 확인. 80x24에서 영역 겹침 없음 검증. Toast truncation 테스트. resource_id 포함 메시지 포맷 테스트.

**Interfaces exposed**:
- `LayoutAreas.toast_bar: Rect` — App에서 toast 렌더 위치
- `ToastMessage.resource_id` — App에서 메시지 생성 시

**Expected tests**: ~10개

---

### Unit 3: Sidebar + Header + StatusBar 리디자인

**Responsibility**: 3개 주요 프레임 컴포넌트를 Theme 기반으로 리디자인
**Dependencies**: Unit 1 (Theme), Unit 2 (LayoutManager — toast 분리)
**Implementation order**: 3

**Scope**:
- **Sidebar**: Borders::RIGHT → Borders::ALL + BorderType::Rounded. 타이틀 bracket 없이 ` Modules `. 하드코딩 Color:: → Theme 호출 (8개). 포커스 보더 Theme::focus_border()/unfocus_border()
- **Header**: Blue 뱃지 → Dim 기반 + `─` 채움선 + 우측 context. 하드코딩 Color:: → Theme (7개)
- **StatusBar**: 투명 → on_dark_gray().white(). toast 파라미터 제거. StatusInfo 구조체 변경 (panel_name, context_hints, bulk_mode). key_hint() 사용. 하드코딩 Color:: → Theme (5개)

**Testable in isolation**: Sidebar 보더 타입/스타일 검증. Header 렌더 출력 검증. StatusBar 새 포맷 검증. 기존 sidebar handle_key 테스트 통과 확인.

**Interfaces exposed**:
- `StatusInfo` 새 구조체 — App에서 생성
- `StatusBar::render()` 새 시그니처 (toast 파라미터 없음)

**Expected tests**: ~12개

---

### Unit 4: ResourceList + DetailView + 팝업 Theme 적용

**Responsibility**: Content 영역 컴포넌트들의 Theme 적용 + 시맨틱 컬러 유지 하이라이트
**Dependencies**: Unit 1 (Theme, Icons)
**Implementation order**: 4 (Unit 3과 병렬 가능)

**Scope**:
- **ResourceList**: 선택 행 Black-on-White → 시맨틱 fg 유지 + Bold. 상태 아이콘 prefix (Icons::status_icon()). 하드코딩 Color:: → Theme (11개)
- **DetailView**: `-- Section --` → Bold Underline 헤더. 상태별 Actions 섹션 구조 예약 (DetailSection::Actions). 스크롤 17행 초과 시 j/k. 하드코딩 Color:: → Theme (8개)
- **InputBar**: 하드코딩 Color:: → Theme (5개)
- **FormWidget**: 하드코딩 Color:: → Theme (29개)
- **ConfirmDialog**: 하드코딩 Color:: → Theme (11개) + BorderType::Rounded, bracket 없음
- **SelectPopup**: 하드코딩 Color:: → Theme (17개) + BorderType::Rounded, bracket 없음

**Testable in isolation**: ResourceList 선택 행 시맨틱 컬러 보존 테스트. 아이콘 prefix 테스트. DetailView 섹션 헤더 스타일 테스트. 팝업 BorderType 테스트.

**Interfaces exposed**:
- ResourceList의 변경된 row_style() — 기존 모듈에서 투명하게 사용

**Expected tests**: ~15개

---

### Unit 5: App 통합 + NotificationHistory + 전체 검증

**Responsibility**: App::render() 통합, NotificationHistory 신규 모듈, 전체 회귀 테스트
**Dependencies**: Unit 1, 2, 3, 4 (모두)
**Implementation order**: 5

**Scope**:
- **App::render()**: toast_bar 렌더, StatusBar toast 파라미터 제거, Content Block에 panel_title + Rounded 보더, context_hints App에서 (FocusPane, Route, ViewState) 매칭으로 생성
- **NotificationHistory**: 신규 `src/ui/notification.rs`. push/entries/toggle/dismiss_all_errors/render_panel. `!` 키 토글 (App handle_key). centered_rect(55,60) 오버레이. max 12행. 에러 큐 3개 초과 시 overflow 표시
- **전체 검증**: cargo test 전체 (691+ 테스트) 회귀 없음 확인. 80x24 레이아웃 검증.
- app.rs 하드코딩 Color:: → Theme (2개)

**Testable in isolation**: App render 시 toast_bar 영역 사용 테스트. NotificationHistory push/toggle/dismiss 테스트. context_hints 매칭 테스트. 전체 691 테스트 회귀.

**Interfaces exposed**: (최종 통합 — 외부 노출 없음)

**Expected tests**: ~10개 (+ 전체 회귀)

---

## 구현 순서 요약

```
Unit 1 (Theme)          — leaf, 의존성 없음
  ↓
Unit 2 (Layout+Toast)   — Theme 의존
  ↓
Unit 3 (Sidebar+Header+StatusBar) ─┐  Unit 1,2 의존
Unit 4 (ResourceList+Detail+팝업)  ─┤  Unit 1 의존 (Unit 3과 병렬 가능)
  ↓                                 ↓
Unit 5 (App 통합 + NotificationHistory + 전체 검증)

예상 테스트: ~62개 추가 (기존 691 + 62 = ~753)
```
