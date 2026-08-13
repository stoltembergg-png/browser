//! Engine contract types — the SPI between browser-core and the engine.
//!
//! These types are engine-neutral. The first adapter (`servo-engine`, PR-016)
//! implements `BrowserEngine` using Servo's embedding API. A `FakeEngine`
//! (this PR-015) implements it for contract testing.
//!
//! All types are `Serialize`/`Deserialize` so they can cross process
//! boundaries in the future (ARCHITECTURE.md §8 — process model evolution).

use std::cell::RefCell;
use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

use crate::surface::SurfaceSpec;

// ---------------------------------------------------------------------------
// API version
// ---------------------------------------------------------------------------

/// The engine API contract version.
///
/// Both sides of the contract must agree on this version. Unknown versions
/// are rejected. Breaking changes require a version bump and a superseding ADR.
pub const ENGINE_API_VERSION: u32 = 1;

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

impl EngineCapabilities {
    /// Check if a command is supported by these capabilities.
    ///
    /// Returns `Err(EngineError::NotSupported)` if the command is outside
    /// the supported subset.
    pub fn check(&self, command: &EngineCommand) -> Result<(), EngineError> {
        match command {
            EngineCommand::Navigate { .. } if !self.can_navigate => {
                Err(EngineError::NotSupported {
                    operation: "navigate".into(),
                })
            }
            EngineCommand::Reload if !self.can_reload => Err(EngineError::NotSupported {
                operation: "reload".into(),
            }),
            EngineCommand::GoBack if !self.can_go_back => Err(EngineError::NotSupported {
                operation: "go_back".into(),
            }),
            EngineCommand::GoForward if !self.can_go_forward => Err(EngineError::NotSupported {
                operation: "go_forward".into(),
            }),
            EngineCommand::SetViewport { .. } if !self.can_resize => {
                Err(EngineError::NotSupported {
                    operation: "set_viewport".into(),
                })
            }
            _ => Ok(()),
        }
    }
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
    /// Render surface specification.
    pub surface: SurfaceSpec,
    /// API version the core expects.
    pub api_version: u32,
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
// Engine errors (typed)
// ---------------------------------------------------------------------------

/// Typed errors returned by the engine.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum EngineError {
    /// The command is not known to this engine version.
    UnknownCommand { command_type: String },
    /// The API version is not supported.
    UnknownVersion { version: u32 },
    /// The command payload is invalid.
    InvalidPayload { reason: String },
    /// The engine does not support this operation (capability not declared).
    NotSupported { operation: String },
    /// The engine instance crashed.
    EngineCrashed { reason: String },
    /// The command timed out.
    Timeout,
    /// The command was cancelled.
    Cancelled,
}

impl EngineError {
    /// Returns `true` if this error is terminal — no further commands accepted.
    pub fn is_terminal(&self) -> bool {
        matches!(self, EngineError::EngineCrashed { .. })
    }
}

impl std::fmt::Display for EngineError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            EngineError::UnknownCommand { command_type } => {
                write!(f, "unknown command: {command_type}")
            }
            EngineError::UnknownVersion { version } => {
                write!(f, "unknown API version: {version}")
            }
            EngineError::InvalidPayload { reason } => {
                write!(f, "invalid payload: {reason}")
            }
            EngineError::NotSupported { operation } => {
                write!(f, "not supported: {operation}")
            }
            EngineError::EngineCrashed { reason } => {
                write!(f, "engine crashed: {reason}")
            }
            EngineError::Timeout => write!(f, "command timed out"),
            EngineError::Cancelled => write!(f, "command cancelled"),
        }
    }
}

impl std::error::Error for EngineError {}

// ---------------------------------------------------------------------------
// Lifecycle state
// ---------------------------------------------------------------------------

/// The lifecycle state of an engine instance.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum LifecycleState {
    Created,
    Starting,
    Ready,
    Navigating,
    Closing,
    Exited,
    Failed,
    Crashed,
    Restarting,
}

impl LifecycleState {
    /// Returns `true` if commands can be sent in this state.
    pub fn accepts_commands(&self) -> bool {
        matches!(self, LifecycleState::Ready | LifecycleState::Navigating)
    }

    /// Returns `true` if this is a terminal state.
    pub fn is_terminal(&self) -> bool {
        matches!(self, LifecycleState::Exited)
    }
}

// ---------------------------------------------------------------------------
// Engine trait (SPI)
// ---------------------------------------------------------------------------

