# Code Generation Plan: form-confirm

> **For agentic workers:** REQUIRED: Use `aidlc:aidlc-code-generation` with the
> "GENERATE" signal to execute this plan. Do NOT implement ad-hoc.
> `"code-generation: GENERATE — proceed with the approved plan for form-confirm"`

## Files to Create
(none)

## Files to Modify
- [ ] `src/ui/form.rs` — FormPhase enum 추가, FormWidget에 phase 필드, render에 required `*` 표시, Confirming phase 키 핸들링 + 확인 뷰 렌더링

## Implementation Steps

- [ ] Step 1: Required 필드 `*` 표시
  - [ ] RED: `test_render_required_asterisk` — required 필드 라벨에 `*` 포함, 선택 필드에는 미포함 확인
  - [ ] Verify RED: 실패 확인
  - [ ] GREEN: `render()` 라벨 포맷에서 `Validation::Required` 체크 → `* Label:` 형식으로 변경
  - [ ] Verify GREEN: 통과 확인 + 전체 회귀
  - [ ] RED: `test_render_hint_shows_required_legend` — hint 영역에 `* = required` 포함 확인
  - [ ] Verify RED: 실패 확인
  - [ ] GREEN: hint 렌더링에 required 필드 존재 시 `* = required` 추가
  - [ ] Verify GREEN: 통과 확인 + 전체 회귀

- [ ] Step 2: FormPhase enum + phase 필드 추가
  - [ ] RED: `test_form_initial_phase_is_editing` — 새 FormWidget의 phase가 Editing인지 확인
  - [ ] Verify RED: 실패 확인
  - [ ] GREEN: `FormPhase` enum (Editing, Confirming) 정의, FormWidget에 `phase` 필드 추가, new()에서 Editing 초기화
  - [ ] Verify GREEN: 통과 확인 + 전체 회귀

- [ ] Step 3: validate → Confirming 전환
  - [ ] RED: `test_submit_enters_confirming_phase` — 유효 입력 후 Enter → phase가 Confirming이고 FormAction::None 반환 (아직 Submit 아님)
  - [ ] Verify RED: 실패 확인
  - [ ] GREEN: `validate_and_submit()` → 검증 통과 시 `self.phase = FormPhase::Confirming` + `FormAction::None` 반환. FormValues 빌드는 별도 메서드로 분리.
  - [ ] Verify GREEN: 통과 확인 + 전체 회귀

- [ ] Step 4: Confirming 상태 키 핸들링
  - [ ] RED: `test_confirming_enter_submits` — Confirming 상태에서 Enter → FormAction::Submit 반환
  - [ ] Verify RED: 실패 확인
  - [ ] GREEN: `handle_key()`에서 phase == Confirming일 때 Enter → build_values() + FormAction::Submit 반환
  - [ ] Verify GREEN: 통과 확인 + 전체 회귀
  - [ ] RED: `test_confirming_esc_returns_to_editing` — Confirming에서 Esc → phase가 Editing으로 복귀, FormAction::None
  - [ ] Verify RED: 실패 확인
  - [ ] GREEN: Confirming에서 Esc/Left → `self.phase = FormPhase::Editing` + FormAction::None
  - [ ] Verify GREEN: 통과 확인 + 전체 회귀

- [ ] Step 5: Confirming 확인 뷰 렌더링
  - [ ] RED: `test_render_confirm_view_shows_values` — Confirming phase에서 render 출력에 필드명:값 요약 포함 확인
  - [ ] Verify RED: 실패 확인
  - [ ] GREEN: `render()`에서 phase 분기 — Confirming이면 `render_confirm_view()` 호출 (필드명:값 요약 + Enter/Esc hint)
  - [ ] Verify GREEN: 통과 확인 + 전체 회귀

- [ ] Step 6: 기존 테스트 호환성 검증
  - [ ] 전체 `cargo test` 실행 → 535+ 테스트 전체 통과 확인
  - [ ] 기존 `test_create_form_submit_produces_action` 등 Submit 관련 테스트가 Confirming 경유 후에도 동작하도록 조정 (필요 시)

## Test Strategy
- [ ] `test_render_required_asterisk`: required 필드 `*` 표시, 선택 필드 미표시
- [ ] `test_render_hint_shows_required_legend`: hint에 `* = required` 안내
- [ ] `test_form_initial_phase_is_editing`: 초기 phase Editing
- [ ] `test_submit_enters_confirming_phase`: 유효 입력 → Confirming 전환 (Submit 아님)
- [ ] `test_confirming_enter_submits`: Confirming + Enter → Submit
- [ ] `test_confirming_esc_returns_to_editing`: Confirming + Esc → Editing 복귀
- [ ] `test_render_confirm_view_shows_values`: Confirming 렌더링에 값 요약 표시

> 각 테스트는 Implementation Steps의 RED 단계에서 작성된다. 별도 테스트 단계가 아님.
