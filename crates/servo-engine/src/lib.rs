#![forbid(unsafe_code)]

//! Servo engine adapter — translates the engine-api contract to Servo's
//! embedding API (revision `859bd5e`).
//!
//! ## Feature flags
//!
//! - `default`: Stub adapter. Compiles without the Servo crate. Used by CI
//!   and contract testing. The stub implements `BrowserEngine` with
//!   capability/`version`/`lifecycle` validation matching the FakeEngine.
//! - `servo-backend`: Real adapter. Enables the `servo` crate dependency.
//!   Requires system libraries (WebKitGTK, JSC, etc.) and a long build.
//!   Not enabled in CI.
//!
//! ## Pin record
//!
//! - **Servo SHA:** `859bd5edd60c0fb162a1f73c083a23e55474faf7`
//! - **Toolchain:** Rust 1.88+ (edition 2024 in Servo, 2021 in browser workspace)
//! - **Features used:** `ServoBuilder::default()` with custom `EventLoopWaker`,
//!   `WebViewBuilder` with `RenderingContext` and `WebViewDelegate`
//! - **Patches:** none
//! - **Spike evidence:** `docs/spikes/servo-embedding-859bd5e.md` (PR-013),
//!   `docs/spikes/render-surface-859bd5e.md` (PR-014)
//! - **Dependency exception:** ADR-003
//!
//! See ADR-003 for the decision and ARCHITECTURE.md §6 for the SPI design.

use engine_api::contract::{
    BrowserEngine, EngineCapabilities, EngineCommand, EngineDescriptor, EngineError,
    EngineInstanceId, EngineInstanceSpec, LifecycleState, ENGINE_API_VERSION,
};

pub const PACKAGE_NAME: &str = "servo-engine";

/// The pinned Servo revision this adapter targets.
pub const SERVO_PINNED_SHA: &str = "859bd5edd60c0fb162a1f73c083a23e55474faf7";

// ---------------------------------------------------------------------------
// Stub adapter (no Servo dependency — CI-safe)
// ---------------------------------------------------------------------------

/// A stub adapter that implements `BrowserEngine` without the Servo crate.
///
/// This exists so the workspace compiles in CI without the heavy Servo
/// dependency chain. The real adapter (behind `servo-backend` feature)
/// replaces the stub logic with Servo embedding API calls.
#[derive(Debug, Default)]
pub struct ServoAdapterStub {
    inner: StubInner,
}

#[derive(Debug, Default)]
struct StubInner {
    instances: std::cell::RefCell<std::collections::BTreeMap<String, StubInstance>>,
}

#[derive(Debug, Clone)]
#[allow(dead_code)]
struct StubInstance {
    state: LifecycleState,
    api_version: u32,
}

impl ServoAdapterStub {
    pub fn new() -> Self {
        Self::default()
    }
}