/// The engine SPI that adapters implement.
///
/// This trait is NOT `Send + Sync` by design: the engine owns its thread.
pub trait BrowserEngine {
    /// Returns the engine descriptor for diagnostics.
    fn descriptor(&self) -> EngineDescriptor;

    /// Returns the capabilities this engine supports.
    fn capabilities(&self) -> EngineCapabilities;

    /// Create a new engine instance with the given spec.
    fn create(&self, spec: EngineInstanceSpec) -> Result<(), EngineError>;

    /// Destroy a running engine instance.
    fn destroy(&self, instance_id: &EngineInstanceId) -> Result<(), EngineError>;

    /// Send a command to a running engine instance.
    fn send_command(
        &self,
        instance_id: &EngineInstanceId,
        command: EngineCommand,
    ) -> Result<(), EngineError>;

    /// Returns the current lifecycle state of an instance.
    fn state(&self, instance_id: &EngineInstanceId) -> Result<LifecycleState, EngineError>;
}

// ---------------------------------------------------------------------------
// Fake engine for contract testing
// ---------------------------------------------------------------------------

/// An in-memory engine that satisfies the SPI for contract testing.
///
/// The fake engine tracks instances in `RefCell<BTreeMap>` and enforces:
/// - API version check on creation
/// - Capability negotiation on commands
/// - Lifecycle state transitions (fail closed)
/// - Rejection of commands outside the supported subset
/// - Terminal state after `Shutdown` or `EngineCrashed`
#[derive(Debug, Default)]
pub struct FakeEngine {
    instances: RefCell<BTreeMap<String, FakeInstance>>,
    capabilities: EngineCapabilities,
}

#[derive(Debug, Clone)]
#[allow(dead_code)]
struct FakeInstance {
    state: LifecycleState,
    api_version: u32,
}

impl FakeEngine {
    pub fn new() -> Self {
        Self {
            instances: RefCell::new(BTreeMap::new()),
            capabilities: EngineCapabilities {
                can_navigate: true,
                can_reload: true,
                can_go_back: true,
                can_go_forward: true,
                can_resize: true,
                can_receive_input: false,
            },
        }
    }

    /// Create a fake engine with reduced capabilities for testing rejection.
    pub fn with_capabilities(caps: EngineCapabilities) -> Self {
        Self {
            instances: RefCell::new(BTreeMap::new()),
            capabilities: caps,
        }
    }

    /// Inject a crash for a specific instance (for testing terminal states).
    pub fn inject_crash(&self, instance_id: &EngineInstanceId, reason: &str) {
        if let Some(instance) = self.instances.borrow_mut().get_mut(&instance_id.0) {
            instance.state = LifecycleState::Crashed;
            let _ = reason; // In a real engine, this would be the crash reason
        }
    }
}

impl BrowserEngine for FakeEngine {
    fn descriptor(&self) -> EngineDescriptor {
        EngineDescriptor {
            name: "fake".to_string(),
            api_revision: ENGINE_API_VERSION,
        }
    }

    fn capabilities(&self) -> EngineCapabilities {
        self.capabilities.clone()
    }

    fn create(&self, spec: EngineInstanceSpec) -> Result<(), EngineError> {
        if spec.api_version != ENGINE_API_VERSION {
            return Err(EngineError::UnknownVersion {
                version: spec.api_version,
            });
        }
        let mut instances = self.instances.borrow_mut();
        if instances.contains_key(&spec.instance_id.0) {
            return Err(EngineError::InvalidPayload {
                reason: "instance already exists".into(),
            });
        }
        instances.insert(
            spec.instance_id.0.clone(),
            FakeInstance {
                state: LifecycleState::Ready,
                api_version: spec.api_version,
            },
        );
        Ok(())
    }

    fn destroy(&self, instance_id: &EngineInstanceId) -> Result<(), EngineError> {
        let mut instances = self.instances.borrow_mut();
        match instances.get_mut(&instance_id.0) {
            Some(instance) => {
                if instance.state.is_terminal() {
                    return Err(EngineError::NotSupported {
                        operation: "destroy: already exited".into(),
                    });
                }
                instance.state = LifecycleState::Exited;
                instances.remove(&instance_id.0);
                Ok(())
            }
            None => Err(EngineError::InvalidPayload {
                reason: "instance not found".into(),
            }),
        }
    }

