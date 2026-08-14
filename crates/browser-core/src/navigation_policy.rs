//! Navigation, external protocol and file access policy.
//!
//! Pure parsing and policy evaluation with no I/O: the caller decides what
//! happens after an action is returned. Default posture is deny: only
//! `http`/`https`/`about` navigate directly, userinfo is rejected from http
//! URLs (anti-spoofing), https downgrades are refused, external protocols
//! require an explicit allowlist plus confirmation, and `file://` navigation
//! is denied unless a rooted access policy is configured.

use std::collections::HashSet;
use std::fmt;
use std::path::{Path, PathBuf};

/// Schemes the policy understands.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Scheme {
    Http,
    Https,
    About,
    File,
    Data,
    Javascript,
    External(String),
}

/// Result of parsing a URL with our conservative parser.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ParsedUrl {
    pub scheme: Scheme,
    pub host: Option<String>,
    pub port: Option<u16>,
}

/// Why a URL or handler was refused.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PolicyError {
    InvalidScheme,
    MissingAuthority,
    InvalidHost,
    InvalidPort,
    UserInfoNotAllowed,
    DisallowedScheme,
    HttpsDowngrade,
    ShellInterpolation,
    HandlerNotAllowed,
    Traversal,
    NullByte,
    InvalidPath,
    NotConfigured,
    Malformed(String),
}

impl fmt::Display for PolicyError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidScheme => formatter.write_str("missing or invalid URL scheme"),
            Self::MissingAuthority => formatter.write_str("URL has no authority"),
            Self::InvalidHost => formatter.write_str("URL has an invalid host"),
            Self::InvalidPort => formatter.write_str("URL has an invalid port"),
            Self::UserInfoNotAllowed => formatter.write_str("userinfo in http(s) URL"),
            Self::DisallowedScheme => formatter.write_str("scheme is not allowed"),
            Self::HttpsDowngrade => formatter.write_str("https to http downgrade refused"),
            Self::ShellInterpolation => formatter.write_str("shell metacharacters not allowed"),
            Self::HandlerNotAllowed => formatter.write_str("external handler not allowlisted"),
            Self::Traversal => formatter.write_str("path escapes the allowed root"),
            Self::NullByte => formatter.write_str("null byte in path"),
            Self::InvalidPath => {
                formatter.write_str("path contains a platform separator or drive prefix")
            }
            Self::NotConfigured => formatter.write_str("file access is not configured"),
            Self::Malformed(reason) => write!(formatter, "malformed URL: {reason}"),
        }
    }
}

impl std::error::Error for PolicyError {}

fn has_control_or_space(input: &str) -> bool {
    input.bytes().any(|byte| byte <= 0x20)
}

fn valid_scheme_chars(scheme: &str) -> bool {
    let mut chars = scheme.chars();
    let Some(first) = chars.next() else {
        return false;
    };
    if !first.is_ascii_alphabetic() {
        return false;
    }
    chars.all(|ch| ch.is_ascii_alphanumeric() || matches!(ch, '+' | '-' | '.'))
}

fn classify_scheme(scheme: &str) -> Scheme {
    match scheme.to_ascii_lowercase().as_str() {
        "http" => Scheme::Http,
        "https" => Scheme::Https,
        "about" => Scheme::About,
        "file" => Scheme::File,
        "data" => Scheme::Data,
        "javascript" => Scheme::Javascript,
        other => Scheme::External(other.to_string()),
    }
}

fn valid_host_labels(host: &str) -> bool {
    if host.is_empty() || !host.is_ascii() {
        return false;
    }
    host.split('.').all(|label| {
        !label.is_empty()
            && !label.starts_with('-')
            && !label.ends_with('-')
            && label
                .bytes()
                .all(|byte| byte.is_ascii_alphanumeric() || byte == b'-')
    })
}

fn parse_port(text: &str) -> Result<u16, PolicyError> {
    if text.is_empty() {
        return Err(PolicyError::InvalidPort);
    }
    let value: u16 = text.parse().map_err(|_| PolicyError::InvalidPort)?;
    if value == 0 {
        return Err(PolicyError::InvalidPort);
    }
    Ok(value)
}

fn split_authority(authority: &str) -> Result<(String, Option<u16>), PolicyError> {
    if authority.starts_with('[') {
        let close = authority.find(']').ok_or(PolicyError::InvalidHost)?;
        let host = &authority[..=close];
        let ipv6 = &host[1..host.len() - 1];
        if ipv6.is_empty() || ipv6.parse::<std::net::Ipv6Addr>().is_err() {
            return Err(PolicyError::InvalidHost);
        }
        let remainder = &authority[close + 1..];
        let port = if remainder.is_empty() {
            None
        } else if let Some(port_text) = remainder.strip_prefix(':') {
            Some(parse_port(port_text)?)
        } else {
            return Err(PolicyError::InvalidHost);
        };
        return Ok((host.to_string(), port));
    }

    if authority.contains(['[', ']']) || authority.matches(':').count() > 1 {
        return Err(PolicyError::InvalidHost);
    }
    let (host, port) = match authority.split_once(':') {
        Some((host, port)) => (host, Some(parse_port(port)?)),
        None => (authority, None),
    };
    if !valid_host_labels(host) {
        return Err(PolicyError::InvalidHost);
    }
    Ok((host.to_string(), port))
}

