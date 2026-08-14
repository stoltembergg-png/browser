# SPEC-031 — tab manager and engine event routing

> Status: accepted for implementation in PR-031; this specification refines the PR card and does not override the architecture or lifecycle ADRs.

## Identity

- Stable plan/Issue ID: PR-031 / #31
- Owner: `browser-core`
- Milestone: M3 — State and persistence
- Status: accepted
- Source SHA/tree: bound by the PR and CI artifact

## Context

PR-030 provides the pure `Tab` state machine. The core now needs one collection owner that creates and closes tabs, tracks the selected tab, binds each tab to an engine incarnation, and rejects events from a different tab or stale engine generation.

## Objective

Provide a browser-core `TabManager` with explicit tab collection ownership, selection, close/shutdown behavior, and engine-event routing fenced by tab identity and engine binding generation.

## Scope

- create tabs with profile and engine-instance bindings;
- reject duplicate tab IDs;
- select one live tab while maintaining visibility/focus invariants;
- rebind an existing tab to a new engine incarnation with a new generation;
- route engine events only under the current `(tab_id, engine_instance_id, binding_generation)`;
- preserve closed tab records and reject late events/mutations;
- close all live tabs during manager shutdown and clear selection.

## Out of scope

- frontend/tab-strip presentation;
- real Servo or native surfaces;
- persistence/session restore;
- multi-window or popup policy;
- engine process management beyond opaque binding identity.

## Invariants and boundaries

- authority/ownership: `TabManager` in `browser-core` owns the collection and active-tab selection; frontend remains presentation-only.
- binding invariant: a route is accepted only when its tab ID and complete binding equal the current binding.
- isolation invariant: an event rejected for one tab cannot mutate another tab.
- generation invariant: every create/rebind receives a monotonic binding generation.
- close invariant: closed records remain inspectable, but late events and selection are rejected.
- shutdown invariant: all live tabs become explicitly `Closed`; subsequent manager mutations return `AlreadyShutdown`.
- engine boundary: only `engine_api::EngineEvent` and opaque IDs cross this module; no Servo types are imported.

## Acceptance criteria

- [x] Create/duplicate/lookup and tab-to-engine binding are tested.
- [x] Selection updates visibility/focus and rejects closed tabs.
- [x] Wrong-tab and stale-engine events are rejected without cross-tab mutation.
- [x] Rebinding increments generation and fences old events.
- [x] Close preserves an explicit closed record and rejects late events.
- [x] Shutdown closes every live tab, clears selection, and is terminal.
- [x] Navigation/title/crash/exit events update only the current bound tab.

## Testing and evidence

- Unit/contract: `cargo test -p browser-core --offline` — 117 passing tests, including 8 manager tests.
- Combined core contracts: `cargo test -p browser-domain -p engine-api -p browser-core --offline` — 203 passing tests.
- Negative/security: duplicate tab, wrong tab binding, stale rebind, closed late event, and shutdown-after-terminal cases are covered.
- Static: fmt, Clippy, architecture, documentation, security, quality-gate scripts, and Python gate suite.
- Required artifact identity: repository, event, base/head/tree SHA, run/attempt, revision and digest are supplied by the PR/CI gate.

## Rollback and supersession

Revert the `tab_manager` module and its export as one change. A later tab-manager revision or multi-window policy must preserve the binding fence and closed-record semantics, with a superseding specification and regression tests.

## Open questions

None for this single-manager core slice. Persistence and popup ownership remain separate cards.

## References

- `PR_PLAN.md` — PR-031
- `ARCHITECTURE.md` — data ownership and engine boundary
- `docs/contracts/runtime-lifecycle.md`
- `docs/pr-dag.yaml`
- `crates/browser-domain/src/tab.rs`
- `crates/browser-core/src/tab_manager.rs`
