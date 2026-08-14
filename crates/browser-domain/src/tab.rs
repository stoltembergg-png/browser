//! Pure tab domain state: identity, lifecycle, visibility, and focus.

use crate::ids::{EngineInstanceId, NavigationId, ProfileId, TabId, TabTitle, Url};
use serde::{Deserialize, Serialize};
use std::fmt;

/// Lifecycle states owned by the browser core for one tab.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum TabLifecycle {
    Created,
    Loading,
    Ready,
    Failed,
    Crashed,
    Closing,
    Closed,
}

impl TabLifecycle {
    /// Returns whether the domain state machine permits the edge.
    pub fn can_transition_to(self, next: Self) -> bool {
        matches!(
            (self, next),
            (Self::Created, Self::Loading)
                | (Self::Created, Self::Closing)
                | (Self::Loading, Self::Ready)
                | (Self::Loading, Self::Failed)
                | (Self::Loading, Self::Crashed)
                | (Self::Loading, Self::Closing)
                | (Self::Ready, Self::Loading)
                | (Self::Ready, Self::Crashed)
                | (Self::Ready, Self::Closing)
                | (Self::Failed, Self::Loading)
                | (Self::Failed, Self::Closing)
                | (Self::Crashed, Self::Closing)
                | (Self::Closing, Self::Closed)
        )
    }
}

/// Whether a tab currently occupies a visible browser surface.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum TabVisibility {
    Hidden,
    Visible,
}

/// Whether a visible tab owns browser focus.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum TabFocus {
    Unfocused,
    Focused,
}

/// Domain errors for illegal tab mutations or stale navigation events.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TabError {
    InvalidTransition {
        from: TabLifecycle,
        to: TabLifecycle,
    },
    TerminalState {
        lifecycle: TabLifecycle,
    },
    NavigationMismatch {
        expected: Option<NavigationId>,
        actual: NavigationId,
    },
    FocusRequiresVisible,
}

impl fmt::Display for TabError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidTransition { from, to } => {
                write!(formatter, "illegal tab transition: {from:?} -> {to:?}")
            }
            Self::TerminalState { lifecycle } => {
                write!(formatter, "tab is not mutable in {lifecycle:?} state")
            }
            Self::NavigationMismatch { expected, actual } => {
                write!(
                    formatter,
                    "stale navigation event: expected {expected:?}, got {actual}"
                )
            }
            Self::FocusRequiresVisible => write!(formatter, "focus requires a visible tab"),
        }
    }
}

impl std::error::Error for TabError {}

/// Browser-core-owned state for one tab.
///
/// The structure contains only domain identifiers and value objects. Engine
/// handles, Servo types, surfaces, and UI presentation state do not cross this
/// boundary.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Tab {
    tab_id: TabId,
    profile_id: ProfileId,
    engine_instance_id: EngineInstanceId,
    lifecycle: TabLifecycle,
    visibility: TabVisibility,
    focus: TabFocus,
    current_url: Option<Url>,
    pending_url: Option<Url>,
    active_navigation: Option<NavigationId>,
    title: TabTitle,
}

impl Tab {
    pub fn new(tab_id: TabId, profile_id: ProfileId, engine_instance_id: EngineInstanceId) -> Self {
        Self {
            tab_id,
            profile_id,
            engine_instance_id,
            lifecycle: TabLifecycle::Created,
            visibility: TabVisibility::Hidden,
            focus: TabFocus::Unfocused,
            current_url: None,
            pending_url: None,
            active_navigation: None,
            title: TabTitle::new("").expect("empty tab title is a valid domain value"),
        }
    }

    pub fn tab_id(&self) -> &TabId {
        &self.tab_id
    }

    pub fn profile_id(&self) -> &ProfileId {
        &self.profile_id
    }

    pub fn engine_instance_id(&self) -> &EngineInstanceId {
        &self.engine_instance_id
    }

    pub fn lifecycle(&self) -> TabLifecycle {
        self.lifecycle
    }

