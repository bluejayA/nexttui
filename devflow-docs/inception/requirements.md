# Requirements Analysis

**Depth**: Comprehensive
**Timestamp**: 2026-03-18T23:15:00+09:00
**Phase**: 1 (코어 프레임워크 + Nova/Neutron/Cinder + Identity/Glance/Monitoring/Admin)

## Project Context

### 기획 배경: Classic → NEXT 전환

NEXT 인프라 관리 시스템에서는 OpenStack이 더 이상 단독 기준 시스템(Source of Truth)이 아니다. OpenStack, 서비스 DB, VPC 제어 계층, 정책/승인 체계가 동시에 기준 시스템의 일부로 작동하는 구조로 전환되었으며, 변경 작업은 복수 기준 간 정합성을 사전에 보장해야 한다.

### 운영 시사점 (실무 인터뷰 기반)

1. **빈번한 정기 운영 작업**을 안정적으로 지원하는 것이 최우선 — 장애 대응/예외 처리보다 자원 정리·최적화 등 반복 작업의 비중이 높음
2. **CUD 작업은 정합성 이슈를 동반** — OpenStack 단독 조치로 완료되지 않으며, 서비스 GW 및 서비스별 관리 데이터와의 정합성 유지 프로세스 필요
3. **운영자 부담은 사전 판단에 집중** — 변경 행위 자체보다, 영향도 파악을 위한 정보 접근(복수 시스템 정보 조합)이 핵심 어려움

### 아키텍처 방향: Phase별 전환 전략

| Phase | 아키텍처 | 설명 |
|-------|---------|------|
| Phase 1 (MVP) | **Thick Client (2안)** | TUI가 OpenStack API 직접 호출. Port/Adapter 추상화로 향후 전환 대비 |
| Phase 2+ | **Service Layer 중심 (1안) 점진 전환** | 고위험/고가치 영역부터 중앙 Service Layer(Admin API GW) 경유로 전환. SOP 워크플로우, 중앙 감사, 정합성 보정을 Service Layer가 담당 |

Phase 1에서는 Adapter 구현체가 백엔드 API를 직접 호출하지만, 추상화 수준을 충분히 설계하여 Phase 2에서 Adapter를 "Service Layer 경유"로 교체만 하면 1안 구조로 전환 가능해야 한다.

### Scope 정의

- **1차 (본 기획 핵심)**: IaaS 기반 운영 통제 — OpenStack (Nova, Cinder, Neutron, Glance, Keystone) + IaaS Service Platform
- **2차**: 운영 판단 지원 연계 — NAS/Manila, 모니터링, 이벤트 정보 (조회 중심)
- **3차 (중장기)**: K8s, PaaS, SaaS 운영 영역

### 참조 문서

- 기획 배경 및 아키텍처 상세: `devflow-docs/inception/requirements_scope_backgroud.md`
- NEXT 운영 시스템 전략 Workshop: https://ktcloud.atlassian.net/wiki/spaces/TECHKCDC/pages/513474988
- Admin API 목록 현황: https://ktcloud.atlassian.net/wiki/x/U4enYw

## User Intent

NEXT Cloud 환경에 적합한 Admin CLI/TUI를 Rust/ratatui 기반으로 구축한다. substation(Swift 기반 OpenStack TUI)의 UX를 참조하되 1:1 포팅이 아닌 Rust 관용구에 맞게 재설계하며, NEXT의 복수 기준 시스템 환경에서 운영자가 안전하게 관리 작업을 수행할 수 있는 도구를 제공한다. Phase 1에서는 Thick Client(MVP)로 OpenStack API를 직접 호출하되, Port/Adapter 추상화를 통해 Phase 2에서 Service Layer 경유 구조로 점진 전환이 가능하도록 설계한다.

## Functional Requirements

### FR-01: 인증 및 클라우드 설정 [Must]

- **FR-01.1**: `~/.config/openstack/clouds.yaml` 파싱 (YAML)
  - 복수 클라우드 정의 지원
  - 인증 방식: password, application_credential
  - region_name (단일/배열)
  - SSL/TLS 설정 (verify, cacert, insecure)
  - 서비스별 API 버전 오버라이드
  - Risk: **High** — 인증 실패 시 전체 앱 사용 불가
