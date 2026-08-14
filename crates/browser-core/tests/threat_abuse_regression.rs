//! Threat/abuse regression suite — negative tests for THREAT_MODEL.md scenarios.
//!
//! Each test proves that a dangerous input is blocked. If any fail, a threat
//! model scenario is unguarded and the release gate must block.
//!
//! Lives in browser-core/tests because it needs both browser-domain types
//! (scheme_security, history, bookmarks, downloads, privacy, diagnostics,
//! permissions, prompts) and browser-core types (popup_policy).

// ── Scheme injection ──────────────────────────────────────────────

#[test]
fn threat_file_scheme_blocked() {
    let broker = browser_domain::scheme_security::SchemeBroker::new();
    let d = broker.evaluate("file:///etc/passwd");
    assert!(
        matches!(
            d,
            browser_domain::scheme_security::SchemeDecision::Deny {
                reason: browser_domain::scheme_security::DenyReason::DeniedScheme { .. }
            }
        ),
        "file scheme must be blocked: {d:?}"
    );
}

#[test]
fn threat_data_scheme_blocked() {
    let broker = browser_domain::scheme_security::SchemeBroker::new();
    let d = broker.evaluate("data:text/html,<script>steal()</script>");
    assert!(matches!(
        d,
        browser_domain::scheme_security::SchemeDecision::Deny { .. }
    ));
}

#[test]
fn threat_javascript_scheme_blocked() {
    let broker = browser_domain::scheme_security::SchemeBroker::new();
    let d = broker.evaluate("javascript:alert(document.cookies)");
    assert!(matches!(
        d,
        browser_domain::scheme_security::SchemeDecision::Deny { .. }
    ));
}

#[test]
fn threat_blob_scheme_blocked() {
    let broker = browser_domain::scheme_security::SchemeBroker::new();
    let d = broker.evaluate("blob:https://evil.com/uuid");
    assert!(matches!(
        d,
        browser_domain::scheme_security::SchemeDecision::Deny { .. }
    ));
}

#[test]
fn threat_unknown_scheme_blocked() {
    let broker = browser_domain::scheme_security::SchemeBroker::new();
    let d = broker.evaluate("custom-scheme://payload");
    assert!(matches!(
        d,
        browser_domain::scheme_security::SchemeDecision::Deny { .. }
    ));
}

// ── Path traversal ────────────────────────────────────────────────

#[test]
fn threat_path_traversal_blocked() {
    let broker = browser_domain::scheme_security::SchemeBroker::new();
    let d = broker.evaluate("https://evil.com/../../../etc/shadow");
    assert!(matches!(
        d,
        browser_domain::scheme_security::SchemeDecision::Deny {
            reason: browser_domain::scheme_security::DenyReason::PathTraversal
        }
    ));
}

#[test]
fn threat_path_traversal_in_redirect_blocked() {
    let broker = browser_domain::scheme_security::SchemeBroker::new();
    let d = broker.evaluate_redirect("https://a.com", "https://b.com/../../etc/passwd");
    assert!(matches!(
        d,
        browser_domain::scheme_security::SchemeDecision::Deny { .. }
    ));
}

// ── Credential-in-URL ─────────────────────────────────────────────

#[test]
fn threat_credential_url_blocked() {
    let broker = browser_domain::scheme_security::SchemeBroker::new();
    let d = broker.evaluate("https://user:password@evil.com/steal");
    assert!(matches!(
        d,
        browser_domain::scheme_security::SchemeDecision::Deny {
            reason: browser_domain::scheme_security::DenyReason::CredentialInUrl
        }
    ));
}

// ── Redirect abuse ────────────────────────────────────────────────

#[test]
fn threat_redirect_to_file_blocked() {
    let broker = browser_domain::scheme_security::SchemeBroker::new();
    let d = broker.evaluate_redirect("https://a.com", "file:///etc/passwd");
    assert!(matches!(
        d,
        browser_domain::scheme_security::SchemeDecision::Deny { .. }
    ));
}

#[test]
fn threat_redirect_to_javascript_blocked() {
    let broker = browser_domain::scheme_security::SchemeBroker::new();
    let d = broker.evaluate_redirect("https://a.com", "javascript:alert(1)");
    assert!(matches!(
        d,
        browser_domain::scheme_security::SchemeDecision::Deny { .. }
    ));
}

#[test]
fn threat_redirect_from_denied_original_blocked() {
    let broker = browser_domain::scheme_security::SchemeBroker::new();
    let d = broker.evaluate_redirect("file:///etc/passwd", "https://safe.com");
    assert!(matches!(
        d,
        browser_domain::scheme_security::SchemeDecision::Deny { .. }
    ));
}

// ── Popup storm ───────────────────────────────────────────────────

