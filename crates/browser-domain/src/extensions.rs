//! Extensions boundary spike: capability gate, manifest validation and
//! explicit go/no-go.
//!
//! Decision for the MVP/Alpha: `extensions=false`. There is no loader, no
//! packaged extension API, no isolated world and no extension process model.
//! Every activation attempt (including hostile ones) is rejected while the
//! capability is disabled. The privilege matrix records what is deferred and
//! why, so the boundary is explicit and testable.

use std::fmt;

/// Runtime capability gate. Default is disabled: extensions are out of the
/// MVP and Alpha.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct ExtensionsCapability {
    enabled: bool,
}

impl ExtensionsCapability {
    pub fn new(enabled: bool) -> Self {
        Self { enabled }
    }

    pub const fn enabled(self) -> bool {
        self.enabled
    }
}

/// Spike-level manifest validation: conservative shape checks only. Full
/// manifest semantics are deferred by ADR-008.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExtensionManifest {
    pub id: String,
    pub name: String,
    pub version: String,
    pub permissions: Vec<String>,
}

/// Allowed extension permissions in the spike model. Anything outside this
/// allowlist is rejected even when extensions are enabled.
pub const ALLOWED_EXTENSION_PERMISSIONS: &[&str] = &["storage"];

const MAX_ID_LEN: usize = 64;
const MAX_NAME_LEN: usize = 128;

fn valid_id(id: &str) -> bool {
    !id.is_empty()
        && !id.contains("..")
        && id.len() <= MAX_ID_LEN
        && id
            .chars()
            .all(|ch| ch.is_ascii_lowercase() || ch.is_ascii_digit() || ch == '_' || ch == '.')
}

fn valid_semver(version: &str) -> bool {
    let parts: Vec<&str> = version.split('.').collect();
    parts.len() == 3
        && parts.iter().all(|part| {
            !part.is_empty() && part.len() <= 10 && part.chars().all(|ch| ch.is_ascii_digit())
        })
}

/// Why an extension activation was rejected.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ExtensionError {
    ExtensionsDisabled,
    InvalidManifest(String),
    PermissionNotAllowed(String),
}

impl fmt::Display for ExtensionError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::ExtensionsDisabled => {
                formatter.write_str("extensions are disabled in this build")
            }
            Self::InvalidManifest(reason) => {
                write!(formatter, "invalid extension manifest: {reason}")
            }
            Self::PermissionNotAllowed(permission) => {
                write!(formatter, "extension permission not allowed: {permission}")
            }
        }
    }
}

impl std::error::Error for ExtensionError {}

/// Attempt to activate an extension under a capability.
///
/// Fail-closed: while the capability is disabled every activation is rejected
/// before any manifest content is trusted.
pub fn try_activate(
    capability: ExtensionsCapability,
    manifest: &ExtensionManifest,
) -> Result<(), ExtensionError> {
    if !capability.enabled() {
        return Err(ExtensionError::ExtensionsDisabled);
    }
    if !valid_id(&manifest.id) {
        return Err(ExtensionError::InvalidManifest("id".to_string()));
    }
    if manifest.name.trim().is_empty() || manifest.name.len() > MAX_NAME_LEN {
        return Err(ExtensionError::InvalidManifest("name".to_string()));
    }
    if !valid_semver(&manifest.version) {
        return Err(ExtensionError::InvalidManifest("version".to_string()));
    }
    for permission in &manifest.permissions {
        if !ALLOWED_EXTENSION_PERMISSIONS.contains(&permission.as_str()) {
            return Err(ExtensionError::PermissionNotAllowed(permission.clone()));
        }
    }
    Ok(())
}

/// One row of the extension privilege matrix.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PrivilegeEntry {
    pub scope: &'static str,
    pub mvp_status: &'static str,
    pub deferral: &'static str,
}

