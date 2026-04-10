# nova-evacuate — Ratatui TUI Implementation Spec

## Purpose

OpenStack Nova **host evacuation** TUI built with Ratatui (Rust).  
Operators use this to drain a failed/maintenance compute node by evacuating
all (or selected) instances to other hosts.

Claude Code must implement this spec exactly as written.  
When ambiguous, prefer **explicit control flow** and **user confirmation** over automation.

---

## Project Structure

```
nova-evacuate/
├── Cargo.toml
├── .env.example          # OS_AUTH_URL, OS_USERNAME, OS_PASSWORD, OS_PROJECT_NAME
├── src/
│   ├── main.rs           # CLI args (clap), tokio runtime, event loop entry
│   ├── app.rs            # App state machine, input handler, async event dispatch
│   ├── ui.rs             # All ratatui render logic (no state mutations here)
│   ├── nova.rs           # OpenStack Nova API client (Keystone + Nova REST)
│   └── types.rs          # Shared types: Hypervisor, Server, EvacTask, LogEntry, AppEvent
```

**Strict rule**: `ui.rs` must be pure render — zero side effects, zero API calls.  
All state mutation lives in `app.rs`.

---

## Dependencies (Cargo.toml)

```toml
ratatui     = "0.29"
crossterm   = { version = "0.28", features = ["event-stream"] }
tokio       = { version = "1", features = ["full"] }
reqwest     = { version = "0.12", features = ["json", "rustls-tls"], default-features = false }
serde       = { version = "1", features = ["derive"] }
serde_json  = "1"
anyhow      = "1"
chrono      = { version = "0.4", features = ["serde"] }
clap        = { version = "4", features = ["derive", "env"] }
futures     = "0.3"
```

---

## CLI Args (main.rs / clap)

```
nova-evacuate [OPTIONS]

Options:
  --mock                Use built-in mock data (no OpenStack required)
  --os-auth-url   <URL>        override OS_AUTH_URL env
  --os-username   <USER>       override OS_USERNAME env
  --os-password   <PASS>       override OS_PASSWORD env
  --os-project    <PROJECT>    override OS_PROJECT_NAME env
  --os-domain     <DOMAIN>     default: "Default"
  --region        <REGION>     default: "RegionOne"
  --all-tenants                pass all_tenants=1 to server list (admin only)
```

`--mock` flag OR missing `OS_AUTH_URL` → load `types::mock_hosts()` and `types::mock_servers()`.  
Mock mode shows `[MOCK]` badge in title bar.

---

## Screen Layout

```
┌─ NOVA EVACUATE [MOCK] ──── KT Cloud · kr-central-1 ───── 2026-04-02 15:42:03 ─┐
│                                                                                 │
├─[HOSTS]──────────────────┬─[INSTANCES]──────────────────────────────────────── ┤
│  col 0: status icon       │  col 0: checkbox                                    │
│  col 1: short hostname    │  col 1: name (truncated)                            │
│  col 2: AZ tag            │  col 2: short UUID (8 chars)                        │
│  col 3: VM count          │  col 3: flavor                                      │
│                           │  col 4: status                                       │
│  [resource bars]          │  col 5: first IP                                    │
│                           │                                                      │
│  [host action hints]      ├─[CONTROL]────────────────────────────────────────── ┤
│                           │  Target dropdown + options + action buttons          │
├─[LOG]─────────────────────────────────────────────────────────────────────── ┤
│  scrollable event log (last 200 lines)                               [?] help  │
└─────────────────────────────────────────────────────────────────────────────── ┘
```

### Exact terminal split ratios (Constraint::Percentage)

| Area | Direction | Size |
|---|---|---|
| Title bar | Vertical | 1 line (Length) |
| Main area | Vertical | Fill |
| Log panel | Vertical | 8 lines (Length) |
| Left (Hosts) | Horizontal | 32% |
| Right | Horizontal | 68% |
| Right-top (Instances) | Vertical | Fill |
| Right-bottom (Control) | Vertical | 6 lines (Length) |

---

## App State (app.rs — `struct App`)