#[test]
fn threat_popup_storm_denied() {
    use browser_core::popup_policy::*;
    use browser_domain::ids::TabId;

    let mut policy = PopupPolicy::new(PopupPolicyConfig {
        max_popups_per_window: 2,
        ..Default::default()
    });

    let origin = Origin::new("https://spam.com");
    for _ in 0..2 {
        let req = PopupRequest {
            opener_origin: origin.clone(),
            target_url: "https://spam.com/ad".into(),
            user_gesture: UserGesture::Yes,
            opener_tab_id: Some(TabId::new("tab-1")),
        };
        assert!(matches!(
            policy.evaluate(&req, Some(TabId::new("tab-1")), TabId::new("tab-new")),
            PopupDecision::NewTab { .. }
        ));
    }

    let req = PopupRequest {
        opener_origin: origin,
        target_url: "https://spam.com/ad".into(),
        user_gesture: UserGesture::Yes,
        opener_tab_id: Some(TabId::new("tab-1")),
    };
    assert!(matches!(
        policy.evaluate(&req, Some(TabId::new("tab-1")), TabId::new("tab-new")),
        PopupDecision::RouteToCurrentTab { .. }
    ));
}

#[test]
fn threat_cross_origin_popup_without_gesture_denied() {
    use browser_core::popup_policy::*;
    use browser_domain::ids::TabId;

    let mut policy = PopupPolicy::with_default_config();
    let req = PopupRequest {
        opener_origin: Origin::new("https://safe.com"),
        target_url: "https://evil.com/popup".into(),
        user_gesture: UserGesture::No,
        opener_tab_id: Some(TabId::new("tab-1")),
    };
    assert!(matches!(
        policy.evaluate(&req, Some(TabId::new("tab-1")), TabId::new("tab-2")),
        PopupDecision::RouteToCurrentTab { .. }
    ));
}

#[test]
fn threat_popup_no_gesture_no_current_tab_denied() {
    use browser_core::popup_policy::*;
    use browser_domain::ids::TabId;

    let mut policy = PopupPolicy::with_default_config();
    let req = PopupRequest {
        opener_origin: Origin::new("https://safe.com"),
        target_url: "https://evil.com/popup".into(),
        user_gesture: UserGesture::No,
        opener_tab_id: Some(TabId::new("tab-1")),
    };
    assert!(matches!(
        policy.evaluate(&req, None, TabId::new("tab-2")),
        PopupDecision::Denied {
            reason: PopupDenyReason::CrossOriginWithoutGesture
        }
    ));
}

#[test]
fn threat_empty_popup_url_denied() {
    use browser_core::popup_policy::*;
    use browser_domain::ids::TabId;

    let mut policy = PopupPolicy::with_default_config();
    let req = PopupRequest {
        opener_origin: Origin::new("https://safe.com"),
        target_url: "".into(),
        user_gesture: UserGesture::Yes,
        opener_tab_id: Some(TabId::new("tab-1")),
    };
    assert!(matches!(
        policy.evaluate(&req, Some(TabId::new("tab-1")), TabId::new("tab-2")),
        PopupDecision::RouteToCurrentTab { .. }
    ));
}

// ── Diagnostics PII leak prevention ────────────────────────────────

#[test]
fn threat_diagnostics_no_raw_page_content() {
    let redactor = browser_domain::diagnostics::DiagnosticsRedactor::new();
    let raw = vec![
        (
            "page_content".to_string(),
            "<html><body>secret</body></html>".to_string(),
        ),
        ("engine_state".to_string(), "crashed".to_string()),
    ];
    let record = redactor
        .build_record(
            browser_domain::diagnostics::DiagnosticsKind::Crash,
            &raw,
            1000,
        )
        .unwrap();
    let json = serde_json::to_string(&record).unwrap();
    assert!(!json.contains("secret"));
    assert!(!json.contains("<html>"));
    assert!(json.contains("[REDACTED]"));
}

#[test]
fn threat_diagnostics_no_credentials() {
    let redactor = browser_domain::diagnostics::DiagnosticsRedactor::new();
    let raw = vec![
        ("api_key".to_string(), "sk-1234567890abcdef".to_string()),
        ("password".to_string(), "hunter2".to_string()),
        ("engine_state".to_string(), "ready".to_string()),
    ];
    let record = redactor
        .build_record(
            browser_domain::diagnostics::DiagnosticsKind::Info,
            &raw,
            1000,
        )
        .unwrap();
    let json = serde_json::to_string(&record).unwrap();
    assert!(!json.contains("sk-1234567890abcdef"));
    assert!(!json.contains("hunter2"));
}

#[test]
fn threat_diagnostics_bearer_token_redacted() {
    let redactor = browser_domain::diagnostics::DiagnosticsRedactor::new();
    let result = redactor.redact_value("navigation_state", "Bearer super-secret-token");
    assert_eq!(result, "Bearer [REDACTED]");
}

