# nexttui

[🇰🇷 한국어](README.md) | 🇺🇸 **English**

[![License: MIT](https://img.shields.io/badge/License-MIT-yellow.svg)](https://opensource.org/licenses/MIT)
[![Rust](https://img.shields.io/badge/Rust-2024-orange.svg)](https://www.rust-lang.org/)
[![ratatui](https://img.shields.io/badge/ratatui-0.30-blue.svg)](https://ratatui.rs)

A terminal UI (TUI) for OpenStack cloud administration. Built with Rust and [ratatui](https://ratatui.rs).

Designed for **cloud infrastructure operators** who manage servers, networks, volumes, and floating IPs across OpenStack environments — directly from the terminal.

## Screenshots

### Server List View
![Server List](docs/screenshots/server-list.png)

### Server Detail View
![Server Detail](docs/screenshots/server-detail.png)

### Server Live Migrate
![Live Migrate](docs/screenshots/live-migrate.png)

### Host Operations
![Host Operations](docs/screenshots/host-ops.png)

### Usage Monitoring
![Usage](docs/screenshots/usage.png)

## Features

### Resource Management
- **17 domain modules**: Servers, Flavors, Networks, Security Groups, Floating IPs, Volumes, Snapshots, Images, Projects, Users, Aggregates, Compute Services, Hypervisors, Network Agents, Host Operations, Migration, Usage
- **Create/Delete forms**: Required field validation, confirmation dialogs, Toast notifications
- **Hierarchical navigation**: Sidebar ↔ List ↔ Detail with consistent arrow key flow

### Operations
- **Volume Attach/Detach**: Bidirectional entry (volume → server, server → volume)
- **Floating IP Associate/Disassociate**: Auto port selection, service disruption warnings
- **Force Detach / State Reset**: Admin-only with TypeToConfirm safety
- **Server Resize / Migration / Evacuate**: Host failure recovery workflows
- **Risk-based confirmation**: Y/N (normal) → TypeToConfirm (risky) → name input (critical)

### Dashboard & Monitoring
- **Usage Module**: btop-style resource dashboard (Infrastructure Summary + Project Usage + Hypervisor Allocation)
- **Gauge bars**: Color thresholds (Green 0–70% / Yellow 71–90% / Red 91–100%)
- **Activity Log**: CUD operation history popup (`!` key), StatusBar error badge

### Safety
- **RBAC 3-tier**: Reader / Member(Operator) / Admin permission model
- **CrossTenantGuard**: Block CUD in all_tenants mode + break-glass (`Ctrl+T`)
- **TransitionGuard**: Disable keys during resource state transitions
- **Boot volume protection**: Non-admin blocked, admin requires TypeToConfirm
- **Audit log**: `~/.config/nexttui/audit.log` JSON Lines with 10MB rotation

### UI/UX
- **Theme system**: Focus highlight, status icons, rounded borders
- **SelectPopup inline search**: `/` key to filter server/volume lists
- **ConfirmDialog context**: Volume name, size, type, project shown in dialogs
- **Resource connectivity**: Connected resources shown by name across all views
- **Navigation shortcuts**: `v`(Volumes) `n`(Networks) `s`(SG) `i`(Images) from server detail

## Requirements

- Rust (edition 2024)
- OpenStack environment + `clouds.yaml`

## Installation

```bash
# Build
cargo build --release

# Run (requires clouds.yaml)
cargo run -- --cloud mycloud

# Demo mode (no API needed)
cargo run -- --demo
```

## Configuration

Place `clouds.yaml` in one of:

1. `$OS_CLIENT_CONFIG_FILE` environment variable
2. `./clouds.yaml` (current directory)
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

## Key Bindings

### General
| Key | Action |
|-----|--------|
| `↑↓` / `j/k` | Navigate list |
| `Enter` / `→` | Detail view / Select |
| `←` / `Esc` | Back |
| `Tab` | Toggle Sidebar ↔ Content focus |
| `1-9, 0` | Jump to sidebar module |
| `c` | Open create form |
| `D` (Shift+D) | Delete |
| `r` | Refresh |
| `!` | Activity Log popup |
| `q` | Quit |

### Volume / Floating IP
| Key | Action |
|-----|--------|
| `a` | Attach / Associate |
| `x` | Detach / Disassociate |
| `F` (Shift+F) | Force Detach (Admin) |
| `R` (Shift+R) | Force State Reset (Admin) |

### Server Detail
| Key | Action |
|-----|--------|
| `A` (Shift+A) | Attach Volume |
| `x` | Detach Volume |
| `f` | Associate Floating IP |
| `v` / `n` / `s` / `i` | Navigate to Volumes/Networks/SG/Images |

### Usage
| Key | Action |
|-----|--------|
| `[` / `]` | Cycle period (This Month / Last Month / Last 7 Days) |
| `j` / `k` | Scroll |
| `r` | Refresh |

## Architecture

```
src/
├── app.rs          # App root (FocusPane, InputMode, AuditLogger)
├── registry.rs     # ModuleRegistry (auto module registration)
├── component.rs    # Component trait
├── worker.rs       # Background worker (Action → API → AppEvent)
├── event_loop.rs   # tokio::select (key/tick/background events)
├── adapter/        # HTTP adapters (Nova, Neutron, Cinder, Glance, Keystone)
├── port/           # Port traits (API abstraction)
├── module/         # 17 domain modules
│   ├── server/     # ServerModule + ServerViewContext
│   ├── volume/     # VolumeModule (attach/detach)
│   ├── floating_ip/# FloatingIpModule (associate/disassociate)
│   ├── host/       # HostModule (evacuate, composite layout)
│   ├── usage/      # UsageModule (btop-style dashboard)
│   └── ...
├── ui/             # UI widgets
│   ├── select_popup.rs  # SelectPopup (inline search)
│   ├── confirm.rs       # ConfirmDialog (YesNo / TypeToConfirm)
│   ├── gauge_bar.rs     # GaugeBar (btop-style)
│   ├── toast.rs         # Toast notifications
│   └── ...
├── models/         # OpenStack API response models
└── infra/          # RBAC, Cache, Config, AuditLogger, CrossTenantGuard
```

**Pattern**: Component-Based + TEA hybrid (Action → Worker → AppEvent → State) + Port/Adapter + ViewContext

## Testing

```bash
cargo test          # 1108 tests
cargo clippy        # lint
```

## Contributing

Contributions are welcome! Please feel free to submit a Pull Request.

1. Fork the repository
2. Create your feature branch (`git checkout -b feature/amazing-feature`)
3. Commit your changes (`git commit -m 'feat: add amazing feature'`)
4. Push to the branch (`git push origin feature/amazing-feature`)
5. Open a Pull Request

## License

This project is licensed under the MIT License — see the [LICENSE](LICENSE) file for details.