- **FR-01.2**: Keystone v3 토큰 발급 및 자동 갱신
  - 토큰 만료 전 자동 갱신 (만료 5분 전 트리거)
  - 서비스 카탈로그 파싱 (엔드포인트 디스커버리)
  - Risk: **High**
- **FR-01.3**: 멀티 클라우드 컨텍스트 전환
  - `:ctx <cloud-name>` 커맨드로 클라우드 전환
  - `:ctx` 커맨드로 가용 클라우드 목록 표시
  - 전환 시 전체 데이터 리로드
  - Risk: **Medium**

### FR-02: TUI 프레임워크 및 레이아웃 [Must]

- **FR-02.1**: 메인 레이아웃 구조
  ```
  ┌────────────────────────────────────────────┐
  │ HEADER: 리소스 타입 | 클라우드명 | 리전     │
  ├──────────┬───────────────────────────────────┤
  │ SIDEBAR  │  MAIN CONTENT                    │
  │ (토글)   │  리스트/상세/폼                    │
  ├──────────┴───────────────────────────────────┤
  │ UNIFIED INPUT BAR (커맨드/검색)              │
  ├──────────────────────────────────────────────┤
  │ STATUS BAR: 메시지, 통계, 도움말             │
  └──────────────────────────────────────────────┘
  ```
- **FR-02.2**: 토글 사이드바
  - 기본: 사이드바 ON (모듈 목록 표시)
  - Tab 키로 ON/OFF 토글
  - OFF 시 콘텐츠 영역 전체 폭 사용
  - Risk: **Low**
- **FR-02.3**: 터미널 리사이즈 대응
  - 터미널 크기 변경 시 레이아웃 자동 재계산
  - Risk: **Low**

### FR-03: 네비게이션 및 입력 시스템 [Must]

- **FR-03.1**: Vi 스타일 리스트 네비게이션
  - `j/k` 또는 화살표: 위/아래 이동
  - `G`: 목록 끝으로, `g`: 목록 처음으로
  - `Page Up/Down`: 페이지 단위 이동
  - `Enter`: 선택 (리스트 → 상세)
  - `Esc`: 뒤로가기
  - Risk: **Low**
- **FR-03.2**: 커맨드 모드 (`:` prefix)
  - 리소스 네비게이션: `:servers`, `:networks`, `:volumes` + 축약어 (`:srv`, `:net`, `:vol`)
  - 시스템 커맨드: `:q` (종료), `:refresh` (새로고침), `:help`
  - 컨텍스트 전환: `:ctx <cloud-name>`
  - Tab 자동완성 (prefix 매칭 → 순환)
  - 커맨드 히스토리 (Up/Down, 최대 50개, 파일 저장)
  - Risk: **Medium**
- **FR-03.3**: 검색 모드 (`/` prefix)
  - 현재 리소스 리스트 내 텍스트 필터링
  - 실시간 필터 적용 (입력할 때마다)
  - `Esc`로 검색 해제
  - Risk: **Low**

### FR-04: 비동기 이벤트 아키텍처 [Must]

- **FR-04.1**: tokio::select! 통합 이벤트 루프
  - 키 입력 (crossterm EventStream)
  - 틱 타이머 (200ms, UI 갱신)
  - 백그라운드 작업 결과 (mpsc 채널)
  - Risk: **High** — 전체 앱의 기반
- **FR-04.2**: mpsc 양방향 채널 통신
  - action_tx: UI → Background (사용자 액션)
  - event_tx: Background → UI (API 결과)
  - Risk: **High**
- **FR-04.3**: 백그라운드 작업 추적
  - 작업별 상태 추적 (InProgress, Completed, Failed)
  - 상태바 Toast 알림 (TTL 기반 자동 제거)
  - Risk: **Medium**

### FR-05: Port/Adapter 패턴 (멀티 백엔드 API 디커플링) [Must]

- **FR-05.1**: 서비스별 trait 정의 (Port)
  - OpenStack: `NovaService`, `NeutronService`, `CinderService`, `KeystoneService`, `GlanceService`
  - 비-OpenStack (Phase 2): `ObjectStorageService` (Cloudian), `NasService` (Manila Admin), `NetworkSystemService` (커스텀)
  - async_trait, Send + Sync
  - Risk: **Medium**
