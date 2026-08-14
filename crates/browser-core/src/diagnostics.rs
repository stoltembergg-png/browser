//! Structured logs, crash bundles and telemetry redaction.
//!
//! Diagnostics never leave the machine: bundles stay local, telemetry is
//! opt-in, and every record is redacted before it is written. Redaction is
//! golden-tested: URLs lose userinfo/query values/fragments, tokens and
//! secrets become `[redacted]`, user paths keep no identity, page content is
//! never logged, and non-allowlisted fields are dropped entirely.

use std::collections::HashSet;

/// Fields allowed to keep a redacted value; everything else is `[redacted]`.
#[derive(Debug, Clone)]
pub struct RedactionConfig {
    allowlisted: HashSet<String>,
}

impl Default for RedactionConfig {
    /// Common diagnostic fields keep redacted values; sensitive fields
    /// (anything else) are dropped entirely.
    fn default() -> Self {
        Self::new(
            [
                "url",
                "header",
                "path",
                "param",
                "log",
                "field",
                "navigation.url",
            ]
            .iter()
            .map(|s| s.to_string())
            .collect(),
        )
    }
}

impl RedactionConfig {
    pub fn new(allowlisted: HashSet<String>) -> Self {
        Self { allowlisted }
    }
}

/// A record after redaction.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RedactedRecord {
    pub field: String,
    pub value: String,
}

const REDACTED: &str = "[redacted]";

const SECRET_KEY_MARKERS: &[&str] = &[
    "password",
    "api_key",
    "apikey",
    "token",
    "secret",
    "authorization",
];

fn redact_url(value: &str) -> String {
    let mut out = value.to_string();
    if let Some(at) = out.find('@') {
        if let Some(scheme_end) = out.find("://") {
            if at > scheme_end {
                out = format!("{}[redacted]@{}", &out[..scheme_end + 3], &out[at + 1..]);
            }
        }
    }
    if let Some(hash) = out.find('#') {
        out.truncate(hash);
        out.push('#');
        out.push_str(REDACTED);
    }
    if let Some(query) = out.find('?') {
        let (head, rest) = out.split_at(query + 1);
        let redacted_pairs: Vec<String> = rest
            .split('&')
            .map(|pair| match pair.split_once('=') {
                Some((key, _)) => format!("{key}={REDACTED}"),
                None => REDACTED.to_string(),
            })
            .collect();
        out = format!("{head}{}", redacted_pairs.join("&"));
    }
    out
}

fn redact_token_markers(value: &str) -> String {
    let mut out = value.to_string();
    let mut cursor = 0;
    while cursor < out.len() {
        let tail = out[cursor..].to_ascii_lowercase();
        let Some((rel, marker_len, separator_len)) = SECRET_KEY_MARKERS.iter().find_map(|marker| {
            let eq = tail.find(&format!("{marker}=")).map(|pos| (pos, 1));
            let space = tail.find(&format!("{marker} ")).map(|pos| (pos, 1));
            let colon = tail.find(&format!("{marker}:")).map(|pos| (pos, 1));
            match (eq, space, colon) {
                (Some((pos, sep)), _, _) => Some((pos, marker.len(), sep)),
                (None, Some((pos, sep)), _) => Some((pos, marker.len(), sep)),
                (None, None, Some((pos, sep))) => Some((pos, marker.len(), sep)),
                (None, None, None) => None,
            }
        }) else {
            break;
        };
        let pos = cursor + rel;
        let mut value_start = pos + marker_len + separator_len;
        while value_start < out.len() && out.as_bytes()[value_start].is_ascii_whitespace() {
            value_start += 1;
        }
        let end = if separator_len == 1 && out.as_bytes()[pos + marker_len] == b':' {
            out.len()
        } else {
            out[value_start..]
                .find(|ch: char| ch.is_whitespace() || ch == '&' || ch == ',')
                .map(|i| value_start + i)
                .unwrap_or(out.len())
        };
        if end > value_start {
            out.replace_range(value_start..end, REDACTED);
            cursor = value_start + REDACTED.len();
        } else {
            cursor = end + 1;
        }
    }
    out
}

fn looks_like_token_string(value: &str) -> bool {
    if value.chars().any(char::is_whitespace) {
        return false;
    }
    let alnum = value
        .chars()
        .filter(|ch| ch.is_ascii_alphanumeric())
        .count();
    value.len() >= 32 && alnum * 10 >= value.len() * 9
}

fn redact_path(value: &str) -> String {
    for prefix in ["/Users/", "/home/"] {
        if let Some(pos) = value.find(prefix) {
            let after = &value[pos + prefix.len()..];
            if after.contains('/') {
                return format!("{}{}[user]/{REDACTED}", &value[..pos], prefix);
            }
        }
    }
    for prefix in ["C:\\Users\\"] {
        if let Some(pos) = value.find(prefix) {
            let after = &value[pos + prefix.len()..];
            if after.contains('\\') {
                return format!("{}{}[user]\\{REDACTED}", &value[..pos], prefix);
            }
        }
    }
    value.to_string()
}

fn looks_like_page_content(value: &str) -> bool {
    value.starts_with('<')
        && (value.contains("<html") || value.contains("<body") || value.contains("<script"))
}

/// Redact one structured record against the config.
pub fn redact_record(field: &str, value: &str, config: &RedactionConfig) -> Option<RedactedRecord> {
    if !config.allowlisted.contains(field) {
        return Some(RedactedRecord {
            field: field.to_string(),
            value: REDACTED.to_string(),
        });
    }
    if looks_like_page_content(value) || looks_like_token_string(value) {
        return Some(RedactedRecord {
            field: field.to_string(),
            value: REDACTED.to_string(),
        });
    }
    let redacted = redact_url(value);
    let redacted = redact_token_markers(&redacted);
    let redacted = redact_path(&redacted);
    Some(RedactedRecord {
        field: field.to_string(),
        value: redacted,
    })
}

/// Local-only crash bundle: redacted records serialized to JSON.
#[derive(Debug, Clone, Default)]
pub struct CrashBundle {
    records: Vec<(String, String)>,
}

impl CrashBundle {
    pub fn new(records: Vec<(String, String)>) -> Self {
        Self { records }
    }

    /// Serialize the bundle locally. Never uploads; callers own the file.
    pub fn to_local_json(&self, config: &RedactionConfig) -> String {
        let mut map = serde_json::Map::new();
        for (field, value) in &self.records {
            let record = redact_record(field, value, config).expect("redaction never fails");
            map.insert(record.field, serde_json::Value::String(record.value));
        }
        serde_json::to_string(&serde_json::Value::Object(map)).expect("bundle serializes")
    }
}

/// Opt-in telemetry gate: nothing leaves the machine unless enabled.
#[derive(Debug, Clone, Copy)]
pub struct TelemetryGate {
    enabled: bool,
}

impl TelemetryGate {
    pub fn new(enabled: bool) -> Self {
        Self { enabled }
    }

    /// Collect one record; `None` when telemetry is not opted in.
    pub fn collect(&self, field: &str, value: &str) -> Option<RedactedRecord> {
        if !self.enabled {
            return None;
        }
        redact_record(field, value, &RedactionConfig::default())
    }
}