impl BrowserEngine for ServoAdapterStub {
    fn descriptor(&self) -> EngineDescriptor {
        EngineDescriptor {
            name: "servo-stub".to_string(),
            api_revision: ENGINE_API_VERSION,
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

    fn create(&self, spec: EngineInstanceSpec) -> Result<(), EngineError> {
        if spec.api_version != ENGINE_API_VERSION {
            return Err(EngineError::UnknownVersion {
                version: spec.api_version,
            });
        }
        let mut instances = self.inner.instances.borrow_mut();
        if instances.contains_key(&spec.instance_id.0) {
            return Err(EngineError::InvalidPayload {
                reason: "instance already exists".into(),
            });
        }
        instances.insert(
            spec.instance_id.0.clone(),
            StubInstance {
                state: LifecycleState::Ready,
                api_version: spec.api_version,
            },
        );
        Ok(())
    }

    fn destroy(&self, instance_id: &EngineInstanceId) -> Result<(), EngineError> {
        let mut instances = self.inner.instances.borrow_mut();
        match instances.get_mut(&instance_id.0) {
            Some(instance) => {
                if instance.state.is_terminal() {
                    return Err(EngineError::NotSupported {
                        operation: "destroy: already exited".into(),
                    });
                }
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
        let mut instances = self.inner.instances.borrow_mut();
        let instance = match instances.get_mut(&instance_id.0) {
            Some(i) => i,
            None => {
                return Err(EngineError::InvalidPayload {
                    reason: "instance not found".into(),
                })
            }
        };

        if instance.state.is_terminal() {
            return Err(EngineError::NotSupported {
                operation: "send_command: instance exited".into(),
            });
        }

        if command == EngineCommand::Shutdown {
            instances.remove(&instance_id.0);
            return Ok(());
        }

        self.capabilities().check(&command)?;

        if let EngineCommand::Navigate { .. } = &command {
            instance.state = LifecycleState::Navigating;
        } else if let EngineCommand::Stop = &command {
            if instance.state == LifecycleState::Navigating {
                instance.state = LifecycleState::Ready;
            }
        }

        Ok(())
    }

    fn state(&self, instance_id: &EngineInstanceId) -> Result<LifecycleState, EngineError> {
        let instances = self.inner.instances.borrow();
        match instances.get(&instance_id.0) {
            Some(instance) => Ok(instance.state),
            None => Err(EngineError::InvalidPayload {
                reason: "instance not found".into(),
            }),
        }
    }
}

// ---------------------------------------------------------------------------
// Type mapping documentation (compile-time only, no Servo needed)
// ---------------------------------------------------------------------------

/// Documents the mapping from engine-api commands to Servo embedding API calls.
///
/// This is a static reference table. The real adapter (behind `servo-backend`)
/// implements these calls. The stub validates the contract without Servo.
#[allow(dead_code)]
mod servo_mapping {
    // | EngineCommand      | Servo API                          |
    // |-------------------|------------------------------------|
    // | Navigate { url }  | WebView::load(Url::parse(&url))    |
    // | Reload            | WebView::reload()                  |
    // | GoBack            | WebView::go_back(1)                |
    // | GoForward         | WebView::go_forward(1)             |
    // | Stop              | WebView::set_throttled(true)       |
    // | SetViewport       | WebView::resize(PhysicalSize)      |
    // | Shutdown          | drop WebView, drain spin_event_loop |
    //
    // | EngineEvent         | Servo WebViewDelegate method         |
    // |---------------------|--------------------------------------|
    // | NavigationStarted   | notify_load_status_changed          |
    // | TitleChanged        | notify_page_title_changed            |
    // | FrameReady          | notify_new_frame_ready               |
    // | EngineCrashed       | notify_crashed                       |
    // | EngineExited        | notify_closed                        |
}

#[cfg(test)]
mod tests {
    use super::*;
    use engine_api::surface::SurfaceSpec;

    fn test_spec(id: &str, version: u32) -> EngineInstanceSpec {
        EngineInstanceSpec {
            instance_id: EngineInstanceId(id.to_string()),
            initial_url: None,
            surface: SurfaceSpec::software(800, 600),
            api_version: version,
        }
    }

    #[test]
    fn package_name_is_stable() {
        assert_eq!(PACKAGE_NAME, "servo-engine");
    }

    #[test]
    fn servo_pin_matches_spike_revision() {
        assert_eq!(SERVO_PINNED_SHA, engine_api::SERVO_SPIKE_REVISION);
    }

    #[test]
    fn stub_descriptor() {
        let adapter = ServoAdapterStub::new();
        let desc = adapter.descriptor();
        assert_eq!(desc.name, "servo-stub");
        assert_eq!(desc.api_revision, ENGINE_API_VERSION);
    }

    #[test]
    fn stub_create_and_destroy() {
        let adapter = ServoAdapterStub::new();
        let spec = test_spec("inst-1", ENGINE_API_VERSION);
        adapter.create(spec.clone()).unwrap();
        assert_eq!(
            adapter.state(&spec.instance_id).unwrap(),
            LifecycleState::Ready
        );
        adapter.destroy(&spec.instance_id).unwrap();
    }

    #[test]
    fn stub_navigate_transitions_to_navigating() {
        let adapter = ServoAdapterStub::new();
        let spec = test_spec("inst-1", ENGINE_API_VERSION);
        adapter.create(spec.clone()).unwrap();
        adapter
            .send_command(
                &spec.instance_id,
                EngineCommand::Navigate {
                    url: "https://example.com".into(),
                },
            )
            .unwrap();
        assert_eq!(
            adapter.state(&spec.instance_id).unwrap(),
            LifecycleState::Navigating
        );
    }

    #[test]
    fn stub_stop_returns_to_ready() {
        let adapter = ServoAdapterStub::new();
        let spec = test_spec("inst-1", ENGINE_API_VERSION);
        adapter.create(spec.clone()).unwrap();
        adapter
            .send_command(
                &spec.instance_id,
                EngineCommand::Navigate {
                    url: "https://example.com".into(),
                },
            )
            .unwrap();
        adapter
            .send_command(&spec.instance_id, EngineCommand::Stop)
            .unwrap();
        assert_eq!(
            adapter.state(&spec.instance_id).unwrap(),
            LifecycleState::Ready
        );
    }

    #[test]
    fn stub_rejects_unknown_version() {
        let adapter = ServoAdapterStub::new();
        let spec = test_spec("inst-1", 99);
        assert!(matches!(
            adapter.create(spec),
            Err(EngineError::UnknownVersion { .. })
        ));
    }

    #[test]
    fn stub_shutdown_removes_instance() {
        let adapter = ServoAdapterStub::new();
        let spec = test_spec("inst-1", ENGINE_API_VERSION);
        adapter.create(spec.clone()).unwrap();
        adapter
            .send_command(&spec.instance_id, EngineCommand::Shutdown)
            .unwrap();
        assert!(adapter.state(&spec.instance_id).is_err());
    }

    #[test]
    fn stub_rejects_command_after_shutdown() {
        let adapter = ServoAdapterStub::new();
        let spec = test_spec("inst-1", ENGINE_API_VERSION);
        adapter.create(spec.clone()).unwrap();
        adapter
            .send_command(&spec.instance_id, EngineCommand::Shutdown)
            .unwrap();
        let result = adapter.send_command(&spec.instance_id, EngineCommand::Reload);
        assert!(matches!(result, Err(EngineError::InvalidPayload { .. })));
    }

    #[test]
    fn stub_rejects_command_to_missing_instance() {
        let adapter = ServoAdapterStub::new();
        let result = adapter.send_command(&EngineInstanceId("nope".into()), EngineCommand::Reload);
        assert!(matches!(result, Err(EngineError::InvalidPayload { .. })));
    }

    #[test]
    fn stub_rejects_duplicate_instance() {
        let adapter = ServoAdapterStub::new();
        let spec = test_spec("inst-1", ENGINE_API_VERSION);
        adapter.create(spec.clone()).unwrap();
        assert!(matches!(
            adapter.create(spec),
            Err(EngineError::InvalidPayload { .. })
        ));
    }
}
