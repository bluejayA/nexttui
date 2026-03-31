# Requirements Analysis

**Depth**: Standard
**Timestamp**: 2026-03-26T17:30:00+09:00

## User Intent

현재 admin/non-admin 이분법 RBAC를 admin/member/reader 3단계로 세분화한다.
Phase 1의 하드코딩 역할 체크를 유지하면서, reader 역할의 읽기 전용 제한과 member 역할의 CUD 허용을 명확히 구분한다.
후속 이슈에서 Capability 기반으로 전환할 때 `can_perform()` 인터페이스는 동일하게 유지되므로 전환 비용이 낮다.

해석 확정: A안 (역할 세분화) → 후속 B안 (Capability 전환)으로 점진 전환.

## Functional Requirements

### FR-1: 역할 3단계 정의
- **Admin**: 모든 액션 허용 (현재와 동일)
- **Member**: CRUD 허용, admin 전용 액션(ForceDelete, Migrate, Evacuate, EnableDisable, ManageQuota) 차단
- **Reader**: 읽기(Read) 전용, 모든 CUD 차단

### FR-2: can_perform 역할 연동
- `can_perform(ActionKind)` 내부에서 현재 역할 기준으로 판단
- Admin → 전부 허용
- Member → Create, Delete, Read 허용; ForceDelete, Migrate, Evacuate, EnableDisable, ManageQuota 차단
- Reader → Read만 허용, 나머지 전부 차단

### FR-3: Sidebar 메뉴 필터링
- Admin 전용 라우트(Migrations, Aggregates, ComputeServices, Hypervisors, Projects, Users, Agents, Usage)는 admin만 표시 (현재와 동일)
- Reader도 일반 라우트(Servers, Networks, Volumes 등)는 볼 수 있어야 함

### FR-4: Worker RBAC 가드 강화
- Worker의 `action_to_kind()` 매핑은 현재와 동일하게 유지
- `can_perform()` 내부 로직만 변경되므로 Worker 코드 변경 불필요

### FR-5: UI 액션 버튼 필터링
- 모듈의 키 핸들링에서 CUD 액션 키('c', 'd' 등)를 역할에 따라 비활성화
- Reader는 'c'(생성), 'd'(삭제) 키가 무시됨

## Non-Functional Requirements

- `can_perform()` 인터페이스 변경 없음 — Worker, Module 호출 코드 최소 변경
- 후속 Capability 전환 시 `is_admin_only_action()` + 역할 match 로직 ~50줄만 폐기
- 기존 테스트 하위 호환 — admin/member 테스트 그대로 통과

## Technology Stack

| 계층 | 선택 | 소스 | 비고 |
|------|------|------|------|
| Language | Rust 2024 | Brownfield | |
| 변경 대상 | src/infra/rbac.rs | 기존 | 핵심 변경 파일 |

## Assumptions

1. Keystone 역할 이름 기준: "admin", "member", "reader" (OpenStack 표준 3역할)
2. 역할이 여러 개면 가장 높은 권한 기준 (admin > member > reader)
3. 알 수 없는 역할은 reader로 처리 (최소 권한 원칙)
4. B단계(Capability 전환)는 별도 이슈로 분리

---

## UI/UX Redesign — Stage 2.5-A (Theme & Polish)

**Added**: 2026-03-31
**Depth**: Standard
**Analysis**: `devflow-docs/inception/ui-redesign-analysis.md`

### User Intent (UI Redesign)

devflow-tui와 btop의 디자인을 참조하여 nexttui의 프레젠테이션 레이어를 개선한다.
도메인 로직/포트/어댑터는 변경하지 않으며, `src/ui/` 파일과 각 모듈의 렌더링 코드만 대상으로 한다.
단, Theme 시스템 도입 시 기존 렌더링 로직과의 정합성, 포커스 상태 전파, Component trait 인터페이스 영향을 고려해야 한다.

### Functional Requirements (UI)

#### FR-UI-1: Theme 시스템 도입 (BL-P2-034)
- `src/ui/theme.rs` 신규 모듈 생성
- `Theme` 구조체: `active()`, `done()`, `error()`, `waiting()`, `focus_border()`, `unfocus_border()`, `highlight()`, `disabled()`, `link()`, `timestamp()` 등 시맨틱 스타일 토큰
- `Icons` 구조체: 상태별 Unicode 아이콘 (`●`, `✓`, `○`, `✗`, `⟳`, `◐`)
- `panel_title(name, focused) -> String`: 포커스 `[ Name ]` / 비포커스 `  Name  `
- `key_hint(key, desc) -> Vec<Span>`: 상태바 키 힌트 (key=Cyan Bold, desc=Dim)
- 기존 12개 파일의 109개 `Color::` 하드코딩을 Theme 호출로 교체
- **정합성**: 교체 시 기존 테스트(694개)가 모두 통과해야 함

