# Requirements Analysis

**Depth**: Standard
**Timestamp**: 2026-03-24T13:40:00+09:00

## User Intent
기존 nexttui(Rust/ratatui OpenStack TUI)의 FormWidget을 설계 문서(detail-design-ui-input.md §8) 기준으로 완성한다. 현재 src/ui/form.rs에 기본 골격(Text/Dropdown/Checkbox 필드, handle_key, validate)만 있고 렌더링이 없으며 모듈과 연동되지 않은 상태. FieldDef/FieldState 분리, SelectOption, Validation, 드롭다운 열기/닫기, MultiSelect, FormValues 반환, render(), 모듈 연동을 구현한다. 방향키(←→↑↓) 계층 네비게이션과 기존 앱 UX의 일관성을 핵심 요구사항으로 반영한다.

## Functional Requirements

### FR-01: FieldDef/FieldState 분리
기존 FormField 단일 구조체를 FieldDef(정의, 불변)와 FieldState(런타임 상태, 가변)로 분리한다.

### FR-02: 필드 타입 지원
4가지 필드 타입을 지원한다:
- **Text**: 문자열 입력, 커서 위치 관리
- **Dropdown**: 단일 선택, 펼침/접기, 스크롤
- **MultiSelect**: 다중 선택, 펼침/접기, 체크 토글
- **Checkbox**: 불리언 토글

### FR-03: SelectOption (value/display 분리)
Dropdown/MultiSelect 옵션은 value(실제 ID)와 display(표시 텍스트)를 분리한다.

### FR-04: Validation 규칙
필드별 검증 규칙을 지원한다:
- Required: 빈 값 불허
- MinLength / MaxLength: 길이 제한
- Numeric: 숫자만 허용
- Cidr: CIDR 형식 검증

### FR-05: FormValues 반환
제출 시 HashMap<String, FormValue>로 결과를 반환한다. FormValue는 Text(String), Selected(String), MultiSelected(Vec<String>), Bool(bool).

### FR-06: validate_and_submit
전체 필드 검증 → 실패 시 첫 오류 필드로 포커스 이동 + FieldError 목록 저장. 성공 시 Ok(FormValues) 반환.

### FR-07: set_field_options
외부에서 Dropdown/MultiSelect 옵션을 동적으로 설정할 수 있다 (비동기 API 응답 후 옵션 갱신).

### FR-08: set_field_value
수정 폼에서 기존 값을 프리셋할 수 있다.

### FR-09: render()
ratatui Frame + Rect를 받아 폼을 렌더링한다:
- 제목 표시
- 필드별 라벨 + 입력 영역
- 포커스된 필드 하이라이트
- 드롭다운/MultiSelect 펼침 시 옵션 목록 오버레이
- 검증 오류 표시 (필드 옆 또는 하단)

### FR-10: 모듈 연동
도메인 모듈(ServerModule 등)의 ViewState::Create에서 FormWidget을 생성하고, FormAction::Submit 결과를 Action으로 변환하여 dispatch한다. App의 InputMode::Form과 연계.

### FR-11: 방향키 네비게이션 일관성
앱 전체의 ←→↑↓ 계층 네비게이션과 일관되게 동작한다:
- **↑↓**: 필드 간 이동 (드롭다운 열려있으면 옵션 내 이동)
- **←**: 폼 취소 (ViewState::List로 복귀). 드롭다운 열려있으면 닫기만.
- **→ / Enter**: 드롭다운 열기/선택 확정. 마지막 필드에서 Enter = 제출.
- **Esc**: 드롭다운 열려있으면 닫기, 아니면 폼 취소
- **Tab / Shift+Tab**: 필드 간 이동 (↑↓과 동일, 보조)

### FR-12: demo 모드 연동
--demo 모드에서 'c' 키로 Create Form이 표시되고, 실제 입력/제출이 동작하는 것을 확인할 수 있다 (최소 ServerModule).

## Non-Functional Requirements

### NFR-01: UX 일관성
폼 내 네비게이션이 앱의 기존 Sidebar↔List↔Detail 방향키 흐름과 동일한 패턴을 따라야 한다. 이전 구현에서 네비게이션 불일치가 사용자 경험을 해쳤으므로 최우선 검증 항목.

### NFR-02: 테스트 커버리지
모든 필드 타입 × 키 입력 조합, 검증 규칙, 제출/취소 흐름에 대한 단위 테스트.

### NFR-03: 렌더링 성능
일반적인 폼(6-10개 필드) 렌더링이 16ms 이내. 드롭다운 옵션이 100개 이상이어도 스크롤로 처리.

## Assumptions
- Password 필드 타입은 Text와 동일하되 표시만 마스킹 — 별도 FieldDef 변형 불필요, Text에 is_password 플래그 또는 렌더링 시 처리
- Regex/Custom validation은 이번 범위에서 제외 (Required, MinLength, MaxLength, Numeric, Cidr만)
- 비동기 옵션 로딩(RequestOptions)은 set_field_options 인터페이스만 제공, 실제 API 호출 연동은 Phase 2

## Open Questions
없음
