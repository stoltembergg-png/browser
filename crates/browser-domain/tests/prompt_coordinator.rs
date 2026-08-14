use browser_domain::ids::{ProfileId, TabId};
use browser_domain::permissions::{
    DenyReason, GrantLifetime, PermissionDecision, PermissionKind, PermissionRequest,
    PermissionStore,
};
use browser_domain::prompts::{PromptCoordinator, PromptError, PromptId, PromptResolution};
use std::time::{SystemTime, UNIX_EPOCH};

fn now() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("clock")
        .as_secs()
}

fn request(
    permission: PermissionKind,
    origin: &str,
    gesture: bool,
    tab: &str,
) -> PermissionRequest {
    PermissionRequest::new(
        permission,
        browser_domain::permissions::Origin::new(origin).expect("origin"),
        browser_domain::permissions::Origin::new("https://top.example").expect("origin"),
        None,
        ProfileId::new("profile-1"),
        TabId::new(tab),
        gesture,
        now(),
    )
}

#[test]
fn unknown_request_is_prompted_and_state_reports_pending() {
    let store = PermissionStore::new();
    let mut coord = PromptCoordinator::new(store);

    let req = request(
        PermissionKind::Geolocation,
        "https://a.example",
        true,
        "tab-1",
    );
    let prompt_id = coord.prompt_for(&req).expect("prompt created");

    let state = coord.prompt_state(prompt_id).expect("state");
    assert_eq!(state.origin.as_str(), "https://a.example");
    assert_eq!(state.top_level.as_str(), "https://top.example");
}

#[test]
fn pending_prompt_displays_verified_origin() {
    let store = PermissionStore::new();
    let mut coord = PromptCoordinator::new(store);

    let req = request(
        PermissionKind::Notifications,
        "https://a.example",
        true,
        "tab-1",
    );
    let prompt_id = coord.prompt_for(&req).expect("prompt created");

    let state = coord.prompt_state(prompt_id).expect("state");
    assert_eq!(state.origin.as_str(), "https://a.example");
    assert_eq!(state.top_level.as_str(), "https://top.example");
}

#[test]
fn allow_one_shot_grants_with_consumption() {
    let store = PermissionStore::new();
    let mut coord = PromptCoordinator::new(store);

    let req = request(PermissionKind::Camera, "https://a.example", true, "tab-1");
    let prompt_id = coord.prompt_for(&req).expect("prompt created");
    let resolution = coord
        .resolve(prompt_id, PromptResolution::Allow(GrantLifetime::OneShot))
        .expect("resolve");

    match resolution {
        PermissionDecision::Granted { .. } => {}
        other => panic!("expected granted, got {other:?}"),
    }

    let again = request(PermissionKind::Camera, "https://a.example", true, "tab-1");
    assert!(
        matches!(coord.prompt_for(&again), Err(PromptError::AlreadyGranted)),
        "one-shot grant answers the second request without a prompt"
    );

    let third = request(PermissionKind::Camera, "https://a.example", true, "tab-1");
    assert_eq!(
        coord.prompt_for(&third).expect("third prompt"),
        PromptId(prompt_id.0 + 1),
        "one-shot consumed, a later request must prompt again"
    );
}

#[test]
fn allow_session_grants_reuse_without_new_prompt() {
    let store = PermissionStore::new();
    let mut coord = PromptCoordinator::new(store);

    let req = request(PermissionKind::Storage, "https://a.example", true, "tab-1");
    let prompt_id = coord.prompt_for(&req).expect("prompt created");
    coord
        .resolve(prompt_id, PromptResolution::Allow(GrantLifetime::Session))
        .expect("resolve");

    let again = request(PermissionKind::Storage, "https://a.example", true, "tab-1");
    assert!(
        coord.prompt_for(&again).is_err(),
        "session grant must be reused without a prompt"
    );
}

