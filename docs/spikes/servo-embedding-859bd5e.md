# Spike Report: Servo Embedding Feasibility

> **Status:** completed
> **Date:** 2026-08-13
> **Servo revision:** `859bd5edd60c0fb162a1f73c083a23e55474faf7`
> **Related:** PR-013, ADR-003

## Objective

Prove that Servo's embedding API can be used to create/destroy a webview, spin the event loop, and receive frame-ready notifications, against a pinned revision. The spike is disposable: it does not add Servo as a workspace dependency or produce a stable API. It documents findings and limitations.

## Method

Examined the Servo source tree at revision `859bd5e` via the GitHub API. Read the embedding crate (`components/servo/`), the webview delegate, the example (`winit_minimal.rs`), and the servoshell event loop. No native compilation was performed — the spike is a static analysis of API surface and types.

## API Findings

### 1. Servo builder and lifecycle

```rust
// components/servo/servo.rs
pub struct ServoBuilder { ... }
impl ServoBuilder {
    pub fn build(self) -> Servo;
    pub fn event_loop_waker(self, waker: Box<dyn EventLoopWaker>) -> Self;
    pub fn opts(self, opts: Opts) -> Self;
    pub fn preferences(self, prefs: Preferences) -> Self;
}

pub struct Servo(Rc<ServoInner>);
impl Servo {
    pub fn spin_event_loop(&self);
    pub fn setup_logging(&self);
}
```

**Finding:** `Servo` is constructed via `ServoBuilder`. The builder requires an `EventLoopWaker` implementation. `Servo` is `Clone` (wraps `Rc<ServoInner>`).

### 2. EventLoopWaker trait

```rust
// components/servo/servo.rs
pub trait EventLoopWaker {
    fn clone_box(&self) -> Box<dyn EventLoopWaker>;
    fn wake(&self);
}
```

**Finding:** The waker is a `Box<dyn EventLoopWaker>` stored inside `Servo`. When Servo needs the embedder to process events, it calls `wake()`. The embedder must ensure `spin_event_loop()` is eventually called. The `clone_box` method is required because Servo clones the waker internally.

### 3. WebView creation

```rust
// components/servo/webview.rs
pub struct WebViewBuilder { ... }
impl WebViewBuilder {
    pub fn new(servo: &Servo, rendering_context: Rc<dyn RenderingContext>) -> Self;
    pub fn url(self, url: Url) -> Self;
    pub fn delegate(self, delegate: Rc<dyn WebViewDelegate>) -> Self;
    pub fn hidpi_scale_factor(self, scale: Scale<f32, DIP, DevicePixel>) -> Self;
    pub fn build(self) -> WebView;
}

pub struct WebView(Rc<RefCell<WebViewInner>>);
```

**Finding:** WebView creation requires:
- A `Servo` reference
- A `RenderingContext` (can be `OffscreenRenderingContext` or `SoftwareRenderingContext` for headless)
- A `WebViewDelegate` implementation

### 4. WebViewDelegate trait

```rust
// components/servo/webview_delegate.rs
pub trait WebViewDelegate {
    fn notify_url_changed(&self, webview: WebView, url: Url) {}
    fn notify_page_title_changed(&self, webview: WebView, title: Option<String>) {}
    fn notify_load_status_changed(&self, webview: WebView, status: LoadStatus) {}
    fn notify_new_frame_ready(&self, webview: WebView) {}
    fn notify_focus_changed(&self, webview: WebView, focused: bool) {}
    fn notify_closed(&self, webview: WebView) {}
    fn notify_crashed(&self, webview: WebView, reason: String, backtrace: Option<String>) {}
    // ... 20+ methods, all with default implementations
}
```

