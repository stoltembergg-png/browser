//! Navigation state machine — pure core logic for page navigation.
//!
//! Implements the navigation lifecycle: intent → started → committed/failed,
//! with history cursor (back/forward), generation tracking for stale event
//! filtering, cancel/reload semantics, and redirect handling.
//!
//! This module is pure logic: no network, no Servo, no I/O. The engine host
//! calls these methods in response to engine events and UI commands.

use crate::navigation_policy::NavigationPolicy;
use std::collections::VecDeque;

/// Navigation intent from the UI or engine.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum NavigationIntent {
    /// User navigated to a new URL.
    Navigate { url: String },
    /// User pressed reload.
    Reload,
    /// User pressed back.
    GoBack,
    /// User pressed forward.
    GoForward,
    /// User pressed stop.
    Stop,
}

/// The state of a single navigation attempt.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NavigationStatus {
    /// Navigation has been initiated but not committed.
    Pending,
    /// Navigation committed (first paint or URL confirmed).
    Committed,
    /// Navigation finished successfully.
    Finished,
    /// Navigation failed.
    Failed,
    /// Navigation was cancelled by the user or a new navigation.
    Cancelled,
    /// Navigation was redirected to a different URL.
    Redirected,
}

/// A single entry in the navigation history.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HistoryEntry {
    pub url: String,
    pub title: Option<String>,
}

/// The navigation state machine for a single tab.
#[derive(Debug)]
pub struct NavigationStateMachine {
    /// Current navigation generation. Incremented on each new navigation.
    /// Events with a lower generation are stale.
    current_generation: u32,

    /// Current navigation status.
    status: NavigationStatus,

    /// The URL being navigated to (if navigating).
    pending_url: Option<String>,

    /// History entries.
    history: VecDeque<HistoryEntry>,

    /// Current position in history (cursor). `None` means no history yet.
    cursor: Option<usize>,

    /// Optional navigation policy re-evaluating redirects.
    policy: Option<NavigationPolicy>,
}

impl NavigationStateMachine {
    pub fn new() -> Self {
        Self {
            current_generation: 0,
            status: NavigationStatus::Finished,
            pending_url: None,
            history: VecDeque::new(),
            cursor: None,
            policy: None,
        }
    }

    pub fn new_with_policy(policy: NavigationPolicy) -> Self {
        Self {
            current_generation: 0,
            status: NavigationStatus::Finished,
            pending_url: None,
            history: VecDeque::new(),
            cursor: None,
            policy: Some(policy),
        }
    }

    pub fn current_generation(&self) -> u32 {
        self.current_generation
    }

    pub fn status(&self) -> NavigationStatus {
        self.status
    }

    pub fn pending_url(&self) -> Option<&str> {
        self.pending_url.as_deref()
    }

    pub fn current_url(&self) -> Option<&str> {
        self.cursor
            .and_then(|i| self.history.get(i))
            .map(|e| e.url.as_str())
    }

    pub fn current_title(&self) -> Option<&str> {
        self.cursor
            .and_then(|i| self.history.get(i))
            .and_then(|e| e.title.as_deref())
    }

    /// Can we go back in history?
    pub fn can_go_back(&self) -> bool {
        self.cursor.is_some_and(|c| c > 0)
    }

    /// Can we go forward in history?
    pub fn can_go_forward(&self) -> bool {
        self.cursor.is_some_and(|c| c + 1 < self.history.len())
    }

    /// Start a new navigation. Cancels any pending navigation.
    ///
    /// Returns the new generation for tracking events.
    pub fn navigate(&mut self, url: String) -> u32 {
        // Cancel any pending navigation
        if self.status == NavigationStatus::Pending {
            self.status = NavigationStatus::Cancelled;
        }

        // Truncate history forward entries (we're branching from current)
        if let Some(c) = self.cursor {
            self.history.truncate(c + 1);
        }

        // Increment generation
        self.current_generation += 1;
        self.status = NavigationStatus::Pending;
        self.pending_url = Some(url);

        self.current_generation
    }

    /// Commit the current navigation (URL confirmed / first paint).
    pub fn commit(&mut self, url: String, generation: u32) -> bool {
        if generation != self.current_generation {
            return false; // Stale event
        }
        if self.status != NavigationStatus::Pending {
            return false; // Can only commit pending navigation
        }

        self.status = NavigationStatus::Committed;

        // Add to history
        let entry = HistoryEntry { url, title: None };
        self.history.push_back(entry);
        self.cursor = Some(self.history.len() - 1);
        self.pending_url = None;

        true
    }

