# Platform Input and Accessibility Contract — PR-051

## Status and boundary

This is the engine-neutral boundary for native platform input. It is owned by
`engine-api::platform` and must not import Tauri, Servo, WebView2, AppKit,
GTK or other toolkit types. A platform adapter translates native events into
this contract; `servo-engine` receives only the engine-neutral result.

The contract is executable through three checked-in fixtures:

- `crates/engine-api/tests/fixtures/platform-input/linux.json`
- `crates/engine-api/tests/fixtures/platform-input/windows.json`
- `crates/engine-api/tests/fixtures/platform-input/macos.json`

The fixtures run through `platform_input_contract.rs` on the OS matrix. This is
contract evidence, not a claim that a native Tauri window or screen-reader
bridge has already been exercised on every OS.

## Identity and ordering

Every native event contains:

- `contract_version`, currently `1`;
- `platform`, one of `linux`, `windows` or `macos`;
- non-zero `event_id`;
- strictly increasing non-zero `sequence`;
- a bounded rational `scale_factor`;
- post-event `focus` state;
- exactly one typed event payload.

A batch must keep one platform and one scale factor. Duplicate IDs, changed
platform/scale, unknown versions, missing identity or non-increasing sequences
are rejected. Normalization is one-to-one: focus transitions remain explicit
`NormalizedPlatformEvent::FocusChanged` values instead of disappearing because
they do not become engine input.

## Scale and coordinates

Scale is represented as a reduced rational with numerator and denominator from
`1..=16`. Logical pointer coordinates are converted to integer device pixels
with deterministic nearest-integer rounding. Negative, overflowing or
out-of-viewport device coordinates are rejected. The viewport remains owned by
the engine-neutral surface contract and is expressed in device pixels.

The checked fixtures cover representative reference values:

| OS | Fixture scale |
|---|---:|
| Linux | `1/1` |
| Windows | `5/4` |
| macOS | `2/1` |

These values are fixture inputs, not declared OS floors or universal DPI claims.

## Focus and keyboard

Pointer, key and text events require focused state. A native adapter must emit a
focus transition before sending focused input; input after focus loss is
rejected with an explicit `FocusRequired` error.

The engine-neutral contract carries:

- `PointerMove`, `PointerDown`, `PointerUp` with a bounded device point after
  normalization;
- `KeyDown` and `KeyUp` with non-empty, NUL-free keys up to 128 bytes;
- `Text` with non-empty, NUL-free text up to 4096 bytes;
- `FocusChanged`, retained as a control event even though it has no direct
  `EngineCommand::Input` payload.

`servo-engine` maps the keyboard variants to Servo keyboard events only inside
its adapter module. No Servo keyboard type crosses `engine-api` or
`browser-core`.

## Accessibility boundary

The contract declares the narrow scope that can be tested without claiming a
complete accessibility implementation:

- focus events: contracted;
- keyboard navigation: contracted;
- text input: contracted;
- native screen-reader bridge: explicitly `Unsupported` with reason
  `native screen-reader bridge is outside PR-051`.

Unsupported paths must remain visible to callers and logs. They must not be
silently treated as successful accessibility support. The complete screen-reader
feature set remains a later card with its own OS evidence and threat/ownership
review.

## Failure and rollback

Invalid or unsupported native input returns a typed error and does not produce a
partial normalized event. The safe rollback is to disable the unsupported
platform path and retain the explicit error; do not drop an event or broaden a
support claim to make a smoke pass.
