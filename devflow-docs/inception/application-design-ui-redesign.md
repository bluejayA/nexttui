# Application Design — UI/UX Redesign Stage 2.5-A

**Mode**: DETAIL (상세 설계, 리뷰 반영)
**Timestamp**: 2026-03-31T17:30:00+09:00
**Review**: 3-agent review 반영 (Layout/UX Flow/Ops Scenario)

## 컴포넌트 목록

### 신규 컴포넌트

| 컴포넌트 | 책임 | 타입 | 파일 |
|---------|------|------|------|
| Theme | 시맨틱 스타일 토큰 중앙 관리 (active/done/error/focus_border 등) | Util | `src/ui/theme.rs` (신규) |
| Icons | 상태별 Unicode 아이콘 (`●✓○✗⟳◐`) 정의 | Util | `src/ui/theme.rs` 내 구조체 |
| NotificationHistory | Toast 이력 저장 + 이력 패널 조회 | Service | `src/ui/notification.rs` (신규) |

### 변경 컴포넌트

| 컴포넌트 | 변경 내용 | 파일 |
|---------|----------|------|
| LayoutManager | Toast 별도 행 추가 (조건부: toast 있을 때만), `LayoutAreas`에 `toast_bar: Option<Rect>` 추가 | `src/ui/layout.rs` |
| Header | Blue 뱃지 → Dim 기반 + `─` 채움선 + 우측 context. Theme 토큰 적용 | `src/ui/header.rs` |
| Sidebar | RIGHT-only 보더 → Rounded 전체 보더. `panel_title()` 적용. Theme 포커스 색상 | `src/ui/sidebar.rs` |
| StatusBar | 투명 bg → `on_dark_gray().white()`. 좌=고정 `[Panel] N/M`, 우=동적 `key_hint()`. Toast 오버라이드 제거 | `src/ui/status_bar.rs` |
| ResourceList | 선택 행 Black on White → White Bold + 시맨틱 컬러 유지. 상태 아이콘 prefix 추가 | `src/ui/resource_list.rs` |
| Toast | `color()` 메서드 → Theme 토큰 적용. 렌더 위치를 status_bar에서 toast_bar로 변경 | `src/ui/toast.rs` |
| DetailView | `-- Section --` 대시 → Bold 섹션 헤더. Theme 토큰 적용 | `src/ui/detail_view.rs` |
| InputBar | 하드코딩 Color → Theme 토큰 | `src/ui/input_bar.rs` |
| FormWidget | 하드코딩 Color → Theme 토큰 | `src/ui/form.rs` |
| ConfirmDialog | 하드코딩 Color → Theme 토큰. Rounded 보더 | `src/ui/confirm.rs` |
| SelectPopup | 하드코딩 Color → Theme 토큰. Rounded 보더 | `src/ui/select_popup.rs` |
| App (draw) | Toast 렌더 위치 변경 (status_bar → toast_bar). Content 영역에 Block 컨테이너 + `panel_title()` 적용 | `src/app.rs` |

### 변경하지 않는 컴포넌트

| 컴포넌트 | 이유 |
|---------|------|
| Component trait | 인터페이스 변경 없음 (제약 조건) |
| Action / AppEvent enum | 변경 없음 (제약 조건) |
| Worker | 도메인 로직 무변경 |
| 각 Module (server/flavor 등) | 렌더링은 ResourceList/DetailView에 위임, 모듈 코드 무변경 |
| Config | 이번 스코프에서 config 변경 없음 (폴링/동시성 config는 Stage 2.5-B) |

총 **15개 컴포넌트** (신규 3 + 변경 12)

---

## 컴포넌트 상세 설계

### 1. Theme (신규)

**Responsibility**: 시맨틱 스타일 토큰 + 아이콘 + 유틸 함수 중앙 관리
**File**: `src/ui/theme.rs`

