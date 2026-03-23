# nexttui 비동기 이벤트 아키텍처 설계

> Agent Council 논의 결과 (2026-03-18)
> 참여: Codex (GPT-5.3), Gemini, Claude (의장)

## 결정 사항

**"Component 패턴 + mpsc 양방향 채널 + select! 통합 루프"** 채택

## 1. 통신 패턴 — mpsc 양방향 채널

```
┌─────────────┐   Action (삭제 요청 등)   ┌──────────────┐
│  UI Thread   │ ──────────────────────►  │ Tokio Worker │
│  (main)      │                          │  (spawn)     │
│              │ ◄──────────────────────  │              │
└─────────────┘   Event (결과/에러)        └──────────────┘
```

```rust
// UI → Background: 사용자 액션
let (action_tx, mut action_rx) = mpsc::unbounded_channel::<Action>();

// Background → UI: API 결과
let (event_tx, mut event_rx) = mpsc::unbounded_channel::<AppEvent>();
```

### 채널 타입별 용도

| 채널 타입 | 용도 | 언제 |
|----------|------|------|
| `mpsc::unbounded` | 메인 통신 | 대부분의 경우 (기본) |
| `oneshot` | 단발성 응답 | 모달 확인 다이얼로그 등 |
| `broadcast` | 다수 구독 | 인증 토큰 갱신 알림 (모든 서비스가 수신) |

**트레이드오프**: Unbounded는 메모리 폭주 위험이 있지만, TUI의 입력 빈도에서는 현실적으로 문제 없음. Bounded를 쓰면 `send`가 async가 되어 UI 스레드에서 사용이 복잡해짐.

## 2. 통합 이벤트 루프 — `tokio::select!`

세 가지 소스를 하나의 `select!`로 통합:

```rust
use crossterm::event::EventStream;
use futures::StreamExt;

pub async fn run_loop(
    terminal: &mut Terminal<impl Backend>,
    app: &mut App,
) -> Result<()> {
    let mut key_events = EventStream::new();
    let mut tick = tokio::time::interval(Duration::from_millis(200));

    loop {
        tokio::select! {
            // ① 키 입력
            Some(Ok(evt)) = key_events.next() => {
                if let Event::Key(key) = evt {
                    app.handle_key(key, &action_tx);
                }
            }

            // ② 틱 (UI 갱신, 스피너 애니메이션)
            _ = tick.tick() => {
                app.on_tick();
            }

            // ③ 백그라운드 작업 결과
            Some(event) = event_rx.recv() => {
                app.handle_event(event);
            }
        }

        terminal.draw(|f| app.render(f))?;

        if app.should_quit {
            break;
        }
    }
    Ok(())
}
```

**틱 주기**: 200ms (5 FPS)면 충분. 애니메이션 필요 시 100ms로 조정.

## 3. 상태 관리 — Component-Based + TEA 하이브리드

### 패턴 비교

| 패턴 | 장점 | 단점 |
|------|------|------|
| **순수 TEA** | 상태 추적 명확, 디버깅 쉬움 | 8개 서비스 → match 문이 거대해짐 |
| **Component** | 모듈 독립성, 확장 용이 | 컴포넌트 간 통신 설계 필요 |

### 채택: Component 패턴 + 전역 TEA 메시지 버스

```rust
// 전역 액션 (서비스 간 공유)
enum Action {
    Navigate(Route),
    Notify(String),
    TokenRefreshed(Token),
}

// 서비스별 컴포넌트 — 각자 로컬 상태 관리
trait Component {
    fn handle_key(&mut self, key: KeyEvent) -> Option<Action>;
    fn handle_event(&mut self, event: AppEvent);
    fn render(&self, frame: &mut Frame, area: Rect);
}

// Nova 컴포넌트 예시
struct ServersComponent {
    servers: Vec<Server>,
    selected: usize,
    loading: bool,
    pending_ops: HashMap<String, OperationStatus>,
}

impl Component for ServersComponent {
    fn handle_key(&mut self, key: KeyEvent) -> Option<Action> {
        match key.code {
            KeyCode::Char('d') => {
                let id = self.servers[self.selected].id.clone();
                self.pending_ops.insert(id.clone(), OperationStatus::InProgress);
                Some(Action::DeleteServer(id))
            }
            KeyCode::Char('j') => { self.selected += 1; None }
            _ => None,
        }
    }

    fn handle_event(&mut self, event: AppEvent) {
        match event {
            AppEvent::ServerDeleted(id) => {
                self.servers.retain(|s| s.id != id);
                self.pending_ops.remove(&id);
            }
            AppEvent::ApiError(id, err) => {
                self.pending_ops.insert(id, OperationStatus::Failed(err));
            }
            _ => {}
        }
    }
    // ...
}

// App은 라우팅 + 공유 상태만 관리
struct App {
    route: Route,
    components: HashMap<Route, Box<dyn Component>>,
    auth: Arc<AuthManager>,
}
```

