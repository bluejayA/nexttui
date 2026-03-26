# Application Design — BL-P2-010 RBAC 역할 세분화

**Mode**: DETAIL
**Depth**: Standard
**Timestamp**: 2026-03-26T18:00:00+09:00

## 변경 개요

현재 `is_admin: bool` 이분법을 `EffectiveRole` enum (Admin/Member/Reader) 3단계로 전환.
`can_perform()`, `can_access_route()` 인터페이스는 변경 없이 내부 로직만 교체.

## 변경 대상 컴포넌트

### 1. `src/infra/rbac.rs` — 핵심 변경

#### 1a. EffectiveRole enum 추가

```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum EffectiveRole {
    Reader,   // 최소 권한
    Member,   // CRUD 허용
    Admin,    // 전체 허용
}

impl EffectiveRole {
    /// Keystone 역할 목록에서 가장 높은 권한의 역할 결정.
    /// 알 수 없는 역할은 무시, 아무것도 매칭 안 되면 Reader.
    pub fn from_roles(roles: &[TokenRole]) -> Self {
        let mut effective = EffectiveRole::Reader;
        for role in roles {
            let level = match role.name.to_lowercase().as_str() {
                "admin" => EffectiveRole::Admin,
                "member" | "operator" => EffectiveRole::Member,
                "reader" => EffectiveRole::Reader,
                _ => continue,  // 알 수 없는 역할 무시
            };
            if level > effective {
                effective = level;
            }
        }
        effective
    }
}
```

**설계 결정:**
- `PartialOrd + Ord` derive — Reader < Member < Admin 순서로 비교 가능
- `"operator"` → Member 매핑 — 기존 Phase 1 user-stories의 Operator 역할 호환
- 알 수 없는 역할은 `continue`로 무시 (Reader 폴백이 아닌 무시 — 다른 알려진 역할이 있으면 그걸 사용)
- 아무 역할도 매칭 안 되면 Reader (최소 권한 원칙)

#### 1b. RbacState 변경

```rust
struct RbacState {
    roles: Vec<TokenRole>,
    project_id: Option<String>,
    effective_role: EffectiveRole,  // was: is_admin: bool
    capabilities: HashSet<Capability>,
}
```

#### 1c. can_perform() 로직 변경

```rust
pub fn can_perform(&self, action: ActionKind) -> bool {
    let role = self.state.read().map(|s| s.effective_role).unwrap_or(EffectiveRole::Reader);
    match role {
        EffectiveRole::Admin => true,
        EffectiveRole::Member => !Self::is_admin_only_action(action),
        EffectiveRole::Reader => action == ActionKind::Read,
    }
}
```

#### 1d. is_admin() 하위 호환

```rust
pub fn is_admin(&self) -> bool {
    self.effective_role() == EffectiveRole::Admin
}

pub fn effective_role(&self) -> EffectiveRole {
    self.state.read().map(|s| s.effective_role).unwrap_or(EffectiveRole::Reader)
}
```

#### 1e. update_roles() 변경

```rust
pub fn update_roles(&self, roles: Vec<TokenRole>, project_id: Option<String>) {
    let effective = EffectiveRole::from_roles(&roles);
    if let Ok(mut s) = self.state.write() {
        s.roles = roles;
        s.project_id = project_id;
        s.effective_role = effective;
        s.capabilities.clear();
    }
}
```

### 2. `src/worker.rs` — 변경 없음

`can_perform(ActionKind)` 인터페이스 동일. Worker 코드 수정 불필요.

### 3. `src/ui/sidebar.rs` — 변경 없음

`can_access_route()` 인터페이스 동일. Sidebar 코드 수정 불필요.

### 4. `src/demo.rs` — 변경 최소

`RbacGuard::new()` 후 `update_roles(admin)` 호출 — 기존과 동일.

### 5. `src/main.rs` — 변경 없음

`RbacGuard::new()` 생성 후 Worker에 전달 — 기존과 동일.

## 하위 호환성

| 기존 API | 변경 여부 | 비고 |
|----------|----------|------|
| `is_admin()` | 유지 | 내부적으로 `effective_role() == Admin` |
| `can_perform(ActionKind)` | 유지 | 내부 로직만 변경 |
| `can_access_route(&Route)` | 유지 | 내부 로직 동일 |
| `has_capability(resource, action)` | 유지 | Phase 2 준비용, 변경 없음 |
| `filter_routes()` | 유지 | `can_access_route` 위임 |
| `filter_actions()` | 유지 | `can_perform` 위임 |

## Phase 2 전환 경로 (B단계)

A단계 완료 후 B단계 전환 시 변경 범위:
- `can_perform()` 내부: `match role` → `has_capability()` 위임
- `EffectiveRole::from_roles()` → `derive_capabilities(roles)` 추가
- `is_admin_only_action()` + `is_admin_only_route()` 폐기 (~30줄)
- `can_perform()` 인터페이스 동일 — 호출자 변경 없음