    pub fn visibility(&self) -> TabVisibility {
        self.visibility
    }

    pub fn focus_state(&self) -> TabFocus {
        self.focus
    }

    pub fn current_url(&self) -> Option<&Url> {
        self.current_url.as_ref()
    }

    pub fn active_navigation(&self) -> Option<&NavigationId> {
        self.active_navigation.as_ref()
    }

    pub fn title(&self) -> &TabTitle {
        &self.title
    }

    pub fn show(&mut self) -> Result<(), TabError> {
        self.ensure_mutable()?;
        self.visibility = TabVisibility::Visible;
        Ok(())
    }

    pub fn hide(&mut self) -> Result<(), TabError> {
        self.ensure_mutable()?;
        self.visibility = TabVisibility::Hidden;
        self.focus = TabFocus::Unfocused;
        Ok(())
    }

    pub fn focus(&mut self) -> Result<(), TabError> {
        self.ensure_mutable()?;
        if self.visibility != TabVisibility::Visible {
            return Err(TabError::FocusRequiresVisible);
        }
        self.focus = TabFocus::Focused;
        Ok(())
    }

    pub fn blur(&mut self) -> Result<(), TabError> {
        self.ensure_mutable()?;
        self.focus = TabFocus::Unfocused;
        Ok(())
    }

    pub fn set_title(&mut self, title: TabTitle) -> Result<(), TabError> {
        self.ensure_mutable()?;
        self.title = title;
        Ok(())
    }

    pub fn start_navigation(
        &mut self,
        navigation_id: NavigationId,
        url: Url,
    ) -> Result<(), TabError> {
        self.ensure_mutable()?;
        self.transition_to(TabLifecycle::Loading)?;
        self.active_navigation = Some(navigation_id);
        self.pending_url = Some(url);
        Ok(())
    }

    pub fn commit_navigation(
        &mut self,
        navigation_id: &NavigationId,
        url: Url,
    ) -> Result<(), TabError> {
        self.ensure_current_navigation(navigation_id)?;
        self.transition_to(TabLifecycle::Ready)?;
        self.current_url = Some(url);
        self.pending_url = None;
        self.active_navigation = None;
        Ok(())
    }

    pub fn fail_navigation(&mut self, navigation_id: &NavigationId) -> Result<(), TabError> {
        self.ensure_current_navigation(navigation_id)?;
        self.transition_to(TabLifecycle::Failed)?;
        self.pending_url = None;
        self.active_navigation = None;
        Ok(())
    }

    pub fn crash(&mut self) -> Result<(), TabError> {
        self.ensure_mutable()?;
        self.transition_to(TabLifecycle::Crashed)?;
        self.pending_url = None;
        self.active_navigation = None;
        self.focus = TabFocus::Unfocused;
        Ok(())
    }

    pub fn begin_close(&mut self) -> Result<(), TabError> {
        self.ensure_mutable()?;
        self.transition_to(TabLifecycle::Closing)?;
        self.pending_url = None;
        self.active_navigation = None;
        self.focus = TabFocus::Unfocused;
        Ok(())
    }

    pub fn complete_close(&mut self) -> Result<(), TabError> {
        self.transition_to(TabLifecycle::Closed)
    }

    fn ensure_current_navigation(&self, actual: &NavigationId) -> Result<(), TabError> {
        if self.active_navigation.as_ref() != Some(actual) {
            return Err(TabError::NavigationMismatch {
                expected: self.active_navigation.clone(),
                actual: actual.clone(),
            });
        }
        Ok(())
    }

    fn ensure_mutable(&self) -> Result<(), TabError> {
        match self.lifecycle {
            TabLifecycle::Closing | TabLifecycle::Closed => Err(TabError::TerminalState {
                lifecycle: self.lifecycle,
            }),
            _ => Ok(()),
        }
    }

