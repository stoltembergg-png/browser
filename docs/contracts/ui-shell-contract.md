# UI Shell Contract — PR-012

> Status: implemented M1 shell mock. This contract defines the typed command/event schema between the privileged Tauri frontend and the browser core. It does not implement browser state or engine behavior.

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

### Rejection rules

- Malformed JSON → `CommandRejected { reason: "malformed JSON: ..." }`
- Unknown command variant → rejected by serde deserialization
- Wrong schema version → `CommandRejected { reason: "unsupported version" }`
- Empty URL → `CommandRejected { reason: "URL must not be empty" }`
- Oversized URL → `CommandRejected { reason: "URL exceeds maximum length" }`

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
| `command_rejected` | `reason: String` | Core rejected a command |

### Rejection rules

- Malformed JSON → rejected
- Unknown event variant → rejected by serde deserialization
- Wrong schema version → rejected
- Oversized title → rejected

## Security constraints

- There is **no** generic `invoke` bridge. Every command is a named, typed variant.
- The frontend does not access filesystem, process, or network directly.
- CSP is `default-src 'self'; style-src 'self' 'unsafe-inline'; script-src 'self'`.
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
