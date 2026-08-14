# Tauri capabilities map — PR-045

## Status

`proposed` with the PR-045 local-shell evidence. This map does not ratify ADR-007 or claim process isolation.

## Capability inventory

| Capability | Target | Permissions | Rationale |
|---|---|---|---|
| `main-window` | Tauri window `main` | `[]` | The current shell has no Tauri command/event/plugin API enabled. |

The empty permission list is intentional. Filesystem, path, shell/process, HTTP/network, SQL, dialog and updater plugins are not available to the frontend by default. Adding one requires an explicit capability entry and a negative test.

## Privileged UI boundary

- Fixed local origin: `http://tauri.localhost`.
- Window label: `main`.
- Frame: top-level only; iframe callers are rejected.
- Lifecycle: caller generation must match the expected generation.
- IPC: `browser-core::ipc_bridge::IpcBridge::validate_command_from` checks caller context before the typed envelope allowlist.
- Commands: only the `browser_domain::ui::UiCommand` variants are accepted; there is no generic `invoke` fallback.

## CSP

The shell CSP is declared in `apps/desktop/src-tauri/tauri.conf.json`:

```text
default-src 'self';
script-src 'self';
style-src 'self';
img-src 'self' data: asset:;
font-src 'self';
connect-src 'none';
object-src 'none';
base-uri 'none';
frame-ancestors 'none';
form-action 'none';
frame-src 'none';
media-src 'none'
```

The frontend uses `styles.css` and `app.js`; inline style/script blocks are not permitted by the fixture test.

## Negative evidence

- `browser-core` rejects attacker origin, wrong window, iframe and stale generation before recording a request.
- `browser-desktop` parses the capability and config fixtures and rejects inline execution, remote `devUrl`/window URL and privileged plugin prefixes.
- Existing IPC tests reject malformed/unknown/oversized/duplicate/wrong-tab/misscoped envelopes.

## Out of scope

Page content, Servo surface integration, real Tauri command registration, OS process isolation, permissions prompts and future plugin capabilities remain separate cards/gates.
