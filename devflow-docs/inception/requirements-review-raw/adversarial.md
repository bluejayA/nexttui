# Adversarial Review — BL-P2-074 requirements.md

**Reviewer**: general-purpose (Codex substitute — Codex agent permission denied)
**Date**: 2026-04-18T23:00+09:00

## HIGH

### H1. "default project = `CloudConfig.auth.project_name`" 의미적 오류
`auth.project_name`은 Keystone bootstrap credential의 scope이지 "사용자가 해당 cloud에서 선호하는 working project"가 아니다. 공유 `clouds.yaml`에서 많은 환경은 `admin`/`service`로 고정하는데, 이를 switch-cloud 기본값으로 재사용하면 운영 cloud로 전환하자마자 실수로 admin scope에 떨어진다 — OWASP A01 계열 privilege 오남용 표면. `--cloud` CLI(PR#55) precedent 재사용은 "동일 소스"라는 이점보다 "초기 bootstrap ≠ 사용자 선호"라는 의미 충돌이 크다. Assumption 1이 이를 "단일 소스 원칙"으로 포장하는 것은 근거 빈약. **최소한 FR-3에 "last-used project per cloud"를 fallback으로 덧붙일 것**을 요구.

### H2. 옵션 (b) 거절 근거 빈약
Unit 6 ContextPicker 의존은 scope creep의 증거가 아니라 **순서 문제**. 현재 toast가 이미 "(picker: Ctrl+P — Unit 6)"로 사용자에게 약속한 상황에서 picker 없이 "완결" 선언은 user contract 위반. (a) 채택 시 picker 도입 후 CloudOnly 경로는 "explicit choice 레이어 아래 잔재"가 되어 두 코드 경로를 유지해야 한다. "(b)를 블록하지 않음"은 true지만 **왜 지금 picker를 못 짓는가**에 대한 비용 근거가 없다.

### H3. FR-5 에러 메시지 vs BL-P2-053 SwitchError 확장 debt
"NotConfigured를 문자열로 우회" 선택은 BL-P2-053 착수 시 **테스트와 사용자 facing message 양쪽에서 breaking migration**이 된다. FR-7이 문자열 substring 어써션을 쓰면 053에서 깨질 것 — 지금 `#[non_exhaustive]` 전용 variant 하나 추가가 더 싸다.

## MEDIUM

### M1. FR-4 no-op race
"이미 활성 project와 동일"을 **resolver 출력 시점**에 비교하는데, state_machine이 `Switching` 상태면 "활성"이 pre-switch인지 pending인지 모호. epoch bump와의 교차 정의 누락. Assumption 5가 "switcher가 담당"으로 미루지만 SwitchContextSwitcher가 CloudOnly 경로에서 동일 idempotency를 보장한다는 증거 없음.

### M2. NFR-3 "100~150 lines" 과소 추정
`ContextRequest`는 `#[non_exhaustive]`가 **없고** `PartialEq/Eq` 파생으로 구조적 매칭 전제. codebase 내 모든 `match request`/`ContextRequest::`을 grep 기반으로 열거하지 않은 채 추정. 테스트 fakes + `CloudDirectory` trait 확장 호출부 업데이트 포함 시 200+ 가능.

### M3. `:switch-back` 시맨틱 공백
CloudOnly 경로 이후 rollback은 "pre-switch ContextTarget"인가 "pre-switch CloudOnly request"인가? requirements에 미정의 — BL-P2-053/054 착수 시 재설계 유발.

## LOW
**L1.** FR-6 "legacy :ctx 유지"는 올바르나 BL-P2-075 timeline을 명시하지 않아 tech debt hard deadline 부재.

## 동의
Assumption 2 (trait 확장 범위), FR-7 테스트 분기 커버리지는 타당.

## 권고
H1(fallback 정책), H2(picker 비용 근거 명문화), H3(전용 variant)를 해결하기 전 application-design 진입 금지.
