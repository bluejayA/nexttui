Loaded cached credentials.
I will begin by reading the design and requirement documents to evaluate the component list and PR strategy.
I've noticed a discrepancy between the requirements for BL-P2-031 and the content of `devflow-docs/inception/requirements.md`. I'll now check the directory for the correct requirements file or any additional context.
I've found a conflict between the `application-design.md` (BL-P2-031) and `requirements.md` (BL-P2-029). I'll now check `devflow-docs/inception/design-review-raw/` for the correct requirements, possibly within the "Codex adversarial review" files.
I'll re-read `requirements.md` to confirm if it truly is BL-P2-029 or if I missed the 10 FR + 5 NFR for BL-P2-031, as `codex.md` is quite large.
The `requirements.md` file is for the wrong BL. I'll search `codex.md` for `BL-P2-031` to find the correct requirements, as it's likely contained there.
There's a discrepancy, so I'll check `devflow-docs/inception/design-review-raw/codex.md` for the "FR" and "NFR" requirements.
`codex.md` looks like a session log, mirroring my instructions. I'll search for a different "FR-01" in it and read the first 100 lines to find the correct 10 FRs and 5 NFRs for BL-P2-031.
The previous agent also found the wrong `requirements.md`. I'll use the prompt's requirements and `application-design.md` to reconstruct the FRs. I'll also check `devflow-docs/inception/design-review-raw/gemini.md` for the missing 10 FRs and 5 NFRs.
`gemini.md` appears to be my session log, which is odd. I'll search `codex.md` for "10 FR" and read the surrounding text to find the functional requirements.
The 10 FRs and 5 NFRs for BL-P2-031 aren't in a single file, but I've inferred them from the prompt and `application-design.md`. I'll now check the codebase, specifically `src/adapter/auth/token_cache.rs` and `src/worker.rs`, to ensure the design aligns with the current implementation.
`TokenCacheStore` supports `TokenScope`. I'll check `src/worker.rs` and `src/app.rs` for worker spawning and state management.
`run_worker` lacks epoch/cancellation, which PR1 addresses. The design seems complete, though I'm curious if project switching clears the app state; `ContextSwitcher` and `App` hint at this. `ContextEpoch` and `CancellationRegistry` provide a good passive/active balance, and the PR boundaries are solid, albeit with a large PR2. Naming and types align with conventions, though I'll verify if `ContextPicker` and `ContextIndicator` belong in `src/ui/` as `Controllers`.
`src/ui/` components lack suffixes, so `context_indicator.rs` and `context_picker.rs` are better names. I'll check `src/component.rs` for the trait definition.
`Component` is core.

### Issues:
1. **State Clearing:** `ContextEpoch` alone isn't enough; we need an `AppEvent::ContextChanged` to clear UI state post-switch.
2. **Epoch Coupling:** `Worker` needs `epoch` validation, so `AppEvent` needs an `epoch` field, and `Action` must be stamped with the current `epoch` in PR1 to avoid race conditions.
3. **PR1 Scope:** Modifying `Action`, `AppEvent`, and `Worker` is deep infra; it's the right place, but all `Action` sites will need updates.

### Changes:
- **Add:** `AppEvent::ContextChanged`.
- **Merge/Split:** `SwitchStateMachine` can stay separate; `KeystoneRescopeAdapter` is fine as-is.

### Verdict:
**APPROVE-WITH-CHANGES**

### Reasoning:
Great stale event handling, but needs explicit UI state clearing. PR1's epoch coupling needs better definition to manage scope. Safety details like `ConfirmDialog` fingerprints are excellent. Cross-cloud switching is complex but seemingly supported by `TokenCacheStore`. I'll now check `src/event.rs` for `AppEvent`'s structure.
`AppEvent` has many variants; adding `epoch` to each is too much. I'll check `src/main.rs`'s event loop to see if wrapping `AppEvent` in a `VersionedEvent` struct or similar is feasible.
I've reviewed the `application-design.md` and codebase. `ContextEpoch` is great for stale events, but a "Clear State" event is needed for the UI. PR1's scope should be clearer on `Action` and `AppEvent` changes. The PR1-PR6 approach works if PR1 covers epoch plumbing. I'll now draft my response.### Review: BL-P2-031 Keystone Rescoping Application Design

Overall, the design is technically sound and aligns well with the existing Port/Adapter architecture. The use of `ContextEpoch` and `CancellationRegistry` directly addresses the high-risk concurrency issues inherent in TUI context switching.

#### Top 3 Issues
1. **Missing UI State Invalidation Event**: While `ContextEpoch` prevents *stale* events from updating the UI, it doesn't proactively clear current data. Switching projects without a "Reset" signal will leave the user looking at Project A's servers until Project B's servers load (or if they fail to load, indefinitely).
   - *Suggestion*: Add `AppEvent::ContextChanged { target: ContextId }`. All `resource_list` and `detail_view` components must handle this by clearing their internal `Vec<T>` or `state` to ensure visual consistency during the transition.
2. **PR1 "Plumbing" Scope Underestimated**: To make `ContextEpoch` effective, every `Action` must be stamped with an `epoch` at dispatch, and `AppEvent` must echo it back. Modifying the `Worker` alone is insufficient without updating the `Action` and `AppEvent` definitions.
   - *Suggestion*: Explicitly include `Action` and `AppEvent` schema updates in PR1. Wrap `AppEvent` in a `VersionedEvent { event: AppEvent, epoch: u64 }` or add `epoch` to the envelope to avoid modifying every variant of `AppEvent`.
3. **`ContextSwitcher` / `App` Ownership Ambiguity**: The design lists `App` as holding the epoch, but `ContextSwitcher` as the orchestrator. If `ContextSwitcher` is a standalone `Service`, it needs a clean way to increment the `App`'s epoch without circular dependencies.
   - *Suggestion*: `ContextSwitcher` should return the new `epoch` as part of its `Commit` result, or `App` should provide a `fn increment_epoch(&mut self) -> u64` that the switcher calls.

#### Components Analysis
*   **Add**: `AppEvent::ContextChanged` (Essential for UI cleanup).
*   **Modify**: `Action` (Add `epoch: u64` field in PR1).
*   **Refine**: `ContextPicker` — Ensure it's placed in `src/ui/context_picker.rs` to match the project's layout convention (similarly for `ContextIndicator`).
*   **Merge**: `SwitchStateMachine` could be private to `ContextSwitcher` if it has no external consumers, but keeping it as a separate `Service` for PR2 is acceptable for unit testing.

#### PR Boundary Concerns
*   **PR1 (Infrastructure)**: This is the most "breaking" PR. It must update all `Action` creation sites (input handlers, timers) to include the current `epoch`. It should be merged only when the `Worker` is ready to handle/echo them.
*   **PR2 (Core)**: Safe. It introduces the logic without exposing it to the user.
*   **PR5 (Safety)**: Excellent inclusion of the `ConfirmDialog` fingerprint. This mitigates the risk of destructive actions if a user "forgets" they just switched projects.

#### Verdict: **APPROVE-WITH-CHANGES**

**Reasoning**: The design is robust regarding stale-data isolation, but the "Clear UI" requirement is a critical missing piece for UX. Explicitly defining the epoch-passing contract in PR1 will prevent significant refactoring later in the cycle.
