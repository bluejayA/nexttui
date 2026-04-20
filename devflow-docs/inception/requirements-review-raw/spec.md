# Spec Review — BL-P2-074 requirements.md

**Reviewer**: aidlc:spec-reviewer
**Date**: 2026-04-18T22:50+09:00

## Status
Issues Found (Important/Suggestion 위주, Critical 1건)

## Critical
- **FR-5 edge case 불완전성**: "cloud with no auth config" (auth 블록 자체가 없는 clouds.yaml 엔트리) 케이스가 명시되지 않음. Assumption 1은 `CloudConfig.auth.project_name`을 단일 소스로 가정하지만 `auth` 자체가 None일 가능성은 다루지 않음. FR-3의 step 2/3은 `default_project_name: Option<String>`만 다루므로 `auth` 부재 시 동일 경로로 수렴하는지 결정 필요. Acceptance 레벨에서 "auth=None → Option::None으로 coalesce"를 명시하면 해결됨.

## Important
- **FR-4 testability 부족**: "resolved ContextTarget이 현재 활성과 동일하면 no-op"의 **관찰 가능한 결과**가 불분명. Toast가 뜨는지, state_machine transition 카운터가 증가하지 않는지, 어느 쪽을 assert할지 명시해야 테스트 작성 가능. "기존 switcher에 idempotent 체크가 있으면 재활용"은 설계 미결(Open Question에 둬야 함).
- **FR-7 acceptance criteria 강도**: "통합: dispatch 검증"이 모호함. `Action::SwitchContext(ContextRequest::CloudOnly { cloud: "prod" })`가 정확히 1회 emit됨을 검증한다고 명시해야 INVEST의 Testable 충족. 실패 경로 3종(unknown / no default / stale)도 "toast 문자열 정확 매칭" vs "ToastLevel::Error 존재" 중 측정 기준을 고를 것.
- **Assumption 3 (SwitchError 재사용) 설계 제약 불충분**: "문자열 메시지로 default-project 부재 표현"은 BL-P2-053이 별도 variant를 도입할 때 마이그레이션 부담을 만듦. `// TODO(BL-P2-053)` 마커 의무화 같은 연결 조건이 FR 또는 NFR에 없으면 forward-compat 리스크.

## Suggestion
- **NFR-3 (코드 규모)**는 스펙이 아니라 견적 — requirements보다는 application-design 산출물. 제거하거나 "참고용"으로 라벨링.
- **FR-2 "Non-exhaustive 영향 재확인"**: 재확인 결과를 이 문서 확정 전에 검증해 Assumption으로 승격하는 편이 INVEST의 Independent/Negotiable 측면에서 더 단단함.
- **누락 edge**: "0 projects available" (default_project_name은 있지만 keystone에 empty list) — FR-5의 "stale" 케이스에 흡수된다고 볼 수 있으나 명시하면 테스트 매트릭스가 깔끔.
- **FR-1 "파서 변경 없음"** 은 assumption이 아니라 검증된 사실 (`Command::SwitchCloud(String)` 이미 존재) — 근거 링크 한 줄 추가 권장.

## Good
- **FR-6** Legacy `:ctx` 분리 유지로 scope creep 차단 — closed scope 원칙 명확.
- **옵션 (b) 거절 근거** 및 forward-compat 서술(picker가 CloudOnly 위에 얹힘)이 Negotiable 측면에서 모범적.
- **Assumption 1** `--cloud` CLI와 동일 소스 원칙 — 단일 소스 불변조건 보호.

## Cross-cutting Notes
- **Security/Quality 리뷰어에게**: FR-5의 toast 메시지가 cloud name을 직접 보간(`cloud '<name>' not found`). NFR-4가 "이미 파서 레벨 sanitization"이라 주장하나 safe_display 사용 조건이 "해당 필요 시"로 느슨함.
- **Quality 리뷰어에게**: FR-4 idempotency 경로는 설계 단계에서 switcher 내부 계약 확인 필수.
