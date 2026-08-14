//! Versioned serializable session records for browser state persistence.
//!
//! This module defines the schema for saving and restoring browser sessions.
//! It is pure domain logic — no file I/O, no credentials, no raw DOM.
//!
//! The schema is forward-compatible: unknown fields are preserved by serde
//! and migration logic converts older versions to the current one.

use crate::ids::{EngineInstanceId, ProfileId, TabId, Url};
use serde::{Deserialize, Serialize};
use std::fmt;

/// Current session schema version.
pub const SESSION_SCHEMA_VERSION: u32 = 1;

/// A complete browser session record.
///
/// Contains enough state to restore tabs, their URLs, ordering, and timestamps
/// without page content, credentials, or DOM data.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SessionRecord {
    /// Schema version — readers must reject or migrate unknown versions.
    pub version: u32,
    /// Profile this session belongs to.
    pub profile_id: ProfileId,
    /// Ordered list of tab records.
    pub tabs: Vec<SessionTab>,
    /// Index of the active tab, if any.
    pub active_tab_index: Option<u32>,
    /// When the session was created (Unix epoch seconds).
    pub created_at: u64,
    /// When the session was last updated (Unix epoch seconds).
    pub updated_at: u64,
}

/// One tab entry within a session record.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SessionTab {
    /// Tab identity.
    pub tab_id: TabId,
    /// Engine instance this tab was bound to.
    pub engine_instance_id: EngineInstanceId,
    /// Current committed URL, if any.
    pub current_url: Option<Url>,
    /// Page title at save time, if any.
    pub title: String,
    /// Whether the tab was visible when saved.
    pub visible: bool,
    /// When the tab was created (Unix epoch seconds).
    pub created_at: u64,
}

/// Errors produced during session serialization, deserialization, or migration.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SessionError {
    /// Schema version is higher than the reader supports.
    UnsupportedVersion { supported: u32, got: u32 },
    /// Schema version is zero or otherwise invalid.
    InvalidVersion,
    /// A tab entry references an out-of-range active tab index.
    ActiveIndexOutOfRange { index: u32, tab_count: u32 },
    /// A URL in the record failed domain validation.
    InvalidUrl { reason: String },
    /// Deserialization failed.
    Deserialize { reason: String },
}

impl fmt::Display for SessionError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::UnsupportedVersion { supported, got } => write!(
                f,
                "unsupported session schema version {got} (reader supports up to {supported})"
            ),
            Self::InvalidVersion => write!(f, "session schema version must be >= 1"),
            Self::ActiveIndexOutOfRange { index, tab_count } => {
                write!(f, "active_tab_index {index} out of range (0..{tab_count})")
            }
            Self::InvalidUrl { reason } => write!(f, "invalid URL in session: {reason}"),
            Self::Deserialize { reason } => write!(f, "session deserialize error: {reason}"),
        }
    }
}

impl std::error::Error for SessionError {}

/// Maximum number of tabs allowed in a single session record.
pub const MAX_SESSION_TABS: usize = 1024;

/// Maximum size of a serialized session record in bytes.
pub const MAX_SESSION_BYTES: usize = 1_048_576; // 1 MiB

impl SessionRecord {
    /// Create a new empty session for the given profile.
    pub fn new(profile_id: ProfileId, now: u64) -> Self {
        Self {
            version: SESSION_SCHEMA_VERSION,
            profile_id,
            tabs: Vec::new(),
            active_tab_index: None,
            created_at: now,
            updated_at: now,
        }
    }

    /// Add a tab to the session. Returns the tab's index.
    pub fn add_tab(&mut self, tab: SessionTab) -> u32 {
        self.updated_at = std::cmp::max(self.updated_at, tab.created_at);
        let index = self.tabs.len() as u32;
        self.tabs.push(tab);
        index
    }

    /// Set the active tab by index.
    pub fn set_active(&mut self, index: Option<u32>) -> Result<(), SessionError> {
        if let Some(idx) = index {
            if idx as usize >= self.tabs.len() {
                return Err(SessionError::ActiveIndexOutOfRange {
                    index: idx,
                    tab_count: self.tabs.len() as u32,
                });
            }
        }
        self.active_tab_index = index;
        Ok(())
    }

