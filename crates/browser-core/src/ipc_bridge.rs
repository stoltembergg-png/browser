//! Typed Tauri IPC bridge — validates and dispatches UI commands to the core.
//!
//! This module sits between the Tauri frontend and the browser core.
//! It enforces:
//! - **Allowlist:** only commands in the `UiCommand` enum are accepted;
//!   no generic `invoke` fallback.
//! - **Caller context:** every command carries a `tab_id` when scoped to a
//!   tab; commands without a tab_id are app-level only.
//! - **Payload limits:** envelopes exceeding `MAX_PAYLOAD_SIZE` are rejected.
//! - **Schema version:** mismatched versions are rejected.
//! - **Double-submit:** duplicate `request_id` values are rejected.
//! - **Wrong-tab:** commands targeting a non-existent tab are rejected.
//!
//! See ADR-007 and `docs/contracts/tauri-capabilities-map.md`.

use std::collections::HashSet;

use browser_domain::ui::{CommandEnvelope, EventEnvelope, UiCommand, UI_CONTRACT_VERSION};

/// Maximum size for a serialized IPC payload (bytes).
pub const MAX_IPC_PAYLOAD_SIZE: usize = 65536;

/// Error returned by the IPC bridge when a command is rejected.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum IpcError {
    /// Schema version mismatch.
    UnknownVersion { expected: u32, got: u32 },
    /// Payload exceeds size limit.
    Oversized { size: usize, max: usize },
    /// Malformed JSON or unknown command variant.
    Malformed { reason: String },
    /// Request ID is empty or duplicate.
    DuplicateRequestId { request_id: String },
    /// Command targets a tab that does not exist.
    WrongTab { tab_id: String },
    /// Command is not in the allowlist.
    Unauthorized { command_type: String },
    /// App-level command incorrectly scoped to a tab.
    Misscoped { command_type: String },
    /// A scoped command's target does not match its envelope tab.
    TargetMismatch {
        scoped_tab_id: String,
        target_tab_id: String,
    },
}

impl std::fmt::Display for IpcError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            IpcError::UnknownVersion { expected, got } => {
                write!(f, "schema version mismatch: expected {expected}, got {got}")
            }
            IpcError::Oversized { size, max } => {
                write!(f, "payload oversized: {size} > {max}")
            }
            IpcError::Malformed { reason } => write!(f, "malformed: {reason}"),
            IpcError::DuplicateRequestId { request_id } => {
                write!(f, "duplicate request_id: {request_id}")
            }
            IpcError::WrongTab { tab_id } => write!(f, "wrong tab: {tab_id}"),
            IpcError::Unauthorized { command_type } => {
                write!(f, "unauthorized: {command_type}")
            }
            IpcError::Misscoped { command_type } => {
                write!(f, "misscoped: {command_type}")
            }
            IpcError::TargetMismatch {
                scoped_tab_id,
                target_tab_id,
            } => write!(
                f,
                "target tab {target_tab_id} does not match scoped tab {scoped_tab_id}"
            ),
        }
    }
}

impl std::error::Error for IpcError {}

/// The IPC bridge — validates commands before they reach the core.
pub struct IpcBridge {
    seen_requests: HashSet<String>,
    known_tabs: HashSet<String>,
}

impl IpcBridge {
    pub fn new() -> Self {
        Self {
            seen_requests: HashSet::new(),
            known_tabs: HashSet::new(),
        }
    }

    /// Register a tab as existing.
    pub fn register_tab(&mut self, tab_id: &str) {
        self.known_tabs.insert(tab_id.to_string());
    }

    /// Unregister a tab.
    pub fn unregister_tab(&mut self, tab_id: &str) {
        self.known_tabs.remove(tab_id);
    }

    /// Validate a raw JSON command envelope from the Tauri frontend.
    ///
    /// Returns `Ok(CommandEnvelope)` if valid, or `Err(IpcError)` if rejected.
    /// Successful validation records the `request_id` for duplicate detection.
    pub fn validate_command(&mut self, raw: &str) -> Result<CommandEnvelope, IpcError> {
        // Size check
        if raw.len() > MAX_IPC_PAYLOAD_SIZE {
            return Err(IpcError::Oversized {
                size: raw.len(),
                max: MAX_IPC_PAYLOAD_SIZE,
            });
        }

        // Parse JSON
        let envelope: CommandEnvelope =
            serde_json::from_str(raw).map_err(|e| IpcError::Malformed {
                reason: format!("JSON parse error: {e}"),
            })?;

        // Schema version
        if envelope.version != UI_CONTRACT_VERSION {
            return Err(IpcError::UnknownVersion {
                expected: UI_CONTRACT_VERSION,
                got: envelope.version,
            });
        }

        // Empty request_id check
        if envelope.request_id.0.is_empty() {
            return Err(IpcError::Malformed {
                reason: "request_id must not be empty".into(),
            });
        }

        // Duplicate detection
        if self.seen_requests.contains(&envelope.request_id.0) {
            return Err(IpcError::DuplicateRequestId {
                request_id: envelope.request_id.0.clone(),
            });
        }

        // Tab scoping check
        if let Some(ref tab_id) = envelope.tab_id {
            if let Some(target_tab_id) = target_tab_id(&envelope.command) {
                if target_tab_id != tab_id {
                    return Err(IpcError::TargetMismatch {
                        scoped_tab_id: tab_id.0.clone(),
                        target_tab_id: target_tab_id.0.clone(),
                    });
                }
            }
            // Tab must exist
            if !self.known_tabs.contains(&tab_id.0) {
                return Err(IpcError::WrongTab {
                    tab_id: tab_id.0.clone(),
                });
            }
        } else {
            // Commands without tab_id must be app-level
            if !is_app_level_command(&envelope.command) {
                return Err(IpcError::Misscoped {
                    command_type: command_type_name(&envelope.command).to_string(),
                });
            }
        }

        // Record request_id
        self.seen_requests.insert(envelope.request_id.0.clone());

        Ok(envelope)
    }

