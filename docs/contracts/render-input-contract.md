# Real Servo render/input contract

- **Status:** proposed for PR-026 validation
- **Authority:** `engine-api` contract plus Servo revision `859bd5edd60c0fb162a1f73c083a23e55474faf7`
- **Adapter:** `servo-engine` with feature `servo-backend`
- **Surface strategy in PR-026:** `SoftwareRenderingContext` on the engine thread

## Lifecycle

1. Validate API version and a non-zero `SurfaceSpec` viewport.
2. Create `SoftwareRenderingContext` and make it current on the engine thread.
3. Build `Servo` with a cloned `EventLoopWaker`.
4. Build a `WebView` with `WebViewDelegate` and optional initial URL.
5. Pump `Servo::spin_event_loop()` after commands and while the fixture waits.
6. On `notify_new_frame_ready`, call `prepare_for_rendering`, `WebView::paint`, `read_to_image`, require at least one non-zero pixel, store the SHA-256 `frame_digest`, and then `RenderingContext::present`. A resize is accepted only after a subsequent frame (`frame_count` strictly increases).
7. After `LoadStatus::Complete`, request `WebView::take_screenshot` and require its callback before accepting input.
8. Drop the `WebView`, then drop the `Servo` handle; Servo's official `Drop` implementation performs the ordered shutdown.

## Normalized input

`engine-api::contract::InputEvent` is engine-neutral and uses integer device pixels:

- `pointer_move`, `pointer_down`, `pointer_up` with a bounded viewport point;
- `text` with a non-empty, NUL-free 4096-byte limit.

The adapter translates these to Servo `MouseMoveEvent`, `MouseButtonEvent`, and bounded keyboard down/up pairs. Servo types do not cross into `engine-api` or `browser-core`.

## Resize and scale

Resize is applied to both `RenderingContext::resize` and `WebView::resize`. The initial adapter uses a scale factor of `1.0`; native window scale negotiation is intentionally deferred until a Tauri/window handle contract exists. The engine host owns last-value-wins coalescing.

## Evidence and non-vacuity

The real contract test uses a local HTTP fixture and must observe:

- HTTP fixture URL and at least one frame through the delegate;
- text input reflected by the fixture title;
- pointer click navigating to `/clicked`;
- viewport resize;
- same-thread evidence;
- ordered shutdown without timeout/deadlock.

When `PR026_ARTIFACT_PATH` is set, the test writes an identity-bound JSON artifact. The E2E rejects a non-increasing resize frame count; `scripts/servo_evidence_check.py` rejects missing identity, wrong Servo revision, zero frames, skipped cases, wrong surface strategy, blank readback (`frame_non_blank`), or missing SHA-256 digest. No-run, no-frame, blank-frame, skip, and stale artifacts are `NO_GO`.

## Out of scope

Window/native-handle attachment, GPU presentation, tabs, downloads, permissions, DevTools, accessibility, process isolation, and claims of cross-platform runtime support remain outside PR-026. Those require their own platform evidence and contracts.
