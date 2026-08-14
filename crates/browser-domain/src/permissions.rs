//! Origin/profile/tab-bound permission state and default-deny policy.
//!
//! This module decides grants only. It does not implement camera, microphone,
//! notification, or other hardware capabilities.

use crate::ids::{ProfileId, TabId};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fmt;

/// Canonicalized web origin supplied by the engine boundary.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct Origin(String);

impl Origin {
    pub fn new(origin: impl Into<String>) -> Result<Self, PermissionError> {
        let raw = origin.into();
        let (scheme, authority) = raw
            .split_once("://")
            .ok_or(PermissionError::InvalidOrigin)?;
        if scheme.is_empty()
            || authority.is_empty()
            || authority.contains(['/', '?', '#', '@'])
            || raw.trim() != raw
        {
            return Err(PermissionError::InvalidOrigin);
        }
        Ok(Self(format!(
            "{}://{}",
            scheme.to_ascii_lowercase(),
            authority.to_ascii_lowercase()
        )))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

/// Sensitive capability categories represented by the policy.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum PermissionKind {
    Camera,
    Microphone,
    Geolocation,
    Notifications,
    Storage,
}

/// Duration of a user-approved grant.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum GrantLifetime {
    OneShot,
    Session,
    Persistent { expires_at: Option<u64> },
}

/// Request context used for both lookup and grant operations.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PermissionRequest {
    permission: PermissionKind,
    requesting_origin: Origin,
    top_level_site: Origin,
    opener_origin: Option<Origin>,
    profile_id: ProfileId,
    tab_id: TabId,
    user_gesture: bool,
    requested_at: u64,
}

impl PermissionRequest {
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        permission: PermissionKind,
        requesting_origin: Origin,
        top_level_site: Origin,
        opener_origin: Option<Origin>,
        profile_id: ProfileId,
        tab_id: TabId,
        user_gesture: bool,
        requested_at: u64,
    ) -> Self {
        Self {
            permission,
            requesting_origin,
            top_level_site,
            opener_origin,
            profile_id,
            tab_id,
            user_gesture,
            requested_at,
        }
    }

    pub const fn permission(&self) -> PermissionKind {
        self.permission
    }

    pub fn requesting_origin(&self) -> &Origin {
        &self.requesting_origin
    }

    pub fn top_level_site(&self) -> &Origin {
        &self.top_level_site
    }

    pub fn opener_origin(&self) -> Option<&Origin> {
        self.opener_origin.as_ref()
    }

    pub fn profile_id(&self) -> &ProfileId {
        &self.profile_id
    }

    pub fn tab_id(&self) -> &TabId {
        &self.tab_id
    }

    pub const fn user_gesture(&self) -> bool {
        self.user_gesture
    }

    pub const fn requested_at(&self) -> u64 {
        self.requested_at
    }
}

/// Observable policy result.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PermissionDecision {
    Granted { expires_at: Option<u64> },
    Denied { reason: DenyReason },
}

/// Safe reasons for a denied request.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DenyReason {
    DefaultDeny,
    Expired,
}

/// Errors while creating or approving permission state.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PermissionError {
    InvalidOrigin,
    UserGestureRequired,
}

impl fmt::Display for PermissionError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidOrigin => formatter.write_str("invalid permission origin"),
            Self::UserGestureRequired => formatter.write_str("user gesture required for grant"),
        }
    }
}

impl std::error::Error for PermissionError {}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
struct GrantKey {
    permission: PermissionKind,
    requesting_origin: Origin,
    top_level_site: Origin,
    opener_origin: Option<Origin>,
    profile_id: ProfileId,
    tab_id: TabId,
}

impl From<&PermissionRequest> for GrantKey {
    fn from(request: &PermissionRequest) -> Self {
        Self {
            permission: request.permission,
            requesting_origin: request.requesting_origin.clone(),
            top_level_site: request.top_level_site.clone(),
            opener_origin: request.opener_origin.clone(),
            profile_id: request.profile_id.clone(),
            tab_id: request.tab_id.clone(),
        }
    }
}

#[derive(Debug, Clone, Copy)]
struct Grant {
    expires_at: Option<u64>,
    one_shot: bool,
}

/// In-memory policy state. A future profile repository can persist the same
/// key/value contract without changing the decision semantics.
#[derive(Debug, Default)]
pub struct PermissionStore {
    grants: HashMap<GrantKey, Grant>,
}

impl PermissionStore {
    pub fn new() -> Self {
        Self::default()
    }

    /// Default deny unless an exact context-bound grant is active.
    pub fn decide(&mut self, request: &PermissionRequest) -> PermissionDecision {
        let key = GrantKey::from(request);
        let Some(grant) = self.grants.get(&key).copied() else {
            return PermissionDecision::Denied {
                reason: DenyReason::DefaultDeny,
            };
        };
        if grant
            .expires_at
            .is_some_and(|expires_at| request.requested_at >= expires_at)
        {
            self.grants.remove(&key);
            return PermissionDecision::Denied {
                reason: DenyReason::Expired,
            };
        }
        if grant.one_shot {
            self.grants.remove(&key);
        }
        PermissionDecision::Granted {
            expires_at: grant.expires_at,
        }
    }

    /// Persist a grant only after a user gesture has been observed.
    pub fn grant(
        &mut self,
        request: &PermissionRequest,
        lifetime: GrantLifetime,
    ) -> Result<(), PermissionError> {
        if !request.user_gesture {
            return Err(PermissionError::UserGestureRequired);
        }
        let (expires_at, one_shot) = match lifetime {
            GrantLifetime::OneShot => (None, true),
            GrantLifetime::Session => (None, false),
            GrantLifetime::Persistent { expires_at } => (expires_at, false),
        };
        self.grants.insert(
            GrantKey::from(request),
            Grant {
                expires_at,
                one_shot,
            },
        );
        Ok(())
    }

    pub fn revoke(&mut self, request: &PermissionRequest) -> bool {
        self.grants.remove(&GrantKey::from(request)).is_some()
    }

    pub fn clear_profile(&mut self, profile_id: &ProfileId) -> usize {
        remove_matching(&mut self.grants, |key| &key.profile_id == profile_id)
    }

    pub fn clear_tab(&mut self, profile_id: &ProfileId, tab_id: &TabId) -> usize {
        remove_matching(&mut self.grants, |key| {
            &key.profile_id == profile_id && &key.tab_id == tab_id
        })
    }

    pub fn expire(&mut self, now: u64) -> usize {
        let before = self.grants.len();
        self.grants
            .retain(|_, grant| !grant.expires_at.is_some_and(|expires_at| now >= expires_at));
        before - self.grants.len()
    }

    pub fn active_grants(&self) -> usize {
        self.grants.len()
    }
}

fn remove_matching<F>(grants: &mut HashMap<GrantKey, Grant>, mut predicate: F) -> usize
where
    F: FnMut(&GrantKey) -> bool,
{
    let before = grants.len();
    grants.retain(|key, _| !predicate(key));
    before - grants.len()
}
