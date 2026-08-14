# SPEC-032 — accessible tab strip UI

> Status: accepted for implementation in PR-032; this specification refines the PR card and does not override the UI shell contract or architecture ADRs.

## Identity

- Stable plan/Issue ID: PR-032 / #32
- Owner: `apps/desktop/frontend`
- Milestone: M3 — State and persistence
- Status: accepted
- Source contract: `docs/contracts/ui-shell-contract.md`

## Objective

Render the tab list, active state, close affordance, and keyboard-accessible tab navigation on the existing Tauri shell mock. Keep all behavior presentation-only and use the already versioned typed UI command/event contract.

## Scope

- render tab records as an accessible `tablist` with `button[role=tab]` controls;
- expose active state through `aria-selected`, focus through roving `tabindex`, and the controlled `tabpanel` through `aria-controls`/`aria-labelledby`;
- provide a separately labeled close button with a usable hit target;
- send `select_tab` and `close_tab` typed commands instead of mutating core state locally;
- support ArrowLeft/ArrowRight/Home/End and Enter/Space keyboard behavior;
- keep omnibox/panel state synchronized after selection and active-tab close;
- ignore events for unknown/stale tab IDs without creating UI state.

## Out of scope

- browser-core manager semantics;
- real Tauri command registration or Servo/native surface;
- popup/new-window policy;
- persistence/session restore;
- pixel-perfect cross-platform visual baselines.

## UI and security boundaries

- the frontend remains a presentation layer and does not import or reference Servo;
- commands remain named variants from the existing UI shell contract; no generic invoke bridge is introduced;
- tab IDs and event identity are treated as untrusted; unknown event IDs are ignored fail-closed;
- close and selection controls are keyboard reachable and have accessible labels;
- the mock does not access filesystem, process, or network.

## Acceptance criteria

- [x] Tab records render with explicit `role="tab"`, `aria-selected`, and active styling.
- [x] Each tab controls the shared `tabpanel`; the panel is labelled by the selected tab.
- [x] Close is a separate labeled button with a minimum target and no nested interactive controls.
- [x] Keyboard navigation supports ArrowLeft/ArrowRight/Home/End and Enter/Space.
- [x] Selection emits `select_tab`; close emits `close_tab`; no generic invoke is used.
- [x] Unknown tab events and stale selections do not mutate UI state.
- [x] Closing the active tab updates fallback selection, panel labeling, and omnibox safely.

## Testing and evidence

- Contract/accessibility: `python3 -m unittest tests.test_shell_contract -v` — 13 tests passing.
- JavaScript syntax: extracted inline script checked by `node --check` — exit code 0.
- Existing shell/security contracts remain required and are run in the PR gate.
- Visual posture is deterministic CSS and semantic state; no cross-OS pixel baseline is claimed by this slice.

## Rollback

Revert the frontend and shell-contract test changes together to restore the PR-012 single-tab mock. The typed UI shell contract remains unchanged.

## References

- `PR_PLAN.md` — PR-032
- `docs/contracts/ui-shell-contract.md`
- `ARCHITECTURE.md` — frontend ownership and no-Servo boundary
- `TESTING_STRATEGY.md` — UI/core contract and accessibility expectations
- `apps/desktop/frontend/index.html`
- `tests/test_shell_contract.py`
