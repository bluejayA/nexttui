### A. Semantic correctness of FR2 (target→origin shift)

1. **What is correct:** 2b is a valid minimal fix for TOCTOU/cache-stale submission after scope switch. That threat is clearly captured.  
Refs: [requirements.md:34](/Users/jay.ahn/projects/infra/nexttui/.worktrees/bl-p2-085-cross-project-scoping/devflow-docs/inception/requirements.md:34), [requirements.md:37](/Users/jay.ahn/projects/infra/nexttui/.worktrees/bl-p2-085-cross-project-scoping/devflow-docs/inception/requirements.md:37), [application-design.md:60](/Users/jay.ahn/projects/infra/nexttui/.worktrees/bl-p2-085-cross-project-scoping/devflow-docs/inception/application-design.md:60)

2. **Main gap:** origin-match does **not** prove target resource belongs to active project. If user never switches (origin==active), cross-project target can still pass.  
Refs: [requirements.md:41](/Users/jay.ahn/projects/infra/nexttui/.worktrees/bl-p2-085-cross-project-scoping/devflow-docs/inception/requirements.md:41), [action.rs:6](/Users/jay.ahn/projects/infra/nexttui/.worktrees/bl-p2-085-cross-project-scoping/src/action.rs:6)

3. **Concrete slip scenario:** Glance is excluded from FR1, list can be broad, and delete is UI-available for admin; FR2 origin check won’t block same-origin submission.  
Refs: [application-design.md:461](/Users/jay.ahn/projects/infra/nexttui/.worktrees/bl-p2-085-cross-project-scoping/devflow-docs/inception/application-design.md:461), [application-design.md:464](/Users/jay.ahn/projects/infra/nexttui/.worktrees/bl-p2-085-cross-project-scoping/devflow-docs/inception/application-design.md:464), [glance.rs:84](/Users/jay.ahn/projects/infra/nexttui/.worktrees/bl-p2-085-cross-project-scoping/src/adapter/http/glance.rs:84), [image/mod.rs:124](/Users/jay.ahn/projects/infra/nexttui/.worktrees/bl-p2-085-cross-project-scoping/src/module/image/mod.rs:124)

### B. `StampedAction` wrapper maintainability

1. Wrapping ~55 dispatch sites is feasible but high regression surface; this project already has one centralized envelope (`VersionedEvent<Action>`), so another envelope at send sites increases churn.  
Refs: [application-design.md:123](/Users/jay.ahn/projects/infra/nexttui/.worktrees/bl-p2-085-cross-project-scoping/devflow-docs/inception/application-design.md:123), [action_channel.rs:24](/Users/jay.ahn/projects/infra/nexttui/.worktrees/bl-p2-085-cross-project-scoping/src/context/action_channel.rs:24)

2. Single stamped channel is the better of the two options (avoids split pipeline bugs).  
Ref: [application-design.md:126](/Users/jay.ahn/projects/infra/nexttui/.worktrees/bl-p2-085-cross-project-scoping/devflow-docs/inception/application-design.md:126)

3. `is_mutation()` exhaustive match is good, but it must stay in lockstep with existing `action_to_kind()` mapping to avoid policy drift.  
Refs: [application-design.md:69](/Users/jay.ahn/projects/infra/nexttui/.worktrees/bl-p2-085-cross-project-scoping/devflow-docs/inception/application-design.md:69), [worker.rs:151](/Users/jay.ahn/projects/infra/nexttui/.worktrees/bl-p2-085-cross-project-scoping/src/worker.rs:151)

4. Simpler alternative: stamp once at `ActionSender` boundary (centralized), not 55 call sites, while preserving TOCTOU defense.

### C. Threat model gaps (security UX lens)

1. **Replay/in-flight action gap:** worker currently does not epoch-drop stale actions; it uses epoch only to stamp outgoing events. Side effects can still execute.  
Refs: [worker.rs:63](/Users/jay.ahn/projects/infra/nexttui/.worktrees/bl-p2-085-cross-project-scoping/src/worker.rs:63), [worker.rs:107](/Users/jay.ahn/projects/infra/nexttui/.worktrees/bl-p2-085-cross-project-scoping/src/worker.rs:107), [app.rs:600](/Users/jay.ahn/projects/infra/nexttui/.worktrees/bl-p2-085-cross-project-scoping/src/app.rs:600), [action_channel.rs:31](/Users/jay.ahn/projects/infra/nexttui/.worktrees/bl-p2-085-cross-project-scoping/src/context/action_channel.rs:31)

2. **Switch race:** mid-switch dispatch is blocked (good), but already spawned worker tasks are not tied to `CancellationRegistry`; long-running operations can continue.  
Refs: [app.rs:562](/Users/jay.ahn/projects/infra/nexttui/.worktrees/bl-p2-085-cross-project-scoping/src/app.rs:562), [worker.rs:100](/Users/jay.ahn/projects/infra/nexttui/.worktrees/bl-p2-085-cross-project-scoping/src/worker.rs:100), [switcher.rs:152](/Users/jay.ahn/projects/infra/nexttui/.worktrees/bl-p2-085-cross-project-scoping/src/context/switcher.rs:152)

3. **Token-refresh boundary:** FR3 relying on cached `RbacGuard.project_id` is fragile; refresh event carries roles only and app writes `project_id=None`.  
Refs: [event.rs:178](/Users/jay.ahn/projects/infra/nexttui/.worktrees/bl-p2-085-cross-project-scoping/src/event.rs:178), [app.rs:617](/Users/jay.ahn/projects/infra/nexttui/.worktrees/bl-p2-085-cross-project-scoping/src/app.rs:617), [rbac.rs:113](/Users/jay.ahn/projects/infra/nexttui/.worktrees/bl-p2-085-cross-project-scoping/src/infra/rbac.rs:113)