- **FR-05.2**: HTTP adapter 구현 (OpenStack)
  - reqwest 기반 REST API 호출
  - 서비스 카탈로그에서 엔드포인트 자동 결정
  - JSON 응답 → 도메인 모델 변환 (serde)
  - Risk: **Medium**
- **FR-05.3**: Mock adapter (테스트용)
  - 각 서비스 trait의 Mock 구현체
  - API 없이 TUI 로직 단위 테스트 가능
  - Risk: **Low**
- **FR-05.4**: 멀티 백엔드 인증 추상화
  - `AuthProvider` trait: 백엔드별 인증 메커니즘 추상화
  - Phase 1: Keystone 토큰 (OpenStack)
  - Phase 2: HMAC 서명 (Cloudian/S3), API Key, 커스텀 인증
  - 인증 정보를 요청 헤더에 자동 주입하는 미들웨어 패턴
  - Risk: **High** — 인증 체계가 백엔드마다 근본적으로 다름
- **FR-05.5**: Adapter 레지스트리 (설정 기반 백엔드 선택)
  - 설정 파일(clouds.yaml 또는 별도 config)에서 백엔드 타입/엔드포인트 정의
  - 런타임에 적절한 adapter 인스턴스를 생성·주입
  - 동일 서비스 카테고리에 대해 다른 백엔드 구현체 교체 가능
  - Risk: **Medium**
- **FR-05.6**: Service Layer 전환 대비 (Phase 2)
  - Phase 1의 "직접 호출" adapter와 동일한 Port trait 인터페이스로, Phase 2에서 "Admin API GW 경유" adapter를 구현할 수 있어야 함
  - 전환 시 TUI 코드 변경 없이 adapter DI(의존성 주입)만으로 경로 변경 가능
  - Risk: **Medium** — Phase 1에서 추상화 수준을 과도/과소하게 잡으면 전환 비용 증가

### FR-06: 캐시 시스템 [Should]

- **FR-06.1**: 단일 레벨 캐시 (HashMap + TTL)
  - 리소스 타입별 TTL 설정
    - 서버 목록: 2분
    - 네트워크/보안그룹: 5분
    - 플레이버/이미지: 10분
  - `:refresh` 커맨드로 캐시 강제 무효화
  - Risk: **Low**

### FR-07: Component 시스템 [Must]

- **FR-07.1**: Component trait 정의
  ```rust
  trait Component {
      fn handle_key(&mut self, key: KeyEvent) -> Option<Action>;
      fn handle_event(&mut self, event: AppEvent);
      fn render(&self, frame: &mut Frame, area: Rect);
  }
  ```
- **FR-07.2**: 모듈 등록 및 라우팅
  - Route enum으로 현재 활성 컴포넌트 결정
  - 컴포넌트 간 독립적 상태 관리
  - Risk: **Medium**

### FR-08: Nova 서비스 (서버 관리) [Must]

- **FR-08.1**: 서버 리스트 뷰
  - 칼럼: 상태아이콘, 이름, 상태, IP 주소, 플레이버/이미지
  - 페이지네이션 (스크롤)
  - 검색 필터링
  - Risk: **Medium**
- **FR-08.2**: 서버 상세 뷰
  - 기본 정보: ID, 이름, 상태, 가용영역, 키페어, 업타임
  - 하드웨어: 플레이버, vCPU, RAM, 디스크
  - 네트워크: 인터페이스 목록 (네트워크명, 고정IP, 유동IP)
  - 볼륨: 연결된 볼륨 목록 (이름, 크기, 디바이스)
  - Risk: **Low**
- **FR-08.3**: 서버 생성 폼
  - 필수: 인스턴스명, 이미지, 플레이버, 네트워크
  - 선택: 보안그룹, 키페어, 가용영역
  - Risk: **High** — 동적 폼 + 다중 서비스 연동
- **FR-08.4**: 서버 기본 액션
  - 삭제, 리부트 (soft/hard), 시작, 중지
  - 확인 다이얼로그 (삭제/리부트 시)
  - Risk: **Medium**
- **FR-08.5**: 플레이버 관리
  - 리스트: 칼럼 — 이름, vCPU, RAM, 디스크, 공개여부
  - 서버 생성 폼에서 선택 가능
  - 생성: 이름, vCPU, RAM, 디스크, 공개여부 (Admin)
  - 삭제: 확인 다이얼로그 (Admin)
  - Risk: **Medium**
