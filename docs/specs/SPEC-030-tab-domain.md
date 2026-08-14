# SPEC-030 — tab domain model

> Status: accepted for implementation in PR-030; this specification refines the PR card and does not override the architecture or lifecycle ADRs.

## Identity

- Stable plan/Issue ID: PR-030 / #30
- Owner: `browser-domain`
- Milestone: M3 — State and persistence
- Status: accepted
- Source SHA/tree: bound by the PR and CI artifact

## Context

The browser core needs one authoritative, engine-neutral model for tab identity, lifecycle, visibility, focus, navigation intent, and the last committed URL. The frontend must not own tab truth, and Servo types must not enter the domain crate.

## Objective

Provide a serializable pure-domain `Tab` model whose legal lifecycle transitions, focus/visibility invariants, and stale-navigation rejection are executable and testable.

## Scope

- stable tab, profile, engine-instance, and navigation identifiers;
- `Created`, `Loading`, `Ready`, `Failed`, `Crashed`, `Closing`, and `Closed` lifecycle states;
- explicit visibility and focus state;
- last committed URL, pending URL, active navigation, and title;
- typed errors for invalid transitions, terminal state, stale navigation, and focusing a hidden tab;
- serde round-trip without engine or Tauri types.

## Out of scope

- tab collection/selection across multiple tabs;
- tab strip or frontend rendering;
- engine commands, Servo integration, surfaces, persistence repositories, session restore, and UI policy;
- process isolation or security claims beyond the crate boundary.

## Invariants and boundaries

- authority/ownership: `browser-domain` owns the tab state; frontend and Servo do not.
- allowed inputs and outputs: mutations accept typed domain IDs/value objects and return `Result`; no generic string command path is added.
- lifecycle/state invariants: only declared lifecycle edges are accepted; `Closed` is terminal; `Closing → Closed` is the sole completion edge.
- navigation invariant: only the current `NavigationId` may commit or fail; failure retains the last committed URL.
- visibility/focus invariant: focus requires `Visible`; hiding clears focus.
- security/privacy boundary: no DOM, page content, secrets, engine handles, or Tauri types are serialized.
- failure and recovery behavior: crash clears pending navigation and focus; close is explicit and terminal.

## Acceptance criteria

- [x] Legal lifecycle transition matrix is executable.
- [x] Stale navigation commit/failure is rejected without mutating current state.
- [x] Visibility/focus invariants are enforced.
- [x] Crash and close transitions are explicit.
- [x] State round-trips through serde without Servo/Tauri dependencies.
- [x] Invalid terminal mutations return typed errors.

## Testing and evidence

- Unit/contract: `cargo test -p browser-domain --offline` — 38 passing tests.
- Integration/E2E: out of scope for this pure domain card.
- Negative/security: stale navigation, illegal transitions, hidden focus, terminal mutation, and absence of engine types are covered.
- Performance/recovery: out of scope; crash state cleanup is covered by unit tests.
- Required artifact identity: repository, event, base/head/tree SHA, run/attempt, revision and digest are supplied by the PR/CI gate.

## Rollback and supersession

Revert the `browser-domain::tab` module and its export as one change. A later tab-manager or session schema may supersede this specification only by preserving the domain invariants and adding migration/compatibility tests.

## Open questions

None for this pure-domain slice. Collection ownership and persistence schema remain separate PRs.

## References

- `PR_PLAN.md` — PR-030
- `ARCHITECTURE.md` — data ownership and engine boundary
- `docs/contracts/runtime-lifecycle.md`
- `docs/pr-dag.yaml`
- `crates/browser-domain/src/tab.rs`