## 4. 에러 핸들링 & 백그라운드 작업 추적

```rust
enum OperationStatus {
    InProgress,
    Completed,
    Failed(String),
}

struct BackgroundTracker {
    operations: HashMap<String, OperationInfo>,
}

struct OperationInfo {
    description: String,   // "서버 web-01 삭제 중"
    started_at: Instant,
    status: OperationStatus,
}

// 백그라운드 작업 spawn 패턴
fn spawn_delete(
    server_id: String,
    client: Arc<OpenStackClient>,
    event_tx: mpsc::UnboundedSender<AppEvent>,
) {
    tokio::spawn(async move {
        match client.nova().delete_server(&server_id).await {
            Ok(_) => {
                event_tx.send(AppEvent::ServerDeleted(server_id)).ok();
            }
            Err(e) => {
                event_tx.send(AppEvent::ApiError(
                    server_id,
                    e.to_string(),
                )).ok();
            }
        }
    });
}
```

알림 시스템: 작업 완료/실패 시 하단 상태바에 Toast 형태로 표시 → on_tick에서 TTL 기반 자동 제거.

## 5. 전체 아키텍처 다이어그램

```
┌─────────────────────────────────────────────────────┐
│                    main thread                       │
│                                                     │
│  ┌─────────┐   select!   ┌──────────────────────┐  │
│  │Crossterm│────────────►│      App (Router)     │  │
│  │EventStream│           │                      │  │
│  └─────────┘             │  ┌─────────────────┐ │  │
│  ┌─────────┐             │  │ ServersComponent│ │  │
│  │  Tick   │────────────►│  │ NetworkComponent│ │  │
│  │ 200ms   │             │  │ VolumeComponent │ │  │
│  └─────────┘             │  └─────────────────┘ │  │
│  ┌─────────┐             │          │           │  │
│  │event_rx │────────────►│    render(frame)     │  │
│  └─────────┘             └──────────────────────┘  │
│       ▲                          │                  │
│       │                    action_tx                │
│       │                          ▼                  │
│  ┌──────────────────────────────────────────────┐  │
│  │              Tokio Runtime                    │  │
│  │  ┌──────┐  ┌──────┐  ┌──────┐  ┌─────────┐ │  │
│  │  │ Nova │  │Neutron│  │Cinder│  │  Auth   │ │  │
│  │  │ API  │  │ API   │  │ API  │  │ Refresh │ │  │
│  │  └──────┘  └──────┘  └──────┘  └─────────┘ │  │
│  └──────────────────────────────────────────────┘  │
│              공유: Arc<OpenStackClient>              │
│              공유: Arc<Cache>                        │
└─────────────────────────────────────────────────────┘
```

## 6. 핵심 크레이트 조합

```toml
[dependencies]
ratatui = "0.30"
crossterm = { version = "0.29", features = ["event-stream"] }
tokio = { version = "1", features = ["full"] }
reqwest = { version = "0.12", features = ["json"] }
serde = { version = "1", features = ["derive"] }
anyhow = "1"
color-eyre = "0.6"
futures = "0.3"
```

## 6.5. Port/Adapter 패턴 — OpenStack API 디커플링

### 문제

컴포넌트가 `Arc<OpenStackClient>`를 직접 참조하면 API 변경 시 TUI 레이어까지 영향이 전파됨.

### 해결: trait 기반 Port/Adapter

```
┌──────────────────────────────────────────────────────┐
│  TUI Layer (Components)                               │
│         │  trait 의존 (컴파일 타임 경계)                │
│         ▼                                            │
│  ┌──────────────────────────────────┐                │
│  │  Port Layer (traits)              │                │
│  │  trait NovaService                │                │
│  │  trait NeutronService             │                │
│  │  trait CinderService              │                │
│  └──────────────────────────────────┘                │
│         ▲  impl                                      │
│  ┌──────────────────────────────────┐                │
│  │  Adapter Layer (HTTP impl)        │ ◄── API 변경  │
│  │  NovaHttpClient                   │     시 여기만  │
│  │  NeutronHttpClient                │     수정       │
│  │  CinderHttpClient                 │                │
│  └──────────────────────────────────┘                │
│         │                                            │
│         ▼                                            │
│  OpenStack REST API                                   │
└──────────────────────────────────────────────────────┘
```

### Port (trait 정의)