    fn send_command(
        &self,
        instance_id: &EngineInstanceId,
        command: EngineCommand,
    ) -> Result<(), EngineError> {
        let mut instances = self.instances.borrow_mut();
        let instance = match instances.get_mut(&instance_id.0) {
            Some(i) => i,
            None => {
                return Err(EngineError::InvalidPayload {
                    reason: "instance not found".into(),
                })
            }
        };

        // Reject commands to terminated instances.
        if instance.state.is_terminal() {
            return Err(EngineError::NotSupported {
                operation: "send_command: instance exited".into(),
            });
        }

        // Reject if crashed.
        if instance.state == LifecycleState::Crashed {
            return Err(EngineError::EngineCrashed {
                reason: "instance is crashed".into(),
            });
        }

        // Shutdown transitions to Closing then Exited.
        if command == EngineCommand::Shutdown {
            instance.state = LifecycleState::Exited;
            instances.remove(&instance_id.0);
            return Ok(());
        }

        // Capability negotiation: reject unsupported commands.
        self.capabilities.check(&command)?;

        // State transition for Navigate.
        if let EngineCommand::Navigate { .. } = &command {
            instance.state = LifecycleState::Navigating;
        } else if let EngineCommand::Stop = &command {
            // Stop returns from Navigating to Ready.
            if instance.state == LifecycleState::Navigating {
                instance.state = LifecycleState::Ready;
            }
        }

        Ok(())
    }