/// Conservative URL parser: validates structure without a third-party crate.
pub fn parse_url(raw: &str) -> Result<ParsedUrl, PolicyError> {
    if raw.is_empty() || has_control_or_space(raw) {
        return Err(PolicyError::Malformed("control characters".to_string()));
    }
    let colon = raw.find(':').ok_or(PolicyError::InvalidScheme)?;
    let scheme_text = &raw[..colon];
    if !valid_scheme_chars(scheme_text) {
        return Err(PolicyError::InvalidScheme);
    }
    let scheme = classify_scheme(scheme_text);
    let rest = &raw[colon + 1..];

    let (host, port) = match &scheme {
        Scheme::Http | Scheme::Https => {
            let after = rest
                .strip_prefix("//")
                .ok_or(PolicyError::MissingAuthority)?;
            let authority_end = after.find(['/', '?', '#']).unwrap_or(after.len());
            let authority = &after[..authority_end];
            if authority.is_empty() {
                return Err(PolicyError::MissingAuthority);
            }
            if authority.contains('@') {
                return Err(PolicyError::UserInfoNotAllowed);
            }
            if authority.bytes().any(|byte| byte <= 0x20) {
                return Err(PolicyError::InvalidHost);
            }
            let (host, port) = split_authority(authority)?;
            (Some(host), port)
        }
        _ => (None, None),
    };

    Ok(ParsedUrl { scheme, host, port })
}

/// What the navigation policy decides for a URL.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NavigationAction {
    Allow,
    Confirm,
    Deny,
}

/// Scheme allowlist and redirect re-evaluation policy.
#[derive(Debug, Clone, Default)]
pub struct NavigationPolicy {
    allow_file: bool,
    allow_http_downgrade: bool,
}

impl NavigationPolicy {
    pub fn classify(&self, url: &str) -> NavigationAction {
        let parsed = match parse_url(url) {
            Ok(parsed) => parsed,
            Err(_) => return NavigationAction::Deny,
        };
        match parsed.scheme {
            Scheme::Http | Scheme::Https | Scheme::About => NavigationAction::Allow,
            Scheme::File if self.allow_file => NavigationAction::Confirm,
            Scheme::External(_) => NavigationAction::Confirm,
            _ => NavigationAction::Deny,
        }
    }

    /// Re-evaluate a redirect target against the policy.
    ///
    /// The target must pass the scheme allowlist and must not downgrade an
    /// https navigation to http.
    pub fn evaluate_redirect(&self, from: &str, to: &str) -> Result<(), PolicyError> {
        let from_parsed = parse_url(from)?;
        let to_parsed = parse_url(to)?;
        if let (Scheme::Https, Scheme::Http) = (&from_parsed.scheme, &to_parsed.scheme) {
            if !self.allow_http_downgrade {
                return Err(PolicyError::HttpsDowngrade);
            }
        }
        match self.classify(to) {
            NavigationAction::Allow => Ok(()),
            NavigationAction::Confirm | NavigationAction::Deny => {
                Err(PolicyError::DisallowedScheme)
            }
        }
    }
}

/// What an external protocol evaluation decided.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ExternalAction {
    OpenAllowed,
    RequiresConfirmation,
    Denied,
}

const SHELL_METACHARS: &[char] = &[';', '&', '|', '<', '>', '`', '$', '(', ')', '{', '}', '\n'];

fn has_shell_metachars(input: &str) -> bool {
    input.chars().any(|ch| SHELL_METACHARS.contains(&ch))
}

/// External protocol handler policy: explicit allowlist, no shell
/// interpolation, optional confirmation.
#[derive(Debug, Clone)]
pub struct ExternalProtocolPolicy {
    allowed_handlers: HashSet<String>,
    require_confirmation: bool,
}

impl ExternalProtocolPolicy {
    pub fn new(allowed_handlers: HashSet<String>, require_confirmation: bool) -> Self {
        Self {
            allowed_handlers,
            require_confirmation,
        }
    }

    pub fn evaluate(&self, handler: &str, args: &str) -> ExternalAction {
        if handler.is_empty()
            || !handler.chars().all(|ch| ch.is_ascii_alphanumeric())
            || has_shell_metachars(handler)
            || has_shell_metachars(args)
        {
            return ExternalAction::Denied;
        }
        if self.allowed_handlers.contains(handler) {
            ExternalAction::OpenAllowed
        } else if self.require_confirmation {
            ExternalAction::RequiresConfirmation
        } else {
            ExternalAction::Denied
        }
    }
}

/// What a file path resolution decided.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FileAccess {
    Allowed(PathBuf),
    Denied(PolicyError),
}

/// Rooted file access: only paths inside the configured root resolve.
#[derive(Debug, Clone, Default)]
pub struct FileAccessPolicy {
    allowed_root: Option<PathBuf>,
}

impl FileAccessPolicy {
    pub fn new(allowed_root: PathBuf) -> Self {
        Self {
            allowed_root: Some(allowed_root),
        }
    }

    pub fn resolve(&self, url_path: &str) -> FileAccess {
        let Some(root) = &self.allowed_root else {
            return FileAccess::Denied(PolicyError::NotConfigured);
        };
        let decoded = url_path
            .replace("%2F", "/")
            .replace("%2f", "/")
            .replace("%2E", ".")
            .replace("%2e", ".")
            .replace("%5C", "\\")
            .replace("%5c", "\\")
            .replace("%3A", ":")
            .replace("%3a", ":")
            .replace("%00", "\0");
        if decoded.contains('\0') {
            return FileAccess::Denied(PolicyError::NullByte);
        }
        if decoded.contains('\\') || decoded.contains(':') {
            return FileAccess::Denied(PolicyError::InvalidPath);
        }

        let mut out = PathBuf::from(root);
        for component in decoded.split('/') {
            if component.is_empty() || component == "." {
                continue;
            }
            if component == ".." {
                return FileAccess::Denied(PolicyError::Traversal);
            }
            out.push(component);
        }

        let out_root = out.as_path();
        if out_root.components().count() < Path::new(root).components().count()
            || !out_root.starts_with(root)
        {
            return FileAccess::Denied(PolicyError::Traversal);
        }
        FileAccess::Allowed(out)
    }
}
