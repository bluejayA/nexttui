# Units Generation

**Timestamp**: 2026-03-23T14:00:00+09:00
**Depth**: Standard
**Source**: application-design.md (52 components) + user-stories.md (48 stories)
**Total Units**: 15

## Dependency Graph

```
Unit 1 (Foundation)
  |
  v
Unit 2 (Core Runtime) --- Unit 3 (Port Layer)
  |                          |
  v                          v
Unit 4 (Infrastructure) --- Unit 5 (Auth Adapter)
  |
  v
Unit 6 (UI Widgets) --- Unit 7 (Input System)
  |
  +---> Unit 8  (Nova: Server + Flavor)
  +---> Unit 9  (Neutron: Network + SG + FIP)
  +---> Unit 10 (Cinder: Volume + Snapshot)
  +---> Unit 11 (Glance: Image)
  +---> Unit 12 (Identity: Project + User + Role + Quota)
  |
  +---> Unit 13 (Nova Admin: Migration + Aggregate + ComputeService + Hypervisor)
  +---> Unit 14 (Neutron/Cinder Admin + Monitoring)
  |
  v
Unit 15 (Integration: AdapterRegistry + RBAC Wiring + Polish)
```

## Units

---

### Unit 1: foundation
**Responsibility**: 프로젝트 구조 초기화, Cargo.toml 의존성, Config(clouds.yaml 파싱), 에러 타입, 도메인 모델 기본 구조체
**Dependencies**: none
**Interfaces**:
- `Config` struct: clouds.yaml 로드, 클라우드 정의 접근
- `AppError` / `Result<T>`: 전역 에러 타입
- Domain model structs: Server, Network, Volume, Image 등 기본 구조체 (serde Deserialize)
**Implementation order**: 1
**Stories**: TR-05 (단일 바이너리), TR-06 (시크릿 메모리 전용)
**Components**: `Config`

---

### Unit 2: core-runtime
**Responsibility**: App 오케스트레이터, EventLoop(tokio::select!), Router, ActionDispatcher, BackgroundTracker, mpsc 채널 통신
**Dependencies**: foundation
**Interfaces**:
- `App::run()`: 메인 진입점
- `EventLoop`: 키 입력 / 틱 / 백그라운드 이벤트 통합 루프
- `Router`: Route enum 기반 활성 컴포넌트 전환
- `ActionDispatcher`: Action → 백그라운드 spawn
- `BackgroundTracker`: 작업 상태 추적
- `Component` trait 정의 (handle_key, handle_event, render)
**Implementation order**: 2
**Stories**: US-008 (논블로킹 API), US-009 (백그라운드 알림)
**Components**: `App`, `EventLoop`, `Router`, `ActionDispatcher`, `BackgroundTracker`

---

### Unit 3: port-layer
**Responsibility**: 모든 서비스 Port trait 정의 + Mock adapter 구현 (테스트용)
**Dependencies**: foundation
**Interfaces**:
- `AuthProvider` trait: authenticate, refresh, get_capabilities
- `NovaPort` trait: servers, flavors, migrations, aggregates, compute services, hypervisors
- `NeutronPort` trait: networks, security groups, floating IPs, agents
- `CinderPort` trait: volumes, snapshots, QoS, storage pools
- `KeystonePort` trait: projects, users, roles, quotas
- `GlancePort` trait: images
- Mock 구현체: `MockNovaAdapter`, `MockNeutronAdapter` 등
**Implementation order**: 2 (Unit 2와 병렬 가능)
**Stories**: TR-01 (Port/Adapter 디커플링), TR-04 (Mock 테스트), TR-08 (서비스별 Port)
**Components**: `AuthProvider`, `NovaPort`, `NeutronPort`, `CinderPort`, `KeystonePort`, `GlancePort`

---

### Unit 4: infrastructure
**Responsibility**: Cache(HashMap+TTL), RbacGuard(역할 기반 가시성), AuditLogger(로컬 감사 로그), ServiceCatalog(엔드포인트 디스커버리)
**Dependencies**: foundation, port-layer
**Interfaces**:
- `Cache`: get/set with TTL, invalidate, resource type별 TTL 설정
- `RbacGuard`: is_admin, can_access, has_capability, 메뉴/액션 필터링
- `AuditLogger`: log_action (CUD), 민감 정보 마스킹
- `ServiceCatalog`: endpoint_for(service, interface)
**Implementation order**: 3
**Stories**: US-045 (RBAC 메뉴 제어), US-047 (로컬 감사 로그)
**Components**: `Cache`, `RbacGuard`, `AuditLogger`, `ServiceCatalog`