#[test]
fn user_cancel_denies_and_remembers_nothing() {
    let store = PermissionStore::new();
    let mut coord = PromptCoordinator::new(store);

    let req = request(
        PermissionKind::Geolocation,
        "https://a.example",
        true,
        "tab-1",
    );
    let prompt_id = coord.prompt_for(&req).expect("prompt created");
    let resolution = coord
        .resolve(prompt_id, PromptResolution::Deny)
        .expect("resolve");

    assert_eq!(
        resolution,
        PermissionDecision::Denied {
            reason: DenyReason::DefaultDeny
        }
    );

    let again = request(
        PermissionKind::Geolocation,
        "https://a.example",
        true,
        "tab-1",
    );
    assert_eq!(
        coord.prompt_for(&again).expect("prompt again"),
        PromptId(prompt_id.0 + 1),
        "cancel must not create a grant, next request prompts again"
    );
}

#[test]
fn grant_without_gesture_is_rejected() {
    let store = PermissionStore::new();
    let mut coord = PromptCoordinator::new(store);

    let req = request(PermissionKind::Camera, "https://a.example", false, "tab-1");
    assert!(matches!(
        coord.prompt_for(&req),
        Err(PromptError::UserGestureRequired)
    ));
}

#[test]
fn resolving_same_prompt_twice_is_stale_and_rejected() {
    let store = PermissionStore::new();
    let mut coord = PromptCoordinator::new(store);

    let req = request(PermissionKind::Camera, "https://a.example", true, "tab-1");
    let prompt_id = coord.prompt_for(&req).expect("prompt created");
    coord
        .resolve(prompt_id, PromptResolution::Deny)
        .expect("first resolve");

    assert!(matches!(
        coord.resolve(prompt_id, PromptResolution::Allow(GrantLifetime::Session)),
        Err(PromptError::PromptNotFound)
    ));
}

#[test]
fn resolving_with_gestureless_request_is_context_mismatch() {
    let store = PermissionStore::new();
    let mut coord = PromptCoordinator::new(store);

    let req = request(PermissionKind::Camera, "https://a.example", true, "tab-1");
    let prompt_id = coord.prompt_for(&req).expect("prompt created");

    let gestureless = request(PermissionKind::Camera, "https://a.example", false, "tab-1");
    assert!(matches!(
        coord.resolve_checked(
            prompt_id,
            PromptResolution::Allow(GrantLifetime::Session),
            &gestureless
        ),
        Err(PromptError::ContextMismatch)
    ));
}

#[test]
fn resolve_with_unrelated_request_context_is_rejected() {
    let store = PermissionStore::new();
    let mut coord = PromptCoordinator::new(store);

    let req = request(PermissionKind::Camera, "https://a.example", true, "tab-1");
    let prompt_id = coord.prompt_for(&req).expect("prompt created");

    let other = request(PermissionKind::Camera, "https://b.example", true, "tab-2");
    assert!(matches!(
        coord.resolve_checked(prompt_id, PromptResolution::Deny, &other),
        Err(PromptError::ContextMismatch)
    ));
}

#[test]
fn duplicate_prompt_for_same_context_is_rejected() {
    let store = PermissionStore::new();
    let mut coord = PromptCoordinator::new(store);

    let req = request(
        PermissionKind::Geolocation,
        "https://a.example",
        true,
        "tab-1",
    );
    let first = coord.prompt_for(&req).expect("first prompt");
    assert!(matches!(
        coord.prompt_for(&req),
        Err(PromptError::DuplicatePrompt)
    ));
    assert_eq!(coord.pending_count(), 1);
    let _ = first;
}

#[test]
fn prompt_for_existing_active_grant_never_shows() {
    let store = PermissionStore::new();
    let mut coord = PromptCoordinator::new(store);

    let req = request(
        PermissionKind::Notifications,
        "https://a.example",
        true,
        "tab-1",
    );
    let prompt_id = coord.prompt_for(&req).expect("first prompt");
    coord
        .resolve(prompt_id, PromptResolution::Allow(GrantLifetime::Session))
        .expect("resolve");

    let again = request(
        PermissionKind::Notifications,
        "https://a.example",
        true,
        "tab-1",
    );
    let state = coord.prompt_for(&again);
    assert!(
        matches!(state, Err(PromptError::AlreadyGranted)),
        "an active grant must suppress the prompt"
    );
}
