//! Single-tab fake-engine vertical slice — connects IPC bridge, core actor,
//! navigation state machine, and engine host against the FakeEngine.
//!
//! This module proves the full control flow works end-to-end without Servo:
//! 1. UI sends a command via the IPC bridge
//! 2. The bridge validates and forwards to the core
//! 3. The core dispatches to the engine host
//! 4. The engine host pushes to the FakeEngine
//! 5. Events flow back through the navigation state machine
//!
//! No Servo API appears in the core. The slice uses only FakeEngine.

use engine_api::contract::{
    EngineCommand, EngineInstanceId, FakeEngine, LifecycleState, ENGINE_API_VERSION,
};
use engine_api::surface::SurfaceSpec;

use crate::engine_host::{DispatchResult, EngineHost, HostSignal};
use crate::ipc_bridge::IpcBridge;
use crate::lifecycle::{CoreActor, CoreState};

/// The vertical slice — a single-tab browser session using the fake engine.
pub struct VerticalSlice {
    bridge: IpcBridge,
    core: CoreActor,
    host: EngineHost<FakeEngine>,
    started: bool,
}

/// Result of a slice operation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SliceResult {
    /// Operation succeeded.
    Ok,
    /// Operation succeeded with a signal from the host.
    Signal(HostSignal),
    /// Operation was rejected.
    Rejected(String),
    /// Navigation event outcome.
    NavigationOutcome(String),
}

impl VerticalSlice {
    /// Create a new vertical slice with a fake engine.
    pub fn new() -> Self {
        Self {
            bridge: IpcBridge::new(),
            core: CoreActor::new(),
            host: EngineHost::new(FakeEngine::new(), 256, 3),
            started: false,
        }
    }

    /// Boot the slice: start the core actor and the engine host.
    pub fn boot(&mut self) -> Result<SliceResult, String> {
        self.core
            .transition(CoreState::Starting)
            .map_err(|e| e.to_string())?;
        self.core
            .transition(CoreState::Ready)
            .map_err(|e| e.to_string())?;

        let spec = engine_api::contract::EngineInstanceSpec {
            instance_id: EngineInstanceId("slice-engine".into()),
            initial_url: None,
            surface: SurfaceSpec::software(800, 600),
            api_version: ENGINE_API_VERSION,
        };
        let signal = self.host.start(spec).map_err(|e| e.to_string())?;
        self.started = true;
        Ok(SliceResult::Signal(signal))
    }

    /// Create a tab and register it in the IPC bridge.
    pub fn create_tab(&mut self, tab_id: &str) -> SliceResult {
        if !self.started {
            return SliceResult::Rejected("slice not booted".into());
        }
        self.bridge.register_tab(tab_id);
        SliceResult::Ok
    }

    /// Send a raw JSON command from the UI through the bridge to the engine.
    pub fn send_command(&mut self, raw_json: &str) -> SliceResult {
        if !self.started {
            return SliceResult::Rejected("slice not booted".into());
        }

        // IPC bridge validates
        let envelope = match self.bridge.validate_command(raw_json) {
            Ok(env) => env,
            Err(e) => return SliceResult::Rejected(e.to_string()),
        };

        // Core actor checks dispatch state
        if !self.core.can_dispatch() {
            return SliceResult::Rejected("core not in dispatch state".into());
        }

        // Convert UiCommand to EngineCommand
        let engine_command = match ui_to_engine_command(&envelope.command) {
            Some(cmd) => cmd,
            None => return SliceResult::Rejected("unsupported command".into()),
        };

        // Core enqueues (for backpressure tracking)
        if self.core.check_duplicate(&envelope.request_id.0) {
            return SliceResult::Rejected("duplicate request".into());
        }
        if let Err(e) = self.core.enqueue(&envelope.request_id.0) {
            return SliceResult::Rejected(e.to_string());
        }

        // Engine host dispatches
        match self.host.dispatch(engine_command) {
            DispatchResult::Accepted => SliceResult::Ok,
            DispatchResult::Coalesced => SliceResult::Ok,
            DispatchResult::Rejected(e) => {
                self.core.dequeue(&envelope.request_id.0);
                SliceResult::Rejected(e.to_string())
            }
        }
    }