#### FR-UI-2: Rounded 보더 + 포커스 피드백 (BL-P2-035)
- 모든 패널(Sidebar, Content)에 `BorderType::Rounded` 적용
- 포커스 패널: `Theme::focus_border()` (Cyan 보더)
- 비포커스 패널: `Theme::unfocus_border()` (DarkGray 보더)
- 기존 Sidebar RIGHT-only 보더 → 전체 Block 보더로 변경
- Content 영역에 Block 컨테이너 추가
- **정합성**: `FocusPane` enum과 연동하여 포커스 상태를 보더에 반영. `handle_key`의 Tab 키 포커스 전환 시 보더 색상 즉시 변경

#### FR-UI-3: 패널 타이틀 포맷 (BL-P2-036)
- Sidebar: `[ Modules ]` (포커스) / `  Modules  ` (비포커스)
- Content: `[ Servers ]`, `[ Server: web-01 ]`, `[ Create Server ]` 등 뷰 상태별 동적 타이틀
- `theme::panel_title()` 함수 활용
- **정합성**: 타이틀 변경이 레이아웃 영역 크기에 영향 주지 않아야 함 (Block title은 보더 안에 렌더링)

#### FR-UI-4: 상태바 리디자인 (BL-P2-037)
- 배경: `Style::new().on_dark_gray().white()` (현재는 투명)
- 좌측: `[패널명] context` (예: `[Servers] 1/5`)
- 우측: key hints — `theme::key_hint()` 활용
- 뷰 상태에 따라 힌트 동적 변경:
  - List: `j/k 이동  Enter 상세  c 생성  d 삭제  / 검색`
  - Detail: `Esc 목록  Tab 링크  r Resize  d 삭제`
  - Form: `↑↓ 필드  Enter 제출  Esc 취소`
- Toast는 상태바 위 별도 행에 표시 (상태바 오버라이드 금지). Toast 없을 때는 행 숨김
- **정합성**: `StatusBar::render()` 시그니처 변경 시 `App::draw()`의 호출부도 함께 수정

#### FR-UI-5: 리스트 하이라이트 개선 (BL-P2-038)
- 선택 행: `Black on White` → `White Bold` + 시맨틱 컬러 유지
- ACTIVE 행 선택 시 Green + Bold, ERROR 행 선택 시 Red + Bold
- 선택 마커: 좌측 `>` 또는 `▶` prefix 추가 (선택 사항)
- **정합성**: `ResourceList`의 `row_style()` 로직 변경. `ColumnDef` 구조는 변경 없음

### Non-Functional Requirements (UI)

- **NFR-UI-1**: Theme 교체 후 기존 694개 테스트 전부 통과
- **NFR-UI-2**: Theme 토큰 변경만으로 색상 체계 일괄 변경 가능 (파일당 수정 불필요)
- **NFR-UI-3**: 렌더링 성능 영향 없음 (Theme 메서드는 `Style` 값 반환, 계산 비용 없음)
- **NFR-UI-4**: 80x24 최소 터미널 크기에서 레이아웃 깨짐 없음

### Constraints (UI)

1. `src/ui/` 와 각 `src/module/*/mod.rs`의 렌더링 코드만 변경 대상
2. `Component` trait 인터페이스(`render`, `handle_key`, `handle_event`) 변경 없음
3. `Action`/`AppEvent` enum 변경 없음
4. 기존 포커스 전환 로직(`FocusPane`, Tab 키) 동작 보존

### Assumptions (UI)

1. devflow-tui Theme 구조 채택 + btop 융합 규칙 적용 (Agent Council 합의, 2026-03-31):
   - 16색 기본 (VDI 호환, truecolor 확장은 Stage 2.5-C)
   - 종료 상태=아이콘(`●○✗`)+시맨틱 컬러, 진행 상태=프로그레스바 (분리 규칙)
   - 시맨틱 컬러 고정: Active/Success=Green, Transition/Warning=Yellow, Error=BrightRed
   - 상태바: 좌측=고정 컨텍스트(`[Panel] N/M`), 우측=동적 key hints
2. NO_COLOR 지원은 Stage 2.5-C (BL-P2-046)에서 별도 구현 — 이번 스코프 제외
3. 다크 테마는 Stage 2.5-C (BL-P2-045)에서 별도 구현 — 이번 스코프 제외
4. `ratatui::widgets::Block`의 `BorderType::Rounded`는 ratatui 0.30에서 지원됨

### Open Questions (UI)

(없음 — 분석 문서에서 범위 확정됨)

## Change Log

- 2026-03-26 INITIAL: RBAC 역할 세분화 요구사항 분석
- 2026-03-31 UPDATE: UI/UX Redesign Stage 2.5-A (BL-P2-034~038) 요구사항 추가. Theme 시스템, Rounded 보더, 패널 타이틀, 상태바, 리스트 하이라이트 5개 기능 요구사항 + 4개 NFR + 4개 제약 조건
- 2026-03-31 UPDATE: 가정 1번 구체화 — Agent Council(Codex+Gemini+Claude) 합의 반영. devflow-tui 구조 + btop 융합 규칙(16색, 상태 분리, 시맨틱 컬러 고정, 상태바 좌고정/우동적)
