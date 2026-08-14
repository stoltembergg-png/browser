use browser_core::download_broker::{
    sanitize_filename, DownloadBroker, DownloadError, DownloadRequest,
};
use browser_domain::ids::{ProfileId, Url};
use std::fs;
use std::time::{SystemTime, UNIX_EPOCH};

fn temp_root(label: &str) -> std::path::PathBuf {
    let nonce = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("clock")
        .as_nanos();
    let path = std::env::temp_dir().join(format!("browser-pr040-{label}-{nonce}"));
    fs::create_dir_all(&path).expect("temp root");
    path
}

fn request(name: &str) -> DownloadRequest {
    DownloadRequest::new(
        ProfileId::new("profile-1"),
        Url::new("https://example.test/file").expect("url"),
        name,
        None,
    )
}

#[test]
fn rejects_traversal_ads_and_device_names() {
    for name in ["../../escape.txt", "report.txt:secret", "CON.txt", "nul"] {
        let error = sanitize_filename(name).expect_err("unsafe filename must be rejected");
        assert!(matches!(
            error,
            DownloadError::Traversal
                | DownloadError::AlternateDataStream
                | DownloadError::DeviceName
        ));
    }
}

#[test]
fn collision_allocates_non_overwriting_name() {
    let root = temp_root("collision");
    fs::write(root.join("report.txt"), b"original").expect("existing file");
    let mut broker =
        DownloadBroker::new(root.clone(), ProfileId::new("profile-1"), 1024).expect("broker");

    let handle = broker.start(request("report.txt")).expect("start");
    assert_eq!(handle.final_path().file_name().unwrap(), "report (1).txt");
    broker.write(handle.id(), b"new").expect("write");
    let completed = broker.finish(handle.id()).expect("finish");

    assert_eq!(
        fs::read(root.join("report.txt")).expect("original"),
        b"original"
    );
    assert_eq!(fs::read(completed.final_path()).expect("new"), b"new");
    fs::remove_dir_all(root).expect("cleanup");
}

#[test]
fn quota_and_interruption_never_finalize_files() {
    let root = temp_root("quota-interruption");
    let mut broker =
        DownloadBroker::new(root.clone(), ProfileId::new("profile-1"), 3).expect("broker");

    let too_large = broker.start(DownloadRequest::new(
        ProfileId::new("profile-1"),
        Url::new("https://example.test/large").expect("url"),
        "large.bin",
        Some(4),
    ));
    assert!(matches!(too_large, Err(DownloadError::QuotaExceeded)));

    let handle = broker.start(request("partial.bin")).expect("start");
    let temp_path = handle.temp_path().to_path_buf();
    let final_path = handle.final_path().to_path_buf();
    assert!(matches!(
        broker.write(handle.id(), b"1234"),
        Err(DownloadError::QuotaExceeded)
    ));
    assert_eq!(broker.active_count(), 0);
    assert!(!temp_path.exists());
    assert!(!final_path.exists());

    let interrupted = broker.start(request("interrupted.bin")).expect("start");
    let interrupted_temp = interrupted.temp_path().to_path_buf();
    let interrupted_final = interrupted.final_path().to_path_buf();
    broker.write(interrupted.id(), b"12").expect("write");
    broker.interrupt(interrupted.id()).expect("interrupt");
    assert!(!interrupted_temp.exists());
    assert!(!interrupted_final.exists());

    fs::remove_dir_all(root).expect("cleanup");
}

#[test]
fn profile_scope_and_quarantine_are_broker_owned() {
    let root = temp_root("quarantine");
    let mut broker = DownloadBroker::new(root.clone(), ProfileId::new("profile-1"), 1024)
        .expect("broker")
        .with_quarantine(true);

    let wrong_profile = broker.start(DownloadRequest::new(
        ProfileId::new("profile-2"),
        Url::new("https://example.test/file").expect("url"),
        "file.txt",
        None,
    ));
    assert!(matches!(wrong_profile, Err(DownloadError::WrongProfile)));

    let handle = broker.start(request("file.txt")).expect("start");
    let quarantine_root = fs::canonicalize(root.join(".quarantine")).expect("quarantine root");
    assert!(handle.final_path().starts_with(quarantine_root));
    broker.write(handle.id(), b"safe").expect("write");
    let completed = broker.finish(handle.id()).expect("finish");
    assert_eq!(
        fs::read(completed.final_path()).expect("quarantine file"),
        b"safe"
    );
    assert!(!root.join("file.txt").exists());

    fs::remove_dir_all(root).expect("cleanup");
}
