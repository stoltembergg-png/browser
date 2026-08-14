//! UI shell contract — typed commands and events for the Tauri frontend.
//!
//! This module defines the schema that the privileged Tauri frontend uses to
//! communicate with the browser core. It deliberately does **not** expose a
//! generic `invoke` bridge: every command is a named variant with a typed
//! payload, and every event is a named variant the core emits to the UI.
//!
//! ## Design constraints
//!
//! - Commands and events are versioned via the `CommandEnvelope` / `EventEnvelope`
//!   wrapper so the core can reject unknown schema versions.
//! - Every command carries a `request_id` for correlation and a `tab_id` when
//!   scoped to a tab.
//! - Envelopes have a strict size expectation (enforced at the bridge layer,
//!   not in the domain).
//! - Unknown command or event variants are rejected — there is no catch-all.
//! - This contract is engine-neutral: it does not leak Servo types or Tauri
//!   specifics into the domain.

use serde::{Deserialize, Serialize};

use crate::ids::{RequestId, TabId};

/// Schema version for the UI contract.
pub const UI_CONTRACT_VERSION: u32 = 1;

// ---------------------------------------------------------------------------
// Commands (UI → core)
// ---------------------------------------------------------------------------

/// Envelope wrapping every command the UI sends to the core.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CommandEnvelope {
    /// Schema version — the core rejects mismatched versions.
    pub version: u32,
    /// Correlation ID set by the UI; the core echoes it in the response event.
    pub request_id: RequestId,
    /// Tab this command targets (None for app-level commands).
    pub tab_id: Option<TabId>,
    /// The typed command payload.
    pub command: UiCommand,
}

/// Typed commands the privileged UI may send to the core.
///
/// There is intentionally **no** `Invoke(String)` or generic fallback variant.
/// Each variant is a named, typed operation with a bounded payload.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum UiCommand {
    /// User submitted text in the omnibox.
    Navigate { url: String },
    /// User clicked the reload button.
    Reload,
    /// User clicked the back button.
    GoBack,
    /// User clicked the forward button.
    GoForward,
    /// User clicked the stop button.
    Stop,
    /// User requested a new tab.
    NewTab,
    /// User closed a tab.
    CloseTab { target_tab_id: TabId },
    /// User selected a different tab.
    SelectTab { target_tab_id: TabId },
    /// User accepted a download; the policy decides the destination.
    DownloadStart {
        url: String,
        suggested_name: String,
        content_length: Option<u64>,
    },
    /// User cancelled an active download.
    DownloadCancel { download_id: u64 },
    /// User asked to retry a failed or cancelled download.
    DownloadRetry { download_id: u64 },
}

// ---------------------------------------------------------------------------
// Events (core → UI)
// ---------------------------------------------------------------------------

/// Envelope wrapping every event the core emits to the UI.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct EventEnvelope {
    /// Schema version — the UI rejects mismatched versions.
    pub version: u32,
    /// Tab this event refers to (None for app-level events).
    pub tab_id: Option<TabId>,
    /// The typed event payload.
    pub event: UiEvent,
}

/// Typed events the core emits to the privileged UI.
///
/// These are shell-level events for the mock; they do not carry page content,
/// DOM data or untrusted payload.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum UiEvent {
    /// A tab was created.
    TabCreated { tab_id: TabId },
    /// A tab was closed.
    TabClosed { tab_id: TabId },
    /// The active tab changed.
    TabSelected { tab_id: TabId },
    /// Navigation to a URL started.
    NavigationStarted { url: String },
    /// Navigation committed (first paint / URL confirmed).
    NavigationCommitted { url: String },
    /// Navigation finished successfully.
    NavigationFinished { url: String },
    /// Navigation failed with a typed error.
    NavigationFailed { reason: String },
    /// Navigation was cancelled by Stop or a newer navigation.
    NavigationCancelled,
    /// The page title changed.
    TitleChanged { title: String },
    /// A download started and is streaming.
    DownloadStarted {
        download_id: u64,
        suggested_name: String,
    },
    /// Download progress (bytes streamed so far).
    DownloadProgress { download_id: u64, bytes: u64 },
    /// A download reached a safe final path.
    DownloadCompleted {
        download_id: u64,
        final_path: String,
    },
    /// A download failed with a typed reason.
    DownloadFailed { download_id: u64, reason: String },
    /// A download was cancelled.
    DownloadCancelled { download_id: u64 },
    /// An unknown or malformed command was received.
    ///
    /// The core emits this instead of executing an invalid command so the UI
    /// can surface an error without crashing.
    CommandRejected { reason: String },
}