- **FR-08.6**: 서버 마이그레이션 (Admin)
  - Live Migration: 대상 호스트 선택 (선택적)
  - Block Migration: `--live --block` 옵션
  - Cold Migration: `openstack server migrate`
  - Risk: **High** — 운영 중 서버 이동, 실패 시 서비스 영향
- **FR-08.7**: 서버 Evacuate (Admin)
  - 장애 호스트의 서버를 다른 호스트로 대피
  - 대상 호스트 선택 (선택적)
  - Risk: **High** — 장애 대응 긴급 작업
- **FR-08.8**: 서버 상태 강제 변경 (Admin)
  - `server set --state` 로 상태 강제 전환 (ACTIVE, ERROR 등)
  - 확인 다이얼로그 필수
  - Risk: **Medium**
- **FR-08.9**: 서버 스냅샷 (인스턴스 이미지 생성)
  - `server image create` — 실행 중 서버의 스냅샷을 Glance 이미지로 생성
  - 스냅샷 이름 입력 폼
  - Risk: **Medium**

### FR-09: Neutron 서비스 (네트워크 관리) [Must]

- **FR-09.1**: 네트워크 리스트 뷰
  - 칼럼: 이름, 상태, Admin 상태, 외부여부, 공유여부, MTU
  - Risk: **Low**
- **FR-09.2**: 네트워크 상세 뷰
  - 기본 정보: ID, 이름, 상태, 설명
  - 설정: 공유, 외부, MTU, 포트 보안
  - 프로바이더 정보: 네트워크 타입, 물리 네트워크, 세그멘테이션 ID
  - 서브넷 목록
  - Risk: **Low**
- **FR-09.3**: 네트워크 생성 폼
  - 필수: 이름, Admin 상태
  - 선택: 공유, 외부, MTU, 포트 보안
  - Risk: **Medium**
- **FR-09.4**: 보안그룹 리스트 뷰
  - 칼럼: 이름, 설명, 룰 개수
  - Risk: **Low**
- **FR-09.5**: 보안그룹 상세 뷰
  - 인그레스/이그레스 룰 목록
  - 프로토콜, 포트 범위, 소스/목적지 (CIDR 또는 보안그룹)
  - Risk: **Low**
- **FR-09.6**: 보안그룹 CRUD
  - 생성: 이름, 설명
  - 수정: 이름, 설명 변경
  - 룰 추가: 방향, 프로토콜, 포트 범위, 소스
  - 룰 삭제
  - 보안그룹 삭제
  - Risk: **Medium**
- **FR-09.7**: Floating IP 관리
  - 리스트: IP 주소, 상태, 연결된 포트/서버, 네트워크
  - 생성: 외부 네트워크 선택
  - 삭제: 확인 다이얼로그
  - Associate: 서버/포트에 연결
  - Disassociate: 연결 해제
  - Risk: **Medium**
- **FR-09.8**: Network Agent 관리 (Admin)
  - 리스트: Agent 타입, 호스트, 상태, alive 여부
  - Enable/Disable: Agent 활성화/비활성화
  - 삭제: 확인 다이얼로그
  - Risk: **Medium**

### FR-10: Cinder 서비스 (볼륨 관리) [Must]

- **FR-10.1**: 볼륨 리스트 뷰
  - 칼럼: 이름, 상태, 크기(GB), 타입, 암호화, 부팅가능, 연결된 서버
  - Risk: **Low**
- **FR-10.2**: 볼륨 상세 뷰
  - 기본 정보: ID, 이름, 설명, 크기, 상태, 타입, 암호화, 부팅가능, 가용영역
  - 연결 정보: 서버명, 디바이스 경로, 연결 상태
  - 스냅샷 목록
  - Risk: **Low**
- **FR-10.3**: 볼륨 생성 폼
  - 필수: 이름, 크기(GB)
  - 선택: 볼륨 타입, 설명, 가용영역, 소스 (빈 볼륨/스냅샷/이미지)
  - Risk: **Medium**
- **FR-10.4**: 볼륨 액션
  - 삭제, 확장 (크기 증가)
  - 서버에 연결/분리
  - 상태 강제 변경 (`volume set --state`) (Admin)
  - 강제 삭제 (`volume delete --force`) (Admin)
  - Risk: **Medium**
