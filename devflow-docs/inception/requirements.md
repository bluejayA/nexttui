# Requirements Analysis

**Depth**: Standard
**Timestamp**: 2026-04-13T00:00:00+09:00
**BL**: BL-P2-031 (#39)

## User Intent
nexttui에서 런타임 중 활성 cloud / project 컨텍스트를 전환할 수 있게 한다. Keystone rescoping을 사용해 토큰 재발급 없이 프로젝트 스코프를 변경하고, 변경된 컨텍스트로 모든 모듈이 일관되게 동작하도록 한다.

**확정 해석**: 트리거 UX는 **B+ (인터랙티브 피커 + 명령 + Identity 리스트 `s` 단축키)**. Codex 적대적 리뷰 결과를 반영해 입력 UX 외에 **컨텍스트 전환의 원자성·이전 컨텍스트 격리·안전 가시성**을 동반 설계로 포함한다.

**구현 전략**: 단일 BL을 단계적 PR로 분할 (옵션 C). feature 브랜치에 PR1~PR6 누적 머지 → 통합 검증 후 main에 단일 머지.

## Functional Requirements

### FR-1. 명령 기반 전환 (Must)
- `:switch-project <name|uuid>` — 현재 cloud 내에서 프로젝트 변경
- `:switch-cloud <name>` — cloud 전환 (프로젝트는 cloud 기본값 또는 미선택 상태)
- `:switch-project <cloud>/<project>` — cloud-qualified 형식
- `:switch-back` — 이전 컨텍스트로 복귀 (1단계 히스토리)
- 이름 충돌 시 후보 목록을 표시하고 재선택을 요구한다 (silent pick 금지)
- Tab 자동완성 지원 (현재 cloud의 프로젝트 목록 기준)

### FR-2. 인터랙티브 피커 (Must)
- 글로벌 단축키 (예: `Ctrl+P`)로 어디서든 호출
- Fuzzy search 지원
- 각 항목은 `cloud • project • domain • project_id` 표시
- 기본 선택은 현재 컨텍스트 행

### FR-3. Identity 리스트 통합 (Must — C-lite)
- Identity 모듈의 Project / Cloud 리스트에서 `s` 키로 해당 행을 활성 컨텍스트로 전환
- `Enter`는 기존 Detail 진입 의미 유지

### FR-4. 전환 상태머신 (Must)
- 상태: `Idle → Switching → Committed | Failed`
- `Switching` 진입 시: epoch++, 이전 컨텍스트의 폴링/in-flight 작업 cancel, destructive 액션 입력 차단
- Keystone rescope 호출 → service catalog 강제 재조회 → 새 컨텍스트로 commit
- 실패 시 이전 컨텍스트로 rollback하고 사용자에게 가시적 에러 표시

### FR-5. ContextEpoch / 동시성 격리 (Must)
- 모든 액션·이벤트에 epoch 태그
- 폴링 루프와 장기 fetch는 `tokio::select!` cancel branch 또는 epoch 검증 필수
- 전환 후 이전 epoch의 이벤트는 폐기 (UI mutation 금지)

### FR-6. Keystone Rescoping Adapter (Must)
- token-method scoped exchange 사용 (Keystone v3)
- 새 토큰의 `expires_at`을 정본으로 신뢰 (TTL 추론 금지)
- rescope 후 service catalog와 endpoint 캐시 무효화·재조회
- rescope 거부 (예: `allow_rescope_scoped_token=false`, app-credential, 권한 부족) 시 가시적 실패 + 사용자에게 full re-auth 안내

### FR-7. 컨텍스트 인디케이터 (Must)
- 영구 표시 (예: 상태바 상단/하단) — `cloud / project` 최소 표시, 가능하면 domain·region 포함
- 전환 직후 일정 시간 강조 표시 (애니메이션 또는 색 강조)

### FR-8. Destructive 액션 안전 게이트 (Must)
- delete / force-delete / evacuate 등 destructive confirm 다이얼로그에 현재 `cloud • project` fingerprint를 명시적으로 표시
- 세션 내에서 직전에 컨텍스트가 변경된 경우 destructive confirm을 한 번 더 강제 (typing 또는 추가 확인)

### FR-9. UPDATE 모드 호환성 (Should)
- BL-P2-029의 다중 토큰 맵을 활용해 cloud별 토큰을 캐시·재사용 (재인증 최소화)
- BL-P2-028 토큰 캐시 영속화와 충돌 없이 동작

### FR-10. Region 전환 (Out of Scope)
- 본 BL은 Keystone scope 변경 (cloud / project)만 다룬다
- region 변경은 별도 명령/모달로 후속 백로그에 분리

### FR-11. T3 Runtime Wire (Must — 본 세션 스코프)
PR1(#68)에서 구현된 switch-core를 main.rs에 실제 연결하여 switch 경로가 end-to-end로 동작하도록 한다. B3 축소 범위 — HTTP 기반 프로젝트 탐색은 BL-P2-052로 분리.

- **ConfigCloudDirectory**: `Config` 래퍼. `CloudDirectory` trait 구현 (`active_cloud()`, `known_clouds()`). `Arc<Config>`를 공유하여 startup 시점 cloud 목록 반영.
- **StaticProjectDirectory**: `Config` 기반 `ProjectDirectoryPort` 구현. `list_projects(cloud)` → 해당 cloud의 `auth.project_name` 1건 반환. project_name 없는 cloud는 빈 목록. project_id는 name을 placeholder로 사용 (실제 id lookup은 BL-P2-052).
- **HttpEndpointCache 노출**: 5개 HttpAdapter(Nova/Neutron/Cinder/Glance/Keystone)의 `BaseHttpClient`를 `Arc<dyn HttpEndpointCache>`로 접근 가능하게 AdapterRegistry에 메서드 추가. `EndpointCatalogInvalidator`에 전달.
- **main.rs wire**: `KeystoneRescopeAdapter` + `EndpointCatalogInvalidator` + `TokenCacheStore` → `ScopedAuthSession` 조립. `SwitchStateMachine` + `CancellationRegistry` + `ContextTargetResolver`(ConfigCloudDirectory + StaticProjectDirectory) + `ContextHistoryStore` → `ContextSwitcher` 조립. `app.wire_context_switch(switcher, event_tx)` 호출.
- **demo 모드**: wire 없이 기존대로 동작 (switcher = None). 변경 없음.

#### T3 Out of Scope (BL-P2-052로 분리)
- `/v3/auth/projects` HTTP 호출 (동적 프로젝트 탐색)
- `AppEvent::ContextChanged` handler (16 모듈 캐시 무효화 + Fetch dispatch)
- Rescoped 토큰 자동 refresh

## Non-Functional Requirements

### NFR-1. 안전성 (Critical)
- 전환 이후 이전 컨텍스트의 stale 이벤트가 새 UI 상태를 변경해서는 안 된다 (epoch 검증으로 보장)
- rescope 실패 시 컨텍스트 인디케이터와 실제 활성 컨텍스트가 불일치해서는 안 된다 (atomic commit)

### NFR-2. 성능
- 전환 액션 (피커 선택 → commit)은 정상 경로에서 1초 이내 완료를 목표
- rescope + catalog 재조회의 네트워크 왕복을 합산해 측정

### NFR-3. 테스트 커버리지
- 단위 테스트: state machine, epoch 검증, 명령 파서, 충돌 disambiguation
- 통합 테스트: rescope 성공/실패, catalog 재조회 실패, 전환 중 in-flight 폴링, app-credential 경로 거부
- 기존 1240 tests baseline 무회귀 (PR1 이후)

### NFR-4. UX 일관성
- 단축키와 명령은 기존 CommandRegistry / KeyMap 패턴 준수
- 모달은 기존 Toast / Popup 컴포넌트 스타일 일관

### NFR-5. 관측성
- 전환 단계별 `tracing` 이벤트 (epoch, 대상 cloud/project, 결과)
- rescope 실패 사유 로깅

### NFR-6. 컴파일 안전성 (T3)
- wire 조립은 모두 컴파일 타임 타입 매칭으로 보장
- `dyn Trait` 다운캐스트 없이 직접 `Arc` 전달 — 런타임 타입 에러 원천 차단

### NFR-7. Demo 모드 무회귀 (T3)
- `--demo` 플래그 시 switcher=None으로 기존 동작 유지
- demo 경로에 switch 관련 코드 진입 금지 — demo 모드 테스트 무회귀

## Technology Stack
| 계층 | 선택 | 소스 | 비고 |
|------|------|------|------|
| Language | Rust (edition 2024) | Brownfield 감지 | — |
| TUI Framework | ratatui 0.30 + crossterm 0.29 | Brownfield 감지 | — |
| HTTP Client | reqwest | Brownfield 감지 | OpenStack 호출 |
| Async Runtime | tokio | Brownfield 감지 | CancellationToken 도입 필요 |
| Test Framework | built-in `#[cfg(test)]` | Brownfield 감지 | — |
| Lint | clippy (deny unwrap/expect) | CLAUDE.md | — |

## Assumptions
1. 대상 OpenStack 배포는 Keystone v3 + token-method rescoping을 허용한다. 비활성 환경은 가시적 실패 + full re-auth 폴백으로 대응한다.
2. cloud 정의는 기존 `clouds.yaml` 또는 nexttui Config의 cloud 목록을 그대로 사용한다 (별도 cloud 추가 UX는 본 BL 비포함).
3. App-credential 인증 사용자는 본 BL의 전환 UX에서 명시적 거부 메시지로 안내한다 (별도 BL로 분리).
4. **T3 한정**: 피커의 프로젝트 목록은 `clouds.yaml`에 선언된 정적 항목만 반환한다. 동적 조회(`/v3/auth/projects`)는 BL-P2-052에서 `StaticProjectDirectory`를 교체하여 구현한다.
5. `:switch-back` 히스토리 깊이는 1 (직전 컨텍스트만). 다단계 히스토리는 후속 백로그.
6. Region은 본 BL 비포함. 별도 후속 BL로 신설한다.

## Open Questions
없음 (Codex 적대적 리뷰의 10개 미결 질문은 위 요구사항에 모두 반영되었거나 명시적 Out of Scope / Assumption으로 처리됨).

## Change Log
- 2026-04-13: 초안 작성. Codex 적대적 리뷰 (10개 질문 + 3개 치명 결함 + 권장 수정안) 반영. UX 안 B+ 확정, 구현 전략 옵션 C (단일 BL 단계적 머지) 확정.
- 2026-04-16: UPDATE — FR-11 (T3 Runtime Wire) 추가. B3 축소 범위: ConfigCloudDirectory + StaticProjectDirectory(config 기반) + HttpEndpointCache 노출 + main.rs wire. NFR-3 baseline 1116→1240. Assumption #4를 T3 한정 정적 목록으로 한정.
- 2026-04-16: NFR-6 (컴파일 안전성), NFR-7 (Demo 모드 무회귀) 추가 — T3 wire 특화.
