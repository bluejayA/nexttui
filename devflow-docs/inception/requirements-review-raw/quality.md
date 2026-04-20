# Quality Review — BL-P2-074 requirements.md

**Reviewer**: aidlc:quality-reviewer
**Date**: 2026-04-18T22:55+09:00

## Assessment
Requires Changes

## Strengths
- Option (a)/(b) 선택 근거가 명시적이고, 미채택 이유가 타당 (BL 경계 존중).
- FR-3 resolver 알고리즘이 단계별로 기술되어 구현 모호성이 낮음.
- Assumption 3이 SwitchError variant 신설을 BL-P2-053과 의식적으로 분리 — 중복 방지 사고 good.
- 기존 `ByName` disambiguation 경로 재사용 결정(FR-3 step 4)이 DRY와 테스트 상속 측면에서 탁월.

## Issues

### P0 (Critical)
1. **FR-7 테스트 스코프 누락 — `CloudDirectory` trait 확장 파장 미반영.** 코드 조사 결과 `CloudDirectory` impl이 최소 4곳(`ConfigCloudDirectory`, `resolver.rs::tests::FakeClouds`, `app.rs:3444 FakeClouds`, `switcher.rs:213 FakeClouds`)에 존재. Assumption 2는 "기존 호출부 모두 업데이트 필요"라고만 적혀 있고, FR-7은 이를 테스트 책임으로 구체화하지 않음. NFR-2 "회귀 0건"을 강제하려면 **각 fake에 `default_project` 구현을 요구하는 명시 조항**이 필요.
2. **no-op/idempotency 테스트 누락 (FR-4).** FR-4는 "기존 idempotent 체크 재활용"에 의존하지만, 해당 체크의 실재 여부·위치(`SwitchContextSwitcher` 어느 지점인지)가 명시되지 않음. Acceptance criterion으로 **"동일 cloud 재입력 시 transition 카운터 불변" 단위 테스트**를 추가해야 함.

### P1 (Important)
3. **`switch-cloud` toast 문구 의존 테스트 파급 누락.** `app.rs:2409 test_command_bar_switch_cloud_emits_info_toast`가 현재 `.contains("switch-cloud")`로 매칭 — 신규 동작은 성공 시 toast를 내지 않을 수 있고, help-toast의 "switch-cloud" 문자열과 충돌 가능.
4. **관측성/tracing 요구 부재.** `CloudOnly` dispatch는 기존 span 상속만으로 충분한지 혹은 "cloud=X, resolved_project=Y" 필드 추가가 필요한지 NFR에 답이 없음.
5. **Assumption 2 — `#[non_exhaustive]` 리스크.** `worker.rs:790` 및 `switcher.rs:321`이 `ContextRequest`를 match하므로 **리스트업된 업데이트 포인트**(resolver, switcher, worker, app dispatch)를 명시해야 clippy `-D warnings`에 의존하지 않는 사전 점검 가능.

### P2 (Minor)
6. **FR-3 step 3 에러 메시지 분기 결정 불명확.**
7. **측정 가능성 — NFR-3 "+100~150 lines"**: "PR diff 상한" 정도로 표현 완화 권장.
8. **FR-5 에러 메시지 사용자 가이드 문구에 `:switch-project <name>` 중복** — Help toast(1758행)와 문구 일관성 확인 조항 누락.

## Cross-cutting Notes
- **To security-reviewer**: `default_project_name`이 clouds.yaml에 담긴 사용자 제어 문자열이므로 toast 에러 메시지에 그대로 삽입될 경우 터미널 이스케이프 injection 가능성.
- **To maintainability-reviewer**: `CloudDirectory` trait가 세 번째 메서드를 얻는 순간, 향후 확장 압력이 커짐.
- **To spec-reviewer**: FR-4 no-op 조항은 측정 기준이 없어 구현 리뷰 시 "기존에 있었다"로 회피될 위험.