    /// Mark the session as updated at a given time.
    pub fn touch(&mut self, now: u64) {
        self.updated_at = now;
    }

    /// Validate the session record's internal consistency.
    pub fn validate(&self) -> Result<(), SessionError> {
        if self.version == 0 {
            return Err(SessionError::InvalidVersion);
        }
        if self.version > SESSION_SCHEMA_VERSION {
            return Err(SessionError::UnsupportedVersion {
                supported: SESSION_SCHEMA_VERSION,
                got: self.version,
            });
        }
        if self.tabs.len() > MAX_SESSION_TABS {
            return Err(SessionError::Deserialize {
                reason: format!("too many tabs: {} > {MAX_SESSION_TABS}", self.tabs.len()),
            });
        }
        if let Some(idx) = self.active_tab_index {
            if idx as usize >= self.tabs.len() {
                return Err(SessionError::ActiveIndexOutOfRange {
                    index: idx,
                    tab_count: self.tabs.len() as u32,
                });
            }
        }
        // Validate URLs in each tab
        for tab in &self.tabs {
            if let Some(ref url) = tab.current_url {
                if url.as_str().is_empty() {
                    return Err(SessionError::InvalidUrl {
                        reason: "empty URL".into(),
                    });
                }
            }
        }
        Ok(())
    }

    /// Serialize to JSON.
    pub fn to_json(&self) -> Result<String, SessionError> {
        serde_json::to_string(self).map_err(|e| SessionError::Deserialize {
            reason: e.to_string(),
        })
    }

    /// Deserialize from JSON, validate, and migrate if needed.
    pub fn from_json(raw: &str) -> Result<Self, SessionError> {
        if raw.len() > MAX_SESSION_BYTES {
            return Err(SessionError::Deserialize {
                reason: format!(
                    "session payload too large: {} > {MAX_SESSION_BYTES}",
                    raw.len()
                ),
            });
        }
        let record: SessionRecord =
            serde_json::from_str(raw).map_err(|e| SessionError::Deserialize {
                reason: e.to_string(),
            })?;
        record.validate()?;
        Ok(record)
    }

    /// Deserialize from a serde_json::Value, allowing unknown fields.
    /// Unknown fields are silently dropped by serde's default behavior,
    /// which is the forward-compatibility strategy.
    pub fn from_value(value: serde_json::Value) -> Result<Self, SessionError> {
        let record: SessionRecord =
            serde_json::from_value(value).map_err(|e| SessionError::Deserialize {
                reason: e.to_string(),
            })?;
        record.validate()?;
        Ok(record)
    }
}

