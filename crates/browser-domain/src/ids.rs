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

/// Opaque identifier for a user profile.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct ProfileId(pub String);

/// Opaque identifier for a navigation session (one URL load attempt).
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct NavigationId(pub String);

/// Opaque identifier for an engine instance (domain-level, not engine-api).
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct EngineInstanceId(pub String);

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

impl ProfileId {
    pub fn new(id: impl Into<String>) -> Self {
        Self(id.into())
    }
}

impl NavigationId {
    pub fn new(id: impl Into<String>) -> Self {
        Self(id.into())
    }
}

impl EngineInstanceId {
    pub fn new(id: impl Into<String>) -> Self {
        Self(id.into())
    }
}

// ---------------------------------------------------------------------------
// Value objects
// ---------------------------------------------------------------------------

/// A validated URL value object.
///
/// This is a thin wrapper around `String` that enforces non-emptiness and
/// a maximum length. Full URL parsing is deferred to the engine; the domain
/// only validates that the value is a non-empty string of bounded length.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct Url(pub String);

/// Maximum allowed length for a URL in the domain.
pub const MAX_URL_LEN: usize = 8192;

impl Url {
    pub fn new(url: impl Into<String>) -> Result<Self, String> {
        let url = url.into();
        if url.is_empty() {
            return Err("URL must not be empty".to_string());
        }
        if url.len() > MAX_URL_LEN {
            return Err(format!("URL exceeds maximum length of {MAX_URL_LEN} bytes"));
        }
        Ok(Self(url))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

/// A validated page title value object.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct TabTitle(pub String);

/// Maximum allowed length for a page title.
pub const MAX_TITLE_LEN: usize = 1024;

impl TabTitle {
    pub fn new(title: impl Into<String>) -> Result<Self, String> {
        let title = title.into();
        if title.len() > MAX_TITLE_LEN {
            return Err(format!(
                "Title exceeds maximum length of {MAX_TITLE_LEN} bytes"
            ));
        }
        Ok(Self(title))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

// ---------------------------------------------------------------------------
// Display impls for all ID types
// ---------------------------------------------------------------------------

impl std::fmt::Display for TabId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl std::fmt::Display for RequestId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl std::fmt::Display for ProfileId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl std::fmt::Display for NavigationId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl std::fmt::Display for EngineInstanceId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl std::fmt::Display for Url {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl std::fmt::Display for TabTitle {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
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

    #[test]
    fn profile_id_roundtrips() {
        let id = ProfileId::new("profile-default");
        let json = serde_json::to_string(&id).expect("serialize");
        let back: ProfileId = serde_json::from_str(&json).expect("deserialize");
        assert_eq!(id, back);
    }

    #[test]
    fn navigation_id_roundtrips() {
        let id = NavigationId::new("nav-1");
        let json = serde_json::to_string(&id).expect("serialize");
        let back: NavigationId = serde_json::from_str(&json).expect("deserialize");
        assert_eq!(id, back);
    }

    #[test]
    fn engine_instance_id_roundtrips() {
        let id = EngineInstanceId::new("engine-1");
        let json = serde_json::to_string(&id).expect("serialize");
        let back: EngineInstanceId = serde_json::from_str(&json).expect("deserialize");
        assert_eq!(id, back);
    }

    // --- Value objects ---

    #[test]
    fn url_valid() {
        let url = Url::new("https://example.com").expect("valid");
        assert_eq!(url.as_str(), "https://example.com");
    }

    #[test]
    fn url_empty_rejected() {
        assert!(Url::new("").is_err());
    }

    #[test]
    fn url_oversized_rejected() {
        let long = "a".repeat(MAX_URL_LEN + 1);
        assert!(Url::new(long).is_err());
    }

    #[test]
    fn url_roundtrips() {
        let url = Url::new("https://example.com").unwrap();
        let json = serde_json::to_string(&url).expect("serialize");
        let back: Url = serde_json::from_str(&json).expect("deserialize");
        assert_eq!(url, back);
    }

    #[test]
    fn tab_title_valid() {
        let title = TabTitle::new("Test Page").unwrap();
        assert_eq!(title.as_str(), "Test Page");
    }

    #[test]
    fn tab_title_oversized_rejected() {
        let long = "x".repeat(MAX_TITLE_LEN + 1);
        assert!(TabTitle::new(long).is_err());
    }

    #[test]
    fn tab_title_empty_allowed() {
        // Empty titles are valid — pages may not have a title yet.
        let title = TabTitle::new("").unwrap();
        assert_eq!(title.as_str(), "");
    }

    #[test]
    fn tab_title_roundtrips() {
        let title = TabTitle::new("Hello").unwrap();
        let json = serde_json::to_string(&title).expect("serialize");
        let back: TabTitle = serde_json::from_str(&json).expect("deserialize");
        assert_eq!(title, back);
    }

    // --- Display ---

    #[test]
    fn ids_display_as_inner_string() {
        assert_eq!(TabId::new("tab-1").to_string(), "tab-1");
        assert_eq!(RequestId::new("req-1").to_string(), "req-1");
        assert_eq!(ProfileId::new("prof-1").to_string(), "prof-1");
        assert_eq!(NavigationId::new("nav-1").to_string(), "nav-1");
        assert_eq!(EngineInstanceId::new("eng-1").to_string(), "eng-1");
    }

    #[test]
    fn url_displays_as_string() {
        let url = Url::new("https://example.com").unwrap();
        assert_eq!(url.to_string(), "https://example.com");
    }

    #[test]
    fn tab_title_displays_as_string() {
        let title = TabTitle::new("Test").unwrap();
        assert_eq!(title.to_string(), "Test");
    }

    // --- No stringly identity: typed IDs are not interchangeable ---

    #[test]
    fn tab_id_and_request_id_are_different_types() {
        // This test documents that TabId and RequestId are distinct types.
        // The compiler enforces this — you cannot pass a TabId where a
        // RequestId is expected.
        let tab = TabId::new("x");
        let req = RequestId::new("x");
        // They are not equal despite having the same inner value.
        // (This comparison would not compile because they're different types.)
        assert_eq!(tab.0, req.0); // Same inner string
                                  // But they are different types — the compiler guarantees this.
    }
}