    /// Validate a raw JSON event envelope from the core to the UI.
    pub fn validate_event(raw: &str) -> Result<EventEnvelope, IpcError> {
        if raw.len() > MAX_IPC_PAYLOAD_SIZE {
            return Err(IpcError::Oversized {
                size: raw.len(),
                max: MAX_IPC_PAYLOAD_SIZE,
            });
        }

        let envelope: EventEnvelope =
            serde_json::from_str(raw).map_err(|e| IpcError::Malformed {
                reason: format!("JSON parse error: {e}"),
            })?;

        if envelope.version != UI_CONTRACT_VERSION {
            return Err(IpcError::UnknownVersion {
                expected: UI_CONTRACT_VERSION,
                got: envelope.version,
            });
        }

        Ok(envelope)
    }

    /// Clear seen requests (for testing or session reset).
    pub fn clear_seen(&mut self) {
        self.seen_requests.clear();
    }
}

impl Default for IpcBridge {
    fn default() -> Self {
        Self::new()
    }
}

/// Returns `true` if a command is app-level (can be sent without a `tab_id`).
fn is_app_level_command(command: &UiCommand) -> bool {
    matches!(command, UiCommand::NewTab)
}

fn target_tab_id(command: &UiCommand) -> Option<&browser_domain::ids::TabId> {
    match command {
        UiCommand::CloseTab { target_tab_id } | UiCommand::SelectTab { target_tab_id } => {
            Some(target_tab_id)
        }
        _ => None,
    }
}