**Interface**:
```rust
pub struct Theme;
impl Theme {
    pub fn active() -> Style;        // Yellow Bold — 전이 상태 (RESIZE, BUILD). warning()과 구분: Bold 유무
    pub fn done() -> Style;          // Green — 정상/완료 (ACTIVE)
    pub fn error() -> Style;         // Red — 에러
    pub fn waiting() -> Style;       // DarkGray — 비활성 (SHUTOFF)
    pub fn warning() -> Style;       // Yellow — 경고
    pub fn focus_border() -> Style;  // Cyan — 포커스 보더
    pub fn unfocus_border() -> Style;// DarkGray — 비포커스 보더
    pub fn highlight() -> Style;     // Bold modifier만 (fg 없음) — 선택 행의 시맨틱 컬러 유지
                                     // 사용: base_style.patch(Theme::highlight()) → 기존 fg + Bold
    pub fn disabled() -> Style;      // DarkGray Dim — admin 전용
    pub fn link() -> Style;          // Cyan Underline — 리소스 링크
    pub fn timestamp() -> Style;     // Cyan — 시간 정보
}

pub struct Icons;
impl Icons {
    pub fn active() -> &'static str;    // "●"
    pub fn shutoff() -> &'static str;   // "○"
    pub fn error() -> &'static str;     // "✗"
    pub fn building() -> &'static str;  // "⟳"
    pub fn verify() -> &'static str;    // "◐"
    pub fn migrating() -> &'static str; // "↔"
    pub fn status_icon(status: &str) -> &'static str; // status 문자열 → 아이콘 매핑
}

pub fn panel_title(name: &str, focused: bool) -> String;
// focused: "[ Name ]", unfocused: "  Name  "
// ⚠ P0 수정: Sidebar에서는 bracket 제거 — 80col에서 15%=12col, 보더 제외 10자.
//   "[ Modules ]"(11자) 오버플로우. Sidebar는 포커스를 보더 색상으로만 구분.
//   Content 패널은 공간 충분하므로 bracket 유지.

pub fn key_hint<'a>(key: &'a str, desc: &'a str) -> Vec<Span<'a>>;
// key=Cyan Bold, desc=Dim
```

**Dependencies**: 없음 (ratatui Style/Span만 사용)

---

### 2. NotificationHistory (신규)

**Responsibility**: Toast 이력 저장, 이력 패널 데이터 제공
**File**: `src/ui/notification.rs`

**Interface**:
```rust
pub struct NotificationEntry {
    pub message: String,
    pub severity: ToastSeverity,
    pub timestamp: Instant,
}

pub struct NotificationHistory {
    entries: VecDeque<NotificationEntry>,
    max_entries: usize,  // 기본 50
    visible: bool,       // P1 추가: 패널 토글 상태
}
impl NotificationHistory {
    pub fn new(max_entries: usize) -> Self;
    pub fn push(&mut self, message: String, severity: ToastSeverity, resource_id: Option<String>);
    pub fn entries(&self) -> &VecDeque<NotificationEntry>;
    pub fn toggle(&mut self);          // P1 추가: 패널 토글
    pub fn is_visible(&self) -> bool;
    pub fn dismiss_all_errors(&mut self); // P1 추가: 에러 일괄 dismiss
    pub fn render_panel(&self, frame: &mut Frame, area: Rect);
}
```

> **P1 결정 — 키바인딩 및 레이아웃**:
> - `!` 키로 이력 패널 토글 (App 레벨 글로벌 키)
> - 패널은 Content 영역 위 오버레이 (centered_rect 60%x70%)
> - j/k 스크롤, `D` dismiss all errors, `Esc` 닫기
> - 에러 toast 3개 초과 시 toast_bar에 `[ERR] + N more errors (! 이력)` 표시
>
> ```
> ╭─[ Notifications ]─────────────────╮
> │ 16:45:22 [ERR] server-01: Timeout │
> │ 16:45:20 [ERR] server-02: 401     │
> │ 16:45:18 [OK]  server-03: Resized │
> │ 16:45:10 [i]   Loading servers... │
> ╰───────── D:dismiss-all  Esc:닫기 ─╯
> ```

**Dependencies**: `toast::ToastSeverity`, `theme::Theme`

---

