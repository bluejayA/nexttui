# User Stories

**Timestamp**: 2026-03-18T23:20:00+09:00
**Source**: devflow-docs/inception/requirements.md

## Actors

- **Operator**: 클라우드 운영자 — OpenStack 인프라를 관리하고, 서버/네트워크/볼륨 등 리소스를 생성·조회·수정·삭제한다.
- **Admin**: 클라우드 관리자 — Identity, Quota, Aggregate, Compute Service 등 플랫폼 수준의 관리 권한을 가진다. Operator의 모든 기능을 포함한다.
- **Developer**: 개발자 — 자신의 프로젝트 내 리소스(서버, 네트워크, 볼륨)를 터미널에서 빠르게 확인하고 관리한다.
- **System**: nexttui 시스템 — 인증, 캐시, 비동기 처리 등 내부 인프라 기능을 자동으로 수행한다.

## Stories

---

### 코어: 인증 및 클라우드 설정

#### US-001: clouds.yaml 기반 인증
**Actor**: Operator
**Story**: As an operator, I want to authenticate using my existing clouds.yaml file so that I can manage OpenStack resources without additional configuration.
**Acceptance Criteria**:
- Given a valid `~/.config/openstack/clouds.yaml` exists, When I launch nexttui, Then the app authenticates with the first cloud and shows the dashboard.
- Given clouds.yaml has multiple cloud entries, When I launch nexttui, Then I can select which cloud to connect to.
- Given clouds.yaml uses password auth, When I authenticate, Then a Keystone v3 token is issued and the service catalog is parsed.
- Given clouds.yaml uses application_credential, When I authenticate, Then token is issued using app credential flow.
- Given clouds.yaml is missing or malformed, When I launch nexttui, Then a clear error message is shown with the expected file path.
**Priority**: Must
**FR**: FR-01.1, FR-01.2

#### US-002: 토큰 자동 갱신
**Actor**: System
**Story**: As the system, I want to automatically refresh the Keystone token before expiry so that long-running sessions don't lose authentication.
**Acceptance Criteria**:
- Given a token expires in less than 5 minutes, When the tick timer fires, Then the system initiates a background token refresh.
- Given token refresh succeeds, When the new token is received, Then all subsequent API calls use the new token seamlessly.
- Given token refresh fails, When the refresh error is received, Then a toast notification warns the user and retries once.
**Priority**: Must
**FR**: FR-01.2

#### US-003: 멀티 클라우드 컨텍스트 전환
**Actor**: Operator
**Story**: As an operator managing multiple clouds, I want to switch between cloud contexts so that I can manage resources across environments.
**Acceptance Criteria**:
- Given I type `:ctx`, When I press Enter, Then a list of available clouds from clouds.yaml is shown.
- Given I type `:ctx mycloud`, When I press Enter, Then the app re-authenticates to "mycloud" and reloads all data.
- Given context switch succeeds, When data reload completes, Then the header updates to show the new cloud name and region.
- Given context switch fails (auth error), When the error is received, Then I remain on the current cloud with an error toast.
**Priority**: Must
**FR**: FR-01.3

---

### 코어: TUI 프레임워크

#### US-004: 메인 레이아웃과 토글 사이드바
**Actor**: Developer
**Story**: As a developer, I want a clean TUI layout with a toggleable sidebar so that I can navigate resources easily while maximizing screen space when needed.
**Acceptance Criteria**:
- Given the app launches, When the main screen renders, Then I see a header, sidebar (ON by default), main content area, input bar, and status bar.
- Given the sidebar is visible, When I press Tab, Then the sidebar hides and the main content expands to full width.
- Given the sidebar is hidden, When I press Tab, Then the sidebar reappears showing the module list.
- Given the terminal is resized, When the resize event fires, Then the layout recalculates and redraws correctly.
**Priority**: Must
**FR**: FR-02.1, FR-02.2, FR-02.3

#### US-005: Vi 스타일 리스트 네비게이션
**Actor**: Developer
**Story**: As a developer familiar with Vi keybindings, I want to navigate resource lists using j/k/G/g keys so that I can quickly find resources.
**Acceptance Criteria**:
- Given I'm on a resource list, When I press `j` or Down arrow, Then the selection moves down one row.
- Given I'm on a resource list, When I press `k` or Up arrow, Then the selection moves up one row.
- Given I'm on a resource list, When I press `G`, Then the selection jumps to the last item.
- Given I'm on a resource list, When I press `g`, Then the selection jumps to the first item.
- Given I'm on a resource list, When I press Enter, Then I navigate to the detail view of the selected resource.
- Given I'm on a detail view, When I press Esc, Then I return to the list view.
**Priority**: Must
**FR**: FR-03.1