    fn transition_to(&mut self, next: TabLifecycle) -> Result<(), TabError> {
        if self.lifecycle == TabLifecycle::Closed
            || (self.lifecycle == TabLifecycle::Closing && next != TabLifecycle::Closed)
        {
            return Err(TabError::TerminalState {
                lifecycle: self.lifecycle,
            });
        }
        if !self.lifecycle.can_transition_to(next) {
            return Err(TabError::InvalidTransition {
                from: self.lifecycle,
                to: next,
            });
        }
        self.lifecycle = next;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ids::{EngineInstanceId, NavigationId, ProfileId, TabId, TabTitle, Url};

    fn tab() -> Tab {
        Tab::new(
            TabId::new("tab-1"),
            ProfileId::new("profile-default"),
            EngineInstanceId::new("engine-1"),
        )
    }

    #[test]
    fn new_tab_has_stable_identity_and_safe_defaults() {
        let tab = tab();
        assert_eq!(tab.tab_id(), &TabId::new("tab-1"));
        assert_eq!(tab.profile_id(), &ProfileId::new("profile-default"));
        assert_eq!(tab.engine_instance_id(), &EngineInstanceId::new("engine-1"));
        assert_eq!(tab.lifecycle(), TabLifecycle::Created);
        assert_eq!(tab.visibility(), TabVisibility::Hidden);
        assert_eq!(tab.focus_state(), TabFocus::Unfocused);
        assert_eq!(tab.current_url(), None);
        assert_eq!(
            tab.title(),
            &TabTitle::new("").expect("empty title is valid")
        );
    }

    #[test]
    fn lifecycle_matrix_allows_only_declared_edges() {
        let states = [
            TabLifecycle::Created,
            TabLifecycle::Loading,
            TabLifecycle::Ready,
            TabLifecycle::Failed,
            TabLifecycle::Crashed,
            TabLifecycle::Closing,
            TabLifecycle::Closed,
        ];
        let allowed = [
            (TabLifecycle::Created, TabLifecycle::Loading),
            (TabLifecycle::Created, TabLifecycle::Closing),
            (TabLifecycle::Loading, TabLifecycle::Ready),
            (TabLifecycle::Loading, TabLifecycle::Failed),
            (TabLifecycle::Loading, TabLifecycle::Crashed),
            (TabLifecycle::Loading, TabLifecycle::Closing),
            (TabLifecycle::Ready, TabLifecycle::Loading),
            (TabLifecycle::Ready, TabLifecycle::Crashed),
            (TabLifecycle::Ready, TabLifecycle::Closing),
            (TabLifecycle::Failed, TabLifecycle::Loading),
            (TabLifecycle::Failed, TabLifecycle::Closing),
            (TabLifecycle::Crashed, TabLifecycle::Closing),
            (TabLifecycle::Closing, TabLifecycle::Closed),
        ];

        for from in states {
            for to in states {
                assert_eq!(
                    allowed.contains(&(from, to)),
                    from.can_transition_to(to),
                    "unexpected lifecycle edge {from:?} -> {to:?}"
                );
            }
        }
    }

    #[test]
    fn navigation_commit_requires_current_navigation_and_moves_to_ready() {
        let mut tab = tab();
        tab.start_navigation(
            NavigationId::new("nav-1"),
            Url::new("https://example.test/one").expect("valid URL"),
        )
        .expect("created tab can navigate");
        assert_eq!(tab.lifecycle(), TabLifecycle::Loading);
        assert_eq!(tab.active_navigation(), Some(&NavigationId::new("nav-1")));

        let stale = tab.commit_navigation(
            &NavigationId::new("nav-old"),
            Url::new("https://example.test/old").expect("valid URL"),
        );
        assert!(matches!(stale, Err(TabError::NavigationMismatch { .. })));
        assert_eq!(tab.lifecycle(), TabLifecycle::Loading);

        tab.commit_navigation(
            &NavigationId::new("nav-1"),
            Url::new("https://example.test/one").expect("valid URL"),
        )
        .expect("current navigation commits");
        assert_eq!(tab.lifecycle(), TabLifecycle::Ready);
        assert_eq!(
            tab.current_url(),
            Some(&Url::new("https://example.test/one").unwrap())
        );
        assert_eq!(tab.active_navigation(), None);
    }

    #[test]
    fn failed_navigation_keeps_last_committed_url_and_clears_pending_state() {
        let mut tab = tab();
        tab.show().expect("show");
        tab.start_navigation(
            NavigationId::new("nav-1"),
            Url::new("https://example.test/one").unwrap(),
        )
        .unwrap();
        tab.commit_navigation(
            &NavigationId::new("nav-1"),
            Url::new("https://example.test/one").unwrap(),
        )
        .unwrap();
        tab.start_navigation(
            NavigationId::new("nav-2"),
            Url::new("https://example.test/two").unwrap(),
        )
        .unwrap();
        tab.fail_navigation(&NavigationId::new("nav-2"))
            .expect("current navigation can fail");

        assert_eq!(tab.lifecycle(), TabLifecycle::Failed);
        assert_eq!(
            tab.current_url(),
            Some(&Url::new("https://example.test/one").unwrap())
        );
        assert_eq!(tab.active_navigation(), None);
    }

    #[test]
    fn visibility_and_focus_are_explicit_and_focus_requires_visibility() {
        let mut tab = tab();
        assert!(matches!(tab.focus(), Err(TabError::FocusRequiresVisible)));
        tab.show().expect("created tab can be shown");
        tab.focus().expect("visible tab can be focused");
        assert_eq!(tab.visibility(), TabVisibility::Visible);
        assert_eq!(tab.focus_state(), TabFocus::Focused);
        tab.hide().expect("visible tab can be hidden");
        assert_eq!(tab.visibility(), TabVisibility::Hidden);
        assert_eq!(tab.focus_state(), TabFocus::Unfocused);
    }

    #[test]
    fn crash_clears_in_flight_navigation_but_close_remains_explicit() {
        let mut tab = tab();
        tab.start_navigation(
            NavigationId::new("nav-1"),
            Url::new("https://example.test/one").unwrap(),
        )
        .unwrap();
        tab.crash().expect("loading tab can crash");
        assert_eq!(tab.lifecycle(), TabLifecycle::Crashed);
        assert_eq!(tab.active_navigation(), None);
        assert!(tab.begin_close().is_ok());
        assert_eq!(tab.lifecycle(), TabLifecycle::Closing);
        tab.complete_close().expect("closing tab can close");
        assert_eq!(tab.lifecycle(), TabLifecycle::Closed);
    }

    #[test]
    fn closed_tab_rejects_all_mutations() {
        let mut tab = tab();
        tab.begin_close().unwrap();
        tab.complete_close().unwrap();
        assert!(matches!(tab.show(), Err(TabError::TerminalState { .. })));
        assert!(matches!(tab.focus(), Err(TabError::TerminalState { .. })));
        assert!(matches!(
            tab.start_navigation(
                NavigationId::new("nav-1"),
                Url::new("https://example.test").unwrap()
            ),
            Err(TabError::TerminalState { .. })
        ));
        assert!(matches!(
            tab.set_title(TabTitle::new("closed").unwrap()),
            Err(TabError::TerminalState { .. })
        ));
    }

    #[test]
    fn tab_state_roundtrips_without_engine_types() {
        let mut tab = tab();
        tab.show().unwrap();
        tab.focus().unwrap();
        tab.set_title(TabTitle::new("Example").unwrap()).unwrap();
        tab.start_navigation(
            NavigationId::new("nav-1"),
            Url::new("https://example.test").unwrap(),
        )
        .unwrap();
        let json = serde_json::to_string(&tab).expect("serialize tab");
        let restored: Tab = serde_json::from_str(&json).expect("deserialize tab");
        assert_eq!(tab, restored);
        assert!(!json.contains("servo"));
    }
}
