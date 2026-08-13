//! Engine contract types — the SPI between browser-core and the engine.
//!
//! These types are engine-neutral. The first adapter (`servo-engine`, PR-016)
//! implements `BrowserEngine` using Servo's embedding API. A `FakeEngine`
//! (PR-015) implements it for contract testing.
//!
//! All types are `Serialize`/`Deserialize` so they can cross process
//! boundaries in the future (ARCHITECTURE.md §8 — process model evolution).

use serde::{Deserialize, Serialize};

// ---------------------------------------------------------------------------
// Descriptors and capabilities
// ---------------------------------------------------------------------------

/// Describes an engine instance for logging and diagnostics.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct EngineDescriptor {
    /// Engine name (e.g. "servo", "fake").
    pub name: String,
    /// API revision the adapter supports.
    pub api_revision: u32,
}

/// Declares which optional operations an engine supports.
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
pub struct EngineCapabilities {
    pub can_navigate: bool,
    pub can_reload: bool,
    pub can_go_back: bool,
    pub can_go_forward: bool,
    pub can_resize: bool,
    pub can_receive_input: bool,
}

// ---------------------------------------------------------------------------
// Instance spec and IDs
// ---------------------------------------------------------------------------

/// Opaque identifier for a running engine instance.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct EngineInstanceId(pub String);

/// Specification for creating a new engine instance.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct EngineInstanceSpec {
    pub instance_id: EngineInstanceId,
    /// Initial URL to load, if any.
    pub initial_url: Option<String>,
}

// ---------------------------------------------------------------------------
// Engine commands (core → engine)
// ---------------------------------------------------------------------------

/// Typed commands the core sends to the engine.
///
/// Each command corresponds to a `UiCommand` from the browser-domain but
/// is engine-neutral — no DOM or page content leaks.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum EngineCommand {
    /// Navigate to a URL.
    Navigate { url: String },
    /// Reload the current page.
    Reload,
    /// Navigate back in history.
    GoBack,
    /// Navigate forward in history.
    GoForward,
    /// Stop loading.
    Stop,
    /// Resize the webview.
    SetViewport { width: u32, height: u32 },
    /// Shutdown the engine instance.
    Shutdown,
}

// ---------------------------------------------------------------------------
// Engine trait (SPI)
// ---------------------------------------------------------------------------

/// The engine SPI that adapters implement.
///
/// This is a **conceptual** trait for planning — the actual adapter (PR-016)
/// will implement a concrete version with channels and async boundaries.
/// This trait is NOT `Send + Sync` by design: the engine owns its thread.
pub trait BrowserEngine {
    /// Returns the engine descriptor for diagnostics.
    fn descriptor(&self) -> EngineDescriptor;

    /// Returns the capabilities this engine supports.
    fn capabilities(&self) -> EngineCapabilities;

    /// Create a new engine instance with the given spec.
    ///
    /// Returns an error string if creation fails.
    fn create(&self, spec: EngineInstanceSpec) -> Result<(), String>;

    /// Destroy a running engine instance.
    fn destroy(&self, instance_id: &EngineInstanceId) -> Result<(), String>;

    /// Send a command to a running engine instance.
    fn send_command(
        &self,
        instance_id: &EngineInstanceId,
        command: EngineCommand,
    ) -> Result<(), String>;
}

// ---------------------------------------------------------------------------
// Fake engine for contract testing (PR-015 will expand this)
// ---------------------------------------------------------------------------

/// A minimal in-memory engine that satisfies the SPI for testing.
///
/// PR-015 will expand this with event injection and contract assertions.
/// This stub exists so the types compile and the architecture is testable.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct FakeEngine {
    instances: std::collections::BTreeSet<String>,
}

impl FakeEngine {
    pub fn new() -> Self {
        Self::default()
    }
}

impl BrowserEngine for FakeEngine {
    fn descriptor(&self) -> EngineDescriptor {
        EngineDescriptor {
            name: "fake".to_string(),
            api_revision: 1,
        }
    }

    fn capabilities(&self) -> EngineCapabilities {
        EngineCapabilities {
            can_navigate: true,
            can_reload: true,
            can_go_back: true,
            can_go_forward: true,
            can_resize: true,
            can_receive_input: false,
        }
    }

    fn create(&self, _spec: EngineInstanceSpec) -> Result<(), String> {
        // PR-015 will implement real instance tracking.
        Ok(())
    }

    fn destroy(&self, _instance_id: &EngineInstanceId) -> Result<(), String> {
        Ok(())
    }

    fn send_command(
        &self,
        _instance_id: &EngineInstanceId,
        _command: EngineCommand,
    ) -> Result<(), String> {
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fake_engine_descriptor() {
        let engine = FakeEngine::new();
        let desc = engine.descriptor();
        assert_eq!(desc.name, "fake");
        assert_eq!(desc.api_revision, 1);
    }

    #[test]
    fn fake_engine_capabilities() {
        let engine = FakeEngine::new();
        let caps = engine.capabilities();
        assert!(caps.can_navigate);
        assert!(caps.can_reload);
        assert!(!caps.can_receive_input);
    }

    #[test]
    fn fake_engine_create_and_destroy() {
        let engine = FakeEngine::new();
        let spec = EngineInstanceSpec {
            instance_id: EngineInstanceId("instance-1".to_string()),
            initial_url: Some("https://example.com".to_string()),
        };
        engine.create(spec.clone()).expect("create should succeed");
        engine
            .destroy(&spec.instance_id)
            .expect("destroy should succeed");
    }

    #[test]
    fn fake_engine_send_navigate_command() {
        let engine = FakeEngine::new();
        let instance = EngineInstanceId("instance-1".to_string());
        engine
            .send_command(
                &instance,
                EngineCommand::Navigate {
                    url: "https://example.com".to_string(),
                },
            )
            .expect("navigate should succeed");
    }

    #[test]
    fn engine_command_roundtrips_through_serde() {
        let cmd = EngineCommand::SetViewport {
            width: 1024,
            height: 768,
        };
        let json = serde_json::to_string(&cmd).expect("serialize");
        let back: EngineCommand = serde_json::from_str(&json).expect("deserialize");
        assert_eq!(cmd, back);
    }

    #[test]
    fn engine_descriptor_roundtrips_through_serde() {
        let desc = EngineDescriptor {
            name: "servo".to_string(),
            api_revision: 1,
        };
        let json = serde_json::to_string(&desc).expect("serialize");
        let back: EngineDescriptor = serde_json::from_str(&json).expect("deserialize");
        assert_eq!(desc, back);
    }

    #[test]
    fn engine_capabilities_roundtrips_through_serde() {
        let caps = EngineCapabilities {
            can_navigate: true,
            can_reload: true,
            can_go_back: false,
            can_go_forward: false,
            can_resize: true,
            can_receive_input: false,
        };
        let json = serde_json::to_string(&caps).expect("serialize");
        let back: EngineCapabilities = serde_json::from_str(&json).expect("deserialize");
        assert_eq!(caps, back);
    }

    #[test]
    fn shutdown_command_serializes_with_snake_case() {
        let cmd = EngineCommand::Shutdown;
        let json = serde_json::to_string(&cmd).expect("serialize");
        assert!(json.contains("\"type\":\"shutdown\""));
    }
}
