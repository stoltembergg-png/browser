//! Engine-neutral popup/new-window policy.
//!
//! Determines whether a request to open a new webview or window is allowed,
//! denied, or routed to a new tab. This is pure domain logic — no engine types,
//! no UI framework, no real OS windows.

use browser_domain::ids::TabId;
use std::fmt;

/// The origin that requested the popup/new window.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct Origin(pub String);

impl Origin {
    pub fn new(origin: impl Into<String>) -> Self {
        Self(origin.into())
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for Origin {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

/// Whether the popup request was initiated by a user gesture.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UserGesture {
    /// Request came from a user action (click, tap).
    Yes,
    /// Request was not from a user gesture (script, timer).
    No,
}

/// Context for a popup/new-window request.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PopupRequest {
    /// Origin of the page requesting the new window.
    pub opener_origin: Origin,
    /// The target URL to open.
    pub target_url: String,
    /// Whether a user gesture was involved.
    pub user_gesture: UserGesture,
    /// The tab from which the request originated, if any.
    pub opener_tab_id: Option<TabId>,
}

/// The decision made by the popup policy.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PopupDecision {
    /// Open the URL in a new tab.
    NewTab { tab_id: TabId },
    /// Navigate the current/original tab to the URL.
    RouteToCurrentTab { tab_id: TabId },
    /// The request was denied.
    Denied { reason: PopupDenyReason },
}

/// Reasons a popup request was denied.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PopupDenyReason {
    /// No user gesture and not same-origin.
    NoUserGesture,
    /// Target origin differs from opener origin and no gesture.
    CrossOriginWithoutGesture,
    /// Popup storm detected (too many requests in a window).
    PopupStorm { count: u32, limit: u32 },
    /// Target URL is empty or invalid.
    InvalidTargetUrl,
}

impl fmt::Display for PopupDenyReason {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::NoUserGesture => write!(f, "popup denied: no user gesture"),
            Self::CrossOriginWithoutGesture => {
                write!(f, "popup denied: cross-origin without user gesture")
            }
            Self::PopupStorm { count, limit } => {
                write!(f, "popup storm: {count} requests exceed limit of {limit}")
            }
            Self::InvalidTargetUrl => write!(f, "popup denied: invalid target URL"),
        }
    }
}

/// Configuration for the popup policy.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PopupPolicyConfig {
    /// Maximum popups allowed from one origin per time window.
    pub max_popups_per_window: u32,
    /// Whether to allow same-origin popups without gesture.
    pub allow_same_origin_without_gesture: bool,
    /// Whether to route denied popups to the current tab.
    pub route_denied_to_current_tab: bool,
}

impl Default for PopupPolicyConfig {
    fn default() -> Self {
        Self {
            max_popups_per_window: 3,
            allow_same_origin_without_gesture: false,
            route_denied_to_current_tab: true,
        }
    }
}

/// Popup storm tracker — counts requests per origin.
#[derive(Debug, Clone, Default)]
pub struct PopupStormTracker {
    counts: std::collections::HashMap<Origin, u32>,
}

impl PopupStormTracker {
    pub fn new() -> Self {
        Self::default()
    }

    /// Record a popup request from the given origin and return the current count.
    pub fn record(&mut self, origin: &Origin) -> u32 {
        let count = self.counts.entry(origin.clone()).or_insert(0);
        *count += 1;
        *count
    }

    /// Check if the origin has exceeded the popup storm limit.
    pub fn is_storm(&self, origin: &Origin, limit: u32) -> bool {
        self.counts.get(origin).is_some_and(|&c| c > limit)
    }

    /// Reset the counter for a specific origin.
    pub fn reset(&mut self, origin: &Origin) {
        self.counts.remove(origin);
    }

    /// Reset all counters.
    pub fn reset_all(&mut self) {
        self.counts.clear();
    }

    /// Get the current count for an origin.
    pub fn count(&self, origin: &Origin) -> u32 {
        self.counts.get(origin).copied().unwrap_or(0)
    }
}

/// The popup policy — evaluates requests and returns decisions.
pub struct PopupPolicy {
    config: PopupPolicyConfig,
    tracker: PopupStormTracker,
}