// ---------------------------------------------------------------------------
// Navigation chrome state
// ---------------------------------------------------------------------------

/// Engine-neutral state rendered by the browser chrome.
///
/// The state deliberately contains no page content or engine handles. The
/// engine/tab boundary is responsible for rejecting stale events before they
/// reach this projection.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct NavigationChromeState {
    url: Option<String>,
    title: Option<String>,
    loading: bool,
    error: Option<String>,
    can_go_back: bool,
    can_go_forward: bool,
}

impl NavigationChromeState {
    pub fn new() -> Self {
        Self::default()
    }

    /// Apply a typed event and report whether it changed chrome state.
    pub fn apply_event(&mut self, event: &UiEvent) -> bool {
        match event {
            UiEvent::NavigationStarted { url } => {
                self.url = Some(url.clone());
                self.loading = true;
                self.error = None;
                true
            }
            UiEvent::NavigationCommitted { url } => {
                self.url = Some(url.clone());
                self.error = None;
                true
            }
            UiEvent::NavigationFinished { url } => {
                self.url = Some(url.clone());
                self.loading = false;
                self.error = None;
                true
            }
            UiEvent::NavigationFailed { reason } => {
                self.loading = false;
                self.error = Some(reason.clone());
                true
            }
            UiEvent::NavigationCancelled => {
                self.loading = false;
                self.error = None;
                true
            }
            UiEvent::TitleChanged { title } if self.url.is_some() => {
                self.title = Some(title.clone());
                true
            }
            UiEvent::CommandRejected { reason } => {
                self.loading = false;
                self.error = Some(reason.clone());
                true
            }
            _ => false,
        }
    }

    pub fn set_history_capabilities(&mut self, can_go_back: bool, can_go_forward: bool) {
        self.can_go_back = can_go_back;
        self.can_go_forward = can_go_forward;
    }

    pub fn url(&self) -> Option<&str> {
        self.url.as_deref()
    }

    pub fn title(&self) -> Option<&str> {
        self.title.as_deref()
    }

    pub fn error(&self) -> Option<&str> {
        self.error.as_deref()
    }

    pub fn is_loading(&self) -> bool {
        self.loading
    }

    pub fn can_go_back(&self) -> bool {
        self.can_go_back
    }

    pub fn can_go_forward(&self) -> bool {
        self.can_go_forward
    }

    pub fn can_reload(&self) -> bool {
        self.url.is_some() && !self.loading
    }

    pub fn can_stop(&self) -> bool {
        self.loading
    }
}

// ---------------------------------------------------------------------------
// Validation helpers
// ---------------------------------------------------------------------------

/// Maximum allowed length for a URL payload in a command.
pub const MAX_URL_LEN: usize = 8192;

/// Maximum allowed length for a title string in an event.
pub const MAX_TITLE_LEN: usize = 1024;

/// Maximum allowed length for a download suggested name.
pub const MAX_SUGGESTED_NAME_LEN: usize = 255;

/// Maximum length for a download failure reason string.
pub const MAX_DOWNLOAD_REASON_LEN: usize = 1024;

/// Validate a navigate command's URL length.
///
/// Returns an error message if the URL exceeds the limit or is empty.
pub fn validate_navigate_url(url: &str) -> Result<(), String> {
    if url.is_empty() {
        return Err("URL must not be empty".to_string());
    }
    if url.len() > MAX_URL_LEN {
        return Err(format!("URL exceeds maximum length of {MAX_URL_LEN} bytes"));
    }
    Ok(())
}

/// Validate a download start command's suggested name.
///
/// The name is metadata only — the destination is decided by the download
/// policy in the core, never by this string.
pub fn validate_suggested_name(name: &str) -> Result<(), String> {
    if name.is_empty() {
        return Err("suggested name must not be empty".to_string());
    }
    if name.len() > MAX_SUGGESTED_NAME_LEN {
        return Err(format!(
            "suggested name exceeds maximum length of {MAX_SUGGESTED_NAME_LEN} bytes"
        ));
    }
    Ok(())
}