#### US-006: 커맨드 모드
**Actor**: Operator
**Story**: As an operator, I want to use Vi-style command mode to quickly navigate to any resource type so that I can work efficiently without a mouse.
**Acceptance Criteria**:
- Given I'm anywhere in the app, When I press `:`, Then the input bar activates in command mode.
- Given I'm in command mode, When I type `srv` and press Enter, Then I navigate to the servers list.
- Given I'm in command mode, When I type `net` and press Tab, Then "networks" auto-completes.
- Given I'm in command mode, When I press Up arrow, Then the previous command from history appears.
- Given I'm in command mode, When I press Esc, Then command mode is cancelled without action.
- Given I type `:q`, When I press Enter, Then the app exits gracefully.
- Given I type `:refresh`, When I press Enter, Then the cache is cleared and the current view reloads.
**Priority**: Must
**FR**: FR-03.2

#### US-007: 검색 필터링
**Actor**: Developer
**Story**: As a developer, I want to search/filter the current resource list by typing `/query` so that I can quickly find specific resources.
**Acceptance Criteria**:
- Given I'm on a resource list, When I press `/` and type "web", Then the list filters in real-time to show only resources containing "web".
- Given I have an active search filter, When I press Esc, Then the filter is cleared and the full list is restored.
- Given I search for a term with no matches, When the filter applies, Then an empty list is shown with "No matching resources" message.
**Priority**: Must
**FR**: FR-03.3

---

### 코어: 비동기 및 시스템

#### US-008: 논블로킹 API 호출
**Actor**: System
**Story**: As the system, I want all API calls to execute in background tokio tasks so that the UI never blocks on network operations.
**Acceptance Criteria**:
- Given a user requests server deletion, When the API call is in progress, Then the UI remains responsive and the user can navigate to other views.
- Given an API call is in progress, When I switch to a different resource, Then the pending operation continues and delivers results when complete.
- Given multiple API calls are in flight, When results arrive, Then each result updates the correct component state.
**Priority**: Must
**FR**: FR-04.1, FR-04.2

#### US-009: 백그라운드 작업 알림
**Actor**: Operator
**Story**: As an operator, I want to see toast notifications for completed background operations so that I know when my actions succeed or fail.
**Acceptance Criteria**:
- Given a server deletion succeeds, When the result arrives, Then a green toast "Server 'web-01' deleted" appears in the status bar.
- Given a server deletion fails, When the error arrives, Then a red toast with the error message appears.
- Given a toast is displayed, When 5 seconds pass, Then the toast auto-removes.
**Priority**: Must
**FR**: FR-04.3, FR-11.5

---

### Nova 서비스

#### US-010: 서버 리스트 조회
**Actor**: Operator
**Story**: As an operator, I want to see a list of all servers with key information so that I can monitor my compute resources at a glance.
**Acceptance Criteria**:
- Given I navigate to `:servers`, When the data loads, Then I see a table with columns: status icon, name, status, IP address, flavor/image.
- Given the server list is loading, When the API call is in progress, Then a loading spinner is shown.
- Given the server list is loaded, When I scroll beyond the visible area, Then the list scrolls smoothly showing more servers.
- Given I type `/web`, When the filter applies, Then only servers with "web" in the name are shown.
**Priority**: Must
**FR**: FR-08.1

#### US-011: 서버 상세 정보 조회
**Actor**: Developer
**Story**: As a developer, I want to see detailed information about a specific server so that I can diagnose issues or verify configuration.
**Acceptance Criteria**:
- Given I'm on the servers list, When I press Enter on a server, Then the detail view shows: basic info (ID, name, status, AZ, keypair, uptime), hardware (flavor, vCPU, RAM, disk), network interfaces, and attached volumes.
- Given the server has multiple network interfaces, When the detail view renders, Then each interface shows network name, fixed IP, and floating IP.
- Given the server has attached volumes, When the detail view renders, Then each volume shows name, size, and device path.
**Priority**: Must
**FR**: FR-08.2

