# Application Design

**Mode**: DETAIL (상세 설계 완료)
**Timestamp**: 2026-03-23T10:30:00+09:00
**Depth**: Comprehensive (NFR Design Patterns 포함)

## 상세 설계 문서

상세 설계는 레이어별로 분리된 문서에 기술:

| 문서 | 대상 | 컴포넌트 수 |
|------|------|-----------|
| [detail-design.md](detail-design.md) | Core / Application Layer + Infrastructure | 10개 |
| [detail-design-port-adapter.md](detail-design-port-adapter.md) | Port Layer (Service Traits) + Adapter Layer (HTTP 구현) | 13개 |
| [detail-design-ui-input.md](detail-design-ui-input.md) | UI Widgets + Input / Navigation | 13개 |
| [detail-design-domain-nfr.md](detail-design-domain-nfr.md) | Domain Modules (Component 구현) + NFR Design Patterns | 16개 + 5 NFR |

**총 52개 컴포넌트 + 5개 NFR 카테고리 상세 설계 완료**

## Agent Council Review

[agent-council-review.md](agent-council-review.md) — Codex + Gemini + Claude 3자 리뷰 (2026-03-23)

**반영된 액션 아이템**:
1. `AuthProvider` trait에 `authenticate_request()` + `get_capabilities()` 메서드 추가 (HMAC/API Key 대응)
2. `BaseHttpClient`가 인증을 `AuthProvider::authenticate_request()`에 위임 (인증 체계 중립)
3. `RbacGuard`에 Capability 기반 확장 경로 (`has_capability()`, `update_capabilities()`)
4. Domain Module에 ViewModel 분리 패턴 (`view_model.rs` 서브모듈)

## 컴포넌트 목록

### Core / Application Layer (5개)

| 컴포넌트 | 책임 | 타입 |
|---------|------|------|
| `App` | 메인 오케스트레이터 — 라우팅, 컴포넌트 등록, 전역 상태(route, auth, quit) 관리 | Controller |
| `EventLoop` | tokio::select! 통합 루프 — 키 입력, 틱, 백그라운드 이벤트 디스패치 | Controller |
| `Router` | Route enum 기반 활성 컴포넌트 결정 및 전환 | Controller |
| `ActionDispatcher` | Action → 백그라운드 작업 spawn, mpsc 채널 관리 (action_tx/event_rx) | Service |
| `BackgroundTracker` | 진행 중 작업 상태 추적 (InProgress/Completed/Failed), Toast 알림 트리거 | Service |

### UI Widgets — 공통 재사용 (10개)

| 컴포넌트 | 책임 | 타입 |
|---------|------|------|
| `LayoutManager` | 메인 레이아웃 계산 (Header/Sidebar/Content/InputBar/StatusBar), 리사이즈 대응 | Util |
| `Header` | 상단바 — 리소스 타입, 클라우드명, 리전 표시 | Util |
| `Sidebar` | 토글 가능 모듈 목록 (Tab 키), RBAC 기반 메뉴 필터링 | Util |
| `InputBar` | 통합 입력바 — 커맨드 모드(`:`) / 검색 모드(`/`) 전환 | Util |
| `StatusBar` | 하단 상태바 — 메시지, 통계, 도움말, Toast 알림 표시 | Util |
| `ResourceList` | 범용 테이블 위젯 — 칼럼 정의 기반 렌더링, 선택/스크롤/검색 하이라이트 | Util |
| `DetailView` | 범용 상세 뷰 — 키-값 섹션, 중첩 테이블, 리소스 간 연관 링크 | Util |
| `FormWidget` | 동적 폼 — 텍스트/드롭다운/멀티셀렉트/체크박스, 필드 검증, Tab 이동 | Util |
| `ConfirmDialog` | 모달 확인 — Y/N 및 리소스명 재입력(2단계 확인) | Util |
| `Toast` | 일시적 알림 — 성공/에러/정보 색상, TTL 자동 제거 | Util |

### Input / Navigation (3개)

| 컴포넌트 | 책임 | 타입 |
|---------|------|------|
| `CommandParser` | `:` 커맨드 파싱, Tab 자동완성, 축약어 매핑, 히스토리 관리 | Service |
| `SearchFilter` | `/` 검색 — 현재 리스트 실시간 텍스트 필터링 | Service |
| `KeyMap` | Vi 스타일 키 바인딩 매핑 (j/k/G/g/Enter/Esc 등) | Util |

### Port Layer — Service Traits (6개)

