use browser_core::navigation_policy::{
    ExternalAction, ExternalProtocolPolicy, FileAccess, FileAccessPolicy, NavigationAction,
    NavigationPolicy, PolicyError, Scheme,
};
use std::path::PathBuf;

fn temp_root(label: &str) -> PathBuf {
    let nonce = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .expect("clock")
        .as_nanos();
    std::env::temp_dir().join(format!("pr044-{label}-{nonce}"))
}

// --- Parser ---

#[test]
fn parses_https_with_host_and_port() {
    let parsed =
        browser_core::navigation_policy::parse_url("https://example.com:8080/path?q=1#frag")
            .expect("parse");
    assert_eq!(parsed.scheme, Scheme::Https);
    assert_eq!(parsed.host.as_deref(), Some("example.com"));
    assert_eq!(parsed.port, Some(8080));
}

#[test]
fn scheme_is_case_insensitive() {
    let parsed = browser_core::navigation_policy::parse_url("HTTPS://EXAMPLE.COM").expect("parse");
    assert_eq!(parsed.scheme, Scheme::Https);
    assert_eq!(parsed.host.as_deref(), Some("EXAMPLE.COM"));
}

#[test]
fn parses_http_and_about() {
    assert_eq!(
        browser_core::navigation_policy::parse_url("http://a.b")
            .expect("parse")
            .scheme,
        Scheme::Http
    );
    assert_eq!(
        browser_core::navigation_policy::parse_url("about:blank")
            .expect("parse")
            .scheme,
        Scheme::About
    );
}

#[test]
fn rejects_missing_scheme() {
    assert!(matches!(
        browser_core::navigation_policy::parse_url("example.com/path"),
        Err(PolicyError::InvalidScheme)
    ));
}

#[test]
fn rejects_invalid_scheme_chars() {
    assert!(matches!(
        browser_core::navigation_policy::parse_url("ht*tp://a"),
        Err(PolicyError::InvalidScheme)
    ));
}

#[test]
fn rejects_missing_authority() {
    assert!(matches!(
        browser_core::navigation_policy::parse_url("https://"),
        Err(PolicyError::MissingAuthority)
    ));
    assert!(matches!(
        browser_core::navigation_policy::parse_url("https:///path"),
        Err(PolicyError::MissingAuthority)
    ));
}

#[test]
fn rejects_control_chars_and_spaces() {
    for url in [
        "https://a.com/\r\nX-Injected: 1",
        "https://a .com",
        "https://a.com/\0tail",
    ] {
        assert!(
            browser_core::navigation_policy::parse_url(url).is_err(),
            "expected rejection for {url:?}"
        );
    }
}

#[test]
fn rejects_userinfo_in_http_urls() {
    assert!(matches!(
        browser_core::navigation_policy::parse_url("https://user:pass@example.com"),
        Err(PolicyError::UserInfoNotAllowed)
    ));
}

#[test]
fn rejects_invalid_ports() {
    for url in [
        "https://a.com:0",
        "https://a.com:70000",
        "https://a.com:abc",
    ] {
        assert!(
            browser_core::navigation_policy::parse_url(url).is_err(),
            "expected rejection for {url:?}"
        );
    }
}

// --- Navigation policy (scheme allowlist) ---

#[test]
fn default_policy_allows_https_http_about() {
    let policy = NavigationPolicy::default();
    for url in ["https://example.com", "http://example.com", "about:blank"] {
        assert_eq!(
            policy.classify(url),
            NavigationAction::Allow,
            "expected allow for {url:?}"
        );
    }
}

#[test]
fn default_policy_denies_data_javascript_file() {
    let policy = NavigationPolicy::default();
    for url in [
        "data:text/html,<b>x</b>",
        "javascript:alert(1)",
        "file:///etc/passwd",
    ] {
        assert_eq!(
            policy.classify(url),
            NavigationAction::Deny,
            "expected deny for {url:?}"
        );
    }
}

#[test]
fn default_policy_confirms_external_protocols() {
    assert_eq!(
        NavigationPolicy::default().classify("mailto:user@example.com"),
        NavigationAction::Confirm
    );
}

#[test]
fn classify_rejects_malformed_urls() {
    assert_eq!(
        NavigationPolicy::default().classify("https://a com"),
        NavigationAction::Deny
    );
}

// --- Redirect re-evaluation ---

#[test]
fn redirect_https_to_https_allowed() {
    let policy = NavigationPolicy::default();
    assert!(policy
        .evaluate_redirect("https://old.com", "https://new.com")
        .is_ok());
}

#[test]
fn redirect_https_to_http_denied_as_downgrade() {
    let policy = NavigationPolicy::default();
    assert!(matches!(
        policy.evaluate_redirect("https://old.com", "http://new.com"),
        Err(PolicyError::HttpsDowngrade)
    ));
}

#[test]
fn redirect_http_to_https_allowed() {
    let policy = NavigationPolicy::default();
    assert!(policy
        .evaluate_redirect("http://old.com", "https://new.com")
        .is_ok());
}

