# Detail Design: UI Widgets & Input/Navigation Components

**Timestamp**: 2026-03-23T15:00:00+09:00
**Scope**: UI Widgets (10) + Input/Navigation (3) = 13 components
**References**: application-design.md, async-event-architecture-design.md, requirements.md (FR-02, FR-03, FR-11)

---

## Table of Contents

1. [LayoutManager](#1-layoutmanager)
2. [Header](#2-header)
3. [Sidebar](#3-sidebar)
4. [InputBar](#4-inputbar)
5. [StatusBar](#5-statusbar)
6. [ResourceList](#6-resourcelist)
7. [DetailView](#7-detailview)
8. [FormWidget](#8-formwidget)
9. [ConfirmDialog](#9-confirmdialog)
10. [Toast](#10-toast)
11. [CommandParser](#11-commandparser)
12. [SearchFilter](#12-searchfilter)
13. [KeyMap](#13-keymap)
14. [End-to-End Interaction Diagram](#14-end-to-end-interaction-diagram)

---

## 1. LayoutManager

**Responsibility**: 메인 레이아웃 영역(Header/Sidebar/Content/InputBar/StatusBar) 크기 계산 및 터미널 리사이즈 대응.

### Interface

```rust
pub struct LayoutManager {
    sidebar_visible: bool,
    terminal_size: Size,
}

pub struct LayoutAreas {
    pub header: Rect,
    pub sidebar: Option<Rect>,
    pub content: Rect,
    pub input_bar: Rect,
    pub status_bar: Rect,
}

pub struct Size {
    pub width: u16,
    pub height: u16,
}

impl LayoutManager {
    /// 새 LayoutManager 생성 (사이드바 기본 ON)
    pub fn new() -> Self;

    /// 현재 프레임 크기로 전체 영역 계산.
    /// ratatui Layout + Constraint 사용.
    pub fn calculate(&self, frame_size: Rect) -> LayoutAreas;

    /// 사이드바 토글 (Tab 키). OFF 시 content가 전체 폭 차지.
    pub fn toggle_sidebar(&mut self);

    /// 사이드바 가시성 조회
    pub fn is_sidebar_visible(&self) -> bool;

    /// 터미널 리사이즈 이벤트 처리.
    /// EventLoop의 crossterm::event::Event::Resize에서 호출.
    pub fn on_resize(&mut self, width: u16, height: u16);

    /// 최소 터미널 크기 충족 여부 (80x24)
    pub fn is_minimum_size(&self) -> bool;
}
```

### Dependencies

| Component | Relationship |
|-----------|-------------|
| `App` | App이 소유, `render()` 시 `calculate()` 호출 |
| ratatui `Layout`, `Constraint`, `Direction` | 레이아웃 계산 |

### Data Owned

```rust
struct LayoutManager {
    sidebar_visible: bool,       // 사이드바 ON/OFF
    terminal_size: Size,         // 현재 터미널 크기
    min_size: Size,              // 최소 크기 (80x24)
    sidebar_width_percent: u16,  // 사이드바 폭 비율 (기본 15%)
}
```

### Interactions

```
  Terminal Resize Event
        |
        v
  EventLoop (crossterm::Event::Resize)
        |
        v
  App.on_resize(w, h)
        |
        v
  LayoutManager.on_resize(w, h)
        |
        (다음 render 사이클)
        v
  App.render(frame)
        |
        v
  LayoutManager.calculate(frame.area())
        |
        v
  LayoutAreas { header, sidebar?, content, input_bar, status_bar }
        |
        +---> Header.render(areas.header)
        +---> Sidebar.render(areas.sidebar)   // None이면 skip
        +---> ActiveComponent.render(areas.content)
        +---> InputBar.render(areas.input_bar)
        +---> StatusBar.render(areas.status_bar)
```

### Layout Constraint Detail

```
Vertical split (전체 화면):
  ┌─────────────────────────────┐
  │ Header:   Min(1)            │  -- 고정 1줄
  ├─────────────────────────────┤
  │ Body:     Min(0) [나머지]    │  -- 가변
  ├─────────────────────────────┤
  │ InputBar: Min(1)            │  -- 고정 1줄
  ├─────────────────────────────┤
  │ StatusBar: Min(1)           │  -- 고정 1줄
  └─────────────────────────────┘

Body horizontal split (sidebar ON):
  ┌──────────┬──────────────────┐
  │ Sidebar  │  Content         │
  │ Pct(15)  │  Min(0)          │
  └──────────┴──────────────────┘

Body horizontal split (sidebar OFF):
  ┌─────────────────────────────┐
  │ Content: Min(0) [100%]      │
  └─────────────────────────────┘
```

---

## 2. Header

**Responsibility**: 상단 1줄 바 -- 현재 리소스 타입명, 클라우드명, 리전 표시.

### Interface

```rust
pub struct HeaderContext {
    pub resource_type: String,   // e.g. "Servers", "Networks"
    pub cloud_name: String,      // e.g. "prod-cloud"
    pub region: String,          // e.g. "RegionOne"
}

pub struct Header;

impl Header {
    pub fn new() -> Self;

    /// ratatui Frame에 헤더 렌더링.
    /// 좌측: 리소스 타입, 우측: cloud_name | region
    pub fn render(&self, frame: &mut Frame, area: Rect, ctx: &HeaderContext);
}
```

### Dependencies

| Component | Relationship |
|-----------|-------------|
| `App` | HeaderContext 데이터 제공 |
| `Router` | 현재 Route에서 resource_type 결정 |
| `Config` | cloud_name, region 제공 |

### Data Owned

```rust
// Header는 상태를 소유하지 않음 (stateless renderer).
// 모든 데이터는 HeaderContext를 통해 렌더 시점에 주입.
struct Header;
```

### Interactions

```
  App.render(frame)
       |
       v
  HeaderContext {
      resource_type: Router.current_route().display_name(),
      cloud_name: Config.active_cloud(),
      region: Config.active_region(),
  }
       |
       v
  Header.render(frame, areas.header, &ctx)
       |
       v
  ┌──────────────────────────────────────────────────┐
  │  Servers                       prod-cloud | RegionOne  │
  └──────────────────────────────────────────────────┘
       Left-aligned              Right-aligned
       (Bold, White)             (Dim, Cyan)
```

---

## 3. Sidebar

**Responsibility**: Tab 키로 토글 가능한 모듈 목록 패널. RbacGuard 기반 메뉴 필터링, 활성 모듈 하이라이트.

### Interface

```rust
pub struct SidebarItem {
    pub label: String,       // 표시명 (e.g. "Servers")
    pub route: Route,        // 이동 대상
    pub shortcut: String,    // 축약어 (e.g. ":srv")
    pub admin_only: bool,    // Admin 전용 여부
}

pub struct Sidebar {
    items: Vec<SidebarItem>,
    selected_index: usize,
}

impl Sidebar {
    pub fn new(items: Vec<SidebarItem>) -> Self;

    /// RbacGuard 기반으로 표시 가능한 항목만 필터링.
    /// Admin이 아니면 admin_only 항목 숨김.
    pub fn visible_items(&self, rbac: &RbacGuard) -> Vec<&SidebarItem>;

    /// 키 입력 처리 (j/k로 이동, Enter로 선택).
    /// 선택 시 Action::Navigate(route) 반환.
    pub fn handle_key(&mut self, key: KeyEvent) -> Option<Action>;

    /// 현재 활성 라우트에 맞춰 하이라이트 동기화
    pub fn sync_active(&mut self, current_route: &Route);

    /// 렌더링. 활성 항목은 반전 색상 + ">" 마커.
    pub fn render(
        &self,
        frame: &mut Frame,
        area: Rect,
        rbac: &RbacGuard,
        current_route: &Route,
    );
}
```

### Dependencies

| Component | Relationship |
|-----------|-------------|
| `RbacGuard` | Admin 전용 메뉴 필터링 |
| `Router` | 현재 Route로 활성 항목 동기화 |
| `LayoutManager` | sidebar 영역 제공 |
| `App` | 키 이벤트 전달 |

### Data Owned

```rust
struct Sidebar {
    items: Vec<SidebarItem>,    // 전체 메뉴 항목 (고정, 앱 시작 시 구성)
    selected_index: usize,       // 현재 커서 위치
}
```

### Interactions

```
  User presses Tab
       |
       v
  App.handle_key() --> LayoutManager.toggle_sidebar()
                       (사이드바 영역 생성/제거)

  User presses j/k (사이드바 포커스 시)
       |
       v
  Sidebar.handle_key(KeyEvent)
       |-- j: selected_index += 1
       |-- k: selected_index -= 1
       |-- Enter: return Some(Action::Navigate(items[selected].route))
       v
  App receives Action::Navigate
       |
       v
  Router.navigate(route)

  Render:
  ┌────────────┐
  │  Modules   │  <-- title
  │            │
  │  Servers   │  <-- dim
  │> Networks  │  <-- selected (반전 색상)
  │  Volumes   │
  │  Images    │
  │  --------- │  <-- separator
  │  Projects  │  <-- admin_only (RbacGuard 통과 시만)
  │  Users     │
  └────────────┘
```

---

## 4. InputBar

**Responsibility**: 통합 입력 바 -- `:` prefix는 커맨드 모드, `/` prefix는 검색 모드. 해당 모드별로 CommandParser 또는 SearchFilter에 위임.

### Interface

```rust
#[derive(Debug, Clone, PartialEq)]
pub enum InputMode {
    Normal,   // 입력 바 비활성 (힌트 텍스트만 표시)
    Command,  // : 커맨드 모드
    Search,   // / 검색 모드
}

pub struct InputBar {
    mode: InputMode,
    buffer: String,
    cursor_pos: usize,
}

impl InputBar {
    pub fn new() -> Self;

    /// 현재 입력 모드
    pub fn mode(&self) -> &InputMode;

    /// 입력 모드 활성화. ':' 또는 '/' 키 수신 시 App이 호출.
    pub fn activate(&mut self, mode: InputMode);

    /// 입력 모드 비활성화 (Esc 또는 Enter 후).
    pub fn deactivate(&mut self);

    /// 키 입력 처리.
    /// - 문자 입력: buffer에 추가
    /// - Backspace: buffer에서 삭제
    /// - Enter: 현재 buffer를 반환하고 deactivate
    /// - Esc: buffer 비우고 deactivate
    /// - Tab: (Command 모드) 자동완성 요청 시그널
    /// - Up/Down: (Command 모드) 히스토리 탐색 시그널
    /// 반환: InputAction (Commit, Cancel, AutoComplete, HistoryUp, HistoryDown, None)
    pub fn handle_key(&mut self, key: KeyEvent) -> InputAction;

    /// 현재 buffer 내용 (읽기 전용)
    pub fn buffer(&self) -> &str;

    /// 자동완성 결과로 buffer 교체
    pub fn set_buffer(&mut self, value: String);

    /// 렌더링.
    /// Normal: "Press : for command, / for search" (dim)
    /// Command: ":srv|" (cursor 표시)
    /// Search: "/web-0|" (cursor 표시)
    pub fn render(&self, frame: &mut Frame, area: Rect);
}

#[derive(Debug)]
pub enum InputAction {
    /// Enter 입력 -- buffer 내용 commit
    Commit(String),
    /// Esc 입력 -- 취소
    Cancel,
    /// Tab 입력 -- 자동완성 요청
    AutoComplete,
    /// Up 화살표 -- 히스토리 이전
    HistoryUp,
    /// Down 화살표 -- 히스토리 다음
    HistoryDown,
    /// 검색 모드에서 문자 입력 -- 실시간 필터 갱신
    SearchChanged(String),
    /// 그 외 내부 처리 완료
    None,
}
```

### Dependencies

| Component | Relationship |
|-----------|-------------|
| `App` | 키 이벤트 라우팅, 모드 전환 제어 |
| `CommandParser` | Command 모드 Commit/AutoComplete/History 위임 |
| `SearchFilter` | Search 모드 SearchChanged 위임 |
| `KeyMap` | `:` / `/` 감지 후 InputBar 활성화 |

### Data Owned

```rust
struct InputBar {
    mode: InputMode,      // 현재 모드
    buffer: String,       // 입력 버퍼
    cursor_pos: usize,    // 커서 위치 (buffer 내 byte offset)
}
```

### Interactions

```
  User types ':'
       |
       v
  KeyMap: Normal mode에서 ':' 감지
       |
       v
  App --> InputBar.activate(InputMode::Command)
       |
       v
  InputBar.mode = Command, buffer = "", cursor = 0

  User types 's', 'r', 'v'
       |
       v
  InputBar.handle_key(Char('s')) -> SearchChanged (Search 모드) 또는 None (Command 모드)
  InputBar.buffer = "srv"

  User presses Tab
       |
       v
  InputBar.handle_key(Tab) -> InputAction::AutoComplete
       |
       v
  App --> CommandParser.auto_complete("srv")
       |    returns "servers"
       v
  App --> InputBar.set_buffer("servers")

  User presses Enter
       |
       v
  InputBar.handle_key(Enter) -> InputAction::Commit("servers")
       |
       v
  App --> CommandParser.parse("servers")
       |    returns Command::Navigate(Route::Servers)
       v
  App --> Router.navigate(Route::Servers)
  App --> InputBar.deactivate()

  Render (Command mode):
  ┌──────────────────────────────────────────────────┐
  │ :servers|                                        │
  └──────────────────────────────────────────────────┘
     ^prefix  ^buffer                       ^cursor
```

---

## 5. StatusBar

**Responsibility**: 하단 1줄 바 -- 상태 메시지, 리소스 통계, 도움말 힌트 표시. Toast 알림 표시 영역 겸용.

### Interface

```rust
pub struct StatusInfo {
    pub message: String,           // 좌측 메시지 (e.g. "5 servers loaded")
    pub help_hint: String,         // 우측 힌트 (e.g. ":help | j/k:move | Enter:select")
    pub item_count: Option<usize>, // 리스트 항목 수
    pub selected_index: Option<usize>, // 현재 선택 인덱스
}

pub struct StatusBar;

impl StatusBar {
    pub fn new() -> Self;

    /// 렌더링.
    /// Toast가 있으면 Toast 우선 표시, 없으면 StatusInfo 표시.
    /// 좌측: message + item stats, 우측: help_hint
    pub fn render(
        &self,
        frame: &mut Frame,
        area: Rect,
        info: &StatusInfo,
        active_toasts: &[ToastMessage],
    );
}
```

### Dependencies

| Component | Relationship |
|-----------|-------------|
| `App` | StatusInfo 구성 및 전달 |
| `Toast` | 활성 Toast 메시지 제공 |
| `ResourceList` | item_count, selected_index 제공 |

### Data Owned

```rust
// StatusBar는 stateless renderer.
// StatusInfo와 Toast 데이터는 외부에서 주입.
struct StatusBar;
```

### Interactions

```
  App.render(frame)
       |
       v
  StatusInfo {
      message: active_component.status_message(),
      help_hint: KeyMap.context_help(current_mode),
      item_count: resource_list.total_count(),
      selected_index: resource_list.selected_index(),
  }
       |
       v
  StatusBar.render(frame, areas.status_bar, &info, &toast.active())
       |
       v
  Toast 존재 시:
  ┌──────────────────────────────────────────────────┐
  │ [OK] Server web-01 deleted successfully          │  <-- green bg
  └──────────────────────────────────────────────────┘

  Toast 없을 시:
  ┌──────────────────────────────────────────────────┐
  │ 5 servers | 3/5          :help | j/k | Enter     │
  └──────────────────────────────────────────────────┘
    ^message   ^idx/total      ^help_hint (right-aligned)
```

---

## 6. ResourceList

**Responsibility**: 범용 테이블 위젯 -- 칼럼 정의 기반 렌더링, 선택 하이라이트, 스크롤, 검색 하이라이트, 로딩 스피너. 모든 도메인 모듈의 리스트 뷰에서 사용.

### Interface

```rust
/// 칼럼 정의
pub struct ColumnDef {
    pub name: String,              // 헤더 표시명
    pub width: ColumnWidth,        // 폭 설정
    pub alignment: Alignment,      // Left, Center, Right
}

pub enum ColumnWidth {
    Fixed(u16),       // 고정 폭
    Percent(u16),     // 비율
    Min(u16),         // 최소 보장, 나머지 균등 분배
}

/// 행 데이터 (칼럼 순서대로 문자열 배열)
pub struct Row {
    pub cells: Vec<String>,
    pub id: String,               // 리소스 고유 ID (선택/액션용)
    pub style_hint: Option<RowStyleHint>,  // 상태 기반 색상 힌트
}

#[derive(Debug, Clone)]
pub enum RowStyleHint {
    Normal,
    Active,      // 초록 (ACTIVE, ENABLED)
    Error,       // 빨강 (ERROR, DOWN)
    Warning,     // 노랑 (BUILD, MIGRATING)
    Disabled,    // dim (SHUTOFF, DISABLED)
}

pub struct ResourceList {
    columns: Vec<ColumnDef>,
    rows: Vec<Row>,
    filtered_indices: Vec<usize>,
    selected: usize,
    scroll_offset: usize,
    loading: bool,
    search_term: Option<String>,
}

impl ResourceList {
    pub fn new(columns: Vec<ColumnDef>) -> Self;

    /// 데이터 설정 (API 응답 후)
    pub fn set_rows(&mut self, rows: Vec<Row>);

    /// 로딩 상태 설정
    pub fn set_loading(&mut self, loading: bool);

    /// 현재 선택된 행의 ID 반환
    pub fn selected_id(&self) -> Option<&str>;

    /// 현재 선택된 행 인덱스 (필터링된 목록 기준)
    pub fn selected_index(&self) -> usize;

    /// 전체 행 수 (필터 적용 후)
    pub fn visible_count(&self) -> usize;

    /// 전체 행 수 (필터 무관)
    pub fn total_count(&self) -> usize;

    /// 검색 필터 적용. SearchFilter에서 호출.
    /// filtered_indices 갱신, selected를 0으로 리셋.
    pub fn apply_filter(&mut self, term: &str);

    /// 검색 필터 해제. 전체 행 표시.
    pub fn clear_filter(&mut self);

    /// 키 입력 처리 (네비게이션).
    /// j/Down: 다음, k/Up: 이전, G: 끝, g: 처음, PageUp/PageDown
    /// Enter 시 Action::SelectResource(id) 반환.
    pub fn handle_key(&mut self, key: KeyEvent) -> Option<Action>;

    /// 렌더링. 칼럼 헤더 + 데이터 행.
    /// - 선택 행: 반전 색상
    /// - 검색 매칭: 노란 하이라이트
    /// - 로딩 중: 중앙 스피너 + "Loading..."
    /// - 빈 목록: 중앙 "No items found"
    pub fn render(&self, frame: &mut Frame, area: Rect);
}
```

### Dependencies

| Component | Relationship |
|-----------|-------------|
| Domain Modules (ServerModule 등) | 칼럼 정의 + Row 데이터 제공 |
| `SearchFilter` | `apply_filter()` / `clear_filter()` 호출 |
| `KeyMap` | 네비게이션 키 해석 |
| `StatusBar` | `visible_count()`, `selected_index()` 통계 제공 |

### Data Owned

```rust
struct ResourceList {
    columns: Vec<ColumnDef>,        // 칼럼 정의 (모듈별 고정)
    rows: Vec<Row>,                  // 전체 행 데이터
    filtered_indices: Vec<usize>,    // 필터 적용 후 표시할 행 인덱스
    selected: usize,                 // 현재 선택 (filtered 기준)
    scroll_offset: usize,            // 스크롤 오프셋 (뷰포트 기준)
    loading: bool,                   // 로딩 스피너 표시 여부
    search_term: Option<String>,     // 현재 검색어 (하이라이트용)
}
```

### Interactions

```
  ServerModule receives AppEvent::ServersLoaded(servers)
       |
       v
  servers.iter().map(|s| Row {
      id: s.id.clone(),
      cells: vec![s.name, s.status, s.flavor, s.ip, s.created],
      style_hint: match s.status {
          "ACTIVE" => Some(Active),
          "ERROR"  => Some(Error),
          _ => None
      },
  })
       |
       v
  ResourceList.set_rows(rows)
  ResourceList.set_loading(false)

  Render:
  ┌──────────────────────────────────────────────────┐
  │ Name         Status    Flavor    IP        Created│  <-- header (bold, underline)
  ├──────────────────────────────────────────────────┤
  │ web-01       ACTIVE    m1.small  10.0.0.1  Mar 20│  <-- green
  │>web-02       ACTIVE    m1.large  10.0.0.2  Mar 21│  <-- selected (반전)
  │ db-01        ERROR     m1.xlarge 10.0.0.3  Mar 19│  <-- red
  │ worker-01    SHUTOFF   m1.small  10.0.0.4  Mar 18│  <-- dim
  └──────────────────────────────────────────────────┘

  Loading:
  ┌──────────────────────────────────────────────────┐
  │                                                  │
  │              [/] Loading servers...               │  <-- 스피너 (tick으로 회전)
  │                                                  │
  └──────────────────────────────────────────────────┘

  스크롤 로직:
  visible_height = area.height - 1 (header 1줄 제외)
  if selected >= scroll_offset + visible_height:
      scroll_offset = selected - visible_height + 1
  if selected < scroll_offset:
      scroll_offset = selected
```

---

## 7. DetailView

**Responsibility**: 범용 상세 뷰 위젯 -- 키-값 섹션 그룹화, 중첩 테이블(네트워크 인터페이스, 볼륨 등), 리소스 간 연관 링크(Enter로 네비게이션).

### Interface

```rust
/// 상세 뷰 데이터 모델
pub struct DetailData {
    pub title: String,
    pub sections: Vec<DetailSection>,
}

pub struct DetailSection {
    pub name: String,                  // 섹션명 (e.g. "Basic Info", "Network")
    pub fields: Vec<DetailField>,
}

pub enum DetailField {
    /// 단순 키-값
    KeyValue {
        key: String,
        value: String,
        style: Option<RowStyleHint>,   // 상태 색상 (e.g. "ACTIVE" -> green)
    },
    /// 중첩 테이블 (e.g. attached volumes, network interfaces)
    NestedTable {
        label: String,
        columns: Vec<String>,
        rows: Vec<Vec<String>>,
    },
    /// 다른 리소스 링크 (Enter로 이동 가능)
    ResourceLink {
        key: String,
        display: String,               // 표시 텍스트 (e.g. "private-net (net-abc123)")
        target_route: Route,           // 이동 대상
        target_id: String,            // 대상 리소스 ID
    },
}

pub struct DetailView {
    data: Option<DetailData>,
    scroll_offset: usize,
    focused_link_index: usize,
    links: Vec<(Route, String)>,      // 추출된 ResourceLink 목록
    loading: bool,
}

impl DetailView {
    pub fn new() -> Self;

    /// 상세 데이터 설정
    pub fn set_data(&mut self, data: DetailData);

    /// 로딩 상태 설정
    pub fn set_loading(&mut self, loading: bool);

    /// 데이터 초기화 (다른 리소스로 전환 시)
    pub fn clear(&mut self);

    /// 키 입력 처리.
    /// j/k: 스크롤, Tab: 링크 간 포커스 이동, Enter: 링크 네비게이션
    /// Esc: 리스트 뷰로 복귀
    pub fn handle_key(&mut self, key: KeyEvent) -> Option<Action>;

    /// 렌더링
    pub fn render(&self, frame: &mut Frame, area: Rect);
}
```

### Dependencies

| Component | Relationship |
|-----------|-------------|
| Domain Modules | DetailData 구성 및 제공 |
| `Router` | ResourceLink Enter 시 Navigate 액션 |
| `KeyMap` | 스크롤/네비게이션 키 |

### Data Owned

```rust
struct DetailView {
    data: Option<DetailData>,         // 현재 표시 중인 상세 데이터
    scroll_offset: usize,             // 세로 스크롤 오프셋
    focused_link_index: usize,        // 현재 포커스된 ResourceLink 인덱스
    links: Vec<(Route, String)>,      // data에서 추출한 모든 ResourceLink 대상
    loading: bool,
}
```

### Interactions

```
  User presses Enter on ResourceList (server "web-01" selected)
       |
       v
  ServerModule -> Action::SelectResource("srv-abc123")
       |
       v
  ServerModule.handle_event(AppEvent::ServerDetail(server))
       |
       v
  DetailData {
      title: "Server: web-01",
      sections: [
          DetailSection {
              name: "Basic Info",
              fields: [
                  KeyValue { key: "ID", value: "srv-abc123" },
                  KeyValue { key: "Status", value: "ACTIVE", style: Active },
                  KeyValue { key: "Flavor", value: "m1.small" },
                  ResourceLink {
                      key: "Image",
                      display: "Ubuntu 22.04 (img-def456)",
                      target_route: Route::ImageDetail,
                      target_id: "img-def456",
                  },
              ],
          },
          DetailSection {
              name: "Network Interfaces",
              fields: [
                  NestedTable {
                      label: "Interfaces",
                      columns: ["Network", "IP", "MAC", "Type"],
                      rows: [
                          ["private-net", "10.0.0.5", "fa:16:3e:xx", "fixed"],
                          ["public-net", "192.168.1.10", "fa:16:3e:yy", "floating"],
                      ],
                  },
              ],
          },
          DetailSection {
              name: "Volumes",
              fields: [
                  NestedTable { ... },
              ],
          },
      ],
  }

  Render:
  ┌──────────────────────────────────────────────────┐
  │ Server: web-01                                   │  <-- title (bold)
  │                                                  │
  │ -- Basic Info ---------------------------------- │  <-- section header
  │  ID:       srv-abc123                            │
  │  Status:   ACTIVE                                │  <-- green
  │  Flavor:   m1.small                              │
  │  Image:    [Ubuntu 22.04 (img-def456)]           │  <-- 링크 (cyan, underline)
  │                                                  │
  │ -- Network Interfaces -------------------------- │
  │  ┌──────────┬──────────┬──────────┬──────┐      │
  │  │ Network  │ IP       │ MAC      │ Type │      │
  │  ├──────────┼──────────┼──────────┼──────┤      │
  │  │private-net│10.0.0.5 │fa:16:3e: │fixed │      │
  │  │public-net│192.168.1 │fa:16:3e: │float │      │
  │  └──────────┴──────────┴──────────┴──────┘      │
  └──────────────────────────────────────────────────┘
```

---

## 8. FormWidget

**Responsibility**: 동적 폼 위젯 -- 필드 타입(Text/Dropdown/MultiSelect/Checkbox), 필드 검증(required/numeric/CIDR), Tab 네비게이션, Enter 제출, Esc 취소. 가장 복잡한 UI 컴포넌트.

### Interface

```rust
/// 필드 타입별 정의
#[derive(Debug, Clone)]
pub enum FieldDef {
    Text {
        name: String,
        label: String,
        placeholder: String,
        validation: Vec<Validation>,
    },
    Dropdown {
        name: String,
        label: String,
        options: Vec<SelectOption>,
        validation: Vec<Validation>,
    },
    MultiSelect {
        name: String,
        label: String,
        options: Vec<SelectOption>,
        validation: Vec<Validation>,
    },
    Checkbox {
        name: String,
        label: String,
        default: bool,
    },
}

#[derive(Debug, Clone)]
pub struct SelectOption {
    pub value: String,     // 실제 값 (e.g. flavor ID)
    pub display: String,   // 표시 텍스트 (e.g. "m1.small (2 vCPU, 4GB RAM)")
}

#[derive(Debug, Clone)]
pub enum Validation {
    Required,
    MinLength(usize),
    MaxLength(usize),
    Numeric,
    Cidr,             // CIDR 형식 (e.g. 192.168.0.0/24)
    Regex(String),    // 커스텀 정규식
    Custom {
        name: String,
        message: String,
        validator: fn(&str) -> bool,
    },
}

/// 필드별 현재 상태
#[derive(Debug, Clone)]
pub enum FieldState {
    TextInput {
        value: String,
        cursor_pos: usize,
    },
    DropdownSelected {
        selected_index: Option<usize>,
        open: bool,           // 드롭다운 펼침 여부
        scroll_offset: usize,
    },
    MultiSelectState {
        selected_indices: Vec<usize>,
        cursor_index: usize,
        open: bool,
    },
    CheckboxState {
        checked: bool,
    },
}

/// 검증 오류
pub struct FieldError {
    pub field_name: String,
    pub message: String,
}

/// 폼 제출 결과
pub type FormValues = HashMap<String, FormValue>;

#[derive(Debug, Clone)]
pub enum FormValue {
    Text(String),
    Selected(String),            // Dropdown 선택값
    MultiSelected(Vec<String>),  // MultiSelect 선택값들
    Bool(bool),                  // Checkbox
}

pub struct FormWidget {
    title: String,
    fields: Vec<FieldDef>,
    states: Vec<FieldState>,
    focused_field: usize,
    errors: Vec<FieldError>,
}

impl FormWidget {
    /// 필드 정의로 폼 생성
    pub fn new(title: String, fields: Vec<FieldDef>) -> Self;

    /// 키 입력 처리.
    /// - Tab/Shift+Tab: 필드 간 이동
    /// - Enter: 현재 필드가 마지막이면 submit, Dropdown이면 toggle open
    /// - Esc: 폼 취소 (Dropdown open이면 close만)
    /// - Space: Checkbox toggle, MultiSelect toggle
    /// - j/k: Dropdown/MultiSelect 내 커서 이동
    /// - 문자 입력: Text 필드에 입력
    /// 반환: FormAction
    pub fn handle_key(&mut self, key: KeyEvent) -> FormAction;

    /// 전체 필드 검증 실행. 오류가 있으면 errors에 저장.
    /// 모두 통과 시 Ok(FormValues) 반환.
    pub fn validate_and_submit(&mut self) -> Result<FormValues, Vec<FieldError>>;

    /// 특정 필드에 외부에서 옵션 설정 (비동기 로딩 후).
    /// e.g. Flavor 목록을 API로 가져온 후 Dropdown options 갱신.
    pub fn set_field_options(&mut self, field_name: &str, options: Vec<SelectOption>);

    /// 특정 필드 값 프리셋 (수정 폼에서 기존 값 채움)
    pub fn set_field_value(&mut self, field_name: &str, value: FormValue);

    /// 현재 포커스된 필드명
    pub fn focused_field_name(&self) -> &str;

    /// 렌더링
    pub fn render(&self, frame: &mut Frame, area: Rect);
}

#[derive(Debug)]
pub enum FormAction {
    /// 폼 제출 (검증 통과)
    Submit(FormValues),
    /// 폼 취소
    Cancel,
    /// 비동기 옵션 로딩 요청 (e.g. 필드 포커스 시 Flavor 목록 로딩)
    RequestOptions { field_name: String },
    /// 내부 처리 완료
    None,
}
```

### Dependencies

| Component | Relationship |
|-----------|-------------|
| Domain Modules | FieldDef 구성, FormValues 소비, 옵션 로딩 |
| `ActionDispatcher` | Submit 시 API 호출 액션 발송 |
| `Toast` | 검증 실패 시 오류 Toast |
| `KeyMap` | Form 모드 키 바인딩 |

### Data Owned

```rust
struct FormWidget {
    title: String,                 // 폼 제목 (e.g. "Create Server")
    fields: Vec<FieldDef>,         // 필드 정의 (모듈이 설정)
    states: Vec<FieldState>,       // 필드별 현재 상태
    focused_field: usize,          // 현재 포커스 필드 인덱스
    errors: Vec<FieldError>,       // 검증 오류 목록
}
```

### Interactions

```
  ServerModule: User presses 'c' (create)
       |
       v
  FormWidget::new("Create Server", vec![
      FieldDef::Text { name: "name", label: "Name", validation: [Required] },
      FieldDef::Dropdown { name: "flavor", label: "Flavor", options: vec![], validation: [Required] },
      FieldDef::Dropdown { name: "image", label: "Image", options: vec![], validation: [Required] },
      FieldDef::Dropdown { name: "network", label: "Network", options: vec![], validation: [Required] },
      FieldDef::Text { name: "key_name", label: "Key Pair", validation: [] },
      FieldDef::MultiSelect { name: "security_groups", label: "Security Groups", options: vec![] },
  ])
       |
       v
  App mode = Form
       |
       v
  (비동기: flavor/image/network/secgroup 옵션 로딩)
  AppEvent::FlavorsLoaded(flavors)
       -> form.set_field_options("flavor", flavors_to_options(flavors))

  User fills form, presses Enter on last field
       |
       v
  FormWidget.validate_and_submit()
       |-- 검증 실패: errors 저장, 첫 오류 필드로 포커스
       |-- 검증 성공: Ok(FormValues)
       v
  ServerModule -> Action::CreateServer(values)
       |
       v
  ActionDispatcher.spawn(create_server_task)

  Render:
  ┌──────────────────────────────────────────────────┐
  │ Create Server                                    │  <-- title
  │                                                  │
  │ Name:      [web-03          ]                    │  <-- focused (border highlight)
  │                                                  │
  │ Flavor:    [m1.small       v]                    │  <-- dropdown (closed)
  │                                                  │
  │ Image:     [Ubuntu 22.04   v]                    │
  │            ┌────────────────┐                    │
  │            │ Ubuntu 22.04   │ <-- open dropdown  │
  │            │ CentOS 9       │                    │
  │            │>Rocky 9        │ <-- cursor          │
  │            │ Debian 12      │                    │
  │            └────────────────┘                    │
  │                                                  │
  │ Network:   [private-net    v]                    │
  │                                                  │
  │ Key Pair:  [my-keypair      ]                    │
  │                                                  │
  │ Security:  [x] default                           │  <-- multi-select
  │            [ ] web-sg                            │
  │            [x] ssh-only                          │
  │                                                  │
  │        !! Name is required !!                    │  <-- 검증 오류 (red)
  │                                                  │
  │              [Submit]  [Cancel]                   │
  └──────────────────────────────────────────────────┘

  Tab 이동 순서:
  Name -> Flavor -> Image -> Network -> Key Pair -> Security Groups -> [Submit]
    ^                                                                      |
    +----------------------------------------------------------------------+
                              (cycle)
```

---

## 9. ConfirmDialog

**Responsibility**: 모달 확인 다이얼로그 -- 단순 Y/N과 강화 모드(파괴적 작업 시 리소스명 재입력). 현재 뷰 위에 오버레이.

### Interface

```rust
#[derive(Debug, Clone)]
pub enum ConfirmMode {
    /// 단순 Y/N
    Simple {
        title: String,
        message: String,
    },
    /// 강화: 리소스명 재입력 필수 (파괴적 작업)
    Enhanced {
        title: String,
        message: String,
        expected_input: String,   // 재입력해야 할 리소스명
    },
}

pub struct ConfirmDialog {
    mode: ConfirmMode,
    input_buffer: String,   // Enhanced 모드에서 사용자 입력
    cursor_pos: usize,
}

#[derive(Debug)]
pub enum ConfirmResult {
    Confirmed,
    Cancelled,
    Pending,   // Enhanced 모드에서 아직 입력 중
}

impl ConfirmDialog {
    /// 다이얼로그 생성
    pub fn new(mode: ConfirmMode) -> Self;

    /// 키 입력 처리.
    /// Simple: 'y'/'Y' -> Confirmed, 'n'/'N'/Esc -> Cancelled
    /// Enhanced: 문자 입력 -> buffer, Enter -> 검증 후 Confirmed/Pending, Esc -> Cancelled
    pub fn handle_key(&mut self, key: KeyEvent) -> ConfirmResult;

    /// Enhanced 모드 입력이 기대값과 일치하는지
    pub fn is_input_match(&self) -> bool;

    /// 모달 오버레이 렌더링.
    /// 화면 중앙, 반투명 배경, 테두리.
    pub fn render(&self, frame: &mut Frame, area: Rect);
}
```

### Dependencies

| Component | Relationship |
|-----------|-------------|
| `App` | 모달 상태 관리, 키 이벤트 우선 라우팅 |
| Domain Modules | ConfirmMode 구성 (작업 종류에 따라 Simple/Enhanced) |
| `ActionDispatcher` | Confirmed 후 실제 액션 발송 |

### Data Owned

```rust
struct ConfirmDialog {
    mode: ConfirmMode,        // 확인 모드 (Simple/Enhanced)
    input_buffer: String,     // Enhanced 모드 입력 버퍼
    cursor_pos: usize,        // Enhanced 모드 커서 위치
}
```

### Interactions

```
  User presses 'd' on server "web-01" (delete)
       |
       v
  ServerModule -> App.show_confirm(ConfirmMode::Enhanced {
      title: "Delete Server",
      message: "Type 'web-01' to confirm deletion:",
      expected_input: "web-01",
  })
       |
       v
  App.active_dialog = Some(ConfirmDialog::new(...))
  App.input_mode = Dialog   // 키 이벤트 다이얼로그 우선

  User types "web-01" + Enter
       |
       v
  ConfirmDialog.handle_key(Enter)
       |-- input_buffer == expected_input -> ConfirmResult::Confirmed
       v
  App -> ServerModule -> Action::DeleteServer("srv-abc123")
  App.active_dialog = None

  Render (overlay):
  ┌──────────────────────────────────────────────────┐
  │  (dimmed background -- 기존 ResourceList)         │
  │                                                  │
  │      ╔════════════════════════════════╗          │
  │      ║  Delete Server                 ║          │
  │      ╠════════════════════════════════╣          │
  │      ║                                ║          │
  │      ║  Type 'web-01' to confirm      ║          │
  │      ║  deletion:                     ║          │
  │      ║                                ║          │
  │      ║  > [web-01|]                   ║  <-- 입력중
  │      ║                                ║
  │      ║       [Enter: Confirm]         ║          │
  │      ║       [Esc: Cancel]            ║          │
  │      ╚════════════════════════════════╝          │
  │                                                  │
  └──────────────────────────────────────────────────┘

  Simple mode:
  ╔════════════════════════════════╗
  ║  Reboot Server                 ║
  ╠════════════════════════════════╣
  ║                                ║
  ║  Reboot server "web-01"?       ║
  ║                                ║
  ║    [Y]es         [N]o          ║
  ╚════════════════════════════════╝
```

---

## 10. Toast

**Responsibility**: 일시적 알림 메시지 -- 성공(green)/에러(red)/정보(blue), TTL 기반 자동 제거, 복수 Toast 스택.

### Interface

```rust
#[derive(Debug, Clone)]
pub enum ToastLevel {
    Success,  // green
    Error,    // red
    Info,     // blue
}

#[derive(Debug, Clone)]
pub struct ToastMessage {
    pub level: ToastLevel,
    pub text: String,
    pub created_at: Instant,
    pub ttl: Duration,
}

pub struct Toast {
    messages: Vec<ToastMessage>,
    max_visible: usize,
}

impl Toast {
    pub fn new() -> Self;

    /// Toast 추가. 스택 상단에 삽입.
    pub fn push(&mut self, level: ToastLevel, text: String);

    /// TTL 커스텀 Toast 추가.
    pub fn push_with_ttl(&mut self, level: ToastLevel, text: String, ttl: Duration);

    /// 틱마다 호출. 만료된 Toast 제거.
    /// EventLoop.on_tick()에서 호출.
    pub fn tick(&mut self);

    /// 현재 활성 Toast 목록 (최신 max_visible개)
    pub fn active(&self) -> &[ToastMessage];

    /// 활성 Toast 존재 여부
    pub fn has_active(&self) -> bool;

    /// 모든 Toast 즉시 제거
    pub fn clear(&mut self);
}
```

### Dependencies

| Component | Relationship |
|-----------|-------------|
| `App` | Toast 인스턴스 소유, `on_tick()`에서 `tick()` 호출 |
| `BackgroundTracker` | 작업 완료/실패 시 `push()` 호출 |
| `StatusBar` | `active()` 데이터로 렌더링 |
| `EventLoop` | 틱 이벤트에서 `tick()` 트리거 |

### Data Owned

```rust
struct Toast {
    messages: Vec<ToastMessage>,   // 활성 Toast 스택
    max_visible: usize,             // 동시 표시 최대 수 (기본 3)
    default_ttl: Duration,          // 기본 TTL (3초)
}
```

### Interactions

```
  BackgroundTracker: server deleted
       |
       v
  Toast.push(Success, "Server web-01 deleted successfully")
       |
       v
  messages = [
      ToastMessage { level: Success, text: "...", created_at: now, ttl: 3s },
  ]

  200ms later: EventLoop tick
       |
       v
  Toast.tick()
       |-- created_at + ttl > now -> 유지
       |-- created_at + ttl <= now -> 제거

  여러 Toast 스택:
  ┌──────────────────────────────────────────────────┐
  │ [OK] Server web-01 deleted                       │  <-- green, 2.5s remaining
  │ [!!] Failed to delete volume vol-01: in use      │  <-- red, 1.0s remaining
  │ [i] Refreshing server list...                    │  <-- blue, 0.5s remaining
  └──────────────────────────────────────────────────┘
     최신이 위, max_visible=3 초과 시 오래된 것 숨김

  StatusBar 렌더링 우선순위:
  Toast.has_active() == true  -> Toast 표시
  Toast.has_active() == false -> 일반 StatusInfo 표시
```

---

## 11. CommandParser

**Responsibility**: `:` 커맨드 파싱, Tab 자동완성(prefix match -> cycle), 축약어 매핑, 히스토리 관리(Up/Down, 최대 50개, 파일 persist).

### Interface

```rust
/// 파싱된 커맨드
#[derive(Debug, Clone)]
pub enum Command {
    /// 리소스 네비게이션
    Navigate(Route),
    /// 시스템 명령
    Quit,                          // :q, :quit
    Refresh,                       // :refresh
    Help,                          // :help
    ContextSwitch(String),         // :ctx <cloud-name>
    ContextList,                   // :ctx (인자 없음)
    /// 알 수 없는 커맨드
    Unknown(String),
}

/// 축약어 매핑 테이블
pub struct AbbreviationMap {
    map: HashMap<String, String>,
}

pub struct CommandParser {
    abbreviations: AbbreviationMap,
    history: CommandHistory,
    completions: Vec<String>,
    completion_index: usize,
}

pub struct CommandHistory {
    entries: Vec<String>,
    max_size: usize,
    cursor: Option<usize>,
    file_path: PathBuf,
}

impl CommandParser {
    /// 기본 축약어 테이블로 생성
    pub fn new(history_path: PathBuf) -> Self;

    /// 커맨드 문자열 파싱. 축약어 해석 포함.
    /// "srv" -> abbreviations["srv"] = "servers" -> Command::Navigate(Route::Servers)
    pub fn parse(&mut self, input: &str) -> Command;

    /// Tab 자동완성.
    /// 첫 Tab: prefix에 매칭되는 후보 수집, 첫 번째 반환.
    /// 연속 Tab: 다음 후보 순환.
    /// prefix가 바뀌면 후보 리셋.
    pub fn auto_complete(&mut self, prefix: &str) -> Option<String>;

    /// 자동완성 후보 초기화 (사용자가 문자 입력 시)
    pub fn reset_completion(&mut self);

    /// 히스토리에 커맨드 추가 (성공적 실행 후)
    pub fn push_history(&mut self, command: &str);

    /// 히스토리 위로 (이전 커맨드)
    pub fn history_prev(&mut self) -> Option<&str>;

    /// 히스토리 아래로 (다음 커맨드)
    pub fn history_next(&mut self) -> Option<&str>;

    /// 히스토리 커서 리셋 (새 입력 시작 시)
    pub fn history_reset_cursor(&mut self);

    /// 히스토리 파일 저장 (~/.config/nexttui/history)
    pub fn save_history(&self) -> Result<()>;

    /// 히스토리 파일 로드
    pub fn load_history(&mut self) -> Result<()>;

    /// 모든 유효 커맨드 목록 (자동완성용)
    pub fn available_commands(&self) -> Vec<&str>;
}
```

### Dependencies

| Component | Relationship |
|-----------|-------------|
| `InputBar` | Commit/AutoComplete/History 이벤트 위임 |
| `Router` | Navigate 커맨드 -> Route 변환 |
| `Config` | history_path, ContextSwitch 시 클라우드 존재 확인 |
| `App` | Quit/Refresh/Help 처리 |

### Data Owned

```rust
struct CommandParser {
    abbreviations: AbbreviationMap,   // 축약어 -> 정식명 매핑
    history: CommandHistory,           // 커맨드 히스토리
    completions: Vec<String>,          // 현재 자동완성 후보 목록
    completion_index: usize,           // 현재 자동완성 순환 위치
    last_prefix: Option<String>,       // 마지막 자동완성 prefix (변경 감지)
}

// 기본 축약어 매핑
// "srv"  -> "servers"
// "net"  -> "networks"
// "vol"  -> "volumes"
// "fip"  -> "floatingip"
// "sec"  -> "security-groups"
// "img"  -> "images"
// "flv"  -> "flavors"
// "prj"  -> "projects"
// "usr"  -> "users"
// "agg"  -> "aggregates"
// "hyp"  -> "hypervisors"
// "mig"  -> "migrations"
// "snap" -> "snapshots"
// "svc"  -> "compute-services"
// "agt"  -> "agents"
// "usg"  -> "usage"

// 커맨드 -> Route 매핑
// "servers"          -> Route::Servers
// "networks"         -> Route::Networks
// "volumes"          -> Route::Volumes
// "floatingip"       -> Route::FloatingIps
// "security-groups"  -> Route::SecurityGroups
// "images"           -> Route::Images
// "flavors"          -> Route::Flavors
// "projects"         -> Route::Projects
// "users"            -> Route::Users
// "aggregates"       -> Route::Aggregates
// "hypervisors"      -> Route::Hypervisors
// "migrations"       -> Route::Migrations
// "snapshots"        -> Route::Snapshots
// "compute-services" -> Route::ComputeServices
// "agents"           -> Route::Agents
// "usage"            -> Route::Usage
```

### Interactions

```
  User types ":srv" + Tab
       |
       v
  InputBar -> InputAction::AutoComplete
       |
       v
  CommandParser.auto_complete("srv")
       |
       |-- 후보 수집: ["servers", "snapshots", "security-groups", "svc"...]
       |   (prefix "srv" 매칭: abbreviations에서 "srv"->"servers" 발견)
       |-- "srv"는 정확한 축약어 -> "servers" 반환
       v
  InputBar.set_buffer("servers")

  User presses Enter
       |
       v
  InputBar -> InputAction::Commit("servers")
       |
       v
  CommandParser.parse("servers")
       |
       |-- 축약어 해석: "servers"는 직접 매핑
       |-- Route 변환: Route::Servers
       v
  Command::Navigate(Route::Servers)
       |
       v
  CommandParser.push_history("servers")
       |
       v
  App -> Router.navigate(Route::Servers)

  Auto-complete cycling:
  ":s" + Tab -> "servers"
  Tab again  -> "security-groups"
  Tab again  -> "snapshots"
  Tab again  -> "servers" (순환)

  History navigation:
  ":" + Up -> "servers"      (마지막 실행 커맨드)
  Up again  -> "networks"    (그 이전)
  Down      -> "servers"     (다시 앞으로)
  Down      -> ""            (현재 입력으로 복귀)
```

---

## 12. SearchFilter

**Responsibility**: `/` 검색 -- 현재 리소스 리스트에서 실시간 텍스트 필터링, Esc로 해제. 필터링된 인덱스 반환.

### Interface

```rust
pub struct SearchFilter {
    active: bool,
    term: String,
}

impl SearchFilter {
    pub fn new() -> Self;

    /// 검색 활성화
    pub fn activate(&mut self);

    /// 검색 비활성화 및 필터 해제
    pub fn deactivate(&mut self);

    /// 검색 활성 여부
    pub fn is_active(&self) -> bool;

    /// 검색어 갱신. InputBar에서 SearchChanged 시 호출.
    /// 반환: 새 검색어 (ResourceList.apply_filter에 전달)
    pub fn update_term(&mut self, term: &str) -> &str;

    /// 현재 검색어
    pub fn term(&self) -> &str;

    /// 행 데이터에 대해 필터링 수행.
    /// 각 Row의 모든 cell에서 대소문자 무시 부분 일치 검색.
    /// 반환: 매칭된 행의 인덱스 목록.
    pub fn filter_rows(&self, rows: &[Row]) -> Vec<usize>;

    /// 특정 텍스트에서 검색어 매칭 범위 반환 (하이라이트용).
    /// 반환: (start, end) byte offset 목록.
    pub fn match_ranges(&self, text: &str) -> Vec<(usize, usize)>;
}
```

### Dependencies

| Component | Relationship |
|-----------|-------------|
| `InputBar` | Search 모드에서 SearchChanged 이벤트 전달 |
| `ResourceList` | `apply_filter()` / `clear_filter()` 호출, 하이라이트 렌더링 |
| `App` | 활성화/비활성화 제어 |

### Data Owned

```rust
struct SearchFilter {
    active: bool,     // 검색 모드 활성 여부
    term: String,     // 현재 검색어
}
```

### Interactions

```
  User types '/'
       |
       v
  App -> InputBar.activate(InputMode::Search)
  App -> SearchFilter.activate()

  User types "web"
       |
       v
  InputBar.handle_key(Char('w')) -> InputAction::SearchChanged("w")
  InputBar.handle_key(Char('e')) -> InputAction::SearchChanged("we")
  InputBar.handle_key(Char('b')) -> InputAction::SearchChanged("web")
       |
       v (매 키 입력마다)
  App -> SearchFilter.update_term("web")
       |
       v
  SearchFilter.filter_rows(&resource_list.rows)
       |-- Row 0: cells = ["web-01", "ACTIVE", ...] -> "web" found -> index 0
       |-- Row 1: cells = ["web-02", "ACTIVE", ...] -> "web" found -> index 1
       |-- Row 2: cells = ["db-01", "ERROR", ...]   -> no match
       |-- Row 3: cells = ["worker-01", ...]         -> no match
       v
  filtered_indices = [0, 1]
       |
       v
  ResourceList.apply_filter("web")
       |-- filtered_indices = [0, 1]
       |-- selected = 0 (리셋)

  Render (ResourceList):
  ┌──────────────────────────────────────────────────┐
  │ Name         Status    Flavor    IP        Created│
  ├──────────────────────────────────────────────────┤
  │>[web]-01     ACTIVE    m1.small  10.0.0.1  Mar 20│  <-- "web" 노란 하이라이트
  │ [web]-02     ACTIVE    m1.large  10.0.0.2  Mar 21│  <-- "web" 노란 하이라이트
  └──────────────────────────────────────────────────┘
    (db-01, worker-01은 필터링되어 숨김)

  InputBar:
  ┌──────────────────────────────────────────────────┐
  │ /web|                                            │
  └──────────────────────────────────────────────────┘

  User presses Esc
       |
       v
  InputBar.deactivate()
  SearchFilter.deactivate()
  ResourceList.clear_filter()  -> 전체 행 복원
```

---

## 13. KeyMap

**Responsibility**: Vi 스타일 키 바인딩 매핑 -- 모드별(Normal/Command/Search/Form/Dialog) 키 해석, 컨텍스트 도움말 생성.

### Interface

```rust
/// 앱 전역 모드
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum AppMode {
    Normal,    // 리스트/상세 뷰 탐색
    Command,   // : 커맨드 입력
    Search,    // / 검색 입력
    Form,      // 폼 입력
    Dialog,    // 확인 다이얼로그
}

/// 키 바인딩이 매핑되는 의미적 액션
#[derive(Debug, Clone, PartialEq)]
pub enum KeyAction {
    // Navigation
    MoveUp,
    MoveDown,
    MoveToTop,
    MoveToBottom,
    PageUp,
    PageDown,
    Select,         // Enter
    Back,           // Esc

    // Mode switching
    EnterCommandMode,   // :
    EnterSearchMode,    // /
    ToggleSidebar,      // Tab

    // Resource actions
    Create,         // c
    Delete,         // d
    Edit,           // e
    Refresh,        // r

    // Form-specific
    NextField,      // Tab (Form mode)
    PrevField,      // Shift+Tab (Form mode)
    ToggleField,    // Space (Checkbox/MultiSelect)
    SubmitForm,     // Enter (last field / submit button)
    CancelForm,     // Esc

    // Dialog-specific
    Confirm,        // y/Y or Enter
    Deny,           // n/N

    // System
    Quit,           // q (Normal mode에서만)
    ForceQuit,      // Ctrl+C

    // 매핑 없음
    Unmapped,

    // 문자 입력 (Command/Search/Form 모드)
    CharInput(char),
}

pub struct KeyMap {
    bindings: HashMap<AppMode, HashMap<KeyEvent, KeyAction>>,
}

impl KeyMap {
    /// 기본 바인딩으로 생성
    pub fn new() -> Self;

    /// 현재 모드 + 키 이벤트로 액션 해석
    pub fn resolve(&self, mode: AppMode, key: KeyEvent) -> KeyAction;

    /// 현재 모드의 컨텍스트 도움말 문자열 생성.
    /// StatusBar.help_hint에 사용.
    pub fn context_help(&self, mode: AppMode) -> String;

    /// 커스텀 바인딩 오버라이드 (향후 설정 파일 기반)
    pub fn override_binding(
        &mut self,
        mode: AppMode,
        key: KeyEvent,
        action: KeyAction,
    );
}
```

### Dependencies

| Component | Relationship |
|-----------|-------------|
| `App` | `handle_key()`에서 `resolve()` 호출, 모드 판단 |
| `StatusBar` | `context_help()` 호출 |
| `Config` | 향후 커스텀 키 바인딩 로드 (Phase 2) |

### Data Owned

```rust
struct KeyMap {
    bindings: HashMap<AppMode, HashMap<KeyEvent, KeyAction>>,
}
```

### Default Bindings

```
Normal Mode:
  j / Down          -> MoveDown
  k / Up            -> MoveUp
  g                 -> MoveToTop
  G (Shift+g)       -> MoveToBottom
  PageUp / Ctrl+u   -> PageUp
  PageDown / Ctrl+d -> PageDown
  Enter             -> Select
  Esc               -> Back
  :                 -> EnterCommandMode
  /                 -> EnterSearchMode
  Tab               -> ToggleSidebar
  c                 -> Create
  d                 -> Delete
  e                 -> Edit
  r                 -> Refresh
  q                 -> Quit
  Ctrl+C            -> ForceQuit

Command Mode:
  Enter             -> Select (commit)
  Esc               -> Back (cancel)
  Tab               -> NextField (auto-complete)
  Up                -> MoveUp (history prev)
  Down              -> MoveDown (history next)
  Backspace         -> (handled by InputBar)
  Any char          -> CharInput(c)

Search Mode:
  Enter             -> Select (commit search, stay in filtered view)
  Esc               -> Back (clear search)
  Any char          -> CharInput(c)

Form Mode:
  Tab               -> NextField
  Shift+Tab         -> PrevField
  Enter             -> Select (open dropdown) or SubmitForm (last field)
  Esc               -> CancelForm (or close dropdown)
  Space             -> ToggleField
  j / Down          -> MoveDown (dropdown/multiselect 내)
  k / Up            -> MoveUp (dropdown/multiselect 내)
  Any char          -> CharInput(c) (text field)

Dialog Mode:
  y / Y             -> Confirm
  n / N / Esc       -> Deny
  Enter             -> Confirm (Enhanced: 입력 검증 후)
  Any char          -> CharInput(c) (Enhanced mode)
```

### Interactions

```
  EventLoop: KeyEvent received
       |
       v
  App.handle_key(key)
       |
       v
  KeyMap.resolve(app.current_mode(), key)
       |
       +-- Normal mode + 'j' -> KeyAction::MoveDown
       |     |
       |     v
       |   ActiveComponent.handle_key() (ResourceList navigates down)
       |
       +-- Normal mode + ':' -> KeyAction::EnterCommandMode
       |     |
       |     v
       |   App.set_mode(Command)
       |   InputBar.activate(InputMode::Command)
       |
       +-- Command mode + Enter -> KeyAction::Select
       |     |
       |     v
       |   InputBar.handle_key(Enter) -> Commit(buffer)
       |   CommandParser.parse(buffer)
       |
       +-- Dialog mode + 'y' -> KeyAction::Confirm
             |
             v
           ConfirmDialog -> ConfirmResult::Confirmed

  context_help 예시:
  Normal: "j/k:move  Enter:select  /:search  ::command  q:quit"
  Form:   "Tab:next  Enter:submit  Esc:cancel"
  Dialog: "y:confirm  n:cancel"
```

---

## 14. End-to-End Interaction Diagram

### Flow: User types `:srv` -> Server list rendered

```
  ┌──────────┐
  │   User   │
  └────┬─────┘
       │ types ':'
       v
  ┌──────────────────────────────────────────────────────────────────────────────┐
  │ EventLoop (tokio::select!)                                                   │
  │   crossterm::EventStream -> KeyEvent(Char(':'))                              │
  └────┬─────────────────────────────────────────────────────────────────────────┘
       │
       v
  ┌──────────────────────────────────────────────────────────────────────────────┐
  │ App.handle_key(KeyEvent)                                                     │
  │   KeyMap.resolve(Normal, ':') -> KeyAction::EnterCommandMode                │
  │   app.mode = AppMode::Command                                               │
  │   InputBar.activate(InputMode::Command)                                     │
  └────┬─────────────────────────────────────────────────────────────────────────┘
       │
       │ User types 's', 'r', 'v'
       v
  ┌──────────────────────────────────────────────────────────────────────────────┐
  │ InputBar.handle_key(Char('s')) -> None  (buffer = "s")                      │
  │ InputBar.handle_key(Char('r')) -> None  (buffer = "sr")                     │
  │ InputBar.handle_key(Char('v')) -> None  (buffer = "srv")                    │
  └────┬─────────────────────────────────────────────────────────────────────────┘
       │
       │ User presses Enter
       v
  ┌──────────────────────────────────────────────────────────────────────────────┐
  │ InputBar.handle_key(Enter)                                                   │
  │   -> InputAction::Commit("srv")                                             │
  └────┬─────────────────────────────────────────────────────────────────────────┘
       │
       v
  ┌──────────────────────────────────────────────────────────────────────────────┐
  │ App receives Commit("srv")                                                   │
  │   CommandParser.parse("srv")                                                │
  │     -> abbreviations: "srv" -> "servers"                                    │
  │     -> command_map: "servers" -> Command::Navigate(Route::Servers)          │
  │   CommandParser.push_history("srv")                                         │
  │   InputBar.deactivate()                                                     │
  │   app.mode = AppMode::Normal                                                │
  └────┬─────────────────────────────────────────────────────────────────────────┘
       │
       v
  ┌──────────────────────────────────────────────────────────────────────────────┐
  │ Router.navigate(Route::Servers)                                              │
  │   active_component = components[Route::Servers]  // ServerModule             │
  │   ServerModule.on_mount()                                                   │
  │     -> Action::FetchServers sent via action_tx                              │
  └────┬─────────────────────────────────────────────────────────────────────────┘
       │
       v
  ┌──────────────────────────────────────────────────────────────────────────────┐
  │ ActionDispatcher receives Action::FetchServers                               │
  │   tokio::spawn(async {                                                      │
  │     let servers = nova_port.list_servers().await?;  // NovaHttpAdapter       │
  │     event_tx.send(AppEvent::ServersLoaded(servers))                         │
  │   })                                                                        │
  └────┬─────────────────────────────────────────────────────────────────────────┘
       │
       │ (meanwhile, UI renders loading state)
       v
  ┌──────────────────────────────────────────────────────────────────────────────┐
  │ Render cycle (during loading):                                               │
  │                                                                             │
  │ ┌────────────────────────────────────────────────────┐                      │
  │ │ Servers                         prod-cloud | RegionOne │  <- Header       │
  │ ├──────────┬───────────────────────────────────────────┤                    │
  │ │ Modules  │                                           │                    │
  │ │          │        [/] Loading servers...              │  <- ResourceList   │
  │ │>Servers  │                                           │     (loading)      │
  │ │ Networks │                                           │                    │
  │ │ Volumes  │                                           │                    │
  │ ├──────────┴───────────────────────────────────────────┤                    │
  │ │ Press : for command, / for search                    │  <- InputBar       │
  │ ├──────────────────────────────────────────────────────┤                    │
  │ │ Loading...                  j/k:move Enter:select    │  <- StatusBar      │
  │ └──────────────────────────────────────────────────────┘                    │
  └────┬─────────────────────────────────────────────────────────────────────────┘
       │
       │ API response arrives via event_rx
       v
  ┌──────────────────────────────────────────────────────────────────────────────┐
  │ EventLoop: event_rx.recv() -> AppEvent::ServersLoaded(servers)              │
  │   App.handle_event(event)                                                   │
  │     ServerModule.handle_event(AppEvent::ServersLoaded(servers))             │
  │       resource_list.set_loading(false)                                      │
  │       resource_list.set_rows(servers_to_rows(servers))                      │
  └────┬─────────────────────────────────────────────────────────────────────────┘
       │
       │ next render cycle
       v
  ┌──────────────────────────────────────────────────────────────────────────────┐
  │ Render cycle (data loaded):                                                  │
  │                                                                             │
  │ ┌────────────────────────────────────────────────────┐                      │
  │ │ Servers                         prod-cloud | RegionOne │  <- Header       │
  │ ├──────────┬───────────────────────────────────────────┤                    │
  │ │ Modules  │ Name       Status  Flavor   IP     Created│                    │
  │ │          │────────────────────────────────────────────│                    │
  │ │>Servers  │>web-01     ACTIVE  m1.small 10.0.  Mar 20 │  <- selected      │
  │ │ Networks │ web-02     ACTIVE  m1.large 10.0.  Mar 21 │                    │
  │ │ Volumes  │ db-01      ERROR   m1.xl    10.0.  Mar 19 │  <- red           │
  │ │ Images   │ worker-01  SHUTOFF m1.small 10.0.  Mar 18 │  <- dim           │
  │ ├──────────┴───────────────────────────────────────────┤                    │
  │ │ Press : for command, / for search                    │  <- InputBar       │
  │ ├──────────────────────────────────────────────────────┤                    │
  │ │ 4 servers | 1/4             j/k:move Enter:select    │  <- StatusBar      │
  │ └──────────────────────────────────────────────────────┘                    │
  └──────────────────────────────────────────────────────────────────────────────┘
```

### Sequence Diagram (Compact)

```
User        EventLoop     App        KeyMap     InputBar   CmdParser   Router    ServerModule  ActionDisp   NovaPort
 |              |           |          |           |          |          |            |            |           |
 |--':' key---->|           |          |           |          |          |            |            |           |
 |              |--key----->|          |           |          |          |            |            |           |
 |              |           |--resolve>|           |          |          |            |            |           |
 |              |           |<-CmdMode-|           |          |          |            |            |           |
 |              |           |--activate----------->|          |          |            |            |           |
 |              |           |          |           |          |          |            |            |           |
 |--'srv'------>|           |          |           |          |          |            |            |           |
 |              |--keys---->|          |           |          |          |            |            |           |
 |              |           |--handle_key--------->|          |          |            |            |           |
 |              |           |          |           |(buffer)  |          |            |            |           |
 |              |           |          |           |          |          |            |            |           |
 |--Enter------>|           |          |           |          |          |            |            |           |
 |              |--key----->|          |           |          |          |            |            |           |
 |              |           |--handle_key--------->|          |          |            |            |           |
 |              |           |<----Commit("srv")----|          |          |            |            |           |
 |              |           |--parse("srv")----------------->|          |            |            |           |
 |              |           |<----Navigate(Servers)-----------|          |            |            |           |
 |              |           |--navigate(Servers)------------------------->|           |            |           |
 |              |           |          |           |          |          |--on_mount-->|            |           |
 |              |           |          |           |          |          |            |--FetchSrv-->|           |
 |              |           |          |           |          |          |            |            |--list()--->|
 |              |           |          |           |          |          |            |            |           |
 |              |           |          |           |          |          |            |            |<--Ok([])--|
 |              |           |          |           |          |          |            |<-SrvLoaded-|           |
 |              |           |<--handle_event(SrvLoaded)------|----------|------------>|            |           |
 |              |           |          |           |          |          |            |            |           |
 |              |--draw---->|          |           |          |          |            |            |           |
 |              |           |========= RENDER ============================================        |           |
 |<-------------|           |          |           |          |          |            |            |           |
```

---

## Component Dependency Graph (Summary)

```
                    ┌──────────┐
                    │   App    │
                    └────┬─────┘
         ┌───────────────┼───────────────────────────────┐
         v               v               v               v
   ┌──────────┐   ┌──────────┐   ┌──────────────┐  ┌────────┐
   │  KeyMap   │   │  Router  │   │LayoutManager │  │ Toast  │
   └──────────┘   └──────────┘   └──────┬───────┘  └────────┘
                                        |
                  ┌────────┬────────┬────+───┬──────────┐
                  v        v        v        v          v
              ┌──────┐ ┌───────┐ ┌───────┐ ┌────────┐ ┌─────────┐
              │Header│ │Sidebar│ │InputBar│ │StatusBar│ │ Content │
              └──────┘ └───┬───┘ └───┬───┘ └────┬───┘ │  Area   │
                           |         |          |      └────┬────┘
                      ┌────┘    ┌────┴────┐     |          |
                      v         v         v     v          v
                 ┌─────────┐ ┌─────┐ ┌──────┐ ┌─────┐  ┌──────────────┐
                 │RbacGuard│ │Cmd  │ │Search│ │Toast│  │Domain Module │
                 └─────────┘ │Parse│ │Filter│ └─────┘  │(ServerModule)│
                             └─────┘ └──┬───┘          └──────┬───────┘
                                        |                     |
                                        v              ┌──────┴──────┐
                                   ┌──────────┐        v             v
                                   │Resource  │   ┌─────────┐  ┌──────────┐
                                   │  List    │   │DetailView│  │FormWidget│
                                   └──────────┘   └─────────┘  └──────────┘
                                                                     |
                                                                     v
                                                               ┌──────────────┐
                                                               │ConfirmDialog │
                                                               └──────────────┘
```

---

## File Structure (Proposed)

```
src/
├── ui/
│   ├── mod.rs
│   ├── layout.rs          // LayoutManager
│   ├── header.rs          // Header
│   ├── sidebar.rs         // Sidebar, SidebarItem
│   ├── input_bar.rs       // InputBar, InputMode, InputAction
│   ├── status_bar.rs      // StatusBar, StatusInfo
│   ├── resource_list.rs   // ResourceList, ColumnDef, Row, ColumnWidth, RowStyleHint
│   ├── detail_view.rs     // DetailView, DetailData, DetailSection, DetailField
│   ├── form.rs            // FormWidget, FieldDef, FieldState, Validation, FormAction
│   ├── confirm_dialog.rs  // ConfirmDialog, ConfirmMode, ConfirmResult
│   └── toast.rs           // Toast, ToastMessage, ToastLevel
├── input/
│   ├── mod.rs
│   ├── command_parser.rs  // CommandParser, Command, AbbreviationMap, CommandHistory
│   ├── search_filter.rs   // SearchFilter
│   └── keymap.rs          // KeyMap, AppMode, KeyAction
└── ...
```
