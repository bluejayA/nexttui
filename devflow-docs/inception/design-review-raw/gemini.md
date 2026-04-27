I will start by reading the primary design document and the associated requirements to understand the proposed structural fix for the cross-project scoping bug.
I will now examine the `Action` enum in `src/action.rs` and the `run_worker` function in `src/worker.rs` to understand the current implementation and evaluate the impact of the proposed `StampedAction` wrapper.
I will now examine `src/infra/rbac.rs` to understand the `ActionKind` enum and `RbacGuard` struct, which will help me evaluate the proposed project-scope guard integration.
I will now examine `src/port/auth.rs` and `src/port/types.rs` to understand the token and project definitions, which will clarify how `active_scope` is handled in the authentication layer.
I will search for `TokenScopeFingerprint` and examine `src/adapter/auth/scoped_session.rs` to understand how the `ScopedAuthSession` handles the active scope and its fingerprinting.
I will now examine `src/adapter/http/neutron.rs` to understand how the filtering logic is currently implemented and evaluate the feasibility of injecting the `tenant_id` query parameter as proposed.
I will now search for the `list_security_groups` implementation in `src/adapter/http/neutron.rs` to understand how it currently handles requests and filtering.
I will now read the implementation of `list_security_groups` in `src/adapter/http/neutron.rs` starting from line 342.
I will now search for the implementation of `build_security_group_query` in `src/adapter/http/neutron.rs` to see how it currently constructs the query parameters.
This design review addresses the **BL-P2-085 Cross-Project Scoping** structural fix. The proposed plan is a comprehensive and pragmatic approach to a P0 security boundary bug, balancing rigorous protection with the reality of a large existing codebase.

### A. Semantic correctness of FR2 (target→origin shift)
The shift from "target-matching" to "origin-matching" (`StampedAction`) is a clever and effective solution for the TUI's stateful environment. 
*   **Success**: It perfectly addresses the **TOCTOU** (Time-of-Check to Time-of-Use) vulnerability where a user switches projects while a mutation form is open. 
*   **Risk**: It relies on the assumption that the `resource_id` within the `Action` belongs to the `origin_project_id`. While true for UI-driven workflows (where IDs are discovered via scoped list calls), it doesn't prevent a "Project A" action from targeting "Project B" resource if the user stays in Project A.
*   **Mitigation**: This is adequately covered by **FR1** (adapter-level scoping) and **FR4** (form validation), which ensure that the IDs available to the user are already project-scoped.

### B. `StampedAction` wrapper maintainability
The proposal to wrap 55 dispatch sites manually is the highest regression risk in the plan.
*   **Gap**: Manual wrapping at each `channel.send` call site is prone to omission.
*   **Improvement**: Centralize stamping by providing a `dispatch` helper on the application context (e.g., `app_ctx.dispatch(action)`). This helper would handle the `StampedAction` wrapping internally using the current active scope, reducing the diff surface and preventing "unstamped" mutations from slipping through.

### C. Threat model gaps
*   **Background Tasks**: Background refresh/polling tasks (e.g., `poll_migration_progress` in `src/worker.rs:109`) should be explicitly audited. If these tasks trigger mutations (like auto-cleaning stale resources), they must carry a valid stamp or be exempt via `is_mutation() == false`.
*   **Toast Delivery**: The design should ensure that the `CrossProjectToast` is sent *before* any `AppEvent::ApiError` resulting from a blocked action to avoid user confusion.

### D. Audit event schema + PII trade-off
The structured audit event schema is excellent for forensics.
*   **PII**: Using plaintext `actor_user_id` is acceptable for internal KT Cloud tools. However, ensure it uses the **Keystone UUID** rather than the username to maintain audit trail integrity across username changes.
*   **Fingerprint**: Concatenating fields for the SHA256 fingerprint is sound, but ensure the `resource_id` is canonicalized (e.g., empty string if `None`) to prevent fingerprint jitter.

### E. Glance visibility exclusion
Excluding Glance from **FR1** (List filtering) is a justified trade-off due to the complexity of `public` and `shared` visibility models.
*   **Safety check**: Ensure that Glance mutation variants (`DeleteImage`, `UpdateImage`) are strictly categorized as `is_mutation() == true` in **C1** so that **FR2** (Worker guard) still protects them.

### F. Assumption verification rigor (A1/A2/A3)
*   **A2 (Pure fn workaround)**: Extracting URL/query builders into pure functions for unit testing is a highly idiomatic Rust solution to the lack of wiremock/mockito. It provides high confidence in `tenant_id` injection without polluting the dependency graph.
*   **A1 (Active scope propagation)**: The verification that `RbacGuard` and `AuthProvider` already hold `project_id` confirms that the structural foundation for this fix is already in place.

### G. Rust idiomaticity and test strategy
*   **Idiomaticity**: `CrossProjectGuard` might be more ergonomic if implemented as a trait or an `impl` on `StampedAction` rather than a set of free functions.
*   **Test Strategy**: The **T1 (mock unit only)** strategy is appropriate for a structural fix of this scale. However, ensure the `Action::is_mutation()` match block is `#[deny(non_exhaustive_omitted_patterns)]` or equivalent to prevent silent failure when adding new variants.

### H. Risk log + action items

#### **Must-fix** (Blocking this BL)
1.  **C1 (ActionKind)**: Use a `match` with no default (`_`) in `is_mutation()` to ensure every new `Action` variant is explicitly categorized.
2.  **C2 (Dispatch)**: Provide a centralized `dispatch_action(action)` helper to automate `StampedAction` creation and minimize churn across 55+ sites.
3.  **C6 (Neutron)**: Verify that `list_subnets` correctly handles the `network_id` filter alongside the new `tenant_id` filter (AND logic).

#### **Should-consider** (Strongly recommended)
1.  **Audit**: Use Keystone UUID for `actor_user_id` instead of username for stable auditing.
2.  **UX**: Set `CrossProjectToast` level to `Warning` to distinguish user-originated scope errors from system API failures.
3.  **Design**: Implement `CrossProjectGuard` as a trait to improve discoverability within the codebase.

#### **Future-BL** (Defer with justification)
1.  **Glance Scoping**: Comprehensive filtering for `public`/`shared` images. (Complex visibility logic deserves a dedicated BL).
2.  **Audit Dashboard**: A UI for viewing `CrossProjectBlockEvent` logs. (Current scope is focused on protection/logging).
