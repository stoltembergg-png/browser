//! Diagnostics redaction — structured logs with PII/secret scrubbing.
//!
//! Provides a diagnostics field allowlist and redaction rules for URLs,
//! tokens, file paths, and page content. Pure domain logic — no I/O,
//! no telemetry, no cloud collection. Output is safe for local crash
//! bundles and structured logs.

use serde::{Deserialize, Serialize};
use std::collections::HashSet;
use std::fmt;

/// The redaction marker applied to fields that fail the allowlist or
/// match a redaction pattern. Matches the `[REDACTED]` convention
/// already used by crash_recovery.
pub const REDACTED_MARKER: &str = "[REDACTED]";

/// Maximum size of a single diagnostics field value.
pub const MAX_FIELD_VALUE_LEN: usize = 4096;

/// Maximum number of fields in a single diagnostics record.
pub const MAX_FIELDS_PER_RECORD: usize = 64;

/// Sensitivity classification for a diagnostics field.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FieldSensitivity {
    /// Safe to include verbatim — no redaction needed.
    Public,
    /// Must be redacted — contains URL, token, path, or page content.
    Sensitive,
}

/// A field allowlist entry — defines which fields are safe to emit.
#[derive(Debug, Clone)]
pub struct AllowlistEntry {
    pub name: String,
    pub sensitivity: FieldSensitivity,
}

/// A structured diagnostics record with redacted fields.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DiagnosticsRecord {
    /// The schema version.
    pub version: u32,
    /// The kind of diagnostics event (crash, hang, error, info).
    pub kind: DiagnosticsKind,
    /// Redacted fields safe for output.
    pub fields: Vec<(String, String)>,
    /// When the record was created (Unix epoch seconds).
    pub timestamp: u64,
}

/// The kind of diagnostics event.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum DiagnosticsKind {
    Crash,
    Hang,
    Error,
    Info,
}

/// Errors from diagnostics operations.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DiagnosticsError {
    TooManyFields {
        count: usize,
        max: usize,
    },
    FieldValueTooLong {
        name: String,
        len: usize,
        max: usize,
    },
}

impl fmt::Display for DiagnosticsError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::TooManyFields { count, max } => {
                write!(f, "too many diagnostics fields: {count} > {max}")
            }
            Self::FieldValueTooLong { name, len, max } => {
                write!(f, "field '{name}' value too long: {len} > {max}")
            }
        }
    }
}

impl std::error::Error for DiagnosticsError {}

/// The diagnostics redactor — owns the allowlist and redaction rules.
pub struct DiagnosticsRedactor {
    allowlist: HashSet<String>,
}

impl DiagnosticsRedactor {
    /// Create a new redactor with the default allowlist.
    pub fn new() -> Self {
        let allowlist = default_allowlist();
        Self { allowlist }
    }

    /// Create a redactor with a custom allowlist.
    pub fn with_allowlist(allowlist: HashSet<String>) -> Self {
        Self { allowlist }
    }

    /// Check if a field name is in the allowlist.
    pub fn is_allowed(&self, name: &str) -> bool {
        self.allowlist.contains(name)
    }

    /// Redact a single field value based on its name and content.
    pub fn redact_value(&self, name: &str, value: &str) -> String {
        if !self.is_allowed(name) {
            return REDACTED_MARKER.into();
        }

        // Even allowed fields get pattern-based redaction
        redact_patterns(value)
    }

    /// Build a diagnostics record from raw fields, applying redaction.
    pub fn build_record(
        &self,
        kind: DiagnosticsKind,
        raw_fields: &[(String, String)],
        timestamp: u64,
    ) -> Result<DiagnosticsRecord, DiagnosticsError> {
        if raw_fields.len() > MAX_FIELDS_PER_RECORD {
            return Err(DiagnosticsError::TooManyFields {
                count: raw_fields.len(),
                max: MAX_FIELDS_PER_RECORD,
            });
        }

        let mut redacted = Vec::with_capacity(raw_fields.len());
        for (name, value) in raw_fields {
            if value.len() > MAX_FIELD_VALUE_LEN {
                return Err(DiagnosticsError::FieldValueTooLong {
                    name: name.clone(),
                    len: value.len(),
                    max: MAX_FIELD_VALUE_LEN,
                });
            }
            let redacted_value = self.redact_value(name, value);
            redacted.push((name.clone(), redacted_value));
        }

        Ok(DiagnosticsRecord {
            version: 1,
            kind,
            fields: redacted,
            timestamp,
        })
    }
}

impl Default for DiagnosticsRedactor {
    fn default() -> Self {
        Self::new()
    }
}

