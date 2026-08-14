//! Secure permission prompt state and typed resolution.
//!
//! The prompt coordinator sits between the engine request and the
//! `PermissionStore`. It never grants silently: a request without an active
//! grant becomes a pending prompt that the user must resolve with a typed
//! choice (allow one-shot/session/persistent, or deny). The prompt displays
//! only the verified request context, never content from the web page.

use crate::permissions::{
    DenyReason, GrantLifetime, Origin, PermissionDecision, PermissionRequest, PermissionStore,
};
use std::collections::HashMap;
use std::fmt;

/// Stable identifier of one pending prompt.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct PromptId(pub u64);

impl fmt::Display for PromptId {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.0.fmt(formatter)
    }
}

/// The user's typed resolution for a pending prompt.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PromptResolution {
    /// Grant with the chosen lifetime.
    Allow(GrantLifetime),
    /// Explicit deny; nothing is remembered.
    Deny,
}

/// What a pending prompt shows and tracks.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PromptState {
    pub prompt_id: PromptId,
    /// Verified requesting origin shown to the user.
    pub origin: Origin,
    /// Top-level site the origin is embedded in.
    pub top_level: Origin,
    pub requested_at: u64,
}

/// Errors produced by the prompt coordinator.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PromptError {
    UserGestureRequired,
    DuplicatePrompt,
    AlreadyGranted,
    PromptNotFound,
    ContextMismatch,
    InvalidRequest(String),
}

impl fmt::Display for PromptError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::UserGestureRequired => {
                formatter.write_str("permission prompt requires a user gesture")
            }
            Self::DuplicatePrompt => {
                formatter.write_str("a prompt for this context is already pending")
            }
            Self::AlreadyGranted => formatter.write_str("permission is already granted"),
            Self::PromptNotFound => formatter.write_str("prompt not found"),
            Self::ContextMismatch => {
                formatter.write_str("resolution context does not match the prompt")
            }
            Self::InvalidRequest(reason) => {
                write!(formatter, "invalid permission request: {reason}")
            }
        }
    }
}

impl std::error::Error for PromptError {}

#[derive(Debug)]
struct PendingPrompt {
    request: PermissionRequest,
}

/// Owns pending prompts and applies typed user resolutions to the store.
#[derive(Debug)]
pub struct PromptCoordinator {
    store: PermissionStore,
    pending: HashMap<PromptId, PendingPrompt>,
    next_id: u64,
}

impl PromptCoordinator {
    pub fn new(store: PermissionStore) -> Self {
        Self {
            store,
            pending: HashMap::new(),
            next_id: 1,
        }
    }

    /// Ask the user only when there is no active grant for the request.
    ///
    /// A request without a user gesture is rejected before any state is
    /// created — grants and prompts never bypass the gesture requirement.
    pub fn prompt_for(&mut self, request: &PermissionRequest) -> Result<PromptId, PromptError> {
        if !request.user_gesture() {
            return Err(PromptError::UserGestureRequired);
        }
        match self.store.decide(request) {
            PermissionDecision::Granted { .. } => return Err(PromptError::AlreadyGranted),
            PermissionDecision::Denied { .. } => {}
        }

        if self
            .pending
            .values()
            .any(|pending| pending.request == *request)
        {
            return Err(PromptError::DuplicatePrompt);
        }

        let id = PromptId(self.next_id);
        self.next_id = self.next_id.saturating_add(1);
        self.pending.insert(
            id,
            PendingPrompt {
                request: request.clone(),
            },
        );
        Ok(id)
    }

    /// Resolve a prompt with the exact request context it was created from.
    pub fn resolve(
        &mut self,
        prompt_id: PromptId,
        resolution: PromptResolution,
    ) -> Result<PermissionDecision, PromptError> {
        let request = self
            .pending
            .get(&prompt_id)
            .map(|pending| pending.request.clone())
            .ok_or(PromptError::PromptNotFound)?;
        self.resolve_checked(prompt_id, resolution, &request)
    }

    /// Resolve a prompt, revalidating a caller-supplied request context.
    ///
    /// The supplied request must be identical to the one that created the
    /// prompt; otherwise the resolution is rejected (anti spoofing: a
    /// different origin/tab can never resolve someone else's prompt).
    pub fn resolve_checked(
        &mut self,
        prompt_id: PromptId,
        resolution: PromptResolution,
        current: &PermissionRequest,
    ) -> Result<PermissionDecision, PromptError> {
        let pending = self
            .pending
            .get(&prompt_id)
            .ok_or(PromptError::PromptNotFound)?;
        if pending.request != *current {
            return Err(PromptError::ContextMismatch);
        }
        if !current.user_gesture() {
            return Err(PromptError::UserGestureRequired);
        }

        let decision = match resolution {
            PromptResolution::Allow(lifetime) => {
                self.store
                    .grant(current, lifetime)
                    .map_err(|error| PromptError::InvalidRequest(error.to_string()))?;
                let expires_at = match lifetime {
                    GrantLifetime::OneShot | GrantLifetime::Session => None,
                    GrantLifetime::Persistent { expires_at } => expires_at,
                };
                PermissionDecision::Granted { expires_at }
            }
            PromptResolution::Deny => PermissionDecision::Denied {
                reason: DenyReason::DefaultDeny,
            },
        };
        self.pending.remove(&prompt_id);
        Ok(decision)
    }

    /// Inspect a pending prompt's display state.
    pub fn prompt_state(&self, prompt_id: PromptId) -> Option<PromptState> {
        self.pending.get(&prompt_id).map(|pending| PromptState {
            prompt_id,
            origin: pending.request.requesting_origin().clone(),
            top_level: pending.request.top_level_site().clone(),
            requested_at: pending.request.requested_at(),
        })
    }

    pub fn pending_count(&self) -> usize {
        self.pending.len()
    }

    pub fn store(&self) -> &PermissionStore {
        &self.store
    }
}