### 3. LayoutManager (변경)

**Responsibility**: 화면 영역 분할. Toast 별도 행 추가 (조건부)
**File**: `src/ui/layout.rs`

**Interface 변경**:
```rust
pub struct LayoutAreas {
    pub header: Rect,
    pub sidebar: Option<Rect>,
    pub content: Rect,
    pub input_bar: Rect,
    pub toast_bar: Rect,      // P0 수정: Option 제거 → 항상 1줄 예약 (지터 방지)
    pub status_bar: Rect,
}

impl LayoutManager {
    // 기존: calculate(&self, frame_size: Rect) -> LayoutAreas
    // 변경: has_toast 파라미터 불필요 (항상 할당)
    pub fn calculate(&self, frame_size: Rect) -> LayoutAreas;
}
```

**레이아웃 변경 (ASCII)**:
```
항상 고정 (toast 유무 불문):
+--[ Header ]-----+        1줄
| Sidebar|Content  |       나머지 - 4줄
+--[ Input Bar ]---+       1줄
+--[ Toast Bar ]---+       1줄 (항상 예약, 비어있으면 빈 행)
+--[ Status Bar ]--+       1줄

80x24 기준 공간 예산:
  Header: 1 + Body: 19 + InputBar: 1 + ToastBar: 1 + StatusBar: 1 = 24
  Body 19줄 중 Sidebar/Content 보더 상하 2줄 → 실제 컨텐츠 17줄
```

> **P0 결정**: Toast 행을 `Option`이 아닌 항상 1줄 고정 할당.
> toast 없을 때 빈 행이 1줄 낭비되지만, 동적 할당/해제 시 화면 전체가
> 떨리는 지터 문제를 완전히 제거. 80x24에서 17줄 컨텐츠는 충분.

**Dependencies**: 없음 (ratatui Layout만 사용)

---

### 4. Header (변경)

**Responsibility**: Blue 뱃지 스타일 → Dim 기반 + 채움선 스타일
**File**: `src/ui/header.rs`

**Interface 변경**: `HeaderContext` 구조체 유지, `render()` 시그니처 동일

**렌더링 변경**:
```
Before:  [SERVER] [ALL]              [prod | RegionOne]
         Blue bg   Yellow bg          RGB(60,60,60) bg

After:   nexttui ──────────── admin@prod | RegionOne
         White Bold  DarkGray fill   Cyan (Theme::focus_border)
```

**Dependencies**: `theme::Theme`

---

### 5. Sidebar (변경)

**Responsibility**: RIGHT 보더 → Rounded 전체 보더 + panel_title
**File**: `src/ui/sidebar.rs`

**Interface 변경**: `render()` 시그니처 동일

**렌더링 변경**:
```
Before:                    After:
 > Servers   |            ╭─ Modules ──╮    (focused, Cyan 보더)
   Networks  |            │ > Servers  │
   Volumes   |            │   Networks │
             |            │   Volumes  │
                          ╰────────────╯

                          ╭─ Modules ──╮    (unfocused, DarkGray 보더)
```

> **P0 수정**: Sidebar 타이틀에서 bracket `[ ]` 제거.
> 80col 15%=12col, 보더 2col 제외 시 usable 10자.
> `"Modules"`(7자)는 안전. 포커스는 보더 색상(Cyan/DarkGray)으로 충분히 구분.

**변경 상세**:
- `Borders::RIGHT` → `Borders::ALL`
- `BorderType::default()` → `BorderType::Rounded`
- `.title(" Modules ")` → `.title(" Modules ")` (bracket 없이, 앞뒤 공백만)
- 하드코딩 Color → Theme 호출

**Dependencies**: `theme::Theme`, `theme::panel_title`

---

### 6. StatusBar (변경)

**Responsibility**: 투명 → DarkGray 배경, 좌=고정 컨텍스트, 우=동적 key_hint
**File**: `src/ui/status_bar.rs`