/// Patterns that are always redacted even in allowlisted fields.
/// Redacts:
/// - Bearer tokens (`Bearer xxx`)
/// - Query string parameters (`?token=`, `?key=`, `?password=`, `?secret=`)
/// - File paths (`/home/`, `/Users/`, `C:\Users\`)
/// - Tab IDs in URLs (kept as `tab-XXX` for diagnostics, but full IDs in
///   non-allowlisted fields are redacted by the allowlist itself)
fn redact_patterns(value: &str) -> String {
    let mut result = value.to_string();

    // Redact Bearer tokens
    if result.starts_with("Bearer ") {
        return "Bearer [REDACTED]".into();
    }

    // Redact sensitive query parameters in URLs
    let sensitive_params = ["token", "key", "password", "secret", "auth", "api_key"];
    if let Some(q_idx) = result.find('?') {
        let (before_q, after_q) = result.split_at(q_idx + 1);
        let mut redacted_params = Vec::new();
        for param in after_q.split('&') {
            let param_lower = param.to_lowercase();
            let should_redact = sensitive_params
                .iter()
                .any(|s| param_lower.starts_with(&format!("{s}=")));
            if should_redact {
                if let Some(eq_idx) = param.find('=') {
                    redacted_params.push(format!("{}={REDACTED_MARKER}", &param[..eq_idx]));
                } else {
                    redacted_params.push(REDACTED_MARKER.into());
                }
            } else {
                redacted_params.push(param.into());
            }
        }
        result = format!("{}{}", before_q, redacted_params.join("&"));
    }

    // Redact file paths containing user home directories
    let home_prefixes = ["/home/", "/Users/", "C:\\Users\\"];
    for prefix in &home_prefixes {
        if result.contains(prefix) {
            // Replace the path with a redacted version
            if let Some(start) = result.find(prefix) {
                let end = result[start..]
                    .find([' ', '\n', '\t'])
                    .map(|e| start + e)
                    .unwrap_or(result.len());
                let path = &result[start..end];
                let redacted_path = redact_path(path);
                result = format!("{}{redacted_path}{}", &result[..start], &result[end..]);
            }
        }
    }

    result
}

/// Redact a file path to show only the structure, not user data.
fn redact_path(path: &str) -> String {
    // Keep the prefix marker and count of segments, but redact user-specific names
    let prefix = if path.starts_with("/home/") {
        "/home/[REDACTED]"
    } else if path.starts_with("/Users/") {
        "/Users/[REDACTED]"
    } else if path.starts_with("C:\\Users\\") {
        "C:\\Users\\[REDACTED]"
    } else {
        return REDACTED_MARKER.into();
    };

    // Count remaining path segments for diagnostics
    let rest = &path[prefix.len() - 10..]; // skip past the known prefix
    let seg_count = rest.matches('/').count() + rest.matches('\\').count();
    if seg_count > 0 {
        format!("{prefix}/...({seg_count} segments)")
    } else {
        prefix.into()
    }
}

