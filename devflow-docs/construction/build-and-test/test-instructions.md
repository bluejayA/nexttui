# Test Instructions

## Unit Tests
Run: `cargo test`
Expected: 544 tests passed, 0 failures

## Manual Verification
- `cargo run -- --demo` → 서버 생성 폼에서 `*` 필수 표시 확인
- 폼에서 Enter → 확인 화면 (노란 테두리, 값 요약) 표시 확인
- 확인 화면에서 Enter → Submit, Esc → 폼 복귀 확인
- CUD 액션 후 StatusBar에 성공/실패 Toast 표시 확인