---

### Unit 5: auth-adapter
**Responsibility**: KeystoneAuthAdapter — Keystone v3 토큰 발급/갱신, 서비스 카탈로그 파싱, BaseHttpClient 기반 인증 요청 위임
**Dependencies**: foundation, port-layer, infrastructure
**Interfaces**:
- `KeystoneAuthAdapter` impl AuthProvider: authenticate, refresh_token, get_service_catalog
- `BaseHttpClient`: reqwest 기반 HTTP 클라이언트, AuthProvider 위임 인증
- clouds.yaml → 인증 플로우 (password, application_credential)
**Implementation order**: 4
**Stories**: US-001 (clouds.yaml 인증), US-002 (토큰 자동 갱신), US-003 (멀티 클라우드 전환)
**Components**: `KeystoneAuthAdapter`, + `BaseHttpClient` (공통 HTTP 인프라)

---

### Unit 6: ui-widgets
**Responsibility**: 모든 공통 UI 위젯 — LayoutManager, Header, Sidebar, StatusBar, Toast, ResourceList, DetailView, FormWidget, ConfirmDialog
**Dependencies**: core-runtime
**Interfaces**:
- `LayoutManager`: 메인 레이아웃 계산 (Header/Sidebar/Content/InputBar/StatusBar), 리사이즈
- `Header`: 리소스 타입, 클라우드명, 리전 표시
- `Sidebar`: 토글 모듈 목록, RBAC 필터링
- `StatusBar`: 메시지, 통계, 도움말
- `Toast`: TTL 기반 일시적 알림 (성공/에러/정보)
- `ResourceList`: 범용 테이블 (칼럼 정의, 선택, 스크롤, 검색 하이라이트)
- `DetailView`: 범용 상세 뷰 (키-값, 중첩 테이블)
- `FormWidget`: 동적 폼 (텍스트/드롭다운/멀티셀렉트/체크박스, 검증)
- `ConfirmDialog`: 모달 확인 (Y/N + 리소스명 재입력)
**Implementation order**: 5
**Stories**: US-004 (레이아웃+사이드바), US-005 (Vi 네비게이션), US-022 (동적 폼), US-023 (확인 다이얼로그), US-046 (2단계 확인)
**Components**: `LayoutManager`, `Header`, `Sidebar`, `StatusBar`, `Toast`, `ResourceList`, `DetailView`, `FormWidget`, `ConfirmDialog`

---

### Unit 7: input-system
**Responsibility**: CommandParser(커맨드 모드), SearchFilter(검색 필터링), KeyMap(Vi 키 바인딩)
**Dependencies**: core-runtime, ui-widgets
**Interfaces**:
- `CommandParser`: parse, autocomplete, history (파일 저장/로드)
- `SearchFilter`: filter_text, apply_to_list, clear
- `KeyMap`: Vi 스타일 키 바인딩 매핑 (j/k/G/g/Enter/Esc 등)
- `InputBar` 위젯과의 통합 (`:` 커맨드 모드, `/` 검색 모드)
**Implementation order**: 5 (Unit 6과 병렬 가능)
**Stories**: US-006 (커맨드 모드), US-007 (검색 필터링)
**Components**: `CommandParser`, `SearchFilter`, `KeyMap`

---

### Unit 8: nova-domain
**Responsibility**: NovaHttpAdapter + ServerModule + FlavorModule — 서버 CRUD/액션, 플레이버 관리
**Dependencies**: port-layer, auth-adapter, ui-widgets, input-system
**Interfaces**:
- `NovaHttpAdapter` impl NovaPort: 서버/플레이버 REST API 호출
- `ServerModule` impl Component: 리스트/상세/생성폼/삭제/리부트/시작/중지 + ViewModel
- `FlavorModule` impl Component: 리스트/생성/삭제 (Admin)
**Implementation order**: 6
**Stories**: US-010 (서버 리스트), US-011 (서버 상세), US-012 (서버 생성), US-013 (서버 액션), US-014 (플레이버)
**Components**: `NovaHttpAdapter`, `ServerModule`, `FlavorModule`