#[test]
fn threat_diagnostics_query_params_redacted() {
    let redactor = browser_domain::diagnostics::DiagnosticsRedactor::new();
    let result = redactor.redact_value(
        "navigation_state",
        "https://api.com/page?token=secret&keep=safe",
    );
    assert!(result.contains("token=[REDACTED]"));
    assert!(result.contains("keep=safe"));
}

#[test]
fn threat_diagnostics_file_path_redacted() {
    let redactor = browser_domain::diagnostics::DiagnosticsRedactor::new();
    let result = redactor.redact_value("checkpoint_committed", "/home/user/.profile/secret.db");
    assert!(result.contains("[REDACTED]"));
    assert!(!result.contains("user"));
}

// ── Privacy clearing completeness ────────────────────────────────

#[test]
fn threat_privacy_clear_all_leaves_no_residual() {
    use browser_domain::bookmarks::*;
    use browser_domain::download_ui::*;
    use browser_domain::history::*;
    use browser_domain::privacy::*;

    let mut history = InMemoryHistoryRepository::new();
    let mut downloads = InMemoryDownloadRepository::new();
    let mut bookmarks = InMemoryBookmarkRepository::new();

    let profile = browser_domain::ids::ProfileId::new("victim");
    let now = 1000u64;

    let mut hist_mgr = HistoryManager::new(history, NavigationCommitPolicy::OnCommit);
    hist_mgr
        .record_visit(&profile, "https://tracking.com/ad", "Ad", now)
        .unwrap();
    history = hist_mgr.into_repository();

    downloads
        .save_record(
            &profile,
            &DownloadRecord::new_pending(
                DownloadId::new("dl-1"),
                "https://malware.com/exe".into(),
                "malware.exe".into(),
                Some(1000),
                now,
            )
            .unwrap(),
        )
        .unwrap();

    bookmarks
        .add(
            &profile,
            Bookmark {
                id: "bm-1".into(),
                url: "https://phishing.com".into(),
                title: "Phish".into(),
                created_at: now,
                updated_at: now,
            },
        )
        .unwrap();

    let mut manager = PrivacyManager::new(history, downloads, bookmarks);
    let result = manager.clear_all(&profile).unwrap();
    assert!(result.total_cleared() > 0);
    assert!(manager.data_counts(&profile).is_empty());
}

#[test]
fn threat_privacy_profiles_isolated_no_cross_leak() {
    use browser_domain::history::*;
    use browser_domain::privacy::*;

    let mut history = InMemoryHistoryRepository::new();
    let downloads = browser_domain::download_ui::InMemoryDownloadRepository::new();
    let bookmarks = browser_domain::bookmarks::InMemoryBookmarkRepository::new();

    let victim = browser_domain::ids::ProfileId::new("victim");
    let attacker = browser_domain::ids::ProfileId::new("attacker");

    let mut hist_mgr = HistoryManager::new(history, NavigationCommitPolicy::OnCommit);
    hist_mgr
        .record_visit(&victim, "https://bank.com/login", "Bank", 1000)
        .unwrap();
    hist_mgr
        .record_visit(&attacker, "https://evil.com/steal", "Evil", 2000)
        .unwrap();
    history = hist_mgr.into_repository();

    let mut manager = PrivacyManager::new(history, downloads, bookmarks);
    manager.clear_history(&victim).unwrap();
    assert_eq!(manager.data_counts(&attacker).history_entries, 1);
    assert_eq!(manager.data_counts(&victim).history_entries, 0);
}

// ── History validation ─────────────────────────────────────────────

#[test]
fn threat_empty_history_url_rejected() {
    use browser_domain::history::*;

    let repo = InMemoryHistoryRepository::new();
    let mut manager = HistoryManager::new(repo, NavigationCommitPolicy::OnCommit);
    let profile = browser_domain::ids::ProfileId::new("p1");
    assert!(matches!(
        manager.record_visit(&profile, "", "T", 1000),
        Err(HistoryError::InvalidUrl { .. })
    ));
}

// ── Bookmark validation ────────────────────────────────────────────

#[test]
fn threat_empty_bookmark_url_rejected() {
    use browser_domain::bookmarks::*;

    let repo = InMemoryBookmarkRepository::new();
    let mut manager = BookmarkManager::new(repo);
    let profile = browser_domain::ids::ProfileId::new("p1");
    assert!(matches!(
        manager.add(&profile, "", "Title", 1000),
        Err(BookmarkError::InvalidUrl { .. })
    ));
}

// ── Download state enforcement ────────────────────────────────────