    /// Finish the current navigation.
    pub fn finish(&mut self, generation: u32) -> bool {
        if generation != self.current_generation {
            return false; // Stale event
        }
        if self.status != NavigationStatus::Committed {
            return false;
        }
        self.status = NavigationStatus::Finished;
        true
    }

    /// Fail the current navigation.
    pub fn fail(&mut self, generation: u32) -> bool {
        if generation != self.current_generation {
            return false;
        }
        if self.status != NavigationStatus::Pending && self.status != NavigationStatus::Committed {
            return false;
        }
        self.status = NavigationStatus::Failed;
        self.pending_url = None;
        true
    }

    /// Cancel the current navigation (user pressed Stop or a new navigation started).
    pub fn cancel(&mut self, generation: u32) -> bool {
        if generation != self.current_generation {
            return false;
        }
        if self.status != NavigationStatus::Pending {
            return false;
        }
        self.status = NavigationStatus::Cancelled;
        self.pending_url = None;
        true
    }

    /// Handle a redirect: the navigation committed but to a different URL.
    ///
    /// When a policy is configured, the redirect target is re-evaluated:
    /// scheme changes into disallowed schemes or https downgrades are
    /// refused without changing state.
    pub fn redirect(&mut self, new_url: String, generation: u32) -> bool {
        if generation != self.current_generation {
            return false;
        }
        if self.status != NavigationStatus::Pending && self.status != NavigationStatus::Committed {
            return false;
        }
        if let (Some(policy), Some(current_url)) = (&self.policy, self.pending_url()) {
            if policy.evaluate_redirect(current_url, &new_url).is_err() {
                return false;
            }
        }
        self.status = NavigationStatus::Redirected;
        // Update pending URL to the redirect target
        self.pending_url = Some(new_url);
        // A redirect is followed by a new commit to the redirected URL
        true
    }

    /// Update the title of the current history entry.
    pub fn set_title(&mut self, title: String) -> bool {
        if let Some(c) = self.cursor {
            if let Some(entry) = self.history.get_mut(c) {
                entry.title = Some(title);
                return true;
            }
        }
        false
    }

    /// Go back in history. Returns the URL to navigate to, if possible.
    pub fn go_back(&mut self) -> Option<String> {
        if !self.can_go_back() {
            return None;
        }
        self.cursor = Some(self.cursor.unwrap() - 1);
        let url = self.history.get(self.cursor.unwrap()).unwrap().url.clone();
        self.current_generation += 1;
        self.status = NavigationStatus::Pending;
        self.pending_url = Some(url.clone());
        Some(url)
    }

    /// Go forward in history. Returns the URL to navigate to, if possible.
    pub fn go_forward(&mut self) -> Option<String> {
        if !self.can_go_forward() {
            return None;
        }
        self.cursor = Some(self.cursor.unwrap() + 1);
        let url = self.history.get(self.cursor.unwrap()).unwrap().url.clone();
        self.current_generation += 1;
        self.status = NavigationStatus::Pending;
        self.pending_url = Some(url.clone());
        Some(url)
    }

    /// Reload the current page. Returns the URL to reload, if there is one.
    pub fn reload(&mut self) -> Option<String> {
        let url = self.current_url()?.to_string();
        self.current_generation += 1;
        self.status = NavigationStatus::Pending;
        self.pending_url = Some(url.clone());
        Some(url)
    }

    /// Check if an event with the given generation is stale.
    pub fn is_stale(&self, generation: u32) -> bool {
        generation < self.current_generation
    }
}

impl Default for NavigationStateMachine {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn navigate_and_commit(nav: &mut NavigationStateMachine, url: &str) -> u32 {
        let gen = nav.navigate(url.to_string());
        assert!(nav.commit(url.to_string(), gen));
        assert!(nav.finish(gen));
        gen
    }

    // --- Basic navigate ---

    #[test]
    fn navigate_starts_pending() {
        let mut nav = NavigationStateMachine::new();
        let gen = nav.navigate("https://example.com".to_string());
        assert_eq!(nav.status(), NavigationStatus::Pending);
        assert_eq!(gen, 1);
        assert_eq!(nav.pending_url(), Some("https://example.com"));
    }