- **FR-10.5**: 스냅샷 관리
  - 리스트: 칼럼 — 이름, 소스 볼륨, 크기, 상태, 생성일
  - 상세: ID, 이름, 소스 볼륨, 크기, 상태, 생성일
  - 삭제: 확인 다이얼로그
  - Risk: **Low**
- **FR-10.6**: 볼륨 QoS 관리 (Admin) [Should]
  - QoS 정책 리스트/조회
  - QoS 생성/삭제/수정
  - Risk: **Medium**
- **FR-10.7**: Storage Pool 조회 (Admin) [Should]
  - 스토리지 백엔드 풀 목록 조회 (`scheduler-stats/get_pools`)
  - 풀별 용량/사용량 정보
  - Risk: **Low**
- **FR-10.8**: 볼륨 마이그레이션 (Admin) [Should]
  - 볼륨을 다른 백엔드로 마이그레이션
  - 대상 호스트 선택
  - Risk: **High** — 데이터 이동, 실패 시 볼륨 손상 위험

### FR-11: 공통 UI 컴포넌트 [Must]

- **FR-11.1**: 리소스 리스트 위젯
  - 칼럼 정의 기반 테이블 렌더링
  - 선택 하이라이트, 스크롤, 검색 하이라이트
  - Risk: **Low**
- **FR-11.2**: 상세 뷰 위젯
  - 키-값 쌍 렌더링 (섹션별 그룹화)
  - 중첩 데이터 처리 (목록, 테이블)
  - Risk: **Low**
- **FR-11.3**: 폼 위젯
  - 필드 타입: 텍스트, 드롭다운(셀렉트), 멀티셀렉트, 체크박스
  - Tab 필드 이동, Enter 제출, Esc 취소
  - 필드 검증 (필수값, 숫자, CIDR 등)
  - Risk: **High** — 동적 폼 생성이 가장 복잡한 UI 컴포넌트
- **FR-11.4**: 확인 다이얼로그 (모달)
  - 삭제/리부트 등 위험 작업 전 확인
  - Y/N 입력
  - Risk: **Low**
- **FR-11.5**: Toast 알림
  - 하단 상태바에 일시적 메시지 표시
  - 성공(초록)/에러(빨강)/정보(파랑) 색상 구분
  - TTL 후 자동 제거
  - Risk: **Low**

### FR-12: Identity 서비스 (Keystone Admin) [Must]

- **FR-12.1**: 프로젝트 관리 (Admin)
  - 리스트: 이름, ID, 활성 상태, 설명
  - 생성: 이름, 설명, 도메인
  - 삭제: 확인 다이얼로그
  - Risk: **Medium**
- **FR-12.2**: 사용자 관리 (Admin)
  - 리스트: 이름, ID, 이메일, 활성 상태, 프로젝트
  - 생성: 이름, 패스워드, 이메일, 프로젝트, 도메인
  - 삭제: 확인 다이얼로그
  - Risk: **Medium**
- **FR-12.3**: 역할 관리 (Admin)
  - 역할 부여: 사용자-프로젝트-역할 매핑
  - 역할 회수: 확인 다이얼로그
  - Risk: **Medium** — 잘못된 역할 변경 시 접근 권한 문제

### FR-13: Quota 관리 (Admin) [Must]

- **FR-13.1**: 프로젝트 Quota 변경
  - 현재 Quota 조회 (cores, ram, instances, volumes, gigabytes 등)
  - Quota 값 수정 폼
  - Risk: **Medium**
- **FR-13.2**: Share Quota 변경 [Phase 2 — Manila 의존]
  - Manila Share Quota 조회/수정
  - Risk: **Low** — Phase 2 (Manila 서비스 구현 후)

### FR-14: Image 서비스 (Glance) [Must]

- **FR-14.1**: 이미지 리스트 뷰
  - 칼럼: 이름, 상태, 디스크 포맷, 크기, 가시성, 생성일
  - 검색 필터링
  - Risk: **Low**
- **FR-14.2**: 이미지 상세 뷰
  - 기본 정보: ID, 이름, 상태, 디스크/컨테이너 포맷, 크기, 체크섬
  - 속성: min_disk, min_ram, 아키텍처, OS 타입
  - 가시성: public/private/shared/community
  - Risk: **Low**
- **FR-14.3**: 이미지 등록 (Admin)
  - 이름, 디스크 포맷, 컨테이너 포맷, 가시성
  - 파일 경로 또는 URL 지정
  - Risk: **High** — 파일 업로드 + 메타데이터 관리
