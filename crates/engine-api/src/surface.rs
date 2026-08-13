//! Render surface types — engine-neutral abstractions for frame presentation.
//!
//! These types model the render surface without depending on Servo's
//! `RenderingContext` trait. The adapter (PR-016) maps these to Servo's
//! concrete implementations (Window/Offscreen/Software).
//!
//! See ARCHITECTURE.md §7 and the spike report for details.

use serde::{Deserialize, Serialize};

// ---------------------------------------------------------------------------
// Viewport and surface specification
// ---------------------------------------------------------------------------

/// A viewport size in physical pixels.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct Viewport {
    pub width: u32,
    pub height: u32,
}

/// Minimum viewport dimension supported by all rendering backends.
pub const MIN_VIEWPORT_SIZE: u32 = 1;

impl Viewport {
    pub fn new(width: u32, height: u32) -> Self {
        Self { width, height }
    }

    /// Returns `true` if both dimensions meet the minimum size.
    pub fn is_valid(&self) -> bool {
        self.width >= MIN_VIEWPORT_SIZE && self.height >= MIN_VIEWPORT_SIZE
    }
}

// ---------------------------------------------------------------------------
// Surface type enum
// ---------------------------------------------------------------------------

/// The kind of rendering surface an engine instance uses.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SurfaceType {
    /// Renders to a native window via GPU.
    Window,
    /// Renders to an offscreen surface sharing a parent's GPU context.
    Offscreen,
    /// Software rasterization — no GPU required.
    Software,
}

// ---------------------------------------------------------------------------
// Surface descriptor
// ---------------------------------------------------------------------------

/// Describes a render surface for engine instance creation.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SurfaceSpec {
    pub surface_type: SurfaceType,
    pub viewport: Viewport,
}

impl SurfaceSpec {
    /// Create a software surface spec for headless/CI use.
    pub fn software(width: u32, height: u32) -> Self {
        Self {
            surface_type: SurfaceType::Software,
            viewport: Viewport::new(width, height),
        }
    }

    /// Create a window surface spec for production use.
    pub fn window(width: u32, height: u32) -> Self {
        Self {
            surface_type: SurfaceType::Window,
            viewport: Viewport::new(width, height),
        }
    }
}

// ---------------------------------------------------------------------------
// Frame state
// ---------------------------------------------------------------------------

/// The state of a frame in the render pipeline.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FrameState {
    /// No frame is pending.
    Idle,
    /// A frame is being rendered by the engine.
    Pending,
    /// A frame is ready for presentation.
    Ready,
    /// Frame rendering failed.
    Failed,
}

// ---------------------------------------------------------------------------
// Shutdown barrier
// ---------------------------------------------------------------------------

/// Coordinates the shutdown of webviews before the rendering context.
///
/// The engine host must:
/// 1. Drop all webview handles.
/// 2. Drain `spin_event_loop` until it returns `false`.
/// 3. Only then drop the rendering context.
///
/// This type documents the contract; the actual barrier is implemented
/// in the engine host (PR-020+).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ShutdownBarrier {
    /// Number of webviews that must be dropped before the context.
    pub pending_webviews: usize,
    /// Whether the event loop has been drained.
    pub event_loop_drained: bool,
}

impl ShutdownBarrier {
    pub fn new(webview_count: usize) -> Self {
        Self {
            pending_webviews: webview_count,
            event_loop_drained: false,
        }
    }

    /// Record that a webview was dropped.
    pub fn webview_dropped(&mut self) {
        if self.pending_webviews > 0 {
            self.pending_webviews -= 1;
        }
    }

    /// Record that the event loop was drained.
    pub fn mark_drained(&mut self) {
        self.event_loop_drained = true;
    }

    /// Returns `true` if it is safe to drop the rendering context.
    pub fn can_destroy_context(&self) -> bool {
        self.pending_webviews == 0 && self.event_loop_drained
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn viewport_validity() {
        assert!(Viewport::new(1, 1).is_valid());
        assert!(Viewport::new(1920, 1080).is_valid());
        assert!(!Viewport::new(0, 100).is_valid());
        assert!(!Viewport::new(100, 0).is_valid());
        assert!(!Viewport::new(0, 0).is_valid());
    }

    #[test]
    fn surface_type_serializes_as_snake_case() {
        let json = serde_json::to_string(&SurfaceType::Software).expect("serialize");
        assert_eq!(json, "\"software\"");
        let json = serde_json::to_string(&SurfaceType::Window).expect("serialize");
        assert_eq!(json, "\"window\"");
    }

    #[test]
    fn surface_spec_constructors() {
        let sw = SurfaceSpec::software(800, 600);
        assert_eq!(sw.surface_type, SurfaceType::Software);
        assert!(sw.viewport.is_valid());

        let win = SurfaceSpec::window(1920, 1080);
        assert_eq!(win.surface_type, SurfaceType::Window);
    }

    #[test]
    fn shutdown_barrier_lifecycle() {
        let mut barrier = ShutdownBarrier::new(2);
        assert!(!barrier.can_destroy_context());

        barrier.webview_dropped();
        assert!(!barrier.can_destroy_context());

        barrier.webview_dropped();
        assert!(!barrier.can_destroy_context());

        barrier.mark_drained();
        assert!(barrier.can_destroy_context());
    }

    #[test]
    fn shutdown_barrier_no_overflow() {
        let mut barrier = ShutdownBarrier::new(1);
        barrier.webview_dropped();
        barrier.webview_dropped(); // Should not underflow
        assert_eq!(barrier.pending_webviews, 0);
    }

    #[test]
    fn frame_state_serializes() {
        let json = serde_json::to_string(&FrameState::Ready).expect("serialize");
        assert_eq!(json, "\"ready\"");
    }

    #[test]
    fn viewport_roundtrips() {
        let vp = Viewport::new(1024, 768);
        let json = serde_json::to_string(&vp).expect("serialize");
        let back: Viewport = serde_json::from_str(&json).expect("deserialize");
        assert_eq!(vp, back);
    }

    #[test]
    fn surface_spec_roundtrips() {
        let spec = SurfaceSpec::software(800, 600);
        let json = serde_json::to_string(&spec).expect("serialize");
        let back: SurfaceSpec = serde_json::from_str(&json).expect("deserialize");
        assert_eq!(spec, back);
    }
}
