//! Scheme/file navigation security — URL scheme allowlist and broker policy.
//!
//! Determines whether a navigation URL is safe to load. Rejects dangerous
//! schemes (file, data, javascript, custom protocols) by default. Re-evaluates
//! on redirect. Pure domain logic — no engine types, no real URL loader.

use std::collections::HashSet;
use std::fmt;

/// Schemes that are always allowed for top-level navigation.
pub const ALLOWED_SCHEMES: &[&str] = &["https", "http", "about"];

/// Schemes that are always denied.
pub const DENIED_SCHEMES: &[&str] = &["file", "data", "javascript", "vbscript", "blob"];

/// The decision for a navigation URL.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SchemeDecision {
    /// The URL is allowed for navigation.
    Allow,
    /// The URL is denied for the given reason.
    Deny { reason: DenyReason },
}

/// Reasons a URL is denied.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DenyReason {
    /// Scheme is not in the allowlist.
    UnknownScheme { scheme: String },
    /// Scheme is explicitly denied (file, data, javascript).
    DeniedScheme { scheme: String },
    /// URL contains path traversal attempts.
    PathTraversal,
    /// URL contains a username/password component (credential URL).
    CredentialInUrl,
    /// Redirect target was denied.
    RedirectDenied { reason: Box<DenyReason> },
    /// URL is malformed.
    MalformedUrl { reason: String },
}

impl fmt::Display for DenyReason {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::UnknownScheme { scheme } => write!(f, "unknown scheme: {scheme}"),
            Self::DeniedScheme { scheme } => write!(f, "denied scheme: {scheme}"),
            Self::PathTraversal => write!(f, "path traversal attempt"),
            Self::CredentialInUrl => write!(f, "credentials in URL"),
            Self::RedirectDenied { reason } => write!(f, "redirect denied: {reason}"),
            Self::MalformedUrl { reason } => write!(f, "malformed URL: {reason}"),
        }
    }
}

impl std::error::Error for DenyReason {}

/// The scheme navigation broker.
pub struct SchemeBroker {
    allowed: HashSet<String>,
    denied: HashSet<String>,
}

impl SchemeBroker {
    pub fn new() -> Self {
        let allowed: HashSet<String> = ALLOWED_SCHEMES.iter().map(|s| s.to_string()).collect();
        let denied: HashSet<String> = DENIED_SCHEMES.iter().map(|s| s.to_string()).collect();
        Self { allowed, denied }
    }

    /// Create a broker with custom scheme lists.
    pub fn with_schemes(allowed: HashSet<String>, denied: HashSet<String>) -> Self {
        Self { allowed, denied }
    }

    /// Extract the scheme from a URL string.
    pub fn extract_scheme(url: &str) -> Result<String, DenyReason> {
        let colon = url.find(':').ok_or(DenyReason::MalformedUrl {
            reason: "no scheme separator".into(),
        })?;
        let scheme = url[..colon].to_lowercase();
        if scheme.is_empty()
            || !scheme
                .chars()
                .all(|c| c.is_ascii_alphanumeric() || c == '+' || c == '-' || c == '.')
        {
            return Err(DenyReason::MalformedUrl {
                reason: "invalid scheme characters".into(),
            });
        }
        Ok(scheme)
    }

    /// Evaluate a URL for navigation.
    pub fn evaluate(&self, url: &str) -> SchemeDecision {
        let scheme = match Self::extract_scheme(url) {
            Ok(s) => s,
            Err(reason) => return SchemeDecision::Deny { reason },
        };

        if self.denied.contains(&scheme) {
            return SchemeDecision::Deny {
                reason: DenyReason::DeniedScheme { scheme },
            };
        }

        if !self.allowed.contains(&scheme) {
            return SchemeDecision::Deny {
                reason: DenyReason::UnknownScheme { scheme },
            };
        }

        // Check for path traversal in the URL path
        if url.contains("..") {
            return SchemeDecision::Deny {
                reason: DenyReason::PathTraversal,
            };
        }

        // Check for credentials in URL (userinfo@host)
        if let Some(at_idx) = url.find('@') {
            // Check if @ is before the first / after the scheme
            let after_scheme = url.find("://").map(|i| i + 3).unwrap_or(0);
            let slash_after_host = url[after_scheme..]
                .find('/')
                .map(|i| after_scheme + i)
                .unwrap_or(url.len());
            if at_idx < slash_after_host && at_idx > after_scheme {
                return SchemeDecision::Deny {
                    reason: DenyReason::CredentialInUrl,
                };
            }
        }

        SchemeDecision::Allow
    }

