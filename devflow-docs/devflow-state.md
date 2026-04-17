# DevFlow State

## Current Phase
CONSTRUCTION

## Current Stage
Unit 4.5 Step A — app.rs wire (InputBar + CommandParser) — TDD RED

## Complexity
Standard

## Selected Approach
Unit 5 구현 중 stub blind spot 발견 — CommandParser + InputBar가 app.rs에 wire되지 않음. Unit 4.5 신규 분해로 선행 처리 후 Unit 5 원래 설계 진행.

## Step Plan
**Unit 4.5 (신규) — Command Bar Integration**
- Step A: app.rs wire — `input_bar: InputBar`, `command_parser: CommandParser` 필드 + handle_key 위임
- Step B: Command → Action dispatch (Navigate/Quit/Refresh/Help/Switch*)

**Unit 5 — Commands & Safety UI** (원래 설계, Unit 4.5에 의존)
- Step 1: `src/input/command.rs` — switch 파싱 + Tab (✅ 선구현 완료, 재테스트 대상)
- Step 2: `src/ui/context_indicator.rs` — 신규 위젯 (패시브 타이머 + highlight 만료)
- Step 3: `src/ui/status_bar.rs` — `set_context_indicator(Arc<RwLock<ContextIndicator>>)` 연결
- Step 4: `src/ui/confirm.rs` — `with_context_fingerprint(snapshot)` + `require_recontext_confirm(recently_switched)`
- Step 5: destructive 콜사이트 ~32개에 fingerprint 적용 + 통합 테스트 1건 (server delete)

## Branch
feat/bl-p2-031-pr3-commands-ui (from a00c044)

## Session Note
- 2026-04-17: PR3 CONSTRUCTION 사이클 시작. 이전 T3(PR #75) 머지 완료 상태에서 분기.
- **2026-04-17 INCEPTION UPDATE**: Step 1 구현 중 CommandParser/InputBar가 app.rs에 wire되지 않은 stub 발견 (`feedback_stub_blind_spot` 패턴). Unit 4.5 "Command Bar Integration"을 Unit 5 앞에 추가. 기존 Unit 5 설계는 유지.
- 설계 산출물 업데이트 대상: `units.md`, `application-design.md`, `workflow-plan.md`
- 커밋 전략: Unit 4.5 Step A/B + Unit 5 Step 1~5 = 7 커밋
- push/PR: CONSTRUCTION 완료 + Codex 리뷰 후 사용자 승인받아 진행