4. Toast timing is mostly okay, but there is a UX race with pre-dispatch progress toast and later block toast.

### D. Audit event schema + PII trade-off

1. Schema is strong for baseline forensics (who/where/what/why/outcome).  
Refs: [requirements.md:58](/Users/jay.ahn/projects/infra/nexttui/.worktrees/bl-p2-085-cross-project-scoping/devflow-docs/inception/requirements.md:58), [application-design.md:335](/Users/jay.ahn/projects/infra/nexttui/.worktrees/bl-p2-085-cross-project-scoping/devflow-docs/inception/application-design.md:335)

2. Missing: `guard_layer` (FR2/FR3/FR4), and an epoch/correlation field for multi-event reconstruction.

3. `target_project_id` is semantically ambiguous under FR2-origin mismatch (often it is actually origin, not true target).

4. Fingerprint concat needs canonicalization (length-prefix or canonical JSON). Plain concat risks boundary ambiguity collisions.  
Ref: [application-design.md:357](/Users/jay.ahn/projects/infra/nexttui/.worktrees/bl-p2-085-cross-project-scoping/devflow-docs/inception/application-design.md:357)

5. Plaintext `actor_user_id` is acceptable for internal admin tooling if retention/access is documented; hashing can be deferred.

### E. Glance visibility exclusion

Not safe to defer as currently framed. Excluding Glance from FR1 while relying on FR2/FR3 leaves a practical mutation surface (`DeleteImage`) inconsistent with the stated policy (“no cross-project mutate/delete”).  
Refs: [requirements.md:17](/Users/jay.ahn/projects/infra/nexttui/.worktrees/bl-p2-085-cross-project-scoping/devflow-docs/inception/requirements.md:17), [application-design.md:461](/Users/jay.ahn/projects/infra/nexttui/.worktrees/bl-p2-085-cross-project-scoping/devflow-docs/inception/application-design.md:461), [image/mod.rs:124](/Users/jay.ahn/projects/infra/nexttui/.worktrees/bl-p2-085-cross-project-scoping/src/module/image/mod.rs:124)

### F. Assumption verification rigor (A1/A2/A3)

1. **A1 is not tight yet.** The doc conflates scope types: `TokenScope` has no project_id, and `ScopedAuthSession` has no project fields.  
Refs: [application-design.md:12](/Users/jay.ahn/projects/infra/nexttui/.worktrees/bl-p2-085-cross-project-scoping/devflow-docs/inception/application-design.md:12), [types.rs:42](/Users/jay.ahn/projects/infra/nexttui/.worktrees/bl-p2-085-cross-project-scoping/src/port/types.rs:42), [scoped_session.rs:33](/Users/jay.ahn/projects/infra/nexttui/.worktrees/bl-p2-085-cross-project-scoping/src/adapter/auth/scoped_session.rs:33)

2. **A2 is directionally reasonable** (no mock infra). But FR1 response-side acceptance is not covered by query-builder-only tests.  
Refs: [Cargo.toml:38](/Users/jay.ahn/projects/infra/nexttui/.worktrees/bl-p2-085-cross-project-scoping/Cargo.toml:38), [requirements.md:30](/Users/jay.ahn/projects/infra/nexttui/.worktrees/bl-p2-085-cross-project-scoping/devflow-docs/inception/requirements.md:30), [application-design.md:270](/Users/jay.ahn/projects/infra/nexttui/.worktrees/bl-p2-085-cross-project-scoping/devflow-docs/inception/application-design.md:270)

3. **A3 verification is solid.** `build_disambiguated_opts` is pure and locally contained.  
Ref: [server/mod.rs:32](/Users/jay.ahn/projects/infra/nexttui/.worktrees/bl-p2-085-cross-project-scoping/src/module/server/mod.rs:32)

### G. Rust idiomaticity and test strategy

1. Pure URL builders are idiomatic and useful. Since `paginated_list` only concatenates `path?query`, fidelity is decent for request-shape tests.  
Ref: [http/mod.rs:97](/Users/jay.ahn/projects/infra/nexttui/.worktrees/bl-p2-085-cross-project-scoping/src/adapter/http/mod.rs:97)

2. But they do not prove response-side scope filtering or endpoint-policy correctness (required by FR1).  
Ref: [requirements.md:30](/Users/jay.ahn/projects/infra/nexttui/.worktrees/bl-p2-085-cross-project-scoping/devflow-docs/inception/requirements.md:30)

3. `CrossProjectGuard` as free functions is idiomatic here (policy is pure, stateless). Trait abstraction is unnecessary unless multiple policy backends are planned.

4. T1 mock-only merge-blocking is risky for a P0 boundary; keep it, but add at least one automated high-risk integration/contract test path.

### H. Risk log + action items

**Must-fix (blocking this BL)**  
- FR1: Add response-side cross-project filtering/assertions, not query-only tests.  
- FR2/FR3: Close same-origin foreign-target mutation gap, especially Glance delete path.  
- Worker: Enforce stale-action drop/cancel before side effects, not event-drop only.  
- FR5/C9: Canonicalize fingerprint input (delimiter/versioned schema) before hashing.

**Should-consider (strongly recommended)**  
- C1/worker: Add parity tests between `is_mutation()` and `action_to_kind()`.  
- C2: Centralize stamping in `ActionSender` to reduce 55-site regression risk.  
- C9: Add `guard_layer` and `epoch/correlation_id` to audit schema.

**Future-BL (defer with justification)**  
- FR5: Configurable user-id hashing mode for stricter privacy deployments.  
- FR1/Glance: Full visibility/owner policy matrix and adapter-level normalization.  
- Test infra: Introduce HTTP mock server harness for end-to-end request assertions.