| 컴포넌트 | 책임 | 타입 |
|---------|------|------|
| `AuthProvider` | 인증 추상화 trait — 토큰 발급/갱신, 서비스 카탈로그. Phase 2에서 HMAC/API Key 추가 | Service |
| `NovaPort` | Nova API trait — 서버/플레이버/Aggregate/ComputeService/Migration/Evacuate | Service |
| `NeutronPort` | Neutron API trait — 네트워크/보안그룹/FloatingIP/Agent | Service |
| `CinderPort` | Cinder API trait — 볼륨/스냅샷/QoS/StoragePool/Migration | Service |
| `KeystonePort` | Keystone Admin API trait — 프로젝트/사용자/역할/Quota | Service |
| `GlancePort` | Glance API trait — 이미지 CRUD | Service |

### Adapter Layer — HTTP 구현 (7개)

| 컴포넌트 | 책임 | 타입 |
|---------|------|------|
| `KeystoneAuthAdapter` | Keystone v3 토큰 발급/갱신, 서비스 카탈로그 파싱, 토큰 만료 전 자동 갱신 | Adapter |
| `NovaHttpAdapter` | Nova REST API 호출 → 도메인 모델 변환 (reqwest + serde) | Adapter |
| `NeutronHttpAdapter` | Neutron REST API 호출 → 도메인 모델 변환 | Adapter |
| `CinderHttpAdapter` | Cinder REST API 호출 → 도메인 모델 변환 | Adapter |
| `KeystoneHttpAdapter` | Keystone Admin REST API 호출 (프로젝트/사용자/역할/Quota) | Adapter |
| `GlanceHttpAdapter` | Glance REST API 호출 → 도메인 모델 변환 | Adapter |
| `AdapterRegistry` | 설정 기반 adapter 인스턴스 생성·주입, 런타임 백엔드 선택. Phase 2에서 Service Layer adapter 추가 | Adapter |

### Domain Modules — Component trait 구현 (16개)

| 컴포넌트 | 책임 | 타입 |
|---------|------|------|
| `ServerModule` | 서버 리스트/상세/생성폼/기본액션(삭제/리부트/시작/중지)/이벤트/스냅샷 | Controller |
| `MigrationModule` | 서버 마이그레이션(Live/Block/Cold)/Evacuate/상태강제변경 (Admin) | Controller |
| `FlavorModule` | 플레이버 리스트/생성/삭제 | Controller |
| `NetworkModule` | 네트워크 리스트/상세/생성 | Controller |
| `SecurityGroupModule` | 보안그룹 리스트/상세/CRUD + 룰 추가/삭제 | Controller |
| `FloatingIpModule` | Floating IP 리스트/생성/삭제/Associate/Disassociate | Controller |
| `AgentModule` | Network Agent 리스트/Enable/Disable/삭제 (Admin) | Controller |
| `VolumeModule` | 볼륨 리스트/상세/생성/액션(삭제/확장/연결/분리/강제삭제/상태변경) | Controller |
| `SnapshotModule` | 볼륨 스냅샷 리스트/상세/삭제 | Controller |
| `ImageModule` | 이미지 리스트/상세/등록/수정/삭제 | Controller |
| `ProjectModule` | 프로젝트 리스트/생성/삭제 + Quota 관리 (Admin) | Controller |
| `UserModule` | 사용자 리스트/생성/삭제 + 역할 부여/회수 (Admin) | Controller |
| `AggregateModule` | Aggregate 리스트/상세/CRUD + 호스트 추가/제거 (Admin) | Controller |
| `ComputeServiceModule` | Compute Service 리스트/Enable/Disable (Admin) | Controller |
| `HypervisorModule` | Hypervisor 리스트/상세 (Admin, 읽기 전용) | Controller |
| `UsageModule` | 프로젝트별 사용량 조회 + 기간 필터 (Admin) | Controller |

### Infrastructure (5개)

| 컴포넌트 | 책임 | 타입 |
|---------|------|------|
| `Config` | clouds.yaml 파싱 + 앱 설정 로드 (~/.config/nexttui/), 멀티 클라우드 정의 | Util |
| `Cache` | HashMap + TTL 단일 레벨 캐시, 리소스 타입별 TTL 설정, `:refresh` 강제 무효화 | Service |
| `RbacGuard` | Keystone 역할 기반 메뉴/액션 가시성 판별, Admin 전용 기능 필터링 | Service |
| `AuditLogger` | CUD 작업 로컬 감사 로그 기록 (~/.config/nexttui/audit.log), 민감 정보 마스킹 | Service |
| `ServiceCatalog` | Keystone 서비스 카탈로그 저장, 서비스별 엔드포인트 디스커버리 | Service |

---

**총 52개 컴포넌트** (Core 5 + UI Widget 10 + Input 3 + Port 6 + Adapter 7 + Domain 16 + Infrastructure 5)