impl PopupPolicy {
    pub fn new(config: PopupPolicyConfig) -> Self {
        Self {
            config,
            tracker: PopupStormTracker::new(),
        }
    }

    pub fn with_default_config() -> Self {
        Self::new(PopupPolicyConfig::default())
    }

    /// Evaluate a popup request and return a decision.
    ///
    /// The `current_tab_id` is used for `RouteToCurrentTab` decisions.
    /// The `new_tab_id` is the ID to assign if a new tab is approved.
    pub fn evaluate(
        &mut self,
        request: &PopupRequest,
        current_tab_id: Option<TabId>,
        new_tab_id: TabId,
    ) -> PopupDecision {
        // Validate target URL
        if request.target_url.is_empty() {
            return self.deny_or_route(PopupDenyReason::InvalidTargetUrl, current_tab_id);
        }

        // Check popup storm
        let count = self.tracker.record(&request.opener_origin);
        if count > self.config.max_popups_per_window {
            return self.deny_or_route(
                PopupDenyReason::PopupStorm {
                    count,
                    limit: self.config.max_popups_per_window,
                },
                current_tab_id,
            );
        }

        // Check user gesture
        let has_gesture = matches!(request.user_gesture, UserGesture::Yes);
        if !has_gesture {
            // Same-origin without gesture may be allowed by config
            let is_same_origin =
                request.opener_origin.as_str() == extract_origin(&request.target_url);
            if !is_same_origin || !self.config.allow_same_origin_without_gesture {
                return self.deny_or_route(
                    if is_same_origin {
                        PopupDenyReason::NoUserGesture
                    } else {
                        PopupDenyReason::CrossOriginWithoutGesture
                    },
                    current_tab_id,
                );
            }
        }

        // Allow: new tab
        PopupDecision::NewTab { tab_id: new_tab_id }
    }

    fn deny_or_route(
        &self,
        reason: PopupDenyReason,
        current_tab_id: Option<TabId>,
    ) -> PopupDecision {
        if self.config.route_denied_to_current_tab {
            if let Some(tab_id) = current_tab_id {
                return PopupDecision::RouteToCurrentTab { tab_id };
            }
        }
        PopupDecision::Denied { reason }
    }

    /// Reset the popup storm counter for a specific origin.
    pub fn reset_storm_count(&mut self, origin: &Origin) {
        self.tracker.reset(origin);
    }

    /// Get the current storm count for an origin.
    pub fn storm_count(&self, origin: &Origin) -> u32 {
        self.tracker.count(origin)
    }

    /// Get the policy configuration.
    pub fn config(&self) -> &PopupPolicyConfig {
        &self.config
    }
}