**Interface 변경**:
```rust
// Before
pub fn render(&self, frame, area, info: &StatusInfo, active_toasts: &[ToastMessage]);

// After — toast 파라미터 제거 (Toast는 별도 행에서 렌더)
pub fn render(&self, frame, area, info: &StatusInfo);
```

**렌더링 변경**:
```
Before: Servers | Normal | 1/5          ←→:Navigate q:Quit /:Search
        투명 bg                          key=Cyan label=Gray

After:  [Servers] 1/5           j/k 이동  Enter 상세  c 생성  q 종료
        on_dark_gray bg          key=Cyan Bold  desc=Dim
```

**`StatusInfo` 변경**:
```rust
pub struct StatusInfo {
    pub panel_name: String,      // 변경: 패널명 (Servers, Flavors 등)
    pub item_count: Option<usize>,
    pub selected_index: Option<usize>,
    pub context_hints: Vec<(String, String)>, // 추가: (key, desc) 쌍
    pub bulk_mode: Option<BulkModeInfo>,      // P1 추가: 벌크 액션 진행 상태
}

pub struct BulkModeInfo {
    pub selected_count: usize,
    pub completed_count: usize,
    pub failed_count: usize,
    pub in_flight_count: usize,
}
// bulk_mode가 있을 때: [Servers] 15 selected | 12✓ 2✗ 1⟳
// bulk_mode가 없을 때: [Servers] 1/5 (기본)
```

> **P0 결정 — 힌트 제공 메커니즘**:
> Component trait 변경 없이 `App::render()`에서 힌트를 결정한다.
> ```rust
> // App::render() 내부
> let context_hints = match (self.focus, self.router.current(), view_state) {
>     (_, Route::Servers, ViewState::List) =>
>         vec![("j/k", "이동"), ("Enter", "상세"), ("c", "생성"), ("d", "삭제")],
>     (_, Route::Servers, ViewState::Detail(_)) =>
>         vec![("Esc", "목록"), ("Tab", "링크"), ("r", "Resize"), ("d", "삭제")],
>     (_, _, ViewState::Create) =>
>         vec![("↑↓", "필드"), ("Enter", "제출"), ("Esc", "취소")],
>     _ => vec![("j/k", "이동"), ("Enter", "선택"), ("q", "종료")],
> };
> ```
> 모듈별 힌트가 필요한 경우 기존 `help_hint()` 메서드 반환값을 파싱하여 활용.

**Dependencies**: `theme::Theme`, `theme::key_hint`

---

### 7. ResourceList (변경)

**Responsibility**: 선택 행 스타일 변경 + 상태 아이콘
**File**: `src/ui/resource_list.rs`

**Interface 변경**: `render()` 시그니처 동일

**렌더링 변경**:
```
Before: 선택 행 = fg(Black) bg(White) — 시맨틱 컬러 손실
After:  선택 행 = 기존 fg 유지 + Bold 추가 — 시맨틱 컬러 보존

Before: web-01   ACTIVE    srv-123
After:  web-01   ● ACTIVE  srv-123     (Green + Bold when selected)
        db-01    ✗ ERROR   srv-789     (Red + Bold when selected)
```

**Dependencies**: `theme::Theme`, `theme::Icons`

---

### 8. Toast (변경)

**Responsibility**: Theme 토큰 적용, 렌더 위치는 App이 결정, 리소스 식별자 포함
**File**: `src/ui/toast.rs`

**Interface 변경**:
```rust
pub struct ToastMessage {
    pub text: String,
    pub severity: ToastSeverity,
    pub resource_id: Option<String>,  // P1 추가: 어떤 리소스의 알림인지 식별
}

// render() — toast_bar 영역에 렌더. resource_id가 있으면 메시지에 포함
// 예: "[ERR] server-01: Resize failed — Conflict"
//     "[OK] server-02: Migration completed"
```

> **P1 결정**: toast에 resource_id 필드 추가. 동시 작업(Scenario 3)에서
> 어떤 리소스의 알림인지 즉시 식별 가능.

**Dependencies**: `theme::Theme`

---

### 9. DetailView (변경)

