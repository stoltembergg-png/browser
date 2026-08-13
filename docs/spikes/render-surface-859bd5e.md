# Spike Report: Native Render Surface Feasibility

> **Status:** completed
> **Date:** 2026-08-13
> **Servo revision:** `859bd5edd60c0fb162a1f73c083a23e55474faf7`
> **Related:** PR-014, ADR-002, ADR-003

## Objective

Prove that Servo's `RenderingContext` trait and its implementations (`WindowRenderingContext`, `OffscreenRenderingContext`, `SoftwareRenderingContext`) can support the browser's render surface requirements: frame visibility, paint/present, input/resize, and shutdown barrier.

## Method

Static analysis of the `RenderingContext` trait and its three concrete implementations in `components/shared/paint/rendering_context.rs` at revision `859bd5e`. No native compilation — the spike documents API surface, threading constraints, and failure modes.

## API Findings

### 1. RenderingContext trait

```rust
pub trait RenderingContext {
    fn prepare_for_rendering(&self) {}
    fn read_to_image(&self, source_rectangle: DeviceIntRect) -> Option<RgbaImage>;
    fn size(&self) -> PhysicalSize<u32>;
    fn size2d(&self) -> Size2D<u32, DevicePixel>;
    fn resize(&self, size: PhysicalSize<u32>);
    fn present(&self);
    fn make_current(&self) -> Result<(), Error>;
    fn gleam_gl_api(&self) -> Rc<dyn Gl>;
    fn glow_gl_api(&self) -> Arc<glow::Context>;
    fn create_texture(&self, surface: Surface) -> Option<(SurfaceTexture, u32, UntypedSize2D)>;
    fn destroy_texture(&self, surface_texture: SurfaceTexture) -> Option<Surface>;
    fn connection(&self) -> Option<Connection>;
    fn refresh_driver(&self) -> Option<Rc<dyn RefreshDriver>>;
}
```

**Finding:** The trait has 13 methods, 4 with default implementations. The minimum a custom implementation must provide: `read_to_image`, `size`, `resize`, `present`, `make_current`, `gleam_gl_api`, `glow_gl_api`. The rest are optional with `None` defaults.

### 2. Three concrete implementations

| Implementation | Use case | Threading |
|---|---|---|
| `WindowRenderingContext` | Production: renders to a native window via `surfman` | `Rc<RefCell>` — same thread as Servo |
| `OffscreenRenderingContext` | Child surfaces: created from a parent `WindowRenderingContext` | `Rc` — shares parent's GL context |
| `SoftwareRenderingContext` | Headless/testing: no GPU, software rasterization | `Rc<RefCell>` — same thread |

**Finding:** All three use `Rc` or `Rc<RefCell>` — not `Send`. This confirms the thread-affinity model: the render surface lives on the engine thread, not the UI thread.

### 3. WindowRenderingContext

```rust
pub struct WindowRenderingContext { ... }

impl WindowRenderingContext {
    pub fn new(
        display: DisplayHandle,
        window: WindowHandle,
        size: PhysicalSize<u32>,
    ) -> Result<Self, Error>;

    pub fn offscreen_context(&self, size: PhysicalSize<u32>) -> OffscreenRenderingContext;
    pub fn replace_window(&self, display: DisplayHandle, window: WindowHandle) -> Result<(), Error>;
}
```

**Finding:** `WindowRenderingContext::new` requires `DisplayHandle` and `WindowHandle` from `raw-window-handle`. This is the standard Rust abstraction for native window handles — winit, Tauri's wry, and GLFW all implement it. This means the render surface can attach to a Tauri window.

### 4. OffscreenRenderingContext

```rust
pub struct OffscreenRenderingContext {
    parent_context: Rc<WindowRenderingContext>,
    // ...
}
```

**Finding:** Offscreen contexts are children of a window context. They share the parent's GL context but render to their own surface. This could support multi-tab rendering where each tab has its own offscreen surface sharing a single GPU context.

### 5. SoftwareRenderingContext

```rust
pub struct SoftwareRenderingContext { ... }

impl SoftwareRenderingContext {
    pub fn new(size: PhysicalSize<u32>) -> Result<Self, Error>;
}
```

**Finding:** `SoftwareRenderingContext` needs only a size — no window handle. It uses a CPU-based GL implementation (likely via `surfman` with software fallback). This is ideal for CI and headless testing.

### 6. paint/present flow

From `components/servo/webview.rs`:

```rust
impl WebView {
    pub fn paint(&self) { ... }
}

impl WebViewDelegate {
    fn notify_new_frame_ready(&self, webview: WebView) {
        // Embedder should:
        // 1. Call webview.paint() to render to the RenderingContext
        // 2. Call rendering_context.present() to swap buffers
    }
}
```