/// Extract the origin from a URL string (scheme://host[:port]).
/// This is a simple parser — the engine performs full URL parsing.
fn extract_origin(url: &str) -> &str {
    // Find scheme://
    if let Some(idx) = url.find("://") {
        let after_scheme = &url[idx + 3..];
        // Find the next / or end
        let end = after_scheme.find('/').unwrap_or(after_scheme.len());
        &url[..idx + 3 + end]
    } else {
        url
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn req(origin: &str, url: &str, gesture: UserGesture) -> PopupRequest {
        PopupRequest {
            opener_origin: Origin::new(origin),
            target_url: url.into(),
            user_gesture: gesture,
            opener_tab_id: Some(TabId::new("tab-1")),
        }
    }

    #[test]
    fn user_gesture_allows_new_tab() {
        let mut policy = PopupPolicy::with_default_config();
        let request = req(
            "https://example.com",
            "https://example.com/page",
            UserGesture::Yes,
        );
        let decision = policy.evaluate(&request, Some(TabId::new("tab-1")), TabId::new("tab-2"));
        assert_eq!(
            decision,
            PopupDecision::NewTab {
                tab_id: TabId::new("tab-2"),
            }
        );
    }

    #[test]
    fn no_gesture_cross_origin_denied_and_routed_to_current() {
        let mut policy = PopupPolicy::with_default_config();
        let request = req(
            "https://example.com",
            "https://evil.com/popup",
            UserGesture::No,
        );
        let decision = policy.evaluate(&request, Some(TabId::new("tab-1")), TabId::new("tab-2"));
        assert_eq!(
            decision,
            PopupDecision::RouteToCurrentTab {
                tab_id: TabId::new("tab-1"),
            }
        );
    }

    #[test]
    fn no_gesture_no_current_tab_denies() {
        let mut policy = PopupPolicy::with_default_config();
        let request = req(
            "https://example.com",
            "https://evil.com/popup",
            UserGesture::No,
        );
        let decision = policy.evaluate(&request, None, TabId::new("tab-2"));
        assert!(matches!(
            decision,
            PopupDecision::Denied {
                reason: PopupDenyReason::CrossOriginWithoutGesture
            }
        ));
    }

    #[test]
    fn popup_storm_denied() {
        let mut policy = PopupPolicy::new(PopupPolicyConfig {
            max_popups_per_window: 2,
            ..PopupPolicyConfig::default()
        });

        let origin = Origin::new("https://spam.com");
        for _ in 0..2 {
            let request = req("https://spam.com", "https://spam.com/ad", UserGesture::Yes);
            let decision =
                policy.evaluate(&request, Some(TabId::new("tab-1")), TabId::new("tab-new"));
            assert!(matches!(decision, PopupDecision::NewTab { .. }));
        }

        // Third one exceeds the limit
        let request = req("https://spam.com", "https://spam.com/ad", UserGesture::Yes);
        let decision = policy.evaluate(&request, Some(TabId::new("tab-1")), TabId::new("tab-new"));
        assert!(matches!(decision, PopupDecision::RouteToCurrentTab { .. }));
        assert_eq!(policy.storm_count(&origin), 3);
    }

    #[test]
    fn empty_url_denied() {
        let mut policy = PopupPolicy::with_default_config();
        let request = req("https://example.com", "", UserGesture::Yes);
        let decision = policy.evaluate(&request, Some(TabId::new("tab-1")), TabId::new("tab-2"));
        assert_eq!(
            decision,
            PopupDecision::RouteToCurrentTab {
                tab_id: TabId::new("tab-1"),
            }
        );
    }

    #[test]
    fn same_origin_no_gesture_denied_by_default() {
        let mut policy = PopupPolicy::with_default_config();
        let request = req(
            "https://example.com",
            "https://example.com/page",
            UserGesture::No,
        );
        let decision = policy.evaluate(&request, Some(TabId::new("tab-1")), TabId::new("tab-2"));
        assert_eq!(
            decision,
            PopupDecision::RouteToCurrentTab {
                tab_id: TabId::new("tab-1"),
            }
        );
    }

    #[test]
    fn same_origin_no_gesture_allowed_by_config() {
        let mut policy = PopupPolicy::new(PopupPolicyConfig {
            allow_same_origin_without_gesture: true,
            ..PopupPolicyConfig::default()
        });
        let request = req(
            "https://example.com",
            "https://example.com/page",
            UserGesture::No,
        );
        let decision = policy.evaluate(&request, Some(TabId::new("tab-1")), TabId::new("tab-2"));
        assert!(matches!(decision, PopupDecision::NewTab { .. }));
    }

    #[test]
    fn route_disabled_means_deny() {
        let mut policy = PopupPolicy::new(PopupPolicyConfig {
            route_denied_to_current_tab: false,
            ..PopupPolicyConfig::default()
        });
        let request = req(
            "https://example.com",
            "https://evil.com/popup",
            UserGesture::No,
        );
        let decision = policy.evaluate(&request, Some(TabId::new("tab-1")), TabId::new("tab-2"));
        assert!(matches!(
            decision,
            PopupDecision::Denied {
                reason: PopupDenyReason::CrossOriginWithoutGesture
            }
        ));
    }

    #[test]
    fn storm_tracker_reset() {
        let mut tracker = PopupStormTracker::new();
        let origin = Origin::new("https://a.com");
        assert_eq!(tracker.record(&origin), 1);
        assert_eq!(tracker.record(&origin), 2);
        tracker.reset(&origin);
        assert_eq!(tracker.count(&origin), 0);
    }

    #[test]
    fn extract_origin_works() {
        assert_eq!(
            extract_origin("https://example.com/path"),
            "https://example.com"
        );
        assert_eq!(
            extract_origin("https://example.com:8080/path?q=1"),
            "https://example.com:8080"
        );
        assert_eq!(extract_origin("about:blank"), "about:blank");
    }
}
