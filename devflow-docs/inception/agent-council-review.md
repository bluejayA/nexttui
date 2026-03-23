# Agent Council Review — Application Design

**Timestamp**: 2026-03-23T10:00:00+09:00
**대상**: nexttui application-design (52개 컴포넌트 Comprehensive 설계)
**참여자**: Codex (GPT-5.3), Gemini, Claude (의장)

## 리뷰 범위

| 문서 | 대상 |
|------|------|
| application-design.md | 52개 컴포넌트 목록 + 인덱스 |
| detail-design.md | Core + Infrastructure 10개 |
| detail-design-port-adapter.md | Port + Adapter 13개 |
| detail-design-ui-input.md | UI Widget + Input 13개 |
| detail-design-domain-nfr.md | Domain Module 16개 + NFR 패턴 |
| async-event-architecture-design.md | 비동기 아키텍처 (이전 Council 합의) |

## 리뷰 포인트 및 결과

---

### 1. Port/Adapter 추상화 수준 (Phase 2 전환 용이성)

**Codex 탐색 결과**:
- `AdapterRegistry`가 `Arc<dyn *Port>` 타입으로 모든 서비스를 보유하여 DI 교체 구조 확인
- `replace_*()` 메서드가 Phase 2 Service Layer 프록시 교체용으로 설계됨
- `BaseHttpClient`가 concrete struct로 composition 패턴 사용 확인

**Gemini 의견**:
- 헥사고날 아키텍처를 충실히 따르고 있어 Phase 2 전환 시 AdapterRegistry에서 구현체만 교체하면 됨
- **단, `BaseHttpClient`가 OpenStack `X-Auth-Token` 방식에 고착되어 있음**
- `AuthProvider`에게 `sign_request(RequestBuilder) -> RequestBuilder` 형태의 책임 위임 필요

**의장 판정**: 구조적으로 적절하나 **1건 보완 필요** (액션 아이템 #1, #2)

---

### 2. Component 시스템 + RBAC (상태머신 및 필터링 구조)

**Codex 탐색 결과**:
- 16개 모듈 Summary Table에서 Admin-only, 2-Step Confirm 등 일관 정의 확인
- `RbacGuard::is_admin()` 기반 런타임 체크가 모듈별 적용됨
- `App::components` 등록 시점에서 RBAC 필터링 가능

**Gemini 의견**:
- ViewState 상태머신 + Component trait 공통 패턴은 적절
- `RbacGuard`가 Keystone 역할에 종속 — Phase 2에서 다른 백엔드 권한 체계 혼합 시 `RequiredRole` 열거형만으로 부족
- `Permission` 테이블을 리소스/액션 단위 Capability 기반으로 추상화 권장

**의장 판정**: Phase 1은 적절, **Phase 2 대비 1건 보완 필요** (액션 아이템 #3)

---

### 3. 멀티 백엔드 인증 (AuthProvider 추상화)

**Codex 탐색 결과**:
- `AuthMethod` enum에 `HmacKey` 변형 계획 확인
- `AuthProvider` trait 인터페이스가 `get_token() -> String` 중심 — HMAC 서명 주입 경로 부재
- `KeystoneAuthAdapter`가 `broadcast::Sender<Token>`으로 토큰 갱신 전파하는 구조 확인

**Gemini 의견**:
- **가장 중요한 보완 포인트**
- `get_token() -> String`은 HMAC 방식 수용에 한계
- HMAC은 요청 경로/바디/시간을 조합해 매 요청마다 서명 생성 필요
- `authenticate_request(method, url, headers, body) -> AuthHeaders` 메서드 추가 필수

**의장 판정**: **설계 보완 필수** (액션 아이템 #1, #2)

---

### 4. 전체 의존성 방향 (순환 의존 검증)

**Codex 탐색 결과**:
- `detail-design.md`의 Dependency Graph에서 순환 의존 없음 확인
- `App → Router/Component/ActionDispatcher/Cache/RbacGuard` 단방향 구조
- `ActionDispatcher → Arc<dyn *Service>` (Port trait 의존)으로 Adapter 직접 참조 없음

**Gemini 의견**:
- DIP를 정확히 준수. Infrastructure가 Arc 주입으로 순환 방지
- Domain → UI Widget 직접 의존은 TUI 특성상 불가피
- Domain Model이 UI 위젯 파라미터에 직접 결합되지 않도록 DTO/ViewModel 분리 권장

**의장 판정**: **순환 의존 없음 확인**, 1건 권고 (액션 아이템 #4)

---

## 합의된 액션 아이템

| # | 우선순위 | 내용 | 반영 시점 |
|---|---------|------|----------|
| 1 | **필수** | `AuthProvider` trait에 `authenticate_request()` 메서드 추가 — 토큰 주입(Keystone)과 요청 서명(HMAC)을 동일 추상화 수준으로 처리 | Phase 1 설계 반영 |
| 2 | **필수** | `BaseHttpClient`가 인증 헤더를 직접 주입하지 않고 `AuthProvider::authenticate_request()`에 위임 | Phase 1 설계 반영 |
| 3 | **권고** | `RbacGuard`에 Capability 기반 확장 경로 마련 — `can_perform(resource, action) -> bool` 인터페이스 준비 | Phase 1 인터페이스, Phase 2 구현 |
| 4 | **권고** | Domain Module의 UI 표현 로직을 `view_model` 모듈로 분리 — Domain Model과 UI 위젯 파라미터의 결합도 감소 | Phase 2 리팩토링 |

## 종합 평가

전체적으로 확장성과 유지보수성이 고려된 수준 높은 설계. **Point 3(HMAC 대응)**에 대한 인터페이스 보완과 **Point 1/2**에서의 **인증 체계 중립적 설계**가 추가되면 Phase 2로의 전환이 매우 매끄러울 것으로 판단됨.
