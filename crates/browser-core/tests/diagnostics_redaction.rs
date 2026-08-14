use browser_core::diagnostics::{
    redact_record, CrashBundle, RedactedRecord, RedactionConfig, TelemetryGate,
};

fn config(allowlisted: &[&str]) -> RedactionConfig {
    RedactionConfig::new(allowlisted.iter().map(|s| s.to_string()).collect())
}

// --- Golden redaction: URLs ---

#[test]
fn url_userinfo_redacted() {
    let record = redact_record(
        "url",
        "https://user:pass@example.com/x",
        &RedactionConfig::default(),
    )
    .expect("record");
    assert_eq!(record.value, "https://[redacted]@example.com/x");
}

#[test]
fn url_query_values_redacted_keys_kept() {
    let record = redact_record(
        "url",
        "https://example.com/?q=alpha&token=abc123",
        &RedactionConfig::default(),
    )
    .expect("record");
    assert_eq!(
        record.value,
        "https://example.com/?q=[redacted]&token=[redacted]"
    );
}

#[test]
fn url_fragment_redacted() {
    let record = redact_record(
        "url",
        "https://example.com/path#section",
        &RedactionConfig::default(),
    )
    .expect("record");
    assert_eq!(record.value, "https://example.com/path#[redacted]");
}

// --- Golden redaction: tokens and secrets ---

#[test]
fn bearer_token_redacted() {
    let record = redact_record(
        "header",
        "Authorization: Bearer abc123def456",
        &RedactionConfig::default(),
    )
    .expect("record");
    assert_eq!(record.value, "Authorization: [redacted]");
}

#[test]
fn password_and_keys_redacted() {
    for (field, value) in [
        ("param", "password=hunter2"),
        ("param", "api_key=secret123"),
        ("param", "apikey=secret123"),
        ("param", "token=abc"),
    ] {
        let record = redact_record(field, value, &RedactionConfig::default()).expect("record");
        assert!(record.value.contains("[redacted]"), "{field}={value}");
    }
}

#[test]
fn long_secret_like_string_redacted() {
    let secret = "0123456789abcdef".repeat(4);
    let record = redact_record("field", &secret, &RedactionConfig::default()).expect("record");
    assert_eq!(record.value, "[redacted]");
}

// --- Golden redaction: paths ---

#[test]
fn unix_home_path_redacted() {
    let record = redact_record(
        "path",
        "/Users/gabriel/Documents/secret.txt",
        &RedactionConfig::default(),
    )
    .expect("record");
    assert_eq!(record.value, "/Users/[user]/[redacted]");
}

#[test]
fn windows_home_path_redacted() {
    let record = redact_record(
        "path",
        "C:\\Users\\gabriel\\Downloads\\notes.txt",
        &RedactionConfig::default(),
    )
    .expect("record");
    assert_eq!(record.value, "C:\\Users\\[user]\\[redacted]");
}

// --- Golden redaction: page content ---

#[test]
fn page_content_redacted() {
    let html = "<html><body><script>document.cookie</script></body></html>";
    let record = redact_record("page_content", html, &RedactionConfig::default()).expect("record");
    assert_eq!(record.value, "[redacted]");
}

// --- Allowlist ---

#[test]
fn allowlisted_field_with_safe_value_kept() {
    let config = config(&["navigation.url"]);
    let record = redact_record("navigation.url", "https://example.com", &config).expect("record");
    assert_eq!(record.value, "https://example.com");
}

#[test]
fn allowlisted_field_still_redacts_secrets() {
    let config = config(&["navigation.url"]);
    let record =
        redact_record("navigation.url", "https://user:pass@example.com", &config).expect("record");
    assert_eq!(record.value, "https://[redacted]@example.com");
}

#[test]
fn non_allowlisted_field_value_redacted_entirely() {
    let record =
        redact_record("profile.ssn", "123-45-6789", &RedactionConfig::default()).expect("record");
    assert_eq!(record.value, "[redacted]");
}

// --- Telemetry opt-in ---

#[test]
fn telemetry_opt_out_collects_nothing() {
    let gate = TelemetryGate::new(false);
    assert!(gate.collect("url", "https://example.com").is_none());
}

#[test]
fn telemetry_opt_in_collects_redacted() {
    let gate = TelemetryGate::new(true);
    let Some(RedactedRecord { field, value }) = gate.collect("url", "https://a.com/path#x") else {
        panic!("expected collected record");
    };
    assert_eq!(field, "url");
    assert_eq!(value, "https://a.com/path#[redacted]");
}

// --- Crash bundle ---

#[test]
fn crash_bundle_local_json_is_redacted_and_has_no_cloud_fields() {
    let bundle = CrashBundle::new(vec![
        (
            "url".to_string(),
            "https://user:pass@example.com/#sec".to_string(),
        ),
        (
            "page_content".to_string(),
            "<html>private</html>".to_string(),
        ),
    ]);
    let json = bundle.to_local_json(&RedactionConfig::default());
    assert!(json.contains("https://[redacted]@example.com/#[redacted]"));
    assert!(json.contains("\"page_content\":\"[redacted]\""));
    assert!(!json.to_lowercase().contains("upload"));
    assert!(!json.to_lowercase().contains("cloud"));
    assert!(!json.contains("user:pass"));
}

// --- Structured logs ---

#[test]
fn structured_log_lines_redacted() {
    let config = RedactionConfig::default();
    let lines = [
        "nav url=https://user:pass@example.com/p",
        "header Authorization: Bearer tok123",
        "path=/Users/gabriel/x",
    ];
    let out: Vec<String> = lines
        .iter()
        .map(|l| redact_record("log", l, &config).expect("r").value)
        .collect();
    assert!(out[0].contains("https://[redacted]@example.com/p"));
    assert!(out[1].contains("Authorization: [redacted]"));
    assert!(out[2].contains("/Users/[user]/[redacted]"));
}

#[test]
fn empty_allowlist_means_deny_all_fields() {
    let record = redact_record("anything", "value", &RedactionConfig::default()).expect("record");
    assert_eq!(record.value, "[redacted]");
}
