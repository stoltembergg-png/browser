use engine_api::contract::{EngineCapabilities, EngineCommand, InputEvent, PointerButton};

#[test]
fn input_contract_roundtrips_without_floating_point_coordinates() {
    let command = EngineCommand::Input {
        event: InputEvent::PointerDown {
            button: PointerButton::Left,
            x: 12,
            y: 24,
        },
    };
    let json = serde_json::to_string(&command).expect("serialize input command");
    let decoded: EngineCommand = serde_json::from_str(&json).expect("deserialize input command");
    assert_eq!(decoded, command);
}

#[test]
fn input_requires_explicit_capability() {
    let command = EngineCommand::Input {
        event: InputEvent::Text {
            text: "typed".to_string(),
        },
    };
    assert!(EngineCapabilities::default().check(&command).is_err());
    assert!(EngineCapabilities {
        can_receive_input: true,
        ..EngineCapabilities::default()
    }
    .check(&command)
    .is_ok());
}