```rust
#[async_trait]
trait NovaService: Send + Sync {
    async fn list_servers(&self) -> Result<Vec<Server>>;
    async fn get_server(&self, id: &str) -> Result<Server>;
    async fn delete_server(&self, id: &str) -> Result<()>;
    async fn reboot_server(&self, id: &str, hard: bool) -> Result<()>;
    async fn list_flavors(&self) -> Result<Vec<Flavor>>;
}

#[async_trait]
trait NeutronService: Send + Sync {
    async fn list_networks(&self) -> Result<Vec<Network>>;
    async fn list_security_groups(&self) -> Result<Vec<SecurityGroup>>;
    // ...
}
```

### Adapter (HTTP 구현)

```rust
struct NovaHttpClient {
    http: reqwest::Client,
    endpoint: String,
    auth: Arc<AuthManager>,
}

#[async_trait]
impl NovaService for NovaHttpClient {
    async fn list_servers(&self) -> Result<Vec<Server>> {
        let resp = self.http
            .get(format!("{}/servers/detail", self.endpoint))
            .header("X-Auth-Token", self.auth.token().await?)
            .send().await?;
        let body: NovaApiResponse = resp.json().await?;
        Ok(body.servers.into_iter().map(Server::from).collect())
    }
    // ...
}
```

### TUI 컴포넌트에서의 사용

```rust
struct ServersComponent {
    nova: Arc<dyn NovaService>,  // 구체 타입 모름, trait만 의존
    servers: Vec<Server>,
    selected: usize,
}
```

### 테스트에서의 Mock

```rust
struct MockNovaService { servers: Vec<Server> }

#[async_trait]
impl NovaService for MockNovaService {
    async fn list_servers(&self) -> Result<Vec<Server>> {
        Ok(self.servers.clone())
    }
    // ...
}
```

### 변경 영향 범위

| 상황 | 수정 범위 |
|------|----------|
| Nova API v2.1 → v2.2 | `NovaHttpClient`만 |
| 새 서비스 추가 (Heat 등) | 새 trait + 새 adapter, 기존 무변경 |
| 테스트 | Mock impl로 API 없이 TUI 테스트 |
| OpenStack → 다른 클라우드 | 새 adapter만 추가 (trait 동일) |

## 7. 수정된 전체 아키텍처 다이어그램

```
┌─────────────────────────────────────────────────────────┐
│                      main thread                         │
│                                                         │
│  ┌──────────┐  select!  ┌───────────────────────────┐  │
│  │ Crossterm │─────────►│        App (Router)        │  │
│  │EventStream│          │                           │  │
│  └──────────┘           │  ┌─────────────────────┐  │  │
│  ┌──────────┐           │  │ ServersComponent     │  │  │
│  │  Tick    │─────────►│  │   nova: dyn NovaService│  │  │
│  │  200ms   │           │  │ NetworkComponent     │  │  │
│  └──────────┘           │  │   neutron: dyn Neutron│  │  │
│  ┌──────────┐           │  │ VolumeComponent      │  │  │
│  │ event_rx │─────────►│  │   cinder: dyn Cinder │  │  │
│  └──────────┘           │  └─────────────────────┘  │  │
│       ▲                 └───────────────────────────┘  │
│       │                          │ action_tx            │
│       │                          ▼                      │
│  ┌──────────────────────────────────────────────────┐  │
│  │               Tokio Runtime                       │  │
│  │                                                  │  │
│  │  ┌─────────────────────────────────────────────┐ │  │
│  │  │  Adapter Layer (trait impl)                  │ │  │
│  │  │  NovaHttpClient  NeutronHttpClient           │ │  │
│  │  │  CinderHttpClient  AuthManager               │ │  │
│  │  └─────────────────────────────────────────────┘ │  │
│  │         │                                        │  │
│  │         ▼  reqwest                               │  │
│  │  OpenStack REST API                              │  │
│  └──────────────────────────────────────────────────┘  │
│                                                         │
│  공유: Arc<AuthManager>, Arc<Cache>                      │
└─────────────────────────────────────────────────────────┘
```

## 설계 원칙

1. **UI는 절대 블로킹하지 않는다** — 모든 API 호출은 tokio::spawn
2. **컴포넌트 독립성** — 각 서비스 컴포넌트는 독립적으로 개발/테스트 가능
3. **전역 상태 최소화** — 인증, 서비스 카탈로그만 Arc로 공유
4. **점진적 복잡도** — 캐시는 단순 HashMap+TTL로 시작, 필요 시 고도화
5. **Port/Adapter 디커플링** — TUI는 trait만 의존, API 변경은 adapter에 격리
