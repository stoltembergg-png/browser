//! Versioned command/event envelopes with correlation, generation tracking,
//! capability negotiation and schema version handling.
//!
//! This module wraps the raw `EngineCommand` and `EngineEvent` in envelopes
//! that provide:
//! - **Schema version:** both sides reject unknown versions.
//! - **Correlation:** `request_id` links commands to responses.
//! - **Navigation generation:** stale events are filtered by generation.
//! - **Size limits:** payloads exceeding limits are rejected.
//! - **Unknown rejection:** commands/events outside the known schema are rejected.
//! - **Duplicate detection:** `request_id` dedup for commands.

use serde::{Deserialize, Serialize};

use crate::contract::{EngineCommand, EngineError};
use crate::events::{EngineEvent, NavigationGeneration};

/// The schema version for command/event envelopes.
pub const ENVELOPE_SCHEMA_VERSION: u32 = 1;

/// Maximum allowed size for a serialized envelope (in bytes).
pub const MAX_ENVELOPE_SIZE: usize = 65536;

// ---------------------------------------------------------------------------
// Command envelope
// ---------------------------------------------------------------------------

/// Envelope wrapping an engine command with versioning and correlation.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CommandEnvelope {
    pub schema_version: u32,
    pub request_id: String,
    pub instance_id: String,
    pub command: EngineCommand,
}

impl CommandEnvelope {
    /// Validate this envelope against the known schema version,
    /// size limits, and duplicate detection set.
    ///
    /// Returns `Ok(())` if valid, or an `EngineError` if rejected.
    pub fn validate(
        &self,
        seen: &mut std::collections::HashSet<String>,
    ) -> Result<(), EngineError> {
        // Version check
        if self.schema_version != ENVELOPE_SCHEMA_VERSION {
            return Err(EngineError::UnknownVersion {
                version: self.schema_version,
            });
        }

        // Request ID must be non-empty
        if self.request_id.is_empty() {
            return Err(EngineError::InvalidPayload {
                reason: "request_id must not be empty".into(),
            });
        }

        // Instance ID must be non-empty
        if self.instance_id.is_empty() {
            return Err(EngineError::InvalidPayload {
                reason: "instance_id must not be empty".into(),
            });
        }

        // Duplicate detection
        if seen.contains(&self.request_id) {
            return Err(EngineError::InvalidPayload {
                reason: format!("duplicate request_id: {}", self.request_id),
            });
        }
        seen.insert(self.request_id.clone());

        Ok(())
    }

    /// Serialize this envelope and check the size limit.
    pub fn to_json_checked(&self) -> Result<String, EngineError> {
        let json = serde_json::to_string(self).map_err(|e| EngineError::InvalidPayload {
            reason: format!("serialization failed: {e}"),
        })?;
        if json.len() > MAX_ENVELOPE_SIZE {
            return Err(EngineError::InvalidPayload {
                reason: format!(
                    "envelope size {} exceeds maximum {MAX_ENVELOPE_SIZE}",
                    json.len()
                ),
            });
        }
        Ok(json)
    }

    /// Parse and validate a JSON string into a `CommandEnvelope`.
    pub fn from_json(raw: &str) -> Result<Self, EngineError> {
        if raw.len() > MAX_ENVELOPE_SIZE {
            return Err(EngineError::InvalidPayload {
                reason: format!(
                    "raw input size {} exceeds maximum {MAX_ENVELOPE_SIZE}",
                    raw.len()
                ),
            });
        }
        serde_json::from_str(raw).map_err(|e| EngineError::InvalidPayload {
            reason: format!("malformed JSON: {e}"),
        })
    }
}

// ---------------------------------------------------------------------------
// Event envelope
// ---------------------------------------------------------------------------

/// Envelope wrapping an engine event with versioning and generation tracking.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct EventEnvelope {
    pub schema_version: u32,
    pub instance_id: String,
    /// Navigation generation for stale filtering. `None` for app-level events.
    pub generation: Option<NavigationGeneration>,
    pub event: EngineEvent,
}

