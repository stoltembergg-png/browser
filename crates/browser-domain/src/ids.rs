//! Stable identifier types for the browser domain.
//!
//! All identifiers are opaque strings with a typed wrapper so that a TabId
//! cannot be confused with a RequestId at a call site.

use serde::{Deserialize, Serialize};

/// Opaque identifier for a browser tab.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct TabId(pub String);

/// Opaque identifier for a UI-originated command request.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct RequestId(pub String);

impl TabId {
    pub fn new(id: impl Into<String>) -> Self {
        Self(id.into())
    }
}

impl RequestId {
    pub fn new(id: impl Into<String>) -> Self {
        Self(id.into())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tab_id_serializes_as_string() {
        let id = TabId::new("tab-1");
        let json = serde_json::to_string(&id).expect("serialize");
        assert_eq!(json, "\"tab-1\"");
    }

    #[test]
    fn request_id_roundtrips() {
        let id = RequestId::new("req-42");
        let json = serde_json::to_string(&id).expect("serialize");
        let back: RequestId = serde_json::from_str(&json).expect("deserialize");
        assert_eq!(id, back);
    }
}