    #[test]
    fn navigate_then_commit() {
        let mut nav = NavigationStateMachine::new();
        let gen = nav.navigate("https://example.com".to_string());
        assert!(nav.commit("https://example.com".to_string(), gen));
        assert_eq!(nav.status(), NavigationStatus::Committed);
        assert_eq!(nav.current_url(), Some("https://example.com"));
    }

    #[test]
    fn navigate_commit_finish_full_cycle() {
        let mut nav = NavigationStateMachine::new();
        navigate_and_commit(&mut nav, "https://example.com");
        assert_eq!(nav.status(), NavigationStatus::Finished);
    }

    // --- Stale events ---

    #[test]
    fn stale_commit_rejected() {
        let mut nav = NavigationStateMachine::new();
        let gen1 = nav.navigate("https://a.com".to_string());
        let gen2 = nav.navigate("https://b.com".to_string()); // Increments generation
        assert_ne!(gen1, gen2);
        // Commit with old generation → stale
        assert!(!nav.commit("https://a.com".to_string(), gen1));
        // Commit with new generation → OK
        assert!(nav.commit("https://b.com".to_string(), gen2));
    }

    #[test]
    fn is_stale_check() {
        let mut nav = NavigationStateMachine::new();
        let gen1 = nav.navigate("https://a.com".to_string());
        assert!(!nav.is_stale(gen1));
        let gen2 = nav.navigate("https://b.com".to_string());
        assert!(nav.is_stale(gen1));
        assert!(!nav.is_stale(gen2));
    }

    // --- Cancel ---

    #[test]
    fn cancel_pending_navigation() {
        let mut nav = NavigationStateMachine::new();
        let gen = nav.navigate("https://example.com".to_string());
        assert!(nav.cancel(gen));
        assert_eq!(nav.status(), NavigationStatus::Cancelled);
        assert_eq!(nav.pending_url(), None);
    }

    #[test]
    fn cancel_after_commit_rejected() {
        let mut nav = NavigationStateMachine::new();
        let gen = nav.navigate("https://example.com".to_string());
        assert!(nav.commit("https://example.com".to_string(), gen));
        assert!(!nav.cancel(gen)); // Can't cancel a committed nav
    }

    // --- Fail ---

    #[test]
    fn fail_pending_navigation() {
        let mut nav = NavigationStateMachine::new();
        let gen = nav.navigate("https://fail.com".to_string());
        assert!(nav.fail(gen));
        assert_eq!(nav.status(), NavigationStatus::Failed);
    }

    #[test]
    fn fail_committed_navigation() {
        let mut nav = NavigationStateMachine::new();
        let gen = nav.navigate("https://example.com".to_string());
        assert!(nav.commit("https://example.com".to_string(), gen));
        assert!(nav.fail(gen)); // Can fail even after commit
        assert_eq!(nav.status(), NavigationStatus::Failed);
    }

    // --- Redirect ---

    #[test]
    fn redirect_updates_pending_url() {
        let mut nav = NavigationStateMachine::new();
        let gen = nav.navigate("https://old.com".to_string());
        assert!(nav.redirect("https://new.com".to_string(), gen));
        assert_eq!(nav.status(), NavigationStatus::Redirected);
        assert_eq!(nav.pending_url(), Some("https://new.com"));
    }

    #[test]
    fn redirect_stale_rejected() {
        let mut nav = NavigationStateMachine::new();
        let gen1 = nav.navigate("https://old.com".to_string());
        let _gen2 = nav.navigate("https://other.com".to_string());
        assert!(!nav.redirect("https://new.com".to_string(), gen1));
    }

    #[test]
    fn redirect_with_policy_rejects_downgrade_without_state_change() {
        let mut nav = NavigationStateMachine::new_with_policy(NavigationPolicy::default());
        let gen = nav.navigate("https://old.com".to_string());
        assert!(!nav.redirect("http://new.com".to_string(), gen));
        assert_eq!(nav.status(), NavigationStatus::Pending);
        assert_eq!(nav.pending_url(), Some("https://old.com"));
    }

    #[test]
    fn redirect_with_policy_rejects_disallowed_scheme() {
        let mut nav = NavigationStateMachine::new_with_policy(NavigationPolicy::default());
        let gen = nav.navigate("https://old.com".to_string());
        assert!(!nav.redirect("javascript:alert(1)".to_string(), gen));
        assert_eq!(nav.status(), NavigationStatus::Pending);
    }