/// Returns the string name of a command for diagnostics.
fn command_type_name(command: &UiCommand) -> &'static str {
    match command {
        UiCommand::Navigate { .. } => "navigate",
        UiCommand::Reload => "reload",
        UiCommand::GoBack => "go_back",
        UiCommand::GoForward => "go_forward",
        UiCommand::Stop => "stop",
        UiCommand::CloseTab { .. } => "close_tab",
        UiCommand::NewTab => "new_tab",
        UiCommand::SelectTab { .. } => "select_tab",
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use browser_domain::ids::{RequestId, TabId};
    use browser_domain::ui::UiCommand;

    fn make_command(command: UiCommand, request_id: &str, tab_id: Option<&str>) -> String {
        let env = CommandEnvelope {
            version: UI_CONTRACT_VERSION,
            request_id: RequestId(request_id.to_string()),
            tab_id: tab_id.map(|t| TabId(t.to_string())),
            command,
        };
        serde_json::to_string(&env).unwrap()
    }

    // --- Valid commands ---

    #[test]
    fn valid_navigate_with_tab() {
        let mut bridge = IpcBridge::new();
        bridge.register_tab("tab-1");
        let raw = make_command(
            UiCommand::Navigate {
                url: "https://example.com".into(),
            },
            "req-1",
            Some("tab-1"),
        );
        assert!(bridge.validate_command(&raw).is_ok());
    }

    #[test]
    fn valid_new_tab_app_level() {
        let mut bridge = IpcBridge::new();
        let raw = make_command(UiCommand::NewTab, "req-1", None);
        assert!(bridge.validate_command(&raw).is_ok());
    }

    // --- Unauthorized: unknown command variant ---

    #[test]
    fn unknown_command_variant_rejected() {
        let mut bridge = IpcBridge::new();
        let raw =
            r#"{"version":1,"request_id":"r1","tab_id":null,"command":{"type":"frobnicate"}}"#;
        assert!(matches!(
            bridge.validate_command(raw),
            Err(IpcError::Malformed { .. })
        ));
    }

    // --- Malformed ---

    #[test]
    fn malformed_json_rejected() {
        let mut bridge = IpcBridge::new();
        assert!(matches!(
            bridge.validate_command("not json"),
            Err(IpcError::Malformed { .. })
        ));
    }

    #[test]
    fn empty_request_id_rejected() {
        let mut bridge = IpcBridge::new();
        bridge.register_tab("tab-1");
        let raw = make_command(UiCommand::Reload, "", Some("tab-1"));
        assert!(matches!(
            bridge.validate_command(&raw),
            Err(IpcError::Malformed { .. })
        ));
    }

    // --- Unknown version ---

    #[test]
    fn wrong_version_rejected() {
        let mut bridge = IpcBridge::new();
        let raw =
            r#"{"version":99,"request_id":"r1","tab_id":"tab-1","command":{"type":"reload"}}"#;
        assert!(matches!(
            bridge.validate_command(raw),
            Err(IpcError::UnknownVersion { .. })
        ));
    }

    // --- Wrong tab ---

    #[test]
    fn wrong_tab_rejected() {
        let mut bridge = IpcBridge::new();
        // Don't register tab-1
        let raw = make_command(UiCommand::Reload, "req-1", Some("tab-1"));
        assert!(matches!(
            bridge.validate_command(&raw),
            Err(IpcError::WrongTab { .. })
        ));
    }

    #[test]
    fn unregistered_tab_rejected() {
        let mut bridge = IpcBridge::new();
        bridge.register_tab("tab-1");
        // Try to target tab-2 which doesn't exist
        let raw = make_command(UiCommand::Reload, "req-1", Some("tab-2"));
        assert!(matches!(
            bridge.validate_command(&raw),
            Err(IpcError::WrongTab { .. })
        ));
    }

    // --- Double submit ---

    #[test]
    fn duplicate_request_id_rejected() {
        let mut bridge = IpcBridge::new();
        bridge.register_tab("tab-1");
        let raw = make_command(UiCommand::Reload, "req-1", Some("tab-1"));
        assert!(bridge.validate_command(&raw).is_ok()); // First time
        let raw2 = make_command(UiCommand::Reload, "req-1", Some("tab-1"));
        assert!(matches!(
            bridge.validate_command(&raw2),
            Err(IpcError::DuplicateRequestId { .. })
        ));
    }

    #[test]
    fn different_request_ids_ok() {
        let mut bridge = IpcBridge::new();
        bridge.register_tab("tab-1");
        let raw1 = make_command(UiCommand::Reload, "req-1", Some("tab-1"));
        let raw2 = make_command(UiCommand::Reload, "req-2", Some("tab-1"));
        assert!(bridge.validate_command(&raw1).is_ok());
        assert!(bridge.validate_command(&raw2).is_ok());
    }

    // --- Misscoped ---

    #[test]
    fn navigate_without_tab_misscoped() {
        let mut bridge = IpcBridge::new();
        let raw = make_command(
            UiCommand::Navigate {
                url: "https://example.com".into(),
            },
            "req-1",
            None,
        );
        assert!(matches!(
            bridge.validate_command(&raw),
            Err(IpcError::Misscoped { .. })
        ));
    }

    #[test]
    fn reload_without_tab_misscoped() {
        let mut bridge = IpcBridge::new();
        let raw = make_command(UiCommand::Reload, "req-1", None);
        assert!(matches!(
            bridge.validate_command(&raw),
            Err(IpcError::Misscoped { .. })
        ));
    }

    // --- Oversized ---

    #[test]
    fn oversized_payload_rejected() {
        let mut bridge = IpcBridge::new();
        let huge = "x".repeat(MAX_IPC_PAYLOAD_SIZE + 1);
        assert!(matches!(
            bridge.validate_command(&huge),
            Err(IpcError::Oversized { .. })
        ));
    }

    // --- Event validation ---

    #[test]
    fn valid_event_accepted() {
        let raw = r#"{"version":1,"event":{"type":"navigation_started","url":"https://example.com"},"tab_id":null}"#;
        assert!(IpcBridge::validate_event(raw).is_ok());
    }

    #[test]
    fn event_wrong_version_rejected() {
        let raw = r#"{"version":99,"event":{"type":"navigation_started","url":"https://example.com"},"tab_id":null}"#;
        assert!(matches!(
            IpcBridge::validate_event(raw),
            Err(IpcError::UnknownVersion { .. })
        ));
    }

    #[test]
    fn event_malformed_rejected() {
        assert!(matches!(
            IpcBridge::validate_event("not json"),
            Err(IpcError::Malformed { .. })
        ));
    }

    // --- Register/unregister tabs ---

    #[test]
    fn unregister_tab_makes_commands_fail() {
        let mut bridge = IpcBridge::new();
        bridge.register_tab("tab-1");
        bridge.unregister_tab("tab-1");
        let raw = make_command(UiCommand::Reload, "req-1", Some("tab-1"));
        assert!(matches!(
            bridge.validate_command(&raw),
            Err(IpcError::WrongTab { .. })
        ));
    }
}