/// Migrate a session record from an older schema version.
///
/// Currently only version 1 exists, so this is a no-op for known versions
/// and an error for unknown ones. Future versions will add migration logic here.
pub fn migrate_session(record: SessionRecord) -> Result<SessionRecord, SessionError> {
    if record.version == 0 {
        return Err(SessionError::InvalidVersion);
    }
    if record.version > SESSION_SCHEMA_VERSION {
        return Err(SessionError::UnsupportedVersion {
            supported: SESSION_SCHEMA_VERSION,
            got: record.version,
        });
    }
    // Version 1 is current — no migration needed.
    Ok(record)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_tab(id: &str, url: Option<&str>, now: u64) -> SessionTab {
        SessionTab {
            tab_id: TabId::new(id),
            engine_instance_id: EngineInstanceId::new("engine-1"),
            current_url: url.map(|u| Url::new(u).expect("valid URL")),
            title: "Test".into(),
            visible: true,
            created_at: now,
        }
    }

    #[test]
    fn empty_session_roundtrips() {
        let session = SessionRecord::new(ProfileId::new("profile-1"), 1000);
        let json = session.to_json().expect("serialize");
        let back = SessionRecord::from_json(&json).expect("deserialize");
        assert_eq!(session, back);
    }

    #[test]
    fn session_with_tabs_roundtrips() {
        let mut session = SessionRecord::new(ProfileId::new("profile-1"), 1000);
        session.add_tab(make_tab("tab-1", Some("https://example.com"), 1000));
        session.add_tab(make_tab("tab-2", Some("https://other.com"), 1001));
        session.set_active(Some(1)).expect("active index valid");

        let json = session.to_json().expect("serialize");
        let back = SessionRecord::from_json(&json).expect("deserialize");
        assert_eq!(session, back);
        assert_eq!(back.tabs.len(), 2);
        assert_eq!(back.active_tab_index, Some(1));
    }

    #[test]
    fn unknown_fields_are_ignored_for_forward_compat() {
        let raw = r#"{
            "version": 1,
            "profile_id": "profile-1",
            "tabs": [],
            "active_tab_index": null,
            "created_at": 1000,
            "updated_at": 1000,
            "future_field": "ignored",
            "another_unknown": 42
        }"#;
        let record = SessionRecord::from_json(raw).expect("deserialize with unknown fields");
        assert_eq!(record.profile_id, ProfileId::new("profile-1"));
        assert!(record.tabs.is_empty());
    }

    #[test]
    fn version_zero_rejected() {
        let raw = r#"{
            "version": 0,
            "profile_id": "profile-1",
            "tabs": [],
            "active_tab_index": null,
            "created_at": 1000,
            "updated_at": 1000
        }"#;
        assert!(matches!(
            SessionRecord::from_json(raw),
            Err(SessionError::InvalidVersion)
        ));
    }

    #[test]
    fn future_version_rejected() {
        let raw = r#"{
            "version": 99,
            "profile_id": "profile-1",
            "tabs": [],
            "active_tab_index": null,
            "created_at": 1000,
            "updated_at": 1000
        }"#;
        assert!(matches!(
            SessionRecord::from_json(raw),
            Err(SessionError::UnsupportedVersion { .. })
        ));
    }

    #[test]
    fn active_index_out_of_range_rejected() {
        let mut session = SessionRecord::new(ProfileId::new("p"), 1000);
        session.add_tab(make_tab("t1", Some("https://a.com"), 1000));
        assert!(matches!(
            session.set_active(Some(5)),
            Err(SessionError::ActiveIndexOutOfRange { .. })
        ));
    }

    #[test]
    fn oversized_payload_rejected() {
        let huge = "x".repeat(MAX_SESSION_BYTES + 1);
        assert!(matches!(
            SessionRecord::from_json(&huge),
            Err(SessionError::Deserialize { .. })
        ));
    }

    #[test]
    fn malformed_json_rejected() {
        assert!(matches!(
            SessionRecord::from_json("not json"),
            Err(SessionError::Deserialize { .. })
        ));
    }

    #[test]
    fn empty_url_in_tab_rejected() {
        let session = SessionRecord {
            version: 1,
            profile_id: ProfileId::new("p"),
            tabs: vec![SessionTab {
                tab_id: TabId::new("t1"),
                engine_instance_id: EngineInstanceId::new("e1"),
                current_url: Some(Url("".into())),
                title: "Empty".into(),
                visible: true,
                created_at: 1000,
            }],
            active_tab_index: Some(0),
            created_at: 1000,
            updated_at: 1000,
        };
        assert!(matches!(
            session.validate(),
            Err(SessionError::InvalidUrl { .. })
        ));
    }

    #[test]
    fn migrate_v1_is_noop() {
        let session = SessionRecord::new(ProfileId::new("p"), 1000);
        let migrated = migrate_session(session.clone()).expect("migrate");
        assert_eq!(session, migrated);
    }

    #[test]
    fn migrate_future_version_rejected() {
        let session = SessionRecord {
            version: 99,
            ..SessionRecord::new(ProfileId::new("p"), 1000)
        };
        assert!(matches!(
            migrate_session(session),
            Err(SessionError::UnsupportedVersion { .. })
        ));
    }

    #[test]
    fn no_secrets_in_serialized_output() {
        let mut session = SessionRecord::new(ProfileId::new("profile-1"), 1000);
        session.add_tab(make_tab("tab-1", Some("https://private.example"), 1000));
        let json = session.to_json().expect("serialize");
        assert!(!json.contains("password"));
        assert!(!json.contains("token"));
        assert!(!json.contains("secret"));
        assert!(!json.contains("credential"));
    }

    #[test]
    fn touch_updates_timestamp() {
        let mut session = SessionRecord::new(ProfileId::new("p"), 1000);
        session.touch(2000);
        assert_eq!(session.updated_at, 2000);
    }
}