    #[test]
    fn redirect_with_policy_allows_safe_target() {
        let mut nav = NavigationStateMachine::new_with_policy(NavigationPolicy::default());
        let gen = nav.navigate("https://old.com".to_string());
        assert!(nav.redirect("https://new.com".to_string(), gen));
        assert_eq!(nav.status(), NavigationStatus::Redirected);
        assert_eq!(nav.pending_url(), Some("https://new.com"));
    }

    // --- History cursor ---

    #[test]
    fn history_back_and_forward() {
        let mut nav = NavigationStateMachine::new();
        navigate_and_commit(&mut nav, "https://a.com");
        navigate_and_commit(&mut nav, "https://b.com");
        navigate_and_commit(&mut nav, "https://c.com");

        assert_eq!(nav.current_url(), Some("https://c.com"));
        assert!(nav.can_go_back());
        assert!(!nav.can_go_forward());

        let url = nav.go_back().unwrap();
        assert_eq!(url, "https://b.com");
        assert_eq!(nav.current_url(), Some("https://b.com"));
        assert!(nav.can_go_back());
        assert!(nav.can_go_forward());

        let url = nav.go_back().unwrap();
        assert_eq!(url, "https://a.com");
        assert!(!nav.can_go_back());

        let url = nav.go_forward().unwrap();
        assert_eq!(url, "https://b.com");
    }

    #[test]
    fn history_truncates_on_navigate() {
        let mut nav = NavigationStateMachine::new();
        navigate_and_commit(&mut nav, "https://a.com");
        navigate_and_commit(&mut nav, "https://b.com");
        navigate_and_commit(&mut nav, "https://c.com");

        nav.go_back(); // At b.com
        nav.go_back(); // At a.com
                       // Navigate from a.com — should truncate forward entries
        let gen = nav.navigate("https://d.com".to_string());
        assert!(nav.commit("https://d.com".to_string(), gen));
        assert!(nav.finish(gen));

        assert_eq!(nav.current_url(), Some("https://d.com"));
        assert!(!nav.can_go_forward());
        assert!(nav.can_go_back());
    }

    #[test]
    fn cannot_go_back_at_start() {
        let nav = NavigationStateMachine::new();
        assert!(!nav.can_go_back());
        assert!(!nav.can_go_forward());
    }

    // --- Reload ---

    #[test]
    fn reload_current_page() {
        let mut nav = NavigationStateMachine::new();
        navigate_and_commit(&mut nav, "https://example.com");
        let url = nav.reload().unwrap();
        assert_eq!(url, "https://example.com");
        assert_eq!(nav.status(), NavigationStatus::Pending);
    }

    #[test]
    fn reload_without_history_returns_none() {
        let mut nav = NavigationStateMachine::new();
        assert!(nav.reload().is_none());
    }

    // --- New navigation cancels pending ---

    #[test]
    fn new_navigation_cancels_pending() {
        let mut nav = NavigationStateMachine::new();
        let gen1 = nav.navigate("https://a.com".to_string());
        assert_eq!(nav.status(), NavigationStatus::Pending);
        let gen2 = nav.navigate("https://b.com".to_string());
        // First navigation should have been cancelled
        assert_eq!(nav.status(), NavigationStatus::Pending);
        assert_ne!(gen1, gen2);
    }

    // --- Title ---

    #[test]
    fn set_title_updates_current_entry() {
        let mut nav = NavigationStateMachine::new();
        navigate_and_commit(&mut nav, "https://example.com");
        assert!(nav.set_title("Example".to_string()));
        assert_eq!(nav.current_title(), Some("Example"));
    }

    #[test]
    fn set_title_without_history_fails() {
        let mut nav = NavigationStateMachine::new();
        assert!(!nav.set_title("Test".to_string()));
    }

    // --- Generation increments ---

    #[test]
    fn generation_increments_on_each_navigation() {
        let mut nav = NavigationStateMachine::new();
        assert_eq!(nav.current_generation(), 0);
        nav.navigate("https://a.com".to_string());
        assert_eq!(nav.current_generation(), 1);
        nav.navigate("https://b.com".to_string());
        assert_eq!(nav.current_generation(), 2);
    }

    #[test]
    fn go_back_increments_generation() {
        let mut nav = NavigationStateMachine::new();
        navigate_and_commit(&mut nav, "https://a.com");
        navigate_and_commit(&mut nav, "https://b.com");
        let gen_before = nav.current_generation();
        nav.go_back();
        assert_eq!(nav.current_generation(), gen_before + 1);
    }
}
