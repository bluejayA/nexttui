# Units Generation

**Timestamp**: 2026-03-24T14:00:00+09:00

## Unit 1: form-core

### Unit: form-core
**Responsibility**: FormWidget 타입 정의 + 키 처리 + 검증 — src/ui/form.rs 전면 재작성
**Dependencies**: none
**Interfaces**:
- `FieldDef` enum (Text/Dropdown/MultiSelect/Checkbox) with name, label, placeholder, validation
- `FieldState` enum (TextInput/DropdownSelected/MultiSelectState/CheckboxState)
- `SelectOption { value, display }`
- `Validation` enum (Required/MinLength/MaxLength/Numeric/Cidr)
- `FieldError { field_name, message }`
- `FormValue` enum, `FormValues = HashMap<String, FormValue>`
- `FormAction` enum (Submit(FormValues)/Cancel/None)
- `FormWidget::new()`, `handle_key()`, `validate_and_submit()`
- `set_field_options()`, `set_field_value()`, `focused_field_name()`
- 방향키 네비게이션: ↑↓ 필드 이동, →/Enter 드롭다운 열기/선택, ← 드롭다운 닫기/폼 취소, Esc 취소
**Implementation order**: 1

## Unit 2: form-render

### Unit: form-render
**Responsibility**: FormWidget의 ratatui 렌더링 — render() 메서드 구현
**Dependencies**: form-core
**Interfaces**:
- `FormWidget::render(&self, frame: &mut Frame, area: Rect)`
- 제목, 필드 라벨 + 입력 영역, 포커스 하이라이트
- 드롭다운/MultiSelect 펼침 오버레이
- 검증 오류 표시
**Implementation order**: 2

## Unit 3: form-integration

### Unit: form-integration
**Responsibility**: ServerModule 폼 연동 + demo 모드 연동
**Dependencies**: form-core, form-render
**Interfaces**:
- ServerModule: ViewState::Create에서 FormWidget 생성, handle_key 위임, render 호출
- App: InputMode::Form 연계 (폼 모드에서 글로벌 키 바이패스)
- Demo: demo.rs에서 Server Create 폼에 flavor/image/network 샘플 옵션 주입
**Implementation order**: 3

## Implementation Order

```
form-core → form-render → form-integration
```