    /// Re-evaluate a redirect target — the target URL must pass the same
    /// checks as the original navigation.
    pub fn evaluate_redirect(&self, original: &str, target: &str) -> SchemeDecision {
        let original_decision = self.evaluate(original);
        if let SchemeDecision::Deny { .. } = original_decision {
            return original_decision;
        }

        match self.evaluate(target) {
            SchemeDecision::Allow => SchemeDecision::Allow,
            SchemeDecision::Deny { reason } => SchemeDecision::Deny {
                reason: DenyReason::RedirectDenied {
                    reason: Box::new(reason),
                },
            },
        }
    }

    /// Check if a scheme is in the allowlist.
    pub fn is_allowed_scheme(&self, scheme: &str) -> bool {
        self.allowed.contains(scheme)
    }

    /// Check if a scheme is in the denylist.
    pub fn is_denied_scheme(&self, scheme: &str) -> bool {
        self.denied.contains(scheme)
    }
}

impl Default for SchemeBroker {
    fn default() -> Self {
        Self::new()
    }
}

/// Decisão para um handler de protocolo externo (mailto:, custom:, etc.).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ExternalAction {
    /// Handler allowlisted e argumentos seguros: pode abrir.
    OpenAllowed,
    /// Handler não allowlisted: exige confirmação explícita do usuário.
    RequiresConfirmation,
    /// Handler ou argumentos rejeitados.
    Denied,
}

/// Caracteres que nunca podem aparecer em handler/args de protocolo externo:
/// impedem shell interpolation e command injection.
const SHELL_METACHARS: &[char] = &[';', '&', '|', '<', '>', '`', '$', '(', ')', '{', '}', '\n'];

