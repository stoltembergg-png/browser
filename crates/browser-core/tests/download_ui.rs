use browser_core::download_ui::DownloadUiCoordinator;
use browser_domain::ids::{ProfileId, RequestId, TabId};
use browser_domain::ui::{EventEnvelope, UiEvent, UI_CONTRACT_VERSION};
use std::fs;
use std::time::{SystemTime, UNIX_EPOCH};

fn temp_root(label: &str) -> std::path::PathBuf {
    let nonce = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("clock")
        .as_nanos();
    let path = std::env::temp_dir().join(format!("browser-pr041-ui-{label}-{nonce}"));
    fs::create_dir_all(&path).expect("temp root");
    fs::canonicalize(&path).expect("canonical temp root")
}

fn coordinator(root: &std::path::Path) -> DownloadUiCoordinator {
    DownloadUiCoordinator::new(root, ProfileId::new("profile-1"), 1024).expect("coordinator")
}

fn command_with_id(request_id: &str, payload: &str) -> String {
    format!(
        r#"{{"version":{UI_CONTRACT_VERSION},"request_id":"{request_id}","tab_id":null,"command":{payload}}}"#
    )
}

fn command(payload: &str) -> String {
    command_with_id("req-dl", payload)
}

fn assert_download_started(events: &[EventEnvelope]) -> u64 {
    let started = events
        .iter()
        .find_map(|envelope| match &envelope.event {
            UiEvent::DownloadStarted {
                download_id,
                suggested_name,
            } => Some((*download_id, suggested_name.clone())),
            _ => None,
        })
        .expect("download started event");
    assert_eq!(started.1, "f.bin");
    started.0
}

#[test]
fn download_start_command_emits_typed_events_and_tracks_state() {
    let root = temp_root("start");
    let mut coord = coordinator(&root);

    let events = coord
        .handle_command(&command(
            r#"{"type":"download_start","url":"https://example.test/f.bin","suggested_name":"f.bin","content_length":null}"#,
        ))
        .expect("start command");

    let id = assert_download_started(&events);
    assert!(events.iter().any(|envelope| matches!(
        &envelope.event,
        UiEvent::DownloadProgress { download_id, bytes: 0 } if *download_id == id
    )));
    assert_eq!(coord.active_count(), 1);
    fs::remove_dir_all(root).expect("cleanup");
}

#[test]
fn download_cancel_command_emits_cancelled_event() {
    let root = temp_root("cancel");
    let mut coord = coordinator(&root);

    let events = coord
        .handle_command(&command(
            r#"{"type":"download_start","url":"https://example.test/f.bin","suggested_name":"f.bin","content_length":null}"#,
        ))
        .expect("start command");
    let id = assert_download_started(&events);

    let cancel_events = coord
        .handle_command(&command_with_id(
            "req-cancel",
            &format!(r#"{{"type":"download_cancel","download_id":{id}}}"#),
        ))
        .expect("cancel command");
    assert!(cancel_events
        .iter()
        .any(|envelope| matches!(&envelope.event, UiEvent::DownloadCancelled { download_id } if *download_id == id)));
    assert_eq!(coord.active_count(), 0);
    fs::remove_dir_all(root).expect("cleanup");
}

#[test]
fn download_retry_command_restarts_from_history() {
    let root = temp_root("retry");
    let mut coord = coordinator(&root);

    let events = coord
        .handle_command(&command(
            r#"{"type":"download_start","url":"https://example.test/f.bin","suggested_name":"f.bin","content_length":null}"#,
        ))
        .expect("start command");
    let id = assert_download_started(&events);
    coord
        .handle_command(&command_with_id(
            "req-cancel",
            &format!(r#"{{"type":"download_cancel","download_id":{id}}}"#),
        ))
        .expect("cancel command");

    let retry_events = coord
        .handle_command(&command_with_id(
            "req-retry",
            &format!(r#"{{"type":"download_retry","download_id":{id}}}"#),
        ))
        .expect("retry command");
    let new_id = assert_download_started(&retry_events);
    assert_ne!(id, new_id);
    assert_eq!(coord.active_count(), 1);
    fs::remove_dir_all(root).expect("cleanup");
}

#[test]
fn retry_of_unknown_download_is_rejected() {
    let root = temp_root("retry-unknown");
    let mut coord = coordinator(&root);

    let error = coord
        .handle_command(&command(r#"{"type":"download_retry","download_id":4242}"#))
        .expect_err("unknown download must be rejected");
    assert!(error.to_string().contains("not found"));
    fs::remove_dir_all(root).expect("cleanup");
}

#[test]
fn invalid_download_payload_is_rejected_by_contract() {
    let root = temp_root("invalid");
    let mut coord = coordinator(&root);

    for payload in [
        r#"{"type":"download_start","url":"https://example.test/f.bin","suggested_name":"","content_length":null}"#,
        r#"{"type":"download_start","url":"","suggested_name":"f.bin","content_length":null}"#,
        r#"{"type":"unknown_command","payload":1}"#,
    ] {
        let error = coord
            .handle_command(&command(payload))
            .expect_err("invalid payload must be rejected");
        assert!(
            error.to_string().contains("rejected") || error.to_string().contains("invalid"),
            "unexpected error for {payload}: {error}"
        );
    }
    assert_eq!(coord.active_count(), 0);
    fs::remove_dir_all(root).expect("cleanup");
}

#[test]
fn coordinator_rejects_scoped_download_command_without_tab_scope() {
    let root = temp_root("scope");
    let mut coord = coordinator(&root);
    let _ = TabId::new("tab-1");
    let _ = RequestId("req-x".to_string());

    let scoped = format!(
        r#"{{"version":{UI_CONTRACT_VERSION},"request_id":"req-x","tab_id":"tab-1","command":{{"type":"download_start","url":"https://example.test/f.bin","suggested_name":"f.bin","content_length":null}}}}"#
    );
    let error = coord
        .handle_command(&scoped)
        .expect_err("download commands must be app-level");
    assert!(error.to_string().contains("tab"));
    fs::remove_dir_all(root).expect("cleanup");
}