/// Validate a download failure reason event string.
pub fn validate_download_reason(reason: &str) -> Result<(), String> {
    if reason.len() > MAX_DOWNLOAD_REASON_LEN {
        return Err(format!(
            "download reason exceeds maximum length of {MAX_DOWNLOAD_REASON_LEN} bytes"
        ));
    }
    Ok(())
}

/// Validate a title event's title length.
pub fn validate_title(title: &str) -> Result<(), String> {
    if title.len() > MAX_TITLE_LEN {
        return Err(format!(
            "Title exceeds maximum length of {MAX_TITLE_LEN} bytes"
        ));
    }
    Ok(())
}

/// Result of parsing and validating a raw JSON command envelope.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CommandParseResult {
    /// The envelope is well-formed and passes validation.
    Ok(CommandEnvelope),
    /// The JSON is malformed or fails schema validation.
    Rejected(String),
}

/// Result of parsing and validating a raw JSON event envelope.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum EventParseResult {
    /// The envelope is well-formed and passes validation.
    Ok(EventEnvelope),
    /// The JSON is malformed or fails schema validation.
    Rejected(String),
}

/// Parse a raw JSON string into a `CommandEnvelope`, rejecting malformed input.
///
/// This is the function the bridge layer calls before forwarding a command to
/// the core. It performs:
///   1. JSON deserialization.
///   2. Schema version check.
///   3. Payload validation (URL/title lengths).
pub fn parse_command(raw: &str) -> CommandParseResult {
    let envelope: CommandEnvelope = match serde_json::from_str(raw) {
        Ok(e) => e,
        Err(err) => return CommandParseResult::Rejected(format!("malformed JSON: {err}")),
    };
    if envelope.version != UI_CONTRACT_VERSION {
        return CommandParseResult::Rejected(format!(
            "unsupported command version {} (expected {})",
            envelope.version, UI_CONTRACT_VERSION
        ));
    }
    if let UiCommand::Navigate { url } = &envelope.command {
        if let Err(reason) = validate_navigate_url(url) {
            return CommandParseResult::Rejected(reason);
        }
    }
    if let UiCommand::DownloadStart {
        url,
        suggested_name,
        ..
    } = &envelope.command
    {
        if let Err(reason) = validate_navigate_url(url) {
            return CommandParseResult::Rejected(reason);
        }
        if let Err(reason) = validate_suggested_name(suggested_name) {
            return CommandParseResult::Rejected(reason);
        }
    }
    CommandParseResult::Ok(envelope)
}

