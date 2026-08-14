use browser_domain::ui::{NavigationChromeState, UiEvent};

#[test]
fn chrome_state_tracks_loading_error_cancel_and_controls() {
    let mut state = NavigationChromeState::new();
    assert!(!state.can_reload());
    assert!(!state.can_stop());

    assert!(state.apply_event(&UiEvent::NavigationStarted {
        url: "https://example.com".to_string(),
    }));
    assert_eq!(state.url(), Some("https://example.com"));
    assert!(state.is_loading());
    assert!(state.can_stop());
    assert!(!state.can_reload());
    assert_eq!(state.error(), None);

    assert!(state.apply_event(&UiEvent::NavigationFailed {
        reason: "TLS failure".to_string(),
    }));
    assert!(!state.is_loading());
    assert!(state.can_reload());
    assert!(!state.can_stop());
    assert_eq!(state.error(), Some("TLS failure"));

    assert!(state.apply_event(&UiEvent::NavigationStarted {
        url: "https://example.com/retry".to_string(),
    }));
    assert!(state.apply_event(&UiEvent::NavigationCancelled));
    assert!(!state.is_loading());
    assert_eq!(state.error(), None);
}

#[test]
fn chrome_state_tracks_history_and_ignores_unrelated_events() {
    let mut state = NavigationChromeState::new();
    assert!(!state.apply_event(&UiEvent::TitleChanged {
        title: "ignored before navigation".to_string(),
    }));

    state.set_history_capabilities(true, false);
    assert!(state.can_go_back());
    assert!(!state.can_go_forward());
    assert!(state.apply_event(&UiEvent::NavigationCommitted {
        url: "https://example.com".to_string(),
    }));
    assert_eq!(state.url(), Some("https://example.com"));
    assert!(state.apply_event(&UiEvent::TitleChanged {
        title: "Example".to_string(),
    }));
    assert_eq!(state.title(), Some("Example"));

    assert!(state.apply_event(&UiEvent::CommandRejected {
        reason: "stale navigation".to_string(),
    }));
    assert_eq!(state.error(), Some("stale navigation"));
}
