# nexttui

OpenStack 관리용 터미널 UI (TUI). Rust + [ratatui](https://ratatui.rs) 기반.

사내 클라우드 운영 조직이 서버, 네트워크, 볼륨 등 OpenStack 리소스를 빠르게 조회하고 관리할 수 있는 CLI 도구입니다.

## 주요 기능

- **리소스 조회/관리**: Servers, Flavors, Networks, Security Groups, Floating IPs, Volumes, Snapshots, Images, Projects, Users
- **생성/삭제 폼**: 필수 필드(`*`) 표시, 입력값 확인 화면, 성공/실패 Toast 알림
- **방향키 계층 네비게이션**: Sidebar ↔ List ↔ Detail, 일관된 ←→↑↓ 흐름
- **포커스 하이라이트**: 활성 패널 Cyan 테두리
- **Module Registry**: 모듈 자동 등록, 동적 Sidebar 생성
- **Demo 모드**: API 없이 샘플 데이터로 UI 확인

## 스크린샷

```
┌ Servers ──────────────────────────────────────────────┐
│ Modules │ Name       Status   IP          Flavor      │
│ > Servers│ web-01    ● ACTIVE  10.0.0.5   m1.small    │
│   Flavors│ db-01     ● ACTIVE  10.0.0.6   m1.medium   │
│   Networks│ test-vm  ○ SHUTOFF 10.0.0.7   m1.tiny     │
│   ...    │                                             │
└──────────┴─────────────────────────────────────────────┘
```

## 요구 사항

- Rust (edition 2024)
- OpenStack 환경 + `clouds.yaml` 설정

## 설치 및 실행

```bash
# 빌드
cargo build --release

# 실행 (clouds.yaml 필요)
cargo run

# Demo 모드 (API 없이)
cargo run -- --demo
```

## clouds.yaml 설정

아래 경로 중 하나에 `clouds.yaml`을 배치합니다:

1. `$OS_CLIENT_CONFIG_FILE` 환경변수
2. `./clouds.yaml` (현재 디렉토리)
3. `~/.config/openstack/clouds.yaml`
4. `/etc/openstack/clouds.yaml`

```yaml
clouds:
  mycloud:
    auth:
      auth_url: https://keystone.example.com/identity/v3
      username: admin
      password: secret
      project_name: admin
      user_domain_name: Default
      project_domain_name: Default
    region_name: RegionOne
```

## 키 바인딩

| 키 | 동작 |
|----|------|
| `↑↓` / `j/k` | 목록 이동 |
| `Enter` / `→` | 상세 보기 / 선택 |
| `←` / `Esc` | 뒤로 |
| `Tab` | Sidebar ↔ Content 포커스 전환 |
| `1-9, 0` | Sidebar 모듈 직접 이동 |
| `c` | 생성 폼 열기 |
| `d` | 삭제 |
| `r` | 새로고침 |
| `q` | 종료 |

## 아키텍처

```
src/
├── app.rs          # App 루트 (FocusPane, InputMode, render)
├── registry.rs     # ModuleRegistry (모듈 자동 등록)
├── component.rs    # Component trait
├── worker.rs       # Background worker (Action → API → AppEvent)
├── event_loop.rs   # tokio::select (key/tick/background event)
├── adapter/        # HTTP adapters (Nova, Neutron, Cinder, Glance, Keystone)
├── port/           # Port traits (API 추상화)
├── module/         # 16 domain modules (server, flavor, network, ...)
├── ui/             # UI widgets (sidebar, form, toast, detail_view, ...)
├── models/         # OpenStack API 응답 모델
└── infra/          # Cache, config
```

**패턴**: Component-Based + TEA 하이브리드 (Action → Worker → AppEvent → State) + Port/Adapter

## 테스트

```bash
cargo test          # 551 tests
cargo clippy        # lint
```

## 라이선스

Private — 사내 전용