    fn state(&self, instance_id: &EngineInstanceId) -> Result<LifecycleState, EngineError> {
        let instances = self.instances.borrow();
        match instances.get(&instance_id.0) {
            Some(instance) => Ok(instance.state),
            None => Err(EngineError::InvalidPayload {
                reason: "instance not found".into(),
            }),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::surface::SurfaceSpec;

    fn test_spec(id: &str, version: u32) -> EngineInstanceSpec {
        EngineInstanceSpec {
            instance_id: EngineInstanceId(id.to_string()),
            initial_url: None,
            surface: SurfaceSpec::software(800, 600),
            api_version: version,
        }
    }

    // --- Basic lifecycle ---

    #[test]
    fn fake_engine_create_and_destroy() {
        let engine = FakeEngine::new();
        let spec = test_spec("inst-1", ENGINE_API_VERSION);
        engine.create(spec.clone()).expect("create");
        assert_eq!(
            engine.state(&spec.instance_id).unwrap(),
            LifecycleState::Ready
        );
        engine.destroy(&spec.instance_id).expect("destroy");
    }

    #[test]
    fn fake_engine_send_navigate() {
        let engine = FakeEngine::new();
        let spec = test_spec("inst-1", ENGINE_API_VERSION);
        engine.create(spec.clone()).unwrap();
        engine
            .send_command(
                &spec.instance_id,
                EngineCommand::Navigate {
                    url: "https://example.com".into(),
                },
            )
            .unwrap();
        assert_eq!(
            engine.state(&spec.instance_id).unwrap(),
            LifecycleState::Navigating
        );
    }

    #[test]
    fn fake_engine_stop_returns_to_ready() {
        let engine = FakeEngine::new();
        let spec = test_spec("inst-1", ENGINE_API_VERSION);
        engine.create(spec.clone()).unwrap();
        engine
            .send_command(
                &spec.instance_id,
                EngineCommand::Navigate {
                    url: "https://example.com".into(),
                },
            )
            .unwrap();
        engine
            .send_command(&spec.instance_id, EngineCommand::Stop)
            .unwrap();
        assert_eq!(
            engine.state(&spec.instance_id).unwrap(),
            LifecycleState::Ready
        );
    }

    // --- Unknown version rejection ---

    #[test]
    fn fake_engine_rejects_unknown_version() {
        let engine = FakeEngine::new();
        let spec = test_spec("inst-1", 99);
        let result = engine.create(spec);
        assert!(matches!(result, Err(EngineError::UnknownVersion { .. })));
    }

    // --- Capability negotiation ---

    #[test]
    fn fake_engine_rejects_unsupported_navigate() {
        let caps = EngineCapabilities {
            can_navigate: false,
            ..Default::default()
        };
        let engine = FakeEngine::with_capabilities(caps);
        let spec = test_spec("inst-1", ENGINE_API_VERSION);
        engine.create(spec.clone()).unwrap();
        let result = engine.send_command(
            &spec.instance_id,
            EngineCommand::Navigate {
                url: "https://example.com".into(),
            },
        );
        assert!(matches!(result, Err(EngineError::NotSupported { .. })));
    }

    #[test]
    fn fake_engine_rejects_unsupported_resize() {
        let caps = EngineCapabilities {
            can_resize: false,
            ..Default::default()
        };
        let engine = FakeEngine::with_capabilities(caps);
        let spec = test_spec("inst-1", ENGINE_API_VERSION);
        engine.create(spec.clone()).unwrap();
        let result = engine.send_command(
            &spec.instance_id,
            EngineCommand::SetViewport {
                width: 1024,
                height: 768,
            },
        );
        assert!(matches!(result, Err(EngineError::NotSupported { .. })));
    }

    // --- Terminal state ---

    #[test]
    fn fake_engine_shutdown_exits() {
        let engine = FakeEngine::new();
        let spec = test_spec("inst-1", ENGINE_API_VERSION);
        engine.create(spec.clone()).unwrap();
        engine
            .send_command(&spec.instance_id, EngineCommand::Shutdown)
            .unwrap();
        // Instance is removed after shutdown.
        let result = engine.state(&spec.instance_id);
        assert!(matches!(result, Err(EngineError::InvalidPayload { .. })));
    }

    #[test]
    fn fake_engine_rejects_command_after_shutdown() {
        let engine = FakeEngine::new();
        let spec = test_spec("inst-1", ENGINE_API_VERSION);
        engine.create(spec.clone()).unwrap();
        engine
            .send_command(&spec.instance_id, EngineCommand::Shutdown)
            .unwrap();
        let result = engine.send_command(
            &spec.instance_id,
            EngineCommand::Navigate {
                url: "https://example.com".into(),
            },
        );
        assert!(matches!(result, Err(EngineError::InvalidPayload { .. })));
    }

    // --- Crash injection ---

    #[test]
    fn fake_engine_crash_blocks_commands() {
        let engine = FakeEngine::new();
        let spec = test_spec("inst-1", ENGINE_API_VERSION);
        engine.create(spec.clone()).unwrap();
        engine.inject_crash(&spec.instance_id, "OOM");
        let result = engine.send_command(&spec.instance_id, EngineCommand::Reload);
        assert!(matches!(result, Err(EngineError::EngineCrashed { .. })));
    }

    // --- Duplicate instance ---

    #[test]
    fn fake_engine_rejects_duplicate_instance() {
        let engine = FakeEngine::new();
        let spec = test_spec("inst-1", ENGINE_API_VERSION);
        engine.create(spec.clone()).unwrap();
        let result = engine.create(spec);
        assert!(matches!(result, Err(EngineError::InvalidPayload { .. })));
    }

    // --- Missing instance ---

    #[test]
    fn fake_engine_rejects_command_to_missing_instance() {
        let engine = FakeEngine::new();
        let result = engine.send_command(&EngineInstanceId("nope".into()), EngineCommand::Reload);
        assert!(matches!(result, Err(EngineError::InvalidPayload { .. })));
    }

    // --- Serde roundtrips ---

    #[test]
    fn engine_error_roundtrips() {
        let err = EngineError::UnknownVersion { version: 99 };
        let json = serde_json::to_string(&err).expect("serialize");
        let back: EngineError = serde_json::from_str(&json).expect("deserialize");
        assert_eq!(err, back);
    }

    #[test]
    fn lifecycle_state_accepts_commands() {
        assert!(LifecycleState::Ready.accepts_commands());
        assert!(LifecycleState::Navigating.accepts_commands());
        assert!(!LifecycleState::Starting.accepts_commands());
        assert!(!LifecycleState::Exited.accepts_commands());
        assert!(!LifecycleState::Crashed.accepts_commands());
    }

    #[test]
    fn engine_error_is_terminal() {
        assert!(EngineError::EngineCrashed { reason: "x".into() }.is_terminal());
        assert!(!EngineError::Timeout.is_terminal());
        assert!(!EngineError::Cancelled.is_terminal());
    }

    #[test]
    fn capabilities_check_allows_supported() {
        let caps = EngineCapabilities {
            can_navigate: true,
            can_reload: true,
            can_go_back: true,
            can_go_forward: true,
            can_resize: true,
            can_receive_input: false,
        };
        assert!(caps
            .check(&EngineCommand::Navigate { url: "x".into() })
            .is_ok());
        assert!(caps.check(&EngineCommand::Reload).is_ok());
        assert!(caps.check(&EngineCommand::GoBack).is_ok());
        assert!(caps.check(&EngineCommand::GoForward).is_ok());
        assert!(caps
            .check(&EngineCommand::SetViewport {
                width: 1,
                height: 1
            })
            .is_ok());
        // Stop and Shutdown are always allowed.
        assert!(caps.check(&EngineCommand::Stop).is_ok());
        assert!(caps.check(&EngineCommand::Shutdown).is_ok());
    }
}