- **FR-14.4**: 이미지 수정
  - 이름, 가시성, 속성 변경
  - Risk: **Low**
- **FR-14.5**: 이미지 삭제 (Admin)
  - 확인 다이얼로그
  - Risk: **Medium**

### FR-15: Compute 관리 (Admin) [Must]

- **FR-15.1**: Aggregate 관리
  - 리스트: 이름, 가용영역, 호스트 수
  - 생성: 이름, 가용영역
  - 수정: 이름, 가용영역, 메타데이터
  - 삭제: 확인 다이얼로그
  - 호스트 추가/제거
  - Risk: **Medium**
- **FR-15.2**: Compute Service 관리
  - 리스트: 호스트, 바이너리, 상태, 활성 여부, 업데이트 시간
  - Enable/Disable: 비활성화 사유 입력 (disable 시)
  - Risk: **Medium** — 비활성화 시 해당 호스트에 신규 배치 불가

### FR-16: Monitoring 대시보드 [Must]

- **FR-16.1**: Hypervisor 조회
  - 리스트: 호스트명, 타입, vCPU (사용/전체), RAM (사용/전체), 디스크 (사용/전체), VM 수
  - 상세: 하이퍼바이저 상세 정보
  - Risk: **Low**
- **FR-16.2**: 사용량 조회
  - 프로젝트별 리소스 사용량 (vCPU, RAM, 인스턴스 수)
  - 기간 필터 (시작일~종료일)
  - Risk: **Medium**
- **FR-16.3**: 서버 이벤트 조회
  - 서버 상세 뷰 내 이벤트 탭/섹션
  - 이벤트: 액션, 시작/종료 시간, 결과, 메시지
  - Risk: **Low**
- **FR-16.4**: Aggregate 조회
  - FR-15.1의 리스트/상세와 공유 (읽기 전용 뷰)
  - Risk: **Low**
- **FR-16.5**: Network Agent 조회
  - FR-09.8의 리스트와 공유 (읽기 전용 뷰)
  - Risk: **Low**
- **FR-16.6**: Resource Provider 조회 [Phase 2 — Placement 의존]
  - RP 리스트/상세, Inventory 조회
  - Risk: **Medium** — Phase 2
- **FR-16.7**: Glance Cache 조회 [Phase 2]
  - 캐시된 이미지 목록
  - Risk: **Low** — Phase 2

### FR-17: RBAC 및 권한 제어 [Must]

- **FR-17.1**: 역할 기반 메뉴/액션 가시성 제어
  - 현재 사용자의 Keystone 역할(admin/member/reader)에 따라 Admin 전용 메뉴/액션 숨김
  - 서비스 카탈로그 기반 가용 서비스 판별
  - Risk: **Medium**
- **FR-17.2**: 위험 작업 확인 강화
  - 삭제, 강제 변경, 마이그레이션 등 고위험 작업에 2단계 확인 (확인 다이얼로그 + 리소스명 재입력)
  - Risk: **Low**
- **FR-17.3**: 중앙 RBAC 연동 (Phase 2)
  - Admin API GW의 중앙 권한 관리와 연동
  - Phase 1에서는 Keystone 역할 기반 로컬 판별, Phase 2에서 중앙 정책 서버로 전환
  - Risk: **Medium** — Phase 2

### FR-18: 감사 로그 [Should]

- **FR-18.1**: 로컬 작업 로그
  - CUD 작업 수행 시 로컬 파일에 감사 로그 기록 (시간, 사용자, 액션, 대상 리소스, 결과)
  - 저장 경로: `~/.config/nexttui/audit.log`
  - Risk: **Low**
- **FR-18.2**: 중앙 감사 로그 연동 (Phase 2)
  - Admin API GW의 중앙 감사 로그 시스템과 연동
  - Risk: **Medium** — Phase 2

### FR-19: 통합 조회 (크로스 시스템 정보 조합) [Should]

- **FR-19.1**: 서버-리소스 연관 뷰
  - 서버 상세에서 연결된 볼륨, 네트워크, 보안그룹, Floating IP를 한 화면에 표시
  - 각 연관 리소스 클릭 시 해당 상세 뷰로 이동
  - Risk: **Medium**