#[test]
fn threat_cancel_terminal_download_rejected() {
    use browser_domain::download_ui::*;
    use browser_domain::ids::ProfileId;

    let repo = InMemoryDownloadRepository::new();
    let mut manager = DownloadUiManager::new(repo);
    let profile = ProfileId::new("p1");

    manager
        .create_download(
            &profile,
            DownloadId::new("dl-1"),
            "https://e.com/f.zip",
            "f.zip",
            Some(100),
            1000,
        )
        .unwrap();
    manager
        .complete_download(&profile, &DownloadId::new("dl-1"), 1001)
        .unwrap();

    assert!(matches!(
        manager.cancel_download(&profile, &DownloadId::new("dl-1"), 1002),
        Err(DownloadUiError::InvalidStateTransition { .. })
    ));
}

#[test]
fn threat_download_empty_url_rejected() {
    use browser_domain::download_ui::*;
    use browser_domain::ids::ProfileId;

    let repo = InMemoryDownloadRepository::new();
    let mut manager = DownloadUiManager::new(repo);
    let profile = ProfileId::new("p1");

    assert!(matches!(
        manager.create_download(&profile, DownloadId::new("dl-1"), "", "f.zip", None, 1000),
        Err(DownloadUiError::InvalidUrl)
    ));
}

#[test]
fn threat_download_empty_filename_rejected() {
    use browser_domain::download_ui::*;
    use browser_domain::ids::ProfileId;

    let repo = InMemoryDownloadRepository::new();
    let mut manager = DownloadUiManager::new(repo);
    let profile = ProfileId::new("p1");

    assert!(matches!(
        manager.create_download(
            &profile,
            DownloadId::new("dl-1"),
            "https://e.com",
            "",
            None,
            1000
        ),
        Err(DownloadUiError::InvalidFilename { .. })
    ));
}

// ── Permission default deny ────────────────────────────────────────

#[test]
fn threat_permission_default_deny() {
    use browser_domain::ids::{ProfileId, TabId};
    use browser_domain::permissions::*;

    let mut store = PermissionStore::new();
    let origin = Origin::new("https://evil.com").unwrap();

    let req = PermissionRequest::new(
        PermissionKind::Geolocation,
        origin.clone(),
        origin,
        None,
        ProfileId::new("victim"),
        TabId::new("tab-1"),
        false,
        1000,
    );

    assert!(matches!(
        store.decide(&req),
        PermissionDecision::Denied {
            reason: DenyReason::DefaultDeny
        }
    ));
}

#[test]
fn threat_permission_no_implicit_grant() {
    use browser_domain::ids::{ProfileId, TabId};
    use browser_domain::permissions::*;

    let mut store = PermissionStore::new();

    let req = PermissionRequest::new(
        PermissionKind::Microphone,
        Origin::new("https://evil.com").unwrap(),
        Origin::new("https://evil.com").unwrap(),
        None,
        ProfileId::new("victim"),
        TabId::new("tab-1"),
        false,
        1000,
    );

    assert!(matches!(
        store.decide(&req),
        PermissionDecision::Denied { .. }
    ));

    // Second request also denied — no implicit grant
    let req2 = PermissionRequest::new(
        PermissionKind::Microphone,
        Origin::new("https://evil.com").unwrap(),
        Origin::new("https://evil.com").unwrap(),
        None,
        ProfileId::new("victim"),
        TabId::new("tab-1"),
        false,
        2000,
    );
    assert!(matches!(
        store.decide(&req2),
        PermissionDecision::Denied {
            reason: DenyReason::DefaultDeny
        }
    ));
}

// ── Malformed input ────────────────────────────────────────────────

#[test]
fn threat_malformed_url_blocked() {
    let broker = browser_domain::scheme_security::SchemeBroker::new();
    let d = broker.evaluate("not a url at all");
    assert!(matches!(
        d,
        browser_domain::scheme_security::SchemeDecision::Deny { .. }
    ));
}

#[test]
fn threat_no_scheme_separator_blocked() {
    let broker = browser_domain::scheme_security::SchemeBroker::new();
    let d = broker.evaluate("just-words-no-colon");
    assert!(matches!(
        d,
        browser_domain::scheme_security::SchemeDecision::Deny { .. }
    ));
}

// ── Safe path doesn't trigger false positive ───────────────────────

#[test]
fn safe_url_with_at_in_query_not_blocked_as_credential() {
    let broker = browser_domain::scheme_security::SchemeBroker::new();
    let d = broker.evaluate("https://example.com/page?email=user@service.com");
    assert_eq!(d, browser_domain::scheme_security::SchemeDecision::Allow);
}

#[test]
fn safe_url_with_double_slash_in_path_not_blocked() {
    let broker = browser_domain::scheme_security::SchemeBroker::new();
    let d = broker.evaluate("https://example.com/path//to//page");
    assert_eq!(d, browser_domain::scheme_security::SchemeDecision::Allow);
}