#### US-012: 서버 생성
**Actor**: Operator
**Story**: As an operator, I want to create a new server through a form so that I can provision compute resources from the TUI.
**Acceptance Criteria**:
- Given I press `c` on the servers list, When the form opens, Then I see fields: instance name, image (dropdown), flavor (dropdown), network (multi-select).
- Given I select a network, When I Tab to security groups, Then available security groups are shown as checkboxes.
- Given all required fields are filled, When I press Enter to submit, Then the API creates the server and I see a success toast.
- Given a required field is empty, When I try to submit, Then validation highlights the missing field.
- Given creation fails, When the error arrives, Then an error toast shows the reason.
**Priority**: Must
**FR**: FR-08.3

#### US-013: 서버 액션 (삭제, 리부트, 시작, 중지)
**Actor**: Operator
**Story**: As an operator, I want to perform actions on servers (delete, reboot, start, stop) so that I can manage server lifecycle.
**Acceptance Criteria**:
- Given I press `d` on a selected server, When the confirm dialog appears and I press `y`, Then the server is deleted and removed from the list.
- Given I press `d` on a selected server, When I press `n` on the confirm dialog, Then the action is cancelled.
- Given I initiate a reboot, When the confirm dialog asks soft/hard, Then I can choose and the reboot executes.
- Given I start a stopped server, When the action completes, Then the status updates to ACTIVE.
**Priority**: Must
**FR**: FR-08.4

#### US-014: 플레이버 관리
**Actor**: Developer / Admin
**Story**: As a developer, I want to see available flavors so that I can choose the right hardware profile. As an admin, I want to create and delete flavors to manage compute resource profiles.
**Acceptance Criteria**:
- Given I navigate to `:flavors`, When the data loads, Then I see a table with: name, vCPUs, RAM (MB), disk (GB), public/private.
- Given flavors are cached, When I revisit within 10 minutes, Then cached data is shown without API call.
- Given I am an admin and press `c` on flavors list, When the form opens, Then I see: name, vCPUs, RAM, disk, public/private fields.
- Given I am an admin and press `d` on a flavor, When I confirm, Then the flavor is deleted.
- Given I am NOT an admin, When I view flavors, Then create/delete actions are hidden.
**Priority**: Must
**FR**: FR-08.5

---

### Neutron 서비스

#### US-015: 네트워크 리스트 및 상세 조회
**Actor**: Operator
**Story**: As an operator, I want to view networks and their details so that I can understand my network topology.
**Acceptance Criteria**:
- Given I navigate to `:networks`, When the data loads, Then I see: name, status, admin state, external, shared, MTU.
- Given I press Enter on a network, When the detail view opens, Then I see: basic info, configuration (shared/external/MTU/port security), provider info (type/physical network/segmentation ID), and subnet list.
**Priority**: Must
**FR**: FR-09.1, FR-09.2

#### US-016: 네트워크 생성
**Actor**: Operator
**Story**: As an operator, I want to create networks so that I can set up connectivity for my servers.
**Acceptance Criteria**:
- Given I press `c` on the networks list, When the form opens, Then I see fields: name (required), admin state, shared, external, MTU, port security.
- Given I fill in the name and submit, When the API call succeeds, Then the network appears in the list and a success toast is shown.
**Priority**: Must
**FR**: FR-09.3

#### US-017: 보안그룹 관리
**Actor**: Operator
**Story**: As an operator, I want to manage security groups and their rules so that I can control network access to my servers.
**Acceptance Criteria**:
- Given I navigate to `:sec`, When the data loads, Then I see security groups with: name, description, rule count.
- Given I press Enter on a security group, When the detail view opens, Then I see ingress and egress rules with: protocol, port range, source/destination.
- Given I press `c` to create, When I fill name and description, Then a new security group is created.
- Given I'm on a security group detail, When I add a rule (direction, protocol, port, source CIDR), Then the rule is added and the detail refreshes.
- Given I select a rule, When I press `d` and confirm, Then the rule is deleted.
**Priority**: Must
**FR**: FR-09.4, FR-09.5, FR-09.6

---

### Cinder 서비스