**Responsibility**: 대시 섹션 구분 → Bold 헤더 + 상태별 액션 힌트 섹션
**File**: `src/ui/detail_view.rs`

**렌더링 변경**:
```
Before: -- Basic Info --      After: Basic Info
        DarkGray                     White Bold Underline
```

> **P1 추가 — 상태별 액션 섹션**:
> 리소스가 actionable 상태(VERIFY_RESIZE, VERIFY_REVERT 등)일 때
> 하단에 액션 힌트 섹션을 렌더링한다.
> ```
> Actions (Yellow Bold)
>   y  Confirm resize (m1.small → m1.medium)
>   n  Revert to m1.small
> ```
> 이 섹션은 `DetailSection::Actions` 타입으로 추가하여 기존 `KeyValue`/`NestedTable`/`ResourceLink`와
> 동일한 구조로 관리한다. 모듈에서 서버 상태를 판단하여 섹션 포함 여부를 결정.

**Dependencies**: `theme::Theme`

---

### 10-12. InputBar, FormWidget, ConfirmDialog, SelectPopup (변경)

**Responsibility**: 하드코딩 Color:: → Theme 토큰 교체
**변경 범위**: 색상 참조만 교체, 로직 무변경
- InputBar: 5개 Color:: → Theme 호출
- FormWidget: 29개 Color:: → Theme 호출
- ConfirmDialog: 11개 Color:: → Theme 호출 + `BorderType::Rounded`
- SelectPopup: 17개 Color:: → Theme 호출 + `BorderType::Rounded`

**Dependencies**: `theme::Theme`

---

### 13. App (draw) (변경)

**Responsibility**: Toast 렌더 위치 변경, Content Block + panel_title
**File**: `src/app.rs`

**`render()` 변경 상세**:
```rust
// Before
let areas = self.layout.calculate(frame.area());
self.status_bar.render(frame, areas.status_bar, &info, &toast_messages);

// After (P0 반영: toast 항상 할당, hints App에서 결정)
let areas = self.layout.calculate(frame.area()); // has_toast 파라미터 불필요

// 1. Toast 항상 렌더 (비어있으면 빈 행)
if let Some(toast) = toast_messages.first() {
    toast.render(frame, areas.toast_bar);
} else {
    // 빈 행 — 아무것도 렌더하지 않음 (기본 배경)
}

// 2. 에러 큐 3개 초과 시 overflow 표시
let error_count = toast_messages.iter().filter(|t| t.severity == ToastSeverity::Error).count();
if error_count > 3 {
    // toast_bar에 "+ N more errors (! 이력)" 표시
}

// 3. StatusBar — App에서 context_hints 결정 (P0)
let context_hints = match (self.focus, self.router.current(), view_state) {
    // ... (위 StatusBar 섹션 참조)
};
let info = StatusInfo { panel_name, item_count, selected_index, context_hints, bulk_mode };
self.status_bar.render(frame, areas.status_bar, &info);

// 4. Content Block — panel_title (Content만 bracket 유지)
let title = panel_title(&route_label, content_focused);
let content_block = Block::default()
    .title(title)
    .borders(Borders::ALL)
    .border_type(BorderType::Rounded)
    .border_style(if content_focused { Theme::focus_border() } else { Theme::unfocus_border() });

// 5. Notification History 오버레이 (P1)
if self.notification_history.is_visible() {
    let overlay = centered_rect(60, 70, areas.content);
    frame.render_widget(Clear, overlay);
    self.notification_history.render_panel(frame, overlay);
}
```

**Dependencies**: `theme::Theme`, `theme::panel_title`, `LayoutManager` (has_toast 파라미터)

---

## 화면 구성 비교

### 현재 레이아웃
```
 [SERVER] [ALL]              [prod | RegionOne]     ← Blue 뱃지, RGB 배경
 > Servers   | Name       Status    ID              ← RIGHT border만, 보더 없는 Content
   Networks  | web-01     ACTIVE    srv-123          ← Black on White 선택
   Volumes   | web-02     SHUTOFF   srv-456
             | db-01      ERROR     srv-789
 : command_                                          ← Input bar
 Servers | Normal | 1/5   ←→:Navigate q:Quit         ← 투명 bg, toast 오버라이드
```

