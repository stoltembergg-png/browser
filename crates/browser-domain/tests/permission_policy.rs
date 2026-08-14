use browser_domain::ids::{ProfileId, TabId};
use browser_domain::permissions::{
    DenyReason, GrantLifetime, Origin, PermissionDecision, PermissionKind, PermissionRequest,
    PermissionStore,
};

fn request(origin: &str, tab: &str, user_gesture: bool) -> PermissionRequest {
    request_at(origin, tab, "profile-1", user_gesture, 100)
}

fn request_at(
    origin: &str,
    tab: &str,
    profile: &str,
    user_gesture: bool,
    requested_at: u64,
) -> PermissionRequest {
    PermissionRequest::new(
        PermissionKind::Camera,
        Origin::new(origin).expect("origin"),
        Origin::new("https://top.example").expect("top origin"),
        None,
        ProfileId::new(profile),
        TabId::new(tab),
        user_gesture,
        requested_at,
    )
}

#[test]
fn default_deny_and_origin_confusion_are_rejected() {
    let mut store = PermissionStore::new();
    assert!(matches!(
        store.decide(&request("https://a.example", "tab-1", false)),
        PermissionDecision::Denied { .. }
    ));

    store
        .grant(
            &request("https://a.example", "tab-1", true),
            browser_domain::permissions::GrantLifetime::Session,
        )
        .expect("user-approved grant");
    assert!(matches!(
        store.decide(&request("https://a.example", "tab-1", false)),
        PermissionDecision::Granted { .. }
    ));
    assert!(matches!(
        store.decide(&request("https://b.example", "tab-1", false)),
        PermissionDecision::Denied { .. }
    ));
}

#[test]
fn grants_require_gesture_and_expire_or_consume_safely() {
    let mut store = PermissionStore::new();
    let no_gesture = request("https://a.example", "tab-1", false);
    assert_eq!(
        store.grant(&no_gesture, GrantLifetime::Session),
        Err(browser_domain::permissions::PermissionError::UserGestureRequired)
    );

    let expiring = request_at("https://a.example", "tab-1", "profile-1", true, 100);
    store
        .grant(
            &expiring,
            GrantLifetime::Persistent {
                expires_at: Some(200),
            },
        )
        .expect("grant");
    assert!(matches!(
        store.decide(&request_at(
            "https://a.example",
            "tab-1",
            "profile-1",
            false,
            199
        )),
        PermissionDecision::Granted {
            expires_at: Some(200)
        }
    ));
    assert!(matches!(
        store.decide(&request_at(
            "https://a.example",
            "tab-1",
            "profile-1",
            false,
            200
        )),
        PermissionDecision::Denied {
            reason: DenyReason::Expired
        }
    ));

    let one_shot = request("https://a.example", "tab-1", true);
    store
        .grant(&one_shot, GrantLifetime::OneShot)
        .expect("one shot");
    assert!(matches!(
        store.decide(&one_shot),
        PermissionDecision::Granted { .. }
    ));
    assert!(matches!(
        store.decide(&one_shot),
        PermissionDecision::Denied {
            reason: DenyReason::DefaultDeny
        }
    ));
}

#[test]
fn revoke_and_clear_are_scoped_to_tab_and_profile() {
    let mut store = PermissionStore::new();
    let tab_one = request("https://a.example", "tab-1", true);
    let tab_two = request("https://a.example", "tab-2", true);
    let other_profile = request_at("https://a.example", "tab-3", "profile-2", true, 100);
    store
        .grant(&tab_one, GrantLifetime::Session)
        .expect("grant tab one");
    store
        .grant(&tab_two, GrantLifetime::Session)
        .expect("grant tab two");
    store
        .grant(&other_profile, GrantLifetime::Session)
        .expect("grant other profile");

    assert!(store.revoke(&tab_one));
    assert!(!store.revoke(&tab_one));
    assert_eq!(
        store.clear_tab(&ProfileId::new("profile-1"), &TabId::new("tab-2")),
        1
    );
    assert!(matches!(
        store.decide(&other_profile),
        PermissionDecision::Granted { .. }
    ));
    assert_eq!(store.clear_profile(&ProfileId::new("profile-2")), 1);
    assert_eq!(store.active_grants(), 0);
}