#### US-018: 볼륨 리스트 및 상세 조회
**Actor**: Operator
**Story**: As an operator, I want to view volumes and their details so that I can manage storage resources.
**Acceptance Criteria**:
- Given I navigate to `:volumes`, When the data loads, Then I see: name, status, size (GB), type, encrypted, bootable, attached server.
- Given I press Enter on a volume, When the detail view opens, Then I see: basic info, attachment info (server, device path), and snapshot list.
**Priority**: Must
**FR**: FR-10.1, FR-10.2

#### US-019: 볼륨 생성
**Actor**: Operator
**Story**: As an operator, I want to create volumes so that I can provision storage for my servers.
**Acceptance Criteria**:
- Given I press `c` on the volumes list, When the form opens, Then I see: name (required), size GB (required), volume type, description, AZ, source.
- Given I fill required fields and submit, When the API succeeds, Then the volume appears in the list.
**Priority**: Must
**FR**: FR-10.3

#### US-020: 볼륨 액션 (삭제, 확장, 연결/분리, 상태 변경)
**Actor**: Operator / Admin
**Story**: As an operator, I want to delete, extend, and attach/detach volumes so that I can manage storage lifecycle. As an admin, I want to force-delete stuck volumes and change volume state.
**Acceptance Criteria**:
- Given I press `d` on a volume, When I confirm, Then the volume is deleted.
- Given I initiate extend on a volume, When I enter a new size larger than current, Then the volume is extended.
- Given I attach a volume to a server, When I select a server from the list, Then the volume is attached and attachment info updates.
- Given I detach a volume, When I confirm, Then the volume status returns to "available".
- Given I am an admin and a volume is stuck in "deleting" state, When I choose force-delete and confirm, Then the volume is force-deleted.
- Given I am an admin, When I change volume state to "available", Then the volume state is updated.
**Priority**: Must
**FR**: FR-10.4

#### US-021: 볼륨 스냅샷 관리
**Actor**: Developer / Operator
**Story**: As a developer, I want to view volume snapshots so that I can track backup points. As an operator, I want to delete snapshots to reclaim storage.
**Acceptance Criteria**:
- Given I navigate to snapshot view, When the data loads, Then I see: name, source volume, size, status, created date.
- Given I press Enter on a snapshot, When the detail view opens, Then I see: ID, name, source volume, size, status, created date.
- Given I press `d` on a snapshot, When I confirm, Then the snapshot is deleted.
**Priority**: Must
**FR**: FR-10.5

---

### 공통 UI 컴포넌트

#### US-022: 동적 폼 시스템
**Actor**: System
**Story**: As the system, I want a reusable form widget supporting multiple field types so that all create/edit forms share consistent UX.
**Acceptance Criteria**:
- Given a form definition with text/dropdown/multiselect/checkbox fields, When the form renders, Then each field type displays correctly with appropriate input behavior.
- Given I'm on a form, When I press Tab, Then focus moves to the next field.
- Given I'm on a form, When I press Enter, Then the form validates and submits if all required fields are filled.
- Given I'm on a form, When I press Esc, Then the form is cancelled and I return to the previous view.
- Given a field has validation (required, numeric, CIDR), When validation fails, Then the field is highlighted with an error message.
**Priority**: Must
**FR**: FR-11.3

#### US-023: 확인 다이얼로그
**Actor**: Operator
**Story**: As an operator, I want confirmation dialogs before destructive actions so that I don't accidentally delete resources.
**Acceptance Criteria**:
- Given I request deletion of a resource, When the dialog appears, Then it shows "Delete [resource-name]? (y/n)".
- Given the dialog is shown, When I press `y`, Then the action proceeds.
- Given the dialog is shown, When I press `n` or Esc, Then the action is cancelled.
**Priority**: Must
**FR**: FR-11.4

---

### Nova 서비스 (Admin 확장)

#### US-024: 서버 마이그레이션
**Actor**: Admin
**Story**: As an admin, I want to migrate servers between compute hosts so that I can perform host maintenance or balance load.
**Acceptance Criteria**:
- Given I select a server, When I choose "Live Migration", Then I can optionally select a destination host and the migration starts.
- Given I choose "Block Migration", When the migration starts, Then the server is live-migrated with local disk copy.
- Given I choose "Cold Migration", When I confirm, Then the server is shut down, migrated, and needs manual confirm/revert.
- Given a migration is in progress, When I check the server status, Then I see the migration state in the detail view.
- Given I am NOT an admin, When I view server actions, Then migration options are hidden.
**Priority**: Must
**FR**: FR-08.6