From `RenderingContext`:
```rust
fn present(&self);  // Swap buffers (double-buffered)
fn read_to_image(&self, rect: DeviceIntRect) -> Option<RgbaImage>;  // Screenshot
```

**Finding:** The flow is: `notify_new_frame_ready` → `WebView::paint()` → `RenderingContext::present()`. This matches the architecture's rendering pipeline (ARCHITECTURE.md §7).

### 7. resize flow

```rust
impl WebView {
    pub fn resize(&self, new_size: PhysicalSize<u32>) { ... }
}

impl RenderingContext {
    fn resize(&self, size: PhysicalSize<u32>);
}
```

**Finding:** Resize requires both `WebView::resize()` and `RenderingContext::resize()`. The embedding app is responsible for coordinating both. Coalescing (last-value-wins) must be implemented by the engine host, not by the RenderingContext.

### 8. shutdown / Drop

```rust
impl Drop for SurfmanRenderingContext {
    fn drop(&mut self) {
        let device = &mut self.device.borrow_mut();
        let context = &mut self.context.borrow_mut();
        let _ = device.destroy_context(context);
    }
}

impl Drop for SoftwareRenderingContext {
    fn drop(&mut self) { ... }
}
```

**Finding:** `Drop` impls clean up GPU resources. The shutdown barrier must ensure all `WebView` handles are dropped before the `RenderingContext` is dropped, otherwise Servo may try to paint to a destroyed context.

## Limitations

1. **`Rc<RefCell>` not `Send:** All rendering context implementations use `Rc`. They cannot cross thread boundaries. The render surface and all webviews must live on the engine thread.

2. **GPU dependency:** `WindowRenderingContext` requires a GPU and native window handles. CI environments without GPU access must use `SoftwareRenderingContext`.

3. **No async present:** `present()` is synchronous and may block on GPU flush. The engine host must not call it from the UI thread.

4. **Offscreen requires parent:** `OffscreenRenderingContext` cannot be created standalone — it needs a `WindowRenderingContext` parent. This limits headless multi-tab to `SoftwareRenderingContext` only.

5. **Minimum size 1x1:** All context types reject size `(0, 0)`. The engine host must enforce a minimum viewport size.

6. **surfman platform requirements:** `surfman` requires Linux (GBM/EGL), macOS (CGL), or Windows (EGL). Wayland support depends on the surfman version.

## Mapping to engine-api

| engine-api concept | RenderingContext API |
|---|---|
| `EngineCommand::SetViewport { width, height }` | `WebView::resize()` + `RenderingContext::resize()` |
| `EngineEvent::FrameReady` (new) | `WebViewDelegate::notify_new_frame_ready` |
| Engine thread surface handle | `Rc<RenderingContext>` on engine thread |
| Headless/CI rendering | `SoftwareRenderingContext::new(size)` |
| Production rendering | `WindowRenderingContext::new(display, window, size)` |
| Multi-tab surfaces | `WindowRenderingContext::offscreen_context(size)` |

## Failure Modes

| Failure | Cause | Mitigation |
|---|---|---|
| Context creation fails | No GPU, no display, no GL drivers | Fall back to `SoftwareRenderingContext` |
| present() after Drop | WebView not dropped before context | Engine host shutdown barrier: drop webviews first |
| Late wake | EventLoopWaker fires after shutdown | `spin_event_loop` returns `false` after shutdown |
| Pending frame at shutdown | `notify_new_frame_ready` queued but not processed | Drain event loop until `spin_event_loop` returns `false` |
| Resize race | Multiple resizes in flight | Coalesce on engine thread (last-value-wins) |
| Offscreen creation fails | Parent context lost | Re-create parent context; webviews are cheap to recreate |

## Conclusion

Servo's `RenderingContext` is **viable** for the browser's render surface. The three implementations cover production (Window), multi-tab (Offscreen), and headless/CI (Software) use cases. The `Rc` constraint confirms the engine thread model. The `raw-window-handle` integration means Tauri's wry surface can provide the handles for `WindowRenderingContext`.

The spike recommends:
1. Using `SoftwareRenderingContext` for CI and contract testing.
2. Using `WindowRenderingContext` with Tauri wry's `HasWindowHandle` for production.
3. Adding `EngineEvent::FrameReady` to the engine-api event enum.
4. Implementing a shutdown barrier that drops all webviews before the rendering context.

## Disposition

This spike is disposable. No code from the Servo source tree was added to the workspace. The findings inform the `render-surface` types added to `engine-api` in this PR and ADR-005 considerations.