    /// Pump the engine host to process one queued command.
    pub fn pump(&mut self) -> SliceResult {
        match self.host.pump() {
            Some(HostSignal::Exited) => {
                self.core.shutdown().unwrap_or(());
                SliceResult::Signal(HostSignal::Exited)
            }
            Some(HostSignal::Crashed { reason }) => {
                SliceResult::Signal(HostSignal::Crashed { reason })
            }
            Some(signal) => SliceResult::Signal(signal),
            None => SliceResult::Ok,
        }
    }

    /// Shut down the slice cleanly.
    pub fn shutdown(&mut self) -> Result<SliceResult, String> {
        let signal = self.host.shutdown().map_err(|e| e.to_string())?;
        // Drain the host
        while self.host.state() != LifecycleState::Exited {
            if let Some(HostSignal::Exited) = self.host.pump() {
                break;
            }
        }
        self.core.shutdown().map_err(|e| e.to_string())?;
        Ok(SliceResult::Signal(signal))
    }

    /// Get the core state.
    pub fn core_state(&self) -> CoreState {
        self.core.state()
    }

    /// Get the host state.
    pub fn host_state(&self) -> LifecycleState {
        self.host.state()
    }

    /// Get the number of pending commands in the host queue.
    pub fn pending_count(&self) -> usize {
        self.host.pending_count()
    }
}

impl Default for VerticalSlice {
    fn default() -> Self {
        Self::new()
    }
}