**Finding:** All delegate methods have default implementations. The minimum required for rendering is `notify_new_frame_ready`. The delegate receives `WebView` by value (it's `Clone`).

### 5. WebView operations

```rust
impl WebView {
    pub fn load(&self, url: Url);
    pub fn reload(&self);
    pub fn go_back(&self, amount: usize) -> TraversalId;
    pub fn go_forward(&self, amount: usize) -> TraversalId;
    pub fn paint(&self);
    pub fn resize(&self, new_size: PhysicalSize<u32>);
    pub fn notify_input_event(&self, event: InputEvent) -> InputEventId;
    pub fn focus(&self);
    pub fn blur(&self);
    pub fn show(&self);
    pub fn hide(&self);
    pub fn set_throttled(&self, throttled: bool);
}
```

**Finding:** WebView exposes load, reload, back/forward, paint, resize and input. These map directly to the `UiCommand` variants defined in PR-012.

### 6. spin_event_loop behavior

```rust
// components/servo/servo.rs
impl ServoInner {
    fn spin_event_loop(&self) -> bool {
        if self.shutdown_state.get() == ShutdownState::FinishedShuttingDown {
            return false;
        }
        // ... processes paint, embedder messages, delegate callbacks
    }
}
```

**Finding:** `spin_event_loop` returns `false` when shutting down, `true` otherwise. It must be called after each batch of input events or WebView operations. It is the heartbeat of the engine.

## Limitations

1. **`Rc<RefCell>` — not `Send`:** `Servo` and `WebView` use `Rc<RefCell>` internally. They cannot cross thread boundaries. The engine host must own the thread affinity and call `spin_event_loop` on the same thread that created the `Servo` instance. This confirms the architecture's "engine thread affinity" model.

2. **No async API:** All operations are synchronous. The embedder must pump `spin_event_loop` manually. There is no `async fn` variant — the engine host actor must wrap synchronous calls in `spawn_blocking` or a dedicated thread.

3. **RenderingContext required:** WebView creation requires a `Rc<dyn RenderingContext>`. For headless/testing, `OffscreenRenderingContext` or `SoftwareRenderingContext` are available. For production, a `WindowRenderingContext` tied to a native window handle is needed.

4. **Sparse documentation:** The Servo Book describes embedding as "sparse and in progress". The primary reference is the `winit_minimal.rs` example. API names may change between Servo revisions.

5. **System libraries:** Compiling the Servo crate requires WebKitGTK, JavaScriptCore, and other system dependencies. This is consistent with the Tauri dependency from PR-011.

6. **Multiprocess mode:** Servo supports a `multiprocess` option, but the single-process mode is the default and simpler for the MVP. The architecture's process model evolution (PR-064+) handles this.

## Mapping to engine-api contract

The `engine-api` contract (ARCHITECTURE.md §6) maps to the Servo embedding API as follows:

| engine-api concept | Servo API |
|---|---|
| `BrowserEngine::create()` | `ServoBuilder::build()` |
| `EngineHandle` | `Servo` + `WebView` handles |
| `EventLoopWaker` | `servo::EventLoopWaker` trait |
| `spin_event_loop()` | `Servo::spin_event_loop()` |
| `WebViewDelegate` | `servo::WebViewDelegate` trait |
| Engine commands (Navigate, Reload, Back, Forward, Stop) | `WebView::load()`, `reload()`, `go_back()`, `go_forward()`, `set_throttled()` |
| Engine events (NavigationStarted, TitleChanged, LoadProgress, FrameReady) | `WebViewDelegate::notify_*` methods |

## Conclusion

Servo's embedding API is **viable** for the browser's initial engine. The builder, waker, webview, and delegate patterns map cleanly to the architecture's engine-api contract. The `Rc<RefCell>` constraint confirms the need for thread-affinity isolation in the engine host. The spike recommends proceeding to PR-016 (adapter implementation) with the contract defined in `engine-api`.

## Disposition

This spike is disposable. No code from the Servo source tree was added to the workspace. The findings inform the `engine-api` contract types added in this PR and the ADR-003 proposal.