/// Política de protocolos externos: allowlist explícita, nomes alfanuméricos,
/// sem shell interpolation, confirmação opcional. Sem launcher de OS.
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
            || handler.chars().any(|ch| SHELL_METACHARS.contains(&ch))
            || args.chars().any(|ch| SHELL_METACHARS.contains(&ch))
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn https_allowed() {
        let broker = SchemeBroker::new();
        assert_eq!(
            broker.evaluate("https://example.com/page"),
            SchemeDecision::Allow
        );
    }

    #[test]
    fn http_allowed() {
        let broker = SchemeBroker::new();
        assert_eq!(
            broker.evaluate("http://example.com/page"),
            SchemeDecision::Allow
        );
    }

    #[test]
    fn file_scheme_denied() {
        let broker = SchemeBroker::new();
        let decision = broker.evaluate("file:///etc/passwd");
        assert!(matches!(
            decision,
            SchemeDecision::Deny {
                reason: DenyReason::DeniedScheme { scheme }
            } if scheme == "file"
        ));
    }

    #[test]
    fn data_scheme_denied() {
        let broker = SchemeBroker::new();
        let decision = broker.evaluate("data:text/html,<script>alert(1)</script>");
        assert!(matches!(
            decision,
            SchemeDecision::Deny {
                reason: DenyReason::DeniedScheme { scheme }
            } if scheme == "data"
        ));
    }

    #[test]
    fn javascript_scheme_denied() {
        let broker = SchemeBroker::new();
        let decision = broker.evaluate("javascript:alert(1)");
        assert!(matches!(
            decision,
            SchemeDecision::Deny {
                reason: DenyReason::DeniedScheme { scheme }
            } if scheme == "javascript"
        ));
    }

    #[test]
    fn unknown_scheme_denied() {
        let broker = SchemeBroker::new();
        let decision = broker.evaluate("custom-protocol://data");
        assert!(matches!(
            decision,
            SchemeDecision::Deny {
                reason: DenyReason::UnknownScheme { .. }
            }
        ));
    }

    #[test]
    fn path_traversal_denied() {
        let broker = SchemeBroker::new();
        let decision = broker.evaluate("https://example.com/../../../etc/passwd");
        assert!(matches!(
            decision,
            SchemeDecision::Deny {
                reason: DenyReason::PathTraversal
            }
        ));
    }

    #[test]
    fn credential_in_url_denied() {
        let broker = SchemeBroker::new();
        let decision = broker.evaluate("https://user:password@example.com/page");
        assert!(matches!(
            decision,
            SchemeDecision::Deny {
                reason: DenyReason::CredentialInUrl
            }
        ));
    }

    #[test]
    fn url_without_credentials_allowed() {
        let broker = SchemeBroker::new();
        // @ in query param should not trigger credential check
        let decision = broker.evaluate("https://example.com/page?email=user@test.com");
        assert_eq!(decision, SchemeDecision::Allow);
    }

    #[test]
    fn malformed_url_denied() {
        let broker = SchemeBroker::new();
        let decision = broker.evaluate("not a url");
        assert!(matches!(
            decision,
            SchemeDecision::Deny {
                reason: DenyReason::MalformedUrl { .. }
            }
        ));
    }

    #[test]
    fn redirect_to_allowed_scheme_passes() {
        let broker = SchemeBroker::new();
        let decision = broker.evaluate_redirect("https://a.com", "https://b.com");
        assert_eq!(decision, SchemeDecision::Allow);
    }

    #[test]
    fn redirect_to_denied_scheme_rejected() {
        let broker = SchemeBroker::new();
        let decision = broker.evaluate_redirect("https://a.com", "file:///etc/passwd");
        assert!(matches!(
            decision,
            SchemeDecision::Deny {
                reason: DenyReason::RedirectDenied { .. }
            }
        ));
    }

    #[test]
    fn redirect_from_denied_original_rejected() {
        let broker = SchemeBroker::new();
        let decision = broker.evaluate_redirect("file:///etc/passwd", "https://b.com");
        assert!(matches!(
            decision,
            SchemeDecision::Deny {
                reason: DenyReason::DeniedScheme { .. }
            }
        ));
    }

    #[test]
    fn redirect_with_traversal_rejected() {
        let broker = SchemeBroker::new();
        let decision = broker.evaluate_redirect("https://a.com", "https://b.com/../etc");
        assert!(matches!(
            decision,
            SchemeDecision::Deny {
                reason: DenyReason::RedirectDenied { .. }
            }
        ));
    }

    #[test]
    fn about_scheme_allowed() {
        let broker = SchemeBroker::new();
        assert_eq!(broker.evaluate("about:blank"), SchemeDecision::Allow);
    }

    #[test]
    fn blob_scheme_denied() {
        let broker = SchemeBroker::new();
        let decision = broker.evaluate("blob:https://example.com/uuid");
        assert!(matches!(
            decision,
            SchemeDecision::Deny {
                reason: DenyReason::DeniedScheme { scheme }
            } if scheme == "blob"
        ));
    }

    #[test]
    fn extract_scheme_works() {
        assert_eq!(
            SchemeBroker::extract_scheme("https://x.com").unwrap(),
            "https"
        );
        assert_eq!(
            SchemeBroker::extract_scheme("HTTP://x.com").unwrap(),
            "http"
        );
        assert_eq!(
            SchemeBroker::extract_scheme("file:///path").unwrap(),
            "file"
        );
        assert!(SchemeBroker::extract_scheme("no-scheme").is_err());
    }

    #[test]
    fn custom_scheme_lists() {
        let allowed: HashSet<String> = ["https", "myapp"].iter().map(|s| s.to_string()).collect();
        let denied: HashSet<String> = ["ftp"].iter().map(|s| s.to_string()).collect();
        let broker = SchemeBroker::with_schemes(allowed, denied);

        assert_eq!(broker.evaluate("https://x.com"), SchemeDecision::Allow);
        assert_eq!(broker.evaluate("myapp://action"), SchemeDecision::Allow);
        assert!(matches!(
            broker.evaluate("ftp://x.com"),
            SchemeDecision::Deny {
                reason: DenyReason::DeniedScheme { .. }
            }
        ));
        assert!(matches!(
            broker.evaluate("http://x.com"),
            SchemeDecision::Deny {
                reason: DenyReason::UnknownScheme { .. }
            }
        ));
    }

    // --- External protocol policy ---

    fn handler_set(handlers: &[&str]) -> HashSet<String> {
        handlers.iter().map(|s| s.to_string()).collect()
    }

    #[test]
    fn allowlisted_handler_with_clean_args_opens() {
        let policy = ExternalProtocolPolicy::new(handler_set(&["mailto"]), false);
        assert_eq!(
            policy.evaluate("mailto", "user@example.com?subject=hello"),
            ExternalAction::OpenAllowed
        );
    }

    #[test]
    fn allowlisted_handler_with_shell_metachar_denied() {
        let policy = ExternalProtocolPolicy::new(handler_set(&["mailto"]), false);
        assert_eq!(
            policy.evaluate("mailto", "user@example.com; rm -rf /"),
            ExternalAction::Denied
        );
    }

    #[test]
    fn non_allowlisted_handler_requires_confirmation() {
        let policy = ExternalProtocolPolicy::new(handler_set(&[]), true);
        assert_eq!(
            policy.evaluate("slack", "channels"),
            ExternalAction::RequiresConfirmation
        );
    }

    #[test]
    fn confirmation_off_denies_non_allowlisted() {
        let policy = ExternalProtocolPolicy::new(handler_set(&[]), false);
        assert_eq!(policy.evaluate("slack", "channels"), ExternalAction::Denied);
    }

    #[test]
    fn handler_with_metachar_denied() {
        let policy = ExternalProtocolPolicy::new(handler_set(&["mailto"]), true);
        assert_eq!(policy.evaluate("mail;to", "x"), ExternalAction::Denied);
    }

    #[test]
    fn empty_handler_denied() {
        let policy = ExternalProtocolPolicy::new(handler_set(&[]), true);
        assert_eq!(policy.evaluate("", "x"), ExternalAction::Denied);
    }
}