#### US-025: 서버 Evacuate
**Actor**: Admin
**Story**: As an admin, I want to evacuate servers from a failed host so that I can recover services during host failures.
**Acceptance Criteria**:
- Given a compute host is down, When I select a server on that host and choose "Evacuate", Then I can optionally select a destination host.
- Given evacuation succeeds, When the operation completes, Then the server is rebuilt on the new host and a success toast is shown.
- Given evacuation fails, When the error arrives, Then an error toast with the reason is shown.
**Priority**: Must
**FR**: FR-08.7

#### US-026: 서버 상태 강제 변경
**Actor**: Admin
**Story**: As an admin, I want to force-change a server's state so that I can recover servers stuck in transitional states (e.g., ERROR, BUILDING).
**Acceptance Criteria**:
- Given I select a server, When I choose "Force State", Then I see a dropdown with available states (active, error, etc.).
- Given I select a new state and confirm, When the API call succeeds, Then the server state updates and a success toast is shown.
- Given this is a destructive operation, When the confirm dialog appears, Then it warns about potential data inconsistency.
**Priority**: Must
**FR**: FR-08.8

#### US-027: 서버 스냅샷 (인스턴스 이미지 생성)
**Actor**: Operator
**Story**: As an operator, I want to create a snapshot of a running server so that I can back up the current state as a Glance image.
**Acceptance Criteria**:
- Given I select a server, When I choose "Create Snapshot", Then a form asks for the snapshot name.
- Given I submit the snapshot name, When the API call starts, Then a toast shows "Creating snapshot..." and the operation runs in background.
- Given the snapshot creation completes, When I navigate to Images, Then the new image appears in the list.
**Priority**: Should
**FR**: FR-08.9

---

### Neutron 서비스 (확장)

#### US-028: Floating IP 관리
**Actor**: Operator
**Story**: As an operator, I want to manage floating IPs so that I can provide external access to my servers.
**Acceptance Criteria**:
- Given I navigate to `:fip` or `:floatingip`, When the data loads, Then I see: IP address, status, associated server/port, floating network.
- Given I press `c`, When the form opens, Then I select an external network and a floating IP is allocated.
- Given I select a floating IP, When I choose "Associate", Then I can pick a server/port to attach it to.
- Given a floating IP is associated, When I choose "Disassociate", Then the IP is detached from the server.
- Given I press `d` on a floating IP, When I confirm, Then the IP is released.
**Priority**: Must
**FR**: FR-09.7

#### US-029: Network Agent 관리
**Actor**: Admin
**Story**: As an admin, I want to manage network agents so that I can monitor and maintain the Neutron agent fleet.
**Acceptance Criteria**:
- Given I navigate to `:agents`, When the data loads, Then I see: agent type, host, status (UP/DOWN), admin state (enabled/disabled), alive.
- Given I select an agent, When I choose "Disable" and provide a reason, Then the agent is disabled.
- Given a disabled agent, When I choose "Enable", Then the agent is re-enabled.
- Given I press `d` on a dead agent, When I confirm, Then the agent record is deleted.
- Given I am NOT an admin, When I navigate, Then agent management is hidden.
**Priority**: Must
**FR**: FR-09.8

---

### Cinder 서비스 (확장)

#### US-030: 볼륨 QoS 관리
**Actor**: Admin
**Story**: As an admin, I want to manage volume QoS policies so that I can control storage performance characteristics.
**Acceptance Criteria**:
- Given I navigate to `:qos`, When the data loads, Then I see QoS policies with: name, consumer, specs.
- Given I press `c`, When the form opens, Then I can create a QoS policy with name and specs (e.g., read_iops_sec, write_iops_sec).
- Given I select a QoS policy, When I press `d` and confirm, Then the policy is deleted.
**Priority**: Should
**FR**: FR-10.6

#### US-031: Storage Pool 조회
**Actor**: Admin
**Story**: As an admin, I want to view storage backend pools so that I can monitor capacity and plan storage allocation.
**Acceptance Criteria**:
- Given I navigate to `:pools`, When the data loads, Then I see: pool name, backend, total capacity, free capacity, provisioned.
- Given I am NOT an admin, When I try to access pools, Then the view is hidden.
**Priority**: Should
**FR**: FR-10.7

