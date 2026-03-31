# Backlog

## Pending

### BL-002: Snapshots 서비스 타입 매핑 수정
**Priority**: Low
**Category**: Bug
**Description**: FetchSnapshots가 "Service unavailable:" 에러 발생. Cinder snapshot API가 volume 서비스와 동일 엔드포인트를 사용하는데 별도 서비스로 조회하는 문제.

### BL-003: DevStack Glance↔Nova 통신 오류 조사
**Priority**: Low
**Category**: Infra
**Description**: Server 생성 시 500 에러 (GlanceConnection). DevStack 환경에서 Glance 이미지 접근 경로 확인 필요. nexttui 코드 문제 아님.

---

## Phase 2 Backlog

> Substation(Swift TUI) 리버스 엔지니어링 분석 + Phase 1 defer 항목 기반.
> Stage 1 = 아키텍처 고도화, Stage 2 = 기능 확장, Stage 3 = 신규 백엔드.

### Stage 1: 아키텍처 고도화

#### BL-P2-002: Multi-Level Cache (L1/L2/L3)
**Priority**: Medium
**Category**: Architecture
**Description**:
- 현재 L1(인메모리 HashMap) 단일 레벨 → 3단계 티어링 도입
  - L1: 인메모리 (현재와 동일, 가장 빠름)
  - L2: gzip 압축 메모리 (대용량 리소스 목록의 메모리 사용량 절감)
  - L3: 디스크 영속 (앱 재시작 시 콜드 스타트 없이 즉시 표시)