/// Parse a raw JSON string into an `EventEnvelope`, rejecting malformed input.
pub fn parse_event(raw: &str) -> EventParseResult {
    let envelope: EventEnvelope = match serde_json::from_str(raw) {
        Ok(e) => e,
        Err(err) => return EventParseResult::Rejected(format!("malformed JSON: {err}")),
    };
    if envelope.version != UI_CONTRACT_VERSION {
        return EventParseResult::Rejected(format!(
            "unsupported event version {} (expected {})",
            envelope.version, UI_CONTRACT_VERSION
        ));
    }
    if let UiEvent::TitleChanged { title } = &envelope.event {
        if let Err(reason) = validate_title(title) {
            return EventParseResult::Rejected(reason);
        }
    }
    if let UiEvent::DownloadFailed { reason, .. } = &envelope.event {
        if let Err(reason) = validate_download_reason(reason) {
            return EventParseResult::Rejected(reason);
        }
    }
    EventParseResult::Ok(envelope)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn valid_envelope_json(command: &str) -> String {
        format!(r#"{{"version":1,"request_id":"req-1","tab_id":"tab-1","command":{command}}}"#)
    }

    #[test]
    fn parse_valid_navigate_command() {
        let raw = valid_envelope_json(r#"{"type":"navigate","url":"https://example.com"}"#);
        match parse_command(&raw) {
            CommandParseResult::Ok(env) => {
                assert_eq!(env.version, 1);
                assert!(matches!(env.command, UiCommand::Navigate { .. }));
            }
            CommandParseResult::Rejected(reason) => panic!("rejected: {reason}"),
        }
    }

    #[test]
    fn parse_malformed_json_rejected() {
        let raw = "{{not valid json";
        match parse_command(raw) {
            CommandParseResult::Rejected(reason) => assert!(reason.contains("malformed")),
            CommandParseResult::Ok(_) => panic!("malformed JSON should be rejected"),
        }
    }

    #[test]
    fn parse_unknown_command_variant_rejected() {
        let raw = valid_envelope_json(r#"{"type":"frobnicate","payload":42}"#);
        match parse_command(&raw) {
            CommandParseResult::Rejected(reason) => assert!(reason.contains("unknown")),
            CommandParseResult::Ok(_) => panic!("unknown variant should be rejected"),
        }
    }

    #[test]
    fn parse_wrong_version_rejected() {
        let raw =
            r#"{"version":99,"request_id":"req-1","tab_id":"tab-1","command":{"type":"reload"}}"#;
        match parse_command(raw) {
            CommandParseResult::Rejected(reason) => assert!(reason.contains("version")),
            CommandParseResult::Ok(_) => panic!("wrong version should be rejected"),
        }
    }

    #[test]
    fn empty_url_rejected() {
        let raw = valid_envelope_json(r#"{"type":"navigate","url":""}"#);
        match parse_command(&raw) {
            CommandParseResult::Rejected(reason) => assert!(reason.contains("empty")),
            CommandParseResult::Ok(_) => panic!("empty URL should be rejected"),
        }
    }

    #[test]
    fn oversized_url_rejected() {
        let url = "a".repeat(MAX_URL_LEN + 1);
        let raw = valid_envelope_json(&format!(r#"{{"type":"navigate","url":"{url}"}}"#));
        match parse_command(&raw) {
            CommandParseResult::Rejected(reason) => assert!(reason.contains("maximum")),
            CommandParseResult::Ok(_) => panic!("oversized URL should be rejected"),
        }
    }

    #[test]
    fn parse_valid_event() {
        let raw =
            r#"{"version":1,"tab_id":"tab-1","event":{"type":"title_changed","title":"Test"}}"#;
        match parse_event(raw) {
            EventParseResult::Ok(env) => {
                assert!(matches!(env.event, UiEvent::TitleChanged { .. }));
            }
            EventParseResult::Rejected(reason) => panic!("rejected: {reason}"),
        }
    }

    #[test]
    fn parse_navigation_cancelled_event() {
        let raw = r#"{"version":1,"tab_id":"tab-1","event":{"type":"navigation_cancelled"}}"#;
        match parse_event(raw) {
            EventParseResult::Ok(env) => assert!(matches!(env.event, UiEvent::NavigationCancelled)),
            EventParseResult::Rejected(reason) => panic!("rejected: {reason}"),
        }
    }

    #[test]
    fn parse_malformed_event_rejected() {
        let raw = "not json at all";
        match parse_event(raw) {
            EventParseResult::Rejected(reason) => assert!(reason.contains("malformed")),
            EventParseResult::Ok(_) => panic!("malformed event should be rejected"),
        }
    }

    #[test]
    fn parse_unknown_event_variant_rejected() {
        let raw =
            r#"{"version":1,"tab_id":"tab-1","event":{"type":"explosion","severity":"high"}}"#;
        match parse_event(raw) {
            EventParseResult::Rejected(reason) => assert!(reason.contains("unknown")),
            EventParseResult::Ok(_) => panic!("unknown event variant should be rejected"),
        }
    }

    #[test]
    fn oversized_title_rejected() {
        let title = "x".repeat(MAX_TITLE_LEN + 1);
        let raw = format!(
            r#"{{"version":1,"tab_id":"tab-1","event":{{"type":"title_changed","title":"{title}"}}}}"#
        );
        match parse_event(&raw) {
            EventParseResult::Rejected(reason) => assert!(reason.contains("maximum")),
            EventParseResult::Ok(_) => panic!("oversized title should be rejected"),
        }
    }

    #[test]
    fn parse_valid_download_start_command() {
        let raw = valid_envelope_json(
            r#"{"type":"download_start","url":"https://example.test/f.bin","suggested_name":"f.bin","content_length":10}"#,
        );
        match parse_command(&raw) {
            CommandParseResult::Ok(env) => {
                assert!(matches!(env.command, UiCommand::DownloadStart { .. }));
            }
            CommandParseResult::Rejected(reason) => panic!("rejected: {reason}"),
        }
    }

    #[test]
    fn parse_download_start_with_empty_name_rejected() {
        let raw = valid_envelope_json(
            r#"{"type":"download_start","url":"https://example.test/f.bin","suggested_name":"","content_length":null}"#,
        );
        match parse_command(&raw) {
            CommandParseResult::Rejected(reason) => assert!(reason.contains("empty")),
            CommandParseResult::Ok(_) => panic!("empty suggested name should be rejected"),
        }
    }

    #[test]
    fn parse_download_cancel_command() {
        let raw = valid_envelope_json(r#"{"type":"download_cancel","download_id":7}"#);
        match parse_command(&raw) {
            CommandParseResult::Ok(env) => {
                assert!(matches!(
                    env.command,
                    UiCommand::DownloadCancel { download_id: 7 }
                ));
            }
            CommandParseResult::Rejected(reason) => panic!("rejected: {reason}"),
        }
    }

    #[test]
    fn parse_download_retry_command() {
        let raw = valid_envelope_json(r#"{"type":"download_retry","download_id":7}"#);
        match parse_command(&raw) {
            CommandParseResult::Ok(env) => {
                assert!(matches!(
                    env.command,
                    UiCommand::DownloadRetry { download_id: 7 }
                ));
            }
            CommandParseResult::Rejected(reason) => panic!("rejected: {reason}"),
        }
    }

    #[test]
    fn parse_download_progress_event() {
        let raw = r#"{"version":1,"tab_id":null,"event":{"type":"download_progress","download_id":3,"bytes":42}}"#;
        match parse_event(raw) {
            EventParseResult::Ok(env) => {
                assert!(matches!(
                    env.event,
                    UiEvent::DownloadProgress {
                        download_id: 3,
                        bytes: 42
                    }
                ));
            }
            EventParseResult::Rejected(reason) => panic!("rejected: {reason}"),
        }
    }

    #[test]
    fn parse_download_completed_event() {
        let raw = r#"{"version":1,"tab_id":null,"event":{"type":"download_completed","download_id":3,"final_path":"/tmp/f.bin"}}"#;
        match parse_event(raw) {
            EventParseResult::Ok(env) => {
                assert!(matches!(
                    env.event,
                    UiEvent::DownloadCompleted { download_id: 3, .. }
                ));
            }
            EventParseResult::Rejected(reason) => panic!("rejected: {reason}"),
        }
    }

    #[test]
    fn parse_download_failed_event_with_oversized_reason_rejected() {
        let reason = "x".repeat(MAX_DOWNLOAD_REASON_LEN + 1);
        let raw = format!(
            r#"{{"version":1,"tab_id":null,"event":{{"type":"download_failed","download_id":3,"reason":"{reason}"}}}}"#
        );
        match parse_event(&raw) {
            EventParseResult::Rejected(reason) => assert!(reason.contains("maximum")),
            EventParseResult::Ok(_) => panic!("oversized reason should be rejected"),
        }
    }

    #[test]
    fn parse_download_cancelled_event() {
        let raw =
            r#"{"version":1,"tab_id":null,"event":{"type":"download_cancelled","download_id":3}}"#;
        match parse_event(raw) {
            EventParseResult::Ok(env) => {
                assert!(matches!(
                    env.event,
                    UiEvent::DownloadCancelled { download_id: 3 }
                ));
            }
            EventParseResult::Rejected(reason) => panic!("rejected: {reason}"),
        }
    }

    #[test]
    fn command_envelope_roundtrips_through_serde() {
        let env = CommandEnvelope {
            version: 1,
            request_id: RequestId::new("req-99"),
            tab_id: Some(TabId::new("tab-3")),
            command: UiCommand::NewTab,
        };
        let json = serde_json::to_string(&env).expect("serialize");
        let back: CommandEnvelope = serde_json::from_str(&json).expect("deserialize");
        assert_eq!(env, back);
    }

    #[test]
    fn event_envelope_roundtrips_through_serde() {
        let env = EventEnvelope {
            version: 1,
            tab_id: None,
            event: UiEvent::TabCreated {
                tab_id: TabId::new("tab-7"),
            },
        };
        let json = serde_json::to_string(&env).expect("serialize");
        let back: EventEnvelope = serde_json::from_str(&json).expect("deserialize");
        assert_eq!(env, back);
    }
}