```rust
pub struct App {
    // Data
    pub hosts: Vec<Hypervisor>,
    pub servers: HashMap<String, Vec<Server>>,  // keyed by hypervisor_hostname

    // Selection
    pub host_idx: usize,
    pub server_idx: usize,
    pub selected_servers: HashSet<String>,       // server IDs

    // Focus
    pub focus: Panel,  // enum Panel { Hosts, Instances, Control }

    // Control panel state
    pub target_host_idx: usize,   // 0 = "Auto (Nova Scheduler)"
    pub on_shared_storage: bool,  // default: true
    pub force_host: bool,         // default: false (--force / on-same-host)

    // Evacuation
    pub evac_tasks: Vec<EvacTask>,
    pub evac_running: bool,

    // Popup
    pub popup: Option<Popup>,

    // Log
    pub logs: VecDeque<LogEntry>,  // max capacity: 200
    pub log_scroll: usize,

    // Misc
    pub tick: u64,
    pub mock_mode: bool,
    pub loading: bool,
}
```

### enum Panel
```rust
pub enum Panel { Hosts, Instances, Control }
```
Tab cycles: `Hosts → Instances → Control → Hosts`

### enum Popup
```rust
pub enum Popup {
    ConfirmHostEvac { source: String, vm_count: usize },
    ConfirmSelectedEvac { source: String, vm_count: usize },
    Evacuating,          // full-screen overlay while in progress
    EvacResult { succeeded: usize, failed: Vec<(String, String)> },  // (name, reason)
    Help,
    Error(String),
}
```

---

## Rendering (ui.rs)

### Title Bar
```
 NOVA EVACUATE [MOCK]    KT Cloud · <region>    <ISO datetime>
 ─────────────────────────────────────────────────────────────
```
- `[MOCK]` badge: Yellow, only when mock_mode
- If `evac_running`: show blinking ` ⚡ EVACUATING ` in Orange (use app.tick % 2)
- Right-aligned clock updates every second via Tick event

### Host List Panel

Each row:
```
 ◉ compute-02   AZ-A   5 VMs
   CPU ████████░░ 75%
   RAM ███████░░░ 68%
```

Status icons:
| State | Icon | Color |
|---|---|---|
| UP + enabled | `○` | Green |
| DOWN | `◉` | Red (bold) |
| UP + disabled | `◌` | Yellow |
| Selected row | `▶` prefix on hostname | Cyan |

Resource bar: use `█` for used, `░` for free, width = 14 chars.  
Format: `CPU ██████████████ 100%`

Below host list, show hint block (only for focused panel):
```
 [e] Host Evacuate    [E] Live Mig All
 [d] Disable Host     [r] Refresh
```

### Instance List Panel

Header row (fixed, dim style):
```
   NAME             UUID      FLAVOR       STATUS   IP
```

Each row:
```
 ☑ prod-web-01    bbb-001  m2.large    ACTIVE   10.10.2.11
```

Checkbox: `☑` selected, `☐` unselected  
Status colors: ACTIVE=Green, SHUTOFF=Yellow, ERROR=Red, REBUILD=Orange(blinking)

Row highlight rules (priority order):
1. Evacuating now → Orange + `⟳ ` prefix on name
2. Evacuation done → Bright Green + `✓ ` prefix
3. Evacuation failed → Red + `✗ ` prefix
4. Selected → Cyan checkbox
5. Cursor row → reversed background

Bottom of panel (1 line): `  3 / 5 selected   [Space] toggle   [a] all   [Enter] evacuate selected`

### Control Panel (right-bottom, 6 lines)

```
 TARGET  [ compute-03 · AZ-B · CPU 28% RAM 25%             ▼ ]
 OPTS    [✓] --on-shared-storage    [ ] --force
         ⚡ Evacuate HOST (5)        ↺  Evacuate Selected (3)
```

Target dropdown: rendered as a bordered block when focused, otherwise inline.  
Option entries include `--on-shared-storage` (default on) and `--force`.
Action buttons: highlight active button with reversed style when cursor is on Control panel.

### Log Panel

```
 EVENT LOG                                                       ↑ scroll
 15:41:55 [INFO] compute-02 detected DOWN · 5 instances at risk
 15:42:01 [INFO] Fetching instance list...
 15:42:02 [ OK ] 5 instances loaded
```

Level colors:
| Level | Color |
|---|---|
| INFO | Dark Green |
| WARN | Yellow |
| ERRR | Red |
|  OK  | Bright Green |
| DEBG | Dark Gray |

Auto-scroll to bottom unless user has manually scrolled up (`log_scroll > 0`).

---

## Popups (centered overlay, ui.rs)

All popups: `Clear` background + bordered block.  
Use `ratatui::widgets::Clear` to erase the background before rendering.

### 1. ConfirmHostEvac / ConfirmSelectedEvac

