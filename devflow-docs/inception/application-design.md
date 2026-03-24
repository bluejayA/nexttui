# Application Design

**Mode**: LIST (목록 단계)
**Timestamp**: 2026-03-24T13:50:00+09:00

## 컴포넌트 목록

| 컴포넌트 | 책임 | 타입 | 상태 |
|---------|------|------|------|
| `FieldDef` | 필드 정의 enum (Text/Dropdown/MultiSelect/Checkbox) + name/label/validation | Type | 신규 (기존 FormFieldType 대체) |
| `FieldState` | 필드 런타임 상태 enum (TextInput/DropdownSelected/MultiSelectState/CheckboxState) | Type | 신규 (기존 FormField에서 분리) |
| `SelectOption` | Dropdown/MultiSelect 옵션의 value/display 쌍 | Type | 신규 |
| `Validation` | 필드 검증 규칙 enum (Required/MinLength/MaxLength/Numeric/Cidr) | Type | 신규 |
| `FieldError` | 검증 오류 (field_name + message) | Type | 신규 |
| `FormValue` | 제출 결과 값 enum (Text/Selected/MultiSelected/Bool) | Type | 신규 |
| `FormWidget` | 폼 코어 — 필드 관리, 키 처리, 검증, 렌더링 | Widget | 전면 재작성 |
| `FormAction` | 폼 키 처리 결과 enum (Submit/Cancel/None) | Type | 수정 |
| Module integration | ServerModule.ViewState::Create에서 FormWidget 생성/키 위임/렌더링 | Controller | 수정 |
| Demo integration | demo 모드에서 Server Create 폼에 샘플 옵션 주입 | Util | 수정 |

## 기존 코드 → 설계 문서 차이 요약

| 기존 코드 (src/ui/form.rs) | 설계 문서 (§8) | 변경 내용 |
|---------------------------|---------------|----------|
| `FormFieldType` (Text/Password/Dropdown/Checkbox) | `FieldDef` enum with name/label/validation | 전면 교체: 필드 정의에 name, validation 포함 |
| `FormField` (단일 구조체: label+value+required+...) | `FieldDef` + `FieldState` 분리 | 정의와 상태 분리 |
| Dropdown: `Vec<String>` 옵션 | `SelectOption { value, display }` | value/display 분리 |
| 검증: `required: bool`만 | `Validation` enum (Required/MinLength/MaxLength/Numeric/Cidr) | 규칙 기반 검증 |
| Submit: `Vec<FormField>` 반환 | `FormValues = HashMap<String, FormValue>` | 이름 기반 결과 맵 |
| handle_key: j/k가 Text에서도 글자 입력 | ↑↓로 필드 이동, 드롭다운 내에서만 옵션 이동 | 방향키 계층 네비게이션 |
| render(): 없음 | ratatui Frame+Rect 기반 렌더링 | 완전 신규 |
| 모듈 연동: 없음 | ServerModule ViewState::Create에서 생성/위임 | 완전 신규 |
| 드롭다운 열기/닫기: 없음 | open: bool, Enter/→로 열기, Esc/←로 닫기 | 완전 신규 |
| MultiSelect: 없음 | 다중 선택, Space 토글 | 완전 신규 |