#### US-032: 볼륨 마이그레이션
**Actor**: Admin
**Story**: As an admin, I want to migrate volumes between storage backends so that I can rebalance or decommission backends.
**Acceptance Criteria**:
- Given I select a volume, When I choose "Migrate" and select a destination backend, Then the migration starts in background.
- Given a migration is in progress, When I check the volume, Then I see migration status in the detail view.
- Given this is a high-risk operation, When the confirm dialog appears, Then it warns about potential data loss on failure.
**Priority**: Should
**FR**: FR-10.8

---

### Identity 서비스 (Keystone Admin)

#### US-033: 프로젝트 관리
**Actor**: Admin
**Story**: As an admin, I want to manage projects so that I can organize tenants and their resources.
**Acceptance Criteria**:
- Given I navigate to `:projects`, When the data loads, Then I see: name, ID, enabled status, description.
- Given I press `c`, When the form opens, Then I see: name (required), description, domain, enabled.
- Given I press `d` on a project, When I confirm, Then the project is deleted.
- Given I am NOT an admin, When I try to access projects, Then the view is hidden.
**Priority**: Must
**FR**: FR-12.1

#### US-034: 사용자 관리
**Actor**: Admin
**Story**: As an admin, I want to manage users so that I can control who can access the cloud.
**Acceptance Criteria**:
- Given I navigate to `:users`, When the data loads, Then I see: name, ID, email, enabled status, default project.
- Given I press `c`, When the form opens, Then I see: name, password, email, default project (dropdown), domain.
- Given I press `d` on a user, When I confirm, Then the user is deleted.
**Priority**: Must
**FR**: FR-12.2

#### US-035: 역할 관리
**Actor**: Admin
**Story**: As an admin, I want to assign and revoke roles so that I can manage user permissions within projects.
**Acceptance Criteria**:
- Given I select a user or project, When I choose "Manage Roles", Then I see current role assignments.
- Given I choose "Add Role", When I select a user, project, and role, Then the role is assigned.
- Given I select an existing role assignment, When I choose "Remove" and confirm, Then the role is revoked.
**Priority**: Must
**FR**: FR-12.3

---

### Quota 관리

#### US-036: 프로젝트 Quota 관리
**Actor**: Admin
**Story**: As an admin, I want to view and modify project quotas so that I can control resource consumption per project.
**Acceptance Criteria**:
- Given I select a project, When I choose "Manage Quota", Then I see current quota values (cores, ram, instances, volumes, gigabytes, etc.).
- Given I modify quota values, When I submit, Then the quotas are updated.
- Given a quota value is invalid (negative, non-numeric), When I try to submit, Then validation highlights the error.
**Priority**: Must
**FR**: FR-13.1

---

### Image 서비스 (Glance)

#### US-037: 이미지 리스트 및 상세 조회
**Actor**: Developer
**Story**: As a developer, I want to view available images so that I can choose the right base image for my servers.
**Acceptance Criteria**:
- Given I navigate to `:images`, When the data loads, Then I see: name, status, disk format, size, visibility, created date.
- Given I press Enter on an image, When the detail view opens, Then I see: ID, name, status, disk/container format, size, checksum, min_disk, min_ram, architecture, OS type, visibility.
- Given I type `/ubuntu`, When the filter applies, Then only images containing "ubuntu" are shown.
**Priority**: Must
**FR**: FR-14.1, FR-14.2

#### US-038: 이미지 등록
**Actor**: Admin
**Story**: As an admin, I want to register new images so that users can provision servers with custom or updated base images.
**Acceptance Criteria**:
- Given I press `c` on the images list, When the form opens, Then I see: name, disk format (dropdown), container format, visibility, file path or URL.
- Given I fill required fields and submit, When the upload starts, Then a background task shows progress in the status bar.
- Given the upload completes, When I return to the images list, Then the new image appears.
**Priority**: Must
**FR**: FR-14.3

#### US-039: 이미지 수정 및 삭제
**Actor**: Admin
**Story**: As an admin, I want to edit image metadata and delete images so that I can maintain the image catalog.
**Acceptance Criteria**:
- Given I select an image, When I choose "Edit", Then I can modify: name, visibility, properties.
- Given I press `d` on an image, When I confirm, Then the image is deleted.
- Given I am NOT an admin, When I view images, Then edit/delete actions are hidden.
**Priority**: Must
**FR**: FR-14.4, FR-14.5

