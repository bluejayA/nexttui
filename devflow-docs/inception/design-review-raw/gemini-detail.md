Loaded cached credentials.
I will start by reading the design document `devflow-docs/inception/application-design.md` focusing on the `# DETAIL Mode` section, along with the requirements and synthesis documents to understand the full context.
I'll start by checking the current definitions of `TokenScope`, `Action`, `AppEvent`, and the worker spawning logic in `src/port/types.rs`, `src/action.rs`, `src/event.rs`, and `src/worker.rs` to ensure the design's "Codebase fit" and "Concurrency hazards" are properly addressed.
I'll read `src/app.rs` to understand how the event loop is structured and where the `epoch` check should be integrated to ensure that stale events from previous contexts are correctly ignored.
I'll examine `src/adapter/http/base.rs` to see how the endpoint cache is implemented and how it can be invalidated, which will help clarify the "Redundant Invalidation" issue I've identified.
The technical review of the **DETAIL** section for feature **BL-P2-031** is as follows. The design is robust in its use of epochs for concurrency control, but there are critical ambiguities in ownership and atomicity that must be addressed before code generation.

### Top 3 Critical Issues

1.  **Redundant Orchestration & Ownership Ambiguity (Dependency Direction)**
    *   **What**: `ContextSwitcher` (Service) depends on both `ContextSessionPort` (Port) and `KeystoneRescopeAdapter` (Adapter). The procedure description has `ContextSwitcher` manually calling `rescope` and `invalidate_all` between `session.begin` and `session.commit`.
    *   **Why**: This splits the atomic transition logic across two layers. If `ContextSessionPort` is the boundary for a "Session," it should encapsulate the *entire* session-level change (rescope + catalog refresh + endpoint invalidation).
    *   **Fix**: Move the rescope and invalidation calls *inside* the `ContextSessionPort` implementation. `ContextSwitcher` should only call `port.transition(target)`, which handles the Keystone handshake and cache clearing as a single unit.

2.  **Incomplete Rollback Contract (Atomicity Gaps)**
    *   **What**: `ContextSessionPort::rollback(handle)` is defined, but `SessionHandle` is opaque. If `rescope` succeeds but `invalidate_all` or `catalog_refresh` fails, the `AuthProvider` state is already mutated (new token scoped to new project).
    *   **Why**: Without restoring the old token/catalog in the `AuthProvider`, the "rollback" only reverts the UI state, leaving the HTTP client with a new-project token but a "Failed" switcher state—a torn state.
    *   **Fix**: `SessionHandle` must contain the **captured previous `Token` and `TokenScope`**. `rollback` must explicitly re-inject these into the `AuthProvider` (KeystoneAuthAdapter) to guarantee a return to the exact previous state.

3.  **Passive UI Highlight Hazard (UI Correctness)**
    *   **What**: `ContextIndicator::highlight_for(Duration)` implies an active timer.
    *   **Why**: `ratatui` components are passive and only move on `render`. A `highlight_for` method called once will not "stop" the highlight after N seconds unless the component tracks time relative to the render loop.
    *   **Fix**: Change the signature to `set_context(&mut self, snapshot: &ContextSnapshot, highlight: bool)`. Internally, the widget must store `last_switch_at: Instant` and check `Instant::now() - last_switch_at < highlight_duration` during every `render` call.

### Specific Signature Corrections

```rust
// src/context.rs
// Change: Add Epoch to ContextSnapshot to ensure snapshot is self-verifying.
pub struct ContextSnapshot {
    pub target: ContextTarget,
    pub epoch: Epoch, 
    pub token: Token, // Explicitly include the scoped token
    pub captured_at: chrono::DateTime<chrono::Utc>,
}

// src/port/auth.rs (ContextSessionPort trait)
// Change: Atomic transition instead of begin/commit split to prevent logic leakage.
#[async_trait]
pub trait ContextSessionPort: Send + Sync {
    /// Performs rescope + catalog refresh. Returns handle for rollback.
    async fn transition(&self, target: &ContextTarget) -> Result<(SessionHandle, ContextSnapshot), SwitchError>;
    async fn rollback(&self, handle: SessionHandle) -> Result<(), SwitchError>;
}

// src/worker.rs
// Change: Worker loop MUST receive the versioned envelope.
pub async fn run_worker(
    // ... existing ...
    mut action_rx: mpsc::UnboundedReceiver<VersionedEvent<Action>>,
    event_tx: mpsc::UnboundedSender<VersionedEvent<AppEvent>>,
)
```

### Missing Components or Interfaces

1.  **Epoch Enforcement Point (Controller)**: The design lacks the explicit check logic in `App::handle_event`. It must be stated that `App` maintains a `current_epoch: AtomicU64` and *drops* any `VersionedEvent<AppEvent>` where `event.epoch < current_epoch`.
2.  **Opaque `SessionHandle` Definition**: Needs to be a struct in `src/port/auth.rs` containing `old_token: Token` and `old_scope: TokenScope`.

### Verdict: **APPROVE-WITH-CHANGES**

**Reasoning**: The concurrency model (Epoch + CancellationToken) is excellent and follows Rust best practices for async TUI. However, the layering of the `ContextSwitcher` vs `ContextSessionPort` is "chatty" and risks leaving the application in a torn state if the rollback doesn't restore the `AuthProvider` token. Consolidating the rescope logic into the `Port` implementation ensures true atomicity.

**Next Step**: Once these ownership and rollback details are clarified in the document, the project is ready for the **RED** phase of PR1 (Infrastructure).