- 리소스 종류별 차등 TTL (현재 구현됨) + 티어별 승격/강등 정책
- `cloud` 필드 기반 멀티 클라우드 캐시 격리 (현재 CacheKey에 cloud 있으나 L1만 활용)
- L3 디스크 캐시: index/data 일관성 보장, 비정상 종료 대비 WAL 또는 atomic write
**Motivation**: VDI 환경에서 앱 재시작이 잦고 서버 목록이 수천 건일 때 매번 API 전체 fetch는 비효율적. L2 압축으로 메모리 절약, L3 영속으로 콜드 스타트 개선
**Ref**: Substation `MultiLevelCacheManager.swift`, `OpenStackCacheManager.swift`
**주의**: Substation의 3중 캐시(OSClient L1/L2/L3 + TUI cacheManager + DataManager MemoryKit) 간 일관성 문제(Risk #1)를 반복하지 않도록, nexttui에서는 단일 Cache 구조체 내에서 티어링 구현

#### BL-P2-003: Intelligent Cache Invalidation (의존성 그래프 기반)
**Priority**: High
**Depends on**: BL-P2-002
**Category**: Architecture
**Description**:
- 리소스 간 의존성 그래프 구축 (예: Server → FloatingIP, Port, Volume)
- CUD 액션 완료 시 관련 리소스 캐시를 연쇄 무효화
- 시간 지연 무효화 지원 (3~10초, API 전파 시간 고려)
- 현재 Cache(RwLock, TTL, GC)를 확장하되 Substation의 3중 캐시 복잡성은 피함
**Motivation**: 서버 삭제 후 FloatingIP 목록에 여전히 연결된 IP가 보이는 등 캐시 불일치 문제 예방. 현재는 단순 TTL 만료에만 의존
**Ref**: Substation `IntelligentCacheInvalidation.swift`

#### BL-P2-003-B: DataProvider Registry 패턴
**Priority**: Medium
**Category**: Architecture
**Description**:
- 문자열 키 기반 DataProvider 조회 (`DataProviderRegistry.fetch("servers")`)
- 각 DataProvider가 Port를 통해 API 호출 → 결과 캐싱 → 갱신 이벤트 발행
- `refreshAllDataOptimized()`: Phase 1(독립) 리소스 우선 로드 → Phase 2/3 순차 로드
- `executeWithTokenRefresh` 래핑: 토큰 만료 시 자동 재인증 후 재시도
**Motivation**: 현재 각 모듈이 개별적으로 데이터를 fetch하는데, 공통 DataProvider 레이어로 캐시/재시도/우선순위를 중앙 관리
**Ref**: Substation `DataManager.refreshAllDataOptimized()`, `ServersDataProvider`

#### BL-P2-004: Adaptive Polling (이벤트 루프 최적화)
**Priority**: Low
**Category**: Performance
**Description**:
- 활성 입력 감지 시: 짧은 폴링 간격 (5ms)
- 유휴 상태: 지수 백오프로 최대 30ms까지 증가
- 현재 crossterm 이벤트 루프의 고정 폴링 간격을 적응형으로 전환
**Motivation**: VDI 환경에서 CPU 사용량 최소화. 입력 시 빠른 반응성 유지하면서 유휴 시 CPU 양보
**Ref**: Substation `TUI.swift` nodelay 모드 + 지수 백오프

#### BL-P2-005: ViewModel 분리 (Domain Model ↔ UI 표현 결합도 감소)
**Priority**: Medium
**Category**: Architecture
**Description**:
- Domain Module의 UI 표현 로직(컬럼 정의, 색상, 포맷팅)을 `view_model` 모듈로 분리
- Domain Model은 순수 데이터, ViewModel이 UI 위젯 파라미터 변환 담당
- UI 위젯 변경 시 view_model만 수정, Domain Model과 Component 로직은 무변경
**Motivation**: Agent Council Review 액션 아이템 #4. 현재 모듈 내에서 모델과 UI 표현이 혼재
**Ref**: Council review `agent-council-review.md` 항목 4

#### BL-P2-006: Microversion 협상
**Priority**: Low
**Category**: Infrastructure
**Description**:
- 서비스별 지원 API 마이크로버전을 자동 협상
- `X-OpenStack-Nova-Microversion` 등 헤더 자동 주입
- 서비스 카탈로그에서 버전 정보 추출 → 요청별 적절한 버전 헤더 설정
**Motivation**: OpenStack 서비스는 동일 엔드포인트에서 마이크로버전별로 응답 스키마가 달라짐. 현재는 고정 버전 또는 버전 미지정으로 호출
**Ref**: Substation `OpenStackClientCore.swift` MicroversionManager

### Stage 2: 기능 확장

#### BL-P2-010: RBAC / Capability 기반 권한 제어
**Priority**: High
**Category**: Feature
**Description**:
- `RbacGuard`에 Capability 기반 확장 경로 구현: `can_perform(resource, action) -> bool`
- Phase 1의 Keystone 역할 기반에서, 다른 백엔드 권한 체계(HMAC, API Key 등)도 수용
- 사용자 역할에 따라 메뉴/액션 자동 필터링
**Motivation**: Agent Council Review 액션 아이템 #3. 멀티 백엔드 환경에서 RequiredRole 열거형만으로는 부족
**Ref**: Council review `agent-council-review.md` 항목 3, `detail-design.md` Phase 2 Capability 주석

#### BL-P2-011: 감사 로그 (Audit Log)
**Priority**: Medium
**Category**: Feature
**Description**:
- `~/.config/nexttui/audit.log`에 JSON Lines 형식 기록
- `AuditLogger`가 Action 채널 구독, 매 기록 즉시 flush
- 기록 항목: 타임스탬프, 사용자, 액션 종류, 대상 리소스, 결과(성공/실패)
- Log rotation 지원
**Motivation**: FR-18 (감사 로그). 운영 환경에서 누가 어떤 작업을 했는지 추적 필요
**Ref**: `detail-design-domain-nfr.md` 항목 G, user-story US-047

#### BL-P2-012: 통합 조회 (서버-리소스 연관 뷰)
**Priority**: Medium
**Category**: Feature
**Description**:
- 서버 상세에서 연결된 Volume, FloatingIP, SecurityGroup, Network를 한 화면에 표시
- 리소스 간 연관 관계 그래프 기반 탐색
**Motivation**: FR-19 (통합 조회). 현재 각 리소스를 개별 모듈에서만 볼 수 있어 서버 전체 상태 파악이 어려움
**Ref**: user-story US-048

#### BL-P2-013: UsageModule (리소스 사용량 모니터링)
**Priority**: Medium
**Category**: Feature
**Description**:
- Nova simple-tenant-usage API 연동
- 프로젝트별 vCPU, RAM, 디스크 사용량 표시
- 쿼터 대비 사용률 시각화
**Motivation**: Phase 1에서 deferred. 운영자가 프로젝트별 리소스 현황을 빠르게 확인할 수 있어야 함
**Ref**: session-summary Unit 14 (UsageModule deferred)

#### BL-P2-014: Server Migration / Evacuate
**Priority**: Medium
**Category**: Feature
**Description**:
- **Live Migration** (US-024): 실행 중 서버를 다른 호스트로 이동, 선택적 대상 호스트 지정
- **Block Migration** (US-024): 로컬 디스크 복사 포함 live migration
- **Cold Migration** (US-024): 서버 중지 → 이동 → 수동 confirm/revert 플로우
- **Evacuate** (US-025): 장애 호스트에서 서버 복구, 대상 호스트 선택
- 모두 Nova Admin API (`POST /servers/{id}/action`)
- 진행 상태 실시간 표시 (migration state in detail view)
- Admin이 아니면 메뉴 숨김 (RBAC 연동, BL-P2-010 선행 권고)
**Testing 제약**: DevStack 단일 노드에서는 `NoValidHost` 에러로 실행 불가. demo 모드에서 UI/확인 플로우만 검증, 실제 API 테스트는 멀티 노드 환경에서 수행
**Motivation**: Phase 1에서 defer된 Admin 운영 핵심 액션
**Ref**: user-stories US-024, US-025

#### BL-P2-015: Attach / Detach / Associate 워크플로우
**Priority**: Medium
**Category**: Feature
**Description**:
- Volume Attach/Detach: 서버에 볼륨 연결/분리
- Volume Migration: 스토리지 백엔드 간 볼륨 이동 (US-032)
- FloatingIP Associate/Disassociate
- Role Grant/Revoke
- Quota Management
**Motivation**: Phase 1에서 defer된 CUD 확장. 빈번한 정기 운영 액션
**Ref**: session-summary Next Steps, user-stories US-027, US-030~032

#### BL-P2-032: 전체 프로젝트 리소스 조회 (all_tenants)
**Priority**: High
**Category**: Feature
**Depends on**: BL-P2-010 (RBAC)
**Description**:
- 모든 프로젝트 리소스를 한눈에 조회 (admin 전용)
- 모델에 tenant_id 추가, Neutron 리소스에 all_tenants 필터 추가
- UI에 전체/내 프로젝트 토글, 프로젝트 컬럼 동적 표시
**Motivation**: 운영자가 클라우드 전체 리소스 현황을 파악해야 함
**Ref**: GitHub Issue #41

#### BL-P2-016: 토큰 보안 강화
**Priority**: Low
**Category**: Security
**Description**:
- 메모리 내 토큰 암호화 저장 (AES-GCM 또는 OS keychain)
- 선제 갱신: 만료 5분 전 자동 갱신 시작
- HTTP 429/5xx 지수 백오프 재시도 (최대 3회)
**Motivation**: Substation이 AES-GCM으로 토큰 보호. 현재 nexttui는 plaintext 토큰 보유
**Ref**: Substation `OpenStackClientCore.swift` CoreTokenManager

#### BL-P2-017: 멀티 인증 방식 지원
**Priority**: Low
**Category**: Feature
**Description**:
- Keystone v3 password (현재) + appCredential + token 인증 지원
- HMAC (Cloudian), API Key 등 비-Keystone 인증 확장
- `AuthProvider` trait에 `sign_request()` 메서드 확장 (현재 Phase 2 주석만 존재)
**Motivation**: FR-05.4, FR-05.5. 멀티 백엔드 환경에서 인증 체계 중립적 설계 필요
**Ref**: `detail-design-port-adapter.md` Phase 2 HMAC/API Key 주석

#### BL-P2-033: TestBackend 스냅샷 테스트 확장
**Priority**: Low
**Category**: Testing / UX Verification
**Description**:
- 현재 FormWidget에만 있는 `TestBackend` 렌더 테스트를 핵심 UI 전체로 확장
- `insta` 크레이트 도입으로 스냅샷 기반 렌더링 회귀 테스트 구축
- 대상: 서버 리스트 테이블, 디테일 패널, 네비게이션 바, 상태 바 등
- 다양한 터미널 크기(40x10, 80x24, 120x40 등)에서 레이아웃 깨짐 검증
- 키 입력 후 상태 변화 → 재렌더 → 스냅샷 비교 시나리오
**Motivation**: 현재 580+ 테스트가 로직/상태 전이를 검증하지만, 실제 렌더링 출력(프레젠테이션 레이어)은 FormWidget 5개 테스트만 커버. UI 변경 시 레이아웃 깨짐이나 스타일 변경을 자동 감지할 수 없음
**Ref**: `src/ui/form.rs:2160` 기존 `render_to_buffer` 헬퍼

#### BL-P2-018: 커스텀 키 바인딩
**Priority**: Low
**Category**: UX
**Description**:
- Config 파일에서 키 바인딩 커스터마이징 로드
- 기본 키맵 + 사용자 오버라이드
**Motivation**: detail-design-ui-input.md에서 Phase 2로 분류
**Ref**: `detail-design-ui-input.md` Config 항목

#### BL-P2-019: 이미지 로컬 파일 업로드
**Priority**: Low
**Category**: Feature
**Description**:
- Glance 이미지 생성 시 URL 지정 외에 로컬 파일 업로드 지원
- 파일 선택 UI + 진행률 표시
**Motivation**: Phase 1은 URL 지정만 지원
**Ref**: `detail-design-domain-nfr.md` line 1171

#### BL-P2-020: Service Layer 전환 대비
**Priority**: Low
**Category**: Architecture
**Description**:
- AdapterRegistry에서 직접 호출 Adapter를 Service Layer 프록시 Adapter로 교체
- Admin API GW 경유 모드 지원
- `replace_*()` 메서드 활용한 런타임 Adapter 스왑
**Motivation**: TR-09. Phase 1의 Thick Client에서 Phase 2의 Service Layer 중심으로 점진 전환
**Ref**: `detail-design-port-adapter.md` Phase 2 Adapter Swap 섹션

### Stage 2.5: UI/UX Redesign

> devflow-tui + btop 참조 UI 개선. 3단계 구현.
> 분석 문서: `devflow-docs/inception/ui-redesign-analysis.md`

#### Stage 2.5-A: Theme & Polish (High Priority)

##### BL-P2-034: Theme 시스템 도입
**Priority**: High
**Category**: UX
**Description**:
- `src/ui/theme.rs` 신규: `Theme` 구조체로 모든 색상/스타일 중앙화
- devflow-tui 패턴 참조: `active()`, `done()`, `error()`, `waiting()`, `focus_border()`, `unfocus_border()`, `highlight()`, `disabled()`
- `key_hint()`, `panel_title()`, `status_span()` 유틸 함수
- `Icons` 구조체: `●✓○✗⟳◐` 상태 아이콘 정의
- 기존 하드코딩된 색상을 Theme 호출로 교체
**Motivation**: 색상이 각 파일에 하드코딩되어 있어 일관성 유지 어려움. 중앙화로 테마 변경 용이
**Ref**: `devflow-tui/src/ui/theme.rs`, `ui-redesign-analysis.md` Section 6

##### BL-P2-035: Rounded 보더 + 포커스 피드백
**Priority**: High
**Category**: UX
**Depends on**: BL-P2-034
**Description**:
- Sidebar + Content 영역 모두 `BorderType::Rounded` 적용
- 포커스 상태: `Theme::focus_border()` (Cyan), 비포커스: `Theme::unfocus_border()` (DarkGray)
- 기존 Sidebar RIGHT-only border → 전체 Block border로 변경
- Content 영역에 Block 컨테이너 추가
**Motivation**: 현재 Content 영역에 보더 없어 텍스트가 떠다니는 느낌. 포커스 상태 불명확

##### BL-P2-036: 패널 타이틀 포맷
**Priority**: High
**Category**: UX
**Depends on**: BL-P2-034
**Description**:
- 포커스 패널: `[ Panel Name ]` (bracket 포함)
- 비포커스 패널: `  Panel Name  ` (space padding)
- `theme::panel_title(name, focused)` 함수 활용
**Motivation**: devflow-tui에서 검증된 패턴. 포커스 상태를 타이틀에서도 즉시 식별 가능

##### BL-P2-037: 상태바 리디자인
**Priority**: High
**Category**: UX
**Depends on**: BL-P2-034
**Description**:
- 배경: `on_dark_gray().white()` (현재는 투명)
- 좌측: `[패널명] context` (예: `[Servers] 1/5`)
- 우측: key hints — key=Cyan Bold + description=Dim
- `theme::key_hint(key, desc)` 활용
- Toast 표시 시 상태바 오버라이드 유지
**Motivation**: 현재 상태바 Gray 힌트 가독성 낮음. devflow-tui 패턴이 더 명확

##### BL-P2-038: 리스트 하이라이트 개선
**Priority**: High
**Category**: UX
**Depends on**: BL-P2-034
**Description**:
- 선택 행: Black on White → White Bold (시맨틱 컬러 유지)
- ACTIVE 행 선택 시에도 Green 유지, ERROR는 Red 유지
- 선택 표시: `>` prefix 또는 White Bold modifier
**Motivation**: 현재 선택 시 시맨틱 컬러(Active=Green, Error=Red) 정보 손실

#### Stage 2.5-B: Visual Enhancement (Medium Priority)

##### BL-P2-039: 헤더 리디자인
**Priority**: Medium
**Category**: UX
**Depends on**: BL-P2-034
**Description**:
- 기본 스타일: Dim (전체)
- 앱명: White Bold (`nexttui`)
- 채움선: `─` 문자로 좌→우 연결
- 우측: `user@cloud | Region` (context 정보)
- devflow-tui 헤더 패턴 참조
**Motivation**: 현재 Blue 뱃지 배경이 과도. Dim 기반이 더 세련됨

##### BL-P2-040: 상태 아이콘 도입
**Priority**: Medium
**Category**: UX
**Depends on**: BL-P2-034
**Description**:
- Server 상태: `●` ACTIVE, `○` SHUTOFF, `✗` ERROR, `⟳` BUILD/RESIZE, `◐` VERIFY_RESIZE, `↔` MIGRATING
- Sidebar 모듈: 선택 마커 `>` → `▶` 또는 유지
- 리스트/디테일 뷰에서 상태 텍스트 앞에 아이콘 표시
- `Theme::Icons` 구조체 활용
**Motivation**: 색상만으로는 접근성 부족. 아이콘으로 추가 시각 채널 제공

##### BL-P2-041: 스크롤바 추가
**Priority**: Medium
**Category**: UX
**Description**:
- 리스트 뷰: 우측 수직 스크롤바 (block characters: `█▐│`)
- 디테일 뷰: 스크롤 가능 시 스크롤바 표시
- 현재 위치/전체 비율 시각화
**Motivation**: 사용자가 더 많은 데이터 존재 여부를 알 수 없음

##### BL-P2-042: Content 보더 컨테이너
**Priority**: Medium
**Category**: UX
**Depends on**: BL-P2-035
**Description**:
- List/Detail/Form 뷰를 Block 컨테이너로 감싸기
- 뷰 상태에 따라 타이틀 변경: `[ Servers ]`, `[ Server: web-01 ]`, `[ Create Server ]`
**Motivation**: Content 영역이 시각적 컨테이너 없이 플로팅

##### BL-P2-043: Detail 섹션 구분 개선
**Priority**: Medium
**Category**: UX
**Depends on**: BL-P2-035
**Description**:
- `-- Section --` 대시 구분 → Block title 또는 bold underline 섹션 헤더
- 섹션 간 1줄 공백 유지
- ResourceLink 포커스 시 시각적 피드백 (reverse 또는 bold)
**Motivation**: ASCII 대시 구분자가 올드 스타일

#### Stage 2.5-C: Advanced Layout (Low Priority)

##### BL-P2-044: 반응형 레이아웃 모드
**Priority**: Low
**Category**: UX
**Description**:
- `LayoutMode` enum: TooSmall / Compact / Standard / Wide
- Compact (< 120x30): Sidebar 숨김, 단일 패널
- Standard (120x30+): 현재 Sidebar + Content
- Wide (200x50+): Sidebar + List + Detail 동시 표시 (3-column)
- devflow-tui `LayoutManager` 패턴 참조
**Motivation**: 터미널 크기에 따라 최적 레이아웃 제공

##### BL-P2-045: 다크 테마 옵션
**Priority**: Low
**Category**: UX
**Depends on**: BL-P2-034
**Description**:
- Config에서 `theme: dark | light` 선택
- 다크 테마: btop 스타일 어두운 배경 + 밝은 텍스트
- Theme 구조체에 variant 추가
**Motivation**: btop처럼 다크 테마가 장시간 모니터링에 적합

##### BL-P2-046: NO_COLOR 접근성 지원
**Priority**: Low
**Category**: UX
**Depends on**: BL-P2-034
**Description**:
- `NO_COLOR` 환경변수 감지 시 색상 대신 Bold/Dim/Underline만 사용
- devflow-tui `no_color()` 패턴 참조
- 모든 Theme 메서드에 NO_COLOR 분기 추가
**Motivation**: 터미널 접근성 표준 (https://no-color.org)

### Stage 3: 신규 백엔드

#### BL-P2-021: Manila (Shared FS / NAS)
**Priority**: Low
**Category**: New Backend
**Description**: Share Network, Migration, QoS, CIFS Account 관리
**Ref**: user-stories Phase 2 예정 서비스

#### BL-P2-022: Cloudian (Object Storage)
**Priority**: Low
**Category**: New Backend
**Description**: Policy, Bucket, Group, Monitor, Permission, QoS 관리. S3 HMAC 인증 필요 (BL-P2-017 선행)
**Ref**: user-stories Phase 2 예정 서비스

#### BL-P2-023: Network System Admin
**Priority**: Low
**Category**: New Backend
**Description**: Routing Table, VPC, Subnet, Routing Rule, NACL, External Network 관리
**Ref**: user-stories Phase 2 예정 서비스

#### BL-P2-024: Placement
**Priority**: Low
**Category**: New Backend
**Description**: Resource Provider, Inventory 조회/관리
**Ref**: user-stories Phase 2 예정 서비스

## Completed
- **BL-001**: Submit 확인 화면 + Toast 피드백 (PR #27 merged, 2026-03-25)
- **BL-P2-001**: Module Registry 시스템 (PR #28 merged, 2026-03-25)
- **#32 BL-P2-027**: Error enum `#[non_exhaustive]` 적용 (PR #36, 2026-03-26)
- **#30 BL-P2-025**: Clippy 엄격 lint 정책 도입 (PR #36, 2026-03-26)
- **#35 BL-P2-030**: Pagination Combinator 추상화 (PR #36, 2026-03-26)
- **#31 BL-P2-026**: tracing 구조적 로깅/계측 도입 (PR #37, 2026-03-26)
- **#33 BL-P2-028**: 토큰 캐시 파일 영속화 (PR #38, 2026-03-26)