---

### Compute 관리 (Admin)

#### US-040: Aggregate 관리
**Actor**: Admin
**Story**: As an admin, I want to manage host aggregates so that I can group compute hosts for scheduling and availability zones.
**Acceptance Criteria**:
- Given I navigate to `:aggregates`, When the data loads, Then I see: name, availability zone, host count.
- Given I press `c`, When the form opens, Then I see: name (required), availability zone.
- Given I press Enter on an aggregate, When the detail view opens, Then I see hosts list and metadata.
- Given I choose "Add Host", When I select a host, Then the host is added to the aggregate.
- Given I choose "Remove Host" on a host, When I confirm, Then the host is removed.
- Given I press `d` on an aggregate, When I confirm, Then the aggregate is deleted.
**Priority**: Must
**FR**: FR-15.1

#### US-041: Compute Service 관리
**Actor**: Admin
**Story**: As an admin, I want to enable/disable compute services so that I can perform host maintenance without new VMs being scheduled there.
**Acceptance Criteria**:
- Given I navigate to `:compute-services` or `:cs`, When the data loads, Then I see: host, binary, status, state (up/down), updated_at.
- Given I select a service, When I choose "Disable", Then a form asks for a disable reason and the service is disabled.
- Given I select a disabled service, When I choose "Enable", Then the service is re-enabled.
**Priority**: Must
**FR**: FR-15.2

---

### Monitoring 대시보드

#### US-042: Hypervisor 조회
**Actor**: Admin
**Story**: As an admin, I want to view hypervisor status so that I can monitor compute host capacity and health.
**Acceptance Criteria**:
- Given I navigate to `:hypervisors` or `:hv`, When the data loads, Then I see: hostname, type, vCPUs (used/total), RAM (used/total), disk (used/total), running VMs.
- Given I press Enter on a hypervisor, When the detail view opens, Then I see full hypervisor details.
**Priority**: Must
**FR**: FR-16.1

#### US-043: 사용량 조회
**Actor**: Admin
**Story**: As an admin, I want to view resource usage per project so that I can monitor consumption and plan capacity.
**Acceptance Criteria**:
- Given I navigate to `:usage`, When the data loads, Then I see per-project usage: vCPUs, RAM, instances.
- Given I specify a date range, When the filter applies, Then usage data for that period is shown.
**Priority**: Must
**FR**: FR-16.2

#### US-044: 서버 이벤트 조회
**Actor**: Operator
**Story**: As an operator, I want to view server event history so that I can troubleshoot issues and audit actions.
**Acceptance Criteria**:
- Given I'm on a server detail view, When I navigate to the Events section, Then I see: action, start/end time, result, message.
- Given a recent migration event exists, When I view events, Then I see migration details.
**Priority**: Must
**FR**: FR-16.3

---

### RBAC 및 감사

#### US-045: 역할 기반 메뉴 제어
**Actor**: System
**Story**: As the system, I want to show/hide menus and actions based on the user's Keystone role so that non-admin users cannot access admin-only functions.
**Acceptance Criteria**:
- Given the user has admin role, When the app loads, Then all menus including admin functions are visible.
- Given the user has member role, When the app loads, Then admin-only menus (Identity, Aggregate, Compute Service, etc.) are hidden.
- Given the user's role changes mid-session (token refresh), When the new token is received, Then menu visibility updates accordingly.
**Priority**: Must
**FR**: FR-17.1

#### US-046: 고위험 작업 2단계 확인
**Actor**: Operator
**Story**: As an operator, I want destructive actions to require enhanced confirmation so that I don't accidentally cause damage.
**Acceptance Criteria**:
- Given I request deletion of a server, When the confirm dialog appears, Then I must type the server name to confirm (not just y/n).
- Given I request a force-delete or state change, When the dialog appears, Then a warning about potential consequences is shown before confirmation.
- Given I mistype the resource name, When I press Enter, Then the action is not executed and I can retry.
**Priority**: Must
**FR**: FR-17.2