```
┌─── Confirm Evacuation ──────────────────────────────┐
│                                                      │
│  Source  : compute-02  (DOWN)                        │
│  Target  : compute-03  · AZ-B  (auto-schedule)       │
│  VMs     : 5 instances                               │
│  Option  : --on-shared-storage                       │
│                                                      │
│  ⚠  This will REBUILD all selected instances.        │
│     Shared storage = no data loss expected.          │
│                                                      │
│          [ Y  Confirm ]     [ N  Cancel ]            │
└──────────────────────────────────────────────────────┘
```
Width: 56 cols, centered.  
Keys: `y`/`Enter` → confirm, `n`/`Esc` → cancel.

### 2. Evacuating (full-screen overlay, blocks all input except Abort)

```
┌─── Evacuating ──────────────────────────────────────┐
│  compute-02  →  compute-03                           │
│                                                      │
│  ████████████░░░░░░░░░░  3 / 5   60%                 │
│                                                      │
│  ✓  prod-web-01      12.3s                           │
│  ✓  prod-web-02       9.1s                           │
│  ⟳  prod-worker-01   Rebuilding...                  │
│  ○  prod-worker-02   Pending                         │
│  ○  prod-monitor     Pending                         │
│                                                      │
│                          [ Abort (a) ]               │
└──────────────────────────────────────────────────────┘
```
Progress bar width: 22 chars.  
`⟳` blinks using `tick % 2` (alternate `⟳` / `↻`).  
`Abort` sends cancellation signal to all pending tasks (tokio CancellationToken).

### 3. EvacResult

```
┌─── Evacuation Complete ─────────────────────────────┐
│                                                      │
│  ✓  4 succeeded                                      │
│  ✗  1 failed                                         │
│                                                      │
│  ✗  prod-worker-01   No valid host found             │
│                                                      │
│  [ Retry Failed ]   [ Disable Host ]   [ OK ]        │
└──────────────────────────────────────────────────────┘
```
Keys: `r` retry failed, `d` disable host (calls Nova disable API), `Enter`/`Esc` dismiss.

### 4. Help

```
┌─── Key Bindings ────────────────────────────────────┐
│                                                      │
│  Navigation                                          │
│  Tab / Shift+Tab   Cycle focus between panels        │
│  ↑ ↓               Move cursor within panel          │
│  Esc               Close popup / deselect            │
│                                                      │
│  Host Panel                                          │
│  e                 Host Evacuate (all VMs)           │
│  E                 Live Migrate All (no downtime)    │
│  d                 Disable / Enable host toggle      │
│  r                 Refresh all data                  │
│                                                      │
│  Instance Panel                                      │
│  Space             Toggle selection                  │
│  a                 Select / deselect all             │
│  Enter             Evacuate selected instances       │
│  f                 Filter by status (cycle)          │
│                                                      │
│  Global                                              │
│  q                 Quit                              │
│  ?                 This help                         │
│                                                      │
│                             [ Close (Esc) ]          │
└──────────────────────────────────────────────────────┘
```

---

## Input Handling (app.rs — `fn handle_key`)

```
Key           Panel::Hosts          Panel::Instances       Panel::Control
────────────  ────────────────────  ─────────────────────  ──────────────────
↑ / k         host_idx -= 1        server_idx -= 1        target_host_idx -= 1
↓ / j         host_idx += 1        server_idx += 1        target_host_idx += 1
Space         —                    toggle selected_servers toggle option under cursor
a             —                    select/deselect all    —
Enter         —                    → ConfirmSelectedEvac  trigger focused button
e             → ConfirmHostEvac    —                      —
E             → ConfirmLiveMig     —                      —
d             toggle host enable   —                      —
r             reload_all()         reload_all()           reload_all()
Tab           → Instances          → Control              → Hosts
Shift+Tab     → Control            → Hosts                → Instances
f             —                    cycle status filter    —
?             Popup::Help          Popup::Help            Popup::Help
q             quit                 quit                   quit
Esc           —                    clear selection        —
```

When `Popup::Evacuating` is active, all keys are consumed except `a` (abort).

---

## Async Event Architecture (app.rs)

Use two channels:

```rust
// Crossterm input events
let (input_tx, input_rx) = tokio::sync::mpsc::channel::<crossterm::event::Event>(32);

// App domain events
let (event_tx, event_rx) = tokio::sync::mpsc::channel::<AppEvent>(128);
```

Tick: 250ms interval, sends `AppEvent::Tick`.  
Render loop: triggered on every `AppEvent` received.