- **FR-19.2**: 볼륨-서버 매핑 뷰
  - 볼륨 상세에서 연결된 서버 정보 표시 + 서버 상세로 이동
  - Risk: **Low**
- **FR-19.3**: 복수 시스템 정보 조합 (Phase 2)
  - OpenStack 리소스와 서비스 GW(DB) 데이터를 조합하여 운영자에게 통합 뷰 제공
  - 예: VM과 연결된 Block/NAS 자원 관계를 한 화면에서 확인
  - Risk: **High** — Phase 2, Service Layer 연동 필요

### FR-20: 운영 워크플로우 지원 [Phase 2]

- **FR-20.1**: SOP 기반 작업 워크플로우
  - 사전 점검 → 실행 → 검증 → 기록의 운영 행위 패턴 지원
  - Phase 1에서는 확인 다이얼로그 수준, Phase 2에서 Service Layer의 SOP 엔진과 연동
  - Risk: **High** — Phase 2
- **FR-20.2**: 정합성 사전 검증 (Phase 2)
  - CUD 작업 전 OpenStack과 서비스 GW 간 상태 정합성 사전 검증
  - 불일치 발견 시 경고 및 작업 중단 옵션
  - Risk: **High** — Phase 2

## Non-Functional Requirements

### NFR-01: 성능
- 키 입력 → 화면 갱신: < 16ms (60 FPS 가능)
- API 호출 중 UI 블로킹: 0ms (완전 비동기)
- 1,000개 리소스 리스트 렌더링: < 50ms
- 메모리 사용량: < 50MB (일반 사용 시)

### NFR-02: 바이너리 배포
- 단일 정적 바이너리 (musl 타겟 지원)
- macOS (aarch64, x86_64) + Linux (x86_64, aarch64)

### NFR-03: 에러 복원력
- API 호출 실패 시 캐시된 데이터 표시 + 에러 알림
- 인증 토큰 만료 시 자동 갱신 시도 → 실패 시 재인증 프롬프트
- 네트워크 일시 단절 시 앱 크래시 없이 상태바에 연결 상태 표시

### NFR-04: 테스트
- Port trait의 Mock 구현으로 API 없이 TUI 로직 테스트 가능
- `#[cfg(test)]` 모듈 내 단위 테스트

### NFR-05: 보안
- clouds.yaml의 패스워드/시크릿은 메모리에서만 유지 (로그 출력 금지)
- TLS 인증서 검증 기본 활성화 (insecure 옵션 시에만 비활성화)
- 감사 로그에 민감 정보(패스워드, 토큰) 기록 금지

### NFR-06: 배포 환경
- VDI 기반 운영 환경에서 실행 가능 (Windows + Linux)
- 관리망 내부 실행 전제 — Internal API 엔드포인트 접근 가능
- 단일 바이너리로 별도 런타임 의존성 없이 배포/회수 용이

## Assumptions

1. **Phase 1은 clouds.yaml만 지원** — 환경변수 인증(`OS_AUTH_URL` 등)은 Phase 2에서 `EnvAuthProvider` adapter로 추가
2. **크로스서비스 검색은 Phase 2** — Phase 1은 현재 리소스 리스트 내 텍스트 필터링만
3. **배치 작업(멀티셀렉트)은 Phase 2** — Phase 1은 단일 리소스 작업만
4. **커맨드 히스토리 파일 저장 경로**: `~/.config/nexttui/command_history`
5. **OpenStack API 버전**: Nova v2.1, Neutron v2.0, Cinder v3.0, Keystone v3, Glance v2
6. **YAML 파서**: `serde_yaml` 크레이트 사용
7. **Admin 기능은 권한 기반** — 서비스 카탈로그/역할로 Admin 여부 판별, Admin이 아니면 해당 메뉴 숨김
8. **Phase 2 서비스**: Manila (Shared FS/NAS), Cloudian (Object Storage), Network System Admin, Placement
9. **멀티 백엔드 아키텍처** — 이 프로젝트는 OpenStack API만이 아닌, 다양한 백엔드 API(Cloudian, Manila Admin, Network System 등)를 수용해야 함. Phase 1에서 Port/Adapter의 추상화 수준을 충분히 설계하여 Phase 2에서 비-OpenStack 백엔드를 adapter 추가만으로 연동 가능해야 함
10. **Phase 1은 Thick Client(MVP)** — TUI가 OpenStack API를 직접 호출. Phase 2에서 고위험/고가치 영역부터 Admin API GW(Service Layer) 경유로 점진 전환
11. **운영 환경은 VDI 기반** — 관리망 내부에서 실행, Tag Agent RDP 또는 VDI 직접 접근 방식
12. **NEXT 환경에서 CUD 작업은 정합성 이슈를 동반** — Phase 1에서는 확인 다이얼로그 수준, Phase 2에서 Service Layer 기반 사전 정합성 검증으로 확장

