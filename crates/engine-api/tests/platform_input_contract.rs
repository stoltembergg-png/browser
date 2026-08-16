use engine_api::contract::{InputEvent, PointerButton};
use engine_api::platform::{
    normalize_events, AccessibilityContract, AccessibilitySupport, FocusState,
    NormalizedPlatformEvent, PlatformInputEnvelope, PlatformInputError, PlatformOs, ScaleFactor,
    PLATFORM_CONTRACT_VERSION,
};
use engine_api::surface::Viewport;

const FIXTURES: &[(&str, &str, u32, u32)] = &[
    (
        "linux",
        include_str!("fixtures/platform-input/linux.json"),
        800,
        600,
    ),
    (
        "windows",
        include_str!("fixtures/platform-input/windows.json"),
        1000,
        800,
    ),
    (
        "macos",
        include_str!("fixtures/platform-input/macos.json"),
        1600,
        1200,
    ),
];

#[test]
fn per_os_fixtures_preserve_event_identity_and_order() {
    for (name, raw, width, height) in FIXTURES {
        let events: Vec<PlatformInputEnvelope> =
            serde_json::from_str(raw).expect("fixture must deserialize");
        let normalized = normalize_events(&events, Viewport::new(*width, *height))
            .expect("fixture must normalize");

        assert_eq!(
            events.len(),
            normalized.len(),
            "fixture {name} lost an event"
        );
        for (input, output) in events.iter().zip(&normalized) {
            assert_eq!(input.event_id(), output.event_id());
            assert_eq!(input.sequence(), output.sequence());
            assert_eq!(input.platform, output.platform());
        }
        assert!(matches!(
            normalized.first(),
            Some(NormalizedPlatformEvent::FocusChanged { focused: true, .. })
        ));
        assert!(matches!(
            normalized.last(),
            Some(NormalizedPlatformEvent::FocusChanged { focused: false, .. })
        ));
    }
}

#[test]
fn windows_scale_is_converted_to_device_pixels_deterministically() {
    let events: Vec<PlatformInputEnvelope> =
        serde_json::from_str(include_str!("fixtures/platform-input/windows.json"))
            .expect("Windows fixture must deserialize");
    let normalized = normalize_events(&events, Viewport::new(1000, 800)).expect("normalize");

    let pointer = normalized.iter().find_map(|event| match event {
        NormalizedPlatformEvent::Engine {
            event:
                InputEvent::PointerDown {
                    button: PointerButton::Left,
                    x,
                    y,
                },
            ..
        } => Some((*x, *y)),
        _ => None,
    });
    assert_eq!(pointer, Some((13, 25)));
}

#[test]
fn input_after_focus_loss_is_rejected_explicitly() {
    let event = PlatformInputEnvelope::focused(
        PlatformOs::Linux,
        2,
        2,
        ScaleFactor::one(),
        engine_api::platform::PlatformInputEvent::Text {
            text: "rejected".to_string(),
        },
        FocusState::Unfocused,
    );

    assert_eq!(
        normalize_events(&[event], Viewport::new(800, 600)),
        Err(PlatformInputError::FocusRequired)
    );
}

#[test]
fn invalid_scale_and_sequence_are_rejected() {
    assert!(ScaleFactor::new(0, 1).is_err());
    assert!(ScaleFactor::new(17, 1).is_err());

    let first = PlatformInputEnvelope::focused(
        PlatformOs::Linux,
        1,
        2,
        ScaleFactor::one(),
        engine_api::platform::PlatformInputEvent::FocusChanged { focused: true },
        FocusState::Focused,
    );
    let second = PlatformInputEnvelope::focused(
        PlatformOs::Linux,
        2,
        1,
        ScaleFactor::one(),
        engine_api::platform::PlatformInputEvent::FocusChanged { focused: false },
        FocusState::Unfocused,
    );
    assert_eq!(
        normalize_events(&[first, second], Viewport::new(800, 600)),
        Err(PlatformInputError::SequenceNotIncreasing)
    );
}

#[test]
fn screen_reader_bridge_is_explicitly_unsupported_per_os() {
    for platform in [PlatformOs::Linux, PlatformOs::Windows, PlatformOs::Macos] {
        let contract = AccessibilityContract::for_platform(platform);
        assert_eq!(contract.contract_version, PLATFORM_CONTRACT_VERSION);
        assert!(matches!(
            &contract.screen_reader_bridge,
            AccessibilitySupport::Unsupported { .. }
        ));
        assert!(contract.screen_reader_reason().is_some());
    }
}