### 목표 레이아웃 (리뷰 반영)
```
 nexttui ──────────────────── admin@prod | RegionOne  ← Dim 기반, 채움선
╭─ Modules ──╮╭─[ Servers ]────────────────────────╮ ← Sidebar: bracket 없음, Content: bracket 있음
│ > Servers  ││ Name       Status    ID            │
│   Networks ││ web-01     ● ACTIVE  srv-123       │ ← 아이콘 + 시맨틱 컬러 유지
│   Volumes  ││ web-02     ○ SHUTOFF srv-456       │
│            ││ db-01      ✗ ERROR   srv-789       │
╰────────────╯╰────────────────────────────────────╯
 / search_                                            ← Input bar
 [OK] server-01: Resize confirmed                     ← Toast 행 (항상 예약, 비면 빈 행)
 [Servers] 1/3       j/k 이동  Enter 상세  c 생성  q 종료  ← DarkGray bg, 항상 표시
```

### 목표 레이아웃 — Bulk Action Mode
```
 nexttui ──────────────────── admin@prod | RegionOne
╭─ Modules ──╮╭─[ Servers ]────────────────────────╮
│ > Servers  ││ [x] web-01  ● ACTIVE  srv-123      │ ← 체크박스 (Stage 2.5-B)
│   Networks ││ [x] web-02  ○ SHUTOFF srv-456      │
│   Volumes  ││ [ ] db-01   ✗ ERROR   srv-789      │
╰────────────╯╰────────────────────────────────────╯
 / search_
 [ERR] web-02: Stop failed — Conflict  + 1 more (! 이력) ← 에러 큐 표시
 [Servers] 2 selected | 1✓ 1✗        Space 선택  d 삭제  ! 이력  ← Bulk 상태바
```

### 목표 레이아웃 — Detail View (VERIFY_RESIZE)
```
 nexttui ──────────────────── admin@prod | RegionOne
╭─ Modules ──╮╭─[ Server: web-01 ]────────────────╮
│ > Servers  ││ Basic Info                         │
│   Networks ││   ID      srv-12345                │
│   Volumes  ││   Status  ◐ VERIFY_RESIZE          │ ← Yellow
│            ││   Flavor  m1.small → m1.medium     │ ← 변경 전→후 표시
│            ││                                    │
│            ││ Actions                            │ ← P1: 상태별 액션 섹션
│            ││   y  Confirm resize                │
│            ││   n  Revert to m1.small            │
╰────────────╯╰────────────────────────────────────╯
 / _
 [i] server-01: Resize awaiting confirmation          ← Toast
 [Server Detail]       Esc 목록  y Confirm  n Revert  ! 이력  ← 상태 인지 힌트
```

### 목표 레이아웃 — Notification History 오버레이
```
 nexttui ──────────────────── admin@prod | RegionOne
╭─ Modules ──╮╭─[ Servers ]────────────────────────╮
│ > Servers  ││  ╭─[ Notifications ]───────────╮   │
│   Networks ││  │ 16:45 [ERR] srv-01: Timeout │   │
│   Volumes  ││  │ 16:44 [ERR] srv-02: 401     │   │
│            ││  │ 16:43 [OK]  srv-03: Resized │   │
│            ││  │ 16:42 [i]   Loading...      │   │
│            ││  ╰──── j/k D:dismiss-all Esc ──╯   │
╰────────────╯╰────────────────────────────────────╯
```

### Detail View 비교
```
현재:                              목표:
 -- Basic Info --                  Basic Info
   ID: srv-123                       ID      srv-123
   Status: ACTIVE                    Status  ● ACTIVE (Green)
                                     Flavor  m1.small
 -- Network --
   eth0  192.168.1.10              Network
                                     eth0    192.168.1.10
 -- Links --
   Image: [Ubuntu 22.04]           Links
                                     Image   Ubuntu 22.04 (Cyan, underline)
```

---

## 의존성 방향 다이어그램