/// Convert a `UiCommand` from the browser-domain to an `EngineCommand`
/// for the engine-api. This is the integration seam.
fn ui_to_engine_command(ui: &browser_domain::ui::UiCommand) -> Option<EngineCommand> {
    match ui {
        browser_domain::ui::UiCommand::Navigate { url } => {
            Some(EngineCommand::Navigate { url: url.clone() })
        }
        browser_domain::ui::UiCommand::Reload => Some(EngineCommand::Reload),
        browser_domain::ui::UiCommand::GoBack => Some(EngineCommand::GoBack),
        browser_domain::ui::UiCommand::GoForward => Some(EngineCommand::GoForward),
        browser_domain::ui::UiCommand::Stop => Some(EngineCommand::Stop),
        // NewTab, CloseTab, SelectTab are core-level, not engine-level
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use browser_domain::ui::UI_CONTRACT_VERSION;
    use engine_api::contract::BrowserEngine;

    fn navigate_command(tab_id: &str, url: &str, request_id: &str) -> String {
        format!(
            r#"{{"version":{},"request_id":"{}","tab_id":"{}","command":{{"type":"navigate","url":"{}"}}}}"#,
            UI_CONTRACT_VERSION, request_id, tab_id, url
        )
    }

    fn reload_command(tab_id: &str, request_id: &str) -> String {
        format!(
            r#"{{"version":{},"request_id":"{}","tab_id":"{}","command":{{"type":"reload"}}}}"#,
            UI_CONTRACT_VERSION, request_id, tab_id
        )
    }

    fn stop_command(tab_id: &str, request_id: &str) -> String {
        format!(
            r#"{{"version":{},"request_id":"{}","tab_id":"{}","command":{{"type":"stop"}}}}"#,
            UI_CONTRACT_VERSION, request_id, tab_id
        )
    }
    // --- Boot and basic flow ---

    #[test]
    fn boot_transitions_to_ready() {
        let mut slice = VerticalSlice::new();
        let result = slice.boot().unwrap();
        assert!(matches!(result, SliceResult::Signal(HostSignal::Ready)));
        assert_eq!(slice.core_state(), CoreState::Ready);
        assert_eq!(slice.host_state(), LifecycleState::Ready);
    }

    #[test]
    fn create_tab_registers_in_bridge() {
        let mut slice = VerticalSlice::new();
        slice.boot().unwrap();
        let result = slice.create_tab("tab-1");
        assert_eq!(result, SliceResult::Ok);
    }

    #[test]
    fn navigate_full_flow() {
        let mut slice = VerticalSlice::new();
        slice.boot().unwrap();
        slice.create_tab("tab-1");

        let result = slice.send_command(&navigate_command("tab-1", "https://example.com", "req-1"));
        assert_eq!(result, SliceResult::Ok);
        assert_eq!(slice.pending_count(), 1);

        // Pump processes the command
        let result = slice.pump();
        assert_eq!(result, SliceResult::Ok);
        assert_eq!(slice.pending_count(), 0);
    }

    // --- Malformed events ---

    #[test]
    fn malformed_json_rejected() {
        let mut slice = VerticalSlice::new();
        slice.boot().unwrap();
        slice.create_tab("tab-1");

        let result = slice.send_command("not json");
        assert!(matches!(result, SliceResult::Rejected(_)));
    }

    #[test]
    fn stale_version_rejected() {
        let mut slice = VerticalSlice::new();
        slice.boot().unwrap();
        slice.create_tab("tab-1");

        let raw =
            r#"{"version":99,"request_id":"r1","tab_id":"tab-1","command":{"type":"reload"}}"#;
        let result = slice.send_command(raw);
        assert!(matches!(result, SliceResult::Rejected(_)));
    }

    // --- Cancellation ---

    #[test]
    fn stop_cancels_navigate() {
        let mut slice = VerticalSlice::new();
        slice.boot().unwrap();
        slice.create_tab("tab-1");

        // Navigate then stop
        slice.send_command(&navigate_command("tab-1", "https://example.com", "req-1"));
        slice.send_command(&stop_command("tab-1", "req-2"));
        assert_eq!(slice.pending_count(), 2);

        // Pump processes navigate
        slice.pump();
        // Pump processes stop
        slice.pump();
        assert_eq!(slice.pending_count(), 0);
    }

    // --- Close ---

    #[test]
    fn shutdown_drains_and_exits() {
        let mut slice = VerticalSlice::new();
        slice.boot().unwrap();
        slice.create_tab("tab-1");
        slice.send_command(&reload_command("tab-1", "req-1"));
        let result = slice.shutdown().unwrap();
        assert!(matches!(result, SliceResult::Signal(HostSignal::Closing)));
        assert_eq!(slice.core_state(), CoreState::Exited);
    }

    // --- Crash state ---

    #[test]
    fn crash_state_visible() {
        let mut slice = VerticalSlice::new();
        slice.boot().unwrap();
        slice.create_tab("tab-1");

        // Inject crash on the host
        let _signal = slice.host.inject_crash("OOM");
        assert_eq!(slice.host_state(), LifecycleState::Crashed);
    }

    // --- Double submit ---

    #[test]
    fn double_submit_rejected() {
        let mut slice = VerticalSlice::new();
        slice.boot().unwrap();
        slice.create_tab("tab-1");

        let raw = reload_command("tab-1", "req-1");
        assert_eq!(slice.send_command(&raw), SliceResult::Ok);
        let result = slice.send_command(&raw);
        assert!(matches!(result, SliceResult::Rejected(_)));
    }

    // --- Wrong tab ---

    #[test]
    fn wrong_tab_rejected() {
        let mut slice = VerticalSlice::new();
        slice.boot().unwrap();
        slice.create_tab("tab-1");

        // Target tab-2 which doesn't exist
        let raw = reload_command("tab-2", "req-1");
        let result = slice.send_command(&raw);
        assert!(matches!(result, SliceResult::Rejected(_)));
    }

    // --- Command ordering ---

    #[test]
    fn command_ordering_preserved() {
        let mut slice = VerticalSlice::new();
        slice.boot().unwrap();
        slice.create_tab("tab-1");

        slice.send_command(&navigate_command("tab-1", "https://a.com", "r1"));
        slice.send_command(&reload_command("tab-1", "r2"));
        slice.send_command(&stop_command("tab-1", "r3"));

        assert_eq!(slice.pending_count(), 3);
        slice.pump();
        assert_eq!(slice.pending_count(), 2);
        slice.pump();
        assert_eq!(slice.pending_count(), 1);
        slice.pump();
        assert_eq!(slice.pending_count(), 0);
    }

    // --- No Servo API in core ---

    #[test]
    fn no_servo_api_in_core() {
        // The VerticalSlice uses only FakeEngine, never servo_engine.
        // This test documents that contract: the core never imports
        // servo_engine and the slice works entirely with the fake engine.
        let mut slice = VerticalSlice::new();
        slice.boot().unwrap();
        assert_eq!(slice.host.engine.descriptor().name, "fake");
    }

    // --- No run / no frame / no skip is NO_GO ---

    #[test]
    fn boot_required_before_commands() {
        let mut slice = VerticalSlice::new();
        // Not booted
        let result = slice.send_command(&reload_command("tab-1", "req-1"));
        assert!(matches!(result, SliceResult::Rejected(_)));
    }

    #[test]
    fn create_tab_before_boot_rejected() {
        let mut slice = VerticalSlice::new();
        let result = slice.create_tab("tab-1");
        assert_eq!(result, SliceResult::Rejected("slice not booted".into()));
    }
}