---

### Unit 9: neutron-domain
**Responsibility**: NeutronHttpAdapter + NetworkModule + SecurityGroupModule + FloatingIpModule
**Dependencies**: port-layer, auth-adapter, ui-widgets, input-system
**Interfaces**:
- `NeutronHttpAdapter` impl NeutronPort: 네트워크/보안그룹/FIP REST API 호출
- `NetworkModule` impl Component: 리스트/상세/생성
- `SecurityGroupModule` impl Component: 리스트/상세/CRUD + 룰 추가/삭제
- `FloatingIpModule` impl Component: 리스트/생성/삭제/Associate/Disassociate
**Implementation order**: 6 (Unit 8과 병렬 가능)
**Stories**: US-015 (네트워크), US-016 (네트워크 생성), US-017 (보안그룹), US-028 (Floating IP)
**Components**: `NeutronHttpAdapter`, `NetworkModule`, `SecurityGroupModule`, `FloatingIpModule`

---

### Unit 10: cinder-domain
**Responsibility**: CinderHttpAdapter + VolumeModule + SnapshotModule
**Dependencies**: port-layer, auth-adapter, ui-widgets, input-system
**Interfaces**:
- `CinderHttpAdapter` impl CinderPort: 볼륨/스냅샷 REST API 호출
- `VolumeModule` impl Component: 리스트/상세/생성/삭제/확장/연결/분리/강제삭제/상태변경
- `SnapshotModule` impl Component: 리스트/상세/삭제
**Implementation order**: 6 (Unit 8, 9와 병렬 가능)
**Stories**: US-018 (볼륨 리스트/상세), US-019 (볼륨 생성), US-020 (볼륨 액션), US-021 (스냅샷)
**Components**: `CinderHttpAdapter`, `VolumeModule`, `SnapshotModule`

---

### Unit 11: glance-domain
**Responsibility**: GlanceHttpAdapter + ImageModule
**Dependencies**: port-layer, auth-adapter, ui-widgets, input-system
**Interfaces**:
- `GlanceHttpAdapter` impl GlancePort: 이미지 REST API 호출
- `ImageModule` impl Component: 리스트/상세/등록/수정/삭제
**Implementation order**: 6 (Unit 8~10과 병렬 가능)
**Stories**: US-037 (이미지 리스트/상세), US-038 (이미지 등록), US-039 (이미지 수정/삭제)
**Components**: `GlanceHttpAdapter`, `ImageModule`

---

### Unit 12: identity-domain
**Responsibility**: KeystoneHttpAdapter(Admin) + ProjectModule + UserModule + Quota 관리
**Dependencies**: port-layer, auth-adapter, ui-widgets, input-system
**Interfaces**:
- `KeystoneHttpAdapter` impl KeystonePort: 프로젝트/사용자/역할/Quota Admin REST API
- `ProjectModule` impl Component: 리스트/생성/삭제 + Quota 관리
- `UserModule` impl Component: 리스트/생성/삭제 + 역할 부여/회수
**Implementation order**: 6 (Unit 8~11과 병렬 가능)
**Stories**: US-033 (프로젝트), US-034 (사용자), US-035 (역할), US-036 (Quota)
**Components**: `KeystoneHttpAdapter`, `ProjectModule`, `UserModule`

---

### Unit 13: nova-admin-domain
**Responsibility**: MigrationModule + AggregateModule + ComputeServiceModule + HypervisorModule — Nova Admin 확장 기능
**Dependencies**: nova-domain (NovaHttpAdapter 확장)
**Interfaces**:
- `MigrationModule` impl Component: Live/Block/Cold Migration, Evacuate, 상태 강제 변경
- `AggregateModule` impl Component: 리스트/상세/CRUD + 호스트 추가/제거
- `ComputeServiceModule` impl Component: 리스트/Enable/Disable
- `HypervisorModule` impl Component: 리스트/상세 (읽기 전용)
**Implementation order**: 7
**Stories**: US-024 (마이그레이션), US-025 (Evacuate), US-026 (상태 강제 변경), US-027 (서버 스냅샷), US-040 (Aggregate), US-041 (Compute Service), US-042 (Hypervisor)
**Components**: `MigrationModule`, `AggregateModule`, `ComputeServiceModule`, `HypervisorModule`