/// The default field allowlist — fields safe to emit in diagnostics.
fn default_allowlist() -> HashSet<String> {
    let entries = [
        // Engine state
        "engine_state",
        "engine_instance_id",
        "tab_id",
        // Lifecycle
        "lifecycle_state",
        "restart_attempt",
        "max_attempts",
        // Navigation
        "navigation_state",
        "navigation_generation",
        // Crash recovery
        "epoch",
        "checkpoint_committed",
        "watchdog_state",
        // Timing
        "duration_ms",
        "timestamp",
        // Error classification (no raw error text)
        "error_kind",
        // Download state
        "download_state",
        "bytes_received",
        "total_bytes",
        // Popup policy
        "popup_decision",
        // Profile
        "profile_id",
    ];

    entries.iter().map(|s| s.to_string()).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn allowed_field_passes_through() {
        let redactor = DiagnosticsRedactor::new();
        let result = redactor.redact_value("engine_state", "ready");
        assert_eq!(result, "ready");
    }

    #[test]
    fn non_allowlisted_field_is_redacted() {
        let redactor = DiagnosticsRedactor::new();
        let result = redactor.redact_value("raw_page_content", "<html>secret</html>");
        assert_eq!(result, REDACTED_MARKER);
    }

    #[test]
    fn bearer_token_is_redacted() {
        let redactor = DiagnosticsRedactor::new();
        let result = redactor.redact_value("navigation_state", "Bearer abc123secret");
        assert_eq!(result, "Bearer [REDACTED]");
    }

    #[test]
    fn sensitive_query_params_are_redacted() {
        let redactor = DiagnosticsRedactor::new();
        let result = redactor.redact_value(
            "navigation_state",
            "https://example.com/page?token=secret&keep=this",
        );
        assert!(result.contains("token=[REDACTED]"));
        assert!(result.contains("keep=this"));
    }

    #[test]
    fn multiple_sensitive_params() {
        let redactor = DiagnosticsRedactor::new();
        let result = redactor.redact_value(
            "navigation_state",
            "https://x.com?p=1&password=secret&key=abc&q=2",
        );
        assert!(result.contains("password=[REDACTED]"));
        assert!(result.contains("key=[REDACTED]"));
        assert!(result.contains("p=1"));
        assert!(result.contains("q=2"));
    }

    #[test]
    fn url_without_params_is_not_modified() {
        let redactor = DiagnosticsRedactor::new();
        let result = redactor.redact_value("navigation_state", "https://example.com/page");
        assert_eq!(result, "https://example.com/page");
    }

    #[test]
    fn file_path_with_home_is_redacted() {
        let redactor = DiagnosticsRedactor::new();
        let result = redactor.redact_value("checkpoint_committed", "/home/user/.profile/data.db");
        assert!(result.contains("[REDACTED]"));
        assert!(!result.contains("user"));
    }

    #[test]
    fn build_record_redacts_non_allowlisted() {
        let redactor = DiagnosticsRedactor::new();
        let raw = vec![
            ("engine_state".into(), "ready".into()),
            ("raw_url".into(), "https://secret.com".into()),
            ("error_kind".into(), "navigation_failed".into()),
        ];

        let record = redactor
            .build_record(DiagnosticsKind::Crash, &raw, 1000)
            .unwrap();

        assert_eq!(record.kind, DiagnosticsKind::Crash);
        assert_eq!(record.fields.len(), 3);

        let field_map: std::collections::HashMap<_, _> = record.fields.into_iter().collect();
        assert_eq!(field_map.get("engine_state"), Some(&"ready".to_string()));
        assert_eq!(field_map.get("raw_url"), Some(&REDACTED_MARKER.to_string()));
        assert_eq!(
            field_map.get("error_kind"),
            Some(&"navigation_failed".to_string())
        );
    }

    #[test]
    fn too_many_fields_rejected() {
        let redactor = DiagnosticsRedactor::new();
        let raw: Vec<(String, String)> = (0..(MAX_FIELDS_PER_RECORD + 1))
            .map(|i| (format!("field_{i}"), "value".into()))
            .collect();

        let result = redactor.build_record(DiagnosticsKind::Info, &raw, 1000);
        assert!(matches!(
            result,
            Err(DiagnosticsError::TooManyFields { .. })
        ));
    }

    #[test]
    fn field_value_too_long_rejected() {
        let redactor = DiagnosticsRedactor::new();
        let long_value = "x".repeat(MAX_FIELD_VALUE_LEN + 1);
        let raw = vec![("engine_state".into(), long_value)];

        let result = redactor.build_record(DiagnosticsKind::Error, &raw, 1000);
        assert!(matches!(
            result,
            Err(DiagnosticsError::FieldValueTooLong { .. })
        ));
    }

    #[test]
    fn no_raw_page_content_in_output() {
        let redactor = DiagnosticsRedactor::new();
        let raw = vec![
            (
                "page_content".into(),
                "<html><body>secret data</body></html>".into(),
            ),
            ("engine_state".into(), "crashed".into()),
        ];

        let record = redactor
            .build_record(DiagnosticsKind::Crash, &raw, 1000)
            .unwrap();

        let json = serde_json::to_string(&record).unwrap();
        assert!(!json.contains("secret data"));
        assert!(!json.contains("<html>"));
        assert!(json.contains(REDACTED_MARKER));
    }

    #[test]
    fn no_credentials_in_output() {
        let redactor = DiagnosticsRedactor::new();
        let raw = vec![
            ("api_key".into(), "sk-1234567890abcdef".into()),
            ("password".into(), "hunter2".into()),
            ("engine_state".into(), "ready".into()),
        ];

        let record = redactor
            .build_record(DiagnosticsKind::Info, &raw, 1000)
            .unwrap();

        let json = serde_json::to_string(&record).unwrap();
        assert!(!json.contains("sk-1234567890abcdef"));
        assert!(!json.contains("hunter2"));
    }

    #[test]
    fn record_version_is_stable() {
        let redactor = DiagnosticsRedactor::new();
        let record = redactor
            .build_record(DiagnosticsKind::Info, &[], 1000)
            .unwrap();
        assert_eq!(record.version, 1);
    }

    #[test]
    fn custom_allowlist_works() {
        let mut allowlist = HashSet::new();
        allowlist.insert("custom_field".into());
        let redactor = DiagnosticsRedactor::with_allowlist(allowlist);

        assert!(redactor.is_allowed("custom_field"));
        assert!(!redactor.is_allowed("engine_state"));

        let result = redactor.redact_value("custom_field", "value");
        assert_eq!(result, "value");

        let result = redactor.redact_value("engine_state", "ready");
        assert_eq!(result, REDACTED_MARKER);
    }
}