```
theme.rs (신규, 의존성 없음)
  ^
  |--- header.rs
  |--- sidebar.rs
  |--- status_bar.rs
  |--- resource_list.rs
  |--- detail_view.rs
  |--- toast.rs
  |--- input_bar.rs
  |--- form.rs
  |--- confirm.rs
  |--- select_popup.rs
  |--- app.rs

notification.rs (신규)
  ^--- toast.rs (ToastSeverity)
  ^--- theme.rs (렌더링)

layout.rs (변경)
  ^--- app.rs (has_toast 전달)
```

**핵심**: `theme.rs`는 **leaf 모듈** — 다른 모듈에 의존하지 않으므로 가장 먼저 구현 가능

---

## 3-Agent Review 반영 요약

### P0 — 즉시 반영 (설계 변경 완료)
| # | 이슈 | 변경 |
|---|------|------|
| 1 | Sidebar 타이틀 오버플로우 | `[ Modules ]` → `Modules` (bracket 제거, 보더 색상으로 포커스 구분) |
| 2 | Toast 지터 | `Option<Rect>` → 항상 1줄 예약 (빈 행 허용, 지터 완전 제거) |
| 3 | 상태바 힌트 미정의 | App::render()에서 (FocusPane, Route, ViewState) 매칭으로 결정 |

### P1 — 확장점 마련 (인터페이스 설계, 구현은 후속)
| # | 이슈 | 변경 |
|---|------|------|
| 4 | Toast에 리소스 식별자 없음 | `ToastMessage.resource_id: Option<String>` 추가 |
| 5 | 벌크 액션 상태 없음 | `StatusInfo.bulk_mode: Option<BulkModeInfo>` 예약 |
| 6 | DetailView 액션 힌트 없음 | `DetailSection::Actions` 타입 + VERIFY_RESIZE 액션 섹션 |
| 7 | NotificationHistory 미정의 | `!` 키 토글, centered_rect 오버레이, dismiss_all, 에러 큐 overflow 표시 |

### P2 — 백로그 이관 (이번 범위 밖)
- 프로그레스 바 위젯 (Migration 진행률)
- Nested detail 위젯 (Hypervisor→서버 목록)
- 멀티셀렉트 체크박스 UI + BulkAction dispatcher
- 백그라운드 작업 카운터 (헤더 뱃지)

---

## 화면 구성 추가 리뷰 (Screen Layout + Visual Consistency)

### 반영된 설계 규칙

| # | 규칙 | 적용 대상 |
|---|------|----------|
| 1 | `Theme::highlight()`는 Bold modifier만 반환 (fg 색상 없음). 시맨틱 컬러 유지를 위해 `base_style.patch(highlight())` 패턴 사용 | ResourceList 선택 행 |
| 2 | `active()` = Yellow **Bold**, `warning()` = Yellow (Bold 없음). Bold 유무로 전이 상태 vs 경고 구분 | Theme 전체 |
| 3 | NotificationHistory 오버레이: `centered_rect(55, 60)` + max 12행 캡. 80x24에서 ~43x14, 주변 컨텍스트 3~4줄 보존 | notification.rs |
| 4 | DetailView 스크롤: 17행 초과 시 j/k 스크롤 가능. 스크롤 위치는 뷰 전환 시 보존 | detail_view.rs |
| 5 | Toast 텍스트: 75자 초과 시 `…` 말줄임 truncation. resource_id 포함 메시지도 75자 제한 적용 | toast.rs |
| 6 | ConfirmDialog/SelectPopup: `BorderType::Rounded`, bracket 타이틀 없음 (모달은 bracket 불필요) | confirm.rs, select_popup.rs |

### 확인 완료 (수정 불필요)
- Toast 항상 예약 빈 행: 의도적 trade-off ✓
- Sidebar/Content bracket 비대칭: 공간 제약 정당화 ✓
- Bulk 상태바 80col 적합: 49자 사용 / 80자 가용 ✓
- 포커스 색상 충돌 없음: FocusPane enum으로 하나만 포커스 ✓