#### US-047: 로컬 감사 로그
**Actor**: Admin
**Story**: As an admin, I want all CUD operations to be logged locally so that I can review what actions were taken.
**Acceptance Criteria**:
- Given I delete a server, When the action completes, Then a log entry is written with: timestamp, user, action, resource ID/name, result.
- Given I want to review past actions, When I check `~/.config/nexttui/audit.log`, Then I see a chronological log of all CUD operations.
- Given sensitive data is involved, When the log is written, Then passwords and tokens are never included.
**Priority**: Should
**FR**: FR-18.1

---

### 통합 조회

#### US-048: 서버-리소스 연관 뷰
**Actor**: Operator
**Story**: As an operator, I want to see all resources connected to a server in one view so that I can make informed operational decisions without switching between multiple screens.
**Acceptance Criteria**:
- Given I'm on a server detail view, When I see the network section, Then I see all interfaces with network name, fixed IP, floating IP, and security groups.
- Given I'm on a server detail view, When I see the storage section, Then I see all attached volumes with name, size, device path.
- Given I select a connected resource (volume, network, floating IP), When I press Enter, Then I navigate to that resource's detail view.
**Priority**: Should
**FR**: FR-19.1

---

## Technical Requirements (Non-Story)

아래 항목은 사용자 스토리로 변환이 부적합한 시스템 내부 요구사항:

- **TR-01**: Port/Adapter 패턴으로 API 레이어 디커플링 (FR-05)
- **TR-02**: Component trait 기반 모듈 시스템 (FR-07)
- **TR-03**: HashMap + TTL 단일 레벨 캐시 (FR-06)
- **TR-04**: Mock adapter로 API 없이 단위 테스트 가능 (FR-05.3)
- **TR-05**: 단일 정적 바이너리 빌드 (NFR-02)
- **TR-06**: 패스워드/시크릿 메모리 전용 유지, 로그 출력 금지 (NFR-05)
- **TR-07**: Admin 권한 감지 — 서비스 카탈로그/역할 기반으로 Admin 여부 판별, 비Admin 시 Admin 전용 메뉴 숨김
- **TR-08**: Identity/Glance/Placement 서비스별 Port trait 추가 (FR-05 확장)
- **TR-09**: Service Layer 전환 대비 — Adapter 인터페이스를 Phase 2에서 "Admin API GW 경유"로 교체 가능하도록 추상화 (FR-05.6)
- **TR-10**: VDI 기반 배포 — Windows/Linux 단일 바이너리, 관리망 내부 실행 (NFR-06)

## Story Map Summary

| 카테고리 | Must | Should | Could | 합계 |
|---------|------|--------|-------|------|
| 코어: 인증 | 3 | 0 | 0 | 3 |
| 코어: TUI | 4 | 0 | 0 | 4 |
| 코어: 비동기 | 2 | 0 | 0 | 2 |
| Nova (기본) | 5 | 0 | 0 | 5 |
| Nova (Admin 확장) | 3 | 1 | 0 | 4 |
| Neutron (기본) | 3 | 0 | 0 | 3 |
| Neutron (확장) | 2 | 0 | 0 | 2 |
| Cinder (기본) | 4 | 0 | 0 | 4 |
| Cinder (확장) | 0 | 3 | 0 | 3 |
| Identity (Admin) | 3 | 0 | 0 | 3 |
| Quota (Admin) | 1 | 0 | 0 | 1 |
| Glance | 3 | 0 | 0 | 3 |
| Compute 관리 (Admin) | 2 | 0 | 0 | 2 |
| Monitoring | 3 | 0 | 0 | 3 |
| 공통 UI | 2 | 0 | 0 | 2 |
| RBAC/감사 | 2 | 1 | 0 | 3 |
| 통합 조회 | 0 | 1 | 0 | 1 |
| **합계** | **42** | **6** | **0** | **48** |

Phase 1 스코프: Must 42개 + Should 6개 = 48개 스토리.
Should 항목: US-027 서버 스냅샷, US-030 QoS, US-031 Storage Pool, US-032 볼륨 마이그레이션, US-047 로컬 감사 로그, US-048 서버-리소스 연관 뷰.

### Phase 2 예정 서비스 (이번 Phase 미포함)
- Manila (Shared FS / NAS): Share Network, Migration, QoS, CIFS Account
- Cloudian (Object Storage): Policy, Bucket, Group, Monitor, Permission, QoS
- Network System Admin: Routing Table, VPC, Subnet, Routing Rule, NACL, External Network
- Placement: Resource Provider, Inventory
