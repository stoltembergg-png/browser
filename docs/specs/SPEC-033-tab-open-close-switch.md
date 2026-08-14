# SPEC-033 — tab open/close/switch integration

> Status: accepted for implementation in PR-033; this specification refines the PR card and does not override the UI shell contract, architecture, or lifecycle ADRs.

## Identity

- Stable plan ID: `PR-033`
- Issue: `#33`
- Owner: `browser-core::tab_ui::TabUiCoordinator`
- Source of truth: typed `CommandEnvelope`/`EventEnvelope` plus `TabManager`

## Behavior

1. `new_tab` is app-scoped, allocates deterministic test IDs, creates one manager record, registers it with `IpcBridge`, selects it, and emits `tab_created` followed by `tab_selected`.
2. `select_tab` requires the envelope `tab_id` and `target_tab_id` to match, then changes visibility/focus for only the target and emits one `tab_selected` event.
3. `close_tab` has a typed `target_tab_id` payload. The target must match the envelope scope; the manager retains a closed tombstone and the bridge unregisters the target.
4. Closing the active tab emits `tab_closed` and selects one live fallback. Closing an inactive tab emits only `tab_closed` and preserves the current selection.
5. Navigation start and engine events retain the tab identity and engine binding. A cross-tab or stale binding is rejected without mutating either tab.
6. The coordinator exposes no claim that Tauri commands, Servo surfaces, frame presentation, popup policy, or session restore are implemented.

## Security and privacy

- No generic IPC invoke path is introduced.
- Envelope scope and typed target are checked before manager mutation.
- Tab IDs, engine IDs, URLs, and titles remain bounded/opaque domain values; no page content or credentials enter events.

## Tests

- new tab creates and selects one manager record;
- switching updates only the target tab;
- active close chooses a live fallback;
- inactive close preserves selection;
- target mismatch is rejected;
- navigation state/event identity cannot cross tabs;
- stale engine binding cannot mutate state;
- close payload serde round-trips with `target_tab_id`.

## Rollback

Revert the coordinator and typed `CloseTab` schema change together. Keep the earlier PR-032 shell mock only if the UI contract and Rust schema remain synchronized.
