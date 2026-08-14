# UI Shell Contract — PR-012

> Status: implemented M1 shell mock, extended by PR-032 tab-strip presentation and PR-041 download UI state. This contract defines the typed command/event schema and accessibility boundary between the privileged Tauri frontend and the browser core. It does not implement browser state or engine behavior.

## Objective

Define the typed commands, events and validation rules for the UI shell, without a generic `invoke` bridge.

## Schema version

`UI_CONTRACT_VERSION = 1`

All envelopes carry a `version` field. The core rejects mismatched versions; the UI rejects mismatched event versions.

## Commands (UI → core)

Every command is wrapped in a `CommandEnvelope`:

```json
{
  "version": 1,
  "request_id": "req-1",
  "tab_id": "tab-1",
  "command": { "type": "navigate", "url": "https://example.com" }
}
```

| Command type | Payload | Notes |
|---|---|---|
| `navigate` | `url: String` | URL must be non-empty and ≤ 8192 bytes |
| `reload` | — | Reload the current tab |
| `go_back` | — | Navigate back in history |
| `go_forward` | — | Navigate forward in history |
| `stop` | — | Stop loading |
| `new_tab` | — | Create a new tab |
| `close_tab` | `target_tab_id: TabId` | Close the specified tab |
| `select_tab` | `target_tab_id: TabId` | Switch to the specified tab |
| `download_start` | `url: String, suggested_name: String, content_length: Option<u64>` | App-level; the policy decides the destination |
| `download_cancel` | `download_id: u64` | App-level; cancel an active download |
| `download_retry` | `download_id: u64` | App-level; restart a failed/cancelled download |

Download commands are app-level (no `tab_id`) because a download is bound to the profile, not to a tab. The `suggested_name` is metadata only — the destination is decided by the brokered download policy in core, never by the UI.

### Rejection rules

- Malformed JSON → `CommandRejected { reason: "malformed JSON: ..." }`
- Unknown command variant → rejected by serde deserialization
- Wrong schema version → `CommandRejected { reason: "unsupported version" }`
- Empty URL → `CommandRejected { reason: "URL must not be empty" }`
- Oversized URL → `CommandRejected { reason: "URL exceeds maximum length" }`
- Scoped `close_tab`/`select_tab` target different from envelope `tab_id` → `TargetMismatch`

## Events (core → UI)

Every event is wrapped in an `EventEnvelope`:

```json
{
  "version": 1,
  "tab_id": "tab-1",
  "event": { "type": "title_changed", "title": "Example" }
}
```

| Event type | Payload | Notes |
|---|---|---|
| `tab_created` | `tab_id: TabId` | A new tab was created |
| `tab_closed` | `tab_id: TabId` | A tab was closed |
| `tab_selected` | `tab_id: TabId` | Active tab changed |
| `navigation_started` | `url: String` | Navigation began |
| `navigation_committed` | `url: String` | First paint / URL confirmed |
| `navigation_finished` | `url: String` | Navigation completed |
| `navigation_failed` | `reason: String` | Navigation failed |
| `title_changed` | `title: String` | Page title changed (≤ 1024 bytes) |
| `download_started` | `download_id: u64, suggested_name: String` | Download began streaming |
| `download_progress` | `download_id: u64, bytes: u64` | Bytes streamed so far |
| `download_completed` | `download_id: u64, final_path: String` | Safe final path reached |
| `download_failed` | `download_id: u64, reason: String` | Failed (reason ≤ 1024 bytes) |
| `download_cancelled` | `download_id: u64` | Cancelled by user or broker |
| `command_rejected` | `reason: String` | Core rejected a command |

### Rejection rules

- Malformed JSON → rejected
- Unknown event variant → rejected by serde deserialization
- Wrong schema version → rejected
- Oversized title → rejected

## Tab strip presentation — PR-032

The shell renders tab records as `button[role="tab"]` controls inside the `tablist`, with a separate labeled close button for each record. The selected tab has `aria-selected="true"` and `tabindex="0"`; inactive tabs use roving `tabindex="-1"`. Arrow keys, Home and End move focus and send the typed `select_tab` command. The selected tab labels the shared `tabpanel` through `aria-labelledby` and `aria-controls="tab-panel"`.

Unknown/stale tab IDs are ignored without creating UI state. Closing the active tab selects an existing fallback record, or clears the panel label and omnibox when no record remains. This is a presentation contract only; browser-core remains the source of truth for tab lifecycle and selection.
## PR-033 command-to-manager integration

`TabUiCoordinator` is the engine-neutral integration seam used by tests and future Tauri adapters. It validates the envelope through `IpcBridge`, applies `new_tab`, `close_tab`, and `select_tab` to `TabManager`, and emits typed `EventEnvelope` records using the same `tab_id`. Navigation start and engine events carry the target tab binding; a binding for another tab or engine incarnation is rejected before state mutation.

The coordinator keeps closed tab records as manager tombstones, unregisters them from the IPC allowlist, and selects a live fallback only when the closed tab was active. Engine-host commands (`reload`, back, forward and stop) remain outside this slice and are not claimed as implemented here.

## PR-041 download UI state

`DownloadUiCoordinator` is the engine-neutral seam for download presentation state. It validates app-level download commands through `IpcBridge`, applies them to `DownloadManager` (which wraps the brokered `DownloadBroker`), and emits typed download events. `download_start` never accepts a destination — the final path is allocated by the broker policy from the profile root, so the UI cannot change destination after policy. Active downloads track streamed bytes; `download_cancel` removes the temporary part, and `download_retry` restarts a failed/cancelled download from its history record with a fresh id. Terminal records (completed, cancelled, failed) are kept in a bounded history (50 entries, most recent first).

On (re)start the broker recovers the download root: orphaned `.part` temporary files from an interrupted session are removed and the id sequence advances past any previously used id. Completed files are never touched, and interrupted downloads never become final files.

## Security constraints

- There is **no** generic `invoke` bridge. Every command is a named, typed variant.
- The frontend does not access filesystem, process, or network directly.
- CSP é `default-src 'self'`, com scripts/styles em assets externos e sem `unsafe-inline`/`unsafe-eval`;
- capability `main-window` é escopada à janela `main` e começa com `permissions: []`;
- Unknown or malformed commands/events produce `CommandRejected` — the UI does not crash.

## Frontier mock behavior

The PR-012 shell mock:
- Renders an omnibox, tab bar and navigation buttons.
- Sends typed commands via `sendCommand()` (no `invoke`).
- Renders typed events via `renderEvent()`.
- Has a `default:` case in `renderEvent` for unknown events.
- Uses ARIA roles (`tablist`, `tab`, `toolbar`, `status`) for accessibility.

## Non-goals

This contract does not implement:
- Real browser state (tabs/navigation are mocked).
- Tauri command registration (that arrives when the bridge connects to core).
- Engine integration (that is PR-013+).
- Schema evolution beyond version 1 (that requires an ADR on backward compatibility).