---

### Unit 14: admin-monitoring
**Responsibility**: AgentModule(Neutron Admin) + UsageModule(사용량 조회) + 서버 이벤트 + Cinder Admin(QoS/StoragePool/볼륨 마이그레이션)
**Dependencies**: neutron-domain, cinder-domain, nova-domain
**Interfaces**:
- `AgentModule` impl Component: Agent 리스트/Enable/Disable/삭제
- `UsageModule` impl Component: 프로젝트별 사용량 + 기간 필터
- Server Events: ServerModule 상세 뷰 내 이벤트 섹션 추가
- Cinder Admin: QoS 관리, StoragePool 조회, 볼륨 마이그레이션
**Implementation order**: 7 (Unit 13과 병렬 가능)
**Stories**: US-029 (Network Agent), US-030 (QoS), US-031 (StoragePool), US-032 (볼륨 마이그레이션), US-043 (사용량), US-044 (서버 이벤트)
**Components**: `AgentModule`, `UsageModule`

---

### Unit 15: integration
**Responsibility**: AdapterRegistry(설정 기반 adapter 생성/주입), RBAC 전체 연결, 서버-리소스 연관 뷰, 전체 통합 테스트
**Dependencies**: all previous units
**Interfaces**:
- `AdapterRegistry`: 설정 기반 adapter 인스턴스 생성, DI, 런타임 백엔드 선택
- RBAC wiring: RbacGuard + Sidebar/Module 연결 — Admin 전용 메뉴/액션 필터링
- Cross-resource view: 서버 상세에서 연관 볼륨/네트워크/FIP 링크 네비게이션
- End-to-end integration: 전체 앱 시작 → 인증 → 모듈 전환 → 액션 수행 흐름 검증
**Implementation order**: 8
**Stories**: US-048 (서버-리소스 연관 뷰), TR-09 (Service Layer 전환 대비)
**Components**: `AdapterRegistry`

---

## Implementation Order Summary

| Order | Units | 설명 |
|-------|-------|------|
| 1 | Unit 1 | Foundation |
| 2 | Unit 2, 3 | Core Runtime + Port Layer (병렬) |
| 3 | Unit 4 | Infrastructure |
| 4 | Unit 5 | Auth Adapter |
| 5 | Unit 6, 7 | UI Widgets + Input System (병렬) |
| 6 | Unit 8, 9, 10, 11, 12 | Domain Modules (병렬 — 각 서비스 독립) |
| 7 | Unit 13, 14 | Admin + Monitoring (병렬) |
| 8 | Unit 15 | Integration |

## Story Coverage

| 카테고리 | Stories | Unit |
|---------|---------|------|
| 인증 | US-001~003 | 5 (auth-adapter) |
| TUI 프레임워크 | US-004~005 | 6 (ui-widgets) |
| 커맨드/검색 | US-006~007 | 7 (input-system) |
| 비동기/알림 | US-008~009 | 2 (core-runtime) |
| Nova 기본 | US-010~014 | 8 (nova-domain) |
| Neutron | US-015~017, 028 | 9 (neutron-domain) |
| Cinder | US-018~021 | 10 (cinder-domain) |
| 공통 UI | US-022~023 | 6 (ui-widgets) |
| Nova Admin | US-024~027 | 13 (nova-admin-domain) |
| Neutron Admin | US-029 | 14 (admin-monitoring) |
| Cinder Admin | US-030~032 | 14 (admin-monitoring) |
| Identity | US-033~035 | 12 (identity-domain) |
| Quota | US-036 | 12 (identity-domain) |
| Glance | US-037~039 | 11 (glance-domain) |
| Compute Admin | US-040~041 | 13 (nova-admin-domain) |
| Monitoring | US-042~044 | 13, 14 |
| RBAC | US-045~046 | 4, 6 (infrastructure, ui-widgets) |
| 감사 | US-047 | 4 (infrastructure) |
| 통합 조회 | US-048 | 15 (integration) |
| Technical | TR-01~10 | 1, 2, 3, 5, 15 |