## Open Questions

열린 질문: 0개

## Dependencies

### 외부 크레이트

| 크레이트 | 용도 | 버전 |
|---------|------|------|
| ratatui | TUI 프레임워크 | 0.30 |
| crossterm | 터미널 이벤트/렌더링 | 0.29 (event-stream) |
| tokio | 비동기 런타임 | 1.x (full) |
| reqwest | HTTP 클라이언트 | 0.12 (json) |
| serde / serde_json | 직렬화 | 1.x |
| serde_yaml | YAML 파싱 | 0.9 |
| anyhow | 에러 핸들링 | 1.x |
| color-eyre | 에러 리포팅 | 0.6 |
| futures | StreamExt | 0.3 |
| async-trait | trait 비동기 메서드 | 0.1 |
| chrono | 시간/날짜 처리 | 0.4 |

### 서비스 간 의존성

```
Nova (Servers) ──depends──► Neutron (Networks, SecurityGroups)
                          ► Cinder (Volumes)
                          ► Glance (Images)
                          ► Nova (Flavors, Keypairs)

Nova (Migration/Evacuate) ──► Nova (Aggregates, Compute Service)
Nova (Server Snapshot) ──► Glance (Images)

Neutron (Networks) ──── 독립 (base module)
Neutron (SecurityGroups) ──── 독립 (base module)
Neutron (Floating IPs) ──depends──► Neutron (Networks - external)
Cinder (Volumes) ──── 독립 (base module)
Glance (Images) ──── 독립 (base module)
Identity (Projects/Users/Roles) ──── 독립 (base module, Admin only)
Quota ──depends──► Identity (Projects)
Monitoring ──── 읽기 전용 (각 서비스 API 활용)
```

서버 생성 폼은 네트워크/보안그룹/볼륨/플레이버/이미지 데이터가 필요하므로, 해당 컴포넌트들이 먼저 구현되어야 함.

## Risk Assessment

| 요구사항 | 리스크 | 사유 |
|---------|--------|------|
| FR-04 비동기 이벤트 루프 | **High** | 전체 앱 기반, 초기 설계 실패 시 전면 재작업 |
| FR-01 인증/토큰 관리 | **High** | 인증 실패 시 전체 앱 사용 불가 |
| FR-08.3 서버 생성 폼 | **High** | 다중 서비스 연동 + 동적 폼 |
| FR-08.6 서버 마이그레이션 | **High** | 운영 서버 이동, 실패 시 서비스 영향 |
| FR-08.7 서버 Evacuate | **High** | 장애 대응 긴급 작업 |
| FR-10.8 볼륨 마이그레이션 | **High** | 데이터 이동, 실패 시 볼륨 손상 |
| FR-14.3 이미지 등록 | **High** | 파일 업로드 + 메타데이터 관리 |
| FR-11.3 폼 위젯 | **High** | 가장 복잡한 UI 컴포넌트 |
| FR-05 Port/Adapter | **Medium** | 설계 선택, trait 경계 결정 필요 |
| FR-03.2 커맨드 모드 | **Medium** | Tab 완성, 히스토리 등 세부 기능 다수 |
| FR-12 Identity 관리 | **Medium** | 잘못된 역할/사용자 변경 시 접근 문제 |
| FR-15 Compute 관리 | **Medium** | Aggregate/Service 변경 시 스케줄링 영향 |
| FR-05.6 Service Layer 전환 | **Medium** | 추상화 수준 과도/과소 시 전환 비용 증가 |
| FR-19.3 복수 시스템 정보 조합 | **High** | Phase 2, Service Layer 연동 필요 |
| FR-20 운영 워크플로우 | **High** | Phase 2, SOP 엔진 + 정합성 검증 |
| 나머지 | **Low** | 패턴 반복, ratatui 위젯 활용 |
