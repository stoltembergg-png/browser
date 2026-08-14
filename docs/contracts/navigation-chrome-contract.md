# Navigation chrome contract

Status: proposed implementation contract for PR-027. This document does not
claim a real Servo surface or reference-platform E2E readiness.

## Scope

The privileged browser chrome owns the omnibox projection and the enablement
of back, forward, reload, and stop controls. It does not own page content,
engine handles, persistence, or tab storage.

## Typed state

`NavigationChromeState` contains only:

- the last committed or pending URL shown in the omnibox;
- the page title, when a URL has been observed;
- loading state;
- a redacted/display-safe error string;
- back/forward capability flags.

The state is updated by `browser_domain::ui::UiEvent` values. An engine or tab
boundary must reject stale, wrong-tab, wrong-generation, and post-close events
before they are projected into the chrome.

## Event behavior

| Event | Chrome behavior |
|---|---|
| `navigation_started` | Set URL, enter loading, clear the previous error, enable Stop. |
| `navigation_committed` | Update URL and clear the previous error. |
| `navigation_finished` | Set URL, leave loading, clear error, enable Reload when a URL exists. |
| `navigation_failed` | Leave loading and expose the typed error in the status region. |
| `navigation_cancelled` | Leave loading, clear transient error, disable Stop. |
| `title_changed` | Update the title only after a URL has been observed. |
| `command_rejected` | Leave loading and expose the rejection in the status region. |

Unknown events and unrelated events are ignored by the state projection. The
frontend also ignores events for unknown tabs and renders an explicit status
message instead of mutating another tab.

## Control enablement

- Back and Forward use the history capability flags supplied by the core.
- Reload requires a known URL and a non-loading tab.
- Stop requires a loading tab.
- The controls remain typed commands; the frontend does not invoke a generic
  bridge or access engine APIs directly.

## Local shell mock behavior

The local shell mock keeps a bounded history cursor per tab so the contract can
exercise control transitions before a real Tauri bridge is available:

- a new navigation truncates any forward branch and appends one URL;
- Back and Forward move the cursor without appending duplicate entries;
- Reload replays the current URL without changing the cursor;
- Stop emits `navigation_cancelled` only while a tab is loading;
- a control with no valid target emits `command_rejected` and leaves the tab
  state explicit rather than guessing a fallback.

This is test-support behavior, not engine or browser-core authority. Real
history capabilities and navigation results must still come from the typed
core/engine path before PR-027 can leave its blocked state.

## Evidence boundary

The Rust unit/integration tests and shell-contract tests prove the projection,
negative states, cancellation, and malformed/stale event handling. They are
engine-neutral contract evidence only. PR-027 cannot be closed until PR-026's
real Servo/surface evidence and the required reference-platform E2E flow are
available on the current SHA/tree.
