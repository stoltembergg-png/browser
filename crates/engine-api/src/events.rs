//! Engine events — typed events the engine emits to the core.
//!
//! These map to the `UiEvent` variants in `browser-domain::ui` but are
//! engine-neutral and lower-level: they carry engine instance IDs and
//! navigation generations for stale-event filtering.

use serde::{Deserialize, Serialize};

/// Opaque identifier for a navigation generation.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct NavigationGeneration(pub u32);

/// Typed events emitted by the engine to the core.
///
/// Each event carries enough context for the core to update tab state,
/// history and diagnostics without seeing DOM or page content.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum EngineEvent {
    /// Engine instance started.
    EngineStarted,
    /// Engine is ready to accept commands.
    EngineReady,
    /// Engine instance exited normally.
    EngineExited,
    /// Engine instance crashed.
    EngineCrashed { reason: String },

    /// Navigation started for a URL.
    NavigationStarted {
        url: String,
        generation: NavigationGeneration,
    },
    /// Navigation committed (first paint / URL confirmed).
    NavigationCommitted {
        url: String,
        generation: NavigationGeneration,
    },
    /// Navigation finished successfully.
    NavigationFinished {
        url: String,
        generation: NavigationGeneration,
    },
    /// Navigation failed.
    NavigationFailed {
        reason: String,
        generation: NavigationGeneration,
    },
    /// Navigation was cancelled.
    NavigationCancelled { generation: NavigationGeneration },

    /// Page title changed.
    TitleChanged { title: String },

    /// A command completed.
    CommandCompleted,
    /// A command was cancelled.
    CommandCancelled,
    /// A command timed out.
    CommandTimedOut,

    /// The command queue is saturated (backpressure signal).
    QueueSaturated,

    /// A frame is ready for presentation.
    ///
    /// The engine host should call `paint` and `present` on the rendering
    /// context. Added in PR-014 per the render surface spike findings.
    FrameReady,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn navigation_started_roundtrips() {
        let event = EngineEvent::NavigationStarted {
            url: "https://example.com".to_string(),
            generation: NavigationGeneration(1),
        };
        let json = serde_json::to_string(&event).expect("serialize");
        let back: EngineEvent = serde_json::from_str(&json).expect("deserialize");
        assert_eq!(event, back);
    }

    #[test]
    fn engine_crashed_serializes_with_reason() {
        let event = EngineEvent::EngineCrashed {
            reason: "OOM".to_string(),
        };
        let json = serde_json::to_string(&event).expect("serialize");
        assert!(json.contains("\"type\":\"engine_crashed\""));
        assert!(json.contains("OOM"));
    }

    #[test]
    fn title_changed_serializes_with_snake_case() {
        let event = EngineEvent::TitleChanged {
            title: "Test".to_string(),
        };
        let json = serde_json::to_string(&event).expect("serialize");
        assert!(json.contains("\"type\":\"title_changed\""));
    }

    #[test]
    fn queue_saturated_has_no_payload() {
        let event = EngineEvent::QueueSaturated;
        let json = serde_json::to_string(&event).expect("serialize");
        assert_eq!(json, r#"{"type":"queue_saturated"}"#);
    }

    #[test]
    fn frame_ready_serializes_with_snake_case() {
        let event = EngineEvent::FrameReady;
        let json = serde_json::to_string(&event).expect("serialize");
        assert_eq!(json, r#"{"type":"frame_ready"}"#);
    }

    #[test]
    fn navigation_generation_ordering() {
        let g1 = NavigationGeneration(1);
        let g2 = NavigationGeneration(2);
        assert!(g1 != g2);
    }
}