/// Extension privilege matrix: every scope that would be needed for real
/// extensions, with explicit MVP status and deferral reason.
pub const PRIVILEGE_MATRIX: &[PrivilegeEntry] = &[
    PrivilegeEntry {
        scope: "manifest",
        mvp_status: "validated",
        deferral: "spike-level validation only; full manifest semantics deferred",
    },
    PrivilegeEntry {
        scope: "loader",
        mvp_status: "absent",
        deferral: "no loader or packaged API exists in the MVP",
    },
    PrivilegeEntry {
        scope: "isolated_world",
        mvp_status: "absent",
        deferral: "requires engine integration; deferred past Alpha",
    },
    PrivilegeEntry {
        scope: "permissions",
        mvp_status: "allowlist",
        deferral: "single permission (storage); broader model deferred",
    },
    PrivilegeEntry {
        scope: "lifecycle",
        mvp_status: "absent",
        deferral: "no install/update/disable lifecycle in the MVP",
    },
    PrivilegeEntry {
        scope: "process_model",
        mvp_status: "absent",
        deferral: "extensions must run out-of-process; blocked by engine host work",
    },
];

/// Explicit go/no-go for the MVP/Alpha: extensions stay disabled; the gate
/// remains closed until the deferrals above are resolved by ADR review.
pub fn go_no_go() -> &'static str {
    "NO_GO: extensions remain disabled for MVP/Alpha (ADR-008)"
}

#[cfg(test)]
mod tests {
    use super::*;

    fn valid_manifest() -> ExtensionManifest {
        ExtensionManifest {
            id: "example.notes".to_string(),
            name: "Example Notes".to_string(),
            version: "1.0.0".to_string(),
            permissions: vec!["storage".to_string()],
        }
    }

    #[test]
    fn disabled_capability_rejects_hostile_activation() {
        let manifest = valid_manifest();
        assert_eq!(
            try_activate(ExtensionsCapability::default(), &manifest),
            Err(ExtensionError::ExtensionsDisabled)
        );
    }

    #[test]
    fn disabled_is_the_default() {
        assert!(!ExtensionsCapability::default().enabled());
    }

    #[test]
    fn enabled_capability_accepts_valid_manifest() {
        let manifest = valid_manifest();
        assert_eq!(
            try_activate(ExtensionsCapability::new(true), &manifest),
            Ok(())
        );
    }

    #[test]
    fn malicious_manifest_ids_rejected() {
        let capability = ExtensionsCapability::new(true);
        for id in [
            "",
            "UPPER",
            "has space",
            "semi;colon",
            "..",
            &"a".repeat(MAX_ID_LEN + 1),
        ] {
            let mut manifest = valid_manifest();
            manifest.id = id.to_string();
            assert!(
                matches!(
                    try_activate(capability, &manifest),
                    Err(ExtensionError::InvalidManifest(_))
                ),
                "id {id:?} must be rejected"
            );
        }
    }

    #[test]
    fn invalid_versions_rejected() {
        let capability = ExtensionsCapability::new(true);
        for version in ["", "1", "1.0", "1.0.0.0", "v1.0.0", "1.a.0", "1.0.0-rc1"] {
            let mut manifest = valid_manifest();
            manifest.version = version.to_string();
            assert!(
                matches!(
                    try_activate(capability, &manifest),
                    Err(ExtensionError::InvalidManifest(_))
                ),
                "version {version:?} must be rejected"
            );
        }
    }

    #[test]
    fn unknown_permission_rejected() {
        let capability = ExtensionsCapability::new(true);
        let mut manifest = valid_manifest();
        manifest.permissions = vec!["tabs".to_string(), "history".to_string()];
        assert_eq!(
            try_activate(capability, &manifest),
            Err(ExtensionError::PermissionNotAllowed("tabs".to_string()))
        );
    }

    #[test]
    fn empty_name_rejected() {
        let capability = ExtensionsCapability::new(true);
        let mut manifest = valid_manifest();
        manifest.name = "   ".to_string();
        assert!(matches!(
            try_activate(capability, &manifest),
            Err(ExtensionError::InvalidManifest(_))
        ));
    }

    #[test]
    fn privilege_matrix_covers_every_deferred_scope() {
        assert_eq!(PRIVILEGE_MATRIX.len(), 6);
        for entry in PRIVILEGE_MATRIX {
            assert!(!entry.mvp_status.is_empty());
            assert!(!entry.deferral.is_empty());
        }
        assert!(PRIVILEGE_MATRIX
            .iter()
            .any(|entry| entry.scope == "process_model"));
        assert!(PRIVILEGE_MATRIX
            .iter()
            .any(|entry| entry.scope == "isolated_world"));
    }

    #[test]
    fn go_no_go_is_explicit_no() {
        assert!(go_no_go().starts_with("NO_GO"));
    }
}
