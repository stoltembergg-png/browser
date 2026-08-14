use browser_core::session_lifecycle::{
    PendingWork, RestoreDisposition, SessionLifecycle, SessionPhase,
};
use browser_domain::ids::{EngineInstanceId, ProfileId, TabId, Url};
use browser_domain::session::{SessionRecord, SessionTab};
use std::fs::OpenOptions;
use std::io::Write;

fn temp_path(label: &str) -> std::path::PathBuf {
    std::env::temp_dir().join(format!(
        "browser-pr036-{}-{}-{label}.journal",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .expect("clock")
            .as_nanos()
    ))
}

fn session(profile: &str, tab: &str, url: &str) -> SessionRecord {
    let profile_id = ProfileId::new(profile);
    let mut record = SessionRecord::new(profile_id, 1000);
    record.add_tab(SessionTab {
        tab_id: TabId::new(tab),
        engine_instance_id: EngineInstanceId::new("engine-1"),
        current_url: Some(Url::new(url).expect("URL")),
        title: "Example".into(),
        visible: true,
        created_at: 1000,
    });
    record.set_active(Some(0)).expect("active tab");
    record
}

#[test]
fn shutdown_quiesces_work_and_commits_a_session_atomically() {
    let path = temp_path("commit");
    let mut lifecycle = SessionLifecycle::open(&path).expect("open");

    let quiesced = lifecycle
        .begin_shutdown(PendingWork::new(2, 1))
        .expect("quiesce");
    assert_eq!(lifecycle.phase(), SessionPhase::Quiescing);
    assert_eq!(quiesced.cancelled_commands(), 2);
    assert_eq!(quiesced.cancelled_downloads(), 1);
    assert!(lifecycle.admit_work().is_err());

    let pending = lifecycle
        .prepare_session(session("profile-1", "tab-1", "https://example.test"))
        .expect("prepare");
    let receipt = lifecycle.commit_session(pending).expect("commit");
    assert_eq!(lifecycle.phase(), SessionPhase::Committed);
    assert_eq!(receipt.cancelled_commands(), 2);
    assert_eq!(
        lifecycle.restore().expect("restore").unwrap().tabs().len(),
        1
    );

    std::fs::remove_file(path).expect("cleanup");
}

#[test]
fn restore_returns_safe_placeholders_without_replaying_navigation() {
    let path = temp_path("placeholder");
    let mut lifecycle = SessionLifecycle::open(&path).expect("open");
    lifecycle
        .begin_shutdown(PendingWork::none())
        .expect("quiesce");
    let pending = lifecycle
        .prepare_session(session("profile-1", "tab-1", "https://example.test"))
        .expect("prepare");
    lifecycle.commit_session(pending).expect("commit");

    let restored = lifecycle.restore().expect("restore").unwrap();
    assert_eq!(
        restored.tabs()[0].disposition(),
        RestoreDisposition::Placeholder
    );
    assert_eq!(
        restored.tabs()[0].current_url().map(Url::as_str),
        Some("https://example.test")
    );

    std::fs::remove_file(path).expect("cleanup");
}

#[test]
fn interrupted_final_record_keeps_the_last_valid_snapshot() {
    let path = temp_path("torn");
    let mut lifecycle = SessionLifecycle::open(&path).expect("open");
    lifecycle
        .begin_shutdown(PendingWork::none())
        .expect("quiesce");
    let pending = lifecycle
        .prepare_session(session("profile-1", "tab-1", "https://stable.test"))
        .expect("prepare");
    lifecycle.commit_session(pending).expect("commit");
    drop(lifecycle);

    let mut file = OpenOptions::new()
        .append(true)
        .open(&path)
        .expect("append torn record");
    file.write_all(b"PR036|1|2|{\"version\":1")
        .expect("write torn record");
    drop(file);

    let reopened = SessionLifecycle::open(&path).expect("reopen");
    let restored = reopened.restore().expect("restore").unwrap();
    assert_eq!(
        restored.tabs()[0].current_url().map(Url::as_str),
        Some("https://stable.test")
    );

    std::fs::remove_file(path).expect("cleanup");
}

#[test]
fn failed_commit_aborts_shutdown_and_preserves_last_valid_snapshot() {
    let path = temp_path("failure");
    let mut initial = SessionLifecycle::open(&path).expect("open");
    initial
        .begin_shutdown(PendingWork::none())
        .expect("quiesce");
    let first = initial
        .prepare_session(session("profile-1", "tab-1", "https://stable.test"))
        .expect("prepare");
    initial.commit_session(first).expect("first commit");
    drop(initial);

    let mut failed = SessionLifecycle::open(&path).expect("reopen");
    std::fs::remove_file(&path).expect("remove journal");
    std::fs::create_dir(&path).expect("directory");
    failed.begin_shutdown(PendingWork::none()).expect("quiesce");
    let pending = failed
        .prepare_session(session("profile-1", "tab-2", "https://new.test"))
        .expect("prepare");
    assert!(failed.commit_session(pending).is_err());
    assert_eq!(failed.phase(), SessionPhase::Aborted);
    let restored = failed.restore().expect("restore").expect("last snapshot");
    assert_eq!(
        restored.tabs()[0].current_url().map(Url::as_str),
        Some("https://stable.test")
    );

    std::fs::remove_dir(path).expect("cleanup directory");
}

#[test]
fn shutdown_and_commit_are_single_use_but_abort_allows_retry() {
    let path = temp_path("retry");
    let mut lifecycle = SessionLifecycle::open(&path).expect("open");
    lifecycle
        .begin_shutdown(PendingWork::none())
        .expect("quiesce");
    assert!(lifecycle.begin_shutdown(PendingWork::none()).is_err());

    let pending = lifecycle
        .prepare_session(session("profile-1", "tab-1", "https://retry.test"))
        .expect("prepare");
    lifecycle.abort_session(pending).expect("abort");
    assert_eq!(lifecycle.phase(), SessionPhase::Aborted);

    lifecycle
        .begin_shutdown(PendingWork::none())
        .expect("retry");
    let pending = lifecycle
        .prepare_session(session("profile-1", "tab-1", "https://retry.test"))
        .expect("prepare retry");
    lifecycle.commit_session(pending).expect("commit retry");
    assert_eq!(lifecycle.phase(), SessionPhase::Committed);

    std::fs::remove_file(path).expect("cleanup");
}