impl EventEnvelope {
    /// Validate the schema version.
    pub fn validate_version(&self) -> Result<(), EngineError> {
        if self.schema_version != ENVELOPE_SCHEMA_VERSION {
            return Err(EngineError::UnknownVersion {
                version: self.schema_version,
            });
        }
        Ok(())
    }

    /// Returns `true` if this event is stale (generation < current).
    pub fn is_stale(&self, current_generation: u32) -> bool {
        match &self.generation {
            Some(NavigationGeneration(g)) => *g < current_generation,
            None => false, // App-level events are never stale
        }
    }

    /// Parse a JSON string into an `EventEnvelope`.
    pub fn from_json(raw: &str) -> Result<Self, EngineError> {
        if raw.len() > MAX_ENVELOPE_SIZE {
            return Err(EngineError::InvalidPayload {
                reason: format!(
                    "raw input size {} exceeds maximum {MAX_ENVELOPE_SIZE}",
                    raw.len()
                ),
            });
        }
        serde_json::from_str(raw).map_err(|e| EngineError::InvalidPayload {
            reason: format!("malformed JSON: {e}"),
        })
    }

    /// Serialize this envelope and check the size limit.
    pub fn to_json_checked(&self) -> Result<String, EngineError> {
        let json = serde_json::to_string(self).map_err(|e| EngineError::InvalidPayload {
            reason: format!("serialization failed: {e}"),
        })?;
        if json.len() > MAX_ENVELOPE_SIZE {
            return Err(EngineError::InvalidPayload {
                reason: format!(
                    "envelope size {} exceeds maximum {MAX_ENVELOPE_SIZE}",
                    json.len()
                ),
            });
        }
        Ok(json)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn valid_command_envelope() -> CommandEnvelope {
        CommandEnvelope {
            schema_version: ENVELOPE_SCHEMA_VERSION,
            request_id: "req-1".to_string(),
            instance_id: "inst-1".to_string(),
            command: EngineCommand::Reload,
        }
    }

    fn valid_event_envelope() -> EventEnvelope {
        EventEnvelope {
            schema_version: ENVELOPE_SCHEMA_VERSION,
            instance_id: "inst-1".to_string(),
            generation: Some(NavigationGeneration(1)),
            event: EngineEvent::CommandCompleted,
        }
    }

    // --- Command envelope: valid ---

    #[test]
    fn command_envelope_valid() {
        let env = valid_command_envelope();
        let mut seen = std::collections::HashSet::new();
        assert!(env.validate(&mut seen).is_ok());
    }

    // --- Unknown version ---

    #[test]
    fn command_envelope_wrong_version_rejected() {
        let mut env = valid_command_envelope();
        env.schema_version = 99;
        let mut seen = std::collections::HashSet::new();
        assert!(matches!(
            env.validate(&mut seen),
            Err(EngineError::UnknownVersion { .. })
        ));
    }

    #[test]
    fn event_envelope_wrong_version_rejected() {
        let mut env = valid_event_envelope();
        env.schema_version = 99;
        assert!(matches!(
            env.validate_version(),
            Err(EngineError::UnknownVersion { .. })
        ));
    }

    // --- Malformed ---

    #[test]
    fn command_envelope_malformed_json_rejected() {
        let result = CommandEnvelope::from_json("not json");
        assert!(matches!(result, Err(EngineError::InvalidPayload { .. })));
    }

    #[test]
    fn event_envelope_malformed_json_rejected() {
        let result = EventEnvelope::from_json("not json");
        assert!(matches!(result, Err(EngineError::InvalidPayload { .. })));
    }

    // --- Unknown command/event variant ---

    #[test]
    fn command_envelope_unknown_variant_rejected() {
        let raw = r#"{"schema_version":1,"request_id":"r1","instance_id":"i1","command":{"type":"frobnicate"}}"#;
        let result = CommandEnvelope::from_json(raw);
        assert!(result.is_err());
    }

    #[test]
    fn event_envelope_unknown_variant_rejected() {
        let raw = r#"{"schema_version":1,"instance_id":"i1","generation":null,"event":{"type":"explosion"}}"#;
        let result = EventEnvelope::from_json(raw);
        assert!(result.is_err());
    }

    // --- Stale event ---

    #[test]
    fn event_envelope_stale_generation() {
        let env = EventEnvelope {
            schema_version: ENVELOPE_SCHEMA_VERSION,
            instance_id: "inst-1".to_string(),
            generation: Some(NavigationGeneration(1)),
            event: EngineEvent::NavigationFinished {
                url: "https://example.com".to_string(),
                generation: NavigationGeneration(1),
            },
        };
        assert!(env.is_stale(2)); // gen 1 < current 2
        assert!(!env.is_stale(1)); // gen 1 == current 1 (not stale)
        assert!(!env.is_stale(0)); // gen 1 > current 0 (not stale)
    }

    #[test]
    fn event_envelope_no_generation_not_stale() {
        let env = EventEnvelope {
            schema_version: ENVELOPE_SCHEMA_VERSION,
            instance_id: "inst-1".to_string(),
            generation: None,
            event: EngineEvent::EngineReady,
        };
        assert!(!env.is_stale(999));
    }

    // --- Duplicate detection ---

    #[test]
    fn command_envelope_duplicate_request_id_rejected() {
        let env = valid_command_envelope();
        let mut seen = std::collections::HashSet::new();
        assert!(env.validate(&mut seen).is_ok()); // First time OK
        assert!(matches!(
            env.validate(&mut seen),
            Err(EngineError::InvalidPayload { .. })
        ));
    }

    #[test]
    fn command_envelope_different_request_ids_ok() {
        let mut seen = std::collections::HashSet::new();
        let env1 = CommandEnvelope {
            request_id: "req-1".to_string(),
            ..valid_command_envelope()
        };
        let env2 = CommandEnvelope {
            request_id: "req-2".to_string(),
            ..valid_command_envelope()
        };
        assert!(env1.validate(&mut seen).is_ok());
        assert!(env2.validate(&mut seen).is_ok());
    }

    // --- Empty fields ---

    #[test]
    fn command_envelope_empty_request_id_rejected() {
        let mut env = valid_command_envelope();
        env.request_id = "".to_string();
        let mut seen = std::collections::HashSet::new();
        assert!(matches!(
            env.validate(&mut seen),
            Err(EngineError::InvalidPayload { .. })
        ));
    }

    #[test]
    fn command_envelope_empty_instance_id_rejected() {
        let mut env = valid_command_envelope();
        env.instance_id = "".to_string();
        let mut seen = std::collections::HashSet::new();
        assert!(matches!(
            env.validate(&mut seen),
            Err(EngineError::InvalidPayload { .. })
        ));
    }

    // --- Size limits ---

    #[test]
    fn command_envelope_oversized_raw_rejected() {
        let huge = "x".repeat(MAX_ENVELOPE_SIZE + 1);
        let result = CommandEnvelope::from_json(&huge);
        assert!(matches!(result, Err(EngineError::InvalidPayload { .. })));
    }

    #[test]
    fn event_envelope_oversized_raw_rejected() {
        let huge = "x".repeat(MAX_ENVELOPE_SIZE + 1);
        let result = EventEnvelope::from_json(&huge);
        assert!(matches!(result, Err(EngineError::InvalidPayload { .. })));
    }

    // --- Roundtrips ---

    #[test]
    fn command_envelope_roundtrips() {
        let env = valid_command_envelope();
        let json = env.to_json_checked().unwrap();
        let back = CommandEnvelope::from_json(&json).unwrap();
        assert_eq!(env, back);
    }

    #[test]
    fn event_envelope_roundtrips() {
        let env = valid_event_envelope();
        let json = env.to_json_checked().unwrap();
        let back = EventEnvelope::from_json(&json).unwrap();
        assert_eq!(env, back);
    }
}