```rust
pub enum AppEvent {
    HostsLoaded(Vec<Hypervisor>),
    ServersLoaded(String, Vec<Server>),  // (hostname, servers)
    EvacStarted(String),                 // server_id
    EvacProgress(String, EvacStatus),    // server_id, new status
    EvacDone(String, Result<(), String>),
    Log(LogLevel, String),
    Tick,
}
```

### Evacuation task pattern

```rust
// Spawn one task per VM, share CancellationToken for abort
let token = CancellationToken::new();

for server in servers_to_evacuate {
    let tx = event_tx.clone();
    let client = nova_client.clone();
    let token = token.clone();

    tokio::spawn(async move {
        tx.send(AppEvent::EvacStarted(server.id.clone())).await.ok();
        tokio::select! {
            result = client.evacuate(&server.id, target, on_shared_storage) => {
                tx.send(AppEvent::EvacDone(server.id, result)).await.ok();
            }
            _ = token.cancelled() => {
                tx.send(AppEvent::EvacDone(server.id, Err("Aborted".into()))).await.ok();
            }
        }
    });
}
```

Store `token` in `App` for abort button to call `token.cancel()`.

---

## Nova API (nova.rs)

### Authentication

```
POST {OS_AUTH_URL}/v3/auth/tokens
Body: { "auth": { "identity": { "methods": ["password"], "password": { ... } },
                  "scope": { "project": { "name": ..., "domain": { "name": ... } } } } }
Response header: X-Subject-Token  → store as bearer token
Response body:   .token.catalog[type=compute].endpoints[interface=public].url → nova_endpoint
```

### API calls

| Method | Endpoint | Purpose |
|---|---|---|
| GET | `/os-hypervisors/detail` | Host list with resource usage |
| GET | `/servers/detail?host={hostname}&all_tenants=1` | Instances on host |
| POST | `/servers/{id}/action` | Evacuate one instance |
| PUT | `/os-services/{id}` | Enable/disable compute service |

Evacuate request body:
```json
{
  "evacuate": {
    "host": "<target_hostname or null for auto>",
    "onSharedStorage": true,
    "force": false
  }
}
```

Token refresh: on 401, re-authenticate once and retry.  
Timeout: 10s per request.  
Error: wrap all errors as `AppEvent::Log(LogLevel::Error, ...)` — never panic.

---

## Mock Mode (types.rs)

`mock_hosts()` returns 5 fixed hypervisors:

| Name | State | AZ | VMs | CPU% | RAM% |
|---|---|---|---|---|---|
| compute-01 | UP/enabled | AZ-A | 3 | 75% | 75% |
| compute-02 | **DOWN**/enabled | AZ-A | **5** | 94% | 90% |
| compute-03 | UP/enabled | AZ-B | 3 | 28% | 25% |
| compute-04 | UP/enabled | AZ-B | 2 | 13% | 13% |
| compute-05 | UP/**disabled** | AZ-C | 0 | 0% | 0% |

`mock_servers(hostname)` returns fixed VMs per host (see types.rs).

Mock evacuation: `sleep(600..1800ms)` per VM, 10% random failure rate.  
Failed reason: `"No valid host found"`.

---

## Status Color Reference (ratatui Style)

| Meaning | Foreground | Modifier |
|---|---|---|
| Host UP | Green | — |
| Host DOWN | Red | Bold |
| Host DISABLED | Yellow | — |
| ACTIVE instance | Green | — |
| SHUTOFF instance | DarkGray | — |
| ERROR instance | Red | Bold |
| Evacuating (⟳) | Yellow | — |
| Evac success (✓) | LightGreen | Bold |
| Evac failed (✗) | Red | Bold |
| Panel border (focused) | Cyan | — |
| Panel border (unfocused) | DarkGray | — |
| Log INFO | DarkGray | — |
| Log WARN | Yellow | — |
| Log ERRR | Red | — |
| Log OK | Green | Bold |

---

## Implementation Order (recommended)

1. `types.rs` — all structs and mock data  
2. `app.rs` — App struct + state transitions (no render)  
3. `ui.rs` — static layout with placeholder data  
4. Connect input loop in `main.rs` → verify navigation  
5. Add `Popup` rendering  
6. Add async evacuation tasks (mock mode first)  
7. `nova.rs` — real API client  
8. Wire real API into app events  
9. Polish: colors, borders, tick animations  

---

## Non-Goals (out of scope)

- Live migration scheduling optimization
- Multi-region support
- Instance console/VNC access
- Flavor/image management
- Writing back to CMDB (KT Cloud internal — separate integration)