#[test]
fn redirect_into_denied_scheme_is_denied() {
    let policy = NavigationPolicy::default();
    assert!(policy
        .evaluate_redirect("https://old.com", "javascript:alert(1)")
        .is_err());
    assert!(policy
        .evaluate_redirect("https://old.com", "data:text/html,x")
        .is_err());
}

#[test]
fn redirect_into_https_is_allowed_from_any_allowed_scheme() {
    let policy = NavigationPolicy::default();
    assert!(policy
        .evaluate_redirect("about:blank", "https://new.com")
        .is_ok());
}

// --- External protocol policy ---

#[test]
fn allowlisted_handler_with_clean_args_opens() {
    let policy = ExternalProtocolPolicy::new(["mailto".to_string()].into_iter().collect(), false);
    assert_eq!(
        policy.evaluate("mailto", "user@example.com?subject=hello"),
        ExternalAction::OpenAllowed
    );
}

#[test]
fn allowlisted_handler_with_shell_metachar_denied() {
    let policy = ExternalProtocolPolicy::new(["mailto".to_string()].into_iter().collect(), false);
    assert_eq!(
        policy.evaluate("mailto", "user@example.com; rm -rf /"),
        ExternalAction::Denied
    );
}

#[test]
fn handler_not_allowlisted_requires_confirmation() {
    let policy = ExternalProtocolPolicy::new(std::collections::HashSet::new(), true);
    assert_eq!(
        policy.evaluate("slack", "channels"),
        ExternalAction::RequiresConfirmation
    );
}

#[test]
fn confirmation_off_and_not_allowlisted_denied() {
    let policy = ExternalProtocolPolicy::new(std::collections::HashSet::new(), false);
    assert_eq!(policy.evaluate("slack", "channels"), ExternalAction::Denied);
}

#[test]
fn handler_with_metachar_denied() {
    let policy = ExternalProtocolPolicy::new(["mailto".to_string()].into_iter().collect(), true);
    assert_eq!(policy.evaluate("mail;to", "x"), ExternalAction::Denied);
}

// --- File access policy ---

#[test]
fn file_without_configured_root_denied() {
    assert!(matches!(
        FileAccessPolicy::default().resolve("index.html"),
        FileAccess::Denied(_)
    ));
}

#[test]
fn file_within_root_allowed() {
    let root = temp_root("within");
    let policy = FileAccessPolicy::new(root.clone());
    match policy.resolve("sub/index.html") {
        FileAccess::Allowed(path) => {
            assert_eq!(path, root.join("sub").join("index.html"));
        }
        other => panic!("expected allowed, got {other:?}"),
    }
}

#[test]
fn file_traversal_denied() {
    let root = temp_root("traversal");
    let policy = FileAccessPolicy::new(root.clone());
    for path in ["../secret", "a/../../secret", "..", "../"] {
        assert!(
            matches!(policy.resolve(path), FileAccess::Denied(_)),
            "expected denial for {path:?}"
        );
    }
    assert!(!root.exists());
}

#[test]
fn file_percent_encoded_traversal_denied() {
    let root = temp_root("encoded");
    let policy = FileAccessPolicy::new(root.clone());
    assert!(matches!(
        policy.resolve("..%2Fsecret"),
        FileAccess::Denied(_)
    ));
    assert!(matches!(
        policy.resolve("%2e%2e/secret"),
        FileAccess::Denied(_)
    ));
}

#[test]
fn file_null_byte_denied() {
    let root = temp_root("null");
    let policy = FileAccessPolicy::new(root);
    assert!(matches!(policy.resolve("x%00y"), FileAccess::Denied(_)));
}

#[test]
fn file_without_scheme_is_not_an_error_path() {
    let root = temp_root("raw");
    let policy = FileAccessPolicy::new(root.clone());
    assert!(matches!(
        policy.resolve("plain.txt"),
        FileAccess::Allowed(_)
    ));
}

#[test]
fn parses_bracketed_ipv6_authority_and_port() {
    let parsed = browser_core::navigation_policy::parse_url("https://[2001:db8::1]:8443/path")
        .expect("parse");
    assert_eq!(parsed.scheme, Scheme::Https);
    assert_eq!(parsed.host.as_deref(), Some("[2001:db8::1]"));
    assert_eq!(parsed.port, Some(8443));
}

#[test]
fn rejects_unbracketed_ipv6_authority() {
    assert!(matches!(
        browser_core::navigation_policy::parse_url("https://2001:db8::1/path"),
        Err(PolicyError::InvalidHost)
    ));
}

#[test]
fn rejects_malformed_host_labels() {
    for url in [
        "https://-example.com",
        "https://example-.com",
        "https://example..com",
    ] {
        assert!(matches!(
            browser_core::navigation_policy::parse_url(url),
            Err(PolicyError::InvalidHost)
        ));
    }
}

#[test]
fn file_windows_separators_and_drive_prefixes_are_denied() {
    let root = temp_root("windows-path");
    let policy = FileAccessPolicy::new(root);
    for path in [
        "..\\\\secret",
        "nested\\\\..\\\\secret",
        "C:%2Fsecret",
        "x%5Cy",
    ] {
        assert!(
            matches!(policy.resolve(path), FileAccess::Denied(_)),
            "expected denial for {path:?}"
        );
    }
}
